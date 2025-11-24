/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "crypto/x509.h"
#include <openssl/conf.h>
#include <openssl/x509v3.h>
#include <openssl/bio.h>
#include "ext_dat.h"

static GENERAL_NAMES *v2i_subject_alt(X509V3_EXT_METHOD *method,
                                      X509V3_CTX *ctx,
                                      STACK_OF(CONF_VALUE) *nval);
static GENERAL_NAMES *v2i_issuer_alt(X509V3_EXT_METHOD *method,
                                     X509V3_CTX *ctx,
                                     STACK_OF(CONF_VALUE) *nval);
static int copy_email(X509V3_CTX *ctx, GENERAL_NAMES *gens, int move_p);
static int copy_issuer(X509V3_CTX *ctx, GENERAL_NAMES *gens);
static int do_othername(GENERAL_NAME *gen, const char *value, X509V3_CTX *ctx);
static int do_dirname(GENERAL_NAME *gen, const char *value, X509V3_CTX *ctx);

const X509V3_EXT_METHOD ossl_v3_alt[3] = {
    {NID_subject_alt_name, 0, ASN1_ITEM_ref(GENERAL_NAMES),
     0, 0, 0, 0,
     0, 0,
     (X509V3_EXT_I2V) i2v_GENERAL_NAMES,
     (X509V3_EXT_V2I)v2i_subject_alt,
     NULL, NULL, NULL},

    {NID_issuer_alt_name, 0, ASN1_ITEM_ref(GENERAL_NAMES),
     0, 0, 0, 0,
     0, 0,
     (X509V3_EXT_I2V) i2v_GENERAL_NAMES,
     (X509V3_EXT_V2I)v2i_issuer_alt,
     NULL, NULL, NULL},

    {NID_certificate_issuer, 0, ASN1_ITEM_ref(GENERAL_NAMES),
     0, 0, 0, 0,
     0, 0,
     (X509V3_EXT_I2V) i2v_GENERAL_NAMES,
     NULL, NULL, NULL, NULL},
};

STACK_OF(CONF_VALUE) *i2v_GENERAL_NAMES(X509V3_EXT_METHOD *method,
                                        GENERAL_NAMES *gens,
                                        STACK_OF(CONF_VALUE) *ret)
{
    int i;
    GENERAL_NAME *gen;
    STACK_OF(CONF_VALUE) *tmpret = NULL, *origret = ret;

    for (i = 0; i < sk_GENERAL_NAME_num(gens); i++) {
        gen = sk_GENERAL_NAME_value(gens, i);
        /*
         * i2v_GENERAL_NAME allocates ret if it is NULL. If something goes
         * wrong we need to free the stack - but only if it was empty when we
         * originally entered this function.
         */
        tmpret = i2v_GENERAL_NAME(method, gen, ret);
        if (tmpret == NULL) {
            if (origret == NULL)
                sk_CONF_VALUE_pop_free(ret, X509V3_conf_free);
            return NULL;
        }
        ret = tmpret;
    }
    if (ret == NULL)
        return sk_CONF_VALUE_new_null();
    return ret;
}

