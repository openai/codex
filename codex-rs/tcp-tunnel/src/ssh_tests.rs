#![cfg(test)]

//! Exercise route selection, credential boundaries, and the native OAuth CONNECT request.
use super::Args;
use super::PUBLIC_GATEWAY;
use super::oauth_bearer;
use super::run_ssh_proxy;
use clap::Parser;
use http::HeaderMap;
use http::Method;
use pretty_assertions::assert_eq;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: Args,
}

fn tcp_tunnel_args<'a>(gateway: &'a str, target: &'a str) -> [&'a str; 5] {
    ["tcp-tunnel", "--proxy-url", gateway, "--target", target]
}

#[test]
fn listener_options_require_the_separate_stdin_control_protocol() {
    let ssh = tcp_tunnel_args(PUBLIC_GATEWAY, "ccarenv_test:22");
    assert!(!Cli::try_parse_from(ssh).unwrap().args.auth_token_stdin);
    for incomplete in [
        &["--auth-token-stdin"][..],
        &["--proxy-origins-file", "origins"],
        &["--listen-addr", "127.0.0.1:0"],
        &["--connect-headers-stdin"],
        &["--auth-token-updates-stdin"],
        &["--diagnostics-json"],
    ] {
        assert!(Cli::try_parse_from(ssh.into_iter().chain(incomplete.iter().copied())).is_err());
    }
}

#[test]
fn connect_routes_by_gateway_authority_and_published_port() -> anyhow::Result<()> {
    let args = Cli::try_parse_from(tcp_tunnel_args(PUBLIC_GATEWAY, "ccarenv_test-1:2222"))?.args;
    let named = Cli::try_parse_from([
        "tcp-tunnel",
        "--environment-id",
        "ccarenv_test-1",
        "--port",
        "2222",
    ])?
    .args;
    let (target, metadata) = named.ssh_destination()?;
    let (explicit_target, explicit_headers) = args.ssh_destination()?;
    assert_eq!(target, explicit_target);
    assert_eq!(metadata, explicit_headers);
    let request = target.request(metadata, oauth_bearer("local-fixture==")?)?;
    assert!(request.headers()[http::header::AUTHORIZATION].is_sensitive());
    assert_eq!(request.method(), Method::CONNECT);
    assert_eq!(
        request.uri().to_string(),
        "codex-portals.api.openai.com:443"
    );
    let expected = HeaderMap::from_iter([
        (
            http::header::AUTHORIZATION,
            "Bearer local-fixture==".parse()?,
        ),
        ("x-portals-auth-mode".parse()?, "oauth".parse()?),
        (
            "x-portals-environment-id".parse()?,
            "ccarenv_test-1".parse()?,
        ),
        ("x-portals-port".parse()?, "2222".parse()?),
    ]);
    assert_eq!(request.headers(), &expected);
    Ok(())
}

#[tokio::test]
async fn invalid_routes_fail_before_reading_oauth_or_writing_stdout() -> anyhow::Result<()> {
    for (url, target) in [
        (PUBLIC_GATEWAY, ":22"),
        (PUBLIC_GATEWAY, "env"),
        (PUBLIC_GATEWAY, "env:0"),
        (PUBLIC_GATEWAY, "env:65536"),
        (PUBLIC_GATEWAY, "id\r\nother: value:22"),
        ("https://codex-portals.api.openai.com.bad", "valid:22"),
        ("https://codex-portals.api.openai.com:4443", "valid:22"),
    ] {
        let args = Cli::try_parse_from(tcp_tunnel_args(url, target))?.args;
        let mut out = Vec::new();
        let result = run_ssh_proxy(
            args,
            async { panic!("unexpected credential lookup") },
            &mut out,
        )
        .await;
        assert!(result.is_err());
        assert!(out.is_empty());
    }
    Ok(())
}

#[test]
fn environment_id_defaults_to_ssh_and_rejects_ambiguous_routes() -> anyhow::Result<()> {
    let named = ["tcp-tunnel", "--environment-id", "ccarenv_test"];
    assert_eq!(
        Cli::try_parse_from(named)?.args.ssh_destination()?.1["x-portals-port"],
        "22"
    );
    for extra in [
        &["--port", "0"][..],
        &["--port", "65536"],
        &["--target", "other:22"],
        &["--auth-token-stdin", "--proxy-origins-file", "origins"],
    ] {
        assert!(Cli::try_parse_from(named.into_iter().chain(extra.iter().copied())).is_err());
    }
    assert!(Cli::try_parse_from(["tcp-tunnel", "--target", "env:22"]).is_err());
    Ok(())
}

#[test]
fn rejects_invalid_oauth_credentials_without_echoing_them() {
    for token in [
        "",
        "=",
        "private=secret",
        "private\r\nvalue",
        "private secret",
    ] {
        assert_eq!(
            oauth_bearer(token).unwrap_err().to_string(),
            "invalid OAuth access token"
        );
    }
}
