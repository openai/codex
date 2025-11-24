//! Implement RequestConnection for wrappers around other connection types, e.g.
//! `Box<Connection>`.

use std::io::IoSlice;

use x11rb_protocol::x11_utils::{ReplyFDsRequest, ReplyRequest, VoidRequest};
use x11rb_protocol::{DiscardMode, RawEventAndSeqNumber, SequenceNumber};

use crate::connection::{
    BufWithFds, Connection, EventAndSeqNumber, ReplyOrError, RequestConnection, RequestKind,
};
use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
use crate::errors::{ConnectionError, ParseError, ReplyError, ReplyOrIdError};
use crate::protocol::xproto::Setup;
use crate::protocol::Event;
use crate::utils::RawFdContainer;
use crate::x11_utils::{ExtensionInformation, TryParse, TryParseFd, X11Error};

macro_rules! impl_deref_request_connection_inner {
    () => {
        type Buf = C::Buf;

        fn send_request_with_reply<R>(
            &self,
            bufs: &[IoSlice<'_>],
            fds: Vec<RawFdContainer>,
        ) -> Result<Cookie<'_, Self, R>, ConnectionError>
        where
            R: TryParse,
        {
            (**self)
                .send_request_with_reply(bufs, fds)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn send_trait_request_with_reply<R>(
            &self,
            request: R,
        ) -> Result<Cookie<'_, Self, <R as ReplyRequest>::Reply>, ConnectionError>
        where
            R: ReplyRequest,
        {
            (**self)
                .send_trait_request_with_reply(request)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn send_request_with_reply_with_fds<R>(
            &self,
            bufs: &[IoSlice<'_>],
            fds: Vec<RawFdContainer>,
        ) -> Result<CookieWithFds<'_, Self, R>, ConnectionError>
        where
            R: TryParseFd,
        {
            (**self)
                .send_request_with_reply_with_fds(bufs, fds)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn send_trait_request_with_reply_with_fds<R>(
            &self,
            request: R,
        ) -> Result<CookieWithFds<'_, Self, R::Reply>, ConnectionError>
        where
            R: ReplyFDsRequest,
        {
            (**self)
                .send_trait_request_with_reply_with_fds(request)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn send_request_without_reply(
            &self,
            bufs: &[IoSlice<'_>],
            fds: Vec<RawFdContainer>,
        ) -> Result<VoidCookie<'_, Self>, ConnectionError> {
            (**self)
                .send_request_without_reply(bufs, fds)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn send_trait_request_without_reply<R>(
            &self,
            request: R,
        ) -> Result<VoidCookie<'_, Self>, ConnectionError>
        where
            R: VoidRequest,
        {
            (**self)
                .send_trait_request_without_reply(request)
                .map(|cookie| cookie.replace_connection(self))
        }

        fn discard_reply(&self, sequence: SequenceNumber, kind: RequestKind, mode: DiscardMode) {
            (**self).discard_reply(sequence, kind, mode)
        }

        fn prefetch_extension_information(
            &self,
            extension_name: &'static str,
        ) -> Result<(), ConnectionError> {
            (**self).prefetch_extension_information(extension_name)
        }

        fn extension_information(
            &self,
            extension_name: &'static str,
        ) -> Result<Option<ExtensionInformation>, ConnectionError> {
            (**self).extension_information(extension_name)
        }

        fn wait_for_reply_or_error(
            &self,
            sequence: SequenceNumber,
        ) -> Result<Self::Buf, ReplyError> {
            (**self).wait_for_reply_or_error(sequence)
        }

        fn wait_for_reply_or_raw_error(
            &self,
            sequence: SequenceNumber,
        ) -> Result<ReplyOrError<Self::Buf>, ConnectionError> {
            (**self).wait_for_reply_or_raw_error(sequence)
        }

        fn wait_for_reply(
            &self,
            sequence: SequenceNumber,
        ) -> Result<Option<Self::Buf>, ConnectionError> {
            (**self).wait_for_reply(sequence)
        }

        fn wait_for_reply_with_fds(
            &self,
            sequence: SequenceNumber,
        ) -> Result<BufWithFds<Self::Buf>, ReplyError> {
            (**self).wait_for_reply_with_fds(sequence)
        }

        fn wait_for_reply_with_fds_raw(
            &self,
            sequence: SequenceNumber,
        ) -> Result<ReplyOrError<BufWithFds<Self::Buf>, Self::Buf>, ConnectionError> {
            (**self).wait_for_reply_with_fds_raw(sequence)
        }

        fn check_for_error(&self, sequence: SequenceNumber) -> Result<(), ReplyError> {
            (**self).check_for_error(sequence)
        }

        fn check_for_raw_error(
            &self,
            sequence: SequenceNumber,
        ) -> Result<Option<Self::Buf>, ConnectionError> {
            (**self).check_for_raw_error(sequence)
        }

        fn prefetch_maximum_request_bytes(&self) {
            (**self).prefetch_maximum_request_bytes()
        }

        fn maximum_request_bytes(&self) -> usize {
            (**self).maximum_request_bytes()
        }

        fn parse_error(&self, error: &[u8]) -> Result<X11Error, ParseError> {
            (**self).parse_error(error)
        }

        fn parse_event(&self, event: &[u8]) -> Result<Event, ParseError> {
            (**self).parse_event(event)
        }
    };
}

macro_rules! impl_deref_connection_inner {
    () => {
        fn wait_for_event(&self) -> Result<Event, ConnectionError> {
            (**self).wait_for_event()
        }

        fn wait_for_raw_event(&self) -> Result<Self::Buf, ConnectionError> {
            (**self).wait_for_raw_event()
        }

        fn wait_for_event_with_sequence(&self) -> Result<EventAndSeqNumber, ConnectionError> {
            (**self).wait_for_event_with_sequence()
        }

        fn wait_for_raw_event_with_sequence(
            &self,
        ) -> Result<RawEventAndSeqNumber<Self::Buf>, ConnectionError> {
            (**self).wait_for_raw_event_with_sequence()
        }

        fn poll_for_event(&self) -> Result<Option<Event>, ConnectionError> {
            (**self).poll_for_event()
        }

        fn poll_for_raw_event(&self) -> Result<Option<Self::Buf>, ConnectionError> {
            (**self).poll_for_raw_event()
        }

        fn poll_for_event_with_sequence(
            &self,
        ) -> Result<Option<EventAndSeqNumber>, ConnectionError> {
            (**self).poll_for_event_with_sequence()
        }

        fn poll_for_raw_event_with_sequence(
            &self,
        ) -> Result<Option<RawEventAndSeqNumber<Self::Buf>>, ConnectionError> {
            (**self).poll_for_raw_event_with_sequence()
        }

        fn flush(&self) -> Result<(), ConnectionError> {
            (**self).flush()
        }

        fn setup(&self) -> &Setup {
            (**self).setup()
        }

        fn generate_id(&self) -> Result<u32, ReplyOrIdError> {
            (**self).generate_id()
        }
    };
}

macro_rules! impl_deref_connection {
    ($type:ty) => {
        impl<C: RequestConnection + ?Sized> RequestConnection for $type {
            impl_deref_request_connection_inner!();
        }
        impl<C: Connection + ?Sized> Connection for $type {
            impl_deref_connection_inner!();
        }
    };
}

impl_deref_connection!(&C);
impl_deref_connection!(&mut C);
impl_deref_connection!(Box<C>);
impl_deref_connection!(std::sync::Arc<C>);
impl_deref_connection!(std::rc::Rc<C>);

impl<C: RequestConnection + ToOwned + ?Sized> RequestConnection for std::borrow::Cow<'_, C> {
    impl_deref_request_connection_inner!();
}
impl<C: Connection + ToOwned + ?Sized> Connection for std::borrow::Cow<'_, C> {
    impl_deref_connection_inner!();
}
