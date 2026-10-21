//! Loads initial channels into an empty local board and pins their source template.

use super::insert_channel;
use super::invalid;
use super::storage_error;
use super::validate_channel;
use crate::ChannelDescription;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Channel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<ChannelDescription>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    version: u32,
    #[serde(default)]
    channels: BTreeMap<String, Channel>,
}

/// Operator input for loading an empty board. This is not an agent tool.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalBoardSetup {
    #[serde(default)]
    timestamp: Option<DateTime<Utc>>,
    template: Template,
}

/// Saves the template and its initial channels atomically. The same template is
/// safe to retry; replacing it or initializing an already-used board is rejected.
pub async fn configure_local_board(
    sqlite: &SqliteConfig,
    board: SessionId,
    setup: LocalBoardSetup,
) -> Result<()> {
    let template = setup.template;
    let encoded = serde_json::to_string(&template).map_err(storage_error)?;
    if template.version != 1 || encoded.len() > 64 * 1024 || template.channels.len() > 64 {
        return Err(invalid(
            "board templates require version 1 and at most 64 KiB / 64 channels",
        ));
    }
    for name in template.channels.keys() {
        validate_channel(name)?;
    }
    let pool = super::pool::open(sqlite).await?;
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(storage_error)?;
    let board_key = board.to_string();
    let deleted: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM deleted_boards WHERE board=?)")
            .bind(&board_key)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
    if deleted {
        return Err(invalid("the board has been permanently deleted"));
    }
    let pinned: Option<String> =
        sqlx::query_scalar("SELECT template FROM board_templates WHERE board=?")
            .bind(&board_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage_error)?;
    if let Some(pinned) = pinned {
        if pinned != encoded {
            return Err(invalid("the board template is already pinned"));
        }
    } else {
        let used: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channels WHERE board=?)")
            .bind(&board_key)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
        if used {
            return Err(invalid(
                "pin the template before creating channels or posts",
            ));
        }
        sqlx::query("INSERT INTO board_templates(board,template) VALUES(?,?)")
            .bind(&board_key)
            .bind(encoded)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        let now = setup.timestamp.unwrap_or_else(Utc::now);
        for (name, channel) in &template.channels {
            insert_channel(
                &mut tx,
                board,
                name,
                channel.description.as_ref(),
                &AgentPath::root(),
                now,
            )
            .await?;
        }
    }
    tx.commit().await.map_err(storage_error)
}
