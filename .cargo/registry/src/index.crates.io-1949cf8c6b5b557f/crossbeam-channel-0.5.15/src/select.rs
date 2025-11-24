//! Interface to the select mechanism.

use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::time::{Duration, Instant};
use std::vec::Vec;

use crossbeam_utils::Backoff;

use crate::channel::{self, Receiver, Sender};
use crate::context::Context;
use crate::err::{ReadyTimeoutError, TryReadyError};
use crate::err::{RecvError, SendError};
use crate::err::{SelectTimeoutError, TrySelectError};
use crate::flavors;
use crate::utils;

/// Temporary data that gets initialized during select or a blocking operation, and is consumed by
/// `read` or `write`.
///
/// Each field contains data associated with a specific channel flavor.
// This is a private API that is used by the select macro.
#[derive(Debug, Default)]
pub struct Token {
    pub(crate) at: flavors::at::AtToken,
    pub(crate) array: flavors::array::ArrayToken,
    pub(crate) list: flavors::list::ListToken,
    #[allow(dead_code)]
    pub(crate) never: flavors::never::NeverToken,
    pub(crate) tick: flavors::tick::TickToken,
    pub(crate) zero: flavors::zero::ZeroToken,
}

/// Identifier associated with an operation by a specific thread on a specific channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation(usize);

impl Operation {
    /// Creates an operation identifier from a mutable reference.
    ///
    /// This function essentially just turns the address of the reference into a number. The
    /// reference should point to a variable that is specific to the thread and the operation,
    /// and is alive for the entire duration of select or blocking operation.
    #[inline]
    pub fn hook<T>(r: &mut T) -> Operation {
        let val = r as *mut T as usize;
        // Make sure that the pointer address doesn't equal the numerical representation of
        // `Selected::{Waiting, Aborted, Disconnected}`.
        assert!(val > 2);
        Operation(val)
    }
}

/// Current state of a select or a blocking operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    /// Still waiting for an operation.
    Waiting,

    /// The attempt to block the current thread has been aborted.
    Aborted,

    /// An operation became ready because a channel is disconnected.
    Disconnected,

    /// An operation became ready because a message can be sent or received.
    Operation(Operation),
}

impl From<usize> for Selected {
    #[inline]
    fn from(val: usize) -> Selected {
        match val {
            0 => Selected::Waiting,
            1 => Selected::Aborted,
            2 => Selected::Disconnected,
            oper => Selected::Operation(Operation(oper)),
        }
    }
}

impl Into<usize> for Selected {
    #[inline]
    fn into(self) -> usize {
        match self {
            Selected::Waiting => 0,
            Selected::Aborted => 1,
            Selected::Disconnected => 2,
            Selected::Operation(Operation(val)) => val,
        }
    }
}

/// A receiver or a sender that can participate in select.
///
/// This is a handle that assists select in executing an operation, registration, deciding on the
/// appropriate deadline for blocking, etc.
// This is a private API (exposed inside crossbeam_channel::internal module) that is used by the select macro.
pub trait SelectHandle {
    /// Attempts to select an operation and returns `true` on success.
    fn try_select(&self, token: &mut Token) -> bool;

    /// Returns a deadline for an operation, if there is one.
    fn deadline(&self) -> Option<Instant>;

    /// Registers an operation for execution and returns `true` if it is now ready.
    fn register(&self, oper: Operation, cx: &Context) -> bool;

    /// Unregisters an operation for execution.
    fn unregister(&self, oper: Operation);

    /// Attempts to select an operation the thread got woken up for and returns `true` on success.
    fn accept(&self, token: &mut Token, cx: &Context) -> bool;

    /// Returns `true` if an operation can be executed without blocking.
    fn is_ready(&self) -> bool;

    /// Registers an operation for readiness notification and returns `true` if it is now ready.
    fn watch(&self, oper: Operation, cx: &Context) -> bool;

    /// Unregisters an operation for readiness notification.
    fn unwatch(&self, oper: Operation);
}

impl<T: SelectHandle> SelectHandle for &T {
    fn try_select(&self, token: &mut Token) -> bool {
        (**self).try_select(token)
    }

