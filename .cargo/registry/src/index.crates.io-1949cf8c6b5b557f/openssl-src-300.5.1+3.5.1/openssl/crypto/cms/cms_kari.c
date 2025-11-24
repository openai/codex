/*
 * Copyright 2013-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level key APIs (DH etc) are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include <openssl/aes.h>
#include "cms_local.h"
#include "crypto/asn1.h"

/* Key Agreement Recipient Info (KARI) routines */

int CMS_RecipientInfo_kari_get0_alg(CMS_RecipientInfo *ri,
                                    X509_ALGOR **palg,
                                    ASN1_OCTET_STRING **pukm)
{
    if (ri->type != CMS_RECIPINFO_AGREE) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_AGREEMENT);
        return 0;
    }
    if (palg)
        *palg = ri->d.kari->keyEncryptionAlgorithm;
    if (pukm)
        *pukm = ri->d.kari->ukm;
    return 1;
}

/* Retrieve recipient encrypted keys from a kari */

STACK_OF(CMS_RecipientEncryptedKey)
*CMS_RecipientInfo_kari_get0_reks(CMS_RecipientInfo *ri)
{
    if (ri->type != CMS_RECIPINFO_AGREE) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_AGREEMENT);
        return NULL;
    }
    return ri->d.kari->recipientEncryptedKeys;
}

int CMS_RecipientInfo_kari_get0_orig_id(CMS_RecipientInfo *ri,
                                        X509_ALGOR **pubalg,
                                        ASN1_BIT_STRING **pubkey,
                                        ASN1_OCTET_STRING **keyid,
                                        X509_NAME **issuer,
                                        ASN1_INTEGER **sno)
{
    CMS_OriginatorIdentifierOrKey *oik;

    if (ri->type != CMS_RECIPINFO_AGREE) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_AGREEMENT);
        return 0;
    }
    oik = ri->d.kari->originator;
    if (issuer)
        *issuer = NULL;
    if (sno)
        *sno = NULL;
    if (keyid)
        *keyid = NULL;
    if (pubalg)
        *pubalg = NULL;
    if (pubkey)
        *pubkey = NULL;
    if (oik->type == CMS_OIK_ISSUER_SERIAL) {
        if (issuer)
            *issuer = oik->d.issuerAndSerialNumber->issuer;
        if (sno)
            *sno = oik->d.issuerAndSerialNumber->serialNumber;
    } else if (oik->type == CMS_OIK_KEYIDENTIFIER) {
        if (keyid)
            *keyid = oik->d.subjectKeyIdentifier;
    } else if (oik->type == CMS_OIK_PUBKEY) {
        if (pubalg)
            *pubalg = oik->d.originatorKey->algorithm;
        if (pubkey)
            *pubkey = oik->d.originatorKey->publicKey;
    } else
        return 0;
    return 1;
}

int CMS_RecipientInfo_kari_orig_id_cmp(CMS_RecipientInfo *ri, X509 *cert)
{
    CMS_OriginatorIdentifierOrKey *oik;

    if (ri->type != CMS_RECIPINFO_AGREE) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_AGREEMENT);
        return -2;
    }
    oik = ri->d.kari->originator;
    if (oik->type == CMS_OIK_ISSUER_SERIAL)
        return ossl_cms_ias_cert_cmp(oik->d.issuerAndSerialNumber, cert);
    else if (oik->type == CMS_OIK_KEYIDENTIFIER)
        return ossl_cms_keyid_cert_cmp(oik->d.subjectKeyIdentifier, cert);
    return -1;
}

