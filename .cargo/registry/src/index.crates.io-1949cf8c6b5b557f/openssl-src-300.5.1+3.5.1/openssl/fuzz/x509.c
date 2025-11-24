/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/x509.h>
#include <openssl/ocsp.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include "fuzzer.h"

int FuzzerInitialize(int *argc, char ***argv)
{
    FuzzerSetRand();
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS
       | OPENSSL_INIT_ADD_ALL_CIPHERS | OPENSSL_INIT_ADD_ALL_DIGESTS, NULL);
    ERR_clear_error();
    CRYPTO_free_ex_index(0, -1);
    return 1;
}

static int cb(int ok, X509_STORE_CTX *ctx)
{
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    const unsigned char *p = buf;
    size_t orig_len = len;
    unsigned char *der = NULL;
    BIO *bio = NULL;
    X509 *x509_1 = NULL, *x509_2 = NULL;
    X509_STORE *store = NULL;
    X509_VERIFY_PARAM *param = NULL;
    X509_STORE_CTX *ctx = NULL;
    X509_CRL *crl = NULL;
    STACK_OF(X509_CRL) *crls = NULL;
    STACK_OF(X509) *certs = NULL;
    OCSP_RESPONSE *resp = NULL;
    OCSP_BASICRESP *bs = NULL;
    OCSP_CERTID *id = NULL;

    x509_1 = d2i_X509(NULL, &p, len);
    if (x509_1 == NULL)
        goto err;

    bio = BIO_new(BIO_s_null());
    if (bio == NULL)
        goto err;

    /* This will load and print the public key as well as extensions */
    X509_print(bio, x509_1);
    BIO_free(bio);

    X509_issuer_and_serial_hash(x509_1);

    i2d_X509(x509_1, &der);
    OPENSSL_free(der);

    len = orig_len - (p - buf);
    x509_2 = d2i_X509(NULL, &p, len);
    if (x509_2 == NULL)
        goto err;

    len = orig_len - (p - buf);
    crl = d2i_X509_CRL(NULL, &p, len);
    if (crl == NULL)
        goto err;

    len = orig_len - (p - buf);
    resp = d2i_OCSP_RESPONSE(NULL, &p, len);

    store = X509_STORE_new();
    if (store == NULL)
        goto err;
    X509_STORE_add_cert(store, x509_2);

    param = X509_VERIFY_PARAM_new();
    if (param == NULL)
        goto err;
    X509_VERIFY_PARAM_set_flags(param, X509_V_FLAG_NO_CHECK_TIME);
    X509_VERIFY_PARAM_set_flags(param, X509_V_FLAG_X509_STRICT);
    X509_VERIFY_PARAM_set_flags(param, X509_V_FLAG_PARTIAL_CHAIN);
    X509_VERIFY_PARAM_set_flags(param, X509_V_FLAG_CRL_CHECK);

    X509_STORE_set1_param(store, param);

    X509_STORE_set_verify_cb(store, cb);

    ctx = X509_STORE_CTX_new();
    if (ctx == NULL)
        goto err;

    X509_STORE_CTX_init(ctx, store, x509_1, NULL);

    if (crl != NULL) {
        crls = sk_X509_CRL_new_null();
        if (crls == NULL
            || !sk_X509_CRL_push(crls, crl))
            goto err;

        X509_STORE_CTX_set0_crls(ctx, crls);
    }

    X509_verify_cert(ctx);

    if (resp != NULL)
        bs = OCSP_response_get1_basic(resp);

    if (bs != NULL) {
        int status, reason;
        ASN1_GENERALIZEDTIME *revtime, *thisupd, *nextupd;

        certs = sk_X509_new_null();
        if (certs == NULL
            || !sk_X509_push(certs, x509_1)
            || !sk_X509_push(certs, x509_2))
            goto err;

        OCSP_basic_verify(bs, certs, store, OCSP_PARTIAL_CHAIN);

        id = OCSP_cert_to_id(NULL, x509_1, x509_2);
        if (id == NULL)
            goto err;
        OCSP_resp_find_status(bs, id, &status, &reason, &revtime, &thisupd,
                              &nextupd);
    }

err:
    X509_STORE_CTX_free(ctx);
    X509_VERIFY_PARAM_free(param);
    X509_STORE_free(store);
    X509_free(x509_1);
    X509_free(x509_2);
    X509_CRL_free(crl);
    OCSP_CERTID_free(id);
    OCSP_BASICRESP_free(bs);
    OCSP_RESPONSE_free(resp);
    sk_X509_CRL_free(crls);
    sk_X509_free(certs);

    ERR_clear_error();
    return 0;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
