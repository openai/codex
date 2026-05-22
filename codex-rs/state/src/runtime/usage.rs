use super::*;
use crate::UsageEntry;
use crate::UsageHeadline;
use crate::UsageRange;
use crate::UsageReport;
use crate::UsageSample;
use codex_protocol::protocol::UsageContributorKind;

const USAGE_RETENTION_DAYS: i64 = 14;
const USAGE_RETENTION_SECONDS: i64 = USAGE_RETENTION_DAYS * 24 * 60 * 60;

impl StateRuntime {
    pub async fn record_usage_sample(&self, sample: &UsageSample) -> anyhow::Result<()> {
        let usage = &sample.attribution;
        let token_usage = &usage.token_usage;
        let retention_cutoff = usage_retention_cutoff(Utc::now().timestamp());
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
INSERT INTO usage_samples (
    sample_id,
    thread_id,
    turn_id,
    response_id,
    occurred_at,
    input_tokens,
    cached_input_tokens,
    non_cached_input_tokens,
    output_tokens,
    reasoning_output_tokens,
    total_tokens,
    blended_tokens,
    prompt_estimated_tokens
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(sample_id) DO UPDATE SET
    thread_id = excluded.thread_id,
    turn_id = excluded.turn_id,
    response_id = excluded.response_id,
    occurred_at = excluded.occurred_at,
    input_tokens = excluded.input_tokens,
    cached_input_tokens = excluded.cached_input_tokens,
    non_cached_input_tokens = excluded.non_cached_input_tokens,
    output_tokens = excluded.output_tokens,
    reasoning_output_tokens = excluded.reasoning_output_tokens,
    total_tokens = excluded.total_tokens,
    blended_tokens = excluded.blended_tokens,
    prompt_estimated_tokens = excluded.prompt_estimated_tokens
            "#,
        )
        .bind(usage.sample_id.as_str())
        .bind(sample.thread_id.to_string())
        .bind(usage.turn_id.as_str())
        .bind(usage.response_id.as_str())
        .bind(usage.occurred_at)
        .bind(token_usage.input_tokens)
        .bind(token_usage.cached_input_tokens)
        .bind(token_usage.non_cached_input())
        .bind(token_usage.output_tokens)
        .bind(token_usage.reasoning_output_tokens)
        .bind(token_usage.total_tokens)
        .bind(token_usage.blended_total())
        .bind(usage.prompt_estimated_tokens)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM usage_sample_contributors WHERE sample_id = ?")
            .bind(usage.sample_id.as_str())
            .execute(&mut *tx)
            .await?;
        for contributor in &usage.contributors {
            sqlx::query(
                r#"
INSERT INTO usage_sample_contributors (
    sample_id,
    kind,
    contributor_id,
    label,
    source_estimated_tokens,
    attributed_tokens
) VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(usage.sample_id.as_str())
            .bind(usage_kind_key(contributor.contributor.kind))
            .bind(contributor.contributor.id.as_str())
            .bind(contributor.contributor.label.as_str())
            .bind(contributor.source_estimated_tokens)
            .bind(contributor.attributed_tokens)
            .execute(&mut *tx)
            .await?;
        }
        prune_usage_samples_before(retention_cutoff, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(crate) async fn run_usage_startup_maintenance(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        prune_usage_samples_before(usage_retention_cutoff(Utc::now().timestamp()), &mut tx).await?;
        tx.commit().await?;
        // PASSIVE checkpoints copy whatever is immediately available and skip
        // frames that would require waiting on active readers or writers.
        sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
            .execute(self.pool.as_ref())
            .await?;
        // Reclaim any free pages left by retention pruning when incremental auto-vacuum is active.
        sqlx::query("PRAGMA incremental_vacuum")
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    pub async fn read_usage_report(
        &self,
        range: UsageRange,
        now: i64,
    ) -> anyhow::Result<UsageReport> {
        let since = now.saturating_sub(range.seconds());
        let total_tokens: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(blended_tokens), 0) FROM usage_samples WHERE occurred_at >= ?",
        )
        .bind(since)
        .fetch_one(self.pool.as_ref())
        .await?;
        let tracked_from: Option<i64> =
            sqlx::query_scalar("SELECT MIN(occurred_at) FROM usage_samples")
                .fetch_one(self.pool.as_ref())
                .await?;
        let mut report = UsageReport {
            range,
            generated_at: now,
            tracked_from,
            total_tokens,
            headline: None,
            skills: self
                .read_usage_contributors(since, UsageContributorKind::Skill, total_tokens)
                .await?,
            subagents: self.read_subagent_usage(since, total_tokens).await?,
            apps: self
                .read_usage_contributors(since, UsageContributorKind::App, total_tokens)
                .await?,
            mcp_servers: self
                .read_usage_contributors(since, UsageContributorKind::McpServer, total_tokens)
                .await?,
            plugins: self
                .read_usage_contributors(since, UsageContributorKind::Plugin, total_tokens)
                .await?,
        };
        report.headline = usage_headline(&report);
        Ok(report)
    }

    async fn read_usage_contributors(
        &self,
        since: i64,
        kind: UsageContributorKind,
        total_tokens: i64,
    ) -> anyhow::Result<Vec<UsageEntry>> {
        let rows = sqlx::query(
            r#"
SELECT contributor_id, label, SUM(attributed_tokens) AS attributed_tokens
FROM usage_sample_contributors
JOIN usage_samples ON usage_samples.sample_id = usage_sample_contributors.sample_id
WHERE usage_samples.occurred_at >= ?
  AND usage_sample_contributors.kind = ?
GROUP BY contributor_id, label
HAVING SUM(attributed_tokens) > 0
ORDER BY attributed_tokens DESC, label ASC
            "#,
        )
        .bind(since)
        .bind(usage_kind_key(kind))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| {
                let attributed_tokens = row.try_get("attributed_tokens")?;
                Ok(UsageEntry {
                    kind,
                    id: row.try_get("contributor_id")?,
                    label: row.try_get("label")?,
                    attributed_tokens,
                    percent_of_usage: usage_percent(attributed_tokens, total_tokens),
                })
            })
            .collect()
    }

    async fn read_subagent_usage(
        &self,
        since: i64,
        total_tokens: i64,
    ) -> anyhow::Result<Vec<UsageEntry>> {
        let rows = sqlx::query(
            r#"
SELECT
    COALESCE(NULLIF(threads.agent_role, ''), NULLIF(threads.agent_nickname, ''), 'unnamed') AS label,
    COALESCE(NULLIF(threads.agent_role, ''), NULLIF(threads.agent_nickname, ''), 'unnamed') AS contributor_id,
    SUM(usage_samples.blended_tokens) AS attributed_tokens
FROM usage_samples
JOIN threads ON threads.id = usage_samples.thread_id
WHERE usage_samples.occurred_at >= ?
  AND threads.thread_source = 'subagent'
GROUP BY contributor_id, label
HAVING SUM(usage_samples.blended_tokens) > 0
ORDER BY attributed_tokens DESC, label ASC
            "#,
        )
        .bind(since)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| {
                let attributed_tokens = row.try_get("attributed_tokens")?;
                Ok(UsageEntry {
                    kind: UsageContributorKind::Subagent,
                    id: row.try_get("contributor_id")?,
                    label: row.try_get("label")?,
                    attributed_tokens,
                    percent_of_usage: usage_percent(attributed_tokens, total_tokens),
                })
            })
            .collect()
    }
}

