use num_traits::ToPrimitive;
use common::types::YoctoValue;

use crate::grpc::portfolio::PortfolioHoldingItem;
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
                .light_line_style(RGBColor(255, 255, 255))
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
                        let value = yocto_to_f64(&th.value_wnear);
                        (label, value)
                    })
                    .collect();

                let max_val = bar_data
                    .iter()
                    .map(|(_, v)| *v)
                    .fold(f64::NEG_INFINITY, f64::max);
                let y_max = if max_val <= 0.0 { 1.0 } else { max_val * 1.2 };

                let mut chart = ChartBuilder::on(&root)
                    .caption("Token Value (NEAR)", ("sans-serif", 14))
                    .margin(10)
                    .x_label_area_size(40)
                    .y_label_area_size(80)
                    .build_cartesian_2d(0..bar_data.len(), 0f64..y_max)
                    .unwrap();

                chart
                    .configure_mesh()
                    .light_line_style(RGBColor(255, 255, 255))
                    .x_labels(bar_data.len())
                    .x_label_formatter(&|idx| {
                        bar_data
                            .get(*idx)
                            .map(|(label, _)| label.clone())
                            .unwrap_or_default()
                    })
                    .y_labels(6)
                    .y_label_formatter(&|v| format_compact(*v))
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
