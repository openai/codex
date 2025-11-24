/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * HMAC low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "internal/cryptlib.h"
#include <openssl/opensslconf.h>
#include <openssl/hmac.h>
#include <openssl/core_names.h>
#include "hmac_local.h"

int HMAC_Init_ex(HMAC_CTX *ctx, const void *key, int len,
                 const EVP_MD *md, ENGINE *impl)
{
    int rv = 0, reset = 0;
    int i, j;
    unsigned char pad[HMAC_MAX_MD_CBLOCK_SIZE];
    unsigned int keytmp_length;
    unsigned char keytmp[HMAC_MAX_MD_CBLOCK_SIZE];

    /* If we are changing MD then we must have a key */
    if (md != NULL && md != ctx->md && (key == NULL || len < 0))
        return 0;

    if (md != NULL)
        ctx->md = md;
    else if (ctx->md != NULL)
        md = ctx->md;
    else
        return 0;

    /*
     * The HMAC construction is not allowed to be used with the
     * extendable-output functions (XOF) shake128 and shake256.
     */
    if (EVP_MD_xof(md))
        return 0;

#ifdef OPENSSL_HMAC_S390X
    rv = s390x_HMAC_init(ctx, key, len, impl);
    if (rv >= 1)
        return rv;
#endif

    if (key != NULL) {
        reset = 1;

        j = EVP_MD_get_block_size(md);
        if (!ossl_assert(j <= (int)sizeof(keytmp)))
            return 0;
        if (j < 0)
            return 0;
        if (j < len) {
            if (!EVP_DigestInit_ex(ctx->md_ctx, md, impl)
                    || !EVP_DigestUpdate(ctx->md_ctx, key, len)
                    || !EVP_DigestFinal_ex(ctx->md_ctx, keytmp,
                                           &keytmp_length))
                return 0;
        } else {
            if (len < 0 || len > (int)sizeof(keytmp))
                return 0;
            memcpy(keytmp, key, len);
            keytmp_length = len;
        }
        if (keytmp_length != HMAC_MAX_MD_CBLOCK_SIZE)
            memset(&keytmp[keytmp_length], 0,
                   HMAC_MAX_MD_CBLOCK_SIZE - keytmp_length);

        for (i = 0; i < HMAC_MAX_MD_CBLOCK_SIZE; i++)
            pad[i] = 0x36 ^ keytmp[i];
        if (!EVP_DigestInit_ex(ctx->i_ctx, md, impl)
                || !EVP_DigestUpdate(ctx->i_ctx, pad,
                                     EVP_MD_get_block_size(md)))
            goto err;

        for (i = 0; i < HMAC_MAX_MD_CBLOCK_SIZE; i++)
            pad[i] = 0x5c ^ keytmp[i];
        if (!EVP_DigestInit_ex(ctx->o_ctx, md, impl)
                || !EVP_DigestUpdate(ctx->o_ctx, pad,
                                     EVP_MD_get_block_size(md)))
            goto err;
    }
    if (!EVP_MD_CTX_copy_ex(ctx->md_ctx, ctx->i_ctx))
        goto err;
    rv = 1;
 err:
    if (reset) {
        OPENSSL_cleanse(keytmp, sizeof(keytmp));
        OPENSSL_cleanse(pad, sizeof(pad));
    }
    return rv;
}

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
int HMAC_Init(HMAC_CTX *ctx, const void *key, int len, const EVP_MD *md)
{
    if (key && md)
        HMAC_CTX_reset(ctx);
    return HMAC_Init_ex(ctx, key, len, md, NULL);
}
#endif

int HMAC_Update(HMAC_CTX *ctx, const unsigned char *data, size_t len)
{
    if (!ctx->md)
        return 0;

#ifdef OPENSSL_HMAC_S390X
    if (ctx->plat.s390x.fc)
        return s390x_HMAC_update(ctx, data, len);
#endif

    return EVP_DigestUpdate(ctx->md_ctx, data, len);
}

int HMAC_Final(HMAC_CTX *ctx, unsigned char *md, unsigned int *len)
{
    unsigned int i;
    unsigned char buf[EVP_MAX_MD_SIZE];

    if (!ctx->md)
        goto err;

#ifdef OPENSSL_HMAC_S390X
    if (ctx->plat.s390x.fc)
        return s390x_HMAC_final(ctx, md, len);
#endif

    if (!EVP_DigestFinal_ex(ctx->md_ctx, buf, &i))
        goto err;
    if (!EVP_MD_CTX_copy_ex(ctx->md_ctx, ctx->o_ctx))
        goto err;
    if (!EVP_DigestUpdate(ctx->md_ctx, buf, i))
        goto err;
    if (!EVP_DigestFinal_ex(ctx->md_ctx, md, len))
        goto err;
    return 1;
 err:
    return 0;
}

