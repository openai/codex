/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <limits.h>
#include <assert.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include <openssl/params.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "internal/core.h"
#include "internal/safe_math.h"
#include "crypto/evp.h"
#include "evp_local.h"

OSSL_SAFE_MATH_SIGNED(int, int)

int EVP_CIPHER_CTX_reset(EVP_CIPHER_CTX *ctx)
{
    if (ctx == NULL)
        return 1;

    if (ctx->cipher == NULL || ctx->cipher->prov == NULL)
        goto legacy;

    if (ctx->algctx != NULL) {
        if (ctx->cipher->freectx != NULL)
            ctx->cipher->freectx(ctx->algctx);
        ctx->algctx = NULL;
    }
    if (ctx->fetched_cipher != NULL)
        EVP_CIPHER_free(ctx->fetched_cipher);
    memset(ctx, 0, sizeof(*ctx));
    ctx->iv_len = -1;

    return 1;

    /* Remove legacy code below when legacy support is removed. */
 legacy:

    if (ctx->cipher != NULL) {
        if (ctx->cipher->cleanup && !ctx->cipher->cleanup(ctx))
            return 0;
        /* Cleanse cipher context data */
        if (ctx->cipher_data && ctx->cipher->ctx_size)
            OPENSSL_cleanse(ctx->cipher_data, ctx->cipher->ctx_size);
    }
    OPENSSL_free(ctx->cipher_data);
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    ENGINE_finish(ctx->engine);
#endif
    memset(ctx, 0, sizeof(*ctx));
    ctx->iv_len = -1;
    return 1;
}

EVP_CIPHER_CTX *EVP_CIPHER_CTX_new(void)
{
    EVP_CIPHER_CTX *ctx;

    ctx = OPENSSL_zalloc(sizeof(EVP_CIPHER_CTX));
    if (ctx == NULL)
        return NULL;

    ctx->iv_len = -1;
    return ctx;
}

void EVP_CIPHER_CTX_free(EVP_CIPHER_CTX *ctx)
{
    if (ctx == NULL)
        return;
    EVP_CIPHER_CTX_reset(ctx);
    OPENSSL_free(ctx);
}

static int evp_cipher_init_internal(EVP_CIPHER_CTX *ctx,
                                    const EVP_CIPHER *cipher,
                                    ENGINE *impl, const unsigned char *key,
                                    const unsigned char *iv, int enc,
                                    uint8_t is_pipeline,
                                    const OSSL_PARAM params[])
{
    int n;
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    ENGINE *tmpimpl = NULL;
#endif

    /*
     * enc == 1 means we are encrypting.
     * enc == 0 means we are decrypting.
     * enc == -1 means, use the previously initialised value for encrypt/decrypt
     */
    if (enc == -1) {
        enc = ctx->encrypt;
    } else {
        if (enc)
            enc = 1;
        ctx->encrypt = enc;
    }

    if (cipher == NULL && ctx->cipher == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    /* Code below to be removed when legacy support is dropped. */
    if (is_pipeline)
        goto nonlegacy;

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    /*
     * Whether it's nice or not, "Inits" can be used on "Final"'d contexts so
     * this context may already have an ENGINE! Try to avoid releasing the
     * previous handle, re-querying for an ENGINE, and having a
     * reinitialisation, when it may all be unnecessary.
     */
    if (ctx->engine && ctx->cipher
        && (cipher == NULL || cipher->nid == ctx->cipher->nid))
        goto skip_to_init;

    if (cipher != NULL && impl == NULL) {
         /* Ask if an ENGINE is reserved for this job */
        tmpimpl = ENGINE_get_cipher_engine(cipher->nid);
    }
#endif

    /*
     * If there are engines involved then we should use legacy handling for now.
     */
    if (ctx->engine != NULL
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
            || tmpimpl != NULL
#endif
            || impl != NULL
            || (cipher != NULL && cipher->origin == EVP_ORIG_METH)
            || (cipher == NULL && ctx->cipher != NULL
                               && ctx->cipher->origin == EVP_ORIG_METH)) {
        if (ctx->cipher == ctx->fetched_cipher)
            ctx->cipher = NULL;
        EVP_CIPHER_free(ctx->fetched_cipher);
        ctx->fetched_cipher = NULL;
        goto legacy;
    }
    /*
     * Ensure a context left lying around from last time is cleared
     * (legacy code)
     */
    if (cipher != NULL && ctx->cipher != NULL) {
        if (ctx->cipher->cleanup != NULL && !ctx->cipher->cleanup(ctx))
            return 0;
        OPENSSL_clear_free(ctx->cipher_data, ctx->cipher->ctx_size);
        ctx->cipher_data = NULL;
    }

    /* Start of non-legacy code below */
nonlegacy:
    /* Ensure a context left lying around from last time is cleared */
    if (cipher != NULL && ctx->cipher != NULL) {
        unsigned long flags = ctx->flags;

        EVP_CIPHER_CTX_reset(ctx);
        /* Restore encrypt and flags */
        ctx->encrypt = enc;
        ctx->flags = flags;
    }

    if (cipher == NULL)
        cipher = ctx->cipher;

    if (cipher->prov == NULL) {
#ifdef FIPS_MODULE
        /* We only do explicit fetches inside the FIPS module */
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
#else
        EVP_CIPHER *provciph =
            EVP_CIPHER_fetch(NULL,
                             cipher->nid == NID_undef ? "NULL"
                                                      : OBJ_nid2sn(cipher->nid),
                             "");

        if (provciph == NULL)
            return 0;
        cipher = provciph;
        EVP_CIPHER_free(ctx->fetched_cipher);
        ctx->fetched_cipher = provciph;
#endif
    }

    if (!ossl_assert(cipher->prov != NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    if (cipher != ctx->fetched_cipher) {
        if (!EVP_CIPHER_up_ref((EVP_CIPHER *)cipher)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
        EVP_CIPHER_free(ctx->fetched_cipher);
        /* Coverity false positive, the reference counting is confusing it */
        /* coverity[use_after_free] */
        ctx->fetched_cipher = (EVP_CIPHER *)cipher;
    }
    ctx->cipher = cipher;

    if (is_pipeline && !EVP_CIPHER_can_pipeline(cipher, enc)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_PIPELINE_NOT_SUPPORTED);
        return 0;
    }

    if (ctx->algctx == NULL) {
        ctx->algctx = ctx->cipher->newctx(ossl_provider_ctx(cipher->prov));
        if (ctx->algctx == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
    }

    if ((ctx->flags & EVP_CIPH_NO_PADDING) != 0) {
        /*
         * If this ctx was already set up for no padding then we need to tell
         * the new cipher about it.
         */
        if (!EVP_CIPHER_CTX_set_padding(ctx, 0))
            return 0;
    }

#ifndef FIPS_MODULE
    /*
     * Fix for CVE-2023-5363
     * Passing in a size as part of the init call takes effect late
     * so, force such to occur before the initialisation.
     *
     * The FIPS provider's internal library context is used in a manner
     * such that this is not an issue.
     */
    if (params != NULL) {
        OSSL_PARAM param_lens[3] = { OSSL_PARAM_END, OSSL_PARAM_END,
                                     OSSL_PARAM_END };
        OSSL_PARAM *q = param_lens;
        const OSSL_PARAM *p;

        p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
        if (p != NULL)
            memcpy(q++, p, sizeof(*q));

        /*
         * Note that OSSL_CIPHER_PARAM_AEAD_IVLEN is a synonym for
         * OSSL_CIPHER_PARAM_IVLEN so both are covered here.
         */
        p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_IVLEN);
        if (p != NULL)
            memcpy(q++, p, sizeof(*q));

        if (q != param_lens) {
            if (!EVP_CIPHER_CTX_set_params(ctx, param_lens)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_LENGTH);
                return 0;
            }
        }
    }
#endif

    if (is_pipeline)
        return 1;

    if (enc) {
        if (ctx->cipher->einit == NULL) {
            /*
             * We still should be able to set the IV using the new API
             * if the key is not specified and old API is not available
             */
            if (key == NULL && ctx->cipher->einit_skey != NULL) {
                return ctx->cipher->einit_skey(ctx->algctx, NULL,
                                               iv,
                                               iv == NULL ? 0
                                                          : EVP_CIPHER_CTX_get_iv_length(ctx),
                                               params);
            }
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }

        return ctx->cipher->einit(ctx->algctx,
                                  key,
                                  key == NULL ? 0
                                              : EVP_CIPHER_CTX_get_key_length(ctx),
                                  iv,
                                  iv == NULL ? 0
                                             : EVP_CIPHER_CTX_get_iv_length(ctx),
                                  params);
    }

    if (ctx->cipher->dinit == NULL) {
        /*
         * We still should be able to set the IV using the new API
         * if the key is not specified and old API is not available
         */
        if (key == NULL && ctx->cipher->dinit_skey != NULL) {
            return ctx->cipher->dinit_skey(ctx->algctx, NULL,
                                           iv,
                                           iv == NULL ? 0
                                                      : EVP_CIPHER_CTX_get_iv_length(ctx),
                                           params);
        }
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    return ctx->cipher->dinit(ctx->algctx,
                              key,
                              key == NULL ? 0
                                          : EVP_CIPHER_CTX_get_key_length(ctx),
                              iv,
                              iv == NULL ? 0
                                         : EVP_CIPHER_CTX_get_iv_length(ctx),
                                  params);

    /* Code below to be removed when legacy support is dropped. */
 legacy:

    if (cipher != NULL) {
        /*
         * Ensure a context left lying around from last time is cleared (we
         * previously attempted to avoid this if the same ENGINE and
         * EVP_CIPHER could be used).
         */
        if (ctx->cipher) {
            unsigned long flags = ctx->flags;
            EVP_CIPHER_CTX_reset(ctx);
            /* Restore encrypt and flags */
            ctx->encrypt = enc;
            ctx->flags = flags;
        }
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
        if (impl != NULL) {
            if (!ENGINE_init(impl)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }
        } else {
            impl = tmpimpl;
        }
        if (impl != NULL) {
            /* There's an ENGINE for this job ... (apparently) */
            const EVP_CIPHER *c = ENGINE_get_cipher(impl, cipher->nid);

            if (c == NULL) {
                /*
                 * One positive side-effect of US's export control history,
                 * is that we should at least be able to avoid using US
                 * misspellings of "initialisation"?
                 */
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }
            /* We'll use the ENGINE's private cipher definition */
            cipher = c;
            /*
             * Store the ENGINE functional reference so we know 'cipher' came
             * from an ENGINE and we need to release it when done.
             */
            ctx->engine = impl;
        } else {
            ctx->engine = NULL;
        }
#endif

        ctx->cipher = cipher;
        if (ctx->cipher->ctx_size) {
            ctx->cipher_data = OPENSSL_zalloc(ctx->cipher->ctx_size);
            if (ctx->cipher_data == NULL) {
                ctx->cipher = NULL;
                return 0;
            }
        } else {
            ctx->cipher_data = NULL;
        }
        ctx->key_len = cipher->key_len;
        /* Preserve wrap enable flag, zero everything else */
        ctx->flags &= EVP_CIPHER_CTX_FLAG_WRAP_ALLOW;
        if (ctx->cipher->flags & EVP_CIPH_CTRL_INIT) {
            if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_INIT, 0, NULL) <= 0) {
                ctx->cipher = NULL;
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }
        }
    }
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
 skip_to_init:
#endif
    if (ctx->cipher == NULL)
        return 0;

    /* we assume block size is a power of 2 in *cryptUpdate */
    OPENSSL_assert(ctx->cipher->block_size == 1
                   || ctx->cipher->block_size == 8
                   || ctx->cipher->block_size == 16);

    if (!(ctx->flags & EVP_CIPHER_CTX_FLAG_WRAP_ALLOW)
        && EVP_CIPHER_CTX_get_mode(ctx) == EVP_CIPH_WRAP_MODE) {
        ERR_raise(ERR_LIB_EVP, EVP_R_WRAP_MODE_NOT_ALLOWED);
        return 0;
    }

    if ((EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(ctx))
                & EVP_CIPH_CUSTOM_IV) == 0) {
        switch (EVP_CIPHER_CTX_get_mode(ctx)) {

        case EVP_CIPH_STREAM_CIPHER:
        case EVP_CIPH_ECB_MODE:
            break;

        case EVP_CIPH_CFB_MODE:
        case EVP_CIPH_OFB_MODE:

            ctx->num = 0;
            /* fall-through */

        case EVP_CIPH_CBC_MODE:
            n = EVP_CIPHER_CTX_get_iv_length(ctx);
            if (n < 0 || n > (int)sizeof(ctx->iv)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_IV_LENGTH);
                return 0;
            }
            if (iv != NULL)
                memcpy(ctx->oiv, iv, n);
            memcpy(ctx->iv, ctx->oiv, n);
            break;

        case EVP_CIPH_CTR_MODE:
            ctx->num = 0;
            /* Don't reuse IV for CTR mode */
            if (iv != NULL) {
                n = EVP_CIPHER_CTX_get_iv_length(ctx);
                if (n <= 0 || n > (int)sizeof(ctx->iv)) {
                    ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_IV_LENGTH);
                    return 0;
                }
                memcpy(ctx->iv, iv, n);
            }
            break;

        default:
            return 0;
        }
    }

    if (key != NULL || (ctx->cipher->flags & EVP_CIPH_ALWAYS_CALL_INIT)) {
        if (!ctx->cipher->init(ctx, key, iv, enc))
            return 0;
    }
    ctx->buf_len = 0;
    ctx->final_used = 0;
    ctx->block_mask = ctx->cipher->block_size - 1;
    return 1;
}

