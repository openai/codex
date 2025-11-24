/*!
This module provides forward and reverse substring search routines.

Unlike the standard library's substring search routines, these work on
arbitrary bytes. For all non-empty needles, these routines will report exactly
the same values as the corresponding routines in the standard library. For
the empty needle, the standard library reports matches only at valid UTF-8
boundaries, where as these routines will report matches at every position.

Other than being able to work on arbitrary bytes, the primary reason to prefer
these routines over the standard library routines is that these will generally
be faster. In some cases, significantly so.

# Example: iterating over substring matches

This example shows how to use [`find_iter`] to find occurrences of a substring
in a haystack.

```
use memchr::memmem;

let haystack = b"foo bar foo baz foo";

let mut it = memmem::find_iter(haystack, "foo");
assert_eq!(Some(0), it.next());
assert_eq!(Some(8), it.next());
assert_eq!(Some(16), it.next());
assert_eq!(None, it.next());
```

# Example: iterating over substring matches in reverse

This example shows how to use [`rfind_iter`] to find occurrences of a substring
in a haystack starting from the end of the haystack.

**NOTE:** This module does not implement double ended iterators, so reverse
searches aren't done by calling `rev` on a forward iterator.

```
use memchr::memmem;

let haystack = b"foo bar foo baz foo";

let mut it = memmem::rfind_iter(haystack, "foo");
assert_eq!(Some(16), it.next());
assert_eq!(Some(8), it.next());
assert_eq!(Some(0), it.next());
assert_eq!(None, it.next());
```

# Example: repeating a search for the same needle

It may be possible for the overhead of constructing a substring searcher to be
measurable in some workloads. In cases where the same needle is used to search
many haystacks, it is possible to do construction once and thus to avoid it for
subsequent searches. This can be done with a [`Finder`] (or a [`FinderRev`] for
reverse searches).

```
use memchr::memmem;

let finder = memmem::Finder::new("foo");

assert_eq!(Some(4), finder.find(b"baz foo quux"));
assert_eq!(None, finder.find(b"quux baz bar"));
```
*/

pub use crate::memmem::searcher::PrefilterConfig as Prefilter;

// This is exported here for use in the crate::arch::all::twoway
// implementation. This is essentially an abstraction breaker. Namely, the
// public API of twoway doesn't support providing a prefilter, but its crate
// internal API does. The main reason for this is that I didn't want to do the
// API design required to support it without a concrete use case.
pub(crate) use crate::memmem::searcher::Pre;

use crate::{
    arch::all::{
        packedpair::{DefaultFrequencyRank, HeuristicFrequencyRank},
        rabinkarp,
    },
    cow::CowBytes,
    memmem::searcher::{PrefilterState, Searcher, SearcherRev},
};

mod searcher;

/// Returns an iterator over all non-overlapping occurrences of a substring in
/// a haystack.
///
/// # Complexity
///
/// This routine is guaranteed to have worst case linear time complexity
/// with respect to both the needle and the haystack. That is, this runs
/// in `O(needle.len() + haystack.len())` time.
///
/// This routine is also guaranteed to have worst case constant space
/// complexity.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use memchr::memmem;
///
/// let haystack = b"foo bar foo baz foo";
/// let mut it = memmem::find_iter(haystack, b"foo");
/// assert_eq!(Some(0), it.next());
/// assert_eq!(Some(8), it.next());
/// assert_eq!(Some(16), it.next());
/// assert_eq!(None, it.next());
/// ```
#[inline]
pub fn find_iter<'h, 'n, N: 'n + ?Sized + AsRef<[u8]>>(
    haystack: &'h [u8],
    needle: &'n N,
) -> FindIter<'h, 'n> {
    FindIter::new(haystack, Finder::new(needle))
}

/// Returns a reverse iterator over all non-overlapping occurrences of a
/// substring in a haystack.
///
/// # Complexity
///
/// This routine is guaranteed to have worst case linear time complexity
/// with respect to both the needle and the haystack. That is, this runs
/// in `O(needle.len() + haystack.len())` time.
///
/// This routine is also guaranteed to have worst case constant space
/// complexity.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use memchr::memmem;
///
/// let haystack = b"foo bar foo baz foo";
/// let mut it = memmem::rfind_iter(haystack, b"foo");
/// assert_eq!(Some(16), it.next());
/// assert_eq!(Some(8), it.next());
/// assert_eq!(Some(0), it.next());
/// assert_eq!(None, it.next());
/// ```
#[inline]
pub fn rfind_iter<'h, 'n, N: 'n + ?Sized + AsRef<[u8]>>(
    haystack: &'h [u8],
    needle: &'n N,
) -> FindRevIter<'h, 'n> {
    FindRevIter::new(haystack, FinderRev::new(needle))
}

