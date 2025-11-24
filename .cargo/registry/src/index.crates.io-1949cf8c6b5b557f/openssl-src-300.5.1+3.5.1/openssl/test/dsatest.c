/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>

#include <openssl/crypto.h>
#include <openssl/rand.h>
#include <openssl/bn.h>
#include <openssl/dsa.h>
#include <openssl/evp.h>
#include <openssl/core_names.h>

#include "testutil.h"
#include "internal/nelem.h"

#ifndef OPENSSL_NO_DSA
static int dsa_cb(int p, int n, BN_GENCB *arg);

static unsigned char out_p[] = {
    0x8d, 0xf2, 0xa4, 0x94, 0x49, 0x22, 0x76, 0xaa,
    0x3d, 0x25, 0x75, 0x9b, 0xb0, 0x68, 0x69, 0xcb,
    0xea, 0xc0, 0xd8, 0x3a, 0xfb, 0x8d, 0x0c, 0xf7,
    0xcb, 0xb8, 0x32, 0x4f, 0x0d, 0x78, 0x82, 0xe5,
    0xd0, 0x76, 0x2f, 0xc5, 0xb7, 0x21, 0x0e, 0xaf,
    0xc2, 0xe9, 0xad, 0xac, 0x32, 0xab, 0x7a, 0xac,
    0x49, 0x69, 0x3d, 0xfb, 0xf8, 0x37, 0x24, 0xc2,
    0xec, 0x07, 0x36, 0xee, 0x31, 0xc8, 0x02, 0x91,
};
static unsigned char out_q[] = {
    0xc7, 0x73, 0x21, 0x8c, 0x73, 0x7e, 0xc8, 0xee,
    0x99, 0x3b, 0x4f, 0x2d, 0xed, 0x30, 0xf4, 0x8e,
    0xda, 0xce, 0x91, 0x5f,
};
static unsigned char out_g[] = {
    0x62, 0x6d, 0x02, 0x78, 0x39, 0xea, 0x0a, 0x13,
    0x41, 0x31, 0x63, 0xa5, 0x5b, 0x4c, 0xb5, 0x00,
    0x29, 0x9d, 0x55, 0x22, 0x95, 0x6c, 0xef, 0xcb,
    0x3b, 0xff, 0x10, 0xf3, 0x99, 0xce, 0x2c, 0x2e,
    0x71, 0xcb, 0x9d, 0xe5, 0xfa, 0x24, 0xba, 0xbf,
    0x58, 0xe5, 0xb7, 0x95, 0x21, 0x92, 0x5c, 0x9c,
    0xc4, 0x2e, 0x9f, 0x6f, 0x46, 0x4b, 0x08, 0x8c,
    0xc5, 0x72, 0xaf, 0x53, 0xe6, 0xd7, 0x88, 0x02,
};

