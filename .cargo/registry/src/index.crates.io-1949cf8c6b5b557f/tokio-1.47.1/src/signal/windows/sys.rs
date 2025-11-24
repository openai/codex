use std::io;
use std::sync::Once;

use crate::signal::registry::{globals, EventId, EventInfo, Init, Storage};
use crate::signal::RxFuture;

use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::Console as console;

pub(super) fn ctrl_break() -> io::Result<RxFuture> {
    new(console::CTRL_BREAK_EVENT)
}

pub(super) fn ctrl_close() -> io::Result<RxFuture> {
    new(console::CTRL_CLOSE_EVENT)
}

pub(super) fn ctrl_c() -> io::Result<RxFuture> {
    new(console::CTRL_C_EVENT)
}

pub(super) fn ctrl_logoff() -> io::Result<RxFuture> {
    new(console::CTRL_LOGOFF_EVENT)
}

pub(super) fn ctrl_shutdown() -> io::Result<RxFuture> {
    new(console::CTRL_SHUTDOWN_EVENT)
}

fn new(signum: u32) -> io::Result<RxFuture> {
    global_init()?;
    let rx = globals().register_listener(signum as EventId);
    Ok(RxFuture::new(rx))
}

fn event_requires_infinite_sleep_in_handler(signum: u32) -> bool {
    // Returning from the handler function of those events immediately terminates the process.
    // So for async systems, the easiest solution is to simply never return from
    // the handler function.
    //
    // For more information, see:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine#remarks
    match signum {
        console::CTRL_CLOSE_EVENT => true,
        console::CTRL_LOGOFF_EVENT => true,
        console::CTRL_SHUTDOWN_EVENT => true,
        _ => false,
    }
}

#[derive(Debug)]
pub(crate) struct OsStorage {
    ctrl_break: EventInfo,
    ctrl_close: EventInfo,
    ctrl_c: EventInfo,
    ctrl_logoff: EventInfo,
    ctrl_shutdown: EventInfo,
}

impl Init for OsStorage {
    fn init() -> Self {
        Self {
            ctrl_break: Default::default(),
            ctrl_close: Default::default(),
            ctrl_c: Default::default(),
            ctrl_logoff: Default::default(),
            ctrl_shutdown: Default::default(),
        }
    }
}

impl Storage for OsStorage {
    fn event_info(&self, id: EventId) -> Option<&EventInfo> {
        match u32::try_from(id) {
            Ok(console::CTRL_BREAK_EVENT) => Some(&self.ctrl_break),
            Ok(console::CTRL_CLOSE_EVENT) => Some(&self.ctrl_close),
            Ok(console::CTRL_C_EVENT) => Some(&self.ctrl_c),
            Ok(console::CTRL_LOGOFF_EVENT) => Some(&self.ctrl_logoff),
            Ok(console::CTRL_SHUTDOWN_EVENT) => Some(&self.ctrl_shutdown),
            _ => None,
        }
    }

    fn for_each<'a, F>(&'a self, mut f: F)
    where
        F: FnMut(&'a EventInfo),
    {
        f(&self.ctrl_break);
        f(&self.ctrl_close);
        f(&self.ctrl_c);
        f(&self.ctrl_logoff);
        f(&self.ctrl_shutdown);
    }
}

#[derive(Debug)]
pub(crate) struct OsExtraData {}

impl Init for OsExtraData {
    fn init() -> Self {
        Self {}
    }
}

fn global_init() -> io::Result<()> {
    static INIT: Once = Once::new();

    let mut init = None;

    INIT.call_once(|| unsafe {
        let rc = console::SetConsoleCtrlHandler(Some(handler), 1);
        let ret = if rc == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };

        init = Some(ret);
    });

    init.unwrap_or_else(|| Ok(()))
}

unsafe extern "system" fn handler(ty: u32) -> BOOL {
    let globals = globals();
    globals.record_event(ty as EventId);

    // According to https://docs.microsoft.com/en-us/windows/console/handlerroutine
    // the handler routine is always invoked in a new thread, thus we don't
    // have the same restrictions as in Unix signal handlers, meaning we can
    // go ahead and perform the broadcast here.
    let event_was_handled = globals.broadcast();

    if event_was_handled && event_requires_infinite_sleep_in_handler(ty) {
        loop {
            std::thread::park();
        }
    }

    if event_was_handled {
        1
    } else {
        // No one is listening for this notification any more
        // let the OS fire the next (possibly the default) handler.
        0
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    use tokio_test::{assert_ok, assert_pending, assert_ready_ok, task};

    unsafe fn raise_event(signum: u32) {
        if event_requires_infinite_sleep_in_handler(signum) {
            // Those events will enter an infinite loop in `handler`, so
            // we need to run them on a separate thread
            std::thread::spawn(move || super::handler(signum));
        } else {
            super::handler(signum);
        }
    }

    #[test]
    fn ctrl_c() {
        let rt = rt();
        let _enter = rt.enter();

        let mut ctrl_c = task::spawn(crate::signal::ctrl_c());

        assert_pending!(ctrl_c.poll());

        // Windows doesn't have a good programmatic way of sending events
        // like sending signals on Unix, so we'll stub out the actual OS
        // integration and test that our handling works.
        unsafe {
            raise_event(console::CTRL_C_EVENT);
        }

        assert_ready_ok!(ctrl_c.poll());
    }

    #[test]
    fn ctrl_break() {
        let rt = rt();

        rt.block_on(async {
            let mut ctrl_break = assert_ok!(crate::signal::windows::ctrl_break());

            // Windows doesn't have a good programmatic way of sending events
            // like sending signals on Unix, so we'll stub out the actual OS
            // integration and test that our handling works.
            unsafe {
                raise_event(console::CTRL_BREAK_EVENT);
            }

            ctrl_break.recv().await.unwrap();
        });
    }

    #[test]
    fn ctrl_close() {
        let rt = rt();

        rt.block_on(async {
            let mut ctrl_close = assert_ok!(crate::signal::windows::ctrl_close());

            // Windows doesn't have a good programmatic way of sending events
            // like sending signals on Unix, so we'll stub out the actual OS
            // integration and test that our handling works.
            unsafe {
                raise_event(console::CTRL_CLOSE_EVENT);
            }

            ctrl_close.recv().await.unwrap();
        });
    }

    #[test]
    fn ctrl_shutdown() {
        let rt = rt();

        rt.block_on(async {
            let mut ctrl_shutdown = assert_ok!(crate::signal::windows::ctrl_shutdown());

            // Windows doesn't have a good programmatic way of sending events
            // like sending signals on Unix, so we'll stub out the actual OS
            // integration and test that our handling works.
            unsafe {
                raise_event(console::CTRL_SHUTDOWN_EVENT);
            }

            ctrl_shutdown.recv().await.unwrap();
        });
    }

    #[test]
    fn ctrl_logoff() {
        let rt = rt();

        rt.block_on(async {
            let mut ctrl_logoff = assert_ok!(crate::signal::windows::ctrl_logoff());

            // Windows doesn't have a good programmatic way of sending events
            // like sending signals on Unix, so we'll stub out the actual OS
            // integration and test that our handling works.
            unsafe {
                raise_event(console::CTRL_LOGOFF_EVENT);
            }

            ctrl_logoff.recv().await.unwrap();
        });
    }

    fn rt() -> Runtime {
        crate::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
}
