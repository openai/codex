/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/core_dispatch.h>
#include <openssl/buffer.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/pkcs12.h>
#include <openssl/pem.h>
#include <openssl/encoder.h>

static int do_pk8pkey(BIO *bp, const EVP_PKEY *x, int isder,
                      int nid, const EVP_CIPHER *enc,
                      const char *kstr, int klen,
                      pem_password_cb *cb, void *u,
                      const char *propq);

#ifndef OPENSSL_NO_STDIO
static int do_pk8pkey_fp(FILE *bp, const EVP_PKEY *x, int isder,
                         int nid, const EVP_CIPHER *enc,
                         const char *kstr, int klen,
                         pem_password_cb *cb, void *u,
                         const char *propq);
#endif
/*
 * These functions write a private key in PKCS#8 format: it is a "drop in"
 * replacement for PEM_write_bio_PrivateKey() and friends. As usual if 'enc'
 * is NULL then it uses the unencrypted private key form. The 'nid' versions
 * uses PKCS#5 v1.5 PBE algorithms whereas the others use PKCS#5 v2.0.
 */

int PEM_write_bio_PKCS8PrivateKey_nid(BIO *bp, const EVP_PKEY *x, int nid,
                                      const char *kstr, int klen,
                                      pem_password_cb *cb, void *u)
{
    return do_pk8pkey(bp, x, 0, nid, NULL, kstr, klen, cb, u, NULL);
}

int PEM_write_bio_PKCS8PrivateKey(BIO *bp, const EVP_PKEY *x, const EVP_CIPHER *enc,
                                  const char *kstr, int klen,
                                  pem_password_cb *cb, void *u)
{
    return do_pk8pkey(bp, x, 0, -1, enc, kstr, klen, cb, u, NULL);
}

int i2d_PKCS8PrivateKey_bio(BIO *bp, const EVP_PKEY *x, const EVP_CIPHER *enc,
                            const char *kstr, int klen,
                            pem_password_cb *cb, void *u)
{
    return do_pk8pkey(bp, x, 1, -1, enc, kstr, klen, cb, u, NULL);
}

int i2d_PKCS8PrivateKey_nid_bio(BIO *bp, const EVP_PKEY *x, int nid,
                                const char *kstr, int klen,
                                pem_password_cb *cb, void *u)
{
    return do_pk8pkey(bp, x, 1, nid, NULL, kstr, klen, cb, u, NULL);
}

static int do_pk8pkey(BIO *bp, const EVP_PKEY *x, int isder, int nid,
                      const EVP_CIPHER *enc, const char *kstr, int klen,
                      pem_password_cb *cb, void *u, const char *propq)
{
    int ret = 0;
    const char *outtype = isder ? "DER" : "PEM";
    OSSL_ENCODER_CTX *ctx =
        OSSL_ENCODER_CTX_new_for_pkey(x, OSSL_KEYMGMT_SELECT_ALL,
                                      outtype, "PrivateKeyInfo", propq);

    if (ctx == NULL)
        return 0;

    /*
     * If no keystring or callback is set, OpenSSL traditionally uses the
     * user's cb argument as a password string, or if that's NULL, it falls
     * back on PEM_def_callback().
     */
    if (kstr == NULL && cb == NULL) {
        if (u != NULL) {
            kstr = u;
            klen = strlen(u);
        } else {
            cb = PEM_def_callback;
        }
    }

    /*
     * NOTE: There is no attempt to do a EVP_CIPHER_fetch() using the nid,
     * since the nid is a PBE algorithm which can't be fetched currently.
     * (e.g. NID_pbe_WithSHA1And2_Key_TripleDES_CBC). Just use the legacy
     * path if the NID is passed.
     */
    if (nid == -1 && OSSL_ENCODER_CTX_get_num_encoders(ctx) != 0) {
        ret = 1;
        if (enc != NULL) {
            ret = 0;
            if (OSSL_ENCODER_CTX_set_cipher(ctx, EVP_CIPHER_get0_name(enc),
                                            NULL)) {
                const unsigned char *ukstr = (const unsigned char *)kstr;

                /*
                 * Try to pass the passphrase if one was given, or the
                 * passphrase callback if one was given.  If none of them
                 * are given and that's wrong, we rely on the _to_bio()
                 * call to generate errors.
                 */
                ret = 1;
                if (kstr != NULL
                    && !OSSL_ENCODER_CTX_set_passphrase(ctx, ukstr, klen))
                    ret = 0;
                else if (cb != NULL
                         && !OSSL_ENCODER_CTX_set_pem_password_cb(ctx, cb, u))
                    ret = 0;
            }
        }
        ret = ret && OSSL_ENCODER_to_bio(ctx, bp);
    } else {
        X509_SIG *p8;
        PKCS8_PRIV_KEY_INFO *p8inf;
        char buf[PEM_BUFSIZE];

        ret = 0;
        if ((p8inf = EVP_PKEY2PKCS8(x)) == NULL) {
            ERR_raise(ERR_LIB_PEM, PEM_R_ERROR_CONVERTING_PRIVATE_KEY);
            goto legacy_end;
        }
        if (enc || (nid != -1)) {
            if (kstr == NULL) {
                klen = cb(buf, PEM_BUFSIZE, 1, u);
                if (klen < 0) {
                    ERR_raise(ERR_LIB_PEM, PEM_R_READ_KEY);
                    goto legacy_end;
                }

                kstr = buf;
            }
            p8 = PKCS8_encrypt(nid, enc, kstr, klen, NULL, 0, 0, p8inf);
            if (kstr == buf)
                OPENSSL_cleanse(buf, klen);
            if (p8 == NULL)
                goto legacy_end;
            if (isder)
                ret = i2d_PKCS8_bio(bp, p8);
            else
                ret = PEM_write_bio_PKCS8(bp, p8);
            X509_SIG_free(p8);
        } else {
            if (isder)
                ret = i2d_PKCS8_PRIV_KEY_INFO_bio(bp, p8inf);
            else
                ret = PEM_write_bio_PKCS8_PRIV_KEY_INFO(bp, p8inf);
        }
     legacy_end:
        PKCS8_PRIV_KEY_INFO_free(p8inf);
    }
    OSSL_ENCODER_CTX_free(ctx);
    return ret;
}

