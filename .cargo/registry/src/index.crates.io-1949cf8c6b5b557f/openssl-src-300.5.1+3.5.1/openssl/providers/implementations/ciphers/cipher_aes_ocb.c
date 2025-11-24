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

#include <openssl/proverr.h>
#include "cipher_aes_ocb.h"
#include "prov/providercommon.h"
#include "prov/ciphercommon_aead.h"
#include "prov/implementations.h"

#define AES_OCB_FLAGS AEAD_FLAGS

#define OCB_DEFAULT_TAG_LEN 16
#define OCB_DEFAULT_IV_LEN  12
#define OCB_MIN_IV_LEN      1
#define OCB_MAX_IV_LEN      15

PROV_CIPHER_FUNC(int, ocb_cipher, (PROV_AES_OCB_CTX *ctx,
                                   const unsigned char *in, unsigned char *out,
                                   size_t nextblock));
/* forward declarations */
static OSSL_FUNC_cipher_encrypt_init_fn aes_ocb_einit;
static OSSL_FUNC_cipher_decrypt_init_fn aes_ocb_dinit;
static OSSL_FUNC_cipher_update_fn aes_ocb_block_update;
static OSSL_FUNC_cipher_final_fn aes_ocb_block_final;
static OSSL_FUNC_cipher_cipher_fn aes_ocb_cipher;
static OSSL_FUNC_cipher_freectx_fn aes_ocb_freectx;
static OSSL_FUNC_cipher_dupctx_fn aes_ocb_dupctx;
static OSSL_FUNC_cipher_get_ctx_params_fn aes_ocb_get_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn aes_ocb_set_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn cipher_ocb_gettable_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn cipher_ocb_settable_ctx_params;

/*
 * The following methods could be moved into PROV_AES_OCB_HW if
 * multiple hardware implementations are ever needed.
 */
static ossl_inline int aes_generic_ocb_setiv(PROV_AES_OCB_CTX *ctx,
                                             const unsigned char *iv,
                                             size_t ivlen, size_t taglen)
{
    return (CRYPTO_ocb128_setiv(&ctx->ocb, iv, ivlen, taglen) == 1);
}

static ossl_inline int aes_generic_ocb_setaad(PROV_AES_OCB_CTX *ctx,
                                              const unsigned char *aad,
                                              size_t alen)
{
    return CRYPTO_ocb128_aad(&ctx->ocb, aad, alen) == 1;
}

static ossl_inline int aes_generic_ocb_gettag(PROV_AES_OCB_CTX *ctx,
                                              unsigned char *tag, size_t tlen)
{
    return CRYPTO_ocb128_tag(&ctx->ocb, tag, tlen) > 0;
}

static ossl_inline int aes_generic_ocb_final(PROV_AES_OCB_CTX *ctx)
{
    return (CRYPTO_ocb128_finish(&ctx->ocb, ctx->tag, ctx->taglen) == 0);
}

static ossl_inline void aes_generic_ocb_cleanup(PROV_AES_OCB_CTX *ctx)
{
    CRYPTO_ocb128_cleanup(&ctx->ocb);
}

static ossl_inline int aes_generic_ocb_cipher(PROV_AES_OCB_CTX *ctx,
                                              const unsigned char *in,
                                              unsigned char *out, size_t len)
{
    if (ctx->base.enc) {
        if (!CRYPTO_ocb128_encrypt(&ctx->ocb, in, out, len))
            return 0;
    } else {
        if (!CRYPTO_ocb128_decrypt(&ctx->ocb, in, out, len))
            return 0;
    }
    return 1;
}

static ossl_inline int aes_generic_ocb_copy_ctx(PROV_AES_OCB_CTX *dst,
                                                PROV_AES_OCB_CTX *src)
{
    return CRYPTO_ocb128_copy_ctx(&dst->ocb, &src->ocb,
                                  &dst->ksenc.ks, &dst->ksdec.ks);
}

