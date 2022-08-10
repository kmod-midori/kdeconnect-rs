use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::{
    context::AppContextRef,
    packet::{NetworkPacket, NetworkPacketWithPayload},
};
use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use windows::{
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    },
    Storage::Streams::DataReader,
};

mod cache;

use super::{IncomingPacket, KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_MPRIS: &str = "kdeconnect.mpris";
const PACKET_TYPE_MPRIS_REQUEST: &str = "kdeconnect.mpris.request";
const COVER_URL_PREFIX: &str = "file:///";

fn log_if_error<R, E: std::fmt::Debug>(text: &str, res: Result<R, E>) {
    if let Err(e) = res {
        log::error!("{}: {:?}", text, e);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WindowsPlaybackInfo {
    can_go_next: bool,
    can_go_previous: bool,
    can_pause: bool,
    can_play: bool,
    is_playing: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsMediaMetadata {
    title: String,
    album: String,
    artist: String,
    player: String,
    now_playing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    album_art_url: Option<String>,
}

impl PartialEq for WindowsMediaMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.album == other.album
            && self.artist == other.artist
            && self.player == other.player
            && self.now_playing == other.now_playing
        // && self.album_art_url == other.album_art_url (do not compare album_art_url)
    }
}
impl Eq for WindowsMediaMetadata {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MprisMetadata {
    #[serde(flatten)]
    properties: WindowsMediaMetadata,
    #[serde(flatten)]
    status: WindowsPlaybackInfo,
}

/*
    can_seek: bool,
    length: u64,
    pos: u64,
    volume: u8,
*/

#[derive(Debug)]
struct CurrentSession {
    session: GlobalSystemMediaTransportControlsSession,
    media_props_token: EventRegistrationToken,
    playback_info_token: EventRegistrationToken,
}

impl Drop for CurrentSession {
    fn drop(&mut self) {
        self.session
            .RemoveMediaPropertiesChanged(self.media_props_token)
            .ok();
        self.session
            .RemovePlaybackInfoChanged(self.playback_info_token)
            .ok();
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum MprisPacket {
    #[serde(rename_all = "camelCase")]
    PlayerList {
        player_list: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        support_album_art_payload: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    TransferringAlbumArt {
        transferring_album_art: bool,
        album_art_url: String,
    },
    #[serde(rename_all = "camelCase")]
    Metadata(MprisMetadata),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MprisRequest {
    player: Option<String>,
    request_player_list: Option<bool>,
    request_now_playing: Option<bool>,
    request_volume: Option<bool>,
    album_art_url: Option<String>,
    #[serde(flatten)]
    commands: HashMap<String, Value>,
}

#[derive(Debug)]
pub struct MprisPlugin {
    ctx: OnceCell<AppContextRef>,
    manager: OnceCell<GlobalSystemMediaTransportControlsSessionManager>,
    sessions: Mutex<HashMap<String, CurrentSession>>,
    metadatas: Mutex<HashMap<String, MprisMetadata>>,
    album_art_cache: cache::AlbumArtCache,
    rt_handle: tokio::runtime::Handle,
}

impl MprisPlugin {
    pub fn new() -> Self {
        Self {
            ctx: OnceCell::new(),
            manager: OnceCell::new(),
            sessions: Mutex::new(HashMap::new()),
            metadatas: Mutex::new(HashMap::new()),
            album_art_cache: cache::AlbumArtCache::new(),
            rt_handle: tokio::runtime::Handle::current(),
        }
    }

    fn ctx(&self) -> &AppContextRef {
        self.ctx.get().expect("ctx is not initialized")
    }

    fn manager(&self) -> &GlobalSystemMediaTransportControlsSessionManager {
        self.manager.get().expect("manager is not initialized")
    }

    async fn update_metadata(&self, sid: &str) -> Result<()> {
        let sessions = self.sessions.lock().await;

        let session = if let Some(session) = sessions.get(sid) {
            session
        } else {
            log::warn!("Session {} not found", sid);
            return Ok(());
        };

        let metadata = session.session.TryGetMediaPropertiesAsync()?.await?;

        let title = metadata.Title()?.to_string_lossy();
        let artist = metadata.Artist()?.to_string_lossy();

        let playback_info = session.session.GetPlaybackInfo()?;
        let controls = playback_info.Controls()?;
        let status = playback_info.PlaybackStatus()?;

        let mut mm = MprisMetadata {
            properties: WindowsMediaMetadata {
                now_playing: format!("{} - {}", artist, title),
                title,
                album: metadata.AlbumTitle()?.to_string_lossy(),
                artist,
                player: session.session.SourceAppUserModelId()?.to_string_lossy(),
                album_art_url: None,
            },
            status: WindowsPlaybackInfo {
                can_go_next: controls.IsNextEnabled()?,
                can_go_previous: controls.IsPreviousEnabled()?,
                can_pause: controls.IsPauseEnabled()?,
                can_play: controls.IsPlayEnabled()?,
                is_playing: status
                    == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
            },
        };

        drop(sessions);

        let mut metadatas = self.metadatas.lock().await;
        let mut update_thumbnail = true;
        if let Some(current_metadata) = metadatas.get(sid) {
            if current_metadata == &mm && mm.properties.album_art_url.is_some() {
                // No need to update, we already have the thumbnail
                return Ok(());
            }
            
            // Metadata as a whole has changed
            if current_metadata.properties == mm.properties {
                // No need to update thumbnail
                update_thumbnail = false;
                mm.properties.album_art_url = current_metadata.properties.album_art_url.clone();
            }
        }

        if update_thumbnail || mm.properties.album_art_url.is_none() {
            log::info!("Loading thumbnail for {}", sid);

            let task = tokio::task::spawn_blocking(move || {
                let stream = metadata.Thumbnail()?.OpenReadAsync()?.get()?;
                let content_type = stream.ContentType()?.to_string_lossy();

                let extension = match content_type.as_str() {
                    "image/jpeg" => "jpg",
                    "image/png" => "png",
                    _ => {
                        anyhow::bail!("Unsupported content type: {}", content_type);
                    }
                };

                let size = stream.Size()? as u32;
                let data_loader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
                let loaded_size = data_loader.LoadAsync(size)?.get()?;

                if size != loaded_size {
                    anyhow::bail!(
                        "Failed to load full thumbnail image, {} full != {} loaded",
                        size,
                        loaded_size
                    );
                }

                let mut buffer = vec![0; loaded_size as usize];
                data_loader.ReadBytes(buffer.as_mut_slice())?;

                let filename = format!("{:x}.{}", md5::compute(buffer.as_slice()), extension);

                Ok::<_, anyhow::Error>((filename, buffer))
            });

            match task.await? {
                Ok((filename, buffer)) => {
                    log::info!("Thumbnail loaded for {} ({} bytes)", sid, buffer.len());
                    self.album_art_cache.put(&filename, buffer).await?;
                    mm.properties.album_art_url = Some(format!("{}{}", COVER_URL_PREFIX, filename));
                }
                Err(e) => {
                    log::warn!("Failed to load thumbnail: {:?}", e);
                }
            }
        }

        // Do update
        metadatas.insert(sid.to_string(), mm);
        drop(metadatas);
        self.send_now_playing(sid).await;

        Ok(())
    }

    async fn update_metadata_with_retry(&self, sid: &str) {
        log_if_error("Failed to update metadata", self.update_metadata(sid).await);

        // Some delay to ensure that thumbnail is populated
        tokio::time::sleep(Duration::from_secs(5)).await;

        log_if_error("Failed to update metadata", self.update_metadata(sid).await);
    }

    async fn init_session(
        self: Arc<Self>,
        session: GlobalSystemMediaTransportControlsSession,
    ) -> Result<CurrentSession> {
        let id = session.SourceAppUserModelId()?.to_string_lossy();

        let this = self.clone();
        let sid = id.clone();
        let media_props_token = session
            .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                log::debug!("MediaPropertiesChanged: {}", sid);

                let this = this.clone();
                let sid = sid.clone();

                this.rt_handle.clone().spawn(async move {
                    this.update_metadata_with_retry(&sid).await;
                });

                Ok(())
            }))
            .context("Subscribe to MediaPropertiesChanged")?;

        let this = self.clone();
        let sid = id.clone();
        let playback_info_token = session
            .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                log::debug!("PlaybackInfoChanged: {}", sid);

                let this = this.clone();
                let sid = sid.clone();

                this.rt_handle.clone().spawn(async move {
                    this.update_metadata_with_retry(&sid).await;
                });

                Ok(())
            }))
            .context("Subscribe to PlaybackInfoChanged")?;

        Ok(CurrentSession {
            session,
            media_props_token,
            playback_info_token,
        })
    }

    async fn handle_sessions_changed(self: Arc<Self>) -> Result<()> {
        log::info!("SessionsChanged");
        let manager = self.manager();

        let sessions = manager
            .GetSessions()
            .context("Get sessions")?
            .into_iter()
            .collect::<Vec<_>>();

        let mut ids = vec![];

        {
            let mut sessions_map = self.sessions.lock().await;
            sessions_map.clear();

            for session in sessions {
                let id = session.SourceAppUserModelId()?.to_string_lossy();

                match self.clone().init_session(session).await {
                    Ok(session) => {
                        ids.push(id.clone());
                        sessions_map.insert(id, session);
                    }
                    Err(e) => {
                        log::warn!("Failed to initialize session for {}: {:?}", id, e);
                    }
                }
            }
        }

        self.send_player_list().await;

        for id in ids {
            let this = self.clone();
            tokio::spawn(async move {
                this.update_metadata_with_retry(&id).await;
            });
        }

        Ok(())
    }

    async fn send_player_list(&self) {
        let players = {
            let sessions = self.sessions.lock().await;
            sessions.keys().cloned().collect::<Vec<_>>()
        };

        let packet = NetworkPacket::new(
            PACKET_TYPE_MPRIS,
            MprisPacket::PlayerList {
                player_list: players,
                support_album_art_payload: Some(true),
            },
        );

        self.ctx().device_manager.broadcast_packet(packet).await;
    }

    async fn send_now_playing(&self, sid: &str) {
        let metadatas = self.metadatas.lock().await;
        if let Some(current_metadata) = metadatas.get(sid) {
            let packet = NetworkPacket::new(
                PACKET_TYPE_MPRIS,
                MprisPacket::Metadata(current_metadata.clone()),
            );
            self.ctx().device_manager.broadcast_packet(packet).await;
        }
    }

    async fn send_album_art(&self, device_id: &str, filename: &str) {
        let data = match self.album_art_cache.get(filename).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                log::warn!("Album art not found: {}", filename);
                return;
            }
            Err(e) => {
                log::error!("Failed to get album art: {}", e);
                return;
            }
        };

        let packet = NetworkPacket::new(
            PACKET_TYPE_MPRIS,
            MprisPacket::TransferringAlbumArt {
                transferring_album_art: true,
                album_art_url: format!("{}{}", COVER_URL_PREFIX, filename),
            },
        );
        self.ctx()
            .device_manager
            .send_packet(device_id, NetworkPacketWithPayload::new(packet, data))
            .await;
    }

    async fn execute_commands(&self, sid: &str, commands: HashMap<String, Value>) -> Result<()> {
        let sessions = self.sessions.lock().await;
        let session = if let Some(session) = sessions.get(sid) {
            session
        } else {
            log::warn!("Session {} not found", sid);
            return Ok(());
        };

        for command in commands {
            match (command.0.as_str(), command.1) {
                ("action", Value::String(action)) => match action.as_str() {
                    "PlayPause" => {
                        session.session.TryTogglePlayPauseAsync()?.await?;
                    }
                    "Play" => {
                        session.session.TryPlayAsync()?.await?;
                    }
                    "Pause" => {
                        session.session.TryPauseAsync()?.await?;
                    }
                    "Stop" => {
                        session.session.TryStopAsync()?.await?;
                    }
                    "Previous" => {
                        session.session.TrySkipPreviousAsync()?.await?;
                    }
                    "Next" => {
                        session.session.TrySkipNextAsync()?.await?;
                    }
                    _ => {
                        log::warn!("Unsupported action: {}", action);
                    }
                },
                (cmd, val) => {
                    log::warn!("Unsupported command: {:?}", (cmd, val));
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for MprisPlugin {
    async fn start(self: Arc<Self>, ctx: AppContextRef) -> Result<()> {
        self.ctx.set(ctx).expect("ctx is already initialized");

        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.await?;
        self.manager
            .set(manager)
            .expect("manager is already initialized");

        let manager = self.manager();

        self.album_art_cache
            .start()
            .await
            .context("Initialize album art cache")?;

        let this = self.clone();
        manager.SessionsChanged(&TypedEventHandler::new(move |_, _| {
            let this = this.clone();
            this.rt_handle.clone().spawn(async move {
                log_if_error(
                    "Failed to handle SessionsChanged",
                    this.handle_sessions_changed().await,
                );
            });

            Ok(())
        }))?;

        log_if_error(
            "Failed to initialize sessions",
            self.handle_sessions_changed().await,
        );

        Ok(())
    }

    async fn handle(&self, packet: IncomingPacket) -> Result<()> {
        match packet.inner.typ.as_str() {
            PACKET_TYPE_MPRIS => {
                // let body: MprisPacket = packet.into_body()?;
                // dbg!(body);
            }
            PACKET_TYPE_MPRIS_REQUEST => {
                let body: MprisRequest = packet.inner.into_body()?;

                if body.request_player_list == Some(true) {
                    log::debug!("Request player list");

                    self.send_player_list().await;
                }

                if let (Some(id), Some(true)) = (&body.player, body.request_now_playing) {
                    log::debug!("Request now playing for {}", id);

                    self.send_now_playing(id).await;
                }

                if let Some(url) = &body.album_art_url {
                    log::debug!("Request album art: {}", url);

                    if url.len() > COVER_URL_PREFIX.len() {
                        let filename = &url[COVER_URL_PREFIX.len()..];
                        self.send_album_art(&packet.device_id, filename).await;
                    } else {
                        log::warn!("Invalid album art url (too short): {}", url);
                    }
                }

                if let (Some(id), true) = (&body.player, !body.commands.is_empty()) {
                    log::debug!("Request commands: {:?}", body.commands);

                    if let Err(e) = self.execute_commands(id, body.commands).await {
                        log::warn!("Failed to execute commands: {:?}", e);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for MprisPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_MPRIS.into(), PACKET_TYPE_MPRIS_REQUEST.into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_MPRIS.into(), PACKET_TYPE_MPRIS_REQUEST.into()]
    }
}
