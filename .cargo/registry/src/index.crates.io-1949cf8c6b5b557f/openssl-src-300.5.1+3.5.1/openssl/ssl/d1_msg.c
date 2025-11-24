/*
 * Copyright 2005-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "ssl_local.h"
#include "internal/ssl_unwrap.h"

int dtls1_write_app_data_bytes(SSL *s, uint8_t type, const void *buf_,
                               size_t len, size_t *written)
{
    int i;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return -1;

    if (SSL_in_init(s) && !ossl_statem_get_in_handshake(sc)) {
        i = sc->handshake_func(s);
        if (i < 0)
            return i;
        if (i == 0) {
            ERR_raise(ERR_LIB_SSL, SSL_R_SSL_HANDSHAKE_FAILURE);
            return -1;
        }
    }

    if (len > SSL3_RT_MAX_PLAIN_LENGTH) {
        ERR_raise(ERR_LIB_SSL, SSL_R_DTLS_MESSAGE_TOO_BIG);
        return -1;
    }

    return dtls1_write_bytes(sc, type, buf_, len, written);
}

int dtls1_dispatch_alert(SSL *ssl)
{
    int i, j;
    void (*cb) (const SSL *ssl, int type, int val) = NULL;
    unsigned char buf[DTLS1_AL_HEADER_LENGTH];
    unsigned char *ptr = &buf[0];
    size_t written;
    SSL_CONNECTION *s = SSL_CONNECTION_FROM_SSL_ONLY(ssl);

    if (s == NULL)
        return 0;

    s->s3.alert_dispatch = SSL_ALERT_DISPATCH_NONE;

    memset(buf, 0, sizeof(buf));
    *ptr++ = s->s3.send_alert[0];
    *ptr++ = s->s3.send_alert[1];

    i = do_dtls1_write(s, SSL3_RT_ALERT, &buf[0], sizeof(buf), &written);
    if (i <= 0) {
        s->s3.alert_dispatch = 1;
        /* fprintf(stderr, "not done with alert\n"); */
    } else {
        (void)BIO_flush(s->wbio);

        if (s->msg_callback)
            s->msg_callback(1, s->version, SSL3_RT_ALERT, s->s3.send_alert,
                            2, ssl, s->msg_callback_arg);

        if (s->info_callback != NULL)
            cb = s->info_callback;
        else if (ssl->ctx->info_callback != NULL)
            cb = ssl->ctx->info_callback;

        if (cb != NULL) {
            j = (s->s3.send_alert[0] << 8) | s->s3.send_alert[1];
            cb(ssl, SSL_CB_WRITE_ALERT, j);
        }
    }
    return i;
}
