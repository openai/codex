//! Watches one missing-path observation per physical executor connection. Changes,
//! supersession and transport loss invalidate the observation without a clock deadline.

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::BTreeSet;
use std::future::Future;
use std::io;
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use crate::protocol::FS_MISSING_CHANGED_METHOD;
use crate::protocol::FsMissingChangedNotification;
use crate::protocol::FsWatchMissingParams;
use crate::protocol::FsWatchMissingResponse;
use crate::rpc::RpcNotificationSender;

use notify::Event;
use notify::EventKind;
use notify::RecursiveMode;
use notify::Watcher;
#[cfg(target_os = "linux")]
use tokio::io::Interest;
#[cfg(target_os = "linux")]
use tokio::io::unix::AsyncFd;
use tokio::sync::oneshot;

const MAX_WATCH_PATHS: usize = 64;

#[derive(Default)]
pub(super) struct MissingWatch {
    id: u64,
    listener: Option<AbortOnDropHandle<()>>,
}

impl MissingWatch {
    pub(super) fn register(
        &mut self,
        params: FsWatchMissingParams,
        notifications: RpcNotificationSender,
        shutdown: CancellationToken,
    ) -> Result<FsWatchMissingResponse, JSONRPCErrorError> {
        let not_watching = FsWatchMissingResponse { watching: false };
        if params.watch_id <= self.id {
            return Ok(not_watching);
        }
        self.id = params.watch_id;
        self.listener.take();
        if !cfg!(target_os = "linux")
            || params.missing.is_empty()
            || params.missing.len() > MAX_WATCH_PATHS
        {
            return Ok(not_watching);
        }
        let watch_id = params.watch_id;
        let watch = match watch_missing(params) {
            Ok(Some(watch)) => watch,
            Ok(None) => return Ok(not_watching),
            Err(error) => {
                tracing::debug!(%error, "metadata watch unavailable; retaining live discovery");
                return Ok(not_watching);
            }
        };
        let changed = FsMissingChangedNotification { watch_id };
        self.listener = Some(AbortOnDropHandle::new(tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => return,
                () = watch => {}
            }
            tracing::debug!(
                watch_id = changed.watch_id,
                "invalidating watched missing paths"
            );
            let _ = notifications
                .notify(FS_MISSING_CHANGED_METHOD, &changed)
                .await;
        })));
        Ok(FsWatchMissingResponse { watching: true })
    }
}

/// Observe directory and mount changes before verifying the paths. Ancestor changes,
/// mount changes, watcher errors and queue overflows invalidate the observation.
fn watch_missing(
    params: FsWatchMissingParams,
) -> io::Result<Option<impl Future<Output = ()> + Send>> {
    let missing = params
        .missing
        .iter()
        .map(|path| path.to_abs_path().map(AbsolutePathBuf::into_path_buf))
        .collect::<io::Result<Vec<_>>>()?;
    let present = params
        .present
        .map(|path| path.to_abs_path().map(AbsolutePathBuf::into_path_buf))
        .transpose()?;
    let mut parents = BTreeSet::new();
    let targets: Vec<_> = missing.iter().chain(&present).cloned().collect();
    for target in &targets {
        for parent in target.ancestors().skip(1) {
            parents.insert(parent.to_owned());
            if parents.len() > MAX_WATCH_PATHS {
                return Ok(None);
            }
        }
    }
    // inotify stays attached to a covered directory. Arm mount-table notifications
    // before installing directory watches and verifying the observed paths.
    #[cfg(target_os = "linux")]
    let mounts = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/proc/self/mounts")
        .and_then(|file| AsyncFd::with_interest(file, Interest::PRIORITY))
        .inspect_err(
            |error| tracing::debug!(%error, "mount changes unavailable for missing-path watch"),
        )
        .ok();
    let (changed, invalidated) = oneshot::channel();
    let mut changed = Some(changed);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        if event.as_ref().map_or(true, |event| {
            event.need_rescan()
                || (!matches!(event.kind, EventKind::Access(_))
                    && (event.paths.is_empty()
                        || event.paths.iter().any(|path| {
                            targets
                                .iter()
                                .any(|target| event_may_affect_target(path, target))
                        })))
        }) && let Some(changed) = changed.take()
        {
            let _ = changed.send(());
        }
    })
    .map_err(io::Error::other)?;

    // Ancestors sort before their descendants. If a missing parent is created during
    // setup its already-watched parent invalidates the observation. Reject symlinks:
    // their targets can change outside any of these directories.
    for parent in parents {
        match std::fs::symlink_metadata(&parent) {
            Ok(metadata) if metadata.is_dir() => {
                // inotify can accept a watch on a network mount yet never see changes
                // made by another host. Only reuse observations on local Linux filesystems.
                #[cfg(target_os = "linux")]
                if !matches!(
                    rustix::fs::statfs(&parent)?.f_type as _,
                    libc::EXT4_SUPER_MAGIC
                        | 0x5846_5342 /* XFS_SUPER_MAGIC from linux/magic.h; absent in libc on musl */
                        | libc::BTRFS_SUPER_MAGIC
                        | libc::TMPFS_MAGIC
                        | libc::OVERLAYFS_SUPER_MAGIC
                ) {
                    return Ok(None);
                }
                watcher
                    .watch(&parent, RecursiveMode::NonRecursive)
                    .map_err(io::Error::other)?;
            }
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    for path in missing {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(path) = present {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_symlink() => {}
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    Ok(Some(async move {
        let _directory_watcher = watcher;
        #[cfg(target_os = "linux")]
        if let Some(mounts) = mounts {
            tokio::select! {
                _ = invalidated => {}
                _ = mounts.ready(Interest::PRIORITY) => {}
            }
            return;
        }
        let _ = invalidated.await;
    }))
}

/// Events preserve the spelling that was written, even in casefolded directories.
/// When either component is non-ASCII, let the next scan resolve Unicode aliases
/// using the filesystem's own rules.
fn event_may_affect_target(path: &Path, target: &Path) -> bool {
    let mut components = target.iter();
    path.iter().all(|name| {
        components.next().is_some_and(|expected| {
            name.eq_ignore_ascii_case(expected) || !name.is_ascii() || !expected.is_ascii()
        })
    })
}
