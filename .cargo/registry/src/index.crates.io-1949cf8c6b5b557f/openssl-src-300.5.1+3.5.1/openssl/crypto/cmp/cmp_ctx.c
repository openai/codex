/*
 * Copyright 2007-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/trace.h>
#include <openssl/bio.h>
#include <openssl/ocsp.h> /* for OCSP_REVOKED_STATUS_* */

#include "cmp_local.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <openssl/cmp.h>
#include <openssl/crmf.h>
#include <openssl/err.h>

#define DEFINE_OSSL_CMP_CTX_get0(FIELD, TYPE) \
    DEFINE_OSSL_CMP_CTX_get0_NAME(FIELD, FIELD, TYPE)
#define DEFINE_OSSL_CMP_CTX_get0_NAME(NAME, FIELD, TYPE) \
TYPE *OSSL_CMP_CTX_get0_##NAME(const OSSL_CMP_CTX *ctx) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return NULL; \
    } \
    return ctx->FIELD; \
}

/*
 * Get current certificate store containing trusted root CA certs
 */
DEFINE_OSSL_CMP_CTX_get0_NAME(trusted, trusted, X509_STORE)

#define DEFINE_OSSL_set0(PREFIX, FIELD, TYPE) \
    DEFINE_OSSL_set0_NAME(PREFIX, FIELD, FIELD, TYPE)
#define DEFINE_OSSL_set0_NAME(PREFIX, NAME, FIELD, TYPE) \
int PREFIX##_set0##_##NAME(OSSL_CMP_CTX *ctx, TYPE *val) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    TYPE##_free(ctx->FIELD); \
    ctx->FIELD = val; \
    return 1; \
}

/*
 * Set certificate store containing trusted (root) CA certs and possibly CRLs
 * and a cert verification callback function used for CMP server authentication.
 * Any already existing store entry is freed. Given NULL, the entry is reset.
 */
DEFINE_OSSL_set0_NAME(OSSL_CMP_CTX, trusted, trusted, X509_STORE)

DEFINE_OSSL_CMP_CTX_get0(libctx, OSSL_LIB_CTX)
DEFINE_OSSL_CMP_CTX_get0(propq, const char)

/* Get current list of non-trusted intermediate certs */
DEFINE_OSSL_CMP_CTX_get0(untrusted, STACK_OF(X509))

/*
 * Set untrusted certificates for path construction in authentication of
 * the CMP server and potentially others (TLS server, newly enrolled cert).
 */
int OSSL_CMP_CTX_set1_untrusted(OSSL_CMP_CTX *ctx, STACK_OF(X509) *certs)
{
    STACK_OF(X509) *untrusted = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (!ossl_x509_add_certs_new(&untrusted, certs,
                                 X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP))
        goto err;
    OSSL_STACK_OF_X509_free(ctx->untrusted);
    ctx->untrusted = untrusted;
    return 1;
 err:
    OSSL_STACK_OF_X509_free(untrusted);
    return 0;
}

static int cmp_ctx_set_md(OSSL_CMP_CTX *ctx, EVP_MD **pmd, int nid)
{
    EVP_MD *md = EVP_MD_fetch(ctx->libctx, OBJ_nid2sn(nid), ctx->propq);
    /* fetching in advance to be able to throw error early if unsupported */

    if (md == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_UNSUPPORTED_ALGORITHM);
        return 0;
    }
    EVP_MD_free(*pmd);
    *pmd = md;
    return 1;
}

/*
 * Allocates and initializes OSSL_CMP_CTX context structure with default values.
 * Returns new context on success, NULL on error
 */
OSSL_CMP_CTX *OSSL_CMP_CTX_new(OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_CMP_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx == NULL)
        goto err;

    ctx->libctx = libctx;
    if (propq != NULL && (ctx->propq = OPENSSL_strdup(propq)) == NULL)
        goto err;

    ctx->log_verbosity = OSSL_CMP_LOG_INFO;

    ctx->status = OSSL_CMP_PKISTATUS_unspecified;
    ctx->failInfoCode = -1;

    ctx->keep_alive = 1;
    ctx->msg_timeout = -1;
    ctx->tls_used = -1; /* default for backward compatibility */

    if ((ctx->untrusted = sk_X509_new_null()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }

    ctx->pbm_slen = 16;
    if (!cmp_ctx_set_md(ctx, &ctx->pbm_owf, NID_sha256))
        goto err;
    ctx->pbm_itercnt = 500;
    ctx->pbm_mac = NID_hmac_sha1;

    if (!cmp_ctx_set_md(ctx, &ctx->digest, NID_sha256))
        goto err;
    ctx->popoMethod = OSSL_CRMF_POPO_SIGNATURE;
    ctx->revocationReason = CRL_REASON_NONE;

    /* all other elements are initialized to 0 or NULL, respectively */
    return ctx;

 err:
    OSSL_CMP_CTX_free(ctx);
    return NULL;
}

