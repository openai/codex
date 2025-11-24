/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/rand.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include "internal/cryptlib.h"
#include "internal/sizes.h"
#include "crypto/evp.h"
#include "pk7_local.h"

static int add_attribute(STACK_OF(X509_ATTRIBUTE) **sk, int nid, int atrtype,
                         void *value);
static ASN1_TYPE *get_attribute(const STACK_OF(X509_ATTRIBUTE) *sk, int nid);

int PKCS7_type_is_other(PKCS7 *p7)
{
    int isOther = 1;

    int nid = OBJ_obj2nid(p7->type);

    switch (nid) {
    case NID_pkcs7_data:
    case NID_pkcs7_signed:
    case NID_pkcs7_enveloped:
    case NID_pkcs7_signedAndEnveloped:
    case NID_pkcs7_digest:
    case NID_pkcs7_encrypted:
        isOther = 0;
        break;
    default:
        isOther = 1;
    }

    return isOther;

}

ASN1_OCTET_STRING *PKCS7_get_octet_string(PKCS7 *p7)
{
    if (PKCS7_type_is_data(p7))
        return p7->d.data;
    if (PKCS7_type_is_other(p7) && p7->d.other
        && (p7->d.other->type == V_ASN1_OCTET_STRING))
        return p7->d.other->value.octet_string;
    return NULL;
}

static ASN1_OCTET_STRING *pkcs7_get1_data(PKCS7 *p7)
{
    ASN1_OCTET_STRING *os = PKCS7_get_octet_string(p7);

    if (os != NULL) {
        /* Edge case for MIME content, see RFC 5652 section-5.2.1 */
        ASN1_OCTET_STRING *osdup = ASN1_OCTET_STRING_dup(os);

        if (osdup != NULL && (os->flags & ASN1_STRING_FLAG_NDEF))
            /* ASN1_STRING_FLAG_NDEF flag is currently used by openssl-smime */
            ASN1_STRING_set0(osdup, NULL, 0);
        return osdup;
    }

    /* General case for PKCS#7 content, see RFC 2315 section-7 */
    if (PKCS7_type_is_other(p7) && (p7->d.other != NULL)
            && (p7->d.other->type == V_ASN1_SEQUENCE)
            && (p7->d.other->value.sequence != NULL)
            && (p7->d.other->value.sequence->length > 0)) {
        const unsigned char *data = p7->d.other->value.sequence->data;
        long len;
        int inf, tag, class;

        os = ASN1_OCTET_STRING_new();
        if (os == NULL)
            return NULL;
        inf = ASN1_get_object(&data, &len, &tag, &class,
                              p7->d.other->value.sequence->length);
        if (inf != V_ASN1_CONSTRUCTED || tag != V_ASN1_SEQUENCE
                || !ASN1_OCTET_STRING_set(os, data, len)) {
            ASN1_OCTET_STRING_free(os);
            os = NULL;
        }
    }
    return os;
}

static int pkcs7_bio_add_digest(BIO **pbio, X509_ALGOR *alg,
                                const PKCS7_CTX *ctx)
{
    BIO *btmp;
    char name[OSSL_MAX_NAME_SIZE];
    EVP_MD *fetched = NULL;
    const EVP_MD *md;

    if ((btmp = BIO_new(BIO_f_md())) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
        goto err;
    }

    OBJ_obj2txt(name, sizeof(name), alg->algorithm, 0);

    (void)ERR_set_mark();
    fetched = EVP_MD_fetch(ossl_pkcs7_ctx_get0_libctx(ctx), name,
                           ossl_pkcs7_ctx_get0_propq(ctx));
    if (fetched != NULL)
        md = fetched;
    else
        md = EVP_get_digestbyname(name);

    if (md == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNKNOWN_DIGEST_TYPE);
        goto err;
    }
    (void)ERR_pop_to_mark();

    if (BIO_set_md(btmp, md) <= 0) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
        EVP_MD_free(fetched);
        goto err;
    }
    EVP_MD_free(fetched);
    if (*pbio == NULL)
        *pbio = btmp;
    else if (!BIO_push(*pbio, btmp)) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
        goto err;
    }
    btmp = NULL;

    return 1;

 err:
    BIO_free(btmp);
    return 0;
}

static int pkcs7_encode_rinfo(PKCS7_RECIP_INFO *ri,
                              unsigned char *key, int keylen)
{
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *pkey = NULL;
    unsigned char *ek = NULL;
    int ret = 0;
    size_t eklen;
    const PKCS7_CTX *ctx = ri->ctx;

    pkey = X509_get0_pubkey(ri->cert);
    if (pkey == NULL)
        return 0;

    pctx = EVP_PKEY_CTX_new_from_pkey(ossl_pkcs7_ctx_get0_libctx(ctx), pkey,
                                      ossl_pkcs7_ctx_get0_propq(ctx));
    if (pctx == NULL)
        return 0;

    if (EVP_PKEY_encrypt_init(pctx) <= 0)
        goto err;

    if (EVP_PKEY_encrypt(pctx, NULL, &eklen, key, keylen) <= 0)
        goto err;

    ek = OPENSSL_malloc(eklen);
    if (ek == NULL)
        goto err;

    if (EVP_PKEY_encrypt(pctx, ek, &eklen, key, keylen) <= 0)
        goto err;

    ASN1_STRING_set0(ri->enc_key, ek, eklen);
    ek = NULL;

    ret = 1;

 err:
    EVP_PKEY_CTX_free(pctx);
    OPENSSL_free(ek);
    return ret;

}

