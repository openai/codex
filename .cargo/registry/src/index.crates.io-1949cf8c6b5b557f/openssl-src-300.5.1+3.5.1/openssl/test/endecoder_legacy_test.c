/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This program tests the following known key type specific function against
 * the corresponding OSSL_ENCODER implementation:
 *
 * - i2d_{TYPE}PrivateKey()
 * - i2d_{TYPE}PublicKey(),
 * - i2d_{TYPE}params(),
 * - i2d_{TYPE}_PUBKEY(),
 * - PEM_write_bio_{TYPE}PrivateKey()
 * - PEM_write_bio_{TYPE}PublicKey()
 * - PEM_write_bio_{TYPE}params()
 * - PEM_write_bio_{TYPE}_PUBKEY()
 *
 * as well as the following functions against the corresponding OSSL_DECODER
 * implementation.
 *
 * - d2i_{TYPE}PrivateKey()
 * - d2i_{TYPE}PublicKey(),
 * - d2i_{TYPE}params(),
 * - d2i_{TYPE}_PUBKEY(),
 * - PEM_read_bio_{TYPE}PrivateKey()
 * - PEM_read_bio_{TYPE}PublicKey()
 * - PEM_read_bio_{TYPE}params()
 * - PEM_read_bio_{TYPE}_PUBKEY()
 */

#include <stdlib.h>
#include <string.h>

/*
 * We test deprecated functions, so we need to suppress deprecation warnings.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/asn1.h>
#include <openssl/pem.h>
#include <openssl/params.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>
#include <openssl/dh.h>
#include <openssl/dsa.h>
#ifndef OPENSSL_NO_DEPRECATED_3_0
# include <openssl/rsa.h>
#endif
#include "internal/nelem.h"
#include "crypto/evp.h"

#include "testutil.h"

typedef int PEM_write_bio_of_void_protected(BIO *out, const void *obj,
                                            const EVP_CIPHER *enc,
                                            unsigned char *kstr, int klen,
                                            pem_password_cb *cb, void *u);
typedef int PEM_write_bio_of_void_unprotected(BIO *out, const void *obj);
typedef void *PEM_read_bio_of_void(BIO *out, void **obj,
                                   pem_password_cb *cb, void *u);
typedef int EVP_PKEY_print_fn(BIO *out, const EVP_PKEY *pkey,
                              int indent, ASN1_PCTX *pctx);
typedef int EVP_PKEY_eq_fn(const EVP_PKEY *a, const EVP_PKEY *b);

static struct test_stanza_st {
    const char *keytype;
    const char *structure[2];
    int evp_type;

    i2d_of_void *i2d_PrivateKey;
    i2d_of_void *i2d_PublicKey;
    i2d_of_void *i2d_params;
    i2d_of_void *i2d_PUBKEY;
    PEM_write_bio_of_void_protected *pem_write_bio_PrivateKey;
    PEM_write_bio_of_void_unprotected *pem_write_bio_PublicKey;
    PEM_write_bio_of_void_unprotected *pem_write_bio_params;
    PEM_write_bio_of_void_unprotected *pem_write_bio_PUBKEY;

    d2i_of_void *d2i_PrivateKey;
    d2i_of_void *d2i_PublicKey;
    d2i_of_void *d2i_params;
    d2i_of_void *d2i_PUBKEY;
    PEM_read_bio_of_void *pem_read_bio_PrivateKey;
    PEM_read_bio_of_void *pem_read_bio_PublicKey;
    PEM_read_bio_of_void *pem_read_bio_params;
    PEM_read_bio_of_void *pem_read_bio_PUBKEY;
} test_stanzas[] = {
#ifndef OPENSSL_NO_DH
    { "DH", { "DH", "type-specific" }, EVP_PKEY_DH,
      NULL,                      /* No i2d_DHPrivateKey */
      NULL,                      /* No i2d_DHPublicKey */
      (i2d_of_void *)i2d_DHparams,
      NULL,                      /* No i2d_DH_PUBKEY */
      NULL,                      /* No PEM_write_bio_DHPrivateKey */
      NULL,                      /* No PEM_write_bio_DHPublicKey */
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_DHparams,
      NULL,                      /* No PEM_write_bio_DH_PUBKEY */
      NULL,                      /* No d2i_DHPrivateKey */
      NULL,                      /* No d2i_DHPublicKey */
      (d2i_of_void *)d2i_DHparams,
      NULL,                      /* No d2i_DH_PUBKEY */
      NULL,                      /* No PEM_read_bio_DHPrivateKey */
      NULL,                      /* No PEM_read_bio_DHPublicKey */
      (PEM_read_bio_of_void *)PEM_read_bio_DHparams,
      NULL },                    /* No PEM_read_bio_DH_PUBKEY */
    { "DHX", { "DHX", "type-specific" }, EVP_PKEY_DHX,
      NULL,                      /* No i2d_DHxPrivateKey */
      NULL,                      /* No i2d_DHxPublicKey */
      (i2d_of_void *)i2d_DHxparams,
      NULL,                      /* No i2d_DHx_PUBKEY */
      NULL,                      /* No PEM_write_bio_DHxPrivateKey */
      NULL,                      /* No PEM_write_bio_DHxPublicKey */
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_DHxparams,
      NULL,                      /* No PEM_write_bio_DHx_PUBKEY */
      NULL,                      /* No d2i_DHxPrivateKey */
      NULL,                      /* No d2i_DHxPublicKey */
      (d2i_of_void *)d2i_DHxparams,
      NULL,                      /* No d2i_DHx_PUBKEY */
      NULL,                      /* No PEM_read_bio_DHxPrivateKey */
      NULL,                      /* No PEM_read_bio_DHxPublicKey */
      NULL,                      /* No PEM_read_bio_DHxparams */
      NULL },                    /* No PEM_read_bio_DHx_PUBKEY */
