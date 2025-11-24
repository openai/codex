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
#include <openssl/param_build.h>
#include <openssl/self_test.h>
#include "crypto/slh_dsa.h"
#include "internal/fips.h"
#include "internal/param_build_set.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"

static OSSL_FUNC_keymgmt_free_fn slh_dsa_free_key;
static OSSL_FUNC_keymgmt_has_fn slh_dsa_has;
static OSSL_FUNC_keymgmt_match_fn slh_dsa_match;
static OSSL_FUNC_keymgmt_import_fn slh_dsa_import;
static OSSL_FUNC_keymgmt_export_fn slh_dsa_export;
static OSSL_FUNC_keymgmt_import_types_fn slh_dsa_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn slh_dsa_imexport_types;
static OSSL_FUNC_keymgmt_load_fn slh_dsa_load;
static OSSL_FUNC_keymgmt_get_params_fn slh_dsa_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn slh_dsa_gettable_params;
static OSSL_FUNC_keymgmt_validate_fn slh_dsa_validate;
static OSSL_FUNC_keymgmt_gen_init_fn slh_dsa_gen_init;
static OSSL_FUNC_keymgmt_gen_cleanup_fn slh_dsa_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn slh_dsa_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn slh_dsa_gen_settable_params;
static OSSL_FUNC_keymgmt_dup_fn slh_dsa_dup_key;

#define SLH_DSA_POSSIBLE_SELECTIONS (OSSL_KEYMGMT_SELECT_KEYPAIR)

struct slh_dsa_gen_ctx {
    SLH_DSA_HASH_CTX *ctx;
    OSSL_LIB_CTX *libctx;
    char *propq;
    uint8_t entropy[SLH_DSA_MAX_N * 3];
    size_t entropy_len;
};

static void *slh_dsa_new_key(void *provctx, const char *alg)
{
    if (!ossl_prov_is_running())
        return 0;

    return ossl_slh_dsa_key_new(PROV_LIBCTX_OF(provctx), NULL, alg);
}

static void slh_dsa_free_key(void *keydata)
{
    ossl_slh_dsa_key_free((SLH_DSA_KEY *)keydata);
}

static void *slh_dsa_dup_key(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_slh_dsa_key_dup(keydata_from, selection);
    return NULL;
}

static int slh_dsa_has(const void *keydata, int selection)
{
    const SLH_DSA_KEY *key = keydata;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;
    if ((selection & SLH_DSA_POSSIBLE_SELECTIONS) == 0)
        return 1; /* the selection is not missing */

    return ossl_slh_dsa_key_has(key, selection);
}

static int slh_dsa_match(const void *keydata1, const void *keydata2, int selection)
{
    const SLH_DSA_KEY *key1 = keydata1;
    const SLH_DSA_KEY *key2 = keydata2;

    if (!ossl_prov_is_running())
        return 0;
    if (key1 == NULL || key2 == NULL)
        return 0;
    return ossl_slh_dsa_key_equal(key1, key2, selection);
}

static int slh_dsa_validate(const void *key_data, int selection, int check_type)
{
    const SLH_DSA_KEY *key = key_data;

    if (!slh_dsa_has(key, selection))
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == OSSL_KEYMGMT_SELECT_KEYPAIR)
        return ossl_slh_dsa_key_pairwise_check(key);
    return 1;
}

static int slh_dsa_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    SLH_DSA_KEY *key = keydata;
    int include_priv;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & SLH_DSA_POSSIBLE_SELECTIONS) == 0)
        return 0;

    include_priv = ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0);
    return ossl_slh_dsa_key_fromdata(key, params, include_priv);
}

#define SLH_DSA_IMEXPORTABLE_PARAMETERS \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0), \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)

static const OSSL_PARAM slh_dsa_key_types[] = {
    SLH_DSA_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM *slh_dsa_imexport_types(int selection)
{
    if ((selection & SLH_DSA_POSSIBLE_SELECTIONS) == 0)
        return NULL;
    return slh_dsa_key_types;
}

static const OSSL_PARAM slh_dsa_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_MANDATORY_DIGEST, NULL, 0),
    SLH_DSA_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM *slh_dsa_gettable_params(void *provctx)
{
    return slh_dsa_params;
}

