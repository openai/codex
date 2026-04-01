//! Runtime-only thread-local job scheduling for follow-on turns.
//!
//! This module owns the in-memory job registry, limited schedule parsing, and
//! the hidden turn context injected when a job fires so the model can act on
//! the scheduled prompt and deschedule itself via `JobDelete(currentJobId)`.

use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub const AFTER_TURN_CRON_EXPRESSION: &str = "@after-turn";
const EVERY_PREFIX: &str = "@every ";
const EVERY_SECONDS_PREFIX: &str = "@every:";
pub const JOB_UPDATED_BACKGROUND_EVENT_PREFIX: &str = "job_updated:";
pub const JOB_FIRED_BACKGROUND_EVENT_PREFIX: &str = "job_fired:";
pub const MAX_ACTIVE_JOBS_PER_THREAD: usize = 256;
const MAX_EVERY_SECONDS: u64 = i64::MAX as u64;
const ONE_SHOT_JOB_TURN_INSTRUCTIONS: &str =
    include_str!("../templates/jobs/one_shot_turn_instructions.md");
const RECURRING_JOB_TURN_INSTRUCTIONS: &str =
    include_str!("../templates/jobs/recurring_turn_instructions.md");
const ONE_SHOT_JOB_PROMPT: &str = include_str!("../templates/jobs/one_shot_prompt.md");
const RECURRING_JOB_PROMPT: &str = include_str!("../templates/jobs/recurring_prompt.md");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadJob {
    pub id: String,
    pub cron_expression: String,
    pub prompt: String,
    pub run_once: bool,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobTurnContext {
    pub(crate) current_job_id: String,
    pub(crate) cron_expression: String,
    pub(crate) prompt: String,
    pub(crate) run_once: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimedJob {
    pub(crate) job: ThreadJob,
    pub(crate) context: JobTurnContext,
    pub(crate) deleted_run_once_job: bool,
}

#[derive(Debug, Default)]
pub(crate) struct JobsState {
    jobs: HashMap<String, JobRuntime>,
}

#[derive(Debug)]
struct JobRuntime {
    job: ThreadJob,
    schedule: JobSchedule,
    pending_run: bool,
    timer_cancel: Option<CancellationToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobSchedule {
    AfterTurn,
    EverySeconds(u64),
}

impl JobSchedule {
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

impl JobsState {
    pub(crate) fn list_jobs(&self) -> Vec<ThreadJob> {
        let mut jobs = self
            .jobs
            .values()
            .map(|runtime| runtime.job.clone())
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        jobs
    }

    pub(crate) fn create_job(
        &mut self,
        id: String,
        cron_expression: String,
        prompt: String,
        run_once: bool,
        now: DateTime<Utc>,
        timer_cancel: Option<CancellationToken>,
    ) -> Result<ThreadJob, String> {
        if self.jobs.len() >= MAX_ACTIVE_JOBS_PER_THREAD {
            return Err(format!(
                "too many active jobs; each thread supports at most {MAX_ACTIVE_JOBS_PER_THREAD} jobs"
            ));
        }
        let schedule = JobSchedule::parse(&cron_expression)?;
        let next_run_at = match schedule {
            JobSchedule::AfterTurn => None,
            JobSchedule::EverySeconds(_) => Some(schedule.next_run_at(now).ok_or_else(|| {
                format!(
                    "unsupported cron_expression `{cron_expression}`; next run time is out of range"
                )
            })?),
        };
        let job = ThreadJob {
            id: id.clone(),
            cron_expression,
            prompt,
            run_once,
            created_at: now.timestamp(),
            next_run_at: next_run_at.map(|value| value.timestamp()),
            last_run_at: None,
        };
        self.jobs.insert(
            id,
            JobRuntime {
                job: job.clone(),
                schedule,
                pending_run: matches!(schedule, JobSchedule::AfterTurn),
                timer_cancel,
            },
        );
        Ok(job)
    }

    pub(crate) fn delete_job(&mut self, id: &str) -> bool {
        let Some(runtime) = self.jobs.remove(id) else {
            return false;
        };
        if let Some(cancel) = runtime.timer_cancel {
            cancel.cancel();
        }
        true
    }

    pub(crate) fn mark_after_turn_jobs_due(&mut self) {
        for runtime in self.jobs.values_mut() {
            if matches!(runtime.schedule, JobSchedule::AfterTurn) {
                runtime.pending_run = true;
            }
        }
    }

    pub(crate) fn mark_job_due(&mut self, id: &str, now: DateTime<Utc>) {
        let Some(runtime) = self.jobs.get_mut(id) else {
            return;
        };
        runtime.pending_run = true;
        runtime.job.next_run_at = runtime
            .schedule
            .next_run_at(now)
            .map(|value| value.timestamp());
    }

    pub(crate) fn claim_next_job(&mut self, now: DateTime<Utc>) -> Option<ClaimedJob> {
        let next_job_id = self
            .jobs
            .values()
            .filter(|runtime| runtime.pending_run)
            .min_by(|left, right| {
                left.job
                    .last_run_at
                    .unwrap_or(left.job.created_at)
                    .cmp(&right.job.last_run_at.unwrap_or(right.job.created_at))
                    .then_with(|| left.job.created_at.cmp(&right.job.created_at))
                    .then_with(|| left.job.id.cmp(&right.job.id))
            })
            .map(|runtime| runtime.job.id.clone())?;

        let runtime = self.jobs.remove(&next_job_id)?;
        let JobRuntime {
            mut job,
            schedule,
            pending_run: _,
            timer_cancel,
        } = runtime;
        let deleted_run_once_job = job.run_once;
        if deleted_run_once_job {
            if let Some(cancel) = timer_cancel {
                cancel.cancel();
            }
        } else {
            job.last_run_at = Some(now.timestamp());
            self.jobs.insert(
                job.id.clone(),
                JobRuntime {
                    job: job.clone(),
                    schedule,
                    pending_run: false,
                    timer_cancel,
                },
            );
        }
        Some(ClaimedJob {
            job: job.clone(),
            context: JobTurnContext {
                current_job_id: job.id,
                cron_expression: job.cron_expression,
                prompt: job.prompt,
                run_once: job.run_once,
            },
            deleted_run_once_job,
        })
    }
}

pub(crate) fn job_turn_developer_instructions(job: &JobTurnContext) -> String {
    if job.run_once {
        render_job_prompt_template(ONE_SHOT_JOB_TURN_INSTRUCTIONS, job)
    } else {
        render_job_prompt_template(RECURRING_JOB_TURN_INSTRUCTIONS, job)
    }
}

pub(crate) fn job_prompt_input_item(job: &JobTurnContext) -> ResponseInputItem {
    let text = if job.run_once {
        render_job_prompt_template(ONE_SHOT_JOB_PROMPT, job)
    } else {
        render_job_prompt_template(RECURRING_JOB_PROMPT, job)
    };
    ResponseInputItem::Message {
        role: "developer".to_string(),
        content: vec![ContentItem::InputText { text }],
    }
}

fn render_job_prompt_template(template: &str, job: &JobTurnContext) -> String {
    template
        .replace("{{CURRENT_JOB_ID}}", &job.current_job_id)
        .replace("{{SCHEDULE}}", &job.cron_expression)
        .replace("{{PROMPT}}", &job.prompt)
        .trim_end()
        .to_string()
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
    use super::JobSchedule;
    use super::JobTurnContext;
    use super::JobsState;
    use super::MAX_ACTIVE_JOBS_PER_THREAD;
    use super::MAX_EVERY_SECONDS;
    use super::job_prompt_input_item;
    use chrono::DateTime;
    use chrono::Duration as ChronoDuration;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_supported_job_schedules() {
        assert_eq!(
            JobSchedule::parse(AFTER_TURN_CRON_EXPRESSION),
            Ok(JobSchedule::AfterTurn)
        );
        assert_eq!(
            JobSchedule::parse("@every 5m"),
            Ok(JobSchedule::EverySeconds(300))
        );
        assert_eq!(
            JobSchedule::parse("@every:3600"),
            Ok(JobSchedule::EverySeconds(3600))
        );
    }

    #[test]
    fn rejects_overflowing_every_job_schedules() {
        let too_large = MAX_EVERY_SECONDS + 1;
        assert_eq!(
            JobSchedule::parse(&format!("@every:{too_large}")),
            Err(format!(
                "unsupported cron_expression `@every:{too_large}`; @every values must be between 1 and {MAX_EVERY_SECONDS} seconds"
            ))
        );
        assert_eq!(
            JobSchedule::parse(&format!("@every {too_large}s")),
            Err(format!(
                "unsupported cron_expression `@every {too_large}s`; @every values must be between 1 and {MAX_EVERY_SECONDS} seconds"
            ))
        );
    }

    #[test]
    fn create_job_rejects_every_schedule_when_next_run_at_is_out_of_range() {
        let now = DateTime::<Utc>::MAX_UTC - ChronoDuration::seconds(1);
        let mut jobs = JobsState::default();

        let result = jobs.create_job(
            "job-1".to_string(),
            "@every:2".to_string(),
            "overflow".to_string(),
            /*run_once*/ false,
            now,
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
    fn claim_run_once_job_removes_it() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut jobs = JobsState::default();
        let job = jobs
            .create_job(
                "job-1".to_string(),
                AFTER_TURN_CRON_EXPRESSION.to_string(),
                "run tests".to_string(),
                /*run_once*/ true,
                now,
                /*timer_cancel*/ None,
            )
            .expect("job should be created");
        assert_eq!(jobs.list_jobs(), vec![job]);

        let claimed = jobs.claim_next_job(now).expect("job should be claimed");
        assert_eq!(claimed.context.current_job_id, "job-1");
        assert!(claimed.deleted_run_once_job);
        assert!(jobs.list_jobs().is_empty());
    }

    #[test]
    fn claim_next_job_prefers_pending_job_that_ran_least_recently() {
        let create_first = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let create_second = Utc.timestamp_opt(101, 0).single().expect("valid timestamp");
        let first_claimed_at = Utc.timestamp_opt(110, 0).single().expect("valid timestamp");
        let second_claimed_at = Utc.timestamp_opt(111, 0).single().expect("valid timestamp");
        let mut jobs = JobsState::default();
        jobs.create_job(
            "job-1".to_string(),
            AFTER_TURN_CRON_EXPRESSION.to_string(),
            "older recurring job".to_string(),
            /*run_once*/ false,
            create_first,
            /*timer_cancel*/ None,
        )
        .expect("job should be created");
        jobs.create_job(
            "job-2".to_string(),
            AFTER_TURN_CRON_EXPRESSION.to_string(),
            "newer recurring job".to_string(),
            /*run_once*/ false,
            create_second,
            /*timer_cancel*/ None,
        )
        .expect("job should be created");

        let first = jobs
            .claim_next_job(first_claimed_at)
            .expect("first job should be claimed");
        assert_eq!(first.context.current_job_id, "job-1");

        jobs.mark_after_turn_jobs_due();

        let second = jobs
            .claim_next_job(second_claimed_at)
            .expect("second job should be claimed");
        assert_eq!(second.context.current_job_id, "job-2");
    }

    #[test]
    fn create_job_rejects_more_than_maximum_active_jobs() {
        let now = Utc.timestamp_opt(100, 0).single().expect("valid timestamp");
        let mut jobs = JobsState::default();
        for index in 0..MAX_ACTIVE_JOBS_PER_THREAD {
            jobs.create_job(
                format!("job-{index}"),
                AFTER_TURN_CRON_EXPRESSION.to_string(),
                format!("prompt-{index}"),
                /*run_once*/ false,
                now,
                /*timer_cancel*/ None,
            )
            .expect("job should be created");
        }

        let result = jobs.create_job(
            "job-overflow".to_string(),
            AFTER_TURN_CRON_EXPRESSION.to_string(),
            "overflow".to_string(),
            /*run_once*/ false,
            now,
            /*timer_cancel*/ None,
        );

        assert_eq!(
            result,
            Err(format!(
                "too many active jobs; each thread supports at most {MAX_ACTIVE_JOBS_PER_THREAD} jobs"
            ))
        );
    }

    #[test]
    fn job_prompt_input_is_hidden_developer_input() {
        let item = job_prompt_input_item(&JobTurnContext {
            current_job_id: "job-1".to_string(),
            cron_expression: "@every 10s".to_string(),
            prompt: "run tests".to_string(),
            run_once: false,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "Recurring scheduled job prompt:\nrun tests\n\nThis job should keep running on its schedule unless the user asked for a stopping condition and that condition is now satisfied.\nIf that stopping condition is satisfied, stop the job by calling JobDelete with {\"id\":\"job-1\"}.".to_string(),
                }],
            }
        );
    }

    #[test]
    fn one_shot_job_prompt_input_omits_delete_instruction() {
        let item = job_prompt_input_item(&JobTurnContext {
            current_job_id: "job-1".to_string(),
            cron_expression: "@after-turn".to_string(),
            prompt: "run tests once".to_string(),
            run_once: true,
        });
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "One-shot scheduled job prompt:\nrun tests once".to_string(),
                }],
            }
        );
    }
}
