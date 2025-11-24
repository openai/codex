/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include "dh_local.h"
#include "crypto/dh.h"

/*-
 * Check that p and g are suitable enough
 *
 * p is odd
 * 1 < g < p - 1
 */
int DH_check_params_ex(const DH *dh)
{
    int errflags = 0;

    if (!DH_check_params(dh, &errflags))
        return 0;

    if ((errflags & DH_CHECK_P_NOT_PRIME) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_P_NOT_PRIME);
    if ((errflags & DH_NOT_SUITABLE_GENERATOR) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_NOT_SUITABLE_GENERATOR);
    if ((errflags & DH_MODULUS_TOO_SMALL) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_SMALL);
    if ((errflags & DH_MODULUS_TOO_LARGE) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);

    return errflags == 0;
}

#ifdef FIPS_MODULE
int DH_check_params(const DH *dh, int *ret)
{
    int nid;

    *ret = 0;
    /*
     * SP800-56A R3 Section 5.5.2 Assurances of Domain Parameter Validity
     * (1a) The domain parameters correspond to any approved safe prime group.
     */
    nid = DH_get_nid((DH *)dh);
    if (nid != NID_undef)
        return 1;
    /*
     * OR
     * (2b) FFC domain params conform to FIPS-186-4 explicit domain param
     * validity tests.
     */
    return ossl_ffc_params_FIPS186_4_validate(dh->libctx, &dh->params,
                                              FFC_PARAM_TYPE_DH, ret, NULL);
}
#else
int DH_check_params(const DH *dh, int *ret)
{
    int ok = 0;
    BIGNUM *tmp = NULL;
    BN_CTX *ctx = NULL;

    *ret = 0;
    ctx = BN_CTX_new_ex(dh->libctx);
    if (ctx == NULL)
        goto err;
    BN_CTX_start(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL)
        goto err;

    if (!BN_is_odd(dh->params.p))
        *ret |= DH_CHECK_P_NOT_PRIME;
    if (BN_is_negative(dh->params.g)
        || BN_is_zero(dh->params.g)
        || BN_is_one(dh->params.g))
        *ret |= DH_NOT_SUITABLE_GENERATOR;
    if (BN_copy(tmp, dh->params.p) == NULL || !BN_sub_word(tmp, 1))
        goto err;
    if (BN_cmp(dh->params.g, tmp) >= 0)
        *ret |= DH_NOT_SUITABLE_GENERATOR;
    if (BN_num_bits(dh->params.p) < DH_MIN_MODULUS_BITS)
        *ret |= DH_MODULUS_TOO_SMALL;
    if (BN_num_bits(dh->params.p) > OPENSSL_DH_MAX_MODULUS_BITS)
        *ret |= DH_MODULUS_TOO_LARGE;

    ok = 1;
 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ok;
}
#endif /* FIPS_MODULE */

/*-
 * Check that p is a safe prime and
 * g is a suitable generator.
 */
int DH_check_ex(const DH *dh)
{
    int errflags = 0;

    if (!DH_check(dh, &errflags))
        return 0;

    if ((errflags & DH_NOT_SUITABLE_GENERATOR) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_NOT_SUITABLE_GENERATOR);
    if ((errflags & DH_CHECK_Q_NOT_PRIME) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_Q_NOT_PRIME);
    if ((errflags & DH_CHECK_INVALID_Q_VALUE) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_INVALID_Q_VALUE);
    if ((errflags & DH_CHECK_INVALID_J_VALUE) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_INVALID_J_VALUE);
    if ((errflags & DH_UNABLE_TO_CHECK_GENERATOR) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_UNABLE_TO_CHECK_GENERATOR);
    if ((errflags & DH_CHECK_P_NOT_PRIME) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_P_NOT_PRIME);
    if ((errflags & DH_CHECK_P_NOT_SAFE_PRIME) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_P_NOT_SAFE_PRIME);
    if ((errflags & DH_MODULUS_TOO_SMALL) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_SMALL);
    if ((errflags & DH_MODULUS_TOO_LARGE) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);

    return errflags == 0;
}

/* Note: according to documentation - this only checks the params */
int DH_check(const DH *dh, int *ret)
{
#ifdef FIPS_MODULE
    return DH_check_params(dh, ret);
#else
    int ok = 0, r, q_good = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *t1 = NULL, *t2 = NULL;
    int nid = DH_get_nid((DH *)dh);

    *ret = 0;
    if (nid != NID_undef)
        return 1;

    /* Don't do any checks at all with an excessively large modulus */
    if (BN_num_bits(dh->params.p) > OPENSSL_DH_CHECK_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);
        *ret = DH_MODULUS_TOO_LARGE | DH_CHECK_P_NOT_PRIME;
        return 0;
    }

    if (!DH_check_params(dh, ret))
        return 0;

    ctx = BN_CTX_new_ex(dh->libctx);
    if (ctx == NULL)
        goto err;
    BN_CTX_start(ctx);
    t1 = BN_CTX_get(ctx);
    t2 = BN_CTX_get(ctx);
    if (t2 == NULL)
        goto err;

    if (dh->params.q != NULL) {
        if (BN_ucmp(dh->params.p, dh->params.q) > 0)
            q_good = 1;
        else
            *ret |= DH_CHECK_INVALID_Q_VALUE;
    }

    if (q_good) {
        if (BN_cmp(dh->params.g, BN_value_one()) <= 0)
            *ret |= DH_NOT_SUITABLE_GENERATOR;
        else if (BN_cmp(dh->params.g, dh->params.p) >= 0)
            *ret |= DH_NOT_SUITABLE_GENERATOR;
        else {
            /* Check g^q == 1 mod p */
            if (!BN_mod_exp(t1, dh->params.g, dh->params.q, dh->params.p, ctx))
                goto err;
            if (!BN_is_one(t1))
                *ret |= DH_NOT_SUITABLE_GENERATOR;
        }
        r = BN_check_prime(dh->params.q, ctx, NULL);
        if (r < 0)
            goto err;
        if (!r)
            *ret |= DH_CHECK_Q_NOT_PRIME;
        /* Check p == 1 mod q  i.e. q divides p - 1 */
        if (!BN_div(t1, t2, dh->params.p, dh->params.q, ctx))
            goto err;
        if (!BN_is_one(t2))
            *ret |= DH_CHECK_INVALID_Q_VALUE;
        if (dh->params.j != NULL
            && BN_cmp(dh->params.j, t1))
            *ret |= DH_CHECK_INVALID_J_VALUE;
    }

    r = BN_check_prime(dh->params.p, ctx, NULL);
    if (r < 0)
        goto err;
    if (!r)
        *ret |= DH_CHECK_P_NOT_PRIME;
    else if (dh->params.q == NULL) {
        if (!BN_rshift1(t1, dh->params.p))
            goto err;
        r = BN_check_prime(t1, ctx, NULL);
        if (r < 0)
            goto err;
        if (!r)
            *ret |= DH_CHECK_P_NOT_SAFE_PRIME;
    }
    ok = 1;
 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ok;
