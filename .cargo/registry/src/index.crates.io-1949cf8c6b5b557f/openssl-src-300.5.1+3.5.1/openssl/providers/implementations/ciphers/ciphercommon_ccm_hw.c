/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"
#include "prov/ciphercommon_ccm.h"

int ossl_ccm_generic_setiv(PROV_CCM_CTX *ctx, const unsigned char *nonce,
                           size_t nlen, size_t mlen)
{
    return CRYPTO_ccm128_setiv(&ctx->ccm_ctx, nonce, nlen, mlen) == 0;
}

int ossl_ccm_generic_setaad(PROV_CCM_CTX *ctx, const unsigned char *aad,
                            size_t alen)
{
    CRYPTO_ccm128_aad(&ctx->ccm_ctx, aad, alen);
    return 1;
}

int ossl_ccm_generic_gettag(PROV_CCM_CTX *ctx, unsigned char *tag, size_t tlen)
{
    return CRYPTO_ccm128_tag(&ctx->ccm_ctx, tag, tlen) > 0;
}

int ossl_ccm_generic_auth_encrypt(PROV_CCM_CTX *ctx, const unsigned char *in,
                                  unsigned char *out, size_t len,
                                  unsigned char *tag, size_t taglen)
{
    int rv;

    if (ctx->str != NULL)
        rv = CRYPTO_ccm128_encrypt_ccm64(&ctx->ccm_ctx, in,
                                         out, len, ctx->str) == 0;
    else
        rv = CRYPTO_ccm128_encrypt(&ctx->ccm_ctx, in, out, len) == 0;

    if (rv == 1 && tag != NULL)
        rv = (CRYPTO_ccm128_tag(&ctx->ccm_ctx, tag, taglen) > 0);
    return rv;
}

int ossl_ccm_generic_auth_decrypt(PROV_CCM_CTX *ctx, const unsigned char *in,
                                  unsigned char *out, size_t len,
                                  unsigned char *expected_tag, size_t taglen)
{
    int rv = 0;

    if (ctx->str != NULL)
        rv = CRYPTO_ccm128_decrypt_ccm64(&ctx->ccm_ctx, in, out, len,
                                         ctx->str) == 0;
    else
        rv = CRYPTO_ccm128_decrypt(&ctx->ccm_ctx, in, out, len) == 0;
    if (rv) {
        unsigned char tag[16];

        if (!CRYPTO_ccm128_tag(&ctx->ccm_ctx, tag, taglen)
            || CRYPTO_memcmp(tag, expected_tag, taglen) != 0)
            rv = 0;
    }
    if (rv == 0)
        OPENSSL_cleanse(out, len);
    return rv;
}
