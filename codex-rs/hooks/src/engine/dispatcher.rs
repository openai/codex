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
use crate::events::common::matches_matcher;

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
            HookEventName::PreToolUse
            | HookEventName::PostToolUse
            | HookEventName::SessionStart => {
                matches_matcher(handler.matcher.as_deref(), matcher_input)
            }
            HookEventName::UserPromptSubmit | HookEventName::Stop => true,
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
        HookEventName::PreToolUse
        | HookEventName::PostToolUse
        | HookEventName::UserPromptSubmit
        | HookEventName::Stop => HookScope::Turn,
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::HookEventName;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

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
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            display_order,
        }
    }

    #[test]
    fn select_handlers_keeps_duplicate_stop_handlers() {
        let handlers = vec![
            make_handler(
                HookEventName::Stop,
                /*matcher*/ None,
                "echo same",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::Stop,
                /*matcher*/ None,
                "echo same",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::Stop, /*matcher_input*/ None);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn select_handlers_keeps_overlapping_session_start_matchers() {
        let handlers = vec![
            make_handler(
                HookEventName::SessionStart,
                Some("start.*"),
                "echo same",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::SessionStart,
                Some("^startup$"),
                "echo same",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::SessionStart, Some("startup"));

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn pre_tool_use_matches_tool_name() {
        let handlers = vec![
            make_handler(
                HookEventName::PreToolUse,
                Some("^Bash$"),
                "echo same",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::PreToolUse,
                Some("^Edit$"),
                "echo same",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::PreToolUse, Some("Bash"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].display_order, 0);
    }

    #[test]
    fn post_tool_use_matches_tool_name() {
        let handlers = vec![
            make_handler(
                HookEventName::PostToolUse,
                Some("^Bash$"),
                "echo same",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::PostToolUse,
                Some("^Edit$"),
                "echo same",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::PostToolUse, Some("Bash"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].display_order, 0);
    }

    #[test]
    fn pre_tool_use_star_matcher_matches_all_tools() {
        let handlers = vec![
            make_handler(
                HookEventName::PreToolUse,
                Some("*"),
                "echo same",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::PreToolUse,
                Some("^Edit$"),
                "echo same",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::PreToolUse, Some("Bash"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].display_order, 0);
    }

    #[test]
    fn pre_tool_use_regex_alternation_matches_each_tool_name() {
        let handlers = vec![make_handler(
            HookEventName::PreToolUse,
            Some("Edit|Write"),
            "echo same",
            /*display_order*/ 0,
        )];

        let selected_edit = select_handlers(&handlers, HookEventName::PreToolUse, Some("Edit"));
        let selected_write = select_handlers(&handlers, HookEventName::PreToolUse, Some("Write"));
        let selected_bash = select_handlers(&handlers, HookEventName::PreToolUse, Some("Bash"));

        assert_eq!(selected_edit.len(), 1);
        assert_eq!(selected_write.len(), 1);
        assert_eq!(selected_bash.len(), 0);
    }

    #[test]
    fn user_prompt_submit_ignores_matcher() {
        let handlers = vec![
            make_handler(
                HookEventName::UserPromptSubmit,
                Some("^hello"),
                "echo first",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::UserPromptSubmit,
                Some("["),
                "echo second",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(
            &handlers,
            HookEventName::UserPromptSubmit,
            /*matcher_input*/ None,
        );

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].display_order, 0);
        assert_eq!(selected[1].display_order, 1);
    }

    #[test]
    fn select_handlers_preserves_declaration_order() {
        let handlers = vec![
            make_handler(
                HookEventName::Stop,
                /*matcher*/ None,
                "first",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::Stop,
                /*matcher*/ None,
                "second",
                /*display_order*/ 1,
            ),
            make_handler(
                HookEventName::Stop,
                /*matcher*/ None,
                "third",
                /*display_order*/ 2,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::Stop, /*matcher_input*/ None);

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].command, "first");
        assert_eq!(selected[1].command, "second");
        assert_eq!(selected[2].command, "third");
    }
}
