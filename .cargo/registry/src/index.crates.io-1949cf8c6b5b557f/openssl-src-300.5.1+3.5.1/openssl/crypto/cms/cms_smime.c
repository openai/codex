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
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include "cms_local.h"
#include "crypto/asn1.h"
#include "crypto/x509.h"

static BIO *cms_get_text_bio(BIO *out, unsigned int flags)
{
    BIO *rbio;

    if (out == NULL)
        rbio = BIO_new(BIO_s_null());
    else if (flags & CMS_TEXT) {
        rbio = BIO_new(BIO_s_mem());
        BIO_set_mem_eof_return(rbio, 0);
    } else
        rbio = out;
    return rbio;
}

static int cms_copy_content(BIO *out, BIO *in, unsigned int flags)
{
    unsigned char buf[4096];
    int r = 0, i;
    BIO *tmpout;

    tmpout = cms_get_text_bio(out, flags);

    if (tmpout == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }

    /* Read all content through chain to process digest, decrypt etc */
    for (;;) {
        i = BIO_read(in, buf, sizeof(buf));
        if (i <= 0) {
            if (BIO_method_type(in) == BIO_TYPE_CIPHER) {
                if (BIO_get_cipher_status(in) <= 0)
                    goto err;
            }
            if (i < 0)
                goto err;
            break;
        }

        if (tmpout != NULL && (BIO_write(tmpout, buf, i) != i))
            goto err;
    }

    if (flags & CMS_TEXT) {
        if (!SMIME_text(tmpout, out)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_SMIME_TEXT_ERROR);
            goto err;
        }
    }

    r = 1;
 err:
    if (tmpout != out)
        BIO_free(tmpout);
    return r;

}

static int check_content(CMS_ContentInfo *cms)
{
    ASN1_OCTET_STRING **pos = CMS_get0_content(cms);

    if (pos == NULL || *pos == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CONTENT);
        return 0;
    }
    return 1;
}

static void do_free_upto(BIO *f, BIO *upto)
{
    if (upto != NULL) {
        BIO *tbio;

        do {
            tbio = BIO_pop(f);
            BIO_free(f);
            f = tbio;
        } while (f != NULL && f != upto);
    } else {
        BIO_free_all(f);
    }
}

int CMS_data(CMS_ContentInfo *cms, BIO *out, unsigned int flags)
{
    BIO *cont;
    int r;

    if (OBJ_obj2nid(CMS_get0_type(cms)) != NID_pkcs7_data) {
        ERR_raise(ERR_LIB_CMS, CMS_R_TYPE_NOT_DATA);
        return 0;
    }
    cont = CMS_dataInit(cms, NULL);
    if (cont == NULL)
        return 0;
    r = cms_copy_content(out, cont, flags);
    BIO_free_all(cont);
    return r;
}

CMS_ContentInfo *CMS_data_create_ex(BIO *in, unsigned int flags,
                                    OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *cms = ossl_cms_Data_create(libctx, propq);

    if (cms == NULL)
        return NULL;

    if ((flags & CMS_STREAM) || CMS_final(cms, in, NULL, flags))
        return cms;

    CMS_ContentInfo_free(cms);
    return NULL;
}

CMS_ContentInfo *CMS_data_create(BIO *in, unsigned int flags)
{
    return CMS_data_create_ex(in, flags, NULL, NULL);
}

int CMS_digest_verify(CMS_ContentInfo *cms, BIO *dcont, BIO *out,
                      unsigned int flags)
{
    BIO *cont;
    int r;

    if (OBJ_obj2nid(CMS_get0_type(cms)) != NID_pkcs7_digest) {
        ERR_raise(ERR_LIB_CMS, CMS_R_TYPE_NOT_DIGESTED_DATA);
        return 0;
    }

    if (dcont == NULL && !check_content(cms))
        return 0;

    cont = CMS_dataInit(cms, dcont);
    if (cont == NULL)
        return 0;

    r = cms_copy_content(out, cont, flags);
    if (r)
        r = ossl_cms_DigestedData_do_final(cms, cont, 1);
    do_free_upto(cont, dcont);
    return r;
}

