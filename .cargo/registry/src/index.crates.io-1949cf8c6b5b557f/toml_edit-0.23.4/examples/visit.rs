//! Example for how to use `VisitMut` to iterate over a table.

use std::collections::BTreeSet;
use toml_edit::visit::{visit_table_like_kv, Visit};
use toml_edit::visit_mut::{visit_table_like_kv_mut, visit_table_mut, VisitMut};
use toml_edit::{Array, DocumentMut, InlineTable, Item, KeyMut, Table, Value};

/// This models the visit state for dependency keys in a `Cargo.toml`.
///
/// Dependencies can be specified as:
///
/// ```toml
/// [dependencies]
/// dep1 = "0.2"
///
/// [build-dependencies]
/// dep2 = "0.3"
///
/// [dev-dependencies]
/// dep3 = "0.4"
///
/// [target.'cfg(windows)'.dependencies]
/// dep4 = "0.5"
///
/// # and target build- and dev-dependencies
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum VisitState {
    /// Represents the root of the table.
    Root,
    /// Represents "dependencies", "build-dependencies" or "dev-dependencies", or the target
    /// forms of these.
    Dependencies,
    /// A table within dependencies.
    SubDependencies,
    /// Represents "target".
    Target,
    /// "target.[TARGET]".
    TargetWithSpec,
    /// Represents some other state.
    Other,
}

impl VisitState {
    /// Figures out the next visit state, given the current state and the given key.
    fn descend(self, key: &str) -> Self {
        match (self, key) {
            (
                Self::Root | Self::TargetWithSpec,
                "dependencies" | "build-dependencies" | "dev-dependencies",
            ) => Self::Dependencies,
            (Self::Root, "target") => Self::Target,
            (Self::Root | Self::TargetWithSpec, _) => Self::Other,
            (Self::Target, _) => Self::TargetWithSpec,
            (Self::Dependencies, _) => Self::SubDependencies,
            (Self::SubDependencies, _) => Self::SubDependencies,
            (Self::Other, _) => Self::Other,
        }
    }
}

/// Collect the names of every dependency key.
#[derive(Debug)]
struct DependencyNameVisitor<'doc> {
    state: VisitState,
    names: BTreeSet<&'doc str>,
}

impl<'doc> Visit<'doc> for DependencyNameVisitor<'doc> {
    fn visit_table_like_kv(&mut self, key: &'doc str, node: &'doc Item) {
        if self.state == VisitState::Dependencies {
            self.names.insert(key);
        } else {
            // Since we're only interested in collecting the top-level keys right under
            // [dependencies], don't recurse unconditionally.

            let old_state = self.state;

            // Figure out the next state given the key.
            self.state = self.state.descend(key);

            // Recurse further into the document tree.
            visit_table_like_kv(self, key, node);

            // Restore the old state after it's done.
            self.state = old_state;
        }
    }
}

/// Normalize all dependency tables into the format:
///
/// ```toml
/// [dependencies]
/// dep = { version = "1.0", features = ["foo", "bar"], ... }
/// ```
///
/// leaving other tables untouched.
#[derive(Debug)]
struct NormalizeDependencyTablesVisitor {
    state: VisitState,
}

impl VisitMut for NormalizeDependencyTablesVisitor {
    fn visit_table_mut(&mut self, node: &mut Table) {
        visit_table_mut(self, node);

        // The conversion from regular tables into inline ones might leave some explicit parent
        // tables hanging, so convert them to implicit.
        if matches!(self.state, VisitState::Target | VisitState::TargetWithSpec) {
            node.set_implicit(true);
        }
    }

