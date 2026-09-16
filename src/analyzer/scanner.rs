use image::GrayImage;

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

/// 从左到右扫描图片，为每一列收集达到亮度阈值的行。
///
/// 亮度首先从 `0..=255` 归一化到 `0.0..=1.0`。一列中所有有效像素的
/// 平均行号作为音高位置，平均亮度作为音量；这样分析阶段只输出结构化
/// 特征，后续无需再次访问图片像素。
pub fn scan_columns(image: &GrayImage, config: ScanConfig) -> Vec<ColumnFeature> {
    let threshold = config.threshold.clamp(0.0, 1.0);
    (0..image.width())
        .map(|x| {
            let mut active_rows = Vec::new();
            let mut brightness_sum = 0.0;

            for y in 0..image.height() {
                let brightness = f32::from(image.get_pixel(x, y)[0]) / 255.0;
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
}
