//! Saves a bounded previous-launch stderr tail before a detached launch truncates its log.
//! Preservation is best effort; missing or empty logs leave the last useful tail intact.

use std::io;
use std::path::Path;
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;

const MAX_PREVIOUS_LOG_BYTES: u64 = 256 * 1024;

pub(super) async fn preserve(path: &Path) {
    let started = Instant::now();
    let result = async {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) if !metadata.is_file() => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let mut file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let metadata = file.metadata().await?;
        let len = metadata.len();
        if !metadata.is_file() || len == 0 {
            return Ok(());
        }
        file.seek(io::SeekFrom::Start(
            len.saturating_sub(MAX_PREVIOUS_LOG_BYTES),
        ))
        .await?;
        let mut bytes = Vec::new();
        file.take(MAX_PREVIOUS_LOG_BYTES)
            .read_to_end(&mut bytes)
            .await?;
        let previous = path.with_extension("log.previous");
        let temporary = path.with_extension("log.previous.tmp");
        let result = async {
            tokio::fs::write(&temporary, bytes).await?;
            tokio::fs::rename(&temporary, previous).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result?;
        anyhow::Ok(())
    }
    .await;
    if let Err(error) = result {
        let _ = crate::diagnostics::result::<()>("stderr_preservation", started, Err(error));
    }
}

#[cfg(test)]
#[path = "stderr_log_tests.rs"]
mod tests;
