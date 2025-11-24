/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#ifdef __TANDEM
# include <strings.h> /* strcasecmp */
#endif
#include <ctype.h>

#include <openssl/bn.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include "internal/nelem.h"
#include "internal/numbers.h"
#include "testutil.h"

/*
 * Things in boring, not in openssl.
 */
#define HAVE_BN_SQRT 0

typedef struct filetest_st {
    const char *name;
    int (*func)(STANZA *s);
} FILETEST;

typedef struct mpitest_st {
    const char *base10;
    const char *mpi;
    size_t mpi_len;
} MPITEST;

static const int NUM0 = 100;           /* number of tests */
static const int NUM1 = 50;            /* additional tests for some functions */
static const int NUM_PRIME_TESTS = 20;
static BN_CTX *ctx;

/*
 * Polynomial coefficients used in GFM tests.
 */
#ifndef OPENSSL_NO_EC2M
static int p0[] = { 163, 7, 6, 3, 0, -1 };
static int p1[] = { 193, 15, 0, -1 };
#endif

/*
 * Look for |key| in the stanza and return it or NULL if not found.
 */
static const char *findattr(STANZA *s, const char *key)
{
    int i = s->numpairs;
    PAIR *pp = s->pairs;

    for ( ; --i >= 0; pp++)
        if (OPENSSL_strcasecmp(pp->key, key) == 0)
            return pp->value;
    return NULL;
}

/*
 * Parse BIGNUM from sparse hex-strings, return |BN_hex2bn| result.
 */
static int parse_bigBN(BIGNUM **out, const char *bn_strings[])
{
    char *bigstring = glue_strings(bn_strings, NULL);
    int ret = BN_hex2bn(out, bigstring);

    OPENSSL_free(bigstring);
    return ret;
}

/*
 * Parse BIGNUM, return number of bytes parsed.
 */
static int parseBN(BIGNUM **out, const char *in)
{
    *out = NULL;
    return BN_hex2bn(out, in);
}

static int parsedecBN(BIGNUM **out, const char *in)
{
    *out = NULL;
    return BN_dec2bn(out, in);
}

static BIGNUM *getBN(STANZA *s, const char *attribute)
{
    const char *hex;
    BIGNUM *ret = NULL;

    if ((hex = findattr(s, attribute)) == NULL) {
        TEST_error("%s:%d: Can't find %s", s->test_file, s->start, attribute);
        return NULL;
    }

    if (parseBN(&ret, hex) != (int)strlen(hex)) {
        TEST_error("Could not decode '%s'", hex);
        return NULL;
    }
    return ret;
}

static int getint(STANZA *s, int *out, const char *attribute)
{
    BIGNUM *ret;
    BN_ULONG word;
    int st = 0;

    if (!TEST_ptr(ret = getBN(s, attribute))
            || !TEST_ulong_le(word = BN_get_word(ret), INT_MAX))
        goto err;

    *out = (int)word;
    st = 1;
 err:
    BN_free(ret);
    return st;
}

static int equalBN(const char *op, const BIGNUM *expected, const BIGNUM *actual)
{
    if (BN_cmp(expected, actual) == 0)
        return 1;

    TEST_error("unexpected %s value", op);
    TEST_BN_eq(expected, actual);
    return 0;
}

/*
 * Return a "random" flag for if a BN should be negated.
 */
static int rand_neg(void)
{
    static unsigned int neg = 0;
    static int sign[8] = { 0, 0, 0, 1, 1, 0, 1, 1 };

    return sign[(neg++) % 8];
}

static int test_swap(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL;
    int top, cond, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new()))
        goto err;

    if (!(TEST_true(BN_bntest_rand(a, 1024, 1, 0))
            && TEST_true(BN_bntest_rand(b, 1024, 1, 0))
            && TEST_ptr(BN_copy(c, a))
            && TEST_ptr(BN_copy(d, b))))
        goto err;
    top = BN_num_bits(a) / BN_BITS2;

    /* regular swap */
    BN_swap(a, b);
    if (!equalBN("swap", a, d)
            || !equalBN("swap", b, c))
        goto err;

    /* regular swap: same pointer */
    BN_swap(a, a);
    if (!equalBN("swap with same pointer", a, d))
        goto err;

    /* conditional swap: true */
    cond = 1;
    BN_consttime_swap(cond, a, b, top);
    if (!equalBN("cswap true", a, c)
            || !equalBN("cswap true", b, d))
        goto err;

    /* conditional swap: true, same pointer */
    BN_consttime_swap(cond, a, a, top);
    if (!equalBN("cswap true", a, c))
        goto err;

    /* conditional swap: false */
    cond = 0;
    BN_consttime_swap(cond, a, b, top);
    if (!equalBN("cswap false", a, c)
            || !equalBN("cswap false", b, d))
        goto err;

    /* conditional swap: false, same pointer */
    BN_consttime_swap(cond, a, a, top);
    if (!equalBN("cswap false", a, c))
        goto err;

    /* same tests but checking flag swap */
    BN_set_flags(a, BN_FLG_CONSTTIME);

    BN_swap(a, b);
    if (!equalBN("swap, flags", a, d)
            || !equalBN("swap, flags", b, c)
            || !TEST_true(BN_get_flags(b, BN_FLG_CONSTTIME))
            || !TEST_false(BN_get_flags(a, BN_FLG_CONSTTIME)))
        goto err;

    cond = 1;
    BN_consttime_swap(cond, a, b, top);
    if (!equalBN("cswap true, flags", a, c)
            || !equalBN("cswap true, flags", b, d)
            || !TEST_true(BN_get_flags(a, BN_FLG_CONSTTIME))
            || !TEST_false(BN_get_flags(b, BN_FLG_CONSTTIME)))
        goto err;

    cond = 0;
    BN_consttime_swap(cond, a, b, top);
    if (!equalBN("cswap false, flags", a, c)
            || !equalBN("cswap false, flags", b, d)
            || !TEST_true(BN_get_flags(a, BN_FLG_CONSTTIME))
            || !TEST_false(BN_get_flags(b, BN_FLG_CONSTTIME)))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    return st;
}

static int test_sub(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL;
    int i, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new()))
        goto err;

    for (i = 0; i < NUM0 + NUM1; i++) {
        if (i < NUM1) {
            if (!(TEST_true(BN_bntest_rand(a, 512, 0, 0)))
                    && TEST_ptr(BN_copy(b, a))
                    && TEST_int_ne(BN_set_bit(a, i), 0)
                    && TEST_true(BN_add_word(b, i)))
                goto err;
        } else {
            if (!TEST_true(BN_bntest_rand(b, 400 + i - NUM1, 0, 0)))
                goto err;
            BN_set_negative(a, rand_neg());
            BN_set_negative(b, rand_neg());
        }
        if (!(TEST_true(BN_sub(c, a, b))
                && TEST_true(BN_add(c, c, b))
                && TEST_true(BN_sub(c, c, a))
                && TEST_BN_eq_zero(c)))
            goto err;
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    return st;
}

static int test_div_recip(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL, *e = NULL;
    BN_RECP_CTX *recp = NULL;
    int st = 0, i;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(recp = BN_RECP_CTX_new()))
        goto err;

    for (i = 0; i < NUM0 + NUM1; i++) {
        if (i < NUM1) {
            if (!(TEST_true(BN_bntest_rand(a, 400, 0, 0))
                    && TEST_ptr(BN_copy(b, a))
                    && TEST_true(BN_lshift(a, a, i))
                    && TEST_true(BN_add_word(a, i))))
                goto err;
        } else {
            if (!(TEST_true(BN_bntest_rand(b, 50 + 3 * (i - NUM1), 0, 0))))
                goto err;
        }
        BN_set_negative(a, rand_neg());
        BN_set_negative(b, rand_neg());
        if (!(TEST_true(BN_RECP_CTX_set(recp, b, ctx))
                && TEST_true(BN_div_recp(d, c, a, recp, ctx))
                && TEST_true(BN_mul(e, d, b, ctx))
                && TEST_true(BN_add(d, e, c))
                && TEST_true(BN_sub(d, d, a))
                && TEST_BN_eq_zero(d)))
            goto err;
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_RECP_CTX_free(recp);
    return st;
}

static struct {
    int n, divisor, result, remainder;
} signed_mod_tests[] = {
    {  10,   3,   3,   1 },
    { -10,   3,  -3,  -1 },
    {  10,  -3,  -3,   1 },
    { -10,  -3,   3,  -1 },
};

static BIGNUM *set_signed_bn(int value)
{
    BIGNUM *bn = BN_new();

    if (bn == NULL)
        return NULL;
    if (!BN_set_word(bn, value < 0 ? -value : value)) {
        BN_free(bn);
        return NULL;
    }
    BN_set_negative(bn, value < 0);
    return bn;
}

static int test_signed_mod_replace_ab(int n)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL;
    int st = 0;

    if (!TEST_ptr(a = set_signed_bn(signed_mod_tests[n].n))
            || !TEST_ptr(b = set_signed_bn(signed_mod_tests[n].divisor))
            || !TEST_ptr(c = set_signed_bn(signed_mod_tests[n].result))
            || !TEST_ptr(d = set_signed_bn(signed_mod_tests[n].remainder)))
        goto err;

    if (TEST_true(BN_div(a, b, a, b, ctx))
            && TEST_BN_eq(a, c)
            && TEST_BN_eq(b, d))
        st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    return st;
}

static int test_signed_mod_replace_ba(int n)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL;
    int st = 0;

    if (!TEST_ptr(a = set_signed_bn(signed_mod_tests[n].n))
            || !TEST_ptr(b = set_signed_bn(signed_mod_tests[n].divisor))
            || !TEST_ptr(c = set_signed_bn(signed_mod_tests[n].result))
            || !TEST_ptr(d = set_signed_bn(signed_mod_tests[n].remainder)))
        goto err;

    if (TEST_true(BN_div(b, a, a, b, ctx))
            && TEST_BN_eq(b, c)
            && TEST_BN_eq(a, d))
        st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    return st;
}

static int test_mod(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL, *e = NULL;
    int st = 0, i;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new()))
        goto err;

    if (!(TEST_true(BN_bntest_rand(a, 1024, 0, 0))))
        goto err;
    for (i = 0; i < NUM0; i++) {
        if (!(TEST_true(BN_bntest_rand(b, 450 + i * 10, 0, 0))))
            goto err;
        BN_set_negative(a, rand_neg());
        BN_set_negative(b, rand_neg());
        if (!(TEST_true(BN_mod(c, a, b, ctx))
                && TEST_true(BN_div(d, e, a, b, ctx))
                && TEST_BN_eq(e, c)
                && TEST_true(BN_mul(c, d, b, ctx))
                && TEST_true(BN_add(d, c, e))
                && TEST_BN_eq(d, a)))
            goto err;
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    return st;
}

static const char *bn1strings[] = {
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000FFFFFFFF00",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000000000000000000000FFFFFFFFFFFFFF",
    NULL
};

static const char *bn2strings[] = {
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000FFFFFFFF0000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000FFFFFFFFFFFFFF00000000",
    NULL
};

/*
 * Test constant-time modular exponentiation with 1024-bit inputs, which on
 * x86_64 cause a different code branch to be taken.
 */
