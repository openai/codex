//! inotify support for working with inotify objects.

use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `IN_*` for use with [`inotify::init`].
    ///
    /// [`inotify::init`]: crate::fs::inotify::init
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CreateFlags: c::c_uint {
        /// `IN_CLOEXEC`
        const CLOEXEC = linux_raw_sys::general::IN_CLOEXEC;
        /// `IN_NONBLOCK`
        const NONBLOCK = linux_raw_sys::general::IN_NONBLOCK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `IN*` for use with [`inotify::add_watch`].
    ///
    /// [`inotify::add_watch`]: crate::fs::inotify::add_watch
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct WatchFlags: c::c_uint {
        /// `IN_ACCESS`
        const ACCESS = linux_raw_sys::general::IN_ACCESS;
        /// `IN_ATTRIB`
        const ATTRIB = linux_raw_sys::general::IN_ATTRIB;
        /// `IN_CLOSE_NOWRITE`
        const CLOSE_NOWRITE = linux_raw_sys::general::IN_CLOSE_NOWRITE;
        /// `IN_CLOSE_WRITE`
        const CLOSE_WRITE = linux_raw_sys::general::IN_CLOSE_WRITE;
        /// `IN_CREATE`
        const CREATE = linux_raw_sys::general::IN_CREATE;
        /// `IN_DELETE`
        const DELETE = linux_raw_sys::general::IN_DELETE;
        /// `IN_DELETE_SELF`
        const DELETE_SELF = linux_raw_sys::general::IN_DELETE_SELF;
        /// `IN_MODIFY`
        const MODIFY = linux_raw_sys::general::IN_MODIFY;
        /// `IN_MOVE_SELF`
        const MOVE_SELF = linux_raw_sys::general::IN_MOVE_SELF;
        /// `IN_MOVED_FROM`
        const MOVED_FROM = linux_raw_sys::general::IN_MOVED_FROM;
        /// `IN_MOVED_TO`
        const MOVED_TO = linux_raw_sys::general::IN_MOVED_TO;
        /// `IN_OPEN`
        const OPEN = linux_raw_sys::general::IN_OPEN;

        /// `IN_CLOSE`
        const CLOSE = linux_raw_sys::general::IN_CLOSE;
        /// `IN_MOVE`
        const MOVE = linux_raw_sys::general::IN_MOVE;
        /// `IN_ALL_EVENTS`
        const ALL_EVENTS = linux_raw_sys::general::IN_ALL_EVENTS;

        /// `IN_DONT_FOLLOW`
        const DONT_FOLLOW = linux_raw_sys::general::IN_DONT_FOLLOW;
        /// `IN_EXCL_UNLINK`
        const EXCL_UNLINK = linux_raw_sys::general::IN_EXCL_UNLINK;
        /// `IN_MASK_ADD`
        const MASK_ADD = linux_raw_sys::general::IN_MASK_ADD;
        /// `IN_MASK_CREATE`
        const MASK_CREATE = linux_raw_sys::general::IN_MASK_CREATE;
        /// `IN_ONESHOT`
        const ONESHOT = linux_raw_sys::general::IN_ONESHOT;
        /// `IN_ONLYDIR`
        const ONLYDIR = linux_raw_sys::general::IN_ONLYDIR;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `IN*` for use with [`inotify::Reader`].
    ///
    /// [`inotify::Reader`]: crate::fs::inotify::InotifyReader
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ReadFlags: c::c_uint {
        /// `IN_ACCESS`
        const ACCESS = linux_raw_sys::general::IN_ACCESS;
        /// `IN_ATTRIB`
        const ATTRIB = linux_raw_sys::general::IN_ATTRIB;
        /// `IN_CLOSE_NOWRITE`
        const CLOSE_NOWRITE = linux_raw_sys::general::IN_CLOSE_NOWRITE;
        /// `IN_CLOSE_WRITE`
        const CLOSE_WRITE = linux_raw_sys::general::IN_CLOSE_WRITE;
        /// `IN_CREATE`
        const CREATE = linux_raw_sys::general::IN_CREATE;
        /// `IN_DELETE`
        const DELETE = linux_raw_sys::general::IN_DELETE;
        /// `IN_DELETE_SELF`
        const DELETE_SELF = linux_raw_sys::general::IN_DELETE_SELF;
        /// `IN_MODIFY`
        const MODIFY = linux_raw_sys::general::IN_MODIFY;
        /// `IN_MOVE_SELF`
        const MOVE_SELF = linux_raw_sys::general::IN_MOVE_SELF;
        /// `IN_MOVED_FROM`
        const MOVED_FROM = linux_raw_sys::general::IN_MOVED_FROM;
        /// `IN_MOVED_TO`
        const MOVED_TO = linux_raw_sys::general::IN_MOVED_TO;
        /// `IN_OPEN`
        const OPEN = linux_raw_sys::general::IN_OPEN;

        /// `IN_IGNORED`
        const IGNORED = linux_raw_sys::general::IN_IGNORED;
        /// `IN_ISDIR`
        const ISDIR = linux_raw_sys::general::IN_ISDIR;
        /// `IN_Q_OVERFLOW`
        const QUEUE_OVERFLOW = linux_raw_sys::general::IN_Q_OVERFLOW;
        /// `IN_UNMOUNT`
        const UNMOUNT = linux_raw_sys::general::IN_UNMOUNT;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