static int key_to_params(SLH_DSA_KEY *key, OSSL_PARAM_BLD *tmpl,
                         int selection)
{
    /* Error if there is no key or public key */
    if (key == NULL || ossl_slh_dsa_key_get_pub(key) == NULL)
        return 0;

    if (((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        && ossl_slh_dsa_key_get_priv(key) != NULL)
        if (ossl_param_build_set_octet_string(tmpl, NULL,
                                              OSSL_PKEY_PARAM_PRIV_KEY,
                                              ossl_slh_dsa_key_get_priv(key),
                                              ossl_slh_dsa_key_get_priv_len(key)) != 1)
            return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) == 0)
        return 1;

    return ossl_param_build_set_octet_string(tmpl, NULL,
                                             OSSL_PKEY_PARAM_PUB_KEY,
                                             ossl_slh_dsa_key_get_pub(key),
                                             ossl_slh_dsa_key_get_pub_len(key));
}

static int slh_dsa_get_params(void *keydata, OSSL_PARAM params[])
{
    SLH_DSA_KEY *key = keydata;
    OSSL_PARAM *p;
    const uint8_t *pub, *priv;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
            && !OSSL_PARAM_set_int(p, 8 * ossl_slh_dsa_key_get_pub_len(key)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
            && !OSSL_PARAM_set_int(p, 8 * ossl_slh_dsa_key_get_n(key)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
            && !OSSL_PARAM_set_int(p, ossl_slh_dsa_key_get_sig_len(key)))
        return 0;

    priv = ossl_slh_dsa_key_get_priv(key);
    if (priv != NULL) {
        p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PRIV_KEY);
        /* Note: ossl_slh_dsa_key_get_priv_len() includes the public key */
        if (p != NULL
            && !OSSL_PARAM_set_octet_string(p, priv,
                                            ossl_slh_dsa_key_get_priv_len(key)))
            return 0;
    }
    pub = ossl_slh_dsa_key_get_pub(key);
    if (pub != NULL) {
        p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PUB_KEY);
        if (p != NULL
            && !OSSL_PARAM_set_octet_string(p, pub,
                                            ossl_slh_dsa_key_get_pub_len(key)))
            return 0;
    }
    /*
     * This allows apps to use an empty digest, so that the old API
     * for digest signing can be used.
     */
    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MANDATORY_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, ""))
        return 0;
    return 1;
}

static int slh_dsa_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
                          void *cbarg)
{
    SLH_DSA_KEY *key = keydata;
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ret = 0;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if (!key_to_params(key, tmpl, selection))
        goto err;

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return ret;
}

static void *slh_dsa_load(const void *reference, size_t reference_sz)
{
    SLH_DSA_KEY *key = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(key)) {
        /* The contents of the reference is the address to our object */
        key = *(SLH_DSA_KEY **)reference;
        /* We grabbed, so we detach it */
        *(SLH_DSA_KEY **)reference = NULL;
        return key;
    }
    return NULL;
}

static void *slh_dsa_gen_init(void *provctx, int selection,
                              const OSSL_PARAM params[])
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct slh_dsa_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->libctx = libctx;
        if (!slh_dsa_gen_set_params(gctx, params)) {
            OPENSSL_free(gctx);
            gctx = NULL;
        }
    }
    return gctx;
}

#ifdef FIPS_MODULE
/*
 * Refer to FIPS 140-3 IG 10.3.A Additional Comment 1
 * Perform a pairwise test for SLH_DSA by signing and verifying a signature.
 */
static int slh_dsa_fips140_pairwise_test(SLH_DSA_HASH_CTX *ctx,
                                         const SLH_DSA_KEY *key,
                                         OSSL_LIB_CTX *lib_ctx)
{
    int ret = 0;
    OSSL_SELF_TEST *st = NULL;
    OSSL_CALLBACK *cb = NULL;
    void *cb_arg = NULL;
    uint8_t msg[16] = {0};
    size_t msg_len = sizeof(msg);
    uint8_t *sig = NULL;
    size_t sig_len;

    /* During self test, it is a waste to do this test */
    if (ossl_fips_self_testing())
        return 1;

    OSSL_SELF_TEST_get_callback(lib_ctx, &cb, &cb_arg);
    st = OSSL_SELF_TEST_new(cb, cb_arg);
    if (st == NULL)
        return 0;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_SLH_DSA);

    sig_len = ossl_slh_dsa_key_get_sig_len(key);
    sig = OPENSSL_malloc(sig_len);
    if (sig == NULL)
        goto err;

    if (ossl_slh_dsa_sign(ctx, msg, msg_len, NULL, 0, NULL, 0,
                          sig, &sig_len, sig_len) != 1)
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(st, sig);

    if (ossl_slh_dsa_verify(ctx, msg, msg_len, NULL, 0, 0, sig, sig_len) != 1)
        goto err;

    ret = 1;
