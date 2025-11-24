/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 */

/*
 * SM2 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/err.h>
#include "crypto/bn.h"
#include "ec_local.h"
#include "internal/common.h"
#include "internal/constant_time.h"

#define P256_LIMBS (256 / BN_BITS2)

#if !defined(OPENSSL_NO_SM2_PRECOMP)
extern const BN_ULONG ecp_sm2p256_precomputed[8 * 32 * 256];
#endif

typedef struct {
    BN_ULONG X[P256_LIMBS];
    BN_ULONG Y[P256_LIMBS];
    BN_ULONG Z[P256_LIMBS];
} P256_POINT;

typedef struct {
    BN_ULONG X[P256_LIMBS];
    BN_ULONG Y[P256_LIMBS];
} P256_POINT_AFFINE;

#if !defined(OPENSSL_NO_SM2_PRECOMP)
/* Coordinates of G, for which we have precomputed tables */
ALIGN32 static const BN_ULONG def_xG[P256_LIMBS] = {
    0x715a4589334c74c7, 0x8fe30bbff2660be1,
    0x5f9904466a39c994, 0x32c4ae2c1f198119
};

ALIGN32 static const BN_ULONG def_yG[P256_LIMBS] = {
    0x02df32e52139f0a0, 0xd0a9877cc62a4740,
    0x59bdcee36b692153, 0xbc3736a2f4f6779c,
};
#endif

/* p and order for SM2 according to GB/T 32918.5-2017 */
ALIGN32 static const BN_ULONG def_p[P256_LIMBS] = {
    0xffffffffffffffff, 0xffffffff00000000,
    0xffffffffffffffff, 0xfffffffeffffffff
};
ALIGN32 static const BN_ULONG def_ord[P256_LIMBS] = {
    0x53bbf40939d54123, 0x7203df6b21c6052b,
    0xffffffffffffffff, 0xfffffffeffffffff
};

ALIGN32 static const BN_ULONG ONE[P256_LIMBS] = {1, 0, 0, 0};

/* Functions implemented in assembly */
/*
 * Most of below mentioned functions *preserve* the property of inputs
 * being fully reduced, i.e. being in [0, modulus) range. Simply put if
 * inputs are fully reduced, then output is too.
 */
