/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include <openssl/x509v3.h>
#include <openssl/x509_vfy.h>
#include "crypto/x509.h"
#include "internal/tsan_assist.h"
#include "x509_local.h"

static int check_ssl_ca(const X509 *x);
static int check_purpose_ssl_client(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf);
static int check_purpose_ssl_server(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf);
static int check_purpose_ns_ssl_server(const X509_PURPOSE *xp, const X509 *x,
                                       int non_leaf);
static int purpose_smime(const X509 *x, int non_leaf);
static int check_purpose_smime_sign(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf);
static int check_purpose_smime_encrypt(const X509_PURPOSE *xp, const X509 *x,
                                       int non_leaf);
static int check_purpose_crl_sign(const X509_PURPOSE *xp, const X509 *x,
                                  int non_leaf);
static int check_purpose_timestamp_sign(const X509_PURPOSE *xp, const X509 *x,
                                        int non_leaf);
static int check_purpose_code_sign(const X509_PURPOSE *xp, const X509 *x,
                                        int non_leaf);
static int no_check_purpose(const X509_PURPOSE *xp, const X509 *x,
                            int non_leaf);
static int check_purpose_ocsp_helper(const X509_PURPOSE *xp, const X509 *x,
                                     int non_leaf);

static int xp_cmp(const X509_PURPOSE *const *a, const X509_PURPOSE *const *b);
static void xptable_free(X509_PURPOSE *p);

/* note that the id must be unique and for the standard entries == idx + 1 */
static X509_PURPOSE xstandard[] = {
    {X509_PURPOSE_SSL_CLIENT, X509_TRUST_SSL_CLIENT, 0,
     check_purpose_ssl_client, "SSL client", "sslclient", NULL},
    {X509_PURPOSE_SSL_SERVER, X509_TRUST_SSL_SERVER, 0,
     check_purpose_ssl_server, "SSL server", "sslserver", NULL},
    {X509_PURPOSE_NS_SSL_SERVER, X509_TRUST_SSL_SERVER, 0,
     check_purpose_ns_ssl_server, "Netscape SSL server", "nssslserver", NULL},
    {X509_PURPOSE_SMIME_SIGN, X509_TRUST_EMAIL, 0, check_purpose_smime_sign,
     "S/MIME signing", "smimesign", NULL},
    {X509_PURPOSE_SMIME_ENCRYPT, X509_TRUST_EMAIL, 0,
     check_purpose_smime_encrypt, "S/MIME encryption", "smimeencrypt", NULL},
    {X509_PURPOSE_CRL_SIGN, X509_TRUST_COMPAT, 0, check_purpose_crl_sign,
     "CRL signing", "crlsign", NULL},
    {X509_PURPOSE_ANY, X509_TRUST_DEFAULT, 0, no_check_purpose,
     "Any Purpose", "any",
     NULL},
    {X509_PURPOSE_OCSP_HELPER, X509_TRUST_COMPAT, 0, check_purpose_ocsp_helper,
     "OCSP helper", "ocsphelper", NULL},
    {X509_PURPOSE_TIMESTAMP_SIGN, X509_TRUST_TSA, 0,
     check_purpose_timestamp_sign, "Time Stamp signing", "timestampsign",
     NULL},
    {X509_PURPOSE_CODE_SIGN, X509_TRUST_OBJECT_SIGN, 0,
     check_purpose_code_sign, "Code signing", "codesign",
     NULL},
};

#define X509_PURPOSE_COUNT OSSL_NELEM(xstandard)

/* the id must be unique, but there may be gaps and maybe table is not sorted */
static STACK_OF(X509_PURPOSE) *xptable = NULL;

static int xp_cmp(const X509_PURPOSE *const *a, const X509_PURPOSE *const *b)
{
    return (*a)->purpose - (*b)->purpose;
}

/*
 * As much as I'd like to make X509_check_purpose use a "const" X509* I really
 * can't because it does recalculate hashes and do other non-const things.
 * If id == -1 it just calls x509v3_cache_extensions() for its side-effect.
 * Returns 1 on success, 0 if x does not allow purpose, -1 on (internal) error.
 */
int X509_check_purpose(X509 *x, int id, int non_leaf)
{
    int idx;
    const X509_PURPOSE *pt;

    if (!ossl_x509v3_cache_extensions(x))
        return -1;
    if (id == -1)
        return 1;

    idx = X509_PURPOSE_get_by_id(id);
    if (idx == -1)
        return -1;
    pt = X509_PURPOSE_get0(idx);
    return pt->check_purpose(pt, x, non_leaf);
}

/* resets to default (any) purpose if purpose == X509_PURPOSE_DEFAULT_ANY (0) */
int X509_PURPOSE_set(int *p, int purpose)
{
    if (purpose != X509_PURPOSE_DEFAULT_ANY && X509_PURPOSE_get_by_id(purpose) == -1) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_PURPOSE);
        return 0;
    }
    *p = purpose;
    return 1;
}

int X509_PURPOSE_get_count(void)
{
    if (!xptable)
        return X509_PURPOSE_COUNT;
    return sk_X509_PURPOSE_num(xptable) + X509_PURPOSE_COUNT;
}

/* find smallest identifier not yet taken - note there might be gaps */
int X509_PURPOSE_get_unused_id(ossl_unused OSSL_LIB_CTX *libctx)
{
    int id = X509_PURPOSE_MAX + 1;

    while (X509_PURPOSE_get_by_id(id) != -1)
        id++;
    return id; /* is guaranteed to be unique and > X509_PURPOSE_MAX and != 0 */
}

