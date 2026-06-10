use crate::AUTOMATION_RUN_JITTER_WINDOW_SECS;
use crate::AutomationKind;
use chrono::DateTime;
use chrono::Datelike;
use chrono::Days;
use chrono::Duration;
use chrono::NaiveDate;
use chrono::NaiveTime;
use chrono::TimeZone;
use chrono::Timelike;
use chrono::Utc;
use chrono::Weekday;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduleFrequency {
    Minutely,
    Hourly,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AutomationSchedule {
    frequency: ScheduleFrequency,
    interval: u32,
    hour: Option<u32>,
    minute: Option<u32>,
    weekdays: Vec<Weekday>,
    one_shot: bool,
}

impl AutomationSchedule {
    pub(super) fn parse(rrule: &str) -> anyhow::Result<Self> {
        let normalized_rrule = rrule
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix("RRULE:").map(str::trim))
            .unwrap_or_else(|| rrule.trim().strip_prefix("RRULE:").unwrap_or(rrule.trim()));
        let mut entries = BTreeMap::new();
        for part in normalized_rrule
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let Some((key, value)) = part.split_once('=') else {
                anyhow::bail!("invalid rrule component: {part}");
            };
            entries.insert(key.to_ascii_uppercase(), value.trim().to_ascii_uppercase());
        }

        let frequency = match entries.get("FREQ").map(String::as_str) {
            Some("MINUTELY") => ScheduleFrequency::Minutely,
            Some("HOURLY") => ScheduleFrequency::Hourly,
            Some("DAILY") => ScheduleFrequency::Daily,
            Some("WEEKLY") => ScheduleFrequency::Weekly,
            Some(other) => anyhow::bail!("unsupported rrule frequency: {other}"),
            None => anyhow::bail!("rrule must include FREQ"),
        };
        let interval = entries
            .get("INTERVAL")
            .map(|value| value.parse::<u32>())
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid rrule INTERVAL"))?
            .unwrap_or(1);
        if interval == 0 {
            anyhow::bail!("rrule INTERVAL must be >= 1");
        }
        let count = entries
            .get("COUNT")
            .map(|value| value.parse::<u32>())
            .transpose()
            .map_err(|_| anyhow::anyhow!("invalid rrule COUNT"))?;
        if matches!(count, Some(value) if value > 1) {
            anyhow::bail!("rrule COUNT > 1 is not supported");
        }

        let minute = parse_optional_u32(entries.get("BYMINUTE"), "BYMINUTE", 59)?;
        let hour = parse_optional_u32(entries.get("BYHOUR"), "BYHOUR", 23)?;
        let weekdays = parse_weekdays(entries.get("BYDAY"))?;

        match frequency {
            ScheduleFrequency::Minutely => {
                if hour.is_some() || minute.is_some() || !weekdays.is_empty() {
                    anyhow::bail!("MINUTELY rrules only support FREQ, INTERVAL, and COUNT");
                }
            }
            ScheduleFrequency::Hourly => {
                if hour.is_some() || !weekdays.is_empty() {
                    anyhow::bail!("HOURLY rrules only support BYMINUTE, INTERVAL, and COUNT");
                }
            }
            ScheduleFrequency::Daily => {
                if hour.is_none() || minute.is_none() {
                    anyhow::bail!("DAILY rrules require BYHOUR and BYMINUTE");
                }
                if !weekdays.is_empty() {
                    anyhow::bail!("DAILY rrules do not support BYDAY");
                }
            }
            ScheduleFrequency::Weekly => {
                if interval != 1 {
                    anyhow::bail!("WEEKLY rrules only support INTERVAL=1");
                }
                if hour.is_none() || minute.is_none() {
                    anyhow::bail!("WEEKLY rrules require BYHOUR and BYMINUTE");
                }
                if weekdays.is_empty() {
                    anyhow::bail!("WEEKLY rrules require BYDAY");
                }
            }
        }

        Ok(Self {
            frequency,
            interval,
            hour,
            minute,
            weekdays,
            one_shot: count == Some(1),
        })
    }
}

pub(super) fn compute_next_run_at(
    kind: AutomationKind,
    automation_id: &str,
    schedule: &AutomationSchedule,
    last_run_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    if schedule.one_shot && last_run_at.is_some() {
        return Ok(None);
    }

    let scheduled = match schedule.frequency {
        ScheduleFrequency::Minutely => align_minutely(schedule.interval, now),
        ScheduleFrequency::Hourly => align_hourly(schedule.interval, schedule.minute, now)?,
        ScheduleFrequency::Daily => {
            align_daily(schedule.interval, schedule.hour, schedule.minute, now)?
        }
        ScheduleFrequency::Weekly => {
            align_weekly(schedule.hour, schedule.minute, &schedule.weekdays, now)?
        }
    };
    let jitter_seconds = if should_jitter(kind, schedule) {
        deterministic_automation_jitter_seconds(automation_id, scheduled.timestamp())
    } else {
        0
    };
    Ok(Some(scheduled + Duration::seconds(jitter_seconds)))
}

