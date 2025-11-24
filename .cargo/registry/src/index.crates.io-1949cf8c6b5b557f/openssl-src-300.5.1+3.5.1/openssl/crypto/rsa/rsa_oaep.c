/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* EME-OAEP as defined in RFC 2437 (PKCS #1 v2.0) */

/*
 * See Victor Shoup, "OAEP reconsidered," Nov. 2000, <URL:
 * http://www.shoup.net/papers/oaep.ps.Z> for problems with the security
 * proof for the original OAEP scheme, which EME-OAEP is based on. A new
 * proof can be found in E. Fujisaki, T. Okamoto, D. Pointcheval, J. Stern,
 * "RSA-OEAP is Still Alive!", Dec. 2000, <URL:
 * http://eprint.iacr.org/2000/061/>. The new proof has stronger requirements
 * for the underlying permutation: "partial-one-wayness" instead of
 * one-wayness.  For the RSA function, this is an equivalent notion.
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "internal/constant_time.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include "rsa_local.h"

int RSA_padding_add_PKCS1_OAEP(unsigned char *to, int tlen,
                               const unsigned char *from, int flen,
                               const unsigned char *param, int plen)
{
    return ossl_rsa_padding_add_PKCS1_OAEP_mgf1_ex(NULL, to, tlen, from, flen,
                                                   param, plen, NULL, NULL);
}

/*
 * Perform the padding as per NIST 800-56B 7.2.2.3
 *      from (K) is the key material.
 *      param (A) is the additional input.
 * Step numbers are included here but not in the constant time inverse below
 * to avoid complicating an already difficult enough function.
 */
int ossl_rsa_padding_add_PKCS1_OAEP_mgf1_ex(OSSL_LIB_CTX *libctx,
                                            unsigned char *to, int tlen,
                                            const unsigned char *from, int flen,
                                            const unsigned char *param,
                                            int plen, const EVP_MD *md,
                                            const EVP_MD *mgf1md)
{
    int rv = 0;
    int i, emlen = tlen - 1;
    unsigned char *db, *seed;
    unsigned char *dbmask = NULL;
    unsigned char seedmask[EVP_MAX_MD_SIZE];
    int mdlen, dbmask_len = 0;

    if (md == NULL) {
#ifndef FIPS_MODULE
        md = EVP_sha1();
#else
        ERR_raise(ERR_LIB_RSA, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
#endif
    }
    if (mgf1md == NULL)
        mgf1md = md;

#ifdef FIPS_MODULE
    /* XOF are approved as standalone; Shake256 in Ed448; MGF */
    if (EVP_MD_xof(md)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DIGEST_NOT_ALLOWED);
        return 0;
    }
    if (EVP_MD_xof(mgf1md)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_MGF1_DIGEST_NOT_ALLOWED);
        return 0;
    }
#endif

    mdlen = EVP_MD_get_size(md);
    if (mdlen <= 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_LENGTH);
        return 0;
    }

    /* step 2b: check KLen > nLen - 2 HLen - 2 */
    if (flen > emlen - 2 * mdlen - 1) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE_FOR_KEY_SIZE);
        return 0;
    }

    if (emlen < 2 * mdlen + 1) {
        ERR_raise(ERR_LIB_RSA, RSA_R_KEY_SIZE_TOO_SMALL);
        return 0;
    }

    /* step 3i: EM = 00000000 || maskedMGF || maskedDB */
    to[0] = 0;
    seed = to + 1;
    db = to + mdlen + 1;

    /* step 3a: hash the additional input */
    if (!EVP_Digest((void *)param, plen, db, NULL, md, NULL))
        goto err;
    /* step 3b: zero bytes array of length nLen - KLen - 2 HLen -2 */
    memset(db + mdlen, 0, emlen - flen - 2 * mdlen - 1);
    /* step 3c: DB = HA || PS || 00000001 || K */
    db[emlen - flen - mdlen - 1] = 0x01;
    memcpy(db + emlen - flen - mdlen, from, (unsigned int)flen);
    /* step 3d: generate random byte string */
    if (RAND_bytes_ex(libctx, seed, mdlen, 0) <= 0)
        goto err;

    dbmask_len = emlen - mdlen;
    dbmask = OPENSSL_malloc(dbmask_len);
    if (dbmask == NULL)
        goto err;

    /* step 3e: dbMask = MGF(mgfSeed, nLen - HLen - 1) */
    if (PKCS1_MGF1(dbmask, dbmask_len, seed, mdlen, mgf1md) < 0)
        goto err;
    /* step 3f: maskedDB = DB XOR dbMask */
    for (i = 0; i < dbmask_len; i++)
        db[i] ^= dbmask[i];

    /* step 3g: mgfSeed = MGF(maskedDB, HLen) */
    if (PKCS1_MGF1(seedmask, mdlen, db, dbmask_len, mgf1md) < 0)
        goto err;
    /* stepo 3h: maskedMGFSeed = mgfSeed XOR mgfSeedMask */
    for (i = 0; i < mdlen; i++)
        seed[i] ^= seedmask[i];
    rv = 1;

 err:
    OPENSSL_cleanse(seedmask, sizeof(seedmask));
    OPENSSL_clear_free(dbmask, dbmask_len);
    return rv;
}

