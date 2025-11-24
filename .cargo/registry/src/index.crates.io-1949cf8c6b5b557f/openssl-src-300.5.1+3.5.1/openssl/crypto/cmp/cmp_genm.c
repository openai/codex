/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2022
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "cmp_local.h"
#include <openssl/cmp_util.h>

static const X509_VERIFY_PARAM *get0_trustedStore_vpm(const OSSL_CMP_CTX *ctx)
{
    const X509_STORE *ts = OSSL_CMP_CTX_get0_trustedStore(ctx);

    return ts == NULL ? NULL : X509_STORE_get0_param(ts);
}

static void cert_msg(const char *func, const char *file, int lineno,
                     OSSL_CMP_severity level, OSSL_CMP_CTX *ctx,
                     const char *source, X509 *cert, const char *msg)
{
    char *subj = X509_NAME_oneline(X509_get_subject_name(cert), NULL, 0);

    ossl_cmp_print_log(level, ctx, func, file, lineno,
                       level == OSSL_CMP_LOG_WARNING ? "WARN" : "ERR",
                       "certificate from '%s' with subject '%s' %s",
                       source, subj, msg);
    OPENSSL_free(subj);
}

/* use |type_CA| -1 (no CA type check) or 0 (must be EE) or 1 (must be CA) */
static int ossl_X509_check(OSSL_CMP_CTX *ctx, const char *source, X509 *cert,
                           int type_CA, const X509_VERIFY_PARAM *vpm)
{
    uint32_t ex_flags = X509_get_extension_flags(cert);
    int res = X509_cmp_timeframe(vpm, X509_get0_notBefore(cert),
                                 X509_get0_notAfter(cert));
    int ret = res == 0;
    OSSL_CMP_severity level =
        vpm == NULL ? OSSL_CMP_LOG_WARNING : OSSL_CMP_LOG_ERR;

    if (!ret)
        cert_msg(OPENSSL_FUNC, OPENSSL_FILE, OPENSSL_LINE, level, ctx,
                 source, cert, res > 0 ? "has expired" : "not yet valid");
    if (type_CA >= 0 && (ex_flags & EXFLAG_V1) == 0) {
        int is_CA = (ex_flags & EXFLAG_CA) != 0;

        if ((type_CA != 0) != is_CA) {
            cert_msg(OPENSSL_FUNC, OPENSSL_FILE, OPENSSL_LINE, level, ctx,
                     source, cert,
                     is_CA ? "is not an EE cert" : "is not a CA cert");
            ret = 0;
        }
    }
    return ret;
}

static int ossl_X509_check_all(OSSL_CMP_CTX *ctx, const char *source,
                               STACK_OF(X509) *certs,
                               int type_CA, const X509_VERIFY_PARAM *vpm)
{
    int i;
    int ret = 1;

    for (i = 0; i < sk_X509_num(certs /* may be NULL */); i++)
        ret = ossl_X509_check(ctx, source,
                              sk_X509_value(certs, i), type_CA, vpm)
            && ret; /* Having 'ret' after the '&&', all certs are checked. */
    return ret;
}