CMS_ContentInfo *CMS_digest_create_ex(BIO *in, const EVP_MD *md,
                                      unsigned int flags, OSSL_LIB_CTX *ctx,
                                      const char *propq)
{
    CMS_ContentInfo *cms;

    /*
     * Because the EVP_MD is cached and can be a legacy algorithm, we
     * cannot fetch the algorithm if it isn't supplied.
     */
    if (md == NULL)
        md = EVP_sha1();
    cms = ossl_cms_DigestedData_create(md, ctx, propq);
    if (cms == NULL)
        return NULL;

    if (!(flags & CMS_DETACHED))
        CMS_set_detached(cms, 0);

    if ((flags & CMS_STREAM) || CMS_final(cms, in, NULL, flags))
        return cms;

    CMS_ContentInfo_free(cms);
    return NULL;
}

CMS_ContentInfo *CMS_digest_create(BIO *in, const EVP_MD *md,
                                   unsigned int flags)
{
    return CMS_digest_create_ex(in, md, flags, NULL, NULL);
}

int CMS_EncryptedData_decrypt(CMS_ContentInfo *cms,
                              const unsigned char *key, size_t keylen,
                              BIO *dcont, BIO *out, unsigned int flags)
{
    BIO *cont;
    int r;

    if (OBJ_obj2nid(CMS_get0_type(cms)) != NID_pkcs7_encrypted) {
        ERR_raise(ERR_LIB_CMS, CMS_R_TYPE_NOT_ENCRYPTED_DATA);
        return 0;
    }

    if (dcont == NULL && !check_content(cms))
        return 0;

    if (CMS_EncryptedData_set1_key(cms, NULL, key, keylen) <= 0)
        return 0;
    cont = CMS_dataInit(cms, dcont);
    if (cont == NULL)
        return 0;
    r = cms_copy_content(out, cont, flags);
    do_free_upto(cont, dcont);
    return r;
}

CMS_ContentInfo *CMS_EncryptedData_encrypt_ex(BIO *in, const EVP_CIPHER *cipher,
                                              const unsigned char *key,
                                              size_t keylen, unsigned int flags,
                                              OSSL_LIB_CTX *libctx,
                                              const char *propq)
{
    CMS_ContentInfo *cms;

    if (cipher == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_CIPHER);
        return NULL;
    }
    cms = CMS_ContentInfo_new_ex(libctx, propq);
    if (cms == NULL)
        return NULL;
    if (!CMS_EncryptedData_set1_key(cms, cipher, key, keylen))
        goto err;

    if (!(flags & CMS_DETACHED))
        CMS_set_detached(cms, 0);

    if ((flags & (CMS_STREAM | CMS_PARTIAL))
        || CMS_final(cms, in, NULL, flags))
        return cms;

 err:
    CMS_ContentInfo_free(cms);
    return NULL;
}

CMS_ContentInfo *CMS_EncryptedData_encrypt(BIO *in, const EVP_CIPHER *cipher,
                                           const unsigned char *key,
                                           size_t keylen, unsigned int flags)
{
    return CMS_EncryptedData_encrypt_ex(in, cipher, key, keylen, flags, NULL,
                                        NULL);
}

static int cms_signerinfo_verify_cert(CMS_SignerInfo *si,
                                      X509_STORE *store,
                                      STACK_OF(X509) *untrusted,
                                      STACK_OF(X509_CRL) *crls,
                                      STACK_OF(X509) **chain,
                                      const CMS_CTX *cms_ctx)
{
    X509_STORE_CTX *ctx;
    X509 *signer;
    int i, j, r = 0;

    ctx = X509_STORE_CTX_new_ex(ossl_cms_ctx_get0_libctx(cms_ctx),
                                ossl_cms_ctx_get0_propq(cms_ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_X509_LIB);
        goto err;
    }
    CMS_SignerInfo_get0_algs(si, NULL, &signer, NULL, NULL);
    if (!X509_STORE_CTX_init(ctx, store, signer, untrusted)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_STORE_INIT_ERROR);
        goto err;
    }
    X509_STORE_CTX_set_default(ctx, "smime_sign");
    if (crls != NULL)
        X509_STORE_CTX_set0_crls(ctx, crls);

    i = X509_verify_cert(ctx);
    if (i <= 0) {
        j = X509_STORE_CTX_get_error(ctx);
        ERR_raise_data(ERR_LIB_CMS, CMS_R_CERTIFICATE_VERIFY_ERROR,
                       "Verify error: %s", X509_verify_cert_error_string(j));
        goto err;
    }
    r = 1;

    /* also send back the trust chain when required */
    if (chain != NULL)
        *chain = X509_STORE_CTX_get1_chain(ctx);
 err:
    X509_STORE_CTX_free(ctx);
    return r;

}

