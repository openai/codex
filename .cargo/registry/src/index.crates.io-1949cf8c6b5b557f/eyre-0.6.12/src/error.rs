use crate::chain::Chain;
use crate::ptr::{MutPtr, OwnedPtr, RefPtr};
use crate::EyreHandler;
use crate::{Report, StdError};
use core::any::TypeId;
use core::fmt::{self, Debug, Display};
use core::mem::{self, ManuallyDrop};
use core::ptr::{self, NonNull};

use core::ops::{Deref, DerefMut};

impl Report {
    /// Create a new error object from any error type.
    ///
    /// The error type must be threadsafe and `'static`, so that the `Report`
    /// will be as well.
    ///
    /// If the error type does not provide a backtrace, a backtrace will be
    /// created here to ensure that a backtrace exists.
    #[cfg_attr(track_caller, track_caller)]
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Report::from_std(error)
    }

    /// Create a new error object from a printable error message.
    ///
    /// If the argument implements std::error::Error, prefer `Report::new`
    /// instead which preserves the underlying error's cause chain and
    /// backtrace. If the argument may or may not implement std::error::Error
    /// now or in the future, use `eyre!(err)` which handles either way
    /// correctly.
    ///
    /// `Report::msg("...")` is equivalent to `eyre!("...")` but occasionally
    /// convenient in places where a function is preferable over a macro, such
    /// as iterator or stream combinators:
    ///
    /// ```
    /// # mod ffi {
    /// #     pub struct Input;
    /// #     pub struct Output;
    /// #     pub async fn do_some_work(_: Input) -> Result<Output, &'static str> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # use ffi::{Input, Output};
    /// #
    /// use eyre::{Report, Result};
    /// use futures::stream::{Stream, StreamExt, TryStreamExt};
    ///
    /// async fn demo<S>(stream: S) -> Result<Vec<Output>>
    /// where
    ///     S: Stream<Item = Input>,
    /// {
    ///     stream
    ///         .then(ffi::do_some_work) // returns Result<Output, &str>
    ///         .map_err(Report::msg)
    ///         .try_collect()
    ///         .await
    /// }
    /// ```
    #[cfg_attr(track_caller, track_caller)]
    pub fn msg<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Report::from_adhoc(message)
    }

    #[cfg_attr(track_caller, track_caller)]
    /// Creates a new error from an implementor of [`std::error::Error`]
    pub(crate) fn from_std<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let vtable = &ErrorVTable {
            object_drop: object_drop::<E>,
            object_ref: object_ref::<E>,
            object_mut: object_mut::<E>,
            object_boxed: object_boxed::<E>,
            object_downcast: object_downcast::<E>,
            object_downcast_mut: object_downcast_mut::<E>,
            object_drop_rest: object_drop_front::<E>,
        };

        // Safety: passing vtable that operates on the right type E.
        let handler = Some(crate::capture_handler(&error));

        unsafe { Report::construct(error, vtable, handler) }
    }

    #[cfg_attr(track_caller, track_caller)]
    pub(crate) fn from_adhoc<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        use crate::wrapper::MessageError;
        let error: MessageError<M> = MessageError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<MessageError<M>>,
            object_ref: object_ref::<MessageError<M>>,
            object_mut: object_mut::<MessageError<M>>,
            object_boxed: object_boxed::<MessageError<M>>,
            object_downcast: object_downcast::<M>,
            object_downcast_mut: object_downcast_mut::<M>,
            object_drop_rest: object_drop_front::<M>,
        };

        // Safety: MessageError is repr(transparent) so it is okay for the
        // vtable to allow casting the MessageError<M> to M.
        let handler = Some(crate::capture_handler(&error));

        unsafe { Report::construct(error, vtable, handler) }
    }

    #[cfg_attr(track_caller, track_caller)]
    pub(crate) fn from_display<M>(message: M) -> Self
    where
        M: Display + Send + Sync + 'static,
    {
        use crate::wrapper::{DisplayError, NoneError};
        let error: DisplayError<M> = DisplayError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<DisplayError<M>>,
            object_ref: object_ref::<DisplayError<M>>,
            object_mut: object_mut::<DisplayError<M>>,
            object_boxed: object_boxed::<DisplayError<M>>,
            object_downcast: object_downcast::<M>,
            object_downcast_mut: object_downcast_mut::<M>,
            object_drop_rest: object_drop_front::<M>,
        };

        // Safety: DisplayError is repr(transparent) so it is okay for the
        // vtable to allow casting the DisplayError<M> to M.
        let handler = Some(crate::capture_handler(&NoneError));

        unsafe { Report::construct(error, vtable, handler) }
    }

    #[cfg_attr(track_caller, track_caller)]
    pub(crate) fn from_msg<D, E>(msg: D, error: E) -> Self
    where
        D: Display + Send + Sync + 'static,
        E: StdError + Send + Sync + 'static,
    {
        let error: ContextError<D, E> = ContextError { msg, error };

        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<D, E>>,
            object_ref: object_ref::<ContextError<D, E>>,
            object_mut: object_mut::<ContextError<D, E>>,
            object_boxed: object_boxed::<ContextError<D, E>>,
            object_downcast: context_downcast::<D, E>,
            object_downcast_mut: context_downcast_mut::<D, E>,
            object_drop_rest: context_drop_rest::<D, E>,
        };

        // Safety: passing vtable that operates on the right type.
        let handler = Some(crate::capture_handler(&error));

        unsafe { Report::construct(error, vtable, handler) }
    }

    #[cfg_attr(track_caller, track_caller)]
    pub(crate) fn from_boxed(error: Box<dyn StdError + Send + Sync>) -> Self {
        use crate::wrapper::BoxedError;
        let error = BoxedError(error);
        let handler = Some(crate::capture_handler(&error));

        let vtable = &ErrorVTable {
            object_drop: object_drop::<BoxedError>,
            object_ref: object_ref::<BoxedError>,
            object_mut: object_mut::<BoxedError>,
            object_boxed: object_boxed::<BoxedError>,
            object_downcast: object_downcast::<Box<dyn StdError + Send + Sync>>,
            object_downcast_mut: object_downcast_mut::<Box<dyn StdError + Send + Sync>>,
            object_drop_rest: object_drop_front::<Box<dyn StdError + Send + Sync>>,
        };

        // Safety: BoxedError is repr(transparent) so it is okay for the vtable
        // to allow casting to Box<dyn StdError + Send + Sync>.
        unsafe { Report::construct(error, vtable, handler) }
    }

    // Takes backtrace as argument rather than capturing it here so that the
    // user sees one fewer layer of wrapping noise in the backtrace.
    //
    // Unsafe because the given vtable must have sensible behavior on the error
    // value of type E.
    unsafe fn construct<E>(
        error: E,
        vtable: &'static ErrorVTable,
        handler: Option<Box<dyn EyreHandler>>,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let inner = ErrorImpl {
            header: ErrorHeader { vtable, handler },
            _object: error,
        };

        // Construct a new owned allocation through a raw pointer
        //
        // This does not keep the allocation around as a `Box` which would invalidate an
        // references when moved
        let ptr = OwnedPtr::<ErrorImpl<E>>::new(inner);

        // Safety: the type
        let ptr = ptr.cast::<ErrorImpl<()>>();
        Report { inner: ptr }
    }

    /// Create a new error from an error message to wrap the existing error.
    ///
    /// For attaching a higher level error message to a `Result` as it is propagated, the
    /// [`WrapErr`][crate::WrapErr] extension trait may be more convenient than this function.
    ///
    /// The primary reason to use `error.wrap_err(...)` instead of `result.wrap_err(...)` via the
    /// `WrapErr` trait would be if the message needs to depend on some data held by the underlying
    /// error:
    ///
    /// ```
    /// # use std::fmt::{self, Debug, Display};
    /// #
    /// # type T = ();
    /// #
    /// # impl std::error::Error for ParseError {}
    /// # impl Debug for ParseError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// # impl Display for ParseError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// use eyre::Result;
    /// use std::fs::File;
    /// use std::path::Path;
    ///
    /// struct ParseError {
    ///     line: usize,
    ///     column: usize,
    /// }
    ///
    /// fn parse_impl(file: File) -> Result<T, ParseError> {
    ///     # const IGNORE: &str = stringify! {
    ///     ...
    ///     # };
    ///     # unimplemented!()
    /// }
    ///
    /// pub fn parse(path: impl AsRef<Path>) -> Result<T> {
    ///     let file = File::open(&path)?;
    ///     parse_impl(file).map_err(|error| {
    ///         let message = format!(
    ///             "only the first {} lines of {} are valid",
    ///             error.line, path.as_ref().display(),
    ///         );
    ///         eyre::Report::new(error).wrap_err(message)
    ///     })
    /// }
    /// ```
    pub fn wrap_err<D>(mut self, msg: D) -> Self
    where
        D: Display + Send + Sync + 'static,
    {
        // Safety: this access a `ErrorImpl<unknown>` as a valid reference to a `ErrorImpl<()>`
        //
        // As the generic is at the end of the struct and the struct is `repr(C)` this reference
        // will be within bounds of the original pointer, and the field will have the same offset
        let handler = header_mut(self.inner.as_mut()).handler.take();
        let error: ContextError<D, Report> = ContextError { msg, error: self };

        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<D, Report>>,
            object_ref: object_ref::<ContextError<D, Report>>,
            object_mut: object_mut::<ContextError<D, Report>>,
            object_boxed: object_boxed::<ContextError<D, Report>>,
            object_downcast: context_chain_downcast::<D>,
            object_downcast_mut: context_chain_downcast_mut::<D>,
            object_drop_rest: context_chain_drop_rest::<D>,
        };

        // Safety: passing vtable that operates on the right type.
        unsafe { Report::construct(error, vtable, handler) }
    }

    /// Access the vtable for the current error object.
    fn vtable(&self) -> &'static ErrorVTable {
        header(self.inner.as_ref()).vtable
    }

    /// An iterator of the chain of source errors contained by this Report.
    ///
    /// This iterator will visit every error in the cause chain of this error
    /// object, beginning with the error that this error object was created
    /// from.
    ///
    /// # Example
    ///
    /// ```
    /// use eyre::Report;
    /// use std::io;
    ///
    /// pub fn underlying_io_error_kind(error: &Report) -> Option<io::ErrorKind> {
    ///     for cause in error.chain() {
    ///         if let Some(io_error) = cause.downcast_ref::<io::Error>() {
    ///             return Some(io_error.kind());
    ///         }
    ///     }
    ///     None
    /// }
    /// ```
    pub fn chain(&self) -> Chain<'_> {
        ErrorImpl::chain(self.inner.as_ref())
    }

    /// The lowest level cause of this error &mdash; this error's cause's
    /// cause's cause etc.
    ///
    /// The root cause is the last error in the iterator produced by
    /// [`chain()`][Report::chain].
    pub fn root_cause(&self) -> &(dyn StdError + 'static) {
        let mut chain = self.chain();
        let mut root_cause = chain.next().unwrap();
        for cause in chain {
            root_cause = cause;
        }
        root_cause
    }

    /// Returns true if `E` is the type held by this error object.
    ///
    /// For errors constructed from messages, this method returns true if `E` matches the type of
    /// the message `D` **or** the type of the error on which the message has been attached. For
    /// details about the interaction between message and downcasting, [see here].
    ///
    /// [see here]: trait.WrapErr.html#effect-on-downcasting
    pub fn is<E>(&self) -> bool
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.downcast_ref::<E>().is_some()
    }

    /// Attempt to downcast the error object to a concrete type.
    pub fn downcast<E>(self) -> Result<E, Self>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = match (self.vtable().object_downcast)(self.inner.as_ref(), target) {
                Some(addr) => addr,
                None => return Err(self),
            };

            // Prepare to read E out of the data structure. We'll drop the rest
            // of the data structure separately so that E is not dropped.
            let outer = ManuallyDrop::new(self);

            // Read E from where the vtable found it.
            let error = ptr::read(addr.cast::<E>().as_ptr());

            // Read Box<ErrorImpl<()>> from self. Can't move it out because
            // Report has a Drop impl which we want to not run.
            let inner = ptr::read(&outer.inner);

            // Drop rest of the data structure outside of E.
            (outer.vtable().object_drop_rest)(inner, target);

            Ok(error)
        }
    }

    /// Downcast this error object by reference.
    ///
    /// # Example
    ///
    /// ```
    /// # use eyre::{Report, eyre};
    /// # use std::fmt::{self, Display};
    /// # use std::task::Poll;
    /// #
    /// # #[derive(Debug)]
    /// # enum DataStoreError {
    /// #     Censored(()),
    /// # }
    /// #
    /// # impl Display for DataStoreError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # impl std::error::Error for DataStoreError {}
    /// #
    /// # const REDACTED_CONTENT: () = ();
    /// #
    /// # #[cfg(not(feature = "auto-install"))]
    /// # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
    /// #
    /// # let error: Report = eyre!("...");
    /// # let root_cause = &error;
    /// #
    /// # let ret =
    /// // If the error was caused by redaction, then return a tombstone instead
    /// // of the content.
    /// match root_cause.downcast_ref::<DataStoreError>() {
    ///     Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
    ///     None => Err(error),
    /// }
    /// # ;
    /// ```
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (self.vtable().object_downcast)(self.inner.as_ref(), target)?;
            Some(addr.cast::<E>().as_ref())
        }
    }

    /// Downcast this error object by mutable reference.
    pub fn downcast_mut<E>(&mut self) -> Option<&mut E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (self.vtable().object_downcast_mut)(self.inner.as_mut(), target)?;
            Some(addr.cast::<E>().as_mut())
        }
    }

    /// Get a reference to the Handler for this Report.
    pub fn handler(&self) -> &dyn EyreHandler {
        header(self.inner.as_ref())
            .handler
            .as_ref()
            .unwrap()
            .as_ref()
    }

    /// Get a mutable reference to the Handler for this Report.
    pub fn handler_mut(&mut self) -> &mut dyn EyreHandler {
        header_mut(self.inner.as_mut())
            .handler
            .as_mut()
            .unwrap()
            .as_mut()
    }

    /// Get a reference to the Handler for this Report.
    #[doc(hidden)]
    pub fn context(&self) -> &dyn EyreHandler {
        header(self.inner.as_ref())
            .handler
            .as_ref()
            .unwrap()
            .as_ref()
    }

    /// Get a mutable reference to the Handler for this Report.
    #[doc(hidden)]
    pub fn context_mut(&mut self) -> &mut dyn EyreHandler {
        header_mut(self.inner.as_mut())
            .handler
            .as_mut()
            .unwrap()
            .as_mut()
    }
}

