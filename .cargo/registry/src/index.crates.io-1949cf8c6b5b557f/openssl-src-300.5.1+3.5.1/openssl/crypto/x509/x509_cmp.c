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
#include <openssl/asn1.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/core_names.h>
#include "crypto/x509.h"

int X509_issuer_and_serial_cmp(const X509 *a, const X509 *b)
{
    int i;
    const X509_CINF *ai, *bi;

    if (b == NULL)
        return a != NULL;
    if (a == NULL)
        return -1;
    ai = &a->cert_info;
    bi = &b->cert_info;
    i = ASN1_INTEGER_cmp(&ai->serialNumber, &bi->serialNumber);
    if (i != 0)
        return i < 0 ? -1 : 1;
    return X509_NAME_cmp(ai->issuer, bi->issuer);
}

#ifndef OPENSSL_NO_MD5
unsigned long X509_issuer_and_serial_hash(X509 *a)
{
    unsigned long ret = 0;
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    unsigned char md[16];
    char *f = NULL;
    EVP_MD *digest = NULL;

    if (ctx == NULL)
        goto err;
    f = X509_NAME_oneline(a->cert_info.issuer, NULL, 0);
    if (f == NULL)
        goto err;
    digest = EVP_MD_fetch(a->libctx, SN_md5, a->propq);
    if (digest == NULL)
        goto err;

    if (!EVP_DigestInit_ex(ctx, digest, NULL))
        goto err;
    if (!EVP_DigestUpdate(ctx, (unsigned char *)f, strlen(f)))
        goto err;
    if (!EVP_DigestUpdate
        (ctx, (unsigned char *)a->cert_info.serialNumber.data,
         (unsigned long)a->cert_info.serialNumber.length))
        goto err;
    if (!EVP_DigestFinal_ex(ctx, &(md[0]), NULL))
        goto err;
    ret = (((unsigned long)md[0]) | ((unsigned long)md[1] << 8L) |
           ((unsigned long)md[2] << 16L) | ((unsigned long)md[3] << 24L)
        ) & 0xffffffffL;
 err:
    OPENSSL_free(f);
    EVP_MD_free(digest);
    EVP_MD_CTX_free(ctx);
    return ret;
}
#endif

int X509_issuer_name_cmp(const X509 *a, const X509 *b)
{
    return X509_NAME_cmp(a->cert_info.issuer, b->cert_info.issuer);
}

int X509_subject_name_cmp(const X509 *a, const X509 *b)
{
    return X509_NAME_cmp(a->cert_info.subject, b->cert_info.subject);
}

int X509_CRL_cmp(const X509_CRL *a, const X509_CRL *b)
{
    return X509_NAME_cmp(a->crl.issuer, b->crl.issuer);
}

int X509_CRL_match(const X509_CRL *a, const X509_CRL *b)
{
    int rv;

    if ((a->flags & EXFLAG_NO_FINGERPRINT) == 0
            && (b->flags & EXFLAG_NO_FINGERPRINT) == 0)
        rv = memcmp(a->sha1_hash, b->sha1_hash, SHA_DIGEST_LENGTH);
    else
        return -2;

    return rv < 0 ? -1 : rv > 0;
}

X509_NAME *X509_get_issuer_name(const X509 *a)
{
    return a->cert_info.issuer;
}

unsigned long X509_issuer_name_hash(X509 *x)
{
    return X509_NAME_hash_ex(x->cert_info.issuer, NULL, NULL, NULL);
}

#ifndef OPENSSL_NO_MD5
unsigned long X509_issuer_name_hash_old(X509 *x)
{
    return X509_NAME_hash_old(x->cert_info.issuer);
}
#endif

X509_NAME *X509_get_subject_name(const X509 *a)
{
    return a->cert_info.subject;
}

ASN1_INTEGER *X509_get_serialNumber(X509 *a)
{
    return &a->cert_info.serialNumber;
}

const ASN1_INTEGER *X509_get0_serialNumber(const X509 *a)
{
    return &a->cert_info.serialNumber;
}

