use std::f32::consts::PI;
use std::path::Path;

use image::{imageops::FilterType, DynamicImage, GrayImage, Rgb, RgbImage};

use super::loader;

/// 将图片缩放到分析分辨率，并转换为 8 位灰度图。
///
/// 使用 Triangle 插值在缩小图片时保留较平滑的局部亮度，避免简单抽样
/// 让细小结构完全消失。V0.1 后续只读取灰度值，不再需要原始 RGB 数据。
pub fn resize_and_grayscale(image: &DynamicImage, width: u32, height: u32) -> GrayImage {
    image
        .resize_exact(width, height, FilterType::Triangle)
        .into_luma8()
}

/// 将图片缩放到分析分辨率并保留 RGB 数据。
///
/// 颜色映射必须在缩放后的图片上进行，避免原图中的大量像素让不同
/// 分辨率的输入产生不一致的音符数量。先在 RGB 空间插值，再转换 HSV，
/// 也能避免直接插值色相时产生不自然的跨色环结果。
pub fn resize_to_rgb(image: &DynamicImage, width: u32, height: u32) -> RgbImage {
    image
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8()
}

/// 从磁盘加载图片，缩放到分析分辨率，再转换成灰度图。
///
/// 尺寸在这里额外校验，避免把零宽或零高传给底层图像处理函数。
pub fn load_grayscale<P: AsRef<Path>>(
    path: P,
    width: u32,
    height: u32,
) -> image::ImageResult<GrayImage> {
    if width == 0 || height == 0 {
        return Err(image::ImageError::Parameter(
            image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::DimensionMismatch,
            ),
        ));
    }

    let loaded = loader::load(path)?;
    Ok(resize_and_grayscale(&loaded, width, height))
}

/// 同时保存缩放后的 RGB 图和每个像素对应的 HSV 值。
///
/// 灰度值仍然可以由 `rgb` 计算得到，但把 HSV 结果缓存下来可以避免扫描
/// 每一列时重复进行 RGB → HSV 转换。
#[derive(Debug, Clone)]
pub struct ColorImage {
    /// 缩放后的 RGB 像素。
    pub rgb: RgbImage,
    /// 按行优先顺序保存的 HSV 像素，长度等于 `width * height`。
    pub hsv: Vec<Hsv>,
}

impl ColorImage {
    /// 返回图片宽度。
    pub fn width(&self) -> u32 {
        self.rgb.width()
    }

    /// 返回图片高度。
    pub fn height(&self) -> u32 {
        self.rgb.height()
    }

    /// 读取指定坐标的 HSV 值；坐标越界时返回 `None`。
    pub fn hsv_at(&self, x: u32, y: u32) -> Option<Hsv> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        self.hsv
            .get((y as usize) * (self.width() as usize) + x as usize)
            .copied()
    }
}

/// 从磁盘加载图片，缩放后生成 RGB + HSV 预处理结果。
pub fn load_color<P: AsRef<Path>>(
    path: P,
    width: u32,
    height: u32,
) -> image::ImageResult<ColorImage> {
    if width == 0 || height == 0 {
        return Err(image::ImageError::Parameter(
            image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::DimensionMismatch,
            ),
        ));
    }

    let loaded = loader::load(path)?;
    let rgb = resize_to_rgb(&loaded, width, height);
    let hsv = rgb.pixels().copied().map(rgb_to_hsv).collect();
    Ok(ColorImage { rgb, hsv })
}

/// HSV 颜色表示，三个分量都归一化到 `0.0..=1.0`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    /// 色相，0 和 1 都表示红色，色相沿色环循环。
    pub hue: f32,
    /// 饱和度，灰度颜色为 0，纯色为 1。
    pub saturation: f32,
    /// 明度，也就是 RGB 三个分量中的最大值。
    pub value: f32,
}

/// 将 RGB 颜色转换为 HSV。
///
/// 色相用一个完整色环的比例表示，因此红色从 0.0 开始并在 1.0 处回绕。
/// 彩色流水线使用该结果参与 Hue/Saturation 音色映射；灰度像素的 Hue
/// 虽然固定为 0，但会因为饱和度为 0 而自动被扫描器忽略。
pub fn rgb_to_hsv(rgb: Rgb<u8>) -> Hsv {
    let r = f32::from(rgb[0]) / 255.0;
    let g = f32::from(rgb[1]) / 255.0;
    let b = f32::from(rgb[2]) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        (((b - r) / delta) + 2.0) / 6.0
    } else {
        (((r - g) / delta) + 4.0) / 6.0
    };

    Hsv {
        hue,
        saturation: if max == 0.0 { 0.0 } else { delta / max },
        value: max,
    }
}

/// 对一组 Hue 做带权圆周平均。
///
/// Hue 是首尾相接的色环，不能直接使用普通算术平均。例如 0.99 和
/// 0.01 都接近红色，普通平均会错误地得到青色。这里把色相投影到单位
/// 圆后求平均，`weight` 通常使用 `saturation * value`，使低饱和度像素
/// 对最终音色的影响自然减弱。
pub fn weighted_hue_mean<I>(samples: I) -> f32
where
    I: IntoIterator<Item = (f32, f32)>,
{
    let mut sin_sum = 0.0;
    let mut cos_sum = 0.0;
    let mut weight_sum = 0.0;

    for (hue, weight) in samples {
        if !hue.is_finite() || !weight.is_finite() || weight <= 0.0 {
            continue;
        }
        let angle = hue.rem_euclid(1.0) * 2.0 * PI;
        sin_sum += angle.sin() * weight;
        cos_sum += angle.cos() * weight;
        weight_sum += weight;
    }

    if weight_sum == 0.0 {
        return 0.0;
    }

    (sin_sum.atan2(cos_sum) / (2.0 * PI)).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_primary_colors_to_expected_hues() {
        assert_eq!(
            rgb_to_hsv(Rgb([255, 0, 0])),
            Hsv {
                hue: 0.0,
                saturation: 1.0,
                value: 1.0
            }
        );
        assert_eq!(
            rgb_to_hsv(Rgb([0, 255, 0])),
            Hsv {
                hue: 1.0 / 3.0,
                saturation: 1.0,
                value: 1.0
            }
        );
        assert_eq!(
            rgb_to_hsv(Rgb([0, 0, 255])),
            Hsv {
                hue: 2.0 / 3.0,
                saturation: 1.0,
                value: 1.0
            }
        );
    }

    #[test]
    fn hue_mean_wraps_across_the_red_boundary() {
        let hue = weighted_hue_mean([(0.99, 1.0), (0.01, 1.0)]);
        assert!(!(0.02..=0.98).contains(&hue));
    }
}