    fn deadline(&self) -> Option<Instant> {
        (**self).deadline()
    }

    fn register(&self, oper: Operation, cx: &Context) -> bool {
        (**self).register(oper, cx)
    }

    fn unregister(&self, oper: Operation) {
        (**self).unregister(oper);
    }

    fn accept(&self, token: &mut Token, cx: &Context) -> bool {
        (**self).accept(token, cx)
    }

    fn is_ready(&self) -> bool {
        (**self).is_ready()
    }

    fn watch(&self, oper: Operation, cx: &Context) -> bool {
        (**self).watch(oper, cx)
    }

    fn unwatch(&self, oper: Operation) {
        (**self).unwatch(oper)
    }
}

/// Determines when a select operation should time out.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Timeout {
    /// No blocking.
    Now,

    /// Block forever.
    Never,

    /// Time out after the time instant.
    At(Instant),
}

/// Runs until one of the operations is selected, potentially blocking the current thread.
///
/// Successful receive operations will have to be followed up by `channel::read()` and successful
/// send operations by `channel::write()`.
fn run_select(
    handles: &mut [(&dyn SelectHandle, usize, *const u8)],
    timeout: Timeout,
    is_biased: bool,
) -> Option<(Token, usize, *const u8)> {
    if handles.is_empty() {
        // Wait until the timeout and return.
        match timeout {
            Timeout::Now => return None,
            Timeout::Never => {
                utils::sleep_until(None);
                unreachable!();
            }
            Timeout::At(when) => {
                utils::sleep_until(Some(when));
                return None;
            }
        }
    }

    if !is_biased {
        // Shuffle the operations for fairness.
        utils::shuffle(handles);
    }

    // Create a token, which serves as a temporary variable that gets initialized in this function
    // and is later used by a call to `channel::read()` or `channel::write()` that completes the
    // selected operation.
    let mut token = Token::default();

    // Try selecting one of the operations without blocking.
    for &(handle, i, ptr) in handles.iter() {
        if handle.try_select(&mut token) {
            return Some((token, i, ptr));
        }
    }

    loop {
        // Prepare for blocking.
        let res = Context::with(|cx| {
            let mut sel = Selected::Waiting;
            let mut registered_count = 0;
            let mut index_ready = None;

            if let Timeout::Now = timeout {
                cx.try_select(Selected::Aborted).unwrap();
            }

            // Register all operations.
            for (handle, i, _) in handles.iter_mut() {
                registered_count += 1;

                // If registration returns `false`, that means the operation has just become ready.
                if handle.register(Operation::hook::<&dyn SelectHandle>(handle), cx) {
                    // Try aborting select.
                    sel = match cx.try_select(Selected::Aborted) {
                        Ok(()) => {
                            index_ready = Some(*i);
                            Selected::Aborted
                        }
                        Err(s) => s,
                    };
                    break;
                }

                // If another thread has already selected one of the operations, stop registration.
                sel = cx.selected();
                if sel != Selected::Waiting {
                    break;
                }
            }

            if sel == Selected::Waiting {
                // Check with each operation for how long we're allowed to block, and compute the
                // earliest deadline.
                let mut deadline: Option<Instant> = match timeout {
                    Timeout::Now => return None,
                    Timeout::Never => None,
                    Timeout::At(when) => Some(when),
                };
                for &(handle, _, _) in handles.iter() {
                    if let Some(x) = handle.deadline() {
                        deadline = deadline.map(|y| x.min(y)).or(Some(x));
                    }
                }

                // Block the current thread.
                sel = cx.wait_until(deadline);
            }

            // Unregister all registered operations.
            for (handle, _, _) in handles.iter_mut().take(registered_count) {
                handle.unregister(Operation::hook::<&dyn SelectHandle>(handle));
            }

            match sel {
                Selected::Waiting => unreachable!(),
                Selected::Aborted => {
                    // If an operation became ready during registration, try selecting it.
                    if let Some(index_ready) = index_ready {
                        for &(handle, i, ptr) in handles.iter() {
                            if i == index_ready && handle.try_select(&mut token) {
                                return Some((i, ptr));
                            }
                        }
                    }
                }
                Selected::Disconnected => {}
                Selected::Operation(_) => {
                    // Find the selected operation.
                    for (handle, i, ptr) in handles.iter_mut() {
                        // Is this the selected operation?
                        if sel == Selected::Operation(Operation::hook::<&dyn SelectHandle>(handle))
                        {
                            // Try selecting this operation.
                            if handle.accept(&mut token, cx) {
                                return Some((*i, *ptr));
                            }
                        }
                    }
                }
            }

            None
        });

        // Return if an operation was selected.
        if let Some((i, ptr)) = res {
            return Some((token, i, ptr));
        }

        // Try selecting one of the operations without blocking.
        for &(handle, i, ptr) in handles.iter() {
            if handle.try_select(&mut token) {
                return Some((token, i, ptr));
            }
        }

        match timeout {
            Timeout::Now => return None,
            Timeout::Never => {}
            Timeout::At(when) => {
                if Instant::now() >= when {
                    return None;
                }
            }
        }
    }
}

