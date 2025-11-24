#![deny(missing_docs)]


//! # inotify bindings for the Rust programming language
//!
//! Please note that these are direct, low-level bindings to C functions that
//! form the inotify C API. Unless you have a specific reason to use this crate,
//! [inotify-rs], which is an idiomatic wrapper, is a much better choice.
//!
//! ## Usage
//!
//! In general, inotify usage follows the following pattern:
//!
//! 1. Create an inotify instance using [`inotify_init`] or [`inotify_init1`].
//! 2. Manage watches with [`inotify_add_watch`] and [`inotify_rm_watch`].
//! 3. Read event using [`read`].
//! 4. Close the inotify instance using [`close`], once you're done.
//!
//! Please refer to the [inotify man page] and the rest of this documentation
//! for full details.
//!
//! [inotify-rs]: https://crates.io/crates/inotify
//! [`inotify_init`]: fn.inotify_init.html
//! [`inotify_init1`]: fn.inotify_init1.html
//! [`inotify_add_watch`]: fn.inotify_add_watch.html
//! [`inotify_rm_watch`]: fn.inotify_rm_watch.html
//! [`read`]: fn.read.html
//! [`close`]: fn.close.html
//! [inotify man page]: http://man7.org/linux/man-pages/man7/inotify.7.html


extern crate libc;


use libc::{
    c_char,
    c_int,
};


/// Set the `FD_CLOEXEC` flag for an inotify instance
///
/// Can be passed to [`inotify_init1`] to set the `FD_CLOEXEC` flag for the
/// inotify instance. This changes the behavior of file descriptor when
/// [execve(2)]'d. From [fcntl(2)]:
///
/// > If the FD_CLOEXEC bit is 0, the file descriptor will
/// > remain open across an [execve(2)], otherwise it will be
/// > closed.
///
/// See [open(2)] and [fcntl(2)] for details.
///
/// [`inotify_init1`]: fn.inotify_init1.html
/// [execve(2)]: http://man7.org/linux/man-pages/man2/execve.2.html
/// [open(2)]: http://man7.org/linux/man-pages/man2/open.2.html
/// [fcntl(2)]: http://man7.org/linux/man-pages/man2/fcntl.2.html
pub const IN_CLOEXEC: c_int = libc::O_CLOEXEC;

/// Set an inotify instance to non-blocking mode
///
/// Can be passed to [`inotify_init1`] to set the `O_NONBLOCK` flag for the
/// inotify instance.
///
/// See [open(2)] for details.
///
/// [`inotify_init1`]: fn.inotify_init1.html
/// [open(2)]: http://man7.org/linux/man-pages/man2/open.2.html
pub const IN_NONBLOCK: c_int = libc::O_NONBLOCK;

/// Event: File was accessed
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ACCESS: u32 = 0x00000001;

/// Event: File was modified
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MODIFY: u32 = 0x00000002;

/// Event: Metadata was changed
///
/// This can include e.g.
///
/// - permissions, see [chmod(2)];
/// - timestamps, see [utimensat(2)];
/// - extended attributes, see [setxattr(2)];
/// - link count, see [link(2)] and [unlink(2)];
/// - user/group, see [chown(2)].
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event can be triggered for both for the
/// directory itself and the files within.
///
/// See [man page] for additional details.
///
/// [chmod(2)]: http://man7.org/linux/man-pages/man2/chmod.2.html
/// [utimensat(2)]: http://man7.org/linux/man-pages/man2/utimensat.2.html
/// [setxattr(2)]: http://man7.org/linux/man-pages/man2/fsetxattr.2.html
/// [link(2)]: http://man7.org/linux/man-pages/man2/link.2.html
/// [unlink(2)]: http://man7.org/linux/man-pages/man2/unlink.2.html
/// [chown(2)]: http://man7.org/linux/man-pages/man2/chown.2.html
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ATTRIB: u32 = 0x00000004;

/// Event: Writable file was closed
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_CLOSE_WRITE: u32 = 0x00000008;

/// Event: Non-writable file or directory was closed
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event can be triggered for both for the
/// directory itself and the files within.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_CLOSE_NOWRITE: u32 = 0x00000010;

/// Event: File or directory was opened
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event can be triggered for both for the
/// directory itself and the files within.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_OPEN: u32 = 0x00000020;

/// Event: File or directory was moved out of watched directory
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MOVED_FROM: u32 = 0x00000040;

/// Event: File or directory was moved into watched directory
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MOVED_TO: u32 = 0x00000080;

