#![allow(unused_attributes)]

extern crate proc_macro;

use std::mem;

#[rustversion::attr(before(1.64), ignore = "requires Rust 1.64+")]
#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[test]
fn test_proc_macro_size() {
    assert_eq!(mem::size_of::<proc_macro::Span>(), 4);
    assert_eq!(mem::size_of::<Option<proc_macro::Span>>(), 4);
    assert_eq!(mem::size_of::<proc_macro::Group>(), 20);
    assert_eq!(mem::size_of::<proc_macro::Ident>(), 12);
    assert_eq!(mem::size_of::<proc_macro::Punct>(), 8);
    assert_eq!(mem::size_of::<proc_macro::Literal>(), 16);
    assert_eq!(mem::size_of::<proc_macro::TokenStream>(), 4);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(wrap_proc_macro, ignore = "wrapper mode")]
#[cfg_attr(span_locations, ignore = "span locations are on")]
#[test]
fn test_proc_macro2_fallback_size_without_locations() {
    assert_eq!(mem::size_of::<proc_macro2::Span>(), 0);
    assert_eq!(mem::size_of::<Option<proc_macro2::Span>>(), 1);
    assert_eq!(mem::size_of::<proc_macro2::Group>(), 16);
    assert_eq!(mem::size_of::<proc_macro2::Ident>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::Punct>(), 8);
    assert_eq!(mem::size_of::<proc_macro2::Literal>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::TokenStream>(), 8);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(wrap_proc_macro, ignore = "wrapper mode")]
#[cfg_attr(not(span_locations), ignore = "span locations are off")]
#[test]
fn test_proc_macro2_fallback_size_with_locations() {
    assert_eq!(mem::size_of::<proc_macro2::Span>(), 8);
    assert_eq!(mem::size_of::<Option<proc_macro2::Span>>(), 12);
    assert_eq!(mem::size_of::<proc_macro2::Group>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::Ident>(), 32);
    assert_eq!(mem::size_of::<proc_macro2::Punct>(), 16);
    assert_eq!(mem::size_of::<proc_macro2::Literal>(), 32);
    assert_eq!(mem::size_of::<proc_macro2::TokenStream>(), 8);
}

#[rustversion::attr(before(1.71), ignore = "requires Rust 1.71+")]
#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(not(wrap_proc_macro), ignore = "fallback mode")]
#[cfg_attr(span_locations, ignore = "span locations are on")]
#[test]
fn test_proc_macro2_wrapper_size_without_locations() {
    assert_eq!(mem::size_of::<proc_macro2::Span>(), 4);
    assert_eq!(mem::size_of::<Option<proc_macro2::Span>>(), 8);
    assert_eq!(mem::size_of::<proc_macro2::Group>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::Ident>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::Punct>(), 12);
    assert_eq!(mem::size_of::<proc_macro2::Literal>(), 24);
    assert_eq!(mem::size_of::<proc_macro2::TokenStream>(), 32);
}

#[rustversion::attr(before(1.65), ignore = "requires Rust 1.65+")]
#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(not(wrap_proc_macro), ignore = "fallback mode")]
#[cfg_attr(not(span_locations), ignore = "span locations are off")]
#[test]
fn test_proc_macro2_wrapper_size_with_locations() {
    assert_eq!(mem::size_of::<proc_macro2::Span>(), 12);
    assert_eq!(mem::size_of::<Option<proc_macro2::Span>>(), 12);
    assert_eq!(mem::size_of::<proc_macro2::Group>(), 32);
    assert_eq!(mem::size_of::<proc_macro2::Ident>(), 32);
    assert_eq!(mem::size_of::<proc_macro2::Punct>(), 20);
    assert_eq!(mem::size_of::<proc_macro2::Literal>(), 32);
    assert_eq!(mem::size_of::<proc_macro2::TokenStream>(), 32);
}
