/*
 * Copyright 2012-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>

#include <openssl/e_os2.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "internal/nelem.h"
#include "testutil.h"

static const char *const names[] = {
    "a", "b", ".", "*", "@",
    ".a", "a.", ".b", "b.", ".*", "*.", "*@", "@*", "a@", "@a", "b@", "..",
    "-example.com", "example-.com",
    "@@", "**", "*.com", "*com", "*.*.com", "*com", "com*", "*example.com",
    "*@example.com", "test@*.example.com", "example.com", "www.example.com",
    "test.www.example.com", "*.example.com", "*.www.example.com",
    "test.*.example.com", "www.*.com",
    ".www.example.com", "*www.example.com",
    "example.net", "xn--rger-koa.example.com",
    "*.xn--rger-koa.example.com", "www.xn--rger-koa.example.com",
    "*.good--example.com", "www.good--example.com",
    "*.xn--bar.com", "xn--foo.xn--bar.com",
    "a.example.com", "b.example.com",
    "postmaster@example.com", "Postmaster@example.com",
    "postmaster@EXAMPLE.COM",
    NULL
};

static const char *const exceptions[] = {
    "set CN: host: [*.example.com] matches [a.example.com]",
    "set CN: host: [*.example.com] matches [b.example.com]",
    "set CN: host: [*.example.com] matches [www.example.com]",
    "set CN: host: [*.example.com] matches [xn--rger-koa.example.com]",
    "set CN: host: [*.www.example.com] matches [test.www.example.com]",
    "set CN: host: [*.www.example.com] matches [.www.example.com]",
    "set CN: host: [*www.example.com] matches [www.example.com]",
    "set CN: host: [test.www.example.com] matches [.www.example.com]",
    "set CN: host: [*.xn--rger-koa.example.com] matches [www.xn--rger-koa.example.com]",
    "set CN: host: [*.xn--bar.com] matches [xn--foo.xn--bar.com]",
    "set CN: host: [*.good--example.com] matches [www.good--example.com]",
    "set CN: host-no-wildcards: [*.www.example.com] matches [.www.example.com]",
    "set CN: host-no-wildcards: [test.www.example.com] matches [.www.example.com]",
    "set emailAddress: email: [postmaster@example.com] does not match [Postmaster@example.com]",
    "set emailAddress: email: [postmaster@EXAMPLE.COM] does not match [Postmaster@example.com]",
    "set emailAddress: email: [Postmaster@example.com] does not match [postmaster@example.com]",
    "set emailAddress: email: [Postmaster@example.com] does not match [postmaster@EXAMPLE.COM]",
    "set dnsName: host: [*.example.com] matches [www.example.com]",
    "set dnsName: host: [*.example.com] matches [a.example.com]",
    "set dnsName: host: [*.example.com] matches [b.example.com]",
    "set dnsName: host: [*.example.com] matches [xn--rger-koa.example.com]",
    "set dnsName: host: [*.www.example.com] matches [test.www.example.com]",
    "set dnsName: host-no-wildcards: [*.www.example.com] matches [.www.example.com]",
    "set dnsName: host-no-wildcards: [test.www.example.com] matches [.www.example.com]",
    "set dnsName: host: [*.www.example.com] matches [.www.example.com]",
    "set dnsName: host: [*www.example.com] matches [www.example.com]",
    "set dnsName: host: [test.www.example.com] matches [.www.example.com]",
    "set dnsName: host: [*.xn--rger-koa.example.com] matches [www.xn--rger-koa.example.com]",
    "set dnsName: host: [*.xn--bar.com] matches [xn--foo.xn--bar.com]",
    "set dnsName: host: [*.good--example.com] matches [www.good--example.com]",
    "set rfc822Name: email: [postmaster@example.com] does not match [Postmaster@example.com]",
    "set rfc822Name: email: [Postmaster@example.com] does not match [postmaster@example.com]",
    "set rfc822Name: email: [Postmaster@example.com] does not match [postmaster@EXAMPLE.COM]",
    "set rfc822Name: email: [postmaster@EXAMPLE.COM] does not match [Postmaster@example.com]",
    NULL
};

static int is_exception(const char *msg)
{
    const char *const *p;

    for (p = exceptions; *p; ++p)
        if (strcmp(msg, *p) == 0)
            return 1;
    return 0;
}

static int set_cn(X509 *crt, ...)
{
    int ret = 0;
    X509_NAME *n = NULL;
    va_list ap;

    va_start(ap, crt);
    n = X509_NAME_new();
    if (n == NULL)
        goto out;

    while (1) {
        int nid;
        const char *name;

        nid = va_arg(ap, int);
        if (nid == 0)
            break;
        name = va_arg(ap, const char *);
        if (!X509_NAME_add_entry_by_NID(n, nid, MBSTRING_ASC,
                                        (unsigned char *)name, -1, -1, 1))
            goto out;
    }
    if (!X509_set_subject_name(crt, n))
        goto out;
    ret = 1;
 out:
    X509_NAME_free(n);
    va_end(ap);
    return ret;
}

/*-
int             X509_add_ext(X509 *x, X509_EXTENSION *ex, int loc);
X509_EXTENSION *X509_EXTENSION_create_by_NID(X509_EXTENSION **ex,
                        int nid, int crit, ASN1_OCTET_STRING *data);
int             X509_add_ext(X509 *x, X509_EXTENSION *ex, int loc);
*/

