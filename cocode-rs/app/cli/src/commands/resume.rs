//! Resume command - resume a previous session.

use cocode_config::ConfigManager;
use cocode_session::SessionManager;
use cocode_session::persistence::session_file_path;

use crate::repl::Repl;

/// Run the resume command.
pub async fn run(session_id: &str, config: &ConfigManager) -> anyhow::Result<()> {
    // Validate session exists before attempting to load
    let session_path = session_file_path(session_id);
    if !session_path.exists() {
        return Err(anyhow::anyhow!(
            "Session not found: {session_id}\n\nUse 'cocode sessions' to list available sessions."
        ));
    }

    println!("Resuming session: {session_id}");
    println!();

    let mut manager = SessionManager::new();

    // Load the session
    manager.load_session(session_id, config).await?;

    // Get the session state
    let state = manager
        .get_session(session_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to get session after loading"))?;

    println!("Model:    {}/{}", state.provider(), state.model());
    println!("Turns:    {}", state.total_turns());
    println!();

    // Start REPL with the resumed session
    let mut repl = Repl::new(state);
    repl.run().await?;

    // Save session on exit
    manager.save_session(session_id).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resume_nonexistent_session() {
        let config = ConfigManager::empty();
        let result = run("nonexistent-session-id", &config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Session not found"));
    }
}
