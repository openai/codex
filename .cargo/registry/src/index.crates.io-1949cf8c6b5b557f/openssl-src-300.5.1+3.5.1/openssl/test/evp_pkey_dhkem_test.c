/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include <openssl/proverr.h>
#include "internal/nelem.h"
#include "testutil.h"

#define TEST_KEM_ENCAP       0
#define TEST_KEM_DECAP       1
#define TEST_KEM_ENCAP_DECAP 2

#define TEST_TYPE_AUTH        0
#define TEST_TYPE_NOAUTH      1
#define TEST_TYPE_AUTH_NOAUTH 2

#define TEST_KEYTYPE_P256         0
#define TEST_KEYTYPE_X25519       1
#define TEST_KEYTYPES_P256_X25519 2

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *nullprov = NULL;
static OSSL_PROVIDER *libprov = NULL;
static OSSL_PARAM opparam[2];
static EVP_PKEY *rkey[TEST_KEYTYPES_P256_X25519] = { NULL, NULL };
static EVP_PKEY_CTX *rctx[TEST_KEYTYPES_P256_X25519] = { NULL, NULL };

#include "dhkem_test.inc"

/* Perform encapsulate KAT's */
static int test_dhkem_encapsulate(int tstid)
{
    int ret = 0;
    EVP_PKEY *rpub = NULL, *spriv = NULL;
    const TEST_ENCAPDATA *t = &ec_encapdata[tstid];

    TEST_note("Test %s %s Decapsulate", t->curve,
              t->spriv != NULL ? "Auth" : "");

    if (!TEST_ptr(rpub = new_raw_public_key(t->curve, t->rpub, t->rpublen)))
        goto err;

    if (t->spriv != NULL) {
        if (!TEST_ptr(spriv = new_raw_private_key(t->curve,
                                                  t->spriv, t->sprivlen,
                                                  t->spub, t->spublen)))
            goto err;
    }
    ret = do_encap(t, rpub, spriv);
err:
    EVP_PKEY_free(spriv);
    EVP_PKEY_free(rpub);
    return ret;
}

/* Perform decapsulate KAT's */
static int test_dhkem_decapsulate(int tstid)
{
    int ret = 0;
    EVP_PKEY *rpriv = NULL, *spub = NULL;
    const TEST_ENCAPDATA *t = &ec_encapdata[tstid];

    TEST_note("Test %s %s Decapsulate", t->curve, t->spub != NULL ? "Auth" : "");

    if (!TEST_ptr(rpriv = new_raw_private_key(t->curve, t->rpriv, t->rprivlen,
                                              t->rpub, t->rpublen)))
        goto err;
    if (t->spub != NULL) {
        if (!TEST_ptr(spub = new_raw_public_key(t->curve, t->spub, t->spublen)))
            goto err;
    }
    ret = do_decap(t, rpriv, spub);
err:
    EVP_PKEY_free(spub);
    EVP_PKEY_free(rpriv);
    return ret;
}

/* Test that there are settables and they have correct data types */
static int test_settables(int tstid)
{
    EVP_PKEY_CTX *ctx = rctx[tstid];
    const OSSL_PARAM *settableparams;
    const OSSL_PARAM *p;

    return TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 1)
           && TEST_ptr(settableparams = EVP_PKEY_CTX_settable_params(ctx))
           && TEST_ptr(p = OSSL_PARAM_locate_const(settableparams,
                                                   OSSL_KEM_PARAM_OPERATION))
           && TEST_uint_eq(p->data_type, OSSL_PARAM_UTF8_STRING)
           && TEST_ptr(p = OSSL_PARAM_locate_const(settableparams,
                                                   OSSL_KEM_PARAM_IKME))
          && TEST_uint_eq(p->data_type, OSSL_PARAM_OCTET_STRING);
}

/* Test initing multiple times passes */
static int test_init_multiple(int tstid)
{
    EVP_PKEY_CTX *ctx = rctx[tstid];

    return TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 1)
           && TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), 1);
}

