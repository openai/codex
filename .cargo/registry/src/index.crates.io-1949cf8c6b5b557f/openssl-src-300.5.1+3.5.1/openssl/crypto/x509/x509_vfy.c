/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <stdio.h>
#include <time.h>
#include <errno.h>
#include <limits.h>

#include "crypto/ctype.h"
#include "internal/cryptlib.h"
#include <openssl/crypto.h>
#include <openssl/buffer.h>
#include <openssl/evp.h>
#include <openssl/asn1.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/objects.h>
#include <openssl/core_names.h>
#include "internal/dane.h"
#include "crypto/x509.h"
#include "x509_local.h"

/* CRL score values */

#define CRL_SCORE_NOCRITICAL    0x100 /* No unhandled critical extensions */
#define CRL_SCORE_SCOPE         0x080 /* certificate is within CRL scope */
#define CRL_SCORE_TIME          0x040 /* CRL times valid */
#define CRL_SCORE_ISSUER_NAME   0x020 /* Issuer name matches certificate */
#define CRL_SCORE_VALID /* If this score or above CRL is probably valid */ \
    (CRL_SCORE_NOCRITICAL | CRL_SCORE_TIME | CRL_SCORE_SCOPE)
#define CRL_SCORE_ISSUER_CERT   0x018 /* CRL issuer is certificate issuer */
#define CRL_SCORE_SAME_PATH     0x008 /* CRL issuer is on certificate path */
#define CRL_SCORE_AKID          0x004 /* CRL issuer matches CRL AKID */
#define CRL_SCORE_TIME_DELTA    0x002 /* Have a delta CRL with valid times */

static int x509_verify_x509(X509_STORE_CTX *ctx);
static int x509_verify_rpk(X509_STORE_CTX *ctx);
static int build_chain(X509_STORE_CTX *ctx);
static int verify_chain(X509_STORE_CTX *ctx);
static int verify_rpk(X509_STORE_CTX *ctx);
static int dane_verify(X509_STORE_CTX *ctx);
static int dane_verify_rpk(X509_STORE_CTX *ctx);
static int null_callback(int ok, X509_STORE_CTX *e);
static int check_issued(X509_STORE_CTX *ctx, X509 *x, X509 *issuer);
static int check_extensions(X509_STORE_CTX *ctx);
static int check_name_constraints(X509_STORE_CTX *ctx);
static int check_id(X509_STORE_CTX *ctx);
static int check_trust(X509_STORE_CTX *ctx, int num_untrusted);
static int check_revocation(X509_STORE_CTX *ctx);
static int check_cert(X509_STORE_CTX *ctx);
static int check_policy(X509_STORE_CTX *ctx);
static int check_dane_issuer(X509_STORE_CTX *ctx, int depth);
static int check_cert_key_level(X509_STORE_CTX *ctx, X509 *cert);
static int check_key_level(X509_STORE_CTX *ctx, EVP_PKEY *pkey);
static int check_sig_level(X509_STORE_CTX *ctx, X509 *cert);
static int check_curve(X509 *cert);

static int get_crl_score(X509_STORE_CTX *ctx, X509 **pissuer,
                         unsigned int *preasons, X509_CRL *crl, X509 *x);
static int get_crl_delta(X509_STORE_CTX *ctx,
                         X509_CRL **pcrl, X509_CRL **pdcrl, X509 *x);
static void get_delta_sk(X509_STORE_CTX *ctx, X509_CRL **dcrl,
                         int *pcrl_score, X509_CRL *base,
                         STACK_OF(X509_CRL) *crls);
static void crl_akid_check(X509_STORE_CTX *ctx, X509_CRL *crl, X509 **pissuer,
                           int *pcrl_score);
static int crl_crldp_check(X509 *x, X509_CRL *crl, int crl_score,
                           unsigned int *preasons);
static int check_crl_path(X509_STORE_CTX *ctx, X509 *x);
static int check_crl_chain(X509_STORE_CTX *ctx,
                           STACK_OF(X509) *cert_path,
                           STACK_OF(X509) *crl_path);

static int internal_verify(X509_STORE_CTX *ctx);

static int null_callback(int ok, X509_STORE_CTX *e)
{
    return ok;
}

/*-
 * Return 1 if given cert is considered self-signed, 0 if not, or -1 on error.
 * This actually verifies self-signedness only if requested.
 * It calls ossl_x509v3_cache_extensions()
 * to match issuer and subject names (i.e., the cert being self-issued) and any
 * present authority key identifier to match the subject key identifier, etc.
 */
int X509_self_signed(X509 *cert, int verify_signature)
{
    EVP_PKEY *pkey;

    if ((pkey = X509_get0_pubkey(cert)) == NULL) { /* handles cert == NULL */
        ERR_raise(ERR_LIB_X509, X509_R_UNABLE_TO_GET_CERTS_PUBLIC_KEY);
        return -1;
    }
    if (!ossl_x509v3_cache_extensions(cert))
        return -1;
    if ((cert->ex_flags & EXFLAG_SS) == 0)
        return 0;
    if (!verify_signature)
        return 1;
    return X509_verify(cert, pkey);
}

/*
 * Given a certificate, try and find an exact match in the store.
 * Returns 1 on success, 0 on not found, -1 on internal error.
 */
static int lookup_cert_match(X509 **result, X509_STORE_CTX *ctx, X509 *x)
{
    STACK_OF(X509) *certs;
    X509 *xtmp = NULL;
    int i, ret;

    *result = NULL;
    /* Lookup all certs with matching subject name */
    ERR_set_mark();
    certs = ctx->lookup_certs(ctx, X509_get_subject_name(x));
    ERR_pop_to_mark();
    if (certs == NULL)
        return -1;

    /* Look for exact match */
    for (i = 0; i < sk_X509_num(certs); i++) {
        xtmp = sk_X509_value(certs, i);
        if (X509_cmp(xtmp, x) == 0)
            break;
        xtmp = NULL;
    }
    ret = xtmp != NULL;
    if (ret) {
        if (!X509_up_ref(xtmp))
            ret = -1;
        else
            *result = xtmp;
    }
    OSSL_STACK_OF_X509_free(certs);
    return ret;
}

/*-
 * Inform the verify callback of an error.
 * The error code is set to |err| if |err| is not X509_V_OK, else
 * |ctx->error| is left unchanged (under the assumption it is set elsewhere).
 * The error depth is |depth| if >= 0, else it defaults to |ctx->error_depth|.
 * The error cert is |x| if not NULL, else the cert in |ctx->chain| at |depth|.
 *
 * Returns 0 to abort verification with an error, non-zero to continue.
 */
static int verify_cb_cert(X509_STORE_CTX *ctx, X509 *x, int depth, int err)
{
    if (depth < 0)
        depth = ctx->error_depth;
    else
        ctx->error_depth = depth;
    ctx->current_cert = x != NULL ? x : sk_X509_value(ctx->chain, depth);
    if (err != X509_V_OK)
        ctx->error = err;
    return ctx->verify_cb(0, ctx);
}

#define CB_FAIL_IF(cond, ctx, cert, depth, err) \
    if ((cond) && verify_cb_cert(ctx, cert, depth, err) == 0) \
        return 0

/*-
 * Inform the verify callback of an error, CRL-specific variant.  Here, the
 * error depth and certificate are already set, we just specify the error
 * number.
 *
 * Returns 0 to abort verification with an error, non-zero to continue.
 */
static int verify_cb_crl(X509_STORE_CTX *ctx, int err)
{
    ctx->error = err;
    return ctx->verify_cb(0, ctx);
}

/* Sadly, returns 0 also on internal error in ctx->verify_cb(). */
static int check_auth_level(X509_STORE_CTX *ctx)
{
    int i;
    int num = sk_X509_num(ctx->chain);

    if (ctx->param->auth_level <= 0)
        return 1;

    for (i = 0; i < num; ++i) {
        X509 *cert = sk_X509_value(ctx->chain, i);

        /*
         * We've already checked the security of the leaf key, so here we only
         * check the security of issuer keys.
         */
        CB_FAIL_IF(i > 0 && !check_cert_key_level(ctx, cert),
                   ctx, cert, i, X509_V_ERR_CA_KEY_TOO_SMALL);
        /*
         * We also check the signature algorithm security of all certificates
         * except those of the trust anchor at index num-1.
         */
        CB_FAIL_IF(i < num - 1 && !check_sig_level(ctx, cert),
                   ctx, cert, i, X509_V_ERR_CA_MD_TOO_WEAK);
    }
    return 1;
}

/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int verify_rpk(X509_STORE_CTX *ctx)
{
    /* Not much to verify on a RPK */
    if (ctx->verify != NULL)
        return ctx->verify(ctx);

    return !!ctx->verify_cb(ctx->error == X509_V_OK, ctx);
}


/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int verify_chain(X509_STORE_CTX *ctx)
{
    int err;
    int ok;

    if ((ok = build_chain(ctx)) <= 0
        || (ok = check_extensions(ctx)) <= 0
        || (ok = check_auth_level(ctx)) <= 0
        || (ok = check_id(ctx)) <= 0
        || (ok = X509_get_pubkey_parameters(NULL, ctx->chain) ? 1 : -1) <= 0
        || (ok = ctx->check_revocation(ctx)) <= 0)
        return ok;

    err = X509_chain_check_suiteb(&ctx->error_depth, NULL, ctx->chain,
                                  ctx->param->flags);
    CB_FAIL_IF(err != X509_V_OK, ctx, NULL, ctx->error_depth, err);

    /* Verify chain signatures and expiration times */
    ok = ctx->verify != NULL ? ctx->verify(ctx) : internal_verify(ctx);
    if (ok <= 0)
        return ok;

    if ((ok = check_name_constraints(ctx)) <= 0)
        return ok;

#ifndef OPENSSL_NO_RFC3779
    /* RFC 3779 path validation, now that CRL check has been done */
    if ((ok = X509v3_asid_validate_path(ctx)) <= 0)
        return ok;
    if ((ok = X509v3_addr_validate_path(ctx)) <= 0)
        return ok;
#endif

    /* If we get this far evaluate policies */
    if ((ctx->param->flags & X509_V_FLAG_POLICY_CHECK) != 0)
        ok = ctx->check_policy(ctx);
    return ok;
}

int X509_STORE_CTX_verify(X509_STORE_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    if (ctx->rpk != NULL)
        return x509_verify_rpk(ctx);
    if (ctx->cert == NULL && sk_X509_num(ctx->untrusted) >= 1)
        ctx->cert = sk_X509_value(ctx->untrusted, 0);
    return x509_verify_x509(ctx);
}

int X509_verify_cert(X509_STORE_CTX *ctx)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
    }
    return (ctx->rpk != NULL) ? x509_verify_rpk(ctx) : x509_verify_x509(ctx);
}

/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int x509_verify_rpk(X509_STORE_CTX *ctx)
{
    int ret;

    /* If the peer's public key is too weak, we can stop early. */
    if (!check_key_level(ctx, ctx->rpk)
        && verify_cb_cert(ctx, NULL, 0, X509_V_ERR_EE_KEY_TOO_SMALL) == 0)
        return 0;

    /* Barring any data to verify the RPK, simply report it as untrusted */
    ctx->error = X509_V_ERR_RPK_UNTRUSTED;

    ret = DANETLS_ENABLED(ctx->dane) ? dane_verify_rpk(ctx) : verify_rpk(ctx);

    /*
     * Safety-net.  If we are returning an error, we must also set ctx->error,
     * so that the chain is not considered verified should the error be ignored
     * (e.g. TLS with SSL_VERIFY_NONE).
     */
    if (ret <= 0 && ctx->error == X509_V_OK)
        ctx->error = X509_V_ERR_UNSPECIFIED;
    return ret;
}

/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int x509_verify_x509(X509_STORE_CTX *ctx)
{
    int ret;

    if (ctx->cert == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_NO_CERT_SET_FOR_US_TO_VERIFY);
        ctx->error = X509_V_ERR_INVALID_CALL;
        return -1;
    }

    if (ctx->chain != NULL) {
        /*
         * This X509_STORE_CTX has already been used to verify a cert. We
         * cannot do another one.
         */
        ERR_raise(ERR_LIB_X509, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        ctx->error = X509_V_ERR_INVALID_CALL;
        return -1;
    }

    if (!ossl_x509_add_cert_new(&ctx->chain, ctx->cert, X509_ADD_FLAG_UP_REF)) {
        ctx->error = X509_V_ERR_OUT_OF_MEM;
        return -1;
    }
    ctx->num_untrusted = 1;

    /* If the peer's public key is too weak, we can stop early. */
    CB_FAIL_IF(!check_cert_key_level(ctx, ctx->cert),
               ctx, ctx->cert, 0, X509_V_ERR_EE_KEY_TOO_SMALL);

    ret = DANETLS_ENABLED(ctx->dane) ? dane_verify(ctx) : verify_chain(ctx);

    /*
     * Safety-net.  If we are returning an error, we must also set ctx->error,
     * so that the chain is not considered verified should the error be ignored
     * (e.g. TLS with SSL_VERIFY_NONE).
     */
    if (ret <= 0 && ctx->error == X509_V_OK)
        ctx->error = X509_V_ERR_UNSPECIFIED;
    return ret;
}

static int sk_X509_contains(STACK_OF(X509) *sk, X509 *cert)
{
    int i, n = sk_X509_num(sk);

    for (i = 0; i < n; i++)
        if (X509_cmp(sk_X509_value(sk, i), cert) == 0)
            return 1;
    return 0;
}

/*-
 * Find in |sk| an issuer cert of cert |x| accepted by |ctx->check_issued|.
 * If no_dup, the issuer must not yet be in |ctx->chain|, yet allowing the
 *     exception that |x| is self-issued and |ctx->chain| has just one element.
 * Prefer the first match with suitable validity period or latest expiration.
 */
/*
 * Note: so far, we do not check during chain building
 * whether any key usage extension stands against a candidate issuer cert.
 * Likely it would be good if build_chain() sets |check_signing_allowed|.
 * Yet if |sk| is a list of trusted certs, as with X509_STORE_CTX_set0_trusted_stack(),
 * better not set |check_signing_allowed|.
 * Maybe not touch X509_STORE_CTX_get1_issuer(), for API backward compatiblity.
 */
static X509 *get0_best_issuer_sk(X509_STORE_CTX *ctx, int check_signing_allowed,
                                 int no_dup, STACK_OF(X509) *sk, X509 *x)
{
    int i;
    X509 *candidate, *issuer = NULL;

    for (i = 0; i < sk_X509_num(sk); i++) {
        candidate = sk_X509_value(sk, i);
        if (no_dup
            && !((x->ex_flags & EXFLAG_SI) != 0 && sk_X509_num(ctx->chain) == 1)
            && sk_X509_contains(ctx->chain, candidate))
            continue;
        if (ctx->check_issued(ctx, x, candidate)) {
            if (check_signing_allowed
                /* yet better not check key usage for trust anchors */
                && ossl_x509_signing_allowed(candidate, x) != X509_V_OK)
                continue;
            if (ossl_x509_check_cert_time(ctx, candidate, -1))
                return candidate;
            /*
             * Leave in *issuer the first match that has the latest expiration
             * date so we return nearest match if no certificate time is OK.
             */
            if (issuer == NULL
                    || ASN1_TIME_compare(X509_get0_notAfter(candidate),
                                         X509_get0_notAfter(issuer)) > 0)
                issuer = candidate;
        }
    }
    return issuer;
}

/*-
 * Try to get issuer cert from |ctx->store| accepted by |ctx->check_issued|.
 * Prefer the first match with suitable validity period or latest expiration.
 *
 * Return values are:
 *  1 lookup successful.
 *  0 certificate not found.
 * -1 some other error.
 */
int X509_STORE_CTX_get1_issuer(X509 **issuer, X509_STORE_CTX *ctx, X509 *x)
{
    const X509_NAME *xn = X509_get_issuer_name(x);
    X509_OBJECT *obj = X509_OBJECT_new();
    STACK_OF(X509) *certs;
    int ret;

    *issuer = NULL;
    if (obj == NULL)
        return -1;
    ret = ossl_x509_store_ctx_get_by_subject(ctx, X509_LU_X509, xn, obj);
    if (ret != 1)
        goto end;

    /* quick happy path: certificate matches and is currently valid */
    if (ctx->check_issued(ctx, x, obj->data.x509)) {
        if (ossl_x509_check_cert_time(ctx, obj->data.x509, -1)) {
            *issuer = obj->data.x509;
            /* |*issuer| has taken over the cert reference from |obj| */
            obj->type = X509_LU_NONE;
            goto end;
        }
    }

    ret = -1;
    if ((certs = X509_STORE_CTX_get1_certs(ctx, xn)) == NULL)
        goto end;
    *issuer = get0_best_issuer_sk(ctx, 0, 0 /* allow duplicates */, certs, x);
    ret = 0;
    if (*issuer != NULL)
        ret = X509_up_ref(*issuer) ? 1 : -1;
    OSSL_STACK_OF_X509_free(certs);
 end:
    X509_OBJECT_free(obj);
    return ret;
}

