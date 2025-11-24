/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/obj_mac.h>
#include "internal/cryptlib.h"
#include "prov/der_rsa.h"
#include "prov/der_digests.h"

/* More complex pre-compiled sequences. */

/*-
 * From https://tools.ietf.org/html/rfc8017#appendix-A.2.1
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
#define DER_V_NULL DER_P_NULL, 0
#define DER_SZ_NULL 2

/*
 * The names for the hash function AlgorithmIdentifiers are borrowed and
 * expanded from https://tools.ietf.org/html/rfc4055#section-2.1
 *
 * sha1Identifier  AlgorithmIdentifier  ::=  { id-sha1, NULL }
 * sha224Identifier  AlgorithmIdentifier  ::=  { id-sha224, NULL }
 * sha256Identifier  AlgorithmIdentifier  ::=  { id-sha256, NULL }
 * sha384Identifier  AlgorithmIdentifier  ::=  { id-sha384, NULL }
 * sha512Identifier  AlgorithmIdentifier  ::=  { id-sha512, NULL }
 */
/*
 * NOTE: Some of the arrays aren't used other than inside sizeof(), which
 * clang complains about (-Wno-unneeded-internal-declaration).  To get
 * around that, we make them non-static, and declare them an extra time to
 * avoid compilers complaining about definitions without declarations.
 */
#define DER_AID_V_sha1Identifier                                        \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha1 + DER_SZ_NULL,                               \
        DER_OID_V_id_sha1,                                              \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha1Identifier[];
const unsigned char ossl_der_aid_sha1Identifier[] = {
    DER_AID_V_sha1Identifier
};
#define DER_AID_SZ_sha1Identifier sizeof(ossl_der_aid_sha1Identifier)

#define DER_AID_V_sha224Identifier                                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha224 + DER_SZ_NULL,                             \
        DER_OID_V_id_sha224,                                            \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha224Identifier[];
const unsigned char ossl_der_aid_sha224Identifier[] = {
    DER_AID_V_sha224Identifier
};
#define DER_AID_SZ_sha224Identifier sizeof(ossl_der_aid_sha224Identifier)

#define DER_AID_V_sha256Identifier                                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha256 + DER_SZ_NULL,                             \
        DER_OID_V_id_sha256,                                            \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha256Identifier[];
const unsigned char ossl_der_aid_sha256Identifier[] = {
    DER_AID_V_sha256Identifier
};
#define DER_AID_SZ_sha256Identifier sizeof(ossl_der_aid_sha256Identifier)

#define DER_AID_V_sha384Identifier                                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha384 + DER_SZ_NULL,                             \
        DER_OID_V_id_sha384,                                            \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha384Identifier[];
const unsigned char ossl_der_aid_sha384Identifier[] = {
    DER_AID_V_sha384Identifier
};
#define DER_AID_SZ_sha384Identifier sizeof(ossl_der_aid_sha384Identifier)

#define DER_AID_V_sha512Identifier                                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha512 + DER_SZ_NULL,                             \
        DER_OID_V_id_sha512,                                            \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha512Identifier[];
const unsigned char ossl_der_aid_sha512Identifier[] = {
    DER_AID_V_sha512Identifier
};
#define DER_AID_SZ_sha512Identifier sizeof(ossl_der_aid_sha512Identifier)

#define DER_AID_V_sha512_224Identifier                                  \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha512_224 + DER_SZ_NULL,                         \
        DER_OID_V_id_sha512_224,                                        \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha512_224Identifier[];
const unsigned char ossl_der_aid_sha512_224Identifier[] = {
    DER_AID_V_sha512_224Identifier
};
#define DER_AID_SZ_sha512_224Identifier sizeof(ossl_der_aid_sha512_224Identifier)

#define DER_AID_V_sha512_256Identifier                                  \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_sha512_256 + DER_SZ_NULL,                         \
        DER_OID_V_id_sha512_256,                                        \
        DER_V_NULL
extern const unsigned char ossl_der_aid_sha512_256Identifier[];
const unsigned char ossl_der_aid_sha512_256Identifier[] = {
    DER_AID_V_sha512_256Identifier
};
#define DER_AID_SZ_sha512_256Identifier sizeof(ossl_der_aid_sha512_256Identifier)

/*-
 * From https://tools.ietf.org/html/rfc8017#appendix-A.2.1
 *
 * HashAlgorithm ::= AlgorithmIdentifier {
 *    {OAEP-PSSDigestAlgorithms}
 * }
 *
 * ...
 *
 * PKCS1MGFAlgorithms    ALGORITHM-IDENTIFIER ::= {
 *     { OID id-mgf1 PARAMETERS HashAlgorithm },
 *     ...  -- Allows for future expansion --
 * }
 */

