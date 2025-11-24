/*
 * Copyright 2015-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file is also used by the test suite. Do not #include "apps.h".
 */
#include "opt.h"
#include "fmt.h"
#include "app_libctx.h"
#include "internal/nelem.h"
#include "internal/numbers.h"
#include <string.h>
#if !defined(OPENSSL_SYS_MSDOS)
# include <unistd.h>
#endif

#include <stdlib.h>
#include <errno.h>
#include <ctype.h>
#include <limits.h>
#include <openssl/err.h>
#include <openssl/bio.h>
#include <openssl/x509v3.h>

#define MAX_OPT_HELP_WIDTH 30
const char OPT_HELP_STR[] = "-H";
const char OPT_MORE_STR[] = "-M";
const char OPT_SECTION_STR[] = "-S";
const char OPT_PARAM_STR[] = "-P";

/* Our state */
static char **argv;
static int argc;
static int opt_index;
static char *arg;
static char *flag;
static char *dunno;
static const char *unknown_name;
static const OPTIONS *unknown;
static const OPTIONS *opts;
static char prog[40];

/*
 * Return the simple name of the program; removing various platform gunk.
 */
#if defined(OPENSSL_SYS_WIN32)

const char *opt_path_end(const char *filename)
{
    const char *p;

    /* find the last '/', '\' or ':' */
    for (p = filename + strlen(filename); --p > filename; )
        if (*p == '/' || *p == '\\' || *p == ':') {
            p++;
            break;
        }
    return p;
}

char *opt_progname(const char *argv0)
{
    size_t i, n;
    const char *p;
    char *q;

    p = opt_path_end(argv0);

    /* Strip off trailing nonsense. */
    n = strlen(p);
    if (n > 4 &&
        (strcmp(&p[n - 4], ".exe") == 0 || strcmp(&p[n - 4], ".EXE") == 0))
        n -= 4;

    /* Copy over the name, in lowercase. */
    if (n > sizeof(prog) - 1)
        n = sizeof(prog) - 1;
    for (q = prog, i = 0; i < n; i++, p++)
        *q++ = tolower((unsigned char)*p);
    *q = '\0';
    return prog;
}

#elif defined(OPENSSL_SYS_VMS)

const char *opt_path_end(const char *filename)
{
    const char *p;

    /* Find last special character sys:[foo.bar]openssl */
    for (p = filename + strlen(filename); --p > filename;)
        if (*p == ':' || *p == ']' || *p == '>') {
            p++;
            break;
        }
    return p;
}

char *opt_progname(const char *argv0)
{
    const char *p, *q;

    /* Find last special character sys:[foo.bar]openssl */
    p = opt_path_end(argv0);
    q = strrchr(p, '.');
    if (prog != p)
        strncpy(prog, p, sizeof(prog) - 1);
    prog[sizeof(prog) - 1] = '\0';
    if (q != NULL && q - p < sizeof(prog))
        prog[q - p] = '\0';
    return prog;
}

#else

const char *opt_path_end(const char *filename)
{
    const char *p;

    /* Could use strchr, but this is like the ones above. */
    for (p = filename + strlen(filename); --p > filename;)
        if (*p == '/') {
            p++;
            break;
        }
    return p;
}

char *opt_progname(const char *argv0)
{
    const char *p;

    p = opt_path_end(argv0);
    if (prog != p)
        strncpy(prog, p, sizeof(prog) - 1);
    prog[sizeof(prog) - 1] = '\0';
    return prog;
}
#endif

char *opt_appname(const char *argv0)
{
    size_t len = strlen(prog);

    if (argv0 != NULL)
        BIO_snprintf(prog + len, sizeof(prog) - len - 1, " %s", argv0);
    return prog;
}

char *opt_getprog(void)
{
    return prog;
}

