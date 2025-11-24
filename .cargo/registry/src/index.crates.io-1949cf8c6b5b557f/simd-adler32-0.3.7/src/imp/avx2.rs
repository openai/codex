use super::Adler32Imp;

/// Resolves update implementation if CPU supports avx2 instructions.
pub fn get_imp() -> Option<Adler32Imp> {
  get_imp_inner()
}

#[inline]
#[cfg(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64")))]
fn get_imp_inner() -> Option<Adler32Imp> {
  if std::is_x86_feature_detected!("avx2") {
    Some(imp::update)
  } else {
    None
  }
}

#[inline]
#[cfg(all(
  target_feature = "avx2",
  not(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64")))
))]
fn get_imp_inner() -> Option<Adler32Imp> {
  Some(imp::update)
}

#[inline]
#[cfg(all(
  not(target_feature = "avx2"),
  not(all(feature = "std", any(target_arch = "x86", target_arch = "x86_64")))
))]
fn get_imp_inner() -> Option<Adler32Imp> {
  None
}

#[cfg(all(
  any(target_arch = "x86", target_arch = "x86_64"),
  any(feature = "std", target_feature = "avx2")
))]
mod imp {
  const MOD: u32 = 65521;
  const NMAX: usize = 5552;
  const BLOCK_SIZE: usize = 32;
  const CHUNK_SIZE: usize = NMAX / BLOCK_SIZE * BLOCK_SIZE;

  #[cfg(target_arch = "x86")]
  use core::arch::x86::*;
  #[cfg(target_arch = "x86_64")]
  use core::arch::x86_64::*;

  pub fn update(a: u16, b: u16, data: &[u8]) -> (u16, u16) {
    unsafe { update_imp(a, b, data) }
  }

  #[inline]
  #[target_feature(enable = "avx2")]
  unsafe fn update_imp(a: u16, b: u16, data: &[u8]) -> (u16, u16) {
    let mut a = a as u32;
    let mut b = b as u32;

    let chunks = data.chunks_exact(CHUNK_SIZE);
    let remainder = chunks.remainder();
    for chunk in chunks {
      update_chunk_block(&mut a, &mut b, chunk);
    }

    update_block(&mut a, &mut b, remainder);

    (a as u16, b as u16)
  }

  #[inline]
  unsafe fn update_chunk_block(a: &mut u32, b: &mut u32, chunk: &[u8]) {
    debug_assert_eq!(
      chunk.len(),
      CHUNK_SIZE,
      "Unexpected chunk size (expected {}, got {})",
      CHUNK_SIZE,
      chunk.len()
    );

    reduce_add_blocks(a, b, chunk);

    *a %= MOD;
    *b %= MOD;
  }

  #[inline]
  unsafe fn update_block(a: &mut u32, b: &mut u32, chunk: &[u8]) {
    debug_assert!(
      chunk.len() <= CHUNK_SIZE,
      "Unexpected chunk size (expected <= {}, got {})",
      CHUNK_SIZE,
      chunk.len()
    );

    for byte in reduce_add_blocks(a, b, chunk) {
      *a += *byte as u32;
      *b += *a;
    }

    *a %= MOD;
    *b %= MOD;
  }

  #[inline(always)]
  unsafe fn reduce_add_blocks<'a>(a: &mut u32, b: &mut u32, chunk: &'a [u8]) -> &'a [u8] {
    if chunk.len() < BLOCK_SIZE {
      return chunk;
    }

    let blocks = chunk.chunks_exact(BLOCK_SIZE);
    let blocks_remainder = blocks.remainder();

    let one_v = _mm256_set1_epi16(1);
    let zero_v = _mm256_setzero_si256();
    let weights = get_weights();

    let mut p_v = _mm256_set_epi32(0, 0, 0, 0, 0, 0, 0, (*a * blocks.len() as u32) as _);
    let mut a_v = _mm256_setzero_si256();
    let mut b_v = _mm256_set_epi32(0, 0, 0, 0, 0, 0, 0, *b as _);

    for block in blocks {
      let block_ptr = block.as_ptr() as *const _;
      let block = _mm256_loadu_si256(block_ptr);

      p_v = _mm256_add_epi32(p_v, a_v);

      a_v = _mm256_add_epi32(a_v, _mm256_sad_epu8(block, zero_v));
      let mad = _mm256_maddubs_epi16(block, weights);
      b_v = _mm256_add_epi32(b_v, _mm256_madd_epi16(mad, one_v));
    }

    b_v = _mm256_add_epi32(b_v, _mm256_slli_epi32(p_v, 5));

    *a += reduce_add(a_v);
    *b = reduce_add(b_v);

    blocks_remainder
  }

  #[inline(always)]
  unsafe fn reduce_add(v: __m256i) -> u32 {
    let sum = _mm_add_epi32(_mm256_castsi256_si128(v), _mm256_extracti128_si256(v, 1));
    let hi = _mm_unpackhi_epi64(sum, sum);

    let sum = _mm_add_epi32(hi, sum);
    let hi = _mm_shuffle_epi32(sum, crate::imp::_MM_SHUFFLE(2, 3, 0, 1));

    let sum = _mm_add_epi32(sum, hi);

    _mm_cvtsi128_si32(sum) as _
  }

  #[inline(always)]
  unsafe fn get_weights() -> __m256i {
    _mm256_set_epi8(
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
      24, 25, 26, 27, 28, 29, 30, 31, 32,
    )
  }
}

#[cfg(test)]
mod tests {
  use rand::Rng;

  #[test]
  fn zeroes() {
    assert_sum_eq(&[]);
    assert_sum_eq(&[0]);
    assert_sum_eq(&[0, 0]);
    assert_sum_eq(&[0; 100]);
    assert_sum_eq(&[0; 1024]);
    assert_sum_eq(&[0; 1024 * 1024]);
  }

  #[test]
  fn ones() {
    assert_sum_eq(&[]);
    assert_sum_eq(&[1]);
    assert_sum_eq(&[1, 1]);
    assert_sum_eq(&[1; 100]);
    assert_sum_eq(&[1; 1024]);
    assert_sum_eq(&[1; 1024 * 1024]);
  }

  #[test]
  fn random() {
    let mut random = [0; 1024 * 1024];
    rand::thread_rng().fill(&mut random[..]);

    assert_sum_eq(&random[..1]);
    assert_sum_eq(&random[..100]);
    assert_sum_eq(&random[..1024]);
    assert_sum_eq(&random[..1024 * 1024]);
  }

  /// Example calculation from https://en.wikipedia.org/wiki/Adler-32.
  #[test]
  fn wiki() {
    assert_sum_eq(b"Wikipedia");
  }

  fn assert_sum_eq(data: &[u8]) {
    if let Some(update) = super::get_imp() {
      let (a, b) = update(1, 0, data);
      let left = u32::from(b) << 16 | u32::from(a);
      let right = adler::adler32_slice(data);

      assert_eq!(left, right, "len({})", data.len());
    }
  }
}
