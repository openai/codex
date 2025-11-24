/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "testutil.h"
#include "helpers/ssltestlib.h"
#include <openssl/objects.h>

#define TEST_true_or_end(a) if (!TEST_true(a)) \
        goto end;

#define TEST_false_or_end(a) if (!TEST_false(a)) \
        goto end;

#define SERVER_PREFERENCE 1
#define CLIENT_PREFERENCE 0

#define WORK_ON_SSL_OBJECT 1
#define WORK_ON_CONTEXT 0

#define SYNTAX_FAILURE "SYNTAX_FAILURE"
#define NEGOTIATION_FAILURE "NEGOTIATION_FAILURE"

typedef enum TEST_TYPE {
    TEST_NEGOTIATION_FAILURE = 0,
    TEST_NEGOTIATION_SUCCESS = 1,
    TEST_SYNTAX_FAILURE = 2
} TEST_TYPE;

typedef enum SERVER_RESPONSE {
    HRR  = 0,
    INIT = 1,
    SH   = 2
} SERVER_RESPONSE;

static char *cert = NULL;
static char *privkey = NULL;

struct tls13groupselection_test_st {
    const char *client_groups;
    const char *server_groups;
    const int preference;
    const char *expected_group;
    const enum SERVER_RESPONSE expected_server_response;
};