#define OSSL_CMP_ITAVs_free(itavs) \
    sk_OSSL_CMP_ITAV_pop_free(itavs, OSSL_CMP_ITAV_free);
#define X509_EXTENSIONS_free(exts) \
    sk_X509_EXTENSION_pop_free(exts, X509_EXTENSION_free)
#define OSSL_CMP_PKIFREETEXT_free(text) \
    sk_ASN1_UTF8STRING_pop_free(text, ASN1_UTF8STRING_free)

/* Prepare the OSSL_CMP_CTX for next use, partly re-initializing OSSL_CMP_CTX */
int OSSL_CMP_CTX_reinit(OSSL_CMP_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

#ifndef OPENSSL_NO_HTTP
    if (ctx->http_ctx != NULL) {
        (void)OSSL_HTTP_close(ctx->http_ctx, 1);
        ossl_cmp_debug(ctx, "disconnected from CMP server");
        ctx->http_ctx = NULL;
    }
#endif
    ctx->status = OSSL_CMP_PKISTATUS_unspecified;
    ctx->failInfoCode = -1;

    OSSL_CMP_ITAVs_free(ctx->genm_ITAVs);
    ctx->genm_ITAVs = NULL;

    return ossl_cmp_ctx_set0_statusString(ctx, NULL)
        && ossl_cmp_ctx_set0_newCert(ctx, NULL)
        && ossl_cmp_ctx_set1_newChain(ctx, NULL)
        && ossl_cmp_ctx_set1_caPubs(ctx, NULL)
        && ossl_cmp_ctx_set1_extraCertsIn(ctx, NULL)
        && ossl_cmp_ctx_set1_validatedSrvCert(ctx, NULL)
        && ossl_cmp_ctx_set1_first_senderNonce(ctx, NULL)
        && OSSL_CMP_CTX_set1_transactionID(ctx, NULL)
        && OSSL_CMP_CTX_set1_senderNonce(ctx, NULL)
        && ossl_cmp_ctx_set1_recipNonce(ctx, NULL);
}

/* Frees OSSL_CMP_CTX variables allocated in OSSL_CMP_CTX_new() */
void OSSL_CMP_CTX_free(OSSL_CMP_CTX *ctx)
{
    if (ctx == NULL)
        return;

#ifndef OPENSSL_NO_HTTP
    if (ctx->http_ctx != NULL) {
        (void)OSSL_HTTP_close(ctx->http_ctx, 1);
        ossl_cmp_debug(ctx, "disconnected from CMP server");
    }
#endif
    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx->serverPath);
    OPENSSL_free(ctx->server);
    OPENSSL_free(ctx->proxy);
    OPENSSL_free(ctx->no_proxy);

    X509_free(ctx->srvCert);
    X509_free(ctx->validatedSrvCert);
    X509_NAME_free(ctx->expected_sender);
    X509_STORE_free(ctx->trusted);
    OSSL_STACK_OF_X509_free(ctx->untrusted);

    X509_free(ctx->cert);
    OSSL_STACK_OF_X509_free(ctx->chain);
    EVP_PKEY_free(ctx->pkey);
    ASN1_OCTET_STRING_free(ctx->referenceValue);
    if (ctx->secretValue != NULL)
        OPENSSL_cleanse(ctx->secretValue->data, ctx->secretValue->length);
    ASN1_OCTET_STRING_free(ctx->secretValue);
    EVP_MD_free(ctx->pbm_owf);

    X509_NAME_free(ctx->recipient);
    EVP_MD_free(ctx->digest);
    ASN1_OCTET_STRING_free(ctx->transactionID);
    ASN1_OCTET_STRING_free(ctx->senderNonce);
    ASN1_OCTET_STRING_free(ctx->recipNonce);
    ASN1_OCTET_STRING_free(ctx->first_senderNonce);
    OSSL_CMP_ITAVs_free(ctx->geninfo_ITAVs);
    OSSL_STACK_OF_X509_free(ctx->extraCertsOut);

    EVP_PKEY_free(ctx->newPkey);
    X509_NAME_free(ctx->issuer);
    ASN1_INTEGER_free(ctx->serialNumber);
    X509_NAME_free(ctx->subjectName);
    sk_GENERAL_NAME_pop_free(ctx->subjectAltNames, GENERAL_NAME_free);
    X509_EXTENSIONS_free(ctx->reqExtensions);
    sk_POLICYINFO_pop_free(ctx->policies, POLICYINFO_free);
    X509_free(ctx->oldCert);
    X509_REQ_free(ctx->p10CSR);

    OSSL_CMP_ITAVs_free(ctx->genm_ITAVs);

    OSSL_CMP_PKIFREETEXT_free(ctx->statusString);
    X509_free(ctx->newCert);
    OSSL_STACK_OF_X509_free(ctx->newChain);
    OSSL_STACK_OF_X509_free(ctx->caPubs);
    OSSL_STACK_OF_X509_free(ctx->extraCertsIn);

    OPENSSL_free(ctx);
}