/* This strongly overlaps with PKCS7_verify() */
int CMS_verify(CMS_ContentInfo *cms, STACK_OF(X509) *certs,
               X509_STORE *store, BIO *dcont, BIO *out, unsigned int flags)
{
    CMS_SignerInfo *si;
    STACK_OF(CMS_SignerInfo) *sinfos;
    STACK_OF(X509) *untrusted = NULL;
    STACK_OF(X509_CRL) *crls = NULL;
    STACK_OF(X509) **si_chains = NULL;
    X509 *signer;
    int i, scount = 0, ret = 0;
    BIO *cmsbio = NULL, *tmpin = NULL, *tmpout = NULL;
    int cadesVerify = (flags & CMS_CADES) != 0;
    const CMS_CTX *ctx = ossl_cms_get0_cmsctx(cms);

    if (dcont == NULL && !check_content(cms))
        return 0;
    if (dcont != NULL && !(flags & CMS_BINARY)) {
        const ASN1_OBJECT *coid = CMS_get0_eContentType(cms);

        if (OBJ_obj2nid(coid) == NID_id_ct_asciiTextWithCRLF)
            flags |= CMS_ASCIICRLF;
    }

    /* Attempt to find all signer certificates */

    sinfos = CMS_get0_SignerInfos(cms);

    if (sk_CMS_SignerInfo_num(sinfos) <= 0) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_SIGNERS);
        goto err;
    }

    for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
        si = sk_CMS_SignerInfo_value(sinfos, i);
        CMS_SignerInfo_get0_algs(si, NULL, &signer, NULL, NULL);
        if (signer != NULL)
            scount++;
    }

    if (scount != sk_CMS_SignerInfo_num(sinfos))
        scount += CMS_set1_signers_certs(cms, certs, flags);

    if (scount != sk_CMS_SignerInfo_num(sinfos)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_SIGNER_CERTIFICATE_NOT_FOUND);
        goto err;
    }

    /* Attempt to verify all signers certs */
    /* at this point scount == sk_CMS_SignerInfo_num(sinfos) */

    if ((flags & CMS_NO_SIGNER_CERT_VERIFY) == 0 || cadesVerify) {
        if (cadesVerify) {
            /* Certificate trust chain is required to check CAdES signature */
            si_chains = OPENSSL_zalloc(scount * sizeof(si_chains[0]));
            if (si_chains == NULL)
                goto err;
        }
        if (!ossl_cms_get1_certs_ex(cms, &untrusted))
            goto err;
        if (sk_X509_num(certs) > 0
            && !ossl_x509_add_certs_new(&untrusted, certs,
                                        X509_ADD_FLAG_UP_REF |
                                        X509_ADD_FLAG_NO_DUP))
            goto err;

        if ((flags & CMS_NOCRL) == 0
            && !ossl_cms_get1_crls_ex(cms, &crls))
            goto err;
        for (i = 0; i < scount; i++) {
            si = sk_CMS_SignerInfo_value(sinfos, i);

            if (!cms_signerinfo_verify_cert(si, store, untrusted, crls,
                                            si_chains ? &si_chains[i] : NULL,
                                            ctx))
                goto err;
        }
    }

    /* Attempt to verify all SignerInfo signed attribute signatures */

    if ((flags & CMS_NO_ATTR_VERIFY) == 0 || cadesVerify) {
        for (i = 0; i < scount; i++) {
            si = sk_CMS_SignerInfo_value(sinfos, i);
            if (CMS_signed_get_attr_count(si) < 0)
                continue;
            if (CMS_SignerInfo_verify(si) <= 0)
                goto err;
            if (cadesVerify) {
                STACK_OF(X509) *si_chain = si_chains ? si_chains[i] : NULL;

                if (ossl_cms_check_signing_certs(si, si_chain) <= 0)
                    goto err;
            }
        }
    }

    /*
     * Performance optimization: if the content is a memory BIO then store
     * its contents in a temporary read only memory BIO. This avoids
     * potentially large numbers of slow copies of data which will occur when
     * reading from a read write memory BIO when signatures are calculated.
     */

    if (dcont != NULL && (BIO_method_type(dcont) == BIO_TYPE_MEM)) {
        char *ptr;
        long len;

        len = BIO_get_mem_data(dcont, &ptr);
        tmpin = (len == 0) ? dcont : BIO_new_mem_buf(ptr, len);
        if (tmpin == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_BIO_LIB);
            goto err2;
        }
    } else {
        tmpin = dcont;
    }
    /*
     * If not binary mode and detached generate digests by *writing* through
     * the BIO. That makes it possible to canonicalise the input.
     */
    if (!(flags & SMIME_BINARY) && dcont) {
        /*
         * Create output BIO so we can either handle text or to ensure
         * included content doesn't override detached content.
         */
        tmpout = cms_get_text_bio(out, flags);
        if (tmpout == NULL) {
            ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
            goto err;
        }
        cmsbio = CMS_dataInit(cms, tmpout);
        if (cmsbio == NULL)
            goto err;
        /*
         * Don't use SMIME_TEXT for verify: it adds headers and we want to
         * remove them.
         */
        if (!SMIME_crlf_copy(dcont, cmsbio, flags & ~SMIME_TEXT))
            goto err;

        if (flags & CMS_TEXT) {
            if (!SMIME_text(tmpout, out)) {
                ERR_raise(ERR_LIB_CMS, CMS_R_SMIME_TEXT_ERROR);
                goto err;
            }
        }
    } else {
        cmsbio = CMS_dataInit(cms, tmpin);
        if (cmsbio == NULL)
            goto err;

        if (!cms_copy_content(out, cmsbio, flags))
            goto err;

    }
    if (!(flags & CMS_NO_CONTENT_VERIFY)) {
        for (i = 0; i < sk_CMS_SignerInfo_num(sinfos); i++) {
            si = sk_CMS_SignerInfo_value(sinfos, i);
            if (CMS_SignerInfo_verify_content(si, cmsbio) <= 0) {
                ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_VERIFY_ERROR);
                goto err;
            }
        }
    }

    ret = 1;
 err:
    if (!(flags & SMIME_BINARY) && dcont) {
        do_free_upto(cmsbio, tmpout);
        if (tmpin != dcont)
            BIO_free(tmpin);
    } else {
        if (dcont && (tmpin == dcont))
            do_free_upto(cmsbio, dcont);
        else
            BIO_free_all(cmsbio);
    }

    if (out != tmpout)
        BIO_free_all(tmpout);

 err2:
    if (si_chains != NULL) {
        for (i = 0; i < scount; ++i)
            OSSL_STACK_OF_X509_free(si_chains[i]);
        OPENSSL_free(si_chains);
    }
    sk_X509_pop_free(untrusted, X509_free);
    sk_X509_CRL_pop_free(crls, X509_CRL_free);

    return ret;
}

