/*
 * Copyright 2006-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include "internal/cryptlib.h"
#include <openssl/objects.h>
#include <openssl/evp.h>
#include "crypto/bn.h"
#ifndef FIPS_MODULE
# include "crypto/asn1.h"
#endif
#include "crypto/evp.h"
#include "evp_local.h"

/*
 * Returns:
 *  1   True
 *  0   False
 * -1   Unsupported (use legacy path)
 */
static int try_provided_check(EVP_PKEY_CTX *ctx, int selection, int checktype)
{
    EVP_KEYMGMT *keymgmt;
    void *keydata;

    if (evp_pkey_ctx_is_legacy(ctx))
        return -1;

    keymgmt = ctx->keymgmt;
    keydata = evp_pkey_export_to_provider(ctx->pkey, ctx->libctx,
                                          &keymgmt, ctx->propquery);
    if (keydata == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INITIALIZATION_ERROR);
        return 0;
    }

    return evp_keymgmt_validate(keymgmt, keydata, selection, checktype);
}

static int evp_pkey_public_check_combined(EVP_PKEY_CTX *ctx, int checktype)
{
    EVP_PKEY *pkey = ctx->pkey;
    int ok;

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        return 0;
    }

    if ((ok = try_provided_check(ctx, OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
                                 checktype)) != -1)
        return ok;

    if (pkey->type == EVP_PKEY_NONE)
        goto not_supported;

#ifndef FIPS_MODULE
    /* legacy */
    /* call customized public key check function first */
    if (ctx->pmeth->public_check != NULL)
        return ctx->pmeth->public_check(pkey);

    /* use default public key check function in ameth */
    if (pkey->ameth == NULL || pkey->ameth->pkey_public_check == NULL)
        goto not_supported;

    return pkey->ameth->pkey_public_check(pkey);
#endif
 not_supported:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return -2;
}

int EVP_PKEY_public_check(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_public_check_combined(ctx, OSSL_KEYMGMT_VALIDATE_FULL_CHECK);
}

int EVP_PKEY_public_check_quick(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_public_check_combined(ctx, OSSL_KEYMGMT_VALIDATE_QUICK_CHECK);
}

static int evp_pkey_param_check_combined(EVP_PKEY_CTX *ctx, int checktype)
{
    EVP_PKEY *pkey = ctx->pkey;
    int ok;

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        return 0;
    }

    if ((ok = try_provided_check(ctx,
                                 OSSL_KEYMGMT_SELECT_ALL_PARAMETERS,
                                 checktype)) != -1)
        return ok;

    if (pkey->type == EVP_PKEY_NONE)
        goto not_supported;

#ifndef FIPS_MODULE
    /* legacy */
    /* call customized param check function first */
    if (ctx->pmeth->param_check != NULL)
        return ctx->pmeth->param_check(pkey);

    /* use default param check function in ameth */
    if (pkey->ameth == NULL || pkey->ameth->pkey_param_check == NULL)
        goto not_supported;

    return pkey->ameth->pkey_param_check(pkey);
#endif
 not_supported:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return -2;
}

int EVP_PKEY_param_check(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_param_check_combined(ctx, OSSL_KEYMGMT_VALIDATE_FULL_CHECK);
}

int EVP_PKEY_param_check_quick(EVP_PKEY_CTX *ctx)
{
    return evp_pkey_param_check_combined(ctx, OSSL_KEYMGMT_VALIDATE_QUICK_CHECK);
}

int EVP_PKEY_private_check(EVP_PKEY_CTX *ctx)
{
    EVP_PKEY *pkey = ctx->pkey;
    int ok;

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        return 0;
    }

    if ((ok = try_provided_check(ctx, OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
                                 OSSL_KEYMGMT_VALIDATE_FULL_CHECK)) != -1)
        return ok;

    /* not supported for legacy keys */
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return -2;
}

int EVP_PKEY_check(EVP_PKEY_CTX *ctx)
{
    return EVP_PKEY_pairwise_check(ctx);
}

int EVP_PKEY_pairwise_check(EVP_PKEY_CTX *ctx)
{
    EVP_PKEY *pkey = ctx->pkey;
    int ok;

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_NO_KEY_SET);
        return 0;
    }

    if ((ok = try_provided_check(ctx, OSSL_KEYMGMT_SELECT_KEYPAIR,
                                 OSSL_KEYMGMT_VALIDATE_FULL_CHECK)) != -1)
        return ok;

    if (pkey->type == EVP_PKEY_NONE)
        goto not_supported;

#ifndef FIPS_MODULE
    /* legacy */
    /* call customized check function first */
    if (ctx->pmeth->check != NULL)
        return ctx->pmeth->check(pkey);

    /* use default check function in ameth */
    if (pkey->ameth == NULL || pkey->ameth->pkey_check == NULL)
        goto not_supported;

    return pkey->ameth->pkey_check(pkey);
#endif
 not_supported:
    ERR_raise(ERR_LIB_EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return -2;
}

