use core::{
    arch::{
        aarch64::{float32x4_t, float64x2_t, uint16x4_t},
        asm,
    },
    mem::MaybeUninit,
    ptr,
};

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f16_to_f32_fp16(i: u16) -> f32 {
    let result: f32;
    asm!(
        "fcvt {0:s}, {1:h}",
        out(vreg) result,
        in(vreg) i,
        options(pure, nomem, nostack, preserves_flags));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f16_to_f64_fp16(i: u16) -> f64 {
    let result: f64;
    asm!(
        "fcvt {0:d}, {1:h}",
        out(vreg) result,
        in(vreg) i,
        options(pure, nomem, nostack, preserves_flags));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f32_to_f16_fp16(f: f32) -> u16 {
    let result: u16;
    asm!(
        "fcvt {0:h}, {1:s}",
        out(vreg) result,
        in(vreg) f,
        options(pure, nomem, nostack, preserves_flags));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f64_to_f16_fp16(f: f64) -> u16 {
    let result: u16;
    asm!(
        "fcvt {0:h}, {1:d}",
        out(vreg) result,
        in(vreg) f,
        options(pure, nomem, nostack, preserves_flags));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f16x4_to_f32x4_fp16(v: &[u16; 4]) -> [f32; 4] {
    let mut vec = MaybeUninit::<uint16x4_t>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 4);
    let result: float32x4_t;
    asm!(
        "fcvtl {0:v}.4s, {1:v}.4h",
        out(vreg) result,
        in(vreg) vec.assume_init(),
        options(pure, nomem, nostack));
    *(&result as *const float32x4_t).cast()
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f32x4_to_f16x4_fp16(v: &[f32; 4]) -> [u16; 4] {
    let mut vec = MaybeUninit::<float32x4_t>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 4);
    let result: uint16x4_t;
    asm!(
        "fcvtn {0:v}.4h, {1:v}.4s",
        out(vreg) result,
        in(vreg) vec.assume_init(),
        options(pure, nomem, nostack));
    *(&result as *const uint16x4_t).cast()
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f16x4_to_f64x4_fp16(v: &[u16; 4]) -> [f64; 4] {
    let mut vec = MaybeUninit::<uint16x4_t>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 4);
    let low: float64x2_t;
    let high: float64x2_t;
    asm!(
        "fcvtl {2:v}.4s, {3:v}.4h", // Convert to f32
        "fcvtl {0:v}.2d, {2:v}.2s", // Convert low part to f64
        "fcvtl2 {1:v}.2d, {2:v}.4s", // Convert high part to f64
        lateout(vreg) low,
        lateout(vreg) high,
        out(vreg) _,
        in(vreg) vec.assume_init(),
        options(pure, nomem, nostack));
    *[low, high].as_ptr().cast()
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn f64x4_to_f16x4_fp16(v: &[f64; 4]) -> [u16; 4] {
    let mut low = MaybeUninit::<float64x2_t>::uninit();
    let mut high = MaybeUninit::<float64x2_t>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), low.as_mut_ptr().cast(), 2);
    ptr::copy_nonoverlapping(v[2..].as_ptr(), high.as_mut_ptr().cast(), 2);
    let result: uint16x4_t;
    asm!(
        "fcvtn {1:v}.2s, {2:v}.2d", // Convert low to f32
        "fcvtn2 {1:v}.4s, {3:v}.2d", // Convert high to f32
        "fcvtn {0:v}.4h, {1:v}.4s", // Convert to f16
        lateout(vreg) result,
        out(vreg) _,
        in(vreg) low.assume_init(),
        in(vreg) high.assume_init(),
        options(pure, nomem, nostack));
    *(&result as *const uint16x4_t).cast()
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn add_f16_fp16(a: u16, b: u16) -> u16 {
    let result: u16;
    asm!(
        "fadd {0:h}, {1:h}, {2:h}",
        out(vreg) result,
        in(vreg) a,
        in(vreg) b,
        options(pure, nomem, nostack));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn subtract_f16_fp16(a: u16, b: u16) -> u16 {
    let result: u16;
    asm!(
        "fsub {0:h}, {1:h}, {2:h}",
        out(vreg) result,
        in(vreg) a,
        in(vreg) b,
        options(pure, nomem, nostack));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn multiply_f16_fp16(a: u16, b: u16) -> u16 {
    let result: u16;
    asm!(
        "fmul {0:h}, {1:h}, {2:h}",
        out(vreg) result,
        in(vreg) a,
        in(vreg) b,
        options(pure, nomem, nostack));
    result
}

#[target_feature(enable = "fp16")]
#[inline]
pub(super) unsafe fn divide_f16_fp16(a: u16, b: u16) -> u16 {
    let result: u16;
    asm!(
        "fdiv {0:h}, {1:h}, {2:h}",
        out(vreg) result,
        in(vreg) a,
        in(vreg) b,
        options(pure, nomem, nostack));
    result
}
