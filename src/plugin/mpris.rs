use std::sync::Arc;

use crate::{context::AppContextRef, packet::NetworkPacket};
use anyhow::Result;
use once_cell::sync::OnceCell;
use windows::{
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    },
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug)]
struct MediaMetadata {
    title: String,
    subtitle: String,
    album_artist: String,
    album_title: String,
    artist: String,
    track_number: u32,
}

fn extract_metadata(session: &GlobalSystemMediaTransportControlsSession) -> Result<()> {
    let metadata = session.TryGetMediaPropertiesAsync()?.get()?;

    let m = MediaMetadata {
        title: metadata.Title()?.to_string_lossy(),
        subtitle: metadata.Subtitle()?.to_string_lossy(),
        album_artist: metadata.AlbumArtist()?.to_string_lossy(),
        album_title: metadata.AlbumTitle()?.to_string_lossy(),
        artist: metadata.Artist()?.to_string_lossy(),
        track_number: metadata.TrackNumber()? as u32,
    };

    dbg!(m);

    Ok(())
}

#[derive(Debug)]
struct CurrentSession {
    session: GlobalSystemMediaTransportControlsSession,
    media_props_token: EventRegistrationToken,
    playback_info_token: EventRegistrationToken,
}

#[derive(Debug)]
pub struct MprisPlugin {
    ctx: OnceCell<AppContextRef>,
    manager: OnceCell<GlobalSystemMediaTransportControlsSessionManager>,
    current_session: std::sync::Mutex<Option<CurrentSession>>,
}

impl MprisPlugin {
    pub fn new() -> Self {
        Self {
            ctx: OnceCell::new(),
            manager: OnceCell::new(),
            current_session: std::sync::Mutex::new(None),
        }
    }

    fn ctx(&self) -> &AppContextRef {
        self.ctx.get().expect("ctx is not initialized")
    }

    fn manager(&self) -> &GlobalSystemMediaTransportControlsSessionManager {
        self.manager.get().expect("manager is not initialized")
    }

    fn handle_session(&self, session: GlobalSystemMediaTransportControlsSession) -> Result<()> {
        let mut current_session = self.current_session.lock().unwrap();

        if let Some(current_session) = current_session.as_ref() {
            // Remove event listeners
            current_session
                .session
                .RemoveMediaPropertiesChanged(current_session.media_props_token)
                .ok();
            current_session
                .session
                .RemovePlaybackInfoChanged(current_session.playback_info_token)
                .ok();
        }

        let media_props_token = session
            .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                log::info!("MediaPropertiesChanged");
                Ok(())
            }))
            .unwrap();
        let playback_info_token = session
            .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                log::info!("PlaybackInfoChanged");
                Ok(())
            }))
            .unwrap();

        extract_metadata(&session).ok();

        *current_session = Some(CurrentSession {
            session,
            media_props_token,
            playback_info_token,
        });

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

        if let Ok(current_session) = manager.GetCurrentSession() {
            self.handle_session(current_session)?;
        }

        let this = self.clone();
        manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
            log::info!("CurrentSessionChanged");
            let manager = this.manager();

            match manager.GetCurrentSession() {
                Ok(session) => {
                    let r = this.handle_session(session);
                    log::info!("Handle session: {:?}", r);
                }
                Err(e) => {
                    log::error!("Failed to get current session: {:?}", e);
                }
            }

            Ok(())
        }))?;
        manager.SessionsChanged(&TypedEventHandler::new(move |_, _| {
            log::info!("SessionsChanged");
            Ok(())
        }))?;

        Ok(())
    }

    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        dbg!(packet);
        Ok(())
    }
}

impl KdeConnectPluginMetadata for MprisPlugin {
    fn incomping_capabilities() -> Vec<String> {
        vec!["kdeconnect.mpris".into(), "kdeconnect.mpris.request".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec!["kdeconnect.mpris".into(), "kdeconnect.mpris.request".into()]
    }
}
