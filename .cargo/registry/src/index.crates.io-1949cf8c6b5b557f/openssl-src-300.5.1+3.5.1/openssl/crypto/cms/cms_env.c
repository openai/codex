/*
 * Copyright 2008-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include <openssl/evp.h>
#include "internal/sizes.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/x509.h"
#include "cms_local.h"

/* CMS EnvelopedData Utilities */
static void cms_env_set_version(CMS_EnvelopedData *env);

#define CMS_ENVELOPED_STANDARD 1
#define CMS_ENVELOPED_AUTH     2

static int cms_get_enveloped_type_simple(const CMS_ContentInfo *cms)
{
    int nid = OBJ_obj2nid(cms->contentType);

    switch (nid) {
    case NID_pkcs7_enveloped:
        return CMS_ENVELOPED_STANDARD;

    case NID_id_smime_ct_authEnvelopedData:
        return CMS_ENVELOPED_AUTH;

    default:
        return 0;
    }
}

static int cms_get_enveloped_type(const CMS_ContentInfo *cms)
{
    int ret = cms_get_enveloped_type_simple(cms);

    if (ret == 0)
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_NOT_ENVELOPED_DATA);
    return ret;
}

CMS_EnvelopedData *ossl_cms_get0_enveloped(CMS_ContentInfo *cms)
{
    if (OBJ_obj2nid(cms->contentType) != NID_pkcs7_enveloped) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_NOT_ENVELOPED_DATA);
        return NULL;
    }
    return cms->d.envelopedData;
}

CMS_AuthEnvelopedData *ossl_cms_get0_auth_enveloped(CMS_ContentInfo *cms)
{
    if (OBJ_obj2nid(cms->contentType) != NID_id_smime_ct_authEnvelopedData) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_NOT_ENVELOPED_DATA);
        return NULL;
    }
    return cms->d.authEnvelopedData;
}

static CMS_EnvelopedData *cms_enveloped_data_init(CMS_ContentInfo *cms)
{
    if (cms->d.other == NULL) {
        cms->d.envelopedData = M_ASN1_new_of(CMS_EnvelopedData);
        if (cms->d.envelopedData == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            return NULL;
        }
        cms->d.envelopedData->version = 0;
        cms->d.envelopedData->encryptedContentInfo->contentType =
            OBJ_nid2obj(NID_pkcs7_data);
        ASN1_OBJECT_free(cms->contentType);
        cms->contentType = OBJ_nid2obj(NID_pkcs7_enveloped);
        return cms->d.envelopedData;
    }
    return ossl_cms_get0_enveloped(cms);
}

static CMS_AuthEnvelopedData *
cms_auth_enveloped_data_init(CMS_ContentInfo *cms)
{
    if (cms->d.other == NULL) {
        cms->d.authEnvelopedData = M_ASN1_new_of(CMS_AuthEnvelopedData);
        if (cms->d.authEnvelopedData == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            return NULL;
        }
        /* Defined in RFC 5083 - Section 2.1. "AuthEnvelopedData Type" */
        cms->d.authEnvelopedData->version = 0;
        cms->d.authEnvelopedData->authEncryptedContentInfo->contentType =
            OBJ_nid2obj(NID_pkcs7_data);
        ASN1_OBJECT_free(cms->contentType);
        cms->contentType = OBJ_nid2obj(NID_id_smime_ct_authEnvelopedData);
        return cms->d.authEnvelopedData;
    }
    return ossl_cms_get0_auth_enveloped(cms);
}

int ossl_cms_env_asn1_ctrl(CMS_RecipientInfo *ri, int cmd)
{
    EVP_PKEY *pkey;
    int i;
    if (ri->type == CMS_RECIPINFO_TRANS)
        pkey = ri->d.ktri->pkey;
    else if (ri->type == CMS_RECIPINFO_AGREE) {
        EVP_PKEY_CTX *pctx = ri->d.kari->pctx;

        if (pctx == NULL)
            return 0;
        pkey = EVP_PKEY_CTX_get0_pkey(pctx);
        if (pkey == NULL)
            return 0;
    } else
        return 0;

    if (EVP_PKEY_is_a(pkey, "DHX") || EVP_PKEY_is_a(pkey, "DH"))
        return ossl_cms_dh_envelope(ri, cmd);
    else if (EVP_PKEY_is_a(pkey, "EC"))
        return ossl_cms_ecdh_envelope(ri, cmd);
    else if (EVP_PKEY_is_a(pkey, "RSA"))
        return ossl_cms_rsa_envelope(ri, cmd);

    /* Something else? We'll give engines etc a chance to handle this */
    if (pkey->ameth == NULL || pkey->ameth->pkey_ctrl == NULL)
        return 1;
    i = pkey->ameth->pkey_ctrl(pkey, ASN1_PKEY_CTRL_CMS_ENVELOPE, cmd, ri);
    if (i == -2) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
        return 0;
    }
    if (i <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CTRL_FAILURE);
        return 0;
    }
    return 1;
}

CMS_EncryptedContentInfo *ossl_cms_get0_env_enc_content(const CMS_ContentInfo *cms)
{
    switch (cms_get_enveloped_type(cms)) {
    case CMS_ENVELOPED_STANDARD:
        return cms->d.envelopedData == NULL ? NULL
            : cms->d.envelopedData->encryptedContentInfo;

    case CMS_ENVELOPED_AUTH:
        return cms->d.authEnvelopedData == NULL ? NULL
            : cms->d.authEnvelopedData->authEncryptedContentInfo;

    default:
        return NULL;
    }
}

