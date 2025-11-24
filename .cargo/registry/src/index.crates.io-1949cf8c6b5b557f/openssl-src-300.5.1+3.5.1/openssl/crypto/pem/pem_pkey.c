/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <openssl/buffer.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/pkcs12.h>
#include <openssl/pem.h>
#include <openssl/engine.h>
#include <openssl/dh.h>
#include <openssl/decoder.h>
#include <openssl/ui.h>
#include "internal/cryptlib.h"
#include "internal/passphrase.h"
#include "crypto/asn1.h"
#include "crypto/x509.h"
#include "crypto/evp.h"
#include "pem_local.h"

int ossl_pem_check_suffix(const char *pem_str, const char *suffix);

static EVP_PKEY *pem_read_bio_key_decoder(BIO *bp, EVP_PKEY **x,
                                          pem_password_cb *cb, void *u,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq,
                                          int selection)
{
    EVP_PKEY *pkey = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    int pos, newpos;

    if ((pos = BIO_tell(bp)) < 0)
        /* We can depend on BIO_tell() thanks to the BIO_f_readbuffer() */
        return NULL;

    dctx = OSSL_DECODER_CTX_new_for_pkey(&pkey, "PEM", NULL, NULL,
                                         selection, libctx, propq);

    if (dctx == NULL)
        return NULL;

    if (cb == NULL)
        cb = PEM_def_callback;

    if (!OSSL_DECODER_CTX_set_pem_password_cb(dctx, cb, u))
        goto err;

    ERR_set_mark();
    while (!OSSL_DECODER_from_bio(dctx, bp) || pkey == NULL)
        if (BIO_eof(bp) != 0 || (newpos = BIO_tell(bp)) < 0 || newpos <= pos) {
            ERR_clear_last_mark();
            goto err;
        } else {
            if (ERR_GET_REASON(ERR_peek_error()) == ERR_R_UNSUPPORTED) {
                /* unsupported PEM data, try again */
                ERR_pop_to_mark();
                ERR_set_mark();
            } else {
                /* other error, bail out */
                ERR_clear_last_mark();
                goto err;
            }
            pos = newpos;
        }
    ERR_pop_to_mark();

    /* if we were asked for private key, the public key is optional */
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        selection = selection & ~OSSL_KEYMGMT_SELECT_PUBLIC_KEY;

    if (!evp_keymgmt_util_has(pkey, selection)) {
        EVP_PKEY_free(pkey);
        pkey = NULL;
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_KEY_COMPONENTS);
        goto err;
    }

    if (x != NULL) {
        EVP_PKEY_free(*x);
        *x = pkey;
    }

 err:
    OSSL_DECODER_CTX_free(dctx);
    return pkey;
}

