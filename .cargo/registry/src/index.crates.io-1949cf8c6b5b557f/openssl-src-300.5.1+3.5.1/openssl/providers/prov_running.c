/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/e_os2.h>
#include "prov/providercommon.h"

/* By default, our providers don't have an error state */
void ossl_set_error_state(const char *type)
{
}

/* By default, our providers are always in a happy state */
int ossl_prov_is_running(void)
{
    return 1;
}
