/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use the deprecated RSA low level calls */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/err.h>
#include <openssl/rsa.h>
#include <openssl/ssl.h>

int SSL_use_RSAPrivateKey(SSL *ssl, RSA *rsa)
{
    EVP_PKEY *pkey;
    int ret;

    if (rsa == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if ((pkey = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    if (!RSA_up_ref(rsa)) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_PKEY_assign_RSA(pkey, rsa) <= 0) {
        RSA_free(rsa);
        EVP_PKEY_free(pkey);
        return 0;
    }

    ret = SSL_use_PrivateKey(ssl, pkey);
    EVP_PKEY_free(pkey);
    return ret;
}

int SSL_use_RSAPrivateKey_file(SSL *ssl, const char *file, int type)
{
    int j, ret = 0;
    BIO *in = NULL;
    RSA *rsa = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        goto end;
    }

    in = BIO_new(BIO_s_file());
    if (in == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
        goto end;
    }

    if (BIO_read_filename(in, file) <= 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SYS_LIB);
        goto end;
    }
    if (type == SSL_FILETYPE_ASN1) {
        j = ERR_R_ASN1_LIB;
        rsa = d2i_RSAPrivateKey_bio(in, NULL);
    } else if (type == SSL_FILETYPE_PEM) {
        j = ERR_R_PEM_LIB;
        rsa = PEM_read_bio_RSAPrivateKey(in, NULL,
                                         SSL_get_default_passwd_cb(ssl),
                                         SSL_get_default_passwd_cb_userdata(ssl));
    } else {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_SSL_FILETYPE);
        goto end;
    }
    if (rsa == NULL) {
        ERR_raise(ERR_LIB_SSL, j);
        goto end;
    }
    ret = SSL_use_RSAPrivateKey(ssl, rsa);
    RSA_free(rsa);
 end:
    BIO_free(in);
    return ret;
}

int SSL_use_RSAPrivateKey_ASN1(SSL *ssl, const unsigned char *d, long len)
{
    int ret;
    const unsigned char *p;
    RSA *rsa;

    p = d;
    if ((rsa = d2i_RSAPrivateKey(NULL, &p, (long)len)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_ASN1_LIB);
        return 0;
    }

    ret = SSL_use_RSAPrivateKey(ssl, rsa);
    RSA_free(rsa);
    return ret;
}

int SSL_CTX_use_RSAPrivateKey(SSL_CTX *ctx, RSA *rsa)
{
    int ret;
    EVP_PKEY *pkey;

    if (rsa == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if ((pkey = EVP_PKEY_new()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    if (!RSA_up_ref(rsa)) {
        EVP_PKEY_free(pkey);
        return 0;
    }

    if (EVP_PKEY_assign_RSA(pkey, rsa) <= 0) {
        RSA_free(rsa);
        EVP_PKEY_free(pkey);
        return 0;
    }

    ret = SSL_CTX_use_PrivateKey(ctx, pkey);
    EVP_PKEY_free(pkey);
    return ret;
}

int SSL_CTX_use_RSAPrivateKey_file(SSL_CTX *ctx, const char *file, int type)
{
    int j, ret = 0;
    BIO *in = NULL;
    RSA *rsa = NULL;

    if (file == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        goto end;
    }

    in = BIO_new(BIO_s_file());
    if (in == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BUF_LIB);
        goto end;
    }

    if (BIO_read_filename(in, file) <= 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_SYS_LIB);
        goto end;
    }
    if (type == SSL_FILETYPE_ASN1) {
        j = ERR_R_ASN1_LIB;
        rsa = d2i_RSAPrivateKey_bio(in, NULL);
    } else if (type == SSL_FILETYPE_PEM) {
        j = ERR_R_PEM_LIB;
        rsa = PEM_read_bio_RSAPrivateKey(in, NULL,
                                         SSL_CTX_get_default_passwd_cb(ctx),
                                         SSL_CTX_get_default_passwd_cb_userdata(ctx));
    } else {
        ERR_raise(ERR_LIB_SSL, SSL_R_BAD_SSL_FILETYPE);
        goto end;
    }
    if (rsa == NULL) {
        ERR_raise(ERR_LIB_SSL, j);
        goto end;
    }
    ret = SSL_CTX_use_RSAPrivateKey(ctx, rsa);
    RSA_free(rsa);
 end:
    BIO_free(in);
    return ret;
}

int SSL_CTX_use_RSAPrivateKey_ASN1(SSL_CTX *ctx, const unsigned char *d,
                                   long len)
{
    int ret;
    const unsigned char *p;
    RSA *rsa;

    p = d;
    if ((rsa = d2i_RSAPrivateKey(NULL, &p, (long)len)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_ASN1_LIB);
        return 0;
    }

    ret = SSL_CTX_use_RSAPrivateKey(ctx, rsa);
    RSA_free(rsa);
    return ret;
}