STACK_OF(CONF_VALUE) *i2v_GENERAL_NAME(X509V3_EXT_METHOD *method,
                                       GENERAL_NAME *gen,
                                       STACK_OF(CONF_VALUE) *ret)
{
    char othername[300];
    char oline[256], *tmp;

    switch (gen->type) {
    case GEN_OTHERNAME:
        switch (OBJ_obj2nid(gen->d.otherName->type_id)) {
        case NID_id_on_SmtpUTF8Mailbox:
            if (gen->d.otherName->value->type != V_ASN1_UTF8STRING
                    || !x509v3_add_len_value_uchar("othername: SmtpUTF8Mailbox",
                            gen->d.otherName->value->value.utf8string->data,
                            gen->d.otherName->value->value.utf8string->length,
                            &ret))
                return NULL;
            break;
        case NID_XmppAddr:
            if (gen->d.otherName->value->type != V_ASN1_UTF8STRING
                    || !x509v3_add_len_value_uchar("othername: XmppAddr",
                            gen->d.otherName->value->value.utf8string->data,
                            gen->d.otherName->value->value.utf8string->length,
                            &ret))
                return NULL;
            break;
        case NID_SRVName:
            if (gen->d.otherName->value->type != V_ASN1_IA5STRING
                    || !x509v3_add_len_value_uchar("othername: SRVName",
                            gen->d.otherName->value->value.ia5string->data,
                            gen->d.otherName->value->value.ia5string->length,
                            &ret))
                return NULL;
            break;
        case NID_ms_upn:
            if (gen->d.otherName->value->type != V_ASN1_UTF8STRING
                    || !x509v3_add_len_value_uchar("othername: UPN",
                            gen->d.otherName->value->value.utf8string->data,
                            gen->d.otherName->value->value.utf8string->length,
                            &ret))
                return NULL;
            break;
        case NID_NAIRealm:
            if (gen->d.otherName->value->type != V_ASN1_UTF8STRING
                    || !x509v3_add_len_value_uchar("othername: NAIRealm",
                            gen->d.otherName->value->value.utf8string->data,
                            gen->d.otherName->value->value.utf8string->length,
                            &ret))
                return NULL;
            break;
        default:
            if (OBJ_obj2txt(oline, sizeof(oline), gen->d.otherName->type_id, 0) > 0)
                BIO_snprintf(othername, sizeof(othername), "othername: %s",
                             oline);
            else
                OPENSSL_strlcpy(othername, "othername", sizeof(othername));

            /* check if the value is something printable */
            if (gen->d.otherName->value->type == V_ASN1_IA5STRING) {
                if (x509v3_add_len_value_uchar(othername,
                             gen->d.otherName->value->value.ia5string->data,
                             gen->d.otherName->value->value.ia5string->length,
                             &ret))
                    return ret;
            }
            if (gen->d.otherName->value->type == V_ASN1_UTF8STRING) {
                if (x509v3_add_len_value_uchar(othername,
                             gen->d.otherName->value->value.utf8string->data,
                             gen->d.otherName->value->value.utf8string->length,
                             &ret))
                    return ret;
            }
            if (!X509V3_add_value(othername, "<unsupported>", &ret))
                return NULL;
            break;
        }
        break;

    case GEN_X400:
        if (!X509V3_add_value("X400Name", "<unsupported>", &ret))
            return NULL;
        break;

    case GEN_EDIPARTY:
        if (!X509V3_add_value("EdiPartyName", "<unsupported>", &ret))
            return NULL;
        break;

    case GEN_EMAIL:
        if (!x509v3_add_len_value_uchar("email", gen->d.ia5->data,
                                        gen->d.ia5->length, &ret))
            return NULL;
        break;

    case GEN_DNS:
        if (!x509v3_add_len_value_uchar("DNS", gen->d.ia5->data,
                                        gen->d.ia5->length, &ret))
            return NULL;
        break;

    case GEN_URI:
        if (!x509v3_add_len_value_uchar("URI", gen->d.ia5->data,
                                        gen->d.ia5->length, &ret))
            return NULL;
        break;

    case GEN_DIRNAME:
        if (X509_NAME_oneline(gen->d.dirn, oline, sizeof(oline)) == NULL
                || !X509V3_add_value("DirName", oline, &ret))
            return NULL;
        break;

    case GEN_IPADD:
        tmp = ossl_ipaddr_to_asc(gen->d.ip->data, gen->d.ip->length);
        if (tmp == NULL || !X509V3_add_value("IP Address", tmp, &ret))
            ret = NULL;
        OPENSSL_free(tmp);
        break;

    case GEN_RID:
        i2t_ASN1_OBJECT(oline, 256, gen->d.rid);
        if (!X509V3_add_value("Registered ID", oline, &ret))
            return NULL;
        break;
    }
    return ret;
}