/*
 * This function is basically evp_cipher_init_internal without ENGINE support.
 * They should be combined when engines are not supported any longer.
 */
static int evp_cipher_init_skey_internal(EVP_CIPHER_CTX *ctx,
                                         const EVP_CIPHER *cipher,
                                         const EVP_SKEY *skey,
                                         const unsigned char *iv, size_t iv_len,
                                         int enc, const OSSL_PARAM params[])
{
    int ret;

    /*
     * enc == 1 means we are encrypting.
     * enc == 0 means we are decrypting.
     * enc == -1 means, use the previously initialised value for encrypt/decrypt
     */
    if (enc == -1)
        enc = ctx->encrypt;
    else
        ctx->encrypt = enc != 0;

    if (cipher == NULL && ctx->cipher == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    /*
     * If there are engines involved then we throw an error
     */
    if (ctx->engine != NULL
            || (cipher != NULL && cipher->origin == EVP_ORIG_METH)
            || (cipher == NULL && ctx->cipher != NULL
                && ctx->cipher->origin == EVP_ORIG_METH)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }
    /*
     * Ensure a context left lying around from last time is cleared
     * (legacy code)
     */
    if (cipher != NULL && ctx->cipher != NULL) {
        if (ctx->cipher->cleanup != NULL && !ctx->cipher->cleanup(ctx))
            return 0;
        OPENSSL_clear_free(ctx->cipher_data, ctx->cipher->ctx_size);
        ctx->cipher_data = NULL;
    }

    /* Ensure a context left lying around from last time is cleared */
    if (cipher != NULL && ctx->cipher != NULL) {
        unsigned long flags = ctx->flags;

        EVP_CIPHER_CTX_reset(ctx);
        /* Restore encrypt and flags */
        ctx->encrypt = enc;
        ctx->flags = flags;
    }

    if (cipher == NULL)
        cipher = ctx->cipher;

    if (cipher->prov == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    if (cipher != ctx->fetched_cipher) {
        if (!EVP_CIPHER_up_ref((EVP_CIPHER *)cipher)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
        EVP_CIPHER_free(ctx->fetched_cipher);
        /* Coverity false positive, the reference counting is confusing it */
        /* coverity[use_after_free] */
        ctx->fetched_cipher = (EVP_CIPHER *)cipher;
    }
    ctx->cipher = cipher;
    if (ctx->algctx == NULL) {
        ctx->algctx = ctx->cipher->newctx(ossl_provider_ctx(cipher->prov));
        if (ctx->algctx == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
    }

    if (skey != NULL && ctx->cipher->prov != skey->skeymgmt->prov) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    if ((ctx->flags & EVP_CIPH_NO_PADDING) != 0) {
        /*
         * If this ctx was already set up for no padding then we need to tell
         * the new cipher about it.
         */
        if (!EVP_CIPHER_CTX_set_padding(ctx, 0))
            return 0;
    }

    if (iv == NULL)
        iv_len = 0;

    /* We have a data managed via key management, using the new callbacks */
    if (enc) {
        if (ctx->cipher->einit_skey == NULL) {
            /*
             *  When skey is NULL, it's a multiple-step init as the current API does.
             *  Otherwise we try to fallback for providers that do not support SKEYs.
             */
            const unsigned char *keydata = NULL;
            size_t keylen = 0;

            if (skey != NULL && !EVP_SKEY_get0_raw_key(skey, &keydata, &keylen)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }

            ret = ctx->cipher->einit(ctx->algctx, keydata, keylen,
                                     iv, iv_len, params);
        } else {
            ret = ctx->cipher->einit_skey(ctx->algctx,
                                          skey == NULL ? NULL : skey->keydata,
                                          iv, iv_len, params);
        }
    } else {
        if (ctx->cipher->dinit_skey == NULL) {
            /*
             *  When skey is NULL, it's a multiple-step init as the current API does.
             *  Otherwise we try to fallback for providers that do not support SKEYs.
             */
            const unsigned char *keydata = NULL;
            size_t keylen = 0;

            if (skey != NULL && !EVP_SKEY_get0_raw_key(skey, &keydata, &keylen)) {
                ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
                return 0;
            }

            ret = ctx->cipher->dinit(ctx->algctx, keydata, keylen,
                                     iv, iv_len, params);
        } else {
            ret = ctx->cipher->dinit_skey(ctx->algctx,
                                          skey == NULL ? NULL : skey->keydata,
                                          iv, iv_len, params);
        }
    }

    return ret;
}

int EVP_CipherInit_SKEY(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                        EVP_SKEY *skey, const unsigned char *iv, size_t iv_len,
                        int enc, const OSSL_PARAM params[])
{
    return evp_cipher_init_skey_internal(ctx, cipher, skey, iv, iv_len, enc, params);
}

int EVP_CipherInit_ex2(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                       const unsigned char *key, const unsigned char *iv,
                       int enc, const OSSL_PARAM params[])
{
    return evp_cipher_init_internal(ctx, cipher, NULL, key, iv, enc, 0, params);
}

int EVP_CipherInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                   const unsigned char *key, const unsigned char *iv, int enc)
{
    if (cipher != NULL)
        EVP_CIPHER_CTX_reset(ctx);
    return evp_cipher_init_internal(ctx, cipher, NULL, key, iv, enc, 0, NULL);
}

int EVP_CipherInit_ex(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                      ENGINE *impl, const unsigned char *key,
                      const unsigned char *iv, int enc)
{
    return evp_cipher_init_internal(ctx, cipher, impl, key, iv, enc, 0, NULL);
}

int EVP_CipherPipelineEncryptInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                                  const unsigned char *key, size_t keylen,
                                  size_t numpipes,
                                  const unsigned char **iv, size_t ivlen)
{
    if (numpipes > EVP_MAX_PIPES) {
        ERR_raise(ERR_LIB_EVP, EVP_R_TOO_MANY_PIPES);
        return 0;
    }

    ctx->numpipes = numpipes;

    if (!evp_cipher_init_internal(ctx, cipher, NULL, NULL, NULL, 1, 1,
                                  NULL))
        return 0;

    if (ctx->cipher->p_einit == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    return ctx->cipher->p_einit(ctx->algctx,
                                key,
                                keylen,
                                numpipes,
                                iv,
                                ivlen,
                                NULL);
}

int EVP_CipherPipelineDecryptInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                                  const unsigned char *key, size_t keylen,
                                  size_t numpipes,
                                  const unsigned char **iv, size_t ivlen)
{
    if (numpipes > EVP_MAX_PIPES) {
        ERR_raise(ERR_LIB_EVP, EVP_R_TOO_MANY_PIPES);
        return 0;
    }

    ctx->numpipes = numpipes;

    if (!evp_cipher_init_internal(ctx, cipher, NULL, NULL, NULL, 0, 1,
                                  NULL))
        return 0;

    if (ctx->cipher->p_dinit == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    return ctx->cipher->p_dinit(ctx->algctx,
                                key,
                                keylen,
                                numpipes,
                                iv,
                                ivlen,
                                NULL);
}

int EVP_CipherUpdate(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl,
                     const unsigned char *in, int inl)
{
    if (ctx->encrypt)
        return EVP_EncryptUpdate(ctx, out, outl, in, inl);
    else
        return EVP_DecryptUpdate(ctx, out, outl, in, inl);
}

int EVP_CipherPipelineUpdate(EVP_CIPHER_CTX *ctx,
                             unsigned char **out, size_t *outl,
                             const size_t *outsize,
                             const unsigned char **in, const size_t *inl)
{
    size_t i;

    if (ossl_unlikely(outl == NULL || inl == NULL)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher->prov == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher->p_cupdate == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    for (i = 0; i < ctx->numpipes; i++)
        outl[i] = 0;

    return ctx->cipher->p_cupdate(ctx->algctx, ctx->numpipes,
                                  out, outl, outsize,
                                  in, inl);
}

int EVP_CipherFinal_ex(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    if (ctx->encrypt)
        return EVP_EncryptFinal_ex(ctx, out, outl);
    else
        return EVP_DecryptFinal_ex(ctx, out, outl);
}

int EVP_CipherFinal(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    if (ctx->encrypt)
        return EVP_EncryptFinal(ctx, out, outl);
    else
        return EVP_DecryptFinal(ctx, out, outl);
}

int EVP_CipherPipelineFinal(EVP_CIPHER_CTX *ctx,
                            unsigned char **out, size_t *outl,
                            const size_t *outsize)
{
    size_t i;

    if (ossl_unlikely(outl == NULL)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher->prov == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher->p_cfinal == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    for (i = 0; i < ctx->numpipes; i++)
        outl[i] = 0;

    return ctx->cipher->p_cfinal(ctx->algctx, ctx->numpipes,
                                 out, outl, outsize);
}

int EVP_EncryptInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                    const unsigned char *key, const unsigned char *iv)
{
    return EVP_CipherInit(ctx, cipher, key, iv, 1);
}

int EVP_EncryptInit_ex(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                       ENGINE *impl, const unsigned char *key,
                       const unsigned char *iv)
{
    return EVP_CipherInit_ex(ctx, cipher, impl, key, iv, 1);
}

int EVP_EncryptInit_ex2(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                        const unsigned char *key, const unsigned char *iv,
                        const OSSL_PARAM params[])
{
    return EVP_CipherInit_ex2(ctx, cipher, key, iv, 1, params);
}

int EVP_DecryptInit(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                    const unsigned char *key, const unsigned char *iv)
{
    return EVP_CipherInit(ctx, cipher, key, iv, 0);
}

int EVP_DecryptInit_ex(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                       ENGINE *impl, const unsigned char *key,
                       const unsigned char *iv)
{
    return EVP_CipherInit_ex(ctx, cipher, impl, key, iv, 0);
}

int EVP_DecryptInit_ex2(EVP_CIPHER_CTX *ctx, const EVP_CIPHER *cipher,
                        const unsigned char *key, const unsigned char *iv,
                        const OSSL_PARAM params[])
{
    return EVP_CipherInit_ex2(ctx, cipher, key, iv, 0, params);
}

/*
 * According to the letter of standard difference between pointers
 * is specified to be valid only within same object. This makes
 * it formally challenging to determine if input and output buffers
 * are not partially overlapping with standard pointer arithmetic.
 */
#ifdef PTRDIFF_T
# undef PTRDIFF_T
#endif
#if defined(OPENSSL_SYS_VMS) && __INITIAL_POINTER_SIZE==64
/*
 * Then we have VMS that distinguishes itself by adhering to
 * sizeof(size_t)==4 even in 64-bit builds, which means that
 * difference between two pointers might be truncated to 32 bits.
 * In the context one can even wonder how comparison for
 * equality is implemented. To be on the safe side we adhere to
 * PTRDIFF_T even for comparison for equality.
 */
# define PTRDIFF_T uint64_t
#else
# define PTRDIFF_T size_t
#endif

int ossl_is_partially_overlapping(const void *ptr1, const void *ptr2, int len)
{
    PTRDIFF_T diff = (PTRDIFF_T)ptr1-(PTRDIFF_T)ptr2;
    /*
     * Check for partially overlapping buffers. [Binary logical
     * operations are used instead of boolean to minimize number
     * of conditional branches.]
     */
    int overlapped = (len > 0) & (diff != 0) & ((diff < (PTRDIFF_T)len) |
                                                (diff > (0 - (PTRDIFF_T)len)));

    return overlapped;
}

static int evp_EncryptDecryptUpdate(EVP_CIPHER_CTX *ctx,
                                    unsigned char *out, int *outl,
                                    const unsigned char *in, int inl)
{
    int i, j, bl, cmpl = inl;

    if (EVP_CIPHER_CTX_test_flags(ctx, EVP_CIPH_FLAG_LENGTH_BITS))
        cmpl = safe_div_round_up_int(cmpl, 8, NULL);

    bl = ctx->cipher->block_size;

    if (ctx->cipher->flags & EVP_CIPH_FLAG_CUSTOM_CIPHER) {
        /* If block size > 1 then the cipher will have to do this check */
        if (bl == 1 && ossl_is_partially_overlapping(out, in, cmpl)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_PARTIALLY_OVERLAPPING);
            return 0;
        }

        i = ctx->cipher->do_cipher(ctx, out, in, inl);
        if (i < 0)
            return 0;
        else
            *outl = i;
        return 1;
    }

    if (inl <= 0) {
        *outl = 0;
        return inl == 0;
    }
    if (ossl_is_partially_overlapping(out + ctx->buf_len, in, cmpl)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_PARTIALLY_OVERLAPPING);
        return 0;
    }

    if (ctx->buf_len == 0 && (inl & (ctx->block_mask)) == 0) {
        if (ctx->cipher->do_cipher(ctx, out, in, inl)) {
            *outl = inl;
            return 1;
        } else {
            *outl = 0;
            return 0;
        }
    }
    i = ctx->buf_len;
    OPENSSL_assert(bl <= (int)sizeof(ctx->buf));
    if (i != 0) {
        if (bl - i > inl) {
            memcpy(&(ctx->buf[i]), in, inl);
            ctx->buf_len += inl;
            *outl = 0;
            return 1;
        } else {
            j = bl - i;

            /*
             * Once we've processed the first j bytes from in, the amount of
             * data left that is a multiple of the block length is:
             * (inl - j) & ~(bl - 1)
             * We must ensure that this amount of data, plus the one block that
             * we process from ctx->buf does not exceed INT_MAX
             */
            if (((inl - j) & ~(bl - 1)) > INT_MAX - bl) {
                ERR_raise(ERR_LIB_EVP, EVP_R_OUTPUT_WOULD_OVERFLOW);
                return 0;
            }
            memcpy(&(ctx->buf[i]), in, j);
            inl -= j;
            in += j;
            if (!ctx->cipher->do_cipher(ctx, out, ctx->buf, bl))
                return 0;
            out += bl;
            *outl = bl;
        }
    } else
        *outl = 0;
    i = inl & (bl - 1);
    inl -= i;
    if (inl > 0) {
        if (!ctx->cipher->do_cipher(ctx, out, in, inl))
            return 0;
        *outl += inl;
    }

    if (i != 0)
        memcpy(ctx->buf, &(in[inl]), i);
    ctx->buf_len = i;
    return 1;
}


int EVP_EncryptUpdate(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl,
                      const unsigned char *in, int inl)
{
    int ret;
    size_t soutl, inl_ = (size_t)inl;
    int blocksize;

    if (ossl_likely(outl != NULL)) {
        *outl = 0;
    } else {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* Prevent accidental use of decryption context when encrypting */
    if (ossl_unlikely(!ctx->encrypt)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher->prov == NULL))
        goto legacy;

    blocksize = ctx->cipher->block_size;

    if (ossl_unlikely(ctx->cipher->cupdate == NULL || blocksize < 1)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }

    ret = ctx->cipher->cupdate(ctx->algctx, out, &soutl,
                               inl_ + (size_t)(blocksize == 1 ? 0 : blocksize),
                               in, inl_);

    if (ossl_likely(ret)) {
        if (soutl > INT_MAX) {
            ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
            return 0;
        }
        *outl = soutl;
    }

    return ret;

    /* Code below to be removed when legacy support is dropped. */
 legacy:

    return evp_EncryptDecryptUpdate(ctx, out, outl, in, inl);
}

int EVP_EncryptFinal(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    int ret;
    ret = EVP_EncryptFinal_ex(ctx, out, outl);
    return ret;
}

int EVP_EncryptFinal_ex(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    int n, ret;
    unsigned int i, b, bl;
    size_t soutl;
    int blocksize;

    if (outl != NULL) {
        *outl = 0;
    } else {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* Prevent accidental use of decryption context when encrypting */
    if (!ctx->encrypt) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ctx->cipher == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }
    if (ctx->cipher->prov == NULL)
        goto legacy;

    blocksize = EVP_CIPHER_CTX_get_block_size(ctx);

    if (blocksize < 1 || ctx->cipher->cfinal == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    ret = ctx->cipher->cfinal(ctx->algctx, out, &soutl,
                              blocksize == 1 ? 0 : blocksize);

    if (ret) {
        if (soutl > INT_MAX) {
            ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
            return 0;
        }
        *outl = soutl;
    }

    return ret;

    /* Code below to be removed when legacy support is dropped. */
 legacy:

    if (ctx->cipher->flags & EVP_CIPH_FLAG_CUSTOM_CIPHER) {
        ret = ctx->cipher->do_cipher(ctx, out, NULL, 0);
        if (ret < 0)
            return 0;
        else
            *outl = ret;
        return 1;
    }

    b = ctx->cipher->block_size;
    OPENSSL_assert(b <= sizeof(ctx->buf));
    if (b == 1) {
        *outl = 0;
        return 1;
    }
    bl = ctx->buf_len;
    if (ctx->flags & EVP_CIPH_NO_PADDING) {
        if (bl) {
            ERR_raise(ERR_LIB_EVP, EVP_R_DATA_NOT_MULTIPLE_OF_BLOCK_LENGTH);
            return 0;
        }
        *outl = 0;
        return 1;
    }

    n = b - bl;
    for (i = bl; i < b; i++)
        ctx->buf[i] = n;
    ret = ctx->cipher->do_cipher(ctx, out, ctx->buf, b);

    if (ret)
        *outl = b;

    return ret;
}

int EVP_DecryptUpdate(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl,
                      const unsigned char *in, int inl)
{
    int fix_len, cmpl = inl, ret;
    unsigned int b;
    size_t soutl, inl_ = (size_t)inl;
    int blocksize;

    if (ossl_likely(outl != NULL)) {
        *outl = 0;
    } else {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* Prevent accidental use of encryption context when decrypting */
    if (ossl_unlikely(ctx->encrypt)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ossl_unlikely(ctx->cipher == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }
    if (ossl_unlikely(ctx->cipher->prov == NULL))
        goto legacy;

    blocksize = EVP_CIPHER_CTX_get_block_size(ctx);

    if (ossl_unlikely(ctx->cipher->cupdate == NULL || blocksize < 1)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
        return 0;
    }
    ret = ctx->cipher->cupdate(ctx->algctx, out, &soutl,
                               inl_ + (size_t)(blocksize == 1 ? 0 : blocksize),
                               in, inl_);

    if (ossl_likely(ret)) {
        if (soutl > INT_MAX) {
            ERR_raise(ERR_LIB_EVP, EVP_R_UPDATE_ERROR);
            return 0;
        }
        *outl = soutl;
    }

    return ret;

    /* Code below to be removed when legacy support is dropped. */
 legacy:

    b = ctx->cipher->block_size;

    if (EVP_CIPHER_CTX_test_flags(ctx, EVP_CIPH_FLAG_LENGTH_BITS))
        cmpl = safe_div_round_up_int(cmpl, 8, NULL);

    if (ctx->cipher->flags & EVP_CIPH_FLAG_CUSTOM_CIPHER) {
        if (b == 1 && ossl_is_partially_overlapping(out, in, cmpl)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_PARTIALLY_OVERLAPPING);
            return 0;
        }

        fix_len = ctx->cipher->do_cipher(ctx, out, in, inl);
        if (fix_len < 0) {
            *outl = 0;
            return 0;
        } else
            *outl = fix_len;
        return 1;
    }

    if (inl <= 0) {
        *outl = 0;
        return inl == 0;
    }

    if (ctx->flags & EVP_CIPH_NO_PADDING)
        return evp_EncryptDecryptUpdate(ctx, out, outl, in, inl);

    OPENSSL_assert(b <= sizeof(ctx->final));

    if (ctx->final_used) {
        /* see comment about PTRDIFF_T comparison above */
        if (((PTRDIFF_T)out == (PTRDIFF_T)in)
            || ossl_is_partially_overlapping(out, in, b)) {
            ERR_raise(ERR_LIB_EVP, EVP_R_PARTIALLY_OVERLAPPING);
            return 0;
        }
        /*
         * final_used is only ever set if buf_len is 0. Therefore the maximum
         * length output we will ever see from evp_EncryptDecryptUpdate is
         * the maximum multiple of the block length that is <= inl, or just:
         * inl & ~(b - 1)
         * Since final_used has been set then the final output length is:
         * (inl & ~(b - 1)) + b
         * This must never exceed INT_MAX
         */
        if ((inl & ~(b - 1)) > INT_MAX - b) {
            ERR_raise(ERR_LIB_EVP, EVP_R_OUTPUT_WOULD_OVERFLOW);
            return 0;
        }
        memcpy(out, ctx->final, b);
        out += b;
        fix_len = 1;
    } else
        fix_len = 0;

    if (!evp_EncryptDecryptUpdate(ctx, out, outl, in, inl))
        return 0;

    /*
     * if we have 'decrypted' a multiple of block size, make sure we have a
     * copy of this last block
     */
    if (b > 1 && !ctx->buf_len) {
        *outl -= b;
        ctx->final_used = 1;
        memcpy(ctx->final, &out[*outl], b);
    } else
        ctx->final_used = 0;

    if (fix_len)
        *outl += b;

    return 1;
}

int EVP_DecryptFinal(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    int ret;
    ret = EVP_DecryptFinal_ex(ctx, out, outl);
    return ret;
}

int EVP_DecryptFinal_ex(EVP_CIPHER_CTX *ctx, unsigned char *out, int *outl)
{
    int i, n;
    unsigned int b;
    size_t soutl;
    int ret;
    int blocksize;

    if (outl != NULL) {
        *outl = 0;
    } else {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* Prevent accidental use of encryption context when decrypting */
    if (ctx->encrypt) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_OPERATION);
        return 0;
    }

    if (ctx->cipher == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    if (ctx->cipher->prov == NULL)
        goto legacy;

    blocksize = EVP_CIPHER_CTX_get_block_size(ctx);

    if (blocksize < 1 || ctx->cipher->cfinal == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
        return 0;
    }

    ret = ctx->cipher->cfinal(ctx->algctx, out, &soutl,
                              blocksize == 1 ? 0 : blocksize);

    if (ret) {
        if (soutl > INT_MAX) {
            ERR_raise(ERR_LIB_EVP, EVP_R_FINAL_ERROR);
            return 0;
        }
        *outl = soutl;
    }

    return ret;

    /* Code below to be removed when legacy support is dropped. */
 legacy:

    *outl = 0;
    if (ctx->cipher->flags & EVP_CIPH_FLAG_CUSTOM_CIPHER) {
        i = ctx->cipher->do_cipher(ctx, out, NULL, 0);
        if (i < 0)
            return 0;
        else
            *outl = i;
        return 1;
    }

    b = ctx->cipher->block_size;
    if (ctx->flags & EVP_CIPH_NO_PADDING) {
        if (ctx->buf_len) {
            ERR_raise(ERR_LIB_EVP, EVP_R_DATA_NOT_MULTIPLE_OF_BLOCK_LENGTH);
            return 0;
        }
        *outl = 0;
        return 1;
    }
    if (b > 1) {
        if (ctx->buf_len || !ctx->final_used) {
            ERR_raise(ERR_LIB_EVP, EVP_R_WRONG_FINAL_BLOCK_LENGTH);
            return 0;
        }
        OPENSSL_assert(b <= sizeof(ctx->final));

        /*
         * The following assumes that the ciphertext has been authenticated.
         * Otherwise it provides a padding oracle.
         */
        n = ctx->final[b - 1];
        if (n == 0 || n > (int)b) {
            ERR_raise(ERR_LIB_EVP, EVP_R_BAD_DECRYPT);
            return 0;
        }
        for (i = 0; i < n; i++) {
            if (ctx->final[--b] != n) {
                ERR_raise(ERR_LIB_EVP, EVP_R_BAD_DECRYPT);
                return 0;
            }
        }
        n = ctx->cipher->block_size - n;
        for (i = 0; i < n; i++)
            out[i] = ctx->final[i];
        *outl = n;
    }
    return 1;
}

int EVP_CIPHER_CTX_set_key_length(EVP_CIPHER_CTX *c, int keylen)
{
    if (c->cipher->prov != NULL) {
        int ok;
        OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
        size_t len;

        if (EVP_CIPHER_CTX_get_key_length(c) == keylen)
            return 1;

        /* Check the cipher actually understands this parameter */
        if (OSSL_PARAM_locate_const(EVP_CIPHER_settable_ctx_params(c->cipher),
                                    OSSL_CIPHER_PARAM_KEYLEN) == NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY_LENGTH);
            return 0;
        }

        params[0] = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_KEYLEN, &len);
        if (!OSSL_PARAM_set_int(params, keylen))
            return 0;
        ok = evp_do_ciph_ctx_setparams(c->cipher, c->algctx, params);
        if (ok <= 0)
            return 0;
        c->key_len = keylen;
        return 1;
    }

    /* Code below to be removed when legacy support is dropped. */

    /*
     * Note there have never been any built-in ciphers that define this flag
     * since it was first introduced.
     */
    if (c->cipher->flags & EVP_CIPH_CUSTOM_KEY_LENGTH)
        return EVP_CIPHER_CTX_ctrl(c, EVP_CTRL_SET_KEY_LENGTH, keylen, NULL);
    if (EVP_CIPHER_CTX_get_key_length(c) == keylen)
        return 1;
    if ((keylen > 0) && (c->cipher->flags & EVP_CIPH_VARIABLE_LENGTH)) {
        c->key_len = keylen;
        return 1;
    }
    ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY_LENGTH);
    return 0;
}

int EVP_CIPHER_CTX_set_padding(EVP_CIPHER_CTX *ctx, int pad)
{
    int ok;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    unsigned int pd = pad;

    if (pad)
        ctx->flags &= ~EVP_CIPH_NO_PADDING;
    else
        ctx->flags |= EVP_CIPH_NO_PADDING;

    if (ctx->cipher != NULL && ctx->cipher->prov == NULL)
        return 1;
    params[0] = OSSL_PARAM_construct_uint(OSSL_CIPHER_PARAM_PADDING, &pd);
    ok = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);

    return ok != 0;
}

