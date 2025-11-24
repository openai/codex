/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Whirlpool low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/crypto.h>
#include <openssl/whrlpool.h>
#include "prov/digestcommon.h"
#include "prov/implementations.h"

/* ossl_wp_functions */
IMPLEMENT_digest_functions(wp, WHIRLPOOL_CTX,
                           WHIRLPOOL_BBLOCK / 8, WHIRLPOOL_DIGEST_LENGTH, 0,
                           WHIRLPOOL_Init, WHIRLPOOL_Update, WHIRLPOOL_Final)
