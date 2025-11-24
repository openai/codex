/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_tdes_default.h"

#define ossl_cipher_hw_tdes_wrap_initkey ossl_cipher_hw_tdes_ede3_initkey

PROV_CIPHER_HW_tdes_mode(wrap, cbc)
