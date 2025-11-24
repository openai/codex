/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/err.h>
#include <openssl/e_os2.h>
#include <openssl/ssl.h>
#include <openssl/ssl3.h>
#include <openssl/tls1.h>

#include "internal/nelem.h"
#include "testutil.h"

typedef struct cipherlist_test_fixture {
    const char *test_case_name;
    SSL_CTX *server;
    SSL_CTX *client;
} CIPHERLIST_TEST_FIXTURE;


static void tear_down(CIPHERLIST_TEST_FIXTURE *fixture)
{
    if (fixture != NULL) {
        SSL_CTX_free(fixture->server);
        SSL_CTX_free(fixture->client);
        fixture->server = fixture->client = NULL;
        OPENSSL_free(fixture);
    }
}

static CIPHERLIST_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    CIPHERLIST_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->server = SSL_CTX_new(TLS_server_method()))
            || !TEST_ptr(fixture->client = SSL_CTX_new(TLS_client_method()))) {
        tear_down(fixture);
        return NULL;
    }
    return fixture;
}

/*
 * All ciphers in the DEFAULT cipherlist meet the default security level.
 * However, default supported ciphers exclude SRP and PSK ciphersuites
 * for which no callbacks have been set up.
 *
 * Supported ciphers also exclude TLSv1.2 ciphers if TLSv1.2 is disabled,
 * and individual disabled algorithms. However, NO_RSA, NO_AES and NO_SHA
 * are currently broken and should be considered mission impossible in libssl.
 */
static const uint32_t default_ciphers_in_order[] = {
#ifndef OPENSSL_NO_TLS1_3
    TLS1_3_CK_AES_256_GCM_SHA384,
# if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
    TLS1_3_CK_CHACHA20_POLY1305_SHA256,
# endif
    TLS1_3_CK_AES_128_GCM_SHA256,
#endif
#ifndef OPENSSL_NO_TLS1_2
# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
    TLS1_CK_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
# endif
# ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_256_GCM_SHA384,
# endif

# if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
#  ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_CHACHA20_POLY1305,
    TLS1_CK_ECDHE_RSA_WITH_CHACHA20_POLY1305,
#  endif
#  ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_CHACHA20_POLY1305,
#  endif
# endif  /* !OPENSSL_NO_CHACHA && !OPENSSL_NO_POLY1305 */

# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
    TLS1_CK_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
# endif
# ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_128_GCM_SHA256,
# endif
# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_256_SHA384,
    TLS1_CK_ECDHE_RSA_WITH_AES_256_SHA384,
# endif
# ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_256_SHA256,
# endif
# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_128_SHA256,
    TLS1_CK_ECDHE_RSA_WITH_AES_128_SHA256,
# endif
# ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_128_SHA256,
# endif
#endif  /* !OPENSSL_NO_TLS1_2 */

#if !defined(OPENSSL_NO_TLS1_2) || defined(OPENSSL_NO_TLS1_3)
    /* These won't be usable if TLSv1.3 is available but TLSv1.2 isn't */
# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_256_CBC_SHA,
    TLS1_CK_ECDHE_RSA_WITH_AES_256_CBC_SHA,
# endif
 #ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_256_SHA,
# endif
# ifndef OPENSSL_NO_EC
    TLS1_CK_ECDHE_ECDSA_WITH_AES_128_CBC_SHA,
    TLS1_CK_ECDHE_RSA_WITH_AES_128_CBC_SHA,
# endif
# ifndef OPENSSL_NO_DH
    TLS1_CK_DHE_RSA_WITH_AES_128_SHA,
# endif
#endif /* !defined(OPENSSL_NO_TLS1_2) || defined(OPENSSL_NO_TLS1_3) */

#ifndef OPENSSL_NO_TLS1_2
    TLS1_CK_RSA_WITH_AES_256_GCM_SHA384,
    TLS1_CK_RSA_WITH_AES_128_GCM_SHA256,
#endif
#ifndef OPENSSL_NO_TLS1_2
    TLS1_CK_RSA_WITH_AES_256_SHA256,
    TLS1_CK_RSA_WITH_AES_128_SHA256,
#endif
#if !defined(OPENSSL_NO_TLS1_2) || defined(OPENSSL_NO_TLS1_3)
    /* These won't be usable if TLSv1.3 is available but TLSv1.2 isn't */
    TLS1_CK_RSA_WITH_AES_256_SHA,
    TLS1_CK_RSA_WITH_AES_128_SHA,
#endif
};

