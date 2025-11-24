/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/pkcs7.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/x509.h" /* for sk_X509_add1_cert() */
#include "pk7_local.h"

long PKCS7_ctrl(PKCS7 *p7, int cmd, long larg, char *parg)
{
    int nid;
    long ret;

    nid = OBJ_obj2nid(p7->type);

    switch (cmd) {
    /* NOTE(emilia): does not support detached digested data. */
    case PKCS7_OP_SET_DETACHED_SIGNATURE:
        if (nid == NID_pkcs7_signed) {
            if (p7->d.sign == NULL) {
                ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_CONTENT);
                ret = 0;
                break;
            }
            ret = p7->detached = (int)larg;
            if (ret && PKCS7_type_is_data(p7->d.sign->contents)) {
                ASN1_OCTET_STRING *os;
                os = p7->d.sign->contents->d.data;
                ASN1_OCTET_STRING_free(os);
                p7->d.sign->contents->d.data = NULL;
            }
        } else {
            ERR_raise(ERR_LIB_PKCS7,
                      PKCS7_R_OPERATION_NOT_SUPPORTED_ON_THIS_TYPE);
            ret = 0;
        }
        break;
    case PKCS7_OP_GET_DETACHED_SIGNATURE:
        if (nid == NID_pkcs7_signed) {
            if (p7->d.sign == NULL || p7->d.sign->contents->d.ptr == NULL)
                ret = 1;
            else
                ret = 0;

            p7->detached = ret;
        } else {
            ERR_raise(ERR_LIB_PKCS7,
                      PKCS7_R_OPERATION_NOT_SUPPORTED_ON_THIS_TYPE);
            ret = 0;
        }

        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNKNOWN_OPERATION);
        ret = 0;
    }
    return ret;
}

int PKCS7_content_new(PKCS7 *p7, int type)
{
    PKCS7 *ret = NULL;

    if ((ret = PKCS7_new()) == NULL)
        goto err;
    if (!PKCS7_set_type(ret, type))
        goto err;
    if (!PKCS7_set_content(p7, ret))
        goto err;

    return 1;
 err:
    PKCS7_free(ret);
    return 0;
}

int PKCS7_set_content(PKCS7 *p7, PKCS7 *p7_data)
{
    int i;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signed:
        PKCS7_free(p7->d.sign->contents);
        p7->d.sign->contents = p7_data;
        break;
    case NID_pkcs7_digest:
        PKCS7_free(p7->d.digest->contents);
        p7->d.digest->contents = p7_data;
        break;
    case NID_pkcs7_data:
    case NID_pkcs7_enveloped:
    case NID_pkcs7_signedAndEnveloped:
    case NID_pkcs7_encrypted:
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CONTENT_TYPE);
        goto err;
    }
    return 1;
 err:
    return 0;
}

int PKCS7_set_type(PKCS7 *p7, int type)
{
    ASN1_OBJECT *obj;

    /*
     * PKCS7_content_free(p7);
     */
    obj = OBJ_nid2obj(type);    /* will not fail */

    switch (type) {
    case NID_pkcs7_signed:
        p7->type = obj;
        if ((p7->d.sign = PKCS7_SIGNED_new()) == NULL)
            goto err;
        if (!ASN1_INTEGER_set(p7->d.sign->version, 1)) {
            PKCS7_SIGNED_free(p7->d.sign);
            p7->d.sign = NULL;
            goto err;
        }
        break;
    case NID_pkcs7_data:
        p7->type = obj;
        if ((p7->d.data = ASN1_OCTET_STRING_new()) == NULL)
            goto err;
        break;
    case NID_pkcs7_signedAndEnveloped:
        p7->type = obj;
        if ((p7->d.signed_and_enveloped = PKCS7_SIGN_ENVELOPE_new())
            == NULL)
            goto err;
        if (!ASN1_INTEGER_set(p7->d.signed_and_enveloped->version, 1))
            goto err;
        p7->d.signed_and_enveloped->enc_data->content_type
            = OBJ_nid2obj(NID_pkcs7_data);
        break;
    case NID_pkcs7_enveloped:
        p7->type = obj;
        if ((p7->d.enveloped = PKCS7_ENVELOPE_new())
            == NULL)
            goto err;
        if (!ASN1_INTEGER_set(p7->d.enveloped->version, 0))
            goto err;
        p7->d.enveloped->enc_data->content_type = OBJ_nid2obj(NID_pkcs7_data);
        break;
    case NID_pkcs7_encrypted:
        p7->type = obj;
        if ((p7->d.encrypted = PKCS7_ENCRYPT_new())
            == NULL)
            goto err;
        if (!ASN1_INTEGER_set(p7->d.encrypted->version, 0))
            goto err;
        p7->d.encrypted->enc_data->content_type = OBJ_nid2obj(NID_pkcs7_data);
        break;

    case NID_pkcs7_digest:
        p7->type = obj;
        if ((p7->d.digest = PKCS7_DIGEST_new())
            == NULL)
            goto err;
        if (!ASN1_INTEGER_set(p7->d.digest->version, 0))
            goto err;
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_UNSUPPORTED_CONTENT_TYPE);
        goto err;
    }
    return 1;
 err:
    return 0;
}

