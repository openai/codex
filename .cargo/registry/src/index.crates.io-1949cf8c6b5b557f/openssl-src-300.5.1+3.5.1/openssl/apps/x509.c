/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
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
#include <openssl/asn1.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/objects.h>
#include <openssl/pem.h>
#include <openssl/rsa.h>
#ifndef OPENSSL_NO_DSA
# include <openssl/dsa.h>
#endif
#include "internal/e_os.h"    /* For isatty() */

#undef POSTFIX
#define POSTFIX ".srl"
#define DEFAULT_DAYS       30 /* default certificate validity period in days */
#define UNSET_DAYS         -2 /* -1 may be used for testing expiration checks */
#define EXT_COPY_UNSET     -1

static int callb(int ok, X509_STORE_CTX *ctx);
static ASN1_INTEGER *x509_load_serial(const char *CAfile,
                                      const char *serialfile, int create);
static int purpose_print(BIO *bio, X509 *cert, X509_PURPOSE *pt);
static int print_x509v3_exts(BIO *bio, X509 *x, const char *ext_names);

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_INFORM, OPT_OUTFORM, OPT_KEYFORM, OPT_REQ, OPT_CAFORM,
    OPT_CAKEYFORM, OPT_VFYOPT, OPT_SIGOPT, OPT_DAYS, OPT_PASSIN, OPT_EXTFILE,
    OPT_EXTENSIONS, OPT_IN, OPT_OUT, OPT_KEY, OPT_SIGNKEY, OPT_CA, OPT_CAKEY,
    OPT_CASERIAL, OPT_SET_SERIAL, OPT_NEW, OPT_FORCE_PUBKEY, OPT_ISSU, OPT_SUBJ,
    OPT_ADDTRUST, OPT_ADDREJECT, OPT_SETALIAS, OPT_CERTOPT, OPT_DATEOPT, OPT_NAMEOPT,
    OPT_EMAIL, OPT_OCSP_URI, OPT_SERIAL, OPT_NEXT_SERIAL,
    OPT_MODULUS, OPT_PUBKEY, OPT_X509TOREQ, OPT_TEXT, OPT_HASH,
    OPT_ISSUER_HASH, OPT_SUBJECT, OPT_ISSUER, OPT_FINGERPRINT, OPT_DATES,
    OPT_PURPOSE, OPT_STARTDATE, OPT_ENDDATE, OPT_CHECKEND, OPT_CHECKHOST,
    OPT_CHECKEMAIL, OPT_CHECKIP, OPT_NOOUT, OPT_TRUSTOUT, OPT_CLRTRUST,
    OPT_CLRREJECT, OPT_ALIAS, OPT_CACREATESERIAL, OPT_CLREXT, OPT_OCSPID,
    OPT_SUBJECT_HASH_OLD, OPT_ISSUER_HASH_OLD, OPT_COPY_EXTENSIONS,
    OPT_BADSIG, OPT_MD, OPT_ENGINE, OPT_NOCERT, OPT_PRESERVE_DATES,
    OPT_NOT_BEFORE, OPT_NOT_AFTER,
    OPT_R_ENUM, OPT_PROV_ENUM, OPT_EXT
} OPTION_CHOICE;

const OPTIONS x509_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},

    {"in", OPT_IN, '<',
     "Certificate input, or CSR input file with -req (default stdin)"},
    {"passin", OPT_PASSIN, 's', "Private key and cert file pass-phrase source"},
    {"new", OPT_NEW, '-', "Generate a certificate from scratch"},
    {"x509toreq", OPT_X509TOREQ, '-',
     "Output a certification request (rather than a certificate)"},
    {"req", OPT_REQ, '-', "Input is a CSR file (rather than a certificate)"},
    {"copy_extensions", OPT_COPY_EXTENSIONS, 's',
     "copy extensions when converting from CSR to x509 or vice versa"},
    {"inform", OPT_INFORM, 'f',
     "CSR input format to use (PEM or DER; by default try PEM first)"},
    {"vfyopt", OPT_VFYOPT, 's', "CSR verification parameter in n:v form"},
    {"key", OPT_KEY, 's',
     "Key for signing, and to include unless using -force_pubkey"},
    {"signkey", OPT_SIGNKEY, 's',
     "Same as -key"},
    {"keyform", OPT_KEYFORM, 'E',
     "Key input format (ENGINE, other values ignored)"},
    {"out", OPT_OUT, '>', "Output file - default stdout"},
    {"outform", OPT_OUTFORM, 'f',
     "Output format (DER or PEM) - default PEM"},
    {"nocert", OPT_NOCERT, '-',
     "No cert output (except for requested printing)"},
    {"noout", OPT_NOOUT, '-', "No output (except for requested printing)"},

    OPT_SECTION("Certificate printing"),
    {"text", OPT_TEXT, '-', "Print the certificate in text form"},
    {"dateopt", OPT_DATEOPT, 's',
     "Datetime format used for printing. (rfc_822/iso_8601). Default is rfc_822."},
    {"certopt", OPT_CERTOPT, 's', "Various certificate text printing options"},
    {"fingerprint", OPT_FINGERPRINT, '-', "Print the certificate fingerprint"},
    {"alias", OPT_ALIAS, '-', "Print certificate alias"},
    {"serial", OPT_SERIAL, '-', "Print serial number value"},
    {"startdate", OPT_STARTDATE, '-', "Print the notBefore field"},
    {"enddate", OPT_ENDDATE, '-', "Print the notAfter field"},
    {"dates", OPT_DATES, '-', "Print both notBefore and notAfter fields"},
    {"subject", OPT_SUBJECT, '-', "Print subject DN"},
    {"issuer", OPT_ISSUER, '-', "Print issuer DN"},
    {"nameopt", OPT_NAMEOPT, 's',
     "Certificate subject/issuer name printing options"},
    {"email", OPT_EMAIL, '-', "Print email address(es)"},
    {"hash", OPT_HASH, '-', "Synonym for -subject_hash (for backward compat)"},
    {"subject_hash", OPT_HASH, '-', "Print subject hash value"},
