/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if defined(_AIX)
/*
 * Some versions of AIX define macros for events and revents for use when
 * accessing pollfd structures (see Github issue #24236). That interferes
 * with our use of these names here. We simply undef them.
 */
# undef revents
# undef events
#endif

/*
 * Test Scripts
 * ============================================================================
 */

/*
 * Test: simple_conn
 * -----------------
 */
DEF_SCRIPT(simple_conn, "simple connection to server")
{
    size_t i;

    for (i = 0; i < 2; ++i) {
        if (i == 0) {
            OP_SIMPLE_PAIR_CONN_D();
        } else {
            OP_CLEAR();
            OP_SIMPLE_PAIR_CONN();
        }

        OP_WRITE_B(C, "apple");

        OP_ACCEPT_CONN_WAIT(L, La, 0);
        OP_ACCEPT_CONN_NONE(L);

        OP_READ_EXPECT_B(La, "apple");
        OP_WRITE_B(La, "orange");
        OP_READ_EXPECT_B(C, "orange");
    }
}

DEF_SCRIPT(simple_thread_child,
           "test that RADIX multithreading is working (child)")
{
}

/*
 * Test: simple_thread
 * -------------------
 */
DEF_SCRIPT(simple_thread,
           "test that RADIX multithreading is working")
{
    size_t i;

    for (i = 0; i < 2; ++i)
        OP_SPAWN_THREAD(simple_thread_child);
}

/*
 * Test: ssl_poll
 * --------------
 */
DEF_SCRIPT(ssl_poll_child,
           "test that SSL_poll is working (child)")
{
    OP_SLEEP(100);
    OP_WRITE_B(C0, "extra");
}