static int set_altname(X509 *crt, ...)
{
    int ret = 0;
    GENERAL_NAMES *gens = NULL;
    GENERAL_NAME *gen = NULL;
    ASN1_IA5STRING *ia5 = NULL;
    va_list ap;
    va_start(ap, crt);
    gens = sk_GENERAL_NAME_new_null();
    if (gens == NULL)
        goto out;
    while (1) {
        int type;
        const char *name;
        type = va_arg(ap, int);
        if (type == 0)
            break;
        name = va_arg(ap, const char *);

        gen = GENERAL_NAME_new();
        if (gen == NULL)
            goto out;
        ia5 = ASN1_IA5STRING_new();
        if (ia5 == NULL)
            goto out;
        if (!ASN1_STRING_set(ia5, name, -1))
            goto out;
        switch (type) {
        case GEN_EMAIL:
        case GEN_DNS:
            GENERAL_NAME_set0_value(gen, type, ia5);
            ia5 = NULL;
            break;
        default:
            abort();
        }
        if (!sk_GENERAL_NAME_push(gens, gen))
            goto out;
        gen = NULL;
    }
    if (!X509_add1_ext_i2d(crt, NID_subject_alt_name, gens, 0, 0))
        goto out;
    ret = 1;
 out:
    ASN1_IA5STRING_free(ia5);
    GENERAL_NAME_free(gen);
    GENERAL_NAMES_free(gens);
    va_end(ap);
    return ret;
}

static int set_cn1(X509 *crt, const char *name)
{
    return set_cn(crt, NID_commonName, name, 0);
}

static int set_cn_and_email(X509 *crt, const char *name)
{
    return set_cn(crt, NID_commonName, name,
                  NID_pkcs9_emailAddress, "dummy@example.com", 0);
}

static int set_cn2(X509 *crt, const char *name)
{
    return set_cn(crt, NID_commonName, "dummy value",
                  NID_commonName, name, 0);
}

static int set_cn3(X509 *crt, const char *name)
{
    return set_cn(crt, NID_commonName, name,
                  NID_commonName, "dummy value", 0);
}

static int set_email1(X509 *crt, const char *name)
{
    return set_cn(crt, NID_pkcs9_emailAddress, name, 0);
}

static int set_email2(X509 *crt, const char *name)
{
    return set_cn(crt, NID_pkcs9_emailAddress, "dummy@example.com",
                  NID_pkcs9_emailAddress, name, 0);
}

static int set_email3(X509 *crt, const char *name)
{
    return set_cn(crt, NID_pkcs9_emailAddress, name,
                  NID_pkcs9_emailAddress, "dummy@example.com", 0);
}

static int set_email_and_cn(X509 *crt, const char *name)
{
    return set_cn(crt, NID_pkcs9_emailAddress, name,
                  NID_commonName, "www.example.org", 0);
}

