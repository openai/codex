/*
 * Copyright 2001-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#ifdef OPENSSL_SYS_VMS
  /* So fd_set and friends get properly defined on OpenVMS */
# define _XOPEN_SOURCE_EXTENDED
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <ctype.h>

/* Needs to be included before the openssl headers */
#include "apps.h"
#include "http_server.h"
#include "progs.h"
#include "internal/sockets.h"
#include <openssl/e_os2.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/evp.h>
#include <openssl/bn.h>
#include <openssl/x509v3.h>

#if defined(OPENSSL_SYS_VXWORKS)
/* not supported */
int setpgid(pid_t pid, pid_t pgid)
{
    errno = ENOSYS;
    return 0;
}
/* not supported */
pid_t fork(void)
{
    errno = ENOSYS;
    return (pid_t) -1;
}
#endif
/* Maximum leeway in validity period: default 5 minutes */
#define MAX_VALIDITY_PERIOD    (5 * 60)

static int add_ocsp_cert(OCSP_REQUEST **req, X509 *cert,
                         const EVP_MD *cert_id_md, X509 *issuer,
                         STACK_OF(OCSP_CERTID) *ids);
static int add_ocsp_serial(OCSP_REQUEST **req, char *serial,
                           const EVP_MD *cert_id_md, X509 *issuer,
                           STACK_OF(OCSP_CERTID) *ids);
static int print_ocsp_summary(BIO *out, OCSP_BASICRESP *bs, OCSP_REQUEST *req,
                              STACK_OF(OPENSSL_STRING) *names,
                              STACK_OF(OCSP_CERTID) *ids, long nsec,
                              long maxage);
static void make_ocsp_response(BIO *err, OCSP_RESPONSE **resp, OCSP_REQUEST *req,
                              CA_DB *db, STACK_OF(X509) *ca, X509 *rcert,
                              EVP_PKEY *rkey, const EVP_MD *md,
                              STACK_OF(OPENSSL_STRING) *sigopts,
                              STACK_OF(X509) *rother, unsigned long flags,
                              int nmin, int ndays, int badsig,
                              const EVP_MD *resp_md);

static char **lookup_serial(CA_DB *db, ASN1_INTEGER *ser);
static int do_responder(OCSP_REQUEST **preq, BIO **pcbio, BIO *acbio,
                        int timeout);
static int send_ocsp_response(BIO *cbio, const OCSP_RESPONSE *resp);
static char *prog;

#ifdef HTTP_DAEMON
static int index_changed(CA_DB *);
#endif

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_OUTFILE, OPT_TIMEOUT, OPT_URL, OPT_HOST, OPT_PORT,
#ifndef OPENSSL_NO_SOCK
    OPT_PROXY, OPT_NO_PROXY,
