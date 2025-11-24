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
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/provider.h>
#include <openssl/rand.h>
#include <openssl/self_test.h>
#include <openssl/param_build.h>
#include "crypto/ml_kem.h"
#include "internal/fips.h"
#include "internal/param_build_set.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"
#include "prov/ml_kem.h"

static OSSL_FUNC_keymgmt_new_fn ml_kem_512_new;
static OSSL_FUNC_keymgmt_new_fn ml_kem_768_new;
static OSSL_FUNC_keymgmt_new_fn ml_kem_1024_new;
static OSSL_FUNC_keymgmt_gen_fn ml_kem_gen;
static OSSL_FUNC_keymgmt_gen_init_fn ml_kem_512_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn ml_kem_768_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn ml_kem_1024_gen_init;
static OSSL_FUNC_keymgmt_gen_cleanup_fn ml_kem_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn ml_kem_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn ml_kem_gen_settable_params;
#ifndef FIPS_MODULE
static OSSL_FUNC_keymgmt_load_fn ml_kem_load;
#endif
static OSSL_FUNC_keymgmt_get_params_fn ml_kem_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn ml_kem_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn ml_kem_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn ml_kem_settable_params;
static OSSL_FUNC_keymgmt_has_fn ml_kem_has;
static OSSL_FUNC_keymgmt_match_fn ml_kem_match;
static OSSL_FUNC_keymgmt_validate_fn ml_kem_validate;
static OSSL_FUNC_keymgmt_import_fn ml_kem_import;
static OSSL_FUNC_keymgmt_export_fn ml_kem_export;
static OSSL_FUNC_keymgmt_import_types_fn ml_kem_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn ml_kem_imexport_types;
static OSSL_FUNC_keymgmt_dup_fn ml_kem_dup;

static const int minimal_selection = OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS
    | OSSL_KEYMGMT_SELECT_PRIVATE_KEY;

typedef struct ml_kem_gen_ctx_st {
    PROV_CTX *provctx;
    char *propq;
    int selection;
    int evp_type;
    uint8_t seedbuf[ML_KEM_SEED_BYTES];
    uint8_t *seed;
} PROV_ML_KEM_GEN_CTX;

static int ml_kem_pairwise_test(const ML_KEM_KEY *key, int key_flags)
{
#ifdef FIPS_MODULE
    OSSL_SELF_TEST *st = NULL;
    OSSL_CALLBACK *cb = NULL;
    void *cbarg = NULL;
#endif
    unsigned char entropy[ML_KEM_RANDOM_BYTES];
    unsigned char secret[ML_KEM_SHARED_SECRET_BYTES];
    unsigned char out[ML_KEM_SHARED_SECRET_BYTES];
    unsigned char *ctext = NULL;
    const ML_KEM_VINFO *v = ossl_ml_kem_key_vinfo(key);
    int operation_result = 0;
    int ret = 0;

    /* Unless we have both a public and private key, we can't do the test */
    if (!ossl_ml_kem_have_prvkey(key)
        || !ossl_ml_kem_have_pubkey(key)
        || (key_flags & ML_KEM_KEY_PCT_TYPE) == 0)
        return 1;
#ifdef FIPS_MODULE
    /* During self test, it is a waste to do this test */
    if (ossl_fips_self_testing())
        return 1;

    /*
     * The functions `OSSL_SELF_TEST_*` will return directly if parameter `st`
     * is NULL.
     */
    OSSL_SELF_TEST_get_callback(key->libctx, &cb, &cbarg);

    st = OSSL_SELF_TEST_new(cb, cbarg);
    if (st == NULL)
        return 0;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_ML_KEM);
