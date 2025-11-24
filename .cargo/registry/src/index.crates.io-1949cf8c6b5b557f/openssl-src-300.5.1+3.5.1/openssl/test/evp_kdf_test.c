/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2018-2020, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Tests of the EVP_KDF_CTX APIs */

#include <stdio.h>
#include <string.h>

#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include "internal/numbers.h"
#include "testutil.h"


static EVP_KDF_CTX *get_kdfbyname_libctx(OSSL_LIB_CTX *libctx, const char *name)
{
    EVP_KDF *kdf = EVP_KDF_fetch(libctx, name, NULL);
    EVP_KDF_CTX *kctx = EVP_KDF_CTX_new(kdf);

    EVP_KDF_free(kdf);
    return kctx;
}

static EVP_KDF_CTX *get_kdfbyname(const char *name)
{
    return get_kdfbyname_libctx(NULL, name);
}

static OSSL_PARAM *construct_tls1_prf_params(const char *digest, const char *secret,
    const char *seed)
{
    OSSL_PARAM *params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 4);
    OSSL_PARAM *p = params;

    if (params == NULL)
        return NULL;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)digest, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SECRET,
                                             (unsigned char *)secret,
                                             strlen(secret));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SEED,
                                             (unsigned char *)seed,
                                             strlen(seed));
    *p = OSSL_PARAM_construct_end();

    return params;
}

static int test_kdf_tls1_prf(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;
    static const unsigned char expected[sizeof(out)] = {
        0x8e, 0x4d, 0x93, 0x25, 0x30, 0xd7, 0x65, 0xa0,
        0xaa, 0xe9, 0x74, 0xc3, 0x04, 0x73, 0x5e, 0xcc
    };

    params = construct_tls1_prf_params("sha256", "secret", "seed");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_invalid_digest(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("blah", "secret", "seed");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_false(EVP_KDF_CTX_set_params(kctx, params));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_zero_output_size(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("sha256", "secret", "seed");

    /* Negative test - derive should fail */
    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        && TEST_int_eq(EVP_KDF_derive(kctx, out, 0, NULL), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_empty_secret(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("sha256", "", "seed");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_1byte_secret(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("sha256", "1", "seed");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_empty_seed(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("sha256", "secret", "");

    /* Negative test - derive should fail */
    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        && TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_tls1_prf_1byte_seed(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[16];
    OSSL_PARAM *params;

    params = construct_tls1_prf_params("sha256", "secret", "1");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_TLS1_PRF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static OSSL_PARAM *construct_hkdf_params(char *digest, char *key,
    size_t keylen, char *salt, char *info)
{
    OSSL_PARAM *params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 5);
    OSSL_PARAM *p = params;

    if (params == NULL)
        return NULL;

    if (digest != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                                digest, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             salt, strlen(salt));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             (unsigned char *)key, keylen);
    if (info != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO,
                                                 info, strlen(info));
    else
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MODE,
                                                "EXTRACT_ONLY", 0);
    *p = OSSL_PARAM_construct_end();

    return params;
}

static int test_kdf_hkdf(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[10];
    OSSL_PARAM *params;
    static const unsigned char expected[sizeof(out)] = {
        0x2a, 0xc4, 0x36, 0x9f, 0x52, 0x59, 0x96, 0xf8, 0xde, 0x13
    };

    params = construct_hkdf_params("sha256", "secret", 6, "salt", "label");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int do_kdf_hkdf_gettables(int expand_only, int has_digest)
{
    int ret = 0;
    size_t sz = 0;
    OSSL_PARAM *params;
    OSSL_PARAM params_get[2];
    const OSSL_PARAM *gettables, *p;
    EVP_KDF_CTX *kctx = NULL;

    if (!TEST_ptr(params = construct_hkdf_params(
                                                 has_digest ? "sha256" : NULL,
                                                 "secret", 6, "salt",
                                                 expand_only ? NULL : "label"))
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params)))
        goto err;

    /* Check OSSL_KDF_PARAM_SIZE is gettable */
    if (!TEST_ptr(gettables = EVP_KDF_CTX_gettable_params(kctx))
        || !TEST_ptr(p = OSSL_PARAM_locate_const(gettables, OSSL_KDF_PARAM_SIZE)))
        goto err;

    /* Get OSSL_KDF_PARAM_SIZE as a size_t */
    params_get[0] = OSSL_PARAM_construct_size_t(OSSL_KDF_PARAM_SIZE, &sz);
    params_get[1] = OSSL_PARAM_construct_end();
    if (has_digest) {
        if (!TEST_int_eq(EVP_KDF_CTX_get_params(kctx, params_get), 1)
            || !TEST_size_t_eq(sz, expand_only ? SHA256_DIGEST_LENGTH : SIZE_MAX))
            goto err;
    } else {
        if (!TEST_int_eq(EVP_KDF_CTX_get_params(kctx, params_get), 0))
            goto err;
    }

    /* Get params returns 1 if an unsupported parameter is requested */
    params_get[0] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_CTX_get_params(kctx, params_get), 1))
        goto err;
    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_hkdf_gettables(void)
{
    return do_kdf_hkdf_gettables(0, 1);
}

static int test_kdf_hkdf_gettables_expandonly(void)
{
    return do_kdf_hkdf_gettables(1, 1);
}

static int test_kdf_hkdf_gettables_no_digest(void)
{
    return do_kdf_hkdf_gettables(1, 0);
}

static int test_kdf_hkdf_invalid_digest(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM *params;

    params = construct_hkdf_params("blah", "secret", 6, "salt", "label");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_false(EVP_KDF_CTX_set_params(kctx, params));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_hkdf_derive_set_params_fail(void)
{
    int ret = 0, i = 0;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[2];
    unsigned char out[10];

    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF)))
        goto end;
    /*
     * Set the wrong type for the digest so that it causes a failure
     * inside kdf_hkdf_derive() when kdf_hkdf_set_ctx_params() is called
     */
    params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_DIGEST, &i);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), params), 0))
        goto end;
    ret = 1;
end:
    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_hkdf_set_invalid_mode(void)
{
    int ret = 0, bad_mode = 100;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[2];

    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF)))
        goto end;
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MODE,
                                                 "BADMODE", 0);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 0))
        goto end;

    params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_MODE, &bad_mode);
    if (!TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 0))
        goto end;

    ret = 1;
end:
    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int do_kdf_hkdf_set_invalid_param(const char *key, int type)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[2];
    unsigned char buf[2];

    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF)))
        goto end;
    /* Set the wrong type for the key so that it causes a failure */
    if (type == OSSL_PARAM_UTF8_STRING)
        params[0] = OSSL_PARAM_construct_utf8_string(key, "BAD", 0);
    else
        params[0] = OSSL_PARAM_construct_octet_string(key, buf, sizeof(buf));
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 0))
        goto end;

    ret = 1;
end:
    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_hkdf_set_ctx_param_fail(void)
{
    return do_kdf_hkdf_set_invalid_param(OSSL_KDF_PARAM_MODE,
                                         OSSL_PARAM_OCTET_STRING)
           && do_kdf_hkdf_set_invalid_param(OSSL_KDF_PARAM_KEY,
                                            OSSL_PARAM_UTF8_STRING)
           && do_kdf_hkdf_set_invalid_param(OSSL_KDF_PARAM_SALT,
                                            OSSL_PARAM_UTF8_STRING)
           && do_kdf_hkdf_set_invalid_param(OSSL_KDF_PARAM_INFO,
                                            OSSL_PARAM_UTF8_STRING);
}

static int test_kdf_hkdf_zero_output_size(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[10];
    OSSL_PARAM *params;

    params = construct_hkdf_params("sha256", "secret", 6, "salt", "label");

    /* Negative test - derive should fail */
    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        && TEST_int_eq(EVP_KDF_derive(kctx, out, 0, NULL), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_hkdf_empty_key(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[10];
    OSSL_PARAM *params;

    params = construct_hkdf_params("sha256", "", 0, "salt", "label");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_hkdf_1byte_key(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[10];
    OSSL_PARAM *params;

    params = construct_hkdf_params("sha256", "1", 1, "salt", "label");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_hkdf_empty_salt(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[10];
    OSSL_PARAM *params;

    params = construct_hkdf_params("sha256", "secret", 6, "", "label");

    ret = TEST_ptr(params)
        && TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static OSSL_PARAM *construct_pbkdf1_params(char *pass, char *digest, char *salt,
    unsigned int *iter)
{
    OSSL_PARAM *params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 5);
    OSSL_PARAM *p = params;

    if (params == NULL)
        return NULL;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (unsigned char *)pass, strlen(pass));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (unsigned char *)salt, strlen(salt));
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_ITER, iter);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                             digest, 0);
    *p = OSSL_PARAM_construct_end();

    return params;
}

