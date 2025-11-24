/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/pem.h>         /* Functions for writing MSBLOB and PVK */
#include <openssl/dsa.h>
#include "internal/passphrase.h"
#include "crypto/rsa.h"
#include "prov/implementations.h"
#include "prov/bio.h"
#include "prov/provider_ctx.h"
#include "endecoder_local.h"

struct key2ms_ctx_st {
    PROV_CTX *provctx;

    int pvk_encr_level;

    struct ossl_passphrase_data_st pwdata;
};

static int write_msblob(struct key2ms_ctx_st *ctx, OSSL_CORE_BIO *cout,
                        EVP_PKEY *pkey, int ispub)
{
    BIO *out = ossl_bio_new_from_core_bio(ctx->provctx, cout);
    int ret;

    if (out == NULL)
        return 0;
    ret = ispub ? i2b_PublicKey_bio(out, pkey) : i2b_PrivateKey_bio(out, pkey);

    BIO_free(out);
    return ret;
}

static int write_pvk(struct key2ms_ctx_st *ctx, OSSL_CORE_BIO *cout,
                     EVP_PKEY *pkey)
{
    BIO *out = NULL;
    int ret;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);

    out = ossl_bio_new_from_core_bio(ctx->provctx, cout);
    if (out == NULL)
        return 0;
    ret = i2b_PVK_bio_ex(out, pkey, ctx->pvk_encr_level,
                         ossl_pw_pvk_password, &ctx->pwdata, libctx, NULL);
    BIO_free(out);
    return ret;
}

static OSSL_FUNC_encoder_freectx_fn key2ms_freectx;
static OSSL_FUNC_encoder_does_selection_fn key2ms_does_selection;

static struct key2ms_ctx_st *key2ms_newctx(void *provctx)
{
    struct key2ms_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->provctx = provctx;
        /* This is the strongest encryption level */
        ctx->pvk_encr_level = 2;
    }
    return ctx;
}

static void key2ms_freectx(void *vctx)
{
    struct key2ms_ctx_st *ctx = vctx;

    ossl_pw_clear_passphrase_data(&ctx->pwdata);
    OPENSSL_free(ctx);
}

static const OSSL_PARAM *key2pvk_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_int(OSSL_ENCODER_PARAM_ENCRYPT_LEVEL, NULL),
        OSSL_PARAM_END,
    };

    return settables;
}

static int key2pvk_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct key2ms_ctx_st *ctx = vctx;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_ENCRYPT_LEVEL);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->pvk_encr_level))
        return 0;
    return 1;
}

static int key2ms_does_selection(void *vctx, int selection)
{
    return (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0;
}

/*
 * The real EVP_PKEY_set1_TYPE() functions take a non-const key, while the key
 * pointer in the encode function is a const pointer.  We violate the constness
 * knowingly, since we know that the key comes from the same provider, is never
 * actually const, and the implied reference count change is safe.
 *
 * EVP_PKEY_assign() can't be used, because there's no way to clear the internal
 * key using that function without freeing the existing internal key.
 */
typedef int evp_pkey_set1_fn(EVP_PKEY *, const void *key);

static int key2msblob_encode(void *vctx, const void *key, int selection,
                             OSSL_CORE_BIO *cout, evp_pkey_set1_fn *set1_key,
                             OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct key2ms_ctx_st *ctx = vctx;
    int ispub = -1;
    EVP_PKEY *pkey = NULL;
    int ok = 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ispub = 0;
    else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ispub = 1;
    else
        return 0;                /* Error */

    if ((pkey = EVP_PKEY_new()) != NULL && set1_key(pkey, key))
        ok = write_msblob(ctx, cout, pkey, ispub);
    EVP_PKEY_free(pkey);
    return ok;
}

static int key2pvk_encode(void *vctx, const void *key, int selection,
                          OSSL_CORE_BIO *cout, evp_pkey_set1_fn *set1_key,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct key2ms_ctx_st *ctx = vctx;
    EVP_PKEY *pkey = NULL;
    int ok = 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) == 0)
        return 0;                /* Error */

    if ((pkey = EVP_PKEY_new()) != NULL && set1_key(pkey, key)
        && (pw_cb == NULL
            || ossl_pw_set_ossl_passphrase_cb(&ctx->pwdata, pw_cb, pw_cbarg)))
        ok = write_pvk(ctx, cout, pkey);
    EVP_PKEY_free(pkey);
    return ok;
}

