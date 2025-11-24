/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for gcm mode */

#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "prov/ciphercommon.h"
#include "prov/ciphercommon_gcm.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "internal/param_names.h"

static int gcm_tls_init(PROV_GCM_CTX *dat, unsigned char *aad, size_t aad_len);
static int gcm_tls_iv_set_fixed(PROV_GCM_CTX *ctx, unsigned char *iv,
                                size_t len);
static int gcm_tls_cipher(PROV_GCM_CTX *ctx, unsigned char *out, size_t *padlen,
                          const unsigned char *in, size_t len);
static int gcm_cipher_internal(PROV_GCM_CTX *ctx, unsigned char *out,
                               size_t *padlen, const unsigned char *in,
                               size_t len);

/*
 * Called from EVP_CipherInit when there is currently no context via
 * the new_ctx() function
 */
void ossl_gcm_initctx(void *provctx, PROV_GCM_CTX *ctx, size_t keybits,
                      const PROV_GCM_HW *hw)
{
    ctx->pad = 1;
    ctx->mode = EVP_CIPH_GCM_MODE;
    ctx->taglen = UNINITIALISED_SIZET;
    ctx->tls_aad_len = UNINITIALISED_SIZET;
    ctx->ivlen = (EVP_GCM_TLS_FIXED_IV_LEN + EVP_GCM_TLS_EXPLICIT_IV_LEN);
    ctx->keylen = keybits / 8;
    ctx->hw = hw;
    ctx->libctx = PROV_LIBCTX_OF(provctx);
}

/*
 * Called by EVP_CipherInit via the _einit and _dinit functions
 */
static int gcm_init(void *vctx, const unsigned char *key, size_t keylen,
                    const unsigned char *iv, size_t ivlen,
                    const OSSL_PARAM params[], int enc)
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    ctx->enc = enc;

    if (iv != NULL) {
        if (ivlen == 0 || ivlen > sizeof(ctx->iv)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        ctx->ivlen = ivlen;
        memcpy(ctx->iv, iv, ivlen);
        ctx->iv_state = IV_STATE_BUFFERED;
    }

    if (key != NULL) {
        if (keylen != ctx->keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!ctx->hw->setkey(ctx, key, ctx->keylen))
            return 0;
        ctx->tls_enc_records = 0;
    }
    return ossl_gcm_set_ctx_params(ctx, params);
}

int ossl_gcm_einit(void *vctx, const unsigned char *key, size_t keylen,
                   const unsigned char *iv, size_t ivlen,
                   const OSSL_PARAM params[])
{
    return gcm_init(vctx, key, keylen, iv, ivlen, params, 1);
}

int ossl_gcm_dinit(void *vctx, const unsigned char *key, size_t keylen,
                   const unsigned char *iv, size_t ivlen,
                   const OSSL_PARAM params[])
{
    return gcm_init(vctx, key, keylen, iv, ivlen, params, 0);
}

/* increment counter (64-bit int) by 1 */
static void ctr64_inc(unsigned char *counter)
{
    int n = 8;
    unsigned char c;

    do {
        --n;
        c = counter[n];
        ++c;
        counter[n] = c;
        if (c > 0)
            return;
    } while (n > 0);
}

static int getivgen(PROV_GCM_CTX *ctx, unsigned char *out, size_t olen)
{
    if (!ctx->iv_gen
        || !ctx->key_set
        || !ctx->hw->setiv(ctx, ctx->iv, ctx->ivlen))
        return 0;
    if (olen == 0 || olen > ctx->ivlen)
        olen = ctx->ivlen;
    memcpy(out, ctx->iv + ctx->ivlen - olen, olen);
    /*
     * Invocation field will be at least 8 bytes in size and so no need
     * to check wrap around or increment more than last 8 bytes.
     */
    ctr64_inc(ctx->iv + ctx->ivlen - 8);
    ctx->iv_state = IV_STATE_COPIED;
    return 1;
}

static int setivinv(PROV_GCM_CTX *ctx, unsigned char *in, size_t inl)
{
    if (!ctx->iv_gen
        || !ctx->key_set
        || ctx->enc)
        return 0;

    memcpy(ctx->iv + ctx->ivlen - inl, in, inl);
    if (!ctx->hw->setiv(ctx, ctx->iv, ctx->ivlen))
        return 0;
    ctx->iv_state = IV_STATE_COPIED;
    return 1;
}

