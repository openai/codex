/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/common.h"
#include "internal/quic_ssl.h"
#include "internal/quic_reactor_wait_ctx.h"
#include <openssl/ssl.h>
#include <openssl/err.h>
#include "../ssl_local.h"
#include "poll_builder.h"

#if defined(_AIX)
/*
 * Some versions of AIX define macros for events and revents for use when
 * accessing pollfd structures (see Github issue #24236). That interferes
 * with our use of these names here. We simply undef them.
 */
# undef revents
# undef events
#endif

#define ITEM_N(items, stride, n) \
    (*(SSL_POLL_ITEM *)((char *)(items) + (n)*(stride)))

#define FAIL_FROM(n)                                                        \
    do {                                                                    \
        size_t j;                                                           \
                                                                            \
        for (j = (n); j < num_items; ++j)                                   \
            ITEM_N(items, stride, j).revents = 0;                           \
                                                                            \
        ok = 0;                                                             \
        goto out;                                                           \
    } while (0)

#define FAIL_ITEM(idx)                                                      \
    do {                                                                    \
        size_t idx_ = (idx);                                                \
                                                                            \
        ITEM_N(items, stride, idx_).revents = SSL_POLL_EVENT_F;             \
        ++result_count;                                                     \
        FAIL_FROM(idx_ + 1);                                                \
    } while (0)

#ifndef OPENSSL_NO_QUIC
static int poll_translate_ssl_quic(SSL *ssl,
                                   QUIC_REACTOR_WAIT_CTX *wctx,
                                   RIO_POLL_BUILDER *rpb,
                                   uint64_t events,
                                   int *abort_blocking)
{
    BIO_POLL_DESCRIPTOR rd, wd;
    int fd1 = -1, fd2 = -1, fd_nfy = -1;
    int fd1_r = 0, fd1_w = 0, fd2_w = 0;

    if (SSL_net_read_desired(ssl)) {
        if (!SSL_get_rpoll_descriptor(ssl, &rd)) {
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll requires the network BIOs underlying "
                           "a QUIC SSL object provide poll descriptors");
            return 0;
        }

        if (rd.type != BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD) {
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll requires the poll descriptors of the "
                           "network BIOs underlying a QUIC SSL object be "
                           "of socket type");
            return 0;
        }

        fd1   = rd.value.fd;
        fd1_r = 1;
    }

    if (SSL_net_write_desired(ssl)) {
        if (!SSL_get_wpoll_descriptor(ssl, &wd)) {
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll requires the network BIOs underlying "
                           "a QUIC SSL object provide poll descriptors");
            return 0;
        }

        if (wd.type != BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD) {
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll requires the poll descriptors of the "
                           "network BIOs underlying a QUIC SSL object be "
                           "of socket type");
            return 0;
        }

        fd2   = wd.value.fd;
        fd2_w = 1;
    }

    if (fd2 == fd1) {
        fd2 = -1;
        fd1_w = fd2_w;
    }

    if (fd1 != -1)
        if (!ossl_rio_poll_builder_add_fd(rpb, fd1, fd1_r, fd1_w))
            return 0;

    if (fd2 != -1 && fd2_w)
        if (!ossl_rio_poll_builder_add_fd(rpb, fd2, /*r = */0, fd2_w))
            return 0;

    /*
     * Add the notifier FD for the QUIC domain this SSL object is a part of (if
     * there is one). This ensures we get woken up if another thread calls into
     * that QUIC domain and some readiness event relevant to the SSL_poll call
     * on this thread arises without the underlying network socket ever becoming
     * readable.
     */
    fd_nfy = ossl_quic_get_notifier_fd(ssl);
    if (fd_nfy != -1) {
        uint64_t revents = 0;

        if (!ossl_rio_poll_builder_add_fd(rpb, fd_nfy, /*r = */1, /*w = */0))
            return 0;

        /* Tell QUIC domain we need to receive notifications. */
        ossl_quic_enter_blocking_section(ssl, wctx);

        /*
         * Only after the above call returns is it guaranteed that any readiness
         * events will cause the above notifier to become readable. Therefore,
         * it is possible the object became ready after our initial
         * poll_readout() call (before we determined that nothing was ready and
         * we needed to block). We now need to do another readout, in which case
         * blocking is to be aborted.
         */
        if (!ossl_quic_conn_poll_events(ssl, events, /*do_tick = */0, &revents)) {
            ossl_quic_leave_blocking_section(ssl, wctx);
            return 0;
        }

        if (revents != 0) {
            ossl_quic_leave_blocking_section(ssl, wctx);
            *abort_blocking = 1;
            return 1;
        }
    }

    return 1;
}

static void postpoll_translation_cleanup_ssl_quic(SSL *ssl,
                                                  QUIC_REACTOR_WAIT_CTX *wctx)
{
    if (ossl_quic_get_notifier_fd(ssl) != -1)
        ossl_quic_leave_blocking_section(ssl, wctx);
}

