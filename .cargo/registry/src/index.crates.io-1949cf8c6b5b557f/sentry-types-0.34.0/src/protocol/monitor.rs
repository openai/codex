use std::{error::Error, fmt::Display};

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::crontab_validator;

/// Error type for errors with parsing a crontab schedule
#[derive(Debug)]
pub struct CrontabParseError {
    invalid_crontab: String,
}

impl Display for CrontabParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\"{}\" is not a valid crontab schedule.\n\t \
            For help determining why this schedule is invalid, you can use this site: \
            https://crontab.guru/#{}",
            self.invalid_crontab,
            self.invalid_crontab
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("_"),
        )
    }
}

impl Error for CrontabParseError {}

impl CrontabParseError {
    /// Constructs a new CrontabParseError from a given invalid crontab string
    ///
    /// ## Example
    /// ```
    /// use sentry_types::protocol::v7::CrontabParseError;
    ///
    /// let error = CrontabParseError::new("* * * *");
    /// ```
    pub fn new(invalid_crontab: &str) -> Self {
        Self {
            invalid_crontab: String::from(invalid_crontab),
        }
    }
}

/// Represents the status of the monitor check-in
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorCheckInStatus {
    /// Check-in had no issues during execution.
    Ok,
    /// Check-in failed or otherwise had some issues.
    Error,
    /// Check-in is expectred to complete.
    InProgress,
    /// Monitor did not check in on time.
    Missed,
    /// No status was passed.
    #[serde(other)]
    Unknown,
}

/// Configuration object of the monitor schedule.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum MonitorSchedule {
    /// A Crontab schedule allows you to use a standard UNIX crontab style schedule string to
    /// configure when a monitor check-in will be expected on Sentry.
    Crontab {
        /// The crontab syntax string defining the schedule.
        value: String,
    },
    /// A Interval schedule allows you to configure a periodic check-in, that will occur at an
    /// interval after the most recent check-in.
    Interval {
        /// The interval value.
        value: u64,
        /// The interval unit of the value.
        unit: MonitorIntervalUnit,
    },
}

impl MonitorSchedule {
    /// Attempts to create a MonitorSchedule from a provided crontab_str. If the crontab_str is a
    /// valid crontab schedule, we return a Result containing the MonitorSchedule; otherwise, we
    /// return a Result containing a CrontabParseError.
    ///
    /// ## Example with valid crontab
    /// ```
    /// use sentry_types::protocol::v7::MonitorSchedule;
    ///
    /// // Create a crontab that runs every other day of the month at midnight.
    /// let result = MonitorSchedule::from_crontab("0 0 */2 * *");
    /// assert!(result.is_ok())
    /// ```
    ///
    /// ## Example with an invalid crontab
    /// ```
    /// use sentry_types::protocol::v7::MonitorSchedule;
    ///
    /// // Invalid crontab.
    /// let result = MonitorSchedule::from_crontab("invalid");
    /// assert!(result.is_err());
    /// ```
    pub fn from_crontab(crontab_str: &str) -> Result<Self, CrontabParseError> {
        if crontab_validator::validate(crontab_str) {
            Ok(Self::Crontab {
                value: String::from(crontab_str),
            })
        } else {
            Err(CrontabParseError::new(crontab_str))
        }
    }
}

/// The unit for the interval schedule type
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorIntervalUnit {
    /// Year Interval.
    Year,
    /// Month Interval.
    Month,
    /// Week Interval.
    Week,
    /// Day Interval.
    Day,
    /// Hour Interval.
    Hour,
    /// Minute Interval.
    Minute,
}

/// The monitor configuration playload for upserting monitors during check-in
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MonitorConfig {
    /// The monitor schedule configuration.
    pub schedule: MonitorSchedule,

    /// How long (in minutes) after the expected check-in time will we wait until we consider the
    /// check-in to have been missed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkin_margin: Option<u64>,

    /// How long (in minutes) is the check-in allowed to run for in
    /// [`MonitorCheckInStatus::InProgress`] before it is considered failed.in_rogress
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runtime: Option<u64>,

    /// tz database style timezone string
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// The number of consecutive failed/error check-ins that triggers issue creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_issue_threshold: Option<u64>,

    /// The number of consecutive successful check-ins that triggers issue resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_threshold: Option<u64>,
}

fn serialize_id<S: Serializer>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_some(&uuid.as_simple())
}

/// The monitor check-in payload.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MonitorCheckIn {
    /// Unique identifier of this check-in.
    #[serde(serialize_with = "serialize_id")]
    pub check_in_id: Uuid,

    /// Identifier of the monitor for this check-in.
    pub monitor_slug: String,

    /// Status of this check-in. Defaults to `"unknown"`.
    pub status: MonitorCheckInStatus,

    /// The environment to associate the check-in with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,

    /// Duration of this check-in since it has started in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,

    /// Monitor configuration to support upserts. When provided a monitor will be created on Sentry
    /// upon receiving the first check-in.
    ///
    /// If the monitor already exists the configuration will be updated with the values provided in
    /// this object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_config: Option<MonitorConfig>,
}
