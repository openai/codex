/* Copyright (c) 2019, Google Inc.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
 * SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */

#include <ring-core/aes.h>

#include "../../internal.h"

// This file contains a constant-time implementation of AES, bitsliced with
// 32-bit or 64-bit, operating on two-, four-, and eight-block
// batches, respectively.
//
// This implementation is based on the algorithms described in the following
// references:
// - https://bearssl.org/constanttime.html#aes
// - https://eprint.iacr.org/2009/129.pdf
// - https://eprint.iacr.org/2009/191.pdf


// Word operations.
//
// An aes_word_t is the word used for this AES implementation. Throughout this
// file, bits and bytes are ordered little-endian, though "left" and "right"
// shifts match the operations themselves, which makes them reversed in a
// little-endian, left-to-right reading.
//
// Eight |aes_word_t|s contain |AES_NOHW_BATCH_SIZE| blocks. The bits in an
// |aes_word_t| are divided into 16 consecutive groups of |AES_NOHW_BATCH_SIZE|
// bits each, each corresponding to a byte in an AES block in column-major
// order (AES's byte order). We refer to these as "logical bytes". Note, in the
// 32-bit and 64-bit implementations, they are smaller than a byte. (The
// contents of a logical byte will be described later.)
//
// MSVC does not support C bit operators on |__m128i|, so the wrapper functions
// |aes_nohw_and|, etc., should be used instead. Note |aes_nohw_shift_left| and
// |aes_nohw_shift_right| measure the shift in logical bytes. That is, the shift
// value ranges from 0 to 15 independent of |aes_word_t| and
// |AES_NOHW_BATCH_SIZE|.
//
// This ordering is different from https://eprint.iacr.org/2009/129.pdf, which
// uses row-major order. Matching the AES order was easier to reason about, and
// we do not have PSHUFB available to arbitrarily permute bytes.

#if defined(OPENSSL_64_BIT)
typedef uint64_t aes_word_t;
#define AES_NOHW_WORD_SIZE 8
#define AES_NOHW_BATCH_SIZE 4
#define AES_NOHW_ROW0_MASK UINT64_C(0x000f000f000f000f)
#define AES_NOHW_ROW1_MASK UINT64_C(0x00f000f000f000f0)
#define AES_NOHW_ROW2_MASK UINT64_C(0x0f000f000f000f00)
#define AES_NOHW_ROW3_MASK UINT64_C(0xf000f000f000f000)
#else  // !OPENSSL_64_BIT
typedef uint32_t aes_word_t;
#define AES_NOHW_WORD_SIZE 4
#define AES_NOHW_BATCH_SIZE 2
#define AES_NOHW_ROW0_MASK 0x03030303
#define AES_NOHW_ROW1_MASK 0x0c0c0c0c
#define AES_NOHW_ROW2_MASK 0x30303030
#define AES_NOHW_ROW3_MASK 0xc0c0c0c0
#endif  // OPENSSL_64_BIT

static inline aes_word_t aes_nohw_and(aes_word_t a, aes_word_t b) {
  return a & b;
}

static inline aes_word_t aes_nohw_or(aes_word_t a, aes_word_t b) {
  return a | b;
}

static inline aes_word_t aes_nohw_xor(aes_word_t a, aes_word_t b) {
  return a ^ b;
}

static inline aes_word_t aes_nohw_not(aes_word_t a) { return ~a; }

static inline aes_word_t aes_nohw_shift_left(aes_word_t a, aes_word_t i) {
  return a << (i * AES_NOHW_BATCH_SIZE);
}

static inline aes_word_t aes_nohw_shift_right(aes_word_t a, aes_word_t i) {
  return a >> (i * AES_NOHW_BATCH_SIZE);
}

OPENSSL_STATIC_ASSERT(AES_NOHW_BATCH_SIZE * 128 == 8 * 8 * sizeof(aes_word_t),
                      "batch size does not match word size");
OPENSSL_STATIC_ASSERT(AES_NOHW_WORD_SIZE == sizeof(aes_word_t),
                      "AES_NOHW_WORD_SIZE is incorrect");


// Block representations.
//
// This implementation uses three representations for AES blocks. First, the
// public API represents blocks as uint8_t[16] in the usual way. Second, most
// AES steps are evaluated in bitsliced form, stored in an |AES_NOHW_BATCH|.
// This stores |AES_NOHW_BATCH_SIZE| blocks in bitsliced order. For 64-bit words
// containing bitsliced blocks a, b, c, d, this would be as follows (vertical
// bars divide logical bytes):
//
//   batch.w[0] = a0 b0 c0 d0 |  a8  b8  c8  d8 | a16 b16 c16 d16 ...
//   batch.w[1] = a1 b1 c1 d1 |  a9  b9  c9  d9 | a17 b17 c17 d17 ...
//   batch.w[2] = a2 b2 c2 d2 | a10 b10 c10 d10 | a18 b18 c18 d18 ...
//   batch.w[3] = a3 b3 c3 d3 | a11 b11 c11 d11 | a19 b19 c19 d19 ...
//   ...
//
// Finally, an individual block may be stored as an intermediate form in an
// aes_word_t[AES_NOHW_BLOCK_WORDS]. In this form, we permute the bits in each
// block, so that block[0]'s ith logical byte contains least-significant
// |AES_NOHW_BATCH_SIZE| bits of byte i, block[1] contains the next group of
// |AES_NOHW_BATCH_SIZE| bits, and so on. We refer to this transformation as
// "compacting" the block. Note this is no-op with 128-bit words because then
// |AES_NOHW_BLOCK_WORDS| is one and |AES_NOHW_BATCH_SIZE| is eight. For 64-bit
// words, one block would be stored in two words:
//
//   block[0] = a0 a1 a2 a3 |  a8  a9 a10 a11 | a16 a17 a18 a19 ...
//   block[1] = a4 a5 a6 a7 | a12 a13 a14 a15 | a20 a21 a22 a23 ...
//
// Observe that the distances between corresponding bits in bitsliced and
// compact bit orders match. If we line up corresponding words of each block,
// the bitsliced and compact representations may be converted by tranposing bits
// in corresponding logical bytes. Continuing the 64-bit example:
//
//   block_a[0] = a0 a1 a2 a3 |  a8  a9 a10 a11 | a16 a17 a18 a19 ...
//   block_b[0] = b0 b1 b2 b3 |  b8  b9 b10 b11 | b16 b17 b18 b19 ...
//   block_c[0] = c0 c1 c2 c3 |  c8  c9 c10 c11 | c16 c17 c18 c19 ...
//   block_d[0] = d0 d1 d2 d3 |  d8  d9 d10 d11 | d16 d17 d18 d19 ...
//
//   batch.w[0] = a0 b0 c0 d0 |  a8  b8  c8  d8 | a16 b16 c16 d16 ...
//   batch.w[1] = a1 b1 c1 d1 |  a9  b9  c9  d9 | a17 b17 c17 d17 ...
//   batch.w[2] = a2 b2 c2 d2 | a10 b10 c10 d10 | a18 b18 c18 d18 ...
//   batch.w[3] = a3 b3 c3 d3 | a11 b11 c11 d11 | a19 b19 c19 d19 ...
//
// Note also that bitwise operations and (logical) byte permutations on an
// |aes_word_t| work equally for the bitsliced and compact words.
//
// We use the compact form in the |AES_KEY| representation to save work
// inflating round keys into |AES_NOHW_BATCH|. The compact form also exists
// temporarily while moving blocks in or out of an |AES_NOHW_BATCH|, immediately
// before or after |aes_nohw_transpose|.

