/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/params.h>
#include <openssl/param_build.h>
#include "internal/nelem.h"
#include "testutil.h"

static const OSSL_PARAM params_empty[] = { OSSL_PARAM_END };

static int template_public_single_zero_test(int idx)
{
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL, *params_blt = NULL, *p;
    BIGNUM *zbn = NULL, *zbn_res = NULL;
    int res = 0;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_ptr(zbn = BN_new())
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "zeronumber",
                                             idx == 0 ? zbn : NULL))
        || !TEST_ptr(params_blt = OSSL_PARAM_BLD_to_param(bld)))
        goto err;

    params = params_blt;
    /* Check BN (zero BN becomes unsigned integer) */
    if (!TEST_ptr(p = OSSL_PARAM_locate(params, "zeronumber"))
        || !TEST_str_eq(p->key, "zeronumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &zbn_res))
        || !TEST_BN_eq(zbn_res, zbn))
        goto err;
    res = 1;
err:
    if (params != params_blt)
        OPENSSL_free(params);
    OSSL_PARAM_free(params_blt);
    OSSL_PARAM_BLD_free(bld);
    BN_free(zbn);
    BN_free(zbn_res);
    return res;
}

static int template_private_single_zero_test(void)
{
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL, *params_blt = NULL, *p;
    BIGNUM *zbn = NULL, *zbn_res = NULL;
    int res = 0;

    if (!TEST_ptr(bld = OSSL_PARAM_BLD_new())
        || !TEST_ptr(zbn = BN_secure_new())
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "zeronumber", zbn))
        || !TEST_ptr(params_blt = OSSL_PARAM_BLD_to_param(bld)))
        goto err;

    params = params_blt;
    /* Check BN (zero BN becomes unsigned integer) */
    if (!TEST_ptr(p = OSSL_PARAM_locate(params, "zeronumber"))
        || !TEST_true(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "zeronumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &zbn_res))
        || !TEST_int_eq(BN_get_flags(zbn, BN_FLG_SECURE), BN_FLG_SECURE)
        || !TEST_BN_eq(zbn_res, zbn))
        goto err;
    res = 1;
err:
    if (params != params_blt)
        OPENSSL_free(params);
    OSSL_PARAM_free(params_blt);
    OSSL_PARAM_BLD_free(bld);
    BN_free(zbn);
    BN_free(zbn_res);
    return res;
}

