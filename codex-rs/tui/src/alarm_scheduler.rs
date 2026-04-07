use crate::AppServerTarget;
use crate::start_app_server_for_picker;
use chrono::Datelike;
use chrono::NaiveDateTime;
use chrono::Timelike;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AlarmDelivery;
use codex_app_server_protocol::AlarmTrigger;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus;
use codex_core::config::Config;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;

const LOCAL_DATE_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ParsedAlarmSpec {
    pub(crate) trigger: AlarmTrigger,
    pub(crate) prompt: String,
    pub(crate) delivery: AlarmDelivery,
}

pub(crate) async fn parse_alarm_spec(
    config: Config,
    target: AppServerTarget,
    spec: String,
) -> std::result::Result<ParsedAlarmSpec, String> {
    if spec.trim().is_empty() {
        return Err("Could not determine a prompt from /loop input.".to_string());
    }
    let user_prompt = build_user_prompt(&spec);
    let mut app_server = start_app_server_for_picker(&config, &target)
        .await
        .map_err(|err| format!("failed to start alarm spec parser: {err}"))?;
    let started = app_server
        .start_ephemeral_thread_with_base_instructions(
            &config,
            PARSE_ALARM_SYSTEM_PROMPT.to_string(),
        )
        .await
        .map_err(|err| format!("failed to start alarm spec parser thread: {err}"))?;
    let thread_id = started.session.thread_id;
    let thread_id_string = thread_id.to_string();
    let response = app_server
        .turn_start(
            thread_id,
            vec![UserInput::Text {
                text: user_prompt,
                text_elements: Vec::new(),
            }],
            started.session.cwd,
            started.session.approval_policy,
            started.session.approvals_reviewer,
            started.session.sandbox_policy,
            started.session.model,
            started.session.reasoning_effort,
            config.model_reasoning_summary,
            Some(config.service_tier),
            /*collaboration_mode*/ None,
            config.personality,
            Some(output_schema()),
        )
        .await
        .map_err(|err| format!("failed to parse alarm spec with model: {err}"))?;
    let turn_id = response.turn.id;
    let result = wait_for_parser_response(&mut app_server, thread_id_string, turn_id).await?;
    let parsed: ParsedAlarmSpec = serde_json::from_str(&result)
        .map_err(|err| format!("model returned invalid alarm parse output: {err}"))?;
    validate_parsed_alarm_spec(parsed)
}

pub(crate) fn format_alarm_summary(
    trigger: &AlarmTrigger,
    delivery: AlarmDelivery,
    prompt: &str,
) -> String {
    let mode = if trigger_is_recurring(trigger) {
        "recurring"
    } else {
        "one-shot"
    };
    format!(
        "{} ({mode}, {}) -> {prompt}",
        format_alarm_trigger(trigger),
        delivery_str(delivery)
    )
}

pub(crate) fn format_alarm_trigger(trigger: &AlarmTrigger) -> String {
    match trigger {
        AlarmTrigger::Delay { seconds, repeat } => {
            let suffix = if repeat.unwrap_or(false) {
                ", repeat"
            } else {
                ""
            };
            format!("delay {seconds}s{suffix}")
        }
        AlarmTrigger::Schedule { dtstart, rrule } => match (dtstart, rrule) {
            (Some(dtstart), Some(rrule)) => format_alarm_rrule(rrule, Some(dtstart))
                .unwrap_or_else(|| {
                    format!("schedule from {}; {rrule}", format_alarm_dtstart(dtstart))
                }),
            (Some(dtstart), None) => format!("at {}", format_alarm_dtstart(dtstart)),
            (None, Some(rrule)) => {
                format_alarm_rrule(rrule, /*dtstart*/ None)
                    .unwrap_or_else(|| format!("schedule {rrule}"))
            }
            (None, None) => "invalid schedule".to_string(),
        },
    }
}

