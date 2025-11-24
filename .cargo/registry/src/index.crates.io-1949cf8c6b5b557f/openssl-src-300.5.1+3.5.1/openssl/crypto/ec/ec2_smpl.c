/*
 * Copyright 2002-2021 The OpenSSL Project Authors. All Rights Reserved.
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

#include "crypto/bn.h"
#include "ec_local.h"

#ifndef OPENSSL_NO_EC2M

/*
 * Initialize a GF(2^m)-based EC_GROUP structure. Note that all other members
 * are handled by EC_GROUP_new.
 */
int ossl_ec_GF2m_simple_group_init(EC_GROUP *group)
{
    group->field = BN_new();
    group->a = BN_new();
    group->b = BN_new();

    if (group->field == NULL || group->a == NULL || group->b == NULL) {
        BN_free(group->field);
        BN_free(group->a);
        BN_free(group->b);
        return 0;
    }
    return 1;
}

/*
 * Free a GF(2^m)-based EC_GROUP structure. Note that all other members are
 * handled by EC_GROUP_free.
 */
void ossl_ec_GF2m_simple_group_finish(EC_GROUP *group)
{
    BN_free(group->field);
    BN_free(group->a);
    BN_free(group->b);
}

/*
 * Clear and free a GF(2^m)-based EC_GROUP structure. Note that all other
 * members are handled by EC_GROUP_clear_free.
 */
void ossl_ec_GF2m_simple_group_clear_finish(EC_GROUP *group)
{
    BN_clear_free(group->field);
    BN_clear_free(group->a);
    BN_clear_free(group->b);
    group->poly[0] = 0;
    group->poly[1] = 0;
    group->poly[2] = 0;
    group->poly[3] = 0;
    group->poly[4] = 0;
    group->poly[5] = -1;
}

/*
 * Copy a GF(2^m)-based EC_GROUP structure. Note that all other members are
 * handled by EC_GROUP_copy.
 */
int ossl_ec_GF2m_simple_group_copy(EC_GROUP *dest, const EC_GROUP *src)
{
    if (!BN_copy(dest->field, src->field))
        return 0;
    if (!BN_copy(dest->a, src->a))
        return 0;
    if (!BN_copy(dest->b, src->b))
        return 0;
    dest->poly[0] = src->poly[0];
    dest->poly[1] = src->poly[1];
    dest->poly[2] = src->poly[2];
    dest->poly[3] = src->poly[3];
    dest->poly[4] = src->poly[4];
    dest->poly[5] = src->poly[5];
    if (bn_wexpand(dest->a, (int)(dest->poly[0] + BN_BITS2 - 1) / BN_BITS2) ==
        NULL)
        return 0;
    if (bn_wexpand(dest->b, (int)(dest->poly[0] + BN_BITS2 - 1) / BN_BITS2) ==
        NULL)
        return 0;
    bn_set_all_zero(dest->a);
    bn_set_all_zero(dest->b);
    return 1;
}

/* Set the curve parameters of an EC_GROUP structure. */
int ossl_ec_GF2m_simple_group_set_curve(EC_GROUP *group,
                                        const BIGNUM *p, const BIGNUM *a,
                                        const BIGNUM *b, BN_CTX *ctx)
{
    int ret = 0, i;

    /* group->field */
    if (!BN_copy(group->field, p))
        goto err;
    i = BN_GF2m_poly2arr(group->field, group->poly, 6) - 1;
    if ((i != 5) && (i != 3)) {
        ERR_raise(ERR_LIB_EC, EC_R_UNSUPPORTED_FIELD);
        goto err;
    }

    /* group->a */
    if (!BN_GF2m_mod_arr(group->a, a, group->poly))
        goto err;
    if (bn_wexpand(group->a, (int)(group->poly[0] + BN_BITS2 - 1) / BN_BITS2)
        == NULL)
        goto err;
    bn_set_all_zero(group->a);

    /* group->b */
    if (!BN_GF2m_mod_arr(group->b, b, group->poly))
        goto err;
    if (bn_wexpand(group->b, (int)(group->poly[0] + BN_BITS2 - 1) / BN_BITS2)
        == NULL)
        goto err;
    bn_set_all_zero(group->b);

    ret = 1;
 err:
    return ret;
}

