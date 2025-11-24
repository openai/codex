use std::borrow::{Borrow, Cow};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::from_utf8;

const MAX_INLINE_STR_LEN: usize = 3 * std::mem::size_of::<isize>() - 2;

/// Returned when trying to convert a `&str` into a `InlineStr`
/// but it fails because it doesn't fit.
#[derive(Debug)]
pub struct StringTooLongError;

/// An inline string that can contain almost three words
/// of utf-8 text.
#[derive(Debug, Clone, Copy, Eq)]
pub struct InlineStr {
    inner: [u8; MAX_INLINE_STR_LEN],
    len: u8,
}

impl AsRef<str> for InlineStr {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl Hash for InlineStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl From<char> for InlineStr {
    fn from(c: char) -> Self {
        let mut inner = [0u8; MAX_INLINE_STR_LEN];
        c.encode_utf8(&mut inner);
        let len = c.len_utf8() as u8;
        Self { inner, len }
    }
}

impl std::cmp::PartialEq<InlineStr> for InlineStr {
    fn eq(&self, other: &InlineStr) -> bool {
        self.deref() == other.deref()
    }
}

impl TryFrom<&str> for InlineStr {
    type Error = StringTooLongError;

    fn try_from(s: &str) -> Result<InlineStr, StringTooLongError> {
        let len = s.len();
        if len <= MAX_INLINE_STR_LEN {
            let mut inner = [0u8; MAX_INLINE_STR_LEN];
            inner[..len].copy_from_slice(s.as_bytes());
            let len = len as u8;
            Ok(Self { inner, len })
        } else {
            Err(StringTooLongError)
        }
    }
}

impl Deref for InlineStr {
    type Target = str;

    fn deref(&self) -> &str {
        let len = self.len as usize;
        from_utf8(&self.inner[..len]).unwrap()
    }
}

impl fmt::Display for InlineStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// A copy-on-write string that can be owned, borrowed
/// or inlined.
///
/// It is three words long.
#[derive(Debug, Eq)]
pub enum CowStr<'a> {
    /// An owned, immutable string.
    Boxed(Box<str>),
    /// A borrowed string.
    Borrowed(&'a str),
    /// A short inline string.
    Inlined(InlineStr),
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::CowStr;
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
    use std::fmt;

    impl<'a> Serialize for CowStr<'a> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.as_ref())
        }
    }

    struct CowStrVisitor;

    impl<'de> de::Visitor<'de> for CowStrVisitor {
        type Value = CowStr<'de>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(CowStr::Borrowed(v))
        }
    }

    impl<'a, 'de: 'a> Deserialize<'de> for CowStr<'a> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(CowStrVisitor)
        }
    }
}

impl<'a> AsRef<str> for CowStr<'a> {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl<'a> Hash for CowStr<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<'a> std::clone::Clone for CowStr<'a> {
    fn clone(&self) -> Self {
        match self {
            CowStr::Boxed(s) => match InlineStr::try_from(&**s) {
                Ok(inline) => CowStr::Inlined(inline),
                Err(..) => CowStr::Boxed(s.clone()),
            },
            CowStr::Borrowed(s) => CowStr::Borrowed(s),
            CowStr::Inlined(s) => CowStr::Inlined(*s),
        }
    }
}

impl<'a> std::cmp::PartialEq<CowStr<'a>> for CowStr<'a> {
    fn eq(&self, other: &CowStr<'_>) -> bool {
        self.deref() == other.deref()
    }
}

impl<'a> From<&'a str> for CowStr<'a> {
    fn from(s: &'a str) -> Self {
        CowStr::Borrowed(s)
    }
}

impl<'a> From<String> for CowStr<'a> {
    fn from(s: String) -> Self {
        CowStr::Boxed(s.into_boxed_str())
    }
}

impl<'a> From<char> for CowStr<'a> {
    fn from(c: char) -> Self {
        CowStr::Inlined(c.into())
    }
}

impl<'a> From<Cow<'a, str>> for CowStr<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        match s {
            Cow::Borrowed(s) => CowStr::Borrowed(s),
            Cow::Owned(s) => CowStr::Boxed(s.into_boxed_str()),
        }
    }
}

impl<'a> From<CowStr<'a>> for Cow<'a, str> {
    fn from(s: CowStr<'a>) -> Self {
        match s {
            CowStr::Boxed(s) => Cow::Owned(s.to_string()),
            CowStr::Inlined(s) => Cow::Owned(s.to_string()),
            CowStr::Borrowed(s) => Cow::Borrowed(s),
        }
    }
}