#ifndef OPENSSL_NO_MD5
    {"subject_hash_old", OPT_SUBJECT_HASH_OLD, '-',
     "Print old-style (MD5) subject hash value"},
#endif
    {"issuer_hash", OPT_ISSUER_HASH, '-', "Print issuer hash value"},
#ifndef OPENSSL_NO_MD5
    {"issuer_hash_old", OPT_ISSUER_HASH_OLD, '-',
     "Print old-style (MD5) issuer hash value"},
#endif
    {"ext", OPT_EXT, 's',
     "Restrict which X.509 extensions to print and/or copy"},
    {"ocspid", OPT_OCSPID, '-',
     "Print OCSP hash values for the subject name and public key"},
    {"ocsp_uri", OPT_OCSP_URI, '-', "Print OCSP Responder URL(s)"},
    {"purpose", OPT_PURPOSE, '-', "Print out certificate purposes"},
    {"pubkey", OPT_PUBKEY, '-', "Print the public key in PEM format"},
    {"modulus", OPT_MODULUS, '-', "Print the RSA key modulus"},

    OPT_SECTION("Certificate checking"),
    {"checkend", OPT_CHECKEND, 'M',
     "Check whether cert expires in the next arg seconds"},
    {OPT_MORE_STR, 1, 1, "Exit 1 (failure) if so, 0 if not"},
    {"checkhost", OPT_CHECKHOST, 's', "Check certificate matches host"},
    {"checkemail", OPT_CHECKEMAIL, 's', "Check certificate matches email"},
    {"checkip", OPT_CHECKIP, 's', "Check certificate matches ipaddr"},

    OPT_SECTION("Certificate output"),
    {"set_serial", OPT_SET_SERIAL, 's',
     "Serial number to use, overrides -CAserial"},
    {"next_serial", OPT_NEXT_SERIAL, '-',
     "Increment current certificate serial number"},
    {"not_before", OPT_NOT_BEFORE, 's',
     "[CC]YYMMDDHHMMSSZ value for notBefore certificate field"},
    {"not_after", OPT_NOT_AFTER, 's',
     "[CC]YYMMDDHHMMSSZ value for notAfter certificate field, overrides -days"},
    {"days", OPT_DAYS, 'n',
     "Number of days until newly generated certificate expires - default 30"},
    {"preserve_dates", OPT_PRESERVE_DATES, '-',
     "Preserve existing validity dates"},
    {"set_issuer", OPT_ISSU, 's', "Set or override certificate issuer"},
    {"set_subject", OPT_SUBJ, 's', "Set or override certificate subject (and issuer)"},
    {"subj", OPT_SUBJ, 's', "Alias for -set_subject"},
    {"force_pubkey", OPT_FORCE_PUBKEY, '<',
     "Key to be placed in new certificate or certificate request"},
    {"clrext", OPT_CLREXT, '-',
     "Do not take over any extensions from the source certificate or request"},
    {"extfile", OPT_EXTFILE, '<', "Config file with X509V3 extensions to add"},
    {"extensions", OPT_EXTENSIONS, 's',
     "Section of extfile to use - default: unnamed section"},
    {"sigopt", OPT_SIGOPT, 's', "Signature parameter, in n:v form"},
    {"badsig", OPT_BADSIG, '-',
     "Corrupt last byte of certificate signature (for test)"},
    {"", OPT_MD, '-', "Any supported digest, used for signing and printing"},

    OPT_SECTION("Micro-CA"),
    {"CA", OPT_CA, '<',
     "Use the given CA certificate, conflicts with -key"},
    {"CAform", OPT_CAFORM, 'F', "CA cert format (PEM/DER/P12); has no effect"},
    {"CAkey", OPT_CAKEY, 's', "The corresponding CA key; default is -CA arg"},
    {"CAkeyform", OPT_CAKEYFORM, 'E',
     "CA key format (ENGINE, other values ignored)"},
    {"CAserial", OPT_CASERIAL, 's',
     "File that keeps track of CA-generated serial number"},
    {"CAcreateserial", OPT_CACREATESERIAL, '-',
     "Create CA serial number file if it does not exist"},

    OPT_SECTION("Certificate trust output"),
    {"trustout", OPT_TRUSTOUT, '-', "Mark certificate PEM output as trusted"},
    {"setalias", OPT_SETALIAS, 's', "Set certificate alias (nickname)"},
    {"clrtrust", OPT_CLRTRUST, '-', "Clear all trusted purposes"},
    {"addtrust", OPT_ADDTRUST, 's', "Trust certificate for a given purpose"},
    {"clrreject", OPT_CLRREJECT, '-',
     "Clears all the prohibited or rejected uses of the certificate"},
    {"addreject", OPT_ADDREJECT, 's',
     "Reject certificate for a given purpose"},

    OPT_R_OPTIONS,
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    OPT_PROV_OPTIONS,
    {NULL}
};

static void warn_copying(ASN1_OBJECT *excluded, const char *names)
{
    const char *sn = OBJ_nid2sn(OBJ_obj2nid(excluded));

    if (names != NULL && strstr(names, sn) != NULL)
        BIO_printf(bio_err,
                   "Warning: -ext should not specify copying %s extension to CSR; ignoring this\n",
                   sn);
}

