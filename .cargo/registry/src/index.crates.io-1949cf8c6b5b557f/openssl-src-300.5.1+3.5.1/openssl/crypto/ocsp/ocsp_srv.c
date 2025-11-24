/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/ocsp.h>
#include "ocsp_local.h"

/*
 * Utility functions related to sending OCSP responses and extracting
 * relevant information from the request.
 */
int OCSP_request_onereq_count(OCSP_REQUEST *req)
{
    return sk_OCSP_ONEREQ_num(req->tbsRequest.requestList);
}

OCSP_ONEREQ *OCSP_request_onereq_get0(OCSP_REQUEST *req, int i)
{
    return sk_OCSP_ONEREQ_value(req->tbsRequest.requestList, i);
}

OCSP_CERTID *OCSP_onereq_get0_id(OCSP_ONEREQ *one)
{
    return one->reqCert;
}

int OCSP_id_get0_info(ASN1_OCTET_STRING **piNameHash, ASN1_OBJECT **pmd,
                      ASN1_OCTET_STRING **pikeyHash,
                      ASN1_INTEGER **pserial, OCSP_CERTID *cid)
{
    if (!cid)
        return 0;
    if (pmd)
        *pmd = cid->hashAlgorithm.algorithm;
    if (piNameHash)
        *piNameHash = &cid->issuerNameHash;
    if (pikeyHash)
        *pikeyHash = &cid->issuerKeyHash;
    if (pserial)
        *pserial = &cid->serialNumber;
    return 1;
}

int OCSP_request_is_signed(OCSP_REQUEST *req)
{
    if (req->optionalSignature)
        return 1;
    return 0;
}

/* Create an OCSP response and encode an optional basic response */
OCSP_RESPONSE *OCSP_response_create(int status, OCSP_BASICRESP *bs)
{
    OCSP_RESPONSE *rsp = NULL;

    if ((rsp = OCSP_RESPONSE_new()) == NULL)
        goto err;
    if (!(ASN1_ENUMERATED_set(rsp->responseStatus, status)))
        goto err;
    if (!bs)
        return rsp;
    if ((rsp->responseBytes = OCSP_RESPBYTES_new()) == NULL)
        goto err;
    rsp->responseBytes->responseType = OBJ_nid2obj(NID_id_pkix_OCSP_basic);
    if (!ASN1_item_pack
        (bs, ASN1_ITEM_rptr(OCSP_BASICRESP), &rsp->responseBytes->response))
         goto err;
    return rsp;
 err:
    OCSP_RESPONSE_free(rsp);
    return NULL;
}

OCSP_SINGLERESP *OCSP_basic_add1_status(OCSP_BASICRESP *rsp,
                                        OCSP_CERTID *cid,
                                        int status, int reason,
                                        ASN1_TIME *revtime,
                                        ASN1_TIME *thisupd,
                                        ASN1_TIME *nextupd)
{
    OCSP_SINGLERESP *single = NULL;
    OCSP_CERTSTATUS *cs;
    OCSP_REVOKEDINFO *ri;

    if (rsp->tbsResponseData.responses == NULL
        && (rsp->tbsResponseData.responses
                = sk_OCSP_SINGLERESP_new_null()) == NULL)
        goto err;

    if ((single = OCSP_SINGLERESP_new()) == NULL)
        goto err;

    if (!ASN1_TIME_to_generalizedtime(thisupd, &single->thisUpdate))
        goto err;
    if (nextupd &&
        !ASN1_TIME_to_generalizedtime(nextupd, &single->nextUpdate))
        goto err;

    OCSP_CERTID_free(single->certId);

    if ((single->certId = OCSP_CERTID_dup(cid)) == NULL)
        goto err;

    cs = single->certStatus;
    switch (cs->type = status) {
    case V_OCSP_CERTSTATUS_REVOKED:
        if (!revtime) {
            ERR_raise(ERR_LIB_OCSP, OCSP_R_NO_REVOKED_TIME);
            goto err;
        }
        if ((cs->value.revoked = ri = OCSP_REVOKEDINFO_new()) == NULL)
            goto err;
        if (!ASN1_TIME_to_generalizedtime(revtime, &ri->revocationTime))
            goto err;
        if (reason != OCSP_REVOKED_STATUS_NOSTATUS) {
            if ((ri->revocationReason = ASN1_ENUMERATED_new()) == NULL)
                goto err;
            if (!(ASN1_ENUMERATED_set(ri->revocationReason, reason)))
                goto err;
        }
        break;

    case V_OCSP_CERTSTATUS_GOOD:
        if ((cs->value.good = ASN1_NULL_new()) == NULL)
            goto err;
        break;

    case V_OCSP_CERTSTATUS_UNKNOWN:
        if ((cs->value.unknown = ASN1_NULL_new()) == NULL)
            goto err;
        break;

    default:
        goto err;

    }
    if (!(sk_OCSP_SINGLERESP_push(rsp->tbsResponseData.responses, single)))
        goto err;
    return single;
 err:
    OCSP_SINGLERESP_free(single);
    return NULL;
}

/* Add a certificate to an OCSP request */
int OCSP_basic_add1_cert(OCSP_BASICRESP *resp, X509 *cert)
{
    return ossl_x509_add_cert_new(&resp->certs, cert, X509_ADD_FLAG_UP_REF);
}

/*
 * Sign an OCSP response using the parameters contained in the digest context,
 * set the responderID to the subject name in the signer's certificate, and
 * include one or more optional certificates in the response.
 */
