/*
 * Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>

#include "internal/nelem.h"

#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/bn.h>

#include "testutil.h"

#include "rsa_local.h"
#include <openssl/rsa.h>

/* taken from RSA2 cavs data */
static const unsigned char cav_e[] = {
    0x01,0x00,0x01
};
static const unsigned char cav_p[] = {
    0xcf,0x72,0x1b,0x9a,0xfd,0x0d,0x22,0x1a,0x74,0x50,0x97,0x22,0x76,0xd8,0xc0,
    0xc2,0xfd,0x08,0x81,0x05,0xdd,0x18,0x21,0x99,0x96,0xd6,0x5c,0x79,0xe3,0x02,
    0x81,0xd7,0x0e,0x3f,0x3b,0x34,0xda,0x61,0xc9,0x2d,0x84,0x86,0x62,0x1e,0x3d,
    0x5d,0xbf,0x92,0x2e,0xcd,0x35,0x3d,0x6e,0xb9,0x59,0x16,0xc9,0x82,0x50,0x41,
    0x30,0x45,0x67,0xaa,0xb7,0xbe,0xec,0xea,0x4b,0x9e,0xa0,0xc3,0x05,0xbc,0x4c,
    0x01,0xa5,0x4b,0xbd,0xa4,0x20,0xb5,0x20,0xd5,0x59,0x6f,0x82,0x5c,0x8f,0x4f,
    0xe0,0x3a,0x4e,0x7e,0xfe,0x44,0xf3,0x3c,0xc0,0x0e,0x14,0x2b,0x32,0xe6,0x28,
    0x8b,0x63,0x87,0x00,0xc3,0x53,0x4a,0x5b,0x71,0x7a,0x5b,0x28,0x40,0xc4,0x18,
    0xb6,0x77,0x0b,0xab,0x59,0xa4,0x96,0x7d
};
static const unsigned char cav_q[] = {
    0xfe,0xab,0xf2,0x7c,0x16,0x4a,0xf0,0x8d,0x31,0xc6,0x0a,0x82,0xe2,0xae,0xbb,
    0x03,0x7e,0x7b,0x20,0x4e,0x64,0xb0,0x16,0xad,0x3c,0x01,0x1a,0xd3,0x54,0xbf,
    0x2b,0xa4,0x02,0x9e,0xc3,0x0d,0x60,0x3d,0x1f,0xb9,0xc0,0x0d,0xe6,0x97,0x68,
    0xbb,0x8c,0x81,0xd5,0xc1,0x54,0x96,0x0f,0x99,0xf0,0xa8,0xa2,0xf3,0xc6,0x8e,
    0xec,0xbc,0x31,0x17,0x70,0x98,0x24,0xa3,0x36,0x51,0xa8,0x54,0xc4,0x44,0xdd,
    0xf7,0x7e,0xda,0x47,0x4a,0x67,0x44,0x5d,0x4e,0x75,0xf0,0x4d,0x00,0x68,0xe1,
    0x4a,0xec,0x1f,0x45,0xf9,0xe6,0xca,0x38,0x95,0x48,0x6f,0xdc,0x9d,0x1b,0xa3,
    0x4b,0xfd,0x08,0x4b,0x54,0xcd,0xeb,0x3d,0xef,0x33,0x11,0x6e,0xce,0xe4,0x5d,
    0xef,0xa9,0x58,0x5c,0x87,0x4d,0xc8,0xcf
};
static const unsigned char cav_n[] = {
    0xce,0x5e,0x8d,0x1a,0xa3,0x08,0x7a,0x2d,0xb4,0x49,0x48,0xf0,0x06,0xb6,0xfe,
    0xba,0x2f,0x39,0x7c,0x7b,0xe0,0x5d,0x09,0x2d,0x57,0x4e,0x54,0x60,0x9c,0xe5,
    0x08,0x4b,0xe1,0x1a,0x73,0xc1,0x5e,0x2f,0xb6,0x46,0xd7,0x81,0xca,0xbc,0x98,
    0xd2,0xf9,0xef,0x1c,0x92,0x8c,0x8d,0x99,0x85,0x28,0x52,0xd6,0xd5,0xab,0x70,
    0x7e,0x9e,0xa9,0x87,0x82,0xc8,0x95,0x64,0xeb,0xf0,0x6c,0x0f,0x3f,0xe9,0x02,
    0x29,0x2e,0x6d,0xa1,0xec,0xbf,0xdc,0x23,0xdf,0x82,0x4f,0xab,0x39,0x8d,0xcc,
    0xac,0x21,0x51,0x14,0xf8,0xef,0xec,0x73,0x80,0x86,0xa3,0xcf,0x8f,0xd5,0xcf,
    0x22,0x1f,0xcc,0x23,0x2f,0xba,0xcb,0xf6,0x17,0xcd,0x3a,0x1f,0xd9,0x84,0xb9,
    0x88,0xa7,0x78,0x0f,0xaa,0xc9,0x04,0x01,0x20,0x72,0x5d,0x2a,0xfe,0x5b,0xdd,
    0x16,0x5a,0xed,0x83,0x02,0x96,0x39,0x46,0x37,0x30,0xc1,0x0d,0x87,0xc2,0xc8,
    0x33,0x38,0xed,0x35,0x72,0xe5,0x29,0xf8,0x1f,0x23,0x60,0xe1,0x2a,0x5b,0x1d,
    0x6b,0x53,0x3f,0x07,0xc4,0xd9,0xbb,0x04,0x0c,0x5c,0x3f,0x0b,0xc4,0xd4,0x61,
    0x96,0x94,0xf1,0x0f,0x4a,0x49,0xac,0xde,0xd2,0xe8,0x42,0xb3,0x4a,0x0b,0x64,
    0x7a,0x32,0x5f,0x2b,0x5b,0x0f,0x8b,0x8b,0xe0,0x33,0x23,0x34,0x64,0xf8,0xb5,
    0x7f,0x69,0x60,0xb8,0x71,0xe9,0xff,0x92,0x42,0xb1,0xf7,0x23,0xa8,0xa7,0x92,
    0x04,0x3d,0x6b,0xff,0xf7,0xab,0xbb,0x14,0x1f,0x4c,0x10,0x97,0xd5,0x6b,0x71,
    0x12,0xfd,0x93,0xa0,0x4a,0x3b,0x75,0x72,0x40,0x96,0x1c,0x5f,0x40,0x40,0x57,
    0x13
};
static const unsigned char cav_d[] = {
    0x47,0x47,0x49,0x1d,0x66,0x2a,0x4b,0x68,0xf5,0xd8,0x4a,0x24,0xfd,0x6c,0xbf,
    0x56,0xb7,0x70,0xf7,0x9a,0x21,0xc8,0x80,0x9e,0xf4,0x84,0xcd,0x88,0x01,0x28,
    0xea,0x50,0xab,0x13,0x63,0xdf,0xea,0x14,0x38,0xb5,0x07,0x42,0x81,0x2f,0xda,
    0xe9,0x24,0x02,0x7e,0xaf,0xef,0x74,0x09,0x0e,0x80,0xfa,0xfb,0xd1,0x19,0x41,
    0xe5,0xba,0x0f,0x7c,0x0a,0xa4,0x15,0x55,0xa2,0x58,0x8c,0x3a,0x48,0x2c,0xc6,
    0xde,0x4a,0x76,0xfb,0x72,0xb6,0x61,0xe6,0xd2,0x10,0x44,0x4c,0x33,0xb8,0xd2,
    0x74,0xb1,0x9d,0x3b,0xcd,0x2f,0xb1,0x4f,0xc3,0x98,0xbd,0x83,0xb7,0x7e,0x75,
    0xe8,0xa7,0x6a,0xee,0xcc,0x51,0x8c,0x99,0x17,0x67,0x7f,0x27,0xf9,0x0d,0x6a,
    0xb7,0xd4,0x80,0x17,0x89,0x39,0x9c,0xf3,0xd7,0x0f,0xdf,0xb0,0x55,0x80,0x1d,
    0xaf,0x57,0x2e,0xd0,0xf0,0x4f,0x42,0x69,0x55,0xbc,0x83,0xd6,0x97,0x83,0x7a,
    0xe6,0xc6,0x30,0x6d,0x3d,0xb5,0x21,0xa7,0xc4,0x62,0x0a,0x20,0xce,0x5e,0x5a,
    0x17,0x98,0xb3,0x6f,0x6b,0x9a,0xeb,0x6b,0xa3,0xc4,0x75,0xd8,0x2b,0xdc,0x5c,
    0x6f,0xec,0x5d,0x49,0xac,0xa8,0xa4,0x2f,0xb8,0x8c,0x4f,0x2e,0x46,0x21,0xee,
    0x72,0x6a,0x0e,0x22,0x80,0x71,0xc8,0x76,0x40,0x44,0x61,0x16,0xbf,0xa5,0xf8,
    0x89,0xc7,0xe9,0x87,0xdf,0xbd,0x2e,0x4b,0x4e,0xc2,0x97,0x53,0xe9,0x49,0x1c,
    0x05,0xb0,0x0b,0x9b,0x9f,0x21,0x19,0x41,0xe9,0xf5,0x61,0xd7,0x33,0x2e,0x2c,
    0x94,0xb8,0xa8,0x9a,0x3a,0xcc,0x6a,0x24,0x8d,0x19,0x13,0xee,0xb9,0xb0,0x48,
    0x61
};

