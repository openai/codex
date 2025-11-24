use std::iter::FromIterator;

use crate::key::Key;
use crate::repr::Decor;
use crate::table::{Iter, IterMut, KeyValuePairs, TableLike};
use crate::{Item, KeyMut, RawString, Table, Value};

/// A TOML [`Value`] that contains a collection of [`Key`]/[`Value`] pairs
#[derive(Debug, Default, Clone)]
pub struct InlineTable {
    // `preamble` represents whitespaces in an empty table
    preamble: RawString,
    // Whether to hide an empty table
    pub(crate) implicit: bool,
    // prefix before `{` and suffix after `}`
    decor: Decor,
    pub(crate) span: Option<std::ops::Range<usize>>,
    // whether this is a proxy for dotted keys
    dotted: bool,
    pub(crate) items: KeyValuePairs,
}

/// Constructors
///
/// See also `FromIterator`
impl InlineTable {
    /// Creates an empty table.
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn with_pairs(items: KeyValuePairs) -> Self {
        Self {
            items,
            ..Default::default()
        }
    }

    /// Convert to a table
    pub fn into_table(self) -> Table {
        let mut t = Table::with_pairs(self.items);
        t.fmt();
        t
    }
}

/// Formatting
impl InlineTable {
    /// Get key/values for values that are visually children of this table
    ///
    /// For example, this will return dotted keys
    pub fn get_values(&self) -> Vec<(Vec<&Key>, &Value)> {
        let mut values = Vec::new();
        let root = Vec::new();
        self.append_values(&root, &mut values);
        values
    }

    pub(crate) fn append_values<'s>(
        &'s self,
        parent: &[&'s Key],
        values: &mut Vec<(Vec<&'s Key>, &'s Value)>,
    ) {
        for (key, value) in self.items.iter() {
            let mut path = parent.to_vec();
            path.push(key);
            match value {
                Item::Value(Value::InlineTable(table)) if table.is_dotted() => {
                    table.append_values(&path, values);
                }
                Item::Value(value) => {
                    values.push((path, value));
                }
                Item::Table(table) => {
                    table.append_all_values(&path, values);
                }
                _ => {}
            }
        }
    }

    /// Auto formats the table.
    pub fn fmt(&mut self) {
        decorate_inline_table(self);
    }

    /// Sorts [Key]/[Value]-pairs of the table
    ///
    /// <div class="warning">
    ///
    /// This is not recursive.
    ///
    /// </div>
    pub fn sort_values(&mut self) {
        // Assuming standard tables have their position set and this won't negatively impact them
        self.items.sort_keys();
        for value in self.items.values_mut() {
            match value {
                Item::Value(Value::InlineTable(table)) if table.is_dotted() => {
                    table.sort_values();
                }
                _ => {}
            }
        }
    }