X509_PURPOSE *X509_PURPOSE_get0(int idx)
{
    if (idx < 0)
        return NULL;
    if (idx < (int)X509_PURPOSE_COUNT)
        return xstandard + idx;
    return sk_X509_PURPOSE_value(xptable, idx - X509_PURPOSE_COUNT);
}

int X509_PURPOSE_get_by_sname(const char *sname)
{
    int i;
    X509_PURPOSE *xptmp;

    for (i = 0; i < X509_PURPOSE_get_count(); i++) {
        xptmp = X509_PURPOSE_get0(i);
        if (strcmp(xptmp->sname, sname) == 0)
            return i;
    }
    return -1;
}

/* Returns -1 on error, else an index => 0 in standard/extended purpose table */
int X509_PURPOSE_get_by_id(int purpose)
{
    X509_PURPOSE tmp;
    int idx;

    if (purpose >= X509_PURPOSE_MIN && purpose <= X509_PURPOSE_MAX)
        return purpose - X509_PURPOSE_MIN;
    if (xptable == NULL)
        return -1;
    tmp.purpose = purpose;
    idx = sk_X509_PURPOSE_find(xptable, &tmp);
    if (idx < 0)
        return -1;
    return idx + X509_PURPOSE_COUNT;
}

/*
 * Add purpose entry identified by |sname|. |id| must be >= X509_PURPOSE_MIN.
 * May also be used to modify existing entry, including changing its id.
 */
int X509_PURPOSE_add(int id, int trust, int flags,
                     int (*ck) (const X509_PURPOSE *, const X509 *, int),
                     const char *name, const char *sname, void *arg)
{
    int old_id = 0;
    int idx;
    X509_PURPOSE *ptmp;

    if (id < X509_PURPOSE_MIN) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_PURPOSE);
        return 0;
    }
    if (trust < X509_TRUST_DEFAULT || name == NULL || sname == NULL || ck == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    /* This is set according to what we change: application can't set it */
    flags &= ~X509_PURPOSE_DYNAMIC;
    /* This will always be set for application modified trust entries */
    flags |= X509_PURPOSE_DYNAMIC_NAME;

    /* Get existing entry if any */
    idx = X509_PURPOSE_get_by_sname(sname);
    if (idx == -1) { /* Need a new entry */
        if (X509_PURPOSE_get_by_id(id) != -1) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_PURPOSE_NOT_UNIQUE);
            return 0;
        }
        if ((ptmp = OPENSSL_malloc(sizeof(*ptmp))) == NULL)
            return 0;
        ptmp->flags = X509_PURPOSE_DYNAMIC;
    } else {
        ptmp = X509_PURPOSE_get0(idx);
        old_id = ptmp->purpose;
        if (id != old_id && X509_PURPOSE_get_by_id(id) != -1) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_PURPOSE_NOT_UNIQUE);
            return 0;
        }
    }

    /* OPENSSL_free existing name if dynamic */
    if ((ptmp->flags & X509_PURPOSE_DYNAMIC_NAME) != 0) {
        OPENSSL_free(ptmp->name);
        OPENSSL_free(ptmp->sname);
    }
    /* Dup supplied name */
    ptmp->name = OPENSSL_strdup(name);
    ptmp->sname = OPENSSL_strdup(sname);
    if (ptmp->name == NULL || ptmp->sname == NULL)
        goto err;
    /* Keep the dynamic flag of existing entry */
    ptmp->flags &= X509_PURPOSE_DYNAMIC;
    /* Set all other flags */
    ptmp->flags |= flags;

    ptmp->purpose = id;
    ptmp->trust = trust;
    ptmp->check_purpose = ck;
    ptmp->usr_data = arg;

    /* If its a new entry manage the dynamic table */
    if (idx == -1) {
        if (xptable == NULL
            && (xptable = sk_X509_PURPOSE_new(xp_cmp)) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
            goto err;
        }
        if (!sk_X509_PURPOSE_push(xptable, ptmp)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
            goto err;
        }
    } else if (id != old_id) {
        /* on changing existing entry id, make sure to reset 'sorted' */
        (void)sk_X509_PURPOSE_set(xptable, idx, ptmp);
    }
    return 1;
 err:
    if (idx == -1) {
        OPENSSL_free(ptmp->name);
        OPENSSL_free(ptmp->sname);
        OPENSSL_free(ptmp);
    }
    return 0;
}

static void xptable_free(X509_PURPOSE *p)
{
    if (p == NULL)
        return;
    if ((p->flags & X509_PURPOSE_DYNAMIC) != 0) {
        if ((p->flags & X509_PURPOSE_DYNAMIC_NAME) != 0) {
            OPENSSL_free(p->name);
            OPENSSL_free(p->sname);
        }
        OPENSSL_free(p);
    }
}

void X509_PURPOSE_cleanup(void)
{
    sk_X509_PURPOSE_pop_free(xptable, xptable_free);
    xptable = NULL;
}

int X509_PURPOSE_get_id(const X509_PURPOSE *xp)
{
    return xp->purpose;
}

char *X509_PURPOSE_get0_name(const X509_PURPOSE *xp)
{
    return xp->name;
}

