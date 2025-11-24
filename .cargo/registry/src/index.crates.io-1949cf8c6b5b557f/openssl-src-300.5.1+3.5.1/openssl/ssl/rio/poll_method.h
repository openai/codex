/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_POLL_METHOD_H
# define OSSL_POLL_METHOD_H

# include "internal/common.h"
# include "internal/sockets.h"

# define RIO_POLL_METHOD_SELECT         1
# define RIO_POLL_METHOD_POLL           2
# define RIO_POLL_METHOD_NONE           3

# ifndef RIO_POLL_METHOD
#  if defined(OPENSSL_SYS_UEFI)
#   define RIO_POLL_METHOD              RIO_POLL_METHOD_NONE
#  elif !defined(OPENSSL_SYS_WINDOWS) && defined(POLLIN)
#   define RIO_POLL_METHOD              RIO_POLL_METHOD_POLL
#  else
#   define RIO_POLL_METHOD              RIO_POLL_METHOD_SELECT
#  endif
# endif

#endif
