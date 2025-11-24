/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_POLL_BUILDER_H
# define OSSL_POLL_BUILDER_H

# include "poll_method.h"
# include "internal/time.h"

/*
 * RIO_POLL_BUILDER
 * ================
 *
 * RIO_POLL_BUILDER provides support for immediate-mode polling architectures.
 * It abstracts OS-specific immediate-mode polling APIs such as select(2) and
 * poll(2) and allows an arbitrarily large number of FDs to be polled for while
 * providing minimal overhead (over the OS APIs themselves) for small numbers of
 * FDs.
 */
typedef struct rio_poll_builder_st {
# if RIO_POLL_METHOD == RIO_POLL_METHOD_NONE
    int             unused_dummy; /* make microsoft compiler happy */
# elif RIO_POLL_METHOD == RIO_POLL_METHOD_SELECT
    fd_set          rfd, wfd, efd;
    int             hwm_fd;
# elif RIO_POLL_METHOD == RIO_POLL_METHOD_POLL
#  define RIO_NUM_STACK_PFDS  32
    struct pollfd   *pfd_heap;
    struct pollfd   pfds[RIO_NUM_STACK_PFDS];
    size_t          pfd_num, pfd_alloc;
# else
#  error Unknown RIO poll method
# endif
} RIO_POLL_BUILDER;

/*
 * Initialises a new poll builder.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_rio_poll_builder_init(RIO_POLL_BUILDER *rpb);

/*
 * Tears down a poll builder, freeing any heap allocations (if any) which may
 * have been made internally.
 */
void ossl_rio_poll_builder_cleanup(RIO_POLL_BUILDER *rpb);

/*
 * Adds a file descriptor to a poll builder. If want_read is 1, listens for
 * readability events (POLLIN). If want_write is 1, listens for writability
 * events (POLLOUT).
 *
 * If this is called with the same fd twice, the result equivalent to calling
 * it one time with logically OR'd values of want_read and want_write.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_rio_poll_builder_add_fd(RIO_POLL_BUILDER *rpb, int fd,
                                 int want_read, int want_write);

/*
 * Polls the set of file descriptors added to a poll builder. deadline is a
 * deadline time based on the ossl_time_now() clock or ossl_time_infinite() for
 * no timeout. Returns 1 on success or 0 on failure.
 */
int ossl_rio_poll_builder_poll(RIO_POLL_BUILDER *rpb, OSSL_TIME deadline);

/*
 * TODO(RIO): No support currently for readout of what was readable/writeable as
 * it is currently not needed.
 */

#endif
