/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include "crypto/evp.h"

#ifndef OPENSSL_NO_STDIO
STACK_OF(X509_INFO)
*PEM_X509_INFO_read_ex(FILE *fp, STACK_OF(X509_INFO) *sk, pem_password_cb *cb,
                       void *u, OSSL_LIB_CTX *libctx, const char *propq)
{
    BIO *b;
    STACK_OF(X509_INFO) *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = PEM_X509_INFO_read_bio_ex(b, sk, cb, u, libctx, propq);
    BIO_free(b);
    return ret;
}

STACK_OF(X509_INFO) *PEM_X509_INFO_read(FILE *fp, STACK_OF(X509_INFO) *sk,
                                        pem_password_cb *cb, void *u)
{
    return PEM_X509_INFO_read_ex(fp, sk, cb, u, NULL, NULL);
}
#endif

STACK_OF(X509_INFO) *PEM_X509_INFO_read_bio_ex(BIO *bp, STACK_OF(X509_INFO) *sk,
                                               pem_password_cb *cb, void *u,
                                               OSSL_LIB_CTX *libctx,
                                               const char *propq)
{
    X509_INFO *xi = NULL;
    char *name = NULL, *header = NULL, *str;
    void *pp;
    unsigned char *data = NULL;
    const unsigned char *p;
    long len, error = 0;
    int ok = 0;
    STACK_OF(X509_INFO) *ret = NULL;
    unsigned int i, raw, ptype;
    d2i_of_void *d2i = 0;

    if (sk == NULL) {
        if ((ret = sk_X509_INFO_new_null()) == NULL) {
            ERR_raise(ERR_LIB_PEM, ERR_R_CRYPTO_LIB);
            goto err;
        }
    } else
        ret = sk;

    if ((xi = X509_INFO_new()) == NULL)
        goto err;
    for (;;) {
        raw = 0;
        ptype = 0;
        ERR_set_mark();
        i = PEM_read_bio(bp, &name, &header, &data, &len);
        if (i == 0) {
            error = ERR_GET_REASON(ERR_peek_last_error());
            if (error == PEM_R_NO_START_LINE) {
                ERR_pop_to_mark();
                break;
            }
            ERR_clear_last_mark();
            goto err;
        }
        ERR_clear_last_mark();
 start:
        if (strcmp(name, PEM_STRING_X509) == 0
                || strcmp(name, PEM_STRING_X509_OLD) == 0
                || strcmp(name, PEM_STRING_X509_TRUSTED) == 0) {
            if (xi->x509 != NULL) {
                if (!sk_X509_INFO_push(ret, xi))
                    goto err;
                if ((xi = X509_INFO_new()) == NULL)
                    goto err;
                goto start;
            }
            if ((strcmp(name, PEM_STRING_X509_TRUSTED) == 0))
                d2i = (D2I_OF(void)) d2i_X509_AUX;
            else
                d2i = (D2I_OF(void)) d2i_X509;
            xi->x509 = X509_new_ex(libctx, propq);
            if (xi->x509 == NULL)
                goto err;
            pp = &(xi->x509);
        } else if (strcmp(name, PEM_STRING_X509_CRL) == 0) {
            d2i = (D2I_OF(void)) d2i_X509_CRL;
            if (xi->crl != NULL) {
                if (!sk_X509_INFO_push(ret, xi))
                    goto err;
                if ((xi = X509_INFO_new()) == NULL)
                    goto err;
                goto start;
            }
            pp = &(xi->crl);
        } else if ((str = strstr(name, PEM_STRING_PKCS8INF)) != NULL) {
            if (xi->x_pkey != NULL) {
                if (!sk_X509_INFO_push(ret, xi))
                    goto err;
                if ((xi = X509_INFO_new()) == NULL)
                    goto err;
                goto start;
            }
            if (str == name || strcmp(name, PEM_STRING_PKCS8) == 0) {
                ptype = EVP_PKEY_NONE;
            } else {
                /* chop " PRIVATE KEY" */
                *--str = '\0';
                ptype = evp_pkey_name2type(name);
            }
            xi->enc_data = NULL;
            xi->enc_len = 0;

            d2i = (D2I_OF(void)) d2i_AutoPrivateKey;
            xi->x_pkey = X509_PKEY_new();
            if (xi->x_pkey == NULL)
                goto err;
            pp = &xi->x_pkey->dec_pkey;
            if ((int)strlen(header) > 10 /* assume encrypted */
                   || strcmp(name, PEM_STRING_PKCS8) == 0)
                raw = 1;
        } else { /* unknown */
            d2i = NULL;
            pp = NULL;
        }

        if (d2i != NULL) {
            if (!raw) {
                EVP_CIPHER_INFO cipher;

                if (!PEM_get_EVP_CIPHER_INFO(header, &cipher))
                    goto err;
                if (!PEM_do_header(&cipher, data, &len, cb, u))
                    goto err;
                p = data;
                if (ptype) {
                    if (d2i_PrivateKey_ex(ptype, pp, &p, len,
                                          libctx, propq) == NULL) {
                        ERR_raise(ERR_LIB_PEM, ERR_R_ASN1_LIB);
                        goto err;
                    }
                } else if (d2i(pp, &p, len) == NULL) {
                    ERR_raise(ERR_LIB_PEM, ERR_R_ASN1_LIB);
                    goto err;
                }
            } else {            /* encrypted key data */
                if (!PEM_get_EVP_CIPHER_INFO(header, &xi->enc_cipher))
                    goto err;
                xi->enc_data = (char *)data;
                xi->enc_len = (int)len;
                data = NULL;
            }
        }
        OPENSSL_free(name);
        name = NULL;
        OPENSSL_free(header);
        header = NULL;
        OPENSSL_free(data);
        data = NULL;
    }

    /*
     * if the last one hasn't been pushed yet and there is anything in it
     * then add it to the stack ...
     */
    if ((xi->x509 != NULL) || (xi->crl != NULL) ||
        (xi->x_pkey != NULL) || (xi->enc_data != NULL)) {
        if (!sk_X509_INFO_push(ret, xi))
            goto err;
        xi = NULL;
    }
    ok = 1;
 err:
    X509_INFO_free(xi);
    if (!ok) {
        for (i = 0; ((int)i) < sk_X509_INFO_num(ret); i++) {
            xi = sk_X509_INFO_value(ret, i);
            X509_INFO_free(xi);
        }
        if (ret != sk)
            sk_X509_INFO_free(ret);
        ret = NULL;
    }

    OPENSSL_free(name);
    OPENSSL_free(header);
    OPENSSL_free(data);
    return ret;
}

