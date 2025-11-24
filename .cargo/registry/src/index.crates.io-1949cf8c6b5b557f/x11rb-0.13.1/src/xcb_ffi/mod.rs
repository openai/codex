//! A FFI-based connection to an X11 server, using libxcb.
//!
//! This module is only available when the `allow-unsafe-code` feature is enabled.

use std::ffi::CStr;
use std::io::{Error as IOError, ErrorKind, IoSlice};
use std::os::raw::c_int;
#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::ptr::{null, null_mut};
use std::sync::{atomic::Ordering, Mutex};

use libc::c_void;

use crate::connection::{
    compute_length_field, Connection, ReplyOrError, RequestConnection, RequestKind,
};
use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
use crate::errors::DisplayParsingError;
pub use crate::errors::{ConnectError, ConnectionError, ParseError, ReplyError, ReplyOrIdError};
use crate::extension_manager::ExtensionManager;
use crate::protocol::xproto::Setup;
use crate::utils::{CSlice, RawFdContainer};
use crate::x11_utils::{ExtensionInformation, TryParse, TryParseFd};

use x11rb_protocol::{DiscardMode, SequenceNumber};

mod atomic_u64;
mod pending_errors;
mod raw_ffi;

use atomic_u64::AtomicU64;
#[cfg(all(not(test), feature = "dl-libxcb"))]
pub use raw_ffi::libxcb_library::load_libxcb;

type Buffer = <XCBConnection as RequestConnection>::Buf;
/// The raw bytes of an event received by [`XCBConnection`] and its sequence number.
pub type RawEventAndSeqNumber = x11rb_protocol::RawEventAndSeqNumber<Buffer>;
/// A combination of a buffer and a list of file descriptors for use by [`XCBConnection`].
pub type BufWithFds = crate::connection::BufWithFds<Buffer>;

/// A connection to an X11 server.
///
/// This type wraps `*mut xcb_connection_t` that is provided by libxcb. It provides a rust
/// interface to this C library.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub struct XCBConnection {
    conn: raw_ffi::XcbConnectionWrapper,
    setup: Setup,
    ext_mgr: Mutex<ExtensionManager>,
    errors: pending_errors::PendingErrors,
    maximum_sequence_received: AtomicU64,
}

impl XCBConnection {
    unsafe fn connection_error_from_connection(
        c: *mut raw_ffi::xcb_connection_t,
    ) -> ConnectionError {
        Self::connection_error_from_c_error(raw_ffi::xcb_connection_has_error(c))
    }

    fn connection_error_from_c_error(error: c_int) -> ConnectionError {
        use crate::xcb_ffi::raw_ffi::connection_errors::*;

        assert_ne!(error, 0);
        match error {
            ERROR => IOError::new(ErrorKind::Other, ConnectionError::UnknownError).into(),
            EXT_NOTSUPPORTED => ConnectionError::UnsupportedExtension,
            MEM_INSUFFICIENT => ConnectionError::InsufficientMemory,
            REQ_LEN_EXCEED => ConnectionError::MaximumRequestLengthExceeded,
            FDPASSING_FAILED => ConnectionError::FdPassingFailed,
            _ => ConnectionError::UnknownError,
            // Not possible here: PARSE_ERR, INVALID_SCREEN
        }
    }

    fn connect_error_from_c_error(error: c_int) -> ConnectError {
        use crate::xcb_ffi::raw_ffi::connection_errors::*;

        assert_ne!(error, 0);
        match error {
            ERROR => IOError::new(ErrorKind::Other, ConnectionError::UnknownError).into(),
            MEM_INSUFFICIENT => ConnectError::InsufficientMemory,
            PARSE_ERR => DisplayParsingError::Unknown.into(),
            INVALID_SCREEN => ConnectError::InvalidScreen,
            _ => ConnectError::UnknownError,
            // Not possible here: EXT_NOTSUPPORTED, REQ_LEN_EXCEED, FDPASSING_FAILED
        }
    }

