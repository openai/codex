/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <assert.h>
#include <string.h> /* memset */
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/der_ml_dsa.h"
#include "crypto/ml_dsa.h"
#include "internal/packet.h"
#include "internal/sizes.h"

#define ML_DSA_MESSAGE_ENCODE_RAW  0
#define ML_DSA_MESSAGE_ENCODE_PURE 1

static OSSL_FUNC_signature_sign_message_init_fn ml_dsa_sign_msg_init;
static OSSL_FUNC_signature_sign_fn ml_dsa_sign;
static OSSL_FUNC_signature_verify_message_init_fn ml_dsa_verify_msg_init;
static OSSL_FUNC_signature_verify_fn ml_dsa_verify;
static OSSL_FUNC_signature_digest_sign_init_fn ml_dsa_digest_signverify_init;
static OSSL_FUNC_signature_digest_sign_fn ml_dsa_digest_sign;
static OSSL_FUNC_signature_digest_verify_fn ml_dsa_digest_verify;
static OSSL_FUNC_signature_freectx_fn ml_dsa_freectx;
static OSSL_FUNC_signature_set_ctx_params_fn ml_dsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn ml_dsa_settable_ctx_params;
static OSSL_FUNC_signature_get_ctx_params_fn ml_dsa_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn ml_dsa_gettable_ctx_params;
static OSSL_FUNC_signature_dupctx_fn ml_dsa_dupctx;

typedef struct {
    ML_DSA_KEY *key;
    OSSL_LIB_CTX *libctx;
    uint8_t context_string[ML_DSA_MAX_CONTEXT_STRING_LEN];
    size_t context_string_len;
    uint8_t test_entropy[ML_DSA_ENTROPY_LEN];
    size_t test_entropy_len;
    int msg_encode;
    int deterministic;
    int evp_type;
    /* The Algorithm Identifier of the signature algorithm */
    uint8_t aid_buf[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t  aid_len;
    int mu;     /* Flag indicating we should begin from \mu, not the message */
} PROV_ML_DSA_CTX;

static void ml_dsa_freectx(void *vctx)
{
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;

    OPENSSL_cleanse(ctx->test_entropy, ctx->test_entropy_len);
    OPENSSL_free(ctx);
}

static void *ml_dsa_newctx(void *provctx, int evp_type, const char *propq)
{
    PROV_ML_DSA_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(PROV_ML_DSA_CTX));
    if (ctx == NULL)
        return NULL;

    ctx->libctx = PROV_LIBCTX_OF(provctx);
    ctx->msg_encode = ML_DSA_MESSAGE_ENCODE_PURE;
    ctx->evp_type = evp_type;
    return ctx;
}

static void *ml_dsa_dupctx(void *vctx)
{
    PROV_ML_DSA_CTX *srcctx = (PROV_ML_DSA_CTX *)vctx;

    if (!ossl_prov_is_running())
        return NULL;

    /*
     * Note that the ML_DSA_KEY is ref counted via EVP_PKEY so we can just copy
     * the key here.
     */
    return OPENSSL_memdup(srcctx, sizeof(*srcctx));
}

static int set_alg_id_buffer(PROV_ML_DSA_CTX *ctx)
{
    int ret;
    WPACKET pkt;
    uint8_t *aid = NULL;

    /*
     * We do not care about DER writing errors.
     * All it really means is that for some reason, there's no
     * AlgorithmIdentifier to be had, but the operation itself is
     * still valid, just as long as it's not used to construct
     * anything that needs an AlgorithmIdentifier.
     */
    ctx->aid_len = 0;
    ret = WPACKET_init_der(&pkt, ctx->aid_buf, sizeof(ctx->aid_buf));
    ret = ret && ossl_DER_w_algorithmIdentifier_ML_DSA(&pkt, -1, ctx->key);
    if (ret && WPACKET_finish(&pkt)) {
        WPACKET_get_total_written(&pkt, &ctx->aid_len);
        aid = WPACKET_get_curr(&pkt);
    }
    WPACKET_cleanup(&pkt);
    if (aid != NULL && ctx->aid_len != 0)
        memmove(ctx->aid_buf, aid, ctx->aid_len);
    return 1;
}

