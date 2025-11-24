/*
 * Copyright 2012-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/e_os2.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "internal/nelem.h"
#include "fuzzer.h"

int FuzzerInitialize(int *argc, char ***argv)
{
    return 1;
}

int FuzzerTestOneInput(const uint8_t* data, size_t size){
    GENERAL_NAME *namesa;
    GENERAL_NAME *namesb;

    const unsigned char *derp = data;
    /*
     * We create two versions of each GENERAL_NAME so that we ensure when
     * we compare them they are always different pointers.
     */
    namesa = d2i_GENERAL_NAME(NULL, &derp, size);
    derp = data;
    namesb = d2i_GENERAL_NAME(NULL, &derp, size);
    GENERAL_NAME_cmp(namesa, namesb);
    if (namesa != NULL)
        GENERAL_NAME_free(namesa);
    if (namesb != NULL)
        GENERAL_NAME_free(namesb);
    return 0;
}

void FuzzerCleanup(void)
{
}
