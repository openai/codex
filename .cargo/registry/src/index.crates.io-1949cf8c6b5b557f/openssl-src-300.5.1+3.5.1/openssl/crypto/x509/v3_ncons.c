/*
 * Copyright 2003-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include "internal/numbers.h"
#include "internal/safe_math.h"
#include <stdio.h>
#include "crypto/asn1.h"
#include <openssl/asn1t.h>
#include <openssl/conf.h>
#include <openssl/http.h>
#include <openssl/x509v3.h>
#include <openssl/bn.h>

#include "crypto/x509.h"
#include "crypto/punycode.h"
#include "ext_dat.h"

OSSL_SAFE_MATH_SIGNED(int, int)

static void *v2i_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                  X509V3_CTX *ctx,
                                  STACK_OF(CONF_VALUE) *nval);
static int i2r_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method, void *a,
                                BIO *bp, int ind);
static int do_i2r_name_constraints(const X509V3_EXT_METHOD *method,
                                   STACK_OF(GENERAL_SUBTREE) *trees, BIO *bp,
                                   int ind, const char *name);
static int print_nc_ipadd(BIO *bp, ASN1_OCTET_STRING *ip);

static int nc_match(GENERAL_NAME *gen, NAME_CONSTRAINTS *nc);
static int nc_match_single(int effective_type, GENERAL_NAME *gen,
                           GENERAL_NAME *base);
static int nc_dn(const X509_NAME *sub, const X509_NAME *nm);
static int nc_dns(ASN1_IA5STRING *sub, ASN1_IA5STRING *dns);
static int nc_email(ASN1_IA5STRING *sub, ASN1_IA5STRING *eml);
static int nc_email_eai(ASN1_TYPE *emltype, ASN1_IA5STRING *base);
static int nc_uri(ASN1_IA5STRING *uri, ASN1_IA5STRING *base);
static int nc_ip(ASN1_OCTET_STRING *ip, ASN1_OCTET_STRING *base);

const X509V3_EXT_METHOD ossl_v3_name_constraints = {
    NID_name_constraints, 0,
    ASN1_ITEM_ref(NAME_CONSTRAINTS),
    0, 0, 0, 0,
    0, 0,
    0, v2i_NAME_CONSTRAINTS,
    i2r_NAME_CONSTRAINTS, 0,
    NULL
};

const X509V3_EXT_METHOD ossl_v3_holder_name_constraints = {
    NID_holder_name_constraints, 0,
    ASN1_ITEM_ref(NAME_CONSTRAINTS),
    0, 0, 0, 0,
    0, 0,
    0, v2i_NAME_CONSTRAINTS,
    i2r_NAME_CONSTRAINTS, 0,
    NULL
};

const X509V3_EXT_METHOD ossl_v3_delegated_name_constraints = {
    NID_delegated_name_constraints, 0,
    ASN1_ITEM_ref(NAME_CONSTRAINTS),
    0, 0, 0, 0,
    0, 0,
    0, v2i_NAME_CONSTRAINTS,
    i2r_NAME_CONSTRAINTS, 0,
    NULL
};

ASN1_SEQUENCE(GENERAL_SUBTREE) = {
        ASN1_SIMPLE(GENERAL_SUBTREE, base, GENERAL_NAME),
        ASN1_IMP_OPT(GENERAL_SUBTREE, minimum, ASN1_INTEGER, 0),
        ASN1_IMP_OPT(GENERAL_SUBTREE, maximum, ASN1_INTEGER, 1)
} ASN1_SEQUENCE_END(GENERAL_SUBTREE)

ASN1_SEQUENCE(NAME_CONSTRAINTS) = {
        ASN1_IMP_SEQUENCE_OF_OPT(NAME_CONSTRAINTS, permittedSubtrees,
                                                        GENERAL_SUBTREE, 0),
        ASN1_IMP_SEQUENCE_OF_OPT(NAME_CONSTRAINTS, excludedSubtrees,
                                                        GENERAL_SUBTREE, 1),
} ASN1_SEQUENCE_END(NAME_CONSTRAINTS)


IMPLEMENT_ASN1_ALLOC_FUNCTIONS(GENERAL_SUBTREE)
IMPLEMENT_ASN1_ALLOC_FUNCTIONS(NAME_CONSTRAINTS)


#define IA5_OFFSET_LEN(ia5base, offset) \
    ((ia5base)->length - ((unsigned char *)(offset) - (ia5base)->data))

/* Like memchr but for ASN1_IA5STRING. Additionally you can specify the
 * starting point to search from
 */
