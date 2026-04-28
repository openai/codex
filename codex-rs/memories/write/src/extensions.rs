use crate::memory_extensions_root;
use chrono::DateTime;
use chrono::Duration;
use chrono::NaiveDateTime;
use chrono::Utc;
use std::path::Path;
use tracing::warn;

pub async fn prune_old_extension_resources(memory_root: &Path) {
    prune_old_extension_resources_with_now(memory_root, Utc::now()).await
}

async fn prune_old_extension_resources_with_now(memory_root: &Path, now: DateTime<Utc>) {
    let cutoff = now - Duration::days(crate::extension_resources::RETENTION_DAYS);
    let extensions_root = memory_extensions_root(memory_root);
    let mut extensions = match tokio::fs::read_dir(&extensions_root).await {
        Ok(extensions) => extensions,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            warn!(
                "failed reading memory extensions root {}: {err}",
                extensions_root.display()
            );
            return;
        }
    };

    while let Ok(Some(extension_entry)) = extensions.next_entry().await {
        let extension_path = extension_entry.path();
        let Ok(file_type) = extension_entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir()
            || !tokio::fs::try_exists(extension_path.join("instructions.md"))
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
            }
        }
    }
}

fn resource_timestamp(file_name: &str) -> Option<DateTime<Utc>> {
    let timestamp = file_name.get(..19)?;
    let naive =
        NaiveDateTime::parse_from_str(timestamp, crate::extension_resources::FILENAME_TS_FORMAT)
            .ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
