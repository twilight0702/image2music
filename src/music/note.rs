/// 时间轴上的一个音符事件。
///
/// `start` 和 `duration` 使用秒；`frequency` 使用 Hz；`amplitude` 为
/// `0.0..=1.0` 的线性振幅。这个结构是图像分析和 PCM 合成之间的解耦层。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    /// 音符开始时间（秒）。
    pub start: f32,
    /// 音符持续时间（秒）。
    pub duration: f32,
    /// 音符频率（Hz）。
    pub frequency: f32,
    /// 音符振幅，通常来自图像亮度。
    pub amplitude: f32,
}

impl NoteEvent {
    /// 返回音符结束时间，便于合成器计算输出缓冲区长度。
    pub fn end(&self) -> f32 {
        self.start + self.duration
    }
}

/// C 大调音阶，以相对于 C 的半音数表示。
pub const C_MAJOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// 默认最低音为中央 C（C4）。
pub const DEFAULT_BASE_MIDI: u8 = 60;
/// 默认覆盖两个八度，最高音为 C6。
pub const DEFAULT_OCTAVES: u8 = 2;

/// 将 MIDI 音符编号转换为频率（Hz）。
///
/// MIDI 69 是标准音 A4（440 Hz），相邻半音的频率比为 `2^(1/12)`。
pub fn midi_to_frequency(midi: f32) -> f32 {
    440.0 * 2.0_f32.powf((midi - 69.0) / 12.0)
}

/// 将图片行号映射为 C 大调中的音高。
///
/// 顶部行对应最高音，底部行对应最低音，使旋律的上下起伏和视觉方向一致。
pub fn row_to_frequency(row: f32, image_height: u32) -> f32 {
    row_to_frequency_in_scale(
        row,
        image_height,
        DEFAULT_BASE_MIDI,
        DEFAULT_OCTAVES,
        &C_MAJOR,
    )
}

pub fn row_to_frequency_in_scale(
    row: f32,
    image_height: u32,
    base_midi: u8,
    octaves: u8,
    scale: &[u8],
) -> f32 {
    // 空图片或空音阶无法进行正常离散映射，此时返回基准音作为安全兜底。
    if image_height == 0 || scale.is_empty() {
        return midi_to_frequency(f32::from(base_midi));
    }

    let scale_len = scale.len();
    // 八度数为 N 时额外保留一个终点音，保证两端都能落在同名主音上。
    let note_count = usize::from(octaves).saturating_mul(scale_len) + 1;
    // y=0 在顶部。先转换成从底部开始的比例，再四舍五入到最近的音阶级数。
    let normalized = (row / (image_height.saturating_sub(1).max(1) as f32)).clamp(0.0, 1.0);
    let index_from_bottom =
        ((1.0 - normalized) * (note_count.saturating_sub(1) as f32)).round() as usize;
    let octave = index_from_bottom / scale_len;
    let degree = index_from_bottom % scale_len;
    let midi = f32::from(base_midi) + (octave as f32 * 12.0) + f32::from(scale[degree]);
    midi_to_frequency(midi)
}

/// 返回前 `count` 个 C 大调频率，可供调用者展示音阶表或调试映射结果。
pub fn c_major_frequencies(count: usize, base_midi: u8) -> Vec<f32> {
    (0..count)
        .map(|index| {
            let octave = index / C_MAJOR.len();
            let degree = index % C_MAJOR.len();
            midi_to_frequency(
                f32::from(base_midi) + octave as f32 * 12.0 + f32::from(C_MAJOR[degree]),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440_hz() {
        assert!((midi_to_frequency(69.0) - 440.0).abs() < 0.001);
    }

    #[test]
    fn rows_stay_in_c_major_over_two_octaves() {
        let bottom = row_to_frequency(31.0, 32);
        let top = row_to_frequency(0.0, 32);
        assert!((bottom - midi_to_frequency(60.0)).abs() < 0.001);
        assert!((top - midi_to_frequency(84.0)).abs() < 0.001);
    }
}
