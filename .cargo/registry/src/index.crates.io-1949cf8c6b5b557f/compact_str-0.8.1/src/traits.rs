use alloc::string::String;
use core::fmt::{
    self,
    Write,
};
use core::num;

use castaway::{
    match_type,
    LifetimeFree,
};

use super::repr::{
    IntoRepr,
    Repr,
};
use crate::{
    CompactString,
    ToCompactStringError,
    UnwrapWithMsg,
};

/// A trait for converting a value to a `CompactString`.
///
/// This trait is automatically implemented for any type which implements the
/// [`fmt::Display`] trait. As such, [`ToCompactString`] shouldn't be implemented directly:
/// [`fmt::Display`] should be implemented instead, and you get the [`ToCompactString`]
/// implementation for free.
pub trait ToCompactString {
    /// Converts the given value to a [`CompactString`].
    ///
    /// # Panics
    ///
    /// Panics if the system runs out of memory and it cannot hold the whole string,
    /// or if [`Display::fmt()`][core::fmt::Display::fmt] returns an error.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::ToCompactString;
    /// # use compact_str::CompactString;
    ///
    /// let i = 5;
    /// let five = CompactString::new("5");
    ///
    /// assert_eq!(i.to_compact_string(), five);
    /// ```
    #[inline]
    #[track_caller]
    fn to_compact_string(&self) -> CompactString {
        self.try_to_compact_string().unwrap_with_msg()
    }

    /// Fallible version of [`ToCompactString::to_compact_string()`]
    ///
    /// This method won't panic if the system is out-of-memory, but return a
    /// [`ReserveError`][crate::ReserveError].
    /// Otherwise it behaves the same as [`ToCompactString::to_compact_string()`].
    fn try_to_compact_string(&self) -> Result<CompactString, ToCompactStringError>;
}

/// # Safety
///
/// * [`CompactString`] does not contain any lifetime
/// * [`CompactString`] is 'static
/// * [`CompactString`] is a container to `u8`, which is `LifetimeFree`.
unsafe impl LifetimeFree for CompactString {}
unsafe impl LifetimeFree for Repr {}

/// # Panics
///
/// In this implementation, the `to_compact_string` method panics if the `Display` implementation
/// returns an error. This indicates an incorrect `Display` implementation since
/// `std::fmt::Write for CompactString` never returns an error itself.
///
/// # Note
///
/// We use the [`castaway`] crate to provide zero-cost specialization for several types, those are:
/// * `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
/// * `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
/// * `NonZeroU*`, `NonZeroI*`
/// * `bool`
/// * `char`
/// * `String`, `CompactString`
/// * `f32`, `f64`
///     * For floats we use [`ryu`] crate which sometimes provides different formatting than [`std`]
impl<T: fmt::Display> ToCompactString for T {
    #[inline]
    fn try_to_compact_string(&self) -> Result<CompactString, ToCompactStringError> {
        let repr = match_type!(self, {
            &u8 as s => s.into_repr()?,
            &i8 as s => s.into_repr()?,
            &u16 as s => s.into_repr()?,
            &i16 as s => s.into_repr()?,
            &u32 as s => s.into_repr()?,
            &i32 as s => s.into_repr()?,
            &u64 as s => s.into_repr()?,
            &i64 as s => s.into_repr()?,
            &u128 as s => s.into_repr()?,
            &i128 as s => s.into_repr()?,
            &usize as s => s.into_repr()?,
            &isize as s => s.into_repr()?,
            &f32 as s => s.into_repr()?,
            &f64 as s => s.into_repr()?,
            &bool as s => s.into_repr()?,
            &char as s => s.into_repr()?,
            &String as s => Repr::new(s)?,
            &CompactString as s => Repr::new(s)?,
            &num::NonZeroU8 as s => s.into_repr()?,
            &num::NonZeroI8 as s => s.into_repr()?,
            &num::NonZeroU16 as s => s.into_repr()?,
            &num::NonZeroI16 as s => s.into_repr()?,
            &num::NonZeroU32 as s => s.into_repr()?,
            &num::NonZeroI32 as s => s.into_repr()?,
            &num::NonZeroU64 as s => s.into_repr()?,
            &num::NonZeroI64 as s => s.into_repr()?,
            &num::NonZeroUsize as s => s.into_repr()?,
            &num::NonZeroIsize as s => s.into_repr()?,
            &num::NonZeroU128 as s => s.into_repr()?,
            &num::NonZeroI128 as s => s.into_repr()?,
            s => {
                let mut c = CompactString::const_new("");
                write!(c, "{}", s)?;
                return Ok(c);
            }
        });

        Ok(CompactString(repr))
    }
}

/// A trait that provides convenience methods for creating a [`CompactString`] from a collection of
/// items. It is implemented for all types that can be converted into an iterator, and that iterator
/// yields types that can be converted into a `str`.
///
/// i.e. `C: IntoIterator<Item = AsRef<str>>`.
///
/// # Concatenate and Join
/// Two methods that this trait provides are `concat_compact(...)` and `join_compact(...)`
/// ```
/// use compact_str::CompactStringExt;
///
/// let words = vec!["☀️", "🌕", "🌑", "☀️"];
///
/// // directly concatenate all the words together
/// let concat = words.concat_compact();
/// assert_eq!(concat, "☀️🌕🌑☀️");
///
/// // join the words, with a separator
/// let join = words.join_compact(" ➡️ ");
/// assert_eq!(join, "☀️ ➡️ 🌕 ➡️ 🌑 ➡️ ☀️");
/// ```
pub trait CompactStringExt {
    /// Concatenates all the items of a collection into a [`CompactString`]
    ///
    /// # Example
    /// ```
    /// use compact_str::CompactStringExt;
    ///
    /// let items = ["hello", " ", "world", "!"];
    /// let compact = items.concat_compact();
    ///
    /// assert_eq!(compact, "hello world!");
    /// ```
    fn concat_compact(&self) -> CompactString;

