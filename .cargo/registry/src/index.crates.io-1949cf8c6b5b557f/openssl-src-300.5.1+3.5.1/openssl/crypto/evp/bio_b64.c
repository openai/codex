/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <errno.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/evp.h>
#include "internal/bio.h"

static int b64_write(BIO *h, const char *buf, int num);
static int b64_read(BIO *h, char *buf, int size);
static int b64_puts(BIO *h, const char *str);
static long b64_ctrl(BIO *h, int cmd, long arg1, void *arg2);
static int b64_new(BIO *h);
static int b64_free(BIO *data);
static long b64_callback_ctrl(BIO *h, int cmd, BIO_info_cb *fp);
#define B64_BLOCK_SIZE  1024
#define B64_BLOCK_SIZE2 768
#define B64_NONE        0
#define B64_ENCODE      1
#define B64_DECODE      2

typedef struct b64_struct {
    /*
     * BIO *bio; moved to the BIO structure
     */
    int buf_len;
    int buf_off;
    int tmp_len;                /* used to find the start when decoding */
    int tmp_nl;                 /* If true, scan until '\n' */
    int encode;
    int start;                  /* have we started decoding yet? */
    int cont;                   /* <= 0 when finished */
    EVP_ENCODE_CTX *base64;
    unsigned char buf[EVP_ENCODE_LENGTH(B64_BLOCK_SIZE) + 10];
    unsigned char tmp[B64_BLOCK_SIZE];
} BIO_B64_CTX;

static const BIO_METHOD methods_b64 = {
    BIO_TYPE_BASE64,
    "base64 encoding",
    bwrite_conv,
    b64_write,
    bread_conv,
    b64_read,
    b64_puts,
    NULL,                       /* b64_gets, */
    b64_ctrl,
    b64_new,
    b64_free,
    b64_callback_ctrl,
};

const BIO_METHOD *BIO_f_base64(void)
{
    return &methods_b64;
}

static int b64_new(BIO *bi)
{
    BIO_B64_CTX *ctx;

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) == NULL)
        return 0;

    ctx->cont = 1;
    ctx->start = 1;
    ctx->base64 = EVP_ENCODE_CTX_new();
    if (ctx->base64 == NULL) {
        OPENSSL_free(ctx);
        return 0;
    }

    BIO_set_data(bi, ctx);
    BIO_set_init(bi, 1);

    return 1;
}

static int b64_free(BIO *a)
{
    BIO_B64_CTX *ctx;

    if (a == NULL)
        return 0;

    ctx = BIO_get_data(a);
    if (ctx == NULL)
        return 0;

    EVP_ENCODE_CTX_free(ctx->base64);
    OPENSSL_free(ctx);
    BIO_set_data(a, NULL);
    BIO_set_init(a, 0);

    return 1;
}

/*
 * Unless `BIO_FLAGS_BASE64_NO_NL` is set, this BIO ignores leading lines that
 * aren't exclusively composed of valid Base64 characters (followed by <CRLF>
 * or <LF>).  Once a valid Base64 line is found, `ctx->start` is set to 0 and
 * lines are processed until EOF or the first line that contains invalid Base64
 * characters.  In a nod to PEM, lines that start with a '-' (hyphen) are
 * treated as a soft EOF, rather than an error.
 */