int CMS_verify_receipt(CMS_ContentInfo *rcms, CMS_ContentInfo *ocms,
                       STACK_OF(X509) *certs,
                       X509_STORE *store, unsigned int flags)
{
    int r;

    flags &= ~(CMS_DETACHED | CMS_TEXT);
    r = CMS_verify(rcms, certs, store, NULL, NULL, flags);
    if (r <= 0)
        return r;
    return ossl_cms_Receipt_verify(rcms, ocms);
}

CMS_ContentInfo *CMS_sign_ex(X509 *signcert, EVP_PKEY *pkey,
                             STACK_OF(X509) *certs, BIO *data,
                             unsigned int flags, OSSL_LIB_CTX *libctx,
                             const char *propq)
{
    CMS_ContentInfo *cms;
    int i;

    cms = CMS_ContentInfo_new_ex(libctx, propq);
    if (cms == NULL || !CMS_SignedData_init(cms)) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }
    if (flags & CMS_ASCIICRLF
        && !CMS_set1_eContentType(cms,
                                  OBJ_nid2obj(NID_id_ct_asciiTextWithCRLF))) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }

    if (pkey != NULL && !CMS_add1_signer(cms, signcert, pkey, NULL, flags)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_ADD_SIGNER_ERROR);
        goto err;
    }

    for (i = 0; i < sk_X509_num(certs); i++) {
        X509 *x = sk_X509_value(certs, i);

        if (!CMS_add1_cert(cms, x)) {
            ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
            goto err;
        }
    }

    if (!(flags & CMS_DETACHED))
        CMS_set_detached(cms, 0);

    if ((flags & (CMS_STREAM | CMS_PARTIAL))
        || CMS_final(cms, data, NULL, flags))
        return cms;
    else
        goto err;

 err:
    CMS_ContentInfo_free(cms);
    return NULL;
}

