//! Interface to the operating system's random number generator.
//!
//! # Supported targets
//!
//! | Target            | Target Triple      | Implementation
//! | ----------------- | ------------------ | --------------
//! | Linux, Android    | `*‑linux‑*`        | [`getrandom`][1] system call if available, otherwise [`/dev/urandom`][2] after successfully polling `/dev/random`
//! | Windows           | `*‑windows‑*`      | [`BCryptGenRandom`]
//! | macOS             | `*‑apple‑darwin`   | [`getentropy`][3]
//! | iOS, tvOS, watchOS | `*‑apple‑ios`, `*-apple-tvos`, `*-apple-watchos` | [`CCRandomGenerateBytes`]
//! | FreeBSD           | `*‑freebsd`        | [`getrandom`][5]
//! | OpenBSD           | `*‑openbsd`        | [`getentropy`][7]
//! | NetBSD            | `*‑netbsd`         | [`getrandom`][16] if available, otherwise [`kern.arandom`][8]
//! | Dragonfly BSD     | `*‑dragonfly`      | [`getrandom`][9]
//! | Solaris           | `*‑solaris`        | [`getrandom`][11] (with `GRND_RANDOM`)
//! | illumos           | `*‑illumos`        | [`getrandom`][12]
//! | Fuchsia OS        | `*‑fuchsia`        | [`cprng_draw`]
//! | Redox             | `*‑redox`          | `/dev/urandom`
//! | Haiku             | `*‑haiku`          | `/dev/urandom` (identical to `/dev/random`)
//! | Hermit            | `*-hermit`         | [`sys_read_entropy`]
//! | Hurd              | `*-hurd-*`         | [`getrandom`][17]
//! | SGX               | `x86_64‑*‑sgx`     | [`RDRAND`]
//! | VxWorks           | `*‑wrs‑vxworks‑*`  | `randABytes` after checking entropy pool initialization with `randSecure`
//! | ESP-IDF           | `*‑espidf`         | [`esp_fill_random`]
//! | Emscripten        | `*‑emscripten`     | [`getentropy`][13]
//! | WASI              | `wasm32‑wasi`      | [`random_get`]
//! | Web Browser and Node.js | `wasm*‑*‑unknown` | [`Crypto.getRandomValues`] if available, then [`crypto.randomFillSync`] if on Node.js, see [WebAssembly support]
//! | SOLID             | `*-kmc-solid_*`    | `SOLID_RNG_SampleRandomBytes`
//! | Nintendo 3DS      | `*-nintendo-3ds`   | [`getrandom`][18]
//! | PS Vita           | `*-vita-*`         | [`getentropy`][13]
//! | QNX Neutrino      | `*‑nto-qnx*`       | [`/dev/urandom`][14] (identical to `/dev/random`)
//! | AIX               | `*-ibm-aix`        | [`/dev/urandom`][15]
//! | Cygwin            | `*-cygwin`         | [`getrandom`][19] (based on [`RtlGenRandom`])
//!
//! Pull Requests that add support for new targets to `getrandom` are always welcome.
//!
//! ## Unsupported targets
//!
//! By default, `getrandom` will not compile on unsupported targets, but certain
//! features allow a user to select a "fallback" implementation if no supported
//! implementation exists.
//!
//! All of the below mechanisms only affect unsupported
//! targets. Supported targets will _always_ use their supported implementations.
//! This prevents a crate from overriding a secure source of randomness
//! (either accidentally or intentionally).
//!
//! ## `/dev/urandom` fallback on Linux and Android
//!
//! On Linux targets the fallback is present only if either `target_env` is `musl`,
//! or `target_arch` is one of the following: `aarch64`, `arm`, `powerpc`, `powerpc64`,
//! `s390x`, `x86`, `x86_64`. Other supported targets [require][platform-support]
//! kernel versions which support `getrandom` system call, so fallback is not needed.
//!
//! On Android targets the fallback is present only for the following `target_arch`es:
//! `aarch64`, `arm`, `x86`, `x86_64`. Other `target_arch`es (e.g. RISC-V) require
//! sufficiently high API levels.
//!
//! The fallback can be disabled by enabling the `linux_disable_fallback` crate feature.
//! Note that doing so will bump minimum supported Linux kernel version to 3.17 and
//! Android API level to 23 (Marshmallow).
//!
//! ### RDRAND on x86
//!
//! *If the `rdrand` Cargo feature is enabled*, `getrandom` will fallback to using
//! the [`RDRAND`] instruction to get randomness on `no_std` `x86`/`x86_64`
//! targets. This feature has no effect on other CPU architectures.
//!
//! ### WebAssembly support
//!
//! This crate fully supports the
//! [`wasm32-wasi`](https://github.com/CraneStation/wasi) and
//! [`wasm32-unknown-emscripten`](https://www.hellorust.com/setup/emscripten/)
//! targets. However, the `wasm32-unknown-unknown` target (i.e. the target used
//! by `wasm-pack`) is not automatically
//! supported since, from the target name alone, we cannot deduce which
//! JavaScript interface is in use (or if JavaScript is available at all).
//!
//! Instead, *if the `js` Cargo feature is enabled*, this crate will assume
//! that you are building for an environment containing JavaScript, and will
//! call the appropriate methods. Both web browser (main window and Web Workers)
//! and Node.js environments are supported, invoking the methods
//! [described above](#supported-targets) using the [`wasm-bindgen`] toolchain.
//!
//! To enable the `js` Cargo feature, add the following to the `dependencies`
//! section in your `Cargo.toml` file:
//! ```toml
//! [dependencies]
//! getrandom = { version = "0.2", features = ["js"] }
//! ```
//!
//! This can be done even if `getrandom` is not a direct dependency. Cargo
//! allows crates to enable features for indirect dependencies.
//!
//! This feature should only be enabled for binary, test, or benchmark crates.
//! Library crates should generally not enable this feature, leaving such a
//! decision to *users* of their library. Also, libraries should not introduce
//! their own `js` features *just* to enable `getrandom`'s `js` feature.
//!
//! This feature has no effect on targets other than `wasm32-unknown-unknown`.
//!
//! #### Node.js ES module support
//!
//! Node.js supports both [CommonJS modules] and [ES modules]. Due to
//! limitations in wasm-bindgen's [`module`] support, we cannot directly
//! support ES Modules running on Node.js. However, on Node v15 and later, the
//! module author can add a simple shim to support the Web Cryptography API:
//! ```js
//! import { webcrypto } from 'node:crypto'
//! globalThis.crypto = webcrypto
//! ```
//! This crate will then use the provided `webcrypto` implementation.
//!
//! ### Platform Support
//! This crate generally supports the same operating system and platform versions
//! that the Rust standard library does. Additional targets may be supported using
//! pluggable custom implementations.
//!
//! This means that as Rust drops support for old versions of operating systems
//! (such as old Linux kernel versions, Android API levels, etc) in stable releases,
//! `getrandom` may create new patch releases (`0.N.x`) that remove support for
//! outdated platform versions.
//!
//! ### Custom implementations
//!
//! The [`register_custom_getrandom!`] macro allows a user to mark their own
//! function as the backing implementation for [`getrandom`]. See the macro's
//! documentation for more information about writing and registering your own
//! custom implementations.
//!
//! Note that registering a custom implementation only has an effect on targets
//! that would otherwise not compile. Any supported targets (including those
//! using `rdrand` and `js` Cargo features) continue using their normal
//! implementations even if a function is registered.
//!
//! ## Early boot
//!
//! Sometimes, early in the boot process, the OS has not collected enough
//! entropy to securely seed its RNG. This is especially common on virtual
//! machines, where standard "random" events are hard to come by.
//!
//! Some operating system interfaces always block until the RNG is securely
//! seeded. This can take anywhere from a few seconds to more than a minute.
//! A few (Linux, NetBSD and Solaris) offer a choice between blocking and
//! getting an error; in these cases, we always choose to block.
//!
//! On Linux (when the `getrandom` system call is not available), reading from
//! `/dev/urandom` never blocks, even when the OS hasn't collected enough
//! entropy yet. To avoid returning low-entropy bytes, we first poll
//! `/dev/random` and only switch to `/dev/urandom` once this has succeeded.
//!
//! On OpenBSD, this kind of entropy accounting isn't available, and on
//! NetBSD, blocking on it is discouraged. On these platforms, nonblocking
//! interfaces are used, even when reliable entropy may not be available.
//! On the platforms where it is used, the reliability of entropy accounting
//! itself isn't free from controversy. This library provides randomness
//! sourced according to the platform's best practices, but each platform has
//! its own limits on the grade of randomness it can promise in environments
//! with few sources of entropy.
//!
//! ## Error handling
//!
//! We always choose failure over returning known insecure "random" bytes. In
//! general, on supported platforms, failure is highly unlikely, though not
//! impossible. If an error does occur, then it is likely that it will occur
//! on every call to `getrandom`, hence after the first successful call one
//! can be reasonably confident that no errors will occur.
//!
//! [1]: https://manned.org/getrandom.2
//! [2]: https://manned.org/urandom.4
//! [3]: https://www.unix.com/man-page/mojave/2/getentropy/
//! [4]: https://www.unix.com/man-page/mojave/4/urandom/
//! [5]: https://www.freebsd.org/cgi/man.cgi?query=getrandom&manpath=FreeBSD+12.0-stable
//! [7]: https://man.openbsd.org/getentropy.2
//! [8]: https://man.netbsd.org/sysctl.7
//! [9]: https://leaf.dragonflybsd.org/cgi/web-man?command=getrandom
//! [11]: https://docs.oracle.com/cd/E88353_01/html/E37841/getrandom-2.html
//! [12]: https://illumos.org/man/2/getrandom
//! [13]: https://github.com/emscripten-core/emscripten/pull/12240
//! [14]: https://www.qnx.com/developers/docs/7.1/index.html#com.qnx.doc.neutrino.utilities/topic/r/random.html
//! [15]: https://www.ibm.com/docs/en/aix/7.3?topic=files-random-urandom-devices
//! [16]: https://man.netbsd.org/getrandom.2
//! [17]: https://www.gnu.org/software/libc/manual/html_mono/libc.html#index-getrandom
//! [18]: https://github.com/rust3ds/shim-3ds/commit/b01d2568836dea2a65d05d662f8e5f805c64389d
//! [19]: https://github.com/cygwin/cygwin/blob/main/winsup/cygwin/libc/getentropy.cc
//!
//! [`BCryptGenRandom`]: https://docs.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom
//! [`RtlGenRandom`]: https://learn.microsoft.com/en-us/windows/win32/api/ntsecapi/nf-ntsecapi-rtlgenrandom
//! [`Crypto.getRandomValues`]: https://www.w3.org/TR/WebCryptoAPI/#Crypto-method-getRandomValues
//! [`RDRAND`]: https://software.intel.com/en-us/articles/intel-digital-random-number-generator-drng-software-implementation-guide
//! [`CCRandomGenerateBytes`]: https://opensource.apple.com/source/CommonCrypto/CommonCrypto-60074/include/CommonRandom.h.auto.html
//! [`cprng_draw`]: https://fuchsia.dev/fuchsia-src/zircon/syscalls/cprng_draw
//! [`crypto.randomFillSync`]: https://nodejs.org/api/crypto.html#cryptorandomfillsyncbuffer-offset-size
//! [`esp_fill_random`]: https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/random.html#_CPPv415esp_fill_randomPv6size_t
//! [`random_get`]: https://github.com/WebAssembly/WASI/blob/main/phases/snapshot/docs.md#-random_getbuf-pointeru8-buf_len-size---errno
//! [WebAssembly support]: #webassembly-support
//! [`wasm-bindgen`]: https://github.com/rustwasm/wasm-bindgen
//! [`module`]: https://rustwasm.github.io/wasm-bindgen/reference/attributes/on-js-imports/module.html
//! [CommonJS modules]: https://nodejs.org/api/modules.html
//! [ES modules]: https://nodejs.org/api/esm.html
//! [`sys_read_entropy`]: https://github.com/hermit-os/kernel/blob/315f58ff5efc81d9bf0618af85a59963ff55f8b1/src/syscalls/entropy.rs#L47-L55
//! [platform-support]: https://doc.rust-lang.org/stable/rustc/platform-support.html

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://docs.rs/getrandom/0.2.16"
)]
#![no_std]
#![warn(rust_2018_idioms, unused_lifetimes, missing_docs)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[macro_use]
extern crate cfg_if;

