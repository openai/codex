/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/byteorder.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "crypto/ml_kem.h"
#include "internal/common.h"
#include "internal/constant_time.h"
#include "internal/sha3.h"

#if defined(OPENSSL_CONSTANT_TIME_VALIDATION)
#include <valgrind/memcheck.h>
#endif

#if ML_KEM_SEED_BYTES != ML_KEM_SHARED_SECRET_BYTES + ML_KEM_RANDOM_BYTES
# error "ML-KEM keygen seed length != shared secret + random bytes length"
#endif
#if ML_KEM_SHARED_SECRET_BYTES != ML_KEM_RANDOM_BYTES
# error "Invalid unequal lengths of ML-KEM shared secret and random inputs"
#endif

#if UINT_MAX < UINT32_MAX
# error "Unsupported compiler: sizeof(unsigned int) < sizeof(uint32_t)"
#endif

/* Handy function-like bit-extraction macros */
#define bit0(b)     ((b) & 1)
#define bitn(n, b)  (((b) >> n) & 1)

/*
 * 12 bits are sufficient to losslessly represent values in [0, q-1].
 * INVERSE_DEGREE is (n/2)^-1 mod q; used in inverse NTT.
 */
#define DEGREE          ML_KEM_DEGREE
#define INVERSE_DEGREE  (ML_KEM_PRIME - 2 * 13)
#define LOG2PRIME       12
#define BARRETT_SHIFT   (2 * LOG2PRIME)

#ifdef SHA3_BLOCKSIZE
# define SHAKE128_BLOCKSIZE SHA3_BLOCKSIZE(128)
#endif

/*
 * Return whether a value that can only be 0 or 1 is non-zero, in constant time
 * in practice!  The return value is a mask that is all ones if true, and all
 * zeros otherwise (twos-complement arithmentic assumed for unsigned values).
 *
 * Although this is used in constant-time selects, we omit a value barrier
 * here.  Value barriers impede auto-vectorization (likely because it forces
 * the value to transit through a general-purpose register). On AArch64, this
 * is a difference of 2x.
 *
 * We usually add value barriers to selects because Clang turns consecutive
 * selects with the same condition into a branch instead of CMOV/CSEL. This
 * condition does not occur in Kyber, so omitting it seems to be safe so far,
 * but see |cbd_2|, |cbd_3|, where reduction needs to be specialised to the
 * sign of the input, rather than adding |q| in advance, and using the generic
 * |reduce_once|.  (David Benjamin, Chromium)
 */
#if 0
# define constish_time_non_zero(b) (~constant_time_is_zero(b));
#else
# define constish_time_non_zero(b) (0u - (b))
#endif

/*
 * The scalar rejection-sampling buffer size needs to be a multiple of 12, but
 * is otherwise arbitrary, the preferred block size matches the internal buffer
 * size of SHAKE128, avoiding internal buffering and copying in SHAKE128. That
 * block size of (1600 - 256)/8 bytes, or 168, just happens to divide by 12!
 *
 * If the blocksize is unknown, or is not divisible by 12, 168 is used as a
 * fallback.
 */
#if defined(SHAKE128_BLOCKSIZE) && (SHAKE128_BLOCKSIZE) % 12 == 0
# define SCALAR_SAMPLING_BUFSIZE (SHAKE128_BLOCKSIZE)
#else
# define SCALAR_SAMPLING_BUFSIZE 168
#endif

/*
 * Structure of keys
 */
typedef struct ossl_ml_kem_scalar_st {
    /* On every function entry and exit, 0 <= c[i] < ML_KEM_PRIME. */
    uint16_t c[ML_KEM_DEGREE];
} scalar;

/* Key material allocation layout */
#define DECLARE_ML_KEM_KEYDATA(name, rank, private_sz) \
    struct name##_alloc { \
        /* Public vector |t| */ \
        scalar tbuf[(rank)]; \
        /* Pre-computed matrix |m| (FIPS 203 |A| transpose) */ \
        scalar mbuf[(rank)*(rank)] \
        /* optional private key data */ \
        private_sz \
    }

