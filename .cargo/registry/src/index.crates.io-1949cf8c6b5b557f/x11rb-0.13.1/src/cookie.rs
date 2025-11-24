//! Cookies are handles to future replies or errors from the X11 server.
//!
//! When sending a request, you get back a cookie. There are different cookies for different
//! kinds of requests.
//!
//! For requests without a reply, you get a [`VoidCookie`]. Requests with a reply are represented
//! by a [`Cookie`] or a [`CookieWithFds`] if the reply also contains file descriptors.
//! Additionally, there are two special cases for requests which generate more than one reply:
//! [`ListFontsWithInfoCookie`] and [`RecordEnableContextCookie`].
//!
//! # Handling X11 errors
//!
//! The X11 server can answer requests with an error packet for various reasons, e.g. because an
//! invalid window ID was given. There are three options what can be done with errors:
//!
//! - Errors can appear as X11 events in `wait_for_event()` (in XCB, this is called "unchecked")
//! - Errors can be checked for locally after a request was sent (in XCB, this is called "checked")
//! - Errors can be completely ignored (the closest analog in XCB would be `xcb_discard_reply()`)
//!
//! There is an additional difference between requests with and without replies.
//!
//! ## Requests without a reply
//!
//! For requests that do not have a reply, you get an instance of `VoidCookie` after sending the
//! request. The different behaviors can be achieved via interacting with this cookie as foolows:
//!
//! | What?           | How?                       |
//! | --------------- | -------------------------- |
//! | Treat as events | Just drop the cookie       |
//! | Check locally   | `VoidCookie::check`        |
//! | Ignore          | `VoidCookie::ignore_error` |
//!
//! ## Requests with a reply
//!
//! For requests with a reply, an additional option is what should happen to the reply. You can get
//! the reply, but any errors are still treated as events. This allows to centralise X11 error
//! handling a bit in case you only want to log errors.
//!
//! The following things can be done with the `Cookie` that you get after sending a request with an
//! error.
//!
//! | Reply  | Errors locally/ignored             | Errors as events          |
//! | ------ | ---------------------------------- | ------------------------- |
//! | Get    | `Cookie::reply`                    | `Cookie::reply_unchecked` |
//! | Ignore | `Cookie::discard_reply_and_errors` | Just drop the cookie      |

use std::marker::PhantomData;

use crate::connection::{BufWithFds, RequestConnection, RequestKind};
use crate::errors::{ConnectionError, ReplyError};
#[cfg(feature = "record")]
use crate::protocol::record::EnableContextReply;
use crate::protocol::xproto::ListFontsWithInfoReply;
use crate::x11_utils::{TryParse, TryParseFd};

use x11rb_protocol::{DiscardMode, SequenceNumber};

/// A handle to a possible error from the X11 server.
///
/// When sending a request for which no reply is expected, this library returns a `VoidCookie`.
/// This `VoidCookie` can then later be used to check if the X11 server sent an error.
///
/// See [crate::cookie#requests-without-a-reply] for infos on the different ways to handle X11
/// errors in response to a request.
#[derive(Debug)]
pub struct VoidCookie<'a, C>
where
    C: RequestConnection + ?Sized,
{
    connection: &'a C,
    sequence_number: SequenceNumber,
}

