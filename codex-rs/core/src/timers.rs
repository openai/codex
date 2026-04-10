//! Persistent thread-local timer scheduling for follow-on turns and same-turn steer delivery.
//!
//! This module owns the in-memory timer registry, trigger evaluation, the user
//! message injected when a timer fires, and the persistent state shape used to
//! restore timers after a harness restart.

use crate::messages::MessageInvocationContext;
use crate::messages::MessagePayload;
use crate::messages::validate_meta;
use crate::timer_trigger::TimerTrigger;
use crate::timer_trigger::TriggerTiming;
use crate::timer_trigger::next_run_after_due;
use crate::timer_trigger::normalize_schedule_dtstart_input;
use crate::timer_trigger::timing_for_new_trigger;
use crate::timer_trigger::timing_for_restored_trigger;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub const TIMER_UPDATED_BACKGROUND_EVENT_PREFIX: &str = "timer_updated:";
pub const TIMER_FIRED_BACKGROUND_EVENT_PREFIX: &str = "timer_fired:";
pub const MAX_ACTIVE_TIMERS_PER_THREAD: usize = 256;
const RECURRING_TIMER_INSTRUCTIONS: &str = "This timer should keep running on its schedule after this invocation.\nDo not call delete_timer just because you completed this invocation.\nCall delete_timer with {\"id\":\"{{CURRENT_TIMER_ID}}\"} only if the user's timer message included an explicit stop condition, such as \"until\", \"stop when\", or \"while\", and that condition is now satisfied.\nDo not expose scheduler internals unless they matter to the user.";
const ONE_SHOT_TIMER_INSTRUCTIONS: &str = "This one-shot timer has already been removed from the schedule, so you do not need to call delete_timer.\nDo not expose scheduler internals unless they matter to the user.";
const TIMER_CONTENT_PREVIEW_MAX_CHARS: usize = 160;

pub use crate::timer_trigger::TimerTrigger as ThreadTimerTrigger;

