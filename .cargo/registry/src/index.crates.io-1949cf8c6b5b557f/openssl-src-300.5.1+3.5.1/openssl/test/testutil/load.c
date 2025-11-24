/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>

#include <openssl/x509.h>
#include <openssl/pem.h>

#include "../testutil.h"

X509 *load_cert_pem(const char *file, OSSL_LIB_CTX *libctx)
{
    X509 *cert = NULL;
    BIO *bio = NULL;

    if (!TEST_ptr(file) || !TEST_ptr(bio = BIO_new(BIO_s_file())))
        return NULL;
    if (TEST_int_gt(BIO_read_filename(bio, file), 0)
            && TEST_ptr(cert = X509_new_ex(libctx, NULL)))
        (void)TEST_ptr(cert = PEM_read_bio_X509(bio, &cert, NULL, NULL));

    BIO_free(bio);
    return cert;
}

STACK_OF(X509) *load_certs_pem(const char *file)
{
    STACK_OF(X509) *certs;
    BIO *bio;
    X509 *x;

    if (!TEST_ptr(file) || (bio = BIO_new_file(file, "r")) == NULL)
        return NULL;

    certs = sk_X509_new_null();
    if (certs == NULL) {
        BIO_free(bio);
        return NULL;
    }

    ERR_set_mark();
    do {
        x = PEM_read_bio_X509(bio, NULL, 0, NULL);
        if (x != NULL && !sk_X509_push(certs, x)) {
            OSSL_STACK_OF_X509_free(certs);
            BIO_free(bio);
            return NULL;
        } else if (x == NULL) {
            /*
             * We probably just ran out of certs, so ignore any errors
             * generated
             */
            ERR_pop_to_mark();
        }
    } while (x != NULL);

    BIO_free(bio);

    return certs;
}

EVP_PKEY *load_pkey_pem(const char *file, OSSL_LIB_CTX *libctx)
{
    EVP_PKEY *key = NULL;
    BIO *bio = NULL;

    if (!TEST_ptr(file) || !TEST_ptr(bio = BIO_new(BIO_s_file())))
        return NULL;
    if (TEST_int_gt(BIO_read_filename(bio, file), 0)) {
        unsigned long err = ERR_peek_error();

        if (TEST_ptr(key = PEM_read_bio_PrivateKey_ex(bio, NULL, NULL, NULL,
                                                      libctx, NULL))
            && err != ERR_peek_error()) {
            TEST_info("Spurious error from reading PEM");
            EVP_PKEY_free(key);
            key = NULL;
        }
    }

    BIO_free(bio);
    return key;
}

X509_REQ *load_csr_der(const char *file, OSSL_LIB_CTX *libctx)
{
    X509_REQ *csr = NULL;
    BIO *bio = NULL;

    if (!TEST_ptr(file) || !TEST_ptr(bio = BIO_new_file(file, "rb")))
        return NULL;

    csr = X509_REQ_new_ex(libctx, NULL);
    if (TEST_ptr(csr))
        (void)TEST_ptr(d2i_X509_REQ_bio(bio, &csr));
    BIO_free(bio);
    return csr;
}
