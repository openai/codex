use std::fmt;

/// HTTP/2 error codes.
///
/// Error codes are used in `RST_STREAM` and `GOAWAY` frames to convey the
/// reasons for the stream or connection error. For example,
/// [`SendStream::send_reset`] takes a `Reason` argument. Also, the `Error` type
/// may contain a `Reason`.
///
/// Error codes share a common code space. Some error codes apply only to
/// streams, others apply only to connections, and others may apply to either.
/// See [RFC 7540] for more information.
///
/// See [Error Codes in the spec][spec].
///
/// [spec]: http://httpwg.org/specs/rfc7540.html#ErrorCodes
/// [`SendStream::send_reset`]: struct.SendStream.html#method.send_reset
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Reason(u32);

impl Reason {
    /// The associated condition is not a result of an error.
    ///
    /// For example, a GOAWAY might include this code to indicate graceful
    /// shutdown of a connection.
    pub const NO_ERROR: Reason = Reason(0);
    /// The endpoint detected an unspecific protocol error.
    ///
    /// This error is for use when a more specific error code is not available.
    pub const PROTOCOL_ERROR: Reason = Reason(1);
    /// The endpoint encountered an unexpected internal error.
    pub const INTERNAL_ERROR: Reason = Reason(2);
    /// The endpoint detected that its peer violated the flow-control protocol.
    pub const FLOW_CONTROL_ERROR: Reason = Reason(3);
    /// The endpoint sent a SETTINGS frame but did not receive a response in
    /// a timely manner.
    pub const SETTINGS_TIMEOUT: Reason = Reason(4);
    /// The endpoint received a frame after a stream was half-closed.
    pub const STREAM_CLOSED: Reason = Reason(5);
    /// The endpoint received a frame with an invalid size.
    pub const FRAME_SIZE_ERROR: Reason = Reason(6);
    /// The endpoint refused the stream prior to performing any application
    /// processing.
    pub const REFUSED_STREAM: Reason = Reason(7);
    /// Used by the endpoint to indicate that the stream is no longer needed.
    pub const CANCEL: Reason = Reason(8);
    /// The endpoint is unable to maintain the header compression context for
    /// the connection.
    pub const COMPRESSION_ERROR: Reason = Reason(9);
    /// The connection established in response to a CONNECT request was reset
    /// or abnormally closed.
    pub const CONNECT_ERROR: Reason = Reason(10);
    /// The endpoint detected that its peer is exhibiting a behavior that might
    /// be generating excessive load.
    pub const ENHANCE_YOUR_CALM: Reason = Reason(11);
    /// The underlying transport has properties that do not meet minimum
    /// security requirements.
    pub const INADEQUATE_SECURITY: Reason = Reason(12);
    /// The endpoint requires that HTTP/1.1 be used instead of HTTP/2.
    pub const HTTP_1_1_REQUIRED: Reason = Reason(13);

    /// Get a string description of the error code.
    pub fn description(&self) -> &str {
        match self.0 {
            0 => "not a result of an error",
            1 => "unspecific protocol error detected",
            2 => "unexpected internal error encountered",
            3 => "flow-control protocol violated",
            4 => "settings ACK not received in timely manner",
            5 => "received frame when stream half-closed",
            6 => "frame with invalid size",
            7 => "refused stream before processing any application logic",
            8 => "stream no longer needed",
            9 => "unable to maintain the header compression context",
            10 => {
                "connection established in response to a CONNECT request was reset or abnormally \
                 closed"
            }
            11 => "detected excessive load generating behavior",
            12 => "security properties do not meet minimum requirements",
            13 => "endpoint requires HTTP/1.1",
            _ => "unknown reason",
        }
    }
}

impl From<u32> for Reason {
    fn from(src: u32) -> Reason {
        Reason(src)
    }
}

impl From<Reason> for u32 {
    fn from(src: Reason) -> u32 {
        src.0
    }
}

impl fmt::Debug for Reason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self.0 {
            0 => "NO_ERROR",
            1 => "PROTOCOL_ERROR",
            2 => "INTERNAL_ERROR",
            3 => "FLOW_CONTROL_ERROR",
            4 => "SETTINGS_TIMEOUT",
            5 => "STREAM_CLOSED",
            6 => "FRAME_SIZE_ERROR",
            7 => "REFUSED_STREAM",
            8 => "CANCEL",
            9 => "COMPRESSION_ERROR",
            10 => "CONNECT_ERROR",
            11 => "ENHANCE_YOUR_CALM",
            12 => "INADEQUATE_SECURITY",
            13 => "HTTP_1_1_REQUIRED",
            other => return f.debug_tuple("Reason").field(&Hex(other)).finish(),
        };
        f.write_str(name)
    }
}

struct Hex(u32);

impl fmt::Debug for Hex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}