#define AES_NOHW_BLOCK_WORDS (16 / sizeof(aes_word_t))

// An AES_NOHW_BATCH stores |AES_NOHW_BATCH_SIZE| blocks. Unless otherwise
// specified, it is in bitsliced form.
typedef struct {
  aes_word_t w[8];
} AES_NOHW_BATCH;

// An AES_NOHW_SCHEDULE is an expanded bitsliced AES key schedule. It is
// suitable for encryption or decryption. It is as large as |AES_NOHW_BATCH|
// |AES_KEY|s so it should not be used as a long-term key representation.
typedef struct {
  // keys is an array of batches, one for each round key. Each batch stores
  // |AES_NOHW_BATCH_SIZE| copies of the round key in bitsliced form.
  AES_NOHW_BATCH keys[AES_MAXNR + 1];
} AES_NOHW_SCHEDULE;

// aes_nohw_batch_set sets the |i|th block of |batch| to |in|. |batch| is in
// compact form.
static inline void aes_nohw_batch_set(AES_NOHW_BATCH *batch,
                                      const aes_word_t in[AES_NOHW_BLOCK_WORDS],
                                      size_t i) {
  // Note the words are interleaved. The order comes from |aes_nohw_transpose|.
  // If |i| is zero and this is the 64-bit implementation, in[0] contains bits
  // 0-3 and in[1] contains bits 4-7. We place in[0] at w[0] and in[1] at
  // w[4] so that bits 0 and 4 are in the correct position. (In general, bits
  // along diagonals of |AES_NOHW_BATCH_SIZE| by |AES_NOHW_BATCH_SIZE| squares
  // will be correctly placed.)
  dev_assert_secret(i < AES_NOHW_BATCH_SIZE);
#if defined(OPENSSL_64_BIT)
  batch->w[i] = in[0];
  batch->w[i + 4] = in[1];
#else
  batch->w[i] = in[0];
  batch->w[i + 2] = in[1];
  batch->w[i + 4] = in[2];
  batch->w[i + 6] = in[3];
#endif
}

// aes_nohw_batch_get writes the |i|th block of |batch| to |out|. |batch| is in
// compact form.
static inline void aes_nohw_batch_get(const AES_NOHW_BATCH *batch,
                                      aes_word_t out[AES_NOHW_BLOCK_WORDS],
                                      size_t i) {
  dev_assert_secret(i < AES_NOHW_BATCH_SIZE);
#if defined(OPENSSL_64_BIT)
  out[0] = batch->w[i];
  out[1] = batch->w[i + 4];
#else
  out[0] = batch->w[i];
  out[1] = batch->w[i + 2];
  out[2] = batch->w[i + 4];
  out[3] = batch->w[i + 6];
#endif
}

// aes_nohw_delta_swap returns |a| with bits |a & mask| and
// |a & (mask << shift)| swapped. |mask| and |mask << shift| may not overlap.
static inline aes_word_t aes_nohw_delta_swap(aes_word_t a, aes_word_t mask,
                                             aes_word_t shift) {
  // See
  // https://reflectionsonsecurity.wordpress.com/2014/05/11/efficient-bit-permutation-using-delta-swaps/
  aes_word_t b = (a ^ (a >> shift)) & mask;
  return a ^ b ^ (b << shift);
}

// In the 32-bit and 64-bit implementations, a block spans multiple words.
// |aes_nohw_compact_block| must permute bits across different words. First we
// implement |aes_nohw_compact_word| which performs a smaller version of the
// transformation which stays within a single word.
//
// These transformations are generalizations of the output of
// http://programming.sirrida.de/calcperm.php on smaller inputs.
#if defined(OPENSSL_64_BIT)
static inline uint64_t aes_nohw_compact_word(uint64_t a) {
#if defined(RING_BIG_ENDIAN)
  a = CRYPTO_bswap8(a);
#endif
  // Numbering the 64/2 = 16 4-bit chunks, least to most significant, we swap
  // quartets of those chunks:
  //   0 1 2 3 | 4 5 6 7 | 8  9 10 11 | 12 13 14 15 =>
  //   0 2 1 3 | 4 6 5 7 | 8 10  9 11 | 12 14 13 15
  a = aes_nohw_delta_swap(a, UINT64_C(0x00f000f000f000f0), 4);
  // Swap quartets of 8-bit chunks (still numbering by 4-bit chunks):
  //   0 2 1 3 | 4 6 5 7 | 8 10  9 11 | 12 14 13 15 =>
  //   0 2 4 6 | 1 3 5 7 | 8 10 12 14 |  9 11 13 15
  a = aes_nohw_delta_swap(a, UINT64_C(0x0000ff000000ff00), 8);
  // Swap quartets of 16-bit chunks (still numbering by 4-bit chunks):
  //   0 2 4 6 | 1  3  5  7 | 8 10 12 14 | 9 11 13 15 =>
  //   0 2 4 6 | 8 10 12 14 | 1  3  5  7 | 9 11 13 15
  a = aes_nohw_delta_swap(a, UINT64_C(0x00000000ffff0000), 16);
  return a;
}