/// Returns the index of the first occurrence of the given needle.
///
/// Note that if you're are searching for the same needle in many different
/// small haystacks, it may be faster to initialize a [`Finder`] once,
/// and reuse it for each search.
///
/// # Complexity
///
/// This routine is guaranteed to have worst case linear time complexity
/// with respect to both the needle and the haystack. That is, this runs
/// in `O(needle.len() + haystack.len())` time.
///
/// This routine is also guaranteed to have worst case constant space
/// complexity.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use memchr::memmem;
///
/// let haystack = b"foo bar baz";
/// assert_eq!(Some(0), memmem::find(haystack, b"foo"));
/// assert_eq!(Some(4), memmem::find(haystack, b"bar"));
/// assert_eq!(None, memmem::find(haystack, b"quux"));
/// ```
#[inline]
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < 64 {
        rabinkarp::Finder::new(needle).find(haystack, needle)
    } else {
        Finder::new(needle).find(haystack)
    }
}

/// Returns the index of the last occurrence of the given needle.
///
/// Note that if you're are searching for the same needle in many different
/// small haystacks, it may be faster to initialize a [`FinderRev`] once,
/// and reuse it for each search.
///
/// # Complexity
///
/// This routine is guaranteed to have worst case linear time complexity
/// with respect to both the needle and the haystack. That is, this runs
/// in `O(needle.len() + haystack.len())` time.
///
/// This routine is also guaranteed to have worst case constant space
/// complexity.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use memchr::memmem;
///
/// let haystack = b"foo bar baz";
/// assert_eq!(Some(0), memmem::rfind(haystack, b"foo"));
/// assert_eq!(Some(4), memmem::rfind(haystack, b"bar"));
/// assert_eq!(Some(8), memmem::rfind(haystack, b"ba"));
/// assert_eq!(None, memmem::rfind(haystack, b"quux"));
/// ```
#[inline]
pub fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < 64 {
        rabinkarp::FinderRev::new(needle).rfind(haystack, needle)
    } else {
        FinderRev::new(needle).rfind(haystack)
    }
}

/// An iterator over non-overlapping substring matches.
///
/// Matches are reported by the byte offset at which they begin.
///
/// `'h` is the lifetime of the haystack while `'n` is the lifetime of the
/// needle.
#[derive(Debug, Clone)]
pub struct FindIter<'h, 'n> {
    haystack: &'h [u8],
    prestate: PrefilterState,
    finder: Finder<'n>,
    pos: usize,
}

impl<'h, 'n> FindIter<'h, 'n> {
    #[inline(always)]
    pub(crate) fn new(
        haystack: &'h [u8],
        finder: Finder<'n>,
    ) -> FindIter<'h, 'n> {
        let prestate = PrefilterState::new();
        FindIter { haystack, prestate, finder, pos: 0 }
    }

    /// Convert this iterator into its owned variant, such that it no longer
    /// borrows the finder and needle.
    ///
    /// If this is already an owned iterator, then this is a no-op. Otherwise,
    /// this copies the needle.
    ///
    /// This is only available when the `alloc` feature is enabled.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> FindIter<'h, 'static> {
        FindIter {
            haystack: self.haystack,
            prestate: self.prestate,
            finder: self.finder.into_owned(),
            pos: self.pos,
        }
    }
}

impl<'h, 'n> Iterator for FindIter<'h, 'n> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let needle = self.finder.needle();
        let haystack = self.haystack.get(self.pos..)?;
        let idx =
            self.finder.searcher.find(&mut self.prestate, haystack, needle)?;

        let pos = self.pos + idx;
        self.pos = pos + needle.len().max(1);

        Some(pos)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // The largest possible number of non-overlapping matches is the
        // quotient of the haystack and the needle (or the length of the
        // haystack, if the needle is empty)
        match self.haystack.len().checked_sub(self.pos) {
            None => (0, Some(0)),
            Some(haystack_len) => match self.finder.needle().len() {
                // Empty needles always succeed and match at every point
                // (including the very end)
                0 => (
                    haystack_len.saturating_add(1),
                    haystack_len.checked_add(1),
                ),
                needle_len => (0, Some(haystack_len / needle_len)),
            },
        }
    }
}

