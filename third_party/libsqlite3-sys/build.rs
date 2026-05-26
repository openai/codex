use std::env;
use std::path::Path;

/// Tells whether we're building for Windows. This is more suitable than a plain
/// `cfg!(windows)`, since the latter does not properly handle cross-compilation
///
/// Note that there is no way to know at compile-time which system we'll be
/// targeting, and this test must be made at run-time (of the build script) See
/// https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
fn win_target() -> bool {
    env::var("CARGO_CFG_WINDOWS").is_ok()
}

/// Tells whether we're building for Android.
/// See [`win_target`]
#[cfg(any(feature = "bundled", feature = "bundled-windows"))]
fn android_target() -> bool {
    env::var("CARGO_CFG_TARGET_OS").map_or(false, |v| v == "android")
}

/// Tells whether a given compiler will be used `compiler_name` is compared to
/// the content of `CARGO_CFG_TARGET_ENV` (and is always lowercase)
///
/// See [`win_target`]
fn is_compiler(compiler_name: &str) -> bool {
    env::var("CARGO_CFG_TARGET_ENV").map_or(false, |v| v == compiler_name)
}

/// Copy bindgen file from `dir` to `out_path`.
fn copy_bindings<T: AsRef<Path>>(dir: &str, bindgen_name: &str, out_path: T) {
    let from = if cfg!(feature = "loadable_extension") {
        format!("{dir}/{bindgen_name}_ext.rs")
    } else {
        format!("{dir}/{bindgen_name}.rs")
    };
    std::fs::copy(from, out_path).expect("Could not copy bindings to output directory");
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("bindgen.rs");
    if cfg!(feature = "in_gecko") {
        // When inside mozilla-central, we are included into the build with
        // sqlite3.o directly, so we don't want to provide any linker arguments.
        copy_bindings("sqlite3", "bindgen_bundled_version", out_path);
        return;
    }

    println!("cargo:rerun-if-env-changed=LIBSQLITE3_SYS_USE_PKG_CONFIG");
    if env::var_os("LIBSQLITE3_SYS_USE_PKG_CONFIG").map_or(false, |s| s != "0")
        || cfg!(feature = "loadable_extension")
    {
        build_linked::main(&out_dir, &out_path);
    } else if cfg!(all(
        feature = "sqlcipher",
        not(feature = "bundled-sqlcipher")
    )) {
        if cfg!(feature = "bundled") || (win_target() && cfg!(feature = "bundled-windows")) {
            println!(
                "cargo:warning=For backwards compatibility, feature 'sqlcipher' overrides
                features 'bundled' and 'bundled-windows'. If you want a bundled build of
                SQLCipher (available for the moment only on Unix), use feature 'bundled-sqlcipher'
                or 'bundled-sqlcipher-vendored-openssl' to also bundle OpenSSL crypto."
            );
        }
        build_linked::main(&out_dir, &out_path);
    } else if cfg!(feature = "bundled")
        || (win_target() && cfg!(feature = "bundled-windows"))
        || cfg!(feature = "bundled-sqlcipher")
    {
        #[cfg(any(
            feature = "bundled",
            feature = "bundled-windows",
            feature = "bundled-sqlcipher"
        ))]
        build_bundled::main(&out_dir, &out_path);
        #[cfg(not(any(
            feature = "bundled",
            feature = "bundled-windows",
            feature = "bundled-sqlcipher"
        )))]
        panic!("The runtime test should not run this branch, which has not compiled any logic.")
    } else {
        build_linked::main(&out_dir, &out_path);
    }
}

#[cfg(any(
    feature = "bundled",
    feature = "bundled-windows",
    feature = "bundled-sqlcipher"
))]
mod build_bundled {
    use std::env;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{is_compiler, win_target};

