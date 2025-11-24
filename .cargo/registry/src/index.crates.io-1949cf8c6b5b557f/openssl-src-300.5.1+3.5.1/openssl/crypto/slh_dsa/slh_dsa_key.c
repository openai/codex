/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/err.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"
#include "internal/encoder.h"

static int slh_dsa_compute_pk_root(SLH_DSA_HASH_CTX *ctx, SLH_DSA_KEY *out, int verify);

static void slh_dsa_key_hash_cleanup(SLH_DSA_KEY *key)
{
    OPENSSL_free(key->propq);
    if (key->md_big != key->md)
        EVP_MD_free(key->md_big);
    key->md_big = NULL;
    EVP_MD_free(key->md);
    EVP_MAC_free(key->hmac);
    key->md = NULL;
}

static int slh_dsa_key_hash_init(SLH_DSA_KEY *key)
{
    int is_shake = key->params->is_shake;
    int security_category = key->params->security_category;
    const char *digest_alg = is_shake ? "SHAKE-256" : "SHA2-256";

    key->md = EVP_MD_fetch(key->libctx, digest_alg, key->propq);
    if (key->md == NULL)
        return 0;
    /*
     * SHA2 algorithm(s) require SHA256 + HMAC_SHA(X) & MGF1(SHAX)
     * SHAKE algorithm(s) use SHAKE for all functions.
     */
    if (is_shake == 0) {
        if (security_category == 1) {
            /* For category 1 SHA2-256 is used for all hash operations */
            key->md_big = key->md;
        } else {
            /* Security categories 3 & 5 also need SHA-512 */
            key->md_big = EVP_MD_fetch(key->libctx, "SHA2-512", key->propq);
            if (key->md_big == NULL)
                goto err;
        }
        key->hmac = EVP_MAC_fetch(key->libctx, "HMAC", key->propq);
        if (key->hmac == NULL)
            goto err;
    }
    key->adrs_func = ossl_slh_get_adrs_fn(is_shake == 0);
    key->hash_func = ossl_slh_get_hash_fn(is_shake);
    return 1;
 err:
    slh_dsa_key_hash_cleanup(key);
    return 0;
}

static void slh_dsa_key_hash_dup(SLH_DSA_KEY *dst, const SLH_DSA_KEY *src)
{
    if (src->md_big != NULL && src->md_big != src->md)
        EVP_MD_up_ref(src->md_big);
    if (src->md != NULL)
        EVP_MD_up_ref(src->md);
    if (src->hmac != NULL)
        EVP_MAC_up_ref(src->hmac);
}

/**
 * @brief Create a new SLH_DSA_KEY object
 *
 * @param libctx A OSSL_LIB_CTX object used for fetching algorithms.
 * @param propq The property query used for fetching algorithms
 * @param alg The algorithm name associated with the key type
 * @returns The new SLH_DSA_KEY object on success, or NULL on malloc failure
 */
SLH_DSA_KEY *ossl_slh_dsa_key_new(OSSL_LIB_CTX *libctx, const char *propq,
                                  const char *alg)
{
    SLH_DSA_KEY *ret;
    const SLH_DSA_PARAMS *params = ossl_slh_dsa_params_get(alg);

    if (params == NULL)
        return NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret != NULL) {
        ret->libctx = libctx;
        ret->params = params;
        if (propq != NULL) {
            ret->propq = OPENSSL_strdup(propq);
            if (ret->propq == NULL)
                goto err;
        }
        if (!slh_dsa_key_hash_init(ret))
            goto err;
    }
    return ret;
 err:
    ossl_slh_dsa_key_free(ret);
    return NULL;
}

/**
 * @brief Destroy a SLH_DSA_KEY object
 */
void ossl_slh_dsa_key_free(SLH_DSA_KEY *key)
{
    if (key == NULL)
        return;

    slh_dsa_key_hash_cleanup(key);
    OPENSSL_cleanse(&key->priv, sizeof(key->priv) >> 1);
    OPENSSL_free(key);
}

/**
 * @brief Duplicate a key
 *
 * @param src A SLH_DSA_KEY object to copy
 * @param selection to select public and/or private components. Selecting the
 *                  private key will also select the public key
 * @returns The duplicated key, or NULL on failure.
 */
