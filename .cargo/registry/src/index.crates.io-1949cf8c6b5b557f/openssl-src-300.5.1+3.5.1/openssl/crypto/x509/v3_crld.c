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
#include <openssl/conf.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/x509v3.h>

#include "crypto/x509.h"
#include "ext_dat.h"
#include "x509_local.h"

static void *v2i_crld(const X509V3_EXT_METHOD *method,
                      X509V3_CTX *ctx, STACK_OF(CONF_VALUE) *nval);
static int i2r_crldp(const X509V3_EXT_METHOD *method, void *pcrldp, BIO *out,
                     int indent);

const X509V3_EXT_METHOD ossl_v3_crld = {
    NID_crl_distribution_points, 0, ASN1_ITEM_ref(CRL_DIST_POINTS),
    0, 0, 0, 0,
    0, 0,
    0,
    v2i_crld,
    i2r_crldp, 0,
    NULL
};

const X509V3_EXT_METHOD ossl_v3_freshest_crl = {
    NID_freshest_crl, 0, ASN1_ITEM_ref(CRL_DIST_POINTS),
    0, 0, 0, 0,
    0, 0,
    0,
    v2i_crld,
    i2r_crldp, 0,
    NULL
};

static STACK_OF(GENERAL_NAME) *gnames_from_sectname(X509V3_CTX *ctx,
                                                    char *sect)
{
    STACK_OF(CONF_VALUE) *gnsect;
    STACK_OF(GENERAL_NAME) *gens;
    if (*sect == '@')
        gnsect = X509V3_get_section(ctx, sect + 1);
    else
        gnsect = X509V3_parse_list(sect);
    if (!gnsect) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_SECTION_NOT_FOUND);
        return NULL;
    }
    gens = v2i_GENERAL_NAMES(NULL, ctx, gnsect);
    if (*sect == '@')
        X509V3_section_free(ctx, gnsect);
    else
        sk_CONF_VALUE_pop_free(gnsect, X509V3_conf_free);
    return gens;
}

static int set_dist_point_name(DIST_POINT_NAME **pdp, X509V3_CTX *ctx,
                               CONF_VALUE *cnf)
{
    STACK_OF(GENERAL_NAME) *fnm = NULL;
    STACK_OF(X509_NAME_ENTRY) *rnm = NULL;

    if (cnf->value == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_MISSING_VALUE);
        goto err;
    }

    if (HAS_PREFIX(cnf->name, "fullname")) {
        fnm = gnames_from_sectname(ctx, cnf->value);
        if (!fnm)
            goto err;
    } else if (strcmp(cnf->name, "relativename") == 0) {
        int ret;
        STACK_OF(CONF_VALUE) *dnsect;
        X509_NAME *nm;
        nm = X509_NAME_new();
        if (nm == NULL)
            return -1;
        dnsect = X509V3_get_section(ctx, cnf->value);
        if (!dnsect) {
            X509_NAME_free(nm);
            ERR_raise(ERR_LIB_X509V3, X509V3_R_SECTION_NOT_FOUND);
            return -1;
        }
        ret = X509V3_NAME_from_section(nm, dnsect, MBSTRING_ASC);
        X509V3_section_free(ctx, dnsect);
        rnm = nm->entries;
        nm->entries = NULL;
        X509_NAME_free(nm);
        if (!ret || sk_X509_NAME_ENTRY_num(rnm) <= 0)
            goto err;
        /*
         * Since its a name fragment can't have more than one RDNSequence
         */
        if (sk_X509_NAME_ENTRY_value(rnm,
                                     sk_X509_NAME_ENTRY_num(rnm) - 1)->set) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_MULTIPLE_RDNS);
            goto err;
        }
    } else
        return 0;

    if (*pdp) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_DISTPOINT_ALREADY_SET);
        goto err;
    }

    *pdp = DIST_POINT_NAME_new();
    if (*pdp == NULL)
        goto err;
    if (fnm) {
        (*pdp)->type = 0;
        (*pdp)->name.fullname = fnm;
    } else {
        (*pdp)->type = 1;
        (*pdp)->name.relativename = rnm;
    }

    return 1;

 err:
    sk_GENERAL_NAME_pop_free(fnm, GENERAL_NAME_free);
    sk_X509_NAME_ENTRY_pop_free(rnm, X509_NAME_ENTRY_free);
    return -1;
}

