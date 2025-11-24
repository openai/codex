/*
 * Copyright 2016 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Test CMS DER parsing.
 */

#include <openssl/bio.h>
#include <openssl/cms.h>
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
    CMS_ContentInfo *cms;
    BIO *in;

    if (len == 0)
        return 0;

    in = BIO_new(BIO_s_mem());
    OPENSSL_assert((size_t)BIO_write(in, buf, len) == len);
    cms = d2i_CMS_bio(in, NULL);
    if (cms != NULL) {
        BIO *out = BIO_new(BIO_s_null());

        i2d_CMS_bio(out, cms);
        BIO_free(out);
        CMS_ContentInfo_free(cms);
    }

    BIO_free(in);
    ERR_clear_error();

    return 0;
}

void FuzzerCleanup(void)
{
}
