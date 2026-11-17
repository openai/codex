//! Update explicit membership and its opt-out together for one subscription target.

use super::LocalAgentMessageBoard;
use super::storage_error;
use super::target_key;
use crate::SubscriptionChange;
use crate::SubscriptionTarget;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use sqlx::Sqlite;
use sqlx::Transaction;

impl LocalAgentMessageBoard {
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
