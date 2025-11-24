// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! Build the non-Rust components.

// It seems like it would be a good idea to use `log!` for logging, but it
// isn't worth having the external dependencies (one for the `log` crate, and
// another for the concrete logging implementation). Instead we use `eprintln!`
// to log everything to stderr.

use std::{
    ffi::{OsStr, OsString},
    fs::{self, DirEntry},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

mod env {
    use std::ffi::OsString;

    /// Read an environment variable and tell Cargo that we depend on it.
    ///
    /// The name is static since we intend to only read a static set of environment
    /// variables.
    pub fn var_os(name: &'static str) -> Option<OsString> {
        println!("cargo:rerun-if-env-changed={}", name);
        std::env::var_os(name)
    }

    pub fn var(name: &'static str) -> Option<String> {
        var_os(name).and_then(|value| value.into_string().ok())
    }
}

const X86: &str = "x86";
const X86_64: &str = "x86_64";
const AARCH64: &str = "aarch64";
const ARM: &str = "arm";
const WASM32: &str = "wasm32";

#[rustfmt::skip]
const RING_SRCS: &[(&[&str], &str)] = &[
    (&[], "crypto/curve25519/curve25519.c"),
    (&[], "crypto/fipsmodule/aes/aes_nohw.c"),
    (&[], "crypto/fipsmodule/bn/montgomery.c"),
    (&[], "crypto/fipsmodule/bn/montgomery_inv.c"),
    (&[], "crypto/fipsmodule/ec/ecp_nistz.c"),
    (&[], "crypto/fipsmodule/ec/gfp_p256.c"),
    (&[], "crypto/fipsmodule/ec/gfp_p384.c"),
    (&[], "crypto/fipsmodule/ec/p256.c"),
    (&[], "crypto/limbs/limbs.c"),
    (&[], "crypto/mem.c"),
    (&[], "crypto/poly1305/poly1305.c"),

    (&[ARM, X86_64, X86], "crypto/crypto.c"),

    (&[X86_64, X86], "crypto/cpu_intel.c"),

    (&[X86], "crypto/fipsmodule/aes/asm/aesni-x86.pl"),
    (&[X86], "crypto/fipsmodule/aes/asm/ghash-x86.pl"),
    (&[X86], "crypto/fipsmodule/aes/asm/vpaes-x86.pl"),
    (&[X86], "crypto/fipsmodule/bn/asm/x86-mont.pl"),
    (&[X86], "crypto/chacha/asm/chacha-x86.pl"),

    (&[X86_64], "crypto/chacha/asm/chacha-x86_64.pl"),
    (&[X86_64], "crypto/curve25519/curve25519_64_adx.c"),
    (&[X86_64], "crypto/fipsmodule/aes/asm/aes-gcm-avx2-x86_64.pl"),
    (&[X86_64], "crypto/fipsmodule/aes/asm/aesni-gcm-x86_64.pl"),
    (&[X86_64], "crypto/fipsmodule/aes/asm/aesni-x86_64.pl"),
    (&[X86_64], "crypto/fipsmodule/aes/asm/ghash-x86_64.pl"),
    (&[X86_64], "crypto/fipsmodule/aes/asm/vpaes-x86_64.pl"),
    (&[X86_64], "crypto/fipsmodule/bn/asm/x86_64-mont.pl"),
    (&[X86_64], "crypto/fipsmodule/bn/asm/x86_64-mont5.pl"),
    (&[X86_64], "crypto/fipsmodule/ec/asm/p256-x86_64-asm.pl"),
    (&[X86_64], SHA512_X86_64),
    (&[X86_64], "crypto/cipher/asm/chacha20_poly1305_x86_64.pl"),
    (&[X86_64], "third_party/fiat/asm/fiat_curve25519_adx_mul.S"),
    (&[X86_64], "third_party/fiat/asm/fiat_curve25519_adx_square.S"),

    (&[AARCH64, X86_64], "crypto/fipsmodule/ec/p256-nistz.c"),

    (&[ARM], "crypto/fipsmodule/aes/asm/bsaes-armv7.pl"),
    (&[ARM], "crypto/fipsmodule/aes/asm/ghash-armv4.pl"),
    (&[ARM], "crypto/fipsmodule/aes/asm/vpaes-armv7.pl"),
    (&[ARM], "crypto/fipsmodule/bn/asm/armv4-mont.pl"),
    (&[ARM], "crypto/chacha/asm/chacha-armv4.pl"),
    (&[ARM], "crypto/curve25519/asm/x25519-asm-arm.S"),
    (&[ARM], "crypto/poly1305/poly1305_arm.c"),
    (&[ARM], "crypto/poly1305/poly1305_arm_asm.S"),
    (&[ARM], "crypto/fipsmodule/sha/asm/sha256-armv4.pl"),
    (&[ARM], "crypto/fipsmodule/sha/asm/sha512-armv4.pl"),

    (&[AARCH64], "crypto/chacha/asm/chacha-armv8.pl"),
    (&[AARCH64], "crypto/cipher/asm/chacha20_poly1305_armv8.pl"),
    (&[AARCH64], "crypto/fipsmodule/aes/asm/aesv8-armx.pl"),
    (&[AARCH64], "crypto/fipsmodule/aes/asm/aesv8-gcm-armv8.pl"),
    (&[AARCH64], "crypto/fipsmodule/aes/asm/ghash-neon-armv8.pl"),
    (&[AARCH64], "crypto/fipsmodule/aes/asm/ghashv8-armx.pl"),
    (&[AARCH64], "crypto/fipsmodule/aes/asm/vpaes-armv8.pl"),
    (&[AARCH64], "crypto/fipsmodule/bn/asm/armv8-mont.pl"),
    (&[AARCH64], "crypto/fipsmodule/ec/asm/p256-armv8-asm.pl"),
    (&[AARCH64], SHA512_ARMV8),
];

const SHA256_X86_64: &str = "crypto/fipsmodule/sha/asm/sha256-x86_64.pl";
const SHA512_X86_64: &str = "crypto/fipsmodule/sha/asm/sha512-x86_64.pl";

const SHA256_ARMV8: &str = "crypto/fipsmodule/sha/asm/sha256-armv8.pl";
const SHA512_ARMV8: &str = "crypto/fipsmodule/sha/asm/sha512-armv8.pl";

const RING_TEST_SRCS: &[&str] = &[("crypto/constant_time_test.c")];

const PREGENERATED: &str = "pregenerated";

fn cpp_flags(compiler: &cc::Tool) -> &'static [&'static str] {
    if !compiler.is_like_msvc() {
        static NON_MSVC_FLAGS: &[&str] = &[
            "-fvisibility=hidden",
            "-std=c1x", // GCC 4.6 requires "c1x" instead of "c11"
            "-Wall",
            "-Wbad-function-cast",
            "-Wcast-align",
            "-Wcast-qual",
            "-Wconversion",
            "-Wmissing-field-initializers",
            "-Wmissing-include-dirs",
            "-Wnested-externs",
            "-Wredundant-decls",
            "-Wshadow",
            "-Wsign-compare",
            "-Wsign-conversion",
            "-Wstrict-prototypes",
            "-Wundef",
            "-Wuninitialized",
        ];
        NON_MSVC_FLAGS
    } else {
        static MSVC_FLAGS: &[&str] = &[
            "/Gy", // Enable function-level linking.
            "/Zc:wchar_t",
            "/Zc:forScope",
            "/Zc:inline",
            // Warnings.
            "/Wall",
            "/wd4127", // C4127: conditional expression is constant
            "/wd4464", // C4464: relative include path contains '..'
            "/wd4514", // C4514: <name>: unreferenced inline function has be
            "/wd4710", // C4710: function not inlined
            "/wd4711", // C4711: function 'function' selected for inline expansion
            "/wd4820", // C4820: <struct>: <n> bytes padding added after <name>
            "/wd5045", /* C5045: Compiler will insert Spectre mitigation for memory load if
                        * /Qspectre switch specified */
        ];
        MSVC_FLAGS
    }
}

