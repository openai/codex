/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/dsa.h>
#include <openssl/evp.h>
#include "crypto/evp.h"

static int dsa_paramgen_check(EVP_PKEY_CTX *ctx)
{
    if (ctx == NULL || !EVP_PKEY_CTX_IS_GEN_OP(ctx)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_COMMAND_NOT_SUPPORTED);
        /* Uses the same return values as EVP_PKEY_CTX_ctrl */
        return -2;
    }
    /* If key type not DSA return error */
    if (ctx->pmeth != NULL && ctx->pmeth->pkey_id != EVP_PKEY_DSA)
        return -1;
    return 1;
}

int EVP_PKEY_CTX_set_dsa_paramgen_type(EVP_PKEY_CTX *ctx, const char *name)
{
    int ret;
    OSSL_PARAM params[2], *p = params;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_FFC_TYPE,
                                            (char *)name, 0);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set_dsa_paramgen_gindex(EVP_PKEY_CTX *ctx, int gindex)
{
    int ret;
    OSSL_PARAM params[2], *p = params;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_int(OSSL_PKEY_PARAM_FFC_GINDEX, &gindex);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set_dsa_paramgen_seed(EVP_PKEY_CTX *ctx,
                                       const unsigned char *seed,
                                       size_t seedlen)
{
    int ret;
    OSSL_PARAM params[2], *p = params;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_FFC_SEED,
                                             (void *)seed, seedlen);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set_dsa_paramgen_bits(EVP_PKEY_CTX *ctx, int nbits)
{
    int ret;
    OSSL_PARAM params[2], *p = params;
    size_t bits = nbits;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_size_t(OSSL_PKEY_PARAM_FFC_PBITS, &bits);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set_dsa_paramgen_q_bits(EVP_PKEY_CTX *ctx, int qbits)
{
    int ret;
    OSSL_PARAM params[2], *p = params;
    size_t bits2 = qbits;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_size_t(OSSL_PKEY_PARAM_FFC_QBITS, &bits2);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

int EVP_PKEY_CTX_set_dsa_paramgen_md_props(EVP_PKEY_CTX *ctx,
                                           const char *md_name,
                                           const char *md_properties)
{
    int ret;
    OSSL_PARAM params[3], *p = params;

    if ((ret = dsa_paramgen_check(ctx)) <= 0)
        return ret;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST,
                                            (char *)md_name, 0);
    if (md_properties != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST_PROPS,
                                                (char *)md_properties, 0);
    *p++ = OSSL_PARAM_construct_end();

    return EVP_PKEY_CTX_set_params(ctx, params);
}

#if !defined(FIPS_MODULE)
int EVP_PKEY_CTX_set_dsa_paramgen_md(EVP_PKEY_CTX *ctx, const EVP_MD *md)
{
    return EVP_PKEY_CTX_ctrl(ctx, EVP_PKEY_DSA, EVP_PKEY_OP_PARAMGEN,
                             EVP_PKEY_CTRL_DSA_PARAMGEN_MD, 0, (void *)(md));
}
#endif