/*
 * Get the curve parameters of an EC_GROUP structure. If p, a, or b are NULL
 * then there values will not be set but the method will return with success.
 */
int ossl_ec_GF2m_simple_group_get_curve(const EC_GROUP *group, BIGNUM *p,
                                        BIGNUM *a, BIGNUM *b, BN_CTX *ctx)
{
    int ret = 0;

    if (p != NULL) {
        if (!BN_copy(p, group->field))
            return 0;
    }

    if (a != NULL) {
        if (!BN_copy(a, group->a))
            goto err;
    }

    if (b != NULL) {
        if (!BN_copy(b, group->b))
            goto err;
    }

    ret = 1;

 err:
    return ret;
}

/*
 * Gets the degree of the field.  For a curve over GF(2^m) this is the value
 * m.
 */
int ossl_ec_GF2m_simple_group_get_degree(const EC_GROUP *group)
{
    return BN_num_bits(group->field) - 1;
}

/*
 * Checks the discriminant of the curve. y^2 + x*y = x^3 + a*x^2 + b is an
 * elliptic curve <=> b != 0 (mod p)
 */
int ossl_ec_GF2m_simple_group_check_discriminant(const EC_GROUP *group,
                                                 BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *b;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;

    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new();
        if (ctx == NULL) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
    }
#endif
    BN_CTX_start(ctx);
    b = BN_CTX_get(ctx);
    if (b == NULL)
        goto err;

    if (!BN_GF2m_mod_arr(b, group->b, group->poly))
        goto err;

    /*
     * check the discriminant: y^2 + x*y = x^3 + a*x^2 + b is an elliptic
     * curve <=> b != 0 (mod p)
     */
    if (BN_is_zero(b))
        goto err;

    ret = 1;

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/* Initializes an EC_POINT. */
int ossl_ec_GF2m_simple_point_init(EC_POINT *point)
{
    point->X = BN_new();
    point->Y = BN_new();
    point->Z = BN_new();

    if (point->X == NULL || point->Y == NULL || point->Z == NULL) {
        BN_free(point->X);
        BN_free(point->Y);
        BN_free(point->Z);
        return 0;
    }
    return 1;
}

/* Frees an EC_POINT. */
void ossl_ec_GF2m_simple_point_finish(EC_POINT *point)
{
    BN_free(point->X);
    BN_free(point->Y);
    BN_free(point->Z);
}

/* Clears and frees an EC_POINT. */
void ossl_ec_GF2m_simple_point_clear_finish(EC_POINT *point)
{
    BN_clear_free(point->X);
    BN_clear_free(point->Y);
    BN_clear_free(point->Z);
    point->Z_is_one = 0;
}

/*
 * Copy the contents of one EC_POINT into another.  Assumes dest is
 * initialized.
 */
int ossl_ec_GF2m_simple_point_copy(EC_POINT *dest, const EC_POINT *src)
{
    if (!BN_copy(dest->X, src->X))
        return 0;
    if (!BN_copy(dest->Y, src->Y))
        return 0;
    if (!BN_copy(dest->Z, src->Z))
        return 0;
    dest->Z_is_one = src->Z_is_one;
    dest->curve_name = src->curve_name;

    return 1;
}

/*
 * Set an EC_POINT to the point at infinity. A point at infinity is
 * represented by having Z=0.
 */
int ossl_ec_GF2m_simple_point_set_to_infinity(const EC_GROUP *group,
                                              EC_POINT *point)
{
    point->Z_is_one = 0;
    BN_zero(point->Z);
    return 1;
}

/*
 * Set the coordinates of an EC_POINT using affine coordinates. Note that
 * the simple implementation only uses affine coordinates.
 */
