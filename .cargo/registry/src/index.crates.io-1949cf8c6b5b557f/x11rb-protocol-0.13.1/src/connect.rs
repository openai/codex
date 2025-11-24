//! Contains utilities for connection to the X11 server.

use crate::errors::{ConnectError, ParseError};
use crate::protocol::xproto::{Setup, SetupAuthenticate, SetupFailed, SetupRequest};
use crate::x11_utils::{Serialize, TryParse};

#[cfg(feature = "std")]
use crate::xauth::{get_auth, Family};

use alloc::{vec, vec::Vec};

use core::fmt;

/// The connection handshake used to connect to the X11 server.
///
/// In order to connect to the X11 server, the client must send the
/// server a request containing important pieces of client data. In
/// response, the server sends the client a response containing one
/// of the following:
///
/// - An error indicating that the setup request is malformed, or the
///   setup otherwise failed.
/// - A request for further authorization data.
/// - The [`Setup`](protocol/xproto/struct.Setup.html) for the connection,
///   which contains server-specific information and is necessary for
///   the client's ability to communicate with the server.
///
/// This handshake contains four relevant methods:
///
/// - `new`, which creates the handshake and also returns the setup request
///   to send to the server.
/// - `buffer`, which returns an `&mut [u8]` containing the buffer
///   which is intended to hold the bytes received from the server.
/// - `advance`, which takes a `usize` indicating how many bytes
///   were received from the server and advances the buffer.
/// - `into_setup`, which consumes this `Connect` and returns the
///   full `Setup`.
///
/// # Examples
///
/// Let's say you have an object `stream` which implements `Read`
/// and `Write`. In addition, you already have the connection family,
/// the address of the connection, and the display. You can use the `Connect`
/// to establish an X11 connection like so:
///
/// ```rust,no_run
/// # use x11rb_protocol::connect::Connect;
/// # use x11rb_protocol::xauth::Family;
/// # use std::{error::Error, io::prelude::*};
/// # fn main() -> Result<(), Box<dyn Error>> {
/// # struct Stream;
/// # impl Read for Stream {
/// #    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
/// #       Ok(buf.len())
/// #    }
/// # }
/// # impl Write for Stream {
/// #    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
/// #       Ok(buf.len())
/// #    }
/// #    fn flush(&mut self) -> std::io::Result<()> {
/// #       Ok(())
/// #    }
/// # }
/// # let mut stream = Stream;
/// let family = Family::INTERNET;
/// let address = b"foobar";
/// let display = 0;
///
/// let (mut connect, setup_request) = Connect::new(family, address, display)?;
///
/// // send the setup request to the server
/// stream.write_all(&setup_request)?;
///
/// // receive the setup response from the server
/// loop {
///     let adv = stream.read(connect.buffer())?;
///
///     // if we've completed the setup, break out of the loop
///     if connect.advance(adv) {
///         break;
///     }
/// }
///
/// // get the setup used for our connection
/// let setup = connect.into_setup()?;
/// # Ok(())
/// # }
/// ```
///
/// If, instead, `stream` implements `AsyncRead` and `AsyncWrite`, the code
/// would be identical, but with `.await` after `read` and `write_all`.
pub struct Connect {
    // input buffer
    buffer: Vec<u8>,
    // position in the buffer that has been filled
    advanced: usize,
}

const INITIAL_CAPACITY: usize = 8;

// X11 interprets capital B as big endian, and lowercase l as little endian.
#[cfg(target_endian = "little")]
const BYTE_ORDER: u8 = b'l';
#[cfg(not(target_endian = "little"))]
const BYTE_ORDER: u8 = b'B';

// protocol version
const PROTOCOL_MAJOR_VERSION: u16 = 11;
const PROTOCOL_MINOR_VERSION: u16 = 0;

impl Connect {
    /// The initial state of a `Connect`.
    fn blank() -> Self {
        Self {
            buffer: vec![0; INITIAL_CAPACITY],
            advanced: 0,
        }
    }

