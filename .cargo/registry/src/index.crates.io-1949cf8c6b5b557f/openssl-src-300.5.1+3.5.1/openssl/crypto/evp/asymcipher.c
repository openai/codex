/*
 * Copyright 2006-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "internal/core.h"
#include "crypto/evp.h"
#include "evp_local.h"

static void evp_asym_cipher_free(void *data)
{
    EVP_ASYM_CIPHER_free(data);
}

static int evp_asym_cipher_up_ref(void *data)
{
    return EVP_ASYM_CIPHER_up_ref(data);
}

static int evp_pkey_asym_cipher_init(EVP_PKEY_CTX *ctx, int operation,
                                     const OSSL_PARAM params[])
{
    int ret = 0;
    void *provkey = NULL;
    EVP_ASYM_CIPHER *cipher = NULL;
    const char *desc;
    EVP_KEYMGMT *tmp_keymgmt = NULL;
    const OSSL_PROVIDER *tmp_prov = NULL;
    const char *supported_ciph = NULL;
    int iter;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    evp_pkey_ctx_free_old_ops(ctx);
    ctx->operation = operation;

    ERR_set_mark();

    if (evp_pkey_ctx_is_legacy(ctx))
        goto legacy;

    if (ctx->pkey == NULL) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        goto err;
    }

    /*
     * Try to derive the supported asym cipher from |ctx->keymgmt|.
     */
    if (!ossl_assert(ctx->pkey->keymgmt == NULL
                     || ctx->pkey->keymgmt == ctx->keymgmt)) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    supported_ciph
        = evp_keymgmt_util_query_operation_name(ctx->keymgmt,
                                                OSSL_OP_ASYM_CIPHER);
    if (supported_ciph == NULL) {
        ERR_clear_last_mark();
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

    /*
     * We perform two iterations:
     *
     * 1.  Do the normal asym cipher fetch, using the fetching data given by
     *     the EVP_PKEY_CTX.
     * 2.  Do the provider specific asym cipher fetch, from the same provider
     *     as |ctx->keymgmt|
     *
     * We then try to fetch the keymgmt from the same provider as the
     * asym cipher, and try to export |ctx->pkey| to that keymgmt (when
     * this keymgmt happens to be the same as |ctx->keymgmt|, the export
     * is a no-op, but we call it anyway to not complicate the code even
     * more).
     * If the export call succeeds (returns a non-NULL provider key pointer),
     * we're done and can perform the operation itself.  If not, we perform
     * the second iteration, or jump to legacy.
     */
    for (iter = 1, provkey = NULL; iter < 3 && provkey == NULL; iter++) {
        EVP_KEYMGMT *tmp_keymgmt_tofree;

        /*
         * If we're on the second iteration, free the results from the first.
         * They are NULL on the first iteration, so no need to check what
         * iteration we're on.
         */
        EVP_ASYM_CIPHER_free(cipher);
        EVP_KEYMGMT_free(tmp_keymgmt);

        switch (iter) {
        case 1:
            cipher = EVP_ASYM_CIPHER_fetch(ctx->libctx, supported_ciph,
                                           ctx->propquery);
            if (cipher != NULL)
                tmp_prov = EVP_ASYM_CIPHER_get0_provider(cipher);
            break;
        case 2:
            tmp_prov = EVP_KEYMGMT_get0_provider(ctx->keymgmt);
            cipher =
                evp_asym_cipher_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                                supported_ciph, ctx->propquery);
            if (cipher == NULL)
                goto legacy;
            break;
        }
        if (cipher == NULL)
            continue;

        /*
         * Ensure that the key is provided, either natively, or as a cached
         * export.  We start by fetching the keymgmt with the same name as
         * |ctx->pkey|, but from the provider of the asym cipher method, using
         * the same property query as when fetching the asym cipher method.
         * With the keymgmt we found (if we did), we try to export |ctx->pkey|
         * to it (evp_pkey_export_to_provider() is smart enough to only actually
         * export it if |tmp_keymgmt| is different from |ctx->pkey|'s keymgmt)
         */
        tmp_keymgmt_tofree = tmp_keymgmt
            = evp_keymgmt_fetch_from_prov((OSSL_PROVIDER *)tmp_prov,
                                          EVP_KEYMGMT_get0_name(ctx->keymgmt),
                                          ctx->propquery);
        if (tmp_keymgmt != NULL)
            provkey = evp_pkey_export_to_provider(ctx->pkey, ctx->libctx,
                                                  &tmp_keymgmt, ctx->propquery);
        if (tmp_keymgmt == NULL)
            EVP_KEYMGMT_free(tmp_keymgmt_tofree);
    }

    if (provkey == NULL) {
        EVP_ASYM_CIPHER_free(cipher);
        goto legacy;
    }

    ERR_pop_to_mark();

    /* No more legacy from here down to legacy: */

    ctx->op.ciph.cipher = cipher;
    ctx->op.ciph.algctx = cipher->newctx(ossl_provider_ctx(cipher->prov));
    if (ctx->op.ciph.algctx == NULL) {
        /* The provider key can stay in the cache */
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

    desc = cipher->description != NULL ? cipher->description : "";
    switch (operation) {
    case EVP_PKEY_OP_ENCRYPT:
        if (cipher->encrypt_init == NULL) {
            ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_ASYM_CIPHER_NOT_SUPPORTED,
                           "%s encrypt_init:%s", cipher->type_name, desc);
            ret = -2;
            goto err;
        }
        ret = cipher->encrypt_init(ctx->op.ciph.algctx, provkey, params);
        break;
    case EVP_PKEY_OP_DECRYPT:
        if (cipher->decrypt_init == NULL) {
            ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_ASYM_CIPHER_NOT_SUPPORTED,
                           "%s decrypt_init:%s", cipher->type_name, desc);
            ret = -2;
            goto err;
        }
        ret = cipher->decrypt_init(ctx->op.ciph.algctx, provkey, params);
        break;
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        goto err;
    }

    if (ret <= 0)
        goto err;
    EVP_KEYMGMT_free(tmp_keymgmt);
    return 1;

 legacy:
    /*
     * If we don't have the full support we need with provided methods,
     * let's go see if legacy does.
     */
    ERR_pop_to_mark();
    EVP_KEYMGMT_free(tmp_keymgmt);
    tmp_keymgmt = NULL;

    if (ctx->pmeth == NULL || ctx->pmeth->encrypt == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }
    switch (ctx->operation) {
    case EVP_PKEY_OP_ENCRYPT:
        if (ctx->pmeth->encrypt_init == NULL)
            return 1;
        ret = ctx->pmeth->encrypt_init(ctx);
        break;
    case EVP_PKEY_OP_DECRYPT:
        if (ctx->pmeth->decrypt_init == NULL)
            return 1;
        ret = ctx->pmeth->decrypt_init(ctx);
        break;
    default:
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        ret = -1;
    }

 err:
    if (ret <= 0) {
        evp_pkey_ctx_free_old_ops(ctx);
        ctx->operation = EVP_PKEY_OP_UNDEFINED;
    }
    EVP_KEYMGMT_free(tmp_keymgmt);
    return ret;
}

