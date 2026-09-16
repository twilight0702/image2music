pub mod analyzer;
pub mod audio;
pub mod image;
pub mod music;

use std::path::Path;

use analyzer::scanner::{scan_columns, ScanConfig};
use audio::oscillator::{render_events, SineOscillator};
use audio::wav::write_wav;
use music::mapper::map_columns_to_events;

/// 图像转音乐流水线的运行参数。
///
/// 图片会先被缩放到 `width × height`，然后按列提取音符。时间轴总长度
/// 由 `duration_seconds` 控制，最终音频使用单声道 16 位 PCM 输出。
#[derive(Debug, Clone)]
pub struct ConversionOptions {
    /// 用于分析的图片宽度，设计文档 V0.1 默认值为 64。
    pub width: u32,
    /// 用于分析的图片高度，设计文档 V0.1 默认值为 32。
    pub height: u32,
    /// 归一化亮度阈值，亮度大于等于该值的像素会触发扫描结果。
    pub threshold: f32,
    /// 生成音频的总时长（秒）。
    pub duration_seconds: f32,
    /// WAV 文件的采样率。
    pub sample_rate: u32,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            width: 64,
            height: 32,
            threshold: 0.5,
            duration_seconds: 8.0,
            sample_rate: 44_100,
        }
    }
}

impl ConversionOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err("image dimensions must be greater than zero".into());
        }
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err("threshold must be between 0.0 and 1.0".into());
        }
        if !self.duration_seconds.is_finite() || self.duration_seconds <= 0.0 {
            return Err("duration must be greater than zero".into());
        }
        if self.sample_rate == 0 {
            return Err("sample rate must be greater than zero".into());
        }
        Ok(())
    }
}

/// 使用默认的正弦振荡器，把图片转换为单声道 PCM 样本。
///
/// 这里串起了文档中的完整 V0.1 流程：加载图片 → 缩放和灰度化 → 按列扫描
/// → 映射为 `NoteEvent` → 合成为 PCM。函数返回样本和生成的音符数量，便于
/// CLI 输出摘要，也方便库调用者进一步处理音频数据。
pub fn image_to_samples<P: AsRef<Path>>(
    input: P,
    options: &ConversionOptions,
) -> Result<(Vec<i16>, usize), Box<dyn std::error::Error>> {
    options
        .validate()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;

    let gray = image::preprocess::load_grayscale(input, options.width, options.height)?;
    let scan = scan_columns(
        &gray,
        ScanConfig {
            threshold: options.threshold,
        },
    );
    let events =
        map_columns_to_events(&scan, gray.width(), gray.height(), options.duration_seconds);
    let samples = render_events(&events, options.sample_rate, &SineOscillator);

    Ok((samples, events.len()))
}

/// 转换图片并把生成的单声道 PCM 数据写入 WAV 文件。
pub fn convert_image<P: AsRef<Path>, Q: AsRef<Path>>(
    input: P,
    output: Q,
    options: &ConversionOptions,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (samples, event_count) = image_to_samples(input, options)?;
    write_wav(output, &samples, options.sample_rate)?;
    Ok(event_count)
}