#endif  /* FIPS_MODULE */

    ctext = OPENSSL_malloc(v->ctext_bytes);
    if (ctext == NULL)
        goto err;

    memset(out, 0, sizeof(out));

    /*
     * The pairwise test is skipped unless either RANDOM or FIXED entropy PCTs
     * are enabled.
     */
    if (key_flags & ML_KEM_KEY_RANDOM_PCT) {
        operation_result = ossl_ml_kem_encap_rand(ctext, v->ctext_bytes,
                                                  secret, sizeof(secret), key);
    } else {
        memset(entropy, 0125, sizeof(entropy));
        operation_result = ossl_ml_kem_encap_seed(ctext, v->ctext_bytes,
                                                  secret, sizeof(secret),
                                                  entropy, sizeof(entropy),
                                                  key);
    }
    if (operation_result != 1)
        goto err;

#ifdef FIPS_MODULE
    OSSL_SELF_TEST_oncorrupt_byte(st, ctext);
#endif

    operation_result = ossl_ml_kem_decap(out, sizeof(out), ctext, v->ctext_bytes,
                                         key);
    if (operation_result != 1 || memcmp(out, secret, sizeof(out)) != 0)
        goto err;

    ret = 1;
err:
#ifdef FIPS_MODULE
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
#else
    if (ret == 0) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                       "public part of %s private key fails to match private",
                       v->algorithm_name);
    }
#endif
    OPENSSL_free(ctext);
    return ret;
}

ML_KEM_KEY *ossl_prov_ml_kem_new(PROV_CTX *ctx, const char *propq, int evp_type)
{
    ML_KEM_KEY *key;

    if (!ossl_prov_is_running())
        return NULL;
    /*
     * When decoding, if the key ends up "loaded" into the same provider, these
     * are the correct config settings, otherwise, new values will be assigned
     * on import into a different provider.  The "load" API does not pass along
     * the provider context.
     */
    if ((key = ossl_ml_kem_key_new(PROV_LIBCTX_OF(ctx), propq, evp_type)) != NULL) {
        const char *pct_type = ossl_prov_ctx_get_param(
            ctx, OSSL_PKEY_PARAM_ML_KEM_IMPORT_PCT_TYPE, "random");

        if (ossl_prov_ctx_get_bool_param(
            ctx, OSSL_PKEY_PARAM_ML_KEM_RETAIN_SEED, 1))
            key->prov_flags |= ML_KEM_KEY_RETAIN_SEED;
        else
            key->prov_flags &= ~ML_KEM_KEY_RETAIN_SEED;
        if (ossl_prov_ctx_get_bool_param(
            ctx, OSSL_PKEY_PARAM_ML_KEM_PREFER_SEED, 1))
            key->prov_flags |= ML_KEM_KEY_PREFER_SEED;
        else
            key->prov_flags &= ~ML_KEM_KEY_PREFER_SEED;
        if (OPENSSL_strcasecmp(pct_type, "random") == 0)
            key->prov_flags |= ML_KEM_KEY_RANDOM_PCT;
        else if (OPENSSL_strcasecmp(pct_type, "fixed") == 0)
            key->prov_flags |= ML_KEM_KEY_FIXED_PCT;
        else
            key->prov_flags &= ~ML_KEM_KEY_PCT_TYPE;
    }
    return key;
}

static int ml_kem_has(const void *vkey, int selection)
{
    const ML_KEM_KEY *key = vkey;

    /* A NULL key MUST fail to have anything */
    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    switch (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) {
    case 0:
        return 1;
    case OSSL_KEYMGMT_SELECT_PUBLIC_KEY:
        return ossl_ml_kem_have_pubkey(key);
    default:
        return ossl_ml_kem_have_prvkey(key);
    }
}

static int ml_kem_match(const void *vkey1, const void *vkey2, int selection)
{
    const ML_KEM_KEY *key1 = vkey1;
    const ML_KEM_KEY *key2 = vkey2;

    if (!ossl_prov_is_running())
        return 0;

    /* All we have that can be compared is key material */
    if (!(selection & OSSL_KEYMGMT_SELECT_KEYPAIR))
        return 1;

    return ossl_ml_kem_pubkey_cmp(key1, key2);
}