unsigned long X509_subject_name_hash(X509 *x)
{
    return X509_NAME_hash_ex(x->cert_info.subject, NULL, NULL, NULL);
}

#ifndef OPENSSL_NO_MD5
unsigned long X509_subject_name_hash_old(X509 *x)
{
    return X509_NAME_hash_old(x->cert_info.subject);
}
#endif

/*
 * Compare two certificates: they must be identical for this to work. NB:
 * Although "cmp" operations are generally prototyped to take "const"
 * arguments (eg. for use in STACKs), the way X509 handling is - these
 * operations may involve ensuring the hashes are up-to-date and ensuring
 * certain cert information is cached. So this is the point where the
 * "depth-first" constification tree has to halt with an evil cast.
 */
int X509_cmp(const X509 *a, const X509 *b)
{
    int rv = 0;

    if (a == b) /* for efficiency */
        return 0;

    /* attempt to compute cert hash */
    (void)X509_check_purpose((X509 *)a, -1, 0);
    (void)X509_check_purpose((X509 *)b, -1, 0);

    if ((a->ex_flags & EXFLAG_NO_FINGERPRINT) == 0
            && (b->ex_flags & EXFLAG_NO_FINGERPRINT) == 0)
        rv = memcmp(a->sha1_hash, b->sha1_hash, SHA_DIGEST_LENGTH);
    if (rv != 0)
        return rv < 0 ? -1 : 1;

    /* Check for match against stored encoding too */
    if (!a->cert_info.enc.modified && !b->cert_info.enc.modified) {
        if (a->cert_info.enc.len < b->cert_info.enc.len)
            return -1;
        if (a->cert_info.enc.len > b->cert_info.enc.len)
            return 1;
        rv = memcmp(a->cert_info.enc.enc,
                    b->cert_info.enc.enc, a->cert_info.enc.len);
    }
    return rv < 0 ? -1 : rv > 0;
}

