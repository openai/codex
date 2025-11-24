use std::iter::FromIterator;

use indexmap::map::IndexMap;

use crate::key::Key;
use crate::repr::Decor;
use crate::value::DEFAULT_VALUE_DECOR;
use crate::{InlineTable, Item, KeyMut, Value};

/// A TOML table, a top-level collection of key/[`Value`] pairs under a header and logical
/// sub-tables
#[derive(Clone, Debug, Default)]
pub struct Table {
    // Comments/spaces before and after the header
    pub(crate) decor: Decor,
    // Whether to hide an empty table
    pub(crate) implicit: bool,
    // Whether this is a proxy for dotted keys
    pub(crate) dotted: bool,
    // Used for putting tables back in their original order when serialising.
    //
    // `None` for user created tables (can be overridden with `set_position`)
    doc_position: Option<isize>,
    pub(crate) span: Option<std::ops::Range<usize>>,
    pub(crate) items: KeyValuePairs,
}

/// Constructors
///
/// See also `FromIterator`
impl Table {
    /// Creates an empty table.
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn with_pos(doc_position: Option<isize>) -> Self {
        Self {
            doc_position,
            ..Default::default()
        }
    }

    pub(crate) fn with_pairs(items: KeyValuePairs) -> Self {
        Self {
            items,
            ..Default::default()
        }
    }

    /// Convert to an inline table
    pub fn into_inline_table(mut self) -> InlineTable {
        for (_, value) in self.items.iter_mut() {
            value.make_value();
        }
        let mut t = InlineTable::with_pairs(self.items);
        t.fmt();
        t
    }
}

/// Formatting
impl Table {
    /// Get key/values for values that are visually children of this table
    ///
    /// For example, this will return dotted keys
    pub fn get_values(&self) -> Vec<(Vec<&Key>, &Value)> {
        let mut values = Vec::new();
        let root = Vec::new();
        self.append_values(&root, &mut values);
        values
    }

