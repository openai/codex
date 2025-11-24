/*
 * Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/core.h"
#include <openssl/objects.h>
#include <openssl/evp.h>
#include "crypto/bn.h"
#ifndef FIPS_MODULE
# include "crypto/asn1.h"
#endif
#include "crypto/evp.h"
#include "evp_local.h"

static int gen_init(EVP_PKEY_CTX *ctx, int operation)
{
    int ret = 0;

    if (ctx == NULL)
        goto not_supported;

    evp_pkey_ctx_free_old_ops(ctx);
    ctx->operation = operation;

    if (ctx->keymgmt == NULL || ctx->keymgmt->gen_init == NULL)
        goto legacy;

    switch (operation) {
    case EVP_PKEY_OP_PARAMGEN:
        ctx->op.keymgmt.genctx =
            evp_keymgmt_gen_init(ctx->keymgmt,
                                 OSSL_KEYMGMT_SELECT_ALL_PARAMETERS, NULL);
        break;
    case EVP_PKEY_OP_KEYGEN:
        ctx->op.keymgmt.genctx =
            evp_keymgmt_gen_init(ctx->keymgmt, OSSL_KEYMGMT_SELECT_KEYPAIR,
                                 NULL);
        break;
    }

    if (ctx->op.keymgmt.genctx == NULL)
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
    else
        ret = 1;
    goto end;

 legacy:
#ifdef FIPS_MODULE
    goto not_supported;
#else
    if (ctx->pmeth == NULL
        || (operation == EVP_PKEY_OP_PARAMGEN
            && ctx->pmeth->paramgen == NULL)
        || (operation == EVP_PKEY_OP_KEYGEN
            && ctx->pmeth->keygen == NULL))
        goto not_supported;

    ret = 1;
    switch (operation) {
    case EVP_PKEY_OP_PARAMGEN:
        if (ctx->pmeth->paramgen_init != NULL)
            ret = ctx->pmeth->paramgen_init(ctx);
        break;
    case EVP_PKEY_OP_KEYGEN:
        if (ctx->pmeth->keygen_init != NULL)
            ret = ctx->pmeth->keygen_init(ctx);
        break;
    }
#endif

 end:
    if (ret <= 0 && ctx != NULL) {
        evp_pkey_ctx_free_old_ops(ctx);
        ctx->operation = EVP_PKEY_OP_UNDEFINED;
    }
    return ret;

 not_supported:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    ret = -2;
    goto end;
}

int EVP_PKEY_paramgen_init(EVP_PKEY_CTX *ctx)
{
    return gen_init(ctx, EVP_PKEY_OP_PARAMGEN);
}

int EVP_PKEY_keygen_init(EVP_PKEY_CTX *ctx)
{
    return gen_init(ctx, EVP_PKEY_OP_KEYGEN);
}

static int ossl_callback_to_pkey_gencb(const OSSL_PARAM params[], void *arg)
{
    EVP_PKEY_CTX *ctx = arg;
    const OSSL_PARAM *param = NULL;
    int p = -1, n = -1;

    if (ctx->pkey_gencb == NULL)
        return 1;                /* No callback?  That's fine */

    if ((param = OSSL_PARAM_locate_const(params, OSSL_GEN_PARAM_POTENTIAL))
        == NULL
        || !OSSL_PARAM_get_int(param, &p))
        return 0;
    if ((param = OSSL_PARAM_locate_const(params, OSSL_GEN_PARAM_ITERATION))
        == NULL
        || !OSSL_PARAM_get_int(param, &n))
        return 0;

    ctx->keygen_info[0] = p;
    ctx->keygen_info[1] = n;

    return ctx->pkey_gencb(ctx);
}