/* Declare variant-specific public and private storage */
#define DECLARE_ML_KEM_VARIANT_KEYDATA(bits) \
    DECLARE_ML_KEM_KEYDATA(pubkey_##bits, ML_KEM_##bits##_RANK,;); \
    DECLARE_ML_KEM_KEYDATA(prvkey_##bits, ML_KEM_##bits##_RANK,;\
        scalar sbuf[ML_KEM_##bits##_RANK]; \
        uint8_t zbuf[2 * ML_KEM_RANDOM_BYTES];)
DECLARE_ML_KEM_VARIANT_KEYDATA(512);
DECLARE_ML_KEM_VARIANT_KEYDATA(768);
DECLARE_ML_KEM_VARIANT_KEYDATA(1024);
#undef DECLARE_ML_KEM_VARIANT_KEYDATA
#undef DECLARE_ML_KEM_KEYDATA

typedef __owur
int (*CBD_FUNC)(scalar *out, uint8_t in[ML_KEM_RANDOM_BYTES + 1],
                EVP_MD_CTX *mdctx, const ML_KEM_KEY *key);
static void scalar_encode(uint8_t *out, const scalar *s, int bits);

/*
 * The wire-form of a losslessly encoded vector uses 12-bits per element.
 *
 * The wire-form public key consists of the lossless encoding of the public
 * vector |t|, followed by the public seed |rho|.
 *
 * Our serialised private key concatenates serialisations of the private vector
 * |s|, the public key, the public key hash, and the failure secret |z|.
 */
#define VECTOR_BYTES(b)     ((3 * DEGREE / 2) * ML_KEM_##b##_RANK)
#define PUBKEY_BYTES(b)     (VECTOR_BYTES(b) + ML_KEM_RANDOM_BYTES)
#define PRVKEY_BYTES(b)     (2 * PUBKEY_BYTES(b) + ML_KEM_PKHASH_BYTES)

/*
 * Encapsulation produces a vector "u" and a scalar "v", whose coordinates
 * (numbers modulo the ML-KEM prime "q") are lossily encoded using as "du" and
 * "dv" bits, respectively.  This encoding is the ciphertext input for
 * decapsulation.
 */
#define U_VECTOR_BYTES(b)   ((DEGREE / 8) * ML_KEM_##b##_DU * ML_KEM_##b##_RANK)
#define V_SCALAR_BYTES(b)   ((DEGREE / 8) * ML_KEM_##b##_DV)
#define CTEXT_BYTES(b)      (U_VECTOR_BYTES(b) + V_SCALAR_BYTES(b))

#if defined(OPENSSL_CONSTANT_TIME_VALIDATION)

/*
 * CONSTTIME_SECRET takes a pointer and a number of bytes and marks that region
 * of memory as secret. Secret data is tracked as it flows to registers and
 * other parts of a memory. If secret data is used as a condition for a branch,
 * or as a memory index, it will trigger warnings in valgrind.
 */
# define CONSTTIME_SECRET(ptr, len) VALGRIND_MAKE_MEM_UNDEFINED(ptr, len)

/*
 * CONSTTIME_DECLASSIFY takes a pointer and a number of bytes and marks that
 * region of memory as public. Public data is not subject to constant-time
 * rules.
 */
# define CONSTTIME_DECLASSIFY(ptr, len) VALGRIND_MAKE_MEM_DEFINED(ptr, len)

#else

# define CONSTTIME_SECRET(ptr, len)
# define CONSTTIME_DECLASSIFY(ptr, len)

#endif

/*
 * Indices of slots in the vinfo tables below
 */
#define ML_KEM_512_VINFO    0
#define ML_KEM_768_VINFO    1
#define ML_KEM_1024_VINFO   2

/*
 * Per-variant fixed parameters
 */
static const ML_KEM_VINFO vinfo_map[3] = {
    {
        "ML-KEM-512",
        PRVKEY_BYTES(512),
        sizeof(struct prvkey_512_alloc),
        PUBKEY_BYTES(512),
        sizeof(struct pubkey_512_alloc),
        CTEXT_BYTES(512),
        VECTOR_BYTES(512),
        U_VECTOR_BYTES(512),
        EVP_PKEY_ML_KEM_512,
        ML_KEM_512_BITS,
        ML_KEM_512_RANK,
        ML_KEM_512_DU,
        ML_KEM_512_DV,
        ML_KEM_512_SECBITS
    },
    {
        "ML-KEM-768",
        PRVKEY_BYTES(768),
        sizeof(struct prvkey_768_alloc),
        PUBKEY_BYTES(768),
        sizeof(struct pubkey_768_alloc),
        CTEXT_BYTES(768),
        VECTOR_BYTES(768),
        U_VECTOR_BYTES(768),
        EVP_PKEY_ML_KEM_768,
        ML_KEM_768_BITS,
        ML_KEM_768_RANK,
        ML_KEM_768_DU,
        ML_KEM_768_DV,
        ML_KEM_768_SECBITS
    },
    {
        "ML-KEM-1024",
        PRVKEY_BYTES(1024),
        sizeof(struct prvkey_1024_alloc),
        PUBKEY_BYTES(1024),
        sizeof(struct pubkey_1024_alloc),
        CTEXT_BYTES(1024),
        VECTOR_BYTES(1024),
        U_VECTOR_BYTES(1024),
        EVP_PKEY_ML_KEM_1024,
        ML_KEM_1024_BITS,
        ML_KEM_1024_RANK,
        ML_KEM_1024_DU,
        ML_KEM_1024_DV,
        ML_KEM_1024_SECBITS
    }
};

/*
 * Remainders modulo `kPrime`, for sufficiently small inputs, are computed in
 * constant time via Barrett reduction, and a final call to reduce_once(),
 * which reduces inputs that are at most 2*kPrime and is also constant-time.
 */
static const int kPrime = ML_KEM_PRIME;
static const unsigned int kBarrettShift = BARRETT_SHIFT;
static const size_t   kBarrettMultiplier = (1 << BARRETT_SHIFT) / ML_KEM_PRIME;
static const uint16_t kHalfPrime = (ML_KEM_PRIME - 1) / 2;
static const uint16_t kInverseDegree = INVERSE_DEGREE;

/*
 * Python helper:
 *
 * p = 3329
 * def bitreverse(i):
 *     ret = 0
 *     for n in range(7):
 *         bit = i & 1
 *         ret <<= 1
 *         ret |= bit
 *         i >>= 1
 *     return ret
 */

/*-
 * First precomputed array from Appendix A of FIPS 203, or else Python:
 * kNTTRoots = [pow(17, bitreverse(i), p) for i in range(128)]
 */
static const uint16_t kNTTRoots[128] = {
    1,    1729, 2580, 3289, 2642, 630,  1897, 848,
    1062, 1919, 193,  797,  2786, 3260, 569,  1746,
    296,  2447, 1339, 1476, 3046, 56,   2240, 1333,
    1426, 2094, 535,  2882, 2393, 2879, 1974, 821,
    289,  331,  3253, 1756, 1197, 2304, 2277, 2055,
    650,  1977, 2513, 632,  2865, 33,   1320, 1915,
    2319, 1435, 807,  452,  1438, 2868, 1534, 2402,
    2647, 2617, 1481, 648,  2474, 3110, 1227, 910,
    17,   2761, 583,  2649, 1637, 723,  2288, 1100,
    1409, 2662, 3281, 233,  756,  2156, 3015, 3050,
    1703, 1651, 2789, 1789, 1847, 952,  1461, 2687,
    939,  2308, 2437, 2388, 733,  2337, 268,  641,
    1584, 2298, 2037, 3220, 375,  2549, 2090, 1645,
    1063, 319,  2773, 757,  2099, 561,  2466, 2594,
    2804, 1092, 403,  1026, 1143, 2150, 2775, 886,
    1722, 1212, 1874, 1029, 2110, 2935, 885,  2154,
};

/*
 * InverseNTTRoots = [pow(17, -bitreverse(i), p) for i in range(128)]
 * Listed in order of use in the inverse NTT loop (index 0 is skipped):
 *
 *  0, 64, 65, ..., 127, 32, 33, ..., 63, 16, 17, ..., 31, 8, 9, ...
 */
static const uint16_t kInverseNTTRoots[128] = {
    1,    1175, 2444, 394,  1219, 2300, 1455, 2117,
    1607, 2443, 554,  1179, 2186, 2303, 2926, 2237,
    525,  735,  863,  2768, 1230, 2572, 556,  3010,
    2266, 1684, 1239, 780,  2954, 109,  1292, 1031,
    1745, 2688, 3061, 992,  2596, 941,  892,  1021,
    2390, 642,  1868, 2377, 1482, 1540, 540,  1678,
    1626, 279,  314,  1173, 2573, 3096, 48,   667,
    1920, 2229, 1041, 2606, 1692, 680,  2746, 568,
    3312, 2419, 2102, 219,  855,  2681, 1848, 712,
    682,  927,  1795, 461,  1891, 2877, 2522, 1894,
    1010, 1414, 2009, 3296, 464,  2697, 816,  1352,
    2679, 1274, 1052, 1025, 2132, 1573, 76,   2998,
    3040, 2508, 1355, 450,  936,  447,  2794, 1235,
    1903, 1996, 1089, 3273, 283,  1853, 1990, 882,
    3033, 1583, 2760, 69,   543,  2532, 3136, 1410,
    2267, 2481, 1432, 2699, 687,  40,   749,  1600,
};

/*
 * Second precomputed array from Appendix A of FIPS 203 (normalised positive),
 * or else Python:
 * ModRoots = [pow(17, 2*bitreverse(i) + 1, p) for i in range(128)]
 */
static const uint16_t kModRoots[128] = {
    17,   3312, 2761, 568,  583,  2746, 2649, 680,  1637, 1692, 723,  2606,
    2288, 1041, 1100, 2229, 1409, 1920, 2662, 667,  3281, 48,   233,  3096,
    756,  2573, 2156, 1173, 3015, 314,  3050, 279,  1703, 1626, 1651, 1678,
    2789, 540,  1789, 1540, 1847, 1482, 952,  2377, 1461, 1868, 2687, 642,
    939,  2390, 2308, 1021, 2437, 892,  2388, 941,  733,  2596, 2337, 992,
    268,  3061, 641,  2688, 1584, 1745, 2298, 1031, 2037, 1292, 3220, 109,
    375,  2954, 2549, 780,  2090, 1239, 1645, 1684, 1063, 2266, 319,  3010,
    2773, 556,  757,  2572, 2099, 1230, 561,  2768, 2466, 863,  2594, 735,
    2804, 525,  1092, 2237, 403,  2926, 1026, 2303, 1143, 2186, 2150, 1179,
    2775, 554,  886,  2443, 1722, 1607, 1212, 2117, 1874, 1455, 1029, 2300,
    2110, 1219, 2935, 394,  885,  2444, 2154, 1175,
};

/*
 * single_keccak hashes |inlen| bytes from |in| and writes |outlen| bytes of
 * output to |out|. If the |md| specifies a fixed-output function, like
 * SHA3-256, then |outlen| must be the correct length for that function.
 */
static __owur
int single_keccak(uint8_t *out, size_t outlen, const uint8_t *in, size_t inlen,
                  EVP_MD_CTX *mdctx)
{
    unsigned int sz = (unsigned int) outlen;

    if (!EVP_DigestUpdate(mdctx, in, inlen))
        return 0;
    if (EVP_MD_xof(EVP_MD_CTX_get0_md(mdctx)))
        return EVP_DigestFinalXOF(mdctx, out, outlen);
    return EVP_DigestFinal_ex(mdctx, out, &sz)
        && ossl_assert((size_t) sz == outlen);
}

/*
 * FIPS 203, Section 4.1, equation (4.3): PRF. Takes 32+1 input bytes, and uses
 * SHAKE256 to produce the input to SamplePolyCBD_eta: FIPS 203, algorithm 8.
 */
static __owur
int prf(uint8_t *out, size_t len, const uint8_t in[ML_KEM_RANDOM_BYTES + 1],
        EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    return EVP_DigestInit_ex(mdctx, key->shake256_md, NULL)
        && single_keccak(out, len, in, ML_KEM_RANDOM_BYTES + 1, mdctx);
}

/*
 * FIPS 203, Section 4.1, equation (4.4): H.  SHA3-256 hash of a variable
 * length input, producing 32 bytes of output.
 */
static __owur
int hash_h(uint8_t out[ML_KEM_PKHASH_BYTES], const uint8_t *in, size_t len,
           EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    return EVP_DigestInit_ex(mdctx, key->sha3_256_md, NULL)
        && single_keccak(out, ML_KEM_PKHASH_BYTES, in, len, mdctx);
}

/* Incremental hash_h of expanded public key */
static int
hash_h_pubkey(uint8_t pkhash[ML_KEM_PKHASH_BYTES],
              EVP_MD_CTX *mdctx, ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;
    const scalar *t = key->t, *end = t + vinfo->rank;
    unsigned int sz;

    if (!EVP_DigestInit_ex(mdctx, key->sha3_256_md, NULL))
        return 0;

    do {
        uint8_t buf[3 * DEGREE / 2];

        scalar_encode(buf, t++, 12);
        if (!EVP_DigestUpdate(mdctx, buf, sizeof(buf)))
            return 0;
    } while (t < end);

    if (!EVP_DigestUpdate(mdctx, key->rho, ML_KEM_RANDOM_BYTES))
        return 0;
    return EVP_DigestFinal_ex(mdctx, pkhash, &sz)
        && ossl_assert(sz == ML_KEM_PKHASH_BYTES);
}

/*
 * FIPS 203, Section 4.1, equation (4.5): G.  SHA3-512 hash of a variable
 * length input, producing 64 bytes of output, in particular the seeds
 * (d,z) for key generation.
 */
static __owur
int hash_g(uint8_t out[ML_KEM_SEED_BYTES], const uint8_t *in, size_t len,
           EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    return EVP_DigestInit_ex(mdctx, key->sha3_512_md, NULL)
        && single_keccak(out, ML_KEM_SEED_BYTES, in, len, mdctx);
}

/*
 * FIPS 203, Section 4.1, equation (4.4): J. SHAKE256 taking a variable length
 * input to compute a 32-byte implicit rejection shared secret, of the same
 * length as the expected shared secret.  (Computed even on success to avoid
 * side-channel leaks).
 */
static __owur
int kdf(uint8_t out[ML_KEM_SHARED_SECRET_BYTES],
        const uint8_t z[ML_KEM_RANDOM_BYTES],
        const uint8_t *ctext, size_t len,
        EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    return EVP_DigestInit_ex(mdctx, key->shake256_md, NULL)
        && EVP_DigestUpdate(mdctx, z, ML_KEM_RANDOM_BYTES)
        && EVP_DigestUpdate(mdctx, ctext, len)
        && EVP_DigestFinalXOF(mdctx, out, ML_KEM_SHARED_SECRET_BYTES);
}

/*
 * FIPS 203, Section 4.2.2, Algorithm 7: "SampleNTT" (steps 3-17, steps 1, 2
 * are performed by the caller). Rejection-samples a Keccak stream to get
 * uniformly distributed elements in the range [0,q). This is used for matrix
 * expansion and only operates on public inputs.
 */
static __owur
int sample_scalar(scalar *out, EVP_MD_CTX *mdctx)
{
    uint16_t *curr = out->c, *endout = curr + DEGREE;
    uint8_t buf[SCALAR_SAMPLING_BUFSIZE], *in;
    uint8_t *endin = buf + sizeof(buf);
    uint16_t d;
    uint8_t b1, b2, b3;

    do {
        if (!EVP_DigestSqueeze(mdctx, in = buf, sizeof(buf)))
            return 0;
        do {
            b1 = *in++;
            b2 = *in++;
            b3 = *in++;

            if (curr >= endout)
                break;
            if ((d = ((b2 & 0x0f) << 8) + b1) < kPrime)
                *curr++ = d;
            if (curr >= endout)
                break;
            if ((d = (b3 << 4) + (b2 >> 4)) < kPrime)
                *curr++ = d;
        } while (in < endin);
    } while (curr < endout);
    return 1;
}

/*-
 * reduce_once reduces 0 <= x < 2*kPrime, mod kPrime.
 *
 * Subtract |q| if the input is larger, without exposing a side-channel,
 * avoiding the "clangover" attack.  See |constish_time_non_zero| for a
 * discussion on why the value barrier is by default omitted.
 */
static __owur uint16_t reduce_once(uint16_t x)
{
    const uint16_t subtracted = x - kPrime;
    uint16_t mask = constish_time_non_zero(subtracted >> 15);

    return (mask & x) | (~mask & subtracted);
}

/*
 * Constant-time reduce x mod kPrime using Barrett reduction. x must be less
 * than kPrime + 2 * kPrime^2.  This is sufficient to reduce a product of
 * two already reduced u_int16 values, in fact it is sufficient for each
 * to be less than 2^12, because (kPrime * (2 * kPrime + 1)) > 2^24.
 */
static __owur uint16_t reduce(uint32_t x)
{
    uint64_t product = (uint64_t)x * kBarrettMultiplier;
    uint32_t quotient = (uint32_t)(product >> kBarrettShift);
    uint32_t remainder = x - quotient * kPrime;

    return reduce_once(remainder);
}

/* Multiply a scalar by a constant. */
static void scalar_mult_const(scalar *s, uint16_t a)
{
    uint16_t *curr = s->c, *end = curr + DEGREE, tmp;

    do {
        tmp = reduce(*curr * a);
        *curr++ = tmp;
    } while (curr < end);
}

/*-
 * FIPS 203, Section 4.3, Algoritm 9: "NTT".
 * In-place number theoretic transform of a given scalar.  Note that ML-KEM's
 * kPrime 3329 does not have a 512th root of unity, so this transform leaves
 * off the last iteration of the usual FFT code, with the 128 relevant roots of
 * unity being stored in NTTRoots.  This means the output should be seen as 128
 * elements in GF(3329^2), with the coefficients of the elements being
 * consecutive entries in |s->c|.
 */
static void scalar_ntt(scalar *s)
{
    const uint16_t *roots = kNTTRoots;
    uint16_t *end = s->c + DEGREE;
    int offset = DEGREE / 2;

    do {
        uint16_t *curr = s->c, *peer;

        do {
            uint16_t *pause = curr + offset, even, odd;
            uint32_t zeta = *++roots;

            peer = pause;
            do {
                even = *curr;
                odd = reduce(*peer * zeta);
                *peer++ = reduce_once(even - odd + kPrime);
                *curr++ = reduce_once(odd + even);
            } while (curr < pause);
        } while ((curr = peer) < end);
    } while ((offset >>= 1) >= 2);
}

/*-
 * FIPS 203, Section 4.3, Algoritm 10: "NTT^(-1)".
 * In-place inverse number theoretic transform of a given scalar, with pairs of
 * entries of s->v being interpreted as elements of GF(3329^2). Just as with
 * the number theoretic transform, this leaves off the first step of the normal
 * iFFT to account for the fact that 3329 does not have a 512th root of unity,
 * using the precomputed 128 roots of unity stored in InverseNTTRoots.
 */
static void scalar_inverse_ntt(scalar *s)
{
    const uint16_t *roots = kInverseNTTRoots;
    uint16_t *end = s->c + DEGREE;
    int offset = 2;

    do {
        uint16_t *curr = s->c, *peer;

        do {
            uint16_t *pause = curr + offset, even, odd;
            uint32_t zeta = *++roots;

            peer = pause;
            do {
                even = *curr;
                odd = *peer;
                *peer++ = reduce(zeta * (even - odd + kPrime));
                *curr++ = reduce_once(odd + even);
            } while (curr < pause);
        } while ((curr = peer) < end);
    } while ((offset <<= 1) < DEGREE);
    scalar_mult_const(s, kInverseDegree);
}

/* Addition updating the LHS scalar in-place. */
static void scalar_add(scalar *lhs, const scalar *rhs)
{
    int i;

    for (i = 0; i < DEGREE; i++)
        lhs->c[i] = reduce_once(lhs->c[i] + rhs->c[i]);
}

/* Subtraction updating the LHS scalar in-place. */
static void scalar_sub(scalar *lhs, const scalar *rhs)
{
    int i;

    for (i = 0; i < DEGREE; i++)
        lhs->c[i] = reduce_once(lhs->c[i] - rhs->c[i] + kPrime);
}

/*
 * Multiplying two scalars in the number theoretically transformed state. Since
 * 3329 does not have a 512th root of unity, this means we have to interpret
 * the 2*ith and (2*i+1)th entries of the scalar as elements of
 * GF(3329)[X]/(X^2 - 17^(2*bitreverse(i)+1)).
 *
 * The value of 17^(2*bitreverse(i)+1) mod 3329 is stored in the precomputed
 * ModRoots table. Note that our Barrett transform only allows us to multipy
 * two reduced numbers together, so we need some intermediate reduction steps,
 * even if an uint64_t could hold 3 multiplied numbers.
 */
static void scalar_mult(scalar *out, const scalar *lhs,
                        const scalar *rhs)
{
    uint16_t *curr = out->c, *end = curr + DEGREE;
    const uint16_t *lc = lhs->c, *rc = rhs->c;
    const uint16_t *roots = kModRoots;

    do {
        uint32_t l0 = *lc++, r0 = *rc++;
        uint32_t l1 = *lc++, r1 = *rc++;
        uint32_t zetapow = *roots++;

        *curr++ = reduce(l0 * r0 + reduce(l1 * r1) * zetapow);
        *curr++ = reduce(l0 * r1 + l1 * r0);
    } while (curr < end);
}

/* Above, but add the result to an existing scalar */
static ossl_inline
void scalar_mult_add(scalar *out, const scalar *lhs,
                     const scalar *rhs)
{
    uint16_t *curr = out->c, *end = curr + DEGREE;
    const uint16_t *lc = lhs->c, *rc = rhs->c;
    const uint16_t *roots = kModRoots;

    do {
        uint32_t l0 = *lc++, r0 = *rc++;
        uint32_t l1 = *lc++, r1 = *rc++;
        uint16_t *c0 = curr++;
        uint16_t *c1 = curr++;
        uint32_t zetapow = *roots++;

        *c0 = reduce(*c0 + l0 * r0 + reduce(l1 * r1) * zetapow);
        *c1 = reduce(*c1 + l0 * r1 + l1 * r0);
    } while (curr < end);
}

/*-
 * FIPS 203, Section 4.2.1, Algorithm 5: "ByteEncode_d", for 2<=d<=12.
 * Here |bits| is |d|.  For efficiency, we handle the d=1 case separately.
 */
static void scalar_encode(uint8_t *out, const scalar *s, int bits)
{
    const uint16_t *curr = s->c, *end = curr + DEGREE;
    uint64_t accum = 0, element;
    int used = 0;

    do {
        element = *curr++;
        if (used + bits < 64) {
            accum |= element << used;
            used += bits;
        } else if (used + bits > 64) {
            out = OPENSSL_store_u64_le(out, accum | (element << used));
            accum = element >> (64 - used);
            used = (used + bits) - 64;
        } else {
            out = OPENSSL_store_u64_le(out, accum | (element << used));
            accum = 0;
            used = 0;
        }
    } while (curr < end);
}

/*
 * scalar_encode_1 is |scalar_encode| specialised for |bits| == 1.
 */
static void scalar_encode_1(uint8_t out[DEGREE / 8], const scalar *s)
{
    int i, j;
    uint8_t out_byte;

    for (i = 0; i < DEGREE; i += 8) {
        out_byte = 0;
        for (j = 0; j < 8; j++)
            out_byte |= bit0(s->c[i + j]) << j;
        *out = out_byte;
        out++;
    }
}

/*-
 * FIPS 203, Section 4.2.1, Algorithm 6: "ByteDecode_d", for 2<=d<12.
 * Here |bits| is |d|.  For efficiency, we handle the d=1 and d=12 cases
 * separately.
 *
 * scalar_decode parses |DEGREE * bits| bits from |in| into |DEGREE| values in
 * |out|.
 */
static void scalar_decode(scalar *out, const uint8_t *in, int bits)
{
    uint16_t *curr = out->c, *end = curr + DEGREE;
    uint64_t accum = 0;
    int accum_bits = 0, todo = bits;
    uint16_t bitmask = (((uint16_t) 1) << bits) - 1, mask = bitmask;
    uint16_t element = 0;

    do {
        if (accum_bits == 0) {
            in = OPENSSL_load_u64_le(&accum, in);
            accum_bits = 64;
        }
        if (todo == bits && accum_bits >= bits) {
            /* No partial "element", and all the required bits available */
            *curr++ = ((uint16_t) accum) & mask;
            accum >>= bits;
            accum_bits -= bits;
        } else if (accum_bits >= todo) {
            /* A partial "element", and all the required bits available */
            *curr++ = element | ((((uint16_t) accum) & mask) << (bits - todo));
            accum >>= todo;
            accum_bits -= todo;
            element = 0;
            todo = bits;
            mask = bitmask;
        } else {
            /*
             * Only some of the requisite bits accumulated, store |accum_bits|
             * of these in |element|.  The accumulated bitcount becomes 0, but
             * as soon as we have more bits we'll want to merge accum_bits
             * fewer of them into the final |element|.
             *
             * Note that with a 64-bit accumulator and |bits| always 12 or
             * less, if we're here, the previous iteration had all the
             * requisite bits, and so there are no kept bits in |element|.
             */
            element = ((uint16_t) accum) & mask;
            todo -= accum_bits;
            mask = bitmask >> accum_bits;
            accum_bits = 0;
        }
    } while (curr < end);
}

static __owur
int scalar_decode_12(scalar *out, const uint8_t in[3 * DEGREE / 2])
{
    int i;
    uint16_t *c = out->c;

    for (i = 0; i < DEGREE / 2; ++i) {
        uint8_t b1 = *in++;
        uint8_t b2 = *in++;
        uint8_t b3 = *in++;
        int outOfRange1 = (*c++ = b1 | ((b2 & 0x0f) << 8)) >= kPrime;
        int outOfRange2 = (*c++ = (b2 >> 4) | (b3 << 4)) >= kPrime;

        if (outOfRange1 | outOfRange2)
            return 0;
    }
    return 1;
}

/*-
 * scalar_decode_decompress_add is a combination of decoding and decompression
 * both specialised for |bits| == 1, with the result added (and sum reduced) to
 * the output scalar.
 *
 * NOTE: this function MUST not leak an input-data-depedennt timing signal.
 * A timing leak in a related function in the reference Kyber implementation
 * made the "clangover" attack (CVE-2024-37880) possible, giving key recovery
 * for ML-KEM-512 in minutes, provided the attacker has access to precise
 * timing of a CPU performing chosen-ciphertext decap.  Admittedly this is only
 * a risk when private keys are reused (perhaps KEMTLS servers).
 */
static void
scalar_decode_decompress_add(scalar *out, const uint8_t in[DEGREE / 8])
{
    static const uint16_t half_q_plus_1 = (ML_KEM_PRIME >> 1) + 1;
    uint16_t *curr = out->c, *end = curr + DEGREE;
    uint16_t mask;
    uint8_t b;

    /*
     * Add |half_q_plus_1| if the bit is set, without exposing a side-channel,
     * avoiding the "clangover" attack.  See |constish_time_non_zero| for a
     * discussion on why the value barrier is by default omitted.
     */
#define decode_decompress_add_bit                               \
        mask = constish_time_non_zero(bit0(b));                 \
        *curr = reduce_once(*curr + (mask & half_q_plus_1));    \
        curr++;                                                 \
        b >>= 1

    /* Unrolled to process each byte in one iteration */
    do {
        b = *in++;
        decode_decompress_add_bit;
        decode_decompress_add_bit;
        decode_decompress_add_bit;
        decode_decompress_add_bit;

        decode_decompress_add_bit;
        decode_decompress_add_bit;
        decode_decompress_add_bit;
        decode_decompress_add_bit;
    } while (curr < end);
#undef decode_decompress_add_bit
}

/*
 * FIPS 203, Section 4.2.1, Equation (4.7): Compress_d.
 *
 * Compresses (lossily) an input |x| mod 3329 into |bits| many bits by grouping
 * numbers close to each other together. The formula used is
 * round(2^|bits|/kPrime*x) mod 2^|bits|.
 * Uses Barrett reduction to achieve constant time. Since we need both the
 * remainder (for rounding) and the quotient (as the result), we cannot use
 * |reduce| here, but need to do the Barrett reduction directly.
 */
static __owur uint16_t compress(uint16_t x, int bits)
{
    uint32_t shifted = (uint32_t)x << bits;
    uint64_t product = (uint64_t)shifted * kBarrettMultiplier;
    uint32_t quotient = (uint32_t)(product >> kBarrettShift);
    uint32_t remainder = shifted - quotient * kPrime;

    /*
     * Adjust the quotient to round correctly:
     *   0 <= remainder <= kHalfPrime round to 0
     *   kHalfPrime < remainder <= kPrime + kHalfPrime round to 1
     *   kPrime + kHalfPrime < remainder < 2 * kPrime round to 2
     */
    quotient += 1 & constant_time_lt_32(kHalfPrime, remainder);
    quotient += 1 & constant_time_lt_32(kPrime + kHalfPrime, remainder);
    return quotient & ((1 << bits) - 1);
}

/*
 * FIPS 203, Section 4.2.1, Equation (4.8): Decompress_d.

 * Decompresses |x| by using a close equi-distant representative. The formula
 * is round(kPrime/2^|bits|*x). Note that 2^|bits| being the divisor allows us
 * to implement this logic using only bit operations.
 */
static __owur uint16_t decompress(uint16_t x, int bits)
{
    uint32_t product = (uint32_t)x * kPrime;
    uint32_t power = 1 << bits;
    /* This is |product| % power, since |power| is a power of 2. */
    uint32_t remainder = product & (power - 1);
    /* This is |product| / power, since |power| is a power of 2. */
    uint32_t lower = product >> bits;

    /*
     * The rounding logic works since the first half of numbers mod |power|
     * have a 0 as first bit, and the second half has a 1 as first bit, since
     * |power| is a power of 2. As a 12 bit number, |remainder| is always
     * positive, so we will shift in 0s for a right shift.
     */
    return lower + (remainder >> (bits - 1));
}

/*-
 * FIPS 203, Section 4.2.1, Equation (4.7): "Compress_d".
 * In-place lossy rounding of scalars to 2^d bits.
 */
static void scalar_compress(scalar *s, int bits)
{
    int i;

    for (i = 0; i < DEGREE; i++)
        s->c[i] = compress(s->c[i], bits);
}

/*
 * FIPS 203, Section 4.2.1, Equation (4.8): "Decompress_d".
 * In-place approximate recovery of scalars from 2^d bit compression.
 */
static void scalar_decompress(scalar *s, int bits)
{
    int i;

    for (i = 0; i < DEGREE; i++)
        s->c[i] = decompress(s->c[i], bits);
}

/* Addition updating the LHS vector in-place. */
static void vector_add(scalar *lhs, const scalar *rhs, int rank)
{
    do {
        scalar_add(lhs++, rhs++);
    } while (--rank > 0);
}

/*
 * Encodes an entire vector into 32*|rank|*|bits| bytes. Note that since 256
 * (DEGREE) is divisible by 8, the individual vector entries will always fill a
 * whole number of bytes, so we do not need to worry about bit packing here.
 */
static void vector_encode(uint8_t *out, const scalar *a, int bits, int rank)
{
    int stride = bits * DEGREE / 8;

    for (; rank-- > 0; out += stride)
        scalar_encode(out, a++, bits);
}

/*
 * Decodes 32*|rank|*|bits| bytes from |in| into |out|. It returns early
 * if any parsed value is >= |ML_KEM_PRIME|.  The resulting scalars are
 * then decompressed and transformed via the NTT.
 *
 * Note: Used only in decrypt_cpa(), which returns void and so does not check
 * the return value of this function.  Side-channels are fine when the input
 * ciphertext to decap() is simply syntactically invalid.
 */
static void
vector_decode_decompress_ntt(scalar *out, const uint8_t *in, int bits, int rank)
{
    int stride = bits * DEGREE / 8;

    for (; rank-- > 0; in += stride, ++out) {
        scalar_decode(out, in, bits);
        scalar_decompress(out, bits);
        scalar_ntt(out);
    }
}

/* vector_decode(), specialised to bits == 12. */
static __owur
int vector_decode_12(scalar *out, const uint8_t in[3 * DEGREE / 2], int rank)
{
    int stride = 3 * DEGREE / 2;

    for (; rank-- > 0; in += stride)
        if (!scalar_decode_12(out++, in))
            return 0;
    return 1;
}

/* In-place compression of each scalar component */
static void vector_compress(scalar *a, int bits, int rank)
{
    do {
        scalar_compress(a++, bits);
    } while (--rank > 0);
}

/* The output scalar must not overlap with the inputs */
static void inner_product(scalar *out, const scalar *lhs, const scalar *rhs,
                          int rank)
{
    scalar_mult(out, lhs, rhs);
    while (--rank > 0)
        scalar_mult_add(out, ++lhs, ++rhs);
}

/*
 * Here, the output vector must not overlap with the inputs, the result is
 * directly subjected to inverse NTT.
 */
static void
matrix_mult_intt(scalar *out, const scalar *m, const scalar *a, int rank)
{
    const scalar *ar;
    int i, j;

    for (i = rank; i-- > 0; ++out) {
        scalar_mult(out, m++, ar = a);
        for (j = rank - 1; j > 0; --j)
            scalar_mult_add(out, m++, ++ar);
        scalar_inverse_ntt(out);
    }
}

/* Here, the output vector must not overlap with the inputs */
static void
matrix_mult_transpose_add(scalar *out, const scalar *m, const scalar *a, int rank)
{
    const scalar *mc = m, *mr, *ar;
    int i, j;

    for (i = rank; i-- > 0; ++out) {
        scalar_mult_add(out, mr = mc++, ar = a);
        for (j = rank; --j > 0; )
            scalar_mult_add(out, (mr += rank), ++ar);
    }
}

/*-
 * Expands the matrix from a seed for key generation and for encaps-CPA.
 * NOTE: FIPS 203 matrix "A" is the transpose of this matrix, computed
 * by appending the (i,j) indices to the seed in the opposite order!
 *
 * Where FIPS 203 computes t = A * s + e, we use the transpose of "m".
 */
static __owur
int matrix_expand(EVP_MD_CTX *mdctx, ML_KEM_KEY *key)
{
    scalar *out = key->m;
    uint8_t input[ML_KEM_RANDOM_BYTES + 2];
    int rank = key->vinfo->rank;
    int i, j;

    memcpy(input, key->rho, ML_KEM_RANDOM_BYTES);
    for (i = 0; i < rank; i++) {
        for (j = 0; j < rank; j++) {
            input[ML_KEM_RANDOM_BYTES] = i;
            input[ML_KEM_RANDOM_BYTES + 1] = j;
            if (!EVP_DigestInit_ex(mdctx, key->shake128_md, NULL)
                || !EVP_DigestUpdate(mdctx, input, sizeof(input))
                || !sample_scalar(out++, mdctx))
                return 0;
        }
    }
    return 1;
}

/*
 * Algorithm 7 from the spec, with eta fixed to two and the PRF call
 * included. Creates binominally distributed elements by sampling 2*|eta| bits,
 * and setting the coefficient to the count of the first bits minus the count of
 * the second bits, resulting in a centered binomial distribution. Since eta is
 * two this gives -2/2 with a probability of 1/16, -1/1 with probability 1/4,
 * and 0 with probability 3/8.
 */
static __owur
int cbd_2(scalar *out, uint8_t in[ML_KEM_RANDOM_BYTES + 1],
          EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint16_t *curr = out->c, *end = curr + DEGREE;
    uint8_t randbuf[4 * DEGREE / 8], *r = randbuf;  /* 64 * eta slots */
    uint16_t value, mask;
    uint8_t b;

    if (!prf(randbuf, sizeof(randbuf), in, mdctx, key))
        return 0;

    do {
        b = *r++;

        /*
         * Add |kPrime| if |value| underflowed.  See |constish_time_non_zero|
         * for a discussion on why the value barrier is by default omitted.
         * While this could have been written reduce_once(value + kPrime), this
         * is one extra addition and small range of |value| tempts some
         * versions of Clang to emit a branch.
         */
        value = bit0(b) + bitn(1, b);
        value -= bitn(2, b) + bitn(3, b);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);

        value = bitn(4, b) + bitn(5, b);
        value -= bitn(6, b) + bitn(7, b);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);
    } while (curr < end);
    return 1;
}