// None means "any OS" or "any target". The first match in sequence order is
// taken.
const ASM_TARGETS: &[AsmTarget] = &[
    AsmTarget {
        oss: LINUX_ABI,
        arch: AARCH64,
        perlasm_format: "linux64",
    },
    AsmTarget {
        oss: LINUX_ABI,
        arch: ARM,
        perlasm_format: "linux32",
    },
    AsmTarget {
        oss: LINUX_ABI,
        arch: X86,
        perlasm_format: "elf",
    },
    AsmTarget {
        oss: LINUX_ABI,
        arch: X86_64,
        perlasm_format: "elf",
    },
    AsmTarget {
        oss: &["horizon"],
        arch: ARM,
        perlasm_format: "linux32",
    },
    AsmTarget {
        oss: APPLE_ABI,
        arch: AARCH64,
        perlasm_format: "ios64",
    },
    AsmTarget {
        oss: APPLE_ABI,
        arch: X86_64,
        perlasm_format: "macosx",
    },
    AsmTarget {
        oss: &[WINDOWS],
        arch: X86,
        perlasm_format: WIN32N,
    },
    AsmTarget {
        oss: &[WINDOWS],
        arch: X86_64,
        perlasm_format: NASM,
    },
    AsmTarget {
        oss: &[WINDOWS],
        arch: AARCH64,
        perlasm_format: "win64",
    },
];