/// Runs until one of the operations becomes ready, potentially blocking the current thread.
fn run_ready(
    handles: &mut [(&dyn SelectHandle, usize, *const u8)],
    timeout: Timeout,
    is_biased: bool,
) -> Option<usize> {
    if handles.is_empty() {
        // Wait until the timeout and return.
        match timeout {
            Timeout::Now => return None,
            Timeout::Never => {
                utils::sleep_until(None);
                unreachable!();
            }
            Timeout::At(when) => {
                utils::sleep_until(Some(when));
                return None;
            }
        }
    }

    if !is_biased {
        // Shuffle the operations for fairness.
        utils::shuffle(handles);
    }

    loop {
        let backoff = Backoff::new();
        loop {
            // Check operations for readiness.
            for &(handle, i, _) in handles.iter() {
                if handle.is_ready() {
                    return Some(i);
                }
            }

            if backoff.is_completed() {
                break;
            } else {
                backoff.snooze();
            }
        }

        // Check for timeout.
        match timeout {
            Timeout::Now => return None,
            Timeout::Never => {}
            Timeout::At(when) => {
                if Instant::now() >= when {
                    return None;
                }
            }
        }

        // Prepare for blocking.
        let res = Context::with(|cx| {
            let mut sel = Selected::Waiting;
            let mut registered_count = 0;

            // Begin watching all operations.
            for (handle, _, _) in handles.iter_mut() {
                registered_count += 1;
                let oper = Operation::hook::<&dyn SelectHandle>(handle);

                // If registration returns `false`, that means the operation has just become ready.
                if handle.watch(oper, cx) {
                    sel = match cx.try_select(Selected::Operation(oper)) {
                        Ok(()) => Selected::Operation(oper),
                        Err(s) => s,
                    };
                    break;
                }

                // If another thread has already chosen one of the operations, stop registration.
                sel = cx.selected();
                if sel != Selected::Waiting {
                    break;
                }
            }

            if sel == Selected::Waiting {
                // Check with each operation for how long we're allowed to block, and compute the
                // earliest deadline.
                let mut deadline: Option<Instant> = match timeout {
                    Timeout::Now => unreachable!(),
                    Timeout::Never => None,
                    Timeout::At(when) => Some(when),
                };
                for &(handle, _, _) in handles.iter() {
                    if let Some(x) = handle.deadline() {
                        deadline = deadline.map(|y| x.min(y)).or(Some(x));
                    }
                }

                // Block the current thread.
                sel = cx.wait_until(deadline);
            }

            // Unwatch all operations.
            for (handle, _, _) in handles.iter_mut().take(registered_count) {
                handle.unwatch(Operation::hook::<&dyn SelectHandle>(handle));
            }

            match sel {
                Selected::Waiting => unreachable!(),
                Selected::Aborted => {}
                Selected::Disconnected => {}
                Selected::Operation(_) => {
                    for (handle, i, _) in handles.iter_mut() {
                        let oper = Operation::hook::<&dyn SelectHandle>(handle);
                        if sel == Selected::Operation(oper) {
                            return Some(*i);
                        }
                    }
                }
            }

            None
        });

        // Return if an operation became ready.
        if res.is_some() {
            return res;
        }
    }
}

