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
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/proverr.h>
#include <openssl/rand.h>
#include "ml_dsa_key.h"
#include "ml_dsa_matrix.h"
#include "ml_dsa_hash.h"
#include "internal/encoder.h"

const ML_DSA_PARAMS *ossl_ml_dsa_key_params(const ML_DSA_KEY *key)
{
    return key->params;
}

/* Returns the seed data or NULL if there is no seed */
const uint8_t *ossl_ml_dsa_key_get_seed(const ML_DSA_KEY *key)
{
    return key->seed;
}

int ossl_ml_dsa_key_get_prov_flags(const ML_DSA_KEY *key)
{
    return key->prov_flags;
}

int ossl_ml_dsa_set_prekey(ML_DSA_KEY *key, int flags_set, int flags_clr,
                           const uint8_t *seed, size_t seed_len,
                           const uint8_t *sk, size_t sk_len)
{
    int ret = 0;

    if (key == NULL
        || key->pub_encoding != NULL
        || key->priv_encoding != NULL
        || (sk != NULL && sk_len != key->params->sk_len)
        || (seed != NULL && seed_len != ML_DSA_SEED_BYTES)
        || key->seed != NULL)
        return 0;

    if (sk != NULL
        && (key->priv_encoding = OPENSSL_memdup(sk, sk_len)) == NULL)
        goto end;
    if (seed != NULL
        && (key->seed = OPENSSL_memdup(seed, seed_len)) == NULL)
        goto end;
    key->prov_flags |= flags_set;
    key->prov_flags &= ~flags_clr;
    ret = 1;

 end:
    if (!ret) {
        OPENSSL_free(key->priv_encoding);
        OPENSSL_free(key->seed);
        key->priv_encoding = key->seed = NULL;
    }
    return ret;
}

/**
 * @brief Create a new ML_DSA_KEY object
 *
 * @param libctx A OSSL_LIB_CTX object used for fetching algorithms.
 * @param propq The property query used for fetching algorithms
 * @param alg The algorithm name associated with the key type
 * @returns The new ML_DSA_KEY object on success, or NULL on malloc failure
 */
ML_DSA_KEY *ossl_ml_dsa_key_new(OSSL_LIB_CTX *libctx, const char *propq,
                                int evp_type)
{
    ML_DSA_KEY *ret;
    const ML_DSA_PARAMS *params = ossl_ml_dsa_params_get(evp_type);

    if (params == NULL)
        return NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret != NULL) {
        ret->libctx = libctx;
        ret->params = params;
        ret->prov_flags = ML_DSA_KEY_PROV_FLAGS_DEFAULT;
        ret->shake128_md = EVP_MD_fetch(libctx, "SHAKE-128", propq);
        ret->shake256_md = EVP_MD_fetch(libctx, "SHAKE-256", propq);
        if (ret->shake128_md == NULL || ret->shake256_md == NULL)
            goto err;
    }
    return ret;
err:
    ossl_ml_dsa_key_free(ret);
    return NULL;
}

int ossl_ml_dsa_key_pub_alloc(ML_DSA_KEY *key)
{
    if (key->t1.poly != NULL)
        return 0;
    return vector_alloc(&key->t1, key->params->k);
}

int ossl_ml_dsa_key_priv_alloc(ML_DSA_KEY *key)
{
    size_t k = key->params->k, l = key->params->l;
    POLY *poly;

    if (key->s1.poly != NULL)
        return 0;
    if (!vector_alloc(&key->s1, l + 2 * k))
        return 0;

    poly = key->s1.poly;
    key->s1.num_poly = l;
    vector_init(&key->s2, poly + l, k);
    vector_init(&key->t0, poly + l + k, k);
    return 1;
}

/**
 * @brief Destroy an ML_DSA_KEY object
 */
void ossl_ml_dsa_key_free(ML_DSA_KEY *key)
{
    if (key == NULL)
        return;

    EVP_MD_free(key->shake128_md);
    EVP_MD_free(key->shake256_md);
    ossl_ml_dsa_key_reset(key);
    OPENSSL_free(key);
}

/**
 * @brief Factory reset an ML_DSA_KEY object
 */
void ossl_ml_dsa_key_reset(ML_DSA_KEY *key)
{
    /*
     * The allocation for |s1.poly| subsumes those for |s2| and |t0|, which we
     * must not access after |s1|'s poly is freed.
     */
    if (key->s1.poly != NULL) {
        vector_zero(&key->s1);
        vector_zero(&key->s2);
        vector_zero(&key->t0);
        vector_free(&key->s1);
        key->s2.poly = NULL;
        key->t0.poly = NULL;
    }
    /* The |t1| vector is public and allocated separately */
    vector_free(&key->t1);
    OPENSSL_cleanse(key->K, sizeof(key->K));
    OPENSSL_free(key->pub_encoding);
    key->pub_encoding = NULL;
    if (key->priv_encoding != NULL)
        OPENSSL_clear_free(key->priv_encoding, key->params->sk_len);
    key->priv_encoding = NULL;
    if (key->seed != NULL)
        OPENSSL_clear_free(key->seed, ML_DSA_SEED_BYTES);
    key->seed = NULL;
}

