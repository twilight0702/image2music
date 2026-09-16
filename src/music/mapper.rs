use super::note::{row_to_frequency, NoteEvent};
use crate::analyzer::scanner::ColumnFeature;

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
            if region_start.is_none() {
                region_start = Some(column.x);
            }
            region_center_sum += column.center_y;
            region_brightness_sum += column.brightness;
            region_columns += 1;
        } else {
            flush(
                &mut region_start,
                &mut region_center_sum,
                &mut region_brightness_sum,
                &mut region_columns,
                index as u32,
                &mut events,
            );
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
}
