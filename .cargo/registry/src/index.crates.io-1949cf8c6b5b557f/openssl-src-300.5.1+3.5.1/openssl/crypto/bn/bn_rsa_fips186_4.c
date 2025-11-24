/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2018-2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * According to NIST SP800-131A "Transitioning the use of cryptographic
 * algorithms and key lengths" Generation of 1024 bit RSA keys are no longer
 * allowed for signatures (Table 2) or key transport (Table 5). In the code
 * below any attempt to generate 1024 bit RSA keys will result in an error (Note
 * that digital signature verification can still use deprecated 1024 bit keys).
 *
 * FIPS 186-4 relies on the use of the auxiliary primes p1, p2, q1 and q2 that
 * must be generated before the module generates the RSA primes p and q.
 * Table B.1 in FIPS 186-4 specifies RSA modulus lengths of 2048 and
 * 3072 bits only, the min/max total length of the auxiliary primes.
 * FIPS 186-5 Table A.1 includes an additional entry for 4096 which has been
 * included here.
 */
#include <stdio.h>
#include <openssl/bn.h>
#include "bn_local.h"
#include "crypto/bn.h"
#include "internal/nelem.h"

#if BN_BITS2 == 64
# define BN_DEF(lo, hi) (BN_ULONG)hi<<32|lo
#else
# define BN_DEF(lo, hi) lo, hi
#endif

/* 1 / sqrt(2) * 2^256, rounded up */
static const BN_ULONG inv_sqrt_2_val[] = {
    BN_DEF(0x83339916UL, 0xED17AC85UL), BN_DEF(0x893BA84CUL, 0x1D6F60BAUL),
    BN_DEF(0x754ABE9FUL, 0x597D89B3UL), BN_DEF(0xF9DE6484UL, 0xB504F333UL)
};

const BIGNUM ossl_bn_inv_sqrt_2 = {
    (BN_ULONG *)inv_sqrt_2_val,
    OSSL_NELEM(inv_sqrt_2_val),
    OSSL_NELEM(inv_sqrt_2_val),
    0,
    BN_FLG_STATIC_DATA
};

/*
 * Refer to FIPS 186-5 Table B.1 for minimum rounds of Miller Rabin
 * required for generation of RSA aux primes (p1, p2, q1 and q2).
 */
static int bn_rsa_fips186_5_aux_prime_MR_rounds(int nbits)
{
    if (nbits >= 4096)
        return 44;
    if (nbits >= 3072)
        return 41;
    if (nbits >= 2048)
        return 38;
    return 0; /* Error */
}

/*
 * Refer to FIPS 186-5 Table B.1 for minimum rounds of Miller Rabin
 * required for generation of RSA primes (p and q)
 */
static int bn_rsa_fips186_5_prime_MR_rounds(int nbits)
{
    if (nbits >= 3072)
        return 4;
    if (nbits >= 2048)
        return 5;
    return 0; /* Error */
}

/*
 * FIPS 186-5 Table A.1. "Min length of auxiliary primes p1, p2, q1, q2".
 * (FIPS 186-5 has an entry for >= 4096 bits).
 *
 * Params:
 *     nbits The key size in bits.
 * Returns:
 *     The minimum size of the auxiliary primes or 0 if nbits is invalid.
 */
static int bn_rsa_fips186_5_aux_prime_min_size(int nbits)
{
    if (nbits >= 4096)
        return 201;
    if (nbits >= 3072)
        return 171;
    if (nbits >= 2048)
        return 141;
    return 0;
}

/*
 * FIPS 186-5 Table A.1 "Max of len(p1) + len(p2) and
 * len(q1) + len(q2) for p,q Probable Primes".
 * (FIPS 186-5 has an entry for >= 4096 bits).
 * Params:
 *     nbits The key size in bits.
 * Returns:
 *     The maximum length or 0 if nbits is invalid.
 */
static int bn_rsa_fips186_5_aux_prime_max_sum_size_for_prob_primes(int nbits)
{
    if (nbits >= 4096)
        return 2030;
    if (nbits >= 3072)
        return 1518;
    if (nbits >= 2048)
        return 1007;
    return 0;
}