pub(super) fn compute_next_heartbeat_cooldown_at(
    rrule: &str,
    last_run_at: Option<DateTime<Utc>>,
    thread_updated_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    automation_id: &str,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    let schedule = AutomationSchedule::parse(rrule)?;
    if schedule.one_shot && last_run_at.is_some() {
        return Ok(None);
    }
    let Some(interval) = heartbeat_interval_duration(&schedule) else {
        return compute_next_run_at(
            AutomationKind::Heartbeat,
            automation_id,
            &schedule,
            last_run_at,
            now,
        );
    };
    let baseline = [last_run_at, thread_updated_at].into_iter().flatten().max();
    Ok(baseline.map(|value| value + interval))
}

fn heartbeat_interval_duration(schedule: &AutomationSchedule) -> Option<Duration> {
    match schedule.frequency {
        ScheduleFrequency::Minutely => Some(Duration::minutes(i64::from(schedule.interval))),
        ScheduleFrequency::Hourly => Some(Duration::hours(i64::from(schedule.interval))),
        ScheduleFrequency::Daily | ScheduleFrequency::Weekly => None,
    }
}

fn parse_optional_u32(value: Option<&String>, key: &str, max: u32) -> anyhow::Result<Option<u32>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = value
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("invalid rrule {key}"))?;
    if parsed > max {
        anyhow::bail!("rrule {key} must be <= {max}");
    }
    Ok(Some(parsed))
}

fn parse_weekdays(value: Option<&String>) -> anyhow::Result<Vec<Weekday>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| match value {
            "MO" => Ok(Weekday::Mon),
            "TU" => Ok(Weekday::Tue),
            "WE" => Ok(Weekday::Wed),
            "TH" => Ok(Weekday::Thu),
            "FR" => Ok(Weekday::Fri),
            "SA" => Ok(Weekday::Sat),
            "SU" => Ok(Weekday::Sun),
            _ => Err(anyhow::anyhow!("invalid rrule weekday: {value}")),
        })
        .collect()
}

fn should_jitter(kind: AutomationKind, schedule: &AutomationSchedule) -> bool {
    if schedule.one_shot {
        return false;
    }
    if kind == AutomationKind::Heartbeat
        && matches!(
            schedule.frequency,
            ScheduleFrequency::Minutely | ScheduleFrequency::Hourly
        )
    {
        return false;
    }
    matches!(
        schedule.frequency,
        ScheduleFrequency::Hourly | ScheduleFrequency::Daily | ScheduleFrequency::Weekly
    )
}

fn deterministic_automation_jitter_seconds(automation_id: &str, run_at: i64) -> i64 {
    if AUTOMATION_RUN_JITTER_WINDOW_SECS <= 0 {
        return 0;
    }
    let mut hash = 0_u64;
    for byte in automation_id.as_bytes() {
        hash = hash.wrapping_mul(131).wrapping_add(u64::from(*byte));
    }
    hash = hash.wrapping_mul(131).wrapping_add(run_at as u64);
    (hash % AUTOMATION_RUN_JITTER_WINDOW_SECS as u64) as i64
}

fn align_minutely(interval: u32, now: DateTime<Utc>) -> DateTime<Utc> {
    let truncated = now
        .with_second(0)
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(now);
    truncated + Duration::minutes(i64::from(interval))
}

fn align_hourly(
    interval: u32,
    minute: Option<u32>,
    now: DateTime<Utc>,
) -> anyhow::Result<DateTime<Utc>> {
    let minute = minute.unwrap_or(now.minute());
    let current = now
        .with_minute(minute)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .ok_or_else(|| anyhow::anyhow!("failed to build hourly schedule candidate"))?;
    if current > now {
        return Ok(current);
    }
    Ok(current + Duration::hours(i64::from(interval)))
}

fn align_daily(
    interval: u32,
    hour: Option<u32>,
    minute: Option<u32>,
    now: DateTime<Utc>,
) -> anyhow::Result<DateTime<Utc>> {
    let candidate = date_time_for(now.date_naive(), hour, minute)?;
    if candidate > now {
        return Ok(candidate);
    }
    let next_date = now
        .date_naive()
        .checked_add_days(Days::new(u64::from(interval)))
        .ok_or_else(|| anyhow::anyhow!("failed to compute daily next run"))?;
    date_time_for(next_date, hour, minute)
}

fn align_weekly(
    hour: Option<u32>,
    minute: Option<u32>,
    weekdays: &[Weekday],
    now: DateTime<Utc>,
) -> anyhow::Result<DateTime<Utc>> {
    for offset in 0..=7_u64 {
        let candidate_date = now
            .date_naive()
            .checked_add_days(Days::new(offset))
            .ok_or_else(|| anyhow::anyhow!("failed to compute weekly next run"))?;
        if !weekdays.contains(&candidate_date.weekday()) {
            continue;
        }
        let candidate = date_time_for(candidate_date, hour, minute)?;
        if candidate > now {
            return Ok(candidate);
        }
    }
    anyhow::bail!("failed to compute weekly next run")
}

fn date_time_for(
    date: NaiveDate,
    hour: Option<u32>,
    minute: Option<u32>,
) -> anyhow::Result<DateTime<Utc>> {
    let time = NaiveTime::from_hms_opt(
        hour.ok_or_else(|| anyhow::anyhow!("schedule missing hour"))?,
        minute.ok_or_else(|| anyhow::anyhow!("schedule missing minute"))?,
        0,
    )
    .ok_or_else(|| anyhow::anyhow!("failed to build schedule time"))?;
    Ok(Utc.from_utc_datetime(&date.and_time(time)))
}