/*
 * Algorithm 7 from the spec, with eta fixed to three and the PRF call
 * included. Creates binominally distributed elements by sampling 3*|eta| bits,
 * and setting the coefficient to the count of the first bits minus the count of
 * the second bits, resulting in a centered binomial distribution.
 */
static __owur
int cbd_3(scalar *out, uint8_t in[ML_KEM_RANDOM_BYTES + 1],
          EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint16_t *curr = out->c, *end = curr + DEGREE;
    uint8_t randbuf[6 * DEGREE / 8], *r = randbuf;  /* 64 * eta slots */
    uint8_t b1, b2, b3;
    uint16_t value, mask;

    if (!prf(randbuf, sizeof(randbuf), in, mdctx, key))
        return 0;

    do {
        b1 = *r++;
        b2 = *r++;
        b3 = *r++;

        /*
         * Add |kPrime| if |value| underflowed.  See |constish_time_non_zero|
         * for a discussion on why the value barrier is by default omitted.
         * While this could have been written reduce_once(value + kPrime), this
         * is one extra addition and small range of |value| tempts some
         * versions of Clang to emit a branch.
         */
        value = bit0(b1) + bitn(1, b1) + bitn(2, b1);
        value -= bitn(3, b1)  + bitn(4, b1) + bitn(5, b1);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);

        value = bitn(6, b1) + bitn(7, b1) + bit0(b2);
        value -= bitn(1, b2) + bitn(2, b2) + bitn(3, b2);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);

        value = bitn(4, b2) + bitn(5, b2) + bitn(6, b2);
        value -= bitn(7, b2) + bit0(b3) + bitn(1, b3);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);

        value = bitn(2, b3) + bitn(3, b3) + bitn(4, b3);
        value -= bitn(5, b3) + bitn(6, b3) + bitn(7, b3);
        mask = constish_time_non_zero(value >> 15);
        *curr++ = value + (kPrime & mask);
    } while (curr < end);
    return 1;
}

