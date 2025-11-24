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
 * EC_GROUP low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/params.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/opensslv.h>
#include <openssl/param_build.h>
#include "crypto/ec.h"
#include "crypto/bn.h"
#include "internal/nelem.h"
#include "ec_local.h"

/* functions for EC_GROUP objects */

EC_GROUP *ossl_ec_group_new_ex(OSSL_LIB_CTX *libctx, const char *propq,
                               const EC_METHOD *meth)
{
    EC_GROUP *ret;

    if (meth == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_SLOT_FULL);
        return NULL;
    }
    if (meth->group_init == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return NULL;
    }

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;

    ret->libctx = libctx;
    if (propq != NULL) {
        ret->propq = OPENSSL_strdup(propq);
        if (ret->propq == NULL)
            goto err;
    }
    ret->meth = meth;
    if ((ret->meth->flags & EC_FLAGS_CUSTOM_CURVE) == 0) {
        ret->order = BN_new();
        if (ret->order == NULL)
            goto err;
        ret->cofactor = BN_new();
        if (ret->cofactor == NULL)
            goto err;
    }
    ret->asn1_flag = OPENSSL_EC_EXPLICIT_CURVE;
    ret->asn1_form = POINT_CONVERSION_UNCOMPRESSED;
    if (!meth->group_init(ret))
        goto err;
    return ret;

 err:
    BN_free(ret->order);
    BN_free(ret->cofactor);
    OPENSSL_free(ret->propq);
    OPENSSL_free(ret);
    return NULL;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
# ifndef FIPS_MODULE
EC_GROUP *EC_GROUP_new(const EC_METHOD *meth)
{
    return ossl_ec_group_new_ex(NULL, NULL, meth);
}
# endif
#endif

void EC_pre_comp_free(EC_GROUP *group)
{
    switch (group->pre_comp_type) {
    case PCT_none:
        break;
    case PCT_nistz256:
#ifdef ECP_NISTZ256_ASM
        EC_nistz256_pre_comp_free(group->pre_comp.nistz256);
#endif
        break;
#ifndef OPENSSL_NO_EC_NISTP_64_GCC_128
    case PCT_nistp224:
        EC_nistp224_pre_comp_free(group->pre_comp.nistp224);
        break;
    case PCT_nistp256:
        EC_nistp256_pre_comp_free(group->pre_comp.nistp256);
        break;
    case PCT_nistp384:
        ossl_ec_nistp384_pre_comp_free(group->pre_comp.nistp384);
        break;
    case PCT_nistp521:
        EC_nistp521_pre_comp_free(group->pre_comp.nistp521);
        break;
#else
    case PCT_nistp224:
    case PCT_nistp256:
    case PCT_nistp384:
    case PCT_nistp521:
        break;
#endif
    case PCT_ec:
        EC_ec_pre_comp_free(group->pre_comp.ec);
        break;
    }
    group->pre_comp.ec = NULL;
}

