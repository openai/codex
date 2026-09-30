//! Filesystem candidate collection for rollout listing.
//!
//! Keep directory scans and metadata lookups on blocking workers. Updated-time listings need
//! all candidates before sorting; creation-time listings retain directory-level early stopping.
//! Scans check cancellation between directory entries and candidate metadata lookups.

use std::cmp::Reverse;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

use super::MAX_SCAN_FILES;
use super::ThreadCandidate;
use super::parse_timestamp_uuid_from_filename;
use super::truncate_to_millis;
use crate::compression;

/// Runs a scan whose worker observes cancellation when its async waiter is dropped.
async fn run_scan<T, F>(read: F) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&Scan) -> io::Result<T> + Send + 'static,
{
    let (stop, keep_running) = tokio::sync::oneshot::channel::<()>();
    let result = tokio::task::spawn_blocking(move || {
        let scan = Scan { stop };
        scan.check_cancelled()?;
        let result = read(&scan);
        scan.check_cancelled()?;
        result
    })
    .await
    .map_err(io::Error::other)?;
    drop(keep_running);
    result
}

struct Scan {
    stop: tokio::sync::oneshot::Sender<()>,
}

impl Scan {
    fn check_cancelled(&self) -> io::Result<()> {
        if self.stop.is_closed() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "rollout scan cancelled",
            ));
        }
        Ok(())
    }
}

pub(super) async fn collect_dirs_desc<T, F>(
    parent: &Path,
    parse: F,
) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy + Send + 'static,
    F: Fn(&str) -> Option<T> + Send + 'static,
{
    let parent = parent.to_path_buf();
    run_scan(move |scan| collect_dirs_desc_sync(&parent, parse, scan)).await
}

pub(super) async fn collect_rollout_day_files(
    day_path: &Path,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let day_path = day_path.to_path_buf();
    run_scan(move |scan| collect_rollout_day_files_sync(&day_path, scan)).await
}

pub(super) async fn collect_flat_rollout_files(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let root = root.to_path_buf();
    let mut scanned = *scanned_files;
    let (result, scanned) = run_scan(move |scan| {
        let result = collect_flat_rollout_files_sync(&root, &mut scanned, scan);
        let result = result.and_then(|mut files| {
            scan.check_cancelled()?;
            files.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
            Ok(files)
        });
        Ok((result, scanned))
    })
    .await?;
    *scanned_files = scanned;
    result
}