int EVP_CIPHER_CTX_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg, void *ptr)
{
    int ret = EVP_CTRL_RET_UNSUPPORTED;
    int set_params = 1;
    size_t sz = arg;
    unsigned int i;
    OSSL_PARAM params[4] = {
        OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END
    };

    if (ctx == NULL || ctx->cipher == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_CIPHER_SET);
        return 0;
    }

    if (ctx->cipher->prov == NULL)
        goto legacy;

    switch (type) {
    case EVP_CTRL_SET_KEY_LENGTH:
        if (arg < 0)
            return 0;
        if (ctx->key_len == arg)
            /* Skip calling into provider if unchanged. */
            return 1;
        params[0] = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_KEYLEN, &sz);
        ctx->key_len = -1;
        break;
    case EVP_CTRL_RAND_KEY:      /* Used by DES */
        set_params = 0;
        params[0] =
            OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_RANDOM_KEY,
                                              ptr, sz);
        break;

    case EVP_CTRL_INIT:
        /*
         * EVP_CTRL_INIT is purely legacy, no provider counterpart.
         * As a matter of fact, this should be dead code, but some caller
         * might still do a direct control call with this command, so...
         * Legacy methods return 1 except for exceptional circumstances, so
         * we do the same here to not be disruptive.
         */
        return 1;
    case EVP_CTRL_SET_PIPELINE_OUTPUT_BUFS: /* Used by DASYNC */
    default:
        goto end;
    case EVP_CTRL_AEAD_SET_IVLEN:
        if (arg < 0)
            return 0;
        if (ctx->iv_len == arg)
            /* Skip calling into provider if unchanged. */
            return 1;
        params[0] = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_IVLEN, &sz);
        ctx->iv_len = -1;
        break;
    case EVP_CTRL_CCM_SET_L:
        if (arg < 2 || arg > 8)
            return 0;
        sz = 15 - arg;
        params[0] = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_IVLEN, &sz);
        ctx->iv_len = -1;
        break;
    case EVP_CTRL_AEAD_SET_IV_FIXED:
        params[0] = OSSL_PARAM_construct_octet_string(
                        OSSL_CIPHER_PARAM_AEAD_TLS1_IV_FIXED, ptr, sz);
        break;
    case EVP_CTRL_GCM_IV_GEN:
        set_params = 0;
        if (arg < 0)
            sz = 0; /* special case that uses the iv length */
        params[0] = OSSL_PARAM_construct_octet_string(
                        OSSL_CIPHER_PARAM_AEAD_TLS1_GET_IV_GEN, ptr, sz);
        break;
    case EVP_CTRL_GCM_SET_IV_INV:
        if (arg < 0)
            return 0;
        params[0] = OSSL_PARAM_construct_octet_string(
                        OSSL_CIPHER_PARAM_AEAD_TLS1_SET_IV_INV, ptr, sz);
        break;
    case EVP_CTRL_GET_RC5_ROUNDS:
        set_params = 0; /* Fall thru */
    case EVP_CTRL_SET_RC5_ROUNDS:
        if (arg < 0)
            return 0;
        i = (unsigned int)arg;
        params[0] = OSSL_PARAM_construct_uint(OSSL_CIPHER_PARAM_ROUNDS, &i);
        break;
    case EVP_CTRL_SET_SPEED:
        if (arg < 0)
            return 0;
        i = (unsigned int)arg;
        params[0] = OSSL_PARAM_construct_uint(OSSL_CIPHER_PARAM_SPEED, &i);
        break;
    case EVP_CTRL_AEAD_GET_TAG:
        set_params = 0; /* Fall thru */
    case EVP_CTRL_AEAD_SET_TAG:
        params[0] = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG,
                                                      ptr, sz);
        break;
    case EVP_CTRL_AEAD_TLS1_AAD:
        /* This one does a set and a get - since it returns a size */
        params[0] =
            OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD,
                                              ptr, sz);
        ret = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            goto end;
        params[0] =
            OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD, &sz);
        ret = evp_do_ciph_ctx_getparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            goto end;
        return sz;