CMS_ContentInfo *CMS_sign(X509 *signcert, EVP_PKEY *pkey, STACK_OF(X509) *certs,
                          BIO *data, unsigned int flags)
{
    return CMS_sign_ex(signcert, pkey, certs, data, flags, NULL, NULL);
}

CMS_ContentInfo *CMS_sign_receipt(CMS_SignerInfo *si,
                                  X509 *signcert, EVP_PKEY *pkey,
                                  STACK_OF(X509) *certs, unsigned int flags)
{
    CMS_SignerInfo *rct_si;
    CMS_ContentInfo *cms = NULL;
    ASN1_OCTET_STRING **pos, *os = NULL;
    BIO *rct_cont = NULL;
    int r = 0;
    const CMS_CTX *ctx = si->cms_ctx;

    flags &= ~(CMS_STREAM | CMS_TEXT);
    /* Not really detached but avoids content being allocated */
    flags |= CMS_PARTIAL | CMS_BINARY | CMS_DETACHED;
    if (pkey == NULL || signcert == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_KEY_OR_CERT);
        return NULL;
    }

    /* Initialize signed data */

    cms = CMS_sign_ex(NULL, NULL, certs, NULL, flags,
                      ossl_cms_ctx_get0_libctx(ctx),
                      ossl_cms_ctx_get0_propq(ctx));
    if (cms == NULL)
        goto err;

    /* Set inner content type to signed receipt */
    if (!CMS_set1_eContentType(cms, OBJ_nid2obj(NID_id_smime_ct_receipt)))
        goto err;

    rct_si = CMS_add1_signer(cms, signcert, pkey, NULL, flags);
    if (!rct_si) {
        ERR_raise(ERR_LIB_CMS, CMS_R_ADD_SIGNER_ERROR);
        goto err;
    }

    os = ossl_cms_encode_Receipt(si);
    if (os == NULL)
        goto err;

    /* Set content to digest */
    rct_cont = BIO_new_mem_buf(os->data, os->length);
    if (rct_cont == NULL)
        goto err;

    /* Add msgSigDigest attribute */

    if (!ossl_cms_msgSigDigest_add1(rct_si, si))
        goto err;

    /* Finalize structure */
    if (!CMS_final(cms, rct_cont, NULL, flags))
        goto err;

    /* Set embedded content */
    pos = CMS_get0_content(cms);
    if (pos == NULL)
        goto err;
    *pos = os;

    r = 1;

 err:
    BIO_free(rct_cont);
    if (r)
        return cms;
    CMS_ContentInfo_free(cms);
    ASN1_OCTET_STRING_free(os);
    return NULL;

}