static const struct tls13groupselection_test_st tls13groupselection_tests[] =
    {

        /*
         * (A) Test with no explicit key share (backward compatibility)
         * Key share is implicitly sent for first client group
         * Test (implicitly) that the key share group is used
         */
        { "secp384r1:secp521r1:X25519:prime256v1:X448", /* test 0 */
          "X25519:secp521r1:secp384r1:prime256v1:X448",
          CLIENT_PREFERENCE,
          "secp384r1", SH
        },
        { "secp521r1:secp384r1:X25519:prime256v1:X448", /* test 1 */
          "X25519:secp521r1:secp384r1:prime256v1:X448",
          SERVER_PREFERENCE,
          "secp521r1", SH
        },

        /*
         * (B) No explicit key share test (backward compatibility)
         * Key share is implicitly sent for first client group
         * Check HRR if server does not support key share group
         */
        { "secp521r1:secp384r1:X25519:prime256v1:X448", /* test 2 */
          "X25519:secp384r1:prime256v1",
          CLIENT_PREFERENCE,
          "secp384r1", HRR
        },
        { "secp521r1:secp384r1:X25519:prime256v1:X448", /* test 3 */
          "X25519:secp384r1:prime256v1",
          SERVER_PREFERENCE,
          "x25519", HRR
        },

        /*
         * (C) Explicit key shares, SH tests
         * Test key share selection as function of client-/server-preference
         * Test (implicitly) that multiple key shares are generated
         * Test (implicitly) that multiple tuples don't influence the client
         * Test (implicitly) that key share prefix doesn't influence the server
         */
        { "secp521r1:secp384r1:*X25519/*prime256v1:X448", /* test 4 */
          "secp521r1:*prime256v1:X25519:X448",
          CLIENT_PREFERENCE,
          "x25519", SH
        },
        { "secp521r1:secp384r1:*X25519/*prime256v1:X448", /* test 5 */
          "secp521r1:*prime256v1:X25519:X448",
          SERVER_PREFERENCE,
          "secp256r1", SH
        },

        /*
         * (D) Explicit key shares, HRR tests
         * Check that HRR is issued if group in first tuple
         * is supported but no key share is available for the tuple
         */
        { "secp521r1:secp384r1:*X25519:prime256v1:*X448", /* test 6 */
          "secp384r1:secp521r1:prime256v1/X25519:X448",
          CLIENT_PREFERENCE,
          "secp521r1", HRR
        },
        { "secp521r1:secp384r1:*X25519:prime256v1:*X448", /* test 7 */
          "secp384r1:secp521r1:prime256v1/X25519:X448",
          SERVER_PREFERENCE,
          "secp384r1", HRR
        },

        /*
         * (E) Multiple tuples tests, client without tuple delimiters
         * Check that second tuple is evaluated if there isn't any match
         * first tuple
         */
        { "*X25519:prime256v1:*X448", /* test 8 */
          "secp521r1:secp384r1/X448:X25519",
          CLIENT_PREFERENCE,
          "x25519", SH
        },
        { "*X25519:prime256v1:*X448", /* test 9 */
          "secp521r1:secp384r1/X448:X25519",
          SERVER_PREFERENCE,
          "x448", SH
        },

        /* (F) Check that '?' will ignore unknown group but use known group */
        { "*X25519:?unknown_group_123:prime256v1:*X448", /* test 10 */
          "secp521r1:secp384r1/X448:?unknown_group_456:?X25519",
          CLIENT_PREFERENCE,
          "x25519", SH
        },
        { "*X25519:prime256v1:*X448:?*unknown_group_789", /* test 11 */
          "secp521r1:secp384r1/?X448:?unknown_group_456:X25519",
          SERVER_PREFERENCE,
          "x448", SH
        },

        /*
         * (G) Check full backward compatibility (= don't explicitly set any groups)
         */
        { NULL, /* test 12 */
          NULL,
          CLIENT_PREFERENCE,
#ifndef OPENSSL_NO_ML_KEM
          "X25519MLKEM768", SH
#else
          "x25519", SH
#endif
        },
        { NULL, /* test 13 */
          NULL,
          SERVER_PREFERENCE,
#ifndef OPENSSL_NO_ML_KEM
          "X25519MLKEM768", SH
#else
          "x25519", SH
#endif
        },

        /*
         * (H) Check that removal of group is 'active'
         */
        { "*X25519:*X448", /* test 14 */
          "secp521r1:X25519:prime256v1:-X25519:secp384r1/X448",
          CLIENT_PREFERENCE,
          "x448", SH
        },
        { "*X25519:*X448", /* test 15 */
          "secp521r1:X25519:prime256v1:-X25519:secp384r1/X448",
          SERVER_PREFERENCE,
          "x448", SH
        },
        { "*X25519:prime256v1:*X448", /* test 16 */
          "X25519:prime256v1/X448:-X25519",
          CLIENT_PREFERENCE,
          "secp256r1", HRR
        },
        { "*X25519:prime256v1:*X448", /* test 17 */
          "X25519:prime256v1/X448:-X25519",
          SERVER_PREFERENCE,
          "secp256r1", HRR
        },
        /*
         * (I) Check handling of the "DEFAULT" 'pseudo group name'
         */
        { "*X25519:DEFAULT:-prime256v1:-X448", /* test 18 */
          "DEFAULT:-X25519:-?X25519MLKEM768",
          CLIENT_PREFERENCE,
          "secp384r1", HRR
        },
        { "*X25519:DEFAULT:-prime256v1:-X448", /* test 19 */
          "DEFAULT:-X25519:-?X25519MLKEM768",
          SERVER_PREFERENCE,
          "secp384r1", HRR
        },
        /*
         * (J) Deduplication check
         */
        { "secp521r1:X25519:prime256v1/X25519:prime256v1/X448", /* test 20 */
          "secp521r1:X25519:prime256v1/X25519:prime256v1/X448",
          CLIENT_PREFERENCE,
          "secp521r1", SH
        },
        { "secp521r1:X25519:prime256v1/X25519:prime256v1/X448", /* test 21 */
          "secp521r1:X25519:prime256v1/X25519:prime256v1/X448",
          SERVER_PREFERENCE,
          "secp521r1", SH
        },
        /*
         * (K) Check group removal when first entry requested a keyshare
         */
        { "*X25519:*prime256v1:-X25519", /* test 22 */
          "X25519:prime256v1",
          CLIENT_PREFERENCE,
          "secp256r1", SH
        },
        /*
         * (L) Syntax errors
         */
        { "*X25519:*prime256v1:NOTVALID", /* test 23 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "X25519//prime256v1", /* test 24 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "**X25519:*prime256v1", /* test 25 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "*X25519:*secp256r1:*X448:*secp521r1:*secp384r1", /* test 26 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "*X25519:*secp256r1:?:*secp521r1", /* test 27 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "*X25519:*secp256r1::secp521r1", /* test 28 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { ":*secp256r1:secp521r1", /* test 29 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "*secp256r1:secp521r1:", /* test 30 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "/secp256r1/secp521r1", /* test 31 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "secp256r1/secp521r1/", /* test 32 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "X25519:??secp256r1:X448", /* test 33 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "X25519:secp256r1:**X448", /* test 34 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "--X25519:secp256r1:X448", /* test 35 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "-DEFAULT",  /* test 36 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        { "?DEFAULT",  /* test 37 */
          "",
          CLIENT_PREFERENCE,
          SYNTAX_FAILURE
        },
        /*
         * Negotiation Failures
         * No overlapping groups between client and server
         */
        /* test 38 remove all groups */
        { "X25519:secp256r1:X448:secp521r1:-X448:-secp256r1:-X25519:-secp521r1",
          "",
          CLIENT_PREFERENCE,
          NEGOTIATION_FAILURE
        },
        { "secp384r1:secp521r1:X25519", /* test 39 */
          "prime256v1:X448",
          CLIENT_PREFERENCE,
          NEGOTIATION_FAILURE
        },
        { "secp521r1:secp384r1:X25519", /* test 40 */
          "prime256v1:X448",
          SERVER_PREFERENCE,
          NEGOTIATION_FAILURE
        },
        /*
         * These are allowed
         * "X25519/prime256v1:-X448", "X25519:-*X25519:*prime256v1, "*DEFAULT"
         */
        /*
         * Tests to show that spaces between tuples are allowed
         */
        { "secp521r1:X25519 / prime256v1/X25519 / prime256v1/X448", /* test 41 */
          "secp521r1:X25519 / prime256v1/X25519 / prime256v1/X448",
          CLIENT_PREFERENCE,
          "secp521r1", SH
        },
        { "secp521r1 / prime256v1:X25519 / prime256v1/X448", /* test 42 */
          "secp521r1 / prime256v1:X25519 / prime256v1/X448",
          SERVER_PREFERENCE,
          "secp521r1", SH
        },
    };

