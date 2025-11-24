/*
 * Copyright 2002-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * EC_KEY low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/cryptlib.h"
#include <string.h>
#include "ec_local.h"
#include "internal/refcount.h"
#include <openssl/err.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include <openssl/self_test.h>
#include "prov/providercommon.h"
#include "prov/ecx.h"
#include "crypto/bn.h"

static int ecdsa_keygen_pairwise_test(EC_KEY *eckey, OSSL_CALLBACK *cb,
                                      void *cbarg);

#ifndef FIPS_MODULE
EC_KEY *EC_KEY_new(void)
{
    return ossl_ec_key_new_method_int(NULL, NULL, NULL);
}
#endif

EC_KEY *EC_KEY_new_ex(OSSL_LIB_CTX *ctx, const char *propq)
{
    return ossl_ec_key_new_method_int(ctx, propq, NULL);
}

EC_KEY *EC_KEY_new_by_curve_name_ex(OSSL_LIB_CTX *ctx, const char *propq,
                                    int nid)
{
    EC_KEY *ret = EC_KEY_new_ex(ctx, propq);
    if (ret == NULL)
        return NULL;
    ret->group = EC_GROUP_new_by_curve_name_ex(ctx, propq, nid);
    if (ret->group == NULL) {
        EC_KEY_free(ret);
        return NULL;
    }
    if (ret->meth->set_group != NULL
        && ret->meth->set_group(ret, ret->group) == 0) {
        EC_KEY_free(ret);
        return NULL;
    }
    return ret;
}

#ifndef FIPS_MODULE
EC_KEY *EC_KEY_new_by_curve_name(int nid)
{
    return EC_KEY_new_by_curve_name_ex(NULL, NULL, nid);
}
#endif

void EC_KEY_free(EC_KEY *r)
{
    int i;

    if (r == NULL)
        return;

    CRYPTO_DOWN_REF(&r->references, &i);
    REF_PRINT_COUNT("EC_KEY", i, r);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    if (r->meth != NULL && r->meth->finish != NULL)
        r->meth->finish(r);

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    ENGINE_finish(r->engine);
#endif

    if (r->group && r->group->meth->keyfinish)
        r->group->meth->keyfinish(r);

#ifndef FIPS_MODULE
    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_EC_KEY, r, &r->ex_data);
#endif
    CRYPTO_FREE_REF(&r->references);
    EC_GROUP_free(r->group);
    EC_POINT_free(r->pub_key);
    BN_clear_free(r->priv_key);
    OPENSSL_free(r->propq);

    OPENSSL_clear_free((void *)r, sizeof(EC_KEY));
}

EC_KEY *EC_KEY_copy(EC_KEY *dest, const EC_KEY *src)
{
    if (dest == NULL || src == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if (src->meth != dest->meth) {
        if (dest->meth->finish != NULL)
            dest->meth->finish(dest);
        if (dest->group && dest->group->meth->keyfinish)
            dest->group->meth->keyfinish(dest);
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
        if (ENGINE_finish(dest->engine) == 0)
            return 0;
        dest->engine = NULL;
#endif
    }
    dest->libctx = src->libctx;
    /* copy the parameters */
    if (src->group != NULL) {
        /* clear the old group */
        EC_GROUP_free(dest->group);
        dest->group = ossl_ec_group_new_ex(src->libctx, src->propq,
                                           src->group->meth);
        if (dest->group == NULL)
            return NULL;
        if (!EC_GROUP_copy(dest->group, src->group))
            return NULL;

        /*  copy the public key */
        if (src->pub_key != NULL) {
            EC_POINT_free(dest->pub_key);
            dest->pub_key = EC_POINT_new(src->group);
            if (dest->pub_key == NULL)
                return NULL;
            if (!EC_POINT_copy(dest->pub_key, src->pub_key))
                return NULL;
        }
        /* copy the private key */
        if (src->priv_key != NULL) {
            if (dest->priv_key == NULL) {
                dest->priv_key = BN_new();
                if (dest->priv_key == NULL)
                    return NULL;
            }
            if (!BN_copy(dest->priv_key, src->priv_key))
                return NULL;
            if (src->group->meth->keycopy
                && src->group->meth->keycopy(dest, src) == 0)
                return NULL;
        }
    }


    /* copy the rest */
    dest->enc_flag = src->enc_flag;
    dest->conv_form = src->conv_form;
    dest->version = src->version;
    dest->flags = src->flags;
