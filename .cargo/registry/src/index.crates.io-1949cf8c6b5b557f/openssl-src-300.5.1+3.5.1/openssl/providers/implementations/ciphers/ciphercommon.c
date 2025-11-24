/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Generic dispatch table functions for ciphers.
 */

/* For SSL3_VERSION */
#include <openssl/prov_ssl.h>
#include <openssl/proverr.h>
#include "ciphercommon_local.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "internal/skey.h"
#include "crypto/types.h"

/*-
 * Generic cipher functions for OSSL_PARAM gettables and settables
 */
static const OSSL_PARAM cipher_known_gettable_params[] = {
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_MODE, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_BLOCK_SIZE, NULL),
    OSSL_PARAM_int(OSSL_CIPHER_PARAM_AEAD, NULL),
    OSSL_PARAM_int(OSSL_CIPHER_PARAM_CUSTOM_IV, NULL),
    OSSL_PARAM_int(OSSL_CIPHER_PARAM_CTS, NULL),
    OSSL_PARAM_int(OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK, NULL),
    OSSL_PARAM_int(OSSL_CIPHER_PARAM_HAS_RAND_KEY, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *ossl_cipher_generic_gettable_params(ossl_unused void *provctx)
{
    return cipher_known_gettable_params;
}

int ossl_cipher_generic_get_params(OSSL_PARAM params[], unsigned int md,
                                   uint64_t flags,
                                   size_t kbits, size_t blkbits, size_t ivbits)
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_MODE);
    if (p != NULL && !OSSL_PARAM_set_uint(p, md)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD);
    if (p != NULL
        && !OSSL_PARAM_set_int(p, (flags & PROV_CIPHER_FLAG_AEAD) != 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_CUSTOM_IV);
    if (p != NULL
        && !OSSL_PARAM_set_int(p, (flags & PROV_CIPHER_FLAG_CUSTOM_IV) != 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_CTS);
    if (p != NULL
        && !OSSL_PARAM_set_int(p, (flags & PROV_CIPHER_FLAG_CTS) != 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS1_MULTIBLOCK);
    if (p != NULL
        && !OSSL_PARAM_set_int(p, (flags & PROV_CIPHER_FLAG_TLS1_MULTIBLOCK) != 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_HAS_RAND_KEY);
    if (p != NULL
        && !OSSL_PARAM_set_int(p, (flags & PROV_CIPHER_FLAG_RAND_KEY) != 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, kbits / 8)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_BLOCK_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, blkbits / 8)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ivbits / 8)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(ossl_cipher_generic)
{ OSSL_CIPHER_PARAM_TLS_MAC, OSSL_PARAM_OCTET_PTR, NULL, 0, OSSL_PARAM_UNMODIFIED },
CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(ossl_cipher_generic)

CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(ossl_cipher_generic)
OSSL_PARAM_uint(OSSL_CIPHER_PARAM_USE_BITS, NULL),
OSSL_PARAM_uint(OSSL_CIPHER_PARAM_TLS_VERSION, NULL),
OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_TLS_MAC_SIZE, NULL),
CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(ossl_cipher_generic)

/*
 * Variable key length cipher functions for OSSL_PARAM settables
 */
int ossl_cipher_var_keylen_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_cipher_generic_set_ctx_params(vctx, params))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL) {
        size_t keylen;

        if (!OSSL_PARAM_get_size_t(p, &keylen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ctx->keylen != keylen) {
            ctx->keylen = keylen;
            ctx->key_set = 0;
        }
    }
    return 1;
}

CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(ossl_cipher_var_keylen)
OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(ossl_cipher_var_keylen)

/*-
 * AEAD cipher functions for OSSL_PARAM gettables and settables
 */
static const OSSL_PARAM cipher_aead_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TAGLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_IV, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_UPDATED_IV, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_GET_IV_GEN, NULL, 0),
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_AEAD_IV_GENERATED, NULL),
    OSSL_PARAM_END
};
const OSSL_PARAM *ossl_cipher_aead_gettable_ctx_params(
        ossl_unused void *cctx, ossl_unused void *provctx
    )
{
    return cipher_aead_known_gettable_ctx_params;
}

static const OSSL_PARAM cipher_aead_known_settable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_AEAD_IVLEN, NULL),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_IV_FIXED, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_SET_IV_INV, NULL, 0),
    OSSL_PARAM_END
};
const OSSL_PARAM *ossl_cipher_aead_settable_ctx_params(
        ossl_unused void *cctx, ossl_unused void *provctx
    )
{
    return cipher_aead_known_settable_ctx_params;
}

