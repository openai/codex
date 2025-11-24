/*
 * Copyright 1999-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/pkcs12.h>
#include "crypto/pkcs7.h"
#include "p12_local.h"

/* Initialise a PKCS12 structure to take data */

PKCS12 *PKCS12_init_ex(int mode, OSSL_LIB_CTX *ctx, const char *propq)
{
    PKCS12 *pkcs12;

    if ((pkcs12 = PKCS12_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    if (!ASN1_INTEGER_set(pkcs12->version, 3))
        goto err;
    pkcs12->authsafes->type = OBJ_nid2obj(mode);

    ossl_pkcs7_set0_libctx(pkcs12->authsafes, ctx);
    if (!ossl_pkcs7_set1_propq(pkcs12->authsafes, propq)) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_PKCS7_LIB);
        goto err;
    }

    switch (mode) {
    case NID_pkcs7_data:
        if ((pkcs12->authsafes->d.data = ASN1_OCTET_STRING_new()) == NULL) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
            goto err;
        }
        break;
    default:
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_UNSUPPORTED_PKCS12_MODE);
        goto err;
    }
    return pkcs12;

 err:
    PKCS12_free(pkcs12);
    return NULL;
}

PKCS12 *PKCS12_init(int mode)
{
    return PKCS12_init_ex(mode, NULL, NULL);
}

const PKCS7_CTX *ossl_pkcs12_get0_pkcs7ctx(const PKCS12 *p12)
{
    if (p12 == NULL || p12->authsafes == NULL)
        return NULL;
    return &p12->authsafes->ctx;
}
