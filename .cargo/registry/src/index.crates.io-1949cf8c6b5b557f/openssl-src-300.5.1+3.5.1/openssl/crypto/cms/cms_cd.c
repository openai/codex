/*
 * Copyright 2008-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"
#include <openssl/asn1t.h>
#include <openssl/pem.h>
#include <openssl/x509v3.h>
#include <openssl/err.h>
#include <openssl/cms.h>
#include <openssl/bio.h>
#include <openssl/comp.h>
#include "cms_local.h"

#ifndef OPENSSL_NO_ZLIB

/* CMS CompressedData Utilities */

CMS_ContentInfo *ossl_cms_CompressedData_create(int comp_nid,
                                                OSSL_LIB_CTX *libctx,
                                                const char *propq)
{
    CMS_ContentInfo *cms;
    CMS_CompressedData *cd;

    /*
     * Will need something cleverer if there is ever more than one
     * compression algorithm or parameters have some meaning...
     */
    if (comp_nid != NID_zlib_compression) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
        return NULL;
    }
    cms = CMS_ContentInfo_new_ex(libctx, propq);
    if (cms == NULL)
        return NULL;

    cd = M_ASN1_new_of(CMS_CompressedData);

    if (cd == NULL)
        goto err;

    cms->contentType = OBJ_nid2obj(NID_id_smime_ct_compressedData);
    cms->d.compressedData = cd;

    cd->version = 0;

    (void)X509_ALGOR_set0(cd->compressionAlgorithm,
                          OBJ_nid2obj(NID_zlib_compression),
                          V_ASN1_UNDEF, NULL); /* cannot fail */

    cd->encapContentInfo->eContentType = OBJ_nid2obj(NID_pkcs7_data);

    return cms;

 err:
    CMS_ContentInfo_free(cms);
    return NULL;
}

BIO *ossl_cms_CompressedData_init_bio(const CMS_ContentInfo *cms)
{
    CMS_CompressedData *cd;
    const ASN1_OBJECT *compoid;

    if (OBJ_obj2nid(cms->contentType) != NID_id_smime_ct_compressedData) {
        ERR_raise(ERR_LIB_CMS, CMS_R_CONTENT_TYPE_NOT_COMPRESSED_DATA);
        return NULL;
    }
    cd = cms->d.compressedData;
    X509_ALGOR_get0(&compoid, NULL, NULL, cd->compressionAlgorithm);
    if (OBJ_obj2nid(compoid) != NID_zlib_compression) {
        ERR_raise(ERR_LIB_CMS, CMS_R_UNSUPPORTED_COMPRESSION_ALGORITHM);
        return NULL;
    }
    return BIO_new(BIO_f_zlib());
}

#endif
