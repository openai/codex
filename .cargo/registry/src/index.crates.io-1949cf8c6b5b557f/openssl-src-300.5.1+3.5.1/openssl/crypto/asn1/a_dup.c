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
#include <openssl/asn1t.h>

#ifndef NO_OLD_ASN1

void *ASN1_dup(i2d_of_void *i2d, d2i_of_void *d2i, const void *x)
{
    unsigned char *b, *p;
    const unsigned char *p2;
    int i;
    char *ret;

    if (x == NULL)
        return NULL;

    i = i2d(x, NULL);
    if (i <= 0)
        return NULL;

    b = OPENSSL_malloc(i + 10);
    if (b == NULL)
        return NULL;
    p = b;
    i = i2d(x, &p);
    p2 = b;
    ret = d2i(NULL, &p2, i);
    OPENSSL_free(b);
    return ret;
}

#endif

/*
 * ASN1_ITEM version of dup: this follows the model above except we don't
 * need to allocate the buffer. At some point this could be rewritten to
 * directly dup the underlying structure instead of doing and encode and
 * decode.
 */

void *ASN1_item_dup(const ASN1_ITEM *it, const void *x)
{
    ASN1_aux_cb *asn1_cb = NULL;
    unsigned char *b = NULL;
    const unsigned char *p;
    long i;
    ASN1_VALUE *ret;
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (x == NULL)
        return NULL;

    if (it->itype == ASN1_ITYPE_SEQUENCE || it->itype == ASN1_ITYPE_CHOICE
        || it->itype == ASN1_ITYPE_NDEF_SEQUENCE) {
        const ASN1_AUX *aux = it->funcs;

        asn1_cb = aux != NULL ? aux->asn1_cb : NULL;
    }

    if (asn1_cb != NULL) {
        if (!asn1_cb(ASN1_OP_DUP_PRE, (ASN1_VALUE **)&x, it, NULL)
                || !asn1_cb(ASN1_OP_GET0_LIBCTX, (ASN1_VALUE **)&x, it, &libctx)
                || !asn1_cb(ASN1_OP_GET0_PROPQ, (ASN1_VALUE **)&x, it, &propq))
            goto auxerr;
    }

    i = ASN1_item_i2d(x, &b, it);
    if (i < 0 || b == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
        return NULL;
    }
    p = b;
    ret = ASN1_item_d2i_ex(NULL, &p, i, it, libctx, propq);
    OPENSSL_free(b);

    if (asn1_cb != NULL
        && !asn1_cb(ASN1_OP_DUP_POST, &ret, it, (void *)x))
        goto auxerr;

    return ret;

 auxerr:
    ERR_raise_data(ERR_LIB_ASN1, ASN1_R_AUX_ERROR, "Type=%s", it->sname);
    return NULL;
}
