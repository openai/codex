/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "internal/e_os.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <sys/types.h>
#include <openssl/conf.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/txt_db.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/objects.h>
#include <openssl/ocsp.h>
#include <openssl/pem.h>

#ifndef W_OK
# ifdef OPENSSL_SYS_VMS
#  include <unistd.h>
# elif !defined(OPENSSL_SYS_VXWORKS) && !defined(OPENSSL_SYS_WINDOWS) && !defined(OPENSSL_SYS_TANDEM)
#  include <sys/file.h>
# endif
#endif

#include "apps.h"
#include "progs.h"

#ifndef W_OK
# define F_OK 0
# define W_OK 2
# define R_OK 4
#endif

#ifndef PATH_MAX
# define PATH_MAX 4096
#endif

#define BASE_SECTION            "ca"

#define ENV_DEFAULT_CA          "default_ca"

#define STRING_MASK             "string_mask"
#define UTF8_IN                 "utf8"

#define ENV_NEW_CERTS_DIR       "new_certs_dir"
#define ENV_CERTIFICATE         "certificate"
#define ENV_SERIAL              "serial"
#define ENV_RAND_SERIAL         "rand_serial"
#define ENV_CRLNUMBER           "crlnumber"
#define ENV_PRIVATE_KEY         "private_key"
#define ENV_DEFAULT_DAYS        "default_days"
#define ENV_DEFAULT_STARTDATE   "default_startdate"
#define ENV_DEFAULT_ENDDATE     "default_enddate"
#define ENV_DEFAULT_CRL_DAYS    "default_crl_days"
#define ENV_DEFAULT_CRL_HOURS   "default_crl_hours"
#define ENV_DEFAULT_MD          "default_md"
#define ENV_DEFAULT_EMAIL_DN    "email_in_dn"
#define ENV_PRESERVE            "preserve"
#define ENV_POLICY              "policy"
#define ENV_EXTENSIONS          "x509_extensions"
#define ENV_CRLEXT              "crl_extensions"
#define ENV_MSIE_HACK           "msie_hack"
#define ENV_NAMEOPT             "name_opt"
#define ENV_CERTOPT             "cert_opt"
#define ENV_EXTCOPY             "copy_extensions"
#define ENV_UNIQUE_SUBJECT      "unique_subject"

#define ENV_DATABASE            "database"

/* Additional revocation information types */
typedef enum {
    REV_VALID             = -1, /* Valid (not-revoked) status */
    REV_NONE              = 0, /* No additional information */
    REV_CRL_REASON        = 1, /* Value is CRL reason code */
    REV_HOLD              = 2, /* Value is hold instruction */
    REV_KEY_COMPROMISE    = 3, /* Value is cert key compromise time */
    REV_CA_COMPROMISE     = 4  /* Value is CA key compromise time */
} REVINFO_TYPE;

static char *lookup_conf(const CONF *conf, const char *group, const char *tag);

static int certify(X509 **xret, const char *infile, int informat,
                   EVP_PKEY *pkey, X509 *x509,
                   const char *dgst,
                   STACK_OF(OPENSSL_STRING) *sigopts,
                   STACK_OF(OPENSSL_STRING) *vfyopts,
                   STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                   BIGNUM *serial, const char *subj, unsigned long chtype,
                   int multirdn, int email_dn, const char *startdate,
                   const char *enddate,
                   long days, int batch, const char *ext_sect, CONF *conf,
                   int verbose, unsigned long certopt, unsigned long nameopt,
                   int default_op, int ext_copy, int selfsign, unsigned long dateopt);
static int certify_cert(X509 **xret, const char *infile, int certformat,
                        const char *passin, EVP_PKEY *pkey, X509 *x509,
                        const char *dgst,
                        STACK_OF(OPENSSL_STRING) *sigopts,
                        STACK_OF(OPENSSL_STRING) *vfyopts,
                        STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                        BIGNUM *serial, const char *subj, unsigned long chtype,
                        int multirdn, int email_dn, const char *startdate,
                        const char *enddate, long days, int batch, const char *ext_sect,
                        CONF *conf, int verbose, unsigned long certopt,
                        unsigned long nameopt, int default_op, int ext_copy, unsigned long dateopt);
static int certify_spkac(X509 **xret, const char *infile, EVP_PKEY *pkey,
                         X509 *x509, const char *dgst,
                         STACK_OF(OPENSSL_STRING) *sigopts,
                         STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                         BIGNUM *serial, const char *subj, unsigned long chtype,
                         int multirdn, int email_dn, const char *startdate,
                         const char *enddate, long days, const char *ext_sect, CONF *conf,
                         int verbose, unsigned long certopt,
                         unsigned long nameopt, int default_op, int ext_copy, unsigned long dateopt);
static int do_body(X509 **xret, EVP_PKEY *pkey, X509 *x509,
                   const char *dgst, STACK_OF(OPENSSL_STRING) *sigopts,
                   STACK_OF(CONF_VALUE) *policy, CA_DB *db, BIGNUM *serial,
                   const char *subj, unsigned long chtype, int multirdn,
                   int email_dn, const char *startdate, const char *enddate, long days,
                   int batch, int verbose, X509_REQ *req, const char *ext_sect,
                   CONF *conf, unsigned long certopt, unsigned long nameopt,
                   int default_op, int ext_copy, int selfsign, unsigned long dateopt);
static int get_certificate_status(const char *ser_status, CA_DB *db);
static int check_time_format(const char *str);
static int do_revoke(X509 *x509, CA_DB *db, REVINFO_TYPE rev_type,
                     const char *extval);
static char *make_revocation_str(REVINFO_TYPE rev_type, const char *rev_arg);
static int make_revoked(X509_REVOKED *rev, const char *str);
static int old_entry_print(const ASN1_OBJECT *obj, const ASN1_STRING *str);
static void write_new_certificate(BIO *bp, X509 *x, int output_der, int notext);

static CONF *extfile_conf = NULL;
static int preserve = 0;
static int msie_hack = 0;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ENGINE, OPT_VERBOSE, OPT_CONFIG, OPT_NAME, OPT_SUBJ, OPT_UTF8,
    OPT_CREATE_SERIAL, OPT_MULTIVALUE_RDN, OPT_STARTDATE, OPT_ENDDATE,
    OPT_DAYS, OPT_MD, OPT_POLICY, OPT_KEYFILE, OPT_KEYFORM, OPT_PASSIN,
    OPT_KEY, OPT_CERT, OPT_CERTFORM, OPT_SELFSIGN,
    OPT_IN, OPT_INFORM, OPT_OUT, OPT_DATEOPT, OPT_OUTDIR, OPT_VFYOPT,
    OPT_SIGOPT, OPT_NOTEXT, OPT_BATCH, OPT_PRESERVEDN, OPT_NOEMAILDN,
    OPT_GENCRL, OPT_MSIE_HACK, OPT_CRL_LASTUPDATE, OPT_CRL_NEXTUPDATE,
    OPT_CRLDAYS, OPT_CRLHOURS, OPT_CRLSEC, OPT_NOT_BEFORE, OPT_NOT_AFTER,
    OPT_INFILES, OPT_SS_CERT, OPT_SPKAC, OPT_REVOKE, OPT_VALID,
    OPT_EXTENSIONS, OPT_EXTFILE, OPT_STATUS, OPT_UPDATEDB, OPT_CRLEXTS,
    OPT_RAND_SERIAL, OPT_QUIET,
    OPT_R_ENUM, OPT_PROV_ENUM,
    /* Do not change the order here; see related case statements below */
    OPT_CRL_REASON, OPT_CRL_HOLD, OPT_CRL_COMPROMISE, OPT_CRL_CA_COMPROMISE
} OPTION_CHOICE;

const OPTIONS ca_options[] = {
    {OPT_HELP_STR, 1, '-', "Usage: %s [options] [certreq...]\n"},

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"verbose", OPT_VERBOSE, '-', "Verbose output during processing"},
    {"quiet", OPT_QUIET, '-', "Terse output during processing"},
    {"outdir", OPT_OUTDIR, '/', "Where to put output cert"},
    {"in", OPT_IN, '<', "The input cert request(s)"},
    {"inform", OPT_INFORM, 'F',
     "CSR input format to use (PEM or DER; by default try PEM first)"},
    {"infiles", OPT_INFILES, '-', "The last argument, requests to process"},
    {"out", OPT_OUT, '>', "Where to put the output file(s)"},
    {"dateopt", OPT_DATEOPT, 's', "Datetime format used for printing. (rfc_822/iso_8601). Default is rfc_822."},
    {"notext", OPT_NOTEXT, '-', "Do not print the generated certificate"},
    {"batch", OPT_BATCH, '-', "Don't ask questions"},
    {"msie_hack", OPT_MSIE_HACK, '-',
     "msie modifications to handle all Universal Strings"},
    {"ss_cert", OPT_SS_CERT, '<', "File contains a self signed cert to sign"},
    {"spkac", OPT_SPKAC, '<',
     "File contains DN and signed public key and challenge"},
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif

    OPT_SECTION("Configuration"),
    {"config", OPT_CONFIG, 's', "A config file"},
    {"name", OPT_NAME, 's', "The particular CA definition to use"},
    {"section", OPT_NAME, 's', "An alias for -name"},
    {"policy", OPT_POLICY, 's', "The CA 'policy' to support"},

    OPT_SECTION("Certificate"),
    {"subj", OPT_SUBJ, 's', "Use arg instead of request's subject"},
    {"utf8", OPT_UTF8, '-', "Input characters are UTF8; default ASCII"},
    {"create_serial", OPT_CREATE_SERIAL, '-',
     "If reading serial fails, create a new random serial"},
    {"rand_serial", OPT_RAND_SERIAL, '-',
     "Always create a random serial; do not store it"},
    {"multivalue-rdn", OPT_MULTIVALUE_RDN, '-',
     "Deprecated; multi-valued RDNs support is always on."},
    {"startdate", OPT_STARTDATE, 's',
     "[CC]YYMMDDHHMMSSZ value for notBefore certificate field"},
    {"not_before", OPT_NOT_BEFORE, 's', "An alias for -startdate"},
    {"enddate", OPT_ENDDATE, 's',
     "[CC]YYMMDDHHMMSSZ value for notAfter certificate field, overrides -days"},
    {"not_after", OPT_NOT_AFTER, 's', "An alias for -enddate"},
    {"days", OPT_DAYS, 'p', "Number of days from today to certify the cert for"},
    {"extensions", OPT_EXTENSIONS, 's',
     "Extension section (override value in config file)"},
    {"extfile", OPT_EXTFILE, '<',
     "Configuration file with X509v3 extensions to add"},
    {"preserveDN", OPT_PRESERVEDN, '-', "Don't re-order the DN"},
    {"noemailDN", OPT_NOEMAILDN, '-', "Don't add the EMAIL field to the DN"},

    OPT_SECTION("Signing"),
    {"md", OPT_MD, 's', "Digest to use, such as sha256"},
    {"keyfile", OPT_KEYFILE, 's', "The CA private key"},
    {"keyform", OPT_KEYFORM, 'f',
     "Private key file format (ENGINE, other values ignored)"},
    {"passin", OPT_PASSIN, 's', "Key and cert input file pass phrase source"},
    {"key", OPT_KEY, 's',
     "Key to decrypt the private key or cert files if encrypted. Better use -passin"},
    {"cert", OPT_CERT, '<', "The CA cert"},
    {"certform", OPT_CERTFORM, 'F',
     "Certificate input format (DER/PEM/P12); has no effect"},
    {"selfsign", OPT_SELFSIGN, '-',
     "Sign a cert with the key associated with it"},
    {"sigopt", OPT_SIGOPT, 's', "Signature parameter in n:v form"},
    {"vfyopt", OPT_VFYOPT, 's', "Verification parameter in n:v form"},

    OPT_SECTION("Revocation"),
    {"gencrl", OPT_GENCRL, '-', "Generate a new CRL"},
    {"valid", OPT_VALID, 's',
     "Add a Valid(not-revoked) DB entry about a cert (given in file)"},
    {"status", OPT_STATUS, 's', "Shows cert status given the serial number"},
    {"updatedb", OPT_UPDATEDB, '-', "Updates db for expired cert"},
    {"crlexts", OPT_CRLEXTS, 's',
     "CRL extension section (override value in config file)"},
    {"crl_reason", OPT_CRL_REASON, 's', "revocation reason"},
    {"crl_hold", OPT_CRL_HOLD, 's',
     "the hold instruction, an OID. Sets revocation reason to certificateHold"},
    {"crl_compromise", OPT_CRL_COMPROMISE, 's',
     "sets compromise time to val and the revocation reason to keyCompromise"},
    {"crl_CA_compromise", OPT_CRL_CA_COMPROMISE, 's',
     "sets compromise time to val and the revocation reason to CACompromise"},
    {"crl_lastupdate", OPT_CRL_LASTUPDATE, 's',
     "Sets the CRL lastUpdate time to val (YYMMDDHHMMSSZ or YYYYMMDDHHMMSSZ)"},
    {"crl_nextupdate", OPT_CRL_NEXTUPDATE, 's',
     "Sets the CRL nextUpdate time to val (YYMMDDHHMMSSZ or YYYYMMDDHHMMSSZ)"},
    {"crldays", OPT_CRLDAYS, 'p', "Days until the next CRL is due"},
    {"crlhours", OPT_CRLHOURS, 'p', "Hours until the next CRL is due"},
    {"crlsec", OPT_CRLSEC, 'p', "Seconds until the next CRL is due"},
    {"revoke", OPT_REVOKE, '<', "Revoke a cert (given in file)"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"certreq", 0, 0, "Certificate requests to be signed (optional)"},
    {NULL}
};