int CMS_RecipientEncryptedKey_get0_id(CMS_RecipientEncryptedKey *rek,
                                      ASN1_OCTET_STRING **keyid,
                                      ASN1_GENERALIZEDTIME **tm,
                                      CMS_OtherKeyAttribute **other,
                                      X509_NAME **issuer, ASN1_INTEGER **sno)
{
    CMS_KeyAgreeRecipientIdentifier *rid = rek->rid;

    if (rid->type == CMS_REK_ISSUER_SERIAL) {
        if (issuer)
            *issuer = rid->d.issuerAndSerialNumber->issuer;
        if (sno)
            *sno = rid->d.issuerAndSerialNumber->serialNumber;
        if (keyid)
            *keyid = NULL;
        if (tm)
            *tm = NULL;
        if (other)
            *other = NULL;
    } else if (rid->type == CMS_REK_KEYIDENTIFIER) {
        if (keyid)
            *keyid = rid->d.rKeyId->subjectKeyIdentifier;
        if (tm)
            *tm = rid->d.rKeyId->date;
        if (other)
            *other = rid->d.rKeyId->other;
        if (issuer)
            *issuer = NULL;
        if (sno)
            *sno = NULL;
    } else
        return 0;
    return 1;
}

int CMS_RecipientEncryptedKey_cert_cmp(CMS_RecipientEncryptedKey *rek,
                                       X509 *cert)
{
    CMS_KeyAgreeRecipientIdentifier *rid = rek->rid;

    if (rid->type == CMS_REK_ISSUER_SERIAL)
        return ossl_cms_ias_cert_cmp(rid->d.issuerAndSerialNumber, cert);
    else if (rid->type == CMS_REK_KEYIDENTIFIER)
        return ossl_cms_keyid_cert_cmp(rid->d.rKeyId->subjectKeyIdentifier,
                                       cert);
    else
        return -1;
}

int CMS_RecipientInfo_kari_set0_pkey_and_peer(CMS_RecipientInfo *ri,
                                              EVP_PKEY *pk, X509 *peer)
{
    EVP_PKEY_CTX *pctx;
    CMS_KeyAgreeRecipientInfo *kari = ri->d.kari;

    EVP_PKEY_CTX_free(kari->pctx);
    kari->pctx = NULL;
    if (pk == NULL)
        return 1;

    pctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(kari->cms_ctx),
                                      pk,
                                      ossl_cms_ctx_get0_propq(kari->cms_ctx));
    if (pctx == NULL || EVP_PKEY_derive_init(pctx) <= 0)
        goto err;

    if (peer != NULL) {
        EVP_PKEY *pub_pkey = X509_get0_pubkey(peer);

        if (EVP_PKEY_derive_set_peer(pctx, pub_pkey) <= 0)
            goto err;
    }

    kari->pctx = pctx;
    return 1;
 err:
    EVP_PKEY_CTX_free(pctx);
    return 0;
}

int CMS_RecipientInfo_kari_set0_pkey(CMS_RecipientInfo *ri, EVP_PKEY *pk)
{
    return CMS_RecipientInfo_kari_set0_pkey_and_peer(ri, pk, NULL);
}

EVP_CIPHER_CTX *CMS_RecipientInfo_kari_get0_ctx(CMS_RecipientInfo *ri)
{
    if (ri->type == CMS_RECIPINFO_AGREE)
        return ri->d.kari->ctx;
    return NULL;
}

/*
 * Derive KEK and decrypt/encrypt with it to produce either the original CEK
 * or the encrypted CEK.
 */

static int cms_kek_cipher(unsigned char **pout, size_t *poutlen,
                          const unsigned char *in, size_t inlen,
                          CMS_KeyAgreeRecipientInfo *kari, int enc)
{
    /* Key encryption key */
    unsigned char kek[EVP_MAX_KEY_LENGTH];
    size_t keklen;
    int rv = 0;
    unsigned char *out = NULL;
    int outlen;

    keklen = EVP_CIPHER_CTX_get_key_length(kari->ctx);
    if (keklen > EVP_MAX_KEY_LENGTH)
        return 0;
    /* Derive KEK */
    if (EVP_PKEY_derive(kari->pctx, kek, &keklen) <= 0)
        goto err;
    /* Set KEK in context */
    if (!EVP_CipherInit_ex(kari->ctx, NULL, NULL, kek, NULL, enc))
        goto err;
    /* obtain output length of ciphered key */
    if (!EVP_CipherUpdate(kari->ctx, NULL, &outlen, in, inlen))
        goto err;
    out = OPENSSL_malloc(outlen);
    if (out == NULL)
        goto err;
    if (!EVP_CipherUpdate(kari->ctx, out, &outlen, in, inlen))
        goto err;
    *pout = out;
    *poutlen = (size_t)outlen;
    rv = 1;

 err:
    OPENSSL_cleanse(kek, keklen);
    if (!rv)
        OPENSSL_free(out);
    EVP_CIPHER_CTX_reset(kari->ctx);
    /* FIXME: WHY IS kari->pctx freed here?  /RL */
    EVP_PKEY_CTX_free(kari->pctx);
    kari->pctx = NULL;
    return rv;
}

