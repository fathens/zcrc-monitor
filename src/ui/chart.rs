use crate::grpc::portfolio::PortfolioHoldingItem;
use plotters::prelude::*;
use slint::{Image, SharedPixelBuffer};

/// yoctoNEAR 文字列 (10^24 が 1 NEAR) を f64 NEAR に変換する。
/// チャート表示用のため、精度低下は許容する。
pub fn ynear_to_near(ynear: &str) -> f64 {
    let digits: String = ynear.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return 0.0;
    }
    let is_negative = ynear.starts_with('-');
    let value = digits.parse::<f64>().unwrap_or(0.0) / 1e24;
    if is_negative { -value } else { value }
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
                .map(|h| ynear_to_near(&h.total_value_wnear))
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
                .caption("ポートフォリオ推移 (NEAR)", ("sans-serif", 14))
                .margin(10)
                .x_label_area_size(30)
                .y_label_area_size(60)
                .build_cartesian_2d(0f64..x_max, y_min..y_max)
                .unwrap();

            chart
                .configure_mesh()
                .x_desc("スナップショット")
                .y_desc("NEAR")
                .draw()
                .unwrap();

            chart
                .draw_series(LineSeries::new(
                    values.iter().enumerate().map(|(i, &v)| (i as f64, v)),
                    &BLUE,
                ))
                .unwrap();
        }

        root.present().unwrap();
    }

    Image::from_rgb8(pixel_buffer)
}

/// 最新スナップショットのトークン別価値を棒グラフでレンダリングする。
pub fn render_bar_chart(
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

        if let Some(latest) = holdings.last() {
            let token_holdings = &latest.token_holdings;
            if !token_holdings.is_empty() {
                let bar_data: Vec<(String, f64)> = token_holdings
                    .iter()
                    .map(|th| {
                        let label = shorten_token(&th.token);
                        let value = ynear_to_near(&th.value_wnear);
                        (label, value)
                    })
                    .collect();

                let max_val = bar_data
                    .iter()
                    .map(|(_, v)| *v)
                    .fold(f64::NEG_INFINITY, f64::max);
                let y_max = if max_val <= 0.0 { 1.0 } else { max_val * 1.2 };

                let mut chart = ChartBuilder::on(&root)
                    .caption("トークン別価値 (NEAR)", ("sans-serif", 14))
                    .margin(10)
                    .x_label_area_size(40)
                    .y_label_area_size(60)
                    .build_cartesian_2d(0..bar_data.len(), 0f64..y_max)
                    .unwrap();

                chart
                    .configure_mesh()
                    .x_labels(bar_data.len())
                    .x_label_formatter(&|idx| {
                        bar_data
                            .get(*idx)
                            .map(|(label, _)| label.clone())
                            .unwrap_or_default()
                    })
                    .y_desc("NEAR")
                    .draw()
                    .unwrap();

                chart
                    .draw_series(bar_data.iter().enumerate().map(|(i, (_, v))| {
                        let color = Palette99::pick(i);
                        Rectangle::new([(i, 0.0), (i + 1, *v)], color.filled())
                    }))
                    .unwrap();
            }
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

    #[test]
    fn ynear_to_near_one_near() {
        let result = ynear_to_near("1000000000000000000000000");
        assert!((result - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ynear_to_near_zero() {
        assert_eq!(ynear_to_near("0"), 0.0);
    }

    #[test]
    fn ynear_to_near_empty() {
        assert_eq!(ynear_to_near(""), 0.0);
    }

    #[test]
    fn ynear_to_near_fractional() {
        // 0.5 NEAR = 5 * 10^23
        let result = ynear_to_near("500000000000000000000000");
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ynear_to_near_large_value() {
        // 100 NEAR
        let result = ynear_to_near("100000000000000000000000000");
        assert!((result - 100.0).abs() < 1e-3);
    }

    #[test]
    fn ynear_to_near_negative() {
        let result = ynear_to_near("-1000000000000000000000000");
        assert!((result - (-1.0)).abs() < 1e-6);
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
        assert_eq!(
            shorten_token("usdt.tether-token.near"),
            "usdt"
        );
    }
}