    fn append_values<'s>(
        &'s self,
        parent: &[&'s Key],
        values: &mut Vec<(Vec<&'s Key>, &'s Value)>,
    ) {
        for (key, value) in self.items.iter() {
            let mut path = parent.to_vec();
            path.push(key);
            match value {
                Item::Table(table) if table.is_dotted() => {
                    table.append_values(&path, values);
                }
                Item::Value(value) => {
                    if let Some(table) = value.as_inline_table() {
                        if table.is_dotted() {
                            table.append_values(&path, values);
                        } else {
                            values.push((path, value));
                        }
                    } else {
                        values.push((path, value));
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn append_all_values<'s>(
        &'s self,
        parent: &[&'s Key],
        values: &mut Vec<(Vec<&'s Key>, &'s Value)>,
    ) {
        for (key, value) in self.items.iter() {
            let mut path = parent.to_vec();
            path.push(key);
            match value {
                Item::Table(table) => {
                    table.append_all_values(&path, values);
                }
                Item::Value(value) => {
                    if let Some(table) = value.as_inline_table() {
                        if table.is_dotted() {
                            table.append_values(&path, values);
                        } else {
                            values.push((path, value));
                        }
                    } else {
                        values.push((path, value));
                    }
                }
                _ => {}
            }
        }
    }

    /// Auto formats the table.
    pub fn fmt(&mut self) {
        decorate_table(self);
    }

    /// Sorts [Key]/[Value]-pairs of the table
    ///
    /// <div class="warning">
    ///
    /// This sorts the syntactic table (everything under the `[header]`) and not the logical map of
    /// key-value pairs.
    /// This does not affect the order of [sub-tables][Table] or [sub-arrays][crate::ArrayOfTables].
    /// This is not recursive.
    ///
    /// </div>
    pub fn sort_values(&mut self) {
        // Assuming standard tables have their doc_position set and this won't negatively impact them
        self.items.sort_keys();
        for value in self.items.values_mut() {
            match value {
                Item::Table(table) if table.is_dotted() => {
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
    /// This sorts the syntactic table (everything under the `[header]`) and not the logical map of
    /// key-value pairs.
    /// This does not affect the order of [sub-tables][Table] or [sub-arrays][crate::ArrayOfTables].
    /// This is not recursive.
    ///
    /// </div>
    pub fn sort_values_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&Key, &Item, &Key, &Item) -> std::cmp::Ordering,
    {
        self.sort_values_by_internal(&mut compare);
    }

    fn sort_values_by_internal<F>(&mut self, compare: &mut F)
    where
        F: FnMut(&Key, &Item, &Key, &Item) -> std::cmp::Ordering,
    {
        let modified_cmp =
            |key1: &Key, val1: &Item, key2: &Key, val2: &Item| -> std::cmp::Ordering {
                compare(key1, val1, key2, val2)
            };

        self.items.sort_by(modified_cmp);

        for value in self.items.values_mut() {
            match value {
                Item::Table(table) if table.is_dotted() => {
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
    pub fn set_implicit(&mut self, implicit: bool) {
        self.implicit = implicit;
    }

    /// If a table has no key/value pairs and implicit, it will not be displayed.
    pub fn is_implicit(&self) -> bool {
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

    /// Sets the position of the `Table` within the [`DocumentMut`][crate::DocumentMut].
    pub fn set_position(&mut self, doc_position: isize) {
        self.doc_position = Some(doc_position);
    }

    /// The position of the `Table` within the [`DocumentMut`][crate::DocumentMut].
    ///
    /// Returns `None` if the `Table` was created manually (i.e. not via parsing)
    /// in which case its position is set automatically.  This can be overridden with
    /// [`Table::set_position`].
    pub fn position(&self) -> Option<isize> {
        self.doc_position
    }

    /// Returns the surrounding whitespace
    pub fn decor_mut(&mut self) -> &mut Decor {
        &mut self.decor
    }

    /// Returns the decor associated with a given key of the table.
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
        for (key, value) in self.items.iter_mut2() {
            key.despan(input);
            value.despan(input);
        }
    }
}

impl Table {
    /// Returns an iterator over all key/value pairs, including empty.
    pub fn iter(&self) -> Iter<'_> {
        Box::new(
            self.items
                .iter()
                .filter(|(_, value)| !value.is_none())
                .map(|(key, value)| (key.get(), value)),
        )
    }

    /// Returns an mutable iterator over all key/value pairs, including empty.
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        use indexmap::map::MutableKeys;
        Box::new(
            self.items
                .iter_mut2()
                .filter(|(_, value)| !value.is_none())
                .map(|(key, value)| (key.as_mut(), value)),
        )
    }

    /// Returns the number of non-empty items in the table.
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
    pub fn entry<'a>(&'a mut self, key: &str) -> Entry<'a> {
        // Accept a `&str` rather than an owned type to keep `String`, well, internal
        match self.items.entry(key.into()) {
            indexmap::map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry { entry }),
            indexmap::map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry { entry }),
        }
    }

    /// Gets the given key's corresponding entry in the Table for in-place manipulation.
    pub fn entry_format<'a>(&'a mut self, key: &Key) -> Entry<'a> {
        // Accept a `&Key` to be consistent with `entry`
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry { entry }),
            indexmap::map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry { entry }),
        }
    }

    /// Returns an optional reference to an item given the key.
    pub fn get<'a>(&'a self, key: &str) -> Option<&'a Item> {
        self.items
            .get(key)
            .and_then(|value| if !value.is_none() { Some(value) } else { None })
    }

    /// Returns an optional mutable reference to an item given the key.
    pub fn get_mut<'a>(&'a mut self, key: &str) -> Option<&'a mut Item> {
        self.items
            .get_mut(key)
            .and_then(|value| if !value.is_none() { Some(value) } else { None })
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

    /// Returns true if the table contains an item with the given key.
    pub fn contains_key(&self, key: &str) -> bool {
        if let Some(value) = self.items.get(key) {
            !value.is_none()
        } else {
            false
        }
    }

    /// Returns true if the table contains a table with the given key.
    pub fn contains_table(&self, key: &str) -> bool {
        if let Some(value) = self.items.get(key) {
            value.is_table()
        } else {
            false
        }
    }

    /// Returns true if the table contains a value with the given key.
    pub fn contains_value(&self, key: &str) -> bool {
        if let Some(value) = self.items.get(key) {
            value.is_value()
        } else {
            false
        }
    }

    /// Returns true if the table contains an array of tables with the given key.
    pub fn contains_array_of_tables(&self, key: &str) -> bool {
        if let Some(value) = self.items.get(key) {
            value.is_array_of_tables()
        } else {
            false
        }
    }

    /// Inserts a key-value pair into the map.
    pub fn insert(&mut self, key: &str, item: Item) -> Option<Item> {
        use indexmap::map::MutableEntryKey;
        let key = Key::new(key);
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                entry.key_mut().fmt();
                let old = std::mem::replace(entry.get_mut(), item);
                Some(old)
            }
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(item);
                None
            }
        }
    }

    /// Inserts a key-value pair into the map.
    pub fn insert_formatted(&mut self, key: &Key, item: Item) -> Option<Item> {
        use indexmap::map::MutableEntryKey;
        match self.items.entry(key.clone()) {
            indexmap::map::Entry::Occupied(mut entry) => {
                *entry.key_mut() = key.clone();
                let old = std::mem::replace(entry.get_mut(), item);
                Some(old)
            }
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(item);
                None
            }
        }
    }

    /// Removes an item given the key.
    pub fn remove(&mut self, key: &str) -> Option<Item> {
        self.items.shift_remove(key)
    }

    /// Removes a key from the map, returning the stored key and value if the key was previously in the map.
    pub fn remove_entry(&mut self, key: &str) -> Option<(Key, Item)> {
        self.items.shift_remove_entry(key)
    }

    /// Retains only the elements specified by the `keep` predicate.
    ///
    /// In other words, remove all pairs `(key, item)` for which
    /// `keep(&key, &mut item)` returns `false`.
    ///
    /// The elements are visited in iteration order.
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&str, &mut Item) -> bool,
    {
        self.items.retain(|key, value| keep(key, value));
    }
}