CMS_ContentInfo *CMS_encrypt_ex(STACK_OF(X509) *certs, BIO *data,
                                const EVP_CIPHER *cipher, unsigned int flags,
                                OSSL_LIB_CTX *libctx, const char *propq)
{
    CMS_ContentInfo *cms;
    int i;
    X509 *recip;


    cms = (EVP_CIPHER_get_flags(cipher) & EVP_CIPH_FLAG_AEAD_CIPHER)
          ? CMS_AuthEnvelopedData_create_ex(cipher, libctx, propq)
          : CMS_EnvelopedData_create_ex(cipher, libctx, propq);
    if (cms == NULL) {
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);
        goto err;
    }
    for (i = 0; i < sk_X509_num(certs); i++) {
        recip = sk_X509_value(certs, i);
        if (!CMS_add1_recipient_cert(cms, recip, flags)) {
            ERR_raise(ERR_LIB_CMS, CMS_R_RECIPIENT_ERROR);
            goto err;
        }
    }

    if (!(flags & CMS_DETACHED))
        CMS_set_detached(cms, 0);

    if ((flags & (CMS_STREAM | CMS_PARTIAL))
        || CMS_final(cms, data, NULL, flags))
        return cms;
    else
        ERR_raise(ERR_LIB_CMS, ERR_R_CMS_LIB);

 err:
    CMS_ContentInfo_free(cms);
    return NULL;
}

CMS_ContentInfo *CMS_encrypt(STACK_OF(X509) *certs, BIO *data,
                             const EVP_CIPHER *cipher, unsigned int flags)
{
    return CMS_encrypt_ex(certs, data, cipher, flags, NULL, NULL);
}

static int cms_kari_set1_pkey_and_peer(CMS_ContentInfo *cms,
                                       CMS_RecipientInfo *ri,
                                       EVP_PKEY *pk, X509 *cert, X509 *peer)
{
    int i;
    STACK_OF(CMS_RecipientEncryptedKey) *reks;
    CMS_RecipientEncryptedKey *rek;

    reks = CMS_RecipientInfo_kari_get0_reks(ri);
    for (i = 0; i < sk_CMS_RecipientEncryptedKey_num(reks); i++) {
        int rv;

        rek = sk_CMS_RecipientEncryptedKey_value(reks, i);
        if (cert != NULL && CMS_RecipientEncryptedKey_cert_cmp(rek, cert))
            continue;
        CMS_RecipientInfo_kari_set0_pkey_and_peer(ri, pk, peer);
        rv = CMS_RecipientInfo_kari_decrypt(cms, ri, rek);
        CMS_RecipientInfo_kari_set0_pkey(ri, NULL);
        if (rv > 0)
            return 1;
        return cert == NULL ? 0 : -1;
    }
    return 0;
}

int CMS_decrypt_set1_pkey(CMS_ContentInfo *cms, EVP_PKEY *pk, X509 *cert)
{
     return CMS_decrypt_set1_pkey_and_peer(cms, pk, cert, NULL);
}

int CMS_decrypt_set1_pkey_and_peer(CMS_ContentInfo *cms, EVP_PKEY *pk,
                                   X509 *cert, X509 *peer)
{
    STACK_OF(CMS_RecipientInfo) *ris = CMS_get0_RecipientInfos(cms);
    CMS_RecipientInfo *ri;
    int i, r, cms_pkey_ri_type;
    int debug = 0, match_ri = 0;
    CMS_EncryptedContentInfo *ec = ossl_cms_get0_env_enc_content(cms);

    /* Prevent mem leak on earlier CMS_decrypt_set1_{pkey_and_peer,password} */
    if (ec != NULL) {
        OPENSSL_clear_free(ec->key, ec->keylen);
        ec->key = NULL;
        ec->keylen = 0;
    }

    if (ris != NULL && ec != NULL)
        debug = ec->debug;

    cms_pkey_ri_type = ossl_cms_pkey_get_ri_type(pk);
    if (cms_pkey_ri_type == CMS_RECIPINFO_NONE) {
         ERR_raise(ERR_LIB_CMS, CMS_R_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
         return 0;
    }

    for (i = 0; i < sk_CMS_RecipientInfo_num(ris); i++) {
        int ri_type;

        ri = sk_CMS_RecipientInfo_value(ris, i);
        ri_type = CMS_RecipientInfo_type(ri);
        if (!ossl_cms_pkey_is_ri_type_supported(pk, ri_type))
            continue;
        match_ri = 1;
        if (ri_type == CMS_RECIPINFO_AGREE) {
            r = cms_kari_set1_pkey_and_peer(cms, ri, pk, cert, peer);
            if (r > 0)
                return 1;
            if (r < 0)
                return 0;
        }
        /* If we have a cert, try matching RecipientInfo, else try them all */
        else if (cert == NULL || !CMS_RecipientInfo_ktri_cert_cmp(ri, cert)) {
            if (!EVP_PKEY_up_ref(pk))
                return 0;
            CMS_RecipientInfo_set0_pkey(ri, pk);
            r = CMS_RecipientInfo_decrypt(cms, ri);
            CMS_RecipientInfo_set0_pkey(ri, NULL);
            if (cert != NULL) {
                /*
                 * If not debugging clear any error and return success to
                 * avoid leaking of information useful to MMA
                 */
                if (!debug) {
                    ERR_clear_error();
                    return 1;
                }
                if (r > 0)
                    return 1;
                ERR_raise(ERR_LIB_CMS, CMS_R_DECRYPT_ERROR);
                return 0;
            }
            /*
             * If no cert and not debugging don't leave loop after first
             * successful decrypt. Always attempt to decrypt all recipients
             * to avoid leaking timing of a successful decrypt.
             */
            else if (r > 0 && (debug || cms_pkey_ri_type != CMS_RECIPINFO_TRANS))
                return 1;
        }
    }
    /* If no cert, key transport and not debugging always return success */
    if (cert == NULL
        && cms_pkey_ri_type == CMS_RECIPINFO_TRANS
        && match_ri
        && !debug) {
        ERR_clear_error();
        return 1;
    }

    if (!match_ri)
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_RECIPIENT);
    return 0;

}