#endif
#ifndef OPENSSL_NO_DSA
    { "DSA", { "DSA", "type-specific" }, EVP_PKEY_DSA,
      (i2d_of_void *)i2d_DSAPrivateKey,
      (i2d_of_void *)i2d_DSAPublicKey,
      (i2d_of_void *)i2d_DSAparams,
      (i2d_of_void *)i2d_DSA_PUBKEY,
      (PEM_write_bio_of_void_protected *)PEM_write_bio_DSAPrivateKey,
      NULL,                      /* No PEM_write_bio_DSAPublicKey */
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_DSAparams,
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_DSA_PUBKEY,
      (d2i_of_void *)d2i_DSAPrivateKey,
      (d2i_of_void *)d2i_DSAPublicKey,
      (d2i_of_void *)d2i_DSAparams,
      (d2i_of_void *)d2i_DSA_PUBKEY,
      (PEM_read_bio_of_void *)PEM_read_bio_DSAPrivateKey,
      NULL,                      /* No PEM_write_bio_DSAPublicKey */
      (PEM_read_bio_of_void *)PEM_read_bio_DSAparams,
      (PEM_read_bio_of_void *)PEM_read_bio_DSA_PUBKEY },
#endif
#ifndef OPENSSL_NO_EC
    { "EC", { "EC", "type-specific" }, EVP_PKEY_EC,
      (i2d_of_void *)i2d_ECPrivateKey,
      NULL,                      /* No i2d_ECPublicKey */
      (i2d_of_void *)i2d_ECParameters,
      (i2d_of_void *)i2d_EC_PUBKEY,
      (PEM_write_bio_of_void_protected *)PEM_write_bio_ECPrivateKey,
      NULL,                      /* No PEM_write_bio_ECPublicKey */
      NULL,                      /* No PEM_write_bio_ECParameters */
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_EC_PUBKEY,
      (d2i_of_void *)d2i_ECPrivateKey,
      NULL,                      /* No d2i_ECPublicKey */
      (d2i_of_void *)d2i_ECParameters,
      (d2i_of_void *)d2i_EC_PUBKEY,
      (PEM_read_bio_of_void *)PEM_read_bio_ECPrivateKey,
      NULL,                      /* No PEM_read_bio_ECPublicKey */
      NULL,                      /* No PEM_read_bio_ECParameters */
      (PEM_read_bio_of_void *)PEM_read_bio_EC_PUBKEY, },
