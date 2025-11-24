macro_rules! impl_partial_eq {
    ($lhs:ty, $rhs:ty) => {
        impl<'a> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let other: &[u8] = other.as_ref();
                PartialEq::eq(self.as_bytes(), other)
            }
        }

        impl<'a> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                let this: &[u8] = self.as_ref();
                PartialEq::eq(this, other.as_bytes())
            }
        }
    };
}

macro_rules! impl_partial_eq_n {
    ($lhs:ty, $rhs:ty) => {
        impl<'a, const N: usize> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let other: &[u8] = other.as_ref();
                PartialEq::eq(self.as_bytes(), other)
            }
        }

        impl<'a, const N: usize> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                let this: &[u8] = self.as_ref();
                PartialEq::eq(this, other.as_bytes())
            }
        }
    };
}

#[cfg(feature = "alloc")]
macro_rules! impl_partial_eq_cow {
    ($lhs:ty, $rhs:ty) => {
        impl<'a> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let other: &[u8] = (&**other).as_ref();
                PartialEq::eq(self.as_bytes(), other)
            }
        }

        impl<'a> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                let this: &[u8] = (&**self).as_ref();
                PartialEq::eq(this, other.as_bytes())
            }
        }
    };
}

macro_rules! impl_partial_ord {
    ($lhs:ty, $rhs:ty) => {
        impl<'a> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<Ordering> {
                let other: &[u8] = other.as_ref();
                PartialOrd::partial_cmp(self.as_bytes(), other)
            }
        }

        impl<'a> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<Ordering> {
                let this: &[u8] = self.as_ref();
                PartialOrd::partial_cmp(this, other.as_bytes())
            }
        }
    };
}

macro_rules! impl_partial_ord_n {
    ($lhs:ty, $rhs:ty) => {
        impl<'a, const N: usize> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<Ordering> {
                let other: &[u8] = other.as_ref();
                PartialOrd::partial_cmp(self.as_bytes(), other)
            }
        }

        impl<'a, const N: usize> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<Ordering> {
                let this: &[u8] = self.as_ref();
                PartialOrd::partial_cmp(this, other.as_bytes())
            }
        }
    };
}

#[cfg(feature = "alloc")]
mod bstring {
    use core::{cmp::Ordering, fmt, hash, ops, str::FromStr};

    use alloc::{
        borrow::{Borrow, BorrowMut, Cow, ToOwned},
        string::String,
        vec,
        vec::Vec,
    };

    use crate::{
        bstr::BStr, bstring::BString, ext_slice::ByteSlice, ext_vec::ByteVec,
    };