static const BIT_STRING_BITNAME reason_flags[] = {
    {0, "Unused", "unused"},
    {1, "Key Compromise", "keyCompromise"},
    {2, "CA Compromise", "CACompromise"},
    {3, "Affiliation Changed", "affiliationChanged"},
    {4, "Superseded", "superseded"},
    {5, "Cessation Of Operation", "cessationOfOperation"},
    {6, "Certificate Hold", "certificateHold"},
    {7, "Privilege Withdrawn", "privilegeWithdrawn"},
    {8, "AA Compromise", "AACompromise"},
    {-1, NULL, NULL}
};

static int set_reasons(ASN1_BIT_STRING **preas, char *value)
{
    STACK_OF(CONF_VALUE) *rsk = NULL;
    const BIT_STRING_BITNAME *pbn;
    const char *bnam;
    int i, ret = 0;
    rsk = X509V3_parse_list(value);
    if (rsk == NULL)
        return 0;
    if (*preas != NULL)
        goto err;
    for (i = 0; i < sk_CONF_VALUE_num(rsk); i++) {
        bnam = sk_CONF_VALUE_value(rsk, i)->name;
        if (*preas == NULL) {
            *preas = ASN1_BIT_STRING_new();
            if (*preas == NULL)
                goto err;
        }
        for (pbn = reason_flags; pbn->lname; pbn++) {
            if (strcmp(pbn->sname, bnam) == 0) {
                if (!ASN1_BIT_STRING_set_bit(*preas, pbn->bitnum, 1))
                    goto err;
                break;
            }
        }
        if (pbn->lname == NULL)
            goto err;
    }
    ret = 1;

 err:
    sk_CONF_VALUE_pop_free(rsk, X509V3_conf_free);
    return ret;
}

static int print_reasons(BIO *out, const char *rname,
                         ASN1_BIT_STRING *rflags, int indent)
{
    int first = 1;
    const BIT_STRING_BITNAME *pbn;
    BIO_printf(out, "%*s%s:\n%*s", indent, "", rname, indent + 2, "");
    for (pbn = reason_flags; pbn->lname; pbn++) {
        if (ASN1_BIT_STRING_get_bit(rflags, pbn->bitnum)) {
            if (first)
                first = 0;
            else
                BIO_puts(out, ", ");
            BIO_puts(out, pbn->lname);
        }
    }
    if (first)
        BIO_puts(out, "<EMPTY>\n");
    else
        BIO_puts(out, "\n");
    return 1;
}

static DIST_POINT *crldp_from_section(X509V3_CTX *ctx,
                                      STACK_OF(CONF_VALUE) *nval)
{
    int i;
    CONF_VALUE *cnf;
    DIST_POINT *point = DIST_POINT_new();

    if (point == NULL)
        goto err;
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        int ret;
        cnf = sk_CONF_VALUE_value(nval, i);
        ret = set_dist_point_name(&point->distpoint, ctx, cnf);
        if (ret > 0)
            continue;
        if (ret < 0)
            goto err;
        if (strcmp(cnf->name, "reasons") == 0) {
            if (!set_reasons(&point->reasons, cnf->value))
                goto err;
        } else if (strcmp(cnf->name, "CRLissuer") == 0) {
            point->CRLissuer = gnames_from_sectname(ctx, cnf->value);
            if (point->CRLissuer == NULL)
                goto err;
        }
    }

    return point;

 err:
    DIST_POINT_free(point);
    return NULL;
}