static inline uint64_t aes_nohw_uncompact_word(uint64_t a) {
  // Reverse the steps of |aes_nohw_uncompact_word|.
  a = aes_nohw_delta_swap(a, UINT64_C(0x00000000ffff0000), 16);
  a = aes_nohw_delta_swap(a, UINT64_C(0x0000ff000000ff00), 8);
  a = aes_nohw_delta_swap(a, UINT64_C(0x00f000f000f000f0), 4);
#if defined(RING_BIG_ENDIAN)
  a = CRYPTO_bswap8(a);
#endif
  return a;
}
#else   // !OPENSSL_64_BIT
static inline uint32_t aes_nohw_compact_word(uint32_t a) {
#if defined(RING_BIG_ENDIAN)
  a = CRYPTO_bswap4(a);
#endif
  // Numbering the 32/2 = 16 pairs of bits, least to most significant, we swap:
  //   0 1 2 3 | 4 5 6 7 | 8  9 10 11 | 12 13 14 15 =>
  //   0 4 2 6 | 1 5 3 7 | 8 12 10 14 |  9 13 11 15
  // Note:  0x00cc = 0b0000_0000_1100_1100
  //   0x00cc << 6 = 0b0011_0011_0000_0000
  a = aes_nohw_delta_swap(a, 0x00cc00cc, 6);
  // Now we swap groups of four bits (still numbering by pairs):
  //   0 4 2  6 | 1 5 3  7 | 8 12 10 14 | 9 13 11 15 =>
  //   0 4 8 12 | 1 5 9 13 | 2  6 10 14 | 3  7 11 15
  // Note: 0x0000_f0f0 << 12 = 0x0f0f_0000
  a = aes_nohw_delta_swap(a, 0x0000f0f0, 12);
  return a;
}

static inline uint32_t aes_nohw_uncompact_word(uint32_t a) {
  // Reverse the steps of |aes_nohw_uncompact_word|.
  a = aes_nohw_delta_swap(a, 0x0000f0f0, 12);
  a = aes_nohw_delta_swap(a, 0x00cc00cc, 6);
#if defined(RING_BIG_ENDIAN)
  a = CRYPTO_bswap4(a);
#endif
  return a;
}

static inline uint32_t aes_nohw_word_from_bytes(uint8_t a0, uint8_t a1,
                                                uint8_t a2, uint8_t a3) {
  return (uint32_t)a0 | ((uint32_t)a1 << 8) | ((uint32_t)a2 << 16) |
         ((uint32_t)a3 << 24);
}

static inline uint8_t lo(uint32_t a) {
  return (uint8_t)a;
}

#endif  // OPENSSL_64_BIT

static inline void aes_nohw_compact_block(aes_word_t out[AES_NOHW_BLOCK_WORDS],
                                          const uint8_t in[16]) {
  OPENSSL_memcpy(out, in, 16);
#if defined(OPENSSL_64_BIT)
  uint64_t a0 = aes_nohw_compact_word(out[0]);
  uint64_t a1 = aes_nohw_compact_word(out[1]);
  out[0] = (a0 & UINT64_C(0x00000000ffffffff)) | (a1 << 32);
  out[1] = (a1 & UINT64_C(0xffffffff00000000)) | (a0 >> 32);
#else
  uint32_t a0 = aes_nohw_compact_word(out[0]);
  uint32_t a1 = aes_nohw_compact_word(out[1]);
  uint32_t a2 = aes_nohw_compact_word(out[2]);
  uint32_t a3 = aes_nohw_compact_word(out[3]);
  // Note clang, when building for ARM Thumb2, will sometimes miscompile
  // expressions such as (a0 & 0x0000ff00) << 8, particularly when building
  // without optimizations. This bug was introduced in
  // https://reviews.llvm.org/rL340261 and fixed in
  // https://reviews.llvm.org/rL351310. The following is written to avoid this.
  out[0] = aes_nohw_word_from_bytes(lo(a0), lo(a1), lo(a2), lo(a3));
  out[1] = aes_nohw_word_from_bytes(lo(a0 >> 8), lo(a1 >> 8), lo(a2 >> 8), lo(a3 >> 8));
  out[2] = aes_nohw_word_from_bytes(lo(a0 >> 16), lo(a1 >> 16), lo(a2 >> 16), lo(a3 >> 16));
  out[3] = aes_nohw_word_from_bytes(lo(a0 >> 24), lo(a1 >> 24), lo(a2 >> 24), lo(a3 >> 24));
#endif
}