int RSA_padding_add_PKCS1_OAEP_mgf1(unsigned char *to, int tlen,
                                    const unsigned char *from, int flen,
                                    const unsigned char *param, int plen,
                                    const EVP_MD *md, const EVP_MD *mgf1md)
{
    return ossl_rsa_padding_add_PKCS1_OAEP_mgf1_ex(NULL, to, tlen, from, flen,
                                                   param, plen, md, mgf1md);
}

int RSA_padding_check_PKCS1_OAEP(unsigned char *to, int tlen,
                                 const unsigned char *from, int flen, int num,
                                 const unsigned char *param, int plen)
{
    return RSA_padding_check_PKCS1_OAEP_mgf1(to, tlen, from, flen, num,
                                             param, plen, NULL, NULL);
}

int RSA_padding_check_PKCS1_OAEP_mgf1(unsigned char *to, int tlen,
                                      const unsigned char *from, int flen,
                                      int num, const unsigned char *param,
                                      int plen, const EVP_MD *md,
                                      const EVP_MD *mgf1md)
{
    int i, dblen = 0, mlen = -1, one_index = 0, msg_index;
    unsigned int good = 0, found_one_byte, mask;
    const unsigned char *maskedseed, *maskeddb;
    /*
     * |em| is the encoded message, zero-padded to exactly |num| bytes: em =
     * Y || maskedSeed || maskedDB
     */
    unsigned char *db = NULL, *em = NULL, seed[EVP_MAX_MD_SIZE],
        phash[EVP_MAX_MD_SIZE];
    int mdlen;

    if (md == NULL) {
#ifndef FIPS_MODULE
        md = EVP_sha1();
#else
        ERR_raise(ERR_LIB_RSA, ERR_R_PASSED_NULL_PARAMETER);
        return -1;
#endif
    }

    if (mgf1md == NULL)
        mgf1md = md;

#ifdef FIPS_MODULE
    /* XOF are approved as standalone; Shake256 in Ed448; MGF */
    if (EVP_MD_xof(md)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DIGEST_NOT_ALLOWED);
        return -1;
    }
    if (EVP_MD_xof(mgf1md)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_MGF1_DIGEST_NOT_ALLOWED);
        return -1;
    }
#endif

    mdlen = EVP_MD_get_size(md);

    if (tlen <= 0 || flen <= 0 || mdlen <= 0)
        return -1;
    /*
     * |num| is the length of the modulus; |flen| is the length of the
     * encoded message. Therefore, for any |from| that was obtained by
     * decrypting a ciphertext, we must have |flen| <= |num|. Similarly,
     * |num| >= 2 * |mdlen| + 2 must hold for the modulus irrespective of
     * the ciphertext, see PKCS #1 v2.2, section 7.1.2.
     * This does not leak any side-channel information.
     */
    if (num < flen || num < 2 * mdlen + 2) {
        ERR_raise(ERR_LIB_RSA, RSA_R_OAEP_DECODING_ERROR);
        return -1;
    }

    dblen = num - mdlen - 1;
    db = OPENSSL_malloc(dblen);
    if (db == NULL)
        goto cleanup;

    em = OPENSSL_malloc(num);
    if (em == NULL)
        goto cleanup;

    /*
     * Caller is encouraged to pass zero-padded message created with
     * BN_bn2binpad. Trouble is that since we can't read out of |from|'s
     * bounds, it's impossible to have an invariant memory access pattern
     * in case |from| was not zero-padded in advance.
     */
    for (from += flen, em += num, i = 0; i < num; i++) {
        mask = ~constant_time_is_zero(flen);
        flen -= 1 & mask;
        from -= 1 & mask;
        *--em = *from & mask;
    }

    /*
     * The first byte must be zero, however we must not leak if this is
     * true. See James H. Manger, "A Chosen Ciphertext  Attack on RSA
     * Optimal Asymmetric Encryption Padding (OAEP) [...]", CRYPTO 2001).
     */
    good = constant_time_is_zero(em[0]);

    maskedseed = em + 1;
    maskeddb = em + 1 + mdlen;

    if (PKCS1_MGF1(seed, mdlen, maskeddb, dblen, mgf1md))
        goto cleanup;
    for (i = 0; i < mdlen; i++)
        seed[i] ^= maskedseed[i];

    if (PKCS1_MGF1(db, dblen, seed, mdlen, mgf1md))
        goto cleanup;
    for (i = 0; i < dblen; i++)
        db[i] ^= maskeddb[i];

    if (!EVP_Digest((void *)param, plen, phash, NULL, md, NULL))
        goto cleanup;

    good &= constant_time_is_zero(CRYPTO_memcmp(db, phash, mdlen));

    found_one_byte = 0;
    for (i = mdlen; i < dblen; i++) {
        /*
         * Padding consists of a number of 0-bytes, followed by a 1.
         */
        unsigned int equals1 = constant_time_eq(db[i], 1);
        unsigned int equals0 = constant_time_is_zero(db[i]);
        one_index = constant_time_select_int(~found_one_byte & equals1,
                                             i, one_index);
        found_one_byte |= equals1;
        good &= (found_one_byte | equals0);
    }

    good &= found_one_byte;

    /*
     * At this point |good| is zero unless the plaintext was valid,
     * so plaintext-awareness ensures timing side-channels are no longer a
     * concern.
     */
    msg_index = one_index + 1;
    mlen = dblen - msg_index;

    /*
     * For good measure, do this check in constant time as well.
     */
    good &= constant_time_ge(tlen, mlen);

    /*
     * Move the result in-place by |dblen|-|mdlen|-1-|mlen| bytes to the left.
     * Then if |good| move |mlen| bytes from |db|+|mdlen|+1 to |to|.
     * Otherwise leave |to| unchanged.
     * Copy the memory back in a way that does not reveal the size of
     * the data being copied via a timing side channel. This requires copying
     * parts of the buffer multiple times based on the bits set in the real
     * length. Clear bits do a non-copy with identical access pattern.
     * The loop below has overall complexity of O(N*log(N)).
     */
    tlen = constant_time_select_int(constant_time_lt(dblen - mdlen - 1, tlen),
                                    dblen - mdlen - 1, tlen);
    for (msg_index = 1; msg_index < dblen - mdlen - 1; msg_index <<= 1) {
        mask = ~constant_time_eq(msg_index & (dblen - mdlen - 1 - mlen), 0);
        for (i = mdlen + 1; i < dblen - msg_index; i++)
            db[i] = constant_time_select_8(mask, db[i + msg_index], db[i]);
    }
    for (i = 0; i < tlen; i++) {
        mask = good & constant_time_lt(i, mlen);
        to[i] = constant_time_select_8(mask, db[i + mdlen + 1], to[i]);
    }

