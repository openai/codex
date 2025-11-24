/*
 * Copyright 2017-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 BaishanCloud. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This aims to test the setting functions, including internal ones */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>

#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/bn.h>

#include "testutil.h"

#include <openssl/rsa.h>
#include "crypto/rsa.h"

#define NUM_EXTRA_PRIMES 1

DEFINE_STACK_OF(BIGNUM)

/* C90 requires string should <= 509 bytes */
static const unsigned char n[] =
    "\x92\x60\xd0\x75\x0a\xe1\x17\xee\xe5\x5c\x3f\x3d\xea\xba\x74\x91"
    "\x75\x21\xa2\x62\xee\x76\x00\x7c\xdf\x8a\x56\x75\x5a\xd7\x3a\x15"
    "\x98\xa1\x40\x84\x10\xa0\x14\x34\xc3\xf5\xbc\x54\xa8\x8b\x57\xfa"
    "\x19\xfc\x43\x28\xda\xea\x07\x50\xa4\xc4\x4e\x88\xcf\xf3\xb2\x38"
    "\x26\x21\xb8\x0f\x67\x04\x64\x43\x3e\x43\x36\xe6\xd0\x03\xe8\xcd"
    "\x65\xbf\xf2\x11\xda\x14\x4b\x88\x29\x1c\x22\x59\xa0\x0a\x72\xb7"
    "\x11\xc1\x16\xef\x76\x86\xe8\xfe\xe3\x4e\x4d\x93\x3c\x86\x81\x87"
    "\xbd\xc2\x6f\x7b\xe0\x71\x49\x3c\x86\xf7\xa5\x94\x1c\x35\x10\x80"
    "\x6a\xd6\x7b\x0f\x94\xd8\x8f\x5c\xf5\xc0\x2a\x09\x28\x21\xd8\x62"
    "\x6e\x89\x32\xb6\x5c\x5b\xd8\xc9\x20\x49\xc2\x10\x93\x2b\x7a\xfa"
    "\x7a\xc5\x9c\x0e\x88\x6a\xe5\xc1\xed\xb0\x0d\x8c\xe2\xc5\x76\x33"
    "\xdb\x26\xbd\x66\x39\xbf\xf7\x3c\xee\x82\xbe\x92\x75\xc4\x02\xb4"
    "\xcf\x2a\x43\x88\xda\x8c\xf8\xc6\x4e\xef\xe1\xc5\xa0\xf5\xab\x80"
    "\x57\xc3\x9f\xa5\xc0\x58\x9c\x3e\x25\x3f\x09\x60\x33\x23\x00\xf9"
    "\x4b\xea\x44\x87\x7b\x58\x8e\x1e\xdb\xde\x97\xcf\x23\x60\x72\x7a"
    "\x09\xb7\x75\x26\x2d\x7e\xe5\x52\xb3\x31\x9b\x92\x66\xf0\x5a\x25";

static const unsigned char e[] = "\x01\x00\x01";