#endif
    OPT_IGNORE_ERR, OPT_NOVERIFY, OPT_NONCE, OPT_NO_NONCE,
    OPT_RESP_NO_CERTS, OPT_RESP_KEY_ID, OPT_NO_CERTS,
    OPT_NO_SIGNATURE_VERIFY, OPT_NO_CERT_VERIFY, OPT_NO_CHAIN,
    OPT_NO_CERT_CHECKS, OPT_NO_EXPLICIT, OPT_TRUST_OTHER,
    OPT_NO_INTERN, OPT_BADSIG, OPT_TEXT, OPT_REQ_TEXT, OPT_RESP_TEXT,
    OPT_REQIN, OPT_RESPIN, OPT_SIGNER, OPT_VAFILE, OPT_SIGN_OTHER,
    OPT_VERIFY_OTHER, OPT_CAFILE, OPT_CAPATH, OPT_CASTORE, OPT_NOCAFILE,
    OPT_NOCAPATH, OPT_NOCASTORE,
    OPT_VALIDITY_PERIOD, OPT_STATUS_AGE, OPT_SIGNKEY, OPT_REQOUT,
    OPT_RESPOUT, OPT_PATH, OPT_ISSUER, OPT_CERT, OPT_SERIAL,
    OPT_INDEX, OPT_CA, OPT_NMIN, OPT_REQUEST, OPT_NDAYS, OPT_RSIGNER,
    OPT_RKEY, OPT_ROTHER, OPT_RMD, OPT_RSIGOPT, OPT_HEADER,
    OPT_PASSIN,
    OPT_RCID,
    OPT_V_ENUM,
    OPT_MD,
    OPT_MULTI, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS ocsp_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"ignore_err", OPT_IGNORE_ERR, '-',
     "Ignore error on OCSP request or response and continue running"},
    {"CAfile", OPT_CAFILE, '<', "Trusted certificates file"},
    {"CApath", OPT_CAPATH, '<', "Trusted certificates directory"},
    {"CAstore", OPT_CASTORE, ':', "Trusted certificates store URI"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load certificates from the default certificates directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load certificates from the default certificates store"},

    OPT_SECTION("Responder"),
    {"timeout", OPT_TIMEOUT, 'p',
     "Connection timeout (in seconds) to the OCSP responder"},
    {"resp_no_certs", OPT_RESP_NO_CERTS, '-',
     "Don't include any certificates in response"},
#ifdef HTTP_DAEMON
    {"multi", OPT_MULTI, 'p', "run multiple responder processes"},
#endif
    {"no_certs", OPT_NO_CERTS, '-',
     "Don't include any certificates in signed request"},
    {"badsig", OPT_BADSIG, '-',
        "Corrupt last byte of loaded OCSP response signature (for test)"},
    {"CA", OPT_CA, '<', "CA certificates"},
    {"nmin", OPT_NMIN, 'p', "Number of minutes before next update"},
    {"nrequest", OPT_REQUEST, 'p',
     "Number of requests to accept (default unlimited)"},
    {"reqin", OPT_REQIN, 's', "File with the DER-encoded request"},
    {"signer", OPT_SIGNER, '<', "Certificate to sign OCSP request with"},
    {"sign_other", OPT_SIGN_OTHER, '<',
     "Additional certificates to include in signed request"},
    {"index", OPT_INDEX, '<', "Certificate status index file"},
    {"ndays", OPT_NDAYS, 'p', "Number of days before next update"},
    {"rsigner", OPT_RSIGNER, '<',
     "Responder certificate to sign responses with"},
    {"rkey", OPT_RKEY, '<', "Responder key to sign responses with"},
    {"passin", OPT_PASSIN, 's', "Responder key pass phrase source"},
    {"rother", OPT_ROTHER, '<', "Other certificates to include in response"},
    {"rmd", OPT_RMD, 's', "Digest Algorithm to use in signature of OCSP response"},
    {"rsigopt", OPT_RSIGOPT, 's', "OCSP response signature parameter in n:v form"},
    {"header", OPT_HEADER, 's', "key=value header to add"},
    {"rcid", OPT_RCID, 's', "Use specified algorithm for cert id in response"},
    {"", OPT_MD, '-', "Any supported digest algorithm (sha1,sha256, ... )"},

    OPT_SECTION("Client"),
    {"url", OPT_URL, 's', "Responder URL"},
    {"host", OPT_HOST, 's', "TCP/IP hostname:port to connect to"},
    {"port", OPT_PORT, 'N', "Port to run responder on"},
    {"path", OPT_PATH, 's', "Path to use in OCSP request"},
#ifndef OPENSSL_NO_SOCK
    {"proxy", OPT_PROXY, 's',
     "[http[s]://]host[:port][/path] of HTTP(S) proxy to use; path is ignored"},
    {"no_proxy", OPT_NO_PROXY, 's',
     "List of addresses of servers not to use HTTP(S) proxy for"},
    {OPT_MORE_STR, 0, 0,
     "Default from environment variable 'no_proxy', else 'NO_PROXY', else none"},
#endif
    {"out", OPT_OUTFILE, '>', "Output filename"},
    {"noverify", OPT_NOVERIFY, '-', "Don't verify response at all"},
    {"nonce", OPT_NONCE, '-', "Add OCSP nonce to request"},
    {"no_nonce", OPT_NO_NONCE, '-', "Don't add OCSP nonce to request"},
    {"no_signature_verify", OPT_NO_SIGNATURE_VERIFY, '-',
     "Don't check signature on response"},
    {"resp_key_id", OPT_RESP_KEY_ID, '-',
     "Identify response by signing certificate key ID"},
    {"no_cert_verify", OPT_NO_CERT_VERIFY, '-',
     "Don't check signing certificate"},
    {"text", OPT_TEXT, '-', "Print text form of request and response"},
    {"req_text", OPT_REQ_TEXT, '-', "Print text form of request"},
    {"resp_text", OPT_RESP_TEXT, '-', "Print text form of response"},
    {"no_chain", OPT_NO_CHAIN, '-', "Don't chain verify response"},
    {"no_cert_checks", OPT_NO_CERT_CHECKS, '-',
     "Don't do additional checks on signing certificate"},
    {"no_explicit", OPT_NO_EXPLICIT, '-',
     "Do not explicitly check the chain, just verify the root"},
    {"trust_other", OPT_TRUST_OTHER, '-',
     "Don't verify additional certificates"},
    {"no_intern", OPT_NO_INTERN, '-',
     "Don't search certificates contained in response for signer"},
    {"respin", OPT_RESPIN, 's', "File with the DER-encoded response"},
    {"VAfile", OPT_VAFILE, '<', "Validator certificates file"},
    {"verify_other", OPT_VERIFY_OTHER, '<',
     "Additional certificates to search for signer"},
    {"cert", OPT_CERT, '<',
     "Certificate to check; may be given multiple times"},
    {"serial", OPT_SERIAL, 's',
     "Serial number to check; may be given multiple times"},
    {"validity_period", OPT_VALIDITY_PERIOD, 'u',
     "Maximum validity discrepancy in seconds"},
    {"signkey", OPT_SIGNKEY, 's', "Private key to sign OCSP request with"},
    {"reqout", OPT_REQOUT, 's', "Output file for the DER-encoded request"},
    {"respout", OPT_RESPOUT, 's', "Output file for the DER-encoded response"},
    {"issuer", OPT_ISSUER, '<', "Issuer certificate"},
    {"status_age", OPT_STATUS_AGE, 'p', "Maximum status age in seconds"},

    OPT_V_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

int ocsp_main(int argc, char **argv)
{
    BIO *acbio = NULL, *cbio = NULL, *derbio = NULL, *out = NULL;
    EVP_MD *cert_id_md = NULL, *rsign_md = NULL;
    STACK_OF(OPENSSL_STRING) *rsign_sigopts = NULL;
    int trailing_md = 0;
    CA_DB *rdb = NULL;
    EVP_PKEY *key = NULL, *rkey = NULL;
    OCSP_BASICRESP *bs = NULL;
    OCSP_REQUEST *req = NULL;
    OCSP_RESPONSE *resp = NULL;
    STACK_OF(CONF_VALUE) *headers = NULL;
    STACK_OF(OCSP_CERTID) *ids = NULL;
    STACK_OF(OPENSSL_STRING) *reqnames = NULL;
    STACK_OF(X509) *sign_other = NULL, *verify_other = NULL, *rother = NULL;
    STACK_OF(X509) *issuers = NULL;
    X509 *issuer = NULL, *cert = NULL;
    STACK_OF(X509) *rca_certs = NULL;
    EVP_MD *resp_certid_md = NULL;
    X509 *signer = NULL, *rsigner = NULL;
    X509_STORE *store = NULL;
    X509_VERIFY_PARAM *vpm = NULL;
    const char *CAfile = NULL, *CApath = NULL, *CAstore = NULL;
    char *header, *value, *respdigname = NULL;
    char *host = NULL, *port = NULL, *path = "/", *outfile = NULL;
#ifndef OPENSSL_NO_SOCK
    char *opt_proxy = NULL;
    char *opt_no_proxy = NULL;
#endif
    char *rca_filename = NULL, *reqin = NULL, *respin = NULL;
    char *reqout = NULL, *respout = NULL, *ridx_filename = NULL;
    char *rsignfile = NULL, *rkeyfile = NULL;
    char *passinarg = NULL, *passin = NULL;
    char *sign_certfile = NULL, *verify_certfile = NULL, *rcertfile = NULL;
    char *signfile = NULL, *keyfile = NULL;
    char *thost = NULL, *tport = NULL, *tpath = NULL;
    int noCAfile = 0, noCApath = 0, noCAstore = 0;
    int accept_count = -1, add_nonce = 1, noverify = 0, use_ssl = -1;
    int vpmtouched = 0, badsig = 0, i, ignore_err = 0, nmin = 0, ndays = -1;
    int req_text = 0, resp_text = 0, res, ret = 1;
    int req_timeout = -1;
    long nsec = MAX_VALIDITY_PERIOD, maxage = -1;
    unsigned long sign_flags = 0, verify_flags = 0, rflags = 0;
    OPTION_CHOICE o;

    if ((reqnames = sk_OPENSSL_STRING_new_null()) == NULL
            || (ids = sk_OCSP_CERTID_new_null()) == NULL
            || (vpm = X509_VERIFY_PARAM_new()) == NULL)
        goto end;

    opt_set_unknown_name("digest");
    prog = opt_init(argc, argv, ocsp_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            ret = 0;
            opt_help(ocsp_options);
            goto end;
        case OPT_OUTFILE:
            outfile = opt_arg();
            break;
        case OPT_TIMEOUT:
#ifndef OPENSSL_NO_SOCK
            req_timeout = atoi(opt_arg());
#endif
            break;
        case OPT_URL:
            OPENSSL_free(thost);
            OPENSSL_free(tport);
            OPENSSL_free(tpath);
            thost = tport = tpath = NULL;
            if (!OSSL_HTTP_parse_url(opt_arg(), &use_ssl, NULL /* userinfo */,
                                     &host, &port, NULL /* port_num */,
                                     &path, NULL /* qry */, NULL /* frag */)) {
                BIO_printf(bio_err, "%s Error parsing -url argument\n", prog);
                goto end;
            }
            thost = host;
            tport = port;
            tpath = path;
            break;
        case OPT_HOST:
            host = opt_arg();
            break;
        case OPT_PORT:
            port = opt_arg();
            break;
        case OPT_PATH:
            path = opt_arg();
            break;
#ifndef OPENSSL_NO_SOCK
        case OPT_PROXY:
            opt_proxy = opt_arg();
            break;
        case OPT_NO_PROXY:
            opt_no_proxy = opt_arg();
            break;
#endif
        case OPT_IGNORE_ERR:
            ignore_err = 1;
            break;
        case OPT_NOVERIFY:
            noverify = 1;
            break;
        case OPT_NONCE:
            add_nonce = 2;
            break;
        case OPT_NO_NONCE:
            add_nonce = 0;
            break;
        case OPT_RESP_NO_CERTS:
            rflags |= OCSP_NOCERTS;
            break;
        case OPT_RESP_KEY_ID:
            rflags |= OCSP_RESPID_KEY;
            break;
        case OPT_NO_CERTS:
            sign_flags |= OCSP_NOCERTS;
            break;
        case OPT_NO_SIGNATURE_VERIFY:
            verify_flags |= OCSP_NOSIGS;
            break;
        case OPT_NO_CERT_VERIFY:
            verify_flags |= OCSP_NOVERIFY;
            break;
        case OPT_NO_CHAIN:
            verify_flags |= OCSP_NOCHAIN;
            break;
        case OPT_NO_CERT_CHECKS:
            verify_flags |= OCSP_NOCHECKS;
            break;
        case OPT_NO_EXPLICIT:
            verify_flags |= OCSP_NOEXPLICIT;
            break;
        case OPT_TRUST_OTHER:
            verify_flags |= OCSP_TRUSTOTHER;
            break;
        case OPT_NO_INTERN:
            verify_flags |= OCSP_NOINTERN;
            break;
        case OPT_BADSIG:
            badsig = 1;
            break;
        case OPT_TEXT:
            req_text = resp_text = 1;
            break;
        case OPT_REQ_TEXT:
            req_text = 1;
            break;
        case OPT_RESP_TEXT:
            resp_text = 1;
            break;
        case OPT_REQIN:
            reqin = opt_arg();
            break;
        case OPT_RESPIN:
            respin = opt_arg();
            break;
        case OPT_SIGNER:
            signfile = opt_arg();
            break;
        case OPT_VAFILE:
            verify_certfile = opt_arg();
            verify_flags |= OCSP_TRUSTOTHER;
            break;
        case OPT_SIGN_OTHER:
            sign_certfile = opt_arg();
            break;
        case OPT_VERIFY_OTHER:
            verify_certfile = opt_arg();
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            break;
        case OPT_NOCAFILE:
            noCAfile = 1;
            break;
        case OPT_NOCAPATH:
            noCApath = 1;
            break;
        case OPT_NOCASTORE:
            noCAstore = 1;
            break;
        case OPT_V_CASES:
            if (!opt_verify(o, vpm))
                goto end;
            vpmtouched++;
            break;
        case OPT_VALIDITY_PERIOD:
            opt_long(opt_arg(), &nsec);
            break;
        case OPT_STATUS_AGE:
            opt_long(opt_arg(), &maxage);
            break;
        case OPT_SIGNKEY:
            keyfile = opt_arg();
            break;
        case OPT_REQOUT:
            reqout = opt_arg();
            break;
        case OPT_RESPOUT:
            respout = opt_arg();
            break;
        case OPT_ISSUER:
            issuer = load_cert(opt_arg(), FORMAT_UNDEF, "issuer certificate");
            if (issuer == NULL)
                goto end;
            if (issuers == NULL) {
                if ((issuers = sk_X509_new_null()) == NULL)
                    goto end;
            }
            if (!sk_X509_push(issuers, issuer))
                goto end;
            break;
        case OPT_CERT:
            reset_unknown();
            X509_free(cert);
            cert = load_cert(opt_arg(), FORMAT_UNDEF, "certificate");
            if (cert == NULL)
                goto end;
            if (cert_id_md == NULL)
                cert_id_md = (EVP_MD *)EVP_sha1();
            if (!add_ocsp_cert(&req, cert, cert_id_md, issuer, ids))
                goto end;
            if (!sk_OPENSSL_STRING_push(reqnames, opt_arg()))
                goto end;
            trailing_md = 0;
            break;
        case OPT_SERIAL:
            reset_unknown();
            if (cert_id_md == NULL)
                cert_id_md = (EVP_MD *)EVP_sha1();
            if (!add_ocsp_serial(&req, opt_arg(), cert_id_md, issuer, ids))
                goto end;
            if (!sk_OPENSSL_STRING_push(reqnames, opt_arg()))
                goto end;
            trailing_md = 0;
            break;
        case OPT_INDEX:
            ridx_filename = opt_arg();
            break;
        case OPT_CA:
            rca_filename = opt_arg();
            break;
        case OPT_NMIN:
            nmin = opt_int_arg();
            if (ndays == -1)
                ndays = 0;
            break;
        case OPT_REQUEST:
            accept_count = opt_int_arg();
            break;
        case OPT_NDAYS:
            ndays = atoi(opt_arg());
            break;
        case OPT_RSIGNER:
            rsignfile = opt_arg();
            break;
        case OPT_RKEY:
            rkeyfile = opt_arg();
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_ROTHER:
            rcertfile = opt_arg();
            break;
        case OPT_RMD:   /* Response MessageDigest */
            respdigname = opt_arg();
            break;
        case OPT_RSIGOPT:
            if (rsign_sigopts == NULL)
                rsign_sigopts = sk_OPENSSL_STRING_new_null();
            if (rsign_sigopts == NULL
                || !sk_OPENSSL_STRING_push(rsign_sigopts, opt_arg()))
                goto end;
            break;
        case OPT_HEADER:
            header = opt_arg();
            value = strchr(header, '=');
            if (value == NULL) {
                BIO_printf(bio_err, "Missing = in header key=value\n");
                goto opthelp;
            }
            *value++ = '\0';
            if (!X509V3_add_value(header, value, &headers))
                goto end;
            break;
        case OPT_RCID:
            if (!opt_md(opt_arg(), &resp_certid_md))
                goto opthelp;
            break;
        case OPT_MD:
            if (trailing_md) {
                BIO_printf(bio_err,
                           "%s: Digest must be before -cert or -serial\n",
                           prog);
                goto opthelp;
            }
            if (!opt_md(opt_unknown(), &cert_id_md))
                goto opthelp;
            trailing_md = 1;
            break;
        case OPT_MULTI:
#ifdef HTTP_DAEMON
            n_responders = atoi(opt_arg());
#endif
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (trailing_md) {
        BIO_printf(bio_err, "%s: Digest must be before -cert or -serial\n",
                   prog);
        goto opthelp;
    }

    if (respdigname != NULL) {
        if (!opt_md(respdigname, &rsign_md))
            goto end;
    }

    /* Have we anything to do? */
    if (req == NULL && reqin == NULL
        && respin == NULL && !(port != NULL && ridx_filename != NULL))
        goto opthelp;

    if (req == NULL && (add_nonce != 2))
        add_nonce = 0;

    if (req == NULL && reqin != NULL) {
        derbio = bio_open_default(reqin, 'r', FORMAT_ASN1);
        if (derbio == NULL)
            goto end;
        req = d2i_OCSP_REQUEST_bio(derbio, NULL);
        BIO_free(derbio);
        if (req == NULL) {
            BIO_printf(bio_err, "Error reading OCSP request\n");
            goto end;
        }
    }

    if (req == NULL && port != NULL) {
#ifndef OPENSSL_NO_SOCK
        acbio = http_server_init(prog, port, -1);
        if (acbio == NULL)
            goto end;
#else
        BIO_printf(bio_err, "Cannot act as server - sockets not supported\n");
        goto end;
#endif
    }

    if (rsignfile != NULL) {
        if (rkeyfile == NULL)
            rkeyfile = rsignfile;
        rsigner = load_cert(rsignfile, FORMAT_UNDEF, "responder certificate");
        if (rsigner == NULL) {
            BIO_printf(bio_err, "Error loading responder certificate\n");
            goto end;
        }
        if (!load_certs(rca_filename, 0, &rca_certs, NULL, "CA certificates"))
            goto end;
        if (rcertfile != NULL) {
            if (!load_certs(rcertfile, 0, &rother, NULL,
                            "responder other certificates"))
                goto end;
        }
        if (!app_passwd(passinarg, NULL, &passin, NULL)) {
            BIO_printf(bio_err, "Error getting password\n");
            goto end;
        }
        rkey = load_key(rkeyfile, FORMAT_UNDEF, 0, passin, NULL,
                        "responder private key");
        if (rkey == NULL)
            goto end;
    }

    if (ridx_filename != NULL
        && (rkey == NULL || rsigner == NULL || rca_certs == NULL)) {
        BIO_printf(bio_err,
                   "Responder mode requires certificate, key, and CA.\n");
        goto end;
    }

    if (ridx_filename != NULL) {
        rdb = load_index(ridx_filename, NULL);
        if (rdb == NULL || index_index(rdb) <= 0) {
            BIO_printf(bio_err,
                "Problem with index file: %s (could not load/parse file)\n",
                ridx_filename);
            ret = 1;
            goto end;
        }
    }

#ifdef HTTP_DAEMON
    if (n_responders != 0 && acbio != NULL)
        spawn_loop(prog);
    if (acbio != NULL && req_timeout > 0)
        signal(SIGALRM, socket_timeout);
#endif

    if (acbio != NULL)
        trace_log_message(-1, prog,
                          LOG_INFO, "waiting for OCSP client connections...");

redo_accept:

    if (acbio != NULL) {
#ifdef HTTP_DAEMON
        if (index_changed(rdb)) {
            CA_DB *newrdb = load_index(ridx_filename, NULL);

            if (newrdb != NULL && index_index(newrdb) > 0) {
                free_index(rdb);
                rdb = newrdb;
            } else {
                free_index(newrdb);
                trace_log_message(-1, prog,
                                  LOG_ERR, "error reloading updated index: %s",
                                  ridx_filename);
            }
        }
#endif

        req = NULL;
        res = do_responder(&req, &cbio, acbio, req_timeout);
        if (res == 0)
            goto redo_accept;

        if (req == NULL) {
            if (res == 1) {
                resp =
                    OCSP_response_create(OCSP_RESPONSE_STATUS_MALFORMEDREQUEST,
                                         NULL);
                send_ocsp_response(cbio, resp);
            }
            goto done_resp;
        }
    }

    if (req == NULL
        && (signfile != NULL || reqout != NULL
            || host != NULL || add_nonce || ridx_filename != NULL)) {
        BIO_printf(bio_err, "Need an OCSP request for this operation!\n");
        goto end;
    }

    if (req != NULL && add_nonce) {
        if (!OCSP_request_add1_nonce(req, NULL, -1))
            goto end;
    }

    if (signfile != NULL) {
        if (keyfile == NULL)
            keyfile = signfile;
        signer = load_cert(signfile, FORMAT_UNDEF, "signer certificate");
        if (signer == NULL) {
            BIO_printf(bio_err, "Error loading signer certificate\n");
            goto end;
        }
        if (sign_certfile != NULL) {
            if (!load_certs(sign_certfile, 0, &sign_other, NULL,
                            "signer certificates"))
                goto end;
        }
        key = load_key(keyfile, FORMAT_UNDEF, 0, NULL, NULL,
                       "signer private key");
        if (key == NULL)
            goto end;

        if (!OCSP_request_sign(req, signer, key, NULL,
                               sign_other, sign_flags)) {
            BIO_printf(bio_err, "Error signing OCSP request\n");
            goto end;
        }
    }

    out = bio_open_default(outfile, 'w', FORMAT_TEXT);
    if (out == NULL)
        goto end;

    if (req_text && req != NULL)
        OCSP_REQUEST_print(out, req, 0);

    if (reqout != NULL) {
        derbio = bio_open_default(reqout, 'w', FORMAT_ASN1);
        if (derbio == NULL)
            goto end;
        i2d_OCSP_REQUEST_bio(derbio, req);
        BIO_free(derbio);
    }

    if (rdb != NULL) {
        make_ocsp_response(bio_err, &resp, req, rdb, rca_certs, rsigner, rkey,
                           rsign_md, rsign_sigopts, rother, rflags, nmin, ndays,
                           badsig, resp_certid_md);
        if (resp == NULL)
            goto end;
        if (cbio != NULL)
            send_ocsp_response(cbio, resp);
    } else if (host != NULL) {
#ifndef OPENSSL_NO_SOCK
        resp = process_responder(req, host, port, path, opt_proxy, opt_no_proxy,
                                 use_ssl, headers, req_timeout);
        if (resp == NULL)
            goto end;
#else
        BIO_printf(bio_err,
                   "Error creating connect BIO - sockets not supported\n");
        goto end;
#endif
    } else if (respin != NULL) {
        derbio = bio_open_default(respin, 'r', FORMAT_ASN1);
        if (derbio == NULL)
            goto end;
        resp = d2i_OCSP_RESPONSE_bio(derbio, NULL);
        BIO_free(derbio);
        if (resp == NULL) {
            BIO_printf(bio_err, "Error reading OCSP response\n");
            goto end;
        }
    } else {
        ret = 0;
        goto end;
    }

 done_resp:

    if (respout != NULL) {
        derbio = bio_open_default(respout, 'w', FORMAT_ASN1);
        if (derbio == NULL)
            goto end;
        i2d_OCSP_RESPONSE_bio(derbio, resp);
        BIO_free(derbio);
    }

    i = OCSP_response_status(resp);
    if (i != OCSP_RESPONSE_STATUS_SUCCESSFUL) {
        BIO_printf(out, "Responder Error: %s (%d)\n",
                   OCSP_response_status_str(i), i);
        if (!ignore_err)
                goto end;
    }

    if (resp_text)
        OCSP_RESPONSE_print(out, resp, 0);

    /* If running as responder don't verify our own response */
    if (cbio != NULL) {
        /* If not unlimited, see if we took all we should. */
        if (accept_count != -1 && --accept_count <= 0) {
            ret = 0;
            goto end;
        }
        BIO_free_all(cbio);
        cbio = NULL;
        OCSP_REQUEST_free(req);
        req = NULL;
        OCSP_RESPONSE_free(resp);
        resp = NULL;
        goto redo_accept;
    }
    if (ridx_filename != NULL) {
        ret = 0;
        goto end;
    }

    if (store == NULL) {
        store = setup_verify(CAfile, noCAfile, CApath, noCApath,
                             CAstore, noCAstore);
        if (!store)
            goto end;
    }
    if (vpmtouched)
        X509_STORE_set1_param(store, vpm);
    if (verify_certfile != NULL) {
        if (!load_certs(verify_certfile, 0, &verify_other, NULL,
                        "validator certificates"))
            goto end;
    }

    bs = OCSP_response_get1_basic(resp);
    if (bs == NULL) {
        BIO_printf(bio_err, "Error parsing response\n");
        goto end;
    }

    ret = 0;

    if (!noverify) {
        if (req != NULL && ((i = OCSP_check_nonce(req, bs)) <= 0)) {
            if (i == -1)
                BIO_printf(bio_err, "WARNING: no nonce in response\n");
            else {
                BIO_printf(bio_err, "Nonce Verify error\n");
                ret = 1;
                goto end;
            }
        }

        i = OCSP_basic_verify(bs, verify_other, store, verify_flags);
        if (i <= 0 && issuers) {
            i = OCSP_basic_verify(bs, issuers, store, OCSP_TRUSTOTHER);
            if (i > 0)
                ERR_clear_error();
        }
        if (i <= 0) {
            BIO_printf(bio_err, "Response Verify Failure\n");
            ERR_print_errors(bio_err);
            ret = 1;
        } else {
            BIO_printf(bio_err, "Response verify OK\n");
        }
    }

    if (!print_ocsp_summary(out, bs, req, reqnames, ids, nsec, maxage))
        ret = 1;

 end:
    ERR_print_errors(bio_err);
    X509_free(signer);
    X509_STORE_free(store);
    X509_VERIFY_PARAM_free(vpm);
    sk_OPENSSL_STRING_free(rsign_sigopts);
    EVP_PKEY_free(key);
    EVP_PKEY_free(rkey);
    EVP_MD_free(cert_id_md);
    EVP_MD_free(rsign_md);
    EVP_MD_free(resp_certid_md);
    X509_free(cert);
    OSSL_STACK_OF_X509_free(issuers);
    X509_free(rsigner);
    OSSL_STACK_OF_X509_free(rca_certs);
    free_index(rdb);
    BIO_free_all(cbio);
    BIO_free_all(acbio);
    BIO_free_all(out);
    OCSP_REQUEST_free(req);
    OCSP_RESPONSE_free(resp);
    OCSP_BASICRESP_free(bs);
    sk_OPENSSL_STRING_free(reqnames);
    sk_OCSP_CERTID_free(ids);
    OSSL_STACK_OF_X509_free(sign_other);
    OSSL_STACK_OF_X509_free(verify_other);
    sk_CONF_VALUE_pop_free(headers, X509V3_conf_free);
    OPENSSL_free(thost);
    OPENSSL_free(tport);
    OPENSSL_free(tpath);

    return ret;
}

#ifdef HTTP_DAEMON

static int index_changed(CA_DB *rdb)
{
    struct stat sb;

    if (rdb != NULL && stat(rdb->dbfname, &sb) != -1) {
        if (rdb->dbst.st_mtime != sb.st_mtime
            || rdb->dbst.st_ctime != sb.st_ctime
            || rdb->dbst.st_ino != sb.st_ino
            || rdb->dbst.st_dev != sb.st_dev) {
            syslog(LOG_INFO, "index file changed, reloading");
            return 1;
        }
    }
    return 0;
}

#endif

static int add_ocsp_cert(OCSP_REQUEST **req, X509 *cert,
                         const EVP_MD *cert_id_md, X509 *issuer,
                         STACK_OF(OCSP_CERTID) *ids)
{
    OCSP_CERTID *id;

    if (issuer == NULL) {
        BIO_printf(bio_err, "No issuer certificate specified\n");
        return 0;
    }
    if (*req == NULL)
        *req = OCSP_REQUEST_new();
    if (*req == NULL)
        goto err;
    id = OCSP_cert_to_id(cert_id_md, cert, issuer);
    if (id == NULL || !sk_OCSP_CERTID_push(ids, id))
        goto err;
    if (!OCSP_request_add0_id(*req, id))
        goto err;
    return 1;

 err:
    BIO_printf(bio_err, "Error Creating OCSP request\n");
    return 0;
}

static int add_ocsp_serial(OCSP_REQUEST **req, char *serial,
                           const EVP_MD *cert_id_md, X509 *issuer,
                           STACK_OF(OCSP_CERTID) *ids)
{
    OCSP_CERTID *id;
    const X509_NAME *iname;
    ASN1_BIT_STRING *ikey;
    ASN1_INTEGER *sno;

    if (issuer == NULL) {
        BIO_printf(bio_err, "No issuer certificate specified\n");
        return 0;
    }
    if (*req == NULL)
        *req = OCSP_REQUEST_new();
    if (*req == NULL)
        goto err;
    iname = X509_get_subject_name(issuer);
    ikey = X509_get0_pubkey_bitstr(issuer);
    sno = s2i_ASN1_INTEGER(NULL, serial);
    if (sno == NULL) {
        BIO_printf(bio_err, "Error converting serial number %s\n", serial);
        return 0;
    }
    id = OCSP_cert_id_new(cert_id_md, iname, ikey, sno);
    ASN1_INTEGER_free(sno);
    if (id == NULL || !sk_OCSP_CERTID_push(ids, id))
        goto err;
    if (!OCSP_request_add0_id(*req, id))
        goto err;
    return 1;

 err:
    BIO_printf(bio_err, "Error Creating OCSP request\n");
    return 0;
}

static int print_ocsp_summary(BIO *out, OCSP_BASICRESP *bs, OCSP_REQUEST *req,
                              STACK_OF(OPENSSL_STRING) *names,
                              STACK_OF(OCSP_CERTID) *ids, long nsec,
                              long maxage)
{
    OCSP_CERTID *id;
    const char *name;
    int i, status, reason;
    ASN1_GENERALIZEDTIME *rev, *thisupd, *nextupd;
    int ret = 1;

    if (req == NULL || !sk_OPENSSL_STRING_num(names))
        return 1;

    if (bs == NULL || !sk_OCSP_CERTID_num(ids))
        return 0;

    for (i = 0; i < sk_OCSP_CERTID_num(ids); i++) {
        id = sk_OCSP_CERTID_value(ids, i);
        name = sk_OPENSSL_STRING_value(names, i);
        BIO_printf(out, "%s: ", name);

        if (!OCSP_resp_find_status(bs, id, &status, &reason,
                                   &rev, &thisupd, &nextupd)) {
            BIO_puts(out, "ERROR: No Status found.\n");
            ret = 0;
            continue;
        }

        /*
         * Check validity: if invalid write to output BIO so we know which
         * response this refers to.
         */
        if (!OCSP_check_validity(thisupd, nextupd, nsec, maxage)) {
            BIO_puts(out, "WARNING: Status times invalid.\n");
            ERR_print_errors(out);
        }
        BIO_printf(out, "%s\n", OCSP_cert_status_str(status));

        BIO_puts(out, "\tThis Update: ");
        ASN1_GENERALIZEDTIME_print(out, thisupd);
        BIO_puts(out, "\n");

        if (nextupd) {
            BIO_puts(out, "\tNext Update: ");
            ASN1_GENERALIZEDTIME_print(out, nextupd);
            BIO_puts(out, "\n");
        }

        if (status != V_OCSP_CERTSTATUS_REVOKED)
            continue;

        if (reason != -1)
            BIO_printf(out, "\tReason: %s\n", OCSP_crl_reason_str(reason));

        BIO_puts(out, "\tRevocation Time: ");
        ASN1_GENERALIZEDTIME_print(out, rev);
        BIO_puts(out, "\n");
    }
    return ret;
}

static void make_ocsp_response(BIO *err, OCSP_RESPONSE **resp, OCSP_REQUEST *req,
                              CA_DB *db, STACK_OF(X509) *ca, X509 *rcert,
                              EVP_PKEY *rkey, const EVP_MD *rmd,
                              STACK_OF(OPENSSL_STRING) *sigopts,
                              STACK_OF(X509) *rother, unsigned long flags,
                              int nmin, int ndays, int badsig,
                              const EVP_MD *resp_md)
{
    ASN1_TIME *thisupd = NULL, *nextupd = NULL;
    OCSP_CERTID *cid;
    OCSP_BASICRESP *bs = NULL;
    int i, id_count;
    EVP_MD_CTX *mctx = NULL;
    EVP_PKEY_CTX *pkctx = NULL;

    id_count = OCSP_request_onereq_count(req);

    if (id_count <= 0) {
        *resp =
            OCSP_response_create(OCSP_RESPONSE_STATUS_MALFORMEDREQUEST, NULL);
        goto end;
    }

    bs = OCSP_BASICRESP_new();
    if (bs == NULL) {
        *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR, bs);
        goto end;
    }
    thisupd = X509_gmtime_adj(NULL, 0);
    if (ndays != -1)
        nextupd = X509_time_adj_ex(NULL, ndays, nmin * 60, NULL);

    /* Examine each certificate id in the request */
    for (i = 0; i < id_count; i++) {
        OCSP_ONEREQ *one;
        ASN1_INTEGER *serial;
        char **inf;
        int jj;
        int found = 0;
        ASN1_OBJECT *cert_id_md_oid;
        const EVP_MD *cert_id_md;
        OCSP_CERTID *cid_resp_md = NULL;

        one = OCSP_request_onereq_get0(req, i);
        cid = OCSP_onereq_get0_id(one);

        OCSP_id_get0_info(NULL, &cert_id_md_oid, NULL, NULL, cid);

        cert_id_md = EVP_get_digestbyobj(cert_id_md_oid);
        if (cert_id_md == NULL) {
            *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR,
                                         NULL);
            goto end;
        }
        for (jj = 0; jj < sk_X509_num(ca) && !found; jj++) {
            X509 *ca_cert = sk_X509_value(ca, jj);
            OCSP_CERTID *ca_id = OCSP_cert_to_id(cert_id_md, NULL, ca_cert);

            if (OCSP_id_issuer_cmp(ca_id, cid) == 0) {
                found = 1;
                if (resp_md != NULL)
                    cid_resp_md = OCSP_cert_to_id(resp_md, NULL, ca_cert);
            }
            OCSP_CERTID_free(ca_id);
        }
        OCSP_id_get0_info(NULL, NULL, NULL, &serial, cid);
        inf = lookup_serial(db, serial);

        /* at this point, we can have cid be an alias of cid_resp_md */
        cid = (cid_resp_md != NULL) ? cid_resp_md : cid;

        if (!found) {
            OCSP_basic_add1_status(bs, cid,
                                   V_OCSP_CERTSTATUS_UNKNOWN,
                                   0, NULL, thisupd, nextupd);
            continue;
        }
        if (inf == NULL) {
            OCSP_basic_add1_status(bs, cid,
                                   V_OCSP_CERTSTATUS_UNKNOWN,
                                   0, NULL, thisupd, nextupd);
        } else if (inf[DB_type][0] == DB_TYPE_VAL) {
            OCSP_basic_add1_status(bs, cid,
                                   V_OCSP_CERTSTATUS_GOOD,
                                   0, NULL, thisupd, nextupd);
        } else if (inf[DB_type][0] == DB_TYPE_REV) {
            ASN1_OBJECT *inst = NULL;
            ASN1_TIME *revtm = NULL;
            ASN1_GENERALIZEDTIME *invtm = NULL;
            OCSP_SINGLERESP *single;
            int reason = -1;

            unpack_revinfo(&revtm, &reason, &inst, &invtm, inf[DB_rev_date]);
            single = OCSP_basic_add1_status(bs, cid,
                                            V_OCSP_CERTSTATUS_REVOKED,
                                            reason, revtm, thisupd, nextupd);
            if (single == NULL) {
                *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR,
                                             NULL);
                goto end;
            }
            if (invtm != NULL)
                OCSP_SINGLERESP_add1_ext_i2d(single, NID_invalidity_date,
                                             invtm, 0, 0);
            else if (inst != NULL)
                OCSP_SINGLERESP_add1_ext_i2d(single,
                                             NID_hold_instruction_code, inst,
                                             0, 0);
            ASN1_OBJECT_free(inst);
            ASN1_TIME_free(revtm);
            ASN1_GENERALIZEDTIME_free(invtm);
        }
        OCSP_CERTID_free(cid_resp_md);
    }

    OCSP_copy_nonce(bs, req);

    mctx = EVP_MD_CTX_new();
    if (mctx == NULL || !EVP_DigestSignInit(mctx, &pkctx, rmd, NULL, rkey)) {
        *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR, NULL);
        goto end;
    }
    for (i = 0; i < sk_OPENSSL_STRING_num(sigopts); i++) {
        char *sigopt = sk_OPENSSL_STRING_value(sigopts, i);

        if (pkey_ctrl_string(pkctx, sigopt) <= 0) {
            BIO_printf(err, "parameter error \"%s\"\n", sigopt);
            ERR_print_errors(bio_err);
            *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR,
                                         NULL);
            goto end;
        }
    }
    if (!OCSP_basic_sign_ctx(bs, rcert, mctx, rother, flags)) {
        *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_INTERNALERROR, bs);
        goto end;
    }

    if (badsig) {
        const ASN1_OCTET_STRING *sig = OCSP_resp_get0_signature(bs);
        corrupt_signature(sig);
    }

    *resp = OCSP_response_create(OCSP_RESPONSE_STATUS_SUCCESSFUL, bs);

 end:
    EVP_MD_CTX_free(mctx);
    ASN1_TIME_free(thisupd);
    ASN1_TIME_free(nextupd);
    OCSP_BASICRESP_free(bs);
}