# define ia5memchr(str, start, c) memchr(start, c, IA5_OFFSET_LEN(str, start))

/* Like memrrchr but for ASN1_IA5STRING */
static char *ia5memrchr(ASN1_IA5STRING *str, int c)
{
    int i;

    for (i = str->length; i > 0 && str->data[i - 1] != c; i--);

    if (i == 0)
        return NULL;

    return (char *)&str->data[i - 1];
}

/*
 * We cannot use strncasecmp here because that applies locale specific rules. It
 * also doesn't work with ASN1_STRINGs that may have embedded NUL characters.
 * For example in Turkish 'I' is not the uppercase character for 'i'. We need to
 * do a simple ASCII case comparison ignoring the locale (that is why we use
 * numeric constants below).
 */
static int ia5ncasecmp(const char *s1, const char *s2, size_t n)
{
    for (; n > 0; n--, s1++, s2++) {
        if (*s1 != *s2) {
            unsigned char c1 = (unsigned char)*s1, c2 = (unsigned char)*s2;

            /* Convert to lower case */
            if (c1 >= 0x41 /* A */ && c1 <= 0x5A /* Z */)
                c1 += 0x20;
            if (c2 >= 0x41 /* A */ && c2 <= 0x5A /* Z */)
                c2 += 0x20;

            if (c1 == c2)
                continue;

            if (c1 < c2)
                return -1;

            /* c1 > c2 */
            return 1;
        }
    }

    return 0;
}

static void *v2i_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                  X509V3_CTX *ctx, STACK_OF(CONF_VALUE) *nval)
{
    int i;
    CONF_VALUE tval, *val;
    STACK_OF(GENERAL_SUBTREE) **ptree = NULL;
    NAME_CONSTRAINTS *ncons = NULL;
    GENERAL_SUBTREE *sub = NULL;

    ncons = NAME_CONSTRAINTS_new();
    if (ncons == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
        goto err;
    }
    for (i = 0; i < sk_CONF_VALUE_num(nval); i++) {
        val = sk_CONF_VALUE_value(nval, i);
        if (HAS_PREFIX(val->name, "permitted") && val->name[9]) {
            ptree = &ncons->permittedSubtrees;
            tval.name = val->name + 10;
        } else if (HAS_PREFIX(val->name, "excluded") && val->name[8]) {
            ptree = &ncons->excludedSubtrees;
            tval.name = val->name + 9;
        } else {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_INVALID_SYNTAX);
            goto err;
        }
        tval.value = val->value;
        sub = GENERAL_SUBTREE_new();
        if (sub == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        if (!v2i_GENERAL_NAME_ex(sub->base, method, ctx, &tval, 1)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_X509V3_LIB);
            goto err;
        }
        if (*ptree == NULL)
            *ptree = sk_GENERAL_SUBTREE_new_null();
        if (*ptree == NULL || !sk_GENERAL_SUBTREE_push(*ptree, sub)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
            goto err;
        }
        sub = NULL;
    }

    return ncons;

 err:
    NAME_CONSTRAINTS_free(ncons);
    GENERAL_SUBTREE_free(sub);

    return NULL;
}

