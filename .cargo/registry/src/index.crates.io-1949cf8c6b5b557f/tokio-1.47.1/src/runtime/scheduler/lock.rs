/// A lock (mutex) yielding generic data.
pub(crate) trait Lock<T> {
    type Handle: AsMut<T>;

    fn lock(self) -> Self::Handle;
}
