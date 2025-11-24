/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * SRP is deprecated and there is no replacement. When SRP is removed,
 * the code in this file can be removed too. Until then we have to use
 * the deprecated APIs.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/srp.h>
#include <openssl/ssl.h>
#include "handshake.h"
#include "../testutil.h"

static char *client_srp_cb(SSL *s, void *arg)
{
    CTX_DATA *ctx_data = (CTX_DATA*)(arg);
    return OPENSSL_strdup(ctx_data->srp_password);
}

static int server_srp_cb(SSL *s, int *ad, void *arg)
{
    CTX_DATA *ctx_data = (CTX_DATA*)(arg);
    if (strcmp(ctx_data->srp_user, SSL_get_srp_username(s)) != 0)
        return SSL3_AL_FATAL;
    if (SSL_set_srp_server_param_pw(s, ctx_data->srp_user,
                                    ctx_data->srp_password,
                                    "2048" /* known group */) < 0) {
        *ad = SSL_AD_INTERNAL_ERROR;
        return SSL3_AL_FATAL;
    }
    return SSL_ERROR_NONE;
}

int configure_handshake_ctx_for_srp(SSL_CTX *server_ctx, SSL_CTX *server2_ctx,
                                    SSL_CTX *client_ctx,
                                    const SSL_TEST_EXTRA_CONF *extra,
                                    CTX_DATA *server_ctx_data,
                                    CTX_DATA *server2_ctx_data,
                                    CTX_DATA *client_ctx_data)
{
    if (extra->server.srp_user != NULL) {
        SSL_CTX_set_srp_username_callback(server_ctx, server_srp_cb);
        server_ctx_data->srp_user = OPENSSL_strdup(extra->server.srp_user);
        server_ctx_data->srp_password = OPENSSL_strdup(extra->server.srp_password);
        if (server_ctx_data->srp_user == NULL || server_ctx_data->srp_password == NULL) {
            OPENSSL_free(server_ctx_data->srp_user);
            OPENSSL_free(server_ctx_data->srp_password);
            server_ctx_data->srp_user = NULL;
            server_ctx_data->srp_password = NULL;
            return 0;
        }
        SSL_CTX_set_srp_cb_arg(server_ctx, server_ctx_data);
    }
    if (extra->server2.srp_user != NULL) {
        if (!TEST_ptr(server2_ctx))
            return 0;
        SSL_CTX_set_srp_username_callback(server2_ctx, server_srp_cb);
        server2_ctx_data->srp_user = OPENSSL_strdup(extra->server2.srp_user);
        server2_ctx_data->srp_password = OPENSSL_strdup(extra->server2.srp_password);
        if (server2_ctx_data->srp_user == NULL || server2_ctx_data->srp_password == NULL) {
            OPENSSL_free(server2_ctx_data->srp_user);
            OPENSSL_free(server2_ctx_data->srp_password);
            server2_ctx_data->srp_user = NULL;
            server2_ctx_data->srp_password = NULL;
            return 0;
        }
        SSL_CTX_set_srp_cb_arg(server2_ctx, server2_ctx_data);
    }
    if (extra->client.srp_user != NULL) {
        if (!TEST_true(SSL_CTX_set_srp_username(client_ctx,
                                                extra->client.srp_user)))
            return 0;
        SSL_CTX_set_srp_client_pwd_callback(client_ctx, client_srp_cb);
        client_ctx_data->srp_password = OPENSSL_strdup(extra->client.srp_password);
        if (client_ctx_data->srp_password == NULL)
            return 0;
        SSL_CTX_set_srp_cb_arg(client_ctx, client_ctx_data);
    }
    return 1;
}
