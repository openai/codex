//! Single-stream OpenSSH ProxyCommand. Only the verified gateway receives credentials.
//! OpenSSH owns SSH authentication, host-key verification, and all stdio bytes.
use std::future::Future;
use std::io::Read;
use std::num::NonZeroU16;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use http::HeaderMap;
use http::HeaderValue;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use super::Args;
use super::BridgeMode;
use super::Code;
use super::ProxyConnection;
use super::ProxyTarget;
use super::bridge;
use super::client_config;
use super::control::TunnelCredential;

// Only gateways shipped with the CLI may receive the signed-in account's credential.
pub(super) const PUBLIC_GATEWAY: &str = "https://codex-portals.api.openai.com";
pub(super) const GATEWAY_ORIGINS: &[&str] = &[PUBLIC_GATEWAY];

impl Args {
    fn ssh_destination(&self) -> Result<(ProxyTarget, HeaderMap)> {
        let (environment_id, port) = match self.environment_id.as_deref() {
            Some(id) => (id, self.port.map_or(/*default*/ 22, NonZeroU16::get)),
            None => {
                let (id, port) = self
                    .target
                    .as_deref()
                    .and_then(|target| target.rsplit_once(':'))
                    .context("SSH target must be ENV:PORT")?;
                (
                    id,
                    port.parse::<NonZeroU16>()
                        .context("invalid SSH target port")?
                        .get(),
                )
            }
        };
        ensure!(
            (1..=256).contains(&environment_id.len())
                && environment_id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
            "invalid environment ID"
        );
        let host = self.proxy_url.host_str().context("missing gateway host")?;
        let authority = format!("{host}:{}", self.proxy_url.port().unwrap_or(443));
        let target = ProxyTarget::parse(&self.proxy_url, GATEWAY_ORIGINS, &authority)?;
        let mut headers = HeaderMap::new();
        headers.insert("x-portals-auth-mode", HeaderValue::from_static("oauth"));
        headers.insert(
            "x-portals-environment-id",
            HeaderValue::from_str(environment_id)?,
        );
        headers.insert("x-portals-port", port.into());
        Ok((target, headers))
    }
}

fn oauth_bearer(token: &str) -> Result<HeaderValue> {
    let unpadded = token.trim_end_matches('=');
    ensure!(
        !unpadded.is_empty()
            && token.len() < super::control::MAX_TOKEN_BYTES
            && unpadded
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~+/".contains(&b)),
        "invalid OAuth access token"
    );
    TunnelCredential::Bearer.header(token)
}

/// Relay one SSH session. Poll the active account's token only after verifying the gateway.
pub async fn run_ssh_proxy(
    args: Args,
    access_token: impl Future<Output = Result<String>>,
    stdout: impl AsyncWrite + Unpin,
) -> Result<()> {
    let (target, headers) = args.ssh_destination()?;
    // Linux can advertise UDP segmentation on paths where batched packets are lost.
    // Single datagrams keep bulk SSH transfers moving on those paths.
    let mut transport = quinn::TransportConfig::default();
    transport.enable_segmentation_offload(!cfg!(target_os = "linux"));
    let ProxyConnection {
        endpoint: _endpoint,
        quic: _quic,
        mut connection,
        mut sender,
    } = ProxyConnection::connect(&target, &client_config(transport)?).await?;
    let request = target.request(headers, oauth_bearer(&access_token.await?)?)?;

    // OpenSSH needs EOF when the peer finishes, even if it still holds stdin open.
    // A detached blocking reader lets the Tokio runtime exit in that case.
    let (mut input, reader) = tokio::io::duplex(64 * 1024);
    let runtime = tokio::runtime::Handle::current();
    let (outcome, input_done) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("ssh-stdin".into())
        .spawn(move || {
            let mut stdin = std::io::stdin().lock();
            let mut buf = [0; 16 * 1024];
            let result = loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break Ok(()),
                    Ok(n) => {
                        if let Err(err) = runtime.block_on(input.write_all(&buf[..n])) {
                            break Err(err);
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(err) => break Err(err),
                }
            };
            let _ = outcome.send(result);
        })?;
    let relay = bridge(
        reader,
        stdout,
        &mut sender,
        request,
        BridgeMode::ProxyCommand,
    );
    let forward = async {
        tokio::pin!(relay);
        tokio::select! {
            result = &mut relay => result,
            result = input_done => { result??; relay.await }
        }
    };
    tokio::pin!(forward);
    let result = tokio::select! {
        result = &mut forward => result,
        // A closing connection may still have a complete status or payload buffered.
        _ = connection.wait_idle() => forward.await,
    };
    // A peer's close reason can contain arbitrary text. Only surface the HTTP status.
    result.map_err(|err| match err.downcast_ref::<Code>() {
        Some(Code::Rejected(status)) => Code::Rejected(*status).into(),
        _ => anyhow::anyhow!("SSH tunnel stream failed"),
    })
}

#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
