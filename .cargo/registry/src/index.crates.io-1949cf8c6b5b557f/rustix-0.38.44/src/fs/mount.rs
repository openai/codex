//! Linux `mount`.
//!
//! These have been moved to a new `rustix::mount` module.

#[deprecated(note = "`rustix::fs::UnmountFlags` moved to `rustix::mount::UnmountFlags`.")]
#[doc(hidden)]
pub use crate::mount::UnmountFlags;

#[deprecated(note = "`rustix::fs::MountFlags` moved to `rustix::mount::MountFlags`.")]
#[doc(hidden)]
pub use crate::mount::MountFlags;

#[deprecated(
    note = "`rustix::fs::MountPropagationFlags` moved to `rustix::mount::MountPropagationFlags`."
)]
#[doc(hidden)]
pub use crate::mount::MountPropagationFlags;

#[deprecated(note = "`rustix::fs::mount` moved to `rustix::mount::mount`.")]
#[doc(hidden)]
pub use crate::mount::mount;

#[deprecated(note = "`rustix::fs::unmount` moved to `rustix::mount::unmount`.")]
#[doc(hidden)]
pub use crate::mount::unmount;

#[deprecated(
    note = "`rustix::fs::remount` is renamed and moved to `rustix::mount::mount_remount`."
)]
#[doc(hidden)]
pub use crate::mount::mount_remount as remount;

#[deprecated(
    note = "`rustix::fs::bind_mount` is renamed and moved to `rustix::mount::mount_bind`."
)]
#[doc(hidden)]
pub use crate::mount::mount_bind as bind_mount;

#[deprecated(
    note = "`rustix::fs::recursive_bind_mount` is renamed and moved to `rustix::mount::mount_recursive_bind`."
)]
#[doc(hidden)]
pub use crate::mount::mount_recursive_bind as recursive_bind_mount;

#[deprecated(
    note = "`rustix::fs::change_mount` is renamed and moved to `rustix::mount::mount_change`."
)]
#[doc(hidden)]
pub use crate::mount::mount_change as change_mount;

#[deprecated(
    note = "`rustix::fs::move_mount` is renamed and moved to `rustix::mount::mount_move`."
)]
#[doc(hidden)]
pub use crate::mount::mount_move as move_mount;