DEF_FUNC(ssl_poll_check)
{
    int ok = 0;
    SSL *La, *Lax[4];
    SSL_POLL_ITEM items[6] = {0}, expected_items[6] = {0};
    size_t result_count = 0, i;
    const struct timeval z_timeout = {0}, *p_timeout = &z_timeout;
    struct timeval timeout = {0};
    uint64_t mode;
    size_t expected_result_count;
    OSSL_TIME time_before, time_after;

    F_POP(mode);
    REQUIRE_SSL_5(La, Lax[0], Lax[1], Lax[2], Lax[3]);

    items[0].desc      = SSL_as_poll_descriptor(La);
    items[0].events    = 0;
    items[0].revents   = 0;

    for (i = 0; i < 4; ++i) {
        items[i + 1].desc        = SSL_as_poll_descriptor(Lax[i]);
        items[i + 1].events      = SSL_POLL_EVENT_R | SSL_POLL_EVENT_I;
        items[i + 1].revents     = 0;
    }

    items[5].desc = SSL_as_poll_descriptor(SSL_get0_listener(La));

    switch (mode) {
    case 0: /* Nothing ready */
    case 2:
        expected_result_count = 0;
        break;
    case 1: /* Various events reported correctly */
        expected_result_count       = 5;
        items[0].events             = SSL_POLL_EVENT_OS;
        expected_items[0].revents   = SSL_POLL_EVENT_OS;

        expected_items[1].revents   = SSL_POLL_EVENT_R;

        for (i = 0; i < 4; ++i) {
            items[i + 1].events             |= SSL_POLL_EVENT_W;
            expected_items[i + 1].revents   |= SSL_POLL_EVENT_W;
        }

        break;
    case 3: /* Blocking test */
        expected_result_count       = 1;
        expected_items[1].revents   = SSL_POLL_EVENT_R;

        p_timeout = &timeout;
        timeout.tv_sec  = 10;
        timeout.tv_usec = 0;
        break;
    case 4: /* Listener test */
        expected_result_count       = 1;
        items[5].events             = SSL_POLL_EVENT_IC;
        expected_items[5].revents   = SSL_POLL_EVENT_IC;
        break;
    default:
        goto err;
    }

    /* Zero-timeout call. */
    result_count = SIZE_MAX;
    time_before = ossl_time_now();
    if (!TEST_true(SSL_poll(items, OSSL_NELEM(items), sizeof(SSL_POLL_ITEM),
                            p_timeout, 0, &result_count)))
        goto err;

    time_after = ossl_time_now();
    if (!TEST_size_t_eq(result_count, expected_result_count))
        goto err;

    for (i = 0; i < OSSL_NELEM(items); ++i)
        if (!TEST_uint64_t_eq(items[i].revents, expected_items[i].revents))
            goto err;

    /*
     * The SSL_poll call for the blocking test definitely shouldn't have
     * returned sooner than in 100ms.
     */
    if (i == 3 && !TEST_uint64_t_ge(ossl_time2ms(ossl_time_subtract(time_after, time_before)), 100))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_SCRIPT(ssl_poll,
           "test that SSL_poll is working")
{
    size_t i;

    OP_SIMPLE_PAIR_CONN_ND();

    /* Setup streams */
    OP_NEW_STREAM(C, C0, 0);
    OP_WRITE_B(C0, "apple");

    OP_NEW_STREAM(C, C1, 0);
    OP_WRITE_B(C1, "orange");

    OP_NEW_STREAM(C, C2, 0);
    OP_WRITE_B(C2, "Strawberry");

    OP_NEW_STREAM(C, C3, 0);
    OP_WRITE_B(C3, "sync");

    OP_ACCEPT_CONN_WAIT1_ND(L, La, 0);

    OP_ACCEPT_STREAM_WAIT(La, La0, 0);
    OP_READ_EXPECT_B(La0, "apple");

    OP_ACCEPT_STREAM_WAIT(La, La1, 0);
    OP_READ_EXPECT_B(La1, "orange");

    OP_ACCEPT_STREAM_WAIT(La, La2, 0);
    OP_READ_EXPECT_B(La2, "Strawberry");

    OP_ACCEPT_STREAM_WAIT(La, La3, 0);
    OP_READ_EXPECT_B(La3, "sync");

    for (i = 0; i <= 4; ++i) {
        /* 0: Check nothing ready */
        /* 1: Check that various events are reported correctly */
        /* 2: Check nothing ready */
        /* 3: Blocking call unblocked from child thread */
        /* 4: Listener test */

        if (i == 1) {
            OP_WRITE_B(C0, "orange");
            OP_WRITE_B(C3, "sync");
            OP_READ_EXPECT_B(La3, "sync");
        } else if (i == 2) {
            OP_READ_EXPECT_B(La0, "orange");
        } else if (i == 3) {
            OP_SPAWN_THREAD(ssl_poll_child);
        } else if (i == 4) {
            OP_NEW_SSL_C(Cb);
            OP_SET_PEER_ADDR_FROM(Cb, L);
            OP_CONNECT_WAIT(Cb);
        }

        OP_SELECT_SSL(0, La);
        OP_SELECT_SSL(1, La0);
        OP_SELECT_SSL(2, La1);
        OP_SELECT_SSL(3, La2);
        OP_SELECT_SSL(4, La3);
        OP_PUSH_U64(i);
        OP_FUNC(ssl_poll_check);

        if (i == 3)
            OP_READ_EXPECT_B(La0, "extra");

        if (i == 4) {
            OP_ACCEPT_CONN_WAIT1_ND(L, Lb, 0);
            OP_NEW_STREAM(Lb, Lb0, 0);
            OP_WRITE_B(Lb0, "foo");
            OP_READ_EXPECT_B(Cb, "foo");
        }
    }
}

DEF_FUNC(check_writeable)
{
    int ok = 0;
    SSL *ssl;
    SSL_POLL_ITEM item;
    size_t result_count = 0;
    uint64_t expect;
    const struct timeval z_timeout = {0}, *p_timeout = &z_timeout;

    F_POP(expect);
    REQUIRE_SSL(ssl);

    item.desc      = SSL_as_poll_descriptor(ssl);
    item.events    = SSL_POLL_EVENT_W;
    item.revents   = 0;

    /* Zero-timeout call. */
    result_count = SIZE_MAX;
    if (!TEST_true(SSL_poll(&item, 1, sizeof(SSL_POLL_ITEM),
                            p_timeout, 0, &result_count)))
        goto err;

    ok = (!!(item.revents & SSL_POLL_EVENT_W) == expect);

 err:
    return ok;
}

DEF_SCRIPT(check_cwm, "check stream obeys cwm")
{
    OP_SIMPLE_PAIR_CONN();

    /* Create the initial stream by writing some data */
    OP_WRITE_RAND(C, 1024);

    /* We should be writeable at the start */
    OP_PUSH_U64(1);
    OP_SELECT_SSL(0, C);
    OP_FUNC(check_writeable);

    /* Default stream cwm is 512k (we already sent 1k). Consume all the rest */
    OP_WRITE_RAND(C, 511 * 1024);

    /* Confirm we are no longer writeable */
    OP_PUSH_U64(0);
    OP_SELECT_SSL(0, C);
    OP_FUNC(check_writeable);

    /* We now expect writes to fail */
    OP_WRITE_FAIL(C);
}

/*
 * List of Test Scripts
 * ============================================================================
 */
static SCRIPT_INFO *const scripts[] = {
    USE(simple_conn)
    USE(simple_thread)
    USE(ssl_poll)
    USE(check_cwm)
};