#endif /* FIPS_MODULE */
}

int DH_check_pub_key_ex(const DH *dh, const BIGNUM *pub_key)
{
    int errflags = 0;

    if (!DH_check_pub_key(dh, pub_key, &errflags))
        return 0;

    if ((errflags & DH_CHECK_PUBKEY_TOO_SMALL) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_PUBKEY_TOO_SMALL);
    if ((errflags & DH_CHECK_PUBKEY_TOO_LARGE) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_PUBKEY_TOO_LARGE);
    if ((errflags & DH_CHECK_PUBKEY_INVALID) != 0)
        ERR_raise(ERR_LIB_DH, DH_R_CHECK_PUBKEY_INVALID);

    return errflags == 0;
}

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Full public key validation.
 */
int DH_check_pub_key(const DH *dh, const BIGNUM *pub_key, int *ret)
{
    /* Don't do any checks at all with an excessively large modulus */
    if (BN_num_bits(dh->params.p) > OPENSSL_DH_CHECK_MAX_MODULUS_BITS) {
        ERR_raise(ERR_LIB_DH, DH_R_MODULUS_TOO_LARGE);
        *ret = DH_MODULUS_TOO_LARGE | DH_CHECK_PUBKEY_INVALID;
        return 0;
    }

    if (dh->params.q != NULL && BN_ucmp(dh->params.p, dh->params.q) < 0) {
        *ret |= DH_CHECK_INVALID_Q_VALUE | DH_CHECK_PUBKEY_INVALID;
        return 1;
    }

    return ossl_ffc_validate_public_key(&dh->params, pub_key, ret);
}

/*
 * See SP800-56Ar3 Section 5.6.2.3.1 : FFC Partial public key validation.
 * To only be used with ephemeral FFC public keys generated using the approved
 * safe-prime groups.
 */
int ossl_dh_check_pub_key_partial(const DH *dh, const BIGNUM *pub_key, int *ret)
{
    return ossl_ffc_validate_public_key_partial(&dh->params, pub_key, ret)
           && *ret == 0;
}

int ossl_dh_check_priv_key(const DH *dh, const BIGNUM *priv_key, int *ret)
{
    int ok = 0;
    BIGNUM *two_powN = NULL, *upper;

    *ret = 0;
    two_powN = BN_new();
    if (two_powN == NULL)
        return 0;

    if (dh->params.q != NULL) {
        upper = dh->params.q;
#ifndef FIPS_MODULE
    } else if (dh->params.p != NULL) {
        /*
         * We do not have q so we just check the key is within some
         * reasonable range, or the number of bits is equal to dh->length.
         */
        int length = dh->length;

        if (length == 0) {
            length = BN_num_bits(dh->params.p) - 1;
            if (BN_num_bits(priv_key) <= length
                && BN_num_bits(priv_key) > 1)
                ok = 1;
        } else if (BN_num_bits(priv_key) == length) {
            ok = 1;
        }
        goto end;
#endif
    } else {
        goto end;
    }

    /* Is it from an approved Safe prime group ?*/
    if (DH_get_nid((DH *)dh) != NID_undef && dh->length != 0) {
        if (!BN_lshift(two_powN, BN_value_one(), dh->length))
            goto end;
        if (BN_cmp(two_powN, dh->params.q) < 0)
            upper = two_powN;
    }
    if (!ossl_ffc_validate_private_key(upper, priv_key, ret))
        goto end;

    ok = 1;
end:
    BN_free(two_powN);
    return ok;
}

/*
 * FFC pairwise check from SP800-56A R3.
 *    Section 5.6.2.1.4 Owner Assurance of Pair-wise Consistency
 */
int ossl_dh_check_pairwise(const DH *dh)
{
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *pub_key = NULL;

    if (dh->params.p == NULL
        || dh->params.g == NULL
        || dh->priv_key == NULL
        || dh->pub_key == NULL)
        return 0;

    ctx = BN_CTX_new_ex(dh->libctx);
    if (ctx == NULL)
        goto err;
    pub_key = BN_new();
    if (pub_key == NULL)
        goto err;

    /* recalculate the public key = (g ^ priv) mod p */
    if (!ossl_dh_generate_public_key(ctx, dh, dh->priv_key, pub_key))
        goto err;
    /* check it matches the existing public_key */
    ret = BN_cmp(pub_key, dh->pub_key) == 0;
err:
    BN_free(pub_key);
    BN_CTX_free(ctx);
    return ret;
}
