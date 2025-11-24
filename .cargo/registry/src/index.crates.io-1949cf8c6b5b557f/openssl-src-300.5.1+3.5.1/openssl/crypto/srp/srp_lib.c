/*
 * Copyright 2004-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2004, EdelKey Project. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Christophe Renou and Peter Sylvester,
 * for the EdelKey project.
 */

/* All the SRP APIs in this file are deprecated */
#define OPENSSL_SUPPRESS_DEPRECATED

#ifndef OPENSSL_NO_SRP
# include "internal/cryptlib.h"
# include <openssl/sha.h>
# include <openssl/srp.h>
# include <openssl/evp.h>
# include "crypto/bn_srp.h"

/* calculate = SHA1(PAD(x) || PAD(y)) */

static BIGNUM *srp_Calc_xy(const BIGNUM *x, const BIGNUM *y, const BIGNUM *N,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    unsigned char digest[SHA_DIGEST_LENGTH];
    unsigned char *tmp = NULL;
    int numN = BN_num_bytes(N);
    BIGNUM *res = NULL;
    EVP_MD *sha1 = EVP_MD_fetch(libctx, "SHA1", propq);

    if (sha1 == NULL)
        return NULL;

    if (x != N && BN_ucmp(x, N) >= 0)
        goto err;
    if (y != N && BN_ucmp(y, N) >= 0)
        goto err;
    if ((tmp = OPENSSL_malloc(numN * 2)) == NULL)
        goto err;
    if (BN_bn2binpad(x, tmp, numN) < 0
        || BN_bn2binpad(y, tmp + numN, numN) < 0
        || !EVP_Digest(tmp, numN * 2, digest, NULL, sha1, NULL))
        goto err;
    res = BN_bin2bn(digest, sizeof(digest), NULL);
 err:
    EVP_MD_free(sha1);
    OPENSSL_free(tmp);
    return res;
}

static BIGNUM *srp_Calc_k(const BIGNUM *N, const BIGNUM *g,
                          OSSL_LIB_CTX *libctx,
                          const char *propq)
{
    /* k = SHA1(N | PAD(g)) -- tls-srp RFC 5054 */
    return srp_Calc_xy(N, g, N, libctx, propq);
}

BIGNUM *SRP_Calc_u_ex(const BIGNUM *A, const BIGNUM *B, const BIGNUM *N,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    /* u = SHA1(PAD(A) || PAD(B) ) -- tls-srp RFC 5054 */
    return srp_Calc_xy(A, B, N, libctx, propq);
}

BIGNUM *SRP_Calc_u(const BIGNUM *A, const BIGNUM *B, const BIGNUM *N)
{
    /* u = SHA1(PAD(A) || PAD(B) ) -- tls-srp RFC 5054 */
    return srp_Calc_xy(A, B, N, NULL, NULL);
}

BIGNUM *SRP_Calc_server_key(const BIGNUM *A, const BIGNUM *v, const BIGNUM *u,
                            const BIGNUM *b, const BIGNUM *N)
{
    BIGNUM *tmp = NULL, *S = NULL;
    BN_CTX *bn_ctx;

    if (u == NULL || A == NULL || v == NULL || b == NULL || N == NULL)
        return NULL;

    if ((bn_ctx = BN_CTX_new()) == NULL || (tmp = BN_new()) == NULL)
        goto err;

    /* S = (A*v**u) ** b */

    if (!BN_mod_exp(tmp, v, u, N, bn_ctx))
        goto err;
    if (!BN_mod_mul(tmp, A, tmp, N, bn_ctx))
        goto err;

    S = BN_new();
    if (S != NULL && !BN_mod_exp(S, tmp, b, N, bn_ctx)) {
        BN_free(S);
        S = NULL;
    }
 err:
    BN_CTX_free(bn_ctx);
    BN_clear_free(tmp);
    return S;
}

