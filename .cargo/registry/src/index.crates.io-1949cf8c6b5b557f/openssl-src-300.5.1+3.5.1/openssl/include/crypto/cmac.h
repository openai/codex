/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_CMAC_H
# define OSSL_CRYPTO_CMAC_H
# pragma once

# include <openssl/types.h>
# include <openssl/cmac.h>
# include <openssl/params.h>

int ossl_cmac_init(CMAC_CTX *ctx, const void *key, size_t keylen,
                   const EVP_CIPHER *cipher, ENGINE *impl,
                   const OSSL_PARAM param[]);

#endif  /* OSSL_CRYPTO_CMAC_H */