/* Set up the arg parsing. */
char *opt_init(int ac, char **av, const OPTIONS *o)
{
    /* Store state. */
    argc = ac;
    argv = av;
    opt_begin();
    opts = o;
    unknown = NULL;
    /* Make sure prog name is set for usage output */
    (void)opt_progname(argv[0]);

    /* Check all options up until the PARAM marker (if present) */
    for (; o->name != NULL && o->name != OPT_PARAM_STR; ++o) {
#ifndef NDEBUG
        const OPTIONS *next;
        int duplicated, i;
#endif

        if (o->name == OPT_HELP_STR
                || o->name == OPT_MORE_STR
                || o->name == OPT_SECTION_STR)
            continue;
#ifndef NDEBUG
        i = o->valtype;

        /* Make sure options are legit. */
        OPENSSL_assert(o->name[0] != '-');
        if (o->valtype == '.')
            OPENSSL_assert(o->retval == OPT_PARAM);
        else
            OPENSSL_assert(o->retval == OPT_DUP || o->retval > OPT_PARAM);
        switch (i) {
        case   0: case '-': case '.':
        case '/': case '<': case '>': case 'E': case 'F':
        case 'M': case 'U': case 'f': case 'l': case 'n': case 'p': case 's':
        case 'u': case 'c': case ':': case 'N': case 'A':
            break;
        default:
            OPENSSL_assert(0);
        }

        /* Make sure there are no duplicates. */
        for (next = o + 1; next->name; ++next) {
            /*
             * Some compilers inline strcmp and the assert string is too long.
             */
            duplicated = next->retval != OPT_DUP
                && strcmp(o->name, next->name) == 0;
            if (duplicated) {
                opt_printf_stderr("%s: Internal error: duplicate option %s\n",
                                  prog, o->name);
                OPENSSL_assert(!duplicated);
            }
        }
#endif
        if (o->name[0] == '\0') {
            OPENSSL_assert(unknown_name != NULL);
            OPENSSL_assert(unknown == NULL);
            unknown = o;
            OPENSSL_assert(unknown->valtype == 0 || unknown->valtype == '-');
        }
    }
    return prog;
}

static OPT_PAIR formats[] = {
    {"pem", OPT_FMT_PEM},
    {"der", OPT_FMT_DER},
    {"b64", OPT_FMT_B64},
    {"pkcs12", OPT_FMT_PKCS12},
    {"smime", OPT_FMT_SMIME},
    {"engine", OPT_FMT_ENGINE},
    {"msblob", OPT_FMT_MSBLOB},
    {"nss", OPT_FMT_NSS},
    {"text", OPT_FMT_TEXT},
    {"http", OPT_FMT_HTTP},
    {"pvk", OPT_FMT_PVK},
    {NULL}
};

void opt_set_unknown_name(const char *name)
{
    unknown_name = name;
}

/* Print an error message about a failed format parse. */
static int opt_format_error(const char *s, unsigned long flags)
{
    OPT_PAIR *ap;

    opt_printf_stderr("%s: Bad format \"%s\"; must be one of: ", prog, s);
    for (ap = formats; ap->name; ap++)
        if (flags & ap->retval)
            opt_printf_stderr(" %s", ap->name);
    opt_printf_stderr("\n");

    return 0;
}