int ossl_gcm_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;
    OSSL_PARAM *p;
    size_t sz;
    int type;

    for (p = params; p->key != NULL; p++) {
        type = ossl_param_find_pidx(p->key);
        switch (type) {
        default:
            break;

        case PIDX_CIPHER_PARAM_IVLEN:
            if (!OSSL_PARAM_set_size_t(p, ctx->ivlen)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_KEYLEN:
            if (!OSSL_PARAM_set_size_t(p, ctx->keylen)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TAGLEN:
            {
                size_t taglen = (ctx->taglen != UNINITIALISED_SIZET) ? ctx->taglen :
                                 GCM_TAG_MAX_SIZE;

                if (!OSSL_PARAM_set_size_t(p, taglen)) {
                    ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                    return 0;
                }
            }
            break;

        case PIDX_CIPHER_PARAM_IV:
            if (ctx->iv_state == IV_STATE_UNINITIALISED)
                return 0;
            if (ctx->ivlen > p->data_size) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
                return 0;
            }
            if (!OSSL_PARAM_set_octet_string(p, ctx->iv, ctx->ivlen)
                && !OSSL_PARAM_set_octet_ptr(p, &ctx->iv, ctx->ivlen)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_UPDATED_IV:
            if (ctx->iv_state == IV_STATE_UNINITIALISED)
                return 0;
            if (ctx->ivlen > p->data_size) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
                return 0;
            }
            if (!OSSL_PARAM_set_octet_string(p, ctx->iv, ctx->ivlen)
                && !OSSL_PARAM_set_octet_ptr(p, &ctx->iv, ctx->ivlen)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TLS1_AAD_PAD:
            if (!OSSL_PARAM_set_size_t(p, ctx->tls_aad_pad_sz)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TAG:
            sz = p->data_size;
            if (sz == 0
                || sz > EVP_GCM_TLS_TAG_LEN
                || !ctx->enc
                || ctx->taglen == UNINITIALISED_SIZET) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG);
                return 0;
            }
            if (!OSSL_PARAM_set_octet_string(p, ctx->buf, sz)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TLS1_GET_IV_GEN:
            if (p->data == NULL
                || p->data_type != OSSL_PARAM_OCTET_STRING
                || !getivgen(ctx, p->data, p->data_size))
                return 0;
            break;
        case PIDX_CIPHER_PARAM_AEAD_IV_GENERATED:
            if (!OSSL_PARAM_set_uint(p, ctx->iv_gen_rand))
                return 0;
        }
    }
    return 1;
}

int ossl_gcm_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;
    const OSSL_PARAM *p;
    size_t sz;
    void *vp;
    int type;

    if (ossl_param_is_empty(params))
        return 1;

    for (p = params; p->key != NULL; p++) {
        type = ossl_param_find_pidx(p->key);
        switch (type) {
        default:
            break;

        case PIDX_CIPHER_PARAM_AEAD_TAG:
            vp = ctx->buf;
            if (!OSSL_PARAM_get_octet_string(p, &vp, EVP_GCM_TLS_TAG_LEN, &sz)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
                return 0;
            }
            if (sz == 0 || ctx->enc) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_TAG);
                return 0;
            }
            ctx->taglen = sz;
            break;

        case PIDX_CIPHER_PARAM_AEAD_IVLEN:
            if (!OSSL_PARAM_get_size_t(p, &sz)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
                return 0;
            }
            if (sz == 0 || sz > sizeof(ctx->iv)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
                return 0;
            }
            if (ctx->ivlen != sz) {
                /* If the iv was already set or autogenerated, it is invalid. */
                if (ctx->iv_state != IV_STATE_UNINITIALISED)
                    ctx->iv_state = IV_STATE_FINISHED;
                ctx->ivlen = sz;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TLS1_AAD:
            if (p->data_type != OSSL_PARAM_OCTET_STRING) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
                return 0;
            }
            sz = gcm_tls_init(ctx, p->data, p->data_size);
            if (sz == 0) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_AAD);
                return 0;
            }
            ctx->tls_aad_pad_sz = sz;
            break;

        case PIDX_CIPHER_PARAM_AEAD_TLS1_IV_FIXED:
            if (p->data_type != OSSL_PARAM_OCTET_STRING) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
                return 0;
            }
            if (gcm_tls_iv_set_fixed(ctx, p->data, p->data_size) == 0) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
                return 0;
            }
            break;

        case PIDX_CIPHER_PARAM_AEAD_TLS1_SET_IV_INV:
            if (p->data == NULL
                || p->data_type != OSSL_PARAM_OCTET_STRING
                || !setivinv(ctx, p->data, p->data_size))
                return 0;
            break;
        }
    }

    return 1;
}