#[cfg(feature = "display")]
impl std::fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let children = self.get_values();
        // print table body
        for (key_path, value) in children {
            crate::encode::encode_key_path_ref(&key_path, f, None, DEFAULT_KEY_DECOR)?;
            write!(f, "=")?;
            crate::encode::encode_value(value, f, None, DEFAULT_VALUE_DECOR)?;
            writeln!(f)?;
        }
        Ok(())
    }
}

impl<K: Into<Key>, V: Into<Item>> Extend<(K, V)> for Table {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (key, value) in iter {
            let key = key.into();
            let value = value.into();
            self.items.insert(key, value);
        }
    }
}

impl<K: Into<Key>, V: Into<Item>> FromIterator<(K, V)> for Table {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut table = Self::new();
        table.extend(iter);
        table
    }
}

impl IntoIterator for Table {
    type Item = (String, Item);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.items.into_iter().map(|(k, value)| (k.into(), value)))
    }
}

impl<'s> IntoIterator for &'s Table {
    type Item = (&'s str, &'s Item);
    type IntoIter = Iter<'s>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub(crate) type KeyValuePairs = IndexMap<Key, Item>;

fn decorate_table(table: &mut Table) {
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

// `key1 = value1`
pub(crate) const DEFAULT_ROOT_DECOR: (&str, &str) = ("", "");
pub(crate) const DEFAULT_KEY_DECOR: (&str, &str) = ("", " ");
pub(crate) const DEFAULT_TABLE_DECOR: (&str, &str) = ("\n", "");
pub(crate) const DEFAULT_KEY_PATH_DECOR: (&str, &str) = ("", "");

/// An owned iterator type over [`Table`]'s [`Key`]/[`Item`] pairs
pub type IntoIter = Box<dyn Iterator<Item = (String, Item)>>;
/// An iterator type over [`Table`]'s [`Key`]/[`Item`] pairs
pub type Iter<'a> = Box<dyn Iterator<Item = (&'a str, &'a Item)> + 'a>;
/// A mutable iterator type over [`Table`]'s [`Key`]/[`Item`] pairs
pub type IterMut<'a> = Box<dyn Iterator<Item = (KeyMut<'a>, &'a mut Item)> + 'a>;

/// This trait represents either a `Table`, or an `InlineTable`.
pub trait TableLike: crate::private::Sealed {
    /// Returns an iterator over key/value pairs.
    fn iter(&self) -> Iter<'_>;
    /// Returns an mutable iterator over all key/value pairs, including empty.
    fn iter_mut(&mut self) -> IterMut<'_>;
    /// Returns the number of nonempty items.
    fn len(&self) -> usize {
        self.iter().filter(|&(_, v)| !v.is_none()).count()
    }
    /// Returns true if the table is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Clears the table, removing all key-value pairs. Keeps the allocated memory for reuse.
    fn clear(&mut self);
    /// Gets the given key's corresponding entry in the Table for in-place manipulation.
    fn entry<'a>(&'a mut self, key: &str) -> Entry<'a>;
    /// Gets the given key's corresponding entry in the Table for in-place manipulation.
    fn entry_format<'a>(&'a mut self, key: &Key) -> Entry<'a>;
    /// Returns an optional reference to an item given the key.
    fn get<'s>(&'s self, key: &str) -> Option<&'s Item>;
    /// Returns an optional mutable reference to an item given the key.
    fn get_mut<'s>(&'s mut self, key: &str) -> Option<&'s mut Item>;
    /// Return references to the key-value pair stored for key, if it is present, else None.
    fn get_key_value<'a>(&'a self, key: &str) -> Option<(&'a Key, &'a Item)>;
    /// Return mutable references to the key-value pair stored for key, if it is present, else None.
    fn get_key_value_mut<'a>(&'a mut self, key: &str) -> Option<(KeyMut<'a>, &'a mut Item)>;
    /// Returns true if the table contains an item with the given key.
    fn contains_key(&self, key: &str) -> bool;
    /// Inserts a key-value pair into the map.
    fn insert(&mut self, key: &str, value: Item) -> Option<Item>;
    /// Removes an item given the key.
    fn remove(&mut self, key: &str) -> Option<Item>;

    /// Get key/values for values that are visually children of this table
    ///
    /// For example, this will return dotted keys
    fn get_values(&self) -> Vec<(Vec<&Key>, &Value)>;

    /// Auto formats the table.
    fn fmt(&mut self);
    /// Sorts [Key]/[Value]-pairs of the table
    ///
    /// <div class="warning">
    ///
    /// This sorts the syntactic table (everything under the `[header]`) and not the logical map of
    /// key-value pairs.
    /// This does not affect the order of [sub-tables][Table] or [sub-arrays][crate::ArrayOfTables].
    /// This is not recursive.
    ///
    /// </div>
    fn sort_values(&mut self);
    /// Change this table's dotted status
    fn set_dotted(&mut self, yes: bool);
    /// Check if this is a wrapper for dotted keys, rather than a standard table
    fn is_dotted(&self) -> bool;

    /// Returns an accessor to a key's formatting
    fn key(&self, key: &str) -> Option<&'_ Key>;
    /// Returns an accessor to a key's formatting
    fn key_mut(&mut self, key: &str) -> Option<KeyMut<'_>>;
}

