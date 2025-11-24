/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include "apps.h"

/*
 * X509_ctrl_str() is sorely lacking in libcrypto, but is still needed to
 * allow the application to process verification options in a manner similar
 * to signature or other options that pass through EVP_PKEY_CTX_ctrl_str(),
 * for uniformity.
 *
 * As soon as more stuff is added, the code will need serious rework.  For
 * the moment, it only handles the FIPS 196 / SM2 distinguishing ID.
 */
#ifdef EVP_PKEY_CTRL_SET1_ID
static ASN1_OCTET_STRING *mk_octet_string(void *value, size_t value_n)
{
    ASN1_OCTET_STRING *v = ASN1_OCTET_STRING_new();

    if (v == NULL) {
        BIO_printf(bio_err, "error: allocation failed\n");
    } else if (!ASN1_OCTET_STRING_set(v, value, (int)value_n)) {
        ASN1_OCTET_STRING_free(v);
        v = NULL;
    }
    return v;
}
#endif

static int x509_ctrl(void *object, int cmd, void *value, size_t value_n)
{
    switch (cmd) {
#ifdef EVP_PKEY_CTRL_SET1_ID
    case EVP_PKEY_CTRL_SET1_ID:
        {
            ASN1_OCTET_STRING *v = mk_octet_string(value, value_n);

            if (v == NULL) {
                BIO_printf(bio_err,
                           "error: setting distinguishing ID in certificate failed\n");
                return 0;
            }

            X509_set0_distinguishing_id(object, v);
            return 1;
        }
#endif
    default:
        break;
    }
    return -2;     /* typical EVP_PKEY return for "unsupported" */
}

static int x509_req_ctrl(void *object, int cmd, void *value, size_t value_n)
{
    switch (cmd) {
#ifdef EVP_PKEY_CTRL_SET1_ID
    case EVP_PKEY_CTRL_SET1_ID:
        {
            ASN1_OCTET_STRING *v = mk_octet_string(value, value_n);

            if (v == NULL) {
                BIO_printf(bio_err,
                           "error: setting distinguishing ID in certificate signing request failed\n");
                return 0;
            }

            X509_REQ_set0_distinguishing_id(object, v);
            return 1;
        }
#endif
    default:
        break;
    }
    return -2;     /* typical EVP_PKEY return for "unsupported" */
}

static int do_x509_ctrl_string(int (*ctrl)(void *object, int cmd,
                                           void *value, size_t value_n),
                               void *object, const char *value)
{
    int rv = 0;
    char *stmp, *vtmp = NULL;
    size_t vtmp_len = 0;
    int cmd = 0; /* Will get command values that make sense somehow */

    stmp = OPENSSL_strdup(value);
    if (stmp == NULL)
        return -1;
    vtmp = strchr(stmp, ':');
    if (vtmp != NULL) {
        *vtmp = 0;
        vtmp++;
        vtmp_len = strlen(vtmp);
    }

    if (strcmp(stmp, "distid") == 0) {
#ifdef EVP_PKEY_CTRL_SET1_ID
        cmd = EVP_PKEY_CTRL_SET1_ID; /* ... except we put it in X509 */
#endif
    } else if (strcmp(stmp, "hexdistid") == 0) {
        if (vtmp != NULL) {
            void *hexid;
            long hexid_len = 0;

            hexid = OPENSSL_hexstr2buf((const char *)vtmp, &hexid_len);
            OPENSSL_free(stmp);
            stmp = vtmp = hexid;
            vtmp_len = (size_t)hexid_len;
        }
#ifdef EVP_PKEY_CTRL_SET1_ID
        cmd = EVP_PKEY_CTRL_SET1_ID; /* ... except we put it in X509 */
#endif
    }

    rv = ctrl(object, cmd, vtmp, vtmp_len);

    OPENSSL_free(stmp);
    return rv;
}

int x509_ctrl_string(X509 *x, const char *value)
{
    return do_x509_ctrl_string(x509_ctrl, x, value);
}

int x509_req_ctrl_string(X509_REQ *x, const char *value)
{
    return do_x509_ctrl_string(x509_req_ctrl, x, value);
}