impl<E> From<E> for Report
where
    E: StdError + Send + Sync + 'static,
{
    #[cfg_attr(track_caller, track_caller)]
    fn from(error: E) -> Self {
        Report::from_std(error)
    }
}

impl Deref for Report {
    type Target = dyn StdError + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        ErrorImpl::error(self.inner.as_ref())
    }
}

impl DerefMut for Report {
    fn deref_mut(&mut self) -> &mut Self::Target {
        ErrorImpl::error_mut(self.inner.as_mut())
    }
}

impl Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        ErrorImpl::display(self.inner.as_ref(), formatter)
    }
}

impl Debug for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        ErrorImpl::debug(self.inner.as_ref(), formatter)
    }
}

impl Drop for Report {
    fn drop(&mut self) {
        unsafe {
            // Read Box<ErrorImpl<()>> from self.
            (self.vtable().object_drop)(self.inner);
        }
    }
}

struct ErrorVTable {
    object_drop: unsafe fn(OwnedPtr<ErrorImpl<()>>),
    object_ref: unsafe fn(RefPtr<'_, ErrorImpl<()>>) -> &(dyn StdError + Send + Sync + 'static),
    object_mut: unsafe fn(MutPtr<'_, ErrorImpl<()>>) -> &mut (dyn StdError + Send + Sync + 'static),
    #[allow(clippy::type_complexity)]
    object_boxed: unsafe fn(OwnedPtr<ErrorImpl<()>>) -> Box<dyn StdError + Send + Sync + 'static>,
    object_downcast: unsafe fn(RefPtr<'_, ErrorImpl<()>>, TypeId) -> Option<NonNull<()>>,
    object_downcast_mut: unsafe fn(MutPtr<'_, ErrorImpl<()>>, TypeId) -> Option<NonNull<()>>,
    object_drop_rest: unsafe fn(OwnedPtr<ErrorImpl<()>>, TypeId),
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_drop<E>(e: OwnedPtr<ErrorImpl<()>>) {
    // Cast to a context type and drop the Box allocation.
    let unerased = unsafe { e.cast::<ErrorImpl<E>>().into_box() };
    drop(unerased);
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_drop_front<E>(e: OwnedPtr<ErrorImpl<()>>, target: TypeId) {
    // Drop the fields of ErrorImpl other than E as well as the Box allocation,
    // without dropping E itself. This is used by downcast after doing a
    // ptr::read to take ownership of the E.
    let _ = target;
    // Note: This must not use `mem::transmute` because it tries to reborrow the `Unique`
    //   contained in `Box`, which must not be done. In practice this probably won't make any
    //   difference by now, but technically it's unsound.
    //   see: https://github.com/rust-lang/unsafe-code-guidelines/blob/master/wip/stacked-borrows.m
    let unerased = unsafe { e.cast::<ErrorImpl<E>>().into_box() };

    mem::forget(unerased._object)
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_ref<E>(e: RefPtr<'_, ErrorImpl<()>>) -> &(dyn StdError + Send + Sync + 'static)
where
    E: StdError + Send + Sync + 'static,
{
    // Attach E's native StdError vtable onto a pointer to self._object.
    &unsafe { e.cast::<ErrorImpl<E>>().as_ref() }._object
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_mut<E>(e: MutPtr<'_, ErrorImpl<()>>) -> &mut (dyn StdError + Send + Sync + 'static)
where
    E: StdError + Send + Sync + 'static,
{
    // Attach E's native StdError vtable onto a pointer to self._object.
    &mut unsafe { e.cast::<ErrorImpl<E>>().into_mut() }._object
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_boxed<E>(e: OwnedPtr<ErrorImpl<()>>) -> Box<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    // Attach ErrorImpl<E>'s native StdError vtable. The StdError impl is below.
    unsafe { e.cast::<ErrorImpl<E>>().into_box() }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_downcast<E>(e: RefPtr<'_, ErrorImpl<()>>, target: TypeId) -> Option<NonNull<()>>
where
    E: 'static,
{
    if TypeId::of::<E>() == target {
        // Caller is looking for an E pointer and e is ErrorImpl<E>, take a
        // pointer to its E field.
        let unerased = unsafe { e.cast::<ErrorImpl<E>>().as_ref() };
        Some(NonNull::from(&(unerased._object)).cast::<()>())
    } else {
        None
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<E>.
unsafe fn object_downcast_mut<E>(
    e: MutPtr<'_, ErrorImpl<()>>,
    target: TypeId,
) -> Option<NonNull<()>>
where
    E: 'static,
{
    if TypeId::of::<E>() == target {
        // Caller is looking for an E pointer and e is ErrorImpl<E>, take a
        // pointer to its E field.
        let unerased = unsafe { e.cast::<ErrorImpl<E>>().into_mut() };
        Some(NonNull::from(&mut (unerased._object)).cast::<()>())
    } else {
        None
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, E>>.
unsafe fn context_downcast<D, E>(
    e: RefPtr<'_, ErrorImpl<()>>,
    target: TypeId,
) -> Option<NonNull<()>>
where
    D: 'static,
    E: 'static,
{
    if TypeId::of::<D>() == target {
        let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, E>>>().as_ref() };
        let addr = NonNull::from(&unerased._object.msg).cast::<()>();
        Some(addr)
    } else if TypeId::of::<E>() == target {
        let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, E>>>().as_ref() };
        let addr = NonNull::from(&unerased._object.error).cast::<()>();
        Some(addr)
    } else {
        None
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, E>>.
unsafe fn context_downcast_mut<D, E>(
    e: MutPtr<'_, ErrorImpl<()>>,
    target: TypeId,
) -> Option<NonNull<()>>
where
    D: 'static,
    E: 'static,
{
    if TypeId::of::<D>() == target {
        let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, E>>>().into_mut() };
        let addr = NonNull::from(&unerased._object.msg).cast::<()>();
        Some(addr)
    } else if TypeId::of::<E>() == target {
        let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, E>>>().into_mut() };
        let addr = NonNull::from(&mut unerased._object.error).cast::<()>();
        Some(addr)
    } else {
        None
    }
}
/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, E>>.
unsafe fn context_drop_rest<D, E>(e: OwnedPtr<ErrorImpl<()>>, target: TypeId)
where
    D: 'static,
    E: 'static,
{
    // Called after downcasting by value to either the D or the E and doing a
    // ptr::read to take ownership of that value.
    if TypeId::of::<D>() == target {
        unsafe {
            e.cast::<ErrorImpl<ContextError<ManuallyDrop<D>, E>>>()
                .into_box()
        };
    } else {
        debug_assert_eq!(TypeId::of::<E>(), target);
        unsafe {
            e.cast::<ErrorImpl<ContextError<D, ManuallyDrop<E>>>>()
                .into_box()
        };
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, Report>>.
unsafe fn context_chain_downcast<D>(
    e: RefPtr<'_, ErrorImpl<()>>,
    target: TypeId,
) -> Option<NonNull<()>>
where
    D: 'static,
{
    let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, Report>>>().as_ref() };
    if TypeId::of::<D>() == target {
        let addr = NonNull::from(&unerased._object.msg).cast::<()>();
        Some(addr)
    } else {
        // Recurse down the context chain per the inner error's vtable.
        let source = &unerased._object.error;
        unsafe { (source.vtable().object_downcast)(source.inner.as_ref(), target) }
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, Report>>.
unsafe fn context_chain_downcast_mut<D>(
    e: MutPtr<'_, ErrorImpl<()>>,
    target: TypeId,
) -> Option<NonNull<()>>
where
    D: 'static,
{
    let unerased = unsafe { e.cast::<ErrorImpl<ContextError<D, Report>>>().into_mut() };
    if TypeId::of::<D>() == target {
        let addr = NonNull::from(&unerased._object.msg).cast::<()>();
        Some(addr)
    } else {
        // Recurse down the context chain per the inner error's vtable.
        let source = &mut unerased._object.error;
        unsafe { (source.vtable().object_downcast_mut)(source.inner.as_mut(), target) }
    }
}

/// # Safety
///
/// Requires layout of *e to match ErrorImpl<ContextError<D, Report>>.
unsafe fn context_chain_drop_rest<D>(e: OwnedPtr<ErrorImpl<()>>, target: TypeId)
where
    D: 'static,
{
    // Called after downcasting by value to either the D or one of the causes
    // and doing a ptr::read to take ownership of that value.
    if TypeId::of::<D>() == target {
        let unerased = unsafe {
            e.cast::<ErrorImpl<ContextError<ManuallyDrop<D>, Report>>>()
                .into_box()
        };
        // Drop the entire rest of the data structure rooted in the next Report.
        drop(unerased);
    } else {
        unsafe {
            let unerased = e
                .cast::<ErrorImpl<ContextError<D, ManuallyDrop<Report>>>>()
                .into_box();
            // Read out a ManuallyDrop<Box<ErrorImpl<()>>> from the next error.
            let inner = ptr::read(&unerased.as_ref()._object.error.inner);
            drop(unerased);
            // Recursively drop the next error using the same target typeid.
            (header(inner.as_ref()).vtable.object_drop_rest)(inner, target);
        }
    }
}

#[repr(C)]
pub(crate) struct ErrorHeader {
    vtable: &'static ErrorVTable,
    pub(crate) handler: Option<Box<dyn EyreHandler>>,
}

// repr C to ensure that E remains in the final position.
#[repr(C)]
pub(crate) struct ErrorImpl<E = ()> {
    header: ErrorHeader,
    // NOTE: Don't use directly. Use only through vtable. Erased type may have
    // different alignment.
    _object: E,
}

// repr C to ensure that ContextError<D, E> has the same layout as
// ContextError<ManuallyDrop<D>, E> and ContextError<D, ManuallyDrop<E>>.
#[repr(C)]
pub(crate) struct ContextError<D, E> {
    pub(crate) msg: D,
    pub(crate) error: E,
}

impl<E> ErrorImpl<E> {
    /// Returns a type erased Error
    fn erase(&self) -> RefPtr<'_, ErrorImpl<()>> {
        // Erase the concrete type of E but preserve the vtable in self.vtable
        // for manipulating the resulting thin pointer. This is analogous to an
        // unsize coersion.
        RefPtr::new(self).cast()
    }
}

// Reads the header out of `p`. This is the same as `p.as_ref().header`, but
// avoids converting `p` into a reference of a shrunk provenance with a type different than the
// allocation.
fn header(p: RefPtr<'_, ErrorImpl<()>>) -> &'_ ErrorHeader {
    // Safety: `ErrorHeader` is the first field of repr(C) `ErrorImpl`
    unsafe { p.cast().as_ref() }
}

fn header_mut(p: MutPtr<'_, ErrorImpl<()>>) -> &mut ErrorHeader {
    // Safety: `ErrorHeader` is the first field of repr(C) `ErrorImpl`
    unsafe { p.cast().into_mut() }
}

impl ErrorImpl<()> {
    pub(crate) fn error(this: RefPtr<'_, Self>) -> &(dyn StdError + Send + Sync + 'static) {
        // Use vtable to attach E's native StdError vtable for the right
        // original type E.
        unsafe { (header(this).vtable.object_ref)(this) }
    }

    pub(crate) fn error_mut(this: MutPtr<'_, Self>) -> &mut (dyn StdError + Send + Sync + 'static) {
        // Use vtable to attach E's native StdError vtable for the right
        // original type E.
        unsafe { (header_mut(this).vtable.object_mut)(this) }
    }

    pub(crate) fn chain(this: RefPtr<'_, Self>) -> Chain<'_> {
        Chain::new(Self::error(this))
    }

    pub(crate) fn header(this: RefPtr<'_, ErrorImpl>) -> &ErrorHeader {
        header(this)
    }
}

impl<E> StdError for ErrorImpl<E>
where
    E: StdError,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        ErrorImpl::<()>::error(self.erase()).source()
    }
}

impl<E> Debug for ErrorImpl<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        ErrorImpl::debug(self.erase(), formatter)
    }
}

impl<E> Display for ErrorImpl<E>
where
    E: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(ErrorImpl::error(self.erase()), formatter)
    }
}

impl From<Report> for Box<dyn StdError + Send + Sync + 'static> {
    fn from(error: Report) -> Self {
        let outer = ManuallyDrop::new(error);
        unsafe {
            // Read Box<ErrorImpl<()>> from error. Can't move it out because
            // Report has a Drop impl which we want to not run.
            // Use vtable to attach ErrorImpl<E>'s native StdError vtable for
            // the right original type E.
            (header(outer.inner.as_ref()).vtable.object_boxed)(outer.inner)
        }
    }
}

impl From<Report> for Box<dyn StdError + 'static> {
    fn from(error: Report) -> Self {
        Box::<dyn StdError + Send + Sync>::from(error)
    }
}

impl AsRef<dyn StdError + Send + Sync> for Report {
    fn as_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        &**self
    }
}

impl AsRef<dyn StdError> for Report {
    fn as_ref(&self) -> &(dyn StdError + 'static) {
        &**self
    }
}

#[cfg(feature = "pyo3")]
mod pyo3_compat;
