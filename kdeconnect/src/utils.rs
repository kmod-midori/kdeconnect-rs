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