    pub fn main(out_dir: &str, out_path: &Path) {
        let lib_name = super::lib_name();

        // This is just a sanity check, the top level `main` should ensure this.
        assert!(!(cfg!(feature = "bundled-windows") && !cfg!(feature = "bundled") && !win_target()),
            "This module should not be used: we're not on Windows and the bundled feature has not been enabled");

        #[cfg(feature = "buildtime_bindgen")]
        {
            use super::{bindings, HeaderLocation};
            let header = HeaderLocation::FromPath(lib_name.to_owned());
            bindings::write_to_out_dir(header, out_path);
        }
        #[cfg(not(feature = "buildtime_bindgen"))]
        {
            super::copy_bindings(lib_name, "bindgen_bundled_version", out_path);
        }
        println!("cargo:include={}/{lib_name}", env!("CARGO_MANIFEST_DIR"));
        println!("cargo:rerun-if-changed={lib_name}/sqlite3.c");
        println!("cargo:rerun-if-changed=sqlite3/wasm32-wasi-vfs.c");
        let mut cfg = cc::Build::new();
        cfg.file(format!("{lib_name}/sqlite3.c"))
            .flag("-DSQLITE_CORE")
            .flag("-DSQLITE_DEFAULT_FOREIGN_KEYS=1")
            .flag("-DSQLITE_ENABLE_API_ARMOR")
            .flag("-DSQLITE_ENABLE_COLUMN_METADATA")
            .flag("-DSQLITE_ENABLE_DBSTAT_VTAB")
            .flag("-DSQLITE_ENABLE_FTS3")
            .flag("-DSQLITE_ENABLE_FTS3_PARENTHESIS")
            .flag("-DSQLITE_ENABLE_FTS5")
            .flag("-DSQLITE_ENABLE_JSON1")
            .flag("-DSQLITE_ENABLE_LOAD_EXTENSION=1")
            .flag("-DSQLITE_ENABLE_MEMORY_MANAGEMENT")
            .flag("-DSQLITE_ENABLE_RTREE")
            .flag("-DSQLITE_ENABLE_STAT4")
            .flag("-DSQLITE_SOUNDEX")
            .flag("-DSQLITE_THREADSAFE=1")
            .flag("-DSQLITE_USE_URI")
            .flag("-DHAVE_USLEEP=1")
            .flag("-DHAVE_ISNAN")
            .flag("-D_POSIX_THREAD_SAFE_FUNCTIONS") // cross compile with MinGW
            .warnings(false);

        if cfg!(feature = "bundled-sqlcipher") {
            cfg.flag("-DSQLITE_HAS_CODEC").flag("-DSQLITE_TEMP_STORE=2");

            let target = env::var("TARGET").unwrap();
            let host = env::var("HOST").unwrap();

            let is_windows = host.contains("windows") && target.contains("windows");
            let is_apple = host.contains("apple") && target.contains("apple");

            let lib_dir = env("OPENSSL_LIB_DIR").map(PathBuf::from);
            let inc_dir = env("OPENSSL_INCLUDE_DIR").map(PathBuf::from);
            let mut use_openssl = false;

            let (lib_dir, inc_dir) = match (lib_dir, inc_dir) {
                (Some(lib_dir), Some(inc_dir)) => {
                    use_openssl = true;
                    (vec![lib_dir], inc_dir)
                }
                (lib_dir, inc_dir) => match find_openssl_dir(&host, &target) {
                    None => {
                        if is_windows && !cfg!(feature = "bundled-sqlcipher-vendored-openssl") {
                            panic!("Missing environment variable OPENSSL_DIR or OPENSSL_DIR is not set")
                        } else {
                            (vec![PathBuf::new()], PathBuf::new())
                        }
                    }
                    Some(openssl_dir) => {
                        let lib_dir = lib_dir.map(|d| vec![d]).unwrap_or_else(|| {
                            let mut lib_dirs = vec![];
                            // OpenSSL 3.0 now puts its libraries in lib64/ by default,
                            // check for both it and lib/.
                            if openssl_dir.join("lib64").exists() {
                                lib_dirs.push(openssl_dir.join("lib64"));
                            }
                            if openssl_dir.join("lib").exists() {
                                lib_dirs.push(openssl_dir.join("lib"));
                            }
                            lib_dirs
                        });
                        let inc_dir = inc_dir.unwrap_or_else(|| openssl_dir.join("include"));

                        if !lib_dir.iter().all(|p| p.exists()) {
                            panic!("OpenSSL library directory does not exist: {:?}", lib_dir);
                        }

                        if !Path::new(&inc_dir).exists() {
                            panic!(
                                "OpenSSL include directory does not exist: {}",
                                inc_dir.to_string_lossy()
                            );
                        }

                        use_openssl = true;
                        (lib_dir, inc_dir)
                    }
                },
            };

            if cfg!(feature = "bundled-sqlcipher-vendored-openssl") {
                cfg.include(env::var("DEP_OPENSSL_INCLUDE").unwrap());
                // cargo will resolve downstream to the static lib in
                // openssl-sys
            } else if use_openssl {
                cfg.include(inc_dir.to_string_lossy().as_ref());
                let lib_name = if is_windows { "libcrypto" } else { "crypto" };
                println!("cargo:rustc-link-lib=dylib={}", lib_name);
                for lib_dir_item in lib_dir.iter() {
                    println!("cargo:rustc-link-search={}", lib_dir_item.to_string_lossy());
                }
            } else if is_apple {
                cfg.flag("-DSQLCIPHER_CRYPTO_CC");
                println!("cargo:rustc-link-lib=framework=Security");
                println!("cargo:rustc-link-lib=framework=CoreFoundation");
            } else {
                // branch not taken on Windows, just `crypto` is fine.
                println!("cargo:rustc-link-lib=dylib=crypto");
            }
        }

        // on android sqlite can't figure out where to put the temp files.
        // the bundled sqlite on android also uses `SQLITE_TEMP_STORE=3`.
        // https://android.googlesource.com/platform/external/sqlite/+/2c8c9ae3b7e6f340a19a0001c2a889a211c9d8b2/dist/Android.mk
        if super::android_target() {
            cfg.flag("-DSQLITE_TEMP_STORE=3");
        }

        if cfg!(feature = "with-asan") {
            cfg.flag("-fsanitize=address");
        }

        // If explicitly requested: enable static linking against the Microsoft Visual
        // C++ Runtime to avoid dependencies on vcruntime140.dll and similar libraries.
        if cfg!(target_feature = "crt-static") && is_compiler("msvc") {
            cfg.static_crt(true);
        }

        if !win_target() {
            cfg.flag("-DHAVE_LOCALTIME_R");
        }
        if env::var("TARGET").map_or(false, |v| v == "wasm32-wasi") {
            cfg.flag("-USQLITE_THREADSAFE")
                .flag("-DSQLITE_THREADSAFE=0")
                // https://github.com/rust-lang/rust/issues/74393
                .flag("-DLONGDOUBLE_TYPE=double")
                .flag("-D_WASI_EMULATED_MMAN")
                .flag("-D_WASI_EMULATED_GETPID")
                .flag("-D_WASI_EMULATED_SIGNAL")
                .flag("-D_WASI_EMULATED_PROCESS_CLOCKS");

            if cfg!(feature = "wasm32-wasi-vfs") {
                cfg.file("sqlite3/wasm32-wasi-vfs.c");
            }
        }
        if cfg!(feature = "unlock_notify") {
            cfg.flag("-DSQLITE_ENABLE_UNLOCK_NOTIFY");
        }
        if cfg!(feature = "preupdate_hook") {
            cfg.flag("-DSQLITE_ENABLE_PREUPDATE_HOOK");
        }
        if cfg!(feature = "session") {
            cfg.flag("-DSQLITE_ENABLE_SESSION");
        }

        if let Ok(limit) = env::var("SQLITE_MAX_VARIABLE_NUMBER") {
            cfg.flag(format!("-DSQLITE_MAX_VARIABLE_NUMBER={limit}"));
        }
        println!("cargo:rerun-if-env-changed=SQLITE_MAX_VARIABLE_NUMBER");

        if let Ok(limit) = env::var("SQLITE_MAX_EXPR_DEPTH") {
            cfg.flag(format!("-DSQLITE_MAX_EXPR_DEPTH={limit}"));
        }
        println!("cargo:rerun-if-env-changed=SQLITE_MAX_EXPR_DEPTH");

        if let Ok(limit) = env::var("SQLITE_MAX_COLUMN") {
            cfg.flag(format!("-DSQLITE_MAX_COLUMN={limit}"));
        }
        println!("cargo:rerun-if-env-changed=SQLITE_MAX_COLUMN");

        if let Ok(extras) = env::var("LIBSQLITE3_FLAGS") {
            for extra in extras.split_whitespace() {
                if extra.starts_with("-D") || extra.starts_with("-U") {
                    cfg.flag(extra);
                } else if extra.starts_with("SQLITE_") {
                    cfg.flag(format!("-D{extra}"));
                } else {
                    panic!("Don't understand {} in LIBSQLITE3_FLAGS", extra);
                }
            }
        }
        println!("cargo:rerun-if-env-changed=LIBSQLITE3_FLAGS");

        cfg.compile(lib_name);

        println!("cargo:lib_dir={out_dir}");
    }