#ifndef FIPS_MODULE
    if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_EC_KEY,
                            &dest->ex_data, &src->ex_data))
        return NULL;
#endif

    if (src->meth != dest->meth) {
#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
        if (src->engine != NULL && ENGINE_init(src->engine) == 0)
            return NULL;
        dest->engine = src->engine;
#endif
        dest->meth = src->meth;
    }

    if (src->meth->copy != NULL && src->meth->copy(dest, src) == 0)
        return NULL;

    dest->dirty_cnt++;

    return dest;
}

EC_KEY *EC_KEY_dup(const EC_KEY *ec_key)
{
    return ossl_ec_key_dup(ec_key, OSSL_KEYMGMT_SELECT_ALL);
}

int EC_KEY_up_ref(EC_KEY *r)
{
    int i;

    if (CRYPTO_UP_REF(&r->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("EC_KEY", i, r);
    REF_ASSERT_ISNT(i < 2);
    return ((i > 1) ? 1 : 0);
}

ENGINE *EC_KEY_get0_engine(const EC_KEY *eckey)
{
    return eckey->engine;
}

int EC_KEY_generate_key(EC_KEY *eckey)
{
    if (eckey == NULL || eckey->group == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (eckey->meth->keygen != NULL) {
        int ret;

        ret = eckey->meth->keygen(eckey);
        if (ret == 1)
            eckey->dirty_cnt++;

        return ret;
    }
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return 0;
}

int ossl_ec_key_gen(EC_KEY *eckey)
{
    int ret;

    ret = eckey->group->meth->keygen(eckey);

    if (ret == 1)
        eckey->dirty_cnt++;
    return ret;
}

/*
 * Refer: FIPS 140-3 IG 10.3.A Additional Comment 1
 * Perform a KAT by duplicating the public key generation.
 *
 * NOTE: This issue requires a background understanding, provided in a separate
 * document; the current IG 10.3.A AC1 is insufficient regarding the PCT for
 * the key agreement scenario.
 *
 * Currently IG 10.3.A requires PCT in the mode of use prior to use of the
 * key pair, citing the PCT defined in the associated standard. For key
 * agreement, the only PCT defined in SP 800-56A is that of Section 5.6.2.4:
 * the comparison of the original public key to a newly calculated public key.
 */
static int ecdsa_keygen_knownanswer_test(EC_KEY *eckey, BN_CTX *ctx,
                                         OSSL_CALLBACK *cb, void *cbarg)
{
    int len, ret = 0;
    OSSL_SELF_TEST *st = NULL;
    unsigned char bytes[512] = {0};
    EC_POINT *pub_key2 = NULL;

    st = OSSL_SELF_TEST_new(cb, cbarg);
    if (st == NULL)
        return 0;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT_KAT,
                               OSSL_SELF_TEST_DESC_PCT_ECDSA);

    if ((pub_key2 = EC_POINT_new(eckey->group)) == NULL)
        goto err;

    /* pub_key = priv_key * G (where G is a point on the curve) */
    if (!EC_POINT_mul(eckey->group, pub_key2, eckey->priv_key, NULL, NULL, ctx))
        goto err;

    if (BN_num_bytes(pub_key2->X) > (int)sizeof(bytes))
        goto err;
    len = BN_bn2bin(pub_key2->X, bytes);
    if (OSSL_SELF_TEST_oncorrupt_byte(st, bytes)
            && BN_bin2bn(bytes, len, pub_key2->X) == NULL)
        goto err;
    ret = !EC_POINT_cmp(eckey->group, eckey->pub_key, pub_key2, ctx);

err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    EC_POINT_free(pub_key2);
    return ret;
}

/*
 * ECC Key generation.
 * See SP800-56AR3 5.6.1.2.2 "Key Pair Generation by Testing Candidates"
 *
 * Params:
 *     libctx A context containing an optional self test callback.
 *     eckey An EC key object that contains domain params. The generated keypair
 *           is stored in this object.
 *     pairwise_test Set to non zero to perform a pairwise test. If the test
 *                   fails then the keypair is not generated,
 * Returns 1 if the keypair was generated or 0 otherwise.
 */
static int ec_generate_key(EC_KEY *eckey, int pairwise_test)
{
    int ok = 0;
    BIGNUM *priv_key = NULL;
    const BIGNUM *tmp = NULL;
    BIGNUM *order = NULL;
    EC_POINT *pub_key = NULL;
    const EC_GROUP *group = eckey->group;
    BN_CTX *ctx = BN_CTX_secure_new_ex(eckey->libctx);
    int sm2 = EC_KEY_get_flags(eckey) & EC_FLAG_SM2_RANGE ? 1 : 0;

    if (ctx == NULL)
        goto err;

    if (eckey->priv_key == NULL) {
        priv_key = BN_secure_new();
        if (priv_key == NULL)
            goto err;
    } else
        priv_key = eckey->priv_key;

    /*
     * Steps (1-2): Check domain parameters and security strength.
     * These steps must be done by the user. This would need to be
     * stated in the security policy.
     */

    tmp = EC_GROUP_get0_order(group);
    if (tmp == NULL)
        goto err;

    /*
     * Steps (3-7): priv_key = DRBG_RAND(order_n_bits) (range [1, n-1]).
     * Although this is slightly different from the standard, it is effectively
     * equivalent as it gives an unbiased result ranging from 1..n-1. It is also
     * faster as the standard needs to retry more often. Also doing
     * 1 + rand[0..n-2] would effect the way that tests feed dummy entropy into
     * rand so the simpler backward compatible method has been used here.
     */

    /* range of SM2 private key is [1, n-1) */
    if (sm2) {
        order = BN_new();
        if (order == NULL || !BN_sub(order, tmp, BN_value_one()))
            goto err;
    } else {
        order = BN_dup(tmp);
        if (order == NULL)
            goto err;
    }

    do
        if (!BN_priv_rand_range_ex(priv_key, order, 0, ctx))
            goto err;
    while (BN_is_zero(priv_key)) ;

    if (eckey->pub_key == NULL) {
        pub_key = EC_POINT_new(group);
        if (pub_key == NULL)
            goto err;
    } else
        pub_key = eckey->pub_key;

    /* Step (8) : pub_key = priv_key * G (where G is a point on the curve) */
    if (!EC_POINT_mul(group, pub_key, priv_key, NULL, NULL, ctx))
        goto err;

    eckey->priv_key = priv_key;
    eckey->pub_key = pub_key;
    priv_key = NULL;
    pub_key = NULL;

    eckey->dirty_cnt++;

#ifdef FIPS_MODULE
    pairwise_test = 1;
#endif /* FIPS_MODULE */

    ok = 1;
    if (pairwise_test) {
        OSSL_CALLBACK *cb = NULL;
        void *cbarg = NULL;

        OSSL_SELF_TEST_get_callback(eckey->libctx, &cb, &cbarg);
        ok = ecdsa_keygen_pairwise_test(eckey, cb, cbarg)
             && ecdsa_keygen_knownanswer_test(eckey, ctx, cb, cbarg);
    }
err:
    /* Step (9): If there is an error return an invalid keypair. */
    if (!ok) {
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
        BN_clear(eckey->priv_key);
        if (eckey->pub_key != NULL)
            EC_POINT_set_to_infinity(group, eckey->pub_key);
    }

    EC_POINT_free(pub_key);
    BN_clear_free(priv_key);
    BN_CTX_free(ctx);
    BN_free(order);
    return ok;
}

#ifndef FIPS_MODULE
/*
 * This is similar to ec_generate_key(), except it uses an ikm to
 * derive the private key.
 */
int ossl_ec_generate_key_dhkem(EC_KEY *eckey,
                               const unsigned char *ikm, size_t ikmlen)
{
    int ok = 0;

    if (eckey->priv_key == NULL) {
        eckey->priv_key = BN_secure_new();
        if (eckey->priv_key == NULL)
            goto err;
    }
    if (ossl_ec_dhkem_derive_private(eckey, eckey->priv_key, ikm, ikmlen) <= 0)
        goto err;
    if (eckey->pub_key == NULL) {
        eckey->pub_key = EC_POINT_new(eckey->group);
        if (eckey->pub_key == NULL)
            goto err;
    }
    if (!ossl_ec_key_simple_generate_public_key(eckey))
        goto err;

    ok = 1;
err:
    if (!ok) {
        BN_clear_free(eckey->priv_key);
        eckey->priv_key = NULL;
        if (eckey->pub_key != NULL)
            EC_POINT_set_to_infinity(eckey->group, eckey->pub_key);
    }
    return ok;
}
#endif

int ossl_ec_key_simple_generate_key(EC_KEY *eckey)
{
    return ec_generate_key(eckey, 0);
}

int ossl_ec_key_simple_generate_public_key(EC_KEY *eckey)
{
    int ret;
    BN_CTX *ctx = BN_CTX_new_ex(eckey->libctx);

    if (ctx == NULL)
        return 0;

    /*
     * See SP800-56AR3 5.6.1.2.2: Step (8)
     * pub_key = priv_key * G (where G is a point on the curve)
     */
    ret = EC_POINT_mul(eckey->group, eckey->pub_key, eckey->priv_key, NULL,
                       NULL, ctx);

    BN_CTX_free(ctx);
    if (ret == 1)
        eckey->dirty_cnt++;

    return ret;
}

int EC_KEY_check_key(const EC_KEY *eckey)
{
    if (eckey == NULL || eckey->group == NULL || eckey->pub_key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (eckey->group->meth->keycheck == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return eckey->group->meth->keycheck(eckey);
}

/*
 * Check the range of the EC public key.
 * See SP800-56A R3 Section 5.6.2.3.3 (Part 2)
 * i.e.
 *  - If q = odd prime p: Verify that xQ and yQ are integers in the
 *    interval[0, p - 1], OR
 *  - If q = 2m: Verify that xQ and yQ are bit strings of length m bits.
 * Returns 1 if the public key has a valid range, otherwise it returns 0.
 */
static int ec_key_public_range_check(BN_CTX *ctx, const EC_KEY *key)
{
    int ret = 0;
    BIGNUM *x, *y;

    BN_CTX_start(ctx);
    x = BN_CTX_get(ctx);
    y = BN_CTX_get(ctx);
    if (y == NULL)
        goto err;

    if (!EC_POINT_get_affine_coordinates(key->group, key->pub_key, x, y, ctx))
        goto err;

    if (EC_GROUP_get_field_type(key->group) == NID_X9_62_prime_field) {
        if (BN_is_negative(x)
            || BN_cmp(x, key->group->field) >= 0
            || BN_is_negative(y)
            || BN_cmp(y, key->group->field) >= 0) {
            goto err;
        }
    } else {
        int m = EC_GROUP_get_degree(key->group);
        if (BN_num_bits(x) > m || BN_num_bits(y) > m) {
            goto err;
        }
    }
    ret = 1;
err:
    BN_CTX_end(ctx);
    return ret;
}

/*
 * ECC Partial Public-Key Validation as specified in SP800-56A R3
 * Section 5.6.2.3.4 ECC Partial Public-Key Validation Routine.
 */
int ossl_ec_key_public_check_quick(const EC_KEY *eckey, BN_CTX *ctx)
{
    if (eckey == NULL || eckey->group == NULL || eckey->pub_key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    /* 5.6.2.3.3 (Step 1): Q != infinity */
    if (EC_POINT_is_at_infinity(eckey->group, eckey->pub_key)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_AT_INFINITY);
        return 0;
    }

    /* 5.6.2.3.3 (Step 2) Test if the public key is in range */
    if (!ec_key_public_range_check(ctx, eckey)) {
        ERR_raise(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        return 0;
    }

    /* 5.6.2.3.3 (Step 3) is the pub_key on the elliptic curve */
    if (EC_POINT_is_on_curve(eckey->group, eckey->pub_key, ctx) <= 0) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_IS_NOT_ON_CURVE);
        return 0;
    }
    return 1;
}

/*
 * ECC Key validation as specified in SP800-56A R3.
 * Section 5.6.2.3.3 ECC Full Public-Key Validation Routine.
 */
int ossl_ec_key_public_check(const EC_KEY *eckey, BN_CTX *ctx)
{
    int ret = 0;
    EC_POINT *point = NULL;
    const BIGNUM *order = NULL;
    const BIGNUM *cofactor = EC_GROUP_get0_cofactor(eckey->group);

    if (!ossl_ec_key_public_check_quick(eckey, ctx))
        return 0;

    if (cofactor != NULL && BN_is_one(cofactor)) {
        /* Skip the unnecessary expensive computation for curves with cofactor of 1. */
        return 1;
    }

    point = EC_POINT_new(eckey->group);
    if (point == NULL)
        return 0;

    order = eckey->group->order;
    if (BN_is_zero(order)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_GROUP_ORDER);
        goto err;
    }
    /* 5.6.2.3.3 (Step 4) : pub_key * order is the point at infinity. */
    if (!EC_POINT_mul(eckey->group, point, NULL, eckey->pub_key, order, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }
    if (!EC_POINT_is_at_infinity(eckey->group, point)) {
        ERR_raise(ERR_LIB_EC, EC_R_WRONG_ORDER);
        goto err;
    }
    ret = 1;
err:
    EC_POINT_free(point);
    return ret;
}

/*
 * ECC Key validation as specified in SP800-56A R3.
 * Section 5.6.2.1.2 Owner Assurance of Private-Key Validity
 * The private key is in the range [1, order-1]
 */
int ossl_ec_key_private_check(const EC_KEY *eckey)
{
    if (eckey == NULL || eckey->group == NULL || eckey->priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (BN_cmp(eckey->priv_key, BN_value_one()) < 0
        || BN_cmp(eckey->priv_key, eckey->group->order) >= 0) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_PRIVATE_KEY);
        return 0;
    }
    return 1;
}

/*
 * ECC Key validation as specified in SP800-56A R3.
 * Section 5.6.2.1.4 Owner Assurance of Pair-wise Consistency (b)
 * Check if generator * priv_key = pub_key
 */
int ossl_ec_key_pairwise_check(const EC_KEY *eckey, BN_CTX *ctx)
{
    int ret = 0;
    EC_POINT *point = NULL;

    if (eckey == NULL
       || eckey->group == NULL
       || eckey->pub_key == NULL
       || eckey->priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    point = EC_POINT_new(eckey->group);
    if (point == NULL)
        goto err;


    if (!EC_POINT_mul(eckey->group, point, eckey->priv_key, NULL, NULL, ctx)) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }
    if (EC_POINT_cmp(eckey->group, point, eckey->pub_key, ctx) != 0) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_PRIVATE_KEY);
        goto err;
    }
    ret = 1;
err:
    EC_POINT_free(point);
    return ret;
}


