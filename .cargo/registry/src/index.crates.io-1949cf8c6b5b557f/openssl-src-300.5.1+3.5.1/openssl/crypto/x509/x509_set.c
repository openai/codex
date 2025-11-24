/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/refcount.h"
#include <openssl/asn1.h>
#include <openssl/objects.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "crypto/asn1.h"
#include "crypto/x509.h"
#include "x509_local.h"

int X509_set_version(X509 *x, long version)
{
    if (x == NULL)
        return 0;
    if (version == X509_get_version(x))
        return 1; /* avoid needless modification even re-allocation */
    if (version == X509_VERSION_1) {
        ASN1_INTEGER_free(x->cert_info.version);
        x->cert_info.version = NULL;
        x->cert_info.enc.modified = 1;
        return 1;
    }
    if (x->cert_info.version == NULL) {
        if ((x->cert_info.version = ASN1_INTEGER_new()) == NULL)
            return 0;
    }
    if (!ASN1_INTEGER_set(x->cert_info.version, version))
        return 0;
    x->cert_info.enc.modified = 1;
    return 1;
}

int X509_set_serialNumber(X509 *x, ASN1_INTEGER *serial)
{
    ASN1_INTEGER *in;

    if (x == NULL)
        return 0;
    in = &x->cert_info.serialNumber;
    if (in != serial)
        return ASN1_STRING_copy(in, serial);
    x->cert_info.enc.modified = 1;
    return 1;
}

int X509_set_issuer_name(X509 *x, const X509_NAME *name)
{
    if (x == NULL || !X509_NAME_set(&x->cert_info.issuer, name))
        return 0;
    x->cert_info.enc.modified = 1;
    return 1;
}

int X509_set_subject_name(X509 *x, const X509_NAME *name)
{
    if (x == NULL || !X509_NAME_set(&x->cert_info.subject, name))
        return 0;
    x->cert_info.enc.modified = 1;
    return 1;
}

int ossl_x509_set1_time(int *modified, ASN1_TIME **ptm, const ASN1_TIME *tm)
{
    ASN1_TIME *new;

    if (*ptm == tm)
        return 1;
    new = ASN1_STRING_dup(tm);
    if (tm != NULL && new == NULL)
        return 0;
    ASN1_TIME_free(*ptm);
    *ptm = new;
    if (modified != NULL)
        *modified = 1;
    return 1;
}

int X509_set1_notBefore(X509 *x, const ASN1_TIME *tm)
{
    if (x == NULL || tm == NULL)
        return 0;
    return ossl_x509_set1_time(&x->cert_info.enc.modified,
                               &x->cert_info.validity.notBefore, tm);
}

int X509_set1_notAfter(X509 *x, const ASN1_TIME *tm)
{
    if (x == NULL || tm == NULL)
        return 0;
    return ossl_x509_set1_time(&x->cert_info.enc.modified,
                               &x->cert_info.validity.notAfter, tm);
}

int X509_set_pubkey(X509 *x, EVP_PKEY *pkey)
{
    if (x == NULL)
        return 0;
    if (!X509_PUBKEY_set(&(x->cert_info.key), pkey))
        return 0;
    x->cert_info.enc.modified = 1;
    return 1;
}