/// An iterator over non-overlapping substring matches in reverse.
///
/// Matches are reported by the byte offset at which they begin.
///
/// `'h` is the lifetime of the haystack while `'n` is the lifetime of the
/// needle.
#[derive(Clone, Debug)]
pub struct FindRevIter<'h, 'n> {
    haystack: &'h [u8],
    finder: FinderRev<'n>,
    /// When searching with an empty needle, this gets set to `None` after
    /// we've yielded the last element at `0`.
    pos: Option<usize>,
}

impl<'h, 'n> FindRevIter<'h, 'n> {
    #[inline(always)]
    pub(crate) fn new(
        haystack: &'h [u8],
        finder: FinderRev<'n>,
    ) -> FindRevIter<'h, 'n> {
        let pos = Some(haystack.len());
        FindRevIter { haystack, finder, pos }
    }

    /// Convert this iterator into its owned variant, such that it no longer
    /// borrows the finder and needle.
    ///
    /// If this is already an owned iterator, then this is a no-op. Otherwise,
    /// this copies the needle.
    ///
    /// This is only available when the `std` feature is enabled.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> FindRevIter<'h, 'static> {
        FindRevIter {
            haystack: self.haystack,
            finder: self.finder.into_owned(),
            pos: self.pos,
        }
    }
}

impl<'h, 'n> Iterator for FindRevIter<'h, 'n> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let pos = match self.pos {
            None => return None,
            Some(pos) => pos,
        };
        let result = self.finder.rfind(&self.haystack[..pos]);
        match result {
            None => None,
            Some(i) => {
                if pos == i {
                    self.pos = pos.checked_sub(1);
                } else {
                    self.pos = Some(i);
                }
                Some(i)
            }
        }
    }
}

/// A single substring searcher fixed to a particular needle.
///
/// The purpose of this type is to permit callers to construct a substring
/// searcher that can be used to search haystacks without the overhead of
/// constructing the searcher in the first place. This is a somewhat niche
/// concern when it's necessary to re-use the same needle to search multiple
/// different haystacks with as little overhead as possible. In general, using
/// [`find`] is good enough, but `Finder` is useful when you can meaningfully
/// observe searcher construction time in a profile.
///
/// When the `std` feature is enabled, then this type has an `into_owned`
/// version which permits building a `Finder` that is not connected to
/// the lifetime of its needle.
#[derive(Clone, Debug)]
pub struct Finder<'n> {
    needle: CowBytes<'n>,
    searcher: Searcher,
}

impl<'n> Finder<'n> {
    /// Create a new finder for the given needle.
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(needle: &'n B) -> Finder<'n> {
        FinderBuilder::new().build_forward(needle)
    }

    /// Returns the index of the first occurrence of this needle in the given
    /// haystack.
    ///
    /// # Complexity
    ///
    /// This routine is guaranteed to have worst case linear time complexity
    /// with respect to both the needle and the haystack. That is, this runs
    /// in `O(needle.len() + haystack.len())` time.
    ///
    /// This routine is also guaranteed to have worst case constant space
    /// complexity.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use memchr::memmem::Finder;
    ///
    /// let haystack = b"foo bar baz";
    /// assert_eq!(Some(0), Finder::new("foo").find(haystack));
    /// assert_eq!(Some(4), Finder::new("bar").find(haystack));
    /// assert_eq!(None, Finder::new("quux").find(haystack));
    /// ```
    #[inline]
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        let mut prestate = PrefilterState::new();
        let needle = self.needle.as_slice();
        self.searcher.find(&mut prestate, haystack, needle)
    }

    /// Returns an iterator over all occurrences of a substring in a haystack.
    ///
    /// # Complexity
    ///
    /// This routine is guaranteed to have worst case linear time complexity
    /// with respect to both the needle and the haystack. That is, this runs
    /// in `O(needle.len() + haystack.len())` time.
    ///
    /// This routine is also guaranteed to have worst case constant space
    /// complexity.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use memchr::memmem::Finder;
    ///
    /// let haystack = b"foo bar foo baz foo";
    /// let finder = Finder::new(b"foo");
    /// let mut it = finder.find_iter(haystack);
    /// assert_eq!(Some(0), it.next());
    /// assert_eq!(Some(8), it.next());
    /// assert_eq!(Some(16), it.next());
    /// assert_eq!(None, it.next());
    /// ```
    #[inline]
    pub fn find_iter<'a, 'h>(
        &'a self,
        haystack: &'h [u8],
    ) -> FindIter<'h, 'a> {
        FindIter::new(haystack, self.as_ref())
    }

