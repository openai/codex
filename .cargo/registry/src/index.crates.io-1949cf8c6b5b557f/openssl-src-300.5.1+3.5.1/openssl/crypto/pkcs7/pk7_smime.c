/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Simple PKCS#7 processing functions */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "crypto/x509.h"
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "pk7_local.h"

#define BUFFERSIZE 4096


static int pkcs7_copy_existing_digest(PKCS7 *p7, PKCS7_SIGNER_INFO *si);

PKCS7 *PKCS7_sign_ex(X509 *signcert, EVP_PKEY *pkey, STACK_OF(X509) *certs,
                     BIO *data, int flags, OSSL_LIB_CTX *libctx,
                     const char *propq)
{
    PKCS7 *p7;
    int i;

    if ((p7 = PKCS7_new_ex(libctx, propq)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_PKCS7_LIB);
        return NULL;
    }

    if (!PKCS7_set_type(p7, NID_pkcs7_signed))
        goto err;

    if (!PKCS7_content_new(p7, NID_pkcs7_data))
        goto err;

    if (pkey && !PKCS7_sign_add_signer(p7, signcert, pkey, NULL, flags)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_PKCS7_ADD_SIGNER_ERROR);
        goto err;
    }

    if (!(flags & PKCS7_NOCERTS)) {
        for (i = 0; i < sk_X509_num(certs); i++) {
            if (!PKCS7_add_certificate(p7, sk_X509_value(certs, i)))
                goto err;
        }
    }

    if (flags & PKCS7_DETACHED)
        PKCS7_set_detached(p7, 1);

    if (flags & (PKCS7_STREAM | PKCS7_PARTIAL))
        return p7;

    if (PKCS7_final(p7, data, flags))
        return p7;

 err:
    PKCS7_free(p7);
    return NULL;
}

PKCS7 *PKCS7_sign(X509 *signcert, EVP_PKEY *pkey, STACK_OF(X509) *certs,
                  BIO *data, int flags)
{
    return PKCS7_sign_ex(signcert, pkey, certs, data, flags, NULL, NULL);
}


int PKCS7_final(PKCS7 *p7, BIO *data, int flags)
{
    BIO *p7bio;
    int ret = 0;

    if ((p7bio = PKCS7_dataInit(p7, NULL)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_PKCS7_LIB);
        return 0;
    }

    if (!SMIME_crlf_copy(data, p7bio, flags))
        goto err;

    (void)BIO_flush(p7bio);

    if (!PKCS7_dataFinal(p7, p7bio)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_PKCS7_DATASIGN);
        goto err;
    }
    ret = 1;
err:
    BIO_free_all(p7bio);

    return ret;

}

/* Check to see if a cipher exists and if so add S/MIME capabilities */

static int add_cipher_smcap(STACK_OF(X509_ALGOR) *sk, int nid, int arg)
{
    if (EVP_get_cipherbynid(nid))
        return PKCS7_simple_smimecap(sk, nid, arg);
    return 1;
}

static int add_digest_smcap(STACK_OF(X509_ALGOR) *sk, int nid, int arg)
{
    if (EVP_get_digestbynid(nid))
        return PKCS7_simple_smimecap(sk, nid, arg);
    return 1;
}

