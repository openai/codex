use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::command_safety::is_dangerous_command::command_might_be_dangerous;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use thiserror::Error;
use tokio::fs;

use crate::posix::mcp_escalation_policy::ExecPolicy;
use crate::posix::mcp_escalation_policy::ExecPolicyOutcome;

const POLICY_DIR_NAME: &str = "policy";
const POLICY_EXTENSION: &str = "codexpolicy";

#[derive(Debug, Error)]
pub(crate) enum ExecPolicyError {
    #[error("failed to resolve CODEX_HOME: {source}")]
    CodexHome { source: std::io::Error },

    #[error("failed to read execpolicy files from {dir}: {source}")]
    ReadDir {
        dir: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read execpolicy file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse execpolicy file {path}: {source}")]
    ParsePolicy {
        path: String,
        source: codex_execpolicy::Error,
    },
}

pub(crate) async fn load_policy_from_codex_home() -> Result<Arc<Policy>, ExecPolicyError> {
    let codex_home = codex_core::config::find_codex_home()
        .map_err(|source| ExecPolicyError::CodexHome { source })?;

    let policy_dir = codex_home.join(POLICY_DIR_NAME);
    let policy_paths = collect_policy_files(&policy_dir).await?;

    let mut parser = PolicyParser::new();
    for policy_path in &policy_paths {
        let contents =
            fs::read_to_string(policy_path)
                .await
                .map_err(|source| ExecPolicyError::ReadFile {
                    path: policy_path.clone(),
                    source,
                })?;
        let identifier = policy_path.to_string_lossy().to_string();
        parser
            .parse(&identifier, &contents)
            .map_err(|source| ExecPolicyError::ParsePolicy {
                path: identifier,
                source,
            })?;
    }

    let policy = Arc::new(parser.build());
    tracing::debug!(
        "loaded execpolicy from {} files in {}",
        policy_paths.len(),
        policy_dir.display()
    );
    Ok(policy)
}

pub(crate) struct ExecPolicyEvaluator {
    policy: Arc<Policy>,
}

impl ExecPolicyEvaluator {
    pub(crate) fn new(policy: Arc<Policy>) -> Self {
        Self { policy }
    }

    fn command_for(file: &Path, argv: &[String]) -> Vec<String> {
        let cmd0 = argv
            .first()
            .and_then(|s| Path::new(s).file_name())
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
            .or_else(|| {
                file.file_name()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.is_empty())
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_else(|| file.display().to_string());

        let mut command = Vec::with_capacity(argv.len().max(1));
        command.push(cmd0);
        if argv.len() > 1 {
            command.extend(argv.iter().skip(1).cloned());
        }
        command
    }
}

impl ExecPolicy for ExecPolicyEvaluator {
    fn evaluate(&self, file: &Path, argv: &[String], _workdir: &Path) -> ExecPolicyOutcome {
        let command = Self::command_for(file, argv);

        match self.policy.check_multiple(std::iter::once(&command)) {
            Evaluation::Match { decision, .. } => match decision {
                Decision::Forbidden => ExecPolicyOutcome::Forbidden,
                Decision::Prompt => ExecPolicyOutcome::Prompt {
                    run_with_escalated_permissions: true,
                },
                Decision::Allow => ExecPolicyOutcome::Allow {
                    run_with_escalated_permissions: true,
                },
            },
            Evaluation::NoMatch { .. } => {
                if command_might_be_dangerous(&command) {
                    ExecPolicyOutcome::Prompt {
                        run_with_escalated_permissions: false,
                    }
                } else {
                    ExecPolicyOutcome::Allow {
                        run_with_escalated_permissions: false,
                    }
                }
            }
        }
    }
}

async fn collect_policy_files(dir: &Path) -> Result<Vec<PathBuf>, ExecPolicyError> {
    let mut read_dir = match fs::read_dir(dir).await {
        Ok(read_dir) => read_dir,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            });
        }
    };

    let mut policy_paths = Vec::new();
    while let Some(entry) =
        read_dir
            .next_entry()
            .await
            .map_err(|source| ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            })?
    {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| ExecPolicyError::ReadDir {
                dir: dir.to_path_buf(),
                source,
            })?;

        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == POLICY_EXTENSION)
            && file_type.is_file()
        {
            policy_paths.push(path);
        }
    }

    policy_paths.sort();
    Ok(policy_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn allow_policy_bypasses_sandbox() {
        let mut parser = PolicyParser::new();
        parser
            .parse(
                "test.codexpolicy",
                r#"prefix_rule(pattern=["echo"], decision="allow")"#,
            )
            .expect("parse policy");
        let evaluator = ExecPolicyEvaluator::new(Arc::new(parser.build()));

        let outcome = evaluator.evaluate(
            Path::new("/bin/echo"),
            &["echo".to_string()],
            Path::new("/"),
        );
        assert!(matches!(
            outcome,
            ExecPolicyOutcome::Allow {
                run_with_escalated_permissions: true
            }
        ));
    }

    #[test]
    fn no_match_dangerous_command_prompts() {
        let evaluator = ExecPolicyEvaluator::new(Arc::new(Policy::empty()));
        let outcome = evaluator.evaluate(
            Path::new("/bin/rm"),
            &["rm".to_string(), "-rf".to_string(), "/".to_string()],
            Path::new("/"),
        );
        assert!(matches!(
            outcome,
            ExecPolicyOutcome::Prompt {
                run_with_escalated_permissions: false
            }
        ));
    }

    #[tokio::test]
    async fn missing_policy_dir_loads_empty() {
        let dir = tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set("CODEX_HOME", dir.path().as_os_str());
        let policy = load_policy_from_codex_home().await.expect("load policy");
        assert!(matches!(
            policy.check_multiple(std::iter::once(&vec!["rm".to_string()])),
            Evaluation::NoMatch { .. }
        ));
    }
}