/* Parse a format string, put it into *result; return 0 on failure, else 1. */
int opt_format(const char *s, unsigned long flags, int *result)
{
    switch (*s) {
    default:
        opt_printf_stderr("%s: Bad format \"%s\"\n", prog, s);
        return 0;
    case 'B':
    case 'b':
        if (s[1] == '\0'
            || strcmp(s, "B64") == 0 || strcmp(s, "b64") == 0
            || strcmp(s, "BASE64") == 0 || strcmp(s, "base64") == 0 ) {
            if ((flags & OPT_FMT_B64) == 0)
                return opt_format_error(s, flags);
            *result = FORMAT_BASE64;
        } else {
            return 0;
        }
        break;
    case 'D':
    case 'd':
        if ((flags & OPT_FMT_DER) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_ASN1;
        break;
    case 'T':
    case 't':
        if ((flags & OPT_FMT_TEXT) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_TEXT;
        break;
    case 'N':
    case 'n':
        if ((flags & OPT_FMT_NSS) == 0)
            return opt_format_error(s, flags);
        if (strcmp(s, "NSS") != 0 && strcmp(s, "nss") != 0)
            return opt_format_error(s, flags);
        *result = FORMAT_NSS;
        break;
    case 'S':
    case 's':
        if ((flags & OPT_FMT_SMIME) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_SMIME;
        break;
    case 'M':
    case 'm':
        if ((flags & OPT_FMT_MSBLOB) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_MSBLOB;
        break;
    case 'E':
    case 'e':
        if ((flags & OPT_FMT_ENGINE) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_ENGINE;
        break;
    case 'H':
    case 'h':
        if ((flags & OPT_FMT_HTTP) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_HTTP;
        break;
    case '1':
        if ((flags & OPT_FMT_PKCS12) == 0)
            return opt_format_error(s, flags);
        *result = FORMAT_PKCS12;
        break;
    case 'P':
    case 'p':
        if (s[1] == '\0' || strcmp(s, "PEM") == 0 || strcmp(s, "pem") == 0) {
            if ((flags & OPT_FMT_PEM) == 0)
                return opt_format_error(s, flags);
            *result = FORMAT_PEM;
        } else if (strcmp(s, "PVK") == 0 || strcmp(s, "pvk") == 0) {
            if ((flags & OPT_FMT_PVK) == 0)
                return opt_format_error(s, flags);
            *result = FORMAT_PVK;
        } else if (strcmp(s, "P12") == 0 || strcmp(s, "p12") == 0
                   || strcmp(s, "PKCS12") == 0 || strcmp(s, "pkcs12") == 0) {
            if ((flags & OPT_FMT_PKCS12) == 0)
                return opt_format_error(s, flags);
            *result = FORMAT_PKCS12;
        } else {
            opt_printf_stderr("%s: Bad format \"%s\"\n", prog, s);
            return 0;
        }
        break;
    }
    return 1;
}

/* Return string representing the given format. */
static const char *format2str(int format)
{
    switch (format) {
    default:
        return "(undefined)";
    case FORMAT_PEM:
        return "PEM";
    case FORMAT_ASN1:
        return "DER";
    case FORMAT_TEXT:
        return "TEXT";
    case FORMAT_NSS:
        return "NSS";
    case FORMAT_SMIME:
        return "SMIME";
    case FORMAT_MSBLOB:
        return "MSBLOB";
    case FORMAT_ENGINE:
        return "ENGINE";
    case FORMAT_HTTP:
        return "HTTP";
    case FORMAT_PKCS12:
        return "P12";
    case FORMAT_PVK:
        return "PVK";
    }
}

/* Print an error message about unsuitable/unsupported format requested. */
void print_format_error(int format, unsigned long flags)
{
    (void)opt_format_error(format2str(format), flags);
}

/*
 * Parse a cipher name, put it in *cipherp after freeing what was there, if
 * cipherp is not NULL.  Return 0 on failure, else 1.
 */
int opt_cipher_silent(const char *name, EVP_CIPHER **cipherp)
{
    EVP_CIPHER *c;

    ERR_set_mark();
    if ((c = EVP_CIPHER_fetch(app_get0_libctx(), name,
                              app_get0_propq())) != NULL
        || (opt_legacy_okay()
            && (c = (EVP_CIPHER *)EVP_get_cipherbyname(name)) != NULL)) {
        ERR_pop_to_mark();
        if (cipherp != NULL) {
            EVP_CIPHER_free(*cipherp);
            *cipherp = c;
        } else {
            EVP_CIPHER_free(c);
        }
        return 1;
    }
    ERR_clear_last_mark();
    return 0;
}

int opt_cipher_any(const char *name, EVP_CIPHER **cipherp)
{
    int ret;

    if (name == NULL)
         return 1;
    if ((ret = opt_cipher_silent(name, cipherp)) == 0)
        opt_printf_stderr("%s: Unknown option or cipher: %s\n", prog, name);
    return ret;
}

int opt_cipher(const char *name, EVP_CIPHER **cipherp)
{
     int mode, ret = 0;
     unsigned long int flags;
     EVP_CIPHER *c = NULL;

    if (name == NULL)
         return 1;
     if (opt_cipher_any(name, &c)) {
        mode = EVP_CIPHER_get_mode(c);
        flags = EVP_CIPHER_get_flags(c);
        if (mode == EVP_CIPH_XTS_MODE) {
            opt_printf_stderr("%s XTS ciphers not supported\n", prog);
        } else if ((flags & EVP_CIPH_FLAG_AEAD_CIPHER) != 0) {
            opt_printf_stderr("%s: AEAD ciphers not supported\n", prog);
        } else {
            ret = 1;
            if (cipherp != NULL)
                *cipherp = c;
        }
    }
    return ret;
}

/*
 * Parse message digest name, put it in *EVP_MD; return 0 on failure, else 1.
 */
int opt_md_silent(const char *name, EVP_MD **mdp)
{
    EVP_MD *md;

    ERR_set_mark();
    if ((md = EVP_MD_fetch(app_get0_libctx(), name, app_get0_propq())) != NULL
        || (opt_legacy_okay()
            && (md = (EVP_MD *)EVP_get_digestbyname(name)) != NULL)) {
        ERR_pop_to_mark();
        if (mdp != NULL) {
            EVP_MD_free(*mdp);
            *mdp = md;
        } else {
            EVP_MD_free(md);
        }
        return 1;
    }
    ERR_clear_last_mark();
    return 0;
}

int opt_md(const char *name, EVP_MD **mdp)
{
    int ret;

    if (name == NULL)
        return 1;
    if ((ret = opt_md_silent(name, mdp)) == 0)
        opt_printf_stderr("%s: Unknown option or message digest: %s\n",
                          prog, name);
    return ret;
}

int opt_check_md(const char *name)
{
    if (opt_md(name, NULL))
        return 1;
    ERR_clear_error();
    return 0;
}

/* Look through a list of name/value pairs. */
int opt_pair(const char *name, const OPT_PAIR* pairs, int *result)
{
    const OPT_PAIR *pp;

    for (pp = pairs; pp->name; pp++)
        if (strcmp(pp->name, name) == 0) {
            *result = pp->retval;
            return 1;
        }
    opt_printf_stderr("%s: Value must be one of:\n", prog);
    for (pp = pairs; pp->name; pp++)
        opt_printf_stderr("\t%s\n", pp->name);
    return 0;
}

/* Look through a list of valid names */
int opt_string(const char *name, const char **options)
{
    const char **p;

    for (p = options; *p != NULL; p++)
        if (strcmp(*p, name) == 0)
            return 1;
    opt_printf_stderr("%s: Value must be one of:\n", prog);
    for (p = options; *p != NULL; p++)
        opt_printf_stderr("\t%s\n", *p);
    return 0;
}

/* Parse an int, put it into *result; return 0 on failure, else 1. */
int opt_int(const char *value, int *result)
{
    long l;

    if (!opt_long(value, &l))
        return 0;
    *result = (int)l;
    if (*result != l) {
        opt_printf_stderr("%s: Value \"%s\" outside integer range\n",
                          prog, value);
        return 0;
    }
    return 1;
}

/* Parse and return an integer, assuming range has been checked before. */
int opt_int_arg(void)
{
    int result = -1;

    (void)opt_int(arg, &result);
    return result;
}

static void opt_number_error(const char *v)
{
    size_t i = 0;
    struct strstr_pair_st {
        char *prefix;
        char *name;
    } b[] = {
        {"0x", "a hexadecimal"},
        {"0X", "a hexadecimal"},
        {"0", "an octal"}
    };

    for (i = 0; i < OSSL_NELEM(b); i++) {
        if (strncmp(v, b[i].prefix, strlen(b[i].prefix)) == 0) {
            opt_printf_stderr("%s: Can't parse \"%s\" as %s number\n",
                              prog, v, b[i].name);
            return;
        }
    }
    opt_printf_stderr("%s: Can't parse \"%s\" as a number\n", prog, v);
    return;
}

/* Parse a long, put it into *result; return 0 on failure, else 1. */
int opt_long(const char *value, long *result)
{
    int oerrno = errno;
    long l;
    char *endp;

    errno = 0;
    l = strtol(value, &endp, 0);
    if (*endp
            || endp == value
            || ((l == LONG_MAX || l == LONG_MIN) && errno == ERANGE)
            || (l == 0 && errno != 0)) {
        opt_number_error(value);
        errno = oerrno;
        return 0;
    }
    *result = l;
    errno = oerrno;
    return 1;
}

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 199901L && \
    defined(INTMAX_MAX) && defined(UINTMAX_MAX) && \
    !defined(OPENSSL_NO_INTTYPES_H)

/* Parse an intmax_t, put it into *result; return 0 on failure, else 1. */
int opt_intmax(const char *value, ossl_intmax_t *result)
{
    int oerrno = errno;
    intmax_t m;
    char *endp;

    errno = 0;
    m = strtoimax(value, &endp, 0);
    if (*endp
            || endp == value
            || ((m == INTMAX_MAX || m == INTMAX_MIN)
                && errno == ERANGE)
            || (m == 0 && errno != 0)) {
        opt_number_error(value);
        errno = oerrno;
        return 0;
    }
    /* Ensure that the value in |m| is never too big for |*result| */
    if (sizeof(m) > sizeof(*result)
        && (m < OSSL_INTMAX_MIN || m > OSSL_INTMAX_MAX)) {
        opt_number_error(value);
        return 0;
    }
    *result = (ossl_intmax_t)m;
    errno = oerrno;
    return 1;
}

/* Parse a uintmax_t, put it into *result; return 0 on failure, else 1. */
int opt_uintmax(const char *value, ossl_uintmax_t *result)
{
    int oerrno = errno;
    uintmax_t m;
    char *endp;

    errno = 0;
    m = strtoumax(value, &endp, 0);
    if (*endp
            || endp == value
            || (m == UINTMAX_MAX && errno == ERANGE)
            || (m == 0 && errno != 0)) {
        opt_number_error(value);
        errno = oerrno;
        return 0;
    }
    /* Ensure that the value in |m| is never too big for |*result| */
    if (sizeof(m) > sizeof(*result)
        && m > OSSL_UINTMAX_MAX) {
        opt_number_error(value);
        return 0;
    }
    *result = (ossl_uintmax_t)m;
    errno = oerrno;
    return 1;
}
#else
/* Fallback implementations based on long */
int opt_intmax(const char *value, ossl_intmax_t *result)
{
    long m;
    int ret;

    if ((ret = opt_long(value, &m)))
        *result = m;
    return ret;
}

int opt_uintmax(const char *value, ossl_uintmax_t *result)
{
    unsigned long m;
    int ret;

    if ((ret = opt_ulong(value, &m)))
        *result = m;
    return ret;
}
#endif

/*
 * Parse an unsigned long, put it into *result; return 0 on failure, else 1.
 */
int opt_ulong(const char *value, unsigned long *result)
{
    int oerrno = errno;
    char *endptr;
    unsigned long l;

    errno = 0;
    l = strtoul(value, &endptr, 0);
    if (*endptr
            || endptr == value
            || ((l == ULONG_MAX) && errno == ERANGE)
            || (l == 0 && errno != 0)) {
        opt_number_error(value);
        errno = oerrno;
        return 0;
    }
    *result = l;
    errno = oerrno;
    return 1;
}

/*
 * We pass opt as an int but cast it to "enum range" so that all the
 * items in the OPT_V_ENUM enumeration are caught; this makes -Wswitch
 * in gcc do the right thing.
 */
enum range { OPT_V_ENUM };

int opt_verify(int opt, X509_VERIFY_PARAM *vpm)
{
    int i;
    ossl_intmax_t t = 0;
    ASN1_OBJECT *otmp;
    X509_PURPOSE *xptmp;
    const X509_VERIFY_PARAM *vtmp;

    OPENSSL_assert(vpm != NULL);
    OPENSSL_assert(opt > OPT_V__FIRST);
    OPENSSL_assert(opt < OPT_V__LAST);

    switch ((enum range)opt) {
    case OPT_V__FIRST:
    case OPT_V__LAST:
        return 0;
    case OPT_V_POLICY:
        otmp = OBJ_txt2obj(opt_arg(), 0);
        if (otmp == NULL) {
            opt_printf_stderr("%s: Invalid Policy %s\n", prog, opt_arg());
            return 0;
        }
        if (!X509_VERIFY_PARAM_add0_policy(vpm, otmp)) {
            ASN1_OBJECT_free(otmp);
            opt_printf_stderr("%s: Internal error adding Policy %s\n",
                              prog, opt_arg());
            return 0;
        }
        break;
    case OPT_V_PURPOSE:
        /* purpose name -> purpose index */
        i = X509_PURPOSE_get_by_sname(opt_arg());
        if (i < 0) {
            opt_printf_stderr("%s: Invalid purpose %s\n", prog, opt_arg());
            return 0;
        }

        /* purpose index -> purpose object */
        xptmp = X509_PURPOSE_get0(i);

        /* purpose object -> purpose value */
        i = X509_PURPOSE_get_id(xptmp);

        if (!X509_VERIFY_PARAM_set_purpose(vpm, i)) {
            opt_printf_stderr("%s: Internal error setting purpose %s\n",
                              prog, opt_arg());
            return 0;
        }
        break;
    case OPT_V_VERIFY_NAME:
        vtmp = X509_VERIFY_PARAM_lookup(opt_arg());
        if (vtmp == NULL) {
            opt_printf_stderr("%s: Invalid verify name %s\n",
                              prog, opt_arg());
            return 0;
        }
        X509_VERIFY_PARAM_set1(vpm, vtmp);
        break;
    case OPT_V_VERIFY_DEPTH:
        i = atoi(opt_arg());
        if (i >= 0)
            X509_VERIFY_PARAM_set_depth(vpm, i);
        break;
    case OPT_V_VERIFY_AUTH_LEVEL:
        i = atoi(opt_arg());
        if (i >= 0)
            X509_VERIFY_PARAM_set_auth_level(vpm, i);
        break;
    case OPT_V_ATTIME:
        if (!opt_intmax(opt_arg(), &t))
            return 0;
        if (t != (time_t)t) {
            opt_printf_stderr("%s: epoch time out of range %s\n",
                              prog, opt_arg());
            return 0;
        }
        X509_VERIFY_PARAM_set_time(vpm, (time_t)t);
        break;
    case OPT_V_VERIFY_HOSTNAME:
        if (!X509_VERIFY_PARAM_set1_host(vpm, opt_arg(), 0))
            return 0;
        break;
    case OPT_V_VERIFY_EMAIL:
        if (!X509_VERIFY_PARAM_set1_email(vpm, opt_arg(), 0))
            return 0;
        break;
    case OPT_V_VERIFY_IP:
        if (!X509_VERIFY_PARAM_set1_ip_asc(vpm, opt_arg()))
            return 0;
        break;
    case OPT_V_IGNORE_CRITICAL:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_IGNORE_CRITICAL);
        break;
    case OPT_V_ISSUER_CHECKS:
        /* NOP, deprecated */
        break;
    case OPT_V_CRL_CHECK:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_CRL_CHECK);
        break;
    case OPT_V_CRL_CHECK_ALL:
        X509_VERIFY_PARAM_set_flags(vpm,
                                    X509_V_FLAG_CRL_CHECK |
                                    X509_V_FLAG_CRL_CHECK_ALL);
        break;
    case OPT_V_POLICY_CHECK:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_POLICY_CHECK);
        break;
    case OPT_V_EXPLICIT_POLICY:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_EXPLICIT_POLICY);
        break;
    case OPT_V_INHIBIT_ANY:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_INHIBIT_ANY);
        break;
    case OPT_V_INHIBIT_MAP:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_INHIBIT_MAP);
        break;
    case OPT_V_X509_STRICT:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_X509_STRICT);
        break;
    case OPT_V_EXTENDED_CRL:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_EXTENDED_CRL_SUPPORT);
        break;
    case OPT_V_USE_DELTAS:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_USE_DELTAS);
        break;
    case OPT_V_POLICY_PRINT:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_NOTIFY_POLICY);
        break;
    case OPT_V_CHECK_SS_SIG:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_CHECK_SS_SIGNATURE);
        break;
    case OPT_V_TRUSTED_FIRST:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_TRUSTED_FIRST);
        break;
    case OPT_V_SUITEB_128_ONLY:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_SUITEB_128_LOS_ONLY);
        break;
    case OPT_V_SUITEB_128:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_SUITEB_128_LOS);
        break;
    case OPT_V_SUITEB_192:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_SUITEB_192_LOS);
        break;
    case OPT_V_PARTIAL_CHAIN:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_PARTIAL_CHAIN);
        break;
    case OPT_V_NO_ALT_CHAINS:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_NO_ALT_CHAINS);
        break;
    case OPT_V_NO_CHECK_TIME:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_NO_CHECK_TIME);
        break;
    case OPT_V_ALLOW_PROXY_CERTS:
        X509_VERIFY_PARAM_set_flags(vpm, X509_V_FLAG_ALLOW_PROXY_CERTS);
        break;
    }
    return 1;

}

