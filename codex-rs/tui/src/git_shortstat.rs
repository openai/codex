use std::io;
use std::path::Path;
use tokio::process::Command;

/// Parsed summary of `git diff --shortstat` output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffShortStat {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Run `git diff --shortstat` for the current workspace directory and parse the
/// resulting summary if available.
pub(crate) async fn get_diff_shortstat(cwd: &Path) -> io::Result<Option<DiffShortStat>> {
    let output = match Command::new("git")
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["diff", "HEAD", "--shortstat"])
        .output()
        .await
    {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    if !output.status.success() && output.status.code() != Some(1) {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_shortstat(stdout.trim()))
}

pub(crate) async fn get_diff_shortstat_against(
    cwd: &Path,
    base: &str,
) -> io::Result<Option<DiffShortStat>> {
    if base.trim().is_empty() {
        return Ok(None);
    }

    let output = match Command::new("git")
        .current_dir(cwd)
        .args(["diff", base, "--shortstat"])
        .output()
        .await
    {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    if !(output.status.success() || output.status.code() == Some(1)) {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_shortstat(stdout.trim()))
}

fn parse_shortstat(stdout: &str) -> Option<DiffShortStat> {
    if stdout.is_empty() {
        // Zero-diff should still show shortstat with zeros.
        return Some(DiffShortStat::default());
    }

    let mut files_changed: Option<u32> = None;
    let mut insertions: Option<u32> = None;
    let mut deletions: Option<u32> = None;

    for part in stdout.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = match trimmed.split_whitespace().next() {
            Some(num) => match num.parse::<u32>() {
                Ok(parsed) => parsed,
                Err(_) => continue,
            },
            None => continue,
        };

        if trimmed.contains("file") {
            files_changed = Some(value);
        } else if trimmed.contains("insertion") {
            insertions = Some(value);
        } else if trimmed.contains("deletion") {
            deletions = Some(value);
        }
    }

    Some(DiffShortStat {
        files_changed: files_changed.unwrap_or_default(),
        insertions: insertions.unwrap_or_default(),
        deletions: deletions.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shortstat_recognizes_full_output() {
        let summary = " 2 files changed, 7 insertions(+), 3 deletions(-)";

        let parsed = parse_shortstat(summary).expect("should parse stats");

        assert_eq!(parsed.files_changed, 2);
        assert_eq!(parsed.insertions, 7);
        assert_eq!(parsed.deletions, 3);
    }

    #[test]
    fn parse_shortstat_handles_missing_fields() {
        let summary = " 1 file changed";

        let parsed = parse_shortstat(summary).expect("should parse stats");

        assert_eq!(parsed.files_changed, 1);
        assert_eq!(parsed.insertions, 0);
        assert_eq!(parsed.deletions, 0);
    }

    #[test]
    fn parse_shortstat_returns_zeros_for_empty_stdout() {
        let parsed = parse_shortstat("").expect("should parse stats");
        assert_eq!(parsed.files_changed, 0);
        assert_eq!(parsed.insertions, 0);
        assert_eq!(parsed.deletions, 0);
    }
}
