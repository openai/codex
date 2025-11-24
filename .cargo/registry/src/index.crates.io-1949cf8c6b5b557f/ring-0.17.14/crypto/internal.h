// Copyright 1995-2016 The OpenSSL Project Authors. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#ifndef OPENSSL_HEADER_CRYPTO_INTERNAL_H
#define OPENSSL_HEADER_CRYPTO_INTERNAL_H

#include <ring-core/base.h> // Must be first.

#include "ring-core/check.h"

#if defined(__clang__)
// Don't require prototypes for functions defined in C that are only
// used from Rust.
#pragma GCC diagnostic ignored "-Wmissing-prototypes"
#endif

#if defined(__GNUC__) && \
    (__GNUC__ * 10000 + __GNUC_MINOR__ * 100 + __GNUC_PATCHLEVEL__) < 40800
// |alignas| and |alignof| were added in C11. GCC added support in version 4.8.
// Testing for __STDC_VERSION__/__cplusplus doesn't work because 4.7 already
// reports support for C11.
#define alignas(x) __attribute__ ((aligned (x)))
#elif defined(_MSC_VER) && !defined(__clang__)
#define alignas(x) __declspec(align(x))
#else
#include <stdalign.h>
#endif

#if defined(__clang__) || defined(__GNUC__)
#define RING_NOINLINE __attribute__((noinline))
#elif defined(_MSC_VER)
#define RING_NOINLINE __declspec(noinline)
#else
#define RING_NOINLINE
#endif

// Some C compilers require a useless cast when dealing with arrays for the
// reason explained in
// https://gustedt.wordpress.com/2011/02/12/const-and-arrays/
#if defined(__clang__) || defined(_MSC_VER)
#define RING_CORE_POINTLESS_ARRAY_CONST_CAST(cast)
#else
#define RING_CORE_POINTLESS_ARRAY_CONST_CAST(cast) cast
#endif

// `uint8_t` isn't guaranteed to be 'unsigned char' and only 'char' and
// 'unsigned char' are allowed to alias according to ISO C.
typedef unsigned char aliasing_uint8_t;

#if (!defined(_MSC_VER) || defined(__clang__)) && defined(OPENSSL_64_BIT)
#define BORINGSSL_HAS_UINT128
typedef __int128_t int128_t;
typedef __uint128_t uint128_t;
#endif

// GCC-like compilers indicate SSE2 with |__SSE2__|. MSVC leaves the caller to
// know that x86_64 has SSE2, and uses _M_IX86_FP to indicate SSE2 on x86.
// https://learn.microsoft.com/en-us/cpp/preprocessor/predefined-macros?view=msvc-170
#if defined(OPENSSL_X86) || defined(OPENSSL_X86_64)
# if defined(_MSC_VER) && !defined(__clang__)
#  if defined(_M_AMD64) || defined(_M_X64) || (defined(_M_IX86_FP) && _M_IX86_FP >= 2)
#   define OPENSSL_SSE2
#  else
#   error "SSE2 is required."
#  endif
# elif !defined(__SSE2__)
#  error "SSE2 is required."
# endif
#endif

// For convenience in testing the fallback code, we allow disabling SSE2
// intrinsics via |OPENSSL_NO_SSE2_FOR_TESTING|. We require SSE2 on x86 and
// x86_64, so we would otherwise need to test such code on a non-x86 platform.
//
// This does not remove the above requirement for SSE2 support with assembly
// optimizations. It only disables some intrinsics-based optimizations so that
// we can test the fallback code on CI.
#if defined(OPENSSL_SSE2) && defined(OPENSSL_NO_SSE2_FOR_TESTING)
#undef OPENSSL_SSE2
#endif

// Pointer utility functions.

// buffers_alias returns one if |a| and |b| alias and zero otherwise.
static inline int buffers_alias(const void *a, size_t a_bytes,
                                const void *b, size_t b_bytes) {
  // Cast |a| and |b| to integers. In C, pointer comparisons between unrelated
  // objects are undefined whereas pointer to integer conversions are merely
  // implementation-defined. We assume the implementation defined it in a sane
  // way.
  uintptr_t a_u = (uintptr_t)a;
  uintptr_t b_u = (uintptr_t)b;
  return a_u + a_bytes > b_u && b_u + b_bytes > a_u;
}