static int test_kdf_pbkdf1(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    unsigned int iterations = 4096;
    OSSL_LIB_CTX *libctx = NULL;
    OSSL_PARAM *params = NULL;
    OSSL_PROVIDER *legacyprov = NULL;
    OSSL_PROVIDER *defprov = NULL;
    const unsigned char expected[sizeof(out)] = {
        0xfb, 0x83, 0x4d, 0x36, 0x6d, 0xbc, 0x53, 0x87, 0x35, 0x1b, 0x34, 0x75,
        0x95, 0x88, 0x32, 0x4f, 0x3e, 0x82, 0x81, 0x01, 0x21, 0x93, 0x64, 0x00,
        0xcc
    };

    if (!TEST_ptr(libctx = OSSL_LIB_CTX_new()))
        goto err;

    /* PBKDF1 only available in the legacy provider */
    legacyprov = OSSL_PROVIDER_load(libctx, "legacy");
    if (legacyprov == NULL) {
        OSSL_LIB_CTX_free(libctx);
        return TEST_skip("PBKDF1 only available in legacy provider");
    }

    if (!TEST_ptr(defprov = OSSL_PROVIDER_load(libctx, "default")))
        goto err;

    params = construct_pbkdf1_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname_libctx(libctx, OSSL_KDF_NAME_PBKDF1))
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        || !TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0)
        || !TEST_mem_eq(out, sizeof(out), expected, sizeof(expected)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    OSSL_PROVIDER_unload(defprov);
    OSSL_PROVIDER_unload(legacyprov);
    OSSL_LIB_CTX_free(libctx);
    return ret;
}

static int test_kdf_pbkdf1_key_too_long(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[EVP_MAX_MD_SIZE + 1];
    unsigned int iterations = 4096;
    OSSL_LIB_CTX *libctx = NULL;
    OSSL_PARAM *params = NULL;
    OSSL_PROVIDER *legacyprov = NULL;
    OSSL_PROVIDER *defprov = NULL;

    if (!TEST_ptr(libctx = OSSL_LIB_CTX_new()))
        goto err;

    /* PBKDF1 only available in the legacy provider */
    legacyprov = OSSL_PROVIDER_load(libctx, "legacy");
    if (legacyprov == NULL) {
        OSSL_LIB_CTX_free(libctx);
        return TEST_skip("PBKDF1 only available in legacy provider");
    }

    if (!TEST_ptr(defprov = OSSL_PROVIDER_load(libctx, "default")))
        goto err;

    params = construct_pbkdf1_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations);

    /*
     * This is the same test sequence as test_kdf_pbkdf1, but we expect
     * failure here as the requested key size is longer than the digest
     * can provide
     */
    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname_libctx(libctx, OSSL_KDF_NAME_PBKDF1))
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        || !TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    OSSL_PROVIDER_unload(defprov);
    OSSL_PROVIDER_unload(legacyprov);
    OSSL_LIB_CTX_free(libctx);
    return ret;
}

static OSSL_PARAM *construct_pbkdf2_params(char *pass, char *digest, char *salt,
    unsigned int *iter, int *mode)
{
    OSSL_PARAM *params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 6);
    OSSL_PARAM *p = params;

    if (params == NULL)
        return NULL;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (unsigned char *)pass, strlen(pass));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (unsigned char *)salt, strlen(salt));
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_ITER, iter);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                             digest, 0);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_PKCS5, mode);
    *p = OSSL_PARAM_construct_end();

    return params;
}

static int test_kdf_pbkdf2(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    unsigned int iterations = 4096;
    int mode = 0;
    OSSL_PARAM *params;
    const unsigned char expected[sizeof(out)] = {
        0x34, 0x8c, 0x89, 0xdb, 0xcb, 0xd3, 0x2b, 0x2f,
        0x32, 0xd8, 0x14, 0xb8, 0x11, 0x6e, 0x84, 0xcf,
        0x2b, 0x17, 0x34, 0x7e, 0xbc, 0x18, 0x00, 0x18,
        0x1c
    };

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        || !TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        || !TEST_mem_eq(out, sizeof(out), expected, sizeof(expected)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_small_output(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    unsigned int iterations = 4096;
    int mode = 0;
    OSSL_PARAM *params;

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        /* A key length that is too small should fail */
        || !TEST_int_eq(EVP_KDF_derive(kctx, out, 112 / 8 - 1, NULL), 0))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_large_output(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    size_t len = 0;
    unsigned int iterations = 4096;
    int mode = 0;
    OSSL_PARAM *params;

    if (sizeof(len) > 32)
        len = SIZE_MAX;

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* A key length that is too large should fail */
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        || (len != 0 && !TEST_int_eq(EVP_KDF_derive(kctx, out, len, NULL), 0)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_small_salt(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned int iterations = 4096;
    int mode = 0;
    OSSL_PARAM *params;

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALT",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* A salt that is too small should fail */
        || !TEST_false(EVP_KDF_CTX_set_params(kctx, params)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_small_iterations(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned int iterations = 1;
    int mode = 0;
    OSSL_PARAM *params;

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* An iteration count that is too small should fail */
        || !TEST_false(EVP_KDF_CTX_set_params(kctx, params)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_small_salt_pkcs5(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    unsigned int iterations = 4096;
    int mode = 1;
    OSSL_PARAM *params;
    OSSL_PARAM mode_params[2];

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALT",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* A salt that is too small should pass in pkcs5 mode */
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        || !TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    mode = 0;
    mode_params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_PKCS5, &mode);
    mode_params[1] = OSSL_PARAM_construct_end();

    /* If the "pkcs5" mode is disabled then the derive will now fail */
    if (!TEST_true(EVP_KDF_CTX_set_params(kctx, mode_params))
        || !TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_small_iterations_pkcs5(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char out[25];
    unsigned int iterations = 1;
    int mode = 1;
    OSSL_PARAM *params;
    OSSL_PARAM mode_params[2];

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "sha256",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* An iteration count that is too small will pass in pkcs5 mode */
        || !TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        || !TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    mode = 0;
    mode_params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_PKCS5, &mode);
    mode_params[1] = OSSL_PARAM_construct_end();

    /* If the "pkcs5" mode is disabled then the derive will now fail */
    if (!TEST_true(EVP_KDF_CTX_set_params(kctx, mode_params))
        || !TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_pbkdf2_invalid_digest(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned int iterations = 4096;
    int mode = 0;
    OSSL_PARAM *params;

    params = construct_pbkdf2_params("passwordPASSWORDpassword", "blah",
                                     "saltSALTsaltSALTsaltSALTsaltSALTsalt",
                                     &iterations, &mode);

    if (!TEST_ptr(params)
        || !TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_PBKDF2))
        /* Unknown digest should fail */
        || !TEST_false(EVP_KDF_CTX_set_params(kctx, params)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

#ifndef OPENSSL_NO_SCRYPT
static int test_kdf_scrypt(void)
{
    int i, ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7], *p = params;
    unsigned char out[64];
    unsigned int nu = 1024, ru = 8, pu = 16, maxmem = 16;
    static const unsigned char expected[sizeof(out)] = {
        0xfd, 0xba, 0xbe, 0x1c, 0x9d, 0x34, 0x72, 0x00,
        0x78, 0x56, 0xe7, 0x19, 0x0d, 0x01, 0xe9, 0xfe,
        0x7c, 0x6a, 0xd7, 0xcb, 0xc8, 0x23, 0x78, 0x30,
        0xe7, 0x73, 0x76, 0x63, 0x4b, 0x37, 0x31, 0x62,
        0x2e, 0xaf, 0x30, 0xd9, 0x2e, 0x22, 0xa3, 0x88,
        0x6f, 0xf1, 0x09, 0x27, 0x9d, 0x98, 0x30, 0xda,
        0xc7, 0x27, 0xaf, 0xb9, 0x4a, 0x83, 0xee, 0x6d,
        0x83, 0x60, 0xcb, 0xdf, 0xa2, 0xcc, 0x06, 0x40
    };

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (char *)"password", 8);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (char *)"NaCl", 4);
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_SCRYPT_N, &nu);
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_SCRYPT_R, &ru);
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_SCRYPT_P, &pu);
    *p++ = OSSL_PARAM_construct_uint(OSSL_KDF_PARAM_SCRYPT_MAXMEM, &maxmem);
    *p = OSSL_PARAM_construct_end();

    ret = TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_SCRYPT));
    for (i = 0; ret && i < 2; ++i) {
        ret = ret
            && TEST_true(EVP_KDF_CTX_set_params(kctx, params));
        if (i == 0)
            ret = ret
                && TEST_int_le(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0)
                && TEST_true(OSSL_PARAM_set_uint(p - 1, 10 * 1024 * 1024))
                && TEST_true(EVP_KDF_CTX_set_params(kctx, p - 1));
        ret = ret
            && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0)
            && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));
        if (i == 0)
            EVP_KDF_CTX_reset(kctx);
    }

    EVP_KDF_CTX_free(kctx);
    return ret;
}
#endif /* OPENSSL_NO_SCRYPT */