void ossl_cipher_generic_reset_ctx(PROV_CIPHER_CTX *ctx)
{
    if (ctx != NULL && ctx->alloced) {
        OPENSSL_free(ctx->tlsmac);
        ctx->alloced = 0;
        ctx->tlsmac = NULL;
    }
}

static int cipher_generic_init_internal(PROV_CIPHER_CTX *ctx,
                                        const unsigned char *key, size_t keylen,
                                        const unsigned char *iv, size_t ivlen,
                                        const OSSL_PARAM params[], int enc)
{
    ctx->num = 0;
    ctx->bufsz = 0;
    ctx->updated = 0;
    ctx->enc = enc ? 1 : 0;

    if (!ossl_prov_is_running())
        return 0;

    if (iv != NULL && ctx->mode != EVP_CIPH_ECB_MODE) {
        if (!ossl_cipher_generic_initiv(ctx, iv, ivlen))
            return 0;
    }
    if (iv == NULL && ctx->iv_set
        && (ctx->mode == EVP_CIPH_CBC_MODE
            || ctx->mode == EVP_CIPH_CFB_MODE
            || ctx->mode == EVP_CIPH_OFB_MODE))
        /* reset IV for these modes to keep compatibility with 1.1.1 */
        memcpy(ctx->iv, ctx->oiv, ctx->ivlen);

    if (key != NULL) {
        if (ctx->variable_keylength == 0) {
            if (keylen != ctx->keylen) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
                return 0;
            }
        } else {
            ctx->keylen = keylen;
        }
        if (!ctx->hw->init(ctx, key, ctx->keylen))
            return 0;
        ctx->key_set = 1;
    }
    return ossl_cipher_generic_set_ctx_params(ctx, params);
}

int ossl_cipher_generic_einit(void *vctx, const unsigned char *key,
                              size_t keylen, const unsigned char *iv,
                              size_t ivlen, const OSSL_PARAM params[])
{
    return cipher_generic_init_internal((PROV_CIPHER_CTX *)vctx, key, keylen,
                                        iv, ivlen, params, 1);
}

int ossl_cipher_generic_dinit(void *vctx, const unsigned char *key,
                              size_t keylen, const unsigned char *iv,
                              size_t ivlen, const OSSL_PARAM params[])
{
    return cipher_generic_init_internal((PROV_CIPHER_CTX *)vctx, key, keylen,
                                        iv, ivlen, params, 0);
}

int ossl_cipher_generic_skey_einit(void *vctx, void *skeydata,
                                   const unsigned char *iv, size_t ivlen,
                                   const OSSL_PARAM params[])
{
    PROV_SKEY *key = skeydata;

    return cipher_generic_init_internal((PROV_CIPHER_CTX *)vctx,
                                        key->data, key->length,
                                        iv, ivlen, params, 1);
}

int ossl_cipher_generic_skey_dinit(void *vctx, void *skeydata,
                                   const unsigned char *iv, size_t ivlen,
                                   const OSSL_PARAM params[])
{
    PROV_SKEY *key = skeydata;

    return cipher_generic_init_internal((PROV_CIPHER_CTX *)vctx,
                                        key->data, key->length,
                                        iv, ivlen, params, 0);
}

/* Max padding including padding length byte */
#define MAX_PADDING 256

