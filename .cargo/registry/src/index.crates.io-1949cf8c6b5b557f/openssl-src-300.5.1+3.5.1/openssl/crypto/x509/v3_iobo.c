/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/x509v3.h>
#include "ext_dat.h"

static int i2r_ISSUED_ON_BEHALF_OF(X509V3_EXT_METHOD *method,
                                   GENERAL_NAME *gn, BIO *out,
                                   int indent)
{
    if (BIO_printf(out, "%*s", indent, "") <= 0)
        return 0;
    if (GENERAL_NAME_print(out, gn) <= 0)
        return 0;
    return BIO_puts(out, "\n") > 0;
}

const X509V3_EXT_METHOD ossl_v3_issued_on_behalf_of = {
    NID_issued_on_behalf_of, 0, ASN1_ITEM_ref(GENERAL_NAME),
    0, 0, 0, 0,
    0, 0,
    0, 0,
    (X509V3_EXT_I2R)i2r_ISSUED_ON_BEHALF_OF,
    0,
    NULL
};
