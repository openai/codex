/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/x509.h>
#include "crypto/x509.h"
#include <openssl/x509v3.h>
#include "x509_local.h"

static int X509_REVOKED_cmp(const X509_REVOKED *const *a,
                            const X509_REVOKED *const *b);
static int setup_idp(X509_CRL *crl, ISSUING_DIST_POINT *idp);

ASN1_SEQUENCE(X509_REVOKED) = {
        ASN1_EMBED(X509_REVOKED, serialNumber, ASN1_INTEGER),
        ASN1_SIMPLE(X509_REVOKED, revocationDate, ASN1_TIME),
        ASN1_SEQUENCE_OF_OPT(X509_REVOKED, extensions, X509_EXTENSION)
} ASN1_SEQUENCE_END(X509_REVOKED)

static int def_crl_verify(X509_CRL *crl, EVP_PKEY *r);
static int def_crl_lookup(X509_CRL *crl,
                          X509_REVOKED **ret, const ASN1_INTEGER *serial,
                          const X509_NAME *issuer);

static X509_CRL_METHOD int_crl_meth = {
    0,
    0, 0,
    def_crl_lookup,
    def_crl_verify
};

static const X509_CRL_METHOD *default_crl_method = &int_crl_meth;

/*
 * The X509_CRL_INFO structure needs a bit of customisation. Since we cache
 * the original encoding the signature won't be affected by reordering of the
 * revoked field.
 */
static int crl_inf_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                      void *exarg)
{
    X509_CRL_INFO *a = (X509_CRL_INFO *)*pval;

    if (!a || !a->revoked)
        return 1;
    switch (operation) {
        /*
         * Just set cmp function here. We don't sort because that would
         * affect the output of X509_CRL_print().
         */
    case ASN1_OP_D2I_POST:
        (void)sk_X509_REVOKED_set_cmp_func(a->revoked, X509_REVOKED_cmp);
        break;
    }
    return 1;
}


ASN1_SEQUENCE_enc(X509_CRL_INFO, enc, crl_inf_cb) = {
        ASN1_OPT(X509_CRL_INFO, version, ASN1_INTEGER),
        ASN1_EMBED(X509_CRL_INFO, sig_alg, X509_ALGOR),
        ASN1_SIMPLE(X509_CRL_INFO, issuer, X509_NAME),
        ASN1_SIMPLE(X509_CRL_INFO, lastUpdate, ASN1_TIME),
        ASN1_OPT(X509_CRL_INFO, nextUpdate, ASN1_TIME),
        ASN1_SEQUENCE_OF_OPT(X509_CRL_INFO, revoked, X509_REVOKED),
        ASN1_EXP_SEQUENCE_OF_OPT(X509_CRL_INFO, extensions, X509_EXTENSION, 0)
} ASN1_SEQUENCE_END_enc(X509_CRL_INFO, X509_CRL_INFO)

/*
 * Set CRL entry issuer according to CRL certificate issuer extension. Check
 * for unhandled critical CRL entry extensions.
 */

static int crl_set_issuers(X509_CRL *crl)
{

    int i, j;
    GENERAL_NAMES *gens, *gtmp;
    STACK_OF(X509_REVOKED) *revoked;

    revoked = X509_CRL_get_REVOKED(crl);

    gens = NULL;
    for (i = 0; i < sk_X509_REVOKED_num(revoked); i++) {
        X509_REVOKED *rev = sk_X509_REVOKED_value(revoked, i);
        STACK_OF(X509_EXTENSION) *exts;
        ASN1_ENUMERATED *reason;
        X509_EXTENSION *ext;

        gtmp = X509_REVOKED_get_ext_d2i(rev,
                                        NID_certificate_issuer, &j, NULL);
        if (gtmp == NULL && j != -1) {
            crl->flags |= EXFLAG_INVALID;
            return 1;
        }

        if (gtmp != NULL) {
            if (crl->issuers == NULL) {
                crl->issuers = sk_GENERAL_NAMES_new_null();
                if (crl->issuers == NULL) {
                    GENERAL_NAMES_free(gtmp);
                    return 0;
                }
            }
            if (!sk_GENERAL_NAMES_push(crl->issuers, gtmp)) {
                GENERAL_NAMES_free(gtmp);
                return 0;
            }
            gens = gtmp;
        }
        rev->issuer = gens;

        reason = X509_REVOKED_get_ext_d2i(rev, NID_crl_reason, &j, NULL);
        if (reason == NULL && j != -1) {
            crl->flags |= EXFLAG_INVALID;
            return 1;
        }

        if (reason != NULL) {
            rev->reason = ASN1_ENUMERATED_get(reason);
            ASN1_ENUMERATED_free(reason);
        } else
            rev->reason = CRL_REASON_NONE;

        /* Check for critical CRL entry extensions */

        exts = rev->extensions;

        for (j = 0; j < sk_X509_EXTENSION_num(exts); j++) {
            ext = sk_X509_EXTENSION_value(exts, j);
            if (X509_EXTENSION_get_critical(ext)) {
                if (OBJ_obj2nid(X509_EXTENSION_get_object(ext))
                    == NID_certificate_issuer)
                    continue;
                crl->flags |= EXFLAG_CRITICAL;
                break;
            }
        }

    }

    return 1;

}

