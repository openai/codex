//! x86/x86-64 CPU feature detection support.
//!
//! Portable, `no_std`-friendly implementation that relies on the x86 `CPUID`
//! instruction for feature detection.

/// Evaluate the given `$body` expression any of the supplied target features
/// are not enabled. Otherwise returns true.
///
/// The `$body` expression is not evaluated on SGX targets, and returns false
/// on these targets unless *all* supplied target features are enabled.
#[macro_export]
#[doc(hidden)]
macro_rules! __unless_target_features {
    ($($tf:tt),+ => $body:expr ) => {{
        #[cfg(not(all($(target_feature=$tf,)*)))]
        {
            #[cfg(not(any(target_env = "sgx", target_os = "none", target_os = "uefi")))]
            $body

            // CPUID is not available on SGX. Freestanding and UEFI targets
            // do not support SIMD features with default compilation flags.
            #[cfg(any(target_env = "sgx", target_os = "none", target_os = "uefi"))]
            false
        }

        #[cfg(all($(target_feature=$tf,)*))]
        true
    }};
}

/// Use CPUID to detect the presence of all supplied target features.
#[macro_export]
#[doc(hidden)]
macro_rules! __detect_target_features {
    ($($tf:tt),+) => {{
        #[cfg(target_arch = "x86")]
        use core::arch::x86::{__cpuid, __cpuid_count, CpuidResult};
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::{__cpuid, __cpuid_count, CpuidResult};

        // These wrappers are workarounds around
        // https://github.com/rust-lang/rust/issues/101346
        //
        // DO NOT remove it until MSRV is bumped to a version
        // with the issue fix (at least 1.64).
        #[inline(never)]
        unsafe fn cpuid(leaf: u32) -> CpuidResult {
            __cpuid(leaf)
        }

        #[inline(never)]
        unsafe fn cpuid_count(leaf: u32, sub_leaf: u32) -> CpuidResult {
            __cpuid_count(leaf, sub_leaf)
        }

        let cr = unsafe {
            [cpuid(1), cpuid_count(7, 0)]
        };

        $($crate::check!(cr, $tf) & )+ true
    }};
}

/// Check that OS supports required SIMD registers
#[macro_export]
#[doc(hidden)]
macro_rules! __xgetbv {
    ($cr:expr, $mask:expr) => {{
        #[cfg(target_arch = "x86")]
        use core::arch::x86 as arch;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64 as arch;

        // Check bits 26 and 27
        let xmask = 0b11 << 26;
        let xsave = $cr[0].ecx & xmask == xmask;
        if xsave {
            let xcr0 = unsafe { arch::_xgetbv(arch::_XCR_XFEATURE_ENABLED_MASK) };
            (xcr0 & $mask) == $mask
        } else {
            false
        }
    }};
}

macro_rules! __expand_check_macro {
    ($(($name:tt, $reg_cap:tt $(, $i:expr, $reg:ident, $offset:expr)*)),* $(,)?) => {
        #[macro_export]
        #[doc(hidden)]
        macro_rules! check {
            $(
                ($cr:expr, $name) => {{
                    // Register bits are listed here:
                    // https://wiki.osdev.org/CPU_Registers_x86#Extended_Control_Registers
                    let reg_cap = match $reg_cap {
                        // Bit 1
                        "xmm" => $crate::__xgetbv!($cr, 0b10),
                        // Bits 1 and 2
                        "ymm" => $crate::__xgetbv!($cr, 0b110),
                        // Bits 1, 2, 5, 6, and 7
                        "zmm" => $crate::__xgetbv!($cr, 0b1110_0110),
                        _ => true,
                    };
                    reg_cap
                    $(
                        & ($cr[$i].$reg & (1 << $offset) != 0)
                    )*
                }};
            )*
        }
    };
}

__expand_check_macro! {
    ("sse3", "", 0, ecx, 0),
    ("pclmulqdq", "", 0, ecx, 1),
    ("ssse3", "", 0, ecx, 9),
    ("fma", "ymm", 0, ecx, 12, 0, ecx, 28),
    ("sse4.1", "", 0, ecx, 19),
    ("sse4.2", "", 0, ecx, 20),
    ("popcnt", "", 0, ecx, 23),
    ("aes", "", 0, ecx, 25),
    ("avx", "xmm", 0, ecx, 28),
    ("rdrand", "", 0, ecx, 30),

    ("mmx", "", 0, edx, 23),
    ("sse", "", 0, edx, 25),
    ("sse2", "", 0, edx, 26),

    ("sgx", "", 1, ebx, 2),
    ("bmi1", "", 1, ebx, 3),
    ("bmi2", "", 1, ebx, 8),
    ("avx2", "ymm", 1, ebx, 5, 0, ecx, 28),
    ("avx512f", "zmm", 1, ebx, 16),
    ("avx512dq", "zmm", 1, ebx, 17),
    ("rdseed", "", 1, ebx, 18),
    ("adx", "", 1, ebx, 19),
    ("avx512ifma", "zmm", 1, ebx, 21),
    ("avx512pf", "zmm", 1, ebx, 26),
    ("avx512er", "zmm", 1, ebx, 27),
    ("avx512cd", "zmm", 1, ebx, 28),
    ("sha", "", 1, ebx, 29),
    ("avx512bw", "zmm", 1, ebx, 30),
    ("avx512vl", "zmm", 1, ebx, 31),
    ("avx512vbmi", "zmm", 1, ecx, 1),
    ("avx512vbmi2", "zmm", 1, ecx, 6),
    ("gfni", "zmm", 1, ecx, 8),
    ("vaes", "zmm", 1, ecx, 9),
    ("vpclmulqdq", "zmm", 1, ecx, 10),
    ("avx512bitalg", "zmm", 1, ecx, 12),
    ("avx512vpopcntdq", "zmm", 1, ecx, 14),
}
