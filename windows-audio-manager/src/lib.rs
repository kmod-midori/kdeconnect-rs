use std::{
    collections::{HashMap, HashSet},
    ptr::null,
    sync::Arc,
};

use anyhow::Result;

use tokio::sync::{mpsc, oneshot};
use windows::{
    core::PCWSTR,
    Win32::{
        Devices::FunctionDiscovery::*,
        Foundation::BOOL,
        Media::Audio::{
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            *,
        },
        System::Com::*,
    },
};

#[derive(Debug)]
enum AudioEvent {
    SendSinkList,
    ReleaseDevice {
        id: String,
    },
    VolumeUpdated {
        id: Arc<String>,
        volume: u8,
        muted: bool,
    },
}

#[windows::core::implement(IMMNotificationClient)]
struct NotificationClient {
    sender: mpsc::Sender<AudioEvent>,
}

impl NotificationClient {
    fn send_sink_list(&self) {
        self.sender.blocking_send(AudioEvent::SendSinkList).ok();
    }

    fn send_release_device(&self, id: String) {
        self.sender
            .blocking_send(AudioEvent::ReleaseDevice { id })
            .ok();
    }
}

#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for NotificationClient {
    fn OnDeviceStateChanged(
        &self,
        pwstrdeviceid: &PCWSTR,
        dwnewstate: u32,
    ) -> windows::core::Result<()> {
        unsafe {
            log::debug!("OnDeviceStateChanged: {} to {}", pwstrdeviceid.display(), dwnewstate);
        }

        if dwnewstate == DEVICE_STATE_UNPLUGGED {
            return self.OnDeviceRemoved(pwstrdeviceid);
        }
        self.send_sink_list();
        Ok(())
    }

    fn OnDeviceAdded(&self, pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        unsafe {
            log::debug!("OnDeviceAdded: {}", pwstrdeviceid.display());
        }

        self.send_sink_list();
        Ok(())
    }

    fn OnDeviceRemoved(&self, pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        unsafe {
            log::debug!("OnDeviceRemoved: {}", pwstrdeviceid.display());

            match pwstrdeviceid.to_string() {
                Ok(s) => {
                    self.send_release_device(s);
                }
                Err(e) => {
                    log::warn!("Failed to decode device ID: {:?}", e);
                }
            }
        }

        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        _role: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        log::debug!("Default device changed: {:?}", flow);

        if flow == eRender {
            self.send_sink_list();
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY,
    ) -> windows::core::Result<()> {
        log::debug!("OnPropertyValueChanged");

        self.send_sink_list();
        Ok(())
    }
}

#[windows::core::implement(IAudioEndpointVolumeCallback)]
struct AudioEndpointVolumeCb {
    id: Arc<String>,
    sender: mpsc::Sender<AudioEvent>,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for AudioEndpointVolumeCb {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        log::debug!("AudioEndpointVolumeCb OnNotify: {}", self.id);

        if let Some(p) = unsafe { pnotify.as_ref() } {
            self.sender
                .blocking_send(AudioEvent::VolumeUpdated {
                    id: Arc::clone(&self.id),
                    volume: (p.fMasterVolume * 100.0) as u8,
                    muted: p.bMuted.as_bool(),
                })
                .ok();
        }
        Ok(())
    }
}

struct AudioSink {
    name: String,
    description: String,
    endpoint: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
    is_active: bool,
}

impl AudioSink {
    fn pause_callback(&mut self) -> Result<()> {
        unsafe {
            self.endpoint
                .UnregisterControlChangeNotify(&self.callback)?;
        }
        Ok(())
    }

    fn resume_callback(&mut self) -> Result<()> {
        unsafe {
            self.endpoint.RegisterControlChangeNotify(&self.callback)?;
        }
        Ok(())
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        unsafe {
            self.endpoint
                .UnregisterControlChangeNotify(&self.callback)
                .ok();
        }
    }
}

pub struct AudioManager {
    enumerator: IMMDeviceEnumerator,
    sinks: HashMap<String, AudioSink>,
    command_rx: mpsc::Receiver<AudioCommand>,
    subscribers: Vec<mpsc::Sender<AudioNotification>>,
}