#define DEFINE_OSSL_set(PREFIX, FIELD, TYPE) \
int PREFIX##_set_##FIELD(OSSL_CMP_CTX *ctx, TYPE val) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    ctx->FIELD = val; \
    return 1; \
}

DEFINE_OSSL_set(ossl_cmp_ctx, status, int)

#define DEFINE_OSSL_get(PREFIX, FIELD, TYPE, ERR_RET) \
TYPE PREFIX##_get_##FIELD(const OSSL_CMP_CTX *ctx) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return ERR_RET; \
    } \
    return ctx->FIELD; \
}

/*
 * Returns the PKIStatus from the last CertRepMessage
 * or Revocation Response or error message, -1 on error
 */
DEFINE_OSSL_get(OSSL_CMP_CTX, status, int, -1)

/*
 * Returns the statusString from the last CertRepMessage
 * or Revocation Response or error message, NULL on error
 */
DEFINE_OSSL_CMP_CTX_get0(statusString, OSSL_CMP_PKIFREETEXT)

DEFINE_OSSL_set0(ossl_cmp_ctx, statusString, OSSL_CMP_PKIFREETEXT)

/* Set callback function for checking if the cert is ok or should be rejected */
DEFINE_OSSL_set(OSSL_CMP_CTX, certConf_cb, OSSL_CMP_certConf_cb_t)

/*
 * Set argument, respectively a pointer to a structure containing arguments,
 * optionally to be used by the certConf callback.
 */
DEFINE_OSSL_set(OSSL_CMP_CTX, certConf_cb_arg, void *)

/*
 * Get argument, respectively the pointer to a structure containing arguments,
 * optionally to be used by certConf callback.
 * Returns callback argument set previously (NULL if not set or on error)
 */
DEFINE_OSSL_get(OSSL_CMP_CTX, certConf_cb_arg, void *, NULL)

#ifndef OPENSSL_NO_TRACE
static size_t ossl_cmp_log_trace_cb(const char *buf, size_t cnt,
                                    int category, int cmd, void *vdata)
{
    OSSL_CMP_CTX *ctx = vdata;
    const char *msg;
    OSSL_CMP_severity level = -1;
    char *func = NULL;
    char *file = NULL;
    int line = 0;

    if (buf == NULL || cnt == 0 || cmd != OSSL_TRACE_CTRL_WRITE || ctx == NULL)
        return 0;
    if (ctx->log_cb == NULL)
        return 1; /* silently drop message */

    msg = ossl_cmp_log_parse_metadata(buf, &level, &func, &file, &line);

    if (level > ctx->log_verbosity) /* excludes the case level is unknown */
        goto end; /* suppress output since severity is not sufficient */

    if (!ctx->log_cb(func != NULL ? func : "(no func)",
                     file != NULL ? file : "(no file)",
                     line, level, msg))
        cnt = 0;

 end:
    OPENSSL_free(func);
    OPENSSL_free(file);
    return cnt;
}
#endif

/* Print CMP log messages (i.e., diagnostic info) via the log cb of the ctx */
int ossl_cmp_print_log(OSSL_CMP_severity level, const OSSL_CMP_CTX *ctx,
                       const char *func, const char *file, int line,
                       const char *level_str, const char *format, ...)
{
    va_list args;
    char hugebuf[1024 * 2];
    int res = 0;

    if (ctx == NULL || ctx->log_cb == NULL)
        return 1; /* silently drop message */

    if (level > ctx->log_verbosity) /* excludes the case level is unknown */
        return 1; /* suppress output since severity is not sufficient */

    if (format == NULL)
        return 0;

    va_start(args, format);

    if (func == NULL)
        func = "(unset function name)";
    if (file == NULL)
        file = "(unset file name)";
    if (level_str == NULL)
        level_str = "(unset level string)";

#ifndef OPENSSL_NO_TRACE
    if (OSSL_TRACE_ENABLED(CMP)) {
        OSSL_TRACE_BEGIN(CMP) {
            int printed =
                BIO_snprintf(hugebuf, sizeof(hugebuf),
                             "%s:%s:%d:" OSSL_CMP_LOG_PREFIX "%s: ",
                             func, file, line, level_str);
            if (printed > 0 && (size_t)printed < sizeof(hugebuf)) {
                if (BIO_vsnprintf(hugebuf + printed,
                                  sizeof(hugebuf) - printed, format, args) > 0)
                    res = BIO_puts(trc_out, hugebuf) > 0;
            }
        } OSSL_TRACE_END(CMP);
    }
#else /* compensate for disabled trace API */
    {
        if (BIO_vsnprintf(hugebuf, sizeof(hugebuf), format, args) > 0)
            res = ctx->log_cb(func, file, line, level, hugebuf);
    }
#endif
    va_end(args);
    return res;
}