    fn visit_table_like_kv_mut(&mut self, mut key: KeyMut<'_>, node: &mut Item) {
        let old_state = self.state;

        // Figure out the next state given the key.
        self.state = self.state.descend(key.get());

        match self.state {
            VisitState::Target | VisitState::TargetWithSpec | VisitState::Dependencies => {
                // Top-level dependency row, or above: turn inline tables into regular ones.
                if let Item::Value(Value::InlineTable(inline_table)) = node {
                    let inline_table = std::mem::replace(inline_table, InlineTable::new());
                    let table = inline_table.into_table();
                    key.fmt();
                    *node = Item::Table(table);
                }
            }
            VisitState::SubDependencies => {
                // Individual dependency: turn regular tables into inline ones.
                if let Item::Table(table) = node {
                    // Turn the table into an inline table.
                    let table = std::mem::replace(table, Table::new());
                    let inline_table = table.into_inline_table();
                    key.fmt();
                    *node = Item::Value(Value::InlineTable(inline_table));
                }
            }
            _ => {}
        }

        // Recurse further into the document tree.
        visit_table_like_kv_mut(self, key, node);

        // Restore the old state after it's done.
        self.state = old_state;
    }

    fn visit_array_mut(&mut self, node: &mut Array) {
        // Format any arrays within dependencies to be on the same line.
        if matches!(
            self.state,
            VisitState::Dependencies | VisitState::SubDependencies
        ) {
            node.fmt();
        }
    }
}

/// This is the input provided to `visit_mut_example`.
static INPUT: &str = r#"
[package]
name = "my-package"

[package.metadata.foo]
bar = 42

[dependencies]
atty = "0.2"
cargo-platform = { path = "crates/cargo-platform", version = "0.1.2" }

[dependencies.pretty_env_logger]
version = "0.4"
optional = true

[target.'cfg(windows)'.dependencies]
fwdansi = "1.1.0"

[target.'cfg(windows)'.dependencies.winapi]
version = "0.3"
features = [
"handleapi",
"jobapi",
]

[target.'cfg(unix)']
dev-dependencies = { miniz_oxide = "0.5" }

[dev-dependencies.cargo-test-macro]
path = "crates/cargo-test-macro"

[build-dependencies.flate2]
version = "0.4"
"#;

/// This is the output produced by `visit_mut_example`.
#[cfg(test)]
static VISIT_MUT_OUTPUT: &str = r#"
[package]
name = "my-package"

[package.metadata.foo]
bar = 42

[dependencies]
atty = "0.2"
cargo-platform = { path = "crates/cargo-platform", version = "0.1.2" }
pretty_env_logger = { version = "0.4", optional = true }

[target.'cfg(windows)'.dependencies]
fwdansi = "1.1.0"
winapi = { version = "0.3", features = ["handleapi", "jobapi"] }

[target.'cfg(unix)'.dev-dependencies]
miniz_oxide = "0.5"

[dev-dependencies]
cargo-test-macro = { path = "crates/cargo-test-macro" }

[build-dependencies]
flate2 = { version = "0.4" }
"#;

fn visit_example(document: &DocumentMut) -> BTreeSet<&str> {
    let mut visitor = DependencyNameVisitor {
        state: VisitState::Root,
        names: BTreeSet::new(),
    };

    visitor.visit_document(document);

    visitor.names
}

fn visit_mut_example(document: &mut DocumentMut) {
    let mut visitor = NormalizeDependencyTablesVisitor {
        state: VisitState::Root,
    };

    visitor.visit_document_mut(document);
}

fn main() {
    let mut document: DocumentMut = INPUT.parse().expect("input is valid TOML");

    println!("** visit example");
    println!("{:?}", visit_example(&document));

    println!("** visit_mut example");
    visit_mut_example(&mut document);
    println!("{document}");
}

#[cfg(test)]
#[test]
fn visit_correct() {
    let document: DocumentMut = INPUT.parse().expect("input is valid TOML");

    let names = visit_example(&document);
    let expected = vec![
        "atty",
        "cargo-platform",
        "pretty_env_logger",
        "fwdansi",
        "winapi",
        "miniz_oxide",
        "cargo-test-macro",
        "flate2",
    ]
    .into_iter()
    .collect();
    assert_eq!(names, expected);
}

#[cfg(test)]
#[test]
fn visit_mut_correct() {
    let mut document: DocumentMut = INPUT.parse().expect("input is valid TOML");

    visit_mut_example(&mut document);
    assert_eq!(format!("{document}"), VISIT_MUT_OUTPUT);
}