/* Fail is various bad inputs are passed to the derivekey (keygen) operation */
static int test_ec_dhkem_derivekey_fail(void)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    OSSL_PARAM params[3];
    EVP_PKEY_CTX *genctx = NULL;
    const TEST_DERIVEKEY_DATA *t = &ec_derivekey_data[0];
    BIGNUM *priv = NULL;

    /* Check non nist curve fails */
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 "secp256k1", 0);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                  (char *)t->ikm, t->ikmlen);
    params[2] = OSSL_PARAM_construct_end();

    if (!TEST_ptr(genctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", NULL))
        || !TEST_int_eq(EVP_PKEY_keygen_init(genctx), 1)
        || !TEST_int_eq(EVP_PKEY_CTX_set_params(genctx, params), 1)
        || !TEST_int_eq(EVP_PKEY_generate(genctx, &pkey),0))
        goto err;

    /* Fail if curve is not one of P-256, P-384 or P-521 */
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 "P-224", 0);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                  (char *)t->ikm, t->ikmlen);
    params[2] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_keygen_init(genctx), 1)
        || !TEST_int_eq(EVP_PKEY_CTX_set_params(genctx, params), 1)
        || !TEST_int_eq(EVP_PKEY_generate(genctx, &pkey), 0))
        goto err;

    /* Fail if ikm len is too small*/
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 "P-256", 0);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                  (char *)t->ikm, t->ikmlen - 1);
    params[2] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_CTX_set_params(genctx, params), 1)
        || !TEST_int_eq(EVP_PKEY_generate(genctx, &pkey), 0))
        goto err;

    ret = 1;
err:
    BN_free(priv);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(genctx);
    return ret;
}

/* Succeed even if the operation parameter is not set */
static int test_no_operation_set(int tstid)
{
    EVP_PKEY_CTX *ctx = rctx[tstid];
    const TEST_ENCAPDATA *t = &ec_encapdata[tstid];
    size_t len = 0;

    return TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 1)
           && TEST_int_eq(EVP_PKEY_encapsulate(ctx, NULL, &len, NULL, NULL), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate(ctx, NULL, &len,
                                               t->expected_enc,
                                               t->expected_enclen), 1);
}

/* Fail if the ikm is too small */
static int test_ikm_small(int tstid)
{
    unsigned char tmp[16] = { 0 };
    unsigned char secret[256];
    unsigned char enc[256];
    size_t secretlen = sizeof(secret);
    size_t enclen = sizeof(enc);
    OSSL_PARAM params[3];
    EVP_PKEY_CTX *ctx = rctx[tstid];

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                                 OSSL_KEM_PARAM_OPERATION_DHKEM,
                                                 0);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_KEM_PARAM_IKME,
                                                  tmp, sizeof(tmp));
    params[2] = OSSL_PARAM_construct_end();

    return TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, params), 1)
           && TEST_int_eq(EVP_PKEY_encapsulate(ctx, enc, &enclen,
                                               secret, &secretlen), 0);
}

/* Fail if buffers lengths are too small to hold returned data */
static int test_input_size_small(int tstid)
{
    int ret = 0;
    unsigned char sec[256];
    unsigned char enc[256];
    size_t seclen = sizeof(sec);
    size_t enclen = sizeof(enc);
    EVP_PKEY_CTX *ctx = rctx[tstid];

    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, opparam), 1)
        || !TEST_int_eq(EVP_PKEY_encapsulate(ctx, NULL, &enclen,
                                             NULL, &seclen), 1))
    goto err;

    /* buffer too small for enc */
    enclen--;
    if (!TEST_int_eq(EVP_PKEY_encapsulate(ctx, enc, &enclen, sec, &seclen),
                     0))
        goto err;
    enclen++;
    /* buffer too small for secret */
    seclen--;
    if (!TEST_int_eq(EVP_PKEY_encapsulate(ctx, enc, &enclen, sec, &seclen), 0))
        goto err;
    seclen++;
    if (!TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, opparam), 1))
        goto err;
     /* buffer too small for decapsulate secret */
    seclen--;
    if (!TEST_int_eq(EVP_PKEY_decapsulate(ctx, sec, &seclen, enc, enclen), 0))
        goto err;
    seclen++;
     /* incorrect enclen passed to decap  */
    enclen--;
    ret = TEST_int_eq(EVP_PKEY_decapsulate(ctx, sec, &seclen, enc, enclen), 0);
