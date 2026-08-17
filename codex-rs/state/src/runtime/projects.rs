use std::collections::BTreeMap;

use sqlx::Row;
use uuid::Uuid;

use super::StateRuntime;
use crate::CreatedProject;
use crate::Project;
use crate::ProjectRoot;
use crate::ProjectsPage;

impl StateRuntime {
    pub async fn set_thread_project(
        &self,
        thread_id: &str,
        project_id: Option<&str>,
    ) -> anyhow::Result<Option<Option<String>>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(project_id) = project_id {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_one(&mut *tx)
                .await?;
            if exists == 0 {
                tx.rollback().await?;
                anyhow::bail!("project not found: {project_id}");
            }
        }
        let previous =
            sqlx::query_scalar::<_, Option<String>>("SELECT project_id FROM threads WHERE id = ?")
                .bind(thread_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(previous) = previous else {
            tx.rollback().await?;
            return Ok(None);
        };
        if previous.as_deref() != project_id {
            sqlx::query("UPDATE threads SET project_id = ? WHERE id = ?")
                .bind(project_id)
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Some(previous))
    }

    pub async fn list_projects(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<ProjectsPage> {
        let anchor = cursor.map(parse_project_cursor).transpose()?;
        let mut tx = self.pool.begin().await?;
        let rows = if let Some((position, id)) = anchor {
            sqlx::query(
                "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects WHERE position > ? OR (position = ? AND id > ?) ORDER BY position ASC, id ASC LIMIT ?",
            )
            .bind(position)
            .bind(position)
            .bind(id)
            .bind((limit + 1) as i64)
            .fetch_all(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects ORDER BY position ASC, id ASC LIMIT ?",
            )
            .bind((limit + 1) as i64)
            .fetch_all(&mut *tx)
            .await?
        };
        let project_ids = rows
            .iter()
            .map(|row| row.try_get("id"))
            .collect::<Result<Vec<String>, _>>()?;
        let mut roots_by_project_id = project_roots_by_id_in_tx(&mut tx, &project_ids).await?;
        let mut projects = Vec::with_capacity(rows.len());
        for row in rows {
            let project_id: String = row.try_get("id")?;
            let roots = roots_by_project_id.remove(&project_id).unwrap_or_default();
            projects.push(project_from_row(&row, roots)?);
        }
        let next_cursor = (projects.len() > limit)
            .then(|| project_cursor(&projects[limit - 1]))
            .flatten();
        projects.truncate(limit);
        tx.commit().await?;
        Ok(ProjectsPage {
            projects,
            next_cursor,
        })
    }

    pub async fn get_project(&self, id: &str) -> anyhow::Result<Option<Project>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let project = match row {
            Some(row) => Some(project_from_row_in_tx(&mut tx, &row).await?),
            None => None,
        };
        tx.commit().await?;
        Ok(project)
    }

    pub async fn get_project_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<Project>> {
        let mut tx = self.pool.begin().await?;
        let project_id = sqlx::query_scalar::<_, String>(
            "SELECT project_id FROM project_idempotency_keys WHERE key = ?",
        )
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(project_id) = project_id else {
            tx.commit().await?;
            return Ok(None);
        };
        let row = sqlx::query(
            "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects WHERE id = ?",
        )
        .bind(&project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            anyhow::bail!("idempotency key refers to deleted project: {idempotency_key}");
        };
        let project = project_from_row_in_tx(&mut tx, &row).await?;
        tx.commit().await?;
        Ok(Some(project))
    }