static EVP_PKEY *pem_read_bio_key_legacy(BIO *bp, EVP_PKEY **x,
                                         pem_password_cb *cb, void *u,
                                         OSSL_LIB_CTX *libctx,
                                         const char *propq,
                                         int selection)
{
    char *nm = NULL;
    const unsigned char *p = NULL;
    unsigned char *data = NULL;
    long len;
    int slen;
    EVP_PKEY *ret = NULL;

    ERR_set_mark();  /* not interested in PEM read errors */
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (!PEM_bytes_read_bio_secmem(&data, &len, &nm,
                                       PEM_STRING_EVP_PKEY,
                                       bp, cb, u)) {
            ERR_pop_to_mark();
            return NULL;
         }
    } else {
        const char *pem_string = PEM_STRING_PARAMETERS;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
            pem_string = PEM_STRING_PUBLIC;
        if (!PEM_bytes_read_bio(&data, &len, &nm,
                                pem_string,
                                bp, cb, u)) {
            ERR_pop_to_mark();
            return NULL;
        }
    }
    ERR_clear_last_mark();
    p = data;

    if (strcmp(nm, PEM_STRING_PKCS8INF) == 0) {
        PKCS8_PRIV_KEY_INFO *p8inf;

        if ((p8inf = d2i_PKCS8_PRIV_KEY_INFO(NULL, &p, len)) == NULL)
            goto p8err;
        ret = evp_pkcs82pkey_legacy(p8inf, libctx, propq);
        if (x != NULL) {
            EVP_PKEY_free(*x);
            *x = ret;
        }
        PKCS8_PRIV_KEY_INFO_free(p8inf);
    } else if (strcmp(nm, PEM_STRING_PKCS8) == 0) {
        PKCS8_PRIV_KEY_INFO *p8inf;
        X509_SIG *p8;
        int klen;
        char psbuf[PEM_BUFSIZE];

        if ((p8 = d2i_X509_SIG(NULL, &p, len)) == NULL)
            goto p8err;
        if (cb != NULL)
            klen = cb(psbuf, PEM_BUFSIZE, 0, u);
        else
            klen = PEM_def_callback(psbuf, PEM_BUFSIZE, 0, u);
        if (klen < 0) {
            ERR_raise(ERR_LIB_PEM, PEM_R_BAD_PASSWORD_READ);
            X509_SIG_free(p8);
            goto err;
        }
        p8inf = PKCS8_decrypt(p8, psbuf, klen);
        X509_SIG_free(p8);
        OPENSSL_cleanse(psbuf, klen);
        if (p8inf == NULL)
            goto p8err;
        ret = evp_pkcs82pkey_legacy(p8inf, libctx, propq);
        if (x != NULL) {
            EVP_PKEY_free(*x);
            *x = ret;
        }
        PKCS8_PRIV_KEY_INFO_free(p8inf);
    } else if ((slen = ossl_pem_check_suffix(nm, "PRIVATE KEY")) > 0) {
        const EVP_PKEY_ASN1_METHOD *ameth;
        ameth = EVP_PKEY_asn1_find_str(NULL, nm, slen);
        if (ameth == NULL || ameth->old_priv_decode == NULL)
            goto p8err;
        ret = ossl_d2i_PrivateKey_legacy(ameth->pkey_id, x, &p, len, libctx,
                                         propq);
    } else if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) == 0
               && (selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        /* Trying legacy PUBKEY decoding only if we do not want private key. */
        ret = ossl_d2i_PUBKEY_legacy(x, &p, len);
    } else if ((selection & EVP_PKEY_KEYPAIR) == 0
               && (slen = ossl_pem_check_suffix(nm, "PARAMETERS")) > 0) {
        /* Trying legacy params decoding only if we do not want a key. */
        ret = EVP_PKEY_new();
        if (ret == NULL)
            goto err;
        if (!EVP_PKEY_set_type_str(ret, nm, slen)
            || !ret->ameth->param_decode
            || !ret->ameth->param_decode(ret, &p, len)) {
            EVP_PKEY_free(ret);
            ret = NULL;
            goto err;
        }
        if (x) {
            EVP_PKEY_free(*x);
            *x = ret;
        }
    }

 p8err:
    if (ret == NULL && ERR_peek_last_error() == 0)
        /* ensure some error is reported but do not hide the real one */
        ERR_raise(ERR_LIB_PEM, ERR_R_ASN1_LIB);
 err:
    OPENSSL_secure_free(nm);
    OPENSSL_secure_clear_free(data, len);
    return ret;
}

