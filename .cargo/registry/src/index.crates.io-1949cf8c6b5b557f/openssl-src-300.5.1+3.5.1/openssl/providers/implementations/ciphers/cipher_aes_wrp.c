/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file uses the low level AES functions (which are deprecated for
 * non-internal use) in order to implement provider AES ciphers.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_aes.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"

/* AES wrap with padding has IV length of 4, without padding 8 */
#define AES_WRAP_PAD_IVLEN   4
#define AES_WRAP_NOPAD_IVLEN 8

#define WRAP_FLAGS (PROV_CIPHER_FLAG_CUSTOM_IV)
#define WRAP_FLAGS_INV (WRAP_FLAGS | PROV_CIPHER_FLAG_INVERSE_CIPHER)

typedef size_t (*aeswrap_fn)(void *key, const unsigned char *iv,
                             unsigned char *out, const unsigned char *in,
                             size_t inlen, block128_f block);

static OSSL_FUNC_cipher_encrypt_init_fn aes_wrap_einit;
static OSSL_FUNC_cipher_decrypt_init_fn aes_wrap_dinit;
static OSSL_FUNC_cipher_update_fn aes_wrap_cipher;
static OSSL_FUNC_cipher_final_fn aes_wrap_final;
static OSSL_FUNC_cipher_freectx_fn aes_wrap_freectx;
static OSSL_FUNC_cipher_set_ctx_params_fn aes_wrap_set_ctx_params;

typedef struct prov_aes_wrap_ctx_st {
    PROV_CIPHER_CTX base;
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ks;
    aeswrap_fn wrapfn;

} PROV_AES_WRAP_CTX;


static void *aes_wrap_newctx(size_t kbits, size_t blkbits,
                             size_t ivbits, unsigned int mode, uint64_t flags)
{
    PROV_AES_WRAP_CTX *wctx;
    PROV_CIPHER_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    wctx = OPENSSL_zalloc(sizeof(*wctx));
    ctx = (PROV_CIPHER_CTX *)wctx;
    if (ctx != NULL) {
        ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits, mode, flags,
                                    NULL, NULL);
        ctx->pad = (ctx->ivlen == AES_WRAP_PAD_IVLEN);
    }
    return wctx;
}

static void *aes_wrap_dupctx(void *wctx)
{
    PROV_AES_WRAP_CTX *ctx = wctx;
    PROV_AES_WRAP_CTX *dctx = wctx;

    if (!ossl_prov_is_running())
        return NULL;

    if (ctx == NULL)
        return NULL;
    dctx = OPENSSL_memdup(ctx, sizeof(*ctx));

    if (dctx != NULL && dctx->base.tlsmac != NULL && dctx->base.alloced) {
        dctx->base.tlsmac = OPENSSL_memdup(dctx->base.tlsmac,
                                           dctx->base.tlsmacsize);
        if (dctx->base.tlsmac == NULL) {
            OPENSSL_free(dctx);
            dctx = NULL;
        }
    }
    return dctx;
}

static void aes_wrap_freectx(void *vctx)
{
    PROV_AES_WRAP_CTX *wctx = (PROV_AES_WRAP_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(wctx,  sizeof(*wctx));
}

static int aes_wrap_init(void *vctx, const unsigned char *key,
                         size_t keylen, const unsigned char *iv,
                         size_t ivlen, const OSSL_PARAM params[], int enc)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    PROV_AES_WRAP_CTX *wctx = (PROV_AES_WRAP_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;
    if (ctx->pad)
        wctx->wrapfn = enc ? CRYPTO_128_wrap_pad : CRYPTO_128_unwrap_pad;
    else
        wctx->wrapfn = enc ? CRYPTO_128_wrap : CRYPTO_128_unwrap;

    if (iv != NULL) {
        if (!ossl_cipher_generic_initiv(ctx, iv, ivlen))
            return 0;
    }
    if (key != NULL) {
        int use_forward_transform;

        if (keylen != ctx->keylen) {
           ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
           return 0;
        }
        /*
         * See SP800-38F : Section 5.1
         * The forward and inverse transformations for the AES block
         * cipher—called “cipher” and “inverse  cipher” are informally known as
         * the AES encryption and AES decryption functions, respectively.
         * If the designated cipher function for a key-wrap algorithm is chosen
         * to be the AES decryption function, then CIPH-1K will be the AES
         * encryption function.
         */
        if (ctx->inverse_cipher == 0)
            use_forward_transform = ctx->enc;
        else
            use_forward_transform = !ctx->enc;
        if (use_forward_transform) {
            AES_set_encrypt_key(key, keylen * 8, &wctx->ks.ks);
            ctx->block = (block128_f)AES_encrypt;
        } else {
            AES_set_decrypt_key(key, keylen * 8, &wctx->ks.ks);
            ctx->block = (block128_f)AES_decrypt;
        }
    }
    return aes_wrap_set_ctx_params(ctx, params);
}

static int aes_wrap_einit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    return aes_wrap_init(ctx, key, keylen, iv, ivlen, params, 1);
}

static int aes_wrap_dinit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    return aes_wrap_init(ctx, key, keylen, iv, ivlen, params, 0);
}

