/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include "internal/cryptlib.h"
#include "bn_local.h"

/*
 * The quick sieve algorithm approach to weeding out primes is Philip
 * Zimmermann's, as implemented in PGP.  I have had a read of his comments
 * and implemented my own version.
 */
#include "bn_prime.h"

static int probable_prime(BIGNUM *rnd, int bits, int safe, prime_t *mods,
                          BN_CTX *ctx);
static int probable_prime_dh(BIGNUM *rnd, int bits, int safe, prime_t *mods,
                             const BIGNUM *add, const BIGNUM *rem,
                             BN_CTX *ctx);
static int bn_is_prime_int(const BIGNUM *w, int checks, BN_CTX *ctx,
                           int do_trial_division, BN_GENCB *cb);

#define square(x) ((BN_ULONG)(x) * (BN_ULONG)(x))

#if BN_BITS2 == 64
# define BN_DEF(lo, hi) (BN_ULONG)hi<<32|lo
#else
# define BN_DEF(lo, hi) lo, hi
#endif

/*
 * See SP800 89 5.3.3 (Step f)
 * The product of the set of primes ranging from 3 to 751
 * Generated using process in test/bn_internal_test.c test_bn_small_factors().
 * This includes 751 (which is not currently included in SP 800-89).
 */
static const BN_ULONG small_prime_factors[] = {
    BN_DEF(0x3ef4e3e1, 0xc4309333), BN_DEF(0xcd2d655f, 0x71161eb6),
    BN_DEF(0x0bf94862, 0x95e2238c), BN_DEF(0x24f7912b, 0x3eb233d3),
    BN_DEF(0xbf26c483, 0x6b55514b), BN_DEF(0x5a144871, 0x0a84d817),
    BN_DEF(0x9b82210a, 0x77d12fee), BN_DEF(0x97f050b3, 0xdb5b93c2),
    BN_DEF(0x4d6c026b, 0x4acad6b9), BN_DEF(0x54aec893, 0xeb7751f3),
    BN_DEF(0x36bc85c4, 0xdba53368), BN_DEF(0x7f5ec78e, 0xd85a1b28),
    BN_DEF(0x6b322244, 0x2eb072d8), BN_DEF(0x5e2b3aea, 0xbba51112),
    BN_DEF(0x0e2486bf, 0x36ed1a6c), BN_DEF(0xec0c5727, 0x5f270460),
    (BN_ULONG)0x000017b1
};

#define BN_SMALL_PRIME_FACTORS_TOP OSSL_NELEM(small_prime_factors)
static const BIGNUM _bignum_small_prime_factors = {
    (BN_ULONG *)small_prime_factors,
    BN_SMALL_PRIME_FACTORS_TOP,
    BN_SMALL_PRIME_FACTORS_TOP,
    0,
    BN_FLG_STATIC_DATA
};

const BIGNUM *ossl_bn_get0_small_factors(void)
{
    return &_bignum_small_prime_factors;
}

/*
 * Calculate the number of trial divisions that gives the best speed in
 * combination with Miller-Rabin prime test, based on the sized of the prime.
 */
static int calc_trial_divisions(int bits)
{
    if (bits <= 512)
        return 64;
    else if (bits <= 1024)
        return 128;
    else if (bits <= 2048)
        return 384;
    else if (bits <= 4096)
        return 1024;
    return NUMPRIMES;
}

/*
 * Use a minimum of 64 rounds of Miller-Rabin, which should give a false
 * positive rate of 2^-128. If the size of the prime is larger than 2048
 * the user probably wants a higher security level than 128, so switch
 * to 128 rounds giving a false positive rate of 2^-256.
 * Returns the number of rounds.
 */
static int bn_mr_min_checks(int bits)
{
    if (bits > 2048)
        return 128;
    return 64;
}

int BN_GENCB_call(BN_GENCB *cb, int a, int b)
{
    /* No callback means continue */
    if (!cb)
        return 1;
    switch (cb->ver) {
    case 1:
        /* Deprecated-style callbacks */
        if (!cb->cb.cb_1)
            return 1;
        cb->cb.cb_1(a, b, cb->arg);
        return 1;
    case 2:
        /* New-style callbacks */
        return cb->cb.cb_2(a, b, cb);
    default:
        break;
    }
    /* Unrecognised callback type */
    return 0;
}