    pub async fn create_project(
        &self,
        name: String,
        roots: Vec<ProjectRoot>,
        metadata: BTreeMap<String, String>,
        thread_ids: &[String],
        idempotency_key: &str,
    ) -> anyhow::Result<CreatedProject> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let existing_project_id = sqlx::query_scalar::<_, String>(
            "SELECT project_id FROM project_idempotency_keys WHERE key = ?",
        )
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_project_id) = existing_project_id {
            let row = sqlx::query(
                "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects WHERE id = ?",
            )
            .bind(&existing_project_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(row) = row else {
                tx.rollback().await?;
                anyhow::bail!("idempotency key refers to deleted project: {idempotency_key}");
            };
            let project = project_from_row_in_tx(&mut tx, &row).await?;
            tx.commit().await?;
            return Ok(CreatedProject {
                project,
                created: false,
            });
        }
        for thread_id in thread_ids {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE id = ?")
                .bind(thread_id)
                .fetch_one(&mut *tx)
                .await?;
            if exists == 0 {
                anyhow::bail!("thread not found: {thread_id}");
            }
        }
        let id = Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let position = sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(position) FROM projects")
            .fetch_one(&mut *tx)
            .await?
            .unwrap_or(-1)
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("project position overflow"))?;
        let metadata_json = serde_json::to_string(&metadata)?;
        sqlx::query(
            "INSERT INTO projects (id, name, metadata, position, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&metadata_json)
        .bind(position)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        replace_roots(&mut tx, &id, &roots).await?;
        for thread_id in thread_ids {
            sqlx::query("UPDATE threads SET project_id = ? WHERE id = ?")
                .bind(&id)
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO project_idempotency_keys (key, project_id, created_at_ms) VALUES (?, ?, ?)",
        )
        .bind(idempotency_key)
        .bind(&id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(CreatedProject {
            project: Project {
                id,
                name,
                roots,
                metadata,
                position,
                created_at_ms: now,
                updated_at_ms: now,
            },
            created: true,
        })
    }

    pub async fn update_project(
        &self,
        id: &str,
        name: Option<String>,
        roots: Option<Vec<ProjectRoot>>,
        metadata: Option<BTreeMap<String, String>>,
    ) -> anyhow::Result<Option<(Project, bool)>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT id, name, metadata, position, created_at_ms, updated_at_ms FROM projects WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(None);
        };
        let current = project_from_row_in_tx(&mut tx, &row).await?;
        let next_name = name.unwrap_or_else(|| current.name.clone());
        let next_roots = roots.unwrap_or_else(|| current.roots.clone());
        let next_metadata = metadata.unwrap_or_else(|| current.metadata.clone());
        if next_name == current.name
            && next_roots == current.roots
            && next_metadata == current.metadata
        {
            tx.rollback().await?;
            return Ok(Some((current, false)));
        }
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("UPDATE projects SET name = ?, metadata = ?, updated_at_ms = ? WHERE id = ?")
            .bind(&next_name)
            .bind(serde_json::to_string(&next_metadata)?)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if next_roots != current.roots {
            replace_roots(&mut tx, id, &next_roots).await?;
        }
        tx.commit().await?;
        Ok(Some((
            Project {
                id: id.to_string(),
                name: next_name,
                roots: next_roots,
                metadata: next_metadata,
                position: current.position,
                created_at_ms: current.created_at_ms,
                updated_at_ms: now,
            },
            true,
        )))
    }

    /// Move a project before another project, or append it when the anchor is absent.
    ///
    /// Returns None when the moved project does not exist and Some(false) for a no-op.
    pub async fn move_project(
        &self,
        project_id: &str,
        before_project_id: Option<&str>,
    ) -> anyhow::Result<Option<bool>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut project_ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM projects ORDER BY position ASC, id ASC",
        )
        .fetch_all(&mut *tx)
        .await?;
        let Some(current_index) = project_ids.iter().position(|id| id == project_id) else {
            tx.rollback().await?;
            return Ok(None);
        };
        if before_project_id == Some(project_id) {
            tx.rollback().await?;
            anyhow::bail!("project {project_id} cannot be moved before itself");
        }
        let original_project_ids = project_ids.clone();
        project_ids.remove(current_index);
        let next_index = if let Some(before_project_id) = before_project_id {
            project_ids
                .iter()
                .position(|id| id == before_project_id)
                .ok_or_else(|| anyhow::anyhow!("before project not found: {before_project_id}"))?
        } else {
            project_ids.len()
        };
        project_ids.insert(next_index, project_id.to_string());
        if project_ids == original_project_ids {
            tx.rollback().await?;
            return Ok(Some(false));
        }
        for (position, id) in project_ids.iter().enumerate() {
            sqlx::query("UPDATE projects SET position = ? WHERE id = ?")
                .bind(position as i64)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE projects SET updated_at_ms = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp_millis())
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(true))
    }

    pub async fn delete_project(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<(Vec<String>, Vec<String>)>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if exists == 0 {
            tx.rollback().await?;
            return Ok(None);
        }
        let active_thread_ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM threads WHERE project_id = ? AND archived = 0 ORDER BY id ASC",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let archived_thread_ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM threads WHERE project_id = ? AND archived = 1 ORDER BY id ASC",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("UPDATE threads SET project_id = NULL WHERE project_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some((active_thread_ids, archived_thread_ids)))
    }
}

