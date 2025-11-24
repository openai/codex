/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/x509.h>

X509_PKEY *X509_PKEY_new(void)
{
    X509_PKEY *ret = NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;

    ret->enc_algor = X509_ALGOR_new();
    ret->enc_pkey = ASN1_OCTET_STRING_new();
    if (ret->enc_algor == NULL || ret->enc_pkey == NULL) {
        X509_PKEY_free(ret);
        ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
        return NULL;
    }

    return ret;
}

void X509_PKEY_free(X509_PKEY *x)
{
    if (x == NULL)
        return;

    X509_ALGOR_free(x->enc_algor);
    ASN1_OCTET_STRING_free(x->enc_pkey);
    EVP_PKEY_free(x->dec_pkey);
    if (x->key_free)
        OPENSSL_free(x->key_data);
    OPENSSL_free(x);
}