static char **lookup_serial(CA_DB *db, ASN1_INTEGER *ser)
{
    int i;
    BIGNUM *bn = NULL;
    char *itmp, *row[DB_NUMBER], **rrow;
    for (i = 0; i < DB_NUMBER; i++)
        row[i] = NULL;
    bn = ASN1_INTEGER_to_BN(ser, NULL);
    OPENSSL_assert(bn);         /* FIXME: should report an error at this
                                 * point and abort */
    if (BN_is_zero(bn)) {
        itmp = OPENSSL_strdup("00");
        OPENSSL_assert(itmp);
    } else {
        itmp = BN_bn2hex(bn);
    }
    row[DB_serial] = itmp;
    BN_free(bn);
    rrow = TXT_DB_get_by_index(db->db, DB_serial, row);
    OPENSSL_free(itmp);
    return rrow;
}

static int do_responder(OCSP_REQUEST **preq, BIO **pcbio, BIO *acbio,
                        int timeout)
{
#ifndef OPENSSL_NO_SOCK
    return http_server_get_asn1_req(ASN1_ITEM_rptr(OCSP_REQUEST),
                                    (ASN1_VALUE **)preq, NULL, pcbio, acbio,
                                    NULL /* found_keep_alive */,
                                    prog, 1 /* accept_get */, timeout);
#else
    BIO_printf(bio_err,
               "Error getting OCSP request - sockets not supported\n");
    *preq = NULL;
    return 0;
#endif
}