int ca_main(int argc, char **argv)
{
    CONF *conf = NULL;
    ENGINE *e = NULL;
    BIGNUM *crlnumber = NULL, *serial = NULL;
    EVP_PKEY *pkey = NULL;
    BIO *in = NULL, *out = NULL, *Sout = NULL;
    ASN1_INTEGER *tmpser;
    CA_DB *db = NULL;
    DB_ATTR db_attr;
    STACK_OF(CONF_VALUE) *attribs = NULL;
    STACK_OF(OPENSSL_STRING) *sigopts = NULL, *vfyopts = NULL;
    STACK_OF(X509) *cert_sk = NULL;
    X509_CRL *crl = NULL;
    char *configfile = default_config_file, *section = NULL;
    char def_dgst[80] = "";
    char *dgst = NULL, *policy = NULL, *keyfile = NULL;
    char *certfile = NULL, *crl_ext = NULL, *crlnumberfile = NULL;
    int certformat = FORMAT_UNDEF, informat = FORMAT_UNDEF;
    unsigned long dateopt = ASN1_DTFLGS_RFC822;
    const char *infile = NULL, *spkac_file = NULL, *ss_cert_file = NULL;
    const char *extensions = NULL, *extfile = NULL, *passinarg = NULL;
    char *passin = NULL;
    char *outdir = NULL, *outfile = NULL, *rev_arg = NULL, *ser_status = NULL;
    const char *serialfile = NULL, *subj = NULL;
    char *prog, *startdate = NULL, *enddate = NULL;
    char *dbfile = NULL, *f;
    char new_cert[PATH_MAX];
    char tmp[10 + 1] = "\0";
    char *const *pp;
    const char *p;
    size_t outdirlen = 0;
    int create_ser = 0, free_passin = 0, total = 0, total_done = 0;
    int batch = 0, default_op = 1, doupdatedb = 0, ext_copy = EXT_COPY_NONE;
    int keyformat = FORMAT_UNDEF, multirdn = 1, notext = 0, output_der = 0;
    int ret = 1, email_dn = 1, req = 0, verbose = 0, gencrl = 0, dorevoke = 0;
    int rand_ser = 0, i, j, selfsign = 0, def_ret;
    char *crl_lastupdate = NULL, *crl_nextupdate = NULL;
    long crldays = 0, crlhours = 0, crlsec = 0, days = 0;
    unsigned long chtype = MBSTRING_ASC, certopt = 0;
    X509 *x509 = NULL, *x509p = NULL, *x = NULL;
    REVINFO_TYPE rev_type = REV_NONE;
    X509_REVOKED *r = NULL;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, ca_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(ca_options);
            ret = 0;
            goto end;
        case OPT_IN:
            req = 1;
            infile = opt_arg();
            break;
        case OPT_INFORM:
            if (!opt_format(opt_arg(), OPT_FMT_PEMDER, &informat))
                goto opthelp;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_DATEOPT:
            if (!set_dateopt(&dateopt, opt_arg()))
                goto opthelp;
            break;
        case OPT_VERBOSE:
            verbose = 1;
            break;
        case OPT_QUIET:
            verbose = 0;
            break;
        case OPT_CONFIG:
            configfile = opt_arg();
            break;
        case OPT_NAME:
            section = opt_arg();
            break;
        case OPT_SUBJ:
            subj = opt_arg();
            /* preserve=1; */
            break;
        case OPT_UTF8:
            chtype = MBSTRING_UTF8;
            break;
        case OPT_RAND_SERIAL:
            rand_ser = 1;
            break;
        case OPT_CREATE_SERIAL:
            create_ser = 1;
            break;
        case OPT_MULTIVALUE_RDN:
            /* obsolete */
            break;
        case OPT_STARTDATE:
        case OPT_NOT_BEFORE:
            startdate = opt_arg();
            break;
        case OPT_ENDDATE:
        case OPT_NOT_AFTER:
            enddate = opt_arg();
            break;
        case OPT_DAYS:
            days = atoi(opt_arg());
            break;
        case OPT_MD:
            dgst = opt_arg();
            break;
        case OPT_POLICY:
            policy = opt_arg();
            break;
        case OPT_KEYFILE:
            keyfile = opt_arg();
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyformat))
                goto opthelp;
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_KEY:
            passin = opt_arg();
            break;
        case OPT_CERT:
            certfile = opt_arg();
            break;
        case OPT_CERTFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &certformat))
                goto opthelp;
            break;
        case OPT_SELFSIGN:
            selfsign = 1;
            break;
        case OPT_OUTDIR:
            outdir = opt_arg();
            break;
        case OPT_SIGOPT:
            if (sigopts == NULL)
                sigopts = sk_OPENSSL_STRING_new_null();
            if (sigopts == NULL || !sk_OPENSSL_STRING_push(sigopts, opt_arg()))
                goto end;
            break;
        case OPT_VFYOPT:
            if (vfyopts == NULL)
                vfyopts = sk_OPENSSL_STRING_new_null();
            if (vfyopts == NULL || !sk_OPENSSL_STRING_push(vfyopts, opt_arg()))
                goto end;
            break;
        case OPT_NOTEXT:
            notext = 1;
            break;
        case OPT_BATCH:
            batch = 1;
            break;
        case OPT_PRESERVEDN:
            preserve = 1;
            break;
        case OPT_NOEMAILDN:
            email_dn = 0;
            break;
        case OPT_GENCRL:
            gencrl = 1;
            break;
        case OPT_MSIE_HACK:
            msie_hack = 1;
            break;
        case OPT_CRL_LASTUPDATE:
            crl_lastupdate = opt_arg();
            break;
        case OPT_CRL_NEXTUPDATE:
            crl_nextupdate = opt_arg();
            break;
        case OPT_CRLDAYS:
            crldays = atol(opt_arg());
            break;
        case OPT_CRLHOURS:
            crlhours = atol(opt_arg());
            break;
        case OPT_CRLSEC:
            crlsec = atol(opt_arg());
            break;
        case OPT_INFILES:
            req = 1;
            goto end_of_options;
        case OPT_SS_CERT:
            ss_cert_file = opt_arg();
            req = 1;
            break;
        case OPT_SPKAC:
            spkac_file = opt_arg();
            req = 1;
            break;
        case OPT_REVOKE:
            infile = opt_arg();
            dorevoke = 1;
            break;
        case OPT_VALID:
            infile = opt_arg();
            dorevoke = 2;
            break;
        case OPT_EXTENSIONS:
            extensions = opt_arg();
            break;
        case OPT_EXTFILE:
            extfile = opt_arg();
            break;
        case OPT_STATUS:
            ser_status = opt_arg();
            break;
        case OPT_UPDATEDB:
            doupdatedb = 1;
            break;
        case OPT_CRLEXTS:
            crl_ext = opt_arg();
            break;
        case OPT_CRL_REASON:   /* := REV_CRL_REASON */
        case OPT_CRL_HOLD:
        case OPT_CRL_COMPROMISE:
        case OPT_CRL_CA_COMPROMISE:
            rev_arg = opt_arg();
            rev_type = (o - OPT_CRL_REASON) + REV_CRL_REASON;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        }
    }

