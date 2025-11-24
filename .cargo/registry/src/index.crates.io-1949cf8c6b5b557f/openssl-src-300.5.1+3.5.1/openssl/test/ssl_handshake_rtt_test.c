/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/engine.h>

#include "helpers/ssltestlib.h"
#include "testutil.h"
#include "testutil/output.h"
#include "internal/ktls.h"
#include "../ssl/ssl_local.h"
#include "../ssl/statem/statem_local.h"
#include "internal/ssl_unwrap.h"

static OSSL_LIB_CTX *libctx = NULL;
static char *cert = NULL;
static char *privkey = NULL;

/*
 * Test 0: Clientside handshake RTT (TLSv1.2)
 * Test 1: Serverside handshake RTT (TLSv1.2)
 * Test 2: Clientside handshake RTT (TLSv1.3)
 * Test 3: Serverside handshake RTT (TLSv1.3)
 * Test 4: Clientside handshake RTT with Early Data (TLSv1.3)
 */
static int test_handshake_rtt(int tst)
{
    SSL_CTX *cctx = NULL, *sctx = NULL;
    SSL *clientssl = NULL, *serverssl = NULL;
    int testresult = 0;
    SSL_CONNECTION *s = NULL;
    OSSL_STATEM *st = NULL;
    uint64_t rtt;

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
                                       &sctx, &cctx, cert, privkey))
            || !TEST_true(create_ssl_objects(sctx, cctx, &serverssl, &clientssl,
                                             NULL, NULL)))
        goto end;

    s = SSL_CONNECTION_FROM_SSL(tst % 2 == 0 ? clientssl : serverssl);
    if (!TEST_ptr(s) || !TEST_ptr(st = &s->statem))
        return 0;

    /* implicitly set handshake rtt with a delay */
    switch (tst) {
    case 0:
        st->hand_state = TLS_ST_CW_CLNT_HELLO;
        ossl_statem_client_write_transition(s);
        OSSL_sleep(1);
        st->hand_state = TLS_ST_CR_SRVR_DONE;
        ossl_statem_client_write_transition(s);
        break;
    case 1:
        st->hand_state = TLS_ST_SW_SRVR_DONE;
        ossl_statem_server_write_transition(s);
        OSSL_sleep(1);
        st->hand_state = TLS_ST_SR_FINISHED;
        ossl_statem_server_write_transition(s);
        break;
    case 2:
        st->hand_state = TLS_ST_CW_CLNT_HELLO;
        ossl_statem_client_write_transition(s);
        OSSL_sleep(1);
        st->hand_state = TLS_ST_CR_SRVR_DONE;
        ossl_statem_client_write_transition(s);
        break;
    case 3:
        st->hand_state = TLS_ST_SW_SRVR_DONE;
        ossl_statem_server_write_transition(s);
        OSSL_sleep(1);
        st->hand_state = TLS_ST_SR_FINISHED;
        ossl_statem_server_write_transition(s);
        break;
    case 4:
        st->hand_state = TLS_ST_EARLY_DATA;
        ossl_statem_client_write_transition(s);
        OSSL_sleep(1);
        st->hand_state = TLS_ST_CR_SRVR_DONE;
        ossl_statem_client_write_transition(s);
        break;
    }

    if (!TEST_int_gt(SSL_get_handshake_rtt(SSL_CONNECTION_GET_SSL(s), &rtt), 0))
        goto end;
    /* 1 millisec is the absolute minimum it could be given the delay */
    if (!TEST_uint64_t_ge(rtt, 1000))
        goto end;

    testresult = 1;

 end:
    SSL_free(serverssl);
    SSL_free(clientssl);
    SSL_CTX_free(sctx);
    SSL_CTX_free(cctx);

    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_handshake_rtt, 5);

    return 1;
}
