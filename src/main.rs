use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::Result;
use socket2::{Domain, Socket};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufStream},
    net::{TcpListener, TcpStream, UdpSocket},
};
use tokio_rustls::{
    rustls::{ClientConfig, ServerName},
    TlsConnector,
};

mod packet;
use packet::NetworkPacket;

use crate::packet::PacketType;

mod tls;

async fn udp_server(tcp_port: u16) -> Result<()> {
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

    loop {
        let identity_packet = NetworkPacket::new_identity(tcp_port);
        let buf = serde_json::to_vec(&identity_packet)?;

        udp_socket.send_to(&buf, broadcast_addr).await?;

        tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
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

async fn handle_conn(stream: TcpStream, addr: SocketAddr, connector: TlsConnector) -> Result<()> {
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
    // dbg!(remote_identity_packet);

    let stream = connector
        .connect(ServerName::IpAddress(addr.ip()), stream)
        .await?;
    let mut stream = BufReader::new(stream);

    // NetworkPacket::new(PacketType::ConnectivityReportRequest { request: true })
    //     .write_to_conn(&mut stream)
    //     .await?;

    loop {
        let mut line = String::new();
        let count = stream.read_line(&mut line).await?;

        if count == 0 {
            return Ok(());
        }

        match serde_json::from_str::<NetworkPacket>(&line) {
            Ok(packet) => match packet.body {
                PacketType::Pair { .. } => {
                    NetworkPacket::new_pair(true)
                        .write_to_conn(&mut stream)
                        .await?;
                    println!("Accepted pairing request");
                }
                p => {
                    dbg!(p);
                }
            },
            Err(err) => {
                dbg!(err);
            }
        }
    }
}

async fn tcp_server(listener: TcpListener, connector: TlsConnector) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await?;

        let connector = connector.clone();

        tokio::spawn(async move {
            let r = handle_conn(stream, addr, connector).await;
            match r {
                Ok(_) => {
                    // println!("Connection from {} closed", addr);
                }
                Err(err) => {
                    eprintln!("Error handling connection: {}", err);
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let (tcp_listener, tcp_port) = open_tcp_server().await?;

    println!("TCP port: {}", tcp_port);

    let (cert, key) = tls::load_or_generate_certs()?;

    let client_config = ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(Arc::new(tls::ServerVerifier))
        // .with_client_cert_verifier(Arc::new(ClientVerifier))
        .with_single_cert(
            vec![tokio_rustls::rustls::Certificate(cert)],
            tokio_rustls::rustls::PrivateKey(key),
        )?;

    let tls_connector = TlsConnector::from(Arc::new(client_config));

    let udp_task = tokio::spawn(async move {
        let e = udp_server(tcp_port).await;
        println!("udp_server exited with {:?}", e);
    });

    let tcp_task = tokio::spawn(async move {
        let e = tcp_server(tcp_listener, tls_connector).await;
        println!("tcp_server exited with {:?}", e);
    });

    udp_task.await?;
    tcp_task.await?;

    Ok(())
}
