/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stddef.h>
#include <openssl/crypto.h>
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"
#include <openssl/evp.h>

/**
 * @brief Create a SLH_DSA_HASH_CTX that contains parameters, functions, and
 * pre-fetched HASH related objects for a SLH_DSA algorithm.This context is passed
 * to most SLH-DSA functions.
 *
 * @param alg An SLH-DSA algorithm name such as "SLH-DSA-SHA2-128s"
 * @param lib_ctx A library context used for fetching. Can be NULL
 * @param propq A propqery query to use for algorithm fetching. Can be NULL.
 *
 * @returns The created SLH_DSA_HASH_CTX object or NULL on failure.
 */
SLH_DSA_HASH_CTX *ossl_slh_dsa_hash_ctx_new(const SLH_DSA_KEY *key)
{
    SLH_DSA_HASH_CTX *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->key = key;
    ret->md_ctx = EVP_MD_CTX_new();
    if (ret->md_ctx == NULL)
        goto err;
    if (EVP_DigestInit_ex2(ret->md_ctx, key->md, NULL) != 1)
        goto err;
    if (key->md_big != NULL) {
        /* Gets here for SHA2 algorithms */
        if (key->md_big == key->md) {
            ret->md_big_ctx = ret->md_ctx;
        } else {
            /* Only gets here for SHA2 */
            ret->md_big_ctx = EVP_MD_CTX_new();
            if (ret->md_big_ctx == NULL)
                goto err;
            if (EVP_DigestInit_ex2(ret->md_big_ctx, key->md_big, NULL) != 1)
                goto err;
        }
        if (key->hmac != NULL) {
            ret->hmac_ctx = EVP_MAC_CTX_new(key->hmac);
            if (ret->hmac_ctx == NULL)
                goto err;
        }
    }
    return ret;
 err:
    ossl_slh_dsa_hash_ctx_free(ret);
    return NULL;
}

/**
 * @brief Duplicate a SLH_DSA_HASH_CTX
 *
 * @param ctx The SLH_DSA_HASH_CTX object to duplicate.
 */
SLH_DSA_HASH_CTX *ossl_slh_dsa_hash_ctx_dup(const SLH_DSA_HASH_CTX *src)
{
    SLH_DSA_HASH_CTX *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->hmac_digest_used = src->hmac_digest_used;
    /* Note that the key is not ref counted, since it does not own the key */
    ret->key = src->key;

    if (src->md_ctx != NULL
            && (ret->md_ctx = EVP_MD_CTX_dup(src->md_ctx)) == NULL)
        goto err;
    if (src->md_big_ctx != NULL) {
        if (src->md_big_ctx != src->md_ctx) {
            if ((ret->md_big_ctx = EVP_MD_CTX_dup(src->md_big_ctx)) == NULL)
                goto err;
        } else {
            ret->md_big_ctx = ret->md_ctx;
        }
    }
    if (src->hmac_ctx != NULL
            && (ret->hmac_ctx = EVP_MAC_CTX_dup(src->hmac_ctx)) == NULL)
        goto err;
    return ret;
 err:
    ossl_slh_dsa_hash_ctx_free(ret);
    return NULL;
}

/**
 * @brief Destroy a SLH_DSA_HASH_CTX
 *
 * @param ctx The SLH_DSA_HASH_CTX object to destroy.
 */
void ossl_slh_dsa_hash_ctx_free(SLH_DSA_HASH_CTX *ctx)
{
    if (ctx == NULL)
        return;
    EVP_MD_CTX_free(ctx->md_ctx);
    if (ctx->md_big_ctx != ctx->md_ctx)
        EVP_MD_CTX_free(ctx->md_big_ctx);
    EVP_MAC_CTX_free(ctx->hmac_ctx);
    OPENSSL_free(ctx);
}
