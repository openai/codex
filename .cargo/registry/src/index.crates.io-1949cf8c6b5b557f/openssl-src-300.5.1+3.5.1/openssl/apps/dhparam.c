/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/dsa.h>
#include <openssl/dh.h>
#include <openssl/x509.h>
#include <openssl/pem.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/param_build.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>

#define DEFBITS 2048

static EVP_PKEY *dsa_to_dh(EVP_PKEY *dh);

static int verbose = 1;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_IN, OPT_OUT,
    OPT_ENGINE, OPT_CHECK, OPT_TEXT, OPT_NOOUT,
    OPT_DSAPARAM, OPT_2, OPT_3, OPT_5, OPT_VERBOSE, OPT_QUIET,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS dhparam_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [numbits]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"check", OPT_CHECK, '-', "Check the DH parameters"},
#if !defined(OPENSSL_NO_DSA) || !defined(OPENSSL_NO_DEPRECATED_3_0)
    {"dsaparam", OPT_DSAPARAM, '-',
     "Read or generate DSA parameters, convert to DH"},
#endif
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine e, possibly a hardware device"},
#endif

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file"},
    {"inform", OPT_INFORM, 'F', "Input format, DER or PEM"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'F', "Output format, DER or PEM"},
    {"text", OPT_TEXT, '-', "Print a text form of the DH parameters"},
    {"noout", OPT_NOOUT, '-', "Don't output any DH parameters"},
    {"2", OPT_2, '-', "Generate parameters using 2 as the generator value"},
    {"3", OPT_3, '-', "Generate parameters using 3 as the generator value"},
    {"5", OPT_5, '-', "Generate parameters using 5 as the generator value"},
    {"verbose", OPT_VERBOSE, '-', "Verbose output"},
    {"quiet", OPT_QUIET, '-', "Terse output"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"numbits", 0, 0, "Number of bits if generating parameters (optional)"},
    {NULL}
};