end_of_options:
    /* Remaining args are files to certify. */
    argc = opt_num_rest();
    argv = opt_rest();

    if ((conf = app_load_config_verbose(configfile, 1)) == NULL)
        goto end;
    if (configfile != default_config_file && !app_load_modules(conf))
        goto end;

    /* Lets get the config section we are using */
    if (section == NULL
        && (section = lookup_conf(conf, BASE_SECTION, ENV_DEFAULT_CA)) == NULL)
        goto end;

    p = app_conf_try_string(conf, NULL, "oid_file");
    if (p != NULL) {
        BIO *oid_bio = BIO_new_file(p, "r");

        if (oid_bio == NULL) {
            ERR_clear_error();
        } else {
            OBJ_create_objects(oid_bio);
            BIO_free(oid_bio);
        }
    }
    if (!add_oid_section(conf))
        goto end;

    app_RAND_load_conf(conf, BASE_SECTION);
    if (!app_RAND_load())
        goto end;

    f = app_conf_try_string(conf, section, STRING_MASK);
    if (f != NULL && !ASN1_STRING_set_default_mask_asc(f)) {
        BIO_printf(bio_err, "Invalid global string mask setting %s\n", f);
        goto end;
    }

    if (chtype != MBSTRING_UTF8) {
        f = app_conf_try_string(conf, section, UTF8_IN);
        if (f != NULL && strcmp(f, "yes") == 0)
            chtype = MBSTRING_UTF8;
    }

    db_attr.unique_subject = 1;
    p = app_conf_try_string(conf, section, ENV_UNIQUE_SUBJECT);
    if (p != NULL)
        db_attr.unique_subject = parse_yesno(p, 1);

    /*****************************************************************/
    /* report status of cert with serial number given on command line */
    if (ser_status) {
        dbfile = lookup_conf(conf, section, ENV_DATABASE);
        if (dbfile == NULL)
            goto end;

        db = load_index(dbfile, &db_attr);
        if (db == NULL) {
            BIO_printf(bio_err, "Problem with index file: %s (could not load/parse file)\n", dbfile);
            goto end;
        }

        if (index_index(db) <= 0)
            goto end;

        if (get_certificate_status(ser_status, db) != 1)
            BIO_printf(bio_err, "Error verifying serial %s!\n", ser_status);
        goto end;
    }

    /*****************************************************************/
    /* we definitely need a private key, so let's get it */

    if (keyfile == NULL
        && (keyfile = lookup_conf(conf, section, ENV_PRIVATE_KEY)) == NULL)
        goto end;

    if (passin == NULL) {
        free_passin = 1;
        if (!app_passwd(passinarg, NULL, &passin, NULL)) {
            BIO_printf(bio_err, "Error getting password\n");
            goto end;
        }
    }
    pkey = load_key(keyfile, keyformat, 0, passin, e, "CA private key");
    cleanse(passin);
    if (pkey == NULL)
        /* load_key() has already printed an appropriate message */
        goto end;

    /*****************************************************************/
    /* we need a certificate */
    if (!selfsign || spkac_file || ss_cert_file || gencrl) {
        if (certfile == NULL
            && (certfile = lookup_conf(conf, section, ENV_CERTIFICATE)) == NULL)
            goto end;

        x509 = load_cert_pass(certfile, certformat, 1, passin, "CA certificate");
        if (x509 == NULL)
            goto end;

        if (!X509_check_private_key(x509, pkey)) {
            BIO_printf(bio_err,
                       "CA certificate and CA private key do not match\n");
            goto end;
        }
    }
    if (!selfsign)
        x509p = x509;

    f = app_conf_try_string(conf, BASE_SECTION, ENV_PRESERVE);
    if (f != NULL && (*f == 'y' || *f == 'Y'))
        preserve = 1;
    f = app_conf_try_string(conf, BASE_SECTION, ENV_MSIE_HACK);
    if (f != NULL && (*f == 'y' || *f == 'Y'))
        msie_hack = 1;

    f = app_conf_try_string(conf, section, ENV_NAMEOPT);
    if (f != NULL) {
        if (!set_nameopt(f)) {
            BIO_printf(bio_err, "Invalid name options: \"%s\"\n", f);
            goto end;
        }
        default_op = 0;
    }

    f = app_conf_try_string(conf, section, ENV_CERTOPT);
    if (f != NULL) {
        if (!set_cert_ex(&certopt, f)) {
            BIO_printf(bio_err, "Invalid certificate options: \"%s\"\n", f);
            goto end;
        }
        default_op = 0;
    }

    f = app_conf_try_string(conf, section, ENV_EXTCOPY);
    if (f != NULL) {
        if (!set_ext_copy(&ext_copy, f)) {
            BIO_printf(bio_err, "Invalid extension copy option: \"%s\"\n", f);
            goto end;
        }
    }

    /*****************************************************************/
    /* lookup where to write new certificates */
    if ((outdir == NULL) && (req)) {

        outdir = NCONF_get_string(conf, section, ENV_NEW_CERTS_DIR);
        if (outdir == NULL) {
            BIO_printf(bio_err,
                       "there needs to be defined a directory for new certificate to be placed in\n");
            goto end;
        }
#ifndef OPENSSL_SYS_VMS
        /*
         * outdir is a directory spec, but access() for VMS demands a
         * filename.  We could use the DEC C routine to convert the
         * directory syntax to Unix, and give that to app_isdir,
         * but for now the fopen will catch the error if it's not a
         * directory
         */
        if (app_isdir(outdir) <= 0) {
            BIO_printf(bio_err, "%s: %s is not a directory\n", prog, outdir);
            perror(outdir);
            goto end;
        }
#endif
    }

    /*****************************************************************/
    /* we need to load the database file */
    dbfile = lookup_conf(conf, section, ENV_DATABASE);
    if (dbfile == NULL)
        goto end;

    db = load_index(dbfile, &db_attr);
    if (db == NULL) {
        BIO_printf(bio_err, "Problem with index file: %s (could not load/parse file)\n", dbfile);
        goto end;
    }

    /* Lets check some fields */
    for (i = 0; i < sk_OPENSSL_PSTRING_num(db->db->data); i++) {
        pp = sk_OPENSSL_PSTRING_value(db->db->data, i);
        if ((pp[DB_type][0] != DB_TYPE_REV) && (pp[DB_rev_date][0] != '\0')) {
            BIO_printf(bio_err,
                       "entry %d: not revoked yet, but has a revocation date\n",
                       i + 1);
            goto end;
        }
        if ((pp[DB_type][0] == DB_TYPE_REV) &&
            !make_revoked(NULL, pp[DB_rev_date])) {
            BIO_printf(bio_err, " in entry %d\n", i + 1);
            goto end;
        }
        if (!check_time_format((char *)pp[DB_exp_date])) {
            BIO_printf(bio_err, "entry %d: invalid expiry date\n", i + 1);
            goto end;
        }
        p = pp[DB_serial];
        j = strlen(p);
        if (*p == '-') {
            p++;
            j--;
        }
        if ((j & 1) || (j < 2)) {
            BIO_printf(bio_err, "entry %d: bad serial number length (%d)\n",
                       i + 1, j);
            goto end;
        }
        for ( ; *p; p++) {
            if (!isxdigit(_UC(*p))) {
                BIO_printf(bio_err,
                           "entry %d: bad char 0%o '%c' in serial number\n",
                           i + 1, *p, *p);
                goto end;
            }
        }
    }
    if (verbose) {
        TXT_DB_write(bio_out, db->db);
        BIO_printf(bio_err, "%d entries loaded from the database\n",
                   sk_OPENSSL_PSTRING_num(db->db->data));
        BIO_printf(bio_err, "generating index\n");
    }

    if (index_index(db) <= 0)
        goto end;

    /*****************************************************************/
    /* Update the db file for expired certificates */
    if (doupdatedb) {
        if (verbose)
            BIO_printf(bio_err, "Updating %s ...\n", dbfile);

        i = do_updatedb(db, NULL);
        if (i == -1) {
            BIO_printf(bio_err, "Malloc failure\n");
            goto end;
        } else if (i == 0) {
            if (verbose)
                BIO_printf(bio_err, "No entries found to mark expired\n");
        } else {
            if (!save_index(dbfile, "new", db))
                goto end;

            if (!rotate_index(dbfile, "new", "old"))
                goto end;

            if (verbose)
                BIO_printf(bio_err, "Done. %d entries marked as expired\n", i);
        }
    }

    /*****************************************************************/
    /* Read extensions config file                                   */
    if (extfile) {
        if ((extfile_conf = app_load_config(extfile)) == NULL) {
            ret = 1;
            goto end;
        }

        if (verbose)
            BIO_printf(bio_err, "Successfully loaded extensions file %s\n",
                       extfile);

        /* We can have sections in the ext file */
        if (extensions == NULL) {
            extensions =
                app_conf_try_string(extfile_conf, "default", "extensions");
            if (extensions == NULL)
                extensions = "default";
        }
    }

    /*****************************************************************/
    if (req || gencrl) {
        if (spkac_file != NULL && outfile != NULL) {
            output_der = 1;
            batch = 1;
        }
    }

    def_ret = EVP_PKEY_get_default_digest_name(pkey, def_dgst, sizeof(def_dgst));
    /*
     * EVP_PKEY_get_default_digest_name() returns 2 if the digest is
     * mandatory for this algorithm.
     *
     * That call may give back the name "UNDEF", which has these meanings:
     *
     * when def_ret == 2: the user MUST leave the digest unspecified
     * when def_ret == 1: the user MAY leave the digest unspecified
     */
    if (def_ret == 2 && strcmp(def_dgst, "UNDEF") == 0) {
        dgst = NULL;
    } else if (dgst == NULL
               && (dgst = lookup_conf(conf, section, ENV_DEFAULT_MD)) == NULL
               && strcmp(def_dgst, "UNDEF") != 0) {
        goto end;
    } else {
        if (strcmp(dgst, "default") == 0 || strcmp(def_dgst, "UNDEF") == 0) {
            if (def_ret <= 0) {
                BIO_puts(bio_err, "no default digest\n");
                goto end;
            }
            dgst = def_dgst;
        }
    }

    if (req) {
        if (email_dn == 1) {
            char *tmp_email_dn = NULL;

            tmp_email_dn =
                app_conf_try_string(conf, section, ENV_DEFAULT_EMAIL_DN);
            if (tmp_email_dn != NULL && strcmp(tmp_email_dn, "no") == 0)
                email_dn = 0;
        }
        if (verbose)
            BIO_printf(bio_err, "message digest is %s\n", dgst);
        if (policy == NULL
            && (policy = lookup_conf(conf, section, ENV_POLICY)) == NULL)
            goto end;

        if (verbose)
            BIO_printf(bio_err, "policy is %s\n", policy);

        if (app_conf_try_string(conf, section, ENV_RAND_SERIAL) != NULL) {
            rand_ser = 1;
        } else {
            serialfile = lookup_conf(conf, section, ENV_SERIAL);
            if (serialfile == NULL)
                goto end;
        }

        if (extfile_conf != NULL) {
            /* Check syntax of extfile */
            X509V3_CTX ctx;

            X509V3_set_ctx_test(&ctx);
            X509V3_set_nconf(&ctx, extfile_conf);
            if (!X509V3_EXT_add_nconf(extfile_conf, &ctx, extensions, NULL)) {
                BIO_printf(bio_err,
                           "Error checking certificate extensions from extfile section %s\n",
                           extensions);
                ret = 1;
                goto end;
            }
        } else {
            /*
             * no '-extfile' option, so we look for extensions in the main
             * configuration file
             */
            if (extensions == NULL)
                extensions = app_conf_try_string(conf, section, ENV_EXTENSIONS);
            if (extensions != NULL) {
                /* Check syntax of config file section */
                X509V3_CTX ctx;

                X509V3_set_ctx_test(&ctx);
                X509V3_set_nconf(&ctx, conf);
                if (!X509V3_EXT_add_nconf(conf, &ctx, extensions, NULL)) {
                    BIO_printf(bio_err,
                               "Error checking certificate extension config section %s\n",
                               extensions);
                    ret = 1;
                    goto end;
                }
            }
        }

        if (startdate == NULL)
            startdate =
                app_conf_try_string(conf, section, ENV_DEFAULT_STARTDATE);
        if (enddate == NULL)
            enddate = app_conf_try_string(conf, section, ENV_DEFAULT_ENDDATE);
        if (days == 0) {
            if (!app_conf_try_number(conf, section, ENV_DEFAULT_DAYS, &days))
                days = 0;
        }
        if (enddate == NULL && days == 0) {
            BIO_printf(bio_err, "cannot lookup how many days to certify for\n");
            goto end;
        }
        if (days != 0 && enddate != NULL)
            BIO_printf(bio_err,
                       "Warning: -enddate or -not_after option overriding -days option\n");

        if (rand_ser) {
            if ((serial = BN_new()) == NULL || !rand_serial(serial, NULL)) {
                BIO_printf(bio_err, "error generating serial number\n");
                goto end;
            }
        } else {
            serial = load_serial(serialfile, NULL, create_ser, NULL);
            if (serial == NULL) {
                BIO_printf(bio_err, "error while loading serial number\n");
                goto end;
            }
            if (verbose) {
                if (BN_is_zero(serial)) {
                    BIO_printf(bio_err, "next serial number is 00\n");
                } else {
                    if ((f = BN_bn2hex(serial)) == NULL)
                        goto end;
                    BIO_printf(bio_err, "next serial number is %s\n", f);
                    OPENSSL_free(f);
                }
            }
        }

        if ((attribs = NCONF_get_section(conf, policy)) == NULL) {
            BIO_printf(bio_err, "unable to find 'section' for %s\n", policy);
            goto end;
        }

        if ((cert_sk = sk_X509_new_null()) == NULL) {
            BIO_printf(bio_err, "Memory allocation failure\n");
            goto end;
        }
        if (spkac_file != NULL) {
            total++;
            j = certify_spkac(&x, spkac_file, pkey, x509, dgst, sigopts,
                              attribs, db, serial, subj, chtype, multirdn,
                              email_dn, startdate, enddate, days, extensions,
                              conf, verbose, certopt, get_nameopt(), default_op,
                              ext_copy, dateopt);
            if (j < 0)
                goto end;
            if (j > 0) {
                total_done++;
                BIO_printf(bio_err, "\n");
                if (!BN_add_word(serial, 1))
                    goto end;
                if (!sk_X509_push(cert_sk, x)) {
                    BIO_printf(bio_err, "Memory allocation failure\n");
                    goto end;
                }
            }
        }
        if (ss_cert_file != NULL) {
            total++;
            j = certify_cert(&x, ss_cert_file, certformat, passin, pkey,
                             x509, dgst, sigopts, vfyopts, attribs,
                             db, serial, subj, chtype, multirdn, email_dn,
                             startdate, enddate, days, batch, extensions,
                             conf, verbose, certopt, get_nameopt(), default_op,
                             ext_copy, dateopt);
            if (j < 0)
                goto end;
            if (j > 0) {
                total_done++;
                BIO_printf(bio_err, "\n");
                if (!BN_add_word(serial, 1))
                    goto end;
                if (!sk_X509_push(cert_sk, x)) {
                    BIO_printf(bio_err, "Memory allocation failure\n");
                    goto end;
                }
            }
        }
        if (infile != NULL) {
            total++;
            j = certify(&x, infile, informat, pkey, x509p, dgst,
                        sigopts, vfyopts, attribs, db,
                        serial, subj, chtype, multirdn, email_dn, startdate,
                        enddate, days, batch, extensions, conf, verbose,
                        certopt, get_nameopt(), default_op, ext_copy, selfsign, dateopt);
            if (j < 0)
                goto end;
            if (j > 0) {
                total_done++;
                BIO_printf(bio_err, "\n");
                if (!BN_add_word(serial, 1))
                    goto end;
                if (!sk_X509_push(cert_sk, x)) {
                    BIO_printf(bio_err, "Memory allocation failure\n");
                    goto end;
                }
            }
        }
        for (i = 0; i < argc; i++) {
            total++;
            j = certify(&x, argv[i], informat, pkey, x509p, dgst,
                        sigopts, vfyopts,
                        attribs, db,
                        serial, subj, chtype, multirdn, email_dn, startdate,
                        enddate, days, batch, extensions, conf, verbose,
                        certopt, get_nameopt(), default_op, ext_copy, selfsign, dateopt);
            if (j < 0)
                goto end;
            if (j > 0) {
                total_done++;
                BIO_printf(bio_err, "\n");
                if (!BN_add_word(serial, 1)) {
                    X509_free(x);
                    goto end;
                }
                if (!sk_X509_push(cert_sk, x)) {
                    BIO_printf(bio_err, "Memory allocation failure\n");
                    X509_free(x);
                    goto end;
                }
            }
        }
        /*
         * we have a stack of newly certified certificates and a database
         * and serial number that need updating
         */

        if (sk_X509_num(cert_sk) > 0) {
            if (!batch) {
                BIO_printf(bio_err,
                           "\n%d out of %d certificate requests certified, commit? [y/n]",
                           total_done, total);
                (void)BIO_flush(bio_err);
                tmp[0] = '\0';
                if (fgets(tmp, sizeof(tmp), stdin) == NULL) {
                    BIO_printf(bio_err, "CERTIFICATION CANCELED: I/O error\n");
                    ret = 0;
                    goto end;
                }
                if (tmp[0] != 'y' && tmp[0] != 'Y') {
                    BIO_printf(bio_err, "CERTIFICATION CANCELED\n");
                    ret = 0;
                    goto end;
                }
            }

            BIO_printf(bio_err, "Write out database with %d new entries\n",
                       sk_X509_num(cert_sk));

            if (serialfile != NULL
                    && !save_serial(serialfile, "new", serial, NULL))
                goto end;

            if (!save_index(dbfile, "new", db))
                goto end;
        }

        outdirlen = OPENSSL_strlcpy(new_cert, outdir, sizeof(new_cert));
#ifndef OPENSSL_SYS_VMS
        outdirlen = OPENSSL_strlcat(new_cert, "/", sizeof(new_cert));
#endif

        if (verbose)
            BIO_printf(bio_err, "writing new certificates\n");

        for (i = 0; i < sk_X509_num(cert_sk); i++) {
            BIO *Cout = NULL;
            X509 *xi = sk_X509_value(cert_sk, i);
            const ASN1_INTEGER *serialNumber = X509_get0_serialNumber(xi);
            const unsigned char *psn = ASN1_STRING_get0_data(serialNumber);
            const int snl = ASN1_STRING_length(serialNumber);
            const int filen_len = 2 * (snl > 0 ? snl : 1) + sizeof(".pem");
            char *n = new_cert + outdirlen;

            if (outdirlen + filen_len > PATH_MAX) {
                BIO_printf(bio_err, "certificate file name too long\n");
                goto end;
            }

            if (snl > 0) {
                static const char HEX_DIGITS[] = "0123456789ABCDEF";

                for (j = 0; j < snl; j++, psn++) {
                    *n++ = HEX_DIGITS[*psn >> 4];
                    *n++ = HEX_DIGITS[*psn & 0x0F];
                }
            } else {
                *(n++) = '0';
                *(n++) = '0';
            }
            *(n++) = '.';
            *(n++) = 'p';
            *(n++) = 'e';
            *(n++) = 'm';
            *n = '\0';          /* closing new_cert */
            if (verbose)
                BIO_printf(bio_err, "writing %s\n", new_cert);

            Sout = bio_open_default(outfile, 'w',
                                    output_der ? FORMAT_ASN1 : FORMAT_TEXT);
            if (Sout == NULL)
                goto end;

            Cout = BIO_new_file(new_cert, "w");
            if (Cout == NULL) {
                perror(new_cert);
                goto end;
            }
            write_new_certificate(Cout, xi, 0, notext);
            write_new_certificate(Sout, xi, output_der, notext);
            BIO_free_all(Cout);
            BIO_free_all(Sout);
            Sout = NULL;
        }

        if (sk_X509_num(cert_sk)) {
            /* Rename the database and the serial file */
            if (serialfile != NULL
                    && !rotate_serial(serialfile, "new", "old"))
                goto end;

            if (!rotate_index(dbfile, "new", "old"))
                goto end;

            BIO_printf(bio_err, "Database updated\n");
        }
    }

    /*****************************************************************/
    if (gencrl) {
        int crl_v2 = 0;

        if (crl_ext == NULL)
            crl_ext = app_conf_try_string(conf, section, ENV_CRLEXT);
        if (crl_ext != NULL) {
            /* Check syntax of file */
            X509V3_CTX ctx;

            X509V3_set_ctx_test(&ctx);
            X509V3_set_nconf(&ctx, conf);
            if (!X509V3_EXT_add_nconf(conf, &ctx, crl_ext, NULL)) {
                BIO_printf(bio_err,
                           "Error checking CRL extension section %s\n", crl_ext);
                ret = 1;
                goto end;
            }
        }

        crlnumberfile = app_conf_try_string(conf, section, ENV_CRLNUMBER);
        if (crlnumberfile != NULL) {
            if ((crlnumber = load_serial(crlnumberfile, NULL, 0, NULL))
                == NULL) {
                BIO_printf(bio_err, "error while loading CRL number\n");
                goto end;
            }
        }

        if (!crldays && !crlhours && !crlsec) {
            if (!app_conf_try_number(conf, section,
                                  ENV_DEFAULT_CRL_DAYS, &crldays))
                crldays = 0;
            if (!app_conf_try_number(conf, section,
                                  ENV_DEFAULT_CRL_HOURS, &crlhours))
                crlhours = 0;
        }
        if ((crl_nextupdate == NULL) &&
                (crldays == 0) && (crlhours == 0) && (crlsec == 0)) {
            BIO_printf(bio_err,
                       "cannot lookup how long until the next CRL is issued\n");
            goto end;
        }

        if (verbose)
            BIO_printf(bio_err, "making CRL\n");
        if ((crl = X509_CRL_new_ex(app_get0_libctx(), app_get0_propq())) == NULL)
            goto end;
        if (!X509_CRL_set_issuer_name(crl, X509_get_subject_name(x509)))
            goto end;

        if (!set_crl_lastupdate(crl, crl_lastupdate)) {
            BIO_puts(bio_err, "error setting CRL lastUpdate\n");
            ret = 1;
            goto end;
        }

        if (!set_crl_nextupdate(crl, crl_nextupdate,
                                crldays, crlhours, crlsec)) {
            BIO_puts(bio_err, "error setting CRL nextUpdate\n");
            ret = 1;
            goto end;
        }

        for (i = 0; i < sk_OPENSSL_PSTRING_num(db->db->data); i++) {
            pp = sk_OPENSSL_PSTRING_value(db->db->data, i);
            if (pp[DB_type][0] == DB_TYPE_REV) {
                if ((r = X509_REVOKED_new()) == NULL)
                    goto end;
                j = make_revoked(r, pp[DB_rev_date]);
                if (!j)
                    goto end;
                if (j == 2)
                    crl_v2 = 1;
                if (!BN_hex2bn(&serial, pp[DB_serial]))
                    goto end;
                tmpser = BN_to_ASN1_INTEGER(serial, NULL);
                BN_free(serial);
                serial = NULL;
                if (!tmpser)
                    goto end;
                X509_REVOKED_set_serialNumber(r, tmpser);
                ASN1_INTEGER_free(tmpser);
                X509_CRL_add0_revoked(crl, r);
            }
        }

        /*
         * sort the data so it will be written in serial number order
         */
        X509_CRL_sort(crl);

        /* we now have a CRL */
        if (verbose)
            BIO_printf(bio_err, "signing CRL\n");

        /* Add any extensions asked for */

        if (crl_ext != NULL || crlnumberfile != NULL) {
            X509V3_CTX crlctx;

            X509V3_set_ctx(&crlctx, x509, NULL, NULL, crl, 0);
            X509V3_set_nconf(&crlctx, conf);

            if (crl_ext != NULL)
                if (!X509V3_EXT_CRL_add_nconf(conf, &crlctx, crl_ext, crl)) {
                    BIO_printf(bio_err,
                               "Error adding CRL extensions from section %s\n", crl_ext);
                    goto end;
                }
            if (crlnumberfile != NULL) {
                tmpser = BN_to_ASN1_INTEGER(crlnumber, NULL);
                if (!tmpser)
                    goto end;
                X509_CRL_add1_ext_i2d(crl, NID_crl_number, tmpser, 0, 0);
                ASN1_INTEGER_free(tmpser);
                crl_v2 = 1;
                if (!BN_add_word(crlnumber, 1))
                    goto end;
            }
        }
        if (crl_ext != NULL || crl_v2) {
            if (!X509_CRL_set_version(crl, X509_CRL_VERSION_2))
                goto end;
        }

        /* we have a CRL number that need updating */
        if (crlnumberfile != NULL
                && !save_serial(crlnumberfile, "new", crlnumber, NULL))
            goto end;

        BN_free(crlnumber);
        crlnumber = NULL;

        if (!do_X509_CRL_sign(crl, pkey, dgst, sigopts))
            goto end;

        Sout = bio_open_default(outfile, 'w',
                                output_der ? FORMAT_ASN1 : FORMAT_TEXT);
        if (Sout == NULL)
            goto end;

        PEM_write_bio_X509_CRL(Sout, crl);

        /* Rename the crlnumber file */
        if (crlnumberfile != NULL
                && !rotate_serial(crlnumberfile, "new", "old"))
            goto end;

    }
    /*****************************************************************/
    if (dorevoke) {
        if (infile == NULL) {
            BIO_printf(bio_err, "no input files\n");
            goto end;
        } else {
            X509 *revcert;

            revcert = load_cert_pass(infile, informat, 1, passin,
                                     "certificate to be revoked");
            if (revcert == NULL)
                goto end;
            if (dorevoke == 2)
                rev_type = REV_VALID;
            j = do_revoke(revcert, db, rev_type, rev_arg);
            if (j <= 0)
                goto end;
            X509_free(revcert);

            if (!save_index(dbfile, "new", db))
                goto end;

            if (!rotate_index(dbfile, "new", "old"))
                goto end;

            BIO_printf(bio_err, "Database updated\n");
        }
    }
    ret = 0;

 end:
    if (ret)
        ERR_print_errors(bio_err);
    BIO_free_all(Sout);
    BIO_free_all(out);
    BIO_free_all(in);
    OSSL_STACK_OF_X509_free(cert_sk);

    cleanse(passin);
    if (free_passin)
        OPENSSL_free(passin);
    BN_free(serial);
    BN_free(crlnumber);
    free_index(db);
    sk_OPENSSL_STRING_free(sigopts);
    sk_OPENSSL_STRING_free(vfyopts);
    EVP_PKEY_free(pkey);
    X509_free(x509);
    X509_CRL_free(crl);
    NCONF_free(conf);
    NCONF_free(extfile_conf);
    release_engine(e);
    return ret;
}

