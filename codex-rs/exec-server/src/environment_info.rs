use futures::FutureExt;
use futures::future::BoxFuture;

use crate::ExecServerError;
use crate::client::LazyRemoteExecServerClient;
use crate::protocol::EnvironmentInfo;
use crate::protocol::ShellInfo;
use codex_shell_command::shell_detect::DetectedShell;

/// Provides environment metadata from either a local environment or a remote exec-server.
pub(crate) trait EnvironmentInfoProvider: Send + Sync {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>>;
}

pub(crate) struct LocalEnvironmentInfoProvider;

impl EnvironmentInfoProvider for LocalEnvironmentInfoProvider {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>> {
        std::future::ready(Ok(EnvironmentInfo::local())).boxed()
    }
}

pub(crate) struct RemoteEnvironmentInfoProvider {
    client: LazyRemoteExecServerClient,
}

impl RemoteEnvironmentInfoProvider {
    pub(crate) fn new(client: LazyRemoteExecServerClient) -> Self {
        Self { client }
    }
}

impl EnvironmentInfoProvider for RemoteEnvironmentInfoProvider {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>> {
        async move { self.client.environment_info().await }.boxed()
    }
}

impl EnvironmentInfo {
    pub(crate) fn local() -> Self {
        Self {
            shell: codex_shell_command::shell_detect::default_user_shell().into(),
        }
    }
}

impl From<DetectedShell> for ShellInfo {
    fn from(shell: DetectedShell) -> Self {
        Self {
            name: shell.name().to_string(),
            path: shell.shell_path.to_string_lossy().into_owned(),
        }
    }
}