#ifndef OPENSSL_NO_RC2
    case EVP_CTRL_GET_RC2_KEY_BITS:
        set_params = 0; /* Fall thru */
    case EVP_CTRL_SET_RC2_KEY_BITS:
        params[0] = OSSL_PARAM_construct_size_t(OSSL_CIPHER_PARAM_RC2_KEYBITS, &sz);
        break;
#endif /* OPENSSL_NO_RC2 */
#if !defined(OPENSSL_NO_MULTIBLOCK)
    case EVP_CTRL_TLS1_1_MULTIBLOCK_MAX_BUFSIZE:
        params[0] = OSSL_PARAM_construct_size_t(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_SEND_FRAGMENT, &sz);
        ret = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return 0;

        params[0] = OSSL_PARAM_construct_size_t(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_BUFSIZE, &sz);
        params[1] = OSSL_PARAM_construct_end();
        ret = evp_do_ciph_ctx_getparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return 0;
        return sz;
    case EVP_CTRL_TLS1_1_MULTIBLOCK_AAD: {
        EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *p =
            (EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *)ptr;

        if (arg < (int)sizeof(EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM))
            return 0;

        params[0] = OSSL_PARAM_construct_octet_string(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD, (void*)p->inp, p->len);
        params[1] = OSSL_PARAM_construct_uint(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE, &p->interleave);
        ret = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return ret;
        /* Retrieve the return values changed by the set */
        params[0] = OSSL_PARAM_construct_size_t(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_AAD_PACKLEN, &sz);
        params[1] = OSSL_PARAM_construct_uint(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE, &p->interleave);
        params[2] = OSSL_PARAM_construct_end();
        ret = evp_do_ciph_ctx_getparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return 0;
        return sz;
    }
    case EVP_CTRL_TLS1_1_MULTIBLOCK_ENCRYPT: {
        EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *p =
            (EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *)ptr;

        params[0] = OSSL_PARAM_construct_octet_string(
                        OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC, p->out, p->len);

        params[1] = OSSL_PARAM_construct_octet_string(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_IN, (void*)p->inp,
                p->len);
        params[2] = OSSL_PARAM_construct_uint(
                OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE, &p->interleave);
        ret = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return ret;
        params[0] = OSSL_PARAM_construct_size_t(
                        OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_LEN, &sz);
        params[1] = OSSL_PARAM_construct_end();
        ret = evp_do_ciph_ctx_getparams(ctx->cipher, ctx->algctx, params);
        if (ret <= 0)
            return 0;
        return sz;
    }
