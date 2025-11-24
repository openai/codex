#![allow(missing_docs)]

//! Document tree traversal to mutate an exclusive borrow of a document tree in place.
//!
//!
//! Each method of the [`VisitMut`] trait is a hook that can be overridden
//! to customize the behavior when mutating the corresponding type of node.
//! By default, every method recursively visits the substructure of the
//! input by invoking the right visitor method of each of its fields.
//!
//! ```
//! # use toml_edit::{Item, ArrayOfTables, Table, Value};
//!
//! pub trait VisitMut {
//!     /* ... */
//!
//!     fn visit_item_mut(&mut self, i: &mut Item) {
//!         visit_item_mut(self, i);
//!     }
//!
//!     /* ... */
//!     # fn visit_value_mut(&mut self, i: &mut Value);
//!     # fn visit_table_mut(&mut self, i: &mut Table);
//!     # fn visit_array_of_tables_mut(&mut self, i: &mut ArrayOfTables);
//! }
//!
//! pub fn visit_item_mut<V>(v: &mut V, node: &mut Item)
//! where
//!     V: VisitMut + ?Sized,
//! {
//!     match node {
//!         Item::None => {}
//!         Item::Value(value) => v.visit_value_mut(value),
//!         Item::Table(table) => v.visit_table_mut(table),
//!         Item::ArrayOfTables(array) => v.visit_array_of_tables_mut(array),
//!     }
//! }
//! ```
//!
//! The API is modeled after [`syn::visit_mut`](https://docs.rs/syn/1/syn/visit_mut).
//!
//! # Examples
//!
//! This visitor replaces every floating point value with its decimal string representation, to
//! 2 decimal points.
//!
//! ```
//! # #[cfg(feature = "parse")] {
//! # #[cfg(feature = "display")] {
//! # use toml_edit::*;
//! use toml_edit::visit_mut::*;
//!
//! struct FloatToString;
//!
//! impl VisitMut for FloatToString {
//!     fn visit_value_mut(&mut self, node: &mut Value) {
//!         if let Value::Float(f) = node {
//!             // Convert the float to a string.
//!             let mut s = Formatted::new(format!("{:.2}", f.value()));
//!             // Copy over the formatting.
//!             std::mem::swap(s.decor_mut(), f.decor_mut());
//!             *node = Value::String(s);
//!         }
//!         // Most of the time, you will also need to call the default implementation to recurse
//!         // further down the document tree.
//!         visit_value_mut(self, node);
//!     }
//! }
//!
//! let input = r#"
//! banana = 3.26
//! table = { apple = 4.5 }
//! "#;
//!
//! let mut document: DocumentMut = input.parse().unwrap();
//! let mut visitor = FloatToString;
//! visitor.visit_document_mut(&mut document);
//!
//! let output = r#"
//! banana = "3.26"
//! table = { apple = "4.50" }
//! "#;
//!
//! assert_eq!(format!("{}", document), output);
//! # }
//! # }
//! ```
//!
//! For a more complex example where the visitor has internal state, see `examples/visit.rs`
//! [on GitHub](https://github.com/toml-rs/toml/blob/main/crates/toml_edit/examples/visit.rs).

use crate::{
    Array, ArrayOfTables, Datetime, DocumentMut, Formatted, InlineTable, Item, KeyMut, Table,
    TableLike, Value,
};

/// Document tree traversal to mutate an exclusive borrow of a document tree in-place.
///
/// See the [module documentation](self) for details.
pub trait VisitMut {
    fn visit_document_mut(&mut self, node: &mut DocumentMut) {
        visit_document_mut(self, node);
    }

    fn visit_item_mut(&mut self, node: &mut Item) {
        visit_item_mut(self, node);
    }

    fn visit_table_mut(&mut self, node: &mut Table) {
        visit_table_mut(self, node);
    }

    fn visit_inline_table_mut(&mut self, node: &mut InlineTable) {
        visit_inline_table_mut(self, node);
    }

    /// [`visit_table_mut`](Self::visit_table_mut) and
    /// [`visit_inline_table_mut`](Self::visit_inline_table_mut) both recurse into this method.
    fn visit_table_like_mut(&mut self, node: &mut dyn TableLike) {
        visit_table_like_mut(self, node);
    }

