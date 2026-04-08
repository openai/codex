//! Trigger validation and next-fire calculation for persistent thread timers.
//!
//! This module keeps calendar and delay scheduling details out of `timers.rs`.
//! It owns the persisted trigger shape, local wall-clock schedule normalization,
//! and RRULE-backed recurrence evaluation.

use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::LocalResult;
use chrono::NaiveDateTime;
use chrono::TimeZone;
use chrono::Utc;
use rrule::RRuleSet;
use rrule::Tz;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

const LOCAL_DATE_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
const RRULE_DATE_TIME_FORMAT: &str = "%Y%m%dT%H%M%S";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TimerTrigger {
    Delay {
        seconds: u64,
        repeat: Option<bool>,
    },
    Schedule {
        dtstart: Option<String>,
        rrule: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TriggerTiming {
    pub(crate) trigger: TimerTrigger,
    pub(crate) pending_run: bool,
    pub(crate) next_run_at: Option<i64>,
    pub(crate) timer_delay: Option<Duration>,
}

impl TimerTrigger {
    pub(crate) fn is_recurring(&self) -> bool {
        match self {
            Self::Delay { repeat, .. } => repeat.unwrap_or(false),
            Self::Schedule { rrule, .. } => rrule.as_ref().is_some_and(|rrule| !rrule.is_empty()),
        }
    }

    pub(crate) fn is_idle_recurring(&self) -> bool {
        matches!(
            self,
            Self::Delay {
                seconds: 0,
                repeat: Some(true),
            }
        )
    }

    pub(crate) fn display(&self) -> String {
        match self {
            Self::Delay { seconds, repeat } => {
                let suffix = if repeat.unwrap_or(false) {
                    ", repeat"
                } else {
                    ""
                };
                format!("delay {seconds}s{suffix}")
            }
            Self::Schedule { dtstart, rrule } => match (dtstart, rrule) {
                (Some(dtstart), Some(rrule)) => format!("schedule {dtstart}; {rrule}"),
                (Some(dtstart), None) => format!("schedule {dtstart}"),
                (None, Some(rrule)) => format!("schedule {rrule}"),
                (None, None) => "invalid schedule".to_string(),
            },
        }
    }
}

pub(crate) fn timing_for_new_trigger(
    trigger: TimerTrigger,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<TriggerTiming, String> {
    let timezone = local_timezone();
    timing_for_new_trigger_with_timezone(trigger, created_at, now, timezone)
}

pub(crate) fn timing_for_restored_trigger(
    trigger: TimerTrigger,
    created_at: i64,
    persisted_pending_run: bool,
    persisted_next_run_at: Option<i64>,
    now: DateTime<Utc>,
) -> Result<TriggerTiming, String> {
    let timezone = local_timezone();
    timing_for_restored_trigger_with_timezone(
        trigger,
        created_at,
        persisted_pending_run,
        persisted_next_run_at,
        now,
        timezone,
    )
}

pub(crate) fn next_run_after_due(
    trigger: &TimerTrigger,
    created_at: i64,
    now: DateTime<Utc>,
) -> Result<Option<i64>, String> {
    let timezone = local_timezone();
    next_run_after_due_with_timezone(trigger, created_at, now, timezone)
}

fn timing_for_new_trigger_with_timezone(
    trigger: TimerTrigger,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
    timezone: Tz,
) -> Result<TriggerTiming, String> {
    let normalized = normalize_trigger(trigger, now, timezone)?;
    match &normalized {
        TimerTrigger::Delay { seconds, repeat } => {
            let repeat = repeat.unwrap_or(false);
            if repeat && *seconds == 0 {
                return Ok(timing(
                    normalized, /*pending_run*/ true, /*next_run_at*/ None, now,
                ));
            }
            let next_run_at = checked_add_seconds(created_at, *seconds)?;
            let pending_run = next_run_at <= now;
            let next_run_at = if repeat {
                if pending_run {
                    next_delay_recurring_run_at(created_at, *seconds, now)?
                } else {
                    Some(next_run_at.timestamp())
                }
            } else {
                Some(next_run_at.timestamp())
            };
            Ok(timing(normalized, pending_run, next_run_at, now))
        }
        TimerTrigger::Schedule { rrule: None, .. } => {
            let due_at = schedule_dtstart_utc(&normalized, timezone)?;
            Ok(timing(
                normalized,
                due_at <= now,
                Some(due_at.timestamp()),
                now,
            ))
        }
        TimerTrigger::Schedule { rrule: Some(_), .. } => {
            let due_or_next = next_schedule_occurrence_at_or_after(&normalized, now, timezone)?;
            let Some(due_or_next) = due_or_next else {
                return Ok(timing(
                    normalized, /*pending_run*/ false, /*next_run_at*/ None, now,
                ));
            };
            let pending_run = due_or_next <= now.timestamp();
            let next_run_at = if pending_run {
                next_schedule_occurrence_after(&normalized, now, timezone)?
            } else {
                Some(due_or_next)
            };
            Ok(timing(normalized, pending_run, next_run_at, now))
        }
    }
}

fn timing_for_restored_trigger_with_timezone(
    trigger: TimerTrigger,
    created_at: i64,
    persisted_pending_run: bool,
    persisted_next_run_at: Option<i64>,
    now: DateTime<Utc>,
    timezone: Tz,
) -> Result<TriggerTiming, String> {
    let normalized = normalize_trigger(trigger, now, timezone)?;
    match &normalized {
        TimerTrigger::Delay { seconds, repeat } => {
            let repeat = repeat.unwrap_or(false);
            if repeat && *seconds == 0 {
                return Ok(timing(
                    normalized, /*pending_run*/ true, /*next_run_at*/ None, now,
                ));
            }
            let next_run_at = persisted_next_run_at
                .or_else(|| next_delay_run_at(created_at, *seconds))
                .ok_or_else(|| "delay next run time is out of range".to_string())?;
            let due = next_run_at <= now.timestamp();
            let pending_run = persisted_pending_run || due;
            let next_run_at = if repeat && due {
                next_delay_recurring_run_at_from_timestamp(created_at, *seconds, now)?
            } else {
                Some(next_run_at)
            };
            Ok(timing(normalized, pending_run, next_run_at, now))
        }
        TimerTrigger::Schedule { rrule: None, .. } => {
            let next_run_at = persisted_next_run_at
                .or_else(|| {
                    schedule_dtstart_utc(&normalized, timezone)
                        .ok()
                        .map(|dt| dt.timestamp())
                })
                .ok_or_else(|| "schedule next run time is out of range".to_string())?;
            Ok(timing(
                normalized,
                persisted_pending_run || next_run_at <= now.timestamp(),
                Some(next_run_at),
                now,
            ))
        }
        TimerTrigger::Schedule { rrule: Some(_), .. } => {
            let due = persisted_next_run_at.is_none_or(|next| next <= now.timestamp());
            let pending_run = persisted_pending_run || due;
            let next_run_at = if due {
                next_schedule_occurrence_after(&normalized, now, timezone)?
            } else {
                persisted_next_run_at
            };
            Ok(timing(normalized, pending_run, next_run_at, now))
        }
    }
}

fn next_run_after_due_with_timezone(
    trigger: &TimerTrigger,
    created_at: i64,
    now: DateTime<Utc>,
    timezone: Tz,
) -> Result<Option<i64>, String> {
    match trigger {
        TimerTrigger::Delay { seconds, repeat } => {
            if repeat.unwrap_or(false) {
                if *seconds == 0 {
                    return Ok(None);
                }
                next_delay_recurring_run_at_from_timestamp(created_at, *seconds, now)
            } else {
                Ok(None)
            }
        }
        TimerTrigger::Schedule { rrule: None, .. } => Ok(None),
        TimerTrigger::Schedule { rrule: Some(_), .. } => {
            next_schedule_occurrence_after(trigger, now, timezone)
        }
    }
}

fn normalize_trigger(
    trigger: TimerTrigger,
    now: DateTime<Utc>,
    timezone: Tz,
) -> Result<TimerTrigger, String> {
    match trigger {
        TimerTrigger::Delay { seconds, repeat } => Ok(TimerTrigger::Delay { seconds, repeat }),
        TimerTrigger::Schedule { dtstart, rrule } => {
            let dtstart = normalize_optional_string(dtstart);
            let rrule = normalize_optional_string(rrule);
            if dtstart.is_none() && rrule.is_none() {
                return Err("schedule trigger requires dtstart, rrule, or both".to_string());
            }
            let dtstart = match (dtstart, rrule.as_ref()) {
                (Some(dtstart), _) => {
                    validate_dtstart(&dtstart, timezone)?;
                    Some(dtstart)
                }
                (None, Some(_)) => Some(format_local_dtstart(now, timezone)),
                (None, None) => None,
            };
            let normalized = TimerTrigger::Schedule { dtstart, rrule };
            if matches!(normalized, TimerTrigger::Schedule { rrule: Some(_), .. }) {
                parse_rrule_set(&normalized, timezone)?;
            }
            Ok(normalized)
        }
    }
}

fn timing(
    trigger: TimerTrigger,
    pending_run: bool,
    next_run_at: Option<i64>,
    now: DateTime<Utc>,
) -> TriggerTiming {
    let keep_timer_for_pending = trigger.is_recurring();
    let timer_delay = next_run_at
        .filter(|_| !pending_run || keep_timer_for_pending)
        .and_then(|next| timer_delay(next, now));
    TriggerTiming {
        trigger,
        pending_run,
        next_run_at,
        timer_delay,
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn checked_add_seconds(start: DateTime<Utc>, seconds: u64) -> Result<DateTime<Utc>, String> {
    let seconds = i64::try_from(seconds)
        .map_err(|_| "delay seconds value is too large to schedule".to_string())?;
    start
        .checked_add_signed(ChronoDuration::seconds(seconds))
        .ok_or_else(|| "delay next run time is out of range".to_string())
}

fn next_delay_run_at(created_at: i64, seconds: u64) -> Option<i64> {
    let seconds = i64::try_from(seconds).ok()?;
    created_at.checked_add(seconds)
}

fn next_delay_recurring_run_at(
    created_at: DateTime<Utc>,
    seconds: u64,
    now: DateTime<Utc>,
) -> Result<Option<i64>, String> {
    next_delay_recurring_run_at_from_timestamp(created_at.timestamp(), seconds, now)
}

fn next_delay_recurring_run_at_from_timestamp(
    created_at: i64,
    seconds: u64,
    now: DateTime<Utc>,
) -> Result<Option<i64>, String> {
    let seconds = i64::try_from(seconds)
        .map_err(|_| "delay seconds value is too large to schedule".to_string())?;
    if seconds <= 0 {
        return Err("delay.repeat requires seconds to be greater than 0".to_string());
    }
    let elapsed = now.timestamp().saturating_sub(created_at);
    let completed_intervals = elapsed.div_euclid(seconds) + 1;
    created_at
        .checked_add(
            completed_intervals
                .checked_mul(seconds)
                .ok_or_else(|| "delay next run time is out of range".to_string())?,
        )
        .map(Some)
        .ok_or_else(|| "delay next run time is out of range".to_string())
}

fn timer_delay(next_run_at: i64, now: DateTime<Utc>) -> Option<Duration> {
    if next_run_at <= now.timestamp() {
        return Some(Duration::ZERO);
    }
    u64::try_from(next_run_at - now.timestamp())
        .ok()
        .map(Duration::from_secs)
}

fn schedule_dtstart_utc(trigger: &TimerTrigger, timezone: Tz) -> Result<DateTime<Utc>, String> {
    let TimerTrigger::Schedule {
        dtstart: Some(dtstart),
        ..
    } = trigger
    else {
        return Err("schedule trigger requires dtstart".to_string());
    };
    local_dtstart_to_utc(dtstart, timezone)
}

fn next_schedule_occurrence_after(
    trigger: &TimerTrigger,
    after: DateTime<Utc>,
    timezone: Tz,
) -> Result<Option<i64>, String> {
    let set = parse_rrule_set(trigger, timezone)?;
    let after = after
        .checked_add_signed(ChronoDuration::seconds(1))
        .unwrap_or(after);
    let result = set.after(after.with_timezone(&timezone)).all(1);
    Ok(result
        .dates
        .into_iter()
        .next()
        .map(|next| next.with_timezone(&Utc).timestamp()))
}

fn next_schedule_occurrence_at_or_after(
    trigger: &TimerTrigger,
    at: DateTime<Utc>,
    timezone: Tz,
) -> Result<Option<i64>, String> {
    let after = at
        .checked_sub_signed(ChronoDuration::seconds(1))
        .unwrap_or(at);
    next_schedule_occurrence_after(trigger, after, timezone)
}

fn parse_rrule_set(trigger: &TimerTrigger, timezone: Tz) -> Result<RRuleSet, String> {
    let TimerTrigger::Schedule {
        dtstart: Some(dtstart),
        rrule: Some(rrule),
    } = trigger
    else {
        return Err("schedule trigger requires dtstart and rrule".to_string());
    };
    let naive = parse_dtstart(dtstart)?;
    let raw_rrule = rrule
        .strip_prefix("RRULE:")
        .or_else(|| rrule.strip_prefix("rrule:"))
        .unwrap_or(rrule);
    let rrule_set = format!(
        "DTSTART;TZID={}:{}\nRRULE:{}",
        timezone.name(),
        naive.format(RRULE_DATE_TIME_FORMAT),
        raw_rrule
    );
    rrule_set
        .parse::<RRuleSet>()
        .map_err(|err| format!("invalid schedule rrule `{rrule}`: {err}"))
}

fn validate_dtstart(dtstart: &str, timezone: Tz) -> Result<(), String> {
    local_dtstart_to_utc(dtstart, timezone).map(|_| ())
}

fn local_dtstart_to_utc(dtstart: &str, timezone: Tz) -> Result<DateTime<Utc>, String> {
    let naive = parse_dtstart(dtstart)?;
    match timezone.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Ok(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest.with_timezone(&Utc)),
        LocalResult::None => Err(format!(
            "schedule dtstart `{dtstart}` does not exist in local timezone {timezone}"
        )),
    }
}

fn parse_dtstart(dtstart: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(dtstart, LOCAL_DATE_TIME_FORMAT)
        .map_err(|_| format!("schedule dtstart `{dtstart}` must use format YYYY-MM-DDTHH:MM:SS"))
}

fn format_local_dtstart(now: DateTime<Utc>, timezone: Tz) -> String {
    now.with_timezone(&timezone)
        .naive_local()
        .format(LOCAL_DATE_TIME_FORMAT)
        .to_string()
}

fn local_timezone() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|timezone| timezone.parse::<chrono_tz::Tz>().ok())
        .map(Tz::Tz)
        .unwrap_or(Tz::UTC)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    const TS_1: i64 = 1;
    const TS_100: i64 = 100;
    const TS_135: i64 = 135;
    const TS_1_700_000_000: i64 = 1_700_000_000;

    fn utc(timestamp: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(timestamp, 0)
            .single()
            .expect("valid timestamp")
    }

    fn utc_datetime(datetime: &str) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &NaiveDateTime::parse_from_str(datetime, LOCAL_DATE_TIME_FORMAT)
                .expect("valid datetime"),
        )
    }

    #[test]
    fn delay_one_shot_becomes_pending_when_due() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Delay {
                seconds: 0,
                repeat: None,
            },
            utc(TS_100),
            utc(TS_100),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(
            timing,
            TriggerTiming {
                trigger: TimerTrigger::Delay {
                    seconds: 0,
                    repeat: None,
                },
                pending_run: true,
                next_run_at: Some(100),
                timer_delay: None,
            }
        );
    }

    #[test]
    fn delay_repeat_zero_is_idle_recurring() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Delay {
                seconds: 0,
                repeat: Some(true),
            },
            utc(TS_100),
            utc(TS_100),
            Tz::UTC,
        )
        .expect("zero repeat should be a valid idle-recurring trigger");
        assert_eq!(
            timing,
            TriggerTiming {
                trigger: TimerTrigger::Delay {
                    seconds: 0,
                    repeat: Some(true),
                },
                pending_run: true,
                next_run_at: None,
                timer_delay: None,
            }
        );
        assert_eq!(
            next_run_after_due_with_timezone(
                &TimerTrigger::Delay {
                    seconds: 0,
                    repeat: Some(true),
                },
                /*created_at*/ 100,
                utc(TS_100),
                Tz::UTC,
            )
            .expect("zero repeat should remain timer-free"),
            None
        );
    }

    #[test]
    fn delay_repeat_coalesces_overdue_runs() {
        let timing = timing_for_restored_trigger_with_timezone(
            TimerTrigger::Delay {
                seconds: 10,
                repeat: Some(true),
            },
            /*created_at*/ 100,
            /*persisted_pending_run*/ false,
            Some(110),
            utc(TS_135),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(timing.pending_run, true);
        assert_eq!(timing.next_run_at, Some(140));
    }

    #[test]
    fn schedule_rrule_only_resolves_dtstart_to_now() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Schedule {
                dtstart: None,
                rrule: Some("FREQ=HOURLY;BYMINUTE=0;BYSECOND=0".to_string()),
            },
            utc(TS_1_700_000_000),
            utc(TS_1_700_000_000),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(
            timing.trigger,
            TimerTrigger::Schedule {
                dtstart: Some("2023-11-14T22:13:20".to_string()),
                rrule: Some("FREQ=HOURLY;BYMINUTE=0;BYSECOND=0".to_string()),
            }
        );
        assert_eq!(timing.pending_run, false);
        assert_eq!(timing.next_run_at, Some(1_700_002_800));
    }

    #[test]
    fn schedule_dtstart_only_is_one_shot() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Schedule {
                dtstart: Some("2024-01-01T09:00:00".to_string()),
                rrule: None,
            },
            utc(TS_1),
            utc(TS_1),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(timing.pending_run, false);
        assert_eq!(timing.next_run_at, Some(1_704_099_600));
    }

    #[test]
    fn schedule_recurring_historical_dtstart_waits_for_next_future_occurrence() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Schedule {
                dtstart: Some("2024-01-01T09:00:00".to_string()),
                rrule: Some("FREQ=DAILY;BYHOUR=9;BYMINUTE=0;BYSECOND=0".to_string()),
            },
            utc_datetime("2024-01-02T08:00:00"),
            utc_datetime("2024-01-02T08:00:00"),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(timing.pending_run, false);
        assert_eq!(
            timing.next_run_at,
            Some(utc_datetime("2024-01-02T09:00:00").timestamp())
        );
    }

    #[test]
    fn schedule_recurring_due_now_becomes_pending() {
        let timing = timing_for_new_trigger_with_timezone(
            TimerTrigger::Schedule {
                dtstart: Some("2024-01-01T09:00:00".to_string()),
                rrule: Some("FREQ=DAILY;BYHOUR=9;BYMINUTE=0;BYSECOND=0".to_string()),
            },
            utc_datetime("2024-01-02T09:00:00"),
            utc_datetime("2024-01-02T09:00:00"),
            Tz::UTC,
        )
        .expect("trigger should be valid");
        assert_eq!(timing.pending_run, true);
        assert_eq!(
            timing.next_run_at,
            Some(utc_datetime("2024-01-03T09:00:00").timestamp())
        );
    }

    #[test]
    fn schedule_rejects_neither_dtstart_nor_rrule() {
        assert_eq!(
            timing_for_new_trigger_with_timezone(
                TimerTrigger::Schedule {
                    dtstart: None,
                    rrule: None,
                },
                utc(TS_1),
                utc(TS_1),
                Tz::UTC,
            )
            .expect_err("empty schedule should be invalid"),
            "schedule trigger requires dtstart, rrule, or both"
        );
    }

    #[test]
    fn schedule_rejects_invalid_dtstart() {
        assert_eq!(
            timing_for_new_trigger_with_timezone(
                TimerTrigger::Schedule {
                    dtstart: Some("2024-01-01 09:00:00".to_string()),
                    rrule: None,
                },
                utc(TS_1),
                utc(TS_1),
                Tz::UTC,
            )
            .expect_err("bad dtstart should be invalid"),
            "schedule dtstart `2024-01-01 09:00:00` must use format YYYY-MM-DDTHH:MM:SS"
        );
    }

    #[test]
    fn schedule_rejects_invalid_rrule() {
        assert!(
            timing_for_new_trigger_with_timezone(
                TimerTrigger::Schedule {
                    dtstart: Some("2024-01-01T09:00:00".to_string()),
                    rrule: Some("FREQ=NEVER".to_string()),
                },
                utc(TS_1),
                utc(TS_1),
                Tz::UTC,
            )
            .expect_err("bad rrule should be invalid")
            .contains("invalid schedule rrule")
        );
    }
}
