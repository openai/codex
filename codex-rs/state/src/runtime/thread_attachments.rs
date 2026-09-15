//! Transactional thread attachment storage using the attachment SQL schema.

use super::StateRuntime;
use crate::AddThreadAttachmentOutcome;
use crate::MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES;
use crate::MAX_THREAD_ATTACHMENT_LIST_PAGE_SIZE;
use crate::MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES;
use crate::MAX_THREAD_ATTACHMENT_TYPE_BYTES;
use crate::MAX_THREAD_ATTACHMENTS_PER_THREAD;
use crate::RemoveThreadAttachmentOutcome;
use crate::ThreadAttachment;
use crate::ThreadAttachmentPage;
use anyhow::Context;
use chrono::Utc;
use codex_protocol::ThreadId;
use serde_json::Value;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

impl StateRuntime {
    /// Atomically copies current membership into a new, empty fork, independent of history cutoffs.
    pub async fn copy_thread_attachments(
        &self,
        source_thread_id: ThreadId,
        destination_thread_id: ThreadId,
    ) -> anyhow::Result<()> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let destination = destination_thread_id.to_string();
        let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM threads WHERE id = ?")
            .bind(&destination)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !exists {
            anyhow::bail!("thread not found: {destination_thread_id}");
        }
        let rows = sqlx::query(
            "SELECT attachment_type, identity_key, payload FROM thread_attachments WHERE thread_id = ? ORDER BY created_at, id",
        )
        .bind(source_thread_id.to_string())
        .fetch_all(&mut *transaction)
        .await?;
        let created_at = Utc::now().timestamp();
        for row in rows {
            sqlx::query(
                "INSERT INTO thread_attachments (id, thread_id, attachment_type, identity_key, payload, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(&destination)
            .bind(row.try_get::<String, _>("attachment_type")?)
            .bind(row.try_get::<String, _>("identity_key")?)
            .bind(row.try_get::<String, _>("payload")?)
            .bind(created_at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Attach an attachment once, returning an existing attachment for repeated requests.
    pub async fn add_thread_attachment(
        &self,
        thread_id: ThreadId,
        attachment_type: &str,
        identity_key: &str,
        payload: &Value,
    ) -> anyhow::Result<AddThreadAttachmentOutcome> {
        validate_attachment_identity(attachment_type, identity_key)?;
        let serialized_payload = serde_json::to_string(payload).context(
            "invalid thread attachment request: attachment payload cannot be serialized",
        )?;
        if serialized_payload.len() > MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES {
            anyhow::bail!(
                "invalid thread attachment request: attachment payload exceeds {MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES} bytes"
            );
        }

        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let thread_id_string = thread_id.to_string();
        let thread_exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM threads WHERE id = ?")
            .bind(&thread_id_string)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !thread_exists {
            anyhow::bail!("thread not found: {thread_id}");
        }

        let existing = sqlx::query(
            "SELECT id, thread_id, attachment_type, identity_key, payload, created_at FROM thread_attachments WHERE thread_id = ? AND attachment_type = ? AND identity_key = ?",
        )
        .bind(&thread_id_string)
        .bind(attachment_type)
        .bind(identity_key)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(existing) = existing {
            let attachment = attachment_from_row(&existing)?;
            transaction.commit().await?;
            return Ok(AddThreadAttachmentOutcome::Existing(attachment));
        }

        let identity_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM thread_attachments WHERE thread_id = ?",
        )
        .bind(&thread_id_string)
        .fetch_one(&mut *transaction)
        .await?;
        if usize::try_from(identity_count)? >= MAX_THREAD_ATTACHMENTS_PER_THREAD {
            anyhow::bail!(
                "invalid thread attachment request: thread attachment identity count exceeds {MAX_THREAD_ATTACHMENTS_PER_THREAD}"
            );
        }

        let attachment = ThreadAttachment {
            id: Uuid::now_v7().to_string(),
            thread_id,
            attachment_type: attachment_type.to_string(),
            identity_key: identity_key.to_string(),
            payload: payload.clone(),
            created_at: Utc::now().timestamp(),
        };
        sqlx::query(
            "INSERT INTO thread_attachments (id, thread_id, attachment_type, identity_key, payload, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&attachment.id)
        .bind(&thread_id_string)
        .bind(attachment_type)
        .bind(identity_key)
        .bind(&serialized_payload)
        .bind(attachment.created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(AddThreadAttachmentOutcome::Created(attachment))
    }

    /// Remove an attached attachment, immediately freeing its slot.
    pub async fn remove_thread_attachment(
        &self,
        thread_id: ThreadId,
        attachment_type: &str,
        identity_key: &str,
    ) -> anyhow::Result<RemoveThreadAttachmentOutcome> {
        validate_attachment_identity(attachment_type, identity_key)?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let thread_id_string = thread_id.to_string();
        let thread_exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM threads WHERE id = ?")
            .bind(&thread_id_string)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !thread_exists {
            anyhow::bail!("thread not found: {thread_id}");
        }

        let removed = sqlx::query(
            "DELETE FROM thread_attachments WHERE thread_id = ? AND attachment_type = ? AND identity_key = ? RETURNING id, thread_id, attachment_type, identity_key, payload, created_at",
        )
        .bind(&thread_id_string)
        .bind(attachment_type)
        .bind(identity_key)
        .fetch_optional(&mut *transaction)
        .await?;
        let outcome = match removed {
            Some(row) => RemoveThreadAttachmentOutcome::Removed(attachment_from_row(&row)?),
            None => RemoveThreadAttachmentOutcome::NotFound,
        };
        transaction.commit().await?;
        Ok(outcome)
    }

    /// List one bounded page of attachments for one thread in stable keyset order.
    pub async fn list_thread_attachments(
        &self,
        thread_id: ThreadId,
        cursor: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<ThreadAttachmentPage> {
        if !(1..=MAX_THREAD_ATTACHMENT_LIST_PAGE_SIZE).contains(&limit) {
            anyhow::bail!(
                "invalid thread attachment request: page limit must be between 1 and {MAX_THREAD_ATTACHMENT_LIST_PAGE_SIZE}"
            );
        }

        let thread_id_string = thread_id.to_string();
        let anchor = cursor.map(parse_attachment_cursor).transpose()?;
        if let Some((cursor_thread_id, _, _)) = anchor.as_ref()
            && cursor_thread_id != &thread_id_string
        {
            anyhow::bail!("invalid thread attachment request: invalid pagination cursor");
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT id, thread_id, attachment_type, identity_key, payload, created_at FROM thread_attachments WHERE thread_id = ",
        );
        query.push_bind(thread_id_string);
        if let Some((_, created_at, attachment_id)) = anchor {
            query.push(" AND (created_at, id) > (");
            query.push_bind(created_at);
            query.push(", ");
            query.push_bind(attachment_id);
            query.push(")");
        }
        query.push(" ORDER BY created_at ASC, id ASC LIMIT ");
        query.push_bind(i64::try_from(limit + 1)?);
        let rows = query.build().fetch_all(self.pool.as_ref()).await?;
        let mut attachments = rows
            .iter()
            .map(attachment_from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor = if attachments.len() > limit {
            attachments.pop();
            attachments.last().map(|attachment| {
                format!(
                    "{}|{}|{}",
                    attachment.thread_id, attachment.created_at, attachment.id
                )
            })
        } else {
            None
        };
        Ok(ThreadAttachmentPage {
            attachments,
            next_cursor,
        })
    }
}

fn validate_attachment_identity(attachment_type: &str, identity_key: &str) -> anyhow::Result<()> {
    if attachment_type.trim().is_empty() {
        anyhow::bail!("invalid thread attachment request: attachment type must not be empty");
    }
    if attachment_type.len() > MAX_THREAD_ATTACHMENT_TYPE_BYTES {
        anyhow::bail!(
            "invalid thread attachment request: attachment type exceeds {MAX_THREAD_ATTACHMENT_TYPE_BYTES} bytes"
        );
    }
    if identity_key.trim().is_empty() {
        anyhow::bail!(
            "invalid thread attachment request: attachment identity key must not be empty"
        );
    }
    if identity_key.len() > MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES {
        anyhow::bail!(
            "invalid thread attachment request: attachment identity key exceeds {MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES} bytes"
        );
    }
    Ok(())
}

fn parse_attachment_cursor(cursor: &str) -> anyhow::Result<(String, i64, String)> {
    let mut segments = cursor.split('|');
    let (Some(thread_id), Some(created_at), Some(attachment_id), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        anyhow::bail!("invalid thread attachment request: invalid pagination cursor");
    };
    if ThreadId::from_string(thread_id).is_err() || Uuid::parse_str(attachment_id).is_err() {
        anyhow::bail!("invalid thread attachment request: invalid pagination cursor");
    }
    let created_at = created_at
        .parse::<i64>()
        .context("invalid thread attachment request: invalid pagination cursor")?;
    Ok((thread_id.to_string(), created_at, attachment_id.to_string()))
}

fn attachment_from_row(row: &SqliteRow) -> anyhow::Result<ThreadAttachment> {
    let thread_id: String = row.try_get("thread_id")?;
    let payload: String = row.try_get("payload")?;
    Ok(ThreadAttachment {
        id: row.try_get("id")?,
        thread_id: ThreadId::from_string(&thread_id)
            .context("invalid persisted thread attachment owner")?,
        attachment_type: row.try_get("attachment_type")?,
        identity_key: row.try_get("identity_key")?,
        payload: serde_json::from_str(&payload)
            .context("invalid persisted thread attachment payload")?,
        created_at: row.try_get("created_at")?,
    })
}

#[cfg(test)]
#[path = "thread_attachments_tests.rs"]
mod tests;
