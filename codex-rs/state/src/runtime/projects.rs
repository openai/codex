//! Persists projects and assignments, and lists projects with computed member recency.
//! Listing pagination precedes root hydration; cursor anchors preserve milliseconds and nulls.

use std::collections::BTreeMap;

use codex_protocol::ThreadId;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use uuid::Uuid;

use super::StateRuntime;
use crate::CreatedProject;
use crate::Project;
use crate::ProjectRoot;
use crate::ProjectSortKey;
use crate::ProjectsPage;
use crate::SortDirection;
use crate::sqlite::SqlitePoolWriteExt;

const PROJECT_SELECT: &str = "SELECT projects.*,
    (SELECT MAX(recency_at_ms) FROM threads
     WHERE project_id = projects.id AND archived = 0) AS recency_at_ms
    FROM projects";

impl StateRuntime {
    /// Use the staged project only until the thread has a row in SQLite.
    pub async fn thread_has_project(
        &self,
        thread_id: ThreadId,
        staged_project_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar(
            "SELECT COALESCE(
                (SELECT project_id IS NOT NULL FROM threads WHERE id = ?),
                EXISTS (SELECT 1 FROM projects WHERE id = ?)
            )",
        )
        .bind(thread_id.to_string())
        .bind(staged_project_id)
        .fetch_one(self.pool.as_ref())
        .await?)
    }

    pub async fn set_thread_project(
        &self,
        thread_id: &str,
        project_id: Option<&str>,
    ) -> anyhow::Result<Option<Option<String>>> {
        let mut tx = self.pool.begin_immediate().await?;
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
        sort_key: ProjectSortKey,
        sort_direction: SortDirection,
    ) -> anyhow::Result<ProjectsPage> {
        let mut query = project_list_query(cursor, limit, sort_key, sort_direction)?;
        let rows = query.build().fetch_all(self.pool.as_ref()).await?;
        let mut projects: Vec<Project> = Vec::new();
        for row in rows {
            let id: String = row.try_get("id")?;
            if projects.last().is_none_or(|project| project.id != id) {
                projects.push(project_from_row(&row, Vec::new())?);
            }
            if let Some(path) = row.try_get::<Option<String>, _>("root_path")? {
                let project = projects
                    .last_mut()
                    .ok_or_else(|| anyhow::anyhow!("project missing for root"))?;
                project.roots.push(ProjectRoot { path });
            }
        }
        let next_cursor = (projects.len() > limit)
            .then(|| project_cursor(&projects[limit - 1], sort_key, sort_direction));
        projects.truncate(limit);
        Ok(ProjectsPage {
            projects,
            next_cursor,
        })
    }

    pub async fn get_project(&self, id: &str) -> anyhow::Result<Option<Project>> {
        let mut tx = self.pool.begin().await?;
        let row = QueryBuilder::<Sqlite>::new(PROJECT_SELECT)
            .push(" WHERE id = ")
            .push_bind(id)
            .build()
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
        let row = QueryBuilder::<Sqlite>::new(PROJECT_SELECT)
            .push(" WHERE id = ")
            .push_bind(&project_id)
            .build()
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
        let mut tx = self.pool.begin_immediate().await?;
        let existing_project_id = sqlx::query_scalar::<_, String>(
            "SELECT project_id FROM project_idempotency_keys WHERE key = ?",
        )
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_project_id) = existing_project_id {
            let row = QueryBuilder::<Sqlite>::new(PROJECT_SELECT)
                .push(" WHERE id = ")
                .push_bind(&existing_project_id)
                .build()
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
        let row = QueryBuilder::<Sqlite>::new(PROJECT_SELECT)
            .push(" WHERE id = ")
            .push_bind(&id)
            .build()
            .fetch_one(&mut *tx)
            .await?;
        let project = project_from_row(&row, roots)?;
        tx.commit().await?;
        Ok(CreatedProject {
            project,
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
        let mut tx = self.pool.begin_immediate().await?;
        let row = QueryBuilder::<Sqlite>::new(PROJECT_SELECT)
            .push(" WHERE id = ")
            .push_bind(id)
            .build()
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
                recency_at_ms: current.recency_at_ms,
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
        let mut tx = self.pool.begin_immediate().await?;
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
        let mut tx = self.pool.begin_immediate().await?;
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
        recency_at_ms: row.try_get("recency_at_ms")?,
    })
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

fn project_list_query(
    cursor: Option<&str>,
    limit: usize,
    sort_key: ProjectSortKey,
    sort_direction: SortDirection,
) -> anyhow::Result<QueryBuilder<Sqlite>> {
    anyhow::ensure!(limit > 0, "project limit must be positive");
    let query_limit = i64::try_from(limit)?
        .checked_add(/*rhs*/ 1)
        .ok_or_else(|| anyhow::anyhow!("project limit overflow"))?;
    let column = match sort_key {
        ProjectSortKey::Position => "p.position",
        ProjectSortKey::RecencyAt => "p.recency_at_ms",
    };
    let operator = match sort_direction {
        SortDirection::Asc => ">",
        SortDirection::Desc => "<",
    };
    let mut query = QueryBuilder::new(format!(
        "WITH project_activity AS ({PROJECT_SELECT}), page AS (SELECT * FROM project_activity p"
    ));
    if let Some(cursor) = cursor {
        let (value, id) = parse_project_cursor(cursor, sort_key, sort_direction)?;
        if let Some(value) = value {
            query
                .push(format!(" WHERE ({column} {operator} "))
                .push_bind(value);
            query.push(format!(" OR ({column} = ")).push_bind(value);
            query
                .push(format!(" AND p.id {operator} "))
                .push_bind(id)
                .push(")");
            if sort_key == ProjectSortKey::RecencyAt {
                query.push(" OR p.recency_at_ms IS NULL");
            }
            query.push(")");
        } else {
            query
                .push(format!(
                    " WHERE p.recency_at_ms IS NULL AND p.id {operator} "
                ))
                .push_bind(id);
        }
    }
    push_project_order(&mut query, sort_key, sort_direction);
    query.push(" LIMIT ").push_bind(query_limit);
    query.push(
        ") SELECT p.*, roots.path AS root_path FROM page p \
        LEFT JOIN project_roots roots ON roots.project_id = p.id",
    );
    push_project_order(&mut query, sort_key, sort_direction);
    query.push(", roots.position ASC");
    Ok(query)
}

fn push_project_order(
    query: &mut QueryBuilder<Sqlite>,
    sort_key: ProjectSortKey,
    sort_direction: SortDirection,
) {
    let direction = match sort_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    query.push(" ORDER BY ");
    match sort_key {
        ProjectSortKey::Position => query.push("p.position"),
        ProjectSortKey::RecencyAt => query.push("p.recency_at_ms IS NULL ASC, p.recency_at_ms"),
    };
    query.push(format!(" {direction}, p.id {direction}"));
}

fn project_cursor(project: &Project, sort_key: ProjectSortKey, direction: SortDirection) -> String {
    if sort_key == ProjectSortKey::Position && direction == SortDirection::Asc {
        // Retain the existing format for clients reconnecting to an older server.
        return format!("{}|{}", project.position, project.id);
    }
    let (key, value) = match sort_key {
        ProjectSortKey::Position => ("position", project.position.to_string()),
        ProjectSortKey::RecencyAt => (
            "recencyAt",
            project
                .recency_at_ms
                .map_or_else(|| "null".to_string(), |value| value.to_string()),
        ),
    };
    let direction = match direction {
        SortDirection::Asc => "asc",
        SortDirection::Desc => "desc",
    };
    format!("v1|{key}|{direction}|{value}|{}", project.id)
}

fn parse_project_cursor(
    cursor: &str,
    sort_key: ProjectSortKey,
    direction: SortDirection,
) -> anyhow::Result<(Option<i64>, String)> {
    let invalid = || anyhow::anyhow!("invalid project cursor: malformed or mismatched sort anchor");
    if cursor.len() > 128 {
        return Err(invalid());
    }
    let parts: Vec<_> = cursor.split('|').collect();
    let key = match sort_key {
        ProjectSortKey::Position => "position",
        ProjectSortKey::RecencyAt => "recencyAt",
    };
    let order = match direction {
        SortDirection::Asc => "asc",
        SortDirection::Desc => "desc",
    };
    let (value, id) = match parts.as_slice() {
        [value, id] if sort_key == ProjectSortKey::Position && direction == SortDirection::Asc => {
            (*value, *id)
        }
        ["v1", cursor_key, cursor_order, value, id]
            if *cursor_key == key && *cursor_order == order =>
        {
            (*value, *id)
        }
        _ => return Err(invalid()),
    };
    let value = if value == "null" && sort_key == ProjectSortKey::RecencyAt {
        None
    } else {
        let parsed: i64 = value.parse().map_err(|_| invalid())?;
        if parsed.to_string() != value || sort_key == ProjectSortKey::Position && parsed < 0 {
            return Err(invalid());
        }
        Some(parsed)
    };
    let uuid = Uuid::parse_str(id).map_err(|_| invalid())?;
    if uuid.to_string() != id {
        return Err(invalid());
    }
    Ok((value, id.to_string()))
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
