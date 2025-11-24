/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * For the prime check..
 * FIPS 186-4 Section C.3 Table C.1
 * Returns the minimum number of Miller Rabin iterations for a L,N pair
 * (where L = len(p), N = len(q))
 *   L   N                Min
 * 1024  160              40
 * 2048  224              56
 * 2048  256              56
 * 3072  256              64
 *
 * BN_check_prime() uses:
 *  64 iterations for L <= 2048 OR
 * 128 iterations for L > 2048
 * So this satisfies the requirement.
 */

#include <string.h> /* memset */
#include <openssl/sha.h> /* SHA_DIGEST_LENGTH */
#include <openssl/rand.h>
#include <openssl/err.h>
#include <openssl/dherr.h>
#include <openssl/dsaerr.h>
#include "crypto/bn.h"
#include "internal/ffc.h"

/*
 * Verify that the passed in L, N pair for DH or DSA is valid.
 * Returns 0 if invalid, otherwise it returns the security strength.
 */

#ifdef FIPS_MODULE
static int ffc_validate_LN(size_t L, size_t N, int type, int verify)
{
    if (type == FFC_PARAM_TYPE_DH) {
        /* Valid DH L,N parameters from SP800-56Ar3 5.5.1 Table 1 */
        if (L == 2048 && (N == 224 || N == 256))
            return 112;
# ifndef OPENSSL_NO_DH
        ERR_raise(ERR_LIB_DH, DH_R_BAD_FFC_PARAMETERS);
# endif
    } else if (type == FFC_PARAM_TYPE_DSA) {
        /* Valid DSA L,N parameters from FIPS 186-4 Section 4.2 */
        /* In fips mode 1024/160 can only be used for verification */
        if (verify && L == 1024 && N == 160)
            return 80;
        if (L == 2048 && (N == 224 || N == 256))
            return 112;
        if (L == 3072 && N == 256)
            return 128;
# ifndef OPENSSL_NO_DSA
        ERR_raise(ERR_LIB_DSA, DSA_R_BAD_FFC_PARAMETERS);
# endif
    }
    return 0;
}
#else
static int ffc_validate_LN(size_t L, size_t N, int type, int verify)
{
    if (type == FFC_PARAM_TYPE_DH) {
        /* Allow legacy 1024/160 in non fips mode */
        if (L == 1024 && N == 160)
            return 80;
        /* Valid DH L,N parameters from SP800-56Ar3 5.5.1 Table 1 */
        if (L == 2048 && (N == 224 || N == 256))
            return 112;
# ifndef OPENSSL_NO_DH
        ERR_raise(ERR_LIB_DH, DH_R_BAD_FFC_PARAMETERS);
# endif
    } else if (type == FFC_PARAM_TYPE_DSA) {
        if (L >= 3072 && N >= 256)
            return 128;
        if (L >= 2048 && N >= 224)
            return 112;
        if (L >= 1024 && N >= 160)
            return 80;
# ifndef OPENSSL_NO_DSA
        ERR_raise(ERR_LIB_DSA, DSA_R_BAD_FFC_PARAMETERS);
# endif
    }
    return 0;
}
#endif /* FIPS_MODULE */

/* FIPS186-4 A.2.1 Unverifiable Generation of Generator g */
static int generate_unverifiable_g(BN_CTX *ctx, BN_MONT_CTX *mont, BIGNUM *g,
                                   BIGNUM *hbn, const BIGNUM *p,
                                   const BIGNUM *e,const BIGNUM *pm1,
                                   int *hret)
{
    int h = 2;

    /* Step (2): choose h (where 1 < h)*/
    if (!BN_set_word(hbn, h))
        return 0;

    for (;;) {
        /* Step (3): g = h^e % p */
        if (!BN_mod_exp_mont(g, hbn, e, p, ctx, mont))
            return 0;
        /* Step (4): Finish if g > 1 */
        if (BN_cmp(g, BN_value_one()) > 0)
            break;

        /* Step (2) Choose any h in the range 1 < h < (p-1) */
        if (!BN_add_word(hbn, 1) || BN_cmp(hbn, pm1) >= 0)
            return 0;
        ++h;
    }
    *hret = h;
    return 1;
}

/*
 * FIPS186-4 A.2 Generation of canonical generator g.
 *
 * It requires the following values as input:
 *   'evpmd' digest, 'p' prime, 'e' cofactor, gindex and seed.
 * tmp is a passed in temporary BIGNUM.
 * mont is used in a BN_mod_exp_mont() with a modulus of p.
 * Returns a value in g.
 */
