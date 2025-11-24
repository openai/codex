/*!
The types module provides a way of associating globs on file names to file
types.

This can be used to match specific types of files. For example, among
the default file types provided, the Rust file type is defined to be `*.rs`
with name `rust`. Similarly, the C file type is defined to be `*.{c,h}` with
name `c`.

Note that the set of default types may change over time.

# Example

This shows how to create and use a simple file type matcher using the default
file types defined in this crate.

```
use ignore::types::TypesBuilder;

let mut builder = TypesBuilder::new();
builder.add_defaults();
builder.select("rust");
let matcher = builder.build().unwrap();

assert!(matcher.matched("foo.rs", false).is_whitelist());
assert!(matcher.matched("foo.c", false).is_ignore());
```

# Example: negation

This is like the previous example, but shows how negating a file type works.
That is, this will let us match file paths that *don't* correspond to a
particular file type.

```
use ignore::types::TypesBuilder;

let mut builder = TypesBuilder::new();
builder.add_defaults();
builder.negate("c");
let matcher = builder.build().unwrap();

assert!(matcher.matched("foo.rs", false).is_none());
assert!(matcher.matched("foo.c", false).is_ignore());
```

# Example: custom file type definitions

This shows how to extend this library default file type definitions with
your own.

```
use ignore::types::TypesBuilder;

let mut builder = TypesBuilder::new();
builder.add_defaults();
builder.add("foo", "*.foo");
// Another way of adding a file type definition.
// This is useful when accepting input from an end user.
builder.add_def("bar:*.bar");
// Note: we only select `foo`, not `bar`.
builder.select("foo");
let matcher = builder.build().unwrap();

assert!(matcher.matched("x.foo", false).is_whitelist());
// This is ignored because we only selected the `foo` file type.
assert!(matcher.matched("x.bar", false).is_ignore());
```

We can also add file type definitions based on other definitions.

```
use ignore::types::TypesBuilder;

let mut builder = TypesBuilder::new();
builder.add_defaults();
builder.add("foo", "*.foo");
builder.add_def("bar:include:foo,cpp");
builder.select("bar");
let matcher = builder.build().unwrap();

assert!(matcher.matched("x.foo", false).is_whitelist());
assert!(matcher.matched("y.cpp", false).is_whitelist());
```
*/

use std::{collections::HashMap, path::Path, sync::Arc};

use {
    globset::{GlobBuilder, GlobSet, GlobSetBuilder},
    regex_automata::util::pool::Pool,
};

use crate::{default_types::DEFAULT_TYPES, pathutil::file_name, Error, Match};

/// Glob represents a single glob in a set of file type definitions.
///
/// There may be more than one glob for a particular file type.
///
/// This is used to report information about the highest precedent glob
/// that matched.
///
/// Note that not all matches necessarily correspond to a specific glob.
/// For example, if there are one or more selections and a file path doesn't
/// match any of those selections, then the file path is considered to be
/// ignored.
///
/// The lifetime `'a` refers to the lifetime of the underlying file type
/// definition, which corresponds to the lifetime of the file type matcher.
#[derive(Clone, Debug)]
pub struct Glob<'a>(GlobInner<'a>);

#[derive(Clone, Debug)]
enum GlobInner<'a> {
    /// No glob matched, but the file path should still be ignored.
    UnmatchedIgnore,
    /// A glob matched.
    Matched {
        /// The file type definition which provided the glob.
        def: &'a FileTypeDef,
    },
}

impl<'a> Glob<'a> {
    fn unmatched() -> Glob<'a> {
        Glob(GlobInner::UnmatchedIgnore)
    }

    /// Return the file type definition that matched, if one exists. A file type
    /// definition always exists when a specific definition matches a file
    /// path.
    pub fn file_type_def(&self) -> Option<&FileTypeDef> {
        match self {
            Glob(GlobInner::UnmatchedIgnore) => None,
            Glob(GlobInner::Matched { def, .. }) => Some(def),
        }
    }
}

/// A single file type definition.
///
/// File type definitions can be retrieved in aggregate from a file type
/// matcher. File type definitions are also reported when its responsible
/// for a match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileTypeDef {
    name: String,
    globs: Vec<String>,
}

impl FileTypeDef {
    /// Return the name of this file type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the globs used to recognize this file type.
    pub fn globs(&self) -> &[String] {
        &self.globs
    }
}