/*
 * Generates a secret vector by using |cbd| with the given seed to generate
 * scalar elements and incrementing |counter| for each slot of the vector.
 */
static __owur
int gencbd_vector(scalar *out, CBD_FUNC cbd, uint8_t *counter,
                  const uint8_t seed[ML_KEM_RANDOM_BYTES], int rank,
                  EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint8_t input[ML_KEM_RANDOM_BYTES + 1];

    memcpy(input, seed, ML_KEM_RANDOM_BYTES);
    do {
        input[ML_KEM_RANDOM_BYTES] = (*counter)++;
        if (!cbd(out++, input, mdctx, key))
            return 0;
    } while (--rank > 0);
    return 1;
}

/*
 * As above plus NTT transform.
 */
static __owur
int gencbd_vector_ntt(scalar *out, CBD_FUNC cbd, uint8_t *counter,
                      const uint8_t seed[ML_KEM_RANDOM_BYTES], int rank,
                      EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint8_t input[ML_KEM_RANDOM_BYTES + 1];

    memcpy(input, seed, ML_KEM_RANDOM_BYTES);
    do {
        input[ML_KEM_RANDOM_BYTES] = (*counter)++;
        if (!cbd(out, input, mdctx, key))
            return 0;
        scalar_ntt(out++);
    } while (--rank > 0);
    return 1;
}

