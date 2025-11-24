/*
 * Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/params.h>
#include <openssl/param_build.h>
#include "internal/param_build_set.h"

#define OSSL_PARAM_ALLOCATED_END    127
#define OSSL_PARAM_MERGE_LIST_MAX   128

#define OSSL_PARAM_BUF_PUBLIC 0
#define OSSL_PARAM_BUF_SECURE 1
#define OSSL_PARAM_BUF_MAX    (OSSL_PARAM_BUF_SECURE + 1)

typedef struct {
    OSSL_PARAM_ALIGNED_BLOCK *alloc; /* The allocated buffer */
    OSSL_PARAM_ALIGNED_BLOCK *cur;   /* Current position in the allocated buf */
    size_t blocks;    /* Number of aligned blocks */
    size_t alloc_sz;  /* The size of the allocated buffer (in bytes) */
} OSSL_PARAM_BUF;

size_t ossl_param_bytes_to_blocks(size_t bytes)
{
    return (bytes + OSSL_PARAM_ALIGN_SIZE - 1) / OSSL_PARAM_ALIGN_SIZE;
}

static int ossl_param_buf_alloc(OSSL_PARAM_BUF *out, size_t extra_blocks,
                                int is_secure)
{
    size_t sz = OSSL_PARAM_ALIGN_SIZE * (extra_blocks + out->blocks);

    out->alloc = is_secure ? OPENSSL_secure_zalloc(sz) : OPENSSL_zalloc(sz);
    if (out->alloc == NULL)
        return 0;
    out->alloc_sz = sz;
    out->cur = out->alloc + extra_blocks;
    return 1;
}

void ossl_param_set_secure_block(OSSL_PARAM *last, void *secure_buffer,
                                 size_t secure_buffer_sz)
{
    last->key = NULL;
    last->data_size = secure_buffer_sz;
    last->data = secure_buffer;
    last->data_type = OSSL_PARAM_ALLOCATED_END;
}

static OSSL_PARAM *ossl_param_dup(const OSSL_PARAM *src, OSSL_PARAM *dst,
                                  OSSL_PARAM_BUF buf[OSSL_PARAM_BUF_MAX],
                                  int *param_count)
{
    const OSSL_PARAM *in;
    int has_dst = (dst != NULL);
    int is_secure;
    size_t param_sz, blks;

    for (in = src; in->key != NULL; in++) {
        is_secure = CRYPTO_secure_allocated(in->data);
        if (has_dst) {
            *dst = *in;
            dst->data = buf[is_secure].cur;
        }

        if (in->data_type == OSSL_PARAM_OCTET_PTR
            || in->data_type == OSSL_PARAM_UTF8_PTR) {
            param_sz = sizeof(in->data);
            if (has_dst)
                *((const void **)dst->data) = *(const void **)in->data;
        } else {
            param_sz = in->data_size;
            if (has_dst)
                memcpy(dst->data, in->data, param_sz);
        }
        if (in->data_type == OSSL_PARAM_UTF8_STRING)
            param_sz++; /* NULL terminator */
        blks = ossl_param_bytes_to_blocks(param_sz);

        if (has_dst) {
            dst++;
            buf[is_secure].cur += blks;
        } else {
            buf[is_secure].blocks += blks;
        }
        if (param_count != NULL)
            ++*param_count;
    }
    return dst;
}

