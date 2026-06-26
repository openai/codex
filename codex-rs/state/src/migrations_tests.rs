use std::borrow::Cow;

use sqlx::Row;
use sqlx::migrate::Migration;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;

use super::STATE_MIGRATOR;
use super::repair_legacy_recency_migration_version;

fn migrator_through(version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            STATE_MIGRATOR
                .migrations
                .iter()
                .filter(|migration| migration.version <= version)
                .cloned()
                .collect(),
        ),
        ignore_missing: STATE_MIGRATOR.ignore_missing,
        locking: STATE_MIGRATOR.locking,
        table_name: STATE_MIGRATOR.table_name.clone(),
        create_schemas: STATE_MIGRATOR.create_schemas.clone(),
        no_tx: STATE_MIGRATOR.no_tx,
    }
}

#[tokio::test]
async fn guardian_metadata_cleanup_compacts_prompt_projection() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database should open");
    migrator_through(/*version*/ 39)
        .run(&pool)
        .await
        .expect("pre-cleanup migrations should apply");

    sqlx::query(
        r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    created_at_ms,
    updated_at_ms,
    source,
    model_provider,
    cwd,
    title,
    preview,
    sandbox_policy,
    approval_mode,
    first_user_message
) VALUES
    (
        '00000000-0000-0000-0000-000000000001',
        '/tmp/guardian.jsonl',
        1700000000,
        1700000100,
        1700000000123,
        1700000100456,
        '{"subagent":{"other":"guardian"}}',
        'openai',
        '/tmp',
        'large guardian prompt',
        'large guardian prompt',
        'read-only',
        'on-request',
        'large guardian prompt'
    ),
    (
        '00000000-0000-0000-0000-000000000002',
        '/tmp/worker.jsonl',
        1700000000,
        1700000100,
        1700000000123,
        1700000100456,
        '{"subagent":{"other":"worker"}}',
        'openai',
        '/tmp',
        'worker title',
        'worker preview',
        'read-only',
        'on-request',
        'worker first message'
    ),
    (
        '00000000-0000-0000-0000-000000000003',
        '/tmp/named-guardian.jsonl',
        1700000000,
        1700000100,
        1700000000123,
        1700000100456,
        '{"subagent":{"other":"guardian"}}',
        'openai',
        '/tmp',
        'Named Guardian review',
        'large guardian prompt',
        'read-only',
        'on-request',
        'large guardian prompt'
    )
        "#,
    )
    .execute(&pool)
    .await
    .expect("legacy metadata rows should insert");

    STATE_MIGRATOR
        .run(&pool)
        .await
        .expect("guardian metadata cleanup should apply");

    let rows =
        sqlx::query("SELECT id, title, preview, first_user_message FROM threads ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("cleaned metadata rows should load");
    let actual = rows
        .iter()
        .map(|row| {
            (
                row.get::<String, _>("id"),
                row.get::<String, _>("title"),
                row.get::<String, _>("preview"),
                row.get::<String, _>("first_user_message"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (
                "00000000-0000-0000-0000-000000000001".to_string(),
                "Guardian review".to_string(),
                "Approval review".to_string(),
                String::new(),
            ),
            (
                "00000000-0000-0000-0000-000000000002".to_string(),
                "worker title".to_string(),
                "worker preview".to_string(),
                "worker first message".to_string(),
            ),
            (
                "00000000-0000-0000-0000-000000000003".to_string(),
                "Named Guardian review".to_string(),
                "Approval review".to_string(),
                String::new(),
            ),
        ]
    );
}

#[tokio::test]
async fn recency_migration_backfills_and_seeds_old_binary_inserts() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database should open");
    migrator_through(/*version*/ 37)
        .run(&pool)
        .await
        .expect("pre-recency migrations should apply");

    sqlx::query(
        r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    created_at_ms,
    updated_at_ms,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("00000000-0000-0000-0000-000000000001")
    .bind("/tmp/first.jsonl")
    .bind(1_700_000_000_i64)
    .bind(1_700_000_100_i64)
    .bind(1_700_000_000_123_i64)
    .bind(1_700_000_100_456_i64)
    .bind("cli")
    .bind("openai")
    .bind("/tmp")
    .bind("")
    .bind("read-only")
    .bind("on-request")
    .execute(&pool)
    .await
    .expect("legacy row should insert");

    STATE_MIGRATOR
        .run(&pool)
        .await
        .expect("recency migration should apply");

    let backfilled = sqlx::query(
        "SELECT updated_at, updated_at_ms, recency_at, recency_at_ms FROM threads WHERE id = ?",
    )
    .bind("00000000-0000-0000-0000-000000000001")
    .fetch_one(&pool)
    .await
    .expect("backfilled row should load");
    assert_eq!(backfilled.get::<i64, _>("recency_at"), 1_700_000_100);
    assert_eq!(backfilled.get::<i64, _>("recency_at_ms"), 1_700_000_100_456);

    sqlx::query(
        r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    created_at_ms,
    updated_at_ms,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("00000000-0000-0000-0000-000000000002")
    .bind("/tmp/second.jsonl")
    .bind(1_700_000_200_i64)
    .bind(1_700_000_300_i64)
    .bind(1_700_000_200_123_i64)
    .bind(1_700_000_300_456_i64)
    .bind("cli")
    .bind("openai")
    .bind("/tmp")
    .bind("")
    .bind("read-only")
    .bind("on-request")
    .execute(&pool)
    .await
    .expect("old-binary row should insert");

    let seeded = sqlx::query("SELECT recency_at, recency_at_ms FROM threads WHERE id = ?")
        .bind("00000000-0000-0000-0000-000000000002")
        .fetch_one(&pool)
        .await
        .expect("old-binary row should load");
    assert_eq!(seeded.get::<i64, _>("recency_at"), 1_700_000_300);
    assert_eq!(seeded.get::<i64, _>("recency_at_ms"), 1_700_000_300_456);
}

#[tokio::test]
async fn repairs_recency_migration_that_was_applied_as_version_38() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database should open");
    migrator_through(/*version*/ 37)
        .run(&pool)
        .await
        .expect("pre-recency migrations should apply");

    let recency_migration = STATE_MIGRATOR
        .migrations
        .iter()
        .find(|migration| migration.version == 39)
        .expect("recency migration should exist");
    let mut legacy_migrations = STATE_MIGRATOR
        .migrations
        .iter()
        .filter(|migration| migration.version <= 37)
        .cloned()
        .collect::<Vec<_>>();
    legacy_migrations.push(Migration::new(
        38,
        recency_migration.description.clone(),
        recency_migration.migration_type,
        recency_migration.sql.clone(),
        recency_migration.no_tx,
    ));
    let legacy_recency_migrator = Migrator::with_migrations(legacy_migrations);
    legacy_recency_migrator
        .run(&pool)
        .await
        .expect("legacy recency migration should apply as version 38");

    repair_legacy_recency_migration_version(&pool, &STATE_MIGRATOR)
        .await
        .expect("legacy migration history should be repaired");
    STATE_MIGRATOR
        .run(&pool)
        .await
        .expect("current migrations should apply after repair");

    let applied = sqlx::query(
        "SELECT version, checksum FROM _sqlx_migrations WHERE version >= 38 ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .expect("applied migrations should load")
    .into_iter()
    .map(|row| {
        (
            row.get::<i64, _>("version"),
            row.get::<Vec<u8>, _>("checksum"),
        )
    })
    .collect::<Vec<_>>();
    let expected = STATE_MIGRATOR
        .migrations
        .iter()
        .filter(|migration| migration.version >= 38)
        .map(|migration| (migration.version, migration.checksum.to_vec()))
        .collect::<Vec<_>>();
    assert_eq!(applied, expected);
}
