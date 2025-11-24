//! Faster, more compact implementation of `Cow`.
//!
//! **[Changelog](https://github.com/maciejhirsz/beef/releases) -**
//! **[Cargo](https://crates.io/crates/beef) -**
//! **[Repository](https://github.com/maciejhirsz/beef)**
//!
//! ```rust
//! use beef::Cow;
//!
//! let borrowed: Cow<str> = Cow::borrowed("Hello");
//! let owned: Cow<str> = Cow::owned(String::from("World"));
//!
//! assert_eq!(
//!     format!("{} {}!", borrowed, owned),
//!     "Hello World!",
//! );
//! ```
//!
//! There are two versions of `Cow` exposed by this crate:
//!
//! + `beef::Cow` is 3 words wide: pointer, length, and capacity. It stores the ownership tag in capacity.
//! + `beef::lean::Cow` is 2 words wide, storing length, capacity, and the ownership tag all in one word.
//!
//! Both versions are leaner than the `std::borrow::Cow`:
//!
//! ```rust
//! use std::mem::size_of;
//!
//! const WORD: usize = size_of::<usize>();
//!
//! assert_eq!(size_of::<std::borrow::Cow<str>>(), 4 * WORD);
//! assert_eq!(size_of::<beef::Cow<str>>(), 3 * WORD);
//!
//! // Lean variant is two words on 64-bit architecture
//! #[cfg(target_pointer_width = "64")]
//! assert_eq!(size_of::<beef::lean::Cow<str>>(), 2 * WORD);
//! ```
#![cfg_attr(feature = "const_fn", feature(const_fn_trait_bound))]
#![warn(missing_docs)]
#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod traits;
mod wide;

#[cfg(feature = "impl_serde")]
mod serde;

pub mod generic;
#[cfg(target_pointer_width = "64")]
pub mod lean;

#[cfg(not(target_pointer_width = "64"))]
pub mod lean {
    /// Re-exports 3-word Cow for non-64-bit targets
    pub use super::wide::Cow;
}

pub use wide::Cow;

