/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "crypto/s390x_arch.h"
#include "hmac_local.h"
#include "openssl/obj_mac.h"
#include "openssl/evp.h"
#include "openssl/err.h"
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
# include <openssl/engine.h>
#endif

#ifdef OPENSSL_HMAC_S390X

static int s390x_fc_from_md(const EVP_MD *md)
{
    int fc;

    if (EVP_MD_is_a(md, "SHA2-224"))
        fc = S390X_HMAC_SHA_224;
    else if (EVP_MD_is_a(md, "SHA2-256"))
        fc = S390X_HMAC_SHA_256;
    else if (EVP_MD_is_a(md, "SHA2-384"))
        fc = S390X_HMAC_SHA_384;
    else if (EVP_MD_is_a(md, "SHA2-512"))
        fc = S390X_HMAC_SHA_512;
    else
        return 0;

    if ((OPENSSL_s390xcap_P.kmac[1] & S390X_CAPBIT(fc)) == 0)
        return 0;

    return fc;
}

static void s390x_call_kmac(HMAC_CTX *ctx, const unsigned char *in, size_t len)
{
    unsigned int fc = ctx->plat.s390x.fc;

    if (ctx->plat.s390x.ikp)
        fc |= S390X_KMAC_IKP;

    if (ctx->plat.s390x.iimp)
        fc |= S390X_KMAC_IIMP;

    switch (ctx->plat.s390x.fc) {
    case S390X_HMAC_SHA_224:
    case S390X_HMAC_SHA_256:
        ctx->plat.s390x.param.hmac_224_256.imbl += ((uint64_t)len * 8);
        break;
    case S390X_HMAC_SHA_384:
    case S390X_HMAC_SHA_512:
        ctx->plat.s390x.param.hmac_384_512.imbl += ((uint128_t)len * 8);
        break;
    default:
        break;
    }

    s390x_kmac(in, len, fc, &ctx->plat.s390x.param);

    ctx->plat.s390x.ikp = 1;
}

static int s390x_check_engine_used(const EVP_MD *md, ENGINE *impl)
{
# if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    const EVP_MD *d;

    if (impl != NULL) {
        if (!ENGINE_init(impl))
            return 0;
    } else {
        impl = ENGINE_get_digest_engine(EVP_MD_get_type(md));
    }

    if (impl == NULL)
        return 0;

    d = ENGINE_get_digest(impl, EVP_MD_get_type(md));
    ENGINE_finish(impl);

    if (d != NULL)
        return 1;
# endif

    return 0;
}

int s390x_HMAC_init(HMAC_CTX *ctx, const void *key, int key_len, ENGINE *impl)
{
    unsigned char *key_param;
    unsigned int key_param_len;

    ctx->plat.s390x.fc = s390x_fc_from_md(ctx->md);
    if (ctx->plat.s390x.fc == 0)
        return -1; /* Not supported by kmac instruction */

    if (s390x_check_engine_used(ctx->md, impl)) {
        ctx->plat.s390x.fc = 0;
        return -1; /* An engine handles the digest, disable acceleration */
    }

    ctx->plat.s390x.blk_size = EVP_MD_get_block_size(ctx->md);
    if (ctx->plat.s390x.blk_size < 0)
        return 0;

    if (ctx->plat.s390x.size !=
        (size_t)(ctx->plat.s390x.blk_size * HMAC_S390X_BUF_NUM_BLOCKS)) {
        OPENSSL_clear_free(ctx->plat.s390x.buf, ctx->plat.s390x.size);
        ctx->plat.s390x.size = 0;
        ctx->plat.s390x.buf = OPENSSL_zalloc(ctx->plat.s390x.blk_size *
                                             HMAC_S390X_BUF_NUM_BLOCKS);
        if (ctx->plat.s390x.buf == NULL)
            return 0;
        ctx->plat.s390x.size = ctx->plat.s390x.blk_size *
            HMAC_S390X_BUF_NUM_BLOCKS;
    }
    ctx->plat.s390x.num = 0;

    ctx->plat.s390x.ikp = 0;
    ctx->plat.s390x.iimp = 1;

    switch (ctx->plat.s390x.fc) {
    case S390X_HMAC_SHA_224:
    case S390X_HMAC_SHA_256:
        ctx->plat.s390x.param.hmac_224_256.imbl = 0;
        OPENSSL_cleanse(ctx->plat.s390x.param.hmac_224_256.h,
                        sizeof(ctx->plat.s390x.param.hmac_224_256.h));
        break;
    case S390X_HMAC_SHA_384:
    case S390X_HMAC_SHA_512:
        ctx->plat.s390x.param.hmac_384_512.imbl = 0;
        OPENSSL_cleanse(ctx->plat.s390x.param.hmac_384_512.h,
                        sizeof(ctx->plat.s390x.param.hmac_384_512.h));
        break;
    default:
        return 0;
    }

    if (key != NULL) {
        switch (ctx->plat.s390x.fc) {
        case S390X_HMAC_SHA_224:
        case S390X_HMAC_SHA_256:
            OPENSSL_cleanse(&ctx->plat.s390x.param.hmac_224_256.key,
                            sizeof(ctx->plat.s390x.param.hmac_224_256.key));
            key_param = ctx->plat.s390x.param.hmac_224_256.key;
            key_param_len = sizeof(ctx->plat.s390x.param.hmac_224_256.key);
            break;
        case S390X_HMAC_SHA_384:
        case S390X_HMAC_SHA_512:
            OPENSSL_cleanse(&ctx->plat.s390x.param.hmac_384_512.key,
                            sizeof(ctx->plat.s390x.param.hmac_384_512.key));
            key_param = ctx->plat.s390x.param.hmac_384_512.key;
            key_param_len = sizeof(ctx->plat.s390x.param.hmac_384_512.key);
            break;
        default:
            return 0;
        }

        if (!ossl_assert(ctx->plat.s390x.blk_size <= (int)key_param_len))
            return 0;

        if (key_len > ctx->plat.s390x.blk_size) {
            if (!EVP_DigestInit_ex(ctx->md_ctx, ctx->md, impl)
                    || !EVP_DigestUpdate(ctx->md_ctx, key, key_len)
                    || !EVP_DigestFinal_ex(ctx->md_ctx, key_param,
                                           &key_param_len))
                return 0;
        } else {
            if (key_len < 0 || key_len > (int)key_param_len)
                return 0;
            memcpy(key_param, key, key_len);
            /* remaining key bytes already zeroed out above */
        }
    }

    return 1;
}