static int template_public_test(int tstid)
{
    OSSL_PARAM_BLD *bld = OSSL_PARAM_BLD_new();
    OSSL_PARAM *params = NULL, *params_blt = NULL, *p1 = NULL, *p;
    BIGNUM *zbn = NULL, *zbn_res = NULL;
    BIGNUM *pbn = NULL, *pbn_res = NULL;
    BIGNUM *nbn = NULL, *nbn_res = NULL;
    int i;
    long int l;
    int32_t i32;
    int64_t i64;
    double d;
    time_t t;
    char *utf = NULL;
    const char *cutf;
    int res = 0;

    if (!TEST_ptr(bld)
        || !TEST_true(OSSL_PARAM_BLD_push_long(bld, "l", 42))
        || !TEST_true(OSSL_PARAM_BLD_push_int32(bld, "i32", 1532))
        || !TEST_true(OSSL_PARAM_BLD_push_int64(bld, "i64", -9999999))
        || !TEST_true(OSSL_PARAM_BLD_push_time_t(bld, "t", 11224))
        || !TEST_true(OSSL_PARAM_BLD_push_double(bld, "d", 1.61803398875))
        || !TEST_ptr(zbn = BN_new())
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "zeronumber", zbn))
        || !TEST_ptr(pbn = BN_new())
        || !TEST_true(BN_set_word(pbn, 1729))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "bignumber", pbn))
        || !TEST_ptr(nbn = BN_secure_new())
        || !TEST_true(BN_set_word(nbn, 1733))
        || !TEST_true((BN_set_negative(nbn, 1), 1))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "negativebignumber", nbn))
        || !TEST_true(OSSL_PARAM_BLD_push_utf8_string(bld, "utf8_s", "foo",
                                                      sizeof("foo")))
        || !TEST_true(OSSL_PARAM_BLD_push_utf8_ptr(bld, "utf8_p", "bar-boom",
                                                   0))
        || !TEST_true(OSSL_PARAM_BLD_push_int(bld, "i", -6))
        || !TEST_ptr(params_blt = OSSL_PARAM_BLD_to_param(bld)))
        goto err;

    switch (tstid) {
    case 0:
        params = params_blt;
        break;
    case 1:
        params = OSSL_PARAM_merge(params_blt, params_empty);
        break;
    case 2:
        params = OSSL_PARAM_dup(params_blt);
        break;
    case 3:
        p1 = OSSL_PARAM_merge(params_blt, params_empty);
        params = OSSL_PARAM_dup(p1);
        break;
    default:
        p1 = OSSL_PARAM_dup(params_blt);
        params = OSSL_PARAM_merge(p1, params_empty);
        break;
    }
    /* Check int */
    if (!TEST_ptr(p = OSSL_PARAM_locate(params, "i"))
        || !TEST_true(OSSL_PARAM_get_int(p, &i))
        || !TEST_str_eq(p->key, "i")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int))
        || !TEST_int_eq(i, -6)
        /* Check int32 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i32"))
        || !TEST_true(OSSL_PARAM_get_int32(p, &i32))
        || !TEST_str_eq(p->key, "i32")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int32_t))
        || !TEST_int_eq((int)i32, 1532)
        /* Check int64 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i64"))
        || !TEST_str_eq(p->key, "i64")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int64_t))
        || !TEST_true(OSSL_PARAM_get_int64(p, &i64))
        || !TEST_long_eq((long)i64, -9999999)
        /* Check long */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "l"))
        || !TEST_str_eq(p->key, "l")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(long int))
        || !TEST_true(OSSL_PARAM_get_long(p, &l))
        || !TEST_long_eq(l, 42)
        /* Check time_t */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "t"))
        || !TEST_str_eq(p->key, "t")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(time_t))
        || !TEST_true(OSSL_PARAM_get_time_t(p, &t))
        || !TEST_time_t_eq(t, 11224)
        /* Check double */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "d"))
        || !TEST_true(OSSL_PARAM_get_double(p, &d))
        || !TEST_str_eq(p->key, "d")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_REAL)
        || !TEST_size_t_eq(p->data_size, sizeof(double))
        || !TEST_double_eq(d, 1.61803398875)
        /* Check UTF8 string */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "utf8_s"))
        || !TEST_str_eq(p->data, "foo")
        || !TEST_true(OSSL_PARAM_get_utf8_string(p, &utf, 0))
        || !TEST_str_eq(utf, "foo")
        /* Check UTF8 pointer */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "utf8_p"))
        || !TEST_true(OSSL_PARAM_get_utf8_ptr(p, &cutf))
        || !TEST_str_eq(cutf, "bar-boom")
        /* Check BN (zero BN becomes unsigned integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "zeronumber"))
        || !TEST_str_eq(p->key, "zeronumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &zbn_res))
        || !TEST_BN_eq(zbn_res, zbn)
        /* Check BN (positive BN becomes unsigned integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "bignumber"))
        || !TEST_str_eq(p->key, "bignumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &pbn_res))
        || !TEST_BN_eq(pbn_res, pbn)
        /* Check BN (negative BN becomes signed integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "negativebignumber"))
        || !TEST_str_eq(p->key, "negativebignumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &nbn_res))
        || !TEST_BN_eq(nbn_res, nbn))
        goto err;
    res = 1;
err:
    OPENSSL_free(p1);
    if (params != params_blt)
        OPENSSL_free(params);
    OSSL_PARAM_free(params_blt);
    OSSL_PARAM_BLD_free(bld);
    OPENSSL_free(utf);
    BN_free(zbn);
    BN_free(zbn_res);
    BN_free(pbn);
    BN_free(pbn_res);
    BN_free(nbn);
    BN_free(nbn_res);
    return res;
}

static int template_private_test(int tstid)
{
    int *data1 = NULL, *data2 = NULL, j;
    const int data1_num = 12;
    const int data1_size = data1_num * sizeof(int);
    const int data2_num = 5;
    const int data2_size = data2_num * sizeof(int);
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL, *params_blt = NULL, *p1 = NULL, *p;
    unsigned int i;
    unsigned long int l;
    uint32_t i32;
    uint64_t i64;
    size_t st;
    BIGNUM *zbn = NULL, *zbn_res = NULL;
    BIGNUM *pbn = NULL, *pbn_res = NULL;
    BIGNUM *nbn = NULL, *nbn_res = NULL;
    int res = 0;

    if (!TEST_ptr(data1 = OPENSSL_secure_malloc(data1_size))
            || !TEST_ptr(data2 = OPENSSL_secure_malloc(data2_size))
            || !TEST_ptr(bld = OSSL_PARAM_BLD_new()))
        goto err;

    for (j = 0; j < data1_num; j++)
        data1[j] = -16 * j;
    for (j = 0; j < data2_num; j++)
        data2[j] = 2 * j;

    if (!TEST_true(OSSL_PARAM_BLD_push_uint(bld, "i", 6))
        || !TEST_true(OSSL_PARAM_BLD_push_ulong(bld, "l", 42))
        || !TEST_true(OSSL_PARAM_BLD_push_uint32(bld, "i32", 1532))
        || !TEST_true(OSSL_PARAM_BLD_push_uint64(bld, "i64", 9999999))
        || !TEST_true(OSSL_PARAM_BLD_push_size_t(bld, "st", 65537))
        || !TEST_ptr(zbn = BN_secure_new())
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "zeronumber", zbn))
        || !TEST_ptr(pbn = BN_secure_new())
        || !TEST_true(BN_set_word(pbn, 1729))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "bignumber", pbn))
        || !TEST_ptr(nbn = BN_secure_new())
        || !TEST_true(BN_set_word(nbn, 1733))
        || !TEST_true((BN_set_negative(nbn, 1), 1))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "negativebignumber", nbn))
        || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld, "oct_s", data1,
                                                       data1_size))
        || !TEST_true(OSSL_PARAM_BLD_push_octet_ptr(bld, "oct_p", data2,
                                                    data2_size))
        || !TEST_ptr(params_blt = OSSL_PARAM_BLD_to_param(bld)))
        goto err;
    switch (tstid) {
    case 0:
        params = params_blt;
        break;
    case 1:
        params = OSSL_PARAM_merge(params_blt, params_empty);
        break;
    case 2:
        params = OSSL_PARAM_dup(params_blt);
        break;
    case 3:
        p1 = OSSL_PARAM_merge(params_blt, params_empty);
        params = OSSL_PARAM_dup(p1);
        break;
    default:
        p1 = OSSL_PARAM_dup(params_blt);
        params = OSSL_PARAM_merge(p1, params_empty);
        break;
    }
    /* Check unsigned int */
    if (!TEST_ptr(p = OSSL_PARAM_locate(params, "i"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_true(OSSL_PARAM_get_uint(p, &i))
        || !TEST_str_eq(p->key, "i")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int))
        || !TEST_uint_eq(i, 6)
        /* Check unsigned int32 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i32"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_true(OSSL_PARAM_get_uint32(p, &i32))
        || !TEST_str_eq(p->key, "i32")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int32_t))
        || !TEST_uint_eq((unsigned int)i32, 1532)
        /* Check unsigned int64 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i64"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "i64")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int64_t))
        || !TEST_true(OSSL_PARAM_get_uint64(p, &i64))
        || !TEST_ulong_eq((unsigned long)i64, 9999999)
        /* Check unsigned long int */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "l"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "l")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(unsigned long int))
        || !TEST_true(OSSL_PARAM_get_ulong(p, &l))
        || !TEST_ulong_eq(l, 42)
        /* Check size_t */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "st"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "st")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(size_t))
        || !TEST_true(OSSL_PARAM_get_size_t(p, &st))
        || !TEST_size_t_eq(st, 65537)
        /* Check octet string */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "oct_s"))
        || !TEST_true(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "oct_s")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_OCTET_STRING)
        || !TEST_mem_eq(p->data, p->data_size, data1, data1_size)
        /* Check octet pointer */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "oct_p"))
        || !TEST_false(CRYPTO_secure_allocated(p->data))
        || !TEST_true(CRYPTO_secure_allocated(*(void **)p->data))
        || !TEST_str_eq(p->key, "oct_p")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_OCTET_PTR)
        || !TEST_mem_eq(*(void **)p->data, p->data_size, data2, data2_size)
        /* Check BN (zero BN becomes unsigned integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "zeronumber"))
        || !TEST_true(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "zeronumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &zbn_res))
        || !TEST_int_eq(BN_get_flags(pbn, BN_FLG_SECURE), BN_FLG_SECURE)
        || !TEST_BN_eq(zbn_res, zbn)
        /* Check BN (positive BN becomes unsigned integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "bignumber"))
        || !TEST_true(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "bignumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &pbn_res))
        || !TEST_int_eq(BN_get_flags(pbn, BN_FLG_SECURE), BN_FLG_SECURE)
        || !TEST_BN_eq(pbn_res, pbn)
        /* Check BN (negative BN becomes signed integer) */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "negativebignumber"))
        || !TEST_true(CRYPTO_secure_allocated(p->data))
        || !TEST_str_eq(p->key, "negativebignumber")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &nbn_res))
        || !TEST_int_eq(BN_get_flags(nbn, BN_FLG_SECURE), BN_FLG_SECURE)
        || !TEST_BN_eq(nbn_res, nbn))
        goto err;
    res = 1;