int ossl_ec_GF2m_simple_point_set_affine_coordinates(const EC_GROUP *group,
                                                     EC_POINT *point,
                                                     const BIGNUM *x,
                                                     const BIGNUM *y,
                                                     BN_CTX *ctx)
{
    int ret = 0;
    if (x == NULL || y == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (!BN_copy(point->X, x))
        goto err;
    BN_set_negative(point->X, 0);
    if (!BN_copy(point->Y, y))
        goto err;
    BN_set_negative(point->Y, 0);
    if (!BN_copy(point->Z, BN_value_one()))
        goto err;
    BN_set_negative(point->Z, 0);
    point->Z_is_one = 1;
    ret = 1;

 err:
    return ret;
}

/*
 * Gets the affine coordinates of an EC_POINT. Note that the simple
 * implementation only uses affine coordinates.
 */
int ossl_ec_GF2m_simple_point_get_affine_coordinates(const EC_GROUP *group,
                                                     const EC_POINT *point,
                                                     BIGNUM *x, BIGNUM *y,
                                                     BN_CTX *ctx)
{
    int ret = 0;

    if (EC_POINT_is_at_infinity(group, point)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_AT_INFINITY);
        return 0;
    }

    if (BN_cmp(point->Z, BN_value_one())) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (x != NULL) {
        if (!BN_copy(x, point->X))
            goto err;
        BN_set_negative(x, 0);
    }
    if (y != NULL) {
        if (!BN_copy(y, point->Y))
            goto err;
        BN_set_negative(y, 0);
    }
    ret = 1;

 err:
    return ret;
}

/*
 * Computes a + b and stores the result in r.  r could be a or b, a could be
 * b. Uses algorithm A.10.2 of IEEE P1363.
 */
int ossl_ec_GF2m_simple_add(const EC_GROUP *group, EC_POINT *r,
                            const EC_POINT *a, const EC_POINT *b, BN_CTX *ctx)
{
    BIGNUM *x0, *y0, *x1, *y1, *x2, *y2, *s, *t;
    int ret = 0;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (EC_POINT_is_at_infinity(group, a)) {
        if (!EC_POINT_copy(r, b))
            return 0;
        return 1;
    }

    if (EC_POINT_is_at_infinity(group, b)) {
        if (!EC_POINT_copy(r, a))
            return 0;
        return 1;
    }

#ifndef FIPS_MODULE
    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new();
        if (ctx == NULL)
            return 0;
    }
#endif

    BN_CTX_start(ctx);
    x0 = BN_CTX_get(ctx);
    y0 = BN_CTX_get(ctx);
    x1 = BN_CTX_get(ctx);
    y1 = BN_CTX_get(ctx);
    x2 = BN_CTX_get(ctx);
    y2 = BN_CTX_get(ctx);
    s = BN_CTX_get(ctx);
    t = BN_CTX_get(ctx);
    if (t == NULL)
        goto err;

    if (a->Z_is_one) {
        if (!BN_copy(x0, a->X))
            goto err;
        if (!BN_copy(y0, a->Y))
            goto err;
    } else {
        if (!EC_POINT_get_affine_coordinates(group, a, x0, y0, ctx))
            goto err;
    }
    if (b->Z_is_one) {
        if (!BN_copy(x1, b->X))
            goto err;
        if (!BN_copy(y1, b->Y))
            goto err;
    } else {
        if (!EC_POINT_get_affine_coordinates(group, b, x1, y1, ctx))
            goto err;
    }

    if (BN_GF2m_cmp(x0, x1)) {
        if (!BN_GF2m_add(t, x0, x1))
            goto err;
        if (!BN_GF2m_add(s, y0, y1))
            goto err;
        if (!group->meth->field_div(group, s, s, t, ctx))
            goto err;
        if (!group->meth->field_sqr(group, x2, s, ctx))
            goto err;
        if (!BN_GF2m_add(x2, x2, group->a))
            goto err;
        if (!BN_GF2m_add(x2, x2, s))
            goto err;
        if (!BN_GF2m_add(x2, x2, t))
            goto err;
    } else {
        if (BN_GF2m_cmp(y0, y1) || BN_is_zero(x1)) {
            if (!EC_POINT_set_to_infinity(group, r))
                goto err;
            ret = 1;
            goto err;
        }
        if (!group->meth->field_div(group, s, y1, x1, ctx))
            goto err;
        if (!BN_GF2m_add(s, s, x1))
            goto err;

        if (!group->meth->field_sqr(group, x2, s, ctx))
            goto err;
        if (!BN_GF2m_add(x2, x2, s))
            goto err;
        if (!BN_GF2m_add(x2, x2, group->a))
            goto err;
    }

    if (!BN_GF2m_add(y2, x1, x2))
        goto err;
    if (!group->meth->field_mul(group, y2, y2, s, ctx))
        goto err;
    if (!BN_GF2m_add(y2, y2, x2))
        goto err;
    if (!BN_GF2m_add(y2, y2, y1))
        goto err;

    if (!EC_POINT_set_affine_coordinates(group, r, x2, y2, ctx))
        goto err;

    ret = 1;

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/*
 * Computes 2 * a and stores the result in r.  r could be a. Uses algorithm
 * A.10.2 of IEEE P1363.
 */
