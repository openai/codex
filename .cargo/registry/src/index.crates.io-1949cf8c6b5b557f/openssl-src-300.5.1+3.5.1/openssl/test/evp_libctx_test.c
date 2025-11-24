/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*

 * These tests are setup to load null into the default library context.
 * Any tests are expected to use the created 'libctx' to find algorithms.
 * The framework runs the tests twice using the 'default' provider or
 * 'fips' provider as inputs.
 */

/*
 * DSA/DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"
#include <assert.h>
#include <string.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include <openssl/dsa.h>
#include <openssl/dh.h>
#include <openssl/safestack.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/x509.h>
#include <openssl/encoder.h>
#include "testutil.h"
#include "internal/nelem.h"
#include "crypto/bn_dh.h"   /* _bignum_ffdhe2048_p */

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *nullprov = NULL;
static OSSL_PROVIDER *libprov = NULL;
static STACK_OF(OPENSSL_STRING) *cipher_names = NULL;
static int is_fips = 0;
static int is_fips_lt_3_5 = 0;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_PROVIDER_NAME,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { "provider", OPT_PROVIDER_NAME, 's',
          "The provider to load (The default value is 'default')" },
        { NULL }
    };
    return test_options;
}

#ifndef OPENSSL_NO_DH
static const char *getname(int id)
{
    const char *name[] = {"p", "q", "g" };

    if (id >= 0 && id < 3)
        return name[id];
    return "?";
}
#endif

static int test_evp_cipher_api_safety(void)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx = NULL;

    ctx = EVP_CIPHER_CTX_new();

    if (!TEST_ptr(ctx))
        goto err;

    /*
     * Ensure that EVP_CIPHER_get_block_size returns 0
     * if we haven't initialized the cipher in this context
     */
    if (!TEST_int_eq(EVP_CIPHER_CTX_get_block_size(ctx), 0))
        goto err_free;

    /*
     * Ensure that EVP_CIPHER_get_iv_length returns 0
     * if we haven't initialized the cipher in this context
     */
    if (!TEST_int_eq(EVP_CIPHER_CTX_get_iv_length(ctx), 0))
        goto err_free;

    ret = 1;
err_free:
    EVP_CIPHER_CTX_free(ctx);
err:
    return ret;
}

/*
 * We're using some DH specific values in this test, so we skip compilation if
 * we're in a no-dh build.
 */
#if !defined(OPENSSL_NO_DSA) && !defined(OPENSSL_NO_DH)

static int test_dsa_param_keygen(int tstid)
{
    int ret = 0;
    int expected;
    EVP_PKEY_CTX *gen_ctx = NULL;
    EVP_PKEY *pkey_parm = NULL;
    EVP_PKEY *pkey = NULL, *dup_pk = NULL;
    DSA *dsa = NULL;
    int pind, qind, gind;
    BIGNUM *p = NULL, *q = NULL, *g = NULL;

    /*
     * Just grab some fixed dh p, q, g values for testing,
     * these 'safe primes' should not be used normally for dsa *.
     */
    static const BIGNUM *bn[] = {
        &ossl_bignum_dh2048_256_p, &ossl_bignum_dh2048_256_q,
        &ossl_bignum_dh2048_256_g
    };

    /*
     * These tests are using bad values for p, q, g by reusing the values.
     * A value of 0 uses p, 1 uses q and 2 uses g.
     * There are 27 different combinations, with only the 1 valid combination.
     */
    pind = tstid / 9;
    qind = (tstid / 3) % 3;
    gind = tstid % 3;
    expected  = (pind == 0 && qind == 1 && gind == 2);

    TEST_note("Testing with (p, q, g) = (%s, %s, %s)\n", getname(pind),
              getname(qind), getname(gind));

    if (!TEST_ptr(pkey_parm = EVP_PKEY_new())
        || !TEST_ptr(dsa = DSA_new())
        || !TEST_ptr(p = BN_dup(bn[pind]))
        || !TEST_ptr(q = BN_dup(bn[qind]))
        || !TEST_ptr(g = BN_dup(bn[gind]))
        || !TEST_true(DSA_set0_pqg(dsa, p, q, g)))
        goto err;
    p = q = g = NULL;

    if (!TEST_true(EVP_PKEY_assign_DSA(pkey_parm, dsa)))
        goto err;
    dsa = NULL;

    if (!TEST_ptr(gen_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey_parm, NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(gen_ctx), 0)
        || !TEST_int_eq(EVP_PKEY_keygen(gen_ctx, &pkey), expected))
        goto err;

    if (expected) {
        if (!TEST_ptr(dup_pk = EVP_PKEY_dup(pkey))
            || !TEST_int_eq(EVP_PKEY_eq(pkey, dup_pk), 1))
            goto err;
    }

    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(dup_pk);
    EVP_PKEY_CTX_free(gen_ctx);
    EVP_PKEY_free(pkey_parm);
    DSA_free(dsa);
    BN_free(g);
    BN_free(q);
    BN_free(p);
    return ret;
}
#endif /* OPENSSL_NO_DSA */

