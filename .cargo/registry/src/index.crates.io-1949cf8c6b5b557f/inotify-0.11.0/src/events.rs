use std::{
    ffi::{
        OsStr,
        OsString,
    },
    mem,
    os::unix::ffi::OsStrExt,
    sync::Weak,
};

use inotify_sys as ffi;

use crate::fd_guard::FdGuard;
use crate::watches::WatchDescriptor;


/// Iterator over inotify events
///
/// Allows for iteration over the events returned by
/// [`Inotify::read_events_blocking`] or [`Inotify::read_events`].
///
/// [`Inotify::read_events_blocking`]: crate::Inotify::read_events_blocking
/// [`Inotify::read_events`]: crate::Inotify::read_events
#[derive(Debug)]
pub struct Events<'a> {
    fd       : Weak<FdGuard>,
    buffer   : &'a [u8],
    num_bytes: usize,
    pos      : usize,
}

impl<'a> Events<'a> {
    pub(crate) fn new(fd: Weak<FdGuard>, buffer: &'a [u8], num_bytes: usize)
        -> Self
    {
        Events {
            fd,
            buffer,
            num_bytes,
            pos: 0,
        }
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = Event<&'a OsStr>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.num_bytes {
            let (step, event) = Event::from_buffer(self.fd.clone(), &self.buffer[self.pos..]);
            self.pos += step;

            Some(event)
        }
        else {
            None
        }
    }
}


/// An inotify event
///
/// A file system event that describes a change that the user previously
/// registered interest in. To watch for events, call [`Watches::add`]. To
/// retrieve events, call [`Inotify::read_events_blocking`] or
/// [`Inotify::read_events`].
///
/// [`Watches::add`]: crate::Watches::add
/// [`Inotify::read_events_blocking`]: crate::Inotify::read_events_blocking
/// [`Inotify::read_events`]: crate::Inotify::read_events
#[derive(Clone, Debug)]
pub struct Event<S> {
    /// Identifies the watch this event originates from
    ///
    /// This [`WatchDescriptor`] is equal to the one that [`Watches::add`]
    /// returned when interest for this event was registered. The
    /// [`WatchDescriptor`] can be used to remove the watch using
    /// [`Watches::remove`], thereby preventing future events of this type
    /// from being created.
    ///
    /// [`Watches::add`]: crate::Watches::add
    /// [`Watches::remove`]: crate::Watches::remove
    pub wd: WatchDescriptor,

    /// Indicates what kind of event this is
    pub mask: EventMask,

    /// Connects related events to each other
    ///
    /// When a file is renamed, this results two events: [`MOVED_FROM`] and
    /// [`MOVED_TO`]. The `cookie` field will be the same for both of them,
    /// thereby making is possible to connect the event pair.
    ///
    /// [`MOVED_FROM`]: EventMask::MOVED_FROM
    /// [`MOVED_TO`]: EventMask::MOVED_TO
    pub cookie: u32,

    /// The name of the file the event originates from
    ///
    /// This field is set only if the subject of the event is a file or directory in a
    /// watched directory. If the event concerns a file or directory that is
    /// watched directly, `name` will be `None`.
    pub name: Option<S>,
}

