#![allow(
    clippy::semicolon_if_nothing_returned, // https://github.com/rust-lang/rust-clippy/issues/7324
    clippy::used_underscore_items,
)]

#[rustversion::attr(all(), const)]
fn _basic() {}
const _BASIC: () = _basic();

#[rustversion::attr(all(), const)]
unsafe fn _unsafe() {}
const _UNSAFE: () = unsafe { _unsafe() };

macro_rules! item {
    ($i:item) => {
        #[rustversion::attr(all(), const)]
        $i
    };
}

item! {fn _item() {}}
const _ITEM: () = _item();

macro_rules! ident {
    ($fn:ident) => {
        #[rustversion::attr(all(), const)]
        $fn _ident() {}
    };
}

ident! {fn}
const _IDENT: () = _ident();

#[rustversion::attr(all(), const)]
/// doc
fn _doc_below() {}
const _DOC_BELOW: () = _doc_below();

/// doc
#[rustversion::attr(all(), const)]
fn _doc_above() {}
const _DOC_ABOVE: () = _doc_above();
