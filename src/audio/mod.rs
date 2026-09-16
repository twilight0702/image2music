//! 音频合成与 WAV 输出模块。
//!
//! 合成器把 `NoteEvent` 叠加成浮点混音，再量化为 16 位 PCM；WAV 模块
//! 只负责把已经生成的 PCM 按标准单声道格式写入文件。

pub mod oscillator;
pub mod wav;