/* Right shift: a >> 1 */
void bn_rshift1(BN_ULONG *a);
/* Sub: r = a - b */
void bn_sub(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
/* Modular div by 2: r = a / 2 mod p */
void ecp_sm2p256_div_by_2(BN_ULONG *r, const BN_ULONG *a);
/* Modular div by 2: r = a / 2 mod n, where n = ord(p) */
void ecp_sm2p256_div_by_2_mod_ord(BN_ULONG *r, const BN_ULONG *a);
/* Modular add: r = a + b mod p */
void ecp_sm2p256_add(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
/* Modular sub: r = a - b mod p */
void ecp_sm2p256_sub(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
/* Modular sub: r = a - b mod n, where n = ord(p) */
void ecp_sm2p256_sub_mod_ord(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
/* Modular mul by 3: out = 3 * a mod p */
void ecp_sm2p256_mul_by_3(BN_ULONG *r, const BN_ULONG *a);
/* Modular mul: r = a * b mod p */
void ecp_sm2p256_mul(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
/* Modular sqr: r = a ^ 2 mod p */
void ecp_sm2p256_sqr(BN_ULONG *r, const BN_ULONG *a);

static ossl_inline BN_ULONG is_zeros(const BN_ULONG *a)
{
    BN_ULONG res;

    res = a[0] | a[1] | a[2] | a[3];

    return constant_time_is_zero_64(res);
}

static ossl_inline int is_equal(const BN_ULONG *a, const BN_ULONG *b)
{
    BN_ULONG res;

    res = a[0] ^ b[0];
    res |= a[1] ^ b[1];
    res |= a[2] ^ b[2];
    res |= a[3] ^ b[3];

    return constant_time_is_zero_64(res);
}

static ossl_inline int is_greater(const BN_ULONG *a, const BN_ULONG *b)
{
    int i;

    for (i = P256_LIMBS - 1; i >= 0; --i) {
        if (a[i] > b[i])
            return 1;
        if (a[i] < b[i])
            return -1;
    }

    return 0;
}

#define is_one(a) is_equal(a, ONE)
#define is_even(a) !(a[0] & 1)
#define is_point_equal(a, b)     \
    is_equal(a->X, b->X) &&      \
    is_equal(a->Y, b->Y) &&      \
    is_equal(a->Z, b->Z)

/* Bignum and field elements conversion */
#define ecp_sm2p256_bignum_field_elem(out, in) \
    bn_copy_words(out, in, P256_LIMBS)

/* Binary algorithm for inversion in Fp */
#define BN_MOD_INV(out, in, mod_div, mod_sub, mod) \
    do {                                           \
        ALIGN32 BN_ULONG u[4];                     \
        ALIGN32 BN_ULONG v[4];                     \
        ALIGN32 BN_ULONG x1[4] = {1, 0, 0, 0};     \
        ALIGN32 BN_ULONG x2[4] = {0};              \
                                                   \
        if (is_zeros(in))                          \
            return;                                \
        memcpy(u, in, 32);                         \
        memcpy(v, mod, 32);                        \
        while (!is_one(u) && !is_one(v)) {         \
            while (is_even(u)) {                   \
                bn_rshift1(u);                     \
                mod_div(x1, x1);                   \
            }                                      \
            while (is_even(v)) {                   \
                bn_rshift1(v);                     \
                mod_div(x2, x2);                   \
            }                                      \
            if (is_greater(u, v) == 1) {           \
                bn_sub(u, u, v);                   \
                mod_sub(x1, x1, x2);               \
            } else {                               \
                bn_sub(v, v, u);                   \
                mod_sub(x2, x2, x1);               \
            }                                      \
        }                                          \
        if (is_one(u))                             \
            memcpy(out, x1, 32);                   \
        else                                       \
            memcpy(out, x2, 32);                   \
    } while (0)

/* Modular inverse |out| = |in|^(-1) mod |p|. */
static ossl_inline void ecp_sm2p256_mod_inverse(BN_ULONG* out,
                                                const BN_ULONG* in) {
    BN_MOD_INV(out, in, ecp_sm2p256_div_by_2, ecp_sm2p256_sub, def_p);
}

/* Modular inverse mod order |out| = |in|^(-1) % |ord|. */
static ossl_inline void ecp_sm2p256_mod_ord_inverse(BN_ULONG* out,
                                                    const BN_ULONG* in) {
    BN_MOD_INV(out, in, ecp_sm2p256_div_by_2_mod_ord, ecp_sm2p256_sub_mod_ord,
               def_ord);
}

/* Point double: R <- P + P */
static void ecp_sm2p256_point_double(P256_POINT *R, const P256_POINT *P)
{
    unsigned int i;
    ALIGN32 BN_ULONG tmp0[P256_LIMBS];
    ALIGN32 BN_ULONG tmp1[P256_LIMBS];
    ALIGN32 BN_ULONG tmp2[P256_LIMBS];

    /* zero-check P->Z */
    if (is_zeros(P->Z)) {
        for (i = 0; i < P256_LIMBS; ++i)
            R->Z[i] = 0;

        return;
    }

    ecp_sm2p256_sqr(tmp0, P->Z);
    ecp_sm2p256_sub(tmp1, P->X, tmp0);
    ecp_sm2p256_add(tmp0, P->X, tmp0);
    ecp_sm2p256_mul(tmp1, tmp1, tmp0);
    ecp_sm2p256_mul_by_3(tmp1, tmp1);
    ecp_sm2p256_add(R->Y, P->Y, P->Y);
    ecp_sm2p256_mul(R->Z, R->Y, P->Z);
    ecp_sm2p256_sqr(R->Y, R->Y);
    ecp_sm2p256_mul(tmp2, R->Y, P->X);
    ecp_sm2p256_sqr(R->Y, R->Y);
    ecp_sm2p256_div_by_2(R->Y, R->Y);
    ecp_sm2p256_sqr(R->X, tmp1);
    ecp_sm2p256_add(tmp0, tmp2, tmp2);
    ecp_sm2p256_sub(R->X, R->X, tmp0);
    ecp_sm2p256_sub(tmp0, tmp2, R->X);
    ecp_sm2p256_mul(tmp0, tmp0, tmp1);
    ecp_sm2p256_sub(tmp1, tmp0, R->Y);
    memcpy(R->Y, tmp1, 32);
}

/* Point add affine: R <- P + Q */
static void ecp_sm2p256_point_add_affine(P256_POINT *R, const P256_POINT *P,
                                         const P256_POINT_AFFINE *Q)
{
    unsigned int i;
    ALIGN32 BN_ULONG tmp0[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG tmp1[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG tmp2[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG tmp3[P256_LIMBS] = {0};

    /* zero-check P->Z */
    if (is_zeros(P->Z)) {
        for (i = 0; i < P256_LIMBS; ++i) {
            R->X[i] = Q->X[i];
            R->Y[i] = Q->Y[i];
            R->Z[i] = 0;
        }
        R->Z[0] = 1;

        return;
    }

    ecp_sm2p256_sqr(tmp0, P->Z);
    ecp_sm2p256_mul(tmp1, tmp0, P->Z);
    ecp_sm2p256_mul(tmp0, tmp0, Q->X);
    ecp_sm2p256_mul(tmp1, tmp1, Q->Y);
    ecp_sm2p256_sub(tmp0, tmp0, P->X);
    ecp_sm2p256_sub(tmp1, tmp1, P->Y);

    /* zero-check tmp0, tmp1 */
    if (is_zeros(tmp0)) {
        if (is_zeros(tmp1)) {
            P256_POINT K;

            for (i = 0; i < P256_LIMBS; ++i) {
                K.X[i] = Q->X[i];
                K.Y[i] = Q->Y[i];
                K.Z[i] = 0;
            }
            K.Z[0] = 1;
            ecp_sm2p256_point_double(R, &K);
        } else {
            for (i = 0; i < P256_LIMBS; ++i)
                R->Z[i] = 0;
        }

        return;
    }

    ecp_sm2p256_mul(R->Z, P->Z, tmp0);
    ecp_sm2p256_sqr(tmp2, tmp0);
    ecp_sm2p256_mul(tmp3, tmp2, tmp0);
    ecp_sm2p256_mul(tmp2, tmp2, P->X);
    ecp_sm2p256_add(tmp0, tmp2, tmp2);
    ecp_sm2p256_sqr(R->X, tmp1);
    ecp_sm2p256_sub(R->X, R->X, tmp0);
    ecp_sm2p256_sub(R->X, R->X, tmp3);
    ecp_sm2p256_sub(tmp2, tmp2, R->X);
    ecp_sm2p256_mul(tmp2, tmp2, tmp1);
    ecp_sm2p256_mul(tmp3, tmp3, P->Y);
    ecp_sm2p256_sub(R->Y, tmp2, tmp3);
}

/* Point add: R <- P + Q */
static void ecp_sm2p256_point_add(P256_POINT *R, const P256_POINT *P,
                                  const P256_POINT *Q)
{
    unsigned int i;
    ALIGN32 BN_ULONG tmp0[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG tmp1[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG tmp2[P256_LIMBS] = {0};

    /* zero-check P | Q ->Z */
    if (is_zeros(P->Z)) {
        for (i = 0; i < P256_LIMBS; ++i) {
            R->X[i] = Q->X[i];
            R->Y[i] = Q->Y[i];
            R->Z[i] = Q->Z[i];
        }

        return;
    } else if (is_zeros(Q->Z)) {
        for (i = 0; i < P256_LIMBS; ++i) {
            R->X[i] = P->X[i];
            R->Y[i] = P->Y[i];
            R->Z[i] = P->Z[i];
        }

        return;
    } else if (is_point_equal(P, Q)) {
        ecp_sm2p256_point_double(R, Q);

        return;
    }

    ecp_sm2p256_sqr(tmp0, P->Z);
    ecp_sm2p256_mul(tmp1, tmp0, P->Z);
    ecp_sm2p256_mul(tmp0, tmp0, Q->X);
    ecp_sm2p256_mul(tmp1, tmp1, Q->Y);
    ecp_sm2p256_mul(R->Y, P->Y, Q->Z);
    ecp_sm2p256_mul(R->Z, Q->Z, P->Z);
    ecp_sm2p256_sqr(tmp2, Q->Z);
    ecp_sm2p256_mul(R->Y, tmp2, R->Y);
    ecp_sm2p256_mul(R->X, tmp2, P->X);
    ecp_sm2p256_sub(tmp0, tmp0, R->X);
    ecp_sm2p256_mul(R->Z, tmp0, R->Z);
    ecp_sm2p256_sub(tmp1, tmp1, R->Y);
    ecp_sm2p256_sqr(tmp2, tmp0);
    ecp_sm2p256_mul(tmp0, tmp0, tmp2);
    ecp_sm2p256_mul(tmp2, tmp2, R->X);
    ecp_sm2p256_sqr(R->X, tmp1);
    ecp_sm2p256_sub(R->X, R->X, tmp2);
    ecp_sm2p256_sub(R->X, R->X, tmp2);
    ecp_sm2p256_sub(R->X, R->X, tmp0);
    ecp_sm2p256_sub(tmp2, tmp2, R->X);
    ecp_sm2p256_mul(tmp2, tmp1, tmp2);
    ecp_sm2p256_mul(tmp0, tmp0, R->Y);
    ecp_sm2p256_sub(R->Y, tmp2, tmp0);
}

#if !defined(OPENSSL_NO_SM2_PRECOMP)
/* Base point mul by scalar: k - scalar, G - base point */
static void ecp_sm2p256_point_G_mul_by_scalar(P256_POINT *R, const BN_ULONG *k)
{
    unsigned int i, index, mask = 0xff;
    P256_POINT_AFFINE Q;

    memset(R, 0, sizeof(P256_POINT));

    if (is_zeros(k))
        return;

    index = k[0] & mask;
    if (index) {
        index = index * 8;
        memcpy(R->X, ecp_sm2p256_precomputed + index, 32);
        memcpy(R->Y, ecp_sm2p256_precomputed + index + P256_LIMBS, 32);
        R->Z[0] = 1;
    }

    for (i = 1; i < 32; ++i) {
        index = (k[i / 8] >> (8 * (i % 8))) & mask;

        if (index) {
            index = index + i * 256;
            index = index * 8;
            memcpy(Q.X, ecp_sm2p256_precomputed + index, 32);
            memcpy(Q.Y, ecp_sm2p256_precomputed + index + P256_LIMBS, 32);
            ecp_sm2p256_point_add_affine(R, R, &Q);
        }
    }
}
#endif

/*
 * Affine point mul by scalar: k - scalar, P - affine point
 */
static void ecp_sm2p256_point_P_mul_by_scalar(P256_POINT *R, const BN_ULONG *k,
                                              P256_POINT_AFFINE P)
{
    int i, init = 0;
    unsigned int index, mask = 0x0f;
    ALIGN64 P256_POINT precomputed[16];

    memset(R, 0, sizeof(P256_POINT));

    if (is_zeros(k))
        return;

    /* The first value of the precomputed table is P. */
    memcpy(precomputed[1].X, P.X, 32);
    memcpy(precomputed[1].Y, P.Y, 32);
    precomputed[1].Z[0] = 1;
    precomputed[1].Z[1] = 0;
    precomputed[1].Z[2] = 0;
    precomputed[1].Z[3] = 0;

    /* The second value of the precomputed table is 2P. */
    ecp_sm2p256_point_double(&precomputed[2], &precomputed[1]);

    /* The subsequent elements are 3P, 4P, and so on. */
    for (i = 3; i < 16; ++i)
        ecp_sm2p256_point_add_affine(&precomputed[i], &precomputed[i - 1], &P);

    for (i = 64 - 1; i >= 0; --i) {
        index = (k[i / 16] >> (4 * (i % 16))) & mask;

        if (init == 0) {
            if (index) {
                memcpy(R, &precomputed[index], sizeof(P256_POINT));
                init = 1;
            }
        } else {
            ecp_sm2p256_point_double(R, R);
            ecp_sm2p256_point_double(R, R);
            ecp_sm2p256_point_double(R, R);
            ecp_sm2p256_point_double(R, R);
            if (index)
                ecp_sm2p256_point_add(R, R, &precomputed[index]);
        }
    }
}

/* Get affine point */
static void ecp_sm2p256_point_get_affine(P256_POINT_AFFINE *R,
                                         const P256_POINT *P)
{
    ALIGN32 BN_ULONG z_inv3[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG z_inv2[P256_LIMBS] = {0};

    if (is_one(P->Z)) {
        memcpy(R->X, P->X, 32);
        memcpy(R->Y, P->Y, 32);
        return;
    }

    ecp_sm2p256_mod_inverse(z_inv3, P->Z);
    ecp_sm2p256_sqr(z_inv2, z_inv3);
    ecp_sm2p256_mul(R->X, P->X, z_inv2);
    ecp_sm2p256_mul(z_inv3, z_inv3, z_inv2);
    ecp_sm2p256_mul(R->Y, P->Y, z_inv3);
}

#if !defined(OPENSSL_NO_SM2_PRECOMP)
static int ecp_sm2p256_is_affine_G(const EC_POINT *generator)
{
    return (bn_get_top(generator->X) == P256_LIMBS)
            && (bn_get_top(generator->Y) == P256_LIMBS)
            && is_equal(bn_get_words(generator->X), def_xG)
            && is_equal(bn_get_words(generator->Y), def_yG)
            && (generator->Z_is_one == 1);
}
#endif

/*
 * Convert Jacobian coordinate point into affine coordinate (x,y)
 */
static int ecp_sm2p256_get_affine(const EC_GROUP *group,
                                  const EC_POINT *point,
                                  BIGNUM *x, BIGNUM *y, BN_CTX *ctx)
{
    ALIGN32 BN_ULONG z_inv2[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG z_inv3[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG x_aff[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG y_aff[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG point_x[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG point_y[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG point_z[P256_LIMBS] = {0};

    if (EC_POINT_is_at_infinity(group, point)) {
        ECerr(ERR_LIB_EC, EC_R_POINT_AT_INFINITY);
        return 0;
    }

    if (ecp_sm2p256_bignum_field_elem(point_x, point->X) <= 0
        || ecp_sm2p256_bignum_field_elem(point_y, point->Y) <= 0
        || ecp_sm2p256_bignum_field_elem(point_z, point->Z) <= 0) {
        ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        return 0;
    }

    ecp_sm2p256_mod_inverse(z_inv3, point_z);
    ecp_sm2p256_sqr(z_inv2, z_inv3);

    if (x != NULL) {
        ecp_sm2p256_mul(x_aff, point_x, z_inv2);
        if (!bn_set_words(x, x_aff, P256_LIMBS))
            return 0;
    }

    if (y != NULL) {
        ecp_sm2p256_mul(z_inv3, z_inv3, z_inv2);
        ecp_sm2p256_mul(y_aff, point_y, z_inv3);
        if (!bn_set_words(y, y_aff, P256_LIMBS))
            return 0;
    }

    return 1;
}

/* r = sum(scalar[i]*point[i]) */
static int ecp_sm2p256_windowed_mul(const EC_GROUP *group,
                                    P256_POINT *r,
                                    const BIGNUM **scalar,
                                    const EC_POINT **point,
                                    size_t num, BN_CTX *ctx)
{
    unsigned int i;
    int ret = 0;
    const BIGNUM **scalars = NULL;
    ALIGN32 BN_ULONG k[P256_LIMBS] = {0};
    P256_POINT kP;
    ALIGN32 union {
        P256_POINT p;
        P256_POINT_AFFINE a;
    } t, p;

    if (num > OPENSSL_MALLOC_MAX_NELEMS(P256_POINT)
        || (scalars = OPENSSL_malloc(num * sizeof(BIGNUM *))) == NULL) {
        ECerr(ERR_LIB_EC, ERR_R_MALLOC_FAILURE);
        goto err;
    }

    memset(r, 0, sizeof(P256_POINT));

    for (i = 0; i < num; i++) {
        if (EC_POINT_is_at_infinity(group, point[i]))
            continue;

        if ((BN_num_bits(scalar[i]) > 256) || BN_is_negative(scalar[i])) {
            BIGNUM *tmp;

            if ((tmp = BN_CTX_get(ctx)) == NULL)
                goto err;
            if (!BN_nnmod(tmp, scalar[i], group->order, ctx)) {
                ECerr(ERR_LIB_EC, ERR_R_BN_LIB);
                goto err;
            }
            scalars[i] = tmp;
        } else {
            scalars[i] = scalar[i];
        }

        if (ecp_sm2p256_bignum_field_elem(k, scalars[i]) <= 0
            || ecp_sm2p256_bignum_field_elem(p.p.X, point[i]->X) <= 0
            || ecp_sm2p256_bignum_field_elem(p.p.Y, point[i]->Y) <= 0
            || ecp_sm2p256_bignum_field_elem(p.p.Z, point[i]->Z) <= 0) {
            ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
            goto err;
        }

        ecp_sm2p256_point_get_affine(&t.a, &p.p);
        ecp_sm2p256_point_P_mul_by_scalar(&kP, k, t.a);
        ecp_sm2p256_point_add(r, r, &kP);
    }

    ret = 1;
err:
    OPENSSL_free(scalars);
    return ret;
}

/* r = scalar*G + sum(scalars[i]*points[i]) */
static int ecp_sm2p256_points_mul(const EC_GROUP *group,
                                  EC_POINT *r,
                                  const BIGNUM *scalar,
                                  size_t num,
                                  const EC_POINT *points[],
                                  const BIGNUM *scalars[], BN_CTX *ctx)
{
    int ret = 0, p_is_infinity = 0;
    const EC_POINT *generator = NULL;
    ALIGN32 BN_ULONG k[P256_LIMBS] = {0};
    ALIGN32 union {
        P256_POINT p;
        P256_POINT_AFFINE a;
    } t, p;

    if ((num + 1) == 0 || (num + 1) > OPENSSL_MALLOC_MAX_NELEMS(void *)) {
        ECerr(ERR_LIB_EC, ERR_R_MALLOC_FAILURE);
        goto err;
    }

    BN_CTX_start(ctx);

    if (scalar) {
        generator = EC_GROUP_get0_generator(group);
        if (generator == NULL) {
            ECerr(ERR_LIB_EC, EC_R_UNDEFINED_GENERATOR);
            goto err;
        }

        if (!ecp_sm2p256_bignum_field_elem(k, scalar)) {
            ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
            goto err;
        }
#if !defined(OPENSSL_NO_SM2_PRECOMP)
        if (ecp_sm2p256_is_affine_G(generator)) {
            ecp_sm2p256_point_G_mul_by_scalar(&p.p, k);
        } else
#endif
        {
            /* if no precomputed table */
            const EC_POINT *new_generator[1];
            const BIGNUM *g_scalars[1];

            new_generator[0] = generator;
            g_scalars[0] = scalar;

            if (!ecp_sm2p256_windowed_mul(group, &p.p, g_scalars, new_generator,
                                          (new_generator[0] != NULL
                                           && g_scalars[0] != NULL), ctx))
                goto err;
        }
    } else {
        p_is_infinity = 1;
    }
    if (num) {
        P256_POINT *out = &t.p;

        if (p_is_infinity)
            out = &p.p;

        if (!ecp_sm2p256_windowed_mul(group, out, scalars, points, num, ctx))
            goto err;

        if (!p_is_infinity)
            ecp_sm2p256_point_add(&p.p, &p.p, out);
    }

    /* Not constant-time, but we're only operating on the public output. */
    if (!bn_set_words(r->X, p.p.X, P256_LIMBS)
        || !bn_set_words(r->Y, p.p.Y, P256_LIMBS)
        || !bn_set_words(r->Z, p.p.Z, P256_LIMBS))
        goto err;
    r->Z_is_one = is_equal(bn_get_words(r->Z), ONE) & 1;

    ret = 1;
err:
    BN_CTX_end(ctx);
    return ret;
}

static int ecp_sm2p256_field_mul(const EC_GROUP *group, BIGNUM *r,
                                 const BIGNUM *a, const BIGNUM *b, BN_CTX *ctx)
{
    ALIGN32 BN_ULONG a_fe[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG b_fe[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG r_fe[P256_LIMBS] = {0};

    if (a == NULL || b == NULL || r == NULL)
        return 0;

    if (!ecp_sm2p256_bignum_field_elem(a_fe, a)
        || !ecp_sm2p256_bignum_field_elem(b_fe, b)) {
        ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        return 0;
    }

    ecp_sm2p256_mul(r_fe, a_fe, b_fe);

    if (!bn_set_words(r, r_fe, P256_LIMBS))
        return 0;

    return 1;
}

static int ecp_sm2p256_field_sqr(const EC_GROUP *group, BIGNUM *r,
                                 const BIGNUM *a, BN_CTX *ctx)
{
    ALIGN32 BN_ULONG a_fe[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG r_fe[P256_LIMBS] = {0};

    if (a == NULL || r == NULL)
        return 0;

    if (!ecp_sm2p256_bignum_field_elem(a_fe, a)) {
        ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        return 0;
    }

    ecp_sm2p256_sqr(r_fe, a_fe);

    if (!bn_set_words(r, r_fe, P256_LIMBS))
        return 0;

    return 1;
}

static int ecp_sm2p256_inv_mod_ord(const EC_GROUP *group, BIGNUM *r,
                                             const BIGNUM *x, BN_CTX *ctx)
{
    int ret = 0;
    ALIGN32 BN_ULONG t[P256_LIMBS] = {0};
    ALIGN32 BN_ULONG out[P256_LIMBS] = {0};

    if (bn_wexpand(r, P256_LIMBS) == NULL) {
        ECerr(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    if ((BN_num_bits(x) > 256) || BN_is_negative(x)) {
        BIGNUM *tmp;

        if ((tmp = BN_CTX_get(ctx)) == NULL
            || !BN_nnmod(tmp, x, group->order, ctx)) {
            ECerr(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
        x = tmp;
    }

    if (!ecp_sm2p256_bignum_field_elem(t, x)) {
        ECerr(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        goto err;
    }

    ecp_sm2p256_mod_ord_inverse(out, t);

    if (!bn_set_words(r, out, P256_LIMBS))
        goto err;

    ret = 1;
err:
    return ret;
}

const EC_METHOD *EC_GFp_sm2p256_method(void)
{
    static const EC_METHOD ret = {
        EC_FLAGS_DEFAULT_OCT,
        NID_X9_62_prime_field,
        ossl_ec_GFp_simple_group_init,
        ossl_ec_GFp_simple_group_finish,
        ossl_ec_GFp_simple_group_clear_finish,
        ossl_ec_GFp_simple_group_copy,
        ossl_ec_GFp_simple_group_set_curve,
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
        ecp_sm2p256_get_affine,
        0, 0, 0,
        ossl_ec_GFp_simple_add,
        ossl_ec_GFp_simple_dbl,
        ossl_ec_GFp_simple_invert,
        ossl_ec_GFp_simple_is_at_infinity,
        ossl_ec_GFp_simple_is_on_curve,
        ossl_ec_GFp_simple_cmp,
        ossl_ec_GFp_simple_make_affine,
        ossl_ec_GFp_simple_points_make_affine,
        ecp_sm2p256_points_mul, /* mul */
        0 /* precompute_mult */,
        0 /* have_precompute_mult */,
        ecp_sm2p256_field_mul,
        ecp_sm2p256_field_sqr,
        0 /* field_div */,
        0 /* field_inv */,
        0 /* field_encode */,
        0 /* field_decode */,
        0 /* field_set_to_one */,
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
        ecp_sm2p256_inv_mod_ord,
        0, /* blind_coordinates */
        0, /* ladder_pre */
        0, /* ladder_step */
        0  /* ladder_post */
    };

    return &ret;
}