/**
 * @brief Duplicate a key
 *
 * @param src A ML_DSA_KEY object to copy
 * @param selection to select public and/or private components. Selecting the
 *                  private key will also select the public key
 * @returns The duplicated key, or NULL on failure.
 */
ML_DSA_KEY *ossl_ml_dsa_key_dup(const ML_DSA_KEY *src, int selection)
{
    ML_DSA_KEY *ret = NULL;

    if (src == NULL)
        return NULL;

    /* Prekeys with just a seed or private key are not dupable */
    if (src->pub_encoding == NULL
        && (src->priv_encoding != NULL || src->seed != NULL))
        return NULL;

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret != NULL) {
        ret->libctx = src->libctx;
        ret->params = src->params;
        ret->prov_flags = src->prov_flags;
        if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
            if (src->pub_encoding != NULL) {
                /* The public components are present if the private key is present */
                memcpy(ret->rho, src->rho, sizeof(src->rho));
                memcpy(ret->tr, src->tr, sizeof(src->tr));
                if (src->t1.poly != NULL) {
                    if (!ossl_ml_dsa_key_pub_alloc(ret))
                        goto err;
                    vector_copy(&ret->t1, &src->t1);
                }
                if ((ret->pub_encoding = OPENSSL_memdup(src->pub_encoding,
                                                        src->params->pk_len)) == NULL)
                    goto err;
            }
            if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
                if (src->priv_encoding != NULL) {
                    memcpy(ret->K, src->K, sizeof(src->K));
                    if (src->s1.poly != NULL) {
                        if (!ossl_ml_dsa_key_priv_alloc(ret))
                            goto err;
                        vector_copy(&ret->s1, &src->s1);
                        vector_copy(&ret->s2, &src->s2);
                        vector_copy(&ret->t0, &src->t0);
                    }
                    if ((ret->priv_encoding =
                            OPENSSL_memdup(src->priv_encoding,
                                           src->params->sk_len)) == NULL)
                        goto err;
                }
                if (src->seed != NULL
                    && (ret->seed = OPENSSL_memdup(src->seed,
                                                   ML_DSA_SEED_BYTES)) == NULL)
                    goto err;
            }
        }
        EVP_MD_up_ref(src->shake128_md);
        EVP_MD_up_ref(src->shake256_md);
        ret->shake128_md = src->shake128_md;
        ret->shake256_md = src->shake256_md;
    }
    return ret;
 err:
    ossl_ml_dsa_key_free(ret);
    return NULL;
}

/**
 * @brief Are 2 keys equal?
 *
 * To be equal the keys must have matching public or private key data and
 * contain the same parameters.
 * (Note that in OpenSSL that the private key always has a public key component).
 *
 * @param key1 A ML_DSA_KEY object
 * @param key2 A ML_DSA_KEY object
 * @param selection to select public and/or private component comparison.
 * @returns 1 if the keys are equal otherwise it returns 0.
 */
int ossl_ml_dsa_key_equal(const ML_DSA_KEY *key1, const ML_DSA_KEY *key2,
                          int selection)
{
    int key_checked = 0;

    if (key1->params != key2->params)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            if (key1->pub_encoding != NULL && key2->pub_encoding != NULL) {
                if (memcmp(key1->pub_encoding, key2->pub_encoding,
                                  key1->params->pk_len) != 0)
                    return 0;
                key_checked = 1;
            }
        }
        if (!key_checked
                && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            if (key1->priv_encoding != NULL && key2->priv_encoding != NULL) {
                if (memcmp(key1->priv_encoding, key2->priv_encoding,
                           key1->params->sk_len) != 0)
                    return 0;
                key_checked = 1;
            }
        }
        return key_checked;
    }
    return 1;
}

int ossl_ml_dsa_key_has(const ML_DSA_KEY *key, int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        /* Note that the public key always exists if there is a private key */
        if (ossl_ml_dsa_key_get_pub(key) == NULL)
            return 0; /* No public key */
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
                && ossl_ml_dsa_key_get_priv(key) == NULL)
            return 0; /* No private key */
        return 1;
    }
    return 0;
}

