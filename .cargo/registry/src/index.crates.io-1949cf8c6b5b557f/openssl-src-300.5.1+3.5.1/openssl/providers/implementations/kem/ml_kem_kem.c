/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "crypto/ml_kem.h"
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "prov/providercommon.h"

static OSSL_FUNC_kem_newctx_fn ml_kem_newctx;
static OSSL_FUNC_kem_freectx_fn ml_kem_freectx;
static OSSL_FUNC_kem_encapsulate_init_fn ml_kem_encapsulate_init;
static OSSL_FUNC_kem_encapsulate_fn ml_kem_encapsulate;
static OSSL_FUNC_kem_decapsulate_init_fn ml_kem_decapsulate_init;
static OSSL_FUNC_kem_decapsulate_fn ml_kem_decapsulate;
static OSSL_FUNC_kem_set_ctx_params_fn ml_kem_set_ctx_params;
static OSSL_FUNC_kem_settable_ctx_params_fn ml_kem_settable_ctx_params;

typedef struct {
    ML_KEM_KEY *key;
    uint8_t entropy_buf[ML_KEM_RANDOM_BYTES];
    uint8_t *entropy;
    int op;
} PROV_ML_KEM_CTX;

static void *ml_kem_newctx(void *provctx)
{
    PROV_ML_KEM_CTX *ctx;

    if ((ctx = OPENSSL_malloc(sizeof(*ctx))) == NULL)
        return NULL;

    ctx->key = NULL;
    ctx->entropy = NULL;
    ctx->op = 0;
    return ctx;
}

static void ml_kem_freectx(void *vctx)
{
    PROV_ML_KEM_CTX *ctx = vctx;

    if (ctx->entropy != NULL)
        OPENSSL_cleanse(ctx->entropy, ML_KEM_RANDOM_BYTES);
    OPENSSL_free(ctx);
}

static int ml_kem_init(void *vctx, int op, void *key,
                       const OSSL_PARAM params[])
{
    PROV_ML_KEM_CTX *ctx = vctx;

    if (!ossl_prov_is_running())
        return 0;
    ctx->key = key;
    ctx->op = op;
    return ml_kem_set_ctx_params(vctx, params);
}

