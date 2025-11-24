//! zeroize integration tests.

use std::{
    marker::{PhantomData, PhantomPinned},
    mem::{size_of, MaybeUninit},
    num::*,
};
use zeroize::*;

#[cfg(feature = "std")]
use std::ffi::CString;

#[derive(Clone, Debug, PartialEq)]
struct ZeroizedOnDrop(u64);

impl Drop for ZeroizedOnDrop {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[test]
fn non_zero() {
    macro_rules! non_zero_test {
        ($($type:ty),+) => {
            $(let mut value = <$type>::new(42).unwrap();
            value.zeroize();
            assert_eq!(value.get(), 1);)+
        };
    }

    non_zero_test!(
        NonZeroI8,
        NonZeroI16,
        NonZeroI32,
        NonZeroI64,
        NonZeroI128,
        NonZeroIsize,
        NonZeroU8,
        NonZeroU16,
        NonZeroU32,
        NonZeroU64,
        NonZeroU128,
        NonZeroUsize
    );
}

#[test]
fn zeroize_byte_arrays() {
    let mut arr = [42u8; 137];
    arr.zeroize();
    assert_eq!(arr.as_ref(), [0u8; 137].as_ref());
}

#[test]
fn zeroize_on_drop_byte_arrays() {
    let mut arr = [ZeroizedOnDrop(42); 1];
    unsafe { core::ptr::drop_in_place(&mut arr) };
    assert_eq!(arr.as_ref(), [ZeroizedOnDrop(0); 1].as_ref());
}

#[test]
fn zeroize_maybeuninit_byte_arrays() {
    let mut arr = [MaybeUninit::new(42u64); 64];
    arr.zeroize();
    let arr_init: [u64; 64] = unsafe { core::mem::transmute(arr) };
    assert_eq!(arr_init, [0u64; 64]);
}

#[test]
fn zeroize_check_zerosize_types() {
    // Since we assume these types have zero size, we test this holds for
    // the current version of Rust.
    assert_eq!(size_of::<()>(), 0);
    assert_eq!(size_of::<PhantomPinned>(), 0);
    assert_eq!(size_of::<PhantomData<usize>>(), 0);
}

#[test]
fn zeroize_check_tuple() {
    let mut tup1 = (42u8,);
    tup1.zeroize();
    assert_eq!(tup1, (0u8,));

    let mut tup2 = (42u8, 42u8);
    tup2.zeroize();
    assert_eq!(tup2, (0u8, 0u8));
}

#[test]
fn zeroize_on_drop_check_tuple() {
    let mut tup1 = (ZeroizedOnDrop(42),);
    unsafe { core::ptr::drop_in_place(&mut tup1) };
    assert_eq!(tup1, (ZeroizedOnDrop(0),));

    let mut tup2 = (ZeroizedOnDrop(42), ZeroizedOnDrop(42));
    unsafe { core::ptr::drop_in_place(&mut tup2) };
    assert_eq!(tup2, (ZeroizedOnDrop(0), ZeroizedOnDrop(0)));
}

#[cfg(feature = "alloc")]
#[test]
fn zeroize_vec() {
    let mut vec = vec![42; 3];
    vec.zeroize();
    assert!(vec.is_empty());
}

#[cfg(feature = "alloc")]
#[test]
fn zeroize_vec_entire_capacity() {
    #[derive(Clone)]
    struct PanicOnNonZeroDrop(u64);

    impl Zeroize for PanicOnNonZeroDrop {
        fn zeroize(&mut self) {
            self.0 = 0;
        }
    }

    impl Drop for PanicOnNonZeroDrop {
        fn drop(&mut self) {
            if self.0 != 0 {
                panic!("dropped non-zeroized data");
            }
        }
    }

    // Ensure that the entire capacity of the vec is zeroized and that no unitinialized data
    // is ever interpreted as initialized
    let mut vec = vec![PanicOnNonZeroDrop(42); 2];

    unsafe {
        vec.set_len(1);
    }

    vec.zeroize();

    unsafe {
        vec.set_len(2);
    }

    drop(vec);
}

#[cfg(feature = "alloc")]
#[test]
fn zeroize_string() {
    let mut string = String::from("Hello, world!");
    string.zeroize();
    assert!(string.is_empty());
}

#[cfg(feature = "alloc")]
#[test]
fn zeroize_string_entire_capacity() {
    let mut string = String::from("Hello, world!");
    string.truncate(5);

    string.zeroize();

    // convert the string to a vec to easily access the unused capacity
    let mut as_vec = string.into_bytes();
    unsafe { as_vec.set_len(as_vec.capacity()) };

    assert!(as_vec.iter().all(|byte| *byte == 0));
}

// TODO(tarcieri): debug flaky test (with potential UB?) See: RustCrypto/utils#774
#[cfg(feature = "std")]
#[ignore]
#[test]
fn zeroize_c_string() {
    let mut cstring = CString::new("Hello, world!").expect("CString::new failed");
    let orig_len = cstring.as_bytes().len();
    let orig_ptr = cstring.as_bytes().as_ptr();
    cstring.zeroize();
    // This doesn't quite test that the original memory has been cleared, but only that
    // cstring now owns an empty buffer
    assert!(cstring.as_bytes().is_empty());
    for i in 0..orig_len {
        unsafe {
            // Using a simple deref, only one iteration of the loop is performed
            // presumably because after zeroize, the internal buffer has a length of one/
            // `read_volatile` seems to "fix" this
            // Note that this is very likely UB
            assert_eq!(orig_ptr.add(i).read_volatile(), 0);
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn zeroize_box() {
    let mut boxed_arr = Box::new([42u8; 3]);
    boxed_arr.zeroize();
    assert_eq!(boxed_arr.as_ref(), &[0u8; 3]);
}

#[cfg(feature = "alloc")]
#[test]
fn asref() {
    let mut buffer: Zeroizing<Vec<u8>> = Default::default();
    let _asmut: &mut [u8] = buffer.as_mut();
    let _asref: &[u8] = buffer.as_ref();

    let mut buffer: Zeroizing<Box<[u8]>> = Default::default();
    let _asmut: &mut [u8] = buffer.as_mut();
    let _asref: &[u8] = buffer.as_ref();
}