static int i2r_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method, void *a,
                                BIO *bp, int ind)
{
    NAME_CONSTRAINTS *ncons = a;
    do_i2r_name_constraints(method, ncons->permittedSubtrees,
                            bp, ind, "Permitted");
    if (ncons->permittedSubtrees && ncons->excludedSubtrees)
        BIO_puts(bp, "\n");
    do_i2r_name_constraints(method, ncons->excludedSubtrees,
                            bp, ind, "Excluded");
    return 1;
}

static int do_i2r_name_constraints(const X509V3_EXT_METHOD *method,
                                   STACK_OF(GENERAL_SUBTREE) *trees,
                                   BIO *bp, int ind, const char *name)
{
    GENERAL_SUBTREE *tree;
    int i;
    if (sk_GENERAL_SUBTREE_num(trees) > 0)
        BIO_printf(bp, "%*s%s:\n", ind, "", name);
    for (i = 0; i < sk_GENERAL_SUBTREE_num(trees); i++) {
        if (i > 0)
            BIO_puts(bp, "\n");
        tree = sk_GENERAL_SUBTREE_value(trees, i);
        BIO_printf(bp, "%*s", ind + 2, "");
        if (tree->base->type == GEN_IPADD)
            print_nc_ipadd(bp, tree->base->d.ip);
        else
            GENERAL_NAME_print(bp, tree->base);
    }
    return 1;
}

static int print_nc_ipadd(BIO *bp, ASN1_OCTET_STRING *ip)
{
    /* ip->length should be 8 or 32 and len1 == len2 == 4 or len1 == len2 == 16 */
    int len1 = ip->length >= 16 ? 16 : ip->length >= 4 ? 4 : ip->length;
    int len2 = ip->length - len1;
    char *ip1 = ossl_ipaddr_to_asc(ip->data, len1);
    char *ip2 = ossl_ipaddr_to_asc(ip->data + len1, len2);
    int ret = ip1 != NULL && ip2 != NULL
        && BIO_printf(bp, "IP:%s/%s", ip1, ip2) > 0;

    OPENSSL_free(ip1);
    OPENSSL_free(ip2);
    return ret;
}

#define NAME_CHECK_MAX (1 << 20)

static int add_lengths(int *out, int a, int b)
{
    int err = 0;

    /* sk_FOO_num(NULL) returns -1 but is effectively 0 when iterating. */
    if (a < 0)
        a = 0;
    if (b < 0)
        b = 0;

    *out = safe_add_int(a, b, &err);
    return !err;
}

/*-
 * Check a certificate conforms to a specified set of constraints.
 * Return values:
 *  X509_V_OK: All constraints obeyed.
 *  X509_V_ERR_PERMITTED_VIOLATION: Permitted subtree violation.
 *  X509_V_ERR_EXCLUDED_VIOLATION: Excluded subtree violation.
 *  X509_V_ERR_SUBTREE_MINMAX: Min or max values present and matching type.
 *  X509_V_ERR_UNSUPPORTED_CONSTRAINT_TYPE:  Unsupported constraint type.
 *  X509_V_ERR_UNSUPPORTED_CONSTRAINT_SYNTAX: bad unsupported constraint syntax.
 *  X509_V_ERR_UNSUPPORTED_NAME_SYNTAX: bad or unsupported syntax of name
 */