static char *lookup_conf(const CONF *conf, const char *section, const char *tag)
{
    char *entry = NCONF_get_string(conf, section, tag);
    if (entry == NULL)
        BIO_printf(bio_err, "variable lookup failed for %s::%s\n", section, tag);
    return entry;
}

static int certify(X509 **xret, const char *infile, int informat,
                   EVP_PKEY *pkey, X509 *x509,
                   const char *dgst,
                   STACK_OF(OPENSSL_STRING) *sigopts,
                   STACK_OF(OPENSSL_STRING) *vfyopts,
                   STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                   BIGNUM *serial, const char *subj, unsigned long chtype,
                   int multirdn, int email_dn, const char *startdate,
                   const char *enddate,
                   long days, int batch, const char *ext_sect, CONF *lconf,
                   int verbose, unsigned long certopt, unsigned long nameopt,
                   int default_op, int ext_copy, int selfsign, unsigned long dateopt)
{
    X509_REQ *req = NULL;
    EVP_PKEY *pktmp = NULL;
    int ok = -1, i;

    req = load_csr_autofmt(infile, informat, vfyopts, "certificate request");
    if (req == NULL)
        goto end;
    if ((pktmp = X509_REQ_get0_pubkey(req)) == NULL) {
        BIO_printf(bio_err, "Error unpacking public key\n");
        goto end;
    }
    if (verbose)
        X509_REQ_print_ex(bio_err, req, nameopt, X509_FLAG_COMPAT);

    BIO_printf(bio_err, "Check that the request matches the signature\n");
    ok = 0;

    if (selfsign && !X509_REQ_check_private_key(req, pkey)) {
        BIO_printf(bio_err,
                   "Certificate request and CA private key do not match\n");
        goto end;
    }
    i = do_X509_REQ_verify(req, pktmp, vfyopts);
    if (i < 0) {
        BIO_printf(bio_err, "Signature verification problems...\n");
        goto end;
    }
    if (i == 0) {
        BIO_printf(bio_err,
                   "Signature did not match the certificate request\n");
        goto end;
    }
    BIO_printf(bio_err, "Signature ok\n");

    ok = do_body(xret, pkey, x509, dgst, sigopts, policy, db, serial, subj,
                 chtype, multirdn, email_dn, startdate, enddate, days, batch,
                 verbose, req, ext_sect, lconf, certopt, nameopt, default_op,
                 ext_copy, selfsign, dateopt);

 end:
    ERR_print_errors(bio_err);
    X509_REQ_free(req);
    return ok;
}