    /// Sort [Key]/[Value]-pairs of the table using the using the comparison function `compare`
    ///
    /// The comparison function receives two key and value pairs to compare (you can sort by keys or
    /// values or their combination as needed).
    ///
    /// <div class="warning">
    ///
    /// This is not recursive.
    ///
    /// </div>
    pub fn sort_values_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&Key, &Value, &Key, &Value) -> std::cmp::Ordering,
    {
        self.sort_values_by_internal(&mut compare);
    }

    fn sort_values_by_internal<F>(&mut self, compare: &mut F)
    where
        F: FnMut(&Key, &Value, &Key, &Value) -> std::cmp::Ordering,
    {
        let modified_cmp =
            |key1: &Key, val1: &Item, key2: &Key, val2: &Item| -> std::cmp::Ordering {
                match (val1.as_value(), val2.as_value()) {
                    (Some(v1), Some(v2)) => compare(key1, v1, key2, v2),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            };

        self.items.sort_by(modified_cmp);
        for value in self.items.values_mut() {
            match value {
                Item::Value(Value::InlineTable(table)) if table.is_dotted() => {
                    table.sort_values_by_internal(compare);
                }
                _ => {}
            }
        }
    }

    /// If a table has no key/value pairs and implicit, it will not be displayed.
    ///
    /// # Examples
    ///
    /// ```notrust
    /// [target."x86_64/windows.json".dependencies]
    /// ```
    ///
    /// In the document above, tables `target` and `target."x86_64/windows.json"` are implicit.
    ///
    /// ```
    /// # #[cfg(feature = "parse")] {
    /// # #[cfg(feature = "display")] {
    /// use toml_edit::DocumentMut;
    /// let mut doc = "[a]\n[a.b]\n".parse::<DocumentMut>().expect("invalid toml");
    ///
    /// doc["a"].as_table_mut().unwrap().set_implicit(true);
    /// assert_eq!(doc.to_string(), "[a.b]\n");
    /// # }
    /// # }
    /// ```
    pub(crate) fn set_implicit(&mut self, implicit: bool) {
        self.implicit = implicit;
    }

    /// If a table has no key/value pairs and implicit, it will not be displayed.
    pub(crate) fn is_implicit(&self) -> bool {
        self.implicit
    }

    /// Change this table's dotted status
    pub fn set_dotted(&mut self, yes: bool) {
        self.dotted = yes;
    }

    /// Check if this is a wrapper for dotted keys, rather than a standard table
    pub fn is_dotted(&self) -> bool {
        self.dotted
    }

    /// Returns the surrounding whitespace
    pub fn decor_mut(&mut self) -> &mut Decor {
        &mut self.decor
    }

    /// Returns the surrounding whitespace
    pub fn decor(&self) -> &Decor {
        &self.decor
    }

    /// Returns an accessor to a key's formatting
    pub fn key(&self, key: &str) -> Option<&'_ Key> {
        self.items.get_full(key).map(|(_, key, _)| key)
    }

    /// Returns an accessor to a key's formatting
    pub fn key_mut(&mut self, key: &str) -> Option<KeyMut<'_>> {
        use indexmap::map::MutableKeys;
        self.items
            .get_full_mut2(key)
            .map(|(_, key, _)| key.as_mut())
    }

    /// Set whitespace after before element
    pub fn set_preamble(&mut self, preamble: impl Into<RawString>) {
        self.preamble = preamble.into();
    }

    /// Whitespace after before element
    pub fn preamble(&self) -> &RawString {
        &self.preamble
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.span.clone()
    }

    pub(crate) fn despan(&mut self, input: &str) {
        use indexmap::map::MutableKeys;
        self.span = None;
        self.decor.despan(input);
        self.preamble.despan(input);
        for (key, value) in self.items.iter_mut2() {
            key.despan(input);
            value.despan(input);
        }
    }
}