static OSSL_CMP_ITAV *get_genm_itav(OSSL_CMP_CTX *ctx,
                                    OSSL_CMP_ITAV *req, /* gets consumed */
                                    int expected, const char *desc)
{
    STACK_OF(OSSL_CMP_ITAV) *itavs = NULL;
    int i, n;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        goto err;
    }
    if (OSSL_CMP_CTX_get_status(ctx) != OSSL_CMP_PKISTATUS_unspecified) {
        ERR_raise_data(ERR_LIB_CMP, CMP_R_UNCLEAN_CTX,
                       "client context in unsuitable state; should call CMPclient_reinit() before");
        goto err;
    }

    if (!OSSL_CMP_CTX_push0_genm_ITAV(ctx, req))
        goto err;
    req = NULL;
    itavs = OSSL_CMP_exec_GENM_ses(ctx);
    if (itavs == NULL) {
        if (OSSL_CMP_CTX_get_status(ctx) != OSSL_CMP_PKISTATUS_request)
            ERR_raise_data(ERR_LIB_CMP, CMP_R_GETTING_GENP,
                           "with infoType %s", desc);
        return NULL;
    }

    if ((n = sk_OSSL_CMP_ITAV_num(itavs)) <= 0) {
        ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_GENP,
                       "response on genm requesting infoType %s does not include suitable value", desc);
        sk_OSSL_CMP_ITAV_free(itavs);
        return NULL;
    }

    if (n > 1)
        ossl_cmp_log2(WARN, ctx,
                      "response on genm contains %d ITAVs; will use the first ITAV with infoType id-it-%s",
                      n, desc);
    for (i = 0; i < n; i++) {
        OSSL_CMP_ITAV *itav = sk_OSSL_CMP_ITAV_shift(itavs);
        ASN1_OBJECT *obj = OSSL_CMP_ITAV_get0_type(itav);
        char name[128] = "genp contains InfoType '";
        size_t offset = strlen(name);

        if (OBJ_obj2nid(obj) == expected) {
            for (i++; i < n; i++)
                OSSL_CMP_ITAV_free(sk_OSSL_CMP_ITAV_shift(itavs));
            sk_OSSL_CMP_ITAV_free(itavs);
            return itav;
        }

        if (OBJ_obj2txt(name + offset, sizeof(name) - offset, obj, 0) < 0)
            strcat(name, "<unknown>");
        ossl_cmp_log2(WARN, ctx, "%s' while expecting 'id-it-%s'", name, desc);
        OSSL_CMP_ITAV_free(itav);
    }
    ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_GENP,
                   "could not find any ITAV for %s", desc);

 err:
    sk_OSSL_CMP_ITAV_free(itavs);
    OSSL_CMP_ITAV_free(req);
    return NULL;
}

