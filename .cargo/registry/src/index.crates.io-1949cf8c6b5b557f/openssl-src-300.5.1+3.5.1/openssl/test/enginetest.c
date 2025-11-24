/*
 * Copyright 2000-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <openssl/e_os2.h>

# include "testutil.h"

#ifndef OPENSSL_NO_ENGINE
# include <openssl/buffer.h>
# include <openssl/crypto.h>
# include <openssl/engine.h>
# include <openssl/rsa.h>
# include <openssl/err.h>
# include <openssl/x509.h>
# include <openssl/pem.h>

static void display_engine_list(void)
{
    ENGINE *h;
    int loop;

    loop = 0;
    for (h = ENGINE_get_first(); h != NULL; h = ENGINE_get_next(h)) {
        TEST_info("#%d: id = \"%s\", name = \"%s\"",
               loop++, ENGINE_get_id(h), ENGINE_get_name(h));
    }

    /*
     * ENGINE_get_first() increases the struct_ref counter, so we must call
     * ENGINE_free() to decrease it again
     */
    ENGINE_free(h);
}

#define NUMTOADD 512

static int test_engines(void)
{
    ENGINE *block[NUMTOADD];
    char *eid[NUMTOADD];
    char *ename[NUMTOADD];
    char buf[256];
    ENGINE *ptr;
    int loop;
    int to_return = 0;
    ENGINE *new_h1 = NULL;
    ENGINE *new_h2 = NULL;
    ENGINE *new_h3 = NULL;
    ENGINE *new_h4 = NULL;

    memset(block, 0, sizeof(block));
    if (!TEST_ptr(new_h1 = ENGINE_new())
            || !TEST_true(ENGINE_set_id(new_h1, "test_id0"))
            || !TEST_true(ENGINE_set_name(new_h1, "First test item"))
            || !TEST_ptr(new_h2 = ENGINE_new())
            || !TEST_true(ENGINE_set_id(new_h2, "test_id1"))
            || !TEST_true(ENGINE_set_name(new_h2, "Second test item"))
            || !TEST_ptr(new_h3 = ENGINE_new())
            || !TEST_true(ENGINE_set_id(new_h3, "test_id2"))
            || !TEST_true(ENGINE_set_name(new_h3, "Third test item"))
            || !TEST_ptr(new_h4 = ENGINE_new())
            || !TEST_true(ENGINE_set_id(new_h4, "test_id3"))
            || !TEST_true(ENGINE_set_name(new_h4, "Fourth test item")))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_add(new_h1)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    ptr = ENGINE_get_first();
    if (!TEST_true(ENGINE_remove(ptr)))
        goto end;
    ENGINE_free(ptr);
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_add(new_h3))
            || !TEST_true(ENGINE_add(new_h2)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_remove(new_h2)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_add(new_h4)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    /* Should fail. */
    if (!TEST_false(ENGINE_add(new_h3)))
        goto end;
    ERR_clear_error();

    /* Should fail. */
    if (!TEST_false(ENGINE_remove(new_h2)))
        goto end;
    ERR_clear_error();

    if (!TEST_true(ENGINE_remove(new_h3)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_remove(new_h4)))
        goto end;
    TEST_info("Engines:");
    display_engine_list();

    /*
     * At this point, we should have an empty list, unless some hardware
     * support engine got added.  However, since we don't allow the config
     * file to be loaded and don't otherwise load any built in engines,
     * that is unlikely.  Still, we check, if for nothing else, then to
     * notify that something is a little off (and might mean that |new_h1|
     * wasn't unloaded when it should have)
     */
    if ((ptr = ENGINE_get_first()) != NULL) {
        if (!ENGINE_remove(ptr))
            TEST_info("Remove failed - probably no hardware support present");
    }
    ENGINE_free(ptr);
    TEST_info("Engines:");
    display_engine_list();

    if (!TEST_true(ENGINE_add(new_h1))
            || !TEST_true(ENGINE_remove(new_h1)))
        goto end;

    TEST_info("About to beef up the engine-type list");
    for (loop = 0; loop < NUMTOADD; loop++) {
        BIO_snprintf(buf, sizeof(buf), "id%d", loop);
        eid[loop] = OPENSSL_strdup(buf);
        BIO_snprintf(buf, sizeof(buf), "Fake engine type %d", loop);
        ename[loop] = OPENSSL_strdup(buf);
        if (!TEST_ptr(block[loop] = ENGINE_new())
                || !TEST_true(ENGINE_set_id(block[loop], eid[loop]))
                || !TEST_true(ENGINE_set_name(block[loop], ename[loop])))
            goto end;
    }
    for (loop = 0; loop < NUMTOADD; loop++) {
        if (!TEST_true(ENGINE_add(block[loop]))) {
            test_note("Adding stopped at %d, (%s,%s)",
                      loop, ENGINE_get_id(block[loop]),
                      ENGINE_get_name(block[loop]));
            goto cleanup_loop;
        }
    }
 cleanup_loop:
    TEST_info("About to empty the engine-type list");
    while ((ptr = ENGINE_get_first()) != NULL) {
        if (!TEST_true(ENGINE_remove(ptr)))
            goto end;
        ENGINE_free(ptr);
    }
    for (loop = 0; loop < NUMTOADD; loop++) {
        OPENSSL_free(eid[loop]);
        OPENSSL_free(ename[loop]);
    }
    to_return = 1;

 end:
    ENGINE_free(new_h1);
    ENGINE_free(new_h2);
    ENGINE_free(new_h3);
    ENGINE_free(new_h4);
    for (loop = 0; loop < NUMTOADD; loop++)
        ENGINE_free(block[loop]);
    return to_return;
}

