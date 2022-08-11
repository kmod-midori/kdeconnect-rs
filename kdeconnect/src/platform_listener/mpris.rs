use anyhow::Result;
use windows::{
    Foundation::TypedEventHandler, Media::Control::GlobalSystemMediaTransportControlsSessionManager,
};

use crate::event::EventSender;

pub fn start(tx: EventSender) -> Result<()> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;

    manager.SessionsChanged(&TypedEventHandler::new(move |_, _| {
        let _ = tx.blocking_send(crate::event::KdeConnectEvent::MediaSessionsChanged);
        Ok(())
    }))?;

    // Just leak the manager, we will never stop.
    Box::leak(Box::new(manager));

    Ok(())
}
