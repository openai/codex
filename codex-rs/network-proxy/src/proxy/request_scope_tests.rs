use super::*;
use crate::state::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

#[tokio::test]
async fn request_scoped_proxy_stamps_blocked_requests() -> Result<()> {
    let (blocked_tx, mut blocked_rx) = tokio::sync::mpsc::unbounded_channel();
    let proxy = NetworkProxy::builder()
        .state(Arc::new(network_proxy_state_for_policy(Default::default())))
        .blocked_request_observer(move |request: crate::runtime::BlockedRequest| {
            let blocked_tx = blocked_tx.clone();
            async move {
                let _ = blocked_tx.send(request.request_origin);
            }
        })
        .build()
        .await?;
    let scoped = proxy.scope_for_request("local", "exec-1".to_string())?;
    let mut stream = tokio::net::TcpStream::connect(scoped.http_addr()).await?;
    stream
        .write_all(
            b"GET http://example.com/ HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n",
        )
        .await?;

    let blocked = timeout(Duration::from_secs(2), blocked_rx.recv()).await?;
    assert_eq!(blocked, Some(Some("exec-1".to_string())));

    #[cfg(target_os = "macos")]
    {
        proxy.runtime_settings.write().unwrap().allow_local_binding = true;
        assert_eq!(
            proxy.scope_for_request("local", "exec-2".to_string())?,
            proxy
        );
    }
    Ok(())
}
