/*
 * Copyright 2018-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <errno.h>
#include "bio_local.h"

static int prefix_write(BIO *b, const char *out, size_t outl,
                        size_t *numwritten);
static int prefix_read(BIO *b, char *buf, size_t size, size_t *numread);
static int prefix_puts(BIO *b, const char *str);
static int prefix_gets(BIO *b, char *str, int size);
static long prefix_ctrl(BIO *b, int cmd, long arg1, void *arg2);
static int prefix_create(BIO *b);
static int prefix_destroy(BIO *b);
static long prefix_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp);

static const BIO_METHOD prefix_meth = {
    BIO_TYPE_BUFFER,
    "prefix",
    prefix_write,
    NULL,
    prefix_read,
    NULL,
    prefix_puts,
    prefix_gets,
    prefix_ctrl,
    prefix_create,
    prefix_destroy,
    prefix_callback_ctrl,
};

const BIO_METHOD *BIO_f_prefix(void)
{
    return &prefix_meth;
}

typedef struct prefix_ctx_st {
    char *prefix;              /* Text prefix, given by user */
    unsigned int indent;       /* Indentation amount, given by user */

    int linestart;             /* flag to indicate we're at the line start */
} PREFIX_CTX;

static int prefix_create(BIO *b)
{
    PREFIX_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx == NULL)
        return 0;

    ctx->prefix = NULL;
    ctx->indent = 0;
    ctx->linestart = 1;
    BIO_set_data(b, ctx);
    BIO_set_init(b, 1);
    return 1;
}

static int prefix_destroy(BIO *b)
{
    PREFIX_CTX *ctx = BIO_get_data(b);

    OPENSSL_free(ctx->prefix);
    OPENSSL_free(ctx);
    return 1;
}

static int prefix_read(BIO *b, char *in, size_t size, size_t *numread)
{
    return BIO_read_ex(BIO_next(b), in, size, numread);
}

static int prefix_write(BIO *b, const char *out, size_t outl,
                        size_t *numwritten)
{
    PREFIX_CTX *ctx = BIO_get_data(b);

    if (ctx == NULL)
        return 0;

    /*
     * If no prefix is set or if it's empty, and no indentation amount is set,
     * we've got nothing to do here
     */
    if ((ctx->prefix == NULL || *ctx->prefix == '\0')
        && ctx->indent == 0) {
        /*
         * We do note if what comes next will be a new line, though, so we're
         * prepared to handle prefix and indentation the next time around.
         */
        if (outl > 0)
            ctx->linestart = (out[outl-1] == '\n');
        return BIO_write_ex(BIO_next(b), out, outl, numwritten);
    }

    *numwritten = 0;

    while (outl > 0) {
        size_t i;
        char c;

        /*
         * If we know that we're at the start of the line, output prefix and
         * indentation.
         */
        if (ctx->linestart) {
            size_t dontcare;

            if (ctx->prefix != NULL
                && !BIO_write_ex(BIO_next(b), ctx->prefix, strlen(ctx->prefix),
                                 &dontcare))
                return 0;
            BIO_printf(BIO_next(b), "%*s", ctx->indent, "");
            ctx->linestart = 0;
        }

        /* Now, go look for the next LF, or the end of the string */
        for (i = 0, c = '\0'; i < outl && (c = out[i]) != '\n'; i++)
            continue;
        if (c == '\n')
            i++;

        /* Output what we found so far */
        while (i > 0) {
            size_t num = 0;

            if (!BIO_write_ex(BIO_next(b), out, i, &num))
                return 0;
            out += num;
            outl -= num;
            *numwritten += num;
            i -= num;
        }

        /* If we found a LF, what follows is a new line, so take note */
        if (c == '\n')
            ctx->linestart = 1;
    }

    return 1;
}

static long prefix_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    long ret = 0;
    PREFIX_CTX *ctx;

    if (b == NULL || (ctx = BIO_get_data(b)) == NULL)
        return -1;

    switch (cmd) {
    case BIO_CTRL_SET_PREFIX:
        OPENSSL_free(ctx->prefix);
        if (ptr == NULL) {
            ctx->prefix = NULL;
            ret = 1;
        } else {
            ctx->prefix = OPENSSL_strdup((const char *)ptr);
            ret = ctx->prefix != NULL;
        }
        break;
    case BIO_CTRL_SET_INDENT:
        if (num >= 0) {
            ctx->indent = (unsigned int)num;
            ret = 1;
        }
        break;
    case BIO_CTRL_GET_INDENT:
        ret = (long)ctx->indent;
        break;
    default:
        /* Commands that we intercept before passing them along */
        switch (cmd) {
        case BIO_C_FILE_SEEK:
        case BIO_CTRL_RESET:
            ctx->linestart = 1;
            break;
        }
        if (BIO_next(b) != NULL)
            ret = BIO_ctrl(BIO_next(b), cmd, num, ptr);
        break;
    }
    return ret;
}

static long prefix_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    return BIO_callback_ctrl(BIO_next(b), cmd, fp);
}

static int prefix_gets(BIO *b, char *buf, int size)
{
    return BIO_gets(BIO_next(b), buf, size);
}

static int prefix_puts(BIO *b, const char *str)
{
    return BIO_write(b, str, strlen(str));
}
