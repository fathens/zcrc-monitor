use num_traits::ToPrimitive;
use common::types::YoctoValue;

use crate::grpc::portfolio::{EvaluationPeriodItem, PortfolioHoldingItem};
use plotters::prelude::*;
use slint::{Image, SharedPixelBuffer};

/// YoctoValue を NEAR 単位の f64 に変換する。
fn yocto_to_f64(yocto: &YoctoValue) -> f64 {
    yocto.to_near().as_bigdecimal().to_f64().unwrap_or(0.0)
}

/// NEAR 値をコンパクトに表示する（例: 1234567 → "1.23M"）
pub fn format_compact(value: f64) -> String {
    let abs = value.abs();
    let (scaled, suffix) = if abs >= 1e18 {
        (value / 1e18, "E")
    } else if abs >= 1e15 {
        (value / 1e15, "P")
    } else if abs >= 1e12 {
        (value / 1e12, "T")
    } else if abs >= 1e9 {
        (value / 1e9, "B")
    } else if abs >= 1e6 {
        (value / 1e6, "M")
    } else if abs >= 1e3 {
        (value / 1e3, "K")
    } else {
        (value, "")
    };
    if suffix.is_empty() {
        format!("{scaled:.2}")
    } else {
        format!("{scaled:.2}{suffix}")
    }
}

/// 評価期間チャートの描画結果
pub struct EvalPeriodsChart {
    pub y_axis: Image,
    pub body: Image,
    pub body_width: u32,
    /// プロットエリアの幅（マージン除く）
    pub plot_width: u32,
    /// X 軸の時間範囲
    pub x_min: i64,
    pub x_max: i64,
    /// 時間順に並んだ (timestamp, 元の periods 配列のインデックス)
    pub sorted_points: Vec<(i64, usize)>,
}

// チャート共通の縦レイアウト定数
const CHART_MARGIN_TOP: u32 = 5;
const CHART_X_LABEL_SIZE: u32 = 30;
const CHART_Y_LABEL_SIZE: u32 = 60;
const Y_AXIS_WIDTH: u32 = 80;

/// Y 軸範囲を計算する。
fn calc_y_range(values: &[f64]) -> (f64, f64) {
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let margin = (max_val - min_val).abs() * 0.1;
    let y_min = if (max_val - min_val).abs() < f64::EPSILON {
        min_val - 1.0
    } else {
        (min_val - margin).min(0.0)
    };
    let y_max = if (max_val - min_val).abs() < f64::EPSILON {
        max_val + 1.0
    } else {
        max_val + margin
    };
    (y_min, y_max)
}

