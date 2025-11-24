/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for ccm mode */

#include <openssl/proverr.h>
#include "prov/ciphercommon.h"
#include "prov/ciphercommon_ccm.h"
#include "prov/providercommon.h"

static int ccm_cipher_internal(PROV_CCM_CTX *ctx, unsigned char *out,
                               size_t *padlen, const unsigned char *in,
                               size_t len);

static int ccm_tls_init(PROV_CCM_CTX *ctx, unsigned char *aad, size_t alen)
{
    size_t len;

    if (!ossl_prov_is_running() || alen != EVP_AEAD_TLS1_AAD_LEN)
        return 0;

    /* Save the aad for later use. */
    memcpy(ctx->buf, aad, alen);
    ctx->tls_aad_len = alen;

    len = ctx->buf[alen - 2] << 8 | ctx->buf[alen - 1];
    if (len < EVP_CCM_TLS_EXPLICIT_IV_LEN)
        return 0;

    /* Correct length for explicit iv. */
    len -= EVP_CCM_TLS_EXPLICIT_IV_LEN;

    if (!ctx->enc) {
        if (len < ctx->m)
            return 0;
        /* Correct length for tag. */
        len -= ctx->m;
    }
    ctx->buf[alen - 2] = (unsigned char)(len >> 8);
    ctx->buf[alen - 1] = (unsigned char)(len & 0xff);

    /* Extra padding: tag appended to record. */
    return ctx->m;
}

static int ccm_tls_iv_set_fixed(PROV_CCM_CTX *ctx, unsigned char *fixed,
                                size_t flen)
{
    if (flen != EVP_CCM_TLS_FIXED_IV_LEN)
        return 0;

    /* Copy to first part of the iv. */
    memcpy(ctx->iv, fixed, flen);
    return 1;
}

static size_t ccm_get_ivlen(PROV_CCM_CTX *ctx)
{
    return 15 - ctx->l;
}

int ossl_ccm_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;
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
        if ((p->data_size & 1) || (p->data_size < 4) || p->data_size > 16) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG_LENGTH);
            return 0;
        }

        if (p->data != NULL) {
            if (ctx->enc) {
                ERR_raise(ERR_LIB_PROV, PROV_R_TAG_NOT_NEEDED);
                return 0;
            }
            memcpy(ctx->buf, p->data, p->data_size);
            ctx->tag_set = 1;
        }
        ctx->m = p->data_size;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_IVLEN);
    if (p != NULL) {
        size_t ivlen;

        if (!OSSL_PARAM_get_size_t(p, &sz)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        ivlen = 15 - sz;
        if (ivlen < 2 || ivlen > 8) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        if (ctx->l != ivlen) {
            ctx->l = ivlen;
            ctx->iv_set = 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        sz = ccm_tls_init(ctx, p->data, p->data_size);
        if (sz == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DATA);
            return 0;
        }
        ctx->tls_aad_pad_sz = sz;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_AEAD_TLS1_IV_FIXED);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
        if (ccm_tls_iv_set_fixed(ctx, p->data, p->data_size) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
    }

    return 1;
}

int ossl_ccm_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ccm_get_ivlen(ctx))) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAGLEN);
    if (p != NULL) {
        size_t m = ctx->m;

        if (!OSSL_PARAM_set_size_t(p, m)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IV);
    if (p != NULL) {
        if (ccm_get_ivlen(ctx) > p->data_size) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        if (!OSSL_PARAM_set_octet_string(p, ctx->iv, p->data_size)
            && !OSSL_PARAM_set_octet_ptr(p, &ctx->iv, p->data_size)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_UPDATED_IV);
    if (p != NULL) {
        if (ccm_get_ivlen(ctx) > p->data_size) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        if (!OSSL_PARAM_set_octet_string(p, ctx->iv, p->data_size)
            && !OSSL_PARAM_set_octet_ptr(p, &ctx->iv, p->data_size)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->keylen)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TLS1_AAD_PAD);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->tls_aad_pad_sz)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_AEAD_TAG);
    if (p != NULL) {
        if (!ctx->enc || !ctx->tag_set) {
            ERR_raise(ERR_LIB_PROV, PROV_R_TAG_NOT_SET);
            return 0;
        }
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
        if (!ctx->hw->gettag(ctx, p->data, p->data_size))
            return 0;
        ctx->tag_set = 0;
        ctx->iv_set = 0;
        ctx->len_set = 0;
    }
    return 1;
}

static int ccm_init(void *vctx, const unsigned char *key, size_t keylen,
                    const unsigned char *iv, size_t ivlen,
                    const OSSL_PARAM params[], int enc)
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;

    if (iv != NULL) {
        if (ivlen != ccm_get_ivlen(ctx)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        memcpy(ctx->iv, iv, ivlen);
        ctx->iv_set = 1;
    }
    if (key != NULL) {
        if (keylen != ctx->keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->hw->setkey(ctx, key, keylen))
            return 0;
    }
    return ossl_ccm_set_ctx_params(ctx, params);
}

int ossl_ccm_einit(void *vctx, const unsigned char *key, size_t keylen,
                   const unsigned char *iv, size_t ivlen,
                   const OSSL_PARAM params[])
{
    return ccm_init(vctx, key, keylen, iv, ivlen, params, 1);
}

int ossl_ccm_dinit(void *vctx, const unsigned char *key, size_t keylen,
                   const unsigned char *iv, size_t ivlen,
                   const OSSL_PARAM params[])
{
    return ccm_init(vctx, key, keylen, iv, ivlen, params, 0);
}

