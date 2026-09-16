use std::path::Path;

use image::DynamicImage;

/// 读取 PNG、JPEG 以及 `image` crate 支持的其他图片格式。
///
/// 解码错误直接返回给上层，由 CLI 统一转换为用户可读的错误信息。
pub fn load<P: AsRef<Path>>(path: P) -> image::ImageResult<DynamicImage> {
    image::open(path)
}
