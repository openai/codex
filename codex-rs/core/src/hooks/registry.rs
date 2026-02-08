use tokio::process::Command;

use super::config::hook_from_entry;
use super::types::Hook;
use super::types::HookEvent;
use super::types::HookOutcome;
use super::types::HookPayload;
use super::user_notification::notify_hook;
use crate::config::Config;

#[derive(Default, Clone)]
pub(crate) struct Hooks {
    after_agent: Vec<Hook>,
    pre_tool_use: Vec<Hook>,
    post_tool_use: Vec<Hook>,
    stop: Vec<Hook>,
    user_prompt_submit: Vec<Hook>,
    notification: Vec<Hook>,
}

fn get_notify_hook(config: &Config) -> Option<Hook> {
    config
        .notify
        .as_ref()
        .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
        .map(|argv| notify_hook(argv.clone()))
}

// Hooks are arbitrary, user-specified functions that are deterministically
// executed after specific events in the Codex lifecycle.
impl Hooks {
    // new creates a new Hooks instance from config.
    // For legacy compatibility, if config.notify is set, it will be added to
    // the after_agent hooks. New-style hooks from [hooks] config section are
    // appended after legacy hooks.
    pub(crate) fn new(config: &Config) -> Self {
        let hooks_config = &config.hooks;

        let mut after_agent: Vec<Hook> = get_notify_hook(config).into_iter().collect();
        after_agent.extend(hooks_config.after_agent.iter().map(hook_from_entry));

        let pre_tool_use = hooks_config
            .pre_tool_use
            .iter()
            .map(hook_from_entry)
            .collect();
        let post_tool_use = hooks_config
            .post_tool_use
            .iter()
            .map(hook_from_entry)
            .collect();
        let stop = hooks_config.stop.iter().map(hook_from_entry).collect();
        let user_prompt_submit = hooks_config
            .user_prompt_submit
            .iter()
            .map(hook_from_entry)
            .collect();
        let notification = hooks_config
            .notification
            .iter()
            .map(hook_from_entry)
            .collect();

        Self {
            after_agent,
            pre_tool_use,
            post_tool_use,
            stop,
            user_prompt_submit,
            notification,
        }
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::AfterAgent { .. } => &self.after_agent,
            HookEvent::PreToolUse { .. } => &self.pre_tool_use,
            HookEvent::PostToolUse { .. } => &self.post_tool_use,
            HookEvent::Stop { .. } => &self.stop,
            HookEvent::UserPromptSubmit { .. } => &self.user_prompt_submit,
            HookEvent::Notification { .. } => &self.notification,
        }
    }

    /// Dispatch hooks for the given event and return the aggregate outcome.
    ///
    /// - If any hook returns `Block`, dispatching stops immediately and
    ///   `Block` is returned.
    /// - If any hook returns `Modify`, the last `Modify` result wins and
    ///   is returned after all hooks run.  Note: subsequent hooks still
    ///   see the *original* payload (modifications are not carried forward
    ///   between hooks in the current implementation).
    /// - Otherwise `Proceed` is returned.
    pub(crate) async fn dispatch(&self, hook_payload: HookPayload) -> HookOutcome {
        let mut result = HookOutcome::Proceed;
        for hook in self.hooks_for_event(&hook_payload.hook_event) {
            let outcome = hook.execute(&hook_payload).await;
            match &outcome {
                HookOutcome::Block { .. } => return outcome,
                HookOutcome::Modify { .. } => {
                    result = outcome;
                }
                HookOutcome::Proceed => {}
            }
        }
        result
    }
}