static X509_REQ *x509_to_req(X509 *cert, int ext_copy, const char *names)
{
    const STACK_OF(X509_EXTENSION) *cert_exts = X509_get0_extensions(cert);
    int i, n = sk_X509_EXTENSION_num(cert_exts /* may be NULL */);
    ASN1_OBJECT *skid = OBJ_nid2obj(NID_subject_key_identifier);
    ASN1_OBJECT *akid = OBJ_nid2obj(NID_authority_key_identifier);
    STACK_OF(X509_EXTENSION) *exts;
    X509_REQ *req = X509_to_X509_REQ(cert, NULL, NULL);

    if (req == NULL)
        return NULL;

    /*
     * Filter out SKID and AKID extensions, which make no sense in a CSR.
     * If names is not NULL, copy only those extensions listed there.
     */
    warn_copying(skid, names);
    warn_copying(akid, names);
    if ((exts = sk_X509_EXTENSION_new_reserve(NULL, n)) == NULL)
        goto err;
    for (i = 0; i < n; i++) {
        X509_EXTENSION *ex = sk_X509_EXTENSION_value(cert_exts, i);
        ASN1_OBJECT *obj = X509_EXTENSION_get_object(ex);

        if (OBJ_cmp(obj, skid) != 0 && OBJ_cmp(obj, akid) != 0
                && !sk_X509_EXTENSION_push(exts, ex))
            goto err;
    }

    if (sk_X509_EXTENSION_num(exts) > 0) {
        if (ext_copy != EXT_COPY_UNSET && ext_copy != EXT_COPY_NONE
                && !X509_REQ_add_extensions(req, exts)) {
            BIO_printf(bio_err, "Error copying extensions from certificate\n");
            goto err;
        }
    }
    sk_X509_EXTENSION_free(exts);
    return req;

 err:
    sk_X509_EXTENSION_free(exts);
    X509_REQ_free(req);
    return NULL;
}

static int self_signed(X509_STORE *ctx, X509 *cert)
{
    X509_STORE_CTX *xsc = X509_STORE_CTX_new();
    int ret = 0;

    if (xsc == NULL || !X509_STORE_CTX_init(xsc, ctx, cert, NULL)) {
        BIO_printf(bio_err, "Error initialising X509 store\n");
    } else {
        X509_STORE_CTX_set_flags(xsc, X509_V_FLAG_CHECK_SS_SIGNATURE);
        ret = X509_verify_cert(xsc) > 0;
    }
    X509_STORE_CTX_free(xsc);
    return ret;
}

int x509_main(int argc, char **argv)
{
    ASN1_INTEGER *sno = NULL;
    ASN1_OBJECT *objtmp = NULL;
    BIO *out = NULL;
    CONF *extconf = NULL;
    int ext_copy = EXT_COPY_UNSET;
    X509V3_CTX ext_ctx;
    EVP_PKEY *privkey = NULL, *CAkey = NULL, *pubkey = NULL;
    EVP_PKEY *pkey;
    int newcert = 0;
    char *issu = NULL, *subj = NULL, *digest = NULL;
    X509_NAME *fissu = NULL, *fsubj = NULL;
    const unsigned long chtype = MBSTRING_ASC;
    const int multirdn = 1;
    STACK_OF(ASN1_OBJECT) *trust = NULL, *reject = NULL;
    STACK_OF(OPENSSL_STRING) *sigopts = NULL, *vfyopts = NULL;
    X509 *x = NULL, *xca = NULL, *issuer_cert;
    X509_REQ *req = NULL, *rq = NULL;
    X509_STORE *ctx = NULL;
    char *CAkeyfile = NULL, *CAserial = NULL, *pubkeyfile = NULL, *alias = NULL;
    char *checkhost = NULL, *checkemail = NULL, *checkip = NULL;
    char *ext_names = NULL;
    char *extsect = NULL, *extfile = NULL, *passin = NULL, *passinarg = NULL;
    char *infile = NULL, *outfile = NULL, *privkeyfile = NULL, *CAfile = NULL;
    char *prog, *not_before = NULL, *not_after = NULL;
    int days = UNSET_DAYS; /* not explicitly set */
    int x509toreq = 0, modulus = 0, print_pubkey = 0, pprint = 0;
    int CAformat = FORMAT_UNDEF, CAkeyformat = FORMAT_UNDEF;
    unsigned long dateopt = ASN1_DTFLGS_RFC822;
    int fingerprint = 0, reqfile = 0, checkend = 0;
    int informat = FORMAT_UNDEF, outformat = FORMAT_PEM, keyformat = FORMAT_UNDEF;
    int next_serial = 0, subject_hash = 0, issuer_hash = 0, ocspid = 0;
    int noout = 0, CA_createserial = 0, email = 0;
    int ocsp_uri = 0, trustout = 0, clrtrust = 0, clrreject = 0, aliasout = 0;
    int ret = 1, i, j, num = 0, badsig = 0, clrext = 0, nocert = 0;
    int text = 0, serial = 0, subject = 0, issuer = 0, startdate = 0, ext = 0;
    int enddate = 0;
    time_t checkoffset = 0;
    unsigned long certflag = 0;
    int preserve_dates = 0;
    OPTION_CHOICE o;
    ENGINE *e = NULL;
#ifndef OPENSSL_NO_MD5
    int subject_hash_old = 0, issuer_hash_old = 0;
#endif

    ctx = X509_STORE_new();
    if (ctx == NULL)
        goto err;
    X509_STORE_set_verify_cb(ctx, callb);

    opt_set_unknown_name("digest");
    prog = opt_init(argc, argv, x509_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto err;
        case OPT_HELP:
            opt_help(x509_options);
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
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &outformat))
                goto opthelp;
            break;
        case OPT_KEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &keyformat))
                goto opthelp;
            break;
        case OPT_CAFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &CAformat))
                goto opthelp;
            break;
        case OPT_CAKEYFORM:
            if (!opt_format(opt_arg(), OPT_FMT_ANY, &CAkeyformat))
                goto opthelp;
            break;
        case OPT_OUT:
            outfile = opt_arg();
            break;
        case OPT_REQ:
            reqfile = 1;
            break;

        case OPT_DATEOPT:
            if (!set_dateopt(&dateopt, opt_arg())) {
                BIO_printf(bio_err,
                           "Invalid date format: %s\n", opt_arg());
                goto err;
            }
            break;
        case OPT_COPY_EXTENSIONS:
            if (!set_ext_copy(&ext_copy, opt_arg())) {
                BIO_printf(bio_err,
                           "Invalid extension copy option: %s\n", opt_arg());
                goto err;
            }
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
                goto err;
            }
            break;
        case OPT_PASSIN:
            passinarg = opt_arg();
            break;
        case OPT_EXTFILE:
            extfile = opt_arg();
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_EXTENSIONS:
            extsect = opt_arg();
            break;
        case OPT_KEY:
        case OPT_SIGNKEY:
            privkeyfile = opt_arg();
            break;
        case OPT_CA:
            CAfile = opt_arg();
            break;
        case OPT_CAKEY:
            CAkeyfile = opt_arg();
            break;
        case OPT_CASERIAL:
            CAserial = opt_arg();
            break;
        case OPT_SET_SERIAL:
            if (sno != NULL) {
                BIO_printf(bio_err, "Serial number supplied twice\n");
                goto opthelp;
            }
            if ((sno = s2i_ASN1_INTEGER(NULL, opt_arg())) == NULL)
                goto opthelp;
            break;
        case OPT_NEW:
            newcert = 1;
            break;
        case OPT_FORCE_PUBKEY:
            pubkeyfile = opt_arg();
            break;
        case OPT_ISSU:
            issu = opt_arg();
            break;
        case OPT_SUBJ:
            subj = opt_arg();
            break;
        case OPT_ADDTRUST:
            if (trust == NULL && (trust = sk_ASN1_OBJECT_new_null()) == NULL)
                goto err;
            if ((objtmp = OBJ_txt2obj(opt_arg(), 0)) == NULL) {
                BIO_printf(bio_err, "%s: Invalid trust object value %s\n",
                           prog, opt_arg());
                goto opthelp;
            }
            if (!sk_ASN1_OBJECT_push(trust, objtmp))
                goto err;
            trustout = 1;
            break;
        case OPT_ADDREJECT:
            if (reject == NULL && (reject = sk_ASN1_OBJECT_new_null()) == NULL)
                goto err;
            if ((objtmp = OBJ_txt2obj(opt_arg(), 0)) == NULL) {
                BIO_printf(bio_err, "%s: Invalid reject object value %s\n",
                           prog, opt_arg());
                goto opthelp;
            }
            if (!sk_ASN1_OBJECT_push(reject, objtmp))
                goto err;
            trustout = 1;
            break;
        case OPT_SETALIAS:
            alias = opt_arg();
            trustout = 1;
            break;
        case OPT_CERTOPT:
            if (!set_cert_ex(&certflag, opt_arg()))
                goto opthelp;
            break;
        case OPT_NAMEOPT:
            if (!set_nameopt(opt_arg()))
                goto opthelp;
            break;
        case OPT_ENGINE:
            e = setup_engine(opt_arg(), 0);
            break;
        case OPT_EMAIL:
            email = ++num;
            break;
        case OPT_OCSP_URI:
            ocsp_uri = ++num;
            break;
        case OPT_SERIAL:
            serial = ++num;
            break;
        case OPT_NEXT_SERIAL:
            next_serial = ++num;
            break;
        case OPT_MODULUS:
            modulus = ++num;
            break;
        case OPT_PUBKEY:
            print_pubkey = ++num;
            break;
        case OPT_X509TOREQ:
            x509toreq = 1;
            break;
        case OPT_TEXT:
            text = ++num;
            break;
        case OPT_SUBJECT:
            subject = ++num;
            break;
        case OPT_ISSUER:
            issuer = ++num;
            break;
        case OPT_FINGERPRINT:
            fingerprint = ++num;
            break;
        case OPT_HASH:
            subject_hash = ++num;
            break;
        case OPT_ISSUER_HASH:
            issuer_hash = ++num;
            break;
        case OPT_PURPOSE:
            pprint = ++num;
            break;
        case OPT_STARTDATE:
            startdate = ++num;
            break;
        case OPT_ENDDATE:
            enddate = ++num;
            break;
        case OPT_NOOUT:
            noout = ++num;
            break;
        case OPT_EXT:
            ext = ++num;
            ext_names = opt_arg();
            break;
        case OPT_NOCERT:
            nocert = 1;
            break;
        case OPT_TRUSTOUT:
            trustout = 1;
            break;
        case OPT_CLRTRUST:
            clrtrust = ++num;
            break;
        case OPT_CLRREJECT:
            clrreject = ++num;
            break;
        case OPT_ALIAS:
            aliasout = ++num;
            break;
        case OPT_CACREATESERIAL:
            CA_createserial = 1;
            break;
        case OPT_CLREXT:
            clrext = 1;
            break;
        case OPT_OCSPID:
            ocspid = ++num;
            break;
        case OPT_BADSIG:
            badsig = 1;
            break;
