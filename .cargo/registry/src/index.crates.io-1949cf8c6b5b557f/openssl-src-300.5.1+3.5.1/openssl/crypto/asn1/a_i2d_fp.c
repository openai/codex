/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/asn1.h>

#ifndef NO_OLD_ASN1

# ifndef OPENSSL_NO_STDIO
int ASN1_i2d_fp(i2d_of_void *i2d, FILE *out, const void *x)
{
    BIO *b;
    int ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, out, BIO_NOCLOSE);
    ret = ASN1_i2d_bio(i2d, b, x);
    BIO_free(b);
    return ret;
}
# endif

int ASN1_i2d_bio(i2d_of_void *i2d, BIO *out, const void *x)
{
    char *b;
    unsigned char *p;
    int i, j = 0, n, ret = 1;

    n = i2d(x, NULL);
    if (n <= 0)
        return 0;

    b = OPENSSL_malloc(n);
    if (b == NULL)
        return 0;

    p = (unsigned char *)b;
    i2d(x, &p);

    for (;;) {
        i = BIO_write(out, &(b[j]), n);
        if (i == n)
            break;
        if (i <= 0) {
            ret = 0;
            break;
        }
        j += i;
        n -= i;
    }
    OPENSSL_free(b);
    return ret;
}

#endif

#ifndef OPENSSL_NO_STDIO
int ASN1_item_i2d_fp(const ASN1_ITEM *it, FILE *out, const void *x)
{
    BIO *b;
    int ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, out, BIO_NOCLOSE);
    ret = ASN1_item_i2d_bio(it, b, x);
    BIO_free(b);
    return ret;
}
#endif

int ASN1_item_i2d_bio(const ASN1_ITEM *it, BIO *out, const void *x)
{
    unsigned char *b = NULL;
    int i, j = 0, n, ret = 1;

    n = ASN1_item_i2d(x, &b, it);
    if (n < 0 || b == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
        return 0;
    }

    for (;;) {
        i = BIO_write(out, &(b[j]), n);
        if (i == n)
            break;
        if (i <= 0) {
            ret = 0;
            break;
        }
        j += i;
        n -= i;
    }
    OPENSSL_free(b);
    return ret;
}

BIO *ASN1_item_i2d_mem_bio(const ASN1_ITEM *it, const ASN1_VALUE *val)
{
    BIO *res;

    if (it == NULL || val == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((res = BIO_new(BIO_s_mem())) == NULL)
        return NULL;
    if (ASN1_item_i2d_bio(it, res, val) <= 0) {
        BIO_free(res);
        res = NULL;
    }
    return res;
}
