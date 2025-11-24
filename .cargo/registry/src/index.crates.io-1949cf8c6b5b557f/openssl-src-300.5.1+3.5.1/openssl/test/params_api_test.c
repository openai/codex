/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include "testutil.h"
#include "internal/nelem.h"
#include "internal/endian.h"
#include <openssl/params.h>
#include <openssl/bn.h>

/* The maximum size of the static buffers used to test most things */
#define MAX_LEN 20

static void swap_copy(unsigned char *out, const void *in, size_t len)
{
    size_t j;

    for (j = 0; j < len; j++)
        out[j] = ((unsigned char *)in)[len - j - 1];
}

/*
 * A memory copy that converts the native byte ordering either to or from
 * little endian format.
 *
 * On a little endian machine copying either is just a memcpy(3), on a
 * big endian machine copying from native to or from little endian involves
 * byte reversal.
 */
static void le_copy(unsigned char *out, size_t outlen,
                    const void *in, size_t inlen)
{
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN) {
        memcpy(out, in, outlen);
    } else {
        if (outlen < inlen) {
            in = (const char *)in + inlen - outlen;
            inlen = outlen;
        }
        if (!ossl_assert(outlen <= inlen))
            return;
        swap_copy(out, in, inlen);
    }
}

static const struct {
    size_t len;
    unsigned char value[MAX_LEN];
} raw_values[] = {
    { 1, { 0x47 } },
    { 1, { 0xd0 } },
    { 2, { 0x01, 0xe9 } },
    { 2, { 0xff, 0x53 } },
    { 3, { 0x16, 0xff, 0x7c } },
    { 3, { 0xa8, 0x9c, 0x0e } },
    { 4, { 0x38, 0x27, 0xbf, 0x3b } },
    { 4, { 0x9f, 0x26, 0x48, 0x22 } },
    { 5, { 0x30, 0x65, 0xfa, 0xe4, 0x81 } },
    { 5, { 0xd1, 0x76, 0x01, 0x1b, 0xcd } },
    { 8, { 0x59, 0xb2, 0x1a, 0xe9, 0x2a, 0xd8, 0x46, 0x40 } },
    { 8, { 0xb4, 0xae, 0xbd, 0xb4, 0xdd, 0x04, 0xb1, 0x4c } },
    { 16, { 0x61, 0xe8, 0x7e, 0x31, 0xe9, 0x33, 0x83, 0x3d,
            0x87, 0x99, 0xc7, 0xd8, 0x5d, 0xa9, 0x8b, 0x42 } },
    { 16, { 0xee, 0x6e, 0x8b, 0xc3, 0xec, 0xcf, 0x37, 0xcc,
            0x89, 0x67, 0xf2, 0x68, 0x33, 0xa0, 0x14, 0xb0 } },
};

static int test_param_type_null(OSSL_PARAM *param)
{
    int rc = 0;
    uint64_t intval;
    double dval;
    BIGNUM *bn;

    switch(param->data_type) {
    case OSSL_PARAM_INTEGER:
        if (param->data_size == sizeof(int32_t))
            rc = OSSL_PARAM_get_int32(param, (int32_t *)&intval);
        else if (param->data_size == sizeof(uint64_t))
            rc = OSSL_PARAM_get_int64(param, (int64_t *)&intval);
        else
            return 1;
        break;
    case OSSL_PARAM_UNSIGNED_INTEGER:
        if (param->data_size == sizeof(uint32_t))
            rc = OSSL_PARAM_get_uint32(param, (uint32_t *)&intval);
        else if (param->data_size == sizeof(uint64_t))
            rc = OSSL_PARAM_get_uint64(param, &intval);
        else
            rc = OSSL_PARAM_get_BN(param, &bn);
        break;
    case OSSL_PARAM_REAL:
        rc = OSSL_PARAM_get_double(param, &dval);
        break;
    case OSSL_PARAM_UTF8_STRING:
    case OSSL_PARAM_OCTET_STRING:
    case OSSL_PARAM_UTF8_PTR:
    case OSSL_PARAM_OCTET_PTR:
        /* these are allowed to be null */
        return 1;
        break;
    }

    /*
     * we expect the various OSSL_PARAM_get functions above
     * to return failure when the data is set to NULL
     */
    return rc == 0;
}

