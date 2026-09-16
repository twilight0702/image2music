use std::f32::consts::PI;

use crate::music::note::{ColoredNoteEvent, NoteEvent};

/// 无状态振荡器：给定频率和时间，返回一个 `-1.0..=1.0` 的波形采样值。
///
/// 使用无状态接口可以让不同波形共用同一个事件渲染器，也为后续根据
/// HSV 色相选择音色留下扩展点。
pub trait Oscillator: Send + Sync {
    /// 计算频率为 `frequency`、时间为 `t`（秒）时的波形值。
    fn sample(&self, frequency: f32, t: f32) -> f32;
}

/// 支持按 HSV 参数动态选择音色的振荡器。
pub trait ColorOscillator: Send + Sync {
    /// 根据频率、时间、Hue 和 Saturation 计算波形采样值。
    fn sample_color(&self, frequency: f32, t: f32, hue: f32, saturation: f32) -> f32;
}

/// 正弦振荡器，V0.1 默认使用的波形。
#[derive(Debug, Default, Clone, Copy)]
pub struct SineOscillator;

impl Oscillator for SineOscillator {
    fn sample(&self, frequency: f32, t: f32) -> f32 {
        (2.0 * PI * frequency * t).sin()
    }
}

/// 三角波振荡器，适合需要较柔和但比正弦波更丰富的泛音时使用。
#[derive(Debug, Default, Clone, Copy)]
pub struct TriangleOscillator;

impl Oscillator for TriangleOscillator {
    fn sample(&self, frequency: f32, t: f32) -> f32 {
        let phase = (frequency * t).rem_euclid(1.0);
        1.0 - 4.0 * (phase - 0.5).abs()
    }
}

/// 方波振荡器，具有明显的奇次泛音和更强的电子音色。
#[derive(Debug, Default, Clone, Copy)]
pub struct SquareOscillator;

impl Oscillator for SquareOscillator {
    fn sample(&self, frequency: f32, t: f32) -> f32 {
        if (frequency * t).rem_euclid(1.0) < 0.5 {
            1.0
        } else {
            -1.0
        }
    }
}

/// 锯齿波振荡器，包含完整的谐波序列，音色更明亮。
#[derive(Debug, Default, Clone, Copy)]
pub struct SawtoothOscillator;

impl Oscillator for SawtoothOscillator {
    fn sample(&self, frequency: f32, t: f32) -> f32 {
        2.0 * (frequency * t).rem_euclid(1.0) - 1.0
    }
}

/// 将 Hue 映射到四种基础波形，并使用 Saturation 控制颜色音色强度。
///
/// Hue 不会直接造成突变的波形切换，而是在相邻波形间线性插值：
/// 色相每转过四分之一色环，就从当前波形平滑过渡到下一个波形。
/// 饱和度为 0 时始终退化为正弦波，因此黑白图片的听感保持稳定。
#[derive(Debug, Default, Clone, Copy)]
pub struct HsvOscillator;

impl ColorOscillator for HsvOscillator {
    fn sample_color(&self, frequency: f32, t: f32, hue: f32, saturation: f32) -> f32 {
        let sine = SineOscillator;
        let triangle = TriangleOscillator;
        let square = SquareOscillator;
        let sawtooth = SawtoothOscillator;
        let waveforms: [&dyn Oscillator; 4] = [&sine, &triangle, &square, &sawtooth];

        let hue_position = hue.rem_euclid(1.0) * waveforms.len() as f32;
        let current = hue_position.floor() as usize % waveforms.len();
        let next = (current + 1) % waveforms.len();
        let blend = hue_position.fract();
        let colored = waveforms[current].sample(frequency, t) * (1.0 - blend)
            + waveforms[next].sample(frequency, t) * blend;
        let saturation = saturation.clamp(0.0, 1.0);

        sine.sample(frequency, t) * (1.0 - saturation) + colored * saturation
    }
}

/// 将音符事件渲染为单声道、有符号 16 位 PCM 样本。
///
/// 所有事件先在同一个浮点缓冲区中相加，因此自然支持重叠事件混音；
/// 最终量化前将结果限制在 `[-1.0, 1.0]`，避免超出 i16 范围发生溢出。
pub fn render_events<O: Oscillator>(
    events: &[NoteEvent],
    sample_rate: u32,
    oscillator: &O,
) -> Vec<i16> {
    render_events_with(events, sample_rate, |event, local_time| {
        oscillator.sample(event.frequency, local_time) * event.amplitude
    })
}

/// 使用彩色振荡器渲染带 HSV 音色信息的音符事件。
pub fn render_colored_events<O: ColorOscillator>(
    events: &[ColoredNoteEvent],
    sample_rate: u32,
    oscillator: &O,
) -> Vec<i16> {
    render_events_with(events, sample_rate, |event, local_time| {
        oscillator.sample_color(
            event.note.frequency,
            local_time,
            event.hue,
            event.saturation,
        ) * event.note.amplitude
    })
}

