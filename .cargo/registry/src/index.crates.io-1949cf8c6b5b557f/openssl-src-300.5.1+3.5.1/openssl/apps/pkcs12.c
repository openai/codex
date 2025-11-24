/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/asn1.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/pkcs12.h>
#include <openssl/provider.h>
#include <openssl/kdf.h>
#include <openssl/rand.h>

#define NOKEYS          0x1
#define NOCERTS         0x2
#define INFO            0x4
#define CLCERTS         0x8
#define CACERTS         0x10

#define PASSWD_BUF_SIZE 2048

#define WARN_EXPORT(opt) \
    BIO_printf(bio_err, "Warning: -%s option ignored with -export\n", opt);
#define WARN_NO_EXPORT(opt) \
    BIO_printf(bio_err, "Warning: -%s option ignored without -export\n", opt);

static int get_cert_chain(X509 *cert, X509_STORE *store,
                          STACK_OF(X509) *untrusted_certs,
                          STACK_OF(X509) **chain);
int dump_certs_keys_p12(BIO *out, const PKCS12 *p12,
                        const char *pass, int passlen, int options,
                        char *pempass, const EVP_CIPHER *enc);
int dump_certs_pkeys_bags(BIO *out, const STACK_OF(PKCS12_SAFEBAG) *bags,
                          const char *pass, int passlen, int options,
                          char *pempass, const EVP_CIPHER *enc);
int dump_certs_pkeys_bag(BIO *out, const PKCS12_SAFEBAG *bags,
                         const char *pass, int passlen,
                         int options, char *pempass, const EVP_CIPHER *enc);
void print_attribute(BIO *out, const ASN1_TYPE *av);
int print_attribs(BIO *out, const STACK_OF(X509_ATTRIBUTE) *attrlst,
                  const char *name);
void hex_prin(BIO *out, unsigned char *buf, int len);
static int alg_print(const X509_ALGOR *alg);
int cert_load(BIO *in, STACK_OF(X509) *sk);
static int set_pbe(int *ppbe, const char *str);
static int jdk_trust(PKCS12_SAFEBAG *bag, void *cbarg);

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_CIPHER, OPT_NOKEYS, OPT_KEYEX, OPT_KEYSIG, OPT_NOCERTS, OPT_CLCERTS,
    OPT_CACERTS, OPT_NOOUT, OPT_INFO, OPT_CHAIN, OPT_TWOPASS, OPT_NOMACVER,
#ifndef OPENSSL_NO_DES
    OPT_DESCERT,
#endif
    OPT_EXPORT, OPT_ITER, OPT_NOITER, OPT_MACITER, OPT_NOMACITER, OPT_MACSALTLEN,
    OPT_NOMAC, OPT_LMK, OPT_NODES, OPT_NOENC, OPT_MACALG, OPT_CERTPBE, OPT_KEYPBE,
    OPT_INKEY, OPT_CERTFILE, OPT_UNTRUSTED, OPT_PASSCERTS,
    OPT_NAME, OPT_CSP, OPT_CANAME,
    OPT_IN, OPT_OUT, OPT_PASSIN, OPT_PASSOUT, OPT_PASSWORD, OPT_CAPATH,
    OPT_CAFILE, OPT_CASTORE, OPT_NOCAPATH, OPT_NOCAFILE, OPT_NOCASTORE, OPT_ENGINE,
    OPT_R_ENUM, OPT_PROV_ENUM, OPT_JDKTRUST, OPT_PBMAC1_PBKDF2, OPT_PBMAC1_PBKDF2_MD,
#ifndef OPENSSL_NO_DES
    OPT_LEGACY_ALG
#endif
} OPTION_CHOICE;

const OPTIONS pkcs12_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"in", OPT_IN, '<', "Input file"},
    {"out", OPT_OUT, '>', "Output file"},
    {"passin", OPT_PASSIN, 's', "Input file pass phrase source"},
    {"passout", OPT_PASSOUT, 's', "Output file pass phrase source"},
    {"password", OPT_PASSWORD, 's', "Set PKCS#12 import/export password source"},
    {"twopass", OPT_TWOPASS, '-', "Separate MAC, encryption passwords"},
    {"nokeys", OPT_NOKEYS, '-', "Don't output private keys"},
    {"nocerts", OPT_NOCERTS, '-', "Don't output certificates"},
    {"noout", OPT_NOOUT, '-', "Don't output anything, just verify PKCS#12 input"},
#ifndef OPENSSL_NO_DES
    {"legacy", OPT_LEGACY_ALG, '-',
# ifdef OPENSSL_NO_RC2
     "Use legacy encryption algorithm 3DES_CBC for keys and certs"
# else
     "Use legacy encryption: 3DES_CBC for keys, RC2_CBC for certs"
# endif
    },
#endif
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    OPT_PROV_OPTIONS,
    OPT_R_OPTIONS,

    OPT_SECTION("PKCS#12 import (parsing PKCS#12)"),
    {"info", OPT_INFO, '-', "Print info about PKCS#12 structure"},
    {"nomacver", OPT_NOMACVER, '-', "Don't verify integrity MAC"},
    {"clcerts", OPT_CLCERTS, '-', "Only output client certificates"},
    {"cacerts", OPT_CACERTS, '-', "Only output CA certificates"},
    {"", OPT_CIPHER, '-', "Any supported cipher for output encryption"},
    {"noenc", OPT_NOENC, '-', "Don't encrypt private keys"},
    {"nodes", OPT_NODES, '-', "Don't encrypt private keys; deprecated"},

    OPT_SECTION("PKCS#12 output (export)"),
    {"export", OPT_EXPORT, '-', "Create PKCS12 file"},
    {"inkey", OPT_INKEY, 's', "Private key, else read from -in input file"},
    {"certfile", OPT_CERTFILE, '<', "Extra certificates for PKCS12 output"},
    {"passcerts", OPT_PASSCERTS, 's', "Certificate file pass phrase source"},
    {"chain", OPT_CHAIN, '-', "Build and add certificate chain for EE cert,"},
    {OPT_MORE_STR, 0, 0,
     "which is the 1st cert from -in matching the private key (if given)"},
    {"untrusted", OPT_UNTRUSTED, '<', "Untrusted certificates for chain building"},
    {"CAfile", OPT_CAFILE, '<', "PEM-format file of CA's"},
    {"CApath", OPT_CAPATH, '/', "PEM-format directory of CA's"},
    {"CAstore", OPT_CASTORE, ':', "URI to store of CA's"},
    {"no-CAfile", OPT_NOCAFILE, '-',
     "Do not load the default certificates file"},
    {"no-CApath", OPT_NOCAPATH, '-',
     "Do not load certificates from the default certificates directory"},
    {"no-CAstore", OPT_NOCASTORE, '-',
     "Do not load certificates from the default certificates store"},
    {"name", OPT_NAME, 's', "Use name as friendly name"},
    {"caname", OPT_CANAME, 's',
     "Use name as CA friendly name (can be repeated)"},
    {"CSP", OPT_CSP, 's', "Microsoft CSP name"},
    {"LMK", OPT_LMK, '-',
     "Add local machine keyset attribute to private key"},
    {"keyex", OPT_KEYEX, '-', "Set key type to MS key exchange"},
    {"keysig", OPT_KEYSIG, '-', "Set key type to MS key signature"},
    {"keypbe", OPT_KEYPBE, 's', "Private key PBE algorithm (default AES-256 CBC)"},
    {"certpbe", OPT_CERTPBE, 's',
     "Certificate PBE algorithm (default PBES2 with PBKDF2 and AES-256 CBC)"},
