use std::io::ErrorKind;
use std::path::Path;

use crate::error::CodexErr;
use crate::rollout::SESSIONS_SUBDIR;

pub(crate) fn map_session_init_error(err: &anyhow::Error, codex_home: &Path) -> CodexErr {
    if let Some(mapped) = err
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .find_map(|io_err| map_rollout_io_error(io_err, codex_home))
    {
        return mapped;
    }

    CodexErr::Fatal(format!("Failed to initialize session: {err:#}"))
}

fn map_rollout_io_error(io_err: &std::io::Error, codex_home: &Path) -> Option<CodexErr> {
    let sessions_dir = codex_home.join(SESSIONS_SUBDIR);
    let hint = match io_err.kind() {
        ErrorKind::PermissionDenied => format!(
            "Codex cannot access session files at {} (permission denied). If sessions were created using sudo, fix ownership: sudo chown -R $(whoami) {}",
            sessions_dir.display(),
            codex_home.display()
        ),
        ErrorKind::NotFound => format!(
            "Session storage missing at {}. Create the directory or choose a different Codex home.",
            sessions_dir.display()
        ),
        ErrorKind::AlreadyExists => format!(
            "Session storage path {} is blocked by an existing file. Remove or rename it so Codex can create sessions.",
            sessions_dir.display()
        ),
        ErrorKind::InvalidData | ErrorKind::InvalidInput => format!(
            "Session data under {} looks corrupt or unreadable. Clearing the sessions directory may help (this will remove saved conversations).",
            sessions_dir.display()
        ),
        ErrorKind::IsADirectory | ErrorKind::NotADirectory => format!(
            "Session storage path {} has an unexpected type. Ensure it is a directory Codex can use for session files.",
            sessions_dir.display()
        ),
        _ => return None,
    };

    Some(CodexErr::Fatal(format!(
        "{hint} (underlying error: {io_err})"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollout::SESSIONS_SUBDIR;
    use std::path::PathBuf;

    fn io_error(kind: ErrorKind, msg: &str) -> anyhow::Error {
        anyhow::Error::new(std::io::Error::new(kind, msg))
    }

    #[test]
    fn startup_errors_propagate_context() {
        let err = anyhow::anyhow!("rollout directory missing");
        let codex_home = PathBuf::from("/tmp/codex-home");
        let mapped = map_session_init_error(&err, &codex_home);

        match mapped {
            CodexErr::Fatal(msg) => {
                assert!(
                    msg.contains("Failed to initialize session"),
                    "expected startup prefix: {msg}"
                );
                assert!(
                    msg.contains("rollout directory missing"),
                    "expected underlying cause: {msg}"
                );
            }
            other => panic!("expected fatal error mapping, got {other:?}"),
        }
    }

    #[test]
    fn permission_denied_rollout_errors_surface_actionable_message() {
        let err = io_error(ErrorKind::PermissionDenied, "no access");
        let codex_home = PathBuf::from("/tmp/codex-home");
        let mapped = map_session_init_error(&err, &codex_home);
        let sessions_dir = codex_home.join(SESSIONS_SUBDIR);

        match mapped {
            CodexErr::Fatal(msg) => {
                let msg_lower = msg.to_lowercase();
                assert!(
                    msg.contains(&sessions_dir.display().to_string()),
                    "expected session path in message: {msg}"
                );
                assert!(
                    msg_lower.contains("permission denied"),
                    "expected permission hint in message: {msg}"
                );
                assert!(
                    msg.contains(&codex_home.display().to_string()),
                    "expected codex home in message: {msg}"
                );
            }
            other => panic!("expected fatal error mapping, got {other:?}"),
        }
    }

    #[test]
    fn not_found_rollout_errors_explain_missing_sessions_dir() {
        let err = io_error(ErrorKind::NotFound, "missing dir");
        let codex_home = PathBuf::from("/tmp/codex-home");
        let mapped = map_session_init_error(&err, &codex_home);
        let sessions_dir = codex_home.join(SESSIONS_SUBDIR);

        match mapped {
            CodexErr::Fatal(msg) => {
                let msg_lower = msg.to_lowercase();
                assert!(
                    msg.contains(&sessions_dir.display().to_string()),
                    "expected session path in message: {msg}"
                );
                assert!(
                    msg_lower.contains("missing") || msg_lower.contains("not found"),
                    "expected not-found hint in message: {msg}"
                );
                assert!(
                    msg.contains("missing dir"),
                    "expected underlying cause in message: {msg}"
                );
            }
            other => panic!("expected fatal error mapping, got {other:?}"),
        }
    }

    #[test]
    fn invalid_rollout_files_surface_corruption_hint() {
        let err = io_error(ErrorKind::InvalidData, "bad json");
        let codex_home = PathBuf::from("/tmp/codex-home");
        let mapped = map_session_init_error(&err, &codex_home);
        let sessions_dir = codex_home.join(SESSIONS_SUBDIR);

        match mapped {
            CodexErr::Fatal(msg) => {
                let msg_lower = msg.to_lowercase();
                assert!(
                    msg_lower.contains("corrupt") || msg_lower.contains("invalid"),
                    "expected corruption hint in message: {msg}"
                );
                assert!(
                    msg.contains("bad json"),
                    "expected underlying cause in message: {msg}"
                );
                assert!(
                    msg.contains(&sessions_dir.display().to_string()),
                    "expected session path in message: {msg}"
                );
            }
            other => panic!("expected fatal error mapping, got {other:?}"),
        }
    }
}