static int generate_canonical_g(BN_CTX *ctx, BN_MONT_CTX *mont,
                                const EVP_MD *evpmd, BIGNUM *g, BIGNUM *tmp,
                                const BIGNUM *p, const BIGNUM *e,
                                int gindex, unsigned char *seed, size_t seedlen)
{
    int ret = 0;
    int counter = 1;
    unsigned char md[EVP_MAX_MD_SIZE];
    EVP_MD_CTX *mctx = NULL;
    int mdsize;

    mdsize = EVP_MD_get_size(evpmd);
    if (mdsize <= 0)
        return 0;

    mctx = EVP_MD_CTX_new();
    if (mctx == NULL)
        return 0;

   /*
    * A.2.3 Step (4) & (5)
    * A.2.4 Step (6) & (7)
    * counter = 0; counter += 1
    */
    for (counter = 1; counter <= 0xFFFF; ++counter) {
        /*
         * A.2.3 Step (7) & (8) & (9)
         * A.2.4 Step (9) & (10) & (11)
         * W = Hash(seed || "ggen" || index || counter)
         * g = W^e % p
         */
        static const unsigned char ggen[4] = { 0x67, 0x67, 0x65, 0x6e };

        md[0] = (unsigned char)(gindex & 0xff);
        md[1] = (unsigned char)((counter >> 8) & 0xff);
        md[2] = (unsigned char)(counter & 0xff);
        if (!EVP_DigestInit_ex(mctx, evpmd, NULL)
                || !EVP_DigestUpdate(mctx, seed, seedlen)
                || !EVP_DigestUpdate(mctx, ggen, sizeof(ggen))
                || !EVP_DigestUpdate(mctx, md, 3)
                || !EVP_DigestFinal_ex(mctx, md, NULL)
                || (BN_bin2bn(md, mdsize, tmp) == NULL)
                || !BN_mod_exp_mont(g, tmp, e, p, ctx, mont))
                    break; /* exit on failure */
        /*
         * A.2.3 Step (10)
         * A.2.4 Step (12)
         * Found a value for g if (g >= 2)
         */
        if (BN_cmp(g, BN_value_one()) > 0) {
            ret = 1;
            break; /* found g */
        }
    }
    EVP_MD_CTX_free(mctx);
    return ret;
}

/* Generation of p is the same for FIPS 186-4 & FIPS 186-2 */
static int generate_p(BN_CTX *ctx, const EVP_MD *evpmd, int max_counter, int n,
                      unsigned char *buf, size_t buf_len, const BIGNUM *q,
                      BIGNUM *p, int L, BN_GENCB *cb, int *counter,
                      int *res)
{
    int ret = -1;
    int i, j, k, r;
    unsigned char md[EVP_MAX_MD_SIZE];
    int mdsize;
    BIGNUM *W, *X, *tmp, *c, *test;

    BN_CTX_start(ctx);
    W = BN_CTX_get(ctx);
    X = BN_CTX_get(ctx);
    c = BN_CTX_get(ctx);
    test = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL)
        goto err;

    if (!BN_lshift(test, BN_value_one(), L - 1))
        goto err;

    mdsize = EVP_MD_get_size(evpmd);
    if (mdsize <= 0)
        goto err;

    /* A.1.1.2 Step (10) AND
     * A.1.1.2 Step (12)
     * offset = 1 (this is handled below)
     */
    /*
     * A.1.1.2 Step (11) AND
     * A.1.1.3 Step (13)
     */
    for (i = 0; i <= max_counter; i++) {
        if ((i != 0) && !BN_GENCB_call(cb, 0, i))
            goto err;

        BN_zero(W);
        /* seed_tmp buffer contains "seed + offset - 1" */
        for (j = 0; j <= n; j++) {
            /* obtain "seed + offset + j" by incrementing by 1: */
            for (k = (int)buf_len - 1; k >= 0; k--) {
                buf[k]++;
                if (buf[k] != 0)
                    break;
            }
            /*
             * A.1.1.2 Step (11.1) AND
             * A.1.1.3 Step (13.1)
             * tmp = V(j) = Hash((seed + offset + j) % 2^seedlen)
             */
            if (!EVP_Digest(buf, buf_len, md, NULL, evpmd, NULL)
                    || (BN_bin2bn(md, mdsize, tmp) == NULL)
                    /*
                     * A.1.1.2 Step (11.2)
                     * A.1.1.3 Step (13.2)
                     * W += V(j) * 2^(outlen * j)
                     */
                    || !BN_lshift(tmp, tmp, (mdsize << 3) * j)
                    || !BN_add(W, W, tmp))
                goto err;
        }

        /*
         * A.1.1.2 Step (11.3) AND
         * A.1.1.3 Step (13.3)
         * X = W + 2^(L-1) where W < 2^(L-1)
         */
        if (!BN_mask_bits(W, L - 1)
                || !BN_copy(X, W)
                || !BN_add(X, X, test)
                /*
                 * A.1.1.2 Step (11.4) AND
                 * A.1.1.3 Step (13.4)
                 * c = X mod 2q
                 */
                || !BN_lshift1(tmp, q)
                || !BN_mod(c, X, tmp, ctx)
                /*
                 * A.1.1.2 Step (11.5) AND
                 * A.1.1.3 Step (13.5)
                 * p = X - (c - 1)
                 */
                || !BN_sub(tmp, c, BN_value_one())
                || !BN_sub(p, X, tmp))
            goto err;

        /*
         * A.1.1.2 Step (11.6) AND
         * A.1.1.3 Step (13.6)
         * if (p < 2 ^ (L-1)) continue
         * This makes sure the top bit is set.
         */
        if (BN_cmp(p, test) >= 0) {
            /*
             * A.1.1.2 Step (11.7) AND
             * A.1.1.3 Step (13.7)
             * Test if p is prime
             * (This also makes sure the bottom bit is set)
             */
            r = BN_check_prime(p, ctx, cb);
            /* A.1.1.2 Step (11.8) : Return if p is prime */
            if (r > 0) {
                *counter = i;
                ret = 1;   /* return success */
                goto err;
            }
            if (r != 0)
                goto err;
        }
        /* Step (11.9) : offset = offset + n + 1 is done auto-magically */
    }
    /* No prime P found */
    ret = 0;
    *res |= FFC_CHECK_P_NOT_PRIME;