err:
    return ret;
}

/* Fail if the auth key has a different curve */
static int test_ec_auth_key_curve_mismatch(void)
{
    int ret = 0;
    EVP_PKEY *auth = NULL;

    if (!TEST_ptr(auth = EVP_PKEY_Q_keygen(libctx, NULL, "EC", "P-521")))
        return 0;

    ret = TEST_int_eq(EVP_PKEY_auth_encapsulate_init(rctx[0], auth, opparam), 0);
    EVP_PKEY_free(auth);
    return ret;
}

/* Fail if the auth key has a different key type to the recipient */
static int test_auth_key_type_mismatch(int tstid)
{
    int id1 = tstid;
    int id2 = !tstid;

    return TEST_int_eq(EVP_PKEY_auth_encapsulate_init(rctx[id1],
                                                      rkey[id2], opparam), 0);
}

static int test_ec_invalid_private_key(void)
{
    int ret = 0;
    EVP_PKEY *priv = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    const TEST_ENCAPDATA *t = &ec_encapdata[0];
    static const unsigned char order[] = {
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xBC, 0xE6, 0xFA, 0xAD, 0xA7, 0x17, 0x9E, 0x84,
        0xF3, 0xB9, 0xCA, 0xC2, 0xFC, 0x63, 0x25, 0x51
    };

    ret = TEST_ptr(priv = new_raw_private_key("P-256", order, sizeof(order),
                                              t->rpub, t->rpublen))
          && TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, priv, NULL))
          && TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 0);
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int test_ec_public_key_infinity(void)
{
    int ret = 0;
    EVP_PKEY *key = NULL;
    EVP_PKEY_CTX *keyctx = NULL;
    unsigned char s[256];
    unsigned char e[256];
    size_t slen = sizeof(s);
    size_t elen = sizeof(e);
    unsigned char tmp[1] = { 0 }; /* The encoding for an EC point at infinity */
    EVP_PKEY_CTX *ctx = rctx[0];
    const TEST_ENCAPDATA *t = &ec_encapdata[0];

    ret = TEST_ptr(key = new_raw_private_key(t->curve, t->rpriv, t->rprivlen,
                                                tmp, sizeof(tmp)))
          && TEST_ptr(keyctx = EVP_PKEY_CTX_new_from_pkey(libctx, key, NULL))
          /* Fail if the recipient public key is invalid */
          && TEST_int_eq(EVP_PKEY_encapsulate_init(keyctx, opparam), 1)
          && TEST_int_eq(EVP_PKEY_encapsulate(keyctx, e, &elen, s, &slen), 0)
          /* Fail the decap if the recipient public key is invalid */
          && TEST_int_eq(EVP_PKEY_decapsulate_init(keyctx, opparam), 1)
          && TEST_int_eq(EVP_PKEY_decapsulate(keyctx, s, &slen,
                                              t->expected_enc,
                                              t->expected_enclen), 0)
          /* Fail if the auth key has a bad public key */
          && TEST_int_eq(EVP_PKEY_auth_encapsulate_init(ctx, key, opparam), 1)
          && TEST_int_eq(EVP_PKEY_encapsulate(ctx, e, &elen, s, &slen), 0);

    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(keyctx);
    return ret;
}

/* Test incorrectly passing NULL values fail */
static int test_null_params(int tstid)
{
    EVP_PKEY_CTX *ctx = rctx[tstid];
    const TEST_ENCAPDATA *t = &ec_encapdata[tstid];

    /* auth_encap/decap init must be passed a non NULL value */
    return TEST_int_eq(EVP_PKEY_auth_encapsulate_init(ctx, NULL, opparam), 0)
           && TEST_int_eq(EVP_PKEY_auth_decapsulate_init(ctx, NULL, opparam), 0)
           /* Check decap fails if NULL params are passed */
           && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, opparam), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate(ctx, NULL, NULL,
                                               t->expected_enc,
                                               t->expected_enclen), 0)
           /* Check encap fails if NULL params are passed */
           && TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, opparam), 1)
           && TEST_int_eq(EVP_PKEY_encapsulate(ctx, NULL, NULL,
                                               NULL, NULL), 0);
}