fn format_alarm_dtstart(dtstart: &str) -> String {
    let Ok(dtstart) = NaiveDateTime::parse_from_str(dtstart, LOCAL_DATE_TIME_FORMAT) else {
        return dtstart.to_string();
    };
    let month_name = MONTH_NAMES
        .get(dtstart.month0() as usize)
        .copied()
        .unwrap_or("???");
    format!(
        "{} {}, {} {}",
        month_name,
        dtstart.day(),
        dtstart.year(),
        format_alarm_time(dtstart)
    )
}

fn format_alarm_time(dtstart: NaiveDateTime) -> String {
    let hour = dtstart.hour();
    let period = if hour < 12 { "AM" } else { "PM" };
    let hour = match hour % 12 {
        0 => 12,
        hour => hour,
    };
    if dtstart.second() == 0 {
        format!("{hour}:{:02} {period}", dtstart.minute())
    } else {
        format!(
            "{hour}:{:02}:{:02} {period}",
            dtstart.minute(),
            dtstart.second()
        )
    }
}

fn format_alarm_rrule(rrule: &str, dtstart: Option<&str>) -> Option<String> {
    let freq = rrule_part(rrule, "FREQ")?;
    let interval = rrule_part(rrule, "INTERVAL")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    let byday = rrule_part(rrule, "BYDAY");
    let byhour = rrule_part(rrule, "BYHOUR").and_then(|value| value.parse::<u32>().ok());
    let byminute = rrule_part(rrule, "BYMINUTE")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let bysecond = rrule_part(rrule, "BYSECOND")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let time = format_alarm_rrule_time(byhour, byminute, bysecond, dtstart);

    match freq {
        "HOURLY" => {
            let cadence = if interval == 1 {
                "hourly".to_string()
            } else {
                format!("every {interval} hours")
            };
            if byminute == 0 && bysecond == 0 {
                Some(format!("{cadence} on the hour"))
            } else {
                Some(format!("{cadence} at :{byminute:02}"))
            }
        }
        "DAILY" => {
            let cadence = if interval == 1 {
                "daily".to_string()
            } else {
                format!("every {interval} days")
            };
            Some(match time {
                Some(time) => format!("{cadence} at {time}"),
                None => cadence,
            })
        }
        "WEEKLY" => {
            let cadence = byday.and_then(format_alarm_rrule_days).unwrap_or_else(|| {
                if interval == 1 {
                    "weekly".to_string()
                } else {
                    format!("every {interval} weeks")
                }
            });
            Some(match time {
                Some(time) => format!("{cadence} at {time}"),
                None => cadence,
            })
        }
        _ => None,
    }
}

fn rrule_part<'a>(rrule: &'a str, key: &str) -> Option<&'a str> {
    rrule.split(';').find_map(|part| {
        part.split_once('=').and_then(|(part_key, value)| {
            (part_key.eq_ignore_ascii_case(key) && !value.is_empty()).then_some(value)
        })
    })
}

fn format_alarm_rrule_days(byday: &str) -> Option<String> {
    if byday == "MO,TU,WE,TH,FR" {
        return Some("weekdays".to_string());
    }
    if byday == "SA,SU" {
        return Some("weekends".to_string());
    }
    let days = byday
        .split(',')
        .filter_map(format_alarm_rrule_day)
        .collect::<Vec<_>>()
        .join(", ");
    (!days.is_empty()).then_some(days)
}

fn format_alarm_rrule_day(day: &str) -> Option<&'static str> {
    match day {
        "MO" => Some("Mondays"),
        "TU" => Some("Tuesdays"),
        "WE" => Some("Wednesdays"),
        "TH" => Some("Thursdays"),
        "FR" => Some("Fridays"),
        "SA" => Some("Saturdays"),
        "SU" => Some("Sundays"),
        _ => None,
    }
}

