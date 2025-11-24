use std::{
    cell::UnsafeCell,
    ffi::OsStr,
    fmt::{self, Write},
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{RefUnwindSafe, UnwindSafe},
    path::Path,
    sync::Once,
};

pub(super) struct JoinOsStrs<'a, T> {
    pub(super) slice: &'a [T],
    pub(super) delimiter: char,
}

impl<T> fmt::Display for JoinOsStrs<'_, T>
where
    T: AsRef<OsStr>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.slice.len();
        for (index, os_str) in self.slice.iter().enumerate() {
            // TODO: Use OsStr::display once it is stablised,
            // Path and OsStr has the same `Display` impl
            write!(f, "{}", Path::new(os_str).display())?;
            if index + 1 < len {
                f.write_char(self.delimiter)?;
            }
        }
        Ok(())
    }
}

pub(super) struct OptionOsStrDisplay<T>(pub(super) Option<T>);

impl<T> fmt::Display for OptionOsStrDisplay<T>
where
    T: AsRef<OsStr>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Use OsStr::display once it is stablised
        // Path and OsStr has the same `Display` impl
        if let Some(os_str) = self.0.as_ref() {
            write!(f, "Some({})", Path::new(os_str).display())
        } else {
            f.write_str("None")
        }
    }
}

pub(crate) struct OnceLock<T> {
    once: Once,
    value: UnsafeCell<MaybeUninit<T>>,
    _marker: PhantomData<T>,
}

impl<T> Default for OnceLock<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> OnceLock<T> {
    pub(crate) const fn new() -> Self {
        Self {
            once: Once::new(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    #[inline]
    fn is_initialized(&self) -> bool {
        self.once.is_completed()
    }

    unsafe fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_initialized());
        #[allow(clippy::needless_borrow)]
        #[allow(unused_unsafe)]
        unsafe {
            (&*self.value.get()).assume_init_ref()
        }
    }

    pub(crate) fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        self.once.call_once(|| {
            unsafe { &mut *self.value.get() }.write(f());
        });
        unsafe { self.get_unchecked() }
    }

    pub(crate) fn get(&self) -> Option<&T> {
        if self.is_initialized() {
            // Safe b/c checked is_initialized
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("OnceLock");
        match self.get() {
            Some(v) => d.field(v),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
unsafe impl<T: Send> Send for OnceLock<T> {}

impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceLock<T> {}
impl<T: UnwindSafe> UnwindSafe for OnceLock<T> {}

impl<T> Drop for OnceLock<T> {
    #[inline]
    fn drop(&mut self) {
        if self.once.is_completed() {
            // SAFETY: The cell is initialized and being dropped, so it can't
            // be accessed again.
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}