int ossl_gcm_stream_update(void *vctx, unsigned char *out, size_t *outl,
                           size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;

    if (inl == 0) {
        *outl = 0;
        return 1;
    }

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (gcm_cipher_internal(ctx, out, outl, in, inl) <= 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_CIPHER_OPERATION_FAILED);
        return 0;
    }
    return 1;
}

int ossl_gcm_stream_final(void *vctx, unsigned char *out, size_t *outl,
                          size_t outsize)
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;
    int i;

    if (!ossl_prov_is_running())
        return 0;

    i = gcm_cipher_internal(ctx, out, outl, NULL, 0);
    if (i <= 0)
        return 0;

    *outl = 0;
    return 1;
}

int ossl_gcm_cipher(void *vctx,
                    unsigned char *out, size_t *outl, size_t outsize,
                    const unsigned char *in, size_t inl)
{
    PROV_GCM_CTX *ctx = (PROV_GCM_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;

    if (outsize < inl) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (gcm_cipher_internal(ctx, out, outl, in, inl) <= 0)
        return 0;

    *outl = inl;
    return 1;
}

/*
 * See SP800-38D (GCM) Section 8 "Uniqueness requirement on IVS and keys"
 *
 * See also 8.2.2 RBG-based construction.
 * Random construction consists of a free field (which can be NULL) and a
 * random field which will use a DRBG that can return at least 96 bits of
 * entropy strength. (The DRBG must be seeded by the FIPS module).
 */
static int gcm_iv_generate(PROV_GCM_CTX *ctx, int offset)
{
    int sz = ctx->ivlen - offset;

    /* Must be at least 96 bits */
    if (sz <= 0 || ctx->ivlen < GCM_IV_DEFAULT_SIZE)
        return 0;

    /* Use DRBG to generate random iv */
    if (RAND_bytes_ex(ctx->libctx, ctx->iv + offset, sz, 0) <= 0)
        return 0;
    ctx->iv_state = IV_STATE_BUFFERED;
    ctx->iv_gen_rand = 1;
    return 1;
}

static int gcm_cipher_internal(PROV_GCM_CTX *ctx, unsigned char *out,
                               size_t *padlen, const unsigned char *in,
                               size_t len)
{
    size_t olen = 0;
    int rv = 0;
    const PROV_GCM_HW *hw = ctx->hw;

    if (ctx->tls_aad_len != UNINITIALISED_SIZET)
        return gcm_tls_cipher(ctx, out, padlen, in, len);

    if (!ctx->key_set || ctx->iv_state == IV_STATE_FINISHED)
        goto err;

    /*
     * FIPS requires generation of AES-GCM IV's inside the FIPS module.
     * The IV can still be set externally (the security policy will state that
     * this is not FIPS compliant). There are some applications
     * where setting the IV externally is the only option available.
     */
    if (ctx->iv_state == IV_STATE_UNINITIALISED) {
        if (!ctx->enc || !gcm_iv_generate(ctx, 0))
            goto err;
    }

    if (ctx->iv_state == IV_STATE_BUFFERED) {
        if (!hw->setiv(ctx, ctx->iv, ctx->ivlen))
            goto err;
        ctx->iv_state = IV_STATE_COPIED;
    }

    if (in != NULL) {
        /*  The input is AAD if out is NULL */
        if (out == NULL) {
            if (!hw->aadupdate(ctx, in, len))
                goto err;
        } else {
            /* The input is ciphertext OR plaintext */
            if (!hw->cipherupdate(ctx, in, len, out))
                goto err;
        }
    } else {
        /* The tag must be set before actually decrypting data */
        if (!ctx->enc && ctx->taglen == UNINITIALISED_SIZET)
            goto err;
        if (!hw->cipherfinal(ctx, ctx->buf))
            goto err;
        ctx->iv_state = IV_STATE_FINISHED; /* Don't reuse the IV */
        goto finish;
    }
    olen = len;
finish:
    rv = 1;
err:
    *padlen = olen;
    return rv;
}

static int gcm_tls_init(PROV_GCM_CTX *dat, unsigned char *aad, size_t aad_len)
{
    unsigned char *buf;
    size_t len;

    if (!ossl_prov_is_running() || aad_len != EVP_AEAD_TLS1_AAD_LEN)
       return 0;

    /* Save the aad for later use. */
    buf = dat->buf;
    memcpy(buf, aad, aad_len);
    dat->tls_aad_len = aad_len;

    len = buf[aad_len - 2] << 8 | buf[aad_len - 1];
    /* Correct length for explicit iv. */
    if (len < EVP_GCM_TLS_EXPLICIT_IV_LEN)
        return 0;
    len -= EVP_GCM_TLS_EXPLICIT_IV_LEN;

    /* If decrypting correct for tag too. */
    if (!dat->enc) {
        if (len < EVP_GCM_TLS_TAG_LEN)
            return 0;
        len -= EVP_GCM_TLS_TAG_LEN;
    }
    buf[aad_len - 2] = (unsigned char)(len >> 8);
    buf[aad_len - 1] = (unsigned char)(len & 0xff);
    /* Extra padding: tag appended to record. */
    return EVP_GCM_TLS_TAG_LEN;
}

static int gcm_tls_iv_set_fixed(PROV_GCM_CTX *ctx, unsigned char *iv,
                                size_t len)
{
    /* Special case: -1 length restores whole IV */
    if (len == (size_t)-1) {
        memcpy(ctx->iv, iv, ctx->ivlen);
        ctx->iv_gen = 1;
        ctx->iv_state = IV_STATE_BUFFERED;
        return 1;
    }
    /* Fixed field must be at least 4 bytes and invocation field at least 8 */
    if ((len < EVP_GCM_TLS_FIXED_IV_LEN)
        || (ctx->ivlen - (int)len) < EVP_GCM_TLS_EXPLICIT_IV_LEN)
            return 0;
    if (len > 0)
        memcpy(ctx->iv, iv, len);
    if (ctx->enc) {
        if (RAND_bytes_ex(ctx->libctx, ctx->iv + len, ctx->ivlen - len, 0) <= 0)
            return 0;
        ctx->iv_gen_rand = 1;
    }
    ctx->iv_gen = 1;
    ctx->iv_state = IV_STATE_BUFFERED;
    return 1;
}

/*
 * Handle TLS GCM packet format. This consists of the last portion of the IV
 * followed by the payload and finally the tag. On encrypt generate IV,
 * encrypt payload and write the tag. On verify retrieve IV, decrypt payload
 * and verify tag.
 */
static int gcm_tls_cipher(PROV_GCM_CTX *ctx, unsigned char *out, size_t *padlen,
                          const unsigned char *in, size_t len)
{
    int rv = 0;
    size_t arg = EVP_GCM_TLS_EXPLICIT_IV_LEN;
    size_t plen = 0;
    unsigned char *tag = NULL;

    if (!ossl_prov_is_running() || !ctx->key_set)
        goto err;

    /* Encrypt/decrypt must be performed in place */
    if (out != in || len < (EVP_GCM_TLS_EXPLICIT_IV_LEN + EVP_GCM_TLS_TAG_LEN))
        goto err;

    /*
     * Check for too many keys as per FIPS 140-2 IG A.5 "Key/IV Pair Uniqueness
     * Requirements from SP 800-38D".  The requirements is for one party to the
     * communication to fail after 2^64 - 1 keys.  We do this on the encrypting
     * side only.
     */
    if (ctx->enc && ++ctx->tls_enc_records == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_TOO_MANY_RECORDS);
        goto err;
    }

    /*
     * Set IV from start of buffer or generate IV and write to start of
     * buffer.
     */
    if (ctx->enc) {
        if (!getivgen(ctx, out, arg))
            goto err;
    } else {
        if (!setivinv(ctx, out, arg))
            goto err;
    }

    /* Fix buffer and length to point to payload */
    in += EVP_GCM_TLS_EXPLICIT_IV_LEN;
    out += EVP_GCM_TLS_EXPLICIT_IV_LEN;
    len -= EVP_GCM_TLS_EXPLICIT_IV_LEN + EVP_GCM_TLS_TAG_LEN;

    tag = ctx->enc ? out + len : (unsigned char *)in + len;
    if (!ctx->hw->oneshot(ctx, ctx->buf, ctx->tls_aad_len, in, len, out, tag,
                          EVP_GCM_TLS_TAG_LEN)) {
        if (!ctx->enc)
            OPENSSL_cleanse(out, len);
        goto err;
    }
    if (ctx->enc)
        plen =  len + EVP_GCM_TLS_EXPLICIT_IV_LEN + EVP_GCM_TLS_TAG_LEN;
    else
        plen = len;

    rv = 1;
err:
    ctx->iv_state = IV_STATE_FINISHED;
    ctx->tls_aad_len = UNINITIALISED_SIZET;
    *padlen = plen;
    return rv;
}