static int b64_read(BIO *b, char *out, int outl)
{
    int ret = 0, i, ii, j, k, x, n, num, ret_code;
    BIO_B64_CTX *ctx;
    unsigned char *p, *q;
    BIO *next;

    if (out == NULL)
        return 0;
    ctx = (BIO_B64_CTX *)BIO_get_data(b);

    next = BIO_next(b);
    if (ctx == NULL || next == NULL)
        return 0;

    BIO_clear_retry_flags(b);

    if (ctx->encode != B64_DECODE) {
        ctx->encode = B64_DECODE;
        ctx->buf_len = 0;
        ctx->buf_off = 0;
        ctx->tmp_len = 0;
        EVP_DecodeInit(ctx->base64);
    }

    /* First check if there are buffered bytes already decoded */
    if (ctx->buf_len > 0) {
        OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
        i = ctx->buf_len - ctx->buf_off;
        if (i > outl)
            i = outl;
        OPENSSL_assert(ctx->buf_off + i < (int)sizeof(ctx->buf));
        memcpy(out, &(ctx->buf[ctx->buf_off]), i);
        ret = i;
        out += i;
        outl -= i;
        ctx->buf_off += i;
        if (ctx->buf_len == ctx->buf_off) {
            ctx->buf_len = 0;
            ctx->buf_off = 0;
        }
    }

    /* Restore any non-retriable error condition (ctx->cont < 0) */
    ret_code = ctx->cont < 0 ? ctx->cont : 0;

    /*
     * At this point, we have room of outl bytes and an either an empty buffer,
     * or outl == 0, so we'll attempt to read in some more.
     */
    while (outl > 0) {
        int again = ctx->cont;

        if (again <= 0)
            break;

        i = BIO_read(next, &(ctx->tmp[ctx->tmp_len]),
                     B64_BLOCK_SIZE - ctx->tmp_len);

        if (i <= 0) {
            ret_code = i;

            /* Should we continue next time we are called? */
            if (!BIO_should_retry(next)) {
                /* Incomplete final Base64 chunk in the decoder is an error */
                if (ctx->tmp_len == 0) {
                    if (EVP_DecodeFinal(ctx->base64, NULL, &num) < 0)
                        ret_code = -1;
                    EVP_DecodeInit(ctx->base64);
                }
                ctx->cont = ret_code;
            }
            if (ctx->tmp_len == 0)
                break;
            /* Fall through and process what we have */
            i = 0;
            /* But don't loop to top-up even if the buffer is not full! */
            again = 0;
        }

        i += ctx->tmp_len;
        ctx->tmp_len = i;

        /*
         * We need to scan, a line at a time until we have a valid line if we
         * are starting.
         */
        if (ctx->start && (BIO_get_flags(b) & BIO_FLAGS_BASE64_NO_NL) != 0) {
            ctx->tmp_len = 0;
        } else if (ctx->start) {
            q = p = ctx->tmp;
            num = 0;
            for (j = 0; j < i; j++) {
                if (*(q++) != '\n')
                    continue;

                /*
                 * due to a previous very long line, we need to keep on
                 * scanning for a '\n' before we even start looking for
                 * base64 encoded stuff.
                 */
                if (ctx->tmp_nl) {
                    p = q;
                    ctx->tmp_nl = 0;
                    continue;
                }

                k = EVP_DecodeUpdate(ctx->base64, ctx->buf, &num, p, q - p);
                EVP_DecodeInit(ctx->base64);
                if (k <= 0 && num == 0) {
                    p = q;
                    continue;
                }

                ctx->start = 0;
                if (p != ctx->tmp) {
                    i -= p - ctx->tmp;
                    for (x = 0; x < i; x++)
                        ctx->tmp[x] = p[x];
                }
                break;
            }

            /* we fell off the end without starting */
            if (ctx->start) {
                /*
                 * Is this is one long chunk?, if so, keep on reading until a
                 * new line.
                 */
                if (p == ctx->tmp) {
                    /* Check buffer full */
                    if (i == B64_BLOCK_SIZE) {
                        ctx->tmp_nl = 1;
                        ctx->tmp_len = 0;
                    }
                } else if (p != q) {
                    /* Retain partial line at end of buffer */
                    n = q - p;
                    for (ii = 0; ii < n; ii++)
                        ctx->tmp[ii] = p[ii];
                    ctx->tmp_len = n;
                } else {
                    /* All we have is newline terminated non-start data */
                    ctx->tmp_len = 0;
                }
                /*
                 * Try to read more if possible, otherwise we can't make
                 * progress unless the underlying BIO is retriable and may
                 * produce more data next time we're called.
                 */
                if (again > 0)
                    continue;
                else
                    break;
            } else {
                ctx->tmp_len = 0;
            }
        } else if (i < B64_BLOCK_SIZE && again > 0) {
            /*
             * If buffer isn't full and we can retry then restart to read in
             * more data.
             */
            continue;
        }

        i = EVP_DecodeUpdate(ctx->base64, ctx->buf, &ctx->buf_len,
                             ctx->tmp, i);
        ctx->tmp_len = 0;
        /*
         * If eof or an error was signalled, then the condition
         * 'ctx->cont <= 0' will prevent b64_read() from reading
         * more data on subsequent calls. This assignment was
         * deleted accidentally in commit 5562cfaca4f3.
         */
        ctx->cont = i;

        ctx->buf_off = 0;
        if (i < 0) {
            ret_code = ctx->start ? 0 : i;
            ctx->buf_len = 0;
            break;
        }

        if (ctx->buf_len <= outl)
            i = ctx->buf_len;
        else
            i = outl;

        memcpy(out, ctx->buf, i);
        ret += i;
        ctx->buf_off = i;
        if (ctx->buf_off == ctx->buf_len) {
            ctx->buf_len = 0;
            ctx->buf_off = 0;
        }
        outl -= i;
        out += i;
    }
    /* BIO_clear_retry_flags(b); */
    BIO_copy_next_retry(b);
    return ret == 0 ? ret_code : ret;
}

