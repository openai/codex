/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/err.h>
#include "crypto/err.h"
#include "crypto/cryptoerr.h"
#include "crypto/asn1err.h"
#include "crypto/bnerr.h"
#include "crypto/ecerr.h"
#include "crypto/buffererr.h"
#include "crypto/bioerr.h"
#include "crypto/comperr.h"
#include "crypto/rsaerr.h"
#include "crypto/dherr.h"
#include "crypto/dsaerr.h"
#include "crypto/evperr.h"
#include "crypto/objectserr.h"
#include "crypto/pemerr.h"
#include "crypto/pkcs7err.h"
#include "crypto/x509err.h"
#include "crypto/x509v3err.h"
#include "crypto/conferr.h"
#include "crypto/pkcs12err.h"
#include "crypto/randerr.h"
#include "internal/dsoerr.h"
#include "crypto/engineerr.h"
#include "crypto/uierr.h"
#include "crypto/httperr.h"
#include "crypto/ocsperr.h"
#include "crypto/tserr.h"
#include "crypto/cmserr.h"
#include "crypto/crmferr.h"
#include "crypto/cmperr.h"
#include "crypto/cterr.h"
#include "crypto/asyncerr.h"
#include "crypto/storeerr.h"
#include "crypto/esserr.h"
#include "internal/propertyerr.h"
#include "prov/proverr.h"

int ossl_err_load_crypto_strings(void)
{
    if (0
#ifndef OPENSSL_NO_ERR
        || ossl_err_load_ERR_strings() == 0 /* include error strings for SYSerr */
        || ossl_err_load_BN_strings() == 0
        || ossl_err_load_RSA_strings() == 0
# ifndef OPENSSL_NO_DH
        || ossl_err_load_DH_strings() == 0
# endif
        || ossl_err_load_EVP_strings() == 0
        || ossl_err_load_BUF_strings() == 0
        || ossl_err_load_OBJ_strings() == 0
        || ossl_err_load_PEM_strings() == 0
# ifndef OPENSSL_NO_DSA
        || ossl_err_load_DSA_strings() == 0
# endif
        || ossl_err_load_X509_strings() == 0
        || ossl_err_load_ASN1_strings() == 0
        || ossl_err_load_CONF_strings() == 0
        || ossl_err_load_CRYPTO_strings() == 0
# ifndef OPENSSL_NO_COMP
        || ossl_err_load_COMP_strings() == 0
# endif
# ifndef OPENSSL_NO_EC
        || ossl_err_load_EC_strings() == 0
# endif
        /* skip ossl_err_load_SSL_strings() because it is not in this library */
        || ossl_err_load_BIO_strings() == 0
        || ossl_err_load_PKCS7_strings() == 0
        || ossl_err_load_X509V3_strings() == 0
        || ossl_err_load_PKCS12_strings() == 0
        || ossl_err_load_RAND_strings() == 0
        || ossl_err_load_DSO_strings() == 0
# ifndef OPENSSL_NO_TS
        || ossl_err_load_TS_strings() == 0
# endif
# ifndef OPENSSL_NO_ENGINE
        || ossl_err_load_ENGINE_strings() == 0
# endif
# ifndef OPENSSL_NO_HTTP
        || ossl_err_load_HTTP_strings() == 0
# endif
# ifndef OPENSSL_NO_OCSP
        || ossl_err_load_OCSP_strings() == 0
# endif
        || ossl_err_load_UI_strings() == 0
# ifndef OPENSSL_NO_CMS
        || ossl_err_load_CMS_strings() == 0
# endif
# ifndef OPENSSL_NO_CRMF
        || ossl_err_load_CRMF_strings() == 0
        || ossl_err_load_CMP_strings() == 0
# endif
# ifndef OPENSSL_NO_CT
        || ossl_err_load_CT_strings() == 0
# endif
        || ossl_err_load_ESS_strings() == 0
        || ossl_err_load_ASYNC_strings() == 0
        || ossl_err_load_OSSL_STORE_strings() == 0
        || ossl_err_load_PROP_strings() == 0
        || ossl_err_load_PROV_strings() == 0
#endif
        )
        return 0;

    return 1;
}
