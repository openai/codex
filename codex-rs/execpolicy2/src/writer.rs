use std::fs;
use std::io::Write;
use std::path::Path;

use thiserror::Error;

use crate::decision::Decision;

#[derive(Debug, Error)]
pub enum WritePolicyError {
    #[error("failed to create policy directory {dir}: {source}")]
    CreateDir { dir: String, source: std::io::Error },

    #[error("failed to read policy file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to write policy file {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
}

/// Append a prefix rule in Starlark form to the given policy file, creating any missing
/// parent directories or the file itself. Currently only supports writing a single
/// `prefix_rule` with a fixed decision.
pub fn append_prefix_rule(
    policy_path: &Path,
    pattern: &[String],
    decision: Decision,
) -> Result<(), WritePolicyError> {
    let parent = policy_path
        .parent()
        .ok_or_else(|| WritePolicyError::CreateDir {
            dir: policy_path.display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing parent"),
        })?;

    if let Err(source) = fs::create_dir_all(parent) {
        return Err(WritePolicyError::CreateDir {
            dir: parent.display().to_string(),
            source,
        });
    }

    let mut buf = Vec::new();
    if let Ok(existing) = fs::read_to_string(policy_path) {
        buf.push(existing);
        if !buf.last().is_some_and(|s| s.ends_with('\n')) {
            buf.push("\n".to_string());
        }
    }

    let serialized_pattern = serialize_pattern(pattern);
    let decision_str = decision.as_str();
    let line = format!("prefix_rule(pattern={serialized_pattern}, decision=\"{decision_str}\")\n");
    buf.push(line);

    let contents = buf.concat();
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(policy_path)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .map_err(|source| WritePolicyError::WriteFile {
            path: policy_path.display().to_string(),
            source,
        })
}

fn serialize_pattern(pattern: &[String]) -> String {
    let tokens: Vec<String> = pattern
        .iter()
        .map(|token| serde_json::to_string(token).unwrap_or_else(|_| "\"\"".to_string()))
        .collect();
    format!("[{}]", tokens.join(", "))
}