async fn prune_usage_samples_before(
    cutoff_ts: i64,
    tx: &mut SqliteConnection,
) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM usage_samples WHERE occurred_at < ?")
        .bind(cutoff_ts)
        .execute(&mut *tx)
        .await?;
    Ok(result.rows_affected())
}

fn usage_retention_cutoff(now: i64) -> i64 {
    now.saturating_sub(USAGE_RETENTION_SECONDS)
}

fn usage_kind_key(kind: UsageContributorKind) -> &'static str {
    match kind {
        UsageContributorKind::Skill => "skill",
        UsageContributorKind::Subagent => "subagent",
        UsageContributorKind::App => "app",
        UsageContributorKind::McpServer => "mcp_server",
        UsageContributorKind::Plugin => "plugin",
    }
}

fn usage_percent(attributed_tokens: i64, total_tokens: i64) -> u8 {
    if attributed_tokens <= 0 || total_tokens <= 0 {
        return 0;
    }
    let rounded = attributed_tokens
        .saturating_mul(/*rhs*/ 100)
        .saturating_add(total_tokens / 2)
        / total_tokens;
    u8::try_from(rounded.max(/*other*/ 1).min(i64::from(u8::MAX))).unwrap_or(u8::MAX)
}

fn usage_headline(report: &UsageReport) -> Option<UsageHeadline> {
    let entry = report
        .skills
        .iter()
        .chain(report.subagents.iter())
        .chain(report.apps.iter())
        .chain(report.mcp_servers.iter())
        .chain(report.plugins.iter())
        .max_by(|left, right| {
            left.attributed_tokens
                .cmp(&right.attributed_tokens)
                .then_with(|| right.label.cmp(&left.label))
        })?
        .clone();
    let note = matches!(
        entry.kind,
        UsageContributorKind::App | UsageContributorKind::McpServer
    )
    .then(|| {
        "Tool results stay in context until compaction; compact or disable sources you do not need."
            .to_string()
    });
    Some(UsageHeadline { entry, note })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::test_thread_metadata;
    use crate::runtime::test_support::unique_temp_dir;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::ThreadSource;
    use codex_protocol::protocol::TokenUsage;
    use codex_protocol::protocol::UsageAttributionContributor;
    use codex_protocol::protocol::UsageAttributionItem;
    use codex_protocol::protocol::UsageContributor;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn usage_report_groups_forward_only_samples_by_range() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let user_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000901").expect("valid thread id");
        let subagent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000902").expect("valid thread id");
        let now = Utc::now().timestamp();

        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                user_thread_id,
                codex_home.clone(),
            ))
            .await
            .expect("user thread insert should succeed");
        let mut subagent_metadata =
            test_thread_metadata(&codex_home, subagent_thread_id, codex_home.clone());
        subagent_metadata.thread_source = Some(ThreadSource::Subagent);
        subagent_metadata.agent_role = Some("code-review".to_string());
        runtime
            .upsert_thread(&subagent_metadata)
            .await
            .expect("subagent thread insert should succeed");

        runtime
            .record_usage_sample(&usage_sample(
                user_thread_id,
                "recent-user",
                /*occurred_at*/ now - 100,
                TokenUsage {
                    input_tokens: 100,
                    cached_input_tokens: 20,
                    output_tokens: 40,
                    reasoning_output_tokens: 0,
                    total_tokens: 140,
                },
                vec![
                    contributor(
                        UsageContributorKind::Skill,
                        "/skills/tmux",
                        "tmux",
                        /*attributed_tokens*/ 50,
                    ),
                    contributor(
                        UsageContributorKind::App,
                        "slack",
                        "Slack",
                        /*attributed_tokens*/ 70,
                    ),
                ],
            ))
            .await
            .expect("recent usage sample should persist");
        runtime
            .record_usage_sample(&usage_sample(
                subagent_thread_id,
                "recent-subagent",
                /*occurred_at*/ now - 50,
                TokenUsage {
                    input_tokens: 30,
                    cached_input_tokens: 0,
                    output_tokens: 10,
                    reasoning_output_tokens: 0,
                    total_tokens: 40,
                },
                Vec::new(),
            ))
            .await
            .expect("subagent usage sample should persist");
        runtime
            .record_usage_sample(&usage_sample(
                user_thread_id,
                "old-user",
                /*occurred_at*/ now - UsageRange::Day.seconds() - 1,
                TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                    reasoning_output_tokens: 0,
                    total_tokens: 10,
                },
                vec![contributor(
                    UsageContributorKind::McpServer,
                    "old-mcp",
                    "old-mcp",
                    /*attributed_tokens*/ 10,
                )],
            ))
            .await
            .expect("old usage sample should persist");

        assert_eq!(
            runtime
                .read_usage_report(UsageRange::Day, now)
                .await
                .expect("usage report should load"),
            UsageReport {
                range: UsageRange::Day,
                generated_at: now,
                tracked_from: Some(now - UsageRange::Day.seconds() - 1),
                total_tokens: 160,
                headline: Some(UsageHeadline {
                    entry: UsageEntry {
                        kind: UsageContributorKind::App,
                        id: "slack".to_string(),
                        label: "Slack".to_string(),
                        attributed_tokens: 70,
                        percent_of_usage: 44,
                    },
                    note: Some(
                        "Tool results stay in context until compaction; compact or disable sources you do not need."
                            .to_string(),
                    ),
                }),
                skills: vec![UsageEntry {
                    kind: UsageContributorKind::Skill,
                    id: "/skills/tmux".to_string(),
                    label: "tmux".to_string(),
                    attributed_tokens: 50,
                    percent_of_usage: 31,
                }],
                subagents: vec![UsageEntry {
                    kind: UsageContributorKind::Subagent,
                    id: "code-review".to_string(),
                    label: "code-review".to_string(),
                    attributed_tokens: 40,
                    percent_of_usage: 25,
                }],
                apps: vec![UsageEntry {
                    kind: UsageContributorKind::App,
                    id: "slack".to_string(),
                    label: "Slack".to_string(),
                    attributed_tokens: 70,
                    percent_of_usage: 44,
                }],
                mcp_servers: Vec::new(),
                plugins: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn usage_report_labels_unnamed_subagents_as_unnamed() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let subagent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000905").expect("valid thread id");
        let mut subagent_metadata =
            test_thread_metadata(&codex_home, subagent_thread_id, codex_home.clone());
        subagent_metadata.thread_source = Some(ThreadSource::Subagent);
        runtime
            .upsert_thread(&subagent_metadata)
            .await
            .expect("subagent thread insert should succeed");
        let now = Utc::now().timestamp();

        runtime
            .record_usage_sample(&usage_sample(
                subagent_thread_id,
                "unnamed-subagent",
                /*occurred_at*/ now,
                TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                    total_tokens: 15,
                },
                Vec::new(),
            ))
            .await
            .expect("subagent usage sample should persist");

        let report = runtime
            .read_usage_report(UsageRange::Day, now)
            .await
            .expect("usage report should load");

        assert_eq!(
            report.subagents,
            vec![UsageEntry {
                kind: UsageContributorKind::Subagent,
                id: "unnamed".to_string(),
                label: "unnamed".to_string(),
                attributed_tokens: 15,
                percent_of_usage: 100,
            }]
        );
    }

    #[tokio::test]
    async fn record_usage_sample_prunes_samples_older_than_retention() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000903").expect("valid thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await
            .expect("thread insert should succeed");
        let now = Utc::now().timestamp();

        runtime
            .record_usage_sample(&usage_sample(
                thread_id,
                "stale",
                /*occurred_at*/ now - USAGE_RETENTION_SECONDS - 1,
                TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                    total_tokens: 15,
                },
                vec![contributor(
                    UsageContributorKind::Skill,
                    "/skills/stale",
                    "stale",
                    /*attributed_tokens*/ 10,
                )],
            ))
            .await
            .expect("stale usage sample should persist then prune");
        runtime
            .record_usage_sample(&usage_sample(
                thread_id,
                "retained",
                /*occurred_at*/ now - UsageRange::Week.seconds(),
                TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                    total_tokens: 15,
                },
                vec![contributor(
                    UsageContributorKind::Skill,
                    "/skills/retained",
                    "retained",
                    /*attributed_tokens*/ 10,
                )],
            ))
            .await
            .expect("retained usage sample should persist");

        assert_eq!(usage_sample_count(&runtime).await, 1);
        assert_eq!(usage_contributor_count(&runtime).await, 1);
    }

    #[tokio::test]
    async fn usage_startup_maintenance_prunes_stale_samples() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000904").expect("valid thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.clone(),
            ))
            .await
            .expect("thread insert should succeed");
        let now = Utc::now().timestamp();
        runtime
            .record_usage_sample(&usage_sample(
                thread_id,
                "stale-after-write",
                /*occurred_at*/ now,
                TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 0,
                    output_tokens: 5,
                    reasoning_output_tokens: 0,
                    total_tokens: 15,
                },
                vec![contributor(
                    UsageContributorKind::Skill,
                    "/skills/stale",
                    "stale",
                    /*attributed_tokens*/ 10,
                )],
            ))
            .await
            .expect("usage sample should persist");
        sqlx::query("UPDATE usage_samples SET occurred_at = ? WHERE sample_id = ?")
            .bind(/*value*/ now - USAGE_RETENTION_SECONDS - 1)
            .bind("stale-after-write")
            .execute(runtime.pool.as_ref())
            .await
            .expect("usage sample should age");

        runtime
            .run_usage_startup_maintenance()
            .await
            .expect("usage startup maintenance should succeed");

        assert_eq!(usage_sample_count(&runtime).await, 0);
        assert_eq!(usage_contributor_count(&runtime).await, 0);
    }

    fn usage_sample(
        thread_id: ThreadId,
        sample_id: &str,
        occurred_at: i64,
        token_usage: TokenUsage,
        contributors: Vec<UsageAttributionContributor>,
    ) -> UsageSample {
        UsageSample {
            thread_id,
            attribution: UsageAttributionItem {
                sample_id: sample_id.to_string(),
                turn_id: format!("{sample_id}-turn"),
                response_id: format!("{sample_id}-response"),
                occurred_at,
                token_usage,
                prompt_estimated_tokens: 100,
                contributors,
            },
        }
    }

    fn contributor(
        kind: UsageContributorKind,
        id: &str,
        label: &str,
        attributed_tokens: i64,
    ) -> UsageAttributionContributor {
        UsageAttributionContributor {
            contributor: UsageContributor {
                kind,
                id: id.to_string(),
                label: label.to_string(),
            },
            source_estimated_tokens: attributed_tokens,
            attributed_tokens,
        }
    }

    async fn usage_sample_count(runtime: &StateRuntime) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM usage_samples")
            .fetch_one(runtime.pool.as_ref())
            .await
            .expect("usage sample count should load")
    }

    async fn usage_contributor_count(runtime: &StateRuntime) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM usage_sample_contributors")
            .fetch_one(runtime.pool.as_ref())
            .await
            .expect("usage contributor count should load")
    }
}