/* Test EVP_PKEY method */
static EVP_PKEY_METHOD *test_rsa = NULL;

static int called_encrypt = 0;

/* Test function to check operation has been redirected */
static int test_encrypt(EVP_PKEY_CTX *ctx, unsigned char *sig,
                        size_t *siglen, const unsigned char *tbs, size_t tbslen)
{
    called_encrypt = 1;
    return 1;
}

static int test_pkey_meths(ENGINE *e, EVP_PKEY_METHOD **pmeth,
                           const int **pnids, int nid)
{
    static const int rnid = EVP_PKEY_RSA;
    if (pmeth == NULL) {
        *pnids = &rnid;
        return 1;
    }

    if (nid == EVP_PKEY_RSA) {
        *pmeth = test_rsa;
        return 1;
    }

    *pmeth = NULL;
    return 0;
}

/* Return a test EVP_PKEY value */

static EVP_PKEY *get_test_pkey(void)
{
    static unsigned char n[] =
        "\x00\xAA\x36\xAB\xCE\x88\xAC\xFD\xFF\x55\x52\x3C\x7F\xC4\x52\x3F"
        "\x90\xEF\xA0\x0D\xF3\x77\x4A\x25\x9F\x2E\x62\xB4\xC5\xD9\x9C\xB5"
        "\xAD\xB3\x00\xA0\x28\x5E\x53\x01\x93\x0E\x0C\x70\xFB\x68\x76\x93"
        "\x9C\xE6\x16\xCE\x62\x4A\x11\xE0\x08\x6D\x34\x1E\xBC\xAC\xA0\xA1"
        "\xF5";
    static unsigned char e[] = "\x11";

    RSA *rsa = RSA_new();
    EVP_PKEY *pk = EVP_PKEY_new();

    if (rsa == NULL || pk == NULL || !EVP_PKEY_assign_RSA(pk, rsa)) {
        RSA_free(rsa);
        EVP_PKEY_free(pk);
        return NULL;
    }

    if (!RSA_set0_key(rsa, BN_bin2bn(n, sizeof(n)-1, NULL),
                      BN_bin2bn(e, sizeof(e)-1, NULL), NULL)) {
        EVP_PKEY_free(pk);
        return NULL;
    }

    return pk;
}