int CMS_decrypt_set1_key(CMS_ContentInfo *cms,
                         unsigned char *key, size_t keylen,
                         const unsigned char *id, size_t idlen)
{
    STACK_OF(CMS_RecipientInfo) *ris;
    CMS_RecipientInfo *ri;
    int i, r, match_ri = 0;

    ris = CMS_get0_RecipientInfos(cms);
    for (i = 0; i < sk_CMS_RecipientInfo_num(ris); i++) {
        ri = sk_CMS_RecipientInfo_value(ris, i);
        if (CMS_RecipientInfo_type(ri) != CMS_RECIPINFO_KEK)
            continue;

        /* If we have an id, try matching RecipientInfo, else try them all */
        if (id == NULL
                || (CMS_RecipientInfo_kekri_id_cmp(ri, id, idlen) == 0)) {
            match_ri = 1;
            CMS_RecipientInfo_set0_key(ri, key, keylen);
            r = CMS_RecipientInfo_decrypt(cms, ri);
            CMS_RecipientInfo_set0_key(ri, NULL, 0);
            if (r > 0)
                return 1;
            if (id != NULL) {
                ERR_raise(ERR_LIB_CMS, CMS_R_DECRYPT_ERROR);
                return 0;
            }
            ERR_clear_error();
        }
    }

    if (!match_ri)
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_RECIPIENT);
    return 0;

}

int CMS_decrypt_set1_password(CMS_ContentInfo *cms,
                              unsigned char *pass, ossl_ssize_t passlen)
{
    STACK_OF(CMS_RecipientInfo) *ris = CMS_get0_RecipientInfos(cms);
    CMS_RecipientInfo *ri;
    int i, r, match_ri = 0;
    CMS_EncryptedContentInfo *ec = ossl_cms_get0_env_enc_content(cms);

    /* Prevent mem leak on earlier CMS_decrypt_set1_{pkey_and_peer,password} */
    if (ec != NULL) {
        OPENSSL_clear_free(ec->key, ec->keylen);
        ec->key = NULL;
        ec->keylen = 0;
    }

    for (i = 0; i < sk_CMS_RecipientInfo_num(ris); i++) {
        ri = sk_CMS_RecipientInfo_value(ris, i);
        if (CMS_RecipientInfo_type(ri) != CMS_RECIPINFO_PASS)
            continue;

        /* Must try each PasswordRecipientInfo */
        match_ri = 1;
        CMS_RecipientInfo_set0_password(ri, pass, passlen);
        r = CMS_RecipientInfo_decrypt(cms, ri);
        CMS_RecipientInfo_set0_password(ri, NULL, 0);
        if (r > 0)
            return 1;
    }

    if (!match_ri)
        ERR_raise(ERR_LIB_CMS, CMS_R_NO_MATCHING_RECIPIENT);
    return 0;

}