static int test_param_type_extra(OSSL_PARAM *param, const unsigned char *cmp,
                                 size_t width)
{
    int32_t i32;
    int64_t i64;
    size_t s, sz;
    unsigned char buf[MAX_LEN];
    const int bit32 = param->data_size <= sizeof(int32_t);
    const int sizet = param->data_size <= sizeof(size_t);
    const int signd = param->data_type == OSSL_PARAM_INTEGER;

    /*
     * Set the unmodified sentinel directly because there is no param array
     * for these tests.
     */
    param->return_size = OSSL_PARAM_UNMODIFIED;
    if (signd) {
        if ((bit32 && !TEST_true(OSSL_PARAM_get_int32(param, &i32)))
            || !TEST_true(OSSL_PARAM_get_int64(param, &i64)))
            return 0;
    } else {
        if ((bit32
             && !TEST_true(OSSL_PARAM_get_uint32(param, (uint32_t *)&i32)))
            || !TEST_true(OSSL_PARAM_get_uint64(param, (uint64_t *)&i64))
            || (sizet && !TEST_true(OSSL_PARAM_get_size_t(param, &s))))
            return 0;
    }
    if (!TEST_false(OSSL_PARAM_modified(param)))
        return 0;

    /* Check signed types */
    if (bit32) {
        le_copy(buf, sizeof(i32), &i32, sizeof(i32));
        sz = sizeof(i32) < width ? sizeof(i32) : width;
        if (!TEST_mem_eq(buf, sz, cmp, sz))
            return 0;
    }
    le_copy(buf, sizeof(i64), &i64, sizeof(i64));
    sz = sizeof(i64) < width ? sizeof(i64) : width;
    if (!TEST_mem_eq(buf, sz, cmp, sz))
        return 0;
    if (sizet && !signd) {
        le_copy(buf, sizeof(s), &s, sizeof(s));
        sz = sizeof(s) < width ? sizeof(s) : width;
        if (!TEST_mem_eq(buf, sz, cmp, sz))
            return 0;
    }

    /* Check a widening write if possible */
    if (sizeof(size_t) > width) {
        if (signd) {
            if (!TEST_true(OSSL_PARAM_set_int32(param, 12345))
                || !TEST_true(OSSL_PARAM_get_int64(param, &i64))
                || !TEST_size_t_eq((size_t)i64, 12345))
                return 0;
        } else {
            if (!TEST_true(OSSL_PARAM_set_uint32(param, 12345))
                || !TEST_true(OSSL_PARAM_get_uint64(param, (uint64_t *)&i64))
                || !TEST_size_t_eq((size_t)i64, 12345))
                return 0;
        }
        if (!TEST_true(OSSL_PARAM_modified(param)))
            return 0;
    }
    return 1;
}

/*
 * The test cases for each of the bastic integral types are similar.
 * For each type, a param of that type is set and an attempt to read it
 * get is made.  Finally, the above function is called to verify that
 * the params can be read as other types.
 *
 * All the real work is done via byte buffers which are converted to machine
 * byte order and to little endian for comparisons.  Narrower values are best
 * compared using little endian because their values and positions don't
 * change.
 */

static int test_param_int(int n)
{
    int in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(int)];
    const size_t len = raw_values[n].len >= sizeof(int) ?
                       sizeof(int) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_int("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_int(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_int(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(int));
}

static int test_param_long(int n)
{
    long int in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(long int)];
    const size_t len = raw_values[n].len >= sizeof(long int)
                       ? sizeof(long int) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_long("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_long(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_long(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(long int));
}

