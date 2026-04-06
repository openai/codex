//! Persistent thread-local alarm scheduling for follow-on turns and same-turn steer delivery.
//!
//! This module owns the in-memory alarm registry, schedule parsing, the hidden
//! alarm prompt injected when an alarm fires, and the JSON sidecar format used
//! to restore alarms after a harness restart.

use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const AFTER_TURN_CRON_EXPRESSION: &str = "@after-turn";
const EVERY_PREFIX: &str = "@every ";
const EVERY_SECONDS_PREFIX: &str = "@every:";
pub const ALARM_UPDATED_BACKGROUND_EVENT_PREFIX: &str = "alarm_updated:";
pub const ALARM_FIRED_BACKGROUND_EVENT_PREFIX: &str = "alarm_fired:";
pub const MAX_ACTIVE_ALARMS_PER_THREAD: usize = 256;
const MAX_EVERY_SECONDS: u64 = i64::MAX as u64;
const ONE_SHOT_ALARM_PROMPT: &str = include_str!("../templates/alarms/one_shot_prompt.md");
const RECURRING_ALARM_PROMPT: &str = include_str!("../templates/alarms/recurring_prompt.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlarmDelivery {
    AfterTurn,
    SteerCurrentTurn,
}

impl AlarmDelivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AfterTurn => "after-turn",
            Self::SteerCurrentTurn => "steer-current-turn",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadAlarm {
    pub id: String,
    pub cron_expression: String,
    pub prompt: String,
    pub run_once: bool,
    pub delivery: AlarmDelivery,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlarmInvocationContext {
    pub(crate) current_alarm_id: String,
    pub(crate) cron_expression: String,
    pub(crate) prompt: String,
    pub(crate) run_once: bool,
    pub(crate) delivery: AlarmDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimedAlarm {
    pub(crate) alarm: ThreadAlarm,
    pub(crate) context: AlarmInvocationContext,
    pub(crate) deleted_run_once_alarm: bool,
}

#[derive(Debug)]
pub(crate) struct CreateAlarm {
    pub(crate) id: String,
    pub(crate) cron_expression: String,
    pub(crate) prompt: String,
    pub(crate) run_once: bool,
    pub(crate) delivery: AlarmDelivery,
    pub(crate) now: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub(crate) struct AlarmsState {
    alarms: HashMap<String, AlarmRuntime>,
}

#[derive(Debug)]
pub(crate) struct AlarmRuntime {
    pub(crate) alarm: ThreadAlarm,
    schedule: AlarmSchedule,
    pending_run: bool,
    pub(crate) timer_cancel: Option<CancellationToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlarmSchedule {
    AfterTurn,
    EverySeconds(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedAlarm {
    pub(crate) alarm: ThreadAlarm,
    pub(crate) pending_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlarmTimerSpec {
    pub(crate) seconds: u64,
    pub(crate) initial_delay: std::time::Duration,
}

impl AlarmSchedule {
    pub(crate) fn parse(cron_expression: &str) -> Result<Self, String> {
        if cron_expression == AFTER_TURN_CRON_EXPRESSION {
            return Ok(Self::AfterTurn);
        }

        if let Some(seconds) = cron_expression
            .strip_prefix(EVERY_PREFIX)
            .map(str::trim)
            .and_then(parse_duration_literal)
        {
            return Self::parse_every_seconds(seconds, cron_expression);
        }

        if let Some(seconds) = cron_expression
            .strip_prefix(EVERY_SECONDS_PREFIX)
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|seconds| *seconds > 0)
        {
            return Self::parse_every_seconds(seconds, cron_expression);
        }

        Err(format!(
            "unsupported cron_expression `{cron_expression}`; supported values are `{AFTER_TURN_CRON_EXPRESSION}`, `@every 5m`, or `@every:300`"
        ))
    }

    fn parse_every_seconds(seconds: u64, cron_expression: &str) -> Result<Self, String> {
        if seconds > MAX_EVERY_SECONDS {
            return Err(format!(
                "unsupported cron_expression `{cron_expression}`; @every values must be between 1 and {MAX_EVERY_SECONDS} seconds"
            ));
        }
        Ok(Self::EverySeconds(seconds))
    }

    fn next_run_at(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::AfterTurn => None,
            Self::EverySeconds(seconds) => {
                now.checked_add_signed(ChronoDuration::seconds(i64::try_from(seconds).ok()?))
            }
        }
    }
}

impl AlarmsState {
    pub(crate) fn list_alarms(&self) -> Vec<ThreadAlarm> {
        let mut alarms = self
            .alarms
            .values()
            .map(|runtime| runtime.alarm.clone())
            .collect::<Vec<_>>();
        alarms.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        alarms
    }

    pub(crate) fn persisted_alarms(&self) -> Vec<PersistedAlarm> {
        let mut alarms = self
            .alarms
            .values()
            .map(|runtime| PersistedAlarm {
                alarm: runtime.alarm.clone(),
                pending_run: runtime.pending_run,
            })
            .collect::<Vec<_>>();
        alarms.sort_by(|left, right| {
            left.alarm
                .created_at
                .cmp(&right.alarm.created_at)
                .then_with(|| left.alarm.id.cmp(&right.alarm.id))
        });
        alarms
    }

    pub(crate) fn create_alarm(
        &mut self,
        create_alarm: CreateAlarm,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<ThreadAlarm, String> {
        if self.alarms.len() >= MAX_ACTIVE_ALARMS_PER_THREAD {
            return Err(format!(
                "too many active alarms; each thread supports at most {MAX_ACTIVE_ALARMS_PER_THREAD} alarms"
            ));
        }
        let CreateAlarm {
            id,
            cron_expression,
            prompt,
            run_once,
            delivery,
            now,
        } = create_alarm;
        let schedule = AlarmSchedule::parse(&cron_expression)?;
        let next_run_at = match schedule {
            AlarmSchedule::AfterTurn => None,
            AlarmSchedule::EverySeconds(_) => Some(schedule.next_run_at(now).ok_or_else(|| {
                format!(
                    "unsupported cron_expression `{cron_expression}`; next run time is out of range"
                )
            })?),
        };
        let alarm = ThreadAlarm {
            id: id.clone(),
            cron_expression,
            prompt,
            run_once,
            delivery,
            created_at: now.timestamp(),
            next_run_at: next_run_at.map(|value| value.timestamp()),
            last_run_at: None,
        };
        self.alarms.insert(
            id,
            AlarmRuntime {
                alarm: alarm.clone(),
                schedule,
                pending_run: matches!(schedule, AlarmSchedule::AfterTurn),
                timer_cancel,
            },
        );
        Ok(alarm)
    }

    pub(crate) fn restore_alarm(
        &mut self,
        persisted: PersistedAlarm,
        now: DateTime<Utc>,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<Option<AlarmTimerSpec>, String> {
        if self.alarms.len() >= MAX_ACTIVE_ALARMS_PER_THREAD {
            return Err(format!(
                "too many persisted alarms; each thread supports at most {MAX_ACTIVE_ALARMS_PER_THREAD} alarms"
            ));
        }
        let schedule = AlarmSchedule::parse(&persisted.alarm.cron_expression)?;
        let mut alarm = persisted.alarm;
        let mut pending_run = persisted.pending_run;
        let timer_spec = match schedule {
            AlarmSchedule::AfterTurn => {
                alarm.next_run_at = None;
                None
            }
            AlarmSchedule::EverySeconds(seconds) => {
                let initial_delay = match alarm
                    .next_run_at
                    .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single())
                {
                    Some(next_run_at) if next_run_at > now => (next_run_at - now)
                        .to_std()
                        .map_err(|err| err.to_string())?,
                    _ => {
                        pending_run = true;
                        alarm.next_run_at =
                            schedule.next_run_at(now).map(|value| value.timestamp());
                        std::time::Duration::from_secs(seconds)
                    }
                };
                Some(AlarmTimerSpec {
                    seconds,
                    initial_delay,
                })
            }
        };
        let id = alarm.id.clone();
        self.alarms.insert(
            id,
            AlarmRuntime {
                alarm,
                schedule,
                pending_run,
                timer_cancel,
            },
        );
        Ok(timer_spec)
    }

    pub(crate) fn remove_alarm(&mut self, id: &str) -> Option<AlarmRuntime> {
        self.alarms.remove(id)
    }

    pub(crate) fn restore_runtime(&mut self, runtime: AlarmRuntime) {
        self.alarms.insert(runtime.alarm.id.clone(), runtime);
    }

    pub(crate) fn cancel_runtime(runtime: &AlarmRuntime) {
        if let Some(cancel) = runtime.timer_cancel.as_ref() {
            cancel.cancel();
        }
    }

    pub(crate) fn mark_after_turn_alarms_due(&mut self) -> bool {
        let mut changed = false;
        for runtime in self.alarms.values_mut() {
            if matches!(runtime.schedule, AlarmSchedule::AfterTurn) && !runtime.pending_run {
                runtime.pending_run = true;
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn mark_alarm_due(&mut self, id: &str, now: DateTime<Utc>) -> bool {
        let Some(runtime) = self.alarms.get_mut(id) else {
            return false;
        };
        let mut changed = !runtime.pending_run;
        runtime.pending_run = true;
        let next_run_at = runtime
            .schedule
            .next_run_at(now)
            .map(|value| value.timestamp());
        if runtime.alarm.next_run_at != next_run_at {
            runtime.alarm.next_run_at = next_run_at;
            changed = true;
        }
        changed
    }

    pub(crate) fn claim_next_alarm(
        &mut self,
        now: DateTime<Utc>,
        can_after_turn: bool,
        can_steer_current_turn: bool,
    ) -> Option<ClaimedAlarm> {
        let (next_alarm_id, actual_delivery) = self
            .alarms
            .values()
            .filter(|runtime| runtime.pending_run)
            .filter_map(|runtime| {
                let actual_delivery = match runtime.alarm.delivery {
                    AlarmDelivery::AfterTurn if can_after_turn => AlarmDelivery::AfterTurn,
                    AlarmDelivery::AfterTurn => return None,
                    AlarmDelivery::SteerCurrentTurn if can_steer_current_turn => {
                        AlarmDelivery::SteerCurrentTurn
                    }
                    AlarmDelivery::SteerCurrentTurn if can_after_turn => AlarmDelivery::AfterTurn,
                    AlarmDelivery::SteerCurrentTurn => return None,
                };
                Some((runtime, actual_delivery))
            })
            .min_by(|(left, _), (right, _)| {
                left.alarm
                    .last_run_at
                    .unwrap_or(left.alarm.created_at)
                    .cmp(&right.alarm.last_run_at.unwrap_or(right.alarm.created_at))
                    .then_with(|| left.alarm.created_at.cmp(&right.alarm.created_at))
                    .then_with(|| left.alarm.id.cmp(&right.alarm.id))
            })
            .map(|(runtime, actual_delivery)| (runtime.alarm.id.clone(), actual_delivery))?;

        let runtime = self.alarms.remove(&next_alarm_id)?;
        let AlarmRuntime {
            mut alarm,
            schedule,
            pending_run: _,
            timer_cancel,
        } = runtime;
        let deleted_run_once_alarm = alarm.run_once;
        if deleted_run_once_alarm {
            if let Some(cancel) = timer_cancel.as_ref() {
                cancel.cancel();
            }
        } else {
            alarm.last_run_at = Some(now.timestamp());
            self.alarms.insert(
                alarm.id.clone(),
                AlarmRuntime {
                    alarm: alarm.clone(),
                    schedule,
                    pending_run: false,
                    timer_cancel,
                },
            );
        }
        Some(ClaimedAlarm {
            alarm: alarm.clone(),
            context: AlarmInvocationContext {
                current_alarm_id: alarm.id,
                cron_expression: alarm.cron_expression,
                prompt: alarm.prompt,
                run_once: alarm.run_once,
                delivery: actual_delivery,
            },
            deleted_run_once_alarm,
        })
    }
}

pub(crate) fn alarm_prompt_input_item(alarm: &AlarmInvocationContext) -> ResponseInputItem {
    let text = if alarm.run_once {
        render_alarm_prompt_template(ONE_SHOT_ALARM_PROMPT, alarm)
    } else {
        render_alarm_prompt_template(RECURRING_ALARM_PROMPT, alarm)
    };
    ResponseInputItem::Message {
        role: "developer".to_string(),
        content: vec![ContentItem::InputText { text }],
    }
}

fn render_alarm_prompt_template(template: &str, alarm: &AlarmInvocationContext) -> String {
    template
        .replace("{{CURRENT_ALARM_ID}}", &alarm.current_alarm_id)
        .replace("{{SCHEDULE}}", &alarm.cron_expression)
        .replace("{{PROMPT}}", &alarm.prompt)
        .replace("{{DELIVERY}}", alarm.delivery.as_str())
        .trim_end()
        .to_string()
}

pub fn alarm_sidecar_path_for_rollout(rollout_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.alarms.json", rollout_path.display()))
}

pub(crate) async fn load_alarm_sidecar(rollout_path: &Path) -> Result<Vec<PersistedAlarm>, String> {
    let sidecar_path = alarm_sidecar_path_for_rollout(rollout_path);
    let bytes = match tokio::fs::read(&sidecar_path).await {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(format!(
                "failed to read alarm sidecar `{}`: {err}",
                sidecar_path.display()
            ));
        }
    };
    serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse alarm sidecar `{}`: {err}",
            sidecar_path.display()
        )
    })
}

pub(crate) async fn write_alarm_sidecar(
    rollout_path: &Path,
    alarms: &[PersistedAlarm],
) -> Result<(), String> {
    let sidecar_path = alarm_sidecar_path_for_rollout(rollout_path);
    if alarms.is_empty() {
        match tokio::fs::remove_file(&sidecar_path).await {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(format!(
                    "failed to remove empty alarm sidecar `{}`: {err}",
                    sidecar_path.display()
                ));
            }
        }
    }

    let bytes = serde_json::to_vec_pretty(alarms).map_err(|err| {
        format!(
            "failed to serialize alarm sidecar `{}`: {err}",
            sidecar_path.display()
        )
    })?;
    let tmp_path = PathBuf::from(format!("{}.tmp-{}", sidecar_path.display(), Uuid::new_v4()));
    tokio::fs::write(&tmp_path, bytes).await.map_err(|err| {
        format!(
            "failed to write temporary alarm sidecar `{}`: {err}",
            tmp_path.display()
        )
    })?;
    tokio::fs::rename(&tmp_path, &sidecar_path)
        .await
        .map_err(|err| {
            format!(
                "failed to atomically replace alarm sidecar `{}`: {err}",
                sidecar_path.display()
            )
        })?;
    Ok(())
}

fn parse_duration_literal(raw: &str) -> Option<u64> {
    let mut digits = String::new();
    let mut unit = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() && unit.is_empty() {
            digits.push(ch);
        } else if !ch.is_whitespace() {
            unit.push(ch);
        }
    }
    let value = digits.parse::<u64>().ok().filter(|value| *value > 0)?;
    match unit.as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => Some(value),
        "m" | "min" | "mins" | "minute" | "minutes" => value.checked_mul(60),
        "h" | "hr" | "hrs" | "hour" | "hours" => value.checked_mul(60 * 60),
        "d" | "day" | "days" => value.checked_mul(60 * 60 * 24),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::AFTER_TURN_CRON_EXPRESSION;
    use super::AlarmDelivery;
    use super::AlarmInvocationContext;
    use super::AlarmSchedule;
    use super::AlarmsState;
    use super::CreateAlarm;
    use super::MAX_ACTIVE_ALARMS_PER_THREAD;
    use super::MAX_EVERY_SECONDS;
    use super::PersistedAlarm;
    use super::alarm_prompt_input_item;
    use super::alarm_sidecar_path_for_rollout;
    use super::load_alarm_sidecar;
    use super::write_alarm_sidecar;
    use chrono::DateTime;
    use chrono::Duration as ChronoDuration;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn parses_supported_alarm_schedules() {
        assert_eq!(
            AlarmSchedule::parse(AFTER_TURN_CRON_EXPRESSION),
            Ok(AlarmSchedule::AfterTurn)
        );
        assert_eq!(
            AlarmSchedule::parse("@every 5m"),
            Ok(AlarmSchedule::EverySeconds(300))
        );
        assert_eq!(
            AlarmSchedule::parse("@every:3600"),
            Ok(AlarmSchedule::EverySeconds(3600))
        );
    }

    #[test]
    fn rejects_overflowing_every_alarm_schedules() {
        let too_large = MAX_EVERY_SECONDS + 1;
        assert_eq!(
            AlarmSchedule::parse(&format!("@every:{too_large}")),
            Err(format!(
                "unsupported cron_expression `@every:{too_large}`; @every values must be between 1 and {MAX_EVERY_SECONDS} seconds"
            ))
        );
        assert_eq!(
            AlarmSchedule::parse(&format!("@every {too_large}s")),
            Err(format!(
                "unsupported cron_expression `@every {too_large}s`; @every values must be between 1 and {MAX_EVERY_SECONDS} seconds"
            ))
        );
    }

    #[test]
    fn create_alarm_rejects_every_schedule_when_next_run_at_is_out_of_range() {
        let now = DateTime::<Utc>::MAX_UTC - ChronoDuration::seconds(1);
        let mut alarms = AlarmsState::default();

        let result = alarms.create_alarm(
            CreateAlarm {
                id: "alarm-1".to_string(),
                cron_expression: "@every:2".to_string(),
                prompt: "overflow".to_string(),
                run_once: false,
                delivery: AlarmDelivery::AfterTurn,
                now,
            },
            /*timer_cancel*/ None,
        );

        assert_eq!(
            result,
            Err(
                "unsupported cron_expression `@every:2`; next run time is out of range".to_string()
            )
        );
    }

    #[test]
    fn claim_run_once_alarm_removes_it() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        let alarm = alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-1".to_string(),
                    cron_expression: AFTER_TURN_CRON_EXPRESSION.to_string(),
                    prompt: "run tests".to_string(),
                    run_once: true,
                    delivery: AlarmDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");
        assert_eq!(alarms.list_alarms(), vec![alarm]);

        let claimed = alarms
            .claim_next_alarm(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .expect("alarm should be claimed");
        assert_eq!(claimed.context.current_alarm_id, "alarm-1");
        assert!(claimed.deleted_run_once_alarm);
        assert!(alarms.list_alarms().is_empty());
    }

    #[test]
    fn claim_next_alarm_prefers_pending_alarm_that_ran_least_recently() {
        let create_first = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let create_second = Utc.timestamp_opt(101, 0).single().expect("valid timestamp");
        let first_claimed_at = Utc.timestamp_opt(110, 0).single().expect("valid timestamp");
        let second_claimed_at = Utc.timestamp_opt(111, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-1".to_string(),
                    cron_expression: AFTER_TURN_CRON_EXPRESSION.to_string(),
                    prompt: "older recurring alarm".to_string(),
                    run_once: false,
                    delivery: AlarmDelivery::AfterTurn,
                    now: create_first,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");
        alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-2".to_string(),
                    cron_expression: AFTER_TURN_CRON_EXPRESSION.to_string(),
                    prompt: "newer recurring alarm".to_string(),
                    run_once: false,
                    delivery: AlarmDelivery::AfterTurn,
                    now: create_second,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");

        let first = alarms
            .claim_next_alarm(
                first_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
            )
            .expect("first alarm should be claimed");
        assert_eq!(first.context.current_alarm_id, "alarm-1");

        alarms.mark_after_turn_alarms_due();

        let second = alarms
            .claim_next_alarm(
                second_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
            )
            .expect("second alarm should be claimed");
        assert_eq!(second.context.current_alarm_id, "alarm-2");
    }

    #[test]
    fn create_alarm_rejects_more_than_maximum_active_alarms() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        for index in 0..MAX_ACTIVE_ALARMS_PER_THREAD {
            alarms
                .create_alarm(
                    CreateAlarm {
                        id: format!("alarm-{index}"),
                        cron_expression: AFTER_TURN_CRON_EXPRESSION.to_string(),
                        prompt: format!("prompt-{index}"),
                        run_once: false,
                        delivery: AlarmDelivery::AfterTurn,
                        now,
                    },
                    /*timer_cancel*/ None,
                )
                .expect("alarm should be created");
        }

        let result = alarms.create_alarm(
            CreateAlarm {
                id: "alarm-overflow".to_string(),
                cron_expression: AFTER_TURN_CRON_EXPRESSION.to_string(),
                prompt: "overflow".to_string(),
                run_once: false,
                delivery: AlarmDelivery::AfterTurn,
                now,
            },
            /*timer_cancel*/ None,
        );

        assert_eq!(
            result,
            Err(format!(
                "too many active alarms; each thread supports at most {MAX_ACTIVE_ALARMS_PER_THREAD} alarms"
            ))
        );
    }

    #[test]
    fn alarm_prompt_input_is_hidden_developer_input() {
        let item = alarm_prompt_input_item(&AlarmInvocationContext {
            current_alarm_id: "alarm-1".to_string(),
            cron_expression: "@every 10s".to_string(),
            prompt: "run tests".to_string(),
            run_once: false,
            delivery: AlarmDelivery::SteerCurrentTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Recurring scheduled alarm prompt:\nrun tests\n\ncurrentAlarmId: alarm-1\nConfigured delivery: steer-current-turn\nSchedule: @every 10s\n\nThis alarm should keep running on its schedule unless the user asked for a stopping condition and that condition is now satisfied.\nIf that stopping condition is satisfied, stop the alarm by calling AlarmDelete with {\"id\":\"alarm-1\"}.\nDo not expose scheduler internals unless they matter to the user.".to_string(),
                }],
            }
        );
    }

    #[test]
    fn one_shot_alarm_prompt_input_omits_delete_instruction() {
        let item = alarm_prompt_input_item(&AlarmInvocationContext {
            current_alarm_id: "alarm-1".to_string(),
            cron_expression: "@after-turn".to_string(),
            prompt: "run tests once".to_string(),
            run_once: true,
            delivery: AlarmDelivery::AfterTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "One-shot scheduled alarm prompt:\nrun tests once\n\ncurrentAlarmId: alarm-1\nConfigured delivery: after-turn\nSchedule: @after-turn\n\nThis one-shot alarm has already been removed from the schedule, so you do not need to call AlarmDelete.\nDo not expose scheduler internals unless they matter to the user.".to_string(),
                }],
            }
        );
    }

    #[tokio::test]
    async fn alarm_sidecar_round_trips_persisted_alarms() {
        let tempdir = TempDir::new().expect("tempdir");
        let rollout_path = tempdir.path().join("rollout.jsonl");
        let persisted = vec![PersistedAlarm {
            alarm: super::ThreadAlarm {
                id: "alarm-1".to_string(),
                cron_expression: "@after-turn".to_string(),
                prompt: "run tests".to_string(),
                run_once: false,
                delivery: AlarmDelivery::AfterTurn,
                created_at: 1,
                next_run_at: None,
                last_run_at: None,
            },
            pending_run: true,
        }];

        write_alarm_sidecar(&rollout_path, &persisted)
            .await
            .expect("write sidecar");
        let loaded = load_alarm_sidecar(&rollout_path)
            .await
            .expect("load sidecar");

        assert_eq!(loaded, persisted);
        assert_eq!(
            alarm_sidecar_path_for_rollout(&rollout_path),
            tempdir.path().join("rollout.jsonl.alarms.json")
        );
    }
}
