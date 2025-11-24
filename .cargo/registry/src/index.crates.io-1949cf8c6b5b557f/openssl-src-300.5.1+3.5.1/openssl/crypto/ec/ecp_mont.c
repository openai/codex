/*
 * Copyright 2001-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low-level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/err.h>

#include "ec_local.h"

const EC_METHOD *EC_GFp_mont_method(void)
{
    static const EC_METHOD ret = {
        EC_FLAGS_DEFAULT_OCT,
        NID_X9_62_prime_field,
        ossl_ec_GFp_mont_group_init,
        ossl_ec_GFp_mont_group_finish,
        ossl_ec_GFp_mont_group_clear_finish,
        ossl_ec_GFp_mont_group_copy,
        ossl_ec_GFp_mont_group_set_curve,
        ossl_ec_GFp_simple_group_get_curve,
        ossl_ec_GFp_simple_group_get_degree,
        ossl_ec_group_simple_order_bits,
        ossl_ec_GFp_simple_group_check_discriminant,
        ossl_ec_GFp_simple_point_init,
        ossl_ec_GFp_simple_point_finish,
        ossl_ec_GFp_simple_point_clear_finish,
        ossl_ec_GFp_simple_point_copy,
        ossl_ec_GFp_simple_point_set_to_infinity,
        ossl_ec_GFp_simple_point_set_affine_coordinates,
        ossl_ec_GFp_simple_point_get_affine_coordinates,
        0, 0, 0,
        ossl_ec_GFp_simple_add,
        ossl_ec_GFp_simple_dbl,
        ossl_ec_GFp_simple_invert,
        ossl_ec_GFp_simple_is_at_infinity,
        ossl_ec_GFp_simple_is_on_curve,
        ossl_ec_GFp_simple_cmp,
        ossl_ec_GFp_simple_make_affine,
        ossl_ec_GFp_simple_points_make_affine,
        0 /* mul */ ,
        0 /* precompute_mult */ ,
        0 /* have_precompute_mult */ ,
        ossl_ec_GFp_mont_field_mul,
        ossl_ec_GFp_mont_field_sqr,
        0 /* field_div */ ,
        ossl_ec_GFp_mont_field_inv,
        ossl_ec_GFp_mont_field_encode,
        ossl_ec_GFp_mont_field_decode,
        ossl_ec_GFp_mont_field_set_to_one,
        ossl_ec_key_simple_priv2oct,
        ossl_ec_key_simple_oct2priv,
        0, /* set private */
        ossl_ec_key_simple_generate_key,
        ossl_ec_key_simple_check_key,
        ossl_ec_key_simple_generate_public_key,
        0, /* keycopy */
        0, /* keyfinish */
        ossl_ecdh_simple_compute_key,
        ossl_ecdsa_simple_sign_setup,
        ossl_ecdsa_simple_sign_sig,
        ossl_ecdsa_simple_verify_sig,
        0, /* field_inverse_mod_ord */
        ossl_ec_GFp_simple_blind_coordinates,
        ossl_ec_GFp_simple_ladder_pre,
        ossl_ec_GFp_simple_ladder_step,
        ossl_ec_GFp_simple_ladder_post
    };

    return &ret;
}

int ossl_ec_GFp_mont_group_init(EC_GROUP *group)
{
    int ok;

    ok = ossl_ec_GFp_simple_group_init(group);
    group->field_data1 = NULL;
    group->field_data2 = NULL;
    return ok;
}

void ossl_ec_GFp_mont_group_finish(EC_GROUP *group)
{
    BN_MONT_CTX_free(group->field_data1);
    group->field_data1 = NULL;
    BN_free(group->field_data2);
    group->field_data2 = NULL;
    ossl_ec_GFp_simple_group_finish(group);
}

void ossl_ec_GFp_mont_group_clear_finish(EC_GROUP *group)
{
    BN_MONT_CTX_free(group->field_data1);
    group->field_data1 = NULL;
    BN_clear_free(group->field_data2);
    group->field_data2 = NULL;
    ossl_ec_GFp_simple_group_clear_finish(group);
}

int ossl_ec_GFp_mont_group_copy(EC_GROUP *dest, const EC_GROUP *src)
{
    BN_MONT_CTX_free(dest->field_data1);
    dest->field_data1 = NULL;
    BN_clear_free(dest->field_data2);
    dest->field_data2 = NULL;

    if (!ossl_ec_GFp_simple_group_copy(dest, src))
        return 0;

    if (src->field_data1 != NULL) {
        dest->field_data1 = BN_MONT_CTX_new();
        if (dest->field_data1 == NULL)
            return 0;
        if (!BN_MONT_CTX_copy(dest->field_data1, src->field_data1))
            goto err;
    }
    if (src->field_data2 != NULL) {
        dest->field_data2 = BN_dup(src->field_data2);
        if (dest->field_data2 == NULL)
            goto err;
    }

    return 1;

 err:
    BN_MONT_CTX_free(dest->field_data1);
    dest->field_data1 = NULL;
    return 0;
}

