//! Operator-only setup for local experiments; no administrative agent tools.

use codex_agent_message_board_extension::local_board_options;
use codex_message_board::LocalBoardSetup;
use codex_message_board::configure_local_board;
use codex_protocol::SessionId;
use codex_state::SqliteConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let [_, sqlite_home, board, input] = args.as_slice() else {
        return Err("Usage: configure_board <sqlite-home> <board-session-id> <setup.json>".into());
    };
    let sqlite = SqliteConfig::from_sqlite_home_with_nfs_enforcement(
        std::path::absolute(PathBuf::from(sqlite_home))?.try_into()?,
        /*enforce_delete_on_nfs*/ false,
    );
    let board = SessionId::from_string(board.to_str().ok_or("invalid board ID")?)?;
    let input = tokio::fs::read(input).await?;
    if input.len() > 64 * 1024 {
        return Err("setup file exceeds 64 KiB".into());
    }
    let setup: LocalBoardSetup = serde_json::from_slice(&input)?;
    configure_local_board(&sqlite, &local_board_options(), board, setup).await?;
    println!("Configured local board {board}");
    Ok(())
}