#ifndef OPENSSL_NO_DH
static int do_dh_param_keygen(int tstid, const BIGNUM **bn)
{
    int ret = 0;
    int expected;
    EVP_PKEY_CTX *gen_ctx = NULL;
    EVP_PKEY *pkey_parm = NULL;
    EVP_PKEY *pkey = NULL, *dup_pk = NULL;
    DH *dh = NULL;
    int pind, qind, gind;
    BIGNUM *p = NULL, *q = NULL, *g = NULL;

    /*
     * These tests are using bad values for p, q, g by reusing the values.
     * A value of 0 uses p, 1 uses q and 2 uses g.
     * There are 27 different combinations, with only the 1 valid combination.
     */
    pind = tstid / 9;
    qind = (tstid / 3) % 3;
    gind = tstid % 3;
    expected  = (pind == 0 && qind == 1 && gind == 2);

    TEST_note("Testing with (p, q, g) = (%s, %s, %s)", getname(pind),
              getname(qind), getname(gind));

    if (!TEST_ptr(pkey_parm = EVP_PKEY_new())
        || !TEST_ptr(dh = DH_new())
        || !TEST_ptr(p = BN_dup(bn[pind]))
        || !TEST_ptr(q = BN_dup(bn[qind]))
        || !TEST_ptr(g = BN_dup(bn[gind]))
        || !TEST_true(DH_set0_pqg(dh, p, q, g)))
        goto err;
    p = q = g = NULL;

    if (!TEST_true(EVP_PKEY_assign_DH(pkey_parm, dh)))
        goto err;
    dh = NULL;

    if (!TEST_ptr(gen_ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey_parm, NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(gen_ctx), 0)
        || !TEST_int_eq(EVP_PKEY_keygen(gen_ctx, &pkey), expected))
        goto err;

    if (expected) {
        if (!TEST_ptr(dup_pk = EVP_PKEY_dup(pkey))
            || !TEST_int_eq(EVP_PKEY_eq(pkey, dup_pk), 1))
            goto err;
    }

    ret = 1;
err:
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(dup_pk);
    EVP_PKEY_CTX_free(gen_ctx);
    EVP_PKEY_free(pkey_parm);
    DH_free(dh);
    BN_free(g);
    BN_free(q);
    BN_free(p);
    return ret;
}

/*
 * Note that we get the fips186-4 path being run for most of these cases since
 * the internal code will detect that the p, q, g does not match a safe prime
 * group (Except for when tstid = 5, which sets the correct p, q, g)
 */
static int test_dh_safeprime_param_keygen(int tstid)
{
    static const BIGNUM *bn[] = {
        &ossl_bignum_ffdhe2048_p,  &ossl_bignum_ffdhe2048_q,
        &ossl_bignum_const_2
    };
    return do_dh_param_keygen(tstid, bn);
}

