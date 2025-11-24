#![doc(test(attr(deny(warnings))))]
#![warn(missing_docs)]
#![allow(unknown_lints, renamed_and_remove_lints, bare_trait_objects)]

//! Backend of the [signal-hook] crate.
//!
//! The [signal-hook] crate tries to provide an API to the unix signals, which are a global
//! resource. Therefore, it is desirable an application contains just one version of the crate
//! which manages this global resource. But that makes it impossible to make breaking changes in
//! the API.
//!
//! Therefore, this crate provides very minimal and low level API to the signals that is unlikely
//! to have to change, while there may be multiple versions of the [signal-hook] that all use this
//! low-level API to provide different versions of the high level APIs.
//!
//! It is also possible some other crates might want to build a completely different API. This
//! split allows these crates to still reuse the same low-level routines in this crate instead of
//! going to the (much more dangerous) unix calls.
//!
//! # What this crate provides
//!
//! The only thing this crate does is multiplexing the signals. An application or library can add
//! or remove callbacks and have multiple callbacks for the same signal.
//!
//! It handles dispatching the callbacks and managing them in a way that uses only the
//! [async-signal-safe] functions inside the signal handler. Note that the callbacks are still run
//! inside the signal handler, so it is up to the caller to ensure they are also
//! [async-signal-safe].
//!
//! # What this is for
//!
//! This is a building block for other libraries creating reasonable abstractions on top of
//! signals. The [signal-hook] is the generally preferred way if you need to handle signals in your
//! application and provides several safe patterns of doing so.
//!
//! # Rust version compatibility
//!
//! Currently builds on 1.26.0 an newer and this is very unlikely to change. However, tests
//! require dependencies that don't build there, so tests need newer Rust version (they are run on
//! stable).
//!
//! Note that this ancient version of rustc no longer compiles current versions of `libc`. If you
//! want to use rustc this old, you need to force your dependency resolution to pick old enough
//! version of `libc` (`0.2.156` was found to work, but newer ones may too).
//!
//! # Portability
//!
//! This crate includes a limited support for Windows, based on `signal`/`raise` in the CRT.
//! There are differences in both API and behavior:
//!
//! - Due to lack of `siginfo_t`, we don't provide `register_sigaction` or `register_unchecked`.
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
//! [signal-hook]: https://docs.rs/signal-hook
//! [async-signal-safe]: http://www.man7.org/linux/man-pages/man7/signal-safety.7.html

extern crate libc;

mod half_lock;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::io::Error;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
// Once::new is now a const-fn. But it is not stable in all the rustc versions we want to support
// yet.
#[allow(deprecated)]
use std::sync::ONCE_INIT;
use std::sync::{Arc, Once};

#[cfg(not(windows))]
use libc::{c_int, c_void, sigaction, siginfo_t};
#[cfg(windows)]
use libc::{c_int, sighandler_t};

#[cfg(not(windows))]
use libc::{SIGFPE, SIGILL, SIGKILL, SIGSEGV, SIGSTOP};
#[cfg(windows)]
use libc::{SIGFPE, SIGILL, SIGSEGV};

use half_lock::HalfLock;

// These constants are not defined in the current version of libc, but it actually
// exists in Windows CRT.
#[cfg(windows)]
const SIG_DFL: sighandler_t = 0;
#[cfg(windows)]
const SIG_IGN: sighandler_t = 1;
#[cfg(windows)]
const SIG_GET: sighandler_t = 2;
#[cfg(windows)]
const SIG_ERR: sighandler_t = !0;

// To simplify implementation. Not to be exposed.
#[cfg(windows)]
#[allow(non_camel_case_types)]
struct siginfo_t;