void opt_begin(void)
{
    opt_index = 1;
    arg = NULL;
    flag = NULL;
}

/*
 * Parse the next flag (and value if specified), return 0 if done, -1 on
 * error, otherwise the flag's retval.
 */
int opt_next(void)
{
    char *p;
    const OPTIONS *o;
    int ival;
    long lval;
    unsigned long ulval;
    ossl_intmax_t imval;
    ossl_uintmax_t umval;

    /* Look at current arg; at end of the list? */
    arg = NULL;
    p = argv[opt_index];
    if (p == NULL)
        return 0;

    /* If word doesn't start with a -, we're done. */
    if (*p != '-')
        return 0;

    /* Hit "--" ? We're done. */
    opt_index++;
    if (strcmp(p, "--") == 0)
        return 0;

    /* Allow -nnn and --nnn */
    if (*++p == '-')
        p++;
    flag = p - 1;

    /* If we have --flag=foo, snip it off */
    if ((arg = strchr(p, '=')) != NULL)
        *arg++ = '\0';
    for (o = opts; o->name; ++o) {
        /* If not this option, move on to the next one. */
        if (!(strcmp(p, "h") == 0 && strcmp(o->name, "help") == 0)
                && strcmp(p, o->name) != 0)
            continue;

        /* If it doesn't take a value, make sure none was given. */
        if (o->valtype == 0 || o->valtype == '-') {
            if (arg) {
                opt_printf_stderr("%s: Option -%s does not take a value\n",
                                  prog, p);
                return -1;
            }
            return o->retval;
        }

        /* Want a value; get the next param if =foo not used. */
        if (arg == NULL) {
            if (argv[opt_index] == NULL) {
                opt_printf_stderr("%s: Option -%s needs a value\n",
                                  prog, o->name);
                return -1;
            }
            arg = argv[opt_index++];
        }

        /* Syntax-check value. */
        switch (o->valtype) {
        default:
        case 's':
        case ':':
            /* Just a string. */
            break;
        case '.':
            /* Parameters */
            break;
        case '/':
            if (opt_isdir(arg) > 0)
                break;
            opt_printf_stderr("%s: Not a directory: %s\n", prog, arg);
            return -1;
        case '<':
            /* Input file. */
            break;
        case '>':
            /* Output file. */
            break;
        case 'p':
        case 'n':
        case 'N':
            if (!opt_int(arg, &ival))
                return -1;
            if (o->valtype == 'p' && ival <= 0) {
                opt_printf_stderr("%s: Non-positive number \"%s\" for option -%s\n",
                                  prog, arg, o->name);
                return -1;
            }
            if (o->valtype == 'N' && ival < 0) {
                opt_printf_stderr("%s: Negative number \"%s\" for option -%s\n",
                                  prog, arg, o->name);
                return -1;
            }
            break;
        case 'M':
            if (!opt_intmax(arg, &imval))
                return -1;
            break;
        case 'U':
            if (!opt_uintmax(arg, &umval))
                return -1;
            break;
        case 'l':
            if (!opt_long(arg, &lval))
                return -1;
            break;
        case 'u':
            if (!opt_ulong(arg, &ulval))
                return -1;
            break;
        case 'c':
        case 'E':
        case 'F':
        case 'f':
        case 'A':
        case 'a':
            if (opt_format(arg,
                           o->valtype == 'c' ? OPT_FMT_PDS :
                           o->valtype == 'E' ? OPT_FMT_PDE :
                           o->valtype == 'F' ? OPT_FMT_PEMDER :
                           o->valtype == 'A' ? OPT_FMT_ASN1 :
                           OPT_FMT_ANY, &ival))
                break;
            opt_printf_stderr("%s: Invalid format \"%s\" for option -%s\n",
                              prog, arg, o->name);
            return -1;
        }

        /* Return the flag value. */
        return o->retval;
    }
    if (unknown != NULL) {
        if (dunno != NULL) {
            opt_printf_stderr("%s: Multiple %s or unknown options: -%s and -%s\n",
                              prog, unknown_name, dunno, p);
            return -1;
        }
        dunno = p;
        return unknown->retval;
    }
    opt_printf_stderr("%s: Unknown option: -%s\n", prog, p);
    return -1;
}

