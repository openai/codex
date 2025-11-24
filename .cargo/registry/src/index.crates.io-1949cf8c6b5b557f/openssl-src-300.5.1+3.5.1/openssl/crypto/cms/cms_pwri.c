/*
 * Copyright 2009-2025 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/rand.h>
#include <openssl/aes.h>
#include "internal/sizes.h"
#include "crypto/asn1.h"
#include "cms_local.h"

int CMS_RecipientInfo_set0_password(CMS_RecipientInfo *ri,
                                    unsigned char *pass, ossl_ssize_t passlen)
{
    CMS_PasswordRecipientInfo *pwri;
    if (ri->type != CMS_RECIPINFO_PASS) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NOT_PWRI);
        return 0;
    }

    pwri = ri->d.pwri;
    pwri->pass = pass;
    if (pass && passlen < 0)
        passlen = strlen((char *)pass);
    pwri->passlen = passlen;
    return 1;
}

CMS_RecipientInfo *CMS_add0_recipient_password(CMS_ContentInfo *cms,
                                               int iter, int wrap_nid,
                                               int pbe_nid,
                                               unsigned char *pass,
                                               ossl_ssize_t passlen,
                                               const EVP_CIPHER *kekciph)
{
    STACK_OF(CMS_RecipientInfo) *ris;
    CMS_RecipientInfo *ri = NULL;
    CMS_EncryptedContentInfo *ec;
    CMS_PasswordRecipientInfo *pwri;
    EVP_CIPHER_CTX *ctx = NULL;
    X509_ALGOR *encalg = NULL;
    unsigned char iv[EVP_MAX_IV_LENGTH];
    int ivlen;
    const CMS_CTX *cms_ctx = ossl_cms_get0_cmsctx(cms);

    ec = ossl_cms_get0_env_enc_content(cms);
    if (ec == NULL)
        return NULL;
    ris = CMS_get0_RecipientInfos(cms);
    if (ris == NULL)
        return NULL;

    if (wrap_nid <= 0)
        wrap_nid = NID_id_alg_PWRI_KEK;

    if (pbe_nid <= 0)
        pbe_nid = NID_id_pbkdf2;

    /* Get from enveloped data */
    if (kekciph == NULL)
        kekciph = ec->cipher;

    if (kekciph == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CIPHER);
        return NULL;
    }
    if (wrap_nid != NID_id_alg_PWRI_KEK) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_KEY_ENCRYPTION_ALGORITHM);
        return NULL;
    }

    /* Setup algorithm identifier for cipher */
    encalg = X509_ALGOR_new();
    if (encalg == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    ctx = EVP_CIPHER_CTX_new();
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    if (EVP_EncryptInit_ex(ctx, kekciph, NULL, NULL, NULL) <= 0) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    ivlen = EVP_CIPHER_CTX_get_iv_length(ctx);
    if (ivlen < 0) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    if (ivlen > 0) {
        if (RAND_bytes_ex(ossl_cms_ctx_get0_libctx(cms_ctx), iv, ivlen, 0) <= 0)
            goto err;
        if (EVP_EncryptInit_ex(ctx, NULL, NULL, NULL, iv) <= 0) {
            ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
            goto err;
        }
        encalg->parameter = ASN1_TYPE_new();
        if (!encalg->parameter) {
            ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
            goto err;
        }
        if (EVP_CIPHER_param_to_asn1(ctx, encalg->parameter) <= 0) {
            ERR_raise(ERR_LIB_CMS, CMS_R_CIPHER_PARAMETER_INITIALISATION_ERROR);
            goto err;
        }
    }

    encalg->algorithm = OBJ_nid2obj(EVP_CIPHER_CTX_get_type(ctx));

    EVP_CIPHER_CTX_free(ctx);
    ctx = NULL;

    /* Initialize recipient info */
    ri = M_ASN1_new_of(CMS_RecipientInfo);
    if (ri == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }

    ri->d.pwri = M_ASN1_new_of(CMS_PasswordRecipientInfo);
    if (ri->d.pwri == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    ri->type = CMS_RECIPINFO_PASS;

    pwri = ri->d.pwri;
    pwri->cms_ctx = cms_ctx;
    /* Since this is overwritten, free up empty structure already there */
    X509_ALGOR_free(pwri->keyEncryptionAlgorithm);
    pwri->keyEncryptionAlgorithm = X509_ALGOR_new();
    if (pwri->keyEncryptionAlgorithm == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    pwri->keyEncryptionAlgorithm->algorithm = OBJ_nid2obj(wrap_nid);
    pwri->keyEncryptionAlgorithm->parameter = ASN1_TYPE_new();
    if (pwri->keyEncryptionAlgorithm->parameter == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }

    if (!ASN1_item_pack(encalg, ASN1_ITEM_rptr(X509_ALGOR),
                        &pwri->keyEncryptionAlgorithm->parameter->
                        value.sequence)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_ASN1_LIB);
        goto err;
    }
    pwri->keyEncryptionAlgorithm->parameter->type = V_ASN1_SEQUENCE;

    X509_ALGOR_free(encalg);
    encalg = NULL;

    /* Setup PBE algorithm */

    pwri->keyDerivationAlgorithm = PKCS5_pbkdf2_set_ex(iter, NULL, 0, -1, -1,
                                                       cms_ctx->libctx);

    if (pwri->keyDerivationAlgorithm == NULL)
        goto err;

    CMS_RecipientInfo_set0_password(ri, pass, passlen);
    pwri->version = 0;

    if (!sk_CMS_RecipientInfo_push(ris, ri)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CRYPTO_LIB);
        goto err;
    }

    return ri;

 err:
    EVP_CIPHER_CTX_free(ctx);
    if (ri)
        M_ASN1_free_of(ri, CMS_RecipientInfo);
    X509_ALGOR_free(encalg);
    return NULL;

}