/// Types is a file type matcher.
#[derive(Clone, Debug)]
pub struct Types {
    /// All of the file type definitions, sorted lexicographically by name.
    defs: Vec<FileTypeDef>,
    /// All of the selections made by the user.
    selections: Vec<Selection<FileTypeDef>>,
    /// Whether there is at least one Selection::Select in our selections.
    /// When this is true, a Match::None is converted to Match::Ignore.
    has_selected: bool,
    /// A mapping from glob index in the set to two indices. The first is an
    /// index into `selections` and the second is an index into the
    /// corresponding file type definition's list of globs.
    glob_to_selection: Vec<(usize, usize)>,
    /// The set of all glob selections, used for actual matching.
    set: GlobSet,
    /// Temporary storage for globs that match.
    matches: Arc<Pool<Vec<usize>>>,
}

/// Indicates the type of a selection for a particular file type.
#[derive(Clone, Debug)]
enum Selection<T> {
    Select(String, T),
    Negate(String, T),
}

impl<T> Selection<T> {
    fn is_negated(&self) -> bool {
        match *self {
            Selection::Select(..) => false,
            Selection::Negate(..) => true,
        }
    }

    fn name(&self) -> &str {
        match *self {
            Selection::Select(ref name, _) => name,
            Selection::Negate(ref name, _) => name,
        }
    }

    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Selection<U> {
        match self {
            Selection::Select(name, inner) => {
                Selection::Select(name, f(inner))
            }
            Selection::Negate(name, inner) => {
                Selection::Negate(name, f(inner))
            }
        }
    }

    fn inner(&self) -> &T {
        match *self {
            Selection::Select(_, ref inner) => inner,
            Selection::Negate(_, ref inner) => inner,
        }
    }
}

impl Types {
    /// Creates a new file type matcher that never matches any path and
    /// contains no file type definitions.
    pub fn empty() -> Types {
        Types {
            defs: vec![],
            selections: vec![],
            has_selected: false,
            glob_to_selection: vec![],
            set: GlobSetBuilder::new().build().unwrap(),
            matches: Arc::new(Pool::new(|| vec![])),
        }
    }

    /// Returns true if and only if this matcher has zero selections.
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// Returns the number of selections used in this matcher.
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Return the set of current file type definitions.
    ///
    /// Definitions and globs are sorted.
    pub fn definitions(&self) -> &[FileTypeDef] {
        &self.defs
    }

    /// Returns a match for the given path against this file type matcher.
    ///
    /// The path is considered whitelisted if it matches a selected file type.
    /// The path is considered ignored if it matches a negated file type.
    /// If at least one file type is selected and `path` doesn't match, then
    /// the path is also considered ignored.
    pub fn matched<'a, P: AsRef<Path>>(
        &'a self,
        path: P,
        is_dir: bool,
    ) -> Match<Glob<'a>> {
        // File types don't apply to directories, and we can't do anything
        // if our glob set is empty.
        if is_dir || self.set.is_empty() {
            return Match::None;
        }
        // We only want to match against the file name, so extract it.
        // If one doesn't exist, then we can't match it.
        let name = match file_name(path.as_ref()) {
            Some(name) => name,
            None if self.has_selected => {
                return Match::Ignore(Glob::unmatched());
            }
            None => {
                return Match::None;
            }
        };
        let mut matches = self.matches.get();
        self.set.matches_into(name, &mut *matches);
        // The highest precedent match is the last one.
        if let Some(&i) = matches.last() {
            let (isel, _) = self.glob_to_selection[i];
            let sel = &self.selections[isel];
            let glob = Glob(GlobInner::Matched { def: sel.inner() });
            return if sel.is_negated() {
                Match::Ignore(glob)
            } else {
                Match::Whitelist(glob)
            };
        }
        if self.has_selected {
            Match::Ignore(Glob::unmatched())
        } else {
            Match::None
        }
    }
}

/// TypesBuilder builds a type matcher from a set of file type definitions and
/// a set of file type selections.
pub struct TypesBuilder {
    types: HashMap<String, FileTypeDef>,
    selections: Vec<Selection<()>>,
}

impl TypesBuilder {
    /// Create a new builder for a file type matcher.
    ///
    /// The builder contains *no* type definitions to start with. A set
    /// of default type definitions can be added with `add_defaults`, and
    /// additional type definitions can be added with `select` and `negate`.
    pub fn new() -> TypesBuilder {
        TypesBuilder { types: HashMap::new(), selections: vec![] }
    }