    /// Establish a new connection to an X11 server.
    ///
    /// If a `dpy_name` is provided, it describes the display that should be connected to, for
    /// example `127.0.0.1:1`. If no value is provided, the `$DISPLAY` environment variable is
    /// used.
    pub fn connect(dpy_name: Option<&CStr>) -> Result<(XCBConnection, usize), ConnectError> {
        use libc::c_int;
        unsafe {
            let mut screen: c_int = 0;
            let dpy_ptr = dpy_name.map_or(null(), |s| s.as_ptr());
            let connection = raw_ffi::XcbConnectionWrapper::new(
                raw_ffi::xcb_connect(dpy_ptr, &mut screen),
                true,
            );
            let error = raw_ffi::xcb_connection_has_error(connection.as_ptr());
            if error != 0 {
                Err(Self::connect_error_from_c_error(error))
            } else {
                let setup = raw_ffi::xcb_get_setup(connection.as_ptr());
                let conn = XCBConnection {
                    // `xcb_connect` will never return null.
                    conn: connection,
                    setup: Self::parse_setup(setup)?,
                    ext_mgr: Default::default(),
                    errors: Default::default(),
                    maximum_sequence_received: AtomicU64::new(0),
                };
                Ok((conn, screen as usize))
            }
        }
    }

    /// Create a connection wrapper for a raw libxcb `xcb_connection_t`.
    ///
    /// `xcb_disconnect` is called on drop only if `should_drop` is `true`.
    /// If this function returns an `Err()` and `should_drop` was true, then
    /// `xcb_disconnect` was already called.
    ///
    /// # Safety
    ///
    /// If `should_drop` is `false`, the connection must live longer than the returned
    /// `XCBConnection`. If `should_drop` is `true`, the returned `XCBConnection` will
    /// take the ownership of the connection.
    pub unsafe fn from_raw_xcb_connection(
        ptr: *mut c_void,
        should_drop: bool,
    ) -> Result<XCBConnection, ConnectError> {
        let ptr = ptr as *mut raw_ffi::xcb_connection_t;
        let conn = raw_ffi::XcbConnectionWrapper::new(ptr, should_drop);
        let setup = raw_ffi::xcb_get_setup(ptr);
        Ok(XCBConnection {
            conn,
            setup: Self::parse_setup(setup)?,
            ext_mgr: Default::default(),
            errors: Default::default(),
            maximum_sequence_received: AtomicU64::new(0),
        })
    }

    unsafe fn parse_setup(setup: *const raw_ffi::xcb_setup_t) -> Result<Setup, ParseError> {
        use std::slice::from_raw_parts;

        // We know that the setup information has at least eight bytes.
        // Use a slice instead of Buffer::CSlice since we must not free() the xcb_setup_t that libxcb owns.
        let wrapper = from_raw_parts(setup as *const u8, 8);

        // The length field is in the last two bytes
        let length = u16::from_ne_bytes([wrapper[6], wrapper[7]]);

        // The length is in four-byte-units after the known header
        let length = usize::from(length) * 4 + 8;

        let slice = from_raw_parts(wrapper.as_ptr(), length);
        let result = Setup::try_parse(slice)?.0;

        Ok(result)
    }

