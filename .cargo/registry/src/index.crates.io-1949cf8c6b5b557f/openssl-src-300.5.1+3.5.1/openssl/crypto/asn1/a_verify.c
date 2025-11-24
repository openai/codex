/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include <sys/types.h>

#include "internal/cryptlib.h"

#include <openssl/bn.h>
#include <openssl/x509.h>
#include <openssl/objects.h>
#include <openssl/buffer.h>
#include <openssl/evp.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/rsa.h"

#ifndef OPENSSL_NO_DEPRECATED_3_0

int ASN1_verify(i2d_of_void *i2d, X509_ALGOR *a, ASN1_BIT_STRING *signature,
                char *data, EVP_PKEY *pkey)
{
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    const EVP_MD *type;
    unsigned char *p, *buf_in = NULL;
    int ret = -1, i, inl;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    i = OBJ_obj2nid(a->algorithm);
    type = EVP_get_digestbyname(OBJ_nid2sn(i));
    if (type == NULL) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_MESSAGE_DIGEST_ALGORITHM);
        goto err;
    }

    if (signature->type == V_ASN1_BIT_STRING && signature->flags & 0x7) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_INVALID_BIT_STRING_BITS_LEFT);
        goto err;
    }

    inl = i2d(data, NULL);
    if (inl <= 0) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    buf_in = OPENSSL_malloc((unsigned int)inl);
    if (buf_in == NULL)
        goto err;
    p = buf_in;

    i2d(data, &p);
    ret = EVP_VerifyInit_ex(ctx, type, NULL)
        && EVP_VerifyUpdate(ctx, (unsigned char *)buf_in, inl);

    OPENSSL_clear_free(buf_in, (unsigned int)inl);

    if (!ret) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    ret = -1;

    if (EVP_VerifyFinal(ctx, (unsigned char *)signature->data,
                        (unsigned int)signature->length, pkey) <= 0) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        ret = 0;
        goto err;
    }
    ret = 1;
 err:
    EVP_MD_CTX_free(ctx);
    return ret;
}

#endif

int ASN1_item_verify(const ASN1_ITEM *it, const X509_ALGOR *alg,
                     const ASN1_BIT_STRING *signature, const void *data,
                     EVP_PKEY *pkey)
{
    return ASN1_item_verify_ex(it, alg, signature, data, NULL, pkey, NULL, NULL);
}

int ASN1_item_verify_ex(const ASN1_ITEM *it, const X509_ALGOR *alg,
                        const ASN1_BIT_STRING *signature, const void *data,
                        const ASN1_OCTET_STRING *id, EVP_PKEY *pkey,
                        OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_MD_CTX *ctx;
    int rv = -1;

    if ((ctx = evp_md_ctx_new_ex(pkey, id, libctx, propq)) != NULL) {
        rv = ASN1_item_verify_ctx(it, alg, signature, data, ctx);
        EVP_PKEY_CTX_free(EVP_MD_CTX_get_pkey_ctx(ctx));
        EVP_MD_CTX_free(ctx);
    }
    return rv;
}

int ASN1_item_verify_ctx(const ASN1_ITEM *it, const X509_ALGOR *alg,
                         const ASN1_BIT_STRING *signature, const void *data,
                         EVP_MD_CTX *ctx)
{
    EVP_PKEY *pkey;
    unsigned char *buf_in = NULL;
    int ret = -1, inl = 0;
    int mdnid, pknid;
    size_t inll = 0;

    pkey = EVP_PKEY_CTX_get0_pkey(EVP_MD_CTX_get_pkey_ctx(ctx));

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }

    if (signature->type == V_ASN1_BIT_STRING && signature->flags & 0x7) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_INVALID_BIT_STRING_BITS_LEFT);
        return -1;
    }

    /* Convert signature OID into digest and public key OIDs */
    if (!OBJ_find_sigid_algs(OBJ_obj2nid(alg->algorithm), &mdnid, &pknid)) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_SIGNATURE_ALGORITHM);
        goto err;
    }

    if (mdnid == NID_undef && evp_pkey_is_legacy(pkey)) {
        if (pkey->ameth == NULL || pkey->ameth->item_verify == NULL) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_SIGNATURE_ALGORITHM);
            goto err;
        }
        ret = pkey->ameth->item_verify(ctx, it, data, alg, signature, pkey);
        /*
         * Return values meaning:
         * <=0: error.
         *   1: method does everything.
         *   2: carry on as normal, method has called EVP_DigestVerifyInit()
         */
        if (ret <= 0)
            ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        if (ret <= 1)
            goto err;
    } else {
        const EVP_MD *type = NULL;

        /*
         * We don't yet have the ability for providers to be able to handle
         * X509_ALGOR style parameters. Fortunately the only one that needs this
         * so far is RSA-PSS, so we just special case this for now. In some
         * future version of OpenSSL we should push this to the provider.
         */
        if (mdnid == NID_undef && pknid == EVP_PKEY_RSA_PSS) {
            if (!EVP_PKEY_is_a(pkey, "RSA") && !EVP_PKEY_is_a(pkey, "RSA-PSS")) {
                ERR_raise(ERR_LIB_ASN1, ASN1_R_WRONG_PUBLIC_KEY_TYPE);
                goto err;
            }
            /* This function also calls EVP_DigestVerifyInit */
            if (ossl_rsa_pss_to_ctx(ctx, NULL, alg, pkey) <= 0) {
                ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        } else {
            /* Check public key OID matches public key type */
            if (!EVP_PKEY_is_a(pkey, OBJ_nid2sn(pknid))) {
                ERR_raise(ERR_LIB_ASN1, ASN1_R_WRONG_PUBLIC_KEY_TYPE);
                goto err;
            }

            if (mdnid != NID_undef) {
                type = EVP_get_digestbynid(mdnid);
                if (type == NULL) {
                    ERR_raise_data(ERR_LIB_ASN1,
                                   ASN1_R_UNKNOWN_MESSAGE_DIGEST_ALGORITHM,
                                   "nid=0x%x", mdnid);
                    goto err;
                }
            }

            /*
             * Note that some algorithms (notably Ed25519 and Ed448) may allow
             * a NULL digest value.
             */
            if (!EVP_DigestVerifyInit(ctx, NULL, type, NULL, pkey)) {
                ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
                ret = 0;
                goto err;
            }
        }
    }

    inl = ASN1_item_i2d(data, &buf_in, it);
    if (inl <= 0) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
        ret = -1;
        goto err;
    }
    if (buf_in == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
        ret = -1;
        goto err;
    }
    inll = inl;

    ret = EVP_DigestVerify(ctx, signature->data, (size_t)signature->length,
                           buf_in, inl);
    if (ret <= 0) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    ret = 1;
 err:
    OPENSSL_clear_free(buf_in, inll);
    return ret;
}
