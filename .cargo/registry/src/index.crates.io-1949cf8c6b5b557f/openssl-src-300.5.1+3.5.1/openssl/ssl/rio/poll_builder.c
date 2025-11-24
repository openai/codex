/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <errno.h>
#include "internal/safe_math.h"
#include "poll_builder.h"

OSSL_SAFE_MATH_UNSIGNED(size_t, size_t)

int ossl_rio_poll_builder_init(RIO_POLL_BUILDER *rpb)
{
#if RIO_POLL_METHOD == RIO_POLL_METHOD_NONE
    return 0;
#elif RIO_POLL_METHOD == RIO_POLL_METHOD_SELECT
    FD_ZERO(&rpb->rfd);
    FD_ZERO(&rpb->wfd);
    FD_ZERO(&rpb->efd);
    rpb->hwm_fd     = -1;
#elif RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
    rpb->pfd_heap   = NULL;
    rpb->pfd_num    = 0;
    rpb->pfd_alloc  = OSSL_NELEM(rpb->pfds);
#endif
    return 1;
}

void ossl_rio_poll_builder_cleanup(RIO_POLL_BUILDER *rpb)
{
    if (rpb == NULL)
        return;

#if RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
    OPENSSL_free(rpb->pfd_heap);
#endif
}

#if RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
static int rpb_ensure_alloc(RIO_POLL_BUILDER *rpb, size_t alloc)
{
    struct pollfd *pfd_heap_new;
    size_t total_size;
    int error = 0;

    if (alloc <= rpb->pfd_alloc)
        return 1;

    total_size = safe_mul_size_t(alloc, sizeof(struct pollfd), &error);
    if (error)
        return 0;

    pfd_heap_new = OPENSSL_realloc(rpb->pfd_heap, total_size);
    if (pfd_heap_new == NULL)
        return 0;

    if (rpb->pfd_heap == NULL) {
        /* Copy the contents of the stacked array. */
        memcpy(pfd_heap_new, rpb->pfds, sizeof(rpb->pfds));
    }
    rpb->pfd_heap   = pfd_heap_new;
    rpb->pfd_alloc  = alloc;
    return 1;
}
#endif

int ossl_rio_poll_builder_add_fd(RIO_POLL_BUILDER *rpb, int fd,
                                 int want_read, int want_write)
{
#if RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
    size_t i;
    struct pollfd *pfds = (rpb->pfd_heap != NULL ? rpb->pfd_heap : rpb->pfds);
    struct pollfd *pfd;
#endif

    if (fd < 0)
        return 0;

#if RIO_POLL_METHOD == RIO_POLL_METHOD_SELECT

# ifndef OPENSSL_SYS_WINDOWS
    /*
     * On Windows there is no relevant limit to the magnitude of a fd value (see
     * above). On *NIX the fd_set uses a bitmap and we must check the limit.
     */
    if (fd >= FD_SETSIZE)
        return 0;
# endif

    if (want_read)
        openssl_fdset(fd, &rpb->rfd);

    if (want_write)
        openssl_fdset(fd, &rpb->wfd);

    openssl_fdset(fd, &rpb->efd);
    if (fd > rpb->hwm_fd)
        rpb->hwm_fd = fd;

    return 1;

#elif RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
    for (i = 0; i < rpb->pfd_num; ++i) {
        pfd = &pfds[i];

        if (pfd->fd == -1 || pfd->fd == fd)
            break;
    }

    if (i >= rpb->pfd_alloc) {
        if (!rpb_ensure_alloc(rpb, rpb->pfd_alloc * 2))
            return 0;
        pfds = rpb->pfd_heap;
    }

    assert((rpb->pfd_heap != NULL && rpb->pfd_heap == pfds) ||
           (rpb->pfd_heap == NULL && rpb->pfds == pfds));
    assert(i <= rpb->pfd_num && rpb->pfd_num <= rpb->pfd_alloc);
    pfds[i].fd      = fd;
    pfds[i].events  = 0;

    if (want_read)
        pfds[i].events |= POLLIN;
    if (want_write)
        pfds[i].events |= POLLOUT;

    if (i == rpb->pfd_num)
        ++rpb->pfd_num;

    return 1;
#endif
}

int ossl_rio_poll_builder_poll(RIO_POLL_BUILDER *rpb, OSSL_TIME deadline)
{
    int rc;

#if RIO_POLL_METHOD == RIO_POLL_METHOD_SELECT
    do {
        struct timeval timeout, *p_timeout = &timeout;

        /*
         * select expects a timeout, not a deadline, so do the conversion.
         * Update for each call to ensure the correct value is used if we repeat
         * due to EINTR.
         */
        if (ossl_time_is_infinite(deadline))
            p_timeout = NULL;
        else
            /*
             * ossl_time_subtract saturates to zero so we don't need to check if
             * now > deadline.
             */
            timeout = ossl_time_to_timeval(ossl_time_subtract(deadline,
                                                              ossl_time_now()));

        rc = select(rpb->hwm_fd + 1, &rpb->rfd, &rpb->wfd, &rpb->efd, p_timeout);
    } while (rc == -1 && get_last_socket_error_is_eintr());
#elif RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
    do {
        int timeout_ms;

        if (ossl_time_is_infinite(deadline))
            timeout_ms = -1;
        else
            timeout_ms = ossl_time2ms(ossl_time_subtract(deadline,
                                                         ossl_time_now()));

        rc = poll(rpb->pfd_heap != NULL ? rpb->pfd_heap : rpb->pfds,
                  rpb->pfd_num, timeout_ms);
    } while (rc == -1 && get_last_socket_error_is_eintr());
#endif

    return rc < 0 ? 0 : 1;
}