#endif /* OPENSSL_NO_MULTIBLOCK */
    case EVP_CTRL_AEAD_SET_MAC_KEY:
        if (arg < 0)
            return -1;
        params[0] = OSSL_PARAM_construct_octet_string(
                OSSL_CIPHER_PARAM_AEAD_MAC_KEY, ptr, sz);
        break;
    }

    if (set_params)
        ret = evp_do_ciph_ctx_setparams(ctx->cipher, ctx->algctx, params);
    else
        ret = evp_do_ciph_ctx_getparams(ctx->cipher, ctx->algctx, params);
    goto end;

    /* Code below to be removed when legacy support is dropped. */
legacy:
    if (ctx->cipher->ctrl == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CTRL_NOT_IMPLEMENTED);
        return 0;
    }

    ret = ctx->cipher->ctrl(ctx, type, arg, ptr);

 end:
    if (ret == EVP_CTRL_RET_UNSUPPORTED) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CTRL_OPERATION_NOT_IMPLEMENTED);
        return 0;
    }
    return ret;
}

int EVP_CIPHER_get_params(EVP_CIPHER *cipher, OSSL_PARAM params[])
{
    if (cipher != NULL && cipher->get_params != NULL)
        return cipher->get_params(params);
    return 0;
}

int EVP_CIPHER_CTX_set_params(EVP_CIPHER_CTX *ctx, const OSSL_PARAM params[])
{
    int r = 0;
    const OSSL_PARAM *p;

    if (ctx->cipher != NULL && ctx->cipher->set_ctx_params != NULL) {
        r = ctx->cipher->set_ctx_params(ctx->algctx, params);
        if (r > 0) {
            p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
            if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->key_len)) {
                r = 0;
                ctx->key_len = -1;
            }
        }
        if (r > 0) {
            p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_IVLEN);
            if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->iv_len)) {
                r = 0;
                ctx->iv_len = -1;
            }
        }
    }
    return r;
}

