use num_traits::ToPrimitive;
use std::cell::RefCell;
use std::rc::Rc;

use crate::grpc::GrpcClient;
use crate::grpc::portfolio::{EvaluationPeriodItem, PortfolioHoldingItem, TokenHoldingItem};
use crate::ui::chart;
use crate::{
    AppWindow, SlintChartZone, SlintEvaluationPeriod, SlintPortfolioHolding, SlintTokenHolding,
};
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel, Weak};

/// チャートクリック・ホバー時に必要なメタデータ
#[derive(Default)]
struct ChartClickInfo {
    plot_width: u32,
    x_min: i64,
    x_max: i64,
    /// (timestamp, 元の periods 配列のインデックス)
    sorted_points: Vec<(i64, usize)>,
    /// 各ポイントのゾーン境界 (zone_start_px, zone_end_px)
    zone_boundaries: Vec<(f64, f64)>,
}

pub fn setup_portfolio_callbacks(app: &AppWindow, client: GrpcClient) {
    let click_info: Rc<RefCell<ChartClickInfo>> = Rc::new(RefCell::new(ChartClickInfo::default()));

    // 初回ロード
    refresh_eval_periods(app.as_weak(), client.clone(), click_info.clone(), 0, 20);

    // eval-periods-refresh
    let weak = app.as_weak();
    let c = client.clone();
    let ci = click_info.clone();
    app.on_eval_periods_refresh(move || {
        let (page, page_size) = get_page_info(&weak);
        refresh_eval_periods(weak.clone(), c.clone(), ci.clone(), page, page_size);
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
    let ci = click_info.clone();
    app.on_eval_periods_next_page(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let page = app.get_eval_periods_page() + 1;
        app.set_eval_periods_page(page);
        app.set_eval_period_selected_index(-1);
        clear_holdings(&app);
        let page_size = app.get_eval_periods_page_size();
        refresh_eval_periods(weak.clone(), c.clone(), ci.clone(), page, page_size);
    });

    // eval-periods-prev-page
    let weak = app.as_weak();
    let c = client.clone();
    let ci = click_info.clone();
    app.on_eval_periods_prev_page(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let page = (app.get_eval_periods_page() - 1).max(0);
        app.set_eval_periods_page(page);
        app.set_eval_period_selected_index(-1);
        clear_holdings(&app);
        let page_size = app.get_eval_periods_page_size();
        refresh_eval_periods(weak.clone(), c.clone(), ci.clone(), page, page_size);
    });

    // eval-period-chart-click: クリック位置から最寄りの期間を選択
    let weak = app.as_weak();
    let c = client;
    let ci = click_info;
    app.on_eval_period_chart_click(move |x_px| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let info = ci.borrow();
        if info.plot_width == 0 || info.sorted_points.is_empty() {
            return;
        }

        // ピクセル座標→タイムスタンプ変換
        let ratio = x_px as f64 / info.plot_width as f64;
        let t = info.x_min as f64 + ratio * (info.x_max - info.x_min) as f64;

        // 最寄りのデータポイントを探す
        let (_, nearest_idx) = info
            .sorted_points
            .iter()
            .min_by_key(|(ts, _)| ((*ts as f64 - t).abs() * 1000.0) as i64)
            .copied()
            .unwrap();

        app.set_eval_period_selected_index(nearest_idx as i32);
        clear_holdings(&app);

        let periods = app.get_eval_periods();
        if let Some(ep) = periods.row_data(nearest_idx) {
            let period_id = ep.period_id.to_string();
            fetch_holdings(weak.clone(), c.clone(), period_id);
        }
    });
}