int dhparam_main(int argc, char **argv)
{
    BIO *in = NULL, *out = NULL;
    EVP_PKEY *pkey = NULL, *tmppkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    char *infile = NULL, *outfile = NULL, *prog;
    ENGINE *e = NULL;
    int dsaparam = 0;
    int text = 0, ret = 1, num = 0, g = 0;
    int informat = FORMAT_PEM, outformat = FORMAT_PEM, check = 0, noout = 0;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, dhparam_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(dhparam_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &informat))
                goto opthelp;
            break;
        case OPT_OUTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &outformat))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_CHECK:
            check = 1;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_DSAPARAM:
            dsaparam = 1;
            break;
        case OPT_2:
            g = 2;
            break;
        case OPT_3:
            g = 3;
            break;
        case OPT_5:
            g = 5;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_VERBOSE:
            verbose = 1;
            break;
        case OPT_QUIET:
            verbose = 0;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* One optional argument, bitsize to generate. */
    argc = opt_num_rest();
    argv = opt_rest();
    if (argc == 1) {
        if (!opt_int(argv[0], &num) || num <= 0)
            goto opthelp;
    } else if (!opt_check_rest_arg(NULL)) {
        goto opthelp;
    }
    if (!app_RAND_load())
        goto end;

    if (g && !num)
        num = DEFBITS;

    if (dsaparam && g) {
        BIO_printf(bio_err,
                   "Error, generator may not be chosen for DSA parameters\n");
        goto end;
    }

    /* DH parameters */
    if (num && !g)
        g = 2;

    if (num) {
        const char *alg = dsaparam ? "DSA" : "DH";

        if (infile != NULL) {
            BIO_printf(bio_err, "Warning, input file %s ignored\n", infile);
        }

        ctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(), alg, app_get0_propq());
        if (ctx == NULL) {
            BIO_printf(bio_err,
                        "Error, %s param generation context allocation failed\n",
                        alg);
            goto end;
        }
        EVP_PKEY_CTX_set_app_data(ctx, bio_err);
        if (verbose) {
            EVP_PKEY_CTX_set_cb(ctx, progress_cb);
            BIO_printf(bio_err,
                        "Generating %s parameters, %d bit long %sprime\n",
                        alg, num, dsaparam ? "" : "safe ");
        }

        if (EVP_PKEY_paramgen_init(ctx) <= 0) {
            BIO_printf(bio_err,
                        "Error, unable to initialise %s parameters\n",
                        alg);
            goto end;
        }

        if (dsaparam) {
            if (EVP_PKEY_CTX_set_dsa_paramgen_bits(ctx, num) <= 0) {
                BIO_printf(bio_err, "Error, unable to set DSA prime length\n");
                goto end;
            }
        } else {
            if (EVP_PKEY_CTX_set_dh_paramgen_prime_len(ctx, num) <= 0) {
                BIO_printf(bio_err, "Error, unable to set DH prime length\n");
                goto end;
            }
            if (EVP_PKEY_CTX_set_dh_paramgen_generator(ctx, g) <= 0) {
                BIO_printf(bio_err, "Error, unable to set generator\n");
                goto end;
            }
        }

        tmppkey = app_paramgen(ctx, alg);
        if (tmppkey == NULL)
            goto end;
        EVP_PKEY_CTX_free(ctx);
        ctx = NULL;
        if (dsaparam) {
            pkey = dsa_to_dh(tmppkey);
            if (pkey == NULL)
                goto end;
            EVP_PKEY_free(tmppkey);
        } else {
            pkey = tmppkey;
        }
        tmppkey = NULL;
    } else {
        OSSL_DECODER_CTX *decoderctx = NULL;
        const char *keytype = "DH";
        int done;

        in = bio_open_default(infile, 'r', informat);
        if (in == NULL)
            goto end;

        do {
            /*
             * We assume we're done unless we explicitly want to retry and set
             * this to 0 below.
             */
            done = 1;
            /*
            * We set NULL for the keytype to allow any key type. We don't know
            * if we're going to get DH or DHX (or DSA in the event of dsaparam).
            * We check that we got one of those key types afterwards.
            */
            decoderctx
                = OSSL_DECODER_CTX_new_for_pkey(&tmppkey,
                                                (informat == FORMAT_ASN1)
                                                    ? "DER" : "PEM",
                                                NULL,
                                                (informat == FORMAT_ASN1)
                                                    ? keytype : NULL,
                                                OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                                                NULL, NULL);

            if (decoderctx != NULL
                    && !OSSL_DECODER_from_bio(decoderctx, in)
                    && informat == FORMAT_ASN1
                    && strcmp(keytype, "DH") == 0) {
                /*
                * When reading DER we explicitly state the expected keytype
                * because, unlike PEM, there is no header to declare what
                * the contents of the DER file are. The decoders just try
                * and guess. Unfortunately with DHX key types they may guess
                * wrong and think we have a DSA keytype. Therefore, we try
                * both DH and DHX sequentially.
                */
                keytype = "DHX";
                /*
                 * BIO_reset() returns 0 for success for file BIOs only!!!
                 * This won't work for stdin (and never has done)
                 */
                if (BIO_reset(in) == 0)
                    done = 0;
            }
            OSSL_DECODER_CTX_free(decoderctx);
        } while (!done);
        if (tmppkey == NULL) {
            BIO_printf(bio_err, "Error, unable to load parameters\n");
            goto end;
        }

        if (dsaparam) {
            if (!EVP_PKEY_is_a(tmppkey, "DSA")) {
                BIO_printf(bio_err, "Error, unable to load DSA parameters\n");
                goto end;
            }
            pkey = dsa_to_dh(tmppkey);
            if (pkey == NULL)
                goto end;
        } else {
            if (!EVP_PKEY_is_a(tmppkey, "DH")
                    && !EVP_PKEY_is_a(tmppkey, "DHX")) {
                BIO_printf(bio_err, "Error, unable to load DH parameters\n");
                goto end;
            }
            pkey = tmppkey;
            tmppkey = NULL;
        }
    }

    out = bio_open_default(outfile, 'w', outformat);
    if (out == NULL)
        goto end;

    if (text)
        EVP_PKEY_print_params(out, pkey, 4, NULL);

    if (check) {
        ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(), pkey, app_get0_propq());
        if (ctx == NULL) {
            BIO_printf(bio_err, "Error, failed to check DH parameters\n");
            goto end;
        }
        if (EVP_PKEY_param_check(ctx) <= 0) {
            BIO_printf(bio_err, "Error, invalid parameters generated\n");
            goto end;
        }
        BIO_printf(bio_err, "DH parameters appear to be ok.\n");
    }

    if (!noout) {
        OSSL_ENCODER_CTX *ectx =
            OSSL_ENCODER_CTX_new_for_pkey(pkey,
                                          OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                                          outformat == FORMAT_ASN1
                                              ? "DER" : "PEM",
                                          NULL, NULL);

        if (ectx == NULL || !OSSL_ENCODER_to_bio(ectx, out)) {
            OSSL_ENCODER_CTX_free(ectx);
            BIO_printf(bio_err, "Error, unable to write DH parameters\n");
            goto end;
        }
        OSSL_ENCODER_CTX_free(ectx);
    }
    ret = 0;
 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    BIO_free(in);
    BIO_free_all(out);
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(tmppkey);
    EVP_PKEY_CTX_free(ctx);
    release_engine(e);
    return ret;
}

/*
 * Historically we had the low-level call DSA_dup_DH() to do this.
 * That is now deprecated with no replacement. Since we still need to do this
 * for backwards compatibility reasons, we do it "manually".
 */
static EVP_PKEY *dsa_to_dh(EVP_PKEY *dh)
{
    OSSL_PARAM_BLD *tmpl = NULL;
    OSSL_PARAM *params = NULL;
    BIGNUM *bn_p = NULL, *bn_q = NULL, *bn_g = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *pkey = NULL;

    if (!EVP_PKEY_get_bn_param(dh, OSSL_PKEY_PARAM_FFC_P, &bn_p)
            || !EVP_PKEY_get_bn_param(dh, OSSL_PKEY_PARAM_FFC_Q, &bn_q)
            || !EVP_PKEY_get_bn_param(dh, OSSL_PKEY_PARAM_FFC_G, &bn_g)) {
        BIO_printf(bio_err, "Error, failed to set DH parameters\n");
        goto err;
    }

    if ((tmpl = OSSL_PARAM_BLD_new()) == NULL
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_P,
                                        bn_p)
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_Q,
                                        bn_q)
            || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_G,
                                        bn_g)
            || (params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL) {
        BIO_printf(bio_err, "Error, failed to set DH parameters\n");
        goto err;
    }

    ctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(), "DHX", app_get0_propq());
    if (ctx == NULL
            || EVP_PKEY_fromdata_init(ctx) <= 0
            || EVP_PKEY_fromdata(ctx, &pkey, EVP_PKEY_KEY_PARAMETERS, params) <= 0) {
        BIO_printf(bio_err, "Error, failed to set DH parameters\n");
        goto err;
    }

 err:
    EVP_PKEY_CTX_free(ctx);
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(tmpl);
    BN_free(bn_p);
    BN_free(bn_q);
    BN_free(bn_g);
    return pkey;
}

