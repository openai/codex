use super::AttributionRegistry;
use super::BindConnectionAttribution;
use super::ConnectionAttribution;
use super::write_attribution_frame;
use crate::network_policy::NetworkRequestContext;
use pretty_assertions::assert_eq;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_core::service::service_fn;
use rama_tcp::TcpStream as RamaTcpStream;
use std::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[test]
fn attribution_frame_has_bounded_binary_prefix() -> io::Result<()> {
    let mut frame = Vec::new();
    write_attribution_frame(&mut frame, "token-1")?;

    assert_eq!(&frame[..8], b"\0CDXPXY1");
    assert_eq!(u16::from_be_bytes([frame[8], frame[9]]), 7);
    assert_eq!(&frame[10..], b"token-1");
    Ok(())
}

#[tokio::test]
async fn framed_connection_receives_registered_attribution() -> Result<(), BoxError> {
    let registry = AttributionRegistry::default();
    let expected = NetworkRequestContext {
        environment_id: Some("local".to_string()),
        execution_id: Some("execution-1".to_string()),
    };
    registry.register("token-1", expected.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let client = tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await?;
        let mut frame = Vec::new();
        write_attribution_frame(&mut frame, "token-1")?;
        stream.write_all(&frame).await
    });

    let (stream, _) = listener.accept().await?;
    let service = BindConnectionAttribution::new(
        service_fn(|stream: RamaTcpStream| async move {
            Ok::<_, io::Error>(stream.extensions().get::<NetworkRequestContext>().cloned())
        }),
        ConnectionAttribution::Registry(registry),
    );
    let actual = service.serve(RamaTcpStream::new(stream)).await?;
    client.await??;

    assert_eq!(actual, Some(expected));
    Ok(())
}
