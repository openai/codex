use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde_json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AmendError {
    #[error("prefix rule requires at least one token")]
    EmptyPrefix,
    #[error("policy path has no parent: {path}")]
    MissingParent { path: PathBuf },
    #[error("failed to create policy directory {dir}: {source}")]
    CreatePolicyDir {
        dir: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to format prefix tokens: {source}")]
    SerializePrefix { source: serde_json::Error },
    #[error("network rule host cannot be empty")]
    EmptyNetworkHost,
    #[error("network rule protocol must be http or https")]
    InvalidNetworkProtocol,
    #[error("network rule decision must be allow, deny, or ask")]
    InvalidNetworkDecision,
    #[error("failed to open policy file {path}: {source}")]
    OpenPolicyFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write to policy file {path}: {source}")]
    WritePolicyFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to lock policy file {path}: {source}")]
    LockPolicyFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to seek policy file {path}: {source}")]
    SeekPolicyFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read policy file {path}: {source}")]
    ReadPolicyFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read metadata for policy file {path}: {source}")]
    PolicyMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Note this thread uses advisory file locking and performs blocking I/O, so it should be used with
/// [`tokio::task::spawn_blocking`] when called from an async context.
pub fn blocking_append_allow_prefix_rule(
    policy_path: &Path,
    prefix: &[String],
) -> Result<(), AmendError> {
    if prefix.is_empty() {
        return Err(AmendError::EmptyPrefix);
    }

    let tokens = prefix
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| AmendError::SerializePrefix { source })?;
    let pattern = format!("[{}]", tokens.join(", "));
    let rule = format!(r#"prefix_rule(pattern={pattern}, decision="allow")"#);

    let dir = policy_path
        .parent()
        .ok_or_else(|| AmendError::MissingParent {
            path: policy_path.to_path_buf(),
        })?;
    match std::fs::create_dir(dir) {
        Ok(()) => {}
        Err(ref source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(AmendError::CreatePolicyDir {
                dir: dir.to_path_buf(),
                source,
            });
        }
    }
    append_locked_line(policy_path, &rule)
}

/// Append a `network_rule(...)` line to the policy file.
pub fn blocking_append_network_rule(
    policy_path: &Path,
    host: &str,
    protocol: &str,
    decision: &str,
    justification: Option<&str>,
) -> Result<(), AmendError> {
    let host = host.trim();
    if host.is_empty() {
        return Err(AmendError::EmptyNetworkHost);
    }
    if !matches!(protocol, "http" | "https") {
        return Err(AmendError::InvalidNetworkProtocol);
    }
    if !matches!(decision, "allow" | "deny" | "ask") {
        return Err(AmendError::InvalidNetworkDecision);
    }
    let host =
        serde_json::to_string(host).map_err(|source| AmendError::SerializePrefix { source })?;
    let mut rule =
        format!(r#"network_rule(host={host}, protocol="{protocol}", decision="{decision}""#);
    if let Some(justification) = justification {
        let justification = serde_json::to_string(justification)
            .map_err(|source| AmendError::SerializePrefix { source })?;
        rule.push_str(&format!(", justification={justification}"));
    }
    rule.push(')');

    let dir = policy_path
        .parent()
        .ok_or_else(|| AmendError::MissingParent {
            path: policy_path.to_path_buf(),
        })?;
    match std::fs::create_dir(dir) {
        Ok(()) => {}
        Err(ref source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(AmendError::CreatePolicyDir {
                dir: dir.to_path_buf(),
                source,
            });
        }
    }
    append_locked_line(policy_path, &rule)
}

fn append_locked_line(policy_path: &Path, line: &str) -> Result<(), AmendError> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(policy_path)
        .map_err(|source| AmendError::OpenPolicyFile {
            path: policy_path.to_path_buf(),
            source,
        })?;
    file.lock().map_err(|source| AmendError::LockPolicyFile {
        path: policy_path.to_path_buf(),
        source,
    })?;

    let len = file
        .metadata()
        .map_err(|source| AmendError::PolicyMetadata {
            path: policy_path.to_path_buf(),
            source,
        })?
        .len();

    // Ensure file ends in a newline before appending.
    if len > 0 {
        file.seek(SeekFrom::End(-1))
            .map_err(|source| AmendError::SeekPolicyFile {
                path: policy_path.to_path_buf(),
                source,
            })?;
        let mut last = [0; 1];
        file.read_exact(&mut last)
            .map_err(|source| AmendError::ReadPolicyFile {
                path: policy_path.to_path_buf(),
                source,
            })?;

        if last[0] != b'\n' {
            file.write_all(b"\n")
                .map_err(|source| AmendError::WritePolicyFile {
                    path: policy_path.to_path_buf(),
                    source,
                })?;
        }
    }

    file.write_all(format!("{line}\n").as_bytes())
        .map_err(|source| AmendError::WritePolicyFile {
            path: policy_path.to_path_buf(),
            source,
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn appends_rule_and_creates_directories() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");

        blocking_append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents = std::fs::read_to_string(&policy_path).expect("default.rules should exist");
        assert_eq!(
            contents,
            r#"prefix_rule(pattern=["echo", "Hello, world!"], decision="allow")
"#
        );
    }

    #[test]
    fn appends_rule_without_duplicate_newline() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");
        std::fs::create_dir_all(policy_path.parent().unwrap()).expect("create policy dir");
        std::fs::write(
            &policy_path,
            r#"prefix_rule(pattern=["ls"], decision="allow")
"#,
        )
        .expect("write seed rule");

        blocking_append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents = std::fs::read_to_string(&policy_path).expect("read policy");
        assert_eq!(
            contents,
            r#"prefix_rule(pattern=["ls"], decision="allow")
prefix_rule(pattern=["echo", "Hello, world!"], decision="allow")
"#
        );
    }

    #[test]
    fn inserts_newline_when_missing_before_append() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");
        std::fs::create_dir_all(policy_path.parent().unwrap()).expect("create policy dir");
        std::fs::write(
            &policy_path,
            r#"prefix_rule(pattern=["ls"], decision="allow")"#,
        )
        .expect("write seed rule without newline");

        blocking_append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents = std::fs::read_to_string(&policy_path).expect("read policy");
        assert_eq!(
            contents,
            r#"prefix_rule(pattern=["ls"], decision="allow")
prefix_rule(pattern=["echo", "Hello, world!"], decision="allow")
"#
        );
    }

    #[test]
    fn appends_network_rule_and_creates_directories() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");

        blocking_append_network_rule(
            &policy_path,
            "api.example.com",
            "https",
            "allow",
            Some("Allow API calls"),
        )
        .expect("append network rule");

        let contents = std::fs::read_to_string(&policy_path).expect("default.rules should exist");
        assert_eq!(
            contents,
            r#"network_rule(host="api.example.com", protocol="https", decision="allow", justification="Allow API calls")
"#
        );
    }

    #[test]
    fn appends_network_rule_without_duplicate_newline() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");
        std::fs::create_dir_all(
            policy_path
                .parent()
                .expect("policy path should have parent"),
        )
        .expect("create policy dir");
        std::fs::write(
            &policy_path,
            r#"prefix_rule(pattern=["ls"], decision="allow")
"#,
        )
        .expect("write seed rule");

        blocking_append_network_rule(&policy_path, "api.example.com", "https", "allow", None)
            .expect("append network rule");

        let contents = std::fs::read_to_string(&policy_path).expect("read policy");
        assert_eq!(
            contents,
            r#"prefix_rule(pattern=["ls"], decision="allow")
network_rule(host="api.example.com", protocol="https", decision="allow")
"#
        );
    }

    #[test]
    fn rejects_invalid_network_rule_inputs() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("rules").join("default.rules");

        assert!(matches!(
            blocking_append_network_rule(&policy_path, " ", "https", "allow", None),
            Err(AmendError::EmptyNetworkHost)
        ));
        assert!(matches!(
            blocking_append_network_rule(&policy_path, "api.example.com", "socks5", "allow", None),
            Err(AmendError::InvalidNetworkProtocol)
        ));
        assert!(matches!(
            blocking_append_network_rule(&policy_path, "api.example.com", "https", "prompt", None),
            Err(AmendError::InvalidNetworkDecision)
        ));
    }
}
