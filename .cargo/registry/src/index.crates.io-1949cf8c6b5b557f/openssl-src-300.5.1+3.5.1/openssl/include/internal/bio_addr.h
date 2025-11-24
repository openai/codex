/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_BIO_ADDR_H
# define OSSL_BIO_ADDR_H

# include "internal/e_os.h"
# include "internal/e_winsock.h"
# include "internal/sockets.h"

# ifndef OPENSSL_NO_SOCK
union bio_addr_st {
    struct sockaddr sa;
#  if OPENSSL_USE_IPV6
    struct sockaddr_in6 s_in6;
#  endif
    struct sockaddr_in s_in;
#  ifndef OPENSSL_NO_UNIX_SOCK
    struct sockaddr_un s_un;
#  endif
};
# endif

#endif
