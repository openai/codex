use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};

/// An opaque ID that uniquely identifies a runtime relative to all other currently
/// running runtimes.
///
/// # Notes
///
/// - Runtime IDs are unique relative to other *currently running* runtimes.
///   When a runtime completes, the same ID may be used for another runtime.
/// - Runtime IDs are *not* sequential, and do not indicate the order in which
///   runtimes are started or any other data.
/// - The runtime ID of the currently running task can be obtained from the
///   Handle.
///
/// # Examples
///
/// ```
/// use tokio::runtime::Handle;
///
/// #[tokio::main(flavor = "multi_thread", worker_threads = 4)]
/// async fn main() {
///   println!("Current runtime id: {}", Handle::current().id());
/// }
/// ```
///
/// **Note**: This is an [unstable API][unstable]. The public API of this type
/// may break in 1.x releases. See [the documentation on unstable
/// features][unstable] for details.
///
/// [unstable]: crate#unstable-features
#[cfg_attr(not(tokio_unstable), allow(unreachable_pub))]
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct Id(NonZeroU64);

impl From<NonZeroU64> for Id {
    fn from(value: NonZeroU64) -> Self {
        Id(value)
    }
}

impl From<NonZeroU32> for Id {
    fn from(value: NonZeroU32) -> Self {
        Id(value.into())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
