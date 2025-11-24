/*
 * Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2018-2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include <openssl/bn.h>
#include "crypto/bn.h"
#include "rsa_local.h"

/*
 * Part of the RSA keypair test.
 * Check the Chinese Remainder Theorem components are valid.
 *
 * See SP800-5bBr1
 *   6.4.1.2.3: rsakpv1-crt Step 7
 *   6.4.1.3.3: rsakpv2-crt Step 7
 */
int ossl_rsa_check_crt_components(const RSA *rsa, BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *r = NULL, *p1 = NULL, *q1 = NULL;

    /* check if only some of the crt components are set */
    if (rsa->dmp1 == NULL || rsa->dmq1 == NULL || rsa->iqmp == NULL) {
        if (rsa->dmp1 != NULL || rsa->dmq1 != NULL || rsa->iqmp != NULL)
            return 0;
        return 1; /* return ok if all components are NULL */
    }

    BN_CTX_start(ctx);
    r = BN_CTX_get(ctx);
    p1 = BN_CTX_get(ctx);
    q1 = BN_CTX_get(ctx);
    if (q1 != NULL) {
        BN_set_flags(r, BN_FLG_CONSTTIME);
        BN_set_flags(p1, BN_FLG_CONSTTIME);
        BN_set_flags(q1, BN_FLG_CONSTTIME);
        ret = 1;
    } else {
        ret = 0;
    }
    ret = ret
          /* p1 = p -1 */
          && (BN_copy(p1, rsa->p) != NULL)
          && BN_sub_word(p1, 1)
          /* q1 = q - 1 */
          && (BN_copy(q1, rsa->q) != NULL)
          && BN_sub_word(q1, 1)
          /* (a) 1 < dP < (p – 1). */
          && (BN_cmp(rsa->dmp1, BN_value_one()) > 0)
          && (BN_cmp(rsa->dmp1, p1) < 0)
          /* (b) 1 < dQ < (q - 1). */
          && (BN_cmp(rsa->dmq1, BN_value_one()) > 0)
          && (BN_cmp(rsa->dmq1, q1) < 0)
          /* (c) 1 < qInv < p */
          && (BN_cmp(rsa->iqmp, BN_value_one()) > 0)
          && (BN_cmp(rsa->iqmp, rsa->p) < 0)
          /* (d) 1 = (dP . e) mod (p - 1)*/
          && BN_mod_mul(r, rsa->dmp1, rsa->e, p1, ctx)
          && BN_is_one(r)
          /* (e) 1 = (dQ . e) mod (q - 1) */
          && BN_mod_mul(r, rsa->dmq1, rsa->e, q1, ctx)
          && BN_is_one(r)
          /* (f) 1 = (qInv . q) mod p */
          && BN_mod_mul(r, rsa->iqmp, rsa->q, rsa->p, ctx)
          && BN_is_one(r);
    BN_clear(r);
    BN_clear(p1);
    BN_clear(q1);
    BN_CTX_end(ctx);
    return ret;
}

/*
 * Part of the RSA keypair test.
 * Check that (√2)(2^(nbits/2 - 1) <= p <= 2^(nbits/2) - 1
 *
 * See SP800-5bBr1 6.4.1.2.1 Part 5 (c) & (g) - used for both p and q.
 *
 * (√2)(2^(nbits/2 - 1) = (√2/2)(2^(nbits/2))
 */
int ossl_rsa_check_prime_factor_range(const BIGNUM *p, int nbits, BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *low;
    int shift;

    nbits >>= 1;
    shift = nbits - BN_num_bits(&ossl_bn_inv_sqrt_2);

    /* Upper bound check */
    if (BN_num_bits(p) != nbits)
        return 0;

    BN_CTX_start(ctx);
    low = BN_CTX_get(ctx);
    if (low == NULL)
        goto err;

    /* set low = (√2)(2^(nbits/2 - 1) */
    if (!BN_copy(low, &ossl_bn_inv_sqrt_2))
        goto err;

    if (shift >= 0) {
        /*
         * We don't have all the bits. ossl_bn_inv_sqrt_2 contains a rounded up
         * value, so there is a very low probability that we'll reject a valid
         * value.
         */
        if (!BN_lshift(low, low, shift))
            goto err;
    } else if (!BN_rshift(low, low, -shift)) {
        goto err;
    }
    if (BN_cmp(p, low) <= 0)
        goto err;
    ret = 1;
err:
    BN_CTX_end(ctx);
    return ret;
}