#define dsa_set1        (evp_pkey_set1_fn *)EVP_PKEY_set1_DSA
#define rsa_set1        (evp_pkey_set1_fn *)EVP_PKEY_set1_RSA

#define msblob_set_params
#define pvk_set_params                                                        \
        { OSSL_FUNC_ENCODER_SETTABLE_CTX_PARAMS,                              \
          (void (*)(void))key2pvk_settable_ctx_params },                      \
        { OSSL_FUNC_ENCODER_SET_CTX_PARAMS,                                   \
          (void (*)(void))key2pvk_set_ctx_params },

#define MAKE_MS_ENCODER(impl, output, type)                                   \
    static OSSL_FUNC_encoder_import_object_fn                                 \
    impl##2##output##_import_object;                                          \
    static OSSL_FUNC_encoder_free_object_fn impl##2##output##_free_object;    \
    static OSSL_FUNC_encoder_encode_fn impl##2##output##_encode;              \
                                                                              \
    static void *                                                             \
    impl##2##output##_import_object(void *ctx, int selection,                 \
                                    const OSSL_PARAM params[])                \
    {                                                                         \
        return ossl_prov_import_key(ossl_##impl##_keymgmt_functions,          \
                                    ctx, selection, params);                  \
    }                                                                         \
    static void impl##2##output##_free_object(void *key)                      \
    {                                                                         \
        ossl_prov_free_key(ossl_##impl##_keymgmt_functions, key);             \
    }                                                                         \
    static int impl##2##output##_encode(void *vctx, OSSL_CORE_BIO *cout,      \
                                        const void *key,                      \
                                        const OSSL_PARAM key_abstract[],      \
                                        int selection,                        \
                                        OSSL_PASSPHRASE_CALLBACK *cb,         \
                                        void *cbarg)                          \
    {                                                                         \
        /* We don't deal with abstract objects */                             \
        if (key_abstract != NULL) {                                           \
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);           \
            return 0;                                                         \
        }                                                                     \
        return key2##output##_encode(vctx, key, selection, cout, type##_set1, \
                                     cb, cbarg);                              \
    }                                                                         \
    const OSSL_DISPATCH ossl_##impl##_to_##output##_encoder_functions[] = {   \
        { OSSL_FUNC_ENCODER_NEWCTX,                                           \
          (void (*)(void))key2ms_newctx },                                    \
        { OSSL_FUNC_ENCODER_FREECTX,                                          \
          (void (*)(void))key2ms_freectx },                                   \
        output##_set_params                                                   \
        { OSSL_FUNC_ENCODER_DOES_SELECTION,                                   \
          (void (*)(void))key2ms_does_selection },                            \
        { OSSL_FUNC_ENCODER_IMPORT_OBJECT,                                    \
          (void (*)(void))impl##2##output##_import_object },                  \
        { OSSL_FUNC_ENCODER_FREE_OBJECT,                                      \
          (void (*)(void))impl##2##output##_free_object },                    \
        { OSSL_FUNC_ENCODER_ENCODE,                                           \
          (void (*)(void))impl##2##output##_encode },                         \
        OSSL_DISPATCH_END                                                     \
    }

#ifndef OPENSSL_NO_DSA
MAKE_MS_ENCODER(dsa, pvk, dsa);
MAKE_MS_ENCODER(dsa, msblob, dsa);
#endif

MAKE_MS_ENCODER(rsa, pvk, rsa);
MAKE_MS_ENCODER(rsa, msblob, rsa);
