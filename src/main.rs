fn main() -> Result<(), slint::PlatformError> {
    tracing_subscriber::fmt::init();
    tracing::info!("zcrc-monitor starting");
    zcrc_monitor::app_main()
}
