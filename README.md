# Image2Music

Image2Music 是一个用 Rust 编写的命令行工具，可以把 PNG/JPEG 图片转换成 WAV 音频。

它把图片看作一张二维乐谱：图片从左向右扫描，图像位置和颜色特征被映射为音符的时间、音高、音量和音色。

## 原理

处理流程如下：

```text
PNG / JPEG
    ↓
缩放到 64×32
    ↓
RGB → HSV
    ↓
按列扫描有效像素
    ↓
生成 ColoredNoteEvent
    ↓
HSV 映射音色并合成 PCM
    ↓
输出单声道 WAV
```

主要映射关系：

| 图像特征 | 音乐参数 |
| --- | --- |
| X 坐标 | 音符开始时间 |
| 连续列宽度 | 音符持续时间 |
| Y 坐标 | C 大调音高 |
| Value / 前景亮度 | 是否触发、音量 |
| Hue | 正弦波、三角波、方波、锯齿波之间的音色变化 |
| Saturation | 彩色音色相对于正弦波的强度 |

默认音域为 C3～C5。连续有效区域会按最多 4 列切分；如果相邻列的纵向位置变化明显，也会自动切分成不同音符。每个音符带有短淡入淡出，以减少波形边界的点击声。

Hue 使用圆周平均，能够正确处理红色色相在 `0.0/1.0` 边界处的情况。低饱和度像素的 Hue 不参与音色计算，因此灰度图片会自然接近正弦音色。

## 使用方式

确保已经安装 Rust，然后在项目根目录执行：

```bash
cargo run -- input.png output.wav
```

例如：

```bash
cargo run --release -- ./cat.png ./cat.wav
```

生成的 `output.wav` 是单声道、16 位 PCM WAV 文件。

### 白底黑图

如果图片是白色背景、黑色线条或轮廓，使用 `--invert` 把暗色像素作为前景：

```bash
cargo run -- input.png output.wav --invert
```

也可以降低阈值，让较暗的前景更容易触发：

```bash
cargo run -- input.png output.wav --invert --threshold 0.3
```

### 参数

```text
--width <WIDTH>              分析图片宽度，默认 64
--height <HEIGHT>            分析图片高度，默认 32
--threshold <THRESHOLD>      亮度阈值，范围 0.0～1.0，默认 0.5
--duration <DURATION>        音频时长，单位为秒，默认 8
--sample-rate <SAMPLE_RATE>  WAV 采样率，默认 44100
-i, --invert                 把暗色像素作为前景
```

查看完整帮助：

```bash
cargo run -- --help
```

## 构建与测试

构建发布版本：

```bash
cargo build --release
```

运行测试：

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## 项目结构

```text
src/
├── main.rs                  CLI 入口
├── image/                   图片加载、缩放、灰度化和 HSV 转换
├── analyzer/                逐列扫描与特征提取
├── music/                   音符模型和图像到音符的映射
└── audio/                   振荡器、混音和 WAV 输出
```

当前版本专注于离线图片到 WAV 的转换，不包含 GUI、实时播放和绘图功能。