/* Check that the given certificate |x| is issued by the certificate |issuer| */
static int check_issued(ossl_unused X509_STORE_CTX *ctx, X509 *x, X509 *issuer)
{
    int err = ossl_x509_likely_issued(issuer, x);

    if (err == X509_V_OK)
        return 1;
    /*
     * SUBJECT_ISSUER_MISMATCH just means 'x' is clearly not issued by 'issuer'.
     * Every other error code likely indicates a real error.
     */
    return 0;
}

/*-
 * Alternative get_issuer method: look up from a STACK_OF(X509) in other_ctx.
 * Returns -1 on internal error.
 */
static int get1_best_issuer_other_sk(X509 **issuer, X509_STORE_CTX *ctx, X509 *x)
{
    *issuer = get0_best_issuer_sk(ctx, 0, 1 /* no_dup */, ctx->other_ctx, x);
    if (*issuer == NULL)
        return 0;
    return X509_up_ref(*issuer) ? 1 : -1;
}

/*-
 * Alternative lookup method: look from a STACK stored in other_ctx.
 * Returns NULL on internal/fatal error, empty stack if not found.
 */
static STACK_OF(X509) *lookup_certs_sk(X509_STORE_CTX *ctx, const X509_NAME *nm)
{
    STACK_OF(X509) *sk = sk_X509_new_null();
    X509 *x;
    int i;

    if (sk == NULL)
        return NULL;
    for (i = 0; i < sk_X509_num(ctx->other_ctx); i++) {
        x = sk_X509_value(ctx->other_ctx, i);
        if (X509_NAME_cmp(nm, X509_get_subject_name(x)) == 0) {
            if (!X509_add_cert(sk, x, X509_ADD_FLAG_UP_REF)) {
                OSSL_STACK_OF_X509_free(sk);
                ctx->error = X509_V_ERR_OUT_OF_MEM;
                return NULL;
            }
        }
    }
    return sk;
}

/*
 * Check EE or CA certificate purpose.  For trusted certificates explicit local
 * auxiliary trust can be used to override EKU-restrictions.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int check_purpose(X509_STORE_CTX *ctx, X509 *x, int purpose, int depth,
                         int must_be_ca)
{
    int tr_ok = X509_TRUST_UNTRUSTED;

    /*
     * For trusted certificates we want to see whether any auxiliary trust
     * settings trump the purpose constraints.
     *
     * This is complicated by the fact that the trust ordinals in
     * ctx->param->trust are entirely independent of the purpose ordinals in
     * ctx->param->purpose!
     *
     * What connects them is their mutual initialization via calls from
     * X509_STORE_CTX_set_default() into X509_VERIFY_PARAM_lookup() which sets
     * related values of both param->trust and param->purpose.  It is however
     * typically possible to infer associated trust values from a purpose value
     * via the X509_PURPOSE API.
     *
     * Therefore, we can only check for trust overrides when the purpose we're
     * checking is the same as ctx->param->purpose and ctx->param->trust is
     * also set.
     */
    if (depth >= ctx->num_untrusted && purpose == ctx->param->purpose)
        tr_ok = X509_check_trust(x, ctx->param->trust, X509_TRUST_NO_SS_COMPAT);

    switch (tr_ok) {
    case X509_TRUST_TRUSTED:
        return 1;
    case X509_TRUST_REJECTED:
        break;
    default: /* can only be X509_TRUST_UNTRUSTED */
        switch (X509_check_purpose(x, purpose, must_be_ca > 0)) {
        case 1:
            return 1;
        case 0:
            break;
        default:
            if ((ctx->param->flags & X509_V_FLAG_X509_STRICT) == 0)
                return 1;
        }
        break;
    }

    return verify_cb_cert(ctx, x, depth, X509_V_ERR_INVALID_PURPOSE);
}

/*-
 * Check extensions of a cert chain for consistency with the supplied purpose.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int check_extensions(X509_STORE_CTX *ctx)
{
    int i, must_be_ca, plen = 0;
    X509 *x;
    int ret, proxy_path_length = 0;
    int purpose, allow_proxy_certs, num = sk_X509_num(ctx->chain);

    /*-
     *  must_be_ca can have 1 of 3 values:
     * -1: we accept both CA and non-CA certificates, to allow direct
     *     use of self-signed certificates (which are marked as CA).
     * 0:  we only accept non-CA certificates.  This is currently not
     *     used, but the possibility is present for future extensions.
     * 1:  we only accept CA certificates.  This is currently used for
     *     all certificates in the chain except the leaf certificate.
     */
    must_be_ca = -1;

    /* CRL path validation */
    if (ctx->parent != NULL) {
        allow_proxy_certs = 0;
        purpose = X509_PURPOSE_CRL_SIGN;
    } else {
        allow_proxy_certs =
            (ctx->param->flags & X509_V_FLAG_ALLOW_PROXY_CERTS) != 0;
        purpose = ctx->param->purpose;
    }

    for (i = 0; i < num; i++) {
        x = sk_X509_value(ctx->chain, i);
        CB_FAIL_IF((ctx->param->flags & X509_V_FLAG_IGNORE_CRITICAL) == 0
                       && (x->ex_flags & EXFLAG_CRITICAL) != 0,
                   ctx, x, i, X509_V_ERR_UNHANDLED_CRITICAL_EXTENSION);
        CB_FAIL_IF(!allow_proxy_certs && (x->ex_flags & EXFLAG_PROXY) != 0,
                   ctx, x, i, X509_V_ERR_PROXY_CERTIFICATES_NOT_ALLOWED);
        ret = X509_check_ca(x);
        switch (must_be_ca) {
        case -1:
            CB_FAIL_IF((ctx->param->flags & X509_V_FLAG_X509_STRICT) != 0
                           && ret != 1 && ret != 0,
                       ctx, x, i, X509_V_ERR_INVALID_CA);
            break;
        case 0:
            CB_FAIL_IF(ret != 0, ctx, x, i, X509_V_ERR_INVALID_NON_CA);
            break;
        default:
            /* X509_V_FLAG_X509_STRICT is implicit for intermediate CAs */
            CB_FAIL_IF(ret == 0
                       || ((i + 1 < num
                            || (ctx->param->flags & X509_V_FLAG_X509_STRICT) != 0)
                           && ret != 1), ctx, x, i, X509_V_ERR_INVALID_CA);
            break;
        }
        if (num > 1) {
            /* Check for presence of explicit elliptic curve parameters */
            ret = check_curve(x);
            CB_FAIL_IF(ret < 0, ctx, x, i, X509_V_ERR_UNSPECIFIED);
            CB_FAIL_IF(ret == 0, ctx, x, i, X509_V_ERR_EC_KEY_EXPLICIT_PARAMS);
        }
        /*
         * Do the following set of checks only if strict checking is requested
         * and not for self-issued (including self-signed) EE (non-CA) certs
         * because RFC 5280 does not apply to them according RFC 6818 section 2.
         */
        if ((ctx->param->flags & X509_V_FLAG_X509_STRICT) != 0
            && num > 1) { /*
                           * this should imply
                           * !(i == 0 && (x->ex_flags & EXFLAG_CA) == 0
                           *          && (x->ex_flags & EXFLAG_SI) != 0)
                           */
            /* Check Basic Constraints according to RFC 5280 section 4.2.1.9 */
            if (x->ex_pathlen != -1) {
                CB_FAIL_IF((x->ex_flags & EXFLAG_CA) == 0,
                           ctx, x, i, X509_V_ERR_PATHLEN_INVALID_FOR_NON_CA);
                CB_FAIL_IF((x->ex_kusage & KU_KEY_CERT_SIGN) == 0, ctx,
                           x, i, X509_V_ERR_PATHLEN_WITHOUT_KU_KEY_CERT_SIGN);
            }
            CB_FAIL_IF((x->ex_flags & EXFLAG_CA) != 0
                           && (x->ex_flags & EXFLAG_BCONS) != 0
                           && (x->ex_flags & EXFLAG_BCONS_CRITICAL) == 0,
                       ctx, x, i, X509_V_ERR_CA_BCONS_NOT_CRITICAL);
            /* Check Key Usage according to RFC 5280 section 4.2.1.3 */
            if ((x->ex_flags & EXFLAG_CA) != 0) {
                CB_FAIL_IF((x->ex_flags & EXFLAG_KUSAGE) == 0,
                           ctx, x, i, X509_V_ERR_CA_CERT_MISSING_KEY_USAGE);
            } else {
                CB_FAIL_IF((x->ex_kusage & KU_KEY_CERT_SIGN) != 0, ctx, x, i,
                           X509_V_ERR_KU_KEY_CERT_SIGN_INVALID_FOR_NON_CA);
            }
            /* Check issuer is non-empty acc. to RFC 5280 section 4.1.2.4 */
            CB_FAIL_IF(X509_NAME_entry_count(X509_get_issuer_name(x)) == 0,
                       ctx, x, i, X509_V_ERR_ISSUER_NAME_EMPTY);
            /* Check subject is non-empty acc. to RFC 5280 section 4.1.2.6 */
            CB_FAIL_IF(((x->ex_flags & EXFLAG_CA) != 0
                        || (x->ex_kusage & KU_CRL_SIGN) != 0
                        || x->altname == NULL)
                       && X509_NAME_entry_count(X509_get_subject_name(x)) == 0,
                       ctx, x, i, X509_V_ERR_SUBJECT_NAME_EMPTY);
            CB_FAIL_IF(X509_NAME_entry_count(X509_get_subject_name(x)) == 0
                           && x->altname != NULL
                           && (x->ex_flags & EXFLAG_SAN_CRITICAL) == 0,
                       ctx, x, i, X509_V_ERR_EMPTY_SUBJECT_SAN_NOT_CRITICAL);
            /* Check SAN is non-empty according to RFC 5280 section 4.2.1.6 */
            CB_FAIL_IF(x->altname != NULL
                           && sk_GENERAL_NAME_num(x->altname) <= 0,
                       ctx, x, i, X509_V_ERR_EMPTY_SUBJECT_ALT_NAME);
            /* Check sig alg consistency acc. to RFC 5280 section 4.1.1.2 */
            CB_FAIL_IF(X509_ALGOR_cmp(&x->sig_alg, &x->cert_info.signature) != 0,
                       ctx, x, i, X509_V_ERR_SIGNATURE_ALGORITHM_INCONSISTENCY);
            CB_FAIL_IF(x->akid != NULL
                           && (x->ex_flags & EXFLAG_AKID_CRITICAL) != 0,
                       ctx, x, i, X509_V_ERR_AUTHORITY_KEY_IDENTIFIER_CRITICAL);
            CB_FAIL_IF(x->skid != NULL
                           && (x->ex_flags & EXFLAG_SKID_CRITICAL) != 0,
                       ctx, x, i, X509_V_ERR_SUBJECT_KEY_IDENTIFIER_CRITICAL);
            if (X509_get_version(x) >= X509_VERSION_3) {
                /* Check AKID presence acc. to RFC 5280 section 4.2.1.1 */
                CB_FAIL_IF(i + 1 < num /*
                                        * this means not last cert in chain,
                                        * taken as "generated by conforming CAs"
                                        */
                           && (x->akid == NULL || x->akid->keyid == NULL), ctx,
                           x, i, X509_V_ERR_MISSING_AUTHORITY_KEY_IDENTIFIER);
                /* Check SKID presence acc. to RFC 5280 section 4.2.1.2 */
                CB_FAIL_IF((x->ex_flags & EXFLAG_CA) != 0 && x->skid == NULL,
                           ctx, x, i, X509_V_ERR_MISSING_SUBJECT_KEY_IDENTIFIER);
            } else {
                CB_FAIL_IF(sk_X509_EXTENSION_num(X509_get0_extensions(x)) > 0,
                           ctx, x, i, X509_V_ERR_EXTENSIONS_REQUIRE_VERSION_3);
            }
        }

        /* check_purpose() makes the callback as needed */
        if (purpose >= X509_PURPOSE_MIN && !check_purpose(ctx, x, purpose, i, must_be_ca))
            return 0;
        /* Check path length */
        CB_FAIL_IF(i > 1 && x->ex_pathlen != -1
                       && plen > x->ex_pathlen + proxy_path_length,
                   ctx, x, i, X509_V_ERR_PATH_LENGTH_EXCEEDED);
        /* Increment path length if not a self-issued intermediate CA */
        if (i > 0 && (x->ex_flags & EXFLAG_SI) == 0)
            plen++;
        /*
         * If this certificate is a proxy certificate, the next certificate
         * must be another proxy certificate or a EE certificate.  If not,
         * the next certificate must be a CA certificate.
         */
        if (x->ex_flags & EXFLAG_PROXY) {
            /*
             * RFC3820, 4.1.3 (b)(1) stipulates that if pCPathLengthConstraint
             * is less than max_path_length, the former should be copied to
             * the latter, and 4.1.4 (a) stipulates that max_path_length
             * should be verified to be larger than zero and decrement it.
             *
             * Because we're checking the certs in the reverse order, we start
             * with verifying that proxy_path_length isn't larger than pcPLC,
             * and copy the latter to the former if it is, and finally,
             * increment proxy_path_length.
             */
            if (x->ex_pcpathlen != -1) {
                CB_FAIL_IF(proxy_path_length > x->ex_pcpathlen,
                           ctx, x, i, X509_V_ERR_PROXY_PATH_LENGTH_EXCEEDED);
                proxy_path_length = x->ex_pcpathlen;
            }
            proxy_path_length++;
            must_be_ca = 0;
        } else {
            must_be_ca = 1;
        }
    }
    return 1;
}

static int has_san_id(X509 *x, int gtype)
{
    int i;
    int ret = 0;
    GENERAL_NAMES *gs = X509_get_ext_d2i(x, NID_subject_alt_name, NULL, NULL);

    if (gs == NULL)
        return 0;

    for (i = 0; i < sk_GENERAL_NAME_num(gs); i++) {
        GENERAL_NAME *g = sk_GENERAL_NAME_value(gs, i);

        if (g->type == gtype) {
            ret = 1;
            break;
        }
    }
    GENERAL_NAMES_free(gs);
    return ret;
}

/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int check_name_constraints(X509_STORE_CTX *ctx)
{
    int i;

    /* Check name constraints for all certificates */
    for (i = sk_X509_num(ctx->chain) - 1; i >= 0; i--) {
        X509 *x = sk_X509_value(ctx->chain, i);
        int j;

        /* Ignore self-issued certs unless last in chain */
        if (i != 0 && (x->ex_flags & EXFLAG_SI) != 0)
            continue;

        /*
         * Proxy certificates policy has an extra constraint, where the
         * certificate subject MUST be the issuer with a single CN entry
         * added.
         * (RFC 3820: 3.4, 4.1.3 (a)(4))
         */
        if ((x->ex_flags & EXFLAG_PROXY) != 0) {
            X509_NAME *tmpsubject = X509_get_subject_name(x);
            X509_NAME *tmpissuer = X509_get_issuer_name(x);
            X509_NAME_ENTRY *tmpentry = NULL;
            int last_nid = 0;
            int err = X509_V_OK;
            int last_loc = X509_NAME_entry_count(tmpsubject) - 1;

            /* Check that there are at least two RDNs */
            if (last_loc < 1) {
                err = X509_V_ERR_PROXY_SUBJECT_NAME_VIOLATION;
                goto proxy_name_done;
            }

            /*
             * Check that there is exactly one more RDN in subject as
             * there is in issuer.
             */
            if (X509_NAME_entry_count(tmpsubject)
                != X509_NAME_entry_count(tmpissuer) + 1) {
                err = X509_V_ERR_PROXY_SUBJECT_NAME_VIOLATION;
                goto proxy_name_done;
            }

            /*
             * Check that the last subject component isn't part of a
             * multi-valued RDN
             */
            if (X509_NAME_ENTRY_set(X509_NAME_get_entry(tmpsubject, last_loc))
                == X509_NAME_ENTRY_set(X509_NAME_get_entry(tmpsubject,
                                                           last_loc - 1))) {
                err = X509_V_ERR_PROXY_SUBJECT_NAME_VIOLATION;
                goto proxy_name_done;
            }

            /*
             * Check that the last subject RDN is a commonName, and that
             * all the previous RDNs match the issuer exactly
             */
            tmpsubject = X509_NAME_dup(tmpsubject);
            if (tmpsubject == NULL) {
                ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
                ctx->error = X509_V_ERR_OUT_OF_MEM;
                return -1;
            }

            tmpentry = X509_NAME_delete_entry(tmpsubject, last_loc);
            last_nid = OBJ_obj2nid(X509_NAME_ENTRY_get_object(tmpentry));

            if (last_nid != NID_commonName
                || X509_NAME_cmp(tmpsubject, tmpissuer) != 0) {
                err = X509_V_ERR_PROXY_SUBJECT_NAME_VIOLATION;
            }

            X509_NAME_ENTRY_free(tmpentry);
            X509_NAME_free(tmpsubject);

        proxy_name_done:
            CB_FAIL_IF(err != X509_V_OK, ctx, x, i, err);
        }

        /*
         * Check against constraints for all certificates higher in chain
         * including trust anchor. Trust anchor not strictly speaking needed
         * but if it includes constraints it is to be assumed it expects them
         * to be obeyed.
         */
        for (j = sk_X509_num(ctx->chain) - 1; j > i; j--) {
            NAME_CONSTRAINTS *nc = sk_X509_value(ctx->chain, j)->nc;

            if (nc) {
                int rv = NAME_CONSTRAINTS_check(x, nc);
                int ret = 1;

                /* If EE certificate check commonName too */
                if (rv == X509_V_OK && i == 0
                    && (ctx->param->hostflags
                        & X509_CHECK_FLAG_NEVER_CHECK_SUBJECT) == 0
                    && ((ctx->param->hostflags
                         & X509_CHECK_FLAG_ALWAYS_CHECK_SUBJECT) != 0
                        || (ret = has_san_id(x, GEN_DNS)) == 0))
                    rv = NAME_CONSTRAINTS_check_CN(x, nc);
                if (ret < 0)
                    return ret;

                switch (rv) {
                case X509_V_OK:
                    break;
                case X509_V_ERR_OUT_OF_MEM:
                    return -1;
                default:
                    CB_FAIL_IF(1, ctx, x, i, rv);
                    break;
                }
            }
        }
    }
    return 1;
}