STACK_OF(CMS_RecipientInfo) *CMS_get0_RecipientInfos(CMS_ContentInfo *cms)
{
    switch (cms_get_enveloped_type(cms)) {
    case CMS_ENVELOPED_STANDARD:
        return cms->d.envelopedData->recipientInfos;

    case CMS_ENVELOPED_AUTH:
        return cms->d.authEnvelopedData->recipientInfos;

    default:
        return NULL;
    }
}

void ossl_cms_RecipientInfos_set_cmsctx(CMS_ContentInfo *cms)
{
    int i;
    CMS_RecipientInfo *ri;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);
    STACK_OF(CMS_RecipientInfo) *rinfos = CMS_get0_RecipientInfos(cms);

    for (i = 0; i < sk_CMS_RecipientInfo_num(rinfos); i++) {
        ri = sk_CMS_RecipientInfo_value(rinfos, i);
        if (ri != NULL) {
            switch (ri->type) {
            case CMS_RECIPINFO_AGREE:
                ri->d.kari->cms_ctx = ctx;
                break;
            case CMS_RECIPINFO_TRANS:
                ri->d.ktri->cms_ctx = ctx;
                ossl_x509_set0_libctx(ri->d.ktri->recip,
                                      ossl_cms_ctx_get0_libctx(ctx),
                                      ossl_cms_ctx_get0_propq(ctx));
                break;
            case CMS_RECIPINFO_KEK:
                ri->d.kekri->cms_ctx = ctx;
                break;
            case CMS_RECIPINFO_PASS:
                ri->d.pwri->cms_ctx = ctx;
                break;
            default:
                break;
            }
        }
    }
}

int CMS_RecipientInfo_type(CMS_RecipientInfo *ri)
{
    return ri->type;
}

EVP_PKEY_CTX *CMS_RecipientInfo_get0_pkey_ctx(CMS_RecipientInfo *ri)
{
    if (ri->type == CMS_RECIPINFO_TRANS)
        return ri->d.ktri->pctx;
    else if (ri->type == CMS_RECIPINFO_AGREE)
        return ri->d.kari->pctx;
    return NULL;
}

CMS_ContentInfo *CMS_EnvelopedData_create_ex(const EVP_CIPHER *cipher,
                                             OSSL_LIB_CTX *libctx,
                                             const char *propq)
{
    CMS_ContentInfo *cms;
    CMS_EnvelopedData *env;

    cms = CMS_ContentInfo_new_ex(libctx, propq);
    if (cms == NULL)
        goto err;
    env = cms_enveloped_data_init(cms);
    if (env == NULL)
        goto err;

    if (!ossl_cms_EncryptedContent_init(env->encryptedContentInfo, cipher, NULL,
                                        0, ossl_cms_get0_cmsctx(cms)))
        goto err;
    return cms;
 err:
    CMS_ContentInfo_free(cms);
    ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
    return NULL;
}

CMS_ContentInfo *CMS_EnvelopedData_create(const EVP_CIPHER *cipher)
{
    return CMS_EnvelopedData_create_ex(cipher, NULL, NULL);
}

BIO *CMS_EnvelopedData_decrypt(CMS_EnvelopedData *env, BIO *detached_data,
                               EVP_PKEY *pkey, X509 *cert,
                               ASN1_OCTET_STRING *secret, unsigned int flags,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *ci;
    BIO *bio = NULL;
    int res = 0;

    if (env == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((ci = CMS_ContentInfo_new_ex(libctx, propq)) == NULL
            || (bio = BIO_new(BIO_s_mem())) == NULL)
        goto end;
    ci->contentType = OBJ_nid2obj(NID_pkcs7_enveloped);
    ci->d.envelopedData = env;
    if (secret != NULL
        && CMS_decrypt_set1_password(ci, (unsigned char *)
                                     ASN1_STRING_get0_data(secret),
                                     ASN1_STRING_length(secret)) != 1)
        goto end;
    res = CMS_decrypt(ci, secret == NULL ? pkey : NULL,
                      secret == NULL ? cert : NULL, detached_data, bio, flags);

 end:
    if (ci != NULL) {
        ci->d.envelopedData = NULL; /* do not indirectly free |env| */
        ci->contentType = NULL;
    }
    CMS_ContentInfo_free(ci);
    if (!res) {
        BIO_free(bio);
        bio = NULL;
    }
    return bio;
}

CMS_ContentInfo *
CMS_AuthEnvelopedData_create_ex(const EVP_CIPHER *cipher, OSSL_LIB_CTX *libctx,
                                const char *propq)
{
    CMS_ContentInfo *cms;
    CMS_AuthEnvelopedData *aenv;

    cms = CMS_ContentInfo_new_ex(libctx, propq);
    if (cms == NULL)
        goto merr;
    aenv = cms_auth_enveloped_data_init(cms);
    if (aenv == NULL)
        goto merr;
    if (!ossl_cms_EncryptedContent_init(aenv->authEncryptedContentInfo,
                                        cipher, NULL, 0,
                                        ossl_cms_get0_cmsctx(cms)))
        goto merr;
    return cms;
 merr:
    CMS_ContentInfo_free(cms);
    ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
    return NULL;
}


CMS_ContentInfo *CMS_AuthEnvelopedData_create(const EVP_CIPHER *cipher)
{
    return CMS_AuthEnvelopedData_create_ex(cipher, NULL, NULL);
}

/* Key Transport Recipient Info (KTRI) routines */

/* Initialise a ktri based on passed certificate and key */

static int cms_RecipientInfo_ktri_init(CMS_RecipientInfo *ri, X509 *recip,
                                       EVP_PKEY *pk, unsigned int flags,
                                       const CMS_CTX *ctx)
{
    CMS_KeyTransRecipientInfo *ktri;
    int idtype;

    ri->d.ktri = M_ASN1_new_of(CMS_KeyTransRecipientInfo);
    if (!ri->d.ktri)
        return 0;
    ri->type = CMS_RECIPINFO_TRANS;

    ktri = ri->d.ktri;
    ktri->cms_ctx = ctx;

    if (flags & CMS_USE_KEYID) {
        ktri->version = 2;
        idtype = CMS_RECIPINFO_KEYIDENTIFIER;
    } else {
        ktri->version = 0;
        idtype = CMS_RECIPINFO_ISSUER_SERIAL;
    }

    /*
     * Not a typo: RecipientIdentifier and SignerIdentifier are the same
     * structure.
     */

    if (!ossl_cms_set1_SignerIdentifier(ktri->rid, recip, idtype, ctx))
        return 0;

    if (!X509_up_ref(recip))
        return 0;
    if (!EVP_PKEY_up_ref(pk)) {
        X509_free(recip);
        return 0;
    }

    ktri->pkey = pk;
    ktri->recip = recip;

    if (flags & CMS_KEY_PARAM) {
        ktri->pctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(ctx),
                                                ktri->pkey,
                                                ossl_cms_ctx_get0_propq(ctx));
        if (ktri->pctx == NULL)
            return 0;
        if (EVP_PKEY_encrypt_init(ktri->pctx) <= 0)
            return 0;
    } else if (!ossl_cms_env_asn1_ctrl(ri, 0))
        return 0;
    return 1;
}