pub fn normalize_thread_timer_dtstart_input(input: &str) -> Result<String, String> {
    normalize_schedule_dtstart_input(input)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimerDelivery {
    AfterTurn,
    SteerCurrentTurn,
}

impl TimerDelivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AfterTurn => "after-turn",
            Self::SteerCurrentTurn => "steer-current-turn",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadTimer {
    pub id: String,
    pub trigger: TimerTrigger,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
    pub delivery: TimerDelivery,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimerInvocationContext {
    pub(crate) current_timer_id: String,
    pub(crate) content: String,
    pub(crate) instructions: Option<String>,
    pub(crate) meta: BTreeMap<String, String>,
    pub(crate) recurring: bool,
    pub(crate) delivery: TimerDelivery,
    pub(crate) queued_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimedTimer {
    pub(crate) timer: ThreadTimer,
    pub(crate) context: TimerInvocationContext,
    pub(crate) deleted_one_shot_timer: bool,
    pub(crate) previous_last_run_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleRecurringTimerPolicy {
    IncludeOnlyNeverRun,
    IncludeAll,
}

#[derive(Debug)]
pub(crate) struct CreateTimer {
    pub(crate) id: String,
    pub(crate) trigger: TimerTrigger,
    pub(crate) payload: MessagePayload,
    pub(crate) delivery: TimerDelivery,
    pub(crate) now: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadTimerStorageCreateParams {
    pub thread_id: String,
    pub source: String,
    pub client_id: String,
    pub trigger: TimerTrigger,
    pub payload: MessagePayload,
    pub delivery: TimerDelivery,
}

#[derive(Debug, Default)]
pub(crate) struct TimersState {
    timers: HashMap<String, TimerRuntime>,
}

#[derive(Debug)]
pub(crate) struct TimerRuntime {
    pub(crate) timer: ThreadTimer,
    pending_run: bool,
    pub(crate) timer_cancel: Option<CancellationToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedTimer {
    pub(crate) timer: ThreadTimer,
    pub(crate) pending_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimerTaskSpec {
    pub(crate) delay: std::time::Duration,
}

#[derive(Debug)]
pub(crate) struct RestoredTimerTask {
    pub(crate) id: String,
    pub(crate) timer_spec: TimerTaskSpec,
    pub(crate) timer_cancel: CancellationToken,
}

impl TimersState {
    pub(crate) fn list_timers(&self) -> Vec<ThreadTimer> {
        let mut timers = self
            .timers
            .values()
            .map(|runtime| runtime.timer.clone())
            .collect::<Vec<_>>();
        timers.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        timers
    }

    pub(crate) fn persisted_timers(&self) -> Vec<PersistedTimer> {
        let mut timers = self
            .timers
            .values()
            .map(|runtime| PersistedTimer {
                timer: runtime.timer.clone(),
                pending_run: runtime.pending_run,
            })
            .collect::<Vec<_>>();
        timers.sort_by(|left, right| {
            left.timer
                .created_at
                .cmp(&right.timer.created_at)
                .then_with(|| left.timer.id.cmp(&right.timer.id))
        });
        timers
    }

    pub(crate) fn persisted_timer(&self, id: &str) -> Option<PersistedTimer> {
        self.timers.get(id).map(|runtime| PersistedTimer {
            timer: runtime.timer.clone(),
            pending_run: runtime.pending_run,
        })
    }

    pub(crate) fn replace_timers_if_changed(
        &mut self,
        persisted: Vec<PersistedTimer>,
        now: chrono::DateTime<Utc>,
    ) -> (bool, Vec<RestoredTimerTask>) {
        if self.persisted_timers() == persisted {
            return (false, Vec::new());
        }

        for runtime in self.timers.values() {
            Self::cancel_runtime(runtime);
        }
        self.timers.clear();

        let mut restored_tasks = Vec::new();
        for persisted_timer in persisted {
            let timer_cancel = CancellationToken::new();
            let timer_id = persisted_timer.timer.id.clone();
            match self.restore_timer(persisted_timer, now, Some(timer_cancel.clone())) {
                Ok(Some(timer_spec)) => {
                    restored_tasks.push(RestoredTimerTask {
                        id: timer_id,
                        timer_spec,
                        timer_cancel,
                    });
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!("skipping invalid persisted timer {timer_id}: {err}");
                }
            }
        }
        (true, restored_tasks)
    }

    pub(crate) fn create_timer(
        &mut self,
        create_timer: CreateTimer,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<(ThreadTimer, Option<TimerTaskSpec>), String> {
        if self.timers.len() >= MAX_ACTIVE_TIMERS_PER_THREAD {
            return Err(format!(
                "too many active timers; each thread supports at most {MAX_ACTIVE_TIMERS_PER_THREAD} timers"
            ));
        }
        let CreateTimer {
            id,
            trigger,
            payload,
            delivery,
            now,
        } = create_timer;
        let TriggerTiming {
            trigger,
            pending_run,
            next_run_at,
            timer_delay,
        } = timing_for_new_trigger(trigger, now, now)?;
        let timer = ThreadTimer {
            id: id.clone(),
            trigger,
            content: payload.content,
            instructions: payload.instructions,
            meta: payload.meta,
            delivery,
            created_at: now.timestamp(),
            next_run_at,
            last_run_at: None,
        };
        self.timers.insert(
            id,
            TimerRuntime {
                timer: timer.clone(),
                pending_run,
                timer_cancel,
            },
        );
        Ok((timer, timer_delay.map(|delay| TimerTaskSpec { delay })))
    }

    pub(crate) fn restore_timer(
        &mut self,
        persisted: PersistedTimer,
        now: chrono::DateTime<Utc>,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<Option<TimerTaskSpec>, String> {
        if self.timers.len() >= MAX_ACTIVE_TIMERS_PER_THREAD {
            return Err(format!(
                "too many persisted timers; each thread supports at most {MAX_ACTIVE_TIMERS_PER_THREAD} timers"
            ));
        }
        let PersistedTimer {
            timer,
            pending_run: persisted_pending_run,
        } = persisted;
        let TriggerTiming {
            trigger,
            pending_run,
            next_run_at,
            timer_delay,
        } = timing_for_restored_trigger(
            timer.trigger,
            timer.created_at,
            persisted_pending_run,
            timer.next_run_at,
            now,
        )?;
        let timer = ThreadTimer {
            trigger,
            next_run_at,
            ..timer
        };
        let id = timer.id.clone();
        self.timers.insert(
            id,
            TimerRuntime {
                timer,
                pending_run,
                timer_cancel,
            },
        );
        Ok(timer_delay.map(|delay| TimerTaskSpec { delay }))
    }

    pub(crate) fn remove_timer(&mut self, id: &str) -> Option<TimerRuntime> {
        self.timers.remove(id)
    }

    pub(crate) fn restore_runtime(&mut self, runtime: TimerRuntime) {
        self.timers.insert(runtime.timer.id.clone(), runtime);
    }

    pub(crate) fn cancel_runtime(runtime: &TimerRuntime) {
        if let Some(cancel) = runtime.timer_cancel.as_ref() {
            cancel.cancel();
        }
    }

    pub(crate) fn mark_timer_due(&mut self, id: &str, now: chrono::DateTime<Utc>) -> bool {
        let Some(runtime) = self.timers.get_mut(id) else {
            return false;
        };
        let mut changed = !runtime.pending_run;
        runtime.pending_run = true;
        match next_run_after_due(&runtime.timer.trigger, runtime.timer.created_at, now) {
            Ok(next_run_at) if runtime.timer.next_run_at != next_run_at => {
                runtime.timer.next_run_at = next_run_at;
                changed = true;
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    "failed to advance timer {} trigger: {err}",
                    runtime.timer.id
                );
            }
        }
        changed
    }

    pub(crate) fn timer_spec_for_timer(
        &self,
        id: &str,
        now: chrono::DateTime<Utc>,
    ) -> Option<TimerTaskSpec> {
        let runtime = self.timers.get(id)?;
        let next_run_at = runtime.timer.next_run_at?;
        if runtime.pending_run && !runtime.timer.trigger.is_recurring() {
            return None;
        }
        Some(TimerTaskSpec {
            delay: if next_run_at <= now.timestamp() {
                std::time::Duration::ZERO
            } else {
                let delay = u64::try_from(next_run_at - now.timestamp()).ok()?;
                std::time::Duration::from_secs(delay)
            },
        })
    }

    pub(crate) fn claim_next_timer(
        &mut self,
        now: chrono::DateTime<Utc>,
        can_after_turn: bool,
        can_steer_current_turn: bool,
        idle_recurring_timer_policy: IdleRecurringTimerPolicy,
    ) -> Option<ClaimedTimer> {
        let (next_timer_id, actual_delivery) = self
            .timers
            .values()
            .filter(|runtime| runtime.pending_run)
            .filter_map(|runtime| {
                if runtime.timer.trigger.is_idle_recurring() {
                    if idle_recurring_timer_policy == IdleRecurringTimerPolicy::IncludeOnlyNeverRun
                        && runtime.timer.last_run_at.is_some()
                    {
                        return None;
                    }
                    if can_after_turn {
                        return Some((runtime, TimerDelivery::AfterTurn));
                    }
                    return None;
                }
                let actual_delivery = match runtime.timer.delivery {
                    TimerDelivery::AfterTurn if can_after_turn => TimerDelivery::AfterTurn,
                    TimerDelivery::AfterTurn => return None,
                    TimerDelivery::SteerCurrentTurn if can_steer_current_turn => {
                        TimerDelivery::SteerCurrentTurn
                    }
                    TimerDelivery::SteerCurrentTurn if can_after_turn => TimerDelivery::AfterTurn,
                    TimerDelivery::SteerCurrentTurn => return None,
                };
                Some((runtime, actual_delivery))
            })
            .min_by(|(left, _), (right, _)| {
                left.timer
                    .last_run_at
                    .unwrap_or(left.timer.created_at)
                    .cmp(&right.timer.last_run_at.unwrap_or(right.timer.created_at))
                    .then_with(|| left.timer.created_at.cmp(&right.timer.created_at))
                    .then_with(|| left.timer.id.cmp(&right.timer.id))
            })
            .map(|(runtime, actual_delivery)| (runtime.timer.id.clone(), actual_delivery))?;

        let runtime = self.timers.remove(&next_timer_id)?;
        let TimerRuntime {
            mut timer,
            pending_run: _,
            timer_cancel,
        } = runtime;
        let is_recurring = timer.trigger.is_recurring();
        let delete_after_claim =
            !is_recurring || (!timer.trigger.is_idle_recurring() && timer.next_run_at.is_none());
        let previous_last_run_at = timer.last_run_at;
        if delete_after_claim {
            if let Some(cancel) = timer_cancel.as_ref() {
                cancel.cancel();
            }
        } else {
            timer.last_run_at = Some(
                previous_last_run_at
                    .map(|previous| now.timestamp().max(previous.saturating_add(1)))
                    .unwrap_or_else(|| now.timestamp()),
            );
            let pending_run = timer.trigger.is_idle_recurring();
            self.timers.insert(
                timer.id.clone(),
                TimerRuntime {
                    timer: timer.clone(),
                    pending_run,
                    timer_cancel,
                },
            );
        }
        Some(ClaimedTimer {
            timer: timer.clone(),
            context: TimerInvocationContext {
                current_timer_id: timer.id,
                content: timer.content,
                instructions: timer.instructions,
                meta: timer.meta,
                recurring: !delete_after_claim,
                delivery: actual_delivery,
                queued_at: now.timestamp(),
            },
            deleted_one_shot_timer: delete_after_claim,
            previous_last_run_at,
        })
    }
}

pub fn build_thread_timer_create_params(
    params: ThreadTimerStorageCreateParams,
) -> Result<codex_state::ThreadTimerCreateParams, String> {
    validate_meta(&params.payload.meta)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    let TriggerTiming {
        trigger,
        pending_run,
        next_run_at,
        timer_delay: _,
    } = timing_for_new_trigger(params.trigger, now, now)?;
    Ok(codex_state::ThreadTimerCreateParams {
        id,
        thread_id: params.thread_id,
        source: params.source,
        client_id: params.client_id,
        trigger_json: serde_json::to_string(&trigger)
            .map_err(|err| format!("failed to serialize timer trigger: {err}"))?,
        content: params.payload.content,
        instructions: params.payload.instructions,
        meta_json: serde_json::to_string(&params.payload.meta)
            .map_err(|err| format!("failed to serialize timer metadata: {err}"))?,
        delivery: params.delivery.as_str().to_string(),
        created_at: now.timestamp(),
        next_run_at,
        last_run_at: None,
        pending_run,
    })
}

#[cfg(test)]
pub(crate) fn timer_prompt_input_item(
    timer: &TimerInvocationContext,
) -> codex_protocol::models::ResponseInputItem {
    crate::messages::message_prompt_input_item(&timer_message_invocation_context(timer))
}

pub(crate) fn timer_message_invocation_context(
    timer: &TimerInvocationContext,
) -> MessageInvocationContext {
    MessageInvocationContext {
        source: format!("timer {}", timer.current_timer_id),
        content: timer_message_content(timer),
        instructions: timer_message_instructions(timer),
        meta: timer.meta.clone(),
        queued_at: timer.queued_at,
    }
}

fn timer_message_content(timer: &TimerInvocationContext) -> String {
    let preview = timer
        .content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if preview.is_empty() {
        return "Timer fired.".to_string();
    }

    let preview = if preview.chars().count() > TIMER_CONTENT_PREVIEW_MAX_CHARS {
        let mut truncated = preview
            .chars()
            .take(TIMER_CONTENT_PREVIEW_MAX_CHARS.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        preview
    };
    format!("Timer fired: {preview}")
}

fn timer_message_instructions(timer: &TimerInvocationContext) -> Option<String> {
    let timer_instructions = if timer.recurring {
        RECURRING_TIMER_INSTRUCTIONS.replace("{{CURRENT_TIMER_ID}}", &timer.current_timer_id)
    } else {
        ONE_SHOT_TIMER_INSTRUCTIONS.to_string()
    };
    let content = timer.content.trim();
    let mut parts = Vec::new();
    if !content.is_empty() {
        parts.push(content.to_string());
    }
    if let Some(instructions) = timer.instructions.as_deref()
        && !instructions.trim().is_empty()
    {
        parts.push(instructions.trim().to_string());
    }
    parts.push(timer_instructions);
    Some(parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::CreateTimer;
    use super::IdleRecurringTimerPolicy;
    use super::MAX_ACTIVE_TIMERS_PER_THREAD;
    use super::PersistedTimer;
    use super::ThreadTimer;
    use super::TimerDelivery;
    use super::TimerInvocationContext;
    use super::TimersState;
    use super::timer_prompt_input_item;
    use crate::messages::MessagePayload;
    use crate::timer_trigger::TimerTrigger;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    const ZERO_SECONDS: u64 = 0;
    const TEN_SECONDS: u64 = 10;
    const SIXTY_SECONDS: u64 = 60;

    fn delay(seconds: u64, repeat: Option<bool>) -> TimerTrigger {
        TimerTrigger::Delay { seconds, repeat }
    }

    #[test]
    fn claim_one_shot_timer_removes_it() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        let (timer, timer_spec) = timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: delay(ZERO_SECONDS, /*repeat*/ None),
                    payload: MessagePayload {
                        content: "run tests".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        assert_eq!(timer_spec, None);
        assert_eq!(timers.list_timers(), vec![timer]);

        let claimed = timers
            .claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("timer should be claimed");
        assert_eq!(claimed.context.current_timer_id, "timer-1");
        assert!(claimed.deleted_one_shot_timer);
        assert!(timers.list_timers().is_empty());
    }

    #[test]
    fn exhausted_recurring_schedule_is_removed_after_final_claim() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        let (timer, timer_spec) = timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: TimerTrigger::Schedule {
                        dtstart: None,
                        rrule: Some("FREQ=MINUTELY;COUNT=1".to_string()),
                    },
                    payload: MessagePayload {
                        content: "final scheduled run".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        assert_eq!(timer_spec, None);
        assert_eq!(
            timers.persisted_timers(),
            vec![PersistedTimer {
                timer: ThreadTimer {
                    next_run_at: None,
                    ..timer
                },
                pending_run: true,
            }]
        );

        let claimed = timers
            .claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("timer should be claimed");
        assert!(claimed.deleted_one_shot_timer);
        assert!(!claimed.context.recurring);
        assert!(timers.list_timers().is_empty());
    }

    #[test]
    fn claim_next_timer_prefers_pending_timer_that_ran_least_recently() {
        let create_first = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let create_second = Utc.timestamp_opt(101, 0).single().expect("valid timestamp");
        let first_claimed_at = Utc.timestamp_opt(110, 0).single().expect("valid timestamp");
        let second_claimed_at = Utc.timestamp_opt(111, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: delay(TEN_SECONDS, Some(true)),
                    payload: MessagePayload {
                        content: "older recurring timer".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now: create_first,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        timers
            .create_timer(
                CreateTimer {
                    id: "timer-2".to_string(),
                    trigger: delay(TEN_SECONDS, Some(true)),
                    payload: MessagePayload {
                        content: "newer recurring timer".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now: create_second,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        timers.mark_timer_due("timer-1", first_claimed_at);
        timers.mark_timer_due("timer-2", first_claimed_at);

        let first = timers
            .claim_next_timer(
                first_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("first timer should be claimed");
        assert_eq!(first.context.current_timer_id, "timer-1");

        let second = timers
            .claim_next_timer(
                second_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("second timer should be claimed");
        assert_eq!(second.context.current_timer_id, "timer-2");
    }

    #[test]
    fn idle_recurring_timer_remains_pending_after_claim() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        let (timer, timer_spec) = timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: delay(ZERO_SECONDS, Some(true)),
                    payload: MessagePayload {
                        content: "keep going".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        assert_eq!(timer_spec, None);

        let claimed = timers
            .claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("timer should be claimed");
        assert!(!claimed.deleted_one_shot_timer);
        assert_eq!(
            timers.persisted_timers(),
            vec![PersistedTimer {
                timer: ThreadTimer {
                    last_run_at: Some(100),
                    ..timer
                },
                pending_run: true,
            }]
        );
    }

    #[test]
    fn idle_recurring_timer_waits_for_idle_even_if_delivery_requests_steer() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: delay(ZERO_SECONDS, Some(true)),
                    payload: MessagePayload {
                        content: "keep going".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::SteerCurrentTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");

        assert_eq!(
            timers.claim_next_timer(
                now,
                /*can_after_turn*/ false,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeAll,
            ),
            None
        );
        let claimed = timers
            .claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ false,
                IdleRecurringTimerPolicy::IncludeAll,
            )
            .expect("timer should be claimed when idle");
        assert_eq!(claimed.context.delivery, TimerDelivery::AfterTurn);
    }

    #[test]
    fn idle_recurring_policy_can_exclude_timer_that_already_ran() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        timers
            .create_timer(
                CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: delay(ZERO_SECONDS, Some(true)),
                    payload: MessagePayload {
                        content: "keep going".to_string(),
                        instructions: None,
                        meta: BTreeMap::new(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");

        let claimed = timers
            .claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeOnlyNeverRun,
            )
            .expect("never-run idle timer should be claimed");
        assert_eq!(claimed.context.current_timer_id, "timer-1");

        assert_eq!(
            timers.claim_next_timer(
                now,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
                IdleRecurringTimerPolicy::IncludeOnlyNeverRun,
            ),
            None
        );
    }

    #[test]
    fn create_timer_rejects_more_than_maximum_active_timers() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut timers = TimersState::default();
        for index in 0..MAX_ACTIVE_TIMERS_PER_THREAD {
            timers
                .create_timer(
                    CreateTimer {
                        id: format!("timer-{index}"),
                        trigger: delay(SIXTY_SECONDS, Some(true)),
                        payload: MessagePayload {
                            content: format!("content-{index}"),
                            instructions: None,
                            meta: BTreeMap::new(),
                        },
                        delivery: TimerDelivery::AfterTurn,
                        now,
                    },
                    /*timer_cancel*/ None,
                )
                .expect("timer should be created");
        }

        let result = timers.create_timer(
            CreateTimer {
                id: "timer-overflow".to_string(),
                trigger: delay(SIXTY_SECONDS, Some(true)),
                payload: MessagePayload {
                    content: "overflow".to_string(),
                    instructions: None,
                    meta: BTreeMap::new(),
                },
                delivery: TimerDelivery::AfterTurn,
                now,
            },
            /*timer_cancel*/ None,
        );

        assert_eq!(
            result,
            Err(format!(
                "too many active timers; each thread supports at most {MAX_ACTIVE_TIMERS_PER_THREAD} timers"
            ))
        );
    }

    #[test]
    fn timer_prompt_input_is_visible_user_input() {
        let item = timer_prompt_input_item(&TimerInvocationContext {
            current_timer_id: "timer-1".to_string(),
            content: "run tests".to_string(),
            instructions: None,
            meta: BTreeMap::new(),
            recurring: true,
            delivery: TimerDelivery::SteerCurrentTurn,
            queued_at: 100,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<codex_message>\n<source>timer timer-1</source>\n<queued_at>100</queued_at>\n<content>\nTimer fired: run tests\n</content>\n<instructions>\nrun tests\n\nThis timer should keep running on its schedule after this invocation.\nDo not call delete_timer just because you completed this invocation.\nCall delete_timer with {&quot;id&quot;:&quot;timer-1&quot;} only if the user's timer message included an explicit stop condition, such as &quot;until&quot;, &quot;stop when&quot;, or &quot;while&quot;, and that condition is now satisfied.\nDo not expose scheduler internals unless they matter to the user.\n</instructions>\n<meta />\n</codex_message>".to_string(),
                }],
            }
        );
    }

    #[test]
    fn one_shot_timer_prompt_input_omits_delete_instruction() {
        let item = timer_prompt_input_item(&TimerInvocationContext {
            current_timer_id: "timer-1".to_string(),
            content: "run tests once".to_string(),
            instructions: Some("user-specific instruction".to_string()),
            meta: BTreeMap::from([("ticket".to_string(), "ABC_123".to_string())]),
            recurring: false,
            delivery: TimerDelivery::AfterTurn,
            queued_at: 101,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<codex_message>\n<source>timer timer-1</source>\n<queued_at>101</queued_at>\n<content>\nTimer fired: run tests once\n</content>\n<instructions>\nrun tests once\n\nuser-specific instruction\n\nThis one-shot timer has already been removed from the schedule, so you do not need to call delete_timer.\nDo not expose scheduler internals unless they matter to the user.\n</instructions>\n<meta>\n  <entry id=\"ticket\">ABC_123</entry>\n</meta>\n</codex_message>".to_string(),
                }],
            }
        );
    }
}
