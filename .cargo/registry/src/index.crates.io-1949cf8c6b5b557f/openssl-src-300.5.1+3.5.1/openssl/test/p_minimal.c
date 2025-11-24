/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is the most minimal provider imaginable.  It can be loaded, and does
 * absolutely nothing else.
 */

#include <openssl/core.h>

OSSL_provider_init_fn OSSL_provider_init; /* Check the function signature */
int OSSL_provider_init(const OSSL_CORE_HANDLE *handle,
                       const OSSL_DISPATCH *oin,
                       const OSSL_DISPATCH **out,
                       void **provctx)
{
    return 1;
}
