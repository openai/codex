use std::ffi::c_int;
use std::ptr;
use std::sync::Arc;

use crate::client::conn;
use crate::rt::Executor as _;

use super::error::hyper_code;
use super::http_types::{hyper_request, hyper_response};
use super::io::hyper_io;
use super::task::{hyper_executor, hyper_task, hyper_task_return_type, AsTaskType, WeakExec};

/// An options builder to configure an HTTP client connection.
///
/// Methods:
///
/// - hyper_clientconn_options_new:     Creates a new set of HTTP clientconn options to be used in a handshake.
/// - hyper_clientconn_options_exec:    Set the client background task executor.
/// - hyper_clientconn_options_http2:   Set whether to use HTTP2.
/// - hyper_clientconn_options_set_preserve_header_case:  Set whether header case is preserved.
/// - hyper_clientconn_options_set_preserve_header_order: Set whether header order is preserved.
/// - hyper_clientconn_options_http1_allow_multiline_headers: Set whether HTTP/1 connections accept obsolete line folding for header values.
/// - hyper_clientconn_options_free:    Free a set of HTTP clientconn options.
pub struct hyper_clientconn_options {
    http1_allow_obsolete_multiline_headers_in_responses: bool,
    http1_preserve_header_case: bool,
    http1_preserve_header_order: bool,
    http2: bool,
    /// Use a `Weak` to prevent cycles.
    exec: WeakExec,
}

/// An HTTP client connection handle.
///
/// These are used to send one or more requests on a single connection.
///
/// It's possible to send multiple requests on a single connection, such
/// as when HTTP/1 keep-alive or HTTP/2 is used.
///
/// To create a `hyper_clientconn`:
///
///   1. Create a `hyper_io` with `hyper_io_new`.
///   2. Create a `hyper_clientconn_options` with `hyper_clientconn_options_new`.
///   3. Call `hyper_clientconn_handshake` with the `hyper_io` and `hyper_clientconn_options`.
///      This creates a `hyper_task`.
///   5. Call `hyper_task_set_userdata` to assign an application-specific pointer to the task.
///      This allows keeping track of multiple connections that may be handshaking
///      simultaneously.
///   4. Add the `hyper_task` to an executor with `hyper_executor_push`.
///   5. Poll that executor until it yields a task of type `HYPER_TASK_CLIENTCONN`.
///   6. Extract the `hyper_clientconn` from the task with `hyper_task_value`.
///      This will require a cast from `void *` to `hyper_clientconn *`.
///
/// This process results in a `hyper_clientconn` that permanently owns the
/// `hyper_io`. Because the `hyper_io` in turn owns a TCP or TLS connection, that means
/// the `hyper_clientconn` owns the connection for both the clientconn's lifetime
/// and the connection's lifetime.
///
/// In other words, each connection (`hyper_io`) must have exactly one `hyper_clientconn`
/// associated with it. That's because `hyper_clientconn_handshake` sends the
/// [HTTP/2 Connection Preface] (for HTTP/2 connections). Since that preface can't
/// be sent twice, handshake can't be called twice.
///
/// [HTTP/2 Connection Preface]: https://datatracker.ietf.org/doc/html/rfc9113#name-http-2-connection-preface
///
/// Methods:
///
/// - hyper_clientconn_handshake:  Creates an HTTP client handshake task.
/// - hyper_clientconn_send:       Creates a task to send a request on the client connection.
/// - hyper_clientconn_free:       Free a hyper_clientconn *.
pub struct hyper_clientconn {
    tx: Tx,
}

enum Tx {
    #[cfg(feature = "http1")]
    Http1(conn::http1::SendRequest<crate::body::Incoming>),
    #[cfg(feature = "http2")]
    Http2(conn::http2::SendRequest<crate::body::Incoming>),
}

// ===== impl hyper_clientconn =====

