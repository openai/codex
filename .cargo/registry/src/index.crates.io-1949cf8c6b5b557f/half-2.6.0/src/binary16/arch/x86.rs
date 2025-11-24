use core::{mem::MaybeUninit, ptr};

#[cfg(target_arch = "x86")]
use core::arch::x86::{
    __m128, __m128i, __m256, _mm256_cvtph_ps, _mm256_cvtps_ph, _mm_cvtph_ps,
    _MM_FROUND_TO_NEAREST_INT,
};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m128, __m128i, __m256, _mm256_cvtph_ps, _mm256_cvtps_ph, _mm_cvtph_ps, _mm_cvtps_ph,
    _MM_FROUND_TO_NEAREST_INT,
};

#[cfg(target_arch = "x86")]
use core::arch::x86::_mm_cvtps_ph;

use super::convert_chunked_slice_8;

/////////////// x86/x86_64 f16c ////////////////

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f16_to_f32_x86_f16c(i: u16) -> f32 {
    let mut vec = MaybeUninit::<__m128i>::zeroed();
    vec.as_mut_ptr().cast::<u16>().write(i);
    let retval = _mm_cvtph_ps(vec.assume_init());
    *(&retval as *const __m128).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f32_to_f16_x86_f16c(f: f32) -> u16 {
    let mut vec = MaybeUninit::<__m128>::zeroed();
    vec.as_mut_ptr().cast::<f32>().write(f);
    let retval = _mm_cvtps_ph(vec.assume_init(), _MM_FROUND_TO_NEAREST_INT);
    *(&retval as *const __m128i).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f16x4_to_f32x4_x86_f16c(v: &[u16; 4]) -> [f32; 4] {
    let mut vec = MaybeUninit::<__m128i>::zeroed();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 4);
    let retval = _mm_cvtph_ps(vec.assume_init());
    *(&retval as *const __m128).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f32x4_to_f16x4_x86_f16c(v: &[f32; 4]) -> [u16; 4] {
    let mut vec = MaybeUninit::<__m128>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 4);
    let retval = _mm_cvtps_ph(vec.assume_init(), _MM_FROUND_TO_NEAREST_INT);
    *(&retval as *const __m128i).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f16x4_to_f64x4_x86_f16c(v: &[u16; 4]) -> [f64; 4] {
    let array = f16x4_to_f32x4_x86_f16c(v);
    // Let compiler vectorize this regular cast for now.
    // TODO: investigate auto-detecting sse2/avx convert features
    [
        array[0] as f64,
        array[1] as f64,
        array[2] as f64,
        array[3] as f64,
    ]
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f64x4_to_f16x4_x86_f16c(v: &[f64; 4]) -> [u16; 4] {
    // Let compiler vectorize this regular cast for now.
    // TODO: investigate auto-detecting sse2/avx convert features
    let v = [v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32];
    f32x4_to_f16x4_x86_f16c(&v)
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f16x8_to_f32x8_x86_f16c(v: &[u16; 8]) -> [f32; 8] {
    let mut vec = MaybeUninit::<__m128i>::zeroed();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 8);
    let retval = _mm256_cvtph_ps(vec.assume_init());
    *(&retval as *const __m256).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f32x8_to_f16x8_x86_f16c(v: &[f32; 8]) -> [u16; 8] {
    let mut vec = MaybeUninit::<__m256>::uninit();
    ptr::copy_nonoverlapping(v.as_ptr(), vec.as_mut_ptr().cast(), 8);
    let retval = _mm256_cvtps_ph(vec.assume_init(), _MM_FROUND_TO_NEAREST_INT);
    *(&retval as *const __m128i).cast()
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f16x8_to_f64x8_x86_f16c(v: &[u16; 8]) -> [f64; 8] {
    let array = f16x8_to_f32x8_x86_f16c(v);
    // Let compiler vectorize this regular cast for now.
    // TODO: investigate auto-detecting sse2/avx convert features
    [
        array[0] as f64,
        array[1] as f64,
        array[2] as f64,
        array[3] as f64,
        array[4] as f64,
        array[5] as f64,
        array[6] as f64,
        array[7] as f64,
    ]
}

#[target_feature(enable = "f16c")]
#[inline]
pub(super) unsafe fn f64x8_to_f16x8_x86_f16c(v: &[f64; 8]) -> [u16; 8] {
    // Let compiler vectorize this regular cast for now.
    // TODO: investigate auto-detecting sse2/avx convert features
    let v = [
        v[0] as f32,
        v[1] as f32,
        v[2] as f32,
        v[3] as f32,
        v[4] as f32,
        v[5] as f32,
        v[6] as f32,
        v[7] as f32,
    ];
    f32x8_to_f16x8_x86_f16c(&v)
}
