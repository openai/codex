/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/param_build.h>
#include <openssl/proverr.h>
#include <openssl/self_test.h>
#include "crypto/ml_dsa.h"
#include "internal/fips.h"
#include "internal/param_build_set.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/ml_dsa.h"

static OSSL_FUNC_keymgmt_free_fn ml_dsa_free_key;
static OSSL_FUNC_keymgmt_has_fn ml_dsa_has;
static OSSL_FUNC_keymgmt_match_fn ml_dsa_match;
static OSSL_FUNC_keymgmt_import_fn ml_dsa_import;
static OSSL_FUNC_keymgmt_export_fn ml_dsa_export;
static OSSL_FUNC_keymgmt_import_types_fn ml_dsa_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn ml_dsa_imexport_types;
static OSSL_FUNC_keymgmt_dup_fn ml_dsa_dup_key;
static OSSL_FUNC_keymgmt_get_params_fn ml_dsa_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn ml_dsa_gettable_params;
static OSSL_FUNC_keymgmt_validate_fn ml_dsa_validate;
static OSSL_FUNC_keymgmt_gen_init_fn ml_dsa_gen_init;
static OSSL_FUNC_keymgmt_gen_cleanup_fn ml_dsa_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn ml_dsa_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn ml_dsa_gen_settable_params;
#ifndef FIPS_MODULE
static OSSL_FUNC_keymgmt_load_fn ml_dsa_load;
#endif

struct ml_dsa_gen_ctx {
    PROV_CTX *provctx;
    char *propq;
    uint8_t entropy[32];
    size_t entropy_len;
};

#ifdef FIPS_MODULE
static int ml_dsa_pairwise_test(const ML_DSA_KEY *key)
{
    OSSL_SELF_TEST *st = NULL;
    OSSL_CALLBACK *cb = NULL;
    OSSL_LIB_CTX *ctx;
    void *cbarg = NULL;
    static const uint8_t msg[] = { 80, 108, 117, 103, 104 };
    uint8_t rnd[ML_DSA_ENTROPY_LEN];
    uint8_t sig[ML_DSA_87_SIG_LEN];
    size_t sig_len = 0;
    int ret = 0;

    if (!ml_dsa_has(key, OSSL_KEYMGMT_SELECT_KEYPAIR)
            || ossl_fips_self_testing())
        return 1;

    /*
     * The functions `OSSL_SELF_TEST_*` will return directly if parameter `st`
     * is NULL.
     */
    ctx = ossl_ml_dsa_key_get0_libctx(key);
    OSSL_SELF_TEST_get_callback(ctx, &cb, &cbarg);

    if ((st = OSSL_SELF_TEST_new(cb, cbarg)) == NULL)
        return 0;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_ML_DSA);

    memset(rnd, 0, sizeof(rnd));
    memset(sig, 0, sizeof(sig));

    if (ossl_ml_dsa_sign(key, 0, msg, sizeof(msg), NULL, 0, rnd, sizeof(rnd), 0,
                         sig, &sig_len, sizeof(sig)) <= 0)
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(st, sig);

    if (ossl_ml_dsa_verify(key, 0, msg, sizeof(msg), NULL, 0, 0,
                           sig, sig_len) <= 0)
        goto err;

    ret = 1;
 err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    return ret;
}
#endif

ML_DSA_KEY *ossl_prov_ml_dsa_new(PROV_CTX *ctx, const char *propq, int evp_type)
{
    ML_DSA_KEY *key;

    if (!ossl_prov_is_running())
        return 0;

    key = ossl_ml_dsa_key_new(PROV_LIBCTX_OF(ctx), propq, evp_type);
    /*
     * When decoding, if the key ends up "loaded" into the same provider, these
     * are the correct config settings, otherwise, new values will be assigned
     * on import into a different provider.  The "load" API does not pass along
     * the provider context.
     */
    if (key != NULL) {
        int flags_set = 0, flags_clr = 0;

        if (ossl_prov_ctx_get_bool_param(
                ctx, OSSL_PKEY_PARAM_ML_DSA_RETAIN_SEED, 1))
            flags_set |= ML_DSA_KEY_RETAIN_SEED;
        else
            flags_clr = ML_DSA_KEY_RETAIN_SEED;

        if (ossl_prov_ctx_get_bool_param(
                ctx, OSSL_PKEY_PARAM_ML_DSA_PREFER_SEED, 1))
            flags_set |= ML_DSA_KEY_PREFER_SEED;
        else
            flags_clr |= ML_DSA_KEY_PREFER_SEED;

        ossl_ml_dsa_set_prekey(key, flags_set, flags_clr, NULL, 0, NULL, 0);
    }
    return key;
}