static int pkcs7_decrypt_rinfo(unsigned char **pek, int *peklen,
                               PKCS7_RECIP_INFO *ri, EVP_PKEY *pkey,
                               size_t fixlen)
{
    EVP_PKEY_CTX *pctx = NULL;
    unsigned char *ek = NULL;
    size_t eklen;
    int ret = -1;
    const PKCS7_CTX *ctx = ri->ctx;

    pctx = EVP_PKEY_CTX_new_from_pkey(ossl_pkcs7_ctx_get0_libctx(ctx), pkey,
                                      ossl_pkcs7_ctx_get0_propq(ctx));
    if (pctx == NULL)
        return -1;

    if (EVP_PKEY_decrypt_init(pctx) <= 0)
        goto err;

    if (EVP_PKEY_is_a(pkey, "RSA"))
        /* upper layer pkcs7 code incorrectly assumes that a successful RSA
         * decryption means that the key matches ciphertext (which never
         * was the case, implicit rejection or not), so to make it work
         * disable implicit rejection for RSA keys */
        EVP_PKEY_CTX_ctrl_str(pctx, "rsa_pkcs1_implicit_rejection", "0");

    ret = evp_pkey_decrypt_alloc(pctx, &ek, &eklen, fixlen,
                                 ri->enc_key->data, ri->enc_key->length);
    if (ret <= 0)
        goto err;

    ret = 1;

    OPENSSL_clear_free(*pek, *peklen);
    *pek = ek;
    *peklen = eklen;

 err:
    EVP_PKEY_CTX_free(pctx);
    if (!ret)
        OPENSSL_free(ek);

    return ret;
}

BIO *PKCS7_dataInit(PKCS7 *p7, BIO *bio)
{
    int i;
    BIO *out = NULL, *btmp = NULL;
    X509_ALGOR *xa = NULL;
    EVP_CIPHER *fetched_cipher = NULL;
    const EVP_CIPHER *cipher;
    const EVP_CIPHER *evp_cipher = NULL;
    STACK_OF(X509_ALGOR) *md_sk = NULL;
    STACK_OF(PKCS7_RECIP_INFO) *rsk = NULL;
    X509_ALGOR *xalg = NULL;
    PKCS7_RECIP_INFO *ri = NULL;
    ASN1_OCTET_STRING *os = NULL;
    const PKCS7_CTX *p7_ctx;
    OSSL_LIB_CTX *libctx;
    const char *propq;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return NULL;
    }
    p7_ctx = ossl_pkcs7_get0_ctx(p7);
    libctx = ossl_pkcs7_ctx_get0_libctx(p7_ctx);
    propq = ossl_pkcs7_ctx_get0_propq(p7_ctx);

    /*
     * The content field in the PKCS7 ContentInfo is optional, but that really
     * only applies to inner content (precisely, detached signatures).
     *
     * When reading content, missing outer content is therefore treated as an
     * error.
     *
     * When creating content, PKCS7_content_new() must be called before
     * calling this method, so a NULL p7->d is always an error.
     */
    if (p7->d.ptr == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        return NULL;
    }

    i = OBJ_obj2nid(p7->type);
    p7->state = PKCS7_S_HEADER;

    switch (i) {
    case NID_pkcs7_signed:
        md_sk = p7->d.sign->md_algs;
        os = pkcs7_get1_data(p7->d.sign->contents);
        break;
    case NID_pkcs7_signedAndEnveloped:
        rsk = p7->d.signed_and_enveloped->recipientinfo;
        md_sk = p7->d.signed_and_enveloped->md_algs;
        xalg = p7->d.signed_and_enveloped->enc_data->algorithm;
        evp_cipher = p7->d.signed_and_enveloped->enc_data->cipher;
        if (evp_cipher == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_CIPHER_NOT_INITIALIZED);
            goto err;
        }
        break;
    case NID_pkcs7_enveloped:
        rsk = p7->d.enveloped->recipientinfo;
        xalg = p7->d.enveloped->enc_data->algorithm;
        evp_cipher = p7->d.enveloped->enc_data->cipher;
        if (evp_cipher == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_CIPHER_NOT_INITIALIZED);
            goto err;
        }
        break;
    case NID_pkcs7_digest:
        xa = p7->d.digest->md;
        os = pkcs7_get1_data(p7->d.digest->contents);
        break;
    case NID_pkcs7_data:
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CONTENT_TYPE);
        goto err;
    }

    for (i = 0; i < sk_X509_ALGOR_num(md_sk); i++)
        if (!pkcs7_bio_add_digest(&out, sk_X509_ALGOR_value(md_sk, i), p7_ctx))
            goto err;

    if (xa && !pkcs7_bio_add_digest(&out, xa, p7_ctx))
        goto err;

    if (evp_cipher != NULL) {
        unsigned char key[EVP_MAX_KEY_LENGTH];
        unsigned char iv[EVP_MAX_IV_LENGTH];
        int keylen, ivlen;
        EVP_CIPHER_CTX *ctx;

        if ((btmp = BIO_new(BIO_f_cipher())) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
            goto err;
        }
        BIO_get_cipher_ctx(btmp, &ctx);
        keylen = EVP_CIPHER_get_key_length(evp_cipher);
        ivlen = EVP_CIPHER_get_iv_length(evp_cipher);
        xalg->algorithm = OBJ_nid2obj(EVP_CIPHER_get_type(evp_cipher));
        if (ivlen > 0)
            if (RAND_bytes_ex(libctx, iv, ivlen, 0) <= 0)
                goto err;

        (void)ERR_set_mark();
        fetched_cipher = EVP_CIPHER_fetch(libctx,
                                          EVP_CIPHER_get0_name(evp_cipher),
                                          propq);
        (void)ERR_pop_to_mark();
        if (fetched_cipher != NULL)
            cipher = fetched_cipher;
        else
            cipher = evp_cipher;

        if (EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, 1) <= 0)
            goto err;

        EVP_CIPHER_free(fetched_cipher);
        fetched_cipher = NULL;

        if (EVP_CIPHER_CTX_rand_key(ctx, key) <= 0)
            goto err;
        if (EVP_CipherInit_ex(ctx, NULL, NULL, key, iv, 1) <= 0)
            goto err;

        if (ivlen > 0) {
            if (xalg->parameter == NULL) {
                xalg->parameter = ASN1_TYPE_new();
                if (xalg->parameter == NULL)
                    goto err;
            }
            if (EVP_CIPHER_param_to_asn1(ctx, xalg->parameter) <= 0)
                goto err;
        }

        /* Lets do the pub key stuff :-) */
        for (i = 0; i < sk_PKCS7_RECIP_INFO_num(rsk); i++) {
            ri = sk_PKCS7_RECIP_INFO_value(rsk, i);
            if (pkcs7_encode_rinfo(ri, key, keylen) <= 0)
                goto err;
        }
        OPENSSL_cleanse(key, keylen);

        if (out == NULL)
            out = btmp;
        else
            BIO_push(out, btmp);
        btmp = NULL;
    }

    if (bio == NULL) {
        if (PKCS7_is_detached(p7)) {
            bio = BIO_new(BIO_s_null());
        } else if (os != NULL && os->length > 0) {
            /*
             * bio needs a copy of os->data instead of a pointer because
             * the data will be used after os has been freed
             */
            bio = BIO_new(BIO_s_mem());
            if (bio != NULL) {
                BIO_set_mem_eof_return(bio, 0);
                if (BIO_write(bio, os->data, os->length) != os->length) {
                    BIO_free_all(bio);
                    bio = NULL;
                }
            }
        } else {
            bio = BIO_new(BIO_s_mem());
            if (bio == NULL)
                goto err;
            BIO_set_mem_eof_return(bio, 0);
        }
        if (bio == NULL)
            goto err;
    }
    if (out)
        BIO_push(out, bio);
    else
        out = bio;

    ASN1_OCTET_STRING_free(os);
    return out;

 err:
    ASN1_OCTET_STRING_free(os);
    EVP_CIPHER_free(fetched_cipher);
    BIO_free_all(out);
    BIO_free_all(btmp);
    return NULL;
}