PKCS7_SIGNER_INFO *PKCS7_sign_add_signer(PKCS7 *p7, X509 *signcert,
                                         EVP_PKEY *pkey, const EVP_MD *md,
                                         int flags)
{
    PKCS7_SIGNER_INFO *si = NULL;
    STACK_OF(X509_ALGOR) *smcap = NULL;

    if (!X509_check_private_key(signcert, pkey)) {
        ERR_raise(ERR_LIB_PKCS7,
                  PKCS7_R_PRIVATE_KEY_DOES_NOT_MATCH_CERTIFICATE);
        return NULL;
    }

    if ((si = PKCS7_add_signature(p7, signcert, pkey, md)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_PKCS7_ADD_SIGNATURE_ERROR);
        return NULL;
    }

    si->ctx = ossl_pkcs7_get0_ctx(p7);
    if (!(flags & PKCS7_NOCERTS)) {
        if (!PKCS7_add_certificate(p7, signcert))
            goto err;
    }

    if (!(flags & PKCS7_NOATTR)) {
        if (!PKCS7_add_attrib_content_type(si, NULL))
            goto err;
        /* Add SMIMECapabilities */
        if (!(flags & PKCS7_NOSMIMECAP)) {
            if ((smcap = sk_X509_ALGOR_new_null()) == NULL) {
                ERR_raise(ERR_LIB_PKCS7, ERR_R_CRYPTO_LIB);
                goto err;
            }
            if (!add_cipher_smcap(smcap, NID_aes_256_cbc, -1)
                || !add_digest_smcap(smcap, NID_id_GostR3411_2012_256, -1)
                || !add_digest_smcap(smcap, NID_id_GostR3411_2012_512, -1)
                || !add_digest_smcap(smcap, NID_id_GostR3411_94, -1)
                || !add_cipher_smcap(smcap, NID_id_Gost28147_89, -1)
                || !add_cipher_smcap(smcap, NID_aes_192_cbc, -1)
                || !add_cipher_smcap(smcap, NID_aes_128_cbc, -1)
                || !add_cipher_smcap(smcap, NID_des_ede3_cbc, -1)
                || !add_cipher_smcap(smcap, NID_rc2_cbc, 128)
                || !add_cipher_smcap(smcap, NID_rc2_cbc, 64)
                || !add_cipher_smcap(smcap, NID_des_cbc, -1)
                || !add_cipher_smcap(smcap, NID_rc2_cbc, 40)
                || !PKCS7_add_attrib_smimecap(si, smcap))
                goto err;
            sk_X509_ALGOR_pop_free(smcap, X509_ALGOR_free);
            smcap = NULL;
        }
        if (flags & PKCS7_REUSE_DIGEST) {
            if (!pkcs7_copy_existing_digest(p7, si))
                goto err;
            if (!(flags & PKCS7_PARTIAL)
                && !PKCS7_SIGNER_INFO_sign(si))
                goto err;
        }
    }
    return si;
 err:
    sk_X509_ALGOR_pop_free(smcap, X509_ALGOR_free);
    return NULL;
}

/*
 * Search for a digest matching SignerInfo digest type and if found copy
 * across.
 */

static int pkcs7_copy_existing_digest(PKCS7 *p7, PKCS7_SIGNER_INFO *si)
{
    int i;
    STACK_OF(PKCS7_SIGNER_INFO) *sinfos;
    PKCS7_SIGNER_INFO *sitmp;
    ASN1_OCTET_STRING *osdig = NULL;
    sinfos = PKCS7_get_signer_info(p7);
    for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(sinfos); i++) {
        sitmp = sk_PKCS7_SIGNER_INFO_value(sinfos, i);
        if (si == sitmp)
            break;
        if (sk_X509_ATTRIBUTE_num(sitmp->auth_attr) <= 0)
            continue;
        if (!OBJ_cmp(si->digest_alg->algorithm, sitmp->digest_alg->algorithm)) {
            osdig = PKCS7_digest_from_attributes(sitmp->auth_attr);
            break;
        }

    }

    if (osdig != NULL)
        return PKCS7_add1_attrib_digest(si, osdig->data, osdig->length);

    ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_MATCHING_DIGEST_TYPE_FOUND);
    return 0;
}