async fn project_from_row_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<Project> {
    let id: String = row.try_get("id")?;
    let roots =
        sqlx::query("SELECT path FROM project_roots WHERE project_id = ? ORDER BY position ASC")
            .bind(&id)
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|row| {
                Ok(ProjectRoot {
                    path: row.try_get("path")?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
    project_from_row(row, roots)
}

fn project_from_row(
    row: &sqlx::sqlite::SqliteRow,
    roots: Vec<ProjectRoot>,
) -> anyhow::Result<Project> {
    Ok(Project {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        roots,
        metadata: serde_json::from_str(&row.try_get::<String, _>("metadata")?)?,
        position: row.try_get("position")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

async fn project_roots_by_id_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_ids: &[String],
) -> anyhow::Result<BTreeMap<String, Vec<ProjectRoot>>> {
    if project_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT project_id, path FROM project_roots WHERE project_id IN (",
    );
    let mut separated = builder.separated(", ");
    for project_id in project_ids {
        separated.push_bind(project_id);
    }
    builder.push(") ORDER BY project_id ASC, position ASC");
    let rows = builder.build().fetch_all(&mut **tx).await?;
    let mut roots_by_project_id = BTreeMap::<String, Vec<ProjectRoot>>::new();
    for row in rows {
        roots_by_project_id
            .entry(row.try_get("project_id")?)
            .or_default()
            .push(ProjectRoot {
                path: row.try_get("path")?,
            });
    }
    Ok(roots_by_project_id)
}

async fn replace_roots(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    roots: &[ProjectRoot],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM project_roots WHERE project_id = ?")
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    for (position, root) in roots.iter().enumerate() {
        sqlx::query("INSERT INTO project_roots (project_id, position, path) VALUES (?, ?, ?)")
            .bind(project_id)
            .bind(position as i64)
            .bind(&root.path)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn project_cursor(project: &Project) -> Option<String> {
    Some(format!("{}|{}", project.position, project.id))
}

fn parse_project_cursor(cursor: &str) -> anyhow::Result<(i64, String)> {
    let mut parts = cursor.split('|');
    let position_component = parts.next().ok_or_else(|| invalid_project_cursor(cursor))?;
    let position: i64 = position_component
        .parse()
        .map_err(|_| invalid_project_cursor(cursor))?;
    let id = parts.next().ok_or_else(|| invalid_project_cursor(cursor))?;
    let uuid = Uuid::parse_str(id).map_err(|_| invalid_project_cursor(cursor))?;
    if parts.next().is_some()
        || position < 0
        || position.to_string() != position_component
        || uuid.to_string() != id
    {
        return Err(invalid_project_cursor(cursor));
    }
    Ok((position, id.to_string()))
}

fn invalid_project_cursor(cursor: &str) -> anyhow::Error {
    anyhow::anyhow!("invalid project cursor: {cursor}")
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