static int check_id_error(X509_STORE_CTX *ctx, int errcode)
{
    return verify_cb_cert(ctx, ctx->cert, 0, errcode);
}

static int check_hosts(X509 *x, X509_VERIFY_PARAM *vpm)
{
    int i;
    int n = sk_OPENSSL_STRING_num(vpm->hosts);
    char *name;

    if (vpm->peername != NULL) {
        OPENSSL_free(vpm->peername);
        vpm->peername = NULL;
    }
    for (i = 0; i < n; ++i) {
        name = sk_OPENSSL_STRING_value(vpm->hosts, i);
        if (X509_check_host(x, name, 0, vpm->hostflags, &vpm->peername) > 0)
            return 1;
    }
    return n == 0;
}

static int check_id(X509_STORE_CTX *ctx)
{
    X509_VERIFY_PARAM *vpm = ctx->param;
    X509 *x = ctx->cert;

    if (vpm->hosts != NULL && check_hosts(x, vpm) <= 0) {
        if (!check_id_error(ctx, X509_V_ERR_HOSTNAME_MISMATCH))
            return 0;
    }
    if (vpm->email != NULL
            && X509_check_email(x, vpm->email, vpm->emaillen, 0) <= 0) {
        if (!check_id_error(ctx, X509_V_ERR_EMAIL_MISMATCH))
            return 0;
    }
    if (vpm->ip != NULL && X509_check_ip(x, vpm->ip, vpm->iplen, 0) <= 0) {
        if (!check_id_error(ctx, X509_V_ERR_IP_ADDRESS_MISMATCH))
            return 0;
    }
    return 1;
}

/* Returns -1 on internal error */
static int check_trust(X509_STORE_CTX *ctx, int num_untrusted)
{
    int i, res;
    X509 *x = NULL;
    X509 *mx;
    SSL_DANE *dane = ctx->dane;
    int num = sk_X509_num(ctx->chain);
    int trust;

    /*
     * Check for a DANE issuer at depth 1 or greater, if it is a DANE-TA(2)
     * match, we're done, otherwise we'll merely record the match depth.
     */
    if (DANETLS_HAS_TA(dane) && num_untrusted > 0 && num_untrusted < num) {
        trust = check_dane_issuer(ctx, num_untrusted);
        if (trust != X509_TRUST_UNTRUSTED)
            return trust;
    }

    /*
     * Check trusted certificates in chain at depth num_untrusted and up.
     * Note, that depths 0..num_untrusted-1 may also contain trusted
     * certificates, but the caller is expected to have already checked those,
     * and wants to incrementally check just any added since.
     */
    for (i = num_untrusted; i < num; i++) {
        x = sk_X509_value(ctx->chain, i);
        trust = X509_check_trust(x, ctx->param->trust, 0);
        /* If explicitly trusted (so not neutral nor rejected) return trusted */
        if (trust == X509_TRUST_TRUSTED)
            goto trusted;
        if (trust == X509_TRUST_REJECTED)
            goto rejected;
    }

    /*
     * If we are looking at a trusted certificate, and accept partial chains,
     * the chain is PKIX trusted.
     */
    if (num_untrusted < num) {
        if ((ctx->param->flags & X509_V_FLAG_PARTIAL_CHAIN) != 0)
            goto trusted;
        return X509_TRUST_UNTRUSTED;
    }

    if (num_untrusted == num
            && (ctx->param->flags & X509_V_FLAG_PARTIAL_CHAIN) != 0) {
        /*
         * Last-resort call with no new trusted certificates, check the leaf
         * for a direct trust store match.
         */
        i = 0;
        x = sk_X509_value(ctx->chain, i);
        res = lookup_cert_match(&mx, ctx, x);
        if (res < 0)
            return res;
        if (res == 0)
            return X509_TRUST_UNTRUSTED;

        /*
         * Check explicit auxiliary trust/reject settings.  If none are set,
         * we'll accept X509_TRUST_UNTRUSTED when not self-signed.
         */
        trust = X509_check_trust(mx, ctx->param->trust, 0);
        if (trust == X509_TRUST_REJECTED) {
            X509_free(mx);
            goto rejected;
        }

        /* Replace leaf with trusted match */
        (void)sk_X509_set(ctx->chain, 0, mx);
        X509_free(x);
        ctx->num_untrusted = 0;
        goto trusted;
    }

    /*
     * If no trusted certs in chain at all return untrusted and allow
     * standard (no issuer cert) etc errors to be indicated.
     */
    return X509_TRUST_UNTRUSTED;

 rejected:
    return verify_cb_cert(ctx, x, i, X509_V_ERR_CERT_REJECTED) == 0
        ? X509_TRUST_REJECTED : X509_TRUST_UNTRUSTED;

 trusted:
    if (!DANETLS_ENABLED(dane))
        return X509_TRUST_TRUSTED;
    if (dane->pdpth < 0)
        dane->pdpth = num_untrusted;
    /* With DANE, PKIX alone is not trusted until we have both */
    if (dane->mdpth >= 0)
        return X509_TRUST_TRUSTED;
    return X509_TRUST_UNTRUSTED;
}

/* Sadly, returns 0 also on internal error. */
static int check_revocation(X509_STORE_CTX *ctx)
{
    int i = 0, last = 0, ok = 0;

    if ((ctx->param->flags & X509_V_FLAG_CRL_CHECK) == 0)
        return 1;
    if ((ctx->param->flags & X509_V_FLAG_CRL_CHECK_ALL) != 0) {
        last = sk_X509_num(ctx->chain) - 1;
    } else {
        /* If checking CRL paths this isn't the EE certificate */
        if (ctx->parent != NULL)
            return 1;
        last = 0;
    }
    for (i = 0; i <= last; i++) {
        ctx->error_depth = i;
        ok = check_cert(ctx);
        if (!ok)
            return ok;
    }
    return 1;
}

/* Sadly, returns 0 also on internal error. */
static int check_cert(X509_STORE_CTX *ctx)
{
    X509_CRL *crl = NULL, *dcrl = NULL;
    int ok = 0;
    int cnum = ctx->error_depth;
    X509 *x = sk_X509_value(ctx->chain, cnum);

    ctx->current_cert = x;
    ctx->current_issuer = NULL;
    ctx->current_crl_score = 0;
    ctx->current_reasons = 0;

    if ((x->ex_flags & EXFLAG_PROXY) != 0)
        return 1;

    while (ctx->current_reasons != CRLDP_ALL_REASONS) {
        unsigned int last_reasons = ctx->current_reasons;

        /* Try to retrieve relevant CRL */
        if (ctx->get_crl != NULL)
            ok = ctx->get_crl(ctx, &crl, x);
        else
            ok = get_crl_delta(ctx, &crl, &dcrl, x);
        /* If error looking up CRL, nothing we can do except notify callback */
        if (!ok) {
            ok = verify_cb_crl(ctx, X509_V_ERR_UNABLE_TO_GET_CRL);
            goto done;
        }
        ctx->current_crl = crl;
        ok = ctx->check_crl(ctx, crl);
        if (!ok)
            goto done;

        if (dcrl != NULL) {
            ok = ctx->check_crl(ctx, dcrl);
            if (!ok)
                goto done;
            ok = ctx->cert_crl(ctx, dcrl, x);
            if (!ok)
                goto done;
        } else {
            ok = 1;
        }

        /* Don't look in full CRL if delta reason is removefromCRL */
        if (ok != 2) {
            ok = ctx->cert_crl(ctx, crl, x);
            if (!ok)
                goto done;
        }

        X509_CRL_free(crl);
        X509_CRL_free(dcrl);
        crl = NULL;
        dcrl = NULL;
        /*
         * If reasons not updated we won't get anywhere by another iteration,
         * so exit loop.
         */
        if (last_reasons == ctx->current_reasons) {
            ok = verify_cb_crl(ctx, X509_V_ERR_UNABLE_TO_GET_CRL);
            goto done;
        }
    }
 done:
    X509_CRL_free(crl);
    X509_CRL_free(dcrl);

    ctx->current_crl = NULL;
    return ok;
}

/* Check CRL times against values in X509_STORE_CTX */
static int check_crl_time(X509_STORE_CTX *ctx, X509_CRL *crl, int notify)
{
    time_t *ptime;
    int i;

    if ((ctx->param->flags & X509_V_FLAG_USE_CHECK_TIME) != 0)
        ptime = &ctx->param->check_time;
    else if ((ctx->param->flags & X509_V_FLAG_NO_CHECK_TIME) != 0)
        return 1;
    else
        ptime = NULL;
    if (notify)
        ctx->current_crl = crl;

    i = X509_cmp_time(X509_CRL_get0_lastUpdate(crl), ptime);
    if (i == 0) {
        if (!notify)
            return 0;
        if (!verify_cb_crl(ctx, X509_V_ERR_ERROR_IN_CRL_LAST_UPDATE_FIELD))
            return 0;
    }

    if (i > 0) {
        if (!notify)
            return 0;
        if (!verify_cb_crl(ctx, X509_V_ERR_CRL_NOT_YET_VALID))
            return 0;
    }

    if (X509_CRL_get0_nextUpdate(crl)) {
        i = X509_cmp_time(X509_CRL_get0_nextUpdate(crl), ptime);

        if (i == 0) {
            if (!notify)
                return 0;
            if (!verify_cb_crl(ctx, X509_V_ERR_ERROR_IN_CRL_NEXT_UPDATE_FIELD))
                return 0;
        }
        /* Ignore expiration of base CRL is delta is valid */
        if (i < 0 && (ctx->current_crl_score & CRL_SCORE_TIME_DELTA) == 0) {
            if (!notify || !verify_cb_crl(ctx, X509_V_ERR_CRL_HAS_EXPIRED))
                return 0;
        }
    }

    if (notify)
        ctx->current_crl = NULL;

    return 1;
}

static int get_crl_sk(X509_STORE_CTX *ctx, X509_CRL **pcrl, X509_CRL **pdcrl,
                      X509 **pissuer, int *pscore, unsigned int *preasons,
                      STACK_OF(X509_CRL) *crls)
{
    int i, crl_score, best_score = *pscore;
    unsigned int reasons, best_reasons = 0;
    X509 *x = ctx->current_cert;
    X509_CRL *crl, *best_crl = NULL;
    X509 *crl_issuer = NULL, *best_crl_issuer = NULL;

    for (i = 0; i < sk_X509_CRL_num(crls); i++) {
        crl = sk_X509_CRL_value(crls, i);
        reasons = *preasons;
        crl_score = get_crl_score(ctx, &crl_issuer, &reasons, crl, x);
        if (crl_score < best_score || crl_score == 0)
            continue;
        /* If current CRL is equivalent use it if it is newer */
        if (crl_score == best_score && best_crl != NULL) {
            int day, sec;

            if (ASN1_TIME_diff(&day, &sec, X509_CRL_get0_lastUpdate(best_crl),
                               X509_CRL_get0_lastUpdate(crl)) == 0)
                continue;
            /*
             * ASN1_TIME_diff never returns inconsistent signs for |day|
             * and |sec|.
             */
            if (day <= 0 && sec <= 0)
                continue;
        }
        best_crl = crl;
        best_crl_issuer = crl_issuer;
        best_score = crl_score;
        best_reasons = reasons;
    }

    if (best_crl != NULL) {
        if (!X509_CRL_up_ref(best_crl))
            return 0;
        X509_CRL_free(*pcrl);
        *pcrl = best_crl;
        *pissuer = best_crl_issuer;
        *pscore = best_score;
        *preasons = best_reasons;
        X509_CRL_free(*pdcrl);
        *pdcrl = NULL;
        get_delta_sk(ctx, pdcrl, pscore, best_crl, crls);
    }

    if (best_score >= CRL_SCORE_VALID)
        return 1;

    return 0;
}

/*
 * Compare two CRL extensions for delta checking purposes. They should be
 * both present or both absent. If both present all fields must be identical.
 */
static int crl_extension_match(X509_CRL *a, X509_CRL *b, int nid)
{
    ASN1_OCTET_STRING *exta = NULL, *extb = NULL;
    int i = X509_CRL_get_ext_by_NID(a, nid, -1);

    if (i >= 0) {
        /* Can't have multiple occurrences */
        if (X509_CRL_get_ext_by_NID(a, nid, i) != -1)
            return 0;
        exta = X509_EXTENSION_get_data(X509_CRL_get_ext(a, i));
    }

    i = X509_CRL_get_ext_by_NID(b, nid, -1);
    if (i >= 0) {
        if (X509_CRL_get_ext_by_NID(b, nid, i) != -1)
            return 0;
        extb = X509_EXTENSION_get_data(X509_CRL_get_ext(b, i));
    }

    if (exta == NULL && extb == NULL)
        return 1;

    if (exta == NULL || extb == NULL)
        return 0;

    return ASN1_OCTET_STRING_cmp(exta, extb) == 0;
}

/* See if a base and delta are compatible */
static int check_delta_base(X509_CRL *delta, X509_CRL *base)
{
    /* Delta CRL must be a delta */
    if (delta->base_crl_number == NULL)
        return 0;
    /* Base must have a CRL number */
    if (base->crl_number == NULL)
        return 0;
    /* Issuer names must match */
    if (X509_NAME_cmp(X509_CRL_get_issuer(base),
                      X509_CRL_get_issuer(delta)) != 0)
        return 0;
    /* AKID and IDP must match */
    if (!crl_extension_match(delta, base, NID_authority_key_identifier))
        return 0;
    if (!crl_extension_match(delta, base, NID_issuing_distribution_point))
        return 0;
    /* Delta CRL base number must not exceed Full CRL number. */
    if (ASN1_INTEGER_cmp(delta->base_crl_number, base->crl_number) > 0)
        return 0;
    /* Delta CRL number must exceed full CRL number */
    return ASN1_INTEGER_cmp(delta->crl_number, base->crl_number) > 0;
}

/*
 * For a given base CRL find a delta... maybe extend to delta scoring or
 * retrieve a chain of deltas...
 */
static void get_delta_sk(X509_STORE_CTX *ctx, X509_CRL **dcrl, int *pscore,
                         X509_CRL *base, STACK_OF(X509_CRL) *crls)
{
    X509_CRL *delta;
    int i;

    if ((ctx->param->flags & X509_V_FLAG_USE_DELTAS) == 0)
        return;
    if (((ctx->current_cert->ex_flags | base->flags) & EXFLAG_FRESHEST) == 0)
        return;
    for (i = 0; i < sk_X509_CRL_num(crls); i++) {
        delta = sk_X509_CRL_value(crls, i);
        if (check_delta_base(delta, base)) {
            if (!X509_CRL_up_ref(delta)) {
                *dcrl = NULL;
                return;
            }

            *dcrl = delta;

            if (check_crl_time(ctx, delta, 0))
                *pscore |= CRL_SCORE_TIME_DELTA;

            return;
        }
    }
    *dcrl = NULL;
}

/*
 * For a given CRL return how suitable it is for the supplied certificate
 * 'x'. The return value is a mask of several criteria. If the issuer is not
 * the certificate issuer this is returned in *pissuer. The reasons mask is
 * also used to determine if the CRL is suitable: if no new reasons the CRL
 * is rejected, otherwise reasons is updated.
 */
