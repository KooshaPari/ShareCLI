//! SchedulePort — cron/interval/daily/weekly triggers with `next_run`.
//!
//! Core defines trigger shapes and the port contract; `substrate-schedule`
//! wraps a vetted cron library for expression parsing.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Day-of-week for weekly schedules (0 = Sunday … 6 = Saturday).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weekday {
    /// Sunday.
    Sun = 0,
    /// Monday.
    Mon = 1,
    /// Tuesday.
    Tue = 2,
    /// Wednesday.
    Wed = 3,
    /// Thursday.
    Thu = 4,
    /// Friday.
    Fri = 5,
    /// Saturday.
    Sat = 6,
}

/// A schedule trigger specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleTrigger {
    /// Standard five-field cron expression (minute hour dom month dow).
    Cron {
        /// Cron expression string.
        expr: String,
    },
    /// Fire every `every_secs` seconds after the anchor.
    Interval {
        /// Period in seconds (must be > 0).
        every_secs: u64,
    },
    /// Fire once per day at `hour:minute` UTC.
    Daily {
        /// Hour 0–23.
        hour: u8,
        /// Minute 0–59.
        minute: u8,
    },
    /// Fire once per week on `weekday` at `hour:minute` UTC.
    Weekly {
        /// Day of week.
        weekday: Weekday,
        /// Hour 0–23.
        hour: u8,
        /// Minute 0–59.
        minute: u8,
    },
}

/// Opaque instant for schedule calculations (unix epoch seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleInstant {
    /// Seconds since the Unix epoch.
    pub secs: i64,
}

/// Computes the next fire time for a [`ScheduleTrigger`].
pub trait SchedulePort: Send + Sync {
    /// Return the next fire instant strictly after `after`.
    fn next_run(
        &self,
        trigger: &ScheduleTrigger,
        after: ScheduleInstant,
    ) -> Result<ScheduleInstant>;
}