static void server_response_check_cb(int write_p, int version,
                                     int content_type, const void *buf,
                                     size_t len, SSL *ssl, void *arg)
{
    /* Cast arg to SERVER_RESPONSE */
    enum SERVER_RESPONSE *server_response = (enum SERVER_RESPONSE *)arg;
    /* Prepare check for HRR */
    const uint8_t *incoming_random = (uint8_t *)buf + 6;
    const uint8_t magic_HRR_random[32] = { 0xCF, 0x21, 0xAD, 0x74, 0xE5, 0x9A, 0x61, 0x11,
                                           0xBE, 0x1D, 0x8C, 0x02, 0x1E, 0x65, 0xB8, 0x91,
                                           0xC2, 0xA2, 0x11, 0x16, 0x7A, 0xBB, 0x8C, 0x5E,
                                           0x07, 0x9E, 0x09, 0xE2, 0xC8, 0xA8, 0x33, 0x9C };

    /* Did a server hello arrive? */
    if (write_p == 0 &&                                /* Incoming data... */
        content_type == SSL3_RT_HANDSHAKE &&           /* carrying a handshake record type ... */
        version == TLS1_3_VERSION &&                   /* for TLSv1.3 ... */
        ((uint8_t *)buf)[0] == SSL3_MT_SERVER_HELLO) {  /* with message type "ServerHello" */
        /* Check what it is: SH or HRR (compare the 'random' data field with HRR magic number) */
        if (memcmp((void *)incoming_random, (void *)magic_HRR_random, 32) == 0)
            *server_response *= HRR;
        else
            *server_response *= SH;
    }
}

static int test_invalidsyntax(const struct tls13groupselection_test_st *current_test_vector,
                              int ssl_or_ctx)
{
    int ok = 0;
    SSL_CTX *client_ctx = NULL, *server_ctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;

    if (!TEST_ptr(current_test_vector->client_groups)
        || !TEST_size_t_ne(strlen(current_test_vector->client_groups), 0))
        goto end;

    /* Creation of the contexts */
    TEST_true_or_end(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                         TLS_client_method(),
                                         TLS1_VERSION, 0,
                                         &server_ctx, &client_ctx,
                                         cert, privkey));

    /* Customization of the contexts */
    if (ssl_or_ctx == WORK_ON_CONTEXT)
        TEST_false_or_end(SSL_CTX_set1_groups_list(client_ctx,
                                                   current_test_vector->client_groups));
    /* Creation of the SSL objects */
    TEST_true_or_end(create_ssl_objects(server_ctx, client_ctx,
                                        &serverssl, &clientssl,
                                        NULL, NULL));

    /* Customization of the SSL objects */
    if (ssl_or_ctx == WORK_ON_SSL_OBJECT)
        TEST_false_or_end(SSL_set1_groups_list(clientssl, current_test_vector->client_groups));

    ok = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(server_ctx);
    SSL_CTX_free(client_ctx);
    return ok;

}