use crate::util::{slice_as_uninit_mut, slice_assume_init_mut};
use core::mem::MaybeUninit;

mod error;
mod util;
// To prevent a breaking change when targets are added, we always export the
// register_custom_getrandom macro, so old Custom RNG crates continue to build.
#[cfg(feature = "custom")]
mod custom;
#[cfg(feature = "std")]
mod error_impls;

pub use crate::error::Error;

// System-specific implementations.
//
// These should all provide getrandom_inner with the signature
// `fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error>`.
// The function MUST fully initialize `dest` when `Ok(())` is returned.
// The function MUST NOT ever write uninitialized bytes into `dest`,
// regardless of what value it returns.
cfg_if! {
    if #[cfg(any(target_os = "haiku", target_os = "redox", target_os = "nto", target_os = "aix"))] {
        mod util_libc;
        #[path = "use_file.rs"] mod imp;
    } else if #[cfg(any(
        target_os = "macos",
        target_os = "openbsd",
        target_os = "vita",
        target_os = "emscripten",
    ))] {
        mod util_libc;
        #[path = "getentropy.rs"] mod imp;
    } else if #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "illumos",
        // Check for target_arch = "arm" to only include the 3DS. Does not
        // include the Nintendo Switch (which is target_arch = "aarch64").
        all(target_os = "horizon", target_arch = "arm"),
        target_os = "cygwin",
    ))] {
        mod util_libc;
        #[path = "getrandom.rs"] mod imp;
    } else if #[cfg(all(
        not(feature = "linux_disable_fallback"),
        any(
            // Rust supports Android API level 19 (KitKat) [0] and the next upgrade targets
            // level 21 (Lollipop) [1], while `getrandom(2)` was added only in
            // level 23 (Marshmallow). Note that it applies only to the "old" `target_arch`es,
            // RISC-V Android targets sufficiently new API level, same will apply for potential
            // new Android `target_arch`es.
            // [0]: https://blog.rust-lang.org/2023/01/09/android-ndk-update-r25.html
            // [1]: https://github.com/rust-lang/rust/pull/120593
            all(
                target_os = "android",
                any(
                    target_arch = "aarch64",
                    target_arch = "arm",
                    target_arch = "x86",
                    target_arch = "x86_64",
                ),
            ),
            // Only on these `target_arch`es Rust supports Linux kernel versions (3.2+)
            // that precede the version (3.17) in which `getrandom(2)` was added:
            // https://doc.rust-lang.org/stable/rustc/platform-support.html
            all(
                target_os = "linux",
                any(
                    target_arch = "aarch64",
                    target_arch = "arm",
                    target_arch = "powerpc",
                    target_arch = "powerpc64",
                    target_arch = "s390x",
                    target_arch = "x86",
                    target_arch = "x86_64",
                    // Minimum supported Linux kernel version for MUSL targets
                    // is not specified explicitly (as of Rust 1.77) and they
                    // are used in practice to target pre-3.17 kernels.
                    target_env = "musl",
                ),
            )
        ),
    ))] {
        mod util_libc;
        mod use_file;
        mod lazy;
        #[path = "linux_android_with_fallback.rs"] mod imp;
    } else if #[cfg(any(target_os = "android", target_os = "linux"))] {
        mod util_libc;
        #[path = "linux_android.rs"] mod imp;
    } else if #[cfg(target_os = "solaris")] {
        mod util_libc;
        #[path = "solaris.rs"] mod imp;
    } else if #[cfg(target_os = "netbsd")] {
        mod util_libc;
        #[path = "netbsd.rs"] mod imp;
    } else if #[cfg(target_os = "fuchsia")] {
        #[path = "fuchsia.rs"] mod imp;
    } else if #[cfg(any(target_os = "ios", target_os = "visionos", target_os = "watchos", target_os = "tvos"))] {
        #[path = "apple-other.rs"] mod imp;
    } else if #[cfg(all(target_arch = "wasm32", target_os = "wasi"))] {
        #[path = "wasi.rs"] mod imp;
    } else if #[cfg(target_os = "hermit")] {
        #[path = "hermit.rs"] mod imp;
    } else if #[cfg(target_os = "vxworks")] {
        mod util_libc;
        #[path = "vxworks.rs"] mod imp;
    } else if #[cfg(target_os = "solid_asp3")] {
        #[path = "solid.rs"] mod imp;
    } else if #[cfg(target_os = "espidf")] {
        #[path = "espidf.rs"] mod imp;
    } else if #[cfg(windows)] {
        #[path = "windows.rs"] mod imp;
    } else if #[cfg(all(target_arch = "x86_64", target_env = "sgx"))] {
        mod lazy;
        #[path = "rdrand.rs"] mod imp;
    } else if #[cfg(all(feature = "rdrand",
                        any(target_arch = "x86_64", target_arch = "x86")))] {
        mod lazy;
        #[path = "rdrand.rs"] mod imp;
    } else if #[cfg(all(feature = "js",
                        any(target_arch = "wasm32", target_arch = "wasm64"),
                        target_os = "unknown"))] {
        #[path = "js.rs"] mod imp;
    } else if #[cfg(feature = "custom")] {
        use custom as imp;
    } else if #[cfg(all(any(target_arch = "wasm32", target_arch = "wasm64"),
                        target_os = "unknown"))] {
        compile_error!("the wasm*-unknown-unknown targets are not supported by \
                        default, you may need to enable the \"js\" feature. \
                        For more information see: \
                        https://docs.rs/getrandom/#webassembly-support");
    } else {
        compile_error!("target is not supported, for more information see: \
                        https://docs.rs/getrandom/#unsupported-targets");
    }
}

