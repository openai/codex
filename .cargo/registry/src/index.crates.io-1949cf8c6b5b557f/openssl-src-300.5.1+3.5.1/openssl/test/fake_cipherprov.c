/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/rand.h>
#include <openssl/provider.h>
#include <openssl/proverr.h>
#include <openssl/param_build.h>
#include "testutil.h"
#include "fake_cipherprov.h"

#define MAX_KEYNAME 32
#define FAKE_KEY_LEN 16

typedef struct prov_cipher_fake_ctx_st {
    char key_name[MAX_KEYNAME];
    unsigned char key[FAKE_KEY_LEN];
} PROV_CIPHER_FAKE_CTX;

static int ctx_from_key_params(PROV_CIPHER_FAKE_CTX *pctx, const OSSL_PARAM *params)
{
    const OSSL_PARAM *p;
    char key_name[MAX_KEYNAME];
    char *pval = key_name;

    memset(key_name, 0, MAX_KEYNAME);

    p = OSSL_PARAM_locate_const(params, FAKE_CIPHER_PARAM_KEY_NAME);
    if (p != NULL) {
        if (!OSSL_PARAM_get_utf8_string(p, &pval, sizeof(key_name))) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }

        memcpy(pctx->key_name, key_name, sizeof(key_name));
    }

    p = OSSL_PARAM_locate_const(params, OSSL_SKEY_PARAM_RAW_BYTES);
    if (p != NULL) {
        size_t data_size = p->data_size;

        if (data_size > FAKE_KEY_LEN)
            data_size = FAKE_KEY_LEN;
        memcpy(pctx->key, p->data, data_size);
    }

    return 1;
}

static void fake_skeymgmt_free(void *keydata)
{
    OPENSSL_free(keydata);
}

static void *fake_skeymgmt_import(void *provctx, int selection, const OSSL_PARAM *p)
{
    PROV_CIPHER_FAKE_CTX *ctx = NULL;

    if (!TEST_ptr(ctx = OPENSSL_zalloc(sizeof(PROV_CIPHER_FAKE_CTX))))
        return 0;

    if (ctx_from_key_params(ctx, p) != 1) {
        OPENSSL_free(ctx);
        return NULL;
    }

    return ctx;
}

static int fake_skeymgmt_export(void *keydata, int selection,
                                OSSL_CALLBACK *param_callback, void *cbarg)
{
    OSSL_PARAM params[3];
    PROV_CIPHER_FAKE_CTX *ctx = (PROV_CIPHER_FAKE_CTX *)keydata;
    OSSL_PARAM *p = params;

    if (selection & OSSL_SKEYMGMT_SELECT_PARAMETERS) {
        *p = OSSL_PARAM_construct_utf8_string(FAKE_CIPHER_PARAM_KEY_NAME,
                                              ctx->key_name,
                                              strlen(ctx->key_name));
        p++;
    }

    if (selection & OSSL_SKEYMGMT_SELECT_SECRET_KEY) {
        *p = OSSL_PARAM_construct_octet_string(OSSL_SKEY_PARAM_RAW_BYTES,
                                               ctx->key,
                                               sizeof(ctx->key));
        p++;
    }
    *p = OSSL_PARAM_construct_end();

    return param_callback(params, cbarg);
}

static const OSSL_DISPATCH fake_skeymgmt_funcs[] = {
    { OSSL_FUNC_SKEYMGMT_FREE, (void (*)(void))fake_skeymgmt_free },
    { OSSL_FUNC_SKEYMGMT_IMPORT, (void (*)(void))fake_skeymgmt_import },
    { OSSL_FUNC_SKEYMGMT_EXPORT, (void (*)(void))fake_skeymgmt_export },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM fake_skeymgmt_algs[] = {
    { "fake_cipher", FAKE_CIPHER_FETCH_PROPS, fake_skeymgmt_funcs, "Fake Cipher Key Management" },
    { NULL, NULL, NULL, NULL }
};
static OSSL_FUNC_cipher_newctx_fn fake_newctx;

static void *fake_newctx(void *provctx)
{
    return OPENSSL_zalloc(sizeof(PROV_CIPHER_FAKE_CTX));
}

static OSSL_FUNC_cipher_freectx_fn fake_freectx;
static void fake_freectx(void *vctx)
{
    OPENSSL_free(vctx);
}

static int fake_skey_init(PROV_CIPHER_FAKE_CTX *ctx, void *pkeyparam,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (pkeyparam != NULL)
        memcpy(ctx, pkeyparam, sizeof(PROV_CIPHER_FAKE_CTX));

    return 1;
}

static OSSL_FUNC_cipher_encrypt_skey_init_fn fake_skey_einit;
static int fake_skey_einit(void *vctx, void *pkeyparam,
                           const unsigned char *iv, size_t ivlen,
                           const OSSL_PARAM params[])
{
    PROV_CIPHER_FAKE_CTX *ctx = (PROV_CIPHER_FAKE_CTX *)vctx;

    if (fake_skey_init(ctx, pkeyparam, iv, ivlen, params) != 1)
        return 0;

    return 1;
}

static OSSL_FUNC_cipher_decrypt_skey_init_fn fake_skey_dinit;
static int fake_skey_dinit(void *vctx, void *pkeyparam,
                           const unsigned char *iv, size_t ivlen,
                           const OSSL_PARAM params[])
{
    PROV_CIPHER_FAKE_CTX *ctx = (PROV_CIPHER_FAKE_CTX *)vctx;

    if (fake_skey_init(ctx, pkeyparam, iv, ivlen, params) != 1)
        return 0;

    return 1;
}

static OSSL_FUNC_cipher_cipher_fn fake_cipher;
static int fake_cipher(void *vctx, unsigned char *out, size_t *outl,
                       size_t outsize, const unsigned char *in, size_t inl)
{
    PROV_CIPHER_FAKE_CTX *ctx = (PROV_CIPHER_FAKE_CTX *)vctx;
    size_t i;

    if (out == NULL || outsize < inl)
        return 0;
    if (in != out)
        memcpy(out, in, inl);
    for (i = 0; i < inl; i++)
        out[i] ^= ctx->key[i % FAKE_KEY_LEN];
    *outl = inl;
    return 1;
}

static OSSL_FUNC_cipher_final_fn fake_final;
static int fake_final(void *vctx, unsigned char *out, size_t *outl,
                      size_t outsize)
{
    *outl = 0;
    return 1;
}

static OSSL_FUNC_cipher_get_params_fn fake_get_params;
static int fake_get_params(OSSL_PARAM params[])
{
    /* FIXME copy of ossl_cipher_generic_get_params */
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 1)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_BLOCK_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 1)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM fake_known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_BLOCK_SIZE, NULL),
    OSSL_PARAM_END
};

