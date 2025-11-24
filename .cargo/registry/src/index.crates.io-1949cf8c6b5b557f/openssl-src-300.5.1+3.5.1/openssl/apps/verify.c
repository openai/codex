/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/pem.h>

static int cb(int ok, X509_STORE_CTX *ctx);
static int check(X509_STORE *ctx, const char *file,
                 STACK_OF(X509) *uchain, STACK_OF(X509) *tchain,
                 STACK_OF(X509_CRL) *crls, int show_chain,
                 STACK_OF(OPENSSL_STRING) *opts);
static int v_verbose = 0, vflags = 0;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ENGINE, OPT_CAPATH, OPT_CAFILE, OPT_CASTORE,
    OPT_NOCAPATH, OPT_NOCAFILE, OPT_NOCASTORE,
    OPT_UNTRUSTED, OPT_TRUSTED, OPT_CRLFILE, OPT_CRL_DOWNLOAD, OPT_SHOW_CHAIN,
    OPT_V_ENUM, OPT_NAMEOPT, OPT_VFYOPT,
    OPT_VERBOSE,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS verify_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [cert...]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    {"verbose", OPT_VERBOSE, '-',
        "Print extra information about the operations being performed."},
    {"nameopt", OPT_NAMEOPT, 's', "Certificate subject/issuer name printing options"},

    OPT_SECTION("Certificate chain"),
    {"trusted", OPT_TRUSTED, '<', "A file of trusted certificates"},
    {"CAfile", OPT_CAFILE, '<', "A file of trusted certificates"},
    {"CApath", OPT_CAPATH, '/', "A directory of files with trusted certificates"},
    {"CAstore", OPT_CASTORE, ':', "URI to a store of trusted certificates"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default trusted certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load trusted certificates from the default directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load trusted certificates from the default certificates store"},
    {"untrusted", OPT_UNTRUSTED, '<', "A file of untrusted certificates"},
    {"CRLfile", OPT_CRLFILE, '<',
        "File containing one or more CRL's (in PEM format) to load"},
    {"crl_download", OPT_CRL_DOWNLOAD, '-',
        "Try downloading CRL information for certificates via their CDP entries"},
    {"show_chain", OPT_SHOW_CHAIN, '-',
        "Display information about the certificate chain"},

    OPT_V_OPTIONS,
    {"vfyopt", OPT_VFYOPT, 's', "Verification parameter in n:v form"},

    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"cert", 0, 0, "Certificate(s) to verify (optional; stdin used otherwise)"},
    {NULL}
};

