/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Ideally, CONF should offer standard parsing methods and cover them
 * in tests. But since we have no CONF tests, we use a custom test for now.
 */

#include <stdio.h>
#include <string.h>

#include "internal/nelem.h"
#include "helpers/ssl_test_ctx.h"
#include "testutil.h"
#include <openssl/e_os2.h>
#include <openssl/err.h>
#include <openssl/conf.h>
#include <openssl/ssl.h>

static CONF *conf = NULL;

typedef struct ssl_test_ctx_test_fixture {
    const char *test_case_name;
    const char *test_section;
    /* Expected parsed configuration. */
    SSL_TEST_CTX *expected_ctx;
} SSL_TEST_CTX_TEST_FIXTURE;


static int clientconf_eq(SSL_TEST_CLIENT_CONF *conf1,
                         SSL_TEST_CLIENT_CONF *conf2)
{
    if (!TEST_int_eq(conf1->verify_callback, conf2->verify_callback)
            || !TEST_int_eq(conf1->servername, conf2->servername)
            || !TEST_str_eq(conf1->npn_protocols, conf2->npn_protocols)
            || !TEST_str_eq(conf1->alpn_protocols, conf2->alpn_protocols)
            || !TEST_int_eq(conf1->ct_validation, conf2->ct_validation)
            || !TEST_int_eq(conf1->max_fragment_len_mode,
                            conf2->max_fragment_len_mode))
        return 0;
    return 1;
}

static int serverconf_eq(SSL_TEST_SERVER_CONF *serv,
                         SSL_TEST_SERVER_CONF *serv2)
{
    if (!TEST_int_eq(serv->servername_callback, serv2->servername_callback)
            || !TEST_str_eq(serv->npn_protocols, serv2->npn_protocols)
            || !TEST_str_eq(serv->alpn_protocols, serv2->alpn_protocols)
            || !TEST_int_eq(serv->broken_session_ticket,
                            serv2->broken_session_ticket)
            || !TEST_str_eq(serv->session_ticket_app_data,
                            serv2->session_ticket_app_data)
            || !TEST_int_eq(serv->cert_status, serv2->cert_status))
        return 0;
    return 1;
}

static int extraconf_eq(SSL_TEST_EXTRA_CONF *extra,
                        SSL_TEST_EXTRA_CONF *extra2)
{
    if (!TEST_true(clientconf_eq(&extra->client, &extra2->client))
            || !TEST_true(serverconf_eq(&extra->server, &extra2->server))
            || !TEST_true(serverconf_eq(&extra->server2, &extra2->server2)))
        return 0;
    return 1;
}

static int testctx_eq(SSL_TEST_CTX *ctx, SSL_TEST_CTX *ctx2)
{
    if (!TEST_int_eq(ctx->method, ctx2->method)
            || !TEST_int_eq(ctx->handshake_mode, ctx2->handshake_mode)
            || !TEST_int_eq(ctx->app_data_size, ctx2->app_data_size)
            || !TEST_int_eq(ctx->max_fragment_size, ctx2->max_fragment_size)
            || !extraconf_eq(&ctx->extra, &ctx2->extra)
            || !extraconf_eq(&ctx->resume_extra, &ctx2->resume_extra)
            || !TEST_int_eq(ctx->expected_result, ctx2->expected_result)
            || !TEST_int_eq(ctx->expected_client_alert,
                            ctx2->expected_client_alert)
            || !TEST_int_eq(ctx->expected_server_alert,
                            ctx2->expected_server_alert)
            || !TEST_int_eq(ctx->expected_protocol, ctx2->expected_protocol)
            || !TEST_int_eq(ctx->expected_servername, ctx2->expected_servername)
            || !TEST_int_eq(ctx->session_ticket_expected,
                            ctx2->session_ticket_expected)
            || !TEST_int_eq(ctx->compression_expected,
                            ctx2->compression_expected)
            || !TEST_str_eq(ctx->expected_npn_protocol,
                            ctx2->expected_npn_protocol)
            || !TEST_str_eq(ctx->expected_alpn_protocol,
                            ctx2->expected_alpn_protocol)
            || !TEST_str_eq(ctx->expected_cipher,
                            ctx2->expected_cipher)
            || !TEST_str_eq(ctx->expected_session_ticket_app_data,
                            ctx2->expected_session_ticket_app_data)
            || !TEST_int_eq(ctx->resumption_expected,
                            ctx2->resumption_expected)
            || !TEST_int_eq(ctx->session_id_expected,
                            ctx2->session_id_expected))
        return 0;
    return 1;
}

static SSL_TEST_CTX_TEST_FIXTURE *set_up(const char *const test_case_name)
{
    SSL_TEST_CTX_TEST_FIXTURE *fixture;

    if (!TEST_ptr(fixture = OPENSSL_zalloc(sizeof(*fixture))))
        return NULL;
    fixture->test_case_name = test_case_name;
    if (!TEST_ptr(fixture->expected_ctx = SSL_TEST_CTX_new(NULL))) {
        OPENSSL_free(fixture);
        return NULL;
    }
    return fixture;
}

