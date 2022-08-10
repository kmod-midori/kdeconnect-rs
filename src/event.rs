use tokio::sync::mpsc;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum KdeConnectEvent {
    ClipboardUpdated,
    HotkeyPressed,
}

pub type EventSender = mpsc::Sender<KdeConnectEvent>;
pub type EventReceiver = mpsc::Receiver<KdeConnectEvent>;