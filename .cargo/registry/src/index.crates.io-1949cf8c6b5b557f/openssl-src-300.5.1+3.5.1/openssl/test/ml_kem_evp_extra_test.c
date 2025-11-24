/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/conf.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/param_build.h>
#include <openssl/rand.h>
#include <crypto/ml_kem.h>
#include "testutil.h"

static OSSL_LIB_CTX *testctx = NULL;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_TEST_RAND,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "test-rand", OPT_TEST_RAND, '-', "Test non-derandomised ML-KEM" },
        { NULL }
    };
    return options;
}

static uint8_t gen_seed[64] = {
    0x7c, 0x99, 0x35, 0xa0, 0xb0, 0x76, 0x94, 0xaa, 0x0c, 0x6d, 0x10, 0xe4,
    0xdb, 0x6b, 0x1a, 0xdd, 0x2f, 0xd8, 0x1a, 0x25, 0xcc, 0xb1, 0x48, 0x03,
    0x2d, 0xcd, 0x73, 0x99, 0x36, 0x73, 0x7f, 0x2d, 0x86, 0x26, 0xed, 0x79,
    0xd4, 0x51, 0x14, 0x08, 0x00, 0xe0, 0x3b, 0x59, 0xb9, 0x56, 0xf8, 0x21,
    0x0e, 0x55, 0x60, 0x67, 0x40, 0x7d, 0x13, 0xdc, 0x90, 0xfa, 0x9e, 0x8b,
    0x87, 0x2b, 0xfb, 0x8f
};
static uint8_t enc_seed[32] = {
    0x14, 0x7c, 0x03, 0xf7, 0xa5, 0xbe, 0xbb, 0xa4, 0x06, 0xc8, 0xfa, 0xe1,
    0x87, 0x4d, 0x7f, 0x13, 0xc8, 0x0e, 0xfe, 0x79, 0xa3, 0xa9, 0xa8, 0x74,
    0xcc, 0x09, 0xfe, 0x76, 0xf6, 0x99, 0x76, 0x15
};
static uint8_t dec_seed[32] = {
    0x4e, 0x6f, 0x74, 0x20, 0x74, 0x68, 0x65, 0x20, 0x64, 0x72, 0x6f, 0x69,
    0x64, 0x73, 0x20, 0x79, 0x6f, 0x75, 0x27, 0x72, 0x65, 0x20, 0x6c, 0x6f,
    0x6f, 0x6b, 0x69, 0x6e, 0x67, 0x20, 0x66, 0x6f
};
static uint8_t expected_rho[3][32] = {
    {
        0x7e, 0xfb, 0x9e, 0x40, 0xc3, 0xbf, 0x0f, 0xf0, 0x43, 0x29, 0x86, 0xae,
        0x4b, 0xc1, 0xa2, 0x42, 0xce, 0x99, 0x21, 0xaa, 0x9e, 0x22, 0x44, 0x88,
        0x19, 0x58, 0x5d, 0xea, 0x30, 0x8e, 0xb0, 0x39
    },
    {
        0x16, 0x2e, 0xc0, 0x98, 0xa9, 0x00, 0xb1, 0x2d, 0xd8, 0xfa, 0xbb, 0xfb,
        0x3f, 0xe8, 0xcb, 0x1d, 0xc4, 0xe8, 0x31, 0x5f, 0x2a, 0xf0, 0xd3, 0x2f,
        0x00, 0x17, 0xae, 0x13, 0x6e, 0x19, 0xf0, 0x28
    },
    {
        0x29, 0xb4, 0xf9, 0xf8, 0xcf, 0xba, 0xdf, 0x2e, 0x41, 0x86, 0x9a, 0xbf,
        0xba, 0xd1, 0x07, 0x38, 0xad, 0x04, 0xcc, 0x75, 0x2b, 0xc2, 0x0c, 0x39,
        0x47, 0x46, 0x85, 0x0e, 0x0c, 0x48, 0x47, 0xdb
    }
};
static uint8_t expected_ctext_sha256[3][32] = {
    {
        0xbc, 0x29, 0xd7, 0xdf, 0x8b, 0xc5, 0x46, 0x5d, 0x98, 0x06, 0x01, 0xd8,
        0x00, 0x25, 0x97, 0x93, 0xe2, 0x60, 0x38, 0x25, 0xa5, 0x72, 0xda, 0x6c,
        0xd1, 0x98, 0xa5, 0x12, 0xcc, 0x6d, 0x1a, 0x34
    },
    {
        0x36, 0x82, 0x9a, 0x2f, 0x35, 0xcb, 0xf4, 0xde, 0xb6, 0x2c, 0x0a, 0x12,
        0xa1, 0x5c, 0x22, 0xda, 0xe9, 0xf8, 0xd2, 0xc2, 0x52, 0x56, 0x6f, 0xc2,
        0x4f, 0x88, 0xab, 0xe8, 0x05, 0xcb, 0x57, 0x5e
    },
    {
        0x50, 0x81, 0x36, 0xa1, 0x3f, 0x8a, 0x79, 0x20, 0xe3, 0x43, 0x44, 0x98,
        0xc6, 0x97, 0x5c, 0xbb, 0xab, 0x45, 0x7d, 0x80, 0x93, 0x09, 0xeb, 0x2f,
        0x92, 0x45, 0x3e, 0x74, 0x09, 0x73, 0x82, 0x10
    }
};
static uint8_t expected_shared_secret[3][32] = {
    {
        0x31, 0x98, 0x39, 0xe8, 0x2a, 0xb6, 0xb2, 0x22, 0xde, 0x7b, 0x61, 0x9e,
        0x80, 0xda, 0x83, 0x91, 0x52, 0x2b, 0xbb, 0x37, 0x67, 0x70, 0x18, 0x49,
        0x4a, 0x47, 0x42, 0xc5, 0x3f, 0x9a, 0xbf, 0xdf
    },
    {
        0xe7, 0x18, 0x4a, 0x09, 0x75, 0xee, 0x34, 0x70, 0x87, 0x8d, 0x2d, 0x15,
        0x9e, 0xc8, 0x31, 0x29, 0xc8, 0xae, 0xc2, 0x53, 0xd4, 0xee, 0x17, 0xb4,
        0x81, 0x03, 0x11, 0xd1, 0x98, 0xcd, 0x03, 0x68
    },
    {
        0x48, 0x9d, 0xd1, 0xe9, 0xc2, 0xbe, 0x4a, 0xf3, 0x48, 0x2b, 0xdb, 0x35,
        0xbb, 0x26, 0xce, 0x76, 0x0e, 0x6e, 0x41, 0x4d, 0xa6, 0xec, 0xbe, 0x48,
        0x99, 0x85, 0x74, 0x8a, 0x82, 0x5f, 0x1c, 0xd6
    },
};