static int dhx_cert_load(void)
{
    int ret = 0;
    X509 *cert = NULL;
    BIO *bio = NULL;

    static const unsigned char dhx_cert[] = {
        0x30,0x82,0x03,0xff,0x30,0x82,0x02,0xe7,0xa0,0x03,0x02,0x01,0x02,0x02,0x09,0x00,
        0xdb,0xf5,0x4d,0x22,0xa0,0x7a,0x67,0xa6,0x30,0x0d,0x06,0x09,0x2a,0x86,0x48,0x86,
        0xf7,0x0d,0x01,0x01,0x05,0x05,0x00,0x30,0x44,0x31,0x0b,0x30,0x09,0x06,0x03,0x55,
        0x04,0x06,0x13,0x02,0x55,0x4b,0x31,0x16,0x30,0x14,0x06,0x03,0x55,0x04,0x0a,0x0c,
        0x0d,0x4f,0x70,0x65,0x6e,0x53,0x53,0x4c,0x20,0x47,0x72,0x6f,0x75,0x70,0x31,0x1d,
        0x30,0x1b,0x06,0x03,0x55,0x04,0x03,0x0c,0x14,0x54,0x65,0x73,0x74,0x20,0x53,0x2f,
        0x4d,0x49,0x4d,0x45,0x20,0x52,0x53,0x41,0x20,0x52,0x6f,0x6f,0x74,0x30,0x1e,0x17,
        0x0d,0x31,0x33,0x30,0x38,0x30,0x32,0x31,0x34,0x34,0x39,0x32,0x39,0x5a,0x17,0x0d,
        0x32,0x33,0x30,0x36,0x31,0x31,0x31,0x34,0x34,0x39,0x32,0x39,0x5a,0x30,0x44,0x31,
        0x0b,0x30,0x09,0x06,0x03,0x55,0x04,0x06,0x13,0x02,0x55,0x4b,0x31,0x16,0x30,0x14,
        0x06,0x03,0x55,0x04,0x0a,0x0c,0x0d,0x4f,0x70,0x65,0x6e,0x53,0x53,0x4c,0x20,0x47,
        0x72,0x6f,0x75,0x70,0x31,0x1d,0x30,0x1b,0x06,0x03,0x55,0x04,0x03,0x0c,0x14,0x54,
        0x65,0x73,0x74,0x20,0x53,0x2f,0x4d,0x49,0x4d,0x45,0x20,0x45,0x45,0x20,0x44,0x48,
        0x20,0x23,0x31,0x30,0x82,0x01,0xb6,0x30,0x82,0x01,0x2b,0x06,0x07,0x2a,0x86,0x48,
        0xce,0x3e,0x02,0x01,0x30,0x82,0x01,0x1e,0x02,0x81,0x81,0x00,0xd4,0x0c,0x4a,0x0c,
        0x04,0x72,0x71,0x19,0xdf,0x59,0x19,0xc5,0xaf,0x44,0x7f,0xca,0x8e,0x2b,0xf0,0x09,
        0xf5,0xd3,0x25,0xb1,0x73,0x16,0x55,0x89,0xdf,0xfd,0x07,0xaf,0x19,0xd3,0x7f,0xd0,
        0x07,0xa2,0xfe,0x3f,0x5a,0xf1,0x01,0xc6,0xf8,0x2b,0xef,0x4e,0x6d,0x03,0x38,0x42,
        0xa1,0x37,0xd4,0x14,0xb4,0x00,0x4a,0xb1,0x86,0x5a,0x83,0xce,0xb9,0x08,0x0e,0xc1,
        0x99,0x27,0x47,0x8d,0x0b,0x85,0xa8,0x82,0xed,0xcc,0x0d,0xb9,0xb0,0x32,0x7e,0xdf,
        0xe8,0xe4,0xf6,0xf6,0xec,0xb3,0xee,0x7a,0x11,0x34,0x65,0x97,0xfc,0x1a,0xb0,0x95,
        0x4b,0x19,0xb9,0xa6,0x1c,0xd9,0x01,0x32,0xf7,0x35,0x7c,0x2d,0x5d,0xfe,0xc1,0x85,
        0x70,0x49,0xf8,0xcc,0x99,0xd0,0xbe,0xf1,0x5a,0x78,0xc8,0x03,0x02,0x81,0x80,0x69,
        0x00,0xfd,0x66,0xf2,0xfc,0x15,0x8b,0x09,0xb8,0xdc,0x4d,0xea,0xaa,0x79,0x55,0xf9,
        0xdf,0x46,0xa6,0x2f,0xca,0x2d,0x8f,0x59,0x2a,0xad,0x44,0xa3,0xc6,0x18,0x2f,0x95,
        0xb6,0x16,0x20,0xe3,0xd3,0xd1,0x8f,0x03,0xce,0x71,0x7c,0xef,0x3a,0xc7,0x44,0x39,
        0x0e,0xe2,0x1f,0xd8,0xd3,0x89,0x2b,0xe7,0x51,0xdc,0x12,0x48,0x4c,0x18,0x4d,0x99,
        0x12,0x06,0xe4,0x17,0x02,0x03,0x8c,0x24,0x05,0x8e,0xa6,0x85,0xf2,0x69,0x1b,0xe1,
        0x6a,0xdc,0xe2,0x04,0x3a,0x01,0x9d,0x64,0xbe,0xfe,0x45,0xf9,0x44,0x18,0x71,0xbd,
        0x2d,0x3e,0x7a,0x6f,0x72,0x7d,0x1a,0x80,0x42,0x57,0xae,0x18,0x6f,0x91,0xd6,0x61,
        0x03,0x8a,0x1c,0x89,0x73,0xc7,0x56,0x41,0x03,0xd3,0xf8,0xed,0x65,0xe2,0x85,0x02,
        0x15,0x00,0x89,0x94,0xab,0x10,0x67,0x45,0x41,0xad,0x63,0xc6,0x71,0x40,0x8d,0x6b,
        0x9e,0x19,0x5b,0xa4,0xc7,0xf5,0x03,0x81,0x84,0x00,0x02,0x81,0x80,0x2f,0x5b,0xde,
        0x72,0x02,0x36,0x6b,0x00,0x5e,0x24,0x7f,0x14,0x2c,0x18,0x52,0x42,0x97,0x4b,0xdb,
        0x6e,0x15,0x50,0x3c,0x45,0x3e,0x25,0xf3,0xb7,0xc5,0x6e,0xe5,0x52,0xe7,0xc4,0xfb,
        0xf4,0xa5,0xf0,0x39,0x12,0x7f,0xbc,0x54,0x1c,0x93,0xb9,0x5e,0xee,0xe9,0x14,0xb0,
        0xdf,0xfe,0xfc,0x36,0xe4,0xf2,0xaf,0xfb,0x13,0xc8,0xdf,0x18,0x94,0x1d,0x40,0xb9,
        0x71,0xdd,0x4c,0x9c,0xa7,0x03,0x52,0x02,0xb5,0xed,0x71,0x80,0x3e,0x23,0xda,0x28,
        0xe5,0xab,0xe7,0x6f,0xf2,0x0a,0x0e,0x00,0x5b,0x7d,0xc6,0x4b,0xd7,0xc7,0xb2,0xc3,
        0xba,0x62,0x7f,0x70,0x28,0xa0,0x9d,0x71,0x13,0x70,0xd1,0x9f,0x32,0x2f,0x3e,0xd2,
        0xcd,0x1b,0xa4,0xc6,0x72,0xa0,0x74,0x5d,0x71,0xef,0x03,0x43,0x6e,0xa3,0x60,0x30,
        0x5e,0x30,0x0c,0x06,0x03,0x55,0x1d,0x13,0x01,0x01,0xff,0x04,0x02,0x30,0x00,0x30,
        0x0e,0x06,0x03,0x55,0x1d,0x0f,0x01,0x01,0xff,0x04,0x04,0x03,0x02,0x05,0xe0,0x30,
        0x1d,0x06,0x03,0x55,0x1d,0x0e,0x04,0x16,0x04,0x14,0x0b,0x5a,0x4d,0x5f,0x7d,0x25,
        0xc7,0xf2,0x9d,0xc1,0xaa,0xb7,0x63,0x82,0x2f,0xfa,0x8f,0x32,0xe7,0xc0,0x30,0x1f,
        0x06,0x03,0x55,0x1d,0x23,0x04,0x18,0x30,0x16,0x80,0x14,0xdf,0x7e,0x5e,0x88,0x05,
        0x24,0x33,0x08,0xdd,0x22,0x81,0x02,0x97,0xcc,0x9a,0xb7,0xb1,0x33,0x27,0x30,0x30,
        0x0d,0x06,0x09,0x2a,0x86,0x48,0x86,0xf7,0x0d,0x01,0x01,0x05,0x05,0x00,0x03,0x82,
        0x01,0x01,0x00,0x5a,0xf2,0x63,0xef,0xd3,0x16,0xd7,0xf5,0xaa,0xdd,0x12,0x00,0x36,
        0x00,0x21,0xa2,0x7b,0x08,0xd6,0x3b,0x9f,0x62,0xac,0x53,0x1f,0xed,0x4c,0xd1,0x15,
        0x34,0x65,0x71,0xee,0x96,0x07,0xa6,0xef,0xb2,0xde,0xd8,0xbb,0x35,0x6e,0x2c,0xe2,
        0xd1,0x26,0xef,0x7e,0x94,0xe2,0x88,0x51,0xa4,0x6c,0xaa,0x27,0x2a,0xd3,0xb6,0xc2,
        0xf7,0xea,0xc3,0x0b,0xa9,0xb5,0x28,0x37,0xa2,0x63,0x08,0xe4,0x88,0xc0,0x1b,0x16,
        0x1b,0xca,0xfd,0x8a,0x07,0x32,0x29,0xa7,0x53,0xb5,0x2d,0x30,0xe4,0xf5,0x16,0xc3,
        0xe3,0xc2,0x4c,0x30,0x5d,0x35,0x80,0x1c,0xa2,0xdb,0xe3,0x4b,0x51,0x0d,0x4c,0x60,
        0x5f,0xb9,0x46,0xac,0xa8,0x46,0xa7,0x32,0xa7,0x9c,0x76,0xf8,0xe9,0xb5,0x19,0xe2,
        0x0c,0xe1,0x0f,0xc6,0x46,0xe2,0x38,0xa7,0x87,0x72,0x6d,0x6c,0xbc,0x88,0x2f,0x9d,
        0x2d,0xe5,0xd0,0x7d,0x1e,0xc7,0x5d,0xf8,0x7e,0xb4,0x0b,0xa6,0xf9,0x6c,0xe3,0x7c,
        0xb2,0x70,0x6e,0x75,0x9b,0x1e,0x63,0xe1,0x4d,0xb2,0x81,0xd3,0x55,0x38,0x94,0x1a,
        0x7a,0xfa,0xbf,0x01,0x18,0x70,0x2d,0x35,0xd3,0xe3,0x10,0x7a,0x9a,0xa7,0x8f,0xf3,
        0xbd,0x56,0x55,0x5e,0xd8,0xbd,0x4e,0x16,0x76,0xd0,0x48,0x4c,0xf9,0x51,0x54,0xdf,
        0x2d,0xb0,0xc9,0xaa,0x5e,0x42,0x38,0x50,0xbf,0x0f,0xc0,0xd9,0x84,0x44,0x4b,0x42,
        0x24,0xec,0x14,0xa3,0xde,0x11,0xdf,0x58,0x7f,0xc2,0x4d,0xb2,0xd5,0x42,0x78,0x6e,
        0x52,0x3e,0xad,0xc3,0x5f,0x04,0xc4,0xe6,0x31,0xaa,0x81,0x06,0x8b,0x13,0x4b,0x3c,
        0x0e,0x6a,0xb1
    };

    if (!TEST_ptr(bio = BIO_new_mem_buf(dhx_cert, sizeof(dhx_cert)))
        || !TEST_ptr(cert = X509_new_ex(libctx, NULL))
        || !TEST_ptr(d2i_X509_bio(bio, &cert)))
        goto err;
    ret = 1;
err:
    X509_free(cert);
    BIO_free(bio);
    return ret;
}