int GENERAL_NAME_print(BIO *out, GENERAL_NAME *gen)
{
    char *tmp;
    int nid;

    switch (gen->type) {
    case GEN_OTHERNAME:
        nid = OBJ_obj2nid(gen->d.otherName->type_id);
        /* Validate the types are as we expect before we use them */
        if ((nid == NID_SRVName
             && gen->d.otherName->value->type != V_ASN1_IA5STRING)
                || (nid != NID_SRVName
                    && gen->d.otherName->value->type != V_ASN1_UTF8STRING)) {
            BIO_printf(out, "othername:<unsupported>");
            break;
        }

        switch (nid) {
        case NID_id_on_SmtpUTF8Mailbox:
            BIO_printf(out, "othername:SmtpUTF8Mailbox:%.*s",
                       gen->d.otherName->value->value.utf8string->length,
                       gen->d.otherName->value->value.utf8string->data);
            break;
        case NID_XmppAddr:
            BIO_printf(out, "othername:XmppAddr:%.*s",
                       gen->d.otherName->value->value.utf8string->length,
                       gen->d.otherName->value->value.utf8string->data);
            break;
        case NID_SRVName:
            BIO_printf(out, "othername:SRVName:%.*s",
                       gen->d.otherName->value->value.ia5string->length,
                       gen->d.otherName->value->value.ia5string->data);
            break;
        case NID_ms_upn:
            BIO_printf(out, "othername:UPN:%.*s",
                       gen->d.otherName->value->value.utf8string->length,
                       gen->d.otherName->value->value.utf8string->data);
            break;
        case NID_NAIRealm:
            BIO_printf(out, "othername:NAIRealm:%.*s",
                       gen->d.otherName->value->value.utf8string->length,
                       gen->d.otherName->value->value.utf8string->data);
            break;
        default:
            BIO_printf(out, "othername:<unsupported>");
            break;
        }
        break;

    case GEN_X400:
        BIO_printf(out, "X400Name:<unsupported>");
        break;

    case GEN_EDIPARTY:
        /* Maybe fix this: it is supported now */
        BIO_printf(out, "EdiPartyName:<unsupported>");
        break;

    case GEN_EMAIL:
        BIO_printf(out, "email:");
        ASN1_STRING_print(out, gen->d.ia5);
        break;

    case GEN_DNS:
        BIO_printf(out, "DNS:");
        ASN1_STRING_print(out, gen->d.ia5);
        break;

    case GEN_URI:
        BIO_printf(out, "URI:");
        ASN1_STRING_print(out, gen->d.ia5);
        break;

    case GEN_DIRNAME:
        BIO_printf(out, "DirName:");
        X509_NAME_print_ex(out, gen->d.dirn, 0, XN_FLAG_ONELINE);
        break;

    case GEN_IPADD:
        tmp = ossl_ipaddr_to_asc(gen->d.ip->data, gen->d.ip->length);
        if (tmp == NULL)
            return 0;
        BIO_printf(out, "IP Address:%s", tmp);
        OPENSSL_free(tmp);
        break;

    case GEN_RID:
        BIO_printf(out, "Registered ID:");
        i2a_ASN1_OBJECT(out, gen->d.rid);
        break;
    }
    return 1;
}

static GENERAL_NAMES *v2i_issuer_alt(X509V3_EXT_METHOD *method,
                                     X509V3_CTX *ctx,
                                     STACK_OF(CONF_VALUE) *nval)
{
    const int num = sk_CONF_VALUE_num(nval);
    GENERAL_NAMES *gens = sk_GENERAL_NAME_new_reserve(NULL, num);
    int i;

    if (gens == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        return NULL;
    }
    for (i = 0; i < num; i++) {
        CONF_VALUE *cnf = sk_CONF_VALUE_value(nval, i);

        if (!ossl_v3_name_cmp(cnf->name, "issuer")
            && cnf->value && strcmp(cnf->value, "copy") == 0) {
            if (!copy_issuer(ctx, gens))
                goto err;
        } else {
            GENERAL_NAME *gen = v2i_GENERAL_NAME(method, ctx, cnf);

            if (gen == NULL)
                goto err;
            sk_GENERAL_NAME_push(gens, gen); /* no failure as it was reserved */
        }
    }
    return gens;
 err:
    sk_GENERAL_NAME_pop_free(gens, GENERAL_NAME_free);
    return NULL;
}