/* helper function */
static BIGNUM *bn_load_new(const unsigned char *data, int sz)
{
    BIGNUM *ret = BN_new();
    if (ret != NULL)
        BN_bin2bn(data, sz, ret);
    return ret;
}

/* Check that small rsa exponents are allowed in non FIPS mode */
static int test_check_public_exponent(void)
{
    int ret = 0;
    BIGNUM *e = NULL;

    ret = TEST_ptr(e = BN_new())
          /* e is too small will fail */
          && TEST_true(BN_set_word(e, 1))
          && TEST_false(ossl_rsa_check_public_exponent(e))
          /* e is even will fail */
          && TEST_true(BN_set_word(e, 65536))
          && TEST_false(ossl_rsa_check_public_exponent(e))
          /* e is ok */
          && TEST_true(BN_set_word(e, 3))
          && TEST_true(ossl_rsa_check_public_exponent(e))
          && TEST_true(BN_set_word(e, 17))
          && TEST_true(ossl_rsa_check_public_exponent(e))
          && TEST_true(BN_set_word(e, 65537))
          && TEST_true(ossl_rsa_check_public_exponent(e))
          /* e = 2^256 + 1 is ok */
          && TEST_true(BN_lshift(e, BN_value_one(), 256))
          && TEST_true(BN_add(e, e, BN_value_one()))
          && TEST_true(ossl_rsa_check_public_exponent(e));
    BN_free(e);
    return ret;
}