static int dsa_test(void)
{
    BN_GENCB *cb;
    DSA *dsa = NULL;
    int counter, ret = 0, i, j;
    unsigned char buf[256];
    unsigned long h;
    unsigned char sig[256];
    unsigned int siglen;
    const BIGNUM *p = NULL, *q = NULL, *g = NULL;
    /*
     * seed, out_p, out_q, out_g are taken from the updated Appendix 5 to FIPS
     * PUB 186 and also appear in Appendix 5 to FIPS PIB 186-1
     */
    static unsigned char seed[20] = {
        0xd5, 0x01, 0x4e, 0x4b, 0x60, 0xef, 0x2b, 0xa8,
        0xb6, 0x21, 0x1b, 0x40, 0x62, 0xba, 0x32, 0x24,
        0xe0, 0x42, 0x7d, 0xd3,
    };
    static const unsigned char str1[] = "12345678901234567890";

    if (!TEST_ptr(cb = BN_GENCB_new()))
        goto end;

    BN_GENCB_set(cb, dsa_cb, NULL);
    if (!TEST_ptr(dsa = DSA_new())
        || !TEST_true(DSA_generate_parameters_ex(dsa, 512, seed, 20,
                                                &counter, &h, cb)))
        goto end;

    if (!TEST_int_eq(counter, 105))
        goto end;
    if (!TEST_int_eq(h, 2))
        goto end;

    DSA_get0_pqg(dsa, &p, &q, &g);
    i = BN_bn2bin(q, buf);
    j = sizeof(out_q);
    if (!TEST_int_eq(i, j) || !TEST_mem_eq(buf, i, out_q, i))
        goto end;

    i = BN_bn2bin(p, buf);
    j = sizeof(out_p);
    if (!TEST_int_eq(i, j) || !TEST_mem_eq(buf, i, out_p, i))
        goto end;

    i = BN_bn2bin(g, buf);
    j = sizeof(out_g);
    if (!TEST_int_eq(i, j) || !TEST_mem_eq(buf, i, out_g, i))
        goto end;

    if (!TEST_true(DSA_generate_key(dsa)))
        goto end;
    if (!TEST_true(DSA_sign(0, str1, 20, sig, &siglen, dsa)))
        goto end;
    if (TEST_int_gt(DSA_verify(0, str1, 20, sig, siglen, dsa), 0))
        ret = 1;
 end:
    DSA_free(dsa);
    BN_GENCB_free(cb);
    return ret;
}

static int dsa_cb(int p, int n, BN_GENCB *arg)
{
    static int ok = 0, num = 0;

    if (p == 0)
        num++;
    if (p == 2)
        ok++;

    if (!ok && (p == 0) && (num > 1)) {
        TEST_error("dsa_cb error");
        return 0;
    }
    return 1;
}

# define P      0
# define Q      1
# define G      2
# define SEED   3
# define PCOUNT 4
# define GINDEX 5
# define HCOUNT 6
# define GROUP  7

