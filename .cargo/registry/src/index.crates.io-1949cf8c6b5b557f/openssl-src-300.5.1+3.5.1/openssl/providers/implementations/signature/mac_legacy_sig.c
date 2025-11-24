/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/macsignature.h"
#include "prov/providercommon.h"

static OSSL_FUNC_signature_newctx_fn mac_hmac_newctx;
static OSSL_FUNC_signature_newctx_fn mac_siphash_newctx;
static OSSL_FUNC_signature_newctx_fn mac_poly1305_newctx;
static OSSL_FUNC_signature_newctx_fn mac_cmac_newctx;
static OSSL_FUNC_signature_digest_sign_init_fn mac_digest_sign_init;
static OSSL_FUNC_signature_digest_sign_update_fn mac_digest_sign_update;
static OSSL_FUNC_signature_digest_sign_final_fn mac_digest_sign_final;
static OSSL_FUNC_signature_freectx_fn mac_freectx;
static OSSL_FUNC_signature_dupctx_fn mac_dupctx;
static OSSL_FUNC_signature_set_ctx_params_fn mac_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn mac_hmac_settable_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn mac_siphash_settable_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn mac_poly1305_settable_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn mac_cmac_settable_ctx_params;

typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    MAC_KEY *key;
    EVP_MAC_CTX *macctx;
} PROV_MAC_CTX;

static void *mac_newctx(void *provctx, const char *propq, const char *macname)
{
    PROV_MAC_CTX *pmacctx;
    EVP_MAC *mac = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    pmacctx = OPENSSL_zalloc(sizeof(PROV_MAC_CTX));
    if (pmacctx == NULL)
        return NULL;

    pmacctx->libctx = PROV_LIBCTX_OF(provctx);
    if (propq != NULL && (pmacctx->propq = OPENSSL_strdup(propq)) == NULL)
        goto err;

    mac = EVP_MAC_fetch(pmacctx->libctx, macname, propq);
    if (mac == NULL)
        goto err;

    pmacctx->macctx = EVP_MAC_CTX_new(mac);
    if (pmacctx->macctx == NULL)
        goto err;

    EVP_MAC_free(mac);

    return pmacctx;

 err:
    OPENSSL_free(pmacctx->propq);
    OPENSSL_free(pmacctx);
    EVP_MAC_free(mac);
    return NULL;
}

#define MAC_NEWCTX(funcname, macname) \
    static void *mac_##funcname##_newctx(void *provctx, const char *propq) \
    { \
        return mac_newctx(provctx, propq, macname); \
    }

MAC_NEWCTX(hmac, "HMAC")
MAC_NEWCTX(siphash, "SIPHASH")
MAC_NEWCTX(poly1305, "POLY1305")
MAC_NEWCTX(cmac, "CMAC")

static int mac_digest_sign_init(void *vpmacctx, const char *mdname, void *vkey,
                                const OSSL_PARAM params[])
{
    PROV_MAC_CTX *pmacctx = (PROV_MAC_CTX *)vpmacctx;
    const char *ciphername = NULL, *engine = NULL;

    if (!ossl_prov_is_running()
        || pmacctx == NULL)
        return 0;

    if (pmacctx->key == NULL && vkey == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }

    if (vkey != NULL) {
        if (!ossl_mac_key_up_ref(vkey))
            return 0;
        ossl_mac_key_free(pmacctx->key);
        pmacctx->key = vkey;
    }

    if (pmacctx->key->cipher.cipher != NULL)
        ciphername = (char *)EVP_CIPHER_get0_name(pmacctx->key->cipher.cipher);
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    if (pmacctx->key->cipher.engine != NULL)
        engine = (char *)ENGINE_get_id(pmacctx->key->cipher.engine);
#endif

    if (!ossl_prov_set_macctx(pmacctx->macctx, NULL,
                              (char *)ciphername,
                              (char *)mdname,
                              (char *)engine,
                              pmacctx->key->properties,
                              NULL, 0))
        return 0;

    if (!EVP_MAC_init(pmacctx->macctx, pmacctx->key->priv_key,
                      pmacctx->key->priv_key_len, params))
        return 0;

    return 1;
}