static int get_crl_score(X509_STORE_CTX *ctx, X509 **pissuer,
                         unsigned int *preasons, X509_CRL *crl, X509 *x)
{
    int crl_score = 0;
    unsigned int tmp_reasons = *preasons, crl_reasons;

    /* First see if we can reject CRL straight away */

    /* Invalid IDP cannot be processed */
    if ((crl->idp_flags & IDP_INVALID) != 0)
        return 0;
    /* Reason codes or indirect CRLs need extended CRL support */
    if ((ctx->param->flags & X509_V_FLAG_EXTENDED_CRL_SUPPORT) == 0) {
        if (crl->idp_flags & (IDP_INDIRECT | IDP_REASONS))
            return 0;
    } else if ((crl->idp_flags & IDP_REASONS) != 0) {
        /* If no new reasons reject */
        if ((crl->idp_reasons & ~tmp_reasons) == 0)
            return 0;
    }
    /* Don't process deltas at this stage */
    else if (crl->base_crl_number != NULL)
        return 0;
    /* If issuer name doesn't match certificate need indirect CRL */
    if (X509_NAME_cmp(X509_get_issuer_name(x), X509_CRL_get_issuer(crl)) != 0) {
        if ((crl->idp_flags & IDP_INDIRECT) == 0)
            return 0;
    } else {
        crl_score |= CRL_SCORE_ISSUER_NAME;
    }

    if ((crl->flags & EXFLAG_CRITICAL) == 0)
        crl_score |= CRL_SCORE_NOCRITICAL;

    /* Check expiration */
    if (check_crl_time(ctx, crl, 0))
        crl_score |= CRL_SCORE_TIME;

    /* Check authority key ID and locate certificate issuer */
    crl_akid_check(ctx, crl, pissuer, &crl_score);

    /* If we can't locate certificate issuer at this point forget it */
    if ((crl_score & CRL_SCORE_AKID) == 0)
        return 0;

    /* Check cert for matching CRL distribution points */
    if (crl_crldp_check(x, crl, crl_score, &crl_reasons)) {
        /* If no new reasons reject */
        if ((crl_reasons & ~tmp_reasons) == 0)
            return 0;
        tmp_reasons |= crl_reasons;
        crl_score |= CRL_SCORE_SCOPE;
    }

    *preasons = tmp_reasons;

    return crl_score;

}

static void crl_akid_check(X509_STORE_CTX *ctx, X509_CRL *crl,
                           X509 **pissuer, int *pcrl_score)
{
    X509 *crl_issuer = NULL;
    const X509_NAME *cnm = X509_CRL_get_issuer(crl);
    int cidx = ctx->error_depth;
    int i;

    if (cidx != sk_X509_num(ctx->chain) - 1)
        cidx++;

    crl_issuer = sk_X509_value(ctx->chain, cidx);

    if (X509_check_akid(crl_issuer, crl->akid) == X509_V_OK) {
        if (*pcrl_score & CRL_SCORE_ISSUER_NAME) {
            *pcrl_score |= CRL_SCORE_AKID | CRL_SCORE_ISSUER_CERT;
            *pissuer = crl_issuer;
            return;
        }
    }

    for (cidx++; cidx < sk_X509_num(ctx->chain); cidx++) {
        crl_issuer = sk_X509_value(ctx->chain, cidx);
        if (X509_NAME_cmp(X509_get_subject_name(crl_issuer), cnm))
            continue;
        if (X509_check_akid(crl_issuer, crl->akid) == X509_V_OK) {
            *pcrl_score |= CRL_SCORE_AKID | CRL_SCORE_SAME_PATH;
            *pissuer = crl_issuer;
            return;
        }
    }

    /* Anything else needs extended CRL support */
    if ((ctx->param->flags & X509_V_FLAG_EXTENDED_CRL_SUPPORT) == 0)
        return;

    /*
     * Otherwise the CRL issuer is not on the path. Look for it in the set of
     * untrusted certificates.
     */
    for (i = 0; i < sk_X509_num(ctx->untrusted); i++) {
        crl_issuer = sk_X509_value(ctx->untrusted, i);
        if (X509_NAME_cmp(X509_get_subject_name(crl_issuer), cnm) != 0)
            continue;
        if (X509_check_akid(crl_issuer, crl->akid) == X509_V_OK) {
            *pissuer = crl_issuer;
            *pcrl_score |= CRL_SCORE_AKID;
            return;
        }
    }
}

/*
 * Check the path of a CRL issuer certificate. This creates a new
 * X509_STORE_CTX and populates it with most of the parameters from the
 * parent. This could be optimised somewhat since a lot of path checking will
 * be duplicated by the parent, but this will rarely be used in practice.
 */
static int check_crl_path(X509_STORE_CTX *ctx, X509 *x)
{
    X509_STORE_CTX crl_ctx = {0};
    int ret;

    /* Don't allow recursive CRL path validation */
    if (ctx->parent != NULL)
        return 0;
    if (!X509_STORE_CTX_init(&crl_ctx, ctx->store, x, ctx->untrusted))
        return -1;

    crl_ctx.crls = ctx->crls;
    /* Copy verify params across */
    X509_STORE_CTX_set0_param(&crl_ctx, ctx->param);

    crl_ctx.parent = ctx;
    crl_ctx.verify_cb = ctx->verify_cb;

    /* Verify CRL issuer */
    ret = X509_verify_cert(&crl_ctx);
    if (ret <= 0)
        goto err;

    /* Check chain is acceptable */
    ret = check_crl_chain(ctx, ctx->chain, crl_ctx.chain);
 err:
    X509_STORE_CTX_cleanup(&crl_ctx);
    return ret;
}

/*
 * RFC3280 says nothing about the relationship between CRL path and
 * certificate path, which could lead to situations where a certificate could
 * be revoked or validated by a CA not authorized to do so. RFC5280 is more
 * strict and states that the two paths must end in the same trust anchor,
 * though some discussions remain... until this is resolved we use the
 * RFC5280 version
 */
static int check_crl_chain(X509_STORE_CTX *ctx,
                           STACK_OF(X509) *cert_path,
                           STACK_OF(X509) *crl_path)
{
    X509 *cert_ta = sk_X509_value(cert_path, sk_X509_num(cert_path) - 1);
    X509 *crl_ta = sk_X509_value(crl_path, sk_X509_num(crl_path) - 1);

    return X509_cmp(cert_ta, crl_ta) == 0;
}

/*-
 * Check for match between two dist point names: three separate cases.
 * 1. Both are relative names and compare X509_NAME types.
 * 2. One full, one relative. Compare X509_NAME to GENERAL_NAMES.
 * 3. Both are full names and compare two GENERAL_NAMES.
 * 4. One is NULL: automatic match.
 */
static int idp_check_dp(DIST_POINT_NAME *a, DIST_POINT_NAME *b)
{
    X509_NAME *nm = NULL;
    GENERAL_NAMES *gens = NULL;
    GENERAL_NAME *gena, *genb;
    int i, j;

    if (a == NULL || b == NULL)
        return 1;
    if (a->type == 1) {
        if (a->dpname == NULL)
            return 0;
        /* Case 1: two X509_NAME */
        if (b->type == 1) {
            if (b->dpname == NULL)
                return 0;
            return X509_NAME_cmp(a->dpname, b->dpname) == 0;
        }
        /* Case 2: set name and GENERAL_NAMES appropriately */
        nm = a->dpname;
        gens = b->name.fullname;
    } else if (b->type == 1) {
        if (b->dpname == NULL)
            return 0;
        /* Case 2: set name and GENERAL_NAMES appropriately */
        gens = a->name.fullname;
        nm = b->dpname;
    }

    /* Handle case 2 with one GENERAL_NAMES and one X509_NAME */
    if (nm != NULL) {
        for (i = 0; i < sk_GENERAL_NAME_num(gens); i++) {
            gena = sk_GENERAL_NAME_value(gens, i);
            if (gena->type != GEN_DIRNAME)
                continue;
            if (X509_NAME_cmp(nm, gena->d.directoryName) == 0)
                return 1;
        }
        return 0;
    }

    /* Else case 3: two GENERAL_NAMES */

    for (i = 0; i < sk_GENERAL_NAME_num(a->name.fullname); i++) {
        gena = sk_GENERAL_NAME_value(a->name.fullname, i);
        for (j = 0; j < sk_GENERAL_NAME_num(b->name.fullname); j++) {
            genb = sk_GENERAL_NAME_value(b->name.fullname, j);
            if (GENERAL_NAME_cmp(gena, genb) == 0)
                return 1;
        }
    }

    return 0;

}

static int crldp_check_crlissuer(DIST_POINT *dp, X509_CRL *crl, int crl_score)
{
    int i;
    const X509_NAME *nm = X509_CRL_get_issuer(crl);

    /* If no CRLissuer return is successful iff don't need a match */
    if (dp->CRLissuer == NULL)
        return (crl_score & CRL_SCORE_ISSUER_NAME) != 0;
    for (i = 0; i < sk_GENERAL_NAME_num(dp->CRLissuer); i++) {
        GENERAL_NAME *gen = sk_GENERAL_NAME_value(dp->CRLissuer, i);

        if (gen->type != GEN_DIRNAME)
            continue;
        if (X509_NAME_cmp(gen->d.directoryName, nm) == 0)
            return 1;
    }
    return 0;
}

/* Check CRLDP and IDP */
static int crl_crldp_check(X509 *x, X509_CRL *crl, int crl_score,
                           unsigned int *preasons)
{
    int i;

    if ((crl->idp_flags & IDP_ONLYATTR) != 0)
        return 0;
    if ((x->ex_flags & EXFLAG_CA) != 0) {
        if ((crl->idp_flags & IDP_ONLYUSER) != 0)
            return 0;
    } else {
        if ((crl->idp_flags & IDP_ONLYCA) != 0)
            return 0;
    }
    *preasons = crl->idp_reasons;
    for (i = 0; i < sk_DIST_POINT_num(x->crldp); i++) {
        DIST_POINT *dp = sk_DIST_POINT_value(x->crldp, i);

        if (crldp_check_crlissuer(dp, crl, crl_score)) {
            if (crl->idp == NULL
                    || idp_check_dp(dp->distpoint, crl->idp->distpoint)) {
                *preasons &= dp->dp_reasons;
                return 1;
            }
        }
    }
    return (crl->idp == NULL || crl->idp->distpoint == NULL)
            && (crl_score & CRL_SCORE_ISSUER_NAME) != 0;
}

/*
 * Retrieve CRL corresponding to current certificate. If deltas enabled try
 * to find a delta CRL too
 */
static int get_crl_delta(X509_STORE_CTX *ctx,
                         X509_CRL **pcrl, X509_CRL **pdcrl, X509 *x)
{
    int ok;
    X509 *issuer = NULL;
    int crl_score = 0;
    unsigned int reasons;
    X509_CRL *crl = NULL, *dcrl = NULL;
    STACK_OF(X509_CRL) *skcrl;
    const X509_NAME *nm = X509_get_issuer_name(x);

    reasons = ctx->current_reasons;
    ok = get_crl_sk(ctx, &crl, &dcrl,
                    &issuer, &crl_score, &reasons, ctx->crls);
    if (ok)
        goto done;

    /* Lookup CRLs from store */
    skcrl = ctx->lookup_crls(ctx, nm);

    /* If no CRLs found and a near match from get_crl_sk use that */
    if (skcrl == NULL && crl != NULL)
        goto done;

    get_crl_sk(ctx, &crl, &dcrl, &issuer, &crl_score, &reasons, skcrl);

    sk_X509_CRL_pop_free(skcrl, X509_CRL_free);

 done:
    /* If we got any kind of CRL use it and return success */
    if (crl != NULL) {
        ctx->current_issuer = issuer;
        ctx->current_crl_score = crl_score;
        ctx->current_reasons = reasons;
        *pcrl = crl;
        *pdcrl = dcrl;
        return 1;
    }
    return 0;
}

/* Check CRL validity */
static int check_crl(X509_STORE_CTX *ctx, X509_CRL *crl)
{
    X509 *issuer = NULL;
    EVP_PKEY *ikey = NULL;
    int cnum = ctx->error_depth;
    int chnum = sk_X509_num(ctx->chain) - 1;

    /* If we have an alternative CRL issuer cert use that */
    if (ctx->current_issuer != NULL) {
        issuer = ctx->current_issuer;
    /*
     * Else find CRL issuer: if not last certificate then issuer is next
     * certificate in chain.
     */
    } else if (cnum < chnum) {
        issuer = sk_X509_value(ctx->chain, cnum + 1);
    } else {
        issuer = sk_X509_value(ctx->chain, chnum);
        if (!ossl_assert(issuer != NULL))
            return 0;
        /* If not self-issued, can't check signature */
        if (!ctx->check_issued(ctx, issuer, issuer) &&
            !verify_cb_crl(ctx, X509_V_ERR_UNABLE_TO_GET_CRL_ISSUER))
            return 0;
    }

    if (issuer == NULL)
        return 1;

    /*
     * Skip most tests for deltas because they have already been done
     */
    if (crl->base_crl_number == NULL) {
        /* Check for cRLSign bit if keyUsage present */
        if ((issuer->ex_flags & EXFLAG_KUSAGE) != 0 &&
            (issuer->ex_kusage & KU_CRL_SIGN) == 0 &&
            !verify_cb_crl(ctx, X509_V_ERR_KEYUSAGE_NO_CRL_SIGN))
            return 0;

        if ((ctx->current_crl_score & CRL_SCORE_SCOPE) == 0 &&
            !verify_cb_crl(ctx, X509_V_ERR_DIFFERENT_CRL_SCOPE))
            return 0;

        if ((ctx->current_crl_score & CRL_SCORE_SAME_PATH) == 0 &&
            check_crl_path(ctx, ctx->current_issuer) <= 0 &&
            !verify_cb_crl(ctx, X509_V_ERR_CRL_PATH_VALIDATION_ERROR))
            return 0;

        if ((crl->idp_flags & IDP_INVALID) != 0 &&
            !verify_cb_crl(ctx, X509_V_ERR_INVALID_EXTENSION))
            return 0;
    }

    if ((ctx->current_crl_score & CRL_SCORE_TIME) == 0 &&
        !check_crl_time(ctx, crl, 1))
        return 0;

    /* Attempt to get issuer certificate public key */
    ikey = X509_get0_pubkey(issuer);
    if (ikey == NULL &&
        !verify_cb_crl(ctx, X509_V_ERR_UNABLE_TO_DECODE_ISSUER_PUBLIC_KEY))
        return 0;

    if (ikey != NULL) {
        int rv = X509_CRL_check_suiteb(crl, ikey, ctx->param->flags);

        if (rv != X509_V_OK && !verify_cb_crl(ctx, rv))
            return 0;
        /* Verify CRL signature */
        if (X509_CRL_verify(crl, ikey) <= 0 &&
            !verify_cb_crl(ctx, X509_V_ERR_CRL_SIGNATURE_FAILURE))
            return 0;
    }
    return 1;
}

/* Check certificate against CRL */
static int cert_crl(X509_STORE_CTX *ctx, X509_CRL *crl, X509 *x)
{
    X509_REVOKED *rev;

    /*
     * The rules changed for this... previously if a CRL contained unhandled
     * critical extensions it could still be used to indicate a certificate
     * was revoked. This has since been changed since critical extensions can
     * change the meaning of CRL entries.
     */
    if ((ctx->param->flags & X509_V_FLAG_IGNORE_CRITICAL) == 0
        && (crl->flags & EXFLAG_CRITICAL) != 0 &&
        !verify_cb_crl(ctx, X509_V_ERR_UNHANDLED_CRITICAL_CRL_EXTENSION))
        return 0;
    /*
     * Look for serial number of certificate in CRL.  If found, make sure
     * reason is not removeFromCRL.
     */
    if (X509_CRL_get0_by_cert(crl, &rev, x)) {
        if (rev->reason == CRL_REASON_REMOVE_FROM_CRL)
            return 2;
        if (!verify_cb_crl(ctx, X509_V_ERR_CERT_REVOKED))
            return 0;
    }

    return 1;
}

