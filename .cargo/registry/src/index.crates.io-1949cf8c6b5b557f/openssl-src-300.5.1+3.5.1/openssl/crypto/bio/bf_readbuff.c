/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is a read only BIO filter that can be used to add BIO_tell() and
 * BIO_seek() support to source/sink BIO's (such as a file BIO that uses stdin).
 * It does this by caching ALL data read from the BIO source/sink into a
 * resizable memory buffer.
 */

#include <stdio.h>
#include <errno.h>
#include "bio_local.h"
#include "internal/cryptlib.h"

#define DEFAULT_BUFFER_SIZE     4096

static int readbuffer_write(BIO *h, const char *buf, int num);
static int readbuffer_read(BIO *h, char *buf, int size);
static int readbuffer_puts(BIO *h, const char *str);
static int readbuffer_gets(BIO *h, char *str, int size);
static long readbuffer_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int readbuffer_new(BIO *h);
static int readbuffer_free(BIO *data);
static long readbuffer_callback_ctrl(BIO *h, int cmd, BIO_info_cb *fp);

static const BIO_METHOD methods_readbuffer = {
    BIO_TYPE_BUFFER,
    "readbuffer",
    bwrite_conv,
    readbuffer_write,
    bread_conv,
    readbuffer_read,
    readbuffer_puts,
    readbuffer_gets,
    readbuffer_ctrl,
    readbuffer_new,
    readbuffer_free,
    readbuffer_callback_ctrl,
};

const BIO_METHOD *BIO_f_readbuffer(void)
{
    return &methods_readbuffer;
}

static int readbuffer_new(BIO *bi)
{
    BIO_F_BUFFER_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx == NULL)
        return 0;
    ctx->ibuf_size = DEFAULT_BUFFER_SIZE;
    ctx->ibuf = OPENSSL_zalloc(DEFAULT_BUFFER_SIZE);
    if (ctx->ibuf == NULL) {
        OPENSSL_free(ctx);
        return 0;
    }

    bi->init = 1;
    bi->ptr = (char *)ctx;
    bi->flags = 0;
    return 1;
}

static int readbuffer_free(BIO *a)
{
    BIO_F_BUFFER_CTX *b;

    if (a == NULL)
        return 0;
    b = (BIO_F_BUFFER_CTX *)a->ptr;
    OPENSSL_free(b->ibuf);
    OPENSSL_free(a->ptr);
    a->ptr = NULL;
    a->init = 0;
    a->flags = 0;
    return 1;
}

static int readbuffer_resize(BIO_F_BUFFER_CTX *ctx, int sz)
{
    char *tmp;

    /* Figure out how many blocks are required */
    sz += (ctx->ibuf_off + DEFAULT_BUFFER_SIZE - 1);
    sz = DEFAULT_BUFFER_SIZE * (sz / DEFAULT_BUFFER_SIZE);

    /* Resize if the buffer is not big enough */
    if (sz > ctx->ibuf_size) {
        tmp = OPENSSL_realloc(ctx->ibuf, sz);
        if (tmp == NULL)
            return 0;
        ctx->ibuf = tmp;
        ctx->ibuf_size = sz;
    }
    return 1;
}

static int readbuffer_read(BIO *b, char *out, int outl)
{
    int i, num = 0;
    BIO_F_BUFFER_CTX *ctx;

    if (out == NULL || outl == 0)
        return 0;
    ctx = (BIO_F_BUFFER_CTX *)b->ptr;

    if ((ctx == NULL) || (b->next_bio == NULL))
        return 0;
    BIO_clear_retry_flags(b);

    for (;;) {
        i = ctx->ibuf_len;
        /* If there is something in the buffer just read it. */
        if (i != 0) {
            if (i > outl)
                i = outl;
            memcpy(out, &(ctx->ibuf[ctx->ibuf_off]), i);
            ctx->ibuf_off += i;
            ctx->ibuf_len -= i;
            num += i;
            /* Exit if we have read the bytes required out of the buffer */
            if (outl == i)
                return num;
            outl -= i;
            out += i;
        }

        /* Only gets here if the buffer has been consumed */
        if (!readbuffer_resize(ctx, outl))
            return 0;

        /* Do some buffering by reading from the next bio */
        i = BIO_read(b->next_bio, ctx->ibuf + ctx->ibuf_off, outl);
        if (i <= 0) {
            BIO_copy_next_retry(b);
            if (i < 0)
                return ((num > 0) ? num : i);
            else
                return num; /* i == 0 */
        }
        ctx->ibuf_len = i;
    }
}

