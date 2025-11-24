/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
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

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/objects.h>
#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_MD2
#  include <openssl/md2.h> /* uses MD2_DIGEST_LENGTH */
# endif
# ifndef OPENSSL_NO_MD4
#  include <openssl/md4.h> /* uses MD4_DIGEST_LENGTH */
# endif
# ifndef OPENSSL_NO_MD5
#  include <openssl/md5.h> /* uses MD5_DIGEST_LENGTH */
# endif
# ifndef OPENSSL_NO_MDC2
#  include <openssl/mdc2.h> /* uses MDC2_DIGEST_LENGTH */
# endif
# ifndef OPENSSL_NO_RMD160
#  include <openssl/ripemd.h> /* uses RIPEMD160_DIGEST_LENGTH */
# endif
# ifndef OPENSSL_NO_SM3
#  include "internal/sm3.h" /* uses SM3_DIGEST_LENGTH */
# endif
#endif
#include <openssl/sha.h> /* uses SHA???_DIGEST_LENGTH */
#include "crypto/rsa.h"
#include "rsa_local.h"

/*
 * The general purpose ASN1 code is not available inside the FIPS provider.
 * To remove the dependency RSASSA-PKCS1-v1_5 DigestInfo encodings can be
 * treated as a special case by pregenerating the required ASN1 encoding.
 * This encoding will also be shared by the default provider.
 *
 * The EMSA-PKCS1-v1_5 encoding method includes an ASN.1 value of type
 * DigestInfo, where the type DigestInfo has the syntax
 *
 *     DigestInfo ::= SEQUENCE {
 *         digestAlgorithm DigestAlgorithm,
 *         digest OCTET STRING
 *     }
 *
 *     DigestAlgorithm ::= AlgorithmIdentifier {
 *         {PKCS1-v1-5DigestAlgorithms}
 *     }
 *
 * The AlgorithmIdentifier is a sequence containing the digest OID and
 * parameters (a value of type NULL).
 *
 * The ENCODE_DIGESTINFO_SHA() and ENCODE_DIGESTINFO_MD() macros define an
 * initialized array containing the DER encoded DigestInfo for the specified
 * SHA or MD digest. The content of the OCTET STRING is not included.
 * |name| is the digest name.
 * |n| is last byte in the encoded OID for the digest.
 * |sz| is the digest length in bytes. It must not be greater than 110.
 */

#define ASN1_SEQUENCE 0x30
#define ASN1_OCTET_STRING 0x04
#define ASN1_NULL 0x05
#define ASN1_OID 0x06

/* SHA OIDs are of the form: (2 16 840 1 101 3 4 2 |n|) */
#define ENCODE_DIGESTINFO_SHA(name, n, sz)                                     \
static const unsigned char digestinfo_##name##_der[] = {                       \
    ASN1_SEQUENCE, 0x11 + sz,                                                  \
      ASN1_SEQUENCE, 0x0d,                                                     \
        ASN1_OID, 0x09, 2 * 40 + 16, 0x86, 0x48, 1, 101, 3, 4, 2, n,           \
        ASN1_NULL, 0x00,                                                       \
      ASN1_OCTET_STRING, sz                                                    \
};

