//! RNDR register backend for aarch64 targets
//!
//! Arm Architecture Reference Manual for A-profile architecture:
//! ARM DDI 0487K.a, ID032224, D23.2.147 RNDR, Random Number
use crate::{
    util::{slice_as_uninit, truncate},
    Error,
};
use core::arch::asm;
use core::mem::{size_of, MaybeUninit};

#[cfg(not(target_arch = "aarch64"))]
compile_error!("the `rndr` backend can be enabled only for AArch64 targets!");

const RETRY_LIMIT: usize = 5;

/// Read a random number from the aarch64 RNDR register
///
/// Callers must ensure that FEAT_RNG is available on the system
/// The function assumes that the RNDR register is available
/// If it fails to read a random number, it will retry up to 5 times
/// After 5 failed reads the function will return `None`
#[target_feature(enable = "rand")]
unsafe fn rndr() -> Option<u64> {
    for _ in 0..RETRY_LIMIT {
        let mut x: u64;
        let mut nzcv: u64;

        // AArch64 RNDR register is accessible by s3_3_c2_c4_0
        asm!(
            "mrs {x}, RNDR",
            "mrs {nzcv}, NZCV",
            x = out(reg) x,
            nzcv = out(reg) nzcv,
        );

        // If the hardware returns a genuine random number, PSTATE.NZCV is set to 0b0000
        if nzcv == 0 {
            return Some(x);
        }
    }

    None
}

#[target_feature(enable = "rand")]
unsafe fn rndr_fill(dest: &mut [MaybeUninit<u8>]) -> Option<()> {
    let mut chunks = dest.chunks_exact_mut(size_of::<u64>());
    for chunk in chunks.by_ref() {
        let src = rndr()?.to_ne_bytes();
        chunk.copy_from_slice(slice_as_uninit(&src));
    }

    let tail = chunks.into_remainder();
    let n = tail.len();
    if n > 0 {
        let src = rndr()?.to_ne_bytes();
        tail.copy_from_slice(slice_as_uninit(&src[..n]));
    }
    Some(())
}

#[cfg(target_feature = "rand")]
fn is_rndr_available() -> bool {
    true
}

#[cfg(not(target_feature = "rand"))]
fn is_rndr_available() -> bool {
    #[path = "../lazy.rs"]
    mod lazy;
    static RNDR_GOOD: lazy::LazyBool = lazy::LazyBool::new();

    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            extern crate std;
            RNDR_GOOD.unsync_init(|| std::arch::is_aarch64_feature_detected!("rand"))
        } else if #[cfg(target_os = "linux")] {
            /// Check whether FEAT_RNG is available on the system
            ///
            /// Requires the caller either be running in EL1 or be on a system supporting MRS
            /// emulation. Due to the above, the implementation is currently restricted to Linux.
            ///
            /// Relying on runtime detection bumps minimum supported Linux kernel version to 4.11.
            fn mrs_check() -> bool {
                let mut id_aa64isar0: u64;

                // If FEAT_RNG is implemented, ID_AA64ISAR0_EL1.RNDR (bits 60-63) are 0b0001
                // This is okay to do from EL0 in Linux because Linux will emulate MRS as per
                // https://docs.kernel.org/arch/arm64/cpu-feature-registers.html
                unsafe {
                    asm!(
                        "mrs {id}, ID_AA64ISAR0_EL1",
                        id = out(reg) id_aa64isar0,
                    );
                }

                (id_aa64isar0 >> 60) & 0xf >= 1
            }

            RNDR_GOOD.unsync_init(mrs_check)
        } else {
            compile_error!(
                "RNDR `no_std` runtime detection is currently supported only on Linux targets. \
                Either enable the `std` crate feature, or `rand` target feature at compile time."
            );
        }
    }
}

#[inline]
pub fn inner_u32() -> Result<u32, Error> {
    if !is_rndr_available() {
        return Err(Error::RNDR_NOT_AVAILABLE);
    }
    // SAFETY: after this point, we know the `rand` target feature is enabled
    let res = unsafe { rndr() };
    res.map(truncate).ok_or(Error::RNDR_FAILURE)
}

#[inline]
pub fn inner_u64() -> Result<u64, Error> {
    if !is_rndr_available() {
        return Err(Error::RNDR_NOT_AVAILABLE);
    }
    // SAFETY: after this point, we know the `rand` target feature is enabled
    let res = unsafe { rndr() };
    res.ok_or(Error::RNDR_FAILURE)
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    if !is_rndr_available() {
        return Err(Error::RNDR_NOT_AVAILABLE);
    }
    // SAFETY: after this point, we know the `rand` target feature is enabled
    unsafe { rndr_fill(dest).ok_or(Error::RNDR_FAILURE) }
}

impl Error {
    /// RNDR register read failed due to a hardware issue.
    pub(crate) const RNDR_FAILURE: Error = Self::new_internal(10);
    /// RNDR register is not supported on this target.
    pub(crate) const RNDR_NOT_AVAILABLE: Error = Self::new_internal(11);
}
