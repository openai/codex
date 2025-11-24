/*
 * Copyright 2018 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/opensslv.h>
#include <openssl/crypto.h>

/* A simple helper for the perl function OpenSSL::Test::openssl_versions */
int main(void)
{
    printf("Build version: %s\n", OPENSSL_FULL_VERSION_STR);
    printf("Library version: %s\n",
           OpenSSL_version(OPENSSL_FULL_VERSION_STRING));
    return 0;
}