static inline void aes_nohw_uncompact_block(
    uint8_t out[16], const aes_word_t in[AES_NOHW_BLOCK_WORDS]) {
#if defined(OPENSSL_64_BIT)
  uint64_t a0 = in[0];
  uint64_t a1 = in[1];
  uint64_t b0 =
      aes_nohw_uncompact_word((a0 & UINT64_C(0x00000000ffffffff)) | (a1 << 32));
  uint64_t b1 =
      aes_nohw_uncompact_word((a1 & UINT64_C(0xffffffff00000000)) | (a0 >> 32));
  OPENSSL_memcpy(out, &b0, 8);
  OPENSSL_memcpy(out + 8, &b1, 8);
#else
  uint32_t a0 = in[0];
  uint32_t a1 = in[1];
  uint32_t a2 = in[2];
  uint32_t a3 = in[3];
  // Note clang, when building for ARM Thumb2, will sometimes miscompile
  // expressions such as (a0 & 0x0000ff00) << 8, particularly when building
  // without optimizations. This bug was introduced in
  // https://reviews.llvm.org/rL340261 and fixed in
  // https://reviews.llvm.org/rL351310. The following is written to avoid this.
  uint32_t b0 = aes_nohw_word_from_bytes(lo(a0), lo(a1), lo(a2), lo(a3));
  uint32_t b1 = aes_nohw_word_from_bytes(lo(a0 >> 8), lo(a1 >> 8), lo(a2 >> 8), lo(a3 >> 8));
  uint32_t b2 =
      aes_nohw_word_from_bytes(lo(a0 >> 16), lo(a1 >> 16), lo(a2 >> 16), lo(a3 >> 16));
  uint32_t b3 =
      aes_nohw_word_from_bytes(lo(a0 >> 24), lo(a1 >> 24), lo(a2 >> 24), lo(a3 >> 24));
  b0 = aes_nohw_uncompact_word(b0);
  b1 = aes_nohw_uncompact_word(b1);
  b2 = aes_nohw_uncompact_word(b2);
  b3 = aes_nohw_uncompact_word(b3);
  OPENSSL_memcpy(out, &b0, 4);
  OPENSSL_memcpy(out + 4, &b1, 4);
  OPENSSL_memcpy(out + 8, &b2, 4);
  OPENSSL_memcpy(out + 12, &b3, 4);
#endif
}

// aes_nohw_swap_bits is a variation on a delta swap. It swaps the bits in
// |*a & (mask << shift)| with the bits in |*b & mask|. |mask| and
// |mask << shift| must not overlap. |mask| is specified as a |uint32_t|, but it
// is repeated to the full width of |aes_word_t|.
static inline void aes_nohw_swap_bits(aes_word_t *a, aes_word_t *b,
                                      uint32_t mask, aes_word_t shift) {
#if defined(OPENSSL_64_BIT)
  aes_word_t mask_w = (((uint64_t)mask) << 32) | mask;
#else
  aes_word_t mask_w = mask;
#endif
  // This is a variation on a delta swap.
  aes_word_t swap = ((*a >> shift) ^ *b) & mask_w;
  *a ^= swap << shift;
  *b ^= swap;
}

// aes_nohw_transpose converts |batch| to and from bitsliced form. It divides
// the 8 × word_size bits into AES_NOHW_BATCH_SIZE × AES_NOHW_BATCH_SIZE squares
// and transposes each square.
static void aes_nohw_transpose(AES_NOHW_BATCH *batch) {
  // Swap bits with index 0 and 1 mod 2 (0x55 = 0b01010101).
  aes_nohw_swap_bits(&batch->w[0], &batch->w[1], 0x55555555, 1);
  aes_nohw_swap_bits(&batch->w[2], &batch->w[3], 0x55555555, 1);
  aes_nohw_swap_bits(&batch->w[4], &batch->w[5], 0x55555555, 1);
  aes_nohw_swap_bits(&batch->w[6], &batch->w[7], 0x55555555, 1);

#if AES_NOHW_BATCH_SIZE >= 4
  // Swap bits with index 0-1 and 2-3 mod 4 (0x33 = 0b00110011).
  aes_nohw_swap_bits(&batch->w[0], &batch->w[2], 0x33333333, 2);
  aes_nohw_swap_bits(&batch->w[1], &batch->w[3], 0x33333333, 2);
  aes_nohw_swap_bits(&batch->w[4], &batch->w[6], 0x33333333, 2);
  aes_nohw_swap_bits(&batch->w[5], &batch->w[7], 0x33333333, 2);
#endif

#if AES_NOHW_BATCH_SIZE >= 8
  // Swap bits with index 0-3 and 4-7 mod 8 (0x0f = 0b00001111).
  aes_nohw_swap_bits(&batch->w[0], &batch->w[4], 0x0f0f0f0f, 4);
  aes_nohw_swap_bits(&batch->w[1], &batch->w[5], 0x0f0f0f0f, 4);
  aes_nohw_swap_bits(&batch->w[2], &batch->w[6], 0x0f0f0f0f, 4);
  aes_nohw_swap_bits(&batch->w[3], &batch->w[7], 0x0f0f0f0f, 4);
#endif
}

// aes_nohw_to_batch initializes |out| with the |num_blocks| blocks from |in|.
// |num_blocks| must be at most |AES_NOHW_BATCH|.
static void aes_nohw_to_batch(AES_NOHW_BATCH *out, const uint8_t *in,
                              size_t num_blocks) {
  // Don't leave unused blocks uninitialized.
  OPENSSL_memset(out, 0, sizeof(AES_NOHW_BATCH));
  debug_assert_nonsecret(num_blocks <= AES_NOHW_BATCH_SIZE);
  for (size_t i = 0; i < num_blocks; i++) {
    aes_word_t block[AES_NOHW_BLOCK_WORDS];
    aes_nohw_compact_block(block, in + 16 * i);
    aes_nohw_batch_set(out, block, i);
  }

  aes_nohw_transpose(out);
}

// aes_nohw_to_batch writes the first |num_blocks| blocks in |batch| to |out|.
// |num_blocks| must be at most |AES_NOHW_BATCH|.
static void aes_nohw_from_batch(uint8_t *out, size_t num_blocks,
                                const AES_NOHW_BATCH *batch) {
  AES_NOHW_BATCH copy = *batch;
  aes_nohw_transpose(&copy);

  debug_assert_nonsecret(num_blocks <= AES_NOHW_BATCH_SIZE);
  for (size_t i = 0; i < num_blocks; i++) {
    aes_word_t block[AES_NOHW_BLOCK_WORDS];
    aes_nohw_batch_get(&copy, block, i);
    aes_nohw_uncompact_block(out + 16 * i, block);
  }
}


// AES round steps.

static void aes_nohw_add_round_key(AES_NOHW_BATCH *batch,
                                   const AES_NOHW_BATCH *key) {
  for (size_t i = 0; i < 8; i++) {
    batch->w[i] = aes_nohw_xor(batch->w[i], key->w[i]);
  }
}