int PKCS7_set0_type_other(PKCS7 *p7, int type, ASN1_TYPE *other)
{
    p7->type = OBJ_nid2obj(type);
    p7->d.other = other;
    return 1;
}

int PKCS7_add_signer(PKCS7 *p7, PKCS7_SIGNER_INFO *psi)
{
    int i, j;
    ASN1_OBJECT *obj;
    X509_ALGOR *alg;
    STACK_OF(PKCS7_SIGNER_INFO) *signer_sk;
    STACK_OF(X509_ALGOR) *md_sk;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signed:
        signer_sk = p7->d.sign->signer_info;
        md_sk = p7->d.sign->md_algs;
        break;
    case NID_pkcs7_signedAndEnveloped:
        signer_sk = p7->d.signed_and_enveloped->signer_info;
        md_sk = p7->d.signed_and_enveloped->md_algs;
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    obj = psi->digest_alg->algorithm;
    /* If the digest is not currently listed, add it */
    j = 0;
    for (i = 0; i < sk_X509_ALGOR_num(md_sk); i++) {
        alg = sk_X509_ALGOR_value(md_sk, i);
        if (OBJ_cmp(obj, alg->algorithm) == 0) {
            j = 1;
            break;
        }
    }
    if (!j) {                   /* we need to add another algorithm */
        int nid;

        if ((alg = X509_ALGOR_new()) == NULL
            || (alg->parameter = ASN1_TYPE_new()) == NULL) {
            X509_ALGOR_free(alg);
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            return 0;
        }
        /*
         * If there is a constant copy of the ASN1 OBJECT in libcrypto, then
         * use that.  Otherwise, use a dynamically duplicated copy
         */
        if ((nid = OBJ_obj2nid(obj)) != NID_undef)
            alg->algorithm = OBJ_nid2obj(nid);
        else
            alg->algorithm = OBJ_dup(obj);
        alg->parameter->type = V_ASN1_NULL;
        if (alg->algorithm == NULL || !sk_X509_ALGOR_push(md_sk, alg)) {
            X509_ALGOR_free(alg);
            return 0;
        }
    }

    psi->ctx = ossl_pkcs7_get0_ctx(p7);
    if (!sk_PKCS7_SIGNER_INFO_push(signer_sk, psi))
        return 0;
    return 1;
}

int PKCS7_add_certificate(PKCS7 *p7, X509 *x509)
{
    int i;
    STACK_OF(X509) **sk;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signed:
        sk = &(p7->d.sign->cert);
        break;
    case NID_pkcs7_signedAndEnveloped:
        sk = &(p7->d.signed_and_enveloped->cert);
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    return ossl_x509_add_cert_new(sk, x509, X509_ADD_FLAG_UP_REF);
}

