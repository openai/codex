/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include "fuzzer.h"
#include <openssl/err.h>
#include <openssl/pkcs7.h>
#include <openssl/x509.h>
#include <stdio.h>

int FuzzerInitialize(int *argc, char ***argv)
{
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    BIO *b = BIO_new_mem_buf(buf, len);
    PKCS7 *p7 = SMIME_read_PKCS7(b, NULL);

    if (p7 != NULL) {
        STACK_OF(PKCS7_SIGNER_INFO) *p7si = PKCS7_get_signer_info(p7);
        int i;

        for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(p7si); i++) {
            STACK_OF(X509_ALGOR) *algs;

            PKCS7_cert_from_signer_info(p7,
                                        sk_PKCS7_SIGNER_INFO_value(p7si, i));
            algs = PKCS7_get_smimecap(sk_PKCS7_SIGNER_INFO_value(p7si, i));
            sk_X509_ALGOR_pop_free(algs, X509_ALGOR_free);
        }
        PKCS7_free(p7);
    }

    BIO_free(b);
    ERR_clear_error();
    return 0;
}

void FuzzerCleanup(void)
{
}