static int ml_dsa_signverify_msg_init(void *vctx, void *vkey,
                                      const OSSL_PARAM params[], int operation,
                                      const char *desc)
{
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;
    ML_DSA_KEY *key = vkey;

    if (!ossl_prov_is_running()
            || ctx == NULL)
        return 0;

    if (vkey == NULL && ctx->key == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (key != NULL)
        ctx->key = vkey;
    if (!ossl_ml_dsa_key_matches(ctx->key, ctx->evp_type))
        return 0;

    set_alg_id_buffer(ctx);
    ctx->mu = 0;

    return ml_dsa_set_ctx_params(ctx, params);
}

static int ml_dsa_sign_msg_init(void *vctx, void *vkey, const OSSL_PARAM params[])
{
    return ml_dsa_signverify_msg_init(vctx, vkey, params,
                                      EVP_PKEY_OP_SIGN, "ML_DSA Sign Init");
}

static int ml_dsa_digest_signverify_init(void *vctx, const char *mdname,
                                         void *vkey, const OSSL_PARAM params[])
{
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;

    if (mdname != NULL && mdname[0] != '\0') {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "Explicit digest not supported for ML-DSA operations");
        return 0;
    }

    ctx->mu = 0;

    if (vkey == NULL && ctx->key != NULL)
        return ml_dsa_set_ctx_params(ctx, params);

    return ml_dsa_signverify_msg_init(vctx, vkey, params,
                                      EVP_PKEY_OP_SIGN, "ML_DSA Sign Init");
}

static int ml_dsa_sign(void *vctx, uint8_t *sig, size_t *siglen, size_t sigsize,
                       const uint8_t *msg, size_t msg_len)
{
    int ret = 0;
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;
    uint8_t rand_tmp[ML_DSA_ENTROPY_LEN], *rnd = NULL;

    if (!ossl_prov_is_running())
        return 0;

    if (sig != NULL) {
        if (ctx->test_entropy_len != 0) {
            rnd = ctx->test_entropy;
        } else {
            rnd = rand_tmp;

            if (ctx->deterministic == 1)
                memset(rnd, 0, sizeof(rand_tmp));
            else if (RAND_priv_bytes_ex(ctx->libctx, rnd, sizeof(rand_tmp), 0) <= 0)
                return 0;
        }
    }
    ret = ossl_ml_dsa_sign(ctx->key, ctx->mu, msg, msg_len,
                           ctx->context_string, ctx->context_string_len,
                           rnd, sizeof(rand_tmp), ctx->msg_encode,
                           sig, siglen, sigsize);
    if (rnd != ctx->test_entropy)
        OPENSSL_cleanse(rand_tmp, sizeof(rand_tmp));
    return ret;
}

static int ml_dsa_digest_sign(void *vctx, uint8_t *sig, size_t *siglen, size_t sigsize,
                              const uint8_t *tbs, size_t tbslen)
{
    return ml_dsa_sign(vctx, sig, siglen, sigsize, tbs, tbslen);
}

static int ml_dsa_verify_msg_init(void *vctx, void *vkey, const OSSL_PARAM params[])
{
    return ml_dsa_signverify_msg_init(vctx, vkey, params, EVP_PKEY_OP_VERIFY,
                                      "ML_DSA Verify Init");
}

static int ml_dsa_verify(void *vctx, const uint8_t *sig, size_t siglen,
                         const uint8_t *msg, size_t msg_len)
{
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;
    return ossl_ml_dsa_verify(ctx->key, ctx->mu, msg, msg_len,
                              ctx->context_string, ctx->context_string_len,
                              ctx->msg_encode, sig, siglen);
}
static int ml_dsa_digest_verify(void *vctx,
                                const uint8_t *sig, size_t siglen,
                                const uint8_t *tbs, size_t tbslen)
{
    return ml_dsa_verify(vctx, sig, siglen, tbs, tbslen);
}

