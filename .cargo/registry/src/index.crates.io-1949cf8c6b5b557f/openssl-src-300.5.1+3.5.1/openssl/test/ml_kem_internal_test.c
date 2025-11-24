/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/rand.h>
#include <openssl/core_names.h>
#ifndef OPENSSL_NO_STDIO
# include <stdio.h>
#endif
#include <crypto/ml_kem.h>
#include "testutil.h"
#include "testutil/output.h"

static uint8_t ml_kem_private_entropy[] = {
    /* Seed for genkey */
    0x7c, 0x99, 0x35, 0xa0, 0xb0, 0x76, 0x94, 0xaa, 0x0c, 0x6d, 0x10, 0xe4,
    0xdb, 0x6b, 0x1a, 0xdd, 0x2f, 0xd8, 0x1a, 0x25, 0xcc, 0xb1, 0x48, 0x03,
    0x2d, 0xcd, 0x73, 0x99, 0x36, 0x73, 0x7f, 0x2d, 0x86, 0x26, 0xed, 0x79,
    0xd4, 0x51, 0x14, 0x08, 0x00, 0xe0, 0x3b, 0x59, 0xb9, 0x56, 0xf8, 0x21,
    0x0e, 0x55, 0x60, 0x67, 0x40, 0x7d, 0x13, 0xdc, 0x90, 0xfa, 0x9e, 0x8b,
    0x87, 0x2b, 0xfb, 0x8f
};
static uint8_t ml_kem_public_entropy[] = {
    /* Seed for encap */
    0x14, 0x7c, 0x03, 0xf7, 0xa5, 0xbe, 0xbb, 0xa4, 0x06, 0xc8, 0xfa, 0xe1,
    0x87, 0x4d, 0x7f, 0x13, 0xc8, 0x0e, 0xfe, 0x79, 0xa3, 0xa9, 0xa8, 0x74,
    0xcc, 0x09, 0xfe, 0x76, 0xf6, 0x99, 0x76, 0x15,
    /* Seed for decap on length error */
    0x4e, 0x6f, 0x74, 0x20, 0x74, 0x68, 0x65, 0x20, 0x64, 0x72, 0x6f, 0x69,
    0x64, 0x73, 0x20, 0x79, 0x6f, 0x75, 0x27, 0x72, 0x65, 0x20, 0x6c, 0x6f,
    0x6f, 0x6b, 0x69, 0x6e, 0x67, 0x20, 0x66, 0x6f
};
static uint8_t ml_kem_expected_rho[3][ML_KEM_RANDOM_BYTES] = {
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
static uint8_t ml_kem_expected_ctext_sha256[3][32] = {
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
static uint8_t ml_kem_expected_shared_secret[3][32] = {
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


static int sanity_test(void)
{
    static const int alg[3] = {
        EVP_PKEY_ML_KEM_512,
        EVP_PKEY_ML_KEM_768,
        EVP_PKEY_ML_KEM_1024
    };
    EVP_RAND_CTX *privctx;
    EVP_RAND_CTX *pubctx;
    EVP_MD *sha256 = EVP_MD_fetch(NULL, "sha256", NULL);
    uint8_t *decap_entropy;
    int i, ret = 0;

    if (!TEST_ptr(sha256))
        return 0;

    if (!TEST_ptr(privctx = RAND_get0_private(NULL))
        || !TEST_ptr(pubctx = RAND_get0_public(NULL)))
        return 0;

    decap_entropy = ml_kem_public_entropy + ML_KEM_RANDOM_BYTES;

    for (i = 0; i < (int) OSSL_NELEM(alg); ++i) {
        OSSL_PARAM params[3];
        uint8_t hash[32];
        uint8_t shared_secret[ML_KEM_SHARED_SECRET_BYTES];
        uint8_t shared_secret2[ML_KEM_SHARED_SECRET_BYTES];
        uint8_t *encoded_public_key = NULL;
        uint8_t *ciphertext = NULL;
        ML_KEM_KEY *private_key = NULL;
        ML_KEM_KEY *public_key = NULL;
        int ret2 = -1;
        unsigned char c;
        unsigned int strength = 256;
        const ML_KEM_VINFO *v;

        /* Configure the private RNG to output just the keygen seed */
        params[0] =
            OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                              ml_kem_private_entropy,
                                              sizeof(ml_kem_private_entropy));
        params[1] =
            OSSL_PARAM_construct_uint(OSSL_RAND_PARAM_STRENGTH, &strength);
        params[2] = OSSL_PARAM_construct_end();
        if (!TEST_true(EVP_RAND_CTX_set_params(privctx, params)))
            return 0;

        public_key = ossl_ml_kem_key_new(NULL, NULL, alg[i]);
        private_key = ossl_ml_kem_key_new(NULL, NULL, alg[i]);
        if (private_key == NULL || public_key == NULL
            || (v = ossl_ml_kem_key_vinfo(public_key)) == NULL)
            goto done;

        encoded_public_key = OPENSSL_malloc(v->pubkey_bytes);
        ciphertext = OPENSSL_malloc(v->ctext_bytes);
        if (encoded_public_key == NULL || ciphertext == NULL)
            goto done;

        ret2 = -2;
        /* Generate a private key */
        if (!ossl_ml_kem_genkey(encoded_public_key, v->pubkey_bytes,
                                private_key))
            goto done;

        /* Check that no more entropy is available! */
        if (!TEST_int_le(RAND_priv_bytes(&c, 1), 0))
            goto done;

        ret2 = -3;
        /* Check that we got the expected 'rho' value in the ciphertext */
        if (!TEST_mem_eq(encoded_public_key + v->vector_bytes,
                         ML_KEM_RANDOM_BYTES,
                         ml_kem_expected_rho[i],
                         ML_KEM_RANDOM_BYTES))
            goto done;

        ret2 = -4;
        /* Create the expected associated public key */
        if (!ossl_ml_kem_parse_public_key(encoded_public_key, v->pubkey_bytes,
                                          public_key))
            goto done;

        /* Configure the public RNG to output the encap and decap seeds */
        params[0] =
            OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                              ml_kem_public_entropy,
                                              sizeof(ml_kem_public_entropy));
        if (!TEST_true(EVP_RAND_CTX_set_params(pubctx, params)))
            goto done;

        /* encaps - decaps test: validate shared secret equality */
        ret2 = -5;
        if (!ossl_ml_kem_encap_rand(ciphertext, v->ctext_bytes,
                                    shared_secret, sizeof(shared_secret),
                                    public_key))
            goto done;

        ret2 = -6;
        /* Check the ciphertext hash */
        if (!TEST_true(EVP_Digest(ciphertext, v->ctext_bytes,
                                  hash, NULL, sha256, NULL))
            || !TEST_mem_eq(hash, sizeof(hash),
                            ml_kem_expected_ctext_sha256[i],
                            sizeof(ml_kem_expected_ctext_sha256[i])))
            goto done;

        /* Check for the expected shared secret */
        if (!TEST_mem_eq(shared_secret, sizeof(shared_secret),
                         ml_kem_expected_shared_secret[i],
                         ML_KEM_SHARED_SECRET_BYTES))
            goto done;

        /* Now decapsulate the ciphertext */
        ret2 = -7;
        if (!ossl_ml_kem_decap(shared_secret2, sizeof(shared_secret2),
                               ciphertext, v->ctext_bytes, private_key))
            goto done;

        /* Check for the same shared secret */
        if (!TEST_mem_eq(shared_secret, sizeof(shared_secret),
                         shared_secret2, sizeof(shared_secret2)))
            goto done;

        ret2 = -8;
        /* Now a quick negative test by zeroing the ciphertext */
        memset(ciphertext, 0, v->ctext_bytes);
        if (!TEST_true(ossl_ml_kem_decap(shared_secret2, sizeof(shared_secret2),
                                         ciphertext, v->ctext_bytes,
                                         private_key)))
            goto done;

        /* Ensure we have a mismatch */
        if (!TEST_mem_ne(shared_secret, sizeof(shared_secret),
                         shared_secret2, sizeof(shared_secret2)))
            goto done;

        ret2 = -9;
        /*
         * Change the ciphertext length, decap should fail, but and consume the
         * last batch of entropy to return a fake shared secret, just in case.
         */
        if (!TEST_false(ossl_ml_kem_decap(shared_secret2, sizeof(shared_secret2),
                                          ciphertext, v->ctext_bytes - 1,
                                          private_key)))
            goto done;

        if (!TEST_mem_eq(shared_secret2, sizeof(shared_secret2),
                         decap_entropy, ML_KEM_SHARED_SECRET_BYTES))
            goto done;

        /* Check that no more entropy is available! */
        if (!TEST_int_le(RAND_bytes(&c, 1), 0))
            goto done;

        ret2 = 0;

    done:
        if (ret2 != 0)
            ret = ret2;
        ossl_ml_kem_key_free(private_key);
        ossl_ml_kem_key_free(public_key);
        OPENSSL_free(encoded_public_key);
        OPENSSL_free(ciphertext);
    }
    EVP_MD_free(sha256);
    return ret == 0;
}

int setup_tests(void)
{
    if (!TEST_true(RAND_set_DRBG_type(NULL, "TEST-RAND", "fips=no", NULL, NULL)))
        return 0;

    ADD_TEST(sanity_test);
    return 1;
}
