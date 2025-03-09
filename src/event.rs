use chrono::{FixedOffset, Timelike};
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
    pub height: f64,
    pub color: Color,
    pub start: u32,
}

impl EventView {
    pub fn from_event(event: EventModel) -> Result<Self> {
        let start_time = event
            .data
            .start
            .as_ref().ok_or_else(|| anyhow::anyhow!("start time is not defined"))?
            .date_time
            .as_ref().ok_or_else(|| anyhow::anyhow!("start time is not defined"))?
            .with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
        let end_time = event
            .data
            .end
            .as_ref().ok_or_else(|| anyhow::anyhow!("end time is not defined"))?
            .date_time
            .as_ref().ok_or_else(|| anyhow::anyhow!("end time is not defined"))?
            .with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
        let start_hour = start_time.hour();
        let end_hour = match end_time.hour() {
            0 => 24,
            hour => hour,
        };

        let event_height = (end_hour as isize - start_hour as isize).max(1) as f64;

        Ok(EventView {
            title: (format!(
                "{} {}~{}",
                event.data.summary.unwrap(),
                start_time.format("%H:%M"),
                end_time.format("%H:%M")
            )),
            height: event_height,
            color: event.calendar_id.color(),
            start: start_hour,
        })
    }
}