int OSSL_CMP_get1_caCerts(OSSL_CMP_CTX *ctx, STACK_OF(X509) **out)
{
    OSSL_CMP_ITAV *req, *itav;
    STACK_OF(X509) *certs = NULL;
    int ret = 0;

    if (out == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    *out = NULL;

    if ((req = OSSL_CMP_ITAV_new_caCerts(NULL)) == NULL)
        return 0;
    if ((itav = get_genm_itav(ctx, req, NID_id_it_caCerts, "caCerts")) == NULL)
        return 0;
    if (!OSSL_CMP_ITAV_get0_caCerts(itav, &certs))
        goto end;
    ret = 1;
    if (certs == NULL) /* no CA certificate available */
        goto end;

    if (!ossl_X509_check_all(ctx, "genp", certs, 1 /* CA */,
                             get0_trustedStore_vpm(ctx))) {
        ret = 0;
        goto end;
    }
    *out = sk_X509_new_reserve(NULL, sk_X509_num(certs));
    if (!X509_add_certs(*out, certs,
                        X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP)) {
        sk_X509_pop_free(*out, X509_free);
        *out = NULL;
        ret = 0;
    }

 end:
    OSSL_CMP_ITAV_free(itav);
    return ret;
}

static int selfsigned_verify_cb(int ok, X509_STORE_CTX *store_ctx)
{
    if (ok == 0
            && X509_STORE_CTX_get_error_depth(store_ctx) == 0
            && X509_STORE_CTX_get_error(store_ctx)
            == X509_V_ERR_DEPTH_ZERO_SELF_SIGNED_CERT) {
        /* in this case, custom chain building */
        int i;
        STACK_OF(X509) *trust;
        STACK_OF(X509) *chain = X509_STORE_CTX_get0_chain(store_ctx);
        STACK_OF(X509) *untrusted = X509_STORE_CTX_get0_untrusted(store_ctx);
        X509_STORE_CTX_check_issued_fn check_issued =
            X509_STORE_CTX_get_check_issued(store_ctx);
        X509 *cert = sk_X509_value(chain, 0); /* target cert */
        X509 *issuer;

        for (i = 0; i < sk_X509_num(untrusted); i++) {
            cert = sk_X509_value(untrusted, i);
            if (!X509_add_cert(chain, cert, X509_ADD_FLAG_UP_REF))
                return 0;
        }

        trust = X509_STORE_get1_all_certs(X509_STORE_CTX_get0_store(store_ctx));
        for (i = 0; i < sk_X509_num(trust); i++) {
            issuer = sk_X509_value(trust, i);
            if ((*check_issued)(store_ctx, cert, issuer)) {
                if (X509_add_cert(chain, cert, X509_ADD_FLAG_UP_REF))
                    ok = 1;
                break;
            }
        }
        sk_X509_pop_free(trust, X509_free);
        return ok;
    } else {
        X509_STORE *ts = X509_STORE_CTX_get0_store(store_ctx);
        X509_STORE_CTX_verify_cb verify_cb;

        if (ts == NULL || (verify_cb = X509_STORE_get_verify_cb(ts)) == NULL)
            return ok;
        return (*verify_cb)(ok, store_ctx);
    }
}

/* vanilla X509_verify_cert() does not support self-signed certs as target */
static int verify_ss_cert(OSSL_LIB_CTX *libctx, const char *propq,
                          X509_STORE *ts, STACK_OF(X509) *untrusted,
                          X509 *target)
{
    X509_STORE_CTX *csc = NULL;
    int ok = 0;

    if (ts == NULL || target == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((csc = X509_STORE_CTX_new_ex(libctx, propq)) == NULL
            || !X509_STORE_CTX_init(csc, ts, target, untrusted))
        goto err;
    X509_STORE_CTX_set_verify_cb(csc, selfsigned_verify_cb);
    ok = X509_verify_cert(csc) > 0;

 err:
    X509_STORE_CTX_free(csc);
    return ok;
}

static int
verify_ss_cert_trans(OSSL_CMP_CTX *ctx, X509 *trusted /* may be NULL */,
                     X509 *trans /* the only untrusted cert, may be NULL */,
                     X509 *target, const char *desc)
{
    X509_STORE *ts = OSSL_CMP_CTX_get0_trusted(ctx);
    STACK_OF(X509) *untrusted = NULL;
    int res = 0;

    if (trusted != NULL) {
        X509_VERIFY_PARAM *vpm = X509_STORE_get0_param(ts);

        if ((ts = X509_STORE_new()) == NULL)
            return 0;
        if (!X509_STORE_set1_param(ts, vpm)
            || !X509_STORE_add_cert(ts, trusted))
            goto err;
    }

    if (trans != NULL
            && !ossl_x509_add_cert_new(&untrusted, trans, X509_ADD_FLAG_UP_REF))
        goto err;

    res = verify_ss_cert(OSSL_CMP_CTX_get0_libctx(ctx),
                         OSSL_CMP_CTX_get0_propq(ctx),
                         ts, untrusted, target);
    if (!res)
        ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_ROOTCAKEYUPDATE,
                       "failed to validate %s certificate received in genp %s",
                       desc, trusted == NULL ? "using trust store"
                       : "with given certificate as trust anchor");

 err:
    sk_X509_pop_free(untrusted, X509_free);
    if (trusted != NULL)
        X509_STORE_free(ts);
    return res;
}

int OSSL_CMP_get1_rootCaKeyUpdate(OSSL_CMP_CTX *ctx,
                                  const X509 *oldWithOld, X509 **newWithNew,
                                  X509 **newWithOld, X509 **oldWithNew)
{
    X509 *oldWithOld_copy = NULL, *my_newWithOld, *my_oldWithNew;
    OSSL_CMP_ITAV *req, *itav;
    int res = 0;

    if (newWithNew == NULL) {
        ERR_raise(ERR_LIB_CMP, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    *newWithNew = NULL;

    if ((req = OSSL_CMP_ITAV_new_rootCaCert(oldWithOld)) == NULL)
        return 0;
    itav = get_genm_itav(ctx, req, NID_id_it_rootCaKeyUpdate, "rootCaKeyUpdate");
    if (itav == NULL)
        return 0;

    if (!OSSL_CMP_ITAV_get0_rootCaKeyUpdate(itav, newWithNew,
                                            &my_newWithOld, &my_oldWithNew))
        goto end;
    /* no root CA cert update available */
    if (*newWithNew == NULL) {
        res = 1;
        goto end;
    }
    if ((oldWithOld_copy = X509_dup(oldWithOld)) == NULL && oldWithOld != NULL)
        goto end;
    if (!verify_ss_cert_trans(ctx, oldWithOld_copy, my_newWithOld,
                              *newWithNew, "newWithNew")) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ROOTCAKEYUPDATE);
        goto end;
    }
    if (oldWithOld != NULL && my_oldWithNew != NULL
        && !verify_ss_cert_trans(ctx, *newWithNew, my_oldWithNew,
                                 oldWithOld_copy, "oldWithOld")) {
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_ROOTCAKEYUPDATE);
        goto end;
    }

    if (!X509_up_ref(*newWithNew))
        goto end;
    if (newWithOld != NULL &&
        (*newWithOld = my_newWithOld) != NULL && !X509_up_ref(*newWithOld))
        goto free;
    if (oldWithNew == NULL ||
        (*oldWithNew = my_oldWithNew) == NULL || X509_up_ref(*oldWithNew)) {
        res = 1;
        goto end;
    }
    if (newWithOld != NULL)
        X509_free(*newWithOld);
 free:
    X509_free(*newWithNew);

 end:
    OSSL_CMP_ITAV_free(itav);
    X509_free(oldWithOld_copy);
    return res;
}