/* Sadly, returns 0 also on internal error in ctx->verify_cb(). */
static int check_policy(X509_STORE_CTX *ctx)
{
    int ret;

    if (ctx->parent)
        return 1;
    /*
     * With DANE, the trust anchor might be a bare public key, not a
     * certificate!  In that case our chain does not have the trust anchor
     * certificate as a top-most element.  This comports well with RFC5280
     * chain verification, since there too, the trust anchor is not part of the
     * chain to be verified.  In particular, X509_policy_check() does not look
     * at the TA cert, but assumes that it is present as the top-most chain
     * element.  We therefore temporarily push a NULL cert onto the chain if it
     * was verified via a bare public key, and pop it off right after the
     * X509_policy_check() call.
     */
    if (ctx->bare_ta_signed && !sk_X509_push(ctx->chain, NULL)) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto memerr;
    }
    ret = X509_policy_check(&ctx->tree, &ctx->explicit_policy, ctx->chain,
                            ctx->param->policies, ctx->param->flags);
    if (ctx->bare_ta_signed)
        (void)sk_X509_pop(ctx->chain);

    if (ret == X509_PCY_TREE_INTERNAL) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto memerr;
    }
    /* Invalid or inconsistent extensions */
    if (ret == X509_PCY_TREE_INVALID) {
        int i, cbcalled = 0;

        /* Locate certificates with bad extensions and notify callback. */
        for (i = 0; i < sk_X509_num(ctx->chain); i++) {
            X509 *x = sk_X509_value(ctx->chain, i);

            if ((x->ex_flags & EXFLAG_INVALID_POLICY) != 0)
                cbcalled = 1;
            CB_FAIL_IF((x->ex_flags & EXFLAG_INVALID_POLICY) != 0,
                       ctx, x, i, X509_V_ERR_INVALID_POLICY_EXTENSION);
        }
        if (!cbcalled) {
            /* Should not be able to get here */
            ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
            return 0;
        }
        /* The callback ignored the error so we return success */
        return 1;
    }
    if (ret == X509_PCY_TREE_FAILURE) {
        ctx->current_cert = NULL;
        ctx->error = X509_V_ERR_NO_EXPLICIT_POLICY;
        return ctx->verify_cb(0, ctx);
    }
    if (ret != X509_PCY_TREE_VALID) {
        ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if ((ctx->param->flags & X509_V_FLAG_NOTIFY_POLICY) != 0) {
        ctx->current_cert = NULL;
        /*
         * Verification errors need to be "sticky", a callback may have allowed
         * an SSL handshake to continue despite an error, and we must then
         * remain in an error state.  Therefore, we MUST NOT clear earlier
         * verification errors by setting the error to X509_V_OK.
         */
        if (!ctx->verify_cb(2, ctx))
            return 0;
    }

    return 1;

 memerr:
    ctx->error = X509_V_ERR_OUT_OF_MEM;
    return -1;
}

/*-
 * Check certificate validity times.
 * If depth >= 0, invoke verification callbacks on error, otherwise just return
 * the validation status.
 *
 * Return 1 on success, 0 otherwise.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
int ossl_x509_check_cert_time(X509_STORE_CTX *ctx, X509 *x, int depth)
{
    time_t *ptime;
    int i;

    if ((ctx->param->flags & X509_V_FLAG_USE_CHECK_TIME) != 0)
        ptime = &ctx->param->check_time;
    else if ((ctx->param->flags & X509_V_FLAG_NO_CHECK_TIME) != 0)
        return 1;
    else
        ptime = NULL;

    i = X509_cmp_time(X509_get0_notBefore(x), ptime);
    if (i >= 0 && depth < 0)
        return 0;
    CB_FAIL_IF(i == 0, ctx, x, depth, X509_V_ERR_ERROR_IN_CERT_NOT_BEFORE_FIELD);
    CB_FAIL_IF(i > 0, ctx, x, depth, X509_V_ERR_CERT_NOT_YET_VALID);

    i = X509_cmp_time(X509_get0_notAfter(x), ptime);
    if (i <= 0 && depth < 0)
        return 0;
    CB_FAIL_IF(i == 0, ctx, x, depth, X509_V_ERR_ERROR_IN_CERT_NOT_AFTER_FIELD);
    CB_FAIL_IF(i < 0, ctx, x, depth, X509_V_ERR_CERT_HAS_EXPIRED);
    return 1;
}

/*
 * Verify the issuer signatures and cert times of ctx->chain.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int internal_verify(X509_STORE_CTX *ctx)
{
    int n;
    X509 *xi;
    X509 *xs;

    /* For RPK: just do the verify callback */
    if (ctx->rpk != NULL) {
        if (!ctx->verify_cb(ctx->error == X509_V_OK, ctx))
            return 0;
        return 1;
    }
    n = sk_X509_num(ctx->chain) - 1;
    xi = sk_X509_value(ctx->chain, n);
    xs = xi;

    ctx->error_depth = n;
    if (ctx->bare_ta_signed) {
        /*
         * With DANE-verified bare public key TA signatures,
         * on the top certificate we check only the timestamps.
         * We report the issuer as NULL because all we have is a bare key.
         */
        xi = NULL;
    } else if (ossl_x509_likely_issued(xi, xi) != X509_V_OK
               /* exceptional case: last cert in the chain is not self-issued */
               && ((ctx->param->flags & X509_V_FLAG_PARTIAL_CHAIN) == 0)) {
        if (n > 0) {
            n--;
            ctx->error_depth = n;
            xs = sk_X509_value(ctx->chain, n);
        } else {
            CB_FAIL_IF(1, ctx, xi, 0,
                       X509_V_ERR_UNABLE_TO_VERIFY_LEAF_SIGNATURE);
        }
        /*
         * The below code will certainly not do a
         * self-signature check on xi because it is not self-issued.
         */
    }

    /*
     * Do not clear error (by ctx->error = X509_V_OK), it must be "sticky",
     * only the user's callback is allowed to reset errors (at its own peril).
     */
    while (n >= 0) {
        /*-
         * For each iteration of this loop:
         * n is the subject depth
         * xs is the subject cert, for which the signature is to be checked
         * xi is NULL for DANE-verified bare public key TA signatures
         *       else the supposed issuer cert containing the public key to use
         * Initially xs == xi if the last cert in the chain is self-issued.
         */
        /*
         * Do signature check for self-signed certificates only if explicitly
         * asked for because it does not add any security and just wastes time.
         */
        if (xi != NULL
            && (xs != xi
                || ((ctx->param->flags & X509_V_FLAG_CHECK_SS_SIGNATURE) != 0
                    && (xi->ex_flags & EXFLAG_SS) != 0))) {
            EVP_PKEY *pkey;
            /*
             * If the issuer's public key is not available or its key usage
             * does not support issuing the subject cert, report the issuer
             * cert and its depth (rather than n, the depth of the subject).
             */
            int issuer_depth = n + (xs == xi ? 0 : 1);
            /*
             * According to https://tools.ietf.org/html/rfc5280#section-6.1.4
             * step (n) we must check any given key usage extension in a CA cert
             * when preparing the verification of a certificate issued by it.
             * According to https://tools.ietf.org/html/rfc5280#section-4.2.1.3
             * we must not verify a certificate signature if the key usage of
             * the CA certificate that issued the certificate prohibits signing.
             * In case the 'issuing' certificate is the last in the chain and is
             * not a CA certificate but a 'self-issued' end-entity cert (i.e.,
             * xs == xi && !(xi->ex_flags & EXFLAG_CA)) RFC 5280 does not apply
             * (see https://tools.ietf.org/html/rfc6818#section-2) and thus
             * we are free to ignore any key usage restrictions on such certs.
             */
            int ret = xs == xi && (xi->ex_flags & EXFLAG_CA) == 0
                ? X509_V_OK : ossl_x509_signing_allowed(xi, xs);

            CB_FAIL_IF(ret != X509_V_OK, ctx, xi, issuer_depth, ret);
            if ((pkey = X509_get0_pubkey(xi)) == NULL) {
                CB_FAIL_IF(1, ctx, xi, issuer_depth,
                           X509_V_ERR_UNABLE_TO_DECODE_ISSUER_PUBLIC_KEY);
            } else {
                CB_FAIL_IF(X509_verify(xs, pkey) <= 0,
                           ctx, xs, n, X509_V_ERR_CERT_SIGNATURE_FAILURE);
            }
        }

        /* In addition to RFC 5280 requirements do also for trust anchor cert */
        /* Calls verify callback as needed */
        if (!ossl_x509_check_cert_time(ctx, xs, n))
            return 0;

        /*
         * Signal success at this depth.  However, the previous error (if any)
         * is retained.
         */
        ctx->current_issuer = xi;
        ctx->current_cert = xs;
        ctx->error_depth = n;
        if (!ctx->verify_cb(1, ctx))
            return 0;

        if (--n >= 0) {
            xi = xs;
            xs = sk_X509_value(ctx->chain, n);
        }
    }
    return 1;
}

int X509_cmp_current_time(const ASN1_TIME *ctm)
{
    return X509_cmp_time(ctm, NULL);
}

/* returns 0 on error, otherwise 1 if ctm > cmp_time, else -1 */
int X509_cmp_time(const ASN1_TIME *ctm, time_t *cmp_time)
{
    static const size_t utctime_length = sizeof("YYMMDDHHMMSSZ") - 1;
    static const size_t generalizedtime_length = sizeof("YYYYMMDDHHMMSSZ") - 1;
    ASN1_TIME *asn1_cmp_time = NULL;
    int i, day, sec, ret = 0;
#ifdef CHARSET_EBCDIC
    const char upper_z = 0x5A;
#else
    const char upper_z = 'Z';
#endif

    /*-
     * Note that ASN.1 allows much more slack in the time format than RFC5280.
     * In RFC5280, the representation is fixed:
     * UTCTime: YYMMDDHHMMSSZ
     * GeneralizedTime: YYYYMMDDHHMMSSZ
     *
     * We do NOT currently enforce the following RFC 5280 requirement:
     * "CAs conforming to this profile MUST always encode certificate
     *  validity dates through the year 2049 as UTCTime; certificate validity
     *  dates in 2050 or later MUST be encoded as GeneralizedTime."
     */
    switch (ctm->type) {
    case V_ASN1_UTCTIME:
        if (ctm->length != (int)(utctime_length))
            return 0;
        break;
    case V_ASN1_GENERALIZEDTIME:
        if (ctm->length != (int)(generalizedtime_length))
            return 0;
        break;
    default:
        return 0;
    }

    /**
     * Verify the format: the ASN.1 functions we use below allow a more
     * flexible format than what's mandated by RFC 5280.
     * Digit and date ranges will be verified in the conversion methods.
     */
    for (i = 0; i < ctm->length - 1; i++) {
        if (!ossl_ascii_isdigit(ctm->data[i]))
            return 0;
    }
    if (ctm->data[ctm->length - 1] != upper_z)
        return 0;

    /*
     * There is ASN1_UTCTIME_cmp_time_t but no
     * ASN1_GENERALIZEDTIME_cmp_time_t or ASN1_TIME_cmp_time_t,
     * so we go through ASN.1
     */
    asn1_cmp_time = X509_time_adj(NULL, 0, cmp_time);
    if (asn1_cmp_time == NULL)
        goto err;
    if (ASN1_TIME_diff(&day, &sec, ctm, asn1_cmp_time) == 0)
        goto err;

    /*
     * X509_cmp_time comparison is <=.
     * The return value 0 is reserved for errors.
     */
    ret = (day >= 0 && sec >= 0) ? -1 : 1;

 err:
    ASN1_TIME_free(asn1_cmp_time);
    return ret;
}

/*
 * Return 0 if time should not be checked or reference time is in range,
 * or else 1 if it is past the end, or -1 if it is before the start
 */
int X509_cmp_timeframe(const X509_VERIFY_PARAM *vpm,
                       const ASN1_TIME *start, const ASN1_TIME *end)
{
    time_t ref_time;
    time_t *time = NULL;
    unsigned long flags = vpm == NULL ? 0 : X509_VERIFY_PARAM_get_flags(vpm);

    if ((flags & X509_V_FLAG_USE_CHECK_TIME) != 0) {
        ref_time = X509_VERIFY_PARAM_get_time(vpm);
        time = &ref_time;
    } else if ((flags & X509_V_FLAG_NO_CHECK_TIME) != 0) {
        return 0; /* this means ok */
    } /* else reference time is the current time */

    if (end != NULL && X509_cmp_time(end, time) < 0)
        return 1;
    if (start != NULL && X509_cmp_time(start, time) > 0)
        return -1;
    return 0;
}

ASN1_TIME *X509_gmtime_adj(ASN1_TIME *s, long adj)
{
    return X509_time_adj(s, adj, NULL);
}

ASN1_TIME *X509_time_adj(ASN1_TIME *s, long offset_sec, time_t *in_tm)
{
    return X509_time_adj_ex(s, 0, offset_sec, in_tm);
}

ASN1_TIME *X509_time_adj_ex(ASN1_TIME *s,
                            int offset_day, long offset_sec, time_t *in_tm)
{
    time_t t;

    if (in_tm)
        t = *in_tm;
    else
        time(&t);

    if (s != NULL && (s->flags & ASN1_STRING_FLAG_MSTRING) == 0) {
        if (s->type == V_ASN1_UTCTIME)
            return ASN1_UTCTIME_adj(s, t, offset_day, offset_sec);
        if (s->type == V_ASN1_GENERALIZEDTIME)
            return ASN1_GENERALIZEDTIME_adj(s, t, offset_day, offset_sec);
    }
    return ASN1_TIME_adj(s, t, offset_day, offset_sec);
}

/* Copy any missing public key parameters up the chain towards pkey */
int X509_get_pubkey_parameters(EVP_PKEY *pkey, STACK_OF(X509) *chain)
{
    EVP_PKEY *ktmp = NULL, *ktmp2;
    int i, j;

    if (pkey != NULL && !EVP_PKEY_missing_parameters(pkey))
        return 1;

    for (i = 0; i < sk_X509_num(chain); i++) {
        ktmp = X509_get0_pubkey(sk_X509_value(chain, i));
        if (ktmp == NULL) {
            ERR_raise(ERR_LIB_X509, X509_R_UNABLE_TO_GET_CERTS_PUBLIC_KEY);
            return 0;
        }
        if (!EVP_PKEY_missing_parameters(ktmp))
            break;
        ktmp = NULL;
    }
    if (ktmp == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNABLE_TO_FIND_PARAMETERS_IN_CHAIN);
        return 0;
    }

    /* first, populate the other certs */
    for (j = i - 1; j >= 0; j--) {
        ktmp2 = X509_get0_pubkey(sk_X509_value(chain, j));
        if (!EVP_PKEY_copy_parameters(ktmp2, ktmp))
            return 0;
    }

    if (pkey != NULL)
        return EVP_PKEY_copy_parameters(pkey, ktmp);
    return 1;
}

/*
 * Make a delta CRL as the difference between two full CRLs.
 * Sadly, returns NULL also on internal error.
 */
X509_CRL *X509_CRL_diff(X509_CRL *base, X509_CRL *newer,
                        EVP_PKEY *skey, const EVP_MD *md, unsigned int flags)
{
    X509_CRL *crl = NULL;
    int i;
    STACK_OF(X509_REVOKED) *revs = NULL;

    /* CRLs can't be delta already */
    if (base->base_crl_number != NULL || newer->base_crl_number != NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_CRL_ALREADY_DELTA);
        return NULL;
    }
    /* Base and new CRL must have a CRL number */
    if (base->crl_number == NULL || newer->crl_number == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_NO_CRL_NUMBER);
        return NULL;
    }
    /* Issuer names must match */
    if (X509_NAME_cmp(X509_CRL_get_issuer(base),
                      X509_CRL_get_issuer(newer)) != 0) {
        ERR_raise(ERR_LIB_X509, X509_R_ISSUER_MISMATCH);
        return NULL;
    }
    /* AKID and IDP must match */
    if (!crl_extension_match(base, newer, NID_authority_key_identifier)) {
        ERR_raise(ERR_LIB_X509, X509_R_AKID_MISMATCH);
        return NULL;
    }
    if (!crl_extension_match(base, newer, NID_issuing_distribution_point)) {
        ERR_raise(ERR_LIB_X509, X509_R_IDP_MISMATCH);
        return NULL;
    }
    /* Newer CRL number must exceed full CRL number */
    if (ASN1_INTEGER_cmp(newer->crl_number, base->crl_number) <= 0) {
        ERR_raise(ERR_LIB_X509, X509_R_NEWER_CRL_NOT_NEWER);
        return NULL;
    }
    /* CRLs must verify */
    if (skey != NULL && (X509_CRL_verify(base, skey) <= 0 ||
                         X509_CRL_verify(newer, skey) <= 0)) {
        ERR_raise(ERR_LIB_X509, X509_R_CRL_VERIFY_FAILURE);
        return NULL;
    }
    /* Create new CRL */
    crl = X509_CRL_new_ex(base->libctx, base->propq);
    if (crl == NULL || !X509_CRL_set_version(crl, X509_CRL_VERSION_2)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }
    /* Set issuer name */
    if (!X509_CRL_set_issuer_name(crl, X509_CRL_get_issuer(newer))) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }

    if (!X509_CRL_set1_lastUpdate(crl, X509_CRL_get0_lastUpdate(newer))) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }
    if (!X509_CRL_set1_nextUpdate(crl, X509_CRL_get0_nextUpdate(newer))) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }

    /* Set base CRL number: must be critical */
    if (X509_CRL_add1_ext_i2d(crl, NID_delta_crl, base->crl_number, 1, 0) <= 0) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }

    /*
     * Copy extensions across from newest CRL to delta: this will set CRL
     * number to correct value too.
     */
    for (i = 0; i < X509_CRL_get_ext_count(newer); i++) {
        X509_EXTENSION *ext = X509_CRL_get_ext(newer, i);

        if (!X509_CRL_add_ext(crl, ext, -1)) {
            ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
            goto err;
        }
    }

    /* Go through revoked entries, copying as needed */
    revs = X509_CRL_get_REVOKED(newer);

    for (i = 0; i < sk_X509_REVOKED_num(revs); i++) {
        X509_REVOKED *rvn, *rvtmp;

        rvn = sk_X509_REVOKED_value(revs, i);
        /*
         * Add only if not also in base.
         * Need something cleverer here for some more complex CRLs covering
         * multiple CAs.
         */
        if (!X509_CRL_get0_by_serial(base, &rvtmp, &rvn->serialNumber)) {
            rvtmp = X509_REVOKED_dup(rvn);
            if (rvtmp == NULL) {
                ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
                goto err;
            }
            if (!X509_CRL_add0_revoked(crl, rvtmp)) {
                X509_REVOKED_free(rvtmp);
                ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
                goto err;
            }
        }
    }

    if (skey != NULL && md != NULL && !X509_CRL_sign(crl, skey, md)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }

    return crl;

 err:
    X509_CRL_free(crl);
    return NULL;
}