    fn env(name: &str) -> Option<OsString> {
        let prefix = env::var("TARGET").unwrap().to_uppercase().replace('-', "_");
        let prefixed = format!("{prefix}_{name}");
        let var = env::var_os(prefixed);

        match var {
            None => env::var_os(name),
            _ => var,
        }
    }

    fn find_openssl_dir(_host: &str, _target: &str) -> Option<PathBuf> {
        let openssl_dir = env("OPENSSL_DIR");
        openssl_dir.map(PathBuf::from)
    }
}

fn env_prefix() -> &'static str {
    if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
        "SQLCIPHER"
    } else {
        "SQLITE3"
    }
}

fn lib_name() -> &'static str {
    if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
        "sqlcipher"
    } else {
        "sqlite3"
    }
}

pub enum HeaderLocation {
    FromEnvironment,
    Wrapper,
    FromPath(String),
}

impl From<HeaderLocation> for String {
    fn from(header: HeaderLocation) -> String {
        match header {
            HeaderLocation::FromEnvironment => {
                let prefix = env_prefix();
                let mut header = env::var(format!("{prefix}_INCLUDE_DIR")).unwrap_or_else(|_| {
                    panic!("{prefix}_INCLUDE_DIR must be set if {prefix}_LIB_DIR is set")
                });
                header.push_str(if cfg!(feature = "loadable_extension") {
                    "/sqlite3ext.h"
                } else {
                    "/sqlite3.h"
                });
                header
            }
            HeaderLocation::Wrapper => if cfg!(feature = "loadable_extension") {
                "wrapper_ext.h"
            } else {
                "wrapper.h"
            }
            .into(),
            HeaderLocation::FromPath(path) => format!(
                "{}/{}",
                path,
                if cfg!(feature = "loadable_extension") {
                    "sqlite3ext.h"
                } else {
                    "sqlite3.h"
                }
            ),
        }
    }
}

