use std::path::Path;

use futures::future::join_all;

use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::HookScope;

use super::CommandShell;
use super::ConfiguredHandler;
use super::command_runner::CommandRunResult;
use super::command_runner::run_command;

#[derive(Debug)]
pub(crate) struct ParsedHandler<T> {
    pub completed: HookCompletedEvent,
    pub data: T,
}

pub(crate) fn select_handlers(
    handlers: &[ConfiguredHandler],
    event_name: HookEventName,
    matcher_input: Option<&str>,
) -> Vec<ConfiguredHandler> {
    handlers
        .iter()
        .filter(|handler| handler.event_name == event_name)
        .filter(|handler| match event_name {
            HookEventName::SessionStart | HookEventName::UserPromptSubmit => {
                match (&handler.matcher, matcher_input) {
                    (Some(matcher), Some(input)) => regex::Regex::new(matcher)
                        .map(|regex| regex.is_match(input))
                        .unwrap_or(false),
                    (None, _) => true,
                    _ => false,
                }
            }
            HookEventName::Stop => true,
        })
        .cloned()
        .collect()
}

pub(crate) fn running_summary(handler: &ConfiguredHandler) -> HookRunSummary {
    HookRunSummary {
        id: handler.run_id(),
        event_name: handler.event_name,
        handler_type: HookHandlerType::Command,
        execution_mode: HookExecutionMode::Sync,
        scope: scope_for_event(handler.event_name),
        source_path: handler.source_path.clone(),
        display_order: handler.display_order,
        status: HookRunStatus::Running,
        status_message: handler.status_message.clone(),
        started_at: chrono::Utc::now().timestamp(),
        completed_at: None,
        duration_ms: None,
        entries: Vec::new(),
    }
}

pub(crate) async fn execute_handlers<T>(
    shell: &CommandShell,
    handlers: Vec<ConfiguredHandler>,
    input_json: String,
    cwd: &Path,
    turn_id: Option<String>,
    parse: fn(&ConfiguredHandler, CommandRunResult, Option<String>) -> ParsedHandler<T>,
) -> Vec<ParsedHandler<T>> {
    let results = join_all(
        handlers
            .iter()
            .map(|handler| run_command(shell, handler, &input_json, cwd)),
    )
    .await;

    handlers
        .into_iter()
        .zip(results)
        .map(|(handler, result)| parse(&handler, result, turn_id.clone()))
        .collect()
}

pub(crate) fn completed_summary(
    handler: &ConfiguredHandler,
    run_result: &CommandRunResult,
    status: HookRunStatus,
    entries: Vec<codex_protocol::protocol::HookOutputEntry>,
) -> HookRunSummary {
    HookRunSummary {
        id: handler.run_id(),
        event_name: handler.event_name,
        handler_type: HookHandlerType::Command,
        execution_mode: HookExecutionMode::Sync,
        scope: scope_for_event(handler.event_name),
        source_path: handler.source_path.clone(),
        display_order: handler.display_order,
        status,
        status_message: handler.status_message.clone(),
        started_at: run_result.started_at,
        completed_at: Some(run_result.completed_at),
        duration_ms: Some(run_result.duration_ms),
        entries,
    }
}

fn scope_for_event(event_name: HookEventName) -> HookScope {
    match event_name {
        HookEventName::SessionStart => HookScope::Thread,
        HookEventName::UserPromptSubmit | HookEventName::Stop => HookScope::Turn,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::protocol::HookEventName;

    use super::ConfiguredHandler;
    use super::select_handlers;

    fn make_handler(
        event_name: HookEventName,
        matcher: Option<&str>,
        command: &str,
        display_order: i64,
    ) -> ConfiguredHandler {
        ConfiguredHandler {
            event_name,
            matcher: matcher.map(str::to_owned),
            command: command.to_string(),
            timeout_sec: 5,
            status_message: None,
            source_path: PathBuf::from("/tmp/hooks.json"),
            display_order,
        }
    }

    #[test]
    fn select_handlers_keeps_duplicate_stop_handlers() {
        let handlers = vec![
            make_handler(HookEventName::Stop, None, "echo same", 0),
            make_handler(HookEventName::Stop, None, "echo same", 1),
        ];

        let selected = select_handlers(&handlers, HookEventName::Stop, None);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn select_handlers_keeps_overlapping_session_start_matchers() {
        let handlers = vec![
            make_handler(HookEventName::SessionStart, Some("start.*"), "echo same", 0),
            make_handler(
                HookEventName::SessionStart,
                Some("^startup$"),
                "echo same",
                1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::SessionStart, Some("startup"));

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn user_prompt_submit_matcher_uses_prompt_text() {
        let handlers = vec![
            make_handler(
                HookEventName::UserPromptSubmit,
                Some("^hello"),
                "echo first",
                0,
            ),
            make_handler(
                HookEventName::UserPromptSubmit,
                Some("world$"),
                "echo second",
                1,
            ),
        ];

        let selected = select_handlers(
            &handlers,
            HookEventName::UserPromptSubmit,
            Some("hello world"),
        );

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn select_handlers_preserves_declaration_order() {
        let handlers = vec![
            make_handler(HookEventName::Stop, None, "first", 0),
            make_handler(HookEventName::Stop, None, "second", 1),
            make_handler(HookEventName::Stop, None, "third", 2),
        ];

        let selected = select_handlers(&handlers, HookEventName::Stop, None);

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].command, "first");
        assert_eq!(selected[1].command, "second");
        assert_eq!(selected[2].command, "third");
    }
}
