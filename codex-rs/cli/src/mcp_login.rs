//! CLI MCP OAuth login modes and bounded, hidden callback input.
//!
//! Manual input remains cancellable while the OAuth flow waits for its HTTP
//! callback, and dropping terminal input restores the terminal immediately.
//! Ctrl-C also cancels token exchange and scope retries in manual login mode.

use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Write;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use codex_mcp::ResolvedMcpOAuthScopes;
use codex_mcp::should_retry_without_scopes;
use codex_rmcp_client::McpOAuthClientRegistration;
use codex_rmcp_client::perform_oauth_login;
use codex_rmcp_client::perform_oauth_login_with_callback_input;
use crossterm::event::Event;
use crossterm::event::EventStream;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::oneshot;

const MAX_CALLBACK_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum McpLoginMode {
    Browser,
    PasteCallback,
}

/// Retry provider-rejected discovered scopes once, retaining the selected input mode.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn perform_oauth_login_retry_without_scopes(
    name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    resolved_scopes: &ResolvedMcpOAuthScopes,
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
    mode: McpLoginMode,
) -> Result<()> {
    let mut pending_input = None;
    // Tokio retains its process-wide signal handler, so keep listening throughout
    // token exchange and scope retries after the callback input reader is dropped.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    for (attempt, scopes) in [resolved_scopes.scopes.as_slice(), &[]]
        .into_iter()
        .enumerate()
    {
        let login = async {
            match mode {
                McpLoginMode::Browser => {
                    perform_oauth_login(
                        name,
                        url,
                        store_mode,
                        keyring_backend_kind,
                        http_headers.clone(),
                        env_http_headers.clone(),
                        scopes,
                        oauth_client_id,
                        client_registration,
                        oauth_resource,
                        callback_port,
                        callback_url,
                        global_callback_url,
                        Arc::clone(&http_client),
                    )
                    .await
                }
                McpLoginMode::PasteCallback => {
                    let input = &mut pending_input;
                    perform_oauth_login_with_callback_input(
                        name,
                        url,
                        store_mode,
                        keyring_backend_kind,
                        http_headers.clone(),
                        env_http_headers.clone(),
                        scopes,
                        oauth_client_id,
                        client_registration,
                        oauth_resource,
                        callback_port,
                        callback_url,
                        global_callback_url,
                        Arc::clone(&http_client),
                        move |authorization_url| read_callback(authorization_url, input),
                    )
                    .await
                }
            }
        };
        let result = tokio::select! {
            result = login => result,
            result = &mut ctrl_c, if matches!(mode, McpLoginMode::PasteCallback) => {
                result.context("failed to listen for Ctrl-C")?;
                bail!("OAuth login cancelled");
            }
        };
        match result {
            Err(error) if attempt == 0 && should_retry_without_scopes(resolved_scopes, &error) => {
                println!("OAuth provider rejected discovered scopes. Retrying without scopes…");
            }
            result => return result,
        }
    }
    unreachable!("the empty-scope attempt always returns")
}

async fn read_callback(
    authorization_url: String,
    pending_input: &mut Option<oneshot::Receiver<Result<String>>>,
) -> Result<String> {
    println!(
        "Authorize the MCP server by opening this URL in your browser:\n{authorization_url}\n"
    );
    println!(
        "After signing in, copy the full URL from your browser's address bar.\n\
         If the callback page cannot load, paste that URL here anyway."
    );
    if io::stdin().is_terminal() {
        return read_terminal_callback().await;
    }
    print_callback_prompt()?;

    // A pipe may stay open after the HTTP callback wins. A detached thread avoids
    // making runtime shutdown wait for an uncancellable blocking stdin read.
    // Retain an unfinished read if an HTTP provider error triggers a scope retry.
    // Otherwise its detached reader could consume the next attempt's pasted URL.
    if pending_input.is_none() {
        let (tx, rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("mcp-oauth-callback-input".to_string())
            .spawn(move || {
                let result = read_callback_line(io::stdin().lock());
                let _ = tx.send(result);
            })
            .context("failed to start callback input reader")?;
        *pending_input = Some(rx);
    }
    let reader = pending_input
        .as_mut()
        .context("callback input reader was unavailable")?;
    let result = reader.await;
    *pending_input = None;
    result.context("callback input reader stopped")?
}

fn read_callback_line(reader: impl BufRead) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_CALLBACK_BYTES + 3) as u64)
        .read_until(b'\n', &mut bytes)
        .context("failed to read callback URL")?;
    if bytes.ends_with(b"\n") {
        bytes.pop();
    }
    if bytes.ends_with(b"\r") {
        bytes.pop();
    }
    if bytes.len() > MAX_CALLBACK_BYTES {
        bail!("OAuth callback URL exceeds 64 KiB");
    }
    if bytes.is_empty() {
        bail!("No OAuth callback URL received before input closed");
    }
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("OAuth callback URL must be valid UTF-8"))
}

struct TerminalInputGuard;

impl Drop for TerminalInputGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        eprintln!();
    }
}

fn print_callback_prompt() -> Result<()> {
    eprint!("Callback URL (input hidden): ");
    io::stderr()
        .flush()
        .context("failed to display callback prompt")
}

async fn read_terminal_callback() -> Result<String> {
    terminal::enable_raw_mode().context("failed to hide callback input")?;
    let _guard = TerminalInputGuard;
    print_callback_prompt()?;
    let mut events = EventStream::new();
    let mut input = String::new();
    loop {
        let event = events
            .next()
            .await
            .context("callback input stream closed")?
            .context("failed to read callback input")?;
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Enter => {
                    if input.is_empty() {
                        bail!("No OAuth callback URL entered");
                    }
                    return Ok(input);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    bail!("OAuth login cancelled");
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    bail!("No OAuth callback URL received before input closed");
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.clear()
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(character)
                    if !character.is_control()
                        && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if input.len() + character.len_utf8() > MAX_CALLBACK_BYTES {
                        bail!("OAuth callback URL exceeds 64 KiB");
                    }
                    input.push(character);
                }
                _ => {}
            },
            Event::Paste(paste) => {
                if input.len() + paste.len() > MAX_CALLBACK_BYTES {
                    bail!("OAuth callback URL exceeds 64 KiB");
                }
                input.push_str(&paste);
            }
            _ => {}
        }
        // Yield even with queued input so callback completion and timeout stay responsive.
        tokio::task::yield_now().await;
    }
}

#[cfg(test)]
#[path = "mcp_login_tests.rs"]
mod tests;