mod build_linked {
    #[cfg(feature = "vcpkg")]
    extern crate vcpkg;

    use super::{bindings, env_prefix, is_compiler, lib_name, win_target, HeaderLocation};
    use std::env;
    use std::path::Path;

    pub fn main(_out_dir: &str, out_path: &Path) {
        let header = find_sqlite();
        if (cfg!(any(
            feature = "bundled_bindings",
            feature = "bundled",
            feature = "bundled-sqlcipher"
        )) || (win_target() && cfg!(feature = "bundled-windows")))
            && !cfg!(feature = "buildtime_bindgen")
        {
            // Generally means the `bundled_bindings` feature is enabled.
            // Most users are better off with turning
            // on buildtime_bindgen instead, but this is still supported as we
            // have runtime version checks and there are good reasons to not
            // want to run bindgen.
            super::copy_bindings(lib_name(), "bindgen_bundled_version", out_path);
        } else {
            bindings::write_to_out_dir(header, out_path);
        }
    }

    #[cfg(not(feature = "loadable_extension"))]
    fn find_link_mode() -> &'static str {
        // If the user specifies SQLITE3_STATIC (or SQLCIPHER_STATIC), do static
        // linking, unless it's explicitly set to 0.
        match &env::var(format!("{}_STATIC", env_prefix())) {
            Ok(v) if v != "0" => "static",
            _ => "dylib",
        }
    }
    // Prints the necessary cargo link commands and returns the path to the header.
    fn find_sqlite() -> HeaderLocation {
        let link_lib = lib_name();

        println!("cargo:rerun-if-env-changed={}_INCLUDE_DIR", env_prefix());
        println!("cargo:rerun-if-env-changed={}_LIB_DIR", env_prefix());
        println!("cargo:rerun-if-env-changed={}_STATIC", env_prefix());
        if cfg!(feature = "vcpkg") && is_compiler("msvc") {
            println!("cargo:rerun-if-env-changed=VCPKGRS_DYNAMIC");
        }

        // dependents can access `DEP_SQLITE3_LINK_TARGET` (`sqlite3` being the
        // `links=` value in our Cargo.toml) to get this value. This might be
        // useful if you need to ensure whatever crypto library sqlcipher relies
        // on is available, for example.
        #[cfg(not(feature = "loadable_extension"))]
        println!("cargo:link-target={link_lib}");

        // Allow users to specify where to find SQLite.
        if let Ok(dir) = env::var(format!("{}_LIB_DIR", env_prefix())) {
            // Try to use pkg-config to determine link commands
            let pkgconfig_path = Path::new(&dir).join("pkgconfig");
            env::set_var("PKG_CONFIG_PATH", pkgconfig_path);
            #[cfg(not(feature = "loadable_extension"))]
            if pkg_config::Config::new().probe(link_lib).is_err() {
                // Otherwise just emit the bare minimum link commands.
                println!("cargo:rustc-link-lib={}={link_lib}", find_link_mode());
                println!("cargo:rustc-link-search={dir}");
            }
            return HeaderLocation::FromEnvironment;
        }

        if let Some(header) = try_vcpkg() {
            return header;
        }

        // See if pkg-config can do everything for us.
        if let Ok(mut lib) = pkg_config::Config::new()
            .print_system_libs(false)
            .probe(link_lib)
        {
            if let Some(header) = lib.include_paths.pop() {
                HeaderLocation::FromPath(header.to_string_lossy().into())
            } else {
                HeaderLocation::Wrapper
            }
        } else {
            // No env var set and pkg-config couldn't help; just output the link-lib
            // request and hope that the library exists on the system paths. We used to
            // output /usr/lib explicitly, but that can introduce other linking problems;
            // see https://github.com/rusqlite/rusqlite/issues/207.
            #[cfg(not(feature = "loadable_extension"))]
            println!("cargo:rustc-link-lib={}={link_lib}", find_link_mode());
            HeaderLocation::Wrapper
        }
    }

    fn try_vcpkg() -> Option<HeaderLocation> {
        if cfg!(feature = "vcpkg") && is_compiler("msvc") {
            // See if vcpkg can find it.
            if let Ok(mut lib) = vcpkg::Config::new().probe(lib_name()) {
                if let Some(header) = lib.include_paths.pop() {
                    return Some(HeaderLocation::FromPath(header.to_string_lossy().into()));
                }
            }
            None
        } else {
            None
        }
    }
}