impl TableLike for Table {
    fn iter(&self) -> Iter<'_> {
        self.iter()
    }
    fn iter_mut(&mut self) -> IterMut<'_> {
        self.iter_mut()
    }
    fn clear(&mut self) {
        self.clear();
    }
    fn entry<'a>(&'a mut self, key: &str) -> Entry<'a> {
        self.entry(key)
    }
    fn entry_format<'a>(&'a mut self, key: &Key) -> Entry<'a> {
        self.entry_format(key)
    }
    fn get<'s>(&'s self, key: &str) -> Option<&'s Item> {
        self.get(key)
    }
    fn get_mut<'s>(&'s mut self, key: &str) -> Option<&'s mut Item> {
        self.get_mut(key)
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
        self.insert(key, value)
    }
    fn remove(&mut self, key: &str) -> Option<Item> {
        self.remove(key)
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
    fn is_dotted(&self) -> bool {
        self.is_dotted()
    }
    fn set_dotted(&mut self, yes: bool) {
        self.set_dotted(yes);
    }

    fn key(&self, key: &str) -> Option<&'_ Key> {
        self.key(key)
    }
    fn key_mut(&mut self, key: &str) -> Option<KeyMut<'_>> {
        self.key_mut(key)
    }
}

/// A view into a single location in a [`Table`], which may be vacant or occupied.
pub enum Entry<'a> {
    /// An occupied Entry.
    Occupied(OccupiedEntry<'a>),
    /// A vacant Entry.
    Vacant(VacantEntry<'a>),
}

impl<'a> Entry<'a> {
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
            Entry::Occupied(e) => e.key(),
            Entry::Vacant(e) => e.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    pub fn or_insert(self, default: Item) -> &'a mut Item {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> Item>(self, default: F) -> &'a mut Item {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
}

/// A view into a single occupied location in a [`Table`].
pub struct OccupiedEntry<'a> {
    pub(crate) entry: indexmap::map::OccupiedEntry<'a, Key, Item>,
}

impl<'a> OccupiedEntry<'a> {
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
    pub fn get(&self) -> &Item {
        self.entry.get()
    }

    /// Gets a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut Item {
        self.entry.get_mut()
    }

    /// Converts the `OccupiedEntry` into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself
    pub fn into_mut(self) -> &'a mut Item {
        self.entry.into_mut()
    }

    /// Sets the value of the entry, and returns the entry's old value
    pub fn insert(&mut self, value: Item) -> Item {
        self.entry.insert(value)
    }

    /// Takes the value out of the entry, and returns it
    pub fn remove(self) -> Item {
        self.entry.shift_remove()
    }
}

/// A view into a single empty location in a [`Table`].
pub struct VacantEntry<'a> {
    pub(crate) entry: indexmap::map::VacantEntry<'a, Key, Item>,
}

impl<'a> VacantEntry<'a> {
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
    pub fn insert(self, value: Item) -> &'a mut Item {
        let entry = self.entry;
        entry.insert(value)
    }
}
