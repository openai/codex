#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn resolve_remote_addr_accepts_websocket_url() {
    assert_eq!(
        resolve_remote_addr("ws://127.0.0.1:4500").expect("ws URL should normalize"),
        RemoteAppServerEndpoint::WebSocket {
            websocket_url: "ws://127.0.0.1:4500/".to_string(),
            auth_token: None,
        }
    );
}

#[test]
fn resolve_remote_addr_accepts_secure_websocket_url() {
    assert_eq!(
        resolve_remote_addr("wss://example.com:443").expect("wss URL should normalize"),
        RemoteAppServerEndpoint::WebSocket {
            websocket_url: "wss://example.com/".to_string(),
            auth_token: None,
        }
    );
}

#[test]
fn resolve_remote_addr_accepts_default_socket() -> color_eyre::Result<()> {
    let codex_home = find_codex_home().wrap_err("failed to resolve CODEX_HOME")?;
    assert_eq!(
        resolve_remote_addr("unix://")?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: crate::app_server_control_socket_path(&codex_home)?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_accepts_relative_socket_path() -> color_eyre::Result<()> {
    assert_eq!(
        resolve_remote_addr("unix://codex.sock")?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_accepts_absolute_socket_path() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let socket_path = temp_dir.path().join("codex.sock");
    assert_eq!(
        resolve_remote_addr(&format!("unix://{}", socket_path.display()))?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::from_absolute_path(&socket_path)?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_rejects_invalid_remote_addresses() {
    for addr in [
        "ws://127.0.0.1",
        "wss://example.com",
        "127.0.0.1:4500",
        "https://127.0.0.1:4500",
    ] {
        let err = resolve_remote_addr(addr).expect_err("invalid remote addresses should fail");
        assert!(
            err.to_string().contains(
                "expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
            )
        );
    }
}
#[test]
fn read_remote_auth_token_from_env_var_reports_missing_values() {
    let err = read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
        Err(std::env::VarError::NotPresent)
    })
    .expect_err("missing env vars should be rejected");
    assert!(err.to_string().contains("is not set"));
}

#[test]
fn read_remote_auth_token_from_env_var_trims_values() {
    let auth_token = read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
        Ok("  bearer-token  ".to_string())
    })
    .expect("env var should parse");
    assert_eq!(auth_token, "bearer-token");
}

#[test]
fn read_remote_auth_token_from_env_var_rejects_empty_values() {
    let err = read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
        Ok(" \n\t ".to_string())
    })
    .expect_err("empty env vars should be rejected");
    assert!(err.to_string().contains("is empty"));
}