/*
 * @brief Given a key containing private key values for rho, s1 & s2
 * generate the public value t and return the compressed values t1, t0.
 *
 * @param key A private key containing params, rh0, s1 & s2.
 * @param md_ctx A EVP_MD_CTX used for sampling.
 * @param t1 The returned polynomial encoding of the 10 MSB of each coefficient
 *        of the uncompressed public key polynomial t.
 * @param t0 The returned polynomial encoding of the 13 LSB of each coefficient
 *        of the uncompressed public key polynomial t.
 * @returns 1 on success, or 0 on failure.
 */
static int public_from_private(const ML_DSA_KEY *key, EVP_MD_CTX *md_ctx,
                               VECTOR *t1, VECTOR *t0)
{
    const ML_DSA_PARAMS *params = key->params;
    uint32_t k = params->k, l = params->l;
    POLY *polys;
    MATRIX a_ntt;
    VECTOR s1_ntt;
    VECTOR t;

    polys = OPENSSL_malloc(sizeof(*polys) * (k + l + k * l));
    if (polys == NULL)
        return 0;

    vector_init(&t, polys, k);
    vector_init(&s1_ntt, t.poly + k, l);
    matrix_init(&a_ntt, s1_ntt.poly + l, k, l);

    /* Using rho generate A' = A in NTT form */
    if (!matrix_expand_A(md_ctx, key->shake128_md, key->rho, &a_ntt))
        goto err;

    /* t = NTT_inv(A' * NTT(s1)) + s2 */
    vector_copy(&s1_ntt, &key->s1);
    vector_ntt(&s1_ntt);

    matrix_mult_vector(&a_ntt, &s1_ntt, &t);
    vector_ntt_inverse(&t);
    vector_add(&t, &key->s2, &t);

    /* Compress t */
    vector_power2_round(&t, t1, t0);

    /* Zeroize secret */
    vector_zero(&s1_ntt);
err:
    OPENSSL_free(polys);
    return 1;
}

int ossl_ml_dsa_key_public_from_private(ML_DSA_KEY *key)
{
    int ret = 0;
    VECTOR t0;
    EVP_MD_CTX *md_ctx = NULL;

    if (!vector_alloc(&t0, key->params->k)) /* t0 is already in the private key */
        return 0;
    ret = ((md_ctx = EVP_MD_CTX_new())!= NULL)
        && ossl_ml_dsa_key_pub_alloc(key)  /* allocate space for t1 */
        && public_from_private(key, md_ctx, &key->t1, &t0)
        && vector_equal(&t0, &key->t0) /* compare the generated t0 to the expected */
        && ossl_ml_dsa_pk_encode(key)
        && shake_xof(md_ctx, key->shake256_md,
                     key->pub_encoding, key->params->pk_len,
                     key->tr, sizeof(key->tr));
    vector_free(&t0);
    EVP_MD_CTX_free(md_ctx);
    return ret;
}

int ossl_ml_dsa_key_pairwise_check(const ML_DSA_KEY *key)
{
    int ret = 0;
    VECTOR t1, t0;
    POLY *polys = NULL;
    uint32_t k = key->params->k;
    EVP_MD_CTX *md_ctx = NULL;

    if (key->pub_encoding == NULL || key->priv_encoding == 0)
        return 0;

    polys = OPENSSL_malloc(sizeof(*polys) * (2 * k));
    if (polys == NULL)
        return 0;
    md_ctx = EVP_MD_CTX_new();
    if (md_ctx == NULL)
        goto err;

    vector_init(&t1, polys, k);
    vector_init(&t0, polys + k, k);
    if (!public_from_private(key, md_ctx, &t1, &t0))
        goto err;

    ret = vector_equal(&t1, &key->t1) && vector_equal(&t0, &key->t0);
err:
    EVP_MD_CTX_free(md_ctx);
    OPENSSL_free(polys);
    return ret;
}

/*
 * @brief Generate a public-private key pair from a seed.
 * See FIPS 204, Algorithm 6 ML-DSA.KeyGen_internal().
 *
 * @param out The generated key (which contains params on input)
 *
 * @returns 1 on success or 0 on failure.
 */