    /// Joins all the items of a collection, placing a separator between them, forming a
    /// [`CompactString`]
    ///
    /// # Example
    /// ```
    /// use compact_str::CompactStringExt;
    ///
    /// let fruits = vec!["apples", "oranges", "bananas"];
    /// let compact = fruits.join_compact(", ");
    ///
    /// assert_eq!(compact, "apples, oranges, bananas");
    /// ```
    fn join_compact<S: AsRef<str>>(&self, separator: S) -> CompactString;
}

impl<I, C> CompactStringExt for C
where
    I: AsRef<str>,
    for<'a> &'a C: IntoIterator<Item = &'a I>,
{
    fn concat_compact(&self) -> CompactString {
        self.into_iter()
            .fold(CompactString::const_new(""), |mut s, item| {
                s.push_str(item.as_ref());
                s
            })
    }

    fn join_compact<S: AsRef<str>>(&self, separator: S) -> CompactString {
        let mut compact_string = CompactString::const_new("");

        let mut iter = self.into_iter().peekable();
        let sep = separator.as_ref();

        while let Some(item) = iter.next() {
            compact_string.push_str(item.as_ref());
            if iter.peek().is_some() {
                compact_string.push_str(sep);
            }
        }

        compact_string
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{
        String,
        ToString,
    };
    use alloc::vec::Vec;
    use core::num;

    use proptest::prelude::*;
    use test_strategy::proptest;

    use super::{
        CompactStringExt,
        ToCompactString,
    };
    use crate::CompactString;

    #[test]
    fn test_join() {
        let slice = ["hello", "world"];
        let c = slice.join_compact(" ");
        assert_eq!(c, "hello world");

        let vector = vec!["🍎", "🍊", "🍌"];
        let c = vector.join_compact(",");
        assert_eq!(c, "🍎,🍊,🍌");
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_join(items: Vec<String>, separator: String) {
        let c: CompactString = items.join_compact(&separator);
        let s: String = items.join(&separator);
        assert_eq!(c, s);
    }

    #[test]
    fn test_concat() {
        let items = vec!["hello", "world"];
        let c = items.join_compact(" ");
        assert_eq!(c, "hello world");

        let vector = vec!["🍎", "🍊", "🍌"];
        let c = vector.concat_compact();
        assert_eq!(c, "🍎🍊🍌");
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_concat(items: Vec<String>) {
        let c: CompactString = items.concat_compact();
        let s: String = items.concat();
        assert_eq!(c, s);
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_u8(val: u8) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_i8(val: i8) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_u16(val: u16) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_i16(val: i16) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_u32(val: u32) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_i32(val: i32) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_u64(val: u64) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_i64(val: i64) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_usize(val: usize) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_isize(val: isize) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_u128(val: u128) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_i128(val: i128) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_u8(
        #[strategy((1..=u8::MAX).prop_map(|x| unsafe { num::NonZeroU8::new_unchecked(x)} ))]
        val: num::NonZeroU8,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_u16(
        #[strategy((1..=u16::MAX).prop_map(|x| unsafe { num::NonZeroU16::new_unchecked(x)} ))]
        val: num::NonZeroU16,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_u32(
        #[strategy((1..=u32::MAX).prop_map(|x| unsafe { num::NonZeroU32::new_unchecked(x)} ))]
        val: num::NonZeroU32,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_u64(
        #[strategy((1..=u64::MAX).prop_map(|x| unsafe { num::NonZeroU64::new_unchecked(x)} ))]
        val: num::NonZeroU64,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_u128(
        #[strategy((1..=u128::MAX).prop_map(|x| unsafe { num::NonZeroU128::new_unchecked(x)} ))]
        val: num::NonZeroU128,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_usize(
        #[strategy((1..=usize::MAX).prop_map(|x| unsafe { num::NonZeroUsize::new_unchecked(x)} ))]
        val: num::NonZeroUsize,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_i8(
        #[strategy((1..=u8::MAX).prop_map(|x| unsafe { num::NonZeroI8::new_unchecked(x as i8)} ))]
        val: num::NonZeroI8,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_i16(
        #[strategy((1..=u16::MAX).prop_map(|x| unsafe { num::NonZeroI16::new_unchecked(x as i16)} ))]
        val: num::NonZeroI16,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_i32(
        #[strategy((1..=u32::MAX).prop_map(|x| unsafe { num::NonZeroI32::new_unchecked(x as i32)} ))]
        val: num::NonZeroI32,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_i64(
        #[strategy((1..=u64::MAX).prop_map(|x| unsafe { num::NonZeroI64::new_unchecked(x as i64)} ))]
        val: num::NonZeroI64,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_i128(
        #[strategy((1..=u128::MAX).prop_map(|x| unsafe { num::NonZeroI128::new_unchecked(x as i128)} ))]
        val: num::NonZeroI128,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_to_compact_string_non_zero_isize(
        #[strategy((1..=usize::MAX).prop_map(|x| unsafe { num::NonZeroIsize::new_unchecked(x as isize)} ))]
        val: num::NonZeroIsize,
    ) {
        let compact = val.to_compact_string();
        prop_assert_eq!(compact.as_str(), val.to_string());
    }
}