/* Return the most recent flag parameter. */
char *opt_arg(void)
{
    return arg;
}

/* Return the most recent flag (option name including the preceding '-'). */
char *opt_flag(void)
{
    return flag;
}

/* Return the unknown option. */
char *opt_unknown(void)
{
    return dunno;
}

/* Reset the unknown option; needed by ocsp to allow multiple digest options. */
void reset_unknown(void)
{
    dunno = NULL;
}

/* Return the rest of the arguments after parsing flags. */
char **opt_rest(void)
{
    return &argv[opt_index];
}

/* How many items in remaining args? */
int opt_num_rest(void)
{
    int i = 0;
    char **pp;

    for (pp = opt_rest(); *pp; pp++, i++)
        continue;
    return i;
}

int opt_check_rest_arg(const char *expected)
{
    char *opt = *opt_rest();

    if (opt == NULL || *opt == '\0') {
        if (expected == NULL)
            return 1;
        opt_printf_stderr("%s: Missing argument: %s\n", prog, expected);
        return 0;
    }
    if (expected != NULL) {
        opt = argv[opt_index + 1];
        if (opt == NULL || *opt == '\0')
            return 1;
        opt_printf_stderr("%s: Extra argument after %s: \"%s\"\n", prog, expected, opt);
        return 0;
    }
    if (opt_unknown() == NULL)
        opt_printf_stderr("%s: Extra option: \"%s\"\n", prog, opt);
    else
        opt_printf_stderr("%s: Extra (unknown) options: \"%s\" \"%s\"\n",
                          prog, opt_unknown(), opt);
    return 0;
}