// # Internal workings
//
// This uses a form of RCU. There's an atomic pointer to the current action descriptors (in the
// form of IndependentArcSwap, to be able to track what, if any, signal handlers still use the
// version). A signal handler takes a copy of the pointer and calls all the relevant actions.
//
// Modifications to that are protected by a mutex, to avoid juggling multiple signal handlers at
// once (eg. not calling sigaction concurrently). This should not be a problem, because modifying
// the signal actions should be initialization only anyway. To avoid all allocations and also
// deallocations inside the signal handler, after replacing the pointer, the modification routine
// needs to busy-wait for the reference count on the old pointer to drop to 1 and take ownership ‒
// that way the one deallocating is the modification routine, outside of the signal handler.

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct ActionId(u128);

/// An ID of registered action.
///
/// This is returned by all the registration routines and can be used to remove the action later on
/// with a call to [`unregister`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SigId {
    signal: c_int,
    action: ActionId,
}

// This should be dyn Fn(...), but we want to support Rust 1.26.0 and that one doesn't allow dyn
// yet.
#[allow(unknown_lints, bare_trait_objects)]
type Action = Fn(&siginfo_t) + Send + Sync;

#[derive(Clone)]
struct Slot {
    prev: Prev,
    // We use BTreeMap here, because we want to run the actions in the order they were inserted.
    // This works, because the ActionIds are assigned in an increasing order.
    actions: BTreeMap<ActionId, Arc<Action>>,
}

impl Slot {
    #[cfg(windows)]
    fn new(signal: libc::c_int) -> Result<Self, Error> {
        let old = unsafe { libc::signal(signal, handler as sighandler_t) };
        if old == SIG_ERR {
            return Err(Error::last_os_error());
        }
        Ok(Slot {
            prev: Prev { signal, info: old },
            actions: BTreeMap::new(),
        })
    }

    #[cfg(not(windows))]
    fn new(signal: libc::c_int) -> Result<Self, Error> {
        // C data structure, expected to be zeroed out.
        let mut new: libc::sigaction = unsafe { mem::zeroed() };

        // Note: AIX fixed their naming in libc 0.2.171.
        //
        // However, if we mandate that _for everyone_, other systems fail to compile on old Rust
        // versions (eg. 1.26.0), because they are no longer able to compile this new libc.
        //
        // There doesn't seem to be a way to make Cargo force the dependency for only one target
        // (it doesn't compile the ones it doesn't need, but it stills considers the other targets
        // for version resolution).
        //
        // Therefore, we let the user have freedom - if they want AIX, they can upgrade to new
        // enough libc. If they want ancient rustc, they can force older versions of libc.
        //
        // See #169.

        new.sa_sigaction = handler as usize; // If it doesn't compile on AIX, upgrade the libc dependency

        // Android is broken and uses different int types than the rest (and different depending on
        // the pointer width). This converts the flags to the proper type no matter what it is on
        // the given platform.
        #[cfg(target_os = "nto")]
        let flags = 0;
        // SA_RESTART is supported by qnx https://www.qnx.com/support/knowledgebase.html?id=50130000000SmiD
        #[cfg(not(target_os = "nto"))]
        let flags = libc::SA_RESTART;
        #[allow(unused_assignments)]
        let mut siginfo = flags;
        siginfo = libc::SA_SIGINFO as _;
        let flags = flags | siginfo;
        new.sa_flags = flags as _;
        // C data structure, expected to be zeroed out.
        let mut old: libc::sigaction = unsafe { mem::zeroed() };
        // FFI ‒ pointers are valid, it doesn't take ownership.
        if unsafe { libc::sigaction(signal, &new, &mut old) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(Slot {
            prev: Prev { signal, info: old },
            actions: BTreeMap::new(),
        })
    }
}

#[derive(Clone)]
struct SignalData {
    signals: HashMap<c_int, Slot>,
    next_id: u128,
}

#[derive(Clone)]
struct Prev {
    signal: c_int,
    #[cfg(windows)]
    info: sighandler_t,
    #[cfg(not(windows))]
    info: sigaction,
}

impl Prev {
    #[cfg(windows)]
    fn detect(signal: c_int) -> Result<Self, Error> {
        let old = unsafe { libc::signal(signal, SIG_GET) };
        if old == SIG_ERR {
            return Err(Error::last_os_error());
        }
        Ok(Prev { signal, info: old })
    }

