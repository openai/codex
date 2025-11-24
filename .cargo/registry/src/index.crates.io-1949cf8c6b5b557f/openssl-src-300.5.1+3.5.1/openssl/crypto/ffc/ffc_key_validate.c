/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/ffc.h"

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Partial public key validation.
 * To only be used with ephemeral FFC public keys generated using the approved
 * safe-prime groups. (Checks that the public key is in the range [2, p - 1]
 *
 * ret contains 0 on success, or error flags (see FFC_ERROR_PUBKEY_TOO_SMALL)
 */
int ossl_ffc_validate_public_key_partial(const FFC_PARAMS *params,
                                         const BIGNUM *pub_key, int *ret)
{
    int ok = 0;
    BIGNUM *tmp = NULL;
    BN_CTX *ctx = NULL;

    *ret = 0;
    if (params == NULL || pub_key == NULL || params->p == NULL) {
        *ret = FFC_ERROR_PASSED_NULL_PARAM;
        return 1;
    }

    ctx = BN_CTX_new_ex(NULL);
    if (ctx == NULL)
        goto err;

    BN_CTX_start(ctx);
    tmp = BN_CTX_get(ctx);
    /* Step(1): Verify pub_key >= 2 */
    if (tmp == NULL
        || !BN_set_word(tmp, 1))
        goto err;
    if (BN_cmp(pub_key, tmp) <= 0)
        *ret |= FFC_ERROR_PUBKEY_TOO_SMALL;
    /* Step(1): Verify pub_key <=  p-2 */
    if (BN_copy(tmp, params->p) == NULL
        || !BN_sub_word(tmp, 1))
        goto err;
    if (BN_cmp(pub_key, tmp) >= 0)
        *ret |= FFC_ERROR_PUBKEY_TOO_LARGE;
    ok = 1;
 err:
    if (ctx != NULL) {
        BN_CTX_end(ctx);
        BN_CTX_free(ctx);
    }
    return ok;
}

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Full public key validation.
 */
int ossl_ffc_validate_public_key(const FFC_PARAMS *params,
                                 const BIGNUM *pub_key, int *ret)
{
    int ok = 0;
    BIGNUM *tmp = NULL;
    BN_CTX *ctx = NULL;

    if (!ossl_ffc_validate_public_key_partial(params, pub_key, ret))
        return 0;

    if (*ret == 0 && params->q != NULL) {
        ctx = BN_CTX_new_ex(NULL);
        if (ctx == NULL)
            goto err;
        BN_CTX_start(ctx);
        tmp = BN_CTX_get(ctx);

        /* Check pub_key^q == 1 mod p */
        if (tmp == NULL
            || !BN_mod_exp(tmp, pub_key, params->q, params->p, ctx))
            goto err;
        if (!BN_is_one(tmp))
            *ret |= FFC_ERROR_PUBKEY_INVALID;
    }

    ok = 1;
 err:
    if (ctx != NULL) {
        BN_CTX_end(ctx);
        BN_CTX_free(ctx);
    }
    return ok;
}

/*
 * See SP800-56Ar3 Section 5.6.2.1.2: Owner assurance of Private key validity.
 * Verifies priv_key is in the range [1..upper-1]. The passed in value of upper
 * is normally params->q but can be 2^N for approved safe prime groups.
 * Note: This assumes that the domain parameters are valid.
 */
int ossl_ffc_validate_private_key(const BIGNUM *upper, const BIGNUM *priv,
                                  int *ret)
{
    int ok = 0;

    *ret = 0;

    if (priv == NULL || upper == NULL) {
        *ret = FFC_ERROR_PASSED_NULL_PARAM;
        goto err;
    }
    if (BN_cmp(priv, BN_value_one()) < 0) {
        *ret |= FFC_ERROR_PRIVKEY_TOO_SMALL;
        goto err;
    }
    if (BN_cmp(priv, upper) >= 0) {
        *ret |= FFC_ERROR_PRIVKEY_TOO_LARGE;
        goto err;
    }
    ok = 1;
err:
    return ok;
}
