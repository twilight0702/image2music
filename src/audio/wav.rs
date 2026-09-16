use std::path::Path;

/// 将单声道、有符号 16 位 PCM 样本写成标准 WAV 文件。
///
/// WAV 的声道数和位深固定为 V0.1 所需的 1 声道/16 位；采样率由调用者
/// 提供并写入文件头，播放器可以据此正确还原音频时长和音高。
pub fn write_wav<P: AsRef<Path>>(
    path: P,
    samples: &[i16],
    sample_rate: u32,
) -> Result<(), hound::Error> {
    if sample_rate == 0 {
        return Err(hound::Error::FormatError(
            "sample rate must be greater than zero",
        ));
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn writes_a_readable_mono_wav() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("test.wav");
        write_wav(&path, &[0, 1, -1, 100], 8_000).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 8_000);
        assert_eq!(reader.len(), 4);
        assert!(!fs::read(&path).unwrap().is_empty());
    }
}
