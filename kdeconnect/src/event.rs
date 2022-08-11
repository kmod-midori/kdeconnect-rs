use tokio::sync::mpsc;

#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum KdeConnectEvent {
    ClipboardUpdated,
    HotkeyPressed,
    MediaSessionsChanged,
}

pub type EventSender = mpsc::Sender<KdeConnectEvent>;
pub type EventReceiver = mpsc::Receiver<KdeConnectEvent>;