int BN_generate_prime_ex2(BIGNUM *ret, int bits, int safe,
                          const BIGNUM *add, const BIGNUM *rem, BN_GENCB *cb,
                          BN_CTX *ctx)
{
    BIGNUM *t;
    int found = 0;
    int i, j, c1 = 0;
    prime_t *mods = NULL;
    int checks = bn_mr_min_checks(bits);

    if (bits < 2) {
        /* There are no prime numbers this small. */
        ERR_raise(ERR_LIB_BN, BN_R_BITS_TOO_SMALL);
        return 0;
    } else if (add == NULL && safe && bits < 6 && bits != 3) {
        /*
         * The smallest safe prime (7) is three bits.
         * But the following two safe primes with less than 6 bits (11, 23)
         * are unreachable for BN_rand with BN_RAND_TOP_TWO.
         */
        ERR_raise(ERR_LIB_BN, BN_R_BITS_TOO_SMALL);
        return 0;
    }

    mods = OPENSSL_zalloc(sizeof(*mods) * NUMPRIMES);
    if (mods == NULL)
        return 0;

    BN_CTX_start(ctx);
    t = BN_CTX_get(ctx);
    if (t == NULL)
        goto err;
 loop:
    /* make a random number and set the top and bottom bits */
    if (add == NULL) {
        if (!probable_prime(ret, bits, safe, mods, ctx))
            goto err;
    } else {
        if (!probable_prime_dh(ret, bits, safe, mods, add, rem, ctx))
            goto err;
    }

    if (!BN_GENCB_call(cb, 0, c1++))
        /* aborted */
        goto err;

    if (!safe) {
        i = bn_is_prime_int(ret, checks, ctx, 0, cb);
        if (i == -1)
            goto err;
        if (i == 0)
            goto loop;
    } else {
        /*
         * for "safe prime" generation, check that (p-1)/2 is prime. Since a
         * prime is odd, We just need to divide by 2
         */
        if (!BN_rshift1(t, ret))
            goto err;

        for (i = 0; i < checks; i++) {
            j = bn_is_prime_int(ret, 1, ctx, 0, cb);
            if (j == -1)
                goto err;
            if (j == 0)
                goto loop;

            j = bn_is_prime_int(t, 1, ctx, 0, cb);
            if (j == -1)
                goto err;
            if (j == 0)
                goto loop;

            if (!BN_GENCB_call(cb, 2, c1 - 1))
                goto err;
            /* We have a safe prime test pass */
        }
    }
    /* we have a prime :-) */
    found = 1;
 err:
    OPENSSL_free(mods);
    BN_CTX_end(ctx);
    bn_check_top(ret);
    return found;
}

#ifndef FIPS_MODULE
int BN_generate_prime_ex(BIGNUM *ret, int bits, int safe,
                         const BIGNUM *add, const BIGNUM *rem, BN_GENCB *cb)
{
    BN_CTX *ctx = BN_CTX_new();
    int retval;

    if (ctx == NULL)
        return 0;

    retval = BN_generate_prime_ex2(ret, bits, safe, add, rem, cb, ctx);

    BN_CTX_free(ctx);
    return retval;
}
#endif

#ifndef OPENSSL_NO_DEPRECATED_3_0
int BN_is_prime_ex(const BIGNUM *a, int checks, BN_CTX *ctx_passed,
                   BN_GENCB *cb)
{
    return ossl_bn_check_prime(a, checks, ctx_passed, 0, cb);
}

int BN_is_prime_fasttest_ex(const BIGNUM *w, int checks, BN_CTX *ctx,
                            int do_trial_division, BN_GENCB *cb)
{
    return ossl_bn_check_prime(w, checks, ctx, do_trial_division, cb);
}
#endif

/* Wrapper around bn_is_prime_int that sets the minimum number of checks */
int ossl_bn_check_prime(const BIGNUM *w, int checks, BN_CTX *ctx,
                        int do_trial_division, BN_GENCB *cb)
{
    int min_checks = bn_mr_min_checks(BN_num_bits(w));

    if (checks < min_checks)
        checks = min_checks;

    return bn_is_prime_int(w, checks, ctx, do_trial_division, cb);
}

/*
 * Use this only for key generation.
 * It always uses trial division. The number of checks
 * (MR rounds) passed in is used without being clamped to a minimum value.
 */