impl InlineTable {
    /// Returns an iterator over key/value pairs.
    pub fn iter(&self) -> InlineTableIter<'_> {
        Box::new(
            self.items
                .iter()
                .filter(|(_, value)| !value.is_none())
                .map(|(key, value)| (key.get(), value.as_value().unwrap())),
        )
    }

    /// Returns an iterator over key/value pairs.
    pub fn iter_mut(&mut self) -> InlineTableIterMut<'_> {
        use indexmap::map::MutableKeys;
        Box::new(
            self.items
                .iter_mut2()
                .filter(|(_, value)| value.is_value())
                .map(|(key, value)| (key.as_mut(), value.as_value_mut().unwrap())),
        )
    }

    /// Returns the number of key/value pairs.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns true if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the table, removing all key-value pairs. Keeps the allocated memory for reuse.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Gets the given key's corresponding entry in the Table for in-place manipulation.
    pub fn entry(&'_ mut self, key: impl Into<String>) -> InlineEntry<'_> {
        match self.items.entry(key.into().into()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                // Ensure it is a `Value` to simplify `InlineOccupiedEntry`'s code.
                let scratch = std::mem::take(entry.get_mut());
                let scratch = Item::Value(
                    scratch
                        .into_value()
                        // HACK: `Item::None` is a corner case of a corner case, let's just pick a
                        // "safe" value
                        .unwrap_or_else(|_| Value::InlineTable(Default::default())),
                );
                *entry.get_mut() = scratch;

                InlineEntry::Occupied(InlineOccupiedEntry { entry })
            }
            indexmap::map::Entry::Vacant(entry) => InlineEntry::Vacant(InlineVacantEntry { entry }),
        }
    }

    /// Gets the given key's corresponding entry in the Table for in-place manipulation.
    pub fn entry_format<'a>(&'a mut self, key: &Key) -> InlineEntry<'a> {
        // Accept a `&Key` to be consistent with `entry`
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                // Ensure it is a `Value` to simplify `InlineOccupiedEntry`'s code.
                let scratch = std::mem::take(entry.get_mut());
                let scratch = Item::Value(
                    scratch
                        .into_value()
                        // HACK: `Item::None` is a corner case of a corner case, let's just pick a
                        // "safe" value
                        .unwrap_or_else(|_| Value::InlineTable(Default::default())),
                );
                *entry.get_mut() = scratch;

                InlineEntry::Occupied(InlineOccupiedEntry { entry })
            }
            indexmap::map::Entry::Vacant(entry) => InlineEntry::Vacant(InlineVacantEntry { entry }),
        }
    }
    /// Return an optional reference to the value at the given the key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.items.get(key).and_then(|value| value.as_value())
    }

    /// Return an optional mutable reference to the value at the given the key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.items
            .get_mut(key)
            .and_then(|value| value.as_value_mut())
    }

    /// Return references to the key-value pair stored for key, if it is present, else None.
    pub fn get_key_value<'a>(&'a self, key: &str) -> Option<(&'a Key, &'a Item)> {
        self.items.get_full(key).and_then(|(_, key, value)| {
            if !value.is_none() {
                Some((key, value))
            } else {
                None
            }
        })
    }

    /// Return mutable references to the key-value pair stored for key, if it is present, else None.
    pub fn get_key_value_mut<'a>(&'a mut self, key: &str) -> Option<(KeyMut<'a>, &'a mut Item)> {
        use indexmap::map::MutableKeys;
        self.items.get_full_mut2(key).and_then(|(_, key, value)| {
            if !value.is_none() {
                Some((key.as_mut(), value))
            } else {
                None
            }
        })
    }

    /// Returns true if the table contains given key.
    pub fn contains_key(&self, key: &str) -> bool {
        if let Some(value) = self.items.get(key) {
            value.is_value()
        } else {
            false
        }
    }

    /// Inserts a key/value pair if the table does not contain the key.
    /// Returns a mutable reference to the corresponding value.
    pub fn get_or_insert<V: Into<Value>>(
        &mut self,
        key: impl Into<String>,
        value: V,
    ) -> &mut Value {
        let key = key.into();
        self.items
            .entry(Key::new(key))
            .or_insert(Item::Value(value.into()))
            .as_value_mut()
            .expect("non-value type in inline table")
    }

    /// Inserts a key-value pair into the map.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        use indexmap::map::MutableEntryKey;
        let key = Key::new(key);
        let value = Item::Value(value);
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                entry.key_mut().fmt();
                let old = std::mem::replace(entry.get_mut(), value);
                old.into_value().ok()
            }
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    /// Inserts a key-value pair into the map.
    pub fn insert_formatted(&mut self, key: &Key, value: Value) -> Option<Value> {
        use indexmap::map::MutableEntryKey;
        let value = Item::Value(value);
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                *entry.key_mut() = key.clone();
                let old = std::mem::replace(entry.get_mut(), value);
                old.into_value().ok()
            }
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    /// Removes an item given the key.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.items
            .shift_remove(key)
            .and_then(|value| value.into_value().ok())
    }

    /// Removes a key from the map, returning the stored key and value if the key was previously in the map.
    pub fn remove_entry(&mut self, key: &str) -> Option<(Key, Value)> {
        self.items
            .shift_remove_entry(key)
            .and_then(|(key, value)| Some((key, value.into_value().ok()?)))
    }

    /// Retains only the elements specified by the `keep` predicate.
    ///
    /// In other words, remove all pairs `(key, value)` for which
    /// `keep(&key, &mut value)` returns `false`.
    ///
    /// The elements are visited in iteration order.
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&str, &mut Value) -> bool,
    {
        self.items.retain(|key, item| {
            item.as_value_mut()
                .map(|value| keep(key, value))
                .unwrap_or(false)
        });
    }
}

#[cfg(feature = "display")]
impl std::fmt::Display for InlineTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::encode::encode_table(self, f, None, ("", ""))
    }
}

impl<K: Into<Key>, V: Into<Value>> Extend<(K, V)> for InlineTable {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (key, value) in iter {
            let key = key.into();
            let value = Item::Value(value.into());
            self.items.insert(key, value);
        }
    }
}