int PKCS7_add_crl(PKCS7 *p7, X509_CRL *crl)
{
    int i;
    STACK_OF(X509_CRL) **sk;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signed:
        sk = &(p7->d.sign->crl);
        break;
    case NID_pkcs7_signedAndEnveloped:
        sk = &(p7->d.signed_and_enveloped->crl);
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    if (*sk == NULL)
        *sk = sk_X509_CRL_new_null();
    if (*sk == NULL) {
        ERR_raise(ERR_LIB_PKCS7, ERR_R_CRYPTO_LIB);
        return 0;
    }

    if (!X509_CRL_up_ref(crl))
        return 0;
    if (!sk_X509_CRL_push(*sk, crl)) {
        X509_CRL_free(crl);
        return 0;
    }
    return 1;
}

static int pkcs7_ecdsa_or_dsa_sign_verify_setup(PKCS7_SIGNER_INFO *si,
                                                int verify)
{
    if (!verify) {
        int snid, hnid;
        X509_ALGOR *alg1, *alg2;
        EVP_PKEY *pkey = si->pkey;

        PKCS7_SIGNER_INFO_get0_algs(si, NULL, &alg1, &alg2);
        if (alg1 == NULL || alg1->algorithm == NULL)
            return -1;
        hnid = OBJ_obj2nid(alg1->algorithm);
        if (hnid == NID_undef)
            return -1;
        if (!OBJ_find_sigid_by_algs(&snid, hnid, EVP_PKEY_get_id(pkey)))
            return -1;
        return X509_ALGOR_set0(alg2, OBJ_nid2obj(snid), V_ASN1_UNDEF, NULL);
    }
    return 1;
}

static int pkcs7_rsa_sign_verify_setup(PKCS7_SIGNER_INFO *si, int verify)
{
    if (!verify) {
        X509_ALGOR *alg = NULL;

        PKCS7_SIGNER_INFO_get0_algs(si, NULL, NULL, &alg);
        if (alg != NULL)
            return X509_ALGOR_set0(alg, OBJ_nid2obj(NID_rsaEncryption),
                                   V_ASN1_NULL, NULL);
    }
    return 1;
}

int PKCS7_SIGNER_INFO_set(PKCS7_SIGNER_INFO *p7i, X509 *x509, EVP_PKEY *pkey,
                          const EVP_MD *dgst)
{
    int ret;

    /* We now need to add another PKCS7_SIGNER_INFO entry */
    if (!ASN1_INTEGER_set(p7i->version, 1))
        return 0;
    if (!X509_NAME_set(&p7i->issuer_and_serial->issuer,
                       X509_get_issuer_name(x509)))
        return 0;

    /*
     * because ASN1_INTEGER_set is used to set a 'long' we will do things the
     * ugly way.
     */
    ASN1_INTEGER_free(p7i->issuer_and_serial->serial);
    if (!(p7i->issuer_and_serial->serial =
          ASN1_INTEGER_dup(X509_get0_serialNumber(x509))))
        return 0;

    /* lets keep the pkey around for a while */
    if (!EVP_PKEY_up_ref(pkey))
        return 0;

    p7i->pkey = pkey;

    /* Set the algorithms */

    if (!X509_ALGOR_set0(p7i->digest_alg, OBJ_nid2obj(EVP_MD_get_type(dgst)),
                         V_ASN1_NULL, NULL))
        return 0;

    if (EVP_PKEY_is_a(pkey, "EC") || EVP_PKEY_is_a(pkey, "DSA"))
        return pkcs7_ecdsa_or_dsa_sign_verify_setup(p7i, 0);
    if (EVP_PKEY_is_a(pkey, "RSA"))
        return pkcs7_rsa_sign_verify_setup(p7i, 0);

    if (pkey->ameth != NULL && pkey->ameth->pkey_ctrl != NULL) {
        ret = pkey->ameth->pkey_ctrl(pkey, ASN1_PKEY_CTRL_PKCS7_SIGN, 0, p7i);
        if (ret > 0)
            return 1;
        if (ret != -2) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SIGNING_CTRL_FAILURE);
            return 0;
        }
    }
    ERR_raise(ERR_LIB_PKCS7, PKCS7_R_SIGNING_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
    return 0;
}