static int test_check_prime_factor_range(void)
{
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL;
    BIGNUM *bn_p1 = NULL, *bn_p2 = NULL, *bn_p3 = NULL, *bn_p4 = NULL;
    /* Some range checks that are larger than 32 bits */
    static const unsigned char p1[] = { 0x0B, 0x50, 0x4F, 0x33, 0x3F };
    static const unsigned char p2[] = { 0x10, 0x00, 0x00, 0x00, 0x00 };
    static const unsigned char p3[] = { 0x0B, 0x50, 0x4F, 0x33, 0x40 };
    static const unsigned char p4[] = { 0x0F, 0xFF, 0xFF, 0xFF, 0xFF };

    /* (√2)(2^(nbits/2 - 1) <= p <= 2^(nbits/2) - 1
     * For 8 bits:   0xB.504F <= p <= 0xF
     * for 72 bits:  0xB504F333F. <= p <= 0xF_FFFF_FFFF
     */
    ret = TEST_ptr(p = BN_new())
          && TEST_ptr(bn_p1 = bn_load_new(p1, sizeof(p1)))
          && TEST_ptr(bn_p2 = bn_load_new(p2, sizeof(p2)))
          && TEST_ptr(bn_p3 = bn_load_new(p3, sizeof(p3)))
          && TEST_ptr(bn_p4 = bn_load_new(p4, sizeof(p4)))
          && TEST_ptr(ctx = BN_CTX_new())
          && TEST_true(BN_set_word(p, 0xA))
          && TEST_false(ossl_rsa_check_prime_factor_range(p, 8, ctx))
          && TEST_true(BN_set_word(p, 0x10))
          && TEST_false(ossl_rsa_check_prime_factor_range(p, 8, ctx))
          && TEST_true(BN_set_word(p, 0xB))
          && TEST_false(ossl_rsa_check_prime_factor_range(p, 8, ctx))
          && TEST_true(BN_set_word(p, 0xC))
          && TEST_true(ossl_rsa_check_prime_factor_range(p, 8, ctx))
          && TEST_true(BN_set_word(p, 0xF))
          && TEST_true(ossl_rsa_check_prime_factor_range(p, 8, ctx))
          && TEST_false(ossl_rsa_check_prime_factor_range(bn_p1, 72, ctx))
          && TEST_false(ossl_rsa_check_prime_factor_range(bn_p2, 72, ctx))
          && TEST_true(ossl_rsa_check_prime_factor_range(bn_p3, 72, ctx))
          && TEST_true(ossl_rsa_check_prime_factor_range(bn_p4, 72, ctx));

    BN_free(bn_p4);
    BN_free(bn_p3);
    BN_free(bn_p2);
    BN_free(bn_p1);
    BN_free(p);
    BN_CTX_free(ctx);
    return ret;
}