    #[cfg(not(windows))]
    fn detect(signal: c_int) -> Result<Self, Error> {
        // C data structure, expected to be zeroed out.
        let mut old: libc::sigaction = unsafe { mem::zeroed() };
        // FFI ‒ pointers are valid, it doesn't take ownership.
        if unsafe { libc::sigaction(signal, ptr::null(), &mut old) } != 0 {
            return Err(Error::last_os_error());
        }

        Ok(Prev { signal, info: old })
    }

    #[cfg(windows)]
    fn execute(&self, sig: c_int) {
        let fptr = self.info;
        if fptr != 0 && fptr != SIG_DFL && fptr != SIG_IGN {
            // FFI ‒ calling the original signal handler.
            unsafe {
                let action = mem::transmute::<usize, extern "C" fn(c_int)>(fptr);
                action(sig);
            }
        }
    }

    #[cfg(not(windows))]
    unsafe fn execute(&self, sig: c_int, info: *mut siginfo_t, data: *mut c_void) {
        let fptr = self.info.sa_sigaction;
        if fptr != 0 && fptr != libc::SIG_DFL && fptr != libc::SIG_IGN {
            // Android is broken and uses different int types than the rest (and different
            // depending on the pointer width). This converts the flags to the proper type no
            // matter what it is on the given platform.
            //
            // The trick is to create the same-typed variable as the sa_flags first and then
            // set it to the proper value (does Rust have a way to copy a type in a different
            // way?)
            #[allow(unused_assignments)]
            let mut siginfo = self.info.sa_flags;
            siginfo = libc::SA_SIGINFO as _;
            if self.info.sa_flags & siginfo == 0 {
                let action = mem::transmute::<usize, extern "C" fn(c_int)>(fptr);
                action(sig);
            } else {
                type SigAction = extern "C" fn(c_int, *mut siginfo_t, *mut c_void);
                let action = mem::transmute::<usize, SigAction>(fptr);
                action(sig, info, data);
            }
        }
    }
}

/// Lazy-initiated data structure with our global variables.
///
/// Used inside a structure to cut down on boilerplate code to lazy-initialize stuff. We don't dare
/// use anything fancy like lazy-static or once-cell, since we are not sure they are
/// async-signal-safe in their access. Our code uses the [Once], but only on the write end outside
/// of signal handler. The handler assumes it has already been initialized.
struct GlobalData {
    /// The data structure describing what needs to be run for each signal.
    data: HalfLock<SignalData>,

    /// A fallback to fight/minimize a race condition during signal initialization.
    ///
    /// See the comment inside [`register_unchecked_impl`].
    race_fallback: HalfLock<Option<Prev>>,
}

static GLOBAL_DATA: AtomicPtr<GlobalData> = AtomicPtr::new(ptr::null_mut());
#[allow(deprecated)]
static GLOBAL_INIT: Once = ONCE_INIT;

impl GlobalData {
    fn get() -> &'static Self {
        let data = GLOBAL_DATA.load(Ordering::Acquire);
        // # Safety
        //
        // * The data actually does live forever - created by Box::into_raw.
        // * It is _never_ modified (apart for interior mutability, but that one is fine).
        unsafe { data.as_ref().expect("We shall be set up already") }
    }
    fn ensure() -> &'static Self {
        GLOBAL_INIT.call_once(|| {
            let data = Box::into_raw(Box::new(GlobalData {
                data: HalfLock::new(SignalData {
                    signals: HashMap::new(),
                    next_id: 1,
                }),
                race_fallback: HalfLock::new(None),
            }));
            let old = GLOBAL_DATA.swap(data, Ordering::Release);
            assert!(old.is_null());
        });
        Self::get()
    }
}

