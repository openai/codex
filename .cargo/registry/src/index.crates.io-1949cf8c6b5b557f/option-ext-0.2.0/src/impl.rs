use OptionExt;

impl<T> OptionExt<T> for Option<T> {
    fn contains<U>(&self, x: &U) -> bool where U: PartialEq<T> {
        match *self {
            Some(ref y) => x == y,
            None => false,
        }
    }

    #[inline]
    fn map_or2<U, F: FnOnce(T) -> U>(self, f: F, default: U) -> U {
        self.map_or(default, f)
    }

    #[inline]
    fn map_or_else2<U, F: FnOnce(T) -> U, D: FnOnce() -> U>(self, f: F, default: D) -> U {
        self.map_or_else(default, f)
    }
}