int ossl_bn_check_generated_prime(const BIGNUM *w, int checks, BN_CTX *ctx,
                                  BN_GENCB *cb)
{
    return bn_is_prime_int(w, checks, ctx, 1, cb);
}

int BN_check_prime(const BIGNUM *p, BN_CTX *ctx, BN_GENCB *cb)
{
    return ossl_bn_check_prime(p, 0, ctx, 1, cb);
}

/*
 * Tests that |w| is probably prime
 * See FIPS 186-4 C.3.1 Miller Rabin Probabilistic Primality Test.
 *
 * Returns 0 when composite, 1 when probable prime, -1 on error.
 */
static int bn_is_prime_int(const BIGNUM *w, int checks, BN_CTX *ctx,
                           int do_trial_division, BN_GENCB *cb)
{
    int i, status, ret = -1;
#ifndef FIPS_MODULE
    BN_CTX *ctxlocal = NULL;
#else

    if (ctx == NULL)
        return -1;
#endif

    /* w must be bigger than 1 */
    if (BN_cmp(w, BN_value_one()) <= 0)
        return 0;

    /* w must be odd */
    if (BN_is_odd(w)) {
        /* Take care of the really small prime 3 */
        if (BN_is_word(w, 3))
            return 1;
    } else {
        /* 2 is the only even prime */
        return BN_is_word(w, 2);
    }

    /* first look for small factors */
    if (do_trial_division) {
        int trial_divisions = calc_trial_divisions(BN_num_bits(w));

        for (i = 1; i < trial_divisions; i++) {
            BN_ULONG mod = BN_mod_word(w, primes[i]);
            if (mod == (BN_ULONG)-1)
                return -1;
            if (mod == 0)
                return BN_is_word(w, primes[i]);
        }
        if (!BN_GENCB_call(cb, 1, -1))
            return -1;
    }
#ifndef FIPS_MODULE
    if (ctx == NULL && (ctxlocal = ctx = BN_CTX_new()) == NULL)
        goto err;
#endif

    if (!ossl_bn_miller_rabin_is_prime(w, checks, ctx, cb, 0, &status)) {
        ret = -1;
        goto err;
    }
    ret = (status == BN_PRIMETEST_PROBABLY_PRIME);
err:
#ifndef FIPS_MODULE
    BN_CTX_free(ctxlocal);
#endif
    return ret;
}

/*
 * Refer to FIPS 186-4 C.3.2 Enhanced Miller-Rabin Probabilistic Primality Test.
 * OR C.3.1 Miller-Rabin Probabilistic Primality Test (if enhanced is zero).
 * The Step numbers listed in the code refer to the enhanced case.
 *
 * if enhanced is set, then status returns one of the following:
 *     BN_PRIMETEST_PROBABLY_PRIME
 *     BN_PRIMETEST_COMPOSITE_WITH_FACTOR
 *     BN_PRIMETEST_COMPOSITE_NOT_POWER_OF_PRIME
 * if enhanced is zero, then status returns either
 *     BN_PRIMETEST_PROBABLY_PRIME or
 *     BN_PRIMETEST_COMPOSITE
 *
 * returns 0 if there was an error, otherwise it returns 1.
 */
