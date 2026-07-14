use bytes::BytesMut;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEventReceiver;
use codex_exec_server::ExecProcessFuture;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use pretty_assertions::assert_eq;
use rmcp::service::RoleClient;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Poll;
use tokio::sync::watch;

use super::ExecutorProcessTransport;
use super::LineBuffer;
use super::LineTooLong;
use super::MAX_MCP_STDOUT_LINE_BYTES;

struct BlockingFirstWriteProcess {
    process_id: ProcessId,
    writes: StdMutex<Vec<Vec<u8>>>,
    release_first_write: AtomicBool,
}

impl BlockingFirstWriteProcess {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl ExecProcess for BlockingFirstWriteProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        watch::channel(0).1
    }

    fn subscribe_events(&self) -> ExecProcessEventReceiver {
        ExecProcessEventReceiver::empty()
    }

    fn read(
        &self,
        _after_seq: Option<u64>,
        _max_bytes: Option<usize>,
        _wait_ms: Option<u64>,
    ) -> ExecProcessFuture<'_, ReadResponse> {
        Box::pin(async { unreachable!("send test should not read process output") })
    }

    fn write(&self, chunk: Vec<u8>) -> ExecProcessFuture<'_, WriteResponse> {
        let first_write = {
            let mut writes = self.writes.lock().expect("writes lock");
            writes.push(chunk);
            writes.len() == 1
        };
        Box::pin(std::future::poll_fn(move |_| {
            if first_write && !self.release_first_write.load(Ordering::Acquire) {
                return Poll::Pending;
            }
            Poll::Ready(Ok(WriteResponse {
                status: WriteStatus::Accepted,
            }))
        }))
    }

    fn signal(&self, _signal: ProcessSignal) -> ExecProcessFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn terminate(&self) -> ExecProcessFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn serializes_concurrent_stdin_writes() {
    let process = Arc::new(BlockingFirstWriteProcess {
        process_id: ProcessId::from("mcp-stdio-test"),
        writes: StdMutex::new(Vec::new()),
        release_first_write: AtomicBool::new(false),
    });
    let mut transport =
        ExecutorProcessTransport::new(process.clone(), "mcp-stdio-test".to_string());
    let first_message: TxJsonRpcMessage<RoleClient> =
        serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
            .expect("first MCP message should deserialize");
    let second_message: TxJsonRpcMessage<RoleClient> =
        serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }))
            .expect("second MCP message should deserialize");

    // Drive both sends explicitly so task scheduling cannot hide an overlapping write.
    let first_send = transport.send(first_message);
    tokio::pin!(first_send);
    assert!(matches!(futures::poll!(first_send.as_mut()), Poll::Pending));
    assert_eq!(
        process.writes(),
        vec![b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec()]
    );

    let second_send = transport.send(second_message);
    tokio::pin!(second_send);
    assert!(matches!(
        futures::poll!(second_send.as_mut()),
        Poll::Pending
    ));
    assert_eq!(
        process.writes(),
        vec![b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec()]
    );

    process.release_first_write.store(true, Ordering::Release);
    assert!(matches!(
        futures::poll!(first_send.as_mut()),
        Poll::Ready(Ok(()))
    ));
    assert!(matches!(
        futures::poll!(second_send.as_mut()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(
        process.writes(),
        vec![
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec(),
            b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n".to_vec(),
        ]
    );
}

#[test]
fn searches_only_new_bytes_after_partial_line() {
    let mut buffer = LineBuffer::default();

    buffer
        .extend_from_slice(b"partial")
        .expect("partial line should fit");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
            pending_line_bytes: 7,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );

    buffer
        .extend_from_slice(b" line")
        .expect("partial line should fit");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial line"[..]),
            scanned_len: 12,
            pending_line_bytes: 12,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );

    buffer
        .extend_from_slice(b"\nnext")
        .expect("completed line should fit");
    assert_eq!(
        buffer.take_line(),
        Some(BytesMut::from(&b"partial line"[..]))
    );
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"next"[..]),
            scanned_len: 0,
            pending_line_bytes: 4,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );
}

#[test]
fn splits_multiple_lines_and_retains_partial_tail() {
    let mut buffer = LineBuffer::default();
    buffer
        .extend_from_slice(b"first\nsecond\npartial")
        .expect("lines should fit");

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"first"[..])));
    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"second"[..])));
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
            pending_line_bytes: 7,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );
}

#[test]
fn takes_unterminated_remaining_bytes_at_eof() {
    let mut buffer = LineBuffer::default();
    buffer
        .extend_from_slice(b"remaining")
        .expect("remaining line should fit");
    assert_eq!(buffer.take_line(), None);

    assert_eq!(
        buffer.take_remaining(),
        Some(BytesMut::from(&b"remaining"[..]))
    );
    assert_eq!(buffer, LineBuffer::default());
}

#[test]
fn rejects_oversized_line_without_retaining_its_prefix() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);
    buffer
        .extend_from_slice(b"12345")
        .expect("line at the limit should fit");
    assert_eq!(buffer.take_line(), None);

    assert_eq!(
        buffer.extend_from_slice(b"6"),
        Err(LineTooLong { max_line_bytes: 5 })
    );
    assert_eq!(buffer, LineBuffer::new(/*max_line_bytes*/ 5));
}

#[test]
fn retains_complete_lines_before_an_oversized_line() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);

    assert_eq!(
        buffer.extend_from_slice(b"first\n123456"),
        Err(LineTooLong { max_line_bytes: 5 })
    );

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"first"[..])));
    assert_eq!(buffer.take_remaining(), None);
}

#[test]
fn accepts_input_larger_than_limit_when_each_line_is_bounded() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);

    buffer
        .extend_from_slice(b"12345\nabcde\ntail")
        .expect("each individual line should fit");

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"12345"[..])));
    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"abcde"[..])));
    assert_eq!(buffer.take_line(), None);
    assert_eq!(buffer.take_remaining(), Some(BytesMut::from(&b"tail"[..])));
}
