//! Platform-specific, assembly instructions to avoid
//! intermediate rounding on architectures with FPUs.
//!
//! This is adapted from the implementation in the Rust core library,
//! the original implementation can be [here](https://github.com/rust-lang/rust/blob/master/library/core/src/num/dec2flt/fpu.rs).
//!
//! It is therefore also subject to a Apache2.0/MIT license.

#![cfg(feature = "nightly")]
#![doc(hidden)]

pub use fpu_precision::set_precision;

// On x86, the x87 FPU is used for float operations if the SSE/SSE2 extensions are not available.
// The x87 FPU operates with 80 bits of precision by default, which means that operations will
// round to 80 bits causing double rounding to happen when values are eventually represented as
// 32/64 bit float values. To overcome this, the FPU control word can be set so that the
// computations are performed in the desired precision.
#[cfg(all(target_arch = "x86", not(target_feature = "sse2")))]
mod fpu_precision {
    use core::mem::size_of;

    /// A structure used to preserve the original value of the FPU control word, so that it can be
    /// restored when the structure is dropped.
    ///
    /// The x87 FPU is a 16-bits register whose fields are as follows:
    ///
    /// | 12-15 | 10-11 | 8-9 | 6-7 |  5 |  4 |  3 |  2 |  1 |  0 |
    /// |------:|------:|----:|----:|---:|---:|---:|---:|---:|---:|
    /// |       | RC    | PC  |     | PM | UM | OM | ZM | DM | IM |
    ///
    /// The documentation for all of the fields is available in the IA-32 Architectures Software
    /// Developer's Manual (Volume 1).
    ///
    /// The only field which is relevant for the following code is PC, Precision Control. This
    /// field determines the precision of the operations performed by the  FPU. It can be set to:
    ///  - 0b00, single precision i.e., 32-bits
    ///  - 0b10, double precision i.e., 64-bits
    ///  - 0b11, double extended precision i.e., 80-bits (default state)
    /// The 0b01 value is reserved and should not be used.
    pub struct FPUControlWord(u16);

    fn set_cw(cw: u16) {
        // SAFETY: the `fldcw` instruction has been audited to be able to work correctly with
        // any `u16`
        unsafe {
            asm!(
                "fldcw word ptr [{}]",
                in(reg) &cw,
                options(nostack),
            )
        }
    }

    /// Sets the precision field of the FPU to `T` and returns a `FPUControlWord`.
    pub fn set_precision<T>() -> FPUControlWord {
        let mut cw = 0_u16;

        // Compute the value for the Precision Control field that is appropriate for `T`.
        let cw_precision = match size_of::<T>() {
            4 => 0x0000, // 32 bits
            8 => 0x0200, // 64 bits
            _ => 0x0300, // default, 80 bits
        };

        // Get the original value of the control word to restore it later, when the
        // `FPUControlWord` structure is dropped
        // SAFETY: the `fnstcw` instruction has been audited to be able to work correctly with
        // any `u16`
        unsafe {
            asm!(
                "fnstcw word ptr [{}]",
                in(reg) &mut cw,
                options(nostack),
            )
        }

        // Set the control word to the desired precision. This is achieved by masking away the old
        // precision (bits 8 and 9, 0x300) and replacing it with the precision flag computed above.
        set_cw((cw & 0xFCFF) | cw_precision);

        FPUControlWord(cw)
    }

    impl Drop for FPUControlWord {
        fn drop(&mut self) {
            set_cw(self.0)
        }
    }
}

// In most architectures, floating point operations have an explicit bit size, therefore the
// precision of the computation is determined on a per-operation basis.
#[cfg(any(not(target_arch = "x86"), target_feature = "sse2"))]
mod fpu_precision {
    pub fn set_precision<T>() {
    }
}