/* Set a callback function for error reporting and logging messages */
int OSSL_CMP_CTX_set_log_cb(OSSL_CMP_CTX *ctx, OSSL_CMP_log_cb_t cb)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    ctx->log_cb = cb;

#ifndef OPENSSL_NO_TRACE
    /* do also in case cb == NULL, to switch off logging output: */
    if (!OSSL_trace_set_callback(OSSL_TRACE_CATEGORY_CMP,
                                 ossl_cmp_log_trace_cb, ctx))
        return 0;
#endif

    return 1;
}

/* Print OpenSSL and CMP errors via the log cb of the ctx or ERR_print_errors */
void OSSL_CMP_CTX_print_errors(const OSSL_CMP_CTX *ctx)
{
    if (ctx != NULL && OSSL_CMP_LOG_ERR > ctx->log_verbosity)
        return; /* suppress output since severity is not sufficient */
    OSSL_CMP_print_errors_cb(ctx == NULL ? NULL : ctx->log_cb);
}

/*
 * Set or clear the reference value to be used for identification
 * (i.e., the user name) when using PBMAC.
 */
int OSSL_CMP_CTX_set1_referenceValue(OSSL_CMP_CTX *ctx,
                                     const unsigned char *ref, int len)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    return
        ossl_cmp_asn1_octet_string_set1_bytes(&ctx->referenceValue, ref, len);
}

/* Set or clear the password to be used for protecting messages with PBMAC */
int OSSL_CMP_CTX_set1_secretValue(OSSL_CMP_CTX *ctx,
                                  const unsigned char *sec, int len)
{
    ASN1_OCTET_STRING *secretValue = NULL;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    if (ossl_cmp_asn1_octet_string_set1_bytes(&secretValue, sec, len) != 1)
        return 0;
    if (ctx->secretValue != NULL) {
        OPENSSL_cleanse(ctx->secretValue->data, ctx->secretValue->length);
        ASN1_OCTET_STRING_free(ctx->secretValue);
    }
    ctx->secretValue = secretValue;
    return 1;
}

#define DEFINE_OSSL_CMP_CTX_get1_certs(FIELD) \
STACK_OF(X509) *OSSL_CMP_CTX_get1_##FIELD(const OSSL_CMP_CTX *ctx) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return NULL; \
    } \
    return X509_chain_up_ref(ctx->FIELD); \
}

/* Returns the cert chain computed by OSSL_CMP_certConf_cb(), NULL on error */
DEFINE_OSSL_CMP_CTX_get1_certs(newChain)

#define DEFINE_OSSL_set1_certs(PREFIX, FIELD) \
int PREFIX##_set1_##FIELD(OSSL_CMP_CTX *ctx, STACK_OF(X509) *certs) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    OSSL_STACK_OF_X509_free(ctx->FIELD); \
    ctx->FIELD = NULL; \
    return certs == NULL || (ctx->FIELD = X509_chain_up_ref(certs)) != NULL; \
}

/*
 * Copies any given stack of inbound X509 certificates to newChain
 * of the OSSL_CMP_CTX structure so that they may be retrieved later.
 */
DEFINE_OSSL_set1_certs(ossl_cmp_ctx, newChain)

/* Returns the stack of extraCerts received in CertRepMessage, NULL on error */
DEFINE_OSSL_CMP_CTX_get1_certs(extraCertsIn)

/*
 * Copies any given stack of inbound X509 certificates to extraCertsIn
 * of the OSSL_CMP_CTX structure so that they may be retrieved later.
 */
DEFINE_OSSL_set1_certs(ossl_cmp_ctx, extraCertsIn)

/*
 * Copies any given stack as the new stack of X509
 * certificates to send out in the extraCerts field.
 */
DEFINE_OSSL_set1_certs(OSSL_CMP_CTX, extraCertsOut)

/*
 * Add the given policy info object
 * to the X509_EXTENSIONS of the requested certificate template.
 */
int OSSL_CMP_CTX_push0_policy(OSSL_CMP_CTX *ctx, POLICYINFO *pinfo)
{
    if (ctx == NULL || pinfo == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (ctx->policies == NULL
            && (ctx->policies = CERTIFICATEPOLICIES_new()) == NULL)
        return 0;

    return sk_POLICYINFO_push(ctx->policies, pinfo);
}

/* Add an ITAV for geninfo of the PKI message header */
int OSSL_CMP_CTX_push0_geninfo_ITAV(OSSL_CMP_CTX *ctx, OSSL_CMP_ITAV *itav)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    return OSSL_CMP_ITAV_push0_stack_item(&ctx->geninfo_ITAVs, itav);
}

int OSSL_CMP_CTX_reset_geninfo_ITAVs(OSSL_CMP_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    OSSL_CMP_ITAVs_free(ctx->geninfo_ITAVs);
    ctx->geninfo_ITAVs = NULL;
    return 1;
}

DEFINE_OSSL_CMP_CTX_get0(geninfo_ITAVs, STACK_OF(OSSL_CMP_ITAV))

