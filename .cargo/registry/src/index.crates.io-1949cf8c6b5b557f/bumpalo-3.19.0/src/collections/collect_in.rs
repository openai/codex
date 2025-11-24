#[cfg(feature = "boxed")]
use crate::boxed::Box;
use crate::collections::{String, Vec};
use crate::Bump;

/// A trait for types that support being constructed from an iterator, parameterized by an allocator.
pub trait FromIteratorIn<A> {
    /// The allocator type
    type Alloc;

    /// Similar to [`FromIterator::from_iter`][from_iter], but with a given allocator.
    ///
    /// [from_iter]: https://doc.rust-lang.org/std/iter/trait.FromIterator.html#tymethod.from_iter
    ///
    /// ```
    /// # use bumpalo::collections::{FromIteratorIn, Vec};
    /// # use bumpalo::Bump;
    /// #
    /// let five_fives = std::iter::repeat(5).take(5);
    /// let bump = Bump::new();
    ///
    /// let v = Vec::from_iter_in(five_fives, &bump);
    ///
    /// assert_eq!(v, [5, 5, 5, 5, 5]);
    /// ```
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = A>;
}

#[cfg(feature = "boxed")]
impl<'bump, T> FromIteratorIn<T> for Box<'bump, [T]> {
    type Alloc = &'bump Bump;

    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Box::from_iter_in(iter, alloc)
    }
}

impl<'bump, T> FromIteratorIn<T> for Vec<'bump, T> {
    type Alloc = &'bump Bump;

    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Vec::from_iter_in(iter, alloc)
    }
}

impl<T, V: FromIteratorIn<T>> FromIteratorIn<Option<T>> for Option<V> {
    type Alloc = V::Alloc;
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = Option<T>>,
    {
        iter.into_iter()
            .map(|x| x.ok_or(()))
            .collect_in::<Result<_, _>>(alloc)
            .ok()
    }
}

impl<T, E, V: FromIteratorIn<T>> FromIteratorIn<Result<T, E>> for Result<V, E> {
    type Alloc = V::Alloc;
    /// Takes each element in the `Iterator`: if it is an `Err`, no further
    /// elements are taken, and the `Err` is returned. Should no `Err` occur, a
    /// container with the values of each `Result` is returned.
    ///
    /// Here is an example which increments every integer in a vector,
    /// checking for overflow:
    ///
    /// ```
    /// # use bumpalo::collections::{FromIteratorIn, CollectIn, Vec, String};
    /// # use bumpalo::Bump;
    /// #
    /// let bump = Bump::new();
    ///
    /// let v = vec![1, 2, u32::MAX];
    /// let res: Result<Vec<u32>, &'static str> = v.iter().take(2).map(|x: &u32|
    ///     x.checked_add(1).ok_or("Overflow!")
    /// ).collect_in(&bump);
    /// assert_eq!(res, Ok(bumpalo::vec![in &bump; 2, 3]));
    ///
    /// let res: Result<Vec<u32>, &'static str> = v.iter().map(|x: &u32|
    ///     x.checked_add(1).ok_or("Overflow!")
    /// ).collect_in(&bump);
    /// assert_eq!(res, Err("Overflow!"));
    /// ```
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = Result<T, E>>,
    {
        let mut iter = iter.into_iter();
        let mut error = None;
        let container = core::iter::from_fn(|| match iter.next() {
            Some(Ok(x)) => Some(x),
            Some(Err(e)) => {
                error = Some(e);
                None
            }
            None => None,
        })
        .collect_in(alloc);

        match error {
            Some(e) => Err(e),
            None => Ok(container),
        }
    }
}

impl<'bump> FromIteratorIn<char> for String<'bump> {
    type Alloc = &'bump Bump;

    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = char>,
    {
        String::from_iter_in(iter, alloc)
    }
}

/// Extension trait for iterators, in order to allow allocator-parameterized collections to be constructed more easily.
pub trait CollectIn: Iterator + Sized {
    /// Collect all items from an iterator, into a collection parameterized by an allocator.
    /// Similar to [`Iterator::collect`][collect].
    ///
    /// [collect]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.collect
    ///
    /// ```
    /// # use bumpalo::collections::{FromIteratorIn, CollectIn, Vec, String};
    /// # use bumpalo::Bump;
    /// #
    /// let bump = Bump::new();
    ///
    /// let str = "hello, world!".to_owned();
    /// let bump_str: String = str.chars().collect_in(&bump);
    /// assert_eq!(&bump_str, &str);
    ///
    /// let nums: Vec<i32> = (0..=3).collect_in::<Vec<_>>(&bump);
    /// assert_eq!(&nums, &[0,1,2,3]);
    /// ```
    fn collect_in<C: FromIteratorIn<Self::Item>>(self, alloc: C::Alloc) -> C {
        C::from_iter_in(self, alloc)
    }
}

impl<I: Iterator> CollectIn for I {}