/*
 * This is an implementation of the key wrapping mechanism in RFC3211, at
 * some point this should go into EVP.
 */

static int kek_unwrap_key(unsigned char *out, size_t *outlen,
                          const unsigned char *in, size_t inlen,
                          EVP_CIPHER_CTX *ctx)
{
    size_t blocklen = EVP_CIPHER_CTX_get_block_size(ctx);
    unsigned char *tmp;
    int outl, rv = 0;

    if (blocklen == 0)
        return 0;

    if (inlen < 2 * blocklen) {
        /* too small */
        return 0;
    }
    if (inlen % blocklen) {
        /* Invalid size */
        return 0;
    }
    if ((tmp = OPENSSL_malloc(inlen)) == NULL)
        return 0;
    /* setup IV by decrypting last two blocks */
    if (!EVP_DecryptUpdate(ctx, tmp + inlen - 2 * blocklen, &outl,
                           in + inlen - 2 * blocklen, blocklen * 2)
        /*
         * Do a decrypt of last decrypted block to set IV to correct value
         * output it to start of buffer so we don't corrupt decrypted block
         * this works because buffer is at least two block lengths long.
         */
        || !EVP_DecryptUpdate(ctx, tmp, &outl,
                              tmp + inlen - blocklen, blocklen)
        /* Can now decrypt first n - 1 blocks */
        || !EVP_DecryptUpdate(ctx, tmp, &outl, in, inlen - blocklen)

        /* Reset IV to original value */
        || !EVP_DecryptInit_ex(ctx, NULL, NULL, NULL, NULL)
        /* Decrypt again */
        || !EVP_DecryptUpdate(ctx, tmp, &outl, tmp, inlen))
        goto err;
    /* Check check bytes */
    if (((tmp[1] ^ tmp[4]) & (tmp[2] ^ tmp[5]) & (tmp[3] ^ tmp[6])) != 0xff) {
        /* Check byte failure */
        goto err;
    }
    if (inlen < (size_t)(tmp[0] - 4)) {
        /* Invalid length value */
        goto err;
    }
    *outlen = (size_t)tmp[0];
    memcpy(out, tmp + 4, *outlen);
    rv = 1;
 err:
    OPENSSL_clear_free(tmp, inlen);
    return rv;

}

static int kek_wrap_key(unsigned char *out, size_t *outlen,
                        const unsigned char *in, size_t inlen,
                        EVP_CIPHER_CTX *ctx, const CMS_CTX *cms_ctx)
{
    size_t blocklen = EVP_CIPHER_CTX_get_block_size(ctx);
    size_t olen;
    int dummy;

    if (blocklen == 0)
        return 0;

    /*
     * First decide length of output buffer: need header and round up to
     * multiple of block length.
     */
    olen = (inlen + 4 + blocklen - 1) / blocklen;
    olen *= blocklen;
    if (olen < 2 * blocklen) {
        /* Key too small */
        return 0;
    }
    if (inlen > 0xFF) {
        /* Key too large */
        return 0;
    }
    if (out) {
        /* Set header */
        out[0] = (unsigned char)inlen;
        out[1] = in[0] ^ 0xFF;
        out[2] = in[1] ^ 0xFF;
        out[3] = in[2] ^ 0xFF;
        memcpy(out + 4, in, inlen);
        /* Add random padding to end */
        if (olen > inlen + 4
            && RAND_bytes_ex(ossl_cms_ctx_get0_libctx(cms_ctx), out + 4 + inlen,
                             olen - 4 - inlen, 0) <= 0)
            return 0;
        /* Encrypt twice */
        if (!EVP_EncryptUpdate(ctx, out, &dummy, out, olen)
            || !EVP_EncryptUpdate(ctx, out, &dummy, out, olen))
            return 0;
    }

    *outlen = olen;

    return 1;
}

