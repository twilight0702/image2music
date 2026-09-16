use super::note::{row_to_frequency, ColoredNoteEvent, NoteEvent};
use crate::analyzer::scanner::{ColorColumnFeature, ColumnFeature};
use crate::image::preprocess::weighted_hue_mean;

/// 连续笔画最长按多少列切成一个音符。
///
/// 这样即使图片前景没有空白列，也能形成稳定的节奏。值设为 4 是为了
/// 在默认宽度 64 下产生大约 16 个时间单元，同时保留足够的持续音感。
const MAX_COLUMNS_PER_EVENT: usize = 4;

/// 相邻列中心相差超过图片高度的该比例时，认为旋律发生了明显变化。
const PITCH_CHANGE_RATIO: f32 = 0.12;

/// 将连续的有效列合并为旋律事件。
///
/// 每个连续区域对应一个音符：区域平均纵向中心决定音高，区域平均亮度
/// 决定振幅，区域的横向长度决定持续时间。空白列会切断当前区域，形成
/// 音符之间的间隔。
pub fn map_columns_to_events(
    columns: &[ColumnFeature],
    image_width: u32,
    image_height: u32,
    total_duration: f32,
) -> Vec<NoteEvent> {
    if columns.is_empty()
        || image_width == 0
        || image_height == 0
        || !total_duration.is_finite()
        || total_duration <= 0.0
    {
        return Vec::new();
    }

    let mut events = Vec::new();
    let mut region_start = None;
    let mut region_center_sum = 0.0;
    let mut region_brightness_sum = 0.0;
    let mut region_columns = 0usize;
    let mut previous_center_y: Option<f32> = None;

    // 把暂存的连续区域写成 NoteEvent。单独抽成闭包，确保遍历结束时
    // 仍能正确提交最后一段没有后续空白列的区域。
    let flush = |region_start: &mut Option<u32>,
                 region_center_sum: &mut f32,
                 region_brightness_sum: &mut f32,
                 region_columns: &mut usize,
                 end_x: u32,
                 events: &mut Vec<NoteEvent>| {
        let Some(start_x) = region_start.take() else {
            return;
        };
        if *region_columns == 0 {
            return;
        }

        // 横坐标线性映射到总时长，保持视觉上的左右顺序。
        let start = start_x as f32 / image_width as f32 * total_duration;
        let end = end_x as f32 / image_width as f32 * total_duration;
        let duration = (end - start).max(total_duration / image_width as f32);
        let center_y = *region_center_sum / *region_columns as f32;
        // 保留极暗但已越过阈值的像素，避免生成完全不可闻的事件。
        let amplitude = (*region_brightness_sum / *region_columns as f32).clamp(0.05, 1.0);

        events.push(NoteEvent {
            start,
            duration,
            frequency: row_to_frequency(center_y, image_height),
            amplitude,
        });
        *region_center_sum = 0.0;
        *region_brightness_sum = 0.0;
        *region_columns = 0;
    };

    for (index, column) in columns.iter().enumerate() {
        if column.is_active() {
            // 除了空白列外，还按最长时长和音高突变切分区域，避免一张
            // 连续前景图最终只生成一个单调的长音。
            let pitch_changed = previous_center_y
                .map(|previous| {
                    (previous - column.center_y).abs()
                        >= (image_height as f32 * PITCH_CHANGE_RATIO).max(1.0)
                })
                .unwrap_or(false);
            if region_start.is_some() && (region_columns >= MAX_COLUMNS_PER_EVENT || pitch_changed)
            {
                flush(
                    &mut region_start,
                    &mut region_center_sum,
                    &mut region_brightness_sum,
                    &mut region_columns,
                    column.x,
                    &mut events,
                );
            }
            if region_start.is_none() {
                region_start = Some(column.x);
            }
            region_center_sum += column.center_y;
            region_brightness_sum += column.brightness;
            region_columns += 1;
            previous_center_y = Some(column.center_y);
        } else {
            flush(
                &mut region_start,
                &mut region_center_sum,
                &mut region_brightness_sum,
                &mut region_columns,
                index as u32,
                &mut events,
            );
            previous_center_y = None;
        }
    }

    flush(
        &mut region_start,
        &mut region_center_sum,
        &mut region_brightness_sum,
        &mut region_columns,
        image_width,
        &mut events,
    );

    events
}