err:
    BN_CTX_end(ctx);
    return ret;
}

static int generate_q_fips186_4(BN_CTX *ctx, BIGNUM *q, const EVP_MD *evpmd,
                                int qsize, unsigned char *seed, size_t seedlen,
                                int generate_seed, int *retm, int *res,
                                BN_GENCB *cb)
{
    int ret = 0, r;
    int m = *retm;
    unsigned char md[EVP_MAX_MD_SIZE];
    int mdsize = EVP_MD_get_size(evpmd);
    unsigned char *pmd;
    OSSL_LIB_CTX *libctx = ossl_bn_get_libctx(ctx);

    if (mdsize <= 0)
        goto err;

    /* find q */
    for (;;) {
        if (!BN_GENCB_call(cb, 0, m++))
            goto err;

        /* A.1.1.2 Step (5) : generate seed with size seed_len */
        if (generate_seed
                && RAND_bytes_ex(libctx, seed, seedlen, 0) <= 0)
            goto err;
        /*
         * A.1.1.2 Step (6) AND
         * A.1.1.3 Step (7)
         * U = Hash(seed) % (2^(N-1))
         */
        if (!EVP_Digest(seed, seedlen, md, NULL, evpmd, NULL))
            goto err;
        /* Take least significant bits of md */
        if (mdsize > qsize)
            pmd = md + mdsize - qsize;
        else
            pmd = md;
        if (mdsize < qsize)
            memset(md + mdsize, 0, qsize - mdsize);

        /*
         * A.1.1.2 Step (7) AND
         * A.1.1.3 Step (8)
         * q = U + 2^(N-1) + (1 - U %2) (This sets top and bottom bits)
         */
        pmd[0] |= 0x80;
        pmd[qsize-1] |= 0x01;
        if (!BN_bin2bn(pmd, qsize, q))
            goto err;

        /*
         * A.1.1.2 Step (8) AND
         * A.1.1.3 Step (9)
         * Test if q is prime
         */
        r = BN_check_prime(q, ctx, cb);
        if (r > 0) {
            ret = 1;
            goto err;
        }
        /*
         * A.1.1.3 Step (9) : If the provided seed didn't produce a prime q
         * return an error.
         */
        if (!generate_seed) {
            *res |= FFC_CHECK_Q_NOT_PRIME;
            goto err;
        }
        if (r != 0)
            goto err;
        /* A.1.1.2 Step (9) : if q is not prime, try another q */
    }
err:
    *retm = m;
    return ret;
}

