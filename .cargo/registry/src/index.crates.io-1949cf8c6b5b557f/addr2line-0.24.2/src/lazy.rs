use core::cell::UnsafeCell;

pub(crate) type LazyResult<T> = LazyCell<Result<T, crate::Error>>;

pub(crate) struct LazyCell<T> {
    contents: UnsafeCell<Option<T>>,
}

impl<T> LazyCell<T> {
    pub(crate) fn new() -> LazyCell<T> {
        LazyCell {
            contents: UnsafeCell::new(None),
        }
    }

    pub(crate) fn borrow(&self) -> Option<&T> {
        unsafe { &*self.contents.get() }.as_ref()
    }

    pub(crate) fn borrow_with(&self, closure: impl FnOnce() -> T) -> &T {
        // First check if we're already initialized...
        let ptr = self.contents.get();
        if let Some(val) = unsafe { &*ptr } {
            return val;
        }
        // Note that while we're executing `closure` our `borrow_with` may
        // be called recursively. This means we need to check again after
        // the closure has executed. For that we use the `get_or_insert`
        // method which will only perform mutation if we aren't already
        // `Some`.
        let val = closure();
        unsafe { (*ptr).get_or_insert(val) }
    }
}