/// Event: File or directory was created in watched directory
///
/// This may also include hard links, symlinks, and UNIX sockets.
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_CREATE: u32 = 0x00000100;

/// Event: File or directory in watched directory was deleted
///
/// This may also include hard links, symlinks, and UNIX sockets.
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// When monitoring a directory, this event will be triggered only for files
/// within the directory.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_DELETE: u32 = 0x00000200;

/// Event: Watched file or directory was deleted
///
/// This may also occur if the object is moved to another filesystem, since
/// [mv(1)] in effect copies the file to the other filesystem and then deletes
/// it from the original.
///
/// An IN_IGNORED event will subsequently be generated.
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// See [man page] for additional details.
///
/// [mv(1)]: http://man7.org/linux/man-pages/man1/mv.1.html
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_DELETE_SELF: u32 = 0x00000400;

/// Event: Watched file or directory was moved
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MOVE_SELF: u32 = 0x00000800;

/// Event: File or directory within watched directory was moved
///
/// This is a combination of [`IN_MOVED_FROM`] and [`IN_MOVED_TO`].
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// See [man page] for additional details.
///
/// [`IN_MOVED_FROM`]: constant.IN_MOVED_FROM.html
/// [`IN_MOVED_TO`]: constant.IN_MOVED_TO.html
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MOVE: u32 = IN_MOVED_FROM | IN_MOVED_TO;

