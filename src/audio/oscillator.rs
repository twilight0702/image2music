use std::f32::consts::PI;

use crate::music::note::NoteEvent;

/// 无状态振荡器：给定频率和时间，返回一个 `-1.0..=1.0` 的波形采样值。
///
/// 使用无状态接口可以让不同波形共用同一个事件渲染器，也为后续根据
/// HSV 色相选择音色留下扩展点。
pub trait Oscillator: Send + Sync {
    /// 计算频率为 `frequency`、时间为 `t`（秒）时的波形值。
    fn sample(&self, frequency: f32, t: f32) -> f32;
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

/// 将音符事件渲染为单声道、有符号 16 位 PCM 样本。
///
/// 所有事件先在同一个浮点缓冲区中相加，因此自然支持重叠事件混音；
/// 最终量化前将结果限制在 `[-1.0, 1.0]`，避免超出 i16 范围发生溢出。
pub fn render_events<O: Oscillator>(
    events: &[NoteEvent],
    sample_rate: u32,
    oscillator: &O,
) -> Vec<i16> {
    if sample_rate == 0 {
        return Vec::new();
    }

    let end_time = events
        .iter()
        .filter(|event| event.start.is_finite() && event.duration.is_finite())
        .map(NoteEvent::end)
        .filter(|end| end.is_finite() && *end > 0.0)
        .fold(0.0_f32, f32::max);
    let sample_count = (end_time * sample_rate as f32).ceil() as usize;
    let mut samples = vec![0.0_f32; sample_count];

    for event in events {
        // 忽略非法事件，避免 NaN、负时间或非正频率破坏整个输出文件。
        if !event.start.is_finite()
            || !event.duration.is_finite()
            || !event.frequency.is_finite()
            || !event.amplitude.is_finite()
            || event.start < 0.0
            || event.duration <= 0.0
            || event.frequency <= 0.0
        {
            continue;
        }

        let start = (event.start * sample_rate as f32).floor() as usize;
        let end = ((event.start + event.duration) * sample_rate as f32)
            .ceil()
            .min(sample_count as f32) as usize;
        // 这里使用相对于事件开始时间的 local_time，让每个音符都从自己的
        // 波形相位 0 开始，且不受它在整首音频中的绝对位置影响。
        for (index, sample) in samples[start.min(sample_count)..end].iter_mut().enumerate() {
            let absolute_index = start + index;
            let local_time = absolute_index as f32 / sample_rate as f32 - event.start;
            *sample += oscillator.sample(event.frequency, local_time) * event.amplitude;
        }
    }

    samples
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
        .collect()
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
}