fn format_alarm_rrule_time(
    byhour: Option<u32>,
    byminute: u32,
    bysecond: u32,
    dtstart: Option<&str>,
) -> Option<String> {
    let dtstart = byhour.is_none().then_some(dtstart).flatten();
    let hour = match byhour {
        Some(hour) => hour,
        None => {
            let dtstart = dtstart?;
            let dtstart = NaiveDateTime::parse_from_str(dtstart, LOCAL_DATE_TIME_FORMAT).ok()?;
            dtstart.hour()
        }
    };
    if hour > 23 || byminute > 59 || bysecond > 59 {
        return None;
    }
    let dtstart = NaiveDateTime::parse_from_str(
        &format!("2000-01-01T{hour:02}:{byminute:02}:{bysecond:02}"),
        LOCAL_DATE_TIME_FORMAT,
    )
    .ok()?;
    Some(format_alarm_time(dtstart))
}

pub(crate) fn trigger_is_recurring(trigger: &AlarmTrigger) -> bool {
    match trigger {
        AlarmTrigger::Delay { repeat, .. } => repeat.unwrap_or(false),
        AlarmTrigger::Schedule { rrule, .. } => {
            rrule.as_ref().is_some_and(|rrule| !rrule.is_empty())
        }
    }
}

fn delivery_str(delivery: AlarmDelivery) -> &'static str {
    match delivery {
        AlarmDelivery::AfterTurn => "after-turn",
        AlarmDelivery::SteerCurrentTurn => "steer-current-turn",
    }
}

fn build_user_prompt(spec: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let timezone = chrono::Local::now().offset().to_string();
    format!("Current local datetime: {now}\nTimezone: {timezone}\nAlarm spec: {spec}")
}

async fn wait_for_parser_response(
    app_server: &mut crate::app_server_session::AppServerSession,
    thread_id: String,
    turn_id: String,
) -> std::result::Result<String, String> {
    let mut last_agent_message = None;
    loop {
        let event =
            tokio::time::timeout(Duration::from_secs(/*secs*/ 120), app_server.next_event())
                .await
                .map_err(|_| "timed out while waiting for alarm spec parser".to_string())?
                .ok_or_else(|| {
                    "alarm spec parser disconnected before returning output".to_string()
                })?;
        match event {
            AppServerEvent::ServerNotification(ServerNotification::ItemCompleted(notification))
                if notification.thread_id == thread_id && notification.turn_id == turn_id =>
            {
                if let Some(text) = thread_item_agent_text(&notification.item) {
                    last_agent_message = Some(text);
                }
            }
            AppServerEvent::ServerNotification(ServerNotification::RawResponseItemCompleted(
                notification,
            )) if notification.thread_id == thread_id && notification.turn_id == turn_id => {
                if let Some(text) = response_item_agent_text(&notification.item) {
                    last_agent_message = Some(text);
                }
            }
            AppServerEvent::ServerNotification(ServerNotification::TurnCompleted(notification))
                if notification.thread_id == thread_id && notification.turn.id == turn_id =>
            {
                if matches!(notification.turn.status, TurnStatus::Failed)
                    && let Some(error) = notification.turn.error
                {
                    return Err(format!("alarm spec parser failed: {}", error.message));
                }
                return last_agent_message.ok_or_else(|| {
                    "alarm spec parser did not return an agent message".to_string()
                });
            }
            AppServerEvent::ServerNotification(_) | AppServerEvent::Lagged { .. } => {}
            AppServerEvent::ServerRequest(_) => {
                return Err("alarm spec parser unexpectedly requested user input".to_string());
            }
            AppServerEvent::Disconnected { message } => {
                return Err(format!("alarm spec parser disconnected: {message}"));
            }
        }
    }
}