static int test_kdf_ss_hash(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[4], *p = params;
    unsigned char out[14];
    static unsigned char z[] = {
        0x6d,0xbd,0xc2,0x3f,0x04,0x54,0x88,0xe4,0x06,0x27,0x57,0xb0,0x6b,0x9e,
        0xba,0xe1,0x83,0xfc,0x5a,0x59,0x46,0xd8,0x0d,0xb9,0x3f,0xec,0x6f,0x62,
        0xec,0x07,0xe3,0x72,0x7f,0x01,0x26,0xae,0xd1,0x2c,0xe4,0xb2,0x62,0xf4,
        0x7d,0x48,0xd5,0x42,0x87,0xf8,0x1d,0x47,0x4c,0x7c,0x3b,0x18,0x50,0xe9
    };
    static unsigned char other[] = {
        0xa1,0xb2,0xc3,0xd4,0xe5,0x43,0x41,0x56,0x53,0x69,0x64,0x3c,0x83,0x2e,
        0x98,0x49,0xdc,0xdb,0xa7,0x1e,0x9a,0x31,0x39,0xe6,0x06,0xe0,0x95,0xde,
        0x3c,0x26,0x4a,0x66,0xe9,0x8a,0x16,0x58,0x54,0xcd,0x07,0x98,0x9b,0x1e,
        0xe0,0xec,0x3f,0x8d,0xbe
    };
    static const unsigned char expected[sizeof(out)] = {
        0xa4,0x62,0xde,0x16,0xa8,0x9d,0xe8,0x46,0x6e,0xf5,0x46,0x0b,0x47,0xb8
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"sha224", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, z, sizeof(z));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO, other,
                                             sizeof(other));
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_SSKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_x963(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[4], *p = params;
    unsigned char out[1024 / 8];
    /*
     * Test data from https://csrc.nist.gov/CSRC/media/Projects/
     *  Cryptographic-Algorithm-Validation-Program/documents/components/
     *  800-135testvectors/ansx963_2001.zip
     */
    static unsigned char z[] = {
        0x00, 0xaa, 0x5b, 0xb7, 0x9b, 0x33, 0xe3, 0x89, 0xfa, 0x58, 0xce, 0xad,
        0xc0, 0x47, 0x19, 0x7f, 0x14, 0xe7, 0x37, 0x12, 0xf4, 0x52, 0xca, 0xa9,
        0xfc, 0x4c, 0x9a, 0xdb, 0x36, 0x93, 0x48, 0xb8, 0x15, 0x07, 0x39, 0x2f,
        0x1a, 0x86, 0xdd, 0xfd, 0xb7, 0xc4, 0xff, 0x82, 0x31, 0xc4, 0xbd, 0x0f,
        0x44, 0xe4, 0x4a, 0x1b, 0x55, 0xb1, 0x40, 0x47, 0x47, 0xa9, 0xe2, 0xe7,
        0x53, 0xf5, 0x5e, 0xf0, 0x5a, 0x2d
    };
    static unsigned char shared[] = {
        0xe3, 0xb5, 0xb4, 0xc1, 0xb0, 0xd5, 0xcf, 0x1d, 0x2b, 0x3a, 0x2f, 0x99,
        0x37, 0x89, 0x5d, 0x31
    };
    static const unsigned char expected[sizeof(out)] = {
        0x44, 0x63, 0xf8, 0x69, 0xf3, 0xcc, 0x18, 0x76, 0x9b, 0x52, 0x26, 0x4b,
        0x01, 0x12, 0xb5, 0x85, 0x8f, 0x7a, 0xd3, 0x2a, 0x5a, 0x2d, 0x96, 0xd8,
        0xcf, 0xfa, 0xbf, 0x7f, 0xa7, 0x33, 0x63, 0x3d, 0x6e, 0x4d, 0xd2, 0xa5,
        0x99, 0xac, 0xce, 0xb3, 0xea, 0x54, 0xa6, 0x21, 0x7c, 0xe0, 0xb5, 0x0e,
        0xef, 0x4f, 0x6b, 0x40, 0xa5, 0xc3, 0x02, 0x50, 0xa5, 0xa8, 0xee, 0xee,
        0x20, 0x80, 0x02, 0x26, 0x70, 0x89, 0xdb, 0xf3, 0x51, 0xf3, 0xf5, 0x02,
        0x2a, 0xa9, 0x63, 0x8b, 0xf1, 0xee, 0x41, 0x9d, 0xea, 0x9c, 0x4f, 0xf7,
        0x45, 0xa2, 0x5a, 0xc2, 0x7b, 0xda, 0x33, 0xca, 0x08, 0xbd, 0x56, 0xdd,
        0x1a, 0x59, 0xb4, 0x10, 0x6c, 0xf2, 0xdb, 0xbc, 0x0a, 0xb2, 0xaa, 0x8e,
        0x2e, 0xfa, 0x7b, 0x17, 0x90, 0x2d, 0x34, 0x27, 0x69, 0x51, 0xce, 0xcc,
        0xab, 0x87, 0xf9, 0x66, 0x1c, 0x3e, 0x88, 0x16
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"sha512", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, z, sizeof(z));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO, shared,
                                             sizeof(shared));
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_X963KDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

#if !defined(OPENSSL_NO_CMAC) && !defined(OPENSSL_NO_CAMELLIA)
/*
 * KBKDF test vectors from RFC 6803 (Camellia Encryption for Kerberos 5)
 * section 10.
 */
static int test_kdf_kbkdf_6803_128(void)
{
    int ret = 0, i, p;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7];
    static unsigned char input_key[] = {
        0x57, 0xD0, 0x29, 0x72, 0x98, 0xFF, 0xD9, 0xD3,
        0x5D, 0xE5, 0xA4, 0x7F, 0xB4, 0xBD, 0xE2, 0x4B,
    };
    static unsigned char constants[][5] = {
        { 0x00, 0x00, 0x00, 0x02, 0x99 },
        { 0x00, 0x00, 0x00, 0x02, 0xaa },
        { 0x00, 0x00, 0x00, 0x02, 0x55 },
    };
    static unsigned char outputs[][16] = {
        {0xD1, 0x55, 0x77, 0x5A, 0x20, 0x9D, 0x05, 0xF0,
         0x2B, 0x38, 0xD4, 0x2A, 0x38, 0x9E, 0x5A, 0x56},
        {0x64, 0xDF, 0x83, 0xF8, 0x5A, 0x53, 0x2F, 0x17,
         0x57, 0x7D, 0x8C, 0x37, 0x03, 0x57, 0x96, 0xAB},
        {0x3E, 0x4F, 0xBD, 0xF3, 0x0F, 0xB8, 0x25, 0x9C,
         0x42, 0x5C, 0xB6, 0xC9, 0x6F, 0x1F, 0x46, 0x35}
    };
    static unsigned char iv[16] = { 0 };
    unsigned char result[16] = { 0 };

    for (i = 0; i < 3; i++) {
        p = 0;
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_CIPHER, "CAMELLIA-128-CBC", 0);
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_MAC, "CMAC", 0);
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_MODE, "FEEDBACK", 0);
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_KEY, input_key, sizeof(input_key));
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_SALT, constants[i], sizeof(constants[i]));
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_SEED, iv, sizeof(iv));
        params[p] = OSSL_PARAM_construct_end();

        kctx = get_kdfbyname("KBKDF");
        ret = TEST_ptr(kctx)
            && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result),
                                          params), 0)
            && TEST_mem_eq(result, sizeof(result), outputs[i],
                           sizeof(outputs[i]));
        EVP_KDF_CTX_free(kctx);
        if (ret != 1)
            return ret;
    }

    return ret;
}

static int test_kdf_kbkdf_6803_256(void)
{
    int ret = 0, i, p;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7];
    static unsigned char input_key[] = {
        0xB9, 0xD6, 0x82, 0x8B, 0x20, 0x56, 0xB7, 0xBE,
        0x65, 0x6D, 0x88, 0xA1, 0x23, 0xB1, 0xFA, 0xC6,
        0x82, 0x14, 0xAC, 0x2B, 0x72, 0x7E, 0xCF, 0x5F,
        0x69, 0xAF, 0xE0, 0xC4, 0xDF, 0x2A, 0x6D, 0x2C,
    };
    static unsigned char constants[][5] = {
        { 0x00, 0x00, 0x00, 0x02, 0x99 },
        { 0x00, 0x00, 0x00, 0x02, 0xaa },
        { 0x00, 0x00, 0x00, 0x02, 0x55 },
    };
    static unsigned char outputs[][32] = {
        {0xE4, 0x67, 0xF9, 0xA9, 0x55, 0x2B, 0xC7, 0xD3,
         0x15, 0x5A, 0x62, 0x20, 0xAF, 0x9C, 0x19, 0x22,
         0x0E, 0xEE, 0xD4, 0xFF, 0x78, 0xB0, 0xD1, 0xE6,
         0xA1, 0x54, 0x49, 0x91, 0x46, 0x1A, 0x9E, 0x50,
        },
        {0x41, 0x2A, 0xEF, 0xC3, 0x62, 0xA7, 0x28, 0x5F,
         0xC3, 0x96, 0x6C, 0x6A, 0x51, 0x81, 0xE7, 0x60,
         0x5A, 0xE6, 0x75, 0x23, 0x5B, 0x6D, 0x54, 0x9F,
         0xBF, 0xC9, 0xAB, 0x66, 0x30, 0xA4, 0xC6, 0x04,
        },
        {0xFA, 0x62, 0x4F, 0xA0, 0xE5, 0x23, 0x99, 0x3F,
         0xA3, 0x88, 0xAE, 0xFD, 0xC6, 0x7E, 0x67, 0xEB,
         0xCD, 0x8C, 0x08, 0xE8, 0xA0, 0x24, 0x6B, 0x1D,
         0x73, 0xB0, 0xD1, 0xDD, 0x9F, 0xC5, 0x82, 0xB0,
        },
    };
    static unsigned char iv[16] = { 0 };
    unsigned char result[32] = { 0 };

    for (i = 0; i < 3; i++) {
        p = 0;
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_CIPHER, "CAMELLIA-256-CBC", 0);
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_MAC, "CMAC", 0);
        params[p++] = OSSL_PARAM_construct_utf8_string(
            OSSL_KDF_PARAM_MODE, "FEEDBACK", 0);
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_KEY, input_key, sizeof(input_key));
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_SALT, constants[i], sizeof(constants[i]));
        params[p++] = OSSL_PARAM_construct_octet_string(
            OSSL_KDF_PARAM_SEED, iv, sizeof(iv));
        params[p] = OSSL_PARAM_construct_end();

        kctx = get_kdfbyname("KBKDF");
        ret = TEST_ptr(kctx)
            && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result),
                                          params), 0)
            && TEST_mem_eq(result, sizeof(result), outputs[i],
                           sizeof(outputs[i]));
        EVP_KDF_CTX_free(kctx);
        if (ret != 1)
            return ret;
    }

    return ret;
}
#endif