impl<K: Into<Key>, V: Into<Value>> FromIterator<(K, V)> for InlineTable {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut table = Self::new();
        table.extend(iter);
        table
    }
}

impl IntoIterator for InlineTable {
    type Item = (String, Value);
    type IntoIter = InlineTableIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(
            self.items
                .into_iter()
                .filter(|(_, value)| value.is_value())
                .map(|(key, value)| (key.into(), value.into_value().unwrap())),
        )
    }
}

impl<'s> IntoIterator for &'s InlineTable {
    type Item = (&'s str, &'s Value);
    type IntoIter = InlineTableIter<'s>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn decorate_inline_table(table: &mut InlineTable) {
    use indexmap::map::MutableKeys;
    for (mut key, value) in table
        .items
        .iter_mut2()
        .filter(|(_, value)| value.is_value())
        .map(|(key, value)| (key.as_mut(), value.as_value_mut().unwrap()))
    {
        key.leaf_decor_mut().clear();
        key.dotted_decor_mut().clear();
        value.decor_mut().clear();
    }
}

/// An owned iterator type over an [`InlineTable`]'s [`Key`]/[`Value`] pairs
pub type InlineTableIntoIter = Box<dyn Iterator<Item = (String, Value)>>;
/// An iterator type over [`InlineTable`]'s [`Key`]/[`Value`] pairs
pub type InlineTableIter<'a> = Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>;
/// A mutable iterator type over [`InlineTable`]'s [`Key`]/[`Value`] pairs
pub type InlineTableIterMut<'a> = Box<dyn Iterator<Item = (KeyMut<'a>, &'a mut Value)> + 'a>;

impl TableLike for InlineTable {
    fn iter(&self) -> Iter<'_> {
        Box::new(self.items.iter().map(|(key, value)| (key.get(), value)))
    }
    fn iter_mut(&mut self) -> IterMut<'_> {
        use indexmap::map::MutableKeys;
        Box::new(
            self.items
                .iter_mut2()
                .map(|(key, value)| (key.as_mut(), value)),
        )
    }
    fn clear(&mut self) {
        self.clear();
    }
    fn entry<'a>(&'a mut self, key: &str) -> crate::Entry<'a> {
        // Accept a `&str` rather than an owned type to keep `String`, well, internal
        match self.items.entry(key.into()) {
            indexmap::map::Entry::Occupied(entry) => {
                crate::Entry::Occupied(crate::OccupiedEntry { entry })
            }
            indexmap::map::Entry::Vacant(entry) => {
                crate::Entry::Vacant(crate::VacantEntry { entry })
            }
        }
    }
    fn entry_format<'a>(&'a mut self, key: &Key) -> crate::Entry<'a> {
        // Accept a `&Key` to be consistent with `entry`
        match self.items.entry(key.get().into()) {
            indexmap::map::Entry::Occupied(entry) => {
                crate::Entry::Occupied(crate::OccupiedEntry { entry })
            }
            indexmap::map::Entry::Vacant(entry) => {
                crate::Entry::Vacant(crate::VacantEntry { entry })
            }
        }
    }
    fn get<'s>(&'s self, key: &str) -> Option<&'s Item> {
        self.items.get(key)
    }
    fn get_mut<'s>(&'s mut self, key: &str) -> Option<&'s mut Item> {
        self.items.get_mut(key)
    }
    fn get_key_value<'a>(&'a self, key: &str) -> Option<(&'a Key, &'a Item)> {
        self.get_key_value(key)
    }
    fn get_key_value_mut<'a>(&'a mut self, key: &str) -> Option<(KeyMut<'a>, &'a mut Item)> {
        self.get_key_value_mut(key)
    }
    fn contains_key(&self, key: &str) -> bool {
        self.contains_key(key)
    }
    fn insert(&mut self, key: &str, value: Item) -> Option<Item> {
        self.insert(key, value.into_value().unwrap())
            .map(Item::Value)
    }
    fn remove(&mut self, key: &str) -> Option<Item> {
        self.remove(key).map(Item::Value)
    }

    fn get_values(&self) -> Vec<(Vec<&Key>, &Value)> {
        self.get_values()
    }
    fn fmt(&mut self) {
        self.fmt();
    }
    fn sort_values(&mut self) {
        self.sort_values();
    }
    fn set_dotted(&mut self, yes: bool) {
        self.set_dotted(yes);
    }
    fn is_dotted(&self) -> bool {
        self.is_dotted()
    }

    fn key(&self, key: &str) -> Option<&'_ Key> {
        self.key(key)
    }
    fn key_mut(&mut self, key: &str) -> Option<KeyMut<'_>> {
        self.key_mut(key)
    }
}

