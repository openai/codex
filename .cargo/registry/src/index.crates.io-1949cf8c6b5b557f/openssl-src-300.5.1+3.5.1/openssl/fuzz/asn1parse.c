/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Fuzz the parser used for dumping ASN.1 using "openssl asn1parse".
 */

#include <stdio.h>
#include <openssl/asn1.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include "fuzzer.h"

static BIO *bio_out;

int FuzzerInitialize(int *argc, char ***argv)
{
    bio_out = BIO_new(BIO_s_null()); /* output will be ignored */
    if (bio_out == NULL)
        return 0;
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    (void)ASN1_parse_dump(bio_out, buf, len, 0, 0);
    ERR_clear_error();
    return 0;
}

void FuzzerCleanup(void)
{
    BIO_free(bio_out);
}