PKCS7_SIGNER_INFO *PKCS7_add_signature(PKCS7 *p7, X509 *x509, EVP_PKEY *pkey,
                                       const EVP_MD *dgst)
{
    PKCS7_SIGNER_INFO *si = NULL;

    if (dgst == NULL) {
        int def_nid;
        if (EVP_PKEY_get_default_digest_nid(pkey, &def_nid) <= 0)
            goto err;
        dgst = EVP_get_digestbynid(def_nid);
        if (dgst == NULL) {
            ERR_raise(ERR_LIB_PKCS7, PKCS7_R_NO_DEFAULT_DIGEST);
            goto err;
        }
    }

    if ((si = PKCS7_SIGNER_INFO_new()) == NULL)
        goto err;
    if (PKCS7_SIGNER_INFO_set(si, x509, pkey, dgst) <= 0)
        goto err;
    if (!PKCS7_add_signer(p7, si))
        goto err;
    return si;
 err:
    PKCS7_SIGNER_INFO_free(si);
    return NULL;
}

STACK_OF(X509) *pkcs7_get0_certificates(const PKCS7 *p7)
{
    if (p7->d.ptr == NULL)
        return NULL;
    if (PKCS7_type_is_signed(p7))
        return p7->d.sign->cert;
    if (PKCS7_type_is_signedAndEnveloped(p7))
        return p7->d.signed_and_enveloped->cert;
    return NULL;
}

static STACK_OF(PKCS7_RECIP_INFO) *pkcs7_get_recipient_info(const PKCS7 *p7)
{
    if (p7->d.ptr == NULL)
        return NULL;
    if (PKCS7_type_is_signedAndEnveloped(p7))
        return p7->d.signed_and_enveloped->recipientinfo;
    if (PKCS7_type_is_enveloped(p7))
        return p7->d.enveloped->recipientinfo;
    return NULL;
}

/*
 * Set up the library context into any loaded structure that needs it.
 * i.e loaded X509 objects.
 */
void ossl_pkcs7_resolve_libctx(PKCS7 *p7)
{
    int i;
    const PKCS7_CTX *ctx = ossl_pkcs7_get0_ctx(p7);
    OSSL_LIB_CTX *libctx = ossl_pkcs7_ctx_get0_libctx(ctx);
    const char *propq = ossl_pkcs7_ctx_get0_propq(ctx);
    STACK_OF(PKCS7_RECIP_INFO) *rinfos;
    STACK_OF(PKCS7_SIGNER_INFO) *sinfos;
    STACK_OF(X509) *certs;

    if (ctx == NULL || p7->d.ptr == NULL)
        return;

    rinfos = pkcs7_get_recipient_info(p7);
    sinfos = PKCS7_get_signer_info(p7);
    certs = pkcs7_get0_certificates(p7);

    for (i = 0; i < sk_X509_num(certs); i++)
        ossl_x509_set0_libctx(sk_X509_value(certs, i), libctx, propq);

    for (i = 0; i < sk_PKCS7_RECIP_INFO_num(rinfos); i++) {
        PKCS7_RECIP_INFO *ri = sk_PKCS7_RECIP_INFO_value(rinfos, i);

        ossl_x509_set0_libctx(ri->cert, libctx, propq);
    }

    for (i = 0; i < sk_PKCS7_SIGNER_INFO_num(sinfos); i++) {
        PKCS7_SIGNER_INFO *si = sk_PKCS7_SIGNER_INFO_value(sinfos, i);

        if (si != NULL)
            si->ctx = ctx;
    }
}

const PKCS7_CTX *ossl_pkcs7_get0_ctx(const PKCS7 *p7)
{
    return p7 != NULL ? &p7->ctx : NULL;
}

void ossl_pkcs7_set0_libctx(PKCS7 *p7, OSSL_LIB_CTX *ctx)
{
    p7->ctx.libctx = ctx;
}

int ossl_pkcs7_set1_propq(PKCS7 *p7, const char *propq)
{
    if (p7->ctx.propq != NULL) {
        OPENSSL_free(p7->ctx.propq);
        p7->ctx.propq = NULL;
    }
    if (propq != NULL) {
        p7->ctx.propq = OPENSSL_strdup(propq);
        if (p7->ctx.propq == NULL)
            return 0;
    }
    return 1;
}