static int ml_kem_validate(const void *vkey, int selection, int check_type)
{
    const ML_KEM_KEY *key = vkey;

    if (!ml_kem_has(key, selection))
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == OSSL_KEYMGMT_SELECT_KEYPAIR)
        return ml_kem_pairwise_test(key, ML_KEM_KEY_RANDOM_PCT);
    return 1;
}

static int ml_kem_export(void *vkey, int selection, OSSL_CALLBACK *param_cb,
                         void *cbarg)
{
    ML_KEM_KEY *key = vkey;
    OSSL_PARAM_BLD *tmpl = NULL;
    OSSL_PARAM *params = NULL;
    const ML_KEM_VINFO *v;
    uint8_t *pubenc = NULL, *prvenc = NULL, *seedenc = NULL;
    size_t prvlen = 0, seedlen = 0;
    int ret = 0;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    v = ossl_ml_kem_key_vinfo(key);
    if (!ossl_ml_kem_have_pubkey(key)) {
        /* Fail when no key material can be returned */
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) == 0
            || !ossl_ml_kem_decoded_key(key)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
            return 0;
        }
    } else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        pubenc = OPENSSL_malloc(v->pubkey_bytes);
        if (pubenc == NULL
            || !ossl_ml_kem_encode_public_key(pubenc, v->pubkey_bytes, key))
            goto err;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        /*
         * The seed and/or private key material are allocated on the secure
         * heap if configured, ossl_param_build_set_octet_string(), will then
         * also use the secure heap.
         */
        if (ossl_ml_kem_have_seed(key)) {
            seedlen = ML_KEM_SEED_BYTES;
            if ((seedenc = OPENSSL_secure_zalloc(seedlen)) == NULL
                || !ossl_ml_kem_encode_seed(seedenc, seedlen, key))
                goto err;
        }
        if (ossl_ml_kem_have_prvkey(key)) {
            prvlen = v->prvkey_bytes;
            if ((prvenc = OPENSSL_secure_zalloc(prvlen)) == NULL
                || !ossl_ml_kem_encode_private_key(prvenc, prvlen, key))
                goto err;
        } else if (ossl_ml_kem_have_dkenc(key)) {
            prvlen = v->prvkey_bytes;
            if ((prvenc = OPENSSL_secure_zalloc(prvlen)) == NULL)
                goto err;
            memcpy(prvenc, key->encoded_dk, prvlen);
        }
    }

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        goto err;

    /* The (d, z) seed, when available and private keys are requested. */
    if (seedenc != NULL
        && !ossl_param_build_set_octet_string(
                tmpl, params, OSSL_PKEY_PARAM_ML_KEM_SEED, seedenc, seedlen))
        goto err;

    /* The private key in the FIPS 203 |dk| format, when requested. */
    if (prvenc != NULL
        && !ossl_param_build_set_octet_string(
                tmpl, params, OSSL_PKEY_PARAM_PRIV_KEY, prvenc, prvlen))
            goto err;

    /* The public key on request; it is always available when either is */
    if (pubenc != NULL
        && !ossl_param_build_set_octet_string(
               tmpl, params, OSSL_PKEY_PARAM_PUB_KEY, pubenc, v->pubkey_bytes))
            goto err;

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);

err:
    OSSL_PARAM_BLD_free(tmpl);
    OPENSSL_secure_clear_free(seedenc, seedlen);
    OPENSSL_secure_clear_free(prvenc, prvlen);
    OPENSSL_free(pubenc);
    return ret;
}

static const OSSL_PARAM *ml_kem_imexport_types(int selection)
{
    static const OSSL_PARAM key_types[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ML_KEM_SEED, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return key_types;
    return NULL;
}

static int check_seed(const uint8_t *seed, const uint8_t *prvenc,
                      ML_KEM_KEY *key)
{
    size_t zlen = ML_KEM_RANDOM_BYTES;

    if (memcmp(seed + ML_KEM_SEED_BYTES - zlen,
               prvenc + key->vinfo->prvkey_bytes - zlen, zlen) == 0)
        return 1;
    ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                   "private %s key implicit rejection secret does"
                   " not match seed", key->vinfo->algorithm_name);
    return 0;
}

