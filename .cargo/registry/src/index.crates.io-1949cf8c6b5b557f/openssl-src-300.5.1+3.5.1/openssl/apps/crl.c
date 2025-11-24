/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
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

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_IN, OPT_OUTFORM, OPT_OUT, OPT_KEYFORM, OPT_KEY,
    OPT_ISSUER, OPT_LASTUPDATE, OPT_NEXTUPDATE, OPT_FINGERPRINT,
    OPT_CRLNUMBER, OPT_BADSIG, OPT_GENDELTA, OPT_CAPATH, OPT_CAFILE, OPT_CASTORE,
    OPT_NOCAPATH, OPT_NOCAFILE, OPT_NOCASTORE, OPT_VERIFY, OPT_DATEOPT, OPT_TEXT, OPT_HASH,
    OPT_HASH_OLD, OPT_NOOUT, OPT_NAMEOPT, OPT_MD, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS crl_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"verify", OPT_VERIFY, '-', "Verify CRL signature"},

    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input file - default stdin"},
    {"inform", OPT_INFORM, 'F', "CRL input format (DER or PEM); has no effect"},
    {"key", OPT_KEY, '<', "CRL signing Private key to use"},
    {"keyform", OPT_KEYFORM, 'F', "Private key file format (DER/PEM/P12); has no effect"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "output file - default stdout"},
    {"outform", OPT_OUTFORM, 'F', "Output format - default PEM"},
    {"dateopt", OPT_DATEOPT, 's', "Datetime format used for printing. (rfc_822/iso_8601). Default is rfc_822."},
    {"text", OPT_TEXT, '-', "Print out a text format version"},
    {"hash", OPT_HASH, '-', "Print hash value"},
#ifndef OPENSSL_NO_MD5
    {"hash_old", OPT_HASH_OLD, '-', "Print old-style (MD5) hash value"},
#endif
    {"nameopt", OPT_NAMEOPT, 's', "Certificate subject/issuer name printing options"},
    {"", OPT_MD, '-', "Any supported digest"},

    OPT_SECTION("CRL"),
    {"issuer", OPT_ISSUER, '-', "Print issuer DN"},
    {"lastupdate", OPT_LASTUPDATE, '-', "Set lastUpdate field"},
    {"nextupdate", OPT_NEXTUPDATE, '-', "Set nextUpdate field"},
    {"noout", OPT_NOOUT, '-', "No CRL output"},
    {"fingerprint", OPT_FINGERPRINT, '-', "Print the crl fingerprint"},
    {"crlnumber", OPT_CRLNUMBER, '-', "Print CRL number"},
    {"badsig", OPT_BADSIG, '-', "Corrupt last byte of loaded CRL signature (for test)" },
    {"gendelta", OPT_GENDELTA, '<', "Other CRL to compare/diff to the Input one"},

    OPT_SECTION("Certificate"),
    {"CApath", OPT_CAPATH, '/', "Verify CRL using certificates in dir"},
    {"CAfile", OPT_CAFILE, '<', "Verify CRL using certificates in file name"},
    {"CAstore", OPT_CASTORE, ':', "Verify CRL using certificates in store URI"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load certificates from the default certificates directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load certificates from the default certificates store"},
    OPT_PROV_OPTIONS,
    {NULL}
};

