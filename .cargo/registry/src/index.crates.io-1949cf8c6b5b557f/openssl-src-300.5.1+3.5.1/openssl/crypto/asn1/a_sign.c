/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/objects.h>
#include <openssl/buffer.h>
#include <openssl/core_names.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"

#ifndef OPENSSL_NO_DEPRECATED_3_0

int ASN1_sign(i2d_of_void *i2d, X509_ALGOR *algor1, X509_ALGOR *algor2,
              ASN1_BIT_STRING *signature, char *data, EVP_PKEY *pkey,
              const EVP_MD *type)
{
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    unsigned char *p, *buf_in = NULL, *buf_out = NULL;
    int i, inl = 0, outl = 0;
    size_t inll = 0, outll = 0;
    X509_ALGOR *a;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    for (i = 0; i < 2; i++) {
        if (i == 0)
            a = algor1;
        else
            a = algor2;
        if (a == NULL)
            continue;
        if (type->pkey_type == NID_dsaWithSHA1) {
            /*
             * special case: RFC 2459 tells us to omit 'parameters' with
             * id-dsa-with-sha1
             */
            ASN1_TYPE_free(a->parameter);
            a->parameter = NULL;
        } else if ((a->parameter == NULL) ||
                   (a->parameter->type != V_ASN1_NULL)) {
            ASN1_TYPE_free(a->parameter);
            if ((a->parameter = ASN1_TYPE_new()) == NULL)
                goto err;
            a->parameter->type = V_ASN1_NULL;
        }
        ASN1_OBJECT_free(a->algorithm);
        a->algorithm = OBJ_nid2obj(type->pkey_type);
        if (a->algorithm == NULL) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_OBJECT_TYPE);
            goto err;
        }
        if (a->algorithm->length == 0) {
            ERR_raise(ERR_LIB_ASN1,
                      ASN1_R_THE_ASN1_OBJECT_IDENTIFIER_IS_NOT_KNOWN_FOR_THIS_MD);
            goto err;
        }
    }
    inl = i2d(data, NULL);
    if (inl <= 0) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    inll = (size_t)inl;
    buf_in = OPENSSL_malloc(inll);
    outll = outl = EVP_PKEY_get_size(pkey);
    buf_out = OPENSSL_malloc(outll);
    if (buf_in == NULL || buf_out == NULL) {
        outl = 0;
        goto err;
    }
    p = buf_in;

    i2d(data, &p);
    if (!EVP_SignInit_ex(ctx, type, NULL)
        || !EVP_SignUpdate(ctx, (unsigned char *)buf_in, inl)
        || !EVP_SignFinal(ctx, (unsigned char *)buf_out,
                          (unsigned int *)&outl, pkey)) {
        outl = 0;
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    ASN1_STRING_set0(signature, buf_out, outl);
    buf_out = NULL;
    /*
     * In the interests of compatibility, I'll make sure that the bit string
     * has a 'not-used bits' value of 0
     */
    ossl_asn1_string_set_bits_left(signature, 0);
 err:
    EVP_MD_CTX_free(ctx);
    OPENSSL_clear_free((char *)buf_in, inll);
    OPENSSL_clear_free((char *)buf_out, outll);
    return outl;
}

#endif

int ASN1_item_sign(const ASN1_ITEM *it, X509_ALGOR *algor1, X509_ALGOR *algor2,
                   ASN1_BIT_STRING *signature, const void *data,
                   EVP_PKEY *pkey, const EVP_MD *md)
{
    return ASN1_item_sign_ex(it, algor1, algor2, signature, data, NULL, pkey,
                             md, NULL, NULL);
}

int ASN1_item_sign_ex(const ASN1_ITEM *it, X509_ALGOR *algor1,
                      X509_ALGOR *algor2, ASN1_BIT_STRING *signature,
                      const void *data, const ASN1_OCTET_STRING *id,
                      EVP_PKEY *pkey, const EVP_MD *md, OSSL_LIB_CTX *libctx,
                      const char *propq)
{
    int rv = 0;
    EVP_MD_CTX *ctx = evp_md_ctx_new_ex(pkey, id, libctx, propq);

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        return 0;
    }
    /* We can use the non _ex variant here since the pkey is already setup */
    if (!EVP_DigestSignInit(ctx, NULL, md, NULL, pkey))
        goto err;

    rv = ASN1_item_sign_ctx(it, algor1, algor2, signature, data, ctx);

 err:
    EVP_PKEY_CTX_free(EVP_MD_CTX_get_pkey_ctx(ctx));
    EVP_MD_CTX_free(ctx);
    return rv;
}

