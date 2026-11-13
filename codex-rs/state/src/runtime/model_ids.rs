//! Session-scoped aliases. SQLite commits each assignment before the model can see it.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use anyhow::Result;
use codex_protocol::SessionId;
use codex_utils_model_facing_id::ModelFacingId;
use codex_utils_model_facing_id::ModelFacingIdGenerator;
use codex_utils_model_facing_id::ModelFacingIdGeneratorState;
use codex_utils_model_facing_id::ModelIdKey;
use codex_utils_model_facing_id::ModelIdMap;
use sqlx::SqliteConnection;
use sqlx::types::Json;

use super::StateRuntime;
use crate::SqlitePoolWriteExt;

pub type ModelIdStoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Stable aliases shared by a session's root and workers. A fork reads the source maps once
/// and installs them in its new session before it can issue an alias of its own.
pub trait SessionModelIdStore: Send + Sync {
    fn lookup(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, Option<ModelFacingId>>;
    fn get_or_assign(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, ModelFacingId>;
    fn snapshot(&self, session: SessionId) -> ModelIdStoreFuture<'_, Vec<ModelIdMap>>;
    fn initialize(&self, session: SessionId, maps: Vec<ModelIdMap>) -> ModelIdStoreFuture<'_, ()>;
}

impl SessionModelIdStore for StateRuntime {
    fn lookup(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, Option<ModelFacingId>> {
        Box::pin(async move {
            let mut conn = self.pool.acquire().await?;
            lookup(&mut conn, &session.to_string(), &key).await
        })
    }

    fn get_or_assign(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, ModelFacingId> {
        Box::pin(async move {
            let session = session.to_string();
            {
                let mut conn = self.pool.acquire().await?;
                if let Some(id) = lookup(&mut conn, &session, &key).await? {
                    return Ok(id);
                }
            }
            // Another writer may have assigned the key since the read, so check again under the lock.
            let mut tx = self.pool.begin_immediate().await?;
            if let Some(id) = lookup(&mut tx, &session, &key).await? {
                tx.commit().await?;
                return Ok(id);
            }
            let initial: Option<Json<ModelFacingIdGeneratorState>> = sqlx::query_scalar(
                "SELECT initial_state FROM session_model_ids WHERE session_id=? AND kind=? AND ordinal=0",
            )
            .bind(&session)
            .bind(key.kind)
            .fetch_optional(&mut *tx)
            .await?;
            let first = initial.is_none();
            let initial = initial.unwrap_or_else(|| {
                Json(ModelFacingIdGenerator::new(rand::random(), key.kind).snapshot())
            });
            let ordinal: i64 = sqlx::query_scalar(
                "INSERT INTO session_model_ids (session_id, kind, external_key, ordinal, initial_state)
                 SELECT ?1, ?2, ?3, COALESCE(MAX(ordinal)+1, 0), ?4 FROM session_model_ids
                 WHERE session_id=?1 AND kind=?2
                 RETURNING ordinal",
            )
            .bind(&session)
            .bind(key.kind)
            .bind(key.key)
            .bind(first.then_some(&initial))
            .fetch_one(&mut *tx)
            .await?;
            let id = initial.id_at(ordinal as u64)?;
            tx.commit().await?;
            Ok(id)
        })
    }

    fn snapshot(&self, session: SessionId) -> ModelIdStoreFuture<'_, Vec<ModelIdMap>> {
        Box::pin(async move {
            let rows: Vec<(Json<ModelFacingIdGeneratorState>, Json<Vec<String>>)> = sqlx::query_as(
                "SELECT MAX(initial_state), json_group_array(external_key ORDER BY ordinal)
                 FROM session_model_ids WHERE session_id=? GROUP BY kind ORDER BY kind",
            )
            .bind(session.to_string())
            .fetch_all(self.pool.as_ref())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(Json(initial), Json(keys))| ModelIdMap { initial, keys })
                .collect())
        })
    }

    fn initialize(&self, session: SessionId, maps: Vec<ModelIdMap>) -> ModelIdStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut tx = self.pool.begin_immediate().await?;
            let session = session.to_string();
            for map in maps {
                sqlx::query(
                    "INSERT INTO session_model_ids (session_id, kind, external_key, ordinal, initial_state)
                     SELECT ?1, ?2, value, key, CASE WHEN key = 0 THEN ?3 END FROM json_each(?4)",
                )
                .bind(&session)
                .bind(map.initial.salt())
                .bind(Json(&map.initial))
                .bind(Json(map.keys))
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            Ok(())
        })
    }
}

async fn lookup(
    conn: &mut SqliteConnection,
    session: &str,
    key: &ModelIdKey,
) -> Result<Option<ModelFacingId>> {
    let row: Option<(Json<ModelFacingIdGeneratorState>, i64)> = sqlx::query_as(
        "SELECT first.initial_state, b.ordinal FROM session_model_ids b
         JOIN session_model_ids first USING (session_id, kind)
         WHERE b.session_id=? AND b.kind=? AND b.external_key=? AND first.ordinal=0",
    )
    .bind(session)
    .bind(key.kind)
    .bind(&key.key)
    .fetch_optional(conn)
    .await?;
    let Some((initial, ordinal)) = row else {
        return Ok(None);
    };
    Ok(Some(initial.id_at(ordinal as u64)?))
}

/// Remove the names with the rest of the session state in the caller's delete transaction.
pub(super) async fn delete_session(conn: &mut SqliteConnection, session: &str) -> Result<()> {
    sqlx::query("DELETE FROM session_model_ids WHERE session_id = ?")
        .bind(session)
        .execute(conn)
        .await?;
    Ok(())
}

/// Uses the same namespace and fork semantics for ephemeral sessions and in-memory thread stores.
#[derive(Default, Debug)]
pub struct MemoryModelIdStore(Mutex<HashMap<SessionId, Vec<ModelIdMap>>>);

impl MemoryModelIdStore {
    pub fn remove(&self, session: SessionId) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&session);
    }
}

impl SessionModelIdStore for MemoryModelIdStore {
    fn lookup(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, Option<ModelFacingId>> {
        Box::pin(async move {
            let sessions = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(map) = sessions
                .get(&session)
                .and_then(|maps| maps.iter().find(|map| map.initial.salt() == key.kind))
            else {
                return Ok(None);
            };
            Ok(map.lookup(&key.key)?)
        })
    }

    fn get_or_assign(
        &self,
        session: SessionId,
        key: ModelIdKey,
    ) -> ModelIdStoreFuture<'_, ModelFacingId> {
        Box::pin(async move {
            let mut sessions = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let maps = sessions.entry(session).or_default();
            let map = match maps.iter().position(|map| map.initial.salt() == key.kind) {
                Some(index) => &mut maps[index],
                None => maps.push_mut(ModelIdMap::new(rand::random(), key.kind)),
            };
            if let Some(id) = map.lookup(&key.key)? {
                return Ok(id);
            }
            let id = map.initial.id_at(map.keys.len() as u64)?;
            map.keys.push(key.key);
            Ok(id)
        })
    }

    fn snapshot(&self, session: SessionId) -> ModelIdStoreFuture<'_, Vec<ModelIdMap>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&session)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn initialize(&self, session: SessionId, maps: Vec<ModelIdMap>) -> ModelIdStoreFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(session, maps);
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "model_ids_tests.rs"]
mod tests;