size_t HMAC_size(const HMAC_CTX *ctx)
{
    int size = EVP_MD_get_size((ctx)->md);

    return (size < 0) ? 0 : size;
}

HMAC_CTX *HMAC_CTX_new(void)
{
    HMAC_CTX *ctx = OPENSSL_zalloc(sizeof(HMAC_CTX));

    if (ctx != NULL) {
        if (!HMAC_CTX_reset(ctx)) {
            HMAC_CTX_free(ctx);
            return NULL;
        }
    }
    return ctx;
}

static void hmac_ctx_cleanup(HMAC_CTX *ctx)
{
    EVP_MD_CTX_reset(ctx->i_ctx);
    EVP_MD_CTX_reset(ctx->o_ctx);
    EVP_MD_CTX_reset(ctx->md_ctx);
    ctx->md = NULL;

#ifdef OPENSSL_HMAC_S390X
    s390x_HMAC_CTX_cleanup(ctx);
#endif
}

void HMAC_CTX_free(HMAC_CTX *ctx)
{
    if (ctx != NULL) {
        hmac_ctx_cleanup(ctx);
        EVP_MD_CTX_free(ctx->i_ctx);
        EVP_MD_CTX_free(ctx->o_ctx);
        EVP_MD_CTX_free(ctx->md_ctx);
        OPENSSL_free(ctx);
    }
}

static int hmac_ctx_alloc_mds(HMAC_CTX *ctx)
{
    if (ctx->i_ctx == NULL)
        ctx->i_ctx = EVP_MD_CTX_new();
    if (ctx->i_ctx == NULL)
        return 0;
    if (ctx->o_ctx == NULL)
        ctx->o_ctx = EVP_MD_CTX_new();
    if (ctx->o_ctx == NULL)
        return 0;
    if (ctx->md_ctx == NULL)
        ctx->md_ctx = EVP_MD_CTX_new();
    if (ctx->md_ctx == NULL)
        return 0;
    return 1;
}

int HMAC_CTX_reset(HMAC_CTX *ctx)
{
    hmac_ctx_cleanup(ctx);
    if (!hmac_ctx_alloc_mds(ctx)) {
        hmac_ctx_cleanup(ctx);
        return 0;
    }
    return 1;
}

int HMAC_CTX_copy(HMAC_CTX *dctx, HMAC_CTX *sctx)
{
    if (!hmac_ctx_alloc_mds(dctx))
        goto err;
    if (!EVP_MD_CTX_copy_ex(dctx->i_ctx, sctx->i_ctx))
        goto err;
    if (!EVP_MD_CTX_copy_ex(dctx->o_ctx, sctx->o_ctx))
        goto err;
    if (!EVP_MD_CTX_copy_ex(dctx->md_ctx, sctx->md_ctx))
        goto err;
    dctx->md = sctx->md;

#ifdef OPENSSL_HMAC_S390X
    if (s390x_HMAC_CTX_copy(dctx, sctx) == 0)
        goto err;
#endif

    return 1;
 err:
    hmac_ctx_cleanup(dctx);
    return 0;
}

unsigned char *HMAC(const EVP_MD *evp_md, const void *key, int key_len,
                    const unsigned char *data, size_t data_len,
                    unsigned char *md, unsigned int *md_len)
{
    static unsigned char static_md[EVP_MAX_MD_SIZE];
    int size = EVP_MD_get_size(evp_md);
    size_t temp_md_len = 0;
    unsigned char *ret = NULL;

    if (size > 0) {
        ret = EVP_Q_mac(NULL, "HMAC", NULL, EVP_MD_get0_name(evp_md), NULL,
                        key, key_len, data, data_len,
                        md == NULL ? static_md : md, size, &temp_md_len);
        if (md_len != NULL)
            *md_len = (unsigned int)temp_md_len;
    }
    return ret;
}

void HMAC_CTX_set_flags(HMAC_CTX *ctx, unsigned long flags)
{
    EVP_MD_CTX_set_flags(ctx->i_ctx, flags);
    EVP_MD_CTX_set_flags(ctx->o_ctx, flags);
    EVP_MD_CTX_set_flags(ctx->md_ctx, flags);
}

const EVP_MD *HMAC_CTX_get_md(const HMAC_CTX *ctx)
{
    return ctx->md;
}
