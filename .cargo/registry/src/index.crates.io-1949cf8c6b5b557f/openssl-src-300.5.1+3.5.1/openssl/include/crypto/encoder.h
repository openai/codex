/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ENCODER_H
# define OSSL_CRYPTO_ENCODER_H
# pragma once

# include <openssl/types.h>

int ossl_encoder_get_number(const OSSL_ENCODER *encoder);
int ossl_encoder_store_cache_flush(OSSL_LIB_CTX *libctx);
int ossl_encoder_store_remove_all_provided(const OSSL_PROVIDER *prov);

#endif
