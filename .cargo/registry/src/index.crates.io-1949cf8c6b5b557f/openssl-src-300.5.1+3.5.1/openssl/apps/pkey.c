/*
 * Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include "ec_common.h"
#include <openssl/pem.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/core_names.h>

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_PASSIN, OPT_PASSOUT, OPT_ENGINE,
    OPT_IN, OPT_OUT, OPT_PUBIN, OPT_PUBOUT, OPT_TEXT_PUB,
    OPT_TEXT, OPT_NOOUT, OPT_CIPHER, OPT_TRADITIONAL, OPT_CHECK, OPT_PUB_CHECK,
    OPT_EC_PARAM_ENC, OPT_EC_CONV_FORM,
    OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS pkey_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    OPT_PROV_OPTIONS,

    {"check", OPT_CHECK, '-', "Check key consistency"},
    {"pubcheck", OPT_PUB_CHECK, '-', "Check public key consistency"},

    OPT_SECTION("Input"),
    {"in", OPT_IN, 's', "Input key"},
    {"inform", OPT_INFORM, 'f',
     "Key input format (ENGINE, other values ignored)"},
    {"passin", OPT_PASSIN, 's', "Key input pass phrase source"},
    {"pubin", OPT_PUBIN, '-',
     "Read only public components from key input"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file for encoded and/or text output"},
    {"outform", OPT_OUTFORM, 'F', "Output encoding format (DER or PEM)"},
    {"", OPT_CIPHER, '-', "Any supported cipher to be used for encryption"},
    {"passout", OPT_PASSOUT, 's', "Output PEM file pass phrase source"},
    {"traditional", OPT_TRADITIONAL, '-',
     "Use traditional format for private key PEM output"},
    {"pubout", OPT_PUBOUT, '-', "Restrict encoded output to public components"},
    {"noout", OPT_NOOUT, '-', "Do not output the key in encoded form"},
    {"text", OPT_TEXT, '-', "Output key components in plaintext"},
    {"text_pub", OPT_TEXT_PUB, '-',
     "Output only public key components in text form"},
    {"ec_conv_form", OPT_EC_CONV_FORM, 's',
     "Specifies the EC point conversion form in the encoding"},
    {"ec_param_enc", OPT_EC_PARAM_ENC, 's',
     "Specifies the way the EC parameters are encoded"},

    {NULL}
};

int pkey_main(int argc, char **argv)
{
    BIO *out = NULL;
    ENGINE *e = NULL;
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_CIPHER *cipher = NULL;
    char *infile = NULL, *outfile = NULL, *passin = NULL, *passout = NULL;
    char *passinarg = NULL, *passoutarg = NULL, *ciphername = NULL, *prog;
    OPTION_CHOICE o;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM;
    int pubin = 0, pubout = 0, text_pub = 0, text = 0, noout = 0, ret = 1;
    int private = 0, traditional = 0, check = 0, pub_check = 0;
#ifndef OPENSSL_NO_EC
    char *asn1_encoding = NULL;
    char *point_format = NULL;
#endif

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, pkey_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(pkey_options);
            ret = 0;
            goto end;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &informat))
                goto opthelp;
            break;
        case OPT_OUTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &outformat))
                goto opthelp;
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_PASSOUT:
            passoutarg = opt_arg();
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_PUBIN:
            pubin = pubout = 1;
            break;
        case OPT_PUBOUT:
            pubout = 1;
            break;
        case OPT_TEXT_PUB:
            text_pub = 1;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_TRADITIONAL:
            traditional = 1;
            break;
        case OPT_CHECK:
            check = 1;
            break;
        case OPT_PUB_CHECK:
            pub_check = 1;
            break;
        case OPT_CIPHER:
            ciphername = opt_unknown();
            break;
        case OPT_EC_CONV_FORM:
#ifdef OPENSSL_NO_EC
            goto opthelp;
#else
            point_format = opt_arg();
            if (!opt_string(point_format, point_format_options))
                goto opthelp;
            break;
#endif
        case OPT_EC_PARAM_ENC:
#ifdef OPENSSL_NO_EC
            goto opthelp;
#else
            asn1_encoding = opt_arg();
            if (!opt_string(asn1_encoding, asn1_encoding_options))
                goto opthelp;
            break;
#endif
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (text && text_pub)
        BIO_printf(bio_err,
                   "Warning: The -text option is ignored with -text_pub\n");
    if (traditional && (noout || outformat != FORMAT_PEM))
        BIO_printf(bio_err,
                   "Warning: The -traditional is ignored since there is no PEM output\n");

    /* -pubout and -text is the same as -text_pub */
    if (!text_pub && pubout && text) {
        text = 0;
        text_pub = 1;
    }

    private = (!noout && !pubout) || (text && !text_pub);

    if (!opt_cipher(ciphername, &cipher))
        goto opthelp;
    if (cipher == NULL) {
        if (passoutarg != NULL)
            BIO_printf(bio_err,
                       "Warning: The -passout option is ignored without a cipher option\n");
    } else {
        if (noout || outformat != FORMAT_PEM) {
            BIO_printf(bio_err,
                       "Error: Cipher options are supported only for PEM output\n");
            goto end;
        }
    }
    if (!app_passwd(passinarg, passoutarg, &passin, &passout)) {
        BIO_printf(bio_err, "Error getting passwords\n");
        goto end;
    }

    if (pubin)
        pkey = load_pubkey(infile, informat, 1, passin, e, "Public Key");
    else
        pkey = load_key(infile, informat, 1, passin, e, "key");
    if (pkey == NULL)
        goto end;

    out = bio_open_owner(outfile, outformat, private);
    if (out == NULL)
        goto end;

