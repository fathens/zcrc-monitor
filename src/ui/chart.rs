use common::types::YoctoValue;
use num_traits::ToPrimitive;

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
    pub y_labels: Vec<(String, f32)>,
    pub x_labels: Vec<(String, f32)>,
    pub chart_points: Vec<(f32, f32)>,
    pub line_path: String,
    pub data: EvalPeriodsData,
    /// 時間順に並んだ (timestamp, 元の periods 配列のインデックス)
    pub sorted_points: Vec<(i64, usize)>,
}

/// 評価期間チャートの再描画に必要なデータ
#[derive(Clone, Default)]
pub struct EvalPeriodsData {
    /// (timestamp, value, original_index) — 時系列順
    pub points: Vec<(i64, f64, usize)>,
    pub x_min: i64,
    pub x_max: i64,
    pub body_width: u32,
    pub height: u32,
}

impl EvalPeriodsData {
    pub fn plot_width(&self) -> u32 {
        self.body_width.saturating_sub(10) // margin_right=10
    }

    /// ビューポート内のデータ値を抽出する
    pub fn visible_values(&self, viewport_x: f32, visible_width: f32) -> Vec<f64> {
        let pw = self.plot_width() as f64;
        let x_range = (self.x_max - self.x_min) as f64;
        if x_range <= 0.0 || pw <= 0.0 {
            return self.points.iter().map(|(_, v, _)| *v).collect();
        }
        let left = viewport_x as f64;
        let right = (viewport_x + visible_width) as f64;
        let visible: Vec<f64> = self
            .points
            .iter()
            .filter_map(|&(ts, v, _)| {
                let x_px = (ts - self.x_min) as f64 / x_range * pw;
                if x_px >= left && x_px <= right {
                    Some(v)
                } else {
                    None
                }
            })
            .collect();
        if visible.is_empty() {
            self.points.iter().map(|(_, v, _)| *v).collect()
        } else {
            visible
        }
    }
}

// チャート共通の縦レイアウト定数
const CHART_MARGIN_TOP: u32 = 5;
const CHART_X_LABEL_SIZE: u32 = 30;

/// Y軸ラベルを等間隔で n 個生成し、各ラベルのテキストとピクセルY座標を返す。
pub fn calc_y_labels(
    y_min: f64,
    y_max: f64,
    n: usize,
    plot_top: f32,
    plot_bottom: f32,
) -> Vec<(String, f32)> {
    if n < 2 {
        let y = (plot_top + plot_bottom) / 2.0;
        return vec![(format_compact((y_min + y_max) / 2.0), y)];
    }
    (0..n)
        .map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let value = y_max - frac * (y_max - y_min); // top=max, bottom=min
            let y_px = plot_top + frac as f32 * (plot_bottom - plot_top);
            (format_compact(value), y_px)
        })
        .collect()
}

/// Y 軸範囲を計算する。
pub fn calc_y_range(values: &[f64]) -> (f64, f64) {
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let margin = (max_val - min_val).abs() * 0.1;
    let y_min = if (max_val - min_val).abs() < f64::EPSILON {
        min_val - 1.0
    } else {
        (min_val - margin).max(0.0)
    };
    let y_max = if (max_val - min_val).abs() < f64::EPSILON {
        max_val + 1.0
    } else {
        max_val + margin
    };
    (y_min, y_max)
}

/// データ点のピクセル座標を計算する
pub fn calc_eval_chart_points(data: &EvalPeriodsData, y_min: f64, y_max: f64) -> Vec<(f32, f32)> {
    let plot_top = CHART_MARGIN_TOP as f64;
    let plot_bottom = data.height as f64 - CHART_X_LABEL_SIZE as f64;
    let pw = data.plot_width() as f64;
    let x_range = (data.x_max - data.x_min) as f64;
    let y_range = y_max - y_min;

    data.points
        .iter()
        .map(|&(ts, v, _)| {
            let x_px = if x_range > 0.0 {
                (ts - data.x_min) as f64 / x_range * pw
            } else {
                pw / 2.0
            };
            let y_px = if y_range > 0.0 {
                plot_top + (1.0 - (v - y_min) / y_range) * (plot_bottom - plot_top)
            } else {
                (plot_top + plot_bottom) / 2.0
            };
            (x_px as f32, y_px as f32)
        })
        .collect()
}