/*
 * Part of the RSA keypair test.
 * Check the prime factor (for either p or q)
 * i.e: p is prime AND GCD(p - 1, e) = 1
 *
 * See SP800-56Br1 6.4.1.2.3 Step 5 (a to d) & (e to h).
 */
int ossl_rsa_check_prime_factor(BIGNUM *p, BIGNUM *e, int nbits, BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *p1 = NULL, *gcd = NULL;

    /* (Steps 5 a-b) prime test */
    if (BN_check_prime(p, ctx, NULL) != 1
            /* (Step 5c) (√2)(2^(nbits/2 - 1) <= p <= 2^(nbits/2 - 1) */
            || ossl_rsa_check_prime_factor_range(p, nbits, ctx) != 1)
        return 0;

    BN_CTX_start(ctx);
    p1 = BN_CTX_get(ctx);
    gcd = BN_CTX_get(ctx);
    if (gcd != NULL) {
        BN_set_flags(p1, BN_FLG_CONSTTIME);
        BN_set_flags(gcd, BN_FLG_CONSTTIME);
        ret = 1;
    } else {
        ret = 0;
    }
    ret = ret
          /* (Step 5d) GCD(p-1, e) = 1 */
          && (BN_copy(p1, p) != NULL)
          && BN_sub_word(p1, 1)
          && BN_gcd(gcd, p1, e, ctx)
          && BN_is_one(gcd);

    BN_clear(p1);
    BN_CTX_end(ctx);
    return ret;
}

/*
 * See SP800-56Br1 6.4.1.2.3 Part 6(a-b) Check the private exponent d
 * satisfies:
 *     (Step 6a) 2^(nBit/2) < d < LCM(p–1, q–1).
 *     (Step 6b) 1 = (d*e) mod LCM(p–1, q–1)
 */
int ossl_rsa_check_private_exponent(const RSA *rsa, int nbits, BN_CTX *ctx)
{
    int ret;
    BIGNUM *r, *p1, *q1, *lcm, *p1q1, *gcd;

    /* (Step 6a) 2^(nbits/2) < d */
    if (BN_num_bits(rsa->d) <= (nbits >> 1))
        return 0;

    BN_CTX_start(ctx);
    r = BN_CTX_get(ctx);
    p1 = BN_CTX_get(ctx);
    q1 = BN_CTX_get(ctx);
    lcm = BN_CTX_get(ctx);
    p1q1 = BN_CTX_get(ctx);
    gcd = BN_CTX_get(ctx);
    if (gcd != NULL) {
        BN_set_flags(r, BN_FLG_CONSTTIME);
        BN_set_flags(p1, BN_FLG_CONSTTIME);
        BN_set_flags(q1, BN_FLG_CONSTTIME);
        BN_set_flags(lcm, BN_FLG_CONSTTIME);
        BN_set_flags(p1q1, BN_FLG_CONSTTIME);
        BN_set_flags(gcd, BN_FLG_CONSTTIME);
        ret = 1;
    } else {
        ret = 0;
    }
    ret = (ret
          /* LCM(p - 1, q - 1) */
          && (ossl_rsa_get_lcm(ctx, rsa->p, rsa->q, lcm, gcd, p1, q1,
                               p1q1) == 1)
          /* (Step 6a) d < LCM(p - 1, q - 1) */
          && (BN_cmp(rsa->d, lcm) < 0)
          /* (Step 6b) 1 = (e . d) mod LCM(p - 1, q - 1) */
          && BN_mod_mul(r, rsa->e, rsa->d, lcm, ctx)
          && BN_is_one(r));

    BN_clear(r);
    BN_clear(p1);
    BN_clear(q1);
    BN_clear(lcm);
    BN_clear(gcd);
    BN_CTX_end(ctx);
    return ret;
}