static int test_set_params(int tstid)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = rctx[tstid];
    OSSL_PARAM badparams[4];
    int val = 1;

    /* wrong data type for operation param */
    badparams[0] = OSSL_PARAM_construct_int(OSSL_KEM_PARAM_OPERATION, &val);
    badparams[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 0))
        goto err;
    /* unknown string used for the operation param */
    badparams[0] = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                                    "unknown_op", 0);
    badparams[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 0))
        goto err;

    /* NULL string set for the operation param */
    badparams[0] = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                                    NULL, 0);
    badparams[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 0))
        goto err;

    /* wrong data type for ikme param */
    badparams[0] = OSSL_PARAM_construct_int(OSSL_KEM_PARAM_IKME, &val);
    badparams[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 0))
        goto err;

    /* Setting the ikme to NULL is allowed */
    badparams[0] = OSSL_PARAM_construct_octet_string(OSSL_KEM_PARAM_IKME, NULL, 0);
    badparams[1] = OSSL_PARAM_construct_end();
    if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 1))
        goto err;

    /* Test that unknown params are ignored */
    badparams[0] = OSSL_PARAM_construct_int("unknownparam", &val);
    badparams[1] = OSSL_PARAM_construct_end();
    ret = TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, badparams), 1);
err:
    return ret;
}

/*
 * ECX keys autogen the public key if a private key is loaded,
 * So this test passes for ECX, but fails for EC
 */
static int test_nopublic(int tstid)
{
    int ret = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *priv = NULL;
    int encap = ((tstid & 1) == 0);
    int keytype = tstid >= TEST_KEM_ENCAP_DECAP;
    const TEST_ENCAPDATA *t = &ec_encapdata[keytype];
    int expected = (keytype == TEST_KEYTYPE_X25519);

    TEST_note("%s %s", t->curve, encap ? "Encap" : "Decap");
    if (!TEST_ptr(priv = new_raw_private_key(t->curve, t->rpriv, t->rprivlen,
                                             NULL, 0)))
        goto err;
    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, priv, NULL)))
        goto err;

    if (encap) {
        if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, opparam), expected))
            goto err;
    } else {
        if (!TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, opparam), expected))
        goto err;
    }
    if (expected == 0
        && !TEST_int_eq(ERR_GET_REASON(ERR_get_error()), PROV_R_NOT_A_PUBLIC_KEY))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(priv);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

/* Test that not setting the auth public key fails the auth encap/decap init */
static int test_noauthpublic(int tstid)
{
    int ret = 0;
    EVP_PKEY *auth = NULL;
    int encap = ((tstid & 1) == 0);
    int keytype = tstid >= TEST_KEM_ENCAP_DECAP;
    const TEST_ENCAPDATA *t = &ec_encapdata[keytype];
    EVP_PKEY_CTX *ctx = rctx[keytype];
    int expected = (keytype == TEST_KEYTYPE_X25519);

    TEST_note("%s %s", t->curve, encap ? "Encap" : "Decap");
    if (!TEST_ptr(auth = new_raw_private_key(t->curve, t->rpriv,
                                             t->rprivlen, NULL, expected)))
        goto err;

    if (encap) {
        if (!TEST_int_eq(EVP_PKEY_auth_encapsulate_init(ctx, auth,
                                                        opparam), expected))
            goto err;
    } else {
        if (!TEST_int_eq(EVP_PKEY_auth_decapsulate_init(ctx, auth,
                                                        opparam), expected))
            goto err;
    }
    if (expected == 0
        && !TEST_int_eq(ERR_GET_REASON(ERR_get_error()),
                        PROV_R_NOT_A_PUBLIC_KEY))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(auth);
    return ret;
}

/* EC specific tests */