    /// Create a new `Connect` from the given authorization data.
    ///
    /// This uses the provided protocol name and data to establish the connection,
    /// rather than the default protocol name and data found in `Xauthority`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use x11rb_protocol::connect::Connect;
    ///
    /// let (connect, setup_request) = Connect::with_authorization(
    ///     b"MIT-MAGIC-COOKIE-1".to_vec(),
    ///     b"my_secret_password".to_vec(),
    /// );
    /// ```
    pub fn with_authorization(protocol_name: Vec<u8>, protocol_data: Vec<u8>) -> (Self, Vec<u8>) {
        // craft the setup request
        let sr = SetupRequest {
            byte_order: BYTE_ORDER,
            protocol_major_version: PROTOCOL_MAJOR_VERSION,
            protocol_minor_version: PROTOCOL_MINOR_VERSION,
            authorization_protocol_name: protocol_name,
            authorization_protocol_data: protocol_data,
        };

        // return it
        (Self::blank(), sr.serialize())
    }

    /// Create a new `Connect` from the information necessary to connect to the X11 server.
    ///
    /// This returns the connection handshake object as well as the setup request to send to the server.
    #[cfg(feature = "std")]
    pub fn new(
        family: Family,
        address: &[u8],
        display: u16,
    ) -> Result<(Self, Vec<u8>), ConnectError> {
        match get_auth(family, address, display)? {
            Some((name, data)) => Ok(Self::with_authorization(name, data)),
            None => {
                // fall through to no authorization
                Ok(Self::with_authorization(Vec::new(), Vec::new()))
            }
        }
    }

    /// Returns the buffer that needs to be filled with incoming data from the server.
    ///
    /// After filling this buffer (using a method like `Read::read`), call [`Self::advance`] with
    /// the number of bytes read to indicate that the buffer has been filled.
    pub fn buffer(&mut self) -> &mut [u8] {
        &mut self.buffer[self.advanced..]
    }

    /// Advance the internal buffer, given the number of bytes that have been read.
    pub fn advance(&mut self, bytes: usize) -> bool {
        self.advanced += bytes;
        debug_assert!(self.buffer.len() >= self.advanced);

        // if we've read up to the initial capacity, tell how many more bytes
        // we need to read
        if self.advanced == INITIAL_CAPACITY {
            // remaining length is at byte range 6-7 in 4-bytes
            let length = u16::from_ne_bytes([self.buffer[6], self.buffer[7]]);
            let length = length as usize * 4;

            // allocate more room
            // use reserve_exact because this will be the final
            // length of the vector
            self.buffer.reserve_exact(length);
            self.buffer.resize(length + self.buffer.len(), 0);
            false
        } else {
            self.advanced == self.buffer.len()
        }
    }

    /// Returns the setup provided by the server.
    ///
    /// # Errors
    ///
    /// - If this method is called before the server returns all of the required data,
    ///   it returns `ConnectError::NotEnoughData`.
    /// - If the server fails to establish the X11 connection, the `ConnectError::SetupFailed`
    ///   variant is returned.
    /// - If the server failed to authenticate the user, the `ConnectError::SetupAuthenticate`
    ///   error is returned.
    /// - If the server failed to parse any of the above responses, the
    ///   `ConnectError::ParseError` error is returned.
    pub fn into_setup(self) -> Result<Setup, ConnectError> {
        // if we aren't full yet, panic
        if self.advanced != self.buffer.len() {
            return Err(ConnectError::Incomplete {
                expected: self.buffer.len(),
                received: self.advanced,
            });
        }

        // parse the setup response
        match self.buffer[0] {
            0 => {
                // an error has occurred
                let (failed, _) = SetupFailed::try_parse(&self.buffer)?;
                Err(ConnectError::SetupFailed(failed))
            }
            1 => {
                // the setup is valid!
                let (success, _) = Setup::try_parse(&self.buffer)?;
                Ok(success)
            }
            2 => {
                // we need further authentication
                let (more_auth, _) = SetupAuthenticate::try_parse(&self.buffer)?;
                Err(ConnectError::SetupAuthenticate(more_auth))
            }
            _ => {
                // this is undefined
                Err(ParseError::InvalidValue.into())
            }
        }
    }
}