/*
 * Check exponent is odd.
 * For FIPS also check the bit length is in the range [17..256]
 */
int ossl_rsa_check_public_exponent(const BIGNUM *e)
{
#ifdef FIPS_MODULE
    int bitlen;

    bitlen = BN_num_bits(e);
    return (BN_is_odd(e) && bitlen > 16 && bitlen < 257);
#else
    /* Allow small exponents larger than 1 for legacy purposes */
    return BN_is_odd(e) && BN_cmp(e, BN_value_one()) > 0;
#endif /* FIPS_MODULE */
}

/*
 * SP800-56Br1 6.4.1.2.1 (Step 5i): |p - q| > 2^(nbits/2 - 100)
 * i.e- numbits(p-q-1) > (nbits/2 -100)
 */
int ossl_rsa_check_pminusq_diff(BIGNUM *diff, const BIGNUM *p, const BIGNUM *q,
                           int nbits)
{
    int bitlen = (nbits >> 1) - 100;

    if (!BN_sub(diff, p, q))
        return -1;
    BN_set_negative(diff, 0);

    if (BN_is_zero(diff))
        return 0;

    if (!BN_sub_word(diff, 1))
        return -1;
    return (BN_num_bits(diff) > bitlen);
}

/*
 * return LCM(p-1, q-1)
 *
 * Caller should ensure that lcm, gcd, p1, q1, p1q1 are flagged with
 * BN_FLG_CONSTTIME.
 */
int ossl_rsa_get_lcm(BN_CTX *ctx, const BIGNUM *p, const BIGNUM *q,
                     BIGNUM *lcm, BIGNUM *gcd, BIGNUM *p1, BIGNUM *q1,
                     BIGNUM *p1q1)
{
    return BN_sub(p1, p, BN_value_one())    /* p-1 */
           && BN_sub(q1, q, BN_value_one()) /* q-1 */
           && BN_mul(p1q1, p1, q1, ctx)     /* (p-1)(q-1) */
           && BN_gcd(gcd, p1, q1, ctx)
           && BN_div(lcm, NULL, p1q1, gcd, ctx); /* LCM((p-1, q-1)) */
}

/*
 * SP800-56Br1 6.4.2.2 Partial Public Key Validation for RSA refers to
 * SP800-89 5.3.3 (Explicit) Partial Public Key Validation for RSA
 * caveat is that the modulus must be as specified in SP800-56Br1
 */
int ossl_rsa_sp800_56b_check_public(const RSA *rsa)
{
    int ret = 0, status;
    int nbits;
    BN_CTX *ctx = NULL;
    BIGNUM *gcd = NULL;

    if (rsa->n == NULL || rsa->e == NULL)
        return 0;

    nbits = BN_num_bits(rsa->n);
    if (nbits > OPENSSL_RSA_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_RSA, RSA_R_MODULUS_TOO_LARGE);
        return 0;
    }

#ifdef FIPS_MODULE
    /*
     * (Step a): modulus must be 2048 or 3072 (caveat from SP800-56Br1)
     * NOTE: changed to allow keys >= 2048
     */
    if (!ossl_rsa_sp800_56b_validate_strength(nbits, -1)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_KEY_LENGTH);
        return 0;
    }
#endif
    if (!BN_is_odd(rsa->n)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MODULUS);
        return 0;
    }
    /* (Steps b-c): 2^16 < e < 2^256, n and e must be odd */
    if (!ossl_rsa_check_public_exponent(rsa->e)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_PUB_EXPONENT_OUT_OF_RANGE);
        return 0;
    }

    ctx = BN_CTX_new_ex(rsa->libctx);
    gcd = BN_new();
    if (ctx == NULL || gcd == NULL)
        goto err;

    /* (Steps d-f):
     * The modulus is composite, but not a power of a prime.
     * The modulus has no factors smaller than 752.
     */
    if (!BN_gcd(gcd, rsa->n, ossl_bn_get0_small_factors(), ctx)
        || !BN_is_one(gcd)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MODULUS);
        goto err;
    }

    /* Highest number of MR rounds from FIPS 186-5 Section B.3 Table B.1 */
    ret = ossl_bn_miller_rabin_is_prime(rsa->n, 5, ctx, NULL, 1, &status);
