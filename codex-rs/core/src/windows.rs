use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

#[cfg(target_os = "windows")]
const WINDOWS_HELPER_DIR: &str = "windows-sandbox-py";
#[cfg(target_os = "windows")]
const WINDOWS_HELPER_SCRIPT: &str = "windows_restricted_token_v3.py";
#[cfg(target_os = "windows")]
const PYTHON_CANDIDATES: &[&str] = &["python", "py"];
#[cfg(target_os = "windows")]
const RUST_SANDBOX_EXE_CANDIDATES: &[&str] =
    &["codex-windows-sandbox.exe", "codex-windows-sandbox"];

#[cfg(target_os = "windows")]
pub async fn spawn_command_under_windows_restricted_token(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> io::Result<Child> {
    spawn_command_under_windows_restricted_token_impl(
        command,
        command_cwd,
        sandbox_policy,
        sandbox_policy_cwd,
        stdio_policy,
        env,
    )
    .await
}

#[cfg(not(target_os = "windows"))]
pub async fn spawn_command_under_windows_restricted_token(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> io::Result<Child> {
    let _ = (
        command,
        command_cwd,
        sandbox_policy,
        sandbox_policy_cwd,
        stdio_policy,
        env,
    );
    Err(io::Error::new(
        ErrorKind::Other,
        "Windows sandbox is only supported on Windows",
    ))
}

#[cfg(target_os = "windows")]
async fn spawn_command_under_windows_restricted_token_impl(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> io::Result<Child> {
    // Prefer the Rust sandbox when explicitly requested.
    if std::env::var("CODEX_USE_RUST_WINDOWS_SANDBOX_INPROCESS")
        .ok()
        .as_deref()
        == Some("1")
    {
        if let Some(rust_sandbox_exe) = locate_rust_sandbox_exe(&command_cwd, sandbox_policy_cwd) {
            let policy_json = serde_json::to_string(sandbox_policy)
                .map_err(|err| io::Error::new(ErrorKind::Other, err))?;

            let policy_cwd_str = sandbox_policy_cwd
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(
                        ErrorKind::InvalidInput,
                        "sandbox policy cwd must be valid UTF-8",
                    )
                })?
                .to_string();

            let mut args: Vec<String> = Vec::with_capacity(5 + command.len());
            args.push("--sandbox-policy-cwd".to_string());
            args.push(policy_cwd_str);
            args.push(policy_json);
            args.push("--".to_string());
            args.extend(command);

            return spawn_child_async(
                rust_sandbox_exe,
                args,
                None,
                command_cwd,
                sandbox_policy,
                stdio_policy,
                env,
            )
            .await;
        }
    }

    let script_path = locate_helper_script(&command_cwd, sandbox_policy_cwd).ok_or_else(|| {
        io::Error::new(
            ErrorKind::NotFound,
            "Unable to locate windows-sandbox-py/windows_restricted_token.py",
        )
    })?;

    let policy_json = serde_json::to_string(sandbox_policy)
        .map_err(|err| io::Error::new(ErrorKind::Other, err))?;

    let policy_cwd_str = sandbox_policy_cwd
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidInput,
                "sandbox policy cwd must be valid UTF-8",
            )
        })?
        .to_string();

    let mut args: Vec<String> = Vec::with_capacity(5 + command.len());
    args.push(script_path.to_string_lossy().to_string());
    args.push("--sandbox-policy-cwd".to_string());
    args.push(policy_cwd_str);
    args.push(policy_json);
    args.push("--".to_string());
    args.extend(command);

    try_spawn_with_python_candidates(args, command_cwd, sandbox_policy, stdio_policy, env).await
}

#[cfg(target_os = "windows")]
fn locate_rust_sandbox_exe(command_cwd: &Path, sandbox_policy_cwd: &Path) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.push(command_cwd.to_path_buf());
    if sandbox_policy_cwd != command_cwd {
        roots.push(sandbox_policy_cwd.to_path_buf());
    }
    if let Ok(current_dir) = std::env::current_dir() {
        if !roots.iter().any(|root| root == &current_dir) {
            roots.push(current_dir);
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let parent = parent.to_path_buf();
            if !roots.iter().any(|root| root == &parent) {
                roots.push(parent);
            }
        }
    }

    for root in roots {
        if let Some(path) = search_upwards_for_rust_sandbox(root) {
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn search_upwards_for_rust_sandbox(initial: PathBuf) -> Option<PathBuf> {
    let mut dir = if initial.is_absolute() {
        initial
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(initial)
    } else {
        return None;
    };

    if let Ok(canonical) = dir.canonicalize() {
        dir = canonical;
    }

    for _ in 0..10 {
        for exe_name in RUST_SANDBOX_EXE_CANDIDATES {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}
#[cfg(target_os = "windows")]
fn locate_helper_script(command_cwd: &Path, sandbox_policy_cwd: &Path) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.push(command_cwd.to_path_buf());
    if sandbox_policy_cwd != command_cwd {
        roots.push(sandbox_policy_cwd.to_path_buf());
    }
    if let Ok(current_dir) = std::env::current_dir() {
        if !roots.iter().any(|root| root == &current_dir) {
            roots.push(current_dir);
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let parent = parent.to_path_buf();
            if !roots.iter().any(|root| root == &parent) {
                roots.push(parent);
            }
        }
    }

    for root in roots {
        if let Some(path) = search_upwards_for_helper(root) {
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn search_upwards_for_helper(initial: PathBuf) -> Option<PathBuf> {
    let mut dir = if initial.is_absolute() {
        initial
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(initial)
    } else {
        return None;
    };

    if let Ok(canonical) = dir.canonicalize() {
        dir = canonical;
    }

    for _ in 0..10 {
        let candidate = dir.join(WINDOWS_HELPER_DIR).join(WINDOWS_HELPER_SCRIPT);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

#[cfg(target_os = "windows")]
async fn try_spawn_with_python_candidates(
    args: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> io::Result<Child> {
    let mut last_not_found: Option<io::Error> = None;
    for candidate in PYTHON_CANDIDATES {
        match spawn_child_async(
            PathBuf::from(candidate),
            args.clone(),
            None,
            command_cwd.clone(),
            sandbox_policy,
            stdio_policy,
            env.clone(),
        )
        .await
        {
            Ok(child) => return Ok(child),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                last_not_found = Some(err);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_not_found.unwrap_or_else(|| {
        io::Error::new(
            ErrorKind::NotFound,
            "python executable not available (tried python, py)",
        )
    }))
}