static int dsa_keygen_test(void)
{
    int ret = 0;
    EVP_PKEY *param_key = NULL, *key = NULL;
    EVP_PKEY_CTX *pg_ctx = NULL, *kg_ctx = NULL;
    BIGNUM *p_in = NULL, *q_in = NULL, *g_in = NULL;
    BIGNUM *p_out = NULL, *q_out = NULL, *g_out = NULL;
    int gindex_out = 0, pcount_out = 0, hcount_out = 0;
    unsigned char seed_out[32];
    char group_out[32];
    size_t len = 0;
    const OSSL_PARAM *settables = NULL;
    static const unsigned char seed_data[] = {
        0xa6, 0xf5, 0x28, 0x8c, 0x50, 0x77, 0xa5, 0x68,
        0x6d, 0x3a, 0xf5, 0xf1, 0xc6, 0x4c, 0xdc, 0x35,
        0x95, 0x26, 0x3f, 0x03, 0xdc, 0x00, 0x3f, 0x44,
        0x7b, 0x2a, 0xc7, 0x29
    };
    static const unsigned char expected_p[]= {
        0xdb, 0x47, 0x07, 0xaf, 0xf0, 0x06, 0x49, 0x55,
        0xc9, 0xbb, 0x09, 0x41, 0xb8, 0xdb, 0x1f, 0xbc,
        0xa8, 0xed, 0x12, 0x06, 0x7f, 0x88, 0x49, 0xb8,
        0xc9, 0x12, 0x87, 0x21, 0xbb, 0x08, 0x6c, 0xbd,
        0xf1, 0x89, 0xef, 0x84, 0xd9, 0x7a, 0x93, 0xe8,
        0x45, 0x40, 0x81, 0xec, 0x37, 0x27, 0x1a, 0xa4,
        0x22, 0x51, 0x99, 0xf0, 0xde, 0x04, 0xdb, 0xea,
        0xa1, 0xf9, 0x37, 0x83, 0x80, 0x96, 0x36, 0x53,
        0xf6, 0xae, 0x14, 0x73, 0x33, 0x0f, 0xdf, 0x0b,
        0xf9, 0x2f, 0x08, 0x46, 0x31, 0xf9, 0x66, 0xcd,
        0x5a, 0xeb, 0x6c, 0xf3, 0xbb, 0x74, 0xf3, 0x88,
        0xf0, 0x31, 0x5c, 0xa4, 0xc8, 0x0f, 0x86, 0xf3,
        0x0f, 0x9f, 0xc0, 0x8c, 0x57, 0xe4, 0x7f, 0x95,
        0xb3, 0x62, 0xc8, 0x4e, 0xae, 0xf3, 0xd8, 0x14,
        0xcc, 0x47, 0xc2, 0x4b, 0x4f, 0xef, 0xaf, 0xcd,
        0xcf, 0xb2, 0xbb, 0xe8, 0xbe, 0x08, 0xca, 0x15,
        0x90, 0x59, 0x35, 0xef, 0x35, 0x1c, 0xfe, 0xeb,
        0x33, 0x2e, 0x25, 0x22, 0x57, 0x9c, 0x55, 0x23,
        0x0c, 0x6f, 0xed, 0x7c, 0xb6, 0xc7, 0x36, 0x0b,
        0xcb, 0x2b, 0x6a, 0x21, 0xa1, 0x1d, 0x55, 0x77,
        0xd9, 0x91, 0xcd, 0xc1, 0xcd, 0x3d, 0x82, 0x16,
        0x9c, 0xa0, 0x13, 0xa5, 0x83, 0x55, 0x3a, 0x73,
        0x7e, 0x2c, 0x44, 0x3e, 0x70, 0x2e, 0x50, 0x91,
        0x6e, 0xca, 0x3b, 0xef, 0xff, 0x85, 0x35, 0x70,
        0xff, 0x61, 0x0c, 0xb1, 0xb2, 0xb7, 0x94, 0x6f,
        0x65, 0xa4, 0x57, 0x62, 0xef, 0x21, 0x83, 0x0f,
        0x3e, 0x71, 0xae, 0x7d, 0xe4, 0xad, 0xfb, 0xe3,
        0xdd, 0xd6, 0x03, 0xda, 0x9a, 0xd8, 0x8f, 0x2d,
        0xbb, 0x90, 0x87, 0xf8, 0xdb, 0xdc, 0xec, 0x71,
        0xf2, 0xdb, 0x0b, 0x8e, 0xfc, 0x1a, 0x7e, 0x79,
        0xb1, 0x1b, 0x0d, 0xfc, 0x70, 0xec, 0x85, 0xc2,
        0xc5, 0xba, 0xb9, 0x69, 0x3f, 0x88, 0xbc, 0xcb
    };
    static const unsigned char expected_q[]= {
        0x99, 0xb6, 0xa0, 0xee, 0xb3, 0xa6, 0x99, 0x1a,
        0xb6, 0x67, 0x8d, 0xc1, 0x2b, 0x9b, 0xce, 0x2b,
        0x01, 0x72, 0x5a, 0x65, 0x76, 0x3d, 0x93, 0x69,
        0xe2, 0x56, 0xae, 0xd7
    };
    static const unsigned char expected_g[]= {
        0x63, 0xf8, 0xb6, 0xee, 0x2a, 0x27, 0xaf, 0x4f,
        0x4c, 0xf6, 0x08, 0x28, 0x87, 0x4a, 0xe7, 0x1f,
        0x45, 0x46, 0x27, 0x52, 0x3b, 0x7f, 0x6f, 0xd2,
        0x29, 0xcb, 0xe8, 0x11, 0x19, 0x25, 0x35, 0x76,
        0x99, 0xcb, 0x4f, 0x1b, 0xe0, 0xed, 0x32, 0x9e,
        0x05, 0xb5, 0xbe, 0xd7, 0xf6, 0x5a, 0xb2, 0xf6,
        0x0e, 0x0c, 0x7e, 0xf5, 0xe1, 0x05, 0xfe, 0xda,
        0xaf, 0x0f, 0x27, 0x1e, 0x40, 0x2a, 0xf7, 0xa7,
        0x23, 0x49, 0x2c, 0xd9, 0x1b, 0x0a, 0xbe, 0xff,
        0xc7, 0x7c, 0x7d, 0x60, 0xca, 0xa3, 0x19, 0xc3,
        0xb7, 0xe4, 0x43, 0xb0, 0xf5, 0x75, 0x44, 0x90,
        0x46, 0x47, 0xb1, 0xa6, 0x48, 0x0b, 0x21, 0x8e,
        0xee, 0x75, 0xe6, 0x3d, 0xa7, 0xd3, 0x7b, 0x31,
        0xd1, 0xd2, 0x9d, 0xe2, 0x8a, 0xfc, 0x57, 0xfd,
        0x8a, 0x10, 0x31, 0xeb, 0x87, 0x36, 0x3f, 0x65,
        0x72, 0x23, 0x2c, 0xd3, 0xd6, 0x17, 0xa5, 0x62,
        0x58, 0x65, 0x57, 0x6a, 0xd4, 0xa8, 0xfe, 0xec,
        0x57, 0x76, 0x0c, 0xb1, 0x4c, 0x93, 0xed, 0xb0,
        0xb4, 0xf9, 0x45, 0xb3, 0x3e, 0xdd, 0x47, 0xf1,
        0xfb, 0x7d, 0x25, 0x79, 0x3d, 0xfc, 0xa7, 0x39,
        0x90, 0x68, 0x6a, 0x6b, 0xae, 0xf2, 0x6e, 0x64,
        0x8c, 0xfb, 0xb8, 0xdd, 0x76, 0x4e, 0x4a, 0x69,
        0x8c, 0x97, 0x15, 0x77, 0xb2, 0x67, 0xdc, 0xeb,
        0x4a, 0x40, 0x6b, 0xb9, 0x47, 0x8f, 0xa6, 0xab,
        0x6e, 0x98, 0xc0, 0x97, 0x9a, 0x0c, 0xea, 0x00,
        0xfd, 0x56, 0x1a, 0x74, 0x9a, 0x32, 0x6b, 0xfe,
        0xbd, 0xdf, 0x6c, 0x82, 0x54, 0x53, 0x4d, 0x70,
        0x65, 0xe3, 0x8b, 0x37, 0xb8, 0xe4, 0x70, 0x08,
        0xb7, 0x3b, 0x30, 0x27, 0xaf, 0x1c, 0x77, 0xf3,
        0x62, 0xd4, 0x9a, 0x59, 0xba, 0xd1, 0x6e, 0x89,
        0x5c, 0x34, 0x9a, 0xa1, 0xb7, 0x4f, 0x7d, 0x8c,
        0xdc, 0xbc, 0x74, 0x25, 0x5e, 0xbf, 0x77, 0x46
    };
    int expected_c = 1316;
    int expected_h = 2;

    if (!TEST_ptr(p_in = BN_bin2bn(expected_p, sizeof(expected_p), NULL))
        || !TEST_ptr(q_in = BN_bin2bn(expected_q, sizeof(expected_q), NULL))
        || !TEST_ptr(g_in = BN_bin2bn(expected_g, sizeof(expected_g), NULL)))
        goto end;
    if (!TEST_ptr(pg_ctx = EVP_PKEY_CTX_new_from_name(NULL, "DSA", NULL))
        || !TEST_int_gt(EVP_PKEY_paramgen_init(pg_ctx), 0)
        || !TEST_ptr(EVP_PKEY_CTX_gettable_params(pg_ctx))
        || !TEST_ptr(settables = EVP_PKEY_CTX_settable_params(pg_ctx))
        || !TEST_ptr(OSSL_PARAM_locate_const(settables,
                                             OSSL_PKEY_PARAM_FFC_PBITS))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_type(pg_ctx, "fips186_4"))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_bits(pg_ctx, 2048))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_q_bits(pg_ctx, 224))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_seed(pg_ctx, seed_data,
                                                         sizeof(seed_data)))
        || !TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_md_props(pg_ctx, "SHA256",
                                                             ""))
        || !TEST_int_gt(EVP_PKEY_generate(pg_ctx, &param_key), 0)
        || !TEST_ptr(kg_ctx = EVP_PKEY_CTX_new_from_pkey(NULL, param_key, NULL))
        || !TEST_int_gt(EVP_PKEY_keygen_init(kg_ctx), 0)
        || !TEST_int_gt(EVP_PKEY_generate(kg_ctx, &key), 0))
        goto end;

    if (!TEST_true(EVP_PKEY_get_bn_param(key, OSSL_PKEY_PARAM_FFC_P, &p_out))
        || !TEST_BN_eq(p_in, p_out)
        || !TEST_true(EVP_PKEY_get_bn_param(key, OSSL_PKEY_PARAM_FFC_Q, &q_out))
        || !TEST_BN_eq(q_in, q_out)
        || !TEST_true(EVP_PKEY_get_bn_param(key, OSSL_PKEY_PARAM_FFC_G, &g_out))
        || !TEST_BN_eq(g_in, g_out)
        || !TEST_true(EVP_PKEY_get_octet_string_param(
                          key, OSSL_PKEY_PARAM_FFC_SEED, seed_out,
                          sizeof(seed_out), &len))
        || !TEST_mem_eq(seed_out, len, seed_data, sizeof(seed_data))
        || !TEST_true(EVP_PKEY_get_int_param(key, OSSL_PKEY_PARAM_FFC_GINDEX,
                                             &gindex_out))
        || !TEST_int_eq(gindex_out, -1)
        || !TEST_true(EVP_PKEY_get_int_param(key, OSSL_PKEY_PARAM_FFC_H,
                                             &hcount_out))
        || !TEST_int_eq(hcount_out, expected_h)
        || !TEST_true(EVP_PKEY_get_int_param(key,
                                             OSSL_PKEY_PARAM_FFC_PCOUNTER,
                                             &pcount_out))
        || !TEST_int_eq(pcount_out, expected_c)
        || !TEST_false(EVP_PKEY_get_utf8_string_param(key,
                                                      OSSL_PKEY_PARAM_GROUP_NAME,
                                                      group_out,
                                                      sizeof(group_out), &len)))
        goto end;
    ret = 1;