static void postpoll_translation_cleanup(SSL_POLL_ITEM *items,
                                         size_t num_items,
                                         size_t stride,
                                         QUIC_REACTOR_WAIT_CTX *wctx)
{
    SSL_POLL_ITEM *item;
    SSL *ssl;
    size_t i;

    for (i = 0; i < num_items; ++i) {
        item = &ITEM_N(items, stride, i);

        switch (item->desc.type) {
        case BIO_POLL_DESCRIPTOR_TYPE_SSL:
            ssl = item->desc.value.ssl;
            if (ssl == NULL)
                break;

            switch (ssl->type) {
# ifndef OPENSSL_NO_QUIC
            case SSL_TYPE_QUIC_LISTENER:
            case SSL_TYPE_QUIC_CONNECTION:
            case SSL_TYPE_QUIC_XSO:
                postpoll_translation_cleanup_ssl_quic(ssl, wctx);
                break;
# endif
            default:
                break;
            }
            break;
        default:
            break;
        }
    }
}

static int poll_translate(SSL_POLL_ITEM *items,
                          size_t num_items,
                          size_t stride,
                          QUIC_REACTOR_WAIT_CTX *wctx,
                          RIO_POLL_BUILDER *rpb,
                          OSSL_TIME *p_earliest_wakeup_deadline,
                          int *abort_blocking,
                          size_t *p_result_count)
{
    int ok = 1;
    SSL_POLL_ITEM *item;
    size_t result_count = 0;
    SSL *ssl;
    OSSL_TIME earliest_wakeup_deadline = ossl_time_infinite();
    struct timeval timeout;
    int is_infinite = 0;
    size_t i;

    for (i = 0; i < num_items; ++i) {
        item = &ITEM_N(items, stride, i);

        switch (item->desc.type) {
        case BIO_POLL_DESCRIPTOR_TYPE_SSL:
            ssl = item->desc.value.ssl;
            if (ssl == NULL)
                /* NULL items are no-ops and have revents reported as 0 */
                break;

            switch (ssl->type) {
# ifndef OPENSSL_NO_QUIC
            case SSL_TYPE_QUIC_LISTENER:
            case SSL_TYPE_QUIC_CONNECTION:
            case SSL_TYPE_QUIC_XSO:
                if (!poll_translate_ssl_quic(ssl, wctx, rpb, item->events,
                                             abort_blocking))
                    FAIL_ITEM(i);

                if (*abort_blocking)
                    return 1;

                if (!SSL_get_event_timeout(ssl, &timeout, &is_infinite))
                    FAIL_ITEM(i++); /* need to clean up this item too */

                if (!is_infinite)
                    earliest_wakeup_deadline
                        = ossl_time_min(earliest_wakeup_deadline,
                                        ossl_time_add(ossl_time_now(),
                                                      ossl_time_from_timeval(timeout)));

                break;
# endif

            default:
                ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                               "SSL_poll currently only supports QUIC SSL "
                               "objects");
                FAIL_ITEM(i);
            }
            break;

        case BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD:
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll currently does not support polling "
                           "sockets");
            FAIL_ITEM(i);

        default:
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll does not support unknown poll descriptor "
                           "type %d", item->desc.type);
            FAIL_ITEM(i);
        }
    }

out:
    if (!ok)
        postpoll_translation_cleanup(items, i, stride, wctx);

    *p_earliest_wakeup_deadline = earliest_wakeup_deadline;
    *p_result_count = result_count;
    return ok;
}

static int poll_block(SSL_POLL_ITEM *items,
                      size_t num_items,
                      size_t stride,
                      OSSL_TIME user_deadline,
                      size_t *p_result_count)
{
    int ok = 0, abort_blocking = 0;
    RIO_POLL_BUILDER rpb;
    QUIC_REACTOR_WAIT_CTX wctx;
    OSSL_TIME earliest_wakeup_deadline;

    /*
     * Blocking is somewhat involved and involves the following steps:
     *
     * - Translation, in which the various logical items (SSL objects, etc.) to
     *   be polled are translated into items an OS polling API understands.
     *
     * - Synchronisation bookkeeping. This ensures that we can be woken up
     *   not just by readiness of any underlying file descriptor distilled from
     *   the provided items but also by other threads, which might do work
     *   on a relevant QUIC object to cause the object to be ready without the
     *   underlying file descriptor ever becoming ready from our perspective.
     *
     * - The blocking call to the OS polling API.
     *
     * - Currently we do not do reverse translation but simply call
     *   poll_readout() again to read out all readiness state for all
     *   descriptors which the user passed.
     *
     *   TODO(QUIC POLLING): In the future we will do reverse translation here
     *   also to facilitate a more efficient readout.
     */
    ossl_quic_reactor_wait_ctx_init(&wctx);
    ossl_rio_poll_builder_init(&rpb);

    if (!poll_translate(items, num_items, stride, &wctx, &rpb,
                        &earliest_wakeup_deadline,
                        &abort_blocking,
                        p_result_count))
        goto out;

    if (abort_blocking)
        goto out;

    earliest_wakeup_deadline = ossl_time_min(earliest_wakeup_deadline,
                                             user_deadline);

    ok = ossl_rio_poll_builder_poll(&rpb, earliest_wakeup_deadline);

    postpoll_translation_cleanup(items, num_items, stride, &wctx);

out:
    ossl_rio_poll_builder_cleanup(&rpb);
    ossl_quic_reactor_wait_ctx_cleanup(&wctx);
    return ok;
}
#endif