struct AsmTarget {
    /// Operating systems.
    oss: &'static [&'static str],

    /// Architectures.
    arch: &'static str,

    /// The PerlAsm format name.
    perlasm_format: &'static str,
}

impl AsmTarget {
    fn use_nasm(&self) -> bool {
        [WIN32N, NASM].contains(&self.perlasm_format)
    }
}

/// Operating systems that have the same ABI as Linux on every architecture
/// mentioned in `ASM_TARGETS`.
const LINUX_ABI: &[&str] = &[
    "android",
    "dragonfly",
    "freebsd",
    "fuchsia",
    "haiku",
    "hurd",
    "illumos",
    "netbsd",
    "openbsd",
    "linux",
    "redox",
    "solaris",
];

const WIN32N: &str = "win32n";
const NASM: &str = "nasm";

/// Operating systems that have the same ABI as macOS on every architecture
/// mentioned in `ASM_TARGETS`.
const APPLE_ABI: &[&str] = &["ios", "macos", "tvos", "visionos", "watchos"];

const WINDOWS: &str = "windows";

fn main() {
    // Avoid assuming the working directory is the same is the $CARGO_MANIFEST_DIR so that toolchains
    // which may assume other working directories can still build this code.
    let c_root_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should always be set"),
    );

    // Keep in sync with `core_name_and_version!` in prefixed.rs.
    let core_name_and_version = [
        &env::var("CARGO_PKG_NAME").unwrap(),
        "core",
        &env::var("CARGO_PKG_VERSION_MAJOR").unwrap(),
        &env::var("CARGO_PKG_VERSION_MINOR").unwrap(),
        &env::var("CARGO_PKG_VERSION_PATCH").unwrap(),
        &env::var("CARGO_PKG_VERSION_PRE").unwrap(), // Often empty
    ]
    .join("_");
    // Ensure `links` in Cargo.toml is consistent with the version.
    assert_eq!(
        &env::var("CARGO_MANIFEST_LINKS").unwrap(),
        &core_name_and_version
    );

    const RING_PREGENERATE_ASM: &str = "RING_PREGENERATE_ASM";
    match env::var_os(RING_PREGENERATE_ASM).as_deref() {
        Some(s) if s == "1" => {
            pregenerate_asm_main(&c_root_dir, &core_name_and_version);
        }
        None => ring_build_rs_main(&c_root_dir, &core_name_and_version),
        _ => {
            panic!("${} has an invalid value", RING_PREGENERATE_ASM);
        }
    }
}

fn ring_build_rs_main(c_root_dir: &Path, core_name_and_version: &str) {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = PathBuf::from(out_dir);

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let endian = env::var("CARGO_CFG_TARGET_ENDIAN").unwrap();
    let is_little_endian = endian == "little";

    let is_git = fs::metadata(c_root_dir.join(".git")).is_ok();

    // Published builds are always built in release mode.
    let is_debug = is_git && env::var("DEBUG").unwrap() != "false";

    // During local development, force warnings in non-Rust code to be treated
    // as errors. Since warnings are highly compiler-dependent and compilers
    // don't maintain backward compatibility w.r.t. which warnings they issue,
    // don't do this for packaged builds.
    let force_warnings_into_errors = is_git;

    let target = Target {
        arch,
        os,
        env,
        is_debug,
        force_warnings_into_errors,
    };

    let asm_target = if is_little_endian {
        ASM_TARGETS.iter().find(|asm_target| {
            asm_target.arch == target.arch && asm_target.oss.contains(&target.os.as_ref())
        })
    } else {
        None
    };

    // If `.git` exists then assume this is the "local hacking" case where
    // we want to make it easy to build *ring* using `cargo build`/`cargo test`
    // without a prerequisite `package` step, at the cost of needing additional
    // tools like `Perl` and/or `nasm`.
    //
    // If `.git` doesn't exist then assume that this is a packaged build where
    // we want to optimize for minimizing the build tools required: No Perl,
    // no nasm, etc.
    let generated_dir = if !is_git {
        c_root_dir.join(PREGENERATED)
    } else {
        generate_sources_and_preassemble(
            &out_dir,
            asm_target.into_iter(),
            c_root_dir,
            core_name_and_version,
        );
        out_dir.clone()
    };

    build_c_code(
        asm_target,
        &target,
        &generated_dir,
        c_root_dir,
        &out_dir,
        core_name_and_version,
    );
    emit_rerun_if_changed()
}