static int test_check_prime_factor(void)
{
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *e = NULL;
    BIGNUM *bn_p1 = NULL, *bn_p2 = NULL, *bn_p3 = NULL;

    /* Some range checks that are larger than 32 bits */
    static const unsigned char p1[] = { 0x0B, 0x50, 0x4f, 0x33, 0x73 };
    static const unsigned char p2[] = { 0x0B, 0x50, 0x4f, 0x33, 0x75 };
    static const unsigned char p3[] = { 0x0F, 0x50, 0x00, 0x03, 0x75 };

    ret = TEST_ptr(p = BN_new())
          && TEST_ptr(bn_p1 = bn_load_new(p1, sizeof(p1)))
          && TEST_ptr(bn_p2 = bn_load_new(p2, sizeof(p2)))
          && TEST_ptr(bn_p3 = bn_load_new(p3, sizeof(p3)))
          && TEST_ptr(e = BN_new())
          && TEST_ptr(ctx = BN_CTX_new())
          /* Fails the prime test */
          && TEST_true(BN_set_word(e, 0x1))
          && TEST_false(ossl_rsa_check_prime_factor(bn_p1, e, 72, ctx))
          /* p is prime and in range and gcd(p-1, e) = 1 */
          && TEST_true(ossl_rsa_check_prime_factor(bn_p2, e, 72, ctx))
          /* gcd(p-1,e) = 1 test fails */
          && TEST_true(BN_set_word(e, 0x2))
          && TEST_false(ossl_rsa_check_prime_factor(p, e, 72, ctx))
          /* p fails the range check */
          && TEST_true(BN_set_word(e, 0x1))
          && TEST_false(ossl_rsa_check_prime_factor(bn_p3, e, 72, ctx));

    BN_free(bn_p3);
    BN_free(bn_p2);
    BN_free(bn_p1);
    BN_free(e);
    BN_free(p);
    BN_CTX_free(ctx);
    return ret;
}

/* This test uses legacy functions because they can take invalid numbers */
static int test_check_private_exponent(void)
{
    int ret = 0;
    RSA *key = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *q = NULL, *e = NULL, *d = NULL, *n = NULL;

    ret = TEST_ptr(key = RSA_new())
          && TEST_ptr(ctx = BN_CTX_new())
          && TEST_ptr(p = BN_new())
          && TEST_ptr(q = BN_new())
          /* lcm(15-1,17-1) = 14*16 / 2 = 112 */
          && TEST_true(BN_set_word(p, 15))
          && TEST_true(BN_set_word(q, 17))
          && TEST_true(RSA_set0_factors(key, p, q));
    if (!ret) {
        BN_free(p);
        BN_free(q);
        goto end;
    }

    ret = TEST_ptr(e = BN_new())
          && TEST_ptr(d = BN_new())
          && TEST_ptr(n = BN_new())
          && TEST_true(BN_set_word(e, 5))
          && TEST_true(BN_set_word(d, 157))
          && TEST_true(BN_set_word(n, 15*17))
          && TEST_true(RSA_set0_key(key, n, e, d));
    if (!ret) {
        BN_free(e);
        BN_free(d);
        BN_free(n);
        goto end;
    }
    /* fails since d >= lcm(p-1, q-1) */
    ret = TEST_false(ossl_rsa_check_private_exponent(key, 8, ctx))
          && TEST_true(BN_set_word(d, 45))
          /* d is correct size and 1 = e.d mod lcm(p-1, q-1) */
          && TEST_true(ossl_rsa_check_private_exponent(key, 8, ctx))
          /* d is too small compared to nbits */
          && TEST_false(ossl_rsa_check_private_exponent(key, 16, ctx))
          /* d is too small compared to nbits */
          && TEST_true(BN_set_word(d, 16))
          && TEST_false(ossl_rsa_check_private_exponent(key, 8, ctx))
          /* fail if 1 != e.d mod lcm(p-1, q-1) */
          && TEST_true(BN_set_word(d, 46))
          && TEST_false(ossl_rsa_check_private_exponent(key, 8, ctx));
end:
    RSA_free(key);
    BN_CTX_free(ctx);
    return ret;
}