#endif
    { "RSA", { "RSA", "type-specific" }, EVP_PKEY_RSA,
      (i2d_of_void *)i2d_RSAPrivateKey,
      (i2d_of_void *)i2d_RSAPublicKey,
      NULL,                      /* No i2d_RSAparams */
      (i2d_of_void *)i2d_RSA_PUBKEY,
      (PEM_write_bio_of_void_protected *)PEM_write_bio_RSAPrivateKey,
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_RSAPublicKey,
      NULL,                      /* No PEM_write_bio_RSAparams */
      (PEM_write_bio_of_void_unprotected *)PEM_write_bio_RSA_PUBKEY,
      (d2i_of_void *)d2i_RSAPrivateKey,
      (d2i_of_void *)d2i_RSAPublicKey,
      NULL,                      /* No d2i_RSAparams */
      (d2i_of_void *)d2i_RSA_PUBKEY,
      (PEM_read_bio_of_void *)PEM_read_bio_RSAPrivateKey,
      (PEM_read_bio_of_void *)PEM_read_bio_RSAPublicKey,
      NULL,                      /* No PEM_read_bio_RSAparams */
      (PEM_read_bio_of_void *)PEM_read_bio_RSA_PUBKEY }
};

/*
 * Keys that we're going to test with.  We initialize this with the intended
 * key types, and generate the keys themselves on program setup.
 * They must all be downgradable with EVP_PKEY_get0()
 */

#ifndef OPENSSL_NO_DH
static const OSSL_PARAM DH_params[] = { OSSL_PARAM_END };
static const OSSL_PARAM DHX_params[] = { OSSL_PARAM_END };
#endif
#ifndef OPENSSL_NO_DSA
static size_t qbits = 160;  /* PVK only tolerates 160 Q bits */
static size_t pbits = 1024; /* With 160 Q bits, we MUST use 1024 P bits */
static const OSSL_PARAM DSA_params[] = {
    OSSL_PARAM_size_t("pbits", &pbits),
    OSSL_PARAM_size_t("qbits", &qbits),
    OSSL_PARAM_END
};
#endif
#ifndef OPENSSL_NO_EC
static char groupname[] = "prime256v1";
static const OSSL_PARAM EC_params[] = {
    OSSL_PARAM_utf8_string("group", groupname, sizeof(groupname) - 1),
    OSSL_PARAM_END
};
#endif

static struct key_st {
    const char *keytype;
    int evp_type;
    /* non-NULL if a template EVP_PKEY must be generated first */
    const OSSL_PARAM *template_params;

    EVP_PKEY *key;
} keys[] = {
#ifndef OPENSSL_NO_DH
    { "DH", EVP_PKEY_DH, DH_params, NULL },
    { "DHX", EVP_PKEY_DHX, DHX_params, NULL },
#endif
#ifndef OPENSSL_NO_DSA
    { "DSA", EVP_PKEY_DSA, DSA_params, NULL },
#endif
#ifndef OPENSSL_NO_EC
    { "EC", EVP_PKEY_EC, EC_params, NULL },
#endif
#ifndef OPENSSL_NO_DEPRECATED_3_0
    { "RSA", EVP_PKEY_RSA, NULL, NULL },
#endif
};

static EVP_PKEY *make_key(const char *type,
                          const OSSL_PARAM *gen_template_params)
{
    EVP_PKEY *template = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM *gen_template_params_noconst =
        (OSSL_PARAM *)gen_template_params;

    if (gen_template_params != NULL
        && ((ctx = EVP_PKEY_CTX_new_from_name(NULL, type, NULL)) == NULL
            || EVP_PKEY_paramgen_init(ctx) <= 0
            || (gen_template_params[0].key != NULL
                && EVP_PKEY_CTX_set_params(ctx, gen_template_params_noconst) <= 0)
            || EVP_PKEY_generate(ctx, &template) <= 0))
        goto end;
    EVP_PKEY_CTX_free(ctx);

    /*
     * No real need to check the errors other than for the cascade
     * effect.  |pkey| will simply remain NULL if something goes wrong.
     */
    ctx =
        template != NULL
        ? EVP_PKEY_CTX_new(template, NULL)
        : EVP_PKEY_CTX_new_from_name(NULL, type, NULL);

    (void)(ctx != NULL
           && EVP_PKEY_keygen_init(ctx) > 0
           && EVP_PKEY_keygen(ctx, &pkey) > 0);

 end:
    EVP_PKEY_free(template);
    EVP_PKEY_CTX_free(ctx);
    return pkey;
}

