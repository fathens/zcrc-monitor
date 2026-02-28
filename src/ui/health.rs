use crate::AppWindow;
use crate::grpc::GrpcClient;
use slint::{ComponentHandle, Timer, TimerMode, Weak};
use std::time::Duration;

pub fn start_health_polling(app: &AppWindow, client: GrpcClient) -> Timer {
    let timer = Timer::default();
    let weak = app.as_weak();

    // 初回チェック
    poll_health(weak.clone(), client.clone());

    // 5 秒間隔で定期ポーリング
    timer.start(TimerMode::Repeated, Duration::from_secs(5), move || {
        poll_health(weak.clone(), client.clone());
    });

    timer
}

fn poll_health(weak: Weak<AppWindow>, client: GrpcClient) {
    if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
        let result = crate::grpc::health::check(&client).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(status) => {
                if status.healthy {
                    app.set_health_status("healthy".into());
                    app.set_health_status_text("正常".into());
                } else {
                    app.set_health_status("unhealthy".into());
                    app.set_health_status_text("異常".into());
                }
                app.set_health_db_status(status.database_status.into());
                app.set_health_error_message("".into());
                tracing::debug!("Health check OK: healthy={}", status.healthy);
            }
            Err(e) => {
                client.reset().await;
                app.set_health_status("disconnected".into());
                app.set_health_status_text("未接続".into());
                app.set_health_db_status("".into());
                app.set_health_error_message(e.clone().into());
                tracing::warn!("Health check failed: {e}");
            }
        }
    })) {
        tracing::error!("Failed to spawn health check task: {e}");
    }
}
