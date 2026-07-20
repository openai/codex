use std::collections::HashSet;
use std::io;
use std::io::SeekFrom;
use std::io::Write as _;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;

const MIGRATION_MARKER_FILENAME: &str = ".sandbox_migration";

/// removes legacy allow rules that newer codex versions no longer offer.
///
/// this migration is intentionally one-shot. once complete, a marker in `codex_home` prevents
/// policies saved by newer codex versions from being removed on later startups.
pub async fn prefix_rule_migration(
    codex_home: &Path,
    policy_path: &Path,
    banned_prefixes: &[&[&str]],
) -> io::Result<()> {
    let marker_path = codex_home.join(MIGRATION_MARKER_FILENAME);
    if tokio::fs::try_exists(&marker_path).await? {
        return Ok(());
    }
    clean_rules_file(policy_path, banned_prefixes).await?;

    write_migration_marker(codex_home, &marker_path).await?;
    Ok(())
}

// atomically writes the marker after creating codex home when needed.
async fn write_migration_marker(codex_home: &Path, marker_path: &Path) -> io::Result<()> {
    tokio::fs::create_dir_all(codex_home).await?;
    let codex_home = codex_home.to_owned();
    let marker_path = marker_path.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut marker = tempfile::NamedTempFile::new_in(codex_home)?;
        marker.write_all(b"v1\n")?;
        match marker.persist_noclobber(marker_path) {
            Ok(_) => Ok(()),
            Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(err) => Err(err.error),
        }
    })
    .await
    .map_err(io::Error::other)?
}

// removes exact banned allow rules only when the policy needs changing.
async fn clean_rules_file(policy_path: &Path, banned_prefixes: &[&[&str]]) -> io::Result<()> {
    let contents = match tokio::fs::read_to_string(policy_path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if strip_banned_allow_rules(&contents, banned_prefixes) == contents {
        return Ok(());
    }

    let mut file = match tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(policy_path)
        .await
    {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let retained = strip_banned_allow_rules(&contents, banned_prefixes);
    if retained == contents {
        return Ok(());
    }

    file.seek(SeekFrom::Start(0)).await?;
    file.write_all(retained.as_bytes()).await?;
    file.set_len(retained.len() as u64).await?;
    Ok(())
}

// returns the policy text without exact banned allow rules.
fn strip_banned_allow_rules(contents: &str, banned_prefixes: &[&[&str]]) -> String {
    let banned_prefixes = banned_prefixes
        .iter()
        .map(|prefix| {
            prefix
                .iter()
                .map(|token| token.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .collect::<HashSet<_>>();
    contents
        .split_inclusive('\n')
        .filter(|line| !should_remove_rule(line, &banned_prefixes))
        .collect()
}

// checks whether a line is an exact banned allow rule.
fn should_remove_rule(line: &str, banned_prefixes: &HashSet<Vec<String>>) -> bool {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let Some(pattern) = line
        .strip_prefix("prefix_rule(pattern=")
        .and_then(|line| line.strip_suffix(r#", decision="allow")"#))
    else {
        return false;
    };
    let Ok(prefix) = serde_json::from_str::<Vec<String>>(pattern) else {
        return false;
    };
    let prefix = prefix
        .iter()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    banned_prefixes.contains(&prefix)
}

#[cfg(test)]
#[path = "sandbox_migration_tests.rs"]
mod tests;