int NAME_CONSTRAINTS_check(X509 *x, NAME_CONSTRAINTS *nc)
{
    int r, i, name_count, constraint_count;
    X509_NAME *nm;

    nm = X509_get_subject_name(x);

    /*
     * Guard against certificates with an excessive number of names or
     * constraints causing a computationally expensive name constraints check.
     */
    if (!add_lengths(&name_count, X509_NAME_entry_count(nm),
                     sk_GENERAL_NAME_num(x->altname))
        || !add_lengths(&constraint_count,
                        sk_GENERAL_SUBTREE_num(nc->permittedSubtrees),
                        sk_GENERAL_SUBTREE_num(nc->excludedSubtrees))
        || (name_count > 0 && constraint_count > NAME_CHECK_MAX / name_count))
        return X509_V_ERR_UNSPECIFIED;

    if (X509_NAME_entry_count(nm) > 0) {
        GENERAL_NAME gntmp;
        gntmp.type = GEN_DIRNAME;
        gntmp.d.directoryName = nm;

        r = nc_match(&gntmp, nc);

        if (r != X509_V_OK)
            return r;

        gntmp.type = GEN_EMAIL;

        /* Process any email address attributes in subject name */

        for (i = -1;;) {
            const X509_NAME_ENTRY *ne;

            i = X509_NAME_get_index_by_NID(nm, NID_pkcs9_emailAddress, i);
            if (i == -1)
                break;
            ne = X509_NAME_get_entry(nm, i);
            gntmp.d.rfc822Name = X509_NAME_ENTRY_get_data(ne);
            if (gntmp.d.rfc822Name->type != V_ASN1_IA5STRING)
                return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;

            r = nc_match(&gntmp, nc);

            if (r != X509_V_OK)
                return r;
        }

    }

    for (i = 0; i < sk_GENERAL_NAME_num(x->altname); i++) {
        GENERAL_NAME *gen = sk_GENERAL_NAME_value(x->altname, i);
        r = nc_match(gen, nc);
        if (r != X509_V_OK)
            return r;
    }

    return X509_V_OK;

}

static int cn2dnsid(ASN1_STRING *cn, unsigned char **dnsid, size_t *idlen)
{
    int utf8_length;
    unsigned char *utf8_value;
    int i;
    int isdnsname = 0;

    /* Don't leave outputs uninitialized */
    *dnsid = NULL;
    *idlen = 0;

    /*-
     * Per RFC 6125, DNS-IDs representing internationalized domain names appear
     * in certificates in A-label encoded form:
     *
     *   https://tools.ietf.org/html/rfc6125#section-6.4.2
     *
     * The same applies to CNs which are intended to represent DNS names.
     * However, while in the SAN DNS-IDs are IA5Strings, as CNs they may be
     * needlessly encoded in 16-bit Unicode.  We perform a conversion to UTF-8
     * to ensure that we get an ASCII representation of any CNs that are
     * representable as ASCII, but just not encoded as ASCII.  The UTF-8 form
     * may contain some non-ASCII octets, and that's fine, such CNs are not
     * valid legacy DNS names.
     *
     * Note, 'int' is the return type of ASN1_STRING_to_UTF8() so that's what
     * we must use for 'utf8_length'.
     */
    if ((utf8_length = ASN1_STRING_to_UTF8(&utf8_value, cn)) < 0)
        return X509_V_ERR_OUT_OF_MEM;

    /*
     * Some certificates have had names that include a *trailing* NUL byte.
     * Remove these harmless NUL characters. They would otherwise yield false
     * alarms with the following embedded NUL check.
     */
    while (utf8_length > 0 && utf8_value[utf8_length - 1] == '\0')
        --utf8_length;

    /* Reject *embedded* NULs */
    if (memchr(utf8_value, 0, utf8_length) != NULL) {
        OPENSSL_free(utf8_value);
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
    }

    /*
     * XXX: Deviation from strict DNS name syntax, also check names with '_'
     * Check DNS name syntax, any '-' or '.' must be internal,
     * and on either side of each '.' we can't have a '-' or '.'.
     *
     * If the name has just one label, we don't consider it a DNS name.  This
     * means that "CN=sometld" cannot be precluded by DNS name constraints, but
     * that is not a problem.
     */
    for (i = 0; i < utf8_length; ++i) {
        unsigned char c = utf8_value[i];

        if ((c >= 'a' && c <= 'z')
            || (c >= 'A' && c <= 'Z')
            || (c >= '0' && c <= '9')
            || c == '_')
            continue;

        /* Dot and hyphen cannot be first or last. */
        if (i > 0 && i < utf8_length - 1) {
            if (c == '-')
                continue;
            /*
             * Next to a dot the preceding and following characters must not be
             * another dot or a hyphen.  Otherwise, record that the name is
             * plausible, since it has two or more labels.
             */
            if (c == '.'
                && utf8_value[i + 1] != '.'
                && utf8_value[i - 1] != '-'
                && utf8_value[i + 1] != '-') {
                isdnsname = 1;
                continue;
            }
        }
        isdnsname = 0;
        break;
    }

    if (isdnsname) {
        *dnsid = utf8_value;
        *idlen = (size_t)utf8_length;
        return X509_V_OK;
    }
    OPENSSL_free(utf8_value);
    return X509_V_OK;
}