/*-
 * Provider dispatch functions
 */
static int aes_ocb_init(void *vctx, const unsigned char *key, size_t keylen,
                        const unsigned char *iv, size_t ivlen,
                        const OSSL_PARAM params[], int enc)
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->aad_buf_len = 0;
    ctx->data_buf_len = 0;
    ctx->base.enc = enc;

    if (iv != NULL) {
        if (ivlen != ctx->base.ivlen) {
            /* IV len must be 1 to 15 */
            if (ivlen < OCB_MIN_IV_LEN || ivlen > OCB_MAX_IV_LEN) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
                return 0;
            }
            ctx->base.ivlen = ivlen;
        }
        if (!ossl_cipher_generic_initiv(&ctx->base, iv, ivlen))
            return 0;
        ctx->iv_state = IV_STATE_BUFFERED;
    }
    if (key != NULL) {
        if (keylen != ctx->base.keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->base.hw->init(&ctx->base, key, keylen))
            return 0;
    }
    return aes_ocb_set_ctx_params(ctx, params);
}

static int aes_ocb_einit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
    return aes_ocb_init(vctx, key, keylen, iv, ivlen, params, 1);
}

static int aes_ocb_dinit(void *vctx, const unsigned char *key, size_t keylen,
                         const unsigned char *iv, size_t ivlen,
                         const OSSL_PARAM params[])
{
    return aes_ocb_init(vctx, key, keylen, iv, ivlen, params, 0);
}

/*
 * Because of the way OCB works, both the AAD and data are buffered in the
 * same way. Only the last block can be a partial block.
 */
static int aes_ocb_block_update_internal(PROV_AES_OCB_CTX *ctx,
                                         unsigned char *buf, size_t *bufsz,
                                         unsigned char *out, size_t *outl,
                                         size_t outsize, const unsigned char *in,
                                         size_t inl, OSSL_ocb_cipher_fn ciph)
{
    size_t nextblocks;
    size_t outlint = 0;

    if (*bufsz != 0)
        nextblocks = ossl_cipher_fillblock(buf, bufsz, AES_BLOCK_SIZE, &in, &inl);
    else
        nextblocks = inl & ~(AES_BLOCK_SIZE-1);

    if (*bufsz == AES_BLOCK_SIZE) {
        if (outsize < AES_BLOCK_SIZE) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        if (!ciph(ctx, buf, out, AES_BLOCK_SIZE)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        *bufsz = 0;
        outlint = AES_BLOCK_SIZE;
        if (out != NULL)
            out += AES_BLOCK_SIZE;
    }
    if (nextblocks > 0) {
        outlint += nextblocks;
        if (outsize < outlint) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        if (!ciph(ctx, in, out, nextblocks)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        in += nextblocks;
        inl -= nextblocks;
    }
    if (inl != 0
        && !ossl_cipher_trailingdata(buf, bufsz, AES_BLOCK_SIZE, &in, &inl)) {
        /* PROVerr already called */
        return 0;
    }

    *outl = outlint;
    return inl == 0;
}

/* A wrapper function that has the same signature as cipher */
static int cipher_updateaad(PROV_AES_OCB_CTX *ctx, const unsigned char *in,
                            unsigned char *out, size_t len)
{
    return aes_generic_ocb_setaad(ctx, in, len);
}

static int update_iv(PROV_AES_OCB_CTX *ctx)
{
    if (ctx->iv_state == IV_STATE_FINISHED
        || ctx->iv_state == IV_STATE_UNINITIALISED)
        return 0;
    if (ctx->iv_state == IV_STATE_BUFFERED) {
        if (!aes_generic_ocb_setiv(ctx, ctx->base.iv, ctx->base.ivlen,
                                   ctx->taglen))
            return 0;
        ctx->iv_state = IV_STATE_COPIED;
    }
    return 1;
}

static int aes_ocb_block_update(void *vctx, unsigned char *out, size_t *outl,
                                size_t outsize, const unsigned char *in,
                                size_t inl)
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;
    unsigned char *buf;
    size_t *buflen;
    OSSL_ocb_cipher_fn fn;

    if (!ctx->key_set || !update_iv(ctx))
        return 0;

    if (inl == 0) {
        *outl = 0;
        return 1;
    }

    /* Are we dealing with AAD or normal data here? */
    if (out == NULL) {
        buf = ctx->aad_buf;
        buflen = &ctx->aad_buf_len;
        fn = cipher_updateaad;
    } else {
        buf = ctx->data_buf;
        buflen = &ctx->data_buf_len;
        fn = aes_generic_ocb_cipher;
    }
    return aes_ocb_block_update_internal(ctx, buf, buflen, out, outl, outsize,
                                         in, inl, fn);
}

static int aes_ocb_block_final(void *vctx, unsigned char *out, size_t *outl,
                               size_t outsize)
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    /* If no block_update has run then the iv still needs to be set */
    if (!ctx->key_set || !update_iv(ctx))
        return 0;

    /*
     * Empty the buffer of any partial block that we might have been provided,
     * both for data and AAD
     */
    *outl = 0;
    if (ctx->data_buf_len > 0) {
        if (!aes_generic_ocb_cipher(ctx, ctx->data_buf, out, ctx->data_buf_len))
            return 0;
        *outl = ctx->data_buf_len;
        ctx->data_buf_len = 0;
    }
    if (ctx->aad_buf_len > 0) {
        if (!aes_generic_ocb_setaad(ctx, ctx->aad_buf, ctx->aad_buf_len))
            return 0;
        ctx->aad_buf_len = 0;
    }
    if (ctx->base.enc) {
        /* If encrypting then just get the tag */
        if (!aes_generic_ocb_gettag(ctx, ctx->tag, ctx->taglen))
            return 0;
    } else {
        /* If decrypting then verify */
        if (ctx->taglen == 0)
            return 0;
        if (!aes_generic_ocb_final(ctx))
            return 0;
    }
    /* Don't reuse the IV */
    ctx->iv_state = IV_STATE_FINISHED;
    return 1;
}