/// 将彩色图片的连续有效列合并为带 HSV 音色信息的旋律事件。
///
/// 时间、音高和持续时间沿用灰度模式；Value 映射到振幅，Hue 和
/// Saturation 被保存到 `ColoredNoteEvent`，交给音频层决定最终波形。
pub fn map_color_columns_to_events(
    columns: &[ColorColumnFeature],
    image_width: u32,
    image_height: u32,
    total_duration: f32,
) -> Vec<ColoredNoteEvent> {
    if columns.is_empty()
        || image_width == 0
        || image_height == 0
        || !total_duration.is_finite()
        || total_duration <= 0.0
    {
        return Vec::new();
    }

    let mut events = Vec::new();
    let mut region_start = None;
    let mut region_center_sum = 0.0;
    let mut region_value_sum = 0.0;
    let mut region_saturation_sum = 0.0;
    let mut region_hues = Vec::new();
    let mut region_columns = 0usize;
    let mut previous_center_y: Option<f32> = None;

    // 把暂存的连续区域提交成彩色音符。Hue 使用圆周平均，避免红色
    // 在色相 0/1 边界处被错误平均成青色。
    let flush = |region_start: &mut Option<u32>,
                 region_center_sum: &mut f32,
                 region_value_sum: &mut f32,
                 region_saturation_sum: &mut f32,
                 region_hues: &mut Vec<(f32, f32)>,
                 region_columns: &mut usize,
                 end_x: u32,
                 events: &mut Vec<ColoredNoteEvent>| {
        let Some(start_x) = region_start.take() else {
            return;
        };
        if *region_columns == 0 {
            return;
        }

        let start = start_x as f32 / image_width as f32 * total_duration;
        let end = end_x as f32 / image_width as f32 * total_duration;
        let duration = (end - start).max(total_duration / image_width as f32);
        let center_y = *region_center_sum / *region_columns as f32;
        let value = (*region_value_sum / *region_columns as f32).clamp(0.05, 1.0);
        let saturation = (*region_saturation_sum / *region_columns as f32).clamp(0.0, 1.0);
        let hue = weighted_hue_mean(region_hues.drain(..));

        events.push(ColoredNoteEvent {
            note: NoteEvent {
                start,
                duration,
                frequency: row_to_frequency(center_y, image_height),
                amplitude: value,
            },
            hue,
            saturation,
        });
        *region_center_sum = 0.0;
        *region_value_sum = 0.0;
        *region_saturation_sum = 0.0;
        *region_columns = 0;
    };

    for (index, column) in columns.iter().enumerate() {
        if column.is_active() {
            // 彩色模式和灰度模式使用相同的节奏切分规则，保证仅改变颜色
            // 不会改变图片的基本时间结构。
            let pitch_changed = previous_center_y
                .map(|previous| {
                    (previous - column.center_y).abs()
                        >= (image_height as f32 * PITCH_CHANGE_RATIO).max(1.0)
                })
                .unwrap_or(false);
            if region_start.is_some() && (region_columns >= MAX_COLUMNS_PER_EVENT || pitch_changed)
            {
                flush(
                    &mut region_start,
                    &mut region_center_sum,
                    &mut region_value_sum,
                    &mut region_saturation_sum,
                    &mut region_hues,
                    &mut region_columns,
                    column.x,
                    &mut events,
                );
            }
            if region_start.is_none() {
                region_start = Some(column.x);
            }
            region_center_sum += column.center_y;
            region_value_sum += column.value;
            region_saturation_sum += column.saturation;
            region_hues.push((column.hue, column.saturation * column.value));
            region_columns += 1;
            previous_center_y = Some(column.center_y);
        } else {
            flush(
                &mut region_start,
                &mut region_center_sum,
                &mut region_value_sum,
                &mut region_saturation_sum,
                &mut region_hues,
                &mut region_columns,
                index as u32,
                &mut events,
            );
            previous_center_y = None;
        }
    }

    flush(
        &mut region_start,
        &mut region_center_sum,
        &mut region_value_sum,
        &mut region_saturation_sum,
        &mut region_hues,
        &mut region_columns,
        image_width,
        &mut events,
    );

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(x: u32, center_y: f32, brightness: f32) -> ColumnFeature {
        ColumnFeature {
            x,
            active_rows: vec![center_y as u32],
            brightness,
            center_y,
        }
    }

    #[test]
    fn contiguous_columns_become_one_event_and_gaps_split_events() {
        let columns = vec![
            column(0, 2.0, 1.0),
            column(1, 3.0, 0.8),
            ColumnFeature {
                x: 2,
                active_rows: vec![],
                brightness: 0.0,
                center_y: 0.0,
            },
            column(3, 20.0, 0.6),
        ];

        let events = map_columns_to_events(&columns, 4, 32, 8.0);
        assert_eq!(events.len(), 2);
        assert!((events[0].start - 0.0).abs() < f32::EPSILON);
        assert!((events[0].duration - 4.0).abs() < f32::EPSILON);
        assert!((events[0].amplitude - 0.9).abs() < 0.001);
        assert!((events[1].start - 6.0).abs() < f32::EPSILON);
        assert!((events[1].duration - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_regions_preserve_hsv_timbre_data() {
        let columns = vec![
            ColorColumnFeature {
                x: 0,
                active_rows: vec![8],
                brightness: 1.0,
                center_y: 8.0,
                hue: 0.99,
                saturation: 1.0,
                value: 1.0,
            },
            ColorColumnFeature {
                x: 1,
                active_rows: vec![8],
                brightness: 1.0,
                center_y: 8.0,
                hue: 0.01,
                saturation: 1.0,
                value: 1.0,
            },
        ];

        let events = map_color_columns_to_events(&columns, 2, 32, 1.0);
        assert_eq!(events.len(), 1);
        assert!(!(0.02..=0.98).contains(&events[0].hue));
        assert!((events[0].saturation - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn long_continuous_region_is_split_into_rhythmic_events() {
        let columns = (0..5).map(|x| column(x, 10.0, 1.0)).collect::<Vec<_>>();
        let events = map_columns_to_events(&columns, 5, 32, 5.0);

        assert_eq!(events.len(), 2);
        assert!((events[0].duration - 4.0).abs() < f32::EPSILON);
        assert!((events[1].start - 4.0).abs() < f32::EPSILON);
    }
}
