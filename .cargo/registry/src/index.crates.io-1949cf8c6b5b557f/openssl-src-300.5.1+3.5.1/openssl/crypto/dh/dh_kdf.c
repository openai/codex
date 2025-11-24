/*
 * Copyright 2013-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/e_os.h"
#include <string.h>
#include <openssl/core_names.h>
#include <openssl/dh.h>
#include <openssl/evp.h>
#include <openssl/asn1.h>
#include <openssl/kdf.h>
#include "internal/provider.h"
#include "crypto/dh.h"

/* Key derivation function from X9.63/SECG */
int ossl_dh_kdf_X9_42_asn1(unsigned char *out, size_t outlen,
                           const unsigned char *Z, size_t Zlen,
                           const char *cek_alg,
                           const unsigned char *ukm, size_t ukmlen,
                           const EVP_MD *md,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    int ret = 0;
    EVP_KDF_CTX *kctx = NULL;
    EVP_KDF *kdf = NULL;
    OSSL_PARAM params[5], *p = params;
    const char *mdname = EVP_MD_get0_name(md);

    kdf = EVP_KDF_fetch(libctx, OSSL_KDF_NAME_X942KDF_ASN1, propq);
    if (kdf == NULL)
        return 0;
    kctx = EVP_KDF_CTX_new(kdf);
    if (kctx == NULL)
        goto err;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_KEY,
                                             (unsigned char *)Z, Zlen);
    if (ukm != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_UKM,
                                                 (unsigned char *)ukm, ukmlen);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_CEK_ALG,
                                            (char *)cek_alg, 0);
    *p = OSSL_PARAM_construct_end();
    ret = EVP_KDF_derive(kctx, out, outlen, params) > 0;
err:
    EVP_KDF_CTX_free(kctx);
    EVP_KDF_free(kdf);
    return ret;
}

#if !defined(FIPS_MODULE)
int DH_KDF_X9_42(unsigned char *out, size_t outlen,
                 const unsigned char *Z, size_t Zlen,
                 ASN1_OBJECT *key_oid,
                 const unsigned char *ukm, size_t ukmlen, const EVP_MD *md)
{
    char key_alg[OSSL_MAX_NAME_SIZE];
    const OSSL_PROVIDER *prov = EVP_MD_get0_provider(md);
    OSSL_LIB_CTX *libctx = ossl_provider_libctx(prov);

    if (OBJ_obj2txt(key_alg, sizeof(key_alg), key_oid, 0) <= 0)
        return 0;

    return ossl_dh_kdf_X9_42_asn1(out, outlen, Z, Zlen, key_alg,
                                  ukm, ukmlen, md, libctx, NULL);
}
#endif /* !defined(FIPS_MODULE) */