static int send_ocsp_response(BIO *cbio, const OCSP_RESPONSE *resp)
{
#ifndef OPENSSL_NO_SOCK
    return http_server_send_asn1_resp(prog, cbio,
                                      0 /* no keep-alive */,
                                      "application/ocsp-response",
                                      ASN1_ITEM_rptr(OCSP_RESPONSE),
                                      (const ASN1_VALUE *)resp);
#else
    BIO_printf(bio_err,
               "Error sending OCSP response - sockets not supported\n");
    return 0;
#endif
}

#ifndef OPENSSL_NO_SOCK
OCSP_RESPONSE *process_responder(OCSP_REQUEST *req, const char *host,
                                 const char *port, const char *path,
                                 const char *proxy, const char *no_proxy,
                                 int use_ssl, STACK_OF(CONF_VALUE) *headers,
                                 int req_timeout)
{
    SSL_CTX *ctx = NULL;
    OCSP_RESPONSE *resp = NULL;

    if (use_ssl == 1) {
        ctx = SSL_CTX_new(TLS_client_method());
        if (ctx == NULL) {
            BIO_printf(bio_err, "Error creating SSL context.\n");
            goto end;
        }
    }

    resp = (OCSP_RESPONSE *)
        app_http_post_asn1(host, port, path, proxy, no_proxy,
                           ctx, headers, "application/ocsp-request",
                           (ASN1_VALUE *)req, ASN1_ITEM_rptr(OCSP_REQUEST),
                           "application/ocsp-response",
                           req_timeout, ASN1_ITEM_rptr(OCSP_RESPONSE));

    if (resp == NULL)
        BIO_printf(bio_err, "Error querying OCSP responder\n");

 end:
    SSL_CTX_free(ctx);
    return resp;
}
#endif
