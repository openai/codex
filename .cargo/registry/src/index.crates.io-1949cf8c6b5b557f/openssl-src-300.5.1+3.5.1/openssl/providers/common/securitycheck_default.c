/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/rsa.h>
#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/obj_mac.h>
#include "prov/securitycheck.h"
#include "internal/nelem.h"

/* Disable the security checks in the default provider */
int ossl_fips_config_securitycheck_enabled(OSSL_LIB_CTX *libctx)
{
    return 0;
}

int ossl_digest_rsa_sign_get_md_nid(const EVP_MD *md)
{
    int mdnid;

    static const OSSL_ITEM name_to_nid[] = {
        { NID_md5,       OSSL_DIGEST_NAME_MD5       },
        { NID_md5_sha1,  OSSL_DIGEST_NAME_MD5_SHA1  },
        { NID_md2,       OSSL_DIGEST_NAME_MD2       },
        { NID_md4,       OSSL_DIGEST_NAME_MD4       },
        { NID_mdc2,      OSSL_DIGEST_NAME_MDC2      },
        { NID_ripemd160, OSSL_DIGEST_NAME_RIPEMD160 },
        { NID_sm3,       OSSL_DIGEST_NAME_SM3 },
    };

    mdnid = ossl_digest_get_approved_nid(md);
    if (mdnid == NID_undef)
        mdnid = ossl_digest_md_to_nid(md, name_to_nid, OSSL_NELEM(name_to_nid));
    return mdnid;
}