static int certify_cert(X509 **xret, const char *infile, int certformat,
                        const char *passin, EVP_PKEY *pkey, X509 *x509,
                        const char *dgst,
                        STACK_OF(OPENSSL_STRING) *sigopts,
                        STACK_OF(OPENSSL_STRING) *vfyopts,
                        STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                        BIGNUM *serial, const char *subj, unsigned long chtype,
                        int multirdn, int email_dn, const char *startdate,
                        const char *enddate, long days, int batch, const char *ext_sect,
                        CONF *lconf, int verbose, unsigned long certopt,
                        unsigned long nameopt, int default_op, int ext_copy, unsigned long dateopt)
{
    X509 *template_cert = NULL;
    X509_REQ *rreq = NULL;
    EVP_PKEY *pktmp = NULL;
    int ok = -1, i;

    if ((template_cert = load_cert_pass(infile, certformat, 1, passin,
                                        "template certificate")) == NULL)
        goto end;
    if (verbose)
        X509_print(bio_err, template_cert);

    BIO_printf(bio_err, "Check that the request matches the signature\n");

    if ((pktmp = X509_get0_pubkey(template_cert)) == NULL) {
        BIO_printf(bio_err, "error unpacking public key\n");
        goto end;
    }
    i = do_X509_verify(template_cert, pktmp, vfyopts);
    if (i < 0) {
        ok = 0;
        BIO_printf(bio_err, "Signature verification problems....\n");
        goto end;
    }
    if (i == 0) {
        ok = 0;
        BIO_printf(bio_err, "Signature did not match the certificate\n");
        goto end;
    } else {
        BIO_printf(bio_err, "Signature ok\n");
    }

    if ((rreq = X509_to_X509_REQ(template_cert, NULL, NULL)) == NULL)
        goto end;

    ok = do_body(xret, pkey, x509, dgst, sigopts, policy, db, serial, subj,
                 chtype, multirdn, email_dn, startdate, enddate, days, batch,
                 verbose, rreq, ext_sect, lconf, certopt, nameopt, default_op,
                 ext_copy, 0, dateopt);

 end:
    X509_REQ_free(rreq);
    X509_free(template_cert);
    return ok;
}

