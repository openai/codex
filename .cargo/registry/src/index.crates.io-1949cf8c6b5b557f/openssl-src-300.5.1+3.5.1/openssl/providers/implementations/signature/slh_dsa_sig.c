/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/der_slh_dsa.h"
#include "crypto/slh_dsa.h"
#include "internal/sizes.h"

#define SLH_DSA_MAX_ADD_RANDOM_LEN 32

#define SLH_DSA_MESSAGE_ENCODE_RAW  0
#define SLH_DSA_MESSAGE_ENCODE_PURE 1

static OSSL_FUNC_signature_sign_message_init_fn slh_dsa_sign_msg_init;
static OSSL_FUNC_signature_sign_fn slh_dsa_sign;
static OSSL_FUNC_signature_verify_message_init_fn slh_dsa_verify_msg_init;
static OSSL_FUNC_signature_verify_fn slh_dsa_verify;
static OSSL_FUNC_signature_digest_sign_init_fn slh_dsa_digest_signverify_init;
static OSSL_FUNC_signature_digest_sign_fn slh_dsa_digest_sign;
static OSSL_FUNC_signature_digest_verify_fn slh_dsa_digest_verify;
static OSSL_FUNC_signature_freectx_fn slh_dsa_freectx;
static OSSL_FUNC_signature_dupctx_fn slh_dsa_dupctx;
static OSSL_FUNC_signature_set_ctx_params_fn slh_dsa_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn slh_dsa_settable_ctx_params;

/*
 * NOTE: Any changes to this structure may require updating slh_dsa_dupctx().
 */
typedef struct {
    SLH_DSA_KEY *key; /* Note that the key is not owned by this object */
    SLH_DSA_HASH_CTX *hash_ctx;
    uint8_t context_string[SLH_DSA_MAX_CONTEXT_STRING_LEN];
    size_t context_string_len;
    uint8_t add_random[SLH_DSA_MAX_ADD_RANDOM_LEN];
    size_t add_random_len;
    int msg_encode;
    int deterministic;
    OSSL_LIB_CTX *libctx;
    char *propq;
    const char *alg;
    /* The Algorithm Identifier of the signature algorithm */
    uint8_t aid_buf[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t  aid_len;
} PROV_SLH_DSA_CTX;

static void slh_dsa_freectx(void *vctx)
{
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;

    ossl_slh_dsa_hash_ctx_free(ctx->hash_ctx);
    OPENSSL_free(ctx->propq);
    OPENSSL_cleanse(ctx->add_random, ctx->add_random_len);
    OPENSSL_free(ctx);
}

static void *slh_dsa_newctx(void *provctx, const char *alg, const char *propq)
{
    PROV_SLH_DSA_CTX *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(PROV_SLH_DSA_CTX));
    if (ctx == NULL)
        return NULL;

    ctx->libctx = PROV_LIBCTX_OF(provctx);
    if (propq != NULL && (ctx->propq = OPENSSL_strdup(propq)) == NULL)
        goto err;
    ctx->alg = alg;
    ctx->msg_encode = SLH_DSA_MESSAGE_ENCODE_PURE;
    return ctx;
 err:
    slh_dsa_freectx(ctx);
    return NULL;
}

static void *slh_dsa_dupctx(void *vctx)
{
    PROV_SLH_DSA_CTX *src = (PROV_SLH_DSA_CTX *)vctx;
    PROV_SLH_DSA_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    /*
     * Note that the SLH_DSA_KEY is ref counted via EVP_PKEY so we can just copy
     * the key here.
     */
    ret = OPENSSL_memdup(src, sizeof(*src));
    if (ret == NULL)
        return NULL;
    ret->propq = NULL;
    ret->hash_ctx = NULL;
    if (src->propq != NULL && (ret->propq = OPENSSL_strdup(src->propq)) == NULL)
        goto err;
    ret->hash_ctx = ossl_slh_dsa_hash_ctx_dup(src->hash_ctx);
    if (ret->hash_ctx == NULL)
        goto err;

    return ret;
 err:
    slh_dsa_freectx(ret);
    return NULL;
}