impl AudioManager {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> AudioManagerHandle {
        let (command_tx, command_rx) = mpsc::channel(1);

        std::thread::spawn(move || {
            let enumerator = unsafe {
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
                    .expect("Failed to get device enumerator")
            };

            let this = Self {
                enumerator,
                sinks: HashMap::new(),
                command_rx,
                subscribers: Vec::new(),
            };

            if let Err(e) = this.manager_main() {
                log::error!("Audio manager failed: {:?}", e);
            }
        });

        AudioManagerHandle { command_tx }
    }

    fn update_sink_list(&mut self, event_tx: mpsc::Sender<AudioEvent>) -> Result<()> {
        let mut found_devices = HashSet::new();

        unsafe {
            let devices = self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let default_device = self
                .enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let default_device_id = default_device.GetId()?.display().to_string();

            for i in 0..devices.GetCount()? {
                let device = devices.Item(i)?;
                let id = device.GetId()?.display().to_string();

                found_devices.insert(id.clone());

                let property_store = device.OpenPropertyStore(STGM_READ)?;

                let name = property_store
                    .GetValue(&PKEY_Device_FriendlyName)?
                    .Anonymous
                    .Anonymous
                    .Anonymous
                    .pwszVal
                    .display()
                    .to_string();

                let desc = property_store
                    .GetValue(&PKEY_Device_DeviceDesc)?
                    .Anonymous
                    .Anonymous
                    .Anonymous
                    .pwszVal
                    .display()
                    .to_string();

                if let Some(sink) = self.sinks.get_mut(&id) {
                    sink.is_active = default_device_id == id;
                } else {
                    let endpoint = match device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                        Ok(e) => e,
                        Err(e) => {
                            log::warn!("Failed to create IAudioEndpointVolume for device: {:?}", e);
                            continue;
                        }
                    };

                    let callback = IAudioEndpointVolumeCallback::from(AudioEndpointVolumeCb {
                        id: Arc::new(id.clone()),
                        sender: event_tx.clone(),
                    });
                    if let Err(e) = endpoint.RegisterControlChangeNotify(&callback) {
                        log::warn!("Failed to register volume callback: {:?}", e);
                        continue;
                    }

                    self.sinks.insert(
                        id.clone(),
                        AudioSink {
                            name,
                            description: desc,
                            endpoint,
                            callback,
                            is_active: default_device_id == id,
                        },
                    );
                }
            }
        }

        self.sinks.retain(|id, _| found_devices.contains(id));

