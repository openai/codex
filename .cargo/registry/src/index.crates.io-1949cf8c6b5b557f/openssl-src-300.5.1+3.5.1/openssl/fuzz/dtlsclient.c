/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <time.h>
#include <openssl/rand.h>
#include <openssl/ssl.h>
#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include <openssl/ec.h>
#include <openssl/dh.h>
#include <openssl/err.h>
#include "fuzzer.h"

/* unused, to avoid warning. */
static int idx;

#define FUZZTIME 1485898104

#define TIME_IMPL(t) { if (t != NULL) *t = FUZZTIME; return FUZZTIME; }

/*
 * This might not work in all cases (and definitely not on Windows
 * because of the way linkers are) and callees can still get the
 * current time instead of the fixed time. This will just result
 * in things not being fully reproducible and have a slightly
 * different coverage.
 */
#if !defined(_WIN32)
time_t time(time_t *t) TIME_IMPL(t)
#endif

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

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    SSL *client = NULL;
    BIO *in;
    BIO *out;
    SSL_CTX *ctx;

    if (len == 0)
        return 0;

    /* This only fuzzes the initial flow from the client so far. */
    ctx = SSL_CTX_new(DTLS_client_method());
    if (ctx == NULL)
        goto end;

    client = SSL_new(ctx);
    if (client == NULL)
        goto end;
    OPENSSL_assert(SSL_set_min_proto_version(client, 0) == 1);
    OPENSSL_assert(SSL_set_cipher_list(client, "ALL:eNULL:@SECLEVEL=0") == 1);
    SSL_set_tlsext_host_name(client, "localhost");
    in = BIO_new(BIO_s_mem());
    if (in == NULL)
        goto end;
    out = BIO_new(BIO_s_mem());
    if (out == NULL) {
        BIO_free(in);
        goto end;
    }
    SSL_set_bio(client, in, out);
    SSL_set_connect_state(client);
    OPENSSL_assert((size_t)BIO_write(in, buf, len) == len);
    if (SSL_do_handshake(client) == 1) {
        /* Keep reading application data until error or EOF. */
        uint8_t tmp[1024];
        for (;;) {
            if (SSL_read(client, tmp, sizeof(tmp)) <= 0) {
                break;
            }
        }
    }
 end:
    SSL_free(client);
    ERR_clear_error();
    SSL_CTX_free(ctx);

    return 0;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