SLH_DSA_KEY *ossl_slh_dsa_key_dup(const SLH_DSA_KEY *src, int selection)
{
    SLH_DSA_KEY *ret = NULL;

    if (src == NULL)
        return NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret != NULL) {
        *ret = *src; /* this copies everything including the keydata in priv[] */
        ret->propq = NULL;
        ret->pub = NULL;
        ret->has_priv = 0;
        slh_dsa_key_hash_dup(ret, src);
        if (src->propq != NULL) {
            ret->propq = OPENSSL_strdup(src->propq);
            if (ret->propq == NULL)
                goto err;
        }
        if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
            /* The public components are present if the private key is present */
            if (src->pub != NULL)
                ret->pub = SLH_DSA_PUB(ret);
            if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
                ret->has_priv = src->has_priv;
        }
    }
    return ret;
 err:
    ossl_slh_dsa_key_free(ret);
    return NULL;
}

/**
 * @brief Are 2 keys equal?
 *
 * To be equal the keys must have the same key data and algorithm name.
 *
 * @param key1 A SLH_DSA_KEY object
 * @param key2 A SLH_DSA_KEY object
 * @param selection to select public and/or private component comparison.
 * @returns 1 if the keys are equal otherwise it returns 0.
 */
int ossl_slh_dsa_key_equal(const SLH_DSA_KEY *key1, const SLH_DSA_KEY *key2,
                           int selection)
{
    int key_checked = 0;

    /* The parameter sets must match - i.e. The same algorithm name */
    if (key1->params != key2->params)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            if (key1->pub != NULL && key2->pub != NULL) {
                if (memcmp(key1->pub, key2->pub, key1->params->pk_len) != 0)
                    return 0;
                key_checked = 1;
            }
        }
        if (!key_checked
                && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            if (key1->has_priv && key2->has_priv) {
                if (memcmp(key1->priv, key2->priv,
                           key1->params->pk_len) != 0)
                    return 0;
                key_checked = 1;
            }
        }
        return key_checked;
    }
    return 1;
}

int ossl_slh_dsa_key_has(const SLH_DSA_KEY *key, int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if (key->pub == NULL)
            return 0; /* No public key */
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
                && key->has_priv == 0)
            return 0; /* No private key */
        return 1;
    }
    return 0;
}

int ossl_slh_dsa_key_pairwise_check(const SLH_DSA_KEY *key)
{
    int ret;
    SLH_DSA_HASH_CTX *ctx = NULL;

    if (key->pub == NULL || key->has_priv == 0)
        return 0;

    ctx = ossl_slh_dsa_hash_ctx_new(key);
    if (ctx == NULL)
        return 0;
    ret = slh_dsa_compute_pk_root(ctx, (SLH_DSA_KEY *)key, 1);
    ossl_slh_dsa_hash_ctx_free(ctx);
    return ret;
}

/**
 * @brief Load a SLH_DSA key from raw data.
 *
 * @param key An SLH_DSA key to load into
 * @param params An array of parameters containing key data.
 * @param include_private Set to 1 to optionally include the private key data
 *                        if it exists.
 * @returns 1 on success, or 0 on failure.
 */
int ossl_slh_dsa_key_fromdata(SLH_DSA_KEY *key, const OSSL_PARAM params[],
                              int include_private)
{
    size_t priv_len, key_len, data_len = 0;
    const OSSL_PARAM *param_priv = NULL, *param_pub = NULL;
    void *p;

    if (key == NULL)
        return 0;

    /* The private key consists of 4 elements SK_SEED, SK_PRF, PK_SEED and PK_ROOT */
    priv_len = ossl_slh_dsa_key_get_priv_len(key);
    /* The size of either SK_SEED + SK_PRF OR PK_SEED + PK_ROOT */
    key_len = priv_len >> 1;

    /* Private key is optional */
    if (include_private) {
        param_priv = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
        if (param_priv != NULL) {
            p = key->priv;
            if (!OSSL_PARAM_get_octet_string(param_priv, &p, priv_len, &data_len))
                return 0;
            /* If the data read includes all 4 elements then we are finished */
            if (data_len == priv_len) {
                key->has_priv = 1;
                key->pub = SLH_DSA_PUB(key);
                return 1;
            }
            /* Otherwise it must be just SK_SEED + SK_PRF */
            if (data_len != key_len)
                goto err;
            key->has_priv = 1;
        }
    }
    /*
     * In the case where the passed in private key does not contain the public key
     * there MUST be a separate public key, since the private key cannot exist
     * without the public key elements. NOTE that this does not accept half of
     * the public key, (Keygen must be used for this case currently).
     */
    p = SLH_DSA_PUB(key);
    param_pub = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (param_pub == NULL
            || !OSSL_PARAM_get_octet_string(param_pub, &p, key_len, &data_len)
            || data_len != key_len)
        goto err;
    key->pub = p;
    return 1;
 err:
    key->pub = NULL;
    key->has_priv = 0;
    OPENSSL_cleanse(key->priv, priv_len);
    return 0;
}

