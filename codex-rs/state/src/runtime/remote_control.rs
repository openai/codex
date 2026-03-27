use super::*;

const REMOTE_CONTROL_ACCOUNT_ID_NONE: &str = "";

fn remote_control_account_id_key(account_id: Option<&str>) -> &str {
    account_id.unwrap_or(REMOTE_CONTROL_ACCOUNT_ID_NONE)
}

impl StateRuntime {
    pub async fn get_remote_control_enrollment(
        &self,
        websocket_url: &str,
        account_id: Option<&str>,
    ) -> anyhow::Result<Option<(String, String)>> {
        let row = sqlx::query(
            r#"
SELECT server_id, server_name
FROM remote_control_enrollments
WHERE websocket_url = ? AND account_id = ?
            "#,
        )
        .bind(websocket_url)
        .bind(remote_control_account_id_key(account_id))
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| Ok((row.try_get("server_id")?, row.try_get("server_name")?)))
            .transpose()
    }

    pub async fn upsert_remote_control_enrollment(
        &self,
        websocket_url: &str,
        account_id: Option<&str>,
        server_id: &str,
        server_name: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO remote_control_enrollments (
    websocket_url,
    account_id,
    server_id,
    server_name,
    updated_at
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT(websocket_url, account_id) DO UPDATE SET
    server_id = excluded.server_id,
    server_name = excluded.server_name,
    updated_at = excluded.updated_at
            "#,
        )
        .bind(websocket_url)
        .bind(remote_control_account_id_key(account_id))
        .bind(server_id)
        .bind(server_name)
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn delete_remote_control_enrollment(
        &self,
        websocket_url: &str,
        account_id: Option<&str>,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
DELETE FROM remote_control_enrollments
WHERE websocket_url = ? AND account_id = ?
            "#,
        )
        .bind(websocket_url)
        .bind(remote_control_account_id_key(account_id))
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn remote_control_enrollment_round_trips_by_target_and_account() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .upsert_remote_control_enrollment(
                "wss://example.com/backend-api/wham/remote/control/server",
                Some("account-a"),
                "srv_e_first",
                "first-server",
            )
            .await
            .expect("insert first enrollment");
        runtime
            .upsert_remote_control_enrollment(
                "wss://example.com/backend-api/wham/remote/control/server",
                Some("account-b"),
                "srv_e_second",
                "second-server",
            )
            .await
            .expect("insert second enrollment");

        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    Some("account-a"),
                )
                .await
                .expect("load first enrollment"),
            Some(("srv_e_first".to_string(), "first-server".to_string()))
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    None,
                )
                .await
                .expect("load missing enrollment"),
            None
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn delete_remote_control_enrollment_removes_only_matching_entry() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .upsert_remote_control_enrollment(
                "wss://example.com/backend-api/wham/remote/control/server",
                None,
                "srv_e_first",
                "first-server",
            )
            .await
            .expect("insert first enrollment");
        runtime
            .upsert_remote_control_enrollment(
                "wss://example.com/backend-api/wham/remote/control/server",
                Some("account-a"),
                "srv_e_second",
                "second-server",
            )
            .await
            .expect("insert second enrollment");

        assert_eq!(
            runtime
                .delete_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    None,
                )
                .await
                .expect("delete first enrollment"),
            1
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    None,
                )
                .await
                .expect("load deleted enrollment"),
            None
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    Some("account-a"),
                )
                .await
                .expect("load retained enrollment"),
            Some(("srv_e_second".to_string(), "second-server".to_string()))
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
