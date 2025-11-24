//! Vendored Libdbus build.
//! 
//! Configures + Compiles + Statically Links a pinned libdbus release.
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::path::Path;
use std::error::Error;
use std::string::ToString;
use std::collections::hash_map::Iter;

// Version of vendored libdbus
const DBUS_VERSION: &str = "\"1.14.4\"";
const DBUS_VERSION_MAJOR: u32 = 1;
const DBUS_VERSION_MINOR: u32 = 14;
const DBUS_VERSION_MICRO: u32 = 4;

// Location of dbus-daemon socket.
// Systemd Default: https://github.com/systemd/systemd/blob/main/src/libsystemd/sd-bus/bus-internal.h#L20
const DBUS_SYSTEM_BUS_DEFAULT_ADDRESS: &str = "/run/dbus/system_bus_socket";
const DBUS_SESSION_SOCKET_DIR: &str = "/tmp";
const DBUS_MACHINE_UUID_FILE: &str = "/etc/machine-id";

/// Source Files Required
/// Reference: https://github.com/freedesktop/dbus/blob/master/dbus/meson.build#L31
/// Meson variable name: dbus_lib_sources
const SOURCE_FILES: &'static [&'static str] = &[
    "./vendor/dbus/dbus/dbus-address.c",
    "./vendor/dbus/dbus/dbus-auth.c",
    "./vendor/dbus/dbus/dbus-bus.c",
    "./vendor/dbus/dbus/dbus-connection.c",
    "./vendor/dbus/dbus/dbus-credentials.c",
    "./vendor/dbus/dbus/dbus-errors.c",
    "./vendor/dbus/dbus/dbus-keyring.c",
    "./vendor/dbus/dbus/dbus-marshal-byteswap.c",
    "./vendor/dbus/dbus/dbus-marshal-header.c",
    "./vendor/dbus/dbus/dbus-marshal-recursive.c",
    "./vendor/dbus/dbus/dbus-marshal-validate.c",
    "./vendor/dbus/dbus/dbus-message.c",
    "./vendor/dbus/dbus/dbus-misc.c",
    "./vendor/dbus/dbus/dbus-nonce.c",
    "./vendor/dbus/dbus/dbus-object-tree.c",
    "./vendor/dbus/dbus/dbus-pending-call.c",
    "./vendor/dbus/dbus/dbus-resources.c",
    "./vendor/dbus/dbus/dbus-server-debug-pipe.c",
    "./vendor/dbus/dbus/dbus-server-socket.c",
    "./vendor/dbus/dbus/dbus-server.c",
    "./vendor/dbus/dbus/dbus-sha.c",
    "./vendor/dbus/dbus/dbus-signature.c",
    "./vendor/dbus/dbus/dbus-syntax.c",
    "./vendor/dbus/dbus/dbus-threads.c",
    "./vendor/dbus/dbus/dbus-timeout.c",
    "./vendor/dbus/dbus/dbus-transport-socket.c",
    "./vendor/dbus/dbus/dbus-transport.c",
    "./vendor/dbus/dbus/dbus-watch.c",
];

/// Internal files
/// Reference: https://github.com/freedesktop/dbus/blob/master/dbus/meson.build#L67
/// Meson variable name: dbus_shared_sources
const INTERNAL_FILES: &'static [&'static str] = &[
    "./vendor/dbus/dbus/dbus-dataslot.c",
    "./vendor/dbus/dbus/dbus-file.c",
    "./vendor/dbus/dbus/dbus-hash.c",
    "./vendor/dbus/dbus/dbus-internals.c",
    "./vendor/dbus/dbus/dbus-list.c",
    "./vendor/dbus/dbus/dbus-marshal-basic.c",
    "./vendor/dbus/dbus/dbus-memory.c",
    "./vendor/dbus/dbus/dbus-mempool.c",
    "./vendor/dbus/dbus/dbus-pipe.c",
    "./vendor/dbus/dbus/dbus-string.c",
    "./vendor/dbus/dbus/dbus-sysdeps.c",
];