/* The |ETA1| value for ML-KEM-512 is 3, the rest and all ETA2 values are 2. */
#define CBD1(evp_type)  ((evp_type) == EVP_PKEY_ML_KEM_512 ? cbd_3 : cbd_2)

/*
 * FIPS 203, Section 5.2, Algorithm 14: K-PKE.Encrypt.
 *
 * Encrypts a message with given randomness to the ciphertext in |out|. Without
 * applying the Fujisaki-Okamoto transform this would not result in a CCA
 * secure scheme, since lattice schemes are vulnerable to decryption failure
 * oracles.
 *
 * The steps are re-ordered to make more efficient/localised use of storage.
 *
 * Note also that the input public key is assumed to hold a precomputed matrix
 * |A| (our key->m, with the public key holding an expanded (16-bit per scalar
 * coefficient) key->t vector).
 *
 * Caller passes storage in |tmp| for for two temporary vectors.
 */
static __owur
int encrypt_cpa(uint8_t out[ML_KEM_SHARED_SECRET_BYTES],
                const uint8_t message[DEGREE / 8],
                const uint8_t r[ML_KEM_RANDOM_BYTES], scalar *tmp,
                EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;
    CBD_FUNC cbd_1 = CBD1(vinfo->evp_type);
    int rank = vinfo->rank;
    /* We can use tmp[0..rank-1] as storage for |y|, then |e1|, ... */
    scalar *y = &tmp[0], *e1 = y, *e2 = y;
    /* We can use tmp[rank]..tmp[2*rank - 1] for |u| */
    scalar *u = &tmp[rank];
    scalar v;
    uint8_t input[ML_KEM_RANDOM_BYTES + 1];
    uint8_t counter = 0;
    int du = vinfo->du;
    int dv = vinfo->dv;

    /* FIPS 203 "y" vector */
    if (!gencbd_vector_ntt(y, cbd_1, &counter, r, rank, mdctx, key))
        return 0;
    /* FIPS 203 "v" scalar */
    inner_product(&v, key->t, y, rank);
    scalar_inverse_ntt(&v);
    /* FIPS 203 "u" vector */
    matrix_mult_intt(u, key->m, y, rank);

    /* All done with |y|, now free to reuse tmp[0] for FIPS 203 |e1| */
    if (!gencbd_vector(e1, cbd_2, &counter, r, rank, mdctx, key))
        return 0;
    vector_add(u, e1, rank);
    vector_compress(u, du, rank);
    vector_encode(out, u, du, rank);

    /* All done with |e1|, now free to reuse tmp[0] for FIPS 203 |e2| */
    memcpy(input, r, ML_KEM_RANDOM_BYTES);
    input[ML_KEM_RANDOM_BYTES] = counter;
    if (!cbd_2(e2, input, mdctx, key))
        return 0;
    scalar_add(&v, e2);

    /* Combine message with |v| */
    scalar_decode_decompress_add(&v, message);
    scalar_compress(&v, dv);
    scalar_encode(out + vinfo->u_vector_bytes, &v, dv);
    return 1;
}

/*
 * FIPS 203, Section 5.3, Algorithm 15: K-PKE.Decrypt.
 */
static void
decrypt_cpa(uint8_t out[ML_KEM_SHARED_SECRET_BYTES],
            const uint8_t *ctext, scalar *u, const ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;
    scalar v, mask;
    int rank = vinfo->rank;
    int du = vinfo->du;
    int dv = vinfo->dv;

    vector_decode_decompress_ntt(u, ctext, du, rank);
    scalar_decode(&v, ctext + vinfo->u_vector_bytes, dv);
    scalar_decompress(&v, dv);
    inner_product(&mask, key->s, u, rank);
    scalar_inverse_ntt(&mask);
    scalar_sub(&v, &mask);
    scalar_compress(&v, 1);
    scalar_encode_1(out, &v);
}

/*-
 * FIPS 203, Section 7.1, Algorithm 19: "ML-KEM.KeyGen".
 * FIPS 203, Section 7.2, Algorithm 20: "ML-KEM.Encaps".
 *
 * Fills the |out| buffer with the |ek| output of "ML-KEM.KeyGen", or,
 * equivalently, the |ek| input of "ML-KEM.Encaps", i.e. returns the
 * wire-format of an ML-KEM public key.
 */
static void encode_pubkey(uint8_t *out, const ML_KEM_KEY *key)
{
    const uint8_t *rho = key->rho;
    const ML_KEM_VINFO *vinfo = key->vinfo;

    vector_encode(out, key->t, 12, vinfo->rank);
    memcpy(out + vinfo->vector_bytes, rho, ML_KEM_RANDOM_BYTES);
}

/*-
 * FIPS 203, Section 7.1, Algorithm 19: "ML-KEM.KeyGen".
 *
 * Fills the |out| buffer with the |dk| output of "ML-KEM.KeyGen".
 * This matches the input format of parse_prvkey() below.
 */
static void encode_prvkey(uint8_t *out, const ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;

    vector_encode(out, key->s, 12, vinfo->rank);
    out += vinfo->vector_bytes;
    encode_pubkey(out, key);
    out += vinfo->pubkey_bytes;
    memcpy(out, key->pkhash, ML_KEM_PKHASH_BYTES);
    out += ML_KEM_PKHASH_BYTES;
    memcpy(out, key->z, ML_KEM_RANDOM_BYTES);
}

/*-
 * FIPS 203, Section 7.1, Algorithm 19: "ML-KEM.KeyGen".
 * FIPS 203, Section 7.2, Algorithm 20: "ML-KEM.Encaps".
 *
 * This function parses the |in| buffer as the |ek| output of "ML-KEM.KeyGen",
 * or, equivalently, the |ek| input of "ML-KEM.Encaps", i.e. decodes the
 * wire-format of the ML-KEM public key.
 */