int OSSL_CMP_get1_crlUpdate(OSSL_CMP_CTX *ctx, const X509 *crlcert,
                            const X509_CRL *last_crl,
                            X509_CRL **crl)
{
    OSSL_CMP_CRLSTATUS *status = NULL;
    STACK_OF(OSSL_CMP_CRLSTATUS) *list = NULL;
    OSSL_CMP_ITAV *req = NULL, *itav = NULL;
    STACK_OF(X509_CRL) *crls = NULL;
    int res = 0;

    if (crl == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    *crl = NULL;

    if ((status = OSSL_CMP_CRLSTATUS_create(last_crl, crlcert, 1)) == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_GENERATE_CRLSTATUS);
        goto end;
    }
    if ((list = sk_OSSL_CMP_CRLSTATUS_new_reserve(NULL, 1)) == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_GENERATE_CRLSTATUS);
        goto end;
    }
    (void)sk_OSSL_CMP_CRLSTATUS_push(list, status); /* cannot fail */

    if ((req = OSSL_CMP_ITAV_new0_crlStatusList(list)) == NULL)
        goto end;
    status = NULL;
    list = NULL;

    if ((itav = get_genm_itav(ctx, req, NID_id_it_crls, "crl")) == NULL)
        goto end;

    if (!OSSL_CMP_ITAV_get0_crls(itav, &crls))
        goto end;

    if (crls == NULL) { /* no CRL update available */
        res = 1;
        goto end;
    }
    if (sk_X509_CRL_num(crls) != 1) {
        ERR_raise_data(ERR_LIB_CMP, CMP_R_INVALID_GENP,
                       "Unexpected number of CRLs in genp: %d",
                       sk_X509_CRL_num(crls));
        goto end;
    }

    if ((*crl = sk_X509_CRL_value(crls, 0)) == NULL || !X509_CRL_up_ref(*crl)) {
        *crl = NULL;
        goto end;
    }
    res = 1;
 end:
    OSSL_CMP_CRLSTATUS_free(status);
    sk_OSSL_CMP_CRLSTATUS_free(list);
    OSSL_CMP_ITAV_free(itav);
    return res;
}

int OSSL_CMP_get1_certReqTemplate(OSSL_CMP_CTX *ctx,
                                  OSSL_CRMF_CERTTEMPLATE **certTemplate,
                                  OSSL_CMP_ATAVS **keySpec)
{
    OSSL_CMP_ITAV *req, *itav = NULL;
    int res = 0;

    if (keySpec != NULL)
        *keySpec = NULL;
    if (certTemplate == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    *certTemplate = NULL;

    if ((req = OSSL_CMP_ITAV_new0_certReqTemplate(NULL, NULL)) == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_GENERATE_CERTREQTEMPLATE);
        return 0;
    }

    if ((itav = get_genm_itav(ctx, req, NID_id_it_certReqTemplate,
                              "certReqTemplate")) == NULL)
        return 0;

    if (!OSSL_CMP_ITAV_get1_certReqTemplate(itav, certTemplate, keySpec))
        goto end;

    res = 1;
 end:
    OSSL_CMP_ITAV_free(itav);
    return res;
}
