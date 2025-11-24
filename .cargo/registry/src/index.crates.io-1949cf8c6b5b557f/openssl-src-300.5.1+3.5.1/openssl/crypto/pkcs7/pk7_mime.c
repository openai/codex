/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/x509.h>
#include <openssl/asn1.h>
#include "pk7_local.h"

/* PKCS#7 wrappers round generalised stream and MIME routines */

int i2d_PKCS7_bio_stream(BIO *out, PKCS7 *p7, BIO *in, int flags)
{
    return i2d_ASN1_bio_stream(out, (ASN1_VALUE *)p7, in, flags,
                               ASN1_ITEM_rptr(PKCS7));
}

int PEM_write_bio_PKCS7_stream(BIO *out, PKCS7 *p7, BIO *in, int flags)
{
    return PEM_write_bio_ASN1_stream(out, (ASN1_VALUE *)p7, in, flags,
                                     "PKCS7", ASN1_ITEM_rptr(PKCS7));
}

int SMIME_write_PKCS7(BIO *bio, PKCS7 *p7, BIO *data, int flags)
{
    STACK_OF(X509_ALGOR) *mdalgs;
    int ctype_nid = OBJ_obj2nid(p7->type);
    const PKCS7_CTX *ctx = ossl_pkcs7_get0_ctx(p7);

    if (ctype_nid == NID_pkcs7_signed) {
        if (p7->d.sign == NULL)
            return 0;
        mdalgs = p7->d.sign->md_algs;
    } else {
        mdalgs = NULL;
    }

    flags ^= SMIME_OLDMIME;

    return SMIME_write_ASN1_ex(bio, (ASN1_VALUE *)p7, data, flags, ctype_nid,
                               NID_undef, mdalgs, ASN1_ITEM_rptr(PKCS7),
                               ossl_pkcs7_ctx_get0_libctx(ctx),
                               ossl_pkcs7_ctx_get0_propq(ctx));
}

PKCS7 *SMIME_read_PKCS7_ex(BIO *bio, BIO **bcont, PKCS7 **p7)
{
    PKCS7 *ret;
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (p7 != NULL && *p7 != NULL) {
        libctx = (*p7)->ctx.libctx;
        propq = (*p7)->ctx.propq;
    }

    ret = (PKCS7 *)SMIME_read_ASN1_ex(bio, 0, bcont, ASN1_ITEM_rptr(PKCS7),
                                      (ASN1_VALUE **)p7, libctx, propq);
    if (ret != NULL)
        ossl_pkcs7_resolve_libctx(ret);
    return ret;
}

PKCS7 *SMIME_read_PKCS7(BIO *bio, BIO **bcont)
{
    return SMIME_read_PKCS7_ex(bio, bcont, NULL);
}
