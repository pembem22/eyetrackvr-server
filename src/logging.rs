use log::{LevelFilter, info};

#[cfg(target_os = "android")]
pub fn setup_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Trace) // limit log level
            .with_tag("RUST_ETFT") // logs will show under mytag tag
            .with_filter(
                android_logger::FilterBuilder::new()
                    .parse("info,eyetrackvr_server=debug")
                    .build(),
            ),
    );

    info!("Initialized logging");
}