static int slh_dsa_set_alg_id_buffer(PROV_SLH_DSA_CTX *ctx)
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
    ret = ret && ossl_DER_w_algorithmIdentifier_SLH_DSA(&pkt, -1, ctx->key);
    if (ret && WPACKET_finish(&pkt)) {
        WPACKET_get_total_written(&pkt, &ctx->aid_len);
        aid = WPACKET_get_curr(&pkt);
    }
    WPACKET_cleanup(&pkt);
    if (aid != NULL && ctx->aid_len != 0)
        memmove(ctx->aid_buf, aid, ctx->aid_len);
    return 1;
}

static int slh_dsa_signverify_msg_init(void *vctx, void *vkey,
                                       const OSSL_PARAM params[], int operation,
                                       const char *desc)
{
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;
    SLH_DSA_KEY *key = vkey;

    if (!ossl_prov_is_running()
            || ctx == NULL)
        return 0;

    if (vkey == NULL && ctx->key == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (key != NULL) {
        if (!ossl_slh_dsa_key_type_matches(key, ctx->alg))
            return 0;
        ctx->hash_ctx = ossl_slh_dsa_hash_ctx_new(key);
        if (ctx->hash_ctx == NULL)
            return 0;
        ctx->key = vkey;
    }

    slh_dsa_set_alg_id_buffer(ctx);
    if (!slh_dsa_set_ctx_params(ctx, params))
        return 0;
    return 1;
}

static int slh_dsa_sign_msg_init(void *vctx, void *vkey, const OSSL_PARAM params[])
{
    return slh_dsa_signverify_msg_init(vctx, vkey, params,
                                       EVP_PKEY_OP_SIGN, "SLH_DSA Sign Init");
}

static int slh_dsa_digest_signverify_init(void *vctx, const char *mdname,
                                          void *vkey, const OSSL_PARAM params[])
{
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;

    if (mdname != NULL && mdname[0] != '\0') {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_DIGEST,
                       "Explicit digest not supported for SLH-DSA operations");
        return 0;
    }

    if (vkey == NULL && ctx->key != NULL)
        return slh_dsa_set_ctx_params(ctx, params);

    return slh_dsa_signverify_msg_init(vctx, vkey, params,
                                       EVP_PKEY_OP_SIGN, "SLH_DSA Sign Init");
}

static int slh_dsa_sign(void *vctx, unsigned char *sig, size_t *siglen,
                        size_t sigsize, const unsigned char *msg, size_t msg_len)
{
    int ret = 0;
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;
    uint8_t add_rand[SLH_DSA_MAX_ADD_RANDOM_LEN], *opt_rand = NULL;
    size_t n = 0;

    if (!ossl_prov_is_running())
        return 0;

    if (sig != NULL) {
        if (ctx->add_random_len != 0) {
            opt_rand = ctx->add_random;
        } else if (ctx->deterministic == 0) {
            n = ossl_slh_dsa_key_get_n(ctx->key);
            if (RAND_priv_bytes_ex(ctx->libctx, add_rand, n, 0) <= 0)
                return 0;
            opt_rand = add_rand;
        }
    }
    ret = ossl_slh_dsa_sign(ctx->hash_ctx, msg, msg_len,
                            ctx->context_string, ctx->context_string_len,
                            opt_rand, ctx->msg_encode,
                            sig, siglen, sigsize);
    if (opt_rand != add_rand)
        OPENSSL_cleanse(opt_rand, n);
    return ret;
}

static int slh_dsa_digest_sign(void *vctx, uint8_t *sig, size_t *siglen, size_t sigsize,
                               const uint8_t *tbs, size_t tbslen)
{
    return slh_dsa_sign(vctx, sig, siglen, sigsize, tbs, tbslen);
}

static int slh_dsa_verify_msg_init(void *vctx, void *vkey, const OSSL_PARAM params[])
{
    return slh_dsa_signverify_msg_init(vctx, vkey, params, EVP_PKEY_OP_VERIFY,
                                       "SLH_DSA Verify Init");
}