fn pregenerate_asm_main(c_root_dir: &Path, core_name_and_version: &str) {
    let pregenerated = c_root_dir.join(PREGENERATED);
    fs::create_dir(&pregenerated).unwrap();
    generate_sources_and_preassemble(
        &pregenerated,
        ASM_TARGETS.iter(),
        c_root_dir,
        core_name_and_version,
    );
}

fn generate_sources_and_preassemble<'a>(
    out_dir: &Path,
    asm_targets: impl Iterator<Item = &'a AsmTarget>,
    c_root_dir: &Path,
    core_name_and_version: &str,
) {
    generate_prefix_symbols_headers(out_dir, core_name_and_version).unwrap();

    let perl_exe = get_perl_exe();

    for asm_target in asm_targets {
        let perlasm_src_dsts = perlasm_src_dsts(out_dir, asm_target);
        perlasm(&perl_exe, &perlasm_src_dsts, asm_target, c_root_dir);

        if asm_target.use_nasm() {
            // Package pregenerated object files in addition to pregenerated
            // assembly language source files, so that the user doesn't need
            // to install the assembler.
            let srcs = asm_srcs(perlasm_src_dsts);
            for src in srcs {
                nasm(&src, asm_target.arch, out_dir, out_dir, c_root_dir);
            }
        }
    }
}

struct Target {
    arch: String,
    os: String,
    env: String,

    /// Is this a debug build? This affects whether assertions might be enabled
    /// in the C code. For packaged builds, this should always be `false`.
    is_debug: bool,

    /// true: Force warnings to be treated as errors.
    /// false: Use the default behavior (perhaps determined by `$CFLAGS`, etc.)
    force_warnings_into_errors: bool,
}

fn build_c_code(
    asm_target: Option<&AsmTarget>,
    target: &Target,
    generated_dir: &Path,
    c_root_dir: &Path,
    out_dir: &Path,
    core_name_and_version: &str,
) {
    let (asm_srcs, obj_srcs) = if let Some(asm_target) = asm_target {
        let perlasm_src_dsts = perlasm_src_dsts(generated_dir, asm_target);

        let asm_srcs = asm_srcs(perlasm_src_dsts);

        if asm_target.use_nasm() {
            // Nasm was already used to generate the object files, so use them instead of
            // assembling.
            let obj_srcs = asm_srcs
                .iter()
                .map(|src| obj_path(generated_dir, src.as_path()))
                .collect::<Vec<_>>();
            (vec![], obj_srcs)
        } else {
            (asm_srcs, vec![])
        }
    } else {
        (vec![], vec![])
    };

    let core_srcs = sources_for_arch(&target.arch)
        .into_iter()
        .filter(|p| !is_perlasm(p))
        .filter(|p| {
            if let Some(extension) = p.extension() {
                // We don't (and can't) use any .S on Windows since MSVC and NASM can't assemble
                // them.
                if extension == "S"
                    && (target.arch == X86_64 || target.arch == X86)
                    && target.os == WINDOWS
                {
                    return false;
                }
            }
            true
        })
        .collect::<Vec<_>>();

    let test_srcs = RING_TEST_SRCS.iter().map(PathBuf::from).collect::<Vec<_>>();

    let libs = [
        (
            core_name_and_version,
            &core_srcs[..],
            &asm_srcs[..],
            &obj_srcs[..],
        ),
        (
            &(String::from(core_name_and_version) + "_test"),
            &test_srcs[..],
            &[],
            &[],
        ),
    ];

    // XXX: Ideally, ring-test would only be built for `cargo test`, but Cargo
    // can't do that yet.
    libs.iter()
        .for_each(|&(lib_name, srcs, asm_srcs, obj_srcs)| {
            let srcs = srcs.iter().chain(asm_srcs);
            build_library(
                target,
                c_root_dir,
                out_dir,
                lib_name,
                srcs,
                generated_dir,
                obj_srcs,
            )
        });

    println!(
        "cargo:rustc-link-search=native={}",
        out_dir.to_str().expect("Invalid path")
    );
}

fn new_build(target: &Target, c_root_dir: &Path, include_dir: &Path) -> cc::Build {
    let mut b = cc::Build::new();
    configure_cc(&mut b, target, c_root_dir, include_dir);
    b
}