int CMS_decrypt(CMS_ContentInfo *cms, EVP_PKEY *pk, X509 *cert,
                BIO *dcont, BIO *out, unsigned int flags)
{
    int r;
    BIO *cont;
    CMS_EncryptedContentInfo *ec;
    int nid = OBJ_obj2nid(CMS_get0_type(cms));

    if (nid != NID_pkcs7_enveloped
            && nid != NID_id_smime_ct_authEnvelopedData) {
        ERR_raise(ERR_LIB_CMS, CMS_R_TYPE_NOT_ENVELOPED_DATA);
        return 0;
    }
    if (dcont == NULL && !check_content(cms))
        return 0;
    ec = ossl_cms_get0_env_enc_content(cms);
    ec->debug = (flags & CMS_DEBUG_DECRYPT) != 0;
    ec->havenocert = cert == NULL;
    if (pk == NULL && cert == NULL && dcont == NULL && out == NULL)
        return 1;
    if (pk != NULL && !CMS_decrypt_set1_pkey(cms, pk, cert))
        return 0;
    cont = CMS_dataInit(cms, dcont);
    if (cont == NULL)
        return 0;
    r = cms_copy_content(out, cont, flags);
    do_free_upto(cont, dcont);
    return r;
}

int CMS_final(CMS_ContentInfo *cms, BIO *data, BIO *dcont, unsigned int flags)
{
    BIO *cmsbio;
    int ret = 0;

    if ((cmsbio = CMS_dataInit(cms, dcont)) == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CMS_LIB);
        return 0;
    }

    if (!SMIME_crlf_copy(data, cmsbio, flags)) {
        goto err;
    }

    (void)BIO_flush(cmsbio);

    if (!CMS_dataFinal(cms, cmsbio)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CMS_DATAFINAL_ERROR);
        goto err;
    }

    ret = 1;

err:
    do_free_upto(cmsbio, dcont);

    return ret;

}

int CMS_final_digest(CMS_ContentInfo *cms,
                     const unsigned char *md, unsigned int mdlen,
                     BIO *dcont, unsigned int flags)
{
    BIO *cmsbio;
    int ret = 0;

    if ((cmsbio = CMS_dataInit(cms, dcont)) == NULL) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CMS_LIB);
        return 0;
    }

    (void)BIO_flush(cmsbio);

    if (!ossl_cms_DataFinal(cms, cmsbio, md, mdlen)) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CMS_DATAFINAL_ERROR);
        goto err;
    }
    ret = 1;

err:
    do_free_upto(cmsbio, dcont);
    return ret;
}

#ifndef OPENSSL_NO_ZLIB

int CMS_uncompress(CMS_ContentInfo *cms, BIO *dcont, BIO *out,
                   unsigned int flags)
{
    BIO *cont;
    int r;

    if (OBJ_obj2nid(CMS_get0_type(cms)) != NID_id_smime_ct_compressedData) {
        ERR_raise(ERR_LIB_CMS, CMS_R_TYPE_NOT_COMPRESSED_DATA);
        return 0;
    }

    if (dcont == NULL && !check_content(cms))
        return 0;

    cont = CMS_dataInit(cms, dcont);
    if (cont == NULL)
        return 0;
    r = cms_copy_content(out, cont, flags);
    do_free_upto(cont, dcont);
    return r;
}

CMS_ContentInfo *CMS_compress(BIO *in, int comp_nid, unsigned int flags)
{
    CMS_ContentInfo *cms;

    if (comp_nid <= 0)
        comp_nid = NID_zlib_compression;
    cms = ossl_cms_CompressedData_create(comp_nid, NULL, NULL);
    if (cms == NULL)
        return NULL;

    if (!(flags & CMS_DETACHED))
        CMS_set_detached(cms, 0);

    if ((flags & CMS_STREAM) || CMS_final(cms, in, NULL, flags))
        return cms;

    CMS_ContentInfo_free(cms);
    return NULL;
}

#else

int CMS_uncompress(CMS_ContentInfo *cms, BIO *dcont, BIO *out,
                   unsigned int flags)
{
    ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
    return 0;
}

CMS_ContentInfo *CMS_compress(BIO *in, int comp_nid, unsigned int flags)
{
    ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
    return NULL;
}

#endif