#endif /* OPENSSL_NO_DH */

static int test_cipher_reinit(int test_id)
{
    int ret = 0, diff, ccm, siv, no_null_key;
    int out1_len = 0, out2_len = 0, out3_len = 0;
    EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *ctx = NULL;
    unsigned char out1[256];
    unsigned char out2[256];
    unsigned char out3[256];
    unsigned char in[16] = {
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10
    };
    unsigned char key[64] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x03, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    };
    unsigned char iv[48] = {
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00
    };
    const char *name = sk_OPENSSL_STRING_value(cipher_names, test_id);

    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new()))
        goto err;

    TEST_note("Fetching %s\n", name);
    if (!TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, name, NULL)))
        goto err;

    /* ccm fails on the second update - this matches OpenSSL 1_1_1 behaviour */
    ccm = (EVP_CIPHER_get_mode(cipher) == EVP_CIPH_CCM_MODE);

    /* siv cannot be called with NULL key as the iv is irrelevant */
    siv = (EVP_CIPHER_get_mode(cipher) == EVP_CIPH_SIV_MODE);

    /*
     * Skip init call with a null key for RC4 as the stream cipher does not
     * handle reinit (1.1.1 behaviour).
     */
    no_null_key = EVP_CIPHER_is_a(cipher, "RC4")
                  || EVP_CIPHER_is_a(cipher, "RC4-40")
                  || EVP_CIPHER_is_a(cipher, "RC4-HMAC-MD5");

    /* DES3-WRAP uses random every update - so it will give a different value */
    diff = EVP_CIPHER_is_a(cipher, "DES3-WRAP");
    if (!TEST_true(EVP_EncryptInit_ex(ctx, cipher, NULL, key, iv))
        || !TEST_true(EVP_EncryptUpdate(ctx, out1, &out1_len, in, sizeof(in)))
        || !TEST_true(EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv))
        || !TEST_int_eq(EVP_EncryptUpdate(ctx, out2, &out2_len, in, sizeof(in)),
                        ccm ? 0 : 1)
        || (!no_null_key
        && (!TEST_true(EVP_EncryptInit_ex(ctx, NULL, NULL, NULL, iv))
        || !TEST_int_eq(EVP_EncryptUpdate(ctx, out3, &out3_len, in, sizeof(in)),
                        ccm || siv ? 0 : 1))))
        goto err;

    if (ccm == 0) {
        if (diff) {
            if (!TEST_mem_ne(out1, out1_len, out2, out2_len)
                || !TEST_mem_ne(out1, out1_len, out3, out3_len)
                || !TEST_mem_ne(out2, out2_len, out3, out3_len))
                goto err;
        } else {
            if (!TEST_mem_eq(out1, out1_len, out2, out2_len)
                || (!siv && !no_null_key && !TEST_mem_eq(out1, out1_len, out3, out3_len)))
                goto err;
        }
    }
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