int mac_digest_sign_update(void *vpmacctx, const unsigned char *data,
                           size_t datalen)
{
    PROV_MAC_CTX *pmacctx = (PROV_MAC_CTX *)vpmacctx;

    if (pmacctx == NULL || pmacctx->macctx == NULL)
        return 0;

    return EVP_MAC_update(pmacctx->macctx, data, datalen);
}

int mac_digest_sign_final(void *vpmacctx, unsigned char *mac, size_t *maclen,
                          size_t macsize)
{
    PROV_MAC_CTX *pmacctx = (PROV_MAC_CTX *)vpmacctx;

    if (!ossl_prov_is_running() || pmacctx == NULL || pmacctx->macctx == NULL)
        return 0;

    return EVP_MAC_final(pmacctx->macctx, mac, maclen, macsize);
}

static void mac_freectx(void *vpmacctx)
{
    PROV_MAC_CTX *ctx = (PROV_MAC_CTX *)vpmacctx;

    OPENSSL_free(ctx->propq);
    EVP_MAC_CTX_free(ctx->macctx);
    ossl_mac_key_free(ctx->key);
    OPENSSL_free(ctx);
}

static void *mac_dupctx(void *vpmacctx)
{
    PROV_MAC_CTX *srcctx = (PROV_MAC_CTX *)vpmacctx;
    PROV_MAC_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->propq = NULL;
    dstctx->key = NULL;
    dstctx->macctx = NULL;

    if (srcctx->propq != NULL && (dstctx->propq = OPENSSL_strdup(srcctx->propq)) == NULL)
        goto err;

    if (srcctx->key != NULL && !ossl_mac_key_up_ref(srcctx->key))
        goto err;
    dstctx->key = srcctx->key;

    if (srcctx->macctx != NULL) {
        dstctx->macctx = EVP_MAC_CTX_dup(srcctx->macctx);
        if (dstctx->macctx == NULL)
            goto err;
    }

    return dstctx;
 err:
    mac_freectx(dstctx);
    return NULL;
}

static int mac_set_ctx_params(void *vpmacctx, const OSSL_PARAM params[])
{
    PROV_MAC_CTX *ctx = (PROV_MAC_CTX *)vpmacctx;

    return EVP_MAC_CTX_set_params(ctx->macctx, params);
}

static const OSSL_PARAM *mac_settable_ctx_params(ossl_unused void *ctx,
                                                 void *provctx,
                                                 const char *macname)
{
    EVP_MAC *mac = EVP_MAC_fetch(PROV_LIBCTX_OF(provctx), macname,
                                 NULL);
    const OSSL_PARAM *params;

    if (mac == NULL)
        return NULL;

    params = EVP_MAC_settable_ctx_params(mac);
    EVP_MAC_free(mac);

    return params;
}

#define MAC_SETTABLE_CTX_PARAMS(funcname, macname) \
    static const OSSL_PARAM *mac_##funcname##_settable_ctx_params(void *ctx, \
                                                                  void *provctx) \
    { \
        return mac_settable_ctx_params(ctx, provctx, macname); \
    }

MAC_SETTABLE_CTX_PARAMS(hmac, "HMAC")
MAC_SETTABLE_CTX_PARAMS(siphash, "SIPHASH")
MAC_SETTABLE_CTX_PARAMS(poly1305, "POLY1305")
MAC_SETTABLE_CTX_PARAMS(cmac, "CMAC")

#define MAC_SIGNATURE_FUNCTIONS(funcname) \
    const OSSL_DISPATCH ossl_mac_legacy_##funcname##_signature_functions[] = { \
        { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))mac_##funcname##_newctx }, \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT, \
        (void (*)(void))mac_digest_sign_init }, \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_UPDATE, \
        (void (*)(void))mac_digest_sign_update }, \
        { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_FINAL, \
        (void (*)(void))mac_digest_sign_final }, \
        { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))mac_freectx }, \
        { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))mac_dupctx }, \
        { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, \
          (void (*)(void))mac_set_ctx_params }, \
        { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS, \
          (void (*)(void))mac_##funcname##_settable_ctx_params }, \
        OSSL_DISPATCH_END \
    };

MAC_SIGNATURE_FUNCTIONS(hmac)
MAC_SIGNATURE_FUNCTIONS(siphash)
MAC_SIGNATURE_FUNCTIONS(poly1305)
MAC_SIGNATURE_FUNCTIONS(cmac)
