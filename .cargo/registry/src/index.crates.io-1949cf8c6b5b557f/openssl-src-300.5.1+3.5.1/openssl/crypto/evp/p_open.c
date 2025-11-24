/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"

#include <stdio.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/rsa.h>

int EVP_OpenInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *type,
                 const unsigned char *ek, int ekl, const unsigned char *iv,
                 EVP_PKEY *priv)
{
    unsigned char *key = NULL;
    size_t keylen = 0;
    int ret = 0;
    EVP_PKEY_CTX *pctx = NULL;

    if (type) {
        EVP_CIPHER_CTX_reset(ctx);
        if (!EVP_DecryptInit_ex(ctx, type, NULL, NULL, NULL))
            goto err;
    }

    if (priv == NULL)
        return 1;

    if ((pctx = EVP_PKEY_CTX_new(priv, NULL)) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        goto err;
    }

    if (EVP_PKEY_decrypt_init(pctx) <= 0
        || EVP_PKEY_decrypt(pctx, NULL, &keylen, ek, ekl) <= 0)
        goto err;

    if ((key = OPENSSL_malloc(keylen)) == NULL)
        goto err;

    if (EVP_PKEY_decrypt(pctx, key, &keylen, ek, ekl) <= 0)
        goto err;

    if (EVP_CIPHER_CTX_set_key_length(ctx, keylen) <= 0
        || !EVP_DecryptInit_ex(ctx, NULL, NULL, key, iv))
        goto err;

    ret = 1;
 err:
    EVP_PKEY_CTX_free(pctx);
    OPENSSL_clear_free(key, keylen);
    return ret;
}

int EVP_OpenFinal(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    int i;

    i = EVP_DecryptFinal_ex(ctx, out, outl);
    if (i)
        i = EVP_DecryptInit_ex(ctx, NULL, NULL, NULL, NULL);
    return i;
}