/*
 * The names for the MGF1 AlgorithmIdentifiers are borrowed and expanded
 * from https://tools.ietf.org/html/rfc4055#section-2.1
 *
 * mgf1SHA1Identifier  AlgorithmIdentifier  ::=
 *                      { id-mgf1, sha1Identifier }
 * mgf1SHA224Identifier  AlgorithmIdentifier  ::=
 *                      { id-mgf1, sha224Identifier }
 * mgf1SHA256Identifier  AlgorithmIdentifier  ::=
 *                      { id-mgf1, sha256Identifier }
 * mgf1SHA384Identifier  AlgorithmIdentifier  ::=
 *                      { id-mgf1, sha384Identifier }
 * mgf1SHA512Identifier  AlgorithmIdentifier  ::=
 *                      { id-mgf1, sha512Identifier }
 */
#if 0                            /* Currently unused */
#define DER_AID_V_mgf1SHA1Identifier                                    \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                                   \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha1Identifier,                 \
        DER_OID_V_id_mgf1,                                              \
        DER_AID_V_sha1Identifier
static const unsigned char der_aid_mgf1SHA1Identifier[] = {
    DER_AID_V_mgf1SHA1Identifier
};
#define DER_AID_SZ_mgf1SHA1Identifier sizeof(der_aid_mgf1SHA1Identifier)
#endif

#define DER_AID_V_mgf1SHA224Identifier                          \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha224Identifier,       \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha224Identifier
static const unsigned char der_aid_mgf1SHA224Identifier[] = {
    DER_AID_V_mgf1SHA224Identifier
};
#define DER_AID_SZ_mgf1SHA224Identifier sizeof(der_aid_mgf1SHA224Identifier)

#define DER_AID_V_mgf1SHA256Identifier                          \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha256Identifier,       \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha256Identifier
static const unsigned char der_aid_mgf1SHA256Identifier[] = {
    DER_AID_V_mgf1SHA256Identifier
};
#define DER_AID_SZ_mgf1SHA256Identifier sizeof(der_aid_mgf1SHA256Identifier)

#define DER_AID_V_mgf1SHA384Identifier                          \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha384Identifier,       \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha384Identifier
static const unsigned char der_aid_mgf1SHA384Identifier[] = {
    DER_AID_V_mgf1SHA384Identifier
};
#define DER_AID_SZ_mgf1SHA384Identifier sizeof(der_aid_mgf1SHA384Identifier)

#define DER_AID_V_mgf1SHA512Identifier                          \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha512Identifier,       \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha512Identifier
static const unsigned char der_aid_mgf1SHA512Identifier[] = {
    DER_AID_V_mgf1SHA512Identifier
};
#define DER_AID_SZ_mgf1SHA512Identifier sizeof(der_aid_mgf1SHA512Identifier)

#define DER_AID_V_mgf1SHA512_224Identifier                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha512_224Identifier,   \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha512_224Identifier
static const unsigned char der_aid_mgf1SHA512_224Identifier[] = {
    DER_AID_V_mgf1SHA512_224Identifier
};
#define DER_AID_SZ_mgf1SHA512_224Identifier sizeof(der_aid_mgf1SHA512_224Identifier)

#define DER_AID_V_mgf1SHA512_256Identifier                      \
    DER_P_SEQUENCE|DER_F_CONSTRUCTED,                           \
        DER_OID_SZ_id_mgf1 + DER_AID_SZ_sha512_256Identifier,   \
        DER_OID_V_id_mgf1,                                      \
        DER_AID_V_sha512_256Identifier
static const unsigned char der_aid_mgf1SHA512_256Identifier[] = {
    DER_AID_V_mgf1SHA512_256Identifier
};
#define DER_AID_SZ_mgf1SHA512_256Identifier sizeof(der_aid_mgf1SHA512_256Identifier)


#define MGF1_SHA_CASE(bits, var)                                \
    case NID_sha##bits:                                         \
        var = der_aid_mgf1SHA##bits##Identifier;                \
        var##_sz = sizeof(der_aid_mgf1SHA##bits##Identifier);   \
        break;

/*-
 * The name is borrowed from https://tools.ietf.org/html/rfc8017#appendix-A.2.1
 *
 * MaskGenAlgorithm ::= AlgorithmIdentifier { {PKCS1MGFAlgorithms} }
 */
static int DER_w_MaskGenAlgorithm(WPACKET *pkt, int tag,
                                  const RSA_PSS_PARAMS_30 *pss)
{
    if (pss != NULL && ossl_rsa_pss_params_30_maskgenalg(pss) == NID_mgf1) {
        int maskgenhashalg_nid = ossl_rsa_pss_params_30_maskgenhashalg(pss);
        const unsigned char *maskgenalg = NULL;
        size_t maskgenalg_sz = 0;

        switch (maskgenhashalg_nid) {
        case NID_sha1:
            break;
            MGF1_SHA_CASE(224, maskgenalg);
            MGF1_SHA_CASE(256, maskgenalg);
            MGF1_SHA_CASE(384, maskgenalg);
            MGF1_SHA_CASE(512, maskgenalg);
            MGF1_SHA_CASE(512_224, maskgenalg);
            MGF1_SHA_CASE(512_256, maskgenalg);
        default:
            return 0;
        }

        /* If there is none (or it was the default), we write nothing */
        if (maskgenalg == NULL)
            return 1;

        return ossl_DER_w_precompiled(pkt, tag, maskgenalg, maskgenalg_sz);
    }
    return 0;
}

