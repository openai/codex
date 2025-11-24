/*
 * Copyright 2014-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_BN_H
# define OSSL_CRYPTO_BN_H
# pragma once

# include <openssl/bn.h>
# include <limits.h>

BIGNUM *bn_wexpand(BIGNUM *a, int words);
BIGNUM *bn_expand2(BIGNUM *a, int words);

void bn_correct_top(BIGNUM *a);

/*
 * Determine the modified width-(w+1) Non-Adjacent Form (wNAF) of 'scalar'.
 * This is an array r[] of values that are either zero or odd with an
 * absolute value less than 2^w satisfying scalar = \sum_j r[j]*2^j where at
 * most one of any w+1 consecutive digits is non-zero with the exception that
 * the most significant digit may be only w-1 zeros away from that next
 * non-zero digit.
 */
signed char *bn_compute_wNAF(const BIGNUM *scalar, int w, size_t *ret_len);

int bn_get_top(const BIGNUM *a);

int bn_get_dmax(const BIGNUM *a);

/* Set all words to zero */
void bn_set_all_zero(BIGNUM *a);

/*
 * Copy the internal BIGNUM words into out which holds size elements (and size
 * must be bigger than top)
 */
int bn_copy_words(BN_ULONG *out, const BIGNUM *in, int size);

BN_ULONG *bn_get_words(const BIGNUM *a);

/*
 * Set the internal data words in a to point to words which contains size
 * elements. The BN_FLG_STATIC_DATA flag is set
 */
void bn_set_static_words(BIGNUM *a, const BN_ULONG *words, int size);

/*
 * Copy words into the BIGNUM |a|, reallocating space as necessary.
 * The negative flag of |a| is not modified.
 * Returns 1 on success and 0 on failure.
 */
/*
 * |num_words| is int because bn_expand2 takes an int. This is an internal
 * function so we simply trust callers not to pass negative values.
 */
int bn_set_words(BIGNUM *a, const BN_ULONG *words, int num_words);

/*
 * Some BIGNUM functions assume most significant limb to be non-zero, which
 * is customarily arranged by bn_correct_top. Output from below functions
 * is not processed with bn_correct_top, and for this reason it may not be
 * returned out of public API. It may only be passed internally into other
 * functions known to support non-minimal or zero-padded BIGNUMs. Even
 * though the goal is to facilitate constant-time-ness, not each subroutine
 * is constant-time by itself. They all have pre-conditions, consult source
 * code...
 */
int bn_mul_mont_fixed_top(BIGNUM *r, const BIGNUM *a, const BIGNUM *b,
                          BN_MONT_CTX *mont, BN_CTX *ctx);
int bn_mod_exp_mont_fixed_top(BIGNUM *rr, const BIGNUM *a, const BIGNUM *p,
                              const BIGNUM *m, BN_CTX *ctx,
                              BN_MONT_CTX *in_mont);
int bn_to_mont_fixed_top(BIGNUM *r, const BIGNUM *a, BN_MONT_CTX *mont,
                         BN_CTX *ctx);
int bn_from_mont_fixed_top(BIGNUM *r, const BIGNUM *a, BN_MONT_CTX *mont,
                           BN_CTX *ctx);
int bn_mod_add_fixed_top(BIGNUM *r, const BIGNUM *a, const BIGNUM *b,
                         const BIGNUM *m);
int bn_mod_sub_fixed_top(BIGNUM *r, const BIGNUM *a, const BIGNUM *b,
                         const BIGNUM *m);
int bn_mul_fixed_top(BIGNUM *r, const BIGNUM *a, const BIGNUM *b, BN_CTX *ctx);
int bn_sqr_fixed_top(BIGNUM *r, const BIGNUM *a, BN_CTX *ctx);
int bn_lshift_fixed_top(BIGNUM *r, const BIGNUM *a, int n);
int bn_rshift_fixed_top(BIGNUM *r, const BIGNUM *a, int n);
int bn_div_fixed_top(BIGNUM *dv, BIGNUM *rem, const BIGNUM *m,
                     const BIGNUM *d, BN_CTX *ctx);
int ossl_bn_mask_bits_fixed_top(BIGNUM *a, int n);
int ossl_bn_is_word_fixed_top(const BIGNUM *a, const BN_ULONG w);
int ossl_bn_priv_rand_range_fixed_top(BIGNUM *r, const BIGNUM *range,
                                      unsigned int strength, BN_CTX *ctx);
int ossl_bn_gen_dsa_nonce_fixed_top(BIGNUM *out, const BIGNUM *range,
                                    const BIGNUM *priv,
                                    const unsigned char *message,
                                    size_t message_len, BN_CTX *ctx);

#define BN_PRIMETEST_COMPOSITE                    0
#define BN_PRIMETEST_COMPOSITE_WITH_FACTOR        1
#define BN_PRIMETEST_COMPOSITE_NOT_POWER_OF_PRIME 2
#define BN_PRIMETEST_PROBABLY_PRIME               3

int ossl_bn_miller_rabin_is_prime(const BIGNUM *w, int iterations, BN_CTX *ctx,
                                  BN_GENCB *cb, int enhanced, int *status);
int ossl_bn_check_generated_prime(const BIGNUM *w, int checks, BN_CTX *ctx,
                                  BN_GENCB *cb);

const BIGNUM *ossl_bn_get0_small_factors(void);

int ossl_bn_rsa_fips186_4_gen_prob_primes(BIGNUM *p, BIGNUM *Xpout,
                                          BIGNUM *p1, BIGNUM *p2,
                                          const BIGNUM *Xp, const BIGNUM *Xp1,
                                          const BIGNUM *Xp2, int nlen,
                                          const BIGNUM *e, BN_CTX *ctx,
                                          BN_GENCB *cb);

int ossl_bn_rsa_fips186_4_derive_prime(BIGNUM *Y, BIGNUM *X, const BIGNUM *Xin,
                                       const BIGNUM *r1, const BIGNUM *r2,
                                       int nlen, const BIGNUM *e, BN_CTX *ctx,
                                       BN_GENCB *cb);

OSSL_LIB_CTX *ossl_bn_get_libctx(BN_CTX *ctx);

extern const BIGNUM ossl_bn_inv_sqrt_2;

#if defined(OPENSSL_SYS_LINUX) && !defined(FIPS_MODULE) && defined (__s390x__) \
    && !defined (OPENSSL_NO_ASM)
# define S390X_MOD_EXP
#endif

int s390x_mod_exp(BIGNUM *r, const BIGNUM *a, const BIGNUM *p,
                const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx);
int s390x_crt(BIGNUM *r, const BIGNUM *i, const BIGNUM *p, const BIGNUM *q,
            const BIGNUM *dmp, const BIGNUM *dmq, const BIGNUM *iqmp);

#endif

int ossl_bn_mont_ctx_set(BN_MONT_CTX *ctx, const BIGNUM *modulus, int ri,
                         const unsigned char *rr, size_t rrlen,
                         uint32_t nlo, uint32_t nhi);

int ossl_bn_mont_ctx_eq(const BN_MONT_CTX *m1, const BN_MONT_CTX *m2);
