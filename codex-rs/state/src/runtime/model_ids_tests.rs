#![cfg(test)]

use super::*;
use crate::SqliteConfig;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_model_facing_id::RegisteredEnvironmentId;
use pretty_assertions::assert_eq;

/// Concurrent writers reserve a key once, and an independent connection observes the winner.
#[tokio::test]
async fn concurrent_assignments_share_a_durable_alias() -> Result<()> {
    let home = tempfile::tempdir()?;
    let config = SqliteConfig::new_for_testing(home.path().abs());
    let a = StateRuntime::init(config.clone(), "test".into()).await?;
    let b = StateRuntime::init(config, "test".into()).await?;
    let session = SessionId::new();
    let key = ModelIdKey::new(&RegisteredEnvironmentId::new("first"))?;
    let (left, right) = tokio::try_join!(
        a.get_or_assign(session, key.clone()),
        b.get_or_assign(session, key.clone()),
    )?;
    assert_eq!(left, right);
    let second = ModelIdKey::new(&RegisteredEnvironmentId::new("second"))?;
    let next = b.get_or_assign(session, second.clone()).await?;
    assert_ne!(left, next);
    assert_eq!(a.lookup(session, key.clone()).await?, Some(left));
    let maps = a.snapshot(session).await?;
    assert_eq!(maps[0].keys, vec![key.key, second.key]);
    Ok(())
}
