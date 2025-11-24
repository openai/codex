#![doc(
    test(attr(deny(warnings))),
    test(attr(allow(bare_trait_objects, unknown_lints)))
)]
#![warn(missing_docs)]
// Don't fail on links to things not enabled in features
#![allow(
    unknown_lints,
    renamed_and_removed_lints,
    intra_doc_link_resolution_failure,
    broken_intra_doc_links
)]
// These little nifty labels saying that something needs a feature to be enabled
#![cfg_attr(docsrs, feature(doc_cfg))]
//! Library for easier and safe Unix signal handling
//!
//! Unix signals are inherently hard to handle correctly, for several reasons:
//!
//! * They are a global resource. If a library wants to set its own signal handlers, it risks
//!   disrupting some other library. It is possible to chain the previous signal handler, but then
//!   it is impossible to remove the old signal handlers from the chains in any practical manner.
//! * They can be called from whatever thread, requiring synchronization. Also, as they can
//!   interrupt a thread at any time, making most handling race-prone.
//! * According to the POSIX standard, the set of functions one may call inside a signal handler is
//!   limited to very few of them. To highlight, mutexes (or other locking mechanisms) and memory
//!   allocation and deallocation is *not* allowed.
//!
//! # The goal of the library
//!
//! The aim is to subscriptions to signals a „structured“ resource, in a similar way memory
//! allocation is ‒ parts of the program can independently subscribe and it's the same part of the
//! program that can give them up, independently of what the other parts do. Therefore, it is
//! possible to register multiple actions to the same signal.
//!
//! Another goal is to shield applications away from differences between platforms. Various Unix
//! systems have little quirks and differences that need to be worked around and that's not
//! something every application should be dealing with. We even try to provide some support for
//! Windows, but we lack the expertise in that area, so that one is not complete and is a bit rough
//! (if you know how it works there and are willing to either contribute the code or consult,
//! please get in touch).
//!
//! Furthermore, it provides implementation of certain common signal-handling patterns, usable from
//! safe Rust, without the application author needing to learn about *all* the traps.
//!
//! Note that despite everything, there are still some quirks around signal handling that are not
//! possible to paper over and need to be considered. Also, there are some signal use cases that
//! are inherently unsafe and they are not covered by this crate.
//!
//! # Anatomy of the crate
//!
//! The crate is split into several modules.
//!
//! The easiest way to handle signals is using the [`Signals`][crate::iterator::Signals] iterator
//! thing. It can register for a set of signals and produce them one by one, in a blocking manner.
//! You can reserve a thread for handling them as they come. If you want something asynchronous,
//! there are adaptor crates for the most common asynchronous runtimes. The module also contains
//! ways to build iterators that produce a bit more information than just the signal number.
//!
//! The [`flag`] module contains routines to set a flag based on incoming signals and to do
//! certain actions inside the signal handlers based on the flags (the flags can also be
//! manipulated by the rest of the application). This allows building things like checking if a
//! signal happened on each loop iteration or making sure application shuts down on the second
//! CTRL+C if it got stuck in graceful shutdown requested by the first.
//!
//! The [`consts`] module contains some constants, most importantly the signal numbers themselves
//! (these are just re-exports from [`libc`] and if your OS has some extra ones, you can use them
//! too, this is just for convenience).
//!
//! And last, there is the [`low_level`] module. It contains routines to directly register and
//! unregister arbitrary actions. Some of the patterns in the above modules return a [`SigId`],
//! which can be used with the [`low_level::unregister`] to remove the action. There are also some
//! other utilities that are more suited to build other abstractions with than to use directly.
//!
//! Certain parts of the library can be enabled or disabled with use flags:
//!
//! * `channel`: The [low_level::channel] module (on by default).
//! * `iterator`: The [iterator] module (on by default).
//! * `extended-sig-info`: Support for providing more information in the iterators or from the
//!   async adaptor crates. This is off by default.
//!
//! # Limitations
//!
//! * OS limitations still apply. Certain signals are not possible to override or subscribe to ‒
//!   `SIGKILL` or `SIGSTOP`.
//! * Overriding some others is probably a very stupid idea (or very unusual needs) ‒ handling eg.
//!   `SIGSEGV` is not something done lightly. For that reason, the crate will panic in case
//!   registering of these is attempted (see [`FORBIDDEN`][crate::consts::FORBIDDEN]. If you still
//!   need to do so, you can find such APIs in the `signal-hook-registry` backend crate, but
//!   additional care must be taken.
//! * Interaction with other signal-handling libraries is limited. If signal-hook finds an existing
//!   handler present, it chain-calls it from the signal it installs and assumes other libraries
//!   would do the same, but that's everything that can be done to make it work with libraries not
//!   based on [`signal-hook-registry`](https://lib.rs/signal-hook-registry)
//!   (the backend of this crate).
//! * The above chaining contains a race condition in multi-threaded programs, where the previous
//!   handler might not get called if it is received during the registration process. This is
//!   handled (at least on non-windows platforms) on the same thread where the registration
//!   happens, therefore it is advised to register at least one action for each signal of interest
//!   early, before any additional threads are started. Registering any additional (or removing and
//!   registering again) action on the same signal is without the race condition.
//! * Once at least one action is registered for a signal, the default action is replaced (this is
//!   how signals work in the OS). Even if all actions of that signal are removed, `signal-hook`
//!   does not restore the default handler (such behaviour would be at times inconsistent with
//!   making the actions independent and there's no reasonable way to do so in a race-free way in a
//!   multi-threaded program while also dealing with signal handlers registered with other
//!   libraries). It is, however, possible to *emulate* the default handler (see the
//!   [`emulate_default_handler`][low_level::emulate_default_handler]) ‒ there are only 4
//!   default handlers:
//!   - Ignore. This is easy to emulate.
//!   - Abort. Depending on if you call it from within a signal handler of from outside, the
//!     [`low_level::abort`] or [`std::process::abort`] can be used.
//!   - Terminate. This can be done with `exit` ([`low_level::exit`] or [`std::process::exit`]).
//!   - Stop. It is possible to [`raise`][low_level::raise] the [`SIGSTOP`][consts::SIGSTOP] signal.
//!     That one can't be replaced and always stops the application.
//! * Many of the patterns here can collate multiple instances of the same signal into fewer
//!   instances, if the application doesn't consume them fast enough. This is consistent with what
//!   the kernel does if the application doesn't keep up with them (at least for non-realtime
//!   signals, see below), so it is something one needs to deal with anyway.
//! * (By design) the library mostly _postpones_ or helps the user postpone acting on the signals
//!   until later. This, in combination with the above collating inside the library may make it
//!   unsuitable for realtime signals. These usually want to be handled directly inside the signal
//!   handler ‒ which still can be done with [signal_hook_registry::register], but using unsafe and
//!   due care. Patterns for working safely with realtime signals are not unwanted in the library,
//!   but nobody contributed them yet.
//!
//! # Signal masks
//!
//! As the library uses `sigaction` under the hood, signal masking works as expected (eg. with
//! `pthread_sigmask`). This means, signals will *not* be delivered if the signal is masked in all
//! program's threads.
//!
//! By the way, if you do want to modify the signal mask (or do other Unix-specific magic), the
//! [nix](https://lib.rs/crates/nix) crate offers safe interface to many low-level functions,
//! including
//! [`pthread_sigmask`](https://docs.rs/nix/0.11.0/nix/sys/signal/fn.pthread_sigmask.html).
//!
//! # Portability
//!
//! It should work on any POSIX.1-2001 system, which are all the major big OSes with the notable
//! exception of Windows.
//!
//! Non-standard signals are also supported. Pass the signal value directly from `libc` or use
//! the numeric value directly.
//!
//! ```rust
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool};
//! let term = Arc::new(AtomicBool::new(false));
//! let _ = signal_hook::flag::register(libc::SIGINT, Arc::clone(&term));
//! ```
//!
//! This crate includes a limited support for Windows, based on `signal`/`raise` in the CRT.
//! There are differences in both API and behavior:
//!
//! - Many parts of the library are not available there.
//! - We have only a few signals: `SIGABRT`, `SIGABRT_COMPAT`, `SIGBREAK`,
//!   `SIGFPE`, `SIGILL`, `SIGINT`, `SIGSEGV` and `SIGTERM`.
//! - Due to lack of signal blocking, there's a race condition.
//!   After the call to `signal`, there's a moment where we miss a signal.
//!   That means when you register a handler, there may be a signal which invokes
//!   neither the default handler or the handler you register.
//! - Handlers registered by `signal` in Windows are cleared on first signal.
//!   To match behavior in other platforms, we re-register the handler each time the handler is
//!   called, but there's a moment where we miss a handler.
//!   That means when you receive two signals in a row, there may be a signal which invokes
//!   the default handler, nevertheless you certainly have registered the handler.
//!
//! Moreover, signals won't work as you expected. `SIGTERM` isn't actually used and
//! not all `Ctrl-C`s are turned into `SIGINT`.
//!
//! Patches to improve Windows support in this library are welcome.
//!
//! # Features
//!
//! There are several feature flags that control how much is available as part of the crate, some
//! enabled by default.
//!
//! * `channel`: (enabled by default) The [Channel][crate::low_level::channel] synchronization
//!   primitive for exporting data out of signal handlers.
//! * `iterator`: (enabled by default) An [Signals iterator][crate::iterator::Signals] that
//!   provides a convenient interface for receiving signals in rust-friendly way.
//! * `extended-siginfo` adds support for providing extra information as part of the iterator
//!   interface.
//!
//! # Examples
//!
//! ## Using a flag to terminate a loop-based application
//!
//! ```rust
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! fn main() -> Result<(), Error> {
//!     let term = Arc::new(AtomicBool::new(false));
//!     signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
//!     while !term.load(Ordering::Relaxed) {
//!         // Do some time-limited stuff here
//!         // (if this could block forever, then there's no guarantee the signal will have any
//!         // effect).
//! #
//! #       // Hack to terminate the example, not part of the real code.
//! #       term.store(true, Ordering::Relaxed);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## A complex signal handling with a background thread
//!
//! This also handles the double CTRL+C situation (eg. the second CTRL+C kills) and resetting the
//! terminal on `SIGTSTP` (CTRL+Z, curses-based applications should do something like this).
//!
//! ```rust
//! # #[cfg(feature = "extended-siginfo")] pub mod test {
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicBool;
//!
//! use signal_hook::consts::signal::*;
//! use signal_hook::consts::TERM_SIGNALS;
//! use signal_hook::flag;
//! // A friend of the Signals iterator, but can be customized by what we want yielded about each
//! // signal.
//! use signal_hook::iterator::SignalsInfo;
//! use signal_hook::iterator::exfiltrator::WithOrigin;
//! use signal_hook::low_level;
//!
//! # struct App;
//! # impl App {
//! # fn run_background() -> Self { Self }
//! # fn wait_for_stop(self) {}
//! # fn restore_term(&self) {}
//! # fn claim_term(&self) {}
//! # fn resize_term(&self) {}
//! # fn reload_config(&self) {}
//! # fn print_stats(&self) {}
//! # }
//! # pub
//! fn main() -> Result<(), Error> {
//!     // Make sure double CTRL+C and similar kills
//!     let term_now = Arc::new(AtomicBool::new(false));
//!     for sig in TERM_SIGNALS {
//!         // When terminated by a second term signal, exit with exit code 1.
//!         // This will do nothing the first time (because term_now is false).
//!         flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
//!         // But this will "arm" the above for the second time, by setting it to true.
//!         // The order of registering these is important, if you put this one first, it will
//!         // first arm and then terminate ‒ all in the first round.
//!         flag::register(*sig, Arc::clone(&term_now))?;
//!     }
//!
//!     // Subscribe to all these signals with information about where they come from. We use the
//!     // extra info only for logging in this example (it is not available on all the OSes or at
//!     // all the occasions anyway, it may return `Unknown`).
//!     let mut sigs = vec![
//!         // Some terminal handling
//!         SIGTSTP, SIGCONT, SIGWINCH,
//!         // Reload of configuration for daemons ‒ um, is this example for a TUI app or a daemon
//!         // O:-)? You choose...
//!         SIGHUP,
//!         // Application-specific action, to print some statistics.
//!         SIGUSR1,
//!     ];
//!     sigs.extend(TERM_SIGNALS);
//!     let mut signals = SignalsInfo::<WithOrigin>::new(&sigs)?;
//! #   low_level::raise(SIGTERM)?; // Trick to terminate the example
//!
//!     // This is the actual application that'll start in its own thread. We'll control it from
//!     // this thread based on the signals, but it keeps running.
//!     // This is called after all the signals got registered, to avoid the short race condition
//!     // in the first registration of each signal in multi-threaded programs.
//!     let app = App::run_background();
//!
//!     // Consume all the incoming signals. This happens in "normal" Rust thread, not in the
//!     // signal handlers. This means that we are allowed to do whatever we like in here, without
//!     // restrictions, but it also means the kernel believes the signal already got delivered, we
//!     // handle them in delayed manner. This is in contrast with eg the above
//!     // `register_conditional_shutdown` where the shutdown happens *inside* the handler.
//!     let mut has_terminal = true;
//!     for info in &mut signals {
//!         // Will print info about signal + where it comes from.
//!         eprintln!("Received a signal {:?}", info);
//!         match info.signal {
//!             SIGTSTP => {
//!                 // Restore the terminal to non-TUI mode
//!                 if has_terminal {
//!                     app.restore_term();
//!                     has_terminal = false;
//!                     // And actually stop ourselves.
//!                     low_level::emulate_default_handler(SIGTSTP)?;
//!                 }
//!             }
//!             SIGCONT => {
//!                 if !has_terminal {
//!                     app.claim_term();
//!                     has_terminal = true;
//!                 }
//!             }
//!             SIGWINCH => app.resize_term(),
//!             SIGHUP => app.reload_config(),
//!             SIGUSR1 => app.print_stats(),
//!             term_sig => { // These are all the ones left
//!                 eprintln!("Terminating");
//!                 assert!(TERM_SIGNALS.contains(&term_sig));
//!                 break;
//!             }
//!         }
//!     }
//!
//!     // If during this another termination signal comes, the trick at the top would kick in and
//!     // terminate early. But if it doesn't, the application shuts down gracefully.
//!     app.wait_for_stop();
//!
//!     Ok(())
//! }
//! # }
//! # fn main() {
//! # #[cfg(feature = "extended-siginfo")] test::main().unwrap();
//! # }
//! ```
//!
//! # Asynchronous runtime support
//!
//! If you are looking for integration with an asynchronous runtime take a look at one of the
//! following adapter crates:
//!
//! * [`signal-hook-async-std`](https://docs.rs/signal-hook-async-std) for async-std support
//! * [`signal-hook-mio`](https://docs.rs/signal-hook-mio) for MIO support
//! * [`signal-hook-tokio`](https://docs.rs/signal-hook-tokio) for Tokio support
//!
//! Feel free to open a pull requests if you want to add support for runtimes not mentioned above.
//!
//! # Porting from previous versions
//!
//! There were some noisy changes when going from 0.2 version to the 0.3 version. In particular:
//!
//! * A lot of things moved around to make the structure of the crate a bit more understandable.
//!   Most of the time it should be possible to just search the documentation for the name that
//!   can't be resolved to discover the new location.
//!   - The signal constants (`SIGTERM`, for example) are in [`consts`] submodule (individual
//!     imports) and in the [`consts::signal`] (for wildcard import of all of them).
//!   - Some APIs that are considered more of a low-level building blocks than for casual day to
//!     day use are now in the [`low_level`] submodule.
//! * The previous version contained the `cleanup` module that allowed for removal of the actions
//!   in rather destructive way (nuking actions of arbitrary other parts of the program). This is
//!   completely gone in this version. The use case of shutting down the application on second
//!   CTRL+C is now supported by a pattern described in the [`flag`] submodule. For other similar
//!   needs, refer above for emulating default handlers.

pub mod flag;
#[cfg(all(not(windows), feature = "iterator"))]
#[cfg_attr(docsrs, doc(cfg(all(not(windows), feature = "iterator"))))]
pub mod iterator;
pub mod low_level;

/// The low-level constants.
///
/// Like the signal numbers.
pub mod consts {

    use libc::c_int;

    /// The signal constants.
    ///
    /// Can be mass-imported by `use signal_hook::consts::signal::*`, without polluting the
    /// namespace with other names. Also available in the [`consts`][crate::consts] directly (but
    /// with more constants around).
    pub mod signal {
        #[cfg(not(windows))]
        pub use libc::{
            SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT, SIGFPE, SIGHUP, SIGILL, SIGINT, SIGKILL,
            SIGPIPE, SIGPROF, SIGQUIT, SIGSEGV, SIGSTOP, SIGSYS, SIGTERM, SIGTRAP, SIGTSTP,
            SIGTTIN, SIGTTOU, SIGURG, SIGUSR1, SIGUSR2, SIGVTALRM, SIGWINCH, SIGXCPU, SIGXFSZ,
        };

        #[cfg(not(any(windows, target_os = "haiku")))]
        pub use libc::SIGIO;

        #[cfg(any(
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        pub use libc::SIGINFO;

        #[cfg(windows)]
        pub use libc::{SIGABRT, SIGFPE, SIGILL, SIGINT, SIGSEGV, SIGTERM};

        // NOTE: they perhaps deserve backport to libc.
        #[cfg(windows)]
        /// Same as `SIGABRT`, but the number is compatible to other platforms.
        pub const SIGABRT_COMPAT: libc::c_int = 6;
        #[cfg(windows)]
        /// Ctrl-Break is pressed for Windows Console processes.
        pub const SIGBREAK: libc::c_int = 21;
    }

    pub use self::signal::*;

    pub use signal_hook_registry::FORBIDDEN;

    /// Various signals commonly requesting shutdown of an application.
    #[cfg(not(windows))]
    pub const TERM_SIGNALS: &[c_int] = &[SIGTERM, SIGQUIT, SIGINT];

    /// Various signals commonly requesting shutdown of an application.
    #[cfg(windows)]
    pub const TERM_SIGNALS: &[c_int] = &[SIGTERM, SIGINT];
}

pub use signal_hook_registry::SigId;
