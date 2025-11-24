/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include "apps.h"
#include "progs.h"
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/evp.h>

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_PROV_ENUM,
    OPT_CIPHER,
    OPT_SKEYOPT, OPT_SKEYMGMT, OPT_GENKEY
} OPTION_CHOICE;

const OPTIONS skeyutl_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"skeyopt", OPT_SKEYOPT, 's', "Key options as opt:value for opaque keys handling"},
    {"skeymgmt", OPT_SKEYMGMT, 's', "Symmetric key management name for opaque keys handling"},
    {"genkey", OPT_GENKEY, '-', "Generate an opaque symmetric key"},
    {"cipher", OPT_CIPHER, 's', "The cipher to generate key for"},
    OPT_PROV_OPTIONS,
    {NULL}
};

int skeyutl_main(int argc, char **argv)
{
    EVP_CIPHER *cipher = NULL;
    int ret = 1;
    OPTION_CHOICE o;
    int genkey = 0;
    char *prog, *ciphername = NULL;
    STACK_OF(OPENSSL_STRING) *skeyopts = NULL;
    const char *skeymgmt = NULL;
    EVP_SKEY *skey = NULL;
    EVP_SKEYMGMT *mgmt = NULL;

    prog = opt_init(argc, argv, skeyutl_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(skeyutl_options);
            ret = 0;
            goto end;
        case OPT_GENKEY:
            genkey = 1;
            break;
        case OPT_CIPHER:
            ciphername = opt_arg();
            break;
        case OPT_SKEYOPT:
            if ((skeyopts == NULL &&
                 (skeyopts = sk_OPENSSL_STRING_new_null()) == NULL) ||
                sk_OPENSSL_STRING_push(skeyopts, opt_arg()) == 0) {
                BIO_printf(bio_err, "%s: out of memory\n", prog);
                goto end;
            }
            break;
        case OPT_SKEYMGMT:
            skeymgmt = opt_arg();
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* Get the cipher name, either from progname (if set) or flag. */
    if (!opt_cipher_any(ciphername, &cipher))
        goto opthelp;

    if (cipher == NULL && skeymgmt == NULL) {
        BIO_printf(bio_err, "Either -skeymgmt -or -cipher option should be specified\n");
        goto end;
    }

    if (genkey) {
        OSSL_PARAM *params = NULL;

        mgmt = EVP_SKEYMGMT_fetch(app_get0_libctx(),
                                  skeymgmt ? skeymgmt : EVP_CIPHER_name(cipher),
                                  app_get0_propq());
        if (mgmt == NULL)
            goto end;
        params = app_params_new_from_opts(skeyopts,
                                          EVP_SKEYMGMT_get0_gen_settable_params(mgmt));

        skey = EVP_SKEY_generate(app_get0_libctx(),
                                 skeymgmt ? skeymgmt : EVP_CIPHER_name(cipher),
                                 app_get0_propq(), params);
        OSSL_PARAM_free(params);
        if (skey == NULL) {
            BIO_printf(bio_err, "Error creating opaque key for skeymgmt %s\n",
                       skeymgmt ? skeymgmt : EVP_CIPHER_name(cipher));
            ERR_print_errors(bio_err);
        } else {
            const char *key_name = EVP_SKEY_get0_key_id(skey);

            BIO_printf(bio_out, "An opaque key identified by %s is created\n",
                       key_name ? key_name : "<unknown>");
            BIO_printf(bio_out, "Provider: %s\n", EVP_SKEY_get0_provider_name(skey));
            BIO_printf(bio_out, "Key management: %s\n", EVP_SKEY_get0_skeymgmt_name(skey));
            ret = 0;
        }
        goto end;
    } else {
        BIO_printf(bio_err, "Key generation is the only supported operation as of now\n");
    }

 end:
    ERR_print_errors(bio_err);
    sk_OPENSSL_STRING_free(skeyopts);
    EVP_SKEYMGMT_free(mgmt);
    EVP_SKEY_free(skey);
    EVP_CIPHER_free(cipher);
    return ret;
}
