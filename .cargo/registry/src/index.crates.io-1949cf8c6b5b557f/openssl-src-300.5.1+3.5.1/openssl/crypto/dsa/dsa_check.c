/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include "dsa_local.h"
#include "crypto/dsa.h"

static int dsa_precheck_params(const DSA *dsa, int *ret)
{
    if (dsa->params.p == NULL || dsa->params.q == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BAD_FFC_PARAMETERS);
        *ret = FFC_CHECK_INVALID_PQ;
        return 0;
    }

    if (BN_num_bits(dsa->params.p) > OPENSSL_DSA_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MODULUS_TOO_LARGE);
        *ret = FFC_CHECK_INVALID_PQ;
        return 0;
    }

    if (BN_num_bits(dsa->params.q) >= BN_num_bits(dsa->params.p)) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BAD_Q_VALUE);
        *ret = FFC_CHECK_INVALID_PQ;
        return 0;
    }

    return 1;
}

int ossl_dsa_check_params(const DSA *dsa, int checktype, int *ret)
{
    if (!dsa_precheck_params(dsa, ret))
        return 0;

    if (checktype == OSSL_KEYMGMT_VALIDATE_QUICK_CHECK)
        return ossl_ffc_params_simple_validate(dsa->libctx, &dsa->params,
                                               FFC_PARAM_TYPE_DSA, ret);
    else
        /*
         * Do full FFC domain params validation according to FIPS-186-4
         *  - always in FIPS_MODULE
         *  - only if possible (i.e., seed is set) in default provider
         */
        return ossl_ffc_params_full_validate(dsa->libctx, &dsa->params,
                                             FFC_PARAM_TYPE_DSA, ret);
}

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Full public key validation.
 */
int ossl_dsa_check_pub_key(const DSA *dsa, const BIGNUM *pub_key, int *ret)
{
    if (!dsa_precheck_params(dsa, ret))
        return 0;

    return ossl_ffc_validate_public_key(&dsa->params, pub_key, ret)
           && *ret == 0;
}

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Partial public key validation.
 * To only be used with ephemeral FFC public keys generated using the approved
 * safe-prime groups.
 */
int ossl_dsa_check_pub_key_partial(const DSA *dsa, const BIGNUM *pub_key, int *ret)
{
    if (!dsa_precheck_params(dsa, ret))
        return 0;

    return ossl_ffc_validate_public_key_partial(&dsa->params, pub_key, ret)
           && *ret == 0;
}

int ossl_dsa_check_priv_key(const DSA *dsa, const BIGNUM *priv_key, int *ret)
{
    *ret = 0;

    if (!dsa_precheck_params(dsa, ret))
        return 0;

    return ossl_ffc_validate_private_key(dsa->params.q, priv_key, ret);
}

/*
 * FFC pairwise check from SP800-56A R3.
 *    Section 5.6.2.1.4 Owner Assurance of Pair-wise Consistency
 */
int ossl_dsa_check_pairwise(const DSA *dsa)
{
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *pub_key = NULL;

    if (!dsa_precheck_params(dsa, &ret))
        return 0;

    if (dsa->params.g == NULL
        || dsa->priv_key == NULL
        || dsa->pub_key == NULL)
        return 0;

    ctx = BN_CTX_new_ex(dsa->libctx);
    if (ctx == NULL)
        goto err;
    pub_key = BN_new();
    if (pub_key == NULL)
        goto err;

    /* recalculate the public key = (g ^ priv) mod p */
    if (!ossl_dsa_generate_public_key(ctx, dsa, dsa->priv_key, pub_key))
        goto err;
    /* check it matches the existing public_key */
    ret = BN_cmp(pub_key, dsa->pub_key) == 0;
err:
    BN_free(pub_key);
    BN_CTX_free(ctx);
    return ret;
}