static int generate_q_fips186_2(BN_CTX *ctx, BIGNUM *q, const EVP_MD *evpmd,
                                unsigned char *buf, unsigned char *seed,
                                size_t qsize, int generate_seed, int *retm,
                                int *res, BN_GENCB *cb)
{
    unsigned char buf2[EVP_MAX_MD_SIZE];
    unsigned char md[EVP_MAX_MD_SIZE];
    int i, r, ret = 0, m = *retm;
    OSSL_LIB_CTX *libctx = ossl_bn_get_libctx(ctx);

    /* find q */
    for (;;) {
        /* step 1 */
        if (!BN_GENCB_call(cb, 0, m++))
            goto err;

        if (generate_seed && RAND_bytes_ex(libctx, seed, qsize, 0) <= 0)
            goto err;

        memcpy(buf, seed, qsize);
        memcpy(buf2, seed, qsize);

        /* precompute "SEED + 1" for step 7: */
        for (i = (int)qsize - 1; i >= 0; i--) {
            buf[i]++;
            if (buf[i] != 0)
                break;
        }

        /* step 2 */
        if (!EVP_Digest(seed, qsize, md, NULL, evpmd, NULL))
            goto err;
        if (!EVP_Digest(buf, qsize, buf2, NULL, evpmd, NULL))
            goto err;
        for (i = 0; i < (int)qsize; i++)
            md[i] ^= buf2[i];

        /* step 3 */
        md[0] |= 0x80;
        md[qsize - 1] |= 0x01;
        if (!BN_bin2bn(md, (int)qsize, q))
            goto err;

        /* step 4 */
        r = BN_check_prime(q, ctx, cb);
        if (r > 0) {
            /* Found a prime */
            ret = 1;
            goto err;
        }
        if (r != 0)
            goto err; /* Exit if error */
        /* Try another iteration if it wasn't prime - was in old code.. */
        generate_seed = 1;
    }
err:
    *retm = m;
    return ret;
}

static const char *default_mdname(size_t N)
{
    if (N == 160)
        return "SHA1";
    else if (N == 224)
        return "SHA-224";
    else if (N == 256)
        return "SHA-256";
    return NULL;
}

/*
 * FIPS 186-4 FFC parameter generation (as defined in Appendix A).
 * The same code is used for validation (when validate_flags != 0)
 *
 * The primes p & q are generated/validated using:
 *   A.1.1.2 Generation of probable primes p & q using approved hash.
 *   A.1.1.3 Validation of generated probable primes
 *
 * Generator 'g' has 2 types in FIPS 186-4:
 *   (1) A.2.1 unverifiable generation of generator g.
 *       A.2.2 Assurance of the validity of unverifiable generator g.
 *   (2) A.2.3 Verifiable Canonical Generation of the generator g.
 *       A.2.4 Validation for Canonical Generation of the generator g.
 *
 * Notes:
 * (1) is only a partial validation of g, The validation of (2) requires
 * the seed and index used during generation as input.
 *
 * params: used to pass in values for generation and validation.
 * params->md: is the digest to use, If this value is NULL, then the digest is
 *   chosen using the value of N.
 * params->flags:
 *  For validation one of:
 *   -FFC_PARAM_FLAG_VALIDATE_PQ
 *   -FFC_PARAM_FLAG_VALIDATE_G
 *   -FFC_PARAM_FLAG_VALIDATE_PQG
 *  For generation of p & q:
 *   - This is skipped if p & q are passed in.
 *   - If the seed is passed in then generation of p & q uses this seed (and if
 *     this fails an error will occur).
 *   - Otherwise the seed is generated, and values of p & q are generated and
 *     the value of seed and counter are optionally returned.
 *  For the generation of g (after the generation of p, q):
 *   - If the seed has been generated or passed in and a valid gindex is passed
 *     in then canonical generation of g is used otherwise unverifiable
 *     generation of g is chosen.
 *  For validation of p & q:
 *   - p, q, and the seed and counter used for generation must be passed in.
 *  For validation of g:
 *   - For a partial validation : p, q and g are required.
 *   - For a canonical validation : the gindex and seed used for generation are
 *     also required.
 * mode: The mode - either FFC_PARAM_MODE_GENERATE or FFC_PARAM_MODE_VERIFY.
 * type: The key type - FFC_PARAM_TYPE_DSA or FFC_PARAM_TYPE_DH.
 * L: is the size of the prime p in bits (e.g 2048)
 * N: is the size of the prime q in bits (e.g 256)
 * res: A returned failure reason (One of FFC_CHECK_XXXX),
 *      or 0 for general failures.
 * cb: A callback (can be NULL) that is called during different phases
 *
 * Returns:
 *   - FFC_PARAM_RET_STATUS_FAILED: if there was an error, or validation failed.
 *   - FFC_PARAM_RET_STATUS_SUCCESS if the generation or validation succeeded.
 *   - FFC_PARAM_RET_STATUS_UNVERIFIABLE_G if the validation of G succeeded,
 *     but G is unverifiable.
 */