static int set_altname_dns(X509 *crt, const char *name)
{
    return set_altname(crt, GEN_DNS, name, 0);
}

static int set_altname_email(X509 *crt, const char *name)
{
    return set_altname(crt, GEN_EMAIL, name, 0);
}

struct set_name_fn {
    int (*fn) (X509 *, const char *);
    const char *name;
    int host;
    int email;
};

static const struct set_name_fn name_fns[] = {
    {set_cn1, "set CN", 1, 0},
    {set_cn2, "set CN", 1, 0},
    {set_cn3, "set CN", 1, 0},
    {set_cn_and_email, "set CN", 1, 0},
    {set_email1, "set emailAddress", 0, 1},
    {set_email2, "set emailAddress", 0, 1},
    {set_email3, "set emailAddress", 0, 1},
    {set_email_and_cn, "set emailAddress", 0, 1},
    {set_altname_dns, "set dnsName", 1, 0},
    {set_altname_email, "set rfc822Name", 0, 1},
};

static X509 *make_cert(void)
{
    X509 *crt = NULL;

    if (!TEST_ptr(crt = X509_new()))
        return NULL;
    if (!TEST_true(X509_set_version(crt, X509_VERSION_3))) {
        X509_free(crt);
        return NULL;
    }
    return crt;
}

static int check_message(const struct set_name_fn *fn, const char *op,
                         const char *nameincert, int match, const char *name)
{
    char msg[1024];

    if (match < 0)
        return 1;
    BIO_snprintf(msg, sizeof(msg), "%s: %s: [%s] %s [%s]",
                 fn->name, op, nameincert,
                 match ? "matches" : "does not match", name);
    if (is_exception(msg))
        return 1;
    TEST_error("%s", msg);
    return 0;
}

static int run_cert(X509 *crt, const char *nameincert,
                     const struct set_name_fn *fn)
{
    const char *const *pname = names;
    int failed = 0;

    for (; *pname != NULL; ++pname) {
        int samename = OPENSSL_strcasecmp(nameincert, *pname) == 0;
        size_t namelen = strlen(*pname);
        char *name = OPENSSL_malloc(namelen + 1);
        int match, ret;

        if (!TEST_ptr(name))
            return 0;
        memcpy(name, *pname, namelen + 1);

        match = -1;
        if (!TEST_int_ge(ret = X509_check_host(crt, name, namelen, 0, NULL),
                         0)) {
            failed = 1;
        } else if (fn->host) {
            if (ret == 1 && !samename)
                match = 1;
            if (ret == 0 && samename)
                match = 0;
        } else if (ret == 1)
            match = 1;
        if (!TEST_true(check_message(fn, "host", nameincert, match, *pname)))
            failed = 1;

        match = -1;
        if (!TEST_int_ge(ret = X509_check_host(crt, name, namelen,
                                               X509_CHECK_FLAG_NO_WILDCARDS,
                                               NULL), 0)) {
            failed = 1;
        } else if (fn->host) {
            if (ret == 1 && !samename)
                match = 1;
            if (ret == 0 && samename)
                match = 0;
        } else if (ret == 1)
            match = 1;
        if (!TEST_true(check_message(fn, "host-no-wildcards",
                                     nameincert, match, *pname)))
            failed = 1;

        match = -1;
        ret = X509_check_email(crt, name, namelen, 0);
        if (fn->email) {
            if (ret && !samename)
                match = 1;
            if (!ret && samename && strchr(nameincert, '@') != NULL)
                match = 0;
        } else if (ret)
            match = 1;
        if (!TEST_true(check_message(fn, "email", nameincert, match, *pname)))
            failed = 1;
        OPENSSL_free(name);
    }

    return failed == 0;
}

static int call_run_cert(int i)
{
    int failed = 0;
    const struct set_name_fn *pfn = &name_fns[i];
    X509 *crt;
    const char *const *pname;

    TEST_info("%s", pfn->name);
    for (pname = names; *pname != NULL; pname++) {
        if (!TEST_ptr(crt = make_cert())
             || !TEST_true(pfn->fn(crt, *pname))
             || !run_cert(crt, *pname, pfn))
            failed = 1;
        X509_free(crt);
    }
    return failed == 0;
}