static int ml_kem_encapsulate_init(void *vctx, void *vkey,
                                   const OSSL_PARAM params[])
{
    ML_KEM_KEY *key = vkey;

    if (!ossl_ml_kem_have_pubkey(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }
    return ml_kem_init(vctx, EVP_PKEY_OP_ENCAPSULATE, key, params);
}

static int ml_kem_decapsulate_init(void *vctx, void *vkey,
                                   const OSSL_PARAM params[])
{
    ML_KEM_KEY *key = vkey;

    if (!ossl_ml_kem_have_prvkey(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }
    return ml_kem_init(vctx, EVP_PKEY_OP_DECAPSULATE, key, params);
}

static int ml_kem_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ML_KEM_CTX *ctx = vctx;
    const OSSL_PARAM *p;

    if (ctx == NULL)
        return 0;

    if (ctx->op == EVP_PKEY_OP_DECAPSULATE && ctx->entropy != NULL) {
        /* Decapsulation is deterministic */
        OPENSSL_cleanse(ctx->entropy, ML_KEM_RANDOM_BYTES);
        ctx->entropy = NULL;
    }

    if (ossl_param_is_empty(params))
        return 1;

    /* Encapsulation ephemeral input key material "ikmE" */
    if (ctx->op == EVP_PKEY_OP_ENCAPSULATE
        && (p = OSSL_PARAM_locate_const(params, OSSL_KEM_PARAM_IKME)) != NULL) {
        size_t len = ML_KEM_RANDOM_BYTES;

        ctx->entropy = ctx->entropy_buf;
        if (OSSL_PARAM_get_octet_string(p, (void **)&ctx->entropy,
                                        len, &len)
            && len == ML_KEM_RANDOM_BYTES)
            return 1;

        /* Possibly, but much less likely wrong type */
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
        ctx->entropy = NULL;
        return 0;
    }

    return 1;
}

static const OSSL_PARAM *ml_kem_settable_ctx_params(ossl_unused void *vctx,
                                                    ossl_unused void *provctx)
{
    static const OSSL_PARAM params[] = {
        OSSL_PARAM_octet_string(OSSL_KEM_PARAM_IKME, NULL, 0),
        OSSL_PARAM_END
    };

    return params;
}

static int ml_kem_encapsulate(void *vctx, unsigned char *ctext, size_t *clen,
                              unsigned char *shsec, size_t *slen)
{
    PROV_ML_KEM_CTX *ctx = vctx;
    ML_KEM_KEY *key = ctx->key;
    const ML_KEM_VINFO *v;
    size_t encap_clen;
    size_t encap_slen;
    int ret = 0;

    if (!ossl_ml_kem_have_pubkey(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        goto end;
    }
    v = ossl_ml_kem_key_vinfo(key);
    encap_clen = v->ctext_bytes;
    encap_slen = ML_KEM_SHARED_SECRET_BYTES;

    if (ctext == NULL) {
        if (clen == NULL && slen == NULL)
            return 0;
        if (clen != NULL)
            *clen = encap_clen;
        if (slen != NULL)
            *slen = encap_slen;
        return 1;
    }
    if (shsec == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                       "NULL shared-secret buffer");
        goto end;
    }

    if (clen == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_NULL_LENGTH_POINTER,
                       "null ciphertext input/output length pointer");
        goto end;
    } else if (*clen < encap_clen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                       "ciphertext buffer too small");
        goto end;
    } else {
        *clen = encap_clen;
    }

    if (slen == NULL) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_NULL_LENGTH_POINTER,
                       "null shared secret input/output length pointer");
        goto end;
    } else if (*slen < encap_slen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                       "shared-secret buffer too small");
        goto end;
    } else {
        *slen = encap_slen;
    }

    if (ctx->entropy != NULL)
        ret = ossl_ml_kem_encap_seed(ctext, encap_clen, shsec, encap_slen,
                                     ctx->entropy, ML_KEM_RANDOM_BYTES, key);
    else
        ret = ossl_ml_kem_encap_rand(ctext, encap_clen, shsec, encap_slen, key);

 end:
    /*
     * One shot entropy, each encapsulate call must either provide a new
     * "ikmE", or else will use a random value.  If a caller sets an explicit
     * ikmE once for testing, and later performs multiple encapsulations
     * without again calling encapsulate_init(), these should not share the
     * original entropy.
     */
    if (ctx->entropy != NULL) {
        OPENSSL_cleanse(ctx->entropy, ML_KEM_RANDOM_BYTES);
        ctx->entropy = NULL;
    }
    return ret;
}

static int ml_kem_decapsulate(void *vctx, uint8_t *shsec, size_t *slen,
                              const uint8_t *ctext, size_t clen)
{
    PROV_ML_KEM_CTX *ctx = vctx;
    ML_KEM_KEY *key = ctx->key;
    size_t decap_slen = ML_KEM_SHARED_SECRET_BYTES;

    if (!ossl_ml_kem_have_prvkey(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    if (shsec == NULL) {
        if (slen == NULL)
            return 0;
        *slen = ML_KEM_SHARED_SECRET_BYTES;
        return 1;
    }

    /* For now tolerate newly-deprecated NULL length pointers. */
    if (slen == NULL) {
        slen = &decap_slen;
    } else if (*slen < decap_slen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                       "shared-secret buffer too small");
        return 0;
    } else {
        *slen = decap_slen;
    }

    /* ML-KEM decap handles incorrect ciphertext lengths internally */
    return ossl_ml_kem_decap(shsec, decap_slen, ctext, clen, key);
}

const OSSL_DISPATCH ossl_ml_kem_asym_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (OSSL_FUNC) ml_kem_newctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT, (OSSL_FUNC) ml_kem_encapsulate_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (OSSL_FUNC) ml_kem_encapsulate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT, (OSSL_FUNC) ml_kem_decapsulate_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (OSSL_FUNC) ml_kem_decapsulate },
    { OSSL_FUNC_KEM_FREECTX, (OSSL_FUNC) ml_kem_freectx },
    { OSSL_FUNC_KEM_SET_CTX_PARAMS, (OSSL_FUNC) ml_kem_set_ctx_params },
    { OSSL_FUNC_KEM_SETTABLE_CTX_PARAMS, (OSSL_FUNC) ml_kem_settable_ctx_params },
    OSSL_DISPATCH_END
};
