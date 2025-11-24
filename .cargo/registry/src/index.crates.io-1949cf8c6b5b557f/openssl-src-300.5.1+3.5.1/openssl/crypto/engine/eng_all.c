/*
 * Copyright 2001-2018 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include "eng_local.h"

void ENGINE_load_builtin_engines(void)
{
    OPENSSL_init_crypto(OPENSSL_INIT_ENGINE_ALL_BUILTIN, NULL);
}

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
# if (defined(__OpenBSD__) || defined(__FreeBSD__) || defined(__DragonFly__))
void ENGINE_setup_bsd_cryptodev(void)
{
}
# endif
#endif