char *X509_PURPOSE_get0_sname(const X509_PURPOSE *xp)
{
    return xp->sname;
}

int X509_PURPOSE_get_trust(const X509_PURPOSE *xp)
{
    return xp->trust;
}

static int nid_cmp(const int *a, const int *b)
{
    return *a - *b;
}

DECLARE_OBJ_BSEARCH_CMP_FN(int, int, nid);
IMPLEMENT_OBJ_BSEARCH_CMP_FN(int, int, nid);

int X509_supported_extension(X509_EXTENSION *ex)
{
    /*
     * This table is a list of the NIDs of supported extensions: that is
     * those which are used by the verify process. If an extension is
     * critical and doesn't appear in this list then the verify process will
     * normally reject the certificate. The list must be kept in numerical
     * order because it will be searched using bsearch.
     */
    static const int supported_nids[] = {
        NID_netscape_cert_type, /* 71 */
        NID_key_usage,          /* 83 */
        NID_subject_alt_name,   /* 85 */
        NID_basic_constraints,  /* 87 */
        NID_certificate_policies, /* 89 */
        NID_crl_distribution_points, /* 103 */
        NID_ext_key_usage,      /* 126 */
#ifndef OPENSSL_NO_RFC3779
        NID_sbgp_ipAddrBlock,   /* 290 */
        NID_sbgp_autonomousSysNum, /* 291 */
#endif
        NID_id_pkix_OCSP_noCheck, /* 369 */
        NID_policy_constraints, /* 401 */
        NID_proxyCertInfo,      /* 663 */
        NID_name_constraints,   /* 666 */
        NID_policy_mappings,    /* 747 */
        NID_inhibit_any_policy  /* 748 */
    };

    int ex_nid = OBJ_obj2nid(X509_EXTENSION_get_object(ex));

    if (ex_nid == NID_undef)
        return 0;

    if (OBJ_bsearch_nid(&ex_nid, supported_nids, OSSL_NELEM(supported_nids)))
        return 1;
    return 0;
}

/* Returns 1 on success, 0 if x is invalid, -1 on (internal) error. */
static int setup_dp(const X509 *x, DIST_POINT *dp)
{
    const X509_NAME *iname = NULL;
    int i;

    if (dp->distpoint == NULL && sk_GENERAL_NAME_num(dp->CRLissuer) <= 0) {
        ERR_raise(ERR_LIB_X509, X509_R_INVALID_DISTPOINT);
        return 0;
    }
    if (dp->reasons != NULL) {
        if (dp->reasons->length > 0)
            dp->dp_reasons = dp->reasons->data[0];
        if (dp->reasons->length > 1)
            dp->dp_reasons |= (dp->reasons->data[1] << 8);
        dp->dp_reasons &= CRLDP_ALL_REASONS;
    } else {
        dp->dp_reasons = CRLDP_ALL_REASONS;
    }
    if (dp->distpoint == NULL || dp->distpoint->type != 1)
        return 1;

    /* Handle name fragment given by nameRelativeToCRLIssuer */
    /*
     * Note that the below way of determining iname is not really compliant
     * with https://tools.ietf.org/html/rfc5280#section-4.2.1.13
     * According to it, sk_GENERAL_NAME_num(dp->CRLissuer) MUST be <= 1
     * and any CRLissuer could be of type different to GEN_DIRNAME.
     */
    for (i = 0; i < sk_GENERAL_NAME_num(dp->CRLissuer); i++) {
        GENERAL_NAME *gen = sk_GENERAL_NAME_value(dp->CRLissuer, i);

        if (gen->type == GEN_DIRNAME) {
            iname = gen->d.directoryName;
            break;
        }
    }
    if (iname == NULL)
        iname = X509_get_issuer_name(x);
    return DIST_POINT_set_dpname(dp->distpoint, iname) ? 1 : -1;
}

/* Return 1 on success, 0 if x is invalid, -1 on (internal) error. */
static int setup_crldp(X509 *x)
{
    int i;

    x->crldp = X509_get_ext_d2i(x, NID_crl_distribution_points, &i, NULL);
    if (x->crldp == NULL && i != -1)
        return 0;

    for (i = 0; i < sk_DIST_POINT_num(x->crldp); i++) {
        int res = setup_dp(x, sk_DIST_POINT_value(x->crldp, i));

        if (res < 1)
            return res;
    }
    return 1;
}

/* Check that issuer public key algorithm matches subject signature algorithm */
static int check_sig_alg_match(const EVP_PKEY *issuer_key, const X509 *subject)
{
    int subj_sig_nid;

    if (issuer_key == NULL)
        return X509_V_ERR_NO_ISSUER_PUBLIC_KEY;
    if (OBJ_find_sigid_algs(OBJ_obj2nid(subject->cert_info.signature.algorithm),
                            NULL, &subj_sig_nid) == 0)
        return X509_V_ERR_UNSUPPORTED_SIGNATURE_ALGORITHM;
    if (EVP_PKEY_is_a(issuer_key, OBJ_nid2sn(subj_sig_nid))
        || (EVP_PKEY_is_a(issuer_key, "RSA") && subj_sig_nid == NID_rsassaPss))
        return X509_V_OK;
    return X509_V_ERR_SIGNATURE_ALGORITHM_MISMATCH;
}

#define V1_ROOT (EXFLAG_V1 | EXFLAG_SS)
#define ku_reject(x, usage) \
    (((x)->ex_flags & EXFLAG_KUSAGE) != 0 && ((x)->ex_kusage & (usage)) == 0)