/// (x_px, y_px) のスライスから SVG パス文字列を生成する
pub fn build_line_path_commands(points: &[(f32, f32)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut s = String::with_capacity(points.len() * 20);
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            s.push_str(&format!("M {} {}", x, y));
        } else {
            s.push_str(&format!(" L {} {}", x, y));
        }
    }
    s
}

/// X 軸ラベルを生成する（最大10個の等間隔ラベル）
pub fn calc_eval_x_labels(data: &EvalPeriodsData) -> Vec<(String, f32)> {
    if data.points.is_empty() {
        return vec![];
    }
    let pw = data.plot_width() as f64;
    let x_range = (data.x_max - data.x_min) as f64;
    let n = data.points.len().min(10);
    let step = if data.points.len() <= n {
        1
    } else {
        data.points.len() / n
    };

    data.points
        .iter()
        .step_by(step)
        .take(n)
        .map(|&(ts, _, _)| {
            let x_px = if x_range > 0.0 {
                (ts - data.x_min) as f64 / x_range * pw
            } else {
                pw / 2.0
            };
            let label = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%m/%d").to_string())
                .unwrap_or_default();
            (label, x_px as f32)
        })
        .collect()
}

/// 評価期間の initial_value を横スクロール折れ線グラフでレンダリングする。
pub fn render_eval_periods_chart(
    periods: &[EvaluationPeriodItem],
    height: u32,
) -> EvalPeriodsChart {
    if periods.is_empty() {
        return EvalPeriodsChart {
            y_labels: vec![],
            x_labels: vec![],
            chart_points: vec![],
            line_path: String::new(),
            data: EvalPeriodsData::default(),
            sorted_points: vec![],
        };
    }

    // データ準備（古い→新しい順、元のインデックスを保持）
    let mut points: Vec<(i64, f64, usize)> = periods
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            (
                p.start_time.timestamp(),
                yocto_to_f64(&p.initial_value),
                idx,
            )
        })
        .collect();
    points.sort_by_key(|(ts, _, _)| *ts);

    let t_min = points.first().unwrap().0;
    let t_max = points.last().unwrap().0;
    let x_min = if t_min == t_max { t_min - 86400 } else { t_min };
    let x_max = if t_min == t_max { t_max + 86400 } else { t_max };
    let body_width = 80_u32.saturating_mul(points.len() as u32).max(300);

    let chart_data = EvalPeriodsData {
        points: points.clone(),
        x_min,
        x_max,
        body_width,
        height,
    };

    let values: Vec<f64> = points.iter().map(|(_, v, _)| *v).collect();
    let (y_min, y_max) = calc_y_range(&values);

    let plot_top = CHART_MARGIN_TOP as f32;
    let plot_bottom = height as f32 - CHART_X_LABEL_SIZE as f32;
    let y_labels = calc_y_labels(y_min, y_max, 4, plot_top, plot_bottom);
    let chart_points = calc_eval_chart_points(&chart_data, y_min, y_max);
    let line_path = build_line_path_commands(&chart_points);
    let x_labels = calc_eval_x_labels(&chart_data);

    let sorted_points: Vec<(i64, usize)> = points.iter().map(|&(ts, _, idx)| (ts, idx)).collect();

    EvalPeriodsChart {
        y_labels,
        x_labels,
        chart_points,
        line_path,
        data: chart_data,
        sorted_points,
    }
}

/// ビューポート範囲に基づいて評価期間チャートを再計算する
pub fn rerender_eval_periods_for_viewport(
    data: &EvalPeriodsData,
    viewport_x: f32,
    visible_width: f32,
) -> (Vec<(f32, f32)>, String, Vec<(String, f32)>) {
    let visible_values = data.visible_values(viewport_x, visible_width);
    let (y_min, y_max) = calc_y_range(&visible_values);
    let plot_top = CHART_MARGIN_TOP as f32;
    let plot_bottom = data.height as f32 - CHART_X_LABEL_SIZE as f32;
    let chart_points = calc_eval_chart_points(data, y_min, y_max);
    let line_path = build_line_path_commands(&chart_points);
    let y_labels = calc_y_labels(y_min, y_max, 4, plot_top, plot_bottom);
    (chart_points, line_path, y_labels)
}