/*
 * Check CN against DNS-ID name constraints.
 */
int NAME_CONSTRAINTS_check_CN(X509 *x, NAME_CONSTRAINTS *nc)
{
    int r, i;
    const X509_NAME *nm = X509_get_subject_name(x);
    ASN1_STRING stmp;
    GENERAL_NAME gntmp;

    stmp.flags = 0;
    stmp.type = V_ASN1_IA5STRING;
    gntmp.type = GEN_DNS;
    gntmp.d.dNSName = &stmp;

    /* Process any commonName attributes in subject name */

    for (i = -1;;) {
        X509_NAME_ENTRY *ne;
        ASN1_STRING *cn;
        unsigned char *idval;
        size_t idlen;

        i = X509_NAME_get_index_by_NID(nm, NID_commonName, i);
        if (i == -1)
            break;
        ne = X509_NAME_get_entry(nm, i);
        cn = X509_NAME_ENTRY_get_data(ne);

        /* Only process attributes that look like hostnames */
        if ((r = cn2dnsid(cn, &idval, &idlen)) != X509_V_OK)
            return r;
        if (idlen == 0)
            continue;

        stmp.length = idlen;
        stmp.data = idval;
        r = nc_match(&gntmp, nc);
        OPENSSL_free(idval);
        if (r != X509_V_OK)
            return r;
    }
    return X509_V_OK;
}

/*
 * Return nonzero if the GeneralSubtree has valid 'minimum' field
 * (must be absent or 0) and valid 'maximum' field (must be absent).
 */
static int nc_minmax_valid(GENERAL_SUBTREE *sub) {
    BIGNUM *bn = NULL;
    int ok = 1;

    if (sub->maximum)
        ok = 0;

    if (sub->minimum) {
        bn = ASN1_INTEGER_to_BN(sub->minimum, NULL);
        if (bn == NULL || !BN_is_zero(bn))
            ok = 0;
        BN_free(bn);
    }

    return ok;
}