#define OAEP_PSS_MD_CASE(name, var)                                     \
    case NID_##name:                                                    \
        var = ossl_der_aid_##name##Identifier;                          \
        var##_sz = sizeof(ossl_der_aid_##name##Identifier);             \
        break;

int ossl_DER_w_RSASSA_PSS_params(WPACKET *pkt, int tag,
                                 const RSA_PSS_PARAMS_30 *pss)
{
    int hashalg_nid, default_hashalg_nid;
    int saltlen, default_saltlen;
    int trailerfield, default_trailerfield;
    const unsigned char *hashalg = NULL;
    size_t hashalg_sz = 0;

    /*
     * For an unrestricted key, this function should not have been called;
     * the caller must be in control, because unrestricted keys are permitted
     * in some situations (when encoding the public key in a SubjectKeyInfo,
     * for example) while not in others, and this function doesn't know the
     * intent.  Therefore, we assert that here, the PSS parameters must show
     * that the key is restricted.
     */
    if (!ossl_assert(pss != NULL
                     && !ossl_rsa_pss_params_30_is_unrestricted(pss)))
        return 0;

    hashalg_nid = ossl_rsa_pss_params_30_hashalg(pss);
    saltlen = ossl_rsa_pss_params_30_saltlen(pss);
    trailerfield = ossl_rsa_pss_params_30_trailerfield(pss);

    if (saltlen < 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_SALT_LENGTH);
        return 0;
    }
    if (trailerfield != 1) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_TRAILER);
        return 0;
    }

    /* Getting default values */
    default_hashalg_nid = ossl_rsa_pss_params_30_hashalg(NULL);
    default_saltlen = ossl_rsa_pss_params_30_saltlen(NULL);
    default_trailerfield = ossl_rsa_pss_params_30_trailerfield(NULL);

    /*
     * From https://tools.ietf.org/html/rfc8017#appendix-A.2.1:
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
    switch (hashalg_nid) {
        OAEP_PSS_MD_CASE(sha1, hashalg);
        OAEP_PSS_MD_CASE(sha224, hashalg);
        OAEP_PSS_MD_CASE(sha256, hashalg);
        OAEP_PSS_MD_CASE(sha384, hashalg);
        OAEP_PSS_MD_CASE(sha512, hashalg);
        OAEP_PSS_MD_CASE(sha512_224, hashalg);
        OAEP_PSS_MD_CASE(sha512_256, hashalg);
    default:
        return 0;
    }

    return ossl_DER_w_begin_sequence(pkt, tag)
        && (trailerfield == default_trailerfield
            || ossl_DER_w_uint32(pkt, 3, (uint32_t)trailerfield))
        && (saltlen == default_saltlen || ossl_DER_w_uint32(pkt, 2, (uint32_t)saltlen))
        && DER_w_MaskGenAlgorithm(pkt, 1, pss)
        && (hashalg_nid == default_hashalg_nid
            || ossl_DER_w_precompiled(pkt, 0, hashalg, hashalg_sz))
        && ossl_DER_w_end_sequence(pkt, tag);
}

/* Aliases so we can have a uniform RSA_CASE */
#define ossl_der_oid_rsassaPss ossl_der_oid_id_RSASSA_PSS

#define RSA_CASE(name, var)                                             \
    var##_nid = NID_##name;                                             \
    var##_oid = ossl_der_oid_##name;                                    \
    var##_oid_sz = sizeof(ossl_der_oid_##name);                         \
    break;

int ossl_DER_w_algorithmIdentifier_RSA_PSS(WPACKET *pkt, int tag,
                                           int rsa_type,
                                           const RSA_PSS_PARAMS_30 *pss)
{
    int rsa_nid = NID_undef;
    const unsigned char *rsa_oid = NULL;
    size_t rsa_oid_sz = 0;

    switch (rsa_type) {
    case RSA_FLAG_TYPE_RSA:
        RSA_CASE(rsaEncryption, rsa);
    case RSA_FLAG_TYPE_RSASSAPSS:
        RSA_CASE(rsassaPss, rsa);
    }

    if (rsa_oid == NULL)
        return 0;

    return ossl_DER_w_begin_sequence(pkt, tag)
        && (rsa_nid != NID_rsassaPss
            || ossl_rsa_pss_params_30_is_unrestricted(pss)
            || ossl_DER_w_RSASSA_PSS_params(pkt, -1, pss))
        && ossl_DER_w_precompiled(pkt, -1, rsa_oid, rsa_oid_sz)
        && ossl_DER_w_end_sequence(pkt, tag);
}

int ossl_DER_w_algorithmIdentifier_RSA(WPACKET *pkt, int tag, RSA *rsa)
{
    int rsa_type = RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK);
    RSA_PSS_PARAMS_30 *pss_params = ossl_rsa_get0_pss_params_30(rsa);

    return ossl_DER_w_algorithmIdentifier_RSA_PSS(pkt, tag, rsa_type,
                                                  pss_params);
}