/* Add an itav for the body of outgoing general messages */
int OSSL_CMP_CTX_push0_genm_ITAV(OSSL_CMP_CTX *ctx, OSSL_CMP_ITAV *itav)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }
    return OSSL_CMP_ITAV_push0_stack_item(&ctx->genm_ITAVs, itav);
}

/*
 * Returns a duplicate of the stack of X509 certificates that
 * were received in the caPubs field of the last CertRepMessage.
 * Returns NULL on error
 */
DEFINE_OSSL_CMP_CTX_get1_certs(caPubs)

/*
 * Copies any given stack of certificates to the given
 * OSSL_CMP_CTX structure so that they may be retrieved later.
 */
DEFINE_OSSL_set1_certs(ossl_cmp_ctx, caPubs)

#define char_dup OPENSSL_strdup
#define char_free OPENSSL_free
#define DEFINE_OSSL_CMP_CTX_set1(FIELD, TYPE) /* this uses _dup */ \
int OSSL_CMP_CTX_set1_##FIELD(OSSL_CMP_CTX *ctx, const TYPE *val) \
{ \
    TYPE *val_dup = NULL; \
    \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    \
    if (val != NULL && (val_dup = TYPE##_dup(val)) == NULL) \
        return 0; \
    TYPE##_free(ctx->FIELD); \
    ctx->FIELD = val_dup; \
    return 1; \
}

#define X509_invalid(cert) (!ossl_x509v3_cache_extensions(cert))
#define EVP_PKEY_invalid(key) 0

#define DEFINE_OSSL_set1_up_ref(PREFIX, FIELD, TYPE) \
int PREFIX##_set1_##FIELD(OSSL_CMP_CTX *ctx, TYPE *val) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    \
    /* prevent misleading error later on malformed cert or provider issue */ \
    if (val != NULL && TYPE##_invalid(val)) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_POTENTIALLY_INVALID_CERTIFICATE); \
        return 0; \
    } \
    if (val != NULL && !TYPE##_up_ref(val)) \
        return 0; \
    TYPE##_free(ctx->FIELD); \
    ctx->FIELD = val; \
    return 1; \
}

DEFINE_OSSL_set1_up_ref(ossl_cmp_ctx, validatedSrvCert, X509)

/*
 * Pins the server certificate to be directly trusted (even if it is expired)
 * for verifying response messages.
 * Cert pointer is not consumed. It may be NULL to clear the entry.
 */
DEFINE_OSSL_set1_up_ref(OSSL_CMP_CTX, srvCert, X509)

/* Set the X509 name of the recipient to be placed in the PKIHeader */
DEFINE_OSSL_CMP_CTX_set1(recipient, X509_NAME)

/* Store the X509 name of the expected sender in the PKIHeader of responses */
DEFINE_OSSL_CMP_CTX_set1(expected_sender, X509_NAME)

/* Set the X509 name of the issuer to be placed in the certTemplate */
DEFINE_OSSL_CMP_CTX_set1(issuer, X509_NAME)

/* Set the ASN1_INTEGER serial to be placed in the certTemplate for rr */
DEFINE_OSSL_CMP_CTX_set1(serialNumber, ASN1_INTEGER)
/*
 * Set the subject name that will be placed in the certificate
 * request. This will be the subject name on the received certificate.
 */
DEFINE_OSSL_CMP_CTX_set1(subjectName, X509_NAME)

/* Set the X.509v3 certificate request extensions to be used in IR/CR/KUR */
int OSSL_CMP_CTX_set0_reqExtensions(OSSL_CMP_CTX *ctx, X509_EXTENSIONS *exts)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (sk_GENERAL_NAME_num(ctx->subjectAltNames) > 0 && exts != NULL
            && X509v3_get_ext_by_NID(exts, NID_subject_alt_name, -1) >= 0) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_SAN_SOURCES);
        return 0;
    }
    X509_EXTENSIONS_free(ctx->reqExtensions);
    ctx->reqExtensions = exts;
    return 1;
}

/* returns 1 if ctx contains a Subject Alternative Name extension, else 0 */
int OSSL_CMP_CTX_reqExtensions_have_SAN(OSSL_CMP_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return -1;
    }
    /* if one of the following conditions 'fail' this is not an error */
    return ctx->reqExtensions != NULL
        && X509v3_get_ext_by_NID(ctx->reqExtensions,
                                 NID_subject_alt_name, -1) >= 0;
}

/*
 * Add a GENERAL_NAME structure that will be added to the CRMF
 * request's extensions field to request subject alternative names.
 */
