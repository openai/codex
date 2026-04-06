use super::*;

const REMOTE_CONTROL_APP_SERVER_CLIENT_NAME_NONE: &str = "";

/// Persisted remote-control server enrollment, including the lookup key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteControlEnrollmentRecord {
    pub websocket_url: String,
    pub account_id: String,
    pub app_server_client_name: Option<String>,
    pub server_id: String,
    pub environment_id: String,
    pub server_name: String,
}

fn remote_control_app_server_client_name_key(app_server_client_name: Option<&str>) -> &str {
    app_server_client_name.unwrap_or(REMOTE_CONTROL_APP_SERVER_CLIENT_NAME_NONE)
}

fn app_server_client_name_from_key(app_server_client_name: String) -> Option<String> {
    if app_server_client_name.is_empty() {
        None
    } else {
        Some(app_server_client_name)
    }
}

impl StateRuntime {
    pub async fn get_remote_control_enrollment(
        &self,
        websocket_url: &str,
        account_id: &str,
        app_server_client_name: Option<&str>,
    ) -> anyhow::Result<Option<RemoteControlEnrollmentRecord>> {
        let row = sqlx::query(
            r#"
SELECT websocket_url, account_id, app_server_client_name, server_id, environment_id, server_name
FROM remote_control_enrollments
WHERE websocket_url = ? AND account_id = ? AND app_server_client_name = ?
            "#,
        )
        .bind(websocket_url)
        .bind(account_id)
        .bind(remote_control_app_server_client_name_key(
            app_server_client_name,
        ))
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| {
            let app_server_client_name: String = row.try_get("app_server_client_name")?;
            Ok(RemoteControlEnrollmentRecord {
                websocket_url: row.try_get("websocket_url")?,
                account_id: row.try_get("account_id")?,
                app_server_client_name: app_server_client_name_from_key(app_server_client_name),
                server_id: row.try_get("server_id")?,
                environment_id: row.try_get("environment_id")?,
                server_name: row.try_get("server_name")?,
            })
        })
        .transpose()
    }

    pub async fn upsert_remote_control_enrollment(
        &self,
        enrollment: &RemoteControlEnrollmentRecord,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO remote_control_enrollments (
    websocket_url,
    account_id,
    app_server_client_name,
    server_id,
    environment_id,
    server_name,
    updated_at
) VALUES (?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(websocket_url, account_id, app_server_client_name) DO UPDATE SET
    server_id = excluded.server_id,
    environment_id = excluded.environment_id,
    server_name = excluded.server_name,
    updated_at = excluded.updated_at
            "#,
        )
        .bind(&enrollment.websocket_url)
        .bind(&enrollment.account_id)
        .bind(remote_control_app_server_client_name_key(
            enrollment.app_server_client_name.as_deref(),
        ))
        .bind(&enrollment.server_id)
        .bind(&enrollment.environment_id)
        .bind(&enrollment.server_name)
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn delete_remote_control_enrollment(
        &self,
        websocket_url: &str,
        account_id: &str,
        app_server_client_name: Option<&str>,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
DELETE FROM remote_control_enrollments
WHERE websocket_url = ? AND account_id = ? AND app_server_client_name = ?
            "#,
        )
        .bind(websocket_url)
        .bind(account_id)
        .bind(remote_control_app_server_client_name_key(
            app_server_client_name,
        ))
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteControlEnrollmentRecord;
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
            .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-a".to_string(),
                app_server_client_name: Some("desktop-client".to_string()),
                server_id: "srv_e_first".to_string(),
                environment_id: "env_first".to_string(),
                server_name: "first-server".to_string(),
            })
            .await
            .expect("insert first enrollment");
        runtime
            .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-b".to_string(),
                app_server_client_name: Some("desktop-client".to_string()),
                server_id: "srv_e_second".to_string(),
                environment_id: "env_second".to_string(),
                server_name: "second-server".to_string(),
            })
            .await
            .expect("insert second enrollment");

        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-a",
                    Some("desktop-client"),
                )
                .await
                .expect("load first enrollment"),
            Some(RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-a".to_string(),
                app_server_client_name: Some("desktop-client".to_string()),
                server_id: "srv_e_first".to_string(),
                environment_id: "env_first".to_string(),
                server_name: "first-server".to_string(),
            })
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-missing",
                    Some("desktop-client"),
                )
                .await
                .expect("load missing enrollment"),
            None
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-a",
                    Some("other-client"),
                )
                .await
                .expect("load wrong client enrollment"),
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
            .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-a".to_string(),
                app_server_client_name: None,
                server_id: "srv_e_first".to_string(),
                environment_id: "env_first".to_string(),
                server_name: "first-server".to_string(),
            })
            .await
            .expect("insert first enrollment");
        runtime
            .upsert_remote_control_enrollment(&RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-b".to_string(),
                app_server_client_name: None,
                server_id: "srv_e_second".to_string(),
                environment_id: "env_second".to_string(),
                server_name: "second-server".to_string(),
            })
            .await
            .expect("insert second enrollment");

        assert_eq!(
            runtime
                .delete_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-a",
                    /*app_server_client_name*/ None,
                )
                .await
                .expect("delete first enrollment"),
            1
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-a",
                    /*app_server_client_name*/ None,
                )
                .await
                .expect("load deleted enrollment"),
            None
        );
        assert_eq!(
            runtime
                .get_remote_control_enrollment(
                    "wss://example.com/backend-api/wham/remote/control/server",
                    "account-b",
                    /*app_server_client_name*/ None,
                )
                .await
                .expect("load retained enrollment"),
            Some(RemoteControlEnrollmentRecord {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                account_id: "account-b".to_string(),
                app_server_client_name: None,
                server_id: "srv_e_second".to_string(),
                environment_id: "env_second".to_string(),
                server_name: "second-server".to_string(),
            })
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