BIGNUM *SRP_Calc_B_ex(const BIGNUM *b, const BIGNUM *N, const BIGNUM *g,
                      const BIGNUM *v, OSSL_LIB_CTX *libctx, const char *propq)
{
    BIGNUM *kv = NULL, *gb = NULL;
    BIGNUM *B = NULL, *k = NULL;
    BN_CTX *bn_ctx;

    if (b == NULL || N == NULL || g == NULL || v == NULL ||
        (bn_ctx = BN_CTX_new_ex(libctx)) == NULL)
        return NULL;

    if ((kv = BN_new()) == NULL ||
        (gb = BN_new()) == NULL || (B = BN_new()) == NULL)
        goto err;

    /* B = g**b + k*v */

    if (!BN_mod_exp(gb, g, b, N, bn_ctx)
        || (k = srp_Calc_k(N, g, libctx, propq)) == NULL
        || !BN_mod_mul(kv, v, k, N, bn_ctx)
        || !BN_mod_add(B, gb, kv, N, bn_ctx)) {
        BN_free(B);
        B = NULL;
    }
 err:
    BN_CTX_free(bn_ctx);
    BN_clear_free(kv);
    BN_clear_free(gb);
    BN_free(k);
    return B;
}

BIGNUM *SRP_Calc_B(const BIGNUM *b, const BIGNUM *N, const BIGNUM *g,
                   const BIGNUM *v)
{
    return SRP_Calc_B_ex(b, N, g, v, NULL, NULL);
}

BIGNUM *SRP_Calc_x_ex(const BIGNUM *s, const char *user, const char *pass,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    unsigned char dig[SHA_DIGEST_LENGTH];
    EVP_MD_CTX *ctxt;
    unsigned char *cs = NULL;
    BIGNUM *res = NULL;
    EVP_MD *sha1 = NULL;

    if ((s == NULL) || (user == NULL) || (pass == NULL))
        return NULL;

    ctxt = EVP_MD_CTX_new();
    if (ctxt == NULL)
        return NULL;
    if ((cs = OPENSSL_malloc(BN_num_bytes(s))) == NULL)
        goto err;

    sha1 = EVP_MD_fetch(libctx, "SHA1", propq);
    if (sha1 == NULL)
        goto err;

    if (!EVP_DigestInit_ex(ctxt, sha1, NULL)
        || !EVP_DigestUpdate(ctxt, user, strlen(user))
        || !EVP_DigestUpdate(ctxt, ":", 1)
        || !EVP_DigestUpdate(ctxt, pass, strlen(pass))
        || !EVP_DigestFinal_ex(ctxt, dig, NULL)
        || !EVP_DigestInit_ex(ctxt, sha1, NULL))
        goto err;
    if (BN_bn2bin(s, cs) < 0)
        goto err;
    if (!EVP_DigestUpdate(ctxt, cs, BN_num_bytes(s)))
        goto err;

    if (!EVP_DigestUpdate(ctxt, dig, sizeof(dig))
        || !EVP_DigestFinal_ex(ctxt, dig, NULL))
        goto err;

    res = BN_bin2bn(dig, sizeof(dig), NULL);

 err:
    EVP_MD_free(sha1);
    OPENSSL_free(cs);
    EVP_MD_CTX_free(ctxt);
    return res;
}

BIGNUM *SRP_Calc_x(const BIGNUM *s, const char *user, const char *pass)
{
    return SRP_Calc_x_ex(s, user, pass, NULL, NULL);
}

BIGNUM *SRP_Calc_A(const BIGNUM *a, const BIGNUM *N, const BIGNUM *g)
{
    BN_CTX *bn_ctx;
    BIGNUM *A = NULL;

    if (a == NULL || N == NULL || g == NULL || (bn_ctx = BN_CTX_new()) == NULL)
        return NULL;

    if ((A = BN_new()) != NULL && !BN_mod_exp(A, g, a, N, bn_ctx)) {
        BN_free(A);
        A = NULL;
    }
    BN_CTX_free(bn_ctx);
    return A;
}

