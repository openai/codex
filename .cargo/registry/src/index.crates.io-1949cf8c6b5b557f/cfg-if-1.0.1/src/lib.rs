//! A macro for defining `#[cfg]` if-else statements.
//!
//! The macro provided by this crate, `cfg_if`, is similar to the `if/elif` C
//! preprocessor macro by allowing definition of a cascade of `#[cfg]` cases,
//! emitting the implementation which matches first.
//!
//! This allows you to conveniently provide a long list `#[cfg]`'d blocks of code
//! without having to rewrite each clause multiple times.
//!
//! # Example
//!
//! ```
//! cfg_if::cfg_if! {
//!     if #[cfg(unix)] {
//!         fn foo() { /* unix specific functionality */ }
//!     } else if #[cfg(target_pointer_width = "32")] {
//!         fn foo() { /* non-unix, 32-bit functionality */ }
//!     } else {
//!         fn foo() { /* fallback implementation */ }
//!     }
//! }
//!
//! # fn main() {}
//! ```

#![no_std]
#![doc(html_root_url = "https://docs.rs/cfg-if")]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(test, allow(unexpected_cfgs))] // we test with features that do not exist

/// The main macro provided by this crate. See crate documentation for more
/// information.
#[macro_export]
macro_rules! cfg_if {
    // match if/else chains with a final `else`
    (
        $(
            if #[cfg( $i_meta:meta )] { $( $i_tokens:tt )* }
        ) else+
        else { $( $e_tokens:tt )* }
    ) => {
        $crate::cfg_if! {
            @__items () ;
            $(
                (( $i_meta ) ( $( $i_tokens )* )) ,
            )+
            (() ( $( $e_tokens )* )) ,
        }
    };

    // match if/else chains lacking a final `else`
    (
        if #[cfg( $i_meta:meta )] { $( $i_tokens:tt )* }
        $(
            else if #[cfg( $e_meta:meta )] { $( $e_tokens:tt )* }
        )*
    ) => {
        $crate::cfg_if! {
            @__items () ;
            (( $i_meta ) ( $( $i_tokens )* )) ,
            $(
                (( $e_meta ) ( $( $e_tokens )* )) ,
            )*
        }
    };

    // Internal and recursive macro to emit all the items
    //
    // Collects all the previous cfgs in a list at the beginning, so they can be
    // negated. After the semicolon are all the remaining items.
    (@__items ( $( $_:meta , )* ) ; ) => {};
    (
        @__items ( $( $no:meta , )* ) ;
        (( $( $yes:meta )? ) ( $( $tokens:tt )* )) ,
        $( $rest:tt , )*
    ) => {
        // Emit all items within one block, applying an appropriate #[cfg]. The
        // #[cfg] will require all `$yes` matchers specified and must also negate
        // all previous matchers.
        #[cfg(all(
            $( $yes , )?
            not(any( $( $no ),* ))
        ))]
        $crate::cfg_if! { @__identity $( $tokens )* }

        // Recurse to emit all other items in `$rest`, and when we do so add all
        // our `$yes` matchers to the list of `$no` matchers as future emissions
        // will have to negate everything we just matched as well.
        $crate::cfg_if! {
            @__items ( $( $no , )* $( $yes , )? ) ;
            $( $rest , )*
        }
    };

    // Internal macro to make __apply work out right for different match types,
    // because of how macros match/expand stuff.
    (@__identity $( $tokens:tt )* ) => {
        $( $tokens )*
    };
}

#[cfg(test)]
mod tests {
    cfg_if! {
        if #[cfg(test)] {
            use core::option::Option as Option2;
            fn works1() -> Option2<u32> { Some(1) }
        } else {
            fn works1() -> Option<u32> { None }
        }
    }

    cfg_if! {
        if #[cfg(foo)] {
            fn works2() -> bool { false }
        } else if #[cfg(test)] {
            fn works2() -> bool { true }
        } else {
            fn works2() -> bool { false }
        }
    }

    cfg_if! {
        if #[cfg(foo)] {
            fn works3() -> bool { false }
        } else {
            fn works3() -> bool { true }
        }
    }

    cfg_if! {
        if #[cfg(test)] {
            use core::option::Option as Option3;
            fn works4() -> Option3<u32> { Some(1) }
        }
    }

    cfg_if! {
        if #[cfg(foo)] {
            fn works5() -> bool { false }
        } else if #[cfg(test)] {
            fn works5() -> bool { true }
        }
    }

    #[test]
    fn it_works() {
        assert!(works1().is_some());
        assert!(works2());
        assert!(works3());
        assert!(works4().is_some());
        assert!(works5());
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_usage_within_a_function() {
        cfg_if! {if #[cfg(debug_assertions)] {
            // we want to put more than one thing here to make sure that they
            // all get configured properly.
            assert!(cfg!(debug_assertions));
            assert_eq!(4, 2+2);
        } else {
            assert!(works1().is_some());
            assert_eq!(10, 5+5);
        }}
    }

    #[allow(dead_code)]
    trait Trait {
        fn blah(&self);
    }

    #[allow(dead_code)]
    struct Struct;

    impl Trait for Struct {
        cfg_if! {
            if #[cfg(feature = "blah")] {
                fn blah(&self) {
                    unimplemented!();
                }
            } else {
                fn blah(&self) {
                    unimplemented!();
                }
            }
        }
    }
}