/**
 * Generate the public key root from private key (seed and prf) and public key seed.
 * See FIPS 205 Section 9.1 Algorithm 18
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param out An SLH_DSA key containing the private key (seed and prf) and public key seed.
 *            The public root key is written to this key.
 * @param validate If set to 1 the computed public key is not written to the key,
 *                 but will be compared to the existing value.
 * @returns 1 if the root key is generated or compared successfully, or 0 on error.
 */
static int slh_dsa_compute_pk_root(SLH_DSA_HASH_CTX *ctx, SLH_DSA_KEY *out,
                                   int validate)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_ADRS_DECLARE(adrs);
    const SLH_DSA_PARAMS *params = key->params;
    size_t n = params->n;
    uint8_t pk_root[SLH_DSA_MAX_N], *dst;

    adrsf->zero(adrs);
    adrsf->set_layer_address(adrs, params->d - 1);

    dst = validate ? pk_root : SLH_DSA_PK_ROOT(out);

    /* Generate the ROOT public key */
    return ossl_slh_xmss_node(ctx, SLH_DSA_SK_SEED(key), 0, params->hm,
                              SLH_DSA_PK_SEED(key), adrs, dst, n)
        && (validate == 0 || memcmp(dst, SLH_DSA_PK_ROOT(out), n) == 0);
}

/**
 * @brief Generate a SLH_DSA keypair. The private key seed and prf as well as the
 * public key seed are generated using an approved DRBG's. The public key root is
 * calculated using these generated values.
 * See FIPS 205 Section 10.1 Algorithm 21
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants
 * @param out An SLH_DSA key to write key pair data to.
 * @param lib_ctx A library context for fetching RAND algorithms
 * @param entropy Optional entropy to use instead of using a DRBG.
 *        Required for ACVP testing. It may be NULL.
 * @param entropy_len the size of |entropy|. If set it must be at least 3 * |n|.
 * @returns 1 if the key is generated or 0 otherwise.
 */
int ossl_slh_dsa_generate_key(SLH_DSA_HASH_CTX *ctx, SLH_DSA_KEY *out,
                              OSSL_LIB_CTX *lib_ctx,
                              const uint8_t *entropy, size_t entropy_len)
{
    size_t n = out->params->n;
    size_t secret_key_len = 2 * n; /* The length of SK_SEED + SK_PRF */
    size_t pk_seed_len = n;        /* The length of PK_SEED */
    size_t entropy_len_expected = secret_key_len + pk_seed_len;
    uint8_t *priv = SLH_DSA_PRIV(out);
    uint8_t *pub = SLH_DSA_PUB(out);

    if (entropy != NULL && entropy_len != 0) {
        if (entropy_len != entropy_len_expected)
            goto err;
        memcpy(priv, entropy, entropy_len_expected);
    } else {
        if (RAND_priv_bytes_ex(lib_ctx, priv, secret_key_len, 0) <= 0
                || RAND_bytes_ex(lib_ctx, pub, pk_seed_len, 0) <= 0)
            goto err;
    }
    if (!slh_dsa_compute_pk_root(ctx, out, 0))
        goto err;
    out->pub = pub;
    out->has_priv = 1;
    return 1;
err:
    out->pub = NULL;
    out->has_priv = 0;
    OPENSSL_cleanse(priv, secret_key_len);
    return 0;
}