int OSSL_CMP_CTX_push1_subjectAltName(OSSL_CMP_CTX *ctx,
                                      const GENERAL_NAME *name)
{
    GENERAL_NAME *name_dup;

    if (ctx == NULL || name == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (OSSL_CMP_CTX_reqExtensions_have_SAN(ctx) == 1) {
        ERR_raise(ERR_LIB_CMP, CMP_R_MULTIPLE_SAN_SOURCES);
        return 0;
    }

    if (ctx->subjectAltNames == NULL
            && (ctx->subjectAltNames = sk_GENERAL_NAME_new_null()) == NULL)
        return 0;
    if ((name_dup = GENERAL_NAME_dup(name)) == NULL)
        return 0;
    if (!sk_GENERAL_NAME_push(ctx->subjectAltNames, name_dup)) {
        GENERAL_NAME_free(name_dup);
        return 0;
    }
    return 1;
}

/*
 * Set our own client certificate, used for example in KUR and when
 * doing the IR with existing certificate.
 */
DEFINE_OSSL_set1_up_ref(OSSL_CMP_CTX, cert, X509)

int OSSL_CMP_CTX_build_cert_chain(OSSL_CMP_CTX *ctx, X509_STORE *own_trusted,
                                  STACK_OF(X509) *candidates)
{
    STACK_OF(X509) *chain;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    if (!ossl_x509_add_certs_new(&ctx->untrusted, candidates,
                                 X509_ADD_FLAG_UP_REF | X509_ADD_FLAG_NO_DUP))
        return 0;

    ossl_cmp_debug(ctx, "trying to build chain for own CMP signer cert");
    chain = X509_build_chain(ctx->cert, ctx->untrusted, own_trusted, 0,
                             ctx->libctx, ctx->propq);
    if (chain == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_FAILED_BUILDING_OWN_CHAIN);
        return 0;
    }
    ossl_cmp_debug(ctx, "success building chain for own CMP signer cert");
    ctx->chain = chain;
    return 1;
}

/*
 * Set the old certificate that we are updating in KUR
 * or the certificate to be revoked in RR, respectively.
 * Also used as reference cert (defaulting to cert) for deriving subject DN
 * and SANs. Its issuer is used as default recipient in the CMP message header.
 */
DEFINE_OSSL_set1_up_ref(OSSL_CMP_CTX, oldCert, X509)

/* Set the PKCS#10 CSR to be sent in P10CR */
DEFINE_OSSL_CMP_CTX_set1(p10CSR, X509_REQ)

/*
 * Set the (newly received in IP/KUP/CP) certificate in the context.
 * This only permits for one cert to be enrolled at a time.
 */
DEFINE_OSSL_set0(ossl_cmp_ctx, newCert, X509)

/* Get successfully validated server cert, if any, of current transaction */
DEFINE_OSSL_CMP_CTX_get0(validatedSrvCert, X509)

/*
 * Get the (newly received in IP/KUP/CP) client certificate from the context
 * This only permits for one client cert to be received...
 */
DEFINE_OSSL_CMP_CTX_get0(newCert, X509)

/* Set the client's current private key */
DEFINE_OSSL_set1_up_ref(OSSL_CMP_CTX, pkey, EVP_PKEY)

/* Set new key pair. Used e.g. when doing Key Update */
int OSSL_CMP_CTX_set0_newPkey(OSSL_CMP_CTX *ctx, int priv, EVP_PKEY *pkey)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    EVP_PKEY_free(ctx->newPkey);
    ctx->newPkey = pkey;
    ctx->newPkey_priv = priv;
    return 1;
}

/* Get the private/public key to use for cert enrollment, or NULL on error */
/* In case |priv| == 0, better use ossl_cmp_ctx_get0_newPubkey() below */
EVP_PKEY *OSSL_CMP_CTX_get0_newPkey(const OSSL_CMP_CTX *ctx, int priv)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }

    if (ctx->newPkey != NULL)
        return priv && !ctx->newPkey_priv ? NULL : ctx->newPkey;
    if (ctx->p10CSR != NULL)
        return priv ? NULL : X509_REQ_get0_pubkey(ctx->p10CSR);
    return ctx->pkey; /* may be NULL */
}

EVP_PKEY *ossl_cmp_ctx_get0_newPubkey(const OSSL_CMP_CTX *ctx)
{
    if (!ossl_assert(ctx != NULL))
        return NULL;
    if (ctx->newPkey != NULL)
        return ctx->newPkey;
    if (ctx->p10CSR != NULL)
        return X509_REQ_get0_pubkey(ctx->p10CSR);
    if (ctx->oldCert != NULL)
        return X509_get0_pubkey(ctx->oldCert);
    if (ctx->cert != NULL)
        return X509_get0_pubkey(ctx->cert);
    return ctx->pkey;
}

#define DEFINE_set1_ASN1_OCTET_STRING(PREFIX, FIELD) \
int PREFIX##_set1_##FIELD(OSSL_CMP_CTX *ctx, const ASN1_OCTET_STRING *id) \
{ \
    if (ctx == NULL) { \
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT); \
        return 0; \
    } \
    return ossl_cmp_asn1_octet_string_set1(&ctx->FIELD, id); \
}

/* Set the given transactionID to the context */
DEFINE_set1_ASN1_OCTET_STRING(OSSL_CMP_CTX, transactionID)

