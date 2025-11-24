//! ARM64 CPU feature detection support.
//!
//! Unfortunately ARM instructions to detect CPU features cannot be called from
//! unprivileged userspace code, so this implementation relies on OS-specific
//! APIs for feature detection.

// Evaluate the given `$body` expression any of the supplied target features
// are not enabled. Otherwise returns true.
#[macro_export]
#[doc(hidden)]
macro_rules! __unless_target_features {
    ($($tf:tt),+ => $body:expr ) => {
        {
            #[cfg(not(all($(target_feature=$tf,)*)))]
            $body

            #[cfg(all($(target_feature=$tf,)*))]
            true
        }
    };
}

// Linux runtime detection of target CPU features using `getauxval`.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[macro_export]
#[doc(hidden)]
macro_rules! __detect_target_features {
    ($($tf:tt),+) => {{
        let hwcaps = $crate::aarch64::getauxval_hwcap();
        $($crate::check!(hwcaps, $tf) & )+ true
    }};
}

/// Linux helper function for calling `getauxval` to get `AT_HWCAP`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn getauxval_hwcap() -> u64 {
    unsafe { libc::getauxval(libc::AT_HWCAP) }
}

// Apple platform's runtime detection of target CPU features using `sysctlbyname`.
#[cfg(target_vendor = "apple")]
#[macro_export]
#[doc(hidden)]
macro_rules! __detect_target_features {
    ($($tf:tt),+) => {{
        $($crate::check!($tf) & )+ true
    }};
}

// Linux `expand_check_macro`
#[cfg(any(target_os = "linux", target_os = "android"))]
macro_rules! __expand_check_macro {
    ($(($name:tt, $hwcap:ident)),* $(,)?) => {
        #[macro_export]
        #[doc(hidden)]
        macro_rules! check {
            $(
                ($hwcaps:expr, $name) => {
                    (($hwcaps & $crate::aarch64::hwcaps::$hwcap) != 0)
                };
            )*
        }
    };
}

// Linux `expand_check_macro`
#[cfg(any(target_os = "linux", target_os = "android"))]
__expand_check_macro! {
    ("aes",    AES),    // Enable AES support.
    ("dit",    DIT),    // Enable DIT support.
    ("sha2",   SHA2),   // Enable SHA1 and SHA256 support.
    ("sha3",   SHA3),   // Enable SHA512 and SHA3 support.
    ("sm4",    SM4),    // Enable SM3 and SM4 support.
}

/// Linux hardware capabilities mapped to target features.
///
/// Note that LLVM target features are coarser grained than what Linux supports
/// and imply more capabilities under each feature. This module attempts to
/// provide that mapping accordingly.
///
/// See this issue for more info: <https://github.com/RustCrypto/utils/issues/395>
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod hwcaps {
    use libc::c_ulong;

    pub const AES: c_ulong = libc::HWCAP_AES | libc::HWCAP_PMULL;
    pub const DIT: c_ulong = libc::HWCAP_DIT;
    pub const SHA2: c_ulong = libc::HWCAP_SHA2;
    pub const SHA3: c_ulong = libc::HWCAP_SHA3 | libc::HWCAP_SHA512;
    pub const SM4: c_ulong = libc::HWCAP_SM3 | libc::HWCAP_SM4;
}

// Apple OS (macOS, iOS, watchOS, and tvOS) `check!` macro.
//
// NOTE: several of these instructions (e.g. `aes`, `sha2`) can be assumed to
// be present on all Apple ARM64 hardware.
//
// Newer CPU instructions now have nodes within sysctl's `hw.optional`
// namespace, however the ones that do not can safely be assumed to be
// present on all Apple ARM64 devices, now and for the foreseeable future.
//
// See discussion on this issue for more information:
// <https://github.com/RustCrypto/utils/issues/378>
#[cfg(target_vendor = "apple")]
#[macro_export]
#[doc(hidden)]
macro_rules! check {
    ("aes") => {
        true
    };
    ("dit") => {
        // https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms#Enable-DIT-for-constant-time-cryptographic-operations
        unsafe {
            $crate::aarch64::sysctlbyname(b"hw.optional.arm.FEAT_DIT\0")
        }
    };
    ("sha2") => {
        true
    };
    ("sha3") => {
        unsafe {
            // `sha3` target feature implies SHA-512 as well
            $crate::aarch64::sysctlbyname(b"hw.optional.armv8_2_sha512\0")
                && $crate::aarch64::sysctlbyname(b"hw.optional.armv8_2_sha3\0")
        }
    };
    ("sm4") => {
        false
    };
}

/// Apple helper function for calling `sysctlbyname`.
#[cfg(target_vendor = "apple")]
pub unsafe fn sysctlbyname(name: &[u8]) -> bool {
    assert_eq!(
        name.last().cloned(),
        Some(0),
        "name is not NUL terminated: {:?}",
        name
    );

    let mut value: u32 = 0;
    let mut size = core::mem::size_of::<u32>();

    let rc = libc::sysctlbyname(
        name.as_ptr() as *const i8,
        &mut value as *mut _ as *mut libc::c_void,
        &mut size,
        core::ptr::null_mut(),
        0,
    );

    assert_eq!(size, 4, "unexpected sysctlbyname(3) result size");
    assert_eq!(rc, 0, "sysctlbyname returned error code: {}", rc);
    value != 0
}

// On other targets, runtime CPU feature detection is unavailable
#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android",)))]
#[macro_export]
#[doc(hidden)]
macro_rules! __detect_target_features {
    ($($tf:tt),+) => {
        false
    };
}
