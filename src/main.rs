use std::path::PathBuf;

use clap::Parser;

use image2music::{convert_image_with_polarity, ConversionOptions};

#[derive(Debug, Parser)]
#[command(
    name = "image2music",
    version,
    about = "将 PNG 或 JPEG 图片转换为 WAV 旋律"
)]
struct Args {
    /// 输入的 PNG 或 JPEG 图片。
    input: PathBuf,

    /// 输出的 WAV 文件。
    output: PathBuf,

    /// 分析图片的宽度。
    #[arg(long, default_value_t = 64)]
    width: u32,

    /// 分析图片的高度。
    #[arg(long, default_value_t = 32)]
    height: u32,

    /// 激活像素的亮度阈值，范围为 0.0 到 1.0。
    #[arg(long, default_value_t = 0.5)]
    threshold: f32,

    /// 生成音频的时长，单位为秒。
    #[arg(long, default_value_t = 8.0)]
    duration: f32,

    /// 输出 WAV 的采样率。
    #[arg(long, default_value_t = 44_100)]
    sample_rate: u32,

    /// 把暗色像素作为前景，适合白底黑图或黑色轮廓图。
    #[arg(long, short = 'i')]
    invert: bool,
}

fn main() {
    let args = Args::parse();
    let options = ConversionOptions {
        width: args.width,
        height: args.height,
        threshold: args.threshold,
        duration_seconds: args.duration,
        sample_rate: args.sample_rate,
    };

    match convert_image_with_polarity(&args.input, &args.output, &options, args.invert) {
        Ok(event_count) => {
            println!(
                "Generated {} note event{} in {}",
                event_count,
                if event_count == 1 { "" } else { "s" },
                args.output.display()
            );
        }
        Err(error) => {
            eprintln!("image2music: {error}");
            std::process::exit(1);
        }
    }
}