#define xku_reject(x, usage) \
    (((x)->ex_flags & EXFLAG_XKUSAGE) != 0 && ((x)->ex_xkusage & (usage)) == 0)
#define ns_reject(x, usage) \
    (((x)->ex_flags & EXFLAG_NSCERT) != 0 && ((x)->ex_nscert & (usage)) == 0)

/*
 * Cache info on various X.509v3 extensions and further derived information,
 * e.g., if cert 'x' is self-issued, in x->ex_flags and other internal fields.
 * x->sha1_hash is filled in, or else EXFLAG_NO_FINGERPRINT is set in x->flags.
 * X509_SIG_INFO_VALID is set in x->flags if x->siginf was filled successfully.
 * Set EXFLAG_INVALID and return 0 in case the certificate is invalid.
 */
int ossl_x509v3_cache_extensions(X509 *x)
{
    BASIC_CONSTRAINTS *bs;
    PROXY_CERT_INFO_EXTENSION *pci;
    ASN1_BIT_STRING *usage;
    ASN1_BIT_STRING *ns;
    EXTENDED_KEY_USAGE *extusage;
    int i;
    int res;

#ifdef tsan_ld_acq
    /* Fast lock-free check, see end of the function for details. */
    if (tsan_ld_acq((TSAN_QUALIFIER int *)&x->ex_cached))
        return (x->ex_flags & EXFLAG_INVALID) == 0;
#endif

    if (!CRYPTO_THREAD_write_lock(x->lock))
        return 0;
    if ((x->ex_flags & EXFLAG_SET) != 0) { /* Cert has already been processed */
        CRYPTO_THREAD_unlock(x->lock);
        return (x->ex_flags & EXFLAG_INVALID) == 0;
    }

    ERR_set_mark();

    /* Cache the SHA1 digest of the cert */
    if (!X509_digest(x, EVP_sha1(), x->sha1_hash, NULL))
        x->ex_flags |= EXFLAG_NO_FINGERPRINT;

    /* V1 should mean no extensions ... */
    if (X509_get_version(x) == X509_VERSION_1)
        x->ex_flags |= EXFLAG_V1;

    /* Handle basic constraints */
    x->ex_pathlen = -1;
    if ((bs = X509_get_ext_d2i(x, NID_basic_constraints, &i, NULL)) != NULL) {
        if (bs->ca)
            x->ex_flags |= EXFLAG_CA;
        if (bs->pathlen != NULL) {
            /*
             * The error case !bs->ca is checked by check_chain()
             * in case ctx->param->flags & X509_V_FLAG_X509_STRICT
             */
            if (bs->pathlen->type == V_ASN1_NEG_INTEGER) {
                ERR_raise(ERR_LIB_X509V3, X509V3_R_NEGATIVE_PATHLEN);
                x->ex_flags |= EXFLAG_INVALID;
            } else {
                x->ex_pathlen = ASN1_INTEGER_get(bs->pathlen);
            }
        }
        BASIC_CONSTRAINTS_free(bs);
        x->ex_flags |= EXFLAG_BCONS;
    } else if (i != -1) {
        x->ex_flags |= EXFLAG_INVALID;
    }

    /* Handle proxy certificates */
    if ((pci = X509_get_ext_d2i(x, NID_proxyCertInfo, &i, NULL)) != NULL) {
        if ((x->ex_flags & EXFLAG_CA) != 0
            || X509_get_ext_by_NID(x, NID_subject_alt_name, -1) >= 0
            || X509_get_ext_by_NID(x, NID_issuer_alt_name, -1) >= 0) {
            x->ex_flags |= EXFLAG_INVALID;
        }
        if (pci->pcPathLengthConstraint != NULL)
            x->ex_pcpathlen = ASN1_INTEGER_get(pci->pcPathLengthConstraint);
        else
            x->ex_pcpathlen = -1;
        PROXY_CERT_INFO_EXTENSION_free(pci);
        x->ex_flags |= EXFLAG_PROXY;
    } else if (i != -1) {
        x->ex_flags |= EXFLAG_INVALID;
    }

    /* Handle (basic) key usage */
    if ((usage = X509_get_ext_d2i(x, NID_key_usage, &i, NULL)) != NULL) {
        x->ex_kusage = 0;
        if (usage->length > 0) {
            x->ex_kusage = usage->data[0];
            if (usage->length > 1)
                x->ex_kusage |= usage->data[1] << 8;
        }
        x->ex_flags |= EXFLAG_KUSAGE;
        ASN1_BIT_STRING_free(usage);
        /* Check for empty key usage according to RFC 5280 section 4.2.1.3 */
        if (x->ex_kusage == 0) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_EMPTY_KEY_USAGE);
            x->ex_flags |= EXFLAG_INVALID;
        }
    } else if (i != -1) {
        x->ex_flags |= EXFLAG_INVALID;
    }

    /* Handle extended key usage */
    x->ex_xkusage = 0;
    if ((extusage = X509_get_ext_d2i(x, NID_ext_key_usage, &i, NULL)) != NULL) {
        x->ex_flags |= EXFLAG_XKUSAGE;
        for (i = 0; i < sk_ASN1_OBJECT_num(extusage); i++) {
            switch (OBJ_obj2nid(sk_ASN1_OBJECT_value(extusage, i))) {
            case NID_server_auth:
                x->ex_xkusage |= XKU_SSL_SERVER;
                break;
            case NID_client_auth:
                x->ex_xkusage |= XKU_SSL_CLIENT;
                break;
            case NID_email_protect:
                x->ex_xkusage |= XKU_SMIME;
                break;
            case NID_code_sign:
                x->ex_xkusage |= XKU_CODE_SIGN;
                break;
            case NID_ms_sgc:
            case NID_ns_sgc:
                x->ex_xkusage |= XKU_SGC;
                break;
            case NID_OCSP_sign:
                x->ex_xkusage |= XKU_OCSP_SIGN;
                break;
            case NID_time_stamp:
                x->ex_xkusage |= XKU_TIMESTAMP;
                break;
            case NID_dvcs:
                x->ex_xkusage |= XKU_DVCS;
                break;
            case NID_anyExtendedKeyUsage:
                x->ex_xkusage |= XKU_ANYEKU;
                break;
            default:
                /* Ignore unknown extended key usage */
                break;
            }
        }
        sk_ASN1_OBJECT_pop_free(extusage, ASN1_OBJECT_free);
    } else if (i != -1) {
        x->ex_flags |= EXFLAG_INVALID;
    }

    /* Handle legacy Netscape extension */
    if ((ns = X509_get_ext_d2i(x, NID_netscape_cert_type, &i, NULL)) != NULL) {
        if (ns->length > 0)
            x->ex_nscert = ns->data[0];
        else
            x->ex_nscert = 0;
        x->ex_flags |= EXFLAG_NSCERT;
        ASN1_BIT_STRING_free(ns);
    } else if (i != -1) {
        x->ex_flags |= EXFLAG_INVALID;
    }

    /* Handle subject key identifier and issuer/authority key identifier */
    x->skid = X509_get_ext_d2i(x, NID_subject_key_identifier, &i, NULL);
    if (x->skid == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;

    x->akid = X509_get_ext_d2i(x, NID_authority_key_identifier, &i, NULL);
    if (x->akid == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;

    /* Check if subject name matches issuer */
    if (X509_NAME_cmp(X509_get_subject_name(x), X509_get_issuer_name(x)) == 0) {
        x->ex_flags |= EXFLAG_SI; /* Cert is self-issued */
        if (X509_check_akid(x, x->akid) == X509_V_OK /* SKID matches AKID */
                /* .. and the signature alg matches the PUBKEY alg: */
                && check_sig_alg_match(X509_get0_pubkey(x), x) == X509_V_OK)
            x->ex_flags |= EXFLAG_SS; /* indicate self-signed */
        /* This is very related to ossl_x509_likely_issued(x, x) == X509_V_OK */
    }

    /* Handle subject alternative names and various other extensions */
    x->altname = X509_get_ext_d2i(x, NID_subject_alt_name, &i, NULL);
    if (x->altname == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;
    x->nc = X509_get_ext_d2i(x, NID_name_constraints, &i, NULL);
    if (x->nc == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;

    /* Handle CRL distribution point entries */
    res = setup_crldp(x);
    if (res == 0)
        x->ex_flags |= EXFLAG_INVALID;

#ifndef OPENSSL_NO_RFC3779
    x->rfc3779_addr = X509_get_ext_d2i(x, NID_sbgp_ipAddrBlock, &i, NULL);
    if (x->rfc3779_addr == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;
    x->rfc3779_asid = X509_get_ext_d2i(x, NID_sbgp_autonomousSysNum, &i, NULL);
    if (x->rfc3779_asid == NULL && i != -1)
        x->ex_flags |= EXFLAG_INVALID;
#endif
    for (i = 0; i < X509_get_ext_count(x); i++) {
        X509_EXTENSION *ex = X509_get_ext(x, i);
        int nid = OBJ_obj2nid(X509_EXTENSION_get_object(ex));

        if (nid == NID_freshest_crl)
            x->ex_flags |= EXFLAG_FRESHEST;
        if (!X509_EXTENSION_get_critical(ex))
            continue;
        if (!X509_supported_extension(ex)) {
            x->ex_flags |= EXFLAG_CRITICAL;
            break;
        }
        switch (nid) {
        case NID_basic_constraints:
            x->ex_flags |= EXFLAG_BCONS_CRITICAL;
            break;
        case NID_authority_key_identifier:
            x->ex_flags |= EXFLAG_AKID_CRITICAL;
            break;
        case NID_subject_key_identifier:
            x->ex_flags |= EXFLAG_SKID_CRITICAL;
            break;
        case NID_subject_alt_name:
            x->ex_flags |= EXFLAG_SAN_CRITICAL;
            break;
        default:
            break;
        }
    }

    /* Set x->siginf, ignoring errors due to unsupported algos */
    (void)ossl_x509_init_sig_info(x);

    x->ex_flags |= EXFLAG_SET; /* Indicate that cert has been processed */
#ifdef tsan_st_rel
    tsan_st_rel((TSAN_QUALIFIER int *)&x->ex_cached, 1);
    /*
     * Above store triggers fast lock-free check in the beginning of the
     * function. But one has to ensure that the structure is "stable", i.e.
     * all stores are visible on all processors. Hence the release fence.
     */
#endif
    ERR_pop_to_mark();

    if ((x->ex_flags & EXFLAG_INVALID) == 0) {
        CRYPTO_THREAD_unlock(x->lock);
        return 1;
    }
    CRYPTO_THREAD_unlock(x->lock);
    ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_CERTIFICATE);
    return 0;
}

/*-
 * CA checks common to all purposes
 * return codes:
 * 0 not a CA
 * 1 is a CA
 * 2 Only possible in older versions of openSSL when basicConstraints are absent
 *   new versions will not return this value. May be a CA
 * 3 basicConstraints absent but self-signed V1.
 * 4 basicConstraints absent but keyUsage present and keyCertSign asserted.
 * 5 Netscape specific CA Flags present
 */

static int check_ca(const X509 *x)
{
    /* keyUsage if present should allow cert signing */
    if (ku_reject(x, KU_KEY_CERT_SIGN))
        return 0;
    if ((x->ex_flags & EXFLAG_BCONS) != 0) {
        /* If basicConstraints says not a CA then say so */
        return (x->ex_flags & EXFLAG_CA) != 0;
    } else {
        /* We support V1 roots for...  uh, I don't really know why. */
        if ((x->ex_flags & V1_ROOT) == V1_ROOT)
            return 3;
        /*
         * If key usage present it must have certSign so tolerate it
         */
        else if ((x->ex_flags & EXFLAG_KUSAGE) != 0)
            return 4;
        /* Older certificates could have Netscape-specific CA types */
        else if ((x->ex_flags & EXFLAG_NSCERT) != 0
                 && (x->ex_nscert & NS_ANY_CA) != 0)
            return 5;
        /* Can this still be regarded a CA certificate?  I doubt it. */
        return 0;
    }
}

void X509_set_proxy_flag(X509 *x)
{
    if (CRYPTO_THREAD_write_lock(x->lock)) {
        x->ex_flags |= EXFLAG_PROXY;
        CRYPTO_THREAD_unlock(x->lock);
    }
}

void X509_set_proxy_pathlen(X509 *x, long l)
{
    x->ex_pcpathlen = l;
}

int X509_check_ca(X509 *x)
{
    /* Note 0 normally means "not a CA" - but in this case means error. */
    if (!ossl_x509v3_cache_extensions(x))
        return 0;

    return check_ca(x);
}

/* Check SSL CA: common checks for SSL client and server. */
static int check_ssl_ca(const X509 *x)
{
    int ca_ret = check_ca(x);

    if (ca_ret == 0)
        return 0;
    /* Check nsCertType if present */
    return ca_ret != 5 || (x->ex_nscert & NS_SSL_CA) != 0;
}

static int check_purpose_ssl_client(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf)
{
    if (xku_reject(x, XKU_SSL_CLIENT))
        return 0;
    if (non_leaf)
        return check_ssl_ca(x);
    /* We need to do digital signatures or key agreement */
    if (ku_reject(x, KU_DIGITAL_SIGNATURE | KU_KEY_AGREEMENT))
        return 0;
    /* nsCertType if present should allow SSL client use */
    if (ns_reject(x, NS_SSL_CLIENT))
        return 0;
    return 1;
}

/*
 * Key usage needed for TLS/SSL server: digital signature, encipherment or
 * key agreement. The ssl code can check this more thoroughly for individual
 * key types.
 */
#define KU_TLS \
    KU_DIGITAL_SIGNATURE | KU_KEY_ENCIPHERMENT | KU_KEY_AGREEMENT

static int check_purpose_ssl_server(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf)
{
    if (xku_reject(x, XKU_SSL_SERVER | XKU_SGC))
        return 0;
    if (non_leaf)
        return check_ssl_ca(x);

    if (ns_reject(x, NS_SSL_SERVER))
        return 0;
    if (ku_reject(x, KU_TLS))
        return 0;

    return 1;

}

static int check_purpose_ns_ssl_server(const X509_PURPOSE *xp, const X509 *x,
                                       int non_leaf)
{
    int ret = check_purpose_ssl_server(xp, x, non_leaf);

    if (!ret || non_leaf)
        return ret;
    /* We need to encipher or Netscape complains */
    return ku_reject(x, KU_KEY_ENCIPHERMENT) ? 0 : ret;
}

/* common S/MIME checks */
static int purpose_smime(const X509 *x, int non_leaf)
{
    if (xku_reject(x, XKU_SMIME))
        return 0;
    if (non_leaf) {
        int ca_ret = check_ca(x);

        if (ca_ret == 0)
            return 0;
        /* Check nsCertType if present */
        if (ca_ret != 5 || (x->ex_nscert & NS_SMIME_CA) != 0)
            return ca_ret;
        else
            return 0;
    }
    if ((x->ex_flags & EXFLAG_NSCERT) != 0) {
        if ((x->ex_nscert & NS_SMIME) != 0)
            return 1;
        /* Workaround for some buggy certificates */
        return (x->ex_nscert & NS_SSL_CLIENT) != 0 ? 2 : 0;
    }
    return 1;
}

static int check_purpose_smime_sign(const X509_PURPOSE *xp, const X509 *x,
                                    int non_leaf)
{
    int ret = purpose_smime(x, non_leaf);

    if (!ret || non_leaf)
        return ret;
    return ku_reject(x, KU_DIGITAL_SIGNATURE | KU_NON_REPUDIATION) ? 0 : ret;
}

static int check_purpose_smime_encrypt(const X509_PURPOSE *xp, const X509 *x,
                                       int non_leaf)
{
    int ret = purpose_smime(x, non_leaf);

    if (!ret || non_leaf)
        return ret;
    return ku_reject(x, KU_KEY_ENCIPHERMENT) ? 0 : ret;
}

static int check_purpose_crl_sign(const X509_PURPOSE *xp, const X509 *x,
                                  int non_leaf)
{
    if (non_leaf) {
        int ca_ret = check_ca(x);

        return ca_ret == 2 ? 0 : ca_ret;
    }
    return !ku_reject(x, KU_CRL_SIGN);
}

/*
 * OCSP helper: this is *not* a full OCSP check. It just checks that each CA
 * is valid. Additional checks must be made on the chain.
 */
static int check_purpose_ocsp_helper(const X509_PURPOSE *xp, const X509 *x,
                                     int non_leaf)
{
    /*
     * Must be a valid CA.  Should we really support the "I don't know" value
     * (2)?
     */
    if (non_leaf)
        return check_ca(x);
    /* Leaf certificate is checked in OCSP_verify() */
    return 1;
}

static int check_purpose_timestamp_sign(const X509_PURPOSE *xp, const X509 *x,
                                        int non_leaf)
{
    int i_ext;

    /*
     * If non_leaf is true we must check if this is a valid CA certificate.
     * The extra requirements by the CA/Browser Forum are not checked.
     */
    if (non_leaf)
        return check_ca(x);

    /*
     * Key Usage is checked according to RFC 5280 and
     * Extended Key Usage attributes is checked according to RFC 3161.
     * The extra (and somewhat conflicting) CA/Browser Forum
     * Baseline Requirements for the Issuance and Management of
     * Publicly‐Trusted Code Signing Certificates, Version 3.0.0,
     * Section 7.1.2.3: Code signing and Timestamp Certificate are not checked.
     */
    /*
     * Check the optional key usage field:
     * if Key Usage is present, it must be one of digitalSignature
     * and/or nonRepudiation (other values are not consistent and shall
     * be rejected).
     */
    if ((x->ex_flags & EXFLAG_KUSAGE) != 0
        && ((x->ex_kusage & ~(KU_NON_REPUDIATION | KU_DIGITAL_SIGNATURE)) ||
            !(x->ex_kusage & (KU_NON_REPUDIATION | KU_DIGITAL_SIGNATURE))))
        return 0;

    /* Only timestamp key usage is permitted and it's required. */
    if ((x->ex_flags & EXFLAG_XKUSAGE) == 0 || x->ex_xkusage != XKU_TIMESTAMP)
        return 0;

    /* Extended Key Usage MUST be critical */
    i_ext = X509_get_ext_by_NID(x, NID_ext_key_usage, -1);
    if (i_ext >= 0
            && !X509_EXTENSION_get_critical(X509_get_ext((X509 *)x, i_ext)))
        return 0;
    return 1;
}

static int check_purpose_code_sign(const X509_PURPOSE *xp, const X509 *x,
                                   int non_leaf)
{
    int i_ext;

    /*
     * If non_leaf is true we must check if this is a valid CA certificate.
     * The extra requirements by the CA/Browser Forum are not checked.
     */
    if (non_leaf)
        return check_ca(x);

    /*
     * Check the key usage and extended key usage fields:
     *
     * Reference: CA/Browser Forum,
     * Baseline Requirements for the Issuance and Management of
     * Publicly‐Trusted Code Signing Certificates, Version 3.0.0,
     * Section 7.1.2.3: Code signing and Timestamp Certificate
     *
     * Checking covers Key Usage and Extended Key Usage attributes.
     * The certificatePolicies, cRLDistributionPoints (CDP), and
     * authorityInformationAccess (AIA) extensions are so far not checked.
     */
    /* Key Usage */
    if ((x->ex_flags & EXFLAG_KUSAGE) == 0)
        return 0;
    if ((x->ex_kusage & KU_DIGITAL_SIGNATURE) == 0)
        return 0;
    if ((x->ex_kusage & (KU_KEY_CERT_SIGN | KU_CRL_SIGN)) != 0)
        return 0;

    /* Key Usage MUST be critical */
    i_ext = X509_get_ext_by_NID(x, NID_key_usage, -1);
    if (i_ext < 0)
        return 0;
    if (i_ext >= 0) {
        X509_EXTENSION *ext = X509_get_ext((X509 *)x, i_ext);
        if (!X509_EXTENSION_get_critical(ext))
            return 0;
    }

    /* Extended Key Usage */
    if ((x->ex_flags & EXFLAG_XKUSAGE) == 0)
        return 0;
    if ((x->ex_xkusage & XKU_CODE_SIGN) == 0)
        return 0;
    if ((x->ex_xkusage & (XKU_ANYEKU | XKU_SSL_SERVER)) != 0)
        return 0;

    return 1;

}

static int no_check_purpose(const X509_PURPOSE *xp, const X509 *x,
                            int non_leaf)
{
    return 1;
}

/*-
 * Various checks to see if one certificate potentially issued the second.
 * This can be used to prune a set of possible issuer certificates which
 * have been looked up using some simple method such as by subject name.
 * These are:
 * 1. issuer_name(subject) == subject_name(issuer)
 * 2. If akid(subject) exists, it matches the respective issuer fields.
 * 3. subject signature algorithm == issuer public key algorithm
 * 4. If key_usage(issuer) exists, it allows for signing subject.
 * Note that this does not include actually checking the signature.
 * Returns 0 for OK, or positive for reason for mismatch
 * where reason codes match those for X509_verify_cert().
 */
int X509_check_issued(X509 *issuer, X509 *subject)
{
    int ret;

    if ((ret = ossl_x509_likely_issued(issuer, subject)) != X509_V_OK)
        return ret;
    return ossl_x509_signing_allowed(issuer, subject);
}

/* do the checks 1., 2., and 3. as described above for X509_check_issued() */
int ossl_x509_likely_issued(X509 *issuer, X509 *subject)
{
    int ret;

    if (X509_NAME_cmp(X509_get_subject_name(issuer),
                      X509_get_issuer_name(subject)) != 0)
        return X509_V_ERR_SUBJECT_ISSUER_MISMATCH;

    /* set issuer->skid and subject->akid */
    if (!ossl_x509v3_cache_extensions(issuer)
            || !ossl_x509v3_cache_extensions(subject))
        return X509_V_ERR_UNSPECIFIED;

    ret = X509_check_akid(issuer, subject->akid);
    if (ret != X509_V_OK)
        return ret;

    /* Check if the subject signature alg matches the issuer's PUBKEY alg */
    return check_sig_alg_match(X509_get0_pubkey(issuer), subject);
}

/*-
 * Check if certificate I<issuer> is allowed to issue certificate I<subject>
 * according to the B<keyUsage> field of I<issuer> if present
 * depending on any proxyCertInfo extension of I<subject>.
 * Returns 0 for OK, or positive for reason for rejection
 * where reason codes match those for X509_verify_cert().
 */
int ossl_x509_signing_allowed(const X509 *issuer, const X509 *subject)
{
    if ((subject->ex_flags & EXFLAG_PROXY) != 0) {
        if (ku_reject(issuer, KU_DIGITAL_SIGNATURE))
            return X509_V_ERR_KEYUSAGE_NO_DIGITAL_SIGNATURE;
    } else if (ku_reject(issuer, KU_KEY_CERT_SIGN)) {
        return X509_V_ERR_KEYUSAGE_NO_CERTSIGN;
    }
    return X509_V_OK;
}

int X509_check_akid(const X509 *issuer, const AUTHORITY_KEYID *akid)
{
    if (akid == NULL)
        return X509_V_OK;

    /* Check key ids (if present) */
    if (akid->keyid && issuer->skid &&
        ASN1_OCTET_STRING_cmp(akid->keyid, issuer->skid))
        return X509_V_ERR_AKID_SKID_MISMATCH;
    /* Check serial number */
    if (akid->serial &&
        ASN1_INTEGER_cmp(X509_get0_serialNumber(issuer), akid->serial))
        return X509_V_ERR_AKID_ISSUER_SERIAL_MISMATCH;
    /* Check issuer name */
    if (akid->issuer) {
        /*
         * Ugh, for some peculiar reason AKID includes SEQUENCE OF
         * GeneralName. So look for a DirName. There may be more than one but
         * we only take any notice of the first.
         */
        GENERAL_NAMES *gens = akid->issuer;
        GENERAL_NAME *gen;
        X509_NAME *nm = NULL;
        int i;

        for (i = 0; i < sk_GENERAL_NAME_num(gens); i++) {
            gen = sk_GENERAL_NAME_value(gens, i);
            if (gen->type == GEN_DIRNAME) {
                nm = gen->d.dirn;
                break;
            }
        }
        if (nm != NULL && X509_NAME_cmp(nm, X509_get_issuer_name(issuer)) != 0)
            return X509_V_ERR_AKID_ISSUER_SERIAL_MISMATCH;
    }
    return X509_V_OK;
}

uint32_t X509_get_extension_flags(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    X509_check_purpose(x, -1, 0);
    return x->ex_flags;
}

uint32_t X509_get_key_usage(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return 0;
    return (x->ex_flags & EXFLAG_KUSAGE) != 0 ? x->ex_kusage : UINT32_MAX;
}

uint32_t X509_get_extended_key_usage(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return 0;
    return (x->ex_flags & EXFLAG_XKUSAGE) != 0 ? x->ex_xkusage : UINT32_MAX;
}

const ASN1_OCTET_STRING *X509_get0_subject_key_id(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return NULL;
    return x->skid;
}

const ASN1_OCTET_STRING *X509_get0_authority_key_id(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return NULL;
    return (x->akid != NULL ? x->akid->keyid : NULL);
}

const GENERAL_NAMES *X509_get0_authority_issuer(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return NULL;
    return (x->akid != NULL ? x->akid->issuer : NULL);
}

const ASN1_INTEGER *X509_get0_authority_serial(X509 *x)
{
    /* Call for side-effect of computing hash and caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1)
        return NULL;
    return (x->akid != NULL ? x->akid->serial : NULL);
}

long X509_get_pathlen(X509 *x)
{
    /* Called for side effect of caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1
            || (x->ex_flags & EXFLAG_BCONS) == 0)
        return -1;
    return x->ex_pathlen;
}

long X509_get_proxy_pathlen(X509 *x)
{
    /* Called for side effect of caching extensions */
    if (X509_check_purpose(x, -1, 0) != 1
            || (x->ex_flags & EXFLAG_PROXY) == 0)
        return -1;
    return x->ex_pcpathlen;
}
