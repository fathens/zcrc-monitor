mod grpc;
mod ui;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    tracing_subscriber::fmt::init();
    tracing::info!("zcrc-monitor starting");

    let app = AppWindow::new()?;
    let client = grpc::GrpcClient::new();

    let _health_timer = ui::health::start_health_polling(&app, client);

    app.run()
}