/// Unix platform files
/// References:
/// - https://github.com/freedesktop/dbus/blob/master/dbus/meson.build#L139
/// - https://github.com/freedesktop/dbus/blob/master/dbus/meson.build#L144
/// Meson variable names: dbus_lib_sources + dbus_shared_sources
const UNIX_SOURCES: &'static [&'static str] = &[
    // Lib
    "./vendor/dbus/dbus/dbus-uuidgen.c",
    "./vendor/dbus/dbus/dbus-server-unix.c",

    // Shared
    "./vendor/dbus/dbus/dbus-file-unix.c",
    "./vendor/dbus/dbus/dbus-pipe-unix.c",
    "./vendor/dbus/dbus/dbus-sysdeps-pthread.c",
    "./vendor/dbus/dbus/dbus-sysdeps-unix.c",
    "./vendor/dbus/dbus/dbus-transport-unix.c",
    "./vendor/dbus/dbus/dbus-userdb.c",
];

/// Taken directly from the libdbus meson config
/// Reference: https://github.com/freedesktop/dbus/blob/master/meson.build#L1026
/// Meson variable: compile_warnings
const CWARNINGS: &'static [&'static str] = &[
    // These warnings are intentionally disabled:
    //  - missing field initializers being implicitly 0 is a feature,
    //    not a bug
    //  - -Wunused-parameter is annoying when writing callbacks that follow
    //    a fixed signature but do not necessarily need all of its parameters
    "-Wno-missing-field-initializers",
    "-Wno-unused-parameter",
    "-Wno-error=duplicated-branches",

    // Under clang, need to ignore cast-align and sign-compare
    // Reference: https://bugs.freedesktop.org/show_bug.cgi?id=10599#c5
    "-Wno-error=cast-align",
    "-Wno-error=sign-compare",

    // Required for 1.14 but not 1.15+
    "-Wno-error=unused-but-set-variable",

    // General warnings for both C and C++
    "-Warray-bounds",
    "-Wchar-subscripts",
    "-Wdouble-promotion",
    "-Wduplicated-branches",
    "-Wduplicated-cond",
    "-Wfloat-equal",
    "-Wformat-nonliteral",
    "-Wformat-security",
    "-Wformat=2",
    "-Winit-self",
    "-Winline",
    "-Wlogical-op",
    "-Wmissing-declarations",
    "-Wmissing-format-attribute",
    "-Wmissing-include-dirs",
    "-Wmissing-noreturn",
    "-Wnull-dereference",
    "-Wpacked",
    "-Wpointer-arith",
    "-Wredundant-decls",
    "-Wrestrict",
    "-Wreturn-type",
    "-Wshadow",
    "-Wstrict-aliasing",
    "-Wswitch-default",
    "-Wswitch-enum",
    "-Wundef",
    "-Wunused-but-set-variable",
    "-Wwrite-strings",

    // Extra warnings just for C
    "-Wdeclaration-after-statement",
    "-Wimplicit-function-declaration",
    "-Wjump-misses-init",
    "-Wmissing-prototypes",
    "-Wnested-externs",
    "-Wold-style-definition",
    "-Wpointer-sign",
    "-Wstrict-prototypes",
];

/// Wrapper type for the libdbus config.
///
/// Boils down to a set of C defs/undefs.
struct Config<'a> {
    defs: HashMap<&'a str, Option<String>>,
    undefs: Vec<&'a str>,
}

impl<'a> Config<'a> {
    fn new() -> Self {
        Self {
            defs: HashMap::new(),
            undefs: Vec::new(),
        }
    }

    fn enable(&mut self, flag: &'a str) {
        self.defs.insert(flag, None);
    }

    fn disable(&mut self, flag: &'a str) {
        self.undefs.push(flag);
    }

    fn set<T: ToString>(&mut self, flag: &'a str, value: T) {
        self.defs.insert(flag, Some(value.to_string()));
    }