static int test_check_crt_components(void)
{
    const int P = 15;
    const int Q = 17;
    const int E = 5;
    const int N = P*Q;
    const int DP = 3;
    const int DQ = 13;
    const int QINV = 8;

    int ret = 0;
    RSA *key = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *q = NULL, *e = NULL;

    ret = TEST_ptr(key = RSA_new())
          && TEST_ptr(ctx = BN_CTX_new())
          && TEST_ptr(p = BN_new())
          && TEST_ptr(q = BN_new())
          && TEST_ptr(e = BN_new())
          && TEST_true(BN_set_word(p, P))
          && TEST_true(BN_set_word(q, Q))
          && TEST_true(BN_set_word(e, E))
          && TEST_true(RSA_set0_factors(key, p, q));
    if (!ret) {
        BN_free(p);
        BN_free(q);
        goto end;
    }

    ret = TEST_int_eq(ossl_rsa_sp800_56b_derive_params_from_pq(key, 8, e, ctx), 1)
          && TEST_BN_eq_word(key->n, N)
          && TEST_BN_eq_word(key->dmp1, DP)
          && TEST_BN_eq_word(key->dmq1, DQ)
          && TEST_BN_eq_word(key->iqmp, QINV)
          && TEST_true(ossl_rsa_check_crt_components(key, ctx))
          /* (a) 1 < dP < (p – 1). */
          && TEST_true(BN_set_word(key->dmp1, 1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmp1, P-1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmp1, DP))
          /* (b) 1 < dQ < (q - 1). */
          && TEST_true(BN_set_word(key->dmq1, 1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmq1, Q-1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmq1, DQ))
          /* (c) 1 < qInv < p */
          && TEST_true(BN_set_word(key->iqmp, 1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->iqmp, P))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->iqmp, QINV))
          /* (d) 1 = (dP . e) mod (p - 1)*/
          && TEST_true(BN_set_word(key->dmp1, DP+1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmp1, DP))
          /* (e) 1 = (dQ . e) mod (q - 1) */
          && TEST_true(BN_set_word(key->dmq1, DQ-1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->dmq1, DQ))
          /* (f) 1 = (qInv . q) mod p */
          && TEST_true(BN_set_word(key->iqmp, QINV+1))
          && TEST_false(ossl_rsa_check_crt_components(key, ctx))
          && TEST_true(BN_set_word(key->iqmp, QINV))
          /* check defaults are still valid */
          && TEST_true(ossl_rsa_check_crt_components(key, ctx));
end:
    BN_free(e);
    RSA_free(key);
    BN_CTX_free(ctx);
    return ret;
}

static const struct derive_from_pq_test {
    int p, q, e;
} derive_from_pq_tests[] = {
    { 15, 17, 6 }, /* Mod_inverse failure */
    { 0, 17, 5 }, /* d is too small */
};