#[cfg(windows)]
extern "C" fn handler(sig: c_int) {
    if sig != SIGFPE {
        // Windows CRT `signal` resets handler every time, unless for SIGFPE.
        // Reregister the handler to retain maximal compatibility.
        // Problems:
        // - It's racy. But this is inevitably racy in Windows.
        // - Interacts poorly with handlers outside signal-hook-registry.
        let old = unsafe { libc::signal(sig, handler as sighandler_t) };
        if old == SIG_ERR {
            // MSDN doesn't describe which errors might occur,
            // but we can tell from the Linux manpage that
            // EINVAL (invalid signal number) is mostly the only case.
            // Therefore, this branch must not occur.
            // In any case we can do nothing useful in the signal handler,
            // so we're going to abort silently.
            unsafe {
                libc::abort();
            }
        }
    }

    let globals = GlobalData::get();
    let fallback = globals.race_fallback.read();
    let sigdata = globals.data.read();

    if let Some(ref slot) = sigdata.signals.get(&sig) {
        slot.prev.execute(sig);

        for action in slot.actions.values() {
            action(&siginfo_t);
        }
    } else if let Some(prev) = fallback.as_ref() {
        // In case we get called but don't have the slot for this signal set up yet, we are under
        // the race condition. We may have the old signal handler stored in the fallback
        // temporarily.
        if sig == prev.signal {
            prev.execute(sig);
        }
        // else -> probably should not happen, but races with other threads are possible so
        // better safe
    }
}

#[cfg(not(windows))]
extern "C" fn handler(sig: c_int, info: *mut siginfo_t, data: *mut c_void) {
    let globals = GlobalData::get();
    let fallback = globals.race_fallback.read();
    let sigdata = globals.data.read();

    if let Some(slot) = sigdata.signals.get(&sig) {
        unsafe { slot.prev.execute(sig, info, data) };

        let info = unsafe { info.as_ref() };
        let info = info.unwrap_or_else(|| {
            // The info being null seems to be illegal according to POSIX, but has been observed on
            // some probably broken platform. We can't do anything about that, that is just broken,
            // but we are not allowed to panic in a signal handler, so we are left only with simply
            // aborting. We try to write a message what happens, but using the libc stuff
            // (`eprintln` is not guaranteed to be async-signal-safe).
            unsafe {
                const MSG: &[u8] =
                    b"Platform broken, got NULL as siginfo to signal handler. Aborting";
                libc::write(2, MSG.as_ptr() as *const _, MSG.len());
                libc::abort();
            }
        });

        for action in slot.actions.values() {
            action(info);
        }
    } else if let Some(prev) = fallback.as_ref() {
        // In case we get called but don't have the slot for this signal set up yet, we are under
        // the race condition. We may have the old signal handler stored in the fallback
        // temporarily.
        if prev.signal == sig {
            unsafe { prev.execute(sig, info, data) };
        }
        // else -> probably should not happen, but races with other threads are possible so
        // better safe
    }
}

/// List of forbidden signals.
///
/// Some signals are impossible to replace according to POSIX and some are so special that this
/// library refuses to handle them (eg. SIGSEGV). The routines panic in case registering one of
/// these signals is attempted.
///
/// See [`register`].
pub const FORBIDDEN: &[c_int] = FORBIDDEN_IMPL;

#[cfg(windows)]
const FORBIDDEN_IMPL: &[c_int] = &[SIGILL, SIGFPE, SIGSEGV];
#[cfg(not(windows))]
const FORBIDDEN_IMPL: &[c_int] = &[SIGKILL, SIGSTOP, SIGILL, SIGFPE, SIGSEGV];