int ossl_pkcs7_ctx_propagate(const PKCS7 *from, PKCS7 *to)
{
    ossl_pkcs7_set0_libctx(to, from->ctx.libctx);
    if (!ossl_pkcs7_set1_propq(to, from->ctx.propq))
        return 0;

    ossl_pkcs7_resolve_libctx(to);
    return 1;
}

OSSL_LIB_CTX *ossl_pkcs7_ctx_get0_libctx(const PKCS7_CTX *ctx)
{
    return ctx != NULL ? ctx->libctx : NULL;
}
const char *ossl_pkcs7_ctx_get0_propq(const PKCS7_CTX *ctx)
{
    return ctx != NULL ? ctx->propq : NULL;
}

int PKCS7_set_digest(PKCS7 *p7, const EVP_MD *md)
{
    if (PKCS7_type_is_digest(p7)) {
        if ((p7->d.digest->md->parameter = ASN1_TYPE_new()) == NULL) {
            ERR_raise(ERR_LIB_PKCS7, ERR_R_ASN1_LIB);
            return 0;
        }
        p7->d.digest->md->parameter->type = V_ASN1_NULL;
        p7->d.digest->md->algorithm = OBJ_nid2obj(EVP_MD_nid(md));
        return 1;
    }

    ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
    return 1;
}

STACK_OF(PKCS7_SIGNER_INFO) *PKCS7_get_signer_info(PKCS7 *p7)
{
    if (p7 == NULL || p7->d.ptr == NULL)
        return NULL;
    if (PKCS7_type_is_signed(p7)) {
        return p7->d.sign->signer_info;
    } else if (PKCS7_type_is_signedAndEnveloped(p7)) {
        return p7->d.signed_and_enveloped->signer_info;
    } else
        return NULL;
}

void PKCS7_SIGNER_INFO_get0_algs(PKCS7_SIGNER_INFO *si, EVP_PKEY **pk,
                                 X509_ALGOR **pdig, X509_ALGOR **psig)
{
    if (pk)
        *pk = si->pkey;
    if (pdig)
        *pdig = si->digest_alg;
    if (psig)
        *psig = si->digest_enc_alg;
}

void PKCS7_RECIP_INFO_get0_alg(PKCS7_RECIP_INFO *ri, X509_ALGOR **penc)
{
    if (penc)
        *penc = ri->key_enc_algor;
}

PKCS7_RECIP_INFO *PKCS7_add_recipient(PKCS7 *p7, X509 *x509)
{
    PKCS7_RECIP_INFO *ri;

    if ((ri = PKCS7_RECIP_INFO_new()) == NULL)
        goto err;
    if (PKCS7_RECIP_INFO_set(ri, x509) <= 0)
        goto err;
    if (!PKCS7_add_recipient_info(p7, ri))
        goto err;
    ri->ctx = ossl_pkcs7_get0_ctx(p7);
    return ri;
 err:
    PKCS7_RECIP_INFO_free(ri);
    return NULL;
}

int PKCS7_add_recipient_info(PKCS7 *p7, PKCS7_RECIP_INFO *ri)
{
    int i;
    STACK_OF(PKCS7_RECIP_INFO) *sk;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signedAndEnveloped:
        sk = p7->d.signed_and_enveloped->recipientinfo;
        break;
    case NID_pkcs7_enveloped:
        sk = p7->d.enveloped->recipientinfo;
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    if (!sk_PKCS7_RECIP_INFO_push(sk, ri))
        return 0;
    return 1;
}

static int pkcs7_rsa_encrypt_decrypt_setup(PKCS7_RECIP_INFO *ri, int decrypt)
{
    X509_ALGOR *alg = NULL;

    if (!decrypt) {
        PKCS7_RECIP_INFO_get0_alg(ri, &alg);
        if (alg != NULL)
            return X509_ALGOR_set0(alg, OBJ_nid2obj(NID_rsaEncryption),
                                   V_ASN1_NULL, NULL);
    }
    return 1;
}