/* Return a string describing the parameter type. */
static const char *valtype2param(const OPTIONS *o)
{
    switch (o->valtype) {
    case 0:
    case '-':
        return "";
    case ':':
        return "uri";
    case 's':
        return "val";
    case '/':
        return "dir";
    case '<':
        return "infile";
    case '>':
        return "outfile";
    case 'p':
        return "+int";
    case 'n':
        return "int";
    case 'l':
        return "long";
    case 'u':
        return "ulong";
    case 'E':
        return "PEM|DER|ENGINE";
    case 'F':
        return "PEM|DER";
    case 'f':
        return "format";
    case 'M':
        return "intmax";
    case 'N':
        return "nonneg";
    case 'U':
        return "uintmax";
    }
    return "parm";
}

static void opt_print(const OPTIONS *o, int doingparams, int width)
{
    const char* help;
    char start[80 + 1];
    int linelen, printlen;

    /* Avoid OOB if width is beyond the buffer size of start */
    if (width >= (int)sizeof(start))
        width = (int)sizeof(start) - 1;

    help = o->helpstr ? o->helpstr : "(No additional info)";
    if (o->name == OPT_HELP_STR) {
        opt_printf_stderr(help, prog);
        return;
    } else if (o->name == OPT_SECTION_STR) {
        opt_printf_stderr("\n");
        opt_printf_stderr(help, prog);
        return;
    } else if (o->name == OPT_PARAM_STR) {
        opt_printf_stderr("\nParameters:\n");
        return;
    }

    /* Pad out prefix */
    memset(start, ' ', sizeof(start) - 1);
    start[sizeof(start) - 1] = '\0';

    if (o->name == OPT_MORE_STR) {
        /* Continuation of previous line; pad and print. */
        start[width] = '\0';
        opt_printf_stderr("%s  %s\n", start, help);
        return;
    }

    /* Build up the "-flag [param]" part. */
    linelen = 0;

    printlen = opt_printf_stderr(" %s", !doingparams ? "-" : "");
    linelen += (printlen > 0) ? printlen : MAX_OPT_HELP_WIDTH;

    printlen = opt_printf_stderr("%s" , o->name[0] ? o->name : "*");
    linelen += (printlen > 0) ? printlen : MAX_OPT_HELP_WIDTH;

    if (o->valtype != '-') {
        printlen = opt_printf_stderr(" %s" , valtype2param(o));
        linelen += (printlen > 0) ? printlen : MAX_OPT_HELP_WIDTH;
    }

    if (linelen >= MAX_OPT_HELP_WIDTH || linelen > width) {
        opt_printf_stderr("%s", "\n");
        memset(start, ' ', sizeof(start));
        linelen = 0;
    }

    width -= linelen;

    start[width] = '\0';
    opt_printf_stderr("%s  %s\n", start, help);
}