/*
 * The X509_CRL structure needs a bit of customisation. Cache some extensions
 * and hash of the whole CRL or set EXFLAG_NO_FINGERPRINT if this fails.
 */
static int crl_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                  void *exarg)
{
    X509_CRL *crl = (X509_CRL *)*pval;
    STACK_OF(X509_EXTENSION) *exts;
    X509_EXTENSION *ext;
    int idx, i;

    switch (operation) {
    case ASN1_OP_D2I_PRE:
        if (crl->meth->crl_free) {
            if (!crl->meth->crl_free(crl))
                return 0;
        }
        AUTHORITY_KEYID_free(crl->akid);
        ISSUING_DIST_POINT_free(crl->idp);
        ASN1_INTEGER_free(crl->crl_number);
        ASN1_INTEGER_free(crl->base_crl_number);
        sk_GENERAL_NAMES_pop_free(crl->issuers, GENERAL_NAMES_free);
        /* fall through */

    case ASN1_OP_NEW_POST:
        crl->idp = NULL;
        crl->akid = NULL;
        crl->flags = 0;
        crl->idp_flags = 0;
        crl->idp_reasons = CRLDP_ALL_REASONS;
        crl->meth = default_crl_method;
        crl->meth_data = NULL;
        crl->issuers = NULL;
        crl->crl_number = NULL;
        crl->base_crl_number = NULL;
        break;

    case ASN1_OP_D2I_POST:
        if (!X509_CRL_digest(crl, EVP_sha1(), crl->sha1_hash, NULL))
            crl->flags |= EXFLAG_NO_FINGERPRINT;
        crl->idp = X509_CRL_get_ext_d2i(crl,
                                        NID_issuing_distribution_point, &i,
                                        NULL);
        if (crl->idp != NULL) {
            if (!setup_idp(crl, crl->idp))
                crl->flags |= EXFLAG_INVALID;
        }
        else if (i != -1) {
            crl->flags |= EXFLAG_INVALID;
        }

        crl->akid = X509_CRL_get_ext_d2i(crl,
                                         NID_authority_key_identifier, &i,
                                         NULL);
        if (crl->akid == NULL && i != -1)
            crl->flags |= EXFLAG_INVALID;

        crl->crl_number = X509_CRL_get_ext_d2i(crl,
                                               NID_crl_number, &i, NULL);
        if (crl->crl_number == NULL && i != -1)
            crl->flags |= EXFLAG_INVALID;

        crl->base_crl_number = X509_CRL_get_ext_d2i(crl,
                                                    NID_delta_crl, &i,
                                                    NULL);
        if (crl->base_crl_number == NULL && i != -1)
            crl->flags |= EXFLAG_INVALID;
        /* Delta CRLs must have CRL number */
        if (crl->base_crl_number && !crl->crl_number)
            crl->flags |= EXFLAG_INVALID;

        /*
         * See if we have any unhandled critical CRL extensions and indicate
         * this in a flag. We only currently handle IDP so anything else
         * critical sets the flag. This code accesses the X509_CRL structure
         * directly: applications shouldn't do this.
         */

        exts = crl->crl.extensions;

        for (idx = 0; idx < sk_X509_EXTENSION_num(exts); idx++) {
            int nid;
            ext = sk_X509_EXTENSION_value(exts, idx);
            nid = OBJ_obj2nid(X509_EXTENSION_get_object(ext));
            if (nid == NID_freshest_crl)
                crl->flags |= EXFLAG_FRESHEST;
            if (X509_EXTENSION_get_critical(ext)) {
                /* We handle IDP and deltas */
                if ((nid == NID_issuing_distribution_point)
                    || (nid == NID_authority_key_identifier)
                    || (nid == NID_delta_crl))
                    continue;
                crl->flags |= EXFLAG_CRITICAL;
                break;
            }
        }

        if (!crl_set_issuers(crl))
            return 0;

        if (crl->meth->crl_init) {
            if (crl->meth->crl_init(crl) == 0)
                return 0;
        }

        crl->flags |= EXFLAG_SET;
        break;

    case ASN1_OP_FREE_POST:
        if (crl->meth != NULL && crl->meth->crl_free != NULL) {
            if (!crl->meth->crl_free(crl))
                return 0;
        }
        AUTHORITY_KEYID_free(crl->akid);
        ISSUING_DIST_POINT_free(crl->idp);
        ASN1_INTEGER_free(crl->crl_number);
        ASN1_INTEGER_free(crl->base_crl_number);
        sk_GENERAL_NAMES_pop_free(crl->issuers, GENERAL_NAMES_free);
        OPENSSL_free(crl->propq);
        break;
    case ASN1_OP_DUP_POST:
        {
            X509_CRL *old = exarg;

            if (!ossl_x509_crl_set0_libctx(crl, old->libctx, old->propq))
                return 0;
        }
        break;
    }
    return 1;
}