int EVP_CIPHER_CTX_get_params(EVP_CIPHER_CTX *ctx, OSSL_PARAM params[])
{
    if (ctx->cipher != NULL && ctx->cipher->get_ctx_params != NULL)
        return ctx->cipher->get_ctx_params(ctx->algctx, params);
    return 0;
}

const OSSL_PARAM *EVP_CIPHER_gettable_params(const EVP_CIPHER *cipher)
{
    if (cipher != NULL && cipher->gettable_params != NULL)
        return cipher->gettable_params(
                   ossl_provider_ctx(EVP_CIPHER_get0_provider(cipher)));
    return NULL;
}

const OSSL_PARAM *EVP_CIPHER_settable_ctx_params(const EVP_CIPHER *cipher)
{
    void *provctx;

    if (cipher != NULL && cipher->settable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_CIPHER_get0_provider(cipher));
        return cipher->settable_ctx_params(NULL, provctx);
    }
    return NULL;
}

const OSSL_PARAM *EVP_CIPHER_gettable_ctx_params(const EVP_CIPHER *cipher)
{
    void *provctx;

    if (cipher != NULL && cipher->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_CIPHER_get0_provider(cipher));
        return cipher->gettable_ctx_params(NULL, provctx);
    }
    return NULL;
}

const OSSL_PARAM *EVP_CIPHER_CTX_settable_params(EVP_CIPHER_CTX *cctx)
{
    void *alg;

    if (cctx != NULL && cctx->cipher->settable_ctx_params != NULL) {
        alg = ossl_provider_ctx(EVP_CIPHER_get0_provider(cctx->cipher));
        return cctx->cipher->settable_ctx_params(cctx->algctx, alg);
    }
    return NULL;
}