static int test_modexp_mont5(void)
{
    BIGNUM *a = NULL, *p = NULL, *m = NULL, *d = NULL, *e = NULL;
    BIGNUM *b = NULL, *n = NULL, *c = NULL;
    BN_MONT_CTX *mont = NULL;
    int st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(p = BN_new())
            || !TEST_ptr(m = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(n = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(mont = BN_MONT_CTX_new()))
        goto err;

    /* must be odd for montgomery */
    if (!(TEST_true(BN_bntest_rand(m, 1024, 0, 1))
            /* Zero exponent */
            && TEST_true(BN_bntest_rand(a, 1024, 0, 0))))
        goto err;
    BN_zero(p);

    if (!TEST_true(BN_mod_exp_mont_consttime(d, a, p, m, ctx, NULL)))
        goto err;
    if (!TEST_BN_eq_one(d))
        goto err;

    /* Regression test for carry bug in mulx4x_mont */
    if (!(TEST_true(BN_hex2bn(&a,
        "7878787878787878787878787878787878787878787878787878787878787878"
        "7878787878787878787878787878787878787878787878787878787878787878"
        "7878787878787878787878787878787878787878787878787878787878787878"
        "7878787878787878787878787878787878787878787878787878787878787878"))
        && TEST_true(BN_hex2bn(&b,
        "095D72C08C097BA488C5E439C655A192EAFB6380073D8C2664668EDDB4060744"
        "E16E57FB4EDB9AE10A0CEFCDC28A894F689A128379DB279D48A2E20849D68593"
        "9B7803BCF46CEBF5C533FB0DD35B080593DE5472E3FE5DB951B8BFF9B4CB8F03"
        "9CC638A5EE8CDD703719F8000E6A9F63BEED5F2FCD52FF293EA05A251BB4AB81"))
        && TEST_true(BN_hex2bn(&n,
        "D78AF684E71DB0C39CFF4E64FB9DB567132CB9C50CC98009FEB820B26F2DED9B"
        "91B9B5E2B83AE0AE4EB4E0523CA726BFBE969B89FD754F674CE99118C3F2D1C5"
        "D81FDC7C54E02B60262B241D53C040E99E45826ECA37A804668E690E1AFC1CA4"
        "2C9A15D84D4954425F0B7642FC0BD9D7B24E2618D2DCC9B729D944BADACFDDAF"))))
        goto err;

    if (!(TEST_true(BN_MONT_CTX_set(mont, n, ctx))
            && TEST_true(BN_mod_mul_montgomery(c, a, b, mont, ctx))
            && TEST_true(BN_mod_mul_montgomery(d, b, a, mont, ctx))
            && TEST_BN_eq(c, d)))
        goto err;

    /* Regression test for carry bug in sqr[x]8x_mont */
    if (!(TEST_true(parse_bigBN(&n, bn1strings))
            && TEST_true(parse_bigBN(&a, bn2strings))))
        goto err;
    BN_free(b);
    if (!(TEST_ptr(b = BN_dup(a))
            && TEST_true(BN_MONT_CTX_set(mont, n, ctx))
            && TEST_true(BN_mod_mul_montgomery(c, a, a, mont, ctx))
            && TEST_true(BN_mod_mul_montgomery(d, a, b, mont, ctx))
            && TEST_BN_eq(c, d)))
        goto err;

    /* Regression test for carry bug in bn_sqrx8x_internal */
    {
        static const char *ahex[] = {
                      "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF8FFEADBCFC4DAE7FFF908E92820306B",
            "9544D954000000006C0000000000000000000000000000000000000000000000",
            "00000000000000000000FF030202FFFFF8FFEBDBCFC4DAE7FFF908E92820306B",
            "9544D954000000006C000000FF0302030000000000FFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF01FC00FF02FFFFFFFF",
            "00FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00FCFD",
            "FCFFFFFFFFFF000000000000000000FF0302030000000000FFFFFFFFFFFFFFFF",
            "FF00FCFDFDFF030202FF00000000FFFFFFFFFFFFFFFFFF00FCFDFCFFFFFFFFFF",
            NULL
        };
        static const char *nhex[] = {
                      "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF8F8F8F8000000",
            "00000010000000006C0000000000000000000000000000000000000000000000",
            "00000000000000000000000000000000000000FFFFFFFFFFFFF8F8F8F8000000",
            "00000010000000006C000000000000000000000000FFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "00FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFF000000000000000000000000000000000000FFFFFFFFFFFFFFFF",
            "FFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            NULL
        };

        if (!(TEST_true(parse_bigBN(&a, ahex))
                && TEST_true(parse_bigBN(&n, nhex))))
            goto err;
    }
    BN_free(b);
    if (!(TEST_ptr(b = BN_dup(a))
            && TEST_true(BN_MONT_CTX_set(mont, n, ctx))))
        goto err;

    if (!TEST_true(BN_mod_mul_montgomery(c, a, a, mont, ctx))
            || !TEST_true(BN_mod_mul_montgomery(d, a, b, mont, ctx))
            || !TEST_BN_eq(c, d))
        goto err;

    /* Regression test for bug in BN_from_montgomery_word */
    if (!(TEST_true(BN_hex2bn(&a,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"))
         && TEST_true(BN_hex2bn(&n,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"))
        && TEST_true(BN_MONT_CTX_set(mont, n, ctx))
        && TEST_false(BN_mod_mul_montgomery(d, a, a, mont, ctx))))
        goto err;

    /* Regression test for bug in rsaz_1024_mul_avx2 */
    if (!(TEST_true(BN_hex2bn(&a,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF2020202020DF"))
        && TEST_true(BN_hex2bn(&b,
        "2020202020202020202020202020202020202020202020202020202020202020"
        "2020202020202020202020202020202020202020202020202020202020202020"
        "20202020202020FF202020202020202020202020202020202020202020202020"
        "2020202020202020202020202020202020202020202020202020202020202020"))
        && TEST_true(BN_hex2bn(&n,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF2020202020FF"))
        && TEST_true(BN_MONT_CTX_set(mont, n, ctx))
        && TEST_true(BN_mod_exp_mont_consttime(c, a, b, n, ctx, mont))
        && TEST_true(BN_mod_exp_mont(d, a, b, n, ctx, mont))
        && TEST_BN_eq(c, d)))
        goto err;

    /*
     * rsaz_1024_mul_avx2 expects fully-reduced inputs.
     * BN_mod_exp_mont_consttime should reduce the input first.
     */
    if (!(TEST_true(BN_hex2bn(&a,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF2020202020DF"))
        && TEST_true(BN_hex2bn(&b,
        "1FA53F26F8811C58BE0357897AA5E165693230BC9DF5F01DFA6A2D59229EC69D"
        "9DE6A89C36E3B6957B22D6FAAD5A3C73AE587B710DBE92E83D3A9A3339A085CB"
        "B58F508CA4F837924BB52CC1698B7FDC2FD74362456A595A5B58E38E38E38E38"
        "E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E38E"))
        && TEST_true(BN_hex2bn(&n,
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF2020202020DF"))
        && TEST_true(BN_MONT_CTX_set(mont, n, ctx))
        && TEST_true(BN_mod_exp_mont_consttime(c, a, b, n, ctx, mont))))
        goto err;
    BN_zero(d);
    if (!TEST_BN_eq(c, d))
        goto err;

    /*
     * Regression test for overflow bug in bn_sqr_comba4/8 for
     * mips-linux-gnu and mipsel-linux-gnu 32bit targets.
     */
    {
        static const char *ehex[] = {
            "95564994a96c45954227b845a1e99cb939d5a1da99ee91acc962396ae999a9ee",
            "38603790448f2f7694c242a875f0cad0aae658eba085f312d2febbbd128dd2b5",
            "8f7d1149f03724215d704344d0d62c587ae3c5939cba4b9b5f3dc5e8e911ef9a",
            "5ce1a5a749a4989d0d8368f6e1f8cdf3a362a6c97fb02047ff152b480a4ad985",
            "2d45efdf0770542992afca6a0590d52930434bba96017afbc9f99e112950a8b1",
            "a359473ec376f329bdae6a19f503be6d4be7393c4e43468831234e27e3838680",
            "b949390d2e416a3f9759e5349ab4c253f6f29f819a6fe4cbfd27ada34903300e",
            "da021f62839f5878a36f1bc3085375b00fd5fa3e68d316c0fdace87a97558465",
            NULL};
        static const char *phex[] = {
            "f95dc0f980fbd22e90caa5a387cc4a369f3f830d50dd321c40db8c09a7e1a241",
            "a536e096622d3280c0c1ba849c1f4a79bf490f60006d081e8cf69960189f0d31",
            "2cd9e17073a3fba7881b21474a13b334116cb2f5dbf3189a6de3515d0840f053",
            "c776d3982d391b6d04d642dda5cc6d1640174c09875addb70595658f89efb439",
            "dc6fbd55f903aadd307982d3f659207f265e1ec6271b274521b7a5e28e8fd7a5",
            "5df089292820477802a43cf5b6b94e999e8c9944ddebb0d0e95a60f88cb7e813",
            "ba110d20e1024774107dd02949031864923b3cb8c3f7250d6d1287b0a40db6a4",
            "7bd5a469518eb65aa207ddc47d8c6e5fc8e0c105be8fc1d4b57b2e27540471d5",
            NULL};
        static const char *mhex[] = {
            "fef15d5ce4625f1bccfbba49fc8439c72bf8202af039a2259678941b60bb4a8f",
            "2987e965d58fd8cf86a856674d519763d0e1211cc9f8596971050d56d9b35db3",
            "785866cfbca17cfdbed6060be3629d894f924a89fdc1efc624f80d41a22f1900",
            "9503fcc3824ef62ccb9208430c26f2d8ceb2c63488ec4c07437aa4c96c43dd8b",
            "9289ed00a712ff66ee195dc71f5e4ead02172b63c543d69baf495f5fd63ba7bc",
            "c633bd309c016e37736da92129d0b053d4ab28d21ad7d8b6fab2a8bbdc8ee647",
            "d2fbcf2cf426cf892e6f5639e0252993965dfb73ccd277407014ea784aaa280c",
            "b7b03972bc8b0baa72360bdb44b82415b86b2f260f877791cd33ba8f2d65229b",
            NULL};

        if (!TEST_true(parse_bigBN(&e, ehex))
                || !TEST_true(parse_bigBN(&p, phex))
                || !TEST_true(parse_bigBN(&m, mhex))
                || !TEST_true(BN_mod_exp_mont_consttime(d, e, p, m, ctx, NULL))
                || !TEST_true(BN_mod_exp_simple(a, e, p, m, ctx))
                || !TEST_BN_eq(a, d))
            goto err;
    }

    /* Zero input */
    if (!TEST_true(BN_bntest_rand(p, 1024, 0, 0)))
        goto err;
    BN_zero(a);
    if (!TEST_true(BN_mod_exp_mont_consttime(d, a, p, m, ctx, NULL))
            || !TEST_BN_eq_zero(d))
        goto err;

    /*
     * Craft an input whose Montgomery representation is 1, i.e., shorter
     * than the modulus m, in order to test the const time precomputation
     * scattering/gathering.
     */
    if (!(TEST_true(BN_one(a))
            && TEST_true(BN_MONT_CTX_set(mont, m, ctx))))
        goto err;
    if (!TEST_true(BN_from_montgomery(e, a, mont, ctx))
            || !TEST_true(BN_mod_exp_mont_consttime(d, e, p, m, ctx, NULL))
            || !TEST_true(BN_mod_exp_simple(a, e, p, m, ctx))
            || !TEST_BN_eq(a, d))
        goto err;

    /* Finally, some regular test vectors. */
    if (!(TEST_true(BN_bntest_rand(e, 1024, 0, 0))
            && TEST_true(BN_mod_exp_mont_consttime(d, e, p, m, ctx, NULL))
            && TEST_true(BN_mod_exp_simple(a, e, p, m, ctx))
            && TEST_BN_eq(a, d)))
        goto err;

    st = 1;

 err:
    BN_MONT_CTX_free(mont);
    BN_free(a);
    BN_free(p);
    BN_free(m);
    BN_free(d);
    BN_free(e);
    BN_free(b);
    BN_free(n);
    BN_free(c);
    return st;
}

#ifndef OPENSSL_NO_EC2M
static int test_gf2m_add(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL;
    int i, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new()))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!(TEST_true(BN_rand(a, 512, 0, 0))
                && TEST_ptr(BN_copy(b, BN_value_one()))))
            goto err;
        BN_set_negative(a, rand_neg());
        BN_set_negative(b, rand_neg());
        if (!(TEST_true(BN_GF2m_add(c, a, b))
                /* Test that two added values have the correct parity. */
                && TEST_false((BN_is_odd(a) && BN_is_odd(c))
                        || (!BN_is_odd(a) && !BN_is_odd(c)))))
            goto err;
        if (!(TEST_true(BN_GF2m_add(c, c, c))
                /* Test that c + c = 0. */
                && TEST_BN_eq_zero(c)))
            goto err;
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    return st;
}

static int test_gf2m_mod(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL, *e = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 1024, 0, 0)))
            goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod(c, a, b[j]))
                    && TEST_true(BN_GF2m_add(d, a, c))
                    && TEST_true(BN_GF2m_mod(e, d, b[j]))
                    /* Test that a + (a mod p) mod p == 0. */
                    && TEST_BN_eq_zero(e)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    return st;
}

static int test_gf2m_mul(void)
{
    BIGNUM *a, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    BIGNUM *e = NULL, *f = NULL, *g = NULL, *h = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(f = BN_new())
            || !TEST_ptr(g = BN_new())
            || !TEST_ptr(h = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!(TEST_true(BN_bntest_rand(a, 1024, 0, 0))
                && TEST_true(BN_bntest_rand(c, 1024, 0, 0))
                && TEST_true(BN_bntest_rand(d, 1024, 0, 0))))
            goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod_mul(e, a, c, b[j], ctx))
                    && TEST_true(BN_GF2m_add(f, a, d))
                    && TEST_true(BN_GF2m_mod_mul(g, f, c, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_mul(h, d, c, b[j], ctx))
                    && TEST_true(BN_GF2m_add(f, e, g))
                    && TEST_true(BN_GF2m_add(f, f, h))
                    /* Test that (a+d)*c = a*c + d*c. */
                    && TEST_BN_eq_zero(f)))
                goto err;
        }
    }
    st = 1;

 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_free(f);
    BN_free(g);
    BN_free(h);
    return st;
}

static int test_gf2m_sqr(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 1024, 0, 0)))
                goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod_sqr(c, a, b[j], ctx))
                    && TEST_true(BN_copy(d, a))
                    && TEST_true(BN_GF2m_mod_mul(d, a, d, b[j], ctx))
                    && TEST_true(BN_GF2m_add(d, c, d))
                    /* Test that a*a = a^2. */
                    && TEST_BN_eq_zero(d)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    return st;
}