/*
 * This test only uses a partial block (half the block size) of input for each
 * EVP_EncryptUpdate() in order to test that the second init/update is not using
 * a leftover buffer from the first init/update.
 * Note: some ciphers don't need a full block to produce output.
 */
static int test_cipher_reinit_partialupdate(int test_id)
{
    int ret = 0, in_len;
    int out1_len = 0, out2_len = 0, out3_len = 0;
    EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *ctx = NULL;
    unsigned char out1[256];
    unsigned char out2[256];
    unsigned char out3[256];
    static const unsigned char in[32] = {
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0xba, 0xbe, 0xba, 0xbe, 0x00, 0x00, 0xba, 0xbe,
        0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    };
    static const unsigned char key[64] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x03, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    };
    static const unsigned char iv[48] = {
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08,
        0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00
    };
    const char *name = sk_OPENSSL_STRING_value(cipher_names, test_id);

    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new()))
        goto err;

    TEST_note("Fetching %s\n", name);
    if (!TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, name, NULL)))
        goto err;

    in_len = EVP_CIPHER_get_block_size(cipher);
    if (!TEST_int_gt(in_len, 0))
        goto err;
    if (in_len > 1)
        in_len /= 2;

    /* skip any ciphers that don't allow partial updates */
    if (((EVP_CIPHER_get_flags(cipher)
          & (EVP_CIPH_FLAG_CTS | EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK)) != 0)
        || EVP_CIPHER_get_mode(cipher) == EVP_CIPH_CCM_MODE
        || EVP_CIPHER_get_mode(cipher) == EVP_CIPH_XTS_MODE
        || EVP_CIPHER_get_mode(cipher) == EVP_CIPH_WRAP_MODE) {
        ret = 1;
        goto err;
    }

    if (!TEST_true(EVP_EncryptInit_ex(ctx, cipher, NULL, key, iv))
        || !TEST_true(EVP_EncryptUpdate(ctx, out1, &out1_len, in, in_len))
        || !TEST_true(EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv))
        || !TEST_true(EVP_EncryptUpdate(ctx, out2, &out2_len, in, in_len)))
        goto err;

    if (EVP_CIPHER_get_iv_length(cipher) != 0)
        if (!TEST_mem_eq(out1, out1_len, out2, out2_len))
            goto err;

    if (EVP_CIPHER_get_mode(cipher) != EVP_CIPH_SIV_MODE) {
        if (!TEST_true(EVP_EncryptInit_ex(ctx, NULL, NULL, NULL, iv))
            || !TEST_true(EVP_EncryptUpdate(ctx, out3, &out3_len, in, in_len)))
            goto err;

        if (EVP_CIPHER_get_iv_length(cipher) != 0)
            if (!TEST_mem_eq(out1, out1_len, out3, out3_len))
                goto err;
    }
    ret = 1;
