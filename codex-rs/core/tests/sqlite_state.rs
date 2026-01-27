use anyhow::Result;
use codex_core::features::Feature;
use codex_state::STATE_DB_FILENAME;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use tokio::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_thread_is_recorded_in_state_db() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let thread_id = test.session_configured.session_id;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let db_path = test.config.codex_home.join(STATE_DB_FILENAME);

    for _ in 0..100 {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let db = codex_state::StateDb::open(&db_path).await?;

    let mut metadata = None;
    for _ in 0..100 {
        metadata = db.get_thread(thread_id).await?;
        if metadata.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let metadata = metadata.expect("thread should exist in state db");
    assert_eq!(metadata.id, thread_id);
    assert_eq!(metadata.rollout_path, rollout_path);

    Ok(())
}