int ossl_ffc_params_FIPS186_4_gen_verify(OSSL_LIB_CTX *libctx,
                                         FFC_PARAMS *params, int mode, int type,
                                         size_t L, size_t N, int *res,
                                         BN_GENCB *cb)
{
    int ok = FFC_PARAM_RET_STATUS_FAILED;
    unsigned char *seed = NULL, *seed_tmp = NULL;
    int mdsize, counter = 0, pcounter = 0, r = 0;
    size_t seedlen = 0;
    BIGNUM *tmp, *pm1, *e, *test;
    BIGNUM *g = NULL, *q = NULL, *p = NULL;
    BN_MONT_CTX *mont = NULL;
    int n = 0, m = 0, qsize;
    int canonical_g = 0, hret = 0;
    BN_CTX *ctx = NULL;
    EVP_MD_CTX *mctx = NULL;
    EVP_MD *md = NULL;
    int verify = (mode == FFC_PARAM_MODE_VERIFY);
    unsigned int flags = verify ? params->flags : 0;
    const char *def_name;

    *res = 0;

    if (params->mdname != NULL) {
        md = EVP_MD_fetch(libctx, params->mdname, params->mdprops);
    } else {
        if (N == 0)
            N = (L >= 2048 ? SHA256_DIGEST_LENGTH : SHA_DIGEST_LENGTH) * 8;
        def_name = default_mdname(N);
        if (def_name == NULL) {
            *res = FFC_CHECK_INVALID_Q_VALUE;
            goto err;
        }
        md = EVP_MD_fetch(libctx, def_name, params->mdprops);
    }
    if (md == NULL)
        goto err;
    mdsize = EVP_MD_get_size(md);
    if (mdsize <= 0)
        goto err;

    if (N == 0)
        N = mdsize * 8;
    qsize = N >> 3;

    /*
     * A.1.1.2 Step (1) AND
     * A.1.1.3 Step (3)
     * Check that the L,N pair is an acceptable pair.
     */
    if (L <= N || !ffc_validate_LN(L, N, type, verify)) {
        *res = FFC_CHECK_BAD_LN_PAIR;
        goto err;
    }

    mctx = EVP_MD_CTX_new();
    if (mctx == NULL)
        goto err;

    if ((ctx = BN_CTX_new_ex(libctx)) == NULL)
        goto err;

    BN_CTX_start(ctx);
    g = BN_CTX_get(ctx);
    pm1 = BN_CTX_get(ctx);
    e = BN_CTX_get(ctx);
    test = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL)
        goto err;

    seedlen = params->seedlen;
    if (seedlen == 0)
        seedlen = (size_t)mdsize;
    /* If the seed was passed in - use this value as the seed */
    if (params->seed != NULL)
        seed = params->seed;

    if (!verify) {
        /* For generation: p & q must both be NULL or NON-NULL */
        if ((params->p == NULL) != (params->q == NULL)) {
            *res = FFC_CHECK_INVALID_PQ;
            goto err;
        }
    } else {
        /* Validation of p,q requires seed and counter to be valid */
        if ((flags & FFC_PARAM_FLAG_VALIDATE_PQ) != 0) {
            if (seed == NULL || params->pcounter < 0) {
                *res = FFC_CHECK_MISSING_SEED_OR_COUNTER;
                goto err;
            }
        }
        if ((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0) {
            /* validation of g also requires g to be set */
            if (params->g == NULL) {
                *res = FFC_CHECK_INVALID_G;
                goto err;
            }
        }
    }

    /*
     * If p & q are passed in and
     *   validate_flags = 0 then skip the generation of PQ.
     *   validate_flags = VALIDATE_G then also skip the validation of PQ.
     */
    if (params->p != NULL && ((flags & FFC_PARAM_FLAG_VALIDATE_PQ) == 0)) {
        /* p and q already exists so only generate g */
        p = params->p;
        q = params->q;
        goto g_only;
        /* otherwise fall through to validate p & q */
    }

    /* p & q will be used for generation and validation */
    p = BN_CTX_get(ctx);
    q = BN_CTX_get(ctx);
    if (q == NULL)
        goto err;

    /*
     * A.1.1.2 Step (2) AND
     * A.1.1.3 Step (6)
     * Return invalid if seedlen  < N
     */
    if ((seedlen * 8) < N) {
        *res = FFC_CHECK_INVALID_SEED_SIZE;
        goto err;
    }

    seed_tmp = OPENSSL_malloc(seedlen);
    if (seed_tmp == NULL)
        goto err;

    if (seed == NULL) {
        /* Validation requires the seed to be supplied */
        if (verify) {
            *res = FFC_CHECK_MISSING_SEED_OR_COUNTER;
            goto err;
        }
        /* if the seed is not supplied then alloc a seed buffer */
        seed = OPENSSL_malloc(seedlen);
        if (seed == NULL)
            goto err;
    }

    /* A.1.1.2 Step (11): max loop count = 4L - 1 */
    counter = 4 * L - 1;
    /* Validation requires the counter to be supplied */
    if (verify) {
        /* A.1.1.3 Step (4) : if (counter > (4L -1)) return INVALID */
        if (params->pcounter > counter) {
            *res = FFC_CHECK_INVALID_COUNTER;
            goto err;
        }
        counter = params->pcounter;
    }

    /*
     * A.1.1.2 Step (3) AND
     * A.1.1.3 Step (10)
     * n = floor(L / hash_outlen) - 1
     */
    n = (L - 1) / (mdsize << 3);

    /* Calculate 2^(L-1): Used in step A.1.1.2 Step (11.3) */
    if (!BN_lshift(test, BN_value_one(), L - 1))
        goto err;

    for (;;) {
        if (!generate_q_fips186_4(ctx, q, md, qsize, seed, seedlen,
                                  seed != params->seed, &m, res, cb))
            goto err;
        /* A.1.1.3 Step (9): Verify that q matches the expected value */
        if (verify && (BN_cmp(q, params->q) != 0)) {
            *res = FFC_CHECK_Q_MISMATCH;
            goto err;
        }
        if (!BN_GENCB_call(cb, 2, 0))
            goto err;
        if (!BN_GENCB_call(cb, 3, 0))
            goto err;

        memcpy(seed_tmp, seed, seedlen);
        r = generate_p(ctx, md, counter, n, seed_tmp, seedlen, q, p, L,
                       cb, &pcounter, res);
        if (r > 0)
            break; /* found p */
        if (r < 0)
            goto err;
        /*
         * A.1.1.3 Step (14):
         * If we get here we failed to get a p for the given seed. If the
         * seed is not random then it needs to fail (as it will always fail).
         */
        if (seed == params->seed) {
            *res = FFC_CHECK_P_NOT_PRIME;
            goto err;
        }
    }
    if(!BN_GENCB_call(cb, 2, 1))
        goto err;
    /*
     * Gets here if we found p.
     * A.1.1.3 Step (14): return error if i != counter OR computed_p != known_p.
     */
    if (verify && (pcounter != counter || (BN_cmp(p, params->p) != 0)))
        goto err;

    /* If validating p & q only then skip the g validation test */
    if ((flags & FFC_PARAM_FLAG_VALIDATE_PQG) == FFC_PARAM_FLAG_VALIDATE_PQ)
        goto pass;
g_only:
    if ((mont = BN_MONT_CTX_new()) == NULL)
        goto err;
    if (!BN_MONT_CTX_set(mont, p, ctx))
        goto err;

    if (((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0)
        && !ossl_ffc_params_validate_unverifiable_g(ctx, mont, p, q, params->g,
                                                    tmp, res))
        goto err;

    /*
     * A.2.1 Step (1) AND
     * A.2.3 Step (3) AND
     * A.2.4 Step (5)
     * e = (p - 1) / q (i.e- Cofactor 'e' is given by p = q * e + 1)
     */
    if (!(BN_sub(pm1, p, BN_value_one()) && BN_div(e, NULL, pm1, q, ctx)))
        goto err;

    /* Canonical g requires a seed and index to be set */
    if ((seed != NULL) && (params->gindex != FFC_UNVERIFIABLE_GINDEX)) {
        canonical_g = 1;
        if (!generate_canonical_g(ctx, mont, md, g, tmp, p, e,
                                  params->gindex, seed, seedlen)) {
            *res = FFC_CHECK_INVALID_G;
            goto err;
        }
        /* A.2.4 Step (13): Return valid if computed_g == g */
        if (verify && BN_cmp(g, params->g) != 0) {
            *res = FFC_CHECK_G_MISMATCH;
            goto err;
        }
    } else if (!verify) {
        if (!generate_unverifiable_g(ctx, mont, g, tmp, p, e, pm1, &hret))
            goto err;
    }

    if (!BN_GENCB_call(cb, 3, 1))
        goto err;

    if (!verify) {
        if (p != params->p) {
            BN_free(params->p);
            params->p = BN_dup(p);
        }
        if (q != params->q) {
            BN_free(params->q);
            params->q = BN_dup(q);
        }
        if (g != params->g) {
            BN_free(params->g);
            params->g = BN_dup(g);
        }
        if (params->p == NULL || params->q == NULL || params->g == NULL)
            goto err;
        if (!ossl_ffc_params_set_validate_params(params, seed, seedlen,
                                                 pcounter))
            goto err;
        params->h = hret;
    }
pass:
    if ((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0 && (canonical_g == 0))
        /* Return for the case where g is partially valid */
        ok = FFC_PARAM_RET_STATUS_UNVERIFIABLE_G;
    else
        ok = FFC_PARAM_RET_STATUS_SUCCESS;
err:
    if (seed != params->seed)
        OPENSSL_free(seed);
    OPENSSL_free(seed_tmp);
    if (ctx != NULL)
        BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    BN_MONT_CTX_free(mont);
    EVP_MD_CTX_free(mctx);
    EVP_MD_free(md);
    return ok;
}

/* Note this function is only used for verification in fips mode */
int ossl_ffc_params_FIPS186_2_gen_verify(OSSL_LIB_CTX *libctx,
                                         FFC_PARAMS *params, int mode, int type,
                                         size_t L, size_t N, int *res,
                                         BN_GENCB *cb)
{
    int ok = FFC_PARAM_RET_STATUS_FAILED;
    unsigned char seed[SHA256_DIGEST_LENGTH];
    unsigned char buf[SHA256_DIGEST_LENGTH];
    BIGNUM *r0, *test, *tmp, *g = NULL, *q = NULL, *p = NULL;
    BN_MONT_CTX *mont = NULL;
    EVP_MD *md = NULL;
    int md_size;
    size_t qsize;
    int n = 0, m = 0;
    int counter = 0, pcounter = 0, use_random_seed;
    int rv;
    BN_CTX *ctx = NULL;
    int hret = -1;
    unsigned char *seed_in = params->seed;
    size_t seed_len = params->seedlen;
    int verify = (mode == FFC_PARAM_MODE_VERIFY);
    unsigned int flags = verify ? params->flags : 0;
    const char *def_name;

    *res = 0;

    if (params->mdname != NULL) {
        md = EVP_MD_fetch(libctx, params->mdname, params->mdprops);
    } else {
        if (N == 0)
            N = (L >= 2048 ? SHA256_DIGEST_LENGTH : SHA_DIGEST_LENGTH) * 8;
        def_name = default_mdname(N);
        if (def_name == NULL) {
            *res = FFC_CHECK_INVALID_Q_VALUE;
            goto err;
        }
        md = EVP_MD_fetch(libctx, def_name, params->mdprops);
    }
    if (md == NULL)
        goto err;
    md_size = EVP_MD_get_size(md);
    if (md_size <= 0)
        goto err;
    if (N == 0)
        N = md_size * 8;
    qsize = N >> 3;

    /*
     * The original spec allowed L = 512 + 64*j (j = 0.. 8)
     * https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-131Ar2.pdf
     * says that 512 can be used for legacy verification.
     */
    if (L < 512) {
        *res = FFC_CHECK_BAD_LN_PAIR;
        goto err;
    }
    if (qsize != SHA_DIGEST_LENGTH
        && qsize != SHA224_DIGEST_LENGTH
        && qsize != SHA256_DIGEST_LENGTH) {
        /* invalid q size */
        *res = FFC_CHECK_INVALID_Q_VALUE;
        goto err;
    }

    L = (L + 63) / 64 * 64;

    if (seed_in != NULL) {
        if (seed_len < qsize) {
            *res = FFC_CHECK_INVALID_SEED_SIZE;
            goto err;
        }
        /* Only consume as much seed as is expected. */
        if (seed_len > qsize)
            seed_len = qsize;
        memcpy(seed, seed_in, seed_len);
    }

    ctx = BN_CTX_new_ex(libctx);
    if (ctx == NULL)
        goto err;

    BN_CTX_start(ctx);

    r0 = BN_CTX_get(ctx);
    g = BN_CTX_get(ctx);
    q = BN_CTX_get(ctx);
    p = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    test = BN_CTX_get(ctx);
    if (test == NULL)
        goto err;

    if (!BN_lshift(test, BN_value_one(), L - 1))
        goto err;

    if (!verify) {
        /* For generation: p & q must both be NULL or NON-NULL */
        if ((params->p != NULL) != (params->q != NULL)) {
            *res = FFC_CHECK_INVALID_PQ;
            goto err;
        }
    } else {
        if ((flags & FFC_PARAM_FLAG_VALIDATE_PQ) != 0) {
            /* Validation of p,q requires seed and counter to be valid */
            if (seed_in == NULL || params->pcounter < 0) {
                *res = FFC_CHECK_MISSING_SEED_OR_COUNTER;
                goto err;
            }
        }
        if ((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0) {
            /* validation of g also requires g to be set */
            if (params->g == NULL) {
                *res = FFC_CHECK_INVALID_G;
                goto err;
            }
        }
    }

    if (params->p != NULL && ((flags & FFC_PARAM_FLAG_VALIDATE_PQ) == 0)) {
        /* p and q already exists so only generate g */
        p = params->p;
        q = params->q;
        goto g_only;
        /* otherwise fall through to validate p and q */
    }

    use_random_seed = (seed_in == NULL);
    for (;;) {
        if (!generate_q_fips186_2(ctx, q, md, buf, seed, qsize,
                                  use_random_seed, &m, res, cb))
            goto err;

        if (!BN_GENCB_call(cb, 2, 0))
            goto err;
        if (!BN_GENCB_call(cb, 3, 0))
            goto err;

        /* step 6 */
        n = (L - 1) / 160;
        counter = 4 * L - 1; /* Was 4096 */
        /* Validation requires the counter to be supplied */
        if (verify) {
            if (params->pcounter > counter) {
                *res = FFC_CHECK_INVALID_COUNTER;
                goto err;
            }
            counter = params->pcounter;
        }

        rv = generate_p(ctx, md, counter, n, buf, qsize, q, p, L, cb,
                        &pcounter, res);
        if (rv > 0)
            break; /* found it */
        if (rv == -1)
            goto err;
        /* This is what the old code did - probably not a good idea! */
        use_random_seed = 1;
    }

    if (!BN_GENCB_call(cb, 2, 1))
        goto err;

    if (verify) {
        if (pcounter != counter) {
            *res = FFC_CHECK_COUNTER_MISMATCH;
            goto err;
        }
        if (BN_cmp(p, params->p) != 0) {
            *res = FFC_CHECK_P_MISMATCH;
            goto err;
        }
    }
    /* If validating p & q only then skip the g validation test */
    if ((flags & FFC_PARAM_FLAG_VALIDATE_PQG) == FFC_PARAM_FLAG_VALIDATE_PQ)
        goto pass;
g_only:
    if ((mont = BN_MONT_CTX_new()) == NULL)
        goto err;
    if (!BN_MONT_CTX_set(mont, p, ctx))
        goto err;

    if (!verify) {
        /* We now need to generate g */
        /* set test = p - 1 */
        if (!BN_sub(test, p, BN_value_one()))
            goto err;
        /* Set r0 = (p - 1) / q */
        if (!BN_div(r0, NULL, test, q, ctx))
            goto err;
        if (!generate_unverifiable_g(ctx, mont, g, tmp, p, r0, test, &hret))
            goto err;
    } else if (((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0)
               && !ossl_ffc_params_validate_unverifiable_g(ctx, mont, p, q,
                                                           params->g, tmp,
                                                           res)) {
        goto err;
    }

    if (!BN_GENCB_call(cb, 3, 1))
        goto err;

    if (!verify) {
        if (p != params->p) {
            BN_free(params->p);
            params->p = BN_dup(p);
        }
        if (q != params->q) {
            BN_free(params->q);
            params->q = BN_dup(q);
        }
        if (g != params->g) {
            BN_free(params->g);
            params->g = BN_dup(g);
        }
        if (params->p == NULL || params->q == NULL || params->g == NULL)
            goto err;
        if (!ossl_ffc_params_set_validate_params(params, seed, qsize, pcounter))
            goto err;
        params->h = hret;
    }
pass:
    if ((flags & FFC_PARAM_FLAG_VALIDATE_G) != 0)
        ok = FFC_PARAM_RET_STATUS_UNVERIFIABLE_G;
    else
        ok = FFC_PARAM_RET_STATUS_SUCCESS;
err:
    if (ctx != NULL)
        BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    BN_MONT_CTX_free(mont);
    EVP_MD_free(md);
    return ok;
}

int ossl_ffc_params_FIPS186_4_generate(OSSL_LIB_CTX *libctx, FFC_PARAMS *params,
                                       int type, size_t L, size_t N,
                                       int *res, BN_GENCB *cb)
{
    return ossl_ffc_params_FIPS186_4_gen_verify(libctx, params,
                                                FFC_PARAM_MODE_GENERATE,
                                                type, L, N, res, cb);
}

/* This should no longer be used in FIPS mode */
int ossl_ffc_params_FIPS186_2_generate(OSSL_LIB_CTX *libctx, FFC_PARAMS *params,
                                       int type, size_t L, size_t N,
                                       int *res, BN_GENCB *cb)
{
    if (!ossl_ffc_params_FIPS186_2_gen_verify(libctx, params,
                                              FFC_PARAM_MODE_GENERATE,
                                              type, L, N, res, cb))
        return 0;

    ossl_ffc_params_enable_flags(params, FFC_PARAM_FLAG_VALIDATE_LEGACY, 1);
    return 1;
}
