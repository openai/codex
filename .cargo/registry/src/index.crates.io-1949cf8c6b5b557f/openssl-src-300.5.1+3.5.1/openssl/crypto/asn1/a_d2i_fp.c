/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <limits.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include <openssl/buffer.h>
#include <openssl/asn1.h>
#include "internal/asn1.h"
#include "crypto/asn1.h"

#ifndef NO_OLD_ASN1
# ifndef OPENSSL_NO_STDIO

void *ASN1_d2i_fp(void *(*xnew) (void), d2i_of_void *d2i, FILE *in, void **x)
{
    BIO *b;
    void *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
        return NULL;
    }
    BIO_set_fp(b, in, BIO_NOCLOSE);
    ret = ASN1_d2i_bio(xnew, d2i, b, x);
    BIO_free(b);
    return ret;
}
# endif

void *ASN1_d2i_bio(void *(*xnew) (void), d2i_of_void *d2i, BIO *in, void **x)
{
    BUF_MEM *b = NULL;
    const unsigned char *p;
    void *ret = NULL;
    int len;

    len = asn1_d2i_read_bio(in, &b);
    if (len < 0)
        goto err;

    p = (unsigned char *)b->data;
    ret = d2i(x, &p, len);
 err:
    BUF_MEM_free(b);
    return ret;
}

#endif

void *ASN1_item_d2i_bio_ex(const ASN1_ITEM *it, BIO *in, void *x,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    BUF_MEM *b = NULL;
    const unsigned char *p;
    void *ret = NULL;
    int len;

    if (in == NULL)
        return NULL;
    len = asn1_d2i_read_bio(in, &b);
    if (len < 0)
        goto err;

    p = (const unsigned char *)b->data;
    ret = ASN1_item_d2i_ex(x, &p, len, it, libctx, propq);
 err:
    BUF_MEM_free(b);
    return ret;
}

void *ASN1_item_d2i_bio(const ASN1_ITEM *it, BIO *in, void *x)
{
    return ASN1_item_d2i_bio_ex(it, in, x, NULL, NULL);
}

#ifndef OPENSSL_NO_STDIO
void *ASN1_item_d2i_fp_ex(const ASN1_ITEM *it, FILE *in, void *x,
                          OSSL_LIB_CTX *libctx, const char *propq)
{
    BIO *b;
    char *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
        return NULL;
    }
    BIO_set_fp(b, in, BIO_NOCLOSE);
    ret = ASN1_item_d2i_bio_ex(it, b, x, libctx, propq);
    BIO_free(b);
    return ret;
}

void *ASN1_item_d2i_fp(const ASN1_ITEM *it, FILE *in, void *x)
{
    return ASN1_item_d2i_fp_ex(it, in, x, NULL, NULL);
}
#endif

#define HEADER_SIZE   8
#define ASN1_CHUNK_INITIAL_SIZE (16 * 1024)
int asn1_d2i_read_bio(BIO *in, BUF_MEM **pb)
{
    BUF_MEM *b;
    unsigned char *p;
    int i;
    size_t want = HEADER_SIZE;
    uint32_t eos = 0;
    size_t off = 0;
    size_t len = 0;
    size_t diff;

    const unsigned char *q;
    long slen;
    int inf, tag, xclass;

    b = BUF_MEM_new();
    if (b == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
        return -1;
    }

    ERR_set_mark();
    for (;;) {
        diff = len - off;
        if (want >= diff) {
            want -= diff;

            if (len + want < len || !BUF_MEM_grow_clean(b, len + want)) {
                ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
                goto err;
            }
            i = BIO_read(in, &(b->data[len]), want);
            if (i < 0 && diff == 0) {
                ERR_raise(ERR_LIB_ASN1, ASN1_R_NOT_ENOUGH_DATA);
                goto err;
            }
            if (i > 0) {
                if (len + i < len) {
                    ERR_raise(ERR_LIB_ASN1, ASN1_R_TOO_LONG);
                    goto err;
                }
                len += i;
                if ((size_t)i < want)
                    continue;

            }
        }
        /* else data already loaded */

        p = (unsigned char *)&(b->data[off]);
        q = p;
        diff = len - off;
        if (diff == 0)
            goto err;
        inf = ASN1_get_object(&q, &slen, &tag, &xclass, diff);
        if (inf & 0x80) {
            unsigned long e;

            e = ERR_GET_REASON(ERR_peek_last_error());
            if (e != ASN1_R_TOO_LONG)
                goto err;
            ERR_pop_to_mark();
        }
        i = q - p;            /* header length */
        off += i;               /* end of data */

        if (inf & 1) {
            /* no data body so go round again */
            if (eos == UINT32_MAX) {
                ERR_raise(ERR_LIB_ASN1, ASN1_R_HEADER_TOO_LONG);
                goto err;
            }
            eos++;
            want = HEADER_SIZE;
        } else if (eos && (slen == 0) && (tag == V_ASN1_EOC)) {
            /* eos value, so go back and read another header */
            eos--;
            if (eos == 0)
                break;
            else
                want = HEADER_SIZE;
        } else {
            /* suck in slen bytes of data */
            want = slen;
            if (want > (len - off)) {
                size_t chunk_max = ASN1_CHUNK_INITIAL_SIZE;

                want -= (len - off);
                if (want > INT_MAX /* BIO_read takes an int length */  ||
                    len + want < len) {
                    ERR_raise(ERR_LIB_ASN1, ASN1_R_TOO_LONG);
                    goto err;
                }
                while (want > 0) {
                    /*
                     * Read content in chunks of increasing size
                     * so we can return an error for EOF without
                     * having to allocate the entire content length
                     * in one go.
                     */
                    size_t chunk = want > chunk_max ? chunk_max : want;

                    if (!BUF_MEM_grow_clean(b, len + chunk)) {
                        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
                        goto err;
                    }
                    want -= chunk;
                    while (chunk > 0) {
                        i = BIO_read(in, &(b->data[len]), chunk);
                        if (i <= 0) {
                            ERR_raise(ERR_LIB_ASN1, ASN1_R_NOT_ENOUGH_DATA);
                            goto err;
                        }
                    /*
                     * This can't overflow because |len+want| didn't
                     * overflow.
                     */
                        len += i;
                        chunk -= i;
                    }
                    if (chunk_max < INT_MAX/2)
                        chunk_max *= 2;
                }
            }
            if (off + slen < off) {
                ERR_raise(ERR_LIB_ASN1, ASN1_R_TOO_LONG);
                goto err;
            }
            off += slen;
            if (eos == 0) {
                break;
            } else
                want = HEADER_SIZE;
        }
    }

    if (off > INT_MAX) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_TOO_LONG);
        goto err;
    }

    *pb = b;
    return off;
 err:
    ERR_clear_last_mark();
    BUF_MEM_free(b);
    return -1;
}
