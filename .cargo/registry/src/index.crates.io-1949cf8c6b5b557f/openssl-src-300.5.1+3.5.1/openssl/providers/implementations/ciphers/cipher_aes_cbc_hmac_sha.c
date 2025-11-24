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

/* Dispatch functions for AES_CBC_HMAC_SHA ciphers */

/* For SSL3_VERSION and TLS1_VERSION */
#include <openssl/prov_ssl.h>
#include <openssl/proverr.h>
#include "cipher_aes_cbc_hmac_sha.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#ifndef AES_CBC_HMAC_SHA_CAPABLE
# define IMPLEMENT_CIPHER(nm, sub, kbits, blkbits, ivbits, flags)              \
const OSSL_DISPATCH ossl_##nm##kbits##sub##_functions[] = {                    \
    OSSL_DISPATCH_END                                                              \
};
#else

# define AES_CBC_HMAC_SHA_FLAGS (PROV_CIPHER_FLAG_AEAD                         \
                                 | PROV_CIPHER_FLAG_TLS1_MULTIBLOCK)

static OSSL_FUNC_cipher_encrypt_init_fn aes_einit;
static OSSL_FUNC_cipher_decrypt_init_fn aes_dinit;
static OSSL_FUNC_cipher_freectx_fn aes_cbc_hmac_sha1_freectx;
static OSSL_FUNC_cipher_freectx_fn aes_cbc_hmac_sha256_freectx;
static OSSL_FUNC_cipher_get_ctx_params_fn aes_get_ctx_params;
static OSSL_FUNC_cipher_gettable_ctx_params_fn aes_gettable_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn aes_set_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn aes_settable_ctx_params;
# define aes_gettable_params ossl_cipher_generic_gettable_params
# define aes_update ossl_cipher_generic_stream_update
# define aes_final ossl_cipher_generic_stream_final
# define aes_cipher ossl_cipher_generic_cipher

static int aes_einit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return aes_set_ctx_params(ctx, params);
}

static int aes_dinit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return aes_set_ctx_params(ctx, params);
}

static const OSSL_PARAM cipher_aes_known_settable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_MAC_KEY, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD, NULL, 0),
# if !defined(OPENSSL_NO_MULTIBLOCK)
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_SEND_FRAGMENT, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_IN, NULL, 0),
# endif /* !defined(OPENSSL_NO_MULTIBLOCK) */
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *aes_settable_ctx_params(ossl_unused void *cctx,
                                          ossl_unused void *provctx)
{
    return cipher_aes_known_settable_ctx_params;
}

static int aes_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_CIPHER_HW_AES_HMAC_SHA *hw =
       (PROV_CIPHER_HW_AES_HMAC_SHA *)ctx->hw;
    const OSSL_PARAM *p;
    int ret = 1;
# if !defined(OPENSSL_NO_MULTIBLOCK)
    EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM mb_param;
# endif

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_MAC_KEY);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        hw->init_mac_key(ctx, p->data, p->data_size);
    }

# if !defined(OPENSSL_NO_MULTIBLOCK)
    p = OSSL_PARAM_locate_const(params,
            OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_SEND_FRAGMENT);
    if (p != NULL
            && !OSSL_PARAM_get_size_t(p, &ctx->multiblock_max_send_fragment)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }
    /*
     * The inputs to tls1_multiblock_aad are:
     *   mb_param->inp
     *   mb_param->len
     *   mb_param->interleave
     * The outputs of tls1_multiblock_aad are written to:
     *   ctx->multiblock_interleave
     *   ctx->multiblock_aad_packlen
     */
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD);
    if (p != NULL) {
        const OSSL_PARAM *p1 = OSSL_PARAM_locate_const(params,
                                   OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE);
        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || p1 == NULL
            || !OSSL_PARAM_get_uint(p1, &mb_param.interleave)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        mb_param.inp = p->data;
        mb_param.len = p->data_size;
        if (hw->tls1_multiblock_aad(vctx, &mb_param) <= 0)
            return 0;
    }

    /*
     * The inputs to tls1_multiblock_encrypt are:
     *   mb_param->inp
     *   mb_param->len
     *   mb_param->interleave
     *   mb_param->out
     * The outputs of tls1_multiblock_encrypt are:
     *   ctx->multiblock_encrypt_len
     */
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC);
    if (p != NULL) {
        const OSSL_PARAM *p1 = OSSL_PARAM_locate_const(params,
                                   OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE);
        const OSSL_PARAM *pin = OSSL_PARAM_locate_const(params,
                                    OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_IN);

        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || pin == NULL
            || pin->data_type != OSSL_PARAM_OCTET_STRING
            || p1 == NULL
            || !OSSL_PARAM_get_uint(p1, &mb_param.interleave)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        mb_param.out = p->data;
        mb_param.inp = pin->data;
        mb_param.len = pin->data_size;
        if (hw->tls1_multiblock_encrypt(vctx, &mb_param) <= 0)
            return 0;
    }