/*
 * Find the first odd integer that is a probable prime.
 *
 * See section FIPS 186-4 B.3.6 (Steps 4.2/5.2).
 *
 * Params:
 *     Xp1 The passed in starting point to find a probably prime.
 *     p1 The returned probable prime (first odd integer >= Xp1)
 *     ctx A BN_CTX object.
 *     rounds The number of Miller Rabin rounds
 *     cb An optional BIGNUM callback.
 * Returns: 1 on success otherwise it returns 0.
 */
static int bn_rsa_fips186_4_find_aux_prob_prime(const BIGNUM *Xp1,
                                                BIGNUM *p1, BN_CTX *ctx,
                                                int rounds,
                                                BN_GENCB *cb)
{
    int ret = 0;
    int i = 0;
    int tmp = 0;

    if (BN_copy(p1, Xp1) == NULL)
        return 0;
    BN_set_flags(p1, BN_FLG_CONSTTIME);

    /* Find the first odd number >= Xp1 that is probably prime */
    for (;;) {
        i++;
        BN_GENCB_call(cb, 0, i);
        /* MR test with trial division */
        tmp = ossl_bn_check_generated_prime(p1, rounds, ctx, cb);
        if (tmp > 0)
            break;
        if (tmp < 0)
            goto err;
        /* Get next odd number */
        if (!BN_add_word(p1, 2))
            goto err;
    }
    BN_GENCB_call(cb, 2, i);
    ret = 1;
err:
    return ret;
}

/*
 * Generate a probable prime (p or q).
 *
 * See FIPS 186-4 B.3.6 (Steps 4 & 5)
 *
 * Params:
 *     p The returned probable prime.
 *     Xpout An optionally returned random number used during generation of p.
 *     p1, p2 The returned auxiliary primes. If NULL they are not returned.
 *     Xp An optional passed in value (that is random number used during
 *        generation of p).
 *     Xp1, Xp2 Optional passed in values that are normally generated
 *              internally. Used to find p1, p2.
 *     nlen The bit length of the modulus (the key size).
 *     e The public exponent.
 *     ctx A BN_CTX object.
 *     cb An optional BIGNUM callback.
 * Returns: 1 on success otherwise it returns 0.
 */
int ossl_bn_rsa_fips186_4_gen_prob_primes(BIGNUM *p, BIGNUM *Xpout,
                                          BIGNUM *p1, BIGNUM *p2,
                                          const BIGNUM *Xp, const BIGNUM *Xp1,
                                          const BIGNUM *Xp2, int nlen,
                                          const BIGNUM *e, BN_CTX *ctx,
                                          BN_GENCB *cb)
{
    int ret = 0;
    BIGNUM *p1i = NULL, *p2i = NULL, *Xp1i = NULL, *Xp2i = NULL;
    int bitlen, rounds;

    if (p == NULL || Xpout == NULL)
        return 0;

    BN_CTX_start(ctx);

    p1i = (p1 != NULL) ? p1 : BN_CTX_get(ctx);
    p2i = (p2 != NULL) ? p2 : BN_CTX_get(ctx);
    Xp1i = (Xp1 != NULL) ? (BIGNUM *)Xp1 : BN_CTX_get(ctx);
    Xp2i = (Xp2 != NULL) ? (BIGNUM *)Xp2 : BN_CTX_get(ctx);
    if (p1i == NULL || p2i == NULL || Xp1i == NULL || Xp2i == NULL)
        goto err;

    bitlen = bn_rsa_fips186_5_aux_prime_min_size(nlen);
    if (bitlen == 0)
        goto err;
    rounds = bn_rsa_fips186_5_aux_prime_MR_rounds(nlen);

    /* (Steps 4.1/5.1): Randomly generate Xp1 if it is not passed in */
    if (Xp1 == NULL) {
        /* Set the top and bottom bits to make it odd and the correct size */
        if (!BN_priv_rand_ex(Xp1i, bitlen, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ODD,
                             0, ctx))
            goto err;
    }
    /* (Steps 4.1/5.1): Randomly generate Xp2 if it is not passed in */
    if (Xp2 == NULL) {
        /* Set the top and bottom bits to make it odd and the correct size */
        if (!BN_priv_rand_ex(Xp2i, bitlen, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ODD,
                             0, ctx))
            goto err;
    }

    /* (Steps 4.2/5.2) - find first auxiliary probable primes */
    if (!bn_rsa_fips186_4_find_aux_prob_prime(Xp1i, p1i, ctx, rounds, cb)
            || !bn_rsa_fips186_4_find_aux_prob_prime(Xp2i, p2i, ctx, rounds, cb))
        goto err;
    /* (Table B.1) auxiliary prime Max length check */
    if ((BN_num_bits(p1i) + BN_num_bits(p2i)) >=
            bn_rsa_fips186_5_aux_prime_max_sum_size_for_prob_primes(nlen))
        goto err;
    /* (Steps 4.3/5.3) - generate prime */
    if (!ossl_bn_rsa_fips186_4_derive_prime(p, Xpout, Xp, p1i, p2i, nlen, e,
                                            ctx, cb))
        goto err;
    ret = 1;
err:
    /* Zeroize any internally generated values that are not returned */
    if (p1 == NULL)
        BN_clear(p1i);
    if (p2 == NULL)
        BN_clear(p2i);
    if (Xp1 == NULL)
        BN_clear(Xp1i);
    if (Xp2 == NULL)
        BN_clear(Xp2i);
    BN_CTX_end(ctx);
    return ret;
}