impl<'a, C> VoidCookie<'a, C>
where
    C: RequestConnection + ?Sized,
{
    /// Construct a new cookie.
    ///
    /// This function should only be used by implementations of
    /// `Connection::send_request_without_reply`.
    pub fn new(connection: &C, sequence_number: SequenceNumber) -> VoidCookie<'_, C> {
        VoidCookie {
            connection,
            sequence_number,
        }
    }

    /// Get the sequence number of the request that generated this cookie.
    pub fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    fn consume(self) -> (&'a C, SequenceNumber) {
        let result = (self.connection, self.sequence_number);
        std::mem::forget(self);
        result
    }

    /// Check if the original request caused an X11 error.
    pub fn check(self) -> Result<(), ReplyError> {
        let (connection, sequence) = self.consume();
        connection.check_for_error(sequence)
    }

    /// Ignore all errors to this request.
    ///
    /// Without calling this method, an error becomes available on the connection as an event after
    /// this cookie was dropped. This function causes errors to be ignored instead.
    pub fn ignore_error(self) {
        let (connection, sequence) = self.consume();
        connection.discard_reply(
            sequence,
            RequestKind::IsVoid,
            DiscardMode::DiscardReplyAndError,
        )
    }

    /// Move this cookie to refer to another connection instance.
    ///
    /// This function may only be used if both connections are "basically the same". For example, a
    /// Cookie for a connection `C` can be moved to `Rc<C>` since that still refers to the same
    /// underlying connection.
    pub(crate) fn replace_connection<C2: RequestConnection + ?Sized>(
        self,
        connection: &C2,
    ) -> VoidCookie<'_, C2> {
        let (_, sequence_number) = self.consume();
        VoidCookie {
            connection,
            sequence_number,
        }
    }
}

impl<C> Drop for VoidCookie<'_, C>
where
    C: RequestConnection + ?Sized,
{
    fn drop(&mut self) {
        self.connection.discard_reply(
            self.sequence_number,
            RequestKind::IsVoid,
            DiscardMode::DiscardReply,
        )
    }
}

/// Internal helper for a cookie with an response
#[derive(Debug)]
struct RawCookie<'a, C>
where
    C: RequestConnection + ?Sized,
{
    connection: &'a C,
    sequence_number: SequenceNumber,
}

impl<C> RawCookie<'_, C>
where
    C: RequestConnection + ?Sized,
{
    /// Construct a new raw cookie.
    ///
    /// This function should only be used by implementations of
    /// `RequestConnection::send_request_with_reply`.
    fn new(connection: &C, sequence_number: SequenceNumber) -> RawCookie<'_, C> {
        RawCookie {
            connection,
            sequence_number,
        }
    }

    /// Consume this instance and get the contained sequence number out.
    fn into_sequence_number(self) -> SequenceNumber {
        let number = self.sequence_number;
        // Prevent drop() from running
        std::mem::forget(self);
        number
    }

    /// Move this cookie to refer to another connection instance.
    ///
    /// This function may only be used if both connections are "basically the same". For example, a
    /// Cookie for a connection `C` can be moved to `Rc<C>` since that still refers to the same
    /// underlying connection.
    fn replace_connection<C2: RequestConnection + ?Sized>(
        self,
        connection: &C2,
    ) -> RawCookie<'_, C2> {
        RawCookie {
            connection,
            sequence_number: self.into_sequence_number(),
        }
    }
}

impl<C> Drop for RawCookie<'_, C>
where
    C: RequestConnection + ?Sized,
{
    fn drop(&mut self) {
        self.connection.discard_reply(
            self.sequence_number,
            RequestKind::HasResponse,
            DiscardMode::DiscardReply,
        );
    }
}

/// A handle to a response from the X11 server.
///
/// When sending a request to the X11 server, this library returns a `Cookie`. This `Cookie` can
/// then later be used to get the response that the server sent.
///
/// See [crate::cookie#requests-with-a-reply] for infos on the different ways to handle X11
/// errors in response to a request.
#[derive(Debug)]
pub struct Cookie<'a, C, R>
where
    C: RequestConnection + ?Sized,
{
    raw_cookie: RawCookie<'a, C>,
    phantom: PhantomData<R>,
}