static int test_gf2m_modinv(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new()))
        goto err;

    /* Test that a non-sensical, too small value causes a failure */
    if (!TEST_true(BN_one(b[0])))
        goto err;
    if (!TEST_true(BN_bntest_rand(a, 512, 0, 0)))
        goto err;
    if (!TEST_false(BN_GF2m_mod_inv(c, a, b[0], ctx)))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 512, 0, 0)))
            goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod_inv(c, a, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_mul(d, a, c, b[j], ctx))
                    /* Test that ((1/a)*a) = 1. */
                    && TEST_BN_eq_one(d)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    return st;
}

static int test_gf2m_moddiv(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    BIGNUM *e = NULL, *f = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(f = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!(TEST_true(BN_bntest_rand(a, 512, 0, 0))
                && TEST_true(BN_bntest_rand(c, 512, 0, 0))))
            goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod_div(d, a, c, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_mul(e, d, c, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_div(f, a, e, b[j], ctx))
                    /* Test that ((a/c)*c)/a = 1. */
                    && TEST_BN_eq_one(f)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_free(f);
    return st;
}

static int test_gf2m_modexp(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    BIGNUM *e = NULL, *f = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(f = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!(TEST_true(BN_bntest_rand(a, 512, 0, 0))
                && TEST_true(BN_bntest_rand(c, 512, 0, 0))
                && TEST_true(BN_bntest_rand(d, 512, 0, 0))))
            goto err;
        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod_exp(e, a, c, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_exp(f, a, d, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_mul(e, e, f, b[j], ctx))
                    && TEST_true(BN_add(f, c, d))
                    && TEST_true(BN_GF2m_mod_exp(f, a, f, b[j], ctx))
                    && TEST_true(BN_GF2m_add(f, e, f))
                    /* Test that a^(c+d)=a^c*a^d. */
                    && TEST_BN_eq_zero(f)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_free(f);
    return st;
}

static int test_gf2m_modsqrt(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    BIGNUM *e = NULL, *f = NULL;
    int i, j, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new())
            || !TEST_ptr(f = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 512, 0, 0)))
            goto err;

        for (j = 0; j < 2; j++) {
            if (!(TEST_true(BN_GF2m_mod(c, a, b[j]))
                    && TEST_true(BN_GF2m_mod_sqrt(d, a, b[j], ctx))
                    && TEST_true(BN_GF2m_mod_sqr(e, d, b[j], ctx))
                    && TEST_true(BN_GF2m_add(f, c, e))
                    /* Test that d^2 = a, where d = sqrt(a). */
                    && TEST_BN_eq_zero(f)))
                goto err;
        }
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_free(f);
    return st;
}

static int test_gf2m_modsolvequad(void)
{
    BIGNUM *a = NULL, *b[2] = {NULL, NULL}, *c = NULL, *d = NULL;
    BIGNUM *e = NULL;
    int i, j, s = 0, t, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b[0] = BN_new())
            || !TEST_ptr(b[1] = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new())
            || !TEST_ptr(e = BN_new()))
        goto err;

    if (!(TEST_true(BN_GF2m_arr2poly(p0, b[0]))
            && TEST_true(BN_GF2m_arr2poly(p1, b[1]))))
        goto err;

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 512, 0, 0)))
            goto err;
        for (j = 0; j < 2; j++) {
            t = BN_GF2m_mod_solve_quad(c, a, b[j], ctx);
            if (t) {
                s++;
                if (!(TEST_true(BN_GF2m_mod_sqr(d, c, b[j], ctx))
                        && TEST_true(BN_GF2m_add(d, c, d))
                        && TEST_true(BN_GF2m_mod(e, a, b[j]))
                        && TEST_true(BN_GF2m_add(e, e, d))
                        /*
                         * Test that solution of quadratic c
                         * satisfies c^2 + c = a.
                         */
                        && TEST_BN_eq_zero(e)))
                    goto err;
            }
        }
    }
    if (!TEST_int_ge(s, 0)) {
        TEST_info("%d tests found no roots; probably an error", NUM0);
        goto err;
    }
    st = 1;
 err:
    BN_free(a);
    BN_free(b[0]);
    BN_free(b[1]);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    return st;
}
#endif

static int test_kronecker(void)
{
    BIGNUM *a = NULL, *b = NULL, *r = NULL, *t = NULL;
    int i, legendre, kronecker, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(r = BN_new())
            || !TEST_ptr(t = BN_new()))
        goto err;

    /*
     * We test BN_kronecker(a, b, ctx) just for b odd (Jacobi symbol). In
     * this case we know that if b is prime, then BN_kronecker(a, b, ctx) is
     * congruent to $a^{(b-1)/2}$, modulo $b$ (Legendre symbol). So we
     * generate a random prime b and compare these values for a number of
     * random a's.  (That is, we run the Solovay-Strassen primality test to
     * confirm that b is prime, except that we don't want to test whether b
     * is prime but whether BN_kronecker works.)
     */

    if (!TEST_true(BN_generate_prime_ex(b, 512, 0, NULL, NULL, NULL)))
        goto err;
    BN_set_negative(b, rand_neg());

    for (i = 0; i < NUM0; i++) {
        if (!TEST_true(BN_bntest_rand(a, 512, 0, 0)))
            goto err;
        BN_set_negative(a, rand_neg());

        /* t := (|b|-1)/2  (note that b is odd) */
        if (!TEST_true(BN_copy(t, b)))
            goto err;
        BN_set_negative(t, 0);
        if (!TEST_true(BN_sub_word(t, 1)))
            goto err;
        if (!TEST_true(BN_rshift1(t, t)))
            goto err;
        /* r := a^t mod b */
        BN_set_negative(b, 0);

        if (!TEST_true(BN_mod_exp_recp(r, a, t, b, ctx)))
            goto err;
        BN_set_negative(b, 1);

        if (BN_is_word(r, 1))
            legendre = 1;
        else if (BN_is_zero(r))
            legendre = 0;
        else {
            if (!TEST_true(BN_add_word(r, 1)))
                goto err;
            if (!TEST_int_eq(BN_ucmp(r, b), 0)) {
                TEST_info("Legendre symbol computation failed");
                goto err;
            }
            legendre = -1;
        }

        if (!TEST_int_ge(kronecker = BN_kronecker(a, b, ctx), -1))
            goto err;
        /* we actually need BN_kronecker(a, |b|) */
        if (BN_is_negative(a) && BN_is_negative(b))
            kronecker = -kronecker;

        if (!TEST_int_eq(legendre, kronecker))
            goto err;
    }

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(r);
    BN_free(t);
    return st;
}

static int file_sum(STANZA *s)
{
    BIGNUM *a = NULL, *b = NULL, *sum = NULL, *ret = NULL;
    BN_ULONG b_word;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(b = getBN(s, "B"))
            || !TEST_ptr(sum = getBN(s, "Sum"))
            || !TEST_ptr(ret = BN_new()))
        goto err;

    if (!TEST_true(BN_add(ret, a, b))
            || !equalBN("A + B", sum, ret)
            || !TEST_true(BN_sub(ret, sum, a))
            || !equalBN("Sum - A", b, ret)
            || !TEST_true(BN_sub(ret, sum, b))
            || !equalBN("Sum - B", a, ret))
        goto err;

    /*
     * Test that the functions work when |r| and |a| point to the same BIGNUM,
     * or when |r| and |b| point to the same BIGNUM.
     * There is no test for all of |r|, |a|, and |b| pointint to the same BIGNUM.
     */
    if (!TEST_true(BN_copy(ret, a))
            || !TEST_true(BN_add(ret, ret, b))
            || !equalBN("A + B (r is a)", sum, ret)
            || !TEST_true(BN_copy(ret, b))
            || !TEST_true(BN_add(ret, a, ret))
            || !equalBN("A + B (r is b)", sum, ret)
            || !TEST_true(BN_copy(ret, sum))
            || !TEST_true(BN_sub(ret, ret, a))
            || !equalBN("Sum - A (r is a)", b, ret)
            || !TEST_true(BN_copy(ret, a))
            || !TEST_true(BN_sub(ret, sum, ret))
            || !equalBN("Sum - A (r is b)", b, ret)
            || !TEST_true(BN_copy(ret, sum))
            || !TEST_true(BN_sub(ret, ret, b))
            || !equalBN("Sum - B (r is a)", a, ret)
            || !TEST_true(BN_copy(ret, b))
            || !TEST_true(BN_sub(ret, sum, ret))
            || !equalBN("Sum - B (r is b)", a, ret))
        goto err;

    /*
     * Test BN_uadd() and BN_usub() with the prerequisites they are
     * documented as having. Note that these functions are frequently used
     * when the prerequisites don't hold. In those cases, they are supposed
     * to work as if the prerequisite hold, but we don't test that yet.
     */
    if (!BN_is_negative(a) && !BN_is_negative(b) && BN_cmp(a, b) >= 0) {
        if (!TEST_true(BN_uadd(ret, a, b))
                || !equalBN("A +u B", sum, ret)
                || !TEST_true(BN_usub(ret, sum, a))
                || !equalBN("Sum -u A", b, ret)
                || !TEST_true(BN_usub(ret, sum, b))
                || !equalBN("Sum -u B", a, ret))
            goto err;
        /*
         * Test that the functions work when |r| and |a| point to the same
         * BIGNUM, or when |r| and |b| point to the same BIGNUM.
         * There is no test for all of |r|, |a|, and |b| pointint to the same
         * BIGNUM.
         */
        if (!TEST_true(BN_copy(ret, a))
                || !TEST_true(BN_uadd(ret, ret, b))
                || !equalBN("A +u B (r is a)", sum, ret)
                || !TEST_true(BN_copy(ret, b))
                || !TEST_true(BN_uadd(ret, a, ret))
                || !equalBN("A +u B (r is b)", sum, ret)
                || !TEST_true(BN_copy(ret, sum))
                || !TEST_true(BN_usub(ret, ret, a))
                || !equalBN("Sum -u A (r is a)", b, ret)
                || !TEST_true(BN_copy(ret, a))
                || !TEST_true(BN_usub(ret, sum, ret))
                || !equalBN("Sum -u A (r is b)", b, ret)
                || !TEST_true(BN_copy(ret, sum))
                || !TEST_true(BN_usub(ret, ret, b))
                || !equalBN("Sum -u B (r is a)", a, ret)
                || !TEST_true(BN_copy(ret, b))
                || !TEST_true(BN_usub(ret, sum, ret))
                || !equalBN("Sum -u B (r is b)", a, ret))
            goto err;
    }

    /*
     * Test with BN_add_word() and BN_sub_word() if |b| is small enough.
     */
    b_word = BN_get_word(b);
    if (!BN_is_negative(b) && b_word != (BN_ULONG)-1) {
        if (!TEST_true(BN_copy(ret, a))
                || !TEST_true(BN_add_word(ret, b_word))
                || !equalBN("A + B (word)", sum, ret)
                || !TEST_true(BN_copy(ret, sum))
                || !TEST_true(BN_sub_word(ret, b_word))
                || !equalBN("Sum - B (word)", a, ret))
            goto err;
    }
    st = 1;

 err:
    BN_free(a);
    BN_free(b);
    BN_free(sum);
    BN_free(ret);
    return st;
}

