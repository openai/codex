//! A Rust library for build scripts to automatically configure code based on
//! compiler support.  Code snippets are dynamically tested to see if the `rustc`
//! will accept them, rather than hard-coding specific version support.
//!
//!
//! ## Usage
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [build-dependencies]
//! autocfg = "1"
//! ```
//!
//! Then use it in your `build.rs` script to detect compiler features.  For
//! example, to test for 128-bit integer support, it might look like:
//!
//! ```rust
//! extern crate autocfg;
//!
//! fn main() {
//! #   // Normally, cargo will set `OUT_DIR` for build scripts.
//! #   let exe = std::env::current_exe().unwrap();
//! #   std::env::set_var("OUT_DIR", exe.parent().unwrap());
//!     let ac = autocfg::new();
//!     ac.emit_has_type("i128");
//!
//!     // (optional) We don't need to rerun for anything external.
//!     autocfg::rerun_path("build.rs");
//! }
//! ```
//!
//! If the type test succeeds, this will write a `cargo:rustc-cfg=has_i128` line
//! for Cargo, which translates to Rust arguments `--cfg has_i128`.  Then in the
//! rest of your Rust code, you can add `#[cfg(has_i128)]` conditions on code that
//! should only be used when the compiler supports it.
//!
//! ## Caution
//!
//! Many of the probing methods of `AutoCfg` document the particular template they
//! use, **subject to change**. The inputs are not validated to make sure they are
//! semantically correct for their expected use, so it's _possible_ to escape and
//! inject something unintended. However, such abuse is unsupported and will not
//! be considered when making changes to the templates.

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
// allow future warnings that can't be fixed while keeping 1.0 compatibility
#![allow(unknown_lints)]
#![allow(bare_trait_objects)]
#![allow(ellipsis_inclusive_range_patterns)]

/// Local macro to avoid `std::try!`, deprecated in Rust 1.39.
macro_rules! try {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    };
}