static int do_body(X509 **xret, EVP_PKEY *pkey, X509 *x509,
                   const char *dgst, STACK_OF(OPENSSL_STRING) *sigopts,
                   STACK_OF(CONF_VALUE) *policy, CA_DB *db, BIGNUM *serial,
                   const char *subj, unsigned long chtype, int multirdn,
                   int email_dn, const char *startdate, const char *enddate, long days,
                   int batch, int verbose, X509_REQ *req, const char *ext_sect,
                   CONF *lconf, unsigned long certopt, unsigned long nameopt,
                   int default_op, int ext_copy, int selfsign, unsigned long dateopt)
{
    const X509_NAME *name = NULL;
    X509_NAME *CAname = NULL, *subject = NULL;
    const ASN1_TIME *tm;
    ASN1_STRING *str, *str2;
    ASN1_OBJECT *obj;
    X509 *ret = NULL;
    X509_NAME_ENTRY *ne, *tne;
    EVP_PKEY *pktmp;
    int ok = -1, i, j, last, nid;
    const char *p;
    CONF_VALUE *cv;
    OPENSSL_STRING row[DB_NUMBER];
    OPENSSL_STRING *irow = NULL;
    OPENSSL_STRING *rrow = NULL;
    char buf[25];
    X509V3_CTX ext_ctx;

    for (i = 0; i < DB_NUMBER; i++)
        row[i] = NULL;

    if (subj) {
        X509_NAME *n = parse_name(subj, chtype, multirdn, "subject");

        if (!n)
            goto end;
        X509_REQ_set_subject_name(req, n);
        X509_NAME_free(n);
    }

    if (default_op)
        BIO_printf(bio_err, "The Subject's Distinguished Name is as follows\n");

    name = X509_REQ_get_subject_name(req);
    for (i = 0; i < X509_NAME_entry_count(name); i++) {
        ne = X509_NAME_get_entry(name, i);
        str = X509_NAME_ENTRY_get_data(ne);
        obj = X509_NAME_ENTRY_get_object(ne);
        nid = OBJ_obj2nid(obj);

        if (msie_hack) {
            /* assume all type should be strings */

            if (str->type == V_ASN1_UNIVERSALSTRING)
                ASN1_UNIVERSALSTRING_to_string(str);

            if (str->type == V_ASN1_IA5STRING && nid != NID_pkcs9_emailAddress)
                str->type = V_ASN1_T61STRING;

            if (nid == NID_pkcs9_emailAddress
                && str->type == V_ASN1_PRINTABLESTRING)
                str->type = V_ASN1_IA5STRING;
        }

        /* If no EMAIL is wanted in the subject */
        if (nid == NID_pkcs9_emailAddress && !email_dn)
            continue;

        /* check some things */
        if (nid == NID_pkcs9_emailAddress && str->type != V_ASN1_IA5STRING) {
            BIO_printf(bio_err,
                       "\nemailAddress type needs to be of type IA5STRING\n");
            goto end;
        }
        if (str->type != V_ASN1_BMPSTRING && str->type != V_ASN1_UTF8STRING) {
            j = ASN1_PRINTABLE_type(str->data, str->length);
            if ((j == V_ASN1_T61STRING && str->type != V_ASN1_T61STRING) ||
                (j == V_ASN1_IA5STRING && str->type == V_ASN1_PRINTABLESTRING))
            {
                BIO_printf(bio_err,
                           "\nThe string contains characters that are illegal for the ASN.1 type\n");
                goto end;
            }
        }

        if (default_op)
            old_entry_print(obj, str);
    }

    /* Ok, now we check the 'policy' stuff. */
    if ((subject = X509_NAME_new()) == NULL) {
        BIO_printf(bio_err, "Memory allocation failure\n");
        goto end;
    }

    /* take a copy of the issuer name before we mess with it. */
    if (selfsign)
        CAname = X509_NAME_dup(name);
    else
        CAname = X509_NAME_dup(X509_get_subject_name(x509));
    if (CAname == NULL)
        goto end;
    str = str2 = NULL;

    for (i = 0; i < sk_CONF_VALUE_num(policy); i++) {
        cv = sk_CONF_VALUE_value(policy, i); /* get the object id */
        if ((j = OBJ_txt2nid(cv->name)) == NID_undef) {
            BIO_printf(bio_err,
                       "%s:unknown object type in 'policy' configuration\n",
                       cv->name);
            goto end;
        }
        obj = OBJ_nid2obj(j);

        last = -1;
        for (;;) {
            X509_NAME_ENTRY *push = NULL;

            /* lookup the object in the supplied name list */
            j = X509_NAME_get_index_by_OBJ(name, obj, last);
            if (j < 0) {
                if (last != -1)
                    break;
                tne = NULL;
            } else {
                tne = X509_NAME_get_entry(name, j);
            }
            last = j;

            /* depending on the 'policy', decide what to do. */
            if (strcmp(cv->value, "optional") == 0) {
                if (tne != NULL)
                    push = tne;
            } else if (strcmp(cv->value, "supplied") == 0) {
                if (tne == NULL) {
                    BIO_printf(bio_err,
                               "The %s field needed to be supplied and was missing\n",
                               cv->name);
                    goto end;
                } else {
                    push = tne;
                }
            } else if (strcmp(cv->value, "match") == 0) {
                int last2;

                if (tne == NULL) {
                    BIO_printf(bio_err,
                               "The mandatory %s field was missing\n",
                               cv->name);
                    goto end;
                }

                last2 = -1;

 again2:
                j = X509_NAME_get_index_by_OBJ(CAname, obj, last2);
                if ((j < 0) && (last2 == -1)) {
                    BIO_printf(bio_err,
                               "The %s field does not exist in the CA certificate,\n"
                               "the 'policy' is misconfigured\n", cv->name);
                    goto end;
                }
                if (j >= 0) {
                    push = X509_NAME_get_entry(CAname, j);
                    str = X509_NAME_ENTRY_get_data(tne);
                    str2 = X509_NAME_ENTRY_get_data(push);
                    last2 = j;
                    if (ASN1_STRING_cmp(str, str2) != 0)
                        goto again2;
                }
                if (j < 0) {
                    BIO_printf(bio_err,
                               "The %s field is different between\n"
                               "CA certificate (%s) and the request (%s)\n",
                               cv->name,
                               ((str2 == NULL) ? "NULL" : (char *)str2->data),
                               ((str == NULL) ? "NULL" : (char *)str->data));
                    goto end;
                }
            } else {
                BIO_printf(bio_err,
                           "%s:invalid type in 'policy' configuration\n",
                           cv->value);
                goto end;
            }

            if (push != NULL) {
                if (!X509_NAME_add_entry(subject, push, -1, 0)) {
                    BIO_printf(bio_err, "Memory allocation failure\n");
                    goto end;
                }
            }
            if (j < 0)
                break;
        }
    }

    if (preserve) {
        X509_NAME_free(subject);
        /* subject=X509_NAME_dup(X509_REQ_get_subject_name(req)); */
        subject = X509_NAME_dup(name);
        if (subject == NULL)
            goto end;
    }

    /* We are now totally happy, lets make and sign the certificate */
    if (verbose)
        BIO_printf(bio_err,
                   "Everything appears to be ok, creating and signing the certificate\n");

    if ((ret = X509_new_ex(app_get0_libctx(), app_get0_propq())) == NULL)
        goto end;

    if (BN_to_ASN1_INTEGER(serial, X509_get_serialNumber(ret)) == NULL)
        goto end;
    if (selfsign) {
        if (!X509_set_issuer_name(ret, subject))
            goto end;
    } else {
        if (!X509_set_issuer_name(ret, X509_get_subject_name(x509)))
            goto end;
    }

    if (!set_cert_times(ret, startdate, enddate, days, 0))
        goto end;

    if (enddate != NULL) {
        int tdays;

        if (!ASN1_TIME_diff(&tdays, NULL, NULL, X509_get0_notAfter(ret)))
            goto end;
        days = tdays;
    }

    if (!X509_set_subject_name(ret, subject))
        goto end;

    pktmp = X509_REQ_get0_pubkey(req);
    i = X509_set_pubkey(ret, pktmp);
    if (!i)
        goto end;

    /* Initialize the context structure */
    X509V3_set_ctx(&ext_ctx, selfsign ? ret : x509,
                   ret, NULL /* no need to give req, needed info is in ret */,
                   NULL, X509V3_CTX_REPLACE);
    /* prepare fallback for AKID, but only if issuer cert equals subject cert */
    if (selfsign) {
        if (!X509V3_set_issuer_pkey(&ext_ctx, pkey))
            goto end;
        if (!cert_matches_key(ret, pkey))
            BIO_printf(bio_err,
                       "Warning: Signature key and public key of cert do not match\n");
    }

    /* Lets add the extensions, if there are any */
    if (ext_sect) {
        if (extfile_conf != NULL) {
            if (verbose)
                BIO_printf(bio_err, "Extra configuration file found\n");

            /* Use the extfile_conf configuration db LHASH */
            X509V3_set_nconf(&ext_ctx, extfile_conf);

            /* Adds exts contained in the configuration file */
            if (!X509V3_EXT_add_nconf(extfile_conf, &ext_ctx, ext_sect, ret)) {
                BIO_printf(bio_err,
                           "Error adding certificate extensions from extfile section %s\n",
                           ext_sect);
                goto end;
            }
            if (verbose)
                BIO_printf(bio_err,
                           "Successfully added extensions from file.\n");
        } else if (ext_sect) {
            /* We found extensions to be set from config file */
            X509V3_set_nconf(&ext_ctx, lconf);

            if (!X509V3_EXT_add_nconf(lconf, &ext_ctx, ext_sect, ret)) {
                BIO_printf(bio_err,
                           "Error adding certificate extensions from config section %s\n",
                           ext_sect);
                goto end;
            }

            if (verbose)
                BIO_printf(bio_err,
                           "Successfully added extensions from config\n");
        }
    }

    /* Copy extensions from request (if any) */

    if (!copy_extensions(ret, req, ext_copy)) {
        BIO_printf(bio_err, "ERROR: adding extensions from request\n");
        goto end;
    }

    if (verbose)
        BIO_printf(bio_err,
                   "The subject name appears to be ok, checking database for clashes\n");

    /* Build the correct Subject if no e-mail is wanted in the subject. */
    if (!email_dn) {
        X509_NAME_ENTRY *tmpne;
        X509_NAME *dn_subject;

        /*
         * Its best to dup the subject DN and then delete any email addresses
         * because this retains its structure.
         */
        if ((dn_subject = X509_NAME_dup(subject)) == NULL) {
            BIO_printf(bio_err, "Memory allocation failure\n");
            goto end;
        }
        i = -1;
        while ((i = X509_NAME_get_index_by_NID(dn_subject,
                                               NID_pkcs9_emailAddress,
                                               i)) >= 0) {
            tmpne = X509_NAME_delete_entry(dn_subject, i--);
            X509_NAME_ENTRY_free(tmpne);
        }

        if (!X509_set_subject_name(ret, dn_subject)) {
            X509_NAME_free(dn_subject);
            goto end;
        }
        X509_NAME_free(dn_subject);
    }

    row[DB_name] = X509_NAME_oneline(X509_get_subject_name(ret), NULL, 0);
    if (row[DB_name] == NULL) {
        BIO_printf(bio_err, "Memory allocation failure\n");
        goto end;
    }

    if (BN_is_zero(serial))
        row[DB_serial] = OPENSSL_strdup("00");
    else
        row[DB_serial] = BN_bn2hex(serial);
    if (row[DB_serial] == NULL) {
        BIO_printf(bio_err, "Memory allocation failure\n");
        goto end;
    }

    if (row[DB_name][0] == '\0') {
        /*
         * An empty subject! We'll use the serial number instead. If
         * unique_subject is in use then we don't want different entries with
         * empty subjects matching each other.
         */
        OPENSSL_free(row[DB_name]);
        row[DB_name] = OPENSSL_strdup(row[DB_serial]);
        if (row[DB_name] == NULL) {
            BIO_printf(bio_err, "Memory allocation failure\n");
            goto end;
        }
    }

    if (db->attributes.unique_subject) {
        OPENSSL_STRING *crow = row;

        rrow = TXT_DB_get_by_index(db->db, DB_name, crow);
        if (rrow != NULL) {
            BIO_printf(bio_err,
                       "ERROR:There is already a certificate for %s\n",
                       row[DB_name]);
        }
    }
    if (rrow == NULL) {
        rrow = TXT_DB_get_by_index(db->db, DB_serial, row);
        if (rrow != NULL) {
            BIO_printf(bio_err,
                       "ERROR:Serial number %s has already been issued,\n",
                       row[DB_serial]);
            BIO_printf(bio_err,
                       "      check the database/serial_file for corruption\n");
        }
    }

    if (rrow != NULL) {
        BIO_printf(bio_err, "The matching entry has the following details\n");
        if (rrow[DB_type][0] == DB_TYPE_EXP)
            p = "Expired";
        else if (rrow[DB_type][0] == DB_TYPE_REV)
            p = "Revoked";
        else if (rrow[DB_type][0] == DB_TYPE_VAL)
            p = "Valid";
        else
            p = "\ninvalid type, Database error\n";
        BIO_printf(bio_err, "Type          :%s\n", p);
        if (rrow[DB_type][0] == DB_TYPE_REV) {
            p = rrow[DB_exp_date];
            if (p == NULL)
                p = "undef";
            BIO_printf(bio_err, "Was revoked on:%s\n", p);
        }
        p = rrow[DB_exp_date];
        if (p == NULL)
            p = "undef";
        BIO_printf(bio_err, "Expires on    :%s\n", p);
        p = rrow[DB_serial];
        if (p == NULL)
            p = "undef";
        BIO_printf(bio_err, "Serial Number :%s\n", p);
        p = rrow[DB_file];
        if (p == NULL)
            p = "undef";
        BIO_printf(bio_err, "File name     :%s\n", p);
        p = rrow[DB_name];
        if (p == NULL)
            p = "undef";
        BIO_printf(bio_err, "Subject Name  :%s\n", p);
        ok = -1;                /* This is now a 'bad' error. */
        goto end;
    }

    if (!default_op) {
        BIO_printf(bio_err, "Certificate Details:\n");
        /*
         * Never print signature details because signature not present
         */
        certopt |= X509_FLAG_NO_SIGDUMP | X509_FLAG_NO_SIGNAME;
        X509_print_ex(bio_err, ret, nameopt, certopt);
    }

    BIO_printf(bio_err, "Certificate is to be certified until ");
    ASN1_TIME_print_ex(bio_err, X509_get0_notAfter(ret), dateopt);
    if (days)
        BIO_printf(bio_err, " (%ld days)", days);
    BIO_printf(bio_err, "\n");

    if (!batch) {

        BIO_printf(bio_err, "Sign the certificate? [y/n]:");
        (void)BIO_flush(bio_err);
        buf[0] = '\0';
        if (fgets(buf, sizeof(buf), stdin) == NULL) {
            BIO_printf(bio_err,
                       "CERTIFICATE WILL NOT BE CERTIFIED: I/O error\n");
            ok = 0;
            goto end;
        }
        if (!(buf[0] == 'y' || buf[0] == 'Y')) {
            BIO_printf(bio_err, "CERTIFICATE WILL NOT BE CERTIFIED\n");
            ok = 0;
            goto end;
        }
    }

    pktmp = X509_get0_pubkey(ret);
    if (EVP_PKEY_missing_parameters(pktmp) &&
        !EVP_PKEY_missing_parameters(pkey))
        EVP_PKEY_copy_parameters(pktmp, pkey);

    if (!do_X509_sign(ret, 0, pkey, dgst, sigopts, &ext_ctx))
        goto end;

    /* We now just add it to the database as DB_TYPE_VAL('V') */
    row[DB_type] = OPENSSL_strdup("V");
    tm = X509_get0_notAfter(ret);
    row[DB_exp_date] = app_malloc(tm->length + 1, "row expdate");
    memcpy(row[DB_exp_date], tm->data, tm->length);
    row[DB_exp_date][tm->length] = '\0';
    row[DB_rev_date] = NULL;
    row[DB_file] = OPENSSL_strdup("unknown");
    if ((row[DB_type] == NULL) || (row[DB_file] == NULL)
        || (row[DB_name] == NULL)) {
        BIO_printf(bio_err, "Memory allocation failure\n");
        goto end;
    }

    irow = app_malloc(sizeof(*irow) * (DB_NUMBER + 1), "row space");
    for (i = 0; i < DB_NUMBER; i++)
        irow[i] = row[i];
    irow[DB_NUMBER] = NULL;

    if (!TXT_DB_insert(db->db, irow)) {
        BIO_printf(bio_err, "failed to update database\n");
        BIO_printf(bio_err, "TXT_DB error number %ld\n", db->db->error);
        goto end;
    }
    irow = NULL;
    ok = 1;
 end:
    if (ok != 1) {
        for (i = 0; i < DB_NUMBER; i++)
            OPENSSL_free(row[i]);
    }
    OPENSSL_free(irow);

    X509_NAME_free(CAname);
    X509_NAME_free(subject);
    if (ok <= 0)
        X509_free(ret);
    else
        *xret = ret;
    return ok;
}

