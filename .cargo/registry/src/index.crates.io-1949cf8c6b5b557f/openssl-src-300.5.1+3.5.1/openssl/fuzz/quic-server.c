/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/bio.h>
#include "fuzzer.h"
#include "internal/sockets.h"
#include "internal/time.h"
#include "internal/quic_ssl.h"

/* unused, to avoid warning. */
static int idx;

static OSSL_TIME fake_now;

static OSSL_TIME fake_now_cb(void *arg)
{
    return fake_now;
}

int FuzzerInitialize(int *argc, char ***argv)
{
    STACK_OF(SSL_COMP) *comp_methods;

    FuzzerSetRand();
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS | OPENSSL_INIT_ASYNC, NULL);
    OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    idx = SSL_get_ex_data_X509_STORE_CTX_idx();
    comp_methods = SSL_COMP_get_compression_methods();
    if (comp_methods != NULL)
        sk_SSL_COMP_sort(comp_methods);

    return 1;
}

#define HANDSHAKING      0
#define READING          1
#define WRITING          2
#define ACCEPTING_STREAM 3
#define CREATING_STREAM  4
#define SWAPPING_STREAM  5

/*
 * This callback validates and negotiates the desired ALPN on the server side.
 * Accept any ALPN.
 */
static int select_alpn(SSL *ssl, const unsigned char **out,
                       unsigned char *out_len, const unsigned char *in,
                       unsigned int in_len, void *arg)
{
    return SSL_TLSEXT_ERR_OK;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    SSL *server = NULL, *stream = NULL;
    SSL *allstreams[] = {NULL, NULL, NULL, NULL};
    size_t i, thisstream = 0, numstreams = 1;
    BIO *in;
    BIO *out;
    SSL_CTX *ctx;
    struct timeval tv;
    int state = HANDSHAKING;
    uint8_t tmp[1024];
    int writelen = 0;

    if (len == 0)
        return 0;

    ctx = SSL_CTX_new(OSSL_QUIC_server_method());
    if (ctx == NULL)
        goto end;

    SSL_CTX_set_alpn_select_cb(ctx, select_alpn, NULL);

    server = SSL_new_listener(ctx, 0);
    allstreams[0] = stream = server;
    if (server == NULL)
        goto end;

    fake_now = ossl_ms2time(1);
    if (!ossl_quic_set_override_now_cb(server, fake_now_cb, NULL))
        goto end;

    in = BIO_new(BIO_s_dgram_mem());
    if (in == NULL)
        goto end;
    out = BIO_new(BIO_s_dgram_mem());
    if (out == NULL) {
        BIO_free(in);
        goto end;
    }
    if (!BIO_dgram_set_caps(out, BIO_DGRAM_CAP_HANDLES_DST_ADDR)) {
        BIO_free(in);
        BIO_free(out);
        goto end;
    }
    SSL_set_bio(server, in, out);
    SSL_set_accept_state(server);

    for (;;) {
        size_t size;
        uint64_t nxtpktms = 0;
        OSSL_TIME nxtpkt = ossl_time_zero(), nxttimeout;
        int isinf, ret = 0;

        if (len >= 2) {
            if (len >= 5 && buf[0] == 0xff && buf[1] == 0xff) {
                switch (buf[2]) {
                case 0x00:
                    if (state == READING)
                        state = ACCEPTING_STREAM;
                    break;
                case 0x01:
                    if (state == READING)
                        state = CREATING_STREAM;
                    break;
                case 0x02:
                    if (state == READING)
                        state = SWAPPING_STREAM;
                    break;
                default:
                    /* ignore */
                    break;
                }
                len -= 3;
                buf += 3;
            }
            nxtpktms = buf[0] + (buf[1] << 8);
            nxtpkt = ossl_time_add(fake_now, ossl_ms2time(nxtpktms));
            len -= 2;
            buf += 2;
        }

        for (;;) {
            switch (state) {
            case HANDSHAKING:
                ret = SSL_accept_connection(stream, 0) != NULL;
                if (ret == 1)
                    state = READING;
                break;

            case READING:
                ret = SSL_read(stream, tmp, sizeof(tmp));
                if (ret > 0) {
                    state = WRITING;
                    writelen = ret;
                    assert(writelen <= (int)sizeof(tmp));
                }
                break;

            case WRITING:
                ret = SSL_write(stream, tmp, writelen);
                if (ret > 0)
                    state = READING;
                break;

            case ACCEPTING_STREAM:
                state = READING;
                ret = 1;
                if (numstreams == OSSL_NELEM(allstreams)
                        || SSL_get_accept_stream_queue_len(server) == 0)
                    break;
                thisstream = numstreams;
                stream = allstreams[numstreams++] = SSL_accept_stream(server, 0);
                if (stream == NULL)
                    goto end;
                break;

            case CREATING_STREAM:
                state = READING;
                ret = 1;
                if (numstreams == OSSL_NELEM(allstreams))
                    break;
                stream = SSL_new_stream(server, 0);
                if (stream == NULL) {
                    /* Ignore, and go back to the previous stream */
                    stream = allstreams[thisstream];
                    break;
                }
                thisstream = numstreams;
                allstreams[numstreams++] = stream;
                break;

            case SWAPPING_STREAM:
                state = READING;
                ret = 1;
                if (numstreams == 1)
                    break;
                if (++thisstream == numstreams)
                    thisstream = 0;
                stream = allstreams[thisstream];
                break;
            }
            assert(stream != NULL);
            assert(thisstream < numstreams);
            if (ret <= 0) {
                switch (SSL_get_error(stream, ret)) {
                case SSL_ERROR_WANT_READ:
                case SSL_ERROR_WANT_WRITE:
                    break;
                default:
                    goto end;
                }
            }

            if (!SSL_get_event_timeout(server, &tv, &isinf))
                goto end;

            if (isinf) {
                fake_now = nxtpkt;
                break;
            } else {
                nxttimeout = ossl_time_add(fake_now,
                                           ossl_time_from_timeval(tv));
                if (len > 3 && ossl_time_compare(nxttimeout, nxtpkt) >= 0) {
                    fake_now = nxtpkt;
                    break;
                }
                fake_now = nxttimeout;
            }
        }

        if (len <= 3)
            break;

        size = buf[0] + (buf[1] << 8);
        if (size > len - 2)
            break;

        if (size > 0)
            BIO_write(in, buf + 2, size);
        len -= size + 2;
        buf += size + 2;
    }
 end:
    for (i = 0; i < numstreams; i++)
        SSL_free(allstreams[i]);
    ERR_clear_error();
    SSL_CTX_free(ctx);

    return 0;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