static int pkcs7_cmp_ri(PKCS7_RECIP_INFO *ri, X509 *pcert)
{
    int ret;
    ret = X509_NAME_cmp(ri->issuer_and_serial->issuer,
                        X509_get_issuer_name(pcert));
    if (ret)
        return ret;
    return ASN1_INTEGER_cmp(X509_get0_serialNumber(pcert),
                            ri->issuer_and_serial->serial);
}

/* int */
BIO *PKCS7_dataDecode(PKCS7 *p7, EVP_PKEY *pkey, BIO *in_bio, X509 *pcert)
{
    int i, len;
    BIO *out = NULL, *btmp = NULL, *etmp = NULL, *bio = NULL;
    X509_ALGOR *xa;
    ASN1_OCTET_STRING *data_body = NULL;
    EVP_MD *evp_md = NULL;
    const EVP_MD *md;
    EVP_CIPHER *evp_cipher = NULL;
    const EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *evp_ctx = NULL;
    X509_ALGOR *enc_alg = NULL;
    STACK_OF(X509_ALGOR) *md_sk = NULL;
    STACK_OF(PKCS7_RECIP_INFO) *rsk = NULL;
    PKCS7_RECIP_INFO *ri = NULL;
    unsigned char *ek = NULL, *tkey = NULL;
    int eklen = 0, tkeylen = 0;
    char name[OSSL_MAX_NAME_SIZE];
    const PKCS7_CTX *p7_ctx;
    OSSL_LIB_CTX *libctx;
    const char *propq;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return NULL;
    }

    p7_ctx = ossl_pkcs7_get0_ctx(p7);
    libctx = ossl_pkcs7_ctx_get0_libctx(p7_ctx);
    propq = ossl_pkcs7_ctx_get0_propq(p7_ctx);

    if (p7->d.ptr == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        return NULL;
    }

    i = OBJ_obj2nid(p7->type);
    p7->state = PKCS7_S_HEADER;

    switch (i) {
    case NID_pkcs7_signed:
        /*
         * p7->d.sign->contents is a PKCS7 structure consisting of a contentType
         * field and optional content.
         * data_body is NULL if that structure has no (=detached) content
         * or if the contentType is wrong (i.e., not "data").
         */
        data_body = PKCS7_get_octet_string(p7->d.sign->contents);
        if (!PKCS7_is_detached(p7) && data_body == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_SIGNED_DATA_TYPE);
            goto err;
        }
        md_sk = p7->d.sign->md_algs;
        break;
    case NID_pkcs7_signedAndEnveloped:
        rsk = p7->d.signed_and_enveloped->recipientinfo;
        md_sk = p7->d.signed_and_enveloped->md_algs;
        /* data_body is NULL if the optional EncryptedContent is missing. */
        data_body = p7->d.signed_and_enveloped->enc_data->enc_data;
        enc_alg = p7->d.signed_and_enveloped->enc_data->algorithm;

        OBJ_obj2txt(name, sizeof(name), enc_alg->algorithm, 0);

        (void)ERR_set_mark();
        evp_cipher = EVP_CIPHER_fetch(libctx, name, propq);
        if (evp_cipher != NULL)
            cipher = evp_cipher;
        else
            cipher = EVP_get_cipherbyname(name);

        if (cipher == NULL) {
            (void)ERR_clear_last_mark();
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CIPHER_TYPE);
            goto err;
        }
        (void)ERR_pop_to_mark();
        break;
    case NID_pkcs7_enveloped:
        rsk = p7->d.enveloped->recipientinfo;
        enc_alg = p7->d.enveloped->enc_data->algorithm;
        /* data_body is NULL if the optional EncryptedContent is missing. */
        data_body = p7->d.enveloped->enc_data->enc_data;
        OBJ_obj2txt(name, sizeof(name), enc_alg->algorithm, 0);

        (void)ERR_set_mark();
        evp_cipher = EVP_CIPHER_fetch(libctx, name, propq);
        if (evp_cipher != NULL)
            cipher = evp_cipher;
        else
            cipher = EVP_get_cipherbyname(name);

        if (cipher == NULL) {
            (void)ERR_clear_last_mark();
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CIPHER_TYPE);
            goto err;
        }
        (void)ERR_pop_to_mark();
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CONTENT_TYPE);
        goto err;
    }

    /* Detached content must be supplied via in_bio instead. */
    if (data_body == NULL && in_bio == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        goto err;
    }

    /* We will be checking the signature */
    if (md_sk != NULL) {
        for (i = 0; i < sk_X509_ALGOR_num(md_sk); i++) {
            xa = sk_X509_ALGOR_value(md_sk, i);
            if ((btmp = BIO_new(BIO_f_md())) == NULL) {
                ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
                goto err;
            }

            OBJ_obj2txt(name, sizeof(name), xa->algorithm, 0);

            (void)ERR_set_mark();
            evp_md = EVP_MD_fetch(libctx, name, propq);
            if (evp_md != NULL)
                md = evp_md;
            else
                md = EVP_get_digestbyname(name);

            if (md == NULL) {
                (void)ERR_clear_last_mark();
                ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNKNOWN_DIGEST_TYPE);
                goto err;
            }
            (void)ERR_pop_to_mark();

            if (BIO_set_md(btmp, md) <= 0) {
                EVP_MD_free(evp_md);
                ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
                goto err;
            }
            EVP_MD_free(evp_md);
            if (out == NULL)
                out = btmp;
            else
                BIO_push(out, btmp);
            btmp = NULL;
        }
    }

    if (cipher != NULL) {
        if ((etmp = BIO_new(BIO_f_cipher())) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
            goto err;
        }

        /*
         * It was encrypted, we need to decrypt the secret key with the
         * private key
         */

        /*
         * Find the recipientInfo which matches the passed certificate (if
         * any)
         */

        if (pcert) {
            for (i = 0; i < sk_PKCS7_RECIP_INFO_num(rsk); i++) {
                ri = sk_PKCS7_RECIP_INFO_value(rsk, i);
                if (!pkcs7_cmp_ri(ri, pcert))
                    break;
                ri = NULL;
            }
            if (ri == NULL) {
                ERR_raise(ERR_LIB_PKCS7,
                          PKCS7_R_NO_RECIPIENT_MATCHES_CERTIFICATE);
                goto err;
            }
        }

        /* If we haven't got a certificate try each ri in turn */
        if (pcert == NULL) {
            /*
             * Always attempt to decrypt all rinfo even after success as a
             * defence against MMA timing attacks.
             */
            for (i = 0; i < sk_PKCS7_RECIP_INFO_num(rsk); i++) {
                ri = sk_PKCS7_RECIP_INFO_value(rsk, i);
                ri->ctx = p7_ctx;
                if (pkcs7_decrypt_rinfo(&ek, &eklen, ri, pkey,
                        EVP_CIPHER_get_key_length(cipher)) < 0)
                    goto err;
                ERR_clear_error();
            }
        } else {
            ri->ctx = p7_ctx;
            /* Only exit on fatal errors, not decrypt failure */
            if (pkcs7_decrypt_rinfo(&ek, &eklen, ri, pkey, 0) < 0)
                goto err;
            ERR_clear_error();
        }

        evp_ctx = NULL;
        BIO_get_cipher_ctx(etmp, &evp_ctx);
        if (EVP_CipherInit_ex(evp_ctx, cipher, NULL, NULL, NULL, 0) <= 0)
            goto err;
        if (EVP_CIPHER_asn1_to_param(evp_ctx, enc_alg->parameter) <= 0)
            goto err;
        /* Generate random key as MMA defence */
        len = EVP_CIPHER_CTX_get_key_length(evp_ctx);
        if (len <= 0)
            goto err;
        tkeylen = (size_t)len;
        tkey = OPENSSL_malloc(tkeylen);
        if (tkey == NULL)
            goto err;
        if (EVP_CIPHER_CTX_rand_key(evp_ctx, tkey) <= 0)
            goto err;
        if (ek == NULL) {
            ek = tkey;
            eklen = tkeylen;
            tkey = NULL;
        }

        if (eklen != EVP_CIPHER_CTX_get_key_length(evp_ctx)) {
            /*
             * Some S/MIME clients don't use the same key and effective key
             * length. The key length is determined by the size of the
             * decrypted RSA key.
             */
            if (EVP_CIPHER_CTX_set_key_length(evp_ctx, eklen) <= 0) {
                /* Use random key as MMA defence */
                OPENSSL_clear_free(ek, eklen);
                ek = tkey;
                eklen = tkeylen;
                tkey = NULL;
            }
        }
        /* Clear errors so we don't leak information useful in MMA */
        ERR_clear_error();
        if (EVP_CipherInit_ex(evp_ctx, NULL, NULL, ek, NULL, 0) <= 0)
            goto err;

        OPENSSL_clear_free(ek, eklen);
        ek = NULL;
        OPENSSL_clear_free(tkey, tkeylen);
        tkey = NULL;

        if (out == NULL)
            out = etmp;
        else
            BIO_push(out, etmp);
        etmp = NULL;
    }
    if (in_bio != NULL) {
        bio = in_bio;
    } else {
        if (data_body->length > 0)
            bio = BIO_new_mem_buf(data_body->data, data_body->length);
        else {
            bio = BIO_new(BIO_s_mem());
            if (bio == NULL)
                goto err;
            BIO_set_mem_eof_return(bio, 0);
        }
        if (bio == NULL)
            goto err;
    }
    BIO_push(out, bio);
    bio = NULL;
    EVP_CIPHER_free(evp_cipher);
    return out;

 err:
    EVP_CIPHER_free(evp_cipher);
    OPENSSL_clear_free(ek, eklen);
    OPENSSL_clear_free(tkey, tkeylen);
    BIO_free_all(out);
    BIO_free_all(btmp);
    BIO_free_all(etmp);
    BIO_free_all(bio);
    return NULL;
}