static void *v2i_crld(const X509V3_EXT_METHOD *method,
                      X509V3_CTX *ctx, STACK_OF(CONF_VALUE) *nval)
{
    STACK_OF(DIST_POINT) *crld;
    GENERAL_NAMES *gens = NULL;
    GENERAL_NAME *gen = NULL;
    CONF_VALUE *cnf;
    const int num = sk_CONF_VALUE_num(nval);
    int i;

    crld = sk_DIST_POINT_new_reserve(NULL, num);
    if (crld == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        goto err;
    }
    for (i = 0; i < num; i++) {
        DIST_POINT *point;

        cnf = sk_CONF_VALUE_value(nval, i);
        if (cnf->value == NULL) {
            STACK_OF(CONF_VALUE) *dpsect;
            dpsect = X509V3_get_section(ctx, cnf->name);
            if (!dpsect)
                goto err;
            point = crldp_from_section(ctx, dpsect);
            X509V3_section_free(ctx, dpsect);
            if (point == NULL)
                goto err;
            sk_DIST_POINT_push(crld, point); /* no failure as it was reserved */
        } else {
            if ((gen = v2i_GENERAL_NAME(method, ctx, cnf)) == NULL)
                goto err;
            if ((gens = GENERAL_NAMES_new()) == NULL) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
            if (!sk_GENERAL_NAME_push(gens, gen)) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
                goto err;
            }
            gen = NULL;
            if ((point = DIST_POINT_new()) == NULL) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
            sk_DIST_POINT_push(crld, point); /* no failure as it was reserved */
            if ((point->distpoint = DIST_POINT_NAME_new()) == NULL) {
                ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
                goto err;
            }
            point->distpoint->name.fullname = gens;
            point->distpoint->type = 0;
            gens = NULL;
        }
    }
    return crld;

 err:
    GENERAL_NAME_free(gen);
    GENERAL_NAMES_free(gens);
    sk_DIST_POINT_pop_free(crld, DIST_POINT_free);
    return NULL;
}

static int dpn_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                  void *exarg)
{
    DIST_POINT_NAME *dpn = (DIST_POINT_NAME *)*pval;

    switch (operation) {
    case ASN1_OP_NEW_POST:
        dpn->dpname = NULL;
        break;

    case ASN1_OP_FREE_POST:
        X509_NAME_free(dpn->dpname);
        break;
    }
    return 1;
}


ASN1_CHOICE_cb(DIST_POINT_NAME, dpn_cb) = {
        ASN1_IMP_SEQUENCE_OF(DIST_POINT_NAME, name.fullname, GENERAL_NAME, 0),
        ASN1_IMP_SET_OF(DIST_POINT_NAME, name.relativename, X509_NAME_ENTRY, 1)
} ASN1_CHOICE_END_cb(DIST_POINT_NAME, DIST_POINT_NAME, type)


IMPLEMENT_ASN1_FUNCTIONS(DIST_POINT_NAME)
IMPLEMENT_ASN1_DUP_FUNCTION(DIST_POINT_NAME)

ASN1_SEQUENCE(DIST_POINT) = {
        ASN1_EXP_OPT(DIST_POINT, distpoint, DIST_POINT_NAME, 0),
        ASN1_IMP_OPT(DIST_POINT, reasons, ASN1_BIT_STRING, 1),
        ASN1_IMP_SEQUENCE_OF_OPT(DIST_POINT, CRLissuer, GENERAL_NAME, 2)
} ASN1_SEQUENCE_END(DIST_POINT)

IMPLEMENT_ASN1_FUNCTIONS(DIST_POINT)

ASN1_ITEM_TEMPLATE(CRL_DIST_POINTS) =
        ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0, CRLDistributionPoints, DIST_POINT)
ASN1_ITEM_TEMPLATE_END(CRL_DIST_POINTS)

IMPLEMENT_ASN1_FUNCTIONS(CRL_DIST_POINTS)

ASN1_SEQUENCE(ISSUING_DIST_POINT) = {
        ASN1_EXP_OPT(ISSUING_DIST_POINT, distpoint, DIST_POINT_NAME, 0),
        ASN1_IMP_OPT(ISSUING_DIST_POINT, onlyuser, ASN1_FBOOLEAN, 1),
        ASN1_IMP_OPT(ISSUING_DIST_POINT, onlyCA, ASN1_FBOOLEAN, 2),
        ASN1_IMP_OPT(ISSUING_DIST_POINT, onlysomereasons, ASN1_BIT_STRING, 3),
        ASN1_IMP_OPT(ISSUING_DIST_POINT, indirectCRL, ASN1_FBOOLEAN, 4),
        ASN1_IMP_OPT(ISSUING_DIST_POINT, onlyattr, ASN1_FBOOLEAN, 5)
} ASN1_SEQUENCE_END(ISSUING_DIST_POINT)