static struct gennamedata {
    const unsigned char der[22];
    size_t derlen;
} gennames[] = {
    {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     SEQUENCE {}
        *   }
        * }
        */
        {
            0xa0, 0x13, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x02, 0x30, 0x00
        },
        21
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     [APPLICATION 0] {}
        *   }
        * }
        */
        {
            0xa0, 0x13, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x02, 0x60, 0x00
        },
        21
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa0, 0x14, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x03, 0x0c, 0x01, 0x61
        },
        22
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.2 }
        *   [0] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa0, 0x14, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x02, 0xa0, 0x03, 0x0c, 0x01, 0x61
        },
        22
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     UTF8String { "b" }
        *   }
        * }
        */
        {
            0xa0, 0x14, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x03, 0x0c, 0x01, 0x62
        },
        22
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     BOOLEAN { TRUE }
        *   }
        * }
        */
        {
            0xa0, 0x14, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x03, 0x01, 0x01, 0xff
        },
        22
    }, {
        /*
        * [0] {
        *   OBJECT_IDENTIFIER { 1.2.840.113554.4.1.72585.2.1 }
        *   [0] {
        *     BOOLEAN { FALSE }
        *   }
        * }
        */
        {
            0xa0, 0x14, 0x06, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04,
            0x01, 0x84, 0xb7, 0x09, 0x02, 0x01, 0xa0, 0x03, 0x01, 0x01, 0x00
        },
        22
    }, {
        /* [1 PRIMITIVE] { "a" } */
        {
            0x81, 0x01, 0x61
        },
        3
    }, {
        /* [1 PRIMITIVE] { "b" } */
        {
            0x81, 0x01, 0x62
        },
        3
    }, {
        /* [2 PRIMITIVE] { "a" } */
        {
            0x82, 0x01, 0x61
        },
        3
    }, {
        /* [2 PRIMITIVE] { "b" } */
        {
            0x82, 0x01, 0x62
        },
        3
    }, {
        /*
        * [4] {
        *   SEQUENCE {
        *     SET {
        *       SEQUENCE {
        *         # commonName
        *         OBJECT_IDENTIFIER { 2.5.4.3 }
        *         UTF8String { "a" }
        *       }
        *     }
        *   }
        * }
        */
        {
            0xa4, 0x0e, 0x30, 0x0c, 0x31, 0x0a, 0x30, 0x08, 0x06, 0x03, 0x55,
            0x04, 0x03, 0x0c, 0x01, 0x61
        },
        16
    }, {
        /*
        * [4] {
        *   SEQUENCE {
        *     SET {
        *       SEQUENCE {
        *         # commonName
        *         OBJECT_IDENTIFIER { 2.5.4.3 }
        *         UTF8String { "b" }
        *       }
        *     }
        *   }
        * }
        */
        {
            0xa4, 0x0e, 0x30, 0x0c, 0x31, 0x0a, 0x30, 0x08, 0x06, 0x03, 0x55,
            0x04, 0x03, 0x0c, 0x01, 0x62
        },
        16
    }, {
        /*
        * [5] {
        *   [1] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa5, 0x05, 0xa1, 0x03, 0x0c, 0x01, 0x61
        },
        7
    }, {
        /*
        * [5] {
        *   [1] {
        *     UTF8String { "b" }
        *   }
        * }
        */
        {
            0xa5, 0x05, 0xa1, 0x03, 0x0c, 0x01, 0x62
        },
        7
    }, {
        /*
        * [5] {
        *   [0] {
        *     UTF8String {}
        *   }
        *   [1] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa5, 0x09, 0xa0, 0x02, 0x0c, 0x00, 0xa1, 0x03, 0x0c, 0x01, 0x61
        },
        11
    }, {
        /*
        * [5] {
        *   [0] {
        *     UTF8String { "a" }
        *   }
        *   [1] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa5, 0x0a, 0xa0, 0x03, 0x0c, 0x01, 0x61, 0xa1, 0x03, 0x0c, 0x01,
            0x61
        },
        12
    }, {
        /*
        * [5] {
        *   [0] {
        *     UTF8String { "b" }
        *   }
        *   [1] {
        *     UTF8String { "a" }
        *   }
        * }
        */
        {
            0xa5, 0x0a, 0xa0, 0x03, 0x0c, 0x01, 0x62, 0xa1, 0x03, 0x0c, 0x01,
            0x61
        },
        12
    }, {
        /* [6 PRIMITIVE] { "a" } */
        {
            0x86, 0x01, 0x61
        },
        3
    }, {
        /* [6 PRIMITIVE] { "b" } */
        {
            0x86, 0x01, 0x62
        },
        3
    }, {
        /* [7 PRIMITIVE] { `11111111` } */
        {
            0x87, 0x04, 0x11, 0x11, 0x11, 0x11
        },
        6
    }, {
        /* [7 PRIMITIVE] { `22222222`} */
        {
            0x87, 0x04, 0x22, 0x22, 0x22, 0x22
        },
        6
    }, {
        /* [7 PRIMITIVE] { `11111111111111111111111111111111` } */
        {
            0x87, 0x10, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
            0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11
        },
        18
    }, {
        /* [7 PRIMITIVE] { `22222222222222222222222222222222` } */
        {
            0x87, 0x10, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
            0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22
        },
        18
    }, {
        /* [8 PRIMITIVE] { 1.2.840.113554.4.1.72585.2.1 } */
        {
            0x88, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04, 0x01, 0x84,
            0xb7, 0x09, 0x02, 0x01
        },
        15
    }, {
        /* [8 PRIMITIVE] { 1.2.840.113554.4.1.72585.2.2 } */
        {
            0x88, 0x0d, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 0x04, 0x01, 0x84,
            0xb7, 0x09, 0x02, 0x02
        },
        15
    }, {
        /*
         * Regression test for CVE-2023-0286.
         */
        {
            0xa3, 0x00
        },
        2
    }
};

