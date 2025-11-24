/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
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
#include "crypto/evp.h"

int EVP_SignFinal_ex(EVP_MD_CTX *ctx, unsigned char *sigret,
                     unsigned int *siglen, EVP_PKEY *pkey, OSSL_LIB_CTX *libctx,
                     const char *propq)
{
    unsigned char m[EVP_MAX_MD_SIZE];
    unsigned int m_len = 0;
    int i = 0;
    size_t sltmp;
    EVP_PKEY_CTX *pkctx = NULL;

    *siglen = 0;
    if (EVP_MD_CTX_test_flags(ctx, EVP_MD_CTX_FLAG_FINALISE)) {
        if (!EVP_DigestFinal_ex(ctx, m, &m_len))
            goto err;
    } else {
        int rv = 0;
        EVP_MD_CTX *tmp_ctx = EVP_MD_CTX_new();

        if (tmp_ctx == NULL) {
            ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
            return 0;
        }
        rv = EVP_MD_CTX_copy_ex(tmp_ctx, ctx);
        if (rv)
            rv = EVP_DigestFinal_ex(tmp_ctx, m, &m_len);
        else
            rv = EVP_DigestFinal_ex(ctx, m, &m_len);
        EVP_MD_CTX_free(tmp_ctx);
        if (!rv)
            return 0;
    }

    sltmp = (size_t)EVP_PKEY_get_size(pkey);
    i = 0;
    pkctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propq);
    if (pkctx == NULL)
        goto err;
    if (EVP_PKEY_sign_init(pkctx) <= 0)
        goto err;
    if (EVP_PKEY_CTX_set_signature_md(pkctx, EVP_MD_CTX_get0_md(ctx)) <= 0)
        goto err;
    if (EVP_PKEY_sign(pkctx, sigret, &sltmp, m, m_len) <= 0)
        goto err;
    *siglen = sltmp;
    i = 1;
 err:
    EVP_PKEY_CTX_free(pkctx);
    return i;
}

int EVP_SignFinal(EVP_MD_CTX *ctx, unsigned char *sigret,
                  unsigned int *siglen, EVP_PKEY *pkey)
{
    return EVP_SignFinal_ex(ctx, sigret, siglen, pkey, NULL, NULL);
}
