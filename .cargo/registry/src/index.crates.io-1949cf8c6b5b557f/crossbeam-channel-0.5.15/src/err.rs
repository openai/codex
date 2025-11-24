use std::error;
use std::fmt;

/// An error returned from the [`send`] method.
///
/// The message could not be sent because the channel is disconnected.
///
/// The error contains the message so it can be recovered.
///
/// [`send`]: super::Sender::send
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(pub T);

/// An error returned from the [`try_send`] method.
///
/// The error contains the message being sent so it can be recovered.
///
/// [`try_send`]: super::Sender::try_send
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TrySendError<T> {
    /// The message could not be sent because the channel is full.
    ///
    /// If this is a zero-capacity channel, then the error indicates that there was no receiver
    /// available to receive the message at the time.
    Full(T),

    /// The message could not be sent because the channel is disconnected.
    Disconnected(T),
}

/// An error returned from the [`send_timeout`] method.
///
/// The error contains the message being sent so it can be recovered.
///
/// [`send_timeout`]: super::Sender::send_timeout
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SendTimeoutError<T> {
    /// The message could not be sent because the channel is full and the operation timed out.
    ///
    /// If this is a zero-capacity channel, then the error indicates that there was no receiver
    /// available to receive the message and the operation timed out.
    Timeout(T),

    /// The message could not be sent because the channel is disconnected.
    Disconnected(T),
}

/// An error returned from the [`recv`] method.
///
/// A message could not be received because the channel is empty and disconnected.
///
/// [`recv`]: super::Receiver::recv
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct RecvError;

/// An error returned from the [`try_recv`] method.
///
/// [`try_recv`]: super::Receiver::try_recv
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TryRecvError {
    /// A message could not be received because the channel is empty.
    ///
    /// If this is a zero-capacity channel, then the error indicates that there was no sender
    /// available to send a message at the time.
    Empty,

    /// The message could not be received because the channel is empty and disconnected.
    Disconnected,
}

/// An error returned from the [`recv_timeout`] method.
///
/// [`recv_timeout`]: super::Receiver::recv_timeout
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum RecvTimeoutError {
    /// A message could not be received because the channel is empty and the operation timed out.
    ///
    /// If this is a zero-capacity channel, then the error indicates that there was no sender
    /// available to send a message and the operation timed out.
    Timeout,

    /// The message could not be received because the channel is empty and disconnected.
    Disconnected,
}

/// An error returned from the [`try_select`] method.
///
/// Failed because none of the channel operations were ready.
///
/// [`try_select`]: super::Select::try_select
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct TrySelectError;

/// An error returned from the [`select_timeout`] method.
///
/// Failed because none of the channel operations became ready before the timeout.
///
/// [`select_timeout`]: super::Select::select_timeout
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct SelectTimeoutError;

/// An error returned from the [`try_ready`] method.
///
/// Failed because none of the channel operations were ready.
///
/// [`try_ready`]: super::Select::try_ready
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct TryReadyError;

/// An error returned from the [`ready_timeout`] method.
///
/// Failed because none of the channel operations became ready before the timeout.
///
/// [`ready_timeout`]: super::Select::ready_timeout
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ReadyTimeoutError;

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "SendError(..)".fmt(f)
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "sending on a disconnected channel".fmt(f)
    }
}

impl<T: Send> error::Error for SendError<T> {}

impl<T> SendError<T> {
    /// Unwraps the message.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    /// drop(r);
    ///
    /// if let Err(err) = s.send("foo") {
    ///     assert_eq!(err.into_inner(), "foo");
    /// }
    /// ```
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => "Full(..)".fmt(f),
            TrySendError::Disconnected(..) => "Disconnected(..)".fmt(f),
        }
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => "sending on a full channel".fmt(f),
            TrySendError::Disconnected(..) => "sending on a disconnected channel".fmt(f),
        }
    }
}

impl<T: Send> error::Error for TrySendError<T> {}

impl<T> From<SendError<T>> for TrySendError<T> {
    fn from(err: SendError<T>) -> TrySendError<T> {
        match err {
            SendError(t) => TrySendError::Disconnected(t),
        }
    }
}

