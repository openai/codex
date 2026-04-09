pub(crate) fn build_timer_list_prompt() -> String {
    wrap_synthetic_user_message(
        "/loop",
        "List the thread timers that are currently scheduled. Call the TimerList tool directly, then summarize the pending timers briefly for the user. If there are no pending timers, say that none are scheduled.",
    )
}

pub(crate) fn build_loop_timer_prompt(spec: &str) -> Result<String, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("Use `/loop <spec>` to create a timer.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let timezone = chrono::Local::now().offset().to_string();
    let prompt = format!(
        r#"Create a Codex thread timer from this `/loop` request. Call the TimerCreate tool directly; do not only describe the timer.

Current local datetime: {now}
Local UTC offset: {timezone}

/loop request:
{spec}

Interpretation rules:
- Extract the timer prompt by removing the scheduling phrase while preserving the task the user wants to run later.
- Use delivery "after-turn" unless the user clearly asks for same-turn/current-turn steering; then use "steer-current-turn".
- Treat `/loop` as recurring by default when there is no explicit one-time timing. A bare absolute date/time is a single run; do not infer recurrence solely from the `/loop` command name.
- For "now", "immediately", or specs with no explicit timing, use a delay trigger with seconds 0 and repeat true. This makes the timer run whenever the thread is idle.
- For relative timing like "in 30 seconds", use a delay trigger with the relative seconds and repeat true unless the user clearly asked for one-shot behavior.
- For interval timing like "every 5 minutes", use a delay trigger with the interval seconds and repeat true.
- For absolute wall-clock timing like "at 9pm", "tomorrow at 8am", or "at 10:57", use a one-shot schedule trigger with dtstart set to the next matching local datetime in YYYY-MM-DDTHH:MM:SS and no rrule unless the user explicitly asks for recurrence with words like "every", "daily", "weekly", "hourly", "each", "repeat", or "recurring".
- For ambiguous wall-clock times without AM/PM, choose the soonest future local occurrence.
- For recurring calendar timing, use a schedule trigger with rrule set to an RFC 5545 RRULE string and dtstart set when the user supplies a start datetime; otherwise omit dtstart.
- For schedule triggers, use floating local wall-clock datetimes without timezone suffixes.
- After TimerCreate succeeds, briefly confirm the schedule and the timer prompt."#
    );
    Ok(wrap_synthetic_user_message(
        &format!("/loop {spec}"),
        &prompt,
    ))
}

fn wrap_synthetic_user_message(display: &str, prompt: &str) -> String {
    format!(
        r#"<codex_tui_synthetic_user_message>
<display>{}</display>
<prompt>
{prompt}
</prompt>
</codex_tui_synthetic_user_message>"#,
        xml_escape(display)
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_loop_timer_prompt_asks_model_to_call_timer_create() {
        let prompt = build_loop_timer_prompt("every 5 minutes run tests")
            .expect("valid /loop prompt should build");

        assert!(prompt.contains("Call the TimerCreate tool directly"));
        assert!(prompt.contains("every 5 minutes run tests"));
        assert!(prompt.contains("Current local datetime:"));
        assert!(prompt.contains("<display>/loop every 5 minutes run tests</display>"));
    }

    #[test]
    fn build_timer_list_prompt_asks_model_to_call_timer_list() {
        let prompt = build_timer_list_prompt();

        assert!(prompt.contains("Call the TimerList tool directly"));
        assert!(prompt.contains("<display>/loop</display>"));
    }
}
