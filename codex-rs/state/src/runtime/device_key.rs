use super::*;

/// Persisted account/client binding for a generated device key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyBindingRecord {
    pub key_id: String,
    pub account_user_id: String,
    pub client_id: String,
}

impl StateRuntime {
    pub async fn get_device_key_binding(
        &self,
        key_id: &str,
    ) -> anyhow::Result<Option<DeviceKeyBindingRecord>> {
        let row = sqlx::query(
            r#"
SELECT key_id, account_user_id, client_id
FROM device_key_bindings
WHERE key_id = ?
            "#,
        )
        .bind(key_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| {
            Ok(DeviceKeyBindingRecord {
                key_id: row.try_get("key_id")?,
                account_user_id: row.try_get("account_user_id")?,
                client_id: row.try_get("client_id")?,
            })
        })
        .transpose()
    }

    pub async fn upsert_device_key_binding(
        &self,
        binding: &DeviceKeyBindingRecord,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
INSERT INTO device_key_bindings (
    key_id,
    account_user_id,
    client_id,
    created_at,
    updated_at
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT(key_id) DO UPDATE SET
    account_user_id = excluded.account_user_id,
    client_id = excluded.client_id,
    updated_at = excluded.updated_at
            "#,
        )
        .bind(&binding.key_id)
        .bind(&binding.account_user_id)
        .bind(&binding.client_id)
        .bind(now)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
