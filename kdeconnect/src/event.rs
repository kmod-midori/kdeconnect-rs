use tao::menu::MenuId;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum SystemEvent {
    ClipboardUpdated,
    PowerStatusUpdated,
    HotkeyPressed,
    MediaSessionsChanged,
    TrayMenuClicked(MenuId),
}

impl SystemEvent {
    pub fn is_menu_clicked(&self, id: MenuId) -> bool {
        match self {
            SystemEvent::TrayMenuClicked(id2) => &id == id2,
            _ => false,
        }
    }
}

pub type EventSender = mpsc::Sender<SystemEvent>;
pub type EventReceiver = mpsc::Receiver<SystemEvent>;