static void aes_nohw_sub_bytes(AES_NOHW_BATCH *batch) {
  // See https://eprint.iacr.org/2009/191.pdf, Appendix C.
  aes_word_t x0 = batch->w[7];
  aes_word_t x1 = batch->w[6];
  aes_word_t x2 = batch->w[5];
  aes_word_t x3 = batch->w[4];
  aes_word_t x4 = batch->w[3];
  aes_word_t x5 = batch->w[2];
  aes_word_t x6 = batch->w[1];
  aes_word_t x7 = batch->w[0];

  // Figure 2, the top linear transformation.
  aes_word_t y14 = aes_nohw_xor(x3, x5);
  aes_word_t y13 = aes_nohw_xor(x0, x6);
  aes_word_t y9 = aes_nohw_xor(x0, x3);
  aes_word_t y8 = aes_nohw_xor(x0, x5);
  aes_word_t t0 = aes_nohw_xor(x1, x2);
  aes_word_t y1 = aes_nohw_xor(t0, x7);
  aes_word_t y4 = aes_nohw_xor(y1, x3);
  aes_word_t y12 = aes_nohw_xor(y13, y14);
  aes_word_t y2 = aes_nohw_xor(y1, x0);
  aes_word_t y5 = aes_nohw_xor(y1, x6);
  aes_word_t y3 = aes_nohw_xor(y5, y8);
  aes_word_t t1 = aes_nohw_xor(x4, y12);
  aes_word_t y15 = aes_nohw_xor(t1, x5);
  aes_word_t y20 = aes_nohw_xor(t1, x1);
  aes_word_t y6 = aes_nohw_xor(y15, x7);
  aes_word_t y10 = aes_nohw_xor(y15, t0);
  aes_word_t y11 = aes_nohw_xor(y20, y9);
  aes_word_t y7 = aes_nohw_xor(x7, y11);
  aes_word_t y17 = aes_nohw_xor(y10, y11);
  aes_word_t y19 = aes_nohw_xor(y10, y8);
  aes_word_t y16 = aes_nohw_xor(t0, y11);
  aes_word_t y21 = aes_nohw_xor(y13, y16);
  aes_word_t y18 = aes_nohw_xor(x0, y16);

  // Figure 3, the middle non-linear section.
  aes_word_t t2 = aes_nohw_and(y12, y15);
  aes_word_t t3 = aes_nohw_and(y3, y6);
  aes_word_t t4 = aes_nohw_xor(t3, t2);
  aes_word_t t5 = aes_nohw_and(y4, x7);
  aes_word_t t6 = aes_nohw_xor(t5, t2);
  aes_word_t t7 = aes_nohw_and(y13, y16);
  aes_word_t t8 = aes_nohw_and(y5, y1);
  aes_word_t t9 = aes_nohw_xor(t8, t7);
  aes_word_t t10 = aes_nohw_and(y2, y7);
  aes_word_t t11 = aes_nohw_xor(t10, t7);
  aes_word_t t12 = aes_nohw_and(y9, y11);
  aes_word_t t13 = aes_nohw_and(y14, y17);
  aes_word_t t14 = aes_nohw_xor(t13, t12);
  aes_word_t t15 = aes_nohw_and(y8, y10);
  aes_word_t t16 = aes_nohw_xor(t15, t12);
  aes_word_t t17 = aes_nohw_xor(t4, t14);
  aes_word_t t18 = aes_nohw_xor(t6, t16);
  aes_word_t t19 = aes_nohw_xor(t9, t14);
  aes_word_t t20 = aes_nohw_xor(t11, t16);
  aes_word_t t21 = aes_nohw_xor(t17, y20);
  aes_word_t t22 = aes_nohw_xor(t18, y19);
  aes_word_t t23 = aes_nohw_xor(t19, y21);
  aes_word_t t24 = aes_nohw_xor(t20, y18);
  aes_word_t t25 = aes_nohw_xor(t21, t22);
  aes_word_t t26 = aes_nohw_and(t21, t23);
  aes_word_t t27 = aes_nohw_xor(t24, t26);
  aes_word_t t28 = aes_nohw_and(t25, t27);
  aes_word_t t29 = aes_nohw_xor(t28, t22);
  aes_word_t t30 = aes_nohw_xor(t23, t24);
  aes_word_t t31 = aes_nohw_xor(t22, t26);
  aes_word_t t32 = aes_nohw_and(t31, t30);
  aes_word_t t33 = aes_nohw_xor(t32, t24);
  aes_word_t t34 = aes_nohw_xor(t23, t33);
  aes_word_t t35 = aes_nohw_xor(t27, t33);
  aes_word_t t36 = aes_nohw_and(t24, t35);
  aes_word_t t37 = aes_nohw_xor(t36, t34);
  aes_word_t t38 = aes_nohw_xor(t27, t36);
  aes_word_t t39 = aes_nohw_and(t29, t38);
  aes_word_t t40 = aes_nohw_xor(t25, t39);
  aes_word_t t41 = aes_nohw_xor(t40, t37);
  aes_word_t t42 = aes_nohw_xor(t29, t33);
  aes_word_t t43 = aes_nohw_xor(t29, t40);
  aes_word_t t44 = aes_nohw_xor(t33, t37);
  aes_word_t t45 = aes_nohw_xor(t42, t41);
  aes_word_t z0 = aes_nohw_and(t44, y15);
  aes_word_t z1 = aes_nohw_and(t37, y6);
  aes_word_t z2 = aes_nohw_and(t33, x7);
  aes_word_t z3 = aes_nohw_and(t43, y16);
  aes_word_t z4 = aes_nohw_and(t40, y1);
  aes_word_t z5 = aes_nohw_and(t29, y7);
  aes_word_t z6 = aes_nohw_and(t42, y11);
  aes_word_t z7 = aes_nohw_and(t45, y17);
  aes_word_t z8 = aes_nohw_and(t41, y10);
  aes_word_t z9 = aes_nohw_and(t44, y12);
  aes_word_t z10 = aes_nohw_and(t37, y3);
  aes_word_t z11 = aes_nohw_and(t33, y4);
  aes_word_t z12 = aes_nohw_and(t43, y13);
  aes_word_t z13 = aes_nohw_and(t40, y5);
  aes_word_t z14 = aes_nohw_and(t29, y2);
  aes_word_t z15 = aes_nohw_and(t42, y9);
  aes_word_t z16 = aes_nohw_and(t45, y14);
  aes_word_t z17 = aes_nohw_and(t41, y8);

  // Figure 4, bottom linear transformation.
  aes_word_t t46 = aes_nohw_xor(z15, z16);
  aes_word_t t47 = aes_nohw_xor(z10, z11);
  aes_word_t t48 = aes_nohw_xor(z5, z13);
  aes_word_t t49 = aes_nohw_xor(z9, z10);
  aes_word_t t50 = aes_nohw_xor(z2, z12);
  aes_word_t t51 = aes_nohw_xor(z2, z5);
  aes_word_t t52 = aes_nohw_xor(z7, z8);
  aes_word_t t53 = aes_nohw_xor(z0, z3);
  aes_word_t t54 = aes_nohw_xor(z6, z7);
  aes_word_t t55 = aes_nohw_xor(z16, z17);
  aes_word_t t56 = aes_nohw_xor(z12, t48);
  aes_word_t t57 = aes_nohw_xor(t50, t53);
  aes_word_t t58 = aes_nohw_xor(z4, t46);
  aes_word_t t59 = aes_nohw_xor(z3, t54);
  aes_word_t t60 = aes_nohw_xor(t46, t57);
  aes_word_t t61 = aes_nohw_xor(z14, t57);
  aes_word_t t62 = aes_nohw_xor(t52, t58);
  aes_word_t t63 = aes_nohw_xor(t49, t58);
  aes_word_t t64 = aes_nohw_xor(z4, t59);
  aes_word_t t65 = aes_nohw_xor(t61, t62);
  aes_word_t t66 = aes_nohw_xor(z1, t63);
  aes_word_t s0 = aes_nohw_xor(t59, t63);
  aes_word_t s6 = aes_nohw_xor(t56, aes_nohw_not(t62));
  aes_word_t s7 = aes_nohw_xor(t48, aes_nohw_not(t60));
  aes_word_t t67 = aes_nohw_xor(t64, t65);
  aes_word_t s3 = aes_nohw_xor(t53, t66);
  aes_word_t s4 = aes_nohw_xor(t51, t66);
  aes_word_t s5 = aes_nohw_xor(t47, t65);
  aes_word_t s1 = aes_nohw_xor(t64, aes_nohw_not(s3));
  aes_word_t s2 = aes_nohw_xor(t55, aes_nohw_not(t67));

  batch->w[0] = s7;
  batch->w[1] = s6;
  batch->w[2] = s5;
  batch->w[3] = s4;
  batch->w[4] = s3;
  batch->w[5] = s2;
  batch->w[6] = s1;
  batch->w[7] = s0;
}