err:
    EVP_CIPHER_free(cipher);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int name_cmp(const char * const *a, const char * const *b)
{
    return OPENSSL_strcasecmp(*a, *b);
}

static void collect_cipher_names(EVP_CIPHER *cipher, void *cipher_names_list)
{
    STACK_OF(OPENSSL_STRING) *names = cipher_names_list;
    const char *name = EVP_CIPHER_get0_name(cipher);
    char *namedup = NULL;

    /* Skip Triple-DES encryption operations in FIPS mode */
    if (OSSL_PROVIDER_available(libctx, "fips")
            && strncmp(name, "DES", 3) == 0)
        return;
    assert(name != NULL);
    /* the cipher will be freed after returning, strdup is needed */
    if ((namedup = OPENSSL_strdup(name)) != NULL
        && !sk_OPENSSL_STRING_push(names, namedup))
        OPENSSL_free(namedup);
}

static int rsa_keygen(int bits, EVP_PKEY **pub, EVP_PKEY **priv)
{
    int ret = 0;
    unsigned char *pub_der = NULL;
    const unsigned char *pp = NULL;
    size_t len = 0;
    OSSL_ENCODER_CTX *ectx = NULL;

    if (!TEST_ptr(*priv = EVP_PKEY_Q_keygen(libctx, NULL, "RSA", (size_t)bits))
        || !TEST_ptr(ectx =
                     OSSL_ENCODER_CTX_new_for_pkey(*priv,
                                                   EVP_PKEY_PUBLIC_KEY,
                                                   "DER", "type-specific",
                                                   NULL))
        || !TEST_true(OSSL_ENCODER_to_data(ectx, &pub_der, &len)))
        goto err;
    pp = pub_der;
    if (!TEST_ptr(d2i_PublicKey(EVP_PKEY_RSA, pub, &pp, len)))
        goto err;
    ret = 1;
err:
    OSSL_ENCODER_CTX_free(ectx);
    OPENSSL_free(pub_der);
    return ret;
}

static int kem_rsa_gen_recover(void)
{
    int ret = 0;
    EVP_PKEY *pub = NULL;
    EVP_PKEY *priv = NULL;
    EVP_PKEY_CTX *sctx = NULL, *rctx = NULL, *dctx = NULL;
    unsigned char secret[256] = { 0, };
    unsigned char ct[256] = { 0, };
    unsigned char unwrap[256] = { 0, };
    size_t ctlen = 0, unwraplen = 0, secretlen = 0;
    int bits = 2048;

    ret = TEST_true(rsa_keygen(bits, &pub, &priv))
          && TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(libctx, pub, NULL))
          && TEST_int_eq(EVP_PKEY_encapsulate_init(sctx, NULL), 1)
          && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(sctx, "RSASVE"), 1)
          && TEST_ptr(dctx = EVP_PKEY_CTX_dup(sctx))
          /* Test that providing a NULL wrappedlen fails */
          && TEST_int_eq(EVP_PKEY_encapsulate(dctx, NULL, NULL, NULL, NULL), 0)
          && TEST_int_eq(EVP_PKEY_encapsulate(dctx, NULL, &ctlen, NULL,
                                              &secretlen), 1)
          && TEST_int_eq(ctlen, secretlen)
          && TEST_int_eq(ctlen, bits / 8)
          && TEST_int_eq(EVP_PKEY_encapsulate(dctx, ct, &ctlen, secret,
                                              &secretlen), 1)
          && TEST_ptr(rctx = EVP_PKEY_CTX_new_from_pkey(libctx, priv, NULL))
          && TEST_int_eq(EVP_PKEY_decapsulate_init(rctx, NULL), 1)
          && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(rctx, "RSASVE"), 1)
          /* Test that providing a NULL unwrappedlen fails */
          && TEST_int_eq(EVP_PKEY_decapsulate(rctx, NULL, NULL, ct, ctlen), 0)
          && TEST_int_eq(EVP_PKEY_decapsulate(rctx, NULL, &unwraplen,
                                              ct, ctlen), 1)
          && TEST_int_eq(EVP_PKEY_decapsulate(rctx, unwrap, &unwraplen,
                                              ct, ctlen), 1)
          && TEST_mem_eq(unwrap, unwraplen, secret, secretlen);

    /* Test that providing a too short unwrapped/ctlen fails */
    if (fips_provider_version_match(libctx, ">=3.4.0")) {
        ctlen = 1;
        if (!TEST_int_eq(EVP_PKEY_encapsulate(dctx, ct, &ctlen, secret,
                                              &secretlen), 0))
            ret = 0;
        unwraplen = 1;
        if (!TEST_int_eq(EVP_PKEY_decapsulate(rctx, unwrap, &unwraplen, ct,
                                              ctlen), 0))
            ret = 0;
    }

    EVP_PKEY_free(pub);
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(rctx);
    EVP_PKEY_CTX_free(dctx);
    EVP_PKEY_CTX_free(sctx);
    return ret;
}