/*
 * Add a recipient certificate using appropriate type of RecipientInfo
 */

CMS_RecipientInfo *CMS_add1_recipient(CMS_ContentInfo *cms, X509 *recip,
                                      EVP_PKEY *originatorPrivKey,
                                      X509 *originator, unsigned int flags)
{
    CMS_RecipientInfo *ri = NULL;
    STACK_OF(CMS_RecipientInfo) *ris;
    EVP_PKEY *pk = NULL;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    ris = CMS_get0_RecipientInfos(cms);
    if (ris == NULL)
        goto err;

    /* Initialize recipient info */
    ri = M_ASN1_new_of(CMS_RecipientInfo);
    if (ri == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }

    pk = X509_get0_pubkey(recip);
    if (pk == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_GETTING_PUBLIC_KEY);
        goto err;
    }

    switch (ossl_cms_pkey_get_ri_type(pk)) {

    case CMS_RECIPINFO_TRANS:
        if (!cms_RecipientInfo_ktri_init(ri, recip, pk, flags, ctx))
            goto err;
        break;

    case CMS_RECIPINFO_AGREE:
        if (!ossl_cms_RecipientInfo_kari_init(ri, recip, pk, originator,
                                              originatorPrivKey, flags, ctx))
            goto err;
        break;

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
        goto err;

    }

    if (!sk_CMS_RecipientInfo_push(ris, ri)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
        goto err;
    }

    return ri;

 err:
    M_ASN1_free_of(ri, CMS_RecipientInfo);
    return NULL;

}

CMS_RecipientInfo *CMS_add1_recipient_cert(CMS_ContentInfo *cms, X509 *recip,
                                           unsigned int flags)
{
     return CMS_add1_recipient(cms, recip, NULL, NULL, flags);
}

int CMS_RecipientInfo_ktri_get0_algs(CMS_RecipientInfo *ri,
                                     EVP_PKEY **pk, X509 **recip,
                                     X509_ALGOR **palg)
{
    CMS_KeyTransRecipientInfo *ktri;
    if (ri->type != CMS_RECIPINFO_TRANS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_TRANSPORT);
        return 0;
    }

    ktri = ri->d.ktri;

    if (pk)
        *pk = ktri->pkey;
    if (recip)
        *recip = ktri->recip;
    if (palg)
        *palg = ktri->keyEncryptionAlgorithm;
    return 1;
}

int CMS_RecipientInfo_ktri_get0_signer_id(CMS_RecipientInfo *ri,
                                          ASN1_OCTET_STRING **keyid,
                                          X509_NAME **issuer,
                                          ASN1_INTEGER **sno)
{
    CMS_KeyTransRecipientInfo *ktri;
    if (ri->type != CMS_RECIPINFO_TRANS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_TRANSPORT);
        return 0;
    }
    ktri = ri->d.ktri;

    return ossl_cms_SignerIdentifier_get0_signer_id(ktri->rid, keyid, issuer,
                                                    sno);
}

int CMS_RecipientInfo_ktri_cert_cmp(CMS_RecipientInfo *ri, X509 *cert)
{
    if (ri->type != CMS_RECIPINFO_TRANS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_TRANSPORT);
        return -2;
    }
    return ossl_cms_SignerIdentifier_cert_cmp(ri->d.ktri->rid, cert);
}

int CMS_RecipientInfo_set0_pkey(CMS_RecipientInfo *ri, EVP_PKEY *pkey)
{
    if (ri->type != CMS_RECIPINFO_TRANS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_TRANSPORT);
        return 0;
    }
    EVP_PKEY_free(ri->d.ktri->pkey);
    ri->d.ktri->pkey = pkey;
    return 1;
}

