/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for internal use.
 */
#include "internal/deprecated.h"

#include "internal/nelem.h"
#include "testutil.h"
#include <openssl/ec.h>
#include "ec_local.h"
#include <crypto/bn.h>
#include <openssl/objects.h>

static size_t crv_len = 0;
static EC_builtin_curve *curves = NULL;

/* sanity checks field_inv function pointer in EC_METHOD */
static int group_field_tests(const EC_GROUP *group, BN_CTX *ctx)
{
    BIGNUM *a = NULL, *b = NULL, *c = NULL;
    int ret = 0;

    if (group->meth->field_inv == NULL || group->meth->field_mul == NULL)
        return 1;

    BN_CTX_start(ctx);
    a = BN_CTX_get(ctx);
    b = BN_CTX_get(ctx);
    if (!TEST_ptr(c = BN_CTX_get(ctx))
        /* 1/1 = 1 */
        || !TEST_true(group->meth->field_inv(group, b, BN_value_one(), ctx))
        || !TEST_true(BN_is_one(b))
        /* (1/a)*a = 1 */
        || !TEST_true(BN_rand(a, BN_num_bits(group->field) - 1,
                              BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY))
        || !TEST_true(group->meth->field_inv(group, b, a, ctx))
        || (group->meth->field_encode &&
            !TEST_true(group->meth->field_encode(group, a, a, ctx)))
        || (group->meth->field_encode &&
            !TEST_true(group->meth->field_encode(group, b, b, ctx)))
        || !TEST_true(group->meth->field_mul(group, c, a, b, ctx))
        || (group->meth->field_decode &&
            !TEST_true(group->meth->field_decode(group, c, c, ctx)))
        || !TEST_true(BN_is_one(c)))
        goto err;

    /* 1/0 = error */
    BN_zero(a);
    if (!TEST_false(group->meth->field_inv(group, b, a, ctx))
        || !TEST_true(ERR_GET_LIB(ERR_peek_last_error()) == ERR_LIB_EC)
        || !TEST_true(ERR_GET_REASON(ERR_peek_last_error()) ==
                      EC_R_CANNOT_INVERT)
        /* 1/p = error */
        || !TEST_false(group->meth->field_inv(group, b, group->field, ctx))
        || !TEST_true(ERR_GET_LIB(ERR_peek_last_error()) == ERR_LIB_EC)
        || !TEST_true(ERR_GET_REASON(ERR_peek_last_error()) ==
                      EC_R_CANNOT_INVERT))
        goto err;

    ERR_clear_error();
    ret = 1;
 err:
    BN_CTX_end(ctx);
    return ret;
}

/* wrapper for group_field_tests for explicit curve params and EC_METHOD */
static int field_tests(const EC_METHOD *meth, const unsigned char *params,
                       int len)
{
    BN_CTX *ctx = NULL;
    BIGNUM *p = NULL, *a = NULL, *b = NULL;
    EC_GROUP *group = NULL;
    int ret = 0;

    if (!TEST_ptr(ctx = BN_CTX_new()))
        return 0;

    BN_CTX_start(ctx);
    p = BN_CTX_get(ctx);
    a = BN_CTX_get(ctx);
    if (!TEST_ptr(b = BN_CTX_get(ctx))
        || !TEST_ptr(group = EC_GROUP_new(meth))
        || !TEST_true(BN_bin2bn(params, len, p))
        || !TEST_true(BN_bin2bn(params + len, len, a))
        || !TEST_true(BN_bin2bn(params + 2 * len, len, b))
        || !TEST_true(EC_GROUP_set_curve(group, p, a, b, ctx))
        || !group_field_tests(group, ctx))
        goto err;
    ret = 1;

 err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    if (group != NULL)
        EC_GROUP_free(group);
    return ret;
}

/* NIST prime curve P-256 */
static const unsigned char params_p256[] = {
    /* p */
    0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    /* a */
    0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFC,
    /* b */
    0x5A, 0xC6, 0x35, 0xD8, 0xAA, 0x3A, 0x93, 0xE7, 0xB3, 0xEB, 0xBD, 0x55,
    0x76, 0x98, 0x86, 0xBC, 0x65, 0x1D, 0x06, 0xB0, 0xCC, 0x53, 0xB0, 0xF6,
    0x3B, 0xCE, 0x3C, 0x3E, 0x27, 0xD2, 0x60, 0x4B
};