IMPLEMENT_ASN1_FUNCTIONS(ISSUING_DIST_POINT)

static int i2r_idp(const X509V3_EXT_METHOD *method, void *pidp, BIO *out,
                   int indent);
static void *v2i_idp(const X509V3_EXT_METHOD *method, X509V3_CTX *ctx,
                     STACK_OF(CONF_VALUE) *nval);

const X509V3_EXT_METHOD ossl_v3_idp = {
    NID_issuing_distribution_point, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(ISSUING_DIST_POINT),
    0, 0, 0, 0,
    0, 0,
    0,
    v2i_idp,
    i2r_idp, 0,
    NULL
};

static void *v2i_idp(const X509V3_EXT_METHOD *method, X509V3_CTX *ctx,
                     STACK_OF(CONF_VALUE) *nval)
{
    ISSUING_DIST_POINT *idp = NULL;
    CONF_VALUE *cnf;
    char *name, *val;
    int i, ret;
    idp = ISSUING_DIST_POINT_new();
    if (idp == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        goto err;
    }
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        cnf = sk_CONF_VALUE_value(nval, i);
        name = cnf->name;
        val = cnf->value;
        ret = set_dist_point_name(&idp->distpoint, ctx, cnf);
        if (ret > 0)
            continue;
        if (ret < 0)
            goto err;
        if (strcmp(name, "onlyuser") == 0) {
            if (!X509V3_get_value_bool(cnf, &idp->onlyuser))
                goto err;
        } else if (strcmp(name, "onlyCA") == 0) {
            if (!X509V3_get_value_bool(cnf, &idp->onlyCA))
                goto err;
        } else if (strcmp(name, "onlyAA") == 0) {
            if (!X509V3_get_value_bool(cnf, &idp->onlyattr))
                goto err;
        } else if (strcmp(name, "indirectCRL") == 0) {
            if (!X509V3_get_value_bool(cnf, &idp->indirectCRL))
                goto err;
        } else if (strcmp(name, "onlysomereasons") == 0) {
            if (!set_reasons(&idp->onlysomereasons, val))
                goto err;
        } else {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_NAME);
            X509V3_conf_add_error_name_value(cnf);
            goto err;
        }
    }
    return idp;

 err:
    ISSUING_DIST_POINT_free(idp);
    return NULL;
}

static int print_distpoint(BIO *out, DIST_POINT_NAME *dpn, int indent)
{
    if (dpn->type == 0) {
        BIO_printf(out, "%*sFull Name:\n", indent, "");
        OSSL_GENERAL_NAMES_print(out, dpn->name.fullname, indent);
        BIO_puts(out, "\n");
    } else {
        X509_NAME ntmp;
        ntmp.entries = dpn->name.relativename;
        BIO_printf(out, "%*sRelative Name:\n%*s", indent, "", indent + 2, "");
        X509_NAME_print_ex(out, &ntmp, 0, XN_FLAG_ONELINE);
        BIO_puts(out, "\n");
    }
    return 1;
}

static int i2r_idp(const X509V3_EXT_METHOD *method, void *pidp, BIO *out,
                   int indent)
{
    ISSUING_DIST_POINT *idp = pidp;
    if (idp->distpoint)
        print_distpoint(out, idp->distpoint, indent);
    if (idp->onlyuser > 0)
        BIO_printf(out, "%*sOnly User Certificates\n", indent, "");
    if (idp->onlyCA > 0)
        BIO_printf(out, "%*sOnly CA Certificates\n", indent, "");
    if (idp->indirectCRL > 0)
        BIO_printf(out, "%*sIndirect CRL\n", indent, "");
    if (idp->onlysomereasons)
        print_reasons(out, "Only Some Reasons", idp->onlysomereasons, indent);
    if (idp->onlyattr > 0)
        BIO_printf(out, "%*sOnly Attribute Certificates\n", indent, "");
    if (!idp->distpoint && (idp->onlyuser <= 0) && (idp->onlyCA <= 0)
        && (idp->indirectCRL <= 0) && !idp->onlysomereasons
        && (idp->onlyattr <= 0))
        BIO_printf(out, "%*s<EMPTY>\n", indent, "");

    return 1;
}

