mod cli;
use std::sync::Arc;

pub use cli::Cli;
use codex_core::codex_wrapper;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::FileChange;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::util::is_inside_git_repo;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::EnvFilter;

pub async fn run_main(cli: Cli) -> anyhow::Result<()> {
    // TODO(mbolin): Take a more thoughtful approach to logging.
    let default_level = "error";
    let allow_ansi = true;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap(),
        )
        .with_ansi(allow_ansi)
        .with_writer(std::io::stderr)
        .try_init();

    let Cli {
        skip_git_repo_check,
        model,
        images,
        prompt,
        ..
    } = cli;

    if !skip_git_repo_check && !is_inside_git_repo() {
        eprintln!("Not inside a Git repo and --skip-git-repo-check was not specified.");
        std::process::exit(1);
    } else if images.is_empty() && prompt.is_none() {
        eprintln!("No images or prompt specified.");
        std::process::exit(1);
    }

    // TODO(mbolin): We are reworking the CLI args right now, so this will
    // likely come from a new --execution-policy arg.
    let approval_policy = AskForApproval::Never;
    let sandbox_policy = SandboxPolicy::NetworkAndFileWriteRestricted;
    let (codex_wrapper, event, ctrl_c) =
        codex_wrapper::init_codex(approval_policy, sandbox_policy, model).await?;
    let codex = Arc::new(codex_wrapper);
    info!("Codex initialized with event: {event:?}");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    {
        let codex = codex.clone();
        tokio::spawn(async move {
            loop {
                let interrupted = ctrl_c.notified();
                tokio::select! {
                    _ = interrupted => {
                        // Forward an interrupt to the codex so it can abort any in‑flight task.
                        let _ = codex
                            .submit(
                                Op::Interrupt,
                            )
                            .await;

                        // Exit the inner loop and return to the main input prompt.  The codex
                        // will emit a `TurnInterrupted` (Error) event which is drained later.
                        break;
                    }
                    res = codex.next_event() => match res {
                        Ok(event) => {
                            debug!("Received event: {event:?}");
                            process_event(&event);
                            if let Err(e) = tx.send(event) {
                                error!("Error sending event: {e:?}");
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Error receiving event: {e:?}");
                            break;
                        }
                    }
                }
            }
        });
    }

    if !images.is_empty() {
        // Send images first.
        let items: Vec<InputItem> = images
            .into_iter()
            .map(|path| InputItem::LocalImage { path })
            .collect();
        let initial_images_event_id = codex.submit(Op::UserInput { items }).await?;
        info!("Sent images with event ID: {initial_images_event_id}");
        while let Ok(event) = codex.next_event().await {
            if event.id == initial_images_event_id && matches!(event.msg, EventMsg::TaskComplete) {
                break;
            }
        }
    }

    if let Some(prompt) = prompt {
        // Send the prompt.
        let items: Vec<InputItem> = vec![InputItem::Text { text: prompt }];
        let initial_prompt_task_id = codex.submit(Op::UserInput { items }).await?;
        info!("Sent prompt with event ID: {initial_prompt_task_id}");
        while let Some(event) = rx.recv().await {
            if event.id == initial_prompt_task_id && matches!(event.msg, EventMsg::TaskComplete) {
                break;
            }
        }
    }

    Ok(())
}

fn process_event(event: &Event) {
    let Event { id, msg } = event;
    match msg {
        EventMsg::Error { message } => {
            println!("Error: {message}");
        }
        EventMsg::BackgroundEvent { .. } => {
            // Ignore these for now.
        }
        EventMsg::TaskStarted => {
            println!("Task started: {id}");
        }
        EventMsg::TaskComplete => {
            println!("Task complete: {id}");
        }
        EventMsg::AgentMessage { message } => {
            println!("Agent message: {message}");
        }
        EventMsg::ExecCommandBegin {
            call_id,
            command,
            cwd,
        } => {
            println!("exec('{call_id}'): {:?} in {cwd}", command);
        }
        EventMsg::ExecCommandEnd {
            call_id,
            stdout,
            stderr,
            exit_code,
        } => {
            let output = if *exit_code == 0 { stdout } else { stderr };
            let truncated_output = output.lines().take(5).collect::<Vec<_>>().join("\n");
            println!("exec('{call_id}') exited {exit_code}:\n{truncated_output}");
        }
        EventMsg::PatchApplyBegin {
            call_id,
            auto_approved,
            changes,
        } => {
            let changes = changes
                .iter()
                .map(|(path, change)| {
                    format!("{} {}", format_file_change(change), path.to_string_lossy())
                })
                .collect::<Vec<_>>()
                .join("\n");
            println!("apply_patch('{call_id}') auto_approved={auto_approved}:\n{changes}");
        }
        EventMsg::PatchApplyEnd {
            call_id,
            stdout,
            stderr,
            success,
        } => {
            let (exit_code, output) = if *success { (0, stdout) } else { (1, stderr) };
            let truncated_output = output.lines().take(5).collect::<Vec<_>>().join("\n");
            println!("apply_patch('{call_id}') exited {exit_code}:\n{truncated_output}");
        }
        EventMsg::ExecApprovalRequest { .. } => {
            // Should we exit?
        }
        EventMsg::ApplyPatchApprovalRequest { .. } => {
            // Should we exit?
        }
        _ => {
            // Ignore event.
        }
    }
}

fn format_file_change(change: &FileChange) -> &'static str {
    match change {
        FileChange::Add { .. } => "A",
        FileChange::Delete => "D",
        FileChange::Update {
            move_path: Some(_), ..
        } => "R",
        FileChange::Update {
            move_path: None, ..
        } => "M",
    }
}