    /// Build the current set of file type definitions *and* selections into
    /// a file type matcher.
    pub fn build(&self) -> Result<Types, Error> {
        let defs = self.definitions();
        let has_selected = self.selections.iter().any(|s| !s.is_negated());

        let mut selections = vec![];
        let mut glob_to_selection = vec![];
        let mut build_set = GlobSetBuilder::new();
        for (isel, selection) in self.selections.iter().enumerate() {
            let def = match self.types.get(selection.name()) {
                Some(def) => def.clone(),
                None => {
                    let name = selection.name().to_string();
                    return Err(Error::UnrecognizedFileType(name));
                }
            };
            for (iglob, glob) in def.globs.iter().enumerate() {
                build_set.add(
                    GlobBuilder::new(glob)
                        .literal_separator(true)
                        .build()
                        .map_err(|err| Error::Glob {
                            glob: Some(glob.to_string()),
                            err: err.kind().to_string(),
                        })?,
                );
                glob_to_selection.push((isel, iglob));
            }
            selections.push(selection.clone().map(move |_| def));
        }
        let set = build_set
            .build()
            .map_err(|err| Error::Glob { glob: None, err: err.to_string() })?;
        Ok(Types {
            defs,
            selections,
            has_selected,
            glob_to_selection,
            set,
            matches: Arc::new(Pool::new(|| vec![])),
        })
    }

    /// Return the set of current file type definitions.
    ///
    /// Definitions and globs are sorted.
    pub fn definitions(&self) -> Vec<FileTypeDef> {
        let mut defs = vec![];
        for def in self.types.values() {
            let mut def = def.clone();
            def.globs.sort();
            defs.push(def);
        }
        defs.sort_by(|def1, def2| def1.name().cmp(def2.name()));
        defs
    }

    /// Select the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are selected.
    pub fn select(&mut self, name: &str) -> &mut TypesBuilder {
        if name == "all" {
            for name in self.types.keys() {
                self.selections.push(Selection::Select(name.to_string(), ()));
            }
        } else {
            self.selections.push(Selection::Select(name.to_string(), ()));
        }
        self
    }

    /// Ignore the file type given by `name`.
    ///
    /// If `name` is `all`, then all file types currently defined are negated.
    pub fn negate(&mut self, name: &str) -> &mut TypesBuilder {
        if name == "all" {
            for name in self.types.keys() {
                self.selections.push(Selection::Negate(name.to_string(), ()));
            }
        } else {
            self.selections.push(Selection::Negate(name.to_string(), ()));
        }
        self
    }

    /// Clear any file type definitions for the type name given.
    pub fn clear(&mut self, name: &str) -> &mut TypesBuilder {
        self.types.remove(name);
        self
    }

    /// Add a new file type definition. `name` can be arbitrary and `pat`
    /// should be a glob recognizing file paths belonging to the `name` type.
    ///
    /// If `name` is `all` or otherwise contains any character that is not a
    /// Unicode letter or number, then an error is returned.
    pub fn add(&mut self, name: &str, glob: &str) -> Result<(), Error> {
        if name == "all" || !name.chars().all(|c| c.is_alphanumeric()) {
            return Err(Error::InvalidDefinition);
        }
        let (key, glob) = (name.to_string(), glob.to_string());
        self.types
            .entry(key)
            .or_insert_with(|| FileTypeDef {
                name: name.to_string(),
                globs: vec![],
            })
            .globs
            .push(glob);
        Ok(())
    }

    /// Add a new file type definition specified in string form. There are two
    /// valid formats:
    /// 1. `{name}:{glob}`.  This defines a 'root' definition that associates the
    ///     given name with the given glob.
    /// 2. `{name}:include:{comma-separated list of already defined names}.
    ///     This defines an 'include' definition that associates the given name
    ///     with the definitions of the given existing types.
    /// Names may not include any characters that are not
    /// Unicode letters or numbers.
    pub fn add_def(&mut self, def: &str) -> Result<(), Error> {
        let parts: Vec<&str> = def.split(':').collect();
        match parts.len() {
            2 => {
                let name = parts[0];
                let glob = parts[1];
                if name.is_empty() || glob.is_empty() {
                    return Err(Error::InvalidDefinition);
                }
                self.add(name, glob)
            }
            3 => {
                let name = parts[0];
                let types_string = parts[2];
                if name.is_empty()
                    || parts[1] != "include"
                    || types_string.is_empty()
                {
                    return Err(Error::InvalidDefinition);
                }
                let types = types_string.split(',');
                // Check ahead of time to ensure that all types specified are
                // present and fail fast if not.
                if types.clone().any(|t| !self.types.contains_key(t)) {
                    return Err(Error::InvalidDefinition);
                }
                for type_name in types {
                    let globs =
                        self.types.get(type_name).unwrap().globs.clone();
                    for glob in globs {
                        self.add(name, &glob)?;
                    }
                }
                Ok(())
            }
            _ => Err(Error::InvalidDefinition),
        }
    }

