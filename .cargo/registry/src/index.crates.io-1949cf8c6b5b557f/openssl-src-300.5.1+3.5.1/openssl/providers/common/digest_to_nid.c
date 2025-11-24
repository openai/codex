/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/objects.h>
#include <openssl/core_names.h>
#include <openssl/evp.h>
#include <openssl/core.h>
#include "prov/securitycheck.h"
#include "internal/nelem.h"

/*
 * Internal library code deals with NIDs, so we need to translate from a name.
 * We do so using EVP_MD_is_a(), and therefore need a name to NID map.
 */
int ossl_digest_md_to_nid(const EVP_MD *md, const OSSL_ITEM *it, size_t it_len)
{
    size_t i;

    if (md == NULL)
        return NID_undef;

    for (i = 0; i < it_len; i++)
        if (EVP_MD_is_a(md, it[i].ptr))
            return (int)it[i].id;
    return NID_undef;
}

/*
 * Retrieve one of the FIPS approved hash algorithms by nid.
 * See FIPS 180-4 "Secure Hash Standard" and FIPS 202 - SHA-3.
 */
int ossl_digest_get_approved_nid(const EVP_MD *md)
{
    /* TODO: FIPS 180-5 RFC 8692 RFC 8702 allow SHAKE */
    static const OSSL_ITEM name_to_nid[] = {
        { NID_sha1,      OSSL_DIGEST_NAME_SHA1      },
        { NID_sha224,    OSSL_DIGEST_NAME_SHA2_224  },
        { NID_sha256,    OSSL_DIGEST_NAME_SHA2_256  },
        { NID_sha384,    OSSL_DIGEST_NAME_SHA2_384  },
        { NID_sha512,    OSSL_DIGEST_NAME_SHA2_512  },
        { NID_sha512_224, OSSL_DIGEST_NAME_SHA2_512_224 },
        { NID_sha512_256, OSSL_DIGEST_NAME_SHA2_512_256 },
        { NID_sha3_224,  OSSL_DIGEST_NAME_SHA3_224  },
        { NID_sha3_256,  OSSL_DIGEST_NAME_SHA3_256  },
        { NID_sha3_384,  OSSL_DIGEST_NAME_SHA3_384  },
        { NID_sha3_512,  OSSL_DIGEST_NAME_SHA3_512  },
    };

    return ossl_digest_md_to_nid(md, name_to_nid, OSSL_NELEM(name_to_nid));
}
