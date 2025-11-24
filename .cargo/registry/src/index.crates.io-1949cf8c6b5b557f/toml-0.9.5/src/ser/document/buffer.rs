use toml_writer::TomlWrite as _;

use crate::alloc_prelude::*;

/// TOML Document serialization buffer
#[derive(Debug, Default)]
pub struct Buffer {
    tables: Vec<Option<Table>>,
}

impl Buffer {
    /// Initialize a new serialization buffer
    pub fn new() -> Self {
        Default::default()
    }

    /// Reset the buffer for serializing another document
    pub fn clear(&mut self) {
        self.tables.clear();
    }

    pub(crate) fn root_table(&mut self) -> Table {
        self.new_table(None)
    }

    pub(crate) fn child_table(&mut self, parent: &mut Table, key: String) -> Table {
        parent.has_children = true;
        let mut key_path = parent.key.clone();
        key_path.get_or_insert_with(Vec::new).push(key);
        self.new_table(key_path)
    }

    pub(crate) fn element_table(&mut self, parent: &mut Table, key: String) -> Table {
        let mut table = self.child_table(parent, key);
        table.array = true;
        table
    }

    pub(crate) fn new_table(&mut self, key: Option<Vec<String>>) -> Table {
        let pos = self.tables.len();
        let table = Table {
            key,
            body: String::new(),
            has_children: false,
            pos,
            array: false,
        };
        self.tables.push(None);
        table
    }

    pub(crate) fn push(&mut self, table: Table) {
        let pos = table.pos;
        self.tables[pos] = Some(table);
    }
}

impl core::fmt::Display for Buffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut tables = self
            .tables
            .iter()
            .filter_map(|t| t.as_ref())
            .filter(|t| required_table(t));
        if let Some(table) = tables.next() {
            table.fmt(f)?;
        }
        for table in tables {
            f.newline()?;
            table.fmt(f)?;
        }
        Ok(())
    }
}

fn required_table(table: &Table) -> bool {
    if table.key.is_none() {
        !table.body.is_empty()
    } else {
        table.array || !table.body.is_empty() || !table.has_children
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Table {
    key: Option<Vec<String>>,
    body: String,
    has_children: bool,
    array: bool,
    pos: usize,
}

impl Table {
    pub(crate) fn body_mut(&mut self) -> &mut String {
        &mut self.body
    }

    pub(crate) fn has_children(&mut self, yes: bool) {
        self.has_children = yes;
    }
}

impl core::fmt::Display for Table {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(key) = &self.key {
            if self.array {
                f.open_array_of_tables_header()?;
            } else {
                f.open_table_header()?;
            }
            let mut key = key.iter();
            if let Some(key) = key.next() {
                write!(f, "{key}")?;
            }
            for key in key {
                f.key_sep()?;
                write!(f, "{key}")?;
            }
            if self.array {
                f.close_array_of_tables_header()?;
            } else {
                f.close_table_header()?;
            }
            f.newline()?;
        }

        self.body.fmt(f)?;

        Ok(())
    }
}