int EVP_PKEY_generate(EVP_PKEY_CTX *ctx, EVP_PKEY **ppkey)
{
    int ret = 0;
    EVP_PKEY *allocated_pkey = NULL;
    /* Legacy compatible keygen callback info, only used with provider impls */
    int gentmp[2];

    if (ppkey == NULL)
        return -1;

    if (ctx == NULL)
        goto not_supported;

    if ((ctx->operation & EVP_PKEY_OP_TYPE_GEN) == 0)
        goto not_initialized;

    if (*ppkey == NULL)
        *ppkey = allocated_pkey = EVP_PKEY_new();

    if (*ppkey == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        return -1;
    }

    if (ctx->op.keymgmt.genctx == NULL)
        goto legacy;

    /*
     * Assigning gentmp to ctx->keygen_info is something our legacy
     * implementations do.  Because the provider implementations aren't
     * allowed to reach into our EVP_PKEY_CTX, we need to provide similar
     * space for backward compatibility.  It's ok that we attach a local
     * variable, as it should only be useful in the calls down from here.
     * This is cleared as soon as it isn't useful any more, i.e. directly
     * after the evp_keymgmt_util_gen() call.
     */
    ctx->keygen_info = gentmp;
    ctx->keygen_info_count = 2;

    ret = 1;
    if (ctx->pkey != NULL) {
        EVP_KEYMGMT *tmp_keymgmt = ctx->keymgmt;
        void *keydata =
            evp_pkey_export_to_provider(ctx->pkey, ctx->libctx,
                                        &tmp_keymgmt, ctx->propquery);

        if (tmp_keymgmt == NULL)
            goto not_supported;
        /*
         * It's ok if keydata is NULL here.  The backend is expected to deal
         * with that as it sees fit.
         */
        ret = evp_keymgmt_gen_set_template(ctx->keymgmt,
                                           ctx->op.keymgmt.genctx, keydata);
    }

    /*
     * the returned value from evp_keymgmt_util_gen() is cached in *ppkey,
     * so we do not need to save it, just check it.
     */
    ret = ret
        && (evp_keymgmt_util_gen(*ppkey, ctx->keymgmt, ctx->op.keymgmt.genctx,
                                 ossl_callback_to_pkey_gencb, ctx)
            != NULL);

    ctx->keygen_info = NULL;

#ifndef FIPS_MODULE
    /* In case |*ppkey| was originally a legacy key */
    if (ret)
        evp_pkey_free_legacy(*ppkey);
#endif

    /*
     * Because we still have legacy keys
     */
    (*ppkey)->type = ctx->legacy_keytype;

    goto end;

 legacy:
#ifdef FIPS_MODULE
    goto not_supported;
#else
    /*
     * If we get here then we're using legacy paramgen/keygen. In that case
     * the pkey in ctx (if there is one) had better not be provided (because the
     * legacy methods may not know how to handle it). However we can only get
     * here if ctx->op.keymgmt.genctx == NULL, but that should never be the case
     * if ctx->pkey is provided because we don't allow this when we initialise
     * the ctx.
     */
    if (ctx->pkey != NULL && !ossl_assert(!evp_pkey_is_provided(ctx->pkey)))
        goto not_accessible;

    switch (ctx->operation) {
    case EVP_PKEY_OP_PARAMGEN:
        ret = ctx->pmeth->paramgen(ctx, *ppkey);
        break;
    case EVP_PKEY_OP_KEYGEN:
        ret = ctx->pmeth->keygen(ctx, *ppkey);
        break;
    default:
        goto not_supported;
    }
#endif

 end:
    if (ret <= 0) {
        if (allocated_pkey != NULL)
            *ppkey = NULL;
        EVP_PKEY_free(allocated_pkey);
    }
    return ret;

 not_supported:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    ret = -2;
    goto end;
 not_initialized:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
    ret = -1;
    goto end;
#ifndef FIPS_MODULE
 not_accessible:
    ERR_raise(ERR_LIB_EVP, EVP_R_INACCESSIBLE_DOMAIN_PARAMETERS);
    ret = -1;
    goto end;
#endif
}

int EVP_PKEY_paramgen(EVP_PKEY_CTX *ctx, EVP_PKEY **ppkey)
{
    if (ctx->operation != EVP_PKEY_OP_PARAMGEN) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }
    return EVP_PKEY_generate(ctx, ppkey);
}

int EVP_PKEY_keygen(EVP_PKEY_CTX *ctx, EVP_PKEY **ppkey)
{
    if (ctx->operation != EVP_PKEY_OP_KEYGEN) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_INITIALIZED);
        return -1;
    }
    return EVP_PKEY_generate(ctx, ppkey);
}

void EVP_PKEY_CTX_set_cb(EVP_PKEY_CTX *ctx, EVP_PKEY_gen_cb *cb)
{
    ctx->pkey_gencb = cb;
}

EVP_PKEY_gen_cb *EVP_PKEY_CTX_get_cb(EVP_PKEY_CTX *ctx)
{
    return ctx->pkey_gencb;
}

/*
 * "translation callback" to call EVP_PKEY_CTX callbacks using BN_GENCB style
 * callbacks.
 */

static int trans_cb(int a, int b, BN_GENCB *gcb)
{
    EVP_PKEY_CTX *ctx = BN_GENCB_get_arg(gcb);
    ctx->keygen_info[0] = a;
    ctx->keygen_info[1] = b;
    return ctx->pkey_gencb(ctx);
}

void evp_pkey_set_cb_translate(BN_GENCB *cb, EVP_PKEY_CTX *ctx)
{
    BN_GENCB_set(cb, trans_cb, ctx);
}

int EVP_PKEY_CTX_get_keygen_info(EVP_PKEY_CTX *ctx, int idx)
{
    if (idx == -1)
        return ctx->keygen_info_count;
    if (idx < 0 || idx > ctx->keygen_info_count)
        return 0;
    return ctx->keygen_info[idx];
}