static OSSL_PARAM *construct_kbkdf_params(char *digest, char *mac, unsigned char *key,
    size_t keylen, char *salt, char *info, int *r)
{
    OSSL_PARAM *params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 8);
    OSSL_PARAM *p = params;

    if (params == NULL)
        return NULL;

    *p++ = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_DIGEST, digest, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_MAC, mac, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_MODE, "COUNTER", 0);
    *p++ = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_KEY, key, keylen);
    *p++ = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_SALT, salt, strlen(salt));
    *p++ = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_INFO, info, strlen(info));
    *p++ = OSSL_PARAM_construct_int(
        OSSL_KDF_PARAM_KBKDF_R, r);
    *p = OSSL_PARAM_construct_end();

    return params;
}

static int test_kdf_kbkdf_invalid_digest(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    int r = 32;

    params = construct_kbkdf_params("blah", "HMAC", key, 1, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    /* Negative test case - set_params should fail */
    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_false(EVP_KDF_CTX_set_params(kctx, params));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_kbkdf_invalid_mac(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    int r = 32;

    params = construct_kbkdf_params("sha256", "blah", key, 1, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    /* Negative test case - set_params should fail */
    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_false(EVP_KDF_CTX_set_params(kctx, params));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_kbkdf_invalid_r(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    int r = 31;

    params = construct_kbkdf_params("sha256", "HMAC", key, 1, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    /* Negative test case - derive should fail */
    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_false(EVP_KDF_CTX_set_params(kctx, params));

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}


static int test_kdf_kbkdf_empty_key(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    unsigned char result[32] = { 0 };
    int r = 32;

    params = construct_kbkdf_params("sha256", "HMAC", key, 0, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    /* Negative test case - derive should fail */
    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        && TEST_int_eq(EVP_KDF_derive(kctx, result, sizeof(result), NULL), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_kbkdf_1byte_key(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    unsigned char result[32] = { 0 };
    int r = 32;

    params = construct_kbkdf_params("sha256", "HMAC", key, 1, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result), params), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

static int test_kdf_kbkdf_zero_output_size(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM *params;

    static unsigned char key[] = {0x01};
    unsigned char result[32] = { 0 };
    int r = 32;

    params = construct_kbkdf_params("sha256", "HMAC", key, 1, "prf", "test", &r);
    if (!TEST_ptr(params))
        return 0;

    /* Negative test case - derive should fail */
    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_true(EVP_KDF_CTX_set_params(kctx, params))
        && TEST_int_eq(EVP_KDF_derive(kctx, result, 0, NULL), 0);

    EVP_KDF_CTX_free(kctx);
    OPENSSL_free(params);
    return ret;
}

/* Two test vectors from RFC 8009 (AES Encryption with HMAC-SHA2 for Kerberos
 * 5) appendix A. */
static int test_kdf_kbkdf_8009_prf1(void)
{
    int ret, i = 0;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[6];
    char *label = "prf", *digest = "sha256", *prf_input = "test",
        *mac = "HMAC";
    static unsigned char input_key[] = {
        0x37, 0x05, 0xD9, 0x60, 0x80, 0xC1, 0x77, 0x28,
        0xA0, 0xE8, 0x00, 0xEA, 0xB6, 0xE0, 0xD2, 0x3C,
    };
    static unsigned char output[] = {
        0x9D, 0x18, 0x86, 0x16, 0xF6, 0x38, 0x52, 0xFE,
        0x86, 0x91, 0x5B, 0xB8, 0x40, 0xB4, 0xA8, 0x86,
        0xFF, 0x3E, 0x6B, 0xB0, 0xF8, 0x19, 0xB4, 0x9B,
        0x89, 0x33, 0x93, 0xD3, 0x93, 0x85, 0x42, 0x95,
    };
    unsigned char result[sizeof(output)] = { 0 };

    params[i++] = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_DIGEST, digest, 0);
    params[i++] = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_MAC, mac, 0);
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_KEY, input_key, sizeof(input_key));
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_SALT, label, strlen(label));
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_INFO, prf_input, strlen(prf_input));
    params[i] = OSSL_PARAM_construct_end();

    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result), params), 0)
        && TEST_mem_eq(result, sizeof(result), output, sizeof(output));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_kbkdf_8009_prf2(void)
{
    int ret, i = 0;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[6];
    char *label = "prf", *digest = "sha384", *prf_input = "test",
        *mac = "HMAC";
    static unsigned char input_key[] = {
        0x6D, 0x40, 0x4D, 0x37, 0xFA, 0xF7, 0x9F, 0x9D,
        0xF0, 0xD3, 0x35, 0x68, 0xD3, 0x20, 0x66, 0x98,
        0x00, 0xEB, 0x48, 0x36, 0x47, 0x2E, 0xA8, 0xA0,
        0x26, 0xD1, 0x6B, 0x71, 0x82, 0x46, 0x0C, 0x52,
    };
    static unsigned char output[] = {
        0x98, 0x01, 0xF6, 0x9A, 0x36, 0x8C, 0x2B, 0xF6,
        0x75, 0xE5, 0x95, 0x21, 0xE1, 0x77, 0xD9, 0xA0,
        0x7F, 0x67, 0xEF, 0xE1, 0xCF, 0xDE, 0x8D, 0x3C,
        0x8D, 0x6F, 0x6A, 0x02, 0x56, 0xE3, 0xB1, 0x7D,
        0xB3, 0xC1, 0xB6, 0x2A, 0xD1, 0xB8, 0x55, 0x33,
        0x60, 0xD1, 0x73, 0x67, 0xEB, 0x15, 0x14, 0xD2,
    };
    unsigned char result[sizeof(output)] = { 0 };

    params[i++] = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_DIGEST, digest, 0);
    params[i++] = OSSL_PARAM_construct_utf8_string(
        OSSL_KDF_PARAM_MAC, mac, 0);
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_KEY, input_key, sizeof(input_key));
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_SALT, label, strlen(label));
    params[i++] = OSSL_PARAM_construct_octet_string(
        OSSL_KDF_PARAM_INFO, prf_input, strlen(prf_input));
    params[i] = OSSL_PARAM_construct_end();

    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result), params), 0)
        && TEST_mem_eq(result, sizeof(result), output, sizeof(output));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

#if !defined(OPENSSL_NO_CMAC)
/*
 * Test vector taken from
 * https://csrc.nist.gov/CSRC/media/Projects/
 *    Cryptographic-Algorithm-Validation-Program/documents/KBKDF800-108/CounterMode.zip
 */
static int test_kdf_kbkdf_fixedinfo(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[8], *p = params;
    static char *cipher = "AES128";
    static char *mac = "CMAC";
    static char *mode = "COUNTER";
    int use_l = 0;
    int use_separator = 0;

    static unsigned char input_key[] = {
        0xc1, 0x0b, 0x15, 0x2e, 0x8c, 0x97, 0xb7, 0x7e,
        0x18, 0x70, 0x4e, 0x0f, 0x0b, 0xd3, 0x83, 0x05,
    };
    static unsigned char fixed_input[] = {
        0x98, 0xcd, 0x4c, 0xbb, 0xbe, 0xbe, 0x15, 0xd1,
        0x7d, 0xc8, 0x6e, 0x6d, 0xba, 0xd8, 0x00, 0xa2,
        0xdc, 0xbd, 0x64, 0xf7, 0xc7, 0xad, 0x0e, 0x78,
        0xe9, 0xcf, 0x94, 0xff, 0xdb, 0xa8, 0x9d, 0x03,
        0xe9, 0x7e, 0xad, 0xf6, 0xc4, 0xf7, 0xb8, 0x06,
        0xca, 0xf5, 0x2a, 0xa3, 0x8f, 0x09, 0xd0, 0xeb,
        0x71, 0xd7, 0x1f, 0x49, 0x7b, 0xcc, 0x69, 0x06,
        0xb4, 0x8d, 0x36, 0xc4,

    };
    static unsigned char output[] = {
        0x26, 0xfa, 0xf6, 0x19, 0x08, 0xad, 0x9e, 0xe8,
        0x81, 0xb8, 0x30, 0x5c, 0x22, 0x1d, 0xb5, 0x3f,
    };
    unsigned char result[sizeof(output)] = { 0 };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_CIPHER, cipher, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC, mac, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MODE, mode, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, input_key,
                                             sizeof(input_key));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO,
                                             fixed_input, sizeof(fixed_input));
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_KBKDF_USE_L, &use_l);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_KBKDF_USE_SEPARATOR,
                                    &use_separator);
    *p = OSSL_PARAM_construct_end();

    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result), params), 0)
        && TEST_mem_eq(result, sizeof(result), output, sizeof(output));

    EVP_KDF_CTX_free(kctx);
    return ret;
}
#endif /* OPENSSL_NO_CMAC */

