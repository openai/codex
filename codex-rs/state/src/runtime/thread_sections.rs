use super::StateRuntime;
use crate::PINNED_THREAD_SECTION_ID;
use crate::ThreadSection;
use crate::ThreadSectionAppearance;
use uuid::Uuid;

impl StateRuntime {
    /// Create a custom thread section with a stable, server-assigned UUIDv7.
    pub async fn create_thread_section(
        &self,
        name: &str,
        appearance: Option<ThreadSectionAppearance>,
    ) -> anyhow::Result<ThreadSection> {
        let section = ThreadSection {
            id: Uuid::now_v7().to_string(),
            name: name.to_string(),
            appearance,
        };

        sqlx::query("INSERT INTO thread_sections (id, name, appearance) VALUES (?, ?, ?)")
            .bind(&section.id)
            .bind(&section.name)
            .bind(
                section
                    .appearance
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            )
            .execute(self.pool.as_ref())
            .await?;

        Ok(section)
    }

    /// Rename a custom thread section without changing its stable identity.
    pub async fn rename_thread_section(
        &self,
        id: &str,
        name: &str,
        appearance: Option<Option<ThreadSectionAppearance>>,
    ) -> anyhow::Result<Option<ThreadSection>> {
        if id == PINNED_THREAD_SECTION_ID {
            anyhow::bail!("built-in pinned thread section cannot be renamed");
        }

        let replace_appearance = appearance.is_some();
        let appearance = appearance
            .flatten()
            .map(|appearance| serde_json::to_string(&appearance))
            .transpose()?;
        let section = sqlx::query_as::<_, (String, String, Option<String>)>(
            "UPDATE thread_sections SET name = ?, appearance = CASE WHEN ? THEN ? ELSE appearance END WHERE id = ? RETURNING id, name, appearance",
        )
        .bind(name)
        .bind(replace_appearance)
        .bind(appearance)
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        section.map(ThreadSection::from_row).transpose()
    }

    /// Delete a custom section and return its threads to the unsectioned list.
    pub async fn delete_thread_section(&self, id: &str) -> anyhow::Result<bool> {
        if id == PINNED_THREAD_SECTION_ID {
            anyhow::bail!("built-in pinned thread section cannot be deleted");
        }

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "UPDATE threads SET section_position = NULL, section_entered_at_ms = NULL WHERE thread_section_id = ?",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        let deleted = sqlx::query("DELETE FROM thread_sections WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            > 0;
        tx.commit().await?;

        Ok(deleted)
    }
}

#[cfg(test)]
#[path = "thread_sections_tests.rs"]
mod tests;