int ossl_x509_add_cert_new(STACK_OF(X509) **p_sk, X509 *cert, int flags)
{
    if (*p_sk == NULL && (*p_sk = sk_X509_new_null()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        return 0;
    }
    return X509_add_cert(*p_sk, cert, flags);
}

int X509_add_cert(STACK_OF(X509) *sk, X509 *cert, int flags)
{
    if (sk == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (cert == NULL)
        return 0;
    if ((flags & X509_ADD_FLAG_NO_DUP) != 0) {
        /*
         * not using sk_X509_set_cmp_func() and sk_X509_find()
         * because this re-orders the certs on the stack
         */
        int i;

        for (i = 0; i < sk_X509_num(sk); i++) {
            if (X509_cmp(sk_X509_value(sk, i), cert) == 0)
                return 1;
        }
    }
    if ((flags & X509_ADD_FLAG_NO_SS) != 0) {
        int ret = X509_self_signed(cert, 0);

        if (ret != 0)
            return ret > 0 ? 1 : 0;
    }
    if ((flags & X509_ADD_FLAG_UP_REF) != 0 && !X509_up_ref(cert))
        return 0;
    if (!sk_X509_insert(sk, cert,
                        (flags & X509_ADD_FLAG_PREPEND) != 0 ? 0 : -1)) {
        if ((flags & X509_ADD_FLAG_UP_REF) != 0)
            X509_free(cert);
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        return 0;
    }
    return 1;
}

int X509_add_certs(STACK_OF(X509) *sk, STACK_OF(X509) *certs, int flags)
/* compiler would allow 'const' for the certs, yet they may get up-ref'ed */
{
    if (sk == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    return ossl_x509_add_certs_new(&sk, certs, flags);
}

int ossl_x509_add_certs_new(STACK_OF(X509) **p_sk, STACK_OF(X509) *certs,
                            int flags)
/* compiler would allow 'const' for the certs, yet they may get up-ref'ed */
{
    int n = sk_X509_num(certs /* may be NULL */);
    int i;

    for (i = 0; i < n; i++) {
        int j = (flags & X509_ADD_FLAG_PREPEND) == 0 ? i : n - 1 - i;
        /* if prepend, add certs in reverse order to keep original order */

        if (!ossl_x509_add_cert_new(p_sk, sk_X509_value(certs, j), flags))
            return 0;
    }
    return 1;
}

int X509_NAME_cmp(const X509_NAME *a, const X509_NAME *b)
{
    int ret;

    if (b == NULL)
        return a != NULL;
    if (a == NULL)
        return -1;

    /* Ensure canonical encoding is present and up to date */
    if (a->canon_enc == NULL || a->modified) {
        ret = i2d_X509_NAME((X509_NAME *)a, NULL);
        if (ret < 0)
            return -2;
    }

    if (b->canon_enc == NULL || b->modified) {
        ret = i2d_X509_NAME((X509_NAME *)b, NULL);
        if (ret < 0)
            return -2;
    }

    ret = a->canon_enclen - b->canon_enclen;
    if (ret == 0 && a->canon_enclen == 0)
        return 0;

    if (ret == 0) {
        if (a->canon_enc == NULL || b->canon_enc == NULL)
            return -2;
        ret = memcmp(a->canon_enc, b->canon_enc, a->canon_enclen);
    }

    return ret < 0 ? -1 : ret > 0;
}

unsigned long X509_NAME_hash_ex(const X509_NAME *x, OSSL_LIB_CTX *libctx,
                                const char *propq, int *ok)
{
    unsigned long ret = 0;
    unsigned char md[SHA_DIGEST_LENGTH];
    EVP_MD *sha1 = EVP_MD_fetch(libctx, "SHA1", propq);
    int i2d_ret;

    /* Make sure X509_NAME structure contains valid cached encoding */
    i2d_ret = i2d_X509_NAME(x, NULL);
    if (ok != NULL)
        *ok = 0;
    if (i2d_ret >= 0 && sha1 != NULL
        && EVP_Digest(x->canon_enc, x->canon_enclen, md, NULL, sha1, NULL)) {
        ret = (((unsigned long)md[0]) | ((unsigned long)md[1] << 8L) |
               ((unsigned long)md[2] << 16L) | ((unsigned long)md[3] << 24L)
               ) & 0xffffffffL;
        if (ok != NULL)
            *ok = 1;
    }
    EVP_MD_free(sha1);
    return ret;
}

#ifndef OPENSSL_NO_MD5
/*
 * I now DER encode the name and hash it.  Since I cache the DER encoding,
 * this is reasonably efficient.
 */
unsigned long X509_NAME_hash_old(const X509_NAME *x)
{
    EVP_MD *md5 = EVP_MD_fetch(NULL, OSSL_DIGEST_NAME_MD5, "-fips");
    EVP_MD_CTX *md_ctx = EVP_MD_CTX_new();
    unsigned long ret = 0;
    unsigned char md[16];

    if (md5 == NULL || md_ctx == NULL)
        goto end;

    /* Make sure X509_NAME structure contains valid cached encoding */
    if (i2d_X509_NAME(x, NULL) < 0)
        goto end;

    if (EVP_DigestInit_ex(md_ctx, md5, NULL)
        && EVP_DigestUpdate(md_ctx, x->bytes->data, x->bytes->length)
        && EVP_DigestFinal_ex(md_ctx, md, NULL))
        ret = (((unsigned long)md[0]) | ((unsigned long)md[1] << 8L) |
               ((unsigned long)md[2] << 16L) | ((unsigned long)md[3] << 24L)
            ) & 0xffffffffL;

 end:
    EVP_MD_CTX_free(md_ctx);
    EVP_MD_free(md5);

    return ret;
}
#endif

/* Search a stack of X509 for a match */
X509 *X509_find_by_issuer_and_serial(STACK_OF(X509) *sk, const X509_NAME *name,
                                     const ASN1_INTEGER *serial)
{
    int i;
    X509 x, *x509 = NULL;

    if (!sk)
        return NULL;

    x.cert_info.serialNumber = *serial;
    x.cert_info.issuer = (X509_NAME *)name; /* won't modify it */

    for (i = 0; i < sk_X509_num(sk); i++) {
        x509 = sk_X509_value(sk, i);
        if (X509_issuer_and_serial_cmp(x509, &x) == 0)
            return x509;
    }
    return NULL;
}

X509 *X509_find_by_subject(STACK_OF(X509) *sk, const X509_NAME *name)
{
    X509 *x509;
    int i;

    for (i = 0; i < sk_X509_num(sk); i++) {
        x509 = sk_X509_value(sk, i);
        if (X509_NAME_cmp(X509_get_subject_name(x509), name) == 0)
            return x509;
    }
    return NULL;
}

EVP_PKEY *X509_get0_pubkey(const X509 *x)
{
    if (x == NULL)
        return NULL;
    return X509_PUBKEY_get0(x->cert_info.key);
}

EVP_PKEY *X509_get_pubkey(X509 *x)
{
    if (x == NULL)
        return NULL;
    return X509_PUBKEY_get(x->cert_info.key);
}

int X509_check_private_key(const X509 *cert, const EVP_PKEY *pkey)
{
    const EVP_PKEY *xk = X509_get0_pubkey(cert);

    if (xk == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNABLE_TO_GET_CERTS_PUBLIC_KEY);
        return 0;
    }
    return ossl_x509_check_private_key(xk, pkey);
}

int ossl_x509_check_private_key(const EVP_PKEY *x, const EVP_PKEY *pkey)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    switch (EVP_PKEY_eq(x, pkey)) {
    case 1:
        return 1;
    case 0:
        ERR_raise(ERR_LIB_X509, X509_R_KEY_VALUES_MISMATCH);
        return 0;
    case -1:
        ERR_raise(ERR_LIB_X509, X509_R_KEY_TYPE_MISMATCH);
        return 0;
    case -2:
        ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_KEY_TYPE);
        /* fall thru */
    default:
        return 0;
    }
}

