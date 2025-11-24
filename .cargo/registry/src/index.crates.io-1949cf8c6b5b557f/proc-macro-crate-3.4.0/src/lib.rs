/*!

[![](https://docs.rs/proc-macro-crate/badge.svg)](https://docs.rs/proc-macro-crate/) [![](https://img.shields.io/crates/v/proc-macro-crate.svg)](https://crates.io/crates/proc-macro-crate) [![](https://img.shields.io/crates/d/proc-macro-crate.png)](https://crates.io/crates/proc-macro-crate) [![Build Status](https://travis-ci.org/bkchr/proc-macro-crate.png?branch=master)](https://travis-ci.org/bkchr/proc-macro-crate)

Providing support for `$crate` in procedural macros.

* [Introduction](#introduction)
* [Example](#example)
* [License](#license)

## Introduction

In `macro_rules!` `$crate` is used to get the path of the crate where a macro is declared in. In
procedural macros there is currently no easy way to get this path. A common hack is to import the
desired crate with a know name and use this. However, with rust edition 2018 and dropping
`extern crate` declarations from `lib.rs`, people start to rename crates in `Cargo.toml` directly.
However, this breaks importing the crate, as the proc-macro developer does not know the renamed
name of the crate that should be imported.

This crate provides a way to get the name of a crate, even if it renamed in `Cargo.toml`. For this
purpose a single function `crate_name` is provided. This function needs to be called in the context
of a proc-macro with the name of the desired crate. `CARGO_MANIFEST_DIR` will be used to find the
current active `Cargo.toml` and this `Cargo.toml` is searched for the desired crate.

## Example

```
use quote::quote;
use syn::Ident;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};

fn import_my_crate() {
    let found_crate = crate_name("my-crate").expect("my-crate is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => quote!( crate::Something ),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( #ident::Something )
        }
    };
}

# fn main() {}
```

## Edge cases

There are multiple edge cases when it comes to determining the correct crate. If you for example
import a crate as its own dependency, like this:

```toml
[package]
name = "my_crate"

[dev-dependencies]
my_crate = { version = "0.1", features = [ "test-feature" ] }
```

The crate will return `FoundCrate::Itself` and you will not be able to find the other instance
of your crate in `dev-dependencies`. Other similar cases are when one crate is imported multiple
times:

```toml
[package]
name = "my_crate"

[dependencies]
some-crate = { version = "0.5" }
some-crate-old = { package = "some-crate", version = "0.1" }
```

When searching for `some-crate` in this `Cargo.toml` it will return `FoundCrate::Name("some_old_crate")`,
aka the last definition of the crate in the `Cargo.toml`.

## License

Licensed under either of

 * [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)

 * [MIT license](https://opensource.org/licenses/MIT)

at your option.
*/

use std::{
    collections::btree_map::{self, BTreeMap},
    env, fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::SystemTime,
};

use toml_edit::{DocumentMut, Item, TableLike, TomlError};

/// Error type used by this crate.
pub enum Error {
    NotFound(PathBuf),
    CargoManifestDirNotSet,
    FailedGettingWorkspaceManifestPath,
    CouldNotRead { path: PathBuf, source: io::Error },
    InvalidToml { source: TomlError },
    CrateNotFound { crate_name: String, path: PathBuf },
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::CouldNotRead { source, .. } => Some(source),
            Error::InvalidToml { source } => Some(source),
            _ => None,
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotFound(path) =>
                write!(f, "Could not find `Cargo.toml` in manifest dir: `{}`.", path.display()),
            Error::CargoManifestDirNotSet =>
                f.write_str("`CARGO_MANIFEST_DIR` env variable not set."),
            Error::CouldNotRead { path, .. } => write!(f, "Could not read `{}`.", path.display()),
            Error::InvalidToml { .. } => f.write_str("Invalid toml file."),
            Error::CrateNotFound { crate_name, path } => write!(
                f,
                "Could not find `{}` in `dependencies` or `dev-dependencies` in `{}`!",
                crate_name,
                path.display(),
            ),
            Error::FailedGettingWorkspaceManifestPath =>
                f.write_str("Failed to get the path of the workspace manifest path."),
        }
    }
}