static int check_prvenc(const uint8_t *prvenc, ML_KEM_KEY *key)
{
    size_t len = key->vinfo->prvkey_bytes;
    uint8_t *buf = OPENSSL_malloc(len);
    int ret = 0;

    if (buf != NULL
        && ossl_ml_kem_encode_private_key(buf, len, key))
        ret = memcmp(buf, prvenc, len) == 0;
    OPENSSL_clear_free(buf, len);
    if (ret)
        return 1;

    if (buf != NULL)
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                       "explicit %s private key does not match seed",
                       key->vinfo->algorithm_name);
    ossl_ml_kem_key_reset(key);
    return 0;
}

static int ml_kem_key_fromdata(ML_KEM_KEY *key,
                               const OSSL_PARAM params[],
                               int include_private)
{
    const OSSL_PARAM *p = NULL;
    const void *pubenc = NULL, *prvenc = NULL, *seedenc = NULL;
    size_t publen = 0, prvlen = 0, seedlen = 0, puboff;
    const ML_KEM_VINFO *v;

    /* Invalid attempt to mutate a key, what is the right error to report? */
    if (key == NULL || ossl_ml_kem_have_pubkey(key))
        return 0;
    v = ossl_ml_kem_key_vinfo(key);

    /*
     * When a private key is provided, without a seed, any public key also
     * provided will be ignored (apart from length), just as with the seed.
     */
    if (include_private) {
        /*
         * When a seed is provided, the private and public keys may be ignored,
         * after validating just their lengths.  Comparing encodings or hashes
         * when applicable is possible, but not currently implemented.
         */
        p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ML_KEM_SEED);
        if (p != NULL
            && OSSL_PARAM_get_octet_string_ptr(p, &seedenc, &seedlen) != 1)
            return 0;
        if (seedlen != 0 && seedlen != ML_KEM_SEED_BYTES) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
            return 0;
        }
        p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
        if (p != NULL
            && OSSL_PARAM_get_octet_string_ptr(p, &prvenc, &prvlen) != 1)
            return 0;
        if (prvlen != 0 && prvlen != v->prvkey_bytes) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }

    /* Used only when no seed or private key is provided. */
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (p != NULL
        && OSSL_PARAM_get_octet_string_ptr(p, &pubenc, &publen) != 1)
        return 0;
    if (publen != 0 && publen != v->pubkey_bytes) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }

    /* The caller MUST specify at least one of seed, private or public keys. */
    if (seedlen == 0 && publen == 0 && prvlen == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    /* Check any explicit public key against embedded value in private key */
    if (publen > 0 && prvlen > 0) {
        /* point to the ek offset in dk = DKpke||ek||H(ek)||z */
        puboff = prvlen - ML_KEM_RANDOM_BYTES - ML_KEM_PKHASH_BYTES - publen;
        if (memcmp(pubenc, (unsigned char *)prvenc + puboff, publen) != 0) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                           "explicit %s public key does not match private",
                           v->algorithm_name);
            return 0;
        }
    }

    if (seedlen != 0
        && (prvlen == 0 || (key->prov_flags & ML_KEM_KEY_PREFER_SEED))) {
        if (prvlen != 0 && !check_seed(seedenc, prvenc, key))
            return 0;
        if (!ossl_ml_kem_set_seed(seedenc, seedlen, key)
            || !ossl_ml_kem_genkey(NULL, 0, key))
            return 0;
        return prvlen == 0 || check_prvenc(prvenc, key);
    } else if (prvlen != 0) {
        return ossl_ml_kem_parse_private_key(prvenc, prvlen, key);
    }
    return ossl_ml_kem_parse_public_key(pubenc, publen, key);
}

static int ml_kem_import(void *vkey, int selection, const OSSL_PARAM params[])
{
    ML_KEM_KEY *key = vkey;
    int include_private;
    int res;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_private = selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;
    res = ml_kem_key_fromdata(key, params, include_private);
    if (res > 0 && include_private
        && !ml_kem_pairwise_test(key, key->prov_flags)) {
#ifdef FIPS_MODULE
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
#endif
        ossl_ml_kem_key_reset(key);
        res = 0;
    }
    return res;
}

