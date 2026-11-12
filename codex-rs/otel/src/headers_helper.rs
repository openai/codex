//! Periodically refresh collector credentials without rebuilding OTel providers.
//! One worker serves all explicit HTTP signals. Failed refreshes replace the cache
//! with an error so exports cannot silently fall back to stale or static credentials.

use crate::config::OtelHeadersHelper;
use codex_http_client::NetworkPolicy;
use http::HeaderMap;
use std::fmt;
use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::oneshot;
use tokio::sync::watch;

type HeaderResult = Result<HeaderMap, String>;

pub(crate) struct HeadersHelper {
    headers: watch::Receiver<Option<HeaderResult>>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    pub(crate) runtime: tokio::runtime::Handle,
}

impl fmt::Debug for HeadersHelper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeadersHelper").finish_non_exhaustive()
    }
}

impl HeadersHelper {
    pub(crate) fn start(
        config: &OtelHeadersHelper,
        cwd: &Path,
        policy: NetworkPolicy,
    ) -> io::Result<Arc<Self>> {
        if config.command.trim().is_empty() || config.refresh_interval.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OTel headers helper requires a command and a positive refresh interval",
            ));
        }
        // SDK exports can run on OS threads without a Tokio runtime. Keep helper
        // execution and its HTTP clients on a runtime with the same lifetime.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let handle = runtime.handle().clone();
        let (tx, rx) = watch::channel(None);
        let (stop, stopped) = oneshot::channel();
        let config = config.clone();
        let cwd = cwd.to_path_buf();
        std::thread::Builder::new()
            .name("otel-headers-helper".to_string())
            .spawn(move || {
                runtime.block_on(async move {
                    tokio::select! {
                        _ = stopped => {}
                        _ = refresh_headers(&config, &cwd, &policy, tx) => {}
                    }
                });
            })?;
        Ok(Arc::new(Self {
            headers: rx,
            stop: Mutex::new(Some(stop)),
            runtime: handle,
        }))
    }

    pub(crate) async fn headers(&self) -> HeaderResult {
        let mut headers = self.headers.clone();
        let value = headers
            .wait_for(Option::is_some)
            .await
            .map_err(|_| "OTel headers helper stopped")?;
        value.clone().ok_or("OTel headers helper has no result")?
    }

    pub(crate) fn shutdown(&self) {
        if let Some(stop) = self
            .stop
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = stop.send(());
        }
    }
}

impl Drop for HeadersHelper {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn refresh_headers(
    config: &OtelHeadersHelper,
    cwd: &Path,
    policy: &NetworkPolicy,
    updates: watch::Sender<Option<HeaderResult>>,
) {
    let mut warned = false;
    loop {
        // The script can open its own sockets, so it needs the same revocable
        // permit as an SDK that cannot enforce individual destinations.
        let result = match policy.acquire_for_unsupported_sdk() {
            Ok(permit) => permit
                .run(run_helper(&config.command, cwd))
                .await
                .unwrap_or_else(|error| Err(error.to_string())),
            Err(error) => Err(error.to_string()),
        };
        let error = result.as_ref().err().cloned();
        if updates.send(Some(result)).is_err() {
            return;
        }
        if let Some(error) = error {
            tracing::warn!(
                error,
                "OTel headers helper failed; telemetry is not being exported"
            );
            if !warned {
                let _ = writeln!(
                    io::stderr().lock(),
                    "WARNING: OTel headers helper failed; telemetry is not being exported: {error}"
                );
                warned = true;
            }
        }
        tokio::time::sleep(config.refresh_interval).await;
    }
}

async fn run_helper(command: &str, cwd: &Path) -> HeaderResult {
    #[cfg(windows)]
    let mut process = codex_http_headers_helper::shell_command(command);
    #[cfg(not(windows))]
    let mut process = {
        // Executable paths containing spaces need no shell quoting.
        let executable = cwd.join(command);
        if executable.is_file() {
            codex_utils_process::background_command(executable)
        } else {
            codex_http_headers_helper::shell_command(command)
        }
    };
    process.current_dir(cwd);
    codex_http_headers_helper::run(process)
        .await
        .map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "headers_helper_tests.rs"]
mod tests;
