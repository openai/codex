// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::*;
use core::fmt;

macro_rules! impl_write_num {
    // random_call exists since usize doesn't have a rand impl. Should always be `random` or empty
    ($u:ty, $i:ty, $test:ident $(,$random_call:ident)?) => {
        impl $crate::Writeable for $u {
            fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
                const MAX_LEN: usize = <$u>::MAX.ilog10() as usize + 1;
                let mut buf = [b'0'; MAX_LEN];
                let mut n = *self;
                let mut i = MAX_LEN;
                #[expect(clippy::indexing_slicing)] // n < 10^i
                while n != 0 {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                if i == MAX_LEN {
                    debug_assert_eq!(*self, 0);
                    i -= 1;
                }
                #[expect(clippy::indexing_slicing)] // buf is ASCII
                let s = unsafe { core::str::from_utf8_unchecked(&buf[i..]) };
                sink.write_str(s)
            }

            fn writeable_length_hint(&self) -> $crate::LengthHint {
                LengthHint::exact(self.checked_ilog10().unwrap_or(0) as usize + 1)
            }
        }

        impl $crate::Writeable for $i {
            fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
                if self.is_negative() {
                    sink.write_str("-")?;
                }
                self.unsigned_abs().write_to(sink)
            }

            fn writeable_length_hint(&self) -> $crate::LengthHint {
                $crate::LengthHint::exact(if self.is_negative() { 1 } else { 0 })
                    + self.unsigned_abs().writeable_length_hint()
            }
        }

        #[test]
        fn $test() {
            use $crate::assert_writeable_eq;
            assert_writeable_eq!(&(0 as $u), "0");
            assert_writeable_eq!(&(0 as $i), "0");
            assert_writeable_eq!(&(-0 as $i), "0");
            assert_writeable_eq!(&(1 as $u), "1");
            assert_writeable_eq!(&(1 as $i), "1");
            assert_writeable_eq!(&(-1 as $i), "-1");
            assert_writeable_eq!(&(9 as $u), "9");
            assert_writeable_eq!(&(9 as $i), "9");
            assert_writeable_eq!(&(-9 as $i), "-9");
            assert_writeable_eq!(&(10 as $u), "10");
            assert_writeable_eq!(&(10 as $i), "10");
            assert_writeable_eq!(&(-10 as $i), "-10");
            assert_writeable_eq!(&(99 as $u), "99");
            assert_writeable_eq!(&(99 as $i), "99");
            assert_writeable_eq!(&(-99 as $i), "-99");
            assert_writeable_eq!(&(100 as $u), "100");
            assert_writeable_eq!(&(-100 as $i), "-100");
            assert_writeable_eq!(&<$u>::MAX, <$u>::MAX.to_string());
            assert_writeable_eq!(&<$i>::MAX, <$i>::MAX.to_string());
            assert_writeable_eq!(&<$i>::MIN, <$i>::MIN.to_string());

            $(

                use rand::{rngs::SmallRng, Rng, SeedableRng};
                let mut rng = SmallRng::seed_from_u64(4); // chosen by fair dice roll.
                                                          // guaranteed to be random.
                for _ in 0..1000 {
                    let rand = rng.$random_call::<$u>();
                    assert_writeable_eq!(rand, rand.to_string());
                }
            )?
        }
    };
}

impl_write_num!(u8, i8, test_u8, random);
impl_write_num!(u16, i16, test_u16, random);
impl_write_num!(u32, i32, test_u32, random);
impl_write_num!(u64, i64, test_u64, random);
impl_write_num!(u128, i128, test_u128, random);
impl_write_num!(usize, isize, test_usize);

impl Writeable for str {
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        sink.write_str(self)
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        LengthHint::exact(self.len())
    }

    #[inline]
    fn writeable_borrow(&self) -> Option<&str> {
        Some(self)
    }
}

#[cfg(feature = "alloc")]
impl Writeable for String {
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        sink.write_str(self)
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        LengthHint::exact(self.len())
    }

    #[inline]
    fn writeable_borrow(&self) -> Option<&str> {
        Some(self)
    }
}

