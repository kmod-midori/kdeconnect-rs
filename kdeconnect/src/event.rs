use tao::menu::MenuId;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum KdeConnectEvent {
    ClipboardUpdated,
    PowerStatusUpdated,
    HotkeyPressed,
    MediaSessionsChanged,
    TrayMenuClicked(MenuId),
}

impl KdeConnectEvent {
    pub fn is_menu_clicked(&self, id: MenuId) -> bool {
        match self {
            KdeConnectEvent::TrayMenuClicked(id2) => &id == id2,
            _ => false,
        }
    }
}

pub type EventSender = mpsc::Sender<KdeConnectEvent>;
pub type EventReceiver = mpsc::Receiver<KdeConnectEvent>;
