/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include "internal/cryptlib.h"
#include "crypto/rand.h"
#include "bn_local.h"
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <openssl/evp.h>

typedef enum bnrand_flag_e {
    NORMAL, TESTING, PRIVATE
} BNRAND_FLAG;

static int bnrand(BNRAND_FLAG flag, BIGNUM *rnd, int bits, int top, int bottom,
                  unsigned int strength, BN_CTX *ctx)
{
    unsigned char *buf = NULL;
    int b, ret = 0, bit, bytes, mask;
    OSSL_LIB_CTX *libctx = ossl_bn_get_libctx(ctx);

    if (bits == 0) {
        if (top != BN_RAND_TOP_ANY || bottom != BN_RAND_BOTTOM_ANY)
            goto toosmall;
        BN_zero(rnd);
        return 1;
    }
    if (bits < 0 || (bits == 1 && top > 0))
        goto toosmall;

    bytes = (bits + 7) / 8;
    bit = (bits - 1) % 8;
    mask = 0xff << (bit + 1);

    buf = OPENSSL_malloc(bytes);
    if (buf == NULL)
        goto err;

    /* make a random number and set the top and bottom bits */
    b = flag == NORMAL ? RAND_bytes_ex(libctx, buf, bytes, strength)
                       : RAND_priv_bytes_ex(libctx, buf, bytes, strength);
    if (b <= 0)
        goto err;

    if (flag == TESTING) {
        /*
         * generate patterns that are more likely to trigger BN library bugs
         */
        int i;
        unsigned char c;

        for (i = 0; i < bytes; i++) {
            if (RAND_bytes_ex(libctx, &c, 1, strength) <= 0)
                goto err;
            if (c >= 128 && i > 0)
                buf[i] = buf[i - 1];
            else if (c < 42)
                buf[i] = 0;
            else if (c < 84)
                buf[i] = 255;
        }
    }

    if (top >= 0) {
        if (top) {
            if (bit == 0) {
                buf[0] = 1;
                buf[1] |= 0x80;
            } else {
                buf[0] |= (3 << (bit - 1));
            }
        } else {
            buf[0] |= (1 << bit);
        }
    }
    buf[0] &= ~mask;
    if (bottom)                 /* set bottom bit if requested */
        buf[bytes - 1] |= 1;
    if (!BN_bin2bn(buf, bytes, rnd))
        goto err;
    ret = 1;
 err:
    OPENSSL_clear_free(buf, bytes);
    bn_check_top(rnd);
    return ret;

toosmall:
    ERR_raise(ERR_LIB_BN, BN_R_BITS_TOO_SMALL);
    return 0;
}

int BN_rand_ex(BIGNUM *rnd, int bits, int top, int bottom,
               unsigned int strength, BN_CTX *ctx)
{
    return bnrand(NORMAL, rnd, bits, top, bottom, strength, ctx);
}
#ifndef FIPS_MODULE
int BN_rand(BIGNUM *rnd, int bits, int top, int bottom)
{
    return bnrand(NORMAL, rnd, bits, top, bottom, 0, NULL);
}

int BN_bntest_rand(BIGNUM *rnd, int bits, int top, int bottom)
{
    return bnrand(TESTING, rnd, bits, top, bottom, 0, NULL);
}
#endif

int BN_priv_rand_ex(BIGNUM *rnd, int bits, int top, int bottom,
                    unsigned int strength, BN_CTX *ctx)
{
    return bnrand(PRIVATE, rnd, bits, top, bottom, strength, ctx);
}

#ifndef FIPS_MODULE
int BN_priv_rand(BIGNUM *rnd, int bits, int top, int bottom)
{
    return bnrand(PRIVATE, rnd, bits, top, bottom, 0, NULL);
}
#endif

