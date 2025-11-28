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
    #[error("failed to unlock policy file {path}: {source}")]
    UnlockPolicyFile {
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

pub fn append_allow_prefix_rule(policy_path: &Path, prefix: &[String]) -> Result<(), AmendError> {
    if prefix.is_empty() {
        return Err(AmendError::EmptyPrefix);
    }

    let pattern =
        serde_json::to_string(prefix).map_err(|source| AmendError::SerializePrefix { source })?;
    let rule = format!("prefix_rule(pattern={pattern}, decision=\"allow\")");

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
    let policy_path = policy_path.to_path_buf();
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&policy_path)
        .map_err(|source| AmendError::OpenPolicyFile {
            path: policy_path.clone(),
            source,
        })?;
    file.lock().map_err(|source| AmendError::LockPolicyFile {
        path: policy_path.clone(),
        source,
    })?;

    let len = file
        .metadata()
        .map_err(|source| AmendError::PolicyMetadata {
            path: policy_path.clone(),
            source,
        })?
        .len();

    if len > 0 {
        file.seek(SeekFrom::End(-1))
            .map_err(|source| AmendError::SeekPolicyFile {
                path: policy_path.clone(),
                source,
            })?;
        let mut last = [0; 1];
        file.read_exact(&mut last)
            .map_err(|source| AmendError::ReadPolicyFile {
                path: policy_path.clone(),
                source,
            })?;

        if last[0] != b'\n' {
            file.write_all(b"\n")
                .map_err(|source| AmendError::WritePolicyFile {
                    path: policy_path.clone(),
                    source,
                })?;
        }
    }

    file.write_all(line.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|source| AmendError::WritePolicyFile {
            path: policy_path.clone(),
            source,
        })?;

    file.unlock()
        .map_err(|source| AmendError::UnlockPolicyFile {
            path: policy_path,
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn appends_rule_and_creates_directories() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("policy").join("default.codexpolicy");

        append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents =
            std::fs::read_to_string(&policy_path).expect("default.codexpolicy should exist");
        assert_eq!(
            contents,
            "prefix_rule(pattern=[\"echo\",\"Hello, world!\"], decision=\"allow\")\n"
        );
    }

    #[test]
    fn appends_rule_without_duplicate_newline() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("policy").join("default.codexpolicy");
        std::fs::create_dir_all(policy_path.parent().unwrap()).expect("create policy dir");
        std::fs::write(
            &policy_path,
            "prefix_rule(pattern=[\"ls\"], decision=\"allow\")\n",
        )
        .expect("write seed rule");

        append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents = std::fs::read_to_string(&policy_path).expect("read policy");
        assert_eq!(
            contents,
            "prefix_rule(pattern=[\"ls\"], decision=\"allow\")\nprefix_rule(pattern=[\"echo\",\"Hello, world!\"], decision=\"allow\")\n"
        );
    }

    #[test]
    fn inserts_newline_when_missing_before_append() {
        let tmp = tempdir().expect("create temp dir");
        let policy_path = tmp.path().join("policy").join("default.codexpolicy");
        std::fs::create_dir_all(policy_path.parent().unwrap()).expect("create policy dir");
        std::fs::write(
            &policy_path,
            "prefix_rule(pattern=[\"ls\"], decision=\"allow\")",
        )
        .expect("write seed rule without newline");

        append_allow_prefix_rule(
            &policy_path,
            &[String::from("echo"), String::from("Hello, world!")],
        )
        .expect("append rule");

        let contents = std::fs::read_to_string(&policy_path).expect("read policy");
        assert_eq!(
            contents,
            "prefix_rule(pattern=[\"ls\"], decision=\"allow\")\nprefix_rule(pattern=[\"echo\",\"Hello, world!\"], decision=\"allow\")\n"
        );
    }
}