static int file_lshift1(STANZA *s)
{
    BIGNUM *a = NULL, *lshift1 = NULL, *zero = NULL, *ret = NULL;
    BIGNUM *two = NULL, *remainder = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(lshift1 = getBN(s, "LShift1"))
            || !TEST_ptr(zero = BN_new())
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(two = BN_new())
            || !TEST_ptr(remainder = BN_new()))
        goto err;

    BN_zero(zero);

    if (!TEST_true(BN_set_word(two, 2))
            || !TEST_true(BN_add(ret, a, a))
            || !equalBN("A + A", lshift1, ret)
            || !TEST_true(BN_mul(ret, a, two, ctx))
            || !equalBN("A * 2", lshift1, ret)
            || !TEST_true(BN_div(ret, remainder, lshift1, two, ctx))
            || !equalBN("LShift1 / 2", a, ret)
            || !equalBN("LShift1 % 2", zero, remainder)
            || !TEST_true(BN_lshift1(ret, a))
            || !equalBN("A << 1", lshift1, ret)
            || !TEST_true(BN_rshift1(ret, lshift1))
            || !equalBN("LShift >> 1", a, ret)
            || !TEST_true(BN_rshift1(ret, lshift1))
            || !equalBN("LShift >> 1", a, ret))
        goto err;

    /* Set the LSB to 1 and test rshift1 again. */
    if (!TEST_true(BN_set_bit(lshift1, 0))
            || !TEST_true(BN_div(ret, NULL /* rem */ , lshift1, two, ctx))
            || !equalBN("(LShift1 | 1) / 2", a, ret)
            || !TEST_true(BN_rshift1(ret, lshift1))
            || !equalBN("(LShift | 1) >> 1", a, ret))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(lshift1);
    BN_free(zero);
    BN_free(ret);
    BN_free(two);
    BN_free(remainder);

    return st;
}

static int file_lshift(STANZA *s)
{
    BIGNUM *a = NULL, *lshift = NULL, *ret = NULL;
    int n = 0, st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(lshift = getBN(s, "LShift"))
            || !TEST_ptr(ret = BN_new())
            || !getint(s, &n, "N"))
        goto err;

    if (!TEST_true(BN_lshift(ret, a, n))
            || !equalBN("A << N", lshift, ret)
            || !TEST_true(BN_rshift(ret, lshift, n))
            || !equalBN("A >> N", a, ret))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(lshift);
    BN_free(ret);
    return st;
}

static int file_rshift(STANZA *s)
{
    BIGNUM *a = NULL, *rshift = NULL, *ret = NULL;
    int n = 0, st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(rshift = getBN(s, "RShift"))
            || !TEST_ptr(ret = BN_new())
            || !getint(s, &n, "N"))
        goto err;

    if (!TEST_true(BN_rshift(ret, a, n))
            || !equalBN("A >> N", rshift, ret))
        goto err;

    /* If N == 1, try with rshift1 as well */
    if (n == 1) {
        if (!TEST_true(BN_rshift1(ret, a))
                || !equalBN("A >> 1 (rshift1)", rshift, ret))
            goto err;
    }
    st = 1;

 err:
    BN_free(a);
    BN_free(rshift);
    BN_free(ret);
    return st;
}

static int file_square(STANZA *s)
{
    BIGNUM *a = NULL, *square = NULL, *zero = NULL, *ret = NULL;
    BIGNUM *remainder = NULL, *tmp = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(square = getBN(s, "Square"))
            || !TEST_ptr(zero = BN_new())
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(remainder = BN_new()))
        goto err;

    BN_zero(zero);
    if (!TEST_true(BN_sqr(ret, a, ctx))
            || !equalBN("A^2", square, ret)
            || !TEST_true(BN_mul(ret, a, a, ctx))
            || !equalBN("A * A", square, ret)
            || !TEST_true(BN_div(ret, remainder, square, a, ctx))
            || !equalBN("Square / A", a, ret)
            || !equalBN("Square % A", zero, remainder))
        goto err;

#if HAVE_BN_SQRT
    BN_set_negative(a, 0);
    if (!TEST_true(BN_sqrt(ret, square, ctx))
            || !equalBN("sqrt(Square)", a, ret))
        goto err;

    /* BN_sqrt should fail on non-squares and negative numbers. */
    if (!TEST_BN_eq_zero(square)) {
        if (!TEST_ptr(tmp = BN_new())
                || !TEST_true(BN_copy(tmp, square)))
            goto err;
        BN_set_negative(tmp, 1);

        if (!TEST_int_eq(BN_sqrt(ret, tmp, ctx), 0))
            goto err;
        ERR_clear_error();

        BN_set_negative(tmp, 0);
        if (BN_add(tmp, tmp, BN_value_one()))
            goto err;
        if (!TEST_int_eq(BN_sqrt(ret, tmp, ctx)))
            goto err;
        ERR_clear_error();
    }
#endif

    st = 1;
 err:
    BN_free(a);
    BN_free(square);
    BN_free(zero);
    BN_free(ret);
    BN_free(remainder);
    BN_free(tmp);
    return st;
}

static int file_product(STANZA *s)
{
    BIGNUM *a = NULL, *b = NULL, *product = NULL, *ret = NULL;
    BIGNUM *remainder = NULL, *zero = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(b = getBN(s, "B"))
            || !TEST_ptr(product = getBN(s, "Product"))
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(remainder = BN_new())
            || !TEST_ptr(zero = BN_new()))
        goto err;

    BN_zero(zero);

    if (!TEST_true(BN_mul(ret, a, b, ctx))
            || !equalBN("A * B", product, ret)
            || !TEST_true(BN_div(ret, remainder, product, a, ctx))
            || !equalBN("Product / A", b, ret)
            || !equalBN("Product % A", zero, remainder)
            || !TEST_true(BN_div(ret, remainder, product, b, ctx))
            || !equalBN("Product / B", a, ret)
            || !equalBN("Product % B", zero, remainder))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(product);
    BN_free(ret);
    BN_free(remainder);
    BN_free(zero);
    return st;
}

static int file_quotient(STANZA *s)
{
    BIGNUM *a = NULL, *b = NULL, *quotient = NULL, *remainder = NULL;
    BIGNUM *ret = NULL, *ret2 = NULL, *nnmod = NULL;
    BN_ULONG b_word, ret_word;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(b = getBN(s, "B"))
            || !TEST_ptr(quotient = getBN(s, "Quotient"))
            || !TEST_ptr(remainder = getBN(s, "Remainder"))
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(ret2 = BN_new())
            || !TEST_ptr(nnmod = BN_new()))
        goto err;

    if (!TEST_true(BN_div(ret, ret2, a, b, ctx))
            || !equalBN("A / B", quotient, ret)
            || !equalBN("A % B", remainder, ret2)
            || !TEST_true(BN_mul(ret, quotient, b, ctx))
            || !TEST_true(BN_add(ret, ret, remainder))
            || !equalBN("Quotient * B + Remainder", a, ret))
        goto err;

    /*
     * Test with BN_mod_word() and BN_div_word() if the divisor is
     * small enough.
     */
    b_word = BN_get_word(b);
    if (!BN_is_negative(b) && b_word != (BN_ULONG)-1) {
        BN_ULONG remainder_word = BN_get_word(remainder);

        assert(remainder_word != (BN_ULONG)-1);
        if (!TEST_ptr(BN_copy(ret, a)))
            goto err;
        ret_word = BN_div_word(ret, b_word);
        if (ret_word != remainder_word) {
#ifdef BN_DEC_FMT1
            TEST_error(
                    "Got A %% B (word) = " BN_DEC_FMT1 ", wanted " BN_DEC_FMT1,
                    ret_word, remainder_word);
#else
            TEST_error("Got A %% B (word) mismatch");
#endif
            goto err;
        }
        if (!equalBN ("A / B (word)", quotient, ret))
            goto err;

        ret_word = BN_mod_word(a, b_word);
        if (ret_word != remainder_word) {
#ifdef BN_DEC_FMT1
            TEST_error(
                    "Got A %% B (word) = " BN_DEC_FMT1 ", wanted " BN_DEC_FMT1 "",
                    ret_word, remainder_word);
#else
            TEST_error("Got A %% B (word) mismatch");
#endif
            goto err;
        }
    }

    /* Test BN_nnmod. */
    if (!BN_is_negative(b)) {
        if (!TEST_true(BN_copy(nnmod, remainder))
                || (BN_is_negative(nnmod)
                        && !TEST_true(BN_add(nnmod, nnmod, b)))
                || !TEST_true(BN_nnmod(ret, a, b, ctx))
                || !equalBN("A % B (non-negative)", nnmod, ret))
            goto err;
    }

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(quotient);
    BN_free(remainder);
    BN_free(ret);
    BN_free(ret2);
    BN_free(nnmod);
    return st;
}

static int file_modmul(STANZA *s)
{
    BIGNUM *a = NULL, *b = NULL, *m = NULL, *mod_mul = NULL, *ret = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(b = getBN(s, "B"))
            || !TEST_ptr(m = getBN(s, "M"))
            || !TEST_ptr(mod_mul = getBN(s, "ModMul"))
            || !TEST_ptr(ret = BN_new()))
        goto err;

    if (!TEST_true(BN_mod_mul(ret, a, b, m, ctx))
            || !equalBN("A * B (mod M)", mod_mul, ret))
        goto err;

    if (BN_is_odd(m)) {
        /* Reduce |a| and |b| and test the Montgomery version. */
        BN_MONT_CTX *mont = BN_MONT_CTX_new();
        BIGNUM *a_tmp = BN_new();
        BIGNUM *b_tmp = BN_new();

        if (mont == NULL || a_tmp == NULL || b_tmp == NULL
                || !TEST_true(BN_MONT_CTX_set(mont, m, ctx))
                || !TEST_true(BN_nnmod(a_tmp, a, m, ctx))
                || !TEST_true(BN_nnmod(b_tmp, b, m, ctx))
                || !TEST_true(BN_to_montgomery(a_tmp, a_tmp, mont, ctx))
                || !TEST_true(BN_to_montgomery(b_tmp, b_tmp, mont, ctx))
                || !TEST_true(BN_mod_mul_montgomery(ret, a_tmp, b_tmp,
                                                    mont, ctx))
                || !TEST_true(BN_from_montgomery(ret, ret, mont, ctx))
                || !equalBN("A * B (mod M) (mont)", mod_mul, ret))
            st = 0;
        else
            st = 1;
        BN_MONT_CTX_free(mont);
        BN_free(a_tmp);
        BN_free(b_tmp);
        if (st == 0)
            goto err;
    }

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(m);
    BN_free(mod_mul);
    BN_free(ret);
    return st;
}

static int file_modexp(STANZA *s)
{
    BIGNUM *a = NULL, *e = NULL, *m = NULL, *mod_exp = NULL, *ret = NULL;
    BIGNUM *b = NULL, *c = NULL, *d = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(e = getBN(s, "E"))
            || !TEST_ptr(m = getBN(s, "M"))
            || !TEST_ptr(mod_exp = getBN(s, "ModExp"))
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(d = BN_new()))
        goto err;

    if (!TEST_true(BN_mod_exp(ret, a, e, m, ctx))
            || !equalBN("A ^ E (mod M)", mod_exp, ret))
        goto err;

    if (BN_is_odd(m)) {
        if (!TEST_true(BN_mod_exp_mont(ret, a, e, m, ctx, NULL))
                || !equalBN("A ^ E (mod M) (mont)", mod_exp, ret)
                || !TEST_true(BN_mod_exp_mont_consttime(ret, a, e, m,
                                                        ctx, NULL))
                || !equalBN("A ^ E (mod M) (mont const", mod_exp, ret))
            goto err;
    }

    /* Regression test for carry propagation bug in sqr8x_reduction */
    BN_hex2bn(&a, "050505050505");
    BN_hex2bn(&b, "02");
    BN_hex2bn(&c,
        "4141414141414141414141274141414141414141414141414141414141414141"
        "4141414141414141414141414141414141414141414141414141414141414141"
        "4141414141414141414141800000000000000000000000000000000000000000"
        "0000000000000000000000000000000000000000000000000000000000000000"
        "0000000000000000000000000000000000000000000000000000000000000000"
        "0000000000000000000000000000000000000000000000000000000001");
    if (!TEST_true(BN_mod_exp(d, a, b, c, ctx))
        || !TEST_true(BN_mul(e, a, a, ctx))
        || !TEST_BN_eq(d, e))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    BN_free(e);
    BN_free(m);
    BN_free(mod_exp);
    BN_free(ret);
    return st;
}

static int file_exp(STANZA *s)
{
    BIGNUM *a = NULL, *e = NULL, *exp = NULL, *ret = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(e = getBN(s, "E"))
            || !TEST_ptr(exp = getBN(s, "Exp"))
            || !TEST_ptr(ret = BN_new()))
        goto err;

    if (!TEST_true(BN_exp(ret, a, e, ctx))
            || !equalBN("A ^ E", exp, ret))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(e);
    BN_free(exp);
    BN_free(ret);
    return st;
}

