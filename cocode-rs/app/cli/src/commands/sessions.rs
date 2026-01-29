//! Sessions command - list saved sessions.

use cocode_config::ConfigManager;
use cocode_session::SessionManager;

/// Run the sessions command.
pub async fn run(_all: bool, _config: &ConfigManager) -> anyhow::Result<()> {
    let manager = SessionManager::new();

    println!("Sessions");
    println!("────────");
    println!();

    let sessions = manager.list_persisted().await?;

    if sessions.is_empty() {
        println!("No saved sessions found.");
        println!();
        println!("Start a new session with: cocode chat");
        return Ok(());
    }

    for session in sessions {
        let title = session.title.unwrap_or_else(|| "(untitled)".to_string());
        println!("{}", session.id);
        println!("  Title:    {title}");
        println!("  Model:    {}/{}", session.provider, session.model);
        println!("  Created:  {}", session.created_at);
        println!("  Activity: {}", session.last_activity_at);
        println!();
    }

    println!("Resume a session with: cocode resume <session_id>");

    Ok(())
}