#ifndef OPENSSL_NO_MD5
        case OPT_SUBJECT_HASH_OLD:
            subject_hash_old = ++num;
            break;
        case OPT_ISSUER_HASH_OLD:
            issuer_hash_old = ++num;
            break;
#else
        case OPT_SUBJECT_HASH_OLD:
        case OPT_ISSUER_HASH_OLD:
            break;
#endif
        case OPT_DATES:
            startdate = ++num;
            enddate = ++num;
            break;
        case OPT_CHECKEND:
            checkend = 1;
            {
                ossl_intmax_t temp = 0;
                if (!opt_intmax(opt_arg(), &temp))
                    goto opthelp;
                checkoffset = (time_t)temp;
                if ((ossl_intmax_t)checkoffset != temp) {
                    BIO_printf(bio_err, "%s: Checkend time out of range %s\n",
                               prog, opt_arg());
                    goto opthelp;
                }
            }
            break;
        case OPT_CHECKHOST:
            checkhost = opt_arg();
            break;
        case OPT_CHECKEMAIL:
            checkemail = opt_arg();
            break;
        case OPT_CHECKIP:
            checkip = opt_arg();
            break;
        case OPT_PRESERVE_DATES:
            preserve_dates = 1;
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
        goto err;

    if (!opt_check_md(digest))
        goto opthelp;

    if (preserve_dates && not_before != NULL) {
        BIO_printf(bio_err, "Cannot use -preserve_dates with -not_before option\n");
        goto err;
    }
    if (preserve_dates && not_after != NULL) {
        BIO_printf(bio_err, "Cannot use -preserve_dates with -not_after option\n");
        goto err;
    }
    if (preserve_dates && days != UNSET_DAYS) {
        BIO_printf(bio_err, "Cannot use -preserve_dates with -days option\n");
        goto err;
    }
    if (days == UNSET_DAYS)
        days = DEFAULT_DAYS;
    else if (not_after != NULL)
        BIO_printf(bio_err, "Warning: -not_after option overriding -days option\n");

    if (!app_passwd(passinarg, NULL, &passin, NULL)) {
        BIO_printf(bio_err, "Error getting password\n");
        goto err;
    }

    if (!X509_STORE_set_default_paths_ex(ctx, app_get0_libctx(),
                                         app_get0_propq()))
        goto err;

    if (newcert && infile != NULL) {
        BIO_printf(bio_err, "The -in option cannot be used with -new\n");
        goto err;
    }
    if (newcert && reqfile) {
        BIO_printf(bio_err, "The -req option cannot be used with -new\n");
        goto err;
    }
    if (privkeyfile != NULL) {
        privkey = load_key(privkeyfile, keyformat, 0, passin, e, "private key");
        if (privkey == NULL)
            goto err;
    }
    if (pubkeyfile != NULL) {
        if ((pubkey = load_pubkey(pubkeyfile, keyformat, 0, NULL, e,
                                  "explicitly set public key")) == NULL)
            goto err;
    }

    if (newcert) {
        if (subj == NULL) {
            BIO_printf(bio_err,
                       "The -new option requires a subject to be set using -subj\n");
            goto err;
        }
        if (privkeyfile == NULL && pubkeyfile == NULL) {
            BIO_printf(bio_err,
                       "The -new option requires using the -key or -force_pubkey option\n");
            goto err;
        }
    }
    if (issu != NULL
            && (fissu = parse_name(issu, chtype, multirdn, "issuer")) == NULL)
        goto err;
    if (subj != NULL
            && (fsubj = parse_name(subj, chtype, multirdn, "subject")) == NULL)
        goto err;

    if (CAkeyfile == NULL)
        CAkeyfile = CAfile;
    if (CAfile != NULL) {
        if (privkeyfile != NULL) {
            BIO_printf(bio_err, "Cannot use both -key/-signkey and -CA option\n");
            goto err;
        }
    } else {
#define WARN_NO_CA(opt) BIO_printf(bio_err, \
        "Warning: ignoring " opt " option since -CA option is not given\n");
        if (CAkeyfile != NULL)
            WARN_NO_CA("-CAkey");
        if (CAkeyformat != FORMAT_UNDEF)
            WARN_NO_CA("-CAkeyform");
        if (CAformat != FORMAT_UNDEF)
            WARN_NO_CA("-CAform");
        if (CAserial != NULL)
            WARN_NO_CA("-CAserial");
        if (CA_createserial)
            WARN_NO_CA("-CAcreateserial");
    }

    if (extfile == NULL) {
        if (extsect != NULL)
            BIO_printf(bio_err,
                       "Warning: ignoring -extensions option without -extfile\n");
    } else {
        X509V3_CTX ctx2;

        if ((extconf = app_load_config(extfile)) == NULL)
            goto err;
        if (extsect == NULL) {
            extsect = app_conf_try_string(extconf, "default", "extensions");
            if (extsect == NULL)
                extsect = "default";
        }
        X509V3_set_ctx_test(&ctx2);
        X509V3_set_nconf(&ctx2, extconf);
        if (!X509V3_EXT_add_nconf(extconf, &ctx2, extsect, NULL)) {
            BIO_printf(bio_err,
                       "Error checking extension section %s\n", extsect);
            goto err;
        }
    }

    if (reqfile) {
        if (infile == NULL && isatty(fileno_stdin()))
            BIO_printf(bio_err,
                       "Warning: Reading cert request from stdin since no -in option is given\n");
        req = load_csr_autofmt(infile, informat, vfyopts,
                               "certificate request input");
        if (req == NULL)
            goto err;

        if ((pkey = X509_REQ_get0_pubkey(req)) == NULL) {
            BIO_printf(bio_err, "Error unpacking public key from CSR\n");
            goto err;
        }
        i = do_X509_REQ_verify(req, pkey, vfyopts);
        if (i <= 0) {
            BIO_printf(bio_err, i < 0
                       ? "Error while verifying certificate request self-signature\n"
                       : "Certificate request self-signature did not match the contents\n");
            goto err;
        }
        BIO_printf(bio_err, "Certificate request self-signature ok\n");

        print_name(bio_err, "subject=", X509_REQ_get_subject_name(req));
    } else if (!x509toreq && ext_copy != EXT_COPY_UNSET) {
        BIO_printf(bio_err, "Warning: ignoring -copy_extensions since neither -x509toreq nor -req is given\n");
    }

    if (reqfile || newcert) {
        if (preserve_dates)
            BIO_printf(bio_err,
                       "Warning: ignoring -preserve_dates option with -req or -new\n");
        preserve_dates = 0;
        if (privkeyfile == NULL && CAkeyfile == NULL) {
            BIO_printf(bio_err,
                       "We need a private key to sign with, use -key or -CAkey or -CA with private key\n");
            goto err;
        }
        if ((x = X509_new_ex(app_get0_libctx(), app_get0_propq())) == NULL)
            goto err;
        if (CAfile == NULL && sno == NULL) {
            sno = ASN1_INTEGER_new();
            if (sno == NULL || !rand_serial(NULL, sno))
                goto err;
        }
        if (req != NULL && ext_copy != EXT_COPY_UNSET) {
            if (clrext && ext_copy != EXT_COPY_NONE) {
                BIO_printf(bio_err, "Must not use -clrext together with -copy_extensions\n");
                goto err;
            } else if (!copy_extensions(x, req, ext_copy)) {
                BIO_printf(bio_err, "Error copying extensions from request\n");
                goto err;
            }
        }
    } else {
        if (infile == NULL && isatty(fileno_stdin()))
            BIO_printf(bio_err,
                       "Warning: Reading certificate from stdin since no -in or -new option is given\n");
        x = load_cert_pass(infile, informat, 1, passin, "certificate");
        if (x == NULL)
            goto err;
    }
    if ((fsubj != NULL || req != NULL)
        && !X509_set_subject_name(x, fsubj != NULL ? fsubj :
                                  X509_REQ_get_subject_name(req)))
        goto err;
    if ((pubkey != NULL || privkey != NULL || req != NULL)
        && !X509_set_pubkey(x, pubkey != NULL ? pubkey :
                            privkey != NULL ? privkey :
                            X509_REQ_get0_pubkey(req)))
        goto err;

    if (CAfile != NULL) {
        xca = load_cert_pass(CAfile, CAformat, 1, passin, "CA certificate");
        if (xca == NULL)
            goto err;
    }

    out = bio_open_default(outfile, 'w', outformat);
    if (out == NULL)
        goto err;

    if (alias)
        X509_alias_set1(x, (unsigned char *)alias, -1);

    if (clrtrust)
        X509_trust_clear(x);
    if (clrreject)
        X509_reject_clear(x);

    if (trust != NULL) {
        for (i = 0; i < sk_ASN1_OBJECT_num(trust); i++)
            X509_add1_trust_object(x, sk_ASN1_OBJECT_value(trust, i));
    }

    if (reject != NULL) {
        for (i = 0; i < sk_ASN1_OBJECT_num(reject); i++)
            X509_add1_reject_object(x, sk_ASN1_OBJECT_value(reject, i));
    }

    if (clrext && ext_names != NULL)
        BIO_printf(bio_err, "Warning: Ignoring -ext since -clrext is given\n");
    for (i = X509_get_ext_count(x) - 1; i >= 0; i--) {
        X509_EXTENSION *ex = X509_get_ext(x, i);
        const char *sn = OBJ_nid2sn(OBJ_obj2nid(X509_EXTENSION_get_object(ex)));

        if (clrext || (ext_names != NULL && strstr(ext_names, sn) == NULL))
            X509_EXTENSION_free(X509_delete_ext(x, i));
    }

    issuer_cert = x;
    if (CAfile != NULL) {
        issuer_cert = xca;
        if (sno == NULL)
            sno = x509_load_serial(CAfile, CAserial, CA_createserial);
        if (sno == NULL)
            goto err;
        if (!x509toreq && !reqfile && !newcert && !self_signed(ctx, x))
            goto err;
    } else {
        if (privkey != NULL && !cert_matches_key(x, privkey))
            BIO_printf(bio_err,
                       "Warning: Signature key and public key of cert do not match\n");
    }

    if (sno != NULL && !X509_set_serialNumber(x, sno))
        goto err;

    if (reqfile || newcert || privkey != NULL || CAfile != NULL) {
        if (!preserve_dates && !set_cert_times(x, not_before, not_after, days, 1))
            goto err;
        if (fissu != NULL) {
            if (!X509_set_issuer_name(x, fissu))
                goto err;
        } else {
            if (!X509_set_issuer_name(x, X509_get_subject_name(issuer_cert)))
                goto err;
        }
    }

    X509V3_set_ctx(&ext_ctx, issuer_cert, x, NULL, NULL, X509V3_CTX_REPLACE);
    /* prepare fallback for AKID, but only if issuer cert equals subject cert */
    if (CAfile == NULL) {
        if (!X509V3_set_issuer_pkey(&ext_ctx, privkey))
            goto err;
    }
    if (extconf != NULL && !x509toreq) {
        X509V3_set_nconf(&ext_ctx, extconf);
        if (!X509V3_EXT_add_nconf(extconf, &ext_ctx, extsect, x)) {
            BIO_printf(bio_err,
                       "Error adding extensions from section %s\n", extsect);
            goto err;
        }
    }

    /* At this point the contents of the certificate x have been finished. */

    pkey = X509_get0_pubkey(x);
    if ((print_pubkey != 0 || modulus != 0) && pkey == NULL) {
        BIO_printf(bio_err, "Error getting public key\n");
        goto err;
    }

    if (x509toreq) { /* also works in conjunction with -req */
        if (privkey == NULL) {
            BIO_printf(bio_err, "Must specify request signing key using -key\n");
            goto err;
        }
        if (clrext && ext_copy != EXT_COPY_NONE) {
            BIO_printf(bio_err, "Must not use -clrext together with -copy_extensions\n");
            goto err;
        }
        if ((rq = x509_to_req(x, ext_copy, ext_names)) == NULL)
            goto err;
        if (extconf != NULL) {
            X509V3_set_nconf(&ext_ctx, extconf);
            if (!X509V3_EXT_REQ_add_nconf(extconf, &ext_ctx, extsect, rq)) {
                BIO_printf(bio_err,
                           "Error adding request extensions from section %s\n", extsect);
                goto err;
            }
        }
        if (!do_X509_REQ_sign(rq, privkey, digest, sigopts))
            goto err;
        if (!noout) {
            if (outformat == FORMAT_ASN1) {
                X509_REQ_print_ex(out, rq, get_nameopt(), X509_FLAG_COMPAT);
                i = i2d_X509_bio(out, x);
            } else {
                i = PEM_write_bio_X509_REQ(out, rq);
            }
            if (!i) {
                BIO_printf(bio_err,
                           "Unable to write certificate request\n");
                goto err;
            }
        }
        noout = 1;
    } else if (CAfile != NULL) {
        if ((CAkey = load_key(CAkeyfile, CAkeyformat,
                              0, passin, e, "CA private key")) == NULL)
            goto err;
        if (!X509_check_private_key(xca, CAkey)) {
            BIO_printf(bio_err,
                       "CA certificate and CA private key do not match\n");
            goto err;
        }

        if (!do_X509_sign(x, 0, CAkey, digest, sigopts, &ext_ctx))
            goto err;
    } else if (privkey != NULL) {
        if (!do_X509_sign(x, 0, privkey, digest, sigopts, &ext_ctx))
            goto err;
    }
    if (badsig) {
        const ASN1_BIT_STRING *signature;

        X509_get0_signature(&signature, NULL, x);
        corrupt_signature(signature);
    }

    /* Process print options in the given order, as indicated by index i */
    for (i = 1; i <= num; i++) {
        if (i == issuer) {
            print_name(out, "issuer=", X509_get_issuer_name(x));
        } else if (i == subject) {
            print_name(out, "subject=", X509_get_subject_name(x));
        } else if (i == serial) {
            BIO_printf(out, "serial=");
            i2a_ASN1_INTEGER(out, X509_get0_serialNumber(x));
            BIO_printf(out, "\n");
        } else if (i == next_serial) {
            ASN1_INTEGER *ser;
            BIGNUM *bnser = ASN1_INTEGER_to_BN(X509_get0_serialNumber(x), NULL);

            if (bnser == NULL)
                goto err;
            if (!BN_add_word(bnser, 1)
                    || (ser = BN_to_ASN1_INTEGER(bnser, NULL)) == NULL) {
                BN_free(bnser);
                goto err;
            }
            BN_free(bnser);
            i2a_ASN1_INTEGER(out, ser);
            ASN1_INTEGER_free(ser);
            BIO_puts(out, "\n");
        } else if (i == email || i == ocsp_uri) {
            STACK_OF(OPENSSL_STRING) *emlst =
                i == email ? X509_get1_email(x) : X509_get1_ocsp(x);

            for (j = 0; j < sk_OPENSSL_STRING_num(emlst); j++)
                BIO_printf(out, "%s\n", sk_OPENSSL_STRING_value(emlst, j));
            X509_email_free(emlst);
        } else if (i == aliasout) {
            unsigned char *alstr = X509_alias_get0(x, NULL);

            if (alstr)
                BIO_printf(out, "%s\n", alstr);
            else
                BIO_puts(out, "<No Alias>\n");
        } else if (i == subject_hash) {
            BIO_printf(out, "%08lx\n", X509_subject_name_hash(x));
#ifndef OPENSSL_NO_MD5
        } else if (i == subject_hash_old) {
            BIO_printf(out, "%08lx\n", X509_subject_name_hash_old(x));
#endif
        } else if (i == issuer_hash) {
            BIO_printf(out, "%08lx\n", X509_issuer_name_hash(x));
#ifndef OPENSSL_NO_MD5
        } else if (i == issuer_hash_old) {
            BIO_printf(out, "%08lx\n", X509_issuer_name_hash_old(x));
#endif
        } else if (i == pprint) {
            BIO_printf(out, "Certificate purposes:\n");
            for (j = 0; j < X509_PURPOSE_get_count(); j++)
                purpose_print(out, x, X509_PURPOSE_get0(j));
        } else if (i == modulus) {
            BIO_printf(out, "Modulus=");
            if (EVP_PKEY_is_a(pkey, "RSA") || EVP_PKEY_is_a(pkey, "RSA-PSS")) {
                BIGNUM *n = NULL;

                /* Every RSA key has an 'n' */
                EVP_PKEY_get_bn_param(pkey, "n", &n);
                BN_print(out, n);
                BN_free(n);
            } else if (EVP_PKEY_is_a(pkey, "DSA")) {
                BIGNUM *dsapub = NULL;

                /* Every DSA key has a 'pub' */
                EVP_PKEY_get_bn_param(pkey, "pub", &dsapub);
                BN_print(out, dsapub);
                BN_free(dsapub);
            } else {
                BIO_printf(out, "No modulus for this public key type");
            }
            BIO_printf(out, "\n");
        } else if (i == print_pubkey) {
            PEM_write_bio_PUBKEY(out, pkey);
        } else if (i == text) {
            X509_print_ex(out, x, get_nameopt(), certflag);
        } else if (i == startdate) {
            BIO_puts(out, "notBefore=");
            ASN1_TIME_print_ex(out, X509_get0_notBefore(x), dateopt);
            BIO_puts(out, "\n");
        } else if (i == enddate) {
            BIO_puts(out, "notAfter=");
            ASN1_TIME_print_ex(out, X509_get0_notAfter(x), dateopt);
            BIO_puts(out, "\n");
        } else if (i == fingerprint) {
            unsigned int n;
            unsigned char md[EVP_MAX_MD_SIZE];
            const char *fdigname = digest;
            EVP_MD *fdig;
            int digres;

            if (fdigname == NULL)
                fdigname = "SHA1";

            if ((fdig = EVP_MD_fetch(app_get0_libctx(), fdigname,
                                     app_get0_propq())) == NULL) {
                BIO_printf(bio_err, "Unknown digest\n");
                goto err;
            }
            digres = X509_digest(x, fdig, md, &n);
            EVP_MD_free(fdig);
            if (!digres) {
                BIO_printf(bio_err, "Out of memory\n");
                goto err;
            }

            BIO_printf(out, "%s Fingerprint=", fdigname);
            for (j = 0; j < (int)n; j++)
                BIO_printf(out, "%02X%c", md[j], (j + 1 == (int)n) ? '\n' : ':');
        } else if (i == ocspid) {
            X509_ocspid_print(out, x);
        } else if (i == ext) {
            print_x509v3_exts(out, x, ext_names);
        }
    }

    if (checkend) {
        time_t tcheck = time(NULL) + checkoffset;

        ret = X509_cmp_time(X509_get0_notAfter(x), &tcheck) < 0;
        if (ret)
            BIO_printf(out, "Certificate will expire\n");
        else
            BIO_printf(out, "Certificate will not expire\n");
        goto end;
    }

    if (!check_cert_attributes(out, x, checkhost, checkemail, checkip, 1))
        goto err;

    if (noout || nocert) {
        ret = 0;
        goto end;
    }

    if (outformat == FORMAT_ASN1) {
        i = i2d_X509_bio(out, x);
    } else if (outformat == FORMAT_PEM) {
        if (trustout)
            i = PEM_write_bio_X509_AUX(out, x);
        else
            i = PEM_write_bio_X509(out, x);
    } else {
        BIO_printf(bio_err, "Bad output format specified for outfile\n");
        goto err;
    }
    if (!i) {
        BIO_printf(bio_err, "Unable to write certificate\n");
        goto err;
    }

    ret = 0;
    goto end;

 err:
    ERR_print_errors(bio_err);

 end:
    NCONF_free(extconf);
    BIO_free_all(out);
    X509_STORE_free(ctx);
    X509_NAME_free(fissu);
    X509_NAME_free(fsubj);
    X509_REQ_free(req);
    X509_free(x);
    X509_free(xca);
    EVP_PKEY_free(privkey);
    EVP_PKEY_free(CAkey);
    EVP_PKEY_free(pubkey);
    sk_OPENSSL_STRING_free(sigopts);
    sk_OPENSSL_STRING_free(vfyopts);
    X509_REQ_free(rq);
    ASN1_INTEGER_free(sno);
    sk_ASN1_OBJECT_pop_free(trust, ASN1_OBJECT_free);
    sk_ASN1_OBJECT_pop_free(reject, ASN1_OBJECT_free);
    release_engine(e);
    clear_free(passin);
    return ret;
}

