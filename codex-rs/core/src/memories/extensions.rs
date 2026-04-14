use crate::memories::memory_extensions_root;
use chrono::DateTime;
use chrono::Duration;
use chrono::NaiveDateTime;
use chrono::Utc;
use std::path::Path;
use tracing::warn;

const FILENAME_TS_FORMAT: &str = "%Y-%m-%dT%H-%M-%S";
pub(super) const EXTENSION_RESOURCE_RETENTION_DAYS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemovedExtensionResource {
    pub(super) extension: String,
    pub(super) resource_path: String,
}

pub(super) async fn prune_old_extension_resources(
    memory_root: &Path,
) -> Vec<RemovedExtensionResource> {
    prune_old_extension_resources_with_now(memory_root, Utc::now()).await
}

async fn prune_old_extension_resources_with_now(
    memory_root: &Path,
    now: DateTime<Utc>,
) -> Vec<RemovedExtensionResource> {
    let mut removed = Vec::new();
    let cutoff = now - Duration::days(EXTENSION_RESOURCE_RETENTION_DAYS);
    let extensions_root = memory_extensions_root(memory_root);
    let mut extensions = match tokio::fs::read_dir(&extensions_root).await {
        Ok(extensions) => extensions,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return removed,
        Err(err) => {
            warn!(
                "failed reading memory extensions root {}: {err}",
                extensions_root.display()
            );
            return removed;
        }
    };

    while let Ok(Some(extension_entry)) = extensions.next_entry().await {
        let extension_path = extension_entry.path();
        let Ok(file_type) = extension_entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(extension) = extension_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        if !tokio::fs::try_exists(extension_path.join("instructions.md"))
            .await
            .unwrap_or(false)
        {
            continue;
        }

        let resources_path = extension_path.join("resources");
        let mut resources = match tokio::fs::read_dir(&resources_path).await {
            Ok(resources) => resources,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                warn!(
                    "failed reading memory extension resources {}: {err}",
                    resources_path.display()
                );
                continue;
            }
        };

        while let Ok(Some(resource_entry)) = resources.next_entry().await {
            let resource_file_path = resource_entry.path();
            let Ok(file_type) = resource_entry.file_type().await else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let Some(file_name) = resource_file_path
                .file_name()
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            if !file_name.ends_with(".md") {
                continue;
            }
            let Some(resource_timestamp) = resource_timestamp(file_name) else {
                continue;
            };
            if resource_timestamp > cutoff {
                continue;
            }

            if let Err(err) = tokio::fs::remove_file(&resource_file_path).await
                && err.kind() != std::io::ErrorKind::NotFound
            {
                warn!(
                    "failed pruning old memory extension resource {}: {err}",
                    resource_file_path.display()
                );
                continue;
            }
            removed.push(RemovedExtensionResource {
                extension: extension.clone(),
                resource_path: format!("resources/{file_name}"),
            });
        }
    }

    removed.sort_by(|left, right| {
        left.extension
            .cmp(&right.extension)
            .then_with(|| left.resource_path.cmp(&right.resource_path))
    });
    removed
}

fn resource_timestamp(file_name: &str) -> Option<DateTime<Utc>> {
    let timestamp = file_name.get(..19)?;
    let naive = NaiveDateTime::parse_from_str(timestamp, FILENAME_TS_FORMAT).ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn prunes_only_old_resources_from_extensions_with_instructions() {
        let codex_home = TempDir::new().expect("create temp codex home");
        let memory_root = codex_home.path().join("memories");
        let extensions_root = memory_extensions_root(&memory_root);
        let telepathy_resources = extensions_root.join("telepathy/resources");
        tokio::fs::create_dir_all(&telepathy_resources)
            .await
            .expect("create telepathy resources");
        tokio::fs::write(
            extensions_root.join("telepathy/instructions.md"),
            "instructions",
        )
        .await
        .expect("write telepathy instructions");

        let now = DateTime::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2026-04-14T12-00-00", FILENAME_TS_FORMAT)
                .expect("parse now"),
            Utc,
        );
        let old_file = telepathy_resources.join("2026-04-06T11-59-59-abcd-10min-old.md");
        let exact_cutoff_file =
            telepathy_resources.join("2026-04-07T12-00-00-abcd-10min-cutoff.md");
        let recent_file = telepathy_resources.join("2026-04-08T12-00-00-abcd-10min-recent.md");
        let invalid_file = telepathy_resources.join("not-a-timestamp.md");
        for file in [&old_file, &exact_cutoff_file, &recent_file, &invalid_file] {
            tokio::fs::write(file, "resource")
                .await
                .expect("write telepathy resource");
        }

        let ignored_resources = extensions_root.join("ignored/resources");
        tokio::fs::create_dir_all(&ignored_resources)
            .await
            .expect("create ignored resources");
        let ignored_old_file = ignored_resources.join("2026-04-06T11-59-59-abcd-10min-old.md");
        tokio::fs::write(&ignored_old_file, "ignored")
            .await
            .expect("write ignored resource");

        let removed = prune_old_extension_resources_with_now(&memory_root, now).await;

        assert_eq!(
            removed,
            vec![
                RemovedExtensionResource {
                    extension: "telepathy".to_string(),
                    resource_path: "resources/2026-04-06T11-59-59-abcd-10min-old.md".to_string(),
                },
                RemovedExtensionResource {
                    extension: "telepathy".to_string(),
                    resource_path: "resources/2026-04-07T12-00-00-abcd-10min-cutoff.md".to_string(),
                },
            ]
        );
        assert!(
            !tokio::fs::try_exists(&old_file)
                .await
                .expect("check old file")
        );
        assert!(
            !tokio::fs::try_exists(&exact_cutoff_file)
                .await
                .expect("check cutoff file")
        );
        assert!(
            tokio::fs::try_exists(&recent_file)
                .await
                .expect("check recent file")
        );
        assert!(
            tokio::fs::try_exists(&invalid_file)
                .await
                .expect("check invalid file")
        );
        assert!(
            tokio::fs::try_exists(&ignored_old_file)
                .await
                .expect("check ignored old file")
        );
    }
}
