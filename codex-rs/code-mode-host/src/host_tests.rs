use codex_code_mode_protocol::host::Capability;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HandshakeRejectReason;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::WireResult;
use pretty_assertions::assert_eq;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::io::AsyncWrite;

struct FailWritesAfterFirstFlush<W> {
    inner: W,
    fail_writes: bool,
}

impl<W> FailWritesAfterFirstFlush<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            fail_writes: false,
        }
    }
}

impl<W> AsyncWrite for FailWritesAfterFirstFlush<W>
where
    W: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.fail_writes {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected writer failure",
            )));
        }
        Pin::new(&mut self.inner).poll_write(context, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.inner).poll_flush(context) {
            Poll::Ready(Ok(())) => {
                self.fail_writes = true;
                Poll::Ready(Ok(()))
            }
            result => result,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

use super::run;

fn client_hello(
    versions: impl IntoIterator<Item = ProtocolVersion>,
    required_capabilities: CapabilitySet,
) -> ClientToHost {
    ClientToHost::ClientHello(
        ClientHello::new(
            SupportedProtocolVersions::try_new(versions).expect("supported versions"),
            required_capabilities,
            CapabilitySet::empty(),
        )
        .expect("client hello"),
    )
}

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session ID")
}

fn request_id(value: i64) -> RequestId {
    RequestId::new(value)
}

#[tokio::test]
async fn handshake_and_multiple_session_lifecycles_are_ordered() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);

    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("read hello"),
        Some(HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            CapabilitySet::empty(),
        )))
    );

    for (request_id, id) in [
        (request_id(/*value*/ 1), "session-1"),
        (request_id(/*value*/ 2), "session-2"),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request: HostRequest::OpenSession {
                    session_id: session_id(id),
                },
            })
            .await
            .expect("open session");
        assert_eq!(
            reader.read::<HostToClient>().await.expect("session ready"),
            Some(HostToClient::Response {
                id: request_id,
                result: WireResult::Ok {
                    value: HostResponse::SessionReady {
                        session_id: session_id(id),
                    },
                },
            })
        );
    }

    for (request_id, id) in [
        (request_id(/*value*/ 3), "session-1"),
        (request_id(/*value*/ 4), "session-2"),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request: HostRequest::ShutdownSession {
                    session_id: session_id(id),
                },
            })
            .await
            .expect("shutdown session");
        assert_eq!(
            reader.read::<HostToClient>().await.expect("session closed"),
            Some(HostToClient::Response {
                id: request_id,
                result: WireResult::Ok {
                    value: HostResponse::SessionClosed {
                        session_id: session_id(id),
                    },
                },
            })
        );
    }

    drop(writer);
    drop(reader);
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn incompatible_or_invalid_handshake_is_rejected() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    let version_two = ProtocolVersion::new(/*value*/ 2).expect("protocol version");

    writer
        .write(&client_hello([version_two], CapabilitySet::empty()))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::NoCompatibleVersion {
                supported_versions: SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                    .expect("host versions"),
            },
        })
    );
    host.await.expect("host task").expect("host connection");

    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 1),
            request: HostRequest::OpenSession {
                session_id: session_id("session-1"),
            },
        })
        .await
        .expect("write invalid first message");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::InvalidHello {
                message: "first message must be connection/hello".to_string(),
            },
        })
    );
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn unsupported_required_capability_is_rejected() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    let capability = Capability::new("required").expect("capability");

    writer
        .write(&client_hello(
            [ProtocolVersion::V1],
            CapabilitySet::try_new([capability.clone()]).expect("capabilities"),
        ))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::MissingRequiredCapability { capability },
        })
    );
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn session_id_cannot_be_reused_after_shutdown() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 2048);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write hello");
    reader
        .read::<HostToClient>()
        .await
        .expect("read hello")
        .expect("host hello");

    let id = session_id("session-1");
    for (request_id, request) in [
        (
            request_id(/*value*/ 1),
            HostRequest::OpenSession {
                session_id: id.clone(),
            },
        ),
        (
            request_id(/*value*/ 2),
            HostRequest::ShutdownSession {
                session_id: id.clone(),
            },
        ),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request,
            })
            .await
            .expect("session request");
        reader
            .read::<HostToClient>()
            .await
            .expect("session response")
            .expect("session response message");
    }
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 3),
            request: HostRequest::OpenSession { session_id: id },
        })
        .await
        .expect("reuse session ID");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("reuse response"),
        Some(HostToClient::Response {
            id: request_id(/*value*/ 3),
            result: WireResult::Err {
                message: "code-mode session ID `session-1` was reused".to_string(),
            },
        })
    );
    drop(writer);
    drop(reader);
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn writer_failure_terminates_the_connection() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(
        host_reader,
        FailWritesAfterFirstFlush::new(host_writer),
    ));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);

    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write hello");
    reader
        .read::<HostToClient>()
        .await
        .expect("read hello")
        .expect("host hello");

    writer
        .write(&ClientToHost::Request {
            id: request_id(1),
            request: HostRequest::OpenSession {
                session_id: session_id("session-1"),
            },
        })
        .await
        .expect("open session");
    let error = tokio::time::timeout(Duration::from_secs(5), host)
        .await
        .expect("host exit timeout")
        .expect("host task")
        .expect_err("writer failure should fail the connection");
    assert!(
        format!("{error:#}").contains("failed to write code-mode host message"),
        "unexpected error: {error:#}"
    );
}