/* Encrypt content key in key transport recipient info */

static int cms_RecipientInfo_ktri_encrypt(const CMS_ContentInfo *cms,
                                          CMS_RecipientInfo *ri)
{
    CMS_KeyTransRecipientInfo *ktri;
    CMS_EncryptedContentInfo *ec;
    EVP_PKEY_CTX *pctx;
    unsigned char *ek = NULL;
    size_t eklen;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    int ret = 0;

    if (ri->type != CMS_RECIPINFO_TRANS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEY_TRANSPORT);
        return 0;
    }
    ktri = ri->d.ktri;
    ec = ossl_cms_get0_env_enc_content(cms);

    pctx = ktri->pctx;

    if (pctx) {
        if (!ossl_cms_env_asn1_ctrl(ri, 0))
            goto err;
    } else {
        pctx = EVP_PKEY_CTX_new_from_pkey(ossl_cms_ctx_get0_libctx(ctx),
                                          ktri->pkey,
                                          ossl_cms_ctx_get0_propq(ctx));
        if (pctx == NULL)
            return 0;

        if (EVP_PKEY_encrypt_init(pctx) <= 0)
            goto err;
    }

    if (EVP_PKEY_encrypt(pctx, NULL, &eklen, ec->key, ec->keylen) <= 0)
        goto err;

    ek = OPENSSL_malloc(eklen);
    if (ek == NULL)
        goto err;

    if (EVP_PKEY_encrypt(pctx, ek, &eklen, ec->key, ec->keylen) <= 0)
        goto err;

    ASN1_STRING_set0(ktri->encryptedKey, ek, eklen);
    ek = NULL;

    ret = 1;

 err:
    EVP_PKEY_CTX_free(pctx);
    ktri->pctx = NULL;
    OPENSSL_free(ek);
    return ret;
}

/* Decrypt content key from KTRI */

static int cms_RecipientInfo_ktri_decrypt(CMS_ContentInfo *cms,
                                          CMS_RecipientInfo *ri)
{
    CMS_KeyTransRecipientInfo *ktri = ri->d.ktri;
    EVP_PKEY *pkey = ktri->pkey;
    unsigned char *ek = NULL;
    size_t eklen;
    int ret = 0;
    size_t fixlen = 0;
    const EVP_CIPHER *cipher = NULL;
    EVP_CIPHER *fetched_cipher = NULL;
    CMS_EncryptedContentInfo *ec;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);
    OSSL_LIB_CTX *libctx = ossl_cms_ctx_get0_libctx(ctx);
    const char *propq = ossl_cms_ctx_get0_propq(ctx);

    ec = ossl_cms_get0_env_enc_content(cms);

    if (ktri->pkey == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_PRIVATE_KEY);
        return 0;
    }

    if (cms->d.envelopedData->encryptedContentInfo->havenocert
            && !cms->d.envelopedData->encryptedContentInfo->debug) {
        X509_ALGOR *calg = ec->contentEncryptionAlgorithm;
        char name[OSSL_MAX_NAME_SIZE];

        OBJ_obj2txt(name, sizeof(name), calg->algorithm, 0);

        (void)ERR_set_mark();
        fetched_cipher = EVP_CIPHER_fetch(libctx, name, propq);

        if (fetched_cipher != NULL)
            cipher = fetched_cipher;
        else
            cipher = EVP_get_cipherbyobj(calg->algorithm);
        if (cipher == NULL) {
            (void)ERR_clear_last_mark();
            ERR_raise(ERR_LIB_CMS, CMS_R_UNKNOWN_CIPHER);
            return 0;
        }
        (void)ERR_pop_to_mark();

        fixlen = EVP_CIPHER_get_key_length(cipher);
        EVP_CIPHER_free(fetched_cipher);
    }

    ktri->pctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propq);
    if (ktri->pctx == NULL)
        goto err;

    if (EVP_PKEY_decrypt_init(ktri->pctx) <= 0)
        goto err;

    if (!ossl_cms_env_asn1_ctrl(ri, 1))
        goto err;

    if (EVP_PKEY_is_a(pkey, "RSA"))
        /* upper layer CMS code incorrectly assumes that a successful RSA
         * decryption means that the key matches ciphertext (which never
         * was the case, implicit rejection or not), so to make it work
         * disable implicit rejection for RSA keys */
        EVP_PKEY_CTX_ctrl_str(ktri->pctx, "rsa_pkcs1_implicit_rejection", "0");

    if (evp_pkey_decrypt_alloc(ktri->pctx, &ek, &eklen, fixlen,
                               ktri->encryptedKey->data,
                               ktri->encryptedKey->length) <= 0)
        goto err;

    ret = 1;

    OPENSSL_clear_free(ec->key, ec->keylen);
    ec->key = ek;
    ec->keylen = eklen;

 err:
    EVP_PKEY_CTX_free(ktri->pctx);
    ktri->pctx = NULL;
    if (!ret)
        OPENSSL_free(ek);

    return ret;
}

/* Key Encrypted Key (KEK) RecipientInfo routines */