static int test_derive_params_from_pq_fail(int tst)
{
    int ret = 0;
    RSA *key = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *q = NULL, *e = NULL;

    ret = TEST_ptr(key = RSA_new())
          && TEST_ptr(ctx = BN_CTX_new())
          && TEST_ptr(p = BN_new())
          && TEST_ptr(q = BN_new())
          && TEST_ptr(e = BN_new())
          && TEST_true(BN_set_word(p, derive_from_pq_tests[tst].p))
          && TEST_true(BN_set_word(q, derive_from_pq_tests[tst].q))
          && TEST_true(BN_set_word(e, derive_from_pq_tests[tst].e))
          && TEST_true(RSA_set0_factors(key, p, q));
    if (!ret) {
        BN_free(p);
        BN_free(q);
        goto end;
    }

    ret = TEST_int_le(ossl_rsa_sp800_56b_derive_params_from_pq(key, 8, e, ctx), 0);
end:
    BN_free(e);
    RSA_free(key);
    BN_CTX_free(ctx);
    return ret;
}

static int test_pq_diff(void)
{
    int ret = 0;
    BIGNUM *tmp = NULL, *p = NULL, *q = NULL;

    ret = TEST_ptr(tmp = BN_new())
          && TEST_ptr(p = BN_new())
          && TEST_ptr(q = BN_new())
          /* |1-(2+1)| > 2^1 */
          && TEST_true(BN_set_word(p, 1))
          && TEST_true(BN_set_word(q, 1+2))
          && TEST_false(ossl_rsa_check_pminusq_diff(tmp, p, q, 202))
          /* Check |p - q| > 2^(nbits/2 - 100) */
          && TEST_true(BN_set_word(q, 1+3))
          && TEST_true(ossl_rsa_check_pminusq_diff(tmp, p, q, 202))
          && TEST_true(BN_set_word(p, 1+3))
          && TEST_true(BN_set_word(q, 1))
          && TEST_true(ossl_rsa_check_pminusq_diff(tmp, p, q, 202));
    BN_free(p);
    BN_free(q);
    BN_free(tmp);
    return ret;
}

static int test_invalid_keypair(void)
{
    int ret = 0;
    RSA *key = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *q = NULL, *n = NULL, *e = NULL, *d = NULL;

    ret = TEST_ptr(key = RSA_new())
          && TEST_ptr(ctx = BN_CTX_new())
          /* NULL parameters */
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2048))
          /* load key */
          && TEST_ptr(p = bn_load_new(cav_p, sizeof(cav_p)))
          && TEST_ptr(q = bn_load_new(cav_q, sizeof(cav_q)))
          && TEST_true(RSA_set0_factors(key, p, q));
    if (!ret) {
        BN_free(p);
        BN_free(q);
        goto end;
    }

    ret = TEST_ptr(e = bn_load_new(cav_e, sizeof(cav_e)))
          && TEST_ptr(n = bn_load_new(cav_n, sizeof(cav_n)))
          && TEST_ptr(d = bn_load_new(cav_d, sizeof(cav_d)))
          && TEST_true(RSA_set0_key(key, n, e, d));
    if (!ret) {
        BN_free(e);
        BN_free(n);
        BN_free(d);
        goto end;
    }
          /* bad strength/key size */
    ret = TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, 100, 2048))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, 112, 1024))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, 128, 2048))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, 140, 3072))
          /* mismatching exponent */
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, BN_value_one(),
                                                         -1, 2048))
          /* bad exponent */
          && TEST_true(BN_add_word(e, 1))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2048))
          && TEST_true(BN_sub_word(e, 1))

          /* mismatch between bits and modulus */
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 3072))
          && TEST_true(ossl_rsa_sp800_56b_check_keypair(key, e, 112, 2048))
          /* check n == pq failure */
          && TEST_true(BN_add_word(n, 1))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2048))
          && TEST_true(BN_sub_word(n, 1))
          /* check that validation fails if len(n) is not even */
          && TEST_true(BN_lshift1(n, n))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2049))
          && TEST_true(BN_rshift1(n, n))
          /* check p  */
          && TEST_true(BN_sub_word(p, 2))
          && TEST_true(BN_mul(n, p, q, ctx))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2048))
          && TEST_true(BN_add_word(p, 2))
          && TEST_true(BN_mul(n, p, q, ctx))
          /* check q  */
          && TEST_true(BN_sub_word(q, 2))
          && TEST_true(BN_mul(n, p, q, ctx))
          && TEST_false(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, 2048))
          && TEST_true(BN_add_word(q, 2))
          && TEST_true(BN_mul(n, p, q, ctx));
end:
    RSA_free(key);
    BN_CTX_free(ctx);
    return ret;
}