pub(super) async fn collect_files_by_updated_at(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<ThreadCandidate>> {
    let root = root.to_path_buf();
    let mut scanned = *scanned_files;
    let (result, scanned) = run_scan(move |scan| {
        let result = (|| {
            let mut candidates = Vec::new();
            'outer: for (_, year_path) in
                collect_dirs_desc_sync(&root, |s| s.parse::<u16>().ok(), scan)?
            {
                if scanned >= MAX_SCAN_FILES {
                    break;
                }
                for (_, month_path) in
                    collect_dirs_desc_sync(&year_path, |s| s.parse::<u8>().ok(), scan)?
                {
                    if scanned >= MAX_SCAN_FILES {
                        break 'outer;
                    }
                    for (_, day_path) in
                        collect_dirs_desc_sync(&month_path, |s| s.parse::<u8>().ok(), scan)?
                    {
                        if scanned >= MAX_SCAN_FILES {
                            break 'outer;
                        }
                        for (_, id, path) in collect_rollout_day_files_sync(&day_path, scan)? {
                            scan.check_cancelled()?;
                            scanned += 1;
                            if scanned > MAX_SCAN_FILES {
                                break 'outer;
                            }
                            candidates.push(thread_candidate(path, id));
                        }
                    }
                }
            }
            Ok(candidates)
        })();
        Ok((result, scanned))
    })
    .await?;
    *scanned_files = scanned;
    result
}

pub(super) async fn collect_flat_files_by_updated_at(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<ThreadCandidate>> {
    let root = root.to_path_buf();
    let mut scanned = *scanned_files;
    let (result, scanned) = run_scan(move |scan| {
        let result = collect_flat_rollout_files_sync(&root, &mut scanned, scan).and_then(|files| {
            files
                .into_iter()
                .map(|(_, id, path)| {
                    scan.check_cancelled()?;
                    Ok(thread_candidate(path, id))
                })
                .collect()
        });
        Ok((result, scanned))
    })
    .await?;
    *scanned_files = scanned;
    result
}

/// Resolves metadata in the same plain-before-compressed order as rollout readers.
fn thread_candidate(path: PathBuf, id: Uuid) -> ThreadCandidate {
    let updated_at = compression::existing_rollout_with_metadata_sync(&path)
        .map(|(_, metadata)| metadata)
        .and_then(|metadata| metadata.modified().ok())
        .map(OffsetDateTime::from)
        .and_then(truncate_to_millis);
    ThreadCandidate {
        path,
        id,
        updated_at,
    }
}

/// Collects immediate subdirectories of `parent`, parses their (string) names with `parse`,
/// and returns them sorted descending by the parsed key.
fn collect_dirs_desc_sync<T, F>(
    parent: &Path,
    parse: F,
    scan: &Scan,
) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy,
    F: Fn(&str) -> Option<T>,
{
    scan.check_cancelled()?;
    let dir = std::fs::read_dir(parent)?;
    let mut vec: Vec<(T, PathBuf)> = Vec::new();
    for entry in dir {
        scan.check_cancelled()?;
        let entry = entry?;
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
            && let Some(s) = entry.file_name().to_str()
            && let Some(v) = parse(s)
        {
            vec.push((v, entry.path()));
        }
    }
    scan.check_cancelled()?;
    vec.sort_by_key(|(v, _)| Reverse(*v));
    Ok(vec)
}

/// Collects files in a directory and parses them with `parse`.
fn collect_files<T, F>(parent: &Path, mut parse: F, scan: &Scan) -> io::Result<Vec<T>>
where
    F: FnMut(&str, &Path) -> Option<T>,
{
    scan.check_cancelled()?;
    let dir = std::fs::read_dir(parent)?;
    let mut collected: Vec<T> = Vec::new();
    for entry in dir {
        scan.check_cancelled()?;
        let entry = entry?;
        if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
            && let Some(s) = entry.file_name().to_str()
            && let Some(v) = parse(s, &entry.path())
        {
            collected.push(v);
        }
    }
    Ok(collected)
}

fn collect_flat_rollout_files_sync(
    root: &Path,
    scanned_files: &mut usize,
    scan: &Scan,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    scan.check_cancelled()?;
    let dir = std::fs::read_dir(root)?;
    let mut collected = Vec::new();
    for entry in dir {
        scan.check_cancelled()?;
        let entry = entry?;
        if *scanned_files >= MAX_SCAN_FILES {
            break;
        }
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }
        let Some(rollout_file) = compression::RolloutFile::from_path(entry.path()) else {
            continue;
        };
        let Some((ts, id)) = parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
        else {
            continue;
        };
        *scanned_files += 1;
        if *scanned_files > MAX_SCAN_FILES {
            break;
        }
        collected.push((ts, id, rollout_file.into_path()));
    }
    Ok(collected)
}

fn collect_rollout_day_files_sync(
    day_path: &Path,
    scan: &Scan,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let mut day_files = collect_files(
        day_path,
        |_name_str, path| {
            let rollout_file = compression::RolloutFile::from_path(path.to_path_buf())?;
            parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
                .map(|(ts, id)| (ts, id, rollout_file.into_path()))
        },
        scan,
    )?;
    // Stable ordering within the same second: (timestamp desc, uuid desc)
    scan.check_cancelled()?;
    day_files.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
    Ok(day_files)
}

#[cfg(test)]
#[path = "list_files_tests.rs"]
mod tests;