static int i2r_crldp(const X509V3_EXT_METHOD *method, void *pcrldp, BIO *out,
                     int indent)
{
    STACK_OF(DIST_POINT) *crld = pcrldp;
    DIST_POINT *point;
    int i;
    for (i = 0; i < sk_DIST_POINT_num(crld); i++) {
        if (i > 0)
            BIO_puts(out, "\n");
        point = sk_DIST_POINT_value(crld, i);
        if (point->distpoint)
            print_distpoint(out, point->distpoint, indent);
        if (point->reasons)
            print_reasons(out, "Reasons", point->reasons, indent);
        if (point->CRLissuer) {
            BIO_printf(out, "%*sCRL Issuer:\n", indent, "");
            OSSL_GENERAL_NAMES_print(out, point->CRLissuer, indent);
        }
    }
    return 1;
}

/* Append any nameRelativeToCRLIssuer in dpn to iname, set in dpn->dpname */
int DIST_POINT_set_dpname(DIST_POINT_NAME *dpn, const X509_NAME *iname)
{
    int i;
    STACK_OF(X509_NAME_ENTRY) *frag;
    X509_NAME_ENTRY *ne;

    if (dpn == NULL || dpn->type != 1)
        return 1;
    frag = dpn->name.relativename;
    X509_NAME_free(dpn->dpname); /* just in case it was already set */
    dpn->dpname = X509_NAME_dup(iname);
    if (dpn->dpname == NULL)
        return 0;
    for (i = 0; i < sk_X509_NAME_ENTRY_num(frag); i++) {
        ne = sk_X509_NAME_ENTRY_value(frag, i);
        if (!X509_NAME_add_entry(dpn->dpname, ne, -1, i ? 0 : 1))
            goto err;
    }
    /* generate cached encoding of name */
    if (i2d_X509_NAME(dpn->dpname, NULL) >= 0)
        return 1;

 err:
    X509_NAME_free(dpn->dpname);
    dpn->dpname = NULL;
    return 0;
}

ASN1_SEQUENCE(OSSL_AA_DIST_POINT) = {
    ASN1_EXP_OPT(OSSL_AA_DIST_POINT, distpoint, DIST_POINT_NAME, 0),
    ASN1_IMP_OPT(OSSL_AA_DIST_POINT, reasons, ASN1_BIT_STRING, 1),
    ASN1_IMP_OPT(OSSL_AA_DIST_POINT, indirectCRL, ASN1_FBOOLEAN, 2),
    ASN1_IMP_OPT(OSSL_AA_DIST_POINT, containsUserAttributeCerts, ASN1_TBOOLEAN, 3),
    ASN1_IMP_OPT(OSSL_AA_DIST_POINT, containsAACerts, ASN1_TBOOLEAN, 4),
    ASN1_IMP_OPT(OSSL_AA_DIST_POINT, containsSOAPublicKeyCerts, ASN1_TBOOLEAN, 5)
} ASN1_SEQUENCE_END(OSSL_AA_DIST_POINT)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_AA_DIST_POINT)

static int print_boolean(BIO *out, ASN1_BOOLEAN b)
{
    return BIO_puts(out, b ? "TRUE" : "FALSE");
}

static OSSL_AA_DIST_POINT *aaidp_from_section(X509V3_CTX *ctx,
                                              STACK_OF(CONF_VALUE) *nval)
{
    int i, ret;
    CONF_VALUE *cnf;
    OSSL_AA_DIST_POINT *point = OSSL_AA_DIST_POINT_new();

    if (point == NULL)
        goto err;
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        cnf = sk_CONF_VALUE_value(nval, i);
        ret = set_dist_point_name(&point->distpoint, ctx, cnf);
        if (ret > 0)
            continue;
        if (ret < 0)
            goto err;
        if (strcmp(cnf->name, "reasons") == 0) {
            if (!set_reasons(&point->reasons, cnf->value))
                goto err;
        } else if (strcmp(cnf->name, "indirectCRL") == 0) {
            if (!X509V3_get_value_bool(cnf, &point->indirectCRL))
                goto err;
        } else if (strcmp(cnf->name, "containsUserAttributeCerts") == 0) {
            if (!X509V3_get_value_bool(cnf, &point->containsUserAttributeCerts))
                goto err;
        } else if (strcmp(cnf->name, "containsAACerts") == 0) {
            if (!X509V3_get_value_bool(cnf, &point->containsAACerts))
                goto err;
        } else if (strcmp(cnf->name, "containsSOAPublicKeyCerts") == 0) {
            if (!X509V3_get_value_bool(cnf, &point->containsSOAPublicKeyCerts))
                goto err;
        }
    }

    return point;

 err:
    OSSL_AA_DIST_POINT_free(point);
    return NULL;
}