#[cfg(not(feature = "buildtime_bindgen"))]
#[allow(dead_code)]
mod bindings {
    use super::HeaderLocation;

    use std::path::Path;

    static PREBUILT_BINDGENS: &[&str] = &["bindgen_3.14.0"];

    pub fn write_to_out_dir(_header: HeaderLocation, out_path: &Path) {
        let name = PREBUILT_BINDGENS[PREBUILT_BINDGENS.len() - 1];
        super::copy_bindings("bindgen-bindings", name, out_path);
    }
}

#[cfg(feature = "buildtime_bindgen")]
mod bindings {
    use super::HeaderLocation;
    use bindgen::callbacks::{IntKind, ParseCallbacks};

    use std::path::Path;
    #[derive(Debug)]
    struct SqliteTypeChooser;

    impl ParseCallbacks for SqliteTypeChooser {
        fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
            if name == "SQLITE_SERIALIZE_NOCOPY"
                || name.starts_with("SQLITE_DESERIALIZE_")
                || name.starts_with("SQLITE_PREPARE_")
            {
                Some(IntKind::UInt)
            } else {
                None
            }
        }
    }

    // Are we generating the bundled bindings? Used to avoid emitting things
    // that would be problematic in bundled builds. This env var is set by
    // `upgrade.sh`.
    fn generating_bundled_bindings() -> bool {
        // Hacky way to know if we're generating the bundled bindings
        println!("cargo:rerun-if-env-changed=LIBSQLITE3_SYS_BUNDLING");
        match std::env::var("LIBSQLITE3_SYS_BUNDLING") {
            Ok(v) => v != "0",
            Err(_) => false,
        }
    }

    pub fn write_to_out_dir(header: HeaderLocation, out_path: &Path) {
        let header: String = header.into();
        let mut bindings = bindgen::builder()
            .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
            .disable_nested_struct_naming()
            .trust_clang_mangling(false)
            .header(header.clone())
            .parse_callbacks(Box::new(SqliteTypeChooser));
        if cfg!(feature = "loadable_extension") {
            bindings = bindings.ignore_functions(); // see generate_functions
        } else {
            bindings = bindings
                .blocklist_function("sqlite3_auto_extension")
                .raw_line(
                    r#"extern "C" {
    pub fn sqlite3_auto_extension(
        xEntryPoint: ::std::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::std::os::raw::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::std::os::raw::c_int,
        >,
    ) -> ::std::os::raw::c_int;
}"#,
                )
                .blocklist_function("sqlite3_cancel_auto_extension")
                .raw_line(
                    r#"extern "C" {
    pub fn sqlite3_cancel_auto_extension(
        xEntryPoint: ::std::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::std::os::raw::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::std::os::raw::c_int,
        >,
    ) -> ::std::os::raw::c_int;
}"#,
                )
                .blocklist_function(".*16.*")
                .blocklist_function("sqlite3_close_v2")
                .blocklist_function("sqlite3_create_collation")
                .blocklist_function("sqlite3_create_function")
                .blocklist_function("sqlite3_create_module")
                .blocklist_function("sqlite3_prepare");
        }

        if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
            bindings = bindings.clang_arg("-DSQLITE_HAS_CODEC");
        }
        if cfg!(feature = "unlock_notify") {
            bindings = bindings.clang_arg("-DSQLITE_ENABLE_UNLOCK_NOTIFY");
        }
        if cfg!(feature = "preupdate_hook") {
            bindings = bindings.clang_arg("-DSQLITE_ENABLE_PREUPDATE_HOOK");
        }
        if cfg!(feature = "session") {
            bindings = bindings.clang_arg("-DSQLITE_ENABLE_SESSION");
        }

        // When cross compiling unless effort is taken to fix the issue, bindgen
        // will find the wrong headers. There's only one header included by the
        // amalgamated `sqlite.h`: `stdarg.h`.
        //
        // Thankfully, there's almost no case where rust code needs to use
        // functions taking `va_list` (It's nearly impossible to get a `va_list`
        // in Rust unless you get passed it by C code for some reason).
        //
        // Arguably, we should never be including these, but we include them for
        // the cases where they aren't totally broken...
        let target_arch = std::env::var("TARGET").unwrap();
        let host_arch = std::env::var("HOST").unwrap();
        let is_cross_compiling = target_arch != host_arch;

        // Note that when generating the bundled file, we're essentially always
        // cross compiling.
        if generating_bundled_bindings() || is_cross_compiling {
            // Get rid of va_list, as it's not
            bindings = bindings
                .blocklist_function("sqlite3_vmprintf")
                .blocklist_function("sqlite3_vsnprintf")
                .blocklist_function("sqlite3_str_vappendf")
                .blocklist_type("va_list")
                .blocklist_item("__.*");
        }

        let bindings = bindings
            .layout_tests(false)
            .generate()
            .unwrap_or_else(|_| panic!("could not run bindgen on header {}", header));

        #[cfg(feature = "loadable_extension")]
        {
            let mut output = Vec::new();
            bindings
                .write(Box::new(&mut output))
                .expect("could not write output of bindgen");
            let mut output = String::from_utf8(output).expect("bindgen output was not UTF-8?!");
            super::loadable_extension::generate_functions(&mut output);
            std::fs::write(out_path, output.as_bytes())
                .unwrap_or_else(|_| panic!("Could not write to {:?}", out_path));
        }
        #[cfg(not(feature = "loadable_extension"))]
        bindings
            .write_to_file(out_path)
            .unwrap_or_else(|_| panic!("Could not write to {:?}", out_path));
    }
}