impl<T> TrySendError<T> {
    /// Unwraps the message.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::bounded;
    ///
    /// let (s, r) = bounded(0);
    ///
    /// if let Err(err) = s.try_send("foo") {
    ///     assert_eq!(err.into_inner(), "foo");
    /// }
    /// ```
    pub fn into_inner(self) -> T {
        match self {
            TrySendError::Full(v) => v,
            TrySendError::Disconnected(v) => v,
        }
    }

    /// Returns `true` if the send operation failed because the channel is full.
    pub fn is_full(&self) -> bool {
        match self {
            TrySendError::Full(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the send operation failed because the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        match self {
            TrySendError::Disconnected(_) => true,
            _ => false,
        }
    }
}

impl<T> fmt::Debug for SendTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "SendTimeoutError(..)".fmt(f)
    }
}

impl<T> fmt::Display for SendTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            SendTimeoutError::Timeout(..) => "timed out waiting on send operation".fmt(f),
            SendTimeoutError::Disconnected(..) => "sending on a disconnected channel".fmt(f),
        }
    }
}

impl<T: Send> error::Error for SendTimeoutError<T> {}

impl<T> From<SendError<T>> for SendTimeoutError<T> {
    fn from(err: SendError<T>) -> SendTimeoutError<T> {
        match err {
            SendError(e) => SendTimeoutError::Disconnected(e),
        }
    }
}

impl<T> SendTimeoutError<T> {
    /// Unwraps the message.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    ///
    /// if let Err(err) = s.send_timeout("foo", Duration::from_secs(1)) {
    ///     assert_eq!(err.into_inner(), "foo");
    /// }
    /// ```
    pub fn into_inner(self) -> T {
        match self {
            SendTimeoutError::Timeout(v) => v,
            SendTimeoutError::Disconnected(v) => v,
        }
    }

    /// Returns `true` if the send operation timed out.
    pub fn is_timeout(&self) -> bool {
        match self {
            SendTimeoutError::Timeout(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the send operation failed because the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        match self {
            SendTimeoutError::Disconnected(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "receiving on an empty and disconnected channel".fmt(f)
    }
}

impl error::Error for RecvError {}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryRecvError::Empty => "receiving on an empty channel".fmt(f),
            TryRecvError::Disconnected => "receiving on an empty and disconnected channel".fmt(f),
        }
    }
}

impl error::Error for TryRecvError {}

impl From<RecvError> for TryRecvError {
    fn from(err: RecvError) -> TryRecvError {
        match err {
            RecvError => TryRecvError::Disconnected,
        }
    }
}

impl TryRecvError {
    /// Returns `true` if the receive operation failed because the channel is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            TryRecvError::Empty => true,
            _ => false,
        }
    }

    /// Returns `true` if the receive operation failed because the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        match self {
            TryRecvError::Disconnected => true,
            _ => false,
        }
    }
}

impl fmt::Display for RecvTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RecvTimeoutError::Timeout => "timed out waiting on receive operation".fmt(f),
            RecvTimeoutError::Disconnected => "channel is empty and disconnected".fmt(f),
        }
    }
}

impl error::Error for RecvTimeoutError {}

impl From<RecvError> for RecvTimeoutError {
    fn from(err: RecvError) -> RecvTimeoutError {
        match err {
            RecvError => RecvTimeoutError::Disconnected,
        }
    }
}

impl RecvTimeoutError {
    /// Returns `true` if the receive operation timed out.
    pub fn is_timeout(&self) -> bool {
        match self {
            RecvTimeoutError::Timeout => true,
            _ => false,
        }
    }

    /// Returns `true` if the receive operation failed because the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        match self {
            RecvTimeoutError::Disconnected => true,
            _ => false,
        }
    }
}

impl fmt::Display for TrySelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "all operations in select would block".fmt(f)
    }
}

impl error::Error for TrySelectError {}

impl fmt::Display for SelectTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "timed out waiting on select".fmt(f)
    }
}

impl error::Error for SelectTimeoutError {}