fn build_library<'a>(
    target: &Target,
    c_root_dir: &Path,
    out_dir: &Path,
    lib_name: &str,
    srcs: impl Iterator<Item = &'a PathBuf>,
    include_dir: &Path,
    preassembled_objs: &[PathBuf],
) {
    let mut c = new_build(target, c_root_dir, include_dir);

    // Compile all the (dirty) source files into object files.
    srcs.for_each(|src| {
        c.file(c_root_dir.join(src));
    });

    preassembled_objs.iter().for_each(|obj| {
        c.object(obj);
    });

    // Rebuild the library if necessary.
    let lib_path = PathBuf::from(out_dir).join(format!("lib{}.a", lib_name));

    // Handled below.
    let _ = c.cargo_metadata(false);

    c.compile(
        lib_path
            .file_name()
            .and_then(|f| f.to_str())
            .expect("No filename"),
    );

    // Link the library. This works even when the library doesn't need to be
    // rebuilt.
    println!("cargo:rustc-link-lib=static={}", lib_name);
}

fn obj_path(out_dir: &Path, src: &Path) -> PathBuf {
    let mut out_path = out_dir.join(src.file_name().unwrap());
    // To eliminate unnecessary conditional logic, use ".o" as the extension,
    // even when the compiler (e.g. MSVC) would normally use something else
    // (e.g. ".obj"). cc-rs seems to do the same.
    assert!(out_path.set_extension("o"));
    out_path
}

fn configure_cc(c: &mut cc::Build, target: &Target, c_root_dir: &Path, include_dir: &Path) {
    let compiler = c.get_compiler();
    // FIXME: On Windows AArch64 we currently must use Clang to compile C code
    let compiler = if target.os == WINDOWS && target.arch == AARCH64 && !compiler.is_like_clang() {
        let _ = c.compiler("clang");
        c.get_compiler()
    } else {
        compiler
    };

    let _ = c.include(c_root_dir.join("include"));
    let _ = c.include(include_dir);
    for f in cpp_flags(&compiler) {
        let _ = c.flag(f);
    }

    if APPLE_ABI.contains(&target.os.as_str()) {
        // ``-gfull`` is required for Darwin's |-dead_strip|.
        let _ = c.flag("-gfull");
    } else if !compiler.is_like_msvc() {
        let _ = c.flag("-g3");
    };

    if !target.is_debug {
        let _ = c.define("NDEBUG", None);
    }

    if target.arch == X86 {
        let is_msvc_not_clang_cl = compiler.is_like_msvc() && !compiler.is_like_clang_cl();
        if !is_msvc_not_clang_cl {
            let _ = c.flag("-msse2");
        }
    }

    // Allow cross-compiling without a target sysroot for these targets.
    if (target.arch == WASM32)
        || (target.os == "linux" && target.env == "musl" && target.arch != X86_64)
    {
        // TODO: Expand this to non-clang compilers in 0.17.0 if practical.
        if compiler.is_like_clang() {
            let _ = c.flag("-nostdlibinc");
            let _ = c.define("RING_CORE_NOSTDLIBINC", "1");
        }
    }

    if target.force_warnings_into_errors {
        c.warnings_into_errors(true);
    }
}

fn nasm(file: &Path, arch: &str, include_dir: &Path, out_dir: &Path, c_root_dir: &Path) {
    let out_file = obj_path(out_dir, file);
    let oformat = match arch {
        x if x == X86_64 => "win64",
        x if x == X86 => "win32",
        _ => panic!("unsupported arch: {}", arch),
    };

    // Nasm requires that the path end in a path separator.
    let mut include_dir = include_dir.as_os_str().to_os_string();
    include_dir.push(OsString::from(String::from(std::path::MAIN_SEPARATOR)));

    let mut c = Command::new("./target/tools/windows/nasm/nasm");
    let _ = c
        .arg("-o")
        .arg(out_file.to_str().expect("Invalid path"))
        .arg("-f")
        .arg(oformat)
        .arg("-i")
        .arg("include/")
        .arg("-i")
        .arg(include_dir)
        .arg("-Xgnu")
        .arg("-gcv8")
        .arg(c_root_dir.join(file));
    run_command(c);
}

fn run_command_with_args(command_name: &Path, args: &[OsString]) {
    let mut cmd = Command::new(command_name);
    let _ = cmd.args(args);
    run_command(cmd)
}

fn run_command(mut cmd: Command) {
    eprintln!("running {:?}", cmd);
    cmd.stderr(Stdio::inherit());
    let status = cmd.status().unwrap_or_else(|e| {
        panic!("failed to execute [{:?}]: {}", cmd, e);
    });
    if !status.success() {
        panic!("execution failed");
    }
}

fn sources_for_arch(arch: &str) -> Vec<PathBuf> {
    RING_SRCS
        .iter()
        .filter(|&&(archs, _)| archs.is_empty() || archs.contains(&arch))
        .map(|&(_, p)| PathBuf::from(p))
        .collect::<Vec<_>>()
}