static int parse_pubkey(const uint8_t *in, EVP_MD_CTX *mdctx, ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;

    /* Decode and check |t| */
    if (!vector_decode_12(key->t, in, vinfo->rank)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                       "%s invalid public 't' vector",
                       vinfo->algorithm_name);
        return 0;
    }
    /* Save the matrix |m| recovery seed |rho| */
    memcpy(key->rho, in + vinfo->vector_bytes, ML_KEM_RANDOM_BYTES);
    /*
     * Pre-compute the public key hash, needed for both encap and decap.
     * Also pre-compute the matrix expansion, stored with the public key.
     */
    if (!hash_h(key->pkhash, in, vinfo->pubkey_bytes, mdctx, key)
        || !matrix_expand(mdctx, key)) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR,
                       "internal error while parsing %s public key",
                       vinfo->algorithm_name);
        return 0;
    }
    return 1;
}

/*
 * FIPS 203, Section 7.1, Algorithm 19: "ML-KEM.KeyGen".
 *
 * Parses the |in| buffer as a |dk| output of "ML-KEM.KeyGen".
 * This matches the output format of encode_prvkey() above.
 */
static int parse_prvkey(const uint8_t *in, EVP_MD_CTX *mdctx, ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo = key->vinfo;

    /* Decode and check |s|. */
    if (!vector_decode_12(key->s, in, vinfo->rank)) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                       "%s invalid private 's' vector",
                       vinfo->algorithm_name);
        return 0;
    }
    in += vinfo->vector_bytes;

    if (!parse_pubkey(in, mdctx, key))
        return 0;
    in += vinfo->pubkey_bytes;

    /* Check public key hash. */
    if (memcmp(key->pkhash, in, ML_KEM_PKHASH_BYTES) != 0) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                       "%s public key hash mismatch",
                       vinfo->algorithm_name);
        return 0;
    }
    in += ML_KEM_PKHASH_BYTES;

    memcpy(key->z, in, ML_KEM_RANDOM_BYTES);
    return 1;
}

/*
 * FIPS 203, Section 6.1, Algorithm 16: "ML-KEM.KeyGen_internal".
 *
 * The implementation of Section 5.1, Algorithm 13, "K-PKE.KeyGen(d)" is
 * inlined.
 *
 * The caller MUST pass a pre-allocated digest context that is not shared with
 * any concurrent computation.
 *
 * This function optionally outputs the serialised wire-form |ek| public key
 * into the provided |pubenc| buffer, and generates the content of the |rho|,
 * |pkhash|, |t|, |m|, |s| and |z| components of the private |key| (which must
 * have preallocated space for these).
 *
 * Keys are computed from a 32-byte random |d| plus the 1 byte rank for
 * domain separation.  These are concatenated and hashed to produce a pair of
 * 32-byte seeds public "rho", used to generate the matrix, and private "sigma",
 * used to generate the secret vector |s|.
 *
 * The second random input |z| is copied verbatim into the Fujisaki-Okamoto
 * (FO) transform "implicit-rejection" secret (the |z| component of the private
 * key), which thwarts chosen-ciphertext attacks, provided decap() runs in
 * constant time, with no side channel leaks, on all well-formed (valid length,
 * and correctly encoded) ciphertext inputs.
 */
static __owur
int genkey(const uint8_t seed[ML_KEM_SEED_BYTES],
           EVP_MD_CTX *mdctx, uint8_t *pubenc, ML_KEM_KEY *key)
{
    uint8_t hashed[2 * ML_KEM_RANDOM_BYTES];
    const uint8_t *const sigma = hashed + ML_KEM_RANDOM_BYTES;
    uint8_t augmented_seed[ML_KEM_RANDOM_BYTES + 1];
    const ML_KEM_VINFO *vinfo = key->vinfo;
    CBD_FUNC cbd_1 = CBD1(vinfo->evp_type);
    int rank = vinfo->rank;
    uint8_t counter = 0;
    int ret = 0;

    /*
     * Use the "d" seed salted with the rank to derive the public and private
     * seeds rho and sigma.
     */
    memcpy(augmented_seed, seed, ML_KEM_RANDOM_BYTES);
    augmented_seed[ML_KEM_RANDOM_BYTES] = (uint8_t) rank;
    if (!hash_g(hashed, augmented_seed, sizeof(augmented_seed), mdctx, key))
        goto end;
    memcpy(key->rho, hashed, ML_KEM_RANDOM_BYTES);
    /* The |rho| matrix seed is public */
    CONSTTIME_DECLASSIFY(key->rho, ML_KEM_RANDOM_BYTES);

    /* FIPS 203 |e| vector is initial value of key->t */
    if (!matrix_expand(mdctx, key)
        || !gencbd_vector_ntt(key->s, cbd_1, &counter, sigma, rank, mdctx, key)
        || !gencbd_vector_ntt(key->t, cbd_1, &counter, sigma, rank, mdctx, key))
        goto end;

    /* To |e| we now add the product of transpose |m| and |s|, giving |t|. */
    matrix_mult_transpose_add(key->t, key->m, key->s, rank);
    /* The |t| vector is public */
    CONSTTIME_DECLASSIFY(key->t, vinfo->rank * sizeof(scalar));

    if (pubenc == NULL) {
        /* Incremental digest of public key without in-full serialisation. */
        if (!hash_h_pubkey(key->pkhash, mdctx, key))
            goto end;
    } else {
        encode_pubkey(pubenc, key);
        if (!hash_h(key->pkhash, pubenc, vinfo->pubkey_bytes, mdctx, key))
            goto end;
    }

    /* Save |z| portion of seed for "implicit rejection" on failure. */
    memcpy(key->z, seed + ML_KEM_RANDOM_BYTES, ML_KEM_RANDOM_BYTES);

    /* Optionally save the |d| portion of the seed */
    key->d = key->z + ML_KEM_RANDOM_BYTES;
    if (key->prov_flags & ML_KEM_KEY_RETAIN_SEED) {
        memcpy(key->d, seed, ML_KEM_RANDOM_BYTES);
    } else {
        OPENSSL_cleanse(key->d, ML_KEM_RANDOM_BYTES);
        key->d = NULL;
    }

    ret = 1;
 end:
    OPENSSL_cleanse((void *)augmented_seed, ML_KEM_RANDOM_BYTES);
    OPENSSL_cleanse((void *)sigma, ML_KEM_RANDOM_BYTES);
    if (ret == 0) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR,
                       "internal error while generating %s private key",
                       vinfo->algorithm_name);
    }
    return ret;
}

/*-
 * FIPS 203, Section 6.2, Algorithm 17: "ML-KEM.Encaps_internal".
 * This is the deterministic version with randomness supplied externally.
 *
 * The caller must pass space for two vectors in |tmp|.
 * The |ctext| buffer have space for the ciphertext of the ML-KEM variant
 * of the provided key.
 */
static
int encap(uint8_t *ctext, uint8_t secret[ML_KEM_SHARED_SECRET_BYTES],
          const uint8_t entropy[ML_KEM_RANDOM_BYTES],
          scalar *tmp, EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint8_t input[ML_KEM_RANDOM_BYTES + ML_KEM_PKHASH_BYTES];
    uint8_t Kr[ML_KEM_SHARED_SECRET_BYTES + ML_KEM_RANDOM_BYTES];
    uint8_t *r = Kr + ML_KEM_SHARED_SECRET_BYTES;
    int ret;

    memcpy(input, entropy, ML_KEM_RANDOM_BYTES);
    memcpy(input + ML_KEM_RANDOM_BYTES, key->pkhash, ML_KEM_PKHASH_BYTES);
    ret = hash_g(Kr, input, sizeof(input), mdctx, key)
        && encrypt_cpa(ctext, entropy, r, tmp, mdctx, key);
    OPENSSL_cleanse((void *)input, sizeof(input));

    if (ret)
        memcpy(secret, Kr, ML_KEM_SHARED_SECRET_BYTES);
    else
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR,
                       "internal error while performing %s encapsulation",
                       key->vinfo->algorithm_name);
    return ret;
}

/*
 * FIPS 203, Section 6.3, Algorithm 18: ML-KEM.Decaps_internal
 *
 * Barring failure of the supporting SHA3/SHAKE primitives, this is fully
 * deterministic, the randomness for the FO transform is extracted during
 * private key generation.
 *
 * The caller must pass space for two vectors in |tmp|.
 * The |ctext| and |tmp_ctext| buffers must each have space for the ciphertext
 * of the key's ML-KEM variant.
 */
static
int decap(uint8_t secret[ML_KEM_SHARED_SECRET_BYTES],
          const uint8_t *ctext, uint8_t *tmp_ctext, scalar *tmp,
          EVP_MD_CTX *mdctx, const ML_KEM_KEY *key)
{
    uint8_t decrypted[ML_KEM_SHARED_SECRET_BYTES + ML_KEM_PKHASH_BYTES];
    uint8_t failure_key[ML_KEM_RANDOM_BYTES];
    uint8_t Kr[ML_KEM_SHARED_SECRET_BYTES + ML_KEM_RANDOM_BYTES];
    uint8_t *r = Kr + ML_KEM_SHARED_SECRET_BYTES;
    const uint8_t *pkhash = key->pkhash;
    const ML_KEM_VINFO *vinfo = key->vinfo;
    int i;
    uint8_t mask;

    /*
     * If our KDF is unavailable, fail early! Otherwise, keep going ignoring
     * any further errors, returning success, and whatever we got for a shared
     * secret.  The decrypt_cpa() function is just arithmetic on secret data,
     * so should not be subject to failure that makes its output predictable.
     *
     * We guard against "should never happen" catastrophic failure of the
     * "pure" function |hash_g| by overwriting the shared secret with the
     * content of the failure key and returning early, if nevertheless hash_g
     * fails.  This is not constant-time, but a failure of |hash_g| already
     * implies loss of side-channel resistance.
     *
     * The same action is taken, if also |encrypt_cpa| should catastrophically
     * fail, due to failure of the |PRF| underlying the CBD functions.
     */
    if (!kdf(failure_key, key->z, ctext, vinfo->ctext_bytes, mdctx, key)) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR,
                       "internal error while performing %s decapsulation",
                       vinfo->algorithm_name);
        return 0;
    }
    decrypt_cpa(decrypted, ctext, tmp, key);
    memcpy(decrypted + ML_KEM_SHARED_SECRET_BYTES, pkhash, ML_KEM_PKHASH_BYTES);
    if (!hash_g(Kr, decrypted, sizeof(decrypted), mdctx, key)
        || !encrypt_cpa(tmp_ctext, decrypted, r, tmp, mdctx, key)) {
        memcpy(secret, failure_key, ML_KEM_SHARED_SECRET_BYTES);
        OPENSSL_cleanse(decrypted, ML_KEM_SHARED_SECRET_BYTES);
        return 1;
    }
    mask = constant_time_eq_int_8(0,
        CRYPTO_memcmp(ctext, tmp_ctext, vinfo->ctext_bytes));
    for (i = 0; i < ML_KEM_SHARED_SECRET_BYTES; i++)
        secret[i] = constant_time_select_8(mask, Kr[i], failure_key[i]);
    OPENSSL_cleanse(decrypted, ML_KEM_SHARED_SECRET_BYTES);
    OPENSSL_cleanse(Kr, sizeof(Kr));
    return 1;
}