static int aes_wrap_cipher_internal(void *vctx, unsigned char *out,
                                    const unsigned char *in, size_t inlen)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    PROV_AES_WRAP_CTX *wctx = (PROV_AES_WRAP_CTX *)vctx;
    size_t rv;
    int pad = ctx->pad;

    /* No final operation so always return zero length */
    if (in == NULL)
        return 0;

    /* Input length must always be non-zero */
    if (inlen == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_INPUT_LENGTH);
        return -1;
    }

    /* If decrypting need at least 16 bytes and multiple of 8 */
    if (!ctx->enc && (inlen < 16 || inlen & 0x7)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_INPUT_LENGTH);
        return -1;
    }

    /* If not padding input must be multiple of 8 */
    if (!pad && inlen & 0x7) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_INPUT_LENGTH);
        return -1;
    }

    if (out == NULL) {
        if (ctx->enc) {
            /* If padding round up to multiple of 8 */
            if (pad)
                inlen = (inlen + 7) / 8 * 8;
            /* 8 byte prefix */
            return inlen + 8;
        } else {
            /*
             * If not padding output will be exactly 8 bytes smaller than
             * input. If padding it will be at least 8 bytes smaller but we
             * don't know how much.
             */
            return inlen - 8;
        }
    }

    rv = wctx->wrapfn(&wctx->ks.ks, ctx->iv_set ? ctx->iv : NULL, out, in,
                      inlen, ctx->block);
    if (!rv) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return -1;
    }
    if (rv > INT_MAX) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH);
        return -1;
    }
    return (int)rv;
}

static int aes_wrap_final(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize)
{
    if (!ossl_prov_is_running())
        return 0;

    *outl = 0;
    return 1;
}

static int aes_wrap_cipher(void *vctx,
                           unsigned char *out, size_t *outl, size_t outsize,
                           const unsigned char *in, size_t inl)
{
    PROV_AES_WRAP_CTX *ctx = (PROV_AES_WRAP_CTX *)vctx;
    size_t len;

    if (!ossl_prov_is_running())
        return 0;

    if (inl == 0) {
        *outl = 0;
        return 1;
    }

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    len = aes_wrap_cipher_internal(ctx, out, in, inl);
    if (len <= 0)
        return 0;

    *outl = len;
    return 1;
}

static int aes_wrap_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t keylen = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->keylen != keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}

#define IMPLEMENT_cipher(mode, fname, UCMODE, flags, kbits, blkbits, ivbits)   \
    static OSSL_FUNC_cipher_get_params_fn aes_##kbits##_##fname##_get_params;  \
    static int aes_##kbits##_##fname##_get_params(OSSL_PARAM params[])         \
    {                                                                          \
        return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,\
                                              flags, kbits, blkbits, ivbits);  \
    }                                                                          \
    static OSSL_FUNC_cipher_newctx_fn aes_##kbits##fname##_newctx;             \
    static void *aes_##kbits##fname##_newctx(void *provctx)                    \
    {                                                                          \
        return aes_##mode##_newctx(kbits, blkbits, ivbits,                     \
                                   EVP_CIPH_##UCMODE##_MODE, flags);           \
    }                                                                          \
    const OSSL_DISPATCH ossl_##aes##kbits##fname##_functions[] = {             \
        { OSSL_FUNC_CIPHER_NEWCTX,                                             \
            (void (*)(void))aes_##kbits##fname##_newctx },                     \
        { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))aes_##mode##_einit }, \
        { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))aes_##mode##_dinit }, \
        { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))aes_##mode##_cipher },      \
        { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))aes_##mode##_final },        \
        { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))aes_##mode##_freectx },    \
        { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))aes_##mode##_dupctx },      \
        { OSSL_FUNC_CIPHER_GET_PARAMS,                                         \
            (void (*)(void))aes_##kbits##_##fname##_get_params },              \
        { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                    \
            (void (*)(void))ossl_cipher_generic_gettable_params },             \
        { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                     \
            (void (*)(void))ossl_cipher_generic_get_ctx_params },              \
        { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                     \
            (void (*)(void))aes_wrap_set_ctx_params },                         \
        { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                \
            (void (*)(void))ossl_cipher_generic_gettable_ctx_params },         \
        { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                \
            (void (*)(void))ossl_cipher_generic_settable_ctx_params },         \
        OSSL_DISPATCH_END                                                      \
    }

IMPLEMENT_cipher(wrap, wrap, WRAP, WRAP_FLAGS, 256, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrap, WRAP, WRAP_FLAGS, 192, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrap, WRAP, WRAP_FLAGS, 128, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappad, WRAP, WRAP_FLAGS, 256, 64, AES_WRAP_PAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappad, WRAP, WRAP_FLAGS, 192, 64, AES_WRAP_PAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappad, WRAP, WRAP_FLAGS, 128, 64, AES_WRAP_PAD_IVLEN * 8);

IMPLEMENT_cipher(wrap, wrapinv, WRAP, WRAP_FLAGS_INV, 256, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrapinv, WRAP, WRAP_FLAGS_INV, 192, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrapinv, WRAP, WRAP_FLAGS_INV, 128, 64, AES_WRAP_NOPAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappadinv, WRAP, WRAP_FLAGS_INV, 256, 64, AES_WRAP_PAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappadinv, WRAP, WRAP_FLAGS_INV, 192, 64, AES_WRAP_PAD_IVLEN * 8);
IMPLEMENT_cipher(wrap, wrappadinv, WRAP, WRAP_FLAGS_INV, 128, 64, AES_WRAP_PAD_IVLEN * 8);