int CMS_RecipientInfo_kari_decrypt(CMS_ContentInfo *cms,
                                   CMS_RecipientInfo *ri,
                                   CMS_RecipientEncryptedKey *rek)
{
    int rv = 0;
    unsigned char *enckey = NULL, *cek = NULL;
    size_t enckeylen;
    size_t ceklen;
    CMS_EncryptedContentInfo *ec;

    enckeylen = rek->encryptedKey->length;
    enckey = rek->encryptedKey->data;
    /* Setup all parameters to derive KEK */
    if (!ossl_cms_env_asn1_ctrl(ri, 1))
        goto err;
    /* Attempt to decrypt CEK */
    if (!cms_kek_cipher(&cek, &ceklen, enckey, enckeylen, ri->d.kari, 0))
        goto err;
    ec = ossl_cms_get0_env_enc_content(cms);
    OPENSSL_clear_free(ec->key, ec->keylen);
    ec->key = cek;
    ec->keylen = ceklen;
    cek = NULL;
    rv = 1;
 err:
    OPENSSL_free(cek);
    return rv;
}

/* Create ephemeral key and initialise context based on it */
static int cms_kari_create_ephemeral_key(CMS_KeyAgreeRecipientInfo *kari,
                                         EVP_PKEY *pk)
{
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *ekey = NULL;
    int rv = 0;
    const CMS_CTX *ctx = kari->cms_ctx;
    OSSL_LIB_CTX *libctx = ossl_cms_ctx_get0_libctx(ctx);
    const char *propq = ossl_cms_ctx_get0_propq(ctx);

    pctx = EVP_PKEY_CTX_new_from_pkey(libctx, pk, propq);
    if (pctx == NULL)
        goto err;
    if (EVP_PKEY_keygen_init(pctx) <= 0)
        goto err;
    if (EVP_PKEY_keygen(pctx, &ekey) <= 0)
        goto err;
    EVP_PKEY_CTX_free(pctx);
    pctx = EVP_PKEY_CTX_new_from_pkey(libctx, ekey, propq);
    if (pctx == NULL)
        goto err;
    if (EVP_PKEY_derive_init(pctx) <= 0)
        goto err;
    kari->pctx = pctx;
    rv = 1;
 err:
    if (!rv)
        EVP_PKEY_CTX_free(pctx);
    EVP_PKEY_free(ekey);
    return rv;
}

/* Set originator private key and initialise context based on it */
static int cms_kari_set_originator_private_key(CMS_KeyAgreeRecipientInfo *kari,
                                               EVP_PKEY *originatorPrivKey )
{
    EVP_PKEY_CTX *pctx = NULL;
    int rv = 0;
    const CMS_CTX *ctx = kari->cms_ctx;

    pctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(ctx),
                                      originatorPrivKey,
                                      ossl_cms_ctx_get0_propq(ctx));
    if (pctx == NULL)
        goto err;
    if (EVP_PKEY_derive_init(pctx) <= 0)
         goto err;

    kari->pctx = pctx;
    rv = 1;
 err:
    if (rv == 0)
        EVP_PKEY_CTX_free(pctx);
    return rv;
}

/* Initialise a kari based on passed certificate and key */