impl<C, R> Cookie<'_, C, R>
where
    R: TryParse,
    C: RequestConnection + ?Sized,
{
    /// Construct a new cookie.
    ///
    /// This function should only be used by implementations of
    /// `RequestConnection::send_request_with_reply`.
    pub fn new(connection: &C, sequence_number: SequenceNumber) -> Cookie<'_, C, R> {
        Cookie {
            raw_cookie: RawCookie::new(connection, sequence_number),
            phantom: PhantomData,
        }
    }

    /// Get the sequence number of the request that generated this cookie.
    pub fn sequence_number(&self) -> SequenceNumber {
        self.raw_cookie.sequence_number
    }

    /// Get the raw reply that the server sent.
    pub fn raw_reply(self) -> Result<C::Buf, ReplyError> {
        let conn = self.raw_cookie.connection;
        conn.wait_for_reply_or_error(self.raw_cookie.into_sequence_number())
    }

    /// Get the raw reply that the server sent, but have errors handled as events.
    pub fn raw_reply_unchecked(self) -> Result<Option<C::Buf>, ConnectionError> {
        let conn = self.raw_cookie.connection;
        conn.wait_for_reply(self.raw_cookie.into_sequence_number())
    }

    /// Get the reply that the server sent.
    pub fn reply(self) -> Result<R, ReplyError> {
        Ok(R::try_parse(self.raw_reply()?.as_ref())?.0)
    }

    /// Get the reply that the server sent, but have errors handled as events.
    pub fn reply_unchecked(self) -> Result<Option<R>, ConnectionError> {
        self.raw_reply_unchecked()?
            .map(|buf| R::try_parse(buf.as_ref()).map(|r| r.0))
            .transpose()
            .map_err(Into::into)
    }

    /// Discard all responses to the request this cookie represents, even errors.
    ///
    /// Without this function, errors are treated as events after the cookie is dropped.
    pub fn discard_reply_and_errors(self) {
        let conn = self.raw_cookie.connection;
        conn.discard_reply(
            self.raw_cookie.into_sequence_number(),
            RequestKind::HasResponse,
            DiscardMode::DiscardReplyAndError,
        )
    }

    /// Consume this instance and get the contained sequence number out.
    pub(crate) fn into_sequence_number(self) -> SequenceNumber {
        self.raw_cookie.into_sequence_number()
    }

    /// Move this cookie to refer to another connection instance.
    ///
    /// This function may only be used if both connections are "basically the same". For example, a
    /// Cookie for a connection `C` can be moved to `Rc<C>` since that still refers to the same
    /// underlying connection.
    pub(crate) fn replace_connection<C2: RequestConnection + ?Sized>(
        self,
        connection: &C2,
    ) -> Cookie<'_, C2, R> {
        Cookie {
            raw_cookie: self.raw_cookie.replace_connection(connection),
            phantom: PhantomData,
        }
    }
}

/// A handle to a response containing `RawFd` from the X11 server.
///
/// When sending a request to the X11 server, this library returns a `Cookie`. This `Cookie` can
/// then later be used to get the response that the server sent.
///
/// This variant of `Cookie` represents a response that can contain `RawFd`s.
///
/// See [crate::cookie#requests-with-a-reply] for infos on the different ways to handle X11
/// errors in response to a request.
#[derive(Debug)]
pub struct CookieWithFds<'a, C, R>
where
    C: RequestConnection + ?Sized,
{
    raw_cookie: RawCookie<'a, C>,
    phantom: PhantomData<R>,
}

