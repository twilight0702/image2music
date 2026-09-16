use image::GrayImage;

use crate::image::preprocess::{weighted_hue_mean, ColorImage};

/// 从分析图片提取有效像素的参数。
#[derive(Debug, Clone, Copy)]
pub struct ScanConfig {
    /// 归一化亮度阈值，大于等于该值的像素视为有效像素。
    pub threshold: f32,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self { threshold: 0.5 }
    }
}

/// 从图片的一列中提取出的特征。
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnFeature {
    /// 当前列在图片中的横坐标。
    pub x: u32,
    /// 达到阈值的像素所在行号。
    pub active_rows: Vec<u32>,
    /// 有效像素的平均归一化亮度；没有有效像素时为 0。
    pub brightness: f32,
    /// 有效像素的平均行号；没有有效像素时为 0。
    pub center_y: f32,
}

impl ColumnFeature {
    /// 判断这一列是否至少包含一个有效像素。
    pub fn is_active(&self) -> bool {
        !self.active_rows.is_empty()
    }
}

/// 彩色扫描后的一列特征。
///
/// `brightness` 直接保存前景亮度的平均值，这样既可作为振幅，也能让
/// 颜色扫描结果与现有灰度映射的数据结构保持相同的语义。开启反转后，
/// 前景亮度是 `1.0 - Value`。
#[derive(Debug, Clone, PartialEq)]
pub struct ColorColumnFeature {
    /// 当前列的横坐标。
    pub x: u32,
    /// 达到 Value 阈值的像素所在行号。
    pub active_rows: Vec<u32>,
    /// 有效像素的平均前景亮度，用作亮度和音量。
    pub brightness: f32,
    /// 有效像素的平均行号，用作音高位置。
    pub center_y: f32,
    /// 按色相环计算出的平均 Hue。
    pub hue: f32,
    /// 有效像素的平均 Saturation。
    pub saturation: f32,
    /// 有效像素的平均前景亮度，正常模式下等于平均 Value。
    pub value: f32,
}

impl ColorColumnFeature {
    /// 判断这一列是否至少包含一个有效像素。
    pub fn is_active(&self) -> bool {
        !self.active_rows.is_empty()
    }
}

/// 从左到右扫描图片，为每一列收集达到亮度阈值的行。
///
/// 亮度首先从 `0..=255` 归一化到 `0.0..=1.0`。一列中所有有效像素的
/// 平均行号作为音高位置，平均亮度作为音量；这样分析阶段只输出结构化
/// 特征，后续无需再次访问图片像素。
pub fn scan_columns(image: &GrayImage, config: ScanConfig) -> Vec<ColumnFeature> {
    scan_columns_with_polarity(image, config, false)
}

/// 按列扫描灰度图，并允许把暗色像素作为前景。
///
/// `invert=true` 时使用 `1.0 - brightness` 参与阈值判断和振幅统计，
/// 适合白色背景上的黑色线条、轮廓或剪影。
pub fn scan_columns_with_polarity(
    image: &GrayImage,
    config: ScanConfig,
    invert: bool,
) -> Vec<ColumnFeature> {
    let threshold = config.threshold.clamp(0.0, 1.0);
    (0..image.width())
        .map(|x| {
            let mut active_rows = Vec::new();
            let mut brightness_sum = 0.0;

            for y in 0..image.height() {
                let source_brightness = f32::from(image.get_pixel(x, y)[0]) / 255.0;
                let brightness = if invert {
                    1.0 - source_brightness
                } else {
                    source_brightness
                };
                if brightness >= threshold {
                    active_rows.push(y);
                    brightness_sum += brightness;
                }
            }

            let brightness = if active_rows.is_empty() {
                0.0
            } else {
                brightness_sum / active_rows.len() as f32
            };
            let center_y = if active_rows.is_empty() {
                0.0
            } else {
                active_rows.iter().map(|&row| row as f32).sum::<f32>() / active_rows.len() as f32
            };

            ColumnFeature {
                x,
                active_rows,
                brightness,
                center_y,
            }
        })
        .collect()
}

