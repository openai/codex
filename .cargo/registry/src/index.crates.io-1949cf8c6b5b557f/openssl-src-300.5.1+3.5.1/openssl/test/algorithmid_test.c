/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/asn1.h>
#include <openssl/pem.h>
#include "internal/sizes.h"
#include "crypto/evp.h"
#include "testutil.h"

/* Collected arguments */
static const char *eecert_filename = NULL;  /* For test_x509_file() */
static const char *cacert_filename = NULL;  /* For test_x509_file() */
static const char *pubkey_filename = NULL;  /* For test_spki_file() */

#define ALGORITHMID_NAME "algorithm-id"

static int test_spki_aid(X509_PUBKEY *pubkey, const char *filename)
{
    const ASN1_OBJECT *oid;
    X509_ALGOR *alg = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_KEYMGMT *keymgmt = NULL;
    void *keydata = NULL;
    char name[OSSL_MAX_NAME_SIZE] = "";
    unsigned char *algid_legacy = NULL;
    int algid_legacy_len = 0;
    static unsigned char algid_prov[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t algid_prov_len = 0;
    const OSSL_PARAM *gettable_params = NULL;
    OSSL_PARAM params[] = {
        OSSL_PARAM_octet_string(ALGORITHMID_NAME,
                                &algid_prov, sizeof(algid_prov)),
        OSSL_PARAM_END
    };
    int ret = 0;

    if (!TEST_true(X509_PUBKEY_get0_param(NULL, NULL, NULL, &alg, pubkey))
        || !TEST_ptr(pkey = X509_PUBKEY_get0(pubkey)))
        goto end;

    if (!TEST_int_ge(algid_legacy_len = i2d_X509_ALGOR(alg, &algid_legacy), 0))
        goto end;

    X509_ALGOR_get0(&oid, NULL, NULL, alg);
    if (!TEST_int_gt(OBJ_obj2txt(name, sizeof(name), oid, 0), 0))
        goto end;

    /*
     * We use an internal functions to ensure we have a provided key.
     * Note that |keydata| should not be freed, as it's cached in |pkey|.
     * The |keymgmt|, however, should, as its reference count is incremented
     * in this function.
     */
    if ((keydata = evp_pkey_export_to_provider(pkey, NULL,
                                               &keymgmt, NULL)) == NULL) {
        TEST_info("The public key found in '%s' doesn't have provider support."
                  "  Skipping...",
                  filename);
        ret = 1;
        goto end;
    }

    if (!TEST_true(EVP_KEYMGMT_is_a(keymgmt, name))) {
        TEST_info("The AlgorithmID key type (%s) for the public key found in"
                  " '%s' doesn't match the key type of the extracted public"
                  " key.",
                  name, filename);
        ret = 1;
        goto end;
    }

    if (!TEST_ptr(gettable_params = EVP_KEYMGMT_gettable_params(keymgmt))
        || !TEST_ptr(OSSL_PARAM_locate_const(gettable_params, ALGORITHMID_NAME))) {
        TEST_info("The %s provider keymgmt appears to lack support for algorithm-id."
                  "  Skipping...",
                  name);
        ret = 1;
        goto end;
    }

    algid_prov[0] = '\0';
    if (!TEST_true(evp_keymgmt_get_params(keymgmt, keydata, params)))
        goto end;
    algid_prov_len = params[0].return_size;

    /* We now have all the algorithm IDs we need, let's compare them */
    if (TEST_mem_eq(algid_legacy, algid_legacy_len,
                    algid_prov, algid_prov_len))
        ret = 1;

 end:
    EVP_KEYMGMT_free(keymgmt);
    OPENSSL_free(algid_legacy);
    return ret;
}

static int test_x509_spki_aid(X509 *cert, const char *filename)
{
    X509_PUBKEY *pubkey = X509_get_X509_PUBKEY(cert);

    return test_spki_aid(pubkey, filename);
}

static int test_x509_sig_aid(X509 *eecert, const char *ee_filename,
                             X509 *cacert, const char *ca_filename)
{
    const ASN1_OBJECT *sig_oid = NULL;
    const X509_ALGOR *alg = NULL;
    int sig_nid = NID_undef, dig_nid = NID_undef, pkey_nid = NID_undef;
    EVP_MD_CTX *mdctx = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *pkey = NULL;
    unsigned char *algid_legacy = NULL;
    int algid_legacy_len = 0;
    static unsigned char algid_prov[OSSL_MAX_ALGORITHM_ID_SIZE];
    size_t algid_prov_len = 0;
    const OSSL_PARAM *gettable_params = NULL;
    OSSL_PARAM params[] = {
        OSSL_PARAM_octet_string("algorithm-id",
                                &algid_prov, sizeof(algid_prov)),
        OSSL_PARAM_END
    };
    int ret = 0;

    X509_get0_signature(NULL, &alg, eecert);
    X509_ALGOR_get0(&sig_oid, NULL, NULL, alg);
    if (!TEST_int_eq(X509_ALGOR_cmp(alg, X509_get0_tbs_sigalg(eecert)), 0))
        goto end;
    if (!TEST_int_ne(sig_nid = OBJ_obj2nid(sig_oid), NID_undef)
        || !TEST_true(OBJ_find_sigid_algs(sig_nid, &dig_nid, &pkey_nid))
        || !TEST_ptr(pkey = X509_get0_pubkey(cacert)))
        goto end;

    if (!TEST_true(EVP_PKEY_is_a(pkey, OBJ_nid2sn(pkey_nid)))) {
        TEST_info("The '%s' pubkey can't be used to verify the '%s' signature",
                  ca_filename, ee_filename);
        TEST_info("Signature algorithm is %s (pkey type %s, hash type %s)",
                  OBJ_nid2sn(sig_nid), OBJ_nid2sn(pkey_nid), OBJ_nid2sn(dig_nid));
        TEST_info("Pkey key type is %s", EVP_PKEY_get0_type_name(pkey));
        goto end;
    }

    if (!TEST_int_ge(algid_legacy_len = i2d_X509_ALGOR(alg, &algid_legacy), 0))
        goto end;

    if (!TEST_ptr(mdctx = EVP_MD_CTX_new())
        || !TEST_true(EVP_DigestVerifyInit_ex(mdctx, &pctx,
                                              OBJ_nid2sn(dig_nid),
                                              NULL, NULL, pkey, NULL))) {
        TEST_info("Couldn't initialize a DigestVerify operation with "
                  "pkey type %s and hash type %s",
                  OBJ_nid2sn(pkey_nid), OBJ_nid2sn(dig_nid));
        goto end;
    }

    if (!TEST_ptr(gettable_params = EVP_PKEY_CTX_gettable_params(pctx))
        || !TEST_ptr(OSSL_PARAM_locate_const(gettable_params, ALGORITHMID_NAME))) {
        TEST_info("The %s provider keymgmt appears to lack support for algorithm-id"
                  "  Skipping...",
                  OBJ_nid2sn(pkey_nid));
        ret = 1;
        goto end;
    }

    algid_prov[0] = '\0';
    if (!TEST_true(EVP_PKEY_CTX_get_params(pctx, params)))
        goto end;
    algid_prov_len = params[0].return_size;

    /* We now have all the algorithm IDs we need, let's compare them */
    if (TEST_mem_eq(algid_legacy, algid_legacy_len,
                    algid_prov, algid_prov_len))
        ret = 1;

 end:
    EVP_MD_CTX_free(mdctx);
    /* pctx is free by EVP_MD_CTX_free() */
    OPENSSL_free(algid_legacy);
    return ret;
}

static int test_spki_file(void)
{
    X509_PUBKEY *pubkey = NULL;
    BIO *b = BIO_new_file(pubkey_filename, "r");
    int ret = 0;

    if (b == NULL) {
        TEST_error("Couldn't open '%s' for reading\n", pubkey_filename);
        TEST_openssl_errors();
        goto end;
    }

    if ((pubkey = PEM_read_bio_X509_PUBKEY(b, NULL, NULL, NULL)) == NULL) {
        TEST_error("'%s' doesn't appear to be a SubjectPublicKeyInfo in PEM format\n",
                   pubkey_filename);
        TEST_openssl_errors();
        goto end;
    }

    ret = test_spki_aid(pubkey, pubkey_filename);
 end:
    BIO_free(b);
    X509_PUBKEY_free(pubkey);
    return ret;
}

static int test_x509_files(void)
{
    X509 *eecert = NULL, *cacert = NULL;
    BIO *bee = NULL, *bca = NULL;
    int ret = 0;

    if ((bee = BIO_new_file(eecert_filename, "r")) == NULL) {
        TEST_error("Couldn't open '%s' for reading\n", eecert_filename);
        TEST_openssl_errors();
        goto end;
    }
    if ((bca = BIO_new_file(cacert_filename, "r")) == NULL) {
        TEST_error("Couldn't open '%s' for reading\n", cacert_filename);
        TEST_openssl_errors();
        goto end;
    }

    if ((eecert = PEM_read_bio_X509(bee, NULL, NULL, NULL)) == NULL) {
        TEST_error("'%s' doesn't appear to be a X.509 certificate in PEM format\n",
                   eecert_filename);
        TEST_openssl_errors();
        goto end;
    }
    if ((cacert = PEM_read_bio_X509(bca, NULL, NULL, NULL)) == NULL) {
        TEST_error("'%s' doesn't appear to be a X.509 certificate in PEM format\n",
                   cacert_filename);
        TEST_openssl_errors();
        goto end;
    }

    ret = test_x509_sig_aid(eecert, eecert_filename, cacert, cacert_filename)
        & test_x509_spki_aid(eecert, eecert_filename)
        & test_x509_spki_aid(cacert, cacert_filename);
 end:
    BIO_free(bee);
    BIO_free(bca);
    X509_free(eecert);
    X509_free(cacert);
    return ret;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_X509,
    OPT_SPKI,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("file...\n"),
        { "x509", OPT_X509, '-', "Test X.509 certificates.  Requires two files" },
        { "spki", OPT_SPKI, '-', "Test public keys in SubjectPublicKeyInfo form.  Requires one file" },
        { OPT_HELP_STR, 1, '-',
          "file...\tFile(s) to run tests on.  All files must be PEM encoded.\n" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;
    int n, x509 = 0, spki = 0, testcount = 0;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_X509:
            x509 = 1;
            break;
        case OPT_SPKI:
            spki = 1;
            break;
        case OPT_TEST_CASES:
           break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    /* |testcount| adds all the given test types together */
    testcount = x509 + spki;

    if (testcount < 1)
        BIO_printf(bio_err, "No test type given\n");
    else if (testcount > 1)
        BIO_printf(bio_err, "Only one test type may be given\n");
    if (testcount != 1)
        return 0;

    n = test_get_argument_count();
    if (spki && n == 1) {
        pubkey_filename = test_get_argument(0);
    } else if (x509 && n == 2) {
        eecert_filename = test_get_argument(0);
        cacert_filename = test_get_argument(1);
    }

    if (spki && pubkey_filename == NULL) {
        BIO_printf(bio_err, "Missing -spki argument\n");
        return 0;
    } else if (x509 && (eecert_filename == NULL || cacert_filename == NULL)) {
        BIO_printf(bio_err, "Missing -x509 argument(s)\n");
        return 0;
    }

    if (x509)
        ADD_TEST(test_x509_files);
    if (spki)
        ADD_TEST(test_spki_file);
    return 1;
}