impl<'a> From<Cow<'a, char>> for CowStr<'a> {
    fn from(s: Cow<'a, char>) -> Self {
        CowStr::Inlined(InlineStr::from(*s))
    }
}

impl<'a> Deref for CowStr<'a> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            CowStr::Boxed(ref b) => b,
            CowStr::Borrowed(b) => b,
            CowStr::Inlined(ref s) => s.deref(),
        }
    }
}

impl<'a> Borrow<str> for CowStr<'a> {
    fn borrow(&self) -> &str {
        self.deref()
    }
}

impl<'a> CowStr<'a> {
    pub fn into_string(self) -> String {
        match self {
            CowStr::Boxed(b) => b.into(),
            CowStr::Borrowed(b) => b.to_owned(),
            CowStr::Inlined(s) => s.deref().to_owned(),
        }
    }
}

impl<'a> fmt::Display for CowStr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[cfg(test)]
mod test_special_string {
    use super::*;

    #[test]
    fn inlinestr_ascii() {
        let s: InlineStr = 'a'.into();
        assert_eq!("a", s.deref());
    }

    #[test]
    fn inlinestr_unicode() {
        let s: InlineStr = '🍔'.into();
        assert_eq!("🍔", s.deref());
    }

    #[test]
    fn cowstr_size() {
        let size = std::mem::size_of::<CowStr>();
        let word_size = std::mem::size_of::<isize>();
        assert_eq!(3 * word_size, size);
    }

    #[test]
    fn cowstr_char_to_string() {
        let c = '藏';
        let smort: CowStr = c.into();
        let owned: String = smort.to_string();
        let expected = "藏".to_owned();
        assert_eq!(expected, owned);
    }

    #[test]
    fn max_inline_str_len_atleast_four() {
        // we need 4 bytes to store a char
        assert!(MAX_INLINE_STR_LEN >= 4);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn inlinestr_fits_twentytwo() {
        let s = "0123456789abcdefghijkl";
        let stack_str = InlineStr::try_from(s).unwrap();
        assert_eq!(stack_str.deref(), s);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn inlinestr_not_fits_twentythree() {
        let s = "0123456789abcdefghijklm";
        let _stack_str = InlineStr::try_from(s).unwrap_err();
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn small_boxed_str_clones_to_stack() {
        let s = "0123456789abcde".to_owned();
        let smort: CowStr = s.into();
        let smort_clone = smort.clone();

        if let CowStr::Inlined(..) = smort_clone {
        } else {
            panic!("Expected a Inlined variant!");
        }
    }

    #[test]
    fn cow_to_cow_str() {
        let s = "some text";
        let cow = Cow::Borrowed(s);
        let actual = CowStr::from(cow);
        let expected = CowStr::Borrowed(s);
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "some text".to_string();
        let cow: Cow<str> = Cow::Owned(s.clone());
        let actual = CowStr::from(cow);
        let expected = CowStr::Boxed(s.into_boxed_str());
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    #[test]
    fn cow_str_to_cow() {
        let s = "some text";
        let cow_str = CowStr::Borrowed(s);
        let actual = Cow::from(cow_str);
        let expected = Cow::Borrowed(s);
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "s";
        let inline_str: InlineStr = InlineStr::try_from(s).unwrap();
        let cow_str = CowStr::Inlined(inline_str);
        let actual = Cow::from(cow_str);
        let expected: Cow<str> = Cow::Owned(s.to_string());
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "s";
        let cow_str = CowStr::Boxed(s.to_string().into_boxed_str());
        let actual = Cow::from(cow_str);
        let expected: Cow<str> = Cow::Owned(s.to_string());
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    #[test]
    fn cow_char_to_cow_str() {
        let c = 'c';
        let cow: Cow<char> = Cow::Owned(c);
        let actual = CowStr::from(cow);
        let expected = CowStr::Inlined(InlineStr::from(c));
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let c = 'c';
        let cow: Cow<char> = Cow::Borrowed(&c);
        let actual = CowStr::from(cow);
        let expected = CowStr::Inlined(InlineStr::from(c));
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    fn variant_eq<T>(a: &T, b: &T) -> bool {
        std::mem::discriminant(a) == std::mem::discriminant(b)
    }
}
