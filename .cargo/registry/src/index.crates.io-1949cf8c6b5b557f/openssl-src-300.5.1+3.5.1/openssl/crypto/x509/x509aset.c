/*
 * Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "x509_acert.h"

static int replace_gentime(ASN1_STRING **dest, const ASN1_GENERALIZEDTIME *src)
{
    ASN1_STRING *s;

    if (src->type != V_ASN1_GENERALIZEDTIME)
        return 0;

    if (*dest == src)
        return 1;

    s = ASN1_STRING_dup(src);
    if (s == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        return 0;
    }

    ASN1_STRING_free(*dest);
    *dest = s;

    return 1;
}

static int replace_dirName(GENERAL_NAMES **names, const X509_NAME *dirName)
{
    GENERAL_NAME *gen_name = NULL;
    STACK_OF(GENERAL_NAME) *new_names = NULL;
    X509_NAME *name_copy;

    if ((name_copy = X509_NAME_dup(dirName)) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    if ((new_names = sk_GENERAL_NAME_new_null()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    if ((gen_name = GENERAL_NAME_new()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    if (sk_GENERAL_NAME_push(new_names, gen_name) <= 0) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }

    GENERAL_NAME_set0_value(gen_name, GEN_DIRNAME, name_copy);

    GENERAL_NAMES_free(*names);
    *names = new_names;

    return 1;

err:
    GENERAL_NAME_free(gen_name);
    sk_GENERAL_NAME_free(new_names);
    X509_NAME_free(name_copy);
    return 0;
}

int OSSL_OBJECT_DIGEST_INFO_set1_digest(OSSL_OBJECT_DIGEST_INFO *o,
                                        int digestedObjectType,
                                        X509_ALGOR *digestAlgorithm,
                                        ASN1_BIT_STRING *digest)
{

    if (ASN1_ENUMERATED_set(&o->digestedObjectType, digestedObjectType) <= 0)
        return 0;

    if (X509_ALGOR_copy(&o->digestAlgorithm, digestAlgorithm) <= 0)
        return 0;

    if (ASN1_STRING_copy(&o->objectDigest, digest) <= 0)
        return 0;

    return 1;
}

int OSSL_ISSUER_SERIAL_set1_issuer(OSSL_ISSUER_SERIAL *isss,
                                   const X509_NAME *issuer)
{
    return replace_dirName(&isss->issuer, issuer);
}

int OSSL_ISSUER_SERIAL_set1_serial(OSSL_ISSUER_SERIAL *isss,
                                   const ASN1_INTEGER *serial)
{
    return ASN1_STRING_copy(&isss->serial, serial);
}

int OSSL_ISSUER_SERIAL_set1_issuerUID(OSSL_ISSUER_SERIAL *isss,
                                      const ASN1_BIT_STRING *uid)
{
    ASN1_BIT_STRING_free(isss->issuerUID);
    isss->issuerUID = ASN1_STRING_dup(uid);
    if (isss->issuerUID == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        return 0;
    }
    return 1;
}

int X509_ACERT_set_version(X509_ACERT *x, long version)
{
    return ASN1_INTEGER_set(&x->acinfo->version, version);
}

void X509_ACERT_set0_holder_entityName(X509_ACERT *x, GENERAL_NAMES *names)
{
    GENERAL_NAMES_free(x->acinfo->holder.entityName);
    x->acinfo->holder.entityName = names;
}

void X509_ACERT_set0_holder_baseCertId(X509_ACERT *x,
                                       OSSL_ISSUER_SERIAL *isss)
{
    OSSL_ISSUER_SERIAL_free(x->acinfo->holder.baseCertificateID);
    x->acinfo->holder.baseCertificateID = isss;
}

void X509_ACERT_set0_holder_digest(X509_ACERT *x,
                                   OSSL_OBJECT_DIGEST_INFO *dinfo)
{
    OSSL_OBJECT_DIGEST_INFO_free(x->acinfo->holder.objectDigestInfo);
    x->acinfo->holder.objectDigestInfo = dinfo;
}

int X509_ACERT_set1_issuerName(X509_ACERT *x, const X509_NAME *name)
{
    X509_ACERT_ISSUER_V2FORM *v2Form;

    v2Form = x->acinfo->issuer.u.v2Form;

    /* only v2Form is supported, so always create that version */
    if (v2Form == NULL) {
        v2Form = X509_ACERT_ISSUER_V2FORM_new();
        if (v2Form == NULL) {
            ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
            return 0;
        }
        x->acinfo->issuer.u.v2Form = v2Form;
        x->acinfo->issuer.type = X509_ACERT_ISSUER_V2;
    }

    return replace_dirName(&v2Form->issuerName, name);
}

int X509_ACERT_set1_serialNumber(X509_ACERT *x, const ASN1_INTEGER *serial)
{
    return ASN1_STRING_copy(&x->acinfo->serialNumber, serial);
}

int X509_ACERT_set1_notBefore(X509_ACERT *x, const ASN1_GENERALIZEDTIME *time)
{
    return replace_gentime(&x->acinfo->validityPeriod.notBefore, time);
}

int X509_ACERT_set1_notAfter(X509_ACERT *x, const ASN1_GENERALIZEDTIME *time)
{
    return replace_gentime(&x->acinfo->validityPeriod.notAfter, time);
}