#ifndef OPENSSL_NO_EC
    if (asn1_encoding != NULL || point_format != NULL) {
        OSSL_PARAM params[3], *p = params;

        if (!EVP_PKEY_is_a(pkey, "EC"))
            goto end;

        if (asn1_encoding != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_EC_ENCODING,
                                                    asn1_encoding, 0);
        if (point_format != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(
                       OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                       point_format, 0);
        *p = OSSL_PARAM_construct_end();
        if (EVP_PKEY_set_params(pkey, params) <= 0)
            goto end;
    }
#endif

    if (check || pub_check) {
        int r;

        ctx = EVP_PKEY_CTX_new(pkey, e);
        if (ctx == NULL) {
            ERR_print_errors(bio_err);
            goto end;
        }

        if (check && !pubin)
            r = EVP_PKEY_check(ctx);
        else
            r = EVP_PKEY_public_check(ctx);

        if (r == 1) {
            BIO_printf(out, "Key is valid\n");
        } else {
            /*
             * Note: at least for RSA keys if this function returns
             * -1, there will be no error reasons.
             */
            BIO_printf(bio_err, "Key is invalid\n");
            ERR_print_errors(bio_err);
            goto end;
        }
    }

    if (!noout) {
        if (outformat == FORMAT_PEM) {
            if (pubout) {
                if (!PEM_write_bio_PUBKEY(out, pkey))
                    goto end;
            } else {
                assert(private);
                if (traditional) {
                    if (!PEM_write_bio_PrivateKey_traditional(out, pkey, cipher,
                                                              NULL, 0, NULL,
                                                              passout))
                        goto end;
                } else {
                    if (!PEM_write_bio_PrivateKey(out, pkey, cipher,
                                                  NULL, 0, NULL, passout))
                        goto end;
                }
            }
        } else if (outformat == FORMAT_ASN1) {
            if (text || text_pub) {
                BIO_printf(bio_err,
                           "Error: Text output cannot be combined with DER output\n");
                goto end;
            }
            if (pubout) {
                if (!i2d_PUBKEY_bio(out, pkey))
                    goto end;
            } else {
                assert(private);
                if (!i2d_PrivateKey_bio(out, pkey))
                    goto end;
            }
        } else {
            BIO_printf(bio_err, "Bad format specified for key\n");
            goto end;
        }
    }

    if (text_pub) {
        if (EVP_PKEY_print_public(out, pkey, 0, NULL) <= 0)
            goto end;
    } else if (text) {
        assert(private);
        if (EVP_PKEY_print_private(out, pkey, 0, NULL) <= 0)
            goto end;
    }

    ret = 0;

 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    EVP_CIPHER_free(cipher);
    release_engine(e);
    BIO_free_all(out);
    OPENSSL_free(passin);
    OPENSSL_free(passout);

    return ret;
}