    /// Iterator over all C defines, set with -Dopt=val
    fn defs<'b>(&'b self) -> Iter<'b, &'a str, Option<String>> {
        self.defs.iter()
    }

    /// Iterator over all undefs, that must be set with -Uopt
    fn undefs<'b>(&'b self) -> std::slice::Iter<'b, &'a str> {
        self.undefs.iter()
    }
}

/// Config.h and dbus-arch-deps.h stubs (we generate config programatically
/// instead to be passed directly to cc)
fn generate_stubs(outdir: &str) -> Result<(), Box<dyn Error>> {
    let mut config = File::create(format!("{}/include/config.h", outdir))?;
    let mut arch = File::create(format!("{}/include/dbus/dbus-arch-deps.h", outdir))?;

    // Stub out the file. Certain flags are only for newer gcc versions.
    config.write_all(b"
        #pragma once
        #pragma GCC diagnostic ignored \"-Wunused-but-set-variable\"
        #if ((defined __GNUC__ && __GNUC__ > 7) || defined(__clang__))
            #pragma GCC diagnostic ignored \"-Wduplicated-branches\"
            #pragma GCC diagnostic ignored \"-Wsign-compare\"
        #endif
    ")?;

    // Define integer types.
    //
    // The libdbus builder relies on detecting int/long/long long width.
    // but we can just use modern C types like uint64_t.
    //
    // Pointer width will still be detected at build time by our
    // generate_config() method.
    arch.write_all(b" 
        #ifndef DBUS_ARCH_DEPS_H
        #define DBUS_ARCH_DEPS_H

        #include <stdint.h>
        #include <stdarg.h>
        #include <dbus/dbus-macros.h>

        DBUS_BEGIN_DECLS

        _DBUS_GNUC_EXTENSION typedef int64_t dbus_int64_t;
        _DBUS_GNUC_EXTENSION typedef uint64_t dbus_uint64_t;

        #define DBUS_INT64_CONSTANT(val)  (_DBUS_GNUC_EXTENSION (val##L))
        #define DBUS_UINT64_CONSTANT(val) (_DBUS_GNUC_EXTENSION (val##UL))

        typedef int32_t dbus_int32_t;
        typedef uint32_t dbus_uint32_t;

        typedef int16_t dbus_int16_t;
        typedef uint16_t dbus_uint16_t;

        // Required for 1.14 but not 1.15
        #define DBUS_VA_COPY va_copy

        DBUS_END_DECLS

        #endif /* DBUS_ARCH_DEPS_H */
    ")?;
    Ok(())
}

/// Generate configuration based on target platform + architecture
fn generate_config(cc: &mut cc::Build, config: &mut Config) -> Result<(), Box<dyn Error>> {
    let ptr_width: u32 = u32::from_str_radix(&env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .or(Err("Pointer width missing."))?, 10)?;

    // Target pointer width
    config.set("DBUS_SIZEOF_VOID_P", ptr_width / 8);

    // Ensure dbus is aware it's compiling
    config.enable("DBUS_COMPILATION");

    // D-Bus no longer supports platforms with no 64-bit integer type.
    config.enable("DBUS_HAVE_INT64");
    config.enable("DBUS_INT64_MODIFIER");

    // Use libc's bswap implementation instead of libdbus' handrolled version
    config.enable("HAVE_BYTESWAP_H");

    // Add version definitions
    let version_number = (DBUS_VERSION_MAJOR << 16) | (DBUS_VERSION_MINOR << 8) | DBUS_VERSION_MICRO;
    config.set("VERSION", DBUS_VERSION);
    config.set("DBUS_VERSION_STRING", DBUS_VERSION);
    config.set("DBUS_VERSION", version_number);
    config.set("DBUS_MAJOR_VERSION", DBUS_VERSION_MAJOR);
    config.set("DBUS_MINOR_VERSION", DBUS_VERSION_MINOR);
    config.set("DBUS_MICRO_VERSION", DBUS_VERSION_MICRO);

    // Basic config (sync, disable asserts, enable checks)
    config.set("DBUS_SESSION_BUS_CONNECT_ADDRESS", "\"autolaunch:\"");
    config.enable("HAVE_DECL_MSG_NOSIGNAL");
    config.enable("DBUS_USE_SYNC");
    config.enable("DBUS_ENABLE_CHECKS");
    config.disable("DBUS_DISABLE_CHECKS");
    config.enable("DBUS_DISABLE_ASSERT");

    // Target endian
    match env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() {
        Ok("little") => {
            config.disable("WORDS_BIGENDIAN");
        },
        Ok("big") => {
            config.enable("WORDS_BIGENDIAN");
        }
        _ => unreachable!(),
    }

    // Platform dependent config
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("linux") => {
            config.enable("DBUS_UNIX");
            config.enable("DBUS_HAVE_LINUX_EPOLL");
            config.enable("HAVE_EPOLL");
            config.enable("HAVE_ERRNO_H");
            config.enable("HAVE_SOCKLEN_T");
            config.enable("HAVE_GETPWNAM_R");
            config.enable("HAVE_UNIX_FD_PASSING");
            config.enable("HAVE_LOCALE_H");
            config.enable("HAVE_DECL_ENVIRON");

            // Disable
            config.disable("HAVE_GETPEERUCRED");
            config.disable("HAVE_CMSGCRED");

            // System paths
            let bus = format!("\"unix:path={}\"", DBUS_SYSTEM_BUS_DEFAULT_ADDRESS);
            let dir = format!("\"{}\"", DBUS_SESSION_SOCKET_DIR);
            let uid = format!("\"{}\"", DBUS_MACHINE_UUID_FILE);
            config.set("DBUS_SESSION_SOCKET_DIR", dir);
            config.set("DBUS_SYSTEM_BUS_DEFAULT_ADDRESS", bus);
            config.set("DBUS_MACHINE_UUID_FILE", uid);

            // GNU source required for struct ucred on linux
            config.enable("_GNU_SOURCE");
            cc.files(UNIX_SOURCES);
        }
        _ => return Err("Unsupported platform.".into()),
    }
    Ok(())
}

/// Vendored build entrypoint.
///
/// Instead of relying on cmake or meson. The build configuration was simple
/// enough to port to cc directly. Allowing downstream crates to rely on the
/// dbus crate without additional host dependencies.
pub fn build_libdbus() -> Result<(), Box<dyn Error>> {
    let mut compiler = cc::Build::new();
    let mut config = Config::new();

    // Obtain the output directoryy
    let outdir = env::var("OUT_DIR").or(Err("No output directory."))?;

    // Create a directory for generated headers
    std::fs::create_dir_all(format!("{}/include/dbus", &outdir))?;

    // Ensure submodule is checked out
    if !Path::new("vendor/dbus/dbus").exists() {
        let _ = Command::new("git")
            .args(&["submodule", "update", "--init", "vendor/dbus"])
            .status()?;
    }

    // Complete configuration
    generate_config(&mut compiler, &mut config)?;

    // Generate stub files
    generate_stubs(&outdir)?;

    // Generic source files
    compiler.files(SOURCE_FILES);
    compiler.files(INTERNAL_FILES);
    compiler.include(format!("{}/include", &outdir));
    compiler.include("./vendor/dbus/");

    // Set the defines
    for (opt, val) in config.defs() {
        compiler.define(opt, val.as_deref());
    }

    // Set the undefs
    for opt in config.undefs() {
        compiler.flag(&format!("-U{}", opt));
    }

    // Set the C flags
    for flag in CWARNINGS.iter() {
        compiler.flag_if_supported(flag);
    }

    // dbus makes assumptions about aliasing that Standard C does not guarantee,
    // particularly in DBusString.
    // See https://gitlab.freedesktop.org/dbus/dbus/-/issues/4
    compiler.flag_if_supported("-fno-strict-aliasing");

    // Complete the build
    compiler
        .shared_flag(false)
        .static_flag(true)
        .compile("libdbus.a");

    // Tell cargo to tell rustc to link it in
    println!("cargo:rustc-link-search=native={}", outdir);
    println!("cargo:rustc-link-lib=static=dbus");
    Ok(())
}