/* MD2, MD4 and MD5 OIDs are of the form: (1 2 840 113549 2 |n|) */
#define ENCODE_DIGESTINFO_MD(name, n, sz)                                      \
static const unsigned char digestinfo_##name##_der[] = {                       \
    ASN1_SEQUENCE, 0x10 + sz,                                                  \
      ASN1_SEQUENCE, 0x0c,                                                     \
        ASN1_OID, 0x08, 1 * 40 + 2, 0x86, 0x48, 0x86, 0xf7, 0x0d, 2, n,        \
        ASN1_NULL, 0x00,                                                       \
      ASN1_OCTET_STRING, sz                                                    \
};

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_MD2
ENCODE_DIGESTINFO_MD(md2, 0x02, MD2_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MD4
ENCODE_DIGESTINFO_MD(md4, 0x03, MD4_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MD5
ENCODE_DIGESTINFO_MD(md5, 0x05, MD5_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MDC2
/* MDC-2 (2 5 8 3 101) */
static const unsigned char digestinfo_mdc2_der[] = {
    ASN1_SEQUENCE, 0x0c + MDC2_DIGEST_LENGTH,
      ASN1_SEQUENCE, 0x08,
        ASN1_OID, 0x04, 2 * 40 + 5, 8, 3, 101,
        ASN1_NULL, 0x00,
      ASN1_OCTET_STRING, MDC2_DIGEST_LENGTH
};
# endif
# ifndef OPENSSL_NO_RMD160
/* RIPEMD160 (1 3 36 3 2 1) */
static const unsigned char digestinfo_ripemd160_der[] = {
    ASN1_SEQUENCE, 0x0d + RIPEMD160_DIGEST_LENGTH,
      ASN1_SEQUENCE, 0x09,
        ASN1_OID, 0x05, 1 * 40 + 3, 36, 3, 2, 1,
        ASN1_NULL, 0x00,
      ASN1_OCTET_STRING, RIPEMD160_DIGEST_LENGTH
};
# endif
# ifndef OPENSSL_NO_SM3
/* SM3 (1 2 156 10197 1 401) */
static const unsigned char digestinfo_sm3_der[] = {
    ASN1_SEQUENCE, 0x0f + SM3_DIGEST_LENGTH,
      ASN1_SEQUENCE, 0x0c,
        ASN1_OID, 0x08, 1 * 40 + 2, 0x81, 0x1c, 0xcf, 0x55, 1, 0x83, 0x78,
        ASN1_NULL, 0x00,
      ASN1_OCTET_STRING, SM3_DIGEST_LENGTH
};
# endif
#endif /* FIPS_MODULE */

/* SHA-1 (1 3 14 3 2 26) */
static const unsigned char digestinfo_sha1_der[] = {
    ASN1_SEQUENCE, 0x0d + SHA_DIGEST_LENGTH,
      ASN1_SEQUENCE, 0x09,
        ASN1_OID, 0x05, 1 * 40 + 3, 14, 3, 2, 26,
        ASN1_NULL, 0x00,
      ASN1_OCTET_STRING, SHA_DIGEST_LENGTH
};

ENCODE_DIGESTINFO_SHA(sha256, 0x01, SHA256_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha384, 0x02, SHA384_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha512, 0x03, SHA512_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha224, 0x04, SHA224_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha512_224, 0x05, SHA224_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha512_256, 0x06, SHA256_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha3_224, 0x07, SHA224_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha3_256, 0x08, SHA256_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha3_384, 0x09, SHA384_DIGEST_LENGTH)
ENCODE_DIGESTINFO_SHA(sha3_512, 0x0a, SHA512_DIGEST_LENGTH)

#define MD_CASE(name)                                                          \
    case NID_##name:                                                           \
        *len = sizeof(digestinfo_##name##_der);                                \
        return digestinfo_##name##_der;

const unsigned char *ossl_rsa_digestinfo_encoding(int md_nid, size_t *len)
{
    switch (md_nid) {
#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_MDC2
    MD_CASE(mdc2)
# endif
# ifndef OPENSSL_NO_MD2
    MD_CASE(md2)
# endif
# ifndef OPENSSL_NO_MD4
    MD_CASE(md4)
# endif
# ifndef OPENSSL_NO_MD5
    MD_CASE(md5)
# endif
# ifndef OPENSSL_NO_RMD160
    MD_CASE(ripemd160)
# endif
# ifndef OPENSSL_NO_SM3
    MD_CASE(sm3)
# endif
#endif /* FIPS_MODULE */
    MD_CASE(sha1)
    MD_CASE(sha224)
    MD_CASE(sha256)
    MD_CASE(sha384)
    MD_CASE(sha512)
    MD_CASE(sha512_224)
    MD_CASE(sha512_256)
    MD_CASE(sha3_224)
    MD_CASE(sha3_256)
    MD_CASE(sha3_384)
    MD_CASE(sha3_512)
    default:
        return NULL;
    }
}

#define MD_NID_CASE(name, sz)                                                  \
    case NID_##name:                                                           \
        return sz;

static int digest_sz_from_nid(int nid)
{
    switch (nid) {
#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_MDC2
    MD_NID_CASE(mdc2, MDC2_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MD2
    MD_NID_CASE(md2, MD2_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MD4
    MD_NID_CASE(md4, MD4_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_MD5
    MD_NID_CASE(md5, MD5_DIGEST_LENGTH)
# endif
# ifndef OPENSSL_NO_RMD160
    MD_NID_CASE(ripemd160, RIPEMD160_DIGEST_LENGTH)
# endif
#endif /* FIPS_MODULE */
    MD_NID_CASE(sha1, SHA_DIGEST_LENGTH)
    MD_NID_CASE(sha224, SHA224_DIGEST_LENGTH)
    MD_NID_CASE(sha256, SHA256_DIGEST_LENGTH)
    MD_NID_CASE(sha384, SHA384_DIGEST_LENGTH)
    MD_NID_CASE(sha512, SHA512_DIGEST_LENGTH)
    MD_NID_CASE(sha512_224, SHA224_DIGEST_LENGTH)
    MD_NID_CASE(sha512_256, SHA256_DIGEST_LENGTH)
    MD_NID_CASE(sha3_224, SHA224_DIGEST_LENGTH)
    MD_NID_CASE(sha3_256, SHA256_DIGEST_LENGTH)
    MD_NID_CASE(sha3_384, SHA384_DIGEST_LENGTH)
    MD_NID_CASE(sha3_512, SHA512_DIGEST_LENGTH)
    default:
        return 0;
    }
}


/* Size of an SSL signature: MD5+SHA1 */
#define SSL_SIG_LENGTH  36

/*
 * Encodes a DigestInfo prefix of hash |type| and digest |m|, as
 * described in EMSA-PKCS1-v1_5-ENCODE, RFC 3447 section 9.2 step 2. This
 * encodes the DigestInfo (T and tLen) but does not add the padding.
 *
 * On success, it returns one and sets |*out| to a newly allocated buffer
 * containing the result and |*out_len| to its length. The caller must free
 * |*out| with OPENSSL_free(). Otherwise, it returns zero.
 */
static int encode_pkcs1(unsigned char **out, size_t *out_len, int type,
                        const unsigned char *m, size_t m_len)
{
    size_t di_prefix_len, dig_info_len;
    const unsigned char *di_prefix;
    unsigned char *dig_info;

    if (type == NID_undef) {
        ERR_raise(ERR_LIB_RSA, RSA_R_UNKNOWN_ALGORITHM_TYPE);
        return 0;
    }
    di_prefix = ossl_rsa_digestinfo_encoding(type, &di_prefix_len);
    if (di_prefix == NULL) {
        ERR_raise(ERR_LIB_RSA,
                  RSA_R_THE_ASN1_OBJECT_IDENTIFIER_IS_NOT_KNOWN_FOR_THIS_MD);
        return 0;
    }
    dig_info_len = di_prefix_len + m_len;
    dig_info = OPENSSL_malloc(dig_info_len);
    if (dig_info == NULL)
        return 0;
    memcpy(dig_info, di_prefix, di_prefix_len);
    memcpy(dig_info + di_prefix_len, m, m_len);

    *out = dig_info;
    *out_len = dig_info_len;
    return 1;
}

int RSA_sign(int type, const unsigned char *m, unsigned int m_len,
             unsigned char *sigret, unsigned int *siglen, RSA *rsa)
{
    int encrypt_len, ret = 0;
    size_t encoded_len = 0;
    unsigned char *tmps = NULL;
    const unsigned char *encoded = NULL;

#ifndef FIPS_MODULE
    if (rsa->meth->rsa_sign != NULL)
        return rsa->meth->rsa_sign(type, m, m_len, sigret, siglen, rsa) > 0;
#endif /* FIPS_MODULE */

    /* Compute the encoded digest. */
    if (type == NID_md5_sha1) {
        /*
         * NID_md5_sha1 corresponds to the MD5/SHA1 combination in TLS 1.1 and
         * earlier. It has no DigestInfo wrapper but otherwise is
         * RSASSA-PKCS1-v1_5.
         */
        if (m_len != SSL_SIG_LENGTH) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MESSAGE_LENGTH);
            return 0;
        }
        encoded_len = SSL_SIG_LENGTH;
        encoded = m;
    } else {
        if (!encode_pkcs1(&tmps, &encoded_len, type, m, m_len))
            goto err;
        encoded = tmps;
    }

    if (encoded_len + RSA_PKCS1_PADDING_SIZE > (size_t)RSA_size(rsa)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DIGEST_TOO_BIG_FOR_RSA_KEY);
        goto err;
    }
    encrypt_len = RSA_private_encrypt((int)encoded_len, encoded, sigret, rsa,
                                      RSA_PKCS1_PADDING);
    if (encrypt_len <= 0)
        goto err;

    *siglen = encrypt_len;
    ret = 1;

err:
    OPENSSL_clear_free(tmps, encoded_len);
    return ret;
}

/*
 * Verify an RSA signature in |sigbuf| using |rsa|.
 * |type| is the NID of the digest algorithm to use.
 * If |rm| is NULL, it verifies the signature for digest |m|, otherwise
 * it recovers the digest from the signature, writing the digest to |rm| and
 * the length to |*prm_len|.
 *
 * It returns one on successful verification or zero otherwise.
 */
int ossl_rsa_verify(int type, const unsigned char *m, unsigned int m_len,
                    unsigned char *rm, size_t *prm_len,
                    const unsigned char *sigbuf, size_t siglen, RSA *rsa)
{
    int len, ret = 0;
    size_t decrypt_len, encoded_len = 0;
    unsigned char *decrypt_buf = NULL, *encoded = NULL;

    if (siglen != (size_t)RSA_size(rsa)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_WRONG_SIGNATURE_LENGTH);
        return 0;
    }

    /* Recover the encoded digest. */
    decrypt_buf = OPENSSL_malloc(siglen);
    if (decrypt_buf == NULL)
        goto err;

    len = RSA_public_decrypt((int)siglen, sigbuf, decrypt_buf, rsa,
                             RSA_PKCS1_PADDING);
    if (len <= 0)
        goto err;
    decrypt_len = len;

#ifndef FIPS_MODULE
    if (type == NID_md5_sha1) {
        /*
         * NID_md5_sha1 corresponds to the MD5/SHA1 combination in TLS 1.1 and
         * earlier. It has no DigestInfo wrapper but otherwise is
         * RSASSA-PKCS1-v1_5.
         */
        if (decrypt_len != SSL_SIG_LENGTH) {
            ERR_raise(ERR_LIB_RSA, RSA_R_BAD_SIGNATURE);
            goto err;
        }

        if (rm != NULL) {
            memcpy(rm, decrypt_buf, SSL_SIG_LENGTH);
            *prm_len = SSL_SIG_LENGTH;
        } else {
            if (m_len != SSL_SIG_LENGTH) {
                ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MESSAGE_LENGTH);
                goto err;
            }

            if (memcmp(decrypt_buf, m, SSL_SIG_LENGTH) != 0) {
                ERR_raise(ERR_LIB_RSA, RSA_R_BAD_SIGNATURE);
                goto err;
            }
        }
    } else if (type == NID_mdc2 && decrypt_len == 2 + 16
               && decrypt_buf[0] == 0x04 && decrypt_buf[1] == 0x10) {
        /*
         * Oddball MDC2 case: signature can be OCTET STRING. check for correct
         * tag and length octets.
         */
        if (rm != NULL) {
            memcpy(rm, decrypt_buf + 2, 16);
            *prm_len = 16;
        } else {
            if (m_len != 16) {
                ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_MESSAGE_LENGTH);
                goto err;
            }

            if (memcmp(m, decrypt_buf + 2, 16) != 0) {
                ERR_raise(ERR_LIB_RSA, RSA_R_BAD_SIGNATURE);
                goto err;
            }
        }
    } else
#endif /* FIPS_MODULE */
    {
        /*
         * If recovering the digest, extract a digest-sized output from the end
         * of |decrypt_buf| for |encode_pkcs1|, then compare the decryption
         * output as in a standard verification.
         */
        if (rm != NULL) {
            len = digest_sz_from_nid(type);

            if (len <= 0)
                goto err;
            m_len = (unsigned int)len;
            if (m_len > decrypt_len) {
                ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_DIGEST_LENGTH);
                goto err;
            }
            m = decrypt_buf + decrypt_len - m_len;
        }

        /* Construct the encoded digest and ensure it matches. */
        if (!encode_pkcs1(&encoded, &encoded_len, type, m, m_len))
            goto err;

        if (encoded_len != decrypt_len
                || memcmp(encoded, decrypt_buf, encoded_len) != 0) {
            ERR_raise(ERR_LIB_RSA, RSA_R_BAD_SIGNATURE);
            goto err;
        }

        /* Output the recovered digest. */
        if (rm != NULL) {
            memcpy(rm, m, m_len);
            *prm_len = m_len;
        }
    }

    ret = 1;

err:
    OPENSSL_clear_free(encoded, encoded_len);
    OPENSSL_clear_free(decrypt_buf, siglen);
    return ret;
}

int RSA_verify(int type, const unsigned char *m, unsigned int m_len,
               const unsigned char *sigbuf, unsigned int siglen, RSA *rsa)
{

    if (rsa->meth->rsa_verify != NULL)
        return rsa->meth->rsa_verify(type, m, m_len, sigbuf, siglen, rsa);

    return ossl_rsa_verify(type, m, m_len, NULL, NULL, sigbuf, siglen, rsa);
}