/* Append subject altname of issuer to issuer alt name of subject */

static int copy_issuer(X509V3_CTX *ctx, GENERAL_NAMES *gens)
{
    GENERAL_NAMES *ialt = NULL;
    GENERAL_NAME *gen;
    X509_EXTENSION *ext;
    int i, num;

    if (ctx != NULL && (ctx->flags & X509V3_CTX_TEST) != 0)
        return 1;
    if (!ctx || !ctx->issuer_cert) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_NO_ISSUER_DETAILS);
        goto err;
    }
    i = X509_get_ext_by_NID(ctx->issuer_cert, NID_subject_alt_name, -1);
    if (i < 0)
        return 1;
    if ((ext = X509_get_ext(ctx->issuer_cert, i)) == NULL
        || (ialt = X509V3_EXT_d2i(ext)) == NULL) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_ISSUER_DECODE_ERROR);
        goto err;
    }

    num = sk_GENERAL_NAME_num(ialt);
    if (!sk_GENERAL_NAME_reserve(gens, num)) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        goto err;
    }

    for (i = 0; i < num; i++) {
        gen = sk_GENERAL_NAME_value(ialt, i);
        sk_GENERAL_NAME_push(gens, gen);     /* no failure as it was reserved */
    }
    sk_GENERAL_NAME_free(ialt);

    return 1;

 err:
    sk_GENERAL_NAME_free(ialt);
    return 0;

}

static GENERAL_NAMES *v2i_subject_alt(X509V3_EXT_METHOD *method,
                                      X509V3_CTX *ctx,
                                      STACK_OF(CONF_VALUE) *nval)
{
    GENERAL_NAMES *gens;
    CONF_VALUE *cnf;
    const int num = sk_CONF_VALUE_num(nval);
    int i;

    gens = sk_GENERAL_NAME_new_reserve(NULL, num);
    if (gens == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        return NULL;
    }

    for (i = 0; i < num; i++) {
        cnf = sk_CONF_VALUE_value(nval, i);
        if (ossl_v3_name_cmp(cnf->name, "email") == 0
            && cnf->value && strcmp(cnf->value, "copy") == 0) {
            if (!copy_email(ctx, gens, 0))
                goto err;
        } else if (ossl_v3_name_cmp(cnf->name, "email") == 0
                   && cnf->value && strcmp(cnf->value, "move") == 0) {
            if (!copy_email(ctx, gens, 1))
                goto err;
        } else {
            GENERAL_NAME *gen;
            if ((gen = v2i_GENERAL_NAME(method, ctx, cnf)) == NULL)
                goto err;
            sk_GENERAL_NAME_push(gens, gen); /* no failure as it was reserved */
        }
    }
    return gens;
 err:
    sk_GENERAL_NAME_pop_free(gens, GENERAL_NAME_free);
    return NULL;
}

/*
 * Copy any email addresses in a certificate or request to GENERAL_NAMES
 */

static int copy_email(X509V3_CTX *ctx, GENERAL_NAMES *gens, int move_p)
{
    X509_NAME *nm;
    ASN1_IA5STRING *email = NULL;
    X509_NAME_ENTRY *ne;
    GENERAL_NAME *gen = NULL;
    int i = -1;

    if (ctx != NULL && (ctx->flags & X509V3_CTX_TEST) != 0)
        return 1;
    if (ctx == NULL
        || (ctx->subject_cert == NULL && ctx->subject_req == NULL)) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_NO_SUBJECT_DETAILS);
        return 0;
    }
    /* Find the subject name */
    nm = ctx->subject_cert != NULL ?
        X509_get_subject_name(ctx->subject_cert) :
        X509_REQ_get_subject_name(ctx->subject_req);

    /* Now add any email address(es) to STACK */
    while ((i = X509_NAME_get_index_by_NID(nm,
                                           NID_pkcs9_emailAddress, i)) >= 0) {
        ne = X509_NAME_get_entry(nm, i);
        email = ASN1_STRING_dup(X509_NAME_ENTRY_get_data(ne));
        if (move_p) {
            X509_NAME_delete_entry(nm, i);
            X509_NAME_ENTRY_free(ne);
            i--;
        }
        if (email == NULL || (gen = GENERAL_NAME_new()) == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
        gen->d.ia5 = email;
        email = NULL;
        gen->type = GEN_EMAIL;
        if (!sk_GENERAL_NAME_push(gens, gen)) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
            goto err;
        }
        gen = NULL;
    }

    return 1;

 err:
    GENERAL_NAME_free(gen);
    ASN1_IA5STRING_free(email);
    return 0;

}

