use tracing_subscriber::{filter, prelude::*};

pub fn setup_logger() -> Result<(), tracing_subscriber::util::TryInitError> {
    let mut filter = filter::Targets::new().with_default(tracing::Level::INFO);

    if cfg!(debug_assertions) {
        filter = filter
            .with_target("kdeconnect", tracing::Level::DEBUG)
            .with_target("windows_audio_manager", tracing::Level::DEBUG);
    }

    let stderr_log = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(stderr_log)
        .with(filter)
        .try_init()
}
