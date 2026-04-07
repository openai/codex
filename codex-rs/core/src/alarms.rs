//! Persistent thread-local alarm scheduling for follow-on turns and same-turn steer delivery.
//!
//! This module owns the in-memory alarm registry, trigger evaluation, the hidden
//! alarm prompt injected when an alarm fires, and the JSON sidecar format used
//! to restore alarms after a harness restart.

use crate::alarm_trigger::AlarmTrigger;
use crate::alarm_trigger::TriggerTiming;
use crate::alarm_trigger::next_run_after_due;
use crate::alarm_trigger::timing_for_new_trigger;
use crate::alarm_trigger::timing_for_restored_trigger;
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

pub const ALARM_UPDATED_BACKGROUND_EVENT_PREFIX: &str = "alarm_updated:";
pub const ALARM_FIRED_BACKGROUND_EVENT_PREFIX: &str = "alarm_fired:";
pub const MAX_ACTIVE_ALARMS_PER_THREAD: usize = 256;
const ONE_SHOT_ALARM_PROMPT: &str = include_str!("../templates/alarms/one_shot_prompt.md");
const RECURRING_ALARM_PROMPT: &str = include_str!("../templates/alarms/recurring_prompt.md");

pub use crate::alarm_trigger::AlarmTrigger as ThreadAlarmTrigger;

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
    pub trigger: AlarmTrigger,
    pub prompt: String,
    pub delivery: AlarmDelivery,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlarmInvocationContext {
    pub(crate) current_alarm_id: String,
    pub(crate) trigger: AlarmTrigger,
    pub(crate) prompt: String,
    pub(crate) recurring: bool,
    pub(crate) delivery: AlarmDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimedAlarm {
    pub(crate) alarm: ThreadAlarm,
    pub(crate) context: AlarmInvocationContext,
    pub(crate) deleted_one_shot_alarm: bool,
}

#[derive(Debug)]
pub(crate) struct CreateAlarm {
    pub(crate) id: String,
    pub(crate) trigger: AlarmTrigger,
    pub(crate) prompt: String,
    pub(crate) delivery: AlarmDelivery,
    pub(crate) now: chrono::DateTime<Utc>,
}

#[derive(Debug, Default)]
pub(crate) struct AlarmsState {
    alarms: HashMap<String, AlarmRuntime>,
}

#[derive(Debug)]
pub(crate) struct AlarmRuntime {
    pub(crate) alarm: ThreadAlarm,
    pending_run: bool,
    pub(crate) timer_cancel: Option<CancellationToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedAlarm {
    pub(crate) alarm: ThreadAlarm,
    pub(crate) pending_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlarmTimerSpec {
    pub(crate) delay: std::time::Duration,
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
    ) -> Result<(ThreadAlarm, Option<AlarmTimerSpec>), String> {
        if self.alarms.len() >= MAX_ACTIVE_ALARMS_PER_THREAD {
            return Err(format!(
                "too many active alarms; each thread supports at most {MAX_ACTIVE_ALARMS_PER_THREAD} alarms"
            ));
        }
        let CreateAlarm {
            id,
            trigger,
            prompt,
            delivery,
            now,
        } = create_alarm;
        let TriggerTiming {
            trigger,
            pending_run,
            next_run_at,
            timer_delay,
        } = timing_for_new_trigger(trigger, now, now)?;
        let alarm = ThreadAlarm {
            id: id.clone(),
            trigger,
            prompt,
            delivery,
            created_at: now.timestamp(),
            next_run_at,
            last_run_at: None,
        };
        self.alarms.insert(
            id,
            AlarmRuntime {
                alarm: alarm.clone(),
                pending_run,
                timer_cancel,
            },
        );
        Ok((alarm, timer_delay.map(|delay| AlarmTimerSpec { delay })))
    }

    pub(crate) fn restore_alarm(
        &mut self,
        persisted: PersistedAlarm,
        now: chrono::DateTime<Utc>,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<Option<AlarmTimerSpec>, String> {
        if self.alarms.len() >= MAX_ACTIVE_ALARMS_PER_THREAD {
            return Err(format!(
                "too many persisted alarms; each thread supports at most {MAX_ACTIVE_ALARMS_PER_THREAD} alarms"
            ));
        }
        let PersistedAlarm {
            alarm,
            pending_run: persisted_pending_run,
        } = persisted;
        let TriggerTiming {
            trigger,
            pending_run,
            next_run_at,
            timer_delay,
        } = timing_for_restored_trigger(
            alarm.trigger,
            alarm.created_at,
            persisted_pending_run,
            alarm.next_run_at,
            now,
        )?;
        let alarm = ThreadAlarm {
            trigger,
            next_run_at,
            ..alarm
        };
        let id = alarm.id.clone();
        self.alarms.insert(
            id,
            AlarmRuntime {
                alarm,
                pending_run,
                timer_cancel,
            },
        );
        Ok(timer_delay.map(|delay| AlarmTimerSpec { delay }))
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

    pub(crate) fn mark_alarm_due(&mut self, id: &str, now: chrono::DateTime<Utc>) -> bool {
        let Some(runtime) = self.alarms.get_mut(id) else {
            return false;
        };
        let mut changed = !runtime.pending_run;
        runtime.pending_run = true;
        match next_run_after_due(&runtime.alarm.trigger, runtime.alarm.created_at, now) {
            Ok(next_run_at) if runtime.alarm.next_run_at != next_run_at => {
                runtime.alarm.next_run_at = next_run_at;
                changed = true;
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    "failed to advance alarm {} trigger: {err}",
                    runtime.alarm.id
                );
            }
        }
        changed
    }

    pub(crate) fn timer_spec_for_alarm(
        &self,
        id: &str,
        now: chrono::DateTime<Utc>,
    ) -> Option<AlarmTimerSpec> {
        let runtime = self.alarms.get(id)?;
        let next_run_at = runtime.alarm.next_run_at?;
        if runtime.pending_run && !runtime.alarm.trigger.is_recurring() {
            return None;
        }
        Some(AlarmTimerSpec {
            delay: if next_run_at <= now.timestamp() {
                std::time::Duration::ZERO
            } else {
                let delay = u64::try_from(next_run_at - now.timestamp()).ok()?;
                std::time::Duration::from_secs(delay)
            },
        })
    }

    pub(crate) fn claim_next_alarm(
        &mut self,
        now: chrono::DateTime<Utc>,
        can_after_turn: bool,
        can_steer_current_turn: bool,
    ) -> Option<ClaimedAlarm> {
        let (next_alarm_id, actual_delivery) = self
            .alarms
            .values()
            .filter(|runtime| runtime.pending_run)
            .filter_map(|runtime| {
                if runtime.alarm.trigger.is_idle_recurring() {
                    if can_after_turn {
                        return Some((runtime, AlarmDelivery::AfterTurn));
                    }
                    return None;
                }
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
            pending_run: _,
            timer_cancel,
        } = runtime;
        let is_recurring = alarm.trigger.is_recurring();
        let deleted_one_shot_alarm = !is_recurring;
        if deleted_one_shot_alarm {
            if let Some(cancel) = timer_cancel.as_ref() {
                cancel.cancel();
            }
        } else {
            alarm.last_run_at = Some(now.timestamp());
            let pending_run = alarm.trigger.is_idle_recurring();
            self.alarms.insert(
                alarm.id.clone(),
                AlarmRuntime {
                    alarm: alarm.clone(),
                    pending_run,
                    timer_cancel,
                },
            );
        }
        Some(ClaimedAlarm {
            alarm: alarm.clone(),
            context: AlarmInvocationContext {
                current_alarm_id: alarm.id,
                trigger: alarm.trigger,
                prompt: alarm.prompt,
                recurring: is_recurring,
                delivery: actual_delivery,
            },
            deleted_one_shot_alarm,
        })
    }
}

pub(crate) fn alarm_prompt_input_item(alarm: &AlarmInvocationContext) -> ResponseInputItem {
    let text = if alarm.recurring {
        render_alarm_prompt_template(RECURRING_ALARM_PROMPT, alarm)
    } else {
        render_alarm_prompt_template(ONE_SHOT_ALARM_PROMPT, alarm)
    };
    ResponseInputItem::Message {
        role: "developer".to_string(),
        content: vec![ContentItem::InputText { text }],
    }
}

fn render_alarm_prompt_template(template: &str, alarm: &AlarmInvocationContext) -> String {
    template
        .replace("{{CURRENT_ALARM_ID}}", &alarm.current_alarm_id)
        .replace("{{TRIGGER}}", &alarm.trigger.display())
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
    let raw_alarms: Vec<serde_json::Value> = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse alarm sidecar `{}`: {err}",
            sidecar_path.display()
        )
    })?;
    let mut alarms = Vec::new();
    for raw_alarm in raw_alarms {
        match serde_json::from_value::<PersistedAlarm>(raw_alarm) {
            Ok(alarm) => alarms.push(alarm),
            Err(err) => {
                tracing::warn!(
                    "skipping invalid persisted alarm from `{}`: {err}",
                    sidecar_path.display()
                );
            }
        }
    }
    Ok(alarms)
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
    match tokio::fs::rename(&tmp_path, &sidecar_path).await {
        Ok(()) => Ok(()),
        Err(initial_error) => {
            #[cfg(target_os = "windows")]
            {
                match tokio::fs::remove_file(&sidecar_path).await {
                    Ok(()) => {
                        tokio::fs::rename(&tmp_path, &sidecar_path)
                            .await
                            .map_err(|err| {
                                format!(
                                    "failed to replace alarm sidecar `{}` with `{}`: {err}",
                                    sidecar_path.display(),
                                    tmp_path.display()
                                )
                            })?;
                        return Ok(());
                    }
                    Err(err) if err.kind() == ErrorKind::NotFound => {}
                    Err(err) => {
                        let _ = tokio::fs::remove_file(&tmp_path).await;
                        return Err(format!(
                            "failed to remove existing alarm sidecar `{}` before replace: {err}",
                            sidecar_path.display()
                        ));
                    }
                }
            }

            let _ = tokio::fs::remove_file(&tmp_path).await;
            Err(format!(
                "failed to atomically replace alarm sidecar `{}` with `{}`: {initial_error}",
                sidecar_path.display(),
                tmp_path.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AlarmDelivery;
    use super::AlarmInvocationContext;
    use super::AlarmsState;
    use super::CreateAlarm;
    use super::MAX_ACTIVE_ALARMS_PER_THREAD;
    use super::PersistedAlarm;
    use super::ThreadAlarm;
    use super::alarm_prompt_input_item;
    use super::alarm_sidecar_path_for_rollout;
    use super::load_alarm_sidecar;
    use super::write_alarm_sidecar;
    use crate::alarm_trigger::AlarmTrigger;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    const ZERO_SECONDS: u64 = 0;
    const TEN_SECONDS: u64 = 10;
    const SIXTY_SECONDS: u64 = 60;

    fn delay(seconds: u64, repeat: Option<bool>) -> AlarmTrigger {
        AlarmTrigger::Delay { seconds, repeat }
    }

    #[test]
    fn claim_one_shot_alarm_removes_it() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        let (alarm, timer_spec) = alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-1".to_string(),
                    trigger: delay(ZERO_SECONDS, /*repeat*/ None),
                    prompt: "run tests".to_string(),
                    delivery: AlarmDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");
        assert_eq!(timer_spec, None);
        assert_eq!(alarms.list_alarms(), vec![alarm]);

        let claimed = alarms
            .claim_next_alarm(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .expect("alarm should be claimed");
        assert_eq!(claimed.context.current_alarm_id, "alarm-1");
        assert!(claimed.deleted_one_shot_alarm);
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
                    trigger: delay(TEN_SECONDS, Some(true)),
                    prompt: "older recurring alarm".to_string(),
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
                    trigger: delay(TEN_SECONDS, Some(true)),
                    prompt: "newer recurring alarm".to_string(),
                    delivery: AlarmDelivery::AfterTurn,
                    now: create_second,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");
        alarms.mark_alarm_due("alarm-1", first_claimed_at);
        alarms.mark_alarm_due("alarm-2", first_claimed_at);

        let first = alarms
            .claim_next_alarm(
                first_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
            )
            .expect("first alarm should be claimed");
        assert_eq!(first.context.current_alarm_id, "alarm-1");

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
    fn idle_recurring_alarm_remains_pending_after_claim() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        let (alarm, timer_spec) = alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-1".to_string(),
                    trigger: delay(ZERO_SECONDS, Some(true)),
                    prompt: "keep going".to_string(),
                    delivery: AlarmDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");
        assert_eq!(timer_spec, None);

        let claimed = alarms
            .claim_next_alarm(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .expect("alarm should be claimed");
        assert!(!claimed.deleted_one_shot_alarm);
        assert_eq!(
            alarms.persisted_alarms(),
            vec![PersistedAlarm {
                alarm: ThreadAlarm {
                    last_run_at: Some(100),
                    ..alarm
                },
                pending_run: true,
            }]
        );
    }

    #[test]
    fn idle_recurring_alarm_waits_for_idle_even_if_delivery_requests_steer() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut alarms = AlarmsState::default();
        alarms
            .create_alarm(
                CreateAlarm {
                    id: "alarm-1".to_string(),
                    trigger: delay(ZERO_SECONDS, Some(true)),
                    prompt: "keep going".to_string(),
                    delivery: AlarmDelivery::SteerCurrentTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("alarm should be created");

        assert_eq!(
            alarms.claim_next_alarm(
                now, /*can_after_turn*/ false, /*can_steer_current_turn*/ true,
            ),
            None
        );
        let claimed = alarms
            .claim_next_alarm(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ false,
            )
            .expect("alarm should be claimed when idle");
        assert_eq!(claimed.context.delivery, AlarmDelivery::AfterTurn);
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
                        trigger: delay(SIXTY_SECONDS, Some(true)),
                        prompt: format!("prompt-{index}"),
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
                trigger: delay(SIXTY_SECONDS, Some(true)),
                prompt: "overflow".to_string(),
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
            trigger: delay(TEN_SECONDS, Some(true)),
            prompt: "run tests".to_string(),
            recurring: true,
            delivery: AlarmDelivery::SteerCurrentTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Recurring scheduled alarm prompt:\nrun tests\n\ncurrentAlarmId: alarm-1\nConfigured delivery: steer-current-turn\nTrigger: delay 10s, repeat\n\nThis alarm should keep running on its schedule unless the user asked for a stopping condition and that condition is now satisfied.\nIf that stopping condition is satisfied, stop the alarm by calling AlarmDelete with {\"id\":\"alarm-1\"}.\nDo not expose scheduler internals unless they matter to the user.".to_string(),
                }],
            }
        );
    }

    #[test]
    fn one_shot_alarm_prompt_input_omits_delete_instruction() {
        let item = alarm_prompt_input_item(&AlarmInvocationContext {
            current_alarm_id: "alarm-1".to_string(),
            trigger: delay(ZERO_SECONDS, /*repeat*/ None),
            prompt: "run tests once".to_string(),
            recurring: false,
            delivery: AlarmDelivery::AfterTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "One-shot scheduled alarm prompt:\nrun tests once\n\ncurrentAlarmId: alarm-1\nConfigured delivery: after-turn\nTrigger: delay 0s\n\nThis one-shot alarm has already been removed from the schedule, so you do not need to call AlarmDelete.\nDo not expose scheduler internals unless they matter to the user.".to_string(),
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
                trigger: delay(ZERO_SECONDS, /*repeat*/ None),
                prompt: "run tests".to_string(),
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

    #[tokio::test]
    async fn alarm_sidecar_overwrites_existing_file() {
        let tempdir = TempDir::new().expect("tempdir");
        let rollout_path = tempdir.path().join("rollout.jsonl");
        let original = vec![PersistedAlarm {
            alarm: super::ThreadAlarm {
                id: "alarm-1".to_string(),
                trigger: delay(ZERO_SECONDS, /*repeat*/ None),
                prompt: "run tests".to_string(),
                delivery: AlarmDelivery::AfterTurn,
                created_at: 1,
                next_run_at: None,
                last_run_at: None,
            },
            pending_run: true,
        }];
        let replacement = vec![PersistedAlarm {
            alarm: super::ThreadAlarm {
                id: "alarm-2".to_string(),
                trigger: delay(SIXTY_SECONDS, Some(true)),
                prompt: "run different tests".to_string(),
                delivery: AlarmDelivery::SteerCurrentTurn,
                created_at: 2,
                next_run_at: None,
                last_run_at: Some(3),
            },
            pending_run: false,
        }];

        write_alarm_sidecar(&rollout_path, &original)
            .await
            .expect("write original sidecar");
        write_alarm_sidecar(&rollout_path, &replacement)
            .await
            .expect("overwrite sidecar");

        let loaded = load_alarm_sidecar(&rollout_path)
            .await
            .expect("load overwritten sidecar");
        assert_eq!(loaded, replacement);
    }

    #[tokio::test]
    async fn alarm_sidecar_skips_invalid_entries() {
        let tempdir = TempDir::new().expect("tempdir");
        let rollout_path = tempdir.path().join("rollout.jsonl");
        let sidecar_path = alarm_sidecar_path_for_rollout(&rollout_path);
        tokio::fs::write(
            &sidecar_path,
            r#"[
              { "alarm": { "id": "old", "cronExpression": "@after-turn" }, "pending_run": true },
              {
                "alarm": {
                  "id": "alarm-1",
                  "trigger": { "kind": "delay", "seconds": 0, "repeat": null },
                  "prompt": "run tests",
                  "delivery": "after-turn",
                  "created_at": 1,
                  "next_run_at": null,
                  "last_run_at": null
                },
                "pending_run": true
              }
            ]"#,
        )
        .await
        .expect("write sidecar");

        let loaded = load_alarm_sidecar(&rollout_path)
            .await
            .expect("load sidecar");

        assert_eq!(
            loaded,
            vec![PersistedAlarm {
                alarm: super::ThreadAlarm {
                    id: "alarm-1".to_string(),
                    trigger: delay(ZERO_SECONDS, /*repeat*/ None),
                    prompt: "run tests".to_string(),
                    delivery: AlarmDelivery::AfterTurn,
                    created_at: 1,
                    next_run_at: None,
                    last_run_at: None,
                },
                pending_run: true,
            }]
        );
    }
}
