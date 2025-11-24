/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/ec.h>
#include "crypto/evp.h"
#include "crypto/ec.h"

/*
 * This file is meant to contain functions to provide EVP_PKEY support for EC
 * keys.
 */

static ossl_inline
int evp_pkey_ctx_getset_ecdh_param_checks(const EVP_PKEY_CTX *ctx)
{
    if (ctx == NULL || !EVP_PKEY_CTX_IS_DERIVE_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }

    /* If key type not EC return error */
    if (evp_pkey_ctx_is_legacy(ctx)
        && ctx->pmeth != NULL && ctx->pmeth->pkey_id != EVP_PKEY_EC)
        return -1;

    return 1;
}

int EVP_PKEY_CTX_set_ecdh_cofactor_mode(EVP_PKEY_CTX *ctx, int cofactor_mode)
{
    int ret;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    /*
     * Valid input values are:
     *  * 0 for disable
     *  * 1 for enable
     *  * -1 for reset to default for associated priv key
     */
    if (cofactor_mode < -1 || cofactor_mode > 1) {
        /* Uses the same return value of pkey_ec_ctrl() */
        return -2;
    }

    *p++ = OSSL_PARAM_construct_int(OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE,
                                    &cofactor_mode);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_set_params_strict(ctx, params);
    if (ret == -2)
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
    return ret;
}

int EVP_PKEY_CTX_get_ecdh_cofactor_mode(EVP_PKEY_CTX *ctx)
{
    int ret, mode;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    *p++ = OSSL_PARAM_construct_int(OSSL_EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE,
                                    &mode);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_get_params_strict(ctx, params);

    switch (ret) {
    case -2:
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        break;
    case 1:
        ret = mode;
        if (mode < 0 || mode > 1) {
            /*
             * The provider should return either 0 or 1, any other value is a
             * provider error.
             */
            ret = -1;
        }
        break;
    default:
        ret = -1;
        break;
    }

    return ret;
}

/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 */
int EVP_PKEY_CTX_set_ecdh_kdf_type(EVP_PKEY_CTX *ctx, int kdf)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_EC, EVP_PKEY_OP_DERIVE,
                             EVP_PKEY_CTRL_EC_KDF_TYPE, kdf, NULL);
}

/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 */
int EVP_PKEY_CTX_get_ecdh_kdf_type(EVP_PKEY_CTX *ctx)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_EC, EVP_PKEY_OP_DERIVE,
                             EVP_PKEY_CTRL_EC_KDF_TYPE, -2, NULL);
}

/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 */
int EVP_PKEY_CTX_set_ecdh_kdf_md(EVP_PKEY_CTX *ctx, const EVP_MD *md)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_EC, EVP_PKEY_OP_DERIVE,
                             EVP_PKEY_CTRL_EC_KDF_MD, 0, (void *)(md));
}

/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 */
int EVP_PKEY_CTX_get_ecdh_kdf_md(EVP_PKEY_CTX *ctx, const EVP_MD **pmd)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_EC, EVP_PKEY_OP_DERIVE,
                             EVP_PKEY_CTRL_GET_EC_KDF_MD, 0, (void *)(pmd));
}

int EVP_PKEY_CTX_set_ecdh_kdf_outlen(EVP_PKEY_CTX *ctx, int outlen)
{
    int ret;
    size_t len = outlen;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    if (outlen <= 0) {
        /*
         * This would ideally be -1 or 0, but we have to retain compatibility
         * with legacy behaviour of EVP_PKEY_CTX_ctrl() which returned -2 if
         * in <= 0
         */
        return -2;
    }

    *p++ = OSSL_PARAM_construct_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN,
                                       &len);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_set_params_strict(ctx, params);
    if (ret == -2)
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
    return ret;
}

int EVP_PKEY_CTX_get_ecdh_kdf_outlen(EVP_PKEY_CTX *ctx, int *plen)
{
    size_t len = UINT_MAX;
    int ret;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    *p++ = OSSL_PARAM_construct_size_t(OSSL_EXCHANGE_PARAM_KDF_OUTLEN,
                                       &len);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_get_params_strict(ctx, params);

    switch (ret) {
    case -2:
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        break;
    case 1:
        if (len <= INT_MAX)
            *plen = (int)len;
        else
            ret = -1;
        break;
    default:
        ret = -1;
        break;
    }

    return ret;
}

int EVP_PKEY_CTX_set0_ecdh_kdf_ukm(EVP_PKEY_CTX *ctx, unsigned char *ukm, int len)
{
    int ret;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_EXCHANGE_PARAM_KDF_UKM,
                                            /*
                                             * Cast away the const. This is read
                                             * only so should be safe
                                             */
                                            (void *)ukm,
                                            (size_t)len);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_set_params_strict(ctx, params);

    switch (ret) {
    case -2:
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        break;
    case 1:
        OPENSSL_free(ukm);
        break;
    }

    return ret;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EVP_PKEY_CTX_get0_ecdh_kdf_ukm(EVP_PKEY_CTX *ctx, unsigned char **pukm)
{
    size_t ukmlen;
    int ret;
    OSSL_PARAM params[2], *p = params;

    ret = evp_pkey_ctx_getset_ecdh_param_checks(ctx);
    if (ret != 1)
        return ret;

    *p++ = OSSL_PARAM_construct_octet_ptr(OSSL_EXCHANGE_PARAM_KDF_UKM,
                                          (void **)pukm, 0);
    *p++ = OSSL_PARAM_construct_end();

    ret = evp_pkey_ctx_get_params_strict(ctx, params);

    switch (ret) {
    case -2:
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        break;
    case 1:
        ret = -1;
        ukmlen = params[0].return_size;
        if (ukmlen <= INT_MAX)
            ret = (int)ukmlen;
        break;
    default:
        ret = -1;
        break;
    }

    return ret;
}
#endif

#ifndef FIPS_MODULE
/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 * ASN1_OBJECT (which would be converted to text internally)?
 */
int EVP_PKEY_CTX_set_ec_paramgen_curve_nid(EVP_PKEY_CTX *ctx, int nid)
{
    int keytype = nid == EVP_PKEY_SM2 ? EVP_PKEY_SM2 : EVP_PKEY_EC;

    return EVP_PKEY_CTX_ctrl(ctx, keytype, EVP_PKEY_OP_TYPE_GEN,
                             EVP_PKEY_CTRL_EC_PARAMGEN_CURVE_NID,
                             nid, NULL);
}

/*
 * This one is currently implemented as an EVP_PKEY_CTX_ctrl() wrapper,
 * simply because that's easier.
 */
int EVP_PKEY_CTX_set_ec_param_enc(EVP_PKEY_CTX *ctx, int param_enc)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_EC, EVP_PKEY_OP_TYPE_GEN,
                             EVP_PKEY_CTRL_EC_PARAM_ENC, param_enc, NULL);
}
#endif