/// total_value_wnear の時系列推移を折れ線グラフでレンダリングする。
/// 生バイト列から slint::Image を構築する。
pub fn image_from_raw_rgb8(data: &[u8], width: u32, height: u32) -> Image {
    let mut pixel_buffer = SharedPixelBuffer::new(width, height);
    pixel_buffer.make_mut_bytes().copy_from_slice(data);
    Image::from_rgb8(pixel_buffer)
}

/// ピクセルデータを Vec<u8> として返す（Send 可能）。
pub fn render_line_chart_raw(
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<(String, f32)>) {
    let mut buf = vec![0u8; (width * height * 3) as usize];
    let labels = render_line_chart_into(&mut buf, holdings, width, height);
    (buf, labels)
}

fn render_line_chart_into(
    buf: &mut [u8],
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> Vec<(String, f32)> {
    let size = (width, height);
    let backend = BitMapBackend::with_buffer(buf, size);
    let root = backend.into_drawing_area();
    root.fill(&WHITE).unwrap();

    let mut labels = vec![];

    if !holdings.is_empty() {
        let values: Vec<f64> = holdings
            .iter()
            .rev()
            .map(|h| yocto_to_f64(&h.total_value_wnear))
            .collect();

        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let margin = (max_val - min_val).abs() * 0.1;
        let mut y_min = if (max_val - min_val).abs() < f64::EPSILON {
            min_val - 1.0
        } else {
            min_val - margin
        };
        if min_val >= 0.0 && y_min < 0.0 {
            y_min = 0.0;
        }
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
            .y_label_area_size(0)
            .build_cartesian_2d(0f64..x_max, y_min..y_max)
            .unwrap();

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .x_labels(6)
            .y_labels(0)
            .draw()
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                values.iter().enumerate().map(|(i, &v)| (i as f64, v)),
                BLUE.stroke_width(2),
            ))
            .unwrap();

        chart
            .draw_series(
                values
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| Circle::new((i as f64, v), 4, BLUE.filled())),
            )
            .unwrap();

        // caption(14) + margin(10) ≈ 24 top, x_label_area(30) + margin(10) = 40 bottom
        let plot_top = 10.0 + 24.0;
        let plot_bottom = height as f32 - 30.0 - 10.0;
        labels = calc_y_labels(y_min, y_max, 6, plot_top, plot_bottom);
    }

    root.present().unwrap();
    labels
}

/// ピクセルデータを Vec<u8> として返す（Send 可能）。
pub fn render_token_lines_chart_raw(
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<(String, f32)>) {
    let mut buf = vec![0u8; (width * height * 3) as usize];
    let labels = render_token_lines_chart_into(&mut buf, holdings, width, height);
    (buf, labels)
}

fn render_token_lines_chart_into(
    buf: &mut [u8],
    holdings: &[PortfolioHoldingItem],
    width: u32,
    height: u32,
) -> Vec<(String, f32)> {
    use std::collections::BTreeMap;

    let size = (width, height);
    let backend = BitMapBackend::with_buffer(buf, size);
    let root = backend.into_drawing_area();
    root.fill(&WHITE).unwrap();

    let mut labels = vec![];

    if !holdings.is_empty() {
        // トークン名 → 時系列値を収集
        let mut token_series: BTreeMap<String, Vec<(usize, f64)>> = BTreeMap::new();
        for (i, h) in holdings.iter().rev().enumerate() {
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
        let mut y_min = if (max_val - min_val).abs() < f64::EPSILON {
            min_val - 1.0
        } else {
            min_val - margin
        };
        if min_val >= 0.0 && y_min < 0.0 {
            y_min = 0.0;
        }
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
            .y_label_area_size(0)
            .build_cartesian_2d(0f64..x_max, y_min..y_max)
            .unwrap();

        chart
            .configure_mesh()
            .disable_x_mesh()
            .disable_y_mesh()
            .x_labels(6)
            .y_labels(0)
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

        let plot_top = 10.0 + 24.0;
        let plot_bottom = height as f32 - 30.0 - 10.0;
        labels = calc_y_labels(y_min, y_max, 6, plot_top, plot_bottom);
    }

    root.present().unwrap();
    labels
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