/// Attempts to select one of the operations without blocking.
// This is a private API (exposed inside crossbeam_channel::internal module) that is used by the select macro.
#[inline]
pub fn try_select<'a>(
    handles: &mut [(&'a dyn SelectHandle, usize, *const u8)],
    is_biased: bool,
) -> Result<SelectedOperation<'a>, TrySelectError> {
    match run_select(handles, Timeout::Now, is_biased) {
        None => Err(TrySelectError),
        Some((token, index, ptr)) => Ok(SelectedOperation {
            token,
            index,
            ptr,
            _marker: PhantomData,
        }),
    }
}

/// Blocks until one of the operations becomes ready and selects it.
// This is a private API (exposed inside crossbeam_channel::internal module) that is used by the select macro.
#[inline]
pub fn select<'a>(
    handles: &mut [(&'a dyn SelectHandle, usize, *const u8)],
    is_biased: bool,
) -> SelectedOperation<'a> {
    if handles.is_empty() {
        panic!("no operations have been added to `Select`");
    }

    let (token, index, ptr) = run_select(handles, Timeout::Never, is_biased).unwrap();
    SelectedOperation {
        token,
        index,
        ptr,
        _marker: PhantomData,
    }
}

/// Blocks for a limited time until one of the operations becomes ready and selects it.
// This is a private API (exposed inside crossbeam_channel::internal module) that is used by the select macro.
#[inline]
pub fn select_timeout<'a>(
    handles: &mut [(&'a dyn SelectHandle, usize, *const u8)],
    timeout: Duration,
    is_biased: bool,
) -> Result<SelectedOperation<'a>, SelectTimeoutError> {
    match Instant::now().checked_add(timeout) {
        Some(deadline) => select_deadline(handles, deadline, is_biased),
        None => Ok(select(handles, is_biased)),
    }
}

/// Blocks until a given deadline, or until one of the operations becomes ready and selects it.
#[inline]
pub(crate) fn select_deadline<'a>(
    handles: &mut [(&'a dyn SelectHandle, usize, *const u8)],
    deadline: Instant,
    is_biased: bool,
) -> Result<SelectedOperation<'a>, SelectTimeoutError> {
    match run_select(handles, Timeout::At(deadline), is_biased) {
        None => Err(SelectTimeoutError),
        Some((token, index, ptr)) => Ok(SelectedOperation {
            token,
            index,
            ptr,
            _marker: PhantomData,
        }),
    }
}

/// Selects from a set of channel operations.
///
/// `Select` allows you to define a set of channel operations, wait until any one of them becomes
/// ready, and finally execute it. If multiple operations are ready at the same time, a random one
/// among them is selected.
///
/// An operation is considered to be ready if it doesn't have to block. Note that it is ready even
/// when it will simply return an error because the channel is disconnected.
///
/// The [`select!`] macro is a convenience wrapper around `Select`. However, it cannot select over a
/// dynamically created list of channel operations.
///
/// [`select!`]: crate::select!
///
/// Once a list of operations has been built with `Select`, there are two different ways of
/// proceeding:
///
/// * Select an operation with [`try_select`], [`select`], or [`select_timeout`]. If successful,
///   the returned selected operation has already begun and **must** be completed. If we don't
///   complete it, a panic will occur.
///
/// * Wait for an operation to become ready with [`try_ready`], [`ready`], or [`ready_timeout`]. If
///   successful, we may attempt to execute the operation, but are not obliged to. In fact, it's
///   possible for another thread to make the operation not ready just before we try executing it,
///   so it's wise to use a retry loop. However, note that these methods might return with success
///   spuriously, so it's a good idea to always double check if the operation is really ready.
///
/// # Examples
///
/// Use [`select`] to receive a message from a list of receivers:
///
/// ```
/// use crossbeam_channel::{Receiver, RecvError, Select};
///
/// fn recv_multiple<T>(rs: &[Receiver<T>]) -> Result<T, RecvError> {
///     // Build a list of operations.
///     let mut sel = Select::new();
///     for r in rs {
///         sel.recv(r);
///     }
///
///     // Complete the selected operation.
///     let oper = sel.select();
///     let index = oper.index();
///     oper.recv(&rs[index])
/// }
/// ```
///
/// Use [`ready`] to receive a message from a list of receivers:
///
/// ```
/// use crossbeam_channel::{Receiver, RecvError, Select};
///
/// fn recv_multiple<T>(rs: &[Receiver<T>]) -> Result<T, RecvError> {
///     // Build a list of operations.
///     let mut sel = Select::new();
///     for r in rs {
///         sel.recv(r);
///     }
///
///     loop {
///         // Wait until a receive operation becomes ready and try executing it.
///         let index = sel.ready();
///         let res = rs[index].try_recv();
///
///         // If the operation turns out not to be ready, retry.
///         if let Err(e) = res {
///             if e.is_empty() {
///                 continue;
///             }
///         }
///
///         // Success!
///         return res.map_err(|_| RecvError);
///     }
/// }
/// ```
///
/// [`try_select`]: Select::try_select
/// [`select`]: Select::select
/// [`select_timeout`]: Select::select_timeout
/// [`try_ready`]: Select::try_ready
/// [`ready`]: Select::ready
/// [`ready_timeout`]: Select::ready_timeout
pub struct Select<'a> {
    /// A list of senders and receivers participating in selection.
    handles: Vec<(&'a dyn SelectHandle, usize, *const u8)>,

    /// The next index to assign to an operation.
    next_index: usize,

    /// Whether to use the index of handles as bias for selecting ready operations.
    biased: bool,
}