impl<'a> Event<&'a OsStr> {
    fn new(fd: Weak<FdGuard>, event: &ffi::inotify_event, name: &'a OsStr)
        -> Self
    {
        let mask = EventMask::from_bits(event.mask)
            .expect("Failed to convert event mask. This indicates a bug.");

        let wd = crate::WatchDescriptor {
            id: event.wd,
            fd,
        };

        let name = if name.is_empty() {
            None
        }
        else {
            Some(name)
        };

        Event {
            wd,
            mask,
            cookie: event.cookie,
            name,
        }
    }

    /// Create an `Event` from a buffer
    ///
    /// Assumes that a full `inotify_event` plus its name is located at the
    /// beginning of `buffer`.
    ///
    /// Returns the number of bytes used from the buffer, and the event.
    ///
    /// # Panics
    ///
    /// Panics if the buffer does not contain a full event, including its name.
    pub(crate) fn from_buffer(
        fd    : Weak<FdGuard>,
        buffer: &'a [u8],
    )
        -> (usize, Self)
    {
        let event_size = mem::size_of::<ffi::inotify_event>();

        // Make sure that the buffer is big enough to contain an event, without
        // the name. Otherwise we can't safely convert it to an `inotify_event`.
        assert!(buffer.len() >= event_size);

        let ffi_event_ptr = buffer.as_ptr() as *const ffi::inotify_event;

        // We have a pointer to an `inotify_event`, pointing to the beginning of
        // `buffer`. Since we know, as per the assertion above, that there are
        // enough bytes in the buffer for at least one event, we can safely
        // read that `inotify_event`.
        // We call `read_unaligned()` since the byte buffer has alignment 1
        // and `inotify_event` has a higher alignment, so `*` cannot be used to dereference
        // the unaligned pointer (undefined behavior).
        let ffi_event = unsafe { ffi_event_ptr.read_unaligned() };

        // The name's length is given by `event.len`. There should always be
        // enough bytes left in the buffer to fit the name. Let's make sure that
        // is the case.
        let bytes_left_in_buffer = buffer.len() - event_size;
        assert!(bytes_left_in_buffer >= ffi_event.len as usize);

        // Directly after the event struct should be a name, if there's one
        // associated with the event. Let's make a new slice that starts with
        // that name. If there's no name, this slice might have a length of `0`.
        let bytes_consumed = event_size + ffi_event.len as usize;
        let name = &buffer[event_size..bytes_consumed];

        // Remove trailing '\0' bytes
        //
        // The events in the buffer are aligned, and `name` is filled up
        // with '\0' up to the alignment boundary. Here we remove those
        // additional bytes.
        //
        // The `unwrap` here is safe, because `splitn` always returns at
        // least one result, even if the original slice contains no '\0'.
        let name = name
            .splitn(2, |b| b == &0u8)
            .next()
            .unwrap();

        let event = Event::new(
            fd,
            &ffi_event,
            OsStr::from_bytes(name),
        );

        (bytes_consumed, event)
    }

    /// Returns an owned copy of the event.
    #[deprecated = "use `to_owned()` instead; methods named `into_owned()` usually take self by value"]
    #[allow(clippy::wrong_self_convention)]
    pub fn into_owned(&self) -> EventOwned {
        self.to_owned()
    }

    /// Returns an owned copy of the event.
    #[must_use = "cloning is often expensive and is not expected to have side effects"]
    pub fn to_owned(&self) -> EventOwned {
        Event {
            wd: self.wd.clone(),
            mask: self.mask,
            cookie: self.cookie,
            name: self.name.map(OsStr::to_os_string),
        }
    }
}


/// An owned version of `Event`
pub type EventOwned = Event<OsString>;


bitflags! {
    /// Indicates the type of an event
    ///
    /// This struct can be retrieved from an [`Event`] via its `mask` field.
    /// You can determine the [`Event`]'s type by comparing the `EventMask` to
    /// its associated constants.
    ///
    /// Please refer to the documentation of [`Event`] for a usage example.
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct EventMask: u32 {
        /// File was accessed
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_ACCESS`].
        const ACCESS = ffi::IN_ACCESS;

        /// Metadata (permissions, timestamps, ...) changed
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_ATTRIB`].
        const ATTRIB = ffi::IN_ATTRIB;

        /// File opened for writing was closed
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_CLOSE_WRITE`].
        const CLOSE_WRITE = ffi::IN_CLOSE_WRITE;

        /// File or directory not opened for writing was closed
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_CLOSE_NOWRITE`].
        const CLOSE_NOWRITE = ffi::IN_CLOSE_NOWRITE;