impl Writeable for char {
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        sink.write_char(*self)
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        LengthHint::exact(self.len_utf8())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn write_to_string(&self) -> Cow<'_, str> {
        let mut s = String::with_capacity(self.len_utf8());
        s.push(*self);
        Cow::Owned(s)
    }
}

impl<T: Writeable + ?Sized> Writeable for &T {
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        (*self).write_to(sink)
    }

    #[inline]
    fn write_to_parts<W: PartsWrite + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        (*self).write_to_parts(sink)
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        (*self).writeable_length_hint()
    }

    #[inline]
    fn writeable_borrow(&self) -> Option<&str> {
        (*self).writeable_borrow()
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn write_to_string(&self) -> Cow<'_, str> {
        (*self).write_to_string()
    }
}

#[cfg(feature = "alloc")]
macro_rules! impl_write_smart_pointer {
    ($ty:path, T: $extra_bound:path) => {
        impl<'a, T: ?Sized + Writeable + $extra_bound> Writeable for $ty {
            #[inline]
            fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
                core::borrow::Borrow::<T>::borrow(self).write_to(sink)
            }
            #[inline]
            fn write_to_parts<W: PartsWrite + ?Sized>(&self, sink: &mut W) -> fmt::Result {
                core::borrow::Borrow::<T>::borrow(self).write_to_parts(sink)
            }
            #[inline]
            fn writeable_length_hint(&self) -> LengthHint {
                core::borrow::Borrow::<T>::borrow(self).writeable_length_hint()
            }
            #[inline]
            fn writeable_borrow(&self) -> Option<&str> {
                core::borrow::Borrow::<T>::borrow(self).writeable_borrow()
            }
            #[inline]
            fn write_to_string(&self) -> Cow<'_, str> {
                core::borrow::Borrow::<T>::borrow(self).write_to_string()
            }
        }
    };
    ($ty:path) => {
        // Add a harmless duplicate Writeable bound
        impl_write_smart_pointer!($ty, T: Writeable);
    };
}

#[cfg(feature = "alloc")]
impl_write_smart_pointer!(Cow<'a, T>, T: alloc::borrow::ToOwned);
#[cfg(feature = "alloc")]
impl_write_smart_pointer!(alloc::boxed::Box<T>);
#[cfg(feature = "alloc")]
impl_write_smart_pointer!(alloc::rc::Rc<T>);
#[cfg(feature = "alloc")]
impl_write_smart_pointer!(alloc::sync::Arc<T>);

#[test]
fn test_string_impls() {
    fn check_writeable_slice<W: Writeable + core::fmt::Display>(writeables: &[W]) {
        assert_writeable_eq!(&writeables[0], "");
        assert_writeable_eq!(&writeables[1], "abc");
        assert!(matches!(writeables[0].write_to_string(), Cow::Borrowed(_)));
        assert!(matches!(writeables[1].write_to_string(), Cow::Borrowed(_)));
    }

    // test str impl
    let arr: &[&str] = &["", "abc"];
    check_writeable_slice(arr);

    // test String impl
    let arr: &[String] = &[String::new(), "abc".to_owned()];
    check_writeable_slice(arr);

    // test char impl
    let chars = ['a', 'β', '你', '😀'];
    for i in 0..chars.len() {
        let s = String::from(chars[i]);
        assert_writeable_eq!(&chars[i], s);
        for j in 0..chars.len() {
            assert_eq!(
                crate::cmp_str(&chars[j], &s),
                chars[j].cmp(&chars[i]),
                "{:?} vs {:?}",
                chars[j],
                chars[i]
            );
        }
    }

    // test Cow impl
    let arr: &[Cow<str>] = &[Cow::Borrowed(""), Cow::Owned("abc".to_string())];
    check_writeable_slice(arr);

    // test Box impl
    let arr: &[Box<str>] = &["".into(), "abc".into()];
    check_writeable_slice(arr);

    // test Rc impl
    let arr: &[alloc::rc::Rc<str>] = &["".into(), "abc".into()];
    check_writeable_slice(arr);

    // test Arc impl
    let arr: &[alloc::sync::Arc<str>] = &["".into(), "abc".into()];
    check_writeable_slice(arr);

    // test &T impl
    let arr: &[&String] = &[&String::new(), &"abc".to_owned()];
    check_writeable_slice(arr);
}