// `{ key1 = value1, ... }`
pub(crate) const DEFAULT_INLINE_KEY_DECOR: (&str, &str) = (" ", " ");

/// A view into a single location in an [`InlineTable`], which may be vacant or occupied.
pub enum InlineEntry<'a> {
    /// An occupied Entry.
    Occupied(InlineOccupiedEntry<'a>),
    /// A vacant Entry.
    Vacant(InlineVacantEntry<'a>),
}

impl<'a> InlineEntry<'a> {
    /// Returns the entry key
    ///
    /// # Examples
    ///
    /// ```
    /// use toml_edit::Table;
    ///
    /// let mut map = Table::new();
    ///
    /// assert_eq!("hello", map.entry("hello").key());
    /// ```
    pub fn key(&self) -> &str {
        match self {
            InlineEntry::Occupied(e) => e.key(),
            InlineEntry::Vacant(e) => e.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    pub fn or_insert(self, default: Value) -> &'a mut Value {
        match self {
            InlineEntry::Occupied(entry) => entry.into_mut(),
            InlineEntry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> Value>(self, default: F) -> &'a mut Value {
        match self {
            InlineEntry::Occupied(entry) => entry.into_mut(),
            InlineEntry::Vacant(entry) => entry.insert(default()),
        }
    }
}

/// A view into a single occupied location in an [`InlineTable`].
pub struct InlineOccupiedEntry<'a> {
    entry: indexmap::map::OccupiedEntry<'a, Key, Item>,
}

impl<'a> InlineOccupiedEntry<'a> {
    /// Gets a reference to the entry key
    ///
    /// # Examples
    ///
    /// ```
    /// use toml_edit::Table;
    ///
    /// let mut map = Table::new();
    ///
    /// assert_eq!("foo", map.entry("foo").key());
    /// ```
    pub fn key(&self) -> &str {
        self.entry.key().get()
    }

    /// Gets a mutable reference to the entry key
    pub fn key_mut(&mut self) -> KeyMut<'_> {
        use indexmap::map::MutableEntryKey;
        self.entry.key_mut().as_mut()
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &Value {
        self.entry.get().as_value().unwrap()
    }

    /// Gets a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut Value {
        self.entry.get_mut().as_value_mut().unwrap()
    }

    /// Converts the `OccupiedEntry` into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself
    pub fn into_mut(self) -> &'a mut Value {
        self.entry.into_mut().as_value_mut().unwrap()
    }

    /// Sets the value of the entry, and returns the entry's old value
    pub fn insert(&mut self, value: Value) -> Value {
        let value = Item::Value(value);
        self.entry.insert(value).into_value().unwrap()
    }

    /// Takes the value out of the entry, and returns it
    pub fn remove(self) -> Value {
        self.entry.shift_remove().into_value().unwrap()
    }
}

/// A view into a single empty location in an [`InlineTable`].
pub struct InlineVacantEntry<'a> {
    entry: indexmap::map::VacantEntry<'a, Key, Item>,
}

impl<'a> InlineVacantEntry<'a> {
    /// Gets a reference to the entry key
    ///
    /// # Examples
    ///
    /// ```
    /// use toml_edit::Table;
    ///
    /// let mut map = Table::new();
    ///
    /// assert_eq!("foo", map.entry("foo").key());
    /// ```
    pub fn key(&self) -> &str {
        self.entry.key().get()
    }

    /// Sets the value of the entry with the `VacantEntry`'s key,
    /// and returns a mutable reference to it
    pub fn insert(self, value: Value) -> &'a mut Value {
        let entry = self.entry;
        let value = Item::Value(value);
        entry.insert(value).as_value_mut().unwrap()
    }
}