int OCSP_basic_sign_ctx(OCSP_BASICRESP *brsp,
                    X509 *signer, EVP_MD_CTX *ctx,
                    STACK_OF(X509) *certs, unsigned long flags)
{
    OCSP_RESPID *rid;
    EVP_PKEY *pkey;

    if (ctx == NULL || EVP_MD_CTX_get_pkey_ctx(ctx) == NULL) {
        ERR_raise(ERR_LIB_OCSP, OCSP_R_NO_SIGNER_KEY);
        goto err;
    }

    pkey = EVP_PKEY_CTX_get0_pkey(EVP_MD_CTX_get_pkey_ctx(ctx));
    if (pkey == NULL || !X509_check_private_key(signer, pkey)) {
        ERR_raise(ERR_LIB_OCSP, OCSP_R_PRIVATE_KEY_DOES_NOT_MATCH_CERTIFICATE);
        goto err;
    }

    if (!(flags & OCSP_NOCERTS)) {
        if (!OCSP_basic_add1_cert(brsp, signer)
            || !X509_add_certs(brsp->certs, certs, X509_ADD_FLAG_UP_REF))
            goto err;
    }

    rid = &brsp->tbsResponseData.responderId;
    if (flags & OCSP_RESPID_KEY) {
        if (!OCSP_RESPID_set_by_key(rid, signer))
            goto err;
    } else if (!OCSP_RESPID_set_by_name(rid, signer)) {
        goto err;
    }

    if (!(flags & OCSP_NOTIME) &&
        !X509_gmtime_adj(brsp->tbsResponseData.producedAt, 0))
        goto err;

    /*
     * Right now, I think that not doing double hashing is the right thing.
     * -- Richard Levitte
     */
    if (!OCSP_BASICRESP_sign_ctx(brsp, ctx, 0))
        goto err;

    return 1;
 err:
    return 0;
}

int OCSP_basic_sign(OCSP_BASICRESP *brsp,
                    X509 *signer, EVP_PKEY *key, const EVP_MD *dgst,
                    STACK_OF(X509) *certs, unsigned long flags)
{
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    EVP_PKEY_CTX *pkctx = NULL;
    int i;

    if (ctx == NULL)
        return 0;

    if (!EVP_DigestSignInit_ex(ctx, &pkctx, EVP_MD_get0_name(dgst),
                               signer->libctx, signer->propq, key, NULL)) {
        EVP_MD_CTX_free(ctx);
        return 0;
    }
    i = OCSP_basic_sign_ctx(brsp, signer, ctx, certs, flags);
    EVP_MD_CTX_free(ctx);
    return i;
}

int OCSP_RESPID_set_by_name(OCSP_RESPID *respid, X509 *cert)
{
    if (!X509_NAME_set(&respid->value.byName, X509_get_subject_name(cert)))
        return 0;

    respid->type = V_OCSP_RESPID_NAME;

    return 1;
}

int OCSP_RESPID_set_by_key_ex(OCSP_RESPID *respid, X509 *cert,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    ASN1_OCTET_STRING *byKey = NULL;
    unsigned char md[SHA_DIGEST_LENGTH];
    EVP_MD *sha1 = EVP_MD_fetch(libctx, "SHA1", propq);
    int ret = 0;

    if (sha1 == NULL)
        return 0;

    /* RFC2560 requires SHA1 */
    if (!X509_pubkey_digest(cert, sha1, md, NULL))
        goto err;

    byKey = ASN1_OCTET_STRING_new();
    if (byKey == NULL)
        goto err;

    if (!(ASN1_OCTET_STRING_set(byKey, md, SHA_DIGEST_LENGTH))) {
        ASN1_OCTET_STRING_free(byKey);
        goto err;
    }

    respid->type = V_OCSP_RESPID_KEY;
    respid->value.byKey = byKey;

    ret = 1;
 err:
    EVP_MD_free(sha1);
    return ret;
}

int OCSP_RESPID_set_by_key(OCSP_RESPID *respid, X509 *cert)
{
    if (cert == NULL)
        return 0;
    return OCSP_RESPID_set_by_key_ex(respid, cert, cert->libctx, cert->propq);
}

int OCSP_RESPID_match_ex(OCSP_RESPID *respid, X509 *cert, OSSL_LIB_CTX *libctx,
                         const char *propq)
{
    EVP_MD *sha1 = NULL;
    int ret = 0;

    if (respid->type == V_OCSP_RESPID_KEY) {
        unsigned char md[SHA_DIGEST_LENGTH];

        sha1 = EVP_MD_fetch(libctx, "SHA1", propq);
        if (sha1 == NULL)
            goto err;

        if (respid->value.byKey == NULL)
            goto err;

        /* RFC2560 requires SHA1 */
        if (!X509_pubkey_digest(cert, sha1, md, NULL))
            goto err;

        ret = (ASN1_STRING_length(respid->value.byKey) == SHA_DIGEST_LENGTH)
              && (memcmp(ASN1_STRING_get0_data(respid->value.byKey), md,
                         SHA_DIGEST_LENGTH) == 0);
    } else if (respid->type == V_OCSP_RESPID_NAME) {
        if (respid->value.byName == NULL)
            return 0;

        return X509_NAME_cmp(respid->value.byName,
                             X509_get_subject_name(cert)) == 0;
    }

 err:
    EVP_MD_free(sha1);
    return ret;
}

int OCSP_RESPID_match(OCSP_RESPID *respid, X509 *cert)
{
    if (cert == NULL)
        return 0;
    return OCSP_RESPID_match_ex(respid, cert, cert->libctx, cert->propq);
}
