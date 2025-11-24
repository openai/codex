/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/kdf.h>
#include "internal/deterministic_nonce.h"
#include "crypto/bn.h"

/*
 * Convert a Bit String to an Integer (See RFC 6979 Section 2.3.2)
 *
 * Params:
 *     out The returned Integer as a BIGNUM
 *     qlen_bits The maximum size of the returned integer in bits. The returned
 *        Integer is shifted right if inlen is larger than qlen_bits..
 *     in, inlen The input Bit String (in bytes).
 * Returns: 1 if successful, or  0 otherwise.
 */
static int bits2int(BIGNUM *out, int qlen_bits,
                    const unsigned char *in, size_t inlen)
{
    int blen_bits = inlen * 8;
    int shift;

    if (BN_bin2bn(in, (int)inlen, out) == NULL)
        return 0;

    shift = blen_bits - qlen_bits;
    if (shift > 0)
        return BN_rshift(out, out, shift);
    return 1;
}

/*
 * Convert as above a Bit String in const time to an Integer w fixed top
 *
 * Params:
 *     out The returned Integer as a BIGNUM
 *     qlen_bits The maximum size of the returned integer in bits. The returned
 *        Integer is shifted right if inlen is larger than qlen_bits..
 *     in, inlen The input Bit String (in bytes). It has sizeof(BN_ULONG) bytes
 *               prefix with all bits set that needs to be cleared out after
 *               the conversion.
 * Returns: 1 if successful, or  0 otherwise.
 */
static int bits2int_consttime(BIGNUM *out, int qlen_bits,
                              const unsigned char *in, size_t inlen)
{
    int blen_bits = (inlen - sizeof(BN_ULONG)) * 8;
    int shift;

    if (BN_bin2bn(in, (int)inlen, out) == NULL)
        return 0;

    BN_set_flags(out, BN_FLG_CONSTTIME);
    ossl_bn_mask_bits_fixed_top(out, blen_bits);

    shift = blen_bits - qlen_bits;
    if (shift > 0)
        return bn_rshift_fixed_top(out, out, shift);
    return 1;
}

/*
 * Convert an Integer to an Octet String (See RFC 6979 2.3.3).
 * The value is zero padded if required.
 *
 * Params:
 *     out The returned Octet String
 *     num The input Integer
 *     rlen The required size of the returned Octet String in bytes
 * Returns: 1 if successful, or  0 otherwise.
 */
static int int2octets(unsigned char *out, const BIGNUM *num, int rlen)
{
    return BN_bn2binpad(num, out, rlen) >= 0;
}

/*
 * Convert a Bit String to an Octet String (See RFC 6979 Section 2.3.4)
 *
 * Params:
 *     out The returned octet string.
 *     q The modulus
 *     qlen_bits The length of q in bits
 *     rlen The value of qlen_bits rounded up to the nearest 8 bits.
 *     in, inlen The input bit string (in bytes)
 * Returns: 1 if successful, or  0 otherwise.
 */
static int bits2octets(unsigned char *out, const BIGNUM *q, int qlen_bits,
                       int rlen, const unsigned char *in, size_t inlen)
{
   int ret = 0;
   BIGNUM *z = BN_new();

   if (z == NULL
           || !bits2int(z, qlen_bits, in, inlen))
       goto err;

   /* z2 = z1 mod q (Do a simple subtract, since z1 < 2^qlen_bits) */
   if (BN_cmp(z, q) >= 0
           && !BN_usub(z, z, q))
       goto err;

   ret = int2octets(out, z, rlen);
err:
   BN_free(z);
   return ret;
}

/*
 * Setup a KDF HMAC_DRBG object using fixed entropy and nonce data.
 *
 * Params:
 *     digestname The digest name for the HMAC
 *     entropy, entropylen A fixed input entropy buffer
 *     nonce, noncelen A fixed input nonce buffer
 *     libctx, propq Are used for fetching algorithms
 *
 * Returns: The created KDF HMAC_DRBG object if successful, or NULL otherwise.
 */
