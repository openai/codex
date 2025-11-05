/// Gemini OAuth 2.0 authentication CLI commands
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Gemini authentication commands
#[derive(Debug, Parser)]
pub struct GeminiAuthCli {
    #[command(subcommand)]
    pub command: GeminiAuthCommand,
}

/// Gemini auth subcommands
#[derive(Debug, Parser)]
pub enum GeminiAuthCommand {
    /// Start OAuth 2.0 + PKCE authentication flow
    Login(LoginCommand),
    /// Check current authentication status
    Status(StatusCommand),
    /// Logout and clear cached tokens
    Logout(LogoutCommand),
}

/// Login command
#[derive(Debug, Parser)]
pub struct LoginCommand {
    /// Custom client ID (optional)
    #[arg(long)]
    pub client_id: Option<String>,
    
    /// Custom redirect URI (optional)
    #[arg(long, default_value = "http://localhost:8080/oauth/callback")]
    pub redirect_uri: String,
}

/// Status command
#[derive(Debug, Parser)]
pub struct StatusCommand {}

/// Logout command
#[derive(Debug, Parser)]
pub struct LogoutCommand {}

/// Handle gemini auth login
pub async fn handle_login(cmd: LoginCommand) -> Result<()> {
    use codex_gemini_cli_mcp_server::oauth::{OAuthConfig, OAuthManager, PKCEChallenge};

    println!("🔐 Starting Gemini OAuth 2.0 + PKCE authentication...\n");

    // Create OAuth config
    let mut config = OAuthConfig::default();
    if let Some(client_id) = cmd.client_id {
        config.client_id = client_id;
    }
    config.redirect_uri = cmd.redirect_uri;

    let mut manager = OAuthManager::new(config.clone());

    // Generate PKCE challenge
    let pkce = PKCEChallenge::generate().context("Failed to generate PKCE challenge")?;
    
    // Get authorization URL
    let auth_url = manager.get_authorization_url(&pkce);

    println!("📋 Step 1: Open this URL in your browser:");
    println!("   {}\n", auth_url);
    println!("📋 Step 2: Authorize the application");
    println!("📋 Step 3: Copy the authorization code from the redirect URL\n");

    // Prompt for authorization code
    print!("🔑 Enter authorization code: ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut code = String::new();
    io::stdin().read_line(&mut code)?;
    let code = code.trim();

    if code.is_empty() {
        anyhow::bail!("Authorization code cannot be empty");
    }

    // Exchange code for token
    println!("\n🔄 Exchanging authorization code for access token...");
    let token = manager.exchange_code(code, &pkce.verifier).await?;

    println!("✅ Authentication successful!");
    println!("   Token type: {}", token.token_type);
    println!("   Expires in: {} seconds", token.expires_in);
    println!("   Token cached to: {:?}", config.token_cache_path);

    Ok(())
}

/// Handle gemini auth status
pub async fn handle_status(_cmd: StatusCommand) -> Result<()> {
    use codex_gemini_cli_mcp_server::oauth::{OAuthConfig, OAuthManager};

    let config = OAuthConfig::default();
    let mut manager = OAuthManager::new(config.clone());

    println!("🔍 Checking Gemini authentication status...\n");

    match manager.load_cached_token()? {
        Some(token) => {
            let remaining = token.remaining_lifetime();
            println!("✅ Authenticated");
            println!("   Token type: {}", token.token_type);
            if token.is_expired() {
                println!("   Status: ⚠️  EXPIRED");
            } else {
                println!("   Status: 🟢 Valid");
                println!("   Expires in: {} seconds ({} minutes)", remaining, remaining / 60);
            }
            println!("   Has refresh token: {}", token.refresh_token.is_some());
            println!("   Cache path: {:?}", config.token_cache_path);
        }
        None => {
            println!("❌ Not authenticated");
            println!("   Run: codex gemini auth login");
        }
    }

    Ok(())
}

/// Handle gemini auth logout
pub async fn handle_logout(_cmd: LogoutCommand) -> Result<()> {
    use codex_gemini_cli_mcp_server::oauth::{OAuthConfig, OAuthManager};

    let config = OAuthConfig::default();
    let mut manager = OAuthManager::new(config);

    println!("🚪 Logging out from Gemini...");

    manager.clear_cache()?;

    println!("✅ Successfully logged out");
    println!("   Token cache cleared");

    Ok(())
}

/// Main handler for gemini auth commands
pub async fn handle_gemini_auth_command(cmd: GeminiAuthCommand) -> Result<()> {
    match cmd {
        GeminiAuthCommand::Login(login_cmd) => handle_login(login_cmd).await,
        GeminiAuthCommand::Status(status_cmd) => handle_status(status_cmd).await,
        GeminiAuthCommand::Logout(logout_cmd) => handle_logout(logout_cmd).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_command_parsing() {
        let cmd = LoginCommand {
            client_id: Some("test-client".to_string()),
            redirect_uri: "http://localhost:8080/callback".to_string(),
        };
        assert_eq!(cmd.client_id, Some("test-client".to_string()));
    }

    #[test]
    fn test_status_command() {
        let _cmd = StatusCommand {};
        // Status command has no fields to test
    }

    #[test]
    fn test_logout_command() {
        let _cmd = LogoutCommand {};
        // Logout command has no fields to test
    }
}