int CMS_RecipientInfo_kekri_id_cmp(CMS_RecipientInfo *ri,
                                   const unsigned char *id, size_t idlen)
{
    ASN1_OCTET_STRING tmp_os;
    CMS_KEKRecipientInfo *kekri;
    if (ri->type != CMS_RECIPINFO_KEK) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEK);
        return -2;
    }
    kekri = ri->d.kekri;
    tmp_os.type = V_ASN1_OCTET_STRING;
    tmp_os.flags = 0;
    tmp_os.data = (unsigned char *)id;
    tmp_os.length = (int)idlen;
    return ASN1_OCTET_STRING_cmp(&tmp_os, kekri->kekid->keyIdentifier);
}

/* For now hard code AES key wrap info */

static size_t aes_wrap_keylen(int nid)
{
    switch (nid) {
    case NID_id_aes128_wrap:
        return 16;

    case NID_id_aes192_wrap:
        return 24;

    case NID_id_aes256_wrap:
        return 32;

    default:
        return 0;
    }
}

CMS_RecipientInfo *CMS_add0_recipient_key(CMS_ContentInfo *cms, int nid,
                                          unsigned char *key, size_t keylen,
                                          unsigned char *id, size_t idlen,
                                          ASN1_GENERALIZEDTIME *date,
                                          ASN1_OBJECT *otherTypeId,
                                          ASN1_TYPE *otherType)
{
    CMS_RecipientInfo *ri = NULL;
    CMS_KEKRecipientInfo *kekri;
    STACK_OF(CMS_RecipientInfo) *ris = CMS_get0_RecipientInfos(cms);

    if (ris == NULL)
        goto err;

    if (nid == NID_undef) {
        switch (keylen) {
        case 16:
            nid = NID_id_aes128_wrap;
            break;

        case 24:
            nid = NID_id_aes192_wrap;
            break;

        case 32:
            nid = NID_id_aes256_wrap;
            break;

        default:
            ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_LENGTH);
            goto err;
        }

    } else {

        size_t exp_keylen = aes_wrap_keylen(nid);

        if (!exp_keylen) {
            ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_KEK_ALGORITHM);
            goto err;
        }

        if (keylen != exp_keylen) {
            ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_LENGTH);
            goto err;
        }

    }

    /* Initialize recipient info */
    ri = M_ASN1_new_of(CMS_RecipientInfo);
    if (!ri) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }

    ri->d.kekri = M_ASN1_new_of(CMS_KEKRecipientInfo);
    if (!ri->d.kekri) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    ri->type = CMS_RECIPINFO_KEK;

    kekri = ri->d.kekri;

    if (otherTypeId) {
        kekri->kekid->other = M_ASN1_new_of(CMS_OtherKeyAttribute);
        if (kekri->kekid->other == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            goto err;
        }
    }

    if (!sk_CMS_RecipientInfo_push(ris, ri)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /* After this point no calls can fail */

    kekri->version = 4;

    kekri->key = key;
    kekri->keylen = keylen;

    ASN1_STRING_set0(kekri->kekid->keyIdentifier, id, idlen);

    kekri->kekid->date = date;

    if (kekri->kekid->other) {
        kekri->kekid->other->keyAttrId = otherTypeId;
        kekri->kekid->other->keyAttr = otherType;
    }

    (void)X509_ALGOR_set0(kekri->keyEncryptionAlgorithm, OBJ_nid2obj(nid),
                          V_ASN1_UNDEF, NULL); /* cannot fail */

    return ri;

 err:
    M_ASN1_free_of(ri, CMS_RecipientInfo);
    return NULL;
}

int CMS_RecipientInfo_kekri_get0_id(CMS_RecipientInfo *ri,
                                    X509_ALGOR **palg,
                                    ASN1_OCTET_STRING **pid,
                                    ASN1_GENERALIZEDTIME **pdate,
                                    ASN1_OBJECT **potherid,
                                    ASN1_TYPE **pothertype)
{
    CMS_KEKIdentifier *rkid;
    if (ri->type != CMS_RECIPINFO_KEK) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEK);
        return 0;
    }
    rkid = ri->d.kekri->kekid;
    if (palg)
        *palg = ri->d.kekri->keyEncryptionAlgorithm;
    if (pid)
        *pid = rkid->keyIdentifier;
    if (pdate)
        *pdate = rkid->date;
    if (potherid) {
        if (rkid->other)
            *potherid = rkid->other->keyAttrId;
        else
            *potherid = NULL;
    }
    if (pothertype) {
        if (rkid->other)
            *pothertype = rkid->other->keyAttr;
        else
            *pothertype = NULL;
    }
    return 1;
}

int CMS_RecipientInfo_set0_key(CMS_RecipientInfo *ri,
                               unsigned char *key, size_t keylen)
{
    CMS_KEKRecipientInfo *kekri;
    if (ri->type != CMS_RECIPINFO_KEK) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_KEK);
        return 0;
    }

    kekri = ri->d.kekri;
    kekri->key = key;
    kekri->keylen = keylen;
    return 1;
}

static EVP_CIPHER *cms_get_key_wrap_cipher(size_t keylen, const CMS_CTX *ctx)
{
    const char *alg = NULL;

    switch (keylen) {
    case 16:
        alg = "AES-128-WRAP";
        break;
    case 24:
        alg = "AES-192-WRAP";
        break;
    case 32:
        alg = "AES-256-WRAP";
        break;
    default:
        return NULL;
    }
    return EVP_CIPHER_fetch(ossl_cms_ctx_get0_libctx(ctx), alg,
                            ossl_cms_ctx_get0_propq(ctx));
}


/* Encrypt content key in KEK recipient info */