static int keygen_size[] = {
    2048, 3072
};

static int test_sp80056b_keygen(int id)
{
    RSA *key = NULL;
    int ret;
    int sz = keygen_size[id];

    ret = TEST_ptr(key = RSA_new())
          && TEST_true(ossl_rsa_sp800_56b_generate_key(key, sz, NULL, NULL))
          && TEST_true(ossl_rsa_sp800_56b_check_public(key))
          && TEST_true(ossl_rsa_sp800_56b_check_private(key))
          && TEST_true(ossl_rsa_sp800_56b_check_keypair(key, NULL, -1, sz));

    RSA_free(key);
    return ret;
}

static int test_check_private_key(void)
{
    int ret = 0;
    BIGNUM *n = NULL, *d = NULL, *e = NULL;
    RSA *key = NULL;

    ret = TEST_ptr(key = RSA_new())
          /* check NULL pointers fail */
          && TEST_false(ossl_rsa_sp800_56b_check_private(key))
          /* load private key */
          && TEST_ptr(n = bn_load_new(cav_n, sizeof(cav_n)))
          && TEST_ptr(d = bn_load_new(cav_d, sizeof(cav_d)))
          && TEST_ptr(e = bn_load_new(cav_e, sizeof(cav_e)))
          && TEST_true(RSA_set0_key(key, n, e, d));
    if (!ret) {
        BN_free(n);
        BN_free(e);
        BN_free(d);
        goto end;
    }
    /* check d is in range */
    ret = TEST_true(ossl_rsa_sp800_56b_check_private(key))
          /* check d is too low */
          && TEST_true(BN_set_word(d, 0))
          && TEST_false(ossl_rsa_sp800_56b_check_private(key))
          /* check d is too high */
          && TEST_ptr(BN_copy(d, n))
          && TEST_false(ossl_rsa_sp800_56b_check_private(key));
end:
    RSA_free(key);
    return ret;
}

static int test_check_public_key(void)
{
    int ret = 0;
    BIGNUM *n = NULL, *e = NULL;
    RSA *key = NULL;

    ret = TEST_ptr(key = RSA_new())
          /* check NULL pointers fail */
          && TEST_false(ossl_rsa_sp800_56b_check_public(key))
          /* load public key */
          && TEST_ptr(e = bn_load_new(cav_e, sizeof(cav_e)))
          && TEST_ptr(n = bn_load_new(cav_n, sizeof(cav_n)))
          && TEST_true(RSA_set0_key(key, n, e, NULL));
    if (!ret) {
        BN_free(e);
        BN_free(n);
        goto end;
    }
    /* check public key is valid */
    ret = TEST_true(ossl_rsa_sp800_56b_check_public(key))
          /* check fail if n is even */
          && TEST_true(BN_add_word(n, 1))
          && TEST_false(ossl_rsa_sp800_56b_check_public(key))
          && TEST_true(BN_sub_word(n, 1))
          /* check fail if n is wrong number of bits */
          && TEST_true(BN_lshift1(n, n))
          && TEST_false(ossl_rsa_sp800_56b_check_public(key))
          && TEST_true(BN_rshift1(n, n))
          /* test odd exponent fails */
          && TEST_true(BN_add_word(e, 1))
          && TEST_false(ossl_rsa_sp800_56b_check_public(key))
          && TEST_true(BN_sub_word(e, 1))
          /* modulus fails composite check */
          && TEST_true(BN_add_word(n, 2))
          && TEST_false(ossl_rsa_sp800_56b_check_public(key));
end:
    RSA_free(key);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_check_public_exponent);
    ADD_TEST(test_check_prime_factor_range);
    ADD_TEST(test_check_prime_factor);
    ADD_TEST(test_check_private_exponent);
    ADD_TEST(test_check_crt_components);
    ADD_ALL_TESTS(test_derive_params_from_pq_fail, (int)OSSL_NELEM(derive_from_pq_tests));
    ADD_TEST(test_check_private_key);
    ADD_TEST(test_check_public_key);
    ADD_TEST(test_invalid_keypair);
    ADD_TEST(test_pq_diff);
    ADD_ALL_TESTS(test_sp80056b_keygen, (int)OSSL_NELEM(keygen_size));
    return 1;
}
