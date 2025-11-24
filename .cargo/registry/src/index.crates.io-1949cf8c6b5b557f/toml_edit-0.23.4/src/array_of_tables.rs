use std::iter::FromIterator;

use crate::{Array, Item, Table};

/// A top-level sequence of [`Table`]s, each under their own header
#[derive(Clone, Debug, Default)]
pub struct ArrayOfTables {
    // Always Vec<Item::Table>, just `Item` to make `Index` work
    pub(crate) span: Option<std::ops::Range<usize>>,
    pub(crate) values: Vec<Item>,
}

/// Constructors
///
/// See also `FromIterator`
impl ArrayOfTables {
    /// Creates an empty array of tables.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Formatting
impl ArrayOfTables {
    /// Convert to an inline array
    pub fn into_array(mut self) -> Array {
        for value in self.values.iter_mut() {
            value.make_value();
        }
        let mut a = Array::with_vec(self.values);
        a.fmt();
        a
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.span.clone()
    }

    pub(crate) fn despan(&mut self, input: &str) {
        self.span = None;
        for value in &mut self.values {
            value.despan(input);
        }
    }
}

impl ArrayOfTables {
    /// Returns an iterator over tables.
    pub fn iter(&self) -> ArrayOfTablesIter<'_> {
        Box::new(self.values.iter().filter_map(Item::as_table))
    }

    /// Returns an iterator over tables.
    pub fn iter_mut(&mut self) -> ArrayOfTablesIterMut<'_> {
        Box::new(self.values.iter_mut().filter_map(Item::as_table_mut))
    }

    /// Returns the length of the underlying Vec.
    /// To get the actual number of items use `a.iter().count()`.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if `self.len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes all the tables.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Returns an optional reference to the table.
    pub fn get(&self, index: usize) -> Option<&Table> {
        self.values.get(index).and_then(Item::as_table)
    }

    /// Returns an optional mutable reference to the table.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Table> {
        self.values.get_mut(index).and_then(Item::as_table_mut)
    }

    /// Appends a table to the array.
    pub fn push(&mut self, table: Table) {
        self.values.push(Item::Table(table));
    }

    /// Removes a table with the given index.
    pub fn remove(&mut self, index: usize) -> Table {
        self.values
            .remove(index)
            .into_table()
            .expect("cannot have any other item in an array-of-tables")
    }

    /// Retains only the elements specified by the `keep` predicate.
    ///
    /// In other words, remove all tables for which `keep(&table)` returns `false`.
    ///
    /// This method operates in place, visiting each element exactly once in the
    /// original order, and preserves the order of the retained elements.
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&Table) -> bool,
    {
        self.values
            .retain(|item| item.as_table().map(&mut keep).unwrap_or(false));
    }
}

/// An iterator type over [`ArrayOfTables`]'s [`Table`]s
pub type ArrayOfTablesIter<'a> = Box<dyn Iterator<Item = &'a Table> + 'a>;
/// An iterator type over [`ArrayOfTables`]'s [`Table`]s
pub type ArrayOfTablesIterMut<'a> = Box<dyn Iterator<Item = &'a mut Table> + 'a>;
/// An iterator type over [`ArrayOfTables`]'s [`Table`]s
pub type ArrayOfTablesIntoIter = Box<dyn Iterator<Item = Table>>;

impl Extend<Table> for ArrayOfTables {
    fn extend<T: IntoIterator<Item = Table>>(&mut self, iter: T) {
        for value in iter {
            self.push(value);
        }
    }
}

impl FromIterator<Table> for ArrayOfTables {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Table>,
    {
        let v = iter.into_iter().map(Item::Table);
        Self {
            values: v.collect(),
            span: None,
        }
    }
}

impl IntoIterator for ArrayOfTables {
    type Item = Table;
    type IntoIter = ArrayOfTablesIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            self.values
                .into_iter()
                .filter(|v| v.is_table())
                .map(|v| v.into_table().unwrap()),
        )
    }
}

impl<'s> IntoIterator for &'s ArrayOfTables {
    type Item = &'s Table;
    type IntoIter = ArrayOfTablesIter<'s>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(feature = "display")]
impl std::fmt::Display for ArrayOfTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // HACK: Without the header, we don't really have a proper way of printing this
        self.clone().into_array().fmt(f)
    }
}