impl<C, R> CookieWithFds<'_, C, R>
where
    R: TryParseFd,
    C: RequestConnection + ?Sized,
{
    /// Construct a new cookie.
    ///
    /// This function should only be used by implementations of
    /// `RequestConnection::send_request_with_reply`.
    pub fn new(connection: &C, sequence_number: SequenceNumber) -> CookieWithFds<'_, C, R> {
        CookieWithFds {
            raw_cookie: RawCookie::new(connection, sequence_number),
            phantom: PhantomData,
        }
    }

    /// Get the sequence number of the request that generated this cookie.
    pub fn sequence_number(&self) -> SequenceNumber {
        self.raw_cookie.sequence_number
    }

    /// Get the raw reply that the server sent.
    pub fn raw_reply(self) -> Result<BufWithFds<C::Buf>, ReplyError> {
        let conn = self.raw_cookie.connection;
        conn.wait_for_reply_with_fds(self.raw_cookie.into_sequence_number())
    }

    /// Get the reply that the server sent.
    pub fn reply(self) -> Result<R, ReplyError> {
        let (buffer, mut fds) = self.raw_reply()?;
        Ok(R::try_parse_fd(buffer.as_ref(), &mut fds)?.0)
    }

    /// Move this cookie to refer to another connection instance.
    ///
    /// This function may only be used if both connections are "basically the same". For example, a
    /// Cookie for a connection `C` can be moved to `Rc<C>` since that still refers to the same
    /// underlying connection.
    pub(crate) fn replace_connection<C2: RequestConnection + ?Sized>(
        self,
        connection: &C2,
    ) -> CookieWithFds<'_, C2, R> {
        CookieWithFds {
            raw_cookie: self.raw_cookie.replace_connection(connection),
            phantom: PhantomData,
        }
    }
}

macro_rules! multiple_reply_cookie {
    (
        $(#[$meta:meta])*
        pub struct $name:ident for $reply:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $name<'a, C: RequestConnection + ?Sized>(Option<RawCookie<'a, C>>);

        impl<'c, C> $name<'c, C>
        where
            C: RequestConnection + ?Sized,
        {
            pub(crate) fn new(
                cookie: Cookie<'c, C, $reply>,
            ) -> Self {
                Self(Some(cookie.raw_cookie))
            }

            /// Get the sequence number of the request that generated this cookie.
            pub fn sequence_number(&self) -> Option<SequenceNumber> {
                self.0.as_ref().map(|x| x.sequence_number)
            }
        }

        impl<C> Iterator for $name<'_, C>
        where
            C: RequestConnection + ?Sized,
        {
            type Item = Result<$reply, ReplyError>;

            fn next(&mut self) -> Option<Self::Item> {
                let cookie = self.0.take()?;
                let reply = cookie
                    .connection
                    .wait_for_reply_or_error(cookie.sequence_number);
                let reply = match reply {
                    Err(e) => return Some(Err(e)),
                    Ok(v) => v,
                };
                let reply = $reply::try_parse(reply.as_ref());
                match reply {
                    // Is this an indicator that no more replies follow?
                    Ok(ref reply) if Self::is_last(&reply.0) => None,
                    Ok(reply) => {
                        self.0 = Some(cookie);
                        Some(Ok(reply.0))
                    }
                    Err(e) => Some(Err(e.into())),
                }
            }
        }
    }
}

multiple_reply_cookie!(
    /// A handle to the replies to a `ListFontsWithInfo` request.
    ///
    /// `ListFontsWithInfo` generated more than one reply, but `Cookie` only allows getting one reply.
    /// This structure implements `Iterator` and allows to get all the replies.
    pub struct ListFontsWithInfoCookie for ListFontsWithInfoReply
);

impl<C> ListFontsWithInfoCookie<'_, C>
where
    C: RequestConnection + ?Sized,
{
    fn is_last(reply: &ListFontsWithInfoReply) -> bool {
        reply.name.is_empty()
    }
}

#[cfg(feature = "record")]
multiple_reply_cookie!(
    /// A handle to the replies to a `record::EnableContext` request.
    ///
    /// `EnableContext` generated more than one reply, but `Cookie` only allows getting one reply.
    /// This structure implements `Iterator` and allows to get all the replies.
    pub struct RecordEnableContextCookie for EnableContextReply
);

#[cfg(feature = "record")]
impl<C> RecordEnableContextCookie<'_, C>
where
    C: RequestConnection + ?Sized,
{
    fn is_last(reply: &EnableContextReply) -> bool {
        // FIXME: There does not seem to be an enumeration of the category values, (value 5 is
        // EndOfData)
        reply.category == 5
    }
}