/// The crate as found by [`crate_name`].
#[derive(Debug, PartialEq, Clone, Eq)]
pub enum FoundCrate {
    /// The searched crate is this crate itself.
    Itself,
    /// The searched crate was found with this name.
    Name(String),
}

// In a rustc invocation, there will only ever be one entry in this map, since every crate is
// compiled with its own rustc process. However, the same is not (currently) the case for
// rust-analyzer.
type Cache = BTreeMap<String, CacheEntry>;

struct CacheEntry {
    manifest_ts: SystemTime,
    workspace_manifest_ts: SystemTime,
    workspace_manifest_path: PathBuf,
    crate_names: CrateNames,
}

type CrateNames = BTreeMap<String, FoundCrate>;

/// Find the crate name for the given `orig_name` in the current `Cargo.toml`.
///
/// `orig_name` should be the original name of the searched crate.
///
/// The current `Cargo.toml` is determined by taking `CARGO_MANIFEST_DIR/Cargo.toml`.
///
/// # Returns
///
/// - `Ok(FoundCrate::Itself)` the searched crate is the current crate being compiled.
/// - `Ok(FoundCrate::Name(new_name))` the searched create was found with the given name in the
///   `Cargo.toml`.
/// - `Err` if an error occurred. See [`Error`].
///
/// The returned crate name is sanitized in such a way that it is a valid rust identifier. Thus,
/// it is ready to be used in `extern crate` as identifier.
pub fn crate_name(orig_name: &str) -> Result<FoundCrate, Error> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").map_err(|_| Error::CargoManifestDirNotSet)?;
    let manifest_path = Path::new(&manifest_dir).join("Cargo.toml");

    let manifest_ts = cargo_toml_timestamp(&manifest_path)?;

    static CACHE: Mutex<Cache> = Mutex::new(BTreeMap::new());
    let mut cache = CACHE.lock().unwrap();

    let crate_names = match cache.entry(manifest_dir) {
        btree_map::Entry::Occupied(entry) => {
            let cache_entry = entry.into_mut();
            let workspace_manifest_path = cache_entry.workspace_manifest_path.as_path();
            let workspace_manifest_ts = cargo_toml_timestamp(&workspace_manifest_path)?;

            // Timestamp changed, rebuild this cache entry.
            if manifest_ts != cache_entry.manifest_ts ||
                workspace_manifest_ts != cache_entry.workspace_manifest_ts
            {
                *cache_entry = read_cargo_toml(
                    &manifest_path,
                    &workspace_manifest_path,
                    manifest_ts,
                    workspace_manifest_ts,
                )?;
            }

            &cache_entry.crate_names
        },
        btree_map::Entry::Vacant(entry) => {
            // If `workspace_manifest_path` returns `None`, we are probably in a vendored deps
            // folder and cargo complaining that we have some package inside a workspace, that isn't
            // part of the workspace. In this case we just use the `manifest_path` as the
            // `workspace_manifest_path`.
            let workspace_manifest_path =
                workspace_manifest_path(&manifest_path)?.unwrap_or_else(|| manifest_path.clone());
            let workspace_manifest_ts = cargo_toml_timestamp(&workspace_manifest_path)?;

            let cache_entry = entry.insert(read_cargo_toml(
                &manifest_path,
                &workspace_manifest_path,
                manifest_ts,
                workspace_manifest_ts,
            )?);
            &cache_entry.crate_names
        },
    };

    Ok(crate_names
        .get(orig_name)
        .ok_or_else(|| Error::CrateNotFound {
            crate_name: orig_name.to_owned(),
            path: manifest_path,
        })?
        .clone())
}

fn workspace_manifest_path(cargo_toml_manifest: &Path) -> Result<Option<PathBuf>, Error> {
    let Ok(cargo) = env::var("CARGO") else {
        return Ok(None);
    };

    let stdout = Command::new(cargo)
        .arg("locate-project")
        .args(&["--workspace", "--message-format=plain"])
        .arg(format!("--manifest-path={}", cargo_toml_manifest.display()))
        .output()
        .map_err(|_| Error::FailedGettingWorkspaceManifestPath)?
        .stdout;

    String::from_utf8(stdout)
        .map_err(|_| Error::FailedGettingWorkspaceManifestPath)
        .map(|s| {
            let path = s.trim();

            if path.is_empty() {
                None
            } else {
                Some(path.into())
            }
        })
}