int ossl_ec_GFp_mont_group_set_curve(EC_GROUP *group, const BIGNUM *p,
                                     const BIGNUM *a, const BIGNUM *b,
                                     BN_CTX *ctx)
{
    BN_CTX *new_ctx = NULL;
    BN_MONT_CTX *mont = NULL;
    BIGNUM *one = NULL;
    int ret = 0;

    BN_MONT_CTX_free(group->field_data1);
    group->field_data1 = NULL;
    BN_free(group->field_data2);
    group->field_data2 = NULL;

    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new_ex(group->libctx);
        if (ctx == NULL)
            return 0;
    }

    mont = BN_MONT_CTX_new();
    if (mont == NULL)
        goto err;
    if (!BN_MONT_CTX_set(mont, p, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }
    one = BN_new();
    if (one == NULL)
        goto err;
    if (!BN_to_montgomery(one, BN_value_one(), mont, ctx))
        goto err;

    group->field_data1 = mont;
    mont = NULL;
    group->field_data2 = one;
    one = NULL;

    ret = ossl_ec_GFp_simple_group_set_curve(group, p, a, b, ctx);

    if (!ret) {
        BN_MONT_CTX_free(group->field_data1);
        group->field_data1 = NULL;
        BN_free(group->field_data2);
        group->field_data2 = NULL;
    }

 err:
    BN_free(one);
    BN_CTX_free(new_ctx);
    BN_MONT_CTX_free(mont);
    return ret;
}

int ossl_ec_GFp_mont_field_mul(const EC_GROUP *group, BIGNUM *r, const BIGNUM *a,
                               const BIGNUM *b, BN_CTX *ctx)
{
    if (group->field_data1 == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_NOT_INITIALIZED);
        return 0;
    }

    return BN_mod_mul_montgomery(r, a, b, group->field_data1, ctx);
}

int ossl_ec_GFp_mont_field_sqr(const EC_GROUP *group, BIGNUM *r, const BIGNUM *a,
                               BN_CTX *ctx)
{
    if (group->field_data1 == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_NOT_INITIALIZED);
        return 0;
    }

    return BN_mod_mul_montgomery(r, a, a, group->field_data1, ctx);
}

/*-
 * Computes the multiplicative inverse of a in GF(p), storing the result in r.
 * If a is zero (or equivalent), you'll get an EC_R_CANNOT_INVERT error.
 * We have a Mont structure, so SCA hardening is FLT inversion.
 */
int ossl_ec_GFp_mont_field_inv(const EC_GROUP *group, BIGNUM *r, const BIGNUM *a,
                               BN_CTX *ctx)
{
    BIGNUM *e = NULL;
    BN_CTX *new_ctx = NULL;
    int ret = 0;

    if (group->field_data1 == NULL)
        return 0;

    if (ctx == NULL
            && (ctx = new_ctx = BN_CTX_secure_new_ex(group->libctx)) == NULL)
        return 0;

    BN_CTX_start(ctx);
    if ((e = BN_CTX_get(ctx)) == NULL)
        goto err;

    /* Inverse in constant time with Fermats Little Theorem */
    if (!BN_set_word(e, 2))
        goto err;
    if (!BN_sub(e, group->field, e))
        goto err;
    /*-
     * Exponent e is public.
     * No need for scatter-gather or BN_FLG_CONSTTIME.
     */
    if (!BN_mod_exp_mont(r, a, e, group->field, ctx, group->field_data1))
        goto err;

    /* throw an error on zero */
    if (BN_is_zero(r)) {
        ERR_raise(ERR_LIB_EC, EC_R_CANNOT_INVERT);
        goto err;
    }

    ret = 1;

 err:
    BN_CTX_end(ctx);
    BN_CTX_free(new_ctx);
    return ret;
}

int ossl_ec_GFp_mont_field_encode(const EC_GROUP *group, BIGNUM *r,
                                  const BIGNUM *a, BN_CTX *ctx)
{
    if (group->field_data1 == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_NOT_INITIALIZED);
        return 0;
    }

    return BN_to_montgomery(r, a, (BN_MONT_CTX *)group->field_data1, ctx);
}

int ossl_ec_GFp_mont_field_decode(const EC_GROUP *group, BIGNUM *r,
                                  const BIGNUM *a, BN_CTX *ctx)
{
    if (group->field_data1 == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_NOT_INITIALIZED);
        return 0;
    }

    return BN_from_montgomery(r, a, group->field_data1, ctx);
}

int ossl_ec_GFp_mont_field_set_to_one(const EC_GROUP *group, BIGNUM *r,
                                      BN_CTX *ctx)
{
    if (group->field_data2 == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_NOT_INITIALIZED);
        return 0;
    }

    if (!BN_copy(r, group->field_data2))
        return 0;
    return 1;
}