static int test_groupnegotiation(const struct tls13groupselection_test_st *current_test_vector,
                                 int ssl_or_ctx, TEST_TYPE test_type)
{
    int ok = 0;
    int negotiated_group_client = 0;
    int negotiated_group_server = 0;
    const char *group_name_client;
    SSL_CTX *client_ctx = NULL, *server_ctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    enum SERVER_RESPONSE server_response;

    /* Creation of the contexts */
    TEST_true_or_end(create_ssl_ctx_pair(NULL, TLS_server_method(),
                                         TLS_client_method(),
                                         TLS1_VERSION, 0,
                                         &server_ctx, &client_ctx,
                                         cert, privkey));

    /* Customization of the contexts */
    if (ssl_or_ctx == WORK_ON_CONTEXT) {
        if (current_test_vector->client_groups != NULL) {
            TEST_true_or_end(SSL_CTX_set1_groups_list(client_ctx,
                                                      current_test_vector->client_groups));
        }
        if (current_test_vector->server_groups != NULL) {
            TEST_true_or_end(SSL_CTX_set1_groups_list(server_ctx,
                                                      current_test_vector->server_groups));
        }
        TEST_true_or_end(SSL_CTX_set_min_proto_version(client_ctx, TLS1_3_VERSION));
        TEST_true_or_end(SSL_CTX_set_min_proto_version(server_ctx, TLS1_3_VERSION));
        if (current_test_vector->preference == SERVER_PREFERENCE)
            SSL_CTX_set_options(server_ctx, SSL_OP_CIPHER_SERVER_PREFERENCE);
    }
    /* Creation of the SSL objects */
    if (!TEST_true(create_ssl_objects(server_ctx, client_ctx,
                                      &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    /* Customization of the SSL objects */
    if (ssl_or_ctx == WORK_ON_SSL_OBJECT) {
        if (current_test_vector->client_groups != NULL)
            TEST_true_or_end(SSL_set1_groups_list(clientssl, current_test_vector->client_groups));

        if (current_test_vector->server_groups != NULL)
            TEST_true_or_end(SSL_set1_groups_list(serverssl, current_test_vector->server_groups));

        TEST_true_or_end(SSL_set_min_proto_version(clientssl, TLS1_3_VERSION));
        TEST_true_or_end(SSL_set_min_proto_version(serverssl, TLS1_3_VERSION));

        if (current_test_vector->preference == SERVER_PREFERENCE)
            SSL_set_options(serverssl, SSL_OP_CIPHER_SERVER_PREFERENCE);
    }

    /* We set the message callback on the client side (which checks SH/HRR) */
    server_response = INIT; /* Variable to hold server response info */
    SSL_set_msg_callback_arg(clientssl, &server_response); /* add it to the callback */
    SSL_set_msg_callback(clientssl, server_response_check_cb); /* and activate callback */

    /* Creating a test connection */
    if (test_type == TEST_NEGOTIATION_SUCCESS) {
        TEST_true_or_end(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE));

        /*
         * Checking that the negotiated group matches our expectation
         * and must be identical on server and client
         * and must be expected SH or HRR
         */
        negotiated_group_client = SSL_get_negotiated_group(clientssl);
        negotiated_group_server = SSL_get_negotiated_group(serverssl);
        group_name_client = SSL_group_to_name(clientssl, negotiated_group_client);
        if (!TEST_int_eq(negotiated_group_client, negotiated_group_server))
            goto end;
        if (!TEST_int_eq((int)current_test_vector->expected_server_response, (int)server_response))
            goto end;
        if (TEST_str_eq(group_name_client, current_test_vector->expected_group))
            ok = 1;
    } else {
        TEST_false_or_end(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE));
        ok = 1;
    }

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(server_ctx);
    SSL_CTX_free(client_ctx);
    return ok;
}

static int tls13groupselection_test(int i)
{
    int testresult = 1; /* Assume the test will succeed */
    int res = 0;
    TEST_TYPE test_type = TEST_NEGOTIATION_SUCCESS;

    /*
     * Call the code under test, once such that the ssl object is used and
     * once such that the ctx is used. If any of the tests fail (= return 0),
     * the end result will be 0 thanks to multiplication
     */
    TEST_info("==> Running TLSv1.3 test %d", i);

    if (strncmp(tls13groupselection_tests[i].expected_group,
                SYNTAX_FAILURE, sizeof(SYNTAX_FAILURE)) == 0)
        test_type = TEST_SYNTAX_FAILURE;
    else if (strncmp(tls13groupselection_tests[i].expected_group,
                     NEGOTIATION_FAILURE, sizeof(NEGOTIATION_FAILURE)) == 0)
        test_type = TEST_NEGOTIATION_FAILURE;

    if (test_type == TEST_SYNTAX_FAILURE)
        res = test_invalidsyntax(&tls13groupselection_tests[i],
                                 WORK_ON_SSL_OBJECT);
    else
        res = test_groupnegotiation(&tls13groupselection_tests[i],
                                    WORK_ON_SSL_OBJECT, test_type);

    if (!res)
        TEST_error("====> [ERROR] TLSv1.3 test %d with WORK_ON_SSL_OBJECT failed", i);
    testresult *= res;

    if (test_type == TEST_SYNTAX_FAILURE)
        res = test_invalidsyntax(&tls13groupselection_tests[i],
                                 WORK_ON_CONTEXT);
    else
        res = test_groupnegotiation(&tls13groupselection_tests[i],
                                    WORK_ON_CONTEXT, test_type);

    if (!res)
        TEST_error("====> [ERROR] TLSv1.3 test %d with WORK_ON_CONTEXT failed", i);
    testresult *= res;

    return testresult;
}

int setup_tests(void)
{
    if (!TEST_ptr(cert = test_get_argument(0))
        || !TEST_ptr(privkey = test_get_argument(1)))
        return 0;

    ADD_ALL_TESTS(tls13groupselection_test, OSSL_NELEM(tls13groupselection_tests));
    return 1;
}
