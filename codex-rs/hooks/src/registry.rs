use std::process::Stdio;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookPayload;
use crate::types::HookResponse;
use crate::types::HookResult;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
    pub session_start_argv: Option<Vec<String>>,
    pub user_prompt_submit_argv: Option<Vec<String>>,
    pub pre_tool_use_argv: Option<Vec<String>>,
    pub post_tool_use_argv: Option<Vec<String>>,
    pub stop_argv: Option<Vec<String>>,
    pub pre_compact_argv: Option<Vec<String>>,
    pub session_end_argv: Option<Vec<String>>,
    pub subagent_start_argv: Option<Vec<String>>,
    pub subagent_stop_argv: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct Hooks {
    after_agent: Vec<Hook>,
    pre_tool_use: Vec<Hook>,
    post_tool_use: Vec<Hook>,
    session_start: Vec<Hook>,
    user_prompt_submit: Vec<Hook>,
    stop: Vec<Hook>,
    pre_compact: Vec<Hook>,
    session_end: Vec<Hook>,
    subagent_start: Vec<Hook>,
    subagent_stop: Vec<Hook>,
}

impl Default for Hooks {
    fn default() -> Self {
        Self::new(HooksConfig::default())
    }
}

// Hooks are arbitrary, user-specified functions that are deterministically
// executed after specific events in the Codex lifecycle.
impl Hooks {
    pub fn new(config: HooksConfig) -> Self {
        let after_agent = config
            .legacy_notify_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(crate::notify_hook)
            .into_iter()
            .collect();

        let session_start = config
            .session_start_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("session_start", argv))
            .into_iter()
            .collect();
        let user_prompt_submit = config
            .user_prompt_submit_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("user_prompt_submit", argv))
            .into_iter()
            .collect();
        let pre_tool_use = config
            .pre_tool_use_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("pre_tool_use", argv))
            .into_iter()
            .collect();
        let post_tool_use = config
            .post_tool_use_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("post_tool_use", argv))
            .into_iter()
            .collect();
        let stop = config
            .stop_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("stop", argv))
            .into_iter()
            .collect();
        let pre_compact = config
            .pre_compact_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("pre_compact", argv))
            .into_iter()
            .collect();
        let session_end = config
            .session_end_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("session_end", argv))
            .into_iter()
            .collect();
        let subagent_start = config
            .subagent_start_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("subagent_start", argv))
            .into_iter()
            .collect();
        let subagent_stop = config
            .subagent_stop_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(|argv| command_hook("subagent_stop", argv))
            .into_iter()
            .collect();
        Self {
            after_agent,
            pre_tool_use,
            post_tool_use,
            session_start,
            user_prompt_submit,
            stop,
            pre_compact,
            session_end,
            subagent_start,
            subagent_stop,
        }
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::AfterAgent { .. } => &self.after_agent,
            HookEvent::PreToolUse { .. } => &self.pre_tool_use,
            HookEvent::PostToolUse { .. } => &self.post_tool_use,
            HookEvent::SessionStart { .. } => &self.session_start,
            HookEvent::UserPromptSubmit { .. } => &self.user_prompt_submit,
            HookEvent::Stop { .. } => &self.stop,
            HookEvent::PreCompact { .. } => &self.pre_compact,
            HookEvent::SessionEnd { .. } => &self.session_end,
            HookEvent::SubagentStart { .. } => &self.subagent_start,
            HookEvent::SubagentStop { .. } => &self.subagent_stop,
        }
    }

    pub async fn dispatch(&self, hook_payload: HookPayload) -> Vec<HookResponse> {
        let hooks = self.hooks_for_event(&hook_payload.hook_event);
        let mut outcomes = Vec::with_capacity(hooks.len());
        for hook in hooks {
            let outcome = hook.execute(&hook_payload).await;
            let should_abort_operation = outcome.result.should_abort_operation();
            outcomes.push(outcome);
            if should_abort_operation {
                break;
            }
        }

        outcomes
    }
}

pub fn command_from_argv(argv: &[String]) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    if program.is_empty() {
        return None;
    }
    let mut command = Command::new(program);
    command.args(args);
    Some(command)
}

