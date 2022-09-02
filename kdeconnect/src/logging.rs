use fern::colors::{Color, ColoredLevelConfig};

pub fn setup_logger() -> Result<(), fern::InitError> {
    let colors = ColoredLevelConfig::new().info(Color::Green);

    let logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Info);

    // if cfg!(debug_assertions) {
    //     logger = logger.level_for("kdeconnect", log::LevelFilter::Debug);
    // }

    logger.chain(std::io::stderr()).apply()?;

    Ok(())
}
