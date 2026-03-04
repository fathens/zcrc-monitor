use crate::grpc::GrpcClient;
use crate::grpc::portfolio::EvaluationPeriodItem;
use crate::{AppWindow, SlintEvaluationPeriod};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

pub fn setup_portfolio_callbacks(app: &AppWindow, client: GrpcClient) {
    // 初回ロード
    refresh_eval_periods(app.as_weak(), client.clone(), 0, 20);

    // eval-periods-refresh
    let weak = app.as_weak();
    let c = client.clone();
    app.on_eval_periods_refresh(move || {
        let (page, page_size) = get_page_info(&weak);
        refresh_eval_periods(weak.clone(), c.clone(), page, page_size);
    });

    // eval-period-select
    let weak = app.as_weak();
    app.on_eval_period_select(move |index| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if app.get_eval_period_selected_index() == index {
            app.set_eval_period_selected_index(-1);
        } else {
            app.set_eval_period_selected_index(index);
        }
    });

    // eval-periods-next-page
    let weak = app.as_weak();
    let c = client.clone();
    app.on_eval_periods_next_page(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let page = app.get_eval_periods_page() + 1;
        app.set_eval_periods_page(page);
        app.set_eval_period_selected_index(-1);
        let page_size = app.get_eval_periods_page_size();
        refresh_eval_periods(weak.clone(), c.clone(), page, page_size);
    });

    // eval-periods-prev-page
    let weak = app.as_weak();
    let c = client;
    app.on_eval_periods_prev_page(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let page = (app.get_eval_periods_page() - 1).max(0);
        app.set_eval_periods_page(page);
        app.set_eval_period_selected_index(-1);
        let page_size = app.get_eval_periods_page_size();
        refresh_eval_periods(weak.clone(), c.clone(), page, page_size);
    });
}

fn get_page_info(weak: &Weak<AppWindow>) -> (i32, i32) {
    weak.upgrade()
        .map(|app| {
            (
                app.get_eval_periods_page(),
                app.get_eval_periods_page_size(),
            )
        })
        .unwrap_or((0, 20))
}

fn refresh_eval_periods(weak: Weak<AppWindow>, client: GrpcClient, page: i32, page_size: i32) {
    spawn(async move {
        let result = crate::grpc::portfolio::get_evaluation_periods(&client, page, page_size).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok((items, total_count)) => {
                let slint_items: Vec<SlintEvaluationPeriod> =
                    items.into_iter().map(to_slint_eval_period).collect();
                app.set_eval_periods(ModelRc::new(VecModel::from(slint_items)));
                app.set_eval_periods_total_count(total_count as i32);
                app.set_eval_periods_loaded(true);
                app.set_eval_periods_error("".into());
                tracing::debug!("Evaluation periods loaded: page={page}, total={total_count}");
            }
            Err(e) => {
                client.reset().await;
                app.set_eval_periods_error(SharedString::from(e.clone()));
                tracing::warn!("Evaluation periods fetch failed: {e}");
            }
        }
    });
}

fn to_slint_eval_period(item: EvaluationPeriodItem) -> SlintEvaluationPeriod {
    SlintEvaluationPeriod {
        id: item.id,
        period_id: SharedString::from(item.period_id),
        start_time: SharedString::from(item.start_time.format("%Y-%m-%d %H:%M:%S").to_string()),
        initial_value: SharedString::from(item.initial_value),
        selected_tokens: SharedString::from(item.selected_tokens.join(", ")),
    }
}

fn spawn(future: impl std::future::Future<Output = ()> + 'static) {
    if let Err(e) = slint::spawn_local(async_compat::Compat::new(future)) {
        tracing::error!("Failed to spawn portfolio task: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    #[test]
    fn to_slint_eval_period_maps_all_fields() {
        let item = EvaluationPeriodItem {
            id: 42,
            period_id: "eval_abc123".to_string(),
            start_time: DateTime::from_timestamp(1700000000, 0).unwrap(),
            initial_value: "1000000".to_string(),
            selected_tokens: vec!["token1".to_string(), "token2".to_string()],
        };
        let slint = to_slint_eval_period(item);
        assert_eq!(slint.id, 42);
        assert_eq!(slint.period_id, "eval_abc123");
        assert_eq!(slint.start_time, "2023-11-14 22:13:20");
        assert_eq!(slint.initial_value, "1000000");
        assert_eq!(slint.selected_tokens, "token1, token2");
    }

    #[test]
    fn to_slint_eval_period_empty_tokens() {
        let item = EvaluationPeriodItem {
            id: 1,
            period_id: "eval_empty".to_string(),
            start_time: DateTime::<Utc>::default(),
            initial_value: "0".to_string(),
            selected_tokens: vec![],
        };
        let slint = to_slint_eval_period(item);
        assert_eq!(slint.selected_tokens, "");
    }

    #[test]
    fn to_slint_eval_period_single_token() {
        let item = EvaluationPeriodItem {
            id: 2,
            period_id: "eval_single".to_string(),
            start_time: DateTime::from_timestamp(0, 0).unwrap(),
            initial_value: "500".to_string(),
            selected_tokens: vec!["only_one".to_string()],
        };
        let slint = to_slint_eval_period(item);
        assert_eq!(slint.selected_tokens, "only_one");
    }
}