/*
 * After allocating storage for public or private key data, update the key
 * component pointers to reference that storage.
 */
static __owur
int add_storage(scalar *p, int private, ML_KEM_KEY *key)
{
    int rank = key->vinfo->rank;

    if (p == NULL)
        return 0;

    /*
     * We're adding key material, the seed buffer will now hold |rho| and
     * |pkhash|.
     */
    memset(key->seedbuf, 0, sizeof(key->seedbuf));
    key->rho = key->seedbuf;
    key->pkhash = key->seedbuf + ML_KEM_RANDOM_BYTES;
    key->d = key->z = NULL;

    /* A public key needs space for |t| and |m| */
    key->m = (key->t = p) + rank;

    /*
     * A private key also needs space for |s| and |z|.
     * The |z| buffer always includes additional space for |d|, but a key's |d|
     * pointer is left NULL when parsed from the NIST format, which omits that
     * information.  Only keys generated from a (d, z) seed pair will have a
     * non-NULL |d| pointer.
     */
    if (private)
        key->z = (uint8_t *)(rank + (key->s = key->m + rank * rank));
    return 1;
}

/*
 * After freeing the storage associated with a key that failed to be
 * constructed, reset the internal pointers back to NULL.
 */
void
ossl_ml_kem_key_reset(ML_KEM_KEY *key)
{
    if (key->t == NULL)
        return;
    /*-
     * Cleanse any sensitive data:
     * - The private vector |s| is immediately followed by the FO failure
     *   secret |z|, and seed |d|, we can cleanse all three in one call.
     *
     * - Otherwise, when key->d is set, cleanse the stashed seed.
     */
    if (ossl_ml_kem_have_prvkey(key))
        OPENSSL_cleanse(key->s,
                        key->vinfo->rank * sizeof(scalar) + 2 * ML_KEM_RANDOM_BYTES);
    OPENSSL_free(key->t);
    key->d = key->z = (uint8_t *)(key->s = key->m = key->t = NULL);
}

/*
 * ----- API exported to the provider
 *
 * Parameters with an implicit fixed length in the internal static API of each
 * variant have an explicit checked length argument at this layer.
 */

/* Retrieve the parameters of one of the ML-KEM variants */
const ML_KEM_VINFO *ossl_ml_kem_get_vinfo(int evp_type)
{
    switch (evp_type) {
    case EVP_PKEY_ML_KEM_512:
        return &vinfo_map[ML_KEM_512_VINFO];
    case EVP_PKEY_ML_KEM_768:
        return &vinfo_map[ML_KEM_768_VINFO];
    case EVP_PKEY_ML_KEM_1024:
        return &vinfo_map[ML_KEM_1024_VINFO];
    }
    return NULL;
}

ML_KEM_KEY *ossl_ml_kem_key_new(OSSL_LIB_CTX *libctx, const char *properties,
                                int evp_type)
{
    const ML_KEM_VINFO *vinfo = ossl_ml_kem_get_vinfo(evp_type);
    ML_KEM_KEY *key;

    if (vinfo == NULL) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT,
                       "unsupported ML-KEM key type: %d", evp_type);
        return NULL;
    }

    if ((key = OPENSSL_malloc(sizeof(*key))) == NULL)
        return NULL;

    key->vinfo = vinfo;
    key->libctx = libctx;
    key->prov_flags = ML_KEM_KEY_PROV_FLAGS_DEFAULT;
    key->shake128_md = EVP_MD_fetch(libctx, "SHAKE128", properties);
    key->shake256_md = EVP_MD_fetch(libctx, "SHAKE256", properties);
    key->sha3_256_md = EVP_MD_fetch(libctx, "SHA3-256", properties);
    key->sha3_512_md = EVP_MD_fetch(libctx, "SHA3-512", properties);
    key->d = key->z = key->rho = key->pkhash = key->encoded_dk = NULL;
    key->s = key->m = key->t = NULL;

    if (key->shake128_md != NULL
        && key->shake256_md != NULL
        && key->sha3_256_md != NULL
        && key->sha3_512_md != NULL)
        return key;

    ossl_ml_kem_key_free(key);
    ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR,
                   "missing SHA3 digest algorithms while creating %s key",
                   vinfo->algorithm_name);
    return NULL;
}

ML_KEM_KEY *ossl_ml_kem_key_dup(const ML_KEM_KEY *key, int selection)
{
    int ok = 0;
    ML_KEM_KEY *ret;

    /*
     * Partially decoded keys, not yet imported or loaded, should never be
     * duplicated.
     */
    if (ossl_ml_kem_decoded_key(key))
        return NULL;

    if (key == NULL
        || (ret = OPENSSL_memdup(key, sizeof(*key))) == NULL)
        return NULL;
    ret->d = ret->z = ret->rho = ret->pkhash = NULL;
    ret->s = ret->m = ret->t = NULL;

    /* Clear selection bits we can't fulfill */
    if (!ossl_ml_kem_have_pubkey(key))
        selection = 0;
    else if (!ossl_ml_kem_have_prvkey(key))
        selection &= ~OSSL_KEYMGMT_SELECT_PRIVATE_KEY;

    switch (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) {
    case 0:
        ok = 1;
        break;
    case OSSL_KEYMGMT_SELECT_PUBLIC_KEY:
        ok = add_storage(OPENSSL_memdup(key->t, key->vinfo->puballoc), 0, ret);
        ret->rho = ret->seedbuf;
        ret->pkhash = ret->rho + ML_KEM_RANDOM_BYTES;
        break;
    case OSSL_KEYMGMT_SELECT_PRIVATE_KEY:
        ok = add_storage(OPENSSL_memdup(key->t, key->vinfo->prvalloc), 1, ret);
        /* Duplicated keys retain |d|, if available */
        if (key->d != NULL)
            ret->d = ret->z + ML_KEM_RANDOM_BYTES;
        break;
    }

    if (!ok) {
        OPENSSL_free(ret);
        return NULL;
    }

    EVP_MD_up_ref(ret->shake128_md);
    EVP_MD_up_ref(ret->shake256_md);
    EVP_MD_up_ref(ret->sha3_256_md);
    EVP_MD_up_ref(ret->sha3_512_md);

    return ret;
}

void ossl_ml_kem_key_free(ML_KEM_KEY *key)
{
    if (key == NULL)
        return;

    EVP_MD_free(key->shake128_md);
    EVP_MD_free(key->shake256_md);
    EVP_MD_free(key->sha3_256_md);
    EVP_MD_free(key->sha3_512_md);

    if (ossl_ml_kem_decoded_key(key)) {
        OPENSSL_cleanse(key->seedbuf, sizeof(key->seedbuf));
        if (ossl_ml_kem_have_dkenc(key)) {
            OPENSSL_cleanse(key->encoded_dk, key->vinfo->prvkey_bytes);
            OPENSSL_free(key->encoded_dk);
        }
    }
    ossl_ml_kem_key_reset(key);
    OPENSSL_free(key);
}

/* Serialise the public component of an ML-KEM key */
int ossl_ml_kem_encode_public_key(uint8_t *out, size_t len,
                                  const ML_KEM_KEY *key)
{
    if (!ossl_ml_kem_have_pubkey(key)
        || len != key->vinfo->pubkey_bytes)
        return 0;
    encode_pubkey(out, key);
    return 1;
}

/* Serialise an ML-KEM private key */
int ossl_ml_kem_encode_private_key(uint8_t *out, size_t len,
                                   const ML_KEM_KEY *key)
{
    if (!ossl_ml_kem_have_prvkey(key)
        || len != key->vinfo->prvkey_bytes)
        return 0;
    encode_prvkey(out, key);
    return 1;
}

int ossl_ml_kem_encode_seed(uint8_t *out, size_t len,
                            const ML_KEM_KEY *key)
{
    if (key == NULL || key->d == NULL || len != ML_KEM_SEED_BYTES)
        return 0;
    /*
     * Both in the seed buffer, and in the allocated storage, the |d| component
     * of the seed is stored last, so we must copy each separately.
     */
    memcpy(out, key->d, ML_KEM_RANDOM_BYTES);
    out += ML_KEM_RANDOM_BYTES;
    memcpy(out, key->z, ML_KEM_RANDOM_BYTES);
    return 1;
}

/*
 * Stash the seed without (yet) performing a keygen, used during decoding, to
 * avoid an extra keygen if we're only going to export the key again to load
 * into another provider.
 */
ML_KEM_KEY *ossl_ml_kem_set_seed(const uint8_t *seed, size_t seedlen, ML_KEM_KEY *key)
{
    if (key == NULL
        || ossl_ml_kem_have_pubkey(key)
        || ossl_ml_kem_have_seed(key)
        || seedlen != ML_KEM_SEED_BYTES)
        return NULL;
    /*
     * With no public or private key material on hand, we can use the seed
     * buffer for |z| and |d|, in that order.
     */
    key->z = key->seedbuf;
    key->d = key->z + ML_KEM_RANDOM_BYTES;
    memcpy(key->d, seed, ML_KEM_RANDOM_BYTES);
    seed += ML_KEM_RANDOM_BYTES;
    memcpy(key->z, seed, ML_KEM_RANDOM_BYTES);
    return key;
}