int crl_main(int argc, char **argv)
{
    X509_CRL *x = NULL;
    BIO *out = NULL;
    X509_STORE *store = NULL;
    X509_STORE_CTX *ctx = NULL;
    X509_LOOKUP *lookup = NULL;
    X509_OBJECT *xobj = NULL;
    EVP_PKEY *pkey;
    EVP_MD *digest = (EVP_MD *)EVP_sha1();
    char *infile = NULL, *outfile = NULL, *crldiff = NULL, *keyfile = NULL;
    char *digestname = NULL;
    const char *CAfile = NULL, *CApath = NULL, *CAstore = NULL, *prog;
    OPTION_CHOICE o;
    int hash = 0, issuer = 0, lastupdate = 0, nextupdate = 0, noout = 0;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, keyformat = FORMAT_UNDEF;
    int ret = 1, num = 0, badsig = 0, fingerprint = 0, crlnumber = 0;
    int text = 0, do_ver = 0, noCAfile = 0, noCApath = 0, noCAstore = 0;
    unsigned long dateopt = ASN1_DTFLGS_RFC822;
    int i;
#ifndef OPENSSL_NO_MD5
    int hash_old = 0;
#endif

    opt_set_unknown_name("digest");
    prog = opt_init(argc, argv, crl_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(crl_options);
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
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyformat))
                goto opthelp;
            break;
        case OPT_KEY:
            keyfile = opt_arg();
            break;
        case OPT_GENDELTA:
            crldiff = opt_arg();
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            do_ver = 1;
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            do_ver = 1;
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            do_ver = 1;
            break;
        case OPT_NOCAPATH:
            noCApath =  1;
            break;
        case OPT_NOCAFILE:
            noCAfile =  1;
            break;
        case OPT_NOCASTORE:
            noCAstore =  1;
            break;
        case OPT_HASH_OLD:
#ifndef OPENSSL_NO_MD5
            hash_old = ++num;