static int poll_readout(SSL_POLL_ITEM *items,
                        size_t num_items,
                        size_t stride,
                        int do_tick,
                        size_t *p_result_count)
{
    int ok = 1;
    size_t i, result_count = 0;
    SSL_POLL_ITEM *item;
    SSL *ssl;
#ifndef OPENSSL_NO_QUIC
    uint64_t events;
#endif
    uint64_t revents;

    for (i = 0; i < num_items; ++i) {
        item    = &ITEM_N(items, stride, i);
#ifndef OPENSSL_NO_QUIC
        events  = item->events;
#endif
        revents = 0;

        switch (item->desc.type) {
        case BIO_POLL_DESCRIPTOR_TYPE_SSL:
            ssl = item->desc.value.ssl;
            if (ssl == NULL)
                /* NULL items are no-ops and have revents reported as 0 */
                break;

            switch (ssl->type) {
#ifndef OPENSSL_NO_QUIC
            case SSL_TYPE_QUIC_LISTENER:
            case SSL_TYPE_QUIC_CONNECTION:
            case SSL_TYPE_QUIC_XSO:
                if (!ossl_quic_conn_poll_events(ssl, events, do_tick, &revents))
                    /* above call raises ERR */
                    FAIL_ITEM(i);

                if (revents != 0)
                    ++result_count;

                break;
#endif

            default:
                ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                               "SSL_poll currently only supports QUIC SSL "
                               "objects");
                FAIL_ITEM(i);
            }
            break;
        case BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD:
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll currently does not support polling "
                           "sockets");
            FAIL_ITEM(i);
        default:
            ERR_raise_data(ERR_LIB_SSL, SSL_R_POLL_REQUEST_NOT_SUPPORTED,
                           "SSL_poll does not support unknown poll descriptor "
                           "type %d", item->desc.type);
            FAIL_ITEM(i);
        }

        item->revents = revents;
    }

out:
    if (p_result_count != NULL)
        *p_result_count = result_count;

    return ok;
}

int SSL_poll(SSL_POLL_ITEM *items,
             size_t num_items,
             size_t stride,
             const struct timeval *timeout,
             uint64_t flags,
             size_t *p_result_count)
{
    int ok = 1;
    size_t result_count = 0;
    ossl_unused int do_tick = ((flags & SSL_POLL_FLAG_NO_HANDLE_EVENTS) == 0);
    OSSL_TIME deadline;

    /* Trivial case. */
    if (num_items == 0) {
        if (timeout == NULL)
            goto out;
        OSSL_sleep(ossl_time2ms(ossl_time_from_timeval(*timeout)));
        goto out;
    }

    /* Convert timeout to deadline. */
    if (timeout == NULL)
        deadline = ossl_time_infinite();
    else if (timeout->tv_sec == 0 && timeout->tv_usec == 0)
        deadline = ossl_time_zero();
    else
        deadline = ossl_time_add(ossl_time_now(),
                                 ossl_time_from_timeval(*timeout));

    /* Loop until we have something to report. */
    for (;;) {
        /* Readout phase - poll current state of each item. */
        if (!poll_readout(items, num_items, stride, do_tick, &result_count)) {
            ok = 0;
            goto out;
        }

        /*
         * If we got anything, or we are in immediate mode (zero timeout), or
         * the deadline has expired, we're done.
         */
        if (result_count > 0
            || ossl_time_is_zero(deadline) /* (avoids now call) */
            || ossl_time_compare(ossl_time_now(), deadline) >= 0)
            goto out;

        /*
         * Block until something is ready. Ignore NO_HANDLE_EVENTS from this
         * point onwards.
         */
        do_tick = 1;
#ifndef OPENSSL_NO_QUIC
        if (!poll_block(items, num_items, stride, deadline, &result_count)) {
            ok = 0;
            goto out;
        }
#endif
    }

    /* TODO(QUIC POLLING): Support for polling FDs */

out:
    if (p_result_count != NULL)
        *p_result_count = result_count;

    return ok;
}