static struct key_st *lookup_key(const char *type)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(keys); i++) {
        if (strcmp(keys[i].keytype, type) == 0)
            return &keys[i];
    }
    return NULL;
}

static int test_membio_str_eq(BIO *bio_provided, BIO *bio_legacy)
{
    char *str_provided = NULL, *str_legacy = NULL;
    long len_provided = BIO_get_mem_data(bio_provided, &str_provided);
    long len_legacy = BIO_get_mem_data(bio_legacy, &str_legacy);

    return TEST_long_ge(len_legacy, 0)
           && TEST_long_ge(len_provided, 0)
           && TEST_strn2_eq(str_provided, len_provided,
                            str_legacy, len_legacy);
}

static int test_protected_PEM(const char *keytype, int evp_type,
                              const void *legacy_key,
                              PEM_write_bio_of_void_protected *pem_write_bio,
                              PEM_read_bio_of_void *pem_read_bio,
                              EVP_PKEY_eq_fn *evp_pkey_eq,
                              EVP_PKEY_print_fn *evp_pkey_print,
                              EVP_PKEY *provided_pkey, int selection,
                              const char *structure)
{
    int ok = 0;
    BIO *membio_legacy = NULL;
    BIO *membio_provided = NULL;
    OSSL_ENCODER_CTX *ectx = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    void *decoded_legacy_key = NULL;
    EVP_PKEY *decoded_legacy_pkey = NULL;
    EVP_PKEY *decoded_provided_pkey = NULL;

    /* Set up the BIOs, so we have them */
    if (!TEST_ptr(membio_legacy = BIO_new(BIO_s_mem()))
        || !TEST_ptr(membio_provided = BIO_new(BIO_s_mem())))
        goto end;

    if (!TEST_ptr(ectx =
                  OSSL_ENCODER_CTX_new_for_pkey(provided_pkey, selection,
                                                "PEM", structure,
                                                NULL))
        || !TEST_true(OSSL_ENCODER_to_bio(ectx, membio_provided))
        || !TEST_true(pem_write_bio(membio_legacy, legacy_key,
                                   NULL, NULL, 0, NULL, NULL))
        || !test_membio_str_eq(membio_provided, membio_legacy))
        goto end;

    if (pem_read_bio != NULL) {
        /* Now try decoding the results and compare the resulting keys */

        if (!TEST_ptr(decoded_legacy_pkey = EVP_PKEY_new())
            || !TEST_ptr(dctx =
                         OSSL_DECODER_CTX_new_for_pkey(&decoded_provided_pkey,
                                                       "PEM", structure,
                                                       keytype, selection,
                                                       NULL, NULL))
            || !TEST_true(OSSL_DECODER_from_bio(dctx, membio_provided))
            || !TEST_ptr(decoded_legacy_key =
                         pem_read_bio(membio_legacy, NULL, NULL, NULL))
            || !TEST_true(EVP_PKEY_assign(decoded_legacy_pkey, evp_type,
                                          decoded_legacy_key)))
            goto end;

        if (!TEST_int_gt(evp_pkey_eq(decoded_provided_pkey,
                                     decoded_legacy_pkey), 0)) {
            TEST_info("decoded_provided_pkey:");
            evp_pkey_print(bio_out, decoded_provided_pkey, 0, NULL);
            TEST_info("decoded_legacy_pkey:");
            evp_pkey_print(bio_out, decoded_legacy_pkey, 0, NULL);
        }
    }
    ok = 1;
 end:
    EVP_PKEY_free(decoded_legacy_pkey);
    EVP_PKEY_free(decoded_provided_pkey);
    OSSL_ENCODER_CTX_free(ectx);
    OSSL_DECODER_CTX_free(dctx);
    BIO_free(membio_provided);
    BIO_free(membio_legacy);
    return ok;
}

