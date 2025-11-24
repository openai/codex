//! Helper for implementing `RequestConnection::extension_information()`.

use std::collections::{hash_map::Entry as HashMapEntry, HashMap};

use crate::connection::RequestConnection;
use crate::cookie::Cookie;
use crate::errors::{ConnectionError, ReplyError};
use crate::protocol::xproto::{ConnectionExt, QueryExtensionReply};
use crate::x11_utils::{ExtInfoProvider, ExtensionInformation};

use x11rb_protocol::SequenceNumber;

/// Helper for implementing `RequestConnection::extension_information()`.
///
/// This helps with implementing `RequestConnection`. Most likely, you do not need this in your own
/// code, unless you really want to implement your own X11 connection.
#[derive(Debug, Default)]
pub struct ExtensionManager(HashMap<&'static str, CheckState>);

#[derive(Debug)]
enum CheckState {
    Prefetched(SequenceNumber),
    Present(ExtensionInformation),
    Missing,
    Error,
}

impl ExtensionManager {
    /// If the extension has not prefetched yet, sends a `QueryExtension`
    /// requests, adds a field to the hash map and returns a reference to it.
    fn prefetch_extension_information_aux<C: RequestConnection>(
        &mut self,
        conn: &C,
        extension_name: &'static str,
    ) -> Result<&mut CheckState, ConnectionError> {
        match self.0.entry(extension_name) {
            // Extension already checked, return the cached value
            HashMapEntry::Occupied(entry) => Ok(entry.into_mut()),
            HashMapEntry::Vacant(entry) => {
                crate::debug!(
                    "Prefetching information about '{}' extension",
                    extension_name
                );
                let cookie = conn.query_extension(extension_name.as_bytes())?;
                Ok(entry.insert(CheckState::Prefetched(cookie.into_sequence_number())))
            }
        }
    }

    /// Prefetchs an extension sending a `QueryExtension` without waiting for
    /// the reply.
    pub fn prefetch_extension_information<C: RequestConnection>(
        &mut self,
        conn: &C,
        extension_name: &'static str,
    ) -> Result<(), ConnectionError> {
        // We are not interested on the reference to the entry.
        let _ = self.prefetch_extension_information_aux(conn, extension_name)?;
        Ok(())
    }

    /// Insert an extension if you already have the information.
    pub fn insert_extension_information(
        &mut self,
        extension_name: &'static str,
        info: Option<ExtensionInformation>,
    ) {
        crate::debug!(
            "Inserting '{}' extension information directly: {:?}",
            extension_name,
            info
        );
        let state = match info {
            Some(info) => CheckState::Present(info),
            None => CheckState::Missing,
        };

        let _ = self.0.insert(extension_name, state);
    }

    /// An implementation of `RequestConnection::extension_information()`.
    ///
    /// The given connection is used for sending a `QueryExtension` request if needed.
    pub fn extension_information<C: RequestConnection>(
        &mut self,
        conn: &C,
        extension_name: &'static str,
    ) -> Result<Option<ExtensionInformation>, ConnectionError> {
        let _guard = crate::debug_span!("extension_information", extension_name).entered();
        let entry = self.prefetch_extension_information_aux(conn, extension_name)?;
        match entry {
            CheckState::Prefetched(sequence_number) => {
                crate::debug!(
                    "Waiting for QueryInfo reply for '{}' extension",
                    extension_name
                );
                match Cookie::<C, QueryExtensionReply>::new(conn, *sequence_number).reply() {
                    Err(err) => {
                        crate::warning!(
                            "Got error {:?} for QueryInfo reply for '{}' extension",
                            err,
                            extension_name
                        );
                        *entry = CheckState::Error;
                        match err {
                            ReplyError::ConnectionError(e) => Err(e),
                            // The X11 protocol specification does not specify any error
                            // for the QueryExtension request, so this should not happen.
                            ReplyError::X11Error(_) => Err(ConnectionError::UnknownError),
                        }
                    }
                    Ok(info) => {
                        if info.present {
                            let info = ExtensionInformation {
                                major_opcode: info.major_opcode,
                                first_event: info.first_event,
                                first_error: info.first_error,
                            };
                            crate::debug!("Extension '{}' is present: {:?}", extension_name, info);
                            *entry = CheckState::Present(info);
                            Ok(Some(info))
                        } else {
                            crate::debug!("Extension '{}' is not present", extension_name);
                            *entry = CheckState::Missing;
                            Ok(None)
                        }
                    }
                }
            }
            CheckState::Present(info) => Ok(Some(*info)),
            CheckState::Missing => Ok(None),
            CheckState::Error => Err(ConnectionError::UnknownError),
        }
    }
}

impl ExtInfoProvider for ExtensionManager {
    fn get_from_major_opcode(&self, major_opcode: u8) -> Option<(&str, ExtensionInformation)> {
        self.0
            .iter()
            .filter_map(|(name, state)| {
                if let CheckState::Present(info) = state {
                    Some((*name, *info))
                } else {
                    None
                }
            })
            .find(|(_, info)| info.major_opcode == major_opcode)
    }

    fn get_from_event_code(&self, event_code: u8) -> Option<(&str, ExtensionInformation)> {
        self.0
            .iter()
            .filter_map(|(name, state)| {
                if let CheckState::Present(info) = state {
                    if info.first_event <= event_code {
                        Some((*name, *info))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .max_by_key(|(_, info)| info.first_event)
    }

    fn get_from_error_code(&self, error_code: u8) -> Option<(&str, ExtensionInformation)> {
        self.0
            .iter()
            .filter_map(|(name, state)| {
                if let CheckState::Present(info) = state {
                    if info.first_error <= error_code {
                        Some((*name, *info))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .max_by_key(|(_, info)| info.first_error)
    }
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::io::IoSlice;

    use crate::connection::{BufWithFds, ReplyOrError, RequestConnection, RequestKind};
    use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
    use crate::errors::{ConnectionError, ParseError};
    use crate::utils::RawFdContainer;
    use crate::x11_utils::{ExtInfoProvider, ExtensionInformation, TryParse, TryParseFd};
    use x11rb_protocol::{DiscardMode, SequenceNumber};

    use super::{CheckState, ExtensionManager};

    struct FakeConnection(RefCell<SequenceNumber>);

    impl RequestConnection for FakeConnection {
        type Buf = Vec<u8>;

        fn send_request_with_reply<R>(
            &self,
            _bufs: &[IoSlice<'_>],
            _fds: Vec<RawFdContainer>,
        ) -> Result<Cookie<'_, Self, R>, ConnectionError>
        where
            R: TryParse,
        {
            Ok(Cookie::new(self, 1))
        }

        fn send_request_with_reply_with_fds<R>(
            &self,
            _bufs: &[IoSlice<'_>],
            _fds: Vec<RawFdContainer>,
        ) -> Result<CookieWithFds<'_, Self, R>, ConnectionError>
        where
            R: TryParseFd,
        {
            unimplemented!()
        }

        fn send_request_without_reply(
            &self,
            _bufs: &[IoSlice<'_>],
            _fds: Vec<RawFdContainer>,
        ) -> Result<VoidCookie<'_, Self>, ConnectionError> {
            unimplemented!()
        }

        fn discard_reply(&self, _sequence: SequenceNumber, _kind: RequestKind, _mode: DiscardMode) {
            unimplemented!()
        }

        fn prefetch_extension_information(
            &self,
            _extension_name: &'static str,
        ) -> Result<(), ConnectionError> {
            unimplemented!();
        }

        fn extension_information(
            &self,
            _extension_name: &'static str,
        ) -> Result<Option<ExtensionInformation>, ConnectionError> {
            unimplemented!()
        }

        fn wait_for_reply_or_raw_error(
            &self,
            sequence: SequenceNumber,
        ) -> Result<ReplyOrError<Vec<u8>>, ConnectionError> {
            // Code should only ask once for the reply to a request. Check that this is the case
            // (by requiring monotonically increasing sequence numbers here).
            let mut last = self.0.borrow_mut();
            assert!(
                *last < sequence,
                "Last sequence number that was awaited was {}, but now {}",
                *last,
                sequence
            );
            *last = sequence;
            // Then return an error, because that's what the #[test] below needs.
            Err(ConnectionError::UnknownError)
        }

        fn wait_for_reply(
            &self,
            _sequence: SequenceNumber,
        ) -> Result<Option<Vec<u8>>, ConnectionError> {
            unimplemented!()
        }

        fn wait_for_reply_with_fds_raw(
            &self,
            _sequence: SequenceNumber,
        ) -> Result<ReplyOrError<BufWithFds<Vec<u8>>, Vec<u8>>, ConnectionError> {
            unimplemented!()
        }

        fn check_for_raw_error(
            &self,
            _sequence: SequenceNumber,
        ) -> Result<Option<Vec<u8>>, ConnectionError> {
            unimplemented!()
        }

        fn maximum_request_bytes(&self) -> usize {
            0
        }

        fn prefetch_maximum_request_bytes(&self) {
            unimplemented!()
        }

        fn parse_error(&self, _error: &[u8]) -> Result<crate::x11_utils::X11Error, ParseError> {
            unimplemented!()
        }

        fn parse_event(&self, _event: &[u8]) -> Result<crate::protocol::Event, ParseError> {
            unimplemented!()
        }
    }

    #[test]
    fn test_double_await() {
        let conn = FakeConnection(RefCell::new(0));
        let mut ext_info = ExtensionManager::default();

        // Ask for an extension info. FakeConnection will return an error.
        match ext_info.extension_information(&conn, "whatever") {
            Err(ConnectionError::UnknownError) => {}
            r => panic!("Unexpected result: {:?}", r),
        }

        // Ask again for the extension information. ExtensionInformation should not try to get the
        // reply again, because that would just hang. Once upon a time, this caused a hang.
        match ext_info.extension_information(&conn, "whatever") {
            Err(ConnectionError::UnknownError) => {}
            r => panic!("Unexpected result: {:?}", r),
        }
    }

    #[test]
    fn test_info_provider() {
        let info = ExtensionInformation {
            major_opcode: 4,
            first_event: 5,
            first_error: 6,
        };

        let mut ext_info = ExtensionManager::default();
        let _ = ext_info.0.insert("prefetched", CheckState::Prefetched(42));
        let _ = ext_info.0.insert("present", CheckState::Present(info));
        let _ = ext_info.0.insert("missing", CheckState::Missing);
        let _ = ext_info.0.insert("error", CheckState::Error);

        assert_eq!(ext_info.get_from_major_opcode(4), Some(("present", info)));
        assert_eq!(ext_info.get_from_event_code(5), Some(("present", info)));
        assert_eq!(ext_info.get_from_error_code(6), Some(("present", info)));
    }
}
