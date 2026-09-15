//! Native process identities and zombie state, independent of locale and timezone.
//! Boot IDs prevent records surviving reboot from matching reused process identifiers.

#[cfg(any(target_os = "linux", target_os = "macos", test))]
use anyhow::Context;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub(super) enum ProcessIdentity {
    Linux {
        boot_id: String,
        start_ticks: u64,
    },
    // Untagged decoding must try this before the timestamp-only MacOs variant.
    MacOsUnique {
        boot_id: String,
        unique_id: u64,
        start_seconds: u64,
        start_microseconds: u64,
    },
    MacOs {
        start_seconds: u64,
        start_microseconds: u64,
    },
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl ProcessIdentity {
    pub(super) async fn matches_process(&self, pid: u32) -> Result<(bool, bool)> {
        // A different boot proves staleness even when the reused PID is inaccessible.
        if let Self::Linux { boot_id, .. } | Self::MacOsUnique { boot_id, .. } = self
            && *boot_id != read_boot_id().await?
        {
            return Ok((false, false));
        }
        #[cfg(target_os = "macos")]
        if let Self::MacOsUnique { unique_id, .. } = self
            && *unique_id != read_unique_id(pid)?
        {
            return Ok((false, false));
        }
        let (is_zombie, actual) = read_process_details(pid).await?;
        let matches = if let (
            // Timestamp-only native records predate the cross-user identity check.
            Self::MacOs {
                start_seconds,
                start_microseconds,
            },
            Self::MacOsUnique {
                start_seconds: actual_seconds,
                start_microseconds: actual_microseconds,
                ..
            },
        ) = (self, &actual)
        {
            start_seconds == actual_seconds && start_microseconds == actual_microseconds
        } else {
            *self == actual
        };
        Ok((is_zombie, matches))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl super::PidBackend {
    pub(crate) async fn promote_legacy_identity(&self) -> Result<()> {
        let _reservation = self.acquire_reservation_lock().await?;
        let super::PidFileState::Running(mut record) =
            self.read_pid_file_state_with_lock_held().await?
        else {
            return Ok(());
        };
        if matches!(
            record.process_identity,
            Some(ProcessIdentity::Linux { .. } | ProcessIdentity::MacOsUnique { .. })
        ) {
            return Ok(());
        }
        // An updater keeps its PID across exec during upgrades. Only promote a
        // legacy record while its original check still identifies the process,
        // with the native identity unchanged on both sides of that check.
        let before = read_process_details(record.pid).await?;
        if before.0 || !super::process_matches_record(&record).await? {
            return Ok(());
        }
        if read_process_details(record.pid).await? != before {
            return Ok(());
        }
        record.process_identity = Some(before.1);
        let temporary = self.pid_file.with_extension("pid.tmp");
        tokio::fs::write(&temporary, serde_json::to_vec(&record)?).await?;
        tokio::fs::rename(&temporary, &self.pid_file).await?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub(super) async fn read_process_details(pid: u32) -> Result<(bool, ProcessIdentity)> {
    let boot_id = read_boot_id().await?;
    let unique_id = read_unique_id(pid)?;
    let pid = libc::pid_t::try_from(pid).context("daemon pid is out of range")?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: the buffer has the exact size and alignment required by this flavor.
    // arg=1 includes zombies so callers can reap children that have exited.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            /*arg*/ 1,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read <= 0 {
        return Err(std::io::Error::last_os_error())
            .context("failed to read daemon process identity");
    }
    anyhow::ensure!(read == size, "incomplete daemon process identity");
    // SAFETY: proc_pidinfo initialized the entire structure on success.
    let info = unsafe { info.assume_init() };
    anyhow::ensure!(
        unique_id == read_unique_id(info.pbi_pid)?,
        "daemon process changed while reading its identity"
    );
    Ok((
        info.pbi_status == libc::SZOMB,
        ProcessIdentity::MacOsUnique {
            boot_id,
            unique_id,
            start_seconds: info.pbi_start_tvsec,
            start_microseconds: info.pbi_start_tvusec,
        },
    ))
}

#[cfg(target_os = "macos")]
fn read_unique_id(pid: u32) -> Result<u64> {
    // XNU's proc_uniqidentifierinfo ABI (flavor 17), unchanged in size since 2013.
    // Unlike PROC_PIDTBSDINFO, this query does not require matching process ownership.
    #[repr(C)]
    struct UniqueInfo {
        executable_uuid: [u8; 16],
        unique_id: u64,
        parent_unique_id: u64,
        reserved: [u64; 3],
    }
    let pid = libc::pid_t::try_from(pid).context("daemon pid is out of range")?;
    let mut info = std::mem::MaybeUninit::<UniqueInfo>::uninit();
    let size = std::mem::size_of::<UniqueInfo>() as libc::c_int;
    // SAFETY: the buffer matches proc_uniqidentifierinfo's size and alignment.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            /*flavor*/ 17,
            /*arg*/ 1,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read <= 0 {
        return Err(std::io::Error::last_os_error()).context("failed to read daemon unique ID");
    }
    anyhow::ensure!(read == size, "incomplete daemon unique ID");
    // SAFETY: proc_pidinfo initialized the entire structure on success.
    Ok(unsafe { info.assume_init() }.unique_id)
}

#[cfg(target_os = "macos")]
async fn read_boot_id() -> Result<String> {
    let mut buffer = [0u8; 37];
    let mut size = buffer.len();
    // SAFETY: the writable buffer has size bytes; the new-value pointer is null.
    let result = unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            /*newlen*/ 0,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error()).context("failed to read macOS boot ID");
    }
    anyhow::ensure!(size == buffer.len(), "incomplete macOS boot ID");
    Ok(std::ffi::CStr::from_bytes_with_nul(&buffer)?
        .to_str()?
        .to_owned())
}

#[cfg(target_os = "linux")]
pub(super) async fn read_process_details(pid: u32) -> Result<(bool, ProcessIdentity)> {
    let stat = tokio::fs::read(format!("/proc/{pid}/stat"))
        .await
        .with_context(|| format!("failed to read process stat for pid-managed process {pid}"))?;
    let (state, start_ticks) = parse_stat(&stat)?;
    Ok((
        state == "Z",
        ProcessIdentity::Linux {
            boot_id: read_boot_id().await?,
            start_ticks,
        },
    ))
}

#[cfg(target_os = "linux")]
async fn read_boot_id() -> Result<String> {
    let boot_id = tokio::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .await
        .context("failed to read Linux boot ID")?;
    let boot_id = boot_id.trim();
    anyhow::ensure!(!boot_id.is_empty(), "Linux boot ID is empty");
    Ok(boot_id.to_owned())
}

#[cfg(any(target_os = "linux", test))]
fn parse_stat(stat: &[u8]) -> Result<(String, u64)> {
    // comm (field 2) can contain spaces and closing parentheses. The final ')'
    // terminates it; splitting the whole line on whitespace miscounts fields.
    let end = stat
        .iter()
        .rposition(|byte| *byte == b')')
        .context("process stat has no comm")?;
    let fields = std::str::from_utf8(&stat[end + 1..]).context("invalid process stat fields")?;
    let mut fields = fields.split_whitespace();
    let state = fields.next().context("process stat has no state")?;
    let start_ticks = fields
        .nth(/*n*/ 18)
        .context("process stat has no start time")?
        .parse()
        .context("process stat start time is invalid")?;
    Ok((state.to_string(), start_ticks))
}

#[cfg(test)]
#[path = "pid_identity_tests.rs"]
mod tests;