static int nc_match(GENERAL_NAME *gen, NAME_CONSTRAINTS *nc)
{
    GENERAL_SUBTREE *sub;
    int i, r, match = 0;
    int effective_type = gen->type;

    /*
     * We need to compare not gen->type field but an "effective" type because
     * the otherName field may contain EAI email address treated specially
     * according to RFC 8398, section 6
     */
    if (effective_type == GEN_OTHERNAME &&
        (OBJ_obj2nid(gen->d.otherName->type_id) == NID_id_on_SmtpUTF8Mailbox)) {
        effective_type = GEN_EMAIL;
    }

    /*
     * Permitted subtrees: if any subtrees exist of matching the type at
     * least one subtree must match.
     */

    for (i = 0; i < sk_GENERAL_SUBTREE_num(nc->permittedSubtrees); i++) {
        sub = sk_GENERAL_SUBTREE_value(nc->permittedSubtrees, i);
        if (effective_type != sub->base->type
            || (effective_type == GEN_OTHERNAME &&
                OBJ_cmp(gen->d.otherName->type_id,
                        sub->base->d.otherName->type_id) != 0))
            continue;
        if (!nc_minmax_valid(sub))
            return X509_V_ERR_SUBTREE_MINMAX;
        /* If we already have a match don't bother trying any more */
        if (match == 2)
            continue;
        if (match == 0)
            match = 1;
        r = nc_match_single(effective_type, gen, sub->base);
        if (r == X509_V_OK)
            match = 2;
        else if (r != X509_V_ERR_PERMITTED_VIOLATION)
            return r;
    }

    if (match == 1)
        return X509_V_ERR_PERMITTED_VIOLATION;

    /* Excluded subtrees: must not match any of these */

    for (i = 0; i < sk_GENERAL_SUBTREE_num(nc->excludedSubtrees); i++) {
        sub = sk_GENERAL_SUBTREE_value(nc->excludedSubtrees, i);
        if (effective_type != sub->base->type
            || (effective_type == GEN_OTHERNAME &&
                OBJ_cmp(gen->d.otherName->type_id,
                        sub->base->d.otherName->type_id) != 0))
            continue;
        if (!nc_minmax_valid(sub))
            return X509_V_ERR_SUBTREE_MINMAX;

        r = nc_match_single(effective_type, gen, sub->base);
        if (r == X509_V_OK)
            return X509_V_ERR_EXCLUDED_VIOLATION;
        else if (r != X509_V_ERR_PERMITTED_VIOLATION)
            return r;

    }

    return X509_V_OK;

}

static int nc_match_single(int effective_type, GENERAL_NAME *gen,
                           GENERAL_NAME *base)
{
    switch (gen->type) {
    case GEN_OTHERNAME:
        switch (effective_type) {
        case GEN_EMAIL:
            /*
             * We are here only when we have SmtpUTF8 name,
             * so we match the value of othername with base->d.rfc822Name
             */
            return nc_email_eai(gen->d.otherName->value, base->d.rfc822Name);

        default:
            return X509_V_ERR_UNSUPPORTED_CONSTRAINT_TYPE;
        }

    case GEN_DIRNAME:
        return nc_dn(gen->d.directoryName, base->d.directoryName);

    case GEN_DNS:
        return nc_dns(gen->d.dNSName, base->d.dNSName);

    case GEN_EMAIL:
        return nc_email(gen->d.rfc822Name, base->d.rfc822Name);

    case GEN_URI:
        return nc_uri(gen->d.uniformResourceIdentifier,
                      base->d.uniformResourceIdentifier);

    case GEN_IPADD:
        return nc_ip(gen->d.iPAddress, base->d.iPAddress);

    default:
        return X509_V_ERR_UNSUPPORTED_CONSTRAINT_TYPE;
    }

}

/*
 * directoryName name constraint matching. The canonical encoding of
 * X509_NAME makes this comparison easy. It is matched if the subtree is a
 * subset of the name.
 */

static int nc_dn(const X509_NAME *nm, const X509_NAME *base)
{
    /* Ensure canonical encodings are up to date.  */
    if (nm->modified && i2d_X509_NAME(nm, NULL) < 0)
        return X509_V_ERR_OUT_OF_MEM;
    if (base->modified && i2d_X509_NAME(base, NULL) < 0)
        return X509_V_ERR_OUT_OF_MEM;
    if (base->canon_enclen > nm->canon_enclen)
        return X509_V_ERR_PERMITTED_VIOLATION;
    if (memcmp(base->canon_enc, nm->canon_enc, base->canon_enclen))
        return X509_V_ERR_PERMITTED_VIOLATION;
    return X509_V_OK;
}

static int nc_dns(ASN1_IA5STRING *dns, ASN1_IA5STRING *base)
{
    char *baseptr = (char *)base->data;
    char *dnsptr = (char *)dns->data;

    /* Empty matches everything */
    if (base->length == 0)
        return X509_V_OK;

    if (dns->length < base->length)
        return X509_V_ERR_PERMITTED_VIOLATION;

    /*
     * Otherwise can add zero or more components on the left so compare RHS
     * and if dns is longer and expect '.' as preceding character.
     */
    if (dns->length > base->length) {
        dnsptr += dns->length - base->length;
        if (*baseptr != '.' && dnsptr[-1] != '.')
            return X509_V_ERR_PERMITTED_VIOLATION;
    }

    if (ia5ncasecmp(baseptr, dnsptr, base->length))
        return X509_V_ERR_PERMITTED_VIOLATION;

    return X509_V_OK;

}

