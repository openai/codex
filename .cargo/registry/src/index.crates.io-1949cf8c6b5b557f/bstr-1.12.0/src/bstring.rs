use alloc::vec::Vec;

use crate::bstr::BStr;

/// A wrapper for `Vec<u8>` that provides convenient string oriented trait
/// impls.
///
/// A `BString` has ownership over its contents and corresponds to
/// a growable or shrinkable buffer. Its borrowed counterpart is a
/// [`BStr`](struct.BStr.html), called a byte string slice.
///
/// Using a `BString` is just like using a `Vec<u8>`, since `BString`
/// implements `Deref` to `Vec<u8>`. So all methods available on `Vec<u8>`
/// are also available on `BString`.
///
/// # Examples
///
/// You can create a new `BString` from a `Vec<u8>` via a `From` impl:
///
/// ```
/// use bstr::BString;
///
/// let s = BString::from("Hello, world!");
/// ```
///
/// # Deref
///
/// The `BString` type implements `Deref` and `DerefMut`, where the target
/// types are `&Vec<u8>` and `&mut Vec<u8>`, respectively. `Deref` permits all of the
/// methods defined on `Vec<u8>` to be implicitly callable on any `BString`.
///
/// For more information about how deref works, see the documentation for the
/// [`std::ops::Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html)
/// trait.
///
/// # Representation
///
/// A `BString` has the same representation as a `Vec<u8>` and a `String`.
/// That is, it is made up of three word sized components: a pointer to a
/// region of memory containing the bytes, a length and a capacity.
#[derive(Clone)]
pub struct BString {
    bytes: Vec<u8>,
}

impl BString {
    /// Constructs a new `BString` from the given [`Vec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use bstr::BString;
    ///
    /// let mut b = BString::new(Vec::with_capacity(10));
    /// ```
    ///
    /// This function is `const`:
    ///
    /// ```
    /// use bstr::BString;
    ///
    /// const B: BString = BString::new(vec![]);
    /// ```
    #[inline]
    pub const fn new(bytes: Vec<u8>) -> BString {
        BString { bytes }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    pub(crate) fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    #[inline]
    pub(crate) fn as_bstr(&self) -> &BStr {
        BStr::new(&self.bytes)
    }

    #[inline]
    pub(crate) fn as_mut_bstr(&mut self) -> &mut BStr {
        BStr::new_mut(&mut self.bytes)
    }

    #[inline]
    pub(crate) fn as_vec(&self) -> &Vec<u8> {
        &self.bytes
    }

    #[inline]
    pub(crate) fn as_vec_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }

    #[inline]
    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
}
