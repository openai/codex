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
#include <time.h>
#include <string.h>
#include <ctype.h>
#include "apps.h"
#include "progs.h"
#include <openssl/core_names.h>
#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/conf.h>
#include <openssl/err.h>
#include <openssl/asn1.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/objects.h>
#include <openssl/pem.h>
#include <openssl/bn.h>
#include <openssl/lhash.h>
#include <openssl/rsa.h>
#ifndef OPENSSL_NO_DSA
# include <openssl/dsa.h>
#endif
#include "internal/e_os.h"    /* For isatty() */

#define BITS               "default_bits"
#define KEYFILE            "default_keyfile"
#define PROMPT             "prompt"
#define DISTINGUISHED_NAME "distinguished_name"
#define ATTRIBUTES         "attributes"
#define V3_EXTENSIONS      "x509_extensions"
#define REQ_EXTENSIONS     "req_extensions"
#define STRING_MASK        "string_mask"
#define UTF8_IN            "utf8"

#define DEFAULT_KEY_LENGTH 2048
#define MIN_KEY_LENGTH     512
#define DEFAULT_DAYS       30 /* default certificate validity period in days */
#define UNSET_DAYS         -2 /* -1 may be used for testing expiration checks */
#define EXT_COPY_UNSET     -1

static int make_REQ(X509_REQ *req, EVP_PKEY *pkey, X509_NAME *fsubj,
                    int mutlirdn, int attribs, unsigned long chtype);
static int prompt_info(X509_REQ *req,
                       STACK_OF(CONF_VALUE) *dn_sk, const char *dn_sect,
                       STACK_OF(CONF_VALUE) *attr_sk, const char *attr_sect,
                       int attribs, unsigned long chtype);
static int auto_info(X509_REQ *req, STACK_OF(CONF_VALUE) *sk,
                     STACK_OF(CONF_VALUE) *attr, int attribs,
                     unsigned long chtype);
static int add_attribute_object(X509_REQ *req, char *text, const char *def,
                                char *value, int nid, int n_min, int n_max,
                                unsigned long chtype);
static int add_DN_object(X509_NAME *n, char *text, const char *def,
                         char *value, int nid, int n_min, int n_max,
                         unsigned long chtype, int mval);
static int build_data(char *text, const char *def, char *value,
                      int n_min, int n_max, char *buf, const int buf_size,
                      const char *desc1, const char *desc2);
static int req_check_len(int len, int n_min, int n_max);
static int check_end(const char *str, const char *end);
static int join(char buf[], size_t buf_size, const char *name,
                const char *tail, const char *desc);
static EVP_PKEY_CTX *set_keygen_ctx(const char *gstr,
                                    char **pkeytype, long *pkeylen,
                                    ENGINE *keygen_engine);

static const char *section = "req";
static CONF *req_conf = NULL;
static CONF *addext_conf = NULL;
static int batch = 0;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_CIPHER,
    OPT_INFORM, OPT_OUTFORM, OPT_ENGINE, OPT_KEYGEN_ENGINE, OPT_KEY,
    OPT_PUBKEY, OPT_NEW, OPT_CONFIG, OPT_KEYFORM, OPT_IN, OPT_OUT,
    OPT_KEYOUT, OPT_PASSIN, OPT_PASSOUT, OPT_NEWKEY,
    OPT_PKEYOPT, OPT_SIGOPT, OPT_VFYOPT, OPT_BATCH, OPT_NEWHDR, OPT_MODULUS,
    OPT_VERIFY, OPT_NOENC, OPT_NODES, OPT_NOOUT, OPT_VERBOSE, OPT_UTF8,
    OPT_NAMEOPT, OPT_REQOPT, OPT_SUBJ, OPT_SUBJECT, OPT_TEXT,
    OPT_X509, OPT_X509V1, OPT_CA, OPT_CAKEY,
    OPT_MULTIVALUE_RDN, OPT_NOT_BEFORE, OPT_NOT_AFTER, OPT_DAYS, OPT_SET_SERIAL,
    OPT_COPY_EXTENSIONS, OPT_EXTENSIONS, OPT_REQEXTS, OPT_ADDEXT,
    OPT_PRECERT, OPT_MD,
    OPT_SECTION, OPT_QUIET,
    OPT_R_ENUM, OPT_PROV_ENUM
} OPTION_CHOICE;

const OPTIONS req_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"cipher", OPT_CIPHER, 's', "Specify the cipher for private key encryption"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
    {"keygen_engine", OPT_KEYGEN_ENGINE, 's',
     "Specify engine to be used for key generation operations"},
