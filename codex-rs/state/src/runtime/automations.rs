use super::*;
use crate::Automation;
use crate::model::AutomationRow;

impl StateRuntime {
    pub async fn list_automations(&self) -> anyhow::Result<Vec<Automation>> {
        let rows = sqlx::query_as::<_, AutomationRow>(
            r#"
SELECT *
FROM automations
ORDER BY updated_at DESC, id ASC
            "#,
        )
        .fetch_all(self.automations_pool.as_ref())
        .await?;
        rows.into_iter()
            .map(Automation::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_automation(&self, id: &str) -> anyhow::Result<Option<Automation>> {
        let row = sqlx::query_as::<_, AutomationRow>(
            r#"
SELECT *
FROM automations
WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.automations_pool.as_ref())
        .await?;
        row.map(Automation::try_from).transpose()
    }
}