static int test_unprotected_PEM(const char *keytype, int evp_type,
                                const void *legacy_key,
                                PEM_write_bio_of_void_unprotected *pem_write_bio,
                                PEM_read_bio_of_void *pem_read_bio,
                                EVP_PKEY_eq_fn *evp_pkey_eq,
                                EVP_PKEY_print_fn *evp_pkey_print,
                                EVP_PKEY *provided_pkey, int selection,
                                const char *structure)
{
    int ok = 0;
    BIO *membio_legacy = NULL;
    BIO *membio_provided = NULL;
    OSSL_ENCODER_CTX *ectx = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    void *decoded_legacy_key = NULL;
    EVP_PKEY *decoded_legacy_pkey = NULL;
    EVP_PKEY *decoded_provided_pkey = NULL;

    /* Set up the BIOs, so we have them */
    if (!TEST_ptr(membio_legacy = BIO_new(BIO_s_mem()))
        || !TEST_ptr(membio_provided = BIO_new(BIO_s_mem())))
        goto end;

    if (!TEST_ptr(ectx =
                  OSSL_ENCODER_CTX_new_for_pkey(provided_pkey, selection,
                                                "PEM", structure,
                                                NULL))
        || !TEST_true(OSSL_ENCODER_to_bio(ectx, membio_provided))
        || !TEST_true(pem_write_bio(membio_legacy, legacy_key))
        || !test_membio_str_eq(membio_provided, membio_legacy))
        goto end;

    if (pem_read_bio != NULL) {
        /* Now try decoding the results and compare the resulting keys */

        if (!TEST_ptr(decoded_legacy_pkey = EVP_PKEY_new())
            || !TEST_ptr(dctx =
                         OSSL_DECODER_CTX_new_for_pkey(&decoded_provided_pkey,
                                                       "PEM", structure,
                                                       keytype, selection,
                                                       NULL, NULL))
            || !TEST_true(OSSL_DECODER_from_bio(dctx, membio_provided))
            || !TEST_ptr(decoded_legacy_key =
                         pem_read_bio(membio_legacy, NULL, NULL, NULL))
            || !TEST_true(EVP_PKEY_assign(decoded_legacy_pkey, evp_type,
                                          decoded_legacy_key)))
            goto end;

        if (!TEST_int_gt(evp_pkey_eq(decoded_provided_pkey,
                                     decoded_legacy_pkey), 0)) {
            TEST_info("decoded_provided_pkey:");
            evp_pkey_print(bio_out, decoded_provided_pkey, 0, NULL);
            TEST_info("decoded_legacy_pkey:");
            evp_pkey_print(bio_out, decoded_legacy_pkey, 0, NULL);
        }
    }
    ok = 1;
 end:
    EVP_PKEY_free(decoded_legacy_pkey);
    EVP_PKEY_free(decoded_provided_pkey);
    OSSL_ENCODER_CTX_free(ectx);
    OSSL_DECODER_CTX_free(dctx);
    BIO_free(membio_provided);
    BIO_free(membio_legacy);
    return ok;
}

