#![cfg(target_pointer_width = "64")]

use std::mem;
use syn::{Expr, Item, Lit, Pat, Type};

#[test]
fn test_expr_size() {
    assert_eq!(mem::size_of::<Expr>(), 272);
}

#[test]
fn test_item_size() {
    assert_eq!(mem::size_of::<Item>(), 320);
}

#[test]
fn test_type_size() {
    assert_eq!(mem::size_of::<Type>(), 288);
}

#[test]
fn test_pat_size() {
    assert_eq!(mem::size_of::<Pat>(), 144);
}

#[test]
fn test_lit_size() {
    assert_eq!(mem::size_of::<Lit>(), 32);
}