static int b64_write(BIO *b, const char *in, int inl)
{
    int ret = 0;
    int n;
    int i;
    BIO_B64_CTX *ctx;
    BIO *next;

    ctx = (BIO_B64_CTX *)BIO_get_data(b);
    next = BIO_next(b);
    if (ctx == NULL || next == NULL)
        return 0;

    BIO_clear_retry_flags(b);

    if (ctx->encode != B64_ENCODE) {
        ctx->encode = B64_ENCODE;
        ctx->buf_len = 0;
        ctx->buf_off = 0;
        ctx->tmp_len = 0;
        EVP_EncodeInit(ctx->base64);
    }

    OPENSSL_assert(ctx->buf_off < (int)sizeof(ctx->buf));
    OPENSSL_assert(ctx->buf_len <= (int)sizeof(ctx->buf));
    OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
    n = ctx->buf_len - ctx->buf_off;
    while (n > 0) {
        i = BIO_write(next, &(ctx->buf[ctx->buf_off]), n);
        if (i <= 0) {
            BIO_copy_next_retry(b);
            return i;
        }
        OPENSSL_assert(i <= n);
        ctx->buf_off += i;
        OPENSSL_assert(ctx->buf_off <= (int)sizeof(ctx->buf));
        OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
        n -= i;
    }
    /* at this point all pending data has been written */
    ctx->buf_off = 0;
    ctx->buf_len = 0;

    if (in == NULL || inl <= 0)
        return 0;

    while (inl > 0) {
        n = inl > B64_BLOCK_SIZE ? B64_BLOCK_SIZE : inl;

        if ((BIO_get_flags(b) & BIO_FLAGS_BASE64_NO_NL) != 0) {
            if (ctx->tmp_len > 0) {
                OPENSSL_assert(ctx->tmp_len <= 3);
                n = 3 - ctx->tmp_len;
                /*
                 * There's a theoretical possibility for this
                 */
                if (n > inl)
                    n = inl;
                memcpy(&(ctx->tmp[ctx->tmp_len]), in, n);
                ctx->tmp_len += n;
                ret += n;
                if (ctx->tmp_len < 3)
                    break;
                ctx->buf_len =
                    EVP_EncodeBlock(ctx->buf, ctx->tmp, ctx->tmp_len);
                OPENSSL_assert(ctx->buf_len <= (int)sizeof(ctx->buf));
                OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
                /*
                 * Since we're now done using the temporary buffer, the
                 * length should be 0'd
                 */
                ctx->tmp_len = 0;
            } else {
                if (n < 3) {
                    memcpy(ctx->tmp, in, n);
                    ctx->tmp_len = n;
                    ret += n;
                    break;
                }
                n -= n % 3;
                ctx->buf_len =
                    EVP_EncodeBlock(ctx->buf, (unsigned char *)in, n);
                OPENSSL_assert(ctx->buf_len <= (int)sizeof(ctx->buf));
                OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
                ret += n;
            }
        } else {
            if (!EVP_EncodeUpdate(ctx->base64, ctx->buf, &ctx->buf_len,
                                  (unsigned char *)in, n))
                return ret == 0 ? -1 : ret;
            OPENSSL_assert(ctx->buf_len <= (int)sizeof(ctx->buf));
            OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
            ret += n;
        }
        inl -= n;
        in += n;

        ctx->buf_off = 0;
        n = ctx->buf_len;
        while (n > 0) {
            i = BIO_write(next, &(ctx->buf[ctx->buf_off]), n);
            if (i <= 0) {
                BIO_copy_next_retry(b);
                return ret == 0 ? i : ret;
            }
            OPENSSL_assert(i <= n);
            n -= i;
            ctx->buf_off += i;
            OPENSSL_assert(ctx->buf_off <= (int)sizeof(ctx->buf));
            OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
        }
        ctx->buf_len = 0;
        ctx->buf_off = 0;
    }
    return ret;
}