static const OSSL_PARAM *ml_kem_gettable_params(void *provctx)
{
    static const OSSL_PARAM arr[] = {
        OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
        /* Exported for import */
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ML_KEM_SEED, NULL, 0),
        /* Exported to EVP_PKEY_get_raw_private_key() */
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        /* Exported to EVP_PKEY_get_raw_public_key() */
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),
        /* Needed by EVP_PKEY_get1_encoded_public_key() */
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    return arr;
}

#ifndef FIPS_MODULE
void *ml_kem_load(const void *reference, size_t reference_sz)
{
    ML_KEM_KEY *key = NULL;
    uint8_t *encoded_dk = NULL;
    uint8_t seed[ML_KEM_SEED_BYTES];

    if (ossl_prov_is_running() && reference_sz == sizeof(key)) {
        /* The contents of the reference is the address to our object */
        key = *(ML_KEM_KEY **)reference;
        encoded_dk = key->encoded_dk;
        key->encoded_dk = NULL;
        /* We grabbed, so we detach it */
        *(ML_KEM_KEY **)reference = NULL;
        if (encoded_dk != NULL
            && ossl_ml_kem_encode_seed(seed, sizeof(seed), key)
            && !check_seed(seed, encoded_dk, key))
            goto err;
        /* Generate the key now, if it holds only a stashed seed. */
        if (ossl_ml_kem_have_seed(key)
            && (encoded_dk == NULL
                || (key->prov_flags & ML_KEM_KEY_PREFER_SEED))) {
            if (!ossl_ml_kem_genkey(NULL, 0, key)
                || (encoded_dk != NULL && !check_prvenc(encoded_dk, key)))
                goto err;
        } else if (encoded_dk != NULL) {
            if (!ossl_ml_kem_parse_private_key(encoded_dk,
                                               key->vinfo->prvkey_bytes, key)) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                               "error parsing %s private key",
                               key->vinfo->algorithm_name);
                goto err;
            }
            if (!ml_kem_pairwise_test(key, key->prov_flags))
                goto err;
        }
        OPENSSL_free(encoded_dk);
        return key;
    }

 err:
    OPENSSL_free(encoded_dk);
    ossl_ml_kem_key_free(key);
    return NULL;
}
#endif

/*
 * It is assumed the key is guaranteed non-NULL here, and is from this provider
 */
static int ml_kem_get_params(void *vkey, OSSL_PARAM params[])
{
    ML_KEM_KEY *key = vkey;
    const ML_KEM_VINFO *v = ossl_ml_kem_key_vinfo(key);
    OSSL_PARAM *p;
    const char *pubparams[] = {
        OSSL_PKEY_PARAM_PUB_KEY,
        OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY
    };
    int i;

    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, v->bits))
            return 0;

    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, v->secbits))
            return 0;

    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, v->ctext_bytes))
            return 0;

    if (ossl_ml_kem_have_pubkey(key)) {
        uint8_t *pubenc = NULL;

        for (i = 0; i < 2; ++i) {
            p = OSSL_PARAM_locate(params, pubparams[i]);
            if (p == NULL)
                continue;
            if (p->data_type != OSSL_PARAM_OCTET_STRING)
                return 0;
            p->return_size = v->pubkey_bytes;
            if (p->data == NULL)
                continue;
            if (p->data_size < p->return_size)
                return 0;
            if (pubenc != NULL) {
                memcpy(p->data, pubenc, p->return_size);
                continue;
            }
            if (!ossl_ml_kem_encode_public_key(p->data, p->return_size, key))
                return 0;
            pubenc = p->data;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PRIV_KEY);
    if (p != NULL && ossl_ml_kem_have_prvkey(key)) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;
        p->return_size = v->prvkey_bytes;
        if (p->data != NULL) {
            if (p->data_size < p->return_size)
                return 0;
            if (!ossl_ml_kem_encode_private_key(p->data, p->return_size, key))
                return 0;
        }
    }

    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ML_KEM_SEED);
    if (p != NULL && ossl_ml_kem_have_seed(key)) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;
        p->return_size = ML_KEM_SEED_BYTES;
        if (p->data != NULL) {
            if (p->data_size < p->return_size)
                return 0;
            if (!ossl_ml_kem_encode_seed(p->data, p->return_size, key))
                return 0;
        }
    }

    return 1;
}