/*
 * Constructs a probable prime (a candidate for p or q) using 2 auxiliary
 * prime numbers and the Chinese Remainder Theorem.
 *
 * See FIPS 186-4 C.9 "Compute a Probable Prime Factor Based on Auxiliary
 * Primes". Used by FIPS 186-4 B.3.6 Section (4.3) for p and Section (5.3) for q.
 *
 * Params:
 *     Y The returned prime factor (private_prime_factor) of the modulus n.
 *     X The returned random number used during generation of the prime factor.
 *     Xin An optional passed in value for X used for testing purposes.
 *     r1 An auxiliary prime.
 *     r2 An auxiliary prime.
 *     nlen The desired length of n (the RSA modulus).
 *     e The public exponent.
 *     ctx A BN_CTX object.
 *     cb An optional BIGNUM callback object.
 * Returns: 1 on success otherwise it returns 0.
 * Assumptions:
 *     Y, X, r1, r2, e are not NULL.
 */
int ossl_bn_rsa_fips186_4_derive_prime(BIGNUM *Y, BIGNUM *X, const BIGNUM *Xin,
                                       const BIGNUM *r1, const BIGNUM *r2,
                                       int nlen, const BIGNUM *e,
                                       BN_CTX *ctx, BN_GENCB *cb)
{
    int ret = 0;
    int i, imax, rounds;
    int bits = nlen >> 1;
    BIGNUM *tmp, *R, *r1r2x2, *y1, *r1x2;
    BIGNUM *base, *range;

    BN_CTX_start(ctx);

    base = BN_CTX_get(ctx);
    range = BN_CTX_get(ctx);
    R = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    r1r2x2 = BN_CTX_get(ctx);
    y1 = BN_CTX_get(ctx);
    r1x2 = BN_CTX_get(ctx);
    if (r1x2 == NULL)
        goto err;

    if (Xin != NULL && BN_copy(X, Xin) == NULL)
        goto err;

    /*
     * We need to generate a random number X in the range
     * 1/sqrt(2) * 2^(nlen/2) <= X < 2^(nlen/2).
     * We can rewrite that as:
     * base = 1/sqrt(2) * 2^(nlen/2)
     * range = ((2^(nlen/2))) - (1/sqrt(2) * 2^(nlen/2))
     * X = base + random(range)
     * We only have the first 256 bit of 1/sqrt(2)
     */
    if (Xin == NULL) {
        if (bits < BN_num_bits(&ossl_bn_inv_sqrt_2))
            goto err;
        if (!BN_lshift(base, &ossl_bn_inv_sqrt_2,
                       bits - BN_num_bits(&ossl_bn_inv_sqrt_2))
            || !BN_lshift(range, BN_value_one(), bits)
            || !BN_sub(range, range, base))
            goto err;
    }

    /*
     * (Step 1) GCD(2r1, r2) = 1.
     *    Note: This algorithm was doing a gcd(2r1, r2)=1 test before doing an
     *    mod_inverse(2r1, r2) which are effectively the same operation.
     *    (The algorithm assumed that the gcd test would be faster). Since the
     *    mod_inverse is currently faster than calling the constant time
     *    BN_gcd(), the call to BN_gcd() has been omitted. The inverse result
     *    is used further down.
     */
    if (!(BN_lshift1(r1x2, r1)
            && (BN_mod_inverse(tmp, r1x2, r2, ctx) != NULL)
            /* (Step 2) R = ((r2^-1 mod 2r1) * r2) - ((2r1^-1 mod r2)*2r1) */
            && (BN_mod_inverse(R, r2, r1x2, ctx) != NULL)
            && BN_mul(R, R, r2, ctx) /* R = (r2^-1 mod 2r1) * r2 */
            && BN_mul(tmp, tmp, r1x2, ctx) /* tmp = (2r1^-1 mod r2)*2r1 */
            && BN_sub(R, R, tmp)
            /* Calculate 2r1r2 */
            && BN_mul(r1r2x2, r1x2, r2, ctx)))
        goto err;
    /* Make positive by adding the modulus */
    if (BN_is_negative(R) && !BN_add(R, R, r1r2x2))
        goto err;

    /*
     * In FIPS 186-4 imax was set to 5 * nlen/2.
     * Analysis by Allen Roginsky
     * (See https://csrc.nist.gov/CSRC/media/Publications/fips/186/4/final/documents/comments-received-fips186-4-december-2015.pdf
     * page 68) indicates this has a 1 in 2 million chance of failure.
     * The number has been updated to 20 * nlen/2 as used in
     * FIPS186-5 Appendix B.9 Step 9.
     */
    rounds = bn_rsa_fips186_5_prime_MR_rounds(nlen);
    imax = 20 * bits; /* max = 20/2 * nbits */
    for (;;) {
        if (Xin == NULL) {
            /*
             * (Step 3) Choose Random X such that
             *    sqrt(2) * 2^(nlen/2-1) <= Random X <= (2^(nlen/2)) - 1.
             */
            if (!BN_priv_rand_range_ex(X, range, 0, ctx) || !BN_add(X, X, base))
                goto err;
        }
        /* (Step 4) Y = X + ((R - X) mod 2r1r2) */
        if (!BN_mod_sub(Y, R, X, r1r2x2, ctx) || !BN_add(Y, Y, X))
            goto err;
        /* (Step 5) */
        i = 0;
        for (;;) {
            /* (Step 6) */
            if (BN_num_bits(Y) > bits) {
                if (Xin == NULL)
                    break; /* Randomly Generated X so Go back to Step 3 */
                else
                    goto err; /* X is not random so it will always fail */
            }
            BN_GENCB_call(cb, 0, 2);

            /* (Step 7) If GCD(Y-1) == 1 & Y is probably prime then return Y */
            if (BN_copy(y1, Y) == NULL
                    || !BN_sub_word(y1, 1))
                goto err;

            if (BN_are_coprime(y1, e, ctx)) {
                int rv = ossl_bn_check_generated_prime(Y, rounds, ctx, cb);

                if (rv > 0)
                    goto end;
                if (rv < 0)
                    goto err;
            }
            /* (Step 8-10) */
            if (++i >= imax) {
                ERR_raise(ERR_LIB_BN, BN_R_NO_PRIME_CANDIDATE);
                goto err;
            }
            if (!BN_add(Y, Y, r1r2x2))
                goto err;
        }
    }
end:
    ret = 1;
    BN_GENCB_call(cb, 3, 0);
err:
    BN_clear(y1);
    BN_CTX_end(ctx);
    return ret;
}