#[cfg(all(feature = "buildtime_bindgen", feature = "loadable_extension"))]
mod loadable_extension {
    /// try to generate similar rust code for all `#define sqlite3_xyz
    /// sqlite3_api->abc` macros` in sqlite3ext.h
    pub fn generate_functions(output: &mut String) {
        // (1) parse sqlite3_api_routines fields from bindgen output
        let ast: syn::File = syn::parse_str(output).expect("could not parse bindgen output");
        let sqlite3_api_routines: syn::ItemStruct = ast
            .items
            .into_iter()
            .find_map(|i| {
                if let syn::Item::Struct(s) = i {
                    if s.ident == "sqlite3_api_routines" {
                        Some(s)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .expect("could not find sqlite3_api_routines");
        let sqlite3_api_routines_ident = sqlite3_api_routines.ident;
        let p_api = quote::format_ident!("p_api");
        let mut stores = Vec::new();
        let mut malloc = Vec::new();
        // (2) `#define sqlite3_xyz sqlite3_api->abc` => `pub unsafe fn
        // sqlite3_xyz(args) -> ty {...}` for each `abc` field:
        for field in sqlite3_api_routines.fields {
            let ident = field.ident.expect("unnamed field");
            let span = ident.span();
            let name = ident.to_string();
            if name.contains("16") {
                continue; // skip UTF-16 api as rust uses UTF-8
            } else if name == "vmprintf" || name == "xvsnprintf" || name == "str_vappendf" {
                continue; // skip va_list
            } else if name == "aggregate_count"
                || name == "expired"
                || name == "global_recover"
                || name == "thread_cleanup"
                || name == "transfer_bindings"
                || name == "create_collation"
                || name == "create_function"
                || name == "create_module"
                || name == "prepare"
                || name == "close_v2"
            {
                continue; // omit deprecated
            }
            let sqlite3_name = match name.as_ref() {
                "xthreadsafe" => "sqlite3_threadsafe".to_owned(),
                "interruptx" => "sqlite3_interrupt".to_owned(),
                _ => {
                    format!("sqlite3_{name}")
                }
            };
            let ptr_name =
                syn::Ident::new(format!("__{}", sqlite3_name.to_uppercase()).as_ref(), span);
            let sqlite3_fn_name = syn::Ident::new(&sqlite3_name, span);
            let method =
                extract_method(&field.ty).unwrap_or_else(|| panic!("unexpected type for {name}"));
            let arg_names: syn::punctuated::Punctuated<&syn::Ident, syn::token::Comma> = method
                .inputs
                .iter()
                .map(|i| &i.name.as_ref().unwrap().0)
                .collect();
            let args = &method.inputs;
            // vtab_config/sqlite3_vtab_config: ok
            let varargs = &method.variadic;
            if varargs.is_some() && "db_config" != name && "log" != name && "vtab_config" != name {
                continue; // skip ...
            }
            let ty = &method.output;
            let tokens = if "db_config" == name {
                quote::quote! {
                    static #ptr_name: ::std::sync::atomic::AtomicPtr<()> = ::std::sync::atomic::AtomicPtr::new(::std::ptr::null_mut());
                    pub unsafe fn #sqlite3_fn_name(#args arg3: ::std::os::raw::c_int, arg4: *mut ::std::os::raw::c_int) #ty {
                        let ptr = #ptr_name.load(::std::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::std::mem::transmute(ptr);
                        (fun)(#arg_names, arg3, arg4)
                    }
                }
            } else if "log" == name {
                quote::quote! {
                    static #ptr_name: ::std::sync::atomic::AtomicPtr<()> = ::std::sync::atomic::AtomicPtr::new(::std::ptr::null_mut());
                    pub unsafe fn #sqlite3_fn_name(#args arg3: *const ::std::os::raw::c_char) #ty {
                        let ptr = #ptr_name.load(::std::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::std::mem::transmute(ptr);
                        (fun)(#arg_names, arg3)
                    }
                }
            } else {
                quote::quote! {
                    static #ptr_name: ::std::sync::atomic::AtomicPtr<()> = ::std::sync::atomic::AtomicPtr::new(::std::ptr::null_mut());
                    pub unsafe fn #sqlite3_fn_name(#args) #ty {
                        let ptr = #ptr_name.load(::std::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized or SQLite feature omitted");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::std::mem::transmute(ptr);
                        (fun)(#arg_names)
                    }
                }
            };
            output.push_str(&prettyplease::unparse(
                &syn::parse2(tokens).expect("could not parse quote output"),
            ));
            output.push('\n');
            if name == "malloc" {
                &mut malloc
            } else {
                &mut stores
            }
            .push(quote::quote! {
                if let Some(fun) = (*#p_api).#ident {
                    #ptr_name.store(
                        fun as usize as *mut (),
                        ::std::sync::atomic::Ordering::Release,
                    );
                }
            });
        }
        // (3) generate rust code similar to SQLITE_EXTENSION_INIT2 macro
        let tokens = quote::quote! {
            /// Like SQLITE_EXTENSION_INIT2 macro
            pub unsafe fn rusqlite_extension_init2(#p_api: *mut #sqlite3_api_routines_ident) -> ::std::result::Result<(),crate::InitError> {
                #(#malloc)* // sqlite3_malloc needed by to_sqlite_error
                if let Some(fun) = (*#p_api).libversion_number {
                    let version = fun();
                    if SQLITE_VERSION_NUMBER > version {
                        return Err(crate::InitError::VersionMismatch{compile_time: SQLITE_VERSION_NUMBER, runtime: version});
                    }
                } else {
                    return Err(crate::InitError::NullFunctionPointer);
                }
                #(#stores)*
                Ok(())
            }
        };
        output.push_str(&prettyplease::unparse(
            &syn::parse2(tokens).expect("could not parse quote output"),
        ));
        output.push('\n');
    }

    fn extract_method(ty: &syn::Type) -> Option<&syn::TypeBareFn> {
        match ty {
            syn::Type::Path(tp) => tp.path.segments.last(),
            _ => None,
        }
        .map(|seg| match &seg.arguments {
            syn::PathArguments::AngleBracketed(args) => args.args.first(),
            _ => None,
        })?
        .map(|arg| match arg {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        })?
        .map(|ty| match ty {
            syn::Type::BareFn(r) => Some(r),
            _ => None,
        })?
    }
}