    fn visit_table_like_kv_mut(&mut self, key: KeyMut<'_>, node: &mut Item) {
        visit_table_like_kv_mut(self, key, node);
    }

    fn visit_array_mut(&mut self, node: &mut Array) {
        visit_array_mut(self, node);
    }

    fn visit_array_of_tables_mut(&mut self, node: &mut ArrayOfTables) {
        visit_array_of_tables_mut(self, node);
    }

    fn visit_value_mut(&mut self, node: &mut Value) {
        visit_value_mut(self, node);
    }

    fn visit_boolean_mut(&mut self, node: &mut Formatted<bool>) {
        visit_boolean_mut(self, node);
    }

    fn visit_datetime_mut(&mut self, node: &mut Formatted<Datetime>) {
        visit_datetime_mut(self, node);
    }

    fn visit_float_mut(&mut self, node: &mut Formatted<f64>) {
        visit_float_mut(self, node);
    }

    fn visit_integer_mut(&mut self, node: &mut Formatted<i64>) {
        visit_integer_mut(self, node);
    }

    fn visit_string_mut(&mut self, node: &mut Formatted<String>) {
        visit_string_mut(self, node);
    }
}

pub fn visit_document_mut<V>(v: &mut V, node: &mut DocumentMut)
where
    V: VisitMut + ?Sized,
{
    v.visit_table_mut(node.as_table_mut());
}

pub fn visit_item_mut<V>(v: &mut V, node: &mut Item)
where
    V: VisitMut + ?Sized,
{
    match node {
        Item::None => {}
        Item::Value(value) => v.visit_value_mut(value),
        Item::Table(table) => v.visit_table_mut(table),
        Item::ArrayOfTables(array) => v.visit_array_of_tables_mut(array),
    }
}

pub fn visit_table_mut<V>(v: &mut V, node: &mut Table)
where
    V: VisitMut + ?Sized,
{
    v.visit_table_like_mut(node);
}

pub fn visit_inline_table_mut<V>(v: &mut V, node: &mut InlineTable)
where
    V: VisitMut + ?Sized,
{
    v.visit_table_like_mut(node);
}

pub fn visit_table_like_mut<V>(v: &mut V, node: &mut dyn TableLike)
where
    V: VisitMut + ?Sized,
{
    for (key, item) in node.iter_mut() {
        v.visit_table_like_kv_mut(key, item);
    }
}

pub fn visit_table_like_kv_mut<V>(v: &mut V, _key: KeyMut<'_>, node: &mut Item)
where
    V: VisitMut + ?Sized,
{
    v.visit_item_mut(node);
}

pub fn visit_array_mut<V>(v: &mut V, node: &mut Array)
where
    V: VisitMut + ?Sized,
{
    for value in node.iter_mut() {
        v.visit_value_mut(value);
    }
}

pub fn visit_array_of_tables_mut<V>(v: &mut V, node: &mut ArrayOfTables)
where
    V: VisitMut + ?Sized,
{
    for table in node.iter_mut() {
        v.visit_table_mut(table);
    }
}

pub fn visit_value_mut<V>(v: &mut V, node: &mut Value)
where
    V: VisitMut + ?Sized,
{
    match node {
        Value::String(s) => v.visit_string_mut(s),
        Value::Integer(i) => v.visit_integer_mut(i),
        Value::Float(f) => v.visit_float_mut(f),
        Value::Boolean(b) => v.visit_boolean_mut(b),
        Value::Datetime(dt) => v.visit_datetime_mut(dt),
        Value::Array(array) => v.visit_array_mut(array),
        Value::InlineTable(table) => v.visit_inline_table_mut(table),
    }
}

macro_rules! empty_visit_mut {
    ($name: ident, $t: ty) => {
        fn $name<V>(_v: &mut V, _node: &mut $t)
        where
            V: VisitMut + ?Sized,
        {
        }
    };
}

empty_visit_mut!(visit_boolean_mut, Formatted<bool>);
empty_visit_mut!(visit_datetime_mut, Formatted<Datetime>);
empty_visit_mut!(visit_float_mut, Formatted<f64>);
empty_visit_mut!(visit_integer_mut, Formatted<i64>);
empty_visit_mut!(visit_string_mut, Formatted<String>);