static int readbuffer_write(BIO *b, const char *in, int inl)
{
    return 0;
}
static int readbuffer_puts(BIO *b, const char *str)
{
    return 0;
}

static long readbuffer_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    BIO_F_BUFFER_CTX *ctx;
    long ret = 1, sz;

    ctx = (BIO_F_BUFFER_CTX *)b->ptr;

    switch (cmd) {
    case BIO_CTRL_EOF:
        if (ctx->ibuf_len > 0)
            return 0;
        if (b->next_bio == NULL)
            return 1;
        ret = BIO_ctrl(b->next_bio, cmd, num, ptr);
        break;

    case BIO_C_FILE_SEEK:
    case BIO_CTRL_RESET:
        sz = ctx->ibuf_off + ctx->ibuf_len;
        /* Assume it can only seek backwards */
        if (num < 0 || num > sz)
            return 0;
        ctx->ibuf_off = num;
        ctx->ibuf_len = sz - num;
        break;

    case BIO_C_FILE_TELL:
    case BIO_CTRL_INFO:
        ret = (long)ctx->ibuf_off;
        break;
    case BIO_CTRL_PENDING:
        ret = (long)ctx->ibuf_len;
        if (ret == 0) {
            if (b->next_bio == NULL)
                return 0;
            ret = BIO_ctrl(b->next_bio, cmd, num, ptr);
        }
        break;
    case BIO_CTRL_DUP:
    case BIO_CTRL_FLUSH:
        ret = 1;
        break;
    default:
        ret = 0;
        break;
    }
    return ret;
}

static long readbuffer_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    if (b->next_bio == NULL)
        return 0;
    return BIO_callback_ctrl(b->next_bio, cmd, fp);
}

static int readbuffer_gets(BIO *b, char *buf, int size)
{
    BIO_F_BUFFER_CTX *ctx;
    int num = 0, num_chars, found_newline;
    char *p;
    int i, j;

    if (buf == NULL || size == 0)
        return 0;
    --size; /* the passed in size includes the terminator - so remove it here */
    ctx = (BIO_F_BUFFER_CTX *)b->ptr;

    if (ctx == NULL || b->next_bio == NULL)
        return 0;
    BIO_clear_retry_flags(b);

    /* If data is already buffered then use this first */
    if (ctx->ibuf_len > 0) {
        p = ctx->ibuf + ctx->ibuf_off;
        found_newline = 0;
        for (num_chars = 0;
             (num_chars < ctx->ibuf_len) && (num_chars < size);
             num_chars++) {
            *buf++ = p[num_chars];
            if (p[num_chars] == '\n') {
                found_newline = 1;
                num_chars++;
                break;
            }
        }
        num += num_chars;
        size -= num_chars;
        ctx->ibuf_len -= num_chars;
        ctx->ibuf_off += num_chars;
        if (found_newline || size == 0) {
            *buf = '\0';
            return num;
        }
    }
    /*
     * If there is no buffered data left then read any remaining data from the
     * next bio.
     */

     /* Resize if we have to */
     if (!readbuffer_resize(ctx, 1 + size))
         return 0;
     /*
      * Read more data from the next bio using BIO_read_ex:
      * Note we cannot use BIO_gets() here as it does not work on a
      * binary stream that contains 0x00. (Since strlen() will stop at
      * any 0x00 not at the last read '\n' in a FILE bio).
      * Also note that some applications open and close the file bio
      * multiple times and need to read the next available block when using
      * stdin - so we need to READ one byte at a time!
      */
     p = ctx->ibuf + ctx->ibuf_off;
     for (i = 0; i < size; ++i) {
         j = BIO_read(b->next_bio, p, 1);
         if (j <= 0) {
             BIO_copy_next_retry(b);
             *buf = '\0';
             return num > 0 ? num : j;
         }
         *buf++ = *p;
         num++;
         ctx->ibuf_off++;
         if (*p == '\n')
             break;
         ++p;
     }
     *buf = '\0';
     return num;
}