impl fmt::Debug for Connect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connect")
            .field(
                "buffer",
                &format_args!("{}/{}", self.advanced, self.buffer.len()),
            )
            .finish()
    }
}

impl TryFrom<Connect> for Setup {
    type Error = ConnectError;

    fn try_from(connect: Connect) -> Result<Self, Self::Error> {
        connect.into_setup()
    }
}

#[cfg(test)]
#[cfg(feature = "extra-traits")]
mod tests {
    use super::Connect;
    use crate::errors::ConnectError;
    use crate::protocol::xproto::{ImageOrder, Setup, SetupAuthenticate, SetupFailed};
    use crate::x11_utils::Serialize;
    use alloc::vec;

    fn test_setup() -> Setup {
        let mut s = Setup {
            status: 1,
            protocol_major_version: 11,
            protocol_minor_version: 0,
            length: 0,
            release_number: 0,
            resource_id_base: 1,
            resource_id_mask: 1,
            motion_buffer_size: 0,
            maximum_request_length: 0,
            image_byte_order: ImageOrder::LSB_FIRST,
            bitmap_format_bit_order: ImageOrder::LSB_FIRST,
            bitmap_format_scanline_unit: 32,
            bitmap_format_scanline_pad: 32,
            min_keycode: 0,
            max_keycode: 0,
            vendor: b"Testing Setup".to_vec(),
            pixmap_formats: vec![],
            roots: vec![],
        };
        // +3 so it rounds up
        s.length = ((s.serialize().len() - 8 + 3) / 4) as u16;
        s
    }

    fn try_receive_bytes(item: &impl Serialize) -> Result<Setup, ConnectError> {
        let mut connect = Connect::blank();

        // feed in a setup
        let mut item_bytes = vec![];
        item.serialize_into(&mut item_bytes);

        let mut i = 0;
        loop {
            i += 1;
            if i > 500 {
                panic!("too many iterations");
            }

            // copy bytes to connect
            let buffer = connect.buffer();
            let bytes_to_copy = std::cmp::min(item_bytes.len(), buffer.len());
            buffer[..bytes_to_copy].copy_from_slice(&item_bytes[..bytes_to_copy]);

            // drain the bytes that we've already copied
            drop(item_bytes.drain(..bytes_to_copy));

            // check advance
            if connect.advance(bytes_to_copy) {
                break;
            }
        }

        connect.into_setup()
    }

    #[test]
    fn test_connect_receive_setup() {
        let setup = test_setup();
        let b = try_receive_bytes(&setup);

        match b {
            Ok(s) => assert_eq!(s, setup),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_connect_receive_setup_authenticate() {
        let setup = SetupAuthenticate {
            status: 2,
            reason: b"Needs more auth.".to_vec(),
        };

        let b = try_receive_bytes(&setup);
        match b {
            Ok(s) => panic!("{:?}", s),
            Err(ConnectError::SetupAuthenticate(e)) => assert_eq!(e, setup),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_connect_receive_setup_failed() {
        let mut setup = SetupFailed {
            status: 0,
            protocol_major_version: 11,
            protocol_minor_version: 0,
            length: 0,
            reason: b"whatever".to_vec(),
        };
        setup.length = ((setup.serialize().len() - 8) / 4) as _;

        let b = try_receive_bytes(&setup);
        match b {
            Ok(s) => panic!("{:?}", s),
            Err(ConnectError::SetupFailed(e)) => assert_eq!(e, setup),
            Err(e) => panic!("{:?}", e),
        }
    }
}