int ossl_bn_miller_rabin_is_prime(const BIGNUM *w, int iterations, BN_CTX *ctx,
                                  BN_GENCB *cb, int enhanced, int *status)
{
    int i, j, a, ret = 0;
    BIGNUM *g, *w1, *w3, *x, *m, *z, *b;
    BN_MONT_CTX *mont = NULL;

    /* w must be odd */
    if (!BN_is_odd(w))
        return 0;

    BN_CTX_start(ctx);
    g = BN_CTX_get(ctx);
    w1 = BN_CTX_get(ctx);
    w3 = BN_CTX_get(ctx);
    x = BN_CTX_get(ctx);
    m = BN_CTX_get(ctx);
    z = BN_CTX_get(ctx);
    b = BN_CTX_get(ctx);

    if (!(b != NULL
            /* w1 := w - 1 */
            && BN_copy(w1, w)
            && BN_sub_word(w1, 1)
            /* w3 := w - 3 */
            && BN_copy(w3, w)
            && BN_sub_word(w3, 3)))
        goto err;

    /* check w is larger than 3, otherwise the random b will be too small */
    if (BN_is_zero(w3) || BN_is_negative(w3))
        goto err;

    /* (Step 1) Calculate largest integer 'a' such that 2^a divides w-1 */
    a = 1;
    while (!BN_is_bit_set(w1, a))
        a++;
    /* (Step 2) m = (w-1) / 2^a */
    if (!BN_rshift(m, w1, a))
        goto err;

    /* Montgomery setup for computations mod a */
    mont = BN_MONT_CTX_new();
    if (mont == NULL || !BN_MONT_CTX_set(mont, w, ctx))
        goto err;

    if (iterations == 0)
        iterations = bn_mr_min_checks(BN_num_bits(w));

    /* (Step 4) */
    for (i = 0; i < iterations; ++i) {
        /* (Step 4.1) obtain a Random string of bits b where 1 < b < w-1 */
        if (!BN_priv_rand_range_ex(b, w3, 0, ctx)
                || !BN_add_word(b, 2)) /* 1 < b < w-1 */
            goto err;

        if (enhanced) {
            /* (Step 4.3) */
            if (!BN_gcd(g, b, w, ctx))
                goto err;
            /* (Step 4.4) */
            if (!BN_is_one(g)) {
                *status = BN_PRIMETEST_COMPOSITE_WITH_FACTOR;
                ret = 1;
                goto err;
            }
        }
        /* (Step 4.5) z = b^m mod w */
        if (!BN_mod_exp_mont(z, b, m, w, ctx, mont))
            goto err;
        /* (Step 4.6) if (z = 1 or z = w-1) */
        if (BN_is_one(z) || BN_cmp(z, w1) == 0)
            goto outer_loop;
        /* (Step 4.7) for j = 1 to a-1 */
        for (j = 1; j < a ; ++j) {
            /* (Step 4.7.1 - 4.7.2) x = z. z = x^2 mod w */
            if (!BN_copy(x, z) || !BN_mod_mul(z, x, x, w, ctx))
                goto err;
            /* (Step 4.7.3) */
            if (BN_cmp(z, w1) == 0)
                goto outer_loop;
            /* (Step 4.7.4) */
            if (BN_is_one(z))
                goto composite;
        }
        /* At this point z = b^((w-1)/2) mod w */
        /* (Steps 4.8 - 4.9) x = z, z = x^2 mod w */
        if (!BN_copy(x, z) || !BN_mod_mul(z, x, x, w, ctx))
            goto err;
        /* (Step 4.10) */
        if (BN_is_one(z))
            goto composite;
        /* (Step 4.11) x = b^(w-1) mod w */
        if (!BN_copy(x, z))
            goto err;
composite:
        if (enhanced) {
            /* (Step 4.1.2) g = GCD(x-1, w) */
            if (!BN_sub_word(x, 1) || !BN_gcd(g, x, w, ctx))
                goto err;
            /* (Steps 4.1.3 - 4.1.4) */
            if (BN_is_one(g))
                *status = BN_PRIMETEST_COMPOSITE_NOT_POWER_OF_PRIME;
            else
                *status = BN_PRIMETEST_COMPOSITE_WITH_FACTOR;
        } else {
            *status = BN_PRIMETEST_COMPOSITE;
        }
        ret = 1;
        goto err;
outer_loop: ;
        /* (Step 4.1.5) */
        if (!BN_GENCB_call(cb, 1, i))
            goto err;
    }
    /* (Step 5) */
    *status = BN_PRIMETEST_PROBABLY_PRIME;
    ret = 1;
err:
    BN_clear(g);
    BN_clear(w1);
    BN_clear(w3);
    BN_clear(x);
    BN_clear(m);
    BN_clear(z);
    BN_clear(b);
    BN_CTX_end(ctx);
    BN_MONT_CTX_free(mont);
    return ret;
}

/*
 * Generate a random number of |bits| bits that is probably prime by sieving.
 * If |safe| != 0, it generates a safe prime.
 * |mods| is a preallocated array that gets reused when called again.
 *
 * The probably prime is saved in |rnd|.
 *
 * Returns 1 on success and 0 on error.
 */