static int test_param_uint(int n)
{
    unsigned int in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(unsigned int)];
    const size_t len = raw_values[n].len >= sizeof(unsigned int) ? sizeof(unsigned int) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_uint("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_uint(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_uint(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(unsigned int));
}

static int test_param_ulong(int n)
{
    unsigned long int in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(unsigned long int)];
    const size_t len = raw_values[n].len >= sizeof(unsigned long int)
                       ? sizeof(unsigned long int) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_ulong("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_ulong(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_ulong(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(unsigned long int));
}

static int test_param_int32(int n)
{
    int32_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(int32_t)];
    const size_t len = raw_values[n].len >= sizeof(int32_t)
                       ? sizeof(int32_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_int32("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_int32(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_int32(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(int32_t));
}

static int test_param_uint32(int n)
{
    uint32_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(uint32_t)];
    const size_t len = raw_values[n].len >= sizeof(uint32_t)
                       ? sizeof(uint32_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_uint32("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_uint32(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_uint32(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(uint32_t));
}

static int test_param_int64(int n)
{
    int64_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(int64_t)];
    const size_t len = raw_values[n].len >= sizeof(int64_t)
                       ? sizeof(int64_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_int64("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_int64(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_int64(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(int64_t));
}

static int test_param_uint64(int n)
{
    uint64_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(uint64_t)];
    const size_t len = raw_values[n].len >= sizeof(uint64_t)
                       ? sizeof(uint64_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_uint64("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_uint64(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_uint64(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(uint64_t));
}

static int test_param_size_t(int n)
{
    size_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(size_t)];
    const size_t len = raw_values[n].len >= sizeof(size_t)
                       ? sizeof(size_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_size_t("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_size_t(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_size_t(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(size_t));
}

static int test_param_time_t(int n)
{
    time_t in, out;
    unsigned char buf[MAX_LEN], cmp[sizeof(time_t)];
    const size_t len = raw_values[n].len >= sizeof(time_t)
                       ? sizeof(time_t) : raw_values[n].len;
    OSSL_PARAM param = OSSL_PARAM_time_t("a", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    memset(buf, 0, sizeof(buf));
    le_copy(buf, sizeof(in), raw_values[n].value, sizeof(in));
    memcpy(&in, buf, sizeof(in));
    param.data = &out;
    if (!TEST_true(OSSL_PARAM_set_time_t(&param, in)))
        return 0;
    le_copy(cmp, sizeof(out), &out, sizeof(out));
    if (!TEST_mem_eq(cmp, len, raw_values[n].value, len))
        return 0;
    in = 0;
    if (!TEST_true(OSSL_PARAM_get_time_t(&param, &in)))
        return 0;
    le_copy(cmp, sizeof(in), &in, sizeof(in));
    if (!TEST_mem_eq(cmp, sizeof(in), raw_values[n].value, sizeof(in)))
        return 0;
    param.data = &out;
    return test_param_type_extra(&param, raw_values[n].value, sizeof(size_t));
}

static int test_param_bignum(int n)
{
    unsigned char buf[MAX_LEN], bnbuf[MAX_LEN];
    const size_t len = raw_values[n].len;
    BIGNUM *b = NULL, *c = NULL;
    OSSL_PARAM param = OSSL_PARAM_DEFN("bn", OSSL_PARAM_UNSIGNED_INTEGER,
                                       NULL, 0);
    int ret = 0;

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    param.data = bnbuf;
    param.data_size = sizeof(bnbuf);

    if (!TEST_ptr(b = BN_lebin2bn(raw_values[n].value, (int)len, NULL)))
        goto err;

    if (!TEST_true(OSSL_PARAM_set_BN(&param, b)))
        goto err;
    le_copy(buf, len, bnbuf, sizeof(bnbuf));
    if (!TEST_mem_eq(raw_values[n].value, len, buf, len))
        goto err;
    param.data_size = param.return_size;
    if (!TEST_true(OSSL_PARAM_get_BN(&param, &c))
        || !TEST_BN_eq(b, c))
        goto err;

    ret = 1;
err:
    BN_free(b);
    BN_free(c);
    return ret;
}

static int test_param_signed_bignum(int n)
{
    unsigned char buf[MAX_LEN], bnbuf[MAX_LEN];
    const size_t len = raw_values[n].len;
    BIGNUM *b = NULL, *c = NULL;
    OSSL_PARAM param = OSSL_PARAM_DEFN("bn", OSSL_PARAM_INTEGER, NULL, 0);
    int ret = 0;

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    param.data = bnbuf;
    param.data_size = sizeof(bnbuf);

    if (!TEST_ptr(b = BN_signed_lebin2bn(raw_values[n].value, (int)len, NULL)))
        goto err;

    /* raw_values are little endian */
    if (!TEST_false(!!(raw_values[n].value[len - 1] & 0x80) ^ BN_is_negative(b)))
        goto err;
    if (!TEST_true(OSSL_PARAM_set_BN(&param, b)))
        goto err;
    le_copy(buf, len, bnbuf, sizeof(bnbuf));
    if (!TEST_mem_eq(raw_values[n].value, len, buf, len))
        goto err;
    param.data_size = param.return_size;
    if (!TEST_true(OSSL_PARAM_get_BN(&param, &c))
        || !TEST_BN_eq(b, c)) {
        BN_print_fp(stderr, c);
        goto err;
    }

    ret = 1;
err:
    BN_free(b);
    BN_free(c);
    return ret;
}

static int test_param_real(void)
{
    double p;
    OSSL_PARAM param = OSSL_PARAM_double("r", NULL);

    if (!TEST_int_eq(test_param_type_null(&param), 1))
        return 0;

    param.data = &p;
    return TEST_true(OSSL_PARAM_set_double(&param, 3.14159))
           && TEST_double_eq(p, 3.14159);
}

static int test_param_construct(int tstid)
{
    static const char *int_names[] = {
        "int", "long", "int32", "int64"
    };
    static const char *uint_names[] = {
        "uint", "ulong", "uint32", "uint64", "size_t"
    };
    static const unsigned char bn_val[16] = {
        0xac, 0x75, 0x22, 0x7d, 0x81, 0x06, 0x7a, 0x23,
        0xa6, 0xed, 0x87, 0xc7, 0xab, 0xf4, 0x73, 0x22
    };
    OSSL_PARAM *p = NULL, *p1 = NULL;
    static const OSSL_PARAM params_empty[] = {
        OSSL_PARAM_END
    };
    OSSL_PARAM params[20];
    char buf[100], buf2[100], *bufp, *bufp2;
    unsigned char ubuf[100];
    void *vp, *vpn = NULL, *vp2;
    OSSL_PARAM *cp;
    int i, n = 0, ret = 0;
    unsigned int u;
    long int l;
    unsigned long int ul;
    int32_t i32;
    uint32_t u32;
    int64_t i64;
    uint64_t u64;
    size_t j, k, s;
    double d, d2;
    BIGNUM *bn = NULL, *bn2 = NULL;

    params[n++] = OSSL_PARAM_construct_int("int", &i);
    params[n++] = OSSL_PARAM_construct_uint("uint", &u);
    params[n++] = OSSL_PARAM_construct_long("long", &l);
    params[n++] = OSSL_PARAM_construct_ulong("ulong", &ul);
    params[n++] = OSSL_PARAM_construct_int32("int32", &i32);
    params[n++] = OSSL_PARAM_construct_int64("int64", &i64);
    params[n++] = OSSL_PARAM_construct_uint32("uint32", &u32);
    params[n++] = OSSL_PARAM_construct_uint64("uint64", &u64);
    params[n++] = OSSL_PARAM_construct_size_t("size_t", &s);
    params[n++] = OSSL_PARAM_construct_double("double", &d);
    params[n++] = OSSL_PARAM_construct_BN("bignum", ubuf, sizeof(ubuf));
    params[n++] = OSSL_PARAM_construct_utf8_string("utf8str", buf, sizeof(buf));
    params[n++] = OSSL_PARAM_construct_octet_string("octstr", buf, sizeof(buf));
    params[n++] = OSSL_PARAM_construct_utf8_ptr("utf8ptr", &bufp, 0);
    params[n++] = OSSL_PARAM_construct_octet_ptr("octptr", &vp, 0);
    params[n] = OSSL_PARAM_construct_end();

    switch (tstid) {
    case 0:
        p = params;
        break;
    case 1:
        p = OSSL_PARAM_merge(params, params_empty);
        break;
    case 2:
        p = OSSL_PARAM_dup(params);
        break;
    default:
        p1 = OSSL_PARAM_dup(params);
        p = OSSL_PARAM_merge(p1, params_empty);
        break;
    }

    /* Search failure */
    if (!TEST_ptr_null(OSSL_PARAM_locate(p, "fnord")))
        goto err;

    /* All signed integral types */
    for (j = 0; j < OSSL_NELEM(int_names); j++) {
        if (!TEST_ptr(cp = OSSL_PARAM_locate(p, int_names[j]))
            || !TEST_true(OSSL_PARAM_set_int32(cp, (int32_t)(3 + j)))
            || !TEST_true(OSSL_PARAM_get_int64(cp, &i64))
            || !TEST_size_t_eq(cp->data_size, cp->return_size)
            || !TEST_size_t_eq((size_t)i64, 3 + j)) {
            TEST_note("iteration %zu var %s", j + 1, int_names[j]);
            goto err;
        }
    }
    /* All unsigned integral types */
    for (j = 0; j < OSSL_NELEM(uint_names); j++) {
        if (!TEST_ptr(cp = OSSL_PARAM_locate(p, uint_names[j]))
            || !TEST_true(OSSL_PARAM_set_uint32(cp, (uint32_t)(3 + j)))
            || !TEST_true(OSSL_PARAM_get_uint64(cp, &u64))
            || !TEST_size_t_eq(cp->data_size, cp->return_size)
            || !TEST_size_t_eq((size_t)u64, 3 + j)) {
            TEST_note("iteration %zu var %s", j + 1, uint_names[j]);
            goto err;
        }
    }
    /* Real */
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "double"))
        || !TEST_true(OSSL_PARAM_set_double(cp, 3.14))
        || !TEST_true(OSSL_PARAM_get_double(cp, &d2))
        || !TEST_size_t_eq(cp->return_size, sizeof(double))
        || !TEST_double_eq(d2, 3.14)
        || (tstid <= 1 && !TEST_double_eq(d, d2)))
        goto err;
    /* UTF8 string */
    bufp = NULL;
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "utf8str"))
        || !TEST_true(OSSL_PARAM_set_utf8_string(cp, "abcdef"))
        || !TEST_size_t_eq(cp->return_size, sizeof("abcdef") - 1)
        || !TEST_true(OSSL_PARAM_get_utf8_string(cp, &bufp, 0))
        || !TEST_str_eq(bufp, "abcdef")) {
        OPENSSL_free(bufp);
        goto err;
    }
    OPENSSL_free(bufp);
    bufp = buf2;
    if (!TEST_true(OSSL_PARAM_get_utf8_string(cp, &bufp, sizeof(buf2)))
        || !TEST_str_eq(buf2, "abcdef"))
        goto err;
    /* UTF8 pointer */
    /* Note that the size of a UTF8 string does *NOT* include the NUL byte */
    bufp = buf;
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "utf8ptr"))
        || !TEST_true(OSSL_PARAM_set_utf8_ptr(cp, "tuvwxyz"))
        || !TEST_size_t_eq(cp->return_size, sizeof("tuvwxyz") - 1)
        || !TEST_true(OSSL_PARAM_get_utf8_ptr(cp, (const char **)&bufp2))
        || !TEST_str_eq(bufp2, "tuvwxyz")
        || (tstid <= 1 && !TEST_ptr_eq(bufp2, bufp)))
        goto err;
    /* OCTET string */
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "octstr"))
        || !TEST_true(OSSL_PARAM_set_octet_string(cp, "abcdefghi",
                                                  sizeof("abcdefghi")))
        || !TEST_size_t_eq(cp->return_size, sizeof("abcdefghi")))
        goto err;
    /* Match the return size to avoid trailing garbage bytes */
    cp->data_size = cp->return_size;
    if (!TEST_true(OSSL_PARAM_get_octet_string(cp, &vpn, 0, &s))
        || !TEST_size_t_eq(s, sizeof("abcdefghi"))
        || !TEST_mem_eq(vpn, sizeof("abcdefghi"),
                        "abcdefghi", sizeof("abcdefghi")))
        goto err;
    vp = buf2;
    if (!TEST_true(OSSL_PARAM_get_octet_string(cp, &vp, sizeof(buf2), &s))
        || !TEST_size_t_eq(s, sizeof("abcdefghi"))
        || !TEST_mem_eq(vp, sizeof("abcdefghi"),
                        "abcdefghi", sizeof("abcdefghi")))
        goto err;
    /* OCTET pointer */
    vp = &l;
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "octptr"))
        || !TEST_true(OSSL_PARAM_set_octet_ptr(cp, &ul, sizeof(ul)))
        || !TEST_size_t_eq(cp->return_size, sizeof(ul))
        || (tstid <= 1 && !TEST_ptr_eq(vp, &ul)))
        goto err;
    /* Match the return size to avoid trailing garbage bytes */
    cp->data_size = cp->return_size;
    if (!TEST_true(OSSL_PARAM_get_octet_ptr(cp, (const void **)&vp2, &k))
        || !TEST_size_t_eq(k, sizeof(ul))
        || (tstid <= 1 && !TEST_ptr_eq(vp2, vp)))
        goto err;
    /* BIGNUM */
    if (!TEST_ptr(cp = OSSL_PARAM_locate(p, "bignum"))
        || !TEST_ptr(bn = BN_lebin2bn(bn_val, (int)sizeof(bn_val), NULL))
        || !TEST_true(OSSL_PARAM_set_BN(cp, bn))
        || !TEST_size_t_eq(cp->data_size, cp->return_size))
        goto err;
    /* Match the return size to avoid trailing garbage bytes */
    cp->data_size = cp->return_size;
    if (!TEST_true(OSSL_PARAM_get_BN(cp, &bn2))
        || !TEST_BN_eq(bn, bn2))
        goto err;
    ret = 1;