int X509_up_ref(X509 *x)
{
    int i;

    if (CRYPTO_UP_REF(&x->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("X509", i, x);
    REF_ASSERT_ISNT(i < 2);
    return i > 1;
}

long X509_get_version(const X509 *x)
{
    return ASN1_INTEGER_get(x->cert_info.version);
}

const ASN1_TIME *X509_get0_notBefore(const X509 *x)
{
    return x->cert_info.validity.notBefore;
}

const ASN1_TIME *X509_get0_notAfter(const X509 *x)
{
    return x->cert_info.validity.notAfter;
}

ASN1_TIME *X509_getm_notBefore(const X509 *x)
{
    return x->cert_info.validity.notBefore;
}

ASN1_TIME *X509_getm_notAfter(const X509 *x)
{
    return x->cert_info.validity.notAfter;
}

int X509_get_signature_type(const X509 *x)
{
    return EVP_PKEY_type(OBJ_obj2nid(x->sig_alg.algorithm));
}

X509_PUBKEY *X509_get_X509_PUBKEY(const X509 *x)
{
    return x->cert_info.key;
}

const STACK_OF(X509_EXTENSION) *X509_get0_extensions(const X509 *x)
{
    return x->cert_info.extensions;
}

void X509_get0_uids(const X509 *x, const ASN1_BIT_STRING **piuid,
                    const ASN1_BIT_STRING **psuid)
{
    if (piuid != NULL)
        *piuid = x->cert_info.issuerUID;
    if (psuid != NULL)
        *psuid = x->cert_info.subjectUID;
}

const X509_ALGOR *X509_get0_tbs_sigalg(const X509 *x)
{
    return &x->cert_info.signature;
}

int X509_SIG_INFO_get(const X509_SIG_INFO *siginf, int *mdnid, int *pknid,
                      int *secbits, uint32_t *flags)
{
    if (mdnid != NULL)
        *mdnid = siginf->mdnid;
    if (pknid != NULL)
        *pknid = siginf->pknid;
    if (secbits != NULL)
        *secbits = siginf->secbits;
    if (flags != NULL)
        *flags = siginf->flags;
    return (siginf->flags & X509_SIG_INFO_VALID) != 0;
}

void X509_SIG_INFO_set(X509_SIG_INFO *siginf, int mdnid, int pknid,
                       int secbits, uint32_t flags)
{
    siginf->mdnid = mdnid;
    siginf->pknid = pknid;
    siginf->secbits = secbits;
    siginf->flags = flags;
}

int X509_get_signature_info(X509 *x, int *mdnid, int *pknid, int *secbits,
                            uint32_t *flags)
{
    X509_check_purpose(x, -1, -1);
    return X509_SIG_INFO_get(&x->siginf, mdnid, pknid, secbits, flags);
}

/* Modify *siginf according to alg and sig. Return 1 on success, else 0. */
static int x509_sig_info_init(X509_SIG_INFO *siginf, const X509_ALGOR *alg,
                              const ASN1_STRING *sig, const EVP_PKEY *pubkey)
{
    int pknid, mdnid, md_size;
    const EVP_MD *md;
    const EVP_PKEY_ASN1_METHOD *ameth;

    siginf->mdnid = NID_undef;
    siginf->pknid = NID_undef;
    siginf->secbits = -1;
    siginf->flags = 0;
    if (!OBJ_find_sigid_algs(OBJ_obj2nid(alg->algorithm), &mdnid, &pknid)
            || pknid == NID_undef) {
        ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_SIGID_ALGS);
        return 0;
    }
    siginf->mdnid = mdnid;
    siginf->pknid = pknid;

    switch (mdnid) {
    case NID_undef:
        /* If we have one, use a custom handler for this algorithm */
        ameth = EVP_PKEY_asn1_find(NULL, pknid);
        if (ameth != NULL && ameth->siginf_set != NULL
                && ameth->siginf_set(siginf, alg, sig))
           break;
        if (pubkey != NULL) {
            int secbits;

            secbits = EVP_PKEY_get_security_bits(pubkey);
            if (secbits != 0) {
                siginf->secbits = secbits;
                break;
            }
        }
        ERR_raise(ERR_LIB_X509, X509_R_ERROR_USING_SIGINF_SET);
        return 0;
        /*
         * SHA1 and MD5 are known to be broken. Reduce security bits so that
         * they're no longer accepted at security level 1.
         * The real values don't really matter as long as they're lower than 80,
         * which is our security level 1.
         */
    case NID_sha1:
        /*
         * https://eprint.iacr.org/2020/014 puts a chosen-prefix attack
         * for SHA1 at2^63.4
         */
        siginf->secbits = 63;
        break;
    case NID_md5:
        /*
         * https://documents.epfl.ch/users/l/le/lenstra/public/papers/lat.pdf
         * puts a chosen-prefix attack for MD5 at 2^39.
         */
        siginf->secbits = 39;
        break;
    case NID_id_GostR3411_94:
        /*
         * There is a collision attack on GOST R 34.11-94 at 2^105, see
         * https://link.springer.com/chapter/10.1007%2F978-3-540-85174-5_10
         */
        siginf->secbits = 105;
        break;
    default:
        /* Security bits: half number of bits in digest */
        if ((md = EVP_get_digestbynid(mdnid)) == NULL) {
            ERR_raise(ERR_LIB_X509, X509_R_ERROR_GETTING_MD_BY_NID);
            return 0;
        }
        md_size = EVP_MD_get_size(md);
        if (md_size <= 0)
            return 0;
        siginf->secbits = md_size * 4;
        break;
    }
    switch (mdnid) {
    case NID_sha1:
    case NID_sha256:
    case NID_sha384:
    case NID_sha512:
        siginf->flags |= X509_SIG_INFO_TLS;
    }
    siginf->flags |= X509_SIG_INFO_VALID;
    return 1;
}

/* Returns 1 on success, 0 on failure */
int ossl_x509_init_sig_info(X509 *x)
{
    return x509_sig_info_init(&x->siginf, &x->sig_alg, &x->signature,
                              X509_PUBKEY_get0(x->cert_info.key));
}
