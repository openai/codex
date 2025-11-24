// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This test is a duplicate of one of the doctests, but is written separately
// since `cargo miri test` doesn't work on doctests yet

use std::borrow::Cow;
use std::mem;
use std::rc::Rc;
use yoke::{Yoke, Yokeable};

fn load_from_cache(_filename: &str) -> Rc<[u8]> {
    // dummy implementation
    Rc::new([0x5, 0, 0, 0, 0, 0, 0, 0, 0x68, 0x65, 0x6c, 0x6c, 0x6f])
}

fn load_object(filename: &str) -> Yoke<Bar<'static>, Rc<[u8]>> {
    let rc: Rc<[u8]> = load_from_cache(filename);
    Yoke::<Bar<'static>, Rc<[u8]>>::attach_to_cart(rc, |data: &[u8]| {
        // A real implementation would properly deserialize `Bar` as a whole
        Bar {
            numbers: Cow::Borrowed(bincode::deserialize(data).unwrap()),
            string: Cow::Borrowed(bincode::deserialize(data).unwrap()),
            owned: Vec::new(),
        }
    })
}

// also implements Yokeable
struct Bar<'a> {
    numbers: Cow<'a, [u8]>,
    string: Cow<'a, str>,
    owned: Vec<u8>,
}

unsafe impl<'a> Yokeable<'a> for Bar<'static> {
    type Output = Bar<'a>;
    #[inline]
    fn transform(&'a self) -> &'a Bar<'a> {
        self
    }
    #[inline]
    fn transform_owned(self) -> Bar<'a> {
        self
    }
    #[inline]
    unsafe fn make(from: Bar<'a>) -> Self {
        let ret = mem::transmute_copy(&from);
        mem::forget(from);
        ret
    }
    #[inline]
    fn transform_mut<F>(&'a mut self, f: F)
    where
        F: 'static + FnOnce(&'a mut Self::Output),
    {
        unsafe { f(mem::transmute::<&mut Bar<'_>, &mut Bar<'a>>(self)) }
    }
}

#[test]
fn test_load() {
    // `load_object()` deserializes an object from a file
    let mut bar = load_object("filename.bincode");
    assert_eq!(bar.get().string, "hello");
    assert!(matches!(bar.get().string, Cow::Borrowed(_)));
    assert_eq!(&*bar.get().numbers, &[0x68, 0x65, 0x6c, 0x6c, 0x6f]);
    assert!(matches!(bar.get().numbers, Cow::Borrowed(_)));
    assert_eq!(&*bar.get().owned, &[]);

    bar.with_mut(|bar| {
        bar.string.to_mut().push_str(" world");
        bar.owned.extend_from_slice(&[1, 4, 1, 5, 9]);
    });

    assert_eq!(bar.get().string, "hello world");
    assert!(matches!(bar.get().string, Cow::Owned(_)));
    assert_eq!(&*bar.get().owned, &[1, 4, 1, 5, 9]);
    // Unchanged and still Cow::Borrowed
    assert_eq!(&*bar.get().numbers, &[0x68, 0x65, 0x6c, 0x6c, 0x6f]);
    assert!(matches!(bar.get().numbers, Cow::Borrowed(_)));
}