#[rustfmt::skip]
macro_rules! test { ($tmod:ident => $cow:path) => {
    #[cfg(test)]
    mod $tmod {
        use $cow;

        #[test]
        fn borrowed_str() {
            let s = "Hello World";
            let c = Cow::borrowed(s);

            assert_eq!(s, c);
            assert_eq!(s, c.as_ref());
            assert_eq!(s, &*c);
        }

        #[test]
        fn owned_string() {
            let s = String::from("Hello World");
            let c: Cow<str> = Cow::owned(s.clone());

            assert_eq!(s, c);
        }

        #[test]
        fn into_owned() {
            let hello = "Hello World";
            let borrowed = Cow::borrowed(hello);
            let owned: Cow<str> = Cow::owned(String::from(hello));

            assert_eq!(borrowed.into_owned(), hello);
            assert_eq!(owned.into_owned(), hello);
        }

        #[test]
        fn borrowed_slice() {
            let s: &[_] = &[1, 2, 42];
            let c = Cow::borrowed(s);

            assert_eq!(s, c);
            assert_eq!(s, c.as_ref());
            assert_eq!(s, &*c);
        }

        #[test]
        fn owned_slice() {
            let s = vec![1, 2, 42];
            let c: Cow<[_]> = Cow::owned(s.clone());

            assert_eq!(s, c);
        }

        #[test]
        fn into_owned_vec() {
            let hello: &[u8] = b"Hello World";
            let borrowed = Cow::borrowed(hello);
            let owned: Cow<[u8]> = Cow::owned(hello.to_vec());

            assert_eq!(borrowed.into_owned(), hello);
            assert_eq!(owned.into_owned(), hello);
        }

        #[test]
        fn hash() {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let slice = "Hello World!";
            let borrowed = Cow::borrowed(slice);
            let owned: Cow<str> = Cow::owned(slice.to_owned());

            let hash1 = {
                let mut hasher = DefaultHasher::default();

                slice.hash(&mut hasher);

                hasher.finish()
            };

            let hash2 = {
                let mut hasher = DefaultHasher::default();

                borrowed.hash(&mut hasher);

                hasher.finish()
            };

            let hash3 = {
                let mut hasher = DefaultHasher::default();

                owned.hash(&mut hasher);

                hasher.finish()
            };

            assert_eq!(hash1, hash2);
            assert_eq!(hash1, hash3);
            assert_eq!(hash2, hash3);
        }

        #[test]
        fn ord_and_partial_ord() {
            use std::cmp::Ordering;

            macro_rules! generate_order_tests {
                ( $f:tt => $order:expr => $left:expr, $right:expr ) => {
                    assert_eq!(
                        Cow::<str>::borrowed($left).$f(&Cow::<str>::borrowed($right)),
                        $order
                    );

                    assert_eq!(
                        Cow::<str>::owned($left.to_owned())
                            .$f(&Cow::<str>::borrowed($right)),
                        $order
                    );

                    assert_eq!(
                        Cow::<str>::borrowed($left)
                            .$f(&Cow::<str>::owned($right.to_owned())),
                        $order
                    );

                    assert_eq!(
                        Cow::<str>::owned($left.to_owned())
                            .$f(&Cow::<str>::owned($right.to_owned())),
                        $order
                    );
                }
            }

            generate_order_tests!(partial_cmp => Some(Ordering::Equal) => "a", "a");
            generate_order_tests!(partial_cmp => Some(Ordering::Less) => "a", "b");
            generate_order_tests!(partial_cmp => Some(Ordering::Greater) => "b", "a");

            generate_order_tests!(cmp => Ordering::Equal => "a", "a");
            generate_order_tests!(cmp => Ordering::Less => "a", "b");
            generate_order_tests!(cmp => Ordering::Greater => "b", "a");
        }

        #[test]
        fn from_std_cow() {
            let std = std::borrow::Cow::Borrowed("Hello World");
            let beef = Cow::from(std.clone());

            assert_eq!(&*std, &*beef);
        }

        #[test]
        fn unwrap_borrowed() {
            let borrowed = Cow::borrowed("Hello");

            assert_eq!(borrowed.unwrap_borrowed(), "Hello");
        }

        #[test]
        #[should_panic]
        fn unwrap_owned() {
            let borrowed: Cow<str> = Cow::owned("Hello".to_string());

            borrowed.unwrap_borrowed();
        }

        #[test]
        fn stress_test_owned() {
            let mut expected = String::from("Hello... ");
            let mut cow: Cow<str> = Cow::borrowed("Hello... ");

            #[cfg(not(miri))]
            let iterations = 1024;
            #[cfg(miri)]
            let iterations = 10;

            for i in 0..iterations {
                if i % 3 == 0 {
                    let old = cow;
                    cow = old.clone();

                    std::mem::drop(old);
                }

                let mut owned = cow.into_owned();

                expected.push_str("Hello?.. ");
                owned.push_str("Hello?.. ");

                cow = owned.into();
            }

            assert_eq!(expected, cow.into_owned());
        }

        #[test]
        fn const_fn_str() {
            const HELLO: Cow<str> = Cow::const_str("Hello");

            assert_eq!(&*HELLO, "Hello");
        }

        #[test]
        #[cfg(feature = "const_fn")]
        fn const_fn_slice() {
            const FOO: Cow<[u8]> = Cow::const_slice(b"bar");

            assert_eq!(&*FOO, b"bar");
        }

        #[test]
        fn default_str() {
            let empty: Cow<str> = Default::default();

            assert_eq!(&*empty, "");
        }

        #[test]
        fn default_slice() {
            let empty: Cow<[u8]> = Default::default();

            assert_eq!(&*empty, b"");
        }
    }
} }

test!(test_wide => crate::wide::Cow);
test!(test_lean => crate::lean::Cow);
