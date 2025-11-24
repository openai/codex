/*
 * Copyright 2018-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/proverr.h>

#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/providercommon.h"

/*
 * Forward declaration of everything implemented here.  This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_mac_newctx_fn gmac_new;
static OSSL_FUNC_mac_dupctx_fn gmac_dup;
static OSSL_FUNC_mac_freectx_fn gmac_free;
static OSSL_FUNC_mac_gettable_params_fn gmac_gettable_params;
static OSSL_FUNC_mac_get_params_fn gmac_get_params;
static OSSL_FUNC_mac_settable_ctx_params_fn gmac_settable_ctx_params;
static OSSL_FUNC_mac_set_ctx_params_fn gmac_set_ctx_params;
static OSSL_FUNC_mac_init_fn gmac_init;
static OSSL_FUNC_mac_update_fn gmac_update;
static OSSL_FUNC_mac_final_fn gmac_final;

/* local GMAC pkey structure */

struct gmac_data_st {
    void *provctx;
    EVP_CIPHER_CTX *ctx;         /* Cipher context */
    PROV_CIPHER cipher;
};

static void gmac_free(void *vmacctx)
{
    struct gmac_data_st *macctx = vmacctx;

    if (macctx != NULL) {
        EVP_CIPHER_CTX_free(macctx->ctx);
        ossl_prov_cipher_reset(&macctx->cipher);
        OPENSSL_free(macctx);
    }
}

static void *gmac_new(void *provctx)
{
    struct gmac_data_st *macctx;

    if (!ossl_prov_is_running())
        return NULL;

    if ((macctx = OPENSSL_zalloc(sizeof(*macctx))) == NULL
        || (macctx->ctx = EVP_CIPHER_CTX_new()) == NULL) {
        gmac_free(macctx);
        return NULL;
    }
    macctx->provctx = provctx;

    return macctx;
}

static void *gmac_dup(void *vsrc)
{
    struct gmac_data_st *src = vsrc;
    struct gmac_data_st *dst;

    if (!ossl_prov_is_running())
        return NULL;

    dst = gmac_new(src->provctx);
    if (dst == NULL)
        return NULL;

    if (!EVP_CIPHER_CTX_copy(dst->ctx, src->ctx)
        || !ossl_prov_cipher_copy(&dst->cipher, &src->cipher)) {
        gmac_free(dst);
        return NULL;
    }
    return dst;
}

static size_t gmac_size(void)
{
    return EVP_GCM_TLS_TAG_LEN;
}

static int gmac_setkey(struct gmac_data_st *macctx,
                       const unsigned char *key, size_t keylen)
{
    EVP_CIPHER_CTX *ctx = macctx->ctx;

    if (keylen != (size_t)EVP_CIPHER_CTX_get_key_length(ctx)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }
    if (!EVP_EncryptInit_ex(ctx, NULL, NULL, key, NULL))
        return 0;
    return 1;
}

static int gmac_init(void *vmacctx, const unsigned char *key,
                     size_t keylen, const OSSL_PARAM params[])
{
    struct gmac_data_st *macctx = vmacctx;

    if (!ossl_prov_is_running() || !gmac_set_ctx_params(macctx, params))
        return 0;
    if (key != NULL)
        return gmac_setkey(macctx, key, keylen);
    return EVP_EncryptInit_ex(macctx->ctx, NULL, NULL, NULL, NULL);
}

static int gmac_update(void *vmacctx, const unsigned char *data,
                       size_t datalen)
{
    struct gmac_data_st *macctx = vmacctx;
    EVP_CIPHER_CTX *ctx = macctx->ctx;
    int outlen;

    if (datalen == 0)
        return 1;

    while (datalen > INT_MAX) {
        if (!EVP_EncryptUpdate(ctx, NULL, &outlen, data, INT_MAX))
            return 0;
        data += INT_MAX;
        datalen -= INT_MAX;
    }
    return EVP_EncryptUpdate(ctx, NULL, &outlen, data, datalen);
}

static int gmac_final(void *vmacctx, unsigned char *out, size_t *outl,
                      size_t outsize)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    struct gmac_data_st *macctx = vmacctx;
    int hlen = 0;

    if (!ossl_prov_is_running())
        return 0;

    if (!EVP_EncryptFinal_ex(macctx->ctx, out, &hlen))
        return 0;

    hlen = gmac_size();
    params[0] = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TAG,
                                                  out, (size_t)hlen);
    if (!EVP_CIPHER_CTX_get_params(macctx->ctx, params))
        return 0;

    *outl = hlen;
    return 1;
}

static const OSSL_PARAM known_gettable_params[] = {
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
    OSSL_PARAM_END
};
static const OSSL_PARAM *gmac_gettable_params(void *provctx)
{
    return known_gettable_params;
}

static int gmac_get_params(OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_SIZE)) != NULL)
        return OSSL_PARAM_set_size_t(p, gmac_size());

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_MAC_PARAM_CIPHER, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_MAC_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_KEY, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_IV, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *gmac_settable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

/*
 * ALL parameters should be set before init().
 */
static int gmac_set_ctx_params(void *vmacctx, const OSSL_PARAM params[])
{
    struct gmac_data_st *macctx = vmacctx;
    EVP_CIPHER_CTX *ctx = macctx->ctx;
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(macctx->provctx);
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;
    if (ctx == NULL)
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_CIPHER)) != NULL) {
        if (!ossl_prov_cipher_load_from_params(&macctx->cipher, params, provctx))
            return 0;
        if (EVP_CIPHER_get_mode(ossl_prov_cipher_cipher(&macctx->cipher))
            != EVP_CIPH_GCM_MODE) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
            return 0;
        }
        if (!EVP_EncryptInit_ex(ctx, ossl_prov_cipher_cipher(&macctx->cipher),
                                ossl_prov_cipher_engine(&macctx->cipher), NULL,
                                NULL))
            return 0;
    }

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_KEY)) != NULL)
        if (p->data_type != OSSL_PARAM_OCTET_STRING
                || !gmac_setkey(macctx, p->data, p->data_size))
            return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_IV)) != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;

        if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN,
                                 p->data_size, NULL) <= 0
            || !EVP_EncryptInit_ex(ctx, NULL, NULL, NULL, p->data))
            return 0;
    }
    return 1;
}

const OSSL_DISPATCH ossl_gmac_functions[] = {
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))gmac_new },
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))gmac_dup },
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))gmac_free },
    { OSSL_FUNC_MAC_INIT, (void (*)(void))gmac_init },
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))gmac_update },
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))gmac_final },
    { OSSL_FUNC_MAC_GETTABLE_PARAMS, (void (*)(void))gmac_gettable_params },
    { OSSL_FUNC_MAC_GET_PARAMS, (void (*)(void))gmac_get_params },
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,
      (void (*)(void))gmac_settable_ctx_params },
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))gmac_set_ctx_params },
    OSSL_DISPATCH_END
};