ffi_fn! {
    /// Creates an HTTP client handshake task.
    ///
    /// Both the `io` and the `options` are consumed in this function call.
    /// They should not be used or freed afterwards.
    ///
    /// The returned task must be polled with an executor until the handshake
    /// completes, at which point the value can be taken.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`, or taken ownership of by `hyper_executor_push`
    /// without subsequently being given back by `hyper_executor_poll`.
    fn hyper_clientconn_handshake(io: *mut hyper_io, options: *mut hyper_clientconn_options) -> *mut hyper_task {
        let options = non_null! { Box::from_raw(options) ?= ptr::null_mut() };
        let io = non_null! { Box::from_raw(io) ?= ptr::null_mut() };

        Box::into_raw(hyper_task::boxed(async move {
            #[cfg(feature = "http2")]
            {
            if options.http2 {
                return conn::http2::Builder::new(options.exec.clone())
                    .handshake::<_, crate::body::Incoming>(io)
                    .await
                    .map(|(tx, conn)| {
                        options.exec.execute(Box::pin(async move {
                            let _ = conn.await;
                        }));
                        hyper_clientconn { tx: Tx::Http2(tx) }
                    });
                }
            }

            conn::http1::Builder::new()
                .allow_obsolete_multiline_headers_in_responses(options.http1_allow_obsolete_multiline_headers_in_responses)
                .preserve_header_case(options.http1_preserve_header_case)
                .preserve_header_order(options.http1_preserve_header_order)
                .handshake::<_, crate::body::Incoming>(io)
                .await
                .map(|(tx, conn)| {
                    options.exec.execute(Box::pin(async move {
                        let _ = conn.await;
                    }));
                    hyper_clientconn { tx: Tx::Http1(tx) }
                })
        }))
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Creates a task to send a request on the client connection.
    ///
    /// This consumes the request. You should not use or free the request
    /// afterwards.
    ///
    /// Returns a task that needs to be polled until it is ready. When ready, the
    /// task yields a `hyper_response *`.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`, or taken ownership of by `hyper_executor_push`
    /// without subsequently being given back by `hyper_executor_poll`.
    fn hyper_clientconn_send(conn: *mut hyper_clientconn, req: *mut hyper_request) -> *mut hyper_task {
        let mut req = non_null! { Box::from_raw(req) ?= ptr::null_mut() };

        // Update request with original-case map of headers
        req.finalize_request();

        let fut = match non_null! { &mut *conn ?= ptr::null_mut() }.tx {
            Tx::Http1(ref mut tx) => futures_util::future::Either::Left(tx.send_request(req.0)),
            Tx::Http2(ref mut tx) => futures_util::future::Either::Right(tx.send_request(req.0)),
        };

        let fut = async move {
            fut.await.map(hyper_response::wrap)
        };

        Box::into_raw(hyper_task::boxed(fut))
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Free a `hyper_clientconn *`.
    ///
    /// This should be used for any connection once it is no longer needed.
    fn hyper_clientconn_free(conn: *mut hyper_clientconn) {
        drop(non_null! { Box::from_raw(conn) ?= () });
    }
}

unsafe impl AsTaskType for hyper_clientconn {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_CLIENTCONN
    }
}

// ===== impl hyper_clientconn_options =====

ffi_fn! {
    /// Creates a new set of HTTP clientconn options to be used in a handshake.
    ///
    /// To avoid a memory leak, the options must eventually be consumed by
    /// `hyper_clientconn_options_free` or `hyper_clientconn_handshake`.
    fn hyper_clientconn_options_new() -> *mut hyper_clientconn_options {
        Box::into_raw(Box::new(hyper_clientconn_options {
            http1_allow_obsolete_multiline_headers_in_responses: false,
            http1_preserve_header_case: false,
            http1_preserve_header_order: false,
            http2: false,
            exec: WeakExec::new(),
        }))
    } ?= std::ptr::null_mut()
}

ffi_fn! {
    /// Set whether header case is preserved.
    ///
    /// Pass `0` to allow lowercase normalization (default), `1` to retain original case.
    fn hyper_clientconn_options_set_preserve_header_case(opts: *mut hyper_clientconn_options, enabled: c_int) {
        let opts = non_null! { &mut *opts ?= () };
        opts.http1_preserve_header_case = enabled != 0;
    }
}

ffi_fn! {
    /// Set whether header order is preserved.
    ///
    /// Pass `0` to allow reordering (default), `1` to retain original ordering.
    fn hyper_clientconn_options_set_preserve_header_order(opts: *mut hyper_clientconn_options, enabled: c_int) {
        let opts = non_null! { &mut *opts ?= () };
        opts.http1_preserve_header_order = enabled != 0;
    }
}

ffi_fn! {
    /// Free a set of HTTP clientconn options.
    ///
    /// This should only be used if the options aren't consumed by
    /// `hyper_clientconn_handshake`.
    fn hyper_clientconn_options_free(opts: *mut hyper_clientconn_options) {
        drop(non_null! { Box::from_raw(opts) ?= () });
    }
}

ffi_fn! {
    /// Set the client background task executor.
    ///
    /// This does not consume the `options` or the `exec`.
    fn hyper_clientconn_options_exec(opts: *mut hyper_clientconn_options, exec: *const hyper_executor) {
        let opts = non_null! { &mut *opts ?= () };

        let exec = non_null! { Arc::from_raw(exec) ?= () };
        let weak_exec = hyper_executor::downgrade(&exec);
        std::mem::forget(exec);

        opts.exec = weak_exec;
    }
}

ffi_fn! {
    /// Set whether to use HTTP2.
    ///
    /// Pass `0` to disable, `1` to enable.
    fn hyper_clientconn_options_http2(opts: *mut hyper_clientconn_options, enabled: c_int) -> hyper_code {
        #[cfg(feature = "http2")]
        {
            let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
            opts.http2 = enabled != 0;
            hyper_code::HYPERE_OK
        }

        #[cfg(not(feature = "http2"))]
        {
            drop(opts);
            drop(enabled);
            hyper_code::HYPERE_FEATURE_NOT_ENABLED
        }
    }
}

ffi_fn! {
    /// Set whether HTTP/1 connections accept obsolete line folding for header values.
    ///
    /// Newline codepoints (\r and \n) will be transformed to spaces when parsing.
    ///
    /// Pass `0` to disable, `1` to enable.
    ///
    fn hyper_clientconn_options_http1_allow_multiline_headers(opts: *mut hyper_clientconn_options, enabled: c_int) -> hyper_code {
        let opts = non_null! { &mut *opts ?= hyper_code::HYPERE_INVALID_ARG };
        opts.http1_allow_obsolete_multiline_headers_in_responses = enabled != 0;
        hyper_code::HYPERE_OK
    }
}
