# synstructure

[![Latest Version](https://img.shields.io/crates/v/synstructure.svg)](https://crates.io/crates/synstructure)
[![Documentation](https://docs.rs/synstructure/badge.svg)](https://docs.rs/synstructure)
[![Build Status](https://github.com/mystor/synstructure/actions/workflows/ci.yml/badge.svg)](https://github.com/mystor/synstructure/actions)
[![Rustc Version 1.31+](https://img.shields.io/badge/rustc-1.31+-lightgray.svg)](https://blog.rust-lang.org/2018/12/06/Rust-1.31-and-rust-2018.html)

> NOTE: What follows is an exerpt from the module level documentation. For full
> details read the docs on [docs.rs](https://docs.rs/synstructure/)

This crate provides helper types for matching against enum variants, and
extracting bindings to each of the fields in the deriving Struct or Enum in
a generic way.

If you are writing a `#[derive]` which needs to perform some operation on
every field, then you have come to the right place!

# Example: `WalkFields`
### Trait Implementation
```rust
pub trait WalkFields: std::any::Any {
    fn walk_fields(&self, walk: &mut FnMut(&WalkFields));
}
impl WalkFields for i32 {
    fn walk_fields(&self, _walk: &mut FnMut(&WalkFields)) {}
}
```

### Custom Derive
```rust
#[macro_use]
extern crate synstructure;
#[macro_use]
extern crate quote;
extern crate proc_macro2;

fn walkfields_derive(s: synstructure::Structure) -> proc_macro2::TokenStream {
    let body = s.each(|bi| quote!{
        walk(#bi)
    });

    s.bound_impl(quote!(example_traits::WalkFields), quote!{
        fn walk_fields(&self, walk: &mut FnMut(&example_traits::WalkFields)) {
            match *self { #body }
        }
    })
}
decl_derive!([WalkFields] => walkfields_derive);

/*
 * Test Case
 */
fn main() {
    test_derive! {
        walkfields_derive {
            enum A<T> {
                B(i32, T),
                C(i32),
            }
        }
        expands to {
            const _: () = {
                extern crate example_traits;
                impl<T> example_traits::WalkFields for A<T>
                    where T: example_traits::WalkFields
                {
                    fn walk_fields(&self, walk: &mut FnMut(&example_traits::WalkFields)) {
                        match *self {
                            A::B(ref __binding_0, ref __binding_1,) => {
                                { walk(__binding_0) }
                                { walk(__binding_1) }
                            }
                            A::C(ref __binding_0,) => {
                                { walk(__binding_0) }
                            }
                        }
                    }
                }
            };
        }
    }
}
```

# Example: `Interest`
### Trait Implementation
```rust
pub trait Interest {
    fn interesting(&self) -> bool;
}
impl Interest for i32 {
    fn interesting(&self) -> bool { *self > 0 }
}
```

### Custom Derive
```rust
#[macro_use]
extern crate synstructure;
#[macro_use]
extern crate quote;
extern crate proc_macro2;

fn interest_derive(mut s: synstructure::Structure) -> proc_macro2::TokenStream {
    let body = s.fold(false, |acc, bi| quote!{
        #acc || example_traits::Interest::interesting(#bi)
    });

    s.bound_impl(quote!(example_traits::Interest), quote!{
        fn interesting(&self) -> bool {
            match *self {
                #body
            }
        }
    })
}
decl_derive!([Interest] => interest_derive);

/*
 * Test Case
 */
fn main() {
    test_derive!{
        interest_derive {
            enum A<T> {
                B(i32, T),
                C(i32),
            }
        }
        expands to {
            const _: () = {
                extern crate example_traits;
                impl<T> example_traits::Interest for A<T>
                    where T: example_traits::Interest
                {
                    fn interesting(&self) -> bool {
                        match *self {
                            A::B(ref __binding_0, ref __binding_1,) => {
                                false ||
                                    example_traits::Interest::interesting(__binding_0) ||
                                    example_traits::Interest::interesting(__binding_1)
                            }
                            A::C(ref __binding_0,) => {
                                false ||
                                    example_traits::Interest::interesting(__binding_0)
                            }
                        }
                    }
                }
            };
        }
    }
}
```

For more example usage, consider investigating the `abomonation_derive` crate,
which makes use of this crate, and is fairly simple.
