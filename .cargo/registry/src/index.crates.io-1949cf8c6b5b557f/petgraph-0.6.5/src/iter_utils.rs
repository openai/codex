pub trait IterUtilsExt: Iterator {
    /// Return the first element that maps to `Some(_)`, or None if the iterator
    /// was exhausted.
    fn ex_find_map<F, R>(&mut self, mut f: F) -> Option<R>
    where
        F: FnMut(Self::Item) -> Option<R>,
    {
        for elt in self {
            if let result @ Some(_) = f(elt) {
                return result;
            }
        }
        None
    }

    /// Return the last element from the back that maps to `Some(_)`, or
    /// None if the iterator was exhausted.
    fn ex_rfind_map<F, R>(&mut self, mut f: F) -> Option<R>
    where
        F: FnMut(Self::Item) -> Option<R>,
        Self: DoubleEndedIterator,
    {
        while let Some(elt) = self.next_back() {
            if let result @ Some(_) = f(elt) {
                return result;
            }
        }
        None
    }
}

impl<I> IterUtilsExt for I where I: Iterator {}
