/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include "internal/ssl_unwrap.h"
#include "internal/quic_tls.h"
#include "../ssl_local.h"

static int crypto_send_cb(const unsigned char *buf, size_t buf_len,
                          size_t *consumed, void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.crypto_send_cb(s, buf, buf_len, consumed, sc->qtarg);
}

static int crypto_recv_rcd_cb(const unsigned char **buf, size_t *bytes_read,
                              void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.crypto_recv_rcd_cb(s, buf, bytes_read, sc->qtarg);
}

static int crypto_release_rcd_cb(size_t bytes_read, void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.crypto_release_rcd_cb(s, bytes_read, sc->qtarg);
}
static int yield_secret_cb(uint32_t prot_level, int direction,
                           uint32_t suite_id, EVP_MD *md,
                           const unsigned char *secret, size_t secret_len,
                           void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.yield_secret_cb(s, prot_level, direction,
                                    secret, secret_len, sc->qtarg);
}

static int got_transport_params_cb(const unsigned char *params,
                                   size_t params_len,
                                   void *arg)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.got_transport_params_cb(s, params, params_len, sc->qtarg);
}

static int alert_cb(void *arg, unsigned char alert_code)
{
    SSL *s = (SSL *)arg;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;
    return sc->qtcb.alert_cb(s, alert_code, sc->qtarg);
}

static int tls_callbacks_from_dispatch(OSSL_QUIC_TLS_CALLBACKS *qtcb,
                                       const OSSL_DISPATCH *qtdis)
{
    for (; qtdis->function_id != 0; qtdis++) {
        switch (qtdis->function_id) {
        case OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_SEND:
            if (qtcb->crypto_send_cb == NULL)
                qtcb->crypto_send_cb = OSSL_FUNC_SSL_QUIC_TLS_crypto_send(qtdis);
            break;
        case OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RECV_RCD:
            if (qtcb->crypto_recv_rcd_cb == NULL)
                qtcb->crypto_recv_rcd_cb =
                    OSSL_FUNC_SSL_QUIC_TLS_crypto_recv_rcd(qtdis);
            break;
        case OSSL_FUNC_SSL_QUIC_TLS_CRYPTO_RELEASE_RCD:
            if (qtcb->crypto_release_rcd_cb == NULL)
                qtcb->crypto_release_rcd_cb =
                    OSSL_FUNC_SSL_QUIC_TLS_crypto_release_rcd(qtdis);
            break;
        case OSSL_FUNC_SSL_QUIC_TLS_YIELD_SECRET:
            if (qtcb->yield_secret_cb == NULL)
                qtcb->yield_secret_cb =
                    OSSL_FUNC_SSL_QUIC_TLS_yield_secret(qtdis);
            break;
        case OSSL_FUNC_SSL_QUIC_TLS_GOT_TRANSPORT_PARAMS:
            if (qtcb->got_transport_params_cb == NULL)
                qtcb->got_transport_params_cb =
                    OSSL_FUNC_SSL_QUIC_TLS_got_transport_params(qtdis);
            break;
        case OSSL_FUNC_SSL_QUIC_TLS_ALERT:
            if (qtcb->alert_cb == NULL)
                qtcb->alert_cb =
                    OSSL_FUNC_SSL_QUIC_TLS_alert(qtdis);
            break;
        }
    }

    if (qtcb->crypto_send_cb == NULL
            || qtcb->crypto_recv_rcd_cb == NULL
            || qtcb->crypto_release_rcd_cb == NULL
            || qtcb->yield_secret_cb == NULL
            || qtcb->got_transport_params_cb == NULL
            || qtcb->alert_cb == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_MISSING_QUIC_TLS_FUNCTIONS);
        return 0;
    }

    return 1;
}

int SSL_set_quic_tls_cbs(SSL *s, const OSSL_DISPATCH *qtdis, void *arg)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    QUIC_TLS_ARGS qtlsargs;

    if (!SSL_is_tls(s)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (!tls_callbacks_from_dispatch(&sc->qtcb, qtdis))
        /* ERR_raise already called */
        return 0;

    sc->qtarg = arg;

    ossl_quic_tls_free(sc->qtls);
    qtlsargs.s = s;
    qtlsargs.crypto_send_cb = crypto_send_cb;
    qtlsargs.crypto_send_cb_arg = s;
    qtlsargs.crypto_recv_rcd_cb = crypto_recv_rcd_cb;
    qtlsargs.crypto_recv_rcd_cb_arg = s;
    qtlsargs.crypto_release_rcd_cb = crypto_release_rcd_cb;
    qtlsargs.crypto_release_rcd_cb_arg = s;
    qtlsargs.yield_secret_cb = yield_secret_cb;
    qtlsargs.yield_secret_cb_arg = s;
    qtlsargs.got_transport_params_cb = got_transport_params_cb;
    qtlsargs.got_transport_params_cb_arg = s;
    qtlsargs.handshake_complete_cb = NULL;
    qtlsargs.handshake_complete_cb_arg = NULL;
    qtlsargs.alert_cb = alert_cb;
    qtlsargs.alert_cb_arg = s;
    qtlsargs.is_server = sc->server;
    qtlsargs.ossl_quic = 0;
    sc->qtls = ossl_quic_tls_new(&qtlsargs);
    if (sc->qtls == NULL)
        return 0;

    if (!ossl_quic_tls_configure(sc->qtls))
        return 0;

    return 1;
}

int SSL_set_quic_tls_transport_params(SSL *s,
                                      const unsigned char *params,
                                      size_t params_len)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    if (sc->qtls == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return ossl_quic_tls_set_transport_params(sc->qtls, params, params_len);
}

int SSL_set_quic_tls_early_data_enabled(SSL *s, int enabled)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (!SSL_is_tls(s)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    if (sc->qtls == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }

    return ossl_quic_tls_set_early_data_enabled(sc->qtls, enabled);
}