/* Perform EC DHKEM KATs */
static int test_ec_dhkem_derivekey(int tstid)
{
    int ret = 0;
    EVP_PKEY *pkey = NULL;
    OSSL_PARAM params[3];
    EVP_PKEY_CTX *genctx = NULL;
    const TEST_DERIVEKEY_DATA *t = &ec_derivekey_data[tstid];
    unsigned char pubkey[133];
    unsigned char privkey[66];
    size_t pubkeylen = 0, privkeylen = 0;
    BIGNUM *priv = NULL;

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 (char *)t->curvename, 0);
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                  (char *)t->ikm, t->ikmlen);
    params[2] = OSSL_PARAM_construct_end();

    ret = TEST_ptr(genctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", NULL))
          && TEST_int_eq(EVP_PKEY_keygen_init(genctx), 1)
          && TEST_int_eq(EVP_PKEY_CTX_set_params(genctx, params), 1)
          && TEST_int_eq(EVP_PKEY_generate(genctx, &pkey), 1)
          && TEST_true(EVP_PKEY_get_octet_string_param(pkey,
                           OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY,
                           pubkey, sizeof(pubkey), &pubkeylen))
          && TEST_true(EVP_PKEY_get_bn_param(pkey, OSSL_PKEY_PARAM_PRIV_KEY,
                                             &priv))
          && TEST_int_gt(privkeylen = BN_bn2bin(priv, privkey), 0)
          && TEST_int_le(privkeylen, sizeof(privkey))
          && TEST_mem_eq(privkey, privkeylen, t->priv, t->privlen)
          && TEST_mem_eq(pubkey, pubkeylen, t->pub, t->publen);

    BN_free(priv);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(genctx);
    return ret;
}

/*
 * Test that encapsulation uses a random seed if the ikm is not specified,
 * and verify that the shared secret matches the decapsulate result.
 */
static int test_ec_noikme(int tstid)
{
    int ret = 0, auth = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *recip = NULL;
    EVP_PKEY *sender_auth = NULL;
    unsigned char sender_secret[256];
    unsigned char recip_secret[256];
    unsigned char sender_pub[256];
    size_t sender_secretlen = sizeof(sender_secret);
    size_t recip_secretlen = sizeof(recip_secret);
    size_t sender_publen = sizeof(sender_pub);
    const char *curve;
    int sz = OSSL_NELEM(dhkem_supported_curves);
    const char *op = OSSL_KEM_PARAM_OPERATION_DHKEM;

    if (tstid >= sz) {
        auth = 1;
        tstid -= sz;
    }
    curve = dhkem_supported_curves[tstid];
    TEST_note("testing encap/decap of curve %s%s\n", curve,
              auth ? " with auth" : "");

    if (curve[0] == 'X') {
        if (!TEST_ptr(recip = EVP_PKEY_Q_keygen(libctx, NULL, curve))
                || (auth
                    && !TEST_ptr(sender_auth = EVP_PKEY_Q_keygen(libctx, NULL,
                                                                 curve))))
            goto err;
    } else {
        if (!TEST_ptr(recip = EVP_PKEY_Q_keygen(libctx, NULL, "EC", curve))
                || (auth
                    && !TEST_ptr(sender_auth = EVP_PKEY_Q_keygen(libctx, NULL,
                                                                 "EC", curve))))
            goto err;
    }

    ret = TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, recip, NULL))
          && (sender_auth == NULL
              || TEST_int_eq(EVP_PKEY_auth_encapsulate_init(ctx, sender_auth,
                                                            NULL), 1))
          && (sender_auth != NULL
              || TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), 1))
          && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(ctx, op), 1)
          && TEST_int_eq(EVP_PKEY_encapsulate(ctx, sender_pub, &sender_publen,
                                              sender_secret, &sender_secretlen), 1)
          && (sender_auth == NULL
              || TEST_int_eq(EVP_PKEY_auth_decapsulate_init(ctx, sender_auth,
                                                            NULL), 1))
          && (sender_auth != NULL
              || TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), 1))
          && TEST_int_eq(EVP_PKEY_CTX_set_kem_op(ctx, op), 1)
          && TEST_int_eq(EVP_PKEY_decapsulate(ctx, recip_secret, &recip_secretlen,
                                             sender_pub, sender_publen), 1)
          && TEST_mem_eq(recip_secret, recip_secretlen,
                         sender_secret, sender_secretlen);
err:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(sender_auth);
    EVP_PKEY_free(recip);
    return ret;
}