    /// Add a set of default file type definitions.
    pub fn add_defaults(&mut self) -> &mut TypesBuilder {
        static MSG: &'static str = "adding a default type should never fail";
        for &(names, exts) in DEFAULT_TYPES {
            for name in names {
                for ext in exts {
                    self.add(name, ext).expect(MSG);
                }
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::TypesBuilder;

    macro_rules! matched {
        ($name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr) => {
            matched!($name, $types, $sel, $selnot, $path, true);
        };
        (not, $name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr) => {
            matched!($name, $types, $sel, $selnot, $path, false);
        };
        ($name:ident, $types:expr, $sel:expr, $selnot:expr,
         $path:expr, $matched:expr) => {
            #[test]
            fn $name() {
                let mut btypes = TypesBuilder::new();
                for tydef in $types {
                    btypes.add_def(tydef).unwrap();
                }
                for sel in $sel {
                    btypes.select(sel);
                }
                for selnot in $selnot {
                    btypes.negate(selnot);
                }
                let types = btypes.build().unwrap();
                let mat = types.matched($path, false);
                assert_eq!($matched, !mat.is_ignore());
            }
        };
    }

    fn types() -> Vec<&'static str> {
        vec![
            "html:*.html",
            "html:*.htm",
            "rust:*.rs",
            "js:*.js",
            "py:*.py",
            "python:*.py",
            "foo:*.{rs,foo}",
            "combo:include:html,rust",
        ]
    }

    matched!(match1, types(), vec!["rust"], vec![], "lib.rs");
    matched!(match2, types(), vec!["html"], vec![], "index.html");
    matched!(match3, types(), vec!["html"], vec![], "index.htm");
    matched!(match4, types(), vec!["html", "rust"], vec![], "main.rs");
    matched!(match5, types(), vec![], vec![], "index.html");
    matched!(match6, types(), vec![], vec!["rust"], "index.html");
    matched!(match7, types(), vec!["foo"], vec!["rust"], "main.foo");
    matched!(match8, types(), vec!["combo"], vec![], "index.html");
    matched!(match9, types(), vec!["combo"], vec![], "lib.rs");
    matched!(match10, types(), vec!["py"], vec![], "main.py");
    matched!(match11, types(), vec!["python"], vec![], "main.py");

    matched!(not, matchnot1, types(), vec!["rust"], vec![], "index.html");
    matched!(not, matchnot2, types(), vec![], vec!["rust"], "main.rs");
    matched!(not, matchnot3, types(), vec!["foo"], vec!["rust"], "main.rs");
    matched!(not, matchnot4, types(), vec!["rust"], vec!["foo"], "main.rs");
    matched!(not, matchnot5, types(), vec!["rust"], vec!["foo"], "main.foo");
    matched!(not, matchnot6, types(), vec!["combo"], vec![], "leftpad.js");
    matched!(not, matchnot7, types(), vec!["py"], vec![], "index.html");
    matched!(not, matchnot8, types(), vec!["python"], vec![], "doc.md");

    #[test]
    fn test_invalid_defs() {
        let mut btypes = TypesBuilder::new();
        for tydef in types() {
            btypes.add_def(tydef).unwrap();
        }
        // Preserve the original definitions for later comparison.
        let original_defs = btypes.definitions();
        let bad_defs = vec![
            // Reference to type that does not exist
            "combo:include:html,qwerty",
            // Bad format
            "combo:foobar:html,rust",
            "",
        ];
        for def in bad_defs {
            assert!(btypes.add_def(def).is_err());
            // Ensure that nothing changed, even if some of the includes were valid.
            assert_eq!(btypes.definitions(), original_defs);
        }
    }
}
