use std::fs::OpenOptions;
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
    #[error("failed to format prefix token {token}: {source}")]
    SerializeToken {
        token: String,
        source: serde_json::Error,
    },
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

    let tokens: Vec<String> = prefix
        .iter()
        .map(|token| {
            serde_json::to_string(token).map_err(|source| AmendError::SerializeToken {
                token: token.clone(),
                source,
            })
        })
        .collect::<Result<_, _>>()?;
    let pattern = tokens.join(", ");
    let rule = format!("prefix_rule(pattern=[{pattern}], decision=\"allow\")\n");

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
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(policy_path)
        .map_err(|source| AmendError::OpenPolicyFile {
            path: policy_path.to_path_buf(),
            source,
        })?;
    let needs_newline = file
        .metadata()
        .map(|metadata| metadata.len() > 0)
        .map_err(|source| AmendError::PolicyMetadata {
            path: policy_path.to_path_buf(),
            source,
        })?;
    let final_rule = if needs_newline {
        format!("\n{rule}")
    } else {
        rule
    };

    file.write_all(final_rule.as_bytes())
        .map_err(|source| AmendError::WritePolicyFile {
            path: policy_path.to_path_buf(),
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
            "prefix_rule(pattern=[\"echo\", \"Hello, world!\"], decision=\"allow\")\n"
        );
    }

    #[test]
    fn separates_rules_with_newlines_when_appending() {
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
            "prefix_rule(pattern=[\"ls\"], decision=\"allow\")\n\nprefix_rule(pattern=[\"echo\", \"Hello, world!\"], decision=\"allow\")\n"
        );
    }
}
