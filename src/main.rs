slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    tracing_subscriber::fmt::init();
    tracing::info!("zcrc-monitor starting");

    let app = AppWindow::new()?;
    app.run()
}