static int test_DER(const char *keytype, int evp_type,
                    const void *legacy_key, i2d_of_void *i2d, d2i_of_void *d2i,
                    EVP_PKEY_eq_fn *evp_pkey_eq,
                    EVP_PKEY_print_fn *evp_pkey_print,
                    EVP_PKEY *provided_pkey, int selection,
                    const char *structure)
{
    int ok = 0;
    unsigned char *der_legacy = NULL;
    const unsigned char *pder_legacy = NULL;
    size_t der_legacy_len = 0;
    unsigned char *der_provided = NULL;
    const unsigned char *pder_provided = NULL;
    size_t der_provided_len = 0;
    size_t tmp_size;
    OSSL_ENCODER_CTX *ectx = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    void *decoded_legacy_key = NULL;
    EVP_PKEY *decoded_legacy_pkey = NULL;
    EVP_PKEY *decoded_provided_pkey = NULL;

    if (!TEST_ptr(ectx =
                 OSSL_ENCODER_CTX_new_for_pkey(provided_pkey, selection,
                                               "DER", structure,
                                               NULL))
        || !TEST_true(OSSL_ENCODER_to_data(ectx,
                                          &der_provided, &der_provided_len))
        || !TEST_size_t_gt(der_legacy_len = i2d(legacy_key, &der_legacy), 0)
        || !TEST_mem_eq(der_provided, der_provided_len,
                        der_legacy, der_legacy_len))
        goto end;

    if (d2i != NULL) {
        /* Now try decoding the results and compare the resulting keys */

        if (!TEST_ptr(decoded_legacy_pkey = EVP_PKEY_new())
            || !TEST_ptr(dctx =
                         OSSL_DECODER_CTX_new_for_pkey(&decoded_provided_pkey,
                                                       "DER", structure,
                                                       keytype, selection,
                                                       NULL, NULL))
            || !TEST_true((pder_provided = der_provided,
                           tmp_size = der_provided_len,
                           OSSL_DECODER_from_data(dctx, &pder_provided,
                                                  &tmp_size)))
            || !TEST_ptr((pder_legacy = der_legacy,
                          decoded_legacy_key = d2i(NULL, &pder_legacy,
                                                   (long)der_legacy_len)))
            || !TEST_true(EVP_PKEY_assign(decoded_legacy_pkey, evp_type,
                                          decoded_legacy_key)))
            goto end;

        if (!TEST_int_gt(evp_pkey_eq(decoded_provided_pkey,
                                     decoded_legacy_pkey), 0)) {
            TEST_info("decoded_provided_pkey:");
            evp_pkey_print(bio_out, decoded_provided_pkey, 0, NULL);
            TEST_info("decoded_legacy_pkey:");
            evp_pkey_print(bio_out, decoded_legacy_pkey, 0, NULL);
        }
    }
    ok = 1;
 end:
    EVP_PKEY_free(decoded_legacy_pkey);
    EVP_PKEY_free(decoded_provided_pkey);
    OSSL_ENCODER_CTX_free(ectx);
    OSSL_DECODER_CTX_free(dctx);
    OPENSSL_free(der_provided);
    OPENSSL_free(der_legacy);
    return ok;
}