#ifndef FIPS_MODULE
    /*
     * To avoid chosen ciphertext attacks, the error message should not
     * reveal which kind of decoding error happened.
     *
     * This trick doesn't work in the FIPS provider because libcrypto manages
     * the error stack. Instead we opt not to put an error on the stack at all
     * in case of padding failure in the FIPS provider.
     */
    ERR_raise(ERR_LIB_RSA, RSA_R_OAEP_DECODING_ERROR);
    err_clear_last_constant_time(1 & good);
#endif
 cleanup:
    OPENSSL_cleanse(seed, sizeof(seed));
    OPENSSL_clear_free(db, dblen);
    OPENSSL_clear_free(em, num);

    return constant_time_select_int(good, mlen, -1);
}

/*
 * Mask Generation Function corresponding to section 7.2.2.2 of NIST SP 800-56B.
 * The variables are named differently to NIST:
 *      mask (T) and len (maskLen)are the returned mask.
 *      seed (mgfSeed).
 * The range checking steps inm the process are performed outside.
 */
int PKCS1_MGF1(unsigned char *mask, long len,
               const unsigned char *seed, long seedlen, const EVP_MD *dgst)
{
    long i, outlen = 0;
    unsigned char cnt[4];
    EVP_MD_CTX *c = EVP_MD_CTX_new();
    unsigned char md[EVP_MAX_MD_SIZE];
    int mdlen;
    int rv = -1;

    if (c == NULL)
        goto err;
    mdlen = EVP_MD_get_size(dgst);
    if (mdlen <= 0)
        goto err;
    /* step 4 */
    for (i = 0; outlen < len; i++) {
        /* step 4a: D = I2BS(counter, 4) */
        cnt[0] = (unsigned char)((i >> 24) & 255);
        cnt[1] = (unsigned char)((i >> 16) & 255);
        cnt[2] = (unsigned char)((i >> 8)) & 255;
        cnt[3] = (unsigned char)(i & 255);
        /* step 4b: T =T || hash(mgfSeed || D) */
        if (!EVP_DigestInit_ex(c, dgst, NULL)
            || !EVP_DigestUpdate(c, seed, seedlen)
            || !EVP_DigestUpdate(c, cnt, 4))
            goto err;
        if (outlen + mdlen <= len) {
            if (!EVP_DigestFinal_ex(c, mask + outlen, NULL))
                goto err;
            outlen += mdlen;
        } else {
            if (!EVP_DigestFinal_ex(c, md, NULL))
                goto err;
            memcpy(mask + outlen, md, len - outlen);
            outlen = len;
        }
    }
    rv = 0;
 err:
    OPENSSL_cleanse(md, sizeof(md));
    EVP_MD_CTX_free(c);
    return rv;
}