/*
 * Check a suite B algorithm is permitted: pass in a public key and the NID
 * of its signature (or 0 if no signature). The pflags is a pointer to a
 * flags field which must contain the suite B verification flags.
 */

#ifndef OPENSSL_NO_EC

static int check_suite_b(EVP_PKEY *pkey, int sign_nid, unsigned long *pflags)
{
    char curve_name[80];
    size_t curve_name_len;
    int curve_nid;

    if (pkey == NULL || !EVP_PKEY_is_a(pkey, "EC"))
        return X509_V_ERR_SUITE_B_INVALID_ALGORITHM;

    if (!EVP_PKEY_get_group_name(pkey, curve_name, sizeof(curve_name),
                                 &curve_name_len))
        return X509_V_ERR_SUITE_B_INVALID_CURVE;

    curve_nid = OBJ_txt2nid(curve_name);
    /* Check curve is consistent with LOS */
    if (curve_nid == NID_secp384r1) { /* P-384 */
        /*
         * Check signature algorithm is consistent with curve.
         */
        if (sign_nid != -1 && sign_nid != NID_ecdsa_with_SHA384)
            return X509_V_ERR_SUITE_B_INVALID_SIGNATURE_ALGORITHM;
        if (!(*pflags & X509_V_FLAG_SUITEB_192_LOS))
            return X509_V_ERR_SUITE_B_LOS_NOT_ALLOWED;
        /* If we encounter P-384 we cannot use P-256 later */
        *pflags &= ~X509_V_FLAG_SUITEB_128_LOS_ONLY;
    } else if (curve_nid == NID_X9_62_prime256v1) { /* P-256 */
        if (sign_nid != -1 && sign_nid != NID_ecdsa_with_SHA256)
            return X509_V_ERR_SUITE_B_INVALID_SIGNATURE_ALGORITHM;
        if (!(*pflags & X509_V_FLAG_SUITEB_128_LOS_ONLY))
            return X509_V_ERR_SUITE_B_LOS_NOT_ALLOWED;
    } else {
        return X509_V_ERR_SUITE_B_INVALID_CURVE;
    }
    return X509_V_OK;
}