static BIO *PKCS7_find_digest(EVP_MD_CTX **pmd, BIO *bio, int nid)
{
    for (;;) {
        bio = BIO_find_type(bio, BIO_TYPE_MD);
        if (bio == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNABLE_TO_FIND_MESSAGE_DIGEST);
            return NULL;
        }
        BIO_get_md_ctx(bio, pmd);
        if (*pmd == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_INTERNAL_ERROR);
            return NULL;
        }
        if (EVP_MD_CTX_get_type(*pmd) == nid)
            return bio;
        bio = BIO_next(bio);
    }
    return NULL;
}

static int do_pkcs7_signed_attrib(PKCS7_SIGNER_INFO *si, EVP_MD_CTX *mctx)
{
    unsigned char md_data[EVP_MAX_MD_SIZE];
    unsigned int md_len;

    /* Add signing time if not already present */
    if (!PKCS7_get_signed_attribute(si, NID_pkcs9_signingTime)) {
        if (!PKCS7_add0_attrib_signing_time(si, NULL)) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_PKCS7_LIB);
            return 0;
        }
    }

    /* Add digest */
    if (!EVP_DigestFinal_ex(mctx, md_data, &md_len)) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_EVP_LIB);
        return 0;
    }
    if (!PKCS7_add1_attrib_digest(si, md_data, md_len)) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_PKCS7_LIB);
        return 0;
    }

    /* Now sign the attributes */
    if (!PKCS7_SIGNER_INFO_sign(si))
        return 0;

    return 1;
}