/* Test encap/decap init fail if the curve is invalid */
static int do_ec_curve_failtest(const char *curve)
{
    int ret;
    EVP_PKEY *key = NULL;
    EVP_PKEY_CTX *ctx = NULL;

    ret = TEST_ptr(key = EVP_PKEY_Q_keygen(libctx, NULL, "EC", curve))
          && TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, key, NULL))
          && TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), -2)
          && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), -2);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int test_ec_curve_nonnist(void)
{
    return do_ec_curve_failtest("secp256k1");
}

static int test_ec_curve_unsupported(void)
{
    return do_ec_curve_failtest("P-224");
}

/* Test that passing a bad recipient public EC key fails during encap/decap */
static int test_ec_badpublic(int tstid)
{
    int ret = 0;
    EVP_PKEY *recippriv = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    unsigned char secret[256];
    unsigned char pub[256];
    size_t secretlen = sizeof(secret);
    int encap = ((tstid & 1) == 0);
    const TEST_ENCAPDATA *t = &ec_encapdata[0];

    TEST_note("%s %s", t->curve, encap ? "Encap" : "Decap");
    /* Set the recipient public key to the point at infinity */
    pub[0] = 0;
    if (!TEST_ptr(recippriv = new_raw_private_key(t->curve, t->rpriv, t->rprivlen,
                                                  pub, 1)))
        goto err;

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, recippriv, NULL)))
        goto err;

    if (encap) {
        unsigned char enc[256];
        size_t enclen = sizeof(enc);

        if (!TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, opparam), 1))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_encapsulate(ctx, enc , &enclen,
                                              secret, &secretlen), 0 ))
            goto err;
    } else {
        if (!TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, opparam), 1))
            goto err;
        if (!TEST_int_eq(EVP_PKEY_decapsulate(ctx, secret, &secretlen,
                                              t->expected_enc,
                                              t->expected_enclen),
                         0))
            goto err;
    }
    if (!TEST_int_eq(ERR_GET_REASON(ERR_get_error()), PROV_R_INVALID_KEY))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(recippriv);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int test_ec_badauth(int tstid)
{
    int ret = 0;
    EVP_PKEY *auth = NULL;
    unsigned char enc[256];
    unsigned char secret[256];
    unsigned char pub[256];
    size_t enclen = sizeof(enc);
    size_t secretlen = sizeof(secret);
    int encap = ((tstid & 1) == 0);
    const TEST_ENCAPDATA *t = &ec_encapdata[TEST_KEYTYPE_P256];
    EVP_PKEY_CTX *ctx = rctx[TEST_KEYTYPE_P256];

    TEST_note("%s %s", t->curve, encap ? "Encap" : "Decap");
    /* Set the auth public key to the point at infinity */
    pub[0] = 0;
    if (!TEST_ptr(auth = new_raw_private_key(t->curve, t->rpriv, t->rprivlen,
                                             pub, 1)))
        goto err;
    if (encap) {
        if (!TEST_int_eq(EVP_PKEY_auth_encapsulate_init(ctx, auth,
                                                        opparam), 1)
            || !TEST_int_eq(EVP_PKEY_encapsulate(ctx, enc, &enclen,
                                                 secret, &secretlen), 0))
            goto err;
    } else {
        if (!TEST_int_eq(EVP_PKEY_auth_decapsulate_init(ctx, auth, opparam), 1)
            || !TEST_int_eq(EVP_PKEY_decapsulate(ctx, secret, &secretlen,
                                                 t->expected_enc,
                                                 t->expected_enclen), 0))
            goto err;
    }
    if (!TEST_int_eq(ERR_GET_REASON(ERR_get_error()), PROV_R_INVALID_KEY))
        goto err;
    ret = 1;
err:
    EVP_PKEY_free(auth);
    return ret;
}

static int test_ec_invalid_decap_enc_buffer(void)
{
    const TEST_ENCAPDATA *t = &ec_encapdata[TEST_KEYTYPE_P256];
    unsigned char enc[256];
    unsigned char secret[256];
    size_t secretlen = sizeof(secret);
    EVP_PKEY_CTX *ctx = rctx[0];

    memcpy(enc, t->expected_enc, t->expected_enclen);
    enc[0] = 0xFF;

    return TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, opparam), 1)
           && TEST_int_eq(EVP_PKEY_decapsulate(ctx, secret, &secretlen,
                                               enc, t->expected_enclen), 0);
}