int X509_chain_check_suiteb(int *perror_depth, X509 *x, STACK_OF(X509) *chain,
                            unsigned long flags)
{
    int rv, i, sign_nid;
    EVP_PKEY *pk;
    unsigned long tflags = flags;

    if (!(flags & X509_V_FLAG_SUITEB_128_LOS))
        return X509_V_OK;

    /* If no EE certificate passed in must be first in chain */
    if (x == NULL) {
        x = sk_X509_value(chain, 0);
        i = 1;
    } else {
        i = 0;
    }
    pk = X509_get0_pubkey(x);

    /*
     * With DANE-EE(3) success, or DANE-EE(3)/PKIX-EE(1) failure we don't build
     * a chain all, just report trust success or failure, but must also report
     * Suite-B errors if applicable.  This is indicated via a NULL chain
     * pointer.  All we need to do is check the leaf key algorithm.
     */
    if (chain == NULL)
        return check_suite_b(pk, -1, &tflags);

    if (X509_get_version(x) != X509_VERSION_3) {
        rv = X509_V_ERR_SUITE_B_INVALID_VERSION;
        /* Correct error depth */
        i = 0;
        goto end;
    }

    /* Check EE key only */
    rv = check_suite_b(pk, -1, &tflags);
    if (rv != X509_V_OK) {
        /* Correct error depth */
        i = 0;
        goto end;
    }
    for (; i < sk_X509_num(chain); i++) {
        sign_nid = X509_get_signature_nid(x);
        x = sk_X509_value(chain, i);
        if (X509_get_version(x) != X509_VERSION_3) {
            rv = X509_V_ERR_SUITE_B_INVALID_VERSION;
            goto end;
        }
        pk = X509_get0_pubkey(x);
        rv = check_suite_b(pk, sign_nid, &tflags);
        if (rv != X509_V_OK)
            goto end;
    }

    /* Final check: root CA signature */
    rv = check_suite_b(pk, X509_get_signature_nid(x), &tflags);
 end:
    if (rv != X509_V_OK) {
        /* Invalid signature or LOS errors are for previous cert */
        if ((rv == X509_V_ERR_SUITE_B_INVALID_SIGNATURE_ALGORITHM
             || rv == X509_V_ERR_SUITE_B_LOS_NOT_ALLOWED) && i)
            i--;
        /*
         * If we have LOS error and flags changed then we are signing P-384
         * with P-256. Use more meaningful error.
         */
        if (rv == X509_V_ERR_SUITE_B_LOS_NOT_ALLOWED && flags != tflags)
            rv = X509_V_ERR_SUITE_B_CANNOT_SIGN_P_384_WITH_P_256;
        if (perror_depth)
            *perror_depth = i;
    }
    return rv;
}

int X509_CRL_check_suiteb(X509_CRL *crl, EVP_PKEY *pk, unsigned long flags)
{
    int sign_nid;
    if (!(flags & X509_V_FLAG_SUITEB_128_LOS))
        return X509_V_OK;
    sign_nid = OBJ_obj2nid(crl->crl.sig_alg.algorithm);
    return check_suite_b(pk, sign_nid, &flags);
}

#else
int X509_chain_check_suiteb(int *perror_depth, X509 *x, STACK_OF(X509) *chain,
                            unsigned long flags)
{
    return 0;
}

int X509_CRL_check_suiteb(X509_CRL *crl, EVP_PKEY *pk, unsigned long flags)
{
    return 0;
}

#endif

/*
 * Not strictly speaking an "up_ref" as a STACK doesn't have a reference
 * count but it has the same effect by duping the STACK and upping the ref of
 * each X509 structure.
 */
STACK_OF(X509) *X509_chain_up_ref(STACK_OF(X509) *chain)
{
    STACK_OF(X509) *ret = sk_X509_dup(chain);
    int i;

    if (ret == NULL)
        return NULL;
    for (i = 0; i < sk_X509_num(ret); i++) {
        X509 *x = sk_X509_value(ret, i);

        if (!X509_up_ref(x))
            goto err;
    }
    return ret;

 err:
    while (i-- > 0)
        X509_free(sk_X509_value(ret, i));
    sk_X509_free(ret);
    return NULL;
}