/* This strongly overlaps with CMS_verify(), partly with PKCS7_dataVerify() */
int PKCS7_verify(PKCS7 *p7, STACK_OF(X509) *certs, X509_STORE *store,
                 BIO *indata, BIO *out, int flags)
{
    STACK_OF(X509) *signers;
    STACK_OF(X509) *included_certs;
    STACK_OF(X509) *untrusted = NULL;
    X509 *signer;
    STACK_OF(PKCS7_SIGNER_INFO) *sinfos;
    PKCS7_SIGNER_INFO *si;
    X509_STORE_CTX *cert_ctx = NULL;
    char *buf = NULL;
    int i, j = 0, k, ret = 0;
    BIO *p7bio = NULL;
    BIO *tmpout = NULL;
    const PKCS7_CTX *p7_ctx;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return 0;
    }

    if (!PKCS7_type_is_signed(p7)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    /* Check for no data and no content: no data to verify signature */
    if (PKCS7_get_detached(p7) && indata == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
        return 0;
    }

    if (flags & PKCS7_NO_DUAL_CONTENT) {
        /*
         * This was originally "#if 0" because we thought that only old broken
         * Netscape did this.  It turns out that Authenticode uses this kind
         * of "extended" PKCS7 format, and things like UEFI secure boot and
         * tools like osslsigncode need it.  In Authenticode the verification
         * process is different, but the existing PKCs7 verification works.
         */
        if (!PKCS7_get_detached(p7) && indata != NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_CONTENT_AND_DATA_PRESENT);
            return 0;
        }
    }

    sinfos = PKCS7_get_signer_info(p7);

    if (!sinfos || !sk_PKCS7_SIGNER_INFO_num(sinfos)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_SIGNATURES_ON_DATA);
        return 0;
    }

    signers = PKCS7_get0_signers(p7, certs, flags);
    if (signers == NULL)
        return 0;

    /* Now verify the certificates */
    p7_ctx = ossl_pkcs7_get0_ctx(p7);
    cert_ctx = X509_STORE_CTX_new_ex(ossl_pkcs7_ctx_get0_libctx(p7_ctx),
                                     ossl_pkcs7_ctx_get0_propq(p7_ctx));
    if (cert_ctx == NULL)
        goto err;
    if ((flags & PKCS7_NOVERIFY) == 0) {
        if (!ossl_x509_add_certs_new(&untrusted, certs, X509_ADD_FLAG_NO_DUP))
            goto err;
        included_certs = pkcs7_get0_certificates(p7);
        if ((flags & PKCS7_NOCHAIN) == 0
            && !ossl_x509_add_certs_new(&untrusted, included_certs,
                                        X509_ADD_FLAG_NO_DUP))
            goto err;

        for (k = 0; k < sk_X509_num(signers); k++) {
            signer = sk_X509_value(signers, k);
            if (!X509_STORE_CTX_init(cert_ctx, store, signer, untrusted)) {
                ERR_raise(ERR_LIB_PKCS7, ERR_R_X509_LIB);
                goto err;
            }
            if ((flags & PKCS7_NOCHAIN) == 0
                    && !X509_STORE_CTX_set_default(cert_ctx, "smime_sign"))
                goto err;
            if (!(flags & PKCS7_NOCRL))
                X509_STORE_CTX_set0_crls(cert_ctx, p7->d.sign->crl);
            i = X509_verify_cert(cert_ctx);
            if (i <= 0) {
                j = X509_STORE_CTX_get_error(cert_ctx);
                ERR_raise_data(ERR_LIB_PKCS7, PKCS7_R_CERTIFICATE_VERIFY_ERROR,
                               "Verify error: %s",
                               X509_verify_cert_error_string(j));
                goto err;
            }
            /* Check for revocation status here */
        }
    }

    if ((p7bio = PKCS7_dataInit(p7, indata)) == NULL)
        goto err;

    if (flags & PKCS7_TEXT) {
        if ((tmpout = BIO_new(BIO_s_mem())) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
            goto err;
        }
        BIO_set_mem_eof_return(tmpout, 0);
    } else
        tmpout = out;

    /* We now have to 'read' from p7bio to calculate digests etc. */
    if ((buf = OPENSSL_malloc(BUFFERSIZE)) == NULL)
        goto err;
    for (;;) {
        i = BIO_read(p7bio, buf, BUFFERSIZE);
        if (i <= 0)
            break;
        if (tmpout)
            BIO_write(tmpout, buf, i);
    }

    if (flags & PKCS7_TEXT) {
        if (!SMIME_text(tmpout, out)) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SMIME_TEXT_ERROR);
            goto err;
        }
    }

    /* Now Verify All Signatures */
    if (!(flags & PKCS7_NOSIGS))
        for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(sinfos); i++) {
            si = sk_PKCS7_SIGNER_INFO_value(sinfos, i);
            signer = sk_X509_value(signers, i);
            j = PKCS7_signatureVerify(p7bio, p7, si, signer);
            if (j <= 0) {
                ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SIGNATURE_FAILURE);
                goto err;
            }
        }

    ret = 1;

 err:
    if (flags & PKCS7_TEXT)
        BIO_free(tmpout);
    X509_STORE_CTX_free(cert_ctx);
    OPENSSL_free(buf);
    if (indata != NULL)
        BIO_pop(p7bio);
    BIO_free_all(p7bio);
    sk_X509_free(signers);
    sk_X509_free(untrusted);
    return ret;
}

STACK_OF(X509) *PKCS7_get0_signers(PKCS7 *p7, STACK_OF(X509) *certs,
                                   int flags)
{
    STACK_OF(X509) *signers, *included_certs;
    STACK_OF(PKCS7_SIGNER_INFO) *sinfos;
    PKCS7_SIGNER_INFO *si;
    PKCS7_ISSUER_AND_SERIAL *ias;
    X509 *signer;
    int i;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return NULL;
    }

    if (!PKCS7_type_is_signed(p7)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return NULL;
    }
    included_certs = pkcs7_get0_certificates(p7);

    /* Collect all the signers together */

    sinfos = PKCS7_get_signer_info(p7);

    if (sk_PKCS7_SIGNER_INFO_num(sinfos) <= 0) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_SIGNERS);
        return 0;
    }

    if ((signers = sk_X509_new_null()) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_CRYPTO_LIB);
        return NULL;
    }

    for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(sinfos); i++) {
        si = sk_PKCS7_SIGNER_INFO_value(sinfos, i);
        ias = si->issuer_and_serial;
        signer = NULL;
        /* If any certificates passed they take priority */
        signer = X509_find_by_issuer_and_serial(certs,
                                                ias->issuer, ias->serial);
        if (signer == NULL && (flags & PKCS7_NOINTERN) == 0)
            signer = X509_find_by_issuer_and_serial(included_certs,
                                                    ias->issuer, ias->serial);
        if (signer == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SIGNER_CERTIFICATE_NOT_FOUND);
            sk_X509_free(signers);
            return 0;
        }

        if (!sk_X509_push(signers, signer)) {
            sk_X509_free(signers);
            return NULL;
        }
    }
    return signers;
}

