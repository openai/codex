use super::StateRuntime;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use std::collections::HashMap;

const SECTION_POSITION_GAP: i64 = 1_000_000;

impl StateRuntime {
    /// Read persisted section ordering for multiple threads in one SQLite query.
    pub async fn get_thread_section_ordering(
        &self,
        thread_ids: &[ThreadId],
    ) -> anyhow::Result<HashMap<ThreadId, (Option<i64>, Option<DateTime<Utc>>)>> {
        if thread_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id, section_position, section_entered_at_ms FROM threads WHERE id IN (",
        );
        let mut separated = builder.separated(", ");
        for thread_id in thread_ids {
            separated.push_bind(thread_id.to_string());
        }
        separated.push_unseparated(")");

        let rows = builder
            .build_query_as::<(String, Option<i64>, Option<i64>)>()
            .fetch_all(self.pool.as_ref())
            .await?;
        rows.into_iter()
            .map(|(thread_id, section_position, section_entered_at_ms)| {
                let thread_id = ThreadId::try_from(thread_id)?;
                let section_entered_at = section_entered_at_ms
                    .map(|millis| {
                        DateTime::<Utc>::from_timestamp_millis(millis).ok_or_else(|| {
                            anyhow::anyhow!("invalid unix timestamp millis: {millis}")
                        })
                    })
                    .transpose()?;
                Ok((thread_id, (section_position, section_entered_at)))
            })
            .collect()
    }

    /// Read an independently persisted thread section by its opaque identifier.
    pub async fn get_thread_section(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<crate::ThreadSection>> {
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT id, name FROM thread_sections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        Ok(row.map(|(id, name)| crate::ThreadSection { id, name }))
    }

    /// List independently persisted sections in stable, cursor-paginated identifier order.
    pub async fn list_thread_sections(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<crate::ThreadSectionsPage> {
        let page_size = limit.max(1);
        let fetch_limit = i64::try_from(page_size.saturating_add(1))?;
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
SELECT id, name
FROM thread_sections
WHERE (? IS NULL OR id > ?)
ORDER BY id
LIMIT ?
            "#,
        )
        .bind(cursor)
        .bind(cursor)
        .bind(fetch_limit)
        .fetch_all(self.pool.as_ref())
        .await?;
        let mut sections = rows
            .into_iter()
            .map(|(id, name)| crate::ThreadSection { id, name })
            .collect::<Vec<_>>();
        let next_cursor = if sections.len() > page_size {
            sections.pop();
            sections.last().map(|section| section.id.clone())
        } else {
            None
        };
        Ok(crate::ThreadSectionsPage {
            sections,
            next_cursor,
        })
    }

    /// Move a thread into or within a section, or clear its section.
    ///
    /// Omitting `before_thread_id` appends the thread to its destination section.
    pub async fn move_thread_to_section(
        &self,
        thread_id: ThreadId,
        section: Option<&str>,
        before_thread_id: Option<ThreadId>,
    ) -> anyhow::Result<bool> {
        if section.is_none() && before_thread_id.is_some() {
            return Err(anyhow::anyhow!(
                "before thread cannot be specified without a section"
            ));
        }

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let thread_id = thread_id.to_string();
        let current_section = sqlx::query_scalar::<_, Option<String>>(
            "SELECT thread_section_id FROM threads WHERE id = ?",
        )
        .bind(&thread_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current_section) = current_section else {
            return Ok(false);
        };
        let Some(section) = section else {
            sqlx::query(
                "UPDATE threads SET thread_section_id = NULL, section_position = NULL, section_entered_at_ms = NULL WHERE id = ?",
            )
            .bind(&thread_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(true);
        };

        if sqlx::query_scalar::<_, i64>("SELECT 1 FROM thread_sections WHERE id = ?")
            .bind(section)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Err(anyhow::anyhow!("section {section} does not exist"));
        }

        let before_thread_id = before_thread_id.map(|id| id.to_string());
        if before_thread_id.as_deref() == Some(thread_id.as_str()) {
            return Err(anyhow::anyhow!(
                "thread {thread_id} cannot be moved before itself"
            ));
        }

        if let Some(before_thread_id) = before_thread_id.as_deref() {
            let before_section = sqlx::query_scalar::<_, Option<String>>(
                "SELECT thread_section_id FROM threads WHERE id = ?",
            )
            .bind(before_thread_id)
            .fetch_optional(&mut *tx)
            .await?;
            if before_section.flatten().as_deref() != Some(section) {
                return Err(anyhow::anyhow!(
                    "before thread {before_thread_id} is not in section {section}"
                ));
            }
        }

        let position =
            section_move_position(&mut tx, section, &thread_id, before_thread_id.as_deref())
                .await?;
        if current_section.as_deref() == Some(section) {
            sqlx::query("UPDATE threads SET section_position = ? WHERE id = ?")
                .bind(position)
                .bind(&thread_id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE threads SET thread_section_id = ?, section_position = ?, section_entered_at_ms = ? WHERE id = ?",
            )
            .bind(section)
            .bind(position)
            .bind(Utc::now().timestamp_millis())
            .bind(&thread_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }
}

async fn section_move_position(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    section: &str,
    thread_id: &str,
    before_thread_id: Option<&str>,
) -> anyhow::Result<i64> {
    let mut renumbered = false;
    loop {
        let position = if let Some(before_thread_id) = before_thread_id {
            let upper = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT section_position FROM threads WHERE id = ? AND thread_section_id = ?",
            )
            .bind(before_thread_id)
            .bind(section)
            .fetch_optional(&mut **tx)
            .await?
            .flatten()
            .ok_or_else(|| {
                anyhow::anyhow!("before thread {before_thread_id} is not in section {section}")
            })?;
            let lower = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT MAX(section_position) FROM threads WHERE thread_section_id = ? AND section_position < ? AND id <> ?",
            )
            .bind(section)
            .bind(upper)
            .bind(thread_id)
            .fetch_one(&mut **tx)
            .await?;
            match lower {
                Some(lower) if i128::from(upper) - i128::from(lower) > 1 => Some(i64::try_from(
                    i128::from(lower) + (i128::from(upper) - i128::from(lower)) / 2,
                )?),
                Some(_) => None,
                None if upper > 1 => Some(upper / 2),
                None => None,
            }
        } else {
            let max_position = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT MAX(section_position) FROM threads WHERE thread_section_id = ? AND id <> ?",
            )
            .bind(section)
            .bind(thread_id)
            .fetch_one(&mut **tx)
            .await?;
            max_position
                .unwrap_or_default()
                .checked_add(SECTION_POSITION_GAP)
        };

        if let Some(position) = position {
            return Ok(position);
        }
        if renumbered {
            return Err(anyhow::anyhow!(
                "section {section} has no remaining thread positions"
            ));
        }
        renumber_section_positions(tx, section, Some(thread_id)).await?;
        renumbered = true;
    }
}

async fn renumber_section_positions(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    section: &str,
    excluded_thread_id: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
UPDATE threads
SET section_position = ranked.position
FROM (
    SELECT id,
           ROW_NUMBER() OVER (ORDER BY section_position ASC, id ASC) * ? AS position
    FROM threads
    WHERE thread_section_id = ? AND (? IS NULL OR id <> ?)
) AS ranked
WHERE threads.id = ranked.id
        "#,
    )
    .bind(SECTION_POSITION_GAP)
    .bind(section)
    .bind(excluded_thread_id)
    .bind(excluded_thread_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "thread_section_order_tests.rs"]
mod tests;