#ifndef OPENSSL_NO_DES
    {"descert", OPT_DESCERT, '-',
     "Encrypt output with 3DES (default PBES2 with PBKDF2 and AES-256 CBC)"},
#endif
    {"macalg", OPT_MACALG, 's',
     "Digest algorithm to use in MAC (default SHA256)"},
    {"pbmac1_pbkdf2", OPT_PBMAC1_PBKDF2, '-', "Use PBMAC1 with PBKDF2 instead of MAC"},
    {"pbmac1_pbkdf2_md", OPT_PBMAC1_PBKDF2_MD, 's', "Digest to use for PBMAC1 KDF (default SHA256)"},
    {"iter", OPT_ITER, 'p', "Specify the iteration count for encryption and MAC"},
    {"noiter", OPT_NOITER, '-', "Don't use encryption iteration"},
    {"nomaciter", OPT_NOMACITER, '-', "Don't use MAC iteration)"},
    {"maciter", OPT_MACITER, '-', "Unused, kept for backwards compatibility"},
    {"macsaltlen", OPT_MACSALTLEN, 'p', "Specify the salt len for MAC"},
    {"nomac", OPT_NOMAC, '-', "Don't generate MAC"},
    {"jdktrust", OPT_JDKTRUST, 's', "Mark certificate in PKCS#12 store as trusted for JDK compatibility"},
    {NULL}
};