/// 让基础音符和带颜色音符共用 PCM 渲染逻辑。
///
/// `RenderableEvent` 只暴露基础 `NoteEvent`，具体波形由闭包决定；这样
/// 两种模式都能正确计算时间范围、混音和 i16 量化，而不会复制整段算法。
trait RenderableEvent {
    fn note(&self) -> &NoteEvent;
}

impl RenderableEvent for NoteEvent {
    fn note(&self) -> &NoteEvent {
        self
    }
}

impl RenderableEvent for ColoredNoteEvent {
    fn note(&self) -> &NoteEvent {
        &self.note
    }
}

fn render_events_with<E, F>(events: &[E], sample_rate: u32, mut sample_at: F) -> Vec<i16>
where
    E: RenderableEvent,
    F: FnMut(&E, f32) -> f32,
{
    if sample_rate == 0 {
        return Vec::new();
    }

    let end_time = events
        .iter()
        .filter(|event| event.note().start.is_finite() && event.note().duration.is_finite())
        .map(|event| event.note().end())
        .filter(|end| end.is_finite() && *end > 0.0)
        .fold(0.0_f32, f32::max);
    let sample_count = (end_time * sample_rate as f32).ceil() as usize;
    let mut samples = vec![0.0_f32; sample_count];

    for event in events {
        let note = event.note();
        // 忽略非法事件，避免 NaN、负时间或非正频率破坏整个输出文件。
        if !note.start.is_finite()
            || !note.duration.is_finite()
            || !note.frequency.is_finite()
            || !note.amplitude.is_finite()
            || note.start < 0.0
            || note.duration <= 0.0
            || note.frequency <= 0.0
        {
            continue;
        }

        let start = (note.start * sample_rate as f32).floor() as usize;
        let end = ((note.start + note.duration) * sample_rate as f32)
            .ceil()
            .min(sample_count as f32) as usize;
        // 这里使用相对于事件开始时间的 local_time，让每个音符都从自己的
        // 波形相位 0 开始，且不受它在整首音频中的绝对位置影响。
        for (index, output) in samples[start.min(sample_count)..end].iter_mut().enumerate() {
            let absolute_index = start + index;
            let local_time = absolute_index as f32 / sample_rate as f32 - note.start;
            // 极短的淡入淡出可以消除波形在音符边界突然截断造成的点击声，
            // 也能让方波和锯齿波在多个短音符之间听起来更平滑。
            *output += sample_at(event, local_time) * note_envelope(local_time, note.duration);
        }
    }

    samples
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
        .collect()
}

/// 计算每个音符的简单淡入淡出包络。
///
/// 包络边缘最多占 5 ms；短于 10 ms 的音符会自动把淡入和淡出压缩到
/// 各占一半，保证包络不会出现负值或超出音符持续时间。
fn note_envelope(local_time: f32, duration: f32) -> f32 {
    const EDGE_SECONDS: f32 = 0.005;

    if local_time <= 0.0 || local_time >= duration {
        return 0.0;
    }
    let edge = EDGE_SECONDS.min(duration / 2.0);
    if local_time < edge {
        local_time / edge
    } else if local_time > duration - edge {
        (duration - local_time) / edge
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_starts_at_zero_and_has_expected_peak() {
        let oscillator = SineOscillator;
        assert!(oscillator.sample(440.0, 0.0).abs() < f32::EPSILON);
        assert!((oscillator.sample(440.0, 1.0 / 1760.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn renderer_mixes_events_and_keeps_silence_before_start() {
        let event = NoteEvent {
            start: 0.01,
            duration: 0.02,
            frequency: 440.0,
            amplitude: 0.5,
        };
        let samples = render_events(&[event], 1_000, &SineOscillator);
        assert_eq!(samples.len(), 30);
        assert_eq!(samples[0], 0);
        assert!(samples[15].unsigned_abs() > 1_000);
    }

    #[test]
    fn hsv_oscillator_is_sine_for_gray_and_changes_with_saturation() {
        let oscillator = HsvOscillator;
        let gray = oscillator.sample_color(440.0, 1.0 / 1760.0, 0.2, 0.0);
        let colored = oscillator.sample_color(440.0, 1.0 / 1760.0, 0.2, 1.0);

        assert!((gray - 1.0).abs() < 0.001);
        assert!((colored - gray).abs() > 0.01);
    }

    #[test]
    fn note_envelope_fades_at_both_edges() {
        assert_eq!(note_envelope(0.0, 1.0), 0.0);
        assert!((note_envelope(0.0025, 1.0) - 0.5).abs() < f32::EPSILON);
        assert_eq!(note_envelope(1.0, 1.0), 0.0);
    }
}