int ossl_cipher_generic_block_update(void *vctx, unsigned char *out,
                                     size_t *outl, size_t outsize,
                                     const unsigned char *in, size_t inl)
{
    size_t outlint = 0;
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    size_t blksz = ctx->blocksize;
    size_t nextblocks;

    if (!ctx->key_set) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (ctx->tlsversion > 0) {
        /*
         * Each update call corresponds to a TLS record and is individually
         * padded
         */

        /* Sanity check inputs */
        if (in == NULL
                || in != out
                || outsize < inl
                || !ctx->pad) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }

        if (ctx->enc) {
            unsigned char padval;
            size_t padnum, loop;

            /* Add padding */

            padnum = blksz - (inl % blksz);

            if (outsize < inl + padnum) {
                ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
                return 0;
            }

            if (padnum > MAX_PADDING) {
                ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
                return 0;
            }
            padval = (unsigned char)(padnum - 1);
            if (ctx->tlsversion == SSL3_VERSION) {
                if (padnum > 1)
                    memset(out + inl, 0, padnum - 1);
                *(out + inl + padnum - 1) = padval;
            } else {
                /* we need to add 'padnum' padding bytes of value padval */
                for (loop = inl; loop < inl + padnum; loop++)
                    out[loop] = padval;
            }
            inl += padnum;
        }

        if ((inl % blksz) != 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }


        /* Shouldn't normally fail */
        if (!ctx->hw->cipher(ctx, out, in, inl)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }

        if (ctx->alloced) {
            OPENSSL_free(ctx->tlsmac);
            ctx->alloced = 0;
            ctx->tlsmac = NULL;
        }

        /* This only fails if padding is publicly invalid */
        *outl = inl;
        if (!ctx->enc
            && !ossl_cipher_tlsunpadblock(ctx->libctx, ctx->tlsversion,
                                          out, outl,
                                          blksz, &ctx->tlsmac, &ctx->alloced,
                                          ctx->tlsmacsize, 0)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        return 1;
    }

    if (ctx->bufsz != 0)
        nextblocks = ossl_cipher_fillblock(ctx->buf, &ctx->bufsz, blksz,
                                           &in, &inl);
    else
        nextblocks = inl & ~(blksz-1);

    /*
     * If we're decrypting and we end an update on a block boundary we hold
     * the last block back in case this is the last update call and the last
     * block is padded.
     */
    if (ctx->bufsz == blksz && (ctx->enc || inl > 0 || !ctx->pad)) {
        if (outsize < blksz) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        if (!ctx->hw->cipher(ctx, out, ctx->buf, blksz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        ctx->bufsz = 0;
        outlint = blksz;
        out += blksz;
    }
    if (nextblocks > 0) {
        if (!ctx->enc && ctx->pad && nextblocks == inl) {
            if (!ossl_assert(inl >= blksz)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
                return 0;
            }
            nextblocks -= blksz;
        }
        outlint += nextblocks;
        if (outsize < outlint) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
    }
    if (nextblocks > 0) {
        if (!ctx->hw->cipher(ctx, out, in, nextblocks)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        in += nextblocks;
        inl -= nextblocks;
    }
    if (inl != 0
        && !ossl_cipher_trailingdata(ctx->buf, &ctx->bufsz, blksz, &in, &inl)) {
        /* ERR_raise already called */
        return 0;
    }

    *outl = outlint;
    return inl == 0;
}

int ossl_cipher_generic_block_final(void *vctx, unsigned char *out,
                                    size_t *outl, size_t outsize)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    size_t blksz = ctx->blocksize;

    if (!ossl_prov_is_running())
        return 0;

    if (!ctx->key_set) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (ctx->tlsversion > 0) {
        /* We never finalize TLS, so this is an error */
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    if (ctx->enc) {
        if (ctx->pad) {
            ossl_cipher_padblock(ctx->buf, &ctx->bufsz, blksz);
        } else if (ctx->bufsz == 0) {
            *outl = 0;
            return 1;
        } else if (ctx->bufsz != blksz) {
            ERR_raise(ERR_LIB_PROV, PROV_R_WRONG_FINAL_BLOCK_LENGTH);
            return 0;
        }

        if (outsize < blksz) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        if (!ctx->hw->cipher(ctx, out, ctx->buf, blksz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
            return 0;
        }
        ctx->bufsz = 0;
        *outl = blksz;
        return 1;
    }

    /* Decrypting */
    if (ctx->bufsz != blksz) {
        if (ctx->bufsz == 0 && !ctx->pad) {
            *outl = 0;
            return 1;
        }
        ERR_raise(ERR_LIB_PROV, PROV_R_WRONG_FINAL_BLOCK_LENGTH);
        return 0;
    }

    if (!ctx->hw->cipher(ctx, ctx->buf, ctx->buf, blksz)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    if (ctx->pad && !ossl_cipher_unpadblock(ctx->buf, &ctx->bufsz, blksz)) {
        /* ERR_raise already called */
        return 0;
    }

    if (outsize < ctx->bufsz) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }
    memcpy(out, ctx->buf, ctx->bufsz);
    *outl = ctx->bufsz;
    ctx->bufsz = 0;
    return 1;
}

int ossl_cipher_generic_stream_update(void *vctx, unsigned char *out,
                                      size_t *outl, size_t outsize,
                                      const unsigned char *in, size_t inl)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;

    if (!ctx->key_set) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (inl == 0) {
        *outl = 0;
        return 1;
    }

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!ctx->hw->cipher(ctx, out, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    *outl = inl;
    if (!ctx->enc && ctx->tlsversion > 0) {
        /*
        * Remove any TLS padding. Only used by cipher_aes_cbc_hmac_sha1_hw.c and
        * cipher_aes_cbc_hmac_sha256_hw.c
        */
        if (ctx->removetlspad) {
            /*
             * We should have already failed in the cipher() call above if this
             * isn't true.
             */
            if (!ossl_assert(*outl >= (size_t)(out[inl - 1] + 1)))
                return 0;
            /* The actual padding length */
            *outl -= out[inl - 1] + 1;
        }

        /* TLS MAC and explicit IV if relevant. We should have already failed
         * in the cipher() call above if *outl is too short.
         */
        if (!ossl_assert(*outl >= ctx->removetlsfixed))
            return 0;
        *outl -= ctx->removetlsfixed;

        /* Extract the MAC if there is one */
        if (ctx->tlsmacsize > 0) {
            if (*outl < ctx->tlsmacsize)
                return 0;

            ctx->tlsmac = out + *outl - ctx->tlsmacsize;
            *outl -= ctx->tlsmacsize;
        }
    }

    return 1;
}
int ossl_cipher_generic_stream_final(void *vctx, unsigned char *out,
                                     size_t *outl, size_t outsize)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (!ctx->key_set) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    *outl = 0;
    return 1;
}

int ossl_cipher_generic_cipher(void *vctx, unsigned char *out, size_t *outl,
                               size_t outsize, const unsigned char *in,
                               size_t inl)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (!ctx->key_set) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!ctx->hw->cipher(ctx, out, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }

    *outl = inl;
    return 1;
}

int ossl_cipher_generic_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_PADDING);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->pad)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IV);
    if (p != NULL
        && !OSSL_PARAM_set_octet_ptr(p, &ctx->oiv, ctx->ivlen)
        && !OSSL_PARAM_set_octet_string(p, &ctx->oiv, ctx->ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_UPDATED_IV);
    if (p != NULL
        && !OSSL_PARAM_set_octet_ptr(p, &ctx->iv, ctx->ivlen)
        && !OSSL_PARAM_set_octet_string(p, &ctx->iv, ctx->ivlen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_NUM);
    if (p != NULL && !OSSL_PARAM_set_uint(p, ctx->num)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_TLS_MAC);
    if (p != NULL
        && !OSSL_PARAM_set_octet_ptr(p, ctx->tlsmac, ctx->tlsmacsize)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

int ossl_cipher_generic_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_PADDING);
    if (p != NULL) {
        unsigned int pad;

        if (!OSSL_PARAM_get_uint(p, &pad)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ctx->pad = pad ? 1 : 0;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_USE_BITS);
    if (p != NULL) {
        unsigned int bits;

        if (!OSSL_PARAM_get_uint(p, &bits)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ctx->use_bits = bits ? 1 : 0;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS_VERSION);
    if (p != NULL) {
        if (!OSSL_PARAM_get_uint(p, &ctx->tlsversion)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_TLS_MAC_SIZE);
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &ctx->tlsmacsize)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_NUM);
    if (p != NULL) {
        unsigned int num;

        if (!OSSL_PARAM_get_uint(p, &num)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ctx->num = num;
    }
    return 1;
}

int ossl_cipher_generic_initiv(PROV_CIPHER_CTX *ctx, const unsigned char *iv,
                               size_t ivlen)
{
    if (ivlen != ctx->ivlen
        || ivlen > sizeof(ctx->iv)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
        return 0;
    }
    ctx->iv_set = 1;
    memcpy(ctx->iv, iv, ivlen);
    memcpy(ctx->oiv, iv, ivlen);
    return 1;
}

void ossl_cipher_generic_initkey(void *vctx, size_t kbits, size_t blkbits,
                                 size_t ivbits, unsigned int mode,
                                 uint64_t flags, const PROV_CIPHER_HW *hw,
                                 void *provctx)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;

    if ((flags & PROV_CIPHER_FLAG_INVERSE_CIPHER) != 0)
        ctx->inverse_cipher = 1;
    if ((flags & PROV_CIPHER_FLAG_VARIABLE_LENGTH) != 0)
        ctx->variable_keylength = 1;

    ctx->pad = 1;
    ctx->keylen = ((kbits) / 8);
    ctx->ivlen = ((ivbits) / 8);
    ctx->hw = hw;
    ctx->mode = mode;
    ctx->blocksize = blkbits / 8;
    if (provctx != NULL)
        ctx->libctx = PROV_LIBCTX_OF(provctx); /* used for rand */
}
