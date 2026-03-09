mod grpc;
mod ui;

slint::include_modules!();

pub fn app_main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let client = grpc::GrpcClient::new();

    let _health_timer = ui::health::start_health_polling(&app, client.clone());
    ui::config::setup_config_callbacks(&app, client.clone());
    ui::portfolio::setup_portfolio_callbacks(&app, client);

    app.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    slint::android::init(app).unwrap();
    app_main().unwrap();
}