/* Set the nonce to be used for the recipNonce in the message created next */
DEFINE_set1_ASN1_OCTET_STRING(ossl_cmp_ctx, recipNonce)

/* Stores the given nonce as the last senderNonce sent out */
DEFINE_set1_ASN1_OCTET_STRING(OSSL_CMP_CTX, senderNonce)

/* store the first req sender nonce for verifying delayed delivery */
DEFINE_set1_ASN1_OCTET_STRING(ossl_cmp_ctx, first_senderNonce)

/* Set the proxy server to use for HTTP(S) connections */
DEFINE_OSSL_CMP_CTX_set1(proxy, char)

/* Set the (HTTP) hostname of the CMP server */
DEFINE_OSSL_CMP_CTX_set1(server, char)

/* Set the server exclusion list of the HTTP proxy server */
DEFINE_OSSL_CMP_CTX_set1(no_proxy, char)

#ifndef OPENSSL_NO_HTTP
/* Set the http connect/disconnect callback function to be used for HTTP(S) */
DEFINE_OSSL_set(OSSL_CMP_CTX, http_cb, OSSL_HTTP_bio_cb_t)

/* Set argument optionally to be used by the http connect/disconnect callback */
DEFINE_OSSL_set(OSSL_CMP_CTX, http_cb_arg, void *)

/*
 * Get argument optionally to be used by the http connect/disconnect callback
 * Returns callback argument set previously (NULL if not set or on error)
 */
DEFINE_OSSL_get(OSSL_CMP_CTX, http_cb_arg, void *, NULL)
#endif

/* Set callback function for sending CMP request and receiving response */
DEFINE_OSSL_set(OSSL_CMP_CTX, transfer_cb, OSSL_CMP_transfer_cb_t)

/* Set argument optionally to be used by the transfer callback */
DEFINE_OSSL_set(OSSL_CMP_CTX, transfer_cb_arg, void *)

/*
 * Get argument optionally to be used by the transfer callback.
 * Returns callback argument set previously (NULL if not set or on error)
 */
DEFINE_OSSL_get(OSSL_CMP_CTX, transfer_cb_arg, void *, NULL)

/** Set the HTTP server port to be used */
DEFINE_OSSL_set(OSSL_CMP_CTX, serverPort, int)

/* Set the HTTP path to be used on the server (e.g "pkix/") */
DEFINE_OSSL_CMP_CTX_set1(serverPath, char)

/* Set the failInfo error code as bit encoding in OSSL_CMP_CTX */
DEFINE_OSSL_set(ossl_cmp_ctx, failInfoCode, int)

/*
 * Get the failInfo error code in OSSL_CMP_CTX as bit encoding.
 * Returns bit string as integer on success, -1 on error
 */
DEFINE_OSSL_get(OSSL_CMP_CTX, failInfoCode, int, -1)