    impl fmt::Display for BString {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(self.as_bstr(), f)
        }
    }

    impl fmt::Debug for BString {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Debug::fmt(self.as_bstr(), f)
        }
    }

    impl FromStr for BString {
        type Err = crate::Utf8Error;

        #[inline]
        fn from_str(s: &str) -> Result<BString, crate::Utf8Error> {
            Ok(BString::from(s))
        }
    }

    impl ops::Deref for BString {
        type Target = Vec<u8>;

        #[inline]
        fn deref(&self) -> &Vec<u8> {
            self.as_vec()
        }
    }

    impl ops::DerefMut for BString {
        #[inline]
        fn deref_mut(&mut self) -> &mut Vec<u8> {
            self.as_vec_mut()
        }
    }

    impl AsRef<[u8]> for BString {
        #[inline]
        fn as_ref(&self) -> &[u8] {
            self.as_bytes()
        }
    }

    impl AsRef<BStr> for BString {
        #[inline]
        fn as_ref(&self) -> &BStr {
            self.as_bstr()
        }
    }

    impl AsMut<[u8]> for BString {
        #[inline]
        fn as_mut(&mut self) -> &mut [u8] {
            self.as_bytes_mut()
        }
    }

    impl AsMut<BStr> for BString {
        #[inline]
        fn as_mut(&mut self) -> &mut BStr {
            self.as_mut_bstr()
        }
    }

    impl Borrow<[u8]> for BString {
        #[inline]
        fn borrow(&self) -> &[u8] {
            self.as_bytes()
        }
    }

    impl Borrow<BStr> for BString {
        #[inline]
        fn borrow(&self) -> &BStr {
            self.as_bstr()
        }
    }

    impl Borrow<BStr> for Vec<u8> {
        #[inline]
        fn borrow(&self) -> &BStr {
            self.as_slice().as_bstr()
        }
    }

    impl Borrow<BStr> for String {
        #[inline]
        fn borrow(&self) -> &BStr {
            self.as_bytes().as_bstr()
        }
    }

    impl BorrowMut<[u8]> for BString {
        #[inline]
        fn borrow_mut(&mut self) -> &mut [u8] {
            self.as_bytes_mut()
        }
    }

    impl BorrowMut<BStr> for BString {
        #[inline]
        fn borrow_mut(&mut self) -> &mut BStr {
            self.as_mut_bstr()
        }
    }

    impl BorrowMut<BStr> for Vec<u8> {
        #[inline]
        fn borrow_mut(&mut self) -> &mut BStr {
            BStr::new_mut(self.as_mut_slice())
        }
    }

    impl ToOwned for BStr {
        type Owned = BString;

        #[inline]
        fn to_owned(&self) -> BString {
            BString::from(self)
        }
    }

    impl Default for BString {
        fn default() -> BString {
            BString::from(vec![])
        }
    }

    impl<'a, const N: usize> From<&'a [u8; N]> for BString {
        #[inline]
        fn from(s: &'a [u8; N]) -> BString {
            BString::from(&s[..])
        }
    }

    impl<const N: usize> From<[u8; N]> for BString {
        #[inline]
        fn from(s: [u8; N]) -> BString {
            BString::from(&s[..])
        }
    }

    impl<'a> From<&'a [u8]> for BString {
        #[inline]
        fn from(s: &'a [u8]) -> BString {
            BString::from(s.to_vec())
        }
    }

    impl From<Vec<u8>> for BString {
        #[inline]
        fn from(s: Vec<u8>) -> BString {
            BString::new(s)
        }
    }

    impl From<BString> for Vec<u8> {
        #[inline]
        fn from(s: BString) -> Vec<u8> {
            s.into_vec()
        }
    }

    impl<'a> From<&'a str> for BString {
        #[inline]
        fn from(s: &'a str) -> BString {
            BString::from(s.as_bytes().to_vec())
        }
    }

    impl From<String> for BString {
        #[inline]
        fn from(s: String) -> BString {
            BString::from(s.into_bytes())
        }
    }

    impl<'a> From<&'a BStr> for BString {
        #[inline]
        fn from(s: &'a BStr) -> BString {
            BString::from(s.bytes.to_vec())
        }
    }

    impl<'a> From<BString> for Cow<'a, BStr> {
        #[inline]
        fn from(s: BString) -> Cow<'a, BStr> {
            Cow::Owned(s)
        }
    }

    impl<'a> From<&'a BString> for Cow<'a, BStr> {
        #[inline]
        fn from(s: &'a BString) -> Cow<'a, BStr> {
            Cow::Borrowed(s.as_bstr())
        }
    }

    impl TryFrom<BString> for String {
        type Error = crate::FromUtf8Error;

        #[inline]
        fn try_from(s: BString) -> Result<String, crate::FromUtf8Error> {
            s.into_vec().into_string()
        }
    }

    impl<'a> TryFrom<&'a BString> for &'a str {
        type Error = crate::Utf8Error;

        #[inline]
        fn try_from(s: &'a BString) -> Result<&'a str, crate::Utf8Error> {
            s.as_bytes().to_str()
        }
    }

    impl FromIterator<char> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> BString {
            BString::from(iter.into_iter().collect::<String>())
        }
    }

    impl FromIterator<u8> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> BString {
            BString::from(iter.into_iter().collect::<Vec<u8>>())
        }
    }

    impl<'a> FromIterator<&'a str> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> BString {
            let mut buf = vec![];
            for b in iter {
                buf.push_str(b);
            }
            BString::from(buf)
        }
    }

    impl<'a> FromIterator<&'a [u8]> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = &'a [u8]>>(iter: T) -> BString {
            let mut buf = vec![];
            for b in iter {
                buf.push_str(b);
            }
            BString::from(buf)
        }
    }

    impl<'a> FromIterator<&'a BStr> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = &'a BStr>>(iter: T) -> BString {
            let mut buf = vec![];
            for b in iter {
                buf.push_str(b);
            }
            BString::from(buf)
        }
    }

    impl FromIterator<BString> for BString {
        #[inline]
        fn from_iter<T: IntoIterator<Item = BString>>(iter: T) -> BString {
            let mut buf = vec![];
            for b in iter {
                buf.push_str(b);
            }
            BString::from(buf)
        }
    }

    impl Eq for BString {}

    impl PartialEq for BString {
        #[inline]
        fn eq(&self, other: &BString) -> bool {
            self[..] == other[..]
        }
    }

    impl_partial_eq!(BString, Vec<u8>);
    impl_partial_eq!(BString, [u8]);
    impl_partial_eq!(BString, &'a [u8]);
    impl_partial_eq!(BString, String);
    impl_partial_eq!(BString, str);
    impl_partial_eq!(BString, &'a str);
    impl_partial_eq!(BString, BStr);
    impl_partial_eq!(BString, &'a BStr);
    impl_partial_eq_n!(BString, [u8; N]);
    impl_partial_eq_n!(BString, &'a [u8; N]);

    impl hash::Hash for BString {
        #[inline]
        fn hash<H: hash::Hasher>(&self, state: &mut H) {
            self.as_bytes().hash(state);
        }
    }

    impl PartialOrd for BString {
        #[inline]
        fn partial_cmp(&self, other: &BString) -> Option<Ordering> {
            PartialOrd::partial_cmp(self.as_bytes(), other.as_bytes())
        }
    }

    impl Ord for BString {
        #[inline]
        fn cmp(&self, other: &BString) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }

    impl_partial_ord!(BString, Vec<u8>);
    impl_partial_ord!(BString, [u8]);
    impl_partial_ord!(BString, &'a [u8]);
    impl_partial_ord!(BString, String);
    impl_partial_ord!(BString, str);
    impl_partial_ord!(BString, &'a str);
    impl_partial_ord!(BString, BStr);
    impl_partial_ord!(BString, &'a BStr);
    impl_partial_ord_n!(BString, [u8; N]);
    impl_partial_ord_n!(BString, &'a [u8; N]);
}

mod bstr {
    use core::{
        borrow::{Borrow, BorrowMut},
        cmp::Ordering,
        fmt, hash, ops,
    };

    #[cfg(feature = "alloc")]
    use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};

    use crate::{bstr::BStr, ext_slice::ByteSlice};

    impl fmt::Display for BStr {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            /// Write the given bstr (lossily) to the given formatter.
            fn write_bstr(
                f: &mut fmt::Formatter<'_>,
                bstr: &BStr,
            ) -> Result<(), fmt::Error> {
                for chunk in bstr.utf8_chunks() {
                    f.write_str(chunk.valid())?;
                    if !chunk.invalid().is_empty() {
                        f.write_str("\u{FFFD}")?;
                    }
                }
                Ok(())
            }

            /// Write 'num' fill characters to the given formatter.
            fn write_pads(
                f: &mut fmt::Formatter<'_>,
                num: usize,
            ) -> fmt::Result {
                let fill = f.fill();
                for _ in 0..num {
                    f.write_fmt(format_args!("{}", fill))?;
                }
                Ok(())
            }

            if let Some(align) = f.align() {
                let width = f.width().unwrap_or(0);
                let nchars = self.chars().count();
                let remaining_pads = width.saturating_sub(nchars);
                match align {
                    fmt::Alignment::Left => {
                        write_bstr(f, self)?;
                        write_pads(f, remaining_pads)?;
                    }
                    fmt::Alignment::Right => {
                        write_pads(f, remaining_pads)?;
                        write_bstr(f, self)?;
                    }
                    fmt::Alignment::Center => {
                        let half = remaining_pads / 2;
                        let second_half = if remaining_pads % 2 == 0 {
                            half
                        } else {
                            half + 1
                        };
                        write_pads(f, half)?;
                        write_bstr(f, self)?;
                        write_pads(f, second_half)?;
                    }
                }
                Ok(())
            } else {
                write_bstr(f, self)?;
                Ok(())
            }
        }
    }

    impl fmt::Debug for BStr {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "\"")?;
            for (s, e, ch) in self.char_indices() {
                match ch {
                    '\0' => write!(f, "\\0")?,
                    '\x01'..='\x7f' => {
                        write!(f, "{}", (ch as u8).escape_ascii())?;
                    }
                    '\u{FFFD}' => {
                        let bytes = self[s..e].as_bytes();
                        if bytes == b"\xEF\xBF\xBD" {
                            write!(f, "{}", ch.escape_debug())?;
                        } else {
                            for &b in self[s..e].as_bytes() {
                                write!(f, "\\x{:02x}", b)?;
                            }
                        }
                    }
                    _ => {
                        write!(f, "{}", ch.escape_debug())?;
                    }
                }
            }
            write!(f, "\"")?;
            Ok(())
        }
    }

    impl ops::Deref for BStr {
        type Target = [u8];

        #[inline]
        fn deref(&self) -> &[u8] {
            &self.bytes
        }
    }

    impl ops::DerefMut for BStr {
        #[inline]
        fn deref_mut(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    impl ops::Index<usize> for BStr {
        type Output = u8;

        #[inline]
        fn index(&self, idx: usize) -> &u8 {
            &self.as_bytes()[idx]
        }
    }

    impl ops::Index<ops::RangeFull> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, _: ops::RangeFull) -> &BStr {
            self
        }
    }

    impl ops::Index<ops::Range<usize>> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, r: ops::Range<usize>) -> &BStr {
            BStr::new(&self.as_bytes()[r.start..r.end])
        }
    }

    impl ops::Index<ops::RangeInclusive<usize>> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, r: ops::RangeInclusive<usize>) -> &BStr {
            BStr::new(&self.as_bytes()[*r.start()..=*r.end()])
        }
    }

    impl ops::Index<ops::RangeFrom<usize>> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, r: ops::RangeFrom<usize>) -> &BStr {
            BStr::new(&self.as_bytes()[r.start..])
        }
    }

    impl ops::Index<ops::RangeTo<usize>> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, r: ops::RangeTo<usize>) -> &BStr {
            BStr::new(&self.as_bytes()[..r.end])
        }
    }

    impl ops::Index<ops::RangeToInclusive<usize>> for BStr {
        type Output = BStr;

        #[inline]
        fn index(&self, r: ops::RangeToInclusive<usize>) -> &BStr {
            BStr::new(&self.as_bytes()[..=r.end])
        }
    }

    impl ops::IndexMut<usize> for BStr {
        #[inline]
        fn index_mut(&mut self, idx: usize) -> &mut u8 {
            &mut self.bytes[idx]
        }
    }

    impl ops::IndexMut<ops::RangeFull> for BStr {
        #[inline]
        fn index_mut(&mut self, _: ops::RangeFull) -> &mut BStr {
            self
        }
    }

    impl ops::IndexMut<ops::Range<usize>> for BStr {
        #[inline]
        fn index_mut(&mut self, r: ops::Range<usize>) -> &mut BStr {
            BStr::from_bytes_mut(&mut self.bytes[r.start..r.end])
        }
    }

    impl ops::IndexMut<ops::RangeInclusive<usize>> for BStr {
        #[inline]
        fn index_mut(&mut self, r: ops::RangeInclusive<usize>) -> &mut BStr {
            BStr::from_bytes_mut(&mut self.bytes[*r.start()..=*r.end()])
        }
    }

    impl ops::IndexMut<ops::RangeFrom<usize>> for BStr {
        #[inline]
        fn index_mut(&mut self, r: ops::RangeFrom<usize>) -> &mut BStr {
            BStr::from_bytes_mut(&mut self.bytes[r.start..])
        }
    }

    impl ops::IndexMut<ops::RangeTo<usize>> for BStr {
        #[inline]
        fn index_mut(&mut self, r: ops::RangeTo<usize>) -> &mut BStr {
            BStr::from_bytes_mut(&mut self.bytes[..r.end])
        }
    }

    impl ops::IndexMut<ops::RangeToInclusive<usize>> for BStr {
        #[inline]
        fn index_mut(&mut self, r: ops::RangeToInclusive<usize>) -> &mut BStr {
            BStr::from_bytes_mut(&mut self.bytes[..=r.end])
        }
    }

    impl AsRef<[u8]> for BStr {
        #[inline]
        fn as_ref(&self) -> &[u8] {
            self.as_bytes()
        }
    }

    impl AsRef<BStr> for BStr {
        #[inline]
        fn as_ref(&self) -> &BStr {
            self
        }
    }

    impl AsRef<BStr> for [u8] {
        #[inline]
        fn as_ref(&self) -> &BStr {
            BStr::new(self)
        }
    }

    impl AsRef<BStr> for str {
        #[inline]
        fn as_ref(&self) -> &BStr {
            BStr::new(self)
        }
    }

    impl AsMut<[u8]> for BStr {
        #[inline]
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    impl AsMut<BStr> for [u8] {
        #[inline]
        fn as_mut(&mut self) -> &mut BStr {
            BStr::new_mut(self)
        }
    }

    impl Borrow<BStr> for [u8] {
        #[inline]
        fn borrow(&self) -> &BStr {
            self.as_bstr()
        }
    }

    impl Borrow<BStr> for str {
        #[inline]
        fn borrow(&self) -> &BStr {
            self.as_bytes().as_bstr()
        }
    }

    impl Borrow<[u8]> for BStr {
        #[inline]
        fn borrow(&self) -> &[u8] {
            self.as_bytes()
        }
    }

    impl BorrowMut<BStr> for [u8] {
        #[inline]
        fn borrow_mut(&mut self) -> &mut BStr {
            BStr::new_mut(self)
        }
    }

    impl BorrowMut<[u8]> for BStr {
        #[inline]
        fn borrow_mut(&mut self) -> &mut [u8] {
            self.as_bytes_mut()
        }
    }

    impl<'a> Default for &'a BStr {
        fn default() -> &'a BStr {
            BStr::from_bytes(b"")
        }
    }

    impl<'a> Default for &'a mut BStr {
        fn default() -> &'a mut BStr {
            BStr::from_bytes_mut(&mut [])
        }
    }

    #[cfg(feature = "alloc")]
    impl Default for Box<BStr> {
        #[inline]
        fn default() -> Self {
            BStr::from_boxed_bytes(Box::default())
        }
    }

    impl<'a, const N: usize> From<&'a [u8; N]> for &'a BStr {
        #[inline]
        fn from(s: &'a [u8; N]) -> &'a BStr {
            BStr::from_bytes(s)
        }
    }

    impl<'a> From<&'a [u8]> for &'a BStr {
        #[inline]
        fn from(s: &'a [u8]) -> &'a BStr {
            BStr::from_bytes(s)
        }
    }

    impl<'a> From<&'a BStr> for &'a [u8] {
        #[inline]
        fn from(s: &'a BStr) -> &'a [u8] {
            BStr::as_bytes(s)
        }
    }

    impl<'a> From<&'a str> for &'a BStr {
        #[inline]
        fn from(s: &'a str) -> &'a BStr {
            BStr::from_bytes(s.as_bytes())
        }
    }

    #[cfg(feature = "alloc")]
    impl<'a> From<&'a BStr> for Cow<'a, BStr> {
        #[inline]
        fn from(s: &'a BStr) -> Cow<'a, BStr> {
            Cow::Borrowed(s)
        }
    }

    #[cfg(feature = "alloc")]
    impl From<Box<[u8]>> for Box<BStr> {
        #[inline]
        fn from(s: Box<[u8]>) -> Box<BStr> {
            BStr::from_boxed_bytes(s)
        }
    }

    #[cfg(feature = "alloc")]
    impl From<Box<BStr>> for Box<[u8]> {
        #[inline]
        fn from(s: Box<BStr>) -> Box<[u8]> {
            BStr::into_boxed_bytes(s)
        }
    }

    impl<'a> TryFrom<&'a BStr> for &'a str {
        type Error = crate::Utf8Error;

        #[inline]
        fn try_from(s: &'a BStr) -> Result<&'a str, crate::Utf8Error> {
            s.as_bytes().to_str()
        }
    }

    #[cfg(feature = "alloc")]
    impl<'a> TryFrom<&'a BStr> for String {
        type Error = crate::Utf8Error;

        #[inline]
        fn try_from(s: &'a BStr) -> Result<String, crate::Utf8Error> {
            Ok(s.as_bytes().to_str()?.into())
        }
    }

    #[cfg(feature = "alloc")]
    impl Clone for Box<BStr> {
        #[inline]
        fn clone(&self) -> Self {
            BStr::from_boxed_bytes(self.as_bytes().into())
        }
    }

    impl Eq for BStr {}

    impl PartialEq<BStr> for BStr {
        #[inline]
        fn eq(&self, other: &BStr) -> bool {
            self.as_bytes() == other.as_bytes()
        }
    }

    impl_partial_eq!(BStr, [u8]);
    impl_partial_eq!(BStr, &'a [u8]);
    impl_partial_eq!(BStr, str);
    impl_partial_eq!(BStr, &'a str);
    impl_partial_eq_n!(BStr, [u8; N]);
    impl_partial_eq_n!(BStr, &'a [u8; N]);

    #[cfg(feature = "alloc")]
    impl_partial_eq!(BStr, Vec<u8>);
    #[cfg(feature = "alloc")]
    impl_partial_eq!(&'a BStr, Vec<u8>);
    #[cfg(feature = "alloc")]
    impl_partial_eq!(BStr, String);
    #[cfg(feature = "alloc")]
    impl_partial_eq!(&'a BStr, String);
    #[cfg(feature = "alloc")]
    impl_partial_eq_cow!(&'a BStr, Cow<'a, BStr>);
    #[cfg(feature = "alloc")]
    impl_partial_eq_cow!(&'a BStr, Cow<'a, str>);
    #[cfg(feature = "alloc")]
    impl_partial_eq_cow!(&'a BStr, Cow<'a, [u8]>);

    impl hash::Hash for BStr {
        #[inline]
        fn hash<H: hash::Hasher>(&self, state: &mut H) {
            self.as_bytes().hash(state);
        }
    }

    impl PartialOrd for BStr {
        #[inline]
        fn partial_cmp(&self, other: &BStr) -> Option<Ordering> {
            PartialOrd::partial_cmp(self.as_bytes(), other.as_bytes())
        }
    }

    impl Ord for BStr {
        #[inline]
        fn cmp(&self, other: &BStr) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }

    impl_partial_ord!(BStr, [u8]);
    impl_partial_ord!(BStr, &'a [u8]);
    impl_partial_ord!(BStr, str);
    impl_partial_ord!(BStr, &'a str);
    impl_partial_ord_n!(BStr, [u8; N]);
    impl_partial_ord_n!(BStr, &'a [u8; N]);

    #[cfg(feature = "alloc")]
    impl_partial_ord!(BStr, Vec<u8>);
    #[cfg(feature = "alloc")]
    impl_partial_ord!(&'a BStr, Vec<u8>);
    #[cfg(feature = "alloc")]
    impl_partial_ord!(BStr, String);
    #[cfg(feature = "alloc")]
    impl_partial_ord!(&'a BStr, String);
}

#[cfg(feature = "serde")]
mod bstr_serde {
    use core::fmt;

    use serde::{
        de::Error, de::Visitor, Deserialize, Deserializer, Serialize,
        Serializer,
    };

    use crate::bstr::BStr;

    impl Serialize for BStr {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_bytes(self.as_bytes())
        }
    }

    impl<'a, 'de: 'a> Deserialize<'de> for &'a BStr {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<&'a BStr, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct BStrVisitor;

            impl<'de> Visitor<'de> for BStrVisitor {
                type Value = &'de BStr;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a borrowed byte string")
                }

                #[inline]
                fn visit_borrowed_bytes<E: Error>(
                    self,
                    value: &'de [u8],
                ) -> Result<&'de BStr, E> {
                    Ok(BStr::new(value))
                }

                #[inline]
                fn visit_borrowed_str<E: Error>(
                    self,
                    value: &'de str,
                ) -> Result<&'de BStr, E> {
                    Ok(BStr::new(value))
                }
            }

            deserializer.deserialize_bytes(BStrVisitor)
        }
    }
}

#[cfg(all(feature = "serde", feature = "alloc"))]
mod bstring_serde {
    use core::{cmp, fmt};

    use alloc::{boxed::Box, string::String, vec::Vec};

    use serde::{
        de::Error, de::SeqAccess, de::Visitor, Deserialize, Deserializer,
        Serialize, Serializer,
    };

    use crate::{bstr::BStr, bstring::BString};

    impl Serialize for BString {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_bytes(self.as_bytes())
        }
    }

    impl<'de> Deserialize<'de> for BString {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<BString, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct BStringVisitor;

            impl<'de> Visitor<'de> for BStringVisitor {
                type Value = BString;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a byte string")
                }

                #[inline]
                fn visit_seq<V: SeqAccess<'de>>(
                    self,
                    mut visitor: V,
                ) -> Result<BString, V::Error> {
                    let len = cmp::min(visitor.size_hint().unwrap_or(0), 256);
                    let mut bytes = Vec::with_capacity(len);
                    while let Some(v) = visitor.next_element()? {
                        bytes.push(v);
                    }
                    Ok(BString::from(bytes))
                }

                #[inline]
                fn visit_bytes<E: Error>(
                    self,
                    value: &[u8],
                ) -> Result<BString, E> {
                    Ok(BString::from(value))
                }

                #[inline]
                fn visit_byte_buf<E: Error>(
                    self,
                    value: Vec<u8>,
                ) -> Result<BString, E> {
                    Ok(BString::from(value))
                }

                #[inline]
                fn visit_str<E: Error>(
                    self,
                    value: &str,
                ) -> Result<BString, E> {
                    Ok(BString::from(value))
                }

                #[inline]
                fn visit_string<E: Error>(
                    self,
                    value: String,
                ) -> Result<BString, E> {
                    Ok(BString::from(value))
                }
            }

            deserializer.deserialize_byte_buf(BStringVisitor)
        }
    }

    impl<'de> Deserialize<'de> for Box<BStr> {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Box<BStr>, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct BoxedBStrVisitor;

            impl<'de> Visitor<'de> for BoxedBStrVisitor {
                type Value = Box<BStr>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a boxed byte string")
                }

                #[inline]
                fn visit_seq<V: SeqAccess<'de>>(
                    self,
                    mut visitor: V,
                ) -> Result<Box<BStr>, V::Error> {
                    let len = cmp::min(visitor.size_hint().unwrap_or(0), 256);
                    let mut bytes = Vec::with_capacity(len);
                    while let Some(v) = visitor.next_element()? {
                        bytes.push(v);
                    }
                    Ok(BStr::from_boxed_bytes(bytes.into_boxed_slice()))
                }

                #[inline]
                fn visit_bytes<E: Error>(
                    self,
                    value: &[u8],
                ) -> Result<Box<BStr>, E> {
                    Ok(BStr::from_boxed_bytes(
                        value.to_vec().into_boxed_slice(),
                    ))
                }

                #[inline]
                fn visit_byte_buf<E: Error>(
                    self,
                    value: Vec<u8>,
                ) -> Result<Box<BStr>, E> {
                    Ok(BStr::from_boxed_bytes(value.into_boxed_slice()))
                }

                #[inline]
                fn visit_str<E: Error>(
                    self,
                    value: &str,
                ) -> Result<Box<BStr>, E> {
                    Ok(BStr::from_boxed_bytes(
                        value.as_bytes().to_vec().into_boxed_slice(),
                    ))
                }

                #[inline]
                fn visit_string<E: Error>(
                    self,
                    value: String,
                ) -> Result<Box<BStr>, E> {
                    Ok(BStr::from_boxed_bytes(
                        value.into_bytes().into_boxed_slice(),
                    ))
                }
            }

            deserializer.deserialize_byte_buf(BoxedBStrVisitor)
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod display {
    use alloc::format;

    #[cfg(not(miri))]
    use crate::bstring::BString;
    use crate::ByteSlice;

    #[test]
    fn clean() {
        assert_eq!(&format!("{}", &b"abc".as_bstr()), "abc");
        assert_eq!(&format!("{}", &b"\xf0\x28\x8c\xbc".as_bstr()), "�(��");
    }

    #[test]
    fn from_str() {
        let s: BString = "abc".parse().unwrap();
        assert_eq!(s, BString::new(b"abc".to_vec()));
    }

    #[test]
    fn width_bigger_than_bstr() {
        assert_eq!(&format!("{:<7}!", &b"abc".as_bstr()), "abc    !");
        assert_eq!(&format!("{:>7}!", &b"abc".as_bstr()), "    abc!");
        assert_eq!(&format!("{:^7}!", &b"abc".as_bstr()), "  abc  !");
        assert_eq!(&format!("{:^6}!", &b"abc".as_bstr()), " abc  !");
        assert_eq!(&format!("{:-<7}!", &b"abc".as_bstr()), "abc----!");
        assert_eq!(&format!("{:->7}!", &b"abc".as_bstr()), "----abc!");
        assert_eq!(&format!("{:-^7}!", &b"abc".as_bstr()), "--abc--!");
        assert_eq!(&format!("{:-^6}!", &b"abc".as_bstr()), "-abc--!");

        assert_eq!(
            &format!("{:<7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��   !"
        );
        assert_eq!(
            &format!("{:>7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "   �(��!"
        );
        assert_eq!(
            &format!("{:^7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            " �(��  !"
        );
        assert_eq!(
            &format!("{:^6}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            " �(�� !"
        );

        assert_eq!(
            &format!("{:-<7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��---!"
        );
        assert_eq!(
            &format!("{:->7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "---�(��!"
        );
        assert_eq!(
            &format!("{:-^7}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "-�(��--!"
        );
        assert_eq!(
            &format!("{:-^6}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "-�(��-!"
        );
    }

    #[test]
    fn width_lesser_than_bstr() {
        assert_eq!(&format!("{:<2}!", &b"abc".as_bstr()), "abc!");
        assert_eq!(&format!("{:>2}!", &b"abc".as_bstr()), "abc!");
        assert_eq!(&format!("{:^2}!", &b"abc".as_bstr()), "abc!");
        assert_eq!(&format!("{:-<2}!", &b"abc".as_bstr()), "abc!");
        assert_eq!(&format!("{:->2}!", &b"abc".as_bstr()), "abc!");
        assert_eq!(&format!("{:-^2}!", &b"abc".as_bstr()), "abc!");

        assert_eq!(
            &format!("{:<3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:>3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:^3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:^2}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );

        assert_eq!(
            &format!("{:-<3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:->3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:-^3}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
        assert_eq!(
            &format!("{:-^2}!", &b"\xf0\x28\x8c\xbc".as_bstr()),
            "�(��!"
        );
    }

    #[cfg(not(miri))]
    quickcheck::quickcheck! {
        fn total_length(bstr: BString) -> bool {
            let size = bstr.chars().count();
            format!("{:<1$}", bstr.as_bstr(), size).chars().count() >= size
        }
    }
}

#[cfg(all(test, feature = "alloc"))]
mod bstring_arbitrary {
    use alloc::{boxed::Box, vec::Vec};

    use crate::bstring::BString;

    use quickcheck::{Arbitrary, Gen};

    impl Arbitrary for BString {
        fn arbitrary(g: &mut Gen) -> BString {
            BString::from(Vec::<u8>::arbitrary(g))
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = BString>> {
            Box::new(self.as_vec().shrink().map(BString::from))
        }
    }
}

#[test]
#[cfg(feature = "std")]
fn test_debug() {
    use alloc::format;

    use crate::{ByteSlice, B};

    assert_eq!(
        r#""\0\0\0 ftypisom\0\0\x02\0isomiso2avc1mp""#,
        format!("{:?}", b"\0\0\0 ftypisom\0\0\x02\0isomiso2avc1mp".as_bstr()),
    );

    // Tests that if the underlying bytes contain the UTF-8 encoding of the
    // replacement codepoint, then we emit the codepoint just like other
    // non-printable Unicode characters.
    assert_eq!(
        b"\"\\xff\xef\xbf\xbd\\xff\"".as_bstr(),
        // Before fixing #72, the output here would be:
        //   \\xFF\\xEF\\xBF\\xBD\\xFF
        B(&format!("{:?}", b"\xff\xef\xbf\xbd\xff".as_bstr())).as_bstr(),
    );

    // Tests that all ASCII control characters are in lower case.
    assert_eq!(
        b"\"\\xed\\xa0\\x80Aa\\x7f\\x0b\"".as_bstr(),
        // Before fixing #188, the output here would be:
        //   \\xED\\xA0\\x80Aa\\x7f\\x0b
        B(&format!("{:?}", b"\xed\xa0\x80Aa\x7f\x0b".as_bstr())).as_bstr(),
    );

    assert_eq!(
        r#""\0\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x11\x12\r\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f \x7f\x80\x81\xfe\xff""#,
        format!("{:?}", b"\0\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x11\x12\r\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f \x7f\x80\x81\xfe\xff".as_bstr()),
    );
}

// See: https://github.com/BurntSushi/bstr/issues/82
#[test]
#[cfg(feature = "std")]
fn test_cows_regression() {
    use std::borrow::Cow;

    use crate::ByteSlice;

    let c1 = Cow::from(b"hello bstr".as_bstr());
    let c2 = b"goodbye bstr".as_bstr();
    assert_ne!(c1, c2);

    let c3 = Cow::from("hello str");
    let c4 = "goodbye str";
    assert_ne!(c3, c4);
}

#[test]
#[cfg(feature = "alloc")]
fn test_eq_ord() {
    use core::cmp::Ordering;

    use crate::{BStr, BString};

    let b = BStr::new("hello");
    assert_eq!(b, b"hello");
    assert_ne!(b, b"world");
    assert_eq!(b.partial_cmp(b"hello"), Some(Ordering::Equal));
    assert_eq!(b.partial_cmp(b"world"), Some(Ordering::Less));

    let b = BString::from("hello");
    assert_eq!(b, b"hello");
    assert_ne!(b, b"world");
    assert_eq!(b.partial_cmp(b"hello"), Some(Ordering::Equal));
    assert_eq!(b.partial_cmp(b"world"), Some(Ordering::Less));
}