fn cargo_toml_timestamp(manifest_path: &Path) -> Result<SystemTime, Error> {
    fs::metadata(manifest_path).and_then(|meta| meta.modified()).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            Error::NotFound(manifest_path.to_owned())
        } else {
            Error::CouldNotRead { path: manifest_path.to_owned(), source }
        }
    })
}

fn read_cargo_toml(
    manifest_path: &Path,
    workspace_manifest_path: &Path,
    manifest_ts: SystemTime,
    workspace_manifest_ts: SystemTime,
) -> Result<CacheEntry, Error> {
    let manifest = open_cargo_toml(manifest_path)?;

    let workspace_dependencies = if manifest_path != workspace_manifest_path {
        let workspace_manifest = open_cargo_toml(workspace_manifest_path)?;
        extract_workspace_dependencies(&workspace_manifest)?
    } else {
        extract_workspace_dependencies(&manifest)?
    };

    let crate_names = extract_crate_names(&manifest, workspace_dependencies)?;

    Ok(CacheEntry {
        manifest_ts,
        workspace_manifest_ts,
        crate_names,
        workspace_manifest_path: workspace_manifest_path.to_path_buf(),
    })
}

/// Extract all `[workspace.dependencies]`.
///
/// Returns a hash map that maps from dep name to the package name. Dep name
/// and package name can be the same if there doesn't exist any rename.
fn extract_workspace_dependencies(
    workspace_toml: &DocumentMut,
) -> Result<BTreeMap<String, String>, Error> {
    Ok(workspace_dep_tables(&workspace_toml)
        .into_iter()
        .map(|t| t.iter())
        .flatten()
        .map(move |(dep_name, dep_value)| {
            let pkg_name = dep_value.get("package").and_then(|i| i.as_str()).unwrap_or(dep_name);

            (dep_name.to_owned(), pkg_name.to_owned())
        })
        .collect())
}

/// Return an iterator over all `[workspace.dependencies]`
fn workspace_dep_tables(cargo_toml: &DocumentMut) -> Option<&dyn TableLike> {
    cargo_toml
        .get("workspace")
        .and_then(|w| w.as_table_like()?.get("dependencies")?.as_table_like())
}

/// Make sure that the given crate name is a valid rust identifier.
fn sanitize_crate_name<S: AsRef<str>>(name: S) -> String {
    name.as_ref().replace('-', "_")
}

/// Open the given `Cargo.toml` and parse it into a hashmap.
fn open_cargo_toml(path: &Path) -> Result<DocumentMut, Error> {
    let content = fs::read_to_string(path)
        .map_err(|e| Error::CouldNotRead { source: e, path: path.into() })?;
    content.parse::<DocumentMut>().map_err(|e| Error::InvalidToml { source: e })
}

/// Extract all crate names from the given `Cargo.toml` by checking the `dependencies` and
/// `dev-dependencies`.
fn extract_crate_names(
    cargo_toml: &DocumentMut,
    workspace_dependencies: BTreeMap<String, String>,
) -> Result<CrateNames, Error> {
    let package_name = extract_package_name(cargo_toml);
    let root_pkg = package_name.as_ref().map(|name| {
        let cr = match env::var_os("CARGO_TARGET_TMPDIR") {
            // We're running for a library/binary crate
            None => FoundCrate::Itself,
            // We're running for an integration test
            Some(_) => FoundCrate::Name(sanitize_crate_name(name)),
        };

        (name.to_string(), cr)
    });

    let dep_tables = dep_tables(cargo_toml.as_table()).chain(target_dep_tables(cargo_toml));
    let dep_pkgs =
        dep_tables.map(|t| t.iter()).flatten().filter_map(move |(dep_name, dep_value)| {
            let pkg_name = dep_value.get("package").and_then(|i| i.as_str()).unwrap_or(dep_name);

            // We already handle this via `root_pkg` above.
            if package_name.as_ref().map_or(false, |n| *n == pkg_name) {
                return None
            }

            // Check if this is a workspace dependency.
            let workspace =
                dep_value.get("workspace").and_then(|w| w.as_bool()).unwrap_or_default();

            let pkg_name = workspace
                .then(|| workspace_dependencies.get(pkg_name).map(|p| p.as_ref()))
                .flatten()
                .unwrap_or(pkg_name);

            let cr = FoundCrate::Name(sanitize_crate_name(dep_name));

            Some((pkg_name.to_owned(), cr))
        });

    Ok(root_pkg.into_iter().chain(dep_pkgs).collect())
}