static const unsigned char d[] =
    "\x6a\x7d\xf2\xca\x63\xea\xd4\xdd\xa1\x91\xd6\x14\xb6\xb3\x85\xe0"
    "\xd9\x05\x6a\x3d\x6d\x5c\xfe\x07\xdb\x1d\xaa\xbe\xe0\x22\xdb\x08"
    "\x21\x2d\x97\x61\x3d\x33\x28\xe0\x26\x7c\x9d\xd2\x3d\x78\x7a\xbd"
    "\xe2\xaf\xcb\x30\x6a\xeb\x7d\xfc\xe6\x92\x46\xcc\x73\xf5\xc8\x7f"
    "\xdf\x06\x03\x01\x79\xa2\x11\x4b\x76\x7d\xb1\xf0\x83\xff\x84\x1c"
    "\x02\x5d\x7d\xc0\x0c\xd8\x24\x35\xb9\xa9\x0f\x69\x53\x69\xe9\x4d"
    "\xf2\x3d\x2c\xe4\x58\xbc\x3b\x32\x83\xad\x8b\xba\x2b\x8f\xa1\xba"
    "\x62\xe2\xdc\xe9\xac\xcf\xf3\x79\x9a\xae\x7c\x84\x00\x16\xf3\xba"
    "\x8e\x00\x48\xc0\xb6\xcc\x43\x39\xaf\x71\x61\x00\x3a\x5b\xeb\x86"
    "\x4a\x01\x64\xb2\xc1\xc9\x23\x7b\x64\xbc\x87\x55\x69\x94\x35\x1b"
    "\x27\x50\x6c\x33\xd4\xbc\xdf\xce\x0f\x9c\x49\x1a\x7d\x6b\x06\x28"
    "\xc7\xc8\x52\xbe\x4f\x0a\x9c\x31\x32\xb2\xed\x3a\x2c\x88\x81\xe9"
    "\xaa\xb0\x7e\x20\xe1\x7d\xeb\x07\x46\x91\xbe\x67\x77\x76\xa7\x8b"
    "\x5c\x50\x2e\x05\xd9\xbd\xde\x72\x12\x6b\x37\x38\x69\x5e\x2d\xd1"
    "\xa0\xa9\x8a\x14\x24\x7c\x65\xd8\xa7\xee\x79\x43\x2a\x09\x2c\xb0"
    "\x72\x1a\x12\xdf\x79\x8e\x44\xf7\xcf\xce\x0c\x49\x81\x47\xa9\xb1";

static const unsigned char p[] =
    "\x06\x77\xcd\xd5\x46\x9b\xc1\xd5\x58\x00\x81\xe2\xf3\x0a\x36\xb1"
    "\x6e\x29\x89\xd5\x2f\x31\x5f\x92\x22\x3b\x9b\x75\x30\x82\xfa\xc5"
    "\xf5\xde\x8a\x36\xdb\xc6\xe5\x8f\xef\x14\x37\xd6\x00\xf9\xab\x90"
    "\x9b\x5d\x57\x4c\xf5\x1f\x77\xc4\xbb\x8b\xdd\x9b\x67\x11\x45\xb2"
    "\x64\xe8\xac\xa8\x03\x0f\x16\x0d\x5d\x2d\x53\x07\x23\xfb\x62\x0d"
    "\xe6\x16\xd3\x23\xe8\xb3";

static const unsigned char q[] =
    "\x06\x66\x9a\x70\x53\xd6\x72\x74\xfd\xea\x45\xc3\xc0\x17\xae\xde"
    "\x79\x17\xae\x79\xde\xfc\x0e\xf7\xa4\x3a\x8c\x43\x8f\xc7\x8a\xa2"
    "\x2c\x51\xc4\xd0\x72\x89\x73\x5c\x61\xbe\xfd\x54\x3f\x92\x65\xde"
    "\x4d\x65\x71\x70\xf6\xf2\xe5\x98\xb9\x0f\xd1\x0b\xe6\x95\x09\x4a"
    "\x7a\xdf\xf3\x10\x16\xd0\x60\xfc\xa5\x10\x34\x97\x37\x6f\x0a\xd5"
    "\x5d\x8f\xd4\xc3\xa0\x5b";

static const unsigned char dmp1[] =
    "\x05\x7c\x9e\x1c\xbd\x90\x25\xe7\x40\x86\xf5\xa8\x3b\x7a\x3f\x99"
    "\x56\x95\x60\x3a\x7b\x95\x4b\xb8\xa0\xd7\xa5\xf1\xcc\xdc\x5f\xb5"
    "\x8c\xf4\x62\x95\x54\xed\x2e\x12\x62\xc2\xe8\xf6\xde\xce\xed\x8e"
    "\x77\x6d\xc0\x40\x25\x74\xb3\x5a\x2d\xaa\xe1\xac\x11\xcb\xe2\x2f"
    "\x0a\x51\x23\x1e\x47\xb2\x05\x88\x02\xb2\x0f\x4b\xf0\x67\x30\xf0"
    "\x0f\x6e\xef\x5f\xf7\xe7";