// aes_nohw_rotate_cols_right returns |v| with the columns in each row rotated
// to the right by |n|. This is a macro because |aes_nohw_shift_*| require
// constant shift counts in the SSE2 implementation.
#define aes_nohw_rotate_cols_right(/* aes_word_t */ v, /* const */ n) \
  (aes_nohw_or(aes_nohw_shift_right((v), (n)*4),                      \
               aes_nohw_shift_left((v), 16 - (n)*4)))

static void aes_nohw_shift_rows(AES_NOHW_BATCH *batch) {
  for (size_t i = 0; i < 8; i++) {
    aes_word_t row0 = aes_nohw_and(batch->w[i], AES_NOHW_ROW0_MASK);
    aes_word_t row1 = aes_nohw_and(batch->w[i], AES_NOHW_ROW1_MASK);
    aes_word_t row2 = aes_nohw_and(batch->w[i], AES_NOHW_ROW2_MASK);
    aes_word_t row3 = aes_nohw_and(batch->w[i], AES_NOHW_ROW3_MASK);
    row1 = aes_nohw_rotate_cols_right(row1, 1);
    row2 = aes_nohw_rotate_cols_right(row2, 2);
    row3 = aes_nohw_rotate_cols_right(row3, 3);
    batch->w[i] = aes_nohw_or(aes_nohw_or(row0, row1), aes_nohw_or(row2, row3));
  }
}

// aes_nohw_rotate_rows_down returns |v| with the rows in each column rotated
// down by one.
static inline aes_word_t aes_nohw_rotate_rows_down(aes_word_t v) {
#if defined(OPENSSL_64_BIT)
  return ((v >> 4) & UINT64_C(0x0fff0fff0fff0fff)) |
         ((v << 12) & UINT64_C(0xf000f000f000f000));
#else
  return ((v >> 2) & 0x3f3f3f3f) | ((v << 6) & 0xc0c0c0c0);
#endif
}

// aes_nohw_rotate_rows_twice returns |v| with the rows in each column rotated
// by two.
static inline aes_word_t aes_nohw_rotate_rows_twice(aes_word_t v) {
#if defined(OPENSSL_64_BIT)
  return ((v >> 8) & UINT64_C(0x00ff00ff00ff00ff)) |
         ((v << 8) & UINT64_C(0xff00ff00ff00ff00));
#else
  return ((v >> 4) & 0x0f0f0f0f) | ((v << 4) & 0xf0f0f0f0);
#endif
}