static int cms_RecipientInfo_kekri_encrypt(const CMS_ContentInfo *cms,
                                           CMS_RecipientInfo *ri)
{
    CMS_EncryptedContentInfo *ec;
    CMS_KEKRecipientInfo *kekri;
    unsigned char *wkey = NULL;
    int wkeylen;
    int r = 0;
    EVP_CIPHER *cipher = NULL;
    int outlen = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    const CMS_CTX *cms_ctx = ossl_cms_get0_cmsctx(cms);

    ec = ossl_cms_get0_env_enc_content(cms);
    if (ec == NULL)
        return 0;

    kekri = ri->d.kekri;

    if (kekri->key == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_KEY);
        return 0;
    }

    cipher = cms_get_key_wrap_cipher(kekri->keylen, cms_ctx);
    if (cipher == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_LENGTH);
        goto err;
    }

    /* 8 byte prefix for AES wrap ciphers */
    wkey = OPENSSL_malloc(ec->keylen + 8);
    if (wkey == NULL)
        goto err;

    ctx = EVP_CIPHER_CTX_new();
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    EVP_CIPHER_CTX_set_flags(ctx, EVP_CIPHER_CTX_FLAG_WRAP_ALLOW);
    if (!EVP_EncryptInit_ex(ctx, cipher, NULL, kekri->key, NULL)
            || !EVP_EncryptUpdate(ctx, wkey, &wkeylen, ec->key, ec->keylen)
            || !EVP_EncryptFinal_ex(ctx, wkey + wkeylen, &outlen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_WRAP_ERROR);
        goto err;
    }
    wkeylen += outlen;
    if (!ossl_assert((size_t)wkeylen == ec->keylen + 8)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_WRAP_ERROR);
        goto err;
    }

    ASN1_STRING_set0(kekri->encryptedKey, wkey, wkeylen);

    r = 1;

 err:
    EVP_CIPHER_free(cipher);
    if (!r)
        OPENSSL_free(wkey);
    EVP_CIPHER_CTX_free(ctx);

    return r;
}

/* Decrypt content key in KEK recipient info */

static int cms_RecipientInfo_kekri_decrypt(CMS_ContentInfo *cms,
                                           CMS_RecipientInfo *ri)
{
    CMS_EncryptedContentInfo *ec;
    CMS_KEKRecipientInfo *kekri;
    unsigned char *ukey = NULL;
    int ukeylen;
    int r = 0, wrap_nid;
    EVP_CIPHER *cipher = NULL;
    int outlen = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    const CMS_CTX *cms_ctx = ossl_cms_get0_cmsctx(cms);

    ec = ossl_cms_get0_env_enc_content(cms);
    if (ec == NULL)
        return 0;

    kekri = ri->d.kekri;

    if (!kekri->key) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_KEY);
        return 0;
    }

    wrap_nid = OBJ_obj2nid(kekri->keyEncryptionAlgorithm->algorithm);
    if (aes_wrap_keylen(wrap_nid) != kekri->keylen) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_LENGTH);
        return 0;
    }

    /* If encrypted key length is invalid don't bother */

    if (kekri->encryptedKey->length < 16) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_ENCRYPTED_KEY_LENGTH);
        goto err;
    }

    cipher = cms_get_key_wrap_cipher(kekri->keylen, cms_ctx);
    if (cipher == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_LENGTH);
        goto err;
    }

    ukey = OPENSSL_malloc(kekri->encryptedKey->length - 8);
    if (ukey == NULL)
        goto err;

    ctx = EVP_CIPHER_CTX_new();
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    if (!EVP_DecryptInit_ex(ctx, cipher, NULL, kekri->key, NULL)
            || !EVP_DecryptUpdate(ctx, ukey, &ukeylen,
                                  kekri->encryptedKey->data,
                                  kekri->encryptedKey->length)
            || !EVP_DecryptFinal_ex(ctx, ukey + ukeylen, &outlen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNWRAP_ERROR);
        goto err;
    }
    ukeylen += outlen;

    OPENSSL_clear_free(ec->key, ec->keylen);
    ec->key = ukey;
    ec->keylen = ukeylen;

    r = 1;

 err:
    EVP_CIPHER_free(cipher);
    if (!r)
        OPENSSL_free(ukey);
    EVP_CIPHER_CTX_free(ctx);

    return r;
}

int CMS_RecipientInfo_decrypt(CMS_ContentInfo *cms, CMS_RecipientInfo *ri)
{
    switch (ri->type) {
    case CMS_RECIPINFO_TRANS:
        return cms_RecipientInfo_ktri_decrypt(cms, ri);

    case CMS_RECIPINFO_KEK:
        return cms_RecipientInfo_kekri_decrypt(cms, ri);

    case CMS_RECIPINFO_PASS:
        return ossl_cms_RecipientInfo_pwri_crypt(cms, ri, 0);

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_RECIPIENTINFO_TYPE);
        return 0;
    }
}

int CMS_RecipientInfo_encrypt(const CMS_ContentInfo *cms, CMS_RecipientInfo *ri)
{
    switch (ri->type) {
    case CMS_RECIPINFO_TRANS:
        return cms_RecipientInfo_ktri_encrypt(cms, ri);

    case CMS_RECIPINFO_AGREE:
        return ossl_cms_RecipientInfo_kari_encrypt(cms, ri);

    case CMS_RECIPINFO_KEK:
        return cms_RecipientInfo_kekri_encrypt(cms, ri);

    case CMS_RECIPINFO_PASS:
        return ossl_cms_RecipientInfo_pwri_crypt(cms, ri, 1);

    default:
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_RECIPIENT_TYPE);
        return 0;
    }
}

