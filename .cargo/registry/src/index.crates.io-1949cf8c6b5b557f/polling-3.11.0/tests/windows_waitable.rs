//! Tests for the waitable polling on Windows.

#![cfg(windows)]

use polling::os::iocp::PollerIocpExt;
use polling::{Event, Events, PollMode, Poller};

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent};

use std::io;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::os::windows::prelude::{AsHandle, BorrowedHandle};
use std::time::Duration;

/// A basic wrapper around the Windows event object.
struct EventHandle(RawHandle);

impl Drop for EventHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0 as _);
        }
    }
}

impl EventHandle {
    fn new(manual_reset: bool) -> io::Result<Self> {
        let handle = unsafe {
            CreateEventW(
                std::ptr::null_mut(),
                manual_reset as _,
                false as _,
                std::ptr::null(),
            )
        };

        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle as _))
        }
    }

    /// Reset the event object.
    fn reset(&self) -> io::Result<()> {
        if unsafe { ResetEvent(self.0 as _) } != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Set the event object.
    fn set(&self) -> io::Result<()> {
        if unsafe { SetEvent(self.0 as _) } != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl AsRawHandle for EventHandle {
    fn as_raw_handle(&self) -> RawHandle {
        self.0
    }
}

impl AsHandle for EventHandle {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.0) }
    }
}

#[test]
fn smoke() {
    let poller = Poller::new().unwrap();

    let event = EventHandle::new(true).unwrap();

    unsafe {
        poller
            .add_waitable(&event, Event::all(0), PollMode::Oneshot)
            .unwrap();
    }

    let mut events = Events::new();
    poller
        .wait(&mut events, Some(Duration::from_millis(100)))
        .unwrap();

    assert!(events.is_empty());

    // Signal the event.
    event.set().unwrap();

    poller
        .wait(&mut events, Some(Duration::from_millis(100)))
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events.iter().next().unwrap().with_no_extra(), Event::all(0));

    // Interest should be cleared.
    events.clear();
    poller
        .wait(&mut events, Some(Duration::from_millis(100)))
        .unwrap();

    assert!(events.is_empty());

    // If we modify the waitable, it should be added again.
    poller
        .modify_waitable(&event, Event::all(0), PollMode::Oneshot)
        .unwrap();

    events.clear();
    poller
        .wait(&mut events, Some(Duration::from_millis(100)))
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events.iter().next().unwrap().with_no_extra(), Event::all(0));

    // If we reset the event, it should not be signaled.
    event.reset().unwrap();
    poller
        .modify_waitable(&event, Event::all(0), PollMode::Oneshot)
        .unwrap();

    events.clear();
    poller
        .wait(&mut events, Some(Duration::from_millis(100)))
        .unwrap();

    assert!(events.is_empty());
}