static int test_kdf_kbkdf_kmac(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[5], *p = params;
    static char *mac = "KMAC256";

    static unsigned char input_key[] = {
        0xDD, 0x81, 0xEF, 0xC8, 0x2C, 0xDD, 0xEC, 0x51,
        0xC4, 0x09, 0xBD, 0x8C, 0xCB, 0xAF, 0x94, 0xF6,
        0x5F, 0xFA, 0x7B, 0x92, 0xF1, 0x11, 0xF9, 0x40,
        0x2B, 0x0D, 0x6A, 0xE0, 0x5E, 0x44, 0x92, 0x34,
        0xF0, 0x3B, 0xBA, 0xF5, 0x4F, 0xEF, 0x19, 0x45,
        0xDA
    };
    static unsigned char context[] = {
        0x81, 0xA1, 0xFE, 0x39, 0x91, 0xEE, 0x3F, 0xD3,
        0x90, 0x4E, 0x82, 0xE6, 0x13, 0x20, 0xEC, 0x6B,
        0x6E, 0x14, 0x0B, 0xBA, 0x95, 0x5D, 0x0B, 0x52,
        0x8E, 0x27, 0x67, 0xB3, 0xE0, 0x77, 0x05, 0x15,
        0xBD, 0x78, 0xF6, 0xE8, 0x8A, 0x7D, 0x9B, 0x08,
        0x20, 0x0F, 0xE9, 0x8D, 0xD6, 0x24, 0x67, 0xE2,
        0xCC, 0x6D, 0x42, 0xE6, 0x60, 0x50, 0x20, 0x77,
        0x89, 0x89, 0xB7, 0x2D, 0xF7, 0x5F, 0xE2, 0x79,
        0xDB, 0x58, 0x0B, 0x7B, 0x02, 0xB9, 0xD9, 0xB0,
        0xFA, 0x6B, 0x0B, 0xB6, 0xD4, 0x95, 0xDB, 0x46,
        0x55, 0x5F, 0x12, 0xC3, 0xF0, 0xE0, 0x6E, 0xC8,
        0xF4, 0xF8, 0xA1, 0x64, 0x2E, 0x96, 0x74, 0x2B,
        0xC6, 0xBD, 0x22, 0xB1, 0x6A, 0xBC, 0x41, 0xDF,
        0x30, 0x32, 0xC7, 0xCE, 0x18, 0x14, 0x70, 0x2A,
        0xED, 0xE5, 0xC4, 0x6B, 0x8A, 0xA8, 0x36, 0xFD,
        0x0A, 0x76, 0x38, 0x44, 0x98, 0x0A, 0xE3, 0xC2,
        0x3A, 0x24, 0xCB, 0x45, 0xBF, 0xC9, 0x2C, 0x19,
        0xCB, 0x9D, 0x6C, 0x27, 0xDE, 0x78, 0x3E, 0x2C,
        0x3D, 0x39, 0x6E, 0x11, 0x59, 0xAE, 0x4F, 0x91,
        0x03, 0xE2, 0x7B, 0x97, 0xD6, 0x0C, 0x7D, 0x9D,
        0x5A, 0xA5, 0x47, 0x57, 0x41, 0xAD, 0x64, 0x5B,
        0xF7, 0x1D, 0x1A, 0xDA, 0x3A, 0x39, 0xDF, 0x85,
        0x0D, 0x0F, 0x50, 0x15, 0xA7, 0x3D, 0x68, 0x81,
        0x7B, 0x0D, 0xF2, 0x24, 0x24, 0x23, 0x37, 0xE5,
        0x77, 0xA6, 0x61, 0xBE, 0xFE, 0x4B, 0x3B, 0x8E,
        0x4F, 0x15, 0x4F, 0xC1, 0x30, 0xCB, 0x9E, 0xF5,
        0x06, 0x9F, 0xBB, 0x0E, 0xF2, 0xF4, 0x43, 0xBB,
        0x64, 0x45, 0xA3, 0x7D, 0x3B, 0xB4, 0x70, 0x47,
        0xDF, 0x4A, 0xA5, 0xD9, 0x2F, 0xE6, 0x25, 0xC8,
        0x1D, 0x43, 0x0A, 0xEA, 0xF9, 0xCC, 0xC7, 0x1F,
        0x8A, 0x2D, 0xD8, 0x95, 0x6B, 0x16, 0x30, 0x1D,
        0x80, 0x90, 0xA4, 0x23, 0x14, 0x59, 0xD1, 0x5A,
        0x00, 0x48, 0x8D, 0xF7, 0xEA, 0x29, 0x23, 0xDF,
        0x35, 0x26, 0x25, 0x22, 0x12, 0xC4, 0x4C, 0x09,
        0x69, 0xB8, 0xD6, 0x0C, 0x0E, 0x71, 0x90, 0x6C,
        0x42, 0x90, 0x02, 0x53, 0xC5, 0x5A, 0xEF, 0x42,
        0x66, 0x1D, 0xAF, 0x45, 0xD5, 0x31, 0xD7, 0x61,
        0x3A, 0xE6, 0x06, 0xFB, 0x83, 0x72, 0xAD, 0x82,
        0xE3, 0x6A, 0x7E, 0x03, 0x9B, 0x37, 0x77, 0xAF,
        0x8D, 0x63, 0x28, 0xC2, 0x8A, 0x5E, 0xC6, 0x3B,
        0x22, 0xA8, 0x94, 0xC0, 0x46, 0x2F, 0x73, 0xE7,
        0xBB, 0x72, 0x44, 0x85, 0x20, 0x1D, 0xD0, 0x6A,
        0x52, 0x8C, 0xB1, 0x8B, 0x96, 0x11, 0xEB, 0xFB,
        0xDD, 0xF5, 0x74, 0x49, 0x19, 0x93, 0xD3, 0x7F,
        0x6C, 0x27, 0x19, 0x54, 0xDD, 0x00, 0x0F, 0x95,
        0xF6, 0x14, 0x15, 0x87, 0x32, 0x54, 0xA5, 0x02,
        0xAD, 0x41, 0x55, 0x5E, 0xDD, 0x32, 0x62, 0x3B,
        0xFC, 0x71, 0xC1, 0x56, 0xC4, 0x6A, 0xFC, 0xD0,
        0xF9, 0x77, 0xDA, 0xC5, 0x20, 0x7D, 0xAC, 0xA8,
        0xEB, 0x8F, 0xBE, 0xF9, 0x4D, 0xE8, 0x6D, 0x9E,
        0x4C, 0x39, 0xB3, 0x15, 0x63, 0xCD, 0xF6, 0x46,
        0xEC, 0x3A, 0xD2, 0x89, 0xA9, 0xFA, 0x24, 0xB4,
        0x0E, 0x62, 0x6F, 0x9F, 0xF3, 0xF1, 0x3C, 0x61,
        0x57, 0xB9, 0x2C, 0xD4, 0x78, 0x4F, 0x76, 0xCF,
        0xFB, 0x6A, 0x51, 0xE8, 0x1E, 0x0A, 0x33, 0x69,
        0x16, 0xCD, 0xB7, 0x5C, 0xDF, 0x03, 0x62, 0x17,
        0x63, 0x37, 0x49, 0xC3, 0xB7, 0x68, 0x09, 0x9E,
        0x22, 0xD2, 0x20, 0x96, 0x37, 0x0D, 0x13, 0xA4,
        0x96, 0xB1, 0x8D, 0x0B, 0x12, 0x87, 0xEB, 0x57,
        0x25, 0x27, 0x08, 0xFC, 0x90, 0x5E, 0x33, 0x77,
        0x50, 0x63, 0xE1, 0x8C, 0xF4, 0x0C, 0x80, 0x89,
        0x76, 0x63, 0x70, 0x0A, 0x61, 0x59, 0x90, 0x1F,
        0xC9, 0x47, 0xBA, 0x12, 0x7B, 0xB2, 0x7A, 0x44,
        0xC3, 0x3D, 0xD0, 0x38, 0xF1, 0x7F, 0x02, 0x92
    };
    static unsigned char label[] = {
        0xA5, 0xDE, 0x2A, 0x0A, 0xF0, 0xDA, 0x59, 0x04,
        0xCC, 0xFF, 0x50, 0xD3, 0xA5, 0xD2, 0xDE, 0xA3,
        0x33, 0xC0, 0x27, 0xED, 0xDC, 0x6A, 0x54, 0x54,
        0x95, 0x78, 0x74, 0x0D, 0xE7, 0xB7, 0x92, 0xD6,
        0x64, 0xD5, 0xFB, 0x1F, 0x0F, 0x87, 0xFD, 0x65,
        0x79, 0x8B, 0x81, 0x83, 0x95, 0x40, 0x7A, 0x19,
        0x8D, 0xCA, 0xE0, 0x4A, 0x93, 0xA8
    };
    static unsigned char output[] = {
        0xB5, 0x61, 0xE3, 0x7D, 0x06, 0xD5, 0x34, 0x80,
        0x74, 0x61, 0x16, 0x08, 0x6F, 0x89, 0x6F, 0xB1,
        0x43, 0xAF, 0x61, 0x28, 0x93, 0xD8, 0xDF, 0xF6,
        0xB6, 0x23, 0x43, 0x68, 0xE4, 0x84, 0xF3, 0xED,
        0x50, 0xB6, 0x81, 0x6D, 0x50, 0xF4, 0xAF, 0xF2,
        0xA5, 0x50, 0x7E, 0x25, 0xBF, 0x05, 0xBE, 0xE7,
        0x07, 0xB0, 0x95, 0xC3, 0x04, 0x38, 0xB4, 0xF9,
        0xC1, 0x1E, 0x96, 0x08, 0xF4, 0xC9, 0x05, 0x54,
        0x4A, 0xB6, 0x81, 0x92, 0x5B, 0x34, 0x8A, 0x45,
        0xDD, 0x7D, 0x98, 0x51, 0x1F, 0xD9, 0x90, 0x23,
        0x59, 0x97, 0xA2, 0x4E, 0x43, 0x49, 0xEB, 0x4E,
        0x86, 0xEC, 0x20, 0x3C, 0x31, 0xFF, 0x49, 0x55,
        0x49, 0xF5, 0xF5, 0x16, 0x79, 0xD9, 0x1C, 0x8E,
        0x6E, 0xB3, 0x1C, 0xAF, 0xC8, 0xAB, 0x3A, 0x5A,
        0xCE, 0xB1, 0xBD, 0x59, 0x69, 0xEE, 0xC0, 0x28,
        0x3E, 0x94, 0xD2, 0xCC, 0x91, 0x93, 0x73, 0x6A,
        0xD6, 0xB6, 0xC1, 0x42, 0x97, 0xB1, 0x13, 0xCF,
        0xF9, 0x55, 0x35, 0x50, 0xFC, 0x86, 0x75, 0x98,
        0x9F, 0xFC, 0x96, 0xB1, 0x43, 0x41, 0x8F, 0xFC,
        0x31, 0x09, 0x3B, 0x35, 0x22, 0x7B, 0x01, 0x96,
        0xA7, 0xF0, 0x78, 0x7B, 0x57, 0x00, 0xF2, 0xE5,
        0x92, 0x36, 0xCE, 0x64, 0xFD, 0x65, 0x09, 0xD8,
        0xBC, 0x5C, 0x82, 0x5C, 0x4C, 0x62, 0x5B, 0xCE,
        0x09, 0xB6, 0xCF, 0x4D, 0xAD, 0x8E, 0xDD, 0x96,
        0xB0, 0xCA, 0x52, 0xC1, 0xF4, 0x17, 0x0E, 0x2D,
        0x4E, 0xC3, 0xF9, 0x89, 0x1A, 0x24, 0x3D, 0x01,
        0xC8, 0x05, 0xBF, 0x7D, 0x2A, 0x46, 0xCD, 0x9A,
        0x66, 0xEE, 0x05, 0x78, 0x88, 0x2A, 0xEF, 0x37,
        0x9E, 0x72, 0x55, 0xDA, 0x82, 0x7A, 0x9B, 0xE8,
        0xF7, 0xA6, 0x74, 0xB8, 0x74, 0x39, 0x03, 0xE8,
        0xB9, 0x1F, 0x97, 0x78, 0xB9, 0xD9, 0x37, 0x16,
        0xFD, 0x2F, 0x31, 0xDE, 0xCC, 0x06, 0xD6, 0x5A,
        0xEB, 0xD1, 0xBB, 0x84, 0x30, 0x16, 0x81, 0xB0,
        0x7E, 0x04, 0x8C, 0x06, 0x67, 0xD1, 0x8A, 0x07,
        0x33, 0x76, 0x42, 0x8E, 0x87, 0xAB, 0x90, 0x6F,
        0x08, 0xED, 0x8D, 0xE8, 0xD0, 0x20, 0x00, 0x7E,
        0x3C, 0x4D, 0xA4, 0x40, 0x37, 0x13, 0x0F, 0x00,
        0x0C, 0xB7, 0x26, 0x03, 0x93, 0xD0, 0xBB, 0x08,
        0xD3, 0xCC, 0xA9, 0x28, 0xC2
    };
    unsigned char result[sizeof(output)] = { 0 };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC, mac, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             input_key, sizeof(input_key));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO,
                                             context, sizeof(context));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             label, sizeof(label));
    *p = OSSL_PARAM_construct_end();

    kctx = get_kdfbyname("KBKDF");
    ret = TEST_ptr(kctx)
        && TEST_size_t_eq(EVP_KDF_CTX_get_kdf_size(kctx), SIZE_MAX)
        && TEST_int_gt(EVP_KDF_derive(kctx, result, sizeof(result), params), 0)
        && TEST_mem_eq(result, sizeof(result), output, sizeof(output));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_ss_hmac(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[6], *p = params;
    unsigned char out[16];
    static unsigned char z[] = {
        0xb7,0x4a,0x14,0x9a,0x16,0x15,0x46,0xf8,0xc2,0x0b,0x06,0xac,0x4e,0xd4
    };
    static unsigned char other[] = {
        0x34,0x8a,0x37,0xa2,0x7e,0xf1,0x28,0x2f,0x5f,0x02,0x0d,0xcc
    };
    static unsigned char salt[] = {
        0x36,0x38,0x27,0x1c,0xcd,0x68,0xa2,0x5d,0xc2,0x4e,0xcd,0xdd,0x39,0xef,
        0x3f,0x89
    };
    static const unsigned char expected[sizeof(out)] = {
        0x44,0xf6,0x76,0xe8,0x5c,0x1b,0x1a,0x8b,0xbc,0x3d,0x31,0x92,0x18,0x63,
        0x1c,0xa3
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC,
                                            (char *)OSSL_MAC_NAME_HMAC, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"sha256", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, z, sizeof(z));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO, other,
                                             sizeof(other));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT, salt,
                                             sizeof(salt));
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_SSKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_ss_kmac(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7], *p = params;
    unsigned char out[64];
    size_t mac_size = 20;
    static unsigned char z[] = {
        0xb7,0x4a,0x14,0x9a,0x16,0x15,0x46,0xf8,0xc2,0x0b,0x06,0xac,0x4e,0xd4
    };
    static unsigned char other[] = {
        0x34,0x8a,0x37,0xa2,0x7e,0xf1,0x28,0x2f,0x5f,0x02,0x0d,0xcc
    };
    static unsigned char salt[] = {
        0x36,0x38,0x27,0x1c,0xcd,0x68,0xa2,0x5d,0xc2,0x4e,0xcd,0xdd,0x39,0xef,
        0x3f,0x89
    };
    static const unsigned char expected[sizeof(out)] = {
        0xe9,0xc1,0x84,0x53,0xa0,0x62,0xb5,0x3b,0xdb,0xfc,0xbb,0x5a,0x34,0xbd,
        0xb8,0xe5,0xe7,0x07,0xee,0xbb,0x5d,0xd1,0x34,0x42,0x43,0xd8,0xcf,0xc2,
        0xc2,0xe6,0x33,0x2f,0x91,0xbd,0xa5,0x86,0xf3,0x7d,0xe4,0x8a,0x65,0xd4,
        0xc5,0x14,0xfd,0xef,0xaa,0x1e,0x67,0x54,0xf3,0x73,0xd2,0x38,0xe1,0x95,
        0xae,0x15,0x7e,0x1d,0xe8,0x14,0x98,0x03
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC,
                                            (char *)OSSL_MAC_NAME_KMAC128, 0);
    /* The digest parameter is not needed here and should be ignored */
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"SHA256", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, z, sizeof(z));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO, other,
                                             sizeof(other));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT, salt,
                                             sizeof(salt));
    *p++ = OSSL_PARAM_construct_size_t(OSSL_KDF_PARAM_MAC_SIZE, &mac_size);
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_SSKDF))
        && TEST_size_t_eq(EVP_KDF_CTX_get_kdf_size(kctx), 0)
        && TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 1)
        /* The bug fix for KMAC returning SIZE_MAX was added in 3.0.8 */
        && (fips_provider_version_lt(NULL, 3, 0, 8)
            || TEST_size_t_eq(EVP_KDF_CTX_get_kdf_size(kctx), SIZE_MAX))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_sshkdf(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[6], *p = params;
    char kdftype = EVP_KDF_SSHKDF_TYPE_INITIAL_IV_CLI_TO_SRV;
    unsigned char out[8];
    /* Test data from NIST CAVS 14.1 test vectors */
    static unsigned char key[] = {
        0x00, 0x00, 0x00, 0x81, 0x00, 0x87, 0x5c, 0x55, 0x1c, 0xef, 0x52, 0x6a,
        0x4a, 0x8b, 0xe1, 0xa7, 0xdf, 0x27, 0xe9, 0xed, 0x35, 0x4b, 0xac, 0x9a,
        0xfb, 0x71, 0xf5, 0x3d, 0xba, 0xe9, 0x05, 0x67, 0x9d, 0x14, 0xf9, 0xfa,
        0xf2, 0x46, 0x9c, 0x53, 0x45, 0x7c, 0xf8, 0x0a, 0x36, 0x6b, 0xe2, 0x78,
        0x96, 0x5b, 0xa6, 0x25, 0x52, 0x76, 0xca, 0x2d, 0x9f, 0x4a, 0x97, 0xd2,
        0x71, 0xf7, 0x1e, 0x50, 0xd8, 0xa9, 0xec, 0x46, 0x25, 0x3a, 0x6a, 0x90,
        0x6a, 0xc2, 0xc5, 0xe4, 0xf4, 0x8b, 0x27, 0xa6, 0x3c, 0xe0, 0x8d, 0x80,
        0x39, 0x0a, 0x49, 0x2a, 0xa4, 0x3b, 0xad, 0x9d, 0x88, 0x2c, 0xca, 0xc2,
        0x3d, 0xac, 0x88, 0xbc, 0xad, 0xa4, 0xb4, 0xd4, 0x26, 0xa3, 0x62, 0x08,
        0x3d, 0xab, 0x65, 0x69, 0xc5, 0x4c, 0x22, 0x4d, 0xd2, 0xd8, 0x76, 0x43,
        0xaa, 0x22, 0x76, 0x93, 0xe1, 0x41, 0xad, 0x16, 0x30, 0xce, 0x13, 0x14,
        0x4e
    };
    static unsigned char xcghash[] = {
        0x0e, 0x68, 0x3f, 0xc8, 0xa9, 0xed, 0x7c, 0x2f, 0xf0, 0x2d, 0xef, 0x23,
        0xb2, 0x74, 0x5e, 0xbc, 0x99, 0xb2, 0x67, 0xda, 0xa8, 0x6a, 0x4a, 0xa7,
        0x69, 0x72, 0x39, 0x08, 0x82, 0x53, 0xf6, 0x42
    };
    static unsigned char sessid[] = {
        0x0e, 0x68, 0x3f, 0xc8, 0xa9, 0xed, 0x7c, 0x2f, 0xf0, 0x2d, 0xef, 0x23,
        0xb2, 0x74, 0x5e, 0xbc, 0x99, 0xb2, 0x67, 0xda, 0xa8, 0x6a, 0x4a, 0xa7,
        0x69, 0x72, 0x39, 0x08, 0x82, 0x53, 0xf6, 0x42
    };
    static const unsigned char expected[sizeof(out)] = {
        0x41, 0xff, 0x2e, 0xad, 0x16, 0x83, 0xf1, 0xe6
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"sha256", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, key,
                                             sizeof(key));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SSHKDF_XCGHASH,
                                             xcghash, sizeof(xcghash));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SSHKDF_SESSION_ID,
                                             sessid, sizeof(sessid));
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_SSHKDF_TYPE,
                                            &kdftype, sizeof(kdftype));
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_SSHKDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdfs_same(EVP_KDF *kdf1, EVP_KDF *kdf2)
{
    /* Fast path in case the two are the same algorithm pointer */
    if (kdf1 == kdf2)
        return 1;
    /*
     * Compare their names and providers instead.
     * This is necessary in a non-caching build (or a cache flush during fetch)
     * because without the algorithm in the cache, fetching it a second time
     * will result in a different pointer.
     */
    return TEST_ptr_eq(EVP_KDF_get0_provider(kdf1), EVP_KDF_get0_provider(kdf2))
           && TEST_str_eq(EVP_KDF_get0_name(kdf1), EVP_KDF_get0_name(kdf2));
}