err:
    OSSL_PARAM_free(p1);
    if (params != params_blt)
        OSSL_PARAM_free(params);
    OSSL_PARAM_free(params_blt);
    OSSL_PARAM_BLD_free(bld);
    OPENSSL_secure_free(data1);
    OPENSSL_secure_free(data2);
    BN_free(zbn);
    BN_free(zbn_res);
    BN_free(pbn);
    BN_free(pbn_res);
    BN_free(nbn);
    BN_free(nbn_res);
    return res;
}

static int builder_limit_test(void)
{
    const int n = 100;
    char names[100][3];
    OSSL_PARAM_BLD *bld = OSSL_PARAM_BLD_new();
    OSSL_PARAM *params = NULL;
    int i, res = 0;

    if (!TEST_ptr(bld))
        goto err;

    for (i = 0; i < n; i++) {
        names[i][0] = 'A' + (i / 26) - 1;
        names[i][1] = 'a' + (i % 26) - 1;
        names[i][2] = '\0';
        if (!TEST_true(OSSL_PARAM_BLD_push_int(bld, names[i], 3 * i + 1)))
            goto err;
    }
    if (!TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld)))
        goto err;
    /* Count the elements in the params array, expecting n */
    for (i = 0; params[i].key != NULL; i++);
    if (!TEST_int_eq(i, n))
        goto err;

    /* Verify that the build, cleared the builder structure */
    OSSL_PARAM_free(params);
    params = NULL;

    if (!TEST_true(OSSL_PARAM_BLD_push_int(bld, "g", 2))
        || !TEST_ptr(params = OSSL_PARAM_BLD_to_param(bld)))
        goto err;
    /* Count the elements in the params array, expecting 1 */
    for (i = 0; params[i].key != NULL; i++);
    if (!TEST_int_eq(i, 1))
        goto err;
    res = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    return res;
}