static int keygen_internal(ML_DSA_KEY *out)
{
    int ret = 0;
    uint8_t augmented_seed[ML_DSA_SEED_BYTES + 2];
    uint8_t expanded_seed[ML_DSA_RHO_BYTES + ML_DSA_PRIV_SEED_BYTES + ML_DSA_K_BYTES];
    const uint8_t *const rho = expanded_seed; /* p = Public Random Seed */
    const uint8_t *const priv_seed = expanded_seed + ML_DSA_RHO_BYTES;
    const uint8_t *const K = priv_seed + ML_DSA_PRIV_SEED_BYTES;
    const ML_DSA_PARAMS *params = out->params;
    EVP_MD_CTX *md_ctx = NULL;

    if (out->seed == NULL
        || (md_ctx = EVP_MD_CTX_new()) == NULL
        || !ossl_ml_dsa_key_pub_alloc(out)
        || !ossl_ml_dsa_key_priv_alloc(out))
        goto err;

    /* augmented_seed = seed || k || l */
    memcpy(augmented_seed, out->seed, ML_DSA_SEED_BYTES);
    augmented_seed[ML_DSA_SEED_BYTES] = (uint8_t)params->k;
    augmented_seed[ML_DSA_SEED_BYTES + 1] = (uint8_t)params->l;
    /* Expand the seed into p[32], p'[64], K[32] */
    if (!shake_xof(md_ctx, out->shake256_md, augmented_seed, sizeof(augmented_seed),
                   expanded_seed, sizeof(expanded_seed)))
        goto err;

    memcpy(out->rho, rho, sizeof(out->rho));
    memcpy(out->K, K, sizeof(out->K));

    ret = vector_expand_S(md_ctx, out->shake256_md, params->eta, priv_seed, &out->s1, &out->s2)
        && public_from_private(out, md_ctx, &out->t1, &out->t0)
        && ossl_ml_dsa_pk_encode(out)
        && shake_xof(md_ctx, out->shake256_md, out->pub_encoding, out->params->pk_len,
                     out->tr, sizeof(out->tr))
        && ossl_ml_dsa_sk_encode(out);

err:
    if (out->seed != NULL && (out->prov_flags & ML_DSA_KEY_RETAIN_SEED) == 0) {
        OPENSSL_clear_free(out->seed, ML_DSA_SEED_BYTES);
        out->seed = NULL;
    }
    EVP_MD_CTX_free(md_ctx);
    OPENSSL_cleanse(augmented_seed, sizeof(augmented_seed));
    OPENSSL_cleanse(expanded_seed, sizeof(expanded_seed));
    return ret;
}

int ossl_ml_dsa_generate_key(ML_DSA_KEY *out)
{
    size_t seed_len = ML_DSA_SEED_BYTES;
    uint8_t *sk;
    int ret;

    if (out->seed == NULL) {
        if ((out->seed = OPENSSL_malloc(seed_len)) == NULL)
            return 0;
        if (RAND_priv_bytes_ex(out->libctx, out->seed, seed_len, 0) <= 0) {
            OPENSSL_free(out->seed);
            out->seed = NULL;
            return 0;
        }
    }
    /* We're generating from a seed, drop private prekey encoding */
    sk = out->priv_encoding;
    out->priv_encoding = NULL;
    if (sk == NULL) {
        ret = keygen_internal(out);
    } else {
        if ((ret = keygen_internal(out)) != 0
            && memcmp(out->priv_encoding, sk, out->params->sk_len) != 0) {
            ret = 0;
            ossl_ml_dsa_key_reset(out);
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                           "explicit %s private key does not match seed",
                           out->params->alg);
        }
        OPENSSL_free(sk);
    }
    return ret;
}

/**
 * @brief This is used when a ML DSA key is used for an operation.
 * This checks that the algorithm is the same (i.e. uses the same parameters)
 *
 * @param key A ML_DSA key to use for an operation.
 * @param evp_type The algorithm nid associated with an operation
 *
 * @returns 1 if the algorithm matches, or 0 otherwise.
 */

int ossl_ml_dsa_key_matches(const ML_DSA_KEY *key, int evp_type)
{
    return (key->params->evp_type == evp_type);
}

/* Returns the public key data or NULL if there is no public key */
const uint8_t *ossl_ml_dsa_key_get_pub(const ML_DSA_KEY *key)
{
    return key->pub_encoding;
}

/* Returns the encoded public key size */
size_t ossl_ml_dsa_key_get_pub_len(const ML_DSA_KEY *key)
{
    return key->params->pk_len;
}

size_t ossl_ml_dsa_key_get_collision_strength_bits(const ML_DSA_KEY *key)
{
    return key->params->bit_strength;
}

/* Returns the private key data or NULL if there is no private key */
const uint8_t *ossl_ml_dsa_key_get_priv(const ML_DSA_KEY *key)
{
    return key->priv_encoding;
}

size_t ossl_ml_dsa_key_get_priv_len(const ML_DSA_KEY *key)
{
    return key->params->sk_len;
}

size_t ossl_ml_dsa_key_get_sig_len(const ML_DSA_KEY *key)
{
    return key->params->sig_len;
}

OSSL_LIB_CTX *ossl_ml_dsa_key_get0_libctx(const ML_DSA_KEY *key)
{
    return key != NULL ? key->libctx : NULL;
}

const char *ossl_ml_dsa_key_get_name(const ML_DSA_KEY *key)
{
    return key->params->alg;
}