unsafe impl Send for Select<'_> {}
unsafe impl Sync for Select<'_> {}

impl<'a> Select<'a> {
    /// Creates an empty list of channel operations for selection.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::Select;
    ///
    /// let mut sel = Select::new();
    ///
    /// // The list of operations is empty, which means no operation can be selected.
    /// assert!(sel.try_select().is_err());
    /// ```
    pub fn new() -> Select<'a> {
        Select {
            handles: Vec::with_capacity(4),
            next_index: 0,
            biased: false,
        }
    }

    /// Creates an empty list of channel operations with biased selection.
    ///
    /// When multiple handles are ready, this will select the operation with the lowest index.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::Select;
    ///
    /// let mut sel = Select::new_biased();
    ///
    /// // The list of operations is empty, which means no operation can be selected.
    /// assert!(sel.try_select().is_err());
    /// ```
    pub fn new_biased() -> Self {
        Self {
            biased: true,
            ..Default::default()
        }
    }

    /// Adds a send operation.
    ///
    /// Returns the index of the added operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s, r) = unbounded::<i32>();
    ///
    /// let mut sel = Select::new();
    /// let index = sel.send(&s);
    /// ```
    pub fn send<T>(&mut self, s: &'a Sender<T>) -> usize {
        let i = self.next_index;
        let ptr = s as *const Sender<_> as *const u8;
        self.handles.push((s, i, ptr));
        self.next_index += 1;
        i
    }

    /// Adds a receive operation.
    ///
    /// Returns the index of the added operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s, r) = unbounded::<i32>();
    ///
    /// let mut sel = Select::new();
    /// let index = sel.recv(&r);
    /// ```
    pub fn recv<T>(&mut self, r: &'a Receiver<T>) -> usize {
        let i = self.next_index;
        let ptr = r as *const Receiver<_> as *const u8;
        self.handles.push((r, i, ptr));
        self.next_index += 1;
        i
    }

    /// Removes a previously added operation.
    ///
    /// This is useful when an operation is selected because the channel got disconnected and we
    /// want to try again to select a different operation instead.
    ///
    /// If new operations are added after removing some, the indices of removed operations will not
    /// be reused.
    ///
    /// # Panics
    ///
    /// An attempt to remove a non-existing or already removed operation will panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded::<i32>();
    /// let (_, r2) = unbounded::<i32>();
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // Both operations are initially ready, so a random one will be executed.
    /// let oper = sel.select();
    /// assert_eq!(oper.index(), oper2);
    /// assert!(oper.recv(&r2).is_err());
    /// sel.remove(oper2);
    ///
    /// s1.send(10).unwrap();
    ///
    /// let oper = sel.select();
    /// assert_eq!(oper.index(), oper1);
    /// assert_eq!(oper.recv(&r1), Ok(10));
    /// ```
    pub fn remove(&mut self, index: usize) {
        assert!(
            index < self.next_index,
            "index out of bounds; {} >= {}",
            index,
            self.next_index,
        );

        let i = self
            .handles
            .iter()
            .enumerate()
            .find(|(_, (_, i, _))| *i == index)
            .expect("no operation with this index")
            .0;

        self.handles.swap_remove(i);
    }

    /// Attempts to select one of the operations without blocking.
    ///
    /// If an operation is ready, it is selected and returned. If multiple operations are ready at
    /// the same time, a random one among them is selected. If none of the operations are ready, an
    /// error is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// The selected operation must be completed with [`SelectedOperation::send`]
    /// or [`SelectedOperation::recv`].
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// s1.send(10).unwrap();
    /// s2.send(20).unwrap();
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // Both operations are initially ready, so a random one will be executed.
    /// let oper = sel.try_select();
    /// match oper {
    ///     Err(_) => panic!("both operations should be ready"),
    ///     Ok(oper) => match oper.index() {
    ///         i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(10)),
    ///         i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(20)),
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// ```
    pub fn try_select(&mut self) -> Result<SelectedOperation<'a>, TrySelectError> {
        try_select(&mut self.handles, self.biased)
    }

    /// Blocks until one of the operations becomes ready and selects it.
    ///
    /// Once an operation becomes ready, it is selected and returned. If multiple operations are
    /// ready at the same time, a random one among them is selected.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// The selected operation must be completed with [`SelectedOperation::send`]
    /// or [`SelectedOperation::recv`].
    ///
    /// # Panics
    ///
    /// Panics if no operations have been added to `Select`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// let oper = sel.select();
    /// match oper.index() {
    ///     i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(10)),
    ///     i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(20)),
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn select(&mut self) -> SelectedOperation<'a> {
        select(&mut self.handles, self.biased)
    }

    /// Blocks for a limited time until one of the operations becomes ready and selects it.
    ///
    /// If an operation becomes ready, it is selected and returned. If multiple operations are
    /// ready at the same time, a random one among them is selected. If none of the operations
    /// become ready for the specified duration, an error is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// The selected operation must be completed with [`SelectedOperation::send`]
    /// or [`SelectedOperation::recv`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// let oper = sel.select_timeout(Duration::from_millis(500));
    /// match oper {
    ///     Err(_) => panic!("should not have timed out"),
    ///     Ok(oper) => match oper.index() {
    ///         i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(10)),
    ///         i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(20)),
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// ```
    pub fn select_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<SelectedOperation<'a>, SelectTimeoutError> {
        select_timeout(&mut self.handles, timeout, self.biased)
    }

    /// Blocks until a given deadline, or until one of the operations becomes ready and selects it.
    ///
    /// If an operation becomes ready, it is selected and returned. If multiple operations are
    /// ready at the same time, a random one among them is selected. If none of the operations
    /// become ready before the given deadline, an error is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// The selected operation must be completed with [`SelectedOperation::send`]
    /// or [`SelectedOperation::recv`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::{Instant, Duration};
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// let deadline = Instant::now() + Duration::from_millis(500);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// let oper = sel.select_deadline(deadline);
    /// match oper {
    ///     Err(_) => panic!("should not have timed out"),
    ///     Ok(oper) => match oper.index() {
    ///         i if i == oper1 => assert_eq!(oper.recv(&r1), Ok(10)),
    ///         i if i == oper2 => assert_eq!(oper.recv(&r2), Ok(20)),
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// ```
    pub fn select_deadline(
        &mut self,
        deadline: Instant,
    ) -> Result<SelectedOperation<'a>, SelectTimeoutError> {
        select_deadline(&mut self.handles, deadline, self.biased)
    }

    /// Attempts to find a ready operation without blocking.
    ///
    /// If an operation is ready, its index is returned. If multiple operations are ready at the
    /// same time, a random one among them is chosen. If none of the operations are ready, an error
    /// is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// Note that this method might return with success spuriously, so it's a good idea to always
    /// double check if the operation is really ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// s1.send(10).unwrap();
    /// s2.send(20).unwrap();
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // Both operations are initially ready, so a random one will be chosen.
    /// match sel.try_ready() {
    ///     Err(_) => panic!("both operations should be ready"),
    ///     Ok(i) if i == oper1 => assert_eq!(r1.try_recv(), Ok(10)),
    ///     Ok(i) if i == oper2 => assert_eq!(r2.try_recv(), Ok(20)),
    ///     Ok(_) => unreachable!(),
    /// }
    /// ```
    pub fn try_ready(&mut self) -> Result<usize, TryReadyError> {
        match run_ready(&mut self.handles, Timeout::Now, self.biased) {
            None => Err(TryReadyError),
            Some(index) => Ok(index),
        }
    }

    /// Blocks until one of the operations becomes ready.
    ///
    /// Once an operation becomes ready, its index is returned. If multiple operations are ready at
    /// the same time, a random one among them is chosen.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// Note that this method might return with success spuriously, so it's a good idea to always
    /// double check if the operation is really ready.
    ///
    /// # Panics
    ///
    /// Panics if no operations have been added to `Select`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// match sel.ready() {
    ///     i if i == oper1 => assert_eq!(r1.try_recv(), Ok(10)),
    ///     i if i == oper2 => assert_eq!(r2.try_recv(), Ok(20)),
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn ready(&mut self) -> usize {
        if self.handles.is_empty() {
            panic!("no operations have been added to `Select`");
        }

        run_ready(&mut self.handles, Timeout::Never, self.biased).unwrap()
    }

    /// Blocks for a limited time until one of the operations becomes ready.
    ///
    /// If an operation becomes ready, its index is returned. If multiple operations are ready at
    /// the same time, a random one among them is chosen. If none of the operations become ready
    /// for the specified duration, an error is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// Note that this method might return with success spuriously, so it's a good idea to double
    /// check if the operation is really ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// match sel.ready_timeout(Duration::from_millis(500)) {
    ///     Err(_) => panic!("should not have timed out"),
    ///     Ok(i) if i == oper1 => assert_eq!(r1.try_recv(), Ok(10)),
    ///     Ok(i) if i == oper2 => assert_eq!(r2.try_recv(), Ok(20)),
    ///     Ok(_) => unreachable!(),
    /// }
    /// ```
    pub fn ready_timeout(&mut self, timeout: Duration) -> Result<usize, ReadyTimeoutError> {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.ready_deadline(deadline),
            None => Ok(self.ready()),
        }
    }

    /// Blocks until a given deadline, or until one of the operations becomes ready.
    ///
    /// If an operation becomes ready, its index is returned. If multiple operations are ready at
    /// the same time, a random one among them is chosen. If none of the operations become ready
    /// before the deadline, an error is returned.
    ///
    /// An operation is considered to be ready if it doesn't have to block. Note that it is ready
    /// even when it will simply return an error because the channel is disconnected.
    ///
    /// Note that this method might return with success spuriously, so it's a good idea to double
    /// check if the operation is really ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use crossbeam_channel::{unbounded, Select};
    ///
    /// let deadline = Instant::now() + Duration::from_millis(500);
    ///
    /// let (s1, r1) = unbounded();
    /// let (s2, r2) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s1.send(10).unwrap();
    /// });
    /// thread::spawn(move || s2.send(20).unwrap());
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r1);
    /// let oper2 = sel.recv(&r2);
    ///
    /// // The second operation will be selected because it becomes ready first.
    /// match sel.ready_deadline(deadline) {
    ///     Err(_) => panic!("should not have timed out"),
    ///     Ok(i) if i == oper1 => assert_eq!(r1.try_recv(), Ok(10)),
    ///     Ok(i) if i == oper2 => assert_eq!(r2.try_recv(), Ok(20)),
    ///     Ok(_) => unreachable!(),
    /// }
    /// ```
    pub fn ready_deadline(&mut self, deadline: Instant) -> Result<usize, ReadyTimeoutError> {
        match run_ready(&mut self.handles, Timeout::At(deadline), self.biased) {
            None => Err(ReadyTimeoutError),
            Some(index) => Ok(index),
        }
    }
}

