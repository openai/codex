use std::io::IsTerminal;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_model_provider_info::AwsAuthRefreshConfig;
use tokio::process::Command;
use tokio::sync::Semaphore;

/// Shares the configured AWS reauthentication command across Bedrock providers.
#[derive(Debug)]
pub(crate) struct AwsAuthRecovery {
    config: AwsAuthRefreshConfig,
    refresh_lock: Semaphore,
    generation: AtomicU64,
}

impl AwsAuthRecovery {
    pub(crate) fn new(config: AwsAuthRefreshConfig) -> Self {
        Self {
            config,
            refresh_lock: Semaphore::new(/*permits*/ 1),
            generation: AtomicU64::new(0),
        }
    }

    pub(crate) async fn refresh(&self) -> std::io::Result<()> {
        if self.config.command != "aws" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "AWS auth refresh command must be `aws`",
            ));
        }

        let generation = self.generation.load(Ordering::Acquire);
        let _refresh_lock = self
            .refresh_lock
            .acquire()
            .await
            .map_err(std::io::Error::other)?;
        if self.generation.load(Ordering::Acquire) != generation {
            return Ok(());
        }

        let mut command = Command::new(&self.config.command);
        command
            .args(&self.config.args)
            .stdin(if std::io::stdin().is_terminal() {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            // Keep interactive prompts visible without corrupting app-server's JSON-RPC stdout.
            .stdout(std::io::stderr())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let status = tokio::time::timeout(self.config.timeout(), command.status())
            .await
            .map_err(|_| {
                std::io::Error::other(format!(
                    "AWS auth refresh command timed out after {} ms",
                    self.config.timeout_ms
                ))
            })?
            .map_err(|error| {
                std::io::Error::other(format!(
                    "failed to run AWS auth refresh command `{}`: {error}",
                    self.config.command
                ))
            })?;

        if status.success() {
            self.generation.fetch_add(1, Ordering::Release);
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "AWS auth refresh command exited with {status}"
            )))
        }
    }
}
