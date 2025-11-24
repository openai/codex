/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/kdf.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/params.h>
#include "internal/numbers.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/kdfexchange.h"
#include "prov/providercommon.h"

static OSSL_FUNC_keyexch_newctx_fn kdf_tls1_prf_newctx;
static OSSL_FUNC_keyexch_newctx_fn kdf_hkdf_newctx;
static OSSL_FUNC_keyexch_newctx_fn kdf_scrypt_newctx;
static OSSL_FUNC_keyexch_init_fn kdf_init;
static OSSL_FUNC_keyexch_derive_fn kdf_derive;
static OSSL_FUNC_keyexch_freectx_fn kdf_freectx;
static OSSL_FUNC_keyexch_dupctx_fn kdf_dupctx;
static OSSL_FUNC_keyexch_set_ctx_params_fn kdf_set_ctx_params;
static OSSL_FUNC_keyexch_get_ctx_params_fn kdf_get_ctx_params;
static OSSL_FUNC_keyexch_settable_ctx_params_fn kdf_tls1_prf_settable_ctx_params;
static OSSL_FUNC_keyexch_settable_ctx_params_fn kdf_hkdf_settable_ctx_params;
static OSSL_FUNC_keyexch_settable_ctx_params_fn kdf_scrypt_settable_ctx_params;
static OSSL_FUNC_keyexch_gettable_ctx_params_fn kdf_tls1_prf_gettable_ctx_params;
static OSSL_FUNC_keyexch_gettable_ctx_params_fn kdf_hkdf_gettable_ctx_params;
static OSSL_FUNC_keyexch_gettable_ctx_params_fn kdf_scrypt_gettable_ctx_params;

typedef struct {
    void *provctx;
    EVP_KDF_CTX *kdfctx;
    KDF_DATA *kdfdata;
} PROV_KDF_CTX;

static void *kdf_newctx(const char *kdfname, void *provctx)
{
    PROV_KDF_CTX *kdfctx;
    EVP_KDF *kdf = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    kdfctx = OPENSSL_zalloc(sizeof(PROV_KDF_CTX));
    if (kdfctx == NULL)
        return NULL;

    kdfctx->provctx = provctx;

    kdf = EVP_KDF_fetch(PROV_LIBCTX_OF(provctx), kdfname, NULL);
    if (kdf == NULL)
        goto err;
    kdfctx->kdfctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);

    if (kdfctx->kdfctx == NULL)
        goto err;

    return kdfctx;
err:
    OPENSSL_free(kdfctx);
    return NULL;
}

#define KDF_NEWCTX(funcname, kdfname) \
    static void *kdf_##funcname##_newctx(void *provctx) \
    { \
        return kdf_newctx(kdfname, provctx); \
    }

KDF_NEWCTX(tls1_prf, "TLS1-PRF")
KDF_NEWCTX(hkdf, "HKDF")
KDF_NEWCTX(scrypt, "SCRYPT")

static int kdf_init(void *vpkdfctx, void *vkdf, const OSSL_PARAM params[])
{
    PROV_KDF_CTX *pkdfctx = (PROV_KDF_CTX *)vpkdfctx;

    if (!ossl_prov_is_running()
            || pkdfctx == NULL
            || vkdf == NULL
            || !ossl_kdf_data_up_ref(vkdf))
        return 0;
    pkdfctx->kdfdata = vkdf;

    return kdf_set_ctx_params(pkdfctx, params);
}

static int kdf_derive(void *vpkdfctx, unsigned char *secret, size_t *secretlen,
                      size_t outlen)
{
    PROV_KDF_CTX *pkdfctx = (PROV_KDF_CTX *)vpkdfctx;
    size_t kdfsize;
    int ret;

    if (!ossl_prov_is_running())
        return 0;

    kdfsize = EVP_KDF_CTX_get_kdf_size(pkdfctx->kdfctx);

    if (secret == NULL) {
        *secretlen = kdfsize;
        return 1;
    }

    if (kdfsize != SIZE_MAX) {
        if (outlen < kdfsize) {
            ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
            return 0;
        }
        outlen = kdfsize;
    }

    ret = EVP_KDF_derive(pkdfctx->kdfctx, secret, outlen, NULL);
    if (ret <= 0)
        return 0;

    *secretlen = outlen;
    return 1;
}

static void kdf_freectx(void *vpkdfctx)
{
    PROV_KDF_CTX *pkdfctx = (PROV_KDF_CTX *)vpkdfctx;

    EVP_KDF_CTX_free(pkdfctx->kdfctx);
    ossl_kdf_data_free(pkdfctx->kdfdata);

    OPENSSL_free(pkdfctx);
}