static void write_new_certificate(BIO *bp, X509 *x, int output_der, int notext)
{

    if (output_der) {
        (void)i2d_X509_bio(bp, x);
        return;
    }
    if (!notext)
        X509_print(bp, x);
    PEM_write_bio_X509(bp, x);
}

static int certify_spkac(X509 **xret, const char *infile, EVP_PKEY *pkey,
                         X509 *x509, const char *dgst,
                         STACK_OF(OPENSSL_STRING) *sigopts,
                         STACK_OF(CONF_VALUE) *policy, CA_DB *db,
                         BIGNUM *serial, const char *subj, unsigned long chtype,
                         int multirdn, int email_dn, const char *startdate,
                         const char *enddate, long days, const char *ext_sect,
                         CONF *lconf, int verbose, unsigned long certopt,
                         unsigned long nameopt, int default_op, int ext_copy, unsigned long dateopt)
{
    STACK_OF(CONF_VALUE) *sk = NULL;
    LHASH_OF(CONF_VALUE) *parms = NULL;
    X509_REQ *req = NULL;
    CONF_VALUE *cv = NULL;
    NETSCAPE_SPKI *spki = NULL;
    char *type, *buf;
    EVP_PKEY *pktmp = NULL;
    X509_NAME *n = NULL;
    X509_NAME_ENTRY *ne = NULL;
    int ok = -1, i, j;
    long errline;
    int nid;

    /*
     * Load input file into a hash table.  (This is just an easy
     * way to read and parse the file, then put it into a convenient
     * STACK format).
     */
    parms = CONF_load(NULL, infile, &errline);
    if (parms == NULL) {
        BIO_printf(bio_err, "error on line %ld of %s\n", errline, infile);
        goto end;
    }

    sk = CONF_get_section(parms, "default");
    if (sk_CONF_VALUE_num(sk) == 0) {
        BIO_printf(bio_err, "no name/value pairs found in %s\n", infile);
        goto end;
    }

    /*
     * Now create a dummy X509 request structure.  We don't actually
     * have an X509 request, but we have many of the components
     * (a public key, various DN components).  The idea is that we
     * put these components into the right X509 request structure
     * and we can use the same code as if you had a real X509 request.
     */
    req = X509_REQ_new();
    if (req == NULL)
        goto end;

    /*
     * Build up the subject name set.
     */
    n = X509_REQ_get_subject_name(req);

    for (i = 0;; i++) {
        if (sk_CONF_VALUE_num(sk) <= i)
            break;

        cv = sk_CONF_VALUE_value(sk, i);
        type = cv->name;
        /*
         * Skip past any leading X. X: X, etc to allow for multiple instances
         */
        for (buf = cv->name; *buf; buf++)
            if ((*buf == ':') || (*buf == ',') || (*buf == '.')) {
                buf++;
                if (*buf)
                    type = buf;
                break;
            }

        buf = cv->value;
        if ((nid = OBJ_txt2nid(type)) == NID_undef) {
            if (strcmp(type, "SPKAC") == 0) {
                spki = NETSCAPE_SPKI_b64_decode(cv->value, -1);
                if (spki == NULL) {
                    BIO_printf(bio_err,
                               "unable to load Netscape SPKAC structure\n");
                    goto end;
                }
            }
            continue;
        }

        if (!X509_NAME_add_entry_by_NID(n, nid, chtype,
                                        (unsigned char *)buf, -1, -1, 0))
            goto end;
    }
    if (spki == NULL) {
        BIO_printf(bio_err, "Netscape SPKAC structure not found in %s\n",
                   infile);
        goto end;
    }

    /*
     * Now extract the key from the SPKI structure.
     */

    BIO_printf(bio_err, "Check that the SPKAC request matches the signature\n");

    if ((pktmp = NETSCAPE_SPKI_get_pubkey(spki)) == NULL) {
        BIO_printf(bio_err, "error unpacking SPKAC public key\n");
        goto end;
    }

    j = NETSCAPE_SPKI_verify(spki, pktmp);
    if (j <= 0) {
        EVP_PKEY_free(pktmp);
        BIO_printf(bio_err,
                   "signature verification failed on SPKAC public key\n");
        goto end;
    }
    BIO_printf(bio_err, "Signature ok\n");

    X509_REQ_set_pubkey(req, pktmp);
    EVP_PKEY_free(pktmp);
    ok = do_body(xret, pkey, x509, dgst, sigopts, policy, db, serial, subj,
                 chtype, multirdn, email_dn, startdate, enddate, days, 1,
                 verbose, req, ext_sect, lconf, certopt, nameopt, default_op,
                 ext_copy, 0, dateopt);
 end:
    X509_REQ_free(req);
    CONF_free(parms);
    NETSCAPE_SPKI_free(spki);
    X509_NAME_ENTRY_free(ne);

    return ok;
}

static int check_time_format(const char *str)
{
    return ASN1_TIME_set_string(NULL, str);
}

static int do_revoke(X509 *x509, CA_DB *db, REVINFO_TYPE rev_type,
                     const char *value)
{
    const ASN1_TIME *tm = NULL;
    char *row[DB_NUMBER], **rrow, **irow;
    char *rev_str = NULL;
    BIGNUM *bn = NULL;
    int ok = -1, i;

    for (i = 0; i < DB_NUMBER; i++)
        row[i] = NULL;
    row[DB_name] = X509_NAME_oneline(X509_get_subject_name(x509), NULL, 0);
    bn = ASN1_INTEGER_to_BN(X509_get0_serialNumber(x509), NULL);
    if (!bn)
        goto end;
    if (BN_is_zero(bn))
        row[DB_serial] = OPENSSL_strdup("00");
    else
        row[DB_serial] = BN_bn2hex(bn);
    BN_free(bn);
    if (row[DB_name] != NULL && row[DB_name][0] == '\0') {
        /* Entries with empty Subjects actually use the serial number instead */
        OPENSSL_free(row[DB_name]);
        row[DB_name] = OPENSSL_strdup(row[DB_serial]);
    }
    if ((row[DB_name] == NULL) || (row[DB_serial] == NULL)) {
        BIO_printf(bio_err, "Memory allocation failure\n");
        goto end;
    }
    /*
     * We have to lookup by serial number because name lookup skips revoked
     * certs
     */
    rrow = TXT_DB_get_by_index(db->db, DB_serial, row);
    if (rrow == NULL) {
        BIO_printf(bio_err,
                   "Adding Entry with serial number %s to DB for %s\n",
                   row[DB_serial], row[DB_name]);

        /* We now just add it to the database as DB_TYPE_REV('V') */
        row[DB_type] = OPENSSL_strdup("V");
        tm = X509_get0_notAfter(x509);
        row[DB_exp_date] = app_malloc(tm->length + 1, "row exp_data");
        memcpy(row[DB_exp_date], tm->data, tm->length);
        row[DB_exp_date][tm->length] = '\0';
        row[DB_rev_date] = NULL;
        row[DB_file] = OPENSSL_strdup("unknown");

        if (row[DB_type] == NULL || row[DB_file] == NULL) {
            BIO_printf(bio_err, "Memory allocation failure\n");
            goto end;
        }

        irow = app_malloc(sizeof(*irow) * (DB_NUMBER + 1), "row ptr");
        for (i = 0; i < DB_NUMBER; i++)
            irow[i] = row[i];
        irow[DB_NUMBER] = NULL;

        if (!TXT_DB_insert(db->db, irow)) {
            BIO_printf(bio_err, "failed to update database\n");
            BIO_printf(bio_err, "TXT_DB error number %ld\n", db->db->error);
            OPENSSL_free(irow);
            goto end;
        }

        for (i = 0; i < DB_NUMBER; i++)
            row[i] = NULL;

        /* Revoke Certificate */
        if (rev_type == REV_VALID)
            ok = 1;
        else
            /* Retry revocation after DB insertion */
            ok = do_revoke(x509, db, rev_type, value);

        goto end;

    } else if (index_name_cmp_noconst(row, rrow)) {
        BIO_printf(bio_err, "ERROR:name does not match %s\n", row[DB_name]);
        goto end;
    } else if (rev_type == REV_VALID) {
        BIO_printf(bio_err, "ERROR:Already present, serial number %s\n",
                   row[DB_serial]);
        goto end;
    } else if (rrow[DB_type][0] == DB_TYPE_REV) {
        BIO_printf(bio_err, "ERROR:Already revoked, serial number %s\n",
                   row[DB_serial]);
        goto end;
    } else {
        BIO_printf(bio_err, "Revoking Certificate %s.\n", rrow[DB_serial]);
        rev_str = make_revocation_str(rev_type, value);
        if (!rev_str) {
            BIO_printf(bio_err, "Error in revocation arguments\n");
            goto end;
        }
        rrow[DB_type][0] = DB_TYPE_REV;
        rrow[DB_type][1] = '\0';
        rrow[DB_rev_date] = rev_str;
    }
    ok = 1;
 end:
    for (i = 0; i < DB_NUMBER; i++)
        OPENSSL_free(row[i]);
    return ok;
}

static int get_certificate_status(const char *serial, CA_DB *db)
{
    char *row[DB_NUMBER], **rrow;
    int ok = -1, i;
    size_t serial_len = strlen(serial);

    /* Free Resources */
    for (i = 0; i < DB_NUMBER; i++)
        row[i] = NULL;

    /* Malloc needed char spaces */
    row[DB_serial] = app_malloc(serial_len + 2, "row serial#");

    if (serial_len % 2) {
        /*
         * Set the first char to 0
         */
        row[DB_serial][0] = '0';

        /* Copy String from serial to row[DB_serial] */
        memcpy(row[DB_serial] + 1, serial, serial_len);
        row[DB_serial][serial_len + 1] = '\0';
    } else {
        /* Copy String from serial to row[DB_serial] */
        memcpy(row[DB_serial], serial, serial_len);
        row[DB_serial][serial_len] = '\0';
    }

    /* Make it Upper Case */
    make_uppercase(row[DB_serial]);

    ok = 1;

    /* Search for the certificate */
    rrow = TXT_DB_get_by_index(db->db, DB_serial, row);
    if (rrow == NULL) {
        BIO_printf(bio_err, "Serial %s not present in db.\n", row[DB_serial]);
        ok = -1;
        goto end;
    } else if (rrow[DB_type][0] == DB_TYPE_VAL) {
        BIO_printf(bio_err, "%s=Valid (%c)\n",
                   row[DB_serial], rrow[DB_type][0]);
        goto end;
    } else if (rrow[DB_type][0] == DB_TYPE_REV) {
        BIO_printf(bio_err, "%s=Revoked (%c)\n",
                   row[DB_serial], rrow[DB_type][0]);
        goto end;
    } else if (rrow[DB_type][0] == DB_TYPE_EXP) {
        BIO_printf(bio_err, "%s=Expired (%c)\n",
                   row[DB_serial], rrow[DB_type][0]);
        goto end;
    } else if (rrow[DB_type][0] == DB_TYPE_SUSP) {
        BIO_printf(bio_err, "%s=Suspended (%c)\n",
                   row[DB_serial], rrow[DB_type][0]);
        goto end;
    } else {
        BIO_printf(bio_err, "%s=Unknown (%c).\n",
                   row[DB_serial], rrow[DB_type][0]);
        ok = -1;
    }
 end:
    for (i = 0; i < DB_NUMBER; i++) {
        OPENSSL_free(row[i]);
    }
    return ok;
}