#endif
            break;
        case OPT_VERIFY:
            do_ver = 1;
            break;
        case OPT_DATEOPT:
            if (!set_dateopt(&dateopt, opt_arg()))
                goto opthelp;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_HASH:
            hash = ++num;
            break;
        case OPT_ISSUER:
            issuer = ++num;
            break;
        case OPT_LASTUPDATE:
            lastupdate = ++num;
            break;
        case OPT_NEXTUPDATE:
            nextupdate = ++num;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_FINGERPRINT:
            fingerprint = ++num;
            break;
        case OPT_CRLNUMBER:
            crlnumber = ++num;
            break;
        case OPT_BADSIG:
            badsig = 1;
            break;
        case OPT_NAMEOPT:
            if (!set_nameopt(opt_arg()))
                goto opthelp;
            break;
        case OPT_MD:
            digestname = opt_unknown();
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        }
    }

    /* No remaining args. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!opt_md(digestname, &digest))
        goto opthelp;
    x = load_crl(infile, informat, 1, "CRL");
    if (x == NULL)
        goto end;

    if (do_ver) {
        if ((store = setup_verify(CAfile, noCAfile, CApath, noCApath,
                                  CAstore, noCAstore)) == NULL)
            goto end;
        lookup = X509_STORE_add_lookup(store, X509_LOOKUP_file());
        if (lookup == NULL)
            goto end;
        ctx = X509_STORE_CTX_new();
        if (ctx == NULL || !X509_STORE_CTX_init(ctx, store, NULL, NULL)) {
            BIO_printf(bio_err, "Error initialising X509 store\n");
            goto end;
        }

        xobj = X509_STORE_CTX_get_obj_by_subject(ctx, X509_LU_X509,
                                                 X509_CRL_get_issuer(x));
        if (xobj == NULL) {
            BIO_printf(bio_err, "Error getting CRL issuer certificate\n");
            goto end;
        }
        pkey = X509_get_pubkey(X509_OBJECT_get0_X509(xobj));
        X509_OBJECT_free(xobj);
        if (pkey == NULL) {
            BIO_printf(bio_err, "Error getting CRL issuer public key\n");
            goto end;
        }
        i = X509_CRL_verify(x, pkey);
        EVP_PKEY_free(pkey);
        if (i < 0)
            goto end;
        if (i == 0) {
            BIO_printf(bio_err, "verify failure\n");
	    goto end;
        } else
            BIO_printf(bio_err, "verify OK\n");
    }

    if (crldiff != NULL) {
        X509_CRL *newcrl, *delta;
        if (!keyfile) {
            BIO_puts(bio_err, "Missing CRL signing key\n");
            goto end;
        }
        newcrl = load_crl(crldiff, informat, 0, "other CRL");
        if (!newcrl)
            goto end;
        pkey = load_key(keyfile, keyformat, 0, NULL, NULL, "CRL signing key");
        if (pkey == NULL) {
            X509_CRL_free(newcrl);
            goto end;
        }
        delta = X509_CRL_diff(x, newcrl, pkey, digest, 0);
        X509_CRL_free(newcrl);
        EVP_PKEY_free(pkey);
        if (delta) {
            X509_CRL_free(x);
            x = delta;
        } else {
            BIO_puts(bio_err, "Error creating delta CRL\n");
            goto end;
        }
    }

    if (badsig) {
        const ASN1_BIT_STRING *sig;

        X509_CRL_get0_signature(x, &sig, NULL);
        corrupt_signature(sig);
    }

    if (num) {
        for (i = 1; i <= num; i++) {
            if (issuer == i) {
                print_name(bio_out, "issuer=", X509_CRL_get_issuer(x));
            }
            if (crlnumber == i) {
                ASN1_INTEGER *crlnum;

                crlnum = X509_CRL_get_ext_d2i(x, NID_crl_number, NULL, NULL);
                BIO_printf(bio_out, "crlNumber=");
                if (crlnum) {
                    BIO_puts(bio_out, "0x");
                    i2a_ASN1_INTEGER(bio_out, crlnum);
                    ASN1_INTEGER_free(crlnum);
                } else {
                    BIO_puts(bio_out, "<NONE>");
                }
                BIO_printf(bio_out, "\n");
            }
            if (hash == i) {
                int ok;
                unsigned long hash_value =
                    X509_NAME_hash_ex(X509_CRL_get_issuer(x), app_get0_libctx(),
                                      app_get0_propq(), &ok);

                if (num > 1)
                    BIO_printf(bio_out, "issuer name hash=");
                if (ok) {
                    BIO_printf(bio_out, "%08lx\n", hash_value);
                } else {
                    BIO_puts(bio_out, "<ERROR>");
                    goto end;
                }
            }
#ifndef OPENSSL_NO_MD5
            if (hash_old == i) {
                if (num > 1)
                    BIO_printf(bio_out, "issuer name old hash=");
                BIO_printf(bio_out, "%08lx\n",
                           X509_NAME_hash_old(X509_CRL_get_issuer(x)));
            }
#endif
            if (lastupdate == i) {
                BIO_printf(bio_out, "lastUpdate=");
                ASN1_TIME_print_ex(bio_out, X509_CRL_get0_lastUpdate(x), dateopt);
                BIO_printf(bio_out, "\n");
            }
            if (nextupdate == i) {
                BIO_printf(bio_out, "nextUpdate=");
                if (X509_CRL_get0_nextUpdate(x))
                    ASN1_TIME_print_ex(bio_out, X509_CRL_get0_nextUpdate(x), dateopt);
                else
                    BIO_printf(bio_out, "NONE");
                BIO_printf(bio_out, "\n");
            }
            if (fingerprint == i) {
                int j;
                unsigned int n;
                unsigned char md[EVP_MAX_MD_SIZE];

                if (!X509_CRL_digest(x, digest, md, &n)) {
                    BIO_printf(bio_err, "out of memory\n");
                    goto end;
                }
                BIO_printf(bio_out, "%s Fingerprint=",
                           EVP_MD_get0_name(digest));
                for (j = 0; j < (int)n; j++) {
                    BIO_printf(bio_out, "%02X%c", md[j], (j + 1 == (int)n)
                               ? '\n' : ':');
                }
            }
        }
    }
    out = bio_open_default(outfile, 'w', outformat);
    if (out == NULL)
        goto end;

    if (text)
        X509_CRL_print_ex(out, x, get_nameopt());

    if (noout) {
        ret = 0;
        goto end;
    }

    if (outformat == FORMAT_ASN1)
        i = (int)i2d_X509_CRL_bio(out, x);
    else
        i = PEM_write_bio_X509_CRL(out, x);
    if (!i) {
        BIO_printf(bio_err, "unable to write CRL\n");
        goto end;
    }
    ret = 0;

 end:
    if (ret != 0)
        ERR_print_errors(bio_err);
    BIO_free_all(out);
    EVP_MD_free(digest);
    X509_CRL_free(x);
    X509_STORE_CTX_free(ctx);
    X509_STORE_free(store);
    return ret;
}