static void *kdf_dupctx(void *vpkdfctx)
{
    PROV_KDF_CTX *srcctx = (PROV_KDF_CTX *)vpkdfctx;
    PROV_KDF_CTX *dstctx;

    if (!ossl_prov_is_running())
        return NULL;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;

    dstctx->kdfctx = EVP_KDF_CTX_dup(srcctx->kdfctx);
    if (dstctx->kdfctx == NULL) {
        OPENSSL_free(dstctx);
        return NULL;
    }
    if (!ossl_kdf_data_up_ref(dstctx->kdfdata)) {
        EVP_KDF_CTX_free(dstctx->kdfctx);
        OPENSSL_free(dstctx);
        return NULL;
    }

    return dstctx;
}

static int kdf_set_ctx_params(void *vpkdfctx, const OSSL_PARAM params[])
{
    PROV_KDF_CTX *pkdfctx = (PROV_KDF_CTX *)vpkdfctx;

    return EVP_KDF_CTX_set_params(pkdfctx->kdfctx, params);
}

static int kdf_get_ctx_params(void *vpkdfctx, OSSL_PARAM params[])
{
    PROV_KDF_CTX *pkdfctx = (PROV_KDF_CTX *)vpkdfctx;

    return EVP_KDF_CTX_get_params(pkdfctx->kdfctx, params);
}

static const OSSL_PARAM *kdf_settable_ctx_params(ossl_unused void *vpkdfctx,
                                                 void *provctx,
                                                 const char *kdfname)
{
    EVP_KDF *kdf = EVP_KDF_fetch(PROV_LIBCTX_OF(provctx), kdfname,
                                 NULL);
    const OSSL_PARAM *params;

    if (kdf == NULL)
        return NULL;

    params = EVP_KDF_settable_ctx_params(kdf);
    EVP_KDF_free(kdf);

    return params;
}

#define KDF_SETTABLE_CTX_PARAMS(funcname, kdfname) \
    static const OSSL_PARAM *kdf_##funcname##_settable_ctx_params(void *vpkdfctx, \
                                                                  void *provctx) \
    { \
        return kdf_settable_ctx_params(vpkdfctx, provctx, kdfname); \
    }

KDF_SETTABLE_CTX_PARAMS(tls1_prf, "TLS1-PRF")
KDF_SETTABLE_CTX_PARAMS(hkdf, "HKDF")
KDF_SETTABLE_CTX_PARAMS(scrypt, "SCRYPT")

static const OSSL_PARAM *kdf_gettable_ctx_params(ossl_unused void *vpkdfctx,
                                                 void *provctx,
                                                 const char *kdfname)
{
    EVP_KDF *kdf = EVP_KDF_fetch(PROV_LIBCTX_OF(provctx), kdfname,
                                 NULL);
    const OSSL_PARAM *params;

    if (kdf == NULL)
        return NULL;

    params = EVP_KDF_gettable_ctx_params(kdf);
    EVP_KDF_free(kdf);

    return params;
}

#define KDF_GETTABLE_CTX_PARAMS(funcname, kdfname) \
    static const OSSL_PARAM *kdf_##funcname##_gettable_ctx_params(void *vpkdfctx, \
                                                                  void *provctx) \
    { \
        return kdf_gettable_ctx_params(vpkdfctx, provctx, kdfname); \
    }

KDF_GETTABLE_CTX_PARAMS(tls1_prf, "TLS1-PRF")
KDF_GETTABLE_CTX_PARAMS(hkdf, "HKDF")
KDF_GETTABLE_CTX_PARAMS(scrypt, "SCRYPT")

#define KDF_KEYEXCH_FUNCTIONS(funcname) \
    const OSSL_DISPATCH ossl_kdf_##funcname##_keyexch_functions[] = { \
        { OSSL_FUNC_KEYEXCH_NEWCTX, (void (*)(void))kdf_##funcname##_newctx }, \
        { OSSL_FUNC_KEYEXCH_INIT, (void (*)(void))kdf_init }, \
        { OSSL_FUNC_KEYEXCH_DERIVE, (void (*)(void))kdf_derive }, \
        { OSSL_FUNC_KEYEXCH_FREECTX, (void (*)(void))kdf_freectx }, \
        { OSSL_FUNC_KEYEXCH_DUPCTX, (void (*)(void))kdf_dupctx }, \
        { OSSL_FUNC_KEYEXCH_SET_CTX_PARAMS, (void (*)(void))kdf_set_ctx_params }, \
        { OSSL_FUNC_KEYEXCH_GET_CTX_PARAMS, (void (*)(void))kdf_get_ctx_params }, \
        { OSSL_FUNC_KEYEXCH_SETTABLE_CTX_PARAMS, \
        (void (*)(void))kdf_##funcname##_settable_ctx_params }, \
        { OSSL_FUNC_KEYEXCH_GETTABLE_CTX_PARAMS, \
        (void (*)(void))kdf_##funcname##_gettable_ctx_params }, \
        OSSL_DISPATCH_END \
    };

KDF_KEYEXCH_FUNCTIONS(tls1_prf)
KDF_KEYEXCH_FUNCTIONS(hkdf)
KDF_KEYEXCH_FUNCTIONS(scrypt)