GENERAL_NAMES *v2i_GENERAL_NAMES(const X509V3_EXT_METHOD *method,
                                 X509V3_CTX *ctx, STACK_OF(CONF_VALUE) *nval)
{
    GENERAL_NAME *gen;
    GENERAL_NAMES *gens;
    CONF_VALUE *cnf;
    const int num = sk_CONF_VALUE_num(nval);
    int i;

    gens = sk_GENERAL_NAME_new_reserve(NULL, num);
    if (gens == NULL) {
        ERR_raise(ERR_LIB_X509V3, ERR_R_CRYPTO_LIB);
        return NULL;
    }

    for (i = 0; i < num; i++) {
        cnf = sk_CONF_VALUE_value(nval, i);
        if ((gen = v2i_GENERAL_NAME(method, ctx, cnf)) == NULL)
            goto err;
        sk_GENERAL_NAME_push(gens, gen);    /* no failure as it was reserved */
    }
    return gens;
 err:
    sk_GENERAL_NAME_pop_free(gens, GENERAL_NAME_free);
    return NULL;
}

GENERAL_NAME *v2i_GENERAL_NAME(const X509V3_EXT_METHOD *method,
                               X509V3_CTX *ctx, CONF_VALUE *cnf)
{
    return v2i_GENERAL_NAME_ex(NULL, method, ctx, cnf, 0);
}

GENERAL_NAME *a2i_GENERAL_NAME(GENERAL_NAME *out,
                               const X509V3_EXT_METHOD *method,
                               X509V3_CTX *ctx, int gen_type, const char *value,
                               int is_nc)
{
    char is_string = 0;
    GENERAL_NAME *gen = NULL;

    if (!value) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_MISSING_VALUE);
        return NULL;
    }

    if (out)
        gen = out;
    else {
        gen = GENERAL_NAME_new();
        if (gen == NULL) {
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            return NULL;
        }
    }

    switch (gen_type) {
    case GEN_URI:
    case GEN_EMAIL:
    case GEN_DNS:
        is_string = 1;
        break;

    case GEN_RID:
        {
            ASN1_OBJECT *obj;
            if ((obj = OBJ_txt2obj(value, 0)) == NULL) {
                ERR_raise_data(ERR_LIB_X509V3, X509V3_R_BAD_OBJECT,
                               "value=%s", value);
                goto err;
            }
            gen->d.rid = obj;
        }
        break;

    case GEN_IPADD:
        if (is_nc)
            gen->d.ip = a2i_IPADDRESS_NC(value);
        else
            gen->d.ip = a2i_IPADDRESS(value);
        if (gen->d.ip == NULL) {
            ERR_raise_data(ERR_LIB_X509V3, X509V3_R_BAD_IP_ADDRESS,
                           "value=%s", value);
            goto err;
        }
        break;

    case GEN_DIRNAME:
        if (!do_dirname(gen, value, ctx)) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_DIRNAME_ERROR);
            goto err;
        }
        break;

    case GEN_OTHERNAME:
        if (!do_othername(gen, value, ctx)) {
            ERR_raise(ERR_LIB_X509V3, X509V3_R_OTHERNAME_ERROR);
            goto err;
        }
        break;
    default:
        ERR_raise(ERR_LIB_X509V3, X509V3_R_UNSUPPORTED_TYPE);
        goto err;
    }

    if (is_string) {
        if ((gen->d.ia5 = ASN1_IA5STRING_new()) == NULL ||
            !ASN1_STRING_set(gen->d.ia5, (unsigned char *)value,
                             strlen(value))) {
            ASN1_IA5STRING_free(gen->d.ia5);
            gen->d.ia5 = NULL;
            ERR_raise(ERR_LIB_X509V3, ERR_R_ASN1_LIB);
            goto err;
        }
    }

    gen->type = gen_type;

    return gen;

 err:
    if (!out)
        GENERAL_NAME_free(gen);
    return NULL;
}