/* Build a complete PKCS#7 enveloped data */

PKCS7 *PKCS7_encrypt_ex(STACK_OF(X509) *certs, BIO *in,
                        const EVP_CIPHER *cipher, int flags,
                        OSSL_LIB_CTX *libctx, const char *propq)
{
    PKCS7 *p7;
    BIO *p7bio = NULL;
    int i;
    X509 *x509;

    if ((p7 = PKCS7_new_ex(libctx, propq)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_PKCS7_LIB);
        return NULL;
    }

    if (!PKCS7_set_type(p7, NID_pkcs7_enveloped))
        goto err;
    if (!PKCS7_set_cipher(p7, cipher)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_ERROR_SETTING_CIPHER);
        goto err;
    }

    for (i = 0; i < sk_X509_num(certs); i++) {
        x509 = sk_X509_value(certs, i);
        if (!PKCS7_add_recipient(p7, x509)) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_ERROR_ADDING_RECIPIENT);
            goto err;
        }
    }

    if (flags & PKCS7_STREAM)
        return p7;

    if (PKCS7_final(p7, in, flags))
        return p7;

 err:

    BIO_free_all(p7bio);
    PKCS7_free(p7);
    return NULL;

}

PKCS7 *PKCS7_encrypt(STACK_OF(X509) *certs, BIO *in, const EVP_CIPHER *cipher,
                     int flags)
{
    return PKCS7_encrypt_ex(certs, in, cipher, flags, NULL, NULL);
}


int PKCS7_decrypt(PKCS7 *p7, EVP_PKEY *pkey, X509 *cert, BIO *data, int flags)
{
    BIO *tmpmem;
    int ret = 0, i;
    char *buf = NULL;

    if (p7 == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_INVALID_NULL_POINTER);
        return 0;
    }

    if (!PKCS7_type_is_enveloped(p7)
        && !PKCS7_type_is_signedAndEnveloped(p7)) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    if (cert && !X509_check_private_key(cert, pkey)) {
        ERR_raise(ERR_LIB_PKCS7,
                  PKCS7_R_PRIVATE_KEY_DOES_NOT_MATCH_CERTIFICATE);
        return 0;
    }

    if ((tmpmem = PKCS7_dataDecode(p7, pkey, NULL, cert)) == NULL) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_DECRYPT_ERROR);
        return 0;
    }

    if (flags & PKCS7_TEXT) {
        BIO *tmpbuf, *bread;
        /* Encrypt BIOs can't do BIO_gets() so add a buffer BIO */
        if ((tmpbuf = BIO_new(BIO_f_buffer())) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
            BIO_free_all(tmpmem);
            return 0;
        }
        if ((bread = BIO_push(tmpbuf, tmpmem)) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_BIO_LIB);
            BIO_free_all(tmpbuf);
            BIO_free_all(tmpmem);
            return 0;
        }
        ret = SMIME_text(bread, data);
        if (ret > 0 && BIO_method_type(tmpmem) == BIO_TYPE_CIPHER) {
            if (BIO_get_cipher_status(tmpmem) <= 0)
                ret = 0;
        }
        BIO_free_all(bread);
        return ret;
    }
    if ((buf = OPENSSL_malloc(BUFFERSIZE)) == NULL)
        goto err;
    for (;;) {
        i = BIO_read(tmpmem, buf, BUFFERSIZE);
        if (i <= 0) {
            ret = 1;
            if (BIO_method_type(tmpmem) == BIO_TYPE_CIPHER) {
                if (BIO_get_cipher_status(tmpmem) <= 0)
                    ret = 0;
            }

            break;
        }
        if (BIO_write(data, buf, i) != i) {
            break;
        }
    }
err:
    OPENSSL_free(buf);
    BIO_free_all(tmpmem);
    return ret;
}