/*
 * This function implements comparison between ASCII/U-label in emltype
 * and A-label in base according to RFC 8398, section 6.
 * Convert base to U-label and ASCII-parts of domain names, for base
 * Octet-to-octet comparison of `emltype` and `base` hostname parts
 * (ASCII-parts should be compared in case-insensitive manner)
 */
static int nc_email_eai(ASN1_TYPE *emltype, ASN1_IA5STRING *base)
{
    ASN1_UTF8STRING *eml;
    char *baseptr = NULL;
    const char *emlptr;
    const char *emlat;
    char ulabel[256];
    size_t size = sizeof(ulabel);
    int ret = X509_V_OK;
    size_t emlhostlen;

    /* We do not accept embedded NUL characters */
    if (base->length > 0 && memchr(base->data, 0, base->length) != NULL)
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;

    /* 'base' may not be NUL terminated. Create a copy that is */
    baseptr = OPENSSL_strndup((char *)base->data, base->length);
    if (baseptr == NULL)
        return X509_V_ERR_OUT_OF_MEM;

    if (emltype->type != V_ASN1_UTF8STRING) {
        ret = X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
        goto end;
    }

    eml = emltype->value.utf8string;
    emlptr = (char *)eml->data;
    emlat = ia5memrchr(eml, '@');

    if (emlat == NULL) {
        ret = X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
        goto end;
    }

    /* Special case: initial '.' is RHS match */
    if (*baseptr == '.') {
        ulabel[0] = '.';
        if (ossl_a2ulabel(baseptr, ulabel + 1, size - 1) <= 0) {
            ret = X509_V_ERR_UNSPECIFIED;
            goto end;
        }

        if ((size_t)eml->length > strlen(ulabel)) {
            emlptr += eml->length - strlen(ulabel);
            /* X509_V_OK */
            if (ia5ncasecmp(ulabel, emlptr, strlen(ulabel)) == 0)
                goto end;
        }
        ret = X509_V_ERR_PERMITTED_VIOLATION;
        goto end;
    }

    if (ossl_a2ulabel(baseptr, ulabel, size) <= 0) {
        ret = X509_V_ERR_UNSPECIFIED;
        goto end;
    }
    /* Just have hostname left to match: case insensitive */
    emlptr = emlat + 1;
    emlhostlen = IA5_OFFSET_LEN(eml, emlptr);
    if (emlhostlen != strlen(ulabel)
            || ia5ncasecmp(ulabel, emlptr, emlhostlen) != 0) {
        ret = X509_V_ERR_PERMITTED_VIOLATION;
        goto end;
    }

 end:
    OPENSSL_free(baseptr);
    return ret;
}