static OSSL_FUNC_cipher_gettable_ctx_params_fn fake_gettable_ctx_params;
static const OSSL_PARAM *fake_gettable_ctx_params(ossl_unused void *cctx,
                                                  ossl_unused void *provctx)
{
    return fake_known_gettable_ctx_params;
}

static OSSL_FUNC_cipher_get_ctx_params_fn fake_get_ctx_params;
static int fake_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_IVLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_KEYLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, sizeof((PROV_CIPHER_FAKE_CTX *)vctx)->key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM fake_known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(FAKE_CIPHER_PARAM_KEY_NAME, NULL, 0),
    OSSL_PARAM_END
};

static OSSL_FUNC_cipher_settable_ctx_params_fn fake_settable_ctx_params;
static const OSSL_PARAM *fake_settable_ctx_params(ossl_unused void *cctx,
                                                  ossl_unused void *provctx)
{
    return fake_known_settable_ctx_params;
}

static const OSSL_DISPATCH ossl_fake_functions[] = {
    { OSSL_FUNC_CIPHER_NEWCTX,
      (void (*)(void)) fake_newctx },
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) fake_freectx },
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) fake_newctx },
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void)) fake_cipher },
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void)) fake_final },
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void)) fake_cipher },
    { OSSL_FUNC_CIPHER_GET_PARAMS, (void (*)(void)) fake_get_params },
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS, (void (*)(void)) fake_get_ctx_params },
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,
      (void (*)(void)) fake_gettable_ctx_params },
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,
      (void (*)(void)) fake_settable_ctx_params },
    { OSSL_FUNC_CIPHER_ENCRYPT_SKEY_INIT, (void (*)(void)) fake_skey_einit },
    { OSSL_FUNC_CIPHER_DECRYPT_SKEY_INIT, (void (*)(void)) fake_skey_dinit },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM fake_cipher_algs[] = {
    { "fake_cipher", FAKE_CIPHER_FETCH_PROPS, ossl_fake_functions},
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *fake_cipher_query(void *provctx,
                                               int operation_id,
                                               int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_CIPHER:
        return fake_cipher_algs;
    case OSSL_OP_SKEYMGMT:
        return fake_skeymgmt_algs;
    }
    return NULL;
}

/* Functions we provide to the core */
static const OSSL_DISPATCH fake_cipher_method[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))OSSL_LIB_CTX_free },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fake_cipher_query },
    OSSL_DISPATCH_END
};

static int fake_cipher_provider_init(const OSSL_CORE_HANDLE *handle,
                                     const OSSL_DISPATCH *in,
                                     const OSSL_DISPATCH **out, void **provctx)
{
    if (!TEST_ptr(*provctx = OSSL_LIB_CTX_new()))
        return 0;
    *out = fake_cipher_method;
    return 1;
}

OSSL_PROVIDER *fake_cipher_start(OSSL_LIB_CTX *libctx)
{
    OSSL_PROVIDER *p;

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, FAKE_PROV_NAME,
                                             fake_cipher_provider_init))
            || !TEST_ptr(p = OSSL_PROVIDER_try_load(libctx, FAKE_PROV_NAME, 1)))
        return NULL;

    return p;
}

void fake_cipher_finish(OSSL_PROVIDER *p)
{
    OSSL_PROVIDER_unload(p);
}