/// 扫描彩色图片，提取位置、亮度和 HSV 音色特征。
///
/// 这里使用 HSV 的 Value 做阈值判断和亮度统计。对于每列的 Hue，使用
/// `saturation * value` 作为权重，因此灰色像素不会因为一个没有意义的
/// Hue 值而改变整列音色。
pub fn scan_color_columns(image: &ColorImage, config: ScanConfig) -> Vec<ColorColumnFeature> {
    scan_color_columns_with_polarity(image, config, false)
}

/// 按列扫描彩色图片，并允许把暗色像素作为前景。
pub fn scan_color_columns_with_polarity(
    image: &ColorImage,
    config: ScanConfig,
    invert: bool,
) -> Vec<ColorColumnFeature> {
    let threshold = config.threshold.clamp(0.0, 1.0);
    (0..image.width())
        .map(|x| {
            let mut active_rows = Vec::new();
            let mut value_sum = 0.0;
            let mut saturation_sum = 0.0;
            let mut hue_samples = Vec::new();

            for y in 0..image.height() {
                let hsv = image
                    .hsv_at(x, y)
                    .expect("扫描坐标由图片宽高生成，不应越界");
                // 正常模式突出明亮像素；反转模式突出暗色前景。这里的
                // foreground_value 同时用于阈值和振幅，使黑色前景在
                // 反转模式中也能产生足够响度。
                let foreground_value = if invert { 1.0 - hsv.value } else { hsv.value };
                if foreground_value >= threshold {
                    active_rows.push(y);
                    value_sum += foreground_value;
                    saturation_sum += hsv.saturation;
                    hue_samples.push((hsv.hue, hsv.saturation * foreground_value));
                }
            }

            let active_count = active_rows.len() as f32;
            let value = if active_count == 0.0 {
                0.0
            } else {
                value_sum / active_count
            };
            let saturation = if active_count == 0.0 {
                0.0
            } else {
                saturation_sum / active_count
            };
            let center_y = if active_count == 0.0 {
                0.0
            } else {
                active_rows.iter().map(|&row| row as f32).sum::<f32>() / active_count
            };

            ColorColumnFeature {
                x,
                active_rows,
                brightness: value,
                center_y,
                hue: weighted_hue_mean(hue_samples),
                saturation,
                value,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use image::{GrayImage, Luma};

    use super::*;

    #[test]
    fn finds_active_rows_and_their_center() {
        let mut image = GrayImage::from_pixel(2, 4, Luma([0]));
        image.put_pixel(0, 1, Luma([255]));
        image.put_pixel(0, 2, Luma([128]));

        let columns = scan_columns(&image, ScanConfig { threshold: 0.5 });
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].active_rows, vec![1, 2]);
        assert!((columns[0].center_y - 1.5).abs() < f32::EPSILON);
        assert!((columns[0].brightness - (255.0 + 128.0) / (255.0 * 2.0)).abs() < 0.001);
        assert!(!columns[1].is_active());
    }

    #[test]
    fn color_scan_uses_value_for_activation_and_keeps_hue() {
        use image::{Rgb, RgbImage};

        let rgb = RgbImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgb([255, 0, 0])
            } else {
                Rgb([20, 20, 20])
            }
        });
        let hsv = rgb
            .pixels()
            .copied()
            .map(crate::image::preprocess::rgb_to_hsv)
            .collect();
        let image = ColorImage { rgb, hsv };
        let columns = scan_color_columns(&image, ScanConfig { threshold: 0.5 });

        assert!(columns[0].is_active());
        assert!((columns[0].hue - 0.0).abs() < f32::EPSILON);
        assert!(!columns[1].is_active());
    }

    #[test]
    fn inverted_color_scan_finds_a_dark_foreground() {
        use image::{Rgb, RgbImage};

        let rgb = RgbImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgb([0, 0, 64])
            } else {
                Rgb([255, 255, 255])
            }
        });
        let hsv = rgb
            .pixels()
            .copied()
            .map(crate::image::preprocess::rgb_to_hsv)
            .collect();
        let image = ColorImage { rgb, hsv };
        let columns = scan_color_columns_with_polarity(&image, ScanConfig { threshold: 0.5 }, true);

        assert!(columns[0].is_active());
        assert!(!columns[1].is_active());
    }
}