# endif /* !defined(OPENSSL_NO_MULTIBLOCK) */

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (hw->set_tls1_aad(ctx, p->data, p->data_size) <= 0)
            return 0;
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

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS_VERSION);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &ctx->base.tlsversion)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->base.tlsversion == SSL3_VERSION
                || ctx->base.tlsversion == TLS1_VERSION) {
            if (!ossl_assert(ctx->base.removetlsfixed >= AES_BLOCK_SIZE)) {
                ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
                return 0;
            }
            /*
             * There is no explicit IV with these TLS versions, so don't attempt
             * to remove it.
             */
            ctx->base.removetlsfixed -= AES_BLOCK_SIZE;
        }
    }
    return ret;
}

static int aes_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    OSSL_PARAM *p;

# if !defined(OPENSSL_NO_MULTIBLOCK)
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_BUFSIZE);
    if (p != NULL) {
        PROV_CIPHER_HW_AES_HMAC_SHA *hw =
           (PROV_CIPHER_HW_AES_HMAC_SHA *)ctx->hw;
        size_t len = hw->tls1_multiblock_max_bufsize(ctx);

        if (!OSSL_PARAM_set_size_t(p, len)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->multiblock_interleave)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD_PACKLEN);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->multiblock_aad_packlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_LEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->multiblock_encrypt_len)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
# endif /* !defined(OPENSSL_NO_MULTIBLOCK) */

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->tls_aad_pad)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->base.ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IV);
    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p, ctx->base.oiv, ctx->base.ivlen)
        && !OSSL_PARAM_set_octet_ptr(p, &ctx->base.oiv, ctx->base.ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_UPDATED_IV);
    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p, ctx->base.iv, ctx->base.ivlen)
        && !OSSL_PARAM_set_octet_ptr(p, &ctx->base.iv, ctx->base.ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM cipher_aes_known_gettable_ctx_params[] = {
# if !defined(OPENSSL_NO_MULTIBLOCK)
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_BUFSIZE, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE, NULL),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD_PACKLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_LEN, NULL),
# endif /* !defined(OPENSSL_NO_MULTIBLOCK) */
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_IV, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_UPDATED_IV, NULL, 0),
    OSSL_PARAM_END
};
const OSSL_PARAM *aes_gettable_ctx_params(ossl_unused void *cctx,
                                          ossl_unused void *provctx)
{
    return cipher_aes_known_gettable_ctx_params;
}

static void base_init(void *provctx, PROV_AES_HMAC_SHA_CTX *ctx,
                      const PROV_CIPHER_HW_AES_HMAC_SHA *meths,
                      size_t kbits, size_t blkbits, size_t ivbits,
                      uint64_t flags)
{
    ossl_cipher_generic_initkey(&ctx->base, kbits, blkbits, ivbits,
                                EVP_CIPH_CBC_MODE, flags,
                                &meths->base, provctx);
    ctx->hw = (PROV_CIPHER_HW_AES_HMAC_SHA *)ctx->base.hw;
}

static void *aes_cbc_hmac_sha1_newctx(void *provctx, size_t kbits,
                                      size_t blkbits, size_t ivbits,
                                      uint64_t flags)
{
    PROV_AES_HMAC_SHA1_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        base_init(provctx, &ctx->base_ctx,
                  ossl_prov_cipher_hw_aes_cbc_hmac_sha1(), kbits, blkbits,
                  ivbits, flags);
    return ctx;
}

