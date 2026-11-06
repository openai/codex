//! Atomic replacement and discovery of a caller's own channel subscriptions.
//! Thread subscriptions and other agents are unaffected; no notifications are emitted.

use super::LocalAgentMessageBoard;
use super::invalid;
use super::storage_error;
use super::target_key;
use super::validate_channel;
use crate::SubscriptionChange;
use crate::SubscriptionTarget;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use sqlx::Sqlite;
use sqlx::Transaction;
use std::collections::BTreeMap;

impl LocalAgentMessageBoard {
    /// Atomically replaces the caller's membership in the supplied channels.
    /// Channels outside this set keep their subscriptions and opt-outs.
    /// A read-only live connection permits this self-only operation.
    pub(super) async fn apply_subscriptions(
        &self,
        caller: ThreadId,
        channels: &BTreeMap<&str, SubscriptionChange>,
    ) -> Result<()> {
        self.host.agent_path(caller).await?;
        for channel in channels.keys() {
            validate_channel(channel)?;
        }
        let names =
            serde_json::to_string(&channels.keys().collect::<Vec<_>>()).map_err(storage_error)?;
        let mut tx = self.begin_write().await?;
        let membership = sqlx::query_as::<_, (String, bool)>(
            "SELECT c.name, EXISTS(
                SELECT 1 FROM subscriptions AS s WHERE s.board=c.board
                AND s.target=json_object('Channel',c.name) AND s.agent=?
             ) FROM channels AS c WHERE c.board=? AND c.name IN (SELECT value FROM json_each(?))",
        )
        .bind(caller.to_string())
        .bind(self.identity.to_string())
        .bind(names)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage_error)?;
        if membership.len() != channels.len() {
            return Err(invalid("channel not found in this board"));
        }
        for (name, subscribed) in membership {
            let change = channels[name.as_str()];
            if subscribed == (change == SubscriptionChange::Subscribe) {
                continue;
            }
            self.change_subscription(&mut tx, &SubscriptionTarget::Channel(name), caller, change)
                .await?;
        }
        tx.commit().await.map_err(storage_error)
    }

    /// Update explicit membership and its matching implicit-subscription opt-out atomically.
    pub(super) async fn change_subscription(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        target: &SubscriptionTarget,
        agent: ThreadId,
        change: SubscriptionChange,
    ) -> Result<()> {
        let board = self.identity.to_string();
        let target = target_key(target)?;
        let agent = agent.to_string();
        let statements = match change {
            SubscriptionChange::Subscribe => [
                "DELETE FROM subscription_opt_outs WHERE board=? AND target=? AND agent=?",
                "INSERT OR IGNORE INTO subscriptions(board,target,agent) VALUES(?,?,?)",
            ],
            SubscriptionChange::Unsubscribe => [
                "DELETE FROM subscriptions WHERE board=? AND target=? AND agent=?",
                "INSERT OR IGNORE INTO subscription_opt_outs(board,target,agent) VALUES(?,?,?)",
            ],
        };
        for statement in statements {
            sqlx::query(statement)
                .bind(&board)
                .bind(&target)
                .bind(&agent)
                .execute(&mut **tx)
                .await
                .map_err(storage_error)?;
        }
        Ok(())
    }
}