use std::env;
use std::ffi::OsString;
use std::fmt::Arguments;
use std::fs;
use std::io::{stderr, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
#[allow(deprecated)]
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::atomic::{AtomicUsize, Ordering};

mod error;
pub use error::Error;

mod rustc;
use rustc::Rustc;

mod version;
use version::Version;

#[cfg(test)]
mod tests;

/// Helper to detect compiler features for `cfg` output in build scripts.
#[derive(Clone, Debug)]
pub struct AutoCfg {
    out_dir: PathBuf,
    rustc: Rustc,
    rustc_version: Version,
    target: Option<OsString>,
    no_std: bool,
    edition: Option<String>,
    rustflags: Vec<String>,
    uuid: u64,
}

/// Writes a config flag for rustc on standard out.
///
/// This looks like: `cargo:rustc-cfg=CFG`
///
/// Cargo will use this in arguments to rustc, like `--cfg CFG`.
///
/// This does not automatically call [`emit_possibility`]
/// so the compiler my generate an [`unexpected_cfgs` warning][check-cfg-flags].
/// However, all the builtin emit methods on [`AutoCfg`] call [`emit_possibility`] automatically.
///
/// [check-cfg-flags]: https://blog.rust-lang.org/2024/05/06/check-cfg.html
pub fn emit(cfg: &str) {
    println!("cargo:rustc-cfg={}", cfg);
}

/// Writes a line telling Cargo to rerun the build script if `path` changes.
///
/// This looks like: `cargo:rerun-if-changed=PATH`
///
/// This requires at least cargo 0.7.0, corresponding to rustc 1.6.0.  Earlier
/// versions of cargo will simply ignore the directive.
pub fn rerun_path(path: &str) {
    println!("cargo:rerun-if-changed={}", path);
}

/// Writes a line telling Cargo to rerun the build script if the environment
/// variable `var` changes.
///
/// This looks like: `cargo:rerun-if-env-changed=VAR`
///
/// This requires at least cargo 0.21.0, corresponding to rustc 1.20.0.  Earlier
/// versions of cargo will simply ignore the directive.
pub fn rerun_env(var: &str) {
    println!("cargo:rerun-if-env-changed={}", var);
}

/// Indicates to rustc that a config flag should not generate an [`unexpected_cfgs` warning][check-cfg-flags]
///
/// This looks like `cargo:rustc-check-cfg=cfg(VAR)`
///
/// As of rust 1.80, the compiler does [automatic checking of cfgs at compile time][check-cfg-flags].
/// All custom configuration flags must be known to rustc, or they will generate a warning.
/// This is done automatically when calling the builtin emit methods on [`AutoCfg`],
/// but not when calling [`autocfg::emit`](crate::emit) directly.
///
/// Versions before rust 1.80 will simply ignore this directive.
///
/// This function indicates to the compiler that the config flag never has a value.
/// If this is not desired, see [the blog post][check-cfg].
///
/// [check-cfg-flags]: https://blog.rust-lang.org/2024/05/06/check-cfg.html
pub fn emit_possibility(cfg: &str) {
    println!("cargo:rustc-check-cfg=cfg({})", cfg);
}

/// Creates a new `AutoCfg` instance.
///
/// # Panics
///
/// Panics if `AutoCfg::new()` returns an error.
pub fn new() -> AutoCfg {
    AutoCfg::new().unwrap()
}

impl AutoCfg {
    /// Creates a new `AutoCfg` instance.
    ///
    /// # Common errors
    ///
    /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
    /// - The version output from `rustc` can't be parsed.
    /// - `OUT_DIR` is not set in the environment, or is not a writable directory.
    ///
    pub fn new() -> Result<Self, Error> {
        match env::var_os("OUT_DIR") {
            Some(d) => Self::with_dir(d),
            None => Err(error::from_str("no OUT_DIR specified!")),
        }
    }

    /// Creates a new `AutoCfg` instance with the specified output directory.
    ///
    /// # Common errors
    ///
    /// - `rustc` can't be executed, from `RUSTC` or in the `PATH`.
    /// - The version output from `rustc` can't be parsed.
    /// - `dir` is not a writable directory.
    ///
    pub fn with_dir<T: Into<PathBuf>>(dir: T) -> Result<Self, Error> {
        let rustc = Rustc::new();
        let rustc_version = try!(rustc.version());

        let target = env::var_os("TARGET");

        // Sanity check the output directory
        let dir = dir.into();
        let meta = try!(fs::metadata(&dir).map_err(error::from_io));
        if !meta.is_dir() || meta.permissions().readonly() {
            return Err(error::from_str("output path is not a writable directory"));
        }

        let mut ac = AutoCfg {
            rustflags: rustflags(&target, &dir),
            out_dir: dir,
            rustc: rustc,
            rustc_version: rustc_version,
            target: target,
            no_std: false,
            edition: None,
            uuid: new_uuid(),
        };

        // Sanity check with and without `std`.
        if ac.probe_raw("").is_err() {
            if ac.probe_raw("#![no_std]").is_ok() {
                ac.no_std = true;
            } else {
                // Neither worked, so assume nothing...
                let warning = b"warning: autocfg could not probe for `std`\n";
                stderr().write_all(warning).ok();
            }
        }
        Ok(ac)
    }

    /// Returns whether `AutoCfg` is using `#![no_std]` in its probes.
    ///
    /// This is automatically detected during construction -- if an empty probe
    /// fails while one with `#![no_std]` succeeds, then the attribute will be
    /// used for all further probes. This is usually only necessary when the
    /// `TARGET` lacks `std` altogether. If neither succeeds, `no_std` is not
    /// set, but that `AutoCfg` will probably only work for version checks.
    ///
    /// This attribute changes the implicit [prelude] from `std` to `core`,
    /// which may affect the paths you need to use in other probes. It also
    /// restricts some types that otherwise get additional methods in `std`,
    /// like floating-point trigonometry and slice sorting.
    ///
    /// See also [`set_no_std`](#method.set_no_std).
    ///
    /// [prelude]: https://doc.rust-lang.org/reference/names/preludes.html#the-no_std-attribute
    pub fn no_std(&self) -> bool {
        self.no_std
    }

    /// Sets whether `AutoCfg` should use `#![no_std]` in its probes.
    ///
    /// See also [`no_std`](#method.no_std).
    pub fn set_no_std(&mut self, no_std: bool) {
        self.no_std = no_std;
    }

    /// Returns the `--edition` string that is currently being passed to `rustc`, if any,
    /// as configured by the [`set_edition`][Self::set_edition] method.
    pub fn edition(&self) -> Option<&str> {
        match self.edition {
            Some(ref edition) => Some(&**edition),
            None => None,
        }
    }

    /// Sets the `--edition` string that will be passed to `rustc`,
    /// or `None` to leave the compiler at its default edition.
    ///
    /// See also [The Rust Edition Guide](https://doc.rust-lang.org/edition-guide/).
    ///
    /// **Warning:** Setting an unsupported edition will likely cause **all** subsequent probes to
    /// fail! As of this writing, the known editions and their minimum Rust versions are:
    ///
    /// | Edition | Version    |
    /// | ------- | ---------- |
    /// | 2015    | 1.27.0[^1] |
    /// | 2018    | 1.31.0     |
    /// | 2021    | 1.56.0     |
    /// | 2024    | 1.85.0     |
    ///
    /// [^1]: Prior to 1.27.0, Rust was effectively 2015 Edition by default, but the concept hadn't
    /// been established yet, so the explicit `--edition` flag wasn't supported either.
    pub fn set_edition(&mut self, edition: Option<String>) {
        self.edition = edition;
    }

    /// Tests whether the current `rustc` reports a version greater than
    /// or equal to "`major`.`minor`".
    pub fn probe_rustc_version(&self, major: usize, minor: usize) -> bool {
        self.rustc_version >= Version::new(major, minor, 0)
    }

    /// Sets a `cfg` value of the form `rustc_major_minor`, like `rustc_1_29`,
    /// if the current `rustc` is at least that version.
    pub fn emit_rustc_version(&self, major: usize, minor: usize) {
        let cfg_flag = format!("rustc_{}_{}", major, minor);
        emit_possibility(&cfg_flag);
        if self.probe_rustc_version(major, minor) {
            emit(&cfg_flag);
        }
    }

    /// Returns a new (hopefully unique) crate name for probes.
    fn new_crate_name(&self) -> String {
        #[allow(deprecated)]
        static ID: AtomicUsize = ATOMIC_USIZE_INIT;

        let id = ID.fetch_add(1, Ordering::Relaxed);
        format!("autocfg_{:016x}_{}", self.uuid, id)
    }

    fn probe_fmt<'a>(&self, source: Arguments<'a>) -> Result<(), Error> {
        let crate_name = self.new_crate_name();
        let mut command = self.rustc.command();
        command
            .arg("--crate-name")
            .arg(&crate_name)
            .arg("--crate-type=lib")
            .arg("--out-dir")
            .arg(&self.out_dir)
            .arg("--emit=llvm-ir");

        if let Some(edition) = self.edition.as_ref() {
            command.arg("--edition").arg(edition);
        }

        if let Some(target) = self.target.as_ref() {
            command.arg("--target").arg(target);
        }

        command.args(&self.rustflags);

        command.arg("-").stdin(Stdio::piped());
        let mut child = try!(command.spawn().map_err(error::from_io));
        let mut stdin = child.stdin.take().expect("rustc stdin");

        try!(stdin.write_fmt(source).map_err(error::from_io));
        drop(stdin);

        match child.wait() {
            Ok(status) if status.success() => {
                // Try to remove the output file so it doesn't look like a build product for
                // systems like bazel -- but this is best-effort, so we can ignore failure.
                // The probe itself is already considered successful at this point.
                let mut file = self.out_dir.join(crate_name);
                file.set_extension("ll");
                let _ = fs::remove_file(file);

                Ok(())
            }
            Ok(status) => Err(error::from_exit(status)),
            Err(error) => Err(error::from_io(error)),
        }
    }

    fn probe<'a>(&self, code: Arguments<'a>) -> bool {
        let result = if self.no_std {
            self.probe_fmt(format_args!("#![no_std]\n{}", code))
        } else {
            self.probe_fmt(code)
        };
        result.is_ok()
    }

    /// Tests whether the given code can be compiled as a Rust library.
    ///
    /// This will only return `Ok` if the compiler ran and exited successfully,
    /// per `ExitStatus::success()`.
    /// The code is passed to the compiler exactly as-is, notably not even
    /// adding the [`#![no_std]`][Self::no_std] attribute like other probes.
    ///
    /// Raw probes are useful for testing functionality that's not yet covered
    /// by the rest of the `AutoCfg` API. For example, the following attribute
    /// **must** be used at the crate level, so it wouldn't work within the code
    /// templates used by other `probe_*` methods.
    ///
    /// ```
    /// # extern crate autocfg;
    /// # // Normally, cargo will set `OUT_DIR` for build scripts.
    /// # let exe = std::env::current_exe().unwrap();
    /// # std::env::set_var("OUT_DIR", exe.parent().unwrap());
    /// let ac = autocfg::new();
    /// assert!(ac.probe_raw("#![no_builtins]").is_ok());
    /// ```
    ///
    /// Rust nightly features could be tested as well -- ideally including a
    /// code sample to ensure the unstable feature still works as expected.
    /// For example, `slice::group_by` was renamed to `chunk_by` when it was
    /// stabilized, even though the feature name was unchanged, so testing the
    /// `#![feature(..)]` alone wouldn't reveal that. For larger snippets,
    /// [`include_str!`] may be useful to load them from separate files.
    ///
    /// ```
    /// # extern crate autocfg;
    /// # // Normally, cargo will set `OUT_DIR` for build scripts.
    /// # let exe = std::env::current_exe().unwrap();
    /// # std::env::set_var("OUT_DIR", exe.parent().unwrap());
    /// let ac = autocfg::new();
    /// let code = r#"
    ///     #![feature(slice_group_by)]
    ///     pub fn probe(slice: &[i32]) -> impl Iterator<Item = &[i32]> {
    ///         slice.group_by(|a, b| a == b)
    ///     }
    /// "#;
    /// if ac.probe_raw(code).is_ok() {
    ///     autocfg::emit("has_slice_group_by");
    /// }
    /// ```
    pub fn probe_raw(&self, code: &str) -> Result<(), Error> {
        self.probe_fmt(format_args!("{}", code))
    }

    /// Tests whether the given sysroot crate can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// extern crate CRATE as probe;
    /// ```
    pub fn probe_sysroot_crate(&self, name: &str) -> bool {
        // Note: `as _` wasn't stabilized until Rust 1.33
        self.probe(format_args!("extern crate {} as probe;", name))
    }

    /// Emits a config value `has_CRATE` if `probe_sysroot_crate` returns true.
    pub fn emit_sysroot_crate(&self, name: &str) {
        let cfg_flag = format!("has_{}", mangle(name));
        emit_possibility(&cfg_flag);
        if self.probe_sysroot_crate(name) {
            emit(&cfg_flag);
        }
    }

    /// Tests whether the given path can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// pub use PATH;
    /// ```
    pub fn probe_path(&self, path: &str) -> bool {
        self.probe(format_args!("pub use {};", path))
    }

    /// Emits a config value `has_PATH` if `probe_path` returns true.
    ///
    /// Any non-identifier characters in the `path` will be replaced with
    /// `_` in the generated config value.
    pub fn emit_has_path(&self, path: &str) {
        self.emit_path_cfg(path, &format!("has_{}", mangle(path)));
    }

    /// Emits the given `cfg` value if `probe_path` returns true.
    pub fn emit_path_cfg(&self, path: &str, cfg: &str) {
        emit_possibility(cfg);
        if self.probe_path(path) {
            emit(cfg);
        }
    }

    /// Tests whether the given trait can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// pub trait Probe: TRAIT + Sized {}
    /// ```
    pub fn probe_trait(&self, name: &str) -> bool {
        self.probe(format_args!("pub trait Probe: {} + Sized {{}}", name))
    }

    /// Emits a config value `has_TRAIT` if `probe_trait` returns true.
    ///
    /// Any non-identifier characters in the trait `name` will be replaced with
    /// `_` in the generated config value.
    pub fn emit_has_trait(&self, name: &str) {
        self.emit_trait_cfg(name, &format!("has_{}", mangle(name)));
    }

    /// Emits the given `cfg` value if `probe_trait` returns true.
    pub fn emit_trait_cfg(&self, name: &str, cfg: &str) {
        emit_possibility(cfg);
        if self.probe_trait(name) {
            emit(cfg);
        }
    }

    /// Tests whether the given type can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// pub type Probe = TYPE;
    /// ```
    pub fn probe_type(&self, name: &str) -> bool {
        self.probe(format_args!("pub type Probe = {};", name))
    }

    /// Emits a config value `has_TYPE` if `probe_type` returns true.
    ///
    /// Any non-identifier characters in the type `name` will be replaced with
    /// `_` in the generated config value.
    pub fn emit_has_type(&self, name: &str) {
        self.emit_type_cfg(name, &format!("has_{}", mangle(name)));
    }

    /// Emits the given `cfg` value if `probe_type` returns true.
    pub fn emit_type_cfg(&self, name: &str, cfg: &str) {
        emit_possibility(cfg);
        if self.probe_type(name) {
            emit(cfg);
        }
    }

    /// Tests whether the given expression can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// pub fn probe() { let _ = EXPR; }
    /// ```
    pub fn probe_expression(&self, expr: &str) -> bool {
        self.probe(format_args!("pub fn probe() {{ let _ = {}; }}", expr))
    }

    /// Emits the given `cfg` value if `probe_expression` returns true.
    pub fn emit_expression_cfg(&self, expr: &str, cfg: &str) {
        emit_possibility(cfg);
        if self.probe_expression(expr) {
            emit(cfg);
        }
    }

    /// Tests whether the given constant expression can be used.
    ///
    /// The test code is subject to change, but currently looks like:
    ///
    /// ```ignore
    /// pub const PROBE: () = ((), EXPR).0;
    /// ```
    pub fn probe_constant(&self, expr: &str) -> bool {
        self.probe(format_args!("pub const PROBE: () = ((), {}).0;", expr))
    }

    /// Emits the given `cfg` value if `probe_constant` returns true.
    pub fn emit_constant_cfg(&self, expr: &str, cfg: &str) {
        emit_possibility(cfg);
        if self.probe_constant(expr) {
            emit(cfg);
        }
    }
}

