use chrono::DateTime;
use chrono::Utc;
use std::path::Path;

pub(crate) async fn file_modified_time_utc(path: &Path) -> Option<DateTime<Utc>> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let updated_at: DateTime<Utc> = modified.into();
    Some(updated_at)
}