impl<'a> Clone for Select<'a> {
    fn clone(&self) -> Select<'a> {
        Select {
            handles: self.handles.clone(),
            next_index: self.next_index,
            biased: self.biased,
        }
    }
}

impl<'a> Default for Select<'a> {
    fn default() -> Select<'a> {
        Select::new()
    }
}

impl fmt::Debug for Select<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Select { .. }")
    }
}

/// A selected operation that needs to be completed.
///
/// To complete the operation, call [`send`] or [`recv`].
///
/// # Panics
///
/// Forgetting to complete the operation is an error and might lead to deadlocks. If a
/// `SelectedOperation` is dropped without completion, a panic occurs.
///
/// [`send`]: SelectedOperation::send
/// [`recv`]: SelectedOperation::recv
#[must_use]
pub struct SelectedOperation<'a> {
    /// Token needed to complete the operation.
    token: Token,

    /// The index of the selected operation.
    index: usize,

    /// The address of the selected `Sender` or `Receiver`.
    ptr: *const u8,

    /// Indicates that `Sender`s and `Receiver`s are borrowed.
    _marker: PhantomData<&'a ()>,
}

impl SelectedOperation<'_> {
    /// Returns the index of the selected operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, Select};
    ///
    /// let (s1, r1) = bounded::<()>(0);
    /// let (s2, r2) = bounded::<()>(0);
    /// let (s3, r3) = bounded::<()>(1);
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.send(&s1);
    /// let oper2 = sel.recv(&r2);
    /// let oper3 = sel.send(&s3);
    ///
    /// // Only the last operation is ready.
    /// let oper = sel.select();
    /// assert_eq!(oper.index(), 2);
    /// assert_eq!(oper.index(), oper3);
    ///
    /// // Complete the operation.
    /// oper.send(&s3, ()).unwrap();
    /// ```
    pub fn index(&self) -> usize {
        self.index
    }

    /// Completes the send operation.
    ///
    /// The passed [`Sender`] reference must be the same one that was used in [`Select::send`]
    /// when the operation was added.
    ///
    /// # Panics
    ///
    /// Panics if an incorrect [`Sender`] reference is passed.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, Select, SendError};
    ///
    /// let (s, r) = bounded::<i32>(0);
    /// drop(r);
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.send(&s);
    ///
    /// let oper = sel.select();
    /// assert_eq!(oper.index(), oper1);
    /// assert_eq!(oper.send(&s, 10), Err(SendError(10)));
    /// ```
    pub fn send<T>(mut self, s: &Sender<T>, msg: T) -> Result<(), SendError<T>> {
        assert!(
            s as *const Sender<T> as *const u8 == self.ptr,
            "passed a sender that wasn't selected",
        );
        let res = unsafe { channel::write(s, &mut self.token, msg) };
        mem::forget(self);
        res.map_err(SendError)
    }

    /// Completes the receive operation.
    ///
    /// The passed [`Receiver`] reference must be the same one that was used in [`Select::recv`]
    /// when the operation was added.
    ///
    /// # Panics
    ///
    /// Panics if an incorrect [`Receiver`] reference is passed.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, Select, RecvError};
    ///
    /// let (s, r) = bounded::<i32>(0);
    /// drop(s);
    ///
    /// let mut sel = Select::new();
    /// let oper1 = sel.recv(&r);
    ///
    /// let oper = sel.select();
    /// assert_eq!(oper.index(), oper1);
    /// assert_eq!(oper.recv(&r), Err(RecvError));
    /// ```
    pub fn recv<T>(mut self, r: &Receiver<T>) -> Result<T, RecvError> {
        assert!(
            r as *const Receiver<T> as *const u8 == self.ptr,
            "passed a receiver that wasn't selected",
        );
        let res = unsafe { channel::read(r, &mut self.token) };
        mem::forget(self);
        res.map_err(|_| RecvError)
    }
}

impl fmt::Debug for SelectedOperation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("SelectedOperation { .. }")
    }
}

impl Drop for SelectedOperation<'_> {
    fn drop(&mut self) {
        panic!("dropped `SelectedOperation` without completing the operation");
    }
}