static void ml_dsa_free_key(void *keydata)
{
    ossl_ml_dsa_key_free((ML_DSA_KEY *)keydata);
}

static void *ml_dsa_dup_key(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_ml_dsa_key_dup(keydata_from, selection);
    return NULL;
}

static int ml_dsa_has(const void *keydata, int selection)
{
    const ML_DSA_KEY *key = keydata;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 1; /* the selection is not missing */

    return ossl_ml_dsa_key_has(key, selection);
}

static int ml_dsa_match(const void *keydata1, const void *keydata2, int selection)
{
    const ML_DSA_KEY *key1 = keydata1;
    const ML_DSA_KEY *key2 = keydata2;

    if (!ossl_prov_is_running())
        return 0;
    if (key1 == NULL || key2 == NULL)
        return 0;
    return ossl_ml_dsa_key_equal(key1, key2, selection);
}

static int ml_dsa_validate(const void *key_data, int selection, int check_type)
{
    const ML_DSA_KEY *key = key_data;

    if (!ml_dsa_has(key, selection))
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == OSSL_KEYMGMT_SELECT_KEYPAIR)
        return ossl_ml_dsa_key_pairwise_check(key);
    return 1;
}

/**
 * @brief Load a ML_DSA key from raw data.
 *
 * @param key An ML_DSA key to load into
 * @param params An array of parameters containing key data.
 * @param include_private Set to 1 to optionally include the private key data
 *                        if it exists.
 * @returns 1 on success, or 0 on failure.
 */
static int ml_dsa_key_fromdata(ML_DSA_KEY *key, const OSSL_PARAM params[],
                               int include_private)
{
    const OSSL_PARAM *p = NULL;
    const ML_DSA_PARAMS *key_params = ossl_ml_dsa_key_params(key);
    const uint8_t *pk = NULL, *sk = NULL, *seed = NULL;
    size_t pk_len = 0, sk_len = 0, seed_len = 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (p != NULL
        && !OSSL_PARAM_get_octet_string_ptr(p, (const void **)&pk, &pk_len))
        return 0;
    if (pk != NULL && pk_len != key_params->pk_len) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH,
                       "Invalid %s public key length", key_params->alg);
        return 0;
    }

    /* Private key is optional */
    if (include_private) {
        p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ML_DSA_SEED);
        if (p != NULL
            && !OSSL_PARAM_get_octet_string_ptr(p, (const void **)&seed,
                                                &seed_len))
            return 0;
        if (seed != NULL && seed_len != ML_DSA_SEED_BYTES) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
            return 0;
        }
        p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
        if (p != NULL
            && !OSSL_PARAM_get_octet_string_ptr(p, (const void **)&sk, &sk_len))
            return 0;
        if (sk != NULL && sk_len != key_params->sk_len) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH,
                           "Invalid %s private key length", key_params->alg);
            return 0;
        }
    }

    /* The caller MUST specify at least one of seed, private or public keys. */
    if (seed_len == 0 && pk_len == 0 && sk_len == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    if (seed_len != 0
        && (sk_len == 0
            || (ossl_ml_dsa_key_get_prov_flags(key) & ML_DSA_KEY_PREFER_SEED))) {
        if (!ossl_ml_dsa_set_prekey(key, 0, 0, seed, seed_len, sk, sk_len))
            return 0;
        if (!ossl_ml_dsa_generate_key(key)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GENERATE_KEY);
            return 0;
        }
    } else if (sk_len > 0) {
        if (!ossl_ml_dsa_sk_decode(key, sk, sk_len))
            return 0;
    } else if (pk_len > 0) {
        if (!ossl_ml_dsa_pk_decode(key, pk, pk_len))
            return 0;
    }

    /* Error if the supplied public key does not match the generated key */
    if (pk_len == 0
        || seed_len + sk_len == 0
        || memcmp(ossl_ml_dsa_key_get_pub(key), pk, pk_len) == 0)
        return 1;
    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                   "explicit %s public key does not match private",
                   key_params->alg);
    ossl_ml_dsa_key_reset(key);
    return 0;
}

static int ml_dsa_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    ML_DSA_KEY *key = keydata;
    int include_priv;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_priv = ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0);
    return ml_dsa_key_fromdata(key, params, include_priv);
}

#define ML_DSA_IMEXPORTABLE_PARAMETERS \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ML_DSA_SEED, NULL, 0), \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0), \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)

static const OSSL_PARAM ml_dsa_key_types[] = {
    ML_DSA_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM *ml_dsa_imexport_types(int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return NULL;
    return ml_dsa_key_types;
}

static const OSSL_PARAM ml_dsa_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_MANDATORY_DIGEST, NULL, 0),
    ML_DSA_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM *ml_dsa_gettable_params(void *provctx)
{
    return ml_dsa_params;
}