static int file_modsqrt(STANZA *s)
{
    BIGNUM *a = NULL, *p = NULL, *mod_sqrt = NULL, *ret = NULL, *ret2 = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(p = getBN(s, "P"))
            || !TEST_ptr(mod_sqrt = getBN(s, "ModSqrt"))
            || !TEST_ptr(ret = BN_new())
            || !TEST_ptr(ret2 = BN_new()))
        goto err;

    if (BN_is_negative(mod_sqrt)) {
        /* A negative testcase */
        if (!TEST_ptr_null(BN_mod_sqrt(ret, a, p, ctx)))
            goto err;

        st = 1;
        goto err;
    }

    /* There are two possible answers. */
    if (!TEST_ptr(BN_mod_sqrt(ret, a, p, ctx))
            || !TEST_true(BN_sub(ret2, p, ret)))
        goto err;

    /* The first condition should NOT be a test. */
    if (BN_cmp(ret2, mod_sqrt) != 0
            && !equalBN("sqrt(A) (mod P)", mod_sqrt, ret))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(p);
    BN_free(mod_sqrt);
    BN_free(ret);
    BN_free(ret2);
    return st;
}

static int file_gcd(STANZA *s)
{
    BIGNUM *a = NULL, *b = NULL, *gcd = NULL, *ret = NULL;
    int st = 0;

    if (!TEST_ptr(a = getBN(s, "A"))
            || !TEST_ptr(b = getBN(s, "B"))
            || !TEST_ptr(gcd = getBN(s, "GCD"))
            || !TEST_ptr(ret = BN_new()))
        goto err;

    if (!TEST_true(BN_gcd(ret, a, b, ctx))
            || !equalBN("gcd(A,B)", gcd, ret))
        goto err;

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(gcd);
    BN_free(ret);
    return st;
}

static int test_bn2padded(void)
{
    uint8_t zeros[256], out[256], reference[128];
    size_t bytes;
    BIGNUM *n;
    int st = 0;

    /* Test edge case at 0. */
    if (!TEST_ptr((n = BN_new())))
        goto err;
    if (!TEST_int_eq(BN_bn2binpad(n, NULL, 0), 0))
        goto err;
    memset(out, -1, sizeof(out));
    if (!TEST_int_eq(BN_bn2binpad(n, out, sizeof(out)), sizeof(out)))
        goto err;
    memset(zeros, 0, sizeof(zeros));
    if (!TEST_mem_eq(zeros, sizeof(zeros), out, sizeof(out)))
        goto err;

    /* Test a random numbers at various byte lengths. */
    for (bytes = 128 - 7; bytes <= 128; bytes++) {
# define TOP_BIT_ON 0
# define BOTTOM_BIT_NOTOUCH 0
        if (!TEST_true(BN_rand(n, bytes * 8, TOP_BIT_ON, BOTTOM_BIT_NOTOUCH)))
            goto err;
        if (!TEST_int_eq(BN_num_bytes(n), bytes)
                || !TEST_int_eq(BN_bn2bin(n, reference), bytes))
            goto err;
        /* Empty buffer should fail. */
        if (!TEST_int_eq(BN_bn2binpad(n, NULL, 0), -1))
            goto err;
        /* One byte short should fail. */
        if (!TEST_int_eq(BN_bn2binpad(n, out, bytes - 1), -1))
            goto err;
        /* Exactly right size should encode. */
        if (!TEST_int_eq(BN_bn2binpad(n, out, bytes), bytes)
                || !TEST_mem_eq(out, bytes, reference, bytes))
            goto err;
        /* Pad up one byte extra. */
        if (!TEST_int_eq(BN_bn2binpad(n, out, bytes + 1), bytes + 1)
                || !TEST_mem_eq(out + 1, bytes, reference, bytes)
                || !TEST_mem_eq(out, 1, zeros, 1))
            goto err;
        /* Pad up to 256. */
        if (!TEST_int_eq(BN_bn2binpad(n, out, sizeof(out)), sizeof(out))
                || !TEST_mem_eq(out + sizeof(out) - bytes, bytes,
                                reference, bytes)
                || !TEST_mem_eq(out, sizeof(out) - bytes,
                                zeros, sizeof(out) - bytes))
            goto err;
    }

    st = 1;
 err:
    BN_free(n);
    return st;
}

static const MPITEST kSignedTests_BE[] = {
    {"-1", "\xff", 1},
    {"0", "", 0},
    {"1", "\x01", 1},
    /*
     * The above cover the basics, now let's go for possible bignum
     * chunk edges and other word edges (for a broad definition of
     * "word", i.e. 1 byte included).
     */
    /* 1 byte edge */
    {"127", "\x7f", 1},
    {"-127", "\x81", 1},
    {"128", "\x00\x80", 2},
    {"-128", "\x80", 1},
    {"129", "\x00\x81", 2},
    {"-129", "\xff\x7f", 2},
    {"255", "\x00\xff", 2},
    {"-255", "\xff\x01", 2},
    {"256", "\x01\x00", 2},
    {"-256", "\xff\x00", 2},
    /* 2 byte edge */
    {"32767", "\x7f\xff", 2},
    {"-32767", "\x80\x01", 2},
    {"32768", "\x00\x80\x00", 3},
    {"-32768", "\x80\x00", 2},
    {"32769", "\x00\x80\x01", 3},
    {"-32769", "\xff\x7f\xff", 3},
    {"65535", "\x00\xff\xff", 3},
    {"-65535", "\xff\x00\x01", 3},
    {"65536", "\x01\x00\x00", 3},
    {"-65536", "\xff\x00\x00", 3},
    /* 4 byte edge */
    {"2147483647", "\x7f\xff\xff\xff", 4},
    {"-2147483647", "\x80\x00\x00\x01", 4},
    {"2147483648", "\x00\x80\x00\x00\x00", 5},
    {"-2147483648", "\x80\x00\x00\x00", 4},
    {"2147483649", "\x00\x80\x00\x00\x01", 5},
    {"-2147483649", "\xff\x7f\xff\xff\xff", 5},
    {"4294967295", "\x00\xff\xff\xff\xff", 5},
    {"-4294967295", "\xff\x00\x00\x00\x01", 5},
    {"4294967296", "\x01\x00\x00\x00\x00", 5},
    {"-4294967296", "\xff\x00\x00\x00\x00", 5},
    /* 8 byte edge */
    {"9223372036854775807", "\x7f\xff\xff\xff\xff\xff\xff\xff", 8},
    {"-9223372036854775807", "\x80\x00\x00\x00\x00\x00\x00\x01", 8},
    {"9223372036854775808", "\x00\x80\x00\x00\x00\x00\x00\x00\x00", 9},
    {"-9223372036854775808", "\x80\x00\x00\x00\x00\x00\x00\x00", 8},
    {"9223372036854775809", "\x00\x80\x00\x00\x00\x00\x00\x00\x01", 9},
    {"-9223372036854775809", "\xff\x7f\xff\xff\xff\xff\xff\xff\xff", 9},
    {"18446744073709551615", "\x00\xff\xff\xff\xff\xff\xff\xff\xff", 9},
    {"-18446744073709551615", "\xff\x00\x00\x00\x00\x00\x00\x00\x01", 9},
    {"18446744073709551616", "\x01\x00\x00\x00\x00\x00\x00\x00\x00", 9},
    {"-18446744073709551616", "\xff\x00\x00\x00\x00\x00\x00\x00\x00", 9},
};

static int copy_reversed(uint8_t *dst, uint8_t *src, size_t len)
{
    for (dst += len - 1; len > 0; src++, dst--, len--)
        *dst = *src;
    return 1;
}

static int test_bn2signed(int i)
{
    uint8_t scratch[10], reversed[10];
    const MPITEST *test = &kSignedTests_BE[i];
    BIGNUM *bn = NULL, *bn2 = NULL;
    int st = 0;

    if (!TEST_ptr(bn = BN_new())
        || !TEST_true(BN_asc2bn(&bn, test->base10)))
        goto err;

    /*
     * Check BN_signed_bn2bin() / BN_signed_bin2bn()
     * The interesting stuff happens in the last bytes of the buffers,
     * the beginning is just padding (i.e. sign extension).
     */
    i = sizeof(scratch) - test->mpi_len;
    if (!TEST_int_eq(BN_signed_bn2bin(bn, scratch, sizeof(scratch)),
                     sizeof(scratch))
        || !TEST_true(copy_reversed(reversed, scratch, sizeof(scratch)))
        || !TEST_mem_eq(test->mpi, test->mpi_len, scratch + i, test->mpi_len))
        goto err;

    if (!TEST_ptr(bn2 = BN_signed_bin2bn(scratch, sizeof(scratch), NULL))
        || !TEST_BN_eq(bn, bn2))
        goto err;

    BN_free(bn2);
    bn2 = NULL;

    /* Check that a parse of the reversed buffer works too */
    if (!TEST_ptr(bn2 = BN_signed_lebin2bn(reversed, sizeof(reversed), NULL))
        || !TEST_BN_eq(bn, bn2))
        goto err;

    BN_free(bn2);
    bn2 = NULL;

    /*
     * Check BN_signed_bn2lebin() / BN_signed_lebin2bn()
     * The interesting stuff happens in the first bytes of the buffers,
     * the end is just padding (i.e. sign extension).
     */
    i = sizeof(reversed) - test->mpi_len;
    if (!TEST_int_eq(BN_signed_bn2lebin(bn, scratch, sizeof(scratch)),
                     sizeof(scratch))
        || !TEST_true(copy_reversed(reversed, scratch, sizeof(scratch)))
        || !TEST_mem_eq(test->mpi, test->mpi_len, reversed + i, test->mpi_len))
        goto err;

    if (!TEST_ptr(bn2 = BN_signed_lebin2bn(scratch, sizeof(scratch), NULL))
        || !TEST_BN_eq(bn, bn2))
        goto err;

    BN_free(bn2);
    bn2 = NULL;

    /* Check that a parse of the reversed buffer works too */
    if (!TEST_ptr(bn2 = BN_signed_bin2bn(reversed, sizeof(reversed), NULL))
        || !TEST_BN_eq(bn, bn2))
        goto err;

    st = 1;
 err:
    BN_free(bn2);
    BN_free(bn);
    return st;
}

static int test_dec2bn(void)
{
    BIGNUM *bn = NULL;
    int st = 0;

    if (!TEST_int_eq(parsedecBN(&bn, "0"), 1)
            || !TEST_BN_eq_word(bn, 0)
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_le_zero(bn)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parsedecBN(&bn, "256"), 3)
            || !TEST_BN_eq_word(bn, 256)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parsedecBN(&bn, "-42"), 3)
            || !TEST_BN_abs_eq_word(bn, 42)
            || !TEST_BN_lt_zero(bn)
            || !TEST_BN_le_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parsedecBN(&bn, "1"), 1)
            || !TEST_BN_eq_word(bn, 1)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_eq_one(bn)
            || !TEST_BN_odd(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parsedecBN(&bn, "-0"), 2)
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_le_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parsedecBN(&bn, "42trailing garbage is ignored"), 2)
            || !TEST_BN_abs_eq_word(bn, 42)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;

    st = 1;
 err:
    BN_free(bn);
    return st;
}

static int test_hex2bn(void)
{
    BIGNUM *bn = NULL;
    int st = 0;

    if (!TEST_int_eq(parseBN(&bn, "0"), 1)
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parseBN(&bn, "256"), 3)
            || !TEST_BN_eq_word(bn, 0x256)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parseBN(&bn, "-42"), 3)
            || !TEST_BN_abs_eq_word(bn, 0x42)
            || !TEST_BN_lt_zero(bn)
            || !TEST_BN_le_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parseBN(&bn, "cb"), 2)
            || !TEST_BN_eq_word(bn, 0xCB)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_odd(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parseBN(&bn, "-0"), 2)
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_le_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    BN_free(bn);
    bn = NULL;

    if (!TEST_int_eq(parseBN(&bn, "abctrailing garbage is ignored"), 3)
            || !TEST_BN_eq_word(bn, 0xabc)
            || !TEST_BN_ge_zero(bn)
            || !TEST_BN_gt_zero(bn)
            || !TEST_BN_ne_zero(bn)
            || !TEST_BN_even(bn))
        goto err;
    st = 1;

 err:
    BN_free(bn);
    return st;
}

static int test_asc2bn(void)
{
    BIGNUM *bn = NULL;
    int st = 0;

    if (!TEST_ptr(bn = BN_new()))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "0"))
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_ge_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "256"))
            || !TEST_BN_eq_word(bn, 256)
            || !TEST_BN_ge_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "-42"))
            || !TEST_BN_abs_eq_word(bn, 42)
            || !TEST_BN_lt_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "0x1234"))
            || !TEST_BN_eq_word(bn, 0x1234)
            || !TEST_BN_ge_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "0X1234"))
            || !TEST_BN_eq_word(bn, 0x1234)
            || !TEST_BN_ge_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "-0xabcd"))
            || !TEST_BN_abs_eq_word(bn, 0xabcd)
            || !TEST_BN_lt_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "-0"))
            || !TEST_BN_eq_zero(bn)
            || !TEST_BN_ge_zero(bn))
        goto err;

    if (!TEST_true(BN_asc2bn(&bn, "123trailing garbage is ignored"))
            || !TEST_BN_eq_word(bn, 123)
            || !TEST_BN_ge_zero(bn))
        goto err;

    st = 1;
 err:
    BN_free(bn);
    return st;
}