static int ml_dsa_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ML_DSA_CTX *pctx = (PROV_ML_DSA_CTX *)vctx;
    const OSSL_PARAM *p;

    if (pctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_CONTEXT_STRING);
    if (p != NULL) {
        void *vp = pctx->context_string;

        if (!OSSL_PARAM_get_octet_string(p, &vp, sizeof(pctx->context_string),
                                         &(pctx->context_string_len))) {
            pctx->context_string_len = 0;
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_TEST_ENTROPY);
    if (p != NULL) {
        void *vp = pctx->test_entropy;

        pctx->test_entropy_len = 0;
        if (!OSSL_PARAM_get_octet_string(p, &vp, sizeof(pctx->test_entropy),
                                         &(pctx->test_entropy_len)))
                return 0;
        if (pctx->test_entropy_len != sizeof(pctx->test_entropy)) {
            pctx->test_entropy_len = 0;
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DETERMINISTIC);
    if (p != NULL && !OSSL_PARAM_get_int(p, &pctx->deterministic))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING);
    if (p != NULL && !OSSL_PARAM_get_int(p, &pctx->msg_encode))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_MU);
    if (p != NULL && !OSSL_PARAM_get_int(p, &pctx->mu))
        return 0;

    return 1;
}

static const OSSL_PARAM *ml_dsa_settable_ctx_params(void *vctx,
                                                    ossl_unused void *provctx)
{
    static const OSSL_PARAM settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_TEST_ENTROPY, NULL, 0),
        OSSL_PARAM_int(OSSL_SIGNATURE_PARAM_DETERMINISTIC, 0),
        OSSL_PARAM_int(OSSL_SIGNATURE_PARAM_MU, 0),
        OSSL_PARAM_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING, 0),
        OSSL_PARAM_END
    };

    return settable_ctx_params;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *ml_dsa_gettable_ctx_params(ossl_unused void *vctx,
                                                    ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int ml_dsa_get_ctx_params(void *vctx, OSSL_PARAM *params)
{
    PROV_ML_DSA_CTX *ctx = (PROV_ML_DSA_CTX *)vctx;
    OSSL_PARAM *p;

    if (ctx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);
    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p,
                                        ctx->aid_len == 0 ? NULL : ctx->aid_buf,
                                        ctx->aid_len))
        return 0;

    return 1;
}

#define MAKE_SIGNATURE_FUNCTIONS(alg)                                          \
    static OSSL_FUNC_signature_newctx_fn ml_dsa_##alg##_newctx;                \
    static void *ml_dsa_##alg##_newctx(void *provctx, const char *propq)       \
    {                                                                          \
        return ml_dsa_newctx(provctx, EVP_PKEY_ML_DSA_##alg, propq);           \
    }                                                                          \
    const OSSL_DISPATCH ossl_ml_dsa_##alg##_signature_functions[] = {          \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))ml_dsa_##alg##_newctx }, \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                               \
          (void (*)(void))ml_dsa_sign_msg_init },                              \
        { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))ml_dsa_sign },             \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                             \
          (void (*)(void))ml_dsa_verify_msg_init },                            \
        { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))ml_dsa_verify },         \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,                                \
          (void (*)(void))ml_dsa_digest_signverify_init },                     \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN,                                     \
          (void (*)(void))ml_dsa_digest_sign },                                \
        { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,                              \
          (void (*)(void))ml_dsa_digest_signverify_init },                     \
        { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY,                                   \
          (void (*)(void))ml_dsa_digest_verify },                              \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))ml_dsa_freectx },       \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS,                                  \
          (void (*)(void))ml_dsa_set_ctx_params },                             \
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                             \
          (void (*)(void))ml_dsa_settable_ctx_params },                        \
        { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                                  \
          (void (*)(void))ml_dsa_get_ctx_params },                             \
        { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                             \
          (void (*)(void))ml_dsa_gettable_ctx_params },                        \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))ml_dsa_dupctx },         \
        OSSL_DISPATCH_END                                                      \
    }

MAKE_SIGNATURE_FUNCTIONS(44);
MAKE_SIGNATURE_FUNCTIONS(65);
MAKE_SIGNATURE_FUNCTIONS(87);