err:
    if (p != params)
        OPENSSL_free(p);
    OPENSSL_free(p1);
    OPENSSL_free(vpn);
    BN_free(bn);
    BN_free(bn2);
    return ret;
}

static int test_param_modified(void)
{
    OSSL_PARAM param[3] = { OSSL_PARAM_int("a", NULL),
                            OSSL_PARAM_int("b", NULL),
                            OSSL_PARAM_END };
    int a, b;

    param->data = &a;
    param[1].data = &b;
    if (!TEST_false(OSSL_PARAM_modified(param))
            && !TEST_true(OSSL_PARAM_set_int32(param, 1234))
            && !TEST_true(OSSL_PARAM_modified(param))
            && !TEST_false(OSSL_PARAM_modified(param + 1))
            && !TEST_true(OSSL_PARAM_set_int32(param + 1, 1))
            && !TEST_true(OSSL_PARAM_modified(param + 1)))
        return 0;
    OSSL_PARAM_set_all_unmodified(param);
    if (!TEST_false(OSSL_PARAM_modified(param))
            && !TEST_true(OSSL_PARAM_set_int32(param, 4321))
            && !TEST_true(OSSL_PARAM_modified(param))
            && !TEST_false(OSSL_PARAM_modified(param + 1))
            && !TEST_true(OSSL_PARAM_set_int32(param + 1, 2))
            && !TEST_true(OSSL_PARAM_modified(param + 1)))
        return 0;
    return 1;
}