/// 評価期間の initial_value を横スクロール折れ線グラフでレンダリングする。
/// Y軸ストリップ（固定表示用）と本体（Flickable 内）の2画像を返す。
pub fn render_eval_periods_chart(
    periods: &[EvaluationPeriodItem],
    height: u32,
) -> EvalPeriodsChart {
    if periods.is_empty() {
        return EvalPeriodsChart {
            y_axis: Image::default(),
            body: Image::default(),
            body_width: 0,
            plot_width: 0,
            x_min: 0,
            x_max: 0,
            sorted_points: vec![],
        };
    }

    // データ準備（古い→新しい順、元のインデックスを保持）
    let mut data: Vec<(i64, f64, usize)> = periods
        .iter()
        .enumerate()
        .map(|(idx, p)| (p.start_time.timestamp(), yocto_to_f64(&p.initial_value), idx))
        .collect();
    data.sort_by_key(|(ts, _, _)| *ts);

    let values: Vec<f64> = data.iter().map(|(_, v, _)| *v).collect();
    let (y_min, y_max) = calc_y_range(&values);

    let t_min = data.first().unwrap().0;
    let t_max = data.last().unwrap().0;
    let x_min = if t_min == t_max { t_min - 86400 } else { t_min };
    let x_max = if t_min == t_max { t_max + 86400 } else { t_max };

    let body_width = 80_u32.saturating_mul(data.len() as u32).max(300);

    // --- Y 軸ストリップ ---
    let y_axis = {
        let mut buf = SharedPixelBuffer::new(Y_AXIS_WIDTH, height);
        {
            let backend = BitMapBackend::with_buffer(buf.make_mut_bytes(), (Y_AXIS_WIDTH, height));
            let root = backend.into_drawing_area();
            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .margin_top(CHART_MARGIN_TOP)
                .margin_bottom(0)
                .margin_left(5)
                .margin_right(5)
                .y_label_area_size(CHART_Y_LABEL_SIZE)
                .x_label_area_size(CHART_X_LABEL_SIZE)
                .build_cartesian_2d(0f64..1f64, y_min..y_max)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .x_labels(0)
                .y_labels(5)
                .y_label_formatter(&|v| format_compact(*v))
                .draw()
                .unwrap();

            root.present().unwrap();
        }
        Image::from_rgb8(buf)
    };

    // --- チャート本体 ---
    let body = {
        let mut buf = SharedPixelBuffer::new(body_width, height);
        {
            let backend = BitMapBackend::with_buffer(buf.make_mut_bytes(), (body_width, height));
            let root = backend.into_drawing_area();
            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .margin_top(CHART_MARGIN_TOP)
                .margin_bottom(0)
                .margin_left(0)
                .margin_right(10)
                .y_label_area_size(0)
                .x_label_area_size(CHART_X_LABEL_SIZE)
                .build_cartesian_2d(x_min..x_max, y_min..y_max)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .x_labels(data.len().min(10))
                .x_label_formatter(&|ts| {
                    chrono::DateTime::from_timestamp(*ts, 0)
                        .map(|dt| dt.format("%m/%d").to_string())
                        .unwrap_or_default()
                })
                .y_labels(0)
                .draw()
                .unwrap();

            chart
                .draw_series(LineSeries::new(
                    data.iter().map(|&(ts, v, _)| (ts, v)),
                    BLUE.stroke_width(2),
                ))
                .unwrap();

            chart
                .draw_series(
                    data.iter()
                        .map(|&(ts, v, _)| Circle::new((ts, v), 4, BLUE.filled())),
                )
                .unwrap();

            root.present().unwrap();
        }
        Image::from_rgb8(buf)
    };

    let plot_width = body_width.saturating_sub(10); // margin_right=10
    let sorted_points: Vec<(i64, usize)> = data.iter().map(|&(ts, _, idx)| (ts, idx)).collect();

    EvalPeriodsChart {
        y_axis,
        body,
        body_width,
        plot_width,
        x_min,
        x_max,
        sorted_points,
    }
}