int PKCS7_dataFinal(PKCS7 *p7, BIO *bio)
{
    int ret = 0;
    int i, j;
    BIO *btmp;
    PKCS7_SIGNER_INFO *si;
    EVP_MD_CTX *mdc, *ctx_tmp;
    STACK_OF(X509_ATTRIBUTE) *sk;
    STACK_OF(PKCS7_SIGNER_INFO) *si_sk = NULL;
    ASN1_OCTET_STRING *os = NULL;
    const PKCS7_CTX *p7_ctx;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return 0;
    }

    p7_ctx = ossl_pkcs7_get0_ctx(p7);

    if (p7->d.ptr == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        return 0;
    }

    ctx_tmp = EVP_MD_CTX_new();
    if (ctx_tmp == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_EVP_LIB);
        return 0;
    }

    i = OBJ_obj2nid(p7->type);
    p7->state = PKCS7_S_HEADER;

    switch (i) {
    case NID_pkcs7_data:
        os = p7->d.data;
        break;
    case NID_pkcs7_signedAndEnveloped:
        /* XXXXXXXXXXXXXXXX */
        si_sk = p7->d.signed_and_enveloped->signer_info;
        os = p7->d.signed_and_enveloped->enc_data->enc_data;
        if (os == NULL) {
            os = ASN1_OCTET_STRING_new();
            if (os == NULL) {
                ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
                goto err;
            }
            p7->d.signed_and_enveloped->enc_data->enc_data = os;
        }
        break;
    case NID_pkcs7_enveloped:
        /* XXXXXXXXXXXXXXXX */
        os = p7->d.enveloped->enc_data->enc_data;
        if (os == NULL) {
            os = ASN1_OCTET_STRING_new();
            if (os == NULL) {
                ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
                goto err;
            }
            p7->d.enveloped->enc_data->enc_data = os;
        }
        break;
    case NID_pkcs7_signed:
        si_sk = p7->d.sign->signer_info;
        os = PKCS7_get_octet_string(p7->d.sign->contents);
        /* If detached data then the content is excluded */
        if (PKCS7_type_is_data(p7->d.sign->contents) && p7->detached) {
            ASN1_OCTET_STRING_free(os);
            os = NULL;
            p7->d.sign->contents->d.data = NULL;
        }
        break;

    case NID_pkcs7_digest:
        os = PKCS7_get_octet_string(p7->d.digest->contents);
        /* If detached data then the content is excluded */
        if (PKCS7_type_is_data(p7->d.digest->contents) && p7->detached) {
            ASN1_OCTET_STRING_free(os);
            os = NULL;
            p7->d.digest->contents->d.data = NULL;
        }
        break;

    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CONTENT_TYPE);
        goto err;
    }

    if (si_sk != NULL) {
        for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(si_sk); i++) {
            si = sk_PKCS7_SIGNER_INFO_value(si_sk, i);
            if (si->pkey == NULL)
                continue;

            j = OBJ_obj2nid(si->digest_alg->algorithm);

            btmp = bio;

            btmp = PKCS7_find_digest(&mdc, btmp, j);

            if (btmp == NULL)
                goto err;

            /*
             * We now have the EVP_MD_CTX, lets do the signing.
             */
            if (!EVP_MD_CTX_copy_ex(ctx_tmp, mdc))
                goto err;

            sk = si->auth_attr;

            /*
             * If there are attributes, we add the digest attribute and only
             * sign the attributes
             */
            if (sk_X509_ATTRIBUTE_num(sk) > 0) {
                if (!do_pkcs7_signed_attrib(si, ctx_tmp))
                    goto err;
            } else {
                unsigned char *abuf = NULL;
                unsigned int abuflen = EVP_PKEY_get_size(si->pkey);

                if (abuflen == 0 || (abuf = OPENSSL_malloc(abuflen)) == NULL)
                    goto err;

                if (!EVP_SignFinal_ex(ctx_tmp, abuf, &abuflen, si->pkey,
                                      ossl_pkcs7_ctx_get0_libctx(p7_ctx),
                                      ossl_pkcs7_ctx_get0_propq(p7_ctx))) {
                    OPENSSL_free(abuf);
                    ERR_raise(ERR_LIB_PKCS7, ERR_R_EVP_LIB);
                    goto err;
                }
                ASN1_STRING_set0(si->enc_digest, abuf, abuflen);
            }
        }
    } else if (i == NID_pkcs7_digest) {
        unsigned char md_data[EVP_MAX_MD_SIZE];
        unsigned int md_len;
        if (!PKCS7_find_digest(&mdc, bio,
                               OBJ_obj2nid(p7->d.digest->md->algorithm)))
            goto err;
        if (!EVP_DigestFinal_ex(mdc, md_data, &md_len))
            goto err;
        if (!ASN1_OCTET_STRING_set(p7->d.digest->digest, md_data, md_len))
            goto err;
    }

    if (!PKCS7_is_detached(p7)) {
        /*
         * NOTE(emilia): I think we only reach os == NULL here because detached
         * digested data support is broken.
         */
        if (os == NULL)
            goto err;
        if (!(os->flags & ASN1_STRING_FLAG_NDEF)) {
            char *cont;
            long contlen;
            btmp = BIO_find_type(bio, BIO_TYPE_MEM);
            if (btmp == NULL) {
                ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNABLE_TO_FIND_MEM_BIO);
                goto err;
            }
            contlen = BIO_get_mem_data(btmp, &cont);
            /*
             * Mark the BIO read only then we can use its copy of the data
             * instead of making an extra copy.
             */
            BIO_set_flags(btmp, BIO_FLAGS_MEM_RDONLY);
            BIO_set_mem_eof_return(btmp, 0);
            ASN1_STRING_set0(os, (unsigned char *)cont, contlen);
        }
    }
    ret = 1;
 err:
    EVP_MD_CTX_free(ctx_tmp);
    return ret;
}