/* random number r:  0 <= r < range */
static int bnrand_range(BNRAND_FLAG flag, BIGNUM *r, const BIGNUM *range,
                        unsigned int strength, BN_CTX *ctx)
{
    int n;
    int count = 100;

    if (r == NULL) {
        ERR_raise(ERR_LIB_BN, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (range->neg || BN_is_zero(range)) {
        ERR_raise(ERR_LIB_BN, BN_R_INVALID_RANGE);
        return 0;
    }

    n = BN_num_bits(range);     /* n > 0 */

    /* BN_is_bit_set(range, n - 1) always holds */

    if (n == 1)
        BN_zero(r);
    else if (!BN_is_bit_set(range, n - 2) && !BN_is_bit_set(range, n - 3)) {
        /*
         * range = 100..._2, so 3*range (= 11..._2) is exactly one bit longer
         * than range
         */
        do {
            if (!bnrand(flag, r, n + 1, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY,
                        strength, ctx))
                return 0;

            /*
             * If r < 3*range, use r := r MOD range (which is either r, r -
             * range, or r - 2*range). Otherwise, iterate once more. Since
             * 3*range = 11..._2, each iteration succeeds with probability >=
             * .75.
             */
            if (BN_cmp(r, range) >= 0) {
                if (!BN_sub(r, r, range))
                    return 0;
                if (BN_cmp(r, range) >= 0)
                    if (!BN_sub(r, r, range))
                        return 0;
            }

            if (!--count) {
                ERR_raise(ERR_LIB_BN, BN_R_TOO_MANY_ITERATIONS);
                return 0;
            }

        }
        while (BN_cmp(r, range) >= 0);
    } else {
        do {
            /* range = 11..._2  or  range = 101..._2 */
            if (!bnrand(flag, r, n, BN_RAND_TOP_ANY, BN_RAND_BOTTOM_ANY,
                        strength, ctx))
                return 0;

            if (!--count) {
                ERR_raise(ERR_LIB_BN, BN_R_TOO_MANY_ITERATIONS);
                return 0;
            }
        }
        while (BN_cmp(r, range) >= 0);
    }

    bn_check_top(r);
    return 1;
}

int BN_rand_range_ex(BIGNUM *r, const BIGNUM *range, unsigned int strength,
                     BN_CTX *ctx)
{
    return bnrand_range(NORMAL, r, range, strength, ctx);
}

#ifndef FIPS_MODULE
int BN_rand_range(BIGNUM *r, const BIGNUM *range)
{
    return bnrand_range(NORMAL, r, range, 0, NULL);
}
#endif

int BN_priv_rand_range_ex(BIGNUM *r, const BIGNUM *range, unsigned int strength,
                          BN_CTX *ctx)
{
    return bnrand_range(PRIVATE, r, range, strength, ctx);
}

#ifndef FIPS_MODULE
int BN_priv_rand_range(BIGNUM *r, const BIGNUM *range)
{
    return bnrand_range(PRIVATE, r, range, 0, NULL);
}

# ifndef OPENSSL_NO_DEPRECATED_3_0
int BN_pseudo_rand(BIGNUM *rnd, int bits, int top, int bottom)
{
    return BN_rand(rnd, bits, top, bottom);
}

int BN_pseudo_rand_range(BIGNUM *r, const BIGNUM *range)
{
    return BN_rand_range(r, range);
}
# endif
#endif

int ossl_bn_priv_rand_range_fixed_top(BIGNUM *r, const BIGNUM *range,
                                      unsigned int strength, BN_CTX *ctx)
{
    int n;
    int count = 100;

    if (r == NULL) {
        ERR_raise(ERR_LIB_BN, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (range->neg || BN_is_zero(range)) {
        ERR_raise(ERR_LIB_BN, BN_R_INVALID_RANGE);
        return 0;
    }

    n = BN_num_bits(range);     /* n > 0 */

    /* BN_is_bit_set(range, n - 1) always holds */

    if (n == 1) {
        BN_zero(r);
    } else {
        BN_set_flags(r, BN_FLG_CONSTTIME);
        do {
            if (!bnrand(PRIVATE, r, n + 1, BN_RAND_TOP_ONE, BN_RAND_BOTTOM_ANY,
                        strength, ctx))
                return 0;

            if (!--count) {
                ERR_raise(ERR_LIB_BN, BN_R_TOO_MANY_ITERATIONS);
                return 0;
            }
            ossl_bn_mask_bits_fixed_top(r, n);
        }
        while (BN_ucmp(r, range) >= 0);
#ifdef BN_DEBUG
        /* With BN_DEBUG on a fixed top number cannot be returned */
        bn_correct_top(r);
#endif
    }

    return 1;
}

/*
 * ossl_bn_gen_dsa_nonce_fixed_top generates a random number 0 <= out < range.
 * Unlike BN_rand_range, it also includes the contents of |priv| and |message|
 * in the generation so that an RNG failure isn't fatal as long as |priv|
 * remains secret. This is intended for use in DSA and ECDSA where an RNG
 * weakness leads directly to private key exposure unless this function is
 * used.
 */
int ossl_bn_gen_dsa_nonce_fixed_top(BIGNUM *out, const BIGNUM *range,
                                    const BIGNUM *priv,
                                    const unsigned char *message,
                                    size_t message_len, BN_CTX *ctx)
{
    EVP_MD_CTX *mdctx = EVP_MD_CTX_new();
    /*
     * We use 512 bits of random data per iteration to ensure that we have at
     * least |range| bits of randomness.
     */
    unsigned char random_bytes[64];
    unsigned char digest[SHA512_DIGEST_LENGTH];
    unsigned done, todo;
    /* We generate |range|+1 bytes of random output. */
    const unsigned num_k_bytes = BN_num_bytes(range) + 1;
    unsigned char private_bytes[96];
    unsigned char *k_bytes = NULL;
    const int max_n = 64;           /* Pr(failure to generate) < 2^max_n */
    int n;
    int ret = 0;
    EVP_MD *md = NULL;
    OSSL_LIB_CTX *libctx = ossl_bn_get_libctx(ctx);

    if (mdctx == NULL)
        goto end;

    k_bytes = OPENSSL_malloc(num_k_bytes);
    if (k_bytes == NULL)
        goto end;
    /* Ensure top byte is set to avoid non-constant time in bin2bn */
    k_bytes[0] = 0xff;

    /* We copy |priv| into a local buffer to avoid exposing its length. */
    if (BN_bn2binpad(priv, private_bytes, sizeof(private_bytes)) < 0) {
        /*
         * No reasonable DSA or ECDSA key should have a private key this
         * large and we don't handle this case in order to avoid leaking the
         * length of the private key.
         */
        ERR_raise(ERR_LIB_BN, BN_R_PRIVATE_KEY_TOO_LARGE);
        goto end;
    }

    md = EVP_MD_fetch(libctx, "SHA512", NULL);
    if (md == NULL) {
        ERR_raise(ERR_LIB_BN, BN_R_NO_SUITABLE_DIGEST);
        goto end;
    }
    for (n = 0; n < max_n; n++) {
        unsigned char i = 0;

        for (done = 1; done < num_k_bytes;) {
            if (RAND_priv_bytes_ex(libctx, random_bytes, sizeof(random_bytes),
                                   0) <= 0)
                goto end;

            if (!EVP_DigestInit_ex(mdctx, md, NULL)
                    || !EVP_DigestUpdate(mdctx, &i, sizeof(i))
                    || !EVP_DigestUpdate(mdctx, private_bytes,
                                         sizeof(private_bytes))
                    || !EVP_DigestUpdate(mdctx, message, message_len)
                    || !EVP_DigestUpdate(mdctx, random_bytes,
                                         sizeof(random_bytes))
                    || !EVP_DigestFinal_ex(mdctx, digest, NULL))
                goto end;

            todo = num_k_bytes - done;
            if (todo > SHA512_DIGEST_LENGTH)
                todo = SHA512_DIGEST_LENGTH;
            memcpy(k_bytes + done, digest, todo);
            done += todo;
            ++i;
        }

        if (!BN_bin2bn(k_bytes, num_k_bytes, out))
            goto end;

        /* Clear out the top bits and rejection filter into range */
        BN_set_flags(out, BN_FLG_CONSTTIME);
        ossl_bn_mask_bits_fixed_top(out, BN_num_bits(range));

        if (BN_ucmp(out, range) < 0) {
            ret = 1;
#ifdef BN_DEBUG
            /* With BN_DEBUG on a fixed top number cannot be returned */
            bn_correct_top(out);
#endif
            goto end;
        }
    }
    /* Failed to generate anything */
    ERR_raise(ERR_LIB_BN, ERR_R_INTERNAL_ERROR);

 end:
    EVP_MD_CTX_free(mdctx);
    EVP_MD_free(md);
    OPENSSL_clear_free(k_bytes, num_k_bytes);
    OPENSSL_cleanse(digest, sizeof(digest));
    OPENSSL_cleanse(random_bytes, sizeof(random_bytes));
    OPENSSL_cleanse(private_bytes, sizeof(private_bytes));
    return ret;
}

int BN_generate_dsa_nonce(BIGNUM *out, const BIGNUM *range,
                          const BIGNUM *priv, const unsigned char *message,
                          size_t message_len, BN_CTX *ctx)
{
    int ret;

    ret = ossl_bn_gen_dsa_nonce_fixed_top(out, range, priv, message,
                                          message_len, ctx);
    /*
     * This call makes the BN_generate_dsa_nonce non-const-time, thus we
     * do not use it internally. But fixed_top BNs currently cannot be returned
     * from public API calls.
     */
    bn_correct_top(out);
    return ret;
}
