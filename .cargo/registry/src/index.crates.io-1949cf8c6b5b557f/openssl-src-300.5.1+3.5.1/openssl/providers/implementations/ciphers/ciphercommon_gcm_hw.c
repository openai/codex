/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"
#include "prov/ciphercommon_gcm.h"


int ossl_gcm_setiv(PROV_GCM_CTX *ctx, const unsigned char *iv, size_t ivlen)
{
    CRYPTO_gcm128_setiv(&ctx->gcm, iv, ivlen);
    return 1;
}

int ossl_gcm_aad_update(PROV_GCM_CTX *ctx, const unsigned char *aad,
                        size_t aad_len)
{
    return CRYPTO_gcm128_aad(&ctx->gcm, aad, aad_len) == 0;
}

int ossl_gcm_cipher_update(PROV_GCM_CTX *ctx, const unsigned char *in,
                           size_t len, unsigned char *out)
{
    if (ctx->enc) {
        if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, len))
            return 0;
    } else {
        if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, len))
            return 0;
    }
    return 1;
}

int ossl_gcm_cipher_final(PROV_GCM_CTX *ctx, unsigned char *tag)
{
    if (ctx->enc) {
        CRYPTO_gcm128_tag(&ctx->gcm, tag, GCM_TAG_MAX_SIZE);
        ctx->taglen = GCM_TAG_MAX_SIZE;
    } else {
        if (CRYPTO_gcm128_finish(&ctx->gcm, tag, ctx->taglen) != 0)
            return 0;
    }
    return 1;
}

int ossl_gcm_one_shot(PROV_GCM_CTX *ctx, unsigned char *aad, size_t aad_len,
                      const unsigned char *in, size_t in_len,
                      unsigned char *out, unsigned char *tag, size_t tag_len)
{
    int ret = 0;

    /* Use saved AAD */
    if (!ctx->hw->aadupdate(ctx, aad, aad_len))
        goto err;
    if (!ctx->hw->cipherupdate(ctx, in, in_len, out))
        goto err;
    ctx->taglen = GCM_TAG_MAX_SIZE;
    if (!ctx->hw->cipherfinal(ctx, tag))
        goto err;
    ret = 1;

err:
    return ret;
}