fn thread_item_agent_text(item: &ThreadItem) -> Option<String> {
    match item {
        ThreadItem::AgentMessage { text, .. } if !text.trim().is_empty() => Some(text.clone()),
        ThreadItem::AgentMessage { .. }
        | ThreadItem::UserMessage { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::McpToolCall { .. }
        | ThreadItem::WebSearch { .. }
        | ThreadItem::DynamicToolCall { .. }
        | ThreadItem::CommandExecution { .. }
        | ThreadItem::FileChange { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::ImageGeneration { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => None,
    }
}

fn response_item_agent_text(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { role, content, .. } if role == "assistant" => {
            let text = content
                .iter()
                .filter_map(|content| match content {
                    ContentItem::OutputText { text } => Some(text.as_str()),
                    ContentItem::InputText { .. } | ContentItem::InputImage { .. } => None,
                })
                .collect::<String>();
            (!text.trim().is_empty()).then_some(text)
        }
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::Other => None,
    }
}

fn validate_parsed_alarm_spec(
    parsed: ParsedAlarmSpec,
) -> std::result::Result<ParsedAlarmSpec, String> {
    if parsed.prompt.trim().is_empty() {
        return Err("model did not return an alarm prompt".to_string());
    }
    Ok(ParsedAlarmSpec {
        prompt: parsed.prompt.trim().to_string(),
        ..parsed
    })
}

fn output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "trigger": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["delay", "schedule"] },
                    "seconds": { "type": ["integer", "null"], "minimum": 0 },
                    "repeat": { "type": ["boolean", "null"] },
                    "dtstart": { "type": ["string", "null"] },
                    "rrule": { "type": ["string", "null"] }
                },
                "required": ["kind", "seconds", "repeat", "dtstart", "rrule"],
                "additionalProperties": false
            },
            "prompt": { "type": "string" },
            "delivery": { "type": "string", "enum": ["after-turn", "steer-current-turn"] }
        },
        "required": ["trigger", "prompt", "delivery"],
        "additionalProperties": false
    })
}

const PARSE_ALARM_SYSTEM_PROMPT: &str = r#"Parse Codex `/loop` alarm specs into a structured alarm definition.

Return only the JSON object requested by the response schema.

