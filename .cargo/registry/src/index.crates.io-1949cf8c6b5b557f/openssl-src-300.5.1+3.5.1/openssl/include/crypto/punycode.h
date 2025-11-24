/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_PUNYCODE_H
# define OSSL_CRYPTO_PUNYCODE_H
# pragma once

# include <stddef.h>     /* for size_t */

int ossl_punycode_decode (
    const char *pEncoded,
    const size_t enc_len,
    unsigned int *pDecoded,
    unsigned int *pout_length
);

int ossl_a2ulabel(const char *in, char *out, size_t outlen);

#endif
