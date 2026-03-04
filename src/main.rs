mod grpc;
mod ui;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    tracing_subscriber::fmt::init();
    tracing::info!("zcrc-monitor starting");

    let app = AppWindow::new()?;
    let client = grpc::GrpcClient::new();

    let _health_timer = ui::health::start_health_polling(&app, client.clone());
    ui::config::setup_config_callbacks(&app, client.clone());
    ui::portfolio::setup_portfolio_callbacks(&app, client);

    app.run()
}
