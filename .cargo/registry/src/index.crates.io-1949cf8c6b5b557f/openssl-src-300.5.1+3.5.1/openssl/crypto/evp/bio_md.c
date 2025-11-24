/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <errno.h>
#include <openssl/buffer.h>
#include <openssl/evp.h>
#include "internal/bio.h"

/*
 * BIO_put and BIO_get both add to the digest, BIO_gets returns the digest
 */

static int md_write(BIO *h, char const *buf, int num);
static int md_read(BIO *h, char *buf, int size);
static int md_gets(BIO *h, char *str, int size);
static long md_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int md_new(BIO *h);
static int md_free(BIO *data);
static long md_callback_ctrl(BIO *h, int cmd, BIO_info_cb *fp);

static const BIO_METHOD methods_md = {
    BIO_TYPE_MD,
    "message digest",
    bwrite_conv,
    md_write,
    bread_conv,
    md_read,
    NULL,                       /* md_puts, */
    md_gets,
    md_ctrl,
    md_new,
    md_free,
    md_callback_ctrl,
};

const BIO_METHOD *BIO_f_md(void)
{
    return &methods_md;
}

static int md_new(BIO *bi)
{
    EVP_MD_CTX *ctx;

    ctx = EVP_MD_CTX_new();
    if (ctx == NULL)
        return 0;

    BIO_set_init(bi, 1);
    BIO_set_data(bi, ctx);

    return 1;
}

static int md_free(BIO *a)
{
    if (a == NULL)
        return 0;
    EVP_MD_CTX_free(BIO_get_data(a));
    BIO_set_data(a, NULL);
    BIO_set_init(a, 0);

    return 1;
}

static int md_read(BIO *b, char *out, int outl)
{
    int ret = 0;
    EVP_MD_CTX *ctx;
    BIO *next;

    if (out == NULL)
        return 0;

    ctx = BIO_get_data(b);
    next = BIO_next(b);

    if ((ctx == NULL) || (next == NULL))
        return 0;

    ret = BIO_read(next, out, outl);
    if (BIO_get_init(b)) {
        if (ret > 0) {
            if (EVP_DigestUpdate(ctx, (unsigned char *)out,
                                 (unsigned int)ret) <= 0)
                return -1;
        }
    }
    BIO_clear_retry_flags(b);
    BIO_copy_next_retry(b);
    return ret;
}

static int md_write(BIO *b, const char *in, int inl)
{
    int ret = 0;
    EVP_MD_CTX *ctx;
    BIO *next;

    if ((in == NULL) || (inl <= 0))
        return 0;

    ctx = BIO_get_data(b);
    next = BIO_next(b);
    if ((ctx != NULL) && (next != NULL))
        ret = BIO_write(next, in, inl);

    if (BIO_get_init(b)) {
        if (ret > 0) {
            if (!EVP_DigestUpdate(ctx, (const unsigned char *)in,
                                  (unsigned int)ret)) {
                BIO_clear_retry_flags(b);
                return 0;
            }
        }
    }
    if (next != NULL) {
        BIO_clear_retry_flags(b);
        BIO_copy_next_retry(b);
    }
    return ret;
}

static long md_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    EVP_MD_CTX *ctx, *dctx, **pctx;
    const EVP_MD **ppmd;
    EVP_MD *md;
    long ret = 1;
    BIO *dbio, *next;


    ctx = BIO_get_data(b);
    next = BIO_next(b);

    switch (cmd) {
    case BIO_CTRL_RESET:
        if (BIO_get_init(b))
            ret = EVP_DigestInit_ex(ctx, EVP_MD_CTX_get0_md(ctx), NULL);
        else
            ret = 0;
        if (ret > 0)
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    case BIO_C_GET_MD:
        if (BIO_get_init(b)) {
            ppmd = ptr;
            *ppmd = EVP_MD_CTX_get0_md(ctx);
        } else
            ret = 0;
        break;
    case BIO_C_GET_MD_CTX:
        pctx = ptr;
        *pctx = ctx;
        BIO_set_init(b, 1);
        break;
    case BIO_C_SET_MD_CTX:
        if (BIO_get_init(b))
            BIO_set_data(b, ptr);
        else
            ret = 0;
        break;
    case BIO_C_DO_STATE_MACHINE:
        BIO_clear_retry_flags(b);
        ret = BIO_ctrl(next, cmd, num, ptr);
        BIO_copy_next_retry(b);
        break;

    case BIO_C_SET_MD:
        md = ptr;
        ret = EVP_DigestInit_ex(ctx, md, NULL);
        if (ret > 0)
            BIO_set_init(b, 1);
        break;
    case BIO_CTRL_DUP:
        dbio = ptr;
        dctx = BIO_get_data(dbio);
        if (!EVP_MD_CTX_copy_ex(dctx, ctx))
            return 0;
        BIO_set_init(b, 1);
        break;
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static long md_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    BIO *next;

    next = BIO_next(b);

    if (next == NULL)
        return 0;

    return BIO_callback_ctrl(next, cmd, fp);
}

static int md_gets(BIO *bp, char *buf, int size)
{
    EVP_MD_CTX *ctx;
    unsigned int ret;

    ctx = BIO_get_data(bp);

    if (size < EVP_MD_CTX_get_size(ctx))
        return 0;

    if (EVP_DigestFinal_ex(ctx, (unsigned char *)buf, &ret) <= 0)
        return -1;

    return (int)ret;
}