int ASN1_item_sign_ctx(const ASN1_ITEM *it, X509_ALGOR *algor1,
                       X509_ALGOR *algor2, ASN1_BIT_STRING *signature,
                       const void *data, EVP_MD_CTX *ctx)
{
    const EVP_MD *md;
    EVP_PKEY *pkey;
    unsigned char *buf_in = NULL, *buf_out = NULL;
    size_t inl = 0, outl = 0, outll = 0;
    int signid, paramtype, buf_len = 0;
    int rv, pkey_id;

    md = EVP_MD_CTX_get0_md(ctx);
    pkey = EVP_PKEY_CTX_get0_pkey(EVP_MD_CTX_get_pkey_ctx(ctx));

    if (pkey == NULL) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_CONTEXT_NOT_INITIALISED);
        goto err;
    }

    if (pkey->ameth == NULL) {
        EVP_PKEY_CTX *pctx = EVP_MD_CTX_get_pkey_ctx(ctx);
        OSSL_PARAM params[2];
        unsigned char aid[128];
        size_t aid_len = 0;

        if (pctx == NULL
            || !EVP_PKEY_CTX_IS_SIGNATURE_OP(pctx)) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_CONTEXT_NOT_INITIALISED);
            goto err;
        }

        params[0] =
            OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID,
                                              aid, sizeof(aid));
        params[1] = OSSL_PARAM_construct_end();

        if (EVP_PKEY_CTX_get_params(pctx, params) <= 0)
            goto err;

        if ((aid_len = params[0].return_size) == 0) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_DIGEST_AND_KEY_TYPE_NOT_SUPPORTED);
            goto err;
        }

        if (algor1 != NULL) {
            const unsigned char *pp = aid;

            if (d2i_X509_ALGOR(&algor1, &pp, aid_len) == NULL) {
                ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        }

        if (algor2 != NULL) {
            const unsigned char *pp = aid;

            if (d2i_X509_ALGOR(&algor2, &pp, aid_len) == NULL) {
                ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        }

        rv = 3;
    } else if (pkey->ameth->item_sign) {
        rv = pkey->ameth->item_sign(ctx, it, data, algor1, algor2, signature);
        if (rv == 1)
            outl = signature->length;
        /*-
         * Return value meanings:
         * <=0: error.
         *   1: method does everything.
         *   2: carry on as normal.
         *   3: ASN1 method sets algorithm identifiers: just sign.
         */
        if (rv <= 0)
            ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        if (rv <= 1)
            goto err;
    } else {
        rv = 2;
    }

    if (rv == 2) {
        if (md == NULL) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_CONTEXT_NOT_INITIALISED);
            goto err;
        }

        pkey_id =
#ifndef OPENSSL_NO_SM2
            EVP_PKEY_get_id(pkey) == NID_sm2 ? NID_sm2 :
#endif
            pkey->ameth->pkey_id;

        if (!OBJ_find_sigid_by_algs(&signid, EVP_MD_nid(md), pkey_id)) {
            ERR_raise(ERR_LIB_ASN1, ASN1_R_DIGEST_AND_KEY_TYPE_NOT_SUPPORTED);
            goto err;
        }

        paramtype = pkey->ameth->pkey_flags & ASN1_PKEY_SIGPARAM_NULL ?
            V_ASN1_NULL : V_ASN1_UNDEF;
        if (algor1 != NULL
            && !X509_ALGOR_set0(algor1, OBJ_nid2obj(signid), paramtype, NULL))
            goto err;
        if (algor2 != NULL
            && !X509_ALGOR_set0(algor2, OBJ_nid2obj(signid), paramtype, NULL))
            goto err;
    }

    buf_len = ASN1_item_i2d(data, &buf_in, it);
    if (buf_len <= 0) {
        outl = 0;
        ERR_raise(ERR_LIB_ASN1, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    inl = buf_len;
    if (!EVP_DigestSign(ctx, NULL, &outll, buf_in, inl)) {
        outl = 0;
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    outl = outll;
    buf_out = OPENSSL_malloc(outll);
    if (buf_in == NULL || buf_out == NULL) {
        outl = 0;
        goto err;
    }

    if (!EVP_DigestSign(ctx, buf_out, &outl, buf_in, inl)) {
        outl = 0;
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }
    ASN1_STRING_set0(signature, buf_out, outl);
    buf_out = NULL;
    /*
     * In the interests of compatibility, I'll make sure that the bit string
     * has a 'not-used bits' value of 0
     */
    ossl_asn1_string_set_bits_left(signature, 0);
 err:
    OPENSSL_clear_free((char *)buf_in, inl);
    OPENSSL_clear_free((char *)buf_out, outll);
    return outl;
}