GENERAL_NAME *v2i_GENERAL_NAME_ex(GENERAL_NAME *out,
                                  const X509V3_EXT_METHOD *method,
                                  X509V3_CTX *ctx, CONF_VALUE *cnf, int is_nc)
{
    int type;

    char *name, *value;

    name = cnf->name;
    value = cnf->value;

    if (!value) {
        ERR_raise(ERR_LIB_X509V3, X509V3_R_MISSING_VALUE);
        return NULL;
    }

    if (!ossl_v3_name_cmp(name, "email"))
        type = GEN_EMAIL;
    else if (!ossl_v3_name_cmp(name, "URI"))
        type = GEN_URI;
    else if (!ossl_v3_name_cmp(name, "DNS"))
        type = GEN_DNS;
    else if (!ossl_v3_name_cmp(name, "RID"))
        type = GEN_RID;
    else if (!ossl_v3_name_cmp(name, "IP"))
        type = GEN_IPADD;
    else if (!ossl_v3_name_cmp(name, "dirName"))
        type = GEN_DIRNAME;
    else if (!ossl_v3_name_cmp(name, "otherName"))
        type = GEN_OTHERNAME;
    else {
        ERR_raise_data(ERR_LIB_X509V3, X509V3_R_UNSUPPORTED_OPTION,
                       "name=%s", name);
        return NULL;
    }

    return a2i_GENERAL_NAME(out, method, ctx, type, value, is_nc);

}

static int do_othername(GENERAL_NAME *gen, const char *value, X509V3_CTX *ctx)
{
    char *objtmp = NULL, *p;
    int objlen;

    if ((p = strchr(value, ';')) == NULL)
        return 0;
    if ((gen->d.otherName = OTHERNAME_new()) == NULL)
        return 0;
    /*
     * Free this up because we will overwrite it. no need to free type_id
     * because it is static
     */
    ASN1_TYPE_free(gen->d.otherName->value);
    if ((gen->d.otherName->value = ASN1_generate_v3(p + 1, ctx)) == NULL)
        goto err;
    objlen = p - value;
    objtmp = OPENSSL_strndup(value, objlen);
    if (objtmp == NULL)
        goto err;
    gen->d.otherName->type_id = OBJ_txt2obj(objtmp, 0);
    OPENSSL_free(objtmp);
    if (!gen->d.otherName->type_id)
        goto err;
    return 1;

 err:
    OTHERNAME_free(gen->d.otherName);
    gen->d.otherName = NULL;
    return 0;
}

static int do_dirname(GENERAL_NAME *gen, const char *value, X509V3_CTX *ctx)
{
    int ret = 0;
    STACK_OF(CONF_VALUE) *sk = NULL;
    X509_NAME *nm;

    if ((nm = X509_NAME_new()) == NULL)
        goto err;
    sk = X509V3_get_section(ctx, value);
    if (!sk) {
        ERR_raise_data(ERR_LIB_X509V3, X509V3_R_SECTION_NOT_FOUND,
                       "section=%s", value);
        goto err;
    }
    /* FIXME: should allow other character types... */
    ret = X509V3_NAME_from_section(nm, sk, MBSTRING_ASC);
    if (!ret)
        goto err;
    gen->d.dirn = nm;

err:
    if (ret == 0)
        X509_NAME_free(nm);
    X509V3_section_free(ctx, sk);
    return ret;
}