fn perlasm_src_dsts(out_dir: &Path, asm_target: &AsmTarget) -> Vec<(PathBuf, PathBuf)> {
    let srcs = sources_for_arch(asm_target.arch);
    let mut src_dsts = srcs
        .iter()
        .filter(|p| is_perlasm(p))
        .map(|src| (src.clone(), asm_path(out_dir, src, asm_target)))
        .collect::<Vec<_>>();

    // Some PerlAsm source files need to be run multiple times with different
    // output paths.
    {
        // Appease the borrow checker.
        let mut maybe_synthesize = |concrete, synthesized| {
            let concrete_path = PathBuf::from(concrete);
            if srcs.contains(&concrete_path) {
                let synthesized_path = PathBuf::from(synthesized);
                src_dsts.push((
                    concrete_path,
                    asm_path(out_dir, &synthesized_path, asm_target),
                ))
            }
        };
        maybe_synthesize(SHA512_X86_64, SHA256_X86_64);
        maybe_synthesize(SHA512_ARMV8, SHA256_ARMV8);
    }

    src_dsts
}

fn asm_srcs(perlasm_src_dsts: Vec<(PathBuf, PathBuf)>) -> Vec<PathBuf> {
    perlasm_src_dsts
        .into_iter()
        .map(|(_src, dst)| dst)
        .collect::<Vec<_>>()
}

fn is_perlasm(path: &Path) -> bool {
    path.extension().unwrap().to_str().unwrap() == "pl"
}

fn asm_path(out_dir: &Path, src: &Path, asm_target: &AsmTarget) -> PathBuf {
    let src_stem = src.file_stem().expect("source file without basename");

    let dst_stem = src_stem.to_str().unwrap();
    let dst_filename = format!("{}-{}", dst_stem, asm_target.perlasm_format);
    let extension = if asm_target.use_nasm() { "asm" } else { "S" };
    out_dir.join(dst_filename).with_extension(extension)
}

fn perlasm(
    perl_exe: &Path,
    src_dst: &[(PathBuf, PathBuf)],
    asm_target: &AsmTarget,
    c_root_dir: &Path,
) {
    for (src, dst) in src_dst {
        let mut args = vec![
            join_components_with_forward_slashes(&c_root_dir.join(src)),
            asm_target.perlasm_format.into(),
        ];
        if asm_target.arch == X86 {
            args.push("-fPIC".into());
        }
        // Work around PerlAsm issue for ARM and AAarch64 targets by replacing
        // back slashes with forward slashes.
        args.push(join_components_with_forward_slashes(dst));
        run_command_with_args(perl_exe, &args);
    }
}

fn join_components_with_forward_slashes(path: &Path) -> OsString {
    let parts = path.components().map(|c| c.as_os_str()).collect::<Vec<_>>();
    parts.join(OsStr::new("/"))
}

fn get_perl_exe() -> PathBuf {
    get_command("PERL_EXECUTABLE", "perl")
}

fn get_command(var: &'static str, default: &str) -> PathBuf {
    PathBuf::from(env::var_os(var).unwrap_or_else(|| default.into()))
}

// TODO: We should emit `cargo:rerun-if-changed-env` for the various
// environment variables that affect the build.
fn emit_rerun_if_changed() {
    for path in &["crypto", "include", "third_party/fiat"] {
        walk_dir(&PathBuf::from(path), &|entry| {
            let path = entry.path();
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("c") | Some("S") | Some("h") | Some("inl") | Some("pl") | None => {
                    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
                }
                _ => {
                    // Ignore other types of files.
                }
            }
        })
    }
}

fn walk_dir(dir: &Path, cb: &impl Fn(&DirEntry)) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, cb);
            } else {
                cb(&entry);
            }
        }
    }
}

/// Creates the necessary header files for symbol renaming.
///
/// For simplicity, both non-Nasm- and Nasm- style headers are always
/// generated, even though local non-packaged builds need only one of them.
fn generate_prefix_symbols_headers(
    out_dir: &Path,
    core_name_and_version: &str,
) -> Result<(), std::io::Error> {
    let prefix = &(String::from(core_name_and_version) + "_");

    generate_prefix_symbols_header(out_dir, "prefix_symbols.h", '#', None, prefix)?;

    generate_prefix_symbols_header(
        out_dir,
        "prefix_symbols_asm.h",
        '#',
        Some("#if defined(__APPLE__)"),
        prefix,
    )?;

    generate_prefix_symbols_header(
        out_dir,
        "prefix_symbols_nasm.inc",
        '%',
        Some("%ifidn __OUTPUT_FORMAT__,win32"),
        prefix,
    )?;

    Ok(())
}

