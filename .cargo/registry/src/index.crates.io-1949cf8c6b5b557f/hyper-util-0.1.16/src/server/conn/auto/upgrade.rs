//! Upgrade utilities.

use bytes::{Bytes, BytesMut};
use hyper::{
    rt::{Read, Write},
    upgrade::Upgraded,
};

use crate::common::rewind::Rewind;

/// Tries to downcast the internal trait object to the type passed.
///
/// On success, returns the downcasted parts. On error, returns the Upgraded back.
/// This is a kludge to work around the fact that the machinery provided by
/// [`hyper_util::server::conn::auto`] wraps the inner `T` with a private type
/// that is not reachable from outside the crate.
///
/// [`hyper_util::server::conn::auto`]: crate::server::conn::auto
///
/// This kludge will be removed when this machinery is added back to the main
/// `hyper` code.
pub fn downcast<T>(upgraded: Upgraded) -> Result<Parts<T>, Upgraded>
where
    T: Read + Write + Unpin + 'static,
{
    let hyper::upgrade::Parts {
        io: rewind,
        mut read_buf,
        ..
    } = upgraded.downcast::<Rewind<T>>()?;

    if let Some(pre) = rewind.pre {
        read_buf = if read_buf.is_empty() {
            pre
        } else {
            let mut buf = BytesMut::from(read_buf);

            buf.extend_from_slice(&pre);

            buf.freeze()
        };
    }

    Ok(Parts {
        io: rewind.inner,
        read_buf,
    })
}

/// The deconstructed parts of an [`Upgraded`] type.
///
/// Includes the original IO type, and a read buffer of bytes that the
/// HTTP state machine may have already read before completing an upgrade.
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    /// The original IO object used before the upgrade.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// For instance, if the `Connection` is used for an HTTP upgrade request,
    /// it is possible the server sent back the first bytes of the new protocol
    /// along with the response upgrade.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
}