static EVP_PKEY *pem_read_bio_key(BIO *bp, EVP_PKEY **x,
                                  pem_password_cb *cb, void *u,
                                  OSSL_LIB_CTX *libctx,
                                  const char *propq,
                                  int selection)
{
    EVP_PKEY *ret = NULL;
    BIO *new_bio = NULL;
    int pos;
    struct ossl_passphrase_data_st pwdata = { 0 };

    if ((pos = BIO_tell(bp)) < 0) {
        new_bio = BIO_new(BIO_f_readbuffer());
        if (new_bio == NULL)
            return NULL;
        bp = BIO_push(new_bio, bp);
        pos = BIO_tell(bp);
    }

    if (cb == NULL)
        cb = PEM_def_callback;

    if (!ossl_pw_set_pem_password_cb(&pwdata, cb, u)
        || !ossl_pw_enable_passphrase_caching(&pwdata))
        goto err;

    ERR_set_mark();
    ret = pem_read_bio_key_decoder(bp, x, ossl_pw_pem_password, &pwdata,
                                   libctx, propq, selection);
    if (ret == NULL
        && (BIO_seek(bp, pos) < 0
            || (ret = pem_read_bio_key_legacy(bp, x,
                                              ossl_pw_pem_password, &pwdata,
                                              libctx, propq,
                                              selection)) == NULL))
        ERR_clear_last_mark();
    else
        ERR_pop_to_mark();

 err:
    ossl_pw_clear_passphrase_data(&pwdata);
    if (new_bio != NULL) {
        BIO_pop(new_bio);
        BIO_free(new_bio);
    }
    return ret;
}

EVP_PKEY *PEM_read_bio_PUBKEY_ex(BIO *bp, EVP_PKEY **x,
                                 pem_password_cb *cb, void *u,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    return pem_read_bio_key(bp, x, cb, u, libctx, propq,
                            EVP_PKEY_PUBLIC_KEY);
}

EVP_PKEY *PEM_read_bio_PUBKEY(BIO *bp, EVP_PKEY **x, pem_password_cb *cb,
                              void *u)
{
    return PEM_read_bio_PUBKEY_ex(bp, x, cb, u, NULL, NULL);
}

#ifndef OPENSSL_NO_STDIO
EVP_PKEY *PEM_read_PUBKEY_ex(FILE *fp, EVP_PKEY **x,
                             pem_password_cb *cb, void *u,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    BIO *b;
    EVP_PKEY *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = PEM_read_bio_PUBKEY_ex(b, x, cb, u, libctx, propq);
    BIO_free(b);
    return ret;
}

EVP_PKEY *PEM_read_PUBKEY(FILE *fp, EVP_PKEY **x, pem_password_cb *cb, void *u)
{
    return PEM_read_PUBKEY_ex(fp, x, cb, u, NULL, NULL);
}
#endif

EVP_PKEY *PEM_read_bio_PrivateKey_ex(BIO *bp, EVP_PKEY **x,
                                     pem_password_cb *cb, void *u,
                                     OSSL_LIB_CTX *libctx, const char *propq)
{
    return pem_read_bio_key(bp, x, cb, u, libctx, propq,
                            /* we also want the public key, if available */
                            EVP_PKEY_KEYPAIR);
}

EVP_PKEY *PEM_read_bio_PrivateKey(BIO *bp, EVP_PKEY **x, pem_password_cb *cb,
                                  void *u)
{
    return PEM_read_bio_PrivateKey_ex(bp, x, cb, u, NULL, NULL);
}

PEM_write_cb_ex_fnsig(PrivateKey, EVP_PKEY, BIO, write_bio)
{
    IMPLEMENT_PEM_provided_write_body_vars(pkey, PrivateKey, propq);

    IMPLEMENT_PEM_provided_write_body_pass();
    IMPLEMENT_PEM_provided_write_body_main(pkey, bio);

 legacy:
    if (x != NULL && (x->ameth == NULL || x->ameth->priv_encode != NULL))
        return PEM_write_bio_PKCS8PrivateKey(out, x, enc,
                                             (const char *)kstr, klen, cb, u);
    return PEM_write_bio_PrivateKey_traditional(out, x, enc, kstr, klen, cb, u);
}

PEM_write_cb_fnsig(PrivateKey, EVP_PKEY, BIO, write_bio)
{
    return PEM_write_bio_PrivateKey_ex(out, x, enc, kstr, klen, cb, u,
                                       NULL, NULL);
}

/*
 * Note: there is no way to tell a provided pkey encoder to use "traditional"
 * encoding.  Therefore, if the pkey is provided, we try to take a copy
 */
