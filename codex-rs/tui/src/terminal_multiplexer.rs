use crate::app_event::ForkPanePlacement;
use codex_core::terminal::Multiplexer;
use codex_core::terminal::terminal_info;
use codex_protocol::ThreadId;
use shlex::try_join;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub(crate) struct MultiplexerSpawnConfig {
    pub(crate) program: &'static str,
    pub(crate) args: Vec<String>,
    pub(crate) description: &'static str,
}

fn codex_executable() -> PathBuf {
    std::env::current_exe()
        .map(|path| resolve_codex_executable(&path))
        .unwrap_or_else(|_| PathBuf::from("codex"))
}

fn resolve_codex_executable(current_exe: &Path) -> PathBuf {
    let Some(file_name) = current_exe.file_name().and_then(|name| name.to_str()) else {
        return PathBuf::from("codex");
    };
    if !file_name.starts_with("codex-tui") {
        return current_exe.to_path_buf();
    }

    let sibling = if let Some((_, extension)) = file_name.rsplit_once('.') {
        current_exe.with_file_name(format!("codex.{extension}"))
    } else {
        current_exe.with_file_name("codex")
    };
    if sibling.is_file() {
        sibling
    } else {
        PathBuf::from("codex")
    }
}

fn resume_command_parts(
    exe: &Path,
    thread_id: &ThreadId,
    runtime_overrides_path: &Path,
) -> Vec<String> {
    vec![
        exe.display().to_string(),
        "resume".to_string(),
        thread_id.to_string(),
        "--runtime-overrides".to_string(),
        runtime_overrides_path.display().to_string(),
    ]
}

fn zellij_direction(placement: ForkPanePlacement) -> Option<&'static str> {
    match placement {
        ForkPanePlacement::Right => Some("right"),
        ForkPanePlacement::Down => Some("down"),
        _ => None,
    }
}

fn build_zellij_new_pane_args(
    resume_command: &[String],
    placement: Option<ForkPanePlacement>,
) -> Vec<String> {
    let mut args = vec![
        "action".to_string(),
        "new-pane".to_string(),
        "--close-on-exit".to_string(),
    ];
    if let Some(placement) = placement {
        if placement == ForkPanePlacement::Float {
            args.push("--floating".to_string());
        } else if let Some(direction) = zellij_direction(placement) {
            args.push("--direction".to_string());
            args.push(direction.to_string());
        } else {
            unreachable!("invalid zellij placement");
        }
    }
    args.push("--".to_string());
    args.extend(resume_command.iter().cloned());
    args
}

fn tmux_split_flags(placement: Option<ForkPanePlacement>) -> [&'static str; 2] {
    match placement {
        None | Some(ForkPanePlacement::Right) => ["-h", ""],
        Some(ForkPanePlacement::Left) => ["-h", "-b"],
        Some(ForkPanePlacement::Down) => ["-v", ""],
        Some(ForkPanePlacement::Up) => ["-v", "-b"],
        _ => unreachable!("invalid tmux placement"),
    }
}

fn build_tmux_new_pane_args(
    resume_command: &[String],
    placement: Option<ForkPanePlacement>,
) -> Vec<String> {
    let command = try_join(resume_command.iter().map(String::as_str))
        .unwrap_or_else(|_| resume_command.join(" "));
    let flags = tmux_split_flags(placement);
    let mut args = vec!["split-window".to_string(), flags[0].to_string()];
    if !flags[1].is_empty() {
        args.push(flags[1].to_string());
    }
    args.push(command);
    args
}

fn fork_spawn_config(
    multiplexer: &Multiplexer,
    exe: &Path,
    thread_id: &ThreadId,
    runtime_overrides_path: &Path,
    placement: Option<ForkPanePlacement>,
) -> MultiplexerSpawnConfig {
    let resume_command = resume_command_parts(exe, thread_id, runtime_overrides_path);
    match multiplexer {
        Multiplexer::Zellij {} => MultiplexerSpawnConfig {
            program: "zellij",
            args: build_zellij_new_pane_args(&resume_command, placement),
            description: "Zellij pane",
        },
        Multiplexer::Tmux { .. } => MultiplexerSpawnConfig {
            program: "tmux",
            args: build_tmux_new_pane_args(&resume_command, placement),
            description: "tmux pane",
        },
    }
}

