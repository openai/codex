/*
 * Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>

#ifndef OSSL_INTERNAL_SSL_H
# define OSSL_INTERNAL_SSL_H
# pragma once

typedef void (*ossl_msg_cb)(int write_p, int version, int content_type,
                            const void *buf, size_t len, SSL *ssl, void *arg);

int ossl_ssl_get_error(const SSL *s, int i, int check_err);

/* Set if this is the QUIC handshake layer */
# define TLS1_FLAGS_QUIC                         0x2000
/* Set if this is our QUIC handshake layer */
# define TLS1_FLAGS_QUIC_INTERNAL                0x4000

#endif
