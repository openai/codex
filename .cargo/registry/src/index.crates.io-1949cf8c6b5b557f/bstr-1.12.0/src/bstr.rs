#[cfg(feature = "alloc")]
use alloc::boxed::Box;

/// A wrapper for `&[u8]` that provides convenient string oriented trait impls.
///
/// If you need ownership or a growable byte string buffer, then use
/// [`BString`](struct.BString.html).
///
/// Using a `&BStr` is just like using a `&[u8]`, since `BStr`
/// implements `Deref` to `[u8]`. So all methods available on `[u8]`
/// are also available on `BStr`.
///
/// # Representation
///
/// A `&BStr` has the same representation as a `&str`. That is, a `&BStr` is
/// a fat pointer which consists of a pointer to some bytes and a length.
///
/// # Trait implementations
///
/// The `BStr` type has a number of trait implementations, and in particular,
/// defines equality and ordinal comparisons between `&BStr`, `&str` and
/// `&[u8]` for convenience.
///
/// The `Debug` implementation for `BStr` shows its bytes as a normal string.
/// For invalid UTF-8, hex escape sequences are used.
///
/// The `Display` implementation behaves as if `BStr` were first lossily
/// converted to a `str`. Invalid UTF-8 bytes are substituted with the Unicode
/// replacement codepoint, which looks like this: �.
#[repr(transparent)]
pub struct BStr {
    pub(crate) bytes: [u8],
}

impl BStr {
    /// Directly creates a `BStr` slice from anything that can be converted
    /// to a byte slice.
    ///
    /// This is very similar to the [`B`](crate::B) function, except this
    /// returns a `&BStr` instead of a `&[u8]`.
    ///
    /// This is a cost-free conversion.
    ///
    /// # Example
    ///
    /// You can create `BStr`'s from byte arrays, byte slices or even string
    /// slices:
    ///
    /// ```
    /// use bstr::BStr;
    ///
    /// let a = BStr::new(b"abc");
    /// let b = BStr::new(&b"abc"[..]);
    /// let c = BStr::new("abc");
    ///
    /// assert_eq!(a, b);
    /// assert_eq!(a, c);
    /// ```
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(bytes: &B) -> &BStr {
        BStr::from_bytes(bytes.as_ref())
    }

    #[inline]
    pub(crate) fn new_mut<B: ?Sized + AsMut<[u8]>>(
        bytes: &mut B,
    ) -> &mut BStr {
        BStr::from_bytes_mut(bytes.as_mut())
    }

    #[inline]
    pub(crate) fn from_bytes(slice: &[u8]) -> &BStr {
        unsafe { &*(slice as *const [u8] as *const BStr) }
    }

    #[inline]
    pub(crate) fn from_bytes_mut(slice: &mut [u8]) -> &mut BStr {
        unsafe { &mut *(slice as *mut [u8] as *mut BStr) }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub(crate) fn from_boxed_bytes(slice: Box<[u8]>) -> Box<BStr> {
        unsafe { Box::from_raw(Box::into_raw(slice) as _) }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub(crate) fn into_boxed_bytes(slice: Box<BStr>) -> Box<[u8]> {
        unsafe { Box::from_raw(Box::into_raw(slice) as _) }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
