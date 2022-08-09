use std::{
    mem::MaybeUninit,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use context::AppContextRef;
use socket2::{Domain, Socket};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufStream},
    net::{TcpListener, TcpStream, UdpSocket},
};
use tokio_rustls::{
    rustls::{ClientConfig, ServerConfig, ServerName},
    TlsConnector, TlsAcceptor,
};

mod packet;
use packet::{IdentityPacket, NetworkPacket};
use trayicon::{Icon, MenuBuilder, MenuItem, TrayIconBuilder};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{DispatchMessageA, GetMessageA, TranslateMessage},
};

mod config;
mod context;
mod device;
mod plugin;
mod tls;

async fn udp_server(tcp_port: u16, ctx: AppContextRef) -> Result<()> {
    let socket = Socket::new(
        Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_broadcast(true)?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;

    let udp_socket = UdpSocket::from_std(socket.into())?;
    let broadcast_addr = (Ipv4Addr::BROADCAST, 1716u16);

    log::info!("UDP server started");

    let mut identity_packet = NetworkPacket::new_identity(
        tcp_port,
        ctx.plugin_repo.incoming_caps.clone(),
        ctx.plugin_repo.outgoing_caps.clone(),
        &ctx.config,
    );

    loop {
        if ctx.device_manager.active_device_count() == 0 {
            // Advertise our presence to all devices on the network if we have no active devices.
            identity_packet.reset_ts();
            let buf = serde_json::to_vec(&identity_packet)?;
            udp_socket.send_to(&buf, broadcast_addr).await?;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn open_tcp_server() -> Result<(TcpListener, u16)> {
    const MIN_PORT: u16 = 1716;
    const MAX_PORT: u16 = 1764;

    let mut last_error = None;

    for port in MIN_PORT..=MAX_PORT {
        let addr = (Ipv4Addr::UNSPECIFIED, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => return Ok((listener, port)),
            Err(err) => last_error = Some(err),
        }
    }

    return Err(last_error.unwrap().into());
}

async fn open_payload_tcp_server() -> Result<(TcpListener, u16)> {
    const MIN_PORT: u16 = 1765;

    let mut last_error = None;

    for port in MIN_PORT.. {
        let addr = (Ipv4Addr::UNSPECIFIED, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => return Ok((listener, port)),
            Err(err) => last_error = Some(err),
        }
    }

    return Err(last_error.unwrap().into());
}

async fn serve_payload(server: TcpListener, data: Arc<Vec<u8>>) -> Result<()> {
    let (stream, addr) = server.accept().await?;

    Ok(())
}

async fn handle_conn(
    stream: TcpStream,
    addr: SocketAddr,
    connector: TlsConnector,
    ctx: AppContextRef,
) -> Result<()> {
    let s2_socket = Socket::from(stream.into_std()?);
    s2_socket.set_keepalive(true)?;
    let mut stream = TcpStream::from_std(s2_socket.into())?;

    let mut remote_identity = vec![];
    loop {
        let b = stream.read_u8().await?;
        if b == 0x0A {
            break;
        }
        remote_identity.push(b);
    }

    let remote_identity_packet: NetworkPacket = serde_json::from_slice(&remote_identity)?;
    if remote_identity_packet.typ != packet::PACKET_TYPE_IDENTITY {
        bail!("Invalid packet type: {:?}", remote_identity_packet.typ);
    }
    let remote_identity = remote_identity_packet.into_body::<IdentityPacket>()?;
    let device_id = remote_identity.device_id.as_str();

    let stream = connector
        .connect(ServerName::IpAddress(addr.ip()), stream)
        .await?;
    let mut stream = BufStream::new(stream);

    log::info!(
        "Handshake successful for {} ({}) from {}",
        remote_identity.device_name,
        device_id,
        addr
    );

    let (conn_id, mut packet_rx) = ctx
        .device_manager
        .add_device(device_id, &remote_identity.device_name, addr)
        .await;

    loop {
        let mut line = String::new();

        tokio::select! {
            packet = packet_rx.recv() => {
                // Send packet
                if let Some(mut packet) = packet {
                    packet.push(0x0A);
                    match stream.write_all(&packet).await {
                        Ok(_) => {
                            log::info!("Write {} bytes to {}", packet.len(), addr);
                        }
                        Err(e) => {
                            log::error!("Failed to write to connection: {:?}", e);
                            break;
                        }
                    }
                } else {
                    log::info!("Device {} packet sender disconnected", device_id);
                    break;
                }
            }

            read_result = stream.read_line(&mut line) => {
                // Receive packet
                match read_result {
                    Ok(0) => {
                        log::warn!("Connection closed (EOF)");
                        break;
                    }
                    Err(e) => {
                        log::error!("Failed to read from connection: {:?}", e);
                        break;
                    }
                    Ok(_) => {
                        // We have actual data to process
                    }
                }

                match serde_json::from_str::<NetworkPacket>(&line) {
                    Ok(packet) => match packet.typ.as_str() {
                        packet::PACKET_TYPE_PAIR => {
                            NetworkPacket::new_pair(true)
                                .write_to_conn(&mut stream)
                                .await?;
                            log::info!("Accepted pairing request");
                        }
                        _ => {
                            let ctx = ctx.clone();
                            tokio::spawn(async move {
                                match ctx.plugin_repo.handle_packet(packet).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::error!("Failed to handle packet: {:?}", e);
                                    }
                                }
                            });
                        },
                    },
                    Err(err) => {
                        log::error!("Failed to parse packet: {:?}", err);
                    }
                }
            }
        }

        if let Err(e) = stream.flush().await {
            log::error!("Failed to flush stream: {:?}", e);
            break;
        }
    }

    // Wait for some time before removing device and notify the user.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    ctx.device_manager.remove_device(device_id, conn_id).await;

    Ok(())
}

async fn tcp_server(
    listener: TcpListener,
    connector: TlsConnector,
    ctx: AppContextRef,
) -> Result<()> {
    log::info!("TCP server started");

    loop {
        let (stream, addr) = listener.accept().await?;

        let connector = connector.clone();
        let ctx = ctx.clone();

        tokio::spawn(async move {
            let r = handle_conn(stream, addr, connector, ctx).await;
            match r {
                Ok(_) => {
                    // println!("Connection from {} closed", addr);
                }
                Err(err) => {
                    log::error!("Error handling connection: {}", err);
                }
            }
        });
    }
}

#[tokio::main]
async fn server_main() -> Result<()> {
    let (tcp_listener, tcp_port) = open_tcp_server().await?;

    log::info!("TCP port: {}", tcp_port);

    let config = config::Config::init_or_load("./config.json")?;

    let ctx = context::ApplicationContext::new(config)
        .await
        .context("Initialize context")?;

    let client_config = ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(Arc::new(tls::ServerVerifier))
        // .with_client_cert_verifier(Arc::new(ClientVerifier))
        .with_single_cert(
            vec![tokio_rustls::rustls::Certificate(
                ctx.config.tls_cert.clone(),
            )],
            tokio_rustls::rustls::PrivateKey(ctx.config.tls_key.clone()),
        )?;

    let server_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(tls::ClientVerifier))
        .with_single_cert(
            vec![tokio_rustls::rustls::Certificate(
                ctx.config.tls_cert.clone(),
            )],
            tokio_rustls::rustls::PrivateKey(ctx.config.tls_key.clone()),
        )?;

    let tls_connector = TlsConnector::from(Arc::new(client_config));
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    let uctx = ctx.clone();
    let udp_task = tokio::spawn(async move {
        let e = udp_server(tcp_port, uctx).await;
        log::warn!("UDP server exited with {:?}", e);
    });

    let tcp_task = tokio::spawn(async move {
        let e = tcp_server(tcp_listener, tls_connector, ctx).await;
        log::warn!("TCP server exited with {:?}", e);
    });

    udp_task.await?;
    tcp_task.await?;

    Ok(())
}

fn main() -> Result<()> {
    setup_logger().expect("Failed to set up logger");

    std::thread::spawn(|| {
        let r = server_main();
        if let Err(e) = r {
            log::error!("Server exited with error: {}", e);
        }
    });

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    enum Events {
        ClickTrayIcon,
        DoubleClickTrayIcon,
        Exit,
        Item1,
        Item2,
        Item3,
        Item4,
        CheckItem1,
        SubItem1,
        SubItem2,
        SubItem3,
    }

    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon = include_bytes!("icons/green.ico");
    let icon2 = include_bytes!("icons/red.ico");

    let second_icon = Icon::from_buffer(icon2, None, None).unwrap();
    let first_icon = Icon::from_buffer(icon, None, None).unwrap();

    let mut tray_icon = TrayIconBuilder::new()
        .sender(s)
        .icon_from_buffer(icon)
        // .tooltip("Cool Tray 👀 Icon")
        // .on_click(Events::ClickTrayIcon)
        // .on_double_click(Events::DoubleClickTrayIcon)
        .menu(
            MenuBuilder::new()
                .item("Item 3 Replace Menu 👍", Events::Item3)
                .item("Item 2 Change Icon Green", Events::Item2)
                .item("Item 1 Change Icon Red", Events::Item1)
                .separator()
                .checkable("This is checkable", true, Events::CheckItem1)
                .submenu(
                    "Sub Menu",
                    MenuBuilder::new()
                        .item("Sub item 1", Events::SubItem1)
                        .item("Sub Item 2", Events::SubItem2)
                        .item("Sub Item 3", Events::SubItem3),
                )
                .with(MenuItem::Item {
                    name: "Item Disabled".into(),
                    disabled: true, // Disabled entry example
                    id: Events::Item4,
                    icon: None,
                })
                .separator()
                .item("E&xit", Events::Exit),
        )
        .build()
        .unwrap();

    std::thread::spawn(move || {
        r.iter().for_each(|m| match m {
            Events::DoubleClickTrayIcon => {
                println!("Double click");
            }
            Events::ClickTrayIcon => {
                println!("Single click");
            }
            Events::Exit => {
                println!("Please exit");
            }
            Events::Item1 => {
                tray_icon.set_icon(&second_icon).unwrap();
            }
            Events::Item2 => {
                tray_icon.set_icon(&first_icon).unwrap();
            }
            Events::Item3 => {
                tray_icon
                    .set_menu(
                        &MenuBuilder::new()
                            .item("New menu item", Events::Item1)
                            .item("Exit", Events::Exit),
                    )
                    .unwrap();
            }
            e => {
                println!("{:?}", e);
            }
        })
    });

    loop {
        unsafe {
            let mut msg = MaybeUninit::uninit();

            let bret = GetMessageA(msg.as_mut_ptr(), HWND(0), 0, 0);

            if bret.as_bool() {
                TranslateMessage(msg.as_ptr());
                DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }

    Ok(())
}

fn setup_logger() -> Result<(), fern::InitError> {
    use fern::colors::{Color, ColoredLevelConfig};
    let colors = ColoredLevelConfig::new().info(Color::Green);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stderr())
        .apply()?;

    Ok(())
}
