/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/decoder.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include "fuzzer.h"

static ASN1_PCTX *pctx;

int FuzzerInitialize(int *argc, char ***argv)
{
    FuzzerSetRand();

    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS
                        | OPENSSL_INIT_ADD_ALL_CIPHERS
                        | OPENSSL_INIT_ADD_ALL_DIGESTS, NULL);

    pctx = ASN1_PCTX_new();
    ASN1_PCTX_set_flags(pctx, ASN1_PCTX_FLAGS_SHOW_ABSENT
                              | ASN1_PCTX_FLAGS_SHOW_SEQUENCE
                              | ASN1_PCTX_FLAGS_SHOW_SSOF
                              | ASN1_PCTX_FLAGS_SHOW_TYPE
                              | ASN1_PCTX_FLAGS_SHOW_FIELD_STRUCT_NAME);
    ASN1_PCTX_set_str_flags(pctx, ASN1_STRFLGS_UTF8_CONVERT
                                  | ASN1_STRFLGS_SHOW_TYPE
                                  | ASN1_STRFLGS_DUMP_ALL);

    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    OSSL_DECODER_CTX *dctx;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    BIO *bio;

    bio = BIO_new(BIO_s_null());
    dctx = OSSL_DECODER_CTX_new_for_pkey(&pkey, NULL, NULL, NULL, 0, NULL,
                                                NULL);
    if (dctx == NULL) {
        return 0;
    }
    if (OSSL_DECODER_from_data(dctx, &buf, &len)) {
        EVP_PKEY *pkey2;

        EVP_PKEY_print_public(bio, pkey, 1, pctx);
        EVP_PKEY_print_private(bio, pkey, 1, pctx);
        EVP_PKEY_print_params(bio, pkey, 1, pctx);

        pkey2 = EVP_PKEY_dup(pkey);
        OPENSSL_assert(pkey2 != NULL);
        EVP_PKEY_eq(pkey, pkey2);
        EVP_PKEY_free(pkey2);

        ctx = EVP_PKEY_CTX_new(pkey, NULL);
        /*
         * Param check will take too long time on large DH parameters.
         * Skip it.
         */
        if ((!EVP_PKEY_is_a(pkey, "DH") && !EVP_PKEY_is_a(pkey, "DHX"))
            || EVP_PKEY_get_bits(pkey) <= 2048)
            EVP_PKEY_param_check(ctx);

        EVP_PKEY_public_check(ctx);
        /* Private and pairwise checks are unbounded, skip for large keys. */
        if (EVP_PKEY_get_bits(pkey) <= 4096) {
            EVP_PKEY_private_check(ctx);
            EVP_PKEY_pairwise_check(ctx);
        }
        OPENSSL_assert(ctx != NULL);
        EVP_PKEY_CTX_free(ctx);
        EVP_PKEY_free(pkey);
    }
    OSSL_DECODER_CTX_free(dctx);

    BIO_free(bio);
    ERR_clear_error();
    return 0;
}

void FuzzerCleanup(void)
{
    ASN1_PCTX_free(pctx);
    FuzzerClearRand();
}