fn mangle(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'...'Z' | 'a'...'z' | '0'...'9' => c,
            _ => '_',
        })
        .collect()
}

fn dir_contains_target(
    target: &Option<OsString>,
    dir: &Path,
    cargo_target_dir: Option<OsString>,
) -> bool {
    target
        .as_ref()
        .and_then(|target| {
            dir.to_str().and_then(|dir| {
                let mut cargo_target_dir = cargo_target_dir
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("target"));
                cargo_target_dir.push(target);

                cargo_target_dir
                    .to_str()
                    .map(|cargo_target_dir| dir.contains(cargo_target_dir))
            })
        })
        .unwrap_or(false)
}

fn rustflags(target: &Option<OsString>, dir: &Path) -> Vec<String> {
    // Starting with rust-lang/cargo#9601, shipped in Rust 1.55, Cargo always sets
    // CARGO_ENCODED_RUSTFLAGS for any host/target build script invocation. This
    // includes any source of flags, whether from the environment, toml config, or
    // whatever may come in the future. The value is either an empty string, or a
    // list of arguments separated by the ASCII unit separator (US), 0x1f.
    if let Ok(a) = env::var("CARGO_ENCODED_RUSTFLAGS") {
        return if a.is_empty() {
            Vec::new()
        } else {
            a.split('\x1f').map(str::to_string).collect()
        };
    }

    // Otherwise, we have to take a more heuristic approach, and we don't
    // support values from toml config at all.
    //
    // Cargo only applies RUSTFLAGS for building TARGET artifact in
    // cross-compilation environment. Sadly, we don't have a way to detect
    // when we're building HOST artifact in a cross-compilation environment,
    // so for now we only apply RUSTFLAGS when cross-compiling an artifact.
    //
    // See https://github.com/cuviper/autocfg/pull/10#issuecomment-527575030.
    if *target != env::var_os("HOST")
        || dir_contains_target(target, dir, env::var_os("CARGO_TARGET_DIR"))
    {
        if let Ok(rustflags) = env::var("RUSTFLAGS") {
            // This is meant to match how cargo handles the RUSTFLAGS environment variable.
            // See https://github.com/rust-lang/cargo/blob/69aea5b6f69add7c51cca939a79644080c0b0ba0/src/cargo/core/compiler/build_context/target_info.rs#L434-L441
            return rustflags
                .split(' ')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
    }

    Vec::new()
}

/// Generates a numeric ID to use in probe crate names.
///
/// This attempts to be random, within the constraints of Rust 1.0 and no dependencies.
fn new_uuid() -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;

    // This set should have an actual random hasher.
    let set: std::collections::HashSet<u64> = (0..256).collect();

    // Feed the `HashSet`-shuffled order into FNV-1a.
    let mut hash: u64 = FNV_OFFSET_BASIS;
    for x in set {
        hash = (hash ^ x).wrapping_mul(FNV_PRIME);
    }
    hash
}
