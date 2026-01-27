use anyhow::Result;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) const SESSIONS_SUBDIR: &str = "sessions";
pub(crate) const ROLLOUT_PREFIX: &str = "rollout-";
pub(crate) const ROLLOUT_SUFFIX: &str = ".jsonl";

pub(crate) fn parse_timestamp_uuid_from_filename(name: &str) -> Option<(String, Uuid)> {
    if !name.starts_with(ROLLOUT_PREFIX) || !name.ends_with(ROLLOUT_SUFFIX) {
        return None;
    }
    let core = name
        .strip_prefix(ROLLOUT_PREFIX)?
        .strip_suffix(ROLLOUT_SUFFIX)?;
    let (sep_idx, uuid) = core.match_indices('-').rev().find_map(|(idx, _)| {
        Uuid::parse_str(&core[idx + 1..])
            .ok()
            .map(|uuid| (idx, uuid))
    })?;
    let ts_str = &core[..sep_idx];
    let naive = NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S").ok()?;
    let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
    Some((dt.to_rfc3339_opts(SecondsFormat::Secs, true), uuid))
}

pub(crate) async fn file_modified_time_rfc3339(path: &Path) -> Option<String> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let updated_at: DateTime<Utc> = modified.into();
    Some(updated_at.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub(crate) async fn collect_rollout_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut stack = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(read_dir) => read_dir,
            Err(err) => {
                tracing::warn!("failed to read directory {}: {err}", dir.display());
                continue;
            }
        };
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            if name.starts_with(ROLLOUT_PREFIX) && name.ends_with(ROLLOUT_SUFFIX) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::parse_timestamp_uuid_from_filename;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    #[test]
    fn parses_timestamp_and_uuid_from_filename() {
        let uuid = Uuid::now_v7();
        let name = format!("rollout-2025-03-01T09-00-00-{uuid}.jsonl");
        let (ts, parsed_uuid) = parse_timestamp_uuid_from_filename(&name).expect("parse filename");
        assert_eq!(parsed_uuid, uuid);
        assert_eq!(ts, "2025-03-01T09:00:00Z");
    }
}