int ossl_ccm_stream_update(void *vctx, unsigned char *out, size_t *outl,
                           size_t outsize, const unsigned char *in,
                           size_t inl)
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (!ccm_cipher_internal(ctx, out, outl, in, inl)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }
    return 1;
}

int ossl_ccm_stream_final(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize)
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;
    int i;

    if (!ossl_prov_is_running())
        return 0;

    i = ccm_cipher_internal(ctx, out, outl, NULL, 0);
    if (i <= 0)
        return 0;

    *outl = 0;
    return 1;
}

int ossl_ccm_cipher(void *vctx, unsigned char *out, size_t *outl, size_t outsize,
                    const unsigned char *in, size_t inl)
{
    PROV_CCM_CTX *ctx = (PROV_CCM_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (ccm_cipher_internal(ctx, out, outl, in, inl) <= 0)
        return 0;

    *outl = inl;
    return 1;
}

/* Copy the buffered iv */
static int ccm_set_iv(PROV_CCM_CTX *ctx, size_t mlen)
{
    const PROV_CCM_HW *hw = ctx->hw;

    if (!hw->setiv(ctx, ctx->iv, ccm_get_ivlen(ctx), mlen))
        return 0;
    ctx->len_set = 1;
    return 1;
}

static int ccm_tls_cipher(PROV_CCM_CTX *ctx,
                          unsigned char *out, size_t *padlen,
                          const unsigned char *in, size_t len)
{
    int rv = 0;
    size_t olen = 0;

    if (!ossl_prov_is_running())
        goto err;

    /* Encrypt/decrypt must be performed in place */
    if (in == NULL || out != in || len < EVP_CCM_TLS_EXPLICIT_IV_LEN + ctx->m)
        goto err;

    /* If encrypting set explicit IV from sequence number (start of AAD) */
    if (ctx->enc)
        memcpy(out, ctx->buf, EVP_CCM_TLS_EXPLICIT_IV_LEN);
    /* Get rest of IV from explicit IV */
    memcpy(ctx->iv + EVP_CCM_TLS_FIXED_IV_LEN, in, EVP_CCM_TLS_EXPLICIT_IV_LEN);
    /* Correct length value */
    len -= EVP_CCM_TLS_EXPLICIT_IV_LEN + ctx->m;
    if (!ccm_set_iv(ctx, len))
        goto err;

    /* Use saved AAD */
    if (!ctx->hw->setaad(ctx, ctx->buf, ctx->tls_aad_len))
        goto err;

    /* Fix buffer to point to payload */
    in += EVP_CCM_TLS_EXPLICIT_IV_LEN;
    out += EVP_CCM_TLS_EXPLICIT_IV_LEN;
    if (ctx->enc) {
        if (!ctx->hw->auth_encrypt(ctx, in, out, len,  out + len, ctx->m))
            goto err;
        olen = len + EVP_CCM_TLS_EXPLICIT_IV_LEN + ctx->m;
    } else {
        if (!ctx->hw->auth_decrypt(ctx, in, out, len,
                                   (unsigned char *)in + len, ctx->m))
            goto err;
        olen = len;
    }
    rv = 1;
err:
    *padlen = olen;
    return rv;
}

static int ccm_cipher_internal(PROV_CCM_CTX *ctx, unsigned char *out,
                               size_t *padlen, const unsigned char *in,
                               size_t len)
{
    int rv = 0;
    size_t olen = 0;
    const PROV_CCM_HW *hw = ctx->hw;

    /* If no key set, return error */
    if (!ctx->key_set)
        return 0;

    if (ctx->tls_aad_len != UNINITIALISED_SIZET)
        return ccm_tls_cipher(ctx, out, padlen, in, len);

    /* EVP_*Final() doesn't return any data */
    if (in == NULL && out != NULL)
        goto finish;

    if (!ctx->iv_set)
        goto err;

    if (out == NULL) {
        if (in == NULL) {
            if (!ccm_set_iv(ctx, len))
                goto err;
        } else {
            /* If we have AAD, we need a message length */
            if (!ctx->len_set && len)
                goto err;
            if (!hw->setaad(ctx, in, len))
                goto err;
        }
    } else {
        /* If not set length yet do it */
        if (!ctx->len_set && !ccm_set_iv(ctx, len))
            goto err;

        if (ctx->enc) {
            if (!hw->auth_encrypt(ctx, in, out, len, NULL, 0))
                goto err;
            ctx->tag_set = 1;
        } else {
            /* The tag must be set before actually decrypting data */
            if (!ctx->tag_set)
                goto err;

            if (!hw->auth_decrypt(ctx, in, out, len, ctx->buf, ctx->m))
                goto err;
            /* Finished - reset flags so calling this method again will fail */
            ctx->iv_set = 0;
            ctx->tag_set = 0;
            ctx->len_set = 0;
        }
    }
    olen = len;
finish:
    rv = 1;
err:
    *padlen = olen;
    return rv;
}

void ossl_ccm_initctx(PROV_CCM_CTX *ctx, size_t keybits, const PROV_CCM_HW *hw)
{
    ctx->keylen = keybits / 8;
    ctx->key_set = 0;
    ctx->iv_set = 0;
    ctx->tag_set = 0;
    ctx->len_set = 0;
    ctx->l = 8;
    ctx->m = 12;
    ctx->tls_aad_len = UNINITIALISED_SIZET;
    ctx->hw = hw;
}
