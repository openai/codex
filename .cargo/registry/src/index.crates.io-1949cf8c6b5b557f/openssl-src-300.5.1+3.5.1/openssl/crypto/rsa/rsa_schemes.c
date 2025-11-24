/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/obj_mac.h>
#include "internal/nelem.h"
#include "crypto/rsa.h"

static int meth2nid(const void *meth,
                    int (*meth_is_a)(const void *meth, const char *name),
                    const OSSL_ITEM *items, size_t items_n)
{
    size_t i;

    if (meth != NULL)
        for (i = 0; i < items_n; i++)
            if (meth_is_a(meth, items[i].ptr))
                return (int)items[i].id;
    return NID_undef;
}

static const char *nid2name(int meth, const OSSL_ITEM *items, size_t items_n)
{
    size_t i;

    for (i = 0; i < items_n; i++)
        if (meth == (int)items[i].id)
            return items[i].ptr;
    return NULL;
}

/*
 * The list of permitted hash functions are taken from
 * https://tools.ietf.org/html/rfc8017#appendix-A.2.1:
 *
 * OAEP-PSSDigestAlgorithms    ALGORITHM-IDENTIFIER ::= {
 *     { OID id-sha1       PARAMETERS NULL }|
 *     { OID id-sha224     PARAMETERS NULL }|
 *     { OID id-sha256     PARAMETERS NULL }|
 *     { OID id-sha384     PARAMETERS NULL }|
 *     { OID id-sha512     PARAMETERS NULL }|
 *     { OID id-sha512-224 PARAMETERS NULL }|
 *     { OID id-sha512-256 PARAMETERS NULL },
 *     ...  -- Allows for future expansion --
 * }
 */
static const OSSL_ITEM oaeppss_name_nid_map[] = {
    { NID_sha1,         OSSL_DIGEST_NAME_SHA1         },
    { NID_sha224,       OSSL_DIGEST_NAME_SHA2_224     },
    { NID_sha256,       OSSL_DIGEST_NAME_SHA2_256     },
    { NID_sha384,       OSSL_DIGEST_NAME_SHA2_384     },
    { NID_sha512,       OSSL_DIGEST_NAME_SHA2_512     },
    { NID_sha512_224,   OSSL_DIGEST_NAME_SHA2_512_224 },
    { NID_sha512_256,   OSSL_DIGEST_NAME_SHA2_512_256 },
};

static int md_is_a(const void *md, const char *name)
{
    return EVP_MD_is_a(md, name);
}

int ossl_rsa_oaeppss_md2nid(const EVP_MD *md)
{
    return meth2nid(md, md_is_a,
                    oaeppss_name_nid_map, OSSL_NELEM(oaeppss_name_nid_map));
}

const char *ossl_rsa_oaeppss_nid2name(int md)
{
    return nid2name(md, oaeppss_name_nid_map, OSSL_NELEM(oaeppss_name_nid_map));
}

const char *ossl_rsa_mgf_nid2name(int mgf)
{
    if (mgf == NID_mgf1)
        return SN_mgf1;
    return NULL;
}
