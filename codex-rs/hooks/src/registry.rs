use tokio::process::Command;

use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookOutcome;
use crate::types::HookPayload;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct Hooks {
    after_agent: Vec<Hook>,
    after_tool_use: Vec<Hook>,
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
        Self {
            after_agent,
            after_tool_use: Vec::new(),
        }
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::AfterAgent { .. } => &self.after_agent,
            HookEvent::AfterToolUse { .. } => &self.after_tool_use,
        }
    }

    pub async fn dispatch(&self, hook_payload: HookPayload) {
        // TODO(gt): support interrupting program execution by returning a result here.
        for hook in self.hooks_for_event(&hook_payload.hook_event) {
            let outcome = hook.execute(&hook_payload).await;
            if matches!(outcome, HookOutcome::Stop) {
                break;
            }
        }
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
    use crate::types::HookEventAfterToolUse;
    use crate::types::HookToolInput;
    use crate::types::HookToolKind;

    const CWD: &str = "/tmp";
    const INPUT_MESSAGE: &str = "hello";

    fn hook_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
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

    fn counting_hook(calls: &Arc<AtomicUsize>, outcome: HookOutcome) -> Hook {
        let calls = Arc::clone(calls);
        Hook {
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    outcome
                })
            }),
        }
    }

    fn after_tool_use_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterToolUse {
                event: HookEventAfterToolUse {
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
            })
            .after_agent
            .is_empty()
        );
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["".to_string()]),
            })
            .after_agent
            .is_empty()
        );
        assert_eq!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["notify-send".to_string()]),
            })
            .after_agent
            .len(),
            1
        );
    }

    #[tokio::test]
    async fn dispatch_executes_hook() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![counting_hook(&calls, HookOutcome::Continue)],
            ..Hooks::default()
        };

        hooks.dispatch(hook_payload("1")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn default_hook_is_noop_and_continues() {
        let payload = hook_payload("d");
        let outcome = Hook::default().execute(&payload).await;
        assert_eq!(outcome, HookOutcome::Continue);
    }

    #[tokio::test]
    async fn dispatch_executes_multiple_hooks_for_same_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                counting_hook(&calls, HookOutcome::Continue),
                counting_hook(&calls, HookOutcome::Continue),
            ],
            ..Hooks::default()
        };

        hooks.dispatch(hook_payload("2")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dispatch_stops_when_hook_returns_stop() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                counting_hook(&calls, HookOutcome::Stop),
                counting_hook(&calls, HookOutcome::Continue),
            ],
            ..Hooks::default()
        };

        hooks.dispatch(hook_payload("3")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_executes_after_tool_use_hooks() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_tool_use: vec![counting_hook(&calls, HookOutcome::Continue)],
            ..Hooks::default()
        };

        hooks.dispatch(after_tool_use_payload("p")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn hook_executes_program_with_payload_argument_unix() -> Result<()> {
        let temp_dir = tempdir()?;
        let payload_path = temp_dir.path().join("payload.json");
        let payload_path_arg = payload_path.to_string_lossy().into_owned();
        let hook = Hook {
            func: Arc::new(move |payload: &HookPayload| {
                let payload_path_arg = payload_path_arg.clone();
                Box::pin(async move {
                    let json = to_string(payload).expect("serialize hook payload");
                    let mut command = command_from_argv(&[
                        "/bin/sh".to_string(),
                        "-c".to_string(),
                        "printf '%s' \"$2\" > \"$1\"".to_string(),
                        "sh".to_string(),
                        payload_path_arg,
                        json,
                    ])
                    .expect("build command");
                    command.status().await.expect("run hook command");
                    HookOutcome::Continue
                })
            }),
        };

        let payload = hook_payload("4");
        let expected = to_string(&payload)?;

        let hooks = Hooks {
            after_agent: vec![hook],
            ..Hooks::default()
        };
        hooks.dispatch(payload).await;

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
    async fn hook_executes_program_with_payload_argument_windows() -> Result<()> {
        let temp_dir = tempdir()?;
        let payload_path = temp_dir.path().join("payload.json");
        let payload_path_arg = payload_path.to_string_lossy().into_owned();
        let script_path = temp_dir.path().join("write_payload.ps1");
        fs::write(&script_path, "[IO.File]::WriteAllText($args[0], $args[1])")?;
        let script_path_arg = script_path.to_string_lossy().into_owned();
        let hook = Hook {
            func: Arc::new(move |payload: &HookPayload| {
                let payload_path_arg = payload_path_arg.clone();
                let script_path_arg = script_path_arg.clone();
                Box::pin(async move {
                    let json = to_string(payload).expect("serialize hook payload");
                    let mut command = command_from_argv(&[
                        "powershell.exe".to_string(),
                        "-NoLogo".to_string(),
                        "-NoProfile".to_string(),
                        "-ExecutionPolicy".to_string(),
                        "Bypass".to_string(),
                        "-File".to_string(),
                        script_path_arg,
                        payload_path_arg,
                        json,
                    ])
                    .expect("build command");
                    command.status().await.expect("run hook command");
                    HookOutcome::Continue
                })
            }),
        };

        let payload = hook_payload("4");
        let expected = to_string(&payload)?;

        let hooks = Hooks {
            after_agent: vec![hook],
            ..Hooks::default()
        };
        hooks.dispatch(payload).await;

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
