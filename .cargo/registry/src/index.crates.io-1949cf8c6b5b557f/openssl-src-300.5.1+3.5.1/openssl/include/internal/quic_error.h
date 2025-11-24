/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_ERROR_H
# define OSSL_QUIC_ERROR_H

# include <openssl/ssl.h>
# include <openssl/quic.h>

# ifndef OPENSSL_NO_QUIC

#  define OSSL_QUIC_ERR_CRYPTO_UNEXPECTED_MESSAGE \
    OSSL_QUIC_ERR_CRYPTO_ERR(SSL3_AD_UNEXPECTED_MESSAGE)

#  define OSSL_QUIC_ERR_CRYPTO_MISSING_EXT \
    OSSL_QUIC_ERR_CRYPTO_ERR(TLS13_AD_MISSING_EXTENSION)

#  define OSSL_QUIC_ERR_CRYPTO_NO_APP_PROTO \
    OSSL_QUIC_ERR_CRYPTO_ERR(TLS1_AD_NO_APPLICATION_PROTOCOL)

const char *ossl_quic_err_to_string(uint64_t error_code);

# endif

#endif
