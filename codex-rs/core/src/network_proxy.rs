use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;

pub(crate) struct ManagedNetworkProxy {
    child: Child,
}

impl ManagedNetworkProxy {
    pub(crate) async fn maybe_start(codex_home: &Path, enabled: bool) -> Result<Option<Self>> {
        if !enabled {
            return Ok(None);
        }

        let mut child = Command::new(network_proxy_binary())
            .env("CODEX_HOME", codex_home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn codex-network-proxy process")?;

        // If the proxy process exits immediately, treat startup as failed so callers can
        // log once and proceed without assuming policy enforcement is active.
        if let Ok(status_result) = timeout(Duration::from_millis(250), child.wait()).await {
            let status = status_result.context("failed to wait for codex-network-proxy")?;
            anyhow::bail!("codex-network-proxy exited early with status {status}");
        }

        Ok(Some(Self { child }))
    }

    pub(crate) async fn shutdown(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .context("failed to check codex-network-proxy state")?
            .is_some()
        {
            return Ok(());
        }

        self.child
            .start_kill()
            .context("failed to signal codex-network-proxy for shutdown")?;
        let _ = self.child.wait().await;
        Ok(())
    }
}

fn network_proxy_binary() -> String {
    std::env::var("CODEX_NETWORK_PROXY_BIN").unwrap_or_else(|_| "codex-network-proxy".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maybe_start_returns_none_when_disabled() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let proxy = ManagedNetworkProxy::maybe_start(codex_home.path(), false)
            .await
            .expect("startup should succeed");
        assert!(proxy.is_none());
    }
}