static int test_default_cipherlist(SSL_CTX *ctx)
{
    STACK_OF(SSL_CIPHER) *ciphers = NULL;
    SSL *ssl = NULL;
    int i, ret = 0, num_expected_ciphers, num_ciphers;
    uint32_t expected_cipher_id, cipher_id;

    if (ctx == NULL)
        return 0;

    if (!TEST_ptr(ssl = SSL_new(ctx))
            || !TEST_ptr(ciphers = SSL_get1_supported_ciphers(ssl)))
        goto err;

    num_expected_ciphers = OSSL_NELEM(default_ciphers_in_order);
    num_ciphers = sk_SSL_CIPHER_num(ciphers);
    if (!TEST_int_eq(num_ciphers, num_expected_ciphers))
        goto err;

    for (i = 0; i < num_ciphers; i++) {
        expected_cipher_id = default_ciphers_in_order[i];
        cipher_id = SSL_CIPHER_get_id(sk_SSL_CIPHER_value(ciphers, i));
        if (!TEST_int_eq(cipher_id, expected_cipher_id)) {
            TEST_info("Wrong cipher at position %d", i);
            goto err;
        }
    }

    ret = 1;

 err:
    sk_SSL_CIPHER_free(ciphers);
    SSL_free(ssl);
    return ret;
}

static int execute_test(CIPHERLIST_TEST_FIXTURE *fixture)
{
    return fixture != NULL
        && test_default_cipherlist(fixture->server)
        && test_default_cipherlist(fixture->client);
}

#define SETUP_CIPHERLIST_TEST_FIXTURE() \
    SETUP_TEST_FIXTURE(CIPHERLIST_TEST_FIXTURE, set_up)

#define EXECUTE_CIPHERLIST_TEST() \
    EXECUTE_TEST(execute_test, tear_down)

static int test_default_cipherlist_implicit(void)
{
    SETUP_CIPHERLIST_TEST_FIXTURE();
    EXECUTE_CIPHERLIST_TEST();
    return result;
}

static int test_default_cipherlist_explicit(void)
{
    SETUP_CIPHERLIST_TEST_FIXTURE();
    if (!TEST_true(SSL_CTX_set_cipher_list(fixture->server, "DEFAULT"))
            || !TEST_true(SSL_CTX_set_cipher_list(fixture->client, "DEFAULT"))) {
        tear_down(fixture);
        fixture = NULL;
    }
    EXECUTE_CIPHERLIST_TEST();
    return result;
}

/* SSL_CTX_set_cipher_list() should fail if it clears all TLSv1.2 ciphers. */
static int test_default_cipherlist_clear(void)
{
    SSL *s = NULL;
    SETUP_CIPHERLIST_TEST_FIXTURE();

    if (!TEST_int_eq(SSL_CTX_set_cipher_list(fixture->server, "no-such"), 0))
        goto end;

    if (!TEST_int_eq(ERR_GET_REASON(ERR_get_error()), SSL_R_NO_CIPHER_MATCH))
        goto end;

    s = SSL_new(fixture->client);

    if (!TEST_ptr(s))
      goto end;

    if (!TEST_int_eq(SSL_set_cipher_list(s, "no-such"), 0))
        goto end;

    if (!TEST_int_eq(ERR_GET_REASON(ERR_get_error()),
                SSL_R_NO_CIPHER_MATCH))
        goto end;

    result = 1;
end:
    SSL_free(s);
    tear_down(fixture);
    return result;
}

/* SSL_CTX_set_cipher_list matching with cipher standard name */
static int test_stdname_cipherlist(void)
{
    SETUP_CIPHERLIST_TEST_FIXTURE();
    if (!TEST_true(SSL_CTX_set_cipher_list(fixture->server, TLS1_RFC_RSA_WITH_AES_128_SHA))
            || !TEST_true(SSL_CTX_set_cipher_list(fixture->client, TLS1_RFC_RSA_WITH_AES_128_SHA))) {
        goto end;
    }
    result = 1;
end:
    tear_down(fixture);
    fixture = NULL;
    return result;
}

int setup_tests(void)
{
    ADD_TEST(test_default_cipherlist_implicit);
    ADD_TEST(test_default_cipherlist_explicit);
    ADD_TEST(test_default_cipherlist_clear);
    ADD_TEST(test_stdname_cipherlist);
    return 1;
}
