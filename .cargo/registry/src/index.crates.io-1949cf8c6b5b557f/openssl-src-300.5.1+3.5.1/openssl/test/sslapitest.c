/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * We need access to the deprecated low level HMAC APIs for legacy purposes
 * when the deprecated calls are not hidden
 */
#ifndef OPENSSL_NO_DEPRECATED_3_0
# define OPENSSL_SUPPRESS_DEPRECATED
#endif

#include <stdio.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/ssl.h>
#include <openssl/ocsp.h>
#include <openssl/srp.h>
#include <openssl/txt_db.h>
#include <openssl/aes.h>
#include <openssl/rand.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/provider.h>
#include <openssl/param_build.h>
#include <openssl/x509v3.h>
#include <openssl/dh.h>
#include <openssl/engine.h>

#include "helpers/ssltestlib.h"
#include "testutil.h"
#include "testutil/output.h"
#include "internal/nelem.h"
#include "internal/tlsgroups.h"
#include "internal/ktls.h"
#include "internal/ssl_unwrap.h"
#include "../ssl/ssl_local.h"
#include "../ssl/record/methods/recmethod_local.h"
#include "filterprov.h"

#undef OSSL_NO_USABLE_TLS1_3
#if defined(OPENSSL_NO_TLS1_3) \
    || (defined(OPENSSL_NO_EC) && defined(OPENSSL_NO_DH))
/*
 * If we don't have ec or dh then there are no built-in groups that are usable
 * with TLSv1.3
 */
# define OSSL_NO_USABLE_TLS1_3
#endif

/* Defined in tls-provider.c */
int tls_provider_init(const OSSL_CORE_HANDLE *handle,
                      const OSSL_DISPATCH *in,
                      const OSSL_DISPATCH **out,
                      void **provctx);

static OSSL_LIB_CTX *libctx = NULL;
static OSSL_PROVIDER *defctxnull = NULL;

#ifndef OSSL_NO_USABLE_TLS1_3

static SSL_SESSION *clientpsk = NULL;
static SSL_SESSION *serverpsk = NULL;
static const char *pskid = "Identity";
static const char *srvid;

static int use_session_cb(SSL *ssl, const EVP_MD *md, const unsigned char **id,
                          size_t *idlen, SSL_SESSION **sess);
static int find_session_cb(SSL *ssl, const unsigned char *identity,
                           size_t identity_len, SSL_SESSION **sess);

static int use_session_cb_cnt = 0;
static int find_session_cb_cnt = 0;
static int end_of_early_data = 0;
#endif

static char *certsdir = NULL;
static char *cert = NULL;
static char *privkey = NULL;
static char *cert2 = NULL;
static char *privkey2 = NULL;
static char *cert1024 = NULL;
static char *privkey1024 = NULL;
static char *cert3072 = NULL;
static char *privkey3072 = NULL;
static char *cert4096 = NULL;
static char *privkey4096 = NULL;
static char *cert8192 = NULL;
static char *privkey8192 = NULL;
static char *srpvfile = NULL;
static char *tmpfilename = NULL;
static char *dhfile = NULL;

static int is_fips = 0;
static int fips_ems_check = 0;

#define LOG_BUFFER_SIZE 2048
static char server_log_buffer[LOG_BUFFER_SIZE + 1] = {0};
static size_t server_log_buffer_index = 0;
static char client_log_buffer[LOG_BUFFER_SIZE + 1] = {0};
static size_t client_log_buffer_index = 0;
static int error_writing_log = 0;

#ifndef OPENSSL_NO_OCSP
static const unsigned char orespder[] = "Dummy OCSP Response";
static int ocsp_server_called = 0;
static int ocsp_client_called = 0;

static int cdummyarg = 1;
static X509 *ocspcert = NULL;
#endif

#define CLIENT_VERSION_LEN      2

/*
 * This structure is used to validate that the correct number of log messages
 * of various types are emitted when emitting secret logs.
 */
struct sslapitest_log_counts {
    unsigned int rsa_key_exchange_count;
    unsigned int master_secret_count;
    unsigned int client_early_secret_count;
    unsigned int client_handshake_secret_count;
    unsigned int server_handshake_secret_count;
    unsigned int client_application_secret_count;
    unsigned int server_application_secret_count;
    unsigned int early_exporter_secret_count;
    unsigned int exporter_secret_count;
};


static int hostname_cb(SSL *s, int *al, void *arg)
{
    const char *hostname = SSL_get_servername(s, TLSEXT_NAMETYPE_host_name);

    if (hostname != NULL && (strcmp(hostname, "goodhost") == 0
                             || strcmp(hostname, "altgoodhost") == 0))
        return  SSL_TLSEXT_ERR_OK;

    return SSL_TLSEXT_ERR_NOACK;
}

static void client_keylog_callback(const SSL *ssl, const char *line)
{
    int line_length = strlen(line);

    /* If the log doesn't fit, error out. */
    if (client_log_buffer_index + line_length > sizeof(client_log_buffer) - 1) {
        TEST_info("Client log too full");
        error_writing_log = 1;
        return;
    }

    strcat(client_log_buffer, line);
    client_log_buffer_index += line_length;
    client_log_buffer[client_log_buffer_index++] = '\n';
}

static void server_keylog_callback(const SSL *ssl, const char *line)
{
    int line_length = strlen(line);

    /* If the log doesn't fit, error out. */
    if (server_log_buffer_index + line_length > sizeof(server_log_buffer) - 1) {
        TEST_info("Server log too full");
        error_writing_log = 1;
        return;
    }

    strcat(server_log_buffer, line);
    server_log_buffer_index += line_length;
    server_log_buffer[server_log_buffer_index++] = '\n';
}

static int compare_hex_encoded_buffer(const char *hex_encoded,
                                      size_t hex_length,
                                      const uint8_t *raw,
                                      size_t raw_length)
{
    size_t i, j;
    char hexed[3];

    if (!TEST_size_t_eq(raw_length * 2, hex_length))
        return 1;

    for (i = j = 0; i < raw_length && j + 1 < hex_length; i++, j += 2) {
        BIO_snprintf(hexed, sizeof(hexed), "%02x", raw[i]);
        if (!TEST_int_eq(hexed[0], hex_encoded[j])
                || !TEST_int_eq(hexed[1], hex_encoded[j + 1]))
            return 1;
    }

    return 0;
}

static int test_keylog_output(char *buffer, const SSL *ssl,
                              const SSL_SESSION *session,
                              struct sslapitest_log_counts *expected)
{
    char *token = NULL;
    unsigned char actual_client_random[SSL3_RANDOM_SIZE] = {0};
    size_t client_random_size = SSL3_RANDOM_SIZE;
    unsigned char actual_master_key[SSL_MAX_MASTER_KEY_LENGTH] = {0};
    size_t master_key_size = SSL_MAX_MASTER_KEY_LENGTH;
    unsigned int rsa_key_exchange_count = 0;
    unsigned int master_secret_count = 0;
    unsigned int client_early_secret_count = 0;
    unsigned int client_handshake_secret_count = 0;
    unsigned int server_handshake_secret_count = 0;
    unsigned int client_application_secret_count = 0;
    unsigned int server_application_secret_count = 0;
    unsigned int early_exporter_secret_count = 0;
    unsigned int exporter_secret_count = 0;

    for (token = strtok(buffer, " \n"); token != NULL;
         token = strtok(NULL, " \n")) {
        if (strcmp(token, "RSA") == 0) {
            /*
             * Premaster secret. Tokens should be: 16 ASCII bytes of
             * hex-encoded encrypted secret, then the hex-encoded pre-master
             * secret.
             */
            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
            if (!TEST_size_t_eq(strlen(token), 16))
                return 0;
            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
            /*
             * We can't sensibly check the log because the premaster secret is
             * transient, and OpenSSL doesn't keep hold of it once the master
             * secret is generated.
             */
            rsa_key_exchange_count++;
        } else if (strcmp(token, "CLIENT_RANDOM") == 0) {
            /*
             * Master secret. Tokens should be: 64 ASCII bytes of hex-encoded
             * client random, then the hex-encoded master secret.
             */
            client_random_size = SSL_get_client_random(ssl,
                                                       actual_client_random,
                                                       SSL3_RANDOM_SIZE);
            if (!TEST_size_t_eq(client_random_size, SSL3_RANDOM_SIZE))
                return 0;

            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
            if (!TEST_size_t_eq(strlen(token), 64))
                return 0;
            if (!TEST_false(compare_hex_encoded_buffer(token, 64,
                                                       actual_client_random,
                                                       client_random_size)))
                return 0;

            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
            master_key_size = SSL_SESSION_get_master_key(session,
                                                         actual_master_key,
                                                         master_key_size);
            if (!TEST_size_t_ne(master_key_size, 0))
                return 0;
            if (!TEST_false(compare_hex_encoded_buffer(token, strlen(token),
                                                       actual_master_key,
                                                       master_key_size)))
                return 0;
            master_secret_count++;
        } else if (strcmp(token, "CLIENT_EARLY_TRAFFIC_SECRET") == 0
                    || strcmp(token, "CLIENT_HANDSHAKE_TRAFFIC_SECRET") == 0
                    || strcmp(token, "SERVER_HANDSHAKE_TRAFFIC_SECRET") == 0
                    || strcmp(token, "CLIENT_TRAFFIC_SECRET_0") == 0
                    || strcmp(token, "SERVER_TRAFFIC_SECRET_0") == 0
                    || strcmp(token, "EARLY_EXPORTER_SECRET") == 0
                    || strcmp(token, "EXPORTER_SECRET") == 0) {
            /*
             * TLSv1.3 secret. Tokens should be: 64 ASCII bytes of hex-encoded
             * client random, and then the hex-encoded secret. In this case,
             * we treat all of these secrets identically and then just
             * distinguish between them when counting what we saw.
             */
            if (strcmp(token, "CLIENT_EARLY_TRAFFIC_SECRET") == 0)
                client_early_secret_count++;
            else if (strcmp(token, "CLIENT_HANDSHAKE_TRAFFIC_SECRET") == 0)
                client_handshake_secret_count++;
            else if (strcmp(token, "SERVER_HANDSHAKE_TRAFFIC_SECRET") == 0)
                server_handshake_secret_count++;
            else if (strcmp(token, "CLIENT_TRAFFIC_SECRET_0") == 0)
                client_application_secret_count++;
            else if (strcmp(token, "SERVER_TRAFFIC_SECRET_0") == 0)
                server_application_secret_count++;
            else if (strcmp(token, "EARLY_EXPORTER_SECRET") == 0)
                early_exporter_secret_count++;
            else if (strcmp(token, "EXPORTER_SECRET") == 0)
                exporter_secret_count++;

            client_random_size = SSL_get_client_random(ssl,
                                                       actual_client_random,
                                                       SSL3_RANDOM_SIZE);
            if (!TEST_size_t_eq(client_random_size, SSL3_RANDOM_SIZE))
                return 0;

            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
            if (!TEST_size_t_eq(strlen(token), 64))
                return 0;
            if (!TEST_false(compare_hex_encoded_buffer(token, 64,
                                                       actual_client_random,
                                                       client_random_size)))
                return 0;

            if (!TEST_ptr(token = strtok(NULL, " \n")))
                return 0;
        } else {
            TEST_info("Unexpected token %s\n", token);
            return 0;
        }
    }

    /* Got what we expected? */
    if (!TEST_size_t_eq(rsa_key_exchange_count,
                        expected->rsa_key_exchange_count)
            || !TEST_size_t_eq(master_secret_count,
                               expected->master_secret_count)
            || !TEST_size_t_eq(client_early_secret_count,
                               expected->client_early_secret_count)
            || !TEST_size_t_eq(client_handshake_secret_count,
                               expected->client_handshake_secret_count)
            || !TEST_size_t_eq(server_handshake_secret_count,
                               expected->server_handshake_secret_count)
            || !TEST_size_t_eq(client_application_secret_count,
                               expected->client_application_secret_count)
            || !TEST_size_t_eq(server_application_secret_count,
                               expected->server_application_secret_count)
            || !TEST_size_t_eq(early_exporter_secret_count,
                               expected->early_exporter_secret_count)
            || !TEST_size_t_eq(exporter_secret_count,
                               expected->exporter_secret_count))
        return 0;
    return 1;
}

#if !defined(OPENSSL_NO_TLS1_2) || defined(OSSL_NO_USABLE_TLS1_3)
static int test_keylog(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    struct sslapitest_log_counts expected;

    /* Clean up logging space */
    memset(&expected, 0, sizeof(expected));
    memset(client_log_buffer, 0, sizeof(client_log_buffer));
    memset(server_log_buffer, 0, sizeof(server_log_buffer));
    client_log_buffer_index = 0;
    server_log_buffer_index = 0;
    error_writing_log = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        return 0;

    /* We cannot log the master secret for TLSv1.3, so we should forbid it. */
    SSL_CTX_set_options(cctx, SSL_OP_NO_TLSv1_3);
    SSL_CTX_set_options(sctx, SSL_OP_NO_TLSv1_3);

    /* We also want to ensure that we use RSA-based key exchange. */
    if (!TEST_true(SSL_CTX_set_cipher_list(cctx, "RSA")))
        goto end;

    if (!TEST_true(SSL_CTX_get_keylog_callback(cctx) == NULL)
            || !TEST_true(SSL_CTX_get_keylog_callback(sctx) == NULL))
        goto end;
    SSL_CTX_set_keylog_callback(cctx, client_keylog_callback);
    if (!TEST_true(SSL_CTX_get_keylog_callback(cctx)
                   == client_keylog_callback))
        goto end;
    SSL_CTX_set_keylog_callback(sctx, server_keylog_callback);
    if (!TEST_true(SSL_CTX_get_keylog_callback(sctx)
                   == server_keylog_callback))
        goto end;

    /* Now do a handshake and check that the logs have been written to. */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_false(error_writing_log)
            || !TEST_int_gt(client_log_buffer_index, 0)
            || !TEST_int_gt(server_log_buffer_index, 0))
        goto end;

    /*
     * Now we want to test that our output data was vaguely sensible. We
     * do that by using strtok and confirming that we have more or less the
     * data we expect. For both client and server, we expect to see one master
     * secret. The client should also see an RSA key exchange.
     */
    expected.rsa_key_exchange_count = 1;
    expected.master_secret_count = 1;
    if (!TEST_true(test_keylog_output(client_log_buffer, clientssl,
                                      SSL_get_session(clientssl), &expected)))
        goto end;

    expected.rsa_key_exchange_count = 0;
    if (!TEST_true(test_keylog_output(server_log_buffer, serverssl,
                                      SSL_get_session(serverssl), &expected)))
        goto end;

    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif

#ifndef OSSL_NO_USABLE_TLS1_3
static int test_keylog_no_master_key(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *sess = NULL;
    int testresult = 0;
    struct sslapitest_log_counts expected;
    unsigned char buf[1];
    size_t readbytes, written;

    /* Clean up logging space */
    memset(&expected, 0, sizeof(expected));
    memset(client_log_buffer, 0, sizeof(client_log_buffer));
    memset(server_log_buffer, 0, sizeof(server_log_buffer));
    client_log_buffer_index = 0;
    server_log_buffer_index = 0;
    error_writing_log = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
        || !TEST_true(SSL_CTX_set_max_early_data(sctx,
                                                 SSL3_RT_MAX_PLAIN_LENGTH)))
        return 0;

    if (!TEST_true(SSL_CTX_get_keylog_callback(cctx) == NULL)
            || !TEST_true(SSL_CTX_get_keylog_callback(sctx) == NULL))
        goto end;

    SSL_CTX_set_keylog_callback(cctx, client_keylog_callback);
    if (!TEST_true(SSL_CTX_get_keylog_callback(cctx)
                   == client_keylog_callback))
        goto end;

    SSL_CTX_set_keylog_callback(sctx, server_keylog_callback);
    if (!TEST_true(SSL_CTX_get_keylog_callback(sctx)
                   == server_keylog_callback))
        goto end;

    /* Now do a handshake and check that the logs have been written to. */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_false(error_writing_log))
        goto end;

    /*
     * Now we want to test that our output data was vaguely sensible. For this
     * test, we expect no CLIENT_RANDOM entry because it doesn't make sense for
     * TLSv1.3, but we do expect both client and server to emit keys.
     */
    expected.client_handshake_secret_count = 1;
    expected.server_handshake_secret_count = 1;
    expected.client_application_secret_count = 1;
    expected.server_application_secret_count = 1;
    expected.exporter_secret_count = 1;
    if (!TEST_true(test_keylog_output(client_log_buffer, clientssl,
                                      SSL_get_session(clientssl), &expected))
            || !TEST_true(test_keylog_output(server_log_buffer, serverssl,
                                             SSL_get_session(serverssl),
                                             &expected)))
        goto end;

    /* Terminate old session and resume with early data. */
    sess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Reset key log */
    memset(client_log_buffer, 0, sizeof(client_log_buffer));
    memset(server_log_buffer, 0, sizeof(server_log_buffer));
    client_log_buffer_index = 0;
    server_log_buffer_index = 0;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess))
            /* Here writing 0 length early data is enough. */
            || !TEST_true(SSL_write_early_data(clientssl, NULL, 0, &written))
            || !TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                                &readbytes),
                            SSL_READ_EARLY_DATA_ERROR)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_ACCEPTED)
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                          SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl)))
        goto end;

    /* In addition to the previous entries, expect early secrets. */
    expected.client_early_secret_count = 1;
    expected.early_exporter_secret_count = 1;
    if (!TEST_true(test_keylog_output(client_log_buffer, clientssl,
                                      SSL_get_session(clientssl), &expected))
            || !TEST_true(test_keylog_output(server_log_buffer, serverssl,
                                             SSL_get_session(serverssl),
                                             &expected)))
        goto end;

    testresult = 1;

end:
    SSL_SESSION_free(sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif

static int verify_retry_cb(X509_STORE_CTX *ctx, void *arg)
{
    int res = X509_verify_cert(ctx);
    int idx = SSL_get_ex_data_X509_STORE_CTX_idx();
    SSL *ssl;

    /* this should not happen but check anyway */
    if (idx < 0
        || (ssl = X509_STORE_CTX_get_ex_data(ctx, idx)) == NULL)
        return 0;

    if (res == 0 && X509_STORE_CTX_get_error(ctx) ==
        X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY)
        /* indicate SSL_ERROR_WANT_RETRY_VERIFY */
        return SSL_set_retry_verify(ssl);

    return res;
}

static int test_client_cert_verify_cb(void)
{
    /* server key, cert, chain, and root */
    char *skey = test_mk_file_path(certsdir, "leaf.key");
    char *leaf = test_mk_file_path(certsdir, "leaf.pem");
    char *int2 = test_mk_file_path(certsdir, "subinterCA.pem");
    char *int1 = test_mk_file_path(certsdir, "interCA.pem");
    char *root = test_mk_file_path(certsdir, "rootCA.pem");
    X509 *crt1 = NULL, *crt2 = NULL;
    STACK_OF(X509) *server_chain;
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, NULL, NULL)))
        goto end;
    if (!TEST_int_eq(SSL_CTX_use_certificate_chain_file(sctx, leaf), 1)
            || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(sctx, skey,
                                                        SSL_FILETYPE_PEM), 1)
            || !TEST_int_eq(SSL_CTX_check_private_key(sctx), 1))
        goto end;
    if (!TEST_true(SSL_CTX_load_verify_locations(cctx, root, NULL)))
        goto end;
    SSL_CTX_set_verify(cctx, SSL_VERIFY_PEER, NULL);
    SSL_CTX_set_cert_verify_callback(cctx, verify_retry_cb, NULL);
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    /* attempt SSL_connect() with incomplete server chain */
    if (!TEST_false(create_ssl_connection(serverssl, clientssl,
                                          SSL_ERROR_WANT_RETRY_VERIFY)))
        goto end;

    /* application provides intermediate certs needed to verify server cert */
    if (!TEST_ptr((crt1 = load_cert_pem(int1, libctx)))
        || !TEST_ptr((crt2 = load_cert_pem(int2, libctx)))
        || !TEST_ptr((server_chain = SSL_get_peer_cert_chain(clientssl))))
        goto end;
    /* add certs in reverse order to demonstrate real chain building */
    if (!TEST_true(sk_X509_push(server_chain, crt1)))
        goto end;
    crt1 = NULL;
    if (!TEST_true(sk_X509_push(server_chain, crt2)))
        goto end;
    crt2 = NULL;

    /* continue SSL_connect(), must now succeed with completed server chain */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

end:
    X509_free(crt1);
    X509_free(crt2);
    if (clientssl != NULL) {
        SSL_shutdown(clientssl);
        SSL_free(clientssl);
    }
    if (serverssl != NULL) {
        SSL_shutdown(serverssl);
        SSL_free(serverssl);
    }
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    OPENSSL_free(skey);
    OPENSSL_free(leaf);
    OPENSSL_free(int2);
    OPENSSL_free(int1);
    OPENSSL_free(root);

    return testresult;
}

static int test_ssl_build_cert_chain(void)
{
    int ret = 0;
    SSL_CTX *ssl_ctx = NULL;
    SSL *ssl = NULL;
    char *skey = test_mk_file_path(certsdir, "leaf.key");
    char *leaf_chain = test_mk_file_path(certsdir, "leaf-chain.pem");

    if (!TEST_ptr(ssl_ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method())))
        goto end;
    if (!TEST_ptr(ssl = SSL_new(ssl_ctx)))
        goto end;
    /* leaf_chain contains leaf + subinterCA + interCA + rootCA */
    if (!TEST_int_eq(SSL_use_certificate_chain_file(ssl, leaf_chain), 1)
        || !TEST_int_eq(SSL_use_PrivateKey_file(ssl, skey, SSL_FILETYPE_PEM), 1)
        || !TEST_int_eq(SSL_check_private_key(ssl), 1))
        goto end;
    if (!TEST_true(SSL_build_cert_chain(ssl, SSL_BUILD_CHAIN_FLAG_NO_ROOT
                                             | SSL_BUILD_CHAIN_FLAG_CHECK)))
        goto end;
    ret = 1;
end:
    SSL_free(ssl);
    SSL_CTX_free(ssl_ctx);
    OPENSSL_free(leaf_chain);
    OPENSSL_free(skey);
    return ret;
}

static int get_password_cb(char *buf, int size, int rw_flag, void *userdata)
{
    static const char pass[] = "testpass";

    if (!TEST_int_eq(size, PEM_BUFSIZE))
        return -1;

    memcpy(buf, pass, sizeof(pass) - 1);
    return sizeof(pass) - 1;
}

static int test_ssl_ctx_build_cert_chain(void)
{
    int ret = 0;
    SSL_CTX *ctx = NULL;
    char *skey = test_mk_file_path(certsdir, "leaf-encrypted.key");
    char *leaf_chain = test_mk_file_path(certsdir, "leaf-chain.pem");

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method())))
        goto end;
    SSL_CTX_set_default_passwd_cb(ctx, get_password_cb);
    /* leaf_chain contains leaf + subinterCA + interCA + rootCA */
    if (!TEST_int_eq(SSL_CTX_use_certificate_chain_file(ctx, leaf_chain), 1)
        || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(ctx, skey,
                                                    SSL_FILETYPE_PEM), 1)
        || !TEST_int_eq(SSL_CTX_check_private_key(ctx), 1))
        goto end;
    if (!TEST_true(SSL_CTX_build_cert_chain(ctx, SSL_BUILD_CHAIN_FLAG_NO_ROOT
                                                | SSL_BUILD_CHAIN_FLAG_CHECK)))
        goto end;
    ret = 1;
end:
    SSL_CTX_free(ctx);
    OPENSSL_free(leaf_chain);
    OPENSSL_free(skey);
    return ret;
}

#ifndef OPENSSL_NO_TLS1_2
static int full_client_hello_callback(SSL *s, int *al, void *arg)
{
    int *ctr = arg;
    const unsigned char *p;
    int *exts;
#ifdef OPENSSL_NO_EC
    const unsigned char expected_ciphers[] = {0x00, 0x9d};
#else
    const unsigned char expected_ciphers[] = {0x00, 0x9d, 0xc0,
                                              0x2c};
#endif
    const int expected_extensions[] = {
                                       65281,
#ifndef OPENSSL_NO_EC
                                       11, 10,
#endif
                                       35, 22, 23, 13};
    size_t len;

    /* Make sure we can defer processing and get called back. */
    if ((*ctr)++ == 0)
        return SSL_CLIENT_HELLO_RETRY;

    len = SSL_client_hello_get0_ciphers(s, &p);
    if (!TEST_mem_eq(p, len, expected_ciphers, sizeof(expected_ciphers))
            || !TEST_size_t_eq(
                       SSL_client_hello_get0_compression_methods(s, &p), 1)
            || !TEST_int_eq(*p, 0))
        return SSL_CLIENT_HELLO_ERROR;
    if (!SSL_client_hello_get1_extensions_present(s, &exts, &len))
        return SSL_CLIENT_HELLO_ERROR;
    if (len != OSSL_NELEM(expected_extensions) ||
        memcmp(exts, expected_extensions, len * sizeof(*exts)) != 0) {
        printf("ClientHello callback expected extensions mismatch\n");
        OPENSSL_free(exts);
        return SSL_CLIENT_HELLO_ERROR;
    }
    OPENSSL_free(exts);
    return SSL_CLIENT_HELLO_SUCCESS;
}

static int test_client_hello_cb(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testctr = 0, testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;
    SSL_CTX_set_client_hello_cb(sctx, full_client_hello_callback, &testctr);

    /* The gimpy cipher list we configure can't do TLS 1.3. */
    SSL_CTX_set_max_proto_version(cctx, TLS1_2_VERSION);
    /* Avoid problems where the default seclevel has been changed */
    SSL_CTX_set_security_level(cctx, 2);
    if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                        "AES256-GCM-SHA384:ECDHE-ECDSA-AES256-GCM-SHA384"))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                             &clientssl, NULL, NULL))
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                        SSL_ERROR_WANT_CLIENT_HELLO_CB))
                /*
                 * Passing a -1 literal is a hack since
                 * the real value was lost.
                 * */
            || !TEST_int_eq(SSL_get_error(serverssl, -1),
                            SSL_ERROR_WANT_CLIENT_HELLO_CB)
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static int test_no_ems(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, status;

    if (!create_ssl_ctx_pair(libctx, TLS_server_method(), TLS_client_method(),
                             TLS1_VERSION, TLS1_2_VERSION,
                             &sctx, &cctx, cert, privkey)) {
        printf("Unable to create SSL_CTX pair\n");
        goto end;
    }

    SSL_CTX_set_options(sctx, SSL_OP_NO_EXTENDED_MASTER_SECRET);

    if (!create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL, NULL)) {
        printf("Unable to create SSL objects\n");
        goto end;
    }

    status = create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE);
    if (fips_ems_check) {
        if (status == 1) {
            printf("When FIPS uses the EMS check a connection that doesn't use EMS should fail\n");
            goto end;
        }
    } else {
        if (!status) {
            printf("Creating SSL connection failed\n");
            goto end;
        }
        if (SSL_get_extms_support(serverssl)) {
            printf("Server reports Extended Master Secret support\n");
            goto end;
        }
        if (SSL_get_extms_support(clientssl)) {
            printf("Client reports Extended Master Secret support\n");
            goto end;
        }
    }
    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Very focused test to exercise a single case in the server-side state
 * machine, when the ChangeCipherState message needs to actually change
 * from one cipher to a different cipher (i.e., not changing from null
 * encryption to real encryption).
 */
static int test_ccs_change_cipher(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *sess = NULL, *sesspre, *sesspost;
    int testresult = 0;
    int i;
    unsigned char buf;
    size_t readbytes;

    /*
     * Create a connection so we can resume and potentially (but not) use
     * a different cipher in the second connection.
     */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION, TLS1_2_VERSION,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(SSL_CTX_set_options(sctx, SSL_OP_NO_TICKET))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                          NULL, NULL))
            || !TEST_true(SSL_set_cipher_list(clientssl, "AES128-GCM-SHA256"))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_ptr(sesspre = SSL_get0_session(serverssl))
            || !TEST_ptr(sess = SSL_get1_session(clientssl)))
        goto end;

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;

    /* Resume, preferring a different cipher. Our server will force the
     * same cipher to be used as the initial handshake. */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                          NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess))
            || !TEST_true(SSL_set_cipher_list(clientssl, "AES256-GCM-SHA384:AES128-GCM-SHA256"))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl))
            || !TEST_true(SSL_session_reused(serverssl))
            || !TEST_ptr(sesspost = SSL_get0_session(serverssl))
            || !TEST_ptr_eq(sesspre, sesspost)
            || !TEST_int_eq(TLS1_CK_RSA_WITH_AES_128_GCM_SHA256,
                            SSL_CIPHER_get_id(SSL_get_current_cipher(clientssl))))
        goto end;
    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;

    /*
     * Now create a fresh connection and try to renegotiate a different
     * cipher on it.
     */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(SSL_set_cipher_list(clientssl, "AES128-GCM-SHA256"))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_ptr(sesspre = SSL_get0_session(serverssl))
            || !TEST_true(SSL_set_cipher_list(clientssl, "AES256-GCM-SHA384"))
            || !TEST_true(SSL_renegotiate(clientssl))
            || !TEST_true(SSL_renegotiate_pending(clientssl)))
        goto end;
    /* Actually drive the renegotiation. */
    for (i = 0; i < 3; i++) {
        if (SSL_read_ex(clientssl, &buf, sizeof(buf), &readbytes) > 0) {
            if (!TEST_ulong_eq(readbytes, 0))
                goto end;
        } else if (!TEST_int_eq(SSL_get_error(clientssl, 0),
                                SSL_ERROR_WANT_READ)) {
            goto end;
        }
        if (SSL_read_ex(serverssl, &buf, sizeof(buf), &readbytes) > 0) {
            if (!TEST_ulong_eq(readbytes, 0))
                goto end;
        } else if (!TEST_int_eq(SSL_get_error(serverssl, 0),
                                SSL_ERROR_WANT_READ)) {
            goto end;
        }
    }
    /* sesspre and sesspost should be different since the cipher changed. */
    if (!TEST_false(SSL_renegotiate_pending(clientssl))
            || !TEST_false(SSL_session_reused(clientssl))
            || !TEST_false(SSL_session_reused(serverssl))
            || !TEST_ptr(sesspost = SSL_get0_session(serverssl))
            || !TEST_ptr_ne(sesspre, sesspost)
            || !TEST_int_eq(TLS1_CK_RSA_WITH_AES_256_GCM_SHA384,
                            SSL_CIPHER_get_id(SSL_get_current_cipher(clientssl))))
        goto end;

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;

    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(sess);

    return testresult;
}
#endif

static int execute_test_large_message(const SSL_METHOD *smeth,
                                      const SSL_METHOD *cmeth,
                                      int min_version, int max_version,
                                      int read_ahead)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, smeth, cmeth, min_version,
                                       max_version, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

#ifdef OPENSSL_NO_DTLS1_2
    if (smeth == DTLS_server_method()) {
        /*
         * Default sigalgs are SHA1 based in <DTLS1.2 which is in security
         * level 0
         */
        if (!TEST_true(SSL_CTX_set_cipher_list(sctx, "DEFAULT:@SECLEVEL=0"))
                || !TEST_true(SSL_CTX_set_cipher_list(cctx,
                                                    "DEFAULT:@SECLEVEL=0")))
            goto end;
    }
#endif

    if (read_ahead) {
        /*
         * Test that read_ahead works correctly when dealing with large
         * records
         */
        SSL_CTX_set_read_ahead(cctx, 1);
    }

    if (!ssl_ctx_add_large_cert_chain(libctx, sctx, cert))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    /*
     * Calling SSL_clear() first is not required but this tests that SSL_clear()
     * doesn't leak.
     */
    if (!TEST_true(SSL_clear(serverssl)))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

#if !defined(OPENSSL_NO_SOCK) && !defined(OPENSSL_NO_KTLS) && \
    !(defined(OSSL_NO_USABLE_TLS1_3) && defined(OPENSSL_NO_TLS1_2))
/* sock must be connected */
static int ktls_chk_platform(int sock)
{
    if (!ktls_enable(sock))
        return 0;
    return 1;
}

static int ping_pong_query(SSL *clientssl, SSL *serverssl)
{
    static char count = 1;
    unsigned char cbuf[16000] = {0};
    unsigned char sbuf[16000];
    size_t err = 0;
    char crec_wseq_before[SEQ_NUM_SIZE];
    char crec_wseq_after[SEQ_NUM_SIZE];
    char crec_rseq_before[SEQ_NUM_SIZE];
    char crec_rseq_after[SEQ_NUM_SIZE];
    char srec_wseq_before[SEQ_NUM_SIZE];
    char srec_wseq_after[SEQ_NUM_SIZE];
    char srec_rseq_before[SEQ_NUM_SIZE];
    char srec_rseq_after[SEQ_NUM_SIZE];
    SSL_CONNECTION *clientsc, *serversc;

    if (!TEST_ptr(clientsc = SSL_CONNECTION_FROM_SSL_ONLY(clientssl))
        || !TEST_ptr(serversc = SSL_CONNECTION_FROM_SSL_ONLY(serverssl)))
        goto end;

    cbuf[0] = count++;
    memcpy(crec_wseq_before, &clientsc->rlayer.wrl->sequence, SEQ_NUM_SIZE);
    memcpy(srec_wseq_before, &serversc->rlayer.wrl->sequence, SEQ_NUM_SIZE);
    memcpy(crec_rseq_before, &clientsc->rlayer.rrl->sequence, SEQ_NUM_SIZE);
    memcpy(srec_rseq_before, &serversc->rlayer.rrl->sequence, SEQ_NUM_SIZE);

    if (!TEST_true(SSL_write(clientssl, cbuf, sizeof(cbuf)) == sizeof(cbuf)))
        goto end;

    while ((err = SSL_read(serverssl, &sbuf, sizeof(sbuf))) != sizeof(sbuf)) {
        if (SSL_get_error(serverssl, err) != SSL_ERROR_WANT_READ) {
            goto end;
        }
    }

    if (!TEST_true(SSL_write(serverssl, sbuf, sizeof(sbuf)) == sizeof(sbuf)))
        goto end;

    while ((err = SSL_read(clientssl, &cbuf, sizeof(cbuf))) != sizeof(cbuf)) {
        if (SSL_get_error(clientssl, err) != SSL_ERROR_WANT_READ) {
            goto end;
        }
    }

    memcpy(crec_wseq_after, &clientsc->rlayer.wrl->sequence, SEQ_NUM_SIZE);
    memcpy(srec_wseq_after, &serversc->rlayer.wrl->sequence, SEQ_NUM_SIZE);
    memcpy(crec_rseq_after, &clientsc->rlayer.rrl->sequence, SEQ_NUM_SIZE);
    memcpy(srec_rseq_after, &serversc->rlayer.rrl->sequence, SEQ_NUM_SIZE);

    /* verify the payload */
    if (!TEST_mem_eq(cbuf, sizeof(cbuf), sbuf, sizeof(sbuf)))
        goto end;

    /*
     * If ktls is used then kernel sequences are used instead of
     * OpenSSL sequences
     */
    if (!BIO_get_ktls_send(clientsc->wbio)) {
        if (!TEST_mem_ne(crec_wseq_before, SEQ_NUM_SIZE,
                         crec_wseq_after, SEQ_NUM_SIZE))
            goto end;
    } else {
        if (!TEST_mem_eq(crec_wseq_before, SEQ_NUM_SIZE,
                         crec_wseq_after, SEQ_NUM_SIZE))
            goto end;
    }

    if (!BIO_get_ktls_send(serversc->wbio)) {
        if (!TEST_mem_ne(srec_wseq_before, SEQ_NUM_SIZE,
                         srec_wseq_after, SEQ_NUM_SIZE))
            goto end;
    } else {
        if (!TEST_mem_eq(srec_wseq_before, SEQ_NUM_SIZE,
                         srec_wseq_after, SEQ_NUM_SIZE))
            goto end;
    }

    if (!BIO_get_ktls_recv(clientsc->wbio)) {
        if (!TEST_mem_ne(crec_rseq_before, SEQ_NUM_SIZE,
                         crec_rseq_after, SEQ_NUM_SIZE))
            goto end;
    } else {
        if (!TEST_mem_eq(crec_rseq_before, SEQ_NUM_SIZE,
                         crec_rseq_after, SEQ_NUM_SIZE))
            goto end;
    }

    if (!BIO_get_ktls_recv(serversc->wbio)) {
        if (!TEST_mem_ne(srec_rseq_before, SEQ_NUM_SIZE,
                         srec_rseq_after, SEQ_NUM_SIZE))
            goto end;
    } else {
        if (!TEST_mem_eq(srec_rseq_before, SEQ_NUM_SIZE,
                         srec_rseq_after, SEQ_NUM_SIZE))
            goto end;
    }

    return 1;
end:
    return 0;
}

static int execute_test_ktls(int cis_ktls, int sis_ktls,
                             int tls_version, const char *cipher)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int ktls_used = 0, testresult = 0;
    int cfd = -1, sfd = -1;
    int rx_supported;
    SSL_CONNECTION *clientsc, *serversc;
    unsigned char *buf = NULL;
    const size_t bufsz = SSL3_RT_MAX_PLAIN_LENGTH + 16;
    int ret;
    size_t offset = 0, i;

    if (!TEST_true(create_test_sockets(&cfd, &sfd, SOCK_STREAM, NULL)))
        goto end;

    /* Skip this test if the platform does not support ktls */
    if (!ktls_chk_platform(cfd)) {
        testresult = TEST_skip("Kernel does not support KTLS");
        goto end;
    }

    if (is_fips && strstr(cipher, "CHACHA") != NULL) {
        testresult = TEST_skip("CHACHA is not supported in FIPS");
        goto end;
    }

    /* Create a session based on SHA-256 */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       tls_version, tls_version,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (tls_version == TLS1_3_VERSION) {
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx, cipher))
            || !TEST_true(SSL_CTX_set_ciphersuites(sctx, cipher)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx, cipher))
            || !TEST_true(SSL_CTX_set_cipher_list(sctx, cipher)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects2(sctx, cctx, &serverssl,
                                       &clientssl, sfd, cfd)))
        goto end;

    if (!TEST_ptr(clientsc = SSL_CONNECTION_FROM_SSL_ONLY(clientssl))
        || !TEST_ptr(serversc = SSL_CONNECTION_FROM_SSL_ONLY(serverssl)))
        goto end;

    if (cis_ktls) {
        if (!TEST_true(SSL_set_options(clientssl, SSL_OP_ENABLE_KTLS)))
            goto end;
    }

    if (sis_ktls) {
        if (!TEST_true(SSL_set_options(serverssl, SSL_OP_ENABLE_KTLS)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /*
     * The running kernel may not support a given cipher suite
     * or direction, so just check that KTLS isn't used when it
     * isn't enabled.
     */
    if (!cis_ktls) {
        if (!TEST_false(BIO_get_ktls_send(clientsc->wbio)))
            goto end;
    } else {
        if (BIO_get_ktls_send(clientsc->wbio))
            ktls_used = 1;
    }

    if (!sis_ktls) {
        if (!TEST_false(BIO_get_ktls_send(serversc->wbio)))
            goto end;
    } else {
        if (BIO_get_ktls_send(serversc->wbio))
            ktls_used = 1;
    }

#if defined(OPENSSL_NO_KTLS_RX)
    rx_supported = 0;
#else
    rx_supported = 1;
#endif
    if (!cis_ktls || !rx_supported) {
        if (!TEST_false(BIO_get_ktls_recv(clientsc->rbio)))
            goto end;
    } else {
        if (BIO_get_ktls_send(clientsc->rbio))
            ktls_used = 1;
    }

    if (!sis_ktls || !rx_supported) {
        if (!TEST_false(BIO_get_ktls_recv(serversc->rbio)))
            goto end;
    } else {
        if (BIO_get_ktls_send(serversc->rbio))
            ktls_used = 1;
    }

    if ((cis_ktls || sis_ktls) && !ktls_used) {
        testresult = TEST_skip("KTLS not supported for %s cipher %s",
                               tls_version == TLS1_3_VERSION ? "TLS 1.3" :
                               "TLS 1.2", cipher);
        goto end;
    }

    if (!TEST_true(ping_pong_query(clientssl, serverssl)))
        goto end;

    buf = OPENSSL_zalloc(bufsz);
    if (!TEST_ptr(buf))
        goto end;

    /*
     * Write some data that exceeds the maximum record length. KTLS may choose
     * to coalesce this data into a single buffer when we read it again.
     */
    while ((ret = SSL_write(clientssl, buf, bufsz)) != (int)bufsz) {
        if (!TEST_true(SSL_get_error(clientssl, ret) == SSL_ERROR_WANT_WRITE))
            goto end;
    }

    /* Now check that we can read all the data we wrote */
    do {
        ret = SSL_read(serverssl, buf + offset, bufsz - offset);
        if (ret <= 0) {
            if (!TEST_true(SSL_get_error(serverssl, ret) == SSL_ERROR_WANT_READ))
                goto end;
        } else {
            offset += ret;
        }
    } while (offset < bufsz);

    if (!TEST_true(offset == bufsz))
        goto end;
    for (i = 0; i < bufsz; i++)
        if (!TEST_true(buf[i] == 0))
            goto end;

    testresult = 1;
end:
    OPENSSL_free(buf);
    if (clientssl) {
        SSL_shutdown(clientssl);
        SSL_free(clientssl);
    }
    if (serverssl) {
        SSL_shutdown(serverssl);
        SSL_free(serverssl);
    }
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    serverssl = clientssl = NULL;
    if (cfd != -1)
        close(cfd);
    if (sfd != -1)
        close(sfd);
    return testresult;
}

#define SENDFILE_SZ                     (16 * 4096)
#define SENDFILE_CHUNK                  (4 * 4096)
#define min(a,b)                        ((a) > (b) ? (b) : (a))

static int execute_test_ktls_sendfile(int tls_version, const char *cipher,
                                      int zerocopy)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    unsigned char *buf, *buf_dst;
    BIO *out = NULL, *in = NULL;
    int cfd = -1, sfd = -1, ffd, err;
    ssize_t chunk_size = 0;
    off_t chunk_off = 0;
    int testresult = 0;
    FILE *ffdp;
    SSL_CONNECTION *serversc;

    buf = OPENSSL_zalloc(SENDFILE_SZ);
    buf_dst = OPENSSL_zalloc(SENDFILE_SZ);
    if (!TEST_ptr(buf) || !TEST_ptr(buf_dst)
        || !TEST_true(create_test_sockets(&cfd, &sfd, SOCK_STREAM, NULL)))
        goto end;

    /* Skip this test if the platform does not support ktls */
    if (!ktls_chk_platform(sfd)) {
        testresult = TEST_skip("Kernel does not support KTLS");
        goto end;
    }

    if (is_fips && strstr(cipher, "CHACHA") != NULL) {
        testresult = TEST_skip("CHACHA is not supported in FIPS");
        goto end;
    }

    /* Create a session based on SHA-256 */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       tls_version, tls_version,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (tls_version == TLS1_3_VERSION) {
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx, cipher))
            || !TEST_true(SSL_CTX_set_ciphersuites(sctx, cipher)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx, cipher))
            || !TEST_true(SSL_CTX_set_cipher_list(sctx, cipher)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects2(sctx, cctx, &serverssl,
                                       &clientssl, sfd, cfd)))
        goto end;

    if (!TEST_ptr(serversc = SSL_CONNECTION_FROM_SSL_ONLY(serverssl)))
        goto end;

    if (!TEST_true(SSL_set_options(serverssl, SSL_OP_ENABLE_KTLS)))
        goto end;

    if (zerocopy) {
        if (!TEST_true(SSL_set_options(serverssl,
                                       SSL_OP_ENABLE_KTLS_TX_ZEROCOPY_SENDFILE)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    if (!BIO_get_ktls_send(serversc->wbio)) {
        testresult = TEST_skip("Failed to enable KTLS for %s cipher %s",
                               tls_version == TLS1_3_VERSION ? "TLS 1.3" :
                               "TLS 1.2", cipher);
        goto end;
    }

    if (!TEST_int_gt(RAND_bytes_ex(libctx, buf, SENDFILE_SZ, 0), 0))
        goto end;

    out = BIO_new_file(tmpfilename, "wb");
    if (!TEST_ptr(out))
        goto end;

    if (BIO_write(out, buf, SENDFILE_SZ) != SENDFILE_SZ)
        goto end;

    BIO_free(out);
    out = NULL;
    in = BIO_new_file(tmpfilename, "rb");
    BIO_get_fp(in, &ffdp);
    ffd = fileno(ffdp);

    while (chunk_off < SENDFILE_SZ) {
        chunk_size = min(SENDFILE_CHUNK, SENDFILE_SZ - chunk_off);
        while ((err = SSL_sendfile(serverssl,
                                   ffd,
                                   chunk_off,
                                   chunk_size,
                                   0)) != chunk_size) {
            if (SSL_get_error(serverssl, err) != SSL_ERROR_WANT_WRITE)
                goto end;
        }
        while ((err = SSL_read(clientssl,
                               buf_dst + chunk_off,
                               chunk_size)) != chunk_size) {
            if (SSL_get_error(clientssl, err) != SSL_ERROR_WANT_READ)
                goto end;
        }

        /* verify the payload */
        if (!TEST_mem_eq(buf_dst + chunk_off,
                         chunk_size,
                         buf + chunk_off,
                         chunk_size))
            goto end;

        chunk_off += chunk_size;
    }

    testresult = 1;
end:
    if (clientssl) {
        SSL_shutdown(clientssl);
        SSL_free(clientssl);
    }
    if (serverssl) {
        SSL_shutdown(serverssl);
        SSL_free(serverssl);
    }
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    serverssl = clientssl = NULL;
    BIO_free(out);
    BIO_free(in);
    if (cfd != -1)
        close(cfd);
    if (sfd != -1)
        close(sfd);
    OPENSSL_free(buf);
    OPENSSL_free(buf_dst);
    return testresult;
}

static struct ktls_test_cipher {
    int tls_version;
    const char *cipher;
} ktls_test_ciphers[] = {
# if !defined(OPENSSL_NO_TLS1_2)
#  ifdef OPENSSL_KTLS_AES_GCM_128
    { TLS1_2_VERSION, "AES128-GCM-SHA256" },
#  endif
#  ifdef OPENSSL_KTLS_AES_CCM_128
    { TLS1_2_VERSION, "AES128-CCM"},
#  endif
#  ifdef OPENSSL_KTLS_AES_GCM_256
    { TLS1_2_VERSION, "AES256-GCM-SHA384"},
#  endif
#  ifdef OPENSSL_KTLS_CHACHA20_POLY1305
#    ifndef OPENSSL_NO_EC
    { TLS1_2_VERSION, "ECDHE-RSA-CHACHA20-POLY1305"},
#    endif
#  endif
# endif
# if !defined(OSSL_NO_USABLE_TLS1_3)
#  ifdef OPENSSL_KTLS_AES_GCM_128
    { TLS1_3_VERSION, "TLS_AES_128_GCM_SHA256" },
#  endif
#  ifdef OPENSSL_KTLS_AES_CCM_128
    { TLS1_3_VERSION, "TLS_AES_128_CCM_SHA256" },
#  endif
#  ifdef OPENSSL_KTLS_AES_GCM_256
    { TLS1_3_VERSION, "TLS_AES_256_GCM_SHA384" },
#  endif
#  ifdef OPENSSL_KTLS_CHACHA20_POLY1305
    { TLS1_3_VERSION, "TLS_CHACHA20_POLY1305_SHA256" },
#  endif
# endif
};

#define NUM_KTLS_TEST_CIPHERS OSSL_NELEM(ktls_test_ciphers)

static int test_ktls(int test)
{
    struct ktls_test_cipher *cipher;
    int cis_ktls, sis_ktls;

    OPENSSL_assert(test / 4 < (int)NUM_KTLS_TEST_CIPHERS);
    cipher = &ktls_test_ciphers[test / 4];

    cis_ktls = (test & 1) != 0;
    sis_ktls = (test & 2) != 0;

    return execute_test_ktls(cis_ktls, sis_ktls, cipher->tls_version,
                             cipher->cipher);
}

static int test_ktls_sendfile(int test)
{
    struct ktls_test_cipher *cipher;
    int tst = test >> 1;

    OPENSSL_assert(tst < (int)NUM_KTLS_TEST_CIPHERS);
    cipher = &ktls_test_ciphers[tst];

    return execute_test_ktls_sendfile(cipher->tls_version, cipher->cipher,
                                      test & 1);
}
#endif

static int test_large_message_tls(void)
{
    return execute_test_large_message(TLS_server_method(), TLS_client_method(),
                                      TLS1_VERSION, 0, 0);
}

static int test_large_message_tls_read_ahead(void)
{
    return execute_test_large_message(TLS_server_method(), TLS_client_method(),
                                      TLS1_VERSION, 0, 1);
}

#ifndef OPENSSL_NO_DTLS
static int test_large_message_dtls(void)
{
# ifdef OPENSSL_NO_DTLS1_2
    /* Not supported in the FIPS provider */
    if (is_fips)
        return 1;
# endif
    /*
     * read_ahead is not relevant to DTLS because DTLS always acts as if
     * read_ahead is set.
     */
    return execute_test_large_message(DTLS_server_method(),
                                      DTLS_client_method(),
                                      DTLS1_VERSION, 0, 0);
}
#endif

/*
 * Test we can successfully send the maximum amount of application data. We
 * test each protocol version individually, each with and without EtM enabled.
 * TLSv1.3 doesn't use EtM so technically it is redundant to test both but it is
 * simpler this way. We also test all combinations with and without the
 * SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS option which affects the size of the
 * underlying buffer.
 */
static int test_large_app_data(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, prot;
    unsigned char *msg, *buf = NULL;
    size_t written, readbytes;
    const SSL_METHOD *smeth = TLS_server_method();
    const SSL_METHOD *cmeth = TLS_client_method();

    switch (tst >> 2) {
    case 0:
#ifndef OSSL_NO_USABLE_TLS1_3
        prot = TLS1_3_VERSION;
        break;
#else
        return TEST_skip("TLS 1.3 not supported");
#endif

    case 1:
#ifndef OPENSSL_NO_TLS1_2
        prot = TLS1_2_VERSION;
        break;
#else
        return TEST_skip("TLS 1.2 not supported");
#endif

    case 2:
#ifndef OPENSSL_NO_TLS1_1
        prot = TLS1_1_VERSION;
        break;
#else
        return TEST_skip("TLS 1.1 not supported");
#endif

    case 3:
#ifndef OPENSSL_NO_TLS1
        prot = TLS1_VERSION;
        break;
#else
        return TEST_skip("TLS 1 not supported");
#endif

    case 4:
#ifndef OPENSSL_NO_SSL3
        prot = SSL3_VERSION;
        break;
#else
        return TEST_skip("SSL 3 not supported");
#endif

    case 5:
#ifndef OPENSSL_NO_DTLS1_2
        prot = DTLS1_2_VERSION;
        smeth = DTLS_server_method();
        cmeth = DTLS_client_method();
        break;
#else
        return TEST_skip("DTLS 1.2 not supported");
#endif

    case 6:
#ifndef OPENSSL_NO_DTLS1
        if (is_fips)
            return TEST_skip("DTLS 1 not supported by FIPS provider");
        prot = DTLS1_VERSION;
        smeth = DTLS_server_method();
        cmeth = DTLS_client_method();
        break;
#else
        return TEST_skip("DTLS 1 not supported");
#endif

    default:
        /* Shouldn't happen */
        return 0;
    }

    if (is_fips && prot < TLS1_2_VERSION)
        return TEST_skip("TLS versions < 1.2 not supported by FIPS provider");

    /* Maximal sized message of zeros */
    msg = OPENSSL_zalloc(SSL3_RT_MAX_PLAIN_LENGTH);
    if (!TEST_ptr(msg))
        goto end;

    buf = OPENSSL_malloc(SSL3_RT_MAX_PLAIN_LENGTH + 1);
    if (!TEST_ptr(buf))
        goto end;
    /* Set whole buffer to all bits set */
    memset(buf, 0xff, SSL3_RT_MAX_PLAIN_LENGTH + 1);

    if (!TEST_true(create_ssl_ctx_pair(libctx, smeth, cmeth, prot, prot,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (prot < TLS1_2_VERSION || prot == DTLS1_VERSION) {
        /* Older protocol versions need SECLEVEL=0 due to SHA1 usage */
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx, "DEFAULT:@SECLEVEL=0"))
                || !TEST_true(SSL_CTX_set_cipher_list(sctx,
                                                      "DEFAULT:@SECLEVEL=0")))
        goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if ((tst & 1) != 0) {
        /* Setting this option gives us a minimally sized underlying buffer */
        if (!TEST_true(SSL_set_options(serverssl,
                                       SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS))
                || !TEST_true(SSL_set_options(clientssl,
                                              SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS)))
            goto end;
    }

    if ((tst & 2) != 0) {
        /*
         * Setting this option means the MAC is added before encryption
         * giving us a larger record for the encryption process
         */
        if (!TEST_true(SSL_set_options(serverssl, SSL_OP_NO_ENCRYPT_THEN_MAC))
                || !TEST_true(SSL_set_options(clientssl,
                                              SSL_OP_NO_ENCRYPT_THEN_MAC)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_true(SSL_write_ex(clientssl, msg, SSL3_RT_MAX_PLAIN_LENGTH,
                                &written))
            || !TEST_size_t_eq(written, SSL3_RT_MAX_PLAIN_LENGTH))
        goto end;

    /* We provide a buffer slightly larger than what we are actually expecting */
    if (!TEST_true(SSL_read_ex(serverssl, buf, SSL3_RT_MAX_PLAIN_LENGTH + 1,
                               &readbytes)))
        goto end;

    if (!TEST_mem_eq(msg, written, buf, readbytes))
        goto end;

    testresult = 1;
end:
    OPENSSL_free(msg);
    OPENSSL_free(buf);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

#if !defined(OPENSSL_NO_TLS1_2) || !defined(OSSL_NO_USABLE_TLS1_3) \
    || !defined(OPENSSL_NO_DTLS)
static int execute_cleanse_plaintext(const SSL_METHOD *smeth,
                                     const SSL_METHOD *cmeth,
                                     int min_version, int max_version)
{
    size_t i;
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    const unsigned char *zbuf;
    SSL_CONNECTION *serversc;
    TLS_RECORD *rr;

    static unsigned char cbuf[16000];
    static unsigned char sbuf[16000];

    if (!TEST_true(create_ssl_ctx_pair(libctx,
                                       smeth, cmeth,
                                       min_version, max_version,
                                       &sctx, &cctx, cert,
                                       privkey)))
        goto end;

# ifdef OPENSSL_NO_DTLS1_2
    if (smeth == DTLS_server_method()) {
        /* Not supported in the FIPS provider */
        if (is_fips) {
            testresult = 1;
            goto end;
        };
        /*
         * Default sigalgs are SHA1 based in <DTLS1.2 which is in security
         * level 0
         */
        if (!TEST_true(SSL_CTX_set_cipher_list(sctx, "DEFAULT:@SECLEVEL=0"))
                || !TEST_true(SSL_CTX_set_cipher_list(cctx,
                                                    "DEFAULT:@SECLEVEL=0")))
            goto end;
    }
# endif

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_options(serverssl, SSL_OP_CLEANSE_PLAINTEXT)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    for (i = 0; i < sizeof(cbuf); i++) {
        cbuf[i] = i & 0xff;
    }

    if (!TEST_int_eq(SSL_write(clientssl, cbuf, sizeof(cbuf)), sizeof(cbuf)))
        goto end;

    if (!TEST_int_eq(SSL_peek(serverssl, &sbuf, sizeof(sbuf)), sizeof(sbuf)))
        goto end;

    if (!TEST_mem_eq(cbuf, sizeof(cbuf), sbuf, sizeof(sbuf)))
        goto end;

    /*
     * Since we called SSL_peek(), we know the data in the record
     * layer is a plaintext record. We can gather the pointer to check
     * for zeroization after SSL_read().
     */
    if (!TEST_ptr(serversc = SSL_CONNECTION_FROM_SSL_ONLY(serverssl)))
        goto end;
    rr = serversc->rlayer.tlsrecs;

    zbuf = &rr->data[rr->off];
    if (!TEST_int_eq(rr->length, sizeof(cbuf)))
        goto end;

    /*
     * After SSL_peek() the plaintext must still be stored in the
     * record.
     */
    if (!TEST_mem_eq(cbuf, sizeof(cbuf), zbuf, sizeof(cbuf)))
        goto end;

    memset(sbuf, 0, sizeof(sbuf));
    if (!TEST_int_eq(SSL_read(serverssl, &sbuf, sizeof(sbuf)), sizeof(sbuf)))
        goto end;

    if (!TEST_mem_eq(cbuf, sizeof(cbuf), sbuf, sizeof(cbuf)))
        goto end;

    /* Check if rbuf is cleansed */
    memset(cbuf, 0, sizeof(cbuf));
    if (!TEST_mem_eq(cbuf, sizeof(cbuf), zbuf, sizeof(cbuf)))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /*
        * !defined(OPENSSL_NO_TLS1_2) || !defined(OSSL_NO_USABLE_TLS1_3)
        * || !defined(OPENSSL_NO_DTLS)
        */

static int test_cleanse_plaintext(void)
{
#if !defined(OPENSSL_NO_TLS1_2)
    if (!TEST_true(execute_cleanse_plaintext(TLS_server_method(),
                                             TLS_client_method(),
                                             TLS1_2_VERSION,
                                             TLS1_2_VERSION)))
        return 0;

#endif

#if !defined(OSSL_NO_USABLE_TLS1_3)
    if (!TEST_true(execute_cleanse_plaintext(TLS_server_method(),
                                             TLS_client_method(),
                                             TLS1_3_VERSION,
                                             TLS1_3_VERSION)))
        return 0;
#endif

#if !defined(OPENSSL_NO_DTLS)

    if (!TEST_true(execute_cleanse_plaintext(DTLS_server_method(),
                                             DTLS_client_method(),
                                             DTLS1_VERSION,
                                             0)))
        return 0;
#endif
    return 1;
}

#ifndef OPENSSL_NO_OCSP
static int ocsp_server_cb(SSL *s, void *arg)
{
    int *argi = (int *)arg;
    unsigned char *copy = NULL;
    STACK_OF(OCSP_RESPID) *ids = NULL;
    OCSP_RESPID *id = NULL;

    if (*argi == 2) {
        /* In this test we are expecting exactly 1 OCSP_RESPID */
        SSL_get_tlsext_status_ids(s, &ids);
        if (ids == NULL || sk_OCSP_RESPID_num(ids) != 1)
            return SSL_TLSEXT_ERR_ALERT_FATAL;

        id = sk_OCSP_RESPID_value(ids, 0);
        if (id == NULL || !OCSP_RESPID_match_ex(id, ocspcert, libctx, NULL))
            return SSL_TLSEXT_ERR_ALERT_FATAL;
    } else if (*argi != 1) {
        return SSL_TLSEXT_ERR_ALERT_FATAL;
    }

    if (!TEST_ptr(copy = OPENSSL_memdup(orespder, sizeof(orespder))))
        return SSL_TLSEXT_ERR_ALERT_FATAL;

    if (!TEST_true(SSL_set_tlsext_status_ocsp_resp(s, copy,
                                                   sizeof(orespder)))) {
        OPENSSL_free(copy);
        return SSL_TLSEXT_ERR_ALERT_FATAL;
    }
    ocsp_server_called = 1;
    return SSL_TLSEXT_ERR_OK;
}

static int ocsp_client_cb(SSL *s, void *arg)
{
    int *argi = (int *)arg;
    const unsigned char *respderin;
    size_t len;

    if (*argi != 1 && *argi != 2)
        return 0;

    len = SSL_get_tlsext_status_ocsp_resp(s, &respderin);
    if (!TEST_mem_eq(orespder, len, respderin, len))
        return 0;

    ocsp_client_called = 1;
    return 1;
}

static int test_tlsext_status_type(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    STACK_OF(OCSP_RESPID) *ids = NULL;
    OCSP_RESPID *id = NULL;
    BIO *certbio = NULL;

    if (!create_ssl_ctx_pair(libctx, TLS_server_method(), TLS_client_method(),
                             TLS1_VERSION, 0,
                             &sctx, &cctx, cert, privkey))
        return 0;

    if (SSL_CTX_get_tlsext_status_type(cctx) != -1)
        goto end;

    /* First just do various checks getting and setting tlsext_status_type */

    clientssl = SSL_new(cctx);
    if (!TEST_ptr(clientssl))
        goto end;
    if (!TEST_int_eq(SSL_get_tlsext_status_type(clientssl), -1)
            || !TEST_true(SSL_set_tlsext_status_type(clientssl,
                                                      TLSEXT_STATUSTYPE_ocsp))
            || !TEST_int_eq(SSL_get_tlsext_status_type(clientssl),
                            TLSEXT_STATUSTYPE_ocsp))
        goto end;

    SSL_free(clientssl);
    clientssl = NULL;

    if (!SSL_CTX_set_tlsext_status_type(cctx, TLSEXT_STATUSTYPE_ocsp)
     || SSL_CTX_get_tlsext_status_type(cctx) != TLSEXT_STATUSTYPE_ocsp)
        goto end;

    clientssl = SSL_new(cctx);
    if (!TEST_ptr(clientssl))
        goto end;
    if (SSL_get_tlsext_status_type(clientssl) != TLSEXT_STATUSTYPE_ocsp)
        goto end;
    SSL_free(clientssl);
    clientssl = NULL;

    /*
     * Now actually do a handshake and check OCSP information is exchanged and
     * the callbacks get called
     */
    SSL_CTX_set_tlsext_status_cb(cctx, ocsp_client_cb);
    SSL_CTX_set_tlsext_status_arg(cctx, &cdummyarg);
    SSL_CTX_set_tlsext_status_cb(sctx, ocsp_server_cb);
    SSL_CTX_set_tlsext_status_arg(sctx, &cdummyarg);
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_true(ocsp_client_called)
            || !TEST_true(ocsp_server_called))
        goto end;
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = NULL;
    clientssl = NULL;

    /* Try again but this time force the server side callback to fail */
    ocsp_client_called = 0;
    ocsp_server_called = 0;
    cdummyarg = 0;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
                /* This should fail because the callback will fail */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                 SSL_ERROR_NONE))
            || !TEST_false(ocsp_client_called)
            || !TEST_false(ocsp_server_called))
        goto end;
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = NULL;
    clientssl = NULL;

    /*
     * This time we'll get the client to send an OCSP_RESPID that it will
     * accept.
     */
    ocsp_client_called = 0;
    ocsp_server_called = 0;
    cdummyarg = 2;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    /*
     * We'll just use any old cert for this test - it doesn't have to be an OCSP
     * specific one. We'll use the server cert.
     */
    if (!TEST_ptr(certbio = BIO_new_file(cert, "r"))
            || !TEST_ptr(id = OCSP_RESPID_new())
            || !TEST_ptr(ids = sk_OCSP_RESPID_new_null())
            || !TEST_ptr(ocspcert = X509_new_ex(libctx, NULL))
            || !TEST_ptr(PEM_read_bio_X509(certbio, &ocspcert, NULL, NULL))
            || !TEST_true(OCSP_RESPID_set_by_key_ex(id, ocspcert, libctx, NULL))
            || !TEST_true(sk_OCSP_RESPID_push(ids, id)))
        goto end;
    id = NULL;
    SSL_set_tlsext_status_ids(clientssl, ids);
    /* Control has been transferred */
    ids = NULL;

    BIO_free(certbio);
    certbio = NULL;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE))
            || !TEST_true(ocsp_client_called)
            || !TEST_true(ocsp_server_called))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    sk_OCSP_RESPID_pop_free(ids, OCSP_RESPID_free);
    OCSP_RESPID_free(id);
    BIO_free(certbio);
    X509_free(ocspcert);
    ocspcert = NULL;

    return testresult;
}
#endif

#if !defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2)
static int new_called, remove_called, get_called;

static int new_session_cb(SSL *ssl, SSL_SESSION *sess)
{
    new_called++;
    /*
     * sess has been up-refed for us, but we don't actually need it so free it
     * immediately.
     */
    SSL_SESSION_free(sess);
    return 1;
}

static void remove_session_cb(SSL_CTX *ctx, SSL_SESSION *sess)
{
    remove_called++;
}

static SSL_SESSION *get_sess_val = NULL;

static SSL_SESSION *get_session_cb(SSL *ssl, const unsigned char *id, int len,
                                   int *copy)
{
    get_called++;
    *copy = 1;
    return get_sess_val;
}

static int execute_test_session(int maxprot, int use_int_cache,
                                int use_ext_cache, long s_options)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl1 = NULL, *clientssl1 = NULL;
    SSL *serverssl2 = NULL, *clientssl2 = NULL;
# ifndef OPENSSL_NO_TLS1_1
    SSL *serverssl3 = NULL, *clientssl3 = NULL;
# endif
    SSL_SESSION *sess1 = NULL, *sess2 = NULL;
    int testresult = 0, numnewsesstick = 1;

    new_called = remove_called = 0;

    /* TLSv1.3 sends 2 NewSessionTickets */
    if (maxprot == TLS1_3_VERSION)
        numnewsesstick = 2;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        return 0;

    /*
     * Only allow the max protocol version so we can force a connection failure
     * later
     */
    SSL_CTX_set_min_proto_version(cctx, maxprot);
    SSL_CTX_set_max_proto_version(cctx, maxprot);

    /* Set up session cache */
    if (use_ext_cache) {
        SSL_CTX_sess_set_new_cb(cctx, new_session_cb);
        SSL_CTX_sess_set_remove_cb(cctx, remove_session_cb);
    }
    if (use_int_cache) {
        /* Also covers instance where both are set */
        SSL_CTX_set_session_cache_mode(cctx, SSL_SESS_CACHE_CLIENT);
    } else {
        SSL_CTX_set_session_cache_mode(cctx,
                                       SSL_SESS_CACHE_CLIENT
                                       | SSL_SESS_CACHE_NO_INTERNAL_STORE);
    }

    if (s_options) {
        SSL_CTX_set_options(sctx, s_options);
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl1, &clientssl1,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl1, clientssl1,
                                                SSL_ERROR_NONE))
            || !TEST_ptr(sess1 = SSL_get1_session(clientssl1)))
        goto end;

    /* Should fail because it should already be in the cache */
    if (use_int_cache && !TEST_false(SSL_CTX_add_session(cctx, sess1)))
        goto end;
    if (use_ext_cache
            && (!TEST_int_eq(new_called, numnewsesstick)

                || !TEST_int_eq(remove_called, 0)))
        goto end;

    new_called = remove_called = 0;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl2,
                                      &clientssl2, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl2, sess1))
            || !TEST_true(create_ssl_connection(serverssl2, clientssl2,
                                                SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl2)))
        goto end;

    if (maxprot == TLS1_3_VERSION) {
        /*
         * In TLSv1.3 we should have created a new session even though we have
         * resumed. Since we attempted a resume we should also have removed the
         * old ticket from the cache so that we try to only use tickets once.
         */
        if (use_ext_cache
                && (!TEST_int_eq(new_called, 1)
                    || !TEST_int_eq(remove_called, 1)))
            goto end;
    } else {
        /*
         * In TLSv1.2 we expect to have resumed so no sessions added or
         * removed.
         */
        if (use_ext_cache
                && (!TEST_int_eq(new_called, 0)
                    || !TEST_int_eq(remove_called, 0)))
            goto end;
    }

    SSL_SESSION_free(sess1);
    if (!TEST_ptr(sess1 = SSL_get1_session(clientssl2)))
        goto end;
    shutdown_ssl_connection(serverssl2, clientssl2);
    serverssl2 = clientssl2 = NULL;

    new_called = remove_called = 0;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl2,
                                      &clientssl2, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl2, clientssl2,
                                                SSL_ERROR_NONE)))
        goto end;

    if (!TEST_ptr(sess2 = SSL_get1_session(clientssl2)))
        goto end;

    if (use_ext_cache
            && (!TEST_int_eq(new_called, numnewsesstick)
                || !TEST_int_eq(remove_called, 0)))
        goto end;

    new_called = remove_called = 0;
    /*
     * This should clear sess2 from the cache because it is a "bad" session.
     * See SSL_set_session() documentation.
     */
    if (!TEST_true(SSL_set_session(clientssl2, sess1)))
        goto end;
    if (use_ext_cache
            && (!TEST_int_eq(new_called, 0) || !TEST_int_eq(remove_called, 1)))
        goto end;
    if (!TEST_ptr_eq(SSL_get_session(clientssl2), sess1))
        goto end;

    if (use_int_cache) {
        /* Should succeeded because it should not already be in the cache */
        if (!TEST_true(SSL_CTX_add_session(cctx, sess2))
                || !TEST_true(SSL_CTX_remove_session(cctx, sess2)))
            goto end;
    }

    new_called = remove_called = 0;
    /* This shouldn't be in the cache so should fail */
    if (!TEST_false(SSL_CTX_remove_session(cctx, sess2)))
        goto end;

    if (use_ext_cache
            && (!TEST_int_eq(new_called, 0) || !TEST_int_eq(remove_called, 1)))
        goto end;

# if !defined(OPENSSL_NO_TLS1_1)
    new_called = remove_called = 0;
    /* Force a connection failure */
    SSL_CTX_set_max_proto_version(sctx, TLS1_1_VERSION);
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl3,
                                      &clientssl3, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl3, sess1))
            /* This should fail because of the mismatched protocol versions */
            || !TEST_false(create_ssl_connection(serverssl3, clientssl3,
                                                 SSL_ERROR_NONE)))
        goto end;

    /* We should have automatically removed the session from the cache */
    if (use_ext_cache
            && (!TEST_int_eq(new_called, 0) || !TEST_int_eq(remove_called, 1)))
        goto end;

    /* Should succeed because it should not already be in the cache */
    if (use_int_cache && !TEST_true(SSL_CTX_add_session(cctx, sess2)))
        goto end;
# endif

    /* Now do some tests for server side caching */
    if (use_ext_cache) {
        SSL_CTX_sess_set_new_cb(cctx, NULL);
        SSL_CTX_sess_set_remove_cb(cctx, NULL);
        SSL_CTX_sess_set_new_cb(sctx, new_session_cb);
        SSL_CTX_sess_set_remove_cb(sctx, remove_session_cb);
        SSL_CTX_sess_set_get_cb(sctx, get_session_cb);
        get_sess_val = NULL;
    }

    SSL_CTX_set_session_cache_mode(cctx, 0);
    /* Internal caching is the default on the server side */
    if (!use_int_cache)
        SSL_CTX_set_session_cache_mode(sctx,
                                       SSL_SESS_CACHE_SERVER
                                       | SSL_SESS_CACHE_NO_INTERNAL_STORE);

    SSL_free(serverssl1);
    SSL_free(clientssl1);
    serverssl1 = clientssl1 = NULL;
    SSL_free(serverssl2);
    SSL_free(clientssl2);
    serverssl2 = clientssl2 = NULL;
    SSL_SESSION_free(sess1);
    sess1 = NULL;
    SSL_SESSION_free(sess2);
    sess2 = NULL;

    SSL_CTX_set_max_proto_version(sctx, maxprot);
    if (maxprot == TLS1_2_VERSION)
        SSL_CTX_set_options(sctx, SSL_OP_NO_TICKET);
    new_called = remove_called = get_called = 0;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl1, &clientssl1,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl1, clientssl1,
                                                SSL_ERROR_NONE))
            || !TEST_ptr(sess1 = SSL_get1_session(clientssl1))
            || !TEST_ptr(sess2 = SSL_get1_session(serverssl1)))
        goto end;

    if (use_int_cache) {
        if (maxprot == TLS1_3_VERSION && !use_ext_cache) {
            /*
             * In TLSv1.3 it should not have been added to the internal cache,
             * except in the case where we also have an external cache (in that
             * case it gets added to the cache in order to generate remove
             * events after timeout).
             */
            if (!TEST_false(SSL_CTX_remove_session(sctx, sess2)))
                goto end;
        } else {
            /* Should fail because it should already be in the cache */
            if (!TEST_false(SSL_CTX_add_session(sctx, sess2)))
                goto end;
        }
    }

    if (use_ext_cache) {
        SSL_SESSION *tmp = sess2;

        if (!TEST_int_eq(new_called, numnewsesstick)
                || !TEST_int_eq(remove_called, 0)
                || !TEST_int_eq(get_called, 0))
            goto end;
        /*
         * Delete the session from the internal cache to force a lookup from
         * the external cache. We take a copy first because
         * SSL_CTX_remove_session() also marks the session as non-resumable.
         */
        if (use_int_cache && maxprot != TLS1_3_VERSION) {
            if (!TEST_ptr(tmp = SSL_SESSION_dup(sess2))
                || !TEST_true(sess2->owner != NULL)
                || !TEST_true(tmp->owner == NULL)
                || !TEST_true(SSL_CTX_remove_session(sctx, sess2)))
                goto end;
            SSL_SESSION_free(sess2);
        }
        sess2 = tmp;
    }

    new_called = remove_called = get_called = 0;
    get_sess_val = sess2;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl2,
                                      &clientssl2, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl2, sess1))
            || !TEST_true(create_ssl_connection(serverssl2, clientssl2,
                                                SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl2)))
        goto end;

    if (use_ext_cache) {
        if (!TEST_int_eq(remove_called, 0))
            goto end;

        if (maxprot == TLS1_3_VERSION) {
            if (!TEST_int_eq(new_called, 1)
                    || !TEST_int_eq(get_called, 0))
                goto end;
        } else {
            if (!TEST_int_eq(new_called, 0)
                    || !TEST_int_eq(get_called, 1))
                goto end;
        }
    }
    /*
     * Make a small cache, force out all other sessions but
     * sess2, try to add sess1, which should succeed. Then
     * make sure it's there by checking the owners. Despite
     * the timeouts, sess1 should have kicked out sess2
     */

    /* Make sess1 expire before sess2 */
    if (!TEST_time_t_gt(SSL_SESSION_set_time_ex(sess1, 1000), 0)
            || !TEST_long_gt(SSL_SESSION_set_timeout(sess1, 1000), 0)
            || !TEST_time_t_gt(SSL_SESSION_set_time_ex(sess2, 2000), 0)
            || !TEST_long_gt(SSL_SESSION_set_timeout(sess2, 2000), 0))
        goto end;

    if (!TEST_long_ne(SSL_CTX_sess_set_cache_size(sctx, 1), 0))
        goto end;

    /* Don't care about results - cache should only be sess2 at end */
    SSL_CTX_add_session(sctx, sess1);
    SSL_CTX_add_session(sctx, sess2);

    /* Now add sess1, and make sure it remains, despite timeout */
    if (!TEST_true(SSL_CTX_add_session(sctx, sess1))
            || !TEST_ptr(sess1->owner)
            || !TEST_ptr_null(sess2->owner))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl1);
    SSL_free(clientssl1);
    SSL_free(serverssl2);
    SSL_free(clientssl2);
# ifndef OPENSSL_NO_TLS1_1
    SSL_free(serverssl3);
    SSL_free(clientssl3);
# endif
    SSL_SESSION_free(sess1);
    SSL_SESSION_free(sess2);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* !defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2) */

static int test_session_with_only_int_cache(void)
{
#ifndef OSSL_NO_USABLE_TLS1_3
    if (!execute_test_session(TLS1_3_VERSION, 1, 0, 0))
        return 0;
#endif

#ifndef OPENSSL_NO_TLS1_2
    return execute_test_session(TLS1_2_VERSION, 1, 0, 0);
#else
    return 1;
#endif
}

static int test_session_with_only_ext_cache(void)
{
#ifndef OSSL_NO_USABLE_TLS1_3
    if (!execute_test_session(TLS1_3_VERSION, 0, 1, 0))
        return 0;
#endif

#ifndef OPENSSL_NO_TLS1_2
    return execute_test_session(TLS1_2_VERSION, 0, 1, 0);
#else
    return 1;
#endif
}

static int test_session_with_both_cache(void)
{
#ifndef OSSL_NO_USABLE_TLS1_3
    if (!execute_test_session(TLS1_3_VERSION, 1, 1, 0))
        return 0;
#endif

#ifndef OPENSSL_NO_TLS1_2
    return execute_test_session(TLS1_2_VERSION, 1, 1, 0);
#else
    return 1;
#endif
}

static int test_session_wo_ca_names(void)
{
#ifndef OSSL_NO_USABLE_TLS1_3
    if (!execute_test_session(TLS1_3_VERSION, 1, 0, SSL_OP_DISABLE_TLSEXT_CA_NAMES))
        return 0;
#endif

#ifndef OPENSSL_NO_TLS1_2
    return execute_test_session(TLS1_2_VERSION, 1, 0, SSL_OP_DISABLE_TLSEXT_CA_NAMES);
#else
    return 1;
#endif
}

#ifndef OSSL_NO_USABLE_TLS1_3
static SSL_SESSION *sesscache[6];
static int do_cache;

static int new_cachesession_cb(SSL *ssl, SSL_SESSION *sess)
{
    if (do_cache) {
        sesscache[new_called] = sess;
    } else {
        /* We don't need the reference to the session, so free it */
        SSL_SESSION_free(sess);
    }
    new_called++;

    return 1;
}

static int post_handshake_verify(SSL *sssl, SSL *cssl)
{
    SSL_set_verify(sssl, SSL_VERIFY_PEER, NULL);
    if (!TEST_true(SSL_verify_client_post_handshake(sssl)))
        return 0;

    /* Start handshake on the server and client */
    if (!TEST_int_eq(SSL_do_handshake(sssl), 1)
            || !TEST_int_le(SSL_read(cssl, NULL, 0), 0)
            || !TEST_int_le(SSL_read(sssl, NULL, 0), 0)
            || !TEST_true(create_ssl_connection(sssl, cssl,
                                                SSL_ERROR_NONE)))
        return 0;

    return 1;
}

static int setup_ticket_test(int stateful, int idx, SSL_CTX **sctx,
                             SSL_CTX **cctx)
{
    int sess_id_ctx = 1;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       sctx, cctx, cert, privkey))
            || !TEST_true(SSL_CTX_set_num_tickets(*sctx, idx))
            || !TEST_true(SSL_CTX_set_session_id_context(*sctx,
                                                         (void *)&sess_id_ctx,
                                                         sizeof(sess_id_ctx))))
        return 0;

    if (stateful)
        SSL_CTX_set_options(*sctx, SSL_OP_NO_TICKET);

    SSL_CTX_set_session_cache_mode(*cctx, SSL_SESS_CACHE_CLIENT
                                          | SSL_SESS_CACHE_NO_INTERNAL_STORE);
    SSL_CTX_sess_set_new_cb(*cctx, new_cachesession_cb);

    return 1;
}

static int check_resumption(int idx, SSL_CTX *sctx, SSL_CTX *cctx, int succ)
{
    SSL *serverssl = NULL, *clientssl = NULL;
    int i;

    /* Test that we can resume with all the tickets we got given */
    for (i = 0; i < idx * 2; i++) {
        new_called = 0;
        if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                              &clientssl, NULL, NULL))
                || !TEST_true(SSL_set_session(clientssl, sesscache[i])))
            goto end;

        SSL_set_post_handshake_auth(clientssl, 1);

        if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                    SSL_ERROR_NONE)))
            goto end;

        /*
         * Following a successful resumption we only get 1 ticket. After a
         * failed one we should get idx tickets.
         */
        if (succ) {
            if (!TEST_true(SSL_session_reused(clientssl))
                    || !TEST_int_eq(new_called, 1))
                goto end;
        } else {
            if (!TEST_false(SSL_session_reused(clientssl))
                    || !TEST_int_eq(new_called, idx))
                goto end;
        }

        new_called = 0;
        /* After a post-handshake authentication we should get 1 new ticket */
        if (succ
                && (!post_handshake_verify(serverssl, clientssl)
                    || !TEST_int_eq(new_called, 1)))
            goto end;

        SSL_shutdown(clientssl);
        SSL_shutdown(serverssl);
        SSL_free(serverssl);
        SSL_free(clientssl);
        serverssl = clientssl = NULL;
        SSL_SESSION_free(sesscache[i]);
        sesscache[i] = NULL;
    }

    return 1;

 end:
    SSL_free(clientssl);
    SSL_free(serverssl);
    return 0;
}

static int test_tickets(int stateful, int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    size_t j;

    /* idx is the test number, but also the number of tickets we want */

    new_called = 0;
    do_cache = 1;

    if (!setup_ticket_test(stateful, idx, &sctx, &cctx))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
               /* Check we got the number of tickets we were expecting */
            || !TEST_int_eq(idx, new_called))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    clientssl = serverssl = NULL;
    sctx = cctx = NULL;

    /*
     * Now we try to resume with the tickets we previously created. The
     * resumption attempt is expected to fail (because we're now using a new
     * SSL_CTX). We should see idx number of tickets issued again.
     */

    /* Stop caching sessions - just count them */
    do_cache = 0;

    if (!setup_ticket_test(stateful, idx, &sctx, &cctx))
        goto end;

    if (!check_resumption(idx, sctx, cctx, 0))
        goto end;

    /* Start again with caching sessions */
    new_called = 0;
    do_cache = 1;
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    sctx = cctx = NULL;

    if (!setup_ticket_test(stateful, idx, &sctx, &cctx))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL)))
        goto end;

    SSL_set_post_handshake_auth(clientssl, 1);

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
               /* Check we got the number of tickets we were expecting */
            || !TEST_int_eq(idx, new_called))
        goto end;

    /* After a post-handshake authentication we should get new tickets issued */
    if (!post_handshake_verify(serverssl, clientssl)
            || !TEST_int_eq(idx * 2, new_called))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Stop caching sessions - just count them */
    do_cache = 0;

    /*
     * Check we can resume with all the tickets we created. This time around the
     * resumptions should all be successful.
     */
    if (!check_resumption(idx, sctx, cctx, 1))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    for (j = 0; j < OSSL_NELEM(sesscache); j++) {
        SSL_SESSION_free(sesscache[j]);
        sesscache[j] = NULL;
    }
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static int test_stateless_tickets(int idx)
{
    return test_tickets(0, idx);
}

static int test_stateful_tickets(int idx)
{
    return test_tickets(1, idx);
}

static int test_psk_tickets(void)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    int sess_id_ctx = 1;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, NULL, NULL))
            || !TEST_true(SSL_CTX_set_session_id_context(sctx,
                                                         (void *)&sess_id_ctx,
                                                         sizeof(sess_id_ctx))))
        goto end;

    SSL_CTX_set_session_cache_mode(cctx, SSL_SESS_CACHE_CLIENT
                                         | SSL_SESS_CACHE_NO_INTERNAL_STORE);
    SSL_CTX_set_psk_use_session_callback(cctx, use_session_cb);
    SSL_CTX_set_psk_find_session_callback(sctx, find_session_cb);
    SSL_CTX_sess_set_new_cb(cctx, new_session_cb);
    use_session_cb_cnt = 0;
    find_session_cb_cnt = 0;
    srvid = pskid;
    new_called = 0;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;
    clientpsk = serverpsk = create_a_psk(clientssl, SHA384_DIGEST_LENGTH);
    if (!TEST_ptr(clientpsk) || !TEST_true(SSL_SESSION_up_ref(clientpsk)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_int_eq(1, find_session_cb_cnt)
            || !TEST_int_eq(1, use_session_cb_cnt)
               /* We should always get 1 ticket when using external PSK */
            || !TEST_int_eq(1, new_called))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;

    return testresult;
}

static int test_extra_tickets(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    BIO *bretry = BIO_new(bio_s_always_retry());
    BIO *tmp = NULL;
    int testresult = 0;
    int stateful = 0;
    size_t nbytes;
    unsigned char c, buf[1];

    new_called = 0;
    do_cache = 1;

    if (idx >= 3) {
        idx -= 3;
        stateful = 1;
    }

    if (!TEST_ptr(bretry) || !setup_ticket_test(stateful, idx, &sctx, &cctx))
        goto end;
    SSL_CTX_sess_set_new_cb(sctx, new_session_cb);
    /* setup_ticket_test() uses new_cachesession_cb which we don't need. */
    SSL_CTX_sess_set_new_cb(cctx, new_session_cb);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL)))
        goto end;

    /*
     * Note that we have new_session_cb on both sctx and cctx, so new_called is
     * incremented by both client and server.
     */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
               /* Check we got the number of tickets we were expecting */
            || !TEST_int_eq(idx * 2, new_called)
            || !TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_int_eq(idx * 2, new_called))
        goto end;

    /* Now try a (real) write to actually send the tickets */
    c = '1';
    if (!TEST_true(SSL_write_ex(serverssl, &c, 1, &nbytes))
            || !TEST_size_t_eq(1, nbytes)
            || !TEST_int_eq(idx * 2 + 2, new_called)
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(idx * 2 + 4, new_called)
            || !TEST_int_eq(sizeof(buf), nbytes)
            || !TEST_int_eq(c, buf[0])
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes)))
        goto end;

    /* Try with only requesting one new ticket, too */
    c = '2';
    new_called = 0;
    if (!TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_write_ex(serverssl, &c, sizeof(c), &nbytes))
            || !TEST_size_t_eq(sizeof(c), nbytes)
            || !TEST_int_eq(1, new_called)
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(2, new_called)
            || !TEST_size_t_eq(sizeof(buf), nbytes)
            || !TEST_int_eq(c, buf[0]))
        goto end;

    /* Do it again but use dummy writes to drive the ticket generation */
    c = '3';
    new_called = 0;
    if (!TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_write_ex(serverssl, &c, 0, &nbytes))
            || !TEST_size_t_eq(0, nbytes)
            || !TEST_int_eq(2, new_called)
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(4, new_called))
        goto end;

    /* Once more, but with SSL_do_handshake() to drive the ticket generation */
    c = '4';
    new_called = 0;
    if (!TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_do_handshake(serverssl))
            || !TEST_int_eq(2, new_called)
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(4, new_called))
        goto end;

    /*
     * Use the always-retry BIO to exercise the logic that forces ticket
     * generation to wait until a record boundary.
     */
    c = '5';
    new_called = 0;
    tmp = SSL_get_wbio(serverssl);
    if (!TEST_ptr(tmp) || !TEST_true(BIO_up_ref(tmp))) {
        tmp = NULL;
        goto end;
    }
    SSL_set0_wbio(serverssl, bretry);
    bretry = NULL;
    if (!TEST_false(SSL_write_ex(serverssl, &c, 1, &nbytes))
            || !TEST_int_eq(SSL_get_error(serverssl, 0), SSL_ERROR_WANT_WRITE)
            || !TEST_size_t_eq(nbytes, 0))
        goto end;
    /* Restore a BIO that will let the write succeed */
    SSL_set0_wbio(serverssl, tmp);
    tmp = NULL;
    /*
     * These calls should just queue the request and not send anything
     * even if we explicitly try to hit the state machine.
     */
    if (!TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_true(SSL_new_session_ticket(serverssl))
            || !TEST_int_eq(0, new_called)
            || !TEST_true(SSL_do_handshake(serverssl))
            || !TEST_int_eq(0, new_called))
        goto end;
    /* Re-do the write; still no tickets sent */
    if (!TEST_true(SSL_write_ex(serverssl, &c, 1, &nbytes))
            || !TEST_size_t_eq(1, nbytes)
            || !TEST_int_eq(0, new_called)
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(0, new_called)
            || !TEST_int_eq(sizeof(buf), nbytes)
            || !TEST_int_eq(c, buf[0])
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes)))
        goto end;
    /* Even trying to hit the state machine now will still not send tickets */
    if (!TEST_true(SSL_do_handshake(serverssl))
            || !TEST_int_eq(0, new_called))
        goto end;
    /* Now the *next* write should send the tickets */
    c = '6';
    if (!TEST_true(SSL_write_ex(serverssl, &c, 1, &nbytes))
            || !TEST_size_t_eq(1, nbytes)
            || !TEST_int_eq(2, new_called)
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes))
            || !TEST_int_eq(4, new_called)
            || !TEST_int_eq(sizeof(buf), nbytes)
            || !TEST_int_eq(c, buf[0])
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &nbytes)))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    testresult = 1;

 end:
    BIO_free(bretry);
    BIO_free(tmp);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    clientssl = serverssl = NULL;
    sctx = cctx = NULL;
    return testresult;
}
#endif

#define USE_NULL            0
#define USE_BIO_1           1
#define USE_BIO_2           2
#define USE_DEFAULT         3

#define CONNTYPE_CONNECTION_SUCCESS  0
#define CONNTYPE_CONNECTION_FAIL     1
#define CONNTYPE_NO_CONNECTION       2

#define TOTAL_NO_CONN_SSL_SET_BIO_TESTS         (3 * 3 * 3 * 3)
#define TOTAL_CONN_SUCCESS_SSL_SET_BIO_TESTS    (2 * 2)
#if !defined(OSSL_NO_USABLE_TLS1_3) && !defined(OPENSSL_NO_TLS1_2)
# define TOTAL_CONN_FAIL_SSL_SET_BIO_TESTS       (2 * 2)
#else
# define TOTAL_CONN_FAIL_SSL_SET_BIO_TESTS       0
#endif

#define TOTAL_SSL_SET_BIO_TESTS TOTAL_NO_CONN_SSL_SET_BIO_TESTS \
                                + TOTAL_CONN_SUCCESS_SSL_SET_BIO_TESTS \
                                + TOTAL_CONN_FAIL_SSL_SET_BIO_TESTS

static void setupbio(BIO **res, BIO *bio1, BIO *bio2, int type)
{
    switch (type) {
    case USE_NULL:
        *res = NULL;
        break;
    case USE_BIO_1:
        *res = bio1;
        break;
    case USE_BIO_2:
        *res = bio2;
        break;
    }
}


/*
 * Tests calls to SSL_set_bio() under various conditions.
 *
 * For the first 3 * 3 * 3 * 3 = 81 tests we do 2 calls to SSL_set_bio() with
 * various combinations of valid BIOs or NULL being set for the rbio/wbio. We
 * then do more tests where we create a successful connection first using our
 * standard connection setup functions, and then call SSL_set_bio() with
 * various combinations of valid BIOs or NULL. We then repeat these tests
 * following a failed connection. In this last case we are looking to check that
 * SSL_set_bio() functions correctly in the case where s->bbio is not NULL.
 */
static int test_ssl_set_bio(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    BIO *bio1 = NULL;
    BIO *bio2 = NULL;
    BIO *irbio = NULL, *iwbio = NULL, *nrbio = NULL, *nwbio = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int initrbio, initwbio, newrbio, newwbio, conntype;
    int testresult = 0;

    if (idx < TOTAL_NO_CONN_SSL_SET_BIO_TESTS) {
        initrbio = idx % 3;
        idx /= 3;
        initwbio = idx % 3;
        idx /= 3;
        newrbio = idx % 3;
        idx /= 3;
        newwbio = idx % 3;
        conntype = CONNTYPE_NO_CONNECTION;
    } else {
        idx -= TOTAL_NO_CONN_SSL_SET_BIO_TESTS;
        initrbio = initwbio = USE_DEFAULT;
        newrbio = idx % 2;
        idx /= 2;
        newwbio = idx % 2;
        idx /= 2;
        conntype = idx % 2;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (conntype == CONNTYPE_CONNECTION_FAIL) {
        /*
         * We won't ever get here if either TLSv1.3 or TLSv1.2 is disabled
         * because we reduced the number of tests in the definition of
         * TOTAL_CONN_FAIL_SSL_SET_BIO_TESTS to avoid this scenario. By setting
         * mismatched protocol versions we will force a connection failure.
         */
        SSL_CTX_set_min_proto_version(sctx, TLS1_3_VERSION);
        SSL_CTX_set_max_proto_version(cctx, TLS1_2_VERSION);
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (initrbio == USE_BIO_1
            || initwbio == USE_BIO_1
            || newrbio == USE_BIO_1
            || newwbio == USE_BIO_1) {
        if (!TEST_ptr(bio1 = BIO_new(BIO_s_mem())))
            goto end;
    }

    if (initrbio == USE_BIO_2
            || initwbio == USE_BIO_2
            || newrbio == USE_BIO_2
            || newwbio == USE_BIO_2) {
        if (!TEST_ptr(bio2 = BIO_new(BIO_s_mem())))
            goto end;
    }

    if (initrbio != USE_DEFAULT) {
        setupbio(&irbio, bio1, bio2, initrbio);
        setupbio(&iwbio, bio1, bio2, initwbio);
        SSL_set_bio(clientssl, irbio, iwbio);

        /*
         * We want to maintain our own refs to these BIO, so do an up ref for
         * each BIO that will have ownership transferred in the SSL_set_bio()
         * call
         */
        if (irbio != NULL && !BIO_up_ref(irbio))
            goto end;
        if (iwbio != NULL && iwbio != irbio && !BIO_up_ref(iwbio)) {
            BIO_free(irbio);
            goto end;
        }
    }

    if (conntype != CONNTYPE_NO_CONNECTION
            && !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)
                          == (conntype == CONNTYPE_CONNECTION_SUCCESS)))
        goto end;

    setupbio(&nrbio, bio1, bio2, newrbio);
    setupbio(&nwbio, bio1, bio2, newwbio);

    /*
     * We will (maybe) transfer ownership again so do more up refs.
     * SSL_set_bio() has some really complicated ownership rules where BIOs have
     * already been set!
     */
    if (nrbio != NULL
            && nrbio != irbio
            && (nwbio != iwbio || nrbio != nwbio))
        if (!TEST_true(BIO_up_ref(nrbio)))
            goto end;
    if (nwbio != NULL
            && nwbio != nrbio
            && (nwbio != iwbio || (nwbio == iwbio && irbio == iwbio)))
        if (!TEST_true(BIO_up_ref(nwbio))) {
            if (nrbio != irbio
                    && (nwbio != iwbio || nrbio != nwbio))
                BIO_free(nrbio);
            goto end;
        }

    SSL_set_bio(clientssl, nrbio, nwbio);

    testresult = 1;

 end:
    BIO_free(bio1);
    BIO_free(bio2);

    /*
     * This test is checking that the ref counting for SSL_set_bio is correct.
     * If we get here and we did too many frees then we will fail in the above
     * functions.
     */
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

typedef enum { NO_BIO_CHANGE, CHANGE_RBIO, CHANGE_WBIO } bio_change_t;

static int execute_test_ssl_bio(int pop_ssl, bio_change_t change_bio)
{
    BIO *sslbio = NULL, *membio1 = NULL, *membio2 = NULL;
    SSL_CTX *ctx;
    SSL *ssl = NULL;
    int testresult = 0;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_method()))
            || !TEST_ptr(ssl = SSL_new(ctx))
            || !TEST_ptr(sslbio = BIO_new(BIO_f_ssl()))
            || !TEST_ptr(membio1 = BIO_new(BIO_s_mem())))
        goto end;

    BIO_set_ssl(sslbio, ssl, BIO_CLOSE);

    /*
     * If anything goes wrong here then we could leak memory.
     */
    BIO_push(sslbio, membio1);

    /* Verify changing the rbio/wbio directly does not cause leaks */
    if (change_bio != NO_BIO_CHANGE) {
        if (!TEST_ptr(membio2 = BIO_new(BIO_s_mem()))) {
            ssl = NULL;
            goto end;
        }
        if (change_bio == CHANGE_RBIO)
            SSL_set0_rbio(ssl, membio2);
        else
            SSL_set0_wbio(ssl, membio2);
    }
    ssl = NULL;

    if (pop_ssl)
        BIO_pop(sslbio);
    else
        BIO_pop(membio1);

    testresult = 1;
 end:
    BIO_free(membio1);
    BIO_free(sslbio);
    SSL_free(ssl);
    SSL_CTX_free(ctx);

    return testresult;
}

static int test_ssl_bio_pop_next_bio(void)
{
    return execute_test_ssl_bio(0, NO_BIO_CHANGE);
}

static int test_ssl_bio_pop_ssl_bio(void)
{
    return execute_test_ssl_bio(1, NO_BIO_CHANGE);
}

static int test_ssl_bio_change_rbio(void)
{
    return execute_test_ssl_bio(0, CHANGE_RBIO);
}

static int test_ssl_bio_change_wbio(void)
{
    return execute_test_ssl_bio(0, CHANGE_WBIO);
}

#if !defined(OPENSSL_NO_TLS1_2) || defined(OSSL_NO_USABLE_TLS1_3)
typedef struct {
    /* The list of sig algs */
    const int *list;
    /* The length of the list */
    size_t listlen;
    /* A sigalgs list in string format */
    const char *liststr;
    /* Whether setting the list should succeed */
    int valid;
    /* Whether creating a connection with the list should succeed */
    int connsuccess;
} sigalgs_list;

static const int validlist1[] = {NID_sha256, EVP_PKEY_RSA};
# ifndef OPENSSL_NO_EC
static const int validlist2[] = {NID_sha256, EVP_PKEY_RSA, NID_sha512, EVP_PKEY_EC};
static const int validlist3[] = {NID_sha512, EVP_PKEY_EC};
# endif
static const int invalidlist1[] = {NID_undef, EVP_PKEY_RSA};
static const int invalidlist2[] = {NID_sha256, NID_undef};
static const int invalidlist3[] = {NID_sha256, EVP_PKEY_RSA, NID_sha256};
static const int invalidlist4[] = {NID_sha256};
static const sigalgs_list testsigalgs[] = {
    {validlist1, OSSL_NELEM(validlist1), NULL, 1, 1},
# ifndef OPENSSL_NO_EC
    {validlist2, OSSL_NELEM(validlist2), NULL, 1, 1},
    {validlist3, OSSL_NELEM(validlist3), NULL, 1, 0},
# endif
    {NULL, 0, "RSA+SHA256", 1, 1},
    {NULL, 0, "RSA+SHA256:?Invalid", 1, 1},
# ifndef OPENSSL_NO_EC
    {NULL, 0, "RSA+SHA256:ECDSA+SHA512", 1, 1},
    {NULL, 0, "ECDSA+SHA512", 1, 0},
# endif
    {invalidlist1, OSSL_NELEM(invalidlist1), NULL, 0, 0},
    {invalidlist2, OSSL_NELEM(invalidlist2), NULL, 0, 0},
    {invalidlist3, OSSL_NELEM(invalidlist3), NULL, 0, 0},
    {invalidlist4, OSSL_NELEM(invalidlist4), NULL, 0, 0},
    {NULL, 0, "RSA", 0, 0},
    {NULL, 0, "SHA256", 0, 0},
    {NULL, 0, "RSA+SHA256:SHA256", 0, 0},
    {NULL, 0, "Invalid", 0, 0}
};

static int test_set_sigalgs(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    const sigalgs_list *curr;
    int testctx;

    /* Should never happen */
    if (!TEST_size_t_le((size_t)idx, OSSL_NELEM(testsigalgs) * 2))
        return 0;

    testctx = ((size_t)idx < OSSL_NELEM(testsigalgs));
    curr = testctx ? &testsigalgs[idx]
                   : &testsigalgs[idx - OSSL_NELEM(testsigalgs)];

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        return 0;

    SSL_CTX_set_max_proto_version(cctx, TLS1_2_VERSION);

    if (testctx) {
        int ret;

        if (curr->list != NULL)
            ret = SSL_CTX_set1_sigalgs(cctx, curr->list, curr->listlen);
        else
            ret = SSL_CTX_set1_sigalgs_list(cctx, curr->liststr);

        if (!ret) {
            if (curr->valid)
                TEST_info("Failure setting sigalgs in SSL_CTX (%d)\n", idx);
            else
                testresult = 1;
            goto end;
        }
        if (!curr->valid) {
            TEST_info("Not-failed setting sigalgs in SSL_CTX (%d)\n", idx);
            goto end;
        }
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!testctx) {
        int ret;

        if (curr->list != NULL)
            ret = SSL_set1_sigalgs(clientssl, curr->list, curr->listlen);
        else
            ret = SSL_set1_sigalgs_list(clientssl, curr->liststr);
        if (!ret) {
            if (curr->valid)
                TEST_info("Failure setting sigalgs in SSL (%d)\n", idx);
            else
                testresult = 1;
            goto end;
        }
        if (!curr->valid)
            goto end;
    }

    if (!TEST_int_eq(create_ssl_connection(serverssl, clientssl,
                                           SSL_ERROR_NONE),
                curr->connsuccess))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif

#ifndef OSSL_NO_USABLE_TLS1_3
static int psk_client_cb_cnt = 0;
static int psk_server_cb_cnt = 0;

static int use_session_cb(SSL *ssl, const EVP_MD *md, const unsigned char **id,
                          size_t *idlen, SSL_SESSION **sess)
{
    switch (++use_session_cb_cnt) {
    case 1:
        /* The first call should always have a NULL md */
        if (md != NULL)
            return 0;
        break;

    case 2:
        /* The second call should always have an md */
        if (md == NULL)
            return 0;
        break;

    default:
        /* We should only be called a maximum of twice */
        return 0;
    }

    if (clientpsk != NULL && !SSL_SESSION_up_ref(clientpsk))
        return 0;

    *sess = clientpsk;
    *id = (const unsigned char *)pskid;
    *idlen = strlen(pskid);

    return 1;
}

#ifndef OPENSSL_NO_PSK
static unsigned int psk_client_cb(SSL *ssl, const char *hint, char *id,
                                  unsigned int max_id_len,
                                  unsigned char *psk,
                                  unsigned int max_psk_len)
{
    unsigned int psklen = 0;

    psk_client_cb_cnt++;

    if (strlen(pskid) + 1 > max_id_len)
        return 0;

    /* We should only ever be called a maximum of twice per connection */
    if (psk_client_cb_cnt > 2)
        return 0;

    if (clientpsk == NULL)
        return 0;

    /* We'll reuse the PSK we set up for TLSv1.3 */
    if (SSL_SESSION_get_master_key(clientpsk, NULL, 0) > max_psk_len)
        return 0;
    psklen = SSL_SESSION_get_master_key(clientpsk, psk, max_psk_len);
    strncpy(id, pskid, max_id_len);

    return psklen;
}
#endif /* OPENSSL_NO_PSK */

static int find_session_cb(SSL *ssl, const unsigned char *identity,
                           size_t identity_len, SSL_SESSION **sess)
{
    find_session_cb_cnt++;

    /* We should only ever be called a maximum of twice per connection */
    if (find_session_cb_cnt > 2)
        return 0;

    if (serverpsk == NULL)
        return 0;

    /* Identity should match that set by the client */
    if (strlen(srvid) != identity_len
            || strncmp(srvid, (const char *)identity, identity_len) != 0) {
        /* No PSK found, continue but without a PSK */
        *sess = NULL;
        return 1;
    }

    if (!SSL_SESSION_up_ref(serverpsk))
        return 0;

    *sess = serverpsk;

    return 1;
}

#ifndef OPENSSL_NO_PSK
static unsigned int psk_server_cb(SSL *ssl, const char *identity,
                                  unsigned char *psk, unsigned int max_psk_len)
{
    unsigned int psklen = 0;

    psk_server_cb_cnt++;

    /* We should only ever be called a maximum of twice per connection */
    if (find_session_cb_cnt > 2)
        return 0;

    if (serverpsk == NULL)
        return 0;

    /* Identity should match that set by the client */
    if (strcmp(srvid, identity) != 0) {
        return 0;
    }

    /* We'll reuse the PSK we set up for TLSv1.3 */
    if (SSL_SESSION_get_master_key(serverpsk, NULL, 0) > max_psk_len)
        return 0;
    psklen = SSL_SESSION_get_master_key(serverpsk, psk, max_psk_len);

    return psklen;
}
#endif /* OPENSSL_NO_PSK */

#define MSG1    "Hello"
#define MSG2    "World."
#define MSG3    "This"
#define MSG4    "is"
#define MSG5    "a"
#define MSG6    "test"
#define MSG7    "message."

static int artificial_ticket_time = 0;

static int sub_session_time(SSL_SESSION *sess)
{
    OSSL_TIME tick_time;

    tick_time = ossl_time_from_time_t(SSL_SESSION_get_time_ex(sess));
    tick_time = ossl_time_subtract(tick_time, ossl_seconds2time(10));

    return SSL_SESSION_set_time_ex(sess, ossl_time_to_time_t(tick_time)) != 0;
}

static int ed_gen_cb(SSL *s, void *arg)
{
    SSL_SESSION *sess = SSL_get0_session(s);

    if (sess == NULL)
        return 0;

    /*
     * Artificially give the ticket some age. Just do it for the number of
     * tickets we've been told to do.
     */
    if (artificial_ticket_time == 0)
        return 1;
    artificial_ticket_time--;

    return sub_session_time(sess);
}

/*
 * Helper method to setup objects for early data test. Caller frees objects on
 * error.
 */
static int setupearly_data_test(SSL_CTX **cctx, SSL_CTX **sctx, SSL **clientssl,
                                SSL **serverssl, SSL_SESSION **sess, int idx,
                                size_t mdsize)
{
    int artificial = (artificial_ticket_time > 0);

    if (*sctx == NULL
            && !TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_VERSION, 0,
                                              sctx, cctx, cert, privkey)))
        return 0;

    if (artificial)
        SSL_CTX_set_session_ticket_cb(*sctx, ed_gen_cb, NULL, NULL);

    if (!TEST_true(SSL_CTX_set_max_early_data(*sctx, SSL3_RT_MAX_PLAIN_LENGTH)))
        return 0;

    if (idx == 1) {
        /* When idx == 1 we repeat the tests with read_ahead set */
        SSL_CTX_set_read_ahead(*cctx, 1);
        SSL_CTX_set_read_ahead(*sctx, 1);
    } else if (idx == 2) {
        /* When idx == 2 we are doing early_data with a PSK. Set up callbacks */
        SSL_CTX_set_psk_use_session_callback(*cctx, use_session_cb);
        SSL_CTX_set_psk_find_session_callback(*sctx, find_session_cb);
        use_session_cb_cnt = 0;
        find_session_cb_cnt = 0;
        srvid = pskid;
    }

    if (!TEST_true(create_ssl_objects(*sctx, *cctx, serverssl, clientssl,
                                      NULL, NULL)))
        return 0;

    /*
     * For one of the run throughs (doesn't matter which one), we'll try sending
     * some SNI data in the initial ClientHello. This will be ignored (because
     * there is no SNI cb set up by the server), so it should not impact
     * early_data.
     */
    if (idx == 1
            && !TEST_true(SSL_set_tlsext_host_name(*clientssl, "localhost")))
        return 0;

    if (idx == 2) {
        clientpsk = create_a_psk(*clientssl, mdsize);
        if (!TEST_ptr(clientpsk)
                   /*
                    * We just choose an arbitrary value for max_early_data which
                    * should be big enough for testing purposes.
                    */
                || !TEST_true(SSL_SESSION_set_max_early_data(clientpsk,
                                                             0x100))
                || !TEST_true(SSL_SESSION_up_ref(clientpsk))) {
            SSL_SESSION_free(clientpsk);
            clientpsk = NULL;
            return 0;
        }
        serverpsk = clientpsk;

        if (sess != NULL) {
            if (!TEST_true(SSL_SESSION_up_ref(clientpsk))) {
                SSL_SESSION_free(clientpsk);
                SSL_SESSION_free(serverpsk);
                clientpsk = serverpsk = NULL;
                return 0;
            }
            *sess = clientpsk;
        }
        return 1;
    }

    if (sess == NULL)
        return 1;

    if (!TEST_true(create_ssl_connection(*serverssl, *clientssl,
                                         SSL_ERROR_NONE)))
        return 0;

    *sess = SSL_get1_session(*clientssl);
    SSL_shutdown(*clientssl);
    SSL_shutdown(*serverssl);
    SSL_free(*serverssl);
    SSL_free(*clientssl);
    *serverssl = *clientssl = NULL;

    /*
     * Artificially give the ticket some age to match the artificial age we
     * gave it on the server side
     */
    if (artificial
            && !TEST_true(sub_session_time(*sess)))
        return 0;

    if (!TEST_true(create_ssl_objects(*sctx, *cctx, serverssl,
                                      clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(*clientssl, *sess)))
        return 0;

    return 1;
}

static int check_early_data_timeout(OSSL_TIME timer)
{
    int res = 0;

    /*
     * Early data is time sensitive. We have an approx 8 second allowance
     * between writing the early data and reading it. If we exceed that time
     * then this test will fail. This can sometimes (rarely) occur in normal CI
     * operation. We can try and detect this and just ignore the result of this
     * test if it has taken too long. We assume anything over 7 seconds is too
     * long
     */
    timer = ossl_time_subtract(ossl_time_now(), timer);
    if (ossl_time_compare(timer, ossl_seconds2time(7)) >= 0)
        res = TEST_skip("Test took too long, ignoring result");

    return res;
}

static int test_early_data_read_write(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char buf[20], data[1024];
    size_t readbytes, written, eoedlen, rawread, rawwritten;
    BIO *rbio;
    OSSL_TIME timer;

    /* Artificially give the next 2 tickets some age for non PSK sessions */
    if (idx != 2)
        artificial_ticket_time = 2;
    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, idx,
                                        SHA384_DIGEST_LENGTH))) {
        artificial_ticket_time = 0;
        goto end;
    }
    artificial_ticket_time = 0;

    /* Write and read some early data */
    timer = ossl_time_now();
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG1)))
        goto end;

    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_SUCCESS)) {
        testresult = check_early_data_timeout(timer);
        goto end;
    }

    if (!TEST_mem_eq(MSG1, readbytes, buf, strlen(MSG1))
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_ACCEPTED))
        goto end;

    /*
     * Server should be able to write data, and client should be able to
     * read it.
     */
    if (!TEST_true(SSL_write_early_data(serverssl, MSG2, strlen(MSG2),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG2))
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    /* Even after reading normal data, client should be able write early data */
    if (!TEST_true(SSL_write_early_data(clientssl, MSG3, strlen(MSG3),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG3)))
        goto end;

    /* Server should still be able read early data after writing data */
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_SUCCESS)
            || !TEST_mem_eq(buf, readbytes, MSG3, strlen(MSG3)))
        goto end;

    /* Write more data from server and read it from client */
    if (!TEST_true(SSL_write_early_data(serverssl, MSG4, strlen(MSG4),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG4))
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG4, strlen(MSG4)))
        goto end;

    /*
     * If client writes normal data it should mean writing early data is no
     * longer possible.
     */
    if (!TEST_true(SSL_write_ex(clientssl, MSG5, strlen(MSG5), &written))
            || !TEST_size_t_eq(written, strlen(MSG5))
            || !TEST_int_eq(SSL_get_early_data_status(clientssl),
                            SSL_EARLY_DATA_ACCEPTED))
        goto end;

    /*
     * At this point the client has written EndOfEarlyData, ClientFinished and
     * normal (fully protected) data. We are going to cause a delay between the
     * arrival of EndOfEarlyData and ClientFinished. We read out all the data
     * in the read BIO, and then just put back the EndOfEarlyData message.
     */
    rbio = SSL_get_rbio(serverssl);
    if (!TEST_true(BIO_read_ex(rbio, data, sizeof(data), &rawread))
            || !TEST_size_t_lt(rawread, sizeof(data))
            || !TEST_size_t_gt(rawread, SSL3_RT_HEADER_LENGTH))
        goto end;

    /* Record length is in the 4th and 5th bytes of the record header */
    eoedlen = SSL3_RT_HEADER_LENGTH + (data[3] << 8 | data[4]);
    if (!TEST_true(BIO_write_ex(rbio, data, eoedlen, &rawwritten))
            || !TEST_size_t_eq(rawwritten, eoedlen))
        goto end;

    /* Server should be told that there is no more early data */
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_FINISH)
            || !TEST_size_t_eq(readbytes, 0))
        goto end;

    /*
     * Server has not finished init yet, so should still be able to write early
     * data.
     */
    if (!TEST_true(SSL_write_early_data(serverssl, MSG6, strlen(MSG6),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG6)))
        goto end;

    /* Push the ClientFinished and the normal data back into the server rbio */
    if (!TEST_true(BIO_write_ex(rbio, data + eoedlen, rawread - eoedlen,
                                &rawwritten))
            || !TEST_size_t_eq(rawwritten, rawread - eoedlen))
        goto end;

    /* Server should be able to read normal data */
    if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_size_t_eq(readbytes, strlen(MSG5)))
        goto end;

    /* Client and server should not be able to write/read early data now */
    if (!TEST_false(SSL_write_early_data(clientssl, MSG6, strlen(MSG6),
                                         &written)))
        goto end;
    ERR_clear_error();
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_ERROR))
        goto end;
    ERR_clear_error();

    /* Client should be able to read the data sent by the server */
    if (!TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG6, strlen(MSG6)))
        goto end;

    /*
     * Make sure we process the two NewSessionTickets. These arrive
     * post-handshake. We attempt reads which we do not expect to return any
     * data.
     */
    if (!TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_false(SSL_read_ex(clientssl, buf, sizeof(buf),
                           &readbytes)))
        goto end;

    /* Server should be able to write normal data */
    if (!TEST_true(SSL_write_ex(serverssl, MSG7, strlen(MSG7), &written))
            || !TEST_size_t_eq(written, strlen(MSG7))
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG7, strlen(MSG7)))
        goto end;

    SSL_SESSION_free(sess);
    sess = SSL_get1_session(clientssl);
    use_session_cb_cnt = 0;
    find_session_cb_cnt = 0;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess)))
        goto end;

    /* Write and read some early data */
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG1))
            || !TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                                &readbytes),
                            SSL_READ_EARLY_DATA_SUCCESS)
            || !TEST_mem_eq(buf, readbytes, MSG1, strlen(MSG1)))
        goto end;

    if (!TEST_int_gt(SSL_connect(clientssl), 0)
            || !TEST_int_gt(SSL_accept(serverssl), 0))
        goto end;

    /* Client and server should not be able to write/read early data now */
    if (!TEST_false(SSL_write_early_data(clientssl, MSG6, strlen(MSG6),
                                         &written)))
        goto end;
    ERR_clear_error();
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_ERROR))
        goto end;
    ERR_clear_error();

    /* Client and server should be able to write/read normal data */
    if (!TEST_true(SSL_write_ex(clientssl, MSG5, strlen(MSG5), &written))
            || !TEST_size_t_eq(written, strlen(MSG5))
            || !TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_size_t_eq(readbytes, strlen(MSG5)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

static int allow_ed_cb_called = 0;

static int allow_early_data_cb(SSL *s, void *arg)
{
    int *usecb = (int *)arg;

    allow_ed_cb_called++;

    if (*usecb == 1)
        return 0;

    return 1;
}

/*
 * idx == 0: Standard early_data setup
 * idx == 1: early_data setup using read_ahead
 * usecb == 0: Don't use a custom early data callback
 * usecb == 1: Use a custom early data callback and reject the early data
 * usecb == 2: Use a custom early data callback and accept the early data
 * confopt == 0: Configure anti-replay directly
 * confopt == 1: Configure anti-replay using SSL_CONF
 */
static int test_early_data_replay_int(int idx, int usecb, int confopt)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    size_t readbytes, written;
    unsigned char buf[20];
    OSSL_TIME timer;

    allow_ed_cb_called = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        return 0;

    if (usecb > 0) {
        if (confopt == 0) {
            SSL_CTX_set_options(sctx, SSL_OP_NO_ANTI_REPLAY);
        } else {
            SSL_CONF_CTX *confctx = SSL_CONF_CTX_new();

            if (!TEST_ptr(confctx))
                goto end;
            SSL_CONF_CTX_set_flags(confctx, SSL_CONF_FLAG_FILE
                                            | SSL_CONF_FLAG_SERVER);
            SSL_CONF_CTX_set_ssl_ctx(confctx, sctx);
            if (!TEST_int_eq(SSL_CONF_cmd(confctx, "Options", "-AntiReplay"),
                             2)) {
                SSL_CONF_CTX_free(confctx);
                goto end;
            }
            SSL_CONF_CTX_free(confctx);
        }
        SSL_CTX_set_allow_early_data_cb(sctx, allow_early_data_cb, &usecb);
    }

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, idx,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    /*
     * The server is configured to accept early data. Create a connection to
     * "use up" the ticket
     */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl)))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess)))
        goto end;

    /* Write and read some early data */
    timer = ossl_time_now();
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG1)))
        goto end;

    if (usecb <= 1) {
        if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                             &readbytes),
                         SSL_READ_EARLY_DATA_FINISH)
                   /*
                    * The ticket was reused, so the we should have rejected the
                    * early data
                    */
                || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                                SSL_EARLY_DATA_REJECTED))
            goto end;
    } else {
        /* In this case the callback decides to accept the early data */
        if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                             &readbytes),
                         SSL_READ_EARLY_DATA_SUCCESS)) {
            testresult = check_early_data_timeout(timer);
            goto end;
        }
        if (!TEST_mem_eq(MSG1, strlen(MSG1), buf, readbytes)
                   /*
                    * Server will have sent its flight so client can now send
                    * end of early data and complete its half of the handshake
                    */
                || !TEST_int_gt(SSL_connect(clientssl), 0)
                || !TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                             &readbytes),
                                SSL_READ_EARLY_DATA_FINISH)
                || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                                SSL_EARLY_DATA_ACCEPTED))
            goto end;
    }

    /* Complete the connection */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_int_eq(SSL_session_reused(clientssl), (usecb > 0) ? 1 : 0)
            || !TEST_int_eq(allow_ed_cb_called, usecb > 0 ? 1 : 0))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

static int test_early_data_replay(int idx)
{
    int ret = 1, usecb, confopt;

    for (usecb = 0; usecb < 3; usecb++) {
        for (confopt = 0; confopt < 2; confopt++)
            ret &= test_early_data_replay_int(idx, usecb, confopt);
    }

    return ret;
}

static const char *ciphersuites[] = {
    "TLS_AES_128_CCM_8_SHA256",
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_AES_128_CCM_SHA256",
#if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
    "TLS_CHACHA20_POLY1305_SHA256",
#else
    NULL,
#endif
#if !defined(OPENSSL_NO_INTEGRITY_ONLY_CIPHERS)
    "TLS_SHA256_SHA256",
    "TLS_SHA384_SHA384"
#endif
};

/*
 * Helper function to test that a server attempting to read early data can
 * handle a connection from a client where the early data should be skipped.
 * testtype: 0 == No HRR
 * testtype: 1 == HRR
 * testtype: 2 == HRR, invalid early_data sent after HRR
 * testtype: 3 == recv_max_early_data set to 0
 */
static int early_data_skip_helper(int testtype, int cipher, int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char buf[20];
    size_t readbytes, written;

    if (is_fips && cipher >= 4)
        return 1;

    if (ciphersuites[cipher] == NULL)
        return TEST_skip("Cipher not supported");

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_VERSION, 0,
                                              &sctx, &cctx, cert, privkey)))
        goto end;

    if (cipher == 0 || cipher == 5 || cipher == 6) {
        SSL_CTX_set_security_level(sctx, 0);
        SSL_CTX_set_security_level(cctx, 0);
    }

    if (!TEST_true(SSL_CTX_set_ciphersuites(sctx, ciphersuites[cipher]))
            || !TEST_true(SSL_CTX_set_ciphersuites(cctx, ciphersuites[cipher])))
        goto end;

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, idx,
                                        (cipher == 2 || cipher == 6)
                                            ? SHA384_DIGEST_LENGTH
                                            : SHA256_DIGEST_LENGTH)))
        goto end;

    if (testtype == 1 || testtype == 2) {
        /* Force an HRR to occur */
#if defined(OPENSSL_NO_EC)
        if (!TEST_true(SSL_set1_groups_list(serverssl, "ffdhe3072")))
            goto end;
#else
        if (!TEST_true(SSL_set1_groups_list(serverssl, "P-384")))
            goto end;
#endif
    } else if (idx == 2) {
        /*
         * We force early_data rejection by ensuring the PSK identity is
         * unrecognised
         */
        srvid = "Dummy Identity";
    } else {
        /*
         * Deliberately corrupt the creation time. We take 20 seconds off the
         * time. It could be any value as long as it is not within tolerance.
         * This should mean the ticket is rejected.
         */
        if (!TEST_true(SSL_SESSION_set_time_ex(sess, time(NULL) - 20)))
            goto end;
    }

    if (testtype == 3
            && !TEST_true(SSL_set_recv_max_early_data(serverssl, 0)))
        goto end;

    /* Write some early data */
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written))
            || !TEST_size_t_eq(written, strlen(MSG1)))
        goto end;

    /* Server should reject the early data */
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_FINISH)
            || !TEST_size_t_eq(readbytes, 0)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_REJECTED))
        goto end;

    switch (testtype) {
    case 0:
        /* Nothing to do */
        break;

    case 1:
        /*
         * Finish off the handshake. We perform the same writes and reads as
         * further down but we expect them to fail due to the incomplete
         * handshake.
         */
        if (!TEST_false(SSL_write_ex(clientssl, MSG2, strlen(MSG2), &written))
                || !TEST_false(SSL_read_ex(serverssl, buf, sizeof(buf),
                               &readbytes)))
            goto end;
        break;

    case 2:
        {
            BIO *wbio = SSL_get_wbio(clientssl);
            /* A record that will appear as bad early_data */
            const unsigned char bad_early_data[] = {
                0x17, 0x03, 0x03, 0x00, 0x01, 0x00
            };

            /*
             * We force the client to attempt a write. This will fail because
             * we're still in the handshake. It will cause the second
             * ClientHello to be sent.
             */
            if (!TEST_false(SSL_write_ex(clientssl, MSG2, strlen(MSG2),
                                         &written)))
                goto end;

            /*
             * Inject some early_data after the second ClientHello. This should
             * cause the server to fail
             */
            if (!TEST_true(BIO_write_ex(wbio, bad_early_data,
                                        sizeof(bad_early_data), &written)))
                goto end;
        }
        /* FALLTHROUGH */

    case 3:
        /*
         * This client has sent more early_data than we are willing to skip
         * (case 3) or sent invalid early_data (case 2) so the connection should
         * abort.
         */
        if (!TEST_false(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
                || !TEST_int_eq(SSL_get_error(serverssl, 0), SSL_ERROR_SSL))
            goto end;

        /* Connection has failed - nothing more to do */
        testresult = 1;
        goto end;

    default:
        TEST_error("Invalid test type");
        goto end;
    }

    ERR_clear_error();
    /*
     * Should be able to send normal data despite rejection of early data. The
     * early_data should be skipped.
     */
    if (!TEST_true(SSL_write_ex(clientssl, MSG2, strlen(MSG2), &written))
            || !TEST_size_t_eq(written, strlen(MSG2))
            || !TEST_int_eq(SSL_get_early_data_status(clientssl),
                            SSL_EARLY_DATA_REJECTED)
            || !TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    /*
     * Failure to decrypt early data records should not leave spurious errors
     * on the error stack
     */
    if (!TEST_long_eq(ERR_peek_error(), 0))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_SESSION_free(sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test that a server attempting to read early data can handle a connection
 * from a client where the early data is not acceptable.
 */
static int test_early_data_skip(int idx)
{
    return early_data_skip_helper(0,
                                  idx % OSSL_NELEM(ciphersuites),
                                  idx / OSSL_NELEM(ciphersuites));
}

/*
 * Test that a server attempting to read early data can handle a connection
 * from a client where an HRR occurs.
 */
static int test_early_data_skip_hrr(int idx)
{
    return early_data_skip_helper(1,
                                  idx % OSSL_NELEM(ciphersuites),
                                  idx / OSSL_NELEM(ciphersuites));
}

/*
 * Test that a server attempting to read early data can handle a connection
 * from a client where an HRR occurs and correctly fails if early_data is sent
 * after the HRR
 */
static int test_early_data_skip_hrr_fail(int idx)
{
    return early_data_skip_helper(2,
                                  idx % OSSL_NELEM(ciphersuites),
                                  idx / OSSL_NELEM(ciphersuites));
}

/*
 * Test that a server attempting to read early data will abort if it tries to
 * skip over too much.
 */
static int test_early_data_skip_abort(int idx)
{
    return early_data_skip_helper(3,
                                  idx % OSSL_NELEM(ciphersuites),
                                  idx / OSSL_NELEM(ciphersuites));
}

/*
 * Test that a server attempting to read early data can handle a connection
 * from a client that doesn't send any.
 */
static int test_early_data_not_sent(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char buf[20];
    size_t readbytes, written;

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, idx,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    /* Write some data - should block due to handshake with server */
    SSL_set_connect_state(clientssl);
    if (!TEST_false(SSL_write_ex(clientssl, MSG1, strlen(MSG1), &written)))
        goto end;

    /* Server should detect that early data has not been sent */
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_FINISH)
            || !TEST_size_t_eq(readbytes, 0)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_NOT_SENT)
            || !TEST_int_eq(SSL_get_early_data_status(clientssl),
                            SSL_EARLY_DATA_NOT_SENT))
        goto end;

    /* Continue writing the message we started earlier */
    if (!TEST_true(SSL_write_ex(clientssl, MSG1, strlen(MSG1), &written))
            || !TEST_size_t_eq(written, strlen(MSG1))
            || !TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG1, strlen(MSG1))
            || !SSL_write_ex(serverssl, MSG2, strlen(MSG2), &written)
            || !TEST_size_t_eq(written, strlen(MSG2)))
        goto end;

    if (!TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

static const char *servalpn;

static int alpn_select_cb(SSL *ssl, const unsigned char **out,
                          unsigned char *outlen, const unsigned char *in,
                          unsigned int inlen, void *arg)
{
    unsigned int protlen = 0;
    const unsigned char *prot;

    for (prot = in; prot < in + inlen; prot += protlen) {
        protlen = *prot++;
        if (in + inlen < prot + protlen)
            return SSL_TLSEXT_ERR_NOACK;

        if (protlen == strlen(servalpn)
                && memcmp(prot, servalpn, protlen) == 0) {
            *out = prot;
            *outlen = protlen;
            return SSL_TLSEXT_ERR_OK;
        }
    }

    return SSL_TLSEXT_ERR_NOACK;
}

/* Test that a PSK can be used to send early_data */
static int test_early_data_psk(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char alpnlist[] = {
        0x08, 'g', 'o', 'o', 'd', 'a', 'l', 'p', 'n', 0x07, 'b', 'a', 'd', 'a',
        'l', 'p', 'n'
    };
#define GOODALPNLEN     9
#define BADALPNLEN      8
#define GOODALPN        (alpnlist)
#define BADALPN         (alpnlist + GOODALPNLEN)
    int err = 0;
    unsigned char buf[20];
    size_t readbytes, written;
    int readearlyres = SSL_READ_EARLY_DATA_SUCCESS, connectres = 1;
    int edstatus = SSL_EARLY_DATA_ACCEPTED;

    /* We always set this up with a final parameter of "2" for PSK */
    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, 2,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    servalpn = "goodalpn";

    /*
     * Note: There is no test for inconsistent SNI with late client detection.
     * This is because servers do not acknowledge SNI even if they are using
     * it in a resumption handshake - so it is not actually possible for a
     * client to detect a problem.
     */
    switch (idx) {
    case 0:
        /* Set inconsistent SNI (early client detection) */
        err = SSL_R_INCONSISTENT_EARLY_DATA_SNI;
        if (!TEST_true(SSL_SESSION_set1_hostname(sess, "goodhost"))
                || !TEST_true(SSL_set_tlsext_host_name(clientssl, "badhost")))
            goto end;
        break;

    case 1:
        /* Set inconsistent ALPN (early client detection) */
        err = SSL_R_INCONSISTENT_EARLY_DATA_ALPN;
        /* SSL_set_alpn_protos returns 0 for success and 1 for failure */
        if (!TEST_true(SSL_SESSION_set1_alpn_selected(sess, GOODALPN,
                                                      GOODALPNLEN))
                || !TEST_false(SSL_set_alpn_protos(clientssl, BADALPN,
                                                   BADALPNLEN)))
            goto end;
        break;

    case 2:
        /*
         * Set invalid protocol version. Technically this affects PSKs without
         * early_data too, but we test it here because it is similar to the
         * SNI/ALPN consistency tests.
         */
        err = SSL_R_BAD_PSK;
        if (!TEST_true(SSL_SESSION_set_protocol_version(sess, TLS1_2_VERSION)))
            goto end;
        break;

    case 3:
        /*
         * Set inconsistent SNI (server side). In this case the connection
         * will succeed and accept early_data. In TLSv1.3 on the server side SNI
         * is associated with each handshake - not the session. Therefore it
         * should not matter that we used a different server name last time.
         */
        SSL_SESSION_free(serverpsk);
        serverpsk = SSL_SESSION_dup(clientpsk);
        if (!TEST_ptr(serverpsk)
                || !TEST_true(SSL_SESSION_set1_hostname(serverpsk, "badhost")))
            goto end;
        /* Fall through */
    case 4:
        /* Set consistent SNI */
        if (!TEST_true(SSL_SESSION_set1_hostname(sess, "goodhost"))
                || !TEST_true(SSL_set_tlsext_host_name(clientssl, "goodhost"))
                || !TEST_true(SSL_CTX_set_tlsext_servername_callback(sctx,
                                hostname_cb)))
            goto end;
        break;

    case 5:
        /*
         * Set inconsistent ALPN (server detected). In this case the connection
         * will succeed but reject early_data.
         */
        servalpn = "badalpn";
        edstatus = SSL_EARLY_DATA_REJECTED;
        readearlyres = SSL_READ_EARLY_DATA_FINISH;
        /* Fall through */
    case 6:
        /*
         * Set consistent ALPN.
         * SSL_set_alpn_protos returns 0 for success and 1 for failure. It
         * accepts a list of protos (each one length prefixed).
         * SSL_set1_alpn_selected accepts a single protocol (not length
         * prefixed)
         */
        if (!TEST_true(SSL_SESSION_set1_alpn_selected(sess, GOODALPN + 1,
                                                      GOODALPNLEN - 1))
                || !TEST_false(SSL_set_alpn_protos(clientssl, GOODALPN,
                                                   GOODALPNLEN)))
            goto end;

        SSL_CTX_set_alpn_select_cb(sctx, alpn_select_cb, NULL);
        break;

    case 7:
        /* Set inconsistent ALPN (late client detection) */
        SSL_SESSION_free(serverpsk);
        serverpsk = SSL_SESSION_dup(clientpsk);
        if (!TEST_ptr(serverpsk)
                || !TEST_true(SSL_SESSION_set1_alpn_selected(clientpsk,
                                                             BADALPN + 1,
                                                             BADALPNLEN - 1))
                || !TEST_true(SSL_SESSION_set1_alpn_selected(serverpsk,
                                                             GOODALPN + 1,
                                                             GOODALPNLEN - 1))
                || !TEST_false(SSL_set_alpn_protos(clientssl, alpnlist,
                                                   sizeof(alpnlist))))
            goto end;
        SSL_CTX_set_alpn_select_cb(sctx, alpn_select_cb, NULL);
        edstatus = SSL_EARLY_DATA_ACCEPTED;
        readearlyres = SSL_READ_EARLY_DATA_SUCCESS;
        /* SSL_connect() call should fail */
        connectres = -1;
        break;

    default:
        TEST_error("Bad test index");
        goto end;
    }

    SSL_set_connect_state(clientssl);
    if (err != 0) {
        if (!TEST_false(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                            &written))
                || !TEST_int_eq(SSL_get_error(clientssl, 0), SSL_ERROR_SSL)
                || !TEST_int_eq(ERR_GET_REASON(ERR_get_error()), err))
            goto end;
    } else {
        OSSL_TIME timer = ossl_time_now();

        if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                            &written)))
            goto end;

        if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                             &readbytes), readearlyres)) {
            testresult = check_early_data_timeout(timer);
            goto end;
        }

        if ((readearlyres == SSL_READ_EARLY_DATA_SUCCESS
                    && !TEST_mem_eq(buf, readbytes, MSG1, strlen(MSG1)))
                || !TEST_int_eq(SSL_get_early_data_status(serverssl), edstatus)
                || !TEST_int_eq(SSL_connect(clientssl), connectres))
            goto end;
    }

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test TLSv1.3 PSK can be used to send early_data with all 7 ciphersuites
 * idx == 0: Test with TLS1_3_RFC_AES_128_GCM_SHA256
 * idx == 1: Test with TLS1_3_RFC_AES_256_GCM_SHA384
 * idx == 2: Test with TLS1_3_RFC_CHACHA20_POLY1305_SHA256,
 * idx == 3: Test with TLS1_3_RFC_AES_128_CCM_SHA256
 * idx == 4: Test with TLS1_3_RFC_AES_128_CCM_8_SHA256
 * idx == 5: Test with TLS1_3_RFC_SHA256_SHA256
 * idx == 6: Test with TLS1_3_RFC_SHA384_SHA384
 */
static int test_early_data_psk_with_all_ciphers(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char buf[20];
    size_t readbytes, written;
    const SSL_CIPHER *cipher;
    OSSL_TIME timer;
    const char *cipher_str[] = {
        TLS1_3_RFC_AES_128_GCM_SHA256,
        TLS1_3_RFC_AES_256_GCM_SHA384,
# if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
        TLS1_3_RFC_CHACHA20_POLY1305_SHA256,
# else
        NULL,
# endif
        TLS1_3_RFC_AES_128_CCM_SHA256,
        TLS1_3_RFC_AES_128_CCM_8_SHA256,
# if !defined(OPENSSL_NO_INTEGRITY_ONLY_CIPHERS)
        TLS1_3_RFC_SHA256_SHA256,
        TLS1_3_RFC_SHA384_SHA384
#else
        NULL,
        NULL
#endif
    };
    const unsigned char *cipher_bytes[] = {
        TLS13_AES_128_GCM_SHA256_BYTES,
        TLS13_AES_256_GCM_SHA384_BYTES,
# if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
        TLS13_CHACHA20_POLY1305_SHA256_BYTES,
# else
        NULL,
# endif
        TLS13_AES_128_CCM_SHA256_BYTES,
        TLS13_AES_128_CCM_8_SHA256_BYTES,
# if !defined(OPENSSL_NO_INTEGRITY_ONLY_CIPHERS)
        TLS13_SHA256_SHA256_BYTES,
        TLS13_SHA384_SHA384_BYTES
#else
        NULL,
        NULL
#endif
    };

    if (cipher_str[idx] == NULL)
        return 1;
    /*
     * Skip ChaCha20Poly1305 and TLS_SHA{256,384}_SHA{256,384} ciphers
     * as currently FIPS module does not support them.
     */
    if ((idx == 2 || idx == 5 || idx == 6) && is_fips == 1)
        return 1;

    /* We always set this up with a final parameter of "2" for PSK */
    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, 2,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    if (idx == 4 || idx == 5 || idx == 6) {
        /*
         * CCM8 ciphers are considered low security due to their short tag.
         * Integrity-only cipher do not provide any confidentiality.
         */
        SSL_set_security_level(clientssl, 0);
        SSL_set_security_level(serverssl, 0);
    }

    if (!TEST_true(SSL_set_ciphersuites(clientssl, cipher_str[idx]))
            || !TEST_true(SSL_set_ciphersuites(serverssl, cipher_str[idx])))
        goto end;

    /*
     * 'setupearly_data_test' creates only one instance of SSL_SESSION
     * and assigns to both client and server with incremented reference
     * and the same instance is updated in 'sess'.
     * So updating ciphersuite in 'sess' which will get reflected in
     * PSK handshake using psk use sess and find sess cb.
     */
    cipher = SSL_CIPHER_find(clientssl, cipher_bytes[idx]);
    if (!TEST_ptr(cipher) || !TEST_true(SSL_SESSION_set_cipher(sess, cipher)))
        goto end;

    SSL_set_connect_state(clientssl);
    timer = ossl_time_now();
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written)))
        goto end;

    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                                         SSL_READ_EARLY_DATA_SUCCESS)) {
        testresult = check_early_data_timeout(timer);
        goto end;
    }

    if (!TEST_mem_eq(buf, readbytes, MSG1, strlen(MSG1))
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                                                      SSL_EARLY_DATA_ACCEPTED)
            || !TEST_int_eq(SSL_connect(clientssl), 1)
            || !TEST_int_eq(SSL_accept(serverssl), 1))
        goto end;

    /* Send some normal data from client to server */
    if (!TEST_true(SSL_write_ex(clientssl, MSG2, strlen(MSG2), &written))
            || !TEST_size_t_eq(written, strlen(MSG2)))
        goto end;

    if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    testresult = 1;
 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    if (clientssl != NULL)
        SSL_shutdown(clientssl);
    if (serverssl != NULL)
        SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test that a server that doesn't try to read early data can handle a
 * client sending some.
 */
static int test_early_data_not_expected(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    unsigned char buf[20];
    size_t readbytes, written;

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, &sess, idx,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    /* Write some early data */
    if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                        &written)))
        goto end;

    /*
     * Server should skip over early data and then block waiting for client to
     * continue handshake
     */
    if (!TEST_int_le(SSL_accept(serverssl), 0)
     || !TEST_int_gt(SSL_connect(clientssl), 0)
     || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                     SSL_EARLY_DATA_REJECTED)
     || !TEST_int_gt(SSL_accept(serverssl), 0)
     || !TEST_int_eq(SSL_get_early_data_status(clientssl),
                     SSL_EARLY_DATA_REJECTED))
        goto end;

    /* Send some normal data from client to server */
    if (!TEST_true(SSL_write_ex(clientssl, MSG2, strlen(MSG2), &written))
            || !TEST_size_t_eq(written, strlen(MSG2)))
        goto end;

    if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}


# ifndef OPENSSL_NO_TLS1_2
/*
 * Test that a server attempting to read early data can handle a connection
 * from a TLSv1.2 client.
 */
static int test_early_data_tls1_2(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    unsigned char buf[20];
    size_t readbytes, written;

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                        &serverssl, NULL, idx,
                                        SHA384_DIGEST_LENGTH)))
        goto end;

    /* Write some data - should block due to handshake with server */
    SSL_set_max_proto_version(clientssl, TLS1_2_VERSION);
    SSL_set_connect_state(clientssl);
    if (!TEST_false(SSL_write_ex(clientssl, MSG1, strlen(MSG1), &written)))
        goto end;

    /*
     * Server should do TLSv1.2 handshake. First it will block waiting for more
     * messages from client after ServerDone. Then SSL_read_early_data should
     * finish and detect that early data has not been sent
     */
    if (!TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                         &readbytes),
                     SSL_READ_EARLY_DATA_ERROR))
        goto end;

    /*
     * Continue writing the message we started earlier. Will still block waiting
     * for the CCS/Finished from server
     */
    if (!TEST_false(SSL_write_ex(clientssl, MSG1, strlen(MSG1), &written))
            || !TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                                &readbytes),
                            SSL_READ_EARLY_DATA_FINISH)
            || !TEST_size_t_eq(readbytes, 0)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_NOT_SENT))
        goto end;

    /* Continue writing the message we started earlier */
    if (!TEST_true(SSL_write_ex(clientssl, MSG1, strlen(MSG1), &written))
            || !TEST_size_t_eq(written, strlen(MSG1))
            || !TEST_int_eq(SSL_get_early_data_status(clientssl),
                            SSL_EARLY_DATA_NOT_SENT)
            || !TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
            || !TEST_mem_eq(buf, readbytes, MSG1, strlen(MSG1))
            || !TEST_true(SSL_write_ex(serverssl, MSG2, strlen(MSG2), &written))
            || !TEST_size_t_eq(written, strlen(MSG2))
            || !SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes)
            || !TEST_mem_eq(buf, readbytes, MSG2, strlen(MSG2)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
# endif /* OPENSSL_NO_TLS1_2 */

/*
 * Test configuring the TLSv1.3 ciphersuites
 *
 * Test 0: Set a default ciphersuite in the SSL_CTX (no explicit cipher_list)
 * Test 1: Set a non-default ciphersuite in the SSL_CTX (no explicit cipher_list)
 * Test 2: Set a default ciphersuite in the SSL (no explicit cipher_list)
 * Test 3: Set a non-default ciphersuite in the SSL (no explicit cipher_list)
 * Test 4: Set a default ciphersuite in the SSL_CTX (SSL_CTX cipher_list)
 * Test 5: Set a non-default ciphersuite in the SSL_CTX (SSL_CTX cipher_list)
 * Test 6: Set a default ciphersuite in the SSL (SSL_CTX cipher_list)
 * Test 7: Set a non-default ciphersuite in the SSL (SSL_CTX cipher_list)
 * Test 8: Set a default ciphersuite in the SSL (SSL cipher_list)
 * Test 9: Set a non-default ciphersuite in the SSL (SSL cipher_list)
 */
static int test_set_ciphersuite(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(SSL_CTX_set_ciphersuites(sctx,
                           "TLS_AES_128_GCM_SHA256:TLS_AES_128_CCM_SHA256")))
        goto end;

    if (idx >=4 && idx <= 7) {
        /* SSL_CTX explicit cipher list */
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx, "AES256-GCM-SHA384")))
            goto end;
    }

    if (idx == 0 || idx == 4) {
        /* Default ciphersuite */
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                                "TLS_AES_128_GCM_SHA256")))
            goto end;
    } else if (idx == 1 || idx == 5) {
        /* Non default ciphersuite */
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                                "TLS_AES_128_CCM_SHA256")))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL)))
        goto end;

    if (idx == 8 || idx == 9) {
        /* SSL explicit cipher list */
        if (!TEST_true(SSL_set_cipher_list(clientssl, "AES256-GCM-SHA384")))
            goto end;
    }

    if (idx == 2 || idx == 6 || idx == 8) {
        /* Default ciphersuite */
        if (!TEST_true(SSL_set_ciphersuites(clientssl,
                                            "TLS_AES_128_GCM_SHA256")))
            goto end;
    } else if (idx == 3 || idx == 7 || idx == 9) {
        /* Non default ciphersuite */
        if (!TEST_true(SSL_set_ciphersuites(clientssl,
                                            "TLS_AES_128_CCM_SHA256")))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static int test_ciphersuite_change(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *clntsess = NULL;
    int testresult = 0;
    const SSL_CIPHER *aes_128_gcm_sha256 = NULL;

    /* Create a session based on SHA-256 */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(SSL_CTX_set_ciphersuites(sctx,
                                                   "TLS_AES_128_GCM_SHA256:"
                                                   "TLS_AES_256_GCM_SHA384:"
                                                   "TLS_AES_128_CCM_SHA256"))
            || !TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                                   "TLS_AES_128_GCM_SHA256")))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    clntsess = SSL_get1_session(clientssl);
    /* Save for later */
    aes_128_gcm_sha256 = SSL_SESSION_get0_cipher(clntsess);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Check we can resume a session with a different SHA-256 ciphersuite */
    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                            "TLS_AES_128_CCM_SHA256"))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                             &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, clntsess))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl)))
        goto end;

    SSL_SESSION_free(clntsess);
    clntsess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /*
     * Check attempting to resume a SHA-256 session with no SHA-256 ciphersuites
     * succeeds but does not resume.
     */
    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx, "TLS_AES_256_GCM_SHA384"))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, clntsess))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_SSL))
            || !TEST_false(SSL_session_reused(clientssl)))
        goto end;

    SSL_SESSION_free(clntsess);
    clntsess = NULL;
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Create a session based on SHA384 */
    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx, "TLS_AES_256_GCM_SHA384"))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    clntsess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                   "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384"))
            || !TEST_true(SSL_CTX_set_ciphersuites(sctx,
                                                   "TLS_AES_256_GCM_SHA384"))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, clntsess))
               /*
                * We use SSL_ERROR_WANT_READ below so that we can pause the
                * connection after the initial ClientHello has been sent to
                * enable us to make some session changes.
                */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_WANT_READ)))
        goto end;

    /* Trick the client into thinking this session is for a different digest */
    clntsess->cipher = aes_128_gcm_sha256;
    clntsess->cipher_id = clntsess->cipher->id;

    /*
     * Continue the previously started connection. Server has selected a SHA-384
     * ciphersuite, but client thinks the session is for SHA-256, so it should
     * bail out.
     */
    if (!TEST_false(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_SSL))
            || !TEST_int_eq(ERR_GET_REASON(ERR_get_error()),
                            SSL_R_CIPHERSUITE_DIGEST_HAS_CHANGED))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(clntsess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test TLSv1.3 Key exchange
 * Test 0 = Test all ECDHE Key exchange with TLSv1.3 client and server
 * Test 1 = Test NID_X9_62_prime256v1 with TLSv1.3 client and server
 * Test 2 = Test NID_secp384r1 with TLSv1.3 client and server
 * Test 3 = Test NID_secp521r1 with TLSv1.3 client and server
 * Test 4 = Test NID_X25519 with TLSv1.3 client and server
 * Test 5 = Test NID_X448 with TLSv1.3 client and server
 * Test 6 = Test all FFDHE Key exchange with TLSv1.3 client and server
 * Test 7 = Test NID_ffdhe2048 with TLSv1.3 client and server
 * Test 8 = Test NID_ffdhe3072 with TLSv1.3 client and server
 * Test 9 = Test NID_ffdhe4096 with TLSv1.3 client and server
 * Test 10 = Test NID_ffdhe6144 with TLSv1.3 client and server
 * Test 11 = Test NID_ffdhe8192 with TLSv1.3 client and server
 * Test 12 = Test all ML-KEM with TLSv1.3 client and server
 * Test 13 = Test MLKEM512
 * Test 14 = Test MLKEM768
 * Test 15 = Test MLKEM1024
 * Test 16 = Test X25519MLKEM768
 * Test 17 = Test SecP256r1MLKEM768
 * Test 18 = Test SecP384r1MLKEM1024
 * Test 19 = Test all ML-KEM with TLSv1.2 client and server
 * Test 20 = Test all FFDHE with TLSv1.2 client and server
 * Test 21 = Test all ECDHE with TLSv1.2 client and server
 */
# ifndef OPENSSL_NO_EC
static int ecdhe_kexch_groups[] = {NID_X9_62_prime256v1, NID_secp384r1,
                                   NID_secp521r1,
#  ifndef OPENSSL_NO_ECX
                                   NID_X25519, NID_X448
#  endif
                                   };
# endif
# ifndef OPENSSL_NO_DH
static int ffdhe_kexch_groups[] = {NID_ffdhe2048, NID_ffdhe3072, NID_ffdhe4096,
                                   NID_ffdhe6144, NID_ffdhe8192};
# endif
static int test_key_exchange(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    int kexch_alg = NID_undef;
    int *kexch_groups = &kexch_alg;
    int kexch_groups_size = 1;
    int max_version = TLS1_3_VERSION;
    char *kexch_name0 = NULL;
    const char *kexch_names = NULL;
    int shared_group0;

    switch (idx) {
# ifndef OPENSSL_NO_EC
# ifndef OPENSSL_NO_TLS1_2
        case 21:
            max_version = TLS1_2_VERSION;
# endif
            /* Fall through */
        case 0:
            kexch_groups = ecdhe_kexch_groups;
            kexch_groups_size = OSSL_NELEM(ecdhe_kexch_groups);
            kexch_name0 = "secp256r1";
            break;
        case 1:
            kexch_alg = NID_X9_62_prime256v1;
            kexch_name0 = "secp256r1";
            break;
        case 2:
            kexch_alg = NID_secp384r1;
            kexch_name0 = "secp384r1";
            break;
        case 3:
            kexch_alg = NID_secp521r1;
            kexch_name0 = "secp521r1";
            break;
#  ifndef OPENSSL_NO_ECX
        case 4:
            if (is_fips)
                return TEST_skip("X25519 might not be supported by fips provider.");
            kexch_alg = NID_X25519;
            kexch_name0 = "x25519";
            break;
        case 5:
            if (is_fips)
                return TEST_skip("X448 might not be supported by fips provider.");
            kexch_alg = NID_X448;
            kexch_name0 = "x448";
            break;
#  endif
# endif
# ifndef OPENSSL_NO_DH
# ifndef OPENSSL_NO_TLS1_2
        case 20:
            max_version = TLS1_2_VERSION;
            kexch_name0 = "ffdhe2048";
# endif
            /* Fall through */
        case 6:
            kexch_groups = ffdhe_kexch_groups;
            kexch_groups_size = OSSL_NELEM(ffdhe_kexch_groups);
            kexch_name0 = "ffdhe2048";
            break;
        case 7:
            kexch_alg = NID_ffdhe2048;
            kexch_name0 = "ffdhe2048";
            break;
        case 8:
            kexch_alg = NID_ffdhe3072;
            kexch_name0 = "ffdhe3072";
            break;
        case 9:
            kexch_alg = NID_ffdhe4096;
            kexch_name0 = "ffdhe4096";
            break;
        case 10:
            kexch_alg = NID_ffdhe6144;
            kexch_name0 = "ffdhe6144";
            break;
        case 11:
            kexch_alg = NID_ffdhe8192;
            kexch_name0 = "ffdhe8192";
            break;
# endif
# ifndef OPENSSL_NO_ML_KEM
#  if !defined(OPENSSL_NO_TLS1_2)
        case 19:
            max_version = TLS1_2_VERSION;
#   if !defined(OPENSSL_NO_EC)
            /* Set at least one EC group so the handshake completes */
            kexch_names = "MLKEM512:MLKEM768:MLKEM1024:secp256r1";
#   elif !defined(OPENSSL_NO_DH)
            kexch_names = "MLKEM512:MLKEM768:MLKEM1024";
#   else
            /* With neither EC nor DH TLS 1.2 can't happen */
            return 1;
#   endif
#  endif
            /* Fall through */
        case 12:
            kexch_groups = NULL;
            if (kexch_names == NULL)
                kexch_names = "MLKEM512:MLKEM768:MLKEM1024";
            kexch_name0 = "MLKEM512";
            break;
        case 13:
            kexch_groups = NULL;
            kexch_name0 = "MLKEM512";
            kexch_names = kexch_name0;
            break;
        case 14:
            kexch_groups = NULL;
            kexch_name0 = "MLKEM768";
            kexch_names = kexch_name0;
            break;
        case 15:
            kexch_groups = NULL;
            kexch_name0 = "MLKEM1024";
            kexch_names = kexch_name0;
            break;
#  ifndef OPENSSL_NO_EC
#   ifndef OPENSSL_NO_ECX
        case 16:
            kexch_groups = NULL;
            kexch_name0 = "X25519MLKEM768";
            kexch_names = kexch_name0;
            break;
#   endif
        case 17:
            kexch_groups = NULL;
            kexch_name0 = "SecP256r1MLKEM768";
            kexch_names = kexch_name0;
            break;
        case 18:
            kexch_groups = NULL;
            kexch_name0 = "SecP384r1MLKEM1024";
            kexch_names = kexch_name0;
            break;
#  endif
# endif
        default:
            /* We're skipping this test */
            return 1;
    }

    if (is_fips && fips_provider_version_lt(libctx, 3, 5, 0)
            && idx >= 12 && idx <= 19)
        return TEST_skip("ML-KEM not supported in this version of fips provider");

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION,
                                       max_version, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    if (!TEST_true(SSL_CTX_set_ciphersuites(sctx,
                   TLS1_3_RFC_AES_128_GCM_SHA256)))
        goto end;

    if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                   TLS1_3_RFC_AES_128_GCM_SHA256)))
        goto end;

    if (!TEST_true(SSL_CTX_set_cipher_list(sctx,
                   TLS1_TXT_ECDHE_RSA_WITH_AES_128_GCM_SHA256 ":"
                   TLS1_TXT_DHE_RSA_WITH_AES_128_GCM_SHA256))
            || !TEST_true(SSL_CTX_set_dh_auto(sctx, 1)))
        goto end;

    /*
     * Must include an EC ciphersuite so that we send supported groups in
     * TLSv1.2
     */
# ifndef OPENSSL_NO_TLS1_2
    if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                   TLS1_TXT_ECDHE_RSA_WITH_AES_128_GCM_SHA256 ":"
                   TLS1_TXT_DHE_RSA_WITH_AES_128_GCM_SHA256)))
        goto end;
# endif

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    if (kexch_groups != NULL) {
        if (!TEST_true(SSL_set1_groups(serverssl, kexch_groups, kexch_groups_size))
            || !TEST_true(SSL_set1_groups(clientssl, kexch_groups, kexch_groups_size)))
            goto end;
    } else {
        if (!TEST_true(SSL_set1_groups_list(serverssl, kexch_names))
            || !TEST_true(SSL_set1_groups_list(clientssl, kexch_names)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /*
     * If the handshake succeeds the negotiated kexch alg should be the first
     * one in configured, except in the case of "all" FFDHE and "all" ML-KEM
     * groups (idx == 19, 20), which are TLSv1.3 only so we expect no shared
     * group to exist.
     */
    shared_group0 = SSL_get_shared_group(serverssl, 0);
    switch (idx) {
    case 19:
# if !defined(OPENSSL_NO_EC)
        /* MLKEM + TLS 1.2 and no DH => "secp526r1" */
        if (!TEST_int_eq(shared_group0, NID_X9_62_prime256v1))
            goto end;
        break;
# endif
        /* Fall through */
    case 20:
        if (!TEST_int_eq(shared_group0, 0))
            goto end;
        break;
    default:
        if (kexch_groups != NULL
            && !TEST_int_eq(shared_group0, kexch_groups[0]))
            goto end;
        if (!TEST_str_eq(SSL_group_to_name(serverssl, shared_group0),
                         kexch_name0))
            goto end;
        if (!TEST_str_eq(SSL_get0_group_name(serverssl), kexch_name0)
            || !TEST_str_eq(SSL_get0_group_name(clientssl), kexch_name0))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_group(serverssl), shared_group0))
            goto end;
        if (!TEST_int_eq(SSL_get_negotiated_group(clientssl), shared_group0))
            goto end;
        break;
    }

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

# if !defined(OPENSSL_NO_TLS1_2) \
     && !defined(OPENSSL_NO_EC)  \
     && !defined(OPENSSL_NO_DH)
static int set_ssl_groups(SSL *serverssl, SSL *clientssl, int clientmulti,
                          int isecdhe, int idx)
{
    int kexch_alg;
    int *kexch_groups = &kexch_alg;
    int numec, numff;

    numec = OSSL_NELEM(ecdhe_kexch_groups);
    numff = OSSL_NELEM(ffdhe_kexch_groups);
    if (isecdhe)
        kexch_alg = ecdhe_kexch_groups[idx];
    else
        kexch_alg = ffdhe_kexch_groups[idx];

    if (clientmulti) {
        if (!TEST_true(SSL_set1_groups(serverssl, kexch_groups, 1)))
            return 0;
        if (isecdhe) {
            if (!TEST_true(SSL_set1_groups(clientssl, ecdhe_kexch_groups,
                                           numec)))
                return 0;
        } else {
            if (!TEST_true(SSL_set1_groups(clientssl, ffdhe_kexch_groups,
                                           numff)))
                return 0;
        }
    } else {
        if (!TEST_true(SSL_set1_groups(clientssl, kexch_groups, 1)))
            return 0;
        if (isecdhe) {
            if (!TEST_true(SSL_set1_groups(serverssl, ecdhe_kexch_groups,
                                           numec)))
                return 0;
        } else {
            if (!TEST_true(SSL_set1_groups(serverssl, ffdhe_kexch_groups,
                                           numff)))
                return 0;
        }
    }
    return 1;
}

/*-
 * Test the SSL_get_negotiated_group() API across a battery of scenarios.
 * Run through both the ECDHE and FFDHE group lists used in the previous
 * test, for both TLS 1.2 and TLS 1.3, negotiating each group in turn,
 * confirming the expected result; then perform a resumption handshake
 * while offering the same group list, and another resumption handshake
 * offering a different group list.  The returned value should be the
 * negotiated group for the initial handshake; for TLS 1.3 resumption
 * handshakes the returned value will be negotiated on the resumption
 * handshake itself, but for TLS 1.2 resumption handshakes the value will
 * be cached in the session from the original handshake, regardless of what
 * was offered in the resumption ClientHello.
 *
 * Using E for the number of EC groups and F for the number of FF groups:
 * E tests of ECDHE with TLS 1.3, server only has one group
 * F tests of FFDHE with TLS 1.3, server only has one group
 * E tests of ECDHE with TLS 1.2, server only has one group
 * F tests of FFDHE with TLS 1.2, server only has one group
 * E tests of ECDHE with TLS 1.3, client sends only one group
 * F tests of FFDHE with TLS 1.3, client sends only one group
 * E tests of ECDHE with TLS 1.2, client sends only one group
 * F tests of FFDHE with TLS 1.2, client sends only one group
 */
static int test_negotiated_group(int idx)
{
    int clientmulti, istls13, isecdhe, numec, numff, numgroups;
    int expectednid;
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    SSL_SESSION *origsess = NULL;
    int testresult = 0;
    int kexch_alg;
    int max_version = TLS1_3_VERSION;

    numec = OSSL_NELEM(ecdhe_kexch_groups);
    numff = OSSL_NELEM(ffdhe_kexch_groups);
    numgroups = numec + numff;
    clientmulti = (idx < 2 * numgroups);
    idx = idx % (2 * numgroups);
    istls13 = (idx < numgroups);
    idx = idx % numgroups;
    isecdhe = (idx < numec);
    if (!isecdhe)
        idx -= numec;
    /* Now 'idx' is an index into ecdhe_kexch_groups or ffdhe_kexch_groups */
    if (isecdhe)
        kexch_alg = ecdhe_kexch_groups[idx];
    else
        kexch_alg = ffdhe_kexch_groups[idx];
    /* We expect nothing for the unimplemented TLS 1.2 FFDHE named groups */
    if (!istls13 && !isecdhe)
        expectednid = NID_undef;
    else
        expectednid = kexch_alg;

    if (is_fips && (kexch_alg == NID_X25519 || kexch_alg == NID_X448))
        return TEST_skip("X25519 and X448 might not be available in fips provider.");

    if (!istls13)
        max_version = TLS1_2_VERSION;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION,
                                       max_version, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    /*
     * Force (EC)DHE ciphers for TLS 1.2.
     * Be sure to enable auto tmp DH so that FFDHE can succeed.
     */
    if (!TEST_true(SSL_CTX_set_cipher_list(sctx,
                   TLS1_TXT_ECDHE_RSA_WITH_AES_128_GCM_SHA256 ":"
                   TLS1_TXT_DHE_RSA_WITH_AES_128_GCM_SHA256))
            || !TEST_true(SSL_CTX_set_dh_auto(sctx, 1)))
        goto end;
    if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                   TLS1_TXT_ECDHE_RSA_WITH_AES_128_GCM_SHA256 ":"
                   TLS1_TXT_DHE_RSA_WITH_AES_128_GCM_SHA256)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    if (!TEST_true(set_ssl_groups(serverssl, clientssl, clientmulti, isecdhe,
                                  idx)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* Initial handshake; always the configured one */
    if (!TEST_uint_eq(SSL_get_negotiated_group(clientssl), expectednid)
            || !TEST_uint_eq(SSL_get_negotiated_group(serverssl), expectednid))
        goto end;

    if (!TEST_ptr((origsess = SSL_get1_session(clientssl))))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* First resumption attempt; use the same config as initial handshake */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, origsess))
            || !TEST_true(set_ssl_groups(serverssl, clientssl, clientmulti,
                                         isecdhe, idx)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl)))
        goto end;

    /* Still had better agree, since nothing changed... */
    if (!TEST_uint_eq(SSL_get_negotiated_group(clientssl), expectednid)
            || !TEST_uint_eq(SSL_get_negotiated_group(serverssl), expectednid))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /*-
     * Second resumption attempt
     * The party that picks one group changes it, which we effectuate by
     * changing 'idx' and updating what we expect.
     */
    if (idx == 0)
        idx = 1;
    else
        idx--;
    if (istls13) {
        if (isecdhe)
            expectednid = ecdhe_kexch_groups[idx];
        else
            expectednid = ffdhe_kexch_groups[idx];
        /* Verify that we are changing what we expect. */
        if (!TEST_int_ne(expectednid, kexch_alg))
            goto end;
    } else {
        /* TLS 1.2 only supports named groups for ECDHE. */
        if (isecdhe)
            expectednid = kexch_alg;
        else
            expectednid = 0;
    }
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, origsess))
            || !TEST_true(set_ssl_groups(serverssl, clientssl, clientmulti,
                                         isecdhe, idx)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl)))
        goto end;

    /* Check that we get what we expected */
    if (!TEST_uint_eq(SSL_get_negotiated_group(clientssl), expectednid)
            || !TEST_uint_eq(SSL_get_negotiated_group(serverssl), expectednid))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(origsess);
    return testresult;
}
# endif /* !defined(OPENSSL_NO_EC) && !defined(OPENSSL_NO_DH) */

/*
 * Test TLSv1.3 Cipher Suite
 * Test 0 = Set TLS1.3 cipher on context
 * Test 1 = Set TLS1.3 cipher on SSL
 * Test 2 = Set TLS1.3 and TLS1.2 cipher on context
 * Test 3 = Set TLS1.3 and TLS1.2 cipher on SSL
 */
static int test_tls13_ciphersuite(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    static const struct {
        const char *ciphername;
        int fipscapable;
        int low_security;
    } t13_ciphers[] = {
        { TLS1_3_RFC_AES_128_GCM_SHA256, 1, 0 },
        { TLS1_3_RFC_AES_256_GCM_SHA384, 1, 0 },
        { TLS1_3_RFC_AES_128_CCM_SHA256, 1, 0 },
# if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
        { TLS1_3_RFC_CHACHA20_POLY1305_SHA256, 0, 0 },
        { TLS1_3_RFC_AES_256_GCM_SHA384
          ":" TLS1_3_RFC_CHACHA20_POLY1305_SHA256, 0, 0 },
# endif
        /* CCM8 ciphers are considered low security due to their short tag */
        { TLS1_3_RFC_AES_128_CCM_8_SHA256
          ":" TLS1_3_RFC_AES_128_CCM_SHA256, 1, 1 },
# if !defined(OPENSSL_NO_INTEGRITY_ONLY_CIPHERS)
        /* Integrity-only cipher do not provide any confidentiality */
        { TLS1_3_RFC_SHA256_SHA256, 0, 1 },
        { TLS1_3_RFC_SHA384_SHA384, 0, 1 }
# endif
    };
    const char *t13_cipher = NULL;
    const char *t12_cipher = NULL;
    const char *negotiated_scipher;
    const char *negotiated_ccipher;
    int set_at_ctx = 0;
    int set_at_ssl = 0;
    int testresult = 0;
    int max_ver;
    size_t i;

    switch (idx) {
        case 0:
            set_at_ctx = 1;
            break;
        case 1:
            set_at_ssl = 1;
            break;
        case 2:
            set_at_ctx = 1;
            t12_cipher = TLS1_TXT_RSA_WITH_AES_128_SHA256;
            break;
        case 3:
            set_at_ssl = 1;
            t12_cipher = TLS1_TXT_RSA_WITH_AES_128_SHA256;
            break;
    }

    for (max_ver = TLS1_2_VERSION; max_ver <= TLS1_3_VERSION; max_ver++) {
# ifdef OPENSSL_NO_TLS1_2
        if (max_ver == TLS1_2_VERSION)
            continue;
# endif
        for (i = 0; i < OSSL_NELEM(t13_ciphers); i++) {
            if (is_fips && !t13_ciphers[i].fipscapable)
                continue;
            t13_cipher = t13_ciphers[i].ciphername;
            if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                               TLS_client_method(),
                                               TLS1_VERSION, max_ver,
                                               &sctx, &cctx, cert, privkey)))
                goto end;

            if (t13_ciphers[i].low_security) {
                SSL_CTX_set_security_level(sctx, 0);
                SSL_CTX_set_security_level(cctx, 0);
            }

            if (set_at_ctx) {
                if (!TEST_true(SSL_CTX_set_ciphersuites(sctx, t13_cipher))
                    || !TEST_true(SSL_CTX_set_ciphersuites(cctx, t13_cipher)))
                    goto end;
                if (t12_cipher != NULL) {
                    if (!TEST_true(SSL_CTX_set_cipher_list(sctx, t12_cipher))
                        || !TEST_true(SSL_CTX_set_cipher_list(cctx,
                                                              t12_cipher)))
                        goto end;
                }
            }

            if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                              &clientssl, NULL, NULL)))
                goto end;

            if (set_at_ssl) {
                if (!TEST_true(SSL_set_ciphersuites(serverssl, t13_cipher))
                    || !TEST_true(SSL_set_ciphersuites(clientssl, t13_cipher)))
                    goto end;
                if (t12_cipher != NULL) {
                    if (!TEST_true(SSL_set_cipher_list(serverssl, t12_cipher))
                        || !TEST_true(SSL_set_cipher_list(clientssl,
                                                          t12_cipher)))
                        goto end;
                }
            }

            if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                                 SSL_ERROR_NONE)))
                goto end;

            negotiated_scipher = SSL_CIPHER_get_name(SSL_get_current_cipher(
                                                                 serverssl));
            negotiated_ccipher = SSL_CIPHER_get_name(SSL_get_current_cipher(
                                                                 clientssl));
            if (!TEST_str_eq(negotiated_scipher, negotiated_ccipher))
                goto end;

            /*
             * TEST_strn_eq is used below because t13_cipher can contain
             * multiple ciphersuites
             */
            if (max_ver == TLS1_3_VERSION
                && !TEST_strn_eq(t13_cipher, negotiated_scipher,
                                 strlen(negotiated_scipher)))
                goto end;

# ifndef OPENSSL_NO_TLS1_2
            /* Below validation is not done when t12_cipher is NULL */
            if (max_ver == TLS1_2_VERSION && t12_cipher != NULL
                && !TEST_str_eq(t12_cipher, negotiated_scipher))
                goto end;
# endif

            SSL_free(serverssl);
            serverssl = NULL;
            SSL_free(clientssl);
            clientssl = NULL;
            SSL_CTX_free(sctx);
            sctx = NULL;
            SSL_CTX_free(cctx);
            cctx = NULL;
        }
    }

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test TLSv1.3 PSKs
 * Test 0 = Test new style callbacks
 * Test 1 = Test both new and old style callbacks
 * Test 2 = Test old style callbacks
 * Test 3 = Test old style callbacks with no certificate
 */
static int test_tls13_psk(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    const SSL_CIPHER *cipher = NULL;
    const unsigned char key[] = {
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23,
        0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f
    };
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, idx == 3 ? NULL : cert,
                                       idx == 3 ? NULL : privkey)))
        goto end;

    if (idx != 3) {
        /*
         * We use a ciphersuite with SHA256 to ease testing old style PSK
         * callbacks which will always default to SHA256. This should not be
         * necessary if we have no cert/priv key. In that case the server should
         * prefer SHA256 automatically.
         */
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                                "TLS_AES_128_GCM_SHA256")))
            goto end;
    } else {
        /*
         * As noted above the server should prefer SHA256 automatically. However
         * we are careful not to offer TLS_CHACHA20_POLY1305_SHA256 so this same
         * code works even if we are testing with only the FIPS provider loaded.
         */
        if (!TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                                "TLS_AES_256_GCM_SHA384:"
                                                "TLS_AES_128_GCM_SHA256")))
            goto end;
    }

    /*
     * Test 0: New style callbacks only
     * Test 1: New and old style callbacks (only the new ones should be used)
     * Test 2: Old style callbacks only
     */
    if (idx == 0 || idx == 1) {
        SSL_CTX_set_psk_use_session_callback(cctx, use_session_cb);
        SSL_CTX_set_psk_find_session_callback(sctx, find_session_cb);
    }
#ifndef OPENSSL_NO_PSK
    if (idx >= 1) {
        SSL_CTX_set_psk_client_callback(cctx, psk_client_cb);
        SSL_CTX_set_psk_server_callback(sctx, psk_server_cb);
    }
#endif
    srvid = pskid;
    use_session_cb_cnt = 0;
    find_session_cb_cnt = 0;
    psk_client_cb_cnt = 0;
    psk_server_cb_cnt = 0;

    if (idx != 3) {
        /*
         * Check we can create a connection if callback decides not to send a
         * PSK
         */
        if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                                 NULL, NULL))
                || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                    SSL_ERROR_NONE))
                || !TEST_false(SSL_session_reused(clientssl))
                || !TEST_false(SSL_session_reused(serverssl)))
            goto end;

        if (idx == 0 || idx == 1) {
            if (!TEST_true(use_session_cb_cnt == 1)
                    || !TEST_true(find_session_cb_cnt == 0)
                       /*
                        * If no old style callback then below should be 0
                        * otherwise 1
                        */
                    || !TEST_true(psk_client_cb_cnt == idx)
                    || !TEST_true(psk_server_cb_cnt == 0))
                goto end;
        } else {
            if (!TEST_true(use_session_cb_cnt == 0)
                    || !TEST_true(find_session_cb_cnt == 0)
                    || !TEST_true(psk_client_cb_cnt == 1)
                    || !TEST_true(psk_server_cb_cnt == 0))
                goto end;
        }

        shutdown_ssl_connection(serverssl, clientssl);
        serverssl = clientssl = NULL;
        use_session_cb_cnt = psk_client_cb_cnt = 0;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    /* Create the PSK */
    cipher = SSL_CIPHER_find(clientssl, TLS13_AES_128_GCM_SHA256_BYTES);
    clientpsk = SSL_SESSION_new();
    if (!TEST_ptr(clientpsk)
            || !TEST_ptr(cipher)
            || !TEST_true(SSL_SESSION_set1_master_key(clientpsk, key,
                                                      sizeof(key)))
            || !TEST_true(SSL_SESSION_set_cipher(clientpsk, cipher))
            || !TEST_true(SSL_SESSION_set_protocol_version(clientpsk,
                                                           TLS1_3_VERSION))
            || !TEST_true(SSL_SESSION_up_ref(clientpsk)))
        goto end;
    serverpsk = clientpsk;

    /* Check we can create a connection and the PSK is used */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl))
            || !TEST_true(SSL_session_reused(serverssl)))
        goto end;

    if (idx == 0 || idx == 1) {
        if (!TEST_true(use_session_cb_cnt == 1)
                || !TEST_true(find_session_cb_cnt == 1)
                || !TEST_true(psk_client_cb_cnt == 0)
                || !TEST_true(psk_server_cb_cnt == 0))
            goto end;
    } else {
        if (!TEST_true(use_session_cb_cnt == 0)
                || !TEST_true(find_session_cb_cnt == 0)
                || !TEST_true(psk_client_cb_cnt == 1)
                || !TEST_true(psk_server_cb_cnt == 1))
            goto end;
    }

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;
    use_session_cb_cnt = find_session_cb_cnt = 0;
    psk_client_cb_cnt = psk_server_cb_cnt = 0;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    /* Force an HRR */
#if defined(OPENSSL_NO_EC)
    if (!TEST_true(SSL_set1_groups_list(serverssl, "ffdhe3072")))
        goto end;
#else
    if (!TEST_true(SSL_set1_groups_list(serverssl, "P-384")))
        goto end;
#endif

    /*
     * Check we can create a connection, the PSK is used and the callbacks are
     * called twice.
     */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl))
            || !TEST_true(SSL_session_reused(serverssl)))
        goto end;

    if (idx == 0 || idx == 1) {
        if (!TEST_true(use_session_cb_cnt == 2)
                || !TEST_true(find_session_cb_cnt == 2)
                || !TEST_true(psk_client_cb_cnt == 0)
                || !TEST_true(psk_server_cb_cnt == 0))
            goto end;
    } else {
        if (!TEST_true(use_session_cb_cnt == 0)
                || !TEST_true(find_session_cb_cnt == 0)
                || !TEST_true(psk_client_cb_cnt == 2)
                || !TEST_true(psk_server_cb_cnt == 2))
            goto end;
    }

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;
    use_session_cb_cnt = find_session_cb_cnt = 0;
    psk_client_cb_cnt = psk_server_cb_cnt = 0;

    if (idx != 3) {
        /*
         * Check that if the server rejects the PSK we can still connect, but with
         * a full handshake
         */
        srvid = "Dummy Identity";
        if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                                 NULL, NULL))
                || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                    SSL_ERROR_NONE))
                || !TEST_false(SSL_session_reused(clientssl))
                || !TEST_false(SSL_session_reused(serverssl)))
            goto end;

        if (idx == 0 || idx == 1) {
            if (!TEST_true(use_session_cb_cnt == 1)
                    || !TEST_true(find_session_cb_cnt == 1)
                    || !TEST_true(psk_client_cb_cnt == 0)
                       /*
                        * If no old style callback then below should be 0
                        * otherwise 1
                        */
                    || !TEST_true(psk_server_cb_cnt == idx))
                goto end;
        } else {
            if (!TEST_true(use_session_cb_cnt == 0)
                    || !TEST_true(find_session_cb_cnt == 0)
                    || !TEST_true(psk_client_cb_cnt == 1)
                    || !TEST_true(psk_server_cb_cnt == 1))
                goto end;
        }

        shutdown_ssl_connection(serverssl, clientssl);
        serverssl = clientssl = NULL;
    }
    testresult = 1;

 end:
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

#ifndef OSSL_NO_USABLE_TLS1_3
/*
 * Test TLS1.3 connection establishment succeeds with various configurations of
 * the options `SSL_OP_ALLOW_NO_DHE_KEX` and `SSL_OP_PREFER_NO_DHE_KEX`.
 * The verification of whether the right KEX mode is chosen is not covered by
 * this test but by `test_tls13kexmodes`.
 *
 * Tests (idx & 1): Server has `SSL_OP_ALLOW_NO_DHE_KEX` set.
 * Tests (idx & 2): Server has `SSL_OP_PREFER_NO_DHE_KEX` set.
 * Tests (idx & 4): Client has `SSL_OP_ALLOW_NO_DHE_KEX` set.
 */
static int test_tls13_no_dhe_kex(const int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    size_t j;
    SSL_SESSION *saved_session;

    int server_allow_no_dhe = (idx & 1) != 0;
    int server_prefer_no_dhe = (idx & 2) != 0;
    int client_allow_no_dhe = (idx & 4) != 0;

    uint64_t server_options = 0
            | (server_allow_no_dhe ? SSL_OP_ALLOW_NO_DHE_KEX : 0)
            | (server_prefer_no_dhe ? SSL_OP_PREFER_NO_DHE_KEX : 0);

    uint64_t client_options = 0
            | (client_allow_no_dhe ? SSL_OP_ALLOW_NO_DHE_KEX : 0);

    new_called = 0;
    do_cache = 1;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_session_cache_mode(cctx, SSL_SESS_CACHE_CLIENT
                                          | SSL_SESS_CACHE_NO_INTERNAL_STORE);

    SSL_CTX_set_options(sctx, server_options);
    SSL_CTX_set_options(cctx, client_options);

    SSL_CTX_sess_set_new_cb(cctx, new_cachesession_cb);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE))
            /* Check we got the number of tickets we were expecting */
            || !TEST_int_eq(2, new_called))
        goto end;

    /* We'll reuse the last ticket. */
    saved_session = sesscache[new_called - 1];

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(cctx);
    clientssl = serverssl = NULL;
    cctx = NULL;

    /*
     * Now we resume with the last ticket we created.
     */

    /* The server context already exists, so we only create the client. */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       NULL, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_options(cctx, client_options);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, saved_session)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    /*
     * Make sure, the session was resumed.
     */
    if (!TEST_true(SSL_session_reused(clientssl)))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    for (j = 0; j < OSSL_NELEM(sesscache); j++) {
        SSL_SESSION_free(sesscache[j]);
        sesscache[j] = NULL;
    }
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* OSSL_NO_USABLE_TLS1_3 */

static unsigned char cookie_magic_value[] = "cookie magic";

static int generate_cookie_callback(SSL *ssl, unsigned char *cookie,
                                    unsigned int *cookie_len)
{
    /*
     * Not suitable as a real cookie generation function but good enough for
     * testing!
     */
    memcpy(cookie, cookie_magic_value, sizeof(cookie_magic_value) - 1);
    *cookie_len = sizeof(cookie_magic_value) - 1;

    return 1;
}

static int verify_cookie_callback(SSL *ssl, const unsigned char *cookie,
                                  unsigned int cookie_len)
{
    if (cookie_len == sizeof(cookie_magic_value) - 1
        && memcmp(cookie, cookie_magic_value, cookie_len) == 0)
        return 1;

    return 0;
}

static int generate_stateless_cookie_callback(SSL *ssl, unsigned char *cookie,
                                        size_t *cookie_len)
{
    unsigned int temp;
    int res = generate_cookie_callback(ssl, cookie, &temp);
    *cookie_len = temp;
    return res;
}

static int verify_stateless_cookie_callback(SSL *ssl, const unsigned char *cookie,
                                      size_t cookie_len)
{
    return verify_cookie_callback(ssl, cookie, cookie_len);
}

static int test_stateless(void)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    /* The arrival of CCS messages can confuse the test */
    SSL_CTX_clear_options(cctx, SSL_OP_ENABLE_MIDDLEBOX_COMPAT);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
               /* Send the first ClientHello */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                 SSL_ERROR_WANT_READ))
               /*
                * This should fail with a -1 return because we have no callbacks
                * set up
                */
            || !TEST_int_eq(SSL_stateless(serverssl), -1))
        goto end;

    /* Fatal error so abandon the connection from this client */
    SSL_free(clientssl);
    clientssl = NULL;

    /* Set up the cookie generation and verification callbacks */
    SSL_CTX_set_stateless_cookie_generate_cb(sctx, generate_stateless_cookie_callback);
    SSL_CTX_set_stateless_cookie_verify_cb(sctx, verify_stateless_cookie_callback);

    /*
     * Create a new connection from the client (we can reuse the server SSL
     * object).
     */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
               /* Send the first ClientHello */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_WANT_READ))
               /* This should fail because there is no cookie */
            || !TEST_int_eq(SSL_stateless(serverssl), 0))
        goto end;

    /* Abandon the connection from this client */
    SSL_free(clientssl);
    clientssl = NULL;

    /*
     * Now create a connection from a new client but with the same server SSL
     * object
     */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
               /* Send the first ClientHello */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_WANT_READ))
               /* This should fail because there is no cookie */
            || !TEST_int_eq(SSL_stateless(serverssl), 0)
               /* Send the second ClientHello */
            || !TEST_false(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_WANT_READ))
               /* This should succeed because a cookie is now present */
            || !TEST_int_eq(SSL_stateless(serverssl), 1)
               /* Complete the connection */
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;
    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;

}
#endif /* OSSL_NO_USABLE_TLS1_3 */

static int clntaddoldcb = 0;
static int clntparseoldcb = 0;
static int srvaddoldcb = 0;
static int srvparseoldcb = 0;
static int clntaddnewcb = 0;
static int clntparsenewcb = 0;
static int srvaddnewcb = 0;
static int srvparsenewcb = 0;
static int snicb = 0;

#define TEST_EXT_TYPE1  0xff00

static int old_add_cb(SSL *s, unsigned int ext_type, const unsigned char **out,
                      size_t *outlen, int *al, void *add_arg)
{
    int *server = (int *)add_arg;
    unsigned char *data;

    if (SSL_is_server(s))
        srvaddoldcb++;
    else
        clntaddoldcb++;

    if (*server != SSL_is_server(s)
            || (data = OPENSSL_malloc(sizeof(*data))) == NULL)
        return -1;

    *data = 1;
    *out = data;
    *outlen = sizeof(char);
    return 1;
}

static void old_free_cb(SSL *s, unsigned int ext_type, const unsigned char *out,
                        void *add_arg)
{
    OPENSSL_free((unsigned char *)out);
}

static int old_parse_cb(SSL *s, unsigned int ext_type, const unsigned char *in,
                        size_t inlen, int *al, void *parse_arg)
{
    int *server = (int *)parse_arg;

    if (SSL_is_server(s))
        srvparseoldcb++;
    else
        clntparseoldcb++;

    if (*server != SSL_is_server(s)
            || inlen != sizeof(char)
            || *in != 1)
        return -1;

    return 1;
}

static int new_add_cb(SSL *s, unsigned int ext_type, unsigned int context,
                      const unsigned char **out, size_t *outlen, X509 *x,
                      size_t chainidx, int *al, void *add_arg)
{
    int *server = (int *)add_arg;
    unsigned char *data;

    if (SSL_is_server(s))
        srvaddnewcb++;
    else
        clntaddnewcb++;

    if (*server != SSL_is_server(s)
            || (data = OPENSSL_malloc(sizeof(*data))) == NULL)
        return -1;

    *data = 1;
    *out = data;
    *outlen = sizeof(*data);
    return 1;
}

static void new_free_cb(SSL *s, unsigned int ext_type, unsigned int context,
                        const unsigned char *out, void *add_arg)
{
    OPENSSL_free((unsigned char *)out);
}

static int new_parse_cb(SSL *s, unsigned int ext_type, unsigned int context,
                        const unsigned char *in, size_t inlen, X509 *x,
                        size_t chainidx, int *al, void *parse_arg)
{
    int *server = (int *)parse_arg;

    if (SSL_is_server(s))
        srvparsenewcb++;
    else
        clntparsenewcb++;

    if (*server != SSL_is_server(s)
            || inlen != sizeof(char) || *in != 1)
        return -1;

    return 1;
}

static int sni_cb(SSL *s, int *al, void *arg)
{
    SSL_CTX *ctx = (SSL_CTX *)arg;

    if (SSL_set_SSL_CTX(s, ctx) == NULL) {
        *al = SSL_AD_INTERNAL_ERROR;
        return SSL_TLSEXT_ERR_ALERT_FATAL;
    }
    snicb++;
    return SSL_TLSEXT_ERR_OK;
}

static int verify_cb(int preverify_ok, X509_STORE_CTX *x509_ctx)
{
    return 1;
}

/*
 * Custom call back tests.
 * Test 0: Old style callbacks in TLSv1.2
 * Test 1: New style callbacks in TLSv1.2
 * Test 2: New style callbacks in TLSv1.2 with SNI
 * Test 3: New style callbacks in TLSv1.3. Extensions in CH and EE
 * Test 4: New style callbacks in TLSv1.3. Extensions in CH, SH, EE, Cert + NST
 * Test 5: New style callbacks in TLSv1.3. Extensions in CR + Client Cert
 */
static int test_custom_exts(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL, *sctx2 = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    static int server = 1;
    static int client = 0;
    SSL_SESSION *sess = NULL;
    unsigned int context;

#if defined(OPENSSL_NO_TLS1_2) && !defined(OSSL_NO_USABLE_TLS1_3)
    /* Skip tests for TLSv1.2 and below in this case */
    if (tst < 3)
        return 1;
#endif

    /* Reset callback counters */
    clntaddoldcb = clntparseoldcb = srvaddoldcb = srvparseoldcb = 0;
    clntaddnewcb = clntparsenewcb = srvaddnewcb = srvparsenewcb = 0;
    snicb = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (tst == 2
            && !TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(), NULL,
                                              TLS1_VERSION, 0,
                                              &sctx2, NULL, cert, privkey)))
        goto end;


    if (tst < 3) {
        SSL_CTX_set_options(cctx, SSL_OP_NO_TLSv1_3);
        SSL_CTX_set_options(sctx, SSL_OP_NO_TLSv1_3);
        if (sctx2 != NULL)
            SSL_CTX_set_options(sctx2, SSL_OP_NO_TLSv1_3);
    }

    if (tst == 5) {
        context = SSL_EXT_TLS1_3_CERTIFICATE_REQUEST
                  | SSL_EXT_TLS1_3_CERTIFICATE;
        SSL_CTX_set_verify(sctx,
                           SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT,
                           verify_cb);
        if (!TEST_int_eq(SSL_CTX_use_certificate_file(cctx, cert,
                                                      SSL_FILETYPE_PEM), 1)
                || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(cctx, privkey,
                                                            SSL_FILETYPE_PEM), 1)
                || !TEST_int_eq(SSL_CTX_check_private_key(cctx), 1))
            goto end;
    } else if (tst == 4) {
        context = SSL_EXT_CLIENT_HELLO
                  | SSL_EXT_TLS1_2_SERVER_HELLO
                  | SSL_EXT_TLS1_3_SERVER_HELLO
                  | SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS
                  | SSL_EXT_TLS1_3_CERTIFICATE
                  | SSL_EXT_TLS1_3_NEW_SESSION_TICKET;
    } else {
        context = SSL_EXT_CLIENT_HELLO
                  | SSL_EXT_TLS1_2_SERVER_HELLO
                  | SSL_EXT_TLS1_3_ENCRYPTED_EXTENSIONS;
    }

    /* Create a client side custom extension */
    if (tst == 0) {
        if (!TEST_true(SSL_CTX_add_client_custom_ext(cctx, TEST_EXT_TYPE1,
                                                     old_add_cb, old_free_cb,
                                                     &client, old_parse_cb,
                                                     &client)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_add_custom_ext(cctx, TEST_EXT_TYPE1, context,
                                              new_add_cb, new_free_cb,
                                              &client, new_parse_cb, &client)))
            goto end;
    }

    /* Should not be able to add duplicates */
    if (!TEST_false(SSL_CTX_add_client_custom_ext(cctx, TEST_EXT_TYPE1,
                                                  old_add_cb, old_free_cb,
                                                  &client, old_parse_cb,
                                                  &client))
            || !TEST_false(SSL_CTX_add_custom_ext(cctx, TEST_EXT_TYPE1,
                                                  context, new_add_cb,
                                                  new_free_cb, &client,
                                                  new_parse_cb, &client)))
        goto end;

    /* Create a server side custom extension */
    if (tst == 0) {
        if (!TEST_true(SSL_CTX_add_server_custom_ext(sctx, TEST_EXT_TYPE1,
                                                     old_add_cb, old_free_cb,
                                                     &server, old_parse_cb,
                                                     &server)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_add_custom_ext(sctx, TEST_EXT_TYPE1, context,
                                              new_add_cb, new_free_cb,
                                              &server, new_parse_cb, &server)))
            goto end;
        if (sctx2 != NULL
                && !TEST_true(SSL_CTX_add_custom_ext(sctx2, TEST_EXT_TYPE1,
                                                     context, new_add_cb,
                                                     new_free_cb, &server,
                                                     new_parse_cb, &server)))
            goto end;
    }

    /* Should not be able to add duplicates */
    if (!TEST_false(SSL_CTX_add_server_custom_ext(sctx, TEST_EXT_TYPE1,
                                                  old_add_cb, old_free_cb,
                                                  &server, old_parse_cb,
                                                  &server))
            || !TEST_false(SSL_CTX_add_custom_ext(sctx, TEST_EXT_TYPE1,
                                                  context, new_add_cb,
                                                  new_free_cb, &server,
                                                  new_parse_cb, &server)))
        goto end;

    if (tst == 2) {
        /* Set up SNI */
        if (!TEST_true(SSL_CTX_set_tlsext_servername_callback(sctx, sni_cb))
                || !TEST_true(SSL_CTX_set_tlsext_servername_arg(sctx, sctx2)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (tst == 0) {
        if (clntaddoldcb != 1
                || clntparseoldcb != 1
                || srvaddoldcb != 1
                || srvparseoldcb != 1)
            goto end;
    } else if (tst == 1 || tst == 2 || tst == 3) {
        if (clntaddnewcb != 1
                || clntparsenewcb != 1
                || srvaddnewcb != 1
                || srvparsenewcb != 1
                || (tst != 2 && snicb != 0)
                || (tst == 2 && snicb != 1))
            goto end;
    } else if (tst == 5) {
        if (clntaddnewcb != 1
                || clntparsenewcb != 1
                || srvaddnewcb != 1
                || srvparsenewcb != 1)
            goto end;
    } else {
        /* In this case there 2 NewSessionTicket messages created */
        if (clntaddnewcb != 1
                || clntparsenewcb != 5
                || srvaddnewcb != 5
                || srvparsenewcb != 1)
            goto end;
    }

    sess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    if (tst == 3 || tst == 5) {
        /* We don't bother with the resumption aspects for these tests */
        testresult = 1;
        goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                               SSL_ERROR_NONE)))
        goto end;

    /*
     * For a resumed session we expect to add the ClientHello extension. For the
     * old style callbacks we ignore it on the server side because they set
     * SSL_EXT_IGNORE_ON_RESUMPTION. The new style callbacks do not ignore
     * them.
     */
    if (tst == 0) {
        if (clntaddoldcb != 2
                || clntparseoldcb != 1
                || srvaddoldcb != 1
                || srvparseoldcb != 1)
            goto end;
    } else if (tst == 1 || tst == 2 || tst == 3) {
        if (clntaddnewcb != 2
                || clntparsenewcb != 2
                || srvaddnewcb != 2
                || srvparsenewcb != 2)
            goto end;
    } else {
        /*
         * No Certificate message extensions in the resumption handshake,
         * 2 NewSessionTickets in the initial handshake, 1 in the resumption
         */
        if (clntaddnewcb != 2
                || clntparsenewcb != 8
                || srvaddnewcb != 8
                || srvparsenewcb != 2)
            goto end;
    }

    testresult = 1;

end:
    SSL_SESSION_free(sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx2);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

#if !defined(OPENSSL_NO_TLS1_2) && !defined(OSSL_NO_USABLE_TLS1_3)

#define  SYNTHV1CONTEXT     (SSL_EXT_TLS1_2_AND_BELOW_ONLY \
                             | SSL_EXT_CLIENT_HELLO \
                             | SSL_EXT_TLS1_2_SERVER_HELLO \
                             | SSL_EXT_IGNORE_ON_RESUMPTION)

#define TLS13CONTEXT (SSL_EXT_TLS1_3_CERTIFICATE \
                      | SSL_EXT_TLS1_2_SERVER_HELLO \
                      | SSL_EXT_CLIENT_HELLO)

#define SERVERINFO_CUSTOM                                 \
    0x00, (char)TLSEXT_TYPE_signed_certificate_timestamp, \
    0x00, 0x03,                                           \
    0x04, 0x05, 0x06                                      \

static const unsigned char serverinfo_custom_tls13[] = {
    0x00, 0x00, (TLS13CONTEXT >> 8) & 0xff, TLS13CONTEXT & 0xff,
    SERVERINFO_CUSTOM
};
static const unsigned char serverinfo_custom_v2[] = {
    0x00, 0x00, (SYNTHV1CONTEXT >> 8) & 0xff,  SYNTHV1CONTEXT & 0xff,
    SERVERINFO_CUSTOM
};
static const unsigned char serverinfo_custom_v1[] = {
    SERVERINFO_CUSTOM
};
static const size_t serverinfo_custom_tls13_len = sizeof(serverinfo_custom_tls13);
static const size_t serverinfo_custom_v2_len = sizeof(serverinfo_custom_v2);
static const size_t serverinfo_custom_v1_len = sizeof(serverinfo_custom_v1);

static int serverinfo_custom_parse_cb(SSL *s, unsigned int ext_type,
                                      unsigned int context,
                                      const unsigned char *in,
                                      size_t inlen, X509 *x,
                                      size_t chainidx, int *al,
                                      void *parse_arg)
{
    const size_t len = serverinfo_custom_v1_len;
    const unsigned char *si = &serverinfo_custom_v1[len - 3];
    int *p_cb_result = (int*)parse_arg;
    *p_cb_result = TEST_mem_eq(in, inlen, si, 3);
    return 1;
}

static int test_serverinfo_custom(const int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    int cb_result = 0;

    /*
     * Following variables are set in the switch statement
     *  according to the test iteration.
     * Default values do not make much sense: test would fail with them.
     */
    int serverinfo_version = 0;
    int protocol_version = 0;
    unsigned int extension_context = 0;
    const unsigned char *si = NULL;
    size_t si_len = 0;

    const int call_use_serverinfo_ex = idx > 0;
    switch (idx) {
    case 0: /* FALLTHROUGH */
    case 1:
        serverinfo_version = SSL_SERVERINFOV1;
        protocol_version = TLS1_2_VERSION;
        extension_context = SYNTHV1CONTEXT;
        si = serverinfo_custom_v1;
        si_len = serverinfo_custom_v1_len;
        break;
    case 2:
        serverinfo_version = SSL_SERVERINFOV2;
        protocol_version = TLS1_2_VERSION;
        extension_context = SYNTHV1CONTEXT;
        si = serverinfo_custom_v2;
        si_len = serverinfo_custom_v2_len;
        break;
    case 3:
        serverinfo_version = SSL_SERVERINFOV2;
        protocol_version = TLS1_3_VERSION;
        extension_context = TLS13CONTEXT;
        si = serverinfo_custom_tls13;
        si_len = serverinfo_custom_tls13_len;
        break;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx,
                                       TLS_method(),
                                       TLS_method(),
                                       protocol_version,
                                       protocol_version,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (call_use_serverinfo_ex) {
        if (!TEST_true(SSL_CTX_use_serverinfo_ex(sctx, serverinfo_version,
                                                 si, si_len)))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_use_serverinfo(sctx, si, si_len)))
            goto end;
    }

    if (!TEST_true(SSL_CTX_add_custom_ext(cctx, TLSEXT_TYPE_signed_certificate_timestamp,
                                          extension_context,
                                          NULL, NULL, NULL,
                                          serverinfo_custom_parse_cb,
                                          &cb_result))
        || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                         NULL, NULL))
        || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                            SSL_ERROR_NONE))
        || !TEST_int_eq(SSL_do_handshake(clientssl), 1))
        goto end;

    if (!TEST_true(cb_result))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif

/*
 * Test that SSL_export_keying_material() produces expected results. There are
 * no test vectors so all we do is test that both sides of the communication
 * produce the same results for different protocol versions.
 */
#define SMALL_LABEL_LEN 10
#define LONG_LABEL_LEN  249
static int test_export_key_mat(int tst)
{
    int testresult = 0;
    SSL_CTX *cctx = NULL, *sctx = NULL, *sctx2 = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    const char label[LONG_LABEL_LEN + 1] = "test label";
    const unsigned char context[] = "context";
    const unsigned char *emptycontext = NULL;
    unsigned char longcontext[1280];
    int test_longcontext = fips_provider_version_ge(libctx, 3, 3, 0);
    unsigned char ckeymat1[80], ckeymat2[80], ckeymat3[80], ckeymat4[80];
    unsigned char skeymat1[80], skeymat2[80], skeymat3[80], skeymat4[80];
    size_t labellen;
    const int protocols[] = {
        TLS1_VERSION,
        TLS1_1_VERSION,
        TLS1_2_VERSION,
        TLS1_3_VERSION,
        TLS1_3_VERSION,
        TLS1_3_VERSION
    };

#ifdef OPENSSL_NO_TLS1
    if (tst == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_TLS1_1
    if (tst == 1)
        return 1;
#endif
    if (is_fips && (tst == 0 || tst == 1))
        return 1;
#ifdef OPENSSL_NO_TLS1_2
    if (tst == 2)
        return 1;
#endif
#ifdef OSSL_NO_USABLE_TLS1_3
    if (tst >= 3)
        return 1;
#endif
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    OPENSSL_assert(tst >= 0 && (size_t)tst < OSSL_NELEM(protocols));
    SSL_CTX_set_max_proto_version(cctx, protocols[tst]);
    SSL_CTX_set_min_proto_version(cctx, protocols[tst]);
    if ((protocols[tst] < TLS1_2_VERSION) &&
        (!SSL_CTX_set_cipher_list(cctx, "DEFAULT:@SECLEVEL=0")
        || !SSL_CTX_set_cipher_list(sctx, "DEFAULT:@SECLEVEL=0")))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    /*
     * Premature call of SSL_export_keying_material should just fail.
     */
    if (!TEST_int_le(SSL_export_keying_material(clientssl, ckeymat1,
                                                sizeof(ckeymat1), label,
                                                SMALL_LABEL_LEN + 1, context,
                                                sizeof(context) - 1, 1), 0))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    if (tst == 5) {
        /*
         * TLSv1.3 imposes a maximum label len of 249 bytes. Check we fail if we
         * go over that.
         */
        if (!TEST_int_le(SSL_export_keying_material(clientssl, ckeymat1,
                                                    sizeof(ckeymat1), label,
                                                    LONG_LABEL_LEN + 1, context,
                                                    sizeof(context) - 1, 1), 0))
            goto end;

        testresult = 1;
        goto end;
    } else if (tst == 4) {
        labellen = LONG_LABEL_LEN;
    } else {
        labellen = SMALL_LABEL_LEN;
    }

    memset(longcontext, 1, sizeof(longcontext));

    if (!TEST_int_eq(SSL_export_keying_material(clientssl, ckeymat1,
                                                sizeof(ckeymat1), label,
                                                labellen, context,
                                                sizeof(context) - 1, 1), 1)
            || !TEST_int_eq(SSL_export_keying_material(clientssl, ckeymat2,
                                                       sizeof(ckeymat2), label,
                                                       labellen,
                                                       emptycontext,
                                                       0, 1), 1)
            || !TEST_int_eq(SSL_export_keying_material(clientssl, ckeymat3,
                                                       sizeof(ckeymat3), label,
                                                       labellen,
                                                       NULL, 0, 0), 1)
            || (test_longcontext
                && !TEST_int_eq(SSL_export_keying_material(clientssl,
                                                           ckeymat4,
                                                           sizeof(ckeymat4), label,
                                                           labellen,
                                                           longcontext,
                                                           sizeof(longcontext), 1),
                                1))
            || !TEST_int_eq(SSL_export_keying_material(serverssl, skeymat1,
                                                       sizeof(skeymat1), label,
                                                       labellen,
                                                       context,
                                                       sizeof(context) -1, 1),
                            1)
            || !TEST_int_eq(SSL_export_keying_material(serverssl, skeymat2,
                                                       sizeof(skeymat2), label,
                                                       labellen,
                                                       emptycontext,
                                                       0, 1), 1)
            || !TEST_int_eq(SSL_export_keying_material(serverssl, skeymat3,
                                                       sizeof(skeymat3), label,
                                                       labellen,
                                                       NULL, 0, 0), 1)
            || (test_longcontext
                && !TEST_int_eq(SSL_export_keying_material(serverssl, skeymat4,
                                                           sizeof(skeymat4), label,
                                                           labellen,
                                                           longcontext,
                                                           sizeof(longcontext), 1),
                                1))
               /*
                * Check that both sides created the same key material with the
                * same context.
                */
            || !TEST_mem_eq(ckeymat1, sizeof(ckeymat1), skeymat1,
                            sizeof(skeymat1))
               /*
                * Check that both sides created the same key material with an
                * empty context.
                */
            || !TEST_mem_eq(ckeymat2, sizeof(ckeymat2), skeymat2,
                            sizeof(skeymat2))
               /*
                * Check that both sides created the same key material without a
                * context.
                */
            || !TEST_mem_eq(ckeymat3, sizeof(ckeymat3), skeymat3,
                            sizeof(skeymat3))
               /*
                * Check that both sides created the same key material with a
                * long context.
                */
            || (test_longcontext
                && !TEST_mem_eq(ckeymat4, sizeof(ckeymat4), skeymat4,
                                sizeof(skeymat4)))
               /* Different contexts should produce different results */
            || !TEST_mem_ne(ckeymat1, sizeof(ckeymat1), ckeymat2,
                            sizeof(ckeymat2)))
        goto end;

    /*
     * Check that an empty context and no context produce different results in
     * protocols less than TLSv1.3. In TLSv1.3 they should be the same.
     */
    if ((tst < 3 && !TEST_mem_ne(ckeymat2, sizeof(ckeymat2), ckeymat3,
                                  sizeof(ckeymat3)))
            || (tst >= 3 && !TEST_mem_eq(ckeymat2, sizeof(ckeymat2), ckeymat3,
                                         sizeof(ckeymat3))))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx2);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

#ifndef OSSL_NO_USABLE_TLS1_3
/*
 * Test that SSL_export_keying_material_early() produces expected
 * results. There are no test vectors so all we do is test that both
 * sides of the communication produce the same results for different
 * protocol versions.
 */
static int test_export_key_mat_early(int idx)
{
    static const char label[] = "test label";
    static const unsigned char context[] = "context";
    int testresult = 0;
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *sess = NULL;
    const unsigned char *emptycontext = NULL;
    unsigned char ckeymat1[80], ckeymat2[80];
    unsigned char skeymat1[80], skeymat2[80];
    unsigned char buf[1];
    size_t readbytes, written;

    if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl, &serverssl,
                                        &sess, idx, SHA384_DIGEST_LENGTH)))
        goto end;

    /* Here writing 0 length early data is enough. */
    if (!TEST_true(SSL_write_early_data(clientssl, NULL, 0, &written))
            || !TEST_int_eq(SSL_read_early_data(serverssl, buf, sizeof(buf),
                                                &readbytes),
                            SSL_READ_EARLY_DATA_ERROR)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                            SSL_EARLY_DATA_ACCEPTED))
        goto end;

    if (!TEST_int_eq(SSL_export_keying_material_early(
                     clientssl, ckeymat1, sizeof(ckeymat1), label,
                     sizeof(label) - 1, context, sizeof(context) - 1), 1)
            || !TEST_int_eq(SSL_export_keying_material_early(
                            clientssl, ckeymat2, sizeof(ckeymat2), label,
                            sizeof(label) - 1, emptycontext, 0), 1)
            || !TEST_int_eq(SSL_export_keying_material_early(
                            serverssl, skeymat1, sizeof(skeymat1), label,
                            sizeof(label) - 1, context, sizeof(context) - 1), 1)
            || !TEST_int_eq(SSL_export_keying_material_early(
                            serverssl, skeymat2, sizeof(skeymat2), label,
                            sizeof(label) - 1, emptycontext, 0), 1)
               /*
                * Check that both sides created the same key material with the
                * same context.
                */
            || !TEST_mem_eq(ckeymat1, sizeof(ckeymat1), skeymat1,
                            sizeof(skeymat1))
               /*
                * Check that both sides created the same key material with an
                * empty context.
                */
            || !TEST_mem_eq(ckeymat2, sizeof(ckeymat2), skeymat2,
                            sizeof(skeymat2))
               /* Different contexts should produce different results */
            || !TEST_mem_ne(ckeymat1, sizeof(ckeymat1), ckeymat2,
                            sizeof(ckeymat2)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

#define NUM_KEY_UPDATE_MESSAGES 40
/*
 * Test KeyUpdate.
 */
static int test_key_update(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, i, j;
    char buf[20];
    static char *mess = "A test message";

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_3_VERSION,
                                       0,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    for (j = 0; j < 2; j++) {
        /* Send lots of KeyUpdate messages */
        for (i = 0; i < NUM_KEY_UPDATE_MESSAGES; i++) {
            if (!TEST_true(SSL_key_update(clientssl,
                                          (j == 0)
                                          ? SSL_KEY_UPDATE_NOT_REQUESTED
                                          : SSL_KEY_UPDATE_REQUESTED))
                    || !TEST_true(SSL_do_handshake(clientssl)))
                goto end;
        }

        /* Check that sending and receiving app data is ok */
        if (!TEST_int_eq(SSL_write(clientssl, mess, strlen(mess)), strlen(mess))
                || !TEST_int_eq(SSL_read(serverssl, buf, sizeof(buf)),
                                         strlen(mess)))
            goto end;

        if (!TEST_int_eq(SSL_write(serverssl, mess, strlen(mess)), strlen(mess))
                || !TEST_int_eq(SSL_read(clientssl, buf, sizeof(buf)),
                                         strlen(mess)))
            goto end;
    }

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test we can handle a KeyUpdate (update requested) message while
 * write data is pending in peer.
 * Test 0: Client sends KeyUpdate while Server is writing
 * Test 1: Server sends KeyUpdate while Client is writing
 */
static int test_key_update_peer_in_write(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char buf[20];
    static char *mess = "A test message";
    BIO *bretry = BIO_new(bio_s_always_retry());
    BIO *tmp = NULL;
    SSL *peerupdate = NULL, *peerwrite = NULL;

    if (!TEST_ptr(bretry)
            || !TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_3_VERSION,
                                              0,
                                              &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    peerupdate = tst == 0 ? clientssl : serverssl;
    peerwrite = tst == 0 ? serverssl : clientssl;

    if (!TEST_true(SSL_key_update(peerupdate, SSL_KEY_UPDATE_REQUESTED))
            || !TEST_int_eq(SSL_do_handshake(peerupdate), 1))
        goto end;

    /* Swap the writing endpoint's write BIO to force a retry */
    tmp = SSL_get_wbio(peerwrite);
    if (!TEST_ptr(tmp) || !TEST_true(BIO_up_ref(tmp))) {
        tmp = NULL;
        goto end;
    }
    SSL_set0_wbio(peerwrite, bretry);
    bretry = NULL;

    /* Write data that we know will fail with SSL_ERROR_WANT_WRITE */
    if (!TEST_int_eq(SSL_write(peerwrite, mess, strlen(mess)), -1)
            || !TEST_int_eq(SSL_get_error(peerwrite, 0), SSL_ERROR_WANT_WRITE)
            || !TEST_true(SSL_want_write(peerwrite))
            || !TEST_true(SSL_net_write_desired(peerwrite)))
        goto end;

    /* Reinstate the original writing endpoint's write BIO */
    SSL_set0_wbio(peerwrite, tmp);
    tmp = NULL;

    /* Now read some data - we will read the key update */
    if (!TEST_int_eq(SSL_read(peerwrite, buf, sizeof(buf)), -1)
            || !TEST_int_eq(SSL_get_error(peerwrite, 0), SSL_ERROR_WANT_READ)
            || !TEST_true(SSL_want_read(peerwrite))
            || !TEST_true(SSL_net_read_desired(peerwrite)))
        goto end;

    /*
     * Complete the write we started previously and read it from the other
     * endpoint
     */
    if (!TEST_int_eq(SSL_write(peerwrite, mess, strlen(mess)), strlen(mess))
            || !TEST_int_eq(SSL_read(peerupdate, buf, sizeof(buf)), strlen(mess)))
        goto end;

    /* Write more data to ensure we send the KeyUpdate message back */
    if (!TEST_int_eq(SSL_write(peerwrite, mess, strlen(mess)), strlen(mess))
            || !TEST_int_eq(SSL_read(peerupdate, buf, sizeof(buf)), strlen(mess)))
        goto end;

    if (!TEST_false(SSL_net_read_desired(peerwrite))
        || !TEST_false(SSL_net_write_desired(peerwrite))
        || !TEST_int_eq(SSL_want(peerwrite), SSL_NOTHING))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    BIO_free(bretry);
    BIO_free(tmp);

    return testresult;
}

/*
 * Test we can handle a KeyUpdate (update requested) message while
 * peer read data is pending after peer accepted keyupdate(the msg header
 * had been read 5 bytes).
 * Test 0: Client sends KeyUpdate while Server is reading
 * Test 1: Server sends KeyUpdate while Client is reading
 */
static int test_key_update_peer_in_read(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char prbuf[515], lwbuf[515] = {0};
    static char *mess = "A test message";
    BIO *lbio = NULL, *pbio = NULL;
    SSL *local = NULL, *peer = NULL;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_3_VERSION,
                                              0,
                                              &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    local = tst == 0 ? clientssl : serverssl;
    peer = tst == 0 ? serverssl : clientssl;

    if (!TEST_int_eq(BIO_new_bio_pair(&lbio, 512, &pbio, 512), 1))
        goto end;

    SSL_set_bio(local, lbio, lbio);
    SSL_set_bio(peer, pbio, pbio);

    /*
     * we first write keyupdate msg then appdata in local
     * write data in local will fail with SSL_ERROR_WANT_WRITE,because
     * lwbuf app data msg size + key updata msg size > 512(the size of
     * the bio pair buffer)
     */
    if (!TEST_true(SSL_key_update(local, SSL_KEY_UPDATE_REQUESTED))
            || !TEST_int_eq(SSL_write(local, lwbuf, sizeof(lwbuf)), -1)
            || !TEST_int_eq(SSL_get_error(local, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    /*
     * first read keyupdate msg in peer in peer
     * then read appdata that we know will fail with SSL_ERROR_WANT_READ
     */
    if (!TEST_int_eq(SSL_read(peer, prbuf, sizeof(prbuf)), -1)
            || !TEST_int_eq(SSL_get_error(peer, -1), SSL_ERROR_WANT_READ))
        goto end;

    /* Now write some data in peer - we will write the key update */
    if (!TEST_int_eq(SSL_write(peer, mess, strlen(mess)), strlen(mess)))
        goto end;

    /*
     * write data in local previously that we will complete
     * read data in peer previously that we will complete
     */
    if (!TEST_int_eq(SSL_write(local, lwbuf, sizeof(lwbuf)), sizeof(lwbuf))
            || !TEST_int_eq(SSL_read(peer, prbuf, sizeof(prbuf)), sizeof(prbuf)))
        goto end;

    /* check that sending and receiving appdata ok */
    if (!TEST_int_eq(SSL_write(local, mess, strlen(mess)), strlen(mess))
            || !TEST_int_eq(SSL_read(peer, prbuf, sizeof(prbuf)), strlen(mess)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test we can't send a KeyUpdate (update requested) message while
 * local write data is pending.
 * Test 0: Client sends KeyUpdate while Client is writing
 * Test 1: Server sends KeyUpdate while Server is writing
 */
static int test_key_update_local_in_write(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char buf[20];
    static char *mess = "A test message";
    BIO *bretry = BIO_new(bio_s_always_retry());
    BIO *tmp = NULL;
    SSL *local = NULL, *peer = NULL;

    if (!TEST_ptr(bretry)
            || !TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_3_VERSION,
                                              0,
                                              &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    local = tst == 0 ? clientssl : serverssl;
    peer = tst == 0 ? serverssl : clientssl;

    /* Swap the writing endpoint's write BIO to force a retry */
    tmp = SSL_get_wbio(local);
    if (!TEST_ptr(tmp) || !TEST_true(BIO_up_ref(tmp))) {
        tmp = NULL;
        goto end;
    }
    SSL_set0_wbio(local, bretry);
    bretry = NULL;

    /* write data in local will fail with SSL_ERROR_WANT_WRITE */
    if (!TEST_int_eq(SSL_write(local, mess, strlen(mess)), -1)
            || !TEST_int_eq(SSL_get_error(local, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    /* Reinstate the original writing endpoint's write BIO */
    SSL_set0_wbio(local, tmp);
    tmp = NULL;

    /* SSL_key_update will fail, because writing in local*/
    if (!TEST_false(SSL_key_update(local, SSL_KEY_UPDATE_REQUESTED))
        || !TEST_int_eq(ERR_GET_REASON(ERR_peek_error()), SSL_R_BAD_WRITE_RETRY))
    goto end;

    ERR_clear_error();
    /* write data in local previously that we will complete */
    if (!TEST_int_eq(SSL_write(local, mess, strlen(mess)), strlen(mess)))
        goto end;

    /* SSL_key_update will succeed because there is no pending write data */
    if (!TEST_true(SSL_key_update(local, SSL_KEY_UPDATE_REQUESTED))
        || !TEST_int_eq(SSL_do_handshake(local), 1))
        goto end;

    /*
     * we write some appdata in local
     * read data in peer - we will read the keyupdate msg
     */
    if (!TEST_int_eq(SSL_write(local, mess, strlen(mess)), strlen(mess))
        || !TEST_int_eq(SSL_read(peer, buf, sizeof(buf)), strlen(mess)))
        goto end;

    /* Write more peer more data to ensure we send the keyupdate message back */
    if (!TEST_int_eq(SSL_write(peer, mess, strlen(mess)), strlen(mess))
            || !TEST_int_eq(SSL_read(local, buf, sizeof(buf)), strlen(mess)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    BIO_free(bretry);
    BIO_free(tmp);

    return testresult;
}

/*
 * Test we can handle a KeyUpdate (update requested) message while
 * local read data is pending(the msg header had been read 5 bytes).
 * Test 0: Client sends KeyUpdate while Client is reading
 * Test 1: Server sends KeyUpdate while Server is reading
 */
static int test_key_update_local_in_read(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char lrbuf[515], pwbuf[515] = {0}, prbuf[20];
    static char *mess = "A test message";
    BIO *lbio = NULL, *pbio = NULL;
    SSL *local = NULL, *peer = NULL;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                              TLS_client_method(),
                                              TLS1_3_VERSION,
                                              0,
                                              &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    local = tst == 0 ? clientssl : serverssl;
    peer = tst == 0 ? serverssl : clientssl;

    if (!TEST_int_eq(BIO_new_bio_pair(&lbio, 512, &pbio, 512), 1))
        goto end;

    SSL_set_bio(local, lbio, lbio);
    SSL_set_bio(peer, pbio, pbio);

    /* write app data in peer will fail with SSL_ERROR_WANT_WRITE */
    if (!TEST_int_eq(SSL_write(peer, pwbuf, sizeof(pwbuf)), -1)
        || !TEST_int_eq(SSL_get_error(peer, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    /* read appdata in local will fail with SSL_ERROR_WANT_READ */
    if (!TEST_int_eq(SSL_read(local, lrbuf, sizeof(lrbuf)), -1)
            || !TEST_int_eq(SSL_get_error(local, -1), SSL_ERROR_WANT_READ))
        goto end;

    /* SSL_do_handshake will send keyupdate msg */
    if (!TEST_true(SSL_key_update(local, SSL_KEY_UPDATE_REQUESTED))
            || !TEST_int_eq(SSL_do_handshake(local), 1))
        goto end;

    /*
     * write data in peer previously that we will complete
     * read data in local previously that we will complete
     */
    if (!TEST_int_eq(SSL_write(peer, pwbuf, sizeof(pwbuf)), sizeof(pwbuf))
        || !TEST_int_eq(SSL_read(local, lrbuf, sizeof(lrbuf)), sizeof(lrbuf)))
        goto end;

    /*
     * write data in local
     * read data in peer - we will read the key update
     */
    if (!TEST_int_eq(SSL_write(local, mess, strlen(mess)), strlen(mess))
        || !TEST_int_eq(SSL_read(peer, prbuf, sizeof(prbuf)), strlen(mess)))
        goto end;

  /* Write more peer data to ensure we send the keyupdate message back */
    if (!TEST_int_eq(SSL_write(peer, mess, strlen(mess)), strlen(mess))
            || !TEST_int_eq(SSL_read(local, lrbuf, sizeof(lrbuf)), strlen(mess)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* OSSL_NO_USABLE_TLS1_3 */

/*
 * Test clearing a connection via SSL_clear(), or resetting it via
 * SSL_set_connect_state()/SSL_set_accept_state()
 * Test 0: SSL_set_connect_state, TLSv1.3
 * Test 1: SSL_set_connect_state, TLSv1.2
 * Test 2: SSL_set_accept_state, TLSv1.3
 * Test 3: SSL_set_accept_state, TLSv1.2
 * Test 4: SSL_clear (client), TLSv1.3
 * Test 5: SSL_clear (client), TLSv1.2
 * Test 6: SSL_clear (server), TLSv1.3
 * Test 7: SSL_clear (server), TLSv1.2
 */
static int test_ssl_clear(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL *writer, *reader;
    int testresult = 0;
    int tls12test, servertest, cleartest;
    size_t written, readbytes;
    const char *msg = "Hello World";
    unsigned char buf[5];

    tls12test = idx & 1;
    idx >>= 1;
    servertest = idx & 1;
    idx >>= 1;
    cleartest = idx & 1;

#ifdef OPENSSL_NO_TLS1_2
    if (tls12test == 1)
        return TEST_skip("No TLSv1.2 in this build");
#endif

    /* Create an initial connection */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey))
            || (tls12test
                && !TEST_true(SSL_CTX_set_max_proto_version(cctx,
                                                            TLS1_2_VERSION)))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (servertest) {
        writer = clientssl;
        reader = serverssl;
    } else {
        writer = serverssl;
        reader = clientssl;
    }

    /* Write some data */
    if (!TEST_true(SSL_write_ex(writer, msg, strlen(msg), &written))
            || written != strlen(msg))
        goto end;

    /*
     * Read a partial record. The remaining buffered data should be cleared by
     * the subsequent clear/reset
     */
    if (!TEST_true(SSL_read_ex(reader, buf, sizeof(buf), &readbytes))
            || readbytes != sizeof(buf))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);

    /* Reset/clear one SSL object in order to reuse it. We free the other one */
    if (servertest) {
        if (cleartest) {
            if (!TEST_true(SSL_clear(serverssl)))
                goto end;
        } else {
            SSL_set_accept_state(serverssl);
        }
        SSL_free(clientssl);
        clientssl = NULL;
    } else {
        if (cleartest) {
            if (!TEST_true(SSL_clear(clientssl)))
                goto end;
        } else {
            SSL_set_connect_state(clientssl);
        }
        SSL_free(serverssl);
        serverssl = NULL;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_true(servertest || SSL_session_reused(clientssl)))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/* Parse CH and retrieve any MFL extension value if present */
static int get_MFL_from_client_hello(BIO *bio, int *mfl_codemfl_code)
{
    long len;
    unsigned char *data;
    PACKET pkt, pkt2, pkt3;
    unsigned int MFL_code = 0, type = 0;

    if (!TEST_uint_gt(len = BIO_get_mem_data(bio, (char **) &data), 0))
        goto end;

    memset(&pkt, 0, sizeof(pkt));
    memset(&pkt2, 0, sizeof(pkt2));
    memset(&pkt3, 0, sizeof(pkt3));

    if (!TEST_long_gt(len, 0)
            || !TEST_true(PACKET_buf_init(&pkt, data, len))
               /* Skip the record header */
            || !PACKET_forward(&pkt, SSL3_RT_HEADER_LENGTH)
               /* Skip the handshake message header */
            || !TEST_true(PACKET_forward(&pkt, SSL3_HM_HEADER_LENGTH))
               /* Skip client version and random */
            || !TEST_true(PACKET_forward(&pkt, CLIENT_VERSION_LEN
                                               + SSL3_RANDOM_SIZE))
               /* Skip session id */
            || !TEST_true(PACKET_get_length_prefixed_1(&pkt, &pkt2))
               /* Skip ciphers */
            || !TEST_true(PACKET_get_length_prefixed_2(&pkt, &pkt2))
               /* Skip compression */
            || !TEST_true(PACKET_get_length_prefixed_1(&pkt, &pkt2))
               /* Extensions len */
            || !TEST_true(PACKET_as_length_prefixed_2(&pkt, &pkt2)))
        goto end;

    /* Loop through all extensions */
    while (PACKET_remaining(&pkt2)) {
        if (!TEST_true(PACKET_get_net_2(&pkt2, &type))
                || !TEST_true(PACKET_get_length_prefixed_2(&pkt2, &pkt3)))
            goto end;

        if (type == TLSEXT_TYPE_max_fragment_length) {
            if (!TEST_uint_ne(PACKET_remaining(&pkt3), 0)
                    || !TEST_true(PACKET_get_1(&pkt3, &MFL_code)))
                goto end;

            *mfl_codemfl_code = MFL_code;
            return 1;
        }
    }

 end:
    return 0;
}

/* Maximum-Fragment-Length TLS extension mode to test */
static const unsigned char max_fragment_len_test[] = {
    TLSEXT_max_fragment_length_512,
    TLSEXT_max_fragment_length_1024,
    TLSEXT_max_fragment_length_2048,
    TLSEXT_max_fragment_length_4096
};

static int test_max_fragment_len_ext(int idx_tst)
{
    SSL_CTX *ctx = NULL;
    SSL *con = NULL;
    int testresult = 0, MFL_mode = 0;
    BIO *rbio, *wbio;

    if (!TEST_true(create_ssl_ctx_pair(libctx, NULL, TLS_client_method(),
                                       TLS1_VERSION, 0, NULL, &ctx, NULL,
                                       NULL)))
        return 0;

    if (!TEST_true(SSL_CTX_set_tlsext_max_fragment_length(
                   ctx, max_fragment_len_test[idx_tst])))
        goto end;

    con = SSL_new(ctx);
    if (!TEST_ptr(con))
        goto end;

    rbio = BIO_new(BIO_s_mem());
    wbio = BIO_new(BIO_s_mem());
    if (!TEST_ptr(rbio)|| !TEST_ptr(wbio)) {
        BIO_free(rbio);
        BIO_free(wbio);
        goto end;
    }

    SSL_set_bio(con, rbio, wbio);

    if (!TEST_int_le(SSL_connect(con), 0)) {
        /* This shouldn't succeed because we don't have a server! */
        goto end;
    }

    if (!TEST_true(get_MFL_from_client_hello(wbio, &MFL_mode)))
        /* no MFL in client hello */
        goto end;
    if (!TEST_true(max_fragment_len_test[idx_tst] == MFL_mode))
        goto end;

    testresult = 1;

end:
    SSL_free(con);
    SSL_CTX_free(ctx);

    return testresult;
}

#ifndef OSSL_NO_USABLE_TLS1_3
static int test_pha_key_update(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        return 0;

    if (!TEST_true(SSL_CTX_set_min_proto_version(sctx, TLS1_3_VERSION))
        || !TEST_true(SSL_CTX_set_max_proto_version(sctx, TLS1_3_VERSION))
        || !TEST_true(SSL_CTX_set_min_proto_version(cctx, TLS1_3_VERSION))
        || !TEST_true(SSL_CTX_set_max_proto_version(cctx, TLS1_3_VERSION)))
        goto end;

    SSL_CTX_set_post_handshake_auth(cctx, 1);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    SSL_set_verify(serverssl, SSL_VERIFY_PEER, NULL);
    if (!TEST_true(SSL_verify_client_post_handshake(serverssl)))
        goto end;

    if (!TEST_true(SSL_key_update(clientssl, SSL_KEY_UPDATE_NOT_REQUESTED)))
        goto end;

    /* Start handshake on the server */
    if (!TEST_int_eq(SSL_do_handshake(serverssl), 1))
        goto end;

    /* Starts with SSL_connect(), but it's really just SSL_do_handshake() */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                         SSL_ERROR_NONE)))
        goto end;

    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}
#endif

#if !defined(OPENSSL_NO_SRP) && !defined(OPENSSL_NO_TLS1_2)

static SRP_VBASE *vbase = NULL;

static int ssl_srp_cb(SSL *s, int *ad, void *arg)
{
    int ret = SSL3_AL_FATAL;
    char *username;
    SRP_user_pwd *user = NULL;

    username = SSL_get_srp_username(s);
    if (username == NULL) {
        *ad = SSL_AD_INTERNAL_ERROR;
        goto err;
    }

    user = SRP_VBASE_get1_by_user(vbase, username);
    if (user == NULL) {
        *ad = SSL_AD_INTERNAL_ERROR;
        goto err;
    }

    if (SSL_set_srp_server_param(s, user->N, user->g, user->s, user->v,
                                 user->info) <= 0) {
        *ad = SSL_AD_INTERNAL_ERROR;
        goto err;
    }

    ret = 0;

 err:
    SRP_user_pwd_free(user);
    return ret;
}

static int create_new_vfile(char *userid, char *password, const char *filename)
{
    char *gNid = NULL;
    OPENSSL_STRING *row = OPENSSL_zalloc(sizeof(row) * (DB_NUMBER + 1));
    TXT_DB *db = NULL;
    int ret = 0;
    BIO *out = NULL, *dummy = BIO_new_mem_buf("", 0);
    size_t i;

    if (!TEST_ptr(dummy) || !TEST_ptr(row))
        goto end;

    gNid = SRP_create_verifier_ex(userid, password, &row[DB_srpsalt],
                                  &row[DB_srpverifier], NULL, NULL, libctx, NULL);
    if (!TEST_ptr(gNid))
        goto end;

    /*
     * The only way to create an empty TXT_DB is to provide a BIO with no data
     * in it!
     */
    db = TXT_DB_read(dummy, DB_NUMBER);
    if (!TEST_ptr(db))
        goto end;

    out = BIO_new_file(filename, "w");
    if (!TEST_ptr(out))
        goto end;

    row[DB_srpid] = OPENSSL_strdup(userid);
    row[DB_srptype] = OPENSSL_strdup("V");
    row[DB_srpgN] = OPENSSL_strdup(gNid);

    if (!TEST_ptr(row[DB_srpid])
            || !TEST_ptr(row[DB_srptype])
            || !TEST_ptr(row[DB_srpgN])
            || !TEST_true(TXT_DB_insert(db, row)))
        goto end;

    row = NULL;

    if (TXT_DB_write(out, db) <= 0)
        goto end;

    ret = 1;
 end:
    if (row != NULL) {
        for (i = 0; i < DB_NUMBER; i++)
            OPENSSL_free(row[i]);
    }
    OPENSSL_free(row);
    BIO_free(dummy);
    BIO_free(out);
    TXT_DB_free(db);

    return ret;
}

static int create_new_vbase(char *userid, char *password)
{
    BIGNUM *verifier = NULL, *salt = NULL;
    const SRP_gN *lgN = NULL;
    SRP_user_pwd *user_pwd = NULL;
    int ret = 0;

    lgN = SRP_get_default_gN(NULL);
    if (!TEST_ptr(lgN))
        goto end;

    if (!TEST_true(SRP_create_verifier_BN_ex(userid, password, &salt, &verifier,
                                             lgN->N, lgN->g, libctx, NULL)))
        goto end;

    user_pwd = OPENSSL_zalloc(sizeof(*user_pwd));
    if (!TEST_ptr(user_pwd))
        goto end;

    user_pwd->N = lgN->N;
    user_pwd->g = lgN->g;
    user_pwd->id = OPENSSL_strdup(userid);
    if (!TEST_ptr(user_pwd->id))
        goto end;

    user_pwd->v = verifier;
    user_pwd->s = salt;
    verifier = salt = NULL;

    if (sk_SRP_user_pwd_insert(vbase->users_pwd, user_pwd, 0) == 0)
        goto end;
    user_pwd = NULL;

    ret = 1;
end:
    SRP_user_pwd_free(user_pwd);
    BN_free(salt);
    BN_free(verifier);

    return ret;
}

/*
 * SRP tests
 *
 * Test 0: Simple successful SRP connection, new vbase
 * Test 1: Connection failure due to bad password, new vbase
 * Test 2: Simple successful SRP connection, vbase loaded from existing file
 * Test 3: Connection failure due to bad password, vbase loaded from existing
 *         file
 * Test 4: Simple successful SRP connection, vbase loaded from new file
 * Test 5: Connection failure due to bad password, vbase loaded from new file
 */
static int test_srp(int tst)
{
    char *userid = "test", *password = "password", *tstsrpfile;
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int ret, testresult = 0;

    vbase = SRP_VBASE_new(NULL);
    if (!TEST_ptr(vbase))
        goto end;

    if (tst == 0 || tst == 1) {
        if (!TEST_true(create_new_vbase(userid, password)))
            goto end;
    } else {
        if (tst == 4 || tst == 5) {
            if (!TEST_true(create_new_vfile(userid, password, tmpfilename)))
                goto end;
            tstsrpfile = tmpfilename;
        } else {
            tstsrpfile = srpvfile;
        }
        if (!TEST_int_eq(SRP_VBASE_init(vbase, tstsrpfile), SRP_NO_ERROR))
            goto end;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set_srp_username_callback(sctx, ssl_srp_cb), 0)
            || !TEST_true(SSL_CTX_set_cipher_list(cctx, "SRP-AES-128-CBC-SHA"))
            || !TEST_true(SSL_CTX_set_max_proto_version(sctx, TLS1_2_VERSION))
            || !TEST_true(SSL_CTX_set_max_proto_version(cctx, TLS1_2_VERSION))
            || !TEST_int_gt(SSL_CTX_set_srp_username(cctx, userid), 0))
        goto end;

    if (tst % 2 == 1) {
        if (!TEST_int_gt(SSL_CTX_set_srp_password(cctx, "badpass"), 0))
            goto end;
    } else {
        if (!TEST_int_gt(SSL_CTX_set_srp_password(cctx, password), 0))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    ret = create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE);
    if (ret) {
        if (!TEST_true(tst % 2 == 0))
            goto end;
    } else {
        if (!TEST_true(tst % 2 == 1))
            goto end;
    }

    testresult = 1;

 end:
    SRP_VBASE_free(vbase);
    vbase = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif

static int info_cb_failed = 0;
static int info_cb_offset = 0;
static int info_cb_this_state = -1;

static struct info_cb_states_st {
    int where;
    const char *statestr;
} info_cb_states[][60] = {
    {
        /* TLSv1.2 server followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWSC"}, {SSL_CB_LOOP, "TWSKE"}, {SSL_CB_LOOP, "TWSD"},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWSD"}, {SSL_CB_LOOP, "TRCKE"},
        {SSL_CB_LOOP, "TRCCS"}, {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TWST"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL},
        {SSL_CB_ALERT, NULL}, {SSL_CB_HANDSHAKE_START, NULL},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"},
        {SSL_CB_LOOP, "TWSH"}, {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWFIN"}, {SSL_CB_LOOP, "TRCCS"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.2 client followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWCH"},
        {SSL_CB_LOOP, "TRSH"}, {SSL_CB_LOOP, "TRSC"}, {SSL_CB_LOOP, "TRSKE"},
        {SSL_CB_LOOP, "TRSD"}, {SSL_CB_LOOP, "TWCKE"}, {SSL_CB_LOOP, "TWCCS"},
        {SSL_CB_LOOP, "TWFIN"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_LOOP, "TRST"}, {SSL_CB_LOOP, "TRCCS"}, {SSL_CB_LOOP, "TRFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL}, {SSL_CB_ALERT, NULL},
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWCH"},
        {SSL_CB_LOOP, "TRSH"}, {SSL_CB_LOOP, "TRCCS"}, {SSL_CB_LOOP, "TRFIN"},
        {SSL_CB_LOOP, "TWCCS"},  {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 server followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWEE"}, {SSL_CB_LOOP, "TWSC"},
        {SSL_CB_LOOP, "TWSCV"}, {SSL_CB_LOOP, "TWFIN"}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TED"}, {SSL_CB_LOOP, "TRFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_LOOP, "TWST"},
        {SSL_CB_LOOP, "TWST"}, {SSL_CB_EXIT, NULL}, {SSL_CB_ALERT, NULL},
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWEE"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_LOOP, "TED"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_LOOP, "TWST"}, {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 client followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWCH"},
        {SSL_CB_LOOP, "TRSH"}, {SSL_CB_LOOP, "TREE"}, {SSL_CB_LOOP, "TRSC"},
        {SSL_CB_LOOP, "TRSCV"}, {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TWCCS"},
        {SSL_CB_LOOP, "TWFIN"},  {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "SSLOK"},
        {SSL_CB_LOOP, "TRST"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "SSLOK"},
        {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "TRST"}, {SSL_CB_EXIT, NULL},
        {SSL_CB_ALERT, NULL}, {SSL_CB_HANDSHAKE_START, NULL},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_LOOP, "TRSH"},  {SSL_CB_LOOP, "TREE"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL},
        {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "TRST"},
        {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 server, early_data */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWEE"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL},
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_LOOP, "TED"}, {SSL_CB_LOOP, "TWEOED"}, {SSL_CB_LOOP, "TRFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_LOOP, "TWST"},
        {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 client, early_data */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_LOOP, "TWCCS"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL},
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_LOOP, "TED"}, {SSL_CB_LOOP, "TRSH"}, {SSL_CB_LOOP, "TREE"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TPEDE"}, {SSL_CB_LOOP, "TWEOED"},
        {SSL_CB_LOOP, "TWFIN"}, {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "SSLOK"},
        {SSL_CB_LOOP, "TRST"}, {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 server, certificate compression, followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWEE"}, {SSL_CB_LOOP, "TWSCC"},
        {SSL_CB_LOOP, "TWSCV"}, {SSL_CB_LOOP, "TWFIN"}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TED"}, {SSL_CB_LOOP, "TRFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_LOOP, "TWST"},
        {SSL_CB_LOOP, "TWST"}, {SSL_CB_EXIT, NULL}, {SSL_CB_ALERT, NULL},
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TRCH"}, {SSL_CB_LOOP, "TWSH"},
        {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWEE"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_LOOP, "TED"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TED"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_LOOP, "TWST"}, {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        /* TLSv1.3 client, certificate compression, followed by resumption */
        {SSL_CB_HANDSHAKE_START, NULL}, {SSL_CB_LOOP, "PINIT"},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "TWCH"},
        {SSL_CB_LOOP, "TRSH"}, {SSL_CB_LOOP, "TREE"}, {SSL_CB_LOOP, "TRSCC"},
        {SSL_CB_LOOP, "TRSCV"}, {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TWCCS"},
        {SSL_CB_LOOP, "TWFIN"},  {SSL_CB_HANDSHAKE_DONE, NULL},
        {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "SSLOK"},
        {SSL_CB_LOOP, "TRST"}, {SSL_CB_EXIT, NULL}, {SSL_CB_LOOP, "SSLOK"},
        {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "TRST"}, {SSL_CB_EXIT, NULL},
        {SSL_CB_ALERT, NULL}, {SSL_CB_HANDSHAKE_START, NULL},
        {SSL_CB_LOOP, "PINIT"}, {SSL_CB_LOOP, "TWCH"}, {SSL_CB_EXIT, NULL},
        {SSL_CB_LOOP, "TWCH"}, {SSL_CB_LOOP, "TRSH"},  {SSL_CB_LOOP, "TREE"},
        {SSL_CB_LOOP, "TRFIN"}, {SSL_CB_LOOP, "TWCCS"}, {SSL_CB_LOOP, "TWFIN"},
        {SSL_CB_HANDSHAKE_DONE, NULL}, {SSL_CB_EXIT, NULL},
        {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "SSLOK"}, {SSL_CB_LOOP, "TRST"},
        {SSL_CB_EXIT, NULL}, {0, NULL},
    }, {
        {0, NULL},
    }
};

static void sslapi_info_callback(const SSL *s, int where, int ret)
{
    struct info_cb_states_st *state = info_cb_states[info_cb_offset];

    /* We do not ever expect a connection to fail in this test */
    if (!TEST_false(ret == 0)) {
        info_cb_failed = 1;
        return;
    }

    /*
     * Do some sanity checks. We never expect these things to happen in this
     * test
     */
    if (!TEST_false((SSL_is_server(s) && (where & SSL_ST_CONNECT) != 0))
            || !TEST_false(!SSL_is_server(s) && (where & SSL_ST_ACCEPT) != 0)
            || !TEST_int_ne(state[++info_cb_this_state].where, 0)) {
        info_cb_failed = 1;
        return;
    }

    /* Now check we're in the right state */
    if (!TEST_true((where & state[info_cb_this_state].where) != 0)) {
        info_cb_failed = 1;
        return;
    }
    if ((where & SSL_CB_LOOP) != 0
            && !TEST_int_eq(strcmp(SSL_state_string(s),
                            state[info_cb_this_state].statestr), 0)) {
        info_cb_failed = 1;
        return;
    }

    /*
     * Check that, if we've got SSL_CB_HANDSHAKE_DONE we are not in init
     */
    if ((where & SSL_CB_HANDSHAKE_DONE)
            && SSL_in_init((SSL *)s) != 0) {
        info_cb_failed = 1;
        return;
    }
}

/*
 * Test the info callback gets called when we expect it to.
 *
 * Test 0: TLSv1.2, server
 * Test 1: TLSv1.2, client
 * Test 2: TLSv1.3, server
 * Test 3: TLSv1.3, client
 * Test 4: TLSv1.3, server, early_data
 * Test 5: TLSv1.3, client, early_data
 * Test 6: TLSv1.3, server, compressed certificate
 * Test 7: TLSv1.3, client, compressed certificate
 */
static int test_info_callback(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *clntsess = NULL;
    int testresult = 0;
    int tlsvers;

    if (tst < 2) {
/* We need either ECDHE or DHE for the TLSv1.2 test to work */
#if !defined(OPENSSL_NO_TLS1_2) && (!defined(OPENSSL_NO_EC) \
                                    || !defined(OPENSSL_NO_DH))
        tlsvers = TLS1_2_VERSION;
#else
        return 1;
#endif
    } else {
#ifndef OSSL_NO_USABLE_TLS1_3
        tlsvers = TLS1_3_VERSION;
#else
        return 1;
#endif
    }

    /* Reset globals */
    info_cb_failed = 0;
    info_cb_this_state = -1;
    info_cb_offset = tst;

#ifndef OSSL_NO_USABLE_TLS1_3
    if (tst >= 4 && tst < 6) {
        SSL_SESSION *sess = NULL;
        size_t written, readbytes;
        unsigned char buf[80];
        OSSL_TIME timer;

        /* early_data tests */
        if (!TEST_true(setupearly_data_test(&cctx, &sctx, &clientssl,
                                            &serverssl, &sess, 0,
                                            SHA384_DIGEST_LENGTH)))
            goto end;

        /* We don't actually need this reference */
        SSL_SESSION_free(sess);

        SSL_set_info_callback((tst % 2) == 0 ? serverssl : clientssl,
                              sslapi_info_callback);

        /* Write and read some early data and then complete the connection */
        timer = ossl_time_now();
        if (!TEST_true(SSL_write_early_data(clientssl, MSG1, strlen(MSG1),
                                            &written))
                || !TEST_size_t_eq(written, strlen(MSG1)))
            goto end;

        if (!TEST_int_eq(SSL_read_early_data(serverssl, buf,
                                             sizeof(buf), &readbytes),
                         SSL_READ_EARLY_DATA_SUCCESS)) {
            testresult = check_early_data_timeout(timer);
            goto end;
        }

        if (!TEST_mem_eq(MSG1, readbytes, buf, strlen(MSG1))
                || !TEST_int_eq(SSL_get_early_data_status(serverssl),
                                SSL_EARLY_DATA_ACCEPTED)
                || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                    SSL_ERROR_NONE))
                || !TEST_false(info_cb_failed))
            goto end;

        testresult = 1;
        goto end;
    }
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       tlsvers, tlsvers, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    if (!TEST_true(SSL_CTX_set_dh_auto(sctx, 1)))
        goto end;

    /*
     * For even numbered tests we check the server callbacks. For odd numbers we
     * check the client.
     */
    SSL_CTX_set_info_callback((tst % 2) == 0 ? sctx : cctx,
                              sslapi_info_callback);
    if (tst >= 6) {
        if (!SSL_CTX_compress_certs(sctx, 0))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                          &clientssl, NULL, NULL))
        || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                            SSL_ERROR_NONE))
        || !TEST_false(info_cb_failed))
    goto end;



    clntsess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Now do a resumption */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL))
            || !TEST_true(SSL_set_session(clientssl, clntsess))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE))
            || !TEST_true(SSL_session_reused(clientssl))
            || !TEST_false(info_cb_failed))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_SESSION_free(clntsess);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

static int test_ssl_pending(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char msg[] = "A test message";
    char buf[5];
    size_t written, readbytes;

    if (tst == 0) {
        if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                           TLS_client_method(),
                                           TLS1_VERSION, 0,
                                           &sctx, &cctx, cert, privkey)))
            goto end;
    } else {
#ifndef OPENSSL_NO_DTLS
        if (!TEST_true(create_ssl_ctx_pair(libctx, DTLS_server_method(),
                                           DTLS_client_method(),
                                           DTLS1_VERSION, 0,
                                           &sctx, &cctx, cert, privkey)))
            goto end;

# ifdef OPENSSL_NO_DTLS1_2
        /* Not supported in the FIPS provider */
        if (is_fips) {
            testresult = 1;
            goto end;
        };
        /*
         * Default sigalgs are SHA1 based in <DTLS1.2 which is in security
         * level 0
         */
        if (!TEST_true(SSL_CTX_set_cipher_list(sctx, "DEFAULT:@SECLEVEL=0"))
                || !TEST_true(SSL_CTX_set_cipher_list(cctx,
                                                    "DEFAULT:@SECLEVEL=0")))
            goto end;
# endif
#else
        return 1;
#endif
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (!TEST_int_eq(SSL_pending(clientssl), 0)
            || !TEST_false(SSL_has_pending(clientssl))
            || !TEST_int_eq(SSL_pending(serverssl), 0)
            || !TEST_false(SSL_has_pending(serverssl))
            || !TEST_true(SSL_write_ex(serverssl, msg, sizeof(msg), &written))
            || !TEST_size_t_eq(written, sizeof(msg))
            || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf), &readbytes))
            || !TEST_size_t_eq(readbytes, sizeof(buf))
            || !TEST_int_eq(SSL_pending(clientssl), (int)(written - readbytes))
            || !TEST_true(SSL_has_pending(clientssl)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static struct {
    unsigned int maxprot;
    const char *clntciphers;
    const char *clnttls13ciphers;
    const char *srvrciphers;
    const char *srvrtls13ciphers;
    const char *shared;
    const char *fipsshared;
} shared_ciphers_data[] = {
/*
 * We can't establish a connection (even in TLSv1.1) with these ciphersuites if
 * TLSv1.3 is enabled but TLSv1.2 is disabled.
 */
#if defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2)
    {
        TLS1_2_VERSION,
        "AES128-SHA:AES256-SHA",
        NULL,
        "AES256-SHA:DHE-RSA-AES128-SHA",
        NULL,
        "AES256-SHA",
        "AES256-SHA"
    },
# if !defined(OPENSSL_NO_CHACHA) \
     && !defined(OPENSSL_NO_POLY1305) \
     && !defined(OPENSSL_NO_EC)
    {
        TLS1_2_VERSION,
        "AES128-SHA:ECDHE-RSA-CHACHA20-POLY1305",
        NULL,
        "AES128-SHA:ECDHE-RSA-CHACHA20-POLY1305",
        NULL,
        "AES128-SHA:ECDHE-RSA-CHACHA20-POLY1305",
        "AES128-SHA"
    },
# endif
    {
        TLS1_2_VERSION,
        "AES128-SHA:DHE-RSA-AES128-SHA:AES256-SHA",
        NULL,
        "AES128-SHA:DHE-RSA-AES256-SHA:AES256-SHA",
        NULL,
        "AES128-SHA:AES256-SHA",
        "AES128-SHA:AES256-SHA"
    },
    {
        TLS1_2_VERSION,
        "AES128-SHA:AES256-SHA",
        NULL,
        "AES128-SHA:DHE-RSA-AES128-SHA",
        NULL,
        "AES128-SHA",
        "AES128-SHA"
    },
#endif
/*
 * This test combines TLSv1.3 and TLSv1.2 ciphersuites so they must both be
 * enabled.
 */
#if !defined(OSSL_NO_USABLE_TLS1_3) && !defined(OPENSSL_NO_TLS1_2) \
    && !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
    {
        TLS1_3_VERSION,
        "AES128-SHA:AES256-SHA",
        NULL,
        "AES256-SHA:AES128-SHA256",
        NULL,
        "TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:"
        "TLS_AES_128_GCM_SHA256:AES256-SHA",
        "TLS_AES_256_GCM_SHA384:TLS_AES_128_GCM_SHA256:AES256-SHA"
    },
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    {
        TLS1_3_VERSION,
        "AES128-SHA",
        "TLS_AES_256_GCM_SHA384",
        "AES256-SHA",
        "TLS_AES_256_GCM_SHA384",
        "TLS_AES_256_GCM_SHA384",
        "TLS_AES_256_GCM_SHA384"
    },
#endif
};

static int int_test_ssl_get_shared_ciphers(int tst, int clnt)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char buf[1024];
    OSSL_LIB_CTX *tmplibctx = OSSL_LIB_CTX_new();

    if (!TEST_ptr(tmplibctx))
        goto end;

    /*
     * Regardless of whether we're testing with the FIPS provider loaded into
     * libctx, we want one peer to always use the full set of ciphersuites
     * available. Therefore we use a separate libctx with the default provider
     * loaded into it. We run the same tests twice - once with the client side
     * having the full set of ciphersuites and once with the server side.
     */
    if (clnt) {
        cctx = SSL_CTX_new_ex(tmplibctx, NULL, TLS_client_method());
        if (!TEST_ptr(cctx))
            goto end;
    } else {
        sctx = SSL_CTX_new_ex(tmplibctx, NULL, TLS_server_method());
        if (!TEST_ptr(sctx))
            goto end;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       shared_ciphers_data[tst].maxprot,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                                        shared_ciphers_data[tst].clntciphers))
            || (shared_ciphers_data[tst].clnttls13ciphers != NULL
                && !TEST_true(SSL_CTX_set_ciphersuites(cctx,
                                    shared_ciphers_data[tst].clnttls13ciphers)))
            || !TEST_true(SSL_CTX_set_cipher_list(sctx,
                                        shared_ciphers_data[tst].srvrciphers))
            || (shared_ciphers_data[tst].srvrtls13ciphers != NULL
                && !TEST_true(SSL_CTX_set_ciphersuites(sctx,
                                    shared_ciphers_data[tst].srvrtls13ciphers))))
        goto end;


    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (!TEST_ptr(SSL_get_shared_ciphers(serverssl, buf, sizeof(buf)))
            || !TEST_int_eq(strcmp(buf,
                                   is_fips
                                   ? shared_ciphers_data[tst].fipsshared
                                   : shared_ciphers_data[tst].shared),
                                   0)) {
        TEST_info("Shared ciphers are: %s\n", buf);
        goto end;
    }

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    OSSL_LIB_CTX_free(tmplibctx);

    return testresult;
}

static int test_ssl_get_shared_ciphers(int tst)
{
    return int_test_ssl_get_shared_ciphers(tst, 0)
           && int_test_ssl_get_shared_ciphers(tst, 1);
}


static const char *appdata = "Hello World";
static int gen_tick_called, dec_tick_called, tick_key_cb_called;
static int tick_key_renew = 0;
static SSL_TICKET_RETURN tick_dec_ret = SSL_TICKET_RETURN_ABORT;

static int gen_tick_cb(SSL *s, void *arg)
{
    gen_tick_called = 1;

    return SSL_SESSION_set1_ticket_appdata(SSL_get_session(s), appdata,
                                           strlen(appdata));
}

static SSL_TICKET_RETURN dec_tick_cb(SSL *s, SSL_SESSION *ss,
                                     const unsigned char *keyname,
                                     size_t keyname_length,
                                     SSL_TICKET_STATUS status,
                                     void *arg)
{
    void *tickdata;
    size_t tickdlen;

    dec_tick_called = 1;

    if (status == SSL_TICKET_EMPTY)
        return SSL_TICKET_RETURN_IGNORE_RENEW;

    if (!TEST_true(status == SSL_TICKET_SUCCESS
                   || status == SSL_TICKET_SUCCESS_RENEW))
        return SSL_TICKET_RETURN_ABORT;

    if (!TEST_true(SSL_SESSION_get0_ticket_appdata(ss, &tickdata,
                                                   &tickdlen))
            || !TEST_size_t_eq(tickdlen, strlen(appdata))
            || !TEST_int_eq(memcmp(tickdata, appdata, tickdlen), 0))
        return SSL_TICKET_RETURN_ABORT;

    if (tick_key_cb_called)  {
        /* Don't change what the ticket key callback wanted to do */
        switch (status) {
        case SSL_TICKET_NO_DECRYPT:
            return SSL_TICKET_RETURN_IGNORE_RENEW;

        case SSL_TICKET_SUCCESS:
            return SSL_TICKET_RETURN_USE;

        case SSL_TICKET_SUCCESS_RENEW:
            return SSL_TICKET_RETURN_USE_RENEW;

        default:
            return SSL_TICKET_RETURN_ABORT;
        }
    }
    return tick_dec_ret;

}

#ifndef OPENSSL_NO_DEPRECATED_3_0
static int tick_key_cb(SSL *s, unsigned char key_name[16],
                       unsigned char iv[EVP_MAX_IV_LENGTH], EVP_CIPHER_CTX *ctx,
                       HMAC_CTX *hctx, int enc)
{
    const unsigned char tick_aes_key[16] = "0123456789abcdef";
    const unsigned char tick_hmac_key[16] = "0123456789abcdef";
    EVP_CIPHER *aes128cbc;
    EVP_MD *sha256;
    int ret;

    tick_key_cb_called = 1;

    if (tick_key_renew == -1)
        return 0;

    aes128cbc = EVP_CIPHER_fetch(libctx, "AES-128-CBC", NULL);
    if (!TEST_ptr(aes128cbc))
        return 0;
    sha256 = EVP_MD_fetch(libctx, "SHA-256", NULL);
    if (!TEST_ptr(sha256)) {
        EVP_CIPHER_free(aes128cbc);
        return 0;
    }

    memset(iv, 0, AES_BLOCK_SIZE);
    memset(key_name, 0, 16);
    if (aes128cbc == NULL
            || sha256 == NULL
            || !EVP_CipherInit_ex(ctx, aes128cbc, NULL, tick_aes_key, iv, enc)
            || !HMAC_Init_ex(hctx, tick_hmac_key, sizeof(tick_hmac_key), sha256,
                             NULL))
        ret = -1;
    else
        ret = tick_key_renew ? 2 : 1;

    EVP_CIPHER_free(aes128cbc);
    EVP_MD_free(sha256);

    return ret;
}
#endif

static int tick_key_evp_cb(SSL *s, unsigned char key_name[16],
                           unsigned char iv[EVP_MAX_IV_LENGTH],
                           EVP_CIPHER_CTX *ctx, EVP_MAC_CTX *hctx, int enc)
{
    const unsigned char tick_aes_key[16] = "0123456789abcdef";
    unsigned char tick_hmac_key[16] = "0123456789abcdef";
    OSSL_PARAM params[2];
    EVP_CIPHER *aes128cbc;
    int ret;

    tick_key_cb_called = 1;

    if (tick_key_renew == -1)
        return 0;

    aes128cbc = EVP_CIPHER_fetch(libctx, "AES-128-CBC", NULL);
    if (!TEST_ptr(aes128cbc))
        return 0;

    memset(iv, 0, AES_BLOCK_SIZE);
    memset(key_name, 0, 16);
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                                 "SHA256", 0);
    params[1] = OSSL_PARAM_construct_end();
    if (aes128cbc == NULL
            || !EVP_CipherInit_ex(ctx, aes128cbc, NULL, tick_aes_key, iv, enc)
            || !EVP_MAC_init(hctx, tick_hmac_key, sizeof(tick_hmac_key),
                             params))
        ret = -1;
    else
        ret = tick_key_renew ? 2 : 1;

    EVP_CIPHER_free(aes128cbc);

    return ret;
}

/*
 * Test the various ticket callbacks
 * Test 0: TLSv1.2, no ticket key callback, no ticket, no renewal
 * Test 1: TLSv1.3, no ticket key callback, no ticket, no renewal
 * Test 2: TLSv1.2, no ticket key callback, no ticket, renewal
 * Test 3: TLSv1.3, no ticket key callback, no ticket, renewal
 * Test 4: TLSv1.2, no ticket key callback, ticket, no renewal
 * Test 5: TLSv1.3, no ticket key callback, ticket, no renewal
 * Test 6: TLSv1.2, no ticket key callback, ticket, renewal
 * Test 7: TLSv1.3, no ticket key callback, ticket, renewal
 * Test 8: TLSv1.2, old ticket key callback, ticket, no renewal
 * Test 9: TLSv1.3, old ticket key callback, ticket, no renewal
 * Test 10: TLSv1.2, old ticket key callback, ticket, renewal
 * Test 11: TLSv1.3, old ticket key callback, ticket, renewal
 * Test 12: TLSv1.2, old ticket key callback, no ticket
 * Test 13: TLSv1.3, old ticket key callback, no ticket
 * Test 14: TLSv1.2, ticket key callback, ticket, no renewal
 * Test 15: TLSv1.3, ticket key callback, ticket, no renewal
 * Test 16: TLSv1.2, ticket key callback, ticket, renewal
 * Test 17: TLSv1.3, ticket key callback, ticket, renewal
 * Test 18: TLSv1.2, ticket key callback, no ticket
 * Test 19: TLSv1.3, ticket key callback, no ticket
 */
static int test_ticket_callbacks(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *clntsess = NULL;
    int testresult = 0;

#ifdef OPENSSL_NO_TLS1_2
    if (tst % 2 == 0)
        return 1;
#endif
#ifdef OSSL_NO_USABLE_TLS1_3
    if (tst % 2 == 1)
        return 1;
#endif
#ifdef OPENSSL_NO_DEPRECATED_3_0
    if (tst >= 8 && tst <= 13)
        return 1;
#endif

    gen_tick_called = dec_tick_called = tick_key_cb_called = 0;

    /* Which tests the ticket key callback should request renewal for */

    if (tst == 10 || tst == 11 || tst == 16 || tst == 17)
        tick_key_renew = 1;
    else if (tst == 12 || tst == 13 || tst == 18 || tst == 19)
        tick_key_renew = -1; /* abort sending the ticket/0-length ticket */
    else
        tick_key_renew = 0;

    /* Which tests the decrypt ticket callback should request renewal for */
    switch (tst) {
    case 0:
    case 1:
        tick_dec_ret = SSL_TICKET_RETURN_IGNORE;
        break;

    case 2:
    case 3:
        tick_dec_ret = SSL_TICKET_RETURN_IGNORE_RENEW;
        break;

    case 4:
    case 5:
        tick_dec_ret = SSL_TICKET_RETURN_USE;
        break;

    case 6:
    case 7:
        tick_dec_ret = SSL_TICKET_RETURN_USE_RENEW;
        break;

    default:
        tick_dec_ret = SSL_TICKET_RETURN_ABORT;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       ((tst % 2) == 0) ? TLS1_2_VERSION
                                                        : TLS1_3_VERSION,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    /*
     * We only want sessions to resume from tickets - not the session cache. So
     * switch the cache off.
     */
    if (!TEST_true(SSL_CTX_set_session_cache_mode(sctx, SSL_SESS_CACHE_OFF)))
        goto end;

    if (!TEST_true(SSL_CTX_set_session_ticket_cb(sctx, gen_tick_cb, dec_tick_cb,
                                                 NULL)))
        goto end;

    if (tst >= 14) {
        if (!TEST_true(SSL_CTX_set_tlsext_ticket_key_evp_cb(sctx, tick_key_evp_cb)))
            goto end;
#ifndef OPENSSL_NO_DEPRECATED_3_0
    } else if (tst >= 8) {
        if (!TEST_true(SSL_CTX_set_tlsext_ticket_key_cb(sctx, tick_key_cb)))
            goto end;
#endif
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    /*
     * The decrypt ticket key callback in TLSv1.2 should be called even though
     * we have no ticket yet, because it gets called with a status of
     * SSL_TICKET_EMPTY (the client indicates support for tickets but does not
     * actually send any ticket data). This does not happen in TLSv1.3 because
     * it is not valid to send empty ticket data in TLSv1.3.
     */
    if (!TEST_int_eq(gen_tick_called, 1)
            || !TEST_int_eq(dec_tick_called, ((tst % 2) == 0) ? 1 : 0))
        goto end;

    gen_tick_called = dec_tick_called = 0;

    clntsess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /* Now do a resumption */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL))
            || !TEST_true(SSL_set_session(clientssl, clntsess))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    if (tick_dec_ret == SSL_TICKET_RETURN_IGNORE
            || tick_dec_ret == SSL_TICKET_RETURN_IGNORE_RENEW
            || tick_key_renew == -1) {
        if (!TEST_false(SSL_session_reused(clientssl)))
            goto end;
    } else {
        if (!TEST_true(SSL_session_reused(clientssl)))
            goto end;
    }

    if (!TEST_int_eq(gen_tick_called,
                     (tick_key_renew
                      || tick_dec_ret == SSL_TICKET_RETURN_IGNORE_RENEW
                      || tick_dec_ret == SSL_TICKET_RETURN_USE_RENEW)
                     ? 1 : 0)
               /* There is no ticket to decrypt in tests 13 and 19 */
            || !TEST_int_eq(dec_tick_called, (tst == 13 || tst == 19) ? 0 : 1))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(clntsess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test incorrect shutdown.
 * Test 0: client does not shutdown properly,
 *         server does not set SSL_OP_IGNORE_UNEXPECTED_EOF,
 *         server should get SSL_ERROR_SSL
 * Test 1: client does not shutdown properly,
 *         server sets SSL_OP_IGNORE_UNEXPECTED_EOF,
 *         server should get SSL_ERROR_ZERO_RETURN
 */
static int test_incorrect_shutdown(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char buf[80];
    BIO *c2s;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (tst == 1)
        SSL_CTX_set_options(sctx, SSL_OP_IGNORE_UNEXPECTED_EOF);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                            NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE)))
        goto end;

    c2s = SSL_get_rbio(serverssl);
    BIO_set_mem_eof_return(c2s, 0);

    if (!TEST_false(SSL_read(serverssl, buf, sizeof(buf))))
        goto end;

    if (tst == 0 && !TEST_int_eq(SSL_get_error(serverssl, 0), SSL_ERROR_SSL) )
        goto end;
    if (tst == 1 && !TEST_int_eq(SSL_get_error(serverssl, 0), SSL_ERROR_ZERO_RETURN) )
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test bi-directional shutdown.
 * Test 0: TLSv1.2
 * Test 1: TLSv1.2, server continues to read/write after client shutdown
 * Test 2: TLSv1.3, no pending NewSessionTicket messages
 * Test 3: TLSv1.3, pending NewSessionTicket messages
 * Test 4: TLSv1.3, server continues to read/write after client shutdown, server
 *                  sends key update, client reads it
 * Test 5: TLSv1.3, server continues to read/write after client shutdown, server
 *                  sends CertificateRequest, client reads and ignores it
 * Test 6: TLSv1.3, server continues to read/write after client shutdown, client
 *                  doesn't read it
 */
static int test_shutdown(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char msg[] = "A test message";
    char buf[80];
    size_t written, readbytes;
    SSL_SESSION *sess;

#ifdef OPENSSL_NO_TLS1_2
    if (tst <= 1)
        return 1;
#endif
#ifdef OSSL_NO_USABLE_TLS1_3
    if (tst >= 2)
        return 1;
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       (tst <= 1) ? TLS1_2_VERSION
                                                  : TLS1_3_VERSION,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (tst == 5)
        SSL_CTX_set_post_handshake_auth(cctx, 1);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    if (tst == 3) {
        if (!TEST_true(create_bare_ssl_connection(serverssl, clientssl,
                                                  SSL_ERROR_NONE, 1, 0))
                || !TEST_ptr_ne(sess = SSL_get_session(clientssl), NULL)
                || !TEST_false(SSL_SESSION_is_resumable(sess)))
            goto end;
    } else if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE))
            || !TEST_ptr_ne(sess = SSL_get_session(clientssl), NULL)
            || !TEST_true(SSL_SESSION_is_resumable(sess))) {
        goto end;
    }

    if (!TEST_int_eq(SSL_shutdown(clientssl), 0))
        goto end;

    if (tst >= 4) {
        /*
         * Reading on the server after the client has sent close_notify should
         * fail and provide SSL_ERROR_ZERO_RETURN
         */
        if (!TEST_false(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes))
                || !TEST_int_eq(SSL_get_error(serverssl, 0),
                                SSL_ERROR_ZERO_RETURN)
                || !TEST_int_eq(SSL_get_shutdown(serverssl),
                                SSL_RECEIVED_SHUTDOWN)
                   /*
                    * Even though we're shutdown on receive we should still be
                    * able to write.
                    */
                || !TEST_true(SSL_write(serverssl, msg, sizeof(msg))))
            goto end;
        if (tst == 4
                && !TEST_true(SSL_key_update(serverssl,
                                             SSL_KEY_UPDATE_REQUESTED)))
            goto end;
        if (tst == 5) {
            SSL_set_verify(serverssl, SSL_VERIFY_PEER, NULL);
            if (!TEST_true(SSL_verify_client_post_handshake(serverssl)))
                goto end;
        }
        if ((tst == 4 || tst == 5)
                && !TEST_true(SSL_write(serverssl, msg, sizeof(msg))))
            goto end;
        if (!TEST_int_eq(SSL_shutdown(serverssl), 1))
            goto end;
        if (tst == 4 || tst == 5) {
            /* Should still be able to read data from server */
            if (!TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf),
                                       &readbytes))
                    || !TEST_size_t_eq(readbytes, sizeof(msg))
                    || !TEST_int_eq(memcmp(msg, buf, readbytes), 0)
                    || !TEST_true(SSL_read_ex(clientssl, buf, sizeof(buf),
                                              &readbytes))
                    || !TEST_size_t_eq(readbytes, sizeof(msg))
                    || !TEST_int_eq(memcmp(msg, buf, readbytes), 0))
                goto end;
        }
    }

    /* Writing on the client after sending close_notify shouldn't be possible */
    if (!TEST_false(SSL_write_ex(clientssl, msg, sizeof(msg), &written)))
        goto end;

    if (tst < 4) {
        /*
         * For these tests the client has sent close_notify but it has not yet
         * been received by the server. The server has not sent close_notify
         * yet.
         */
        if (!TEST_int_eq(SSL_shutdown(serverssl), 0)
                   /*
                    * Writing on the server after sending close_notify shouldn't
                    * be possible.
                    */
                || !TEST_false(SSL_write_ex(serverssl, msg, sizeof(msg), &written))
                || !TEST_int_eq(SSL_shutdown(clientssl), 1)
                || !TEST_ptr_ne(sess = SSL_get_session(clientssl), NULL)
                || !TEST_true(SSL_SESSION_is_resumable(sess))
                || !TEST_int_eq(SSL_shutdown(serverssl), 1))
            goto end;
    } else if (tst == 4 || tst == 5) {
        /*
         * In this test the client has sent close_notify and it has been
         * received by the server which has responded with a close_notify. The
         * client needs to read the close_notify sent by the server.
         */
        if (!TEST_int_eq(SSL_shutdown(clientssl), 1)
                || !TEST_ptr_ne(sess = SSL_get_session(clientssl), NULL)
                || !TEST_true(SSL_SESSION_is_resumable(sess)))
            goto end;
    } else {
        /*
         * tst == 6
         *
         * The client has sent close_notify and is expecting a close_notify
         * back, but instead there is application data first. The shutdown
         * should fail with a fatal error.
         */
        if (!TEST_int_eq(SSL_shutdown(clientssl), -1)
                || !TEST_int_eq(SSL_get_error(clientssl, -1), SSL_ERROR_SSL))
            goto end;
    }

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

/*
 * Test that sending close_notify alerts works correctly in the case of a
 * retryable write failure.
 */
static int test_async_shutdown(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    BIO *bretry = BIO_new(bio_s_always_retry()), *tmp = NULL;

    if (!TEST_ptr(bretry))
        goto end;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       0, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* Close write side of clientssl */
    if (!TEST_int_eq(SSL_shutdown(clientssl), 0))
        goto end;

    tmp = SSL_get_wbio(serverssl);
    if (!TEST_true(BIO_up_ref(tmp))) {
        tmp = NULL;
        goto end;
    }
    SSL_set0_wbio(serverssl, bretry);
    bretry = NULL;

    /* First server shutdown should fail because of a retrable write failure */
    if (!TEST_int_eq(SSL_shutdown(serverssl), -1)
            || !TEST_int_eq(SSL_get_error(serverssl, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    /* Second server shutdown should fail for the same reason */
    if (!TEST_int_eq(SSL_shutdown(serverssl), -1)
            || !TEST_int_eq(SSL_get_error(serverssl, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    SSL_set0_wbio(serverssl, tmp);
    tmp = NULL;

    /* Third server shutdown should send close_notify */
    if (!TEST_int_eq(SSL_shutdown(serverssl), 0))
        goto end;

    /* Fourth server shutdown should read close_notify from client and finish */
    if (!TEST_int_eq(SSL_shutdown(serverssl), 1))
        goto end;

    /* Client should also successfully fully shutdown */
    if (!TEST_int_eq(SSL_shutdown(clientssl), 1))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    BIO_free(bretry);
    BIO_free(tmp);

    return testresult;
}

#if !defined(OPENSSL_NO_TLS1_2) || !defined(OSSL_NO_USABLE_TLS1_3)
static int cert_cb_cnt;

static int load_chain(const char *file, EVP_PKEY **pkey, X509 **x509,
                      STACK_OF(X509) *chain)
{
    char *path = test_mk_file_path(certsdir, file);
    BIO *in = NULL;
    X509 *x = NULL;
    int ok = 0;

    if (path == NULL)
        return 0;
    if ((in = BIO_new(BIO_s_file())) == NULL
        || BIO_read_filename(in, path) <= 0)
        goto out;
    if (pkey == NULL) {
        if ((x = X509_new_ex(libctx, NULL)) == NULL
            || PEM_read_bio_X509(in, &x, NULL, NULL) == NULL)
            goto out;
        if (chain == NULL)
            *x509 = x;
        else if (!sk_X509_push(chain, x))
            goto out;
    } else if (PEM_read_bio_PrivateKey_ex(in, pkey, NULL, NULL,
                                          libctx, NULL) == NULL) {
        goto out;
    }

    x = NULL;
    ok = 1;
 out:
    X509_free(x);
    BIO_free(in);
    OPENSSL_free(path);
    return ok;
}

static int cert_cb(SSL *s, void *arg)
{
    SSL_CTX *ctx = (SSL_CTX *)arg;
    EVP_PKEY *pkey = NULL;
    X509 *x509 = NULL, *x = NULL;
    STACK_OF(X509) *chain = NULL;
    int ret = 0;

    if (cert_cb_cnt == 0) {
        /* Suspend the handshake */
        cert_cb_cnt++;
        return -1;
    } else if (cert_cb_cnt == 1) {
        /*
         * Update the SSL_CTX, set the certificate and private key and then
         * continue the handshake normally.
         */
        if (ctx != NULL && !TEST_ptr(SSL_set_SSL_CTX(s, ctx)))
            return 0;

        if (!TEST_true(SSL_use_certificate_file(s, cert, SSL_FILETYPE_PEM))
                || !TEST_true(SSL_use_PrivateKey_file(s, privkey,
                                                      SSL_FILETYPE_PEM))
                || !TEST_true(SSL_check_private_key(s)))
            return 0;
        cert_cb_cnt++;
        return 1;
    } else if (cert_cb_cnt == 3) {
        int rv;

        chain = sk_X509_new_null();
        if (!TEST_ptr(chain)
            || !TEST_true(load_chain("ca-cert.pem", NULL, NULL, chain))
            || !TEST_true(load_chain("root-cert.pem", NULL, NULL, chain))
            || !TEST_true(load_chain("p256-ee-rsa-ca-cert.pem", NULL,
                                     &x509, NULL))
            || !TEST_true(load_chain("p256-ee-rsa-ca-key.pem", &pkey,
                                     NULL, NULL)))
            goto out;
        rv = SSL_check_chain(s, x509, pkey, chain);
        /*
         * If the cert doesn't show as valid here (e.g., because we don't
         * have any shared sigalgs), then we will not set it, and there will
         * be no certificate at all on the SSL or SSL_CTX.  This, in turn,
         * will cause tls_choose_sigalgs() to fail the connection.
         */
        if ((rv & (CERT_PKEY_VALID | CERT_PKEY_CA_SIGNATURE))
                == (CERT_PKEY_VALID | CERT_PKEY_CA_SIGNATURE)) {
            if (!SSL_use_cert_and_key(s, x509, pkey, NULL, 1))
                goto out;
        }

        ret = 1;
    }

    /* Abort the handshake */
 out:
    EVP_PKEY_free(pkey);
    X509_free(x509);
    X509_free(x);
    OSSL_STACK_OF_X509_free(chain);
    return ret;
}

/*
 * Test the certificate callback.
 * Test 0: Callback fails
 * Test 1: Success - no SSL_set_SSL_CTX() in the callback
 * Test 2: Success - SSL_set_SSL_CTX() in the callback
 * Test 3: Success - Call SSL_check_chain from the callback
 * Test 4: Failure - SSL_check_chain fails from callback due to bad cert in the
 *                   chain
 * Test 5: Failure - SSL_check_chain fails from callback due to bad ee cert
 */
static int test_cert_cb_int(int prot, int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL, *snictx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, ret;

#ifdef OPENSSL_NO_EC
    /* We use an EC cert in these tests, so we skip in a no-ec build */
    if (tst >= 3)
        return 1;
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       prot,
                                       prot,
                                       &sctx, &cctx, NULL, NULL)))
        goto end;

    if (tst == 0)
        cert_cb_cnt = -1;
    else if (tst >= 3)
        cert_cb_cnt = 3;
    else
        cert_cb_cnt = 0;

    if (tst == 2) {
        snictx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
        if (!TEST_ptr(snictx))
            goto end;
    }

    SSL_CTX_set_cert_cb(sctx, cert_cb, snictx);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (tst == 4) {
        /*
         * We cause SSL_check_chain() to fail by specifying sig_algs that
         * the chain doesn't meet (the root uses an RSA cert)
         */
        if (!TEST_true(SSL_set1_sigalgs_list(clientssl,
                                             "ecdsa_secp256r1_sha256")))
            goto end;
    } else if (tst == 5) {
        /*
         * We cause SSL_check_chain() to fail by specifying sig_algs that
         * the ee cert doesn't meet (the ee uses an ECDSA cert)
         */
        if (!TEST_true(SSL_set1_sigalgs_list(clientssl,
                           "rsa_pss_rsae_sha256:rsa_pkcs1_sha256")))
            goto end;
    }

    ret = create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE);
    if (!TEST_true(tst == 0 || tst == 4 || tst == 5 ? !ret : ret)
            || (tst > 0
                && !TEST_int_eq((cert_cb_cnt - 2) * (cert_cb_cnt - 3), 0))) {
        goto end;
    }

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_CTX_free(snictx);

    return testresult;
}
#endif

static int test_cert_cb(int tst)
{
    int testresult = 1;

#ifndef OPENSSL_NO_TLS1_2
    testresult &= test_cert_cb_int(TLS1_2_VERSION, tst);
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    testresult &= test_cert_cb_int(TLS1_3_VERSION, tst);
#endif

    return testresult;
}

static int client_cert_cb(SSL *ssl, X509 **x509, EVP_PKEY **pkey)
{
    X509 *xcert;
    EVP_PKEY *privpkey;
    BIO *in = NULL;
    BIO *priv_in = NULL;

    /* Check that SSL_get0_peer_certificate() returns something sensible */
    if (!TEST_ptr(SSL_get0_peer_certificate(ssl)))
        return 0;

    in = BIO_new_file(cert, "r");
    if (!TEST_ptr(in))
        return 0;

    if (!TEST_ptr(xcert = X509_new_ex(libctx, NULL))
            || !TEST_ptr(PEM_read_bio_X509(in, &xcert, NULL, NULL))
            || !TEST_ptr(priv_in = BIO_new_file(privkey, "r"))
            || !TEST_ptr(privpkey = PEM_read_bio_PrivateKey_ex(priv_in, NULL,
                                                               NULL, NULL,
                                                               libctx, NULL)))
        goto err;

    *x509 = xcert;
    *pkey = privpkey;

    BIO_free(in);
    BIO_free(priv_in);
    return 1;
err:
    X509_free(xcert);
    BIO_free(in);
    BIO_free(priv_in);
    return 0;
}

static int test_client_cert_cb(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

#ifdef OPENSSL_NO_TLS1_2
    if (tst == 0)
        return 1;
#endif
#ifdef OSSL_NO_USABLE_TLS1_3
    if (tst == 1)
        return 1;
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       tst == 0 ? TLS1_2_VERSION
                                                : TLS1_3_VERSION,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    /*
     * Test that setting a client_cert_cb results in a client certificate being
     * sent.
     */
    SSL_CTX_set_client_cert_cb(cctx, client_cert_cb);
    SSL_CTX_set_verify(sctx,
                       SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT,
                       verify_cb);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

#if !defined(OPENSSL_NO_TLS1_2) || !defined(OSSL_NO_USABLE_TLS1_3)
/*
 * Test setting certificate authorities on both client and server.
 *
 * Test 0: SSL_CTX_set0_CA_list() only
 * Test 1: Both SSL_CTX_set0_CA_list() and SSL_CTX_set_client_CA_list()
 * Test 2: Only SSL_CTX_set_client_CA_list()
 */
static int test_ca_names_int(int prot, int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    size_t i;
    X509_NAME *name[] = { NULL, NULL, NULL, NULL };
    char *strnames[] = { "Jack", "Jill", "John", "Joanne" };
    STACK_OF(X509_NAME) *sk1 = NULL, *sk2 = NULL;
    const STACK_OF(X509_NAME) *sktmp = NULL;

    for (i = 0; i < OSSL_NELEM(name); i++) {
        name[i] = X509_NAME_new();
        if (!TEST_ptr(name[i])
                || !TEST_true(X509_NAME_add_entry_by_txt(name[i], "CN",
                                                         MBSTRING_ASC,
                                                         (unsigned char *)
                                                         strnames[i],
                                                         -1, -1, 0)))
            goto end;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       prot,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_verify(sctx, SSL_VERIFY_PEER, NULL);

    if (tst == 0 || tst == 1) {
        if (!TEST_ptr(sk1 = sk_X509_NAME_new_null())
                || !TEST_true(sk_X509_NAME_push(sk1, X509_NAME_dup(name[0])))
                || !TEST_true(sk_X509_NAME_push(sk1, X509_NAME_dup(name[1])))
                || !TEST_ptr(sk2 = sk_X509_NAME_new_null())
                || !TEST_true(sk_X509_NAME_push(sk2, X509_NAME_dup(name[0])))
                || !TEST_true(sk_X509_NAME_push(sk2, X509_NAME_dup(name[1]))))
            goto end;

        SSL_CTX_set0_CA_list(sctx, sk1);
        SSL_CTX_set0_CA_list(cctx, sk2);
        sk1 = sk2 = NULL;
    }
    if (tst == 1 || tst == 2) {
        if (!TEST_ptr(sk1 = sk_X509_NAME_new_null())
                || !TEST_true(sk_X509_NAME_push(sk1, X509_NAME_dup(name[2])))
                || !TEST_true(sk_X509_NAME_push(sk1, X509_NAME_dup(name[3])))
                || !TEST_ptr(sk2 = sk_X509_NAME_new_null())
                || !TEST_true(sk_X509_NAME_push(sk2, X509_NAME_dup(name[2])))
                || !TEST_true(sk_X509_NAME_push(sk2, X509_NAME_dup(name[3]))))
            goto end;

        SSL_CTX_set_client_CA_list(sctx, sk1);
        SSL_CTX_set_client_CA_list(cctx, sk2);
        sk1 = sk2 = NULL;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    /*
     * We only expect certificate authorities to have been sent to the server
     * if we are using TLSv1.3 and SSL_set0_CA_list() was used
     */
    sktmp = SSL_get0_peer_CA_list(serverssl);
    if (prot == TLS1_3_VERSION
            && (tst == 0 || tst == 1)) {
        if (!TEST_ptr(sktmp)
                || !TEST_int_eq(sk_X509_NAME_num(sktmp), 2)
                || !TEST_int_eq(X509_NAME_cmp(sk_X509_NAME_value(sktmp, 0),
                                              name[0]), 0)
                || !TEST_int_eq(X509_NAME_cmp(sk_X509_NAME_value(sktmp, 1),
                                              name[1]), 0))
            goto end;
    } else if (!TEST_ptr_null(sktmp)) {
        goto end;
    }

    /*
     * In all tests we expect certificate authorities to have been sent to the
     * client. However, SSL_set_client_CA_list() should override
     * SSL_set0_CA_list()
     */
    sktmp = SSL_get0_peer_CA_list(clientssl);
    if (!TEST_ptr(sktmp)
            || !TEST_int_eq(sk_X509_NAME_num(sktmp), 2)
            || !TEST_int_eq(X509_NAME_cmp(sk_X509_NAME_value(sktmp, 0),
                                          name[tst == 0 ? 0 : 2]), 0)
            || !TEST_int_eq(X509_NAME_cmp(sk_X509_NAME_value(sktmp, 1),
                                          name[tst == 0 ? 1 : 3]), 0))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    for (i = 0; i < OSSL_NELEM(name); i++)
        X509_NAME_free(name[i]);
    sk_X509_NAME_pop_free(sk1, X509_NAME_free);
    sk_X509_NAME_pop_free(sk2, X509_NAME_free);

    return testresult;
}
#endif

static int test_ca_names(int tst)
{
    int testresult = 1;

#ifndef OPENSSL_NO_TLS1_2
    testresult &= test_ca_names_int(TLS1_2_VERSION, tst);
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    testresult &= test_ca_names_int(TLS1_3_VERSION, tst);
#endif

    return testresult;
}

#ifndef OPENSSL_NO_TLS1_2
static const char *multiblock_cipherlist_data[]=
{
    "AES128-SHA",
    "AES128-SHA256",
    "AES256-SHA",
    "AES256-SHA256",
};

/* Reduce the fragment size - so the multiblock test buffer can be small */
# define MULTIBLOCK_FRAGSIZE 512

static int test_multiblock_write(int test_index)
{
    static const char *fetchable_ciphers[]=
    {
        "AES-128-CBC-HMAC-SHA1",
        "AES-128-CBC-HMAC-SHA256",
        "AES-256-CBC-HMAC-SHA1",
        "AES-256-CBC-HMAC-SHA256"
    };
    const char *cipherlist = multiblock_cipherlist_data[test_index];
    const SSL_METHOD *smeth = TLS_server_method();
    const SSL_METHOD *cmeth = TLS_client_method();
    int min_version = TLS1_VERSION;
    int max_version = TLS1_2_VERSION; /* Don't select TLS1_3 */
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    /*
     * Choose a buffer large enough to perform a multi-block operation
     * i.e: write_len >= 4 * frag_size
     * 9 * is chosen so that multiple multiblocks are used + some leftover.
     */
    unsigned char msg[MULTIBLOCK_FRAGSIZE * 9];
    unsigned char buf[sizeof(msg)], *p = buf;
    size_t readbytes, written, len;
    EVP_CIPHER *ciph = NULL;

    /*
     * Check if the cipher exists before attempting to use it since it only has
     * a hardware specific implementation.
     */
    ciph = EVP_CIPHER_fetch(libctx, fetchable_ciphers[test_index], "");
    if (ciph == NULL) {
        TEST_skip("Multiblock cipher is not available for %s", cipherlist);
        return 1;
    }
    EVP_CIPHER_free(ciph);

    /* Set up a buffer with some data that will be sent to the client */
    RAND_bytes(msg, sizeof(msg));

    if (!TEST_true(create_ssl_ctx_pair(libctx, smeth, cmeth, min_version,
                                       max_version, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    if (!TEST_true(SSL_CTX_set_max_send_fragment(sctx, MULTIBLOCK_FRAGSIZE)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
            goto end;

    /* settings to force it to use AES-CBC-HMAC_SHA */
    SSL_set_options(serverssl, SSL_OP_NO_ENCRYPT_THEN_MAC);
    if (!TEST_true(SSL_CTX_set_cipher_list(cctx, cipherlist)))
       goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_true(SSL_write_ex(serverssl, msg, sizeof(msg), &written))
        || !TEST_size_t_eq(written, sizeof(msg)))
        goto end;

    len = written;
    while (len > 0) {
        if (!TEST_true(SSL_read_ex(clientssl, p, MULTIBLOCK_FRAGSIZE, &readbytes)))
            goto end;
        p += readbytes;
        len -= readbytes;
    }
    if (!TEST_mem_eq(msg, sizeof(msg), buf, sizeof(buf)))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* OPENSSL_NO_TLS1_2 */

static int test_session_timeout(int test)
{
    /*
     * Test session ordering and timeout
     * Can't explicitly test performance of the new code,
     * but can test to see if the ordering of the sessions
     * are correct, and they are removed as expected
     */
    SSL_SESSION *early = NULL;
    SSL_SESSION *middle = NULL;
    SSL_SESSION *late = NULL;
    SSL_CTX *ctx;
    int testresult = 0;
    time_t now = time(NULL);
#define TIMEOUT 10

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_method()))
        || !TEST_ptr(early = SSL_SESSION_new())
        || !TEST_ptr(middle = SSL_SESSION_new())
        || !TEST_ptr(late = SSL_SESSION_new()))
        goto end;

    /* assign unique session ids */
    early->session_id_length = SSL3_SSL_SESSION_ID_LENGTH;
    memset(early->session_id, 1, SSL3_SSL_SESSION_ID_LENGTH);
    middle->session_id_length = SSL3_SSL_SESSION_ID_LENGTH;
    memset(middle->session_id, 2, SSL3_SSL_SESSION_ID_LENGTH);
    late->session_id_length = SSL3_SSL_SESSION_ID_LENGTH;
    memset(late->session_id, 3, SSL3_SSL_SESSION_ID_LENGTH);

    if (!TEST_int_eq(SSL_CTX_add_session(ctx, early), 1)
        || !TEST_int_eq(SSL_CTX_add_session(ctx, middle), 1)
        || !TEST_int_eq(SSL_CTX_add_session(ctx, late), 1))
        goto end;

    /* Make sure they are all added */
    if (!TEST_ptr(early->prev)
        || !TEST_ptr(middle->prev)
        || !TEST_ptr(late->prev))
        goto end;

    if (!TEST_time_t_ne(SSL_SESSION_set_time_ex(early, now - 10), 0)
        || !TEST_time_t_ne(SSL_SESSION_set_time_ex(middle, now), 0)
        || !TEST_time_t_ne(SSL_SESSION_set_time_ex(late, now + 10), 0))
        goto end;

    if (!TEST_int_ne(SSL_SESSION_set_timeout(early, TIMEOUT), 0)
        || !TEST_int_ne(SSL_SESSION_set_timeout(middle, TIMEOUT), 0)
        || !TEST_int_ne(SSL_SESSION_set_timeout(late, TIMEOUT), 0))
        goto end;

    /* Make sure they are all still there */
    if (!TEST_ptr(early->prev)
        || !TEST_ptr(middle->prev)
        || !TEST_ptr(late->prev))
        goto end;

    /* Make sure they are in the expected order */
    if (!TEST_ptr_eq(late->next, middle)
        || !TEST_ptr_eq(middle->next, early)
        || !TEST_ptr_eq(early->prev, middle)
        || !TEST_ptr_eq(middle->prev, late))
        goto end;

    /* This should remove "early" */
    SSL_CTX_flush_sessions_ex(ctx, now + TIMEOUT - 1);
    if (!TEST_ptr_null(early->prev)
        || !TEST_ptr(middle->prev)
        || !TEST_ptr(late->prev))
        goto end;

    /* This should remove "middle" */
    SSL_CTX_flush_sessions_ex(ctx, now + TIMEOUT + 1);
    if (!TEST_ptr_null(early->prev)
        || !TEST_ptr_null(middle->prev)
        || !TEST_ptr(late->prev))
        goto end;

    /* This should remove "late" */
    SSL_CTX_flush_sessions_ex(ctx, now + TIMEOUT + 11);
    if (!TEST_ptr_null(early->prev)
        || !TEST_ptr_null(middle->prev)
        || !TEST_ptr_null(late->prev))
        goto end;

    /* Add them back in again */
    if (!TEST_int_eq(SSL_CTX_add_session(ctx, early), 1)
        || !TEST_int_eq(SSL_CTX_add_session(ctx, middle), 1)
        || !TEST_int_eq(SSL_CTX_add_session(ctx, late), 1))
        goto end;

    /* Make sure they are all added */
    if (!TEST_ptr(early->prev)
        || !TEST_ptr(middle->prev)
        || !TEST_ptr(late->prev))
        goto end;

    /* This should remove all of them */
    SSL_CTX_flush_sessions_ex(ctx, 0);
    if (!TEST_ptr_null(early->prev)
        || !TEST_ptr_null(middle->prev)
        || !TEST_ptr_null(late->prev))
        goto end;

    (void)SSL_CTX_set_session_cache_mode(ctx, SSL_SESS_CACHE_UPDATE_TIME
                                         | SSL_CTX_get_session_cache_mode(ctx));

    /* make sure |now| is NOT  equal to the current time */
    now -= 10;
    if (!TEST_time_t_ne(SSL_SESSION_set_time_ex(early, now), 0)
        || !TEST_int_eq(SSL_CTX_add_session(ctx, early), 1)
        || !TEST_time_t_ne(SSL_SESSION_get_time_ex(early), now))
        goto end;

    testresult = 1;
 end:
    SSL_CTX_free(ctx);
    SSL_SESSION_free(early);
    SSL_SESSION_free(middle);
    SSL_SESSION_free(late);
    return testresult;
}

/*
 * Test that a session cache overflow works as expected
 * Test 0: TLSv1.3, timeout on new session later than old session
 * Test 1: TLSv1.2, timeout on new session later than old session
 * Test 2: TLSv1.3, timeout on new session earlier than old session
 * Test 3: TLSv1.2, timeout on new session earlier than old session
 */
#if !defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2)
static int test_session_cache_overflow(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;

#ifdef OSSL_NO_USABLE_TLS1_3
    /* If no TLSv1.3 available then do nothing in this case */
    if (idx % 2 == 0)
        return TEST_skip("No TLSv1.3 available");
#endif
#ifdef OPENSSL_NO_TLS1_2
    /* If no TLSv1.2 available then do nothing in this case */
    if (idx % 2 == 1)
        return TEST_skip("No TLSv1.2 available");
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION,
                                       (idx % 2 == 0) ? TLS1_3_VERSION
                                                      : TLS1_2_VERSION,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(SSL_CTX_set_options(sctx, SSL_OP_NO_TICKET)))
        goto end;

    SSL_CTX_sess_set_get_cb(sctx, get_session_cb);
    get_sess_val = NULL;

    SSL_CTX_sess_set_cache_size(sctx, 1);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (idx > 1) {
        sess = SSL_get_session(serverssl);
        if (!TEST_ptr(sess))
            goto end;

        /*
         * Cause this session to have a longer timeout than the next session to
         * be added.
         */
        if (!TEST_true(SSL_SESSION_set_timeout(sess, LONG_MAX))) {
            sess = NULL;
            goto end;
        }
        sess = NULL;
    }

    SSL_shutdown(serverssl);
    SSL_shutdown(clientssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    /*
     * Session cache size is 1 and we already populated the cache with a session
     * so the next connection should cause an overflow.
     */

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /*
     * The session we just negotiated may have been already removed from the
     * internal cache - but we will return it anyway from our external cache.
     */
    get_sess_val = SSL_get_session(serverssl);
    if (!TEST_ptr(get_sess_val))
        goto end;
    sess = SSL_get1_session(clientssl);
    if (!TEST_ptr(sess))
        goto end;

    SSL_shutdown(serverssl);
    SSL_shutdown(clientssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_session(clientssl, sess)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(sess);

    return testresult;
}
#endif /* !defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2) */

/*
 * Test 0: Client sets servername and server acknowledges it (TLSv1.2)
 * Test 1: Client sets servername and server does not acknowledge it (TLSv1.2)
 * Test 2: Client sets inconsistent servername on resumption (TLSv1.2)
 * Test 3: Client does not set servername on initial handshake (TLSv1.2)
 * Test 4: Client does not set servername on resumption handshake (TLSv1.2)
 * Test 5: Client sets servername and server acknowledges it (TLSv1.3)
 * Test 6: Client sets servername and server does not acknowledge it (TLSv1.3)
 * Test 7: Client sets inconsistent servername on resumption (TLSv1.3)
 * Test 8: Client does not set servername on initial handshake(TLSv1.3)
 * Test 9: Client does not set servername on resumption handshake (TLSv1.3)
 */
static int test_servername(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    const char *sexpectedhost = NULL, *cexpectedhost = NULL;

#ifdef OPENSSL_NO_TLS1_2
    if (tst <= 4)
        return 1;
#endif
#ifdef OSSL_NO_USABLE_TLS1_3
    if (tst >= 5)
        return 1;
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_VERSION,
                                       (tst <= 4) ? TLS1_2_VERSION
                                                  : TLS1_3_VERSION,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    if (tst != 1 && tst != 6) {
        if (!TEST_true(SSL_CTX_set_tlsext_servername_callback(sctx,
                                                              hostname_cb)))
            goto end;
    }

    if (tst != 3 && tst != 8) {
        if (!TEST_true(SSL_set_tlsext_host_name(clientssl, "goodhost")))
            goto end;
        sexpectedhost = cexpectedhost = "goodhost";
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_str_eq(SSL_get_servername(clientssl, TLSEXT_NAMETYPE_host_name),
                     cexpectedhost)
            || !TEST_str_eq(SSL_get_servername(serverssl,
                                               TLSEXT_NAMETYPE_host_name),
                            sexpectedhost))
        goto end;

    /* Now repeat with a resumption handshake */

    if (!TEST_int_eq(SSL_shutdown(clientssl), 0)
            || !TEST_ptr_ne(sess = SSL_get1_session(clientssl), NULL)
            || !TEST_true(SSL_SESSION_is_resumable(sess))
            || !TEST_int_eq(SSL_shutdown(serverssl), 0))
        goto end;

    SSL_free(clientssl);
    SSL_free(serverssl);
    clientssl = serverssl = NULL;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (!TEST_true(SSL_set_session(clientssl, sess)))
        goto end;

    sexpectedhost = cexpectedhost = "goodhost";
    if (tst == 2 || tst == 7) {
        /* Set an inconsistent hostname */
        if (!TEST_true(SSL_set_tlsext_host_name(clientssl, "altgoodhost")))
            goto end;
        /*
         * In TLSv1.2 we expect the hostname from the original handshake, in
         * TLSv1.3 we expect the hostname from this handshake
         */
        if (tst == 7)
            sexpectedhost = cexpectedhost = "altgoodhost";

        if (!TEST_str_eq(SSL_get_servername(clientssl,
                                            TLSEXT_NAMETYPE_host_name),
                         "altgoodhost"))
            goto end;
    } else if (tst == 4 || tst == 9) {
        /*
         * A TLSv1.3 session does not associate a session with a servername,
         * but a TLSv1.2 session does.
         */
        if (tst == 9)
            sexpectedhost = cexpectedhost = NULL;

        if (!TEST_str_eq(SSL_get_servername(clientssl,
                                            TLSEXT_NAMETYPE_host_name),
                         cexpectedhost))
            goto end;
    } else {
        if (!TEST_true(SSL_set_tlsext_host_name(clientssl, "goodhost")))
            goto end;
        /*
         * In a TLSv1.2 resumption where the hostname was not acknowledged
         * we expect the hostname on the server to be empty. On the client we
         * return what was requested in this case.
         *
         * Similarly if the client didn't set a hostname on an original TLSv1.2
         * session but is now, the server hostname will be empty, but the client
         * is as we set it.
         */
        if (tst == 1 || tst == 3)
            sexpectedhost = NULL;

        if (!TEST_str_eq(SSL_get_servername(clientssl,
                                            TLSEXT_NAMETYPE_host_name),
                         "goodhost"))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_true(SSL_session_reused(clientssl))
            || !TEST_true(SSL_session_reused(serverssl))
            || !TEST_str_eq(SSL_get_servername(clientssl,
                                               TLSEXT_NAMETYPE_host_name),
                            cexpectedhost)
            || !TEST_str_eq(SSL_get_servername(serverssl,
                                               TLSEXT_NAMETYPE_host_name),
                            sexpectedhost))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static int test_unknown_sigalgs_groups(void)
{
    int ret = 0;
    SSL_CTX *ctx = NULL;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method())))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set1_sigalgs_list(ctx,
                                               "RSA+SHA256:?nonexistent:?RSA+SHA512"),
                                               0))
        goto end;
    if (!TEST_size_t_eq(ctx->cert->conf_sigalgslen, 2)
        || !TEST_int_eq(ctx->cert->conf_sigalgs[0], TLSEXT_SIGALG_rsa_pkcs1_sha256)
        || !TEST_int_eq(ctx->cert->conf_sigalgs[1], TLSEXT_SIGALG_rsa_pkcs1_sha512))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set1_client_sigalgs_list(ctx,
                                                      "RSA+SHA256:?nonexistent:?RSA+SHA512"),
                                                      0))
        goto end;
    if (!TEST_size_t_eq(ctx->cert->client_sigalgslen, 2)
        || !TEST_int_eq(ctx->cert->client_sigalgs[0], TLSEXT_SIGALG_rsa_pkcs1_sha256)
        || !TEST_int_eq(ctx->cert->client_sigalgs[1], TLSEXT_SIGALG_rsa_pkcs1_sha512))
        goto end;

    if (!TEST_int_le(SSL_CTX_set1_groups_list(ctx,
                                              "nonexistent"),
                                              0))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set1_groups_list(ctx,
                                              "?nonexistent1:?nonexistent2:?nonexistent3"),
                                              0))
        goto end;

#ifndef OPENSSL_NO_EC
    if (!TEST_int_le(SSL_CTX_set1_groups_list(ctx,
                                              "P-256:nonexistent"),
                                              0))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set1_groups_list(ctx,
                                              "P-384:?nonexistent:?P-521"),
                                              0))
        goto end;
    if (!TEST_size_t_eq(ctx->ext.supportedgroups_len, 2)
        || !TEST_int_eq(ctx->ext.supportedgroups[0], OSSL_TLS_GROUP_ID_secp384r1)
        || !TEST_int_eq(ctx->ext.supportedgroups[1], OSSL_TLS_GROUP_ID_secp521r1))
        goto end;
#endif

    ret = 1;
 end:
    SSL_CTX_free(ctx);
    return ret;
}

#if (!defined(OPENSSL_NO_EC) || !defined(OPENSSL_NO_DH)) || !defined(OPENSSL_NO_ML_KEM)
static int test_configuration_of_groups(void)
{
    int ret = 0;
    SSL_CTX *ctx = NULL;
    size_t groups_len;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method())))
        goto end;
    groups_len = ctx->ext.supportedgroups_len;

    if (!TEST_size_t_gt(groups_len, 0)
        || !TEST_int_gt(SSL_CTX_set1_groups_list(ctx, "DEFAULT"), 0)
        || !TEST_size_t_eq(ctx->ext.supportedgroups_len, groups_len))
        goto end;

    if (!TEST_int_gt(SSL_CTX_set1_groups_list(ctx, "DEFAULT:-?P-256"), 0)
# if !defined(OPENSSL_NO_EC)
        || !TEST_size_t_eq(ctx->ext.supportedgroups_len, groups_len - 1)
# else
        || !TEST_size_t_eq(ctx->ext.supportedgroups_len, groups_len)
# endif
        )
        goto end;

# if !defined(OPENSSL_NO_EC)
    if (!TEST_int_gt(SSL_CTX_set1_groups_list(ctx, "?P-256:?P-521:-?P-256"), 0)
        || !TEST_size_t_eq(ctx->ext.supportedgroups_len, 1)
        || !TEST_int_eq(ctx->ext.supportedgroups[0], OSSL_TLS_GROUP_ID_secp521r1)
        )
        goto end;
# endif

    ret = 1;

end:
    SSL_CTX_free(ctx);
    return ret;
}
#endif

#if !defined(OPENSSL_NO_EC) \
    && (!defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2))
/*
 * Test that if signature algorithms are not available, then we do not offer or
 * accept them.
 * Test 0: Two RSA sig algs available: both RSA sig algs shared
 * Test 1: The client only has SHA2-256: only SHA2-256 algorithms shared
 * Test 2: The server only has SHA2-256: only SHA2-256 algorithms shared
 * Test 3: An RSA and an ECDSA sig alg available: both sig algs shared
 * Test 4: The client only has an ECDSA sig alg: only ECDSA algorithms shared
 * Test 5: The server only has an ECDSA sig alg: only ECDSA algorithms shared
 */
static int test_sigalgs_available(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    OSSL_LIB_CTX *tmpctx = OSSL_LIB_CTX_new();
    OSSL_LIB_CTX *clientctx = libctx, *serverctx = libctx;
    OSSL_PROVIDER *filterprov = NULL;
    int sig, hash, numshared, numshared_expected, hash_expected, sig_expected;
    const char *sigalg_name, *signame_expected;

    if (!TEST_ptr(tmpctx))
        goto end;

    if (idx != 0 && idx != 3) {
        if (!TEST_true(OSSL_PROVIDER_add_builtin(tmpctx, "filter",
                                                 filter_provider_init)))
            goto end;

        filterprov = OSSL_PROVIDER_load(tmpctx, "filter");
        if (!TEST_ptr(filterprov))
            goto end;

        if (idx < 3) {
            /*
             * Only enable SHA2-256 so rsa_pss_rsae_sha384 should not be offered
             * or accepted for the peer that uses this libctx. Note that libssl
             * *requires* SHA2-256 to be available so we cannot disable that. We
             * also need SHA1 for our certificate.
             */
            if (!TEST_true(filter_provider_set_filter(OSSL_OP_DIGEST,
                                                      "SHA2-256:SHA1")))
                goto end;
        } else {
            if (!TEST_true(filter_provider_set_filter(OSSL_OP_SIGNATURE,
                                                      "ECDSA"))
# ifdef OPENSSL_NO_ECX
                    || !TEST_true(filter_provider_set_filter(OSSL_OP_KEYMGMT, "EC"))
# else
                    || !TEST_true(filter_provider_set_filter(OSSL_OP_KEYMGMT,
                                                             "EC:X25519:X448"))
# endif
                )
                goto end;
        }

        if (idx == 1 || idx == 4)
            clientctx = tmpctx;
        else
            serverctx = tmpctx;
    }

    cctx = SSL_CTX_new_ex(clientctx, NULL, TLS_client_method());
    sctx = SSL_CTX_new_ex(serverctx, NULL, TLS_server_method());
    if (!TEST_ptr(cctx) || !TEST_ptr(sctx))
        goto end;

    /* Avoid MLKEM groups that depend on possibly filtered-out digests */
    if (!TEST_true(SSL_CTX_set1_groups_list(cctx,
                        "?X25519:?secp256r1:?ffdhe2048:?ffdhe3072"))
        || !TEST_true(SSL_CTX_set1_groups_list(sctx,
                        "?X25519:?secp256r1:?ffdhe2048:?ffdhe3072")))
        goto end;

    if (idx != 5) {
        /* RSA first server key */
        if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                           TLS_client_method(),
                                           TLS1_VERSION,
                                           0,
                                           &sctx, &cctx, cert, privkey)))
            goto end;
    } else {
        /* ECDSA P-256 first server key */
        if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                           TLS_client_method(),
                                           TLS1_VERSION,
                                           0,
                                           &sctx, &cctx, cert2, privkey2)))
            goto end;
    }

    /* Ensure we only use TLSv1.2 ciphersuites based on SHA256 */
    if (idx < 4) {
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                                               "ECDHE-RSA-AES128-GCM-SHA256")))
            goto end;
    } else {
        if (!TEST_true(SSL_CTX_set_cipher_list(cctx,
                                               "ECDHE-ECDSA-AES128-GCM-SHA256")))
            goto end;
    }

    if (idx < 3) {
        if (!SSL_CTX_set1_sigalgs_list(cctx,
                                       "rsa_pss_rsae_sha384"
                                       ":rsa_pss_rsae_sha256")
                || !SSL_CTX_set1_sigalgs_list(sctx,
                                              "rsa_pss_rsae_sha384"
                                              ":rsa_pss_rsae_sha256"))
            goto end;
    } else {
        if (!SSL_CTX_set1_sigalgs_list(cctx, "rsa_pss_rsae_sha256:ECDSA+SHA256")
                || !SSL_CTX_set1_sigalgs_list(sctx,
                                              "rsa_pss_rsae_sha256:ECDSA+SHA256"))
            goto end;
    }

    /* ECDSA P-256 second server key, unless already first */
    if (idx != 5
        && (!TEST_int_eq(SSL_CTX_use_certificate_file(sctx, cert2,
                                                      SSL_FILETYPE_PEM), 1)
            || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(sctx,
                                                        privkey2,
                                                        SSL_FILETYPE_PEM), 1)
            || !TEST_int_eq(SSL_CTX_check_private_key(sctx), 1)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* For tests 0 and 3 we expect 2 shared sigalgs, otherwise exactly 1 */
    numshared = SSL_get_shared_sigalgs(serverssl, 0, &sig, &hash,
                                        NULL, NULL, NULL);
    numshared_expected = 1;
    hash_expected = NID_sha256;
    sig_expected = NID_rsassaPss;
    signame_expected = "rsa_pss_rsae_sha256";
    switch (idx) {
    case 0:
        hash_expected = NID_sha384;
        signame_expected = "rsa_pss_rsae_sha384";
        /* FALLTHROUGH */
    case 3:
        numshared_expected = 2;
        break;
    case 4:
    case 5:
        sig_expected = EVP_PKEY_EC;
        signame_expected = "ecdsa_secp256r1_sha256";
        break;
    }
    if (!TEST_int_eq(numshared, numshared_expected)
        || !TEST_int_eq(hash, hash_expected)
        || !TEST_int_eq(sig, sig_expected)
        || !TEST_true(SSL_get0_peer_signature_name(clientssl, &sigalg_name))
        || !TEST_ptr(sigalg_name)
        || !TEST_str_eq(sigalg_name, signame_expected))
        goto end;

    testresult = filter_provider_check_clean_finish();

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    OSSL_PROVIDER_unload(filterprov);
    OSSL_LIB_CTX_free(tmpctx);

    return testresult;
}
#endif /*
        * !defined(OPENSSL_NO_EC) \
        * && (!defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2))
        */

#ifndef OPENSSL_NO_TLS1_3
/* This test can run in TLSv1.3 even if ec and dh are disabled */
static int test_pluggable_group(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    OSSL_PROVIDER *tlsprov = OSSL_PROVIDER_load(libctx, "tls-provider");
    /* Check that we are not impacted by a provider without any groups */
    OSSL_PROVIDER *legacyprov = OSSL_PROVIDER_load(libctx, "legacy");
    const char *group_name = idx == 0 ? "xorkemgroup" : "xorgroup";

    if (!TEST_ptr(tlsprov))
        goto end;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_3_VERSION,
                                       TLS1_3_VERSION,
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    /* ensure GROUPLIST_INCREMENT (=40) logic triggers: */
    if (!TEST_true(SSL_set1_groups_list(serverssl, "xorgroup:xorkemgroup:dummy1:dummy2:dummy3:dummy4:dummy5:dummy6:dummy7:dummy8:dummy9:dummy10:dummy11:dummy12:dummy13:dummy14:dummy15:dummy16:dummy17:dummy18:dummy19:dummy20:dummy21:dummy22:dummy23:dummy24:dummy25:dummy26:dummy27:dummy28:dummy29:dummy30:dummy31:dummy32:dummy33:dummy34:dummy35:dummy36:dummy37:dummy38:dummy39:dummy40:dummy41:dummy42:dummy43"))
    /* removing a single algorithm from the list makes the test pass */
            || !TEST_true(SSL_set1_groups_list(clientssl, group_name)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_str_eq(group_name,
                     SSL_group_to_name(serverssl, SSL_get_shared_group(serverssl, 0))))
        goto end;

    if (!TEST_str_eq(group_name, SSL_get0_group_name(serverssl))
        || !TEST_str_eq(group_name, SSL_get0_group_name(clientssl)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    OSSL_PROVIDER_unload(tlsprov);
    OSSL_PROVIDER_unload(legacyprov);

    return testresult;
}

/*
 * This function triggers encode, decode and sign functions
 * of the artificial "xorhmacsig" algorithm implemented in tls-provider
 * creating private key and certificate files for use in TLS testing.
 */
static int create_cert_key(int idx, char *certfilename, char *privkeyfilename)
{
    EVP_PKEY_CTX *evpctx = EVP_PKEY_CTX_new_from_name(libctx,
                             (idx == 0) ? "xorhmacsig" : "xorhmacsha2sig", NULL);
    EVP_PKEY *pkey = NULL;
    X509 *x509 = X509_new();
    X509_NAME *name = NULL;
    BIO *keybio = NULL, *certbio = NULL;
    int ret = 1;

    if (!TEST_ptr(evpctx)
        || !TEST_int_gt(EVP_PKEY_keygen_init(evpctx), 0)
        || !TEST_true(EVP_PKEY_generate(evpctx, &pkey))
        || !TEST_ptr(pkey)
        || !TEST_ptr(x509)
        || !TEST_true(ASN1_INTEGER_set(X509_get_serialNumber(x509), 1))
        || !TEST_true(X509_gmtime_adj(X509_getm_notBefore(x509), 0))
        || !TEST_true(X509_gmtime_adj(X509_getm_notAfter(x509), 31536000L))
        || !TEST_true(X509_set_pubkey(x509, pkey))
        || !TEST_ptr(name = X509_get_subject_name(x509))
        || !TEST_true(X509_NAME_add_entry_by_txt(name, "C",  MBSTRING_ASC,
                           (unsigned char *)"CH", -1, -1, 0))
        || !TEST_true(X509_NAME_add_entry_by_txt(name, "O",  MBSTRING_ASC,
                           (unsigned char *)"test.org", -1, -1, 0))
        || !TEST_true(X509_NAME_add_entry_by_txt(name, "CN", MBSTRING_ASC,
                           (unsigned char *)"localhost", -1, -1, 0))
        || !TEST_true(X509_set_issuer_name(x509, name))
        || !TEST_true(X509_sign(x509, pkey, EVP_sha1()))
        || !TEST_ptr(keybio = BIO_new_file(privkeyfilename, "wb"))
        || !TEST_true(PEM_write_bio_PrivateKey(keybio, pkey, NULL, NULL, 0, NULL, NULL))
        || !TEST_ptr(certbio = BIO_new_file(certfilename, "wb"))
        || !TEST_true(PEM_write_bio_X509(certbio, x509)))
        ret = 0;

    EVP_PKEY_free(pkey);
    X509_free(x509);
    EVP_PKEY_CTX_free(evpctx);
    BIO_free(keybio);
    BIO_free(certbio);
    return ret;
}

/*
 * Test that signature algorithms loaded via the provider interface can
 * correctly establish a TLS (1.3) connection.
 * Test 0: Signature algorithm with built-in hashing functionality: "xorhmacsig"
 * Test 1: Signature algorithm using external SHA2 hashing: "xorhmacsha2sig"
 * Test 2: Signature algorithm with built-in hashing configured via SSL_CONF_cmd
 * Test 3: Test 0 using RPK
 * Test 4: Test 1 using RPK
 * Test 5: Test 2 using RPK
 */
static int test_pluggable_signature(int idx)
{
    static const unsigned char cert_type_rpk[] = { TLSEXT_cert_type_rpk, TLSEXT_cert_type_x509 };
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    OSSL_PROVIDER *tlsprov = OSSL_PROVIDER_load(libctx, "tls-provider");
    OSSL_PROVIDER *defaultprov = OSSL_PROVIDER_load(libctx, "default");
    char *certfilename = "tls-prov-cert.pem";
    char *privkeyfilename = "tls-prov-key.pem";
    const char *sigalg_name = NULL, *expected_sigalg_name;
    int sigidx = idx % 3;
    int rpkidx = idx / 3;
    int do_conf_cmd = 0;

    if (sigidx == 2) {
        sigidx = 0;
        do_conf_cmd = 1;
    }

    /* See create_cert_key() above */
    expected_sigalg_name = (sigidx == 0) ? "xorhmacsig" : "xorhmacsha2sig";

    /* create key and certificate for the different algorithm types */
    if (!TEST_ptr(tlsprov)
        || !TEST_true(create_cert_key(sigidx, certfilename, privkeyfilename)))
        goto end;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       TLS1_3_VERSION,
                                       TLS1_3_VERSION,
                                       &sctx, &cctx, NULL, NULL)))
        goto end;

    if (do_conf_cmd) {
        SSL_CONF_CTX *confctx = SSL_CONF_CTX_new();

        if (!TEST_ptr(confctx))
            goto end;
        SSL_CONF_CTX_set_flags(confctx, SSL_CONF_FLAG_FILE
                                        | SSL_CONF_FLAG_SERVER
                                        | SSL_CONF_FLAG_CERTIFICATE
                                        | SSL_CONF_FLAG_REQUIRE_PRIVATE
                                        | SSL_CONF_FLAG_SHOW_ERRORS);
        SSL_CONF_CTX_set_ssl_ctx(confctx, sctx);
        if (!TEST_int_gt(SSL_CONF_cmd(confctx, "Certificate", certfilename), 0)
                || !TEST_int_gt(SSL_CONF_cmd(confctx, "PrivateKey", privkeyfilename), 0)
                || !TEST_true(SSL_CONF_CTX_finish(confctx))) {
            SSL_CONF_CTX_free(confctx);
            goto end;
        }
        SSL_CONF_CTX_free(confctx);
    } else {
        if (!TEST_int_eq(SSL_CTX_use_certificate_file(sctx, certfilename,
                                                      SSL_FILETYPE_PEM), 1)
                || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(sctx,
                                                            privkeyfilename,
                                                            SSL_FILETYPE_PEM), 1))
            goto end;
    }
    if (!TEST_int_eq(SSL_CTX_check_private_key(sctx), 1))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    /* Enable RPK for server cert */
    if (rpkidx) {
        if (!TEST_true(SSL_set1_server_cert_type(serverssl, cert_type_rpk, sizeof(cert_type_rpk)))
                || !TEST_true(SSL_set1_server_cert_type(clientssl, cert_type_rpk, sizeof(cert_type_rpk))))
            goto end;
    }

    /* This is necessary to pass minimal setup w/o other groups configured */
    if (!TEST_true(SSL_set1_groups_list(serverssl, "xorgroup"))
            || !TEST_true(SSL_set1_groups_list(clientssl, "xorgroup")))
        goto end;

    /*
     * If this connection gets established, it must have been completed
     * via the tls-provider-implemented "hmacsig" algorithm, testing
     * both sign and verify functions during handshake.
     */
    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* If using RPK, make sure we got one */
    if (rpkidx && !TEST_long_eq(SSL_get_verify_result(clientssl), X509_V_ERR_RPK_UNTRUSTED))
        goto end;

    if (!TEST_true(SSL_get0_peer_signature_name(clientssl, &sigalg_name))
        || !TEST_str_eq(sigalg_name, expected_sigalg_name)
        || !TEST_ptr(sigalg_name))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    OSSL_PROVIDER_unload(tlsprov);
    OSSL_PROVIDER_unload(defaultprov);

    return testresult;
}
#endif

#ifndef OPENSSL_NO_TLS1_2
static int test_ssl_dup(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL, *client2ssl = NULL;
    int testresult = 0;
    BIO *rbio = NULL, *wbio = NULL;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       0,
                                       0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_min_proto_version(clientssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_max_proto_version(clientssl, TLS1_2_VERSION)))
        goto end;

    client2ssl = SSL_dup(clientssl);
    rbio = SSL_get_rbio(clientssl);
    if (!TEST_ptr(rbio)
            || !TEST_true(BIO_up_ref(rbio)))
        goto end;
    SSL_set0_rbio(client2ssl, rbio);
    rbio = NULL;

    wbio = SSL_get_wbio(clientssl);
    if (!TEST_ptr(wbio) || !TEST_true(BIO_up_ref(wbio)))
        goto end;
    SSL_set0_wbio(client2ssl, wbio);
    rbio = NULL;

    if (!TEST_ptr(client2ssl)
               /* Handshake not started so pointers should be different */
            || !TEST_ptr_ne(clientssl, client2ssl))
        goto end;

    if (!TEST_int_eq(SSL_get_min_proto_version(client2ssl), TLS1_2_VERSION)
            || !TEST_int_eq(SSL_get_max_proto_version(client2ssl), TLS1_2_VERSION))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, client2ssl, SSL_ERROR_NONE)))
        goto end;

    SSL_free(clientssl);
    clientssl = SSL_dup(client2ssl);
    if (!TEST_ptr(clientssl)
               /* Handshake has finished so pointers should be the same */
            || !TEST_ptr_eq(clientssl, client2ssl))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_free(client2ssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

static int secret_cb(SSL *s, void *secretin, int *secret_len,
                     STACK_OF(SSL_CIPHER) *peer_ciphers,
                     const SSL_CIPHER **cipher, void *arg)
{
    int i;
    unsigned char *secret = secretin;

    /* Just use a fixed master secret */
    for (i = 0; i < *secret_len; i++)
        secret[i] = 0xff;

    /* We don't set a preferred cipher */

    return 1;
}

/*
 * Test the session_secret_cb which is designed for use with EAP-FAST
 */
static int test_session_secret_cb(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    SSL_SESSION *secret_sess = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       0,
                                       0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    /* Create an initial connection and save the session */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    /* session_secret_cb does not support TLSv1.3 */
    if (!TEST_true(SSL_set_min_proto_version(clientssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_max_proto_version(serverssl, TLS1_2_VERSION)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_ptr(secret_sess = SSL_get1_session(clientssl)))
        goto end;

    shutdown_ssl_connection(serverssl, clientssl);
    serverssl = clientssl = NULL;

    /* Resume the earlier session */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    /*
     * No session ids for EAP-FAST - otherwise the state machine gets very
     * confused.
     */
    if (!TEST_true(SSL_SESSION_set1_id(secret_sess, NULL, 0)))
        goto end;

    if (!TEST_true(SSL_set_min_proto_version(clientssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_max_proto_version(serverssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_session_secret_cb(serverssl, secret_cb,
                                                    NULL))
            || !TEST_true(SSL_set_session_secret_cb(clientssl, secret_cb,
                                                    NULL))
            || !TEST_true(SSL_set_session(clientssl, secret_sess)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(secret_sess);
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

# ifndef OPENSSL_NO_DH

static EVP_PKEY *tmp_dh_params = NULL;

/* Helper function for the test_set_tmp_dh() tests */
static EVP_PKEY *get_tmp_dh_params(void)
{
    if (tmp_dh_params == NULL) {
        BIGNUM *p = NULL;
        OSSL_PARAM_BLD *tmpl = NULL;
        EVP_PKEY_CTX *pctx = NULL;
        OSSL_PARAM *params = NULL;
        EVP_PKEY *dhpkey = NULL;

        p = BN_get_rfc3526_prime_2048(NULL);
        if (!TEST_ptr(p))
            goto end;

        pctx = EVP_PKEY_CTX_new_from_name(libctx, "DH", NULL);
        if (!TEST_ptr(pctx)
                || !TEST_int_eq(EVP_PKEY_fromdata_init(pctx), 1))
            goto end;

        tmpl = OSSL_PARAM_BLD_new();
        if (!TEST_ptr(tmpl)
                || !TEST_true(OSSL_PARAM_BLD_push_BN(tmpl,
                                                        OSSL_PKEY_PARAM_FFC_P,
                                                        p))
                || !TEST_true(OSSL_PARAM_BLD_push_uint(tmpl,
                                                        OSSL_PKEY_PARAM_FFC_G,
                                                        2)))
            goto end;

        params = OSSL_PARAM_BLD_to_param(tmpl);
        if (!TEST_ptr(params)
                || !TEST_int_eq(EVP_PKEY_fromdata(pctx, &dhpkey,
                                                  EVP_PKEY_KEY_PARAMETERS,
                                                  params), 1))
            goto end;

        tmp_dh_params = dhpkey;
    end:
        BN_free(p);
        EVP_PKEY_CTX_free(pctx);
        OSSL_PARAM_BLD_free(tmpl);
        OSSL_PARAM_free(params);
    }

    if (tmp_dh_params != NULL && !EVP_PKEY_up_ref(tmp_dh_params))
        return NULL;

    return tmp_dh_params;
}

#  ifndef OPENSSL_NO_DEPRECATED_3_0
/* Callback used by test_set_tmp_dh() */
static DH *tmp_dh_callback(SSL *s, int is_export, int keylen)
{
    EVP_PKEY *dhpkey = get_tmp_dh_params();
    DH *ret = NULL;

    if (!TEST_ptr(dhpkey))
        return NULL;

    /*
     * libssl does not free the returned DH, so we free it now knowing that even
     * after we free dhpkey, there will still be a reference to the owning
     * EVP_PKEY in tmp_dh_params, and so the DH object will live for the length
     * of time we need it for.
     */
    ret = EVP_PKEY_get1_DH(dhpkey);
    DH_free(ret);

    EVP_PKEY_free(dhpkey);

    return ret;
}
#  endif

/*
 * Test the various methods for setting temporary DH parameters
 *
 * Test  0: Default (no auto) setting
 * Test  1: Explicit SSL_CTX auto off
 * Test  2: Explicit SSL auto off
 * Test  3: Explicit SSL_CTX auto on
 * Test  4: Explicit SSL auto on
 * Test  5: Explicit SSL_CTX auto off, custom DH params via EVP_PKEY
 * Test  6: Explicit SSL auto off, custom DH params via EVP_PKEY
 *
 * The following are testing deprecated APIs, so we only run them if available
 * Test  7: Explicit SSL_CTX auto off, custom DH params via DH
 * Test  8: Explicit SSL auto off, custom DH params via DH
 * Test  9: Explicit SSL_CTX auto off, custom DH params via callback
 * Test 10: Explicit SSL auto off, custom DH params via callback
 */
static int test_set_tmp_dh(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    int dhauto = (idx == 3 || idx == 4) ? 1 : 0;
    int expected = (idx <= 2) ? 0 : 1;
    EVP_PKEY *dhpkey = NULL;
#  ifndef OPENSSL_NO_DEPRECATED_3_0
    DH *dh = NULL;
#  else

    if (idx >= 7)
        return 1;
#  endif

    if (idx >= 5 && idx <= 8) {
        dhpkey = get_tmp_dh_params();
        if (!TEST_ptr(dhpkey))
            goto end;
    }
#  ifndef OPENSSL_NO_DEPRECATED_3_0
    if (idx == 7 || idx == 8) {
        dh = EVP_PKEY_get1_DH(dhpkey);
        if (!TEST_ptr(dh))
            goto end;
    }
#  endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(),
                                       0,
                                       0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if ((idx & 1) == 1) {
        if (!TEST_true(SSL_CTX_set_dh_auto(sctx, dhauto)))
            goto end;
    }

    if (idx == 5) {
        if (!TEST_true(SSL_CTX_set0_tmp_dh_pkey(sctx, dhpkey)))
            goto end;
        dhpkey = NULL;
    }
#  ifndef OPENSSL_NO_DEPRECATED_3_0
    else if (idx == 7) {
        if (!TEST_true(SSL_CTX_set_tmp_dh(sctx, dh)))
            goto end;
    } else if (idx == 9) {
        SSL_CTX_set_tmp_dh_callback(sctx, tmp_dh_callback);
    }
#  endif

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if ((idx & 1) == 0 && idx != 0) {
        if (!TEST_true(SSL_set_dh_auto(serverssl, dhauto)))
            goto end;
    }
    if (idx == 6) {
        if (!TEST_true(SSL_set0_tmp_dh_pkey(serverssl, dhpkey)))
            goto end;
        dhpkey = NULL;
    }
#  ifndef OPENSSL_NO_DEPRECATED_3_0
    else if (idx == 8) {
        if (!TEST_true(SSL_set_tmp_dh(serverssl, dh)))
            goto end;
    } else if (idx == 10) {
        SSL_set_tmp_dh_callback(serverssl, tmp_dh_callback);
    }
#  endif

    if (!TEST_true(SSL_set_min_proto_version(serverssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_max_proto_version(serverssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_cipher_list(serverssl, "DHE-RSA-AES128-SHA")))
        goto end;

    /*
     * If autoon then we should succeed. Otherwise we expect failure because
     * there are no parameters
     */
    if (!TEST_int_eq(create_ssl_connection(serverssl, clientssl,
                                           SSL_ERROR_NONE), expected))
        goto end;

    testresult = 1;

 end:
#  ifndef OPENSSL_NO_DEPRECATED_3_0
    DH_free(dh);
#  endif
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    EVP_PKEY_free(dhpkey);

    return testresult;
}

/*
 * Test the auto DH keys are appropriately sized
 */
static int test_dh_auto(int idx)
{
    SSL_CTX *cctx = SSL_CTX_new_ex(libctx, NULL, TLS_client_method());
    SSL_CTX *sctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    EVP_PKEY *tmpkey = NULL;
    char *thiscert = NULL, *thiskey = NULL;
    size_t expdhsize = 0;
    const char *ciphersuite = "DHE-RSA-AES128-SHA";

    if (!TEST_ptr(sctx) || !TEST_ptr(cctx))
        goto end;

    switch (idx) {
    case 0:
        /* The FIPS provider doesn't support this DH size - so we ignore it */
        if (is_fips) {
            testresult = 1;
            goto end;
        }
        thiscert = cert1024;
        thiskey = privkey1024;
        expdhsize = 1024;
        SSL_CTX_set_security_level(sctx, 1);
        SSL_CTX_set_security_level(cctx, 1);
        break;
    case 1:
        /* 2048 bit prime */
        thiscert = cert;
        thiskey = privkey;
        expdhsize = 2048;
        break;
    case 2:
        thiscert = cert3072;
        thiskey = privkey3072;
        expdhsize = 3072;
        break;
    case 3:
        thiscert = cert4096;
        thiskey = privkey4096;
        expdhsize = 4096;
        break;
    case 4:
        thiscert = cert8192;
        thiskey = privkey8192;
        expdhsize = 8192;
        break;
    /* No certificate cases */
    case 5:
        /* The FIPS provider doesn't support this DH size - so we ignore it */
        if (is_fips) {
            testresult = 1;
            goto end;
        }
        ciphersuite = "ADH-AES128-SHA256:@SECLEVEL=0";
        expdhsize = 1024;
        break;
    case 6:
        ciphersuite = "ADH-AES256-SHA256:@SECLEVEL=0";
        expdhsize = 3072;
        break;
    default:
        TEST_error("Invalid text index");
        goto end;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, NULL,
                                       NULL,
                                       0,
                                       0,
                                       &sctx, &cctx, thiscert, thiskey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                      NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_dh_auto(serverssl, 1))
            || !TEST_true(SSL_set_min_proto_version(serverssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_max_proto_version(serverssl, TLS1_2_VERSION))
            || !TEST_true(SSL_set_cipher_list(serverssl, ciphersuite))
            || !TEST_true(SSL_set_cipher_list(clientssl, ciphersuite)))
        goto end;

    /*
     * Send the server's first flight. At this point the server has created the
     * temporary DH key but hasn't finished using it yet. Once used it is
     * removed, so we cannot test it.
     */
    if (!TEST_int_le(SSL_connect(clientssl), 0)
            || !TEST_int_le(SSL_accept(serverssl), 0))
        goto end;

    if (!TEST_int_gt(SSL_get_tmp_key(serverssl, &tmpkey), 0))
        goto end;
    if (!TEST_size_t_eq(EVP_PKEY_get_bits(tmpkey), expdhsize))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    EVP_PKEY_free(tmpkey);

    return testresult;

}
# endif /* OPENSSL_NO_DH */
#endif /* OPENSSL_NO_TLS1_2 */

#ifndef OSSL_NO_USABLE_TLS1_3
/*
 * Test that setting an SNI callback works with TLSv1.3. Specifically we check
 * that it works even without a certificate configured for the original
 * SSL_CTX
 */
static int test_sni_tls13(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL, *sctx2 = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;

    /* Reset callback counter */
    snicb = 0;

    /* Create an initial SSL_CTX with no certificate configured */
    sctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
    if (!TEST_ptr(sctx))
        goto end;
    /* Require TLSv1.3 as a minimum */
    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx2, &cctx, cert, privkey)))
        goto end;

    /* Set up SNI */
    if (!TEST_true(SSL_CTX_set_tlsext_servername_callback(sctx, sni_cb))
            || !TEST_true(SSL_CTX_set_tlsext_servername_arg(sctx, sctx2)))
        goto end;

    /*
     * Connection should still succeed because the final SSL_CTX has the right
     * certificates configured.
     */
    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(create_ssl_connection(serverssl, clientssl,
                                                SSL_ERROR_NONE)))
        goto end;

    /* We should have had the SNI callback called exactly once */
    if (!TEST_int_eq(snicb, 1))
        goto end;

    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx2);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test that the lifetime hint of a TLSv1.3 ticket is no more than 1 week
 * 0 = TLSv1.2
 * 1 = TLSv1.3
 */
static int test_ticket_lifetime(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    int version = TLS1_3_VERSION;

#define ONE_WEEK_SEC (7 * 24 * 60 * 60)
#define TWO_WEEK_SEC (2 * ONE_WEEK_SEC)

    if (idx == 0) {
#ifdef OPENSSL_NO_TLS1_2
        return TEST_skip("TLS 1.2 is disabled.");
#else
        version = TLS1_2_VERSION;
#endif
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), version, version,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    /*
     * Set the timeout to be more than 1 week
     * make sure the returned value is the default
     */
    if (!TEST_long_eq(SSL_CTX_set_timeout(sctx, TWO_WEEK_SEC),
                      SSL_get_default_timeout(serverssl)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (idx == 0) {
        /* TLSv1.2 uses the set value */
        if (!TEST_ulong_eq(SSL_SESSION_get_ticket_lifetime_hint(SSL_get_session(clientssl)), TWO_WEEK_SEC))
            goto end;
    } else {
        /* TLSv1.3 uses the limited value */
        if (!TEST_ulong_le(SSL_SESSION_get_ticket_lifetime_hint(SSL_get_session(clientssl)), ONE_WEEK_SEC))
            goto end;
    }
    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}
#endif
/*
 * Test that setting an ALPN does not violate RFC
 */
static int test_set_alpn(void)
{
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    int testresult = 0;

    unsigned char bad0[] = { 0x00, 'b', 'a', 'd' };
    unsigned char good[] = { 0x04, 'g', 'o', 'o', 'd' };
    unsigned char bad1[] = { 0x01, 'b', 'a', 'd' };
    unsigned char bad2[] = { 0x03, 'b', 'a', 'd', 0x00};
    unsigned char bad3[] = { 0x03, 'b', 'a', 'd', 0x01, 'b', 'a', 'd'};
    unsigned char bad4[] = { 0x03, 'b', 'a', 'd', 0x06, 'b', 'a', 'd'};

    /* Create an initial SSL_CTX with no certificate configured */
    ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
    if (!TEST_ptr(ctx))
        goto end;

    /* the set_alpn functions return 0 (false) on success, non-zero (true) on failure */
    if (!TEST_false(SSL_CTX_set_alpn_protos(ctx, NULL, 2)))
        goto end;
    if (!TEST_false(SSL_CTX_set_alpn_protos(ctx, good, 0)))
        goto end;
    if (!TEST_false(SSL_CTX_set_alpn_protos(ctx, good, sizeof(good))))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, good, 1)))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, bad0, sizeof(bad0))))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, bad1, sizeof(bad1))))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, bad2, sizeof(bad2))))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, bad3, sizeof(bad3))))
        goto end;
    if (!TEST_true(SSL_CTX_set_alpn_protos(ctx, bad4, sizeof(bad4))))
        goto end;

    ssl = SSL_new(ctx);
    if (!TEST_ptr(ssl))
        goto end;

    if (!TEST_false(SSL_set_alpn_protos(ssl, NULL, 2)))
        goto end;
    if (!TEST_false(SSL_set_alpn_protos(ssl, good, 0)))
        goto end;
    if (!TEST_false(SSL_set_alpn_protos(ssl, good, sizeof(good))))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, good, 1)))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, bad0, sizeof(bad0))))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, bad1, sizeof(bad1))))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, bad2, sizeof(bad2))))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, bad3, sizeof(bad3))))
        goto end;
    if (!TEST_true(SSL_set_alpn_protos(ssl, bad4, sizeof(bad4))))
        goto end;

    testresult = 1;

end:
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return testresult;
}

/*
 * Test SSL_CTX_set1_verify/chain_cert_store and SSL_CTX_get_verify/chain_cert_store.
 */
static int test_set_verify_cert_store_ssl_ctx(void)
{
   SSL_CTX *ctx = NULL;
   int testresult = 0;
   X509_STORE *store = NULL, *new_store = NULL,
              *cstore = NULL, *new_cstore = NULL;

   /* Create an initial SSL_CTX. */
   ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
   if (!TEST_ptr(ctx))
       goto end;

   /* Retrieve verify store pointer. */
   if (!TEST_true(SSL_CTX_get0_verify_cert_store(ctx, &store)))
       goto end;

   /* Retrieve chain store pointer. */
   if (!TEST_true(SSL_CTX_get0_chain_cert_store(ctx, &cstore)))
       goto end;

   /* We haven't set any yet, so this should be NULL. */
   if (!TEST_ptr_null(store) || !TEST_ptr_null(cstore))
       goto end;

   /* Create stores. We use separate stores so pointers are different. */
   new_store = X509_STORE_new();
   if (!TEST_ptr(new_store))
       goto end;

   new_cstore = X509_STORE_new();
   if (!TEST_ptr(new_cstore))
       goto end;

   /* Set stores. */
   if (!TEST_true(SSL_CTX_set1_verify_cert_store(ctx, new_store)))
       goto end;

   if (!TEST_true(SSL_CTX_set1_chain_cert_store(ctx, new_cstore)))
       goto end;

   /* Should be able to retrieve the same pointer. */
   if (!TEST_true(SSL_CTX_get0_verify_cert_store(ctx, &store)))
       goto end;

   if (!TEST_true(SSL_CTX_get0_chain_cert_store(ctx, &cstore)))
       goto end;

   if (!TEST_ptr_eq(store, new_store) || !TEST_ptr_eq(cstore, new_cstore))
       goto end;

   /* Should be able to unset again. */
   if (!TEST_true(SSL_CTX_set1_verify_cert_store(ctx, NULL)))
       goto end;

   if (!TEST_true(SSL_CTX_set1_chain_cert_store(ctx, NULL)))
       goto end;

   /* Should now be NULL. */
   if (!TEST_true(SSL_CTX_get0_verify_cert_store(ctx, &store)))
       goto end;

   if (!TEST_true(SSL_CTX_get0_chain_cert_store(ctx, &cstore)))
       goto end;

   if (!TEST_ptr_null(store) || !TEST_ptr_null(cstore))
       goto end;

   testresult = 1;

end:
   X509_STORE_free(new_store);
   X509_STORE_free(new_cstore);
   SSL_CTX_free(ctx);
   return testresult;
}

/*
 * Test SSL_set1_verify/chain_cert_store and SSL_get_verify/chain_cert_store.
 */
static int test_set_verify_cert_store_ssl(void)
{
   SSL_CTX *ctx = NULL;
   SSL *ssl = NULL;
   int testresult = 0;
   X509_STORE *store = NULL, *new_store = NULL,
              *cstore = NULL, *new_cstore = NULL;

   /* Create an initial SSL_CTX. */
   ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
   if (!TEST_ptr(ctx))
       goto end;

   /* Create an SSL object. */
   ssl = SSL_new(ctx);
   if (!TEST_ptr(ssl))
       goto end;

   /* Retrieve verify store pointer. */
   if (!TEST_true(SSL_get0_verify_cert_store(ssl, &store)))
       goto end;

   /* Retrieve chain store pointer. */
   if (!TEST_true(SSL_get0_chain_cert_store(ssl, &cstore)))
       goto end;

   /* We haven't set any yet, so this should be NULL. */
   if (!TEST_ptr_null(store) || !TEST_ptr_null(cstore))
       goto end;

   /* Create stores. We use separate stores so pointers are different. */
   new_store = X509_STORE_new();
   if (!TEST_ptr(new_store))
       goto end;

   new_cstore = X509_STORE_new();
   if (!TEST_ptr(new_cstore))
       goto end;

   /* Set stores. */
   if (!TEST_true(SSL_set1_verify_cert_store(ssl, new_store)))
       goto end;

   if (!TEST_true(SSL_set1_chain_cert_store(ssl, new_cstore)))
       goto end;

   /* Should be able to retrieve the same pointer. */
   if (!TEST_true(SSL_get0_verify_cert_store(ssl, &store)))
       goto end;

   if (!TEST_true(SSL_get0_chain_cert_store(ssl, &cstore)))
       goto end;

   if (!TEST_ptr_eq(store, new_store) || !TEST_ptr_eq(cstore, new_cstore))
       goto end;

   /* Should be able to unset again. */
   if (!TEST_true(SSL_set1_verify_cert_store(ssl, NULL)))
       goto end;

   if (!TEST_true(SSL_set1_chain_cert_store(ssl, NULL)))
       goto end;

   /* Should now be NULL. */
   if (!TEST_true(SSL_get0_verify_cert_store(ssl, &store)))
       goto end;

   if (!TEST_true(SSL_get0_chain_cert_store(ssl, &cstore)))
       goto end;

   if (!TEST_ptr_null(store) || !TEST_ptr_null(cstore))
       goto end;

   testresult = 1;

end:
   X509_STORE_free(new_store);
   X509_STORE_free(new_cstore);
   SSL_free(ssl);
   SSL_CTX_free(ctx);
   return testresult;
}


static int test_inherit_verify_param(void)
{
    int testresult = 0;

    SSL_CTX *ctx = NULL;
    X509_VERIFY_PARAM *cp = NULL;
    SSL *ssl = NULL;
    X509_VERIFY_PARAM *sp = NULL;
    int hostflags = X509_CHECK_FLAG_NEVER_CHECK_SUBJECT;

    ctx = SSL_CTX_new_ex(libctx, NULL, TLS_server_method());
    if (!TEST_ptr(ctx))
        goto end;

    cp = SSL_CTX_get0_param(ctx);
    if (!TEST_ptr(cp))
        goto end;
    if (!TEST_int_eq(X509_VERIFY_PARAM_get_hostflags(cp), 0))
        goto end;

    X509_VERIFY_PARAM_set_hostflags(cp, hostflags);

    ssl = SSL_new(ctx);
    if (!TEST_ptr(ssl))
        goto end;

    sp = SSL_get0_param(ssl);
    if (!TEST_ptr(sp))
        goto end;
    if (!TEST_int_eq(X509_VERIFY_PARAM_get_hostflags(sp), hostflags))
        goto end;

    testresult = 1;

 end:
    SSL_free(ssl);
    SSL_CTX_free(ctx);

    return testresult;
}

static int test_load_dhfile(void)
{
#ifndef OPENSSL_NO_DH
    int testresult = 0;

    SSL_CTX *ctx = NULL;
    SSL_CONF_CTX *cctx = NULL;

    if (dhfile == NULL)
        return 1;

    if (!TEST_ptr(ctx = SSL_CTX_new_ex(libctx, NULL, TLS_client_method()))
        || !TEST_ptr(cctx = SSL_CONF_CTX_new()))
        goto end;

    SSL_CONF_CTX_set_ssl_ctx(cctx, ctx);
    SSL_CONF_CTX_set_flags(cctx,
                           SSL_CONF_FLAG_CERTIFICATE
                           | SSL_CONF_FLAG_SERVER
                           | SSL_CONF_FLAG_FILE);

    if (!TEST_int_eq(SSL_CONF_cmd(cctx, "DHParameters", dhfile), 2))
        goto end;

    testresult = 1;
end:
    SSL_CONF_CTX_free(cctx);
    SSL_CTX_free(ctx);

    return testresult;
#else
    return TEST_skip("DH not supported by this build");
#endif
}

#ifndef OSSL_NO_USABLE_TLS1_3
/* Test that read_ahead works across a key change */
static int test_read_ahead_key_change(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char *msg = "Hello World";
    size_t written, readbytes;
    char buf[80];
    int i;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_read_ahead(sctx, 1);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* Write some data, send a key update, write more data */
    if (!TEST_true(SSL_write_ex(clientssl, msg, strlen(msg), &written))
        || !TEST_size_t_eq(written, strlen(msg)))
        goto end;

    if (!TEST_true(SSL_key_update(clientssl, SSL_KEY_UPDATE_NOT_REQUESTED)))
        goto end;

    if (!TEST_true(SSL_write_ex(clientssl, msg, strlen(msg), &written))
        || !TEST_size_t_eq(written, strlen(msg)))
        goto end;

    /*
     * Since read_ahead is on the first read below should read the record with
     * the first app data, the second record with the key update message, and
     * the third record with the app data all in one go. We should be able to
     * still process the read_ahead data correctly even though it crosses
     * epochs
     */
    for (i = 0; i < 2; i++) {
        if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf) - 1,
                                    &readbytes)))
            goto end;

        buf[readbytes] = '\0';
        if (!TEST_str_eq(buf, msg))
            goto end;
    }

    testresult = 1;

end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

static size_t record_pad_cb(SSL *s, int type, size_t len, void *arg)
{
    int *called = arg;

    switch ((*called)++) {
    case 0:
        /* Add some padding to first record */
        return 512;
    case 1:
        /* Maximally pad the second record */
        return SSL3_RT_MAX_PLAIN_LENGTH - len;
    case 2:
        /*
         * Exceeding the maximum padding should be fine. It should just pad to
         * the maximum anyway
         */
        return SSL3_RT_MAX_PLAIN_LENGTH + 1 - len;
    case 3:
        /*
         * Very large padding should also be ok. Should just pad to the maximum
         * allowed
         */
        return SIZE_MAX;
    default:
        return 0;
    }
}

/*
 * Test that setting record padding in TLSv1.3 works as expected
 * Test 0: Record padding callback on the SSL_CTX
 * Test 1: Record padding callback on the SSL
 * Test 2: Record block padding on the SSL_CTX
 * Test 3: Record block padding on the SSL
 * Test 4: Extended record block padding on the SSL_CTX
 * Test 5: Extended record block padding on the SSL
 */
static int test_tls13_record_padding(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    char *msg = "Hello World";
    size_t written, readbytes;
    char buf[80];
    int i;
    int called = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (idx == 0) {
        SSL_CTX_set_record_padding_callback(cctx, record_pad_cb);
        SSL_CTX_set_record_padding_callback_arg(cctx, &called);
        if (!TEST_ptr_eq(SSL_CTX_get_record_padding_callback_arg(cctx), &called))
            goto end;
    } else if (idx == 2) {
        /* Exceeding the max plain length should fail */
        if (!TEST_false(SSL_CTX_set_block_padding(cctx,
                                                  SSL3_RT_MAX_PLAIN_LENGTH + 1)))
            goto end;
        if (!TEST_true(SSL_CTX_set_block_padding(cctx, 512)))
            goto end;
    } else if (idx == 4) {
        /* pad only handshake/alert messages */
        if (!TEST_true(SSL_CTX_set_block_padding_ex(cctx, 0, 512)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (idx == 1) {
        SSL_set_record_padding_callback(clientssl, record_pad_cb);
        SSL_set_record_padding_callback_arg(clientssl, &called);
        if (!TEST_ptr_eq(SSL_get_record_padding_callback_arg(clientssl), &called))
            goto end;
    } else if (idx == 3) {
        /* Exceeding the max plain length should fail */
        if (!TEST_false(SSL_set_block_padding(clientssl,
                                              SSL3_RT_MAX_PLAIN_LENGTH + 1)))
            goto end;
        if (!TEST_true(SSL_set_block_padding(clientssl, 512)))
            goto end;
    } else if (idx == 5) {
        /* Exceeding the max plain length should fail */
        if (!TEST_false(SSL_set_block_padding_ex(clientssl, 0,
                                                 SSL3_RT_MAX_PLAIN_LENGTH + 1)))
            goto end;
        /* pad server and client handshake only */
        if (!TEST_true(SSL_set_block_padding_ex(clientssl, 0, 512)))
            goto end;
        if (!TEST_true(SSL_set_block_padding_ex(serverssl, 0, 512)))
            goto end;
    }

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    called = 0;
    /*
     * Write some data, then check we can read it. Do this four times to check
     * we can continue to write and read padded data after the initial record
     * padding has been added. We don't actually check that the padding has
     * been applied to the record - just that we can continue to communicate
     * normally and that the callback has been called (if appropriate).
     */
    for (i = 0; i < 4; i++) {
        if (!TEST_true(SSL_write_ex(clientssl, msg, strlen(msg), &written))
            || !TEST_size_t_eq(written, strlen(msg)))
            goto end;

        if (!TEST_true(SSL_read_ex(serverssl, buf, sizeof(buf) - 1,
                                    &readbytes))
                || !TEST_size_t_eq(written, readbytes))
            goto end;

        buf[readbytes] = '\0';
        if (!TEST_str_eq(buf, msg))
            goto end;
    }

    if ((idx == 0 || idx == 1) && !TEST_int_eq(called, 4))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}
#endif /* OSSL_NO_USABLE_TLS1_3 */

#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DYNAMIC_ENGINE)
/*
 * Test TLSv1.2 with a pipeline capable cipher. TLSv1.3 and DTLS do not
 * support this yet. The only pipeline capable cipher that we have is in the
 * dasync engine (providers don't support this yet), so we have to use
 * deprecated APIs for this test.
 *
 * Test 0: Client has pipelining enabled, server does not
 * Test 1: Server has pipelining enabled, client does not
 * Test 2: Client has pipelining enabled, server does not: not enough data to
 *         fill all the pipelines
 * Test 3: Client has pipelining enabled, server does not: not enough data to
 *         fill all the pipelines by more than a full pipeline's worth
 * Test 4: Client has pipelining enabled, server does not: more data than all
 *         the available pipelines can take
 * Test 5: Client has pipelining enabled, server does not: Maximum size pipeline
 * Test 6: Repeat of test 0, but the engine is loaded late (after the SSL_CTX
 *         is created)
 */
static int test_pipelining(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL, *peera, *peerb;
    int testresult = 0, numreads;
    /* A 55 byte message */
    unsigned char *msg = (unsigned char *)
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz123";
    size_t written, readbytes, offset, msglen, fragsize = 10, numpipes = 5;
    size_t expectedreads;
    unsigned char *buf = NULL;
    ENGINE *e = NULL;

    if (idx != 6) {
        e = load_dasync();
        if (e == NULL)
            return 0;
    }

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0,
                                       TLS1_2_VERSION, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    if (idx == 6) {
        e = load_dasync();
        if (e == NULL)
            goto end;
        /* Now act like test 0 */
        idx = 0;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_true(SSL_set_cipher_list(clientssl, "AES128-SHA")))
        goto end;

    /* peera is always configured for pipelining, while peerb is not. */
    if (idx == 1) {
        peera = serverssl;
        peerb = clientssl;

    } else {
        peera = clientssl;
        peerb = serverssl;
    }

    if (idx == 5) {
        numpipes = 2;
        /* Maximum allowed fragment size */
        fragsize = SSL3_RT_MAX_PLAIN_LENGTH;
        msglen = fragsize * numpipes;
        msg = OPENSSL_malloc(msglen);
        if (!TEST_ptr(msg))
            goto end;
        if (!TEST_int_gt(RAND_bytes_ex(libctx, msg, msglen, 0), 0))
            goto end;
    } else if (idx == 4) {
        msglen = 55;
    } else {
        msglen = 50;
    }
    if (idx == 2)
        msglen -= 2; /* Send 2 less bytes */
    else if (idx == 3)
        msglen -= 12; /* Send 12 less bytes */

    buf = OPENSSL_malloc(msglen);
    if (!TEST_ptr(buf))
        goto end;

    if (idx == 5) {
        /*
         * Test that setting a split send fragment longer than the maximum
         * allowed fails
         */
        if (!TEST_false(SSL_set_split_send_fragment(peera, fragsize + 1)))
            goto end;
    }

    /*
     * In the normal case. We have 5 pipelines with 10 bytes per pipeline
     * (50 bytes in total). This is a ridiculously small number of bytes -
     * but sufficient for our purposes
     */
    if (!TEST_true(SSL_set_max_pipelines(peera, numpipes))
            || !TEST_true(SSL_set_split_send_fragment(peera, fragsize)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* Write some data from peera to peerb */
    if (!TEST_true(SSL_write_ex(peera, msg, msglen, &written))
        || !TEST_size_t_eq(written, msglen))
        goto end;

    /*
     * If the pipelining code worked, then we expect all |numpipes| pipelines to
     * have been used - except in test 3 where only |numpipes - 1| pipelines
     * will be used. This will result in |numpipes| records (|numpipes - 1| for
     * test 3) having been sent to peerb. Since peerb is not using read_ahead we
     * expect this to be read in |numpipes| or |numpipes - 1| separate
     * SSL_read_ex calls. In the case of test 4, there is then one additional
     * read for left over data that couldn't fit in the previous pipelines
     */
    for (offset = 0, numreads = 0;
         offset < msglen;
         offset += readbytes, numreads++) {
        if (!TEST_true(SSL_read_ex(peerb, buf + offset,
                                   msglen - offset, &readbytes)))
            goto end;
    }

    expectedreads = idx == 4 ? numpipes + 1
                             : (idx == 3 ? numpipes - 1 : numpipes);
    if (!TEST_mem_eq(msg, msglen, buf, offset)
            || !TEST_int_eq(numreads, expectedreads))
        goto end;

    /*
     * Write some data from peerb to peera. We do this in up to |numpipes + 1|
     * chunks to exercise the read pipelining code on peera.
     */
    for (offset = 0; offset < msglen; offset += fragsize) {
        size_t sendlen = msglen - offset;

        if (sendlen > fragsize)
            sendlen = fragsize;
        if (!TEST_true(SSL_write_ex(peerb, msg + offset, sendlen, &written))
                || !TEST_size_t_eq(written, sendlen))
            goto end;
    }

    /*
     * The data was written in |numpipes|, |numpipes - 1| or |numpipes + 1|
     * separate chunks (depending on which test we are running). If the
     * pipelining is working then we expect peera to read up to numpipes chunks
     * and process them in parallel, giving back the complete result in a single
     * call to SSL_read_ex
     */
    if (!TEST_true(SSL_read_ex(peera, buf, msglen, &readbytes))
            || !TEST_size_t_le(readbytes, msglen))
        goto end;

    if (idx == 4) {
        size_t readbytes2;

        if (!TEST_true(SSL_read_ex(peera, buf + readbytes,
                                   msglen - readbytes, &readbytes2)))
            goto end;
        readbytes += readbytes2;
        if (!TEST_size_t_le(readbytes, msglen))
            goto end;
    }

    if (!TEST_mem_eq(msg, msglen, buf, readbytes))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    if (e != NULL) {
        ENGINE_unregister_ciphers(e);
        ENGINE_finish(e);
        ENGINE_free(e);
    }
    OPENSSL_free(buf);
    if (fragsize == SSL3_RT_MAX_PLAIN_LENGTH)
        OPENSSL_free(msg);
    return testresult;
}
#endif /* !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DYNAMIC_ENGINE) */

static int check_version_string(SSL *s, int version)
{
    const char *verstr = NULL;

    switch (version) {
    case SSL3_VERSION:
        verstr = "SSLv3";
        break;
    case TLS1_VERSION:
        verstr = "TLSv1";
        break;
    case TLS1_1_VERSION:
        verstr = "TLSv1.1";
        break;
    case TLS1_2_VERSION:
        verstr = "TLSv1.2";
        break;
    case TLS1_3_VERSION:
        verstr = "TLSv1.3";
        break;
    case DTLS1_VERSION:
        verstr = "DTLSv1";
        break;
    case DTLS1_2_VERSION:
        verstr = "DTLSv1.2";
    }

    return TEST_str_eq(verstr, SSL_get_version(s));
}

/*
 * Test that SSL_version, SSL_get_version, SSL_is_quic, SSL_is_tls and
 * SSL_is_dtls return the expected results for a (D)TLS connection. Compare with
 * test_version() in quicapitest.c which does the same thing for QUIC
 * connections.
 */
static int test_version(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, version;
    const SSL_METHOD *servmeth = TLS_server_method();
    const SSL_METHOD *clientmeth = TLS_client_method();

    switch (idx) {
#if !defined(OPENSSL_NO_SSL3)
    case 0:
        version = SSL3_VERSION;
        break;
#endif
#if !defined(OPENSSL_NO_TLS1)
    case 1:
        version = TLS1_VERSION;
        break;
#endif
#if !defined(OPENSSL_NO_TLS1_2)
    case 2:
        version = TLS1_2_VERSION;
        break;
#endif
#if !defined(OSSL_NO_USABLE_TLS1_3)
    case 3:
        version = TLS1_3_VERSION;
        break;
#endif
#if !defined(OPENSSL_NO_DTLS1)
    case 4:
        version = DTLS1_VERSION;
        break;
#endif
#if !defined(OPENSSL_NO_DTLS1_2)
    case 5:
        version = DTLS1_2_VERSION;
        break;
#endif
    /*
     * NB we do not support QUIC in this test. That is covered by quicapitest.c
     * We also don't support DTLS1_BAD_VER since we have no server support for
     * that.
     */
    default:
        TEST_skip("Unsupported protocol version");
        return 1;
    }

    if (is_fips
            && (version == SSL3_VERSION
                || version == TLS1_VERSION
                || version == DTLS1_VERSION)) {
        TEST_skip("Protocol version not supported with FIPS");
        return 1;
    }

#if !defined(OPENSSL_NO_DTLS)
    if (version == DTLS1_VERSION || version == DTLS1_2_VERSION) {
        servmeth = DTLS_server_method();
        clientmeth = DTLS_client_method();
    }
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, servmeth, clientmeth, version,
                                       version, &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(SSL_CTX_set_cipher_list(sctx, "DEFAULT:@SECLEVEL=0"))
            || !TEST_true(SSL_CTX_set_cipher_list(cctx,
                                                "DEFAULT:@SECLEVEL=0")))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_int_eq(SSL_version(serverssl), version)
            || !TEST_int_eq(SSL_version(clientssl), version)
            || !TEST_true(check_version_string(serverssl, version))
            || !TEST_true(check_version_string(clientssl, version)))
        goto end;

    if (version == DTLS1_VERSION || version == DTLS1_2_VERSION) {
        if (!TEST_true(SSL_is_dtls(serverssl))
                || !TEST_true(SSL_is_dtls(clientssl))
                || !TEST_false(SSL_is_tls(serverssl))
                || !TEST_false(SSL_is_tls(clientssl))
                || !TEST_false(SSL_is_quic(serverssl))
                || !TEST_false(SSL_is_quic(clientssl)))
        goto end;
    } else {
        if (!TEST_true(SSL_is_tls(serverssl))
                || !TEST_true(SSL_is_tls(clientssl))
                || !TEST_false(SSL_is_dtls(serverssl))
                || !TEST_false(SSL_is_dtls(clientssl))
                || !TEST_false(SSL_is_quic(serverssl))
                || !TEST_false(SSL_is_quic(clientssl)))
        goto end;
    }

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Test that the SSL_rstate_string*() APIs return sane results
 */
static int test_rstate_string(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0, version;
    const SSL_METHOD *servmeth = TLS_server_method();
    const SSL_METHOD *clientmeth = TLS_client_method();
    size_t written, readbytes;
    unsigned char buf[2];
    unsigned char dummyheader[SSL3_RT_HEADER_LENGTH] = {
        SSL3_RT_APPLICATION_DATA,
        TLS1_2_VERSION_MAJOR,
        0, /* To be filled in later */
        0,
        1
    };

    if (!TEST_true(create_ssl_ctx_pair(libctx, servmeth, clientmeth, 0,
                                       0, &sctx, &cctx, cert, privkey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    if (!TEST_str_eq(SSL_rstate_string(serverssl), "RH")
            || !TEST_str_eq(SSL_rstate_string_long(serverssl), "read header"))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_str_eq(SSL_rstate_string(serverssl), "RH")
            || !TEST_str_eq(SSL_rstate_string_long(serverssl), "read header"))
        goto end;

    /* Fill in the correct version for the record header */
    version = SSL_version(serverssl);
    if (version == TLS1_3_VERSION)
        version = TLS1_2_VERSION;
    dummyheader[2] = version & 0xff;

    /*
     * Send a dummy header. If we continued to read the body as well this
     * would fail with a bad record mac, but we're not going to go that far.
     */
    if (!TEST_true(BIO_write_ex(SSL_get_rbio(serverssl), dummyheader,
                                sizeof(dummyheader), &written))
            || !TEST_size_t_eq(written, SSL3_RT_HEADER_LENGTH))
        goto end;

    if (!TEST_false(SSL_read_ex(serverssl, buf, sizeof(buf), &readbytes)))
        goto end;

    if (!TEST_str_eq(SSL_rstate_string(serverssl), "RB")
            || !TEST_str_eq(SSL_rstate_string_long(serverssl), "read body"))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    return testresult;
}

/*
 * Force a write retry during handshaking. We test various combinations of
 * scenarios. We test a large certificate message which will fill the buffering
 * BIO used in the handshake. We try with client auth on and off. Finally we
 * also try a BIO that indicates retry via a 0 return. BIO_write() is documented
 * to indicate retry via -1 - but sometimes BIOs don't do that.
 *
 * Test 0: Standard certificate message
 * Test 1: Large certificate message
 * Test 2: Standard cert, verify peer
 * Test 3: Large cert, verify peer
 * Test 4: Standard cert, BIO returns 0 on retry
 * Test 5: Large cert, BIO returns 0 on retry
 * Test 6: Standard cert, verify peer, BIO returns 0 on retry
 * Test 7: Large cert, verify peer, BIO returns 0 on retry
 * Test 8-15: Repeat of above with TLSv1.2
 */
static int test_handshake_retry(int idx)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    BIO *tmp = NULL, *bretry = BIO_new(bio_s_always_retry());
    int maxversion = 0;

    if (!TEST_ptr(bretry))
        goto end;

#ifndef OPENSSL_NO_TLS1_2
    if ((idx & 8) == 8)
        maxversion = TLS1_2_VERSION;
#else
    if ((idx & 8) == 8)
        return TEST_skip("No TLSv1.2");
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0, maxversion,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    /*
     * Add a large amount of data to fill the buffering BIO used by the SSL
     * object
     */
    if ((idx & 1) == 1 && !ssl_ctx_add_large_cert_chain(libctx, sctx, cert))
        goto end;

    /*
     * We don't actually configure a client cert, but neither do we fail if one
     * isn't present.
     */
    if ((idx & 2) == 2)
        SSL_CTX_set_verify(sctx, SSL_VERIFY_PEER, NULL);

    if ((idx & 4) == 4)
        set_always_retry_err_val(0);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL)))
        goto end;

    tmp = SSL_get_wbio(serverssl);
    if (!TEST_ptr(tmp) || !TEST_true(BIO_up_ref(tmp))) {
        tmp = NULL;
        goto end;
    }
    SSL_set0_wbio(serverssl, bretry);
    bretry = NULL;

    if (!TEST_int_eq(SSL_connect(clientssl), -1))
        goto end;

    if (!TEST_int_eq(SSL_accept(serverssl), -1)
            || !TEST_int_eq(SSL_get_error(serverssl, -1), SSL_ERROR_WANT_WRITE))
        goto end;

    /* Restore a BIO that will let the write succeed */
    SSL_set0_wbio(serverssl, tmp);
    tmp = NULL;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    BIO_free(bretry);
    BIO_free(tmp);
    set_always_retry_err_val(-1);
    return testresult;
}

/*
 * Test that receiving retries when writing application data works as expected
 */
static int test_data_retry(void)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    unsigned char inbuf[1200], outbuf[1200];
    size_t i;
    BIO *tmp = NULL;
    BIO *bretry = BIO_new(bio_s_maybe_retry());
    size_t written, readbytes, totread = 0;

    if (!TEST_ptr(bretry))
        goto end;

    for (i = 0; i < sizeof(inbuf); i++)
        inbuf[i] = (unsigned char)(0xff & i);
    memset(outbuf, 0, sizeof(outbuf));

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0, 0, &sctx, &cctx,
                                       cert, privkey)))
        goto end;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    /* Smallest possible max send fragment is 512 */
    if (!TEST_true(SSL_set_max_send_fragment(clientssl, 512)))
        goto end;

    tmp = SSL_get_wbio(clientssl);
    if (!TEST_ptr(tmp))
        goto end;
    if (!TEST_true(BIO_up_ref(tmp)))
        goto end;
    BIO_push(bretry, tmp);
    tmp = NULL;
    SSL_set0_wbio(clientssl, bretry);
    if (!BIO_up_ref(bretry)) {
        bretry = NULL;
        goto end;
    }

    for (i = 0; i < 3; i++) {
        /* We expect this call to make no progress and indicate retry */
        if (!TEST_false(SSL_write_ex(clientssl, inbuf, sizeof(inbuf), &written)))
            goto end;
        if (!TEST_int_eq(SSL_get_error(clientssl, 0), SSL_ERROR_WANT_WRITE))
            goto end;

        /* Allow one write to progress, but the next one to signal retry */
        if (!TEST_true(BIO_ctrl(bretry, MAYBE_RETRY_CTRL_SET_RETRY_AFTER_CNT, 1,
                                NULL)))
            goto end;

        if (i == 2)
            break;

        /*
         * This call will hopefully make progress but will still indicate retry
         * because there is more data than will fit into a single record.
         */
        if (!TEST_false(SSL_write_ex(clientssl, inbuf, sizeof(inbuf), &written)))
            goto end;
        if (!TEST_int_eq(SSL_get_error(clientssl, 0), SSL_ERROR_WANT_WRITE))
            goto end;
    }

    /* The final call should write the last chunk of data and succeed */
    if (!TEST_true(SSL_write_ex(clientssl, inbuf, sizeof(inbuf), &written)))
        goto end;
    /* Read all the data available */
    while (SSL_read_ex(serverssl, outbuf + totread, sizeof(outbuf) - totread,
                       &readbytes))
        totread += readbytes;
    if (!TEST_mem_eq(inbuf, sizeof(inbuf), outbuf, totread))
        goto end;

    testresult = 1;
end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    BIO_free_all(bretry);
    BIO_free(tmp);
    return testresult;
}

struct resume_servername_cb_data {
    int i;
    SSL_CTX *cctx;
    SSL_CTX *sctx;
    SSL_SESSION *sess;
    int recurse;
};

/*
 * Servername callback. We use it here to run another complete handshake using
 * the same session - and mark the session as not_resuamble at the end
 */
static int resume_servername_cb(SSL *s, int *ad, void *arg)
{
    struct resume_servername_cb_data *cbdata = arg;
    SSL *serverssl = NULL, *clientssl = NULL;
    int ret = SSL_TLSEXT_ERR_ALERT_FATAL;

    if (cbdata->recurse)
        return SSL_TLSEXT_ERR_ALERT_FATAL;

    if ((cbdata->i % 3) != 1)
        return SSL_TLSEXT_ERR_OK;

    cbdata->recurse = 1;

    if (!TEST_true(create_ssl_objects(cbdata->sctx, cbdata->cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, cbdata->sess)))
        goto end;

    ERR_set_mark();
    /*
     * We expect this to fail - because the servername cb will fail. This will
     * mark the session as not_resumable.
     */
    if (!TEST_false(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE))) {
        ERR_clear_last_mark();
        goto end;
    }
    ERR_pop_to_mark();

    ret = SSL_TLSEXT_ERR_OK;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    cbdata->recurse = 0;
    return ret;
}
/*
 * Test multiple resumptions and cache size handling
 * Test 0: TLSv1.3 (max_early_data set)
 * Test 1: TLSv1.3 (SSL_OP_NO_TICKET set)
 * Test 2: TLSv1.3 (max_early_data and SSL_OP_NO_TICKET set)
 * Test 3: TLSv1.3 (SSL_OP_NO_TICKET, simultaneous resumes)
 * Test 4: TLSv1.2
 */
static int test_multi_resume(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    SSL_SESSION *sess = NULL;
    int max_version = TLS1_3_VERSION;
    int i, testresult = 0;
    struct resume_servername_cb_data cbdata;

#if defined(OPENSSL_NO_TLS1_2)
    if (idx == 4)
        return TEST_skip("TLSv1.2 is disabled in this build");
#else
    if (idx == 4)
        max_version = TLS1_2_VERSION;
#endif
#if defined(OSSL_NO_USABLE_TLS1_3)
    if (idx != 4)
        return TEST_skip("No usable TLSv1.3 in this build");
#endif

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_VERSION,
                                       max_version, &sctx, &cctx, cert,
                                       privkey)))
        goto end;

    /*
     * TLSv1.3 only uses a session cache if either max_early_data > 0 (used for
     * replay protection), or if SSL_OP_NO_TICKET is in use
     */
    if (idx == 0 || idx == 2)  {
        if (!TEST_true(SSL_CTX_set_max_early_data(sctx, 1024)))
            goto end;
    }
    if (idx == 1 || idx == 2 || idx == 3)
        SSL_CTX_set_options(sctx, SSL_OP_NO_TICKET);

    SSL_CTX_sess_set_cache_size(sctx, 5);

    if (idx == 3) {
        SSL_CTX_set_tlsext_servername_callback(sctx, resume_servername_cb);
        SSL_CTX_set_tlsext_servername_arg(sctx, &cbdata);
        cbdata.cctx = cctx;
        cbdata.sctx = sctx;
        cbdata.recurse = 0;
    }

    for (i = 0; i < 30; i++) {
        if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                                NULL, NULL))
                || !TEST_true(SSL_set_session(clientssl, sess)))
            goto end;

        /*
         * Check simultaneous resumes. We pause the connection part way through
         * the handshake by (mis)using the servername_cb. The pause occurs after
         * session resumption has already occurred, but before any session
         * tickets have been issued. While paused we run another complete
         * handshake resuming the same session.
         */
        if (idx == 3) {
            cbdata.i = i;
            cbdata.sess = sess;
        }

        /*
         * Recreate a bug where dynamically changing the max_early_data value
         * can cause sessions in the session cache which cannot be deleted.
         */
        if ((idx == 0 || idx == 2) && (i % 3) == 2)
            SSL_set_max_early_data(serverssl, 0);

        if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
            goto end;

        if (sess == NULL || (idx == 0 && (i % 3) == 2)) {
            if (!TEST_false(SSL_session_reused(clientssl)))
                goto end;
        } else {
            if (!TEST_true(SSL_session_reused(clientssl)))
                goto end;
        }
        SSL_SESSION_free(sess);

        /* Do a full handshake, followed by two resumptions */
        if ((i % 3) == 2) {
            sess = NULL;
        } else {
            if (!TEST_ptr((sess = SSL_get1_session(clientssl))))
                goto end;
        }

        SSL_shutdown(clientssl);
        SSL_shutdown(serverssl);
        SSL_free(serverssl);
        SSL_free(clientssl);
        serverssl = clientssl = NULL;
    }

    /* We should never exceed the session cache size limit */
    if (!TEST_long_le(SSL_CTX_sess_number(sctx), 5))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);
    SSL_SESSION_free(sess);
    return testresult;
}

static struct next_proto_st {
    int serverlen;
    unsigned char server[40];
    int clientlen;
    unsigned char client[40];
    int expected_ret;
    size_t selectedlen;
    unsigned char selected[40];
} next_proto_tests[] = {
    {
        4, { 3, 'a', 'b', 'c' },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        7, { 3, 'a', 'b', 'c', 2, 'a', 'b' },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        7, { 2, 'a', 'b', 3, 'a', 'b', 'c', },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        4, { 3, 'a', 'b', 'c' },
        7, { 3, 'a', 'b', 'c', 2, 'a', 'b', },
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        4, { 3, 'a', 'b', 'c' },
        7, { 2, 'a', 'b', 3, 'a', 'b', 'c'},
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        7, { 2, 'b', 'c', 3, 'a', 'b', 'c' },
        7, { 2, 'a', 'b', 3, 'a', 'b', 'c'},
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        10, { 2, 'b', 'c', 3, 'a', 'b', 'c', 2, 'a', 'b' },
        7, { 2, 'a', 'b', 3, 'a', 'b', 'c'},
        OPENSSL_NPN_NEGOTIATED,
        3, { 'a', 'b', 'c' }
    },
    {
        4, { 3, 'b', 'c', 'd' },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NO_OVERLAP,
        3, { 'a', 'b', 'c' }
    },
    {
        0, { 0 },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NO_OVERLAP,
        3, { 'a', 'b', 'c' }
    },
    {
        -1, { 0 },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NO_OVERLAP,
        3, { 'a', 'b', 'c' }
    },
    {
        4, { 3, 'a', 'b', 'c' },
        0, { 0 },
        OPENSSL_NPN_NO_OVERLAP,
        0, { 0 }
    },
    {
        4, { 3, 'a', 'b', 'c' },
        -1, { 0 },
        OPENSSL_NPN_NO_OVERLAP,
        0, { 0 }
    },
    {
        3, { 3, 'a', 'b', 'c' },
        4, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NO_OVERLAP,
        3, { 'a', 'b', 'c' }
    },
    {
        4, { 3, 'a', 'b', 'c' },
        3, { 3, 'a', 'b', 'c' },
        OPENSSL_NPN_NO_OVERLAP,
        0, { 0 }
    }
};

static int test_select_next_proto(int idx)
{
    struct next_proto_st *np = &next_proto_tests[idx];
    int ret = 0;
    unsigned char *out, *client, *server;
    unsigned char outlen;
    unsigned int clientlen, serverlen;

    if (np->clientlen == -1) {
        client = NULL;
        clientlen = 0;
    } else {
        client = np->client;
        clientlen = (unsigned int)np->clientlen;
    }
    if (np->serverlen == -1) {
        server = NULL;
        serverlen = 0;
    } else {
        server = np->server;
        serverlen = (unsigned int)np->serverlen;
    }

    if (!TEST_int_eq(SSL_select_next_proto(&out, &outlen, server, serverlen,
                                           client, clientlen),
                     np->expected_ret))
        goto err;

    if (np->selectedlen == 0) {
        if (!TEST_ptr_null(out) || !TEST_uchar_eq(outlen, 0))
            goto err;
    } else {
        if (!TEST_mem_eq(out, outlen, np->selected, np->selectedlen))
            goto err;
    }

    ret = 1;
 err:
    return ret;
}

static const unsigned char fooprot[] = {3, 'f', 'o', 'o' };
static const unsigned char barprot[] = {3, 'b', 'a', 'r' };

#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_NEXTPROTONEG)
static int npn_advert_cb(SSL *ssl, const unsigned char **out,
                         unsigned int *outlen, void *arg)
{
    int *idx = (int *)arg;

    switch (*idx) {
    default:
    case 0:
        *out = fooprot;
        *outlen = sizeof(fooprot);
        return SSL_TLSEXT_ERR_OK;

    case 1:
        *out = NULL;
        *outlen = 0;
        return SSL_TLSEXT_ERR_OK;

    case 2:
        return SSL_TLSEXT_ERR_NOACK;
    }
}

static int npn_select_cb(SSL *s, unsigned char **out, unsigned char *outlen,
                         const unsigned char *in, unsigned int inlen, void *arg)
{
    int *idx = (int *)arg;

    switch (*idx) {
    case 0:
    case 1:
        *out = (unsigned char *)(fooprot + 1);
        *outlen = *fooprot;
        return SSL_TLSEXT_ERR_OK;

    case 3:
        *out = (unsigned char *)(barprot + 1);
        *outlen = *barprot;
        return SSL_TLSEXT_ERR_OK;

    case 4:
        *outlen = 0;
        return SSL_TLSEXT_ERR_OK;

    default:
    case 2:
        return SSL_TLSEXT_ERR_ALERT_FATAL;
    }
}

/*
 * Test the NPN callbacks
 * Test 0: advert = foo, select = foo
 * Test 1: advert = <empty>, select = foo
 * Test 2: no advert
 * Test 3: advert = foo, select = bar
 * Test 4: advert = foo, select = <empty> (should fail)
 */
static int test_npn(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0, TLS1_2_VERSION,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_next_protos_advertised_cb(sctx, npn_advert_cb, &idx);
    SSL_CTX_set_next_proto_select_cb(cctx, npn_select_cb, &idx);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (idx == 4) {
        /* We don't allow empty selection of NPN, so this should fail */
        if (!TEST_false(create_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE)))
            goto end;
    } else {
        const unsigned char *prot;
        unsigned int protlen;

        if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                             SSL_ERROR_NONE)))
            goto end;

        SSL_get0_next_proto_negotiated(serverssl, &prot, &protlen);
        switch (idx) {
        case 0:
        case 1:
            if (!TEST_mem_eq(prot, protlen, fooprot + 1, *fooprot))
                goto end;
            break;
        case 2:
            if (!TEST_uint_eq(protlen, 0))
                goto end;
            break;
        case 3:
            if (!TEST_mem_eq(prot, protlen, barprot + 1, *barprot))
                goto end;
            break;
        default:
            TEST_error("Should not get here");
            goto end;
        }
    }

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_NEXTPROTONEG) */

static int alpn_select_cb2(SSL *ssl, const unsigned char **out,
                           unsigned char *outlen, const unsigned char *in,
                           unsigned int inlen, void *arg)
{
    int *idx = (int *)arg;

    switch (*idx) {
    case 0:
        *out = (unsigned char *)(fooprot + 1);
        *outlen = *fooprot;
        return SSL_TLSEXT_ERR_OK;

    case 2:
        *out = (unsigned char *)(barprot + 1);
        *outlen = *barprot;
        return SSL_TLSEXT_ERR_OK;

    case 3:
        *outlen = 0;
        return SSL_TLSEXT_ERR_OK;

    default:
    case 1:
        return SSL_TLSEXT_ERR_ALERT_FATAL;
    }
    return 0;
}

/*
 * Test the ALPN callbacks
 * Test 0: client = foo, select = foo
 * Test 1: client = <empty>, select = none
 * Test 2: client = foo, select = bar (should fail)
 * Test 3: client = foo, select = <empty> (should fail)
 */
static int test_alpn(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    const unsigned char *prots = fooprot;
    unsigned int protslen = sizeof(fooprot);

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), 0, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_alpn_select_cb(sctx, alpn_select_cb2, &idx);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (idx == 1) {
        prots = NULL;
        protslen = 0;
    }

    /* SSL_set_alpn_protos returns 0 for success! */
    if (!TEST_false(SSL_set_alpn_protos(clientssl, prots, protslen)))
        goto end;

    if (idx == 2 || idx == 3) {
        /* We don't allow empty selection of NPN, so this should fail */
        if (!TEST_false(create_ssl_connection(serverssl, clientssl,
                                              SSL_ERROR_NONE)))
            goto end;
    } else {
        const unsigned char *prot;
        unsigned int protlen;

        if (!TEST_true(create_ssl_connection(serverssl, clientssl,
                                             SSL_ERROR_NONE)))
            goto end;

        SSL_get0_alpn_selected(clientssl, &prot, &protlen);
        switch (idx) {
        case 0:
            if (!TEST_mem_eq(prot, protlen, fooprot + 1, *fooprot))
                goto end;
            break;
        case 1:
            if (!TEST_uint_eq(protlen, 0))
                goto end;
            break;
        default:
            TEST_error("Should not get here");
            goto end;
        }
    }

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

#if !defined(OSSL_NO_USABLE_TLS1_3)
struct quic_tls_test_data {
    struct quic_tls_test_data *peer;
    uint32_t renc_level;
    uint32_t wenc_level;
    unsigned char rcd_data[4][2048];
    size_t rcd_data_len[4];
    unsigned char rsecret[3][48];
    size_t rsecret_len[3];
    unsigned char wsecret[3][48];
    size_t wsecret_len[3];
    unsigned char params[3];
    size_t params_len;
    int alert;
    int err;
    int forcefail;
    int sm_count;
};

static int clientquicdata = 0xff, serverquicdata = 0xfe;

static int check_app_data(SSL *s)
{
    int *data, *comparedata;

    /* Check app data works */
    data = (int *)SSL_get_app_data(s);
    comparedata = SSL_is_server(s) ? &serverquicdata : &clientquicdata;

    if (!TEST_true(comparedata == data))
        return 0;

    return 1;
}

static int crypto_send_cb(SSL *s, const unsigned char *buf, size_t buf_len,
                          size_t *consumed, void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;
    struct quic_tls_test_data *peer = data->peer;
    size_t max_len = sizeof(peer->rcd_data[data->wenc_level])
                     - peer->rcd_data_len[data->wenc_level];

    if (!check_app_data(s)) {
        data->err = 1;
        return 0;
    }

    if (buf_len > max_len)
        buf_len = max_len;

    if (buf_len == 0) {
        *consumed = 0;
        return 1;
    }

    memcpy(peer->rcd_data[data->wenc_level]
           + peer->rcd_data_len[data->wenc_level], buf, buf_len);
    peer->rcd_data_len[data->wenc_level] += buf_len;

    *consumed = buf_len;
    return 1;
}
static int crypto_recv_rcd_cb(SSL *s, const unsigned char **buf,
                              size_t *bytes_read, void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;

    if (!check_app_data(s)) {
        data->err = 1;
        return 0;
    }

    *bytes_read = data->rcd_data_len[data->renc_level];
    *buf = data->rcd_data[data->renc_level];
    return 1;
}

static int crypto_release_rcd_cb(SSL *s, size_t bytes_read, void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;

    if (!check_app_data(s)) {
        data->err = 1;
        return 0;
    }

    /* See if we need to force a failure in this callback */
    if (data->forcefail) {
        data->forcefail = 0;
        data->err = 1;
        return 0;
    }

    if (!TEST_size_t_eq(bytes_read, data->rcd_data_len[data->renc_level])
            || !TEST_size_t_gt(bytes_read, 0)) {
        data->err = 1;
        return 0;
    }
    data->rcd_data_len[data->renc_level] = 0;

    return 1;
}

struct secret_yield_entry {
    uint8_t recorded;
    int prot_level;
    int direction;
    int sm_generation;
    SSL *ssl;
};

static struct secret_yield_entry secret_history[16];
static int secret_history_idx = 0;
/*
 * Note, this enum needs to match the direction values passed
 * to yield_secret_cb
 */
typedef enum {
    LAST_DIR_READ = 0,
    LAST_DIR_WRITE = 1,
    LAST_DIR_UNSET = 2
} last_dir_history_state;

static int check_secret_history(SSL *s)
{
    int i;
    int ret = 0;
    last_dir_history_state last_state = LAST_DIR_UNSET;
    int last_prot_level = 0;
    int last_generation = 0;

    TEST_info("Checking history for %p\n", (void *)s);
    for (i = 0; secret_history[i].recorded == 1; i++) {
        if (secret_history[i].ssl != s)
            continue;
        TEST_info("Got %s(%d) secret for level %d, last level %d, last state %d, gen %d\n",
                  secret_history[i].direction == 1 ? "Write" : "Read", secret_history[i].direction,
                  secret_history[i].prot_level, last_prot_level, last_state,
                  secret_history[i].sm_generation);

        if (last_state == LAST_DIR_UNSET) {
            last_prot_level = secret_history[i].prot_level;
            last_state = secret_history[i].direction;
            last_generation = secret_history[i].sm_generation;
            continue;
        }

        switch(secret_history[i].direction) {
        case 1:
            /*
             * write case
             * NOTE: There is an odd corner case here.  It may occur that
             * in a single iteration of the state machine, the read key is yielded
             * prior to the write key for the same level.  This is undesireable
             * for quic, but it is ok, as the general implementation of every 3rd
             * party quic stack while prefering write keys before read, allows
             * for read before write if both keys are yielded in the same call
             * to SSL_do_handshake, as the tls adaptation code for that quic stack
             * can then cache keys until both are available, so we allow read before
             * write here iff they occur in the same iteration of SSL_do_handshake
             * as represented by the recorded sm_generation value.
             */
            if (last_prot_level == secret_history[i].prot_level
                && last_state == LAST_DIR_READ) {
                if (last_generation == secret_history[i].sm_generation) {
                    TEST_info("Read before write key in same SSL state machine iteration is ok");
                } else {
                    TEST_error("Got read key before write key");
                    goto end;
                }
            }
            /* FALLTHROUGH */
        case 0:
            /*
             * Read case
             */
            break;
        default:
            TEST_error("Unknown direction");
            goto end;
        }
        last_prot_level = secret_history[i].prot_level;
        last_state = secret_history[i].direction;
        last_generation = secret_history[i].sm_generation;
    }

    ret = 1;
end:
    return ret;
}

static int yield_secret_cb(SSL *s, uint32_t prot_level, int direction,
                           const unsigned char *secret, size_t secret_len,
                           void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;

    if (!check_app_data(s))
        goto err;

    if (prot_level < OSSL_RECORD_PROTECTION_LEVEL_EARLY
            || prot_level > OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
        goto err;

    switch (direction) {
    case 0: /* read */
        if (!TEST_size_t_le(secret_len, sizeof(data->rsecret)))
            goto err;
        data->renc_level = prot_level;
        memcpy(data->rsecret[prot_level - 1], secret, secret_len);
        data->rsecret_len[prot_level - 1] = secret_len;
        break;

    case 1: /* write */
        if (!TEST_size_t_le(secret_len, sizeof(data->wsecret)))
            goto err;
        data->wenc_level = prot_level;
        memcpy(data->wsecret[prot_level - 1], secret, secret_len);
        data->wsecret_len[prot_level - 1] = secret_len;
        break;

    default:
        goto err;
    }

    secret_history[secret_history_idx].direction = direction;
    secret_history[secret_history_idx].prot_level = (int)prot_level;
    secret_history[secret_history_idx].recorded = 1;
    secret_history[secret_history_idx].ssl = s;
    secret_history[secret_history_idx].sm_generation = data->sm_count;
    secret_history_idx++;
    return 1;
 err:
    data->err = 1;
    return 0;
}

static int yield_secret_cb_fail(SSL *s, uint32_t prot_level, int direction,
                                const unsigned char *secret, size_t secret_len,
                                void *arg)
{
    (void)s;
    (void)prot_level;
    (void)direction;
    (void)secret;
    (void)secret_len;
    (void)arg;
    /*
     * This callback is to test double free in quic tls
     */
    return 0;
}

static int got_transport_params_cb(SSL *s, const unsigned char *params,
                                   size_t params_len,
                                   void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;

    if (!check_app_data(s)) {
        data->err = 1;
        return 0;
    }

    if (!TEST_size_t_le(params_len, sizeof(data->params))) {
        data->err = 1;
        return 0;
    }

    memcpy(data->params, params, params_len);
    data->params_len = params_len;

    return 1;
}

static int alert_cb(SSL *s, unsigned char alert_code, void *arg)
{
    struct quic_tls_test_data *data = (struct quic_tls_test_data *)arg;

    if (!check_app_data(s)) {
        data->err = 1;
        return 0;
    }

    data->alert = 1;
    return 1;
}

/*
 * Test the QUIC TLS API
 * Test 0: Normal run
 * Test 1: Force a failure
 * Test 3: Use a CCM based ciphersuite
 * Test 4: fail yield_secret_cb to see double free
 * Test 5: Normal run with SNI
 */
static int test_quic_tls(int idx)
{
    SSL_CTX *sctx = NULL, *sctx2 = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    OSSL_DISPATCH qtdis[] = {
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_SEND, (void (*)(void))crypto_send_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RECV_RCD,
         (void (*)(void))crypto_recv_rcd_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RELEASE_RCD,
         (void (*)(void))crypto_release_rcd_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_YIELD_SECRET,
         (void (*)(void))yield_secret_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_GOT_TRANSPORT_PARAMS,
         (void (*)(void))got_transport_params_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_ALERT, (void (*)(void))alert_cb},
        {0, NULL}
    };
    struct quic_tls_test_data sdata, cdata;
    const unsigned char cparams[] = {
        0xff, 0x01, 0x00
    };
    const unsigned char sparams[] = {
        0xfe, 0x01, 0x00
    };
    int i;

    if (idx == 4)
        qtdis[3].function = (void (*)(void))yield_secret_cb_fail;

    snicb = 0;
    memset(secret_history, 0, sizeof(secret_history));
    secret_history_idx = 0;
    memset(&sdata, 0, sizeof(sdata));
    memset(&cdata, 0, sizeof(cdata));
    sdata.peer = &cdata;
    cdata.peer = &sdata;
    if (idx == 1)
        sdata.forcefail = 1;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    if (idx == 5) {
        if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(), NULL,
                                           TLS1_3_VERSION, 0,
                                           &sctx2, NULL, cert, privkey)))
            goto end;

        /* Set up SNI */
        if (!TEST_true(SSL_CTX_set_tlsext_servername_callback(sctx, sni_cb))
                || !TEST_true(SSL_CTX_set_tlsext_servername_arg(sctx, sctx2)))
            goto end;
    }

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    /* Reset the BIOs we set in create_ssl_objects. We should not need them */
    SSL_set_bio(serverssl, NULL, NULL);
    SSL_set_bio(clientssl, NULL, NULL);

    if (idx == 2) {
        if (!TEST_true(SSL_set_ciphersuites(serverssl, "TLS_AES_128_CCM_SHA256"))
                || !TEST_true(SSL_set_ciphersuites(clientssl, "TLS_AES_128_CCM_SHA256")))
            goto end;
    }

    if (!TEST_true(SSL_set_app_data(clientssl, &clientquicdata))
            || !TEST_true(SSL_set_app_data(serverssl, &serverquicdata)))
        goto end;

    if (!TEST_true(SSL_set_quic_tls_cbs(clientssl, qtdis, &cdata))
            || !TEST_true(SSL_set_quic_tls_cbs(serverssl, qtdis, &sdata))
            || !TEST_true(SSL_set_quic_tls_transport_params(clientssl, cparams,
                                                            sizeof(cparams)))
            || !TEST_true(SSL_set_quic_tls_transport_params(serverssl, sparams,
                                                            sizeof(sparams))))
        goto end;

    if (idx != 1 && idx != 4) {
        if (!TEST_true(create_ssl_connection_ex(serverssl, clientssl, SSL_ERROR_NONE,
                                                &cdata.sm_count, &sdata.sm_count)))
            goto end;
    } else {
        /* We expect this connection to fail */
        if (!TEST_false(create_ssl_connection_ex(serverssl, clientssl, SSL_ERROR_NONE,
                                                 &cdata.sm_count, &sdata.sm_count)))
            goto end;
        testresult = 1;
        sdata.err = 0;
        goto end;
    }

    /* We should have had the SNI callback called exactly once */
    if (idx == 5) {
        if (!TEST_int_eq(snicb, 1))
            goto end;
    }

    /* Check no problems during the handshake */
    if (!TEST_false(sdata.alert)
            || !TEST_false(cdata.alert)
            || !TEST_false(sdata.err)
            || !TEST_false(cdata.err))
        goto end;

    /* Check the secrets all match */
    for (i = OSSL_RECORD_PROTECTION_LEVEL_EARLY - 1;
         i < OSSL_RECORD_PROTECTION_LEVEL_APPLICATION;
         i++) {
        if (!TEST_mem_eq(sdata.wsecret[i], sdata.wsecret_len[i],
                         cdata.rsecret[i], cdata.rsecret_len[i]))
            goto end;
    }

    /*
     * Check that our secret history yields write secrets before read secrets
     */
    if (!TEST_int_eq(check_secret_history(serverssl), 1))
        goto end;
    if (!TEST_int_eq(check_secret_history(clientssl), 1))
        goto end;

    /* Check the transport params */
    if (!TEST_mem_eq(sdata.params, sdata.params_len, cparams, sizeof(cparams))
            || !TEST_mem_eq(cdata.params, cdata.params_len, sparams,
                            sizeof(sparams)))
        goto end;

    /* Check the encryption levels are what we expect them to be */
    if (!TEST_true(sdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(sdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(cdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(cdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx2);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    /* Check that we didn't suddenly hit an unexpected failure during cleanup */
    if (!TEST_false(sdata.err) || !TEST_false(cdata.err))
        testresult = 0;

    return testresult;
}

static void assert_no_end_of_early_data(int write_p, int version, int content_type,
                                        const void *buf, size_t msglen, SSL *ssl, void *arg)
{
    const unsigned char *msg = buf;

    if (content_type == SSL3_RT_HANDSHAKE && msg[0] == SSL3_MT_END_OF_EARLY_DATA)
        end_of_early_data = 1;
}

static int test_quic_tls_early_data(void)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0;
    SSL_SESSION *sess = NULL;
    const OSSL_DISPATCH qtdis[] = {
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_SEND, (void (*)(void))crypto_send_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RECV_RCD,
         (void (*)(void))crypto_recv_rcd_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RELEASE_RCD,
         (void (*)(void))crypto_release_rcd_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_YIELD_SECRET,
         (void (*)(void))yield_secret_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_GOT_TRANSPORT_PARAMS,
         (void (*)(void))got_transport_params_cb},
        {OSSL_FUNC_SSL_QUIC_TLS_ALERT, (void (*)(void))alert_cb},
        {0, NULL}
    };
    struct quic_tls_test_data sdata, cdata;
    const unsigned char cparams[] = {
        0xff, 0x01, 0x00
    };
    const unsigned char sparams[] = {
        0xfe, 0x01, 0x00
    };
    int i;

    memset(secret_history, 0, sizeof(secret_history));
    secret_history_idx = 0;
    memset(&sdata, 0, sizeof(sdata));
    memset(&cdata, 0, sizeof(cdata));
    sdata.peer = &cdata;
    cdata.peer = &sdata;
    end_of_early_data = 0;

    if (!TEST_true(create_ssl_ctx_pair(libctx, TLS_server_method(),
                                       TLS_client_method(), TLS1_3_VERSION, 0,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_max_early_data(sctx, 0xffffffff);
    SSL_CTX_set_max_early_data(cctx, 0xffffffff);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    sess = SSL_get1_session(clientssl);
    SSL_shutdown(clientssl);
    SSL_shutdown(serverssl);
    SSL_free(serverssl);
    SSL_free(clientssl);
    serverssl = clientssl = NULL;

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl,
                                      &clientssl, NULL, NULL))
            || !TEST_true(SSL_set_session(clientssl, sess)))
        goto end;

    /* Reset the BIOs we set in create_ssl_objects. We should not need them */
    SSL_set_bio(serverssl, NULL, NULL);
    SSL_set_bio(clientssl, NULL, NULL);

    if (!TEST_true(SSL_set_app_data(clientssl, &clientquicdata))
        || !TEST_true(SSL_set_app_data(serverssl, &serverquicdata)))
        goto end;

    if (!TEST_true(SSL_set_quic_tls_cbs(clientssl, qtdis, &cdata))
            || !TEST_true(SSL_set_quic_tls_cbs(serverssl, qtdis, &sdata))
            || !TEST_true(SSL_set_quic_tls_transport_params(clientssl, cparams,
                                                            sizeof(cparams)))
            || !TEST_true(SSL_set_quic_tls_transport_params(serverssl, sparams,
                                                            sizeof(sparams))))
        goto end;

    /*
     * Reset our secret history so we get the record of the second connection
     */
    memset(secret_history, 0, sizeof(secret_history));
    secret_history_idx = 0;

    SSL_set_quic_tls_early_data_enabled(serverssl, 1);
    SSL_set_quic_tls_early_data_enabled(clientssl, 1);

    SSL_set_msg_callback(serverssl, assert_no_end_of_early_data);
    SSL_set_msg_callback(clientssl, assert_no_end_of_early_data);

    if (!TEST_int_eq(SSL_connect(clientssl), -1)
            || !TEST_int_eq(SSL_accept(serverssl), -1)
            || !TEST_int_eq(SSL_get_early_data_status(serverssl), SSL_EARLY_DATA_ACCEPTED)
            || !TEST_int_eq(SSL_get_error(clientssl, 0), SSL_ERROR_WANT_READ)
            || !TEST_int_eq(SSL_get_error(serverssl, 0), SSL_ERROR_WANT_READ))
        goto end;

    /* Check the encryption levels are what we expect them to be */
    if (!TEST_true(sdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_HANDSHAKE)
            || !TEST_true(sdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(cdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_NONE)
            || !TEST_true(cdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_EARLY))
        goto end;

    sdata.sm_count = 0;
    cdata.sm_count = 0;
    if (!TEST_true(create_ssl_connection_ex(serverssl, clientssl, SSL_ERROR_NONE,
                                            &cdata.sm_count, &sdata.sm_count)))
        goto end;

    /* Check no problems during the handshake */
    if (!TEST_false(sdata.alert)
            || !TEST_false(cdata.alert)
            || !TEST_false(sdata.err)
            || !TEST_false(cdata.err))
        goto end;

    /* Check the secrets all match */
    for (i = OSSL_RECORD_PROTECTION_LEVEL_EARLY - 1;
         i < OSSL_RECORD_PROTECTION_LEVEL_APPLICATION;
         i++) {
        if (!TEST_mem_eq(sdata.wsecret[i], sdata.wsecret_len[i],
                         cdata.rsecret[i], cdata.rsecret_len[i]))
            goto end;
    }

    if (!TEST_int_eq(check_secret_history(serverssl), 1))
        goto end;
    if (!TEST_int_eq(check_secret_history(clientssl), 1))
        goto end;

    /* Check the transport params */
    if (!TEST_mem_eq(sdata.params, sdata.params_len, cparams, sizeof(cparams))
            || !TEST_mem_eq(cdata.params, cdata.params_len, sparams,
                            sizeof(sparams)))
        goto end;

    /* Check the encryption levels are what we expect them to be */
    if (!TEST_true(sdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(sdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(cdata.renc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION)
            || !TEST_true(cdata.wenc_level == OSSL_RECORD_PROTECTION_LEVEL_APPLICATION))
        goto end;

    /* Check there is no EndOfEearlyData in handshake */
    if (!TEST_int_eq(end_of_early_data, 0))
        goto end;

    testresult = 1;
 end:
    SSL_SESSION_free(sess);
    SSL_SESSION_free(clientpsk);
    SSL_SESSION_free(serverpsk);
    clientpsk = serverpsk = NULL;
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}
#endif /* !defined(OSSL_NO_USABLE_TLS1_3) */

static int test_no_renegotiation(int idx)
{
    SSL_CTX *sctx = NULL, *cctx = NULL;
    SSL *serverssl = NULL, *clientssl = NULL;
    int testresult = 0, ret;
    int max_proto;
    const SSL_METHOD *sm, *cm;
    unsigned char buf[5];

    if (idx == 0) {
#ifndef OPENSSL_NO_TLS1_2
        max_proto = TLS1_2_VERSION;
        sm = TLS_server_method();
        cm = TLS_client_method();
#else
        return TEST_skip("TLSv1.2 is disabled in this build");
#endif
    } else {
#ifndef OPENSSL_NO_DTLS1_2
        max_proto = DTLS1_2_VERSION;
        sm = DTLS_server_method();
        cm = DTLS_client_method();
#else
        return TEST_skip("DTLSv1.2 is disabled in this build");
#endif
    }
    if (!TEST_true(create_ssl_ctx_pair(libctx, sm, cm, 0, max_proto,
                                       &sctx, &cctx, cert, privkey)))
        goto end;

    SSL_CTX_set_options(sctx, SSL_OP_NO_RENEGOTIATION);

    if (!TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl, NULL,
                                      NULL)))
        goto end;

    if (!TEST_true(create_ssl_connection(serverssl, clientssl, SSL_ERROR_NONE)))
        goto end;

    if (!TEST_true(SSL_renegotiate(clientssl))
            || !TEST_int_le(ret = SSL_connect(clientssl), 0)
            || !TEST_int_eq(SSL_get_error(clientssl, ret), SSL_ERROR_WANT_READ))
        goto end;

    /*
     * We've not sent any application data, so we expect this to fail. It should
     * also read the renegotiation attempt, and send back a no_renegotiation
     * warning alert because we have renegotiation disabled.
     */
    if (!TEST_int_le(ret = SSL_read(serverssl, buf, sizeof(buf)), 0))
        goto end;
    if (!TEST_int_eq(SSL_get_error(serverssl, ret), SSL_ERROR_WANT_READ))
        goto end;

    /*
     * The client should now see the no_renegotiation warning and fail the
     * connection
     */
    if (!TEST_int_le(ret = SSL_connect(clientssl), 0)
            || !TEST_int_eq(SSL_get_error(clientssl, ret), SSL_ERROR_SSL)
            || !TEST_int_eq(ERR_GET_REASON(ERR_get_error()), SSL_R_NO_RENEGOTIATION))
        goto end;

    testresult = 1;
 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

OPT_TEST_DECLARE_USAGE("certfile privkeyfile srpvfile tmpfile provider config dhfile\n")

int setup_tests(void)
{
    char *modulename;
    char *configfile;

    libctx = OSSL_LIB_CTX_new();
    if (!TEST_ptr(libctx))
        return 0;

    defctxnull = OSSL_PROVIDER_load(NULL, "null");

    /*
     * Verify that the default and fips providers in the default libctx are not
     * available
     */
    if (!TEST_false(OSSL_PROVIDER_available(NULL, "default"))
            || !TEST_false(OSSL_PROVIDER_available(NULL, "fips")))
        return 0;

    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(certsdir = test_get_argument(0))
            || !TEST_ptr(srpvfile = test_get_argument(1))
            || !TEST_ptr(tmpfilename = test_get_argument(2))
            || !TEST_ptr(modulename = test_get_argument(3))
            || !TEST_ptr(configfile = test_get_argument(4))
            || !TEST_ptr(dhfile = test_get_argument(5)))
        return 0;

    if (!TEST_true(OSSL_LIB_CTX_load_config(libctx, configfile)))
        return 0;

    /* Check we have the expected provider available */
    if (!TEST_true(OSSL_PROVIDER_available(libctx, modulename)))
        return 0;

    /* Check the default provider is not available */
    if (strcmp(modulename, "default") != 0
            && !TEST_false(OSSL_PROVIDER_available(libctx, "default")))
        return 0;

    if (strcmp(modulename, "fips") == 0) {
        OSSL_PROVIDER *prov = NULL;
        OSSL_PARAM params[2];

        is_fips = 1;

        prov = OSSL_PROVIDER_load(libctx, "fips");
        if (prov != NULL) {
            /* Query the fips provider to check if the check ems option is enabled */
            params[0] =
                OSSL_PARAM_construct_int(OSSL_PROV_PARAM_TLS1_PRF_EMS_CHECK,
                                         &fips_ems_check);
            params[1] = OSSL_PARAM_construct_end();
            OSSL_PROVIDER_get_params(prov, params);
            OSSL_PROVIDER_unload(prov);
        }
    }

    /*
     * We add, but don't load the test "tls-provider". We'll load it when we
     * need it.
     */
    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, "tls-provider",
                                             tls_provider_init)))
        return 0;


    if (getenv("OPENSSL_TEST_GETCOUNTS") != NULL) {
#ifdef OPENSSL_NO_CRYPTO_MDEBUG
        TEST_error("not supported in this build");
        return 0;
#else
        int i, mcount, rcount, fcount;

        for (i = 0; i < 4; i++)
            test_export_key_mat(i);
        CRYPTO_get_alloc_counts(&mcount, &rcount, &fcount);
        test_printf_stdout("malloc %d realloc %d free %d\n",
                mcount, rcount, fcount);
        return 1;
#endif
    }

    cert = test_mk_file_path(certsdir, "servercert.pem");
    if (cert == NULL)
        goto err;

    privkey = test_mk_file_path(certsdir, "serverkey.pem");
    if (privkey == NULL)
        goto err;

    cert2 = test_mk_file_path(certsdir, "server-ecdsa-cert.pem");
    if (cert2 == NULL)
        goto err;

    privkey2 = test_mk_file_path(certsdir, "server-ecdsa-key.pem");
    if (privkey2 == NULL)
        goto err;

    cert1024 = test_mk_file_path(certsdir, "ee-cert-1024.pem");
    if (cert1024 == NULL)
        goto err;

    privkey1024 = test_mk_file_path(certsdir, "ee-key-1024.pem");
    if (privkey1024 == NULL)
        goto err;

    cert3072 = test_mk_file_path(certsdir, "ee-cert-3072.pem");
    if (cert3072 == NULL)
        goto err;

    privkey3072 = test_mk_file_path(certsdir, "ee-key-3072.pem");
    if (privkey3072 == NULL)
        goto err;

    cert4096 = test_mk_file_path(certsdir, "ee-cert-4096.pem");
    if (cert4096 == NULL)
        goto err;

    privkey4096 = test_mk_file_path(certsdir, "ee-key-4096.pem");
    if (privkey4096 == NULL)
        goto err;

    cert8192 = test_mk_file_path(certsdir, "ee-cert-8192.pem");
    if (cert8192 == NULL)
        goto err;

    privkey8192 = test_mk_file_path(certsdir, "ee-key-8192.pem");
    if (privkey8192 == NULL)
        goto err;

    if (fips_ems_check) {
#ifndef OPENSSL_NO_TLS1_2
        ADD_TEST(test_no_ems);
#endif
        return 1;
    }
#if !defined(OPENSSL_NO_KTLS) && !defined(OPENSSL_NO_SOCK)
# if !defined(OPENSSL_NO_TLS1_2) || !defined(OSSL_NO_USABLE_TLS1_3)
    ADD_ALL_TESTS(test_ktls, NUM_KTLS_TEST_CIPHERS * 4);
    ADD_ALL_TESTS(test_ktls_sendfile, NUM_KTLS_TEST_CIPHERS * 2);
# endif
#endif
    ADD_TEST(test_large_message_tls);
    ADD_TEST(test_large_message_tls_read_ahead);
#ifndef OPENSSL_NO_DTLS
    ADD_TEST(test_large_message_dtls);
#endif
    ADD_ALL_TESTS(test_large_app_data, 28);
    ADD_TEST(test_cleanse_plaintext);
#ifndef OPENSSL_NO_OCSP
    ADD_TEST(test_tlsext_status_type);
#endif
    ADD_TEST(test_session_with_only_int_cache);
    ADD_TEST(test_session_with_only_ext_cache);
    ADD_TEST(test_session_with_both_cache);
    ADD_TEST(test_session_wo_ca_names);
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_ALL_TESTS(test_stateful_tickets, 3);
    ADD_ALL_TESTS(test_stateless_tickets, 3);
    ADD_TEST(test_psk_tickets);
    ADD_ALL_TESTS(test_extra_tickets, 6);
#endif
    ADD_ALL_TESTS(test_ssl_set_bio, TOTAL_SSL_SET_BIO_TESTS);
    ADD_TEST(test_ssl_bio_pop_next_bio);
    ADD_TEST(test_ssl_bio_pop_ssl_bio);
    ADD_TEST(test_ssl_bio_change_rbio);
    ADD_TEST(test_ssl_bio_change_wbio);
#if !defined(OPENSSL_NO_TLS1_2) || defined(OSSL_NO_USABLE_TLS1_3)
    ADD_ALL_TESTS(test_set_sigalgs, OSSL_NELEM(testsigalgs) * 2);
    ADD_TEST(test_keylog);
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_TEST(test_keylog_no_master_key);
#endif
    ADD_TEST(test_client_cert_verify_cb);
    ADD_TEST(test_ssl_build_cert_chain);
    ADD_TEST(test_ssl_ctx_build_cert_chain);
#ifndef OPENSSL_NO_TLS1_2
    ADD_TEST(test_client_hello_cb);
    ADD_TEST(test_no_ems);
    ADD_TEST(test_ccs_change_cipher);
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_ALL_TESTS(test_early_data_read_write, 6);
    /*
     * We don't do replay tests for external PSK. Replay protection isn't used
     * in that scenario.
     */
    ADD_ALL_TESTS(test_early_data_replay, 2);
    ADD_ALL_TESTS(test_early_data_skip, OSSL_NELEM(ciphersuites) * 3);
    ADD_ALL_TESTS(test_early_data_skip_hrr, OSSL_NELEM(ciphersuites) * 3);
    ADD_ALL_TESTS(test_early_data_skip_hrr_fail, OSSL_NELEM(ciphersuites) * 3);
    ADD_ALL_TESTS(test_early_data_skip_abort, OSSL_NELEM(ciphersuites) * 3);
    ADD_ALL_TESTS(test_early_data_not_sent, 3);
    ADD_ALL_TESTS(test_early_data_psk, 8);
    ADD_ALL_TESTS(test_early_data_psk_with_all_ciphers, 7);
    ADD_ALL_TESTS(test_early_data_not_expected, 3);
# ifndef OPENSSL_NO_TLS1_2
    ADD_ALL_TESTS(test_early_data_tls1_2, 3);
# endif
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_ALL_TESTS(test_set_ciphersuite, 10);
    ADD_TEST(test_ciphersuite_change);
    ADD_ALL_TESTS(test_tls13_ciphersuite, 4);
# ifdef OPENSSL_NO_PSK
    ADD_ALL_TESTS(test_tls13_psk, 1);
# else
    ADD_ALL_TESTS(test_tls13_psk, 4);
# endif  /* OPENSSL_NO_PSK */
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_ALL_TESTS(test_tls13_no_dhe_kex, 8);
#endif /* OSSL_NO_USABLE_TLS1_3 */
# ifndef OPENSSL_NO_TLS1_2
    /* Test with both TLSv1.3 and 1.2 versions */
    ADD_ALL_TESTS(test_key_exchange, 21);
#  if !defined(OPENSSL_NO_EC) && !defined(OPENSSL_NO_DH)
    ADD_ALL_TESTS(test_negotiated_group,
                  4 * (OSSL_NELEM(ecdhe_kexch_groups)
                       + OSSL_NELEM(ffdhe_kexch_groups)));
#  endif
# else
    /* Test with only TLSv1.3 versions */
    ADD_ALL_TESTS(test_key_exchange, 18);
# endif
    ADD_ALL_TESTS(test_custom_exts, 6);
    ADD_TEST(test_stateless);
    ADD_TEST(test_pha_key_update);
#else
    ADD_ALL_TESTS(test_custom_exts, 3);
#endif
    ADD_ALL_TESTS(test_export_key_mat, 6);
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_ALL_TESTS(test_export_key_mat_early, 3);
    ADD_TEST(test_key_update);
    ADD_ALL_TESTS(test_key_update_peer_in_write, 2);
    ADD_ALL_TESTS(test_key_update_peer_in_read, 2);
    ADD_ALL_TESTS(test_key_update_local_in_write, 2);
    ADD_ALL_TESTS(test_key_update_local_in_read, 2);
#endif
    ADD_ALL_TESTS(test_ssl_clear, 8);
    ADD_ALL_TESTS(test_max_fragment_len_ext, OSSL_NELEM(max_fragment_len_test));
#if !defined(OPENSSL_NO_SRP) && !defined(OPENSSL_NO_TLS1_2)
    ADD_ALL_TESTS(test_srp, 6);
#endif
#if !defined(OPENSSL_NO_COMP_ALG)
    /* Add compression case */
    ADD_ALL_TESTS(test_info_callback, 8);
#else
    ADD_ALL_TESTS(test_info_callback, 6);
#endif
    ADD_ALL_TESTS(test_ssl_pending, 2);
    ADD_ALL_TESTS(test_ssl_get_shared_ciphers, OSSL_NELEM(shared_ciphers_data));
    ADD_ALL_TESTS(test_ticket_callbacks, 20);
    ADD_ALL_TESTS(test_shutdown, 7);
    ADD_TEST(test_async_shutdown);
    ADD_ALL_TESTS(test_incorrect_shutdown, 2);
    ADD_ALL_TESTS(test_cert_cb, 6);
    ADD_ALL_TESTS(test_client_cert_cb, 2);
    ADD_ALL_TESTS(test_ca_names, 3);
#ifndef OPENSSL_NO_TLS1_2
    ADD_ALL_TESTS(test_multiblock_write, OSSL_NELEM(multiblock_cipherlist_data));
#endif
    ADD_ALL_TESTS(test_servername, 10);
    ADD_TEST(test_unknown_sigalgs_groups);
#if (!defined(OPENSSL_NO_EC) || !defined(OPENSSL_NO_DH)) || !defined(OPENSSL_NO_ML_KEM)
    ADD_TEST(test_configuration_of_groups);
#endif
#if !defined(OPENSSL_NO_EC) \
    && (!defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2))
    ADD_ALL_TESTS(test_sigalgs_available, 6);
#endif
#ifndef OPENSSL_NO_TLS1_3
    ADD_ALL_TESTS(test_pluggable_group, 2);
    ADD_ALL_TESTS(test_pluggable_signature, 6);
#endif
#ifndef OPENSSL_NO_TLS1_2
    ADD_TEST(test_ssl_dup);
    ADD_TEST(test_session_secret_cb);
# ifndef OPENSSL_NO_DH
    ADD_ALL_TESTS(test_set_tmp_dh, 11);
    ADD_ALL_TESTS(test_dh_auto, 7);
# endif
#endif
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_TEST(test_sni_tls13);
    ADD_ALL_TESTS(test_ticket_lifetime, 2);
#endif
    ADD_TEST(test_inherit_verify_param);
    ADD_TEST(test_set_alpn);
    ADD_TEST(test_set_verify_cert_store_ssl_ctx);
    ADD_TEST(test_set_verify_cert_store_ssl);
    ADD_ALL_TESTS(test_session_timeout, 1);
#if !defined(OSSL_NO_USABLE_TLS1_3) || !defined(OPENSSL_NO_TLS1_2)
    ADD_ALL_TESTS(test_session_cache_overflow, 4);
#endif
    ADD_TEST(test_load_dhfile);
#ifndef OSSL_NO_USABLE_TLS1_3
    ADD_TEST(test_read_ahead_key_change);
    ADD_ALL_TESTS(test_tls13_record_padding, 6);
#endif
#if !defined(OPENSSL_NO_TLS1_2) && !defined(OSSL_NO_USABLE_TLS1_3)
    ADD_ALL_TESTS(test_serverinfo_custom, 4);
#endif
#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DYNAMIC_ENGINE)
    ADD_ALL_TESTS(test_pipelining, 7);
#endif
    ADD_ALL_TESTS(test_version, 6);
    ADD_TEST(test_rstate_string);
    ADD_ALL_TESTS(test_handshake_retry, 16);
    ADD_TEST(test_data_retry);
    ADD_ALL_TESTS(test_multi_resume, 5);
    ADD_ALL_TESTS(test_select_next_proto, OSSL_NELEM(next_proto_tests));
#if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_NEXTPROTONEG)
    ADD_ALL_TESTS(test_npn, 5);
#endif
    ADD_ALL_TESTS(test_alpn, 4);
#if !defined(OSSL_NO_USABLE_TLS1_3)
    ADD_ALL_TESTS(test_quic_tls, 6);
    ADD_TEST(test_quic_tls_early_data);
#endif
    ADD_ALL_TESTS(test_no_renegotiation, 2);
    return 1;

 err:
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    OPENSSL_free(cert2);
    OPENSSL_free(privkey2);
    return 0;
}

void cleanup_tests(void)
{
# if !defined(OPENSSL_NO_TLS1_2) && !defined(OPENSSL_NO_DH)
    EVP_PKEY_free(tmp_dh_params);
#endif
    OPENSSL_free(cert);
    OPENSSL_free(privkey);
    OPENSSL_free(cert2);
    OPENSSL_free(privkey2);
    OPENSSL_free(cert1024);
    OPENSSL_free(privkey1024);
    OPENSSL_free(cert3072);
    OPENSSL_free(privkey3072);
    OPENSSL_free(cert4096);
    OPENSSL_free(privkey4096);
    OPENSSL_free(cert8192);
    OPENSSL_free(privkey8192);
    bio_s_mempacket_test_free();
    bio_s_always_retry_free();
    bio_s_maybe_retry_free();
    OSSL_PROVIDER_unload(defctxnull);
    OSSL_LIB_CTX_free(libctx);
}