/// total_value_wnear の時系列推移を折れ線グラフでレンダリングする。
pub fn render_line_chart(
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> Image {
    let mut pixel_buffer = SharedPixelBuffer::new(width, height);
    let size = (width, height);

    {
        let backend = BitMapBackend::with_buffer(pixel_buffer.make_mut_bytes(), size);
        let root = backend.into_drawing_area();
        root.fill(&WHITE).unwrap();

        if !holdings.is_empty() {
            let values: Vec<f64> = holdings
                .iter()
                .map(|h| yocto_to_f64(&h.total_value_wnear))
                .collect();

            let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let margin = (max_val - min_val).abs() * 0.1;
            let y_min = if (max_val - min_val).abs() < f64::EPSILON {
                min_val - 1.0
            } else {
                min_val - margin
            };
            let y_max = if (max_val - min_val).abs() < f64::EPSILON {
                max_val + 1.0
            } else {
                max_val + margin
            };

            let x_max = (values.len() as f64 - 1.0).max(1.0);

            let mut chart = ChartBuilder::on(&root)
                .caption("Portfolio (NEAR)", ("sans-serif", 14))
                .margin(10)
                .x_label_area_size(30)
                .y_label_area_size(80)
                .build_cartesian_2d(0f64..x_max, y_min..y_max)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .x_labels(6)
                .y_labels(6)
                .y_label_formatter(&|v| format_compact(*v))
                .draw()
                .unwrap();

            chart
                .draw_series(LineSeries::new(
                    values.iter().enumerate().map(|(i, &v)| (i as f64, v)),
                    BLUE.stroke_width(2),
                ))
                .unwrap();

            // データポイントにマーカーを描画
            chart
                .draw_series(values.iter().enumerate().map(|(i, &v)| {
                    Circle::new((i as f64, v), 4, BLUE.filled())
                }))
                .unwrap();
        }

        root.present().unwrap();
    }

    Image::from_rgb8(pixel_buffer)
}

/// トークンごとの value_wnear 時系列推移を折れ線グラフでレンダリングする。
pub fn render_token_lines_chart(
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> Image {
    use std::collections::BTreeMap;

    let mut pixel_buffer = SharedPixelBuffer::new(width, height);
    let size = (width, height);

    {
        let backend = BitMapBackend::with_buffer(pixel_buffer.make_mut_bytes(), size);
        let root = backend.into_drawing_area();
        root.fill(&WHITE).unwrap();

        if !holdings.is_empty() {
            // トークン名 → 時系列値を収集
            let mut token_series: BTreeMap<String, Vec<(usize, f64)>> = BTreeMap::new();
            for (i, h) in holdings.iter().enumerate() {
                for th in &h.token_holdings {
                    let name = shorten_token(&th.token);
                    let value = yocto_to_f64(&th.value_wnear);
                    token_series.entry(name).or_default().push((i, value));
                }
            }

            // Y 軸の範囲を計算
            let all_values: Vec<f64> = token_series
                .values()
                .flat_map(|pts| pts.iter().map(|(_, v)| *v))
                .collect();
            let min_val = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let margin = (max_val - min_val).abs() * 0.1;
            let y_min = if (max_val - min_val).abs() < f64::EPSILON {
                min_val - 1.0
            } else {
                (min_val - margin).min(0.0)
            };
            let y_max = if (max_val - min_val).abs() < f64::EPSILON {
                max_val + 1.0
            } else {
                max_val + margin
            };

            let x_max = (holdings.len() as f64 - 1.0).max(1.0);

            let mut chart = ChartBuilder::on(&root)
                .caption("Token Value (NEAR)", ("sans-serif", 14))
                .margin(10)
                .x_label_area_size(30)
                .y_label_area_size(80)
                .build_cartesian_2d(0f64..x_max, y_min..y_max)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .x_labels(6)
                .y_labels(6)
                .y_label_formatter(&|v| format_compact(*v))
                .draw()
                .unwrap();

            // トークンごとに色分けして折れ線 + マーカー
            for (color_idx, (name, pts)) in token_series.iter().enumerate() {
                let color = Palette99::pick(color_idx);
                let color2 = Palette99::pick(color_idx);
                let color3 = Palette99::pick(color_idx);
                chart
                    .draw_series(LineSeries::new(
                        pts.iter().map(|&(i, v)| (i as f64, v)),
                        color.stroke_width(2),
                    ))
                    .unwrap()
                    .label(name.as_str())
                    .legend(move |(x, y)| {
                        Rectangle::new([(x, y - 5), (x + 10, y + 5)], color2.filled())
                    });

                chart
                    .draw_series(
                        pts.iter()
                            .map(|&(i, v)| Circle::new((i as f64, v), 3, color3.filled())),
                    )
                    .unwrap();
            }

            // 凡例を描画
            chart
                .configure_series_labels()
                .position(SeriesLabelPosition::UpperRight)
                .background_style(WHITE.mix(0.8))
                .border_style(BLACK)
                .label_font(("sans-serif", 11))
                .draw()
                .unwrap();
        }

        root.present().unwrap();
    }

    Image::from_rgb8(pixel_buffer)
}

/// トークン名を短縮する（例: "wrap.near" → "wrap"）
fn shorten_token(token: &str) -> String {
    token.split('.').next().unwrap_or(token).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_yocto(s: &str) -> YoctoValue {
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        YoctoValue::from_yocto(BigDecimal::from_str(s).unwrap())
    }

    #[test]
    fn yocto_to_f64_one_near() {
        let result = yocto_to_f64(&make_yocto("1000000000000000000000000"));
        assert!((result - 1.0).abs() < 1e-6);
    }

    #[test]
    fn yocto_to_f64_zero() {
        assert_eq!(yocto_to_f64(&YoctoValue::zero()), 0.0);
    }

    #[test]
    fn yocto_to_f64_fractional() {
        let result = yocto_to_f64(&make_yocto("500000000000000000000000"));
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn yocto_to_f64_large_value() {
        let result = yocto_to_f64(&make_yocto("100000000000000000000000000"));
        assert!((result - 100.0).abs() < 1e-3);
    }

    #[test]
    fn format_compact_small() {
        assert_eq!(format_compact(42.5), "42.50");
    }

    #[test]
    fn format_compact_thousands() {
        assert_eq!(format_compact(1500.0), "1.50K");
    }

    #[test]
    fn format_compact_millions() {
        assert_eq!(format_compact(2_500_000.0), "2.50M");
    }

    #[test]
    fn format_compact_billions() {
        assert_eq!(format_compact(5_856_815_104.0), "5.86B");
    }

    #[test]
    fn format_compact_trillions() {
        assert_eq!(format_compact(1.5e12), "1.50T");
    }

    #[test]
    fn shorten_token_with_dots() {
        assert_eq!(shorten_token("wrap.near"), "wrap");
    }

    #[test]
    fn shorten_token_no_dots() {
        assert_eq!(shorten_token("NEAR"), "NEAR");
    }

    #[test]
    fn shorten_token_long_name() {
        assert_eq!(shorten_token("usdt.tether-token.near"), "usdt");
    }
}