BIGNUM *SRP_Calc_client_key_ex(const BIGNUM *N, const BIGNUM *B, const BIGNUM *g,
                            const BIGNUM *x, const BIGNUM *a, const BIGNUM *u,
                            OSSL_LIB_CTX *libctx, const char *propq)
{
    BIGNUM *tmp = NULL, *tmp2 = NULL, *tmp3 = NULL, *k = NULL, *K = NULL;
    BIGNUM *xtmp = NULL;
    BN_CTX *bn_ctx;

    if (u == NULL || B == NULL || N == NULL || g == NULL || x == NULL
        || a == NULL || (bn_ctx = BN_CTX_new_ex(libctx)) == NULL)
        return NULL;

    if ((tmp = BN_new()) == NULL ||
        (tmp2 = BN_new()) == NULL ||
        (tmp3 = BN_new()) == NULL ||
        (xtmp = BN_new()) == NULL)
        goto err;

    BN_with_flags(xtmp, x, BN_FLG_CONSTTIME);
    BN_set_flags(tmp, BN_FLG_CONSTTIME);
    if (!BN_mod_exp(tmp, g, xtmp, N, bn_ctx))
        goto err;
    if ((k = srp_Calc_k(N, g, libctx, propq)) == NULL)
        goto err;
    if (!BN_mod_mul(tmp2, tmp, k, N, bn_ctx))
        goto err;
    if (!BN_mod_sub(tmp, B, tmp2, N, bn_ctx))
        goto err;
    if (!BN_mul(tmp3, u, xtmp, bn_ctx))
        goto err;
    if (!BN_add(tmp2, a, tmp3))
        goto err;
    K = BN_new();
    if (K != NULL && !BN_mod_exp(K, tmp, tmp2, N, bn_ctx)) {
        BN_free(K);
        K = NULL;
    }

 err:
    BN_CTX_free(bn_ctx);
    BN_free(xtmp);
    BN_clear_free(tmp);
    BN_clear_free(tmp2);
    BN_clear_free(tmp3);
    BN_free(k);
    return K;
}

BIGNUM *SRP_Calc_client_key(const BIGNUM *N, const BIGNUM *B, const BIGNUM *g,
                            const BIGNUM *x, const BIGNUM *a, const BIGNUM *u)
{
    return SRP_Calc_client_key_ex(N, B, g, x, a, u, NULL, NULL);
}

int SRP_Verify_B_mod_N(const BIGNUM *B, const BIGNUM *N)
{
    BIGNUM *r;
    BN_CTX *bn_ctx;
    int ret = 0;

    if (B == NULL || N == NULL || (bn_ctx = BN_CTX_new()) == NULL)
        return 0;

    if ((r = BN_new()) == NULL)
        goto err;
    /* Checks if B % N == 0 */
    if (!BN_nnmod(r, B, N, bn_ctx))
        goto err;
    ret = !BN_is_zero(r);
 err:
    BN_CTX_free(bn_ctx);
    BN_free(r);
    return ret;
}

int SRP_Verify_A_mod_N(const BIGNUM *A, const BIGNUM *N)
{
    /* Checks if A % N == 0 */
    return SRP_Verify_B_mod_N(A, N);
}

static SRP_gN knowngN[] = {
    {"8192", &ossl_bn_generator_19, &ossl_bn_group_8192},
    {"6144", &ossl_bn_generator_5, &ossl_bn_group_6144},
    {"4096", &ossl_bn_generator_5, &ossl_bn_group_4096},
    {"3072", &ossl_bn_generator_5, &ossl_bn_group_3072},
    {"2048", &ossl_bn_generator_2, &ossl_bn_group_2048},
    {"1536", &ossl_bn_generator_2, &ossl_bn_group_1536},
    {"1024", &ossl_bn_generator_2, &ossl_bn_group_1024},
};

# define KNOWN_GN_NUMBER sizeof(knowngN) / sizeof(SRP_gN)

/*
 * Check if G and N are known parameters. The values have been generated
 * from the IETF RFC 5054
 */
char *SRP_check_known_gN_param(const BIGNUM *g, const BIGNUM *N)
{
    size_t i;
    if ((g == NULL) || (N == NULL))
        return NULL;

    for (i = 0; i < KNOWN_GN_NUMBER; i++) {
        if (BN_cmp(knowngN[i].g, g) == 0 && BN_cmp(knowngN[i].N, N) == 0)
            return knowngN[i].id;
    }
    return NULL;
}

SRP_gN *SRP_get_default_gN(const char *id)
{
    size_t i;

    if (id == NULL)
        return knowngN;
    for (i = 0; i < KNOWN_GN_NUMBER; i++) {
        if (strcmp(knowngN[i].id, id) == 0)
            return knowngN + i;
    }
    return NULL;
}
#endif