end:
    BN_free(p_in);
    BN_free(q_in);
    BN_free(g_in);
    BN_free(p_out);
    BN_free(q_out);
    BN_free(g_out);
    EVP_PKEY_free(param_key);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(kg_ctx);
    EVP_PKEY_CTX_free(pg_ctx);
    return ret;
}

static int test_dsa_default_paramgen_validate(int i)
{
    int ret;
    EVP_PKEY_CTX *gen_ctx = NULL;
    EVP_PKEY_CTX *check_ctx = NULL;
    EVP_PKEY *params = NULL;

    ret = TEST_ptr(gen_ctx = EVP_PKEY_CTX_new_from_name(NULL, "DSA", NULL))
          && TEST_int_gt(EVP_PKEY_paramgen_init(gen_ctx), 0)
          && (i == 0
              || TEST_true(EVP_PKEY_CTX_set_dsa_paramgen_bits(gen_ctx, 512)))
          && TEST_int_gt(EVP_PKEY_generate(gen_ctx, &params), 0)
          && TEST_ptr(check_ctx = EVP_PKEY_CTX_new_from_pkey(NULL, params, NULL))
          && TEST_int_gt(EVP_PKEY_param_check(check_ctx), 0);

    EVP_PKEY_free(params);
    EVP_PKEY_CTX_free(check_ctx);
    EVP_PKEY_CTX_free(gen_ctx);
    return ret;
}