/* Check structures and fixup version numbers (if necessary) */

static void cms_env_set_originfo_version(CMS_EnvelopedData *env)
{
    CMS_OriginatorInfo *org = env->originatorInfo;
    int i;
    if (org == NULL)
        return;
    for (i = 0; i < sk_CMS_CertificateChoices_num(org->certificates); i++) {
        CMS_CertificateChoices *cch;
        cch = sk_CMS_CertificateChoices_value(org->certificates, i);
        if (cch->type == CMS_CERTCHOICE_OTHER) {
            env->version = 4;
            return;
        } else if (cch->type == CMS_CERTCHOICE_V2ACERT) {
            if (env->version < 3)
                env->version = 3;
        }
    }

    for (i = 0; i < sk_CMS_RevocationInfoChoice_num(org->crls); i++) {
        CMS_RevocationInfoChoice *rch;
        rch = sk_CMS_RevocationInfoChoice_value(org->crls, i);
        if (rch->type == CMS_REVCHOICE_OTHER) {
            env->version = 4;
            return;
        }
    }
}

static void cms_env_set_version(CMS_EnvelopedData *env)
{
    int i;
    CMS_RecipientInfo *ri;

    /*
     * Can't set version higher than 4 so if 4 or more already nothing to do.
     */
    if (env->version >= 4)
        return;

    cms_env_set_originfo_version(env);

    if (env->version >= 3)
        return;

    for (i = 0; i < sk_CMS_RecipientInfo_num(env->recipientInfos); i++) {
        ri = sk_CMS_RecipientInfo_value(env->recipientInfos, i);
        if (ri->type == CMS_RECIPINFO_PASS || ri->type == CMS_RECIPINFO_OTHER) {
            env->version = 3;
            return;
        } else if (ri->type != CMS_RECIPINFO_TRANS
                   || ri->d.ktri->version != 0) {
            env->version = 2;
        }
    }
    if (env->originatorInfo || env->unprotectedAttrs)
        env->version = 2;
    if (env->version == 2)
        return;
    env->version = 0;
}

static int cms_env_encrypt_content_key(const CMS_ContentInfo *cms,
                                       STACK_OF(CMS_RecipientInfo) *ris)
{
    int i;
    CMS_RecipientInfo *ri;

    for (i = 0; i < sk_CMS_RecipientInfo_num(ris); i++) {
        ri = sk_CMS_RecipientInfo_value(ris, i);
        if (CMS_RecipientInfo_encrypt(cms, ri) <= 0)
            return -1;
    }
    return 1;
}

static void cms_env_clear_ec(CMS_EncryptedContentInfo *ec)
{
    ec->cipher = NULL;
    OPENSSL_clear_free(ec->key, ec->keylen);
    ec->key = NULL;
    ec->keylen = 0;
}

static BIO *cms_EnvelopedData_Decryption_init_bio(CMS_ContentInfo *cms)
{
    CMS_EncryptedContentInfo *ec = cms->d.envelopedData->encryptedContentInfo;
    BIO *contentBio = ossl_cms_EncryptedContent_init_bio(ec,
                                                         ossl_cms_get0_cmsctx(cms));
    EVP_CIPHER_CTX *ctx = NULL;

    if (contentBio == NULL)
        return NULL;

    BIO_get_cipher_ctx(contentBio, &ctx);
    if (ctx == NULL) {
        BIO_free(contentBio);
        return NULL;
    }
    /*
     * If the selected cipher supports unprotected attributes,
     * deal with it using special ctrl function
     */
    if ((EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(ctx))
                & EVP_CIPH_FLAG_CIPHER_WITH_MAC) != 0
         && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_PROCESS_UNPROTECTED, 0,
                                cms->d.envelopedData->unprotectedAttrs) <= 0) {
        BIO_free(contentBio);
        return NULL;
    }
    return contentBio;
}

static BIO *cms_EnvelopedData_Encryption_init_bio(CMS_ContentInfo *cms)
{
    CMS_EncryptedContentInfo *ec;
    STACK_OF(CMS_RecipientInfo) *rinfos;
    int ok = 0;
    BIO *ret;
    CMS_EnvelopedData *env = cms->d.envelopedData;

    /* Get BIO first to set up key */

    ec = env->encryptedContentInfo;
    ret = ossl_cms_EncryptedContent_init_bio(ec, ossl_cms_get0_cmsctx(cms));

    /* If error end of processing */
    if (!ret)
        return ret;

    /* Now encrypt content key according to each RecipientInfo type */
    rinfos = env->recipientInfos;
    if (cms_env_encrypt_content_key(cms, rinfos) < 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_SETTING_RECIPIENTINFO);
        goto err;
    }

    /* And finally set the version */
    cms_env_set_version(env);

    ok = 1;

 err:
    cms_env_clear_ec(ec);
    if (ok)
        return ret;
    BIO_free(ret);
    return NULL;
}

BIO *ossl_cms_EnvelopedData_init_bio(CMS_ContentInfo *cms)
{
    if (cms->d.envelopedData->encryptedContentInfo->cipher != NULL) {
         /* If cipher is set it's encryption */
         return cms_EnvelopedData_Encryption_init_bio(cms);
    }

    /* If cipher is not set it's decryption */
    return cms_EnvelopedData_Decryption_init_bio(cms);
}