/*
 * ECC Key validation as specified in SP800-56A R3.
 *    Section 5.6.2.3.3 ECC Full Public-Key Validation
 *    Section 5.6.2.1.2 Owner Assurance of Private-Key Validity
 *    Section 5.6.2.1.4 Owner Assurance of Pair-wise Consistency
 * NOTES:
 *    Before calling this method in fips mode, there should be an assurance that
 *    an approved elliptic-curve group is used.
 * Returns 1 if the key is valid, otherwise it returns 0.
 */
int ossl_ec_key_simple_check_key(const EC_KEY *eckey)
{
    int ok = 0;
    BN_CTX *ctx = NULL;

    if (eckey == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if ((ctx = BN_CTX_new_ex(eckey->libctx)) == NULL)
        return 0;

    if (!ossl_ec_key_public_check(eckey, ctx))
        goto err;

    if (eckey->priv_key != NULL) {
        if (!ossl_ec_key_private_check(eckey)
            || !ossl_ec_key_pairwise_check(eckey, ctx))
            goto err;
    }
    ok = 1;
err:
    BN_CTX_free(ctx);
    return ok;
}

int EC_KEY_set_public_key_affine_coordinates(EC_KEY *key, BIGNUM *x,
                                             BIGNUM *y)
{
    BN_CTX *ctx = NULL;
    BIGNUM *tx, *ty;
    EC_POINT *point = NULL;
    int ok = 0;

    if (key == NULL || key->group == NULL || x == NULL || y == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ctx = BN_CTX_new_ex(key->libctx);
    if (ctx == NULL)
        return 0;

    BN_CTX_start(ctx);
    point = EC_POINT_new(key->group);

    if (point == NULL)
        goto err;

    tx = BN_CTX_get(ctx);
    ty = BN_CTX_get(ctx);
    if (ty == NULL)
        goto err;

    if (!EC_POINT_set_affine_coordinates(key->group, point, x, y, ctx))
        goto err;
    if (!EC_POINT_get_affine_coordinates(key->group, point, tx, ty, ctx))
        goto err;

    /*
     * Check if retrieved coordinates match originals. The range check is done
     * inside EC_KEY_check_key().
     */
    if (BN_cmp(x, tx) || BN_cmp(y, ty)) {
        ERR_raise(ERR_LIB_EC, EC_R_COORDINATES_OUT_OF_RANGE);
        goto err;
    }

    /* EC_KEY_set_public_key updates dirty_cnt */
    if (!EC_KEY_set_public_key(key, point))
        goto err;

    if (EC_KEY_check_key(key) == 0)
        goto err;

    ok = 1;

 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    EC_POINT_free(point);
    return ok;

}

OSSL_LIB_CTX *ossl_ec_key_get_libctx(const EC_KEY *key)
{
    return key->libctx;
}

const char *ossl_ec_key_get0_propq(const EC_KEY *key)
{
    return key->propq;
}

void ossl_ec_key_set0_libctx(EC_KEY *key, OSSL_LIB_CTX *libctx)
{
    key->libctx = libctx;
    /* Do we need to propagate this to the group? */
}

const EC_GROUP *EC_KEY_get0_group(const EC_KEY *key)
{
    return key->group;
}

int EC_KEY_set_group(EC_KEY *key, const EC_GROUP *group)
{
    if (key->meth->set_group != NULL && key->meth->set_group(key, group) == 0)
        return 0;
    EC_GROUP_free(key->group);
    key->group = EC_GROUP_dup(group);
    if (key->group != NULL && EC_GROUP_get_curve_name(key->group) == NID_sm2)
        EC_KEY_set_flags(key, EC_FLAG_SM2_RANGE);

    key->dirty_cnt++;
    return (key->group == NULL) ? 0 : 1;
}

const BIGNUM *EC_KEY_get0_private_key(const EC_KEY *key)
{
    return key->priv_key;
}

int EC_KEY_set_private_key(EC_KEY *key, const BIGNUM *priv_key)
{
    int fixed_top;
    const BIGNUM *order = NULL;
    BIGNUM *tmp_key = NULL;

    if (key->group == NULL || key->group->meth == NULL)
        return 0;

    /*
     * Not only should key->group be set, but it should also be in a valid
     * fully initialized state.
     *
     * Specifically, to operate in constant time, we need that the group order
     * is set, as we use its length as the fixed public size of any scalar used
     * as an EC private key.
     */
    order = EC_GROUP_get0_order(key->group);
    if (order == NULL || BN_is_zero(order))
        return 0; /* This should never happen */

    if (key->group->meth->set_private != NULL
        && key->group->meth->set_private(key, priv_key) == 0)
        return 0;
    if (key->meth->set_private != NULL
        && key->meth->set_private(key, priv_key) == 0)
        return 0;

    /*
     * Return `0` to comply with legacy behavior for this function, see
     * https://github.com/openssl/openssl/issues/18744#issuecomment-1195175696
     */
    if (priv_key == NULL) {
        BN_clear_free(key->priv_key);
        key->priv_key = NULL;
        return 0; /* intentional for legacy compatibility */
    }

    /*
     * We should never leak the bit length of the secret scalar in the key,
     * so we always set the `BN_FLG_CONSTTIME` flag on the internal `BIGNUM`
     * holding the secret scalar.
     *
     * This is important also because `BN_dup()` (and `BN_copy()`) do not
     * propagate the `BN_FLG_CONSTTIME` flag from the source `BIGNUM`, and
     * this brings an extra risk of inadvertently losing the flag, even when
     * the caller specifically set it.
     *
     * The propagation has been turned on and off a few times in the past
     * years because in some conditions has shown unintended consequences in
     * some code paths, so at the moment we can't fix this in the BN layer.
     *
     * In `EC_KEY_set_private_key()` we can work around the propagation by
     * manually setting the flag after `BN_dup()` as we know for sure that
     * inside the EC module the `BN_FLG_CONSTTIME` is always treated
     * correctly and should not generate unintended consequences.
     *
     * Setting the BN_FLG_CONSTTIME flag alone is never enough, we also have
     * to preallocate the BIGNUM internal buffer to a fixed public size big
     * enough that operations performed during the processing never trigger
     * a realloc which would leak the size of the scalar through memory
     * accesses.
     *
     * Fixed Length
     * ------------
     *
     * The order of the large prime subgroup of the curve is our choice for
     * a fixed public size, as that is generally the upper bound for
     * generating a private key in EC cryptosystems and should fit all valid
     * secret scalars.
     *
     * For preallocating the BIGNUM storage we look at the number of "words"
     * required for the internal representation of the order, and we
     * preallocate 2 extra "words" in case any of the subsequent processing
     * might temporarily overflow the order length.
     */
    tmp_key = BN_dup(priv_key);
    if (tmp_key == NULL)
        return 0;

    BN_set_flags(tmp_key, BN_FLG_CONSTTIME);

    fixed_top = bn_get_top(order) + 2;
    if (bn_wexpand(tmp_key, fixed_top) == NULL) {
        BN_clear_free(tmp_key);
        return 0;
    }

    BN_clear_free(key->priv_key);
    key->priv_key = tmp_key;
    key->dirty_cnt++;

    return 1;
}

const EC_POINT *EC_KEY_get0_public_key(const EC_KEY *key)
{
    return key->pub_key;
}

int EC_KEY_set_public_key(EC_KEY *key, const EC_POINT *pub_key)
{
    if (key->meth->set_public != NULL
        && key->meth->set_public(key, pub_key) == 0)
        return 0;
    EC_POINT_free(key->pub_key);
    key->pub_key = EC_POINT_dup(pub_key, key->group);
    key->dirty_cnt++;
    return (key->pub_key == NULL) ? 0 : 1;
}

unsigned int EC_KEY_get_enc_flags(const EC_KEY *key)
{
    return key->enc_flag;
}

void EC_KEY_set_enc_flags(EC_KEY *key, unsigned int flags)
{
    key->enc_flag = flags;
}

point_conversion_form_t EC_KEY_get_conv_form(const EC_KEY *key)
{
    return key->conv_form;
}

void EC_KEY_set_conv_form(EC_KEY *key, point_conversion_form_t cform)
{
    key->conv_form = cform;
    if (key->group != NULL)
        EC_GROUP_set_point_conversion_form(key->group, cform);
}

void EC_KEY_set_asn1_flag(EC_KEY *key, int flag)
{
    if (key->group != NULL)
        EC_GROUP_set_asn1_flag(key->group, flag);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_KEY_precompute_mult(EC_KEY *key, BN_CTX *ctx)
{
    if (key->group == NULL)
        return 0;
    return EC_GROUP_precompute_mult(key->group, ctx);
}
#endif

int EC_KEY_get_flags(const EC_KEY *key)
{
    return key->flags;
}

void EC_KEY_set_flags(EC_KEY *key, int flags)
{
    key->flags |= flags;
    key->dirty_cnt++;
}

void EC_KEY_clear_flags(EC_KEY *key, int flags)
{
    key->flags &= ~flags;
    key->dirty_cnt++;
}

int EC_KEY_decoded_from_explicit_params(const EC_KEY *key)
{
    if (key == NULL || key->group == NULL)
        return -1;
    return key->group->decoded_from_explicit_params;
}

size_t EC_KEY_key2buf(const EC_KEY *key, point_conversion_form_t form,
                        unsigned char **pbuf, BN_CTX *ctx)
{
    if (key == NULL || key->pub_key == NULL || key->group == NULL)
        return 0;
    return EC_POINT_point2buf(key->group, key->pub_key, form, pbuf, ctx);
}

int EC_KEY_oct2key(EC_KEY *key, const unsigned char *buf, size_t len,
                   BN_CTX *ctx)
{
    if (key == NULL || key->group == NULL)
        return 0;
    if (key->pub_key == NULL)
        key->pub_key = EC_POINT_new(key->group);
    if (key->pub_key == NULL)
        return 0;
    if (EC_POINT_oct2point(key->group, key->pub_key, buf, len, ctx) == 0)
        return 0;
    key->dirty_cnt++;
    /*
     * Save the point conversion form.
     * For non-custom curves the first octet of the buffer (excluding
     * the last significant bit) contains the point conversion form.
     * EC_POINT_oct2point() has already performed sanity checking of
     * the buffer so we know it is valid.
     */
    if ((key->group->meth->flags & EC_FLAGS_CUSTOM_CURVE) == 0)
        key->conv_form = (point_conversion_form_t)(buf[0] & ~0x01);
    return 1;
}

size_t EC_KEY_priv2oct(const EC_KEY *eckey,
                       unsigned char *buf, size_t len)
{
    if (eckey->group == NULL || eckey->group->meth == NULL)
        return 0;
    if (eckey->group->meth->priv2oct == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return eckey->group->meth->priv2oct(eckey, buf, len);
}

size_t ossl_ec_key_simple_priv2oct(const EC_KEY *eckey,
                                   unsigned char *buf, size_t len)
{
    size_t buf_len;

    buf_len = (EC_GROUP_order_bits(eckey->group) + 7) / 8;
    if (eckey->priv_key == NULL)
        return 0;
    if (buf == NULL)
        return buf_len;
    else if (len < buf_len)
        return 0;

    /* Octetstring may need leading zeros if BN is to short */

    if (BN_bn2binpad(eckey->priv_key, buf, buf_len) == -1) {
        ERR_raise(ERR_LIB_EC, EC_R_BUFFER_TOO_SMALL);
        return 0;
    }

    return buf_len;
}

int EC_KEY_oct2priv(EC_KEY *eckey, const unsigned char *buf, size_t len)
{
    int ret;

    if (eckey->group == NULL || eckey->group->meth == NULL)
        return 0;
    if (eckey->group->meth->oct2priv == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    ret = eckey->group->meth->oct2priv(eckey, buf, len);
    if (ret == 1)
        eckey->dirty_cnt++;
    return ret;
}

int ossl_ec_key_simple_oct2priv(EC_KEY *eckey, const unsigned char *buf,
                                size_t len)
{
    if (eckey->priv_key == NULL)
        eckey->priv_key = BN_secure_new();
    if (eckey->priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        return 0;
    }
    if (BN_bin2bn(buf, len, eckey->priv_key) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        return 0;
    }
    eckey->dirty_cnt++;
    return 1;
}

size_t EC_KEY_priv2buf(const EC_KEY *eckey, unsigned char **pbuf)
{
    size_t len;
    unsigned char *buf;

    len = EC_KEY_priv2oct(eckey, NULL, 0);
    if (len == 0)
        return 0;
    if ((buf = OPENSSL_malloc(len)) == NULL)
        return 0;
    len = EC_KEY_priv2oct(eckey, buf, len);
    if (len == 0) {
        OPENSSL_free(buf);
        return 0;
    }
    *pbuf = buf;
    return len;
}

int EC_KEY_can_sign(const EC_KEY *eckey)
{
    if (eckey->group == NULL || eckey->group->meth == NULL
        || (eckey->group->meth->flags & EC_FLAGS_NO_SIGN))
        return 0;
    return 1;
}

/*
 * FIPS 140-2 IG 9.9 AS09.33
 * Perform a sign/verify operation.
 *
 * NOTE: When generating keys for key-agreement schemes - FIPS 140-2 IG 9.9
 * states that no additional pairwise tests are required (apart from the tests
 * specified in SP800-56A) when generating keys. Hence pairwise ECDH tests are
 * omitted here.
 */
static int ecdsa_keygen_pairwise_test(EC_KEY *eckey, OSSL_CALLBACK *cb,
                                      void *cbarg)
{
    int ret = 0;
    unsigned char dgst[16] = {0};
    int dgst_len = (int)sizeof(dgst);
    ECDSA_SIG *sig = NULL;
    OSSL_SELF_TEST *st = NULL;

    st = OSSL_SELF_TEST_new(cb, cbarg);
    if (st == NULL)
        return 0;

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_ECDSA);

    sig = ECDSA_do_sign(dgst, dgst_len, eckey);
    if (sig == NULL)
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(st, dgst);

    if (ECDSA_do_verify(dgst, dgst_len, sig, eckey) != 1)
        goto err;

    ret = 1;
err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    ECDSA_SIG_free(sig);
    return ret;
}