static const MPITEST kMPITests[] = {
    {"0", "\x00\x00\x00\x00", 4},
    {"1", "\x00\x00\x00\x01\x01", 5},
    {"-1", "\x00\x00\x00\x01\x81", 5},
    {"128", "\x00\x00\x00\x02\x00\x80", 6},
    {"256", "\x00\x00\x00\x02\x01\x00", 6},
    {"-256", "\x00\x00\x00\x02\x81\x00", 6},
};

static int test_mpi(int i)
{
    uint8_t scratch[8];
    const MPITEST *test = &kMPITests[i];
    size_t mpi_len, mpi_len2;
    BIGNUM *bn = NULL;
    BIGNUM *bn2 = NULL;
    int st = 0;

    if (!TEST_ptr(bn = BN_new())
            || !TEST_true(BN_asc2bn(&bn, test->base10)))
        goto err;
    mpi_len = BN_bn2mpi(bn, NULL);
    if (!TEST_size_t_le(mpi_len, sizeof(scratch)))
        goto err;

    if (!TEST_size_t_eq(mpi_len2 = BN_bn2mpi(bn, scratch), mpi_len)
            || !TEST_mem_eq(test->mpi, test->mpi_len, scratch, mpi_len))
        goto err;

    if (!TEST_ptr(bn2 = BN_mpi2bn(scratch, mpi_len, NULL)))
        goto err;

    if (!TEST_BN_eq(bn, bn2)) {
        BN_free(bn2);
        goto err;
    }
    BN_free(bn2);

    st = 1;
 err:
    BN_free(bn);
    return st;
}

static int test_bin2zero(void)
{
    unsigned char input[] = { 0 };
    BIGNUM *zbn = NULL;
    int ret = 0;

    if (!TEST_ptr(zbn = BN_new()))
        goto err;

#define zerotest(fn)                           \
    if (!TEST_ptr(fn(input, 1, zbn))    \
        || !TEST_true(BN_is_zero(zbn))   \
        || !TEST_ptr(fn(input, 0, zbn)) \
        || !TEST_true(BN_is_zero(zbn))   \
        || !TEST_ptr(fn(NULL, 0, zbn))  \
        || !TEST_true(BN_is_zero(zbn)))  \
        goto err

    zerotest(BN_bin2bn);
    zerotest(BN_signed_bin2bn);
    zerotest(BN_lebin2bn);
    zerotest(BN_signed_lebin2bn);
#undef zerotest

    ret = 1;
 err:
    BN_free(zbn);
    return ret;
}