static int test_ml_kem(void)
{
    EVP_PKEY *akey, *bkey = NULL;
    int res = 0;
    size_t publen;
    unsigned char *rawpub = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    unsigned char *wrpkey = NULL, *agenkey = NULL, *bgenkey = NULL;
    size_t wrpkeylen, agenkeylen, bgenkeylen, i;

    /* Generate Alice's key */
    akey = EVP_PKEY_Q_keygen(testctx, NULL, "ML-KEM-768");
    if (!TEST_ptr(akey))
        goto err;

    /* Get the raw public key */
    publen = EVP_PKEY_get1_encoded_public_key(akey, &rawpub);
    if (!TEST_size_t_gt(publen, 0))
        goto err;

    /* Create Bob's key and populate it with Alice's public key data */
    bkey = EVP_PKEY_new();
    if (!TEST_ptr(bkey))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_copy_parameters(bkey, akey), 0))
        goto err;

    if (!TEST_true(EVP_PKEY_set1_encoded_public_key(bkey, rawpub, publen)))
        goto err;

    /* Encapsulate Bob's key */
    ctx = EVP_PKEY_CTX_new_from_pkey(testctx, bkey, NULL);
    if (!TEST_ptr(ctx))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_encapsulate_init(ctx, NULL), 0))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_encapsulate(ctx, NULL, &wrpkeylen, NULL,
                                          &bgenkeylen), 0))
        goto err;

    if (!TEST_size_t_gt(wrpkeylen, 0) || !TEST_size_t_gt(bgenkeylen, 0))
        goto err;

    wrpkey = OPENSSL_zalloc(wrpkeylen);
    bgenkey = OPENSSL_zalloc(bgenkeylen);
    if (!TEST_ptr(wrpkey) || !TEST_ptr(bgenkey))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_encapsulate(ctx, wrpkey, &wrpkeylen, bgenkey,
                                          &bgenkeylen), 0))
        goto err;

    EVP_PKEY_CTX_free(ctx);

    /* Alice now decapsulates Bob's key */
    ctx = EVP_PKEY_CTX_new_from_pkey(testctx, akey, NULL);
    if (!TEST_ptr(ctx))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_decapsulate_init(ctx, NULL), 0))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_decapsulate(ctx, NULL, &agenkeylen, wrpkey,
                                          wrpkeylen), 0))
        goto err;

    if (!TEST_size_t_gt(agenkeylen, 0))
        goto err;

    agenkey = OPENSSL_zalloc(agenkeylen);
    if (!TEST_ptr(agenkey))
        goto err;

    if (!TEST_int_gt(EVP_PKEY_decapsulate(ctx, agenkey, &agenkeylen, wrpkey,
                                          wrpkeylen), 0))
        goto err;

    /* Hopefully we ended up with a shared key */
    if (!TEST_mem_eq(agenkey, agenkeylen, bgenkey, bgenkeylen))
        goto err;

    /* Verify we generated a non-zero shared key */
    for (i = 0; i < agenkeylen; i++)
        if (agenkey[i] != 0)
            break;
    if (!TEST_size_t_ne(i, agenkeylen))
        goto err;

    res = 1;
 err:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(akey);
    EVP_PKEY_free(bkey);
    OPENSSL_free(rawpub);
    OPENSSL_free(wrpkey);
    OPENSSL_free(agenkey);
    OPENSSL_free(bgenkey);
    return res;
}

