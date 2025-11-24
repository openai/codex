//! Implementation for Windows 10 and later
//!
//! On Windows 10 and later, ProcessPrng "is the primary interface to the
//! user-mode per-processor PRNGs" and only requires bcryptprimitives.dll,
//! making it a better option than the other Windows RNG APIs:
//!   - BCryptGenRandom: https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom
//!     - Requires bcrypt.dll (which loads bcryptprimitives.dll anyway)
//!     - Can cause crashes/hangs as BCrypt accesses the Windows Registry:
//!       https://github.com/rust-lang/rust/issues/99341
//!     - Causes issues inside sandboxed code:
//!       https://issues.chromium.org/issues/40277768
//!   - CryptGenRandom: https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-cryptgenrandom
//!     - Deprecated and not available on UWP targets
//!     - Requires advapi32.lib/advapi32.dll (in addition to bcryptprimitives.dll)
//!     - Thin wrapper around ProcessPrng
//!   - RtlGenRandom: https://learn.microsoft.com/en-us/windows/win32/api/ntsecapi/nf-ntsecapi-rtlgenrandom
//!     - Deprecated and not available on UWP targets
//!     - Requires advapi32.dll (in addition to bcryptprimitives.dll)
//!     - Requires using name "SystemFunction036"
//!     - Thin wrapper around ProcessPrng
//!
//! For more information see the Windows RNG Whitepaper: https://aka.ms/win10rng
use crate::Error;
use core::mem::MaybeUninit;

pub use crate::util::{inner_u32, inner_u64};

// Binding to the Windows.Win32.Security.Cryptography.ProcessPrng API. As
// bcryptprimitives.dll lacks an import library, we use "raw-dylib". This
// was added in Rust 1.65 for x86_64/aarch64 and in Rust 1.71 for x86.
// We don't need MSRV 1.71, as we only use this backend on Rust 1.78 and later.
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "bcryptprimitives",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
#[cfg_attr(
    not(target_arch = "x86"),
    link(name = "bcryptprimitives", kind = "raw-dylib")
)]
extern "system" {
    fn ProcessPrng(pbdata: *mut u8, cbdata: usize) -> BOOL;
}
#[allow(clippy::upper_case_acronyms)]
type BOOL = core::ffi::c_int; // MSRV 1.64, similarly OK for this backend.
const TRUE: BOOL = 1;

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let result = unsafe { ProcessPrng(dest.as_mut_ptr().cast::<u8>(), dest.len()) };
    // Since Windows 10, calls to the user-mode RNG are guaranteed to never
    // fail during runtime (rare windows W); `ProcessPrng` will only ever
    // return 1 (which is how windows represents TRUE).
    // See the bottom of page 6 of the aforementioned Windows RNG
    // whitepaper for more information.
    debug_assert!(result == TRUE);
    Ok(())
}