static void aes_nohw_mix_columns(AES_NOHW_BATCH *batch) {
  // See https://eprint.iacr.org/2009/129.pdf, section 4.4 and appendix A.
  aes_word_t a0 = batch->w[0];
  aes_word_t a1 = batch->w[1];
  aes_word_t a2 = batch->w[2];
  aes_word_t a3 = batch->w[3];
  aes_word_t a4 = batch->w[4];
  aes_word_t a5 = batch->w[5];
  aes_word_t a6 = batch->w[6];
  aes_word_t a7 = batch->w[7];

  aes_word_t r0 = aes_nohw_rotate_rows_down(a0);
  aes_word_t a0_r0 = aes_nohw_xor(a0, r0);
  aes_word_t r1 = aes_nohw_rotate_rows_down(a1);
  aes_word_t a1_r1 = aes_nohw_xor(a1, r1);
  aes_word_t r2 = aes_nohw_rotate_rows_down(a2);
  aes_word_t a2_r2 = aes_nohw_xor(a2, r2);
  aes_word_t r3 = aes_nohw_rotate_rows_down(a3);
  aes_word_t a3_r3 = aes_nohw_xor(a3, r3);
  aes_word_t r4 = aes_nohw_rotate_rows_down(a4);
  aes_word_t a4_r4 = aes_nohw_xor(a4, r4);
  aes_word_t r5 = aes_nohw_rotate_rows_down(a5);
  aes_word_t a5_r5 = aes_nohw_xor(a5, r5);
  aes_word_t r6 = aes_nohw_rotate_rows_down(a6);
  aes_word_t a6_r6 = aes_nohw_xor(a6, r6);
  aes_word_t r7 = aes_nohw_rotate_rows_down(a7);
  aes_word_t a7_r7 = aes_nohw_xor(a7, r7);

  batch->w[0] =
      aes_nohw_xor(aes_nohw_xor(a7_r7, r0), aes_nohw_rotate_rows_twice(a0_r0));
  batch->w[1] =
      aes_nohw_xor(aes_nohw_xor(a0_r0, a7_r7),
                   aes_nohw_xor(r1, aes_nohw_rotate_rows_twice(a1_r1)));
  batch->w[2] =
      aes_nohw_xor(aes_nohw_xor(a1_r1, r2), aes_nohw_rotate_rows_twice(a2_r2));
  batch->w[3] =
      aes_nohw_xor(aes_nohw_xor(a2_r2, a7_r7),
                   aes_nohw_xor(r3, aes_nohw_rotate_rows_twice(a3_r3)));
  batch->w[4] =
      aes_nohw_xor(aes_nohw_xor(a3_r3, a7_r7),
                   aes_nohw_xor(r4, aes_nohw_rotate_rows_twice(a4_r4)));
  batch->w[5] =
      aes_nohw_xor(aes_nohw_xor(a4_r4, r5), aes_nohw_rotate_rows_twice(a5_r5));
  batch->w[6] =
      aes_nohw_xor(aes_nohw_xor(a5_r5, r6), aes_nohw_rotate_rows_twice(a6_r6));
  batch->w[7] =
      aes_nohw_xor(aes_nohw_xor(a6_r6, r7), aes_nohw_rotate_rows_twice(a7_r7));
}

static void aes_nohw_encrypt_batch(const AES_NOHW_SCHEDULE *key,
                                   size_t num_rounds, AES_NOHW_BATCH *batch) {
  aes_nohw_add_round_key(batch, &key->keys[0]);
  for (size_t i = 1; i < num_rounds; i++) {
    aes_nohw_sub_bytes(batch);
    aes_nohw_shift_rows(batch);
    aes_nohw_mix_columns(batch);
    aes_nohw_add_round_key(batch, &key->keys[i]);
  }
  aes_nohw_sub_bytes(batch);
  aes_nohw_shift_rows(batch);
  aes_nohw_add_round_key(batch, &key->keys[num_rounds]);
}

// Key schedule.

static void aes_nohw_expand_round_keys(AES_NOHW_SCHEDULE *out,
                                       const AES_KEY *key) {
  for (size_t i = 0; i <= key->rounds; i++) {
    // Copy the round key into each block in the batch.
    for (size_t j = 0; j < AES_NOHW_BATCH_SIZE; j++) {
      aes_word_t tmp[AES_NOHW_BLOCK_WORDS];
      OPENSSL_memcpy(tmp, key->rd_key + 4 * i, 16);
      aes_nohw_batch_set(&out->keys[i], tmp, j);
    }
    aes_nohw_transpose(&out->keys[i]);
  }
}

static const uint8_t aes_nohw_rcon[10] = {0x01, 0x02, 0x04, 0x08, 0x10,
                                          0x20, 0x40, 0x80, 0x1b, 0x36};

// aes_nohw_rcon_slice returns the |i|th group of |AES_NOHW_BATCH_SIZE| bits in
// |rcon|, stored in a |aes_word_t|.
static inline aes_word_t aes_nohw_rcon_slice(uint8_t rcon, size_t i) {
  rcon = (rcon >> (i * AES_NOHW_BATCH_SIZE)) & ((1 << AES_NOHW_BATCH_SIZE) - 1);
  return ((aes_word_t)rcon);
}

static void aes_nohw_sub_block(aes_word_t out[AES_NOHW_BLOCK_WORDS],
                               const aes_word_t in[AES_NOHW_BLOCK_WORDS]) {
  AES_NOHW_BATCH batch;
  OPENSSL_memset(&batch, 0, sizeof(batch));
  aes_nohw_batch_set(&batch, in, 0);
  aes_nohw_transpose(&batch);
  aes_nohw_sub_bytes(&batch);
  aes_nohw_transpose(&batch);
  aes_nohw_batch_get(&batch, out, 0);
}

static void aes_nohw_setup_key_128(AES_KEY *key, const uint8_t in[16]) {
  key->rounds = 10;

  aes_word_t block[AES_NOHW_BLOCK_WORDS];
  aes_nohw_compact_block(block, in);
  OPENSSL_memcpy(key->rd_key, block, 16);

  for (size_t i = 1; i <= 10; i++) {
    aes_word_t sub[AES_NOHW_BLOCK_WORDS];
    aes_nohw_sub_block(sub, block);
    uint8_t rcon = aes_nohw_rcon[i - 1];
    for (size_t j = 0; j < AES_NOHW_BLOCK_WORDS; j++) {
      // Incorporate |rcon| and the transformed word into the first word.
      block[j] = aes_nohw_xor(block[j], aes_nohw_rcon_slice(rcon, j));
      block[j] = aes_nohw_xor(
          block[j],
          aes_nohw_shift_right(aes_nohw_rotate_rows_down(sub[j]), 12));
      // Propagate to the remaining words. Note this is reordered from the usual
      // formulation to avoid needing masks.
      aes_word_t v = block[j];
      block[j] = aes_nohw_xor(block[j], aes_nohw_shift_left(v, 4));
      block[j] = aes_nohw_xor(block[j], aes_nohw_shift_left(v, 8));
      block[j] = aes_nohw_xor(block[j], aes_nohw_shift_left(v, 12));
    }
    OPENSSL_memcpy(key->rd_key + 4 * i, block, 16);
  }
}