static int test_param_copy_null(void)
{
    int ret, val;
    int a = 1, b = 2, i = 0;
    OSSL_PARAM *cp1 = NULL, *cp2 = NULL, *p;
    OSSL_PARAM param[3];

    param[i++] = OSSL_PARAM_construct_int("a", &a);
    param[i++] = OSSL_PARAM_construct_int("b", &b);
    param[i] = OSSL_PARAM_construct_end();

    ret = TEST_ptr_null(OSSL_PARAM_dup(NULL))
          && TEST_ptr(cp1 = OSSL_PARAM_merge(NULL, param))
          && TEST_ptr(p = OSSL_PARAM_locate(cp1, "a"))
          && TEST_true(OSSL_PARAM_get_int(p, &val))
          && TEST_int_eq(val, 1)
          && TEST_ptr(p = OSSL_PARAM_locate(cp1, "b"))
          && TEST_true(OSSL_PARAM_get_int(p, &val))
          && TEST_int_eq(val, 2)
          && TEST_ptr(cp2 = OSSL_PARAM_merge(param, NULL))
          && TEST_ptr(p = OSSL_PARAM_locate(cp2, "a"))
          && TEST_true(OSSL_PARAM_get_int(p, &val))
          && TEST_int_eq(val, 1)
          && TEST_ptr(p = OSSL_PARAM_locate(cp2, "b"))
          && TEST_true(OSSL_PARAM_get_int(p, &val))
          && TEST_int_eq(val, 2)
          && TEST_ptr_null(OSSL_PARAM_merge(NULL, NULL));
    OSSL_PARAM_free(cp2);
    OSSL_PARAM_free(cp1);
    return ret;
}
static int test_param_merge(void)
{
    int val, ret;
    int values[] = {1, 2, 3, 4};
    OSSL_PARAM *p = NULL, *cp = NULL;
    OSSL_PARAM param[3], param1[3];

    param[0] = OSSL_PARAM_construct_int("diff1", &values[0]);
    param[1] = OSSL_PARAM_construct_int("same", &values[1]);
    param[2] = OSSL_PARAM_construct_end();
    param1[0] = OSSL_PARAM_construct_int("diff2", &values[2]);
    param1[1] = OSSL_PARAM_construct_int("same", &values[3]);
    param1[2] = OSSL_PARAM_construct_end();

    ret = TEST_ptr(p = OSSL_PARAM_merge(param, param1))
        && TEST_ptr(cp = OSSL_PARAM_locate(p, "diff1"))
        && TEST_true(OSSL_PARAM_get_int(p, &val))
        && TEST_int_eq(val, values[0])
        && TEST_ptr(cp = OSSL_PARAM_locate(p, "diff2"))
        && TEST_true(OSSL_PARAM_get_int(cp, &val))
        && TEST_int_eq(val, values[2])
        && TEST_ptr(cp = OSSL_PARAM_locate(p, "same"))
        && TEST_true(OSSL_PARAM_get_int(cp, &val))
        && TEST_int_eq(val, values[3]);
    OSSL_PARAM_free(p);
    return ret;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_param_int, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_long, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_uint, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_ulong, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_int32, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_uint32, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_size_t, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_time_t, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_int64, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_uint64, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_bignum, OSSL_NELEM(raw_values));
    ADD_ALL_TESTS(test_param_signed_bignum, OSSL_NELEM(raw_values));
    ADD_TEST(test_param_real);
    ADD_ALL_TESTS(test_param_construct, 4);
    ADD_TEST(test_param_modified);
    ADD_TEST(test_param_copy_null);
    ADD_TEST(test_param_merge);
    return 1;
}