static ASN1_INTEGER *x509_load_serial(const char *CAfile,
                                      const char *serialfile, int create)
{
    char *buf = NULL;
    ASN1_INTEGER *bs = NULL;
    BIGNUM *serial = NULL;
    int defaultfile = 0, file_exists;

    if (serialfile == NULL) {
        const char *p = strrchr(CAfile, '.');
        size_t len = p != NULL ? (size_t)(p - CAfile) : strlen(CAfile);

        buf = app_malloc(len + sizeof(POSTFIX), "serial# buffer");
        memcpy(buf, CAfile, len);
        memcpy(buf + len, POSTFIX, sizeof(POSTFIX));
        serialfile = buf;
        defaultfile = 1;
    }

    serial = load_serial(serialfile, &file_exists, create || defaultfile, NULL);
    if (serial == NULL)
        goto end;

    if (!BN_add_word(serial, 1)) {
        BIO_printf(bio_err, "Serial number increment failure\n");
        goto end;
    }

    if (file_exists || create)
        save_serial(serialfile, NULL, serial, &bs);
    else
        bs = BN_to_ASN1_INTEGER(serial, NULL);

 end:
    OPENSSL_free(buf);
    BN_free(serial);
    return bs;
}

static int callb(int ok, X509_STORE_CTX *ctx)
{
    int err;
    X509 *err_cert;

    /*
     * It is ok to use a self-signed certificate. This case will catch both
     * the initial ok == 0 and the final ok == 1 calls to this function.
     */
    err = X509_STORE_CTX_get_error(ctx);
    if (err == X509_V_ERR_DEPTH_ZERO_SELF_SIGNED_CERT)
        return 1;

    if (!ok) {
        err_cert = X509_STORE_CTX_get_current_cert(ctx);
        print_name(bio_err, "subject=", X509_get_subject_name(err_cert));
        BIO_printf(bio_err,
                   "Error with certificate - error %d at depth %d\n%s\n", err,
                   X509_STORE_CTX_get_error_depth(ctx),
                   X509_verify_cert_error_string(err));
        return 1;
    }

    return 1;
}