static int test_bin2bn_lengths(void)
{
    unsigned char input[] = { 1, 2 };
    BIGNUM *bn_be = NULL, *bn_expected_be = NULL;
    BIGNUM *bn_le = NULL, *bn_expected_le = NULL;
    int ret = 0;

    if (!TEST_ptr(bn_be = BN_new())
        || !TEST_ptr(bn_expected_be = BN_new())
        || !TEST_true(BN_set_word(bn_expected_be, 0x102))
        || !TEST_ptr(bn_le = BN_new())
        || !TEST_ptr(bn_expected_le = BN_new())
        || !TEST_true(BN_set_word(bn_expected_le, 0x201)))
        goto err;

#define lengthtest(fn, e)                                       \
    if (!TEST_ptr_null(fn(input, -1, bn_##e))                   \
        || !TEST_ptr(fn(input, 0, bn_##e))                      \
        || !TEST_true(BN_is_zero(bn_##e))                       \
        || !TEST_ptr(fn(input, 2, bn_##e))                      \
        || !TEST_int_eq(BN_cmp(bn_##e, bn_expected_##e), 0))    \
        goto err

    lengthtest(BN_bin2bn, be);
    lengthtest(BN_signed_bin2bn, be);
    lengthtest(BN_lebin2bn, le);
    lengthtest(BN_signed_lebin2bn, le);
#undef lengthtest

    ret = 1;
 err:
    BN_free(bn_be);
    BN_free(bn_expected_be);
    BN_free(bn_le);
    BN_free(bn_expected_le);
    return ret;
}

static int test_rand(void)
{
    BIGNUM *bn = NULL;
    int st = 0;

    if (!TEST_ptr(bn = BN_new()))
        return 0;

    /* Test BN_rand for degenerate cases with |top| and |bottom| parameters. */
    if (!TEST_false(BN_rand(bn, 0, 0 /* top */ , 0 /* bottom */ ))
            || !TEST_false(BN_rand(bn, 0, 1 /* top */ , 1 /* bottom */ ))
            || !TEST_true(BN_rand(bn, 1, 0 /* top */ , 0 /* bottom */ ))
            || !TEST_BN_eq_one(bn)
            || !TEST_false(BN_rand(bn, 1, 1 /* top */ , 0 /* bottom */ ))
            || !TEST_true(BN_rand(bn, 1, -1 /* top */ , 1 /* bottom */ ))
            || !TEST_BN_eq_one(bn)
            || !TEST_true(BN_rand(bn, 2, 1 /* top */ , 0 /* bottom */ ))
            || !TEST_BN_eq_word(bn, 3))
        goto err;

    st = 1;
 err:
    BN_free(bn);
    return st;
}

/*
 * Run some statistical tests to provide a degree confidence that the
 * BN_rand_range() function works as expected.  The test cases and
 * critical values are generated by the bn_rand_range script.
 *
 * Each individual test is a Chi^2 goodness of fit for a specified number
 * of samples and range.  The samples are assumed to be independent and
 * that they are from a discrete uniform distribution.
 *
 * Some of these individual tests are expected to fail, the success/failure
 * of each is an independent Bernoulli trial.  The number of such successes
 * will form a binomial distribution.  The count of the successes is compared
 * against a precomputed critical value to determine the overall outcome.
 */
struct rand_range_case {
    unsigned int range;
    unsigned int iterations;
    double critical;
};

#include "bn_rand_range.h"

static int test_rand_range_single(size_t n)
{
    const unsigned int range = rand_range_cases[n].range;
    const unsigned int iterations = rand_range_cases[n].iterations;
    const double critical = rand_range_cases[n].critical;
    const double expected = iterations / (double)range;
    double sum = 0;
    BIGNUM *rng = NULL, *val = NULL;
    size_t *counts;
    unsigned int i, v;
    int res = 0;

    if (!TEST_ptr(counts = OPENSSL_zalloc(sizeof(*counts) * range))
        || !TEST_ptr(rng = BN_new())
        || !TEST_ptr(val = BN_new())
        || !TEST_true(BN_set_word(rng, range)))
        goto err;
    for (i = 0; i < iterations; i++) {
        if (!TEST_true(BN_rand_range(val, rng))
            || !TEST_uint_lt(v = (unsigned int)BN_get_word(val), range))
            goto err;
        counts[v]++;
    }

    for (i = 0; i < range; i++) {
        const double delta = counts[i] - expected;
        sum += delta * delta;
    }
    sum /= expected;

    if (sum > critical) {
        TEST_info("Chi^2 test negative %.4f > %4.f", sum, critical);
        TEST_note("test case %zu  range %u  iterations %u", n + 1, range,
                  iterations);
        goto err;
    }

    res = 1;
err:
    BN_free(rng);
    BN_free(val);
    OPENSSL_free(counts);
    return res;
}

static int test_rand_range(void)
{
    int n_success = 0;
    size_t i;

    for (i = 0; i < OSSL_NELEM(rand_range_cases); i++)
        n_success += test_rand_range_single(i);
    if (TEST_int_ge(n_success, binomial_critical))
        return 1;
    TEST_note("This test is expected to fail by chance 0.01%% of the time.");
    return 0;
}

static int test_negzero(void)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL, *d = NULL;
    BIGNUM *numerator = NULL, *denominator = NULL;
    int consttime, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(c = BN_new())
            || !TEST_ptr(d = BN_new()))
        goto err;

    /* Test that BN_mul never gives negative zero. */
    if (!TEST_true(BN_set_word(a, 1)))
        goto err;
    BN_set_negative(a, 1);
    BN_zero(b);
    if (!TEST_true(BN_mul(c, a, b, ctx)))
        goto err;
    if (!TEST_BN_eq_zero(c)
            || !TEST_BN_ge_zero(c))
        goto err;

    for (consttime = 0; consttime < 2; consttime++) {
        if (!TEST_ptr(numerator = BN_new())
                || !TEST_ptr(denominator = BN_new()))
            goto err;
        if (consttime) {
            BN_set_flags(numerator, BN_FLG_CONSTTIME);
            BN_set_flags(denominator, BN_FLG_CONSTTIME);
        }
        /* Test that BN_div never gives negative zero in the quotient. */
        if (!TEST_true(BN_set_word(numerator, 1))
                || !TEST_true(BN_set_word(denominator, 2)))
            goto err;
        BN_set_negative(numerator, 1);
        if (!TEST_true(BN_div(a, b, numerator, denominator, ctx))
                || !TEST_BN_eq_zero(a)
                || !TEST_BN_ge_zero(a))
            goto err;

        /* Test that BN_div never gives negative zero in the remainder. */
        if (!TEST_true(BN_set_word(denominator, 1))
                || !TEST_true(BN_div(a, b, numerator, denominator, ctx))
                || !TEST_BN_eq_zero(b)
                || !TEST_BN_ge_zero(b))
            goto err;
        BN_free(numerator);
        BN_free(denominator);
        numerator = denominator = NULL;
    }

    /* Test that BN_set_negative will not produce a negative zero. */
    BN_zero(a);
    BN_set_negative(a, 1);
    if (BN_is_negative(a))
        goto err;
    st = 1;

 err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(d);
    BN_free(numerator);
    BN_free(denominator);
    return st;
}

static int test_badmod(void)
{
    BIGNUM *a = NULL, *b = NULL, *zero = NULL;
    BN_MONT_CTX *mont = NULL;
    int st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(zero = BN_new())
            || !TEST_ptr(mont = BN_MONT_CTX_new()))
        goto err;
    BN_zero(zero);

    if (!TEST_false(BN_div(a, b, BN_value_one(), zero, ctx)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_mul(a, BN_value_one(), BN_value_one(), zero, ctx)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_exp(a, BN_value_one(), BN_value_one(), zero, ctx)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_exp_mont(a, BN_value_one(), BN_value_one(),
                                    zero, ctx, NULL)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_exp_mont_consttime(a, BN_value_one(), BN_value_one(),
                                              zero, ctx, NULL)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_MONT_CTX_set(mont, zero, ctx)))
        goto err;
    ERR_clear_error();

    /* Some operations also may not be used with an even modulus. */
    if (!TEST_true(BN_set_word(b, 16)))
        goto err;

    if (!TEST_false(BN_MONT_CTX_set(mont, b, ctx)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_exp_mont(a, BN_value_one(), BN_value_one(),
                                    b, ctx, NULL)))
        goto err;
    ERR_clear_error();

    if (!TEST_false(BN_mod_exp_mont_consttime(a, BN_value_one(), BN_value_one(),
                                              b, ctx, NULL)))
        goto err;
    ERR_clear_error();

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(zero);
    BN_MONT_CTX_free(mont);
    return st;
}

static int test_expmodzero(void)
{
    BIGNUM *a = NULL, *r = NULL, *zero = NULL;
    int st = 0;

    if (!TEST_ptr(zero = BN_new())
            || !TEST_ptr(a = BN_new())
            || !TEST_ptr(r = BN_new()))
        goto err;
    BN_zero(zero);

    if (!TEST_true(BN_mod_exp(r, a, zero, BN_value_one(), NULL))
            || !TEST_BN_eq_zero(r)
            || !TEST_true(BN_mod_exp_mont(r, a, zero, BN_value_one(),
                                          NULL, NULL))
            || !TEST_BN_eq_zero(r)
            || !TEST_true(BN_mod_exp_mont_consttime(r, a, zero,
                                                    BN_value_one(),
                                                    NULL, NULL))
            || !TEST_BN_eq_zero(r)
            || !TEST_true(BN_mod_exp_mont_word(r, 42, zero,
                                               BN_value_one(), NULL, NULL))
            || !TEST_BN_eq_zero(r))
        goto err;

    st = 1;
 err:
    BN_free(zero);
    BN_free(a);
    BN_free(r);
    return st;
}

static int test_expmodone(void)
{
    int ret = 0, i;
    BIGNUM *r = BN_new();
    BIGNUM *a = BN_new();
    BIGNUM *p = BN_new();
    BIGNUM *m = BN_new();

    if (!TEST_ptr(r)
            || !TEST_ptr(a)
            || !TEST_ptr(p)
            || !TEST_ptr(p)
            || !TEST_ptr(m)
            || !TEST_true(BN_set_word(a, 1))
            || !TEST_true(BN_set_word(p, 0))
            || !TEST_true(BN_set_word(m, 1)))
        goto err;

    /* Calculate r = 1 ^ 0 mod 1, and check the result is always 0 */
    for (i = 0; i < 2; i++) {
        if (!TEST_true(BN_mod_exp(r, a, p, m, NULL))
                || !TEST_BN_eq_zero(r)
                || !TEST_true(BN_mod_exp_mont(r, a, p, m, NULL, NULL))
                || !TEST_BN_eq_zero(r)
                || !TEST_true(BN_mod_exp_mont_consttime(r, a, p, m, NULL, NULL))
                || !TEST_BN_eq_zero(r)
                || !TEST_true(BN_mod_exp_mont_word(r, 1, p, m, NULL, NULL))
                || !TEST_BN_eq_zero(r)
                || !TEST_true(BN_mod_exp_simple(r, a, p, m, NULL))
                || !TEST_BN_eq_zero(r)
                || !TEST_true(BN_mod_exp_recp(r, a, p, m, NULL))
                || !TEST_BN_eq_zero(r))
            goto err;
        /* Repeat for r = 1 ^ 0 mod -1 */
        if (i == 0)
            BN_set_negative(m, 1);
    }

    ret = 1;
 err:
    BN_free(r);
    BN_free(a);
    BN_free(p);
    BN_free(m);
    return ret;
}

static int test_smallprime(int kBits)
{
    BIGNUM *r;
    int st = 0;

    if (!TEST_ptr(r = BN_new()))
        goto err;

    if (kBits <= 1) {
        if (!TEST_false(BN_generate_prime_ex(r, kBits, 0,
                                             NULL, NULL, NULL)))
            goto err;
    } else {
        if (!TEST_true(BN_generate_prime_ex(r, kBits, 0,
                                            NULL, NULL, NULL))
                || !TEST_int_eq(BN_num_bits(r), kBits))
            goto err;
    }

    st = 1;
 err:
    BN_free(r);
    return st;
}

static int test_smallsafeprime(int kBits)
{
    BIGNUM *r;
    int st = 0;

    if (!TEST_ptr(r = BN_new()))
        goto err;

    if (kBits <= 5 && kBits != 3) {
        if (!TEST_false(BN_generate_prime_ex(r, kBits, 1,
                                             NULL, NULL, NULL)))
            goto err;
    } else {
        if (!TEST_true(BN_generate_prime_ex(r, kBits, 1,
                                            NULL, NULL, NULL))
                || !TEST_int_eq(BN_num_bits(r), kBits))
            goto err;
    }

    st = 1;
 err:
    BN_free(r);
    return st;
}

static int primes[] = { 2, 3, 5, 7, 17863 };

static int test_is_prime(int i)
{
    int ret = 0;
    BIGNUM *r = NULL;
    int trial;

    if (!TEST_ptr(r = BN_new()))
        goto err;

    for (trial = 0; trial <= 1; ++trial) {
        if (!TEST_true(BN_set_word(r, primes[i]))
                || !TEST_int_eq(BN_check_prime(r, ctx, NULL),
                                1))
            goto err;
    }

    ret = 1;
 err:
    BN_free(r);
    return ret;
}

static int not_primes[] = { -1, 0, 1, 4 };

static int test_not_prime(int i)
{
    int ret = 0;
    BIGNUM *r = NULL;
    int trial;

    if (!TEST_ptr(r = BN_new()))
        goto err;

    for (trial = 0; trial <= 1; ++trial) {
        if (!TEST_true(BN_set_word(r, not_primes[i]))
                || !TEST_int_eq(BN_check_prime(r, ctx, NULL), 0))
            goto err;
    }

    ret = 1;
 err:
    BN_free(r);
    return ret;
}

static int test_ctx_set_ct_flag(BN_CTX *c)
{
    int st = 0;
    size_t i;
    BIGNUM *b[15];

    BN_CTX_start(c);
    for (i = 0; i < OSSL_NELEM(b); i++) {
        if (!TEST_ptr(b[i] = BN_CTX_get(c)))
            goto err;
        if (i % 2 == 1)
            BN_set_flags(b[i], BN_FLG_CONSTTIME);
    }

    st = 1;
 err:
    BN_CTX_end(c);
    return st;
}

static int test_ctx_check_ct_flag(BN_CTX *c)
{
    int st = 0;
    size_t i;
    BIGNUM *b[30];

    BN_CTX_start(c);
    for (i = 0; i < OSSL_NELEM(b); i++) {
        if (!TEST_ptr(b[i] = BN_CTX_get(c)))
            goto err;
        if (!TEST_false(BN_get_flags(b[i], BN_FLG_CONSTTIME)))
            goto err;
    }

    st = 1;
 err:
    BN_CTX_end(c);
    return st;
}

static int test_ctx_consttime_flag(void)
{
    /*-
     * The constant-time flag should not "leak" among BN_CTX frames:
     *
     * - test_ctx_set_ct_flag() starts a frame in the given BN_CTX and
     *   sets the BN_FLG_CONSTTIME flag on some of the BIGNUMs obtained
     *   from the frame before ending it.
     * - test_ctx_check_ct_flag() then starts a new frame and gets a
     *   number of BIGNUMs from it. In absence of leaks, none of the
     *   BIGNUMs in the new frame should have BN_FLG_CONSTTIME set.
     *
     * In actual BN_CTX usage inside libcrypto the leak could happen at
     * any depth level in the BN_CTX stack, with varying results
     * depending on the patterns of sibling trees of nested function
     * calls sharing the same BN_CTX object, and the effect of
     * unintended BN_FLG_CONSTTIME on the called BN_* functions.
     *
     * This simple unit test abstracts away this complexity and verifies
     * that the leak does not happen between two sibling functions
     * sharing the same BN_CTX object at the same level of nesting.
     *
     */
    BN_CTX *nctx = NULL;
    BN_CTX *sctx = NULL;
    size_t i = 0;
    int st = 0;

    if (!TEST_ptr(nctx = BN_CTX_new())
            || !TEST_ptr(sctx = BN_CTX_secure_new()))
        goto err;

    for (i = 0; i < 2; i++) {
        BN_CTX *c = i == 0 ? nctx : sctx;
        if (!TEST_true(test_ctx_set_ct_flag(c))
                || !TEST_true(test_ctx_check_ct_flag(c)))
            goto err;
    }

    st = 1;
 err:
    BN_CTX_free(nctx);
    BN_CTX_free(sctx);
    return st;
}

static int test_coprime(void)
{
    BIGNUM *a = NULL, *b = NULL;
    int ret = 0;

    ret = TEST_ptr(a = BN_new())
          && TEST_ptr(b = BN_new())
          && TEST_true(BN_set_word(a, 66))
          && TEST_true(BN_set_word(b, 99))
          && TEST_int_eq(BN_are_coprime(a, b, ctx), 0)
          && TEST_int_eq(BN_are_coprime(b, a, ctx), 0)
          && TEST_true(BN_set_word(a, 67))
          && TEST_int_eq(BN_are_coprime(a, b, ctx), 1)
          && TEST_int_eq(BN_are_coprime(b, a, ctx), 1);
    BN_free(a);
    BN_free(b);
    return ret;
}

static int test_gcd_prime(void)
{
    BIGNUM *a = NULL, *b = NULL, *gcd = NULL;
    int i, st = 0;

    if (!TEST_ptr(a = BN_new())
            || !TEST_ptr(b = BN_new())
            || !TEST_ptr(gcd = BN_new()))
        goto err;

    if (!TEST_true(BN_generate_prime_ex(a, 1024, 0, NULL, NULL, NULL)))
            goto err;
    for (i = 0; i < NUM_PRIME_TESTS; i++) {
        if (!TEST_true(BN_generate_prime_ex(b, 1024, 0,
                                            NULL, NULL, NULL))
                || !TEST_true(BN_gcd(gcd, a, b, ctx))
                || !TEST_true(BN_is_one(gcd))
                || !TEST_true(BN_are_coprime(a, b, ctx)))
            goto err;
    }

    st = 1;
 err:
    BN_free(a);
    BN_free(b);
    BN_free(gcd);
    return st;
}

typedef struct mod_exp_test_st {
    const char *base;
    const char *exp;
    const char *mod;
    const char *res;
} MOD_EXP_TEST;

static const MOD_EXP_TEST ModExpTests[] = {
   /* original test vectors for rsaz_512_sqr bug, by OSS-Fuzz */
   {
       "1166180238001879113042182292626169621106255558914000595999312084"
       "4627946820899490684928760491249738643524880720584249698100907201"
       "002086675047927600340800371",
       "8000000000000000000000000000000000000000000000000000000000000000"
       "0000000000000000000000000000000000000000000000000000000000000000"
       "00000000",
       "1340780792684523720980737645613191762604395855615117867483316354"
       "3294276330515137663421134775482798690129946803802212663956180562"
       "088664022929883876655300863",
       "8243904058268085430037326628480645845409758077568738532059032482"
       "8294114415890603594730158120426756266457928475330450251339773498"
       "26758407619521544102068438"
   },
   {
       "4974270041410803822078866696159586946995877618987010219312844726"
       "0284386121835740784990869050050504348861513337232530490826340663"
       "197278031692737429054",
       "4974270041410803822078866696159586946995877428188754995041148539"
       "1663243362592271353668158565195557417149981094324650322556843202"
       "946445882670777892608",
       "1340780716511420227215592830971452482815377482627251725537099028"
       "4429769497230131760206012644403029349547320953206103351725462999"
       "947509743623340557059752191",
       "5296244594780707015616522701706118082963369547253192207884519362"
       "1767869984947542695665420219028522815539559194793619684334900442"
       "49304558011362360473525933"
   },
   /* test vectors for rsaz_512_srq bug, with rcx/rbx=1 */
   {   /* between first and second iteration */
       "5148719036160389201525610950887605325980251964889646556085286545"
       "3931548809178823413169359635978762036512397113080988070677858033"
       "36463909753993540214027190",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between second and third iteration */
       "8908340854353752577419678771330460827942371434853054158622636544"
       "8151360109722890949471912566649465436296659601091730745087014189"
       "2672764191218875181826063",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between third and fourth iteration */
       "3427446396505596330634350984901719674479522569002785244080234738"
       "4288743635435746136297299366444548736533053717416735379073185344"
       "26985272974404612945608761",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between fourth and fifth iteration */
       "3472743044917564564078857826111874560045331237315597383869652985"
       "6919870028890895988478351133601517365908445058405433832718206902"
       "4088133164805266956353542",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between fifth and sixth iteration */
       "3608632990153469264412378349742339216742409743898601587274768025"
       "0110772032985643555192767717344946174122842255204082586753499651"
       "14483434992887431333675068",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between sixth and seventh iteration */
       "8455374370234070242910508226941981520235709767260723212165264877"
       "8689064388017521524568434328264431772644802567028663962962025746"
       "9283458217850119569539086",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between seventh and eighth iteration */
       "5155371529688532178421209781159131443543419764974688878527112131"
       "7446518205609427412336183157918981038066636807317733319323257603"
       "04416292040754017461076359",
       "1005585594745694782468051874865438459560952436544429503329267108"
       "2791323022555160232601405723625177570767523893639864538140315412"
       "108959927459825236754563832",
       "1005585594745694782468051874865438459560952436544429503329267108"
       "2791323022555160232601405723625177570767523893639864538140315412"
       "108959927459825236754563833",
       "1"
   },
   /* test vectors for rsaz_512_srq bug, with rcx/rbx=2 */
   {   /* between first and second iteration */
       "3155666506033786929967309937640790361084670559125912405342594979"
       "4345142818528956285490897841406338022378565972533508820577760065"
       "58494345853302083699912572",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between second and third iteration */
       "3789819583801342198190405714582958759005991915505282362397087750"
       "4213544724644823098843135685133927198668818185338794377239590049"
       "41019388529192775771488319",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between third and forth iteration */
       "4695752552040706867080542538786056470322165281761525158189220280"
       "4025547447667484759200742764246905647644662050122968912279199065"
       "48065034299166336940507214",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between forth and fifth iteration */
       "2159140240970485794188159431017382878636879856244045329971239574"
       "8919691133560661162828034323196457386059819832804593989740268964"
       "74502911811812651475927076",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between fifth and sixth iteration */
       "5239312332984325668414624633307915097111691815000872662334695514"
       "5436533521392362443557163429336808208137221322444780490437871903"
       "99972784701334569424519255",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between sixth and seventh iteration */
       "1977953647322612860406858017869125467496941904523063466791308891"
       "1172796739058531929470539758361774569875505293428856181093904091"
       "33788264851714311303725089",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042158",
       "6703903964971298549787012499102923063739682910296196688861780721"
       "8608820150367734884009371490834517138450159290932430254268769414"
       "05973284973216824503042159",
       "1"
   },
   {   /* between seventh and eighth iteration */
       "6456987954117763835533395796948878140715006860263624787492985786"
       "8514630216966738305923915688821526449499763719943997120302368211"
       "04813318117996225041943964",
       "1340780792994259709957402499820584612747936582059239337772356144"
       "3721764030073546976801874298166903427690031858186486050853753882"
       "811946551499689575296532556",
       "1340780792994259709957402499820584612747936582059239337772356144"
       "3721764030073546976801874298166903427690031858186486050853753882"
       "811946551499689575296532557",
       "1"
   }
};

static int test_mod_exp(int i)
{
    const MOD_EXP_TEST *test = &ModExpTests[i];
    int res = 0;
    BIGNUM* result = NULL;
    BIGNUM *base = NULL, *exponent = NULL, *modulo = NULL;
    char *s = NULL;

    if (!TEST_ptr(result = BN_new())
            || !TEST_true(BN_dec2bn(&base, test->base))
            || !TEST_true(BN_dec2bn(&exponent, test->exp))
            || !TEST_true(BN_dec2bn(&modulo, test->mod)))
        goto err;

    if (!TEST_int_eq(BN_mod_exp(result, base, exponent, modulo, ctx), 1))
        goto err;

    if (!TEST_ptr(s = BN_bn2dec(result)))
        goto err;

    if (!TEST_mem_eq(s, strlen(s), test->res, strlen(test->res)))
        goto err;

    res = 1;

 err:
    OPENSSL_free(s);
    BN_free(result);
    BN_free(base);
    BN_free(exponent);
    BN_free(modulo);
    return res;
}

static int test_mod_exp_consttime(int i)
{
    const MOD_EXP_TEST *test = &ModExpTests[i];
    int res = 0;
    BIGNUM* result = NULL;
    BIGNUM *base = NULL, *exponent = NULL, *modulo = NULL;
    char *s = NULL;

    if (!TEST_ptr(result = BN_new())
            || !TEST_true(BN_dec2bn(&base, test->base))
            || !TEST_true(BN_dec2bn(&exponent, test->exp))
            || !TEST_true(BN_dec2bn(&modulo, test->mod)))
        goto err;

    BN_set_flags(base, BN_FLG_CONSTTIME);
    BN_set_flags(exponent, BN_FLG_CONSTTIME);
    BN_set_flags(modulo, BN_FLG_CONSTTIME);

    if (!TEST_int_eq(BN_mod_exp(result, base, exponent, modulo, ctx), 1))
        goto err;

    if (!TEST_ptr(s = BN_bn2dec(result)))
        goto err;

    if (!TEST_mem_eq(s, strlen(s), test->res, strlen(test->res)))
        goto err;

    res = 1;

 err:
    OPENSSL_free(s);
    BN_free(result);
    BN_free(base);
    BN_free(exponent);
    BN_free(modulo);
    return res;
}

/*
 * Regression test to ensure BN_mod_exp2_mont fails safely if argument m is
 * zero.
 */
static int test_mod_exp2_mont(void)
{
    int res = 0;
    BIGNUM *exp_result = NULL;
    BIGNUM *exp_a1 = NULL, *exp_p1 = NULL, *exp_a2 = NULL, *exp_p2 = NULL,
           *exp_m = NULL;

    if (!TEST_ptr(exp_result = BN_new())
            || !TEST_ptr(exp_a1 = BN_new())
            || !TEST_ptr(exp_p1 = BN_new())
            || !TEST_ptr(exp_a2 = BN_new())
            || !TEST_ptr(exp_p2 = BN_new())
            || !TEST_ptr(exp_m = BN_new()))
        goto err;

    if (!TEST_true(BN_one(exp_a1))
            || !TEST_true(BN_one(exp_p1))
            || !TEST_true(BN_one(exp_a2))
            || !TEST_true(BN_one(exp_p2)))
        goto err;

    BN_zero(exp_m);

    /* input of 0 is even, so must fail */
    if (!TEST_int_eq(BN_mod_exp2_mont(exp_result, exp_a1, exp_p1, exp_a2,
                exp_p2, exp_m, ctx, NULL), 0))
        goto err;

    res = 1;

err:
    BN_free(exp_result);
    BN_free(exp_a1);
    BN_free(exp_p1);
    BN_free(exp_a2);
    BN_free(exp_p2);
    BN_free(exp_m);
    return res;
}

static int test_mod_inverse(void)
{
    int res = 0;
    char *str = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *r = NULL;

    if (!TEST_true(BN_dec2bn(&a, "5193817943")))
        goto err;
    if (!TEST_true(BN_dec2bn(&b, "3259122431")))
        goto err;
    if (!TEST_ptr(r = BN_new()))
        goto err;
    if (!TEST_ptr_eq(BN_mod_inverse(r, a, b, ctx), r))
        goto err;
    if (!TEST_ptr_ne(str = BN_bn2dec(r), NULL))
        goto err;
    if (!TEST_int_eq(strcmp(str, "2609653924"), 0))
        goto err;

    /* Note that this aliases the result with the modulus. */
    if (!TEST_ptr_null(BN_mod_inverse(b, a, b, ctx)))
        goto err;

    res = 1;

err:
    BN_free(a);
    BN_free(b);
    BN_free(r);
    OPENSSL_free(str);
    return res;
}

static int test_mod_exp_alias(int idx)
{
    int res = 0;
    char *str = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *c = NULL;
    BIGNUM *r = NULL;

    if (!TEST_true(BN_dec2bn(&a, "15")))
        goto err;
    if (!TEST_true(BN_dec2bn(&b, "10")))
        goto err;
    if (!TEST_true(BN_dec2bn(&c, "39")))
        goto err;
    if (!TEST_ptr(r = BN_new()))
        goto err;

    if (!TEST_int_eq((idx == 0 ? BN_mod_exp_simple
                               : BN_mod_exp_recp)(r, a, b, c, ctx), 1))
        goto err;
    if (!TEST_ptr_ne(str = BN_bn2dec(r), NULL))
        goto err;
    if (!TEST_str_eq(str, "36"))
        goto err;

    OPENSSL_free(str);
    str = NULL;

    BN_copy(r, b);

    /* Aliasing with exponent must work. */
    if (!TEST_int_eq((idx == 0 ? BN_mod_exp_simple
                               : BN_mod_exp_recp)(r, a, r, c, ctx), 1))
        goto err;
    if (!TEST_ptr_ne(str = BN_bn2dec(r), NULL))
        goto err;
    if (!TEST_str_eq(str, "36"))
        goto err;

    OPENSSL_free(str);
    str = NULL;

    /* Aliasing with modulus should return failure for the simple call. */
    if (idx == 0) {
        if (!TEST_int_eq(BN_mod_exp_simple(c, a, b, c, ctx), 0))
            goto err;
    } else {
        if (!TEST_int_eq(BN_mod_exp_recp(c, a, b, c, ctx), 1))
            goto err;
        if (!TEST_ptr_ne(str = BN_bn2dec(c), NULL))
            goto err;
        if (!TEST_str_eq(str, "36"))
            goto err;
    }

    res = 1;

err:
    BN_free(a);
    BN_free(b);
    BN_free(c);
    BN_free(r);
    OPENSSL_free(str);
    return res;
}

static int file_test_run(STANZA *s)
{
    static const FILETEST filetests[] = {
        {"Sum", file_sum},
        {"LShift1", file_lshift1},
        {"LShift", file_lshift},
        {"RShift", file_rshift},
        {"Square", file_square},
        {"Product", file_product},
        {"Quotient", file_quotient},
        {"ModMul", file_modmul},
        {"ModExp", file_modexp},
        {"Exp", file_exp},
        {"ModSqrt", file_modsqrt},
        {"GCD", file_gcd},
    };
    int numtests = OSSL_NELEM(filetests);
    const FILETEST *tp = filetests;

    for ( ; --numtests >= 0; tp++) {
        if (findattr(s, tp->name) != NULL) {
            if (!tp->func(s)) {
                TEST_info("%s:%d: Failed %s test",
                          s->test_file, s->start, tp->name);
                return 0;
            }
            return 1;
        }
    }
    TEST_info("%s:%d: Unknown test", s->test_file, s->start);
    return 0;
}

static int run_file_tests(int i)
{
    STANZA *s = NULL;
    char *testfile = test_get_argument(i);
    int c;

    if (!TEST_ptr(s = OPENSSL_zalloc(sizeof(*s))))
        return 0;
    if (!test_start_file(s, testfile)) {
        OPENSSL_free(s);
        return 0;
    }

    /* Read test file. */
    while (!BIO_eof(s->fp) && test_readstanza(s)) {
        if (s->numpairs == 0)
            continue;
        if (!file_test_run(s))
            s->errors++;
        s->numtests++;
        test_clearstanza(s);
    }
    test_end_file(s);
    c = s->errors;
    OPENSSL_free(s);

    return c == 0;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_STOCHASTIC_TESTS,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("[file...]\n"),
        { "stochastic", OPT_STOCHASTIC_TESTS, '-', "Run stochastic tests" },
        { OPT_HELP_STR, 1, '-',
          "file\tFile to run tests on. Normal tests are not run\n" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    int n, stochastic = 0;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_STOCHASTIC_TESTS:
            stochastic = 1;
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }
    n  = test_get_argument_count();

    if (!TEST_ptr(ctx = BN_CTX_new()))
        return 0;

    if (n == 0) {
        ADD_TEST(test_sub);
        ADD_TEST(test_div_recip);
        ADD_ALL_TESTS(test_signed_mod_replace_ab, OSSL_NELEM(signed_mod_tests));
        ADD_ALL_TESTS(test_signed_mod_replace_ba, OSSL_NELEM(signed_mod_tests));
        ADD_TEST(test_mod);
        ADD_TEST(test_mod_inverse);
        ADD_ALL_TESTS(test_mod_exp_alias, 2);
        ADD_TEST(test_modexp_mont5);
        ADD_TEST(test_kronecker);
        ADD_TEST(test_rand);
        ADD_TEST(test_bn2padded);
        ADD_TEST(test_dec2bn);
        ADD_TEST(test_hex2bn);
        ADD_TEST(test_asc2bn);
        ADD_TEST(test_bin2zero);
        ADD_TEST(test_bin2bn_lengths);
        ADD_ALL_TESTS(test_mpi, (int)OSSL_NELEM(kMPITests));
        ADD_ALL_TESTS(test_bn2signed, (int)OSSL_NELEM(kSignedTests_BE));
        ADD_TEST(test_negzero);
        ADD_TEST(test_badmod);
        ADD_TEST(test_expmodzero);
        ADD_TEST(test_expmodone);
        ADD_ALL_TESTS(test_smallprime, 16);
        ADD_ALL_TESTS(test_smallsafeprime, 16);
        ADD_TEST(test_swap);
        ADD_TEST(test_ctx_consttime_flag);
#ifndef OPENSSL_NO_EC2M
        ADD_TEST(test_gf2m_add);
        ADD_TEST(test_gf2m_mod);
        ADD_TEST(test_gf2m_mul);
        ADD_TEST(test_gf2m_sqr);
        ADD_TEST(test_gf2m_modinv);
        ADD_TEST(test_gf2m_moddiv);
        ADD_TEST(test_gf2m_modexp);
        ADD_TEST(test_gf2m_modsqrt);
        ADD_TEST(test_gf2m_modsolvequad);
#endif
        ADD_ALL_TESTS(test_is_prime, (int)OSSL_NELEM(primes));
        ADD_ALL_TESTS(test_not_prime, (int)OSSL_NELEM(not_primes));
        ADD_TEST(test_gcd_prime);
        ADD_TEST(test_coprime);
        ADD_ALL_TESTS(test_mod_exp, (int)OSSL_NELEM(ModExpTests));
        ADD_ALL_TESTS(test_mod_exp_consttime, (int)OSSL_NELEM(ModExpTests));
        ADD_TEST(test_mod_exp2_mont);
        if (stochastic)
            ADD_TEST(test_rand_range);
    } else {
        ADD_ALL_TESTS(run_file_tests, n);
    }
    return 1;
}

void cleanup_tests(void)
{
    BN_CTX_free(ctx);
}