static void aes_nohw_setup_key_256(AES_KEY *key, const uint8_t in[32]) {
  key->rounds = 14;

  // Each key schedule iteration produces two round keys.
  aes_word_t block1[AES_NOHW_BLOCK_WORDS], block2[AES_NOHW_BLOCK_WORDS];
  aes_nohw_compact_block(block1, in);
  OPENSSL_memcpy(key->rd_key, block1, 16);

  aes_nohw_compact_block(block2, in + 16);
  OPENSSL_memcpy(key->rd_key + 4, block2, 16);

  for (size_t i = 2; i <= 14; i += 2) {
    aes_word_t sub[AES_NOHW_BLOCK_WORDS];
    aes_nohw_sub_block(sub, block2);
    uint8_t rcon = aes_nohw_rcon[i / 2 - 1];
    for (size_t j = 0; j < AES_NOHW_BLOCK_WORDS; j++) {
      // Incorporate |rcon| and the transformed word into the first word.
      block1[j] = aes_nohw_xor(block1[j], aes_nohw_rcon_slice(rcon, j));
      block1[j] = aes_nohw_xor(
          block1[j],
          aes_nohw_shift_right(aes_nohw_rotate_rows_down(sub[j]), 12));
      // Propagate to the remaining words.
      aes_word_t v = block1[j];
      block1[j] = aes_nohw_xor(block1[j], aes_nohw_shift_left(v, 4));
      block1[j] = aes_nohw_xor(block1[j], aes_nohw_shift_left(v, 8));
      block1[j] = aes_nohw_xor(block1[j], aes_nohw_shift_left(v, 12));
    }
    OPENSSL_memcpy(key->rd_key + 4 * i, block1, 16);

    if (i == 14) {
      break;
    }

    aes_nohw_sub_block(sub, block1);
    for (size_t j = 0; j < AES_NOHW_BLOCK_WORDS; j++) {
      // Incorporate the transformed word into the first word.
      block2[j] = aes_nohw_xor(block2[j], aes_nohw_shift_right(sub[j], 12));
      // Propagate to the remaining words.
      aes_word_t v = block2[j];
      block2[j] = aes_nohw_xor(block2[j], aes_nohw_shift_left(v, 4));
      block2[j] = aes_nohw_xor(block2[j], aes_nohw_shift_left(v, 8));
      block2[j] = aes_nohw_xor(block2[j], aes_nohw_shift_left(v, 12));
    }
    OPENSSL_memcpy(key->rd_key + 4 * (i + 1), block2, 16);
  }
}


// External API.

int aes_nohw_set_encrypt_key(const uint8_t *key, unsigned bits,
                             AES_KEY *aeskey) {
  switch (bits) {
    case 128:
      aes_nohw_setup_key_128(aeskey, key);
      return 0;
    case 256:
      aes_nohw_setup_key_256(aeskey, key);
      return 0;
  }
  return 1;
}

void aes_nohw_encrypt(const uint8_t *in, uint8_t *out, const AES_KEY *key) {
  AES_NOHW_SCHEDULE sched;
  aes_nohw_expand_round_keys(&sched, key);
  AES_NOHW_BATCH batch;
  aes_nohw_to_batch(&batch, in, /*num_blocks=*/1);
  aes_nohw_encrypt_batch(&sched, key->rounds, &batch);
  aes_nohw_from_batch(out, /*num_blocks=*/1, &batch);
}

static inline void aes_nohw_xor_block(uint8_t out[16], const uint8_t a[16],
                                      const uint8_t b[16]) {
  for (size_t i = 0; i < 16; i += sizeof(aes_word_t)) {
    aes_word_t x, y;
    OPENSSL_memcpy(&x, a + i, sizeof(aes_word_t));
    OPENSSL_memcpy(&y, b + i, sizeof(aes_word_t));
    x = aes_nohw_xor(x, y);
    OPENSSL_memcpy(out + i, &x, sizeof(aes_word_t));
  }
}

void aes_nohw_ctr32_encrypt_blocks(const uint8_t *in, uint8_t *out,
                                   size_t blocks, const AES_KEY *key,
                                   const uint8_t ivec[16]) {
  if (blocks == 0) {
    return;
  }

  AES_NOHW_SCHEDULE sched;
  aes_nohw_expand_round_keys(&sched, key);

  // Make |AES_NOHW_BATCH_SIZE| copies of |ivec|.
  alignas(AES_NOHW_WORD_SIZE) uint8_t ivs[AES_NOHW_BATCH_SIZE * 16];
  alignas(AES_NOHW_WORD_SIZE) uint8_t enc_ivs[AES_NOHW_BATCH_SIZE * 16];
  for (size_t i = 0; i < AES_NOHW_BATCH_SIZE; i++) {
    OPENSSL_memcpy(ivs + 16 * i, ivec, 16);
  }

  uint32_t ctr = CRYPTO_load_u32_be(ivs + 12);
  for (;;) {
    // Update counters.
    for (size_t i = 0; i < AES_NOHW_BATCH_SIZE; i++) {
      CRYPTO_store_u32_be(ivs + 16 * i + 12, ctr + (uint32_t)i);
    }

    size_t todo = blocks >= AES_NOHW_BATCH_SIZE ? AES_NOHW_BATCH_SIZE : blocks;
    AES_NOHW_BATCH batch;
    aes_nohw_to_batch(&batch, ivs, todo);
    aes_nohw_encrypt_batch(&sched, key->rounds, &batch);
    aes_nohw_from_batch(enc_ivs, todo, &batch);

    for (size_t i = 0; i < todo; i++) {
      aes_nohw_xor_block(out + 16 * i, in + 16 * i, enc_ivs + 16 * i);
    }

    blocks -= todo;
    if (blocks == 0) {
      break;
    }

    in += 16 * AES_NOHW_BATCH_SIZE;
    out += 16 * AES_NOHW_BATCH_SIZE;
    ctr += AES_NOHW_BATCH_SIZE;
  }
}
