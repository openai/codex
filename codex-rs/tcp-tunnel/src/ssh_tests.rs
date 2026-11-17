//! Check route authority and credential isolation before any network I/O.
use clap::Parser;
use http::Method;

use super::PUBLIC_GATEWAY;
use super::oauth_bearer;
use super::run_ssh_proxy;
use crate::Args;
use crate::SetupError;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: Args,
}

#[test]
fn routes_oauth_and_legacy_without_ambiguous_authority() -> anyhow::Result<()> {
    let oauth = Cli::try_parse_from([
        "tcp-tunnel",
        "--environment-id",
        "ccarenv_test",
        "--port",
        "2222",
    ])?
    .args;
    let (target, headers) = oauth.ssh_destination()?;
    let request = target.request(headers, oauth_bearer("fixture==")?)?;
    assert_eq!(request.method(), Method::CONNECT);
    assert_eq!(
        request.uri().to_string(),
        "codex-portals.api.openai.com:443"
    );
    assert_eq!(
        request.headers()["x-portals-environment-id"],
        "ccarenv_test"
    );
    assert_eq!(request.headers()["x-portals-port"], "2222");
    assert!(request.headers()["authorization"].is_sensitive());
    let legacy = Cli::try_parse_from([
        "tcp-tunnel",
        "--token-file",
        "/private/token",
        "--target",
        "10.0.0.1:22",
    ])?
    .args;
    let (target, headers) = legacy.ssh_destination()?;
    assert_eq!(target.authority, "10.0.0.1:22");
    assert!(headers.is_empty());
    assert!(Cli::try_parse_from(["tcp-tunnel", "--target", "10.0.0.1:22"]).is_err());
    assert!(
        Cli::try_parse_from([
            "tcp-tunnel",
            "--environment-id",
            "env",
            "--target",
            "10.0.0.1:22",
            "--token-file",
            "/private/token"
        ])
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn rejects_bad_origin_and_route_before_loading_credentials() -> anyhow::Result<()> {
    for (url, id) in [
        (PUBLIC_GATEWAY, "bad\r\nvalue"),
        ("https://codex-portals.api.openai.com.attacker.net", "valid"),
        ("https://codex-portals.api.openai.com:4443", "valid"),
    ] {
        let args =
            Cli::try_parse_from(["tcp-tunnel", "--environment-id", id, "--proxy-url", url])?.args;
        let mut stdout = Vec::new();
        assert!(
            run_ssh_proxy(
                args,
                async { panic!("credential lookup before gateway validation") },
                &mut stdout
            )
            .await
            .is_err()
        );
        assert!(stdout.is_empty());
    }
    Ok(())
}

#[test]
fn rejects_invalid_credentials_and_sanitizes_peer_errors() {
    for token in [
        "",
        "=",
        "private=secret",
        "private\r\nvalue",
        "private secret",
    ] {
        assert!(oauth_bearer(token).is_err());
    }
    assert!(
        !super::safe_tunnel_error(anyhow::anyhow!("peer supplied secret"))
            .to_string()
            .contains("secret")
    );
    assert!(
        super::safe_tunnel_error(SetupError::Rejected(http::StatusCode::FORBIDDEN).into())
            .to_string()
            .contains("403")
    );
}
