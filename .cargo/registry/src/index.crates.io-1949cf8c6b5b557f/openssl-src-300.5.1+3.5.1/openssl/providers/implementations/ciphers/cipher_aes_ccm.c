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

/* Dispatch functions for AES CCM mode */

#include "cipher_aes_ccm.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

static void *aes_ccm_newctx(void *provctx, size_t keybits)
{
    PROV_AES_CCM_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        ossl_ccm_initctx(&ctx->base, keybits, ossl_prov_aes_hw_ccm(keybits));
    return ctx;
}

static void *aes_ccm_dupctx(void *provctx)
{
    PROV_AES_CCM_CTX *ctx = provctx;
    PROV_AES_CCM_CTX *dupctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if (ctx == NULL)
        return NULL;
    dupctx = OPENSSL_memdup(provctx, sizeof(*ctx));
    if (dupctx == NULL)
        return NULL;
    /*
     * ossl_cm_initctx, via the ossl_prov_aes_hw_ccm functions assign a
     * provctx->ccm.ks.ks to the ccm context key so we need to point it to
     * the memduped copy
     */
    dupctx->base.ccm_ctx.key = &dupctx->ccm.ks.ks;

    return dupctx;
}

static OSSL_FUNC_cipher_freectx_fn aes_ccm_freectx;
static void aes_ccm_freectx(void *vctx)
{
    PROV_AES_CCM_CTX *ctx = (PROV_AES_CCM_CTX *)vctx;

    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

/* ossl_aes128ccm_functions */
IMPLEMENT_aead_cipher(aes, ccm, CCM, AEAD_FLAGS, 128, 8, 96);
/* ossl_aes192ccm_functions */
IMPLEMENT_aead_cipher(aes, ccm, CCM, AEAD_FLAGS, 192, 8, 96);
/* ossl_aes256ccm_functions */
IMPLEMENT_aead_cipher(aes, ccm, CCM, AEAD_FLAGS, 256, 8, 96);
