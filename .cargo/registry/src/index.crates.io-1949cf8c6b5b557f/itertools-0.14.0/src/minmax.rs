/// `MinMaxResult` is an enum returned by `minmax`.
///
/// See [`.minmax()`](crate::Itertools::minmax) for more detail.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MinMaxResult<T> {
    /// Empty iterator
    NoElements,

    /// Iterator with one element, so the minimum and maximum are the same
    OneElement(T),

    /// More than one element in the iterator, the first element is not larger
    /// than the second
    MinMax(T, T),
}

impl<T: Clone> MinMaxResult<T> {
    /// `into_option` creates an `Option` of type `(T, T)`. The returned `Option`
    /// has variant `None` if and only if the `MinMaxResult` has variant
    /// `NoElements`. Otherwise `Some((x, y))` is returned where `x <= y`.
    /// If the `MinMaxResult` has variant `OneElement(x)`, performing this
    /// operation will make one clone of `x`.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::MinMaxResult::{self, NoElements, OneElement, MinMax};
    ///
    /// let r: MinMaxResult<i32> = NoElements;
    /// assert_eq!(r.into_option(), None);
    ///
    /// let r = OneElement(1);
    /// assert_eq!(r.into_option(), Some((1, 1)));
    ///
    /// let r = MinMax(1, 2);
    /// assert_eq!(r.into_option(), Some((1, 2)));
    /// ```
    pub fn into_option(self) -> Option<(T, T)> {
        match self {
            Self::NoElements => None,
            Self::OneElement(x) => Some((x.clone(), x)),
            Self::MinMax(x, y) => Some((x, y)),
        }
    }
}

/// Implementation guts for `minmax` and `minmax_by_key`.
pub fn minmax_impl<I, K, F, L>(mut it: I, mut key_for: F, mut lt: L) -> MinMaxResult<I::Item>
where
    I: Iterator,
    F: FnMut(&I::Item) -> K,
    L: FnMut(&I::Item, &I::Item, &K, &K) -> bool,
{
    let (mut min, mut max, mut min_key, mut max_key) = match it.next() {
        None => return MinMaxResult::NoElements,
        Some(x) => match it.next() {
            None => return MinMaxResult::OneElement(x),
            Some(y) => {
                let xk = key_for(&x);
                let yk = key_for(&y);
                if !lt(&y, &x, &yk, &xk) {
                    (x, y, xk, yk)
                } else {
                    (y, x, yk, xk)
                }
            }
        },
    };

    loop {
        // `first` and `second` are the two next elements we want to look
        // at.  We first compare `first` and `second` (#1). The smaller one
        // is then compared to current minimum (#2). The larger one is
        // compared to current maximum (#3). This way we do 3 comparisons
        // for 2 elements.
        let first = match it.next() {
            None => break,
            Some(x) => x,
        };
        let second = match it.next() {
            None => {
                let first_key = key_for(&first);
                if lt(&first, &min, &first_key, &min_key) {
                    min = first;
                } else if !lt(&first, &max, &first_key, &max_key) {
                    max = first;
                }
                break;
            }
            Some(x) => x,
        };
        let first_key = key_for(&first);
        let second_key = key_for(&second);
        if !lt(&second, &first, &second_key, &first_key) {
            if lt(&first, &min, &first_key, &min_key) {
                min = first;
                min_key = first_key;
            }
            if !lt(&second, &max, &second_key, &max_key) {
                max = second;
                max_key = second_key;
            }
        } else {
            if lt(&second, &min, &second_key, &min_key) {
                min = second;
                min_key = second_key;
            }
            if !lt(&first, &max, &first_key, &max_key) {
                max = first;
                max_key = first_key;
            }
        }
    }

    MinMaxResult::MinMax(min, max)
}