static int test_redirect(void)
{
    const unsigned char pt[] = "Hello World\n";
    unsigned char *tmp = NULL;
    size_t len;
    EVP_PKEY_CTX *ctx = NULL;
    ENGINE *e = NULL;
    EVP_PKEY *pkey = NULL;

    int to_return = 0;

    if (!TEST_ptr(pkey = get_test_pkey()))
        goto err;

    len = EVP_PKEY_get_size(pkey);
    if (!TEST_ptr(tmp = OPENSSL_malloc(len)))
        goto err;

    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new(pkey, NULL)))
        goto err;
    TEST_info("EVP_PKEY_encrypt test: no redirection");
    /* Encrypt some data: should succeed but not be redirected */
    if (!TEST_int_gt(EVP_PKEY_encrypt_init(ctx), 0)
            || !TEST_int_gt(EVP_PKEY_encrypt(ctx, tmp, &len, pt, sizeof(pt)), 0)
            || !TEST_false(called_encrypt))
        goto err;
    EVP_PKEY_CTX_free(ctx);
    ctx = NULL;

    /* Create a test ENGINE */
    if (!TEST_ptr(e = ENGINE_new())
            || !TEST_true(ENGINE_set_id(e, "Test redirect engine"))
            || !TEST_true(ENGINE_set_name(e, "Test redirect engine")))
        goto err;

    /*
     * Try to create a context for this engine and test key.
     * Try setting test key engine. Both should fail because the
     * engine has no public key methods.
     */
    if (!TEST_ptr_null(ctx = EVP_PKEY_CTX_new(pkey, e))
            || !TEST_int_le(EVP_PKEY_set1_engine(pkey, e), 0))
        goto err;

    /* Setup an empty test EVP_PKEY_METHOD and set callback to return it */
    if (!TEST_ptr(test_rsa = EVP_PKEY_meth_new(EVP_PKEY_RSA, 0)))
        goto err;
    ENGINE_set_pkey_meths(e, test_pkey_meths);

    /* Getting a context for test ENGINE should now succeed */
    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new(pkey, e)))
        goto err;
    /* Encrypt should fail because operation is not supported */
    if (!TEST_int_le(EVP_PKEY_encrypt_init(ctx), 0))
        goto err;
    EVP_PKEY_CTX_free(ctx);
    ctx = NULL;

    /* Add test encrypt operation to method */
    EVP_PKEY_meth_set_encrypt(test_rsa, 0, test_encrypt);

    TEST_info("EVP_PKEY_encrypt test: redirection via EVP_PKEY_CTX_new()");
    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new(pkey, e)))
        goto err;
    /* Encrypt some data: should succeed and be redirected */
    if (!TEST_int_gt(EVP_PKEY_encrypt_init(ctx), 0)
            || !TEST_int_gt(EVP_PKEY_encrypt(ctx, tmp, &len, pt, sizeof(pt)), 0)
            || !TEST_true(called_encrypt))
        goto err;

    EVP_PKEY_CTX_free(ctx);
    ctx = NULL;
    called_encrypt = 0;

    /* Create context with default engine: should not be redirected */
    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new(pkey, NULL))
            || !TEST_int_gt(EVP_PKEY_encrypt_init(ctx), 0)
            || !TEST_int_gt(EVP_PKEY_encrypt(ctx, tmp, &len, pt, sizeof(pt)), 0)
            || !TEST_false(called_encrypt))
        goto err;

    EVP_PKEY_CTX_free(ctx);
    ctx = NULL;

    /* Set engine explicitly for test key */
    if (!TEST_true(EVP_PKEY_set1_engine(pkey, e)))
        goto err;

    TEST_info("EVP_PKEY_encrypt test: redirection via EVP_PKEY_set1_engine()");

    /* Create context with default engine: should be redirected now */
    if (!TEST_ptr(ctx = EVP_PKEY_CTX_new(pkey, NULL))
            || !TEST_int_gt(EVP_PKEY_encrypt_init(ctx), 0)
            || !TEST_int_gt(EVP_PKEY_encrypt(ctx, tmp, &len, pt, sizeof(pt)), 0)
            || !TEST_true(called_encrypt))
        goto err;

    to_return = 1;

 err:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    ENGINE_free(e);
    OPENSSL_free(tmp);
    return to_return;
}