#ifdef FIPS_MODULE
    if (ret != 1 || status != BN_PRIMETEST_COMPOSITE_NOT_POWER_OF_PRIME) {
#else
    if (ret != 1 || (status != BN_PRIMETEST_COMPOSITE_NOT_POWER_OF_PRIME
                     && (nbits >= RSA_MIN_MODULUS_BITS
                         || status != BN_PRIMETEST_COMPOSITE_WITH_FACTOR))) {
#endif
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MODULUS);
        ret = 0;
        goto err;
    }

    ret = 1;
err:
    BN_free(gcd);
    BN_CTX_free(ctx);
    return ret;
}

/*
 * Perform validation of the RSA private key to check that 0 < D < N.
 */
int ossl_rsa_sp800_56b_check_private(const RSA *rsa)
{
    if (rsa->d == NULL || rsa->n == NULL)
        return 0;
    return BN_cmp(rsa->d, BN_value_one()) >= 0 && BN_cmp(rsa->d, rsa->n) < 0;
}

/*
 * RSA key pair validation.
 *
 * SP800-56Br1.
 *    6.4.1.2 "RSAKPV1 Family: RSA Key - Pair Validation with a Fixed Exponent"
 *    6.4.1.3 "RSAKPV2 Family: RSA Key - Pair Validation with a Random Exponent"
 *
 * It uses:
 *     6.4.1.2.3 "rsakpv1 - crt"
 *     6.4.1.3.3 "rsakpv2 - crt"
 */
int ossl_rsa_sp800_56b_check_keypair(const RSA *rsa, const BIGNUM *efixed,
                                     int strength, int nbits)
{
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *r = NULL;

    if (rsa->p == NULL
            || rsa->q == NULL
            || rsa->e == NULL
            || rsa->d == NULL
            || rsa->n == NULL) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_REQUEST);
        return 0;
    }
    /* (Step 1): Check Ranges */
    if (!ossl_rsa_sp800_56b_validate_strength(nbits, strength))
        return 0;

    /* If the exponent is known */
    if (efixed != NULL) {
        /* (2): Check fixed exponent matches public exponent. */
        if (BN_cmp(efixed, rsa->e) != 0) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_REQUEST);
            return 0;
        }
    }
    /* (Step 1.c): e is odd integer 65537 <= e < 2^256 */
    if (!ossl_rsa_check_public_exponent(rsa->e)) {
        /* exponent out of range */
        ERR_raise(ERR_LIB_RSA, RSA_R_PUB_EXPONENT_OUT_OF_RANGE);
        return 0;
    }
    /* (Step 3.b): check the modulus */
    if (nbits != BN_num_bits(rsa->n)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_KEYPAIR);
        return 0;
    }
    /* (Step 3.c): check that the modulus length is a positive even integer */
    if (nbits <= 0 || (nbits & 0x1)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_KEYPAIR);
        return 0;
    }

    ctx = BN_CTX_new_ex(rsa->libctx);
    if (ctx == NULL)
        return 0;

    BN_CTX_start(ctx);
    r = BN_CTX_get(ctx);
    if (r == NULL || !BN_mul(r, rsa->p, rsa->q, ctx))
        goto err;
    /* (Step 4.c): Check n = pq */
    if (BN_cmp(rsa->n, r) != 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_REQUEST);
        goto err;
    }

    /* (Step 5): check prime factors p & q */
    ret = ossl_rsa_check_prime_factor(rsa->p, rsa->e, nbits, ctx)
          && ossl_rsa_check_prime_factor(rsa->q, rsa->e, nbits, ctx)
          && (ossl_rsa_check_pminusq_diff(r, rsa->p, rsa->q, nbits) > 0)
          /* (Step 6): Check the private exponent d */
          && ossl_rsa_check_private_exponent(rsa, nbits, ctx)
          /* 6.4.1.2.3 (Step 7): Check the CRT components */
          && ossl_rsa_check_crt_components(rsa, ctx);
    if (ret != 1)
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_KEYPAIR);

err:
    BN_clear(r);
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ret;
}
