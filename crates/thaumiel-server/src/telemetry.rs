use thaumiel_config::TelemetryConfig;

/// Initializes the global `tracing` subscriber. Call once, before anything
/// else logs. `RUST_LOG`/`THAUMIEL_TELEMETRY__LOG_LEVEL` controls verbosity;
/// `telemetry.json = true` switches to structured JSON logs for log
/// aggregators.
pub fn init(config: &TelemetryConfig) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);
    if config.json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }
}