static void *v2i_aaidp(const X509V3_EXT_METHOD *method,
                       X509V3_CTX *ctx, STACK_OF(CONF_VALUE) *nval)
{
    GENERAL_NAMES *gens = NULL;
    GENERAL_NAME *gen = NULL;
    CONF_VALUE *cnf;
    OSSL_AA_DIST_POINT *point = NULL;
    STACK_OF(CONF_VALUE) *dpsect;

    cnf = sk_CONF_VALUE_value(nval, 0);
    if (cnf == NULL)
        return NULL;
    if (cnf->value == NULL) {
        dpsect = X509V3_get_section(ctx, cnf->name);
        if (dpsect == NULL)
            goto err;
        point = aaidp_from_section(ctx, dpsect);
        X509V3_section_free(ctx, dpsect);
        if (point == NULL)
            goto err;
    } else {
        if ((gen = v2i_GENERAL_NAME(method, ctx, cnf)) == NULL)
            goto err;
        if ((gens = GENERAL_NAMES_new()) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        if (!sk_GENERAL_NAME_push(gens, gen)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
            goto err;
        }
        gen = NULL;
        if ((point = OSSL_AA_DIST_POINT_new()) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        if ((point->distpoint = DIST_POINT_NAME_new()) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        point->distpoint->name.fullname = gens;
        point->distpoint->type = 0;
        gens = NULL;
    }
    return point;

 err:
    OSSL_AA_DIST_POINT_free(point);
    GENERAL_NAME_free(gen);
    GENERAL_NAMES_free(gens);
    return NULL;
}

static int i2r_aaidp(const X509V3_EXT_METHOD *method, void *dp, BIO *out,
                     int indent)
{
    OSSL_AA_DIST_POINT *pdp = dp;

    if (pdp->distpoint)
        if (print_distpoint(out, pdp->distpoint, indent) <= 0)
            return 0;
    if (pdp->reasons)
        if (print_reasons(out, "Reasons", pdp->reasons, indent) <= 0)
            return 0;
    if (pdp->indirectCRL) {
        if (BIO_printf(out, "%*sIndirect CRL: ", indent, "") <= 0)
            return 0;
        if (print_boolean(out, pdp->indirectCRL) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    if (pdp->containsUserAttributeCerts) {
        if (BIO_printf(out, "%*sContains User Attribute Certificates: ", indent, "") <= 0)
            return 0;
        if (print_boolean(out, pdp->containsUserAttributeCerts) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    if (pdp->containsAACerts) {
        if (BIO_printf(out, "%*sContains Attribute Authority (AA) Certificates: ",
                       indent, "") <= 0)
            return 0;
        if (print_boolean(out, pdp->containsAACerts) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    if (pdp->containsSOAPublicKeyCerts) {
        if (BIO_printf(out,
                       "%*sContains Source Of Authority (SOA) Public Key Certificates: ",
                       indent, "") <= 0)
            return 0;
        if (print_boolean(out, pdp->containsSOAPublicKeyCerts) <= 0)
            return 0;
        if (BIO_puts(out, "\n") <= 0)
            return 0;
    }
    return 1;
}

const X509V3_EXT_METHOD ossl_v3_aa_issuing_dist_point = {
    NID_id_aa_issuing_distribution_point, 0,
    ASN1_ITEM_ref(OSSL_AA_DIST_POINT),
    0, 0, 0, 0,
    0, 0,
    0,
    v2i_aaidp,
    i2r_aaidp, 0,
    NULL
};