/// 各データポイントのゾーン境界（ピクセル座標）を計算する。
/// 隣接するポイントとの中間点をゾーン境界とする。
fn calc_zone_boundaries(info: &ChartClickInfo) -> Vec<(f64, f64)> {
    if info.sorted_points.is_empty() || info.plot_width == 0 {
        return vec![];
    }
    let pw = info.plot_width as f64;
    let range = (info.x_max - info.x_min) as f64;
    if range <= 0.0 {
        return vec![(0.0, pw)];
    }

    // 各ポイントのピクセルX座標
    let positions: Vec<f64> = info
        .sorted_points
        .iter()
        .map(|&(ts, _)| (ts - info.x_min) as f64 / range * pw)
        .collect();

    let n = positions.len();
    let mut zones = Vec::with_capacity(n);
    for i in 0..n {
        let start = if i == 0 {
            0.0
        } else {
            (positions[i - 1] + positions[i]) / 2.0
        };
        let end = if i == n - 1 {
            pw
        } else {
            (positions[i] + positions[i + 1]) / 2.0
        };
        zones.push((start, end));
    }
    zones
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

fn refresh_eval_periods(
    weak: Weak<AppWindow>,
    client: GrpcClient,
    click_info: Rc<RefCell<ChartClickInfo>>,
    page: i32,
    page_size: i32,
) {
    spawn(async move {
        let result = crate::grpc::portfolio::get_evaluation_periods(&client, page, page_size).await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok((items, total_count)) => {
                let periods_chart = chart::render_eval_periods_chart(&items, 150);
                app.set_eval_periods_y_axis_image(periods_chart.y_axis);
                app.set_eval_periods_chart_body_image(periods_chart.body);
                app.set_eval_periods_chart_width(periods_chart.body_width as i32);

                // クリック・ホバー判定用メタデータを保存
                {
                    let mut info = click_info.borrow_mut();
                    info.plot_width = periods_chart.plot_width;
                    info.x_min = periods_chart.x_min;
                    info.x_max = periods_chart.x_max;
                    info.sorted_points = periods_chart.sorted_points;
                    info.zone_boundaries = calc_zone_boundaries(&info);

                    // ゾーン境界を Slint モデルとして設定
                    let slint_zones: Vec<SlintChartZone> = info
                        .zone_boundaries
                        .iter()
                        .map(|&(start, end)| SlintChartZone {
                            start: start as f32,
                            end: end as f32,
                        })
                        .collect();
                    app.set_chart_zones(ModelRc::new(VecModel::from(slint_zones)));
                }

                // 最新の期間を自動選択してホールディングをフェッチ
                if let Some(first) = items.first() {
                    let period_id = first.period_id.clone();
                    app.set_eval_period_selected_index(0);
                    fetch_holdings(weak.clone(), client.clone(), period_id);
                }

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
                // リストを先に表示
                let slint_items: Vec<SlintPortfolioHolding> =
                    items.iter().map(to_slint_portfolio_holding_ref).collect();
                app.set_eval_period_holdings(ModelRc::new(VecModel::from(slint_items)));
                app.set_eval_period_holdings_loaded(true);
                app.set_eval_period_holdings_error("".into());

                // チャートをバックグラウンドスレッドで描画（Vec<u8> は Send）
                let charts = tokio::task::spawn_blocking(move || {
                    let line = chart::render_line_chart_raw(&items, 400, 250);
                    let bar = chart::render_token_lines_chart_raw(&items, 400, 250);
                    (line, bar)
                })
                .await;

                if let Ok((line_raw, bar_raw)) = charts
                    && let Some(app) = weak.upgrade()
                {
                    app.set_line_chart_image(chart::image_from_raw_rgb8(&line_raw, 400, 250));
                    app.set_bar_chart_image(chart::image_from_raw_rgb8(&bar_raw, 400, 250));
                }
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
    let near = item.initial_value.to_near();
    let near_f64 = near.as_bigdecimal().to_f64().unwrap_or(0.0);
    let display = chart::format_compact(near_f64);
    SlintEvaluationPeriod {
        id: item.id,
        period_id: SharedString::from(item.period_id),
        start_time: SharedString::from(item.start_time.format("%Y-%m-%d %H:%M:%S").to_string()),
        initial_value: SharedString::from(format!("{display} NEAR")),
        selected_tokens: SharedString::from(item.selected_tokens.join(", ")),
    }
}

fn to_slint_token_holding_ref(item: &TokenHoldingItem) -> SlintTokenHolding {
    let balance_f64 = item.balance.to_whole().to_f64().unwrap_or(0.0);
    let balance_display = chart::format_compact(balance_f64);
    let near = item.value_wnear.to_near();
    let near_f64 = near.as_bigdecimal().to_f64().unwrap_or(0.0);
    let value_display = chart::format_compact(near_f64);
    SlintTokenHolding {
        token: SharedString::from(&item.token),
        balance: SharedString::from(balance_display),
        value_wnear: SharedString::from(format!("{value_display} NEAR")),
    }
}

fn to_slint_portfolio_holding_ref(item: &PortfolioHoldingItem) -> SlintPortfolioHolding {
    let token_holdings: Vec<SlintTokenHolding> = item
        .token_holdings
        .iter()
        .map(to_slint_token_holding_ref)
        .collect();
    let near = item.total_value_wnear.to_near();
    let near_f64 = near.as_bigdecimal().to_f64().unwrap_or(0.0);
    let total_display = chart::format_compact(near_f64);
    SlintPortfolioHolding {
        timestamp: SharedString::from(item.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()),
        token_holdings: ModelRc::new(VecModel::from(token_holdings)),
        total_value_wnear: SharedString::from(format!("{total_display} NEAR")),
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
    use bigdecimal::BigDecimal;
    use chrono::{DateTime, Utc};
    use common::types::{TokenAmount, YoctoValue};
    use std::str::FromStr;

    fn yocto(s: &str) -> YoctoValue {
        YoctoValue::from_yocto(BigDecimal::from_str(s).unwrap())
    }

    fn token_amount(s: &str, decimals: u8) -> TokenAmount {
        TokenAmount::from_smallest_units(BigDecimal::from_str(s).unwrap(), decimals)
    }

    #[test]
    fn to_slint_eval_period_maps_all_fields() {
        // 19.21 NEAR = 19210000000000000000000000 yoctoNEAR
        let item = EvaluationPeriodItem {
            id: 42,
            period_id: "eval_abc123".to_string(),
            start_time: DateTime::from_timestamp(1700000000, 0).unwrap(),
            initial_value: yocto("19210000000000000000000000"),
            selected_tokens: vec!["token1".to_string(), "token2".to_string()],
        };
        let slint = to_slint_eval_period(item);
        assert_eq!(slint.id, 42);
        assert_eq!(slint.period_id, "eval_abc123");
        assert_eq!(slint.start_time, "2023-11-14 22:13:20");
        assert_eq!(slint.initial_value, "19.21 NEAR");
        assert_eq!(slint.selected_tokens, "token1, token2");
    }

    #[test]
    fn to_slint_eval_period_empty_tokens() {
        let item = EvaluationPeriodItem {
            id: 1,
            period_id: "eval_empty".to_string(),
            start_time: DateTime::<Utc>::default(),
            initial_value: YoctoValue::zero(),
            selected_tokens: vec![],
        };
        let slint = to_slint_eval_period(item);
        assert_eq!(slint.selected_tokens, "");
    }

    #[test]
    fn to_slint_token_holding_maps_fields() {
        let item = TokenHoldingItem {
            token: "wrap.near".to_string(),
            balance: token_amount("1000000000000000000000000", 24),
            value_wnear: yocto("1000000000000000000000000"),
        };
        let slint = to_slint_token_holding_ref(&item);
        assert_eq!(slint.token, "wrap.near");
        assert_eq!(slint.balance, "1.00");
        assert_eq!(slint.value_wnear, "1.00 NEAR");
    }

    #[test]
    fn to_slint_portfolio_holding_maps_fields() {
        let item = PortfolioHoldingItem {
            timestamp: DateTime::from_timestamp(1700000000, 0).unwrap(),
            token_holdings: vec![
                TokenHoldingItem {
                    token: "wrap.near".to_string(),
                    balance: token_amount("1000000000000000000000000", 24),
                    value_wnear: yocto("1000000000000000000000000"),
                },
                TokenHoldingItem {
                    token: "usdt.tether-token.near".to_string(),
                    balance: token_amount("5000000", 6),
                    value_wnear: yocto("2000000000000000000000000"),
                },
            ],
            total_value_wnear: yocto("3000000000000000000000000"),
        };
        let slint = to_slint_portfolio_holding_ref(&item);
        assert_eq!(slint.timestamp, "2023-11-14 22:13:20");
        assert_eq!(slint.token_holdings.row_count(), 2);
        let th0 = slint.token_holdings.row_data(0).unwrap();
        assert_eq!(th0.token, "wrap.near");
        assert_eq!(th0.value_wnear, "1.00 NEAR");
        let th1 = slint.token_holdings.row_data(1).unwrap();
        assert_eq!(th1.token, "usdt.tether-token.near");
        assert_eq!(th1.balance, "5.00");
    }

    #[test]
    fn to_slint_portfolio_holding_empty() {
        let item = PortfolioHoldingItem {
            timestamp: DateTime::<Utc>::default(),
            token_holdings: vec![],
            total_value_wnear: YoctoValue::zero(),
        };
        let slint = to_slint_portfolio_holding_ref(&item);
        assert_eq!(slint.token_holdings.row_count(), 0);
        assert_eq!(slint.total_value_wnear, "0.00 NEAR");
    }

    // --- calc_zone_boundaries tests ---

    fn make_click_info(
        plot_width: u32,
        x_min: i64,
        x_max: i64,
        sorted_points: Vec<(i64, usize)>,
    ) -> ChartClickInfo {
        ChartClickInfo {
            plot_width,
            x_min,
            x_max,
            sorted_points,
            zone_boundaries: vec![],
        }
    }

    #[test]
    fn zone_boundaries_empty_points() {
        let info = make_click_info(400, 0, 100, vec![]);
        assert!(calc_zone_boundaries(&info).is_empty());
    }

    #[test]
    fn zone_boundaries_zero_plot_width() {
        let info = make_click_info(0, 0, 100, vec![(50, 0)]);
        assert!(calc_zone_boundaries(&info).is_empty());
    }

    #[test]
    fn zone_boundaries_single_point() {
        let info = make_click_info(400, 0, 100, vec![(50, 0)]);
        let zones = calc_zone_boundaries(&info);
        // 1ポイントの場合、ゾーンは全幅
        assert_eq!(zones.len(), 1);
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((zones[0].1 - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zone_boundaries_same_timestamps() {
        // x_min == x_max の場合、range <= 0 で全幅1ゾーン
        let info = make_click_info(400, 100, 100, vec![(100, 0)]);
        let zones = calc_zone_boundaries(&info);
        assert_eq!(zones.len(), 1);
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((zones[0].1 - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zone_boundaries_two_points_equal_spacing() {
        // 0 と 100 の2ポイント、plot_width=400
        // 位置: 0px, 400px → ゾーン: [0, 200), [200, 400)
        let info = make_click_info(400, 0, 100, vec![(0, 0), (100, 1)]);
        let zones = calc_zone_boundaries(&info);
        assert_eq!(zones.len(), 2);
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((zones[0].1 - 200.0).abs() < f64::EPSILON);
        assert!((zones[1].0 - 200.0).abs() < f64::EPSILON);
        assert!((zones[1].1 - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zone_boundaries_three_points() {
        // 0, 50, 100 の3ポイント、plot_width=400
        // 位置: 0px, 200px, 400px
        // ゾーン: [0, 100), [100, 300), [300, 400)
        let info = make_click_info(400, 0, 100, vec![(0, 0), (50, 1), (100, 2)]);
        let zones = calc_zone_boundaries(&info);
        assert_eq!(zones.len(), 3);
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((zones[0].1 - 100.0).abs() < f64::EPSILON);
        assert!((zones[1].0 - 100.0).abs() < f64::EPSILON);
        assert!((zones[1].1 - 300.0).abs() < f64::EPSILON);
        assert!((zones[2].0 - 300.0).abs() < f64::EPSILON);
        assert!((zones[2].1 - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zone_boundaries_uneven_spacing() {
        // 0, 20, 100 の3ポイント、plot_width=500
        // 位置: 0px, 100px, 500px
        // ゾーン: [0, 50), [50, 300), [300, 500)
        let info = make_click_info(500, 0, 100, vec![(0, 0), (20, 1), (100, 2)]);
        let zones = calc_zone_boundaries(&info);
        assert_eq!(zones.len(), 3);
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((zones[0].1 - 50.0).abs() < f64::EPSILON);
        assert!((zones[1].0 - 50.0).abs() < f64::EPSILON);
        assert!((zones[1].1 - 300.0).abs() < f64::EPSILON);
        assert!((zones[2].0 - 300.0).abs() < f64::EPSILON);
        assert!((zones[2].1 - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zone_boundaries_are_contiguous() {
        // ゾーン間にギャップがないことを確認
        let info = make_click_info(600, 0, 300, vec![(0, 0), (100, 1), (200, 2), (300, 3)]);
        let zones = calc_zone_boundaries(&info);
        assert_eq!(zones.len(), 4);
        // 最初のゾーンは0から
        assert!((zones[0].0 - 0.0).abs() < f64::EPSILON);
        // 最後のゾーンはplot_widthまで
        assert!((zones[3].1 - 600.0).abs() < f64::EPSILON);
        // 各ゾーンの end == 次のゾーンの start
        for i in 0..zones.len() - 1 {
            assert!(
                (zones[i].1 - zones[i + 1].0).abs() < f64::EPSILON,
                "Gap between zone {} and {}: {} vs {}",
                i,
                i + 1,
                zones[i].1,
                zones[i + 1].0
            );
        }
    }
}