/// Event: File was closed
///
/// This is a combination of [`IN_CLOSE_WRITE`] and [`IN_CLOSE_NOWRITE`].
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in this type of event, or it can be used to check (via [`inotify_event`]'s
/// [`mask`] field) whether an event is of this type.
///
/// See [man page] for additional details.
///
/// [`IN_CLOSE_WRITE`]: constant.IN_CLOSE_WRITE.html
/// [`IN_CLOSE_NOWRITE`]: constant.IN_CLOSE_NOWRITE.html
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [`inotify_event`]: struct.inotify_event.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_CLOSE: u32 = IN_CLOSE_WRITE | IN_CLOSE_NOWRITE;

/// Event: Any event occured
///
/// This is a combination of all the other event constants:
///
/// - [`IN_ACCESS`]
/// - [`IN_ATTRIB`]
/// - [`IN_CLOSE_WRITE`]
/// - [`IN_CLOSE_NOWRITE`]
/// - [`IN_MODIFY`]
/// - [`IN_CREATE`]
/// - [`IN_DELETE`]
/// - [`IN_DELETE_SELF`]
/// - [`IN_MODIFY`]
/// - [`IN_MOVE_SELF`]
/// - [`IN_MOVED_FROM`]
/// - [`IN_MOVED_TO`]
/// - [`IN_OPEN`]
///
/// This constant can be passed to [`inotify_add_watch`], to register interest
/// in any type of event.
///
/// See [man page] for additional details.
///
/// [`IN_ACCESS`]: constant.IN_ACCESS.html
/// [`IN_ATTRIB`]: constant.IN_ATTRIB.html
/// [`IN_CLOSE_WRITE`]: constant.IN_CLOSE_WRITE.html
/// [`IN_CLOSE_NOWRITE`]: constant.IN_CLOSE_NOWRITE.html
/// [`IN_MODIFY`]: constant.IN_MODIFY.html
/// [`IN_CREATE`]: constant.IN_CREATE.html
/// [`IN_DELETE`]: constant.IN_DELETE.html
/// [`IN_DELETE_SELF`]: constant.IN_DELETE_SELF.html
/// [`IN_MODIFY`]: constant.IN_MODIFY.html
/// [`IN_MOVE_SELF`]: constant.IN_MOVE_SELF.html
/// [`IN_MOVED_FROM`]: constant.IN_MOVED_FROM.html
/// [`IN_MOVED_TO`]: constant.IN_MOVED_TO.html
/// [`IN_OPEN`]: constant.IN_OPEN.html
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ALL_EVENTS: u32 =
    IN_ACCESS | IN_MODIFY | IN_ATTRIB | IN_CLOSE_WRITE | IN_CLOSE_NOWRITE
    | IN_OPEN | IN_MOVED_FROM | IN_MOVED_TO | IN_CREATE | IN_DELETE
    | IN_DELETE_SELF | IN_MOVE_SELF;

/// Only watch path, if it is a directory
///
/// This bit can be set in [`inotify_add_watch`]'s `mask` parameter, to
/// configure the watch.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ONLYDIR: u32 = 0x01000000;

/// Don't dereference path, if it is a symbolic link
///
/// This bit can be set in [`inotify_add_watch`]'s `mask` parameter, to
/// configure the watch.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_DONT_FOLLOW: u32 = 0x02000000;

/// Ignore events for children, that have been unlinked from watched directory
///
/// This bit can be set in [`inotify_add_watch`]'s `mask` parameter, to
/// configure the watch.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_EXCL_UNLINK: u32 = 0x04000000;

/// Update existing watch mask, instead of replacing it
///
/// This bit can be set in [`inotify_add_watch`]'s `mask` parameter, to
/// configure the watch.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_MASK_ADD: u32 = 0x20000000;

/// Remove watch after one event
///
/// This bit can be set in [`inotify_add_watch`]'s `mask` parameter, to
/// configure the watch.
///
/// See [man page] for additional details.
///
/// [`inotify_add_watch`]: fn.inotify_add_watch.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ONESHOT: u32 = 0x80000000;

/// Indicates that the subject of an event is a directory
///
/// This constant can be used to check against the [`mask`] field in
/// [`inotify_event`].
///
/// See [man page] for additional details.
///
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [`inotify_event`]: struct.inotify_event.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_ISDIR: u32 = 0x40000000;

/// Indicates that file system containing a watched object has been unmounted
///
/// An [`IN_IGNORED`] event will be generated subsequently.
///
/// This constant can be used to check against the [`mask`] field in
/// [`inotify_event`].
///
/// See [man page] for additional details.
///
/// [`IN_IGNORED`]: constant.IN_IGNORED.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [`inotify_event`]: struct.inotify_event.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_UNMOUNT: u32 = 0x00002000;

/// Indicates that the event queue has overflowed
///
/// This constant can be used to check against the [`mask`] field in
/// [`inotify_event`].
///
/// See [man page] for additional details.
///
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [`inotify_event`]: struct.inotify_event.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_Q_OVERFLOW: u32 = 0x00004000;

/// Indicates that a file system watch was removed
///
/// This can occur as a result of [`inotify_rm_watch`], because a watched item
///  was deleted, the containing filesystem was unmounted, or after a
/// [`IN_ONESHOT`] watch is complete.
///
/// This constant can be used to check against the [`mask`] field in
/// [`inotify_event`].
///
/// See [man page] for additional details.
///
/// [`inotify_rm_watch`]: fn.inotify_rm_watch.html
/// [`IN_ONESHOT`]: constant.IN_ONESHOT.html
/// [`mask`]: struct.inotify_event.html#structfield.mask
/// [`inotify_event`]: struct.inotify_event.html
/// [man page]: http://man7.org/linux/man-pages/man7/inotify.7.html
pub const IN_IGNORED: u32 = 0x00008000;


/// Describes a file system event
///
/// From [inotify(7)]:
///
/// > To determine what events have occurred, an application [read(2)]s
/// > from the inotify file descriptor.  If no events have so far occurred,
/// > then, assuming a blocking file descriptor, [read(2)] will block until
/// > at least one event occurs (unless interrupted by a signal, in which
/// > case the call fails with the error EINTR; see [signal(7)]).
/// >
/// > Each successful [read(2)] returns a buffer containing one or more of
/// > this structure.
///
/// [inotify(7)]: http://man7.org/linux/man-pages/man7/inotify.7.html
/// [read(2)]: http://man7.org/linux/man-pages/man2/read.2.html
/// [signal(7)]: http://man7.org/linux/man-pages/man7/signal.7.html
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct inotify_event {
    /// Identifies the watch for which this event occurs
    ///
    /// This is one of the watch descriptors returned by a previous call to
    /// [`inotify_add_watch()`].
    ///
    /// [`inotify_add_watch()`]: fn.inotify_add_watch.html
    pub wd: c_int,

    /// Describes the type file system event
    ///
    /// One of the following bits will be set, to identify the type of event:
    ///
    /// - [`IN_ACCESS`]
    /// - [`IN_ATTRIB`]
    /// - [`IN_CLOSE_NOWRITE`]
    /// - [`IN_CLOSE_WRITE`]
    /// - [`IN_CREATE`]
    /// - [`IN_DELETE`]
    /// - [`IN_DELETE_SELF`]
    /// - [`IN_IGNORED`]
    /// - [`IN_MODIFY`]
    /// - [`IN_MOVED_FROM`]
    /// - [`IN_MOVED_TO`]
    /// - [`IN_MOVE_SELF`]
    /// - [`IN_OPEN`]
    /// - [`IN_Q_OVERFLOW`]
    /// - [`IN_UNMOUNT`]
    ///
    /// Some constants cover multiple bits, and can be used for a less precise
    /// check of the event type:
    ///
    /// - [`IN_CLOSE`]
    /// - [`IN_MOVE`]
    ///
    /// In addition, the [`IN_ISDIR`] bit can be set.
    ///
    /// [`IN_ACCESS`]: constant.IN_ACCESS.html
    /// [`IN_ATTRIB`]: constant.IN_ATTRIB.html
    /// [`IN_CLOSE`]: constant.IN_CLOSE.html
    /// [`IN_CLOSE_NOWRITE`]: constant.IN_CLOSE_NOWRITE.html
    /// [`IN_CLOSE_WRITE`]: constant.IN_CLOSE_WRITE.html
    /// [`IN_CREATE`]: constant.IN_CREATE.html
    /// [`IN_DELETE`]: constant.IN_DELETE.html
    /// [`IN_DELETE_SELF`]: constant.IN_DELETE_SELF.html
    /// [`IN_IGNORED`]: constant.IN_IGNORED.html
    /// [`IN_ISDIR`]: constant.IN_ISDIR.html
    /// [`IN_MODIFY`]: constant.IN_MODIFY.html
    /// [`IN_MOVE`]: constant.IN_MOVE.html
    /// [`IN_MOVED_FROM`]: constant.IN_MOVED_FROM.html
    /// [`IN_MOVED_TO`]: constant.IN_MOVED_TO.html
    /// [`IN_MOVE_SELF`]: constant.IN_MOVE_SELF.html
    /// [`IN_OPEN`]: constant.IN_OPEN.html
    /// [`IN_Q_OVERFLOW`]: constant.IN_Q_OVERFLOW.html
    /// [`IN_UNMOUNT`]: constant.IN_UNMOUNT.html
    pub mask: u32,

    /// A number that connects related events
    ///
    /// Currently used only for rename events. A related pair of
    /// [`IN_MOVED_FROM`] and [`IN_MOVED_TO`] events will have the same,
    /// non-zero, cookie. For all other events, cookie is 0.
    ///
    /// [`IN_MOVED_FROM`]: constant.IN_MOVED_FROM.html
    /// [`IN_MOVED_TO`]: constant.IN_MOVED_TO.html
    pub cookie: u32,

    /// The length of `name`
    ///
    /// Used to determine the size of this structure. When `name`
    /// isn't present (`name` is only present when an event occurs
    /// for a file inside a watched directory), it is 0. When `name`
    /// *is* present, it counts all of `name`'s bytes, including `\0`.
    ///
    /// > The `name` field is present only when an event is returned for
    /// > a file inside a watched directory; it identifies the file
    /// > pathname relative to the watched directory. This pathname is
    /// > null-terminated, and may include further null bytes ('\0') to
    /// > align subsequent reads to a suitable address boundary.
    ///
    /// The `name` field has been ommited in this struct's definition.
    pub len: u32,
}


extern {
    /// Creates an inotify instance
    ///
    /// If you need more flexibility, consider using [`inotify_init1`] instead.
    ///
    /// Returns `-1`, if an error occured, or an inotify file descriptor
    /// otherwise.
    ///
    /// Please refer to the [man page] for additional details.
    ///
    /// [`inotify_init1`]: fn.inotify_init1.html
    /// [man page]: http://man7.org/linux/man-pages/man2/inotify_init.2.html
    pub fn inotify_init() -> c_int;

    /// Creates an inotify instance
    ///
    /// Takes an argument to configure the new inotify instance. The following
    /// flags can be set:
    ///
    /// - [`IN_CLOEXEC`]
    /// - [`IN_NONBLOCK`]
    ///
    /// Returns `-1`, if an error occured, or an inotify file descriptor
    /// otherwise.
    ///
    /// Please refer to the [man page] for additional details.
    ///
    /// [`IN_CLOEXEC`]: constant.IN_CLOEXEC.html
    /// [`IN_NONBLOCK`]: constant.IN_NONBLOCK.html
    /// [man page]: http://man7.org/linux/man-pages/man2/inotify_init1.2.html
    pub fn inotify_init1(flags: c_int) -> c_int;

    /// Adds or updates an inotify watch
    ///
    /// Adds an item to the watch list of an inotify instance, or modifies an
    /// item on that list. This function takes the following arguments:
    ///
    /// - `fd` is the file descriptor of the inotify instance (created by
    ///   [`inotify_init`] or [`inotify_init1`])
    /// - `pathname` is the path of the file or directory watch
    /// - `mask` defines the behavior of this function and configures the watch
    ///
    /// The following flags in `mask` control the type of events to watch for:
    ///
    /// - [`IN_ACCESS`]
    /// - [`IN_ATTRIB`]
    /// - [`IN_CLOSE_NOWRITE`]
    /// - [`IN_CLOSE_WRITE`]
    /// - [`IN_CREATE`]
    /// - [`IN_DELETE`]
    /// - [`IN_DELETE_SELF`]
    /// - [`IN_MODIFY`]
    /// - [`IN_MOVED_FROM`]
    /// - [`IN_MOVED_TO`]
    /// - [`IN_MOVE_SELF`]
    /// - [`IN_OPEN`]
    ///
    /// The following constants can be used as shortcuts to set multiple event
    /// flags:
    ///
    /// - [`IN_ALL_EVENTS`]
    /// - [`IN_CLOSE`]
    /// - [`IN_MOVE`]
    ///
    /// In addition, the following flags can be set to control the behaviors of
    /// the watch and this function:
    ///
    /// - [`IN_DONT_FOLLOW`]
    /// - [`IN_EXCL_UNLINK`]
    /// - [`IN_MASK_ADD`]
    /// - [`IN_ONESHOT`]
    /// - [`IN_ONLYDIR`]
    ///
    /// The function returns `-1` if an error occured. Otherwise, it returns a
    /// watch descriptor that can be used to remove the watch using
    /// [`inotify_rm_watch`] or identify the watch via [`inotify_event`]'s [wd`]
    /// field.
    ///
    /// Please refer to the [man page] for additional details.
    ///
    /// [`inotify_init`]: fn.inotify_init.html
    /// [`inotify_init1`]: fn.inotify_init1.html
    /// [`IN_ACCESS`]: constant.IN_ACCESS.html
    /// [`IN_ATTRIB`]: constant.IN_ATTRIB.html
    /// [`IN_CLOSE_NOWRITE`]: constant.IN_CLOSE_NOWRITE.html
    /// [`IN_CLOSE_WRITE`]: constant.IN_CLOSE_WRITE.html
    /// [`IN_CREATE`]: constant.IN_CREATE.html
    /// [`IN_DELETE`]: constant.IN_DELETE.html
    /// [`IN_DELETE_SELF`]: constant.IN_DELETE_SELF.html
    /// [`IN_MODIFY`]: constant.IN_MODIFY.html
    /// [`IN_MOVED_FROM`]: constant.IN_MOVED_FROM.html
    /// [`IN_MOVED_TO`]: constant.IN_MOVED_TO.html
    /// [`IN_MOVE_SELF`]: constant.IN_MOVE_SELF.html
    /// [`IN_OPEN`]: constant.IN_OPEN.html
    /// [`IN_ALL_EVENTS`]: constant.IN_ALL_EVENTS.html
    /// [`IN_CLOSE`]: constant.IN_CLOSE.html
    /// [`IN_MOVE`]: constant.IN_MOVE.html
    /// [`IN_DONT_FOLLOW`]: constant.IN_DONT_FOLLOW.html
    /// [`IN_EXCL_UNLINK`]: constant.IN_EXCL_UNLINK.html
    /// [`IN_MASK_ADD`]: constant.IN_MASK_ADD.html
    /// [`IN_ONESHOT`]: constant.IN_ONESHOT.html
    /// [`IN_ONLYDIR`]: constant.IN_ONLYDIR.html
    /// [`inotify_rm_watch`]: fn.inotify_rm_watch.html
    /// [`inotify_event`]: struct.inotify_event.html
    /// [`wd`]: struct.inotify_event.html#structfield.wd
    /// [man page]: http://man7.org/linux/man-pages/man2/inotify_add_watch.2.html
    pub fn inotify_add_watch(fd: c_int, pathname: *const c_char, mask: u32) -> c_int;

    /// Removes an inotify watch
    ///
    /// Removes an item from the watch list of an inotify instance. The inotify
    /// instance is identified by the `fd` argument. The watch is identified by
    /// the `wd` argument.
    ///
    /// Returns `0` on success, `-1` on failure.
    ///
    /// Please refer to the [man page] for additional details.
    ///
    /// [man page]: http://man7.org/linux/man-pages/man2/inotify_rm_watch.2.html
    pub fn inotify_rm_watch(fd: c_int, wd: c_int) -> c_int;
}

pub use libc::{
    close,
    read,
};