err:
    OPENSSL_free(sig);
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    return ret;
}
#endif /* FIPS_MODULE */

static void *slh_dsa_gen(void *genctx, const char *alg)
{
    struct slh_dsa_gen_ctx *gctx = genctx;
    SLH_DSA_KEY *key = NULL;
    SLH_DSA_HASH_CTX *ctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;
    key = ossl_slh_dsa_key_new(gctx->libctx, gctx->propq, alg);
    if (key == NULL)
        return NULL;
    ctx = ossl_slh_dsa_hash_ctx_new(key);
    if (ctx == NULL)
        return NULL;
    if (!ossl_slh_dsa_generate_key(ctx, key, gctx->libctx,
                                   gctx->entropy, gctx->entropy_len))
        goto err;
#ifdef FIPS_MODULE
    if (!slh_dsa_fips140_pairwise_test(ctx, key, gctx->libctx)) {
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
        goto err;
    }
#endif /* FIPS_MODULE */
    ossl_slh_dsa_hash_ctx_free(ctx);
    return key;
 err:
    ossl_slh_dsa_hash_ctx_free(ctx);
    ossl_slh_dsa_key_free(key);
    return NULL;
}

static int slh_dsa_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct slh_dsa_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_SLH_DSA_SEED);
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
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->propq);
        gctx->propq = OPENSSL_strdup(p->data);
        if (gctx->propq == NULL)
            return 0;
    }
    return 1;
}

static const OSSL_PARAM *slh_dsa_gen_settable_params(ossl_unused void *genctx,
                                                     ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_SLH_DSA_SEED, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static void slh_dsa_gen_cleanup(void *genctx)
{
    struct slh_dsa_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_cleanse(gctx->entropy, gctx->entropy_len);
    OPENSSL_free(gctx->propq);
    OPENSSL_free(gctx);
}

#define MAKE_KEYMGMT_FUNCTIONS(alg, fn)                                        \
    static OSSL_FUNC_keymgmt_new_fn slh_dsa_##fn##_new_key;                    \
    static OSSL_FUNC_keymgmt_gen_fn slh_dsa_##fn##_gen;                        \
    static void *slh_dsa_##fn##_new_key(void *provctx)                         \
    {                                                                          \
        return slh_dsa_new_key(provctx, alg);                                  \
    }                                                                          \
    static void *slh_dsa_##fn##_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)\
    {                                                                          \
        return slh_dsa_gen(genctx, alg);                                       \
    }                                                                          \
    const OSSL_DISPATCH ossl_slh_dsa_##fn##_keymgmt_functions[] = {            \
        { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))slh_dsa_##fn##_new_key },     \
        { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))slh_dsa_free_key },          \
        { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))slh_dsa_dup_key },            \
        { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))slh_dsa_has },                \
        { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))slh_dsa_match },            \
        { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))slh_dsa_import },          \
        { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))slh_dsa_imexport_types },\
        { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))slh_dsa_export },          \
        { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))slh_dsa_imexport_types },\
        { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))slh_dsa_load },              \
        { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))slh_dsa_get_params }, \
        { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))slh_dsa_gettable_params },\
        { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))slh_dsa_validate },      \
        { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))slh_dsa_gen_init },      \
        { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))slh_dsa_##fn##_gen },         \
        { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))slh_dsa_gen_cleanup },\
        { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS,                                    \
          (void (*)(void))slh_dsa_gen_set_params },                            \
        { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,                               \
          (void (*)(void))slh_dsa_gen_settable_params },                       \
        OSSL_DISPATCH_END                                                      \
    }

MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-128s", sha2_128s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-128f", sha2_128f);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-192s", sha2_192s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-192f", sha2_192f);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-256s", sha2_256s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHA2-256f", sha2_256f);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-128s", shake_128s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-128f", shake_128f);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-192s", shake_192s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-192f", shake_192f);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-256s", shake_256s);
MAKE_KEYMGMT_FUNCTIONS("SLH-DSA-SHAKE-256f", shake_256f);