#endif
    {"in", OPT_IN, '<', "X.509 request input file (default stdin)"},
    {"inform", OPT_INFORM, 'F',
     "CSR input format to use (PEM or DER; by default try PEM first)"},
    {"verify", OPT_VERIFY, '-', "Verify self-signature on the request"},

    OPT_SECTION("Certificate"),
    {"new", OPT_NEW, '-', "New request"},
    {"config", OPT_CONFIG, '<', "Request template file"},
    {"section", OPT_SECTION, 's', "Config section to use (default \"req\")"},
    {"utf8", OPT_UTF8, '-', "Input characters are UTF8 (default ASCII)"},
    {"nameopt", OPT_NAMEOPT, 's', "Certificate subject/issuer name printing options"},
    {"reqopt", OPT_REQOPT, 's', "Various request text options"},
    {"text", OPT_TEXT, '-', "Text form of request"},
    {"x509", OPT_X509, '-',
     "Output an X.509 certificate structure instead of a cert request"},
    {"x509v1", OPT_X509V1, '-', "Request cert generation with X.509 version 1"},
    {"CA", OPT_CA, '<', "Issuer cert to use for signing a cert, implies -x509"},
    {"CAkey", OPT_CAKEY, 's',
     "Issuer private key to use with -CA; default is -CA arg"},
    {OPT_MORE_STR, 1, 1, "(Required by some CA's)"},
    {"subj", OPT_SUBJ, 's', "Set or modify subject of request or cert"},
    {"subject", OPT_SUBJECT, '-',
     "Print the subject of the output request or cert"},
    {"multivalue-rdn", OPT_MULTIVALUE_RDN, '-',
     "Deprecated; multi-valued RDNs support is always on."},
    {"not_before", OPT_NOT_BEFORE, 's',
     "[CC]YYMMDDHHMMSSZ value for notBefore certificate field"},
    {"not_after", OPT_NOT_AFTER, 's',
     "[CC]YYMMDDHHMMSSZ value for notAfter certificate field, overrides -days"},
    {"days", OPT_DAYS, 'p', "Number of days certificate is valid for"},
    {"set_serial", OPT_SET_SERIAL, 's', "Serial number to use"},
    {"copy_extensions", OPT_COPY_EXTENSIONS, 's',
     "copy extensions from request when using -x509"},
    {"extensions", OPT_EXTENSIONS, 's',
     "Cert or request extension section (override value in config file)"},
    {"reqexts", OPT_REQEXTS, 's', "An alias for -extensions"},
    {"addext", OPT_ADDEXT, 's',
     "Additional cert extension key=value pair (may be given more than once)"},
    {"precert", OPT_PRECERT, '-', "Add a poison extension to generated cert (implies -new)"},

    OPT_SECTION("Keys and Signing"),
    {"key", OPT_KEY, 's', "Key for signing, and to include unless -in given"},
    {"keyform", OPT_KEYFORM, 'f', "Key file format (ENGINE, other values ignored)"},
    {"pubkey", OPT_PUBKEY, '-', "Output public key"},
    {"keyout", OPT_KEYOUT, '>', "File to write private key to"},
    {"passin", OPT_PASSIN, 's', "Private key and certificate password source"},
    {"passout", OPT_PASSOUT, 's', "Output file pass phrase source"},
    {"newkey", OPT_NEWKEY, 's',
     "Generate new key with [<alg>:]<nbits> or <alg>[:<file>] or param:<file>"},
    {"pkeyopt", OPT_PKEYOPT, 's', "Public key options as opt:value"},
    {"sigopt", OPT_SIGOPT, 's', "Signature parameter in n:v form"},
    {"vfyopt", OPT_VFYOPT, 's', "Verification parameter in n:v form"},
    {"", OPT_MD, '-', "Any supported digest"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output file"},
    {"outform", OPT_OUTFORM, 'F', "Output format - DER or PEM"},
    {"batch", OPT_BATCH, '-',
     "Do not ask anything during request generation"},
    {"verbose", OPT_VERBOSE, '-', "Verbose output"},
    {"quiet", OPT_QUIET, '-', "Terse output"},
    {"noenc", OPT_NOENC, '-', "Don't encrypt private keys"},
    {"nodes", OPT_NODES, '-', "Don't encrypt private keys; deprecated"},
    {"noout", OPT_NOOUT, '-', "Do not output REQ"},
    {"newhdr", OPT_NEWHDR, '-', "Output \"NEW\" in the header lines"},
    {"modulus", OPT_MODULUS, '-', "RSA modulus"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,
    {NULL}
};

/*
 * An LHASH of strings, where each string is an extension name.
 */
static unsigned long ext_name_hash(const OPENSSL_STRING *a)
{
    return OPENSSL_LH_strhash((const char *)a);
}

static int ext_name_cmp(const OPENSSL_STRING *a, const OPENSSL_STRING *b)
{
    return strcmp((const char *)a, (const char *)b);
}

static void exts_cleanup(OPENSSL_STRING *x)
{
    OPENSSL_free((char *)x);
}

/*
 * Is the |kv| key already duplicated?
 * Return 0 if unique, -1 on runtime error, -2 on syntax error; 1 if found.
 */
static int duplicated(LHASH_OF(OPENSSL_STRING) *addexts, char *kv)
{
    char *p;
    size_t off;

    /* Check syntax. */
    /* Skip leading whitespace, make a copy. */
    while (isspace(_UC(*kv)))
        kv++;
    if ((p = strchr(kv, '=')) == NULL) {
        BIO_printf(bio_err, "Parse error on -addext: missing '='\n");
        return -2;
    }
    off = p - kv;
    if ((kv = OPENSSL_strdup(kv)) == NULL)
        return -1;

    /* Skip trailing space before the equal sign. */
    for (p = kv + off; p > kv; --p)
        if (!isspace(_UC(p[-1])))
            break;
    if (p == kv) {
        BIO_printf(bio_err, "Parse error on -addext: missing key\n");
        OPENSSL_free(kv);
        return -2;
    }
    *p = '\0';

    /* Finally have a clean "key"; see if it's there [by attempt to add it]. */
    p = (char *)lh_OPENSSL_STRING_insert(addexts, (OPENSSL_STRING *)kv);
    if (p != NULL) {
        BIO_printf(bio_err, "Duplicate extension name: %s\n", kv);
        OPENSSL_free(p);
        return 1;
    } else if (lh_OPENSSL_STRING_error(addexts)) {
        OPENSSL_free(kv);
        return -1;
    }

    return 0;
}

int req_main(int argc, char **argv)
{
    ASN1_INTEGER *serial = NULL;
    BIO *out = NULL;
    ENGINE *e = NULL, *gen_eng = NULL;
    EVP_PKEY *pkey = NULL, *CAkey = NULL;
    EVP_PKEY_CTX *genctx = NULL;
    STACK_OF(OPENSSL_STRING) *pkeyopts = NULL, *sigopts = NULL, *vfyopts = NULL;
    LHASH_OF(OPENSSL_STRING) *addexts = NULL;
    X509 *new_x509 = NULL, *CAcert = NULL;
    X509_REQ *req = NULL;
    const EVP_CIPHER *cipher = NULL;
    int ext_copy = EXT_COPY_UNSET;
    BIO *addext_bio = NULL;
    char *extsect = NULL;
    const char *infile = NULL, *CAfile = NULL, *CAkeyfile = NULL;
    char *outfile = NULL, *keyfile = NULL, *digest = NULL;
    char *keyalgstr = NULL, *p, *prog, *passargin = NULL, *passargout = NULL;
    char *passin = NULL, *passout = NULL;
    char *nofree_passin = NULL, *nofree_passout = NULL;
    char *subj = NULL;
    X509_NAME *fsubj = NULL;
    char *template = default_config_file, *keyout = NULL;
    const char *keyalg = NULL;
    OPTION_CHOICE o;
    char *not_before = NULL, *not_after = NULL;
    int days = UNSET_DAYS;
    int ret = 1, gen_x509 = 0, i = 0, newreq = 0, verbose = 0, progress = 1;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, keyform = FORMAT_UNDEF;
    int modulus = 0, multirdn = 1, verify = 0, noout = 0, text = 0;
    int noenc = 0, newhdr = 0, subject = 0, pubkey = 0, precert = 0, x509v1 = 0;
    long newkey_len = -1;
    unsigned long chtype = MBSTRING_ASC, reqflag = 0;

    cipher = (EVP_CIPHER *)EVP_aes_256_cbc();

    opt_set_unknown_name("digest");
    prog = opt_init(argc, argv, req_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(req_options);
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
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_KEYGEN_ENGINE:
#ifndef OPENSSL_NO_ENGINE
            gen_eng = setup_engine(opt_arg(), 0);
            if (gen_eng == NULL) {
                BIO_printf(bio_err, "Can't find keygen engine %s\n", *argv);
                goto opthelp;
            }
#endif
            break;
        case OPT_KEY:
            keyfile = opt_arg();
            break;
        case OPT_PUBKEY:
            pubkey = 1;
            break;
        case OPT_NEW:
            newreq = 1;
            break;
        case OPT_CONFIG:
            template = opt_arg();
            break;
        case OPT_SECTION:
            section = opt_arg();
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyform))
                goto opthelp;
            break;
        case OPT_IN:
            infile = opt_arg();
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_KEYOUT:
            keyout = opt_arg();
            break;
        case OPT_PASSIN:
            passargin = opt_arg();
            break;
        case OPT_PASSOUT:
            passargout = opt_arg();
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_NEWKEY:
            keyalg = opt_arg();
            newreq = 1;
            break;
        case OPT_PKEYOPT:
            if (pkeyopts == NULL)
                pkeyopts = sk_OPENSSL_STRING_new_null();
            if (pkeyopts == NULL
                    || !sk_OPENSSL_STRING_push(pkeyopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_SIGOPT:
            if (!sigopts)
                sigopts = sk_OPENSSL_STRING_new_null();
            if (!sigopts || !sk_OPENSSL_STRING_push(sigopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_VFYOPT:
            if (!vfyopts)
                vfyopts = sk_OPENSSL_STRING_new_null();
            if (!vfyopts || !sk_OPENSSL_STRING_push(vfyopts, opt_arg()))
                goto opthelp;
            break;
        case OPT_BATCH:
            batch = 1;
            break;
        case OPT_NEWHDR:
            newhdr = 1;
            break;
        case OPT_MODULUS:
            modulus = 1;
            break;
        case OPT_VERIFY:
            verify = 1;
            break;
        case OPT_NODES:
        case OPT_NOENC:
            noenc = 1;
            break;
        case OPT_NOOUT:
            noout = 1;
            break;
        case OPT_VERBOSE:
            verbose = 1;
            progress = 1;
            break;
        case OPT_QUIET:
            verbose = 0;
            progress = 0;
            break;
        case OPT_UTF8:
            chtype = MBSTRING_UTF8;
            break;
        case OPT_NAMEOPT:
            if (!set_nameopt(opt_arg()))
                goto opthelp;
            break;
        case OPT_REQOPT:
            if (!set_cert_ex(&reqflag, opt_arg()))
                goto opthelp;
            break;
        case OPT_TEXT:
            text = 1;
            break;
        case OPT_X509V1:
            x509v1 = 1;
            /* fall thru */
        case OPT_X509:
            gen_x509 = 1;
            break;
        case OPT_CA:
            CAfile = opt_arg();
            gen_x509 = 1;
            break;
        case OPT_CAKEY:
            CAkeyfile = opt_arg();
            break;
        case OPT_NOT_BEFORE:
            not_before = opt_arg();
            break;
        case OPT_NOT_AFTER:
            not_after = opt_arg();
            break;
        case OPT_DAYS:
            days = atoi(opt_arg());
            if (days <= UNSET_DAYS) {
                BIO_printf(bio_err, "%s: -days parameter arg must be >= -1\n",
                           prog);
                goto end;
            }
            break;
        case OPT_SET_SERIAL:
            if (serial != NULL) {
                BIO_printf(bio_err, "Serial number supplied twice\n");
                goto opthelp;
            }
            serial = s2i_ASN1_INTEGER(NULL, opt_arg());
            if (serial == NULL)
                goto opthelp;
            break;
        case OPT_SUBJECT:
            subject = 1;
            break;
        case OPT_SUBJ:
            subj = opt_arg();
            break;
        case OPT_MULTIVALUE_RDN:
            /* obsolete */
            break;
        case OPT_COPY_EXTENSIONS:
            if (!set_ext_copy(&ext_copy, opt_arg())) {
                BIO_printf(bio_err, "Invalid extension copy option: \"%s\"\n",
                           opt_arg());
                goto end;
            }
            break;
        case OPT_EXTENSIONS:
        case OPT_REQEXTS:
            extsect = opt_arg();
            break;
        case OPT_ADDEXT:
            p = opt_arg();
            if (addexts == NULL) {
                addexts = lh_OPENSSL_STRING_new(ext_name_hash, ext_name_cmp);
                addext_bio = BIO_new(BIO_s_mem());
                if (addexts == NULL || addext_bio == NULL)
                    goto end;
            }
            i = duplicated(addexts, p);
            if (i == 1)
                goto end;
            if (i == -1)
                BIO_printf(bio_err, "Internal error handling -addext %s\n", p);
            if (i < 0 || BIO_printf(addext_bio, "%s\n", p) < 0)
                goto end;
            break;
        case OPT_PRECERT:
            newreq = precert = 1;
            break;
        case OPT_CIPHER:
            cipher = EVP_get_cipherbyname(opt_arg());
            if (cipher == NULL) {
                BIO_printf(bio_err, "Unknown cipher: %s\n", opt_arg());
                goto opthelp;
            }
            break;
        case OPT_MD:
            digest = opt_unknown();
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!app_RAND_load())
        goto end;

    if (!gen_x509) {
        if (days != UNSET_DAYS)
            BIO_printf(bio_err, "Warning: Ignoring -days without -x509; not generating a certificate\n");
        if (not_before != NULL)
            BIO_printf(bio_err, "Warning: Ignoring -not_before without -x509; not generating a certificate\n");
        if (not_after != NULL)
            BIO_printf(bio_err, "Warning: Ignoring -not_after without -x509; not generating a certificate\n");
        if (ext_copy == EXT_COPY_NONE)
            BIO_printf(bio_err, "Warning: Ignoring -copy_extensions 'none' when -x509 is not given\n");
    }
    if (infile == NULL) {
        if (gen_x509)
            newreq = 1;
        else if (!newreq && isatty(fileno_stdin()))
            BIO_printf(bio_err,
                       "Warning: Will read cert request from stdin since no -in option is given\n");
    }

    if (!app_passwd(passargin, passargout, &passin, &passout)) {
        BIO_printf(bio_err, "Error getting passwords\n");
        goto end;
    }

    if ((req_conf = app_load_config_verbose(template, verbose)) == NULL)
        goto end;
    if (addext_bio != NULL) {
        if (verbose)
            BIO_printf(bio_err,
                       "Using additional configuration from -addext options\n");
        if ((addext_conf = app_load_config_bio(addext_bio, NULL)) == NULL)
            goto end;
    }
    if (template != default_config_file && !app_load_modules(req_conf))
        goto end;

    if (req_conf != NULL) {
        p = app_conf_try_string(req_conf, NULL, "oid_file");
        if (p != NULL) {
            BIO *oid_bio = BIO_new_file(p, "r");

            if (oid_bio == NULL) {
                if (verbose)
                    BIO_printf(bio_err,
                               "Problems opening '%s' for extra OIDs\n", p);
            } else {
                OBJ_create_objects(oid_bio);
                BIO_free(oid_bio);
            }
        }
    }
    if (!add_oid_section(req_conf))
        goto end;

    /* Check that any specified digest is fetchable */
    if (digest != NULL) {
        if (!opt_check_md(digest))
            goto opthelp;
    } else {
        /* No digest specified, default to configuration */
        p = app_conf_try_string(req_conf, section, "default_md");
        if (p != NULL)
            digest = p;
    }

    if (extsect == NULL)
        extsect = app_conf_try_string(req_conf, section,
                                   gen_x509 ? V3_EXTENSIONS : REQ_EXTENSIONS);
    if (extsect != NULL) {
        /* Check syntax of extension section in config file */
        X509V3_CTX ctx;

        X509V3_set_ctx_test(&ctx);
        X509V3_set_nconf(&ctx, req_conf);
        if (!X509V3_EXT_add_nconf(req_conf, &ctx, extsect, NULL)) {
            BIO_printf(bio_err,
                       "Error checking %s extension section %s\n",
                       gen_x509 ? "x509" : "request", extsect);
            goto end;
        }
    }
    if (addext_conf != NULL) {
        /* Check syntax of command line extensions */
        X509V3_CTX ctx;

        X509V3_set_ctx_test(&ctx);
        X509V3_set_nconf(&ctx, req_conf);
        if (!X509V3_EXT_add_nconf(addext_conf, &ctx, "default", NULL)) {
            BIO_printf(bio_err, "Error checking extensions defined using -addext\n");
            goto end;
        }
    }

    if (passin == NULL)
        passin = nofree_passin =
            app_conf_try_string(req_conf, section, "input_password");

    if (passout == NULL)
        passout = nofree_passout =
            app_conf_try_string(req_conf, section, "output_password");

    p = app_conf_try_string(req_conf, section, STRING_MASK);
    if (p != NULL && !ASN1_STRING_set_default_mask_asc(p)) {
        BIO_printf(bio_err, "Invalid global string mask setting %s\n", p);
        goto end;
    }

    if (chtype != MBSTRING_UTF8) {
        p = app_conf_try_string(req_conf, section, UTF8_IN);
        if (p != NULL && strcmp(p, "yes") == 0)
            chtype = MBSTRING_UTF8;
    }

    if (keyfile != NULL) {
        pkey = load_key(keyfile, keyform, 0, passin, e, "private key");
        if (pkey == NULL)
            goto end;
        app_RAND_load_conf(req_conf, section);
    }
    if (keyalg != NULL && pkey != NULL) {
        BIO_printf(bio_err,
                   "Warning: Not generating key via given -newkey option since -key is given\n");
        /* Better throw an error in this case */
    }
    if (newreq && pkey == NULL) {
        app_RAND_load_conf(req_conf, section);

        if (!app_conf_try_number(req_conf, section, BITS, &newkey_len))
            newkey_len = DEFAULT_KEY_LENGTH;

        genctx = set_keygen_ctx(keyalg, &keyalgstr, &newkey_len, gen_eng);
        if (genctx == NULL)
            goto end;

        if (newkey_len < MIN_KEY_LENGTH
            && (EVP_PKEY_CTX_is_a(genctx, "RSA")
                || EVP_PKEY_CTX_is_a(genctx, "RSA-PSS")
                || EVP_PKEY_CTX_is_a(genctx, "DSA"))) {
            BIO_printf(bio_err, "Private key length too short, needs to be at least %d bits, not %ld.\n",
                       MIN_KEY_LENGTH, newkey_len);
            goto end;
        }

        if (newkey_len > OPENSSL_RSA_MAX_MODULUS_BITS
            && (EVP_PKEY_CTX_is_a(genctx, "RSA")
                || EVP_PKEY_CTX_is_a(genctx, "RSA-PSS")))
            BIO_printf(bio_err,
                       "Warning: It is not recommended to use more than %d bit for RSA keys.\n"
                       "         Your key size is %ld! Larger key size may behave not as expected.\n",
                       OPENSSL_RSA_MAX_MODULUS_BITS, newkey_len);

#ifndef OPENSSL_NO_DSA
        if (EVP_PKEY_CTX_is_a(genctx, "DSA")
                && newkey_len > OPENSSL_DSA_MAX_MODULUS_BITS)
            BIO_printf(bio_err,
                       "Warning: It is not recommended to use more than %d bit for DSA keys.\n"
                       "         Your key size is %ld! Larger key size may behave not as expected.\n",
                       OPENSSL_DSA_MAX_MODULUS_BITS, newkey_len);
#endif

        if (pkeyopts != NULL) {
            char *genopt;
            for (i = 0; i < sk_OPENSSL_STRING_num(pkeyopts); i++) {
                genopt = sk_OPENSSL_STRING_value(pkeyopts, i);
                if (pkey_ctrl_string(genctx, genopt) <= 0) {
                    BIO_printf(bio_err, "Key parameter error \"%s\"\n", genopt);
                    goto end;
                }
            }
        }

        EVP_PKEY_CTX_set_app_data(genctx, bio_err);
        if (progress)
            EVP_PKEY_CTX_set_cb(genctx, progress_cb);

        pkey = app_keygen(genctx, keyalgstr, newkey_len, verbose);
        if (pkey == NULL)
            goto end;

        EVP_PKEY_CTX_free(genctx);
        genctx = NULL;
    }
    if (keyout == NULL && keyfile == NULL)
        keyout = app_conf_try_string(req_conf, section, KEYFILE);

    if (pkey != NULL && (keyfile == NULL || keyout != NULL)) {
        if (verbose) {
            BIO_printf(bio_err, "Writing private key to ");
            if (keyout == NULL)
                BIO_printf(bio_err, "stdout\n");
            else
                BIO_printf(bio_err, "'%s'\n", keyout);
        }
        out = bio_open_owner(keyout, outformat, newreq);
        if (out == NULL)
            goto end;

        p = app_conf_try_string(req_conf, section, "encrypt_rsa_key");
        if (p == NULL)
            p = app_conf_try_string(req_conf, section, "encrypt_key");
        if (p != NULL && strcmp(p, "no") == 0)
            cipher = NULL;
        if (noenc)
            cipher = NULL;

        i = 0;
 loop:
        if (!PEM_write_bio_PrivateKey(out, pkey, cipher,
                                      NULL, 0, NULL, passout)) {
            if ((ERR_GET_REASON(ERR_peek_error()) ==
                 PEM_R_PROBLEMS_GETTING_PASSWORD) && (i < 3)) {
                ERR_clear_error();
                i++;
                goto loop;
            }
            goto end;
        }
        BIO_free_all(out);
        out = NULL;
        BIO_printf(bio_err, "-----\n");
    }

    /*
     * subj is expected to be in the format /type0=value0/type1=value1/type2=...
     * where characters may be escaped by \
     */
    if (subj != NULL
            && (fsubj = parse_name(subj, chtype, multirdn, "subject")) == NULL)
        goto end;

    if (!newreq) {
        if (keyfile != NULL)
            BIO_printf(bio_err,
                       "Warning: Not placing -key in cert or request since request is used\n");
        req = load_csr_autofmt(infile /* if NULL, reads from stdin */,
                               informat, vfyopts, "X509 request");
        if (req == NULL)
            goto end;
    } else if (infile != NULL) {
        BIO_printf(bio_err,
                   "Warning: Ignoring -in option since -new or -newkey or -precert is given\n");
        /* Better throw an error in this case, as done in the x509 app */
    }

    if (CAkeyfile == NULL)
        CAkeyfile = CAfile;
    if (CAkeyfile != NULL) {
        if (CAfile == NULL) {
            BIO_printf(bio_err,
                       "Warning: Ignoring -CAkey option since no -CA option is given\n");
        } else {
            if ((CAkey = load_key(CAkeyfile, FORMAT_UNDEF,
                                  0, passin, e,
                                  CAkeyfile != CAfile
                                  ? "issuer private key from -CAkey arg"
                                  : "issuer private key from -CA arg")) == NULL)
                goto end;
        }
    }
    if (CAfile != NULL) {
        if ((CAcert = load_cert_pass(CAfile, FORMAT_UNDEF, 1, passin,
                                     "issuer cert from -CA arg")) == NULL)
            goto end;
        if (!X509_check_private_key(CAcert, CAkey)) {
            BIO_printf(bio_err,
                       "Issuer CA certificate and key do not match\n");
            goto end;
        }
    }
    if (newreq || gen_x509) {
        if (CAcert == NULL && pkey == NULL) {
            BIO_printf(bio_err, "Must provide a signature key using -key or"
                " provide -CA / -CAkey\n");
            goto end;
        }

        if (req == NULL) {
            req = X509_REQ_new_ex(app_get0_libctx(), app_get0_propq());
            if (req == NULL) {
                goto end;
            }

            if (!make_REQ(req, pkey, fsubj, multirdn, !gen_x509, chtype)) {
                BIO_printf(bio_err, "Error making certificate request\n");
                goto end;
            }
            /* Note that -x509 can take over -key and -subj option values. */
        }
        if (gen_x509) {
            EVP_PKEY *pub_key = X509_REQ_get0_pubkey(req);
            EVP_PKEY *issuer_key = CAcert != NULL ? CAkey : pkey;
            X509V3_CTX ext_ctx;
            X509_NAME *issuer = CAcert != NULL ? X509_get_subject_name(CAcert) :
                X509_REQ_get_subject_name(req);
            X509_NAME *n_subj = fsubj != NULL ? fsubj :
                X509_REQ_get_subject_name(req);

            if (CAcert != NULL && keyfile != NULL)
                BIO_printf(bio_err,
                           "Warning: Not using -key or -newkey for signing since -CA option is given\n");

            if ((new_x509 = X509_new_ex(app_get0_libctx(),
                                        app_get0_propq())) == NULL)
                goto end;

            if (serial != NULL) {
                if (!X509_set_serialNumber(new_x509, serial))
                    goto end;
            } else {
                if (!rand_serial(NULL, X509_get_serialNumber(new_x509)))
                    goto end;
            }

            if (!X509_set_issuer_name(new_x509, issuer))
                goto end;
            if (days == UNSET_DAYS)
                days = DEFAULT_DAYS;
            else if (not_after != NULL)
                BIO_printf(bio_err,"Warning: -not_after option overriding -days option\n");
            if (!set_cert_times(new_x509, not_before, not_after, days, 1))
                goto end;
            if (!X509_set_subject_name(new_x509, n_subj))
                goto end;
            if (!pub_key || !X509_set_pubkey(new_x509, pub_key))
                goto end;
            if (ext_copy == EXT_COPY_UNSET) {
                if (infile != NULL)
                    BIO_printf(bio_err, "Warning: No -copy_extensions given; ignoring any extensions in the request\n");
            } else if (!copy_extensions(new_x509, req, ext_copy)) {
                BIO_printf(bio_err, "Error copying extensions from request\n");
                goto end;
            }

            /* Set up V3 context struct */
            X509V3_set_ctx(&ext_ctx, CAcert != NULL ? CAcert : new_x509,
                           new_x509, NULL, NULL, X509V3_CTX_REPLACE);
            /* prepare fallback for AKID, but only if issuer cert == new_x509 */
            if (CAcert == NULL) {
                if (!X509V3_set_issuer_pkey(&ext_ctx, issuer_key))
                    goto end;
                if (!cert_matches_key(new_x509, issuer_key))
                    BIO_printf(bio_err,
                               "Warning: Signature key and public key of cert do not match\n");
            }
            X509V3_set_nconf(&ext_ctx, req_conf);

            /* Add extensions */
            if (extsect != NULL
                && !X509V3_EXT_add_nconf(req_conf, &ext_ctx, extsect, new_x509)) {
                BIO_printf(bio_err, "Error adding x509 extensions from section %s\n",
                           extsect);
                goto end;
            }
            if (addext_conf != NULL
                && !X509V3_EXT_add_nconf(addext_conf, &ext_ctx, "default",
                                         new_x509)) {
                BIO_printf(bio_err, "Error adding x509 extensions defined via -addext\n");
                goto end;
            }

            /* If a pre-cert was requested, we need to add a poison extension */
            if (precert) {
                if (X509_add1_ext_i2d(new_x509, NID_ct_precert_poison,
                                      NULL, 1, 0) != 1) {
                    BIO_printf(bio_err, "Error adding poison extension\n");
                    goto end;
                }
            }

            i = do_X509_sign(new_x509, x509v1, issuer_key, digest, sigopts,
                             &ext_ctx);
            if (!i)
                goto end;
        } else {
            X509V3_CTX ext_ctx;

            if (precert) {
                BIO_printf(bio_err,
                           "Warning: Ignoring -precert flag since no cert is produced\n");
            }
            /* Set up V3 context struct */
            X509V3_set_ctx(&ext_ctx, NULL, NULL, req, NULL, X509V3_CTX_REPLACE);
            X509V3_set_nconf(&ext_ctx, req_conf);

            /* Add extensions */
            if (extsect != NULL
                && !X509V3_EXT_REQ_add_nconf(req_conf, &ext_ctx, extsect, req)) {
                BIO_printf(bio_err, "Error adding request extensions from section %s\n",
                           extsect);
                goto end;
            }
            if (addext_conf != NULL
                && !X509V3_EXT_REQ_add_nconf(addext_conf, &ext_ctx, "default",
                                             req)) {
                BIO_printf(bio_err, "Error adding request extensions defined via -addext\n");
                goto end;
            }
            i = do_X509_REQ_sign(req, pkey, digest, sigopts);
            if (!i)
                goto end;
        }
    }

    if (subj != NULL && !newreq && !gen_x509) {
        if (verbose) {
            BIO_printf(out, "Modifying subject of certificate request\n");
            print_name(out, "Old subject=", X509_REQ_get_subject_name(req));
        }

        if (!X509_REQ_set_subject_name(req, fsubj)) {
            BIO_printf(bio_err, "Error modifying subject of certificate request\n");
            goto end;
        }

        if (verbose) {
            print_name(out, "New subject=", X509_REQ_get_subject_name(req));
        }
    }

    if (verify) {
        EVP_PKEY *tpubkey = pkey;

        if (tpubkey == NULL) {
            tpubkey = X509_REQ_get0_pubkey(req);
            if (tpubkey == NULL)
                goto end;
        }

        i = do_X509_REQ_verify(req, tpubkey, vfyopts);

        if (i < 0)
            goto end;
        if (i == 0) {
            BIO_printf(bio_err, "Certificate request self-signature verify failure\n");
	    goto end;
        } else /* i > 0 */
            BIO_printf(bio_out, "Certificate request self-signature verify OK\n");
    }

    if (noout && !text && !modulus && !subject && !pubkey) {
        ret = 0;
        goto end;
    }

    out = bio_open_default(outfile,
                           keyout != NULL && outfile != NULL &&
                           strcmp(keyout, outfile) == 0 ? 'a' : 'w',
                           outformat);
    if (out == NULL)
        goto end;

    if (pubkey) {
        EVP_PKEY *tpubkey = X509_REQ_get0_pubkey(req);

        if (tpubkey == NULL) {
            BIO_printf(bio_err, "Error getting public key\n");
            goto end;
        }
        PEM_write_bio_PUBKEY(out, tpubkey);
    }

    if (text) {
        if (gen_x509)
            ret = X509_print_ex(out, new_x509, get_nameopt(), reqflag);
        else
            ret = X509_REQ_print_ex(out, req, get_nameopt(), reqflag);

        if (ret == 0) {
            if (gen_x509)
                BIO_printf(bio_err, "Error printing certificate\n");
            else
                BIO_printf(bio_err, "Error printing certificate request\n");
            goto end;
        }
    }

    if (subject) {
        print_name(out, "subject=", gen_x509
                   ? X509_get_subject_name(new_x509)
                   : X509_REQ_get_subject_name(req));
    }

    if (modulus) {
        EVP_PKEY *tpubkey;

        if (gen_x509)
            tpubkey = X509_get0_pubkey(new_x509);
        else
            tpubkey = X509_REQ_get0_pubkey(req);
        if (tpubkey == NULL) {
            BIO_puts(bio_err, "Modulus is unavailable\n");
            goto end;
        }
        BIO_puts(out, "Modulus=");
        if (EVP_PKEY_is_a(tpubkey, "RSA") || EVP_PKEY_is_a(tpubkey, "RSA-PSS")) {
            BIGNUM *n = NULL;

            if (!EVP_PKEY_get_bn_param(tpubkey, "n", &n))
                goto end;
            BN_print(out, n);
            BN_free(n);
        } else {
            BIO_puts(out, "Wrong Algorithm type");
        }
        BIO_puts(out, "\n");
    }

    if (!noout && !gen_x509) {
        if (outformat == FORMAT_ASN1)
            i = i2d_X509_REQ_bio(out, req);
        else if (newhdr)
            i = PEM_write_bio_X509_REQ_NEW(out, req);
        else
            i = PEM_write_bio_X509_REQ(out, req);
        if (!i) {
            BIO_printf(bio_err, "Unable to write certificate request\n");
            goto end;
        }
    }
    if (!noout && gen_x509 && new_x509 != NULL) {
        if (outformat == FORMAT_ASN1)
            i = i2d_X509_bio(out, new_x509);
        else
            i = PEM_write_bio_X509(out, new_x509);
        if (!i) {
            BIO_printf(bio_err, "Unable to write X509 certificate\n");
            goto end;
        }
    }
    ret = 0;
 end:
    if (ret) {
        ERR_print_errors(bio_err);
    }
    NCONF_free(req_conf);
    NCONF_free(addext_conf);
    BIO_free(addext_bio);
    BIO_free_all(out);
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(genctx);
    sk_OPENSSL_STRING_free(pkeyopts);
    sk_OPENSSL_STRING_free(sigopts);
    sk_OPENSSL_STRING_free(vfyopts);
    lh_OPENSSL_STRING_doall(addexts, exts_cleanup);
    lh_OPENSSL_STRING_free(addexts);
#ifndef OPENSSL_NO_ENGINE
    release_engine(gen_eng);
#endif
    OPENSSL_free(keyalgstr);
    X509_REQ_free(req);
    X509_NAME_free(fsubj);
    X509_free(new_x509);
    X509_free(CAcert);
    EVP_PKEY_free(CAkey);
    ASN1_INTEGER_free(serial);
    release_engine(e);
    if (passin != nofree_passin)
        OPENSSL_free(passin);
    if (passout != nofree_passout)
        OPENSSL_free(passout);
    return ret;
}

static int make_REQ(X509_REQ *req, EVP_PKEY *pkey, X509_NAME *fsubj,
                    int multirdn, int attribs, unsigned long chtype)
{
    int ret = 0, i;
    char no_prompt = 0;
    STACK_OF(CONF_VALUE) *dn_sk = NULL, *attr_sk = NULL;
    char *tmp, *dn_sect, *attr_sect;

    tmp = app_conf_try_string(req_conf, section, PROMPT);
    if (tmp != NULL && strcmp(tmp, "no") == 0)
        no_prompt = 1;

    dn_sect = app_conf_try_string(req_conf, section, DISTINGUISHED_NAME);
    if (dn_sect != NULL) {
        dn_sk = NCONF_get_section(req_conf, dn_sect);
        if (dn_sk == NULL) {
            BIO_printf(bio_err, "Unable to get '%s' section\n", dn_sect);
            goto err;
        }
    }

    attr_sect = app_conf_try_string(req_conf, section, ATTRIBUTES);
    if (attr_sect != NULL) {
        attr_sk = NCONF_get_section(req_conf, attr_sect);
        if (attr_sk == NULL) {
            BIO_printf(bio_err, "Unable to get '%s' section\n", attr_sect);
            goto err;
        }
    }

    /* so far there is only version 1 */
    if (!X509_REQ_set_version(req, X509_REQ_VERSION_1))
        goto err;

    if (fsubj != NULL)
        i = X509_REQ_set_subject_name(req, fsubj);
    else if (no_prompt)
        i = auto_info(req, dn_sk, attr_sk, attribs, chtype);
    else
        i = prompt_info(req, dn_sk, dn_sect, attr_sk, attr_sect, attribs,
                        chtype);
    if (!i)
        goto err;

    if (!X509_REQ_set_pubkey(req, pkey))
        goto err;

    ret = 1;
 err:
    return ret;
}

static int prompt_info(X509_REQ *req,
                       STACK_OF(CONF_VALUE) *dn_sk, const char *dn_sect,
                       STACK_OF(CONF_VALUE) *attr_sk, const char *attr_sect,
                       int attribs, unsigned long chtype)
{
    int i;
    char *p, *q;
    char buf[100];
    int nid, mval;
    long n_min, n_max;
    char *type, *value;
    const char *def;
    CONF_VALUE *v;
    X509_NAME *subj = X509_REQ_get_subject_name(req);

    if (!batch) {
        BIO_printf(bio_err,
                   "You are about to be asked to enter information that will be incorporated\n");
        BIO_printf(bio_err, "into your certificate request.\n");
        BIO_printf(bio_err,
                   "What you are about to enter is what is called a Distinguished Name or a DN.\n");
        BIO_printf(bio_err,
                   "There are quite a few fields but you can leave some blank\n");
        BIO_printf(bio_err,
                   "For some fields there will be a default value,\n");
        BIO_printf(bio_err,
                   "If you enter '.', the field will be left blank.\n");
        BIO_printf(bio_err, "-----\n");
    }

    if (sk_CONF_VALUE_num(dn_sk)) {
        i = -1;
 start:
        for (;;) {
            i++;
            if (sk_CONF_VALUE_num(dn_sk) <= i)
                break;

            v = sk_CONF_VALUE_value(dn_sk, i);
            p = q = NULL;
            type = v->name;
            if (!check_end(type, "_min") || !check_end(type, "_max") ||
                !check_end(type, "_default") || !check_end(type, "_value"))
                continue;
            /*
             * Skip past any leading X. X: X, etc to allow for multiple
             * instances
             */
            for (p = v->name; *p; p++)
                if ((*p == ':') || (*p == ',') || (*p == '.')) {
                    p++;
                    if (*p)
                        type = p;
                    break;
                }
            if (*type == '+') {
                mval = -1;
                type++;
            } else {
                mval = 0;
            }
            /* If OBJ not recognised ignore it */
            if ((nid = OBJ_txt2nid(type)) == NID_undef)
                goto start;
            if (!join(buf, sizeof(buf), v->name, "_default", "Name"))
                return 0;
            if ((def = app_conf_try_string(req_conf, dn_sect, buf)) == NULL)
                def = "";

            if (!join(buf, sizeof(buf), v->name, "_value", "Name"))
                return 0;
            if ((value = app_conf_try_string(req_conf, dn_sect, buf)) == NULL)
                value = NULL;

            if (!join(buf, sizeof(buf), v->name, "_min", "Name"))
                return 0;
            if (!app_conf_try_number(req_conf, dn_sect, buf, &n_min))
                n_min = -1;

            if (!join(buf, sizeof(buf), v->name, "_max", "Name"))
                return 0;
            if (!app_conf_try_number(req_conf, dn_sect, buf, &n_max))
                n_max = -1;

            if (!add_DN_object(subj, v->value, def, value, nid,
                               n_min, n_max, chtype, mval))
                return 0;
        }
        if (X509_NAME_entry_count(subj) == 0) {
            BIO_printf(bio_err, "Error: No objects specified in config file\n");
            return 0;
        }

        if (attribs) {
            if ((attr_sk != NULL) && (sk_CONF_VALUE_num(attr_sk) > 0)
                && (!batch)) {
                BIO_printf(bio_err,
                           "\nPlease enter the following 'extra' attributes\n");
                BIO_printf(bio_err,
                           "to be sent with your certificate request\n");
            }

            i = -1;
 start2:
            for (;;) {
                i++;
                if ((attr_sk == NULL) || (sk_CONF_VALUE_num(attr_sk) <= i))
                    break;

                v = sk_CONF_VALUE_value(attr_sk, i);
                type = v->name;
                if ((nid = OBJ_txt2nid(type)) == NID_undef)
                    goto start2;

                if (!join(buf, sizeof(buf), type, "_default", "Name"))
                    return 0;
                def = app_conf_try_string(req_conf, attr_sect, buf);
                if (def == NULL)
                    def = "";

                if (!join(buf, sizeof(buf), type, "_value", "Name"))
                    return 0;
                value = app_conf_try_string(req_conf, attr_sect, buf);

                if (!join(buf, sizeof(buf), type, "_min", "Name"))
                    return 0;
                if (!app_conf_try_number(req_conf, attr_sect, buf, &n_min))
                    n_min = -1;

                if (!join(buf, sizeof(buf), type, "_max", "Name"))
                    return 0;
                if (!app_conf_try_number(req_conf, attr_sect, buf, &n_max))
                    n_max = -1;

                if (!add_attribute_object(req,
                                          v->value, def, value, nid, n_min,
                                          n_max, chtype))
                    return 0;
            }
        }
    } else {
        BIO_printf(bio_err, "No template, please set one up.\n");
        return 0;
    }

    return 1;

}

static int auto_info(X509_REQ *req, STACK_OF(CONF_VALUE) *dn_sk,
                     STACK_OF(CONF_VALUE) *attr_sk, int attribs,
                     unsigned long chtype)
{
    int i, spec_char, plus_char;
    char *p, *q;
    char *type;
    CONF_VALUE *v;
    X509_NAME *subj;

    subj = X509_REQ_get_subject_name(req);

    for (i = 0; i < sk_CONF_VALUE_num(dn_sk); i++) {
        int mval;
        v = sk_CONF_VALUE_value(dn_sk, i);
        p = q = NULL;
        type = v->name;
        /*
         * Skip past any leading X. X: X, etc to allow for multiple instances
         */
        for (p = v->name; *p; p++) {
#ifndef CHARSET_EBCDIC
            spec_char = (*p == ':' || *p == ',' || *p == '.');
#else
            spec_char = (*p == os_toascii[':'] || *p == os_toascii[',']
                         || *p == os_toascii['.']);
#endif
            if (spec_char) {
                p++;
                if (*p)
                    type = p;
                break;
            }
        }
#ifndef CHARSET_EBCDIC
        plus_char = (*type == '+');
#else
        plus_char = (*type == os_toascii['+']);
#endif
        if (plus_char) {
            type++;
            mval = -1;
        } else {
            mval = 0;
        }
        if (!X509_NAME_add_entry_by_txt(subj, type, chtype,
                                        (unsigned char *)v->value, -1, -1,
                                        mval))
            return 0;

    }

    if (!X509_NAME_entry_count(subj)) {
        BIO_printf(bio_err, "Error: No objects specified in config file\n");
        return 0;
    }
    if (attribs) {
        for (i = 0; i < sk_CONF_VALUE_num(attr_sk); i++) {
            v = sk_CONF_VALUE_value(attr_sk, i);
            if (!X509_REQ_add1_attr_by_txt(req, v->name, chtype,
                                           (unsigned char *)v->value, -1))
                return 0;
        }
    }
    return 1;
}

static int add_DN_object(X509_NAME *n, char *text, const char *def,
                         char *value, int nid, int n_min, int n_max,
                         unsigned long chtype, int mval)
{
    int ret = 0;
    char buf[1024];

    ret = build_data(text, def, value, n_min, n_max, buf, sizeof(buf),
                     "DN value", "DN default");
    if ((ret == 0) || (ret == 1))
        return ret;
    ret = 1;

    if (!X509_NAME_add_entry_by_NID(n, nid, chtype,
                                    (unsigned char *)buf, -1, -1, mval))
        ret = 0;

    return ret;
}

static int add_attribute_object(X509_REQ *req, char *text, const char *def,
                                char *value, int nid, int n_min,
                                int n_max, unsigned long chtype)
{
    int ret = 0;
    char buf[1024];

    ret = build_data(text, def, value, n_min, n_max, buf, sizeof(buf),
                     "Attribute value", "Attribute default");
    if ((ret == 0) || (ret == 1))
        return ret;
    ret = 1;

    if (!X509_REQ_add1_attr_by_NID(req, nid, chtype,
                                   (unsigned char *)buf, -1)) {
        BIO_printf(bio_err, "Error adding attribute\n");
        ret = 0;
    }

    return ret;
}

static int build_data(char *text, const char *def, char *value,
                      int n_min, int n_max, char *buf, const int buf_size,
                      const char *desc1, const char *desc2)
{
    int i;
 start:
    if (!batch)
        BIO_printf(bio_err, "%s [%s]:", text, def);
    (void)BIO_flush(bio_err);
    if (value != NULL) {
        if (!join(buf, buf_size, value, "\n", desc1))
            return 0;
        BIO_printf(bio_err, "%s\n", value);
    } else {
        buf[0] = '\0';
        if (!batch) {
            if (!fgets(buf, buf_size, stdin))
                return 0;
        } else {
            buf[0] = '\n';
            buf[1] = '\0';
        }
    }

    if (buf[0] == '\0')
        return 0;
    if (buf[0] == '\n') {
        if ((def == NULL) || (def[0] == '\0'))
            return 1;
        if (!join(buf, buf_size, def, "\n", desc2))
            return 0;
    } else if ((buf[0] == '.') && (buf[1] == '\n')) {
        return 1;
    }

    i = strlen(buf);
    if (buf[i - 1] != '\n') {
        BIO_printf(bio_err, "Missing newline at end of input\n");
        return 0;
    }
    buf[--i] = '\0';
#ifdef CHARSET_EBCDIC
    ebcdic2ascii(buf, buf, i);
#endif
    if (!req_check_len(i, n_min, n_max)) {
        if (batch || value)
            return 0;
        goto start;
    }
    return 2;
}

static int req_check_len(int len, int n_min, int n_max)
{
    if (n_min > 0 && len < n_min) {
        BIO_printf(bio_err,
                   "String too short, must be at least %d bytes long\n", n_min);
        return 0;
    }
    if (n_max >= 0 && len > n_max) {
        BIO_printf(bio_err,
                   "String too long, must be at most %d bytes long\n", n_max);
        return 0;
    }
    return 1;
}

/* Check if the end of a string matches 'end' */
static int check_end(const char *str, const char *end)
{
    size_t elen, slen;
    const char *tmp;

    elen = strlen(end);
    slen = strlen(str);
    if (elen > slen)
        return 1;
    tmp = str + slen - elen;
    return strcmp(tmp, end);
}

/*
 * Merge the two strings together into the result buffer checking for
 * overflow and producing an error message if there is.
 */
static int join(char buf[], size_t buf_size, const char *name,
                const char *tail, const char *desc)
{
    const size_t name_len = strlen(name), tail_len = strlen(tail);

    if (name_len + tail_len + 1 > buf_size) {
        BIO_printf(bio_err, "%s '%s' too long\n", desc, name);
        return 0;
    }
    memcpy(buf, name, name_len);
    memcpy(buf + name_len, tail, tail_len + 1);
    return 1;
}

static EVP_PKEY_CTX *set_keygen_ctx(const char *gstr,
                                    char **pkeytype, long *pkeylen,
                                    ENGINE *keygen_engine)
{
    EVP_PKEY_CTX *gctx = NULL;
    EVP_PKEY *param = NULL;
    long keylen = -1;
    BIO *pbio = NULL;
    const char *keytype = NULL;
    size_t keytypelen = 0;
    int expect_paramfile = 0;
    const char *paramfile = NULL;

    /* Treat the first part of gstr, and only that */
    if (gstr == NULL) {
        /*
         * Special case: when no string given, default to RSA and the
         * key length given by |*pkeylen|.
         */
        keytype = "RSA";
        keylen = *pkeylen;
    } else if (gstr[0] >= '0' && gstr[0] <= '9') {
        /* Special case: only keylength given from string, so default to RSA */
        keytype = "RSA";
        /* The second part treatment will do the rest */
    } else {
        const char *p = strchr(gstr, ':');
        int len;

        if (p != NULL)
            len = p - gstr;
        else
            len = strlen(gstr);

        if (strncmp(gstr, "param", len) == 0) {
            expect_paramfile = 1;
            if (p == NULL) {
                BIO_printf(bio_err,
                           "Parameter file requested but no path given: %s\n",
                           gstr);
                return NULL;
            }
        } else {
            keytype = gstr;
            keytypelen = len;
        }

        if (p != NULL)
            gstr = gstr + len + 1;
        else
            gstr = NULL;
    }

    /* Treat the second part of gstr, if there is one */
    if (gstr != NULL) {
        /* If the second part starts with a digit, we assume it's a size */
        if (!expect_paramfile && gstr[0] >= '0' && gstr[0] <= '9')
            keylen = atol(gstr);
        else
            paramfile = gstr;
    }

    if (paramfile != NULL) {
        pbio = BIO_new_file(paramfile, "r");
        if (pbio == NULL) {
            BIO_printf(bio_err, "Cannot open parameter file %s\n", paramfile);
            return NULL;
        }
        param = PEM_read_bio_Parameters(pbio, NULL);

        if (param == NULL) {
            X509 *x;

            (void)BIO_reset(pbio);
            x = PEM_read_bio_X509(pbio, NULL, NULL, NULL);
            if (x != NULL) {
                param = X509_get_pubkey(x);
                X509_free(x);
            }
        }

        BIO_free(pbio);

        if (param == NULL) {
            BIO_printf(bio_err, "Error reading parameter file %s\n", paramfile);
            return NULL;
        }
        if (keytype == NULL) {
            keytype = EVP_PKEY_get0_type_name(param);
            if (keytype == NULL) {
                EVP_PKEY_free(param);
                BIO_puts(bio_err, "Unable to determine key type\n");
                return NULL;
            }
        }
    }

    if (keytypelen > 0)
        *pkeytype = OPENSSL_strndup(keytype, keytypelen);
    else
        *pkeytype = OPENSSL_strdup(keytype);

    if (*pkeytype == NULL) {
        BIO_printf(bio_err, "Out of memory\n");
        EVP_PKEY_free(param);
        return NULL;
    }

    if (keylen >= 0)
        *pkeylen = keylen;

    if (param != NULL) {
        if (!EVP_PKEY_is_a(param, *pkeytype)) {
            BIO_printf(bio_err, "Key type does not match parameters\n");
            EVP_PKEY_free(param);
            return NULL;
        }

        if (keygen_engine != NULL)
            gctx = EVP_PKEY_CTX_new(param, keygen_engine);
        else
            gctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(),
                                              param, app_get0_propq());
        *pkeylen = EVP_PKEY_get_bits(param);
        EVP_PKEY_free(param);
    } else {
        if (keygen_engine != NULL) {
            int pkey_id = get_legacy_pkey_id(app_get0_libctx(), *pkeytype,
                                             keygen_engine);

            if (pkey_id != NID_undef)
                gctx = EVP_PKEY_CTX_new_id(pkey_id, keygen_engine);
        } else {
            gctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(),
                                              *pkeytype, app_get0_propq());
        }
    }

    if (gctx == NULL) {
        BIO_puts(bio_err, "Error allocating keygen context\n");
        return NULL;
    }

    if (EVP_PKEY_keygen_init(gctx) <= 0) {
        BIO_puts(bio_err, "Error initializing keygen context\n");
        EVP_PKEY_CTX_free(gctx);
        return NULL;
    }
    if (keylen == -1 && (EVP_PKEY_CTX_is_a(gctx, "RSA")
        || EVP_PKEY_CTX_is_a(gctx, "RSA-PSS")))
        keylen = *pkeylen;

    if (keylen != -1) {
        OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END };
        size_t bits = keylen;

        params[0] =
            OSSL_PARAM_construct_size_t(OSSL_PKEY_PARAM_BITS, &bits);
        if (EVP_PKEY_CTX_set_params(gctx, params) <= 0) {
            BIO_puts(bio_err, "Error setting keysize\n");
            EVP_PKEY_CTX_free(gctx);
            return NULL;
        }
    }

    return gctx;
}