int PEM_write_bio_PrivateKey_traditional(BIO *bp, const EVP_PKEY *x,
                                         const EVP_CIPHER *enc,
                                         const unsigned char *kstr, int klen,
                                         pem_password_cb *cb, void *u)
{
    char pem_str[80];
    EVP_PKEY *copy = NULL;
    int ret;

    if (x == NULL)
        return 0;

    if (evp_pkey_is_assigned(x)
        && evp_pkey_is_provided(x)
        && evp_pkey_copy_downgraded(&copy, x))
        x = copy;

    if (x->ameth == NULL || x->ameth->old_priv_encode == NULL) {
        ERR_raise(ERR_LIB_PEM, PEM_R_UNSUPPORTED_PUBLIC_KEY_TYPE);
        EVP_PKEY_free(copy);
        return 0;
    }
    BIO_snprintf(pem_str, 80, "%s PRIVATE KEY", x->ameth->pem_str);
    ret = PEM_ASN1_write_bio((i2d_of_void *)i2d_PrivateKey,
                             pem_str, bp, x, enc, kstr, klen, cb, u);

    EVP_PKEY_free(copy);
    return ret;
}

static int no_password_cb(char *buf, int num, int rwflag, void *userdata)
{
    return -1;
}

EVP_PKEY *PEM_read_bio_Parameters_ex(BIO *bp, EVP_PKEY **x,
                                     OSSL_LIB_CTX *libctx, const char *propq)
{
    /*
     * PEM_read_bio_Parameters(_ex) should never ask for a password. Any attempt
     * to get a password just fails.
     */
    return pem_read_bio_key(bp, x, no_password_cb, NULL, libctx, propq,
                            EVP_PKEY_KEY_PARAMETERS);
}

EVP_PKEY *PEM_read_bio_Parameters(BIO *bp, EVP_PKEY **x)
{
    return PEM_read_bio_Parameters_ex(bp, x, NULL, NULL);
}

PEM_write_fnsig(Parameters, EVP_PKEY, BIO, write_bio)
{
    char pem_str[80];
    IMPLEMENT_PEM_provided_write_body_vars(pkey, Parameters, NULL);

    IMPLEMENT_PEM_provided_write_body_main(pkey, bio);

 legacy:
    if (!x->ameth || !x->ameth->param_encode)
        return 0;

    BIO_snprintf(pem_str, 80, "%s PARAMETERS", x->ameth->pem_str);
    return PEM_ASN1_write_bio((i2d_of_void *)x->ameth->param_encode,
                              pem_str, out, x, NULL, NULL, 0, 0, NULL);
}

#ifndef OPENSSL_NO_STDIO
EVP_PKEY *PEM_read_PrivateKey_ex(FILE *fp, EVP_PKEY **x, pem_password_cb *cb,
                                 void *u, OSSL_LIB_CTX *libctx,
                                 const char *propq)
{
    BIO *b;
    EVP_PKEY *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = PEM_read_bio_PrivateKey_ex(b, x, cb, u, libctx, propq);
    BIO_free(b);
    return ret;
}

EVP_PKEY *PEM_read_PrivateKey(FILE *fp, EVP_PKEY **x, pem_password_cb *cb,
                              void *u)
{
    return PEM_read_PrivateKey_ex(fp, x, cb, u, NULL, NULL);
}

PEM_write_cb_ex_fnsig(PrivateKey, EVP_PKEY, FILE, write)
{
    BIO *b;
    int ret;

    if ((b = BIO_new_fp(out, BIO_NOCLOSE)) == NULL) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        return 0;
    }
    ret = PEM_write_bio_PrivateKey_ex(b, x, enc, kstr, klen, cb, u,
                                      libctx, propq);
    BIO_free(b);
    return ret;
}

PEM_write_cb_fnsig(PrivateKey, EVP_PKEY, FILE, write)
{
    return PEM_write_PrivateKey_ex(out, x, enc, kstr, klen, cb, u, NULL, NULL);
}
#endif