/* Convert IDP into a more convenient form */

static int setup_idp(X509_CRL *crl, ISSUING_DIST_POINT *idp)
{
    int idp_only = 0;

    /* Set various flags according to IDP */
    crl->idp_flags |= IDP_PRESENT;
    if (idp->onlyuser > 0) {
        idp_only++;
        crl->idp_flags |= IDP_ONLYUSER;
    }
    if (idp->onlyCA > 0) {
        idp_only++;
        crl->idp_flags |= IDP_ONLYCA;
    }
    if (idp->onlyattr > 0) {
        idp_only++;
        crl->idp_flags |= IDP_ONLYATTR;
    }

    if (idp_only > 1)
        crl->idp_flags |= IDP_INVALID;

    if (idp->indirectCRL > 0)
        crl->idp_flags |= IDP_INDIRECT;

    if (idp->onlysomereasons) {
        crl->idp_flags |= IDP_REASONS;
        if (idp->onlysomereasons->length > 0)
            crl->idp_reasons = idp->onlysomereasons->data[0];
        if (idp->onlysomereasons->length > 1)
            crl->idp_reasons |= (idp->onlysomereasons->data[1] << 8);
        crl->idp_reasons &= CRLDP_ALL_REASONS;
    }

    return DIST_POINT_set_dpname(idp->distpoint, X509_CRL_get_issuer(crl));
}

ASN1_SEQUENCE_ref(X509_CRL, crl_cb) = {
        ASN1_EMBED(X509_CRL, crl, X509_CRL_INFO),
        ASN1_EMBED(X509_CRL, sig_alg, X509_ALGOR),
        ASN1_EMBED(X509_CRL, signature, ASN1_BIT_STRING)
} ASN1_SEQUENCE_END_ref(X509_CRL, X509_CRL)

IMPLEMENT_ASN1_FUNCTIONS(X509_REVOKED)

IMPLEMENT_ASN1_DUP_FUNCTION(X509_REVOKED)

IMPLEMENT_ASN1_FUNCTIONS(X509_CRL_INFO)

IMPLEMENT_ASN1_FUNCTIONS(X509_CRL)

IMPLEMENT_ASN1_DUP_FUNCTION(X509_CRL)

static int X509_REVOKED_cmp(const X509_REVOKED *const *a,
                            const X509_REVOKED *const *b)
{
    return (ASN1_STRING_cmp((ASN1_STRING *)&(*a)->serialNumber,
                            (ASN1_STRING *)&(*b)->serialNumber));
}

X509_CRL *X509_CRL_new_ex(OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_CRL *crl = NULL;

    crl = (X509_CRL *)ASN1_item_new((X509_CRL_it()));
    if (!ossl_x509_crl_set0_libctx(crl, libctx, propq)) {
        X509_CRL_free(crl);
        crl = NULL;
    }
    return crl;
}

int X509_CRL_add0_revoked(X509_CRL *crl, X509_REVOKED *rev)
{
    X509_CRL_INFO *inf;

    inf = &crl->crl;
    if (inf->revoked == NULL)
        inf->revoked = sk_X509_REVOKED_new(X509_REVOKED_cmp);
    if (inf->revoked == NULL || !sk_X509_REVOKED_push(inf->revoked, rev)) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_CRYPTO_LIB);
        return 0;
    }
    inf->enc.modified = 1;
    return 1;
}

int X509_CRL_verify(X509_CRL *crl, EVP_PKEY *r)
{
    if (crl->meth->crl_verify)
        return crl->meth->crl_verify(crl, r);
    return 0;
}

int X509_CRL_get0_by_serial(X509_CRL *crl,
                            X509_REVOKED **ret, const ASN1_INTEGER *serial)
{
    if (crl->meth->crl_lookup)
        return crl->meth->crl_lookup(crl, ret, serial, NULL);
    return 0;
}