        /// File/directory created in watched directory
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_CREATE`].
        const CREATE = ffi::IN_CREATE;

        /// File/directory deleted from watched directory
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        const DELETE = ffi::IN_DELETE;

        /// Watched file/directory was deleted
        ///
        /// See [`inotify_sys::IN_DELETE_SELF`].
        const DELETE_SELF = ffi::IN_DELETE_SELF;

        /// File was modified
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MODIFY`].
        const MODIFY = ffi::IN_MODIFY;

        /// Watched file/directory was moved
        ///
        /// See [`inotify_sys::IN_MOVE_SELF`].
        const MOVE_SELF = ffi::IN_MOVE_SELF;

        /// File was renamed/moved; watched directory contained old name
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MOVED_FROM`].
        const MOVED_FROM = ffi::IN_MOVED_FROM;

        /// File was renamed/moved; watched directory contains new name
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MOVED_TO`].
        const MOVED_TO = ffi::IN_MOVED_TO;

        /// File or directory was opened
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_OPEN`].
        const OPEN = ffi::IN_OPEN;

        /// Watch was removed
        ///
        /// This event will be generated, if the watch was removed explicitly
        /// (via [`Watches::remove`]), or automatically (because the file was
        /// deleted or the file system was unmounted).
        ///
        /// See [`inotify_sys::IN_IGNORED`].
        ///
        /// [`Watches::remove`]: crate::Watches::remove
        const IGNORED = ffi::IN_IGNORED;

        /// Event related to a directory
        ///
        /// The subject of the event is a directory.
        ///
        /// See [`inotify_sys::IN_ISDIR`].
        const ISDIR = ffi::IN_ISDIR;

        /// Event queue overflowed
        ///
        /// The event queue has overflowed and events have presumably been lost.
        ///
        /// See [`inotify_sys::IN_Q_OVERFLOW`].
        const Q_OVERFLOW = ffi::IN_Q_OVERFLOW;

        /// File system containing watched object was unmounted.
        /// File system was unmounted
        ///
        /// The file system that contained the watched object has been
        /// unmounted. An event with [`EventMask::IGNORED`] will subsequently be
        /// generated for the same watch descriptor.
        ///
        /// See [`inotify_sys::IN_UNMOUNT`].
        const UNMOUNT = ffi::IN_UNMOUNT;
    }
}

impl EventMask {
    /// Wrapper around [`Self::from_bits_retain`] for backwards compatibility
    ///
    /// # Safety
    ///
    /// This function is not actually unsafe. It is just a wrapper around the
    /// safe [`Self::from_bits_retain`].
    #[deprecated = "Use the safe `from_bits_retain` method instead"]
    pub unsafe fn from_bits_unchecked(bits: u32) -> Self {
        Self::from_bits_retain(bits)
    }
}


#[cfg(test)]
mod tests {
    use std::{
        io::prelude::*,
        mem,
        slice,
        sync,
    };

    use inotify_sys as ffi;

    use super::Event;


    #[test]
    fn from_buffer_should_not_mistake_next_event_for_name_of_previous_event() {
        let mut buffer = [0u8; 1024];

        // First, put a normal event into the buffer
        let event = ffi::inotify_event {
            wd:     0,
            mask:   0,
            cookie: 0,
            len:    0, // no name following after event
        };
        let event = unsafe {
                slice::from_raw_parts(
                &event as *const _ as *const u8,
                mem::size_of_val(&event),
            )
        };
        (&mut buffer[..]).write(event)
            .expect("Failed to write into buffer");

        // After that event, simulate an event that starts with a non-zero byte.
        buffer[mem::size_of_val(&event)] = 1;

        // Now create the event and verify that the name is actually `None`, as
        // dictated by the value `len` above.
        let (_, event) = Event::from_buffer(
            sync::Weak::new(),
            &buffer,
        );
        assert_eq!(event.name, None);
    }
}