int EVP_PKEY_encrypt_init(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_asym_cipher_init(ctx, EVP_PKEY_OP_ENCRYPT, NULL);
}

int EVP_PKEY_encrypt_init_ex(EVP_PKEY_CTX *ctx, const OSSL_PARAM params[])
{
    return evp_pkey_asym_cipher_init(ctx, EVP_PKEY_OP_ENCRYPT, params);
}

int EVP_PKEY_encrypt(EVP_PKEY_CTX *ctx,
                     unsigned char *out, size_t *outlen,
                     const unsigned char *in, size_t inlen)
{
    EVP_ASYM_CIPHER *cipher;
    const char *desc;
    int ret;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    if (ctx->operation != EVP_PKEY_OP_ENCRYPT) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }

    if (ctx->op.ciph.algctx == NULL)
        goto legacy;

    cipher = ctx->op.ciph.cipher;
    desc = cipher->description != NULL ? cipher->description : "";
    ret = cipher->encrypt(ctx->op.ciph.algctx, out, outlen, (out == NULL ? 0 : *outlen), in, inlen);
    if (ret <= 0)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_ASYM_CIPHER_FAILURE,
                       "%s encrypt:%s", cipher->type_name, desc);
    return ret;

 legacy:
    if (ctx->pmeth == NULL || ctx->pmeth->encrypt == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }
    M_check_autoarg(ctx, out, outlen, EVP_F_EVP_PKEY_ENCRYPT)
        return ctx->pmeth->encrypt(ctx, out, outlen, in, inlen);
}

