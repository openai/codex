/*
 * Copyright 2011-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <openssl/err.h>

/*
 * Perform any essential OpenSSL initialization operations. Currently does
 * nothing.
 */

void OPENSSL_init(void)
{
    return;
}