/* Encrypt/Decrypt content key in PWRI recipient info */

int ossl_cms_RecipientInfo_pwri_crypt(const CMS_ContentInfo *cms,
                                      CMS_RecipientInfo *ri, int en_de)
{
    CMS_EncryptedContentInfo *ec;
    CMS_PasswordRecipientInfo *pwri;
    int r = 0;
    X509_ALGOR *algtmp, *kekalg = NULL;
    EVP_CIPHER_CTX *kekctx = NULL;
    char name[OSSL_MAX_NAME_SIZE];
    EVP_CIPHER *kekcipher;
    unsigned char *key = NULL;
    size_t keylen;
    const CMS_CTX *cms_ctx = ossl_cms_get0_cmsctx(cms);

    ec = ossl_cms_get0_env_enc_content(cms);

    pwri = ri->d.pwri;

    if (pwri->pass == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_PASSWORD);
        return 0;
    }
    algtmp = pwri->keyEncryptionAlgorithm;

    if (!algtmp || OBJ_obj2nid(algtmp->algorithm) != NID_id_alg_PWRI_KEK) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_KEY_ENCRYPTION_ALGORITHM);
        return 0;
    }

    kekalg = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(X509_ALGOR),
                                       algtmp->parameter);

    if (kekalg == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_INVALID_KEY_ENCRYPTION_PARAMETER);
        return 0;
    }

    OBJ_obj2txt(name, sizeof(name), kekalg->algorithm, 0);
    kekcipher = EVP_CIPHER_fetch(ossl_cms_ctx_get0_libctx(cms_ctx), name,
                                 ossl_cms_ctx_get0_propq(cms_ctx));

    if (kekcipher == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNKNOWN_CIPHER);
        goto err;
    }

    kekctx = EVP_CIPHER_CTX_new();
    if (kekctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }
    /* Fixup cipher based on AlgorithmIdentifier to set IV etc */
    if (!EVP_CipherInit_ex(kekctx, kekcipher, NULL, NULL, NULL, en_de))
        goto err;
    EVP_CIPHER_CTX_set_padding(kekctx, 0);
    if (EVP_CIPHER_asn1_to_param(kekctx, kekalg->parameter) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CIPHER_PARAMETER_INITIALISATION_ERROR);
        goto err;
    }

    algtmp = pwri->keyDerivationAlgorithm;

    /* Finish password based key derivation to setup key in "ctx" */

    if (EVP_PBE_CipherInit_ex(algtmp->algorithm,
                              (char *)pwri->pass, pwri->passlen,
                              algtmp->parameter, kekctx, en_de,
                              cms_ctx->libctx, cms_ctx->propq) < 0) {
        ERR_raise(ERR_LIB_CMS, ERR_R_EVP_LIB);
        goto err;
    }

    /* Finally wrap/unwrap the key */

    if (en_de) {

        if (!kek_wrap_key(NULL, &keylen, ec->key, ec->keylen, kekctx, cms_ctx))
            goto err;

        key = OPENSSL_malloc(keylen);

        if (key == NULL)
            goto err;

        if (!kek_wrap_key(key, &keylen, ec->key, ec->keylen, kekctx, cms_ctx))
            goto err;
        pwri->encryptedKey->data = key;
        pwri->encryptedKey->length = keylen;
    } else {
        key = OPENSSL_malloc(pwri->encryptedKey->length);
        if (key == NULL)
            goto err;
        if (!kek_unwrap_key(key, &keylen,
                            pwri->encryptedKey->data,
                            pwri->encryptedKey->length, kekctx)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_UNWRAP_FAILURE);
            goto err;
        }

        OPENSSL_clear_free(ec->key, ec->keylen);
        ec->key = key;
        ec->keylen = keylen;

    }

    r = 1;

 err:
    EVP_CIPHER_free(kekcipher);
    EVP_CIPHER_CTX_free(kekctx);

    if (!r)
        OPENSSL_free(key);
    X509_ALGOR_free(kekalg);

    return r;

}