int X509_STORE_CTX_set_ex_data(X509_STORE_CTX *ctx, int idx, void *data)
{
    return CRYPTO_set_ex_data(&ctx->ex_data, idx, data);
}

void *X509_STORE_CTX_get_ex_data(const X509_STORE_CTX *ctx, int idx)
{
    return CRYPTO_get_ex_data(&ctx->ex_data, idx);
}

int X509_STORE_CTX_get_error(const X509_STORE_CTX *ctx)
{
    return ctx->error;
}

void X509_STORE_CTX_set_error(X509_STORE_CTX *ctx, int err)
{
    ctx->error = err;
}

int X509_STORE_CTX_get_error_depth(const X509_STORE_CTX *ctx)
{
    return ctx->error_depth;
}

void X509_STORE_CTX_set_error_depth(X509_STORE_CTX *ctx, int depth)
{
    ctx->error_depth = depth;
}

X509 *X509_STORE_CTX_get_current_cert(const X509_STORE_CTX *ctx)
{
    return ctx->current_cert;
}

void X509_STORE_CTX_set_current_cert(X509_STORE_CTX *ctx, X509 *x)
{
    ctx->current_cert = x;
}

STACK_OF(X509) *X509_STORE_CTX_get0_chain(const X509_STORE_CTX *ctx)
{
    return ctx->chain;
}

STACK_OF(X509) *X509_STORE_CTX_get1_chain(const X509_STORE_CTX *ctx)
{
    if (ctx->chain == NULL)
        return NULL;
    return X509_chain_up_ref(ctx->chain);
}

X509 *X509_STORE_CTX_get0_current_issuer(const X509_STORE_CTX *ctx)
{
    return ctx->current_issuer;
}

X509_CRL *X509_STORE_CTX_get0_current_crl(const X509_STORE_CTX *ctx)
{
    return ctx->current_crl;
}

X509_STORE_CTX *X509_STORE_CTX_get0_parent_ctx(const X509_STORE_CTX *ctx)
{
    return ctx->parent;
}

void X509_STORE_CTX_set_cert(X509_STORE_CTX *ctx, X509 *x)
{
    ctx->cert = x;
}

void X509_STORE_CTX_set0_rpk(X509_STORE_CTX *ctx, EVP_PKEY *rpk)
{
    ctx->rpk = rpk;
}

void X509_STORE_CTX_set0_crls(X509_STORE_CTX *ctx, STACK_OF(X509_CRL) *sk)
{
    ctx->crls = sk;
}

int X509_STORE_CTX_set_purpose(X509_STORE_CTX *ctx, int purpose)
{
    /*
     * XXX: Why isn't this function always used to set the associated trust?
     * Should there even be a VPM->trust field at all?  Or should the trust
     * always be inferred from the purpose by X509_STORE_CTX_init().
     */
    return X509_STORE_CTX_purpose_inherit(ctx, 0, purpose, 0);
}

int X509_STORE_CTX_set_trust(X509_STORE_CTX *ctx, int trust)
{
    /*
     * XXX: See above, this function would only be needed when the default
     * trust for the purpose needs an override in a corner case.
     */
    return X509_STORE_CTX_purpose_inherit(ctx, 0, 0, trust);
}

/*
 * This function is used to set the X509_STORE_CTX purpose and trust values.
 * This is intended to be used when another structure has its own trust and
 * purpose values which (if set) will be inherited by the ctx. If they aren't
 * set then we will usually have a default purpose in mind which should then
 * be used to set the trust value. An example of this is SSL use: an SSL
 * structure will have its own purpose and trust settings which the
 * application can set: if they aren't set then we use the default of SSL
 * client/server.
 */
int X509_STORE_CTX_purpose_inherit(X509_STORE_CTX *ctx, int def_purpose,
                                   int purpose, int trust)
{
    int idx;

    /* If purpose not set use default */
    if (purpose == 0)
        purpose = def_purpose;
    /*
     * If purpose is set but we don't have a default then set the default to
     * the current purpose
     */
    else if (def_purpose == 0)
        def_purpose = purpose;
    /* If we have a purpose then check it is valid */
    if (purpose != 0) {
        X509_PURPOSE *ptmp;

        idx = X509_PURPOSE_get_by_id(purpose);
        if (idx == -1) {
            ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_PURPOSE_ID);
            return 0;
        }
        ptmp = X509_PURPOSE_get0(idx);
        if (ptmp->trust == X509_TRUST_DEFAULT) {
            idx = X509_PURPOSE_get_by_id(def_purpose);
            if (idx == -1) {
                ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_PURPOSE_ID);
                return 0;
            }
            ptmp = X509_PURPOSE_get0(idx);
        }
        /* If trust not set then get from purpose default */
        if (trust == 0)
            trust = ptmp->trust;
    }
    if (trust != 0) {
        idx = X509_TRUST_get_by_id(trust);
        if (idx == -1) {
            ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_TRUST_ID);
            return 0;
        }
    }

    if (ctx->param->purpose == 0 && purpose != 0)
        ctx->param->purpose = purpose;
    if (ctx->param->trust == 0 && trust != 0)
        ctx->param->trust = trust;
    return 1;
}

X509_STORE_CTX *X509_STORE_CTX_new_ex(OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_STORE_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx == NULL)
        return NULL;

    ctx->libctx = libctx;
    if (propq != NULL) {
        ctx->propq = OPENSSL_strdup(propq);
        if (ctx->propq == NULL) {
            OPENSSL_free(ctx);
            return NULL;
        }
    }

    return ctx;
}

X509_STORE_CTX *X509_STORE_CTX_new(void)
{
    return X509_STORE_CTX_new_ex(NULL, NULL);
}

void X509_STORE_CTX_free(X509_STORE_CTX *ctx)
{
    if (ctx == NULL)
        return;

    X509_STORE_CTX_cleanup(ctx);

    /* libctx and propq survive X509_STORE_CTX_cleanup() */
    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx);
}


int X509_STORE_CTX_init_rpk(X509_STORE_CTX *ctx, X509_STORE *store, EVP_PKEY *rpk)
{
    if (!X509_STORE_CTX_init(ctx, store, NULL, NULL))
        return 0;
    ctx->rpk = rpk;
    return 1;
}

int X509_STORE_CTX_init(X509_STORE_CTX *ctx, X509_STORE *store, X509 *x509,
                        STACK_OF(X509) *chain)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    X509_STORE_CTX_cleanup(ctx);

    ctx->store = store;
    ctx->cert = x509;
    ctx->untrusted = chain;
    ctx->crls = NULL;
    ctx->num_untrusted = 0;
    ctx->other_ctx = NULL;
    ctx->valid = 0;
    ctx->chain = NULL;
    ctx->error = X509_V_OK;
    ctx->explicit_policy = 0;
    ctx->error_depth = 0;
    ctx->current_cert = NULL;
    ctx->current_issuer = NULL;
    ctx->current_crl = NULL;
    ctx->current_crl_score = 0;
    ctx->current_reasons = 0;
    ctx->tree = NULL;
    ctx->parent = NULL;
    ctx->dane = NULL;
    ctx->bare_ta_signed = 0;
    ctx->rpk = NULL;
    /* Zero ex_data to make sure we're cleanup-safe */
    memset(&ctx->ex_data, 0, sizeof(ctx->ex_data));

    /* store->cleanup is always 0 in OpenSSL, if set must be idempotent */
    if (store != NULL)
        ctx->cleanup = store->cleanup;
    else
        ctx->cleanup = NULL;

    if (store != NULL && store->check_issued != NULL)
        ctx->check_issued = store->check_issued;
    else
        ctx->check_issued = check_issued;

    if (store != NULL && store->get_issuer != NULL)
        ctx->get_issuer = store->get_issuer;
    else
        ctx->get_issuer = X509_STORE_CTX_get1_issuer;

    if (store != NULL && store->verify_cb != NULL)
        ctx->verify_cb = store->verify_cb;
    else
        ctx->verify_cb = null_callback;

    if (store != NULL && store->verify != NULL)
        ctx->verify = store->verify;
    else
        ctx->verify = internal_verify;

    if (store != NULL && store->check_revocation != NULL)
        ctx->check_revocation = store->check_revocation;
    else
        ctx->check_revocation = check_revocation;

    if (store != NULL && store->get_crl != NULL)
        ctx->get_crl = store->get_crl;
    else
        ctx->get_crl = NULL;

    if (store != NULL && store->check_crl != NULL)
        ctx->check_crl = store->check_crl;
    else
        ctx->check_crl = check_crl;

    if (store != NULL && store->cert_crl != NULL)
        ctx->cert_crl = store->cert_crl;
    else
        ctx->cert_crl = cert_crl;

    if (store != NULL && store->check_policy != NULL)
        ctx->check_policy = store->check_policy;
    else
        ctx->check_policy = check_policy;

    if (store != NULL && store->lookup_certs != NULL)
        ctx->lookup_certs = store->lookup_certs;
    else
        ctx->lookup_certs = X509_STORE_CTX_get1_certs;

    if (store != NULL && store->lookup_crls != NULL)
        ctx->lookup_crls = store->lookup_crls;
    else
        ctx->lookup_crls = X509_STORE_CTX_get1_crls;

    ctx->param = X509_VERIFY_PARAM_new();
    if (ctx->param == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        goto err;
    }

    /* Inherit callbacks and flags from X509_STORE if not set use defaults. */
    if (store == NULL)
        ctx->param->inh_flags |= X509_VP_FLAG_DEFAULT | X509_VP_FLAG_ONCE;
    else if (X509_VERIFY_PARAM_inherit(ctx->param, store->param) == 0)
        goto err;

    if (!X509_STORE_CTX_set_default(ctx, "default"))
        goto err;

    /*
     * XXX: For now, continue to inherit trust from VPM, but infer from the
     * purpose if this still yields the default value.
     */
    if (ctx->param->trust == X509_TRUST_DEFAULT) {
        int idx = X509_PURPOSE_get_by_id(ctx->param->purpose);
        X509_PURPOSE *xp = X509_PURPOSE_get0(idx);

        if (xp != NULL)
            ctx->param->trust = X509_PURPOSE_get_trust(xp);
    }

    if (CRYPTO_new_ex_data(CRYPTO_EX_INDEX_X509_STORE_CTX, ctx,
                           &ctx->ex_data))
        return 1;
    ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);

 err:
    /*
     * On error clean up allocated storage, if the store context was not
     * allocated with X509_STORE_CTX_new() this is our last chance to do so.
     */
    X509_STORE_CTX_cleanup(ctx);
    return 0;
}

/*
 * Set alternative get_issuer method: just from a STACK of trusted certificates.
 * This avoids the complexity of X509_STORE where it is not needed.
 */
void X509_STORE_CTX_set0_trusted_stack(X509_STORE_CTX *ctx, STACK_OF(X509) *sk)
{
    ctx->other_ctx = sk;
    ctx->get_issuer = get1_best_issuer_other_sk;
    ctx->lookup_certs = lookup_certs_sk;
}

void X509_STORE_CTX_cleanup(X509_STORE_CTX *ctx)
{
    /*
     * We need to be idempotent because, unfortunately, free() also calls
     * cleanup(), so the natural call sequence new(), init(), cleanup(), free()
     * calls cleanup() for the same object twice!  Thus we must zero the
     * pointers below after they're freed!
     */
    /* Seems to always be NULL in OpenSSL, do this at most once. */
    if (ctx->cleanup != NULL) {
        ctx->cleanup(ctx);
        ctx->cleanup = NULL;
    }
    if (ctx->param != NULL) {
        if (ctx->parent == NULL)
            X509_VERIFY_PARAM_free(ctx->param);
        ctx->param = NULL;
    }
    X509_policy_tree_free(ctx->tree);
    ctx->tree = NULL;
    OSSL_STACK_OF_X509_free(ctx->chain);
    ctx->chain = NULL;
    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_X509_STORE_CTX, ctx, &(ctx->ex_data));
    memset(&ctx->ex_data, 0, sizeof(ctx->ex_data));
}

void X509_STORE_CTX_set_depth(X509_STORE_CTX *ctx, int depth)
{
    X509_VERIFY_PARAM_set_depth(ctx->param, depth);
}

void X509_STORE_CTX_set_flags(X509_STORE_CTX *ctx, unsigned long flags)
{
    X509_VERIFY_PARAM_set_flags(ctx->param, flags);
}

void X509_STORE_CTX_set_time(X509_STORE_CTX *ctx, unsigned long flags,
                             time_t t)
{
    X509_VERIFY_PARAM_set_time(ctx->param, t);
}

void X509_STORE_CTX_set_current_reasons(X509_STORE_CTX *ctx,
                                        unsigned int current_reasons)
{
   ctx->current_reasons = current_reasons;
}

X509 *X509_STORE_CTX_get0_cert(const X509_STORE_CTX *ctx)
{
    return ctx->cert;
}

EVP_PKEY *X509_STORE_CTX_get0_rpk(const X509_STORE_CTX *ctx)
{
    return ctx->rpk;
}

STACK_OF(X509) *X509_STORE_CTX_get0_untrusted(const X509_STORE_CTX *ctx)
{
    return ctx->untrusted;
}

void X509_STORE_CTX_set0_untrusted(X509_STORE_CTX *ctx, STACK_OF(X509) *sk)
{
    ctx->untrusted = sk;
}

void X509_STORE_CTX_set0_verified_chain(X509_STORE_CTX *ctx, STACK_OF(X509) *sk)
{
    OSSL_STACK_OF_X509_free(ctx->chain);
    ctx->chain = sk;
}

void X509_STORE_CTX_set_verify_cb(X509_STORE_CTX *ctx,
                                  X509_STORE_CTX_verify_cb verify_cb)
{
    ctx->verify_cb = verify_cb;
}

X509_STORE_CTX_verify_cb X509_STORE_CTX_get_verify_cb(const X509_STORE_CTX *ctx)
{
    return ctx->verify_cb;
}

void X509_STORE_CTX_set_verify(X509_STORE_CTX *ctx,
                               X509_STORE_CTX_verify_fn verify)
{
    ctx->verify = verify;
}

X509_STORE_CTX_verify_fn X509_STORE_CTX_get_verify(const X509_STORE_CTX *ctx)
{
    return ctx->verify;
}

X509_STORE_CTX_get_issuer_fn
X509_STORE_CTX_get_get_issuer(const X509_STORE_CTX *ctx)
{
    return ctx->get_issuer;
}

X509_STORE_CTX_check_issued_fn
X509_STORE_CTX_get_check_issued(const X509_STORE_CTX *ctx)
{
    return ctx->check_issued;
}

X509_STORE_CTX_check_revocation_fn
X509_STORE_CTX_get_check_revocation(const X509_STORE_CTX *ctx)
{
    return ctx->check_revocation;
}

X509_STORE_CTX_get_crl_fn X509_STORE_CTX_get_get_crl(const X509_STORE_CTX *ctx)
{
    return ctx->get_crl;
}

void X509_STORE_CTX_set_get_crl(X509_STORE_CTX *ctx,
                                X509_STORE_CTX_get_crl_fn get_crl)
{
   ctx->get_crl = get_crl;
}

X509_STORE_CTX_check_crl_fn
X509_STORE_CTX_get_check_crl(const X509_STORE_CTX *ctx)
{
    return ctx->check_crl;
}

X509_STORE_CTX_cert_crl_fn
X509_STORE_CTX_get_cert_crl(const X509_STORE_CTX *ctx)
{
    return ctx->cert_crl;
}

X509_STORE_CTX_check_policy_fn
X509_STORE_CTX_get_check_policy(const X509_STORE_CTX *ctx)
{
    return ctx->check_policy;
}

X509_STORE_CTX_lookup_certs_fn
X509_STORE_CTX_get_lookup_certs(const X509_STORE_CTX *ctx)
{
    return ctx->lookup_certs;
}