OSSL_PARAM *OSSL_PARAM_dup(const OSSL_PARAM *src)
{
    size_t param_blocks;
    OSSL_PARAM_BUF buf[OSSL_PARAM_BUF_MAX];
    OSSL_PARAM *last, *dst;
    int param_count = 1; /* Include terminator in the count */

    if (src == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    memset(buf, 0, sizeof(buf));

    /* First Pass: get the param_count and block sizes required */
    (void)ossl_param_dup(src, NULL, buf, &param_count);

    param_blocks = ossl_param_bytes_to_blocks(param_count * sizeof(*src));
    /*
     * The allocated buffer consists of an array of OSSL_PARAM followed by
     * aligned data bytes that the array elements will point to.
     */
    if (!ossl_param_buf_alloc(&buf[OSSL_PARAM_BUF_PUBLIC], param_blocks, 0))
        return NULL;

    /* Allocate a secure memory buffer if required */
    if (buf[OSSL_PARAM_BUF_SECURE].blocks > 0
        && !ossl_param_buf_alloc(&buf[OSSL_PARAM_BUF_SECURE], 0, 1)) {
        OPENSSL_free(buf[OSSL_PARAM_BUF_PUBLIC].alloc);
        return NULL;
    }

    dst = (OSSL_PARAM *)buf[OSSL_PARAM_BUF_PUBLIC].alloc;
    last = ossl_param_dup(src, dst, buf, NULL);
    /* Store the allocated secure memory buffer in the last param block */
    ossl_param_set_secure_block(last, buf[OSSL_PARAM_BUF_SECURE].alloc,
                                buf[OSSL_PARAM_BUF_SECURE].alloc_sz);
    return dst;
}

static int compare_params(const void *left, const void *right)
{
    const OSSL_PARAM *l = *(const OSSL_PARAM **)left;
    const OSSL_PARAM *r = *(const OSSL_PARAM **)right;

    return OPENSSL_strcasecmp(l->key, r->key);
}

OSSL_PARAM *OSSL_PARAM_merge(const OSSL_PARAM *p1, const OSSL_PARAM *p2)
{
    const OSSL_PARAM *list1[OSSL_PARAM_MERGE_LIST_MAX + 1];
    const OSSL_PARAM *list2[OSSL_PARAM_MERGE_LIST_MAX + 1];
    const OSSL_PARAM *p = NULL;
    const OSSL_PARAM **p1cur, **p2cur;
    OSSL_PARAM *params, *dst;
    size_t  list1_sz = 0, list2_sz = 0;
    int diff;

    if (p1 == NULL && p2 == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    /* Copy p1 to list1 */
    if (p1 != NULL) {
        for (p = p1; p->key != NULL && list1_sz < OSSL_PARAM_MERGE_LIST_MAX; p++)
            list1[list1_sz++] = p;
    }
    list1[list1_sz] = NULL;

    /* copy p2 to a list2 */
    if (p2 != NULL) {
        for (p = p2; p->key != NULL && list2_sz < OSSL_PARAM_MERGE_LIST_MAX; p++)
            list2[list2_sz++] = p;
    }
    list2[list2_sz] = NULL;
    if (list1_sz == 0 && list2_sz == 0) {
        ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_NO_PARAMS_TO_MERGE);
        return NULL;
    }

    /* Sort the 2 lists */
    qsort(list1, list1_sz, sizeof(OSSL_PARAM *), compare_params);
    qsort(list2, list2_sz, sizeof(OSSL_PARAM *), compare_params);

   /* Allocate enough space to store the merged parameters */
    params = OPENSSL_zalloc((list1_sz + list2_sz + 1) * sizeof(*p1));
    if (params == NULL)
        return NULL;
    dst = params;
    p1cur = list1;
    p2cur = list2;
    while (1) {
        /* If list1 is finished just tack list2 onto the end */
        if (*p1cur == NULL) {
            while (*p2cur != NULL) {
                *dst++ = **p2cur;
                p2cur++;
            }
            break;
        }
        /* If list2 is finished just tack list1 onto the end */
        if (*p2cur == NULL) {
            while (*p1cur != NULL) {
                *dst++ = **p1cur;
                p1cur++;
            }
            break;
        }
        /* consume the list element with the smaller key */
        diff = OPENSSL_strcasecmp((*p1cur)->key, (*p2cur)->key);
        if (diff == 0) {
            /* If the keys are the same then throw away the list1 element */
            *dst++ = **p2cur;
            p2cur++;
            p1cur++;
        } else if (diff > 0) {
            *dst++ = **p2cur;
            p2cur++;
        } else {
            *dst++ = **p1cur;
            p1cur++;
        }
    }
    return params;
}

void OSSL_PARAM_free(OSSL_PARAM *params)
{
    if (params != NULL) {
        OSSL_PARAM *p;

        for (p = params; p->key != NULL; p++)
            ;
        if (p->data_type == OSSL_PARAM_ALLOCATED_END)
            OPENSSL_secure_clear_free(p->data, p->data_size);
        OPENSSL_free(params);
    }
}