static int test_kdf_get_kdf(void)
{
    EVP_KDF *kdf1 = NULL, *kdf2 = NULL;
    ASN1_OBJECT *obj;
    int ok = 1;

    if (!TEST_ptr(obj = OBJ_nid2obj(NID_id_pbkdf2))
        || !TEST_ptr(kdf1 = EVP_KDF_fetch(NULL, OSSL_KDF_NAME_PBKDF2, NULL))
        || !TEST_ptr(kdf2 = EVP_KDF_fetch(NULL, OBJ_nid2sn(OBJ_obj2nid(obj)),
                                          NULL))
        || !test_kdfs_same(kdf1, kdf2))
        ok = 0;
    EVP_KDF_free(kdf1);
    kdf1 = NULL;
    EVP_KDF_free(kdf2);
    kdf2 = NULL;

    if (!TEST_ptr(kdf1 = EVP_KDF_fetch(NULL, SN_tls1_prf, NULL))
        || !TEST_ptr(kdf2 = EVP_KDF_fetch(NULL, LN_tls1_prf, NULL))
        || !test_kdfs_same(kdf1, kdf2))
        ok = 0;
    /* kdf1 is reused below, so don't free it here */
    EVP_KDF_free(kdf2);
    kdf2 = NULL;

    if (!TEST_ptr(kdf2 = EVP_KDF_fetch(NULL, OBJ_nid2sn(NID_tls1_prf), NULL))
        || !test_kdfs_same(kdf1, kdf2))
        ok = 0;
    EVP_KDF_free(kdf1);
    kdf1 = NULL;
    EVP_KDF_free(kdf2);
    kdf2 = NULL;

    return ok;
}