int EVP_PKEY_decrypt_init(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_asym_cipher_init(ctx, EVP_PKEY_OP_DECRYPT, NULL);
}

int EVP_PKEY_decrypt_init_ex(EVP_PKEY_CTX *ctx, const OSSL_PARAM params[])
{
    return evp_pkey_asym_cipher_init(ctx, EVP_PKEY_OP_DECRYPT, params);
}

int EVP_PKEY_decrypt(EVP_PKEY_CTX *ctx,
                     unsigned char *out, size_t *outlen,
                     const unsigned char *in, size_t inlen)
{
    EVP_ASYM_CIPHER *cipher;
    const char *desc;
    int ret;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    if (ctx->operation != EVP_PKEY_OP_DECRYPT) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }

    if (ctx->op.ciph.algctx == NULL)
        goto legacy;

    cipher = ctx->op.ciph.cipher;
    desc = cipher->description != NULL ? cipher->description : "";
    ret = cipher->decrypt(ctx->op.ciph.algctx, out, outlen, (out == NULL ? 0 : *outlen), in, inlen);
    if (ret <= 0)
        ERR_raise_data(ERR_LIB_EVP, EVP_R_PROVIDER_ASYM_CIPHER_FAILURE,
                       "%s decrypt:%s", cipher->type_name, desc);

    return ret;

 legacy:
    if (ctx->pmeth == NULL || ctx->pmeth->decrypt == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }
    M_check_autoarg(ctx, out, outlen, EVP_F_EVP_PKEY_DECRYPT)
        return ctx->pmeth->decrypt(ctx, out, outlen, in, inlen);
}

/* decrypt to new buffer of dynamic size, checking any pre-determined size */
int evp_pkey_decrypt_alloc(EVP_PKEY_CTX *ctx, unsigned char **outp,
                           size_t *outlenp, size_t expected_outlen,
                           const unsigned char *in, size_t inlen)
{
    if (EVP_PKEY_decrypt(ctx, NULL, outlenp, in, inlen) <= 0
            || (*outp = OPENSSL_malloc(*outlenp)) == NULL)
        return -1;
    if (EVP_PKEY_decrypt(ctx, *outp, outlenp, in, inlen) <= 0
            || *outlenp == 0
            || (expected_outlen != 0 && *outlenp != expected_outlen)) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        OPENSSL_clear_free(*outp, *outlenp);
        *outp = NULL;
        return 0;
    }
    return 1;
}

static EVP_ASYM_CIPHER *evp_asym_cipher_new(OSSL_PROVIDER *prov)
{
    EVP_ASYM_CIPHER *cipher = OPENSSL_zalloc(sizeof(EVP_ASYM_CIPHER));

    if (cipher == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&cipher->refcnt, 1)
            || !ossl_provider_up_ref(prov)) {
        CRYPTO_FREE_REF(&cipher->refcnt);
        OPENSSL_free(cipher);
        return NULL;
    }
    cipher->prov = prov;

    return cipher;
}