int PKCS7_SIGNER_INFO_sign(PKCS7_SIGNER_INFO *si)
{
    EVP_MD_CTX *mctx;
    EVP_PKEY_CTX *pctx = NULL;
    unsigned char *abuf = NULL;
    int alen;
    size_t siglen;
    const EVP_MD *md = NULL;
    const PKCS7_CTX *ctx = si->ctx;

    md = EVP_get_digestbyobj(si->digest_alg->algorithm);
    if (md == NULL)
        return 0;

    mctx = EVP_MD_CTX_new();
    if (mctx == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_EVP_LIB);
        goto err;
    }

    if (EVP_DigestSignInit_ex(mctx, &pctx, EVP_MD_get0_name(md),
                              ossl_pkcs7_ctx_get0_libctx(ctx),
                              ossl_pkcs7_ctx_get0_propq(ctx), si->pkey,
                              NULL) <= 0)
        goto err;

    alen = ASN1_item_i2d((ASN1_VALUE *)si->auth_attr, &abuf,
                         ASN1_ITEM_rptr(PKCS7_ATTR_SIGN));
    if (alen < 0 || abuf == NULL)
        goto err;
    if (EVP_DigestSignUpdate(mctx, abuf, alen) <= 0)
        goto err;
    OPENSSL_free(abuf);
    abuf = NULL;
    if (EVP_DigestSignFinal(mctx, NULL, &siglen) <= 0)
        goto err;
    abuf = OPENSSL_malloc(siglen);
    if (abuf == NULL)
        goto err;
    if (EVP_DigestSignFinal(mctx, abuf, &siglen) <= 0)
        goto err;

    EVP_MD_CTX_free(mctx);

    ASN1_STRING_set0(si->enc_digest, abuf, siglen);

    return 1;

 err:
    OPENSSL_free(abuf);
    EVP_MD_CTX_free(mctx);
    return 0;
}

