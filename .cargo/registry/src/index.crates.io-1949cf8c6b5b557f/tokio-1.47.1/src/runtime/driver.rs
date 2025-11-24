//! Abstracts out the entire chain of runtime sub-drivers into common types.

// Eventually, this file will see significant refactoring / cleanup. For now, we
// don't need to worry much about dead code with certain feature permutations.
#![cfg_attr(
    any(not(all(tokio_unstable, feature = "full")), target_family = "wasm"),
    allow(dead_code)
)]

use crate::runtime::park::{ParkThread, UnparkThread};

use std::io;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct Driver {
    inner: TimeDriver,
}

#[derive(Debug)]
pub(crate) struct Handle {
    /// IO driver handle
    pub(crate) io: IoHandle,

    /// Signal driver handle
    #[cfg_attr(any(not(unix), loom), allow(dead_code))]
    pub(crate) signal: SignalHandle,

    /// Time driver handle
    pub(crate) time: TimeHandle,

    /// Source of `Instant::now()`
    #[cfg_attr(not(all(feature = "time", feature = "test-util")), allow(dead_code))]
    pub(crate) clock: Clock,
}

pub(crate) struct Cfg {
    pub(crate) enable_io: bool,
    pub(crate) enable_time: bool,
    pub(crate) enable_pause_time: bool,
    pub(crate) start_paused: bool,
    pub(crate) nevents: usize,
}

impl Driver {
    pub(crate) fn new(cfg: Cfg) -> io::Result<(Self, Handle)> {
        let (io_stack, io_handle, signal_handle) = create_io_stack(cfg.enable_io, cfg.nevents)?;

        let clock = create_clock(cfg.enable_pause_time, cfg.start_paused);

        let (time_driver, time_handle) = create_time_driver(cfg.enable_time, io_stack, &clock);

        Ok((
            Self { inner: time_driver },
            Handle {
                io: io_handle,
                signal: signal_handle,
                time: time_handle,
                clock,
            },
        ))
    }

    pub(crate) fn park(&mut self, handle: &Handle) {
        self.inner.park(handle);
    }

    pub(crate) fn park_timeout(&mut self, handle: &Handle, duration: Duration) {
        self.inner.park_timeout(handle, duration);
    }

    pub(crate) fn shutdown(&mut self, handle: &Handle) {
        self.inner.shutdown(handle);
    }
}

impl Handle {
    pub(crate) fn unpark(&self) {
        #[cfg(feature = "time")]
        if let Some(handle) = &self.time {
            handle.unpark();
        }

        self.io.unpark();
    }

    cfg_io_driver! {
        #[track_caller]
        pub(crate) fn io(&self) -> &crate::runtime::io::Handle {
            self.io
                .as_ref()
                .expect("A Tokio 1.x context was found, but IO is disabled. Call `enable_io` on the runtime builder to enable IO.")
        }
    }

    cfg_signal_internal_and_unix! {
        #[track_caller]
        pub(crate) fn signal(&self) -> &crate::runtime::signal::Handle {
            self.signal
                .as_ref()
                .expect("there is no signal driver running, must be called from the context of Tokio runtime")
        }
    }

    cfg_time! {
        /// Returns a reference to the time driver handle.
        ///
        /// Panics if no time driver is present.
        #[track_caller]
        pub(crate) fn time(&self) -> &crate::runtime::time::Handle {
            self.time
                .as_ref()
                .expect("A Tokio 1.x context was found, but timers are disabled. Call `enable_time` on the runtime builder to enable timers.")
        }

        pub(crate) fn clock(&self) -> &Clock {
            &self.clock
        }
    }
}

// ===== io driver =====

cfg_io_driver! {
    pub(crate) type IoDriver = crate::runtime::io::Driver;

    #[derive(Debug)]
    pub(crate) enum IoStack {
        Enabled(ProcessDriver),
        Disabled(ParkThread),
    }

    #[derive(Debug)]
    pub(crate) enum IoHandle {
        Enabled(crate::runtime::io::Handle),
        Disabled(UnparkThread),
    }

    fn create_io_stack(enabled: bool, nevents: usize) -> io::Result<(IoStack, IoHandle, SignalHandle)> {
        #[cfg(loom)]
        assert!(!enabled);

        let ret = if enabled {
            let (io_driver, io_handle) = crate::runtime::io::Driver::new(nevents)?;

            let (signal_driver, signal_handle) = create_signal_driver(io_driver, &io_handle)?;
            let process_driver = create_process_driver(signal_driver);

            (IoStack::Enabled(process_driver), IoHandle::Enabled(io_handle), signal_handle)
        } else {
            let park_thread = ParkThread::new();
            let unpark_thread = park_thread.unpark();
            (IoStack::Disabled(park_thread), IoHandle::Disabled(unpark_thread), Default::default())
        };

        Ok(ret)
    }

    impl IoStack {
        pub(crate) fn park(&mut self, handle: &Handle) {
            match self {
                IoStack::Enabled(v) => v.park(handle),
                IoStack::Disabled(v) => v.park(),
            }
        }

        pub(crate) fn park_timeout(&mut self, handle: &Handle, duration: Duration) {
            match self {
                IoStack::Enabled(v) => v.park_timeout(handle, duration),
                IoStack::Disabled(v) => v.park_timeout(duration),
            }
        }

        pub(crate) fn shutdown(&mut self, handle: &Handle) {
            match self {
                IoStack::Enabled(v) => v.shutdown(handle),
                IoStack::Disabled(v) => v.shutdown(),
            }
        }
    }

    impl IoHandle {
        pub(crate) fn unpark(&self) {
            match self {
                IoHandle::Enabled(handle) => handle.unpark(),
                IoHandle::Disabled(handle) => handle.unpark(),
            }
        }

        pub(crate) fn as_ref(&self) -> Option<&crate::runtime::io::Handle> {
            match self {
                IoHandle::Enabled(v) => Some(v),
                IoHandle::Disabled(..) => None,
            }
        }
    }
}