static int ml_dsa_get_params(void *keydata, OSSL_PARAM params[])
{
    ML_DSA_KEY *key = keydata;
    OSSL_PARAM *p;
    const uint8_t *pub, *priv, *seed;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
            && !OSSL_PARAM_set_int(p, 8 * ossl_ml_dsa_key_get_pub_len(key)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
            && !OSSL_PARAM_set_int(p, ossl_ml_dsa_key_get_collision_strength_bits(key)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
            && !OSSL_PARAM_set_int(p, ossl_ml_dsa_key_get_sig_len(key)))
        return 0;

    pub = ossl_ml_dsa_key_get_pub(key);
    priv = ossl_ml_dsa_key_get_priv(key);
    seed = ossl_ml_dsa_key_get_seed(key);

    if (seed != NULL
        && (p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ML_DSA_SEED)) != NULL
        && !OSSL_PARAM_set_octet_string(p, seed, ML_DSA_SEED_BYTES))
        return 0;
    if (priv != NULL
        && (p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PRIV_KEY)) != NULL
        && !OSSL_PARAM_set_octet_string(p, priv,
                                        ossl_ml_dsa_key_get_priv_len(key)))
        return 0;
    if (pub != NULL
        && (p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PUB_KEY)) != NULL
        && !OSSL_PARAM_set_octet_string(p, pub,
                                        ossl_ml_dsa_key_get_pub_len(key)))
        return 0;
    /*
     * This allows apps to use an empty digest, so that the old API
     * for digest signing can be used.
     */
    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MANDATORY_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, ""))
        return 0;

    return 1;
}

static int ml_dsa_export(void *keydata, int selection,
                         OSSL_CALLBACK *param_cb, void *cbarg)
{
    ML_DSA_KEY *key = keydata;
    OSSL_PARAM params[4];
    const uint8_t *buf;
    int include_private, pnum = 0;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_private = ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0);

    /*
     * Note that if the seed is present, both the seed and the private key are
     * exported.  The recipient will have a choice.
     */
    if (include_private) {
        if ((buf = ossl_ml_dsa_key_get_seed(key)) != NULL) {
            params[pnum++] = OSSL_PARAM_construct_octet_string
                (OSSL_PKEY_PARAM_ML_DSA_SEED, (void *)buf, ML_DSA_SEED_BYTES);
        }
        if ((buf = ossl_ml_dsa_key_get_priv(key)) != NULL) {
            params[pnum++] = OSSL_PARAM_construct_octet_string
                (OSSL_PKEY_PARAM_PRIV_KEY, (void *)buf,
                 ossl_ml_dsa_key_get_priv_len(key));
        }
    }
    if (((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        && ((buf = ossl_ml_dsa_key_get_pub(key)) != NULL)) {
        params[pnum++] = OSSL_PARAM_construct_octet_string
            (OSSL_PKEY_PARAM_PUB_KEY, (void *)buf,
             ossl_ml_dsa_key_get_pub_len(key));
    }
    if (pnum == 0)
        return 0;
    params[pnum] = OSSL_PARAM_construct_end();
    return param_cb(params, cbarg);
}

#ifndef FIPS_MODULE
static void *ml_dsa_load(const void *reference, size_t reference_sz)
{
    ML_DSA_KEY *key = NULL;
    const ML_DSA_PARAMS *key_params;
    const uint8_t *sk, *seed;

    if (ossl_prov_is_running() && reference_sz == sizeof(key)) {
        /* The contents of the reference is the address to our object */
        key = *(ML_DSA_KEY **)reference;
        /* We grabbed, so we detach it */
        *(ML_DSA_KEY **)reference = NULL;
        /* All done, if the pubkey is present. */
        if (key == NULL || ossl_ml_dsa_key_get_pub(key) != NULL)
            return key;
        /* Handle private prekey inputs. */
        sk = ossl_ml_dsa_key_get_priv(key);
        seed = ossl_ml_dsa_key_get_seed(key);
        if (seed != NULL
            && (sk == NULL || (ossl_ml_dsa_key_get_prov_flags(key)
                               & ML_DSA_KEY_PREFER_SEED))) {
            if (ossl_ml_dsa_generate_key(key))
                return key;
        } else if (sk != NULL) {
            if (ossl_ml_dsa_sk_decode(key, sk,
                                      ossl_ml_dsa_key_get_priv_len(key)))
                return key;
            key_params = ossl_ml_dsa_key_params(key);
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                           "error parsing %s private key",
                           key_params->alg);
        } else {
            return key;
        }
    }

    ossl_ml_dsa_key_free(key);
    return NULL;
}
#endif

