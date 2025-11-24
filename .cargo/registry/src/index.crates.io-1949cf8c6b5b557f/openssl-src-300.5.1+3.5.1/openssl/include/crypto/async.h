/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ASYNC_H
# define OSSL_CRYPTO_ASYNC_H
# pragma once

# include <openssl/async.h>

int async_init(void);
void async_deinit(void);

#endif
