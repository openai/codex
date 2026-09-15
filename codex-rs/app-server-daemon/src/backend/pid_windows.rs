//! Windows updater publication handshake. The predecessor holds the operation
//! and reservation locks until its successor initializes and acknowledges readiness.

use super::super::windows::Process;
use super::PidBackend;
use super::PidFileState;
use super::PidRecord;
use super::START_TIMEOUT;
use super::STOP_POLL_INTERVAL;
use super::process_matches_record;
use super::read_process_start_time;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use tokio::fs;
use tokio::time::sleep;

impl PidBackend {
    pub(crate) async fn wait_for_ownership(&self) -> Result<()> {
        let deadline = tokio::time::Instant::now() + START_TIMEOUT;
        loop {
            if let PidFileState::Running(record) = self.read_pid_file_state().await?
                && record.pid == std::process::id()
                && self.record_is_active(&record).await?
            {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("updater was not published by its launcher");
            }
            sleep(STOP_POLL_INTERVAL).await;
        }
    }

    pub(crate) async fn mark_ready(&self) -> Result<()> {
        fs::write(self.pid_file.with_extension("ready"), b"")
            .await
            .context("failed to acknowledge updater startup")
    }

    pub(crate) async fn replace_current_updater(&self) -> Result<()> {
        let record = PidRecord {
            pid: std::process::id(),
            process_start_time: read_process_start_time(std::process::id()).await?,
            process_identity: None,
            executable_identity: None,
        };
        self.start_inner(Some(record)).await?;
        Ok(())
    }

    pub(super) async fn finish_updater_start(
        &self,
        record: &PidRecord,
        replacement: Option<&PidRecord>,
    ) -> Result<()> {
        let temp_pid_file = self.pid_file.with_extension("pid.tmp");
        let ready = self.pid_file.with_extension("ready");
        let deadline = tokio::time::Instant::now() + START_TIMEOUT;
        let startup = async {
            loop {
                if !process_matches_record(record).await? {
                    bail!("updater exited before becoming ready");
                }
                match fs::remove_file(&ready).await {
                    Ok(()) => return Ok(()),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err.into()),
                }
                if tokio::time::Instant::now() >= deadline {
                    bail!("updater did not become ready");
                }
                sleep(STOP_POLL_INTERVAL).await;
            }
        }
        .await;
        if let Err(err) = startup {
            if let Some(process) = Process::open(record.pid)?
                && process.start_time()? == record.process_start_time
            {
                process.terminate()?;
                let deadline = tokio::time::Instant::now() + START_TIMEOUT;
                while process.is_running()? {
                    if tokio::time::Instant::now() >= deadline {
                        bail!("failed updater did not exit; ownership was not restored");
                    }
                    sleep(STOP_POLL_INTERVAL).await;
                }
            }
            if let Some(previous) = replacement {
                fs::write(&temp_pid_file, serde_json::to_vec(previous)?).await?;
                fs::rename(&temp_pid_file, &self.pid_file).await?;
            } else {
                let _ = fs::remove_file(&self.pid_file).await;
            }
            return Err(err);
        }
        Ok(())
    }
}