/// Registers an arbitrary action for the given signal.
///
/// This makes sure there's a signal handler for the given signal. It then adds the action to the
/// ones called each time the signal is delivered. If multiple actions are set for the same signal,
/// all are called, in the order of registration.
///
/// If there was a previous signal handler for the given signal, it is chained ‒ it will be called
/// as part of this library's signal handler, before any actions set through this function.
///
/// On success, the function returns an ID that can be used to remove the action again with
/// [`unregister`].
///
/// # Panics
///
/// If the signal is one of (see [`FORBIDDEN`]):
///
/// * `SIGKILL`
/// * `SIGSTOP`
/// * `SIGILL`
/// * `SIGFPE`
/// * `SIGSEGV`
///
/// The first two are not possible to override (and the underlying C functions simply ignore all
/// requests to do so, which smells of possible bugs, or return errors). The rest can be set, but
/// generally needs very special handling to do so correctly (direct manipulation of the
/// application's address space, `longjmp` and similar). Unless you know very well what you're
/// doing, you'll shoot yourself into the foot and this library won't help you with that.
///
/// # Errors
///
/// Since the library manipulates signals using the low-level C functions, all these can return
/// errors. Generally, the errors mean something like the specified signal does not exist on the
/// given platform ‒ after a program is debugged and tested on a given OS, it should never return
/// an error.
///
/// However, if an error *is* returned, there are no guarantees if the given action was registered
/// or not.
///
/// # Safety
///
/// This function is unsafe, because the `action` is run inside a signal handler. The set of
/// functions allowed to be called from within is very limited (they are called async-signal-safe
/// functions by POSIX). These specifically do *not* contain mutexes and memory
/// allocation/deallocation. They *do* contain routines to terminate the program, to further
/// manipulate signals (by the low-level functions, not by this library) and to read and write file
/// descriptors. Calling program's own functions consisting only of these is OK, as is manipulating
/// program's variables ‒ however, as the action can be called on any thread that does not have the
/// given signal masked (by default no signal is masked on any thread), and mutexes are a no-go,
/// this is harder than it looks like at first.
///
/// As panicking from within a signal handler would be a panic across FFI boundary (which is
/// undefined behavior), the passed handler must not panic.
///
/// If you find these limitations hard to satisfy, choose from the helper functions in the
/// [signal-hook](https://docs.rs/signal-hook) crate ‒ these provide safe interface to use some
/// common signal handling patters.
///
/// # Race condition
///
/// Upon registering the first hook for a given signal into this library, there's a short race
/// condition under the following circumstances:
///
/// * The program already has a signal handler installed for this particular signal (through some
///   other library, possibly).
/// * Concurrently, some other thread installs a different signal handler while it is being
///   installed by this library.
/// * At the same time, the signal is delivered.
///
/// Under such conditions signal-hook might wrongly "chain" to the older signal handler for a short
/// while (until the registration is fully complete).
///
/// Note that the exact conditions of the race condition might change in future versions of the
/// library. The recommended way to avoid it is to register signals before starting any additional
/// threads, or at least not to register signals concurrently.
///
/// Alternatively, make sure all signals are handled through this library.
///
/// # Performance
///
/// Even when it is possible to repeatedly install and remove actions during the lifetime of a
/// program, the installation and removal is considered a slow operation and should not be done
/// very often. Also, there's limited (though huge) amount of distinct IDs (they are `u128`).
///
/// # Examples
///
/// ```rust
/// extern crate signal_hook_registry;
///
/// use std::io::Error;
/// use std::process;
///
/// fn main() -> Result<(), Error> {
///     let signal = unsafe {
///         signal_hook_registry::register(signal_hook::consts::SIGTERM, || process::abort())
///     }?;
///     // Stuff here...
///     signal_hook_registry::unregister(signal); // Not really necessary.
///     Ok(())
/// }
/// ```
pub unsafe fn register<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn() + Sync + Send + 'static,
{
    register_sigaction_impl(signal, move |_: &_| action())
}

/// Register a signal action.
///
/// This acts in the same way as [`register`], including the drawbacks, panics and performance
/// characteristics. The only difference is the provided action accepts a [`siginfo_t`] argument,
/// providing information about the received signal.
///
/// # Safety
///
/// See the details of [`register`].
#[cfg(not(windows))]
pub unsafe fn register_sigaction<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    register_sigaction_impl(signal, action)
}

unsafe fn register_sigaction_impl<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    assert!(
        !FORBIDDEN.contains(&signal),
        "Attempted to register forbidden signal {}",
        signal,
    );
    register_unchecked_impl(signal, action)
}

