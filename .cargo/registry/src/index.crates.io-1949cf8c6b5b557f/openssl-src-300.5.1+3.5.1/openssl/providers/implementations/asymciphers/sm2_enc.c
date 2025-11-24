/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "crypto/sm2.h"
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_util.h"

static OSSL_FUNC_asym_cipher_newctx_fn sm2_newctx;
static OSSL_FUNC_asym_cipher_encrypt_init_fn sm2_init;
static OSSL_FUNC_asym_cipher_encrypt_fn sm2_asym_encrypt;
static OSSL_FUNC_asym_cipher_decrypt_init_fn sm2_init;
static OSSL_FUNC_asym_cipher_decrypt_fn sm2_asym_decrypt;
static OSSL_FUNC_asym_cipher_freectx_fn sm2_freectx;
static OSSL_FUNC_asym_cipher_dupctx_fn sm2_dupctx;
static OSSL_FUNC_asym_cipher_get_ctx_params_fn sm2_get_ctx_params;
static OSSL_FUNC_asym_cipher_gettable_ctx_params_fn sm2_gettable_ctx_params;
static OSSL_FUNC_asym_cipher_set_ctx_params_fn sm2_set_ctx_params;
static OSSL_FUNC_asym_cipher_settable_ctx_params_fn sm2_settable_ctx_params;

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 * We happen to know that our KEYMGMT simply passes EC_KEY structures, so
 * we use that here too.
 */

typedef struct {
    OSSL_LIB_CTX *libctx;
    EC_KEY *key;
    PROV_DIGEST md;
} PROV_SM2_CTX;

static void *sm2_newctx(void *provctx)
{
    PROV_SM2_CTX *psm2ctx =  OPENSSL_zalloc(sizeof(PROV_SM2_CTX));

    if (psm2ctx == NULL)
        return NULL;
    psm2ctx->libctx = PROV_LIBCTX_OF(provctx);

    return psm2ctx;
}

static int sm2_init(void *vpsm2ctx, void *vkey, const OSSL_PARAM params[])
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;

    if (psm2ctx == NULL || vkey == NULL || !EC_KEY_up_ref(vkey))
        return 0;
    EC_KEY_free(psm2ctx->key);
    psm2ctx->key = vkey;

    return sm2_set_ctx_params(psm2ctx, params);
}

static const EVP_MD *sm2_get_md(PROV_SM2_CTX *psm2ctx)
{
    const EVP_MD *md = ossl_prov_digest_md(&psm2ctx->md);

    if (md == NULL)
        md = ossl_prov_digest_fetch(&psm2ctx->md, psm2ctx->libctx, "SM3", NULL);

    return md;
}

static int sm2_asym_encrypt(void *vpsm2ctx, unsigned char *out, size_t *outlen,
                            size_t outsize, const unsigned char *in,
                            size_t inlen)
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;
    const EVP_MD *md = sm2_get_md(psm2ctx);

    if (md == NULL)
        return 0;

    if (out == NULL) {
        if (!ossl_sm2_ciphertext_size(psm2ctx->key, md, inlen, outlen)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
            return 0;
        }
        return 1;
    }

    return ossl_sm2_encrypt(psm2ctx->key, md, in, inlen, out, outlen);
}

static int sm2_asym_decrypt(void *vpsm2ctx, unsigned char *out, size_t *outlen,
                            size_t outsize, const unsigned char *in,
                            size_t inlen)
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;
    const EVP_MD *md = sm2_get_md(psm2ctx);

    if (md == NULL)
        return 0;

    if (out == NULL) {
        if (!ossl_sm2_plaintext_size(in, inlen, outlen))
            return 0;
        return 1;
    }

    return ossl_sm2_decrypt(psm2ctx->key, md, in, inlen, out, outlen);
}

static void sm2_freectx(void *vpsm2ctx)
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;

    EC_KEY_free(psm2ctx->key);
    ossl_prov_digest_reset(&psm2ctx->md);

    OPENSSL_free(psm2ctx);
}

static void *sm2_dupctx(void *vpsm2ctx)
{
    PROV_SM2_CTX *srcctx = (PROV_SM2_CTX *)vpsm2ctx;
    PROV_SM2_CTX *dstctx;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    memset(&dstctx->md, 0, sizeof(dstctx->md));

    if (dstctx->key != NULL && !EC_KEY_up_ref(dstctx->key)) {
        OPENSSL_free(dstctx);
        return NULL;
    }

    if (!ossl_prov_digest_copy(&dstctx->md, &srcctx->md)) {
        sm2_freectx(dstctx);
        return NULL;
    }

    return dstctx;
}

static int sm2_get_ctx_params(void *vpsm2ctx, OSSL_PARAM *params)
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;
    OSSL_PARAM *p;

    if (vpsm2ctx == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_ASYM_CIPHER_PARAM_DIGEST);
    if (p != NULL) {
        const EVP_MD *md = ossl_prov_digest_md(&psm2ctx->md);

        if (!OSSL_PARAM_set_utf8_string(p, md == NULL ? ""
                                                      : EVP_MD_get0_name(md)))
            return 0;
    }

    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_ASYM_CIPHER_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *sm2_gettable_ctx_params(ossl_unused void *vpsm2ctx,
                                                 ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int sm2_set_ctx_params(void *vpsm2ctx, const OSSL_PARAM params[])
{
    PROV_SM2_CTX *psm2ctx = (PROV_SM2_CTX *)vpsm2ctx;

    if (psm2ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_prov_digest_load_from_params(&psm2ctx->md, params,
                                           psm2ctx->libctx))
        return 0;

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_ASYM_CIPHER_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_ASYM_CIPHER_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_ASYM_CIPHER_PARAM_ENGINE, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *sm2_settable_ctx_params(ossl_unused void *vpsm2ctx,
                                                 ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

const OSSL_DISPATCH ossl_sm2_asym_cipher_functions[] = {
    { OSSL_FUNC_ASYM_CIPHER_NEWCTX, (void (*)(void))sm2_newctx },
    { OSSL_FUNC_ASYM_CIPHER_ENCRYPT_INIT, (void (*)(void))sm2_init },
    { OSSL_FUNC_ASYM_CIPHER_ENCRYPT, (void (*)(void))sm2_asym_encrypt },
    { OSSL_FUNC_ASYM_CIPHER_DECRYPT_INIT, (void (*)(void))sm2_init },
    { OSSL_FUNC_ASYM_CIPHER_DECRYPT, (void (*)(void))sm2_asym_decrypt },
    { OSSL_FUNC_ASYM_CIPHER_FREECTX, (void (*)(void))sm2_freectx },
    { OSSL_FUNC_ASYM_CIPHER_DUPCTX, (void (*)(void))sm2_dupctx },
    { OSSL_FUNC_ASYM_CIPHER_GET_CTX_PARAMS,
      (void (*)(void))sm2_get_ctx_params },
    { OSSL_FUNC_ASYM_CIPHER_GETTABLE_CTX_PARAMS,
      (void (*)(void))sm2_gettable_ctx_params },
    { OSSL_FUNC_ASYM_CIPHER_SET_CTX_PARAMS,
      (void (*)(void))sm2_set_ctx_params },
    { OSSL_FUNC_ASYM_CIPHER_SETTABLE_CTX_PARAMS,
      (void (*)(void))sm2_settable_ctx_params },
    OSSL_DISPATCH_END
};
