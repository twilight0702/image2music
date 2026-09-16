# Image → Music

> Rust 图像声音化工具

## 1. 项目目标

输入 PNG/JPEG 图片，分析其二维视觉特征，将其转换为音符事件并合成为 WAV 音频。

首版采用 CLI，不实现 GUI 与绘制功能。

```bash
image2music input.png output.wav
````

---

## 2. 核心映射

将图片视为一张二维乐谱，从左向右扫描。

| 图像特征 | 音乐参数           |
| ---- | -------------- |
| X 坐标 | 时间             |
| Y 坐标 | 音高             |
| 亮度   | 音量 / 是否触发音符    |
| Hue  | 音色             |
| 局部结构 | 节奏 / 持续时间 / 和弦 |

音高不直接连续映射频率，而是限制到指定音阶，例如 C Major。

---

## 3. 处理流程

```text
PNG / JPEG
     │
     ▼
Image Loading
     │
     ▼
Resize
     │
     ▼
Grayscale / Threshold
     │
     ▼
Column Scan
     │
     ▼
Feature Extraction
     │
     ▼
Music Mapping
     │
     ▼
NoteEvent[]
     │
     ▼
Synthesizer
     │
     ▼
PCM Samples
     │
     ▼
output.wav
```

---

## 4. 中间数据模型

图像处理与音频合成通过 `NoteEvent` 解耦。

```rust
struct NoteEvent {
    start: f32,
    duration: f32,
    frequency: f32,
    amplitude: f32,
}
```

图像模块只负责：

```text
Image → Vec<NoteEvent>
```

音频模块只负责：

```text
Vec<NoteEvent> → PCM → WAV
```

---

## 5. 模块设计

```text
src/
├── main.rs
│
├── image/
│   ├── loader.rs
│   └── preprocess.rs
│
├── analyzer/
│   └── scanner.rs
│
├── music/
│   ├── note.rs
│   └── mapper.rs
│
└── audio/
    ├── oscillator.rs
    └── wav.rs
```

### image

负责：

* 图片读取
* Resize
* 灰度化
* RGB → HSV
* Threshold

### analyzer

负责：

* 从左向右扫描
* 像素区域检测
* 特征提取

### music

负责：

* Y → 音高
* 音阶约束
* 图像特征 → `NoteEvent`

### audio

负责：

* Oscillator
* 混音
* PCM Sample 生成
* WAV 输出

---

## 6. V0.1

只实现黑白图片到旋律。

### Pipeline

```text
Image
 ↓
Resize 64×32
 ↓
Grayscale
 ↓
Column Scan
 ↓
Brightness Threshold
 ↓
Y → Scale Note
 ↓
NoteEvent
 ↓
Sine Wave
 ↓
WAV
```

### 功能

1. 读取 PNG/JPEG
2. Resize 到固定尺寸
3. 转换为灰度图
4. 按列扫描
5. 根据亮度阈值寻找有效区域
6. Y 坐标映射到固定音阶
7. 生成 `NoteEvent`
8. 使用正弦振荡器生成 PCM
9. 导出 WAV

目标：

```bash
image2music cat.png cat.wav
```

---

## 7. V0.2：颜色

引入 HSV。

```text
Hue        → Oscillator / Timbre
Saturation → Timbre Strength
Value      → Amplitude
```

实现：

```rust
trait Oscillator {
    fn sample(&self, frequency: f32, t: f32) -> f32;
}
```

支持：

```text
Sine
Triangle
Square
Sawtooth
```

---

## 8. 后续版本

| 版本   | 功能        | 学习内容                           |
| ---- | --------- | ------------------------------ |
| V0.1 | 黑白图片 → 旋律 | 像素、灰度、PCM                      |
| V0.2 | 颜色 → 音色   | HSV、颜色量化                       |
| V0.3 | 轮廓 → 旋律结构 | Sobel、Canny、连通区域               |
| V0.4 | 更丰富的声音    | Oscillator、ADSR、Filter         |
| V0.5 | 音乐结构优化    | Scale、Chord、Tempo、Quantization |
| V1.0 | 实时播放      | Audio Callback、并发、实时音频         |

---

## 9. Rust 技术选择

基础依赖：

```text
image    图片读取与基础处理
hound    WAV 编解码
clap     CLI
```

后续：

```text
cpal     实时音频输出
```

核心部分自行实现：

* 图像扫描
* 特征提取
* 音高映射
* Oscillator
* PCM 合成
* DSP

---

## 10. 设计原则

1. 首版不引入 AI。
2. 保持视觉 → 音乐映射可解释。
3. 图像分析与音频合成解耦。
4. 每次只增加一种图像或声音能力。
5. 核心算法优先自行实现。
6. GUI 最后考虑。

```