EVP_PKEY *d2i_PKCS8PrivateKey_bio(BIO *bp, EVP_PKEY **x, pem_password_cb *cb,
                                  void *u)
{
    PKCS8_PRIV_KEY_INFO *p8inf = NULL;
    X509_SIG *p8 = NULL;
    int klen;
    EVP_PKEY *ret;
    char psbuf[PEM_BUFSIZE + 1]; /* reserve one byte at the end */

    p8 = d2i_PKCS8_bio(bp, NULL);
    if (p8 == NULL)
        return NULL;
    if (cb != NULL)
        klen = cb(psbuf, PEM_BUFSIZE, 0, u);
    else
        klen = PEM_def_callback(psbuf, PEM_BUFSIZE, 0, u);
    if (klen < 0 || klen > PEM_BUFSIZE) {
        ERR_raise(ERR_LIB_PEM, PEM_R_BAD_PASSWORD_READ);
        X509_SIG_free(p8);
        return NULL;
    }
    p8inf = PKCS8_decrypt(p8, psbuf, klen);
    X509_SIG_free(p8);
    OPENSSL_cleanse(psbuf, klen);
    if (p8inf == NULL)
        return NULL;
    ret = EVP_PKCS82PKEY(p8inf);
    PKCS8_PRIV_KEY_INFO_free(p8inf);
    if (!ret)
        return NULL;
    if (x != NULL) {
        EVP_PKEY_free(*x);
        *x = ret;
    }
    return ret;
}

#ifndef OPENSSL_NO_STDIO

int i2d_PKCS8PrivateKey_fp(FILE *fp, const EVP_PKEY *x, const EVP_CIPHER *enc,
                           const char *kstr, int klen,
                           pem_password_cb *cb, void *u)
{
    return do_pk8pkey_fp(fp, x, 1, -1, enc, kstr, klen, cb, u, NULL);
}

int i2d_PKCS8PrivateKey_nid_fp(FILE *fp, const EVP_PKEY *x, int nid,
                               const char *kstr, int klen,
                               pem_password_cb *cb, void *u)
{
    return do_pk8pkey_fp(fp, x, 1, nid, NULL, kstr, klen, cb, u, NULL);
}

int PEM_write_PKCS8PrivateKey_nid(FILE *fp, const EVP_PKEY *x, int nid,
                                  const char *kstr, int klen,
                                  pem_password_cb *cb, void *u)
{
    return do_pk8pkey_fp(fp, x, 0, nid, NULL, kstr, klen, cb, u, NULL);
}

int PEM_write_PKCS8PrivateKey(FILE *fp, const EVP_PKEY *x, const EVP_CIPHER *enc,
                              const char *kstr, int klen,
                              pem_password_cb *cb, void *u)
{
    return do_pk8pkey_fp(fp, x, 0, -1, enc, kstr, klen, cb, u, NULL);
}

static int do_pk8pkey_fp(FILE *fp, const EVP_PKEY *x, int isder, int nid,
                         const EVP_CIPHER *enc, const char *kstr, int klen,
                         pem_password_cb *cb, void *u, const char *propq)
{
    BIO *bp;
    int ret;

    if ((bp = BIO_new_fp(fp, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return 0;
    }
    ret = do_pk8pkey(bp, x, isder, nid, enc, kstr, klen, cb, u, propq);
    BIO_free(bp);
    return ret;
}

EVP_PKEY *d2i_PKCS8PrivateKey_fp(FILE *fp, EVP_PKEY **x, pem_password_cb *cb,
                                 void *u)
{
    BIO *bp;
    EVP_PKEY *ret;

    if ((bp = BIO_new_fp(fp, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return NULL;
    }
    ret = d2i_PKCS8PrivateKey_bio(bp, x, cb, u);
    BIO_free(bp);
    return ret;
}

#endif

IMPLEMENT_PEM_rw(PKCS8, X509_SIG, PEM_STRING_PKCS8, X509_SIG)


IMPLEMENT_PEM_rw(PKCS8_PRIV_KEY_INFO, PKCS8_PRIV_KEY_INFO, PEM_STRING_PKCS8INF,
                 PKCS8_PRIV_KEY_INFO)