static int slh_dsa_verify(void *vctx, const uint8_t *sig, size_t siglen,
                          const uint8_t *msg, size_t msg_len)
{
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;

    if (!ossl_prov_is_running())
        return 0;
    return ossl_slh_dsa_verify(ctx->hash_ctx, msg, msg_len,
                               ctx->context_string, ctx->context_string_len,
                               ctx->msg_encode, sig, siglen);
}
static int slh_dsa_digest_verify(void *vctx, const uint8_t *sig, size_t siglen,
                                 const uint8_t *tbs, size_t tbslen)
{
    return slh_dsa_verify(vctx, sig, siglen, tbs, tbslen);
}

static int slh_dsa_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_SLH_DSA_CTX *pctx = (PROV_SLH_DSA_CTX *)vctx;
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
        void *vp = pctx->add_random;
        size_t n = ossl_slh_dsa_key_get_n(pctx->key);

        if (!OSSL_PARAM_get_octet_string(p, &vp, n, &(pctx->add_random_len))
                || pctx->add_random_len != n) {
            pctx->add_random_len = 0;
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DETERMINISTIC);
    if (p != NULL && !OSSL_PARAM_get_int(p, &pctx->deterministic))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING);
    if (p != NULL && !OSSL_PARAM_get_int(p, &pctx->msg_encode))
        return 0;
    return 1;
}

static const OSSL_PARAM *slh_dsa_settable_ctx_params(void *vctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_TEST_ENTROPY, NULL, 0),
        OSSL_PARAM_int(OSSL_SIGNATURE_PARAM_DETERMINISTIC, 0),
        OSSL_PARAM_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING, 0),
        OSSL_PARAM_END
    };

    return settable_ctx_params;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *slh_dsa_gettable_ctx_params(ossl_unused void *vctx,
                                                     ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int slh_dsa_get_ctx_params(void *vctx, OSSL_PARAM *params)
{
    PROV_SLH_DSA_CTX *ctx = (PROV_SLH_DSA_CTX *)vctx;
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

#define MAKE_SIGNATURE_FUNCTIONS(alg, fn)                                      \
    static OSSL_FUNC_signature_newctx_fn slh_dsa_##fn##_newctx;                \
    static void *slh_dsa_##fn##_newctx(void *provctx, const char *propq)       \
    {                                                                          \
        return slh_dsa_newctx(provctx, alg, propq);                            \
    }                                                                          \
    const OSSL_DISPATCH ossl_slh_dsa_##fn##_signature_functions[] = {          \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))slh_dsa_##fn##_newctx }, \
        { OSSL_FUNC_SIGNATURE_SIGN_MESSAGE_INIT,                               \
          (void (*)(void))slh_dsa_sign_msg_init },                             \
        { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))slh_dsa_sign },            \
        { OSSL_FUNC_SIGNATURE_VERIFY_MESSAGE_INIT,                             \
          (void (*)(void))slh_dsa_verify_msg_init },                           \
        { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))slh_dsa_verify },        \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,                                \
          (void (*)(void))slh_dsa_digest_signverify_init },                    \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN,                                     \
          (void (*)(void))slh_dsa_digest_sign },                               \
        { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,                              \
          (void (*)(void))slh_dsa_digest_signverify_init },                    \
        { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY,                                   \
          (void (*)(void))slh_dsa_digest_verify },                             \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))slh_dsa_freectx },      \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))slh_dsa_dupctx },        \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, (void (*)(void))slh_dsa_set_ctx_params },\
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,                             \
          (void (*)(void))slh_dsa_settable_ctx_params },                       \
        { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS,                                  \
          (void (*)(void))slh_dsa_get_ctx_params },                            \
        { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,                             \
          (void (*)(void))slh_dsa_gettable_ctx_params },                       \
        OSSL_DISPATCH_END                                                      \
    }

MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-128s", sha2_128s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-128f", sha2_128f);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-192s", sha2_192s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-192f", sha2_192f);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-256s", sha2_256s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHA2-256f", sha2_256f);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-128s", shake_128s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-128f", shake_128f);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-192s", shake_192s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-192f", shake_192f);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-256s", shake_256s);
MAKE_SIGNATURE_FUNCTIONS("SLH-DSA-SHAKE-256f", shake_256f);