int ossl_cms_RecipientInfo_kari_init(CMS_RecipientInfo *ri,  X509 *recip,
                                     EVP_PKEY *recipPubKey, X509 *originator,
                                     EVP_PKEY *originatorPrivKey,
                                     unsigned int flags, const CMS_CTX *ctx)
{
    CMS_KeyAgreeRecipientInfo *kari;
    CMS_RecipientEncryptedKey *rek = NULL;

    ri->d.kari = M_ASN1_new_of(CMS_KeyAgreeRecipientInfo);
    if (ri->d.kari == NULL)
        return 0;
    ri->type = CMS_RECIPINFO_AGREE;

    kari = ri->d.kari;
    kari->version = 3;
    kari->cms_ctx = ctx;

    rek = M_ASN1_new_of(CMS_RecipientEncryptedKey);
    if (rek == NULL)
        return 0;

    if (!sk_CMS_RecipientEncryptedKey_push(kari->recipientEncryptedKeys, rek)) {
        M_ASN1_free_of(rek, CMS_RecipientEncryptedKey);
        return 0;
    }

    if (flags & CMS_USE_KEYID) {
        rek->rid->type = CMS_REK_KEYIDENTIFIER;
        rek->rid->d.rKeyId = M_ASN1_new_of(CMS_RecipientKeyIdentifier);
        if (rek->rid->d.rKeyId == NULL)
            return 0;
        if (!ossl_cms_set1_keyid(&rek->rid->d.rKeyId->subjectKeyIdentifier, recip))
            return 0;
    } else {
        rek->rid->type = CMS_REK_ISSUER_SERIAL;
        if (!ossl_cms_set1_ias(&rek->rid->d.issuerAndSerialNumber, recip))
            return 0;
    }

    if (originatorPrivKey == NULL && originator == NULL) {
        /* Create ephemeral key */
        if (!cms_kari_create_ephemeral_key(kari, recipPubKey))
            return 0;
    } else {
        /* Use originator key */
        CMS_OriginatorIdentifierOrKey *oik = ri->d.kari->originator;

        if (originatorPrivKey == NULL || originator == NULL)
            return 0;

        if (flags & CMS_USE_ORIGINATOR_KEYID) {
             oik->type = CMS_OIK_KEYIDENTIFIER;
             oik->d.subjectKeyIdentifier = ASN1_OCTET_STRING_new();
             if (oik->d.subjectKeyIdentifier == NULL)
                  return 0;
             if (!ossl_cms_set1_keyid(&oik->d.subjectKeyIdentifier, originator))
                  return 0;
        } else {
             oik->type = CMS_REK_ISSUER_SERIAL;
             if (!ossl_cms_set1_ias(&oik->d.issuerAndSerialNumber, originator))
                  return 0;
        }

        if (!cms_kari_set_originator_private_key(kari, originatorPrivKey))
            return 0;
    }

    if (!EVP_PKEY_up_ref(recipPubKey))
        return 0;

    rek->pkey = recipPubKey;
    return 1;
}

