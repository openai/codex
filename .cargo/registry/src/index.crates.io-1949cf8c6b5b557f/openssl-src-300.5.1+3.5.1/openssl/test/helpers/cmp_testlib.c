/*
 * Copyright 2007-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "cmp_testlib.h"
#include <openssl/rsa.h> /* needed in case config no-deprecated */

OSSL_CMP_MSG *load_pkimsg(const char *file, OSSL_LIB_CTX *libctx)
{
    OSSL_CMP_MSG *msg;

    (void)TEST_ptr((msg = OSSL_CMP_MSG_read(file, libctx, NULL)));
    return msg;
}

/*
 * Checks whether the syntax of msg conforms to ASN.1
 */
int valid_asn1_encoding(const OSSL_CMP_MSG *msg)
{
    return msg != NULL ? i2d_OSSL_CMP_MSG(msg, NULL) > 0 : 0;
}

/*
 * Compares two stacks of certificates in the order of their elements.
 * Returns 0 if sk1 and sk2 are equal and another value otherwise
 */
int STACK_OF_X509_cmp(const STACK_OF(X509) *sk1, const STACK_OF(X509) *sk2)
{
    int i, res;
    X509 *a, *b;

    if (sk1 == sk2)
        return 0;
    if (sk1 == NULL)
        return -1;
    if (sk2 == NULL)
        return 1;
    if ((res = sk_X509_num(sk1) - sk_X509_num(sk2)))
        return res;
    for (i = 0; i < sk_X509_num(sk1); i++) {
        a = sk_X509_value(sk1, i);
        b = sk_X509_value(sk2, i);
        if (a != b)
            if ((res = X509_cmp(a, b)) != 0)
                return res;
    }
    return 0;
}

/*
 * Up refs and push a cert onto sk.
 * Returns the number of certificates on the stack on success
 * Returns -1 or 0 on error
 */
int STACK_OF_X509_push1(STACK_OF(X509) *sk, X509 *cert)
{
    int res;

    if (sk == NULL || cert == NULL)
        return -1;
    if (!X509_up_ref(cert))
        return -1;
    res = sk_X509_push(sk, cert);
    if (res <= 0)
        X509_free(cert); /* down-ref */
    return res;
}

int print_to_bio_out(const char *func, const char *file, int line,
                     OSSL_CMP_severity level, const char *msg)
{
    return OSSL_CMP_print_to_bio(bio_out, func, file, line, level, msg);
}