#ifndef OPENSSL_NO_ECX
/* ECX specific tests */

/* Perform ECX DHKEM KATs */
static int test_ecx_dhkem_derivekey(int tstid)
{
    int ret;
    OSSL_PARAM params[2];
    EVP_PKEY_CTX *genctx;
    EVP_PKEY *pkey = NULL;
    unsigned char pubkey[64];
    unsigned char privkey[64];
    unsigned char masked_priv[64];
    size_t pubkeylen = 0, privkeylen = 0;
    const TEST_DERIVEKEY_DATA *t = &ecx_derivekey_data[tstid];

    memcpy(masked_priv, t->priv, t->privlen);
    if (OPENSSL_strcasecmp(t->curvename, "X25519") == 0) {
        /*
         * The RFC test vector seems incorrect since it is not in serialized form,
         * So manually do the conversion here for now.
         */
        masked_priv[0] &= 248;
        masked_priv[t->privlen - 1] &= 127;
        masked_priv[t->privlen - 1] |= 64;
    } else {
        masked_priv[0] &= 252;
        masked_priv[t->privlen - 1] |= 128;
    }

    params[0] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                  (char *)t->ikm, t->ikmlen);
    params[1] = OSSL_PARAM_construct_end();

    ret = TEST_ptr(genctx = EVP_PKEY_CTX_new_from_name(libctx, t->curvename, NULL))
          && TEST_int_eq(EVP_PKEY_keygen_init(genctx), 1)
          && TEST_int_eq(EVP_PKEY_CTX_set_params(genctx, params), 1)
          && TEST_int_eq(EVP_PKEY_keygen(genctx, &pkey), 1)
          && TEST_int_eq(EVP_PKEY_get_octet_string_param(pkey,
                             OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY,
                             pubkey, sizeof(pubkey), &pubkeylen), 1)
          && TEST_int_eq(EVP_PKEY_get_octet_string_param(pkey,
                             OSSL_PKEY_PARAM_PRIV_KEY,
                             privkey, sizeof(privkey), &privkeylen), 1)
          && TEST_mem_eq(t->pub, t->publen, pubkey, pubkeylen)
          && TEST_mem_eq(masked_priv, t->privlen, privkey, privkeylen);

    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(genctx);
    return ret;
}

/* Fail if the auth key has a different curve */
static int test_ecx_auth_key_curve_mismatch(void)
{
    int ret = 0;
    EVP_PKEY *auth = NULL;

    if (!TEST_ptr(auth = EVP_PKEY_Q_keygen(libctx, NULL, "X448")))
        return 0;

    ret = TEST_int_eq(EVP_PKEY_auth_encapsulate_init(rctx[TEST_KEYTYPE_X25519],
                                                     auth, opparam), 0);
    EVP_PKEY_free(auth);
    return ret;
}

/* Fail if ED448 is used for DHKEM */
static int test_ed_curve_unsupported(void)
{
    int ret;
    EVP_PKEY *key = NULL;
    EVP_PKEY_CTX *ctx = NULL;

    ret = TEST_ptr(key = EVP_PKEY_Q_keygen(libctx, NULL, "ED448"))
          && TEST_ptr(ctx = EVP_PKEY_CTX_new_from_pkey(libctx, key, NULL))
          && TEST_int_eq(EVP_PKEY_encapsulate_init(ctx, NULL), -2)
          && TEST_int_eq(EVP_PKEY_decapsulate_init(ctx, NULL), -2);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(ctx);
    return ret;
}
#endif

int setup_tests(void)
{
    const char *prov_name = "default";
    char *config_file = NULL;
    char *op = OSSL_KEM_PARAM_OPERATION_DHKEM;

    if (!test_get_libctx(&libctx, &nullprov, config_file, &libprov, prov_name))
        return 0;
    opparam[0] = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                                  op, 0);
    opparam[1] = OSSL_PARAM_construct_end();

    /* Create P256 and X25519 keys and ctxs */
    if (!TEST_ptr(rkey[TEST_KEYTYPE_P256] = EVP_PKEY_Q_keygen(libctx, NULL,
                                                              "EC", "P-256")))
        goto err;