static int nc_email(ASN1_IA5STRING *eml, ASN1_IA5STRING *base)
{
    const char *baseptr = (char *)base->data;
    const char *emlptr = (char *)eml->data;
    const char *baseat = ia5memrchr(base, '@');
    const char *emlat = ia5memrchr(eml, '@');
    size_t basehostlen, emlhostlen;

    if (!emlat)
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
    /* Special case: initial '.' is RHS match */
    if (!baseat && base->length > 0 && (*baseptr == '.')) {
        if (eml->length > base->length) {
            emlptr += eml->length - base->length;
            if (ia5ncasecmp(baseptr, emlptr, base->length) == 0)
                return X509_V_OK;
        }
        return X509_V_ERR_PERMITTED_VIOLATION;
    }

    /* If we have anything before '@' match local part */

    if (baseat) {
        if (baseat != baseptr) {
            if ((baseat - baseptr) != (emlat - emlptr))
                return X509_V_ERR_PERMITTED_VIOLATION;
            if (memchr(baseptr, 0, baseat - baseptr) ||
                memchr(emlptr, 0, emlat - emlptr))
                return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
            /* Case sensitive match of local part */
            if (strncmp(baseptr, emlptr, emlat - emlptr))
                return X509_V_ERR_PERMITTED_VIOLATION;
        }
        /* Position base after '@' */
        baseptr = baseat + 1;
    }
    emlptr = emlat + 1;
    basehostlen = IA5_OFFSET_LEN(base, baseptr);
    emlhostlen = IA5_OFFSET_LEN(eml, emlptr);
    /* Just have hostname left to match: case insensitive */
    if (basehostlen != emlhostlen || ia5ncasecmp(baseptr, emlptr, emlhostlen))
        return X509_V_ERR_PERMITTED_VIOLATION;

    return X509_V_OK;

}

static int nc_uri(ASN1_IA5STRING *uri, ASN1_IA5STRING *base)
{
    const char *baseptr = (char *)base->data;
    char *uri_copy;
    char *scheme;
    char *host;
    int hostlen;
    int ret;

    if ((uri_copy = OPENSSL_strndup((const char *)uri->data, uri->length)) == NULL)
        return X509_V_ERR_UNSPECIFIED;

    if (!OSSL_parse_url(uri_copy, &scheme, NULL, &host, NULL, NULL, NULL, NULL, NULL)) {
        OPENSSL_free(uri_copy);
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
    }

    /* Make sure the scheme is there */
    if (scheme == NULL || *scheme == '\0') {
        ERR_raise_data(ERR_LIB_X509V3, X509_V_ERR_UNSUPPORTED_NAME_SYNTAX,
                       "x509: missing scheme in URI: %s\n", uri_copy);
        OPENSSL_free(uri_copy);
        ret = X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
        goto end;
    }

    /* We don't need these anymore */
    OPENSSL_free(scheme);
    OPENSSL_free(uri_copy);

    hostlen = strlen(host);

    /* Special case: initial '.' is RHS match */
    if (base->length > 0 && *baseptr == '.') {
        if (hostlen > base->length) {
            if (ia5ncasecmp(host + hostlen - base->length, baseptr, base->length) == 0) {
                ret = X509_V_OK;
                goto end;
            }
        }
        ret = X509_V_ERR_PERMITTED_VIOLATION;
        goto end;
    }

    if ((base->length != hostlen)
        || ia5ncasecmp(host, baseptr, hostlen) != 0) {
        ret = X509_V_ERR_PERMITTED_VIOLATION;
        goto end;
    }

    ret = X509_V_OK;
end:
    OPENSSL_free(host);
    return ret;

}

static int nc_ip(ASN1_OCTET_STRING *ip, ASN1_OCTET_STRING *base)
{
    int hostlen, baselen, i;
    unsigned char *hostptr, *baseptr, *maskptr;
    hostptr = ip->data;
    hostlen = ip->length;
    baseptr = base->data;
    baselen = base->length;

    /* Invalid if not IPv4 or IPv6 */
    if (!((hostlen == 4) || (hostlen == 16)))
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
    if (!((baselen == 8) || (baselen == 32)))
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;

    /* Do not match IPv4 with IPv6 */
    if (hostlen * 2 != baselen)
        return X509_V_ERR_PERMITTED_VIOLATION;

    maskptr = base->data + hostlen;

    /* Considering possible not aligned base ipAddress */
    /* Not checking for wrong mask definition: i.e.: 255.0.255.0 */
    for (i = 0; i < hostlen; i++)
        if ((hostptr[i] & maskptr[i]) != (baseptr[i] & maskptr[i]))
            return X509_V_ERR_PERMITTED_VIOLATION;

    return X509_V_OK;

}