static const unsigned char dmq1[] =
    "\x01\xa5\x6b\xbc\xcd\xe3\x0e\x46\xc6\x72\xf5\x04\x56\x28\x01\x22"
    "\x58\x74\x5d\xbc\x1c\x3c\x29\x41\x49\x6c\x81\x5c\x72\xe2\xf7\xe5"
    "\xa3\x8e\x58\x16\xe0\x0e\x37\xac\x1f\xbb\x75\xfd\xaf\xe7\xdf\xe9"
    "\x1f\x70\xa2\x8f\x52\x03\xc0\x46\xd9\xf9\x96\x63\x00\x27\x7e\x5f"
    "\x38\x60\xd6\x6b\x61\xe2\xaf\xbe\xea\x58\xd3\x9d\xbc\x75\x03\x8d"
    "\x42\x65\xd6\x6b\x85\x97";

static const unsigned char iqmp[] =
    "\x03\xa1\x8b\x80\xe4\xd8\x87\x25\x17\x5d\xcc\x8d\xa9\x8a\x22\x2b"
    "\x6c\x15\x34\x6f\x80\xcc\x1c\x44\x04\x68\xbc\x03\xcd\x95\xbb\x69"
    "\x37\x61\x48\xb4\x23\x13\x08\x16\x54\x6a\xa1\x7c\xf5\xd4\x3a\xe1"
    "\x4f\xa4\x0c\xf5\xaf\x80\x85\x27\x06\x0d\x70\xc0\xc5\x19\x28\xfe"
    "\xee\x8e\x86\x21\x98\x8a\x37\xb7\xe5\x30\x25\x70\x93\x51\x2d\x49"
    "\x85\x56\xb3\x0c\x2b\x96";

static const unsigned char ex_prime[] =
    "\x03\x89\x22\xa0\xb7\x3a\x91\xcb\x5e\x0c\xfd\x73\xde\xa7\x38\xa9"
    "\x47\x43\xd6\x02\xbf\x2a\xb9\x3c\x48\xf3\x06\xd6\x58\x35\x50\x56"
    "\x16\x5c\x34\x9b\x61\x87\xc8\xaa\x0a\x5d\x8a\x0a\xcd\x9c\x41\xd9"
    "\x96\x24\xe0\xa9\x9b\x26\xb7\xa8\x08\xc9\xea\xdc\xa7\x15\xfb\x62"
    "\xa0\x2d\x90\xe6\xa7\x55\x6e\xc6\x6c\xff\xd6\x10\x6d\xfa\x2e\x04"
    "\x50\xec\x5c\x66\xe4\x05";

static const unsigned char ex_exponent[] =
    "\x02\x0a\xcd\xc3\x82\xd2\x03\xb0\x31\xac\xd3\x20\x80\x34\x9a\x57"
    "\xbc\x60\x04\x57\x25\xd0\x29\x9a\x16\x90\xb9\x1c\x49\x6a\xd1\xf2"
    "\x47\x8c\x0e\x9e\xc9\x20\xc2\xd8\xe4\x8f\xce\xd2\x1a\x9c\xec\xb4"
    "\x1f\x33\x41\xc8\xf5\x62\xd1\xa5\xef\x1d\xa1\xd8\xbd\x71\xc6\xf7"
    "\xda\x89\x37\x2e\xe2\xec\x47\xc5\xb8\xe3\xb4\xe3\x5c\x82\xaa\xdd"
    "\xb7\x58\x2e\xaf\x07\x79";

