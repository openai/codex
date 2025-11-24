//! A build dependency for Cargo libraries to find libraries in a
//! [Vcpkg](https://github.com/microsoft/vcpkg) tree
//!
//! From a Vcpkg package name
//! this build helper will emit cargo metadata to link it and it's dependencies
//! (excluding system libraries, which it does not determine).
//!
//! The simplest possible usage looks like this :-
//!
//! ```rust,no_run
//! // build.rs
//! vcpkg::find_package("libssh2").unwrap();
//! ```
//!
//! The cargo metadata that is emitted can be changed like this :-
//!
//! ```rust,no_run
//! // build.rs
//! vcpkg::Config::new()
//!     .emit_includes(true)
//!     .find_package("zlib").unwrap();
//! ```
//!
//! If the search was successful all appropriate Cargo metadata will be printed
//! to stdout.
//!
//! # Static vs. dynamic linking
//! ## Linux and Mac
//! At this time, vcpkg has a single triplet on macOS and Linux, which builds
//! static link versions of libraries. This triplet works well with Rust. It is also possible
//! to select a custom triplet using the `VCPKGRS_TRIPLET` environment variable.
//! ## Windows
//! On Windows there are three
//! configurations that are supported for 64-bit builds and another three for 32-bit.
//! The default 64-bit configuration is `x64-windows-static-md` which is a
//! [community supported](https://github.com/microsoft/vcpkg/blob/master/docs/users/triplets.md#community-triplets)
//! configuration that is a good match for Rust - dynamically linking to the C runtime,
//! and statically linking to the packages in vcpkg.
//!
//! Another option is to build a fully static
//! binary using `RUSTFLAGS=-Ctarget-feature=+crt-static`. This will link to libraries built
//! with vcpkg triplet `x64-windows-static`.
//!
//! For dynamic linking, set `VCPKGRS_DYNAMIC=1` in the
//! environment. This will link to libraries built with vcpkg triplet `x64-windows`. If `VCPKGRS_DYNAMIC` is set, `cargo install` will
//! generate dynamically linked binaries, in which case you will have to arrange for
//! dlls from your Vcpkg installation to be available in your path.
//!
//! # Environment variables
//!
//! A number of environment variables are available to globally configure which
//! libraries are selected.
//!
//! * `VCPKG_ROOT` - Set the directory to look in for a vcpkg installation. If
//! it is not set, vcpkg will use the user-wide installation if one has been
//! set up with `vcpkg integrate install`, and check the crate source and target
//! to see if a vcpkg tree has been created by [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg).
//!
//! * `VCPKGRS_TRIPLET` - Use this to override vcpkg-rs' default triplet selection with your own.
//! This is how to select a custom vcpkg triplet.
//!
//! * `VCPKGRS_NO_FOO` - if set, vcpkg-rs will not attempt to find the
//! library named `foo`.
//!
//! * `VCPKGRS_DISABLE` - if set, vcpkg-rs will not attempt to find any libraries.
//!
//! * `VCPKGRS_DYNAMIC` - if set, vcpkg-rs will link to DLL builds of ports.
//! # Related tools
//! ## cargo vcpkg
//! [`cargo vcpkg`](https://crates.io/crates/cargo-vcpkg) can fetch and build a vcpkg installation of
//! required packages from scratch. It merges package requirements specified in the `Cargo.toml` of
//! crates in the dependency tree.  
//! ## vcpkg_cli
//! There is also a rudimentary companion crate, `vcpkg_cli` that allows testing of environment
//! and flag combinations.
//!
//! ```Batchfile
//! C:\src> vcpkg_cli probe -l static mysqlclient
//! Found library mysqlclient
//! Include paths:
//!         C:\src\[..]\vcpkg\installed\x64-windows-static\include
//! Library paths:
//!         C:\src\[..]\vcpkg\installed\x64-windows-static\lib
//! Cargo metadata:
//!         cargo:rustc-link-search=native=C:\src\[..]\vcpkg\installed\x64-windows-static\lib
//!         cargo:rustc-link-lib=static=mysqlclient
//! ```

// The CI will test vcpkg-rs on 1.12 because that is how far back vcpkg-rs 0.2 tries to be
// compatible (was actually 1.10 see #29).  This was originally based on how far back
// rust-openssl's openssl-sys was backward compatible when this crate originally released.
//
// This will likely get bumped by the next major release.
#![allow(deprecated)]
#![allow(warnings)]

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[allow(unused_imports)]
use std::ascii::AsciiExt;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// Configuration options for finding packages, setting up the tree and emitting metadata to cargo
#[derive(Default)]
pub struct Config {
    /// should the cargo metadata actually be emitted
    cargo_metadata: bool,

    /// should cargo:include= metadata be emitted (defaults to false)
    emit_includes: bool,

    /// .lib/.a files that must be be found for probing to be considered successful
    required_libs: Vec<String>,

    /// .dlls that must be be found for probing to be considered successful
    required_dlls: Vec<String>,

    /// should DLLs be copied to OUT_DIR?
    copy_dlls: bool,

    /// override VCPKG_ROOT environment variable
    vcpkg_root: Option<PathBuf>,

    target: Option<TargetTriplet>,
}

/// Details of a package that was found
#[derive(Debug)]
pub struct Library {
    /// Paths for the linker to search for static or import libraries
    pub link_paths: Vec<PathBuf>,

    /// Paths to search at runtme to find DLLs
    pub dll_paths: Vec<PathBuf>,

    /// Paths to include files
    pub include_paths: Vec<PathBuf>,

    /// cargo: metadata lines
    pub cargo_metadata: Vec<String>,

    /// libraries found are static
    pub is_static: bool,

    /// DLLs found
    pub found_dlls: Vec<PathBuf>,

    /// static libs or import libs found
    pub found_libs: Vec<PathBuf>,

    /// link name of libraries found, this is useful to emit linker commands
    pub found_names: Vec<String>,

    /// ports that are providing the libraries to link to, in port link order
    pub ports: Vec<String>,

    /// the vcpkg triplet that has been selected
    pub vcpkg_triplet: String,
}

#[derive(Clone)]
struct TargetTriplet {
    triplet: String,
    is_static: bool,
    lib_suffix: String,
    strip_lib_prefix: bool,
}

impl<S: AsRef<str>> From<S> for TargetTriplet {
    fn from(triplet: S) -> TargetTriplet {
        let triplet = triplet.as_ref();
        if triplet.contains("windows") {
            TargetTriplet {
                triplet: triplet.into(),
                is_static: triplet.contains("-static"),
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            }
        } else {
            TargetTriplet {
                triplet: triplet.into(),
                is_static: true,
                lib_suffix: "a".into(),
                strip_lib_prefix: true,
            }
        }
    }
}

#[derive(Debug)] // need Display?
pub enum Error {
    /// Aborted because of a `VCPKGRS_NO_*` environment variable.
    ///
    /// Contains the name of the responsible environment variable.
    DisabledByEnv(String),

    /// Aborted because a required environment variable was not set.
    RequiredEnvMissing(String),

    /// On Windows, only MSVC ABI is supported
    NotMSVC,

    /// Can't find a vcpkg tree
    VcpkgNotFound(String),

    /// Library not found in vcpkg tree
    LibNotFound(String),

    /// Could not understand vcpkg installation
    VcpkgInstallation(String),