const TMUX_FLOAT_UNSUPPORTED_MESSAGE: &str = "tmux does not support /fork float.";
const ZELLIJ_UNSUPPORTED_MESSAGE: &str = "Zellij only supports /fork [right|down|float].";
const FORK_PLACEMENT_REQUIRES_MULTIPLEXER_MESSAGE: &str =
    "Fork pane placement requires a terminal multiplexer.";

pub(crate) fn validate_fork_placement(placement: Option<ForkPanePlacement>) -> Result<(), String> {
    let terminal_info = terminal_info();
    let Some(multiplexer) = terminal_info.multiplexer.as_ref() else {
        return match placement {
            Some(_) => Err(FORK_PLACEMENT_REQUIRES_MULTIPLEXER_MESSAGE.to_string()),
            None => Ok(()),
        };
    };
    match multiplexer {
        Multiplexer::Zellij {} => match placement {
            None
            | Some(ForkPanePlacement::Right)
            | Some(ForkPanePlacement::Down)
            | Some(ForkPanePlacement::Float) => Ok(()),
            _ => Err(ZELLIJ_UNSUPPORTED_MESSAGE.to_string()),
        },
        Multiplexer::Tmux { .. } => match placement {
            None
            | Some(ForkPanePlacement::Left)
            | Some(ForkPanePlacement::Right)
            | Some(ForkPanePlacement::Up)
            | Some(ForkPanePlacement::Down) => Ok(()),
            _ => Err(TMUX_FLOAT_UNSUPPORTED_MESSAGE.to_string()),
        },
    }
}

pub(crate) async fn spawn_fork_in_new_pane(
    multiplexer: &Multiplexer,
    thread_id: &ThreadId,
    runtime_overrides_path: &Path,
    placement: Option<ForkPanePlacement>,
) -> Result<&'static str, String> {
    let exe = codex_executable();
    let config = fork_spawn_config(
        multiplexer,
        &exe,
        thread_id,
        runtime_overrides_path,
        placement,
    );
    let MultiplexerSpawnConfig {
        program,
        args,
        description,
    } = config;
    let status = match tokio::task::spawn_blocking(move || {
        Command::new(program).args(args).status()
    })
    .await
    {
        Ok(status) => status,
        Err(err) => {
            cleanup_runtime_overrides(runtime_overrides_path);
            return Err(format!("failed to spawn {program} pane: {err}"));
        }
    };
    match status {
        Ok(status) if status.success() => Ok(description),
        Ok(status) => {
            cleanup_runtime_overrides(runtime_overrides_path);
            Err(format!("{program} exited with status {status}"))
        }
        Err(err) => {
            cleanup_runtime_overrides(runtime_overrides_path);
            Err(format!("failed to run {program}: {err}"))
        }
    }
}

fn cleanup_runtime_overrides(path: &Path) {
    if let Err(err) = fs::remove_file(path)
        && err.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(path = %path.display(), error = %err, "failed to remove runtime overrides");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn resolve_codex_executable_rewrites_codex_tui_to_sibling_codex() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let current_exe = tmp.path().join("codex-tui");
        let sibling_codex = tmp.path().join("codex");
        std::fs::write(&sibling_codex, "").expect("write sibling codex");

        let resolved = resolve_codex_executable(&current_exe);

        assert_eq!(resolved, sibling_codex);
    }

    #[test]
    fn resolve_codex_executable_keeps_non_tui_binary() {
        let current_exe = PathBuf::from("/tmp/codex");

        let resolved = resolve_codex_executable(&current_exe);

        assert_eq!(resolved, current_exe);
    }

    #[test]
    fn resume_command_parts_include_runtime_overrides() {
        let exe = PathBuf::from("/tmp/codex");
        let thread_id = ThreadId::new();
        let runtime_overrides_path = PathBuf::from("/tmp/runtime-overrides.json");

        let command = resume_command_parts(&exe, &thread_id, &runtime_overrides_path);

        assert_eq!(
            command,
            vec![
                "/tmp/codex".to_string(),
                "resume".to_string(),
                thread_id.to_string(),
                "--runtime-overrides".to_string(),
                "/tmp/runtime-overrides.json".to_string(),
            ]
        );
    }
}
