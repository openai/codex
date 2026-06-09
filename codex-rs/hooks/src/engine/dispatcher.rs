use std::path::Path;

use futures::StreamExt;
use futures::stream::FuturesUnordered;

use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::HookScope;

use super::CommandShell;
use super::ConfiguredHandler;
use super::async_output::AsyncCommandRuntime;
use super::command_runner::CommandRunResult;
use super::command_runner::run_command;
use crate::events::common::matches_matcher;

#[derive(Debug)]
pub(crate) struct ParsedHandler<T> {
    pub completed: HookCompletedEvent,
    pub data: T,
    pub completion_order: usize,
}

pub(crate) fn select_handlers(
    handlers: &[ConfiguredHandler],
    event_name: HookEventName,
    matcher_input: Option<&str>,
) -> Vec<ConfiguredHandler> {
    let matcher_inputs = matcher_input.into_iter().collect::<Vec<_>>();
    select_handlers_for_matcher_inputs(handlers, event_name, &matcher_inputs)
}

pub(crate) fn select_sync_handlers(
    handlers: &[ConfiguredHandler],
    event_name: HookEventName,
    matcher_input: Option<&str>,
) -> Vec<ConfiguredHandler> {
    select_handlers(handlers, event_name, matcher_input)
        .into_iter()
        .filter(|handler| !handler.r#async)
        .collect()
}

pub(crate) fn select_handlers_for_matcher_inputs(
    handlers: &[ConfiguredHandler],
    event_name: HookEventName,
    matcher_inputs: &[&str],
) -> Vec<ConfiguredHandler> {
    // Check each configured handler once, even when several compatibility names
    // match the same regex. A hook like `apply_patch|Write|Edit` should run a
    // single time for one tool call, not once per matching alias.
    handlers
        .iter()
        .filter(|handler| handler.event_name == event_name)
        .filter(|handler| match event_name {
            HookEventName::PreToolUse
            | HookEventName::PermissionRequest
            | HookEventName::PostToolUse
            | HookEventName::SessionStart
            | HookEventName::SubagentStart
            | HookEventName::SubagentStop
            | HookEventName::PreCompact
            | HookEventName::PostCompact => {
                if matcher_inputs.is_empty() {
                    matches_matcher(handler.matcher.as_deref(), /*input*/ None)
                } else {
                    matcher_inputs
                        .iter()
                        .any(|input| matches_matcher(handler.matcher.as_deref(), Some(input)))
                }
            }
            HookEventName::UserPromptSubmit | HookEventName::Stop => true,
        })
        .cloned()
        .collect()
}

pub(crate) fn select_sync_handlers_for_matcher_inputs(
    handlers: &[ConfiguredHandler],
    event_name: HookEventName,
    matcher_inputs: &[&str],
) -> Vec<ConfiguredHandler> {
    select_handlers_for_matcher_inputs(handlers, event_name, matcher_inputs)
        .into_iter()
        .filter(|handler| !handler.r#async)
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
        source: handler.source,
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
    async_runtime: &AsyncCommandRuntime,
    handlers: Vec<ConfiguredHandler>,
    input_json: Result<String, String>,
    cwd: &Path,
    turn_id: Option<String>,
    parse: fn(&ConfiguredHandler, CommandRunResult, Option<String>) -> ParsedHandler<T>,
) -> Vec<ParsedHandler<T>> {
    let (handlers, asynchronous) = handlers
        .into_iter()
        .partition::<Vec<_>, _>(|handler| !handler.r#async);
    for handler in asynchronous {
        async_runtime.spawn_handler(
            shell.clone(),
            handler,
            input_json.clone(),
            cwd.to_path_buf(),
        );
    }

    let mut pending = FuturesUnordered::new();
    for (configured_order, handler) in handlers.into_iter().enumerate() {
        let input_json = input_json.clone();
        let turn_id = turn_id.clone();
        pending.push(async move {
            let result = match input_json {
                Ok(input_json) => run_command(shell, &handler, &input_json, cwd).await,
                Err(error) => CommandRunResult::failed(error),
            };
            (configured_order, parse(&handler, result, turn_id))
        });
    }

    let mut completed = Vec::new();
    let mut completion_order = 0;
    while let Some((configured_order, mut parsed)) = pending.next().await {
        parsed.completion_order = completion_order;
        completion_order += 1;
        completed.push((configured_order, parsed));
    }
    completed.sort_by_key(|(configured_order, _)| *configured_order);
    completed.into_iter().map(|(_, parsed)| parsed).collect()
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
        source: handler.source,
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
        HookEventName::SessionStart | HookEventName::SubagentStart => HookScope::Thread,
        HookEventName::PreToolUse
        | HookEventName::PermissionRequest
        | HookEventName::PostToolUse
        | HookEventName::PreCompact
        | HookEventName::PostCompact
        | HookEventName::UserPromptSubmit
        | HookEventName::SubagentStop
        | HookEventName::Stop => HookScope::Turn,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use codex_protocol::protocol::HookCompletedEvent;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookRunStatus;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

    use super::CommandRunResult;
    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::ParsedHandler;
    use super::completed_summary;
    use super::execute_handlers;
    use super::select_handlers;
    use super::select_handlers_for_matcher_inputs;
    use crate::engine::async_output::AsyncCommandRuntime;

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
            r#async: false,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: HookSource::User,
            display_order,
            env: std::collections::HashMap::new(),
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
    fn compact_hooks_match_trigger() {
        let handlers = vec![
            make_handler(
                HookEventName::PreCompact,
                Some("manual"),
                "echo manual",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::PreCompact,
                Some("auto"),
                "echo auto",
                /*display_order*/ 1,
            ),
        ];

        let selected = select_handlers(&handlers, HookEventName::PreCompact, Some("manual"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].display_order, 0);
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
    fn pre_tool_use_aliases_match_once_per_handler() {
        let handlers = vec![
            make_handler(
                HookEventName::PreToolUse,
                Some("^apply_patch$"),
                "echo apply_patch",
                /*display_order*/ 0,
            ),
            make_handler(
                HookEventName::PreToolUse,
                Some("^Write$"),
                "echo write",
                /*display_order*/ 1,
            ),
            make_handler(
                HookEventName::PreToolUse,
                Some("^Edit$"),
                "echo edit",
                /*display_order*/ 2,
            ),
            make_handler(
                HookEventName::PreToolUse,
                Some("apply_patch|Write|Edit"),
                "echo combined",
                /*display_order*/ 3,
            ),
        ];

        let selected = select_handlers_for_matcher_inputs(
            &handlers,
            HookEventName::PreToolUse,
            &["apply_patch", "Write", "Edit"],
        );

        assert_eq!(selected.len(), 4);
        assert_eq!(
            selected
                .iter()
                .map(|handler| handler.display_order)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3],
        );
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
    fn sync_handler_selection_preserves_order_and_excludes_async_handlers() {
        let mut handlers = vec![
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
        handlers[1].r#async = true;

        let selected = select_handlers(&handlers, HookEventName::Stop, /*matcher_input*/ None);
        let synchronous = super::select_sync_handlers(
            &handlers,
            HookEventName::Stop,
            /*matcher_input*/ None,
        );

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].command, "first");
        assert_eq!(selected[1].command, "second");
        assert_eq!(selected[2].command, "third");
        assert_eq!(
            synchronous
                .iter()
                .map(|handler| handler.command.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "third"],
        );
    }

    #[tokio::test]
    async fn input_serialization_failure_uses_sync_and_async_execution_paths() {
        let synchronous = make_handler(
            HookEventName::PreToolUse,
            Some("Bash"),
            "sync",
            /*display_order*/ 0,
        );
        let mut asynchronous = synchronous.clone();
        asynchronous.command = "async".to_string();
        asynchronous.display_order = 1;
        asynchronous.r#async = true;
        let runtime = AsyncCommandRuntime::default();
        let cwd = test_path_buf("/tmp").abs();

        let results = execute_handlers(
            &CommandShell {
                program: String::new(),
                args: Vec::new(),
            },
            &runtime,
            vec![synchronous, asynchronous],
            Err("serialize failed".to_string()),
            cwd.as_path(),
            Some("turn-1".to_string()),
            parse_failure,
        )
        .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].completed.run.status, HookRunStatus::Failed);
        assert_eq!(results[0].data, "serialize failed");
        let batch = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(batch) = runtime.prepare_batch() {
                    break batch;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("async serialization failure completion");
        let output = runtime.commit(batch);
        assert!(output.contains("Async hook failed to run: serialize failed"));
        assert!(output.contains("event=\"PreToolUse\""));
        runtime.shutdown().await;
    }

    fn parse_failure(
        handler: &ConfiguredHandler,
        run_result: CommandRunResult,
        turn_id: Option<String>,
    ) -> ParsedHandler<String> {
        let error = run_result.error.clone().expect("failed command result");
        ParsedHandler {
            completed: HookCompletedEvent {
                turn_id,
                run: completed_summary(handler, &run_result, HookRunStatus::Failed, Vec::new()),
            },
            data: error,
            completion_order: 0,
        }
    }
}