int verify_main(int argc, char **argv)
{
    ENGINE *e = NULL;
    STACK_OF(X509) *untrusted = NULL, *trusted = NULL;
    STACK_OF(X509_CRL) *crls = NULL;
    STACK_OF(OPENSSL_STRING) *vfyopts = NULL;
    X509_STORE *store = NULL;
    X509_VERIFY_PARAM *vpm = NULL;
    const char *prog, *CApath = NULL, *CAfile = NULL, *CAstore = NULL;
    int noCApath = 0, noCAfile = 0, noCAstore = 0;
    int vpmtouched = 0, crl_download = 0, show_chain = 0, i = 0, ret = 1;
    OPTION_CHOICE o;

    if ((vpm = X509_VERIFY_PARAM_new()) == NULL)
        goto end;

    prog = opt_init(argc, argv, verify_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(verify_options);
            BIO_printf(bio_err, "\nRecognized certificate chain purposes:\n");
            for (i = 0; i < X509_PURPOSE_get_count(); i++) {
                X509_PURPOSE *ptmp = X509_PURPOSE_get0(i);

                BIO_printf(bio_err, "  %-15s  %s\n",
                        X509_PURPOSE_get0_sname(ptmp),
                        X509_PURPOSE_get0_name(ptmp));
            }

            BIO_printf(bio_err, "Recognized certificate policy names:\n");
            for (i = 0; i < X509_VERIFY_PARAM_get_count(); i++) {
                const X509_VERIFY_PARAM *vptmp = X509_VERIFY_PARAM_get0(i);

                BIO_printf(bio_err, "  %s\n",
                        X509_VERIFY_PARAM_get0_name(vptmp));
            }
            ret = 0;
            goto end;
        case OPT_V_CASES:
            if (!opt_verify(o, vpm))
                goto end;
            vpmtouched++;
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            break;
        case OPT_NOCAPATH:
            noCApath = 1;
            break;
        case OPT_NOCAFILE:
            noCAfile = 1;
            break;
        case OPT_NOCASTORE:
            noCAstore = 1;
            break;
        case OPT_UNTRUSTED:
            /* Zero or more times */
            if (!load_certs(opt_arg(), 0, &untrusted, NULL,
                            "untrusted certificates"))
                goto end;
            break;
        case OPT_TRUSTED:
            /* Zero or more times */
            noCAfile = 1;
            noCApath = 1;
            noCAstore = 1;
            if (!load_certs(opt_arg(), 0, &trusted, NULL, "trusted certificates"))
                goto end;
            break;
        case OPT_CRLFILE:
            /* Zero or more times */
            if (!load_crls(opt_arg(), &crls, NULL, "other CRLs"))
                goto end;
            break;
        case OPT_CRL_DOWNLOAD:
            crl_download = 1;
            break;
        case OPT_ENGINE:
            if ((e = setup_engine(opt_arg(), 0)) == NULL) {
                /* Failure message already displayed */
                goto end;
            }
            break;
        case OPT_SHOW_CHAIN:
            show_chain = 1;
            break;
        case OPT_NAMEOPT:
            if (!set_nameopt(opt_arg()))
                goto end;
            break;
        case OPT_VFYOPT:
            if (!vfyopts)
                vfyopts = sk_OPENSSL_STRING_new_null();
            if (!vfyopts || !sk_OPENSSL_STRING_push(vfyopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_VERBOSE:
            v_verbose = 1;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* Extra arguments are certificates to verify. */
    argc = opt_num_rest();
    argv = opt_rest();

    if (trusted != NULL
        && (CAfile != NULL || CApath != NULL || CAstore != NULL)) {
        BIO_printf(bio_err,
                   "%s: Cannot use -trusted with -CAfile, -CApath or -CAstore\n",
                   prog);
        goto end;
    }

    if ((store = setup_verify(CAfile, noCAfile, CApath, noCApath,
                              CAstore, noCAstore)) == NULL)
        goto end;
    X509_STORE_set_verify_cb(store, cb);

    if (vpmtouched)
        X509_STORE_set1_param(store, vpm);

    ERR_clear_error();

    if (crl_download)
        store_setup_crl_download(store);

    ret = 0;
    if (argc < 1) {
        if (check(store, NULL, untrusted, trusted, crls, show_chain,
                  vfyopts) != 1)
            ret = -1;
    } else {
        for (i = 0; i < argc; i++)
            if (check(store, argv[i], untrusted, trusted, crls, show_chain,
                      vfyopts) != 1)
                ret = -1;
    }

 end:
    X509_VERIFY_PARAM_free(vpm);
    X509_STORE_free(store);
    OSSL_STACK_OF_X509_free(untrusted);
    OSSL_STACK_OF_X509_free(trusted);
    sk_X509_CRL_pop_free(crls, X509_CRL_free);
    sk_OPENSSL_STRING_free(vfyopts);
    release_engine(e);
    return (ret < 0 ? 2 : ret);
}

static int check(X509_STORE *ctx, const char *file,
                 STACK_OF(X509) *uchain, STACK_OF(X509) *tchain,
                 STACK_OF(X509_CRL) *crls, int show_chain,
                 STACK_OF(OPENSSL_STRING) *opts)
{
    X509 *x = NULL;
    int i = 0, ret = 0;
    X509_STORE_CTX *csc;
    STACK_OF(X509) *chain = NULL;
    int num_untrusted;

    x = load_cert(file, FORMAT_UNDEF, "certificate file");
    if (x == NULL)
        goto end;

    if (opts != NULL) {
        for (i = 0; i < sk_OPENSSL_STRING_num(opts); i++) {
            char *opt = sk_OPENSSL_STRING_value(opts, i);
            if (x509_ctrl_string(x, opt) <= 0) {
                BIO_printf(bio_err, "parameter error \"%s\"\n", opt);
                ERR_print_errors(bio_err);
                X509_free(x);
                return 0;
            }
        }
    }

    csc = X509_STORE_CTX_new();
    if (csc == NULL) {
        BIO_printf(bio_err, "error %s: X.509 store context allocation failed\n",
                   (file == NULL) ? "stdin" : file);
        goto end;
    }

    X509_STORE_set_flags(ctx, vflags);
    if (!X509_STORE_CTX_init(csc, ctx, x, uchain)) {
        X509_STORE_CTX_free(csc);
        BIO_printf(bio_err,
                   "error %s: X.509 store context initialization failed\n",
                   (file == NULL) ? "stdin" : file);
        goto end;
    }
    if (tchain != NULL)
        X509_STORE_CTX_set0_trusted_stack(csc, tchain);
    if (crls != NULL)
        X509_STORE_CTX_set0_crls(csc, crls);
    i = X509_verify_cert(csc);
    if (i > 0 && X509_STORE_CTX_get_error(csc) == X509_V_OK) {
        BIO_printf(bio_out, "%s: OK\n", (file == NULL) ? "stdin" : file);
        ret = 1;
        if (show_chain) {
            int j;

            chain = X509_STORE_CTX_get1_chain(csc);
            num_untrusted = X509_STORE_CTX_get_num_untrusted(csc);
            BIO_printf(bio_out, "Chain:\n");
            for (j = 0; j < sk_X509_num(chain); j++) {
                X509 *cert = sk_X509_value(chain, j);
                BIO_printf(bio_out, "depth=%d: ", j);
                X509_NAME_print_ex_fp(stdout,
                                      X509_get_subject_name(cert),
                                      0, get_nameopt());
                if (j < num_untrusted)
                    BIO_printf(bio_out, " (untrusted)");
                BIO_printf(bio_out, "\n");
            }
            OSSL_STACK_OF_X509_free(chain);
        }
    } else {
        BIO_printf(bio_err,
                   "error %s: verification failed\n",
                   (file == NULL) ? "stdin" : file);
    }
    X509_STORE_CTX_free(csc);

 end:
    if (i <= 0)
        ERR_print_errors(bio_err);
    X509_free(x);

    return ret;
}

static int cb(int ok, X509_STORE_CTX *ctx)
{
    int cert_error = X509_STORE_CTX_get_error(ctx);
    X509 *current_cert = X509_STORE_CTX_get_current_cert(ctx);

    if (!ok) {
        if (current_cert != NULL) {
            X509_NAME_print_ex(bio_err,
                            X509_get_subject_name(current_cert),
                            0, get_nameopt());
            BIO_printf(bio_err, "\n");
        }
        BIO_printf(bio_err, "%serror %d at %d depth lookup: %s\n",
               X509_STORE_CTX_get0_parent_ctx(ctx) ? "[CRL path] " : "",
               cert_error,
               X509_STORE_CTX_get_error_depth(ctx),
               X509_verify_cert_error_string(cert_error));

        /*
         * Pretend that some errors are ok, so they don't stop further
         * processing of the certificate chain.  Setting ok = 1 does this.
         * After X509_verify_cert() is done, we verify that there were
         * no actual errors, even if the returned value was positive.
         */
        switch (cert_error) {
        case X509_V_ERR_NO_EXPLICIT_POLICY:
            policies_print(ctx);
            /* fall through */
        case X509_V_ERR_CERT_HAS_EXPIRED:
            /* Continue even if the leaf is a self-signed cert */
        case X509_V_ERR_DEPTH_ZERO_SELF_SIGNED_CERT:
            /* Continue after extension errors too */
        case X509_V_ERR_INVALID_CA:
        case X509_V_ERR_INVALID_NON_CA:
        case X509_V_ERR_PATH_LENGTH_EXCEEDED:
        case X509_V_ERR_CRL_HAS_EXPIRED:
        case X509_V_ERR_CRL_NOT_YET_VALID:
        case X509_V_ERR_UNHANDLED_CRITICAL_EXTENSION:
            /* errors due to strict conformance checking (-x509_strict) */
        case X509_V_ERR_INVALID_PURPOSE:
        case X509_V_ERR_PATHLEN_INVALID_FOR_NON_CA:
        case X509_V_ERR_PATHLEN_WITHOUT_KU_KEY_CERT_SIGN:
        case X509_V_ERR_CA_BCONS_NOT_CRITICAL:
        case X509_V_ERR_CA_CERT_MISSING_KEY_USAGE:
        case X509_V_ERR_KU_KEY_CERT_SIGN_INVALID_FOR_NON_CA:
        case X509_V_ERR_ISSUER_NAME_EMPTY:
        case X509_V_ERR_SUBJECT_NAME_EMPTY:
        case X509_V_ERR_EMPTY_SUBJECT_SAN_NOT_CRITICAL:
        case X509_V_ERR_EMPTY_SUBJECT_ALT_NAME:
        case X509_V_ERR_SIGNATURE_ALGORITHM_INCONSISTENCY:
        case X509_V_ERR_AUTHORITY_KEY_IDENTIFIER_CRITICAL:
        case X509_V_ERR_SUBJECT_KEY_IDENTIFIER_CRITICAL:
        case X509_V_ERR_MISSING_AUTHORITY_KEY_IDENTIFIER:
        case X509_V_ERR_MISSING_SUBJECT_KEY_IDENTIFIER:
        case X509_V_ERR_EXTENSIONS_REQUIRE_VERSION_3:
            ok = 1;
        }
        return ok;

    }
    if (cert_error == X509_V_OK && ok == 2)
        policies_print(ctx);
    if (!v_verbose)
        ERR_clear_error();
    return ok;
}
