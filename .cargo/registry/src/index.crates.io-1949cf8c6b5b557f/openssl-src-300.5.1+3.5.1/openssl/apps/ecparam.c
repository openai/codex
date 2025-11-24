/*
 * Copyright 2002-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/evp.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include "apps.h"
#include "progs.h"
#include "ec_common.h"

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_IN, OPT_OUT, OPT_TEXT,
    OPT_CHECK, OPT_LIST_CURVES, OPT_NO_SEED, OPT_NOOUT, OPT_NAME,
    OPT_CONV_FORM, OPT_PARAM_ENC, OPT_GENKEY, OPT_ENGINE, OPT_CHECK_NAMED,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS ecparam_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"list_curves", OPT_LIST_CURVES, '-',
     "Prints a list of all curve 'short names'"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    {"genkey", OPT_GENKEY, '-', "Generate ec key"},
    {"in", OPT_IN, '<', "Input file  - default stdin"},
    {"inform", OPT_INFORM, 'F', "Input format - default PEM (DER or PEM)"},
    {"out", OPT_OUT, '>', "Output file - default stdout"},
    {"outform", OPT_OUTFORM, 'F', "Output format - default PEM"},

    OPT_SECTION("Output"),
    {"text", OPT_TEXT, '-', "Print the ec parameters in text form"},
    {"noout", OPT_NOOUT, '-', "Do not print the ec parameter"},
    {"param_enc", OPT_PARAM_ENC, 's',
     "Specifies the way the ec parameters are encoded"},

    OPT_SECTION("Parameter"),
    {"check", OPT_CHECK, '-', "Validate the ec parameters"},
    {"check_named", OPT_CHECK_NAMED, '-',
     "Check that named EC curve parameters have not been modified"},
    {"no_seed", OPT_NO_SEED, '-',
     "If 'explicit' parameters are chosen do not use the seed"},
    {"name", OPT_NAME, 's',
     "Use the ec parameters with specified 'short name'"},
    {"conv_form", OPT_CONV_FORM, 's', "Specifies the point conversion form "},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

static int list_builtin_curves(BIO *out)
{
    EC_builtin_curve *curves = NULL;
    size_t n, crv_len = EC_get_builtin_curves(NULL, 0);

    curves = app_malloc((int)sizeof(*curves) * crv_len, "list curves");
    EC_get_builtin_curves(curves, crv_len);

    for (n = 0; n < crv_len; n++) {
        const char *comment = curves[n].comment;
        const char *sname = OBJ_nid2sn(curves[n].nid);

        if (comment == NULL)
            comment = "CURVE DESCRIPTION NOT AVAILABLE";
        if (sname == NULL)
            sname = "";

        BIO_printf(out, "  %-10s: ", sname);
        BIO_printf(out, "%s\n", comment);
    }
    OPENSSL_free(curves);
    return 1;
}

int ecparam_main(int argc, char **argv)
{
    EVP_PKEY_CTX *gctx_params = NULL, *gctx_key = NULL, *pctx = NULL;
    EVP_PKEY *params_key = NULL, *key = NULL;
    OSSL_ENCODER_CTX *ectx_key = NULL, *ectx_params = NULL;
    OSSL_DECODER_CTX *dctx_params = NULL;
    ENGINE *e = NULL;
    BIO *out = NULL;
    char *curve_name = NULL;
    char *asn1_encoding = NULL;
    char *point_format = NULL;
    char *infile = NULL, *outfile = NULL, *prog;
    OPTION_CHOICE o;
    int informat = FORMAT_PEM, outformat = FORMAT_PEM, noout = 0;
    int ret = 1, private = 0;
    int no_seed = 0, check = 0, check_named = 0, text = 0, genkey = 0;
    int list_curves = 0;

    prog = opt_init(argc, argv, ecparam_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(ecparam_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &informat))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &outformat))
                goto opthelp;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_CHECK:
            check = 1;
            break;
        case OPT_CHECK_NAMED:
            check_named = 1;
            break;
        case OPT_LIST_CURVES:
            list_curves = 1;
            break;
        case OPT_NO_SEED:
            no_seed = 1;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_NAME:
            curve_name = opt_arg();
            break;
        case OPT_CONV_FORM:
            point_format = opt_arg();
            if (!opt_string(point_format, point_format_options))
                goto opthelp;
            break;
        case OPT_PARAM_ENC:
            asn1_encoding = opt_arg();
            if (!opt_string(asn1_encoding, asn1_encoding_options))
                goto opthelp;
            break;
        case OPT_GENKEY:
            genkey = 1;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        }
    }

    /* No extra args. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!app_RAND_load())
        goto end;

    if (list_curves) {
        out = bio_open_owner(outfile, outformat, private);
        if (out == NULL)
            goto end;

        if (list_builtin_curves(out))
            ret = 0;
        goto end;
    }

    private = genkey ? 1 : 0;

    if (curve_name != NULL) {
        OSSL_PARAM params[4];
        OSSL_PARAM *p = params;

        if (strcmp(curve_name, "secp192r1") == 0) {
            BIO_printf(bio_err,
                       "using curve name prime192v1 instead of secp192r1\n");
            curve_name = SN_X9_62_prime192v1;
        } else if (strcmp(curve_name, "secp256r1") == 0) {
            BIO_printf(bio_err,
                       "using curve name prime256v1 instead of secp256r1\n");
            curve_name = SN_X9_62_prime256v1;
        }
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                curve_name, 0);
        if (asn1_encoding != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_EC_ENCODING,
                                                    asn1_encoding, 0);
        if (point_format != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(
                       OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                       point_format, 0);
        *p = OSSL_PARAM_construct_end();

        if (OPENSSL_strcasecmp(curve_name, "SM2") == 0)
            gctx_params = EVP_PKEY_CTX_new_from_name(app_get0_libctx(), "sm2",
                                                     app_get0_propq());
        else
            gctx_params = EVP_PKEY_CTX_new_from_name(app_get0_libctx(), "ec",
                                                     app_get0_propq());
        if (gctx_params == NULL
            || EVP_PKEY_keygen_init(gctx_params) <= 0
            || EVP_PKEY_CTX_set_params(gctx_params, params) <= 0
            || EVP_PKEY_keygen(gctx_params, &params_key) <= 0) {
            BIO_printf(bio_err, "unable to generate key\n");
            goto end;
        }
    } else {
        params_key = load_keyparams_suppress(infile, informat, 1, "EC",
                                             "EC parameters", 1);
        if (params_key == NULL)
            params_key = load_keyparams_suppress(infile, informat, 1, "SM2",
                                                 "SM2 parameters", 1);

        if (params_key == NULL) {
            BIO_printf(bio_err, "Unable to load parameters from %s\n", infile);
            goto end;
        }

        if (point_format
            && !EVP_PKEY_set_utf8_string_param(
                    params_key, OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                    point_format)) {
            BIO_printf(bio_err, "unable to set point conversion format\n");
            goto end;
        }

        if (asn1_encoding != NULL
            && !EVP_PKEY_set_utf8_string_param(
                    params_key, OSSL_PKEY_PARAM_EC_ENCODING, asn1_encoding)) {
            BIO_printf(bio_err, "unable to set asn1 encoding format\n");
            goto end;
        }
    }

    if (no_seed
        && !EVP_PKEY_set_octet_string_param(params_key, OSSL_PKEY_PARAM_EC_SEED,
                                            NULL, 0)) {
        BIO_printf(bio_err, "unable to clear seed\n");
        goto end;
    }

    out = bio_open_owner(outfile, outformat, private);
    if (out == NULL)
        goto end;

    if (text
        && EVP_PKEY_print_params(out, params_key, 0, NULL) <= 0) {
        BIO_printf(bio_err, "unable to print params\n");
        goto end;
    }

    if (check || check_named) {
        BIO_printf(bio_err, "checking elliptic curve parameters: ");

        if (check_named
            && !EVP_PKEY_set_utf8_string_param(params_key,
                                           OSSL_PKEY_PARAM_EC_GROUP_CHECK_TYPE,
                                           OSSL_PKEY_EC_GROUP_CHECK_NAMED)) {
                BIO_printf(bio_err, "unable to set check_type\n");
                goto end;
        }
        pctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(), params_key,
                                          app_get0_propq());
        if (pctx == NULL || EVP_PKEY_param_check(pctx) <= 0) {
            BIO_printf(bio_err, "failed\n");
            goto end;
        }
        BIO_printf(bio_err, "ok\n");
    }

    if (outformat == FORMAT_ASN1 && genkey)
        noout = 1;

    if (!noout) {
        ectx_params = OSSL_ENCODER_CTX_new_for_pkey(
                          params_key, OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                          outformat == FORMAT_ASN1 ? "DER" : "PEM", NULL, NULL);
        if (!OSSL_ENCODER_to_bio(ectx_params, out)) {
            BIO_printf(bio_err, "unable to write elliptic curve parameters\n");
            goto end;
        }
    }

    if (genkey) {
        /*
         * NOTE: EC keygen does not normally need to pass in the param_key
         * for named curves. This can be achieved using:
         *    gctx = EVP_PKEY_CTX_new_from_name(NULL, "EC", NULL);
         *    EVP_PKEY_keygen_init(gctx);
         *    EVP_PKEY_CTX_set_group_name(gctx, curvename);
         *    EVP_PKEY_keygen(gctx, &key) <= 0)
         */
        gctx_key = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(), params_key,
                                              app_get0_propq());
        if (EVP_PKEY_keygen_init(gctx_key) <= 0
            || EVP_PKEY_keygen(gctx_key, &key) <= 0) {
            BIO_printf(bio_err, "unable to generate key\n");
            goto end;
        }
        assert(private);
        ectx_key = OSSL_ENCODER_CTX_new_for_pkey(
                       key, OSSL_KEYMGMT_SELECT_ALL,
                       outformat == FORMAT_ASN1 ? "DER" : "PEM", NULL, NULL);
        if (!OSSL_ENCODER_to_bio(ectx_key, out)) {
            BIO_printf(bio_err, "unable to write elliptic "
                       "curve parameters\n");
            goto end;
        }
    }

    ret = 0;
end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    release_engine(e);
    EVP_PKEY_free(params_key);
    EVP_PKEY_free(key);
    EVP_PKEY_CTX_free(pctx);
    EVP_PKEY_CTX_free(gctx_params);
    EVP_PKEY_CTX_free(gctx_key);
    OSSL_DECODER_CTX_free(dctx_params);
    OSSL_ENCODER_CTX_free(ectx_params);
    OSSL_ENCODER_CTX_free(ectx_key);
    BIO_free_all(out);
    return ret;
}