void opt_help(const OPTIONS *list)
{
    const OPTIONS *o;
    int i, sawparams = 0, width = 5;
    int standard_prolog;

    /* Starts with its own help message? */
    standard_prolog = list[0].name != OPT_HELP_STR;

    /* Find the widest help. */
    for (o = list; o->name; o++) {
        if (o->name == OPT_MORE_STR)
            continue;

        i = 2 + (int)strlen(o->name);
        if (o->valtype != '-')
            i += 1 + strlen(valtype2param(o));

        if (i > width)
            width = i;
    }

    if (width > MAX_OPT_HELP_WIDTH)
        width = MAX_OPT_HELP_WIDTH;

    if (standard_prolog) {
        opt_printf_stderr("Usage: %s [options]\n", prog);
        if (list[0].name != OPT_SECTION_STR)
            opt_printf_stderr("Valid options are:\n", prog);
    }

    /* Now let's print. */
    for (o = list; o->name; o++) {
        if (o->name == OPT_PARAM_STR)
            sawparams = 1;
        opt_print(o, sawparams, width);
    }
}

/* opt_isdir section */
#ifdef _WIN32
# include <windows.h>
int opt_isdir(const char *name)
{
    DWORD attr;
# if defined(UNICODE) || defined(_UNICODE)
    size_t i, len_0 = strlen(name) + 1;
    WCHAR tempname[MAX_PATH];

    if (len_0 > MAX_PATH)
        return -1;

#  if !defined(_WIN32_WCE) || _WIN32_WCE>=101
    if (!MultiByteToWideChar(CP_ACP, 0, name, len_0, tempname, MAX_PATH))
#  endif
        for (i = 0; i < len_0; i++)
            tempname[i] = (WCHAR)name[i];

    attr = GetFileAttributes(tempname);
# else
    attr = GetFileAttributes(name);
# endif
    if (attr == INVALID_FILE_ATTRIBUTES)
        return -1;
    return ((attr & FILE_ATTRIBUTE_DIRECTORY) != 0);
}
#else
# include <sys/stat.h>
# ifndef S_ISDIR
#  if defined(_S_IFMT) && defined(_S_IFDIR)
#   define S_ISDIR(a)   (((a) & _S_IFMT) == _S_IFDIR)
#  else
#   define S_ISDIR(a)   (((a) & S_IFMT) == S_IFDIR)
#  endif
# endif

int opt_isdir(const char *name)
{
# if defined(S_ISDIR)
    struct stat st;

    if (stat(name, &st) == 0)
        return S_ISDIR(st.st_mode);
    else
        return -1;
# else
    return -1;
# endif
}
#endif
