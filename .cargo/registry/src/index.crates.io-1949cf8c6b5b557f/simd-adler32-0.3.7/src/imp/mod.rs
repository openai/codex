pub mod avx2;
pub mod avx512;
pub mod scalar;
pub mod sse2;
pub mod ssse3;
pub mod wasm;

pub type Adler32Imp = fn(u16, u16, &[u8]) -> (u16, u16);

#[inline]
#[allow(non_snake_case)]
pub const fn _MM_SHUFFLE(z: u32, y: u32, x: u32, w: u32) -> i32 {
  ((z << 6) | (y << 4) | (x << 2) | w) as i32
}

pub fn get_imp() -> Adler32Imp {
  avx512::get_imp()
    .or_else(avx2::get_imp)
    .or_else(ssse3::get_imp)
    .or_else(sse2::get_imp)
    .or_else(wasm::get_imp)
    .unwrap_or(scalar::update)
}