/**
 * @brief This is used when a SLH key is used for an operation.
 * This checks that the algorithm is the same (i.e. uses the same parameters)
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants to be used for
 *            an operation.
 * @param key A SLH_DSA key to use for an operation.
 *
 * @returns 1 if the algorithm matches, or 0 otherwise.
 */
int ossl_slh_dsa_key_type_matches(const SLH_DSA_KEY *key, const char *alg)
{
    return (OPENSSL_strcasecmp(key->params->alg, alg) == 0);
}

/* Returns the public key data or NULL if there is no public key */
const uint8_t *ossl_slh_dsa_key_get_pub(const SLH_DSA_KEY *key)
{
    return key->pub;
}

/* Returns the constant 2 * |n| which is the size of PK_SEED + PK_ROOT */
size_t ossl_slh_dsa_key_get_pub_len(const SLH_DSA_KEY *key)
{
    return 2 * key->params->n;
}

/* Returns the private key data or NULL if there is no private key */
const uint8_t *ossl_slh_dsa_key_get_priv(const SLH_DSA_KEY *key)
{
    return key->has_priv ? key->priv : NULL;
}

/*
 * Returns the constant 4 * |n| which is the size of both
 * the private and public key components.
 * SK_SEED + SK_ROOT + PK_SEED + PK_ROOT
 */
size_t ossl_slh_dsa_key_get_priv_len(const SLH_DSA_KEY *key)
{
    return 4 * key->params->n;
}

size_t ossl_slh_dsa_key_get_n(const SLH_DSA_KEY *key)
{
    return key->params->n;
}

size_t ossl_slh_dsa_key_get_sig_len(const SLH_DSA_KEY *key)
{
    return key->params->sig_len;
}
const char *ossl_slh_dsa_key_get_name(const SLH_DSA_KEY *key)
{
    return key->params->alg;
}
int ossl_slh_dsa_key_get_type(const SLH_DSA_KEY *key)
{
    return key->params->type;
}

int ossl_slh_dsa_set_priv(SLH_DSA_KEY *key, const uint8_t *priv, size_t priv_len)
{
    if (ossl_slh_dsa_key_get_priv_len(key) != priv_len)
        return 0;
    memcpy(key->priv, priv, priv_len);
    key->has_priv = 1;
    key->pub = SLH_DSA_PUB(key);
    return 1;
}

int ossl_slh_dsa_set_pub(SLH_DSA_KEY *key, const uint8_t *pub, size_t pub_len)
{
    if (ossl_slh_dsa_key_get_pub_len(key) != pub_len)
        return 0;
    key->pub = SLH_DSA_PUB(key);
    memcpy(key->pub, pub, pub_len);
    key->has_priv = 0;
    return 1;
}

#ifndef FIPS_MODULE
int ossl_slh_dsa_key_to_text(BIO *out, const SLH_DSA_KEY *key, int selection)
{
    const char *name;

    if (out == NULL || key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    name = ossl_slh_dsa_key_get_name(key);
    if (ossl_slh_dsa_key_get_pub(key) == NULL) {
        /* Regardless of the |selection|, there must be a public key */
        ERR_raise_data(ERR_LIB_PROV, PROV_R_MISSING_KEY,
                       "no %s key material available", name);
        return 0;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (ossl_slh_dsa_key_get_priv(key) == NULL) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_MISSING_KEY,
                           "no %s key material available", name);
            return 0;
        }
        if (BIO_printf(out, "%s Private-Key:\n", name) <= 0)
            return 0;
        if (!ossl_bio_print_labeled_buf(out, "priv:", ossl_slh_dsa_key_get_priv(key),
                                        ossl_slh_dsa_key_get_priv_len(key)))
            return 0;
    } else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        if (BIO_printf(out, "%s Public-Key:\n", name) <= 0)
            return 0;
    }

    if (!ossl_bio_print_labeled_buf(out, "pub:", ossl_slh_dsa_key_get_pub(key),
                                    ossl_slh_dsa_key_get_pub_len(key)))
        return 0;

    return 1;
}
#endif /* FIPS_MODULE */
