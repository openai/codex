/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_RIO_NOTIFIER_H
# define OSSL_RIO_NOTIFIER_H

# include "internal/common.h"
# include "internal/sockets.h"

/*
 * Pollable Notifier
 * =================
 *
 * RIO_NOTIFIER provides an OS-pollable resource which can be plugged into an
 * OS's socket polling APIs to allow socket polling calls to be woken
 * artificially by other threads.
 */
# define RIO_NOTIFIER_METHOD_SOCKET      1
# define RIO_NOTIFIER_METHOD_SOCKETPAIR  2

# if !defined(RIO_NOTIFIER_METHOD)
#  if defined(OPENSSL_SYS_WINDOWS)
#   define RIO_NOTIFIER_METHOD          RIO_NOTIFIER_METHOD_SOCKET
#  elif defined(OPENSSL_SYS_UNIX)
#   define RIO_NOTIFIER_METHOD          RIO_NOTIFIER_METHOD_SOCKETPAIR
#  else
#   define RIO_NOTIFIER_METHOD          RIO_NOTIFIER_METHOD_SOCKET
#  endif
# endif

typedef struct rio_notifier_st {
    int rfd, wfd;
} RIO_NOTIFIER;

/*
 * Initialises a RIO_NOTIFIER. Returns 1 on success or 0 on failure.
 */
int ossl_rio_notifier_init(RIO_NOTIFIER *nfy);

/*
 * Cleans up a RIO_NOTIFIER, tearing down any allocated resources.
 */
void ossl_rio_notifier_cleanup(RIO_NOTIFIER *nfy);

/*
 * Signals a RIO_NOTIFIER, waking up any waiting threads.
 */
int ossl_rio_notifier_signal(RIO_NOTIFIER *nfy);

/*
 * Unsignals a RIO_NOTIFIER.
 */
int ossl_rio_notifier_unsignal(RIO_NOTIFIER *nfy);

/*
 * Returns an OS socket handle (FD or Win32 SOCKET) which can be polled for
 * readability to determine when the notifier has been signalled.
 */
static ossl_inline ossl_unused int ossl_rio_notifier_as_fd(RIO_NOTIFIER *nfy)
{
    return nfy->rfd;
}

#endif