BIO *ossl_cms_AuthEnvelopedData_init_bio(CMS_ContentInfo *cms)
{
    CMS_EncryptedContentInfo *ec;
    STACK_OF(CMS_RecipientInfo) *rinfos;
    int ok = 0;
    BIO *ret;
    CMS_AuthEnvelopedData *aenv = cms->d.authEnvelopedData;

    /* Get BIO first to set up key */
    ec = aenv->authEncryptedContentInfo;
    /* Set tag for decryption */
    if (ec->cipher == NULL) {
        ec->tag = aenv->mac->data;
        ec->taglen = aenv->mac->length;
    }
    ret = ossl_cms_EncryptedContent_init_bio(ec, ossl_cms_get0_cmsctx(cms));

    /* If error or no cipher end of processing */
    if (ret == NULL || ec->cipher == NULL)
        return ret;

    /* Now encrypt content key according to each RecipientInfo type */
    rinfos = aenv->recipientInfos;
    if (cms_env_encrypt_content_key(cms, rinfos) < 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_ERROR_SETTING_RECIPIENTINFO);
        goto err;
    }

    /* And finally set the version */
    aenv->version = 0;

    ok = 1;

 err:
    cms_env_clear_ec(ec);
    if (ok)
        return ret;
    BIO_free(ret);
    return NULL;
}

int ossl_cms_EnvelopedData_final(CMS_ContentInfo *cms, BIO *chain)
{
    CMS_EnvelopedData *env = NULL;
    EVP_CIPHER_CTX *ctx = NULL;
    BIO *mbio = BIO_find_type(chain, BIO_TYPE_CIPHER);

    env = ossl_cms_get0_enveloped(cms);
    if (env == NULL)
        return 0;

    if (mbio == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_NOT_FOUND);
        return 0;
    }

    BIO_get_cipher_ctx(mbio, &ctx);

    /*
     * If the selected cipher supports unprotected attributes,
     * deal with it using special ctrl function
     */
    if ((EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(ctx))
            & EVP_CIPH_FLAG_CIPHER_WITH_MAC) != 0) {
        if (env->unprotectedAttrs == NULL)
            env->unprotectedAttrs = sk_X509_ATTRIBUTE_new_null();

        if (env->unprotectedAttrs == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
            return 0;
        }

        if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_PROCESS_UNPROTECTED,
                                1, env->unprotectedAttrs) <= 0) {
            ERR_raise(ERR_LIB_CMS, CMS_R_CTRL_FAILURE);
            return 0;
        }
    }

    cms_env_set_version(cms->d.envelopedData);
    return 1;
}

int ossl_cms_AuthEnvelopedData_final(CMS_ContentInfo *cms, BIO *cmsbio)
{
    EVP_CIPHER_CTX *ctx;
    unsigned char *tag = NULL;
    int taglen, ok = 0;

    BIO_get_cipher_ctx(cmsbio, &ctx);

    /*
     * The tag is set only for encryption. There is nothing to do for
     * decryption.
     */
    if (!EVP_CIPHER_CTX_is_encrypting(ctx))
        return 1;

    taglen = EVP_CIPHER_CTX_get_tag_length(ctx);
    if (taglen <= 0
            || (tag = OPENSSL_malloc(taglen)) == NULL
            || EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG, taglen,
                                   tag) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CIPHER_GET_TAG);
        goto err;
    }

    if (!ASN1_OCTET_STRING_set(cms->d.authEnvelopedData->mac, tag, taglen))
        goto err;

    ok = 1;
err:
    OPENSSL_free(tag);
    return ok;
}

/*
 * Get RecipientInfo type (if any) supported by a key (public or private). To
 * retain compatibility with previous behaviour if the ctrl value isn't
 * supported we assume key transport.
 */
int ossl_cms_pkey_get_ri_type(EVP_PKEY *pk)
{
    /* Check types that we know about */
    if (EVP_PKEY_is_a(pk, "DH"))
        return CMS_RECIPINFO_AGREE;
    else if (EVP_PKEY_is_a(pk, "DHX"))
        return CMS_RECIPINFO_AGREE;
    else if (EVP_PKEY_is_a(pk, "DSA"))
        return CMS_RECIPINFO_NONE;
    else if (EVP_PKEY_is_a(pk, "EC"))
        return CMS_RECIPINFO_AGREE;
    else if (EVP_PKEY_is_a(pk, "RSA"))
        return CMS_RECIPINFO_TRANS;

    /*
     * Otherwise this might ben an engine implementation, so see if we can get
     * the type from the ameth.
     */
    if (pk->ameth && pk->ameth->pkey_ctrl) {
        int i, r;
        i = pk->ameth->pkey_ctrl(pk, ASN1_PKEY_CTRL_CMS_RI_TYPE, 0, &r);
        if (i > 0)
            return r;
    }
    return CMS_RECIPINFO_TRANS;
}

int ossl_cms_pkey_is_ri_type_supported(EVP_PKEY *pk, int ri_type)
{
    int supportedRiType;

    if (pk->ameth != NULL && pk->ameth->pkey_ctrl != NULL) {
        int i, r;

        i = pk->ameth->pkey_ctrl(pk, ASN1_PKEY_CTRL_CMS_IS_RI_TYPE_SUPPORTED,
                                 ri_type, &r);
        if (i > 0)
            return r;
    }

    supportedRiType = ossl_cms_pkey_get_ri_type(pk);
    if (supportedRiType < 0)
        return 0;

    return (supportedRiType == ri_type);
}