STACK_OF(X509_INFO) *PEM_X509_INFO_read_bio(BIO *bp, STACK_OF(X509_INFO) *sk,
                                            pem_password_cb *cb, void *u)
{
    return PEM_X509_INFO_read_bio_ex(bp, sk, cb, u, NULL, NULL);
}

/* A TJH addition */
int PEM_X509_INFO_write_bio(BIO *bp, const X509_INFO *xi, EVP_CIPHER *enc,
                            const unsigned char *kstr, int klen,
                            pem_password_cb *cb, void *u)
{
    int i, ret = 0;
    unsigned char *data = NULL;
    const char *objstr = NULL;
    char buf[PEM_BUFSIZE];
    const unsigned char *iv = NULL;

    if (enc != NULL) {
        objstr = EVP_CIPHER_get0_name(enc);
        if (objstr == NULL
               /*
                * Check "Proc-Type: 4,Encrypted\nDEK-Info: objstr,hex-iv\n"
                * fits into buf
                */
            || strlen(objstr) + 23 + 2 * EVP_CIPHER_get_iv_length(enc) + 13
               > sizeof(buf)) {
            ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_CIPHER);
            goto err;
        }
    }

    /*
     * now for the fun part ... if we have a private key then we have to be
     * able to handle a not-yet-decrypted key being written out correctly ...
     * if it is decrypted or it is non-encrypted then we use the base code
     */
    if (xi->x_pkey != NULL) {
        if ((xi->enc_data != NULL) && (xi->enc_len > 0)) {
            if (enc == NULL) {
                ERR_raise(ERR_LIB_PEM, PEM_R_CIPHER_IS_NULL);
                goto err;
            }

            /* copy from weirdo names into more normal things */
            iv = xi->enc_cipher.iv;
            data = (unsigned char *)xi->enc_data;
            i = xi->enc_len;

            /*
             * we take the encryption data from the internal stuff rather
             * than what the user has passed us ... as we have to match
             * exactly for some strange reason
             */
            objstr = EVP_CIPHER_get0_name(xi->enc_cipher.cipher);
            if (objstr == NULL) {
                ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_CIPHER);
                goto err;
            }

            /* Create the right magic header stuff */
            buf[0] = '\0';
            PEM_proc_type(buf, PEM_TYPE_ENCRYPTED);
            PEM_dek_info(buf, objstr, EVP_CIPHER_get_iv_length(enc),
                         (const char *)iv);

            /* use the normal code to write things out */
            i = PEM_write_bio(bp, PEM_STRING_RSA, buf, data, i);
            if (i <= 0)
                goto err;
        } else {
            /* Add DSA/DH */
            /* normal optionally encrypted stuff */
            if (PEM_write_bio_RSAPrivateKey(bp,
                                            EVP_PKEY_get0_RSA(xi->x_pkey->dec_pkey),
                                            enc, kstr, klen, cb, u) <= 0)
                goto err;
        }
    }

    /* if we have a certificate then write it out now */
    if ((xi->x509 != NULL) && (PEM_write_bio_X509(bp, xi->x509) <= 0))
        goto err;

    /*
     * we are ignoring anything else that is loaded into the X509_INFO
     * structure for the moment ... as I don't need it so I'm not coding it
     * here and Eric can do it when this makes it into the base library --tjh
     */

    ret = 1;

 err:
    OPENSSL_cleanse(buf, PEM_BUFSIZE);
    return ret;
}