static const unsigned char ex_coefficient[] =
    "\x00\x9c\x09\x88\x9b\xc8\x57\x08\x69\x69\xab\x2d\x9e\x29\x1c\x3c"
    "\x6d\x59\x33\x12\x0d\x2b\x09\x2e\xaf\x01\x2c\x27\x01\xfc\xbd\x26"
    "\x13\xf9\x2d\x09\x22\x4e\x49\x11\x03\x82\x88\x87\xf4\x43\x1d\xac"
    "\xca\xec\x86\xf7\x23\xf1\x64\xf3\xf5\x81\xf0\x37\x36\xcf\x67\xff"
    "\x1a\xff\x7a\xc7\xf9\xf9\x67\x2d\xa0\x9d\x61\xf8\xf6\x47\x5c\x2f"
    "\xe7\x66\xe8\x3c\x3a\xe8";

static int key2048_key(RSA *key)
{
    if (!TEST_int_eq(RSA_set0_key(key,
                                  BN_bin2bn(n, sizeof(n) - 1, NULL),
                                  BN_bin2bn(e, sizeof(e) - 1, NULL),
                                  BN_bin2bn(d, sizeof(d) - 1, NULL)), 1))
        return 0;

    return RSA_size(key);
}

static int key2048p3_v1(RSA *key)
{
    BIGNUM **pris = NULL, **exps = NULL, **coeffs = NULL;
    int rv = RSA_size(key);


    if (!TEST_int_eq(RSA_set0_factors(key,
                                      BN_bin2bn(p, sizeof(p) - 1, NULL),
                                      BN_bin2bn(q, sizeof(q) - 1, NULL)), 1))
        goto err;

    if (!TEST_int_eq(RSA_set0_crt_params(key,
                                         BN_bin2bn(dmp1, sizeof(dmp1) - 1, NULL),
                                         BN_bin2bn(dmq1, sizeof(dmq1) - 1, NULL),
                                         BN_bin2bn(iqmp, sizeof(iqmp) - 1,
                                                   NULL)), 1))
        return 0;

    pris = OPENSSL_zalloc(sizeof(BIGNUM *));
    exps = OPENSSL_zalloc(sizeof(BIGNUM *));
    coeffs = OPENSSL_zalloc(sizeof(BIGNUM *));
    if (!TEST_ptr(pris) || !TEST_ptr(exps) || !TEST_ptr(coeffs))
        goto err;

    pris[0] = BN_bin2bn(ex_prime, sizeof(ex_prime) - 1, NULL);
    exps[0] = BN_bin2bn(ex_exponent, sizeof(ex_exponent) - 1, NULL);
    coeffs[0] = BN_bin2bn(ex_coefficient, sizeof(ex_coefficient) - 1, NULL);
    if (!TEST_ptr(pris[0]) || !TEST_ptr(exps[0]) || !TEST_ptr(coeffs[0]))
        goto err;

    if (!TEST_true(RSA_set0_multi_prime_params(key, pris, exps,
                                               coeffs, NUM_EXTRA_PRIMES)))
        goto err;

 ret:
    OPENSSL_free(pris);
    OPENSSL_free(exps);
    OPENSSL_free(coeffs);
    return rv;
 err:
    if (pris != NULL)
        BN_free(pris[0]);
    if (exps != NULL)
        BN_free(exps[0]);
    if (coeffs != NULL)
        BN_free(coeffs[0]);
    rv = 0;
    goto ret;
}