static void *evp_asym_cipher_from_algorithm(int name_id,
                                            const OSSL_ALGORITHM *algodef,
                                            OSSL_PROVIDER *prov)
{
    const OSSL_DISPATCH *fns = algodef->implementation;
    EVP_ASYM_CIPHER *cipher = NULL;
    int ctxfncnt = 0, encfncnt = 0, decfncnt = 0;
    int gparamfncnt = 0, sparamfncnt = 0;

    if ((cipher = evp_asym_cipher_new(prov)) == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        goto err;
    }

    cipher->name_id = name_id;
    if ((cipher->type_name = ossl_algorithm_get1_first_name(algodef)) == NULL)
        goto err;
    cipher->description = algodef->algorithm_description;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_ASYM_CIPHER_NEWCTX:
            if (cipher->newctx != NULL)
                break;
            cipher->newctx = OSSL_FUNC_asym_cipher_newctx(fns);
            ctxfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_ENCRYPT_INIT:
            if (cipher->encrypt_init != NULL)
                break;
            cipher->encrypt_init = OSSL_FUNC_asym_cipher_encrypt_init(fns);
            encfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_ENCRYPT:
            if (cipher->encrypt != NULL)
                break;
            cipher->encrypt = OSSL_FUNC_asym_cipher_encrypt(fns);
            encfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_DECRYPT_INIT:
            if (cipher->decrypt_init != NULL)
                break;
            cipher->decrypt_init = OSSL_FUNC_asym_cipher_decrypt_init(fns);
            decfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_DECRYPT:
            if (cipher->decrypt != NULL)
                break;
            cipher->decrypt = OSSL_FUNC_asym_cipher_decrypt(fns);
            decfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_FREECTX:
            if (cipher->freectx != NULL)
                break;
            cipher->freectx = OSSL_FUNC_asym_cipher_freectx(fns);
            ctxfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_DUPCTX:
            if (cipher->dupctx != NULL)
                break;
            cipher->dupctx = OSSL_FUNC_asym_cipher_dupctx(fns);
            break;
        case OSSL_FUNC_ASYM_CIPHER_GET_CTX_PARAMS:
            if (cipher->get_ctx_params != NULL)
                break;
            cipher->get_ctx_params
                = OSSL_FUNC_asym_cipher_get_ctx_params(fns);
            gparamfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_GETTABLE_CTX_PARAMS:
            if (cipher->gettable_ctx_params != NULL)
                break;
            cipher->gettable_ctx_params
                = OSSL_FUNC_asym_cipher_gettable_ctx_params(fns);
            gparamfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_SET_CTX_PARAMS:
            if (cipher->set_ctx_params != NULL)
                break;
            cipher->set_ctx_params
                = OSSL_FUNC_asym_cipher_set_ctx_params(fns);
            sparamfncnt++;
            break;
        case OSSL_FUNC_ASYM_CIPHER_SETTABLE_CTX_PARAMS:
            if (cipher->settable_ctx_params != NULL)
                break;
            cipher->settable_ctx_params
                = OSSL_FUNC_asym_cipher_settable_ctx_params(fns);
            sparamfncnt++;
            break;
        }
    }
    if (ctxfncnt != 2
        || (encfncnt != 0 && encfncnt != 2)
        || (decfncnt != 0 && decfncnt != 2)
        || (encfncnt != 2 && decfncnt != 2)
        || (gparamfncnt != 0 && gparamfncnt != 2)
        || (sparamfncnt != 0 && sparamfncnt != 2)) {
        /*
         * In order to be a consistent set of functions we must have at least
         * a set of context functions (newctx and freectx) as well as a pair of
         * "cipher" functions: (encrypt_init, encrypt) or
         * (decrypt_init decrypt). set_ctx_params and settable_ctx_params are
         * optional, but if one of them is present then the other one must also
         * be present. The same applies to get_ctx_params and
         * gettable_ctx_params. The dupctx function is optional.
         */
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_PROVIDER_FUNCTIONS);
        goto err;
    }

    return cipher;
 err:
    EVP_ASYM_CIPHER_free(cipher);
    return NULL;
}