#ifndef OPENSSL_NO_EC2M
/* NIST binary curve B-283 */
static const unsigned char params_b283[] = {
    /* p */
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0xA1,
    /* a */
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    /* b */
    0x02, 0x7B, 0x68, 0x0A, 0xC8, 0xB8, 0x59, 0x6D, 0xA5, 0xA4, 0xAF, 0x8A,
    0x19, 0xA0, 0x30, 0x3F, 0xCA, 0x97, 0xFD, 0x76, 0x45, 0x30, 0x9F, 0xA2,
    0xA5, 0x81, 0x48, 0x5A, 0xF6, 0x26, 0x3E, 0x31, 0x3B, 0x79, 0xA2, 0xF5
};
#endif

/* test EC_GFp_simple_method directly */
static int field_tests_ecp_simple(void)
{
    TEST_info("Testing EC_GFp_simple_method()\n");
    return field_tests(EC_GFp_simple_method(), params_p256,
                       sizeof(params_p256) / 3);
}

/* test EC_GFp_mont_method directly */
static int field_tests_ecp_mont(void)
{
    TEST_info("Testing EC_GFp_mont_method()\n");
    return field_tests(EC_GFp_mont_method(), params_p256,
                       sizeof(params_p256) / 3);
}

#ifndef OPENSSL_NO_EC2M
/* Test that decoding of invalid GF2m field parameters fails. */
static int ec2m_field_sanity(void)
{
    int ret = 0;
    BN_CTX *ctx = BN_CTX_new();
    BIGNUM *p, *a, *b;
    EC_GROUP *group1 = NULL, *group2 = NULL, *group3 = NULL;

    TEST_info("Testing GF2m hardening\n");

    BN_CTX_start(ctx);
    p = BN_CTX_get(ctx);
    a = BN_CTX_get(ctx);
    if (!TEST_ptr(b = BN_CTX_get(ctx))
        || !TEST_true(BN_one(a))
        || !TEST_true(BN_one(b)))
        goto out;

    /* Even pentanomial value should be rejected */
    if (!TEST_true(BN_set_word(p, 0xf2)))
        goto out;
    if (!TEST_ptr_null(group1 = EC_GROUP_new_curve_GF2m(p, a, b, ctx)))
        TEST_error("Zero constant term accepted in GF2m polynomial");

    /* Odd hexanomial should also be rejected */
    if (!TEST_true(BN_set_word(p, 0xf3)))
        goto out;
    if (!TEST_ptr_null(group2 = EC_GROUP_new_curve_GF2m(p, a, b, ctx)))
        TEST_error("Hexanomial accepted as GF2m polynomial");

    /* Excessive polynomial degree should also be rejected */
    if (!TEST_true(BN_set_word(p, 0x71))
        || !TEST_true(BN_set_bit(p, OPENSSL_ECC_MAX_FIELD_BITS + 1)))
        goto out;
    if (!TEST_ptr_null(group3 = EC_GROUP_new_curve_GF2m(p, a, b, ctx)))
        TEST_error("GF2m polynomial degree > %d accepted",
                   OPENSSL_ECC_MAX_FIELD_BITS);

    ret = group1 == NULL && group2 == NULL && group3 == NULL;

 out:
    EC_GROUP_free(group1);
    EC_GROUP_free(group2);
    EC_GROUP_free(group3);
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);

    return ret;
}

/* test EC_GF2m_simple_method directly */
static int field_tests_ec2_simple(void)
{
    TEST_info("Testing EC_GF2m_simple_method()\n");
    return field_tests(EC_GF2m_simple_method(), params_b283,
                       sizeof(params_b283) / 3);
}
#endif

/* test default method for a named curve */
static int field_tests_default(int n)
{
    BN_CTX *ctx = NULL;
    EC_GROUP *group = NULL;
    int nid = curves[n].nid;
    int ret = 0;

    TEST_info("Testing curve %s\n", OBJ_nid2sn(nid));

    if (!TEST_ptr(group = EC_GROUP_new_by_curve_name(nid))
        || !TEST_ptr(ctx = BN_CTX_new())
        || !group_field_tests(group, ctx))
        goto err;

    ret = 1;
 err:
    if (group != NULL)
        EC_GROUP_free(group);
    if (ctx != NULL)
        BN_CTX_free(ctx);
    return ret;
}

#ifndef OPENSSL_NO_EC_NISTP_64_GCC_128
/*
 * Tests a point known to cause an incorrect underflow in an old version of
 * ecp_nist521.c
 */