static int test_dsa_sig_infinite_loop(void)
{
    int ret = 0;
    DSA *dsa = NULL;
    BIGNUM *p = NULL, *q = NULL, *g = NULL, *priv = NULL, *pub = NULL, *priv2 = NULL;
    BIGNUM *badq = NULL, *badpriv = NULL;
    const unsigned char msg[] = { 0x00 };
    unsigned int signature_len0;
    unsigned int signature_len;
    unsigned char signature[64];

    static unsigned char out_priv[] = {
        0x17, 0x00, 0xb2, 0x8d, 0xcb, 0x24, 0xc9, 0x98,
        0xd0, 0x7f, 0x1f, 0x83, 0x1a, 0xa1, 0xc4, 0xa4,
        0xf8, 0x0f, 0x7f, 0x12
    };
    static unsigned char out_pub[] = {
        0x04, 0x72, 0xee, 0x8d, 0xaa, 0x4d, 0x89, 0x60,
        0x0e, 0xb2, 0xd4, 0x38, 0x84, 0xa2, 0x2a, 0x60,
        0x5f, 0x67, 0xd7, 0x9e, 0x24, 0xdd, 0xe8, 0x50,
        0xf2, 0x23, 0x71, 0x55, 0x53, 0x94, 0x0d, 0x6b,
        0x2e, 0xcd, 0x30, 0xda, 0x6f, 0x1e, 0x2c, 0xcf,
        0x59, 0xbe, 0x05, 0x6c, 0x07, 0x0e, 0xc6, 0x38,
        0x05, 0xcb, 0x0c, 0x44, 0x0a, 0x08, 0x13, 0xb6,
        0x0f, 0x14, 0xde, 0x4a, 0xf6, 0xed, 0x4e, 0xc3
    };
    if (!TEST_ptr(p = BN_bin2bn(out_p, sizeof(out_p), NULL))
        || !TEST_ptr(q = BN_bin2bn(out_q, sizeof(out_q), NULL))
        || !TEST_ptr(g = BN_bin2bn(out_g, sizeof(out_g), NULL))
        || !TEST_ptr(pub = BN_bin2bn(out_pub, sizeof(out_pub), NULL))
        || !TEST_ptr(priv = BN_bin2bn(out_priv, sizeof(out_priv), NULL))
        || !TEST_ptr(priv2 = BN_dup(priv))
        || !TEST_ptr(badq = BN_new())
        || !TEST_true(BN_set_word(badq, 1))
        || !TEST_ptr(badpriv = BN_new())
        || !TEST_true(BN_set_word(badpriv, 0))
        || !TEST_ptr(dsa = DSA_new()))
        goto err;

    if (!TEST_true(DSA_set0_pqg(dsa, p, q, g)))
        goto err;
    p = q = g = NULL;

    if (!TEST_true(DSA_set0_key(dsa, pub, priv)))
        goto err;
    pub = priv = NULL;

    if (!TEST_int_le(DSA_size(dsa), sizeof(signature)))
        goto err;

    /* Test passing signature as NULL */
    if (!TEST_true(DSA_sign(0, msg, sizeof(msg), NULL, &signature_len0, dsa))
        || !TEST_int_gt(signature_len0, 0))
        goto err;

    if (!TEST_true(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa))
        || !TEST_int_gt(signature_len, 0)
        || !TEST_int_le(signature_len, signature_len0))
        goto err;

    /* Test using a private key of zero fails - this causes an infinite loop without the retry test */
    if (!TEST_true(DSA_set0_key(dsa, NULL, badpriv)))
        goto err;
    badpriv = NULL;
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    /* Restore private and set a bad q - this caused an infinite loop in the setup */
    if (!TEST_true(DSA_set0_key(dsa, NULL, priv2)))
        goto err;
    priv2 = NULL;
    if (!TEST_true(DSA_set0_pqg(dsa, NULL, badq, NULL)))
        goto err;
    badq = NULL;
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    ret = 1;
err:
    BN_free(badq);
    BN_free(badpriv);
    BN_free(pub);
    BN_free(priv);
    BN_free(priv2);
    BN_free(g);
    BN_free(q);
    BN_free(p);
    DSA_free(dsa);
    return ret;
}