/// Register a signal action without checking for forbidden signals.
///
/// This acts in the same way as [`register_unchecked`], including the drawbacks, panics and
/// performance characteristics. The only difference is the provided action doesn't accept a
/// [`siginfo_t`] argument.
///
/// # Safety
///
/// See the details of [`register`].
pub unsafe fn register_signal_unchecked<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn() + Sync + Send + 'static,
{
    register_unchecked_impl(signal, move |_: &_| action())
}

/// Register a signal action without checking for forbidden signals.
///
/// This acts the same way as [`register_sigaction`], but without checking for the [`FORBIDDEN`]
/// signals. All the signals passed are registered and it is up to the caller to make some sense of
/// them.
///
/// Note that you really need to know what you're doing if you change eg. the `SIGSEGV` signal
/// handler. Generally, you don't want to do that. But unlike the other functions here, this
/// function still allows you to do it.
///
/// # Safety
///
/// See the details of [`register`].
#[cfg(not(windows))]
pub unsafe fn register_unchecked<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    register_unchecked_impl(signal, action)
}

unsafe fn register_unchecked_impl<F>(signal: c_int, action: F) -> Result<SigId, Error>
where
    F: Fn(&siginfo_t) + Sync + Send + 'static,
{
    let globals = GlobalData::ensure();
    let action = Arc::from(action);

    let mut lock = globals.data.write();

    let mut sigdata = SignalData::clone(&lock);
    let id = ActionId(sigdata.next_id);
    sigdata.next_id += 1;

    match sigdata.signals.entry(signal) {
        Entry::Occupied(mut occupied) => {
            assert!(occupied.get_mut().actions.insert(id, action).is_none());
        }
        Entry::Vacant(place) => {
            // While the sigaction/signal exchanges the old one atomically, we are not able to
            // atomically store it somewhere a signal handler could read it. That poses a race
            // condition where we could lose some signals delivered in between changing it and
            // storing it.
            //
            // Therefore we first store the old one in the fallback storage. The fallback only
            // covers the cases where the slot is not yet active and becomes "inert" after that,
            // even if not removed (it may get overwritten by some other signal, but for that the
            // mutex in globals.data must be unlocked here - and by that time we already stored the
            // slot.
            //
            // And yes, this still leaves a short race condition when some other thread could
            // replace the signal handler and we would be calling the outdated one for a short
            // time, until we install the slot.
            globals
                .race_fallback
                .write()
                .store(Some(Prev::detect(signal)?));

            let mut slot = Slot::new(signal)?;
            slot.actions.insert(id, action);
            place.insert(slot);
        }
    }

    lock.store(sigdata);

    Ok(SigId { signal, action: id })
}

/// Removes a previously installed action.
///
/// This function does nothing if the action was already removed. It returns true if it was removed
/// and false if the action wasn't found.
///
/// It can unregister all the actions installed by [`register`] as well as the ones from downstream
/// crates (like [`signal-hook`](https://docs.rs/signal-hook)).
///
/// # Warning
///
/// This does *not* currently return the default/previous signal handler if the last action for a
/// signal was just unregistered. That means that if you replaced for example `SIGTERM` and then
/// removed the action, the program will effectively ignore `SIGTERM` signals from now on, not
/// terminate on them as is the default action. This is OK if you remove it as part of a shutdown,
/// but it is not recommended to remove termination actions during the normal runtime of
/// application (unless the desired effect is to create something that can be terminated only by
/// SIGKILL).
pub fn unregister(id: SigId) -> bool {
    let globals = GlobalData::ensure();
    let mut replace = false;
    let mut lock = globals.data.write();
    let mut sigdata = SignalData::clone(&lock);
    if let Some(slot) = sigdata.signals.get_mut(&id.signal) {
        replace = slot.actions.remove(&id.action).is_some();
    }
    if replace {
        lock.store(sigdata);
    }
    replace
}