X509_STORE_CTX_lookup_crls_fn
X509_STORE_CTX_get_lookup_crls(const X509_STORE_CTX *ctx)
{
    return ctx->lookup_crls;
}

X509_STORE_CTX_cleanup_fn X509_STORE_CTX_get_cleanup(const X509_STORE_CTX *ctx)
{
    return ctx->cleanup;
}

X509_POLICY_TREE *X509_STORE_CTX_get0_policy_tree(const X509_STORE_CTX *ctx)
{
    return ctx->tree;
}

int X509_STORE_CTX_get_explicit_policy(const X509_STORE_CTX *ctx)
{
    return ctx->explicit_policy;
}

int X509_STORE_CTX_get_num_untrusted(const X509_STORE_CTX *ctx)
{
    return ctx->num_untrusted;
}

int X509_STORE_CTX_set_default(X509_STORE_CTX *ctx, const char *name)
{
    const X509_VERIFY_PARAM *param;

    param = X509_VERIFY_PARAM_lookup(name);
    if (param == NULL) {
        ERR_raise_data(ERR_LIB_X509, X509_R_UNKNOWN_PURPOSE_ID, "name=%s", name);
        return 0;
    }
    return X509_VERIFY_PARAM_inherit(ctx->param, param);
}

X509_VERIFY_PARAM *X509_STORE_CTX_get0_param(const X509_STORE_CTX *ctx)
{
    return ctx->param;
}

void X509_STORE_CTX_set0_param(X509_STORE_CTX *ctx, X509_VERIFY_PARAM *param)
{
    X509_VERIFY_PARAM_free(ctx->param);
    ctx->param = param;
}

void X509_STORE_CTX_set0_dane(X509_STORE_CTX *ctx, SSL_DANE *dane)
{
    ctx->dane = dane;
}

static unsigned char *dane_i2d(X509 *cert, uint8_t selector,
                               unsigned int *i2dlen)
{
    unsigned char *buf = NULL;
    int len;

    /*
     * Extract ASN.1 DER form of certificate or public key.
     */
    switch (selector) {
    case DANETLS_SELECTOR_CERT:
        len = i2d_X509(cert, &buf);
        break;
    case DANETLS_SELECTOR_SPKI:
        len = i2d_X509_PUBKEY(X509_get_X509_PUBKEY(cert), &buf);
        break;
    default:
        ERR_raise(ERR_LIB_X509, X509_R_BAD_SELECTOR);
        return NULL;
    }

    if (len < 0 || buf == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_ASN1_LIB);
        return NULL;
    }

    *i2dlen = (unsigned int)len;
    return buf;
}

#define DANETLS_NONE 256 /* impossible uint8_t */

/* Returns -1 on internal error */
static int dane_match_cert(X509_STORE_CTX *ctx, X509 *cert, int depth)
{
    SSL_DANE *dane = ctx->dane;
    unsigned usage = DANETLS_NONE;
    unsigned selector = DANETLS_NONE;
    unsigned ordinal = DANETLS_NONE;
    unsigned mtype = DANETLS_NONE;
    unsigned char *i2dbuf = NULL;
    unsigned int i2dlen = 0;
    unsigned char mdbuf[EVP_MAX_MD_SIZE];
    unsigned char *cmpbuf = NULL;
    unsigned int cmplen = 0;
    int i;
    int recnum;
    int matched = 0;
    danetls_record *t = NULL;
    uint32_t mask;

    mask = (depth == 0) ? DANETLS_EE_MASK : DANETLS_TA_MASK;

    /* The trust store is not applicable with DANE-TA(2) */
    if (depth >= ctx->num_untrusted)
        mask &= DANETLS_PKIX_MASK;

    /*
     * If we've previously matched a PKIX-?? record, no need to test any
     * further PKIX-?? records, it remains to just build the PKIX chain.
     * Had the match been a DANE-?? record, we'd be done already.
     */
    if (dane->mdpth >= 0)
        mask &= ~DANETLS_PKIX_MASK;

    /*-
     * https://tools.ietf.org/html/rfc7671#section-5.1
     * https://tools.ietf.org/html/rfc7671#section-5.2
     * https://tools.ietf.org/html/rfc7671#section-5.3
     * https://tools.ietf.org/html/rfc7671#section-5.4
     *
     * We handle DANE-EE(3) records first as they require no chain building
     * and no expiration or hostname checks.  We also process digests with
     * higher ordinals first and ignore lower priorities except Full(0) which
     * is always processed (last).  If none match, we then process PKIX-EE(1).
     *
     * NOTE: This relies on DANE usages sorting before the corresponding PKIX
     * usages in SSL_dane_tlsa_add(), and also on descending sorting of digest
     * priorities.  See twin comment in ssl/ssl_lib.c.
     *
     * We expect that most TLSA RRsets will have just a single usage, so we
     * don't go out of our way to cache multiple selector-specific i2d buffers
     * across usages, but if the selector happens to remain the same as switch
     * usages, that's OK.  Thus, a set of "3 1 1", "3 0 1", "1 1 1", "1 0 1",
     * records would result in us generating each of the certificate and public
     * key DER forms twice, but more typically we'd just see multiple "3 1 1"
     * or multiple "3 0 1" records.
     *
     * As soon as we find a match at any given depth, we stop, because either
     * we've matched a DANE-?? record and the peer is authenticated, or, after
     * exhausting all DANE-?? records, we've matched a PKIX-?? record, which is
     * sufficient for DANE, and what remains to do is ordinary PKIX validation.
     */
    recnum = (dane->umask & mask) != 0 ? sk_danetls_record_num(dane->trecs) : 0;
    for (i = 0; matched == 0 && i < recnum; ++i) {
        t = sk_danetls_record_value(dane->trecs, i);
        if ((DANETLS_USAGE_BIT(t->usage) & mask) == 0)
            continue;
        if (t->usage != usage) {
            usage = t->usage;

            /* Reset digest agility for each usage/selector pair */
            mtype = DANETLS_NONE;
            ordinal = dane->dctx->mdord[t->mtype];
        }
        if (t->selector != selector) {
            selector = t->selector;

            /* Update per-selector state */
            OPENSSL_free(i2dbuf);
            i2dbuf = dane_i2d(cert, selector, &i2dlen);
            if (i2dbuf == NULL)
                return -1;

            /* Reset digest agility for each usage/selector pair */
            mtype = DANETLS_NONE;
            ordinal = dane->dctx->mdord[t->mtype];
        } else if (t->mtype != DANETLS_MATCHING_FULL) {
            /*-
             * Digest agility:
             *
             *     <https://tools.ietf.org/html/rfc7671#section-9>
             *
             * For a fixed selector, after processing all records with the
             * highest mtype ordinal, ignore all mtypes with lower ordinals
             * other than "Full".
             */
            if (dane->dctx->mdord[t->mtype] < ordinal)
                continue;
        }

        /*
         * Each time we hit a (new selector or) mtype, re-compute the relevant
         * digest, more complex caching is not worth the code space.
         */
        if (t->mtype != mtype) {
            const EVP_MD *md = dane->dctx->mdevp[mtype = t->mtype];

            cmpbuf = i2dbuf;
            cmplen = i2dlen;

            if (md != NULL) {
                cmpbuf = mdbuf;
                if (!EVP_Digest(i2dbuf, i2dlen, cmpbuf, &cmplen, md, 0)) {
                    matched = -1;
                    break;
                }
            }
        }

        /*
         * Squirrel away the certificate and depth if we have a match.  Any
         * DANE match is dispositive, but with PKIX we still need to build a
         * full chain.
         */
        if (cmplen == t->dlen &&
            memcmp(cmpbuf, t->data, cmplen) == 0) {
            if (DANETLS_USAGE_BIT(usage) & DANETLS_DANE_MASK)
                matched = 1;
            if (matched || dane->mdpth < 0) {
                if (!X509_up_ref(cert)) {
                    matched = -1;
                    break;
                }

                OPENSSL_free(dane->mcert);
                dane->mcert = cert;
                dane->mdpth = depth;
                dane->mtlsa = t;
            }
            break;
        }
    }

    /* Clear the one-element DER cache */
    OPENSSL_free(i2dbuf);
    return matched;
}

/* Returns -1 on internal error */
static int check_dane_issuer(X509_STORE_CTX *ctx, int depth)
{
    SSL_DANE *dane = ctx->dane;
    int matched = 0;
    X509 *cert;

    if (!DANETLS_HAS_TA(dane) || depth == 0)
        return X509_TRUST_UNTRUSTED;

    /*
     * Record any DANE trust anchor matches, for the first depth to test, if
     * there's one at that depth. (This'll be false for length 1 chains looking
     * for an exact match for the leaf certificate).
     */
    cert = sk_X509_value(ctx->chain, depth);
    if (cert != NULL && (matched = dane_match_cert(ctx, cert, depth)) < 0)
        return matched;
    if (matched > 0) {
        ctx->num_untrusted = depth - 1;
        return X509_TRUST_TRUSTED;
    }

    return X509_TRUST_UNTRUSTED;
}

static int check_dane_pkeys(X509_STORE_CTX *ctx)
{
    SSL_DANE *dane = ctx->dane;
    danetls_record *t;
    int num = ctx->num_untrusted;
    X509 *cert = sk_X509_value(ctx->chain, num - 1);
    int recnum = sk_danetls_record_num(dane->trecs);
    int i;

    for (i = 0; i < recnum; ++i) {
        t = sk_danetls_record_value(dane->trecs, i);
        if (t->usage != DANETLS_USAGE_DANE_TA ||
            t->selector != DANETLS_SELECTOR_SPKI ||
            t->mtype != DANETLS_MATCHING_FULL ||
            X509_verify(cert, t->spki) <= 0)
            continue;

        /* Clear any PKIX-?? matches that failed to extend to a full chain */
        X509_free(dane->mcert);
        dane->mcert = NULL;

        /* Record match via a bare TA public key */
        ctx->bare_ta_signed = 1;
        dane->mdpth = num - 1;
        dane->mtlsa = t;

        /* Prune any excess chain certificates */
        num = sk_X509_num(ctx->chain);
        for (; num > ctx->num_untrusted; --num)
            X509_free(sk_X509_pop(ctx->chain));

        return X509_TRUST_TRUSTED;
    }

    return X509_TRUST_UNTRUSTED;
}

/*
 * Only DANE-EE and SPKI are supported
 * Returns -1 on internal error
 */
static int dane_match_rpk(X509_STORE_CTX *ctx, EVP_PKEY *rpk)
{
    SSL_DANE *dane = ctx->dane;
    danetls_record *t = NULL;
    int mtype = DANETLS_MATCHING_FULL;
    unsigned char *i2dbuf = NULL;
    unsigned int i2dlen = 0;
    unsigned char mdbuf[EVP_MAX_MD_SIZE];
    unsigned char *cmpbuf;
    unsigned int cmplen = 0;
    int len;
    int recnum = sk_danetls_record_num(dane->trecs);
    int i;
    int matched = 0;

    /* Calculate ASN.1 DER of RPK */
    if ((len = i2d_PUBKEY(rpk, &i2dbuf)) <= 0)
        return -1;
    cmplen = i2dlen = (unsigned int)len;
    cmpbuf = i2dbuf;

    for (i = 0; i < recnum; i++) {
        t = sk_danetls_record_value(dane->trecs, i);
        if (t->usage != DANETLS_USAGE_DANE_EE || t->selector != DANETLS_SELECTOR_SPKI)
            continue;

        /* Calculate hash - keep only one around */
        if (t->mtype != mtype) {
            const EVP_MD *md = dane->dctx->mdevp[mtype = t->mtype];

            cmpbuf = i2dbuf;
            cmplen = i2dlen;

            if (md != NULL) {
                cmpbuf = mdbuf;
                if (!EVP_Digest(i2dbuf, i2dlen, cmpbuf, &cmplen, md, 0)) {
                    matched = -1;
                    break;
                }
            }
        }
        if (cmplen == t->dlen && memcmp(cmpbuf, t->data, cmplen) == 0) {
            matched = 1;
            dane->mdpth = 0;
            dane->mtlsa = t;
            break;
        }
    }
    OPENSSL_free(i2dbuf);
    return matched;
}

static void dane_reset(SSL_DANE *dane)
{
    /* Reset state to verify another chain, or clear after failure. */
    X509_free(dane->mcert);
    dane->mcert = NULL;
    dane->mtlsa = NULL;
    dane->mdpth = -1;
    dane->pdpth = -1;
}

/* Sadly, returns 0 also on internal error in ctx->verify_cb(). */
static int check_leaf_suiteb(X509_STORE_CTX *ctx, X509 *cert)
{
    int err = X509_chain_check_suiteb(NULL, cert, NULL, ctx->param->flags);

    CB_FAIL_IF(err != X509_V_OK, ctx, cert, 0, err);
    return 1;
}

/* Returns -1 on internal error */
static int dane_verify_rpk(X509_STORE_CTX *ctx)
{
    SSL_DANE *dane = ctx->dane;
    int matched;

    dane_reset(dane);

    /*
     * Look for a DANE record for RPK
     * If error, return -1
     * If found, call ctx->verify_cb(1, ctx)
     * If not found call ctx->verify_cb(0, ctx)
     */
    matched = dane_match_rpk(ctx, ctx->rpk);
    ctx->error_depth = 0;

    if (matched < 0) {
        ctx->error = X509_V_ERR_UNSPECIFIED;
        return -1;
    }

    if (matched > 0)
        ctx->error = X509_V_OK;
    else
        ctx->error = X509_V_ERR_DANE_NO_MATCH;

    return verify_rpk(ctx);
}

/* Returns -1 on internal error */
static int dane_verify(X509_STORE_CTX *ctx)
{
    X509 *cert = ctx->cert;
    SSL_DANE *dane = ctx->dane;
    int matched;
    int done;

    dane_reset(dane);

    /*-
     * When testing the leaf certificate, if we match a DANE-EE(3) record,
     * dane_match() returns 1 and we're done.  If however we match a PKIX-EE(1)
     * record, the match depth and matching TLSA record are recorded, but the
     * return value is 0, because we still need to find a PKIX trust anchor.
     * Therefore, when DANE authentication is enabled (required), we're done
     * if:
     *   + matched < 0, internal error.
     *   + matched == 1, we matched a DANE-EE(3) record
     *   + matched == 0, mdepth < 0 (no PKIX-EE match) and there are no
     *     DANE-TA(2) or PKIX-TA(0) to test.
     */
    matched = dane_match_cert(ctx, ctx->cert, 0);
    done = matched != 0 || (!DANETLS_HAS_TA(dane) && dane->mdpth < 0);

    if (done && !X509_get_pubkey_parameters(NULL, ctx->chain))
        return -1;

    if (matched > 0) {
        /* Callback invoked as needed */
        if (!check_leaf_suiteb(ctx, cert))
            return 0;
        /* Callback invoked as needed */
        if ((dane->flags & DANE_FLAG_NO_DANE_EE_NAMECHECKS) == 0 &&
            !check_id(ctx))
            return 0;
        /* Bypass internal_verify(), issue depth 0 success callback */
        ctx->error_depth = 0;
        ctx->current_cert = cert;
        return ctx->verify_cb(1, ctx);
    }

    if (matched < 0) {
        ctx->error_depth = 0;
        ctx->current_cert = cert;
        ctx->error = X509_V_ERR_OUT_OF_MEM;
        return -1;
    }

    if (done) {
        /* Fail early, TA-based success is not possible */
        if (!check_leaf_suiteb(ctx, cert))
            return 0;
        return verify_cb_cert(ctx, cert, 0, X509_V_ERR_DANE_NO_MATCH);
    }

    /*
     * Chain verification for usages 0/1/2.  TLSA record matching of depth > 0
     * certificates happens in-line with building the rest of the chain.
     */
    return verify_chain(ctx);
}

/*
 * Get trusted issuer, without duplicate suppression
 * Returns -1 on internal error.
 */
static int get1_trusted_issuer(X509 **issuer, X509_STORE_CTX *ctx, X509 *cert)
{
    STACK_OF(X509) *saved_chain = ctx->chain;
    int ok;

    ctx->chain = NULL;
    ok = ctx->get_issuer(issuer, ctx, cert);
    ctx->chain = saved_chain;

    return ok;
}

/*-
 * Returns -1 on internal error.
 * Sadly, returns 0 also on internal error in ctx->verify_cb().
 */
