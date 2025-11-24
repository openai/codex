/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/ess.h>
#include "internal/sizes.h"
#include "crypto/ess.h"
#include "crypto/x509.h"

static ESS_CERT_ID *ESS_CERT_ID_new_init(const X509 *cert,
                                         int set_issuer_serial);
static ESS_CERT_ID_V2 *ESS_CERT_ID_V2_new_init(const EVP_MD *hash_alg,
                                               const X509 *cert,
                                               int set_issuer_serial);

ESS_SIGNING_CERT *OSSL_ESS_signing_cert_new_init(const X509 *signcert,
                                                 const STACK_OF(X509) *certs,
                                                 int set_issuer_serial)
{
    ESS_CERT_ID *cid = NULL;
    ESS_SIGNING_CERT *sc;
    int i;

    if ((sc = ESS_SIGNING_CERT_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    if (sc->cert_ids == NULL
        && (sc->cert_ids = sk_ESS_CERT_ID_new_null()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((cid = ESS_CERT_ID_new_init(signcert, set_issuer_serial)) == NULL
        || !sk_ESS_CERT_ID_push(sc->cert_ids, cid)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    for (i = 0; i < sk_X509_num(certs); ++i) {
        X509 *cert = sk_X509_value(certs, i);

        if ((cid = ESS_CERT_ID_new_init(cert, 1)) == NULL) {
            ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
            goto err;
        }
        if (!sk_ESS_CERT_ID_push(sc->cert_ids, cid)) {
            ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    return sc;
 err:
    ESS_SIGNING_CERT_free(sc);
    ESS_CERT_ID_free(cid);
    return NULL;
}

static ESS_CERT_ID *ESS_CERT_ID_new_init(const X509 *cert,
                                         int set_issuer_serial)
{
    ESS_CERT_ID *cid = NULL;
    GENERAL_NAME *name = NULL;
    unsigned char cert_sha1[SHA_DIGEST_LENGTH];

    if ((cid = ESS_CERT_ID_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    if (!X509_digest(cert, EVP_sha1(), cert_sha1, NULL)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_X509_LIB);
        goto err;
    }
    if (!ASN1_OCTET_STRING_set(cid->hash, cert_sha1, SHA_DIGEST_LENGTH)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }

    /* Setting the issuer/serial if requested. */
    if (!set_issuer_serial)
        return cid;

    if (cid->issuer_serial == NULL
        && (cid->issuer_serial = ESS_ISSUER_SERIAL_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    if ((name = GENERAL_NAME_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }
    name->type = GEN_DIRNAME;
    if ((name->d.dirn = X509_NAME_dup(X509_get_issuer_name(cert))) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_X509_LIB);
        goto err;
    }
    if (!sk_GENERAL_NAME_push(cid->issuer_serial->issuer, name)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
        goto err;
    }
    name = NULL;            /* Ownership is lost. */
    ASN1_INTEGER_free(cid->issuer_serial->serial);
    if ((cid->issuer_serial->serial
         = ASN1_INTEGER_dup(X509_get0_serialNumber(cert))) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }

    return cid;
 err:
    GENERAL_NAME_free(name);
    ESS_CERT_ID_free(cid);
    return NULL;
}

ESS_SIGNING_CERT_V2 *OSSL_ESS_signing_cert_v2_new_init(const EVP_MD *hash_alg,
                                                       const X509 *signcert,
                                                       const
                                                       STACK_OF(X509) *certs,
                                                       int set_issuer_serial)
{
    ESS_CERT_ID_V2 *cid = NULL;
    ESS_SIGNING_CERT_V2 *sc;
    int i;

    if ((sc = ESS_SIGNING_CERT_V2_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    cid = ESS_CERT_ID_V2_new_init(hash_alg, signcert, set_issuer_serial);
    if (cid == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    if (!sk_ESS_CERT_ID_V2_push(sc->cert_ids, cid)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
        goto err;
    }
    cid = NULL;

    for (i = 0; i < sk_X509_num(certs); ++i) {
        X509 *cert = sk_X509_value(certs, i);

        if ((cid = ESS_CERT_ID_V2_new_init(hash_alg, cert, 1)) == NULL) {
            ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
            goto err;
        }
        if (!sk_ESS_CERT_ID_V2_push(sc->cert_ids, cid)) {
            ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
            goto err;
        }
        cid = NULL;
    }

    return sc;
 err:
    ESS_SIGNING_CERT_V2_free(sc);
    ESS_CERT_ID_V2_free(cid);
    return NULL;
}

static ESS_CERT_ID_V2 *ESS_CERT_ID_V2_new_init(const EVP_MD *hash_alg,
                                               const X509 *cert,
                                               int set_issuer_serial)
{
    ESS_CERT_ID_V2 *cid;
    GENERAL_NAME *name = NULL;
    unsigned char hash[EVP_MAX_MD_SIZE];
    unsigned int hash_len = sizeof(hash);
    X509_ALGOR *alg = NULL;

    memset(hash, 0, sizeof(hash));

    if ((cid = ESS_CERT_ID_V2_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }

    if (!EVP_MD_is_a(hash_alg, SN_sha256)) {
        alg = X509_ALGOR_new();
        if (alg == NULL) {
            ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
            goto err;
        }
        X509_ALGOR_set_md(alg, hash_alg);
        if (alg->algorithm == NULL) {
            ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
            goto err;
        }
        cid->hash_alg = alg;
        alg = NULL;
    } else {
        cid->hash_alg = NULL;
    }

    if (!X509_digest(cert, hash_alg, hash, &hash_len)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_X509_LIB);
        goto err;
    }

    if (!ASN1_OCTET_STRING_set(cid->hash, hash, hash_len)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }

    if (!set_issuer_serial)
        return cid;

    if ((cid->issuer_serial = ESS_ISSUER_SERIAL_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ESS_LIB);
        goto err;
    }
    if ((name = GENERAL_NAME_new()) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }
    name->type = GEN_DIRNAME;
    if ((name->d.dirn = X509_NAME_dup(X509_get_issuer_name(cert))) == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }
    if (!sk_GENERAL_NAME_push(cid->issuer_serial->issuer, name)) {
        ERR_raise(ERR_LIB_ESS, ERR_R_CRYPTO_LIB);
        goto err;
    }
    name = NULL;            /* Ownership is lost. */
    ASN1_INTEGER_free(cid->issuer_serial->serial);
    cid->issuer_serial->serial = ASN1_INTEGER_dup(X509_get0_serialNumber(cert));
    if (cid->issuer_serial->serial == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_ASN1_LIB);
        goto err;
    }

    return cid;
 err:
    X509_ALGOR_free(alg);
    GENERAL_NAME_free(name);
    ESS_CERT_ID_V2_free(cid);
    return NULL;
}

static int ess_issuer_serial_cmp(const ESS_ISSUER_SERIAL *is, const X509 *cert)
{
    GENERAL_NAME *issuer;

    if (is == NULL || cert == NULL || sk_GENERAL_NAME_num(is->issuer) != 1)
        return -1;

    issuer = sk_GENERAL_NAME_value(is->issuer, 0);
    if (issuer->type != GEN_DIRNAME
        || X509_NAME_cmp(issuer->d.dirn, X509_get_issuer_name(cert)) != 0)
        return -1;

    return ASN1_INTEGER_cmp(is->serial, X509_get0_serialNumber(cert));
}

/*
 * Find the cert in |certs| referenced by |cid| if not NULL, else by |cid_v2|.
 * The cert must be the first one in |certs| if and only if |index| is 0.
 * Return 0 on not found, -1 on error, else 1 + the position in |certs|.
 */
static int find(const ESS_CERT_ID *cid, const ESS_CERT_ID_V2 *cid_v2,
                int index, const STACK_OF(X509) *certs)
{
    const X509 *cert;
    EVP_MD *md = NULL;
    char name[OSSL_MAX_NAME_SIZE];
    unsigned char cert_digest[EVP_MAX_MD_SIZE];
    unsigned int len, cid_hash_len;
    const ESS_ISSUER_SERIAL *is;
    int i;
    int ret = -1;

    if (cid == NULL && cid_v2 == NULL) {
        ERR_raise(ERR_LIB_ESS, ERR_R_PASSED_INVALID_ARGUMENT);
        return -1;
    }

    if (cid != NULL)
        strcpy(name, "SHA1");
    else if (cid_v2->hash_alg == NULL)
        strcpy(name, "SHA256");
    else
        OBJ_obj2txt(name, sizeof(name), cid_v2->hash_alg->algorithm, 0);

    (void)ERR_set_mark();
    md = EVP_MD_fetch(NULL, name, NULL);

    if (md == NULL)
        md = (EVP_MD *)EVP_get_digestbyname(name);

    if (md == NULL) {
        (void)ERR_clear_last_mark();
        ERR_raise(ERR_LIB_ESS, ESS_R_ESS_DIGEST_ALG_UNKNOWN);
        goto end;
    }
    (void)ERR_pop_to_mark();

    for (i = 0; i < sk_X509_num(certs); ++i) {
        cert = sk_X509_value(certs, i);

        cid_hash_len = cid != NULL ? cid->hash->length : cid_v2->hash->length;
        if (!X509_digest(cert, md, cert_digest, &len)
                || cid_hash_len != len) {
            ERR_raise(ERR_LIB_ESS, ESS_R_ESS_CERT_DIGEST_ERROR);
            goto end;
        }

        if (memcmp(cid != NULL ? cid->hash->data : cid_v2->hash->data,
                   cert_digest, len) == 0) {
            is = cid != NULL ? cid->issuer_serial : cid_v2->issuer_serial;
            /* Well, it's not really required to match the serial numbers. */
            if (is == NULL || ess_issuer_serial_cmp(is, cert) == 0) {
                if ((i == 0) == (index == 0)) {
                    ret = i + 1;
                    goto end;
                }
                ERR_raise(ERR_LIB_ESS, ESS_R_ESS_CERT_ID_WRONG_ORDER);
                goto end;
            }
        }
    }

    ret = 0;
    ERR_raise(ERR_LIB_ESS, ESS_R_ESS_CERT_ID_NOT_FOUND);
end:
    EVP_MD_free(md);
    return ret;
}

int OSSL_ESS_check_signing_certs(const ESS_SIGNING_CERT *ss,
                                 const ESS_SIGNING_CERT_V2 *ssv2,
                                 const STACK_OF(X509) *chain,
                                 int require_signing_cert)
{
    int n_v1 = ss == NULL ? -1 : sk_ESS_CERT_ID_num(ss->cert_ids);
    int n_v2 = ssv2 == NULL ? -1 : sk_ESS_CERT_ID_V2_num(ssv2->cert_ids);
    int i, ret;

    if (require_signing_cert && ss == NULL && ssv2 == NULL) {
        ERR_raise(ERR_LIB_ESS, ESS_R_MISSING_SIGNING_CERTIFICATE_ATTRIBUTE);
        return -1;
    }
    if (n_v1 == 0 || n_v2 == 0) {
        ERR_raise(ERR_LIB_ESS, ESS_R_EMPTY_ESS_CERT_ID_LIST);
        return -1;
    }
    /* If both ss and ssv2 exist, as required evaluate them independently. */
    for (i = 0; i < n_v1; i++) {
        ret = find(sk_ESS_CERT_ID_value(ss->cert_ids, i), NULL, i, chain);
        if (ret <= 0)
            return ret;
    }
    for (i = 0; i < n_v2; i++) {
        ret = find(NULL, sk_ESS_CERT_ID_V2_value(ssv2->cert_ids, i), i, chain);
        if (ret <= 0)
            return ret;
    }
    return 1;
}