int s390x_HMAC_update(HMAC_CTX *ctx, const unsigned char *data, size_t len)
{
    size_t remain, num;

    if (ctx->plat.s390x.iimp != 1) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    if (len == 0)
        return 1;

    /* buffer is full, process it now */
    if (ctx->plat.s390x.num == ctx->plat.s390x.size) {
        s390x_call_kmac(ctx, ctx->plat.s390x.buf, ctx->plat.s390x.num);

        ctx->plat.s390x.num = 0;
    }

    remain = ctx->plat.s390x.size - ctx->plat.s390x.num;
    if (len > remain) {
        /* data does not fit into buffer */
        if (ctx->plat.s390x.num > 0) {
            /* first fill buffer and process it */
            memcpy(&ctx->plat.s390x.buf[ctx->plat.s390x.num], data, remain);
            ctx->plat.s390x.num += remain;

            s390x_call_kmac(ctx, ctx->plat.s390x.buf, ctx->plat.s390x.num);

            ctx->plat.s390x.num = 0;

            data += remain;
            len -= remain;
        }

        if (!ossl_assert(ctx->plat.s390x.num == 0))
            return 0;

        if (len > ctx->plat.s390x.size) {
            /*
             * remaining data is still larger than buffer, process remaining
             * full blocks of input directly
             */
            remain = len % ctx->plat.s390x.blk_size;
            num = len - remain;

            s390x_call_kmac(ctx, data, num);

            data += num;
            len -= num;
        }
    }

    /* add remaining input data (which is < buffer size) to buffer */
    if (!ossl_assert(len <= ctx->plat.s390x.size))
        return 0;

    if (len > 0) {
        memcpy(&ctx->plat.s390x.buf[ctx->plat.s390x.num], data, len);
        ctx->plat.s390x.num += len;
    }

    return 1;
}

int s390x_HMAC_final(HMAC_CTX *ctx, unsigned char *md, unsigned int *len)
{
    void *result;
    unsigned int res_len;

    if (ctx->plat.s390x.iimp != 1) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    ctx->plat.s390x.iimp = 0; /* last block */
    s390x_call_kmac(ctx, ctx->plat.s390x.buf, ctx->plat.s390x.num);

    ctx->plat.s390x.num = 0;

    switch (ctx->plat.s390x.fc) {
    case S390X_HMAC_SHA_224:
        result = &ctx->plat.s390x.param.hmac_224_256.h[0];
        res_len = SHA224_DIGEST_LENGTH;
        break;
    case S390X_HMAC_SHA_256:
        result = &ctx->plat.s390x.param.hmac_224_256.h[0];
        res_len = SHA256_DIGEST_LENGTH;
        break;
    case S390X_HMAC_SHA_384:
        result = &ctx->plat.s390x.param.hmac_384_512.h[0];
        res_len = SHA384_DIGEST_LENGTH;
        break;
    case S390X_HMAC_SHA_512:
        result = &ctx->plat.s390x.param.hmac_384_512.h[0];
        res_len = SHA512_DIGEST_LENGTH;
        break;
    default:
        return 0;
    }

    memcpy(md, result, res_len);
    if (len != NULL)
        *len = res_len;

    return 1;
}

int s390x_HMAC_CTX_copy(HMAC_CTX *dctx, HMAC_CTX *sctx)
{
    dctx->plat.s390x.fc = sctx->plat.s390x.fc;
    dctx->plat.s390x.blk_size = sctx->plat.s390x.blk_size;
    dctx->plat.s390x.ikp = sctx->plat.s390x.ikp;
    dctx->plat.s390x.iimp = sctx->plat.s390x.iimp;

    memcpy(&dctx->plat.s390x.param, &sctx->plat.s390x.param,
           sizeof(dctx->plat.s390x.param));

    OPENSSL_clear_free(dctx->plat.s390x.buf, dctx->plat.s390x.size);
    dctx->plat.s390x.buf = NULL;
    if (sctx->plat.s390x.buf != NULL) {
        dctx->plat.s390x.buf = OPENSSL_memdup(sctx->plat.s390x.buf,
                                              sctx->plat.s390x.size);
        if (dctx->plat.s390x.buf == NULL)
            return 0;
    }

    dctx->plat.s390x.size = sctx->plat.s390x.size;
    dctx->plat.s390x.num = sctx->plat.s390x.num;

    return 1;
}

int s390x_HMAC_CTX_cleanup(HMAC_CTX *ctx)
{
    OPENSSL_clear_free(ctx->plat.s390x.buf, ctx->plat.s390x.size);
    ctx->plat.s390x.buf = NULL;
    ctx->plat.s390x.size = 0;
    ctx->plat.s390x.num = 0;

    OPENSSL_cleanse(&ctx->plat.s390x.param, sizeof(ctx->plat.s390x.param));

    ctx->plat.s390x.blk_size = 0;
    ctx->plat.s390x.ikp = 0;
    ctx->plat.s390x.iimp = 1;

    ctx->plat.s390x.fc = 0;

    return 1;
}

#endif