int ossl_ec_GF2m_simple_dbl(const EC_GROUP *group, EC_POINT *r,
                            const EC_POINT *a, BN_CTX *ctx)
{
    return ossl_ec_GF2m_simple_add(group, r, a, a, ctx);
}

int ossl_ec_GF2m_simple_invert(const EC_GROUP *group, EC_POINT *point,
                               BN_CTX *ctx)
{
    if (EC_POINT_is_at_infinity(group, point) || BN_is_zero(point->Y))
        /* point is its own inverse */
        return 1;

    if (group->meth->make_affine == NULL
        || !group->meth->make_affine(group, point, ctx))
        return 0;
    return BN_GF2m_add(point->Y, point->X, point->Y);
}

/* Indicates whether the given point is the point at infinity. */
int ossl_ec_GF2m_simple_is_at_infinity(const EC_GROUP *group,
                                       const EC_POINT *point)
{
    return BN_is_zero(point->Z);
}

/*-
 * Determines whether the given EC_POINT is an actual point on the curve defined
 * in the EC_GROUP.  A point is valid if it satisfies the Weierstrass equation:
 *      y^2 + x*y = x^3 + a*x^2 + b.
 */
int ossl_ec_GF2m_simple_is_on_curve(const EC_GROUP *group, const EC_POINT *point,
                                    BN_CTX *ctx)
{
    int ret = -1;
    BIGNUM *lh, *y2;
    int (*field_mul) (const EC_GROUP *, BIGNUM *, const BIGNUM *,
                      const BIGNUM *, BN_CTX *);
    int (*field_sqr) (const EC_GROUP *, BIGNUM *, const BIGNUM *, BN_CTX *);
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (EC_POINT_is_at_infinity(group, point))
        return 1;

    field_mul = group->meth->field_mul;
    field_sqr = group->meth->field_sqr;

    /* only support affine coordinates */
    if (!point->Z_is_one)
        return -1;

#ifndef FIPS_MODULE
    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new();
        if (ctx == NULL)
            return -1;
    }