static int test_x509_dup_w_engine(void)
{
    ENGINE *e = NULL;
    X509 *cert = NULL, *dupcert = NULL;
    X509_PUBKEY *pubkey, *duppubkey = NULL;
    int ret = 0;
    BIO *b = NULL;
    RSA_METHOD *rsameth = NULL;

    if (!TEST_ptr(b = BIO_new_file(test_get_argument(0), "r"))
        || !TEST_ptr(cert = PEM_read_bio_X509(b, NULL, NULL, NULL)))
        goto err;

    /* Dup without an engine */
    if (!TEST_ptr(dupcert = X509_dup(cert)))
        goto err;
    X509_free(dupcert);
    dupcert = NULL;

    if (!TEST_ptr(pubkey = X509_get_X509_PUBKEY(cert))
        || !TEST_ptr(duppubkey = X509_PUBKEY_dup(pubkey))
        || !TEST_ptr_ne(duppubkey, pubkey)
        || !TEST_ptr_ne(X509_PUBKEY_get0(duppubkey), X509_PUBKEY_get0(pubkey)))
        goto err;

    X509_PUBKEY_free(duppubkey);
    duppubkey = NULL;

    X509_free(cert);
    cert = NULL;

    /* Create a test ENGINE */
    if (!TEST_ptr(e = ENGINE_new())
            || !TEST_true(ENGINE_set_id(e, "Test dummy engine"))
            || !TEST_true(ENGINE_set_name(e, "Test dummy engine")))
        goto err;

    if (!TEST_ptr(rsameth = RSA_meth_dup(RSA_get_default_method())))
        goto err;

    ENGINE_set_RSA(e, rsameth);

    if (!TEST_true(ENGINE_set_default_RSA(e)))
        goto err;

    if (!TEST_int_ge(BIO_seek(b, 0), 0)
        || !TEST_ptr(cert = PEM_read_bio_X509(b, NULL, NULL, NULL)))
        goto err;

    /* Dup with an engine set on the key */
    if (!TEST_ptr(dupcert = X509_dup(cert)))
        goto err;

    if (!TEST_ptr(pubkey = X509_get_X509_PUBKEY(cert))
        || !TEST_ptr(duppubkey = X509_PUBKEY_dup(pubkey))
        || !TEST_ptr_ne(duppubkey, pubkey)
        || !TEST_ptr_ne(X509_PUBKEY_get0(duppubkey), X509_PUBKEY_get0(pubkey)))
        goto err;

    ret = 1;

 err:
    X509_free(cert);
    X509_free(dupcert);
    X509_PUBKEY_free(duppubkey);
    if (e != NULL) {
        ENGINE_unregister_RSA(e);
        ENGINE_free(e);
    }
    RSA_meth_free(rsameth);
    BIO_free(b);
    return ret;
}
#endif

int global_init(void)
{
    /*
     * If the config file gets loaded, the dynamic engine will be loaded,
     * and that interferes with our test above.
     */
    return OPENSSL_init_crypto(OPENSSL_INIT_NO_LOAD_CONFIG, NULL);
}

OPT_TEST_DECLARE_USAGE("certfile\n")

int setup_tests(void)
{
#ifdef OPENSSL_NO_ENGINE
    TEST_note("No ENGINE support");
#else
    int n;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    n = test_get_argument_count();
    if (n == 0)
        return 0;

    ADD_TEST(test_engines);
    ADD_TEST(test_redirect);
    ADD_TEST(test_x509_dup_w_engine);
#endif
    return 1;
}