static int purpose_print(BIO *bio, X509 *cert, X509_PURPOSE *pt)
{
    int id, i, idret;
    const char *pname;
    id = X509_PURPOSE_get_id(pt);
    pname = X509_PURPOSE_get0_name(pt);
    for (i = 0; i < 2; i++) {
        idret = X509_check_purpose(cert, id, i);
        BIO_printf(bio, "%s%s : ", pname, i ? " CA" : "");
        if (idret == 1)
            BIO_printf(bio, "Yes\n");
        else if (idret == 0)
            BIO_printf(bio, "No\n");
        else
            BIO_printf(bio, "Yes (WARNING code=%d)\n", idret);
    }
    return 1;
}

static int parse_ext_names(char *names, const char **result)
{
    char *p, *q;
    int cnt = 0, len = 0;

    p = q = names;
    len = strlen(names);

    while (q - names <= len) {
        if (*q != ',' && *q != '\0') {
            q++;
            continue;
        }
        if (p != q) {
            /* found */
            if (result != NULL) {
                result[cnt] = p;
                *q = '\0';
            }
            cnt++;
        }
        p = ++q;
    }

    return cnt;
}

static int print_x509v3_exts(BIO *bio, X509 *x, const char *ext_names)
{
    const STACK_OF(X509_EXTENSION) *exts = NULL;
    STACK_OF(X509_EXTENSION) *exts2 = NULL;
    X509_EXTENSION *ext = NULL;
    ASN1_OBJECT *obj;
    int i, j, ret = 0, num, nn = 0;
    const char *sn, **names = NULL;
    char *tmp_ext_names = NULL;

    exts = X509_get0_extensions(x);
    if ((num = sk_X509_EXTENSION_num(exts)) <= 0) {
        BIO_printf(bio_err, "No extensions in certificate\n");
        ret = 1;
        goto end;
    }

    /* parse comma separated ext name string */
    if ((tmp_ext_names = OPENSSL_strdup(ext_names)) == NULL)
        goto end;
    if ((nn = parse_ext_names(tmp_ext_names, NULL)) == 0) {
        BIO_printf(bio, "Invalid extension names: %s\n", ext_names);
        goto end;
    }
    if ((names = OPENSSL_malloc(sizeof(char *) * nn)) == NULL)
        goto end;
    parse_ext_names(tmp_ext_names, names);

    for (i = 0; i < num; i++) {
        ext = sk_X509_EXTENSION_value(exts, i);

        /* check if this ext is what we want */
        obj = X509_EXTENSION_get_object(ext);
        sn = OBJ_nid2sn(OBJ_obj2nid(obj));
        if (sn == NULL || strcmp(sn, "UNDEF") == 0)
            continue;

        for (j = 0; j < nn; j++) {
            if (strcmp(sn, names[j]) == 0) {
                /* push the extension into a new stack */
                if (exts2 == NULL
                    && (exts2 = sk_X509_EXTENSION_new_null()) == NULL)
                    goto end;
                if (!sk_X509_EXTENSION_push(exts2, ext))
                    goto end;
            }
        }
    }

    if (!sk_X509_EXTENSION_num(exts2)) {
        BIO_printf(bio, "No extensions matched with %s\n", ext_names);
        ret = 1;
        goto end;
    }

    ret = X509V3_extensions_print(bio, NULL, exts2, 0, 0);
 end:
    sk_X509_EXTENSION_free(exts2);
    OPENSSL_free(names);
    OPENSSL_free(tmp_ext_names);
    return ret;
}
