//! RDRAND backend for x86(-64) targets
use crate::{util::slice_as_uninit, Error};
use core::mem::{size_of, MaybeUninit};

#[path = "../lazy.rs"]
mod lazy;

#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
compile_error!("`rdrand` backend can be enabled only for x86 and x86-64 targets!");

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        use core::arch::x86_64 as arch;
        use arch::_rdrand64_step as rdrand_step;
        type Word = u64;
    } else if #[cfg(target_arch = "x86")] {
        use core::arch::x86 as arch;
        use arch::_rdrand32_step as rdrand_step;
        type Word = u32;
    }
}

static RDRAND_GOOD: lazy::LazyBool = lazy::LazyBool::new();

// Recommendation from "Intel® Digital Random Number Generator (DRNG) Software
// Implementation Guide" - Section 5.2.1 and "Intel® 64 and IA-32 Architectures
// Software Developer’s Manual" - Volume 1 - Section 7.3.17.1.
const RETRY_LIMIT: usize = 10;

#[target_feature(enable = "rdrand")]
unsafe fn rdrand() -> Option<Word> {
    for _ in 0..RETRY_LIMIT {
        let mut val = 0;
        if rdrand_step(&mut val) == 1 {
            return Some(val);
        }
    }
    None
}

// "rdrand" target feature requires "+rdrand" flag, see https://github.com/rust-lang/rust/issues/49653.
#[cfg(all(target_env = "sgx", not(target_feature = "rdrand")))]
compile_error!(
    "SGX targets require 'rdrand' target feature. Enable by using -C target-feature=+rdrand."
);

// Run a small self-test to make sure we aren't repeating values
// Adapted from Linux's test in arch/x86/kernel/cpu/rdrand.c
// Fails with probability < 2^(-90) on 32-bit systems
#[target_feature(enable = "rdrand")]
unsafe fn self_test() -> bool {
    // On AMD, RDRAND returns 0xFF...FF on failure, count it as a collision.
    let mut prev = !0; // TODO(MSRV 1.43): Move to usize::MAX
    let mut fails = 0;
    for _ in 0..8 {
        match rdrand() {
            Some(val) if val == prev => fails += 1,
            Some(val) => prev = val,
            None => return false,
        };
    }
    fails <= 2
}

fn is_rdrand_good() -> bool {
    #[cfg(not(target_feature = "rdrand"))]
    {
        // SAFETY: All Rust x86 targets are new enough to have CPUID, and we
        // check that leaf 1 is supported before using it.
        let cpuid0 = unsafe { arch::__cpuid(0) };
        if cpuid0.eax < 1 {
            return false;
        }
        let cpuid1 = unsafe { arch::__cpuid(1) };

        let vendor_id = [
            cpuid0.ebx.to_le_bytes(),
            cpuid0.edx.to_le_bytes(),
            cpuid0.ecx.to_le_bytes(),
        ];
        if vendor_id == [*b"Auth", *b"enti", *b"cAMD"] {
            let mut family = (cpuid1.eax >> 8) & 0xF;
            if family == 0xF {
                family += (cpuid1.eax >> 20) & 0xFF;
            }
            // AMD CPUs families before 17h (Zen) sometimes fail to set CF when
            // RDRAND fails after suspend. Don't use RDRAND on those families.
            // See https://bugzilla.redhat.com/show_bug.cgi?id=1150286
            if family < 0x17 {
                return false;
            }
        }

        const RDRAND_FLAG: u32 = 1 << 30;
        if cpuid1.ecx & RDRAND_FLAG == 0 {
            return false;
        }
    }

    // SAFETY: We have already checked that rdrand is available.
    unsafe { self_test() }
}

// TODO: make this function safe when we have feature(target_feature_11)
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_exact(dest: &mut [MaybeUninit<u8>]) -> Option<()> {
    // We use chunks_exact_mut instead of chunks_mut as it allows almost all
    // calls to memcpy to be elided by the compiler.
    let mut chunks = dest.chunks_exact_mut(size_of::<Word>());
    for chunk in chunks.by_ref() {
        let src = rdrand()?.to_ne_bytes();
        chunk.copy_from_slice(slice_as_uninit(&src));
    }

    let tail = chunks.into_remainder();
    let n = tail.len();
    if n > 0 {
        let src = rdrand()?.to_ne_bytes();
        tail.copy_from_slice(slice_as_uninit(&src[..n]));
    }
    Some(())
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_u32() -> Option<u32> {
    rdrand().map(crate::util::truncate)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_u64() -> Option<u64> {
    rdrand()
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_u32() -> Option<u32> {
    rdrand()
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_u64() -> Option<u64> {
    let a = rdrand()?;
    let b = rdrand()?;
    Some((u64::from(a) << 32) | u64::from(b))
}

#[inline]
pub fn inner_u32() -> Result<u32, Error> {
    if !RDRAND_GOOD.unsync_init(is_rdrand_good) {
        return Err(Error::NO_RDRAND);
    }
    // SAFETY: After this point, we know rdrand is supported.
    unsafe { rdrand_u32() }.ok_or(Error::FAILED_RDRAND)
}

#[inline]
pub fn inner_u64() -> Result<u64, Error> {
    if !RDRAND_GOOD.unsync_init(is_rdrand_good) {
        return Err(Error::NO_RDRAND);
    }
    // SAFETY: After this point, we know rdrand is supported.
    unsafe { rdrand_u64() }.ok_or(Error::FAILED_RDRAND)
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    if !RDRAND_GOOD.unsync_init(is_rdrand_good) {
        return Err(Error::NO_RDRAND);
    }
    // SAFETY: After this point, we know rdrand is supported.
    unsafe { rdrand_exact(dest) }.ok_or(Error::FAILED_RDRAND)
}

impl Error {
    /// RDRAND instruction failed due to a hardware issue.
    pub(crate) const FAILED_RDRAND: Error = Self::new_internal(10);
    /// RDRAND instruction unsupported on this target.
    pub(crate) const NO_RDRAND: Error = Self::new_internal(11);
}
