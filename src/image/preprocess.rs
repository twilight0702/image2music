use std::path::Path;

use image::{imageops::FilterType, DynamicImage, GrayImage, Rgb};

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
/// 当前 V0.1 只使用灰度分支，但提前实现该函数可以直接支撑 V0.2 的音色映射。
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
}