#endif

    BN_CTX_start(ctx);
    y2 = BN_CTX_get(ctx);
    lh = BN_CTX_get(ctx);
    if (lh == NULL)
        goto err;

    /*-
     * We have a curve defined by a Weierstrass equation
     *      y^2 + x*y = x^3 + a*x^2 + b.
     *  <=> x^3 + a*x^2 + x*y + b + y^2 = 0
     *  <=> ((x + a) * x + y) * x + b + y^2 = 0
     */
    if (!BN_GF2m_add(lh, point->X, group->a))
        goto err;
    if (!field_mul(group, lh, lh, point->X, ctx))
        goto err;
    if (!BN_GF2m_add(lh, lh, point->Y))
        goto err;
    if (!field_mul(group, lh, lh, point->X, ctx))
        goto err;
    if (!BN_GF2m_add(lh, lh, group->b))
        goto err;
    if (!field_sqr(group, y2, point->Y, ctx))
        goto err;
    if (!BN_GF2m_add(lh, lh, y2))
        goto err;
    ret = BN_is_zero(lh);

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/*-
 * Indicates whether two points are equal.
 * Return values:
 *  -1   error
 *   0   equal (in affine coordinates)
 *   1   not equal
 */
int ossl_ec_GF2m_simple_cmp(const EC_GROUP *group, const EC_POINT *a,
                            const EC_POINT *b, BN_CTX *ctx)
{
    BIGNUM *aX, *aY, *bX, *bY;
    int ret = -1;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (EC_POINT_is_at_infinity(group, a)) {
        return EC_POINT_is_at_infinity(group, b) ? 0 : 1;
    }

    if (EC_POINT_is_at_infinity(group, b))
        return 1;

    if (a->Z_is_one && b->Z_is_one) {
        return ((BN_cmp(a->X, b->X) == 0) && BN_cmp(a->Y, b->Y) == 0) ? 0 : 1;
    }

#ifndef FIPS_MODULE
    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new();
        if (ctx == NULL)
            return -1;
    }