/* This partly overlaps with PKCS7_verify(). It does not support flags. */
int PKCS7_dataVerify(X509_STORE *cert_store, X509_STORE_CTX *ctx, BIO *bio,
                     PKCS7 *p7, PKCS7_SIGNER_INFO *si)
{
    PKCS7_ISSUER_AND_SERIAL *ias;
    int ret = 0, i;
    STACK_OF(X509) *untrusted;
    STACK_OF(X509_CRL) *crls;
    X509 *signer;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return 0;
    }

    if (p7->d.ptr == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        return 0;
    }

    if (PKCS7_type_is_signed(p7)) {
        untrusted = p7->d.sign->cert;
        crls = p7->d.sign->crl;
    } else if (PKCS7_type_is_signedAndEnveloped(p7)) {
        untrusted = p7->d.signed_and_enveloped->cert;
        crls = p7->d.signed_and_enveloped->crl;
    } else {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_PKCS7_TYPE);
        goto err;
    }
    X509_STORE_CTX_set0_crls(ctx, crls);

    /* XXXXXXXXXXXXXXXXXXXXXXX */
    ias = si->issuer_and_serial;

    signer = X509_find_by_issuer_and_serial(untrusted, ias->issuer, ias->serial);

    /* Were we able to find the signer certificate in passed to us? */
    if (signer == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNABLE_TO_FIND_CERTIFICATE);
        goto err;
    }

    /* Lets verify */
    if (!X509_STORE_CTX_init(ctx, cert_store, signer, untrusted)) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_X509_LIB);
        goto err;
    }
    X509_STORE_CTX_set_purpose(ctx, X509_PURPOSE_SMIME_SIGN);
    i = X509_verify_cert(ctx);
    if (i <= 0) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_X509_LIB);
        goto err;
    }

    return PKCS7_signatureVerify(bio, p7, si, signer);
 err:
    return ret;
}

int PKCS7_signatureVerify(BIO *bio, PKCS7 *p7, PKCS7_SIGNER_INFO *si,
                          X509 *signer)
{
    ASN1_OCTET_STRING *os;
    EVP_MD_CTX *mdc_tmp, *mdc;
    const EVP_MD *md;
    EVP_MD *fetched_md = NULL;
    int ret = 0, i;
    int md_type;
    STACK_OF(X509_ATTRIBUTE) *sk;
    BIO *btmp;
    EVP_PKEY *pkey;
    unsigned char *abuf = NULL;
    const PKCS7_CTX *ctx = ossl_pkcs7_get0_ctx(p7);
    OSSL_LIB_CTX *libctx = ossl_pkcs7_ctx_get0_libctx(ctx);
    const char *propq = ossl_pkcs7_ctx_get0_propq(ctx);

    mdc_tmp = EVP_MD_CTX_new();
    if (mdc_tmp == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_EVP_LIB);
        goto err;
    }

    if (!PKCS7_type_is_signed(p7) && !PKCS7_type_is_signedAndEnveloped(p7)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_PKCS7_TYPE);
        goto err;
    }

    md_type = OBJ_obj2nid(si->digest_alg->algorithm);

    btmp = bio;
    for (;;) {
        if ((btmp == NULL) ||
            ((btmp = BIO_find_type(btmp, BIO_TYPE_MD)) == NULL)) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNABLE_TO_FIND_MESSAGE_DIGEST);
            goto err;
        }
        BIO_get_md_ctx(btmp, &mdc);
        if (mdc == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        if (EVP_MD_CTX_get_type(mdc) == md_type)
            break;
        /*
         * Workaround for some broken clients that put the signature OID
         * instead of the digest OID in digest_alg->algorithm
         */
        if (EVP_MD_get_pkey_type(EVP_MD_CTX_get0_md(mdc)) == md_type)
            break;
        btmp = BIO_next(btmp);
    }

    /*
     * mdc is the digest ctx that we want, unless there are attributes, in
     * which case the digest is the signed attributes
     */
    if (!EVP_MD_CTX_copy_ex(mdc_tmp, mdc))
        goto err;

    sk = si->auth_attr;
    if ((sk != NULL) && (sk_X509_ATTRIBUTE_num(sk) != 0)) {
        unsigned char md_dat[EVP_MAX_MD_SIZE];
        unsigned int md_len;
        int alen;
        ASN1_OCTET_STRING *message_digest;

        if (!EVP_DigestFinal_ex(mdc_tmp, md_dat, &md_len))
            goto err;
        message_digest = PKCS7_digest_from_attributes(sk);
        if (!message_digest) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNABLE_TO_FIND_MESSAGE_DIGEST);
            goto err;
        }
        if ((message_digest->length != (int)md_len) ||
            (memcmp(message_digest->data, md_dat, md_len))) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_DIGEST_FAILURE);
            ret = -1;
            goto err;
        }

        (void)ERR_set_mark();
        fetched_md = EVP_MD_fetch(libctx, OBJ_nid2sn(md_type), propq);

        if (fetched_md != NULL)
            md = fetched_md;
        else
            md = EVP_get_digestbynid(md_type);

        if (md == NULL || !EVP_VerifyInit_ex(mdc_tmp, md, NULL)) {
            (void)ERR_clear_last_mark();
            goto err;
        }
        (void)ERR_pop_to_mark();

        alen = ASN1_item_i2d((ASN1_VALUE *)sk, &abuf,
                             ASN1_ITEM_rptr(PKCS7_ATTR_VERIFY));
        if (alen <= 0 || abuf == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            ret = -1;
            goto err;
        }
        if (!EVP_VerifyUpdate(mdc_tmp, abuf, alen))
            goto err;
    }

    os = si->enc_digest;
    pkey = X509_get0_pubkey(signer);
    if (pkey == NULL) {
        ret = -1;
        goto err;
    }

    i = EVP_VerifyFinal_ex(mdc_tmp, os->data, os->length, pkey, libctx, propq);
    if (i <= 0) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SIGNATURE_FAILURE);
        ret = -1;
        goto err;
    }
    ret = 1;
 err:
    OPENSSL_free(abuf);
    EVP_MD_CTX_free(mdc_tmp);
    EVP_MD_free(fetched_md);
    return ret;
}