void EVP_ASYM_CIPHER_free(EVP_ASYM_CIPHER *cipher)
{
    int i;

    if (cipher == NULL)
        return;
    CRYPTO_DOWN_REF(&cipher->refcnt, &i);
    if (i > 0)
        return;
    OPENSSL_free(cipher->type_name);
    ossl_provider_free(cipher->prov);
    CRYPTO_FREE_REF(&cipher->refcnt);
    OPENSSL_free(cipher);
}

int EVP_ASYM_CIPHER_up_ref(EVP_ASYM_CIPHER *cipher)
{
    int ref = 0;

    CRYPTO_UP_REF(&cipher->refcnt, &ref);
    return 1;
}

OSSL_PROVIDER *EVP_ASYM_CIPHER_get0_provider(const EVP_ASYM_CIPHER *cipher)
{
    return cipher->prov;
}

EVP_ASYM_CIPHER *EVP_ASYM_CIPHER_fetch(OSSL_LIB_CTX *ctx, const char *algorithm,
                                       const char *properties)
{
    return evp_generic_fetch(ctx, OSSL_OP_ASYM_CIPHER, algorithm, properties,
                             evp_asym_cipher_from_algorithm,
                             evp_asym_cipher_up_ref,
                             evp_asym_cipher_free);
}

EVP_ASYM_CIPHER *evp_asym_cipher_fetch_from_prov(OSSL_PROVIDER *prov,
                                                 const char *algorithm,
                                                 const char *properties)
{
    return evp_generic_fetch_from_prov(prov, OSSL_OP_ASYM_CIPHER,
                                       algorithm, properties,
                                       evp_asym_cipher_from_algorithm,
                                       evp_asym_cipher_up_ref,
                                       evp_asym_cipher_free);
}

int EVP_ASYM_CIPHER_is_a(const EVP_ASYM_CIPHER *cipher, const char *name)
{
    return evp_is_a(cipher->prov, cipher->name_id, NULL, name);
}

int evp_asym_cipher_get_number(const EVP_ASYM_CIPHER *cipher)
{
    return cipher->name_id;
}

const char *EVP_ASYM_CIPHER_get0_name(const EVP_ASYM_CIPHER *cipher)
{
    return cipher->type_name;
}

const char *EVP_ASYM_CIPHER_get0_description(const EVP_ASYM_CIPHER *cipher)
{
    return cipher->description;
}

void EVP_ASYM_CIPHER_do_all_provided(OSSL_LIB_CTX *libctx,
                                     void (*fn)(EVP_ASYM_CIPHER *cipher,
                                                void *arg),
                                     void *arg)
{
    evp_generic_do_all(libctx, OSSL_OP_ASYM_CIPHER,
                       (void (*)(void *, void *))fn, arg,
                       evp_asym_cipher_from_algorithm,
                       evp_asym_cipher_up_ref,
                       evp_asym_cipher_free);
}


int EVP_ASYM_CIPHER_names_do_all(const EVP_ASYM_CIPHER *cipher,
                                 void (*fn)(const char *name, void *data),
                                 void *data)
{
    if (cipher->prov != NULL)
        return evp_names_do_all(cipher->prov, cipher->name_id, fn, data);

    return 1;
}

const OSSL_PARAM *EVP_ASYM_CIPHER_gettable_ctx_params(const EVP_ASYM_CIPHER *cip)
{
    void *provctx;

    if (cip == NULL || cip->gettable_ctx_params == NULL)
        return NULL;

    provctx = ossl_provider_ctx(EVP_ASYM_CIPHER_get0_provider(cip));
    return cip->gettable_ctx_params(NULL, provctx);
}

const OSSL_PARAM *EVP_ASYM_CIPHER_settable_ctx_params(const EVP_ASYM_CIPHER *cip)
{
    void *provctx;

    if (cip == NULL || cip->settable_ctx_params == NULL)
        return NULL;

    provctx = ossl_provider_ctx(EVP_ASYM_CIPHER_get0_provider(cip));
    return cip->settable_ctx_params(NULL, provctx);
}