static void *aes_cbc_hmac_sha1_dupctx(void *provctx)
{
    PROV_AES_HMAC_SHA1_CTX *ctx = provctx;

    if (!ossl_prov_is_running())
        return NULL;

    if (ctx == NULL)
        return NULL;

    return OPENSSL_memdup(ctx, sizeof(*ctx));
}

static void aes_cbc_hmac_sha1_freectx(void *vctx)
{
    PROV_AES_HMAC_SHA1_CTX *ctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;

    if (ctx != NULL) {
        ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
        OPENSSL_clear_free(ctx, sizeof(*ctx));
    }
}

static void *aes_cbc_hmac_sha256_newctx(void *provctx, size_t kbits,
                                        size_t blkbits, size_t ivbits,
                                        uint64_t flags)
{
    PROV_AES_HMAC_SHA256_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx != NULL)
        base_init(provctx, &ctx->base_ctx,
                  ossl_prov_cipher_hw_aes_cbc_hmac_sha256(), kbits, blkbits,
                  ivbits, flags);
    return ctx;
}

static void *aes_cbc_hmac_sha256_dupctx(void *provctx)
{
    PROV_AES_HMAC_SHA256_CTX *ctx = provctx;

    if (!ossl_prov_is_running())
        return NULL;

    return OPENSSL_memdup(ctx, sizeof(*ctx));
}

static void aes_cbc_hmac_sha256_freectx(void *vctx)
{
    PROV_AES_HMAC_SHA256_CTX *ctx = (PROV_AES_HMAC_SHA256_CTX *)vctx;

    if (ctx != NULL) {
        ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
        OPENSSL_clear_free(ctx, sizeof(*ctx));
    }
}

# define IMPLEMENT_CIPHER(nm, sub, kbits, blkbits, ivbits, flags)              \
static OSSL_FUNC_cipher_newctx_fn nm##_##kbits##_##sub##_newctx;               \
static void *nm##_##kbits##_##sub##_newctx(void *provctx)                      \
{                                                                              \
    return nm##_##sub##_newctx(provctx, kbits, blkbits, ivbits, flags);        \
}                                                                              \
static OSSL_FUNC_cipher_get_params_fn nm##_##kbits##_##sub##_get_params;       \
static int nm##_##kbits##_##sub##_get_params(OSSL_PARAM params[])              \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_CBC_MODE,           \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
const OSSL_DISPATCH ossl_##nm##kbits##sub##_functions[] = {                    \
    { OSSL_FUNC_CIPHER_NEWCTX, (void (*)(void))nm##_##kbits##_##sub##_newctx },\
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))nm##_##sub##_freectx },        \
    { OSSL_FUNC_CIPHER_DUPCTX,  (void (*)(void))nm##_##sub##_dupctx},          \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))nm##_einit },             \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))nm##_dinit },             \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))nm##_update },                  \
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))nm##_final },                    \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))nm##_cipher },                  \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
        (void (*)(void))nm##_##kbits##_##sub##_get_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
        (void (*)(void))nm##_gettable_params },                                \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
         (void (*)(void))nm##_get_ctx_params },                                \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
        (void (*)(void))nm##_gettable_ctx_params },                            \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
        (void (*)(void))nm##_set_ctx_params },                                 \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
        (void (*)(void))nm##_settable_ctx_params },                            \
    OSSL_DISPATCH_END                                                          \
};

#endif /* AES_CBC_HMAC_SHA_CAPABLE */

/* ossl_aes128cbc_hmac_sha1_functions */
IMPLEMENT_CIPHER(aes, cbc_hmac_sha1, 128, 128, 128, AES_CBC_HMAC_SHA_FLAGS)
/* ossl_aes256cbc_hmac_sha1_functions */
IMPLEMENT_CIPHER(aes, cbc_hmac_sha1, 256, 128, 128, AES_CBC_HMAC_SHA_FLAGS)
/* ossl_aes128cbc_hmac_sha256_functions */
IMPLEMENT_CIPHER(aes, cbc_hmac_sha256, 128, 128, 128, AES_CBC_HMAC_SHA_FLAGS)
/* ossl_aes256cbc_hmac_sha256_functions */
IMPLEMENT_CIPHER(aes, cbc_hmac_sha256, 256, 128, 128, AES_CBC_HMAC_SHA_FLAGS)
