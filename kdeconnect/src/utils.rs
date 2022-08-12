use winrt_toast::{Text, Toast};

pub fn unix_ts_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn log_if_error<R, E: std::fmt::Debug>(text: &str, res: Result<R, E>) {
    if let Err(e) = res {
        log::error!("{}: {:?}", text, e);
    }
}

/// Creates a toast header that is used to display KDE Connect's own notifications.
pub fn global_toast_header() -> winrt_toast::Header {
    winrt_toast::Header::new("kdeconnect", "KDE Connect", "action=headerClick")
}

pub async fn simple_toast(title: &str, content: &str) {
    let mut toast = Toast::new();
    toast
        .header(crate::utils::global_toast_header())
        .text1(title);

    if !content.is_empty() {
        toast.text2(content);
    }

    // let manager = self.toast_manager.clone();
    // tokio::task::spawn_blocking(move || manager.show(&toast, None, None, None)).await??;
}