int do_updatedb(CA_DB *db, time_t *now)
{
    ASN1_TIME *a_tm = NULL;
    int i, cnt = 0;
    char **rrow;

    a_tm = ASN1_TIME_new();
    if (a_tm == NULL)
        return -1;

    /* get actual time */
    if (X509_time_adj(a_tm, 0, now) == NULL) {
        ASN1_TIME_free(a_tm);
        return -1;
    }

    for (i = 0; i < sk_OPENSSL_PSTRING_num(db->db->data); i++) {
        rrow = sk_OPENSSL_PSTRING_value(db->db->data, i);

        if (rrow[DB_type][0] == DB_TYPE_VAL) {
            /* ignore entries that are not valid */
            ASN1_TIME *exp_date = NULL;

            exp_date = ASN1_TIME_new();
            if (exp_date == NULL) {
                ASN1_TIME_free(a_tm);
                return -1;
            }

            if (!ASN1_TIME_set_string(exp_date, rrow[DB_exp_date])) {
                ASN1_TIME_free(a_tm);
                ASN1_TIME_free(exp_date);
                return -1;
            }

            if (ASN1_TIME_compare(exp_date, a_tm) <= 0) {
                rrow[DB_type][0] = DB_TYPE_EXP;
                rrow[DB_type][1] = '\0';
                cnt++;

                BIO_printf(bio_err, "%s=Expired\n", rrow[DB_serial]);
            }
            ASN1_TIME_free(exp_date);
        }
    }

    ASN1_TIME_free(a_tm);
    return cnt;
}

static const char *crl_reasons[] = {
    /* CRL reason strings */
    "unspecified",
    "keyCompromise",
    "CACompromise",
    "affiliationChanged",
    "superseded",
    "cessationOfOperation",
    "certificateHold",
    "removeFromCRL",
    /* Additional pseudo reasons */
    "holdInstruction",
    "keyTime",
    "CAkeyTime"
};

#define NUM_REASONS OSSL_NELEM(crl_reasons)

/*
 * Given revocation information convert to a DB string. The format of the
 * string is: revtime[,reason,extra]. Where 'revtime' is the revocation time
 * (the current time). 'reason' is the optional CRL reason and 'extra' is any
 * additional argument
 */

static char *make_revocation_str(REVINFO_TYPE rev_type, const char *rev_arg)
{
    char *str;
    const char *reason = NULL, *other = NULL;
    ASN1_OBJECT *otmp;
    ASN1_UTCTIME *revtm = NULL;
    int i;

    switch (rev_type) {
    case REV_NONE:
    case REV_VALID:
        break;

    case REV_CRL_REASON:
        for (i = 0; i < 8; i++) {
            if (OPENSSL_strcasecmp(rev_arg, crl_reasons[i]) == 0) {
                reason = crl_reasons[i];
                break;
            }
        }
        if (reason == NULL) {
            BIO_printf(bio_err, "Unknown CRL reason %s\n", rev_arg);
            return NULL;
        }
        break;

    case REV_HOLD:
        /* Argument is an OID */
        otmp = OBJ_txt2obj(rev_arg, 0);
        ASN1_OBJECT_free(otmp);

        if (otmp == NULL) {
            BIO_printf(bio_err, "Invalid object identifier %s\n", rev_arg);
            return NULL;
        }

        reason = "holdInstruction";
        other = rev_arg;
        break;

    case REV_KEY_COMPROMISE:
    case REV_CA_COMPROMISE:
        /* Argument is the key compromise time  */
        if (!ASN1_GENERALIZEDTIME_set_string(NULL, rev_arg)) {
            BIO_printf(bio_err,
                       "Invalid time format %s. Need YYYYMMDDHHMMSSZ\n",
                       rev_arg);
            return NULL;
        }
        other = rev_arg;
        if (rev_type == REV_KEY_COMPROMISE)
            reason = "keyTime";
        else
            reason = "CAkeyTime";

        break;
    }

    revtm = X509_gmtime_adj(NULL, 0);

    if (!revtm)
        return NULL;

    i = revtm->length + 1;

    if (reason)
        i += strlen(reason) + 1;
    if (other)
        i += strlen(other) + 1;

    str = app_malloc(i, "revocation reason");
    OPENSSL_strlcpy(str, (char *)revtm->data, i);
    if (reason) {
        OPENSSL_strlcat(str, ",", i);
        OPENSSL_strlcat(str, reason, i);
    }
    if (other) {
        OPENSSL_strlcat(str, ",", i);
        OPENSSL_strlcat(str, other, i);
    }
    ASN1_UTCTIME_free(revtm);
    return str;
}

/*-
 * Convert revocation field to X509_REVOKED entry
 * return code:
 * 0 error
 * 1 OK
 * 2 OK and some extensions added (i.e. V2 CRL)
 */

static int make_revoked(X509_REVOKED *rev, const char *str)
{
    char *tmp = NULL;
    int reason_code = -1;
    int i, ret = 0;
    ASN1_OBJECT *hold = NULL;
    ASN1_GENERALIZEDTIME *comp_time = NULL;
    ASN1_ENUMERATED *rtmp = NULL;

    ASN1_TIME *revDate = NULL;

    i = unpack_revinfo(&revDate, &reason_code, &hold, &comp_time, str);

    if (i == 0)
        goto end;

    if (rev && !X509_REVOKED_set_revocationDate(rev, revDate))
        goto end;

    if (rev && (reason_code != OCSP_REVOKED_STATUS_NOSTATUS)) {
        rtmp = ASN1_ENUMERATED_new();
        if (rtmp == NULL || !ASN1_ENUMERATED_set(rtmp, reason_code))
            goto end;
        if (X509_REVOKED_add1_ext_i2d(rev, NID_crl_reason, rtmp, 0, 0) <= 0)
            goto end;
    }

    if (rev && comp_time) {
        if (X509_REVOKED_add1_ext_i2d
            (rev, NID_invalidity_date, comp_time, 0, 0) <= 0)
            goto end;
    }
    if (rev && hold) {
        if (X509_REVOKED_add1_ext_i2d
            (rev, NID_hold_instruction_code, hold, 0, 0) <= 0)
            goto end;
    }

    if (reason_code != OCSP_REVOKED_STATUS_NOSTATUS)
        ret = 2;
    else
        ret = 1;

 end:

    OPENSSL_free(tmp);
    ASN1_OBJECT_free(hold);
    ASN1_GENERALIZEDTIME_free(comp_time);
    ASN1_ENUMERATED_free(rtmp);
    ASN1_TIME_free(revDate);

    return ret;
}

static int old_entry_print(const ASN1_OBJECT *obj, const ASN1_STRING *str)
{
    char buf[25], *pbuf;
    const char *p;
    int j;

    j = i2a_ASN1_OBJECT(bio_err, obj);
    pbuf = buf;
    for (j = 22 - j; j > 0; j--)
        *(pbuf++) = ' ';
    *(pbuf++) = ':';
    *(pbuf++) = '\0';
    BIO_puts(bio_err, buf);

    if (str->type == V_ASN1_PRINTABLESTRING)
        BIO_printf(bio_err, "PRINTABLE:'");
    else if (str->type == V_ASN1_T61STRING)
        BIO_printf(bio_err, "T61STRING:'");
    else if (str->type == V_ASN1_IA5STRING)
        BIO_printf(bio_err, "IA5STRING:'");
    else if (str->type == V_ASN1_UNIVERSALSTRING)
        BIO_printf(bio_err, "UNIVERSALSTRING:'");
    else
        BIO_printf(bio_err, "ASN.1 %2d:'", str->type);

    p = (const char *)str->data;
    for (j = str->length; j > 0; j--) {
        if ((*p >= ' ') && (*p <= '~'))
            BIO_printf(bio_err, "%c", *p);
        else if (*p & 0x80)
            BIO_printf(bio_err, "\\0x%02X", *p);
        else if ((unsigned char)*p == 0xf7)
            BIO_printf(bio_err, "^?");
        else
            BIO_printf(bio_err, "^%c", *p + '@');
        p++;
    }
    BIO_printf(bio_err, "'\n");
    return 1;
}

int unpack_revinfo(ASN1_TIME **prevtm, int *preason, ASN1_OBJECT **phold,
                   ASN1_GENERALIZEDTIME **pinvtm, const char *str)
{
    char *tmp;
    char *rtime_str, *reason_str = NULL, *arg_str = NULL, *p;
    int reason_code = -1;
    int ret = 0;
    unsigned int i;
    ASN1_OBJECT *hold = NULL;
    ASN1_GENERALIZEDTIME *comp_time = NULL;

    tmp = OPENSSL_strdup(str);
    if (!tmp) {
        BIO_printf(bio_err, "memory allocation failure\n");
        goto end;
    }

    p = strchr(tmp, ',');

    rtime_str = tmp;

    if (p) {
        *p = '\0';
        p++;
        reason_str = p;
        p = strchr(p, ',');
        if (p) {
            *p = '\0';
            arg_str = p + 1;
        }
    }

    if (prevtm) {
        *prevtm = ASN1_UTCTIME_new();
        if (*prevtm == NULL) {
            BIO_printf(bio_err, "memory allocation failure\n");
            goto end;
        }
        if (!ASN1_UTCTIME_set_string(*prevtm, rtime_str)) {
            BIO_printf(bio_err, "invalid revocation date %s\n", rtime_str);
            goto end;
        }
    }
    if (reason_str) {
        for (i = 0; i < NUM_REASONS; i++) {
            if (OPENSSL_strcasecmp(reason_str, crl_reasons[i]) == 0) {
                reason_code = i;
                break;
            }
        }
        if (reason_code == OCSP_REVOKED_STATUS_NOSTATUS) {
            BIO_printf(bio_err, "invalid reason code %s\n", reason_str);
            goto end;
        }

        if (reason_code == 7) {
            reason_code = OCSP_REVOKED_STATUS_REMOVEFROMCRL;
        } else if (reason_code == 8) { /* Hold instruction */
            if (!arg_str) {
                BIO_printf(bio_err, "missing hold instruction\n");
                goto end;
            }
            reason_code = OCSP_REVOKED_STATUS_CERTIFICATEHOLD;
            hold = OBJ_txt2obj(arg_str, 0);

            if (!hold) {
                BIO_printf(bio_err, "invalid object identifier %s\n", arg_str);
                goto end;
            }
            if (phold)
                *phold = hold;
            else
                ASN1_OBJECT_free(hold);
        } else if ((reason_code == 9) || (reason_code == 10)) {
            if (!arg_str) {
                BIO_printf(bio_err, "missing compromised time\n");
                goto end;
            }
            comp_time = ASN1_GENERALIZEDTIME_new();
            if (comp_time == NULL) {
                BIO_printf(bio_err, "memory allocation failure\n");
                goto end;
            }
            if (!ASN1_GENERALIZEDTIME_set_string(comp_time, arg_str)) {
                BIO_printf(bio_err, "invalid compromised time %s\n", arg_str);
                goto end;
            }
            if (reason_code == 9)
                reason_code = OCSP_REVOKED_STATUS_KEYCOMPROMISE;
            else
                reason_code = OCSP_REVOKED_STATUS_CACOMPROMISE;
        }
    }

    if (preason)
        *preason = reason_code;
    if (pinvtm) {
        *pinvtm = comp_time;
        comp_time = NULL;
    }

    ret = 1;

 end:

    OPENSSL_free(tmp);
    ASN1_GENERALIZEDTIME_free(comp_time);

    return ret;
}
