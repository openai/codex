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
#include <openssl/err.h>
#include <openssl/cryptoerr.h>
#include <openssl/params.h>
#include <openssl/types.h>
#include <openssl/safestack.h>
#include "internal/param_build_set.h"

/*
 * Special internal param type to indicate the end of an allocate OSSL_PARAM
 * array.
 */

typedef struct {
    const char *key;
    int type;
    int secure;
    size_t size;
    size_t alloc_blocks;
    const BIGNUM *bn;
    const void *string;
    union {
        /*
         * These fields are never directly addressed, but their sizes are
         * important so that all native types can be copied here without overrun.
         */
        ossl_intmax_t i;
        ossl_uintmax_t u;
        double d;
    } num;
} OSSL_PARAM_BLD_DEF;

DEFINE_STACK_OF(OSSL_PARAM_BLD_DEF)

struct ossl_param_bld_st {
    size_t total_blocks;
    size_t secure_blocks;
    STACK_OF(OSSL_PARAM_BLD_DEF) *params;
};

static OSSL_PARAM_BLD_DEF *param_push(OSSL_PARAM_BLD *bld, const char *key,
                                      size_t size, size_t alloc, int type,
                                      int secure)
{
    OSSL_PARAM_BLD_DEF *pd = OPENSSL_zalloc(sizeof(*pd));

    if (pd == NULL)
        return NULL;
    pd->key = key;
    pd->type = type;
    pd->size = size;
    pd->alloc_blocks = ossl_param_bytes_to_blocks(alloc);
    if ((pd->secure = secure) != 0)
        bld->secure_blocks += pd->alloc_blocks;
    else
        bld->total_blocks += pd->alloc_blocks;
    if (sk_OSSL_PARAM_BLD_DEF_push(bld->params, pd) <= 0) {
        OPENSSL_free(pd);
        pd = NULL;
    }
    return pd;
}

static int param_push_num(OSSL_PARAM_BLD *bld, const char *key,
                          void *num, size_t size, int type)
{
    OSSL_PARAM_BLD_DEF *pd = param_push(bld, key, size, size, type, 0);

    if (pd == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (size > sizeof(pd->num)) {
        ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_MANY_BYTES);
        return 0;
    }
    memcpy(&pd->num, num, size);
    return 1;
}

OSSL_PARAM_BLD *OSSL_PARAM_BLD_new(void)
{
    OSSL_PARAM_BLD *r = OPENSSL_zalloc(sizeof(OSSL_PARAM_BLD));

    if (r != NULL) {
        r->params = sk_OSSL_PARAM_BLD_DEF_new_null();
        if (r->params == NULL) {
            OPENSSL_free(r);
            r = NULL;
        }
    }
    return r;
}

static void free_all_params(OSSL_PARAM_BLD *bld)
{
    int i, n = sk_OSSL_PARAM_BLD_DEF_num(bld->params);

    for (i = 0; i < n; i++)
        OPENSSL_free(sk_OSSL_PARAM_BLD_DEF_pop(bld->params));
}

void OSSL_PARAM_BLD_free(OSSL_PARAM_BLD *bld)
{
    if (bld == NULL)
        return;
    free_all_params(bld);
    sk_OSSL_PARAM_BLD_DEF_free(bld->params);
    OPENSSL_free(bld);
}

int OSSL_PARAM_BLD_push_int(OSSL_PARAM_BLD *bld, const char *key, int num)
{
    return param_push_num(bld, key, &num, sizeof(num), OSSL_PARAM_INTEGER);
}