static int test_GENERAL_NAME_cmp(void)
{
    size_t i, j;
    GENERAL_NAME **namesa = OPENSSL_malloc(sizeof(*namesa)
                                           * OSSL_NELEM(gennames));
    GENERAL_NAME **namesb = OPENSSL_malloc(sizeof(*namesb)
                                           * OSSL_NELEM(gennames));
    int testresult = 0;

    if (!TEST_ptr(namesa) || !TEST_ptr(namesb))
        goto end;

    for (i = 0; i < OSSL_NELEM(gennames); i++) {
        const unsigned char *derp = gennames[i].der;

        /*
         * We create two versions of each GENERAL_NAME so that we ensure when
         * we compare them they are always different pointers.
         */
        namesa[i] = d2i_GENERAL_NAME(NULL, &derp, gennames[i].derlen);
        derp = gennames[i].der;
        namesb[i] = d2i_GENERAL_NAME(NULL, &derp, gennames[i].derlen);
        if (!TEST_ptr(namesa[i]) || !TEST_ptr(namesb[i]))
            goto end;
    }

    /* Every name should be equal to itself and not equal to any others. */
    for (i = 0; i < OSSL_NELEM(gennames); i++) {
        for (j = 0; j < OSSL_NELEM(gennames); j++) {
            if (i == j) {
                if (!TEST_int_eq(GENERAL_NAME_cmp(namesa[i], namesb[j]), 0))
                    goto end;
            } else {
                if (!TEST_int_ne(GENERAL_NAME_cmp(namesa[i], namesb[j]), 0))
                    goto end;
            }
        }
    }
    testresult = 1;

 end:
    for (i = 0; i < OSSL_NELEM(gennames); i++) {
        if (namesa != NULL)
            GENERAL_NAME_free(namesa[i]);
        if (namesb != NULL)
            GENERAL_NAME_free(namesb[i]);
    }
    OPENSSL_free(namesa);
    OPENSSL_free(namesb);

    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(call_run_cert, OSSL_NELEM(name_fns));
    ADD_TEST(test_GENERAL_NAME_cmp);
    return 1;
}
