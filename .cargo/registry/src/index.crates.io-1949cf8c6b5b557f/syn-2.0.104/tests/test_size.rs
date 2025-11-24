// Assumes proc-macro2's "span-locations" feature is off.

use std::mem;
use syn::{Expr, Item, Lit, Pat, Type};

#[rustversion::attr(before(2022-11-24), ignore = "requires nightly-2022-11-24 or newer")]
#[rustversion::attr(
    since(2022-11-24),
    cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")
)]
#[test]
fn test_expr_size() {
    assert_eq!(mem::size_of::<Expr>(), 176);
}

#[rustversion::attr(before(2022-09-09), ignore = "requires nightly-2022-09-09 or newer")]
#[rustversion::attr(
    since(2022-09-09),
    cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")
)]
#[test]
fn test_item_size() {
    assert_eq!(mem::size_of::<Item>(), 352);
}

#[rustversion::attr(before(2023-04-29), ignore = "requires nightly-2023-04-29 or newer")]
#[rustversion::attr(
    since(2023-04-29),
    cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")
)]
#[test]
fn test_type_size() {
    assert_eq!(mem::size_of::<Type>(), 224);
}

#[rustversion::attr(before(2023-04-29), ignore = "requires nightly-2023-04-29 or newer")]
#[rustversion::attr(
    since(2023-04-29),
    cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")
)]
#[test]
fn test_pat_size() {
    assert_eq!(mem::size_of::<Pat>(), 184);
}

#[rustversion::attr(before(2023-12-20), ignore = "requires nightly-2023-12-20 or newer")]
#[rustversion::attr(
    since(2023-12-20),
    cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")
)]
#[test]
fn test_lit_size() {
    assert_eq!(mem::size_of::<Lit>(), 24);
}
