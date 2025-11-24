/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/pem.h>
#include <openssl/err.h>
#include "fuzzer.h"

int FuzzerInitialize(int *argc, char ***argv)
{
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    BIO *in;
    char *name = NULL, *header = NULL;
    unsigned char *data = NULL;
    long outlen;

    if (len <= 1)
        return 0;

    in = BIO_new(BIO_s_mem());
    OPENSSL_assert((size_t)BIO_write(in, buf + 1, len - 1) == len - 1);
    if (PEM_read_bio_ex(in, &name, &header, &data, &outlen, buf[0]) == 1) {
	/* Try to read all the data we get to see if allocated properly. */
        BIO_write(in, name, strlen(name));
	BIO_write(in, header, strlen(header));
	BIO_write(in, data, outlen);
    }
    if (buf[0] & PEM_FLAG_SECURE) {
        OPENSSL_secure_free(name);
        OPENSSL_secure_free(header);
        OPENSSL_secure_free(data);
    } else {
        OPENSSL_free(name);
        OPENSSL_free(header);
        OPENSSL_free(data);
    }

    BIO_free(in);
    ERR_clear_error();

    return 0;
}

void FuzzerCleanup(void)
{
}