PKCS7_ISSUER_AND_SERIAL *PKCS7_get_issuer_and_serial(PKCS7 *p7, int idx)
{
    STACK_OF(PKCS7_RECIP_INFO) *rsk;
    PKCS7_RECIP_INFO *ri;
    int i;

    i = OBJ_obj2nid(p7->type);
    if (i != NID_pkcs7_signedAndEnveloped)
        return NULL;
    if (p7->d.signed_and_enveloped == NULL)
        return NULL;
    rsk = p7->d.signed_and_enveloped->recipientinfo;
    if (rsk == NULL)
        return NULL;
    if (sk_PKCS7_RECIP_INFO_num(rsk) <= idx)
        return NULL;
    ri = sk_PKCS7_RECIP_INFO_value(rsk, idx);
    return ri->issuer_and_serial;
}

ASN1_TYPE *PKCS7_get_signed_attribute(const PKCS7_SIGNER_INFO *si, int nid)
{
    return get_attribute(si->auth_attr, nid);
}

ASN1_TYPE *PKCS7_get_attribute(const PKCS7_SIGNER_INFO *si, int nid)
{
    return get_attribute(si->unauth_attr, nid);
}

static ASN1_TYPE *get_attribute(const STACK_OF(X509_ATTRIBUTE) *sk, int nid)
{
    int idx = X509at_get_attr_by_NID(sk, nid, -1);

    if (idx < 0)
        return NULL;
    return X509_ATTRIBUTE_get0_type(X509at_get_attr(sk, idx), 0);
}

ASN1_OCTET_STRING *PKCS7_digest_from_attributes(STACK_OF(X509_ATTRIBUTE) *sk)
{
    ASN1_TYPE *astype;
    if ((astype = get_attribute(sk, NID_pkcs9_messageDigest)) == NULL)
        return NULL;
    return astype->value.octet_string;
}

int PKCS7_set_signed_attributes(PKCS7_SIGNER_INFO *p7si,
                                STACK_OF(X509_ATTRIBUTE) *sk)
{
    sk_X509_ATTRIBUTE_pop_free(p7si->auth_attr, X509_ATTRIBUTE_free);
    p7si->auth_attr = sk_X509_ATTRIBUTE_deep_copy(sk, X509_ATTRIBUTE_dup, X509_ATTRIBUTE_free);
    if (p7si->auth_attr == NULL)
        return 0;
    return 1;
}

int PKCS7_set_attributes(PKCS7_SIGNER_INFO *p7si,
                         STACK_OF(X509_ATTRIBUTE) *sk)
{
    sk_X509_ATTRIBUTE_pop_free(p7si->unauth_attr, X509_ATTRIBUTE_free);
    p7si->unauth_attr = sk_X509_ATTRIBUTE_deep_copy(sk, X509_ATTRIBUTE_dup, X509_ATTRIBUTE_free);
    if (p7si->unauth_attr == NULL)
        return 0;
    return 1;
}

int PKCS7_add_signed_attribute(PKCS7_SIGNER_INFO *p7si, int nid, int atrtype,
                               void *value)
{
    return add_attribute(&(p7si->auth_attr), nid, atrtype, value);
}

int PKCS7_add_attribute(PKCS7_SIGNER_INFO *p7si, int nid, int atrtype,
                        void *value)
{
    return add_attribute(&(p7si->unauth_attr), nid, atrtype, value);
}

static int add_attribute(STACK_OF(X509_ATTRIBUTE) **sk, int nid, int atrtype,
                         void *value)
{
    X509_ATTRIBUTE *attr = NULL;
    int i, n;

    if (*sk == NULL) {
        if ((*sk = sk_X509_ATTRIBUTE_new_null()) == NULL)
            return 0;
    }
    n = sk_X509_ATTRIBUTE_num(*sk);
    for (i = 0; i < n; i++) {
        attr = sk_X509_ATTRIBUTE_value(*sk, i);
        if (OBJ_obj2nid(X509_ATTRIBUTE_get0_object(attr)) == nid)
            goto end;
    }
    if (!sk_X509_ATTRIBUTE_push(*sk, NULL))
        return 0;

 end:
    attr = X509_ATTRIBUTE_create(nid, atrtype, value);
    if (attr == NULL) {
        if (i == n)
            sk_X509_ATTRIBUTE_pop(*sk);
        return 0;
    }
    X509_ATTRIBUTE_free(sk_X509_ATTRIBUTE_value(*sk, i));
    (void) sk_X509_ATTRIBUTE_set(*sk, i, attr);
    return 1;
}