int PKCS7_RECIP_INFO_set(PKCS7_RECIP_INFO *p7i, X509 *x509)
{
    int ret;
    EVP_PKEY *pkey = NULL;
    if (!ASN1_INTEGER_set(p7i->version, 0))
        return 0;
    if (!X509_NAME_set(&p7i->issuer_and_serial->issuer,
                       X509_get_issuer_name(x509)))
        return 0;

    ASN1_INTEGER_free(p7i->issuer_and_serial->serial);
    if (!(p7i->issuer_and_serial->serial =
          ASN1_INTEGER_dup(X509_get0_serialNumber(x509))))
        return 0;

    pkey = X509_get0_pubkey(x509);
    if (pkey == NULL)
        return 0;

    if (EVP_PKEY_is_a(pkey, "RSA-PSS"))
        return -2;
    if (EVP_PKEY_is_a(pkey, "RSA")) {
        if (pkcs7_rsa_encrypt_decrypt_setup(p7i, 0) <= 0)
            goto err;
        goto finished;
    }

    if (pkey->ameth == NULL || pkey->ameth->pkey_ctrl == NULL) {
        ERR_raise(ERR_LIB_PKCS7,
                  PKCS7_R_ENCRYPTION_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
        goto err;
    }

    ret = pkey->ameth->pkey_ctrl(pkey, ASN1_PKEY_CTRL_PKCS7_ENCRYPT, 0, p7i);
    if (ret == -2) {
        ERR_raise(ERR_LIB_PKCS7,
                  PKCS7_R_ENCRYPTION_NOT_SUPPORTED_FOR_THIS_KEY_TYPE);
        goto err;
    }
    if (ret <= 0) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_ENCRYPTION_CTRL_FAILURE);
        goto err;
    }
finished:
    if (!X509_up_ref(x509))
        goto err;

    p7i->cert = x509;

    return 1;

 err:
    return 0;
}

X509 *PKCS7_cert_from_signer_info(PKCS7 *p7, PKCS7_SIGNER_INFO *si)
{
    if (PKCS7_type_is_signed(p7))
        return (X509_find_by_issuer_and_serial(p7->d.sign->cert,
                                               si->issuer_and_serial->issuer,
                                               si->
                                               issuer_and_serial->serial));
    else
        return NULL;
}

int PKCS7_set_cipher(PKCS7 *p7, const EVP_CIPHER *cipher)
{
    int i;
    PKCS7_ENC_CONTENT *ec;

    i = OBJ_obj2nid(p7->type);
    switch (i) {
    case NID_pkcs7_signedAndEnveloped:
        ec = p7->d.signed_and_enveloped->enc_data;
        break;
    case NID_pkcs7_enveloped:
        ec = p7->d.enveloped->enc_data;
        break;
    default:
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_WRONG_CONTENT_TYPE);
        return 0;
    }

    /* Check cipher OID exists and has data in it */
    i = EVP_CIPHER_get_type(cipher);
    if (i == NID_undef) {
        ERR_raise(ERR_LIB_PKCS7, PKCS7_R_CIPHER_HAS_NO_OBJECT_IDENTIFIER);
        return 0;
    }

    ec->cipher = cipher;
    ec->ctx = ossl_pkcs7_get0_ctx(p7);
    return 1;
}

/* unfortunately cannot constify BIO_new_NDEF() due to this and CMS_stream() */
int PKCS7_stream(unsigned char ***boundary, PKCS7 *p7)
{
    ASN1_OCTET_STRING *os = NULL;

    switch (OBJ_obj2nid(p7->type)) {
    case NID_pkcs7_data:
        os = p7->d.data;
        break;

    case NID_pkcs7_signedAndEnveloped:
        os = p7->d.signed_and_enveloped->enc_data->enc_data;
        if (os == NULL) {
            os = ASN1_OCTET_STRING_new();
            p7->d.signed_and_enveloped->enc_data->enc_data = os;
        }
        break;

    case NID_pkcs7_enveloped:
        os = p7->d.enveloped->enc_data->enc_data;
        if (os == NULL) {
            os = ASN1_OCTET_STRING_new();
            p7->d.enveloped->enc_data->enc_data = os;
        }
        break;

    case NID_pkcs7_signed:
        os = p7->d.sign->contents->d.data;
        break;

    default:
        os = NULL;
        break;
    }

    if (os == NULL)
        return 0;

    os->flags |= ASN1_STRING_FLAG_NDEF;
    *boundary = &os->data;

    return 1;
}