static void *ml_dsa_gen_init(void *provctx, int selection,
                             const OSSL_PARAM params[])
{
    struct ml_dsa_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->provctx = provctx;
        if (!ml_dsa_gen_set_params(gctx, params)) {
            OPENSSL_free(gctx);
            gctx = NULL;
        }
    }
    return gctx;
}

static void *ml_dsa_gen(void *genctx, int evp_type)
{
    struct ml_dsa_gen_ctx *gctx = genctx;
    ML_DSA_KEY *key = NULL;

    if (!ossl_prov_is_running())
        return NULL;
    key = ossl_prov_ml_dsa_new(gctx->provctx, gctx->propq, evp_type);
    if (key == NULL)
        return NULL;
    if (gctx->entropy_len != 0
        && !ossl_ml_dsa_set_prekey(key, 0, 0,
                                   gctx->entropy, gctx->entropy_len, NULL, 0))
        goto err;
    if (!ossl_ml_dsa_generate_key(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GENERATE_KEY);
        goto err;
    }
#ifdef FIPS_MODULE
    if (!ml_dsa_pairwise_test(key)) {
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
        goto err;
    }
#endif
    return key;
 err:
    ossl_ml_dsa_key_free(key);
    return NULL;
}

static int ml_dsa_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct ml_dsa_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ML_DSA_SEED);
    if (p != NULL) {
        void *vp = gctx->entropy;
        size_t len = sizeof(gctx->entropy);

        if (!OSSL_PARAM_get_octet_string(p, &vp, len, &(gctx->entropy_len))) {
            gctx->entropy_len = 0;
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        OPENSSL_free(gctx->propq);
        gctx->propq = NULL;
        if (!OSSL_PARAM_get_utf8_string(p, &gctx->propq, 0))
            return 0;
    }
    return 1;
}

static const OSSL_PARAM *ml_dsa_gen_settable_params(ossl_unused void *genctx,
                                                    ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ML_DSA_SEED, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static void ml_dsa_gen_cleanup(void *genctx)
{
    struct ml_dsa_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_cleanse(gctx->entropy, gctx->entropy_len);
    OPENSSL_free(gctx->propq);
    OPENSSL_free(gctx);
}

#ifndef FIPS_MODULE
# define DISPATCH_LOAD_FN \
        { OSSL_FUNC_KEYMGMT_LOAD, (OSSL_FUNC) ml_dsa_load },
#else
# define DISPATCH_LOAD_FN   /* Non-FIPS only */
#endif

#define MAKE_KEYMGMT_FUNCTIONS(alg)                                            \
    static OSSL_FUNC_keymgmt_new_fn ml_dsa_##alg##_new_key;                    \
    static OSSL_FUNC_keymgmt_gen_fn ml_dsa_##alg##_gen;                        \
    static void *ml_dsa_##alg##_new_key(void *provctx)                         \
    {                                                                          \
        return ossl_prov_ml_dsa_new(provctx, NULL, EVP_PKEY_ML_DSA_##alg);     \
    }                                                                          \
    static void *ml_dsa_##alg##_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)\
    {                                                                          \
        return ml_dsa_gen(genctx, EVP_PKEY_ML_DSA_##alg);                      \
    }                                                                          \
    const OSSL_DISPATCH ossl_ml_dsa_##alg##_keymgmt_functions[] = {            \
        { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))ml_dsa_##alg##_new_key },     \
        { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))ml_dsa_free_key },           \
        { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))ml_dsa_has },                 \
        { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))ml_dsa_match },             \
        { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))ml_dsa_import },           \
        { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))ml_dsa_imexport_types },\
        { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))ml_dsa_export },           \
        { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))ml_dsa_imexport_types },\
        DISPATCH_LOAD_FN                                                       \
        { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))ml_dsa_get_params },  \
        { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))ml_dsa_gettable_params },\
        { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))ml_dsa_validate },       \
        { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))ml_dsa_gen_init },       \
        { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))ml_dsa_##alg##_gen },         \
        { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))ml_dsa_gen_cleanup }, \
        { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS,                                    \
          (void (*)(void))ml_dsa_gen_set_params },                             \
        { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,                               \
          (void (*)(void))ml_dsa_gen_settable_params },                        \
        { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))ml_dsa_dup_key },             \
        OSSL_DISPATCH_END                                                      \
    }

MAKE_KEYMGMT_FUNCTIONS(44);
MAKE_KEYMGMT_FUNCTIONS(65);
MAKE_KEYMGMT_FUNCTIONS(87);