static int builder_merge_test(void)
{
    static int data1[] = { 2, 3, 5, 7, 11, 15, 17 };
    static unsigned char data2[] = { 2, 4, 6, 8, 10 };
    OSSL_PARAM_BLD *bld = OSSL_PARAM_BLD_new();
    OSSL_PARAM_BLD *bld2 = OSSL_PARAM_BLD_new();
    OSSL_PARAM *params = NULL, *params_blt = NULL, *params2_blt = NULL, *p;
    unsigned int i;
    unsigned long int l;
    uint32_t i32;
    uint64_t i64;
    size_t st;
    BIGNUM *bn_priv = NULL, *bn_priv_res = NULL;
    BIGNUM *bn_pub = NULL, *bn_pub_res = NULL;
    int res = 0;

    if (!TEST_ptr(bld)
        || !TEST_true(OSSL_PARAM_BLD_push_uint(bld, "i", 6))
        || !TEST_true(OSSL_PARAM_BLD_push_ulong(bld, "l", 42))
        || !TEST_true(OSSL_PARAM_BLD_push_uint32(bld, "i32", 1532))
        || !TEST_true(OSSL_PARAM_BLD_push_uint64(bld, "i64", 9999999))
        || !TEST_true(OSSL_PARAM_BLD_push_size_t(bld, "st", 65537))
        || !TEST_ptr(bn_priv = BN_secure_new())
        || !TEST_true(BN_set_word(bn_priv, 1729))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld, "bignumber_priv", bn_priv))
        || !TEST_ptr(params_blt = OSSL_PARAM_BLD_to_param(bld)))
        goto err;

    if (!TEST_ptr(bld2)
        || !TEST_true(OSSL_PARAM_BLD_push_octet_string(bld2, "oct_s", data1,
                                                       sizeof(data1)))
        || !TEST_true(OSSL_PARAM_BLD_push_octet_ptr(bld2, "oct_p", data2,
                                                    sizeof(data2)))
        || !TEST_true(OSSL_PARAM_BLD_push_uint32(bld2, "i32", 99))
        || !TEST_ptr(bn_pub = BN_new())
        || !TEST_true(BN_set_word(bn_pub, 0x42))
        || !TEST_true(OSSL_PARAM_BLD_push_BN(bld2, "bignumber_pub", bn_pub))
        || !TEST_ptr(params2_blt = OSSL_PARAM_BLD_to_param(bld2)))
        goto err;

    if (!TEST_ptr(params = OSSL_PARAM_merge(params_blt, params2_blt)))
        goto err;

    if (!TEST_ptr(p = OSSL_PARAM_locate(params, "i"))
        || !TEST_true(OSSL_PARAM_get_uint(p, &i))
        || !TEST_str_eq(p->key, "i")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int))
        || !TEST_uint_eq(i, 6)
        /* Check unsigned int32 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i32"))
        || !TEST_true(OSSL_PARAM_get_uint32(p, &i32))
        || !TEST_str_eq(p->key, "i32")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int32_t))
        || !TEST_uint_eq((unsigned int)i32, 99)
        /* Check unsigned int64 */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "i64"))
        || !TEST_str_eq(p->key, "i64")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(int64_t))
        || !TEST_true(OSSL_PARAM_get_uint64(p, &i64))
        || !TEST_ulong_eq((unsigned long)i64, 9999999)
        /* Check unsigned long int */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "l"))
        || !TEST_str_eq(p->key, "l")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(unsigned long int))
        || !TEST_true(OSSL_PARAM_get_ulong(p, &l))
        || !TEST_ulong_eq(l, 42)
        /* Check size_t */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "st"))
        || !TEST_str_eq(p->key, "st")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_size_t_eq(p->data_size, sizeof(size_t))
        || !TEST_true(OSSL_PARAM_get_size_t(p, &st))
        || !TEST_size_t_eq(st, 65537)
        /* Check octet string */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "oct_s"))
        || !TEST_str_eq(p->key, "oct_s")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_OCTET_STRING)
        || !TEST_mem_eq(p->data, p->data_size, data1, sizeof(data1))
        /* Check octet pointer */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "oct_p"))
        || !TEST_str_eq(p->key, "oct_p")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_OCTET_PTR)
        || !TEST_mem_eq(*(void **)p->data, p->data_size, data2, sizeof(data2))
        /* Check BN */
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "bignumber_pub"))
        || !TEST_str_eq(p->key, "bignumber_pub")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &bn_pub_res))
        || !TEST_int_eq(BN_cmp(bn_pub_res, bn_pub), 0)
        || !TEST_ptr(p = OSSL_PARAM_locate(params, "bignumber_priv"))
        || !TEST_str_eq(p->key, "bignumber_priv")
        || !TEST_uint_eq(p->data_type, OSSL_PARAM_UNSIGNED_INTEGER)
        || !TEST_true(OSSL_PARAM_get_BN(p, &bn_priv_res))
        || !TEST_int_eq(BN_cmp(bn_priv_res, bn_priv), 0))
        goto err;
    res = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_free(params_blt);
    OSSL_PARAM_free(params2_blt);
    OSSL_PARAM_BLD_free(bld);
    OSSL_PARAM_BLD_free(bld2);
    BN_free(bn_priv);
    BN_free(bn_priv_res);
    BN_free(bn_pub);
    BN_free(bn_pub_res);
    return res;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(template_public_single_zero_test, 2);
    ADD_ALL_TESTS(template_public_test, 5);
    /* Only run the secure memory testing if we have secure memory available */
    if (CRYPTO_secure_malloc_init(1<<16, 16)) {
        ADD_TEST(template_private_single_zero_test);
        ADD_ALL_TESTS(template_private_test, 5);
    }
    ADD_TEST(builder_limit_test);
    ADD_TEST(builder_merge_test);
    return 1;
}