// We keep this one here for strict backwards compatibility, but the API is kind of bad. One can
// delete actions that don't belong to them, which is kind of against the whole idea of not
// breaking stuff for others.
#[deprecated(
    since = "1.3.0",
    note = "Don't use. Can influence unrelated parts of program / unknown actions"
)]
#[doc(hidden)]
pub fn unregister_signal(signal: c_int) -> bool {
    let globals = GlobalData::ensure();
    let mut replace = false;
    let mut lock = globals.data.write();
    let mut sigdata = SignalData::clone(&lock);
    if let Some(slot) = sigdata.signals.get_mut(&signal) {
        if !slot.actions.is_empty() {
            slot.actions.clear();
            replace = true;
        }
    }
    if replace {
        lock.store(sigdata);
    }
    replace
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[cfg(not(windows))]
    use libc::{pid_t, SIGUSR1, SIGUSR2};

    #[cfg(windows)]
    use libc::SIGTERM as SIGUSR1;
    #[cfg(windows)]
    use libc::SIGTERM as SIGUSR2;

    use super::*;

    #[test]
    #[should_panic]
    fn panic_forbidden() {
        let _ = unsafe { register(SIGILL, || ()) };
    }

    /// Registering the forbidden signals is allowed in the _unchecked version.
    #[test]
    #[allow(clippy::redundant_closure)] // Clippy, you're wrong. Because it changes the return value.
    fn forbidden_raw() {
        unsafe { register_signal_unchecked(SIGFPE, || std::process::abort()).unwrap() };
    }

    #[test]
    fn signal_without_pid() {
        let status = Arc::new(AtomicUsize::new(0));
        let action = {
            let status = Arc::clone(&status);
            move || {
                status.store(1, Ordering::Relaxed);
            }
        };
        unsafe {
            register(SIGUSR2, action).unwrap();
            libc::raise(SIGUSR2);
        }
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let current = status.load(Ordering::Relaxed);
            match current {
                // Not yet
                0 => continue,
                // Good, we are done with the correct result
                _ if current == 1 => return,
                _ => panic!("Wrong result value {}", current),
            }
        }
        panic!("Timed out waiting for the signal");
    }

    #[test]
    #[cfg(not(windows))]
    fn signal_with_pid() {
        let status = Arc::new(AtomicUsize::new(0));
        let action = {
            let status = Arc::clone(&status);
            move |siginfo: &siginfo_t| {
                // Hack: currently, libc exposes only the first 3 fields of siginfo_t. The pid
                // comes somewhat later on. Therefore, we do a Really Ugly Hack and define our
                // own structure (and hope it is correct on all platforms). But hey, this is
                // only the tests, so we are going to get away with this.
                #[repr(C)]
                struct SigInfo {
                    _fields: [c_int; 3],
                    #[cfg(all(target_pointer_width = "64", target_os = "linux"))]
                    _pad: c_int,
                    pid: pid_t,
                }
                let s: &SigInfo = unsafe {
                    (siginfo as *const _ as usize as *const SigInfo)
                        .as_ref()
                        .unwrap()
                };
                status.store(s.pid as usize, Ordering::Relaxed);
            }
        };
        let pid;
        unsafe {
            pid = libc::getpid();
            register_sigaction(SIGUSR2, action).unwrap();
            libc::raise(SIGUSR2);
        }
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let current = status.load(Ordering::Relaxed);
            match current {
                // Not yet (PID == 0 doesn't happen)
                0 => continue,
                // Good, we are done with the correct result
                _ if current == pid as usize => return,
                _ => panic!("Wrong status value {}", current),
            }
        }
        panic!("Timed out waiting for the signal");
    }

    /// Check that registration works as expected and that unregister tells if it did or not.
    #[test]
    fn register_unregister() {
        let signal = unsafe { register(SIGUSR1, || ()).unwrap() };
        // It was there now, so we can unregister
        assert!(unregister(signal));
        // The next time unregistering does nothing and tells us so.
        assert!(!unregister(signal));
    }
}