fn command_hook(name: &str, argv: Vec<String>) -> Hook {
    let hook_name = name.to_string();
    let argv = Arc::new(argv);
    Hook {
        name: hook_name,
        func: Arc::new(move |payload: &HookPayload| {
            let argv = Arc::clone(&argv);
            Box::pin(async move {
                let mut command = match command_from_argv(&argv) {
                    Some(command) => command,
                    None => return HookResult::Success,
                };
                let payload_json = match serde_json::to_string(payload) {
                    Ok(payload_json) => payload_json,
                    Err(err) => return HookResult::FailedContinue(err.into()),
                };
                command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(err) => return HookResult::FailedContinue(err.into()),
                };

                if let Some(mut stdin) = child.stdin.take()
                    && let Err(err) = stdin.write_all(payload_json.as_bytes()).await
                {
                    return HookResult::FailedContinue(Box::new(err));
                }

                match child.wait_with_output().await {
                    Ok(output) if output.status.success() => HookResult::Success,
                    Ok(output) => {
                        let code = output.status.code();
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let message = if stderr.is_empty() {
                            match code {
                                Some(code) => format!("hook exited with status code {code}"),
                                None => "hook terminated without a status code".to_string(),
                            }
                        } else {
                            stderr
                        };
                        if code == Some(2) && payload.hook_event.aborts_on_exit_code_two() {
                            HookResult::FailedAbort(std::io::Error::other(message).into())
                        } else {
                            HookResult::FailedContinue(std::io::Error::other(message).into())
                        }
                    }
                    Err(err) => HookResult::FailedContinue(err.into()),
                }
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use anyhow::Result;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::to_string;
    use tempfile::tempdir;
    use tokio::time::timeout;

    use super::*;
    use crate::types::HookEventAfterAgent;
    use crate::types::HookEventLifecycle;
    use crate::types::HookEventPostToolUse;
    use crate::types::HookEventPreToolUse;
    use crate::types::HookResult;
    use crate::types::HookToolInput;
    use crate::types::HookToolKind;

    const CWD: &str = "/tmp";
    const INPUT_MESSAGE: &str = "hello";

    fn hook_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            transcript_path: None,
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id: ThreadId::new(),
                    turn_id: format!("turn-{label}"),
                    input_messages: vec![INPUT_MESSAGE.to_string()],
                    last_assistant_message: Some("hi".to_string()),
                },
            },
        }
    }

    fn counting_success_hook(calls: &Arc<AtomicUsize>, name: &str) -> Hook {
        let hook_name = name.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookResult::Success
                })
            }),
        }
    }

    fn failing_continue_hook(calls: &Arc<AtomicUsize>, name: &str, message: &str) -> Hook {
        let hook_name = name.to_string();
        let message = message.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                let message = message.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookResult::FailedContinue(std::io::Error::other(message).into())
                })
            }),
        }
    }

    fn failing_abort_hook(calls: &Arc<AtomicUsize>, name: &str, message: &str) -> Hook {
        let hook_name = name.to_string();
        let message = message.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                let message = message.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookResult::FailedAbort(std::io::Error::other(message).into())
                })
            }),
        }
    }

    fn after_tool_use_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            transcript_path: None,
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PostToolUse {
                event: HookEventPostToolUse {
                    turn_id: format!("turn-{label}"),
                    call_id: format!("call-{label}"),
                    tool_name: "apply_patch".to_string(),
                    tool_kind: HookToolKind::Custom,
                    tool_input: HookToolInput::Custom {
                        input: "*** Begin Patch".to_string(),
                    },
                    executed: true,
                    success: true,
                    duration_ms: 1,
                    mutating: true,
                    sandbox: "none".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    output_preview: "ok".to_string(),
                },
            },
        }
    }

    fn lifecycle_payload(event: HookEvent) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            transcript_path: Some("/tmp/rollout.jsonl".to_string()),
            cwd: PathBuf::from(CWD),
            client: Some("codex-tui".to_string()),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: event,
        }
    }

    fn sample_lifecycle_event() -> HookEventLifecycle {
        HookEventLifecycle {
            previous_session_id: None,
            prompt: Some("hello".to_string()),
            last_assistant_message: Some("done".to_string()),
            tool_use_id: Some("toolu_123".to_string()),
            tool_input: Some(HookToolInput::Custom {
                input: "payload".to_string(),
            }),
            subagent_id: Some(ThreadId::new()),
            metadata: None,
        }
    }

    #[test]
    fn command_from_argv_returns_none_for_empty_args() {
        assert!(command_from_argv(&[]).is_none());
        assert!(command_from_argv(&["".to_string()]).is_none());
    }

    #[tokio::test]
    async fn command_from_argv_builds_command() -> Result<()> {
        let argv = if cfg!(windows) {
            vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo hello world".to_string(),
            ]
        } else {
            vec!["echo".to_string(), "hello".to_string(), "world".to_string()]
        };
        let mut command = command_from_argv(&argv).ok_or_else(|| anyhow::anyhow!("command"))?;
        let output = command.stdout(Stdio::piped()).output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim_end_matches(['\r', '\n']);
        assert_eq!(trimmed, "hello world");
        Ok(())
    }

    #[test]
    fn hooks_new_requires_program_name() {
        assert!(Hooks::new(HooksConfig::default()).after_agent.is_empty());
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec![]),
                ..HooksConfig::default()
            })
            .after_agent
            .is_empty()
        );
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["".to_string()]),
                ..HooksConfig::default()
            })
            .after_agent
            .is_empty()
        );
        assert_eq!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["notify-send".to_string()]),
                ..HooksConfig::default()
            })
            .after_agent
            .len(),
            1
        );
        assert!(
            Hooks::new(HooksConfig {
                session_start_argv: Some(vec!["".to_string()]),
                ..HooksConfig::default()
            })
            .session_start
            .is_empty()
        );
        assert_eq!(
            Hooks::new(HooksConfig {
                session_start_argv: Some(vec!["hooks-cli".to_string()]),
                ..HooksConfig::default()
            })
            .session_start
            .len(),
            1
        );
    }

    #[test]
    fn hooks_new_wires_all_lifecycle_commands() {
        let hooks = Hooks::new(HooksConfig {
            session_start_argv: Some(vec!["hooks-cli".to_string()]),
            user_prompt_submit_argv: Some(vec!["hooks-cli".to_string()]),
            pre_tool_use_argv: Some(vec!["hooks-cli".to_string()]),
            post_tool_use_argv: Some(vec!["hooks-cli".to_string()]),
            stop_argv: Some(vec!["hooks-cli".to_string()]),
            pre_compact_argv: Some(vec!["hooks-cli".to_string()]),
            session_end_argv: Some(vec!["hooks-cli".to_string()]),
            subagent_start_argv: Some(vec!["hooks-cli".to_string()]),
            subagent_stop_argv: Some(vec!["hooks-cli".to_string()]),
            ..HooksConfig::default()
        });

        assert_eq!(hooks.session_start.len(), 1);
        assert_eq!(hooks.user_prompt_submit.len(), 1);
        assert_eq!(hooks.pre_tool_use.len(), 1);
        assert_eq!(hooks.post_tool_use.len(), 1);
        assert_eq!(hooks.stop.len(), 1);
        assert_eq!(hooks.pre_compact.len(), 1);
        assert_eq!(hooks.session_end.len(), 1);
        assert_eq!(hooks.subagent_start.len(), 1);
        assert_eq!(hooks.subagent_stop.len(), 1);
    }

    #[tokio::test]
    async fn dispatch_executes_hook() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![counting_success_hook(&calls, "counting")],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("1")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "counting");
        assert!(matches!(outcomes[0].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn default_hook_is_noop_and_continues() {
        let payload = hook_payload("d");
        let outcome = Hook::default().execute(&payload).await;
        assert_eq!(outcome.hook_name, "default");
        assert!(matches!(outcome.result, HookResult::Success));
    }

    #[tokio::test]
    async fn dispatch_executes_multiple_hooks_for_same_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                counting_success_hook(&calls, "counting-1"),
                counting_success_hook(&calls, "counting-2"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("2")).await;
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].hook_name, "counting-1");
        assert_eq!(outcomes[1].hook_name, "counting-2");
        assert!(matches!(outcomes[0].result, HookResult::Success));
        assert!(matches!(outcomes[1].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dispatch_stops_when_hook_requests_abort() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                failing_abort_hook(&calls, "abort", "hook failed"),
                counting_success_hook(&calls, "counting"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("3")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "abort");
        assert!(matches!(outcomes[0].result, HookResult::FailedAbort(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_executes_after_tool_use_hooks() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            post_tool_use: vec![counting_success_hook(&calls, "counting")],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(after_tool_use_payload("p")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "counting");
        assert!(matches!(outcomes[0].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_executes_lifecycle_hooks() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            session_start: vec![counting_success_hook(&calls, "counting")],
            ..Hooks::default()
        };

        let outcomes = hooks
            .dispatch(lifecycle_payload(HookEvent::SessionStart {
                event: HookEventLifecycle {
                    previous_session_id: None,
                    prompt: None,
                    last_assistant_message: None,
                    tool_use_id: None,
                    tool_input: None,
                    subagent_id: None,
                    metadata: None,
                },
            }))
            .await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "counting");
        assert!(matches!(outcomes[0].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_routes_each_lifecycle_event_to_matching_hooks() {
        let session_start_calls = Arc::new(AtomicUsize::new(0));
        let turn_start_calls = Arc::new(AtomicUsize::new(0));
        let pre_tool_use_calls = Arc::new(AtomicUsize::new(0));
        let post_tool_use_calls = Arc::new(AtomicUsize::new(0));
        let stop_calls = Arc::new(AtomicUsize::new(0));
        let pre_compact_calls = Arc::new(AtomicUsize::new(0));
        let session_end_calls = Arc::new(AtomicUsize::new(0));
        let subagent_start_calls = Arc::new(AtomicUsize::new(0));
        let subagent_stop_calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            session_start: vec![counting_success_hook(&session_start_calls, "session_start")],
            user_prompt_submit: vec![counting_success_hook(
                &turn_start_calls,
                "user_prompt_submit",
            )],
            pre_tool_use: vec![counting_success_hook(&pre_tool_use_calls, "pre_tool_use")],
            post_tool_use: vec![counting_success_hook(&post_tool_use_calls, "post_tool_use")],
            stop: vec![counting_success_hook(&stop_calls, "stop")],
            pre_compact: vec![counting_success_hook(&pre_compact_calls, "pre_compact")],
            session_end: vec![counting_success_hook(&session_end_calls, "session_end")],
            subagent_start: vec![counting_success_hook(
                &subagent_start_calls,
                "subagent_start",
            )],
            subagent_stop: vec![counting_success_hook(&subagent_stop_calls, "subagent_stop")],
            ..Hooks::default()
        };

        let cases = vec![
            (
                "session_start",
                lifecycle_payload(HookEvent::SessionStart {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&session_start_calls),
            ),
            (
                "user_prompt_submit",
                lifecycle_payload(HookEvent::UserPromptSubmit {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&turn_start_calls),
            ),
            (
                "pre_tool_use",
                lifecycle_payload(HookEvent::PreToolUse {
                    event: HookEventPreToolUse {
                        turn_id: "turn-pre".to_string(),
                        call_id: "call-pre".to_string(),
                        tool_name: "apply_patch".to_string(),
                        tool_kind: HookToolKind::Custom,
                        tool_input: HookToolInput::Custom {
                            input: "*** Begin Patch".to_string(),
                        },
                        mutating: Some(true),
                        sandbox: Some("none".to_string()),
                        sandbox_policy: Some("danger-full-access".to_string()),
                    },
                }),
                Arc::clone(&pre_tool_use_calls),
            ),
            (
                "post_tool_use",
                after_tool_use_payload("post"),
                Arc::clone(&post_tool_use_calls),
            ),
            (
                "stop",
                lifecycle_payload(HookEvent::Stop {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&stop_calls),
            ),
            (
                "pre_compact",
                lifecycle_payload(HookEvent::PreCompact {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&pre_compact_calls),
            ),
            (
                "session_end",
                lifecycle_payload(HookEvent::SessionEnd {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&session_end_calls),
            ),
            (
                "subagent_start",
                lifecycle_payload(HookEvent::SubagentStart {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&subagent_start_calls),
            ),
            (
                "subagent_stop",
                lifecycle_payload(HookEvent::SubagentStop {
                    event: sample_lifecycle_event(),
                }),
                Arc::clone(&subagent_stop_calls),
            ),
        ];

        for (hook_name, payload, counter) in cases {
            let outcomes = hooks.dispatch(payload).await;
            assert_eq!(outcomes.len(), 1);
            assert_eq!(outcomes[0].hook_name, hook_name);
            assert!(matches!(outcomes[0].result, HookResult::Success));
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn dispatch_continues_after_continueable_failure() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                failing_continue_hook(&calls, "failing", "hook failed"),
                counting_success_hook(&calls, "counting"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("err")).await;
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].hook_name, "failing");
        assert!(matches!(outcomes[0].result, HookResult::FailedContinue(_)));
        assert_eq!(outcomes[1].hook_name, "counting");
        assert!(matches!(outcomes[1].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dispatch_returns_after_tool_use_failure_outcome() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            post_tool_use: vec![failing_continue_hook(
                &calls,
                "failing",
                "post_tool_use hook failed",
            )],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(after_tool_use_payload("err-tool")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "failing");
        assert!(matches!(outcomes[0].result, HookResult::FailedContinue(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pre_tool_use_exit_code_two_aborts_operation() {
        let hooks = Hooks::new(HooksConfig {
            pre_tool_use_argv: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "exit 2".to_string(),
            ]),
            ..HooksConfig::default()
        });

        let outcomes = hooks
            .dispatch(lifecycle_payload(HookEvent::PreToolUse {
                event: HookEventPreToolUse {
                    turn_id: "turn-pre".to_string(),
                    call_id: "call-pre".to_string(),
                    tool_name: "apply_patch".to_string(),
                    tool_kind: HookToolKind::Custom,
                    tool_input: HookToolInput::Custom {
                        input: "*** Begin Patch".to_string(),
                    },
                    mutating: Some(true),
                    sandbox: Some("none".to_string()),
                    sandbox_policy: Some("danger-full-access".to_string()),
                },
            }))
            .await;

        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::FailedAbort(_)));
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn post_tool_use_exit_code_two_continues_operation() {
        let hooks = Hooks::new(HooksConfig {
            post_tool_use_argv: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "exit 2".to_string(),
            ]),
            ..HooksConfig::default()
        });

        let outcomes = hooks
            .dispatch(after_tool_use_payload("post-exit-two"))
            .await;

        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::FailedContinue(_)));
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn hook_executes_program_with_payload_on_stdin_unix() -> Result<()> {
        let temp_dir = tempdir()?;
        let payload_path = temp_dir.path().join("payload.json");
        let payload_path_arg = payload_path.to_string_lossy().into_owned();
        let payload = lifecycle_payload(HookEvent::SessionStart {
            event: sample_lifecycle_event(),
        });
        let expected = to_string(&payload)?;

        let hooks = Hooks::new(HooksConfig {
            session_start_argv: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "cat > \"$1\"".to_string(),
                "sh".to_string(),
                payload_path_arg,
            ]),
            ..HooksConfig::default()
        });
        let outcomes = hooks.dispatch(payload).await;
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::Success));

        let contents = timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(contents) = fs::read_to_string(&payload_path)
                    && !contents.is_empty()
                {
                    return contents;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;

        assert_eq!(contents, expected);
        Ok(())
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn hook_executes_program_with_payload_on_stdin_windows() -> Result<()> {
        let temp_dir = tempdir()?;
        let payload_path = temp_dir.path().join("payload.json");
        let payload_path_arg = payload_path.to_string_lossy().into_owned();
        let script_path = temp_dir.path().join("write_payload.ps1");
        fs::write(
            &script_path,
            "$inputData = [Console]::In.ReadToEnd(); [IO.File]::WriteAllText($args[0], $inputData)",
        )?;
        let script_path_arg = script_path.to_string_lossy().into_owned();
        let payload = lifecycle_payload(HookEvent::SessionStart {
            event: sample_lifecycle_event(),
        });
        let expected = to_string(&payload)?;

        let hooks = Hooks::new(HooksConfig {
            session_start_argv: Some(vec![
                "powershell.exe".to_string(),
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                script_path_arg,
                payload_path_arg,
            ]),
            ..HooksConfig::default()
        });
        let outcomes = hooks.dispatch(payload).await;
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::Success));

        let contents = timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(contents) = fs::read_to_string(&payload_path)
                    && !contents.is_empty()
                {
                    return contents;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;

        assert_eq!(contents, expected);
        Ok(())
    }
}