static int execute_test(SSL_TEST_CTX_TEST_FIXTURE *fixture)
{
    int success = 0;
    SSL_TEST_CTX *ctx;

    if (!TEST_ptr(ctx = SSL_TEST_CTX_create(conf, fixture->test_section,
                                            fixture->expected_ctx->libctx))
            || !testctx_eq(ctx, fixture->expected_ctx))
        goto err;

    success = 1;
 err:
    SSL_TEST_CTX_free(ctx);
    return success;
}

static void tear_down(SSL_TEST_CTX_TEST_FIXTURE *fixture)
{
    SSL_TEST_CTX_free(fixture->expected_ctx);
    OPENSSL_free(fixture);
}

#define SETUP_SSL_TEST_CTX_TEST_FIXTURE() \
    SETUP_TEST_FIXTURE(SSL_TEST_CTX_TEST_FIXTURE, set_up);
#define EXECUTE_SSL_TEST_CTX_TEST() \
    EXECUTE_TEST(execute_test, tear_down)

static int test_empty_configuration(void)
{
    SETUP_SSL_TEST_CTX_TEST_FIXTURE();
    fixture->test_section = "ssltest_default";
    fixture->expected_ctx->expected_result = SSL_TEST_SUCCESS;
    EXECUTE_SSL_TEST_CTX_TEST();
    return result;
}

static int test_good_configuration(void)
{
    SETUP_SSL_TEST_CTX_TEST_FIXTURE();
    fixture->test_section = "ssltest_good";
    fixture->expected_ctx->method = SSL_TEST_METHOD_DTLS;
    fixture->expected_ctx->handshake_mode = SSL_TEST_HANDSHAKE_RESUME;
    fixture->expected_ctx->app_data_size = 1024;
    fixture->expected_ctx->max_fragment_size = 2048;

    fixture->expected_ctx->expected_result = SSL_TEST_SERVER_FAIL;
    fixture->expected_ctx->expected_client_alert = SSL_AD_UNKNOWN_CA;
    fixture->expected_ctx->expected_server_alert = 0;  /* No alert. */
    fixture->expected_ctx->expected_protocol = TLS1_1_VERSION;
    fixture->expected_ctx->expected_servername = SSL_TEST_SERVERNAME_SERVER2;
    fixture->expected_ctx->session_ticket_expected = SSL_TEST_SESSION_TICKET_YES;
    fixture->expected_ctx->compression_expected = SSL_TEST_COMPRESSION_NO;
    fixture->expected_ctx->session_id_expected = SSL_TEST_SESSION_ID_IGNORE;
    fixture->expected_ctx->resumption_expected = 1;

    fixture->expected_ctx->extra.client.verify_callback =
        SSL_TEST_VERIFY_REJECT_ALL;
    fixture->expected_ctx->extra.client.servername = SSL_TEST_SERVERNAME_SERVER2;
    fixture->expected_ctx->extra.client.npn_protocols =
        OPENSSL_strdup("foo,bar");
    if (!TEST_ptr(fixture->expected_ctx->extra.client.npn_protocols))
        goto err;
    fixture->expected_ctx->extra.client.max_fragment_len_mode = 0;

    fixture->expected_ctx->extra.server.servername_callback =
        SSL_TEST_SERVERNAME_IGNORE_MISMATCH;
    fixture->expected_ctx->extra.server.broken_session_ticket = 1;

    fixture->expected_ctx->resume_extra.server2.alpn_protocols =
        OPENSSL_strdup("baz");
    if (!TEST_ptr(fixture->expected_ctx->resume_extra.server2.alpn_protocols))
        goto err;

    fixture->expected_ctx->resume_extra.client.ct_validation =
        SSL_TEST_CT_VALIDATION_STRICT;

    EXECUTE_SSL_TEST_CTX_TEST();
    return result;

err:
    tear_down(fixture);
    return 0;
}

static const char *bad_configurations[] = {
    "ssltest_unknown_option",
    "ssltest_wrong_section",
    "ssltest_unknown_expected_result",
    "ssltest_unknown_alert",
    "ssltest_unknown_protocol",
    "ssltest_unknown_verify_callback",
    "ssltest_unknown_servername",
    "ssltest_unknown_servername_callback",
    "ssltest_unknown_session_ticket_expected",
    "ssltest_unknown_compression_expected",
    "ssltest_unknown_session_id_expected",
    "ssltest_unknown_method",
    "ssltest_unknown_handshake_mode",
    "ssltest_unknown_resumption_expected",
    "ssltest_unknown_ct_validation",
    "ssltest_invalid_max_fragment_len",
};

static int test_bad_configuration(int idx)
{
    SSL_TEST_CTX *ctx;

    if (!TEST_ptr_null(ctx = SSL_TEST_CTX_create(conf,
                                                 bad_configurations[idx], NULL))) {
        SSL_TEST_CTX_free(ctx);
        return 0;
    }

    return 1;
}

OPT_TEST_DECLARE_USAGE("conf_file\n")

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(conf = NCONF_new(NULL)))
        return 0;
    /* argument should point to test/ssl_test_ctx_test.cnf */
    if (!TEST_int_gt(NCONF_load(conf, test_get_argument(0), NULL), 0))
        return 0;

    ADD_TEST(test_empty_configuration);
    ADD_TEST(test_good_configuration);
    ADD_ALL_TESTS(test_bad_configuration, OSSL_NELEM(bad_configurations));
    return 1;
}

void cleanup_tests(void)
{
    NCONF_free(conf);
}
