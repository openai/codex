/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * AES low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

/* Dispatch functions for AES GCM mode */

#include "cipher_aes_gcm.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static void *aes_gcm_newctx(void *provctx, size_t keybits)
{
    PROV_AES_GCM_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ossl_gcm_initctx(provctx, &ctx->base, keybits,
                         ossl_prov_aes_hw_gcm(keybits));
    return ctx;
}

static void *aes_gcm_dupctx(void *provctx)
{
    PROV_AES_GCM_CTX *ctx = provctx;
    PROV_AES_GCM_CTX *dctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if (ctx == NULL)
        return NULL;

    dctx = OPENSSL_memdup(ctx, sizeof(*ctx));
    if (dctx != NULL && dctx->base.gcm.key != NULL)
        dctx->base.gcm.key = &dctx->ks.ks;

    return dctx;
}

static OSSL_FUNC_cipher_freectx_fn aes_gcm_freectx;
static void aes_gcm_freectx(void *vctx)
{
    PROV_AES_GCM_CTX *ctx = (PROV_AES_GCM_CTX *)vctx;

    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

/* ossl_aes128gcm_functions */
IMPLEMENT_aead_cipher(aes, gcm, GCM, AEAD_FLAGS, 128, 8, 96);
/* ossl_aes192gcm_functions */
IMPLEMENT_aead_cipher(aes, gcm, GCM, AEAD_FLAGS, 192, 8, 96);
/* ossl_aes256gcm_functions */
IMPLEMENT_aead_cipher(aes, gcm, GCM, AEAD_FLAGS, 256, 8, 96);
