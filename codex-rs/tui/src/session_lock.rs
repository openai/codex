//! Observe the local session lock for one explicit realtime call, including its transport retries.
//!
//! Setup takes an initial snapshot before audio starts. Dropping the returned future unregisters
//! observation; there are no detached tasks or threads. Polling requires the TUI runtime to run.

use std::future::Future;
use std::pin::Pin;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
use macos::is_locked;
#[cfg(windows)]
use windows::is_locked;

pub(crate) type Activation = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

pub(crate) async fn observe() -> anyhow::Result<Activation> {
    #[cfg(target_os = "linux")]
    return linux::observe().await;

    #[cfg(any(target_os = "macos", windows))]
    {
        let initially_locked = is_locked()?;
        Ok(Box::pin(async move {
            if initially_locked {
                return Ok(());
            }
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                if is_locked()? {
                    return Ok(());
                }
            }
        }))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    Ok(Box::pin(std::future::pending()))
}