static int build_chain(X509_STORE_CTX *ctx)
{
    SSL_DANE *dane = ctx->dane;
    int num = sk_X509_num(ctx->chain);
    STACK_OF(X509) *sk_untrusted = NULL;
    unsigned int search;
    int may_trusted = 0;
    int may_alternate = 0;
    int trust = X509_TRUST_UNTRUSTED;
    int alt_untrusted = 0;
    int max_depth;
    int ok = 0;
    int i;

    /* Our chain starts with a single untrusted element. */
    if (!ossl_assert(num == 1 && ctx->num_untrusted == num))
        goto int_err;

#define S_DOUNTRUSTED (1 << 0) /* Search untrusted chain */
#define S_DOTRUSTED   (1 << 1) /* Search trusted store */
#define S_DOALTERNATE (1 << 2) /* Retry with pruned alternate chain */
    /*
     * Set up search policy, untrusted if possible, trusted-first if enabled,
     * which is the default.
     * If we're doing DANE and not doing PKIX-TA/PKIX-EE, we never look in the
     * trust_store, otherwise we might look there first.  If not trusted-first,
     * and alternate chains are not disabled, try building an alternate chain
     * if no luck with untrusted first.
     */
    search = ctx->untrusted != NULL ? S_DOUNTRUSTED : 0;
    if (DANETLS_HAS_PKIX(dane) || !DANETLS_HAS_DANE(dane)) {
        if (search == 0 || (ctx->param->flags & X509_V_FLAG_TRUSTED_FIRST) != 0)
            search |= S_DOTRUSTED;
        else if (!(ctx->param->flags & X509_V_FLAG_NO_ALT_CHAINS))
            may_alternate = 1;
        may_trusted = 1;
    }

    /* Initialize empty untrusted stack. */
    if ((sk_untrusted = sk_X509_new_null()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto memerr;
    }

    /*
     * If we got any "Cert(0) Full(0)" trust anchors from DNS, *prepend* them
     * to our working copy of the untrusted certificate stack.
     */
    if (DANETLS_ENABLED(dane) && dane->certs != NULL
        && !X509_add_certs(sk_untrusted, dane->certs, X509_ADD_FLAG_DEFAULT)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto memerr;
    }

    /*
     * Shallow-copy the stack of untrusted certificates (with TLS, this is
     * typically the content of the peer's certificate message) so we can make
     * multiple passes over it, while free to remove elements as we go.
     */
    if (!X509_add_certs(sk_untrusted, ctx->untrusted, X509_ADD_FLAG_DEFAULT)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto memerr;
    }

    /*
     * Still absurdly large, but arithmetically safe, a lower hard upper bound
     * might be reasonable.
     */
    if (ctx->param->depth > INT_MAX / 2)
        ctx->param->depth = INT_MAX / 2;

    /*
     * Try to extend the chain until we reach an ultimately trusted issuer.
     * Build chains up to one longer the limit, later fail if we hit the limit,
     * with an X509_V_ERR_CERT_CHAIN_TOO_LONG error code.
     */
    max_depth = ctx->param->depth + 1;

    while (search != 0) {
        X509 *curr, *issuer = NULL;

        num = sk_X509_num(ctx->chain);
        ctx->error_depth = num - 1;
        /*
         * Look in the trust store if enabled for first lookup, or we've run
         * out of untrusted issuers and search here is not disabled.  When we
         * reach the depth limit, we stop extending the chain, if by that point
         * we've not found a trust anchor, any trusted chain would be too long.
         *
         * The error reported to the application verify callback is at the
         * maximal valid depth with the current certificate equal to the last
         * not ultimately-trusted issuer.  For example, with verify_depth = 0,
         * the callback will report errors at depth=1 when the immediate issuer
         * of the leaf certificate is not a trust anchor.  No attempt will be
         * made to locate an issuer for that certificate, since such a chain
         * would be a-priori too long.
         */
        if ((search & S_DOTRUSTED) != 0) {
            i = num;
            if ((search & S_DOALTERNATE) != 0) {
                /*
                 * As high up the chain as we can, look for an alternative
                 * trusted issuer of an untrusted certificate that currently
                 * has an untrusted issuer.  We use the alt_untrusted variable
                 * to track how far up the chain we find the first match.  It
                 * is only if and when we find a match, that we prune the chain
                 * and reset ctx->num_untrusted to the reduced count of
                 * untrusted certificates.  While we're searching for such a
                 * match (which may never be found), it is neither safe nor
                 * wise to preemptively modify either the chain or
                 * ctx->num_untrusted.
                 *
                 * Note, like ctx->num_untrusted, alt_untrusted is a count of
                 * untrusted certificates, not a "depth".
                 */
                i = alt_untrusted;
            }
            curr = sk_X509_value(ctx->chain, i - 1);

            /* Note: get1_trusted_issuer() must be used even if self-signed. */
            ok = num > max_depth ? 0 : get1_trusted_issuer(&issuer, ctx, curr);

            if (ok < 0) {
                trust = -1;
                ctx->error = X509_V_ERR_STORE_LOOKUP;
                break;
            }

            if (ok > 0) {
                int self_signed = X509_self_signed(curr, 0);

                if (self_signed < 0) {
                    X509_free(issuer);
                    goto int_err;
                }
                /*
                 * Alternative trusted issuer for a mid-chain untrusted cert?
                 * Pop the untrusted cert's successors and retry.  We might now
                 * be able to complete a valid chain via the trust store.  Note
                 * that despite the current trust store match we might still
                 * fail complete the chain to a suitable trust anchor, in which
                 * case we may prune some more untrusted certificates and try
                 * again.  Thus the S_DOALTERNATE bit may yet be turned on
                 * again with an even shorter untrusted chain!
                 *
                 * If in the process we threw away our matching PKIX-TA trust
                 * anchor, reset DANE trust.  We might find a suitable trusted
                 * certificate among the ones from the trust store.
                 */
                if ((search & S_DOALTERNATE) != 0) {
                    if (!ossl_assert(num > i && i > 0 && !self_signed)) {
                        X509_free(issuer);
                        goto int_err;
                    }
                    search &= ~S_DOALTERNATE;
                    for (; num > i; --num)
                        X509_free(sk_X509_pop(ctx->chain));
                    ctx->num_untrusted = num;

                    if (DANETLS_ENABLED(dane) &&
                        dane->mdpth >= ctx->num_untrusted) {
                        dane->mdpth = -1;
                        X509_free(dane->mcert);
                        dane->mcert = NULL;
                    }
                    if (DANETLS_ENABLED(dane) &&
                        dane->pdpth >= ctx->num_untrusted)
                        dane->pdpth = -1;
                }

                if (!self_signed) { /* untrusted not self-signed certificate */
                    /* Grow the chain by trusted issuer */
                    if (!sk_X509_push(ctx->chain, issuer)) {
                        X509_free(issuer);
                        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
                        goto memerr;
                    }
                    if ((self_signed = X509_self_signed(issuer, 0)) < 0)
                        goto int_err;
                } else {
                    /*
                     * We have a self-signed untrusted cert that has the same
                     * subject name (and perhaps keyid and/or serial number) as
                     * a trust anchor.  We must have an exact match to avoid
                     * possible impersonation via key substitution etc.
                     */
                    if (X509_cmp(curr, issuer) != 0) {
                        /* Self-signed untrusted mimic. */
                        X509_free(issuer);
                        ok = 0;
                    } else { /* curr "==" issuer */
                        /*
                         * Replace self-signed untrusted certificate
                         * by its trusted matching issuer.
                         */
                        X509_free(curr);
                        ctx->num_untrusted = --num;
                        (void)sk_X509_set(ctx->chain, num, issuer);
                    }
                }

                /*
                 * We've added a new trusted certificate to the chain, re-check
                 * trust.  If not done, and not self-signed look deeper.
                 * Whether or not we're doing "trusted first", we no longer
                 * look for untrusted certificates from the peer's chain.
                 *
                 * At this point ctx->num_trusted and num must reflect the
                 * correct number of untrusted certificates, since the DANE
                 * logic in check_trust() depends on distinguishing CAs from
                 * "the wire" from CAs from the trust store.  In particular, the
                 * certificate at depth "num" should be the new trusted
                 * certificate with ctx->num_untrusted <= num.
                 */
                if (ok) {
                    if (!ossl_assert(ctx->num_untrusted <= num))
                        goto int_err;
                    search &= ~S_DOUNTRUSTED;
                    trust = check_trust(ctx, num);
                    if (trust != X509_TRUST_UNTRUSTED)
                        break;
                    if (!self_signed)
                        continue;
                }
            }

            /*
             * No dispositive decision, and either self-signed or no match, if
             * we were doing untrusted-first, and alt-chains are not disabled,
             * do that, by repeatedly losing one untrusted element at a time,
             * and trying to extend the shorted chain.
             */
            if ((search & S_DOUNTRUSTED) == 0) {
                /* Continue search for a trusted issuer of a shorter chain? */
                if ((search & S_DOALTERNATE) != 0 && --alt_untrusted > 0)
                    continue;
                /* Still no luck and no fallbacks left? */
                if (!may_alternate || (search & S_DOALTERNATE) != 0 ||
                    ctx->num_untrusted < 2)
                    break;
                /* Search for a trusted issuer of a shorter chain */
                search |= S_DOALTERNATE;
                alt_untrusted = ctx->num_untrusted - 1;
            }
        }

        /*
         * Try to extend chain with peer-provided untrusted certificate
         */
        if ((search & S_DOUNTRUSTED) != 0) {
            num = sk_X509_num(ctx->chain);
            if (!ossl_assert(num == ctx->num_untrusted))
                goto int_err;
            curr = sk_X509_value(ctx->chain, num - 1);
            issuer = (X509_self_signed(curr, 0) > 0 || num > max_depth) ?
                NULL : get0_best_issuer_sk(ctx, 0, 1 /* no_dup */, sk_untrusted, curr);
            if (issuer == NULL) {
                /*
                 * Once we have reached a self-signed cert or num > max_depth
                 * or can't find an issuer in the untrusted list we stop looking
                 * there and start looking only in the trust store if enabled.
                 */
                search &= ~S_DOUNTRUSTED;
                if (may_trusted)
                    search |= S_DOTRUSTED;
                continue;
            }

            /* Drop this issuer from future consideration */
            (void)sk_X509_delete_ptr(sk_untrusted, issuer);

            /* Grow the chain by untrusted issuer */
            if (!X509_add_cert(ctx->chain, issuer, X509_ADD_FLAG_UP_REF))
                goto int_err;

            ++ctx->num_untrusted;

            /* Check for DANE-TA trust of the topmost untrusted certificate. */
            trust = check_dane_issuer(ctx, ctx->num_untrusted - 1);
            if (trust == X509_TRUST_TRUSTED || trust == X509_TRUST_REJECTED)
                break;
        }
    }
    sk_X509_free(sk_untrusted);

    if (trust < 0) /* internal error */
        return trust;

    /*
     * Last chance to make a trusted chain, either bare DANE-TA public-key
     * signers, or else direct leaf PKIX trust.
     */
    num = sk_X509_num(ctx->chain);
    if (num <= max_depth) {
        if (trust == X509_TRUST_UNTRUSTED && DANETLS_HAS_DANE_TA(dane))
            trust = check_dane_pkeys(ctx);
        if (trust == X509_TRUST_UNTRUSTED && num == ctx->num_untrusted)
            trust = check_trust(ctx, num);
    }

    switch (trust) {
    case X509_TRUST_TRUSTED:
        return 1;
    case X509_TRUST_REJECTED:
        /* Callback already issued */
        return 0;
    case X509_TRUST_UNTRUSTED:
    default:
        switch (ctx->error) {
        case X509_V_ERR_ERROR_IN_CERT_NOT_BEFORE_FIELD:
        case X509_V_ERR_CERT_NOT_YET_VALID:
        case X509_V_ERR_ERROR_IN_CERT_NOT_AFTER_FIELD:
        case X509_V_ERR_CERT_HAS_EXPIRED:
            return 0; /* Callback already done by ossl_x509_check_cert_time() */
        default: /* A preliminary error has become final */
            return verify_cb_cert(ctx, NULL, num - 1, ctx->error);
        case X509_V_OK:
            break;
        }
        CB_FAIL_IF(num > max_depth,
                   ctx, NULL, num - 1, X509_V_ERR_CERT_CHAIN_TOO_LONG);
        CB_FAIL_IF(DANETLS_ENABLED(dane)
                       && (!DANETLS_HAS_PKIX(dane) || dane->pdpth >= 0),
                   ctx, NULL, num - 1, X509_V_ERR_DANE_NO_MATCH);
        if (X509_self_signed(sk_X509_value(ctx->chain, num - 1), 0) > 0)
            return verify_cb_cert(ctx, NULL, num - 1,
                                  num == 1
                                  ? X509_V_ERR_DEPTH_ZERO_SELF_SIGNED_CERT
                                  : X509_V_ERR_SELF_SIGNED_CERT_IN_CHAIN);
        return verify_cb_cert(ctx, NULL, num - 1,
                              ctx->num_untrusted < num
                              ? X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT
                              : X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY);
    }

 int_err:
    ERR_raise(ERR_LIB_X509, ERR_R_INTERNAL_ERROR);
    ctx->error = X509_V_ERR_UNSPECIFIED;
    sk_X509_free(sk_untrusted);
    return -1;

 memerr:
    ctx->error = X509_V_ERR_OUT_OF_MEM;
    sk_X509_free(sk_untrusted);
    return -1;
}

STACK_OF(X509) *X509_build_chain(X509 *target, STACK_OF(X509) *certs,
                                 X509_STORE *store, int with_self_signed,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    int finish_chain = store != NULL;
    X509_STORE_CTX *ctx;
    int flags = X509_ADD_FLAG_UP_REF;
    STACK_OF(X509) *result = NULL;

    if (target == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((ctx = X509_STORE_CTX_new_ex(libctx, propq)) == NULL)
        return NULL;
    if (!X509_STORE_CTX_init(ctx, store, target, finish_chain ? certs : NULL))
        goto err;
    if (!finish_chain)
        X509_STORE_CTX_set0_trusted_stack(ctx, certs);
    if (!ossl_x509_add_cert_new(&ctx->chain, target, X509_ADD_FLAG_UP_REF)) {
        ctx->error = X509_V_ERR_OUT_OF_MEM;
        goto err;
    }
    ctx->num_untrusted = 1;

    if (!build_chain(ctx) && finish_chain)
        goto err;

    /* result list to store the up_ref'ed certificates */
    if (sk_X509_num(ctx->chain) > 1 && !with_self_signed)
        flags |= X509_ADD_FLAG_NO_SS;
    if (!ossl_x509_add_certs_new(&result, ctx->chain, flags)) {
        sk_X509_free(result);
        result = NULL;
    }

 err:
    X509_STORE_CTX_free(ctx);
    return result;
}

/*
 * note that there's a corresponding minbits_table in ssl/ssl_cert.c
 * in ssl_get_security_level_bits that's used for selection of DH parameters
 */
static const int minbits_table[] = { 80, 112, 128, 192, 256 };
static const int NUM_AUTH_LEVELS = OSSL_NELEM(minbits_table);

/*-
 * Check whether the given public key meets the security level of `ctx`.
 * Returns 1 on success, 0 otherwise.
 */
static int check_key_level(X509_STORE_CTX *ctx, EVP_PKEY *pkey)
{
    int level = ctx->param->auth_level;

    /*
     * At security level zero, return without checking for a supported public
     * key type.  Some engines support key types not understood outside the
     * engine, and we only need to understand the key when enforcing a security
     * floor.
     */
    if (level <= 0)
        return 1;

    /* Unsupported or malformed keys are not secure */
    if (pkey == NULL)
        return 0;

    if (level > NUM_AUTH_LEVELS)
        level = NUM_AUTH_LEVELS;

    return EVP_PKEY_get_security_bits(pkey) >= minbits_table[level - 1];
}

/*-
 * Check whether the public key of `cert` meets the security level of `ctx`.
 * Returns 1 on success, 0 otherwise.
 */
static int check_cert_key_level(X509_STORE_CTX *ctx, X509 *cert)
{
    return check_key_level(ctx, X509_get0_pubkey(cert));
}

/*-
 * Check whether the public key of ``cert`` does not use explicit params
 * for an elliptic curve.
 *
 * Returns 1 on success, 0 if check fails, -1 for other errors.
 */
static int check_curve(X509 *cert)
{
    EVP_PKEY *pkey = X509_get0_pubkey(cert);
    int ret, val;

    /* Unsupported or malformed key */
    if (pkey == NULL)
        return -1;
    if (EVP_PKEY_get_id(pkey) != EVP_PKEY_EC)
        return 1;

    ret =
        EVP_PKEY_get_int_param(pkey,
                               OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS,
                               &val);
    return ret == 1 ? !val : -1;
}

/*-
 * Check whether the signature digest algorithm of ``cert`` meets the security
 * level of ``ctx``.  Should not be checked for trust anchors (whether
 * self-signed or otherwise).
 *
 * Returns 1 on success, 0 otherwise.
 */
static int check_sig_level(X509_STORE_CTX *ctx, X509 *cert)
{
    int secbits = -1;
    int level = ctx->param->auth_level;

    if (level <= 0)
        return 1;
    if (level > NUM_AUTH_LEVELS)
        level = NUM_AUTH_LEVELS;

    if (!X509_get_signature_info(cert, NULL, NULL, &secbits, NULL))
        return 0;

    return secbits >= minbits_table[level - 1];
}