static void *aes_ocb_newctx(void *provctx, size_t kbits, size_t blkbits,
                            size_t ivbits, unsigned int mode, uint64_t flags)
{
    PROV_AES_OCB_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL) {
        ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits, mode, flags,
                                    ossl_prov_cipher_hw_aes_ocb(kbits), NULL);
        ctx->taglen = OCB_DEFAULT_TAG_LEN;
    }
    return ctx;
}

static void aes_ocb_freectx(void *vctx)
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;

    if (ctx != NULL) {
        aes_generic_ocb_cleanup(ctx);
        ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
        OPENSSL_clear_free(ctx,  sizeof(*ctx));
    }
}

static void *aes_ocb_dupctx(void *vctx)
{
    PROV_AES_OCB_CTX *in = (PROV_AES_OCB_CTX *)vctx;
    PROV_AES_OCB_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;
    if (!aes_generic_ocb_copy_ctx(ret, in)) {
        OPENSSL_free(ret);
        ret = NULL;
    }
    return ret;
}

static int aes_ocb_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t sz;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (p->data == NULL) {
            /* Tag len must be 0 to 16 */
            if (p->data_size > OCB_MAX_TAG_LEN) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
                return 0;
            }
            ctx->taglen = p->data_size;
        } else {
            if (ctx->base.enc) {
                ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
                return 0;
            }
            if (p->data_size != ctx->taglen) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
                return 0;
            }
            memcpy(ctx->tag, p->data, p->data_size);
        }
     }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_IVLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &sz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        /* IV len must be 1 to 15 */
        if (sz < OCB_MIN_IV_LEN || sz > OCB_MAX_IV_LEN)
            return 0;
        if (ctx->base.ivlen != sz) {
            ctx->base.ivlen = sz;
            ctx->iv_state = IV_STATE_UNINITIALISED;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        size_t keylen;

        if (!OSSL_PARAM_get_size_t(p, &keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->base.keylen != keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}

static int aes_ocb_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAGLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_set_size_t(p, ctx->taglen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IV);
    if (p != NULL) {
        if (ctx->base.ivlen > p->data_size) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        if (!OSSL_PARAM_set_octet_string(p, ctx->base.oiv, ctx->base.ivlen)
            && !OSSL_PARAM_set_octet_ptr(p, &ctx->base.oiv, ctx->base.ivlen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_UPDATED_IV);
    if (p != NULL) {
        if (ctx->base.ivlen > p->data_size) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        if (!OSSL_PARAM_set_octet_string(p, ctx->base.iv, ctx->base.ivlen)
            && !OSSL_PARAM_set_octet_ptr(p, &ctx->base.iv, ctx->base.ivlen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (!ctx->base.enc || p->data_size != ctx->taglen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
            return 0;
        }
        memcpy(p->data, ctx->tag, ctx->taglen);
    }
    return 1;
}

static const OSSL_PARAM cipher_ocb_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_IV, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_UPDATED_IV, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *cipher_ocb_gettable_ctx_params(ossl_unused void *cctx,
                                                        ossl_unused void *p_ctx)
{
    return cipher_ocb_known_gettable_ctx_params;
}

static const OSSL_PARAM cipher_ocb_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_IVLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *cipher_ocb_settable_ctx_params(ossl_unused void *cctx,
                                                        ossl_unused void *p_ctx)
{
    return cipher_ocb_known_settable_ctx_params;
}

static int aes_ocb_cipher(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_AES_OCB_CTX *ctx = (PROV_AES_OCB_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!aes_generic_ocb_cipher(ctx, in, out, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    *outl = inl;
    return 1;
}

#define IMPLEMENT_cipher(mode, UCMODE, flags, kbits, blkbits, ivbits)          \
static OSSL_FUNC_cipher_get_params_fn aes_##kbits##_##mode##_get_params;       \
static int aes_##kbits##_##mode##_get_params(OSSL_PARAM params[])              \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn aes_##kbits##_##mode##_newctx;               \
static void *aes_##kbits##_##mode##_newctx(void *provctx)                      \
{                                                                              \
    return aes_##mode##_newctx(provctx, kbits, blkbits, ivbits,                \
                               EVP_CIPH_##UCMODE##_MODE, flags);               \
}                                                                              \
const OSSL_DISPATCH ossl_##aes##kbits##mode##_functions[] = {                  \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
        (void (*)(void))aes_##kbits##_##mode##_newctx },                       \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))aes_##mode##_einit },     \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))aes_##mode##_dinit },     \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))aes_##mode##_block_update },    \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))aes_##mode##_block_final },      \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))aes_ocb_cipher },               \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))aes_##mode##_freectx },        \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))aes_##mode##_dupctx },          \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
        (void (*)(void))aes_##kbits##_##mode##_get_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
        (void (*)(void))aes_##mode##_get_ctx_params },                         \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
        (void (*)(void))aes_##mode##_set_ctx_params },                         \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
        (void (*)(void))ossl_cipher_generic_gettable_params },                 \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
        (void (*)(void))cipher_ocb_gettable_ctx_params },                      \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
        (void (*)(void))cipher_ocb_settable_ctx_params },                      \
    OSSL_DISPATCH_END                                                          \
}

IMPLEMENT_cipher(ocb, OCB, AES_OCB_FLAGS, 256, 128, OCB_DEFAULT_IV_LEN * 8);
IMPLEMENT_cipher(ocb, OCB, AES_OCB_FLAGS, 192, 128, OCB_DEFAULT_IV_LEN * 8);
IMPLEMENT_cipher(ocb, OCB, AES_OCB_FLAGS, 128, 128, OCB_DEFAULT_IV_LEN * 8);