    /// Convert this finder into its owned variant, such that it no longer
    /// borrows the needle.
    ///
    /// If this is already an owned finder, then this is a no-op. Otherwise,
    /// this copies the needle.
    ///
    /// This is only available when the `alloc` feature is enabled.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> Finder<'static> {
        Finder {
            needle: self.needle.into_owned(),
            searcher: self.searcher.clone(),
        }
    }

    /// Convert this finder into its borrowed variant.
    ///
    /// This is primarily useful if your finder is owned and you'd like to
    /// store its borrowed variant in some intermediate data structure.
    ///
    /// Note that the lifetime parameter of the returned finder is tied to the
    /// lifetime of `self`, and may be shorter than the `'n` lifetime of the
    /// needle itself. Namely, a finder's needle can be either borrowed or
    /// owned, so the lifetime of the needle returned must necessarily be the
    /// shorter of the two.
    #[inline]
    pub fn as_ref(&self) -> Finder<'_> {
        Finder {
            needle: CowBytes::new(self.needle()),
            searcher: self.searcher.clone(),
        }
    }

    /// Returns the needle that this finder searches for.
    ///
    /// Note that the lifetime of the needle returned is tied to the lifetime
    /// of the finder, and may be shorter than the `'n` lifetime. Namely, a
    /// finder's needle can be either borrowed or owned, so the lifetime of the
    /// needle returned must necessarily be the shorter of the two.
    #[inline]
    pub fn needle(&self) -> &[u8] {
        self.needle.as_slice()
    }
}

/// A single substring reverse searcher fixed to a particular needle.
///
/// The purpose of this type is to permit callers to construct a substring
/// searcher that can be used to search haystacks without the overhead of
/// constructing the searcher in the first place. This is a somewhat niche
/// concern when it's necessary to re-use the same needle to search multiple
/// different haystacks with as little overhead as possible. In general,
/// using [`rfind`] is good enough, but `FinderRev` is useful when you can
/// meaningfully observe searcher construction time in a profile.
///
/// When the `std` feature is enabled, then this type has an `into_owned`
/// version which permits building a `FinderRev` that is not connected to
/// the lifetime of its needle.
#[derive(Clone, Debug)]
pub struct FinderRev<'n> {
    needle: CowBytes<'n>,
    searcher: SearcherRev,
}

impl<'n> FinderRev<'n> {
    /// Create a new reverse finder for the given needle.
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(needle: &'n B) -> FinderRev<'n> {
        FinderBuilder::new().build_reverse(needle)
    }

    /// Returns the index of the last occurrence of this needle in the given
    /// haystack.
    ///
    /// The haystack may be any type that can be cheaply converted into a
    /// `&[u8]`. This includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// # Complexity
    ///
    /// This routine is guaranteed to have worst case linear time complexity
    /// with respect to both the needle and the haystack. That is, this runs
    /// in `O(needle.len() + haystack.len())` time.
    ///
    /// This routine is also guaranteed to have worst case constant space
    /// complexity.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use memchr::memmem::FinderRev;
    ///
    /// let haystack = b"foo bar baz";
    /// assert_eq!(Some(0), FinderRev::new("foo").rfind(haystack));
    /// assert_eq!(Some(4), FinderRev::new("bar").rfind(haystack));
    /// assert_eq!(None, FinderRev::new("quux").rfind(haystack));
    /// ```
    pub fn rfind<B: AsRef<[u8]>>(&self, haystack: B) -> Option<usize> {
        self.searcher.rfind(haystack.as_ref(), self.needle.as_slice())
    }

    /// Returns a reverse iterator over all occurrences of a substring in a
    /// haystack.
    ///
    /// # Complexity
    ///
    /// This routine is guaranteed to have worst case linear time complexity
    /// with respect to both the needle and the haystack. That is, this runs
    /// in `O(needle.len() + haystack.len())` time.
    ///
    /// This routine is also guaranteed to have worst case constant space
    /// complexity.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use memchr::memmem::FinderRev;
    ///
    /// let haystack = b"foo bar foo baz foo";
    /// let finder = FinderRev::new(b"foo");
    /// let mut it = finder.rfind_iter(haystack);
    /// assert_eq!(Some(16), it.next());
    /// assert_eq!(Some(8), it.next());
    /// assert_eq!(Some(0), it.next());
    /// assert_eq!(None, it.next());
    /// ```
    #[inline]
    pub fn rfind_iter<'a, 'h>(
        &'a self,
        haystack: &'h [u8],
    ) -> FindRevIter<'h, 'a> {
        FindRevIter::new(haystack, self.as_ref())
    }

    /// Convert this finder into its owned variant, such that it no longer
    /// borrows the needle.
    ///
    /// If this is already an owned finder, then this is a no-op. Otherwise,
    /// this copies the needle.
    ///
    /// This is only available when the `std` feature is enabled.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> FinderRev<'static> {
        FinderRev {
            needle: self.needle.into_owned(),
            searcher: self.searcher.clone(),
        }
    }