int X509_CRL_get0_by_cert(X509_CRL *crl, X509_REVOKED **ret, X509 *x)
{
    if (crl->meth->crl_lookup)
        return crl->meth->crl_lookup(crl, ret,
                                     X509_get0_serialNumber(x),
                                     X509_get_issuer_name(x));
    return 0;
}

static int def_crl_verify(X509_CRL *crl, EVP_PKEY *r)
{
    return ASN1_item_verify_ex(ASN1_ITEM_rptr(X509_CRL_INFO),
                               &crl->sig_alg, &crl->signature, &crl->crl, NULL,
                               r, crl->libctx, crl->propq);
}

static int crl_revoked_issuer_match(X509_CRL *crl, const X509_NAME *nm,
                                    X509_REVOKED *rev)
{
    int i;

    if (!rev->issuer) {
        if (!nm)
            return 1;
        if (!X509_NAME_cmp(nm, X509_CRL_get_issuer(crl)))
            return 1;
        return 0;
    }

    if (!nm)
        nm = X509_CRL_get_issuer(crl);

    for (i = 0; i < sk_GENERAL_NAME_num(rev->issuer); i++) {
        GENERAL_NAME *gen = sk_GENERAL_NAME_value(rev->issuer, i);
        if (gen->type != GEN_DIRNAME)
            continue;
        if (!X509_NAME_cmp(nm, gen->d.directoryName))
            return 1;
    }
    return 0;

}

static int def_crl_lookup(X509_CRL *crl,
                          X509_REVOKED **ret, const ASN1_INTEGER *serial,
                          const X509_NAME *issuer)
{
    X509_REVOKED rtmp, *rev;
    int idx, num;

    if (crl->crl.revoked == NULL)
        return 0;

    /*
     * Sort revoked into serial number order if not already sorted. Do this
     * under a lock to avoid race condition.
     */
    if (!sk_X509_REVOKED_is_sorted(crl->crl.revoked)) {
        if (!CRYPTO_THREAD_write_lock(crl->lock))
            return 0;
        sk_X509_REVOKED_sort(crl->crl.revoked);
        CRYPTO_THREAD_unlock(crl->lock);
    }
    rtmp.serialNumber = *serial;
    idx = sk_X509_REVOKED_find(crl->crl.revoked, &rtmp);
    if (idx < 0)
        return 0;
    /* Need to look for matching name */
    for (num = sk_X509_REVOKED_num(crl->crl.revoked); idx < num; idx++) {
        rev = sk_X509_REVOKED_value(crl->crl.revoked, idx);
        if (ASN1_INTEGER_cmp(&rev->serialNumber, serial))
            return 0;
        if (crl_revoked_issuer_match(crl, issuer, rev)) {
            if (ret)
                *ret = rev;
            if (rev->reason == CRL_REASON_REMOVE_FROM_CRL)
                return 2;
            return 1;
        }
    }
    return 0;
}

void X509_CRL_set_default_method(const X509_CRL_METHOD *meth)
{
    if (meth == NULL)
        default_crl_method = &int_crl_meth;
    else
        default_crl_method = meth;
}

X509_CRL_METHOD *X509_CRL_METHOD_new(int (*crl_init) (X509_CRL *crl),
                                     int (*crl_free) (X509_CRL *crl),
                                     int (*crl_lookup) (X509_CRL *crl,
                                                        X509_REVOKED **ret,
                                                        const ASN1_INTEGER *ser,
                                                        const X509_NAME *issuer),
                                     int (*crl_verify) (X509_CRL *crl,
                                                        EVP_PKEY *pk))
{
    X509_CRL_METHOD *m = OPENSSL_malloc(sizeof(*m));

    if (m == NULL)
        return NULL;
    m->crl_init = crl_init;
    m->crl_free = crl_free;
    m->crl_lookup = crl_lookup;
    m->crl_verify = crl_verify;
    m->flags = X509_CRL_METHOD_DYNAMIC;
    return m;
}

void X509_CRL_METHOD_free(X509_CRL_METHOD *m)
{
    if (m == NULL || !(m->flags & X509_CRL_METHOD_DYNAMIC))
        return;
    OPENSSL_free(m);
}

void X509_CRL_set_meth_data(X509_CRL *crl, void *dat)
{
    crl->meth_data = dat;
}

void *X509_CRL_get_meth_data(X509_CRL *crl)
{
    return crl->meth_data;
}

int ossl_x509_crl_set0_libctx(X509_CRL *x, OSSL_LIB_CTX *libctx,
                              const char *propq)
{
    if (x != NULL) {
        x->libctx = libctx;
        OPENSSL_free(x->propq);
        x->propq = NULL;
        if (propq != NULL) {
            x->propq = OPENSSL_strdup(propq);
            if (x->propq == NULL)
                return 0;
        }
    }
    return 1;
}