/// Fill `dest` with random bytes from the system's preferred random number
/// source.
///
/// This function returns an error on any failure, including partial reads. We
/// make no guarantees regarding the contents of `dest` on error. If `dest` is
/// empty, `getrandom` immediately returns success, making no calls to the
/// underlying operating system.
///
/// Blocking is possible, at least during early boot; see module documentation.
///
/// In general, `getrandom` will be fast enough for interactive usage, though
/// significantly slower than a user-space CSPRNG; for the latter consider
/// [`rand::thread_rng`](https://docs.rs/rand/*/rand/fn.thread_rng.html).
#[inline]
pub fn getrandom(dest: &mut [u8]) -> Result<(), Error> {
    // SAFETY: The `&mut MaybeUninit<_>` reference doesn't escape, and
    // `getrandom_uninit` guarantees it will never de-initialize any part of
    // `dest`.
    getrandom_uninit(unsafe { slice_as_uninit_mut(dest) })?;
    Ok(())
}

/// Version of the `getrandom` function which fills `dest` with random bytes
/// returns a mutable reference to those bytes.
///
/// On successful completion this function is guaranteed to return a slice
/// which points to the same memory as `dest` and has the same length.
/// In other words, it's safe to assume that `dest` is initialized after
/// this function has returned `Ok`.
///
/// No part of `dest` will ever be de-initialized at any point, regardless
/// of what is returned.
///
/// # Examples
///
/// ```ignore
/// # // We ignore this test since `uninit_array` is unstable.
/// #![feature(maybe_uninit_uninit_array)]
/// # fn main() -> Result<(), getrandom::Error> {
/// let mut buf = core::mem::MaybeUninit::uninit_array::<1024>();
/// let buf: &mut [u8] = getrandom::getrandom_uninit(&mut buf)?;
/// # Ok(()) }
/// ```
#[inline]
pub fn getrandom_uninit(dest: &mut [MaybeUninit<u8>]) -> Result<&mut [u8], Error> {
    if !dest.is_empty() {
        imp::getrandom_inner(dest)?;
    }
    // SAFETY: `dest` has been fully initialized by `imp::getrandom_inner`
    // since it returned `Ok`.
    Ok(unsafe { slice_assume_init_mut(dest) })
}