static int underflow_test(void)
{
    BN_CTX *ctx = NULL;
    EC_GROUP *grp = NULL;
    EC_POINT *P = NULL, *Q = NULL, *R = NULL;
    BIGNUM *x1 = NULL, *y1 = NULL, *z1 = NULL, *x2 = NULL, *y2 = NULL;
    BIGNUM *k = NULL;
    int testresult = 0;
    const char *x1str =
        "1534f0077fffffe87e9adcfe000000000000000000003e05a21d2400002e031b1f4"
        "b80000c6fafa4f3c1288798d624a247b5e2ffffffffffffffefe099241900004";
    const char *p521m1 =
        "1ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe";

    ctx = BN_CTX_new();
    if (!TEST_ptr(ctx))
        return 0;

    BN_CTX_start(ctx);
    x1 = BN_CTX_get(ctx);
    y1 = BN_CTX_get(ctx);
    z1 = BN_CTX_get(ctx);
    x2 = BN_CTX_get(ctx);
    y2 = BN_CTX_get(ctx);
    k = BN_CTX_get(ctx);
    if (!TEST_ptr(k))
        goto err;

    grp = EC_GROUP_new_by_curve_name(NID_secp521r1);
    P = EC_POINT_new(grp);
    Q = EC_POINT_new(grp);
    R = EC_POINT_new(grp);
    if (!TEST_ptr(grp) || !TEST_ptr(P) || !TEST_ptr(Q) || !TEST_ptr(R))
        goto err;

    if (!TEST_int_gt(BN_hex2bn(&x1, x1str), 0)
            || !TEST_int_gt(BN_hex2bn(&y1, p521m1), 0)
            || !TEST_int_gt(BN_hex2bn(&z1, p521m1), 0)
            || !TEST_int_gt(BN_hex2bn(&k, "02"), 0)
            || !TEST_true(ossl_ec_GFp_simple_set_Jprojective_coordinates_GFp(grp, P, x1,
                                                                             y1, z1, ctx))
            || !TEST_true(EC_POINT_mul(grp, Q, NULL, P, k, ctx))
            || !TEST_true(EC_POINT_get_affine_coordinates(grp, Q, x1, y1, ctx))
            || !TEST_true(EC_POINT_dbl(grp, R, P, ctx))
            || !TEST_true(EC_POINT_get_affine_coordinates(grp, R, x2, y2, ctx)))
        goto err;

    if (!TEST_int_eq(BN_cmp(x1, x2), 0)
            || !TEST_int_eq(BN_cmp(y1, y2), 0))
        goto err;

    testresult = 1;

 err:
    BN_CTX_end(ctx);
    EC_POINT_free(P);
    EC_POINT_free(Q);
    EC_POINT_free(R);
    EC_GROUP_free(grp);
    BN_CTX_free(ctx);

    return testresult;
}
#endif

/*
 * Tests behavior of the EC_KEY_set_private_key
 */
static int set_private_key(void)
{
    EC_KEY *key = NULL, *aux_key = NULL;
    int testresult = 0;

    key = EC_KEY_new_by_curve_name(NID_secp224r1);
    aux_key = EC_KEY_new_by_curve_name(NID_secp224r1);
    if (!TEST_ptr(key)
        || !TEST_ptr(aux_key)
        || !TEST_int_eq(EC_KEY_generate_key(key), 1)
        || !TEST_int_eq(EC_KEY_generate_key(aux_key), 1))
        goto err;

    /* Test setting a valid private key */
    if (!TEST_int_eq(EC_KEY_set_private_key(key, aux_key->priv_key), 1))
        goto err;

    /* Test compliance with legacy behavior for NULL private keys */
    if (!TEST_int_eq(EC_KEY_set_private_key(key, NULL), 0)
        || !TEST_ptr_null(key->priv_key))
        goto err;

    testresult = 1;

 err:
    EC_KEY_free(key);
    EC_KEY_free(aux_key);
    return testresult;
}

/*
 * Tests behavior of the decoded_from_explicit_params flag and API
 */