#endif

    BN_CTX_start(ctx);
    aX = BN_CTX_get(ctx);
    aY = BN_CTX_get(ctx);
    bX = BN_CTX_get(ctx);
    bY = BN_CTX_get(ctx);
    if (bY == NULL)
        goto err;

    if (!EC_POINT_get_affine_coordinates(group, a, aX, aY, ctx))
        goto err;
    if (!EC_POINT_get_affine_coordinates(group, b, bX, bY, ctx))
        goto err;
    ret = ((BN_cmp(aX, bX) == 0) && BN_cmp(aY, bY) == 0) ? 0 : 1;

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/* Forces the given EC_POINT to internally use affine coordinates. */
int ossl_ec_GF2m_simple_make_affine(const EC_GROUP *group, EC_POINT *point,
                                    BN_CTX *ctx)
{
    BIGNUM *x, *y;
    int ret = 0;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (point->Z_is_one || EC_POINT_is_at_infinity(group, point))
        return 1;

#ifndef FIPS_MODULE
    if (ctx == NULL) {
        ctx = new_ctx = BN_CTX_new();
        if (ctx == NULL)
            return 0;
    }
#endif

    BN_CTX_start(ctx);
    x = BN_CTX_get(ctx);
    y = BN_CTX_get(ctx);
    if (y == NULL)
        goto err;

    if (!EC_POINT_get_affine_coordinates(group, point, x, y, ctx))
        goto err;
    if (!BN_copy(point->X, x))
        goto err;
    if (!BN_copy(point->Y, y))
        goto err;
    if (!BN_one(point->Z))
        goto err;
    point->Z_is_one = 1;

    ret = 1;

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/*
 * Forces each of the EC_POINTs in the given array to use affine coordinates.
 */
int ossl_ec_GF2m_simple_points_make_affine(const EC_GROUP *group, size_t num,
                                           EC_POINT *points[], BN_CTX *ctx)
{
    size_t i;

    for (i = 0; i < num; i++) {
        if (!group->meth->make_affine(group, points[i], ctx))
            return 0;
    }

    return 1;
}

/* Wrapper to simple binary polynomial field multiplication implementation. */
int ossl_ec_GF2m_simple_field_mul(const EC_GROUP *group, BIGNUM *r,
                                  const BIGNUM *a, const BIGNUM *b, BN_CTX *ctx)
{
    return BN_GF2m_mod_mul_arr(r, a, b, group->poly, ctx);
}

/* Wrapper to simple binary polynomial field squaring implementation. */
int ossl_ec_GF2m_simple_field_sqr(const EC_GROUP *group, BIGNUM *r,
                                  const BIGNUM *a, BN_CTX *ctx)
{
    return BN_GF2m_mod_sqr_arr(r, a, group->poly, ctx);
}

/* Wrapper to simple binary polynomial field division implementation. */
int ossl_ec_GF2m_simple_field_div(const EC_GROUP *group, BIGNUM *r,
                                  const BIGNUM *a, const BIGNUM *b, BN_CTX *ctx)
{
    return BN_GF2m_mod_div(r, a, b, group->field, ctx);
}

/*-
 * Lopez-Dahab ladder, pre step.
 * See e.g. "Guide to ECC" Alg 3.40.
 * Modified to blind s and r independently.
 * s:= p, r := 2p
 */
static
int ec_GF2m_simple_ladder_pre(const EC_GROUP *group,
                              EC_POINT *r, EC_POINT *s,
                              EC_POINT *p, BN_CTX *ctx)
{
    /* if p is not affine, something is wrong */
    if (p->Z_is_one == 0)
        return 0;

    /* s blinding: make sure lambda (s->Z here) is not zero */
    do {
        if (!BN_priv_rand_ex(s->Z, BN_num_bits(group->field) - 1,
                             BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY, 0, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            return 0;
        }
    } while (BN_is_zero(s->Z));

    /* if field_encode defined convert between representations */
    if ((group->meth->field_encode != NULL
         && !group->meth->field_encode(group, s->Z, s->Z, ctx))
        || !group->meth->field_mul(group, s->X, p->X, s->Z, ctx))
        return 0;

    /* r blinding: make sure lambda (r->Y here for storage) is not zero */
    do {
        if (!BN_priv_rand_ex(r->Y, BN_num_bits(group->field) - 1,
                             BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY, 0, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            return 0;
        }
    } while (BN_is_zero(r->Y));

    if ((group->meth->field_encode != NULL
         && !group->meth->field_encode(group, r->Y, r->Y, ctx))
        || !group->meth->field_sqr(group, r->Z, p->X, ctx)
        || !group->meth->field_sqr(group, r->X, r->Z, ctx)
        || !BN_GF2m_add(r->X, r->X, group->b)
        || !group->meth->field_mul(group, r->Z, r->Z, r->Y, ctx)
        || !group->meth->field_mul(group, r->X, r->X, r->Y, ctx))
        return 0;

    s->Z_is_one = 0;
    r->Z_is_one = 0;

    return 1;
}

/*-
 * Ladder step: differential addition-and-doubling, mixed Lopez-Dahab coords.
 * http://www.hyperelliptic.org/EFD/g12o/auto-code/shortw/xz/ladder/mladd-2003-s.op3
 * s := r + s, r := 2r
 */
static
int ec_GF2m_simple_ladder_step(const EC_GROUP *group,
                               EC_POINT *r, EC_POINT *s,
                               EC_POINT *p, BN_CTX *ctx)
{
    if (!group->meth->field_mul(group, r->Y, r->Z, s->X, ctx)
        || !group->meth->field_mul(group, s->X, r->X, s->Z, ctx)
        || !group->meth->field_sqr(group, s->Y, r->Z, ctx)
        || !group->meth->field_sqr(group, r->Z, r->X, ctx)
        || !BN_GF2m_add(s->Z, r->Y, s->X)
        || !group->meth->field_sqr(group, s->Z, s->Z, ctx)
        || !group->meth->field_mul(group, s->X, r->Y, s->X, ctx)
        || !group->meth->field_mul(group, r->Y, s->Z, p->X, ctx)
        || !BN_GF2m_add(s->X, s->X, r->Y)
        || !group->meth->field_sqr(group, r->Y, r->Z, ctx)
        || !group->meth->field_mul(group, r->Z, r->Z, s->Y, ctx)
        || !group->meth->field_sqr(group, s->Y, s->Y, ctx)
        || !group->meth->field_mul(group, s->Y, s->Y, group->b, ctx)
        || !BN_GF2m_add(r->X, r->Y, s->Y))
        return 0;

    return 1;
}

/*-
 * Recover affine (x,y) result from Lopez-Dahab r and s, affine p.
 * See e.g. "Fast Multiplication on Elliptic Curves over GF(2**m)
 * without Precomputation" (Lopez and Dahab, CHES 1999),
 * Appendix Alg Mxy.
 */
static
int ec_GF2m_simple_ladder_post(const EC_GROUP *group,
                               EC_POINT *r, EC_POINT *s,
                               EC_POINT *p, BN_CTX *ctx)
{
    int ret = 0;
    BIGNUM *t0, *t1, *t2 = NULL;

    if (BN_is_zero(r->Z))
        return EC_POINT_set_to_infinity(group, r);

    if (BN_is_zero(s->Z)) {
        if (!EC_POINT_copy(r, p)
            || !EC_POINT_invert(group, r, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            return 0;
        }
        return 1;
    }

    BN_CTX_start(ctx);
    t0 = BN_CTX_get(ctx);
    t1 = BN_CTX_get(ctx);
    t2 = BN_CTX_get(ctx);
    if (t2 == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    if (!group->meth->field_mul(group, t0, r->Z, s->Z, ctx)
        || !group->meth->field_mul(group, t1, p->X, r->Z, ctx)
        || !BN_GF2m_add(t1, r->X, t1)
        || !group->meth->field_mul(group, t2, p->X, s->Z, ctx)
        || !group->meth->field_mul(group, r->Z, r->X, t2, ctx)
        || !BN_GF2m_add(t2, t2, s->X)
        || !group->meth->field_mul(group, t1, t1, t2, ctx)
        || !group->meth->field_sqr(group, t2, p->X, ctx)
        || !BN_GF2m_add(t2, p->Y, t2)
        || !group->meth->field_mul(group, t2, t2, t0, ctx)
        || !BN_GF2m_add(t1, t2, t1)
        || !group->meth->field_mul(group, t2, p->X, t0, ctx)
        || !group->meth->field_inv(group, t2, t2, ctx)
        || !group->meth->field_mul(group, t1, t1, t2, ctx)
        || !group->meth->field_mul(group, r->X, r->Z, t2, ctx)
        || !BN_GF2m_add(t2, p->X, r->X)
        || !group->meth->field_mul(group, t2, t2, t1, ctx)
        || !BN_GF2m_add(r->Y, p->Y, t2)
        || !BN_one(r->Z))
        goto err;

    r->Z_is_one = 1;

    /* GF(2^m) field elements should always have BIGNUM::neg = 0 */
    BN_set_negative(r->X, 0);
    BN_set_negative(r->Y, 0);

    ret = 1;

 err:
    BN_CTX_end(ctx);
    return ret;
}

static
int ec_GF2m_simple_points_mul(const EC_GROUP *group, EC_POINT *r,
                              const BIGNUM *scalar, size_t num,
                              const EC_POINT *points[],
                              const BIGNUM *scalars[],
                              BN_CTX *ctx)
{
    int ret = 0;
    EC_POINT *t = NULL;

    /*-
     * We limit use of the ladder only to the following cases:
     * - r := scalar * G
     *   Fixed point mul: scalar != NULL && num == 0;
     * - r := scalars[0] * points[0]
     *   Variable point mul: scalar == NULL && num == 1;
     * - r := scalar * G + scalars[0] * points[0]
     *   used, e.g., in ECDSA verification: scalar != NULL && num == 1
     *
     * In any other case (num > 1) we use the default wNAF implementation.
     *
     * We also let the default implementation handle degenerate cases like group
     * order or cofactor set to 0.
     */
    if (num > 1 || BN_is_zero(group->order) || BN_is_zero(group->cofactor))
        return ossl_ec_wNAF_mul(group, r, scalar, num, points, scalars, ctx);

    if (scalar != NULL && num == 0)
        /* Fixed point multiplication */
        return ossl_ec_scalar_mul_ladder(group, r, scalar, NULL, ctx);

    if (scalar == NULL && num == 1)
        /* Variable point multiplication */
        return ossl_ec_scalar_mul_ladder(group, r, scalars[0], points[0], ctx);

    /*-
     * Double point multiplication:
     *  r := scalar * G + scalars[0] * points[0]
     */

    if ((t = EC_POINT_new(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        return 0;
    }

    if (!ossl_ec_scalar_mul_ladder(group, t, scalar, NULL, ctx)
        || !ossl_ec_scalar_mul_ladder(group, r, scalars[0], points[0], ctx)
        || !EC_POINT_add(group, r, t, r, ctx))
        goto err;

    ret = 1;

 err:
    EC_POINT_free(t);
    return ret;
}

/*-
 * Computes the multiplicative inverse of a in GF(2^m), storing the result in r.
 * If a is zero (or equivalent), you'll get an EC_R_CANNOT_INVERT error.
 * SCA hardening is with blinding: BN_GF2m_mod_inv does that.
 */
static int ec_GF2m_simple_field_inv(const EC_GROUP *group, BIGNUM *r,
                                    const BIGNUM *a, BN_CTX *ctx)
{
    int ret;

    if (!(ret = BN_GF2m_mod_inv(r, a, group->field, ctx)))
        ERR_raise(ERR_LIB_EC, EC_R_CANNOT_INVERT);
    return ret;
}

const EC_METHOD *EC_GF2m_simple_method(void)
{
    static const EC_METHOD ret = {
        EC_FLAGS_DEFAULT_OCT,
        NID_X9_62_characteristic_two_field,
        ossl_ec_GF2m_simple_group_init,
        ossl_ec_GF2m_simple_group_finish,
        ossl_ec_GF2m_simple_group_clear_finish,
        ossl_ec_GF2m_simple_group_copy,
        ossl_ec_GF2m_simple_group_set_curve,
        ossl_ec_GF2m_simple_group_get_curve,
        ossl_ec_GF2m_simple_group_get_degree,
        ossl_ec_group_simple_order_bits,
        ossl_ec_GF2m_simple_group_check_discriminant,
        ossl_ec_GF2m_simple_point_init,
        ossl_ec_GF2m_simple_point_finish,
        ossl_ec_GF2m_simple_point_clear_finish,
        ossl_ec_GF2m_simple_point_copy,
        ossl_ec_GF2m_simple_point_set_to_infinity,
        ossl_ec_GF2m_simple_point_set_affine_coordinates,
        ossl_ec_GF2m_simple_point_get_affine_coordinates,
        0, /* point_set_compressed_coordinates */
        0, /* point2oct */
        0, /* oct2point */
        ossl_ec_GF2m_simple_add,
        ossl_ec_GF2m_simple_dbl,
        ossl_ec_GF2m_simple_invert,
        ossl_ec_GF2m_simple_is_at_infinity,
        ossl_ec_GF2m_simple_is_on_curve,
        ossl_ec_GF2m_simple_cmp,
        ossl_ec_GF2m_simple_make_affine,
        ossl_ec_GF2m_simple_points_make_affine,
        ec_GF2m_simple_points_mul,
        0, /* precompute_mult */
        0, /* have_precompute_mult */
        ossl_ec_GF2m_simple_field_mul,
        ossl_ec_GF2m_simple_field_sqr,
        ossl_ec_GF2m_simple_field_div,
        ec_GF2m_simple_field_inv,
        0, /* field_encode */
        0, /* field_decode */
        0, /* field_set_to_one */
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
        0, /* blind_coordinates */
        ec_GF2m_simple_ladder_pre,
        ec_GF2m_simple_ladder_step,
        ec_GF2m_simple_ladder_post
    };

    return &ret;
}

#endif