static int test_dsa_sig_neg_param(void)
{
    int ret = 0, setpqg = 0;
    DSA *dsa = NULL;
    BIGNUM *p = NULL, *q = NULL, *g = NULL, *priv = NULL, *pub = NULL;
    const unsigned char msg[] = { 0x00 };
    unsigned int signature_len;
    unsigned char signature[64];

    static unsigned char out_priv[] = {
        0x17, 0x00, 0xb2, 0x8d, 0xcb, 0x24, 0xc9, 0x98,
        0xd0, 0x7f, 0x1f, 0x83, 0x1a, 0xa1, 0xc4, 0xa4,
        0xf8, 0x0f, 0x7f, 0x12
    };
    static unsigned char out_pub[] = {
        0x04, 0x72, 0xee, 0x8d, 0xaa, 0x4d, 0x89, 0x60,
        0x0e, 0xb2, 0xd4, 0x38, 0x84, 0xa2, 0x2a, 0x60,
        0x5f, 0x67, 0xd7, 0x9e, 0x24, 0xdd, 0xe8, 0x50,
        0xf2, 0x23, 0x71, 0x55, 0x53, 0x94, 0x0d, 0x6b,
        0x2e, 0xcd, 0x30, 0xda, 0x6f, 0x1e, 0x2c, 0xcf,
        0x59, 0xbe, 0x05, 0x6c, 0x07, 0x0e, 0xc6, 0x38,
        0x05, 0xcb, 0x0c, 0x44, 0x0a, 0x08, 0x13, 0xb6,
        0x0f, 0x14, 0xde, 0x4a, 0xf6, 0xed, 0x4e, 0xc3
    };
    if (!TEST_ptr(p = BN_bin2bn(out_p, sizeof(out_p), NULL))
        || !TEST_ptr(q = BN_bin2bn(out_q, sizeof(out_q), NULL))
        || !TEST_ptr(g = BN_bin2bn(out_g, sizeof(out_g), NULL))
        || !TEST_ptr(pub = BN_bin2bn(out_pub, sizeof(out_pub), NULL))
        || !TEST_ptr(priv = BN_bin2bn(out_priv, sizeof(out_priv), NULL))
        || !TEST_ptr(dsa = DSA_new()))
        goto err;

    if (!TEST_true(DSA_set0_pqg(dsa, p, q, g)))
        goto err;
    setpqg = 1;

    if (!TEST_true(DSA_set0_key(dsa, pub, priv)))
        goto err;
    pub = priv = NULL;

    BN_set_negative(p, 1);
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    BN_set_negative(p, 0);
    BN_set_negative(q, 1);
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    BN_set_negative(q, 0);
    BN_set_negative(g, 1);
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    BN_set_negative(p, 1);
    BN_set_negative(q, 1);
    BN_set_negative(g, 1);
    if (!TEST_false(DSA_sign(0, msg, sizeof(msg), signature, &signature_len, dsa)))
        goto err;

    ret = 1;
err:
    BN_free(pub);
    BN_free(priv);

    if (setpqg == 0) {
        BN_free(g);
        BN_free(q);
        BN_free(p);
    }
    DSA_free(dsa);
    return ret;
}

#endif /* OPENSSL_NO_DSA */

int setup_tests(void)
{
#ifndef OPENSSL_NO_DSA
    ADD_TEST(dsa_test);
    ADD_TEST(dsa_keygen_test);
    ADD_TEST(test_dsa_sig_infinite_loop);
    ADD_TEST(test_dsa_sig_neg_param);
    ADD_ALL_TESTS(test_dsa_default_paramgen_validate, 2);
#endif
    return 1;
}