#ifndef OPENSSL_NO_DES
/*
 * This test makes sure that EVP_CIPHER_CTX_rand_key() works correctly
 * For fips mode this code would produce an error if the flag is not set.
 */
static int test_cipher_tdes_randkey(void)
{
    int ret;
    EVP_CIPHER_CTX *ctx = NULL;
    EVP_CIPHER *tdes_cipher = NULL, *aes_cipher = NULL;
    unsigned char key[24] = { 0 };
    OSSL_PARAM params[2];
    int check = 0;

    params[0] = OSSL_PARAM_construct_int("encrypt-check", &check);
    params[1] = OSSL_PARAM_construct_end();
    ret = TEST_ptr(aes_cipher = EVP_CIPHER_fetch(libctx, "AES-256-CBC", NULL))
          && TEST_int_eq(EVP_CIPHER_get_flags(aes_cipher) & EVP_CIPH_RAND_KEY, 0)
          && TEST_ptr(tdes_cipher = EVP_CIPHER_fetch(libctx, "DES-EDE3-CBC", NULL))
          && TEST_int_ne(EVP_CIPHER_get_flags(tdes_cipher) & EVP_CIPH_RAND_KEY, 0)
          && TEST_ptr(ctx = EVP_CIPHER_CTX_new())
          && TEST_true(EVP_CipherInit_ex2(ctx, tdes_cipher, NULL, NULL, 1,
                                          params))
          && TEST_int_gt(EVP_CIPHER_CTX_rand_key(ctx, key), 0);

    EVP_CIPHER_CTX_free(ctx);
    EVP_CIPHER_free(tdes_cipher);
    EVP_CIPHER_free(aes_cipher);
    return ret;
}
#endif /* OPENSSL_NO_DES */

static int kem_rsa_params(void)
{
    int ret = 0;
    EVP_PKEY *pub = NULL;
    EVP_PKEY *priv = NULL;
    EVP_PKEY_CTX *pubctx = NULL, *privctx = NULL;
    unsigned char secret[256] = { 0, };
    unsigned char ct[256] = { 0, };
    size_t ctlen = 0, secretlen = 0;

    ret = TEST_true(rsa_keygen(2048, &pub, &priv))
        && TEST_ptr(pubctx = EVP_PKEY_CTX_new_from_pkey(libctx, pub, NULL))
        && TEST_ptr(privctx = EVP_PKEY_CTX_new_from_pkey(libctx, priv, NULL))
        /* Test setting kem op before the init fails */
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(pubctx, "RSASVE"), -2)
        /* Test NULL ctx passed */
        && TEST_int_eq(EVP_PKEY_encapsulate_init(NULL, NULL), 0)
        && TEST_int_eq(EVP_PKEY_encapsulate(NULL, NULL, NULL, NULL, NULL), 0)
        && TEST_int_eq(EVP_PKEY_decapsulate_init(NULL, NULL), 0)
        && TEST_int_eq(EVP_PKEY_decapsulate(NULL, NULL, NULL, NULL, 0), 0)
        /* Test Invalid operation */
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, NULL, NULL, NULL), -1)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, NULL, NULL, NULL, 0), 0)
        /* Wrong key component - no secret should be returned on failure */
        && TEST_int_eq(EVP_PKEY_decapsulate_init(pubctx, NULL), 1)
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(pubctx, "RSASVE"), 1)
        && TEST_int_eq(EVP_PKEY_decapsulate(pubctx, secret, &secretlen, ct,
                                            sizeof(ct)), 0)
        && TEST_uchar_eq(secret[0], 0)
        /* Unless older FIPS, test encapsulate succeeds even if the mode is not set */
        && TEST_int_eq(EVP_PKEY_encapsulate_init(pubctx, NULL), 1)
        && (is_fips_lt_3_5 ||
            (TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, &ctlen, NULL, &secretlen), 1)
             && TEST_true(ctlen <= sizeof(ct))
             && TEST_true(secretlen <= sizeof(secret))
             && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, ct, &ctlen, secret, &secretlen), 1)))
        /* Test setting a bad kem ops fail */
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(pubctx, "RSA"), 0)
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(pubctx, NULL), 0)
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(NULL,  "RSASVE"), 0)
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(NULL,  NULL), 0)
        /* Test secretlen is optional */
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(pubctx, "RSASVE"), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, &ctlen, NULL, NULL), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, ct, &ctlen, secret, NULL), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, &ctlen, NULL, NULL), 1)
        /* Test outlen is optional */
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, NULL, NULL, &secretlen), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, ct, NULL, secret, &secretlen), 1)
        /* test that either len must be set if out is NULL */
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, NULL, NULL, NULL), 0)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, &ctlen, NULL, NULL), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, NULL, NULL, &secretlen), 1)
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, NULL, &ctlen, NULL, &secretlen), 1)
        /* Secret buffer should be set if there is an output buffer */
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, ct, &ctlen, NULL, NULL), 0)
        /* Test that lengths are optional if ct is not NULL */
        && TEST_int_eq(EVP_PKEY_encapsulate(pubctx, ct, NULL, secret, NULL), 1)
        /* Pass if secret or secret length are not NULL */
        && TEST_int_eq(EVP_PKEY_decapsulate_init(privctx, NULL), 1)
        && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(privctx, "RSASVE"), 1)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, secret, NULL, ct, sizeof(ct)), 1)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, NULL, &secretlen, ct, sizeof(ct)), 1)
        && TEST_int_eq(secretlen, 256)
        /* Fail if passed NULL arguments */
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, NULL, NULL, ct, sizeof(ct)), 0)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, secret, &secretlen, NULL, 0), 0)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, secret, &secretlen, NULL, sizeof(ct)), 0)
        && TEST_int_eq(EVP_PKEY_decapsulate(privctx, secret, &secretlen, ct, 0), 0);

    EVP_PKEY_free(pub);
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(pubctx);
    EVP_PKEY_CTX_free(privctx);
    return ret;
}