static const OSSL_PARAM *ml_kem_settable_params(void *provctx)
{
    static const OSSL_PARAM arr[] = {
        /* Used in TLS via EVP_PKEY_set1_encoded_public_key(). */
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    return arr;
}

static int ml_kem_set_params(void *vkey, const OSSL_PARAM params[])
{
    ML_KEM_KEY *key = vkey;
    const OSSL_PARAM *p;
    const void *pubenc = NULL;
    size_t publen = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL
        && (OSSL_PARAM_get_octet_string_ptr(p, &pubenc, &publen) != 1
            || publen != key->vinfo->pubkey_bytes)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        return 0;
    }

    if (publen == 0)
        return 1;

    /* Key mutation is reportedly generally not allowed */
    if (ossl_ml_kem_have_pubkey(key)) {
        ERR_raise_data(ERR_LIB_PROV,
                       PROV_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE,
                       "ML-KEM keys cannot be mutated");
        return 0;
    }

    return ossl_ml_kem_parse_public_key(pubenc, publen, key);
}

static int ml_kem_gen_set_params(void *vgctx, const OSSL_PARAM params[])
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->propq);
        if ((gctx->propq = OPENSSL_strdup(p->data)) == NULL)
            return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ML_KEM_SEED);
    if (p != NULL) {
        size_t len = ML_KEM_SEED_BYTES;

        gctx->seed = gctx->seedbuf;
        if (OSSL_PARAM_get_octet_string(p, (void **)&gctx->seed, len, &len)
            && len == ML_KEM_SEED_BYTES)
            return 1;

        /* Possibly, but less likely wrong data type */
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_SEED_LENGTH);
        gctx->seed = NULL;
        return 0;
    }

    return 1;
}

static void *ml_kem_gen_init(void *provctx, int selection,
                             const OSSL_PARAM params[], int evp_type)
{
    PROV_ML_KEM_GEN_CTX *gctx = NULL;

    /*
     * We can only generate private keys, check that the selection is
     * appropriate.
     */
    if (!ossl_prov_is_running()
        || (selection & minimal_selection) == 0
        || (gctx = OPENSSL_zalloc(sizeof(*gctx))) == NULL)
        return NULL;

    gctx->selection = selection;
    gctx->evp_type = evp_type;
    gctx->provctx = provctx;
    if (ml_kem_gen_set_params(gctx, params))
        return gctx;

    ml_kem_gen_cleanup(gctx);
    return NULL;
}

static const OSSL_PARAM *ml_kem_gen_settable_params(ossl_unused void *vgctx,
                                                    ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ML_KEM_SEED, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static void *ml_kem_gen(void *vgctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;
    ML_KEM_KEY *key;
    uint8_t *nopub = NULL;
    uint8_t *seed;
    int genok = 0;

    if (gctx == NULL
        || (gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) ==
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY)
        return NULL;
    seed = gctx->seed;
    key = ossl_prov_ml_kem_new(gctx->provctx, gctx->propq, gctx->evp_type);
    if (key == NULL)
        return NULL;

    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    if (seed != NULL && !ossl_ml_kem_set_seed(seed, ML_KEM_SEED_BYTES, key))
        return NULL;
    genok = ossl_ml_kem_genkey(nopub, 0, key);

    /* Erase the single-use seed */
    if (seed != NULL)
        OPENSSL_cleanse(seed, ML_KEM_SEED_BYTES);
    gctx->seed = NULL;

    if (genok) {
#ifdef FIPS_MODULE
        if (!ml_kem_pairwise_test(key, ML_KEM_KEY_FIXED_PCT)) {
            ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
            ossl_ml_kem_key_free(key);
            return NULL;
        }
#endif  /* FIPS_MODULE */
        return key;
    }

    ossl_ml_kem_key_free(key);
    return NULL;
}

static void ml_kem_gen_cleanup(void *vgctx)
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;

    if (gctx == NULL)
        return;

    if (gctx->seed != NULL)
        OPENSSL_cleanse(gctx->seed, ML_KEM_RANDOM_BYTES);
    OPENSSL_free(gctx->propq);
    OPENSSL_free(gctx);
}

