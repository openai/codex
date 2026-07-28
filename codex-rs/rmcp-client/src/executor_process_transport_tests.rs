use bytes::BytesMut;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEventReceiver;
use codex_exec_server::ExecProcessFuture;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessOutputChunk;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use pretty_assertions::assert_eq;
use rmcp::service::RoleClient;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use serde_json::json;
use std::io;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Poll;
use tokio::sync::watch;

use super::ExecutorProcessTransport;
use super::LineBuffer;
use super::LineTooLong;
use super::MAX_MCP_STDERR_LINE_BYTES;
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

struct RetainedReadProcess {
    process_id: ProcessId,
    response: ReadResponse,
}

impl ExecProcess for RetainedReadProcess {
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
        Box::pin(async { Ok(self.response.clone()) })
    }

    fn write(&self, _chunk: Vec<u8>) -> ExecProcessFuture<'_, WriteResponse> {
        Box::pin(async { unreachable!("recovery tests should not write process input") })
    }

    fn signal(&self, _signal: ProcessSignal) -> ExecProcessFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn terminate(&self) -> ExecProcessFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn recovered_output(seq: u64, stream: ExecOutputStream, bytes: &[u8]) -> ProcessOutputChunk {
    ProcessOutputChunk {
        seq,
        stream,
        chunk: bytes.to_vec().into(),
    }
}

fn retained_read_response(chunks: Vec<ProcessOutputChunk>, next_seq: u64) -> ReadResponse {
    ReadResponse {
        chunks,
        next_seq,
        exited: false,
        exit_code: None,
        closed: false,
        failure: None,
        sandbox_denied: false,
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

#[tokio::test]
async fn rejects_lagged_recovery_when_retained_output_skips_a_sequence() {
    let process = Arc::new(RetainedReadProcess {
        process_id: ProcessId::from("mcp-stdio-lost-output"),
        response: retained_read_response(
            vec![recovered_output(
                /*seq*/ 6,
                ExecOutputStream::Stdout,
                br#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
            )],
            /*next_seq*/ 7,
        ),
    });
    let mut transport = ExecutorProcessTransport::new(process, "mcp-stdio-lost-output".to_string());
    transport.last_seq = 4;
    transport
        .stdout
        .extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"result":""#)
        .expect("partial stdout should fit");
    transport
        .stderr
        .extend_from_slice(b"partial diagnostic")
        .expect("partial stderr should fit");

    let error = transport
        .recover_lagged_events()
        .await
        .expect_err("evicted output must close the transport");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "remote MCP server output stream lost process events: expected sequence 5, received 6"
    );
    assert_eq!(transport.stdout, LineBuffer::default());
    assert_eq!(transport.stderr, LineBuffer::new(MAX_MCP_STDERR_LINE_BYTES));
    assert!(transport.closed);
}

#[tokio::test]
async fn rejects_lagged_recovery_when_all_missing_output_was_evicted() {
    let process = Arc::new(RetainedReadProcess {
        process_id: ProcessId::from("mcp-stdio-evicted-output"),
        response: retained_read_response(Vec::new(), /*next_seq*/ 7),
    });
    let mut transport =
        ExecutorProcessTransport::new(process, "mcp-stdio-evicted-output".to_string());
    transport.last_seq = 4;

    let error = transport
        .recover_lagged_events()
        .await
        .expect_err("missing retained output must close the transport");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "remote MCP server output stream lost process events: expected sequence 5, received 7"
    );
    assert!(transport.closed);
}

#[tokio::test]
async fn recovers_contiguous_output_after_a_duplicate_replayed_event() {
    let process = Arc::new(RetainedReadProcess {
        process_id: ProcessId::from("mcp-stdio-duplicate-output"),
        response: retained_read_response(
            vec![
                recovered_output(
                    /*seq*/ 4,
                    ExecOutputStream::Stdout,
                    b"duplicate output\n",
                ),
                recovered_output(
                    /*seq*/ 5,
                    ExecOutputStream::Stdout,
                    b"recovered stdout\n",
                ),
                recovered_output(
                    /*seq*/ 6,
                    ExecOutputStream::Stderr,
                    b"recovered stderr\n",
                ),
            ],
            /*next_seq*/ 7,
        ),
    });
    let mut transport =
        ExecutorProcessTransport::new(process, "mcp-stdio-duplicate-output".to_string());
    transport.last_seq = 4;

    transport
        .recover_lagged_events()
        .await
        .expect("duplicate replay must not prevent recovery of contiguous output");

    assert_eq!(
        transport.stdout.take_line(),
        Some(BytesMut::from(&b"recovered stdout"[..]))
    );
    assert_eq!(transport.stdout.take_line(), None);
    assert_eq!(transport.stderr, LineBuffer::new(MAX_MCP_STDERR_LINE_BYTES));
    assert_eq!(transport.last_seq, 6);
    assert!(!transport.closed);
}

#[tokio::test]
async fn recovers_contiguous_output_and_accounts_for_terminal_events() {
    let mut response = retained_read_response(
        vec![
            recovered_output(
                /*seq*/ 5,
                ExecOutputStream::Stdout,
                b"recovered stdout\n",
            ),
            recovered_output(
                /*seq*/ 6,
                ExecOutputStream::Stderr,
                b"recovered stderr\n",
            ),
        ],
        /*next_seq*/ 9,
    );
    response.exited = true;
    response.exit_code = Some(0);
    response.closed = true;
    let process = Arc::new(RetainedReadProcess {
        process_id: ProcessId::from("mcp-stdio-recovered-output"),
        response,
    });
    let mut transport =
        ExecutorProcessTransport::new(process, "mcp-stdio-recovered-output".to_string());
    transport.last_seq = 4;

    transport
        .recover_lagged_events()
        .await
        .expect("contiguous retained output should remain recoverable");

    assert_eq!(
        transport.stdout.take_line(),
        Some(BytesMut::from(&b"recovered stdout"[..]))
    );
    assert_eq!(transport.stderr, LineBuffer::new(MAX_MCP_STDERR_LINE_BYTES));
    assert_eq!(transport.last_seq, 8);
    assert!(transport.closed);
}

#[tokio::test]
async fn closes_on_an_unsequenced_process_failure_without_inventing_missing_output() {
    let mut response = retained_read_response(Vec::new(), /*next_seq*/ 5);
    response.exited = true;
    response.closed = true;
    response.failure = Some("executor disconnected".to_string());
    let process = Arc::new(RetainedReadProcess {
        process_id: ProcessId::from("mcp-stdio-failed-process"),
        response,
    });
    let mut transport =
        ExecutorProcessTransport::new(process, "mcp-stdio-failed-process".to_string());
    transport.last_seq = 4;

    transport
        .recover_lagged_events()
        .await
        .expect("an unsequenced executor failure is not an output sequence gap");

    assert_eq!(transport.last_seq, 4);
    assert!(transport.closed);
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
