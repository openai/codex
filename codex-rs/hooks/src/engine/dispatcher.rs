use std::path::Path;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use serde::Serialize;

use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::HookScope;

use super::ClaudeHooksEngine;
use super::ConfiguredHandler;
use super::command_runner::CommandRunResult;
use super::command_runner::run_command;
use crate::events::common::matches_matcher;

#[derive(Debug)]
pub(crate) struct ParsedHandler<T> {
    pub completed: HookCompletedEvent,
    pub data: T,
    pub completion_order: usize,
}

fn select_handlers_for_matcher_inputs(
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

fn running_summary(handler: &ConfiguredHandler) -> HookRunSummary {
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

impl ClaudeHooksEngine {
    pub(crate) fn preview_commands(
        &self,
        event_name: HookEventName,
        matcher_inputs: &[&str],
    ) -> Vec<HookRunSummary> {
        select_handlers_for_matcher_inputs(&self.handlers, event_name, matcher_inputs)
            .into_iter()
            .filter(|handler| !handler.r#async)
            .map(|handler| running_summary(&handler))
            .collect()
    }

    pub(crate) async fn execute_commands<T, I>(
        &self,
        event_name: HookEventName,
        matcher_inputs: &[&str],
        input: &I,
        cwd: &Path,
        turn_id: Option<String>,
        parse: fn(&ConfiguredHandler, CommandRunResult, Option<String>) -> ParsedHandler<T>,
    ) -> Vec<ParsedHandler<T>>
    where
        I: Serialize + ?Sized,
    {
        let handlers =
            select_handlers_for_matcher_inputs(&self.handlers, event_name, matcher_inputs);
        if handlers.is_empty() {
            return Vec::new();
        }

        let input_label = match event_name {
            HookEventName::PreToolUse => "pre tool use",
            HookEventName::PermissionRequest => "permission request",
            HookEventName::PostToolUse => "post tool use",
            HookEventName::PreCompact => "pre compact",
            HookEventName::PostCompact => "post compact",
            HookEventName::SessionStart => "session start",
            HookEventName::UserPromptSubmit => "user prompt submit",
            HookEventName::SubagentStart => "subagent start",
            HookEventName::SubagentStop => "subagent stop",
            HookEventName::Stop => "stop",
        };
        let input_json = serde_json::to_string(input)
            .map_err(|error| format!("failed to serialize {input_label} hook input: {error}"));
        let (handlers, asynchronous) = handlers
            .into_iter()
            .partition::<Vec<_>, _>(|handler| !handler.r#async);
        for handler in asynchronous {
            self.async_runtime.spawn_handler(
                self.shell.clone(),
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
                    Ok(input_json) => run_command(&self.shell, &handler, &input_json, cwd).await,
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
    use serde::Serialize;
    use serde::Serializer;
    use serde::ser::Error as _;

    use super::ClaudeHooksEngine;
    use super::CommandRunResult;
    use super::ConfiguredHandler;
    use super::ParsedHandler;
    use super::completed_summary;
    use super::select_handlers_for_matcher_inputs;
    use crate::engine::CommandShell;
    use crate::engine::async_output::AsyncCommandRuntime;
    use crate::output_spill::HookOutputSpiller;

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

    fn engine_with_handlers(
        handlers: Vec<ConfiguredHandler>,
        async_runtime: AsyncCommandRuntime,
    ) -> ClaudeHooksEngine {
        ClaudeHooksEngine {
            handlers,
            warnings: Vec::new(),
            shell: CommandShell {
                program: String::new(),
                args: Vec::new(),
            },
            async_runtime,
            output_spiller: HookOutputSpiller::new(),
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

        let selected = select_handlers_for_matcher_inputs(&handlers, HookEventName::Stop, &[]);

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

        let selected = select_handlers_for_matcher_inputs(
            &handlers,
            HookEventName::SessionStart,
            &["startup"],
        );

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

        let selected =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreCompact, &["manual"]);

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

        let selected =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreToolUse, &["Bash"]);

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

        let selected =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PostToolUse, &["Bash"]);

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

        let selected =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreToolUse, &["Bash"]);

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

        let selected_edit =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreToolUse, &["Edit"]);
        let selected_write =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreToolUse, &["Write"]);
        let selected_bash =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::PreToolUse, &["Bash"]);

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

        let selected =
            select_handlers_for_matcher_inputs(&handlers, HookEventName::UserPromptSubmit, &[]);

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

        let selected = select_handlers_for_matcher_inputs(&handlers, HookEventName::Stop, &[]);
        let engine = engine_with_handlers(handlers, AsyncCommandRuntime::default());
        let synchronous = engine.preview_commands(HookEventName::Stop, &[]);

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].command, "first");
        assert_eq!(selected[1].command, "second");
        assert_eq!(selected[2].command, "third");
        assert_eq!(
            synchronous
                .iter()
                .map(|run| run.display_order)
                .collect::<Vec<_>>(),
            vec![0, 2],
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
        let engine = engine_with_handlers(vec![synchronous, asynchronous], runtime.clone());
        let cwd = test_path_buf("/tmp").abs();

        let results = engine
            .execute_commands(
                HookEventName::PreToolUse,
                &["Bash"],
                &SerializationFailure,
                cwd.as_path(),
                Some("turn-1".to_string()),
                parse_failure,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            results[0].data,
            "failed to serialize pre tool use hook input: serialize failed"
        );
        let output = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let boundary = runtime.ready_boundary();
                if let Some(output) = runtime.flush_through(boundary) {
                    break output;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("async serialization failure completion");
        assert!(output.contains(
            "Async hook failed to run: failed to serialize pre tool use hook input: serialize failed"
        ));
        assert!(output.contains("event=\"PreToolUse\""));
        runtime.shutdown().await;
    }

    struct SerializationFailure;

    impl Serialize for SerializationFailure {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("serialize failed"))
        }
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