const OSSL_PARAM *EVP_CIPHER_CTX_gettable_params(EVP_CIPHER_CTX *cctx)
{
    void *provctx;

    if (cctx != NULL && cctx->cipher->gettable_ctx_params != NULL) {
        provctx = ossl_provider_ctx(EVP_CIPHER_get0_provider(cctx->cipher));
        return cctx->cipher->gettable_ctx_params(cctx->algctx, provctx);
    }
    return NULL;
}

#ifndef FIPS_MODULE
static OSSL_LIB_CTX *EVP_CIPHER_CTX_get_libctx(EVP_CIPHER_CTX *ctx)
{
    const EVP_CIPHER *cipher = ctx->cipher;
    const OSSL_PROVIDER *prov;

    if (cipher == NULL)
        return NULL;

    prov = EVP_CIPHER_get0_provider(cipher);
    return ossl_provider_libctx(prov);
}
#endif

int EVP_CIPHER_CTX_rand_key(EVP_CIPHER_CTX *ctx, unsigned char *key)
{
    if (ctx->cipher->flags & EVP_CIPH_RAND_KEY)
        return EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_RAND_KEY, 0, key);

#ifdef FIPS_MODULE
    return 0;
#else
    {
        int kl;
        OSSL_LIB_CTX *libctx = EVP_CIPHER_CTX_get_libctx(ctx);

        kl = EVP_CIPHER_CTX_get_key_length(ctx);
        if (kl <= 0 || RAND_priv_bytes_ex(libctx, key, kl, 0) <= 0)
            return 0;
        return 1;
    }
#endif /* FIPS_MODULE */
}

EVP_CIPHER_CTX *EVP_CIPHER_CTX_dup(const EVP_CIPHER_CTX *in)
{
    EVP_CIPHER_CTX *out = EVP_CIPHER_CTX_new();

    if (out != NULL && !EVP_CIPHER_CTX_copy(out, in)) {
        EVP_CIPHER_CTX_free(out);
        out = NULL;
    }
    return out;
}

int EVP_CIPHER_CTX_copy(EVP_CIPHER_CTX *out, const EVP_CIPHER_CTX *in)
{
    if ((in == NULL) || (in->cipher == NULL)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INPUT_NOT_INITIALIZED);
        return 0;
    }

    if (in->cipher->prov == NULL)
        goto legacy;

    if (in->cipher->dupctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NOT_ABLE_TO_COPY_CTX);
        return 0;
    }

    EVP_CIPHER_CTX_reset(out);

    *out = *in;
    out->algctx = NULL;

    if (in->fetched_cipher != NULL && !EVP_CIPHER_up_ref(in->fetched_cipher)) {
        out->fetched_cipher = NULL;
        return 0;
    }

    out->algctx = in->cipher->dupctx(in->algctx);
    if (out->algctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NOT_ABLE_TO_COPY_CTX);
        return 0;
    }

    return 1;

    /* Code below to be removed when legacy support is dropped. */
 legacy:

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    /* Make sure it's safe to copy a cipher context using an ENGINE */
    if (in->engine && !ENGINE_init(in->engine)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_ENGINE_LIB);
        return 0;
    }
#endif

    EVP_CIPHER_CTX_reset(out);
    memcpy(out, in, sizeof(*out));

    if (in->cipher_data && in->cipher->ctx_size) {
        out->cipher_data = OPENSSL_malloc(in->cipher->ctx_size);
        if (out->cipher_data == NULL) {
            out->cipher = NULL;
            return 0;
        }
        memcpy(out->cipher_data, in->cipher_data, in->cipher->ctx_size);
    }

    if (in->cipher->flags & EVP_CIPH_CUSTOM_COPY)
        if (!in->cipher->ctrl((EVP_CIPHER_CTX *)in, EVP_CTRL_COPY, 0, out)) {
            out->cipher = NULL;
            ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
            return 0;
        }
    return 1;
}

EVP_CIPHER *evp_cipher_new(void)
{
    EVP_CIPHER *cipher = OPENSSL_zalloc(sizeof(EVP_CIPHER));

    if (cipher != NULL && !CRYPTO_NEW_REF(&cipher->refcnt, 1)) {
        OPENSSL_free(cipher);
        return NULL;
    }
    return cipher;
}

/*
 * FIPS module note: since internal fetches will be entirely
 * provider based, we know that none of its code depends on legacy
 * NIDs or any functionality that use them.
 */
#ifndef FIPS_MODULE
/* After removal of legacy support get rid of the need for legacy NIDs */
static void set_legacy_nid(const char *name, void *vlegacy_nid)
{
    int nid;
    int *legacy_nid = vlegacy_nid;
    /*
     * We use lowest level function to get the associated method, because
     * higher level functions such as EVP_get_cipherbyname() have changed
     * to look at providers too.
     */
    const void *legacy_method = OBJ_NAME_get(name, OBJ_NAME_TYPE_CIPHER_METH);

    if (*legacy_nid == -1)       /* We found a clash already */
        return;
    if (legacy_method == NULL)
        return;
    nid = EVP_CIPHER_get_nid(legacy_method);
    if (*legacy_nid != NID_undef && *legacy_nid != nid) {
        *legacy_nid = -1;
        return;
    }
    *legacy_nid = nid;
}
#endif