#if !defined(OPENSSL_NO_CMS) && !defined(OPENSSL_NO_DES)
static int test_kdf_x942_asn1(void)
{
    int ret;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[4], *p = params;
    const char *cek_alg = SN_id_smime_alg_CMS3DESwrap;
    unsigned char out[24];
    /* RFC2631 Section 2.1.6 Test data */
    static unsigned char z[] = {
        0x00,0x01,0x02,0x03,0x04,0x05,0x06,0x07,0x08,0x09,0x0a,0x0b,0x0c,0x0d,
        0x0e,0x0f,0x10,0x11,0x12,0x13
    };
    static const unsigned char expected[sizeof(out)] = {
        0xa0,0x96,0x61,0x39,0x23,0x76,0xf7,0x04,
        0x4d,0x90,0x52,0xa3,0x97,0x88,0x32,0x46,
        0xb6,0x7f,0x5f,0x1e,0xf6,0x3e,0xb5,0xfb
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)"sha1", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, z,
                                             sizeof(z));
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_CEK_ALG,
                                            (char *)cek_alg, 0);
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_X942KDF_ASN1))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}
#endif /* OPENSSL_NO_CMS */

static int test_kdf_krb5kdf(void)
{
    int ret;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[4], *p = params;
    unsigned char out[16];
    static unsigned char key[] = {
        0x42, 0x26, 0x3C, 0x6E, 0x89, 0xF4, 0xFC, 0x28,
        0xB8, 0xDF, 0x68, 0xEE, 0x09, 0x79, 0x9F, 0x15
    };
    static unsigned char constant[] = {
        0x00, 0x00, 0x00, 0x02, 0x99
    };
    static const unsigned char expected[sizeof(out)] = {
        0x34, 0x28, 0x0A, 0x38, 0x2B, 0xC9, 0x27, 0x69,
        0xB2, 0xDA, 0x2F, 0x9E, 0xF0, 0x66, 0x85, 0x4B
    };

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_CIPHER,
                                            (char *)"AES-128-CBC", 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY, key,
                                             sizeof(key));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_CONSTANT,
                                             constant, sizeof(constant));
    *p = OSSL_PARAM_construct_end();

    ret =
        TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_KRB5KDF))
        && TEST_int_gt(EVP_KDF_derive(kctx, out, sizeof(out), params), 0)
        && TEST_mem_eq(out, sizeof(out), expected, sizeof(expected));

    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_hmac_drbg_settables(void)
{
    int ret = 0, i = 0, j = 0;
    EVP_KDF_CTX *kctx = NULL;
    const OSSL_PARAM *settableparams;
    OSSL_PARAM params[5];
    static const unsigned char ent[32] = { 0 };
    unsigned char out[32];
    char digestname[32];
    char macname[32];
    EVP_MD *shake256 = NULL;

    /* Test there are settables */
    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HMACDRBGKDF))
            || !TEST_ptr(settableparams = EVP_KDF_CTX_settable_params(kctx)))
        goto err;

    /* Fail if no params have been set when doing a derive */
    if (!TEST_int_le(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 0))
        goto err;

    /* Fail if we pass the wrong type for params */
    params[1] = OSSL_PARAM_construct_end();
    for (i = 0; settableparams[i].key != NULL; ++i) {
        /* Skip "properties" key since it returns 1 unless the digest is also set */
        if (OPENSSL_strcasecmp(settableparams[i].key,
                               OSSL_KDF_PARAM_PROPERTIES) != 0) {
            TEST_note("Testing set int into %s fails", settableparams[i].key);
            params[0] = OSSL_PARAM_construct_int(settableparams[i].key, &j);
            if (!TEST_int_le(EVP_KDF_CTX_set_params(kctx, params), 0))
                goto err;
        }
    }
    /* Test that we can set values multiple times */
    params[0] = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_HMACDRBG_ENTROPY,
                                                  (char *)ent, sizeof(ent));
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_HMACDRBG_NONCE,
                                                  (char *)ent, sizeof(ent));
    params[2] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_DIGEST, "SHA256",
                                                 0);
    params[3] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_PROPERTIES, "",
                                                 0);
    params[4] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 1))
        goto err;
    if (!TEST_int_eq(EVP_KDF_CTX_set_params(kctx, params), 1))
        goto err;
    /* Test we can retrieve values back */
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_DIGEST,
                                                 digestname, sizeof(digestname));
    params[1] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_MAC,
                                                 macname, sizeof(macname));
    params[2] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_KDF_CTX_get_params(kctx, params), 1)
            || !TEST_mem_eq(digestname, params[0].return_size, "SHA2-256", 8)
            || !TEST_mem_eq(macname, params[1].return_size, "HMAC", 4))
        goto err;

    /* Test the derive */
    if (!TEST_int_eq(EVP_KDF_derive(kctx, out, sizeof(out), NULL), 1))
        goto err;

    /* test that XOF digests are not allowed */
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_DIGEST,
                                                 "shake256", 0);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_le(EVP_KDF_CTX_set_params(kctx, params), 0))
        goto err;

    ret = 1;