static int decoded_flag_test(void)
{
    EC_GROUP *grp;
    EC_GROUP *grp_copy = NULL;
    ECPARAMETERS *ecparams = NULL;
    ECPKPARAMETERS *ecpkparams = NULL;
    EC_KEY *key = NULL;
    unsigned char *encodedparams = NULL;
    const unsigned char *encp;
    int encodedlen;
    int testresult = 0;

    /* Test EC_GROUP_new not setting the flag */
    grp = EC_GROUP_new(EC_GFp_simple_method());
    if (!TEST_ptr(grp)
        || !TEST_int_eq(grp->decoded_from_explicit_params, 0))
        goto err;
    EC_GROUP_free(grp);

    /* Test EC_GROUP_new_by_curve_name not setting the flag */
    grp = EC_GROUP_new_by_curve_name(NID_X9_62_prime256v1);
    if (!TEST_ptr(grp)
        || !TEST_int_eq(grp->decoded_from_explicit_params, 0))
        goto err;

    /* Test EC_GROUP_new_from_ecparameters not setting the flag */
    if (!TEST_ptr(ecparams = EC_GROUP_get_ecparameters(grp, NULL))
        || !TEST_ptr(grp_copy = EC_GROUP_new_from_ecparameters(ecparams))
        || !TEST_int_eq(grp_copy->decoded_from_explicit_params, 0))
        goto err;
    EC_GROUP_free(grp_copy);
    grp_copy = NULL;
    ECPARAMETERS_free(ecparams);
    ecparams = NULL;

    /* Test EC_GROUP_new_from_ecpkparameters not setting the flag */
    if (!TEST_int_eq(EC_GROUP_get_asn1_flag(grp), OPENSSL_EC_NAMED_CURVE)
        || !TEST_ptr(ecpkparams = EC_GROUP_get_ecpkparameters(grp, NULL))
        || !TEST_ptr(grp_copy = EC_GROUP_new_from_ecpkparameters(ecpkparams))
        || !TEST_int_eq(grp_copy->decoded_from_explicit_params, 0)
        || !TEST_ptr(key = EC_KEY_new())
    /* Test EC_KEY_decoded_from_explicit_params on key without a group */
        || !TEST_int_eq(EC_KEY_decoded_from_explicit_params(key), -1)
        || !TEST_int_eq(EC_KEY_set_group(key, grp_copy), 1)
    /* Test EC_KEY_decoded_from_explicit_params negative case */
        || !TEST_int_eq(EC_KEY_decoded_from_explicit_params(key), 0))
        goto err;
    EC_GROUP_free(grp_copy);
    grp_copy = NULL;
    ECPKPARAMETERS_free(ecpkparams);
    ecpkparams = NULL;

    /* Test d2i_ECPKParameters with named params not setting the flag */
    if (!TEST_int_gt(encodedlen = i2d_ECPKParameters(grp, &encodedparams), 0)
        || !TEST_ptr(encp = encodedparams)
        || !TEST_ptr(grp_copy = d2i_ECPKParameters(NULL, &encp, encodedlen))
        || !TEST_int_eq(grp_copy->decoded_from_explicit_params, 0))
        goto err;
    EC_GROUP_free(grp_copy);
    grp_copy = NULL;
    OPENSSL_free(encodedparams);
    encodedparams = NULL;

    /* Asn1 flag stays set to explicit with EC_GROUP_new_from_ecpkparameters */
    EC_GROUP_set_asn1_flag(grp, OPENSSL_EC_EXPLICIT_CURVE);
    if (!TEST_ptr(ecpkparams = EC_GROUP_get_ecpkparameters(grp, NULL))
        || !TEST_ptr(grp_copy = EC_GROUP_new_from_ecpkparameters(ecpkparams))
        || !TEST_int_eq(EC_GROUP_get_asn1_flag(grp_copy), OPENSSL_EC_EXPLICIT_CURVE)
        || !TEST_int_eq(grp_copy->decoded_from_explicit_params, 0))
        goto err;
    EC_GROUP_free(grp_copy);
    grp_copy = NULL;

    /* Test d2i_ECPKParameters with explicit params setting the flag */
    if (!TEST_int_gt(encodedlen = i2d_ECPKParameters(grp, &encodedparams), 0)
        || !TEST_ptr(encp = encodedparams)
        || !TEST_ptr(grp_copy = d2i_ECPKParameters(NULL, &encp, encodedlen))
        || !TEST_int_eq(EC_GROUP_get_asn1_flag(grp_copy), OPENSSL_EC_EXPLICIT_CURVE)
        || !TEST_int_eq(grp_copy->decoded_from_explicit_params, 1)
        || !TEST_int_eq(EC_KEY_set_group(key, grp_copy), 1)
    /* Test EC_KEY_decoded_from_explicit_params positive case */
        || !TEST_int_eq(EC_KEY_decoded_from_explicit_params(key), 1))
        goto err;

    testresult = 1;

 err:
    EC_KEY_free(key);
    EC_GROUP_free(grp);
    EC_GROUP_free(grp_copy);
    ECPARAMETERS_free(ecparams);
    ECPKPARAMETERS_free(ecpkparams);
    OPENSSL_free(encodedparams);

    return testresult;
}