    // Slince the warning about ioslice.len().try_into().unwrap(). The target type is sometimes
    // usize (where this warning is correct) and sometimes c_int (where we need the conversion). We
    // need this here due to https://github.com/rust-lang/rust/issues/60681.
    #[allow(clippy::useless_conversion)]
    fn send_request(
        &self,
        bufs: &[IoSlice<'_>],
        fds: Vec<RawFdContainer>,
        has_reply: bool,
        reply_has_fds: bool,
    ) -> Result<SequenceNumber, ConnectionError> {
        let mut storage = Default::default();
        let new_bufs = compute_length_field(self, bufs, &mut storage)?;

        // Now wrap the buffers with IoSlice
        let mut new_bufs_ffi = Vec::with_capacity(2 + new_bufs.len());
        // XCB wants to access bufs[-1] and bufs[-2], so we need to add two empty items in front.
        new_bufs_ffi.push(raw_ffi::iovec {
            iov_base: null_mut(),
            iov_len: 0,
        });
        new_bufs_ffi.push(raw_ffi::iovec {
            iov_base: null_mut(),
            iov_len: 0,
        });
        new_bufs_ffi.extend(new_bufs.iter().map(|ioslice| raw_ffi::iovec {
            iov_base: ioslice.as_ptr() as _,
            iov_len: ioslice.len().try_into().unwrap(),
        }));

        // Set up the information that libxcb needs
        let protocol_request = raw_ffi::xcb_protocol_request_t {
            count: new_bufs.len(),
            ext: null_mut(), // Not needed since we always use raw
            opcode: 0,
            isvoid: u8::from(!has_reply),
        };
        let mut flags = raw_ffi::send_request_flags::RAW;
        assert!(has_reply || !reply_has_fds);
        flags |= raw_ffi::send_request_flags::CHECKED;
        if reply_has_fds {
            flags |= raw_ffi::send_request_flags::REPLY_FDS;
        }

        let seqno = if fds.is_empty() {
            unsafe {
                raw_ffi::xcb_send_request64(
                    self.conn.as_ptr(),
                    flags,
                    &mut new_bufs_ffi[2],
                    &protocol_request,
                )
            }
        } else {
            #[cfg(unix)]
            {
                // Convert the FDs into an array of ints. libxcb will close the FDs.
                let mut fds: Vec<_> = fds.into_iter().map(RawFdContainer::into_raw_fd).collect();
                let num_fds = fds.len().try_into().unwrap();
                let fds_ptr = fds.as_mut_ptr();
                unsafe {
                    raw_ffi::xcb_send_request_with_fds64(
                        self.conn.as_ptr(),
                        flags,
                        &mut new_bufs_ffi[2],
                        &protocol_request,
                        num_fds,
                        fds_ptr,
                    )
                }
            }
            #[cfg(not(unix))]
            {
                unreachable!("it is not possible to create a `RawFdContainer` on non-unix");
            }
        };
        if seqno == 0 {
            unsafe { Err(Self::connection_error_from_connection(self.conn.as_ptr())) }
        } else {
            Ok(seqno)
        }
    }

    /// Check if the underlying XCB connection is in an error state.
    pub fn has_error(&self) -> Option<ConnectionError> {
        unsafe {
            let error = raw_ffi::xcb_connection_has_error(self.conn.as_ptr());
            if error == 0 {
                None
            } else {
                Some(Self::connection_error_from_c_error(error))
            }
        }
    }

    /// Get access to the raw libxcb `xcb_connection_t`.
    ///
    /// The returned pointer is valid for as long as the original object was not dropped. No
    /// ownerhsip is transferred.
    pub fn get_raw_xcb_connection(&self) -> *mut c_void {
        self.conn.as_ptr() as _
    }

    /// Check if a reply to the given request already received.
    ///
    /// Return Err(()) when the reply was not yet received. Returns Ok(None) when there can be no
    /// reply. Returns Ok(buffer) with the reply if there is one (this buffer can be an error or a
    /// reply).
    fn poll_for_reply(&self, sequence: SequenceNumber) -> Result<Option<CSlice>, ()> {
        unsafe {
            let mut reply = null_mut();
            let mut error = null_mut();
            let found =
                raw_ffi::xcb_poll_for_reply64(self.conn.as_ptr(), sequence, &mut reply, &mut error);
            if found == 0 {
                return Err(());
            }
            assert_eq!(found, 1);
            match (reply.is_null(), error.is_null()) {
                (true, true) => Ok(None),
                (true, false) => Ok(Some(self.wrap_error(error as _, sequence))),
                (false, true) => Ok(Some(self.wrap_reply(reply as _, sequence))),
                (false, false) => unreachable!(),
            }
        }
    }

    unsafe fn wrap_reply(&self, reply: *const u8, sequence: SequenceNumber) -> CSlice {
        // Update our "max sequence number received" field
        let _ = self
            .maximum_sequence_received
            .fetch_max(sequence, Ordering::Relaxed);

        let header = CSlice::new(reply, 32);

        let length_field = u32::from_ne_bytes(header[4..8].try_into().unwrap());
        let length_field: usize = length_field
            .try_into()
            .expect("usize should have at least 32 bits");

        let length = 32 + length_field * 4;
        CSlice::new(header.into_ptr(), length)
    }

    unsafe fn wrap_error(&self, error: *const u8, sequence: SequenceNumber) -> CSlice {
        // Update our "max sequence number received" field
        let _ = self
            .maximum_sequence_received
            .fetch_max(sequence, Ordering::Relaxed);

        CSlice::new(error, 32)
    }

    unsafe fn wrap_event(&self, event: *mut u8) -> Result<RawEventAndSeqNumber, ParseError> {
        let header = CSlice::new(event, 36);
        let mut length = 32;
        // XCB inserts a uint32_t with the sequence number after the first 32 bytes.
        let seqno = u32::from_ne_bytes([header[32], header[33], header[34], header[35]]);
        let seqno = self.reconstruct_full_sequence(seqno);

        // The first byte contains the event type, check for XGE events
        if (*event & 0x7f) == super::protocol::xproto::GE_GENERIC_EVENT {
            // Read the length field of the event to get its length
            let length_field = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
            let length_field: usize = length_field
                .try_into()
                .or(Err(ParseError::ConversionFailed))?;
            length += length_field * 4;
            // Discard the `full_sequence` field inserted by xcb at
            // the 32-byte boundary.
            std::ptr::copy(event.add(36), event.add(32), length_field * 4);
        }
        Ok((CSlice::new(header.into_ptr(), length), seqno))
    }

    /// Reconstruct a full sequence number based on a partial value.
    ///
    /// The assumption for the algorithm here is that the given sequence number was received
    /// recently. Thus, the maximum sequence number that was received so far is used to fill in the
    /// missing bytes for the result.
    fn reconstruct_full_sequence(&self, seqno: u32) -> SequenceNumber {
        reconstruct_full_sequence_impl(
            self.maximum_sequence_received.load(Ordering::Relaxed),
            seqno,
        )
    }
}

impl RequestConnection for XCBConnection {
    type Buf = CSlice;