static int key2048p3_v2(RSA *key)
{
    STACK_OF(BIGNUM) *primes = NULL, *exps = NULL, *coeffs = NULL;
    BIGNUM *num = NULL;
    int rv = RSA_size(key);

    if (!TEST_ptr(primes = sk_BIGNUM_new_null())
        || !TEST_ptr(exps = sk_BIGNUM_new_null())
        || !TEST_ptr(coeffs = sk_BIGNUM_new_null()))
        goto err;

    if (!TEST_ptr(num = BN_bin2bn(p, sizeof(p) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(primes, num), 0)
        || !TEST_ptr(num = BN_bin2bn(q, sizeof(q) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(primes, num), 0)
        || !TEST_ptr(num = BN_bin2bn(ex_prime, sizeof(ex_prime) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(primes, num), 0))
        goto err;

    if (!TEST_ptr(num = BN_bin2bn(dmp1, sizeof(dmp1) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(exps, num), 0)
        || !TEST_ptr(num = BN_bin2bn(dmq1, sizeof(dmq1) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(exps, num), 0)
        || !TEST_ptr(num = BN_bin2bn(ex_exponent, sizeof(ex_exponent) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(exps, num), 0))
        goto err;

    if (!TEST_ptr(num = BN_bin2bn(iqmp, sizeof(iqmp) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(coeffs, num), 0)
        || !TEST_ptr(num = BN_bin2bn(ex_coefficient, sizeof(ex_coefficient) - 1, NULL))
        || !TEST_int_ne(sk_BIGNUM_push(coeffs, num), 0))
        goto err;

    if (!TEST_true(ossl_rsa_set0_all_params(key, primes, exps, coeffs)))
        goto err;

 ret:
    sk_BIGNUM_free(primes);
    sk_BIGNUM_free(exps);
    sk_BIGNUM_free(coeffs);
    return rv;
 err:
    sk_BIGNUM_pop_free(primes, BN_free);
    sk_BIGNUM_pop_free(exps, BN_free);
    sk_BIGNUM_pop_free(coeffs, BN_free);
    primes = exps = coeffs = NULL;
    rv = 0;
    goto ret;
}

static int test_rsa_mp(int i)
{
    int ret = 0;
    RSA *key;
    unsigned char ptext[256];
    unsigned char ctext[256];
    static unsigned char ptext_ex[] = "\x54\x85\x9b\x34\x2c\x49\xea\x2a";
    int plen;
    int clen = 0;
    int num;
    static int (*param_set[])(RSA *) = {
        key2048p3_v1,
        key2048p3_v2,
    };

    plen = sizeof(ptext_ex) - 1;
    key = RSA_new();
    if (!TEST_ptr(key))
        goto err;

    if (!TEST_int_eq((clen = key2048_key(key)), 256)
        || !TEST_int_eq((clen = param_set[i](key)), 256))
        goto err;

    if (!TEST_true(RSA_check_key_ex(key, NULL)))
        goto err;

    num = RSA_public_encrypt(plen, ptext_ex, ctext, key,
                             RSA_PKCS1_PADDING);
    if (!TEST_int_eq(num, clen))
        goto err;

    num = RSA_private_decrypt(num, ctext, ptext, key, RSA_PKCS1_PADDING);
    if (!TEST_mem_eq(ptext, num, ptext_ex, plen))
        goto err;

    ret = 1;
err:
    RSA_free(key);
    return ret;
}

static int test_rsa_mp_gen_bad_input(void)
{
    int ret = 0;
    RSA *rsa = NULL;
    BIGNUM *ebn = NULL;

    if (!TEST_ptr(rsa = RSA_new()))
        goto err;

    if (!TEST_ptr(ebn = BN_new()))
        goto err;
    if (!TEST_true(BN_set_word(ebn, 65537)))
        goto err;

    /* Test that a NULL exponent fails and does not segfault */
    if (!TEST_int_eq(RSA_generate_multi_prime_key(rsa, 1024, 2, NULL, NULL), 0))
        goto err;

    /* Test invalid bitsize fails */
    if (!TEST_int_eq(RSA_generate_multi_prime_key(rsa, 500, 2, ebn, NULL), 0))
        goto err;

    /* Test invalid prime count fails */
    if (!TEST_int_eq(RSA_generate_multi_prime_key(rsa, 1024, 1, ebn, NULL), 0))
        goto err;
    ret = 1;
err:
    BN_free(ebn);
    RSA_free(rsa);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_rsa_mp_gen_bad_input);
    ADD_ALL_TESTS(test_rsa_mp, 2);
    return 1;
}
