/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
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
#include "prov/implementations.h"

/* desx_cbc_functions */
IMPLEMENT_tdes_cipher(desx, DESX, cbc, CBC, TDES_FLAGS, 64*3, 64, 64, block);

