use std::io;
#[cfg(not(target_os = "hermit"))]
use std::os::fd::RawFd;
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::RawFd;

use crate::{event, Interest, Registry, Token};

/// Adapter for [`RawFd`] providing an [`event::Source`] implementation.
///
/// `SourceFd` enables registering any type with an FD with [`Poll`].
///
/// While only implementations for TCP and UDP are provided, Mio supports
/// registering any FD that can be registered with the underlying OS selector.
/// `SourceFd` provides the necessary bridge.
///
/// Note that `SourceFd` takes a `&RawFd`. This is because `SourceFd` **does
/// not** take ownership of the FD. Specifically, it will not manage any
/// lifecycle related operations, such as closing the FD on drop. It is expected
/// that the `SourceFd` is constructed right before a call to
/// [`Registry::register`]. See the examples for more detail.
///
/// [`event::Source`]: ../event/trait.Source.html
/// [`Poll`]: ../struct.Poll.html
/// [`Registry::register`]: ../struct.Registry.html#method.register
///
/// # Examples
///
/// Basic usage.
///
#[cfg_attr(
    all(feature = "os-poll", feature = "net", feature = "os-ext"),
    doc = "```"
)]
#[cfg_attr(
    not(all(feature = "os-poll", feature = "net", feature = "os-ext")),
    doc = "```ignore"
)]
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use mio::{Interest, Poll, Token};
/// use mio::unix::SourceFd;
///
/// use std::os::fd::AsRawFd;
/// use std::net::TcpListener;
///
/// // Bind a std listener
/// let listener = TcpListener::bind("127.0.0.1:0")?;
///
/// let poll = Poll::new()?;
///
/// // Register the listener
/// poll.registry().register(
///     &mut SourceFd(&listener.as_raw_fd()),
///     Token(0),
///     Interest::READABLE)?;
/// #     Ok(())
/// # }
/// ```
///
/// Implementing [`event::Source`] for a custom type backed by a [`RawFd`].
///
#[cfg_attr(all(feature = "os-poll", feature = "os-ext"), doc = "```")]
#[cfg_attr(not(all(feature = "os-poll", feature = "os-ext")), doc = "```ignore")]
/// use mio::{event, Interest, Registry, Token};
/// use mio::unix::SourceFd;
///
/// use std::os::fd::RawFd;
/// use std::io;
///
/// # #[allow(dead_code)]
/// pub struct MyIo {
///     fd: RawFd,
/// }
///
/// impl event::Source for MyIo {
///     fn register(&mut self, registry: &Registry, token: Token, interests: Interest)
///         -> io::Result<()>
///     {
///         SourceFd(&self.fd).register(registry, token, interests)
///     }
///
///     fn reregister(&mut self, registry: &Registry, token: Token, interests: Interest)
///         -> io::Result<()>
///     {
///         SourceFd(&self.fd).reregister(registry, token, interests)
///     }
///
///     fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
///         SourceFd(&self.fd).deregister(registry)
///     }
/// }
/// ```
#[derive(Debug)]
pub struct SourceFd<'a>(pub &'a RawFd);

impl<'a> event::Source for SourceFd<'a> {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        registry.selector().register(*self.0, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        registry.selector().reregister(*self.0, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        registry.selector().deregister(*self.0)
    }
}