static int test_non_derandomised_ml_kem(void)
{
    static const int alg[3] = {
        EVP_PKEY_ML_KEM_512,
        EVP_PKEY_ML_KEM_768,
        EVP_PKEY_ML_KEM_1024
    };
    EVP_RAND_CTX *privctx;
    EVP_RAND_CTX *pubctx;
    EVP_MD *sha256;
    int i, ret = 0;

    if (!TEST_ptr(privctx = RAND_get0_private(NULL))
        || !TEST_ptr(pubctx = RAND_get0_public(NULL)))
        return 0;

    if (!TEST_ptr(sha256 = EVP_MD_fetch(NULL, "sha256", NULL)))
        return 0;

    for (i = 0; i < (int) OSSL_NELEM(alg); ++i) {
        const ML_KEM_VINFO *v;
        OSSL_PARAM params[3], *p;
        uint8_t hash[32];
        EVP_PKEY *akey = NULL, *bkey = NULL;
        size_t publen;
        unsigned char *rawpub = NULL;
        EVP_PKEY_CTX *ctx = NULL;
        unsigned char *wrpkey = NULL, *agenkey = NULL, *bgenkey = NULL;
        size_t wrpkeylen, agenkeylen, bgenkeylen;
        unsigned int strength = 256;
        unsigned char c;
        int res = -1;

        if ((v = ossl_ml_kem_get_vinfo(alg[i])) == NULL)
            goto done;

        /* Configure the private RNG to output just the keygen seed */
        p = params;
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                                 gen_seed, sizeof(gen_seed));
        *p++ = OSSL_PARAM_construct_uint(OSSL_RAND_PARAM_STRENGTH, &strength);
        *p = OSSL_PARAM_construct_end();
        if (!TEST_true(EVP_RAND_CTX_set_params(privctx, params)))
            goto done;

        res = -2;
        /* Generate Alice's key */
        akey = EVP_PKEY_Q_keygen(testctx, NULL, v->algorithm_name);
        if (!TEST_ptr(akey))
            goto done;

        /* Check that no more entropy is available! */
        if (!TEST_int_le(RAND_priv_bytes(&c, 1), 0))
            goto done;

        /* Get the raw public key */
        publen = EVP_PKEY_get1_encoded_public_key(akey, &rawpub);
        if (!TEST_size_t_eq(publen, v->pubkey_bytes))
            goto done;

        res = -3;
        /* Check that we got the expected 'rho' value in the ciphertext */
        if (!TEST_mem_eq(rawpub + v->vector_bytes, ML_KEM_RANDOM_BYTES,
                         expected_rho[i], ML_KEM_RANDOM_BYTES))
            goto done;

        res = -4;
        /* Create Bob's key and populate it with Alice's public key data */
        bkey = EVP_PKEY_new();
        if (!TEST_ptr(bkey))
            goto done;
        if (!TEST_int_gt(EVP_PKEY_copy_parameters(bkey, akey), 0))
            goto done;
        if (!TEST_true(EVP_PKEY_set1_encoded_public_key(bkey, rawpub, publen)))
            goto done;

        /* Configure the public RNG to output just the encap seed */
        p = params;
        *p = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                               enc_seed, sizeof(enc_seed));
        if (!TEST_true(EVP_RAND_CTX_set_params(pubctx, params)))
            goto done;

        /* Encapsulate Bob's key */
        res = -5;
        ctx = EVP_PKEY_CTX_new_from_pkey(testctx, bkey, NULL);
        if (!TEST_ptr(ctx))
            goto done;
        if (!TEST_int_gt(EVP_PKEY_encapsulate_init(ctx, NULL), 0))
            goto done;
        if (!TEST_int_gt(EVP_PKEY_encapsulate(ctx, NULL, &wrpkeylen, NULL,
                                              &bgenkeylen), 0))
            goto done;
        if (!TEST_size_t_eq(wrpkeylen, v->ctext_bytes)
            || !TEST_size_t_eq(bgenkeylen, ML_KEM_SHARED_SECRET_BYTES))
            goto done;
        wrpkey = OPENSSL_zalloc(wrpkeylen);
        bgenkey = OPENSSL_zalloc(bgenkeylen);
        if (!TEST_ptr(wrpkey) || !TEST_ptr(bgenkey))
            goto done;
        if (!TEST_true(EVP_PKEY_encapsulate(ctx, wrpkey, &wrpkeylen, bgenkey,
                                            &bgenkeylen)))
            goto done;
        EVP_PKEY_CTX_free(ctx);
        ctx = NULL;
        /* Check that no more public entropy is available! */
        if (!TEST_int_le(RAND_bytes(&c, 1), 0))
            goto done;

        res = -6;
        /* Check the ciphertext hash */
        if (!TEST_true(EVP_Digest(wrpkey, v->ctext_bytes,
                                  hash, NULL, sha256, NULL))
            || !TEST_mem_eq(hash, sizeof(hash),
                            expected_ctext_sha256[i],
                            sizeof(expected_ctext_sha256[i])))
            goto done;
        /* Check for the expected shared secret */
        if (!TEST_mem_eq(bgenkey, bgenkeylen,
                         expected_shared_secret[i], ML_KEM_SHARED_SECRET_BYTES))
            goto done;

        /*
         * Alice now decapsulates Bob's key.  Decap should not need a seed if
         * the ciphertext length is good.
         */
        res = -7;
        ctx = EVP_PKEY_CTX_new_from_pkey(testctx, akey, NULL);
        if (!TEST_ptr(ctx))
            goto done;
        if (!TEST_int_gt(EVP_PKEY_decapsulate_init(ctx, NULL), 0))
            goto done;
        if (!TEST_true(EVP_PKEY_decapsulate(ctx, NULL, &agenkeylen, wrpkey,
                                            wrpkeylen)))
            goto done;
        if (!TEST_size_t_eq(agenkeylen, ML_KEM_SHARED_SECRET_BYTES))
            goto done;
        agenkey = OPENSSL_zalloc(agenkeylen);
        if (!TEST_ptr(agenkey))
            goto done;
        if (!TEST_true(EVP_PKEY_decapsulate(ctx, agenkey, &agenkeylen, wrpkey,
                                            wrpkeylen)))
            goto done;
        /* Hopefully we ended up with a shared key */
        if (!TEST_mem_eq(agenkey, agenkeylen, bgenkey, bgenkeylen))
            goto done;

        res = -8;
        /* Now a quick negative test by zeroing the ciphertext */
        memset(wrpkey, 0, v->ctext_bytes);
        if (!TEST_true(EVP_PKEY_decapsulate(ctx, agenkey, &agenkeylen, wrpkey,
                                            wrpkeylen)))
            goto done;
        if (!TEST_mem_ne(agenkey, agenkeylen, bgenkey, bgenkeylen))
            goto done;

        res = -9;
        /* Configure decap entropy for a bad ciphertext length */
        p = params;
        *p = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                               dec_seed, sizeof(dec_seed));
        if (!TEST_true(EVP_RAND_CTX_set_params(pubctx, params)))
            goto done;

        /* This time decap should fail, and return the decap entropy */
        if (!TEST_false(EVP_PKEY_decapsulate(ctx, agenkey, &agenkeylen, wrpkey,
                                             wrpkeylen - 1)))
            goto done;
        if (!TEST_mem_eq(agenkey, agenkeylen, dec_seed, sizeof(dec_seed)))
            goto done;

        res = 0;

     done:
        EVP_PKEY_CTX_free(ctx);
        EVP_PKEY_free(akey);
        EVP_PKEY_free(bkey);
        OPENSSL_free(rawpub);
        OPENSSL_free(wrpkey);
        OPENSSL_free(agenkey);
        OPENSSL_free(bgenkey);
        if (res != 0)
            ret = res;
    }

    EVP_MD_free(sha256);
    return ret == 0;
}

int setup_tests(void)
{
    int test_rand = 0;
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_TEST_RAND:
            test_rand = 1;
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    if (test_rand != 0) {
        /* Cargo-culted from test/rand_test.c, this may need changes */
        if (!TEST_true(RAND_set_DRBG_type(NULL, "TEST-RAND", "fips=no",
                                             NULL, NULL)))
            return 0;
        ADD_TEST(test_non_derandomised_ml_kem);
        return 1;
    }

    ADD_TEST(test_ml_kem);
    return 1;
}