err:
    EVP_MD_free(shake256);
    EVP_KDF_CTX_free(kctx);
    return ret;
}

static int test_kdf_hmac_drbg_gettables(void)
{
    int ret = 0, i, j = 0;
    EVP_KDF_CTX *kctx = NULL;
    const OSSL_PARAM *gettableparams;
    OSSL_PARAM params[3];
    char buf[64];

    /* Test there are gettables */
    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_HMACDRBGKDF))
            || !TEST_ptr(gettableparams = EVP_KDF_CTX_gettable_params(kctx)))
        goto err;
    /* Fail if we pass the wrong type for params */
    params[1] = OSSL_PARAM_construct_end();
    for (i = 0; gettableparams[i].key != NULL; ++i) {
        params[0] = OSSL_PARAM_construct_int(gettableparams[i].key, &j);
        if (!TEST_int_le(EVP_KDF_CTX_get_params(kctx, params), 0))
            goto err;
    }
    /* fail to get params if they are not set yet */
    for (i = 0; gettableparams[i].key != NULL; ++i) {
        params[0] = OSSL_PARAM_construct_utf8_string(gettableparams[i].key,
                                                     buf, sizeof(buf));
        if (!TEST_int_le(EVP_KDF_CTX_get_params(kctx, params), 0))
            goto err;
    }
    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    return ret;
}

/* Test that changing the KBKDF algorithm from KMAC to HMAC works correctly */
static int test_kbkdf_mac_change(void)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[9], *p = params;
    /* Test data taken from the evptest corpus */
    int l = 0, sep = 0, r = 8;
    static /* const */ unsigned char key[] = {
        0x3e, 0xdc, 0x6b, 0x5b, 0x8f, 0x7a, 0xad, 0xbd,
        0x71, 0x37, 0x32, 0xb4, 0x82, 0xb8, 0xf9, 0x79,
        0x28, 0x6e, 0x1e, 0xa3, 0xb8, 0xf8, 0xf9, 0x9c,
        0x30, 0xc8, 0x84, 0xcf, 0xe3, 0x34, 0x9b, 0x83
    };
    static /* const */ unsigned char info[] = {
        0x98, 0xe9, 0x98, 0x8b, 0xb4, 0xcc, 0x8b, 0x34,
        0xd7, 0x92, 0x2e, 0x1c, 0x68, 0xad, 0x69, 0x2b,
        0xa2, 0xa1, 0xd9, 0xae, 0x15, 0x14, 0x95, 0x71,
        0x67, 0x5f, 0x17, 0xa7, 0x7a, 0xd4, 0x9e, 0x80,
        0xc8, 0xd2, 0xa8, 0x5e, 0x83, 0x1a, 0x26, 0x44,
        0x5b, 0x1f, 0x0f, 0xf4, 0x4d, 0x70, 0x84, 0xa1,
        0x72, 0x06, 0xb4, 0x89, 0x6c, 0x81, 0x12, 0xda,
        0xad, 0x18, 0x60, 0x5a
    };
    static const unsigned char output[] = {
        0x6c, 0x03, 0x76, 0x52, 0x99, 0x06, 0x74, 0xa0,
        0x78, 0x44, 0x73, 0x2d, 0x0a, 0xd9, 0x85, 0xf9
    };
    unsigned char out[sizeof(output)];

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC,
                                                 OSSL_MAC_NAME_KMAC128, 0);
    params[1] = OSSL_PARAM_construct_end();
    if (!TEST_ptr(kctx = get_kdfbyname(OSSL_KDF_NAME_KBKDF))
            || !TEST_true(EVP_KDF_CTX_set_params(kctx, params)))
        goto err;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MODE, "COUNTER", 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_MAC, "HMAC", 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST, "SHA256", 0);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_KBKDF_USE_L, &l);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_KBKDF_USE_SEPARATOR, &sep);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_KBKDF_R, &r);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             key, sizeof(key));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_INFO,
                                             info, sizeof(info));
    *p = OSSL_PARAM_construct_end();
    if (!TEST_true(EVP_KDF_derive(kctx, out, sizeof(out), params))
            || !TEST_mem_eq(out, sizeof(out), output, sizeof(output)))
        goto err;

    ret = 1;
err:
    EVP_KDF_CTX_free(kctx);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_kdf_pbkdf1);
    ADD_TEST(test_kdf_pbkdf1_key_too_long);
#if !defined(OPENSSL_NO_CMAC) && !defined(OPENSSL_NO_CAMELLIA)
    ADD_TEST(test_kdf_kbkdf_6803_128);
    ADD_TEST(test_kdf_kbkdf_6803_256);
#endif
    ADD_TEST(test_kdf_kbkdf_invalid_digest);
    ADD_TEST(test_kdf_kbkdf_invalid_mac);
    ADD_TEST(test_kdf_kbkdf_invalid_r);
    ADD_TEST(test_kdf_kbkdf_zero_output_size);
    ADD_TEST(test_kdf_kbkdf_empty_key);
    ADD_TEST(test_kdf_kbkdf_1byte_key);
    ADD_TEST(test_kdf_kbkdf_8009_prf1);
    ADD_TEST(test_kdf_kbkdf_8009_prf2);
#if !defined(OPENSSL_NO_CMAC)
    ADD_TEST(test_kdf_kbkdf_fixedinfo);
#endif
    if (fips_provider_version_ge(NULL, 3, 1, 0))
        ADD_TEST(test_kdf_kbkdf_kmac);
    ADD_TEST(test_kdf_get_kdf);
    ADD_TEST(test_kdf_tls1_prf);
    ADD_TEST(test_kdf_tls1_prf_invalid_digest);
    ADD_TEST(test_kdf_tls1_prf_zero_output_size);
    ADD_TEST(test_kdf_tls1_prf_empty_secret);
    ADD_TEST(test_kdf_tls1_prf_1byte_secret);
    ADD_TEST(test_kdf_tls1_prf_empty_seed);
    ADD_TEST(test_kdf_tls1_prf_1byte_seed);
    ADD_TEST(test_kdf_hkdf);
    ADD_TEST(test_kdf_hkdf_invalid_digest);
    ADD_TEST(test_kdf_hkdf_zero_output_size);
    ADD_TEST(test_kdf_hkdf_empty_key);
    ADD_TEST(test_kdf_hkdf_1byte_key);
    ADD_TEST(test_kdf_hkdf_empty_salt);
    ADD_TEST(test_kdf_hkdf_gettables);
    ADD_TEST(test_kdf_hkdf_gettables_expandonly);
    ADD_TEST(test_kdf_hkdf_gettables_no_digest);
    ADD_TEST(test_kdf_hkdf_derive_set_params_fail);
    ADD_TEST(test_kdf_hkdf_set_invalid_mode);
    ADD_TEST(test_kdf_hkdf_set_ctx_param_fail);
    ADD_TEST(test_kdf_pbkdf2);
    ADD_TEST(test_kdf_pbkdf2_small_output);
    ADD_TEST(test_kdf_pbkdf2_large_output);
    ADD_TEST(test_kdf_pbkdf2_small_salt);
    ADD_TEST(test_kdf_pbkdf2_small_iterations);
    ADD_TEST(test_kdf_pbkdf2_small_salt_pkcs5);
    ADD_TEST(test_kdf_pbkdf2_small_iterations_pkcs5);
    ADD_TEST(test_kdf_pbkdf2_invalid_digest);
#ifndef OPENSSL_NO_SCRYPT
    ADD_TEST(test_kdf_scrypt);
#endif
    ADD_TEST(test_kdf_ss_hash);
    ADD_TEST(test_kdf_ss_hmac);
    ADD_TEST(test_kdf_ss_kmac);
    ADD_TEST(test_kdf_sshkdf);
    ADD_TEST(test_kdf_x963);
#if !defined(OPENSSL_NO_CMS) && !defined(OPENSSL_NO_DES)
    ADD_TEST(test_kdf_x942_asn1);
#endif
    ADD_TEST(test_kdf_krb5kdf);
    ADD_TEST(test_kdf_hmac_drbg_settables);
    ADD_TEST(test_kdf_hmac_drbg_gettables);
    ADD_TEST(test_kbkdf_mac_change);
    return 1;
}
