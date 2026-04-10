//! Persistent thread-local timer scheduling for follow-on turns and same-turn steer delivery.
//!
//! This module owns the in-memory timer registry, trigger evaluation, the user
//! message injected when a timer fires, and the persistent state shape used to
//! restore timers after a harness restart.

use crate::timer_trigger::TimerTrigger;
use crate::timer_trigger::TriggerTiming;
use crate::timer_trigger::next_run_after_due;
use crate::timer_trigger::timing_for_new_trigger;
use crate::timer_trigger::timing_for_restored_trigger;
use chrono::Utc;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub const TIMER_UPDATED_BACKGROUND_EVENT_PREFIX: &str = "timer_updated:";
pub const TIMER_FIRED_BACKGROUND_EVENT_PREFIX: &str = "timer_fired:";
pub const MAX_ACTIVE_TIMERS_PER_THREAD: usize = 256;
const ONE_SHOT_TIMER_PROMPT: &str = include_str!("../templates/timers/one_shot_prompt.md");
const RECURRING_TIMER_PROMPT: &str = include_str!("../templates/timers/recurring_prompt.md");

pub use crate::timer_trigger::TimerTrigger as ThreadTimerTrigger;

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
    pub prompt: String,
    pub delivery: TimerDelivery,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimerInvocationContext {
    pub(crate) current_timer_id: String,
    pub(crate) trigger: TimerTrigger,
    pub(crate) prompt: String,
    pub(crate) recurring: bool,
    pub(crate) delivery: TimerDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimedTimer {
    pub(crate) timer: ThreadTimer,
    pub(crate) context: TimerInvocationContext,
    pub(crate) deleted_one_shot_timer: bool,
}

#[derive(Debug)]
pub(crate) struct CreateTimer {
    pub(crate) id: String,
    pub(crate) trigger: TimerTrigger,
    pub(crate) prompt: String,
    pub(crate) delivery: TimerDelivery,
    pub(crate) now: chrono::DateTime<Utc>,
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
            prompt,
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
            prompt,
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
    ) -> Option<ClaimedTimer> {
        let (next_timer_id, actual_delivery) = self
            .timers
            .values()
            .filter(|runtime| runtime.pending_run)
            .filter_map(|runtime| {
                if runtime.timer.trigger.is_idle_recurring() {
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
        let deleted_one_shot_timer = !is_recurring;
        if deleted_one_shot_timer {
            if let Some(cancel) = timer_cancel.as_ref() {
                cancel.cancel();
            }
        } else {
            timer.last_run_at = Some(now.timestamp());
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
                trigger: timer.trigger,
                prompt: timer.prompt,
                recurring: is_recurring,
                delivery: actual_delivery,
            },
            deleted_one_shot_timer,
        })
    }
}

pub(crate) fn timer_prompt_input_item(timer: &TimerInvocationContext) -> ResponseInputItem {
    let text = if timer.recurring {
        render_timer_prompt_template(RECURRING_TIMER_PROMPT, timer)
    } else {
        render_timer_prompt_template(ONE_SHOT_TIMER_PROMPT, timer)
    };
    ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text }],
    }
}

fn render_timer_prompt_template(template: &str, timer: &TimerInvocationContext) -> String {
    template
        .replace("\r\n", "\n")
        .replace("{{CURRENT_TIMER_ID}}", &timer.current_timer_id)
        .replace("{{TRIGGER}}", &timer.trigger.display())
        .replace("{{PROMPT}}", &timer.prompt)
        .replace("{{DELIVERY}}", timer.delivery.as_str())
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::CreateTimer;
    use super::MAX_ACTIVE_TIMERS_PER_THREAD;
    use super::PersistedTimer;
    use super::ThreadTimer;
    use super::TimerDelivery;
    use super::TimerInvocationContext;
    use super::TimersState;
    use super::timer_prompt_input_item;
    use crate::timer_trigger::TimerTrigger;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;

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
                    prompt: "run tests".to_string(),
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
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .expect("timer should be claimed");
        assert_eq!(claimed.context.current_timer_id, "timer-1");
        assert!(claimed.deleted_one_shot_timer);
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
                    prompt: "older recurring timer".to_string(),
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
                    prompt: "newer recurring timer".to_string(),
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
            )
            .expect("first timer should be claimed");
        assert_eq!(first.context.current_timer_id, "timer-1");

        let second = timers
            .claim_next_timer(
                second_claimed_at,
                /*can_after_turn*/ true,
                /*can_steer_current_turn*/ true,
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
                    prompt: "keep going".to_string(),
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");
        assert_eq!(timer_spec, None);

        let claimed = timers
            .claim_next_timer(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
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
                    prompt: "keep going".to_string(),
                    delivery: TimerDelivery::SteerCurrentTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("timer should be created");

        assert_eq!(
            timers.claim_next_timer(
                now, /*can_after_turn*/ false, /*can_steer_current_turn*/ true,
            ),
            None
        );
        let claimed = timers
            .claim_next_timer(
                now, /*can_after_turn*/ true, /*can_steer_current_turn*/ false,
            )
            .expect("timer should be claimed when idle");
        assert_eq!(claimed.context.delivery, TimerDelivery::AfterTurn);
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
                        prompt: format!("prompt-{index}"),
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
                prompt: "overflow".to_string(),
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
            trigger: delay(TEN_SECONDS, Some(true)),
            prompt: "run tests".to_string(),
            recurring: true,
            delivery: TimerDelivery::SteerCurrentTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<timer_fired>\n<id>timer-1</id>\n<trigger>delay 10s, repeat</trigger>\n<delivery>steer-current-turn</delivery>\n<recurring>true</recurring>\n<prompt>\nrun tests\n</prompt>\n<instructions>\nThis timer should keep running on its schedule after this invocation.\nDo not call TimerDelete just because you completed this invocation.\nCall TimerDelete with {\"id\":\"timer-1\"} only if the user's timer prompt included an explicit stop condition, such as \"until\", \"stop when\", or \"while\", and that condition is now satisfied.\nDo not expose scheduler internals unless they matter to the user.\n</instructions>\n</timer_fired>".to_string(),
                }],
            }
        );
    }

    #[test]
    fn one_shot_timer_prompt_input_omits_delete_instruction() {
        let item = timer_prompt_input_item(&TimerInvocationContext {
            current_timer_id: "timer-1".to_string(),
            trigger: delay(ZERO_SECONDS, /*repeat*/ None),
            prompt: "run tests once".to_string(),
            recurring: false,
            delivery: TimerDelivery::AfterTurn,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<timer_fired>\n<id>timer-1</id>\n<trigger>delay 0s</trigger>\n<delivery>after-turn</delivery>\n<recurring>false</recurring>\n<prompt>\nrun tests once\n</prompt>\n<instructions>\nThis one-shot timer has already been removed from the schedule, so you do not need to call TimerDelete.\nDo not expose scheduler internals unless they matter to the user.\n</instructions>\n</timer_fired>".to_string(),
                }],
            }
        );
    }
}