fn extract_package_name(cargo_toml: &DocumentMut) -> Option<&str> {
    cargo_toml.get("package")?.get("name")?.as_str()
}

fn target_dep_tables(cargo_toml: &DocumentMut) -> impl Iterator<Item = &dyn TableLike> {
    cargo_toml
        .get("target")
        .into_iter()
        .filter_map(Item::as_table_like)
        .flat_map(|t| {
            t.iter()
                .map(|(_, value)| value)
                .filter_map(Item::as_table_like)
                .flat_map(dep_tables)
        })
}

fn dep_tables(table: &dyn TableLike) -> impl Iterator<Item = &dyn TableLike> {
    table
        .get("dependencies")
        .into_iter()
        .chain(table.get("dev-dependencies"))
        .filter_map(Item::as_table_like)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! create_test {
        (
            $name:ident,
            $cargo_toml:expr,
            $workspace_toml:expr,
            $( $result:tt )*
        ) => {
            #[test]
            fn $name() {
                let cargo_toml = $cargo_toml.parse::<DocumentMut>()
                    .expect("Parses `Cargo.toml`");
                let workspace_cargo_toml = $workspace_toml.parse::<DocumentMut>()
                    .expect("Parses workspace `Cargo.toml`");

                let workspace_deps = extract_workspace_dependencies(&workspace_cargo_toml)
                    .expect("Extracts workspace dependencies");

                match extract_crate_names(&cargo_toml, workspace_deps)
                    .map(|mut map| map.remove("my_crate"))
                {
                   $( $result )* => (),
                   o => panic!("Invalid result: {:?}", o),
               }
            }
        };
    }

    create_test! {
        deps_with_crate,
        r#"
            [dependencies]
            my_crate = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate"
    }

    // forbidding toml_edit::Item::as_table ought to mean this is OK, but let's have a test too
    create_test! {
        deps_with_crate_inline_table,
        r#"
            dependencies = { my_crate = "0.1" }
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate"
    }

    create_test! {
        dev_deps_with_crate,
        r#"
            [dev-dependencies]
            my_crate = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate"
    }

    create_test! {
        deps_with_crate_renamed,
        r#"
            [dependencies]
            cool = { package = "my_crate", version = "0.1" }
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "cool"
    }

    create_test! {
        deps_with_crate_renamed_second,
        r#"
            [dependencies.cool]
            package = "my_crate"
            version = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "cool"
    }

    create_test! {
        deps_empty,
        r#"
            [dependencies]
        "#,
        "",
        Ok(None)
    }

    create_test! {
        crate_not_found,
        r#"
            [dependencies]
            serde = "1.0"
        "#,
        "",
        Ok(None)
    }

    create_test! {
        target_dependency,
        r#"
            [target.'cfg(target_os="android")'.dependencies]
            my_crate = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate"
    }

    create_test! {
        target_dependency2,
        r#"
            [target.x86_64-pc-windows-gnu.dependencies]
            my_crate = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate"
    }

    create_test! {
        own_crate,
        r#"
            [package]
            name = "my_crate"
        "#,
        "",
        Ok(Some(FoundCrate::Itself))
    }

    create_test! {
        own_crate_and_in_deps,
        r#"
            [package]
            name = "my_crate"

            [dev-dependencies]
            my_crate = "0.1"
        "#,
        "",
        Ok(Some(FoundCrate::Itself))
    }

    create_test! {
        multiple_times,
        r#"
            [dependencies]
            my_crate = { version = "0.5" }
            my-crate-old = { package = "my_crate", version = "0.1" }
        "#,
        "",
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate_old"
    }

    create_test! {
        workspace_deps,
        r#"
            [dependencies]
            my_crate_cool = { workspace = true }
        "#,
        r#"
            [workspace.dependencies]
            my_crate_cool = { package = "my_crate" }
        "#,
        Ok(Some(FoundCrate::Name(name))) if name == "my_crate_cool"
    }
}