#ifndef OPENSSL_NO_DH
static EVP_PKEY *gen_dh_key(void)
{
    EVP_PKEY_CTX *gctx = NULL;
    EVP_PKEY *pkey = NULL;
    OSSL_PARAM params[2];

    params[0] = OSSL_PARAM_construct_utf8_string("group", "ffdhe2048", 0);
    params[1] = OSSL_PARAM_construct_end();

    if (!TEST_ptr(gctx = EVP_PKEY_CTX_new_from_name(libctx, "DH", NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(gctx), 0)
        || !TEST_true(EVP_PKEY_CTX_set_params(gctx, params))
        || !TEST_true(EVP_PKEY_keygen(gctx, &pkey)))
        goto err;
err:
    EVP_PKEY_CTX_free(gctx);
    return pkey;
}

/* Fail if we try to use a dh key */
static int kem_invalid_keytype(void)
{
    int ret = 0;
    EVP_PKEY *key = NULL;
    EVP_PKEY_CTX *sctx = NULL;

    if (!TEST_ptr(key = gen_dh_key()))
        goto done;

    if (!TEST_ptr(sctx = EVP_PKEY_CTX_new_from_pkey(libctx, key, NULL)))
        goto done;
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(sctx, NULL), -2))
        goto done;

    ret = 1;
done:
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(sctx);
    return ret;
}
#endif /* OPENSSL_NO_DH */

int setup_tests(void)
{
    const char *prov_name = "default";
    char *config_file = NULL;
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_PROVIDER_NAME:
            prov_name = opt_arg();
            break;
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    if (!test_get_libctx(&libctx, &nullprov, config_file, &libprov, prov_name))
        return 0;

    ADD_TEST(test_evp_cipher_api_safety);

    if (strcmp(prov_name, "fips") == 0)
        is_fips = 1;

    is_fips_lt_3_5 = is_fips && fips_provider_version_lt(libctx, 3, 5, 0);

#if !defined(OPENSSL_NO_DSA) && !defined(OPENSSL_NO_DH)
    if (!is_fips || fips_provider_version_lt(libctx, 3, 4, 0))
        ADD_ALL_TESTS(test_dsa_param_keygen, 3 * 3 * 3);
#endif
#ifndef OPENSSL_NO_DH
    ADD_ALL_TESTS(test_dh_safeprime_param_keygen, 3 * 3 * 3);
    ADD_TEST(dhx_cert_load);
#endif

    if (!TEST_ptr(cipher_names = sk_OPENSSL_STRING_new(name_cmp)))
        return 0;
    EVP_CIPHER_do_all_provided(libctx, collect_cipher_names, cipher_names);

    ADD_ALL_TESTS(test_cipher_reinit, sk_OPENSSL_STRING_num(cipher_names));
    ADD_ALL_TESTS(test_cipher_reinit_partialupdate,
                  sk_OPENSSL_STRING_num(cipher_names));
    ADD_TEST(kem_rsa_gen_recover);
    ADD_TEST(kem_rsa_params);
#ifndef OPENSSL_NO_DH
    ADD_TEST(kem_invalid_keytype);
#endif
#ifndef OPENSSL_NO_DES
    ADD_TEST(test_cipher_tdes_randkey);
#endif
    return 1;
}

/* Because OPENSSL_free is a macro, it can't be passed as a function pointer */
static void string_free(char *m)
{
    OPENSSL_free(m);
}

void cleanup_tests(void)
{
    sk_OPENSSL_STRING_pop_free(cipher_names, string_free);
    OSSL_PROVIDER_unload(libprov);
    OSSL_LIB_CTX_free(libctx);
    OSSL_PROVIDER_unload(nullprov);
}