void EC_GROUP_free(EC_GROUP *group)
{
    if (!group)
        return;

    if (group->meth->group_finish != 0)
        group->meth->group_finish(group);

    EC_pre_comp_free(group);
    BN_MONT_CTX_free(group->mont_data);
    EC_POINT_free(group->generator);
    BN_free(group->order);
    BN_free(group->cofactor);
    OPENSSL_free(group->seed);
    OPENSSL_free(group->propq);
    OPENSSL_free(group);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
void EC_GROUP_clear_free(EC_GROUP *group)
{
    if (!group)
        return;

    if (group->meth->group_clear_finish != 0)
        group->meth->group_clear_finish(group);
    else if (group->meth->group_finish != 0)
        group->meth->group_finish(group);

    EC_pre_comp_free(group);
    BN_MONT_CTX_free(group->mont_data);
    EC_POINT_clear_free(group->generator);
    BN_clear_free(group->order);
    BN_clear_free(group->cofactor);
    OPENSSL_clear_free(group->seed, group->seed_len);
    OPENSSL_clear_free(group, sizeof(*group));
}
#endif

int EC_GROUP_copy(EC_GROUP *dest, const EC_GROUP *src)
{
    if (dest->meth->group_copy == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (dest->meth != src->meth) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (dest == src)
        return 1;

    dest->libctx = src->libctx;
    dest->curve_name = src->curve_name;

    /* Copy precomputed */
    dest->pre_comp_type = src->pre_comp_type;
    switch (src->pre_comp_type) {
    case PCT_none:
        dest->pre_comp.ec = NULL;
        break;
    case PCT_nistz256:
#ifdef ECP_NISTZ256_ASM
        dest->pre_comp.nistz256 = EC_nistz256_pre_comp_dup(src->pre_comp.nistz256);
#endif
        break;
#ifndef OPENSSL_NO_EC_NISTP_64_GCC_128
    case PCT_nistp224:
        dest->pre_comp.nistp224 = EC_nistp224_pre_comp_dup(src->pre_comp.nistp224);
        break;
    case PCT_nistp256:
        dest->pre_comp.nistp256 = EC_nistp256_pre_comp_dup(src->pre_comp.nistp256);
        break;
    case PCT_nistp384:
        dest->pre_comp.nistp384 = ossl_ec_nistp384_pre_comp_dup(src->pre_comp.nistp384);
        break;
    case PCT_nistp521:
        dest->pre_comp.nistp521 = EC_nistp521_pre_comp_dup(src->pre_comp.nistp521);
        break;
#else
    case PCT_nistp224:
    case PCT_nistp256:
    case PCT_nistp384:
    case PCT_nistp521:
        break;
#endif
    case PCT_ec:
        dest->pre_comp.ec = EC_ec_pre_comp_dup(src->pre_comp.ec);
        break;
    }

    if (src->mont_data != NULL) {
        if (dest->mont_data == NULL) {
            dest->mont_data = BN_MONT_CTX_new();
            if (dest->mont_data == NULL)
                return 0;
        }
        if (!BN_MONT_CTX_copy(dest->mont_data, src->mont_data))
            return 0;
    } else {
        /* src->generator == NULL */
        BN_MONT_CTX_free(dest->mont_data);
        dest->mont_data = NULL;
    }

    if (src->generator != NULL) {
        if (dest->generator == NULL) {
            dest->generator = EC_POINT_new(dest);
            if (dest->generator == NULL)
                return 0;
        }
        if (!EC_POINT_copy(dest->generator, src->generator))
            return 0;
    } else {
        /* src->generator == NULL */
        EC_POINT_clear_free(dest->generator);
        dest->generator = NULL;
    }

    if ((src->meth->flags & EC_FLAGS_CUSTOM_CURVE) == 0) {
        if (!BN_copy(dest->order, src->order))
            return 0;
        if (!BN_copy(dest->cofactor, src->cofactor))
            return 0;
    }

    dest->asn1_flag = src->asn1_flag;
    dest->asn1_form = src->asn1_form;
    dest->decoded_from_explicit_params = src->decoded_from_explicit_params;

    if (src->seed) {
        OPENSSL_free(dest->seed);
        if ((dest->seed = OPENSSL_malloc(src->seed_len)) == NULL)
            return 0;
        if (!memcpy(dest->seed, src->seed, src->seed_len))
            return 0;
        dest->seed_len = src->seed_len;
    } else {
        OPENSSL_free(dest->seed);
        dest->seed = NULL;
        dest->seed_len = 0;
    }

    return dest->meth->group_copy(dest, src);
}

EC_GROUP *EC_GROUP_dup(const EC_GROUP *a)
{
    EC_GROUP *t = NULL;
    int ok = 0;

    if (a == NULL)
        return NULL;

    if ((t = ossl_ec_group_new_ex(a->libctx, a->propq, a->meth)) == NULL)
        return NULL;
    if (!EC_GROUP_copy(t, a))
        goto err;

    ok = 1;

 err:
    if (!ok) {
        EC_GROUP_free(t);
        return NULL;
    }
        return t;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
const EC_METHOD *EC_GROUP_method_of(const EC_GROUP *group)
{
    return group->meth;
}

int EC_METHOD_get_field_type(const EC_METHOD *meth)
{
    return meth->field_type;
}
#endif

static int ec_precompute_mont_data(EC_GROUP *);

/*-
 * Try computing cofactor from the generator order (n) and field cardinality (q).
 * This works for all curves of cryptographic interest.
 *
 * Hasse thm: q + 1 - 2*sqrt(q) <= n*h <= q + 1 + 2*sqrt(q)
 * h_min = (q + 1 - 2*sqrt(q))/n
 * h_max = (q + 1 + 2*sqrt(q))/n
 * h_max - h_min = 4*sqrt(q)/n
 * So if n > 4*sqrt(q) holds, there is only one possible value for h:
 * h = \lfloor (h_min + h_max)/2 \rceil = \lfloor (q + 1)/n \rceil
 *
 * Otherwise, zero cofactor and return success.
 */
static int ec_guess_cofactor(EC_GROUP *group) {
    int ret = 0;
    BN_CTX *ctx = NULL;
    BIGNUM *q = NULL;

    /*-
     * If the cofactor is too large, we cannot guess it.
     * The RHS of below is a strict overestimate of lg(4 * sqrt(q))
     */
    if (BN_num_bits(group->order) <= (BN_num_bits(group->field) + 1) / 2 + 3) {
        /* default to 0 */
        BN_zero(group->cofactor);
        /* return success */
        return 1;
    }

    if ((ctx = BN_CTX_new_ex(group->libctx)) == NULL)
        return 0;

    BN_CTX_start(ctx);
    if ((q = BN_CTX_get(ctx)) == NULL)
        goto err;

    /* set q = 2**m for binary fields; q = p otherwise */
    if (group->meth->field_type == NID_X9_62_characteristic_two_field) {
        BN_zero(q);
        if (!BN_set_bit(q, BN_num_bits(group->field) - 1))
            goto err;
    } else {
        if (!BN_copy(q, group->field))
            goto err;
    }

    /* compute h = \lfloor (q + 1)/n \rceil = \lfloor (q + 1 + n/2)/n \rfloor */
    if (!BN_rshift1(group->cofactor, group->order) /* n/2 */
        || !BN_add(group->cofactor, group->cofactor, q) /* q + n/2 */
        /* q + 1 + n/2 */
        || !BN_add(group->cofactor, group->cofactor, BN_value_one())
        /* (q + 1 + n/2)/n */
        || !BN_div(group->cofactor, NULL, group->cofactor, group->order, ctx))
        goto err;
    ret = 1;
 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ret;
}

int EC_GROUP_set_generator(EC_GROUP *group, const EC_POINT *generator,
                           const BIGNUM *order, const BIGNUM *cofactor)
{
    if (generator == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* require group->field >= 1 */
    if (group->field == NULL || BN_is_zero(group->field)
        || BN_is_negative(group->field)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_FIELD);
        return 0;
    }

    /*-
     * - require order >= 1
     * - enforce upper bound due to Hasse thm: order can be no more than one bit
     *   longer than field cardinality
     */
    if (order == NULL || BN_is_zero(order) || BN_is_negative(order)
        || BN_num_bits(order) > BN_num_bits(group->field) + 1) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GROUP_ORDER);
        return 0;
    }

    /*-
     * Unfortunately the cofactor is an optional field in many standards.
     * Internally, the lib uses 0 cofactor as a marker for "unknown cofactor".
     * So accept cofactor == NULL or cofactor >= 0.
     */
    if (cofactor != NULL && BN_is_negative(cofactor)) {
        ERR_raise(ERR_LIB_EC, EC_R_UNKNOWN_COFACTOR);
        return 0;
    }

    if (group->generator == NULL) {
        group->generator = EC_POINT_new(group);
        if (group->generator == NULL)
            return 0;
    }
    if (!EC_POINT_copy(group->generator, generator))
        return 0;

    if (!BN_copy(group->order, order))
        return 0;

    /* Either take the provided positive cofactor, or try to compute it */
    if (cofactor != NULL && !BN_is_zero(cofactor)) {
        if (!BN_copy(group->cofactor, cofactor))
            return 0;
    } else if (!ec_guess_cofactor(group)) {
        BN_zero(group->cofactor);
        return 0;
    }

    /*
     * Some groups have an order with
     * factors of two, which makes the Montgomery setup fail.
     * |group->mont_data| will be NULL in this case.
     */
    if (BN_is_odd(group->order)) {
        return ec_precompute_mont_data(group);
    }

    BN_MONT_CTX_free(group->mont_data);
    group->mont_data = NULL;
    return 1;
}

const EC_POINT *EC_GROUP_get0_generator(const EC_GROUP *group)
{
    return group->generator;
}

BN_MONT_CTX *EC_GROUP_get_mont_data(const EC_GROUP *group)
{
    return group->mont_data;
}

int EC_GROUP_get_order(const EC_GROUP *group, BIGNUM *order, BN_CTX *ctx)
{
    if (group->order == NULL)
        return 0;
    if (!BN_copy(order, group->order))
        return 0;

    return !BN_is_zero(order);
}

const BIGNUM *EC_GROUP_get0_order(const EC_GROUP *group)
{
    return group->order;
}

int EC_GROUP_order_bits(const EC_GROUP *group)
{
    return group->meth->group_order_bits(group);
}

int EC_GROUP_get_cofactor(const EC_GROUP *group, BIGNUM *cofactor,
                          BN_CTX *ctx)
{

    if (group->cofactor == NULL)
        return 0;
    if (!BN_copy(cofactor, group->cofactor))
        return 0;

    return !BN_is_zero(group->cofactor);
}

const BIGNUM *EC_GROUP_get0_cofactor(const EC_GROUP *group)
{
    return group->cofactor;
}

void EC_GROUP_set_curve_name(EC_GROUP *group, int nid)
{
    group->curve_name = nid;
    group->asn1_flag =
        (nid != NID_undef)
        ? OPENSSL_EC_NAMED_CURVE
        : OPENSSL_EC_EXPLICIT_CURVE;
}

int EC_GROUP_get_curve_name(const EC_GROUP *group)
{
    return group->curve_name;
}

const BIGNUM *EC_GROUP_get0_field(const EC_GROUP *group)
{
    return group->field;
}

int EC_GROUP_get_field_type(const EC_GROUP *group)
{
    return group->meth->field_type;
}

void EC_GROUP_set_asn1_flag(EC_GROUP *group, int flag)
{
    group->asn1_flag = flag;
}

int EC_GROUP_get_asn1_flag(const EC_GROUP *group)
{
    return group->asn1_flag;
}

void EC_GROUP_set_point_conversion_form(EC_GROUP *group,
                                        point_conversion_form_t form)
{
    group->asn1_form = form;
}

point_conversion_form_t EC_GROUP_get_point_conversion_form(const EC_GROUP
                                                           *group)
{
    return group->asn1_form;
}

size_t EC_GROUP_set_seed(EC_GROUP *group, const unsigned char *p, size_t len)
{
    OPENSSL_free(group->seed);
    group->seed = NULL;
    group->seed_len = 0;

    if (!len || !p)
        return 1;

    if ((group->seed = OPENSSL_malloc(len)) == NULL)
        return 0;
    memcpy(group->seed, p, len);
    group->seed_len = len;

    return len;
}

unsigned char *EC_GROUP_get0_seed(const EC_GROUP *group)
{
    return group->seed;
}

size_t EC_GROUP_get_seed_len(const EC_GROUP *group)
{
    return group->seed_len;
}

int EC_GROUP_set_curve(EC_GROUP *group, const BIGNUM *p, const BIGNUM *a,
                       const BIGNUM *b, BN_CTX *ctx)
{
    if (group->meth->group_set_curve == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    return group->meth->group_set_curve(group, p, a, b, ctx);
}

int EC_GROUP_get_curve(const EC_GROUP *group, BIGNUM *p, BIGNUM *a, BIGNUM *b,
                       BN_CTX *ctx)
{
    if (group->meth->group_get_curve == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    return group->meth->group_get_curve(group, p, a, b, ctx);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_GROUP_set_curve_GFp(EC_GROUP *group, const BIGNUM *p, const BIGNUM *a,
                           const BIGNUM *b, BN_CTX *ctx)
{
    return EC_GROUP_set_curve(group, p, a, b, ctx);
}

int EC_GROUP_get_curve_GFp(const EC_GROUP *group, BIGNUM *p, BIGNUM *a,
                           BIGNUM *b, BN_CTX *ctx)
{
    return EC_GROUP_get_curve(group, p, a, b, ctx);
}

# ifndef OPENSSL_NO_EC2M
int EC_GROUP_set_curve_GF2m(EC_GROUP *group, const BIGNUM *p, const BIGNUM *a,
                            const BIGNUM *b, BN_CTX *ctx)
{
    return EC_GROUP_set_curve(group, p, a, b, ctx);
}

int EC_GROUP_get_curve_GF2m(const EC_GROUP *group, BIGNUM *p, BIGNUM *a,
                            BIGNUM *b, BN_CTX *ctx)
{
    return EC_GROUP_get_curve(group, p, a, b, ctx);
}
# endif
#endif

int EC_GROUP_get_degree(const EC_GROUP *group)
{
    if (group->meth->group_get_degree == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    return group->meth->group_get_degree(group);
}

int EC_GROUP_check_discriminant(const EC_GROUP *group, BN_CTX *ctx)
{
    if (group->meth->group_check_discriminant == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    return group->meth->group_check_discriminant(group, ctx);
}

int EC_GROUP_cmp(const EC_GROUP *a, const EC_GROUP *b, BN_CTX *ctx)
{
    int r = 0;
    BIGNUM *a1, *a2, *a3, *b1, *b2, *b3;
#ifndef FIPS_MODULE
    BN_CTX *ctx_new = NULL;
#endif

    /* compare the field types */
    if (EC_GROUP_get_field_type(a) != EC_GROUP_get_field_type(b))
        return 1;
    /* compare the curve name (if present in both) */
    if (EC_GROUP_get_curve_name(a) && EC_GROUP_get_curve_name(b) &&
        EC_GROUP_get_curve_name(a) != EC_GROUP_get_curve_name(b))
        return 1;
    if (a->meth->flags & EC_FLAGS_CUSTOM_CURVE)
        return 0;

#ifndef FIPS_MODULE
    if (ctx == NULL)
        ctx_new = ctx = BN_CTX_new();
#endif
    if (ctx == NULL)
        return -1;

    BN_CTX_start(ctx);
    a1 = BN_CTX_get(ctx);
    a2 = BN_CTX_get(ctx);
    a3 = BN_CTX_get(ctx);
    b1 = BN_CTX_get(ctx);
    b2 = BN_CTX_get(ctx);
    b3 = BN_CTX_get(ctx);
    if (b3 == NULL) {
        BN_CTX_end(ctx);
#ifndef FIPS_MODULE
        BN_CTX_free(ctx_new);
#endif
        return -1;
    }

    /*
     * XXX This approach assumes that the external representation of curves
     * over the same field type is the same.
     */
    if (!a->meth->group_get_curve(a, a1, a2, a3, ctx) ||
        !b->meth->group_get_curve(b, b1, b2, b3, ctx))
        r = 1;

    /* return 1 if the curve parameters are different */
    if (r || BN_cmp(a1, b1) != 0 || BN_cmp(a2, b2) != 0 || BN_cmp(a3, b3) != 0)
        r = 1;

    /* XXX EC_POINT_cmp() assumes that the methods are equal */
    /* return 1 if the generators are different */
    if (r || EC_POINT_cmp(a, EC_GROUP_get0_generator(a),
                          EC_GROUP_get0_generator(b), ctx) != 0)
        r = 1;

    if (!r) {
        const BIGNUM *ao, *bo, *ac, *bc;
        /* compare the orders */
        ao = EC_GROUP_get0_order(a);
        bo = EC_GROUP_get0_order(b);
        if (ao == NULL || bo == NULL) {
            /* return an error if either order is NULL */
            r = -1;
            goto end;
        }
        if (BN_cmp(ao, bo) != 0) {
            /* return 1 if orders are different */
            r = 1;
            goto end;
        }
        /*
         * It gets here if the curve parameters and generator matched.
         * Now check the optional cofactors (if both are present).
         */
        ac = EC_GROUP_get0_cofactor(a);
        bc = EC_GROUP_get0_cofactor(b);
        /* Returns 1 (mismatch) if both cofactors are specified and different */
        if (!BN_is_zero(ac) && !BN_is_zero(bc) && BN_cmp(ac, bc) != 0)
            r = 1;
        /* Returns 0 if the parameters matched */
    }
end:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(ctx_new);
#endif
    return r;
}

/* functions for EC_POINT objects */

EC_POINT *EC_POINT_new(const EC_GROUP *group)
{
    EC_POINT *ret;

    if (group == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (group->meth->point_init == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return NULL;
    }

    ret = OPENSSL_zalloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;

    ret->meth = group->meth;
    ret->curve_name = group->curve_name;

    if (!ret->meth->point_init(ret)) {
        OPENSSL_free(ret);
        return NULL;
    }

    return ret;
}

void EC_POINT_free(EC_POINT *point)
{
    if (point == NULL)
        return;

#ifdef OPENSSL_PEDANTIC_ZEROIZATION
    EC_POINT_clear_free(point);
#else
    if (point->meth->point_finish != 0)
        point->meth->point_finish(point);
    OPENSSL_free(point);
#endif
}

void EC_POINT_clear_free(EC_POINT *point)
{
    if (point == NULL)
        return;

    if (point->meth->point_clear_finish != 0)
        point->meth->point_clear_finish(point);
    else if (point->meth->point_finish != 0)
        point->meth->point_finish(point);
    OPENSSL_clear_free(point, sizeof(*point));
}

int EC_POINT_copy(EC_POINT *dest, const EC_POINT *src)
{
    if (dest->meth->point_copy == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (dest->meth != src->meth
            || (dest->curve_name != src->curve_name
                 && dest->curve_name != 0
                 && src->curve_name != 0)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (dest == src)
        return 1;
    return dest->meth->point_copy(dest, src);
}

EC_POINT *EC_POINT_dup(const EC_POINT *a, const EC_GROUP *group)
{
    EC_POINT *t;
    int r;

    if (a == NULL)
        return NULL;

    t = EC_POINT_new(group);
    if (t == NULL)
        return NULL;
    r = EC_POINT_copy(t, a);
    if (!r) {
        EC_POINT_free(t);
        return NULL;
    }
    return t;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
const EC_METHOD *EC_POINT_method_of(const EC_POINT *point)
{
    return point->meth;
}
#endif

int EC_POINT_set_to_infinity(const EC_GROUP *group, EC_POINT *point)
{
    if (group->meth->point_set_to_infinity == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (group->meth != point->meth) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->point_set_to_infinity(group, point);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINT_set_Jprojective_coordinates_GFp(const EC_GROUP *group,
                                             EC_POINT *point, const BIGNUM *x,
                                             const BIGNUM *y, const BIGNUM *z,
                                             BN_CTX *ctx)
{
    if (group->meth->field_type != NID_X9_62_prime_field) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return ossl_ec_GFp_simple_set_Jprojective_coordinates_GFp(group, point,
                                                              x, y, z, ctx);
}

int EC_POINT_get_Jprojective_coordinates_GFp(const EC_GROUP *group,
                                             const EC_POINT *point, BIGNUM *x,
                                             BIGNUM *y, BIGNUM *z,
                                             BN_CTX *ctx)
{
    if (group->meth->field_type != NID_X9_62_prime_field) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return ossl_ec_GFp_simple_get_Jprojective_coordinates_GFp(group, point,
                                                              x, y, z, ctx);
}
#endif

int EC_POINT_set_affine_coordinates(const EC_GROUP *group, EC_POINT *point,
                                    const BIGNUM *x, const BIGNUM *y,
                                    BN_CTX *ctx)
{
    if (group->meth->point_set_affine_coordinates == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (!group->meth->point_set_affine_coordinates(group, point, x, y, ctx))
        return 0;

    if (EC_POINT_is_on_curve(group, point, ctx) <= 0) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_IS_NOT_ON_CURVE);
        return 0;
    }
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINT_set_affine_coordinates_GFp(const EC_GROUP *group,
                                        EC_POINT *point, const BIGNUM *x,
                                        const BIGNUM *y, BN_CTX *ctx)
{
    return EC_POINT_set_affine_coordinates(group, point, x, y, ctx);
}

# ifndef OPENSSL_NO_EC2M
int EC_POINT_set_affine_coordinates_GF2m(const EC_GROUP *group,
                                         EC_POINT *point, const BIGNUM *x,
                                         const BIGNUM *y, BN_CTX *ctx)
{
    return EC_POINT_set_affine_coordinates(group, point, x, y, ctx);
}
# endif
#endif

int EC_POINT_get_affine_coordinates(const EC_GROUP *group,
                                    const EC_POINT *point, BIGNUM *x, BIGNUM *y,
                                    BN_CTX *ctx)
{
    if (group->meth->point_get_affine_coordinates == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (EC_POINT_is_at_infinity(group, point)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_AT_INFINITY);
        return 0;
    }
    return group->meth->point_get_affine_coordinates(group, point, x, y, ctx);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINT_get_affine_coordinates_GFp(const EC_GROUP *group,
                                        const EC_POINT *point, BIGNUM *x,
                                        BIGNUM *y, BN_CTX *ctx)
{
    return EC_POINT_get_affine_coordinates(group, point, x, y, ctx);
}

# ifndef OPENSSL_NO_EC2M
int EC_POINT_get_affine_coordinates_GF2m(const EC_GROUP *group,
                                         const EC_POINT *point, BIGNUM *x,
                                         BIGNUM *y, BN_CTX *ctx)
{
    return EC_POINT_get_affine_coordinates(group, point, x, y, ctx);
}
# endif
#endif

int EC_POINT_add(const EC_GROUP *group, EC_POINT *r, const EC_POINT *a,
                 const EC_POINT *b, BN_CTX *ctx)
{
    if (group->meth->add == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(r, group) || !ec_point_is_compat(a, group)
        || !ec_point_is_compat(b, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->add(group, r, a, b, ctx);
}

int EC_POINT_dbl(const EC_GROUP *group, EC_POINT *r, const EC_POINT *a,
                 BN_CTX *ctx)
{
    if (group->meth->dbl == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(r, group) || !ec_point_is_compat(a, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->dbl(group, r, a, ctx);
}

int EC_POINT_invert(const EC_GROUP *group, EC_POINT *a, BN_CTX *ctx)
{
    if (group->meth->invert == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(a, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->invert(group, a, ctx);
}

int EC_POINT_is_at_infinity(const EC_GROUP *group, const EC_POINT *point)
{
    if (group->meth->is_at_infinity == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->is_at_infinity(group, point);
}

/*
 * Check whether an EC_POINT is on the curve or not. Note that the return
 * value for this function should NOT be treated as a boolean. Return values:
 *  1: The point is on the curve
 *  0: The point is not on the curve
 * -1: An error occurred
 */
int EC_POINT_is_on_curve(const EC_GROUP *group, const EC_POINT *point,
                         BN_CTX *ctx)
{
    if (group->meth->is_on_curve == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->is_on_curve(group, point, ctx);
}

int EC_POINT_cmp(const EC_GROUP *group, const EC_POINT *a, const EC_POINT *b,
                 BN_CTX *ctx)
{
    if (group->meth->point_cmp == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return -1;
    }
    if (!ec_point_is_compat(a, group) || !ec_point_is_compat(b, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return -1;
    }
    return group->meth->point_cmp(group, a, b, ctx);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINT_make_affine(const EC_GROUP *group, EC_POINT *point, BN_CTX *ctx)
{
    if (group->meth->make_affine == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    return group->meth->make_affine(group, point, ctx);
}

int EC_POINTs_make_affine(const EC_GROUP *group, size_t num,
                          EC_POINT *points[], BN_CTX *ctx)
{
    size_t i;

    if (group->meth->points_make_affine == 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    for (i = 0; i < num; i++) {
        if (!ec_point_is_compat(points[i], group)) {
            ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
            return 0;
        }
    }
    return group->meth->points_make_affine(group, num, points, ctx);
}
#endif

/*
 * Functions for point multiplication. If group->meth->mul is 0, we use the
 * wNAF-based implementations in ec_mult.c; otherwise we dispatch through
 * methods.
 */

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINTs_mul(const EC_GROUP *group, EC_POINT *r, const BIGNUM *scalar,
                  size_t num, const EC_POINT *points[],
                  const BIGNUM *scalars[], BN_CTX *ctx)
{
    int ret = 0;
    size_t i = 0;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (!ec_point_is_compat(r, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }

    if (scalar == NULL && num == 0)
        return EC_POINT_set_to_infinity(group, r);

    for (i = 0; i < num; i++) {
        if (!ec_point_is_compat(points[i], group)) {
            ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
            return 0;
        }
    }

#ifndef FIPS_MODULE
    if (ctx == NULL)
        ctx = new_ctx = BN_CTX_secure_new();
#endif
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (group->meth->mul != NULL)
        ret = group->meth->mul(group, r, scalar, num, points, scalars, ctx);
    else
        /* use default */
        ret = ossl_ec_wNAF_mul(group, r, scalar, num, points, scalars, ctx);

#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}
#endif

int EC_POINT_mul(const EC_GROUP *group, EC_POINT *r, const BIGNUM *g_scalar,
                 const EC_POINT *point, const BIGNUM *p_scalar, BN_CTX *ctx)
{
    int ret = 0;
    size_t num;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (!ec_point_is_compat(r, group)
        || (point != NULL && !ec_point_is_compat(point, group))) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }

    if (g_scalar == NULL && p_scalar == NULL)
        return EC_POINT_set_to_infinity(group, r);

#ifndef FIPS_MODULE
    if (ctx == NULL)
        ctx = new_ctx = BN_CTX_secure_new();
#endif
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    num = (point != NULL && p_scalar != NULL) ? 1 : 0;
    if (group->meth->mul != NULL)
        ret = group->meth->mul(group, r, g_scalar, num, &point, &p_scalar, ctx);
    else
        /* use default */
        ret = ossl_ec_wNAF_mul(group, r, g_scalar, num, &point, &p_scalar, ctx);

#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_GROUP_precompute_mult(EC_GROUP *group, BN_CTX *ctx)
{
    if (group->meth->mul == 0)
        /* use default */
        return ossl_ec_wNAF_precompute_mult(group, ctx);

    if (group->meth->precompute_mult != 0)
        return group->meth->precompute_mult(group, ctx);
    else
        return 1;               /* nothing to do, so report success */
}

int EC_GROUP_have_precompute_mult(const EC_GROUP *group)
{
    if (group->meth->mul == 0)
        /* use default */
        return ossl_ec_wNAF_have_precompute_mult(group);

    if (group->meth->have_precompute_mult != 0)
        return group->meth->have_precompute_mult(group);
    else
        return 0;               /* cannot tell whether precomputation has
                                 * been performed */
}
#endif

/*
 * ec_precompute_mont_data sets |group->mont_data| from |group->order| and
 * returns one on success. On error it returns zero.
 */
static int ec_precompute_mont_data(EC_GROUP *group)
{
    BN_CTX *ctx = BN_CTX_new_ex(group->libctx);
    int ret = 0;

    BN_MONT_CTX_free(group->mont_data);
    group->mont_data = NULL;

    if (ctx == NULL)
        goto err;

    group->mont_data = BN_MONT_CTX_new();
    if (group->mont_data == NULL)
        goto err;

    if (!BN_MONT_CTX_set(group->mont_data, group->order, ctx)) {
        BN_MONT_CTX_free(group->mont_data);
        group->mont_data = NULL;
        goto err;
    }

    ret = 1;

 err:

    BN_CTX_free(ctx);
    return ret;
}

#ifndef FIPS_MODULE
int EC_KEY_set_ex_data(EC_KEY *key, int idx, void *arg)
{
    return CRYPTO_set_ex_data(&key->ex_data, idx, arg);
}

void *EC_KEY_get_ex_data(const EC_KEY *key, int idx)
{
    return CRYPTO_get_ex_data(&key->ex_data, idx);
}
#endif

int ossl_ec_group_simple_order_bits(const EC_GROUP *group)
{
    if (group->order == NULL)
        return 0;
    return BN_num_bits(group->order);
}

static int ec_field_inverse_mod_ord(const EC_GROUP *group, BIGNUM *r,
                                    const BIGNUM *x, BN_CTX *ctx)
{
    BIGNUM *e = NULL;
    int ret = 0;
#ifndef FIPS_MODULE
    BN_CTX *new_ctx = NULL;
#endif

    if (group->mont_data == NULL)
        return 0;

#ifndef FIPS_MODULE
    if (ctx == NULL)
        ctx = new_ctx = BN_CTX_secure_new();
#endif
    if (ctx == NULL)
        return 0;

    BN_CTX_start(ctx);
    if ((e = BN_CTX_get(ctx)) == NULL)
        goto err;

    /*-
     * We want inverse in constant time, therefore we utilize the fact
     * order must be prime and use Fermats Little Theorem instead.
     */
    if (!BN_set_word(e, 2))
        goto err;
    if (!BN_sub(e, group->order, e))
        goto err;
    /*-
     * Although the exponent is public we want the result to be
     * fixed top.
     */
    if (!bn_mod_exp_mont_fixed_top(r, x, e, group->order, ctx, group->mont_data))
        goto err;

    ret = 1;

 err:
    BN_CTX_end(ctx);
#ifndef FIPS_MODULE
    BN_CTX_free(new_ctx);
#endif
    return ret;
}

/*-
 * Default behavior, if group->meth->field_inverse_mod_ord is NULL:
 * - When group->order is even, this function returns an error.
 * - When group->order is otherwise composite, the correctness
 *   of the output is not guaranteed.
 * - When x is outside the range [1, group->order), the correctness
 *   of the output is not guaranteed.
 * - Otherwise, this function returns the multiplicative inverse in the
 *   range [1, group->order).
 *
 * EC_METHODs must implement their own field_inverse_mod_ord for
 * other functionality.
 */
int ossl_ec_group_do_inverse_ord(const EC_GROUP *group, BIGNUM *res,
                                 const BIGNUM *x, BN_CTX *ctx)
{
    if (group->meth->field_inverse_mod_ord != NULL)
        return group->meth->field_inverse_mod_ord(group, res, x, ctx);
    else
        return ec_field_inverse_mod_ord(group, res, x, ctx);
}

/*-
 * Coordinate blinding for EC_POINT.
 *
 * The underlying EC_METHOD can optionally implement this function:
 * underlying implementations should return 0 on errors, or 1 on
 * success.
 *
 * This wrapper returns 1 in case the underlying EC_METHOD does not
 * support coordinate blinding.
 */
int ossl_ec_point_blind_coordinates(const EC_GROUP *group, EC_POINT *p,
                                    BN_CTX *ctx)
{
    if (group->meth->blind_coordinates == NULL)
        return 1; /* ignore if not implemented */

    return group->meth->blind_coordinates(group, p, ctx);
}

int EC_GROUP_get_basis_type(const EC_GROUP *group)
{
    int i;

    if (EC_GROUP_get_field_type(group) != NID_X9_62_characteristic_two_field)
        /* everything else is currently not supported */
        return 0;

    /* Find the last non-zero element of group->poly[] */
    for (i = 0;
         i < (int)OSSL_NELEM(group->poly) && group->poly[i] != 0;
         i++)
        continue;

    if (i == 4)
        return NID_X9_62_ppBasis;
    else if (i == 2)
        return NID_X9_62_tpBasis;
    else
        /* everything else is currently not supported */
        return 0;
}

#ifndef OPENSSL_NO_EC2M
int EC_GROUP_get_trinomial_basis(const EC_GROUP *group, unsigned int *k)
{
    if (group == NULL)
        return 0;

    if (EC_GROUP_get_field_type(group) != NID_X9_62_characteristic_two_field
        || !((group->poly[0] != 0) && (group->poly[1] != 0)
             && (group->poly[2] == 0))) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (k)
        *k = group->poly[1];

    return 1;
}

int EC_GROUP_get_pentanomial_basis(const EC_GROUP *group, unsigned int *k1,
                                   unsigned int *k2, unsigned int *k3)
{
    if (group == NULL)
        return 0;

    if (EC_GROUP_get_field_type(group) != NID_X9_62_characteristic_two_field
        || !((group->poly[0] != 0) && (group->poly[1] != 0)
             && (group->poly[2] != 0) && (group->poly[3] != 0)
             && (group->poly[4] == 0))) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (k1)
        *k1 = group->poly[3];
    if (k2)
        *k2 = group->poly[2];
    if (k3)
        *k3 = group->poly[1];

    return 1;
}
#endif

#ifndef FIPS_MODULE
/*
 * Check if the explicit parameters group matches any built-in curves.
 *
 * We create a copy of the group just built, so that we can remove optional
 * fields for the lookup: we do this to avoid the possibility that one of
 * the optional parameters is used to force the library into using a less
 * performant and less secure EC_METHOD instead of the specialized one.
 * In any case, `seed` is not really used in any computation, while a
 * cofactor different from the one in the built-in table is just
 * mathematically wrong anyway and should not be used.
 */
static EC_GROUP *ec_group_explicit_to_named(const EC_GROUP *group,
                                            OSSL_LIB_CTX *libctx,
                                            const char *propq,
                                            BN_CTX *ctx)
{
    EC_GROUP *ret_group = NULL, *dup = NULL;
    int curve_name_nid;

    const EC_POINT *point = EC_GROUP_get0_generator(group);
    const BIGNUM *order = EC_GROUP_get0_order(group);
    int no_seed = (EC_GROUP_get0_seed(group) == NULL);

    if ((dup = EC_GROUP_dup(group)) == NULL
            || EC_GROUP_set_seed(dup, NULL, 0) != 1
            || !EC_GROUP_set_generator(dup, point, order, NULL))
        goto err;
    if ((curve_name_nid = ossl_ec_curve_nid_from_params(dup, ctx)) != NID_undef) {
        /*
         * The input explicit parameters successfully matched one of the
         * built-in curves: often for built-in curves we have specialized
         * methods with better performance and hardening.
         *
         * In this case we replace the `EC_GROUP` created through explicit
         * parameters with one created from a named group.
         */

# ifndef OPENSSL_NO_EC_NISTP_64_GCC_128
        /*
         * NID_wap_wsg_idm_ecid_wtls12 and NID_secp224r1 are both aliases for
         * the same curve, we prefer the SECP nid when matching explicit
         * parameters as that is associated with a specialized EC_METHOD.
         */
        if (curve_name_nid == NID_wap_wsg_idm_ecid_wtls12)
            curve_name_nid = NID_secp224r1;
# endif /* !def(OPENSSL_NO_EC_NISTP_64_GCC_128) */

        ret_group = EC_GROUP_new_by_curve_name_ex(libctx, propq, curve_name_nid);
        if (ret_group == NULL)
            goto err;

        /*
         * Set the flag so that EC_GROUPs created from explicit parameters are
         * serialized using explicit parameters by default.
         */
        EC_GROUP_set_asn1_flag(ret_group, OPENSSL_EC_EXPLICIT_CURVE);

        /*
         * If the input params do not contain the optional seed field we make
         * sure it is not added to the returned group.
         *
         * The seed field is not really used inside libcrypto anyway, and
         * adding it to parsed explicit parameter keys would alter their DER
         * encoding output (because of the extra field) which could impact
         * applications fingerprinting keys by their DER encoding.
         */
        if (no_seed) {
            if (EC_GROUP_set_seed(ret_group, NULL, 0) != 1)
                goto err;
        }
    } else {
        ret_group = (EC_GROUP *)group;
    }
    EC_GROUP_free(dup);
    return ret_group;
err:
    EC_GROUP_free(dup);
    EC_GROUP_free(ret_group);
    return NULL;
}
#endif /* FIPS_MODULE */

static EC_GROUP *group_new_from_name(const OSSL_PARAM *p,
                                     OSSL_LIB_CTX *libctx, const char *propq)
{
    int ok = 0, nid;
    const char *curve_name = NULL;

    switch (p->data_type) {
    case OSSL_PARAM_UTF8_STRING:
        /* The OSSL_PARAM functions have no support for this */
        curve_name = p->data;
        ok = (curve_name != NULL);
        break;
    case OSSL_PARAM_UTF8_PTR:
        ok = OSSL_PARAM_get_utf8_ptr(p, &curve_name);
        break;
    }

    if (ok) {
        nid = ossl_ec_curve_name2nid(curve_name);
        if (nid == NID_undef) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_CURVE);
            return NULL;
        } else {
            return EC_GROUP_new_by_curve_name_ex(libctx, propq, nid);
        }
    }
    return NULL;
}

/* These parameters can be set directly into an EC_GROUP */
int ossl_ec_group_set_params(EC_GROUP *group, const OSSL_PARAM params[])
{
    int encoding_flag = -1, format = -1;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT);
    if (p != NULL) {
        if (!ossl_ec_pt_format_param2id(p, &format)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_FORM);
            return 0;
        }
        EC_GROUP_set_point_conversion_form(group, format);
    }

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_ENCODING);
    if (p != NULL) {
        if (!ossl_ec_encoding_param2id(p, &encoding_flag)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_FORM);
            return 0;
        }
        EC_GROUP_set_asn1_flag(group, encoding_flag);
    }
    /* Optional seed */
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_SEED);
    if (p != NULL) {
        /* The seed is allowed to be NULL */
        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || !EC_GROUP_set_seed(group, p->data, p->data_size)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_SEED);
            return 0;
        }
    }
    return 1;
}

EC_GROUP *EC_GROUP_new_from_params(const OSSL_PARAM params[],
                                   OSSL_LIB_CTX *libctx, const char *propq)
{
    const OSSL_PARAM *ptmp;
    EC_GROUP *group = NULL;

#ifndef FIPS_MODULE
    const OSSL_PARAM *pa, *pb;
    int ok = 0;
    EC_GROUP *named_group = NULL;
    BIGNUM *p = NULL, *a = NULL, *b = NULL, *order = NULL, *cofactor = NULL;
    EC_POINT *point = NULL;
    int field_bits = 0;
    int is_prime_field = 1;
    BN_CTX *bnctx = NULL;
    const unsigned char *buf = NULL;
    int encoding_flag = -1;
#endif

    /* This is the simple named group case */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_GROUP_NAME);
    if (ptmp != NULL) {
        int decoded = 0;

        if ((group = group_new_from_name(ptmp, libctx, propq)) == NULL)
            return NULL;
        if (!ossl_ec_group_set_params(group, params)) {
            EC_GROUP_free(group);
            return NULL;
        }

        ptmp = OSSL_PARAM_locate_const(params,
                                       OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS);
        if (ptmp != NULL && !OSSL_PARAM_get_int(ptmp, &decoded)) {
            ERR_raise(ERR_LIB_EC, EC_R_WRONG_CURVE_PARAMETERS);
            EC_GROUP_free(group);
            return NULL;
        }
        group->decoded_from_explicit_params = decoded > 0;
        return group;
    }
#ifdef FIPS_MODULE
    ERR_raise(ERR_LIB_EC, EC_R_EXPLICIT_PARAMS_NOT_SUPPORTED);
    return NULL;
#else
    /* If it gets here then we are trying explicit parameters */
    bnctx = BN_CTX_new_ex(libctx);
    if (bnctx == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        return 0;
    }
    BN_CTX_start(bnctx);

    p = BN_CTX_get(bnctx);
    a = BN_CTX_get(bnctx);
    b = BN_CTX_get(bnctx);
    order = BN_CTX_get(bnctx);
    if (order == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_FIELD_TYPE);
    if (ptmp == NULL || ptmp->data_type != OSSL_PARAM_UTF8_STRING) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_FIELD);
        goto err;
    }
    if (OPENSSL_strcasecmp(ptmp->data, SN_X9_62_prime_field) == 0) {
        is_prime_field = 1;
    } else if (OPENSSL_strcasecmp(ptmp->data,
                                  SN_X9_62_characteristic_two_field) == 0) {
        is_prime_field = 0;
    } else {
        /* Invalid field */
        ERR_raise(ERR_LIB_EC, EC_R_UNSUPPORTED_FIELD);
        goto err;
    }

    pa = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_A);
    if (!OSSL_PARAM_get_BN(pa, &a)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_A);
        goto err;
    }
    pb = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_B);
    if (!OSSL_PARAM_get_BN(pb, &b)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_B);
        goto err;
    }

    /* extract the prime number or irreducible polynomial */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_P);
    if (!OSSL_PARAM_get_BN(ptmp, &p)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_P);
        goto err;
    }

    if (is_prime_field) {
        if (BN_is_negative(p) || BN_is_zero(p)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_P);
            goto err;
        }
        field_bits = BN_num_bits(p);
        if (field_bits > OPENSSL_ECC_MAX_FIELD_BITS) {
            ERR_raise(ERR_LIB_EC, EC_R_FIELD_TOO_LARGE);
            goto err;
        }

        /* create the EC_GROUP structure */
        group = EC_GROUP_new_curve_GFp(p, a, b, bnctx);
    } else {
# ifdef OPENSSL_NO_EC2M
        ERR_raise(ERR_LIB_EC, EC_R_GF2M_NOT_SUPPORTED);
        goto err;
# else
        /* create the EC_GROUP structure */
        group = EC_GROUP_new_curve_GF2m(p, a, b, NULL);
        if (group != NULL) {
            field_bits = EC_GROUP_get_degree(group);
            if (field_bits > OPENSSL_ECC_MAX_FIELD_BITS) {
                ERR_raise(ERR_LIB_EC, EC_R_FIELD_TOO_LARGE);
                goto err;
            }
        }
# endif /* OPENSSL_NO_EC2M */
    }

    if (group == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    /* Optional seed */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_SEED);
    if (ptmp != NULL) {
        if (ptmp->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_SEED);
            goto err;
        }
        if (!EC_GROUP_set_seed(group, ptmp->data, ptmp->data_size))
            goto err;
    }

    /* generator base point */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_GENERATOR);
    if (ptmp == NULL
        || ptmp->data_type != OSSL_PARAM_OCTET_STRING) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GENERATOR);
        goto err;
    }
    buf = (const unsigned char *)(ptmp->data);
    if ((point = EC_POINT_new(group)) == NULL)
        goto err;
    EC_GROUP_set_point_conversion_form(group,
                                       (point_conversion_form_t)buf[0] & ~0x01);
    if (!EC_POINT_oct2point(group, point, buf, ptmp->data_size, bnctx)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GENERATOR);
        goto err;
    }

    /* order */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_ORDER);
    if (!OSSL_PARAM_get_BN(ptmp, &order)
        || (BN_is_negative(order) || BN_is_zero(order))
        || (BN_num_bits(order) > (int)field_bits + 1)) { /* Hasse bound */
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GROUP_ORDER);
        goto err;
    }

    /* Optional cofactor */
    ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_COFACTOR);
    if (ptmp != NULL) {
        cofactor = BN_CTX_get(bnctx);
        if (cofactor == NULL || !OSSL_PARAM_get_BN(ptmp, &cofactor)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_COFACTOR);
            goto err;
        }
    }

    /* set the generator, order and cofactor (if present) */
    if (!EC_GROUP_set_generator(group, point, order, cofactor)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GENERATOR);
        goto err;
    }

    named_group = ec_group_explicit_to_named(group, libctx, propq, bnctx);
    if (named_group == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_NAMED_GROUP_CONVERSION);
        goto err;
    }
    if (named_group == group) {
        /*
         * If we did not find a named group then the encoding should be explicit
         * if it was specified
         */
        ptmp = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_ENCODING);
        if (ptmp != NULL
            && !ossl_ec_encoding_param2id(ptmp, &encoding_flag)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
            goto err;
        }
        if (encoding_flag == OPENSSL_EC_NAMED_CURVE) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
            goto err;
        }
        EC_GROUP_set_asn1_flag(group, OPENSSL_EC_EXPLICIT_CURVE);
    } else {
        EC_GROUP_free(group);
        group = named_group;
    }
    /* We've imported the group from explicit parameters, set it so. */
    group->decoded_from_explicit_params = 1;
    ok = 1;
 err:
    if (!ok) {
        EC_GROUP_free(group);
        group = NULL;
    }
    EC_POINT_free(point);
    BN_CTX_end(bnctx);
    BN_CTX_free(bnctx);

    return group;
#endif /* FIPS_MODULE */
}

OSSL_PARAM *EC_GROUP_to_params(const EC_GROUP *group, OSSL_LIB_CTX *libctx,
                               const char *propq, BN_CTX *bnctx)
{
    OSSL_PARAM_BLD *tmpl = NULL;
    BN_CTX *new_bnctx = NULL;
    unsigned char *gen_buf = NULL;
    OSSL_PARAM *params = NULL;

    if (group == NULL)
        goto err;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        goto err;

    if (bnctx == NULL)
        bnctx = new_bnctx = BN_CTX_new_ex(libctx);
    if (bnctx == NULL)
        goto err;
    BN_CTX_start(bnctx);

    if (!ossl_ec_group_todata(
            group, tmpl, NULL, libctx, propq, bnctx, &gen_buf))
        goto err;

    params = OSSL_PARAM_BLD_to_param(tmpl);

 err:
    OSSL_PARAM_BLD_free(tmpl);
    OPENSSL_free(gen_buf);
    BN_CTX_end(bnctx);
    BN_CTX_free(new_bnctx);
    return params;
}