static int cms_wrap_init(CMS_KeyAgreeRecipientInfo *kari,
                         const EVP_CIPHER *cipher)
{
    const CMS_CTX *cms_ctx = kari->cms_ctx;
    EVP_CIPHER_CTX *ctx = kari->ctx;
    const EVP_CIPHER *kekcipher;
    EVP_CIPHER *fetched_kekcipher;
    const char *kekcipher_name;
    int keylen;
    int ret;

    /* If a suitable wrap algorithm is already set nothing to do */
    kekcipher = EVP_CIPHER_CTX_get0_cipher(ctx);
    if (kekcipher != NULL) {
        if (EVP_CIPHER_CTX_get_mode(ctx) != EVP_CIPH_WRAP_MODE)
            return 0;
        return 1;
    }
    if (cipher == NULL)
        return 0;
    keylen = EVP_CIPHER_get_key_length(cipher);
    if ((EVP_CIPHER_get_flags(cipher) & EVP_CIPH_FLAG_GET_WRAP_CIPHER) != 0) {
        ret = EVP_CIPHER_meth_get_ctrl(cipher)(NULL, EVP_CTRL_GET_WRAP_CIPHER,
                                               0, &kekcipher);
        if (ret <= 0)
             return 0;

        if (kekcipher != NULL) {
             if (EVP_CIPHER_get_mode(kekcipher) != EVP_CIPH_WRAP_MODE)
                 return 0;
             kekcipher_name = EVP_CIPHER_get0_name(kekcipher);
             goto enc;
        }
    }

    /*
     * Pick a cipher based on content encryption cipher. If it is DES3 use
     * DES3 wrap otherwise use AES wrap similar to key size.
     */
#ifndef OPENSSL_NO_DES
    if (EVP_CIPHER_get_type(cipher) == NID_des_ede3_cbc)
        kekcipher_name = SN_id_smime_alg_CMS3DESwrap;
    else
#endif
    if (keylen <= 16)
        kekcipher_name = SN_id_aes128_wrap;
    else if (keylen <= 24)
        kekcipher_name = SN_id_aes192_wrap;
    else
        kekcipher_name = SN_id_aes256_wrap;
enc:
    fetched_kekcipher = EVP_CIPHER_fetch(ossl_cms_ctx_get0_libctx(cms_ctx),
                                         kekcipher_name,
                                         ossl_cms_ctx_get0_propq(cms_ctx));
    if (fetched_kekcipher == NULL)
        return 0;
    ret = EVP_EncryptInit_ex(ctx, fetched_kekcipher, NULL, NULL, NULL);
    EVP_CIPHER_free(fetched_kekcipher);
    return ret;
}

/* Encrypt content key in key agreement recipient info */

int ossl_cms_RecipientInfo_kari_encrypt(const CMS_ContentInfo *cms,
                                        CMS_RecipientInfo *ri)
{
    CMS_KeyAgreeRecipientInfo *kari;
    CMS_EncryptedContentInfo *ec;
    CMS_RecipientEncryptedKey *rek;
    STACK_OF(CMS_RecipientEncryptedKey) *reks;
    int i;

    if (ri->type != CMS_RECIPINFO_AGREE) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_AGREEMENT);
        return 0;
    }
    kari = ri->d.kari;
    reks = kari->recipientEncryptedKeys;
    ec = ossl_cms_get0_env_enc_content(cms);
    /* Initialise wrap algorithm parameters */
    if (!cms_wrap_init(kari, ec->cipher))
        return 0;
    /*
     * If no originator key set up initialise for ephemeral key the public key
     * ASN1 structure will set the actual public key value.
     */
    if (kari->originator->type == -1) {
        CMS_OriginatorIdentifierOrKey *oik = kari->originator;
        oik->type = CMS_OIK_PUBKEY;
        oik->d.originatorKey = M_ASN1_new_of(CMS_OriginatorPublicKey);
        if (!oik->d.originatorKey)
            return 0;
    } else {
        /*
         * Currently it is not possible to get public key as it is not stored
         * during kari initialization.
         */
        ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_UNSUPPORTED_STATIC_KEY_AGREEMENT);
        return 0;
    }
    /* Initialise KDF algorithm */
    if (!ossl_cms_env_asn1_ctrl(ri, 0))
        return 0;
    /* For each rek, derive KEK, encrypt CEK */
    for (i = 0; i < sk_CMS_RecipientEncryptedKey_num(reks); i++) {
        unsigned char *enckey;
        size_t enckeylen;
        rek = sk_CMS_RecipientEncryptedKey_value(reks, i);
        if (EVP_PKEY_derive_set_peer(kari->pctx, rek->pkey) <= 0)
            return 0;
        if (!cms_kek_cipher(&enckey, &enckeylen, ec->key, ec->keylen,
                            kari, 1))
            return 0;
        ASN1_STRING_set0(rek->encryptedKey, enckey, enckeylen);
    }

    return 1;
}