// Constant-time utility functions.
//
// The following methods return a bitmask of all ones (0xff...f) for true and 0
// for false. This is useful for choosing a value based on the result of a
// conditional in constant time. For example,
//
// if (a < b) {
//   c = a;
// } else {
//   c = b;
// }
//
// can be written as
//
// crypto_word_t lt = constant_time_lt_w(a, b);
// c = constant_time_select_w(lt, a, b);

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wconversion"
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
#if defined(_MSC_VER) && !defined(__clang__)
#pragma warning(push)
// '=': conversion from 'crypto_word_t' to 'uint8_t', possible loss of data
#pragma warning(disable: 4242)
//  'initializing': conversion from 'crypto_word_t' to 'uint8_t', ...
#pragma warning(disable: 4244)
#endif

// crypto_word_t is the type that most constant-time functions use. Ideally we
// would like it to be |size_t|, but NaCl builds in 64-bit mode with 32-bit
// pointers, which means that |size_t| can be 32 bits when |BN_ULONG| is 64
// bits. Since we want to be able to do constant-time operations on a
// |BN_ULONG|, |crypto_word_t| is defined as an unsigned value with the native
// word length.
#if defined(OPENSSL_64_BIT)
typedef uint64_t crypto_word_t;
#define CRYPTO_WORD_BITS (64u)
#elif defined(OPENSSL_32_BIT)
typedef uint32_t crypto_word_t;
#define CRYPTO_WORD_BITS (32u)
#else
#error "Must define either OPENSSL_32_BIT or OPENSSL_64_BIT"
#endif

#define CONSTTIME_TRUE_W ~((crypto_word_t)0)
#define CONSTTIME_FALSE_W ((crypto_word_t)0)

// value_barrier_w returns |a|, but prevents GCC and Clang from reasoning about
// the returned value. This is used to mitigate compilers undoing constant-time
// code, until we can express our requirements directly in the language.
//
// Note the compiler is aware that |value_barrier_w| has no side effects and
// always has the same output for a given input. This allows it to eliminate
// dead code, move computations across loops, and vectorize.
static inline crypto_word_t value_barrier_w(crypto_word_t a) {
#if defined(__GNUC__) || defined(__clang__)
  __asm__("" : "+r"(a) : /* no inputs */);
#endif
  return a;
}

// value_barrier_u32 behaves like |value_barrier_w| but takes a |uint32_t|.
static inline uint32_t value_barrier_u32(uint32_t a) {
#if defined(__GNUC__) || defined(__clang__)
  __asm__("" : "+r"(a) : /* no inputs */);
#endif
  return a;
}

// |value_barrier_u8| could be defined as above, but compilers other than
// clang seem to still materialize 0x00..00MM instead of reusing 0x??..??MM.

// constant_time_msb_w returns the given value with the MSB copied to all the
// other bits.
static inline crypto_word_t constant_time_msb_w(crypto_word_t a) {
  return 0u - (a >> (sizeof(a) * 8 - 1));
}

// constant_time_is_zero returns 0xff..f if a == 0 and 0 otherwise.
static inline crypto_word_t constant_time_is_zero_w(crypto_word_t a) {
  // Here is an SMT-LIB verification of this formula:
  //
  // (define-fun is_zero ((a (_ BitVec 32))) (_ BitVec 32)
  //   (bvand (bvnot a) (bvsub a #x00000001))
  // )
  //
  // (declare-fun a () (_ BitVec 32))
  //
  // (assert (not (= (= #x00000001 (bvlshr (is_zero a) #x0000001f)) (= a #x00000000))))
  // (check-sat)
  // (get-model)
  return constant_time_msb_w(~a & (a - 1));
}

static inline crypto_word_t constant_time_is_nonzero_w(crypto_word_t a) {
  return ~constant_time_is_zero_w(a);
}

// constant_time_eq_w returns 0xff..f if a == b and 0 otherwise.
static inline crypto_word_t constant_time_eq_w(crypto_word_t a,
                                               crypto_word_t b) {
  return constant_time_is_zero_w(a ^ b);
}

// constant_time_select_w returns (mask & a) | (~mask & b). When |mask| is all
// 1s or all 0s (as returned by the methods above), the select methods return
// either |a| (if |mask| is nonzero) or |b| (if |mask| is zero).
static inline crypto_word_t constant_time_select_w(crypto_word_t mask,
                                                   crypto_word_t a,
                                                   crypto_word_t b) {
  // Clang recognizes this pattern as a select. While it usually transforms it
  // to a cmov, it sometimes further transforms it into a branch, which we do
  // not want.
  //
  // Hiding the value of the mask from the compiler evades this transformation.
  mask = value_barrier_w(mask);
  return (mask & a) | (~mask & b);
}