static int probable_prime(BIGNUM *rnd, int bits, int safe, prime_t *mods,
                          BN_CTX *ctx)
{
    int i;
    BN_ULONG delta;
    int trial_divisions = calc_trial_divisions(bits);
    BN_ULONG maxdelta = BN_MASK2 - primes[trial_divisions - 1];

 again:
    if (!BN_priv_rand_ex(rnd, bits, BN_RAND_TOP_TWO, BN_RAND_BOTTOM_ODD, 0,
                         ctx))
        return 0;
    if (safe && !BN_set_bit(rnd, 1))
        return 0;
    /* we now have a random number 'rnd' to test. */
    for (i = 1; i < trial_divisions; i++) {
        BN_ULONG mod = BN_mod_word(rnd, (BN_ULONG)primes[i]);
        if (mod == (BN_ULONG)-1)
            return 0;
        mods[i] = (prime_t) mod;
    }
    delta = 0;
 loop:
    for (i = 1; i < trial_divisions; i++) {
        /*
         * check that rnd is a prime and also that
         * gcd(rnd-1,primes) == 1 (except for 2)
         * do the second check only if we are interested in safe primes
         * in the case that the candidate prime is a single word then
         * we check only the primes up to sqrt(rnd)
         */
        if (bits <= 31 && delta <= 0x7fffffff
                && square(primes[i]) > BN_get_word(rnd) + delta)
            break;
        if (safe ? (mods[i] + delta) % primes[i] <= 1
                 : (mods[i] + delta) % primes[i] == 0) {
            delta += safe ? 4 : 2;
            if (delta > maxdelta)
                goto again;
            goto loop;
        }
    }
    if (!BN_add_word(rnd, delta))
        return 0;
    if (BN_num_bits(rnd) != bits)
        goto again;
    bn_check_top(rnd);
    return 1;
}

/*
 * Generate a random number |rnd| of |bits| bits that is probably prime
 * and satisfies |rnd| % |add| == |rem| by sieving.
 * If |safe| != 0, it generates a safe prime.
 * |mods| is a preallocated array that gets reused when called again.
 *
 * Returns 1 on success and 0 on error.
 */
static int probable_prime_dh(BIGNUM *rnd, int bits, int safe, prime_t *mods,
                             const BIGNUM *add, const BIGNUM *rem,
                             BN_CTX *ctx)
{
    int i, ret = 0;
    BIGNUM *t1;
    BN_ULONG delta;
    int trial_divisions = calc_trial_divisions(bits);
    BN_ULONG maxdelta = BN_MASK2 - primes[trial_divisions - 1];

    BN_CTX_start(ctx);
    if ((t1 = BN_CTX_get(ctx)) == NULL)
        goto err;

    if (maxdelta > BN_MASK2 - BN_get_word(add))
        maxdelta = BN_MASK2 - BN_get_word(add);

 again:
    if (!BN_rand_ex(rnd, bits, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ODD, 0, ctx))
        goto err;

    /* we need ((rnd-rem) % add) == 0 */

    if (!BN_mod(t1, rnd, add, ctx))
        goto err;
    if (!BN_sub(rnd, rnd, t1))
        goto err;
    if (rem == NULL) {
        if (!BN_add_word(rnd, safe ? 3u : 1u))
            goto err;
    } else {
        if (!BN_add(rnd, rnd, rem))
            goto err;
    }

    if (BN_num_bits(rnd) < bits
            || BN_get_word(rnd) < (safe ? 5u : 3u)) {
        if (!BN_add(rnd, rnd, add))
            goto err;
    }

    /* we now have a random number 'rnd' to test. */
    for (i = 1; i < trial_divisions; i++) {
        BN_ULONG mod = BN_mod_word(rnd, (BN_ULONG)primes[i]);
        if (mod == (BN_ULONG)-1)
            goto err;
        mods[i] = (prime_t) mod;
    }
    delta = 0;
 loop:
    for (i = 1; i < trial_divisions; i++) {
        /* check that rnd is a prime */
        if (bits <= 31 && delta <= 0x7fffffff
                && square(primes[i]) > BN_get_word(rnd) + delta)
            break;
        /* rnd mod p == 1 implies q = (rnd-1)/2 is divisible by p */
        if (safe ? (mods[i] + delta) % primes[i] <= 1
                 : (mods[i] + delta) % primes[i] == 0) {
            delta += BN_get_word(add);
            if (delta > maxdelta)
                goto again;
            goto loop;
        }
    }
    if (!BN_add_word(rnd, delta))
        goto err;
    ret = 1;

 err:
    BN_CTX_end(ctx);
    bn_check_top(rnd);
    return ret;
}