    #[doc(hidden)]
    __Nonexhaustive,
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::DisabledByEnv(_) => "vcpkg-rs requested to be aborted",
            Error::RequiredEnvMissing(_) => "a required env setting is missing",
            Error::NotMSVC => "vcpkg-rs only can only find libraries for MSVC ABI builds",
            Error::VcpkgNotFound(_) => "could not find Vcpkg tree",
            Error::LibNotFound(_) => "could not find library in Vcpkg tree",
            Error::VcpkgInstallation(_) => "could not look up details of packages in vcpkg tree",
            Error::__Nonexhaustive => panic!(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            // Error::Command { ref cause, .. } => Some(cause),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Error::DisabledByEnv(ref name) => write!(f, "Aborted because {} is set", name),
            Error::RequiredEnvMissing(ref name) => write!(f, "Aborted because {} is not set", name),
            Error::NotMSVC => write!(
                f,
                "the vcpkg-rs Vcpkg build helper can only find libraries built for the MSVC ABI."
            ),
            Error::VcpkgNotFound(ref detail) => write!(f, "Could not find Vcpkg tree: {}", detail),
            Error::LibNotFound(ref detail) => {
                write!(f, "Could not find library in Vcpkg tree {}", detail)
            }
            Error::VcpkgInstallation(ref detail) => write!(
                f,
                "Could not look up details of packages in vcpkg tree {}",
                detail
            ),
            Error::__Nonexhaustive => panic!(),
        }
    }
}

/// Deprecated in favor of the find_package function
#[doc(hidden)]
pub fn probe_package(name: &str) -> Result<Library, Error> {
    Config::new().probe(name)
}

/// Find the package `package` in a Vcpkg tree.
///
/// Emits cargo metadata to link to libraries provided by the Vcpkg package/port
/// named, and any (non-system) libraries that they depend on.
///
/// This will select the architecture and linkage based on environment
/// variables and build flags as described in the module docs.
pub fn find_package(package: &str) -> Result<Library, Error> {
    Config::new().find_package(package)
}

/// Find the vcpkg root
#[doc(hidden)]
pub fn find_vcpkg_root(cfg: &Config) -> Result<PathBuf, Error> {
    // prefer the setting from the use if there is one
    if let &Some(ref path) = &cfg.vcpkg_root {
        return Ok(path.clone());
    }

    // otherwise, use the setting from the environment
    if let Some(path) = env::var_os("VCPKG_ROOT") {
        return Ok(PathBuf::from(path));
    }

    // see if there is a per-user vcpkg tree that has been integrated into msbuild
    // using `vcpkg integrate install`
    if let Ok(ref local_app_data) = env::var("LOCALAPPDATA") {
        let vcpkg_user_targets_path = Path::new(local_app_data.as_str())
            .join("vcpkg")
            .join("vcpkg.user.targets");

        if let Ok(file) = File::open(vcpkg_user_targets_path.clone()) {
            let file = BufReader::new(&file);

            for line in file.lines() {
                let line = try!(line.map_err(|_| Error::VcpkgNotFound(format!(
                    "Parsing of {} failed.",
                    vcpkg_user_targets_path.to_string_lossy().to_owned()
                ))));
                let mut split = line.split("Project=\"");
                split.next(); // eat anything before Project="
                if let Some(found) = split.next() {
                    // " is illegal in a Windows pathname
                    if let Some(found) = found.split_terminator('"').next() {
                        let mut vcpkg_root = PathBuf::from(found);
                        if !(vcpkg_root.pop()
                            && vcpkg_root.pop()
                            && vcpkg_root.pop()
                            && vcpkg_root.pop())
                        {
                            return Err(Error::VcpkgNotFound(format!(
                                "Could not find vcpkg root above {}",
                                found
                            )));
                        }
                        return Ok(vcpkg_root);
                    }
                }
            }

            // return Err(Error::VcpkgNotFound(format!(
            //     "Project location not found parsing {}.",
            //     vcpkg_user_targets_path.to_string_lossy().to_owned()
            // )));
        }
    }

    // walk up the directory structure and see if it is there
    if let Some(path) = env::var_os("OUT_DIR") {
        // path.ancestors() is supported from Rust 1.28
        let mut path = PathBuf::from(path);
        while path.pop() {
            let mut try_root = path.clone();
            try_root.push("vcpkg");
            try_root.push(".vcpkg-root");
            if try_root.exists() {
                try_root.pop();

                // this could walk up beyond the target directory and find a vcpkg installation
                // that would not have been found by previous versions of vcpkg-rs, so this
                // checks that the vcpkg tree was created by cargo-vcpkg and ignores it if not.
                let mut cv_cfg = try_root.clone();
                cv_cfg.push("downloads");
                cv_cfg.push("cargo-vcpkg.toml");
                if cv_cfg.exists() {
                    return Ok(try_root);
                }
            }
        }
    }

    Err(Error::VcpkgNotFound(
        "No vcpkg installation found. Set the VCPKG_ROOT environment \
             variable or run 'vcpkg integrate install'"
            .to_string(),
    ))
}

fn validate_vcpkg_root(path: &PathBuf) -> Result<(), Error> {
    let mut vcpkg_root_path = path.clone();
    vcpkg_root_path.push(".vcpkg-root");

    if vcpkg_root_path.exists() {
        Ok(())
    } else {
        Err(Error::VcpkgNotFound(format!(
            "Could not find Vcpkg root at {}",
            vcpkg_root_path.to_string_lossy()
        )))
    }
}

fn find_vcpkg_target(cfg: &Config, target_triplet: &TargetTriplet) -> Result<VcpkgTarget, Error> {
    let vcpkg_root = try!(find_vcpkg_root(&cfg));
    try!(validate_vcpkg_root(&vcpkg_root));

    let mut base = vcpkg_root.clone();
    base.push("installed");
    let status_path = base.join("vcpkg");

    base.push(&target_triplet.triplet);

    let lib_path = base.join("lib");
    let bin_path = base.join("bin");
    let include_path = base.join("include");
    let packages_path = vcpkg_root.join("packages");

    Ok(VcpkgTarget {
        lib_path: lib_path,
        bin_path: bin_path,
        include_path: include_path,
        status_path: status_path,
        packages_path: packages_path,
        target_triplet: target_triplet.clone(),
    })
}

/// Parsed knowledge from a .pc file.
#[derive(Debug)]
struct PcFile {
    /// The pkg-config name of this library.
    id: String,
    /// List of libraries found as '-l', translated to a given vcpkg_target. e.g. libbrotlicommon.a
    libs: Vec<String>,
    /// List of pkgconfig dependencies, e.g. PcFile::id.
    deps: Vec<String>,
}
impl PcFile {
    fn parse_pc_file(vcpkg_target: &VcpkgTarget, path: &Path) -> Result<Self, Error> {
        // Extract the pkg-config name.
        let id = try!(path
            .file_stem()
            .ok_or_else(|| Error::VcpkgInstallation(format!(
                "pkg-config file {} has bogus name",
                path.to_string_lossy()
            ))))
        .to_string_lossy();
        // Read through the file and gather what we want.
        let mut file = try!(File::open(path)
            .map_err(|_| Error::VcpkgInstallation(format!("Couldn't open {}", path.display()))));
        let mut pc_file_contents = String::new();

        try!(file
            .read_to_string(&mut pc_file_contents)
            .map_err(|_| Error::VcpkgInstallation(format!("Couldn't read {}", path.display()))));
        PcFile::from_str(&id, &pc_file_contents, &vcpkg_target.target_triplet)
    }
    fn from_str(id: &str, s: &str, target_triplet: &TargetTriplet) -> Result<Self, Error> {
        let mut libs = Vec::new();
        let mut deps = Vec::new();

        for line in s.lines() {
            // We could collect a lot of stuff here, but we only care about Requires and Libs for the moment.
            if line.starts_with("Requires:") {
                let mut requires_args = line
                    .split(":")
                    .skip(1)
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .flat_map(|e| e.split(","))
                    .filter(|s| *s != "");
                while let Some(dep) = requires_args.next() {
                    // Drop any versioning requirements, we only care about library order and rely upon
                    // port dependencies to resolve versioning.
                    if let Some(_) = dep.find(|c| c == '=' || c == '<' || c == '>') {
                        requires_args.next();
                        continue;
                    }
                    deps.push(dep.to_owned());
                }
            } else if line.starts_with("Libs:") {
                let lib_flags = line
                    .split(":")
                    .skip(1)
                    .next()
                    .unwrap_or("")
                    .split_whitespace();
                for lib_flag in lib_flags {
                    if lib_flag.starts_with("-l") {
                        // reconstruct the library name.
                        let lib = format!(
                            "{}{}.{}",
                            if target_triplet.strip_lib_prefix {
                                "lib"
                            } else {
                                ""
                            },
                            lib_flag.trim_left_matches("-l"),
                            target_triplet.lib_suffix
                        );
                        libs.push(lib);
                    }
                }
            }
        }

        Ok(PcFile {
            id: id.to_string(),
            libs: libs,
            deps: deps,
        })
    }
}