    /// Convert this finder into its borrowed variant.
    ///
    /// This is primarily useful if your finder is owned and you'd like to
    /// store its borrowed variant in some intermediate data structure.
    ///
    /// Note that the lifetime parameter of the returned finder is tied to the
    /// lifetime of `self`, and may be shorter than the `'n` lifetime of the
    /// needle itself. Namely, a finder's needle can be either borrowed or
    /// owned, so the lifetime of the needle returned must necessarily be the
    /// shorter of the two.
    #[inline]
    pub fn as_ref(&self) -> FinderRev<'_> {
        FinderRev {
            needle: CowBytes::new(self.needle()),
            searcher: self.searcher.clone(),
        }
    }

    /// Returns the needle that this finder searches for.
    ///
    /// Note that the lifetime of the needle returned is tied to the lifetime
    /// of the finder, and may be shorter than the `'n` lifetime. Namely, a
    /// finder's needle can be either borrowed or owned, so the lifetime of the
    /// needle returned must necessarily be the shorter of the two.
    #[inline]
    pub fn needle(&self) -> &[u8] {
        self.needle.as_slice()
    }
}

/// A builder for constructing non-default forward or reverse memmem finders.
///
/// A builder is primarily useful for configuring a substring searcher.
/// Currently, the only configuration exposed is the ability to disable
/// heuristic prefilters used to speed up certain searches.
#[derive(Clone, Debug, Default)]
pub struct FinderBuilder {
    prefilter: Prefilter,
}

impl FinderBuilder {
    /// Create a new finder builder with default settings.
    pub fn new() -> FinderBuilder {
        FinderBuilder::default()
    }

    /// Build a forward finder using the given needle from the current
    /// settings.
    pub fn build_forward<'n, B: ?Sized + AsRef<[u8]>>(
        &self,
        needle: &'n B,
    ) -> Finder<'n> {
        self.build_forward_with_ranker(DefaultFrequencyRank, needle)
    }

    /// Build a forward finder using the given needle and a custom heuristic for
    /// determining the frequency of a given byte in the dataset.
    /// See [`HeuristicFrequencyRank`] for more details.
    pub fn build_forward_with_ranker<
        'n,
        R: HeuristicFrequencyRank,
        B: ?Sized + AsRef<[u8]>,
    >(
        &self,
        ranker: R,
        needle: &'n B,
    ) -> Finder<'n> {
        let needle = needle.as_ref();
        Finder {
            needle: CowBytes::new(needle),
            searcher: Searcher::new(self.prefilter, ranker, needle),
        }
    }

    /// Build a reverse finder using the given needle from the current
    /// settings.
    pub fn build_reverse<'n, B: ?Sized + AsRef<[u8]>>(
        &self,
        needle: &'n B,
    ) -> FinderRev<'n> {
        let needle = needle.as_ref();
        FinderRev {
            needle: CowBytes::new(needle),
            searcher: SearcherRev::new(needle),
        }
    }

    /// Configure the prefilter setting for the finder.
    ///
    /// See the documentation for [`Prefilter`] for more discussion on why
    /// you might want to configure this.
    pub fn prefilter(&mut self, prefilter: Prefilter) -> &mut FinderBuilder {
        self.prefilter = prefilter;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    define_substring_forward_quickcheck!(|h, n| Some(Finder::new(n).find(h)));
    define_substring_reverse_quickcheck!(|h, n| Some(
        FinderRev::new(n).rfind(h)
    ));

    #[test]
    fn forward() {
        crate::tests::substring::Runner::new()
            .fwd(|h, n| Some(Finder::new(n).find(h)))
            .run();
    }

    #[test]
    fn reverse() {
        crate::tests::substring::Runner::new()
            .rev(|h, n| Some(FinderRev::new(n).rfind(h)))
            .run();
    }
}