cfg_not_io_driver! {
    pub(crate) type IoHandle = UnparkThread;

    #[derive(Debug)]
    pub(crate) struct IoStack(ParkThread);

    fn create_io_stack(_enabled: bool, _nevents: usize) -> io::Result<(IoStack, IoHandle, SignalHandle)> {
        let park_thread = ParkThread::new();
        let unpark_thread = park_thread.unpark();
        Ok((IoStack(park_thread), unpark_thread, Default::default()))
    }

    impl IoStack {
        pub(crate) fn park(&mut self, _handle: &Handle) {
            self.0.park();
        }

        pub(crate) fn park_timeout(&mut self, _handle: &Handle, duration: Duration) {
            self.0.park_timeout(duration);
        }

        pub(crate) fn shutdown(&mut self, _handle: &Handle) {
            self.0.shutdown();
        }

        /// This is not a "real" driver, so it is not considered enabled.
        pub(crate) fn is_enabled(&self) -> bool {
            false
        }
    }
}

// ===== signal driver =====

cfg_signal_internal_and_unix! {
    type SignalDriver = crate::runtime::signal::Driver;
    pub(crate) type SignalHandle = Option<crate::runtime::signal::Handle>;

    fn create_signal_driver(io_driver: IoDriver, io_handle: &crate::runtime::io::Handle) -> io::Result<(SignalDriver, SignalHandle)> {
        let driver = crate::runtime::signal::Driver::new(io_driver, io_handle)?;
        let handle = driver.handle();
        Ok((driver, Some(handle)))
    }
}

cfg_not_signal_internal! {
    pub(crate) type SignalHandle = ();

    cfg_io_driver! {
        type SignalDriver = IoDriver;

        fn create_signal_driver(io_driver: IoDriver, _io_handle: &crate::runtime::io::Handle) -> io::Result<(SignalDriver, SignalHandle)> {
            Ok((io_driver, ()))
        }
    }
}

// ===== process driver =====

cfg_process_driver! {
    type ProcessDriver = crate::runtime::process::Driver;

    fn create_process_driver(signal_driver: SignalDriver) -> ProcessDriver {
        ProcessDriver::new(signal_driver)
    }
}

cfg_not_process_driver! {
    cfg_io_driver! {
        type ProcessDriver = SignalDriver;

        fn create_process_driver(signal_driver: SignalDriver) -> ProcessDriver {
            signal_driver
        }
    }
}

// ===== time driver =====

cfg_time! {
    #[derive(Debug)]
    pub(crate) enum TimeDriver {
        Enabled {
            driver: crate::runtime::time::Driver,
        },
        Disabled(IoStack),
    }

    pub(crate) type Clock = crate::time::Clock;
    pub(crate) type TimeHandle = Option<crate::runtime::time::Handle>;

    fn create_clock(enable_pausing: bool, start_paused: bool) -> Clock {
        crate::time::Clock::new(enable_pausing, start_paused)
    }

    fn create_time_driver(
        enable: bool,
        io_stack: IoStack,
        clock: &Clock,
    ) -> (TimeDriver, TimeHandle) {
        if enable {
            let (driver, handle) = crate::runtime::time::Driver::new(io_stack, clock);

            (TimeDriver::Enabled { driver }, Some(handle))
        } else {
            (TimeDriver::Disabled(io_stack), None)
        }
    }

    impl TimeDriver {
        pub(crate) fn park(&mut self, handle: &Handle) {
            match self {
                TimeDriver::Enabled { driver, .. } => driver.park(handle),
                TimeDriver::Disabled(v) => v.park(handle),
            }
        }

        pub(crate) fn park_timeout(&mut self, handle: &Handle, duration: Duration) {
            match self {
                TimeDriver::Enabled { driver } => driver.park_timeout(handle, duration),
                TimeDriver::Disabled(v) => v.park_timeout(handle, duration),
            }
        }

        pub(crate) fn shutdown(&mut self, handle: &Handle) {
            match self {
                TimeDriver::Enabled { driver } => driver.shutdown(handle),
                TimeDriver::Disabled(v) => v.shutdown(handle),
            }
        }
    }
}

cfg_not_time! {
    type TimeDriver = IoStack;

    pub(crate) type Clock = ();
    pub(crate) type TimeHandle = ();

    fn create_clock(_enable_pausing: bool, _start_paused: bool) -> Clock {
        ()
    }

    fn create_time_driver(
        _enable: bool,
        io_stack: IoStack,
        _clock: &Clock,
    ) -> (TimeDriver, TimeHandle) {
        (io_stack, ())
    }
}

cfg_tokio_uring! {
    pub(crate) mod op;
}