fn generate_prefix_symbols_header(
    out_dir: &Path,
    filename: &str,
    pp: char,
    prefix_condition: Option<&str>,
    prefix: &str,
) -> Result<(), std::io::Error> {
    let dir = out_dir.join("ring_core_generated");
    fs::create_dir_all(&dir)?;

    let path = dir.join(filename);
    let mut file = fs::File::create(path)?;

    let filename_ident = filename.replace('.', "_").to_uppercase();
    writeln!(
        file,
        r#"
{pp}ifndef ring_core_generated_{filename_ident}
{pp}define ring_core_generated_{filename_ident}
"#,
        pp = pp,
        filename_ident = filename_ident
    )?;

    if let Some(prefix_condition) = prefix_condition {
        writeln!(file, "{}", prefix_condition)?;
        writeln!(file, "{}", prefix_all_symbols(pp, "_", prefix))?;
        writeln!(file, "{pp}else", pp = pp)?;
    };
    writeln!(file, "{}", prefix_all_symbols(pp, "", prefix))?;
    if prefix_condition.is_some() {
        writeln!(file, "{pp}endif", pp = pp)?
    }

    writeln!(file, "{pp}endif", pp = pp)?;

    Ok(())
}

fn prefix_all_symbols(pp: char, prefix_prefix: &str, prefix: &str) -> String {
    // Rename some nistz256 assembly functions to match the names of their
    // polyfills.
    static SYMBOLS_TO_RENAME: &[(&str, &str)] = &[
        ("ecp_nistz256_point_double", "p256_point_double"),
        ("ecp_nistz256_point_add", "p256_point_add"),
        ("ecp_nistz256_point_add_affine", "p256_point_add_affine"),
        ("ecp_nistz256_ord_mul_mont", "p256_scalar_mul_mont"),
        ("ecp_nistz256_ord_sqr_mont", "p256_scalar_sqr_rep_mont"),
        ("ecp_nistz256_mul_mont", "p256_mul_mont"),
        ("ecp_nistz256_sqr_mont", "p256_sqr_mont"),
    ];

    static SYMBOLS_TO_PREFIX: &[&str] = &[
        "adx_bmi2_available",
        "avx2_available",
        "CRYPTO_memcmp",
        "CRYPTO_poly1305_finish",
        "CRYPTO_poly1305_finish_neon",
        "CRYPTO_poly1305_init",
        "CRYPTO_poly1305_init_neon",
        "CRYPTO_poly1305_update",
        "CRYPTO_poly1305_update_neon",
        "ChaCha20_ctr32",
        "ChaCha20_ctr32_avx2",
        "ChaCha20_ctr32_neon",
        "ChaCha20_ctr32_nohw",
        "ChaCha20_ctr32_ssse3",
        "ChaCha20_ctr32_ssse3_4x",
        "LIMB_is_zero",
        "LIMBS_add_mod",
        "LIMBS_are_zero",
        "LIMBS_equal",
        "LIMBS_less_than",
        "LIMBS_reduce_once",
        "LIMBS_select_512_32",
        "LIMBS_shl_mod",
        "LIMBS_sub_mod",
        "LIMBS_window5_split_window",
        "LIMBS_window5_unsplit_window",
        "LIMB_shr",
        "OPENSSL_cpuid_setup",
        "aes_gcm_dec_kernel",
        "aes_gcm_dec_update_vaes_avx2",
        "aes_gcm_enc_kernel",
        "aes_gcm_enc_update_vaes_avx2",
        "aes_hw_ctr32_encrypt_blocks",
        "aes_hw_set_encrypt_key",
        "aes_hw_set_encrypt_key_alt",
        "aes_hw_set_encrypt_key_base",
        "aes_nohw_ctr32_encrypt_blocks",
        "aes_nohw_encrypt",
        "aes_nohw_set_encrypt_key",
        "aesni_gcm_decrypt",
        "aesni_gcm_encrypt",
        "bn_from_montgomery_in_place",
        "bn_gather5",
        "bn_mul_mont",
        "bn_mul_mont_nohw",
        "bn_mul4x_mont",
        "bn_mulx4x_mont",
        "bn_mul8x_mont_neon",
        "bn_mul4x_mont_gather5",
        "bn_mulx4x_mont_gather5",
        "bn_neg_inv_mod_r_u64",
        "bn_power5_nohw",
        "bn_powerx5",
        "bn_scatter5",
        "bn_sqr8x_internal",
        "bn_sqr8x_mont",
        "bn_sqrx8x_internal",
        "bsaes_ctr32_encrypt_blocks",
        "bssl_constant_time_test_conditional_memcpy",
        "bssl_constant_time_test_conditional_memxor",
        "bssl_constant_time_test_main",
        "chacha20_poly1305_open",
        "chacha20_poly1305_open_avx2",
        "chacha20_poly1305_open_sse41",
        "chacha20_poly1305_seal",
        "chacha20_poly1305_seal_avx2",
        "chacha20_poly1305_seal_sse41",
        "ecp_nistz256_mul_mont_adx",
        "ecp_nistz256_mul_mont_nohw",
        "ecp_nistz256_ord_mul_mont_adx",
        "ecp_nistz256_ord_mul_mont_nohw",
        "ecp_nistz256_ord_sqr_mont_adx",
        "ecp_nistz256_ord_sqr_mont_nohw",
        "ecp_nistz256_point_add_adx",
        "ecp_nistz256_point_add_nohw",
        "ecp_nistz256_point_add_affine_adx",
        "ecp_nistz256_point_add_affine_nohw",
        "ecp_nistz256_point_double_adx",
        "ecp_nistz256_point_double_nohw",
        "ecp_nistz256_select_w5_avx2",
        "ecp_nistz256_select_w5_nohw",
        "ecp_nistz256_select_w7_avx2",
        "ecp_nistz256_select_w7_nohw",
        "ecp_nistz256_sqr_mont_adx",
        "ecp_nistz256_sqr_mont_nohw",
        "fiat_curve25519_adx_mul",
        "fiat_curve25519_adx_square",
        "gcm_ghash_avx",
        "gcm_ghash_clmul",
        "gcm_ghash_neon",
        "gcm_ghash_vpclmulqdq_avx2_1",
        "gcm_gmult_clmul",
        "gcm_gmult_neon",
        "gcm_init_avx",
        "gcm_init_clmul",
        "gcm_init_neon",
        "gcm_init_vpclmulqdq_avx2",
        "k25519Precomp",
        "limbs_mul_add_limb",
        "little_endian_bytes_from_scalar",
        "ecp_nistz256_neg",
        "ecp_nistz256_select_w5",
        "ecp_nistz256_select_w7",
        "neon_available",
        "p256_mul_mont",
        "p256_point_add",
        "p256_point_add_affine",
        "p256_point_double",
        "p256_point_mul",
        "p256_point_mul_base",
        "p256_point_mul_base_vartime",
        "p256_scalar_mul_mont",
        "p256_scalar_sqr_rep_mont",
        "p256_sqr_mont",
        "p384_elem_div_by_2",
        "p384_elem_mul_mont",
        "p384_elem_neg",
        "p384_elem_sub",
        "p384_point_add",
        "p384_point_double",
        "p384_point_mul",
        "p384_scalar_mul_mont",
        "openssl_poly1305_neon2_addmulmod",
        "openssl_poly1305_neon2_blocks",
        "sha256_block_data_order",
        "sha256_block_data_order_avx",
        "sha256_block_data_order_ssse3",
        "sha256_block_data_order_hw",
        "sha256_block_data_order_neon",
        "sha256_block_data_order_nohw",
        "sha512_block_data_order",
        "sha512_block_data_order_avx",
        "sha512_block_data_order_hw",
        "sha512_block_data_order_neon",
        "sha512_block_data_order_nohw",
        "vpaes_ctr32_encrypt_blocks",
        "vpaes_encrypt",
        "vpaes_encrypt_key_to_bsaes",
        "vpaes_set_encrypt_key",
        "x25519_NEON",
        "x25519_fe_invert",
        "x25519_fe_isnegative",
        "x25519_fe_mul_ttt",
        "x25519_fe_neg",
        "x25519_fe_tobytes",
        "x25519_ge_double_scalarmult_vartime",
        "x25519_ge_frombytes_vartime",
        "x25519_ge_scalarmult_base",
        "x25519_ge_scalarmult_base_adx",
        "x25519_public_from_private_generic_masked",
        "x25519_sc_mask",
        "x25519_sc_muladd",
        "x25519_sc_reduce",
        "x25519_scalar_mult_adx",
        "x25519_scalar_mult_generic_masked",
    ];

    let mut out = String::new();

    for (old, new) in SYMBOLS_TO_RENAME {
        let line = format!(
            "{pp}define {prefix_prefix}{old} {prefix_prefix}{new}\n",
            pp = pp,
            prefix_prefix = prefix_prefix,
            old = old,
            new = new
        );
        out += &line;
    }

    for symbol in SYMBOLS_TO_PREFIX {
        let line = format!(
            "{pp}define {prefix_prefix}{symbol} {prefix_prefix}{prefix}{symbol}\n",
            pp = pp,
            prefix_prefix = prefix_prefix,
            prefix = prefix,
            symbol = symbol
        );
        out += &line;
    }

    out
}