Rules:
- Extract the alarm prompt by removing the scheduling phrase but preserving the user's requested task.
- Use delivery "after-turn" unless the user clearly asks for same-turn/current-turn steering; then use "steer-current-turn".
- Treat `/loop` as recurring by default when there is no explicit one-time timing. A bare absolute date/time is a single run; do not infer recurrence solely from the `/loop` command name.
- For "now", "immediately", or specs with no explicit timing, use { "kind": "delay", "seconds": 0, "repeat": true } unless the user clearly asked for one-shot behavior. This means the alarm fires whenever the thread is idle.
- For delay triggers, set dtstart and rrule to null.
- For schedule triggers, set seconds and repeat to null.
- For relative timing like "in 30 seconds", use a delay trigger with seconds set to the relative delay and repeat true unless the user clearly asked for one-shot behavior.
- For interval timing like "every 5 minutes", use a delay trigger with seconds set to the interval and repeat true.
- For absolute wall-clock timing like "at 9pm", "tomorrow at 8am", or "at 10:57", use a one-shot schedule trigger with dtstart set to the next matching local datetime in YYYY-MM-DDTHH:MM:SS and rrule null unless the user explicitly asks for recurrence with words like "every", "daily", "weekly", "hourly", "each", "repeat", or "recurring".
- For ambiguous wall-clock times without AM/PM, choose the soonest future local occurrence. Example: if the current local datetime is 2026-04-06T23:28:00 and the spec says "at 11:30", return 2026-04-06T23:30:00, not 2026-04-07T11:30:00.
- For recurring calendar timing, use a schedule trigger with rrule set to an RFC 5545 RRULE string and dtstart set when the user supplies a start datetime; otherwise null.
- For schedule triggers, use floating local wall-clock datetimes without timezone suffixes.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn format_alarm_summary_renders_delay() {
        assert_eq!(
            format_alarm_summary(
                &AlarmTrigger::Delay {
                    seconds: 30,
                    repeat: Some(false),
                },
                AlarmDelivery::AfterTurn,
                "remind me to take a break",
            ),
            "delay 30s (one-shot, after-turn) -> remind me to take a break"
        );
    }

    #[test]
    fn format_alarm_trigger_renders_one_shot_schedule_as_local_time() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: Some("2026-04-07T21:00:00".to_string()),
                rrule: None,
            }),
            "at Apr 7, 2026 9:00 PM"
        );
    }

    #[test]
    fn format_alarm_trigger_preserves_invalid_schedule_dtstart() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: Some("not-a-date".to_string()),
                rrule: None,
            }),
            "at not-a-date"
        );
    }

    #[test]
    fn format_alarm_trigger_renders_daily_rrule_as_local_time() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: Some("2026-04-07T21:00:00".to_string()),
                rrule: Some("FREQ=DAILY;BYHOUR=21;BYMINUTE=0;BYSECOND=0".to_string()),
            }),
            "daily at 9:00 PM"
        );
    }

    #[test]
    fn format_alarm_trigger_renders_weekday_rrule_as_local_time() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: Some("2026-04-07T17:00:00".to_string()),
                rrule: Some(
                    "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=17;BYMINUTE=0;BYSECOND=0".to_string(),
                ),
            }),
            "weekdays at 5:00 PM"
        );
    }

    #[test]
    fn format_alarm_trigger_renders_hourly_rrule_as_text() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: None,
                rrule: Some("FREQ=HOURLY;BYMINUTE=0;BYSECOND=0".to_string()),
            }),
            "hourly on the hour"
        );
    }

    #[test]
    fn format_alarm_trigger_preserves_unrecognized_rrule() {
        assert_eq!(
            format_alarm_trigger(&AlarmTrigger::Schedule {
                dtstart: Some("2026-04-07T21:00:00".to_string()),
                rrule: Some("FREQ=YEARLY;BYMONTH=4".to_string()),
            }),
            "schedule from Apr 7, 2026 9:00 PM; FREQ=YEARLY;BYMONTH=4"
        );
    }

    #[test]
    fn parser_output_schema_avoids_unsupported_union_keywords() {
        let schema = output_schema();
        assert_eq!(schema.pointer("/properties/trigger/oneOf"), None);
        assert_eq!(schema.pointer("/properties/trigger/anyOf"), None);
        assert_eq!(
            schema.pointer("/properties/trigger/properties/kind/enum"),
            Some(&json!(["delay", "schedule"]))
        );
    }

    #[test]
    fn parser_prompt_defaults_ambiguous_loop_to_idle_recurring() {
        assert!(
            PARSE_ALARM_SYSTEM_PROMPT
                .contains(r#"{ "kind": "delay", "seconds": 0, "repeat": true }"#)
        );
        assert!(PARSE_ALARM_SYSTEM_PROMPT.contains("A bare absolute date/time is a single run"));
        assert!(PARSE_ALARM_SYSTEM_PROMPT.contains("For absolute wall-clock timing like"));
        assert!(PARSE_ALARM_SYSTEM_PROMPT.contains("choose the soonest future local occurrence"));
        assert!(PARSE_ALARM_SYSTEM_PROMPT.contains("2026-04-06T23:30:00"));
    }

    #[test]
    fn parsed_alarm_spec_accepts_permissive_delay_trigger_shape() {
        let parsed: ParsedAlarmSpec = serde_json::from_value(json!({
            "trigger": {
                "kind": "delay",
                "seconds": 10,
                "repeat": false,
                "dtstart": null,
                "rrule": null
            },
            "prompt": "tell me a joke",
            "delivery": "after-turn"
        }))
        .expect("permissive parser schema output should deserialize");

        assert_eq!(
            parsed,
            ParsedAlarmSpec {
                trigger: AlarmTrigger::Delay {
                    seconds: 10,
                    repeat: Some(false),
                },
                prompt: "tell me a joke".to_string(),
                delivery: AlarmDelivery::AfterTurn,
            }
        );
    }
}
