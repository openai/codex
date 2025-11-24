use std::ffi::{c_int, c_void};
use std::mem::ManuallyDrop;
use std::ptr;
use std::task::{Context, Poll};

use http_body_util::BodyExt as _;

use super::task::{hyper_context, hyper_task, hyper_task_return_type, AsTaskType};
use super::{UserDataPointer, HYPER_ITER_CONTINUE};
use crate::body::{Bytes, Frame, Incoming as IncomingBody};
use crate::ffi::size_t;

/// A streaming HTTP body.
///
/// This is used both for sending requests (with `hyper_request_set_body`) and
/// for receiving responses (with `hyper_response_body`).
///
/// For outgoing request bodies, call `hyper_body_set_data_func` to provide the
/// data.
///
/// For incoming response bodies, call `hyper_body_data` to get a task that will
/// yield a chunk of data each time it is polled. That task must be then be
/// added to the executor with `hyper_executor_push`.
///
/// Methods:
///
/// - hyper_body_new:           Create a new “empty” body.
/// - hyper_body_set_userdata:  Set userdata on this body, which will be passed to callback functions.
/// - hyper_body_set_data_func: Set the data callback for this body.
/// - hyper_body_data:          Creates a task that will poll a response body for the next buffer of data.
/// - hyper_body_foreach:       Creates a task to execute the callback with each body chunk received.
/// - hyper_body_free:          Free a body.
pub struct hyper_body(pub(super) IncomingBody);

/// A buffer of bytes that is sent or received on a `hyper_body`.
///
/// Obtain one of these in the callback of `hyper_body_foreach` or by receiving
/// a task of type `HYPER_TASK_BUF` from `hyper_executor_poll` (after calling
/// `hyper_body_data` and pushing the resulting task).
///
/// Methods:
///
/// - hyper_buf_bytes: Get a pointer to the bytes in this buffer.
/// - hyper_buf_copy:  Create a new hyper_buf * by copying the provided bytes.
/// - hyper_buf_free:  Free this buffer.
/// - hyper_buf_len:   Get the length of the bytes this buffer contains.
pub struct hyper_buf(pub(crate) Bytes);

pub(crate) struct UserBody {
    data_func: hyper_body_data_callback,
    userdata: *mut c_void,
}

// ===== Body =====

type hyper_body_foreach_callback = extern "C" fn(*mut c_void, *const hyper_buf) -> c_int;