static
int ecpkparams_i2d2i_test(int n)
{
    EC_GROUP *g1 = NULL, *g2 = NULL;
    FILE *fp = NULL;
    int nid = curves[n].nid;
    int testresult = 0;

    /* create group */
    if (!TEST_ptr(g1 = EC_GROUP_new_by_curve_name(nid)))
        goto end;

    /* encode params to file */
    if (!TEST_ptr(fp = fopen("params.der", "wb"))
            || !TEST_true(i2d_ECPKParameters_fp(fp, g1)))
        goto end;

    /* flush and close file */
    if (!TEST_int_eq(fclose(fp), 0)) {
        fp = NULL;
        goto end;
    }
    fp = NULL;

    /* decode params from file */
    if (!TEST_ptr(fp = fopen("params.der", "rb"))
            || !TEST_ptr(g2 = d2i_ECPKParameters_fp(fp, NULL)))
        goto end;

    testresult = 1; /* PASS */

end:
    if (fp != NULL)
        fclose(fp);

    EC_GROUP_free(g1);
    EC_GROUP_free(g2);

    return testresult;
}


static int check_bn_mont_ctx(BN_MONT_CTX *mont, BIGNUM *mod, BN_CTX *ctx)
{
    int ret = 0;
    BN_MONT_CTX *regenerated = BN_MONT_CTX_new();

    if (!TEST_ptr(regenerated))
        return ret;
    if (!TEST_ptr(mont))
        goto err;

    if (!TEST_true(BN_MONT_CTX_set(regenerated, mod, ctx)))
        goto err;

    if (!TEST_true(ossl_bn_mont_ctx_eq(regenerated, mont)))
        goto err;

    ret = 1;

 err:
    BN_MONT_CTX_free(regenerated);
    return ret;
}

static int montgomery_correctness_test(EC_GROUP *group)
{
    int ret = 0;
    BN_CTX *ctx = NULL;

    ctx = BN_CTX_new();
    if (!TEST_ptr(ctx))
        return ret;
    if (!TEST_true(check_bn_mont_ctx(group->mont_data, group->order, ctx))) {
        TEST_error("group order issue");
        goto err;
    }
    if (group->field_data1 != NULL) {
        if (!TEST_true(check_bn_mont_ctx(group->field_data1, group->field, ctx)))
            goto err;
    }
    ret = 1;
 err:
    BN_CTX_free(ctx);
    return ret;
}

static int named_group_creation_test(void)
{
    int ret = 0;
    EC_GROUP *group = NULL;

    if (!TEST_ptr(group = EC_GROUP_new_by_curve_name(NID_X9_62_prime256v1))
        || !TEST_true(montgomery_correctness_test(group)))
      goto err;

    ret = 1;

 err:
    EC_GROUP_free(group);
    return ret;
}

int setup_tests(void)
{
    crv_len = EC_get_builtin_curves(NULL, 0);
    if (!TEST_ptr(curves = OPENSSL_malloc(sizeof(*curves) * crv_len))
        || !TEST_true(EC_get_builtin_curves(curves, crv_len)))
        return 0;

    ADD_TEST(field_tests_ecp_simple);
    ADD_TEST(field_tests_ecp_mont);
#ifndef OPENSSL_NO_EC2M
    ADD_TEST(ec2m_field_sanity);
    ADD_TEST(field_tests_ec2_simple);
#endif
    ADD_ALL_TESTS(field_tests_default, crv_len);
#ifndef OPENSSL_NO_EC_NISTP_64_GCC_128
    ADD_TEST(underflow_test);
#endif
    ADD_TEST(set_private_key);
    ADD_TEST(decoded_flag_test);
    ADD_ALL_TESTS(ecpkparams_i2d2i_test, crv_len);
    ADD_TEST(named_group_creation_test);

    return 1;
}

void cleanup_tests(void)
{
    OPENSSL_free(curves);
}
