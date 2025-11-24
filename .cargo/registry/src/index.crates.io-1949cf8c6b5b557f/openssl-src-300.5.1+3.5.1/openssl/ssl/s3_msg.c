/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "ssl_local.h"
#include "internal/ssl_unwrap.h"

int ssl3_do_change_cipher_spec(SSL_CONNECTION *s)
{
    int i;
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    if (s->server)
        i = SSL3_CHANGE_CIPHER_SERVER_READ;
    else
        i = SSL3_CHANGE_CIPHER_CLIENT_READ;

    if (s->s3.tmp.key_block == NULL) {
        if (s->session == NULL || s->session->master_key_length == 0) {
            /* might happen if dtls1_read_bytes() calls this */
            ERR_raise(ERR_LIB_SSL, SSL_R_CCS_RECEIVED_EARLY);
            return 0;
        }

        s->session->cipher = s->s3.tmp.new_cipher;
        if (!ssl->method->ssl3_enc->setup_key_block(s)) {
            /* SSLfatal() already called */
            return 0;
        }
    }

    if (!ssl->method->ssl3_enc->change_cipher_state(s, i)) {
        /* SSLfatal() already called */
        return 0;
    }

    return 1;
}

int ssl3_send_alert(SSL_CONNECTION *s, int level, int desc)
{
    SSL *ssl = SSL_CONNECTION_GET_SSL(s);

    /* Map tls/ssl alert value to correct one */
    if (SSL_CONNECTION_TREAT_AS_TLS13(s))
        desc = tls13_alert_code(desc);
    else
        desc = ssl->method->ssl3_enc->alert_value(desc);
    if (s->version == SSL3_VERSION && desc == SSL_AD_PROTOCOL_VERSION)
        desc = SSL_AD_HANDSHAKE_FAILURE; /* SSL 3.0 does not have
                                          * protocol_version alerts */
    if (desc < 0)
        return -1;
    if (s->shutdown & SSL_SENT_SHUTDOWN && desc != SSL_AD_CLOSE_NOTIFY)
        return -1;
    /* If a fatal one, remove from cache */
    if ((level == SSL3_AL_FATAL) && (s->session != NULL))
        SSL_CTX_remove_session(s->session_ctx, s->session);

    s->s3.alert_dispatch = SSL_ALERT_DISPATCH_PENDING;
    s->s3.send_alert[0] = level;
    s->s3.send_alert[1] = desc;
    if (!RECORD_LAYER_write_pending(&s->rlayer)) {
        /* data still being written out? */
        return ssl->method->ssl_dispatch_alert(ssl);
    }
    /*
     * else data is still being written out, we will get written some time in
     * the future
     */
    return -1;
}

int ssl3_dispatch_alert(SSL *s)
{
    int i, j;
    void (*cb) (const SSL *ssl, int type, int val) = NULL;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    OSSL_RECORD_TEMPLATE templ;

    if (sc == NULL)
        return -1;

    if (sc->rlayer.wrlmethod == NULL) {
        /* No write record layer so we can't sent and alert. We just ignore it */
        sc->s3.alert_dispatch = SSL_ALERT_DISPATCH_NONE;
        return 1;
    }

    templ.type = SSL3_RT_ALERT;
    templ.version = (sc->version == TLS1_3_VERSION) ? TLS1_2_VERSION
                                                    : sc->version;
    if (SSL_get_state(s) == TLS_ST_CW_CLNT_HELLO
            && !sc->renegotiate
            && TLS1_get_version(s) > TLS1_VERSION
            && sc->hello_retry_request == SSL_HRR_NONE) {
        templ.version = TLS1_VERSION;
    }
    templ.buf = &sc->s3.send_alert[0];
    templ.buflen = 2;

    if (RECORD_LAYER_write_pending(&sc->rlayer)) {
        if (sc->s3.alert_dispatch != SSL_ALERT_DISPATCH_RETRY) {
            /*
             * We have a write pending but it wasn't from a previous call to
             * this function! Can we ever get here? Maybe via API misuse??
             * Give up.
             */
            sc->s3.alert_dispatch = SSL_ALERT_DISPATCH_NONE;
            return -1;
        }
        /* Retry what we've already got pending */
        i = HANDLE_RLAYER_WRITE_RETURN(sc,
                sc->rlayer.wrlmethod->retry_write_records(sc->rlayer.wrl));
        if (i <= 0) {
            /* Could be NBIO. Keep alert_dispatch as SSL_ALERT_DISPATCH_RETRY */
            return -1;
        }
        sc->rlayer.wpend_tot = 0;
        sc->s3.alert_dispatch = SSL_ALERT_DISPATCH_NONE;
        return 1;
    }

    i = HANDLE_RLAYER_WRITE_RETURN(sc,
            sc->rlayer.wrlmethod->write_records(sc->rlayer.wrl, &templ, 1));

    if (i <= 0) {
        sc->s3.alert_dispatch = SSL_ALERT_DISPATCH_RETRY;
        sc->rlayer.wpend_tot = templ.buflen;
        sc->rlayer.wpend_type = templ.type;
        sc->rlayer.wpend_buf = templ.buf;
    } else {
        /*
         * Alert sent to BIO - now flush. If the message does not get sent due
         * to non-blocking IO, we will not worry too much.
         */
        (void)BIO_flush(sc->wbio);
        sc->s3.alert_dispatch = SSL_ALERT_DISPATCH_NONE;

        if (sc->msg_callback)
            sc->msg_callback(1, sc->version, SSL3_RT_ALERT, sc->s3.send_alert,
                             2, s, sc->msg_callback_arg);

        if (sc->info_callback != NULL)
            cb = sc->info_callback;
        else if (s->ctx->info_callback != NULL)
            cb = s->ctx->info_callback;

        if (cb != NULL) {
            j = (sc->s3.send_alert[0] << 8) | sc->s3.send_alert[1];
            cb(s, SSL_CB_WRITE_ALERT, j);
        }
    }
    return i;
}