type hyper_body_data_callback =
    extern "C" fn(*mut c_void, *mut hyper_context<'_>, *mut *mut hyper_buf) -> c_int;

ffi_fn! {
    /// Creates a new "empty" body.
    ///
    /// If not configured, this body acts as an empty payload.
    ///
    /// To avoid a memory leak, the body must eventually be consumed by
    /// `hyper_body_free`, `hyper_body_foreach`, or `hyper_request_set_body`.
    fn hyper_body_new() -> *mut hyper_body {
        Box::into_raw(Box::new(hyper_body(IncomingBody::ffi())))
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Free a body.
    ///
    /// This should only be used if the request isn't consumed by
    /// `hyper_body_foreach` or `hyper_request_set_body`.
    fn hyper_body_free(body: *mut hyper_body) {
        drop(non_null!(Box::from_raw(body) ?= ()));
    }
}

ffi_fn! {
    /// Creates a task that will poll a response body for the next buffer of data.
    ///
    /// The task may have different types depending on the outcome:
    ///
    /// - `HYPER_TASK_BUF`: Success, and more data was received.
    /// - `HYPER_TASK_ERROR`: An error retrieving the data.
    /// - `HYPER_TASK_EMPTY`: The body has finished streaming data.
    ///
    /// When the application receives the task from `hyper_executor_poll`,
    /// if the task type is `HYPER_TASK_BUF`, it should cast the task to
    /// `hyper_buf *` and consume all the bytes in the buffer. Then
    /// the application should call `hyper_body_data` again for the same
    /// `hyper_body *`, to create a task for the next buffer of data.
    /// Repeat until the polled task type is `HYPER_TASK_ERROR` or
    /// `HYPER_TASK_EMPTY`.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`, or taken ownership of by `hyper_executor_push`
    /// without subsequently being given back by `hyper_executor_poll`.
    ///
    /// This does not consume the `hyper_body *`, so it may be used again.
    /// However, the `hyper_body *` MUST NOT be used or freed until the
    /// related task is returned from `hyper_executor_poll`.
    ///
    /// For a more convenient method, see also `hyper_body_foreach`.
    fn hyper_body_data(body: *mut hyper_body) -> *mut hyper_task {
        // This doesn't take ownership of the Body, so don't allow destructor
        let mut body = ManuallyDrop::new(non_null!(Box::from_raw(body) ?= ptr::null_mut()));

        Box::into_raw(hyper_task::boxed(async move {
            loop {
                match body.0.frame().await {
                    Some(Ok(frame)) => {
                        if let Ok(data) = frame.into_data() {
                            return Ok(Some(hyper_buf(data)));
                        } else {
                            continue;
                        }
                    },
                    Some(Err(e)) => return Err(e),
                    None => return Ok(None),
                }
            }
        }))
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Creates a task to execute the callback with each body chunk received.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`, or taken ownership of by `hyper_executor_push`
    /// without subsequently being given back by `hyper_executor_poll`.
    ///
    /// The `hyper_buf` pointer is only a borrowed reference. It cannot live outside
    /// the execution of the callback. You must make a copy of the bytes to retain them.
    ///
    /// The callback should return `HYPER_ITER_CONTINUE` to continue iterating
    /// chunks as they are received, or `HYPER_ITER_BREAK` to cancel. Each
    /// invocation of the callback must consume all the bytes it is provided.
    /// There is no mechanism to signal to Hyper that only a subset of bytes were
    /// consumed.
    ///
    /// This will consume the `hyper_body *`, you shouldn't use it anymore or free it.
    fn hyper_body_foreach(body: *mut hyper_body, func: hyper_body_foreach_callback, userdata: *mut c_void) -> *mut hyper_task {
        let mut body = non_null!(Box::from_raw(body) ?= ptr::null_mut());
        let userdata = UserDataPointer(userdata);

        Box::into_raw(hyper_task::boxed(async move {
            let _ = &userdata;
            while let Some(item) = body.0.frame().await {
                let frame = item?;
                if let Ok(chunk) = frame.into_data() {
                    if HYPER_ITER_CONTINUE != func(userdata.0, &hyper_buf(chunk)) {
                        return Err(crate::Error::new_user_aborted_by_callback());
                    }
                }
            }
            Ok(())
        }))
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Set userdata on this body, which will be passed to callback functions.
    fn hyper_body_set_userdata(body: *mut hyper_body, userdata: *mut c_void) {
        let b = non_null!(&mut *body ?= ());
        b.0.as_ffi_mut().userdata = userdata;
    }
}

ffi_fn! {
    /// Set the outgoing data callback for this body.
    ///
    /// The callback is called each time hyper needs to send more data for the
    /// body. It is passed the value from `hyper_body_set_userdata`.
    ///
    /// If there is data available, the `hyper_buf **` argument should be set
    /// to a `hyper_buf *` containing the data, and `HYPER_POLL_READY` should
    /// be returned.
    ///
    /// Returning `HYPER_POLL_READY` while the `hyper_buf **` argument points
    /// to `NULL` will indicate the body has completed all data.
    ///
    /// If there is more data to send, but it isn't yet available, a
    /// `hyper_waker` should be saved from the `hyper_context *` argument, and
    /// `HYPER_POLL_PENDING` should be returned. You must wake the saved waker
    /// to signal the task when data is available.
    ///
    /// If some error has occurred, you can return `HYPER_POLL_ERROR` to abort
    /// the body.
    fn hyper_body_set_data_func(body: *mut hyper_body, func: hyper_body_data_callback) {
        let b = non_null!{ &mut *body ?= () };
        b.0.as_ffi_mut().data_func = func;
    }
}

// ===== impl UserBody =====

impl UserBody {
    pub(crate) fn new() -> UserBody {
        UserBody {
            data_func: data_noop,
            userdata: std::ptr::null_mut(),
        }
    }

    pub(crate) fn poll_data(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<crate::Result<Frame<Bytes>>>> {
        let mut out = std::ptr::null_mut();
        match (self.data_func)(self.userdata, hyper_context::wrap(cx), &mut out) {
            super::task::HYPER_POLL_READY => {
                if out.is_null() {
                    Poll::Ready(None)
                } else {
                    let buf = unsafe { Box::from_raw(out) };
                    Poll::Ready(Some(Ok(Frame::data(buf.0))))
                }
            }
            super::task::HYPER_POLL_PENDING => Poll::Pending,
            super::task::HYPER_POLL_ERROR => {
                Poll::Ready(Some(Err(crate::Error::new_body_write_aborted())))
            }
            unexpected => Poll::Ready(Some(Err(crate::Error::new_body_write(format!(
                "unexpected hyper_body_data_func return code {}",
                unexpected
            ))))),
        }
    }
}

/// cbindgen:ignore
extern "C" fn data_noop(
    _userdata: *mut c_void,
    _: *mut hyper_context<'_>,
    _: *mut *mut hyper_buf,
) -> c_int {
    super::task::HYPER_POLL_READY
}

unsafe impl Send for UserBody {}
unsafe impl Sync for UserBody {}

// ===== Bytes =====

ffi_fn! {
    /// Create a new `hyper_buf *` by copying the provided bytes.
    ///
    /// This makes an owned copy of the bytes, so the `buf` argument can be
    /// freed (with `hyper_buf_free`) or changed afterwards.
    ///
    /// To avoid a memory leak, the copy must eventually be consumed by
    /// `hyper_buf_free`.
    ///
    /// This returns `NULL` if allocating a new buffer fails.
    fn hyper_buf_copy(buf: *const u8, len: size_t) -> *mut hyper_buf {
        let slice = unsafe {
            std::slice::from_raw_parts(buf, len)
        };
        Box::into_raw(Box::new(hyper_buf(Bytes::copy_from_slice(slice))))
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Get a pointer to the bytes in this buffer.
    ///
    /// This should be used in conjunction with `hyper_buf_len` to get the length
    /// of the bytes data.
    ///
    /// This pointer is borrowed data, and not valid once the `hyper_buf` is
    /// consumed/freed.
    fn hyper_buf_bytes(buf: *const hyper_buf) -> *const u8 {
        unsafe { (*buf).0.as_ptr() }
    } ?= ptr::null()
}

ffi_fn! {
    /// Get the length of the bytes this buffer contains.
    fn hyper_buf_len(buf: *const hyper_buf) -> size_t {
        unsafe { (*buf).0.len() }
    }
}

ffi_fn! {
    /// Free this buffer.
    ///
    /// This should be used for any buffer once it is no longer needed.
    fn hyper_buf_free(buf: *mut hyper_buf) {
        drop(unsafe { Box::from_raw(buf) });
    }
}

unsafe impl AsTaskType for hyper_buf {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_BUF
    }
}
