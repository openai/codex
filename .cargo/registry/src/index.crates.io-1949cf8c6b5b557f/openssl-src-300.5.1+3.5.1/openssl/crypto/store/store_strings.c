/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/store.h>

#include "internal/nelem.h"

static char *type_strings[] = {
    "Name",                      /* OSSL_STORE_INFO_NAME */
    "Parameters",                /* OSSL_STORE_INFO_PARAMS */
    "Public key",                /* OSSL_STORE_INFO_PUBKEY */
    "Pkey",                      /* OSSL_STORE_INFO_PKEY */
    "Certificate",               /* OSSL_STORE_INFO_CERT */
    "CRL"                        /* OSSL_STORE_INFO_CRL */
};

const char *OSSL_STORE_INFO_type_string(int type)
{
    int types = OSSL_NELEM(type_strings);

    if (type < 1 || type > types)
        return NULL;

    return type_strings[type - 1];
}
