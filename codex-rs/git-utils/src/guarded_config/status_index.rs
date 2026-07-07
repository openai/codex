use std::io;

use super::GuardedGitConfig;
use super::MAX_STATUS_TRACKED_PATHS;
use super::command_failure;
use crate::FsmonitorOverride;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;

impl GuardedGitConfig<'_> {
    /// Return stage-zero paths that can select a Status content filter.
    /// Gitlinks and unmerged stages never convert worktree contents through
    /// clean/process helpers. Index symlinks do when `core.symlinks=false`,
    /// because Git then represents them as ordinary worktree files.
    pub(super) async fn read_status_tracked_paths_async(
        &self,
        core_symlinks: bool,
    ) -> io::Result<Vec<Vec<u8>>> {
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "tracked paths must be read before status policy installation",
            ));
        }
        let mut command =
            self.pending_status_command(FsmonitorOverride::Disabled, /*neutralizer*/ None)?;
        command
            .disable_optional_locks()
            .args(["ls-files", "-z", "--cached", "--stage"]);
        let output = command.output().await?;
        if !output.status.success() {
            return Err(command_failure("tracked-path probe", &output));
        }
        let mut paths = parse_status_filter_candidate_paths(&output.stdout, core_symlinks)?;
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

fn parse_status_filter_candidate_paths(
    output: &[u8],
    core_symlinks: bool,
) -> io::Result<Vec<Vec<u8>>> {
    if output.len() > MAX_INTERNAL_GIT_OUTPUT_BYTES {
        return Err(invalid_status_tracked_path_output(
            "Git tracked-path output exceeds the Status byte limit",
        ));
    }
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let body = output.strip_suffix(&[0]).ok_or_else(|| {
        invalid_status_tracked_path_output("unterminated Git tracked-path output")
    })?;
    if body.is_empty() {
        return Err(invalid_status_tracked_path_output(
            "empty Git tracked-path record",
        ));
    }
    let record_count = body.split(|byte| *byte == 0).count();
    if record_count > MAX_STATUS_TRACKED_PATHS {
        return Err(invalid_status_tracked_path_output(&format!(
            "tracked path record count {record_count} exceeds the status limit {MAX_STATUS_TRACKED_PATHS}",
        )));
    }

    let mut paths = Vec::new();
    for record in body.split(|byte| *byte == 0) {
        if record.is_empty() {
            return Err(invalid_status_tracked_path_output(
                "empty Git tracked-path record",
            ));
        }
        let separator = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| {
                invalid_status_tracked_path_output("missing Git tracked-path separator")
            })?;
        let (header, path) = record.split_at(separator);
        let path = &path[1..];
        if path.is_empty() {
            return Err(invalid_status_tracked_path_output("empty Git tracked path"));
        }

        let mut fields = header.split(|byte| *byte == b' ');
        let mode = fields.next().unwrap_or_default();
        let object_id = fields.next().unwrap_or_default();
        let stage = fields.next().unwrap_or_default();
        if fields.next().is_some() || mode.is_empty() || object_id.is_empty() || stage.is_empty() {
            return Err(invalid_status_tracked_path_output(
                "noncanonical Git tracked-path header",
            ));
        }
        if mode.len() != 6 || !mode.iter().all(|byte| matches!(byte, b'0'..=b'7')) {
            return Err(invalid_status_tracked_path_output(
                "invalid Git tracked-path mode",
            ));
        }
        let filter_candidate = match mode {
            b"100644" | b"100755" => true,
            b"120000" => !core_symlinks,
            b"160000" => false,
            _ => {
                return Err(invalid_status_tracked_path_output(
                    "unsupported Git tracked-path mode",
                ));
            }
        };
        if !matches!(object_id.len(), 40 | 64)
            || !object_id
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(invalid_status_tracked_path_output(
                "invalid Git tracked-path object ID",
            ));
        }
        let stage = match stage {
            b"0" => 0,
            b"1" => 1,
            b"2" => 2,
            b"3" => 3,
            _ => {
                return Err(invalid_status_tracked_path_output(
                    "invalid Git tracked-path stage",
                ));
            }
        };
        if stage == 0 && filter_candidate {
            paths.push(path.to_vec());
        }
    }
    Ok(paths)
}

pub(super) fn status_core_symlinks_for_filter_screening(configured: Option<bool>) -> bool {
    status_core_symlinks_for_filter_screening_on(configured, cfg!(windows))
}

fn status_core_symlinks_for_filter_screening_on(configured: Option<bool>, windows: bool) -> bool {
    configured.unwrap_or(!windows)
}

fn invalid_status_tracked_path_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "status_index_tests.rs"]
mod tests;