/// Collection of PcFile.  Can be built and queried as a set of .pc files.
#[derive(Debug)]
struct PcFiles {
    files: HashMap<String, PcFile>,
}
impl PcFiles {
    fn load_pkgconfig_dir(vcpkg_target: &VcpkgTarget, path: &PathBuf) -> Result<Self, Error> {
        let mut files = HashMap::new();
        for dir_entry in try!(path.read_dir().map_err(|e| {
            Error::VcpkgInstallation(format!(
                "Missing pkgconfig directory {}: {}",
                path.to_string_lossy(),
                e
            ))
        })) {
            let dir_entry = try!(dir_entry.map_err(|e| {
                Error::VcpkgInstallation(format!(
                    "Troubling reading pkgconfig dir {}: {}",
                    path.to_string_lossy(),
                    e
                ))
            }));
            // Only look at .pc files.
            if dir_entry.path().extension() != Some(OsStr::new("pc")) {
                continue;
            }
            let pc_file = try!(PcFile::parse_pc_file(vcpkg_target, &dir_entry.path()));
            files.insert(pc_file.id.to_owned(), pc_file);
        }
        Ok(PcFiles { files: files })
    }
    /// Use the .pc files as a hint to the library sort order.
    fn fix_ordering(&self, mut libs: Vec<String>) -> Vec<String> {
        // Overall heuristic: for each library given as input, identify which PcFile declared it.
        // Then, looking at that PcFile, check its Requires: (deps), and if the pc file for that
        // dep is in our set, check if its libraries are in our set of libs.  If so, move it to the
        // end to ensure it gets linked afterwards.

        // We may need to do this a few times to properly handle the case where A -> (depends on) B
        // -> C -> D and libraries were originally sorted D, C, B, A.  Avoid recursion so we don't
        // have to detect potential cycles.
        for _iter in 0..3 {
            let mut required_lib_order: Vec<String> = Vec::new();
            for lib in &libs {
                required_lib_order.push(lib.to_owned());
                if let Some(pc_file) = self.locate_pc_file_by_lib(lib) {
                    // Consider its requirements:
                    for dep in &pc_file.deps {
                        // Only consider pkgconfig dependencies we know about.
                        if let Some(dep_pc_file) = self.files.get(dep) {
                            // Intra-port library ordering found, pivot any already seen dep_lib to the
                            // end of the list.
                            for dep_lib in &dep_pc_file.libs {
                                if let Some(removed) = remove_item(&mut required_lib_order, dep_lib)
                                {
                                    required_lib_order.push(removed);
                                }
                            }
                        }
                    }
                }
            }
            // We should always end up with the same number of libraries, only their order should
            // change.
            assert_eq!(libs.len(), required_lib_order.len());
            // Termination:
            if required_lib_order == libs {
                // Nothing changed, we're done here.
                return libs;
            }
            libs = required_lib_order;
        }
        println!("cargo:warning=vcpkg gave up trying to resolve pkg-config ordering.");
        libs
    }
    /// Locate which PcFile contains this library, if any.
    fn locate_pc_file_by_lib(&self, lib: &str) -> Option<&PcFile> {
        for (id, pc_file) in &self.files {
            if pc_file.libs.contains(&lib.to_owned()) {
                return Some(pc_file);
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
struct Port {
    // dlls if any
    dlls: Vec<String>,

    // libs (static or import)
    libs: Vec<String>,

    // ports that this port depends on
    deps: Vec<String>,
}

fn load_port_manifest(
    path: &PathBuf,
    port: &str,
    version: &str,
    vcpkg_target: &VcpkgTarget,
) -> Result<(Vec<String>, Vec<String>), Error> {
    let manifest_file = path.join("info").join(format!(
        "{}_{}_{}.list",
        port, version, vcpkg_target.target_triplet.triplet
    ));

    let mut dlls = Vec::new();
    let mut libs = Vec::new();

    let f = try!(
        File::open(&manifest_file).map_err(|_| Error::VcpkgInstallation(format!(
            "Could not open port manifest file {}",
            manifest_file.display()
        )))
    );

    let file = BufReader::new(&f);

    let dll_prefix = Path::new(&vcpkg_target.target_triplet.triplet).join("bin");
    let lib_prefix = Path::new(&vcpkg_target.target_triplet.triplet).join("lib");

    for line in file.lines() {
        let line = line.unwrap();

        let file_path = Path::new(&line);

        if let Ok(dll) = file_path.strip_prefix(&dll_prefix) {
            if dll.extension() == Some(OsStr::new("dll"))
                && dll.components().collect::<Vec<_>>().len() == 1
            {
                // match "mylib.dll" but not "debug/mylib.dll" or "manual_link/mylib.dll"

                dll.to_str().map(|s| dlls.push(s.to_owned()));
            }
        } else if let Ok(lib) = file_path.strip_prefix(&lib_prefix) {
            if lib.extension() == Some(OsStr::new(&vcpkg_target.target_triplet.lib_suffix))
                && lib.components().collect::<Vec<_>>().len() == 1
            {
                if let Some(lib) = vcpkg_target.link_name_for_lib(lib) {
                    libs.push(lib);
                }
            }
        }
    }

    // Load .pc files for hints about intra-port library ordering.
    let pkg_config_prefix = vcpkg_target
        .packages_path
        .join(format!("{}_{}", port, vcpkg_target.target_triplet.triplet))
        .join("lib")
        .join("pkgconfig");
    // Try loading the pc files, if they are present. Not all ports have pkgconfig.
    if let Ok(pc_files) = PcFiles::load_pkgconfig_dir(vcpkg_target, &pkg_config_prefix) {
        // Use the .pc file data to potentially sort the libs to the correct order.
        libs = pc_files.fix_ordering(libs);
    }

    Ok((dlls, libs))
}

// load ports from the status file or one of the incremental updates
fn load_port_file(
    filename: &PathBuf,
    port_info: &mut Vec<BTreeMap<String, String>>,
) -> Result<(), Error> {
    let f = try!(
        File::open(&filename).map_err(|e| Error::VcpkgInstallation(format!(
            "Could not open status file at {}: {}",
            filename.display(),
            e
        )))
    );
    let file = BufReader::new(&f);
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for line in file.lines() {
        let line = line.unwrap();
        let parts = line.splitn(2, ": ").clone().collect::<Vec<_>>();
        if parts.len() == 2 {
            // a key: value line
            current.insert(parts[0].trim().into(), parts[1].trim().into());
        } else if line.len() == 0 {
            // end of section
            port_info.push(current.clone());
            current.clear();
        } else {
            // ignore all extension lines of the form
            //
            // Description: a package with a
            //   very long description
            //
            // the description key is not used so this is harmless but
            // this will eat extension lines for any multiline key which
            // could become an issue in future
        }
    }

    if !current.is_empty() {
        port_info.push(current);
    }

    Ok(())
}

fn load_ports(target: &VcpkgTarget) -> Result<BTreeMap<String, Port>, Error> {
    let mut ports: BTreeMap<String, Port> = BTreeMap::new();

    let mut port_info: Vec<BTreeMap<String, String>> = Vec::new();

    // load the main status file. It is not an error if this file does not
    // exist. If the only command that has been run in a Vcpkg installation
    // is a single `vcpkg install package` then there will likely be no
    // status file, only incremental updates. This is the typical case when
    // running in a CI environment.
    let status_filename = target.status_path.join("status");
    load_port_file(&status_filename, &mut port_info).ok();

    // load updates to the status file that have yet to be normalized
    let status_update_dir = target.status_path.join("updates");

    let paths = try!(
        fs::read_dir(status_update_dir).map_err(|e| Error::VcpkgInstallation(format!(
            "could not read status file updates dir: {}",
            e
        )))
    );

    // get all of the paths of the update files into a Vec<PathBuf>
    let mut paths = try!(paths
        .map(|rde| rde.map(|de| de.path())) // Result<DirEntry, io::Error> -> Result<PathBuf, io::Error>
        .collect::<Result<Vec<_>, _>>() // collect into Result<Vec<PathBuf>, io::Error>
        .map_err(|e| {
            Error::VcpkgInstallation(format!(
                "could not read status file update filenames: {}",
                e
            ))
        }));

    // Sort the paths and read them. This could be done directly from the iterator if
    // read_dir() guarantees that the files will be read in alpha order but that appears
    // to be unspecified as the underlying operating system calls used are unspecified
    // https://doc.rust-lang.org/nightly/std/fs/fn.read_dir.html#platform-specific-behavior
    paths.sort();
    for path in paths {
        //       println!("Name: {}", path.display());
        try!(load_port_file(&path, &mut port_info));
    }
    //println!("{:#?}", port_info);

    let mut seen_names = BTreeMap::new();
    for current in &port_info {
        // store them by name and arch, clobbering older details
        match (
            current.get("Package"),
            current.get("Architecture"),
            current.get("Feature"),
        ) {
            (Some(pkg), Some(arch), feature) => {
                seen_names.insert((pkg, arch, feature), current);
            }
            _ => {}
        }
    }

    for (&(name, arch, feature), current) in &seen_names {
        if **arch == target.target_triplet.triplet {
            let mut deps = if let Some(deps) = current.get("Depends") {
                deps.split(", ").map(|x| x.to_owned()).collect()
            } else {
                Vec::new()
            };

            if current
                .get("Status")
                .unwrap_or(&String::new())
                .ends_with(" installed")
            {
                match (current.get("Version"), feature) {
                    (Some(version), _) => {
                        // this failing here and bailing out causes everything to fail
                        let lib_info = try!(load_port_manifest(
                            &target.status_path,
                            &name,
                            version,
                            &target
                        ));
                        let port = Port {
                            dlls: lib_info.0,
                            libs: lib_info.1,
                            deps: deps,
                        };

                        ports.insert(name.to_string(), port);
                    }
                    (_, Some(_feature)) => match ports.get_mut(name) {
                        Some(ref mut port) => {
                            port.deps.append(&mut deps);
                        }
                        _ => {
                            println!("found a feature that had no corresponding port :-");
                            println!("current {:+?}", current);
                            continue;
                        }
                    },
                    (_, _) => {
                        println!("didn't know how to deal with status file entry :-");
                        println!("{:+?}", current);
                        continue;
                    }
                }
            }
        }
    }

    Ok(ports)
}

/// paths and triple for the chosen target
struct VcpkgTarget {
    lib_path: PathBuf,
    bin_path: PathBuf,
    include_path: PathBuf,

    // directory containing the status file
    status_path: PathBuf,
    // directory containing the install files per port.
    packages_path: PathBuf,

    // target-specific settings.
    target_triplet: TargetTriplet,
}

impl VcpkgTarget {
    fn link_name_for_lib(&self, filename: &std::path::Path) -> Option<String> {
        if self.target_triplet.strip_lib_prefix {
            filename.to_str().map(|s| s.to_owned())
        // filename
        //     .to_str()
        //     .map(|s| s.trim_left_matches("lib").to_owned())
        } else {
            filename.to_str().map(|s| s.to_owned())
        }
    }
}

impl Config {
    pub fn new() -> Config {
        Config {
            cargo_metadata: true,
            copy_dlls: true,
            ..Default::default()
        }
    }

    fn get_target_triplet(&mut self) -> Result<TargetTriplet, Error> {
        if self.target.is_none() {
            let target = if let Ok(triplet_str) = env::var("VCPKGRS_TRIPLET") {
                triplet_str.into()
            } else {
                try!(msvc_target())
            };
            self.target = Some(target);
        }

        Ok(self.target.as_ref().unwrap().clone())
    }

    /// Find the package `port_name` in a Vcpkg tree.
    ///
    /// Emits cargo metadata to link to libraries provided by the Vcpkg package/port
    /// named, and any (non-system) libraries that they depend on.
    ///
    /// This will select the architecture and linkage based on environment
    /// variables and build flags as described in the module docs, and any configuration
    /// set on the builder.
    pub fn find_package(&mut self, port_name: &str) -> Result<Library, Error> {
        // determine the target type, bailing out if it is not some
        // kind of msvc
        let msvc_target = try!(self.get_target_triplet());

        // bail out if requested to not try at all
        if env::var_os("VCPKGRS_DISABLE").is_some() {
            return Err(Error::DisabledByEnv("VCPKGRS_DISABLE".to_owned()));
        }

        // bail out if requested to not try at all (old)
        if env::var_os("NO_VCPKG").is_some() {
            return Err(Error::DisabledByEnv("NO_VCPKG".to_owned()));
        }

        // bail out if requested to skip this package
        let abort_var_name = format!("VCPKGRS_NO_{}", envify(port_name));
        if env::var_os(&abort_var_name).is_some() {
            return Err(Error::DisabledByEnv(abort_var_name));
        }

        // bail out if requested to skip this package (old)
        let abort_var_name = format!("{}_NO_VCPKG", envify(port_name));
        if env::var_os(&abort_var_name).is_some() {
            return Err(Error::DisabledByEnv(abort_var_name));
        }

        let vcpkg_target = try!(find_vcpkg_target(&self, &msvc_target));
        let mut required_port_order = Vec::new();

        // if no overrides have been selected, then the Vcpkg port name
        // is the the .lib name and the .dll name
        if self.required_libs.is_empty() {
            let ports = try!(load_ports(&vcpkg_target));

            if ports.get(&port_name.to_owned()).is_none() {
                return Err(Error::LibNotFound(format!(
                    "package {} is not installed for vcpkg triplet {}",
                    port_name.to_owned(),
                    vcpkg_target.target_triplet.triplet
                )));
            }

            // the complete set of ports required
            let mut required_ports: BTreeMap<String, Port> = BTreeMap::new();
            // working of ports that we need to include
            //        let mut ports_to_scan: BTreeSet<String> = BTreeSet::new();
            //        ports_to_scan.insert(port_name.to_owned());
            let mut ports_to_scan = vec![port_name.to_owned()]; //: Vec<String> = BTreeSet::new();

            while !ports_to_scan.is_empty() {
                let port_name = ports_to_scan.pop().unwrap();

                if required_ports.contains_key(&port_name) {
                    continue;
                }

                if let Some(port) = ports.get(&port_name) {
                    for dep in &port.deps {
                        ports_to_scan.push(dep.clone());
                    }
                    required_ports.insert(port_name.clone(), (*port).clone());
                    remove_item(&mut required_port_order, &port_name);
                    required_port_order.push(port_name);
                } else {
                    // what?
                }
            }

            // for port in ports {
            //     println!("port {:?}", port);
            // }
            // println!("== Looking for port {}", port_name);
            // for port in &required_port_order {
            //     println!("ordered required port {:?}", port);
            // }
            // println!("=============================");
            // for port in &required_ports {
            //     println!("required port {:?}", port);
            // }

            // if no overrides have been selected, then the Vcpkg port name
            // is the the .lib name and the .dll name
            if self.required_libs.is_empty() {
                for port_name in &required_port_order {
                    let port = required_ports.get(port_name).unwrap();
                    self.required_libs.extend(port.libs.iter().map(|s| {
                        Path::new(&s)
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    }));
                    self.required_dlls
                        .extend(port.dlls.iter().cloned().map(|s| {
                            Path::new(&s)
                                .file_stem()
                                .unwrap()
                                .to_string_lossy()
                                .into_owned()
                        }));
                }
            }
        }
        // require explicit opt-in before using dynamically linked
        // variants, otherwise cargo install of various things will
        // stop working if Vcpkg is installed.
        if !vcpkg_target.target_triplet.is_static && !env::var_os("VCPKGRS_DYNAMIC").is_some() {
            return Err(Error::RequiredEnvMissing("VCPKGRS_DYNAMIC".to_owned()));
        }

        let mut lib = Library::new(
            vcpkg_target.target_triplet.is_static,
            &vcpkg_target.target_triplet.triplet,
        );

        if self.emit_includes {
            lib.cargo_metadata.push(format!(
                "cargo:include={}",
                vcpkg_target.include_path.display()
            ));
        }
        lib.include_paths.push(vcpkg_target.include_path.clone());

        lib.cargo_metadata.push(format!(
            "cargo:rustc-link-search=native={}",
            vcpkg_target
                .lib_path
                .to_str()
                .expect("failed to convert string type")
        ));
        lib.link_paths.push(vcpkg_target.lib_path.clone());
        if !vcpkg_target.target_triplet.is_static {
            lib.cargo_metadata.push(format!(
                "cargo:rustc-link-search=native={}",
                vcpkg_target
                    .bin_path
                    .to_str()
                    .expect("failed to convert string type")
            ));
            // this path is dropped by recent versions of cargo hence the copies to OUT_DIR below
            lib.dll_paths.push(vcpkg_target.bin_path.clone());
        }

        lib.ports = required_port_order;

        try!(self.emit_libs(&mut lib, &vcpkg_target));

        if self.copy_dlls {
            try!(self.do_dll_copy(&mut lib));
        }

        if self.cargo_metadata {
            for line in &lib.cargo_metadata {
                println!("{}", line);
            }
        }
        Ok(lib)
    }

    /// Define whether metadata should be emitted for cargo allowing it to
    /// automatically link the binary. Defaults to `true`.
    pub fn cargo_metadata(&mut self, cargo_metadata: bool) -> &mut Config {
        self.cargo_metadata = cargo_metadata;
        self
    }

    /// Define cargo:include= metadata should be emitted. Defaults to `false`.
    pub fn emit_includes(&mut self, emit_includes: bool) -> &mut Config {
        self.emit_includes = emit_includes;
        self
    }

    /// Should DLLs be copied to OUT_DIR?
    /// Defaults to `true`.
    pub fn copy_dlls(&mut self, copy_dlls: bool) -> &mut Config {
        self.copy_dlls = copy_dlls;
        self
    }

    /// Define which path to use as vcpkg root overriding the VCPKG_ROOT environment variable
    /// Default to `None`, which means use VCPKG_ROOT or try to find out automatically
    pub fn vcpkg_root(&mut self, vcpkg_root: PathBuf) -> &mut Config {
        self.vcpkg_root = Some(vcpkg_root);
        self
    }

    /// Specify target triplet. When triplet is not specified, inferred triplet from rust target is used.
    ///
    /// Specifying a triplet using `target_triplet` will override the default triplet for this crate. This
    /// cannot change the choice of triplet made by other crates, so a safer choice will be to set
    /// `VCPKGRS_TRIPLET` in the environment which will allow all crates to use a consistent set of
    /// external dependencies.
    pub fn target_triplet<S: AsRef<str>>(&mut self, triplet: S) -> &mut Config {
        self.target = Some(triplet.into());
        self
    }

    /// Find the library `port_name` in a Vcpkg tree.
    ///
    /// This will use all configuration previously set to select the
    /// architecture and linkage.
    /// Deprecated in favor of the find_package function
    #[doc(hidden)]
    pub fn probe(&mut self, port_name: &str) -> Result<Library, Error> {
        // determine the target type, bailing out if it is not some
        // kind of msvc
        let msvc_target = try!(self.get_target_triplet());

        // bail out if requested to not try at all
        if env::var_os("VCPKGRS_DISABLE").is_some() {
            return Err(Error::DisabledByEnv("VCPKGRS_DISABLE".to_owned()));
        }

        // bail out if requested to not try at all (old)
        if env::var_os("NO_VCPKG").is_some() {
            return Err(Error::DisabledByEnv("NO_VCPKG".to_owned()));
        }

        // bail out if requested to skip this package
        let abort_var_name = format!("VCPKGRS_NO_{}", envify(port_name));
        if env::var_os(&abort_var_name).is_some() {
            return Err(Error::DisabledByEnv(abort_var_name));
        }

        // bail out if requested to skip this package (old)
        let abort_var_name = format!("{}_NO_VCPKG", envify(port_name));
        if env::var_os(&abort_var_name).is_some() {
            return Err(Error::DisabledByEnv(abort_var_name));
        }

        // if no overrides have been selected, then the Vcpkg port name
        // is the the .lib name and the .dll name
        if self.required_libs.is_empty() {
            self.required_libs.push(port_name.to_owned());
            self.required_dlls.push(port_name.to_owned());
        }

        let vcpkg_target = try!(find_vcpkg_target(&self, &msvc_target));

        // require explicit opt-in before using dynamically linked
        // variants, otherwise cargo install of various things will
        // stop working if Vcpkg is installed.
        if !vcpkg_target.target_triplet.is_static && !env::var_os("VCPKGRS_DYNAMIC").is_some() {
            return Err(Error::RequiredEnvMissing("VCPKGRS_DYNAMIC".to_owned()));
        }

        let mut lib = Library::new(
            vcpkg_target.target_triplet.is_static,
            &vcpkg_target.target_triplet.triplet,
        );

        if self.emit_includes {
            lib.cargo_metadata.push(format!(
                "cargo:include={}",
                vcpkg_target.include_path.display()
            ));
        }
        lib.include_paths.push(vcpkg_target.include_path.clone());

        lib.cargo_metadata.push(format!(
            "cargo:rustc-link-search=native={}",
            vcpkg_target
                .lib_path
                .to_str()
                .expect("failed to convert string type")
        ));
        lib.link_paths.push(vcpkg_target.lib_path.clone());
        if !vcpkg_target.target_triplet.is_static {
            lib.cargo_metadata.push(format!(
                "cargo:rustc-link-search=native={}",
                vcpkg_target
                    .bin_path
                    .to_str()
                    .expect("failed to convert string type")
            ));
            // this path is dropped by recent versions of cargo hence the copies to OUT_DIR below
            lib.dll_paths.push(vcpkg_target.bin_path.clone());
        }

        try!(self.emit_libs(&mut lib, &vcpkg_target));

        if self.copy_dlls {
            try!(self.do_dll_copy(&mut lib));
        }

        if self.cargo_metadata {
            for line in &lib.cargo_metadata {
                println!("{}", line);
            }
        }
        Ok(lib)
    }

    fn emit_libs(&mut self, lib: &mut Library, vcpkg_target: &VcpkgTarget) -> Result<(), Error> {
        for required_lib in &self.required_libs {
            // this could use static-nobundle= for static libraries but it is apparently
            // not necessary to make the distinction for windows-msvc.

            let link_name = match vcpkg_target.target_triplet.strip_lib_prefix {
                true => required_lib.trim_left_matches("lib"),
                false => required_lib,
            };

            lib.cargo_metadata
                .push(format!("cargo:rustc-link-lib={}", link_name));

            lib.found_names.push(String::from(link_name));

            // verify that the library exists
            let mut lib_location = vcpkg_target.lib_path.clone();
            lib_location.push(required_lib.clone() + "." + &vcpkg_target.target_triplet.lib_suffix);

            if !lib_location.exists() {
                return Err(Error::LibNotFound(lib_location.display().to_string()));
            }
            lib.found_libs.push(lib_location);
        }

        if !vcpkg_target.target_triplet.is_static {
            for required_dll in &self.required_dlls {
                let mut dll_location = vcpkg_target.bin_path.clone();
                dll_location.push(required_dll.clone() + ".dll");

                // verify that the DLL exists
                if !dll_location.exists() {
                    return Err(Error::LibNotFound(dll_location.display().to_string()));
                }
                lib.found_dlls.push(dll_location);
            }
        }

        Ok(())
    }

    fn do_dll_copy(&mut self, lib: &mut Library) -> Result<(), Error> {
        if let Some(target_dir) = env::var_os("OUT_DIR") {
            if !lib.found_dlls.is_empty() {
                for file in &lib.found_dlls {
                    let mut dest_path = Path::new(target_dir.as_os_str()).to_path_buf();
                    dest_path.push(Path::new(file.file_name().unwrap()));
                    try!(
                        fs::copy(file, &dest_path).map_err(|_| Error::LibNotFound(format!(
                            "Can't copy file {} to {}",
                            file.to_string_lossy(),
                            dest_path.to_string_lossy()
                        )))
                    );
                    println!(
                        "vcpkg build helper copied {} to {}",
                        file.to_string_lossy(),
                        dest_path.to_string_lossy()
                    );
                }
                lib.cargo_metadata.push(format!(
                    "cargo:rustc-link-search=native={}",
                    env::var("OUT_DIR").unwrap()
                ));
                // work around https://github.com/rust-lang/cargo/issues/3957
                lib.cargo_metadata.push(format!(
                    "cargo:rustc-link-search={}",
                    env::var("OUT_DIR").unwrap()
                ));
            }
        } else {
            return Err(Error::LibNotFound("Unable to get OUT_DIR".to_owned()));
        }
        Ok(())
    }

    /// Override the name of the library to look for if it differs from the package name.
    ///
    /// It should not be necessary to use `lib_name` anymore. Calling `find_package` with a package name
    /// will result in the correct library names.
    /// This may be called more than once if multiple libs are required.
    /// All libs must be found for the probe to succeed. `.probe()` must
    /// be run with a different configuration to look for libraries under one of several names.
    /// `.libname("ssleay32")` will look for ssleay32.lib and also ssleay32.dll if
    /// dynamic linking is selected.
    pub fn lib_name(&mut self, lib_stem: &str) -> &mut Config {
        self.required_libs.push(lib_stem.to_owned());
        self.required_dlls.push(lib_stem.to_owned());
        self
    }

    /// Override the name of the library to look for if it differs from the package name.
    ///
    /// It should not be necessary to use `lib_names` anymore. Calling `find_package` with a package name
    /// will result in the correct library names.
    /// This may be called more than once if multiple libs are required.
    /// All libs must be found for the probe to succeed. `.probe()` must
    /// be run with a different configuration to look for libraries under one of several names.
    /// `.lib_names("libcurl_imp","curl")` will look for libcurl_imp.lib and also curl.dll if
    /// dynamic linking is selected.
    pub fn lib_names(&mut self, lib_stem: &str, dll_stem: &str) -> &mut Config {
        self.required_libs.push(lib_stem.to_owned());
        self.required_dlls.push(dll_stem.to_owned());
        self
    }
}

fn remove_item(cont: &mut Vec<String>, item: &String) -> Option<String> {
    match cont.iter().position(|x| *x == *item) {
        Some(pos) => Some(cont.remove(pos)),
        None => None,
    }
}

impl Library {
    fn new(is_static: bool, vcpkg_triplet: &str) -> Library {
        Library {
            link_paths: Vec::new(),
            dll_paths: Vec::new(),
            include_paths: Vec::new(),
            cargo_metadata: Vec::new(),
            is_static: is_static,
            found_dlls: Vec::new(),
            found_libs: Vec::new(),
            found_names: Vec::new(),
            ports: Vec::new(),
            vcpkg_triplet: vcpkg_triplet.to_string(),
        }
    }
}

fn envify(name: &str) -> String {
    name.chars()
        .map(|c| c.to_ascii_uppercase())
        .map(|c| if c == '-' { '_' } else { c })
        .collect()
}

fn msvc_target() -> Result<TargetTriplet, Error> {
    let is_definitely_dynamic = env::var("VCPKGRS_DYNAMIC").is_ok();
    let target = env::var("TARGET").unwrap_or(String::new());
    let is_static = env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or(String::new()) // rustc 1.10
        .contains("crt-static");
    if target == "x86_64-apple-darwin" {
        Ok(TargetTriplet {
            triplet: "x64-osx".into(),
            is_static: true,
            lib_suffix: "a".into(),
            strip_lib_prefix: true,
        })
    } else if target == "aarch64-apple-darwin" {
        Ok(TargetTriplet {
            triplet: "arm64-osx".into(),
            is_static: true,
            lib_suffix: "a".into(),
            strip_lib_prefix: true,
        })
    } else if target == "x86_64-unknown-linux-gnu" {
        Ok(TargetTriplet {
            triplet: "x64-linux".into(),
            is_static: true,
            lib_suffix: "a".into(),
            strip_lib_prefix: true,
        })
    } else if target == "aarch64-apple-ios" {
        Ok(TargetTriplet {
            triplet: "arm64-ios".into(),
            is_static: true,
            lib_suffix: "a".into(),
            strip_lib_prefix: true,
        })
    } else if !target.contains("-pc-windows-msvc") {
        Err(Error::NotMSVC)
    } else if target.starts_with("x86_64-") {
        if is_static {
            Ok(TargetTriplet {
                triplet: "x64-windows-static".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else if is_definitely_dynamic {
            Ok(TargetTriplet {
                triplet: "x64-windows".into(),
                is_static: false,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else {
            Ok(TargetTriplet {
                triplet: "x64-windows-static-md".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        }
    } else if target.starts_with("aarch64") {
        if is_static {
            Ok(TargetTriplet {
                triplet: "arm64-windows-static".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else if is_definitely_dynamic {
            Ok(TargetTriplet {
                triplet: "arm64-windows".into(),
                is_static: false,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else {
            Ok(TargetTriplet {
                triplet: "arm64-windows-static-md".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        }
    } else {
        // everything else is x86
        if is_static {
            Ok(TargetTriplet {
                triplet: "x86-windows-static".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else if is_definitely_dynamic {
            Ok(TargetTriplet {
                triplet: "x86-windows".into(),
                is_static: false,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        } else {
            Ok(TargetTriplet {
                triplet: "x86-windows-static-md".into(),
                is_static: true,
                lib_suffix: "lib".into(),
                strip_lib_prefix: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {

    extern crate tempdir;

    use super::*;
    use std::env;
    use std::sync::Mutex;

    lazy_static! {
        static ref LOCK: Mutex<()> = Mutex::new(());
    }

    #[test]
    fn do_nothing_for_unsupported_target() {
        let _g = LOCK.lock();
        env::set_var("VCPKG_ROOT", "/");
        env::set_var("TARGET", "x86_64-pc-windows-gnu");
        assert!(match ::probe_package("foo") {
            Err(Error::NotMSVC) => true,
            _ => false,
        });

        env::set_var("TARGET", "x86_64-pc-windows-gnu");
        assert_eq!(env::var("TARGET"), Ok("x86_64-pc-windows-gnu".to_string()));
        assert!(match ::probe_package("foo") {
            Err(Error::NotMSVC) => true,
            _ => false,
        });
        env::remove_var("TARGET");
        env::remove_var("VCPKG_ROOT");
    }

    #[test]
    fn do_nothing_for_bailout_variables_set() {
        let _g = LOCK.lock();
        env::set_var("VCPKG_ROOT", "/");
        env::set_var("TARGET", "x86_64-pc-windows-msvc");

        for &var in &[
            "VCPKGRS_DISABLE",
            "VCPKGRS_NO_FOO",
            "FOO_NO_VCPKG",
            "NO_VCPKG",
        ] {
            env::set_var(var, "1");
            assert!(match ::probe_package("foo") {
                Err(Error::DisabledByEnv(ref v)) if v == var => true,
                _ => false,
            });
            env::remove_var(var);
        }
        env::remove_var("TARGET");
        env::remove_var("VCPKG_ROOT");
    }

    // these tests are good but are leaning on a real vcpkg installation

    // #[test]
    // fn default_build_refuses_dynamic() {
    //     let _g = LOCK.lock();
    //     clean_env();
    //     env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("no-status"));
    //     env::set_var("TARGET", "x86_64-pc-windows-msvc");
    //     println!("Result is {:?}", ::find_package("libmysql"));
    //     assert!(match ::find_package("libmysql") {
    //         Err(Error::RequiredEnvMissing(ref v)) if v == "VCPKGRS_DYNAMIC" => true,
    //         _ => false,
    //     });
    //     clean_env();
    // }

    #[test]
    fn static_build_finds_lib() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "x86_64-pc-windows-msvc");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        // CARGO_CFG_TARGET_FEATURE is set in response to
        // RUSTFLAGS=-Ctarget-feature=+crt-static. It would
        //  be nice to test that also.
        env::set_var("CARGO_CFG_TARGET_FEATURE", "crt-static");
        println!("Result is {:?}", ::find_package("libmysql"));
        assert!(match ::find_package("libmysql") {
            Ok(_) => true,
            _ => false,
        });
        clean_env();
    }

    #[test]
    fn dynamic_build_finds_lib() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("no-status"));
        env::set_var("TARGET", "x86_64-pc-windows-msvc");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        println!("Result is {:?}", ::find_package("libmysql"));
        assert!(match ::find_package("libmysql") {
            Ok(_) => true,
            _ => false,
        });
        clean_env();
    }

    #[test]
    fn handle_multiline_description() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("multiline-description"));
        env::set_var("TARGET", "i686-pc-windows-msvc");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        println!("Result is {:?}", ::find_package("graphite2"));
        assert!(match ::find_package("graphite2") {
            Ok(_) => true,
            _ => false,
        });
        clean_env();
    }

    #[test]
    fn link_libs_required_by_optional_features() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "i686-pc-windows-msvc");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        println!("Result is {:?}", ::find_package("harfbuzz"));
        assert!(match ::find_package("harfbuzz") {
            Ok(lib) => lib
                .cargo_metadata
                .iter()
                .find(|&x| x == "cargo:rustc-link-lib=icuuc")
                .is_some(),
            _ => false,
        });
        clean_env();
    }

    #[test]
    fn link_lib_name_is_correct() {
        let _g = LOCK.lock();

        for target in &[
            "x86_64-apple-darwin",
            "i686-pc-windows-msvc",
            //      "x86_64-pc-windows-msvc",
            //    "x86_64-unknown-linux-gnu",
        ] {
            clean_env();
            env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
            env::set_var("TARGET", target);
            env::set_var("VCPKGRS_DYNAMIC", "1");
            let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
            env::set_var("OUT_DIR", tmp_dir.path());

            println!("Result is {:?}", ::find_package("harfbuzz"));
            assert!(match ::find_package("harfbuzz") {
                Ok(lib) => lib
                    .cargo_metadata
                    .iter()
                    .find(|&x| x == "cargo:rustc-link-lib=harfbuzz")
                    .is_some(),
                _ => false,
            });
            clean_env();
        }
    }

    #[test]
    fn link_dependencies_after_port() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "i686-pc-windows-msvc");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        let lib = ::find_package("harfbuzz").unwrap();

        check_before(&lib, "freetype", "zlib");
        check_before(&lib, "freetype", "bzip2");
        check_before(&lib, "freetype", "libpng");
        check_before(&lib, "harfbuzz", "freetype");
        check_before(&lib, "harfbuzz", "ragel");
        check_before(&lib, "libpng", "zlib");

        clean_env();

        fn check_before(lib: &Library, earlier: &str, later: &str) {
            match (
                lib.ports.iter().position(|x| *x == *earlier),
                lib.ports.iter().position(|x| *x == *later),
            ) {
                (Some(earlier_pos), Some(later_pos)) if earlier_pos < later_pos => {
                    // ok
                }
                _ => {
                    println!(
                        "earlier: {}, later: {}\nLibrary found: {:#?}",
                        earlier, later, lib
                    );
                    panic!();
                }
            }
        }
    }

    #[test]
    fn custom_target_triplet_in_config() {
        let _g = LOCK.lock();

        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "aarch64-apple-ios");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        let harfbuzz = ::Config::new()
            // For the sake of testing, force this build to try to
            // link to the arm64-osx libraries in preference to the
            // default of arm64-ios.
            .target_triplet("x64-osx")
            .find_package("harfbuzz");
        println!("Result with specifying target triplet is {:?}", &harfbuzz);
        let harfbuzz = harfbuzz.unwrap();
        assert_eq!(harfbuzz.vcpkg_triplet, "x64-osx");
        clean_env();
    }

    #[test]
    fn custom_target_triplet_by_env_no_default() {
        let _g = LOCK.lock();

        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "aarch64-apple-doesnotexist");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        let harfbuzz = ::find_package("harfbuzz");
        println!("Result with inference is {:?}", &harfbuzz);
        assert!(harfbuzz.is_err());

        env::set_var("VCPKGRS_TRIPLET", "x64-osx");
        let harfbuzz = ::find_package("harfbuzz").unwrap();
        println!("Result with setting VCPKGRS_TRIPLET is {:?}", &harfbuzz);
        assert_eq!(harfbuzz.vcpkg_triplet, "x64-osx");
        clean_env();
    }

    #[test]
    fn custom_target_triplet_by_env_with_default() {
        let _g = LOCK.lock();

        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "aarch64-apple-ios");
        env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        let harfbuzz = ::find_package("harfbuzz").unwrap();
        println!("Result with inference is {:?}", &harfbuzz);
        assert_eq!(harfbuzz.vcpkg_triplet, "arm64-ios");

        env::set_var("VCPKGRS_TRIPLET", "x64-osx");
        let harfbuzz = ::find_package("harfbuzz").unwrap();
        println!("Result with setting VCPKGRS_TRIPLET is {:?}", &harfbuzz);
        assert_eq!(harfbuzz.vcpkg_triplet, "x64-osx");
        clean_env();
    }

    // #[test]
    // fn dynamic_build_package_specific_bailout() {
    //     clean_env();
    //     env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("no-status"));
    //     env::set_var("TARGET", "x86_64-pc-windows-msvc");
    //     env::set_var("VCPKGRS_DYNAMIC", "1");
    //     env::set_var("VCPKGRS_NO_LIBMYSQL", "1");

    //     println!("Result is {:?}", ::find_package("libmysql"));
    //     assert!(match ::find_package("libmysql") {
    //         Err(Error::DisabledByEnv(ref v)) if v == "VCPKGRS_NO_LIBMYSQL" => true,
    //         _ => false,
    //     });
    //     clean_env();
    // }

    // #[test]
    // fn dynamic_build_global_bailout() {
    //     clean_env();
    //     env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("no-status"));
    //     env::set_var("TARGET", "x86_64-pc-windows-msvc");
    //     env::set_var("VCPKGRS_DYNAMIC", "1");
    //     env::set_var("VCPKGRS_DISABLE", "1");

    //     println!("Result is {:?}", ::find_package("libmysql"));
    //     assert!(match ::find_package("libmysql") {
    //         Err(Error::DisabledByEnv(ref v)) if v == "VCPKGRS_DISABLE" => true,
    //         _ => false,
    //     });
    //     clean_env();
    // }

    #[test]
    fn pc_files_reordering() {
        let _g = LOCK.lock();
        clean_env();
        env::set_var("VCPKG_ROOT", vcpkg_test_tree_loc("normalized"));
        env::set_var("TARGET", "x86_64-unknown-linux-gnu");
        // env::set_var("VCPKGRS_DYNAMIC", "1");
        let tmp_dir = tempdir::TempDir::new("vcpkg_tests").unwrap();
        env::set_var("OUT_DIR", tmp_dir.path());

        let target_triplet = msvc_target().unwrap();

        // The brotli use-case.
        {
            let mut pc_files = PcFiles {
                files: HashMap::new(),
            };
            pc_files.files.insert(
                "libbrotlicommon".to_owned(),
                PcFile::from_str(
                    "libbrotlicommon",
                    "Libs: -lbrotlicommon-static\nRequires:",
                    &target_triplet,
                )
                .unwrap(),
            );
            pc_files.files.insert(
                "libbrotlienc".to_owned(),
                PcFile::from_str(
                    "libbrotlienc",
                    "Libs: -lbrotlienc-static\nRequires: libbrotlicommon",
                    &target_triplet,
                )
                .unwrap(),
            );
            pc_files.files.insert(
                "libbrotlidec".to_owned(),
                PcFile::from_str(
                    "brotlidec",
                    "Libs: -lbrotlidec-static\nRequires: libbrotlicommon >= 1.0.9",
                    &target_triplet,
                )
                .unwrap(),
            );
            // Note that the input is alphabetically sorted.
            let input_libs = vec![
                "libbrotlicommon-static.a".to_owned(),
                "libbrotlidec-static.a".to_owned(),
                "libbrotlienc-static.a".to_owned(),
            ];
            let output_libs = pc_files.fix_ordering(input_libs);
            assert_eq!(output_libs[0], "libbrotlidec-static.a");
            assert_eq!(output_libs[1], "libbrotlienc-static.a");
            assert_eq!(output_libs[2], "libbrotlicommon-static.a");
        }

        // Concoct elaborate dependency graph, try all variations of input sort.
        // Throw some (ignored) version dependencies as well as extra libs not represented in the
        // pc_files dataset.
        {
            let mut pc_files = PcFiles {
                files: HashMap::new(),
            };
            pc_files.files.insert(
                "libA".to_owned(),
                PcFile::from_str(
                    "libA",
                    "Libs: -lA\n\
                     Requires:",
                    &target_triplet,
                )
                .unwrap(),
            );
            pc_files.files.insert(
                "libB".to_owned(),
                PcFile::from_str(
                    "libB",
                    "Libs:  -lB -lm -pthread \n\
                     Requires: libA",
                    &target_triplet,
                )
                .unwrap(),
            );
            pc_files.files.insert(
                "libC".to_owned(),
                PcFile::from_str(
                    "libC",
                    "Libs: -lC -L${libdir}\n\
                     Requires: libB <=1.0 , libmysql-client = 0.9, ",
                    &target_triplet,
                )
                .unwrap(),
            );
            pc_files.files.insert(
                "libD".to_owned(),
                PcFile::from_str(
                    "libD",
                    "Libs: -Lpath/to/libs -Rplugins -lD\n\
                     Requires: libpostgres libC",
                    &target_triplet,
                )
                .unwrap(),
            );
            let permutations: Vec<Vec<&str>> = vec![
                vec!["libA.a", "libB.a", "libC.a", "libD.a"],
                vec!["libA.a", "libB.a", "libD.a", "libC.a"],
                vec!["libA.a", "libC.a", "libB.a", "libD.a"],
                vec!["libA.a", "libC.a", "libD.a", "libB.a"],
                vec!["libA.a", "libD.a", "libB.a", "libC.a"],
                vec!["libA.a", "libD.a", "libC.a", "libB.a"],
                //
                vec!["libB.a", "libA.a", "libC.a", "libD.a"],
                vec!["libB.a", "libA.a", "libD.a", "libC.a"],
                vec!["libB.a", "libC.a", "libA.a", "libD.a"],
                vec!["libB.a", "libC.a", "libD.a", "libA.a"],
                vec!["libB.a", "libD.a", "libA.a", "libC.a"],
                vec!["libB.a", "libD.a", "libC.a", "libA.a"],
                //
                vec!["libC.a", "libA.a", "libB.a", "libD.a"],
                vec!["libC.a", "libA.a", "libD.a", "libB.a"],
                vec!["libC.a", "libB.a", "libA.a", "libD.a"],
                vec!["libC.a", "libB.a", "libD.a", "libA.a"],
                vec!["libC.a", "libD.a", "libA.a", "libB.a"],
                vec!["libC.a", "libD.a", "libB.a", "libA.a"],
                //
                vec!["libD.a", "libA.a", "libB.a", "libC.a"],
                vec!["libD.a", "libA.a", "libC.a", "libB.a"],
                vec!["libD.a", "libB.a", "libA.a", "libC.a"],
                vec!["libD.a", "libB.a", "libC.a", "libA.a"],
                vec!["libD.a", "libC.a", "libA.a", "libB.a"],
                vec!["libD.a", "libC.a", "libB.a", "libA.a"],
            ];
            for permutation in permutations {
                let input_libs = vec![
                    permutation[0].to_owned(),
                    permutation[1].to_owned(),
                    permutation[2].to_owned(),
                    permutation[3].to_owned(),
                ];
                let output_libs = pc_files.fix_ordering(input_libs);
                assert_eq!(output_libs.len(), 4);
                assert_eq!(output_libs[0], "libD.a");
                assert_eq!(output_libs[1], "libC.a");
                assert_eq!(output_libs[2], "libB.a");
                assert_eq!(output_libs[3], "libA.a");
            }
        }

        // Test parsing of a couple different Requires: lines.
        {
            let pc_file = PcFile::from_str(
                "test",
                "Libs: -ltest\n\
                 Requires: cairo libpng",
                &target_triplet,
            )
            .unwrap();
            assert_eq!(pc_file.deps, vec!["cairo", "libpng"]);
            let pc_file = PcFile::from_str(
                "test",
                "Libs: -ltest\n\
                 Requires: cairo xcb >= 1.6 xcb-render >= 1.6",
                &target_triplet,
            )
            .unwrap();
            assert_eq!(pc_file.deps, vec!["cairo", "xcb", "xcb-render"]);
            let pc_file = PcFile::from_str(
                "test",
                "Libs: -ltest\n\
                 Requires: glib-2.0, gobject-2.0",
                &target_triplet,
            )
            .unwrap();
            assert_eq!(pc_file.deps, vec!["glib-2.0", "gobject-2.0"]);
            let pc_file = PcFile::from_str(
                "test",
                "Libs: -ltest\n\
                 Requires: glib-2.0 >=  2.58.0, gobject-2.0 >=  2.58.0",
                &target_triplet,
            )
            .unwrap();
            assert_eq!(pc_file.deps, vec!["glib-2.0", "gobject-2.0"]);
        }

        clean_env();
    }

    fn clean_env() {
        env::remove_var("TARGET");
        env::remove_var("VCPKG_ROOT");
        env::remove_var("VCPKGRS_DYNAMIC");
        env::remove_var("RUSTFLAGS");
        env::remove_var("CARGO_CFG_TARGET_FEATURE");
        env::remove_var("VCPKGRS_DISABLE");
        env::remove_var("VCPKGRS_NO_LIBMYSQL");
        env::remove_var("VCPKGRS_TRIPLET");
    }

    // path to a to vcpkg installation to test against
    fn vcpkg_test_tree_loc(name: &str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(env::var("CARGO_MANIFEST_DIR").unwrap());
        path.push("test-data");
        path.push(name);
        path
    }
}
