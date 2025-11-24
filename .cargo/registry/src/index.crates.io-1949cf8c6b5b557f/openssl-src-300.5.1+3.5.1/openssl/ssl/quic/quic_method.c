/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/macros.h>
#include <openssl/objects.h>
#include "quic_local.h"

IMPLEMENT_quic_meth_func(OSSL_QUIC_ANY_VERSION,
                         OSSL_QUIC_client_method,
                         ssl_undefined_function,
                         ossl_quic_connect, ssl3_undef_enc_method)

IMPLEMENT_quic_meth_func(OSSL_QUIC_ANY_VERSION,
                         OSSL_QUIC_client_thread_method,
                         ssl_undefined_function,
                         ossl_quic_connect, ssl3_undef_enc_method)

IMPLEMENT_quic_meth_func(OSSL_QUIC_ANY_VERSION,
                         OSSL_QUIC_server_method,
                         ossl_quic_accept,
                         ssl_undefined_function, ssl3_undef_enc_method)