#ifndef OPENSSL_NO_ECX
    if (!TEST_ptr(rkey[TEST_KEYTYPE_X25519] = EVP_PKEY_Q_keygen(libctx, NULL,
                                                                "X25519")))
        goto err;
#endif
    if (!TEST_ptr(rctx[TEST_KEYTYPE_P256] =
                      EVP_PKEY_CTX_new_from_pkey(libctx,
                                                 rkey[TEST_KEYTYPE_P256], NULL)))
       goto err;
#ifndef OPENSSL_NO_ECX
    if (!TEST_ptr(rctx[TEST_KEYTYPE_X25519] =
                      EVP_PKEY_CTX_new_from_pkey(libctx,
                                                 rkey[TEST_KEYTYPE_X25519], NULL)))
       goto err;
#endif

    ADD_ALL_TESTS(test_dhkem_encapsulate, OSSL_NELEM(ec_encapdata));
    ADD_ALL_TESTS(test_dhkem_decapsulate, OSSL_NELEM(ec_encapdata));
#ifndef OPENSSL_NO_ECX
    ADD_ALL_TESTS(test_settables, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_init_multiple, TEST_KEYTYPES_P256_X25519);

    ADD_ALL_TESTS(test_auth_key_type_mismatch, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_no_operation_set, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_ikm_small, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_input_size_small, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_null_params, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_set_params, TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_nopublic,
                  TEST_KEM_ENCAP_DECAP * TEST_KEYTYPES_P256_X25519);
    ADD_ALL_TESTS(test_noauthpublic,
                  TEST_KEM_ENCAP_DECAP * TEST_KEYTYPES_P256_X25519);
#else
    ADD_ALL_TESTS(test_settables, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_init_multiple, TEST_KEYTYPE_P256);

    ADD_ALL_TESTS(test_auth_key_type_mismatch, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_no_operation_set, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_ikm_small, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_input_size_small, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_null_params, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_set_params, TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_nopublic,
                  TEST_KEM_ENCAP_DECAP * TEST_KEYTYPE_P256);
    ADD_ALL_TESTS(test_noauthpublic,
                  TEST_KEM_ENCAP_DECAP * TEST_KEYTYPE_P256);
#endif
    /* EC Specific tests */
    ADD_ALL_TESTS(test_ec_dhkem_derivekey, OSSL_NELEM(ec_derivekey_data));
    ADD_ALL_TESTS(test_ec_noikme,
                  TEST_TYPE_AUTH_NOAUTH * OSSL_NELEM(dhkem_supported_curves));
    ADD_TEST(test_ec_auth_key_curve_mismatch);
    ADD_TEST(test_ec_invalid_private_key);
    ADD_TEST(test_ec_dhkem_derivekey_fail);
    ADD_TEST(test_ec_curve_nonnist);
    ADD_TEST(test_ec_curve_unsupported);
    ADD_TEST(test_ec_invalid_decap_enc_buffer);
    ADD_TEST(test_ec_public_key_infinity);
    ADD_ALL_TESTS(test_ec_badpublic, TEST_KEM_ENCAP_DECAP);
    ADD_ALL_TESTS(test_ec_badauth, TEST_KEM_ENCAP_DECAP);

    /* ECX specific tests */
#ifndef OPENSSL_NO_ECX
    ADD_ALL_TESTS(test_ecx_dhkem_derivekey, OSSL_NELEM(ecx_derivekey_data));
    ADD_TEST(test_ecx_auth_key_curve_mismatch);
    ADD_TEST(test_ed_curve_unsupported);
#endif
    return 1;
err:
    return 0;
}

void cleanup_tests(void)
{
    EVP_PKEY_free(rkey[1]);
    EVP_PKEY_free(rkey[0]);
    EVP_PKEY_CTX_free(rctx[1]);
    EVP_PKEY_CTX_free(rctx[0]);
    OSSL_PROVIDER_unload(libprov);
    OSSL_LIB_CTX_free(libctx);
    OSSL_PROVIDER_unload(nullprov);
}