int OSSL_PARAM_BLD_push_uint(OSSL_PARAM_BLD *bld, const char *key,
                             unsigned int num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_long(OSSL_PARAM_BLD *bld, const char *key,
                             long int num)
{
    return param_push_num(bld, key, &num, sizeof(num), OSSL_PARAM_INTEGER);
}

int OSSL_PARAM_BLD_push_ulong(OSSL_PARAM_BLD *bld, const char *key,
                              unsigned long int num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_int32(OSSL_PARAM_BLD *bld, const char *key,
                              int32_t num)
{
    return param_push_num(bld, key, &num, sizeof(num), OSSL_PARAM_INTEGER);
}

int OSSL_PARAM_BLD_push_uint32(OSSL_PARAM_BLD *bld, const char *key,
                               uint32_t num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_int64(OSSL_PARAM_BLD *bld, const char *key,
                              int64_t num)
{
    return param_push_num(bld, key, &num, sizeof(num), OSSL_PARAM_INTEGER);
}

int OSSL_PARAM_BLD_push_uint64(OSSL_PARAM_BLD *bld, const char *key,
                               uint64_t num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_size_t(OSSL_PARAM_BLD *bld, const char *key,
                               size_t num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_time_t(OSSL_PARAM_BLD *bld, const char *key,
                               time_t num)
{
    return param_push_num(bld, key, &num, sizeof(num),
                          OSSL_PARAM_INTEGER);
}

int OSSL_PARAM_BLD_push_double(OSSL_PARAM_BLD *bld, const char *key,
                               double num)
{
    return param_push_num(bld, key, &num, sizeof(num), OSSL_PARAM_REAL);
}

static int push_BN(OSSL_PARAM_BLD *bld, const char *key,
                   const BIGNUM *bn, size_t sz, int type)
{
    int n, secure = 0;
    OSSL_PARAM_BLD_DEF *pd;

    if (!ossl_assert(type == OSSL_PARAM_UNSIGNED_INTEGER
                     || type == OSSL_PARAM_INTEGER))
        return 0;

    if (bn != NULL) {
        if (type == OSSL_PARAM_UNSIGNED_INTEGER && BN_is_negative(bn)) {
            ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_UNSUPPORTED,
                           "Negative big numbers are unsupported for OSSL_PARAM_UNSIGNED_INTEGER");
            return 0;
        }

        n = BN_num_bytes(bn);
        if (n < 0) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_ZERO_LENGTH_NUMBER);
            return 0;
        }
        if (sz < (size_t)n) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_SMALL_BUFFER);
            return 0;
        }
        if (BN_get_flags(bn, BN_FLG_SECURE) == BN_FLG_SECURE)
            secure = 1;

        /* The BIGNUM is zero, we must transfer at least one byte */
        if (sz == 0)
            sz++;
    }
    pd = param_push(bld, key, sz, sz, type, secure);
    if (pd == NULL)
        return 0;
    pd->bn = bn;
    return 1;
}