/* Set a Boolean or integer option of the context to the "val" arg */
int OSSL_CMP_CTX_set_option(OSSL_CMP_CTX *ctx, int opt, int val)
{
    int min_val;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return 0;
    }

    switch (opt) {
    case OSSL_CMP_OPT_REVOCATION_REASON:
        min_val = OCSP_REVOKED_STATUS_NOSTATUS;
        break;
    case OSSL_CMP_OPT_POPO_METHOD:
        min_val = OSSL_CRMF_POPO_NONE;
        break;
    default:
        min_val = 0;
        break;
    }
    if (val < min_val) {
        ERR_raise(ERR_LIB_CMP, CMP_R_VALUE_TOO_SMALL);
        return 0;
    }

    switch (opt) {
    case OSSL_CMP_OPT_LOG_VERBOSITY:
        if (val > OSSL_CMP_LOG_MAX) {
            ERR_raise(ERR_LIB_CMP, CMP_R_VALUE_TOO_LARGE);
            return 0;
        }
        ctx->log_verbosity = val;
        break;
    case OSSL_CMP_OPT_IMPLICIT_CONFIRM:
        ctx->implicitConfirm = val;
        break;
    case OSSL_CMP_OPT_DISABLE_CONFIRM:
        ctx->disableConfirm = val;
        break;
    case OSSL_CMP_OPT_UNPROTECTED_SEND:
        ctx->unprotectedSend = val;
        break;
    case OSSL_CMP_OPT_UNPROTECTED_ERRORS:
        ctx->unprotectedErrors = val;
        break;
    case OSSL_CMP_OPT_NO_CACHE_EXTRACERTS:
        ctx->noCacheExtraCerts = val;
        break;
    case OSSL_CMP_OPT_VALIDITY_DAYS:
        ctx->days = val;
        break;
    case OSSL_CMP_OPT_SUBJECTALTNAME_NODEFAULT:
        ctx->SubjectAltName_nodefault = val;
        break;
    case OSSL_CMP_OPT_SUBJECTALTNAME_CRITICAL:
        ctx->setSubjectAltNameCritical = val;
        break;
    case OSSL_CMP_OPT_POLICIES_CRITICAL:
        ctx->setPoliciesCritical = val;
        break;
    case OSSL_CMP_OPT_IGNORE_KEYUSAGE:
        ctx->ignore_keyusage = val;
        break;
    case OSSL_CMP_OPT_POPO_METHOD:
        if (val > OSSL_CRMF_POPO_KEYAGREE) {
            ERR_raise(ERR_LIB_CMP, CMP_R_VALUE_TOO_LARGE);
            return 0;
        }
        ctx->popoMethod = val;
        break;
    case OSSL_CMP_OPT_DIGEST_ALGNID:
        if (!cmp_ctx_set_md(ctx, &ctx->digest, val))
            return 0;
        break;
    case OSSL_CMP_OPT_OWF_ALGNID:
        if (!cmp_ctx_set_md(ctx, &ctx->pbm_owf, val))
            return 0;
        break;
    case OSSL_CMP_OPT_MAC_ALGNID:
        ctx->pbm_mac = val;
        break;
    case OSSL_CMP_OPT_KEEP_ALIVE:
        ctx->keep_alive = val;
        break;
    case OSSL_CMP_OPT_MSG_TIMEOUT:
        ctx->msg_timeout = val;
        break;
    case OSSL_CMP_OPT_TOTAL_TIMEOUT:
        ctx->total_timeout = val;
        break;
    case OSSL_CMP_OPT_USE_TLS:
        ctx->tls_used = val;
        break;
    case OSSL_CMP_OPT_PERMIT_TA_IN_EXTRACERTS_FOR_IR:
        ctx->permitTAInExtraCertsForIR = val;
        break;
    case OSSL_CMP_OPT_REVOCATION_REASON:
        if (val > OCSP_REVOKED_STATUS_AACOMPROMISE) {
            ERR_raise(ERR_LIB_CMP, CMP_R_VALUE_TOO_LARGE);
            return 0;
        }
        ctx->revocationReason = val;
        break;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_OPTION);
        return 0;
    }

    return 1;
}

/*
 * Reads a Boolean or integer option value from the context.
 * Returns -1 on error (which is the default OSSL_CMP_OPT_REVOCATION_REASON)
 */
int OSSL_CMP_CTX_get_option(const OSSL_CMP_CTX *ctx, int opt)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return -1;
    }

    switch (opt) {
    case OSSL_CMP_OPT_LOG_VERBOSITY:
        return ctx->log_verbosity;
    case OSSL_CMP_OPT_IMPLICIT_CONFIRM:
        return ctx->implicitConfirm;
    case OSSL_CMP_OPT_DISABLE_CONFIRM:
        return ctx->disableConfirm;
    case OSSL_CMP_OPT_UNPROTECTED_SEND:
        return ctx->unprotectedSend;
    case OSSL_CMP_OPT_UNPROTECTED_ERRORS:
        return ctx->unprotectedErrors;
    case OSSL_CMP_OPT_NO_CACHE_EXTRACERTS:
        return ctx->noCacheExtraCerts;
    case OSSL_CMP_OPT_VALIDITY_DAYS:
        return ctx->days;
    case OSSL_CMP_OPT_SUBJECTALTNAME_NODEFAULT:
        return ctx->SubjectAltName_nodefault;
    case OSSL_CMP_OPT_SUBJECTALTNAME_CRITICAL:
        return ctx->setSubjectAltNameCritical;
    case OSSL_CMP_OPT_POLICIES_CRITICAL:
        return ctx->setPoliciesCritical;
    case OSSL_CMP_OPT_IGNORE_KEYUSAGE:
        return ctx->ignore_keyusage;
    case OSSL_CMP_OPT_POPO_METHOD:
        return ctx->popoMethod;
    case OSSL_CMP_OPT_DIGEST_ALGNID:
        return EVP_MD_get_type(ctx->digest);
    case OSSL_CMP_OPT_OWF_ALGNID:
        return EVP_MD_get_type(ctx->pbm_owf);
    case OSSL_CMP_OPT_MAC_ALGNID:
        return ctx->pbm_mac;
    case OSSL_CMP_OPT_KEEP_ALIVE:
        return ctx->keep_alive;
    case OSSL_CMP_OPT_MSG_TIMEOUT:
        return ctx->msg_timeout;
    case OSSL_CMP_OPT_TOTAL_TIMEOUT:
        return ctx->total_timeout;
    case OSSL_CMP_OPT_USE_TLS:
        return ctx->tls_used;
    case OSSL_CMP_OPT_PERMIT_TA_IN_EXTRACERTS_FOR_IR:
        return ctx->permitTAInExtraCertsForIR;
    case OSSL_CMP_OPT_REVOCATION_REASON:
        return ctx->revocationReason;
    default:
        ERR_raise(ERR_LIB_CMP, CMP_R_INVALID_OPTION);
        return -1;
    }
}