static void *evp_cipher_from_algorithm(const int name_id,
                                       const OSSL_ALGORITHM *algodef,
                                       OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_CIPHER *cipher = NULL;
    int fnciphcnt = 0, encinit = 0, decinit = 0, fnpipecnt = 0, fnctxcnt = 0;

    if ((cipher = evp_cipher_new()) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        return NULL;
    }

#ifndef FIPS_MODULE
    cipher->nid = NID_undef;
    if (!evp_names_do_all(prov, name_id, set_legacy_nid, &cipher->nid)
            || cipher->nid == -1) {
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        goto err;
    }
#endif

    cipher->name_id = name_id;
    if ((cipher->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL)
        goto err;

    cipher->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_CIPHER_NEWCTX:
            if (cipher->newctx != NULL)
                break;
            cipher->newctx = OSSL_FUNC_cipher_newctx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_CIPHER_ENCRYPT_INIT:
            if (cipher->einit != NULL)
                break;
            cipher->einit = OSSL_FUNC_cipher_encrypt_init(fns);
            encinit = 1;
            break;
        case OSSL_FUNC_CIPHER_DECRYPT_INIT:
            if (cipher->dinit != NULL)
                break;
            cipher->dinit = OSSL_FUNC_cipher_decrypt_init(fns);
            decinit = 1;
            break;
        case OSSL_FUNC_CIPHER_ENCRYPT_SKEY_INIT:
            if (cipher->einit_skey != NULL)
                break;
            cipher->einit_skey = OSSL_FUNC_cipher_encrypt_skey_init(fns);
            encinit = 1;
            break;
        case OSSL_FUNC_CIPHER_DECRYPT_SKEY_INIT:
            if (cipher->dinit_skey != NULL)
                break;
            cipher->dinit_skey = OSSL_FUNC_cipher_decrypt_skey_init(fns);
            decinit = 1;
            break;
        case OSSL_FUNC_CIPHER_UPDATE:
            if (cipher->cupdate != NULL)
                break;
            cipher->cupdate = OSSL_FUNC_cipher_update(fns);
            fnciphcnt++;
            break;
        case OSSL_FUNC_CIPHER_FINAL:
            if (cipher->cfinal != NULL)
                break;
            cipher->cfinal = OSSL_FUNC_cipher_final(fns);
            fnciphcnt++;
            break;
        case OSSL_FUNC_CIPHER_CIPHER:
            if (cipher->ccipher != NULL)
                break;
            cipher->ccipher = OSSL_FUNC_cipher_cipher(fns);
            break;
        case OSSL_FUNC_CIPHER_PIPELINE_ENCRYPT_INIT:
            if (cipher->p_einit != NULL)
                break;
            cipher->p_einit = OSSL_FUNC_cipher_pipeline_encrypt_init(fns);
            fnpipecnt++;
            break;
        case OSSL_FUNC_CIPHER_PIPELINE_DECRYPT_INIT:
            if (cipher->p_dinit != NULL)
                break;
            cipher->p_dinit = OSSL_FUNC_cipher_pipeline_decrypt_init(fns);
            fnpipecnt++;
            break;
        case OSSL_FUNC_CIPHER_PIPELINE_UPDATE:
            if (cipher->p_cupdate != NULL)
                break;
            cipher->p_cupdate = OSSL_FUNC_cipher_pipeline_update(fns);
            fnpipecnt++;
            break;
        case OSSL_FUNC_CIPHER_PIPELINE_FINAL:
            if (cipher->p_cfinal != NULL)
                break;
            cipher->p_cfinal = OSSL_FUNC_cipher_pipeline_final(fns);
            fnpipecnt++;
            break;
        case OSSL_FUNC_CIPHER_FREECTX:
            if (cipher->freectx != NULL)
                break;
            cipher->freectx = OSSL_FUNC_cipher_freectx(fns);
            fnctxcnt++;
            break;
        case OSSL_FUNC_CIPHER_DUPCTX:
            if (cipher->dupctx != NULL)
                break;
            cipher->dupctx = OSSL_FUNC_cipher_dupctx(fns);
            break;
        case OSSL_FUNC_CIPHER_GET_PARAMS:
            if (cipher->get_params != NULL)
                break;
            cipher->get_params = OSSL_FUNC_cipher_get_params(fns);
            break;
        case OSSL_FUNC_CIPHER_GET_CTX_PARAMS:
            if (cipher->get_ctx_params != NULL)
                break;
            cipher->get_ctx_params = OSSL_FUNC_cipher_get_ctx_params(fns);
            break;
        case OSSL_FUNC_CIPHER_SET_CTX_PARAMS:
            if (cipher->set_ctx_params != NULL)
                break;
            cipher->set_ctx_params = OSSL_FUNC_cipher_set_ctx_params(fns);
            break;
        case OSSL_FUNC_CIPHER_GETTABLE_PARAMS:
            if (cipher->gettable_params != NULL)
                break;
            cipher->gettable_params = OSSL_FUNC_cipher_gettable_params(fns);
            break;
        case OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS:
            if (cipher->gettable_ctx_params != NULL)
                break;
            cipher->gettable_ctx_params =
                OSSL_FUNC_cipher_gettable_ctx_params(fns);
            break;
        case OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS:
            if (cipher->settable_ctx_params != NULL)
                break;
            cipher->settable_ctx_params =
                OSSL_FUNC_cipher_settable_ctx_params(fns);
            break;
        }
    }
    fnciphcnt += encinit + decinit;
    if ((fnciphcnt != 0 && fnciphcnt != 3 && fnciphcnt != 4)
            || (fnciphcnt == 0 && cipher->ccipher == NULL && fnpipecnt == 0)
            || (fnpipecnt != 0 && (fnpipecnt < 3 || cipher->p_cupdate == NULL
                                   || cipher->p_cfinal == NULL))
            || fnctxcnt != 2) {
        /*
         * In order to be a consistent set of functions we must have at least
         * a complete set of "encrypt" functions, or a complete set of "decrypt"
         * functions, or a single "cipher" function. In all cases we need both
         * the "newctx" and "freectx" functions.
         */
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        goto err;
    }
    if (prov != NULL && !ossl_provider_up_ref(prov))
        goto err;

    cipher->prov = prov;

    if (!evp_cipher_cache_constants(cipher)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_CACHE_CONSTANTS_FAILED);
        goto err;
    }

    return cipher;

err:
    EVP_CIPHER_free(cipher);
    return NULL;
}

static int evp_cipher_up_ref(void *cipher)
{
    return EVP_CIPHER_up_ref(cipher);
}

static void evp_cipher_free(void *cipher)
{
    EVP_CIPHER_free(cipher);
}

EVP_CIPHER *EVP_CIPHER_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                             const char *properties)
{
    EVP_CIPHER *cipher =
        evp_generic_fetch(ctx, OSSL_OP_CIPHER, algorithm, properties,
                          evp_cipher_from_algorithm, evp_cipher_up_ref,
                          evp_cipher_free);

    return cipher;
}

EVP_CIPHER *evp_cipher_fetch_from_prov(OSSL_PROVIDER *prov,
                                       const char *algorithm,
                                       const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_CIPHER,
                                       algorithm, properties,
                                       evp_cipher_from_algorithm,
                                       evp_cipher_up_ref,
                                       evp_cipher_free);
}

int EVP_CIPHER_can_pipeline(const EVP_CIPHER *cipher, int enc)
{
    if (((enc && cipher->p_einit != NULL) || (!enc && cipher->p_dinit != NULL))
        && cipher->p_cupdate != NULL && cipher->p_cfinal != NULL)
        return 1;

    return 0;
}

int EVP_CIPHER_up_ref(EVP_CIPHER *cipher)
{
    int ref = 0;

    if (cipher->origin == EVP_ORIG_DYNAMIC)
        CRYPTO_UP_REF(&cipher->refcnt, &ref);
    return 1;
}

void evp_cipher_free_int(EVP_CIPHER *cipher)
{
    OPENSSL_free(cipher->type_name);
    ossl_provider_free(cipher->prov);
    CRYPTO_FREE_REF(&cipher->refcnt);
    OPENSSL_free(cipher);
}

void EVP_CIPHER_free(EVP_CIPHER *cipher)
{
    int i;

    if (cipher == NULL || cipher->origin != EVP_ORIG_DYNAMIC)
        return;

    CRYPTO_DOWN_REF(&cipher->refcnt, &i);
    if (i > 0)
        return;
    evp_cipher_free_int(cipher);
}

void EVP_CIPHER_do_all_provided(OSSL_LIB_CTX *libctx,
                                void (*fn)(EVP_CIPHER *mac, void *arg),
                                void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_CIPHER,
                       (void (*)(void *, void *))fn, arg,
                       evp_cipher_from_algorithm, evp_cipher_up_ref,
                       evp_cipher_free);
}