// constant_time_select_8 acts like |constant_time_select| but operates on
// 8-bit values.
static inline uint8_t constant_time_select_8(crypto_word_t mask, uint8_t a,
                                             uint8_t b) {
  // |mask| is a word instead of |uint8_t| to avoid materializing 0x000..0MM
  // Making both |mask| and its value barrier |uint8_t| would allow the compiler
  // to materialize 0x????..?MM instead, but only clang is that clever.
  // However, vectorization of bitwise operations seems to work better on
  // |uint8_t| than a mix of |uint64_t| and |uint8_t|, so |m| is cast to
  // |uint8_t| after the value barrier but before the bitwise operations.
  uint8_t m = value_barrier_w(mask);
  return (m & a) | (~m & b);
}

// constant_time_conditional_memcpy copies |n| bytes from |src| to |dst| if
// |mask| is 0xff..ff and does nothing if |mask| is 0. The |n|-byte memory
// ranges at |dst| and |src| must not overlap, as when calling |memcpy|.
static inline void constant_time_conditional_memcpy(void *dst, const void *src,
                                                    const size_t n,
                                                    const crypto_word_t mask) {
  debug_assert_nonsecret(!buffers_alias(dst, n, src, n));
  uint8_t *out = (uint8_t *)dst;
  const uint8_t *in = (const uint8_t *)src;
  for (size_t i = 0; i < n; i++) {
    out[i] = constant_time_select_8(mask, in[i], out[i]);
  }
}

// constant_time_conditional_memxor xors |n| bytes from |src| to |dst| if
// |mask| is 0xff..ff and does nothing if |mask| is 0. The |n|-byte memory
// ranges at |dst| and |src| must not overlap, as when calling |memcpy|.
static inline void constant_time_conditional_memxor(void *dst, const void *src,
                                                    size_t n,
                                                    const crypto_word_t mask) {
  debug_assert_nonsecret(!buffers_alias(dst, n, src, n));
  aliasing_uint8_t *out = dst;
  const aliasing_uint8_t *in = src;
#if defined(__GNUC__) && !defined(__clang__)
  // gcc 13.2.0 doesn't automatically vectorize this loop regardless of barrier
  typedef aliasing_uint8_t v32u8 __attribute__((vector_size(32), aligned(1), may_alias));
  size_t n_vec = n&~(size_t)31;
  v32u8 masks = ((aliasing_uint8_t)mask-(v32u8){}); // broadcast
  for (size_t i = 0; i < n_vec; i += 32) {
    *(v32u8*)&out[i] ^= masks & *(v32u8 const*)&in[i];
  }
  out += n_vec;
  n -= n_vec;
#endif
  for (size_t i = 0; i < n; i++) {
    out[i] ^= value_barrier_w(mask) & in[i];
  }
}

#if defined(BORINGSSL_CONSTANT_TIME_VALIDATION)

// CONSTTIME_SECRET takes a pointer and a number of bytes and marks that region
// of memory as secret. Secret data is tracked as it flows to registers and
// other parts of a memory. If secret data is used as a condition for a branch,
// or as a memory index, it will trigger warnings in valgrind.
#define CONSTTIME_SECRET(ptr, len) VALGRIND_MAKE_MEM_UNDEFINED(ptr, len)

// CONSTTIME_DECLASSIFY takes a pointer and a number of bytes and marks that
// region of memory as public. Public data is not subject to constant-time
// rules.
#define CONSTTIME_DECLASSIFY(ptr, len) VALGRIND_MAKE_MEM_DEFINED(ptr, len)

#else

#define CONSTTIME_SECRET(ptr, len)
#define CONSTTIME_DECLASSIFY(ptr, len)

#endif  // BORINGSSL_CONSTANT_TIME_VALIDATION

static inline crypto_word_t constant_time_declassify_w(crypto_word_t v) {
  // Return |v| through a value barrier to be safe. Valgrind-based constant-time
  // validation is partly to check the compiler has not undone any constant-time
  // work. Any place |BORINGSSL_CONSTANT_TIME_VALIDATION| influences
  // optimizations, this validation is inaccurate.
  //
  // However, by sending pointers through valgrind, we likely inhibit escape
  // analysis. On local variables, particularly booleans, we likely
  // significantly impact optimizations.
  //
  // Thus, to be safe, stick a value barrier, in hopes of comparably inhibiting
  // compiler analysis.
  CONSTTIME_DECLASSIFY(&v, sizeof(v));
  return value_barrier_w(v);
}

static inline int constant_time_declassify_int(int v) {
  OPENSSL_STATIC_ASSERT(sizeof(uint32_t) == sizeof(int),
                "int is not the same size as uint32_t");
  // See comment above.
  CONSTTIME_DECLASSIFY(&v, sizeof(v));
  return value_barrier_u32((uint32_t)v);
}

#if defined(_MSC_VER) && !defined(__clang__)
// '=': conversion from 'int64_t' to 'int32_t', possible loss of data
#pragma warning(pop)
#endif
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif

// declassify_assert behaves like |assert| but declassifies the result of
// evaluating |expr|. This allows the assertion to branch on the (presumably
// public) result, but still ensures that values leading up to the computation
// were secret.
#define declassify_assert(expr) dev_assert_secret(constant_time_declassify_int(expr))

// Endianness conversions.

#if defined(__GNUC__) && __GNUC__ >= 2
static inline uint32_t CRYPTO_bswap4(uint32_t x) {
  return __builtin_bswap32(x);
}

static inline uint64_t CRYPTO_bswap8(uint64_t x) {
  return __builtin_bswap64(x);
}
#elif defined(_MSC_VER)
#pragma warning(push, 3)
#include <stdlib.h>
#pragma warning(pop)
#pragma intrinsic(_byteswap_ulong)
static inline uint32_t CRYPTO_bswap4(uint32_t x) {
  return _byteswap_ulong(x);
}
#endif

#if !defined(RING_CORE_NOSTDLIBINC)
#include <string.h>
#endif

static inline void *OPENSSL_memcpy(void *dst, const void *src, size_t n) {
#if !defined(RING_CORE_NOSTDLIBINC)
  if (n == 0) {
    return dst;
  }
  return memcpy(dst, src, n);
#else
  aliasing_uint8_t *d = dst;
  const aliasing_uint8_t *s = src;
  for (size_t i = 0; i < n; ++i) {
    d[i] = s[i];
  }
  return dst;
#endif
}

static inline void *OPENSSL_memset(void *dst, int c, size_t n) {
#if !defined(RING_CORE_NOSTDLIBINC)
  if (n == 0) {
    return dst;
  }
  return memset(dst, c, n);
#else
  aliasing_uint8_t *d = dst;
  for (size_t i = 0; i < n; ++i) {
    d[i] = (aliasing_uint8_t)c;
  }
  return dst;
#endif
}


// Loads and stores.
//
// The following functions load and store sized integers with the specified
// endianness. They use |memcpy|, and so avoid alignment or strict aliasing
// requirements on the input and output pointers.

#if defined(__BYTE_ORDER__) && defined(__ORDER_BIG_ENDIAN__)
#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
#define RING_BIG_ENDIAN
#endif
#endif

static inline uint32_t CRYPTO_load_u32_le(const void *in) {
  uint32_t v;
  OPENSSL_memcpy(&v, in, sizeof(v));
#if defined(RING_BIG_ENDIAN)
  return CRYPTO_bswap4(v);
#else
  return v;
#endif
}

static inline void CRYPTO_store_u32_le(void *out, uint32_t v) {
#if defined(RING_BIG_ENDIAN)
  v = CRYPTO_bswap4(v);
#endif
  OPENSSL_memcpy(out, &v, sizeof(v));
}

static inline uint32_t CRYPTO_load_u32_be(const void *in) {
  uint32_t v;
  OPENSSL_memcpy(&v, in, sizeof(v));
#if !defined(RING_BIG_ENDIAN)
  return CRYPTO_bswap4(v);
#else
  return v;
#endif
}

static inline void CRYPTO_store_u32_be(void *out, uint32_t v) {
#if !defined(RING_BIG_ENDIAN)
  v = CRYPTO_bswap4(v);
#endif
  OPENSSL_memcpy(out, &v, sizeof(v));
}

// Runtime CPU feature support

#if defined(OPENSSL_X86) || defined(OPENSSL_X86_64)
// OPENSSL_ia32cap_P contains the Intel CPUID bits when running on an x86 or
// x86-64 system.
//
//   Index 0:
//     EDX for CPUID where EAX = 1
//     Bit 30 is used to indicate an Intel CPU
//   Index 1:
//     ECX for CPUID where EAX = 1
//   Index 2:
//     EBX for CPUID where EAX = 7, ECX = 0
//     Bit 14 (for removed feature MPX) is used to indicate a preference for ymm
//       registers over zmm even when zmm registers are supported
//   Index 3:
//     ECX for CPUID where EAX = 7, ECX = 0
//
// Note: the CPUID bits are pre-adjusted for the OSXSAVE bit and the XMM, YMM,
// and AVX512 bits in XCR0, so it is not necessary to check those. (WARNING: See
// caveats in cpu_intel.c.)
#if defined(OPENSSL_X86_64)
extern uint32_t avx2_available;
extern uint32_t adx_bmi2_available;
#endif
#endif


#if defined(OPENSSL_ARM)
extern alignas(4) uint32_t neon_available;
#endif  // OPENSSL_ARM

#endif  // OPENSSL_HEADER_CRYPTO_INTERNAL_H
