use crate::grpc::GrpcClient;
use crate::grpc::portfolio::{EvaluationPeriodItem, PortfolioHoldingItem, TokenHoldingItem};
use crate::ui::chart;
use crate::{AppWindow, SlintEvaluationPeriod, SlintPortfolioHolding, SlintTokenHolding};
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel, Weak};

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
    let c = client.clone();
    app.on_eval_period_select(move |index| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if app.get_eval_period_selected_index() == index {
            // 選択解除
            app.set_eval_period_selected_index(-1);
            clear_holdings(&app);
        } else {
            app.set_eval_period_selected_index(index);
            clear_holdings(&app);

            // period_id を取得してホールディングをフェッチ
            let periods = app.get_eval_periods();
            if let Some(ep) = periods.row_data(index as usize) {
                let period_id = ep.period_id.to_string();
                fetch_holdings(weak.clone(), c.clone(), period_id);
            }
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
        clear_holdings(&app);
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
        clear_holdings(&app);
        let page_size = app.get_eval_periods_page_size();
        refresh_eval_periods(weak.clone(), c.clone(), page, page_size);
    });
}

fn clear_holdings(app: &AppWindow) {
    app.set_eval_period_holdings(ModelRc::new(VecModel::<SlintPortfolioHolding>::default()));
    app.set_eval_period_holdings_error("".into());
    app.set_eval_period_holdings_loaded(false);
    app.set_line_chart_image(Image::default());
    app.set_bar_chart_image(Image::default());
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

fn fetch_holdings(weak: Weak<AppWindow>, client: GrpcClient, period_id: String) {
    spawn(async move {
        let result = crate::grpc::portfolio::get_portfolio_holdings(&client, &period_id).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(items) => {
                let line_chart = chart::render_line_chart(&items, 400, 250);
                let bar_chart = chart::render_bar_chart(&items, 400, 250);
                let slint_items: Vec<SlintPortfolioHolding> =
                    items.into_iter().map(to_slint_portfolio_holding).collect();
                app.set_eval_period_holdings(ModelRc::new(VecModel::from(slint_items)));
                app.set_eval_period_holdings_loaded(true);
                app.set_eval_period_holdings_error("".into());
                app.set_line_chart_image(line_chart);
                app.set_bar_chart_image(bar_chart);
                tracing::debug!("Portfolio holdings loaded for {period_id}");
            }
            Err(e) => {
                client.reset().await;
                app.set_eval_period_holdings_error(SharedString::from(e.clone()));
                tracing::warn!("Portfolio holdings fetch failed: {e}");
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

fn format_smallest_units(smallest_units: &str, decimals: u32) -> String {
    if decimals == 0 {
        return smallest_units.to_string();
    }
    let decimals = decimals as usize;
    let is_negative = smallest_units.starts_with('-');
    let digits: String = smallest_units
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();

    if digits.is_empty() || digits.chars().all(|c| c == '0') {
        return "0".to_string();
    }

    let (integer_part, fractional_part) = if digits.len() <= decimals {
        let padded = format!("{:0>width$}", digits, width = decimals + 1);
        let (i, f) = padded.split_at(padded.len() - decimals);
        (i.to_string(), f.to_string())
    } else {
        let (i, f) = digits.split_at(digits.len() - decimals);
        (i.to_string(), f.to_string())
    };

    let trimmed = fractional_part.trim_end_matches('0');
    let prefix = if is_negative { "-" } else { "" };
    if trimmed.is_empty() {
        format!("{prefix}{integer_part}")
    } else {
        format!("{prefix}{integer_part}.{trimmed}")
    }
}

fn to_slint_token_holding(item: TokenHoldingItem) -> SlintTokenHolding {
    let balance_display = format_smallest_units(&item.balance, item.decimals);
    let value_display = format_smallest_units(&item.value_wnear, 24);
    SlintTokenHolding {
        token: SharedString::from(item.token),
        balance: SharedString::from(balance_display),
        value_wnear: SharedString::from(value_display),
    }
}

fn to_slint_portfolio_holding(item: PortfolioHoldingItem) -> SlintPortfolioHolding {
    let token_holdings: Vec<SlintTokenHolding> = item
        .token_holdings
        .into_iter()
        .map(to_slint_token_holding)
        .collect();
    let total_display = format_smallest_units(&item.total_value_wnear, 24);
    SlintPortfolioHolding {
        timestamp: SharedString::from(item.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()),
        token_holdings: ModelRc::new(VecModel::from(token_holdings)),
        total_value_wnear: SharedString::from(total_display),
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

    #[test]
    fn to_slint_token_holding_maps_fields() {
        let item = TokenHoldingItem {
            token: "wrap.near".to_string(),
            balance: "1000000000000000000000000".to_string(),
            decimals: 24,
            value_wnear: "1000000000000000000000000".to_string(),
        };
        let slint = to_slint_token_holding(item);
        assert_eq!(slint.token, "wrap.near");
        assert_eq!(slint.balance, "1"); // 1e24 / 10^24 = 1
        assert_eq!(slint.value_wnear, "1"); // 1e24 yoctoNEAR = 1 NEAR
    }

    #[test]
    fn to_slint_portfolio_holding_maps_fields() {
        let item = PortfolioHoldingItem {
            timestamp: DateTime::from_timestamp(1700000000, 0).unwrap(),
            token_holdings: vec![
                TokenHoldingItem {
                    token: "wrap.near".to_string(),
                    balance: "100".to_string(),
                    decimals: 24,
                    value_wnear: "100".to_string(),
                },
                TokenHoldingItem {
                    token: "usdt.tether-token.near".to_string(),
                    balance: "5000000".to_string(),
                    decimals: 6,
                    value_wnear: "200".to_string(),
                },
            ],
            total_value_wnear: "300".to_string(),
        };
        let slint = to_slint_portfolio_holding(item);
        assert_eq!(slint.timestamp, "2023-11-14 22:13:20");
        assert_eq!(slint.token_holdings.row_count(), 2);
        let th0 = slint.token_holdings.row_data(0).unwrap();
        assert_eq!(th0.token, "wrap.near");
        let th1 = slint.token_holdings.row_data(1).unwrap();
        assert_eq!(th1.token, "usdt.tether-token.near");
    }

    #[test]
    fn to_slint_portfolio_holding_empty() {
        let item = PortfolioHoldingItem {
            timestamp: DateTime::<Utc>::default(),
            token_holdings: vec![],
            total_value_wnear: "0".to_string(),
        };
        let slint = to_slint_portfolio_holding(item);
        assert_eq!(slint.token_holdings.row_count(), 0);
        assert_eq!(slint.total_value_wnear, "0");
    }

    #[test]
    fn format_smallest_units_whole_number() {
        // 1 NEAR = 10^24 yoctoNEAR
        assert_eq!(format_smallest_units("1000000000000000000000000", 24), "1");
    }

    #[test]
    fn format_smallest_units_fractional() {
        // 1.5 NEAR
        assert_eq!(
            format_smallest_units("1500000000000000000000000", 24),
            "1.5"
        );
    }

    #[test]
    fn format_smallest_units_small_value() {
        // 0.001 NEAR
        assert_eq!(format_smallest_units("1000000000000000000000", 24), "0.001");
    }

    #[test]
    fn format_smallest_units_usdt() {
        // 5 USDT (decimals = 6)
        assert_eq!(format_smallest_units("5000000", 6), "5");
    }

    #[test]
    fn format_smallest_units_zero() {
        assert_eq!(format_smallest_units("0", 24), "0");
    }

    #[test]
    fn format_smallest_units_no_decimals() {
        assert_eq!(format_smallest_units("42", 0), "42");
    }
}
