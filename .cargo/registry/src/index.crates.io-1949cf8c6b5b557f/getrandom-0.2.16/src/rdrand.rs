//! RDRAND backend for x86(-64) targets
use crate::{lazy::LazyBool, util::slice_as_uninit, Error};
use core::mem::{size_of, MaybeUninit};

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        use core::arch::x86_64 as arch;
        use arch::_rdrand64_step as rdrand_step;
    } else if #[cfg(target_arch = "x86")] {
        use core::arch::x86 as arch;
        use arch::_rdrand32_step as rdrand_step;
    }
}

// Recommendation from "Intel® Digital Random Number Generator (DRNG) Software
// Implementation Guide" - Section 5.2.1 and "Intel® 64 and IA-32 Architectures
// Software Developer’s Manual" - Volume 1 - Section 7.3.17.1.
const RETRY_LIMIT: usize = 10;

#[target_feature(enable = "rdrand")]
unsafe fn rdrand() -> Option<usize> {
    for _ in 0..RETRY_LIMIT {
        let mut val = 0;
        if rdrand_step(&mut val) == 1 {
            return Some(val as usize);
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

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    static RDRAND_GOOD: LazyBool = LazyBool::new();
    if !RDRAND_GOOD.unsync_init(is_rdrand_good) {
        return Err(Error::NO_RDRAND);
    }
    // SAFETY: After this point, we know rdrand is supported.
    unsafe { rdrand_exact(dest) }.ok_or(Error::FAILED_RDRAND)
}

// TODO: make this function safe when we have feature(target_feature_11)
#[target_feature(enable = "rdrand")]
unsafe fn rdrand_exact(dest: &mut [MaybeUninit<u8>]) -> Option<()> {
    // We use chunks_exact_mut instead of chunks_mut as it allows almost all
    // calls to memcpy to be elided by the compiler.
    let mut chunks = dest.chunks_exact_mut(size_of::<usize>());
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