    fn send_request_with_reply<R>(
        &self,
        bufs: &[IoSlice<'_>],
        fds: Vec<RawFdContainer>,
    ) -> Result<Cookie<'_, Self, R>, ConnectionError>
    where
        R: TryParse,
    {
        Ok(Cookie::new(
            self,
            self.send_request(bufs, fds, true, false)?,
        ))
    }

    fn send_request_with_reply_with_fds<R>(
        &self,
        bufs: &[IoSlice<'_>],
        fds: Vec<RawFdContainer>,
    ) -> Result<CookieWithFds<'_, Self, R>, ConnectionError>
    where
        R: TryParseFd,
    {
        Ok(CookieWithFds::new(
            self,
            self.send_request(bufs, fds, true, true)?,
        ))
    }

    fn send_request_without_reply(
        &self,
        bufs: &[IoSlice<'_>],
        fds: Vec<RawFdContainer>,
    ) -> Result<VoidCookie<'_, Self>, ConnectionError> {
        Ok(VoidCookie::new(
            self,
            self.send_request(bufs, fds, false, false)?,
        ))
    }

    fn discard_reply(&self, sequence: SequenceNumber, _kind: RequestKind, mode: DiscardMode) {
        match mode {
            DiscardMode::DiscardReplyAndError => unsafe {
                // libxcb can throw away everything for us
                raw_ffi::xcb_discard_reply64(self.conn.as_ptr(), sequence);
            },
            // We have to check for errors ourselves
            DiscardMode::DiscardReply => self.errors.discard_reply(sequence),
        }
    }

    fn prefetch_extension_information(
        &self,
        extension_name: &'static str,
    ) -> Result<(), ConnectionError> {
        self.ext_mgr
            .lock()
            .unwrap()
            .prefetch_extension_information(self, extension_name)
    }

    fn extension_information(
        &self,
        extension_name: &'static str,
    ) -> Result<Option<ExtensionInformation>, ConnectionError> {
        self.ext_mgr
            .lock()
            .unwrap()
            .extension_information(self, extension_name)
    }

    fn wait_for_reply_or_raw_error(
        &self,
        sequence: SequenceNumber,
    ) -> Result<ReplyOrError<CSlice>, ConnectionError> {
        unsafe {
            let mut error = null_mut();
            let reply = raw_ffi::xcb_wait_for_reply64(self.conn.as_ptr(), sequence, &mut error);
            match (reply.is_null(), error.is_null()) {
                (true, true) => Err(Self::connection_error_from_connection(self.conn.as_ptr())),
                (false, true) => Ok(ReplyOrError::Reply(self.wrap_reply(reply as _, sequence))),
                (true, false) => Ok(ReplyOrError::Error(self.wrap_error(error as _, sequence))),
                // At least one of these pointers must be NULL.
                (false, false) => unreachable!(),
            }
        }
    }

    fn wait_for_reply(&self, sequence: SequenceNumber) -> Result<Option<CSlice>, ConnectionError> {
        match self.wait_for_reply_or_raw_error(sequence)? {
            ReplyOrError::Reply(reply) => Ok(Some(reply)),
            ReplyOrError::Error(error) => {
                self.errors.append_error((sequence, error));
                Ok(None)
            }
        }
    }

    #[cfg(unix)]
    fn wait_for_reply_with_fds_raw(
        &self,
        sequence: SequenceNumber,
    ) -> Result<ReplyOrError<BufWithFds, Buffer>, ConnectionError> {
        let buffer = match self.wait_for_reply_or_raw_error(sequence)? {
            ReplyOrError::Reply(reply) => reply,
            ReplyOrError::Error(error) => return Ok(ReplyOrError::Error(error)),
        };

        // Get a pointer to the array of integers where libxcb saved the FD numbers.
        // libxcb saves the list of FDs after the data of the reply. Since the reply's
        // length is encoded in "number of 4 bytes block", the following pointer is aligned
        // correctly (if malloc() returned an aligned chunk, which it does).
        #[allow(clippy::cast_ptr_alignment)]
        let fd_ptr = (unsafe { buffer.as_ptr().add(buffer.len()) }) as *const RawFd;

        // The number of FDs is in the second byte (= buffer[1]) in all replies.
        let fd_slice = unsafe { std::slice::from_raw_parts(fd_ptr, usize::from(buffer[1])) };
        let fd_vec = fd_slice
            .iter()
            .map(|&fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .collect();

        Ok(ReplyOrError::Reply((buffer, fd_vec)))
    }

    #[cfg(not(unix))]
    fn wait_for_reply_with_fds_raw(
        &self,
        _sequence: SequenceNumber,
    ) -> Result<ReplyOrError<BufWithFds, Buffer>, ConnectionError> {
        unimplemented!("FD passing is currently only implemented on Unix-like systems")
    }

    fn check_for_raw_error(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Option<Buffer>, ConnectionError> {
        let cookie = raw_ffi::xcb_void_cookie_t {
            sequence: sequence as _,
        };
        let error = unsafe { raw_ffi::xcb_request_check(self.conn.as_ptr(), cookie) };
        if error.is_null() {
            Ok(None)
        } else {
            unsafe { Ok(Some(self.wrap_error(error as _, sequence))) }
        }
    }

    fn maximum_request_bytes(&self) -> usize {
        4 * unsafe { raw_ffi::xcb_get_maximum_request_length(self.conn.as_ptr()) as usize }
    }

    fn prefetch_maximum_request_bytes(&self) {
        unsafe { raw_ffi::xcb_prefetch_maximum_request_length(self.conn.as_ptr()) };
    }

    fn parse_error(&self, error: &[u8]) -> Result<crate::x11_utils::X11Error, ParseError> {
        let ext_mgr = self.ext_mgr.lock().unwrap();
        crate::x11_utils::X11Error::try_parse(error, &*ext_mgr)
    }

    fn parse_event(&self, event: &[u8]) -> Result<crate::protocol::Event, ParseError> {
        let ext_mgr = self.ext_mgr.lock().unwrap();
        crate::protocol::Event::parse(event, &*ext_mgr)
    }
}

impl Connection for XCBConnection {
    fn wait_for_raw_event_with_sequence(&self) -> Result<RawEventAndSeqNumber, ConnectionError> {
        if let Some(error) = self.errors.get(self) {
            return Ok((error.1, error.0));
        }
        unsafe {
            let event = raw_ffi::xcb_wait_for_event(self.conn.as_ptr());
            if event.is_null() {
                return Err(Self::connection_error_from_connection(self.conn.as_ptr()));
            }
            Ok(self.wrap_event(event as _)?)
        }
    }

    fn poll_for_raw_event_with_sequence(
        &self,
    ) -> Result<Option<RawEventAndSeqNumber>, ConnectionError> {
        if let Some(error) = self.errors.get(self) {
            return Ok(Some((error.1, error.0)));
        }
        unsafe {
            let event = raw_ffi::xcb_poll_for_event(self.conn.as_ptr());
            if event.is_null() {
                let err = raw_ffi::xcb_connection_has_error(self.conn.as_ptr());
                if err == 0 {
                    return Ok(None);
                } else {
                    return Err(Self::connection_error_from_c_error(err));
                }
            }
            Ok(Some(self.wrap_event(event as _)?))
        }
    }

    fn flush(&self) -> Result<(), ConnectionError> {
        // xcb_flush() returns 0 if the connection is in (or just entered) an error state, else 1.
        let res = unsafe { raw_ffi::xcb_flush(self.conn.as_ptr()) };
        if res != 0 {
            Ok(())
        } else {
            unsafe { Err(Self::connection_error_from_connection(self.conn.as_ptr())) }
        }
    }

    fn generate_id(&self) -> Result<u32, ReplyOrIdError> {
        unsafe {
            let id = raw_ffi::xcb_generate_id(self.conn.as_ptr());
            // XCB does not document the behaviour of `xcb_generate_id` when
            // there is an error. Looking at its source code it seems that it
            // returns `-1` (presumably `u32::max_value()`).
            if id == u32::max_value() {
                Err(Self::connection_error_from_connection(self.conn.as_ptr()).into())
            } else {
                Ok(id)
            }
        }
    }

    fn setup(&self) -> &Setup {
        &self.setup
    }
}

#[cfg(unix)]
impl AsRawFd for XCBConnection {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { raw_ffi::xcb_get_file_descriptor(self.conn.as_ptr()) }
    }
}

#[cfg(unix)]
impl AsFd for XCBConnection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

// SAFETY: We provide a valid xcb_connection_t that is valid for as long as required by the trait.
unsafe impl as_raw_xcb_connection::AsRawXcbConnection for XCBConnection {
    fn as_raw_xcb_connection(&self) -> *mut as_raw_xcb_connection::xcb_connection_t {
        self.get_raw_xcb_connection().cast()
    }
}

/// Reconstruct a partial sequence number based on a recently received 'full' sequence number.
///
/// The new sequence number may be before or after the `recent` sequence number.
fn reconstruct_full_sequence_impl(recent: SequenceNumber, value: u32) -> SequenceNumber {
    // Expand 'value' to a full sequence number. The high bits are copied from 'recent'.
    let u32_max = SequenceNumber::from(u32::max_value());
    let expanded_value = SequenceNumber::from(value) | (recent & !u32_max);

    // The "step size" is the difference between two sequence numbers that cannot be told apart
    // from their truncated value.
    let step: SequenceNumber = SequenceNumber::from(1u8) << 32;

    // There are three possible values for the returned sequence number:
    // - The extended value
    // - The extended value plus one step
    // - The extended value minus one step
    // Pick the value out of the possible values that is closest to `recent`.
    let result = [
        expanded_value,
        expanded_value + step,
        expanded_value.wrapping_sub(step),
    ]
    .iter()
    .copied()
    .min_by_key(|&value| {
        if value > recent {
            value - recent
        } else {
            recent - value
        }
    })
    .unwrap();
    // Just because: Check that the result matches the passed-in value in the low bits
    assert_eq!(
        result & SequenceNumber::from(u32::max_value()),
        SequenceNumber::from(value),
    );
    result
}

#[cfg(test)]
mod test {
    use super::XCBConnection;
    use std::ffi::CString;

    #[test]
    fn xcb_connect_smoke_test() {
        // in cfg(test), raw_ffi does not call XCB, but instead uses a mock. This test calls into
        // that mock and tests a bit of XCBConnection.

        let str = CString::new("display name").unwrap();
        let (_conn, screen) = XCBConnection::connect(Some(&str)).expect("Failed to 'connect'");
        assert_eq!(screen, 0);
    }

    #[test]
    fn reconstruct_full_sequence() {
        use super::reconstruct_full_sequence_impl;
        let max32 = u32::max_value();
        let max32_: u64 = max32.into();
        let max32_p1 = max32_ + 1;
        let large_offset = max32_p1 * u64::from(u16::max_value());
        for &(recent, value, expected) in &[
            (0, 0, 0),
            (0, 10, 10),
            // This one is a special case: Technically, -1 is closer and should be reconstructed,
            // but -1 does not fit into an unsigned integer.
            (0, max32, max32_),
            (max32_, 0, max32_p1),
            (max32_, 10, max32_p1 + 10),
            (max32_, max32, max32_),
            (max32_p1, 0, max32_p1),
            (max32_p1, 10, max32_p1 + 10),
            (max32_p1, max32, max32_),
            (large_offset | 0xdead_cafe, 0, large_offset + max32_p1),
            (large_offset | 0xdead_cafe, max32, large_offset + max32_),
            (0xabcd_1234_5678, 0xf000_0000, 0xabcc_f000_0000),
            (0xabcd_8765_4321, 0xf000_0000, 0xabcd_f000_0000),
        ] {
            let actual = reconstruct_full_sequence_impl(recent, value);
            assert_eq!(
                actual, expected,
                "reconstruct({:x}, {:x}) == {:x}, but was {:x}",
                recent, value, expected, actual,
            );
        }
    }
}