static long b64_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    BIO_B64_CTX *ctx;
    long ret = 1;
    int i;
    BIO *next;

    ctx = (BIO_B64_CTX *)BIO_get_data(b);
    next = BIO_next(b);
    if (ctx == NULL || next == NULL)
        return 0;

    switch (cmd) {
    case BIO_CTRL_RESET:
        ctx->cont = 1;
        ctx->start = 1;
        ctx->encode = B64_NONE;
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    case BIO_CTRL_EOF:         /* More to read */
        if (ctx->cont <= 0)
            ret = 1;
        else
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    case BIO_CTRL_WPENDING:    /* More to write in buffer */
        OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
        ret = ctx->buf_len - ctx->buf_off;
        if (ret == 0 && ctx->encode != B64_NONE
            && EVP_ENCODE_CTX_num(ctx->base64) != 0)
            ret = 1;
        else if (ret <= 0)
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    case BIO_CTRL_PENDING:     /* More to read in buffer */
        OPENSSL_assert(ctx->buf_len >= ctx->buf_off);
        ret = ctx->buf_len - ctx->buf_off;
        if (ret <= 0)
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    case BIO_CTRL_FLUSH:
        /* do a final write */
 again:
        while (ctx->buf_len != ctx->buf_off) {
            i = b64_write(b, NULL, 0);
            if (i < 0)
                return i;
        }
        if (BIO_get_flags(b) & BIO_FLAGS_BASE64_NO_NL) {
            if (ctx->tmp_len != 0) {
                ctx->buf_len = EVP_EncodeBlock(ctx->buf,
                                               ctx->tmp, ctx->tmp_len);
                ctx->buf_off = 0;
                ctx->tmp_len = 0;
                goto again;
            }
        } else if (ctx->encode != B64_NONE
                   && EVP_ENCODE_CTX_num(ctx->base64) != 0) {
            ctx->buf_off = 0;
            EVP_EncodeFinal(ctx->base64, ctx->buf, &(ctx->buf_len));
            /* push out the bytes */
            goto again;
        }
        /* Finally flush the underlying BIO */
        ret = BIO_ctrl(next, cmd, num, ptr);
        BIO_copy_next_retry(b);
        break;

    case BIO_C_DO_STATE_MACHINE:
        BIO_clear_retry_flags(b);
        ret = BIO_ctrl(next, cmd, num, ptr);
        BIO_copy_next_retry(b);
        break;

    case BIO_CTRL_DUP:
        break;
    case BIO_CTRL_INFO:
    case BIO_CTRL_GET:
    case BIO_CTRL_SET:
    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;
    }
    return ret;
}

static long b64_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    BIO *next = BIO_next(b);

    if (next == NULL)
        return 0;

    return BIO_callback_ctrl(next, cmd, fp);
}

static int b64_puts(BIO *b, const char *str)
{
    return b64_write(b, str, strlen(str));
}