#ifndef FIPS_MODULE

EVP_PKEY *EVP_PKEY_new_mac_key(int type, ENGINE *e,
                               const unsigned char *key, int keylen)
{
    EVP_PKEY_CTX *mac_ctx = NULL;
    EVP_PKEY *mac_key = NULL;
    mac_ctx = EVP_PKEY_CTX_new_id(type, e);
    if (!mac_ctx)
        return NULL;
    if (EVP_PKEY_keygen_init(mac_ctx) <= 0)
        goto merr;
    if (EVP_PKEY_CTX_set_mac_key(mac_ctx, key, keylen) <= 0)
        goto merr;
    if (EVP_PKEY_keygen(mac_ctx, &mac_key) <= 0)
        goto merr;
 merr:
    EVP_PKEY_CTX_free(mac_ctx);
    return mac_key;
}

#endif /* FIPS_MODULE */

/*- All methods below can also be used in FIPS_MODULE */

static int fromdata_init(EVP_PKEY_CTX *ctx, int operation)
{
    if (ctx == NULL || ctx->keytype == NULL)
        goto not_supported;

    evp_pkey_ctx_free_old_ops(ctx);
    if (ctx->keymgmt == NULL)
        goto not_supported;

    ctx->operation = operation;
    return 1;

 not_supported:
    if (ctx != NULL)
        ctx->operation = EVP_PKEY_OP_UNDEFINED;
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return -2;
}

int EVP_PKEY_fromdata_init(EVP_PKEY_CTX *ctx)
{
    return fromdata_init(ctx, EVP_PKEY_OP_FROMDATA);
}

int EVP_PKEY_fromdata(EVP_PKEY_CTX *ctx, EVP_PKEY **ppkey, int selection,
                      OSSL_PARAM params[])
{
    void *keydata = NULL;
    EVP_PKEY *allocated_pkey = NULL;

    if (ctx == NULL || (ctx->operation & EVP_PKEY_OP_FROMDATA) == 0) {
        ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
        return -2;
    }

    if (ppkey == NULL)
        return -1;

    if (*ppkey == NULL)
        allocated_pkey = *ppkey = EVP_PKEY_new();

    if (*ppkey == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_EVP_LIB);
        return -1;
    }

    keydata = evp_keymgmt_util_fromdata(*ppkey, ctx->keymgmt, selection, params);
    if (keydata == NULL) {
        if (allocated_pkey != NULL) {
            *ppkey = NULL;
            EVP_PKEY_free(allocated_pkey);
        }
        return 0;
    }
    /* keydata is cached in *ppkey, so we need not bother with it further */
    return 1;
}

const OSSL_PARAM *EVP_PKEY_fromdata_settable(EVP_PKEY_CTX *ctx, int selection)
{
    /* We call fromdata_init to get ctx->keymgmt populated */
    if (fromdata_init(ctx, EVP_PKEY_OP_UNDEFINED) == 1)
        return evp_keymgmt_import_types(ctx->keymgmt, selection);
    return NULL;
}

static OSSL_CALLBACK ossl_pkey_todata_cb;

static int ossl_pkey_todata_cb(const OSSL_PARAM params[], void *arg)
{
    OSSL_PARAM **ret = arg;

    *ret = OSSL_PARAM_dup(params);
    return 1;
}

int EVP_PKEY_todata(const EVP_PKEY *pkey, int selection, OSSL_PARAM **params)
{
    if (params == NULL)
        return 0;
    return EVP_PKEY_export(pkey, selection, ossl_pkey_todata_cb, params);
}

#ifndef FIPS_MODULE
struct fake_import_data_st {
    OSSL_CALLBACK *export_cb;
    void *export_cbarg;
};

static OSSL_FUNC_keymgmt_import_fn pkey_fake_import;
static int pkey_fake_import(void *fake_keydata, int ignored_selection,
                            const OSSL_PARAM params[])
{
    struct fake_import_data_st *data = fake_keydata;

    return data->export_cb(params, data->export_cbarg);
}
#endif

int EVP_PKEY_export(const EVP_PKEY *pkey, int selection,
                    OSSL_CALLBACK *export_cb, void *export_cbarg)
{
    if (pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
#ifndef FIPS_MODULE
    if (evp_pkey_is_legacy(pkey)) {
        struct fake_import_data_st data;

        data.export_cb = export_cb;
        data.export_cbarg = export_cbarg;

        /*
         * We don't need to care about libctx or propq here, as we're only
         * interested in the resulting OSSL_PARAM array.
         */
        return pkey->ameth->export_to(pkey, &data, pkey_fake_import,
                                      NULL, NULL);
    }
#endif
    return evp_keymgmt_util_export(pkey, selection, export_cb, export_cbarg);
}