static EVP_KDF_CTX *kdf_setup(const char *digestname,
                              const unsigned char *entropy, size_t entropylen,
                              const unsigned char *nonce, size_t noncelen,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    EVP_KDF_CTX *ctx = NULL;
    EVP_KDF *kdf = NULL;
    OSSL_PARAM params[5], *p;

    kdf = EVP_KDF_fetch(libctx, "HMAC-DRBG-KDF", propq);
    ctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (ctx == NULL)
        goto err;

    p = params;
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)digestname, 0);
    if (propq != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_PROPERTIES,
                                                (char *)propq, 0);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_HMACDRBG_ENTROPY,
                                             (void *)entropy, entropylen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_HMACDRBG_NONCE,
                                             (void *)nonce, noncelen);
    *p = OSSL_PARAM_construct_end();

    if (EVP_KDF_CTX_set_params(ctx, params) <= 0)
        goto err;

    return ctx;
err:
    EVP_KDF_CTX_free(ctx);
    return NULL;
}

/*
 * Generate a Deterministic nonce 'k' for DSA/ECDSA as defined in
 * RFC 6979 Section 3.3.  "Alternate Description of the Generation of k"
 *
 * Params:
 *     out Returns the generated deterministic nonce 'k'
 *     q A large prime number used for modulus operations for DSA and ECDSA.
 *     priv The private key in the range [1, q-1]
 *     hm, hmlen The digested message buffer in bytes
 *     digestname The digest name used for signing. It is used as the HMAC digest.
 *     libctx, propq Used for fetching algorithms
 *
 * Returns: 1 if successful, or  0 otherwise.
 */
int ossl_gen_deterministic_nonce_rfc6979(BIGNUM *out, const BIGNUM *q,
                                         const BIGNUM *priv,
                                         const unsigned char *hm, size_t hmlen,
                                         const char *digestname,
                                         OSSL_LIB_CTX *libctx,
                                         const char *propq)
{
    EVP_KDF_CTX *kdfctx = NULL;
    int ret = 0, rlen = 0, qlen_bits = 0;
    unsigned char *entropyx = NULL, *nonceh = NULL, *rbits = NULL, *T = NULL;
    size_t allocsz = 0;
    const size_t prefsz = sizeof(BN_ULONG);

    if (out == NULL)
        return 0;

    qlen_bits = BN_num_bits(q);
    if (qlen_bits == 0)
        return 0;

    /* Note rlen used here is in bytes since the input values are byte arrays */
    rlen = (qlen_bits + 7) / 8;
    allocsz = prefsz + 3 * rlen;

    /* Use a single alloc for the buffers T, nonceh and entropyx */
    T = (unsigned char *)OPENSSL_zalloc(allocsz);
    if (T == NULL)
        return 0;
    rbits = T + prefsz;
    nonceh = rbits + rlen;
    entropyx = nonceh + rlen;

    memset(T, 0xff, prefsz);

    if (!int2octets(entropyx, priv, rlen)
            || !bits2octets(nonceh, q, qlen_bits, rlen, hm, hmlen))
        goto end;

    kdfctx = kdf_setup(digestname, entropyx, rlen, nonceh, rlen, libctx, propq);
    if (kdfctx == NULL)
        goto end;

    do {
        if (!EVP_KDF_derive(kdfctx, rbits, rlen, NULL)
                || !bits2int_consttime(out, qlen_bits, T, rlen + prefsz))
            goto end;
    } while (ossl_bn_is_word_fixed_top(out, 0)
            || ossl_bn_is_word_fixed_top(out, 1)
            || BN_ucmp(out, q) >= 0);
#ifdef BN_DEBUG
    /* With BN_DEBUG on a fixed top number cannot be returned */
    bn_correct_top(out);
#endif
    ret = 1;

end:
    EVP_KDF_CTX_free(kdfctx);
    OPENSSL_clear_free(T, allocsz);
    return ret;
}