        Ok(())
    }

    fn gather_sink_info(&self) -> HashMap<String, AudioSinkInfo> {
        let mut ret = HashMap::new();

        for (id, sink) in self.sinks.iter() {
            let is_muted = unsafe { sink.endpoint.GetMute() }
                .unwrap_or(BOOL(0))
                .as_bool();
            let volume =
                unsafe { sink.endpoint.GetMasterVolumeLevelScalar() }.unwrap_or(0.0) * 100.0;

            ret.insert(
                id.clone(),
                AudioSinkInfo {
                    name: sink.name.clone(),
                    description: sink.description.clone(),
                    is_active: sink.is_active,
                    is_muted,
                    volume: volume as u8,
                },
            );
        }

        ret
    }

    fn update_sink_list_or_log(&mut self, notify_tx: mpsc::Sender<AudioEvent>) {
        if let Err(e) = self.update_sink_list(notify_tx) {
            log::warn!("Failed to update sink list: {:?}", e);
        }
    }

    async fn emit_notification(&mut self, notify: AudioNotification) {
        let mut failed = vec![];

        for tx in self.subscribers.iter() {
            if (tx.send(notify.clone()).await).is_err() {
                failed.push(tx.clone());
            }
        }

        // Remove any failed subscribers
        for tx in failed {
            self.subscribers.retain(|x| !x.same_channel(&tx));
        }
    }

    async fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::SubscribeNotification { sender } => {
                self.subscribers.push(sender);
            }
            AudioCommand::RequestAudioSinkInfo { reply } => {
                reply.send(self.gather_sink_info()).ok();
            }
            AudioCommand::SetVolume { id, volume } => {
                if let Some(sink) = self.sinks.get_mut(&id) {
                    let paused = sink.pause_callback().is_ok();

                    let volume = volume as f32 / 100.0;
                    if let Err(e) =
                        unsafe { sink.endpoint.SetMasterVolumeLevelScalar(volume, null()) }
                    {
                        log::warn!("Failed to set volume: {:?}", e);
                    }

                    if paused {
                        sink.resume_callback().ok();
                    }
                }
            }
            AudioCommand::SetMuted { id, muted } => {
                if let Some(sink) = self.sinks.get_mut(&id) {
                    let paused = sink.pause_callback().is_ok();

                    if let Err(e) = unsafe { sink.endpoint.SetMute(muted, null()) } {
                        log::warn!("Failed to set mute: {:?}", e);
                    }

                    if paused {
                        sink.resume_callback().ok();
                    }
                }
            }
        }
    }

    async fn handle_event(&mut self, event: AudioEvent, event_tx: &mpsc::Sender<AudioEvent>) {
        match event {
            AudioEvent::SendSinkList => {
                self.update_sink_list_or_log(event_tx.clone());
                self.emit_notification(AudioNotification::SinkListUpdated)
                    .await;
            }
            AudioEvent::ReleaseDevice { id } => {
                self.sinks.remove(&id);
                self.emit_notification(AudioNotification::SinkListUpdated)
                    .await;
            }
            AudioEvent::VolumeUpdated { id, volume, muted } => {
                if let Some(sink) = self.sinks.get(id.as_str()) {
                    self.emit_notification(AudioNotification::VolumeUpdated {
                        id,
                        name: sink.name.clone(),
                        volume,
                        muted,
                    })
                    .await;
                }
            }
        }
    }

    #[tokio::main(flavor = "current_thread")]
    async fn manager_main(mut self) -> Result<()> {
        let (event_tx, mut event_rx) = mpsc::channel(1);

        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE)?;
            self.update_sink_list_or_log(event_tx.clone());
        }

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    let event = if let Some(e) = event {
                        e
                    } else {
                        return Ok(());
                    };
                    self.handle_event(event, &event_tx).await;
                }
                command = self.command_rx.recv() => {
                    let command = if let Some(c) = command {
                        c
                    } else {
                        return Ok(());
                    };
                    self.handle_command(command).await;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioSinkInfo {
    pub name: String,
    pub description: String,
    pub is_active: bool,
    pub is_muted: bool,
    pub volume: u8,
}

#[derive(Debug, Clone)]
pub enum AudioNotification {
    SinkListUpdated,
    VolumeUpdated {
        id: Arc<String>,
        name: String,
        volume: u8,
        muted: bool,
    },
}

#[derive(Debug)]
enum AudioCommand {
    SubscribeNotification {
        sender: mpsc::Sender<AudioNotification>,
    },
    RequestAudioSinkInfo {
        reply: oneshot::Sender<HashMap<String, AudioSinkInfo>>,
    },
    SetVolume {
        id: String,
        volume: u8,
    },
    SetMuted {
        id: String,
        muted: bool,
    },
}

#[derive(Clone)]
pub struct AudioManagerHandle {
    command_tx: mpsc::Sender<AudioCommand>,
}

impl AudioManagerHandle {
    pub async fn get_audio_sink_info(&self) -> Result<HashMap<String, AudioSinkInfo>> {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.command_tx
            .send(AudioCommand::RequestAudioSinkInfo { reply: reply_tx })
            .await?;

        Ok(reply_rx.await?)
    }

    pub async fn subscribe_notification(&self) -> Result<mpsc::Receiver<AudioNotification>> {
        let (sender, receiver) = mpsc::channel(1);

        self.command_tx
            .send(AudioCommand::SubscribeNotification { sender })
            .await?;

        Ok(receiver)
    }

    pub async fn set_volume(&self, id: &str, volume: u8) -> Result<()> {
        self.command_tx
            .send(AudioCommand::SetVolume {
                id: id.to_owned(),
                volume,
            })
            .await?;

        Ok(())
    }

    pub async fn set_muted(&self, id: &str, muted: bool) -> Result<()> {
        self.command_tx
            .send(AudioCommand::SetMuted {
                id: id.to_owned(),
                muted,
            })
            .await?;

        Ok(())
    }
}