static void *ml_kem_dup(const void *vkey, int selection)
{
    const ML_KEM_KEY *key = vkey;

    if (!ossl_prov_is_running())
        return NULL;

    return ossl_ml_kem_key_dup(key, selection);
}

#ifndef FIPS_MODULE
# define DISPATCH_LOAD_FN \
        { OSSL_FUNC_KEYMGMT_LOAD, (OSSL_FUNC) ml_kem_load },
#else
# define DISPATCH_LOAD_FN   /* Non-FIPS only */
#endif

#define DECLARE_VARIANT(bits) \
    static void *ml_kem_##bits##_new(void *provctx) \
    { \
        return ossl_prov_ml_kem_new(provctx, NULL, EVP_PKEY_ML_KEM_##bits); \
    } \
    static void *ml_kem_##bits##_gen_init(void *provctx, int selection, \
                                          const OSSL_PARAM params[]) \
    { \
        return ml_kem_gen_init(provctx, selection, params, \
                               EVP_PKEY_ML_KEM_##bits); \
    } \
    const OSSL_DISPATCH ossl_ml_kem_##bits##_keymgmt_functions[] = { \
        { OSSL_FUNC_KEYMGMT_NEW, (OSSL_FUNC) ml_kem_##bits##_new }, \
        { OSSL_FUNC_KEYMGMT_FREE, (OSSL_FUNC) ossl_ml_kem_key_free }, \
        { OSSL_FUNC_KEYMGMT_GET_PARAMS, (OSSL_FUNC) ml_kem_get_params }, \
        { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (OSSL_FUNC) ml_kem_gettable_params }, \
        { OSSL_FUNC_KEYMGMT_SET_PARAMS, (OSSL_FUNC) ml_kem_set_params }, \
        { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (OSSL_FUNC) ml_kem_settable_params }, \
        { OSSL_FUNC_KEYMGMT_HAS, (OSSL_FUNC) ml_kem_has }, \
        { OSSL_FUNC_KEYMGMT_MATCH, (OSSL_FUNC) ml_kem_match }, \
        { OSSL_FUNC_KEYMGMT_VALIDATE, (OSSL_FUNC) ml_kem_validate }, \
        { OSSL_FUNC_KEYMGMT_GEN_INIT, (OSSL_FUNC) ml_kem_##bits##_gen_init }, \
        { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (OSSL_FUNC) ml_kem_gen_set_params }, \
        { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS, (OSSL_FUNC) ml_kem_gen_settable_params }, \
        { OSSL_FUNC_KEYMGMT_GEN, (OSSL_FUNC) ml_kem_gen }, \
        { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (OSSL_FUNC) ml_kem_gen_cleanup }, \
        DISPATCH_LOAD_FN \
        { OSSL_FUNC_KEYMGMT_DUP, (OSSL_FUNC) ml_kem_dup }, \
        { OSSL_FUNC_KEYMGMT_IMPORT, (OSSL_FUNC) ml_kem_import }, \
        { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (OSSL_FUNC) ml_kem_imexport_types }, \
        { OSSL_FUNC_KEYMGMT_EXPORT, (OSSL_FUNC) ml_kem_export }, \
        { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (OSSL_FUNC) ml_kem_imexport_types }, \
        OSSL_DISPATCH_END \
    }
DECLARE_VARIANT(512);
DECLARE_VARIANT(768);
DECLARE_VARIANT(1024);