static int test_key(int idx)
{
    struct test_stanza_st *test_stanza = NULL;
    struct key_st *key = NULL;
    int ok = 0;
    size_t i;
    EVP_PKEY *pkey = NULL, *downgraded_pkey = NULL;
    const void *legacy_obj = NULL;

    /* Get the test data */
    if (!TEST_ptr(test_stanza = &test_stanzas[idx])
        || !TEST_ptr(key = lookup_key(test_stanza->keytype)))
        goto end;

    /* Set up the keys */
    if (!TEST_ptr(pkey = key->key)
        || !TEST_true(evp_pkey_copy_downgraded(&downgraded_pkey, pkey))
        || !TEST_ptr(downgraded_pkey)
        || !TEST_int_eq(EVP_PKEY_get_id(downgraded_pkey), key->evp_type)
        || !TEST_ptr(legacy_obj = EVP_PKEY_get0(downgraded_pkey)))
        goto end;

    ok = 1;

    /* Test PrivateKey to PEM */
    if (test_stanza->pem_write_bio_PrivateKey != NULL) {
        int selection = OSSL_KEYMGMT_SELECT_ALL;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against PEM_write_bio_{TYPE}PrivateKey for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_protected_PEM(key->keytype, key->evp_type, legacy_obj,
                                    test_stanza->pem_write_bio_PrivateKey,
                                    test_stanza->pem_read_bio_PrivateKey,
                                    EVP_PKEY_eq, EVP_PKEY_print_private,
                                    pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test PublicKey to PEM */
    if (test_stanza->pem_write_bio_PublicKey != NULL) {
        int selection =
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY
            | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against PEM_write_bio_{TYPE}PublicKey for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_unprotected_PEM(key->keytype, key->evp_type, legacy_obj,
                                      test_stanza->pem_write_bio_PublicKey,
                                      test_stanza->pem_read_bio_PublicKey,
                                      EVP_PKEY_eq, EVP_PKEY_print_public,
                                      pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test params to PEM */
    if (test_stanza->pem_write_bio_params != NULL) {
        int selection = OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against PEM_write_bio_{TYPE}params for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_unprotected_PEM(key->keytype, key->evp_type, legacy_obj,
                                      test_stanza->pem_write_bio_params,
                                      test_stanza->pem_read_bio_params,
                                      EVP_PKEY_parameters_eq,
                                      EVP_PKEY_print_params,
                                      pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test PUBKEY to PEM */
    if (test_stanza->pem_write_bio_PUBKEY != NULL) {
        int selection =
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY
            | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;
        const char *structure = "SubjectPublicKeyInfo";

        TEST_info("Test OSSL_ENCODER against PEM_write_bio_{TYPE}_PUBKEY for %s, %s",
                  test_stanza->keytype, structure);
        if (!test_unprotected_PEM(key->keytype, key->evp_type, legacy_obj,
                                  test_stanza->pem_write_bio_PUBKEY,
                                  test_stanza->pem_read_bio_PUBKEY,
                                  EVP_PKEY_eq, EVP_PKEY_print_public,
                                  pkey, selection, structure))
            ok = 0;
    }


    /* Test PrivateKey to DER */
    if (test_stanza->i2d_PrivateKey != NULL) {
        int selection = OSSL_KEYMGMT_SELECT_ALL;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against i2d_{TYPE}PrivateKey for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_DER(key->keytype, key->evp_type, legacy_obj,
                          test_stanza->i2d_PrivateKey,
                          test_stanza->d2i_PrivateKey,
                          EVP_PKEY_eq, EVP_PKEY_print_private,
                          pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test PublicKey to DER */
    if (test_stanza->i2d_PublicKey != NULL) {
        int selection =
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY
            | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against i2d_{TYPE}PublicKey for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_DER(key->keytype, key->evp_type, legacy_obj,
                          test_stanza->i2d_PublicKey,
                          test_stanza->d2i_PublicKey,
                          EVP_PKEY_eq, EVP_PKEY_print_public,
                          pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test params to DER */
    if (test_stanza->i2d_params != NULL) {
        int selection = OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;

        for (i = 0; i < OSSL_NELEM(test_stanza->structure); i++) {
            const char *structure = test_stanza->structure[i];

            TEST_info("Test OSSL_ENCODER against i2d_{TYPE}params for %s, %s",
                      test_stanza->keytype, structure);
            if (!test_DER(key->keytype, key->evp_type, legacy_obj,
                          test_stanza->i2d_params, test_stanza->d2i_params,
                          EVP_PKEY_parameters_eq, EVP_PKEY_print_params,
                          pkey, selection, structure))
                ok = 0;
        }
    }

    /* Test PUBKEY to DER */
    if (test_stanza->i2d_PUBKEY != NULL) {
        int selection =
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY
            | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS;
        const char *structure = "SubjectPublicKeyInfo";

        TEST_info("Test OSSL_ENCODER against i2d_{TYPE}_PUBKEY for %s, %s",
                  test_stanza->keytype, structure);
        if (!test_DER(key->keytype, key->evp_type, legacy_obj,
                      test_stanza->i2d_PUBKEY, test_stanza->d2i_PUBKEY,
                      EVP_PKEY_eq, EVP_PKEY_print_public,
                      pkey, selection, structure))
            ok = 0;
    }
 end:
    EVP_PKEY_free(downgraded_pkey);
    return ok;
}

#define USAGE "rsa-key.pem dh-key.pem\n"
OPT_TEST_DECLARE_USAGE(USAGE)

int setup_tests(void)
{
    size_t i;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }
    if (test_get_argument_count() != 2) {
        TEST_error("usage: endecoder_legacy_test %s", USAGE);
        return 0;
    }

    TEST_info("Generating keys...");

    for (i = 0; i < OSSL_NELEM(keys); i++) {
#ifndef OPENSSL_NO_DH
        if (strcmp(keys[i].keytype, "DH") == 0) {
            if (!TEST_ptr(keys[i].key =
                          load_pkey_pem(test_get_argument(1), NULL)))
                return  0;
            continue;
        }
#endif
#ifndef OPENSSL_NO_DEPRECATED_3_0
        if (strcmp(keys[i].keytype, "RSA") == 0) {
            if (!TEST_ptr(keys[i].key =
                          load_pkey_pem(test_get_argument(0), NULL)))
                return  0;
            continue;
        }
#endif
        TEST_info("Generating %s key...", keys[i].keytype);
        if (!TEST_ptr(keys[i].key =
                      make_key(keys[i].keytype, keys[i].template_params)))
            return 0;
    }

    TEST_info("Generating keys done");

    ADD_ALL_TESTS(test_key, OSSL_NELEM(test_stanzas));
    return 1;
}

void cleanup_tests(void)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(keys); i++)
        EVP_PKEY_free(keys[i].key);
}