int pkcs12_main(int argc, char **argv)
{
    char *infile = NULL, *outfile = NULL, *keyname = NULL, *certfile = NULL;
    char *untrusted = NULL, *ciphername = NULL, *enc_name = NULL;
    char *passcertsarg = NULL, *passcerts = NULL;
    char *name = NULL, *csp_name = NULL;
    char pass[PASSWD_BUF_SIZE] = "", macpass[PASSWD_BUF_SIZE] = "";
    int export_pkcs12 = 0, options = 0, chain = 0, twopass = 0, keytype = 0;
    char *jdktrust = NULL;
#ifndef OPENSSL_NO_DES
    int use_legacy = 0;
#endif
    /* use library defaults for the iter, maciter, cert, and key PBE */
    int iter = 0, maciter = 0, pbmac1_pbkdf2 = 0;
    int macsaltlen = PKCS12_SALT_LEN;
    int cert_pbe = NID_undef;
    int key_pbe = NID_undef;
    int ret = 1, macver = 1, add_lmk = 0, private = 0;
    int noprompt = 0;
    char *passinarg = NULL, *passoutarg = NULL, *passarg = NULL;
    char *passin = NULL, *passout = NULL, *macalg = NULL, *pbmac1_pbkdf2_md = NULL;
    char *cpass = NULL, *mpass = NULL, *badpass = NULL;
    const char *CApath = NULL, *CAfile = NULL, *CAstore = NULL, *prog;
    int noCApath = 0, noCAfile = 0, noCAstore = 0;
    ENGINE *e = NULL;
    BIO *in = NULL, *out = NULL;
    PKCS12 *p12 = NULL;
    STACK_OF(OPENSSL_STRING) *canames = NULL;
    EVP_CIPHER *default_enc = (EVP_CIPHER *)EVP_aes_256_cbc();
    EVP_CIPHER *enc = (EVP_CIPHER *)default_enc;
    OPTION_CHOICE o;

    opt_set_unknown_name("cipher");
    prog = opt_init(argc, argv, pkcs12_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(pkcs12_options);
            ret = 0;
            goto end;
        case OPT_NOKEYS:
            options |= NOKEYS;
            break;
        case OPT_KEYEX:
            keytype = KEY_EX;
            break;
        case OPT_KEYSIG:
            keytype = KEY_SIG;
            break;
        case OPT_NOCERTS:
            options |= NOCERTS;
            break;
        case OPT_CLCERTS:
            options |= CLCERTS;
            break;
        case OPT_CACERTS:
            options |= CACERTS;
            break;
        case OPT_NOOUT:
            options |= (NOKEYS | NOCERTS);
            break;
        case OPT_JDKTRUST:
            jdktrust = opt_arg();
            /* Adding jdk trust implies nokeys */
            options |= NOKEYS;
            break;
        case OPT_INFO:
            options |= INFO;
            break;
        case OPT_CHAIN:
            chain = 1;
            break;
        case OPT_TWOPASS:
            twopass = 1;
            break;
        case OPT_NOMACVER:
            macver = 0;
            break;
#ifndef OPENSSL_NO_DES
        case OPT_DESCERT:
            cert_pbe = NID_pbe_WithSHA1And3_Key_TripleDES_CBC;
            break;
#endif
        case OPT_EXPORT:
            export_pkcs12 = 1;
            break;
        case OPT_NODES:
        case OPT_NOENC:
            /*
             * |enc_name| stores the name of the option used so it
             * can be printed if an error message is output.
             */
            enc_name = opt_flag() + 1;
            enc = NULL;
            ciphername = NULL;
            break;
        case OPT_CIPHER:
            enc_name = ciphername = opt_unknown();
            break;
        case OPT_ITER:
            maciter = iter = opt_int_arg();
            break;
        case OPT_NOITER:
            iter = 1;
            break;
        case OPT_MACITER:
            /* no-op */
            break;
        case OPT_NOMACITER:
            maciter = 1;
            break;
        case OPT_MACSALTLEN:
            macsaltlen = opt_int_arg();
            break;
        case OPT_NOMAC:
            cert_pbe = -1;
            maciter = -1;
            break;
        case OPT_MACALG:
            macalg = opt_arg();
            break;
        case OPT_PBMAC1_PBKDF2:
            pbmac1_pbkdf2 = 1;
            break;
        case OPT_PBMAC1_PBKDF2_MD:
            pbmac1_pbkdf2_md = opt_arg();
            break;
        case OPT_CERTPBE:
            if (!set_pbe(&cert_pbe, opt_arg()))
                goto opthelp;
            break;
        case OPT_KEYPBE:
            if (!set_pbe(&key_pbe, opt_arg()))
                goto opthelp;
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_INKEY:
            keyname = opt_arg();
            break;
        case OPT_CERTFILE:
            certfile = opt_arg();
            break;
        case OPT_UNTRUSTED:
            untrusted = opt_arg();
            break;
        case OPT_PASSCERTS:
            passcertsarg = opt_arg();
            break;
        case OPT_NAME:
            name = opt_arg();
            break;
        case OPT_LMK:
            add_lmk = 1;
            break;
        case OPT_CSP:
            csp_name = opt_arg();
            break;
        case OPT_CANAME:
            if (canames == NULL
                && (canames = sk_OPENSSL_STRING_new_null()) == NULL)
                goto end;
            if (sk_OPENSSL_STRING_push(canames, opt_arg()) <= 0)
                goto end;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_PASSOUT:
            passoutarg = opt_arg();
            break;
        case OPT_PASSWORD:
            passarg = opt_arg();
            break;
        case OPT_CAPATH:
            CApath = opt_arg();
            break;
        case OPT_CASTORE:
            CAstore = opt_arg();
            break;
        case OPT_CAFILE:
            CAfile = opt_arg();
            break;
        case OPT_NOCAPATH:
            noCApath = 1;
            break;
        case OPT_NOCASTORE:
            noCAstore = 1;
            break;
        case OPT_NOCAFILE:
            noCAfile = 1;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
#ifndef OPENSSL_NO_DES
        case OPT_LEGACY_ALG:
            use_legacy = 1;
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

    if (!app_RAND_load())
        goto end;

    if (!opt_cipher_any(ciphername, &enc))
        goto opthelp;
    if (export_pkcs12) {
        if ((options & INFO) != 0)
            WARN_EXPORT("info");
        if (macver == 0)
            WARN_EXPORT("nomacver");
        if ((options & CLCERTS) != 0)
            WARN_EXPORT("clcerts");
        if ((options & CACERTS) != 0)
            WARN_EXPORT("cacerts");
        if (enc != default_enc)
            BIO_printf(bio_err,
                       "Warning: output encryption option -%s ignored with -export\n", enc_name);
    } else {
        if (keyname != NULL)
            WARN_NO_EXPORT("inkey");
        if (certfile != NULL)
            WARN_NO_EXPORT("certfile");
        if (passcertsarg != NULL)
            WARN_NO_EXPORT("passcerts");
        if (chain)
            WARN_NO_EXPORT("chain");
        if (untrusted != NULL)
            WARN_NO_EXPORT("untrusted");
        if (CAfile != NULL)
            WARN_NO_EXPORT("CAfile");
        if (CApath != NULL)
            WARN_NO_EXPORT("CApath");
        if (CAstore != NULL)
            WARN_NO_EXPORT("CAstore");
        if (noCAfile)
            WARN_NO_EXPORT("no-CAfile");
        if (noCApath)
            WARN_NO_EXPORT("no-CApath");
        if (noCAstore)
            WARN_NO_EXPORT("no-CAstore");
        if (name != NULL)
            WARN_NO_EXPORT("name");
        if (canames != NULL)
            WARN_NO_EXPORT("caname");
        if (csp_name != NULL)
            WARN_NO_EXPORT("CSP");
        if (add_lmk)
            WARN_NO_EXPORT("LMK");
        if (keytype == KEY_EX)
            WARN_NO_EXPORT("keyex");
        if (keytype == KEY_SIG)
            WARN_NO_EXPORT("keysig");
        if (key_pbe != NID_undef)
            WARN_NO_EXPORT("keypbe");
        if (cert_pbe != NID_undef && cert_pbe != -1)
            WARN_NO_EXPORT("certpbe and -descert");
        if (macalg != NULL)
            WARN_NO_EXPORT("macalg");
        if (iter != 0)
            WARN_NO_EXPORT("iter and -noiter");
        if (maciter == 1)
            WARN_NO_EXPORT("nomaciter");
        if (cert_pbe == -1 && maciter == -1)
            WARN_NO_EXPORT("nomac");
        if (macsaltlen != PKCS12_SALT_LEN)
            WARN_NO_EXPORT("macsaltlen");
    }
#ifndef OPENSSL_NO_DES
    if (use_legacy) {
        /* load the legacy provider if not loaded already*/
        if (!OSSL_PROVIDER_available(app_get0_libctx(), "legacy")) {
            if (!app_provider_load(app_get0_libctx(), "legacy"))
                goto end;
            /* load the default provider explicitly */
            if (!app_provider_load(app_get0_libctx(), "default"))
                goto end;
        }
        if (cert_pbe == NID_undef) {
            /* Adapt default algorithm */
# ifndef OPENSSL_NO_RC2
            cert_pbe = NID_pbe_WithSHA1And40BitRC2_CBC;
# else
            cert_pbe = NID_pbe_WithSHA1And3_Key_TripleDES_CBC;
# endif
        }

        if (key_pbe == NID_undef)
            key_pbe = NID_pbe_WithSHA1And3_Key_TripleDES_CBC;
        if (enc == default_enc)
            enc = (EVP_CIPHER *)EVP_des_ede3_cbc();
        if (macalg == NULL)
            macalg = "sha1";
    }
#endif

    private = 1;

    if (!app_passwd(passcertsarg, NULL, &passcerts, NULL)) {
        BIO_printf(bio_err, "Error getting certificate file password\n");
        goto end;
    }

    if (passarg != NULL) {
        if (export_pkcs12)
            passoutarg = passarg;
        else
            passinarg = passarg;
    }

    if (!app_passwd(passinarg, passoutarg, &passin, &passout)) {
        BIO_printf(bio_err, "Error getting passwords\n");
        goto end;
    }

    if (cpass == NULL) {
        if (export_pkcs12)
            cpass = passout;
        else
            cpass = passin;
    }

    if (cpass != NULL) {
        mpass = cpass;
        noprompt = 1;
        if (twopass) {
            if (export_pkcs12)
                BIO_printf(bio_err, "Option -twopass cannot be used with -passout or -password\n");
            else
                BIO_printf(bio_err, "Option -twopass cannot be used with -passin or -password\n");
            goto end;
        }
    } else {
        cpass = pass;
        mpass = macpass;
    }

    if (twopass) {
        /* To avoid bit rot */
        if (1) {
#ifndef OPENSSL_NO_UI_CONSOLE
            if (EVP_read_pw_string(
                macpass, sizeof(macpass), "Enter MAC Password:", export_pkcs12)) {
                BIO_printf(bio_err, "Can't read Password\n");
                goto end;
            }
        } else {
#endif
            BIO_printf(bio_err, "Unsupported option -twopass\n");
            goto end;
        }
    }

    if (export_pkcs12) {
        EVP_PKEY *key = NULL;
        X509 *ee_cert = NULL, *x = NULL;
        STACK_OF(X509) *certs = NULL;
        STACK_OF(X509) *untrusted_certs = NULL;
        EVP_MD *macmd = NULL;
        unsigned char *catmp = NULL;
        int i;
        ASN1_OBJECT *obj = NULL;

        if ((options & (NOCERTS | NOKEYS)) == (NOCERTS | NOKEYS)) {
            BIO_printf(bio_err, "Nothing to export due to -noout or -nocerts and -nokeys\n");
            goto export_end;
        }

        if ((options & NOCERTS) != 0) {
            chain = 0;
            BIO_printf(bio_err, "Warning: -chain option ignored with -nocerts\n");
        }

        if (!(options & NOKEYS)) {
            key = load_key(keyname ? keyname : infile,
                           FORMAT_PEM, 1, passin, e,
                           keyname ?
                           "private key from -inkey file" :
                           "private key from -in file");
            if (key == NULL)
                goto export_end;
        }

        /* Load all certs in input file */
        if (!(options & NOCERTS)) {
            if (!load_certs(infile, 1, &certs, passin,
                            "certificates from -in file"))
                goto export_end;
            if (sk_X509_num(certs) < 1) {
                BIO_printf(bio_err, "No certificate in -in file %s\n", infile);
                goto export_end;
            }

            if (key != NULL) {
                /* Look for matching private key */
                for (i = 0; i < sk_X509_num(certs); i++) {
                    x = sk_X509_value(certs, i);
                    if (cert_matches_key(x, key)) {
                        ee_cert = x;
                        /* Zero keyid and alias */
                        X509_keyid_set1(ee_cert, NULL, 0);
                        X509_alias_set1(ee_cert, NULL, 0);
                        /* Remove from list */
                        (void)sk_X509_delete(certs, i);
                        break;
                    }
                }
                if (ee_cert == NULL) {
                    BIO_printf(bio_err,
                               "No cert in -in file '%s' matches private key\n",
                               infile);
                    goto export_end;
                }
            }
        }

        /* Load any untrusted certificates for chain building */
        if (untrusted != NULL) {
            if (!load_certs(untrusted, 0, &untrusted_certs, passcerts,
                            "untrusted certificates"))
                goto export_end;
        }

        /* If chaining get chain from end entity cert */
        if (chain) {
            int vret;
            STACK_OF(X509) *chain2;
            X509_STORE *store;
            X509 *ee_cert_tmp = ee_cert;

            /* Assume the first cert if we haven't got anything else */
            if (ee_cert_tmp == NULL && certs != NULL)
                ee_cert_tmp = sk_X509_value(certs, 0);

            if (ee_cert_tmp == NULL) {
                BIO_printf(bio_err,
                           "No end entity certificate to check with -chain\n");
                goto export_end;
            }

            if ((store = setup_verify(CAfile, noCAfile, CApath, noCApath,
                                      CAstore, noCAstore))
                    == NULL)
                goto export_end;

            vret = get_cert_chain(ee_cert_tmp, store, untrusted_certs, &chain2);
            X509_STORE_free(store);

            if (vret == X509_V_OK) {
                int add_certs;
                /* Remove from chain2 the first (end entity) certificate */
                X509_free(sk_X509_shift(chain2));
                /* Add the remaining certs (except for duplicates) */
                add_certs = X509_add_certs(certs, chain2, X509_ADD_FLAG_UP_REF
                                           | X509_ADD_FLAG_NO_DUP);
                OSSL_STACK_OF_X509_free(chain2);
                if (!add_certs)
                    goto export_end;
            } else {
                if (vret != X509_V_ERR_UNSPECIFIED)
                    BIO_printf(bio_err, "Error getting chain: %s\n",
                               X509_verify_cert_error_string(vret));
                goto export_end;
            }
        }

        /* Add any extra certificates asked for */
        if (certfile != NULL) {
            if (!load_certs(certfile, 0, &certs, passcerts,
                            "extra certificates from -certfile"))
                goto export_end;
        }

        /* Add any CA names */
        for (i = 0; i < sk_OPENSSL_STRING_num(canames); i++) {
            catmp = (unsigned char *)sk_OPENSSL_STRING_value(canames, i);
            X509_alias_set1(sk_X509_value(certs, i), catmp, -1);
        }

        if (csp_name != NULL && key != NULL)
            EVP_PKEY_add1_attr_by_NID(key, NID_ms_csp_name,
                                      MBSTRING_ASC, (unsigned char *)csp_name,
                                      -1);

        if (add_lmk && key != NULL)
            EVP_PKEY_add1_attr_by_NID(key, NID_LocalKeySet, 0, NULL, -1);

        if (!noprompt && !(enc == NULL && maciter == -1)) {
            /* To avoid bit rot */
            if (1) {
#ifndef OPENSSL_NO_UI_CONSOLE
                if (EVP_read_pw_string(pass, sizeof(pass),
                                       "Enter Export Password:", 1)) {
                    BIO_printf(bio_err, "Can't read Password\n");
                    goto export_end;
                }
            } else {
#endif
                BIO_printf(bio_err, "Password required\n");
                goto export_end;
            }
        }

        if (!twopass)
            OPENSSL_strlcpy(macpass, pass, sizeof(macpass));

        if (jdktrust != NULL) {
            obj = OBJ_txt2obj(jdktrust, 0);
        }

        p12 = PKCS12_create_ex2(cpass, name, key, ee_cert, certs,
                                key_pbe, cert_pbe, iter, -1, keytype,
                                app_get0_libctx(), app_get0_propq(),
                                jdk_trust, (void*)obj);

        if (p12 == NULL) {
            BIO_printf(bio_err, "Error creating PKCS12 structure for %s\n",
                       outfile);
            goto export_end;
        }

        if (macalg != NULL) {
            if (!opt_md(macalg, &macmd))
                goto opthelp;
        }

        if (maciter != -1) {
            if (pbmac1_pbkdf2 == 1) {
                if (!PKCS12_set_pbmac1_pbkdf2(p12, mpass, -1, NULL,
                                              macsaltlen, maciter,
                                              macmd, pbmac1_pbkdf2_md)) {
                    BIO_printf(bio_err, "Error creating PBMAC1\n");
                    goto export_end;
                }
            } else {
                if (!PKCS12_set_mac(p12, mpass, -1, NULL, macsaltlen, maciter, macmd)) {
                    BIO_printf(bio_err, "Error creating PKCS12 MAC; no PKCS12KDF support?\n");
                    BIO_printf(bio_err,
                               "Use -nomac or -pbmac1_pbkdf2 if PKCS12KDF support not available\n");
                    goto export_end;
                }
            }
        }
        assert(private);

        out = bio_open_owner(outfile, FORMAT_PKCS12, private);
        if (out == NULL)
            goto end;

        i2d_PKCS12_bio(out, p12);

        ret = 0;

 export_end:

        EVP_PKEY_free(key);
        EVP_MD_free(macmd);
        OSSL_STACK_OF_X509_free(certs);
        OSSL_STACK_OF_X509_free(untrusted_certs);
        X509_free(ee_cert);
        ASN1_OBJECT_free(obj);
        ERR_print_errors(bio_err);
        goto end;

    }

    in = bio_open_default(infile, 'r', FORMAT_PKCS12);
    if (in == NULL)
        goto end;

    p12 = PKCS12_init_ex(NID_pkcs7_data, app_get0_libctx(), app_get0_propq());
    if (p12 == NULL) {
        ERR_print_errors(bio_err);
        goto end;
    }
    if ((p12 = d2i_PKCS12_bio(in, &p12)) == NULL) {
        ERR_print_errors(bio_err);
        goto end;
    }

    if (!noprompt) {
        if (1) {
#ifndef OPENSSL_NO_UI_CONSOLE
            if (EVP_read_pw_string(pass, sizeof(pass), "Enter Import Password:",
                                   0)) {
                BIO_printf(bio_err, "Can't read Password\n");
                goto end;
            }
        } else {
#endif
            BIO_printf(bio_err, "Password required\n");
            goto end;
        }
    }

    if (!twopass)
        OPENSSL_strlcpy(macpass, pass, sizeof(macpass));

    if ((options & INFO) && PKCS12_mac_present(p12)) {
        const ASN1_INTEGER *tmaciter;
        const X509_ALGOR *macalgid;
        const ASN1_OBJECT *macobj;
        const ASN1_OCTET_STRING *tmac;
        const ASN1_OCTET_STRING *tsalt;

        PKCS12_get0_mac(&tmac, &macalgid, &tsalt, &tmaciter, p12);
        /* current hash algorithms do not use parameters so extract just name,
           in future alg_print() may be needed */
        X509_ALGOR_get0(&macobj, NULL, NULL, macalgid);
        BIO_puts(bio_err, "MAC: ");
        i2a_ASN1_OBJECT(bio_err, macobj);
        if (OBJ_obj2nid(macobj) == NID_pbmac1) {
            PBKDF2PARAM *pbkdf2_param = PBMAC1_get1_pbkdf2_param(macalgid);

            if (pbkdf2_param == NULL) {
                BIO_printf(bio_err, ", Unsupported KDF or params for PBMAC1\n");
            } else {
                const ASN1_OBJECT *prfobj;
                int prfnid;

                BIO_printf(bio_err, " using PBKDF2, Iteration %ld\n",
                           ASN1_INTEGER_get(pbkdf2_param->iter));
                BIO_printf(bio_err, "Key length: %ld, Salt length: %d\n",
                           ASN1_INTEGER_get(pbkdf2_param->keylength),
                           ASN1_STRING_length(pbkdf2_param->salt->value.octet_string));
                if (pbkdf2_param->prf == NULL) {
                    prfnid = NID_hmacWithSHA1;
                } else {
                    X509_ALGOR_get0(&prfobj, NULL, NULL, pbkdf2_param->prf);
                    prfnid = OBJ_obj2nid(prfobj);
                }
                BIO_printf(bio_err, "PBKDF2 PRF: %s\n", OBJ_nid2sn(prfnid));
            }
            PBKDF2PARAM_free(pbkdf2_param);
        } else {
            BIO_printf(bio_err, ", Iteration %ld\n",
                       tmaciter != NULL ? ASN1_INTEGER_get(tmaciter) : 1L);
            BIO_printf(bio_err, "MAC length: %ld, salt length: %ld\n",
                       tmac != NULL ? ASN1_STRING_length(tmac) : 0L,
                       tsalt != NULL ? ASN1_STRING_length(tsalt) : 0L);
        }
    }

    if (macver) {
        const X509_ALGOR *macalgid;
        const ASN1_OBJECT *macobj;

        PKCS12_get0_mac(NULL, &macalgid, NULL, NULL, p12);

        if (macalgid == NULL) {
            BIO_printf(bio_err, "Warning: MAC is absent!\n");
            goto dump;
        }

        X509_ALGOR_get0(&macobj, NULL, NULL, macalgid);

        if (OBJ_obj2nid(macobj) != NID_pbmac1) {
            EVP_KDF *pkcs12kdf;

            pkcs12kdf = EVP_KDF_fetch(app_get0_libctx(), "PKCS12KDF",
                                      app_get0_propq());
            if (pkcs12kdf == NULL) {
                BIO_printf(bio_err, "Error verifying PKCS12 MAC; no PKCS12KDF support.\n");
                BIO_printf(bio_err, "Use -nomacver if MAC verification is not required.\n");
                goto end;
            }
            EVP_KDF_free(pkcs12kdf);
        }

        /* If we enter empty password try no password first */
        if (!mpass[0] && PKCS12_verify_mac(p12, NULL, 0)) {
            /* If mac and crypto pass the same set it to NULL too */
            if (!twopass)
                cpass = NULL;
        } else if (!PKCS12_verify_mac(p12, mpass, -1)) {
            /*
             * May be UTF8 from previous version of OpenSSL:
             * convert to a UTF8 form which will translate
             * to the same Unicode password.
             */
            unsigned char *utmp;
            int utmplen;
            unsigned long err = ERR_peek_error();

            if (ERR_GET_LIB(err) == ERR_LIB_PKCS12
                && ERR_GET_REASON(err) == PKCS12_R_MAC_ABSENT) {
                BIO_printf(bio_err, "Warning: MAC is absent!\n");
                goto dump;
            }

            utmp = OPENSSL_asc2uni(mpass, -1, NULL, &utmplen);
            if (utmp == NULL)
                goto end;
            badpass = OPENSSL_uni2utf8(utmp, utmplen);
            OPENSSL_free(utmp);
            if (!PKCS12_verify_mac(p12, badpass, -1)) {
                BIO_printf(bio_err, "Mac verify error: invalid password?\n");
                ERR_print_errors(bio_err);
                goto end;
            } else {
                BIO_printf(bio_err, "Warning: using broken algorithm\n");
                if (!twopass)
                    cpass = badpass;
            }
        }
    }

 dump:
    assert(private);

    out = bio_open_owner(outfile, FORMAT_PEM, private);
    if (out == NULL)
        goto end;

    if (!dump_certs_keys_p12(out, p12, cpass, -1, options, passout, enc)) {
        BIO_printf(bio_err, "Error outputting keys and certificates\n");
        ERR_print_errors(bio_err);
        goto end;
    }
    ret = 0;
 end:
    PKCS12_free(p12);
    release_engine(e);
    BIO_free(in);
    BIO_free_all(out);
    sk_OPENSSL_STRING_free(canames);
    OPENSSL_free(badpass);
    OPENSSL_free(passcerts);
    OPENSSL_free(passin);
    OPENSSL_free(passout);
    return ret;
}

static int jdk_trust(PKCS12_SAFEBAG *bag, void *cbarg)
{
    STACK_OF(X509_ATTRIBUTE) *attrs = NULL;
    X509_ATTRIBUTE *attr = NULL;

    /* Nothing to do */
    if (cbarg == NULL)
        return 1;

    /* Get the current attrs */
    attrs = (STACK_OF(X509_ATTRIBUTE)*)PKCS12_SAFEBAG_get0_attrs(bag);

    /* Create a new attr for the JDK Trusted Usage and add it */
    attr = X509_ATTRIBUTE_create(NID_oracle_jdk_trustedkeyusage, V_ASN1_OBJECT, (ASN1_OBJECT*)cbarg);

    /* Add the new attr, if attrs is NULL, it'll be initialised */
    X509at_add1_attr(&attrs, attr);

    /* Set the bag attrs */
    PKCS12_SAFEBAG_set0_attrs(bag, attrs);

    X509_ATTRIBUTE_free(attr);
    return 1;
}

int dump_certs_keys_p12(BIO *out, const PKCS12 *p12, const char *pass,
                        int passlen, int options, char *pempass,
                        const EVP_CIPHER *enc)
{
    STACK_OF(PKCS7) *asafes = NULL;
    int i, bagnid;
    int ret = 0;
    PKCS7 *p7;

    if ((asafes = PKCS12_unpack_authsafes(p12)) == NULL)
        return 0;
    for (i = 0; i < sk_PKCS7_num(asafes); i++) {
        STACK_OF(PKCS12_SAFEBAG) *bags;

        p7 = sk_PKCS7_value(asafes, i);
        bagnid = OBJ_obj2nid(p7->type);
        if (bagnid == NID_pkcs7_data) {
            bags = PKCS12_unpack_p7data(p7);
            if (options & INFO)
                BIO_printf(bio_err, "PKCS7 Data\n");
        } else if (bagnid == NID_pkcs7_encrypted) {
            if (options & INFO) {
                BIO_printf(bio_err, "PKCS7 Encrypted data: ");
                if (p7->d.encrypted == NULL) {
                    BIO_printf(bio_err, "<no data>\n");
                } else {
                    alg_print(p7->d.encrypted->enc_data->algorithm);
                }
            }
            bags = PKCS12_unpack_p7encdata(p7, pass, passlen);
        } else {
            continue;
        }
        if (bags == NULL)
            goto err;
        if (!dump_certs_pkeys_bags(out, bags, pass, passlen,
                                   options, pempass, enc)) {
            sk_PKCS12_SAFEBAG_pop_free(bags, PKCS12_SAFEBAG_free);
            goto err;
        }
        sk_PKCS12_SAFEBAG_pop_free(bags, PKCS12_SAFEBAG_free);
    }
    ret = 1;

 err:
    sk_PKCS7_pop_free(asafes, PKCS7_free);
    return ret;
}

int dump_certs_pkeys_bags(BIO *out, const STACK_OF(PKCS12_SAFEBAG) *bags,
                          const char *pass, int passlen, int options,
                          char *pempass, const EVP_CIPHER *enc)
{
    int i;
    for (i = 0; i < sk_PKCS12_SAFEBAG_num(bags); i++) {
        if (!dump_certs_pkeys_bag(out,
                                  sk_PKCS12_SAFEBAG_value(bags, i),
                                  pass, passlen, options, pempass, enc))
            return 0;
    }
    return 1;
}

int dump_certs_pkeys_bag(BIO *out, const PKCS12_SAFEBAG *bag,
                         const char *pass, int passlen, int options,
                         char *pempass, const EVP_CIPHER *enc)
{
    EVP_PKEY *pkey;
    PKCS8_PRIV_KEY_INFO *p8;
    const PKCS8_PRIV_KEY_INFO *p8c;
    X509 *x509;
    const STACK_OF(X509_ATTRIBUTE) *attrs;
    int ret = 0;

    attrs = PKCS12_SAFEBAG_get0_attrs(bag);

    switch (PKCS12_SAFEBAG_get_nid(bag)) {
    case NID_keyBag:
        if (options & INFO)
            BIO_printf(bio_err, "Key bag\n");
        if (options & NOKEYS)
            return 1;
        print_attribs(out, attrs, "Bag Attributes");
        p8c = PKCS12_SAFEBAG_get0_p8inf(bag);
        if ((pkey = EVP_PKCS82PKEY(p8c)) == NULL)
            return 0;
        print_attribs(out, PKCS8_pkey_get0_attrs(p8c), "Key Attributes");
        ret = PEM_write_bio_PrivateKey(out, pkey, enc, NULL, 0, NULL, pempass);
        EVP_PKEY_free(pkey);
        break;

    case NID_pkcs8ShroudedKeyBag:
        if (options & INFO) {
            const X509_SIG *tp8;
            const X509_ALGOR *tp8alg;

            BIO_printf(bio_err, "Shrouded Keybag: ");
            tp8 = PKCS12_SAFEBAG_get0_pkcs8(bag);
            X509_SIG_get0(tp8, &tp8alg, NULL);
            alg_print(tp8alg);
        }
        if (options & NOKEYS)
            return 1;
        print_attribs(out, attrs, "Bag Attributes");
        if ((p8 = PKCS12_decrypt_skey(bag, pass, passlen)) == NULL)
            return 0;
        if ((pkey = EVP_PKCS82PKEY(p8)) == NULL) {
            PKCS8_PRIV_KEY_INFO_free(p8);
            return 0;
        }
        print_attribs(out, PKCS8_pkey_get0_attrs(p8), "Key Attributes");
        PKCS8_PRIV_KEY_INFO_free(p8);
        ret = PEM_write_bio_PrivateKey(out, pkey, enc, NULL, 0, NULL, pempass);
        EVP_PKEY_free(pkey);
        break;

    case NID_certBag:
        if (options & INFO)
            BIO_printf(bio_err, "Certificate bag\n");
        if (options & NOCERTS)
            return 1;
        if (PKCS12_SAFEBAG_get0_attr(bag, NID_localKeyID)) {
            if (options & CACERTS)
                return 1;
        } else if (options & CLCERTS)
            return 1;
        print_attribs(out, attrs, "Bag Attributes");
        if (PKCS12_SAFEBAG_get_bag_nid(bag) != NID_x509Certificate)
            return 1;
        if ((x509 = PKCS12_SAFEBAG_get1_cert(bag)) == NULL)
            return 0;
        dump_cert_text(out, x509);
        ret = PEM_write_bio_X509(out, x509);
        X509_free(x509);
        break;

    case NID_secretBag:
        if (options & INFO)
            BIO_printf(bio_err, "Secret bag\n");
        print_attribs(out, attrs, "Bag Attributes");
        BIO_printf(bio_err, "Bag Type: ");
        i2a_ASN1_OBJECT(bio_err, PKCS12_SAFEBAG_get0_bag_type(bag));
        BIO_printf(bio_err, "\nBag Value: ");
        print_attribute(out, PKCS12_SAFEBAG_get0_bag_obj(bag));
        return 1;

    case NID_safeContentsBag:
        if (options & INFO)
            BIO_printf(bio_err, "Safe Contents bag\n");
        print_attribs(out, attrs, "Bag Attributes");
        return dump_certs_pkeys_bags(out, PKCS12_SAFEBAG_get0_safes(bag),
                                     pass, passlen, options, pempass, enc);

    default:
        BIO_printf(bio_err, "Warning unsupported bag type: ");
        i2a_ASN1_OBJECT(bio_err, PKCS12_SAFEBAG_get0_type(bag));
        BIO_printf(bio_err, "\n");
        return 1;
    }
    return ret;
}

/* Given a single certificate return a verified chain or NULL if error */

static int get_cert_chain(X509 *cert, X509_STORE *store,
                          STACK_OF(X509) *untrusted_certs,
                          STACK_OF(X509) **chain)
{
    X509_STORE_CTX *store_ctx = NULL;
    STACK_OF(X509) *chn = NULL;
    int i = 0;

    store_ctx = X509_STORE_CTX_new_ex(app_get0_libctx(), app_get0_propq());
    if (store_ctx == NULL) {
        i =  X509_V_ERR_UNSPECIFIED;
        goto end;
    }
    if (!X509_STORE_CTX_init(store_ctx, store, cert, untrusted_certs)) {
        i =  X509_V_ERR_UNSPECIFIED;
        goto end;
    }


    if (X509_verify_cert(store_ctx) > 0)
        chn = X509_STORE_CTX_get1_chain(store_ctx);
    else if ((i = X509_STORE_CTX_get_error(store_ctx)) == 0)
        i = X509_V_ERR_UNSPECIFIED;

end:
    X509_STORE_CTX_free(store_ctx);
    *chain = chn;
    return i;
}

static int alg_print(const X509_ALGOR *alg)
{
    int pbenid, aparamtype;
    const ASN1_OBJECT *aoid;
    const void *aparam;
    PBEPARAM *pbe = NULL;

    X509_ALGOR_get0(&aoid, &aparamtype, &aparam, alg);

    pbenid = OBJ_obj2nid(aoid);

    BIO_printf(bio_err, "%s", OBJ_nid2ln(pbenid));

    /*
     * If PBE algorithm is PBES2 decode algorithm parameters
     * for additional details.
     */
    if (pbenid == NID_pbes2) {
        PBE2PARAM *pbe2 = NULL;
        int encnid;
        if (aparamtype == V_ASN1_SEQUENCE)
            pbe2 = ASN1_item_unpack(aparam, ASN1_ITEM_rptr(PBE2PARAM));
        if (pbe2 == NULL) {
            BIO_puts(bio_err, ", <unsupported parameters>");
            goto done;
        }
        X509_ALGOR_get0(&aoid, &aparamtype, &aparam, pbe2->keyfunc);
        pbenid = OBJ_obj2nid(aoid);
        X509_ALGOR_get0(&aoid, NULL, NULL, pbe2->encryption);
        encnid = OBJ_obj2nid(aoid);
        BIO_printf(bio_err, ", %s, %s", OBJ_nid2ln(pbenid),
                   OBJ_nid2sn(encnid));
        /* If KDF is PBKDF2 decode parameters */
        if (pbenid == NID_id_pbkdf2) {
            PBKDF2PARAM *kdf = NULL;
            int prfnid;
            if (aparamtype == V_ASN1_SEQUENCE)
                kdf = ASN1_item_unpack(aparam, ASN1_ITEM_rptr(PBKDF2PARAM));
            if (kdf == NULL) {
                BIO_puts(bio_err, ", <unsupported parameters>");
                goto done;
            }

            if (kdf->prf == NULL) {
                prfnid = NID_hmacWithSHA1;
            } else {
                X509_ALGOR_get0(&aoid, NULL, NULL, kdf->prf);
                prfnid = OBJ_obj2nid(aoid);
            }
            BIO_printf(bio_err, ", Iteration %ld, PRF %s",
                       ASN1_INTEGER_get(kdf->iter), OBJ_nid2sn(prfnid));
            PBKDF2PARAM_free(kdf);
#ifndef OPENSSL_NO_SCRYPT
        } else if (pbenid == NID_id_scrypt) {
            SCRYPT_PARAMS *kdf = NULL;

            if (aparamtype == V_ASN1_SEQUENCE)
                kdf = ASN1_item_unpack(aparam, ASN1_ITEM_rptr(SCRYPT_PARAMS));
            if (kdf == NULL) {
                BIO_puts(bio_err, ", <unsupported parameters>");
                goto done;
            }
            BIO_printf(bio_err, ", Salt length: %d, Cost(N): %ld, "
                       "Block size(r): %ld, Parallelism(p): %ld",
                       ASN1_STRING_length(kdf->salt),
                       ASN1_INTEGER_get(kdf->costParameter),
                       ASN1_INTEGER_get(kdf->blockSize),
                       ASN1_INTEGER_get(kdf->parallelizationParameter));
            SCRYPT_PARAMS_free(kdf);
#endif
        }
        PBE2PARAM_free(pbe2);
    } else {
        if (aparamtype == V_ASN1_SEQUENCE)
            pbe = ASN1_item_unpack(aparam, ASN1_ITEM_rptr(PBEPARAM));
        if (pbe == NULL) {
            BIO_puts(bio_err, ", <unsupported parameters>");
            goto done;
        }
        BIO_printf(bio_err, ", Iteration %ld", ASN1_INTEGER_get(pbe->iter));
        PBEPARAM_free(pbe);
    }
 done:
    BIO_puts(bio_err, "\n");
    return 1;
}

/* Load all certificates from a given file */

int cert_load(BIO *in, STACK_OF(X509) *sk)
{
    int ret = 0;
    X509 *cert;

    while ((cert = PEM_read_bio_X509(in, NULL, NULL, NULL))) {
        ret = 1;
        if (!sk_X509_push(sk, cert))
            return 0;
    }
    if (ret)
        ERR_clear_error();
    return ret;
}

/* Generalised x509 attribute value print */

void print_attribute(BIO *out, const ASN1_TYPE *av)
{
    char *value;
    const char *ln;
    char objbuf[80];

    switch (av->type) {
    case V_ASN1_BMPSTRING:
        value = OPENSSL_uni2asc(av->value.bmpstring->data,
                                av->value.bmpstring->length);
        BIO_printf(out, "%s\n", value);
        OPENSSL_free(value);
        break;

    case V_ASN1_UTF8STRING:
        BIO_printf(out, "%.*s\n", av->value.utf8string->length,
                   av->value.utf8string->data);
        break;

    case V_ASN1_OCTET_STRING:
        hex_prin(out, av->value.octet_string->data,
                 av->value.octet_string->length);
        BIO_printf(out, "\n");
        break;

    case V_ASN1_BIT_STRING:
        hex_prin(out, av->value.bit_string->data,
                 av->value.bit_string->length);
        BIO_printf(out, "\n");
        break;

    case V_ASN1_OBJECT:
        ln = OBJ_nid2ln(OBJ_obj2nid(av->value.object));
        if (!ln)
            ln = "";
        OBJ_obj2txt(objbuf, sizeof(objbuf), av->value.object, 1);
        BIO_printf(out, "%s (%s)", ln, objbuf);
        BIO_printf(out, "\n");
        break;

    default:
        BIO_printf(out, "<Unsupported tag %d>\n", av->type);
        break;
    }
}

/* Generalised attribute print: handle PKCS#8 and bag attributes */

int print_attribs(BIO *out, const STACK_OF(X509_ATTRIBUTE) *attrlst,
                  const char *name)
{
    X509_ATTRIBUTE *attr;
    ASN1_TYPE *av;
    int i, j, attr_nid;
    if (!attrlst) {
        BIO_printf(out, "%s: <No Attributes>\n", name);
        return 1;
    }
    if (!sk_X509_ATTRIBUTE_num(attrlst)) {
        BIO_printf(out, "%s: <Empty Attributes>\n", name);
        return 1;
    }
    BIO_printf(out, "%s\n", name);
    for (i = 0; i < sk_X509_ATTRIBUTE_num(attrlst); i++) {
        ASN1_OBJECT *attr_obj;
        attr = sk_X509_ATTRIBUTE_value(attrlst, i);
        attr_obj = X509_ATTRIBUTE_get0_object(attr);
        attr_nid = OBJ_obj2nid(attr_obj);
        BIO_printf(out, "    ");
        if (attr_nid == NID_undef) {
            i2a_ASN1_OBJECT(out, attr_obj);
            BIO_printf(out, ": ");
        } else {
            BIO_printf(out, "%s: ", OBJ_nid2ln(attr_nid));
        }

        if (X509_ATTRIBUTE_count(attr)) {
            for (j = 0; j < X509_ATTRIBUTE_count(attr); j++) {
                av = X509_ATTRIBUTE_get0_type(attr, j);
                print_attribute(out, av);
            }
        } else {
            BIO_printf(out, "<No Values>\n");
        }
    }
    return 1;
}

void hex_prin(BIO *out, unsigned char *buf, int len)
{
    int i;
    for (i = 0; i < len; i++)
        BIO_printf(out, "%02X ", buf[i]);
}

static int set_pbe(int *ppbe, const char *str)
{
    if (!str)
        return 0;
    if (strcmp(str, "NONE") == 0) {
        *ppbe = -1;
        return 1;
    }
    *ppbe = OBJ_txt2nid(str);
    if (*ppbe == NID_undef) {
        BIO_printf(bio_err, "Unknown PBE algorithm %s\n", str);
        return 0;
    }
    return 1;
}