pub(super) fn command_from_argv(argv: &[String]) -> Option<Command> {
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

    use crate::config::test_config;

    use super::super::types::Hook;
    use super::super::types::HookEvent;
    use super::super::types::HookEventAfterAgent;
    use super::super::types::HookOutcome;
    use super::super::types::HookPayload;
    use super::Hooks;
    use super::command_from_argv;
    use super::get_notify_hook;

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
                let outcome = outcome.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    outcome
                })
            }),
        }
    }

    fn hooks_for_after_agent(hooks: Vec<Hook>) -> Hooks {
        Hooks {
            after_agent: hooks,
            ..Default::default()
        }
    }

    fn hooks_for_pre_tool_use(hooks: Vec<Hook>) -> Hooks {
        Hooks {
            pre_tool_use: hooks,
            ..Default::default()
        }
    }

    fn hooks_for_post_tool_use(hooks: Vec<Hook>) -> Hooks {
        Hooks {
            post_tool_use: hooks,
            ..Default::default()
        }
    }

    fn hook_payload_pre_tool_use(label: &str) -> HookPayload {
        use super::super::types::HookEventPreToolUse;

        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventPreToolUse {
                    tool_name: format!("tool-{label}"),
                    tool_input: r#"{"arg": "value"}"#.to_string(),
                },
            },
        }
    }

    fn hook_payload_post_tool_use(label: &str) -> HookPayload {
        use super::super::types::HookEventPostToolUse;

        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PostToolUse {
                event: HookEventPostToolUse {
                    tool_name: format!("tool-{label}"),
                    tool_output: "success".to_string(),
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
    fn get_notify_hook_requires_program_name() {
        let mut config = test_config();

        config.notify = Some(vec![]);
        assert!(get_notify_hook(&config).is_none());

        config.notify = Some(vec!["".to_string()]);
        assert!(get_notify_hook(&config).is_none());

        config.notify = Some(vec!["notify-send".to_string()]);
        assert!(get_notify_hook(&config).is_some());
    }

    #[tokio::test]
    async fn dispatch_executes_hook() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_after_agent(vec![counting_hook(&calls, HookOutcome::Proceed)]);

        hooks.dispatch(hook_payload("1")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn default_hook_is_noop_and_proceeds() {
        let payload = hook_payload("d");
        let outcome = Hook::default().execute(&payload).await;
        assert_eq!(outcome, HookOutcome::Proceed);
    }

    #[tokio::test]
    async fn dispatch_executes_multiple_hooks_for_same_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_after_agent(vec![
            counting_hook(&calls, HookOutcome::Proceed),
            counting_hook(&calls, HookOutcome::Proceed),
        ]);

        hooks.dispatch(hook_payload("2")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dispatch_stops_when_hook_returns_block() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_after_agent(vec![
            counting_hook(&calls, HookOutcome::Block { message: None }),
            counting_hook(&calls, HookOutcome::Proceed),
        ]);

        hooks.dispatch(hook_payload("3")).await;
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
                    HookOutcome::Proceed
                })
            }),
        };

        let payload = hook_payload("4");
        let expected = to_string(&payload)?;

        let hooks = hooks_for_after_agent(vec![hook]);
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
                    let powershell = crate::powershell::try_find_powershell_executable_blocking()
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "powershell.exe".to_string());
                    let mut command = command_from_argv(&[
                        powershell,
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
                    HookOutcome::Proceed
                })
            }),
        };

        let payload = hook_payload("4");
        let expected = to_string(&payload)?;

        let hooks = hooks_for_after_agent(vec![hook]);
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

    #[tokio::test]
    async fn dispatch_pre_tool_use_hooks_for_pre_tool_use_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_pre_tool_use(vec![counting_hook(&calls, HookOutcome::Proceed)]);

        hooks.dispatch(hook_payload_pre_tool_use("1")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_post_tool_use_hooks_for_post_tool_use_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_post_tool_use(vec![counting_hook(&calls, HookOutcome::Proceed)]);

        hooks.dispatch(hook_payload_post_tool_use("1")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_does_not_fire_hooks_for_different_event_type() {
        let calls_after = Arc::new(AtomicUsize::new(0));
        let calls_pre = Arc::new(AtomicUsize::new(0));

        let hooks = Hooks {
            after_agent: vec![counting_hook(&calls_after, HookOutcome::Proceed)],
            pre_tool_use: vec![counting_hook(&calls_pre, HookOutcome::Proceed)],
            ..Default::default()
        };

        // Dispatch PreToolUse event should not fire after_agent hooks
        hooks.dispatch(hook_payload_pre_tool_use("1")).await;
        assert_eq!(calls_after.load(Ordering::SeqCst), 0);
        assert_eq!(calls_pre.load(Ordering::SeqCst), 1);

        // Dispatch AfterAgent event should not fire pre_tool_use hooks
        hooks.dispatch(hook_payload("2")).await;
        assert_eq!(calls_after.load(Ordering::SeqCst), 1);
        assert_eq!(calls_pre.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_modify_outcome_is_carried_forward() {
        let hooks = hooks_for_after_agent(vec![
            Hook {
                func: Arc::new(|_| {
                    Box::pin(async {
                        HookOutcome::Modify {
                            content: "first".to_string(),
                        }
                    })
                }),
            },
            Hook {
                func: Arc::new(|_| Box::pin(async { HookOutcome::Proceed })),
            },
            Hook {
                func: Arc::new(|_| {
                    Box::pin(async {
                        HookOutcome::Modify {
                            content: "second".to_string(),
                        }
                    })
                }),
            },
        ]);

        let outcome = hooks.dispatch(hook_payload("1")).await;
        // Last Modify wins
        assert_eq!(
            outcome,
            HookOutcome::Modify {
                content: "second".to_string()
            }
        );
    }

    #[tokio::test]
    async fn dispatch_modify_returned_after_all_hooks_run() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = hooks_for_after_agent(vec![
            Hook {
                func: Arc::new(|_| {
                    Box::pin(async {
                        HookOutcome::Modify {
                            content: "modified".to_string(),
                        }
                    })
                }),
            },
            counting_hook(&calls, HookOutcome::Proceed),
            counting_hook(&calls, HookOutcome::Proceed),
        ]);

        let outcome = hooks.dispatch(hook_payload("1")).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2); // Both subsequent hooks ran
        assert_eq!(
            outcome,
            HookOutcome::Modify {
                content: "modified".to_string()
            }
        );
    }
}