int OSSL_PARAM_BLD_push_BN(OSSL_PARAM_BLD *bld, const char *key,
                           const BIGNUM *bn)
{
    if (bn != NULL && BN_is_negative(bn))
        return push_BN(bld, key, bn, BN_num_bytes(bn) + 1,
                       OSSL_PARAM_INTEGER);
    return push_BN(bld, key, bn, bn == NULL ? 0 : BN_num_bytes(bn),
                   OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_BN_pad(OSSL_PARAM_BLD *bld, const char *key,
                               const BIGNUM *bn, size_t sz)
{
    if (bn != NULL && BN_is_negative(bn))
        return push_BN(bld, key, bn, BN_num_bytes(bn),
                       OSSL_PARAM_INTEGER);
    return push_BN(bld, key, bn, sz, OSSL_PARAM_UNSIGNED_INTEGER);
}

int OSSL_PARAM_BLD_push_utf8_string(OSSL_PARAM_BLD *bld, const char *key,
                                    const char *buf, size_t bsize)
{
    OSSL_PARAM_BLD_DEF *pd;
    int secure;

    if (bsize == 0)
        bsize = strlen(buf);
    secure = CRYPTO_secure_allocated(buf);
    pd = param_push(bld, key, bsize, bsize + 1, OSSL_PARAM_UTF8_STRING, secure);
    if (pd == NULL)
        return 0;
    pd->string = buf;
    return 1;
}

int OSSL_PARAM_BLD_push_utf8_ptr(OSSL_PARAM_BLD *bld, const char *key,
                                 char *buf, size_t bsize)
{
    OSSL_PARAM_BLD_DEF *pd;

    if (bsize == 0)
        bsize = strlen(buf);
    pd = param_push(bld, key, bsize, sizeof(buf), OSSL_PARAM_UTF8_PTR, 0);
    if (pd == NULL)
        return 0;
    pd->string = buf;
    return 1;
}

int OSSL_PARAM_BLD_push_octet_string(OSSL_PARAM_BLD *bld, const char *key,
                                     const void *buf, size_t bsize)
{
    OSSL_PARAM_BLD_DEF *pd;
    int secure;

    secure = CRYPTO_secure_allocated(buf);
    pd = param_push(bld, key, bsize, bsize, OSSL_PARAM_OCTET_STRING, secure);
    if (pd == NULL)
        return 0;
    pd->string = buf;
    return 1;
}

int OSSL_PARAM_BLD_push_octet_ptr(OSSL_PARAM_BLD *bld, const char *key,
                                  void *buf, size_t bsize)
{
    OSSL_PARAM_BLD_DEF *pd;

    pd = param_push(bld, key, bsize, sizeof(buf), OSSL_PARAM_OCTET_PTR, 0);
    if (pd == NULL)
        return 0;
    pd->string = buf;
    return 1;
}

static OSSL_PARAM *param_bld_convert(OSSL_PARAM_BLD *bld, OSSL_PARAM *param,
                                     OSSL_PARAM_ALIGNED_BLOCK *blk,
                                     OSSL_PARAM_ALIGNED_BLOCK *secure)
{
    int i, num = sk_OSSL_PARAM_BLD_DEF_num(bld->params);
    OSSL_PARAM_BLD_DEF *pd;
    void *p;

    for (i = 0; i < num; i++) {
        pd = sk_OSSL_PARAM_BLD_DEF_value(bld->params, i);
        param[i].key = pd->key;
        param[i].data_type = pd->type;
        param[i].data_size = pd->size;
        param[i].return_size = OSSL_PARAM_UNMODIFIED;

        if (pd->secure) {
            p = secure;
            secure += pd->alloc_blocks;
        } else {
            p = blk;
            blk += pd->alloc_blocks;
        }
        param[i].data = p;
        if (pd->bn != NULL) {
            /* BIGNUM */
            if (pd->type == OSSL_PARAM_UNSIGNED_INTEGER)
                BN_bn2nativepad(pd->bn, (unsigned char *)p, pd->size);
            else
                BN_signed_bn2native(pd->bn, (unsigned char *)p, pd->size);
        } else if (pd->type == OSSL_PARAM_OCTET_PTR
                   || pd->type == OSSL_PARAM_UTF8_PTR) {
            /* PTR */
            *(const void **)p = pd->string;
        } else if (pd->type == OSSL_PARAM_OCTET_STRING
                   || pd->type == OSSL_PARAM_UTF8_STRING) {
            if (pd->string != NULL)
                memcpy(p, pd->string, pd->size);
            else
                memset(p, 0, pd->size);
            if (pd->type == OSSL_PARAM_UTF8_STRING)
                ((char *)p)[pd->size] = '\0';
        } else {
            /* Number, but could also be a NULL BIGNUM */
            if (pd->size > sizeof(pd->num))
                memset(p, 0, pd->size);
            else if (pd->size > 0)
                memcpy(p, &pd->num, pd->size);
        }
    }
    param[i] = OSSL_PARAM_construct_end();
    return param + i;
}

OSSL_PARAM *OSSL_PARAM_BLD_to_param(OSSL_PARAM_BLD *bld)
{
    OSSL_PARAM_ALIGNED_BLOCK *blk, *s = NULL;
    OSSL_PARAM *params, *last;
    const int num = sk_OSSL_PARAM_BLD_DEF_num(bld->params);
    const size_t p_blks = ossl_param_bytes_to_blocks((1 + num) * sizeof(*params));
    const size_t total = OSSL_PARAM_ALIGN_SIZE * (p_blks + bld->total_blocks);
    const size_t ss = OSSL_PARAM_ALIGN_SIZE * bld->secure_blocks;

    if (ss > 0) {
        s = OPENSSL_secure_malloc(ss);
        if (s == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_SECURE_MALLOC_FAILURE);
            return NULL;
        }
    }
    params = OPENSSL_malloc(total);
    if (params == NULL) {
        OPENSSL_secure_free(s);
        return NULL;
    }
    blk = p_blks + (OSSL_PARAM_ALIGNED_BLOCK *)(params);
    last = param_bld_convert(bld, params, blk, s);
    ossl_param_set_secure_block(last, s, ss);

    /* Reset builder for reuse */
    bld->total_blocks = 0;
    bld->secure_blocks = 0;
    free_all_params(bld);
    return params;
}
