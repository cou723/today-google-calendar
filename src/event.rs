use chrono::Timelike;
use chrono_tz::{Asia::Tokyo, Tz};
use ratatui::style::Color;

use crate::calendar::Calendar;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct EventModel {
    data: google_calendar3::api::Event,
    calendar_id: Calendar,
}

impl EventModel {
    pub fn new(data: google_calendar3::api::Event, calendar_id: Calendar) -> Self {
        EventModel { data, calendar_id }
    }
}

pub struct EventView {
    pub title: String,
    pub height: u16,
    pub color: Color,
    pub start: u16,
}

impl EventView {
    pub fn from_event(event: EventModel) -> Result<Self> {
        let start_time = event
            .data
            .start
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("start time is not defined"))?
            .date_time
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("start time is not defined"))?
            .with_timezone(&Tokyo);
        let end_time = event
            .data
            .end
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("end time is not defined"))?
            .date_time
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("end time is not defined"))?
            .with_timezone(&Tokyo);

        let start_height = Self::date_time_to_height(start_time, &Tokyo);
        let event_height = match Self::date_time_to_height(end_time, &Tokyo) {
            0 => 48,
            x => x,
        } - start_height;

        Ok(EventView {
            title: (format!(
                "{} {}~{}",
                event.data.summary.unwrap(),
                start_time.format("%H:%M"),
                end_time.format("%H:%M")
            )),
            height: event_height.max(1),
            color: event.calendar_id.color(),
            start: start_height as u16,
        })
    }

    // DateTimeからUI用の高さに変換。
    /*
    example:
    00:30 => 1
    01:00 => 2
    12:00 => 24
    24:00 => 0
     */
    pub fn date_time_to_height(date_time: chrono::DateTime<Tz>, tz: &Tz) -> u16 {
        let hour = date_time.with_timezone(tz).hour();
        let minute = date_time.with_timezone(tz).minute();
        let unit = hour * 2 + minute / 30;
        unit as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Asia::Tokyo;

    #[test]
    fn test_date_time_to_height() {
        let tz = Tokyo;

        // 00:00 should be 0
        let dt = tz.with_ymd_and_hms(2023, 10, 1, 0, 0, 0).unwrap();
        assert_eq!(EventView::date_time_to_height(dt, &tz), 0);

        // 00:30 should be 1
        let dt = tz.with_ymd_and_hms(2023, 10, 1, 0, 30, 0).unwrap();
        assert_eq!(EventView::date_time_to_height(dt, &tz), 1);

        // 01:00 should be 2
        let dt = tz.with_ymd_and_hms(2023, 10, 1, 1, 0, 0).unwrap();
        assert_eq!(EventView::date_time_to_height(dt, &tz), 2);

        // 12:00 should be 24
        let dt = tz.with_ymd_and_hms(2023, 10, 1, 12, 0, 0).unwrap();
        assert_eq!(EventView::date_time_to_height(dt, &tz), 24);

        // 24:00 should be 0
        let dt = tz.with_ymd_and_hms(2023, 10, 1, 0, 0, 0).unwrap();
        assert_eq!(EventView::date_time_to_height(dt, &tz), 0);
    }
}