/* Parse input as a public key */
int ossl_ml_kem_parse_public_key(const uint8_t *in, size_t len, ML_KEM_KEY *key)
{
    EVP_MD_CTX *mdctx = NULL;
    const ML_KEM_VINFO *vinfo;
    int ret = 0;

    /* Keys with key material are immutable */
    if (key == NULL
        || ossl_ml_kem_have_pubkey(key)
        || ossl_ml_kem_have_dkenc(key))
        return 0;
    vinfo = key->vinfo;

    if (len != vinfo->pubkey_bytes
        || (mdctx = EVP_MD_CTX_new()) == NULL)
        return 0;

    if (add_storage(OPENSSL_malloc(vinfo->puballoc), 0, key))
        ret = parse_pubkey(in, mdctx, key);

    if (!ret)
        ossl_ml_kem_key_reset(key);
    EVP_MD_CTX_free(mdctx);
    return ret;
}

/* Parse input as a new private key */
int ossl_ml_kem_parse_private_key(const uint8_t *in, size_t len,
                                  ML_KEM_KEY *key)
{
    EVP_MD_CTX *mdctx = NULL;
    const ML_KEM_VINFO *vinfo;
    int ret = 0;

    /* Keys with key material are immutable */
    if (key == NULL
        || ossl_ml_kem_have_pubkey(key)
        || ossl_ml_kem_have_dkenc(key))
        return 0;
    vinfo = key->vinfo;

    if (len != vinfo->prvkey_bytes
        || (mdctx = EVP_MD_CTX_new()) == NULL)
        return 0;

    if (add_storage(OPENSSL_malloc(vinfo->prvalloc), 1, key))
        ret = parse_prvkey(in, mdctx, key);

    if (!ret)
        ossl_ml_kem_key_reset(key);
    EVP_MD_CTX_free(mdctx);
    return ret;
}

/*
 * Generate a new keypair, either from the saved seed (when non-null), or from
 * the RNG.
 */
int ossl_ml_kem_genkey(uint8_t *pubenc, size_t publen, ML_KEM_KEY *key)
{
    uint8_t seed[ML_KEM_SEED_BYTES];
    EVP_MD_CTX *mdctx = NULL;
    const ML_KEM_VINFO *vinfo;
    int ret = 0;

    if (key == NULL
        || ossl_ml_kem_have_pubkey(key)
        || ossl_ml_kem_have_dkenc(key))
        return 0;
    vinfo = key->vinfo;

    if (pubenc != NULL && publen != vinfo->pubkey_bytes)
        return 0;

    if (ossl_ml_kem_have_seed(key)) {
        if (!ossl_ml_kem_encode_seed(seed, sizeof(seed), key))
            return 0;
        key->d = key->z = NULL;
    } else if (RAND_priv_bytes_ex(key->libctx, seed, sizeof(seed),
                                  key->vinfo->secbits) <= 0) {
        return 0;
    }

    if ((mdctx = EVP_MD_CTX_new()) == NULL)
        return 0;

    /*
     * Data derived from (d, z) defaults secret, and to avoid side-channel
     * leaks should not influence control flow.
     */
    CONSTTIME_SECRET(seed, ML_KEM_SEED_BYTES);

    if (add_storage(OPENSSL_malloc(vinfo->prvalloc), 1, key))
        ret = genkey(seed, mdctx, pubenc, key);
    OPENSSL_cleanse(seed, sizeof(seed));

    /* Declassify secret inputs and derived outputs before returning control */
    CONSTTIME_DECLASSIFY(seed, ML_KEM_SEED_BYTES);

    EVP_MD_CTX_free(mdctx);
    if (!ret) {
        ossl_ml_kem_key_reset(key);
        return 0;
    }

    /* The public components are already declassified */
    CONSTTIME_DECLASSIFY(key->s, vinfo->rank * sizeof(scalar));
    CONSTTIME_DECLASSIFY(key->z, 2 * ML_KEM_RANDOM_BYTES);
    return 1;
}

/*
 * FIPS 203, Section 6.2, Algorithm 17: ML-KEM.Encaps_internal
 * This is the deterministic version with randomness supplied externally.
 */
int ossl_ml_kem_encap_seed(uint8_t *ctext, size_t clen,
                           uint8_t *shared_secret, size_t slen,
                           const uint8_t *entropy, size_t elen,
                           const ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo;
    EVP_MD_CTX *mdctx;
    int ret = 0;

    if (key == NULL || !ossl_ml_kem_have_pubkey(key))
        return 0;
    vinfo = key->vinfo;

    if (ctext == NULL || clen != vinfo->ctext_bytes
        || shared_secret == NULL || slen != ML_KEM_SHARED_SECRET_BYTES
        || entropy == NULL || elen != ML_KEM_RANDOM_BYTES
        || (mdctx = EVP_MD_CTX_new()) == NULL)
        return 0;
    /*
     * Data derived from the encap entropy defaults secret, and to avoid
     * side-channel leaks should not influence control flow.
     */
    CONSTTIME_SECRET(entropy, elen);

    /*-
     * This avoids the need to handle allocation failures for two (max 2KB
     * each) vectors, that are never retained on return from this function.
     * We stack-allocate these.
     */
#   define case_encap_seed(bits)                                            \
    case EVP_PKEY_ML_KEM_##bits:                                            \
        {                                                                   \
            scalar tmp[2 * ML_KEM_##bits##_RANK];                           \
                                                                            \
            ret = encap(ctext, shared_secret, entropy, tmp, mdctx, key);    \
            OPENSSL_cleanse((void *)tmp, sizeof(tmp));                      \
            break;                                                          \
        }
    switch (vinfo->evp_type) {
    case_encap_seed(512);
    case_encap_seed(768);
    case_encap_seed(1024);
    }
#   undef case_encap_seed

    /* Declassify secret inputs and derived outputs before returning control */
    CONSTTIME_DECLASSIFY(entropy, elen);
    CONSTTIME_DECLASSIFY(ctext, clen);
    CONSTTIME_DECLASSIFY(shared_secret, slen);

    EVP_MD_CTX_free(mdctx);
    return ret;
}

int ossl_ml_kem_encap_rand(uint8_t *ctext, size_t clen,
                           uint8_t *shared_secret, size_t slen,
                           const ML_KEM_KEY *key)
{
    uint8_t r[ML_KEM_RANDOM_BYTES];

    if (key == NULL)
        return 0;

    if (RAND_bytes_ex(key->libctx, r, ML_KEM_RANDOM_BYTES,
                      key->vinfo->secbits) < 1)
        return 0;

    return ossl_ml_kem_encap_seed(ctext, clen, shared_secret, slen,
                                  r, sizeof(r), key);
}

int ossl_ml_kem_decap(uint8_t *shared_secret, size_t slen,
                      const uint8_t *ctext, size_t clen,
                      const ML_KEM_KEY *key)
{
    const ML_KEM_VINFO *vinfo;
    EVP_MD_CTX *mdctx;
    int ret = 0;
#if defined(OPENSSL_CONSTANT_TIME_VALIDATION)
    int classify_bytes;
#endif

    /* Need a private key here */
    if (!ossl_ml_kem_have_prvkey(key))
        return 0;
    vinfo = key->vinfo;

    if (shared_secret == NULL || slen != ML_KEM_SHARED_SECRET_BYTES
        || ctext == NULL || clen != vinfo->ctext_bytes
        || (mdctx = EVP_MD_CTX_new()) == NULL) {
        (void)RAND_bytes_ex(key->libctx, shared_secret,
                            ML_KEM_SHARED_SECRET_BYTES, vinfo->secbits);
        return 0;
    }
#if defined(OPENSSL_CONSTANT_TIME_VALIDATION)
    /*
     * Data derived from |s| and |z| defaults secret, and to avoid side-channel
     * leaks should not influence control flow.
     */
    classify_bytes = 2 * sizeof(scalar) + ML_KEM_RANDOM_BYTES;
    CONSTTIME_SECRET(key->s, classify_bytes);
#endif

    /*-
     * This avoids the need to handle allocation failures for two (max 2KB
     * each) vectors and an encoded ciphertext (max 1568 bytes), that are never
     * retained on return from this function.
     * We stack-allocate these.
     */
#   define case_decap(bits)                                             \
    case EVP_PKEY_ML_KEM_##bits:                                        \
        {                                                               \
            uint8_t cbuf[CTEXT_BYTES(bits)];                            \
            scalar tmp[2 * ML_KEM_##bits##_RANK];                       \
                                                                        \
            ret = decap(shared_secret, ctext, cbuf, tmp, mdctx, key);   \
            OPENSSL_cleanse((void *)tmp, sizeof(tmp));                  \
            break;                                                      \
        }
    switch (vinfo->evp_type) {
    case_decap(512);
    case_decap(768);
    case_decap(1024);
    }

    /* Declassify secret inputs and derived outputs before returning control */
    CONSTTIME_DECLASSIFY(key->s, classify_bytes);
    CONSTTIME_DECLASSIFY(shared_secret, slen);
    EVP_MD_CTX_free(mdctx);

    return ret;
#   undef case_decap
}

int ossl_ml_kem_pubkey_cmp(const ML_KEM_KEY *key1, const ML_KEM_KEY *key2)
{
    /*
     * This handles any unexpected differences in the ML-KEM variant rank,
     * giving different key component structures, barring SHA3-256 hash
     * collisions, the keys are the same size.
     */
    if (ossl_ml_kem_have_pubkey(key1) && ossl_ml_kem_have_pubkey(key2))
        return memcmp(key1->pkhash, key2->pkhash, ML_KEM_PKHASH_BYTES) == 0;

    /*
     * No match if just one of the public keys is not available, otherwise both
     * are unavailable, and for now such keys are considered equal.
     */
    return (ossl_ml_kem_have_pubkey(key1) ^ ossl_ml_kem_have_pubkey(key2));
}
