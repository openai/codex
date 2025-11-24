/*
 * Copyright 2004-2025 The OpenSSL Project Authors. All Rights Reserved.
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

/*
 * We need to use the SRP deprecated APIs in order to implement the SSL SRP
 * APIs - which are themselves deprecated.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/crypto.h>
#include <openssl/rand.h>
#include <openssl/err.h>
#include "ssl_local.h"
#include "internal/ssl_unwrap.h"

#ifndef OPENSSL_NO_SRP
# include <openssl/srp.h>

/*
 * The public API SSL_CTX_SRP_CTX_free() is deprecated so we use
 * ssl_ctx_srp_ctx_free_intern() internally.
 */
int ssl_ctx_srp_ctx_free_intern(SSL_CTX *ctx)
{
    if (ctx == NULL)
        return 0;
    OPENSSL_free(ctx->srp_ctx.login);
    OPENSSL_free(ctx->srp_ctx.info);
    BN_free(ctx->srp_ctx.N);
    BN_free(ctx->srp_ctx.g);
    BN_free(ctx->srp_ctx.s);
    BN_free(ctx->srp_ctx.B);
    BN_free(ctx->srp_ctx.A);
    BN_free(ctx->srp_ctx.a);
    BN_free(ctx->srp_ctx.b);
    BN_free(ctx->srp_ctx.v);
    memset(&ctx->srp_ctx, 0, sizeof(ctx->srp_ctx));
    ctx->srp_ctx.strength = SRP_MINIMAL_N;
    return 1;
}

int SSL_CTX_SRP_CTX_free(SSL_CTX *ctx)
{
    return ssl_ctx_srp_ctx_free_intern(ctx);
}

/*
 * The public API SSL_SRP_CTX_free() is deprecated so we use
 * ssl_srp_ctx_free_intern() internally.
 */
int ssl_srp_ctx_free_intern(SSL_CONNECTION *s)
{
    if (s == NULL)
        return 0;
    OPENSSL_free(s->srp_ctx.login);
    OPENSSL_free(s->srp_ctx.info);
    BN_free(s->srp_ctx.N);
    BN_free(s->srp_ctx.g);
    BN_free(s->srp_ctx.s);
    BN_free(s->srp_ctx.B);
    BN_free(s->srp_ctx.A);
    BN_free(s->srp_ctx.a);
    BN_free(s->srp_ctx.b);
    BN_free(s->srp_ctx.v);
    memset(&s->srp_ctx, 0, sizeof(s->srp_ctx));
    s->srp_ctx.strength = SRP_MINIMAL_N;
    return 1;
}

int SSL_SRP_CTX_free(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    /* the call works with NULL sc */
    return ssl_srp_ctx_free_intern(sc);
}

/*
 * The public API SSL_SRP_CTX_init() is deprecated so we use
 * ssl_srp_ctx_init_intern() internally.
 */
int ssl_srp_ctx_init_intern(SSL_CONNECTION *s)
{
    SSL_CTX *ctx;

    if (s == NULL || (ctx = SSL_CONNECTION_GET_CTX(s)) == NULL)
        return 0;

    memset(&s->srp_ctx, 0, sizeof(s->srp_ctx));

    s->srp_ctx.SRP_cb_arg = ctx->srp_ctx.SRP_cb_arg;
    /* set client Hello login callback */
    s->srp_ctx.TLS_ext_srp_username_callback =
        ctx->srp_ctx.TLS_ext_srp_username_callback;
    /* set SRP N/g param callback for verification */
    s->srp_ctx.SRP_verify_param_callback =
        ctx->srp_ctx.SRP_verify_param_callback;
    /* set SRP client passwd callback */
    s->srp_ctx.SRP_give_srp_client_pwd_callback =
        ctx->srp_ctx.SRP_give_srp_client_pwd_callback;

    s->srp_ctx.strength = ctx->srp_ctx.strength;

    if (((ctx->srp_ctx.N != NULL) &&
         ((s->srp_ctx.N = BN_dup(ctx->srp_ctx.N)) == NULL)) ||
        ((ctx->srp_ctx.g != NULL) &&
         ((s->srp_ctx.g = BN_dup(ctx->srp_ctx.g)) == NULL)) ||
        ((ctx->srp_ctx.s != NULL) &&
         ((s->srp_ctx.s = BN_dup(ctx->srp_ctx.s)) == NULL)) ||
        ((ctx->srp_ctx.B != NULL) &&
         ((s->srp_ctx.B = BN_dup(ctx->srp_ctx.B)) == NULL)) ||
        ((ctx->srp_ctx.A != NULL) &&
         ((s->srp_ctx.A = BN_dup(ctx->srp_ctx.A)) == NULL)) ||
        ((ctx->srp_ctx.a != NULL) &&
         ((s->srp_ctx.a = BN_dup(ctx->srp_ctx.a)) == NULL)) ||
        ((ctx->srp_ctx.v != NULL) &&
         ((s->srp_ctx.v = BN_dup(ctx->srp_ctx.v)) == NULL)) ||
        ((ctx->srp_ctx.b != NULL) &&
         ((s->srp_ctx.b = BN_dup(ctx->srp_ctx.b)) == NULL))) {
        ERR_raise(ERR_LIB_SSL, ERR_R_BN_LIB);
        goto err;
    }
    if ((ctx->srp_ctx.login != NULL) &&
        ((s->srp_ctx.login = OPENSSL_strdup(ctx->srp_ctx.login)) == NULL)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if ((ctx->srp_ctx.info != NULL) &&
        ((s->srp_ctx.info = OPENSSL_strdup(ctx->srp_ctx.info)) == NULL)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    s->srp_ctx.srp_Mask = ctx->srp_ctx.srp_Mask;

    return 1;
 err:
    OPENSSL_free(s->srp_ctx.login);
    OPENSSL_free(s->srp_ctx.info);
    BN_free(s->srp_ctx.N);
    BN_free(s->srp_ctx.g);
    BN_free(s->srp_ctx.s);
    BN_free(s->srp_ctx.B);
    BN_free(s->srp_ctx.A);
    BN_free(s->srp_ctx.a);
    BN_free(s->srp_ctx.b);
    BN_free(s->srp_ctx.v);
    memset(&s->srp_ctx, 0, sizeof(s->srp_ctx));
    return 0;
}

int SSL_SRP_CTX_init(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    /* the call works with NULL sc */
    return ssl_srp_ctx_init_intern(sc);
}

/*
 * The public API SSL_CTX_SRP_CTX_init() is deprecated so we use
 * ssl_ctx_srp_ctx_init_intern() internally.
 */
int ssl_ctx_srp_ctx_init_intern(SSL_CTX *ctx)
{
    if (ctx == NULL)
        return 0;

    memset(&ctx->srp_ctx, 0, sizeof(ctx->srp_ctx));
    ctx->srp_ctx.strength = SRP_MINIMAL_N;

    return 1;
}

int SSL_CTX_SRP_CTX_init(SSL_CTX *ctx)
{
    return ssl_ctx_srp_ctx_init_intern(ctx);
}

/* server side */
/*
 * The public API SSL_srp_server_param_with_username() is deprecated so we use
 * ssl_srp_server_param_with_username_intern() internally.
 */
int ssl_srp_server_param_with_username_intern(SSL_CONNECTION *s, int *ad)
{
    unsigned char b[SSL_MAX_MASTER_KEY_LENGTH];
    int al;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    *ad = SSL_AD_UNKNOWN_PSK_IDENTITY;
    if ((s->srp_ctx.TLS_ext_srp_username_callback != NULL) &&
        ((al =
          s->srp_ctx.TLS_ext_srp_username_callback(SSL_CONNECTION_GET_USER_SSL(s),
                                                   ad,
                                                   s->srp_ctx.SRP_cb_arg)) !=
         SSL_ERROR_NONE))
        return al;

    *ad = SSL_AD_INTERNAL_ERROR;
    if ((s->srp_ctx.N == NULL) ||
        (s->srp_ctx.g == NULL) ||
        (s->srp_ctx.s == NULL) || (s->srp_ctx.v == NULL))
        return SSL3_AL_FATAL;

    if (RAND_priv_bytes_ex(SSL_CONNECTION_GET_CTX(s)->libctx, b, sizeof(b),
                           0) <= 0)
        return SSL3_AL_FATAL;
    s->srp_ctx.b = BN_bin2bn(b, sizeof(b), NULL);
    OPENSSL_cleanse(b, sizeof(b));

    /* Calculate:  B = (kv + g^b) % N  */

    return ((s->srp_ctx.B =
             SRP_Calc_B_ex(s->srp_ctx.b, s->srp_ctx.N, s->srp_ctx.g,
                           s->srp_ctx.v, sctx->libctx, sctx->propq)) !=
            NULL) ? SSL_ERROR_NONE : SSL3_AL_FATAL;
}

int SSL_srp_server_param_with_username(SSL *s, int *ad)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return SSL3_AL_FATAL;

    return ssl_srp_server_param_with_username_intern(sc, ad);
}

/*
 * If the server just has the raw password, make up a verifier entry on the
 * fly
 */
int SSL_set_srp_server_param_pw(SSL *s, const char *user, const char *pass,
                                const char *grp)
{
    SRP_gN *GN;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    GN = SRP_get_default_gN(grp);
    if (GN == NULL)
        return -1;
    sc->srp_ctx.N = BN_dup(GN->N);
    sc->srp_ctx.g = BN_dup(GN->g);
    BN_clear_free(sc->srp_ctx.v);
    sc->srp_ctx.v = NULL;
    BN_clear_free(sc->srp_ctx.s);
    sc->srp_ctx.s = NULL;
    if (!SRP_create_verifier_BN_ex(user, pass, &sc->srp_ctx.s, &sc->srp_ctx.v,
                                   sc->srp_ctx.N, sc->srp_ctx.g, s->ctx->libctx,
                                   s->ctx->propq))
        return -1;

    return 1;
}

int SSL_set_srp_server_param(SSL *s, const BIGNUM *N, const BIGNUM *g,
                             BIGNUM *sa, BIGNUM *v, char *info)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return -1;

    if (N != NULL) {
        if (sc->srp_ctx.N != NULL) {
            if (!BN_copy(sc->srp_ctx.N, N)) {
                BN_free(sc->srp_ctx.N);
                sc->srp_ctx.N = NULL;
            }
        } else
            sc->srp_ctx.N = BN_dup(N);
    }
    if (g != NULL) {
        if (sc->srp_ctx.g != NULL) {
            if (!BN_copy(sc->srp_ctx.g, g)) {
                BN_free(sc->srp_ctx.g);
                sc->srp_ctx.g = NULL;
            }
        } else
            sc->srp_ctx.g = BN_dup(g);
    }
    if (sa != NULL) {
        if (sc->srp_ctx.s != NULL) {
            if (!BN_copy(sc->srp_ctx.s, sa)) {
                BN_free(sc->srp_ctx.s);
                sc->srp_ctx.s = NULL;
            }
        } else
            sc->srp_ctx.s = BN_dup(sa);
    }
    if (v != NULL) {
        if (sc->srp_ctx.v != NULL) {
            if (!BN_copy(sc->srp_ctx.v, v)) {
                BN_free(sc->srp_ctx.v);
                sc->srp_ctx.v = NULL;
            }
        } else
            sc->srp_ctx.v = BN_dup(v);
    }
    if (info != NULL) {
        if (sc->srp_ctx.info)
            OPENSSL_free(sc->srp_ctx.info);
        if ((sc->srp_ctx.info = OPENSSL_strdup(info)) == NULL)
            return -1;
    }

    if (!(sc->srp_ctx.N) ||
        !(sc->srp_ctx.g) || !(sc->srp_ctx.s) || !(sc->srp_ctx.v))
        return -1;

    return 1;
}

int srp_generate_server_master_secret(SSL_CONNECTION *s)
{
    BIGNUM *K = NULL, *u = NULL;
    int ret = 0, tmp_len = 0;
    unsigned char *tmp = NULL;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    if (!SRP_Verify_A_mod_N(s->srp_ctx.A, s->srp_ctx.N))
        goto err;
    if ((u = SRP_Calc_u_ex(s->srp_ctx.A, s->srp_ctx.B, s->srp_ctx.N,
                           sctx->libctx, sctx->propq)) == NULL)
        goto err;
    if ((K = SRP_Calc_server_key(s->srp_ctx.A, s->srp_ctx.v, u, s->srp_ctx.b,
                                 s->srp_ctx.N)) == NULL)
        goto err;

    tmp_len = BN_num_bytes(K);
    if ((tmp = OPENSSL_malloc(tmp_len)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }
    BN_bn2bin(K, tmp);
    /* Calls SSLfatal() as required */
    ret = ssl_generate_master_secret(s, tmp, tmp_len, 1);
 err:
    BN_clear_free(K);
    BN_clear_free(u);
    return ret;
}

/* client side */
int srp_generate_client_master_secret(SSL_CONNECTION *s)
{
    BIGNUM *x = NULL, *u = NULL, *K = NULL;
    int ret = 0, tmp_len = 0;
    char *passwd = NULL;
    unsigned char *tmp = NULL;
    SSL_CTX *sctx = SSL_CONNECTION_GET_CTX(s);

    /*
     * Checks if b % n == 0
     */
    if (SRP_Verify_B_mod_N(s->srp_ctx.B, s->srp_ctx.N) == 0
            || (u = SRP_Calc_u_ex(s->srp_ctx.A, s->srp_ctx.B, s->srp_ctx.N,
                                  sctx->libctx, sctx->propq))
               == NULL
            || s->srp_ctx.SRP_give_srp_client_pwd_callback == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if ((passwd = s->srp_ctx.SRP_give_srp_client_pwd_callback(SSL_CONNECTION_GET_USER_SSL(s),
                                                              s->srp_ctx.SRP_cb_arg))
            == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, SSL_R_CALLBACK_FAILED);
        goto err;
    }
    if ((x = SRP_Calc_x_ex(s->srp_ctx.s, s->srp_ctx.login, passwd,
                           sctx->libctx, sctx->propq)) == NULL
            || (K = SRP_Calc_client_key_ex(s->srp_ctx.N, s->srp_ctx.B,
                                           s->srp_ctx.g, x,
                                           s->srp_ctx.a, u,
                                           sctx->libctx,
                                           sctx->propq)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    tmp_len = BN_num_bytes(K);
    if ((tmp = OPENSSL_malloc(tmp_len)) == NULL) {
        SSLfatal(s, SSL_AD_INTERNAL_ERROR, ERR_R_CRYPTO_LIB);
        goto err;
    }
    BN_bn2bin(K, tmp);
    /* Calls SSLfatal() as required */
    ret = ssl_generate_master_secret(s, tmp, tmp_len, 1);
 err:
    BN_clear_free(K);
    BN_clear_free(x);
    if (passwd != NULL)
        OPENSSL_clear_free(passwd, strlen(passwd));
    BN_clear_free(u);
    return ret;
}

int srp_verify_server_param(SSL_CONNECTION *s)
{
    SRP_CTX *srp = &s->srp_ctx;
    /*
     * Sanity check parameters: we can quickly check B % N == 0 by checking B
     * != 0 since B < N
     */
    if (BN_ucmp(srp->g, srp->N) >= 0 || BN_ucmp(srp->B, srp->N) >= 0
        || BN_is_zero(srp->B)) {
        SSLfatal(s, SSL_AD_ILLEGAL_PARAMETER, SSL_R_BAD_DATA);
        return 0;
    }

    if (BN_num_bits(srp->N) < srp->strength) {
        SSLfatal(s, SSL_AD_INSUFFICIENT_SECURITY, SSL_R_INSUFFICIENT_SECURITY);
        return 0;
    }

    if (srp->SRP_verify_param_callback) {
        if (srp->SRP_verify_param_callback(SSL_CONNECTION_GET_USER_SSL(s),
                                           srp->SRP_cb_arg) <= 0) {
            SSLfatal(s, SSL_AD_INSUFFICIENT_SECURITY, SSL_R_CALLBACK_FAILED);
            return 0;
        }
    } else if (!SRP_check_known_gN_param(srp->g, srp->N)) {
        SSLfatal(s, SSL_AD_INSUFFICIENT_SECURITY,
                 SSL_R_INSUFFICIENT_SECURITY);
        return 0;
    }

    return 1;
}

/*
 * The public API SRP_Calc_A_param() is deprecated so we use
 * ssl_srp_calc_a_param_intern() internally.
 */
int ssl_srp_calc_a_param_intern(SSL_CONNECTION *s)
{
    unsigned char rnd[SSL_MAX_MASTER_KEY_LENGTH];

    if (RAND_priv_bytes_ex(SSL_CONNECTION_GET_CTX(s)->libctx,
                           rnd, sizeof(rnd), 0) <= 0)
        return 0;
    s->srp_ctx.a = BN_bin2bn(rnd, sizeof(rnd), s->srp_ctx.a);
    OPENSSL_cleanse(rnd, sizeof(rnd));

    if (!(s->srp_ctx.A = SRP_Calc_A(s->srp_ctx.a, s->srp_ctx.N, s->srp_ctx.g)))
        return 0;

    return 1;
}

int SRP_Calc_A_param(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return 0;

    return ssl_srp_calc_a_param_intern(sc);
}

BIGNUM *SSL_get_srp_g(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->srp_ctx.g != NULL)
        return sc->srp_ctx.g;
    return s->ctx->srp_ctx.g;
}

BIGNUM *SSL_get_srp_N(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->srp_ctx.N != NULL)
        return sc->srp_ctx.N;
    return s->ctx->srp_ctx.N;
}

char *SSL_get_srp_username(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->srp_ctx.login != NULL)
        return sc->srp_ctx.login;
    return s->ctx->srp_ctx.login;
}

char *SSL_get_srp_userinfo(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);

    if (sc == NULL)
        return NULL;

    if (sc->srp_ctx.info != NULL)
        return sc->srp_ctx.info;
    return s->ctx->srp_ctx.info;
}

# define tls1_ctx_ctrl ssl3_ctx_ctrl
# define tls1_ctx_callback_ctrl ssl3_ctx_callback_ctrl

int SSL_CTX_set_srp_username(SSL_CTX *ctx, char *name)
{
    return tls1_ctx_ctrl(ctx, SSL_CTRL_SET_TLS_EXT_SRP_USERNAME, 0, name);
}

int SSL_CTX_set_srp_password(SSL_CTX *ctx, char *password)
{
    return tls1_ctx_ctrl(ctx, SSL_CTRL_SET_TLS_EXT_SRP_PASSWORD, 0, password);
}

int SSL_CTX_set_srp_strength(SSL_CTX *ctx, int strength)
{
    return tls1_ctx_ctrl(ctx, SSL_CTRL_SET_TLS_EXT_SRP_STRENGTH, strength,
                         NULL);
}

int SSL_CTX_set_srp_verify_param_callback(SSL_CTX *ctx,
                                          int (*cb) (SSL *, void *))
{
    return tls1_ctx_callback_ctrl(ctx, SSL_CTRL_SET_SRP_VERIFY_PARAM_CB,
                                  (void (*)(void))cb);
}

int SSL_CTX_set_srp_cb_arg(SSL_CTX *ctx, void *arg)
{
    return tls1_ctx_ctrl(ctx, SSL_CTRL_SET_SRP_ARG, 0, arg);
}

int SSL_CTX_set_srp_username_callback(SSL_CTX *ctx,
                                      int (*cb) (SSL *, int *, void *))
{
    return tls1_ctx_callback_ctrl(ctx, SSL_CTRL_SET_TLS_EXT_SRP_USERNAME_CB,
                                  (void (*)(void))cb);
}

int SSL_CTX_set_srp_client_pwd_callback(SSL_CTX *ctx,
                                        char *(*cb) (SSL *, void *))
{
    return tls1_ctx_callback_ctrl(ctx, SSL_CTRL_SET_SRP_GIVE_CLIENT_PWD_CB,
                                  (void (*)(void))cb);
}

#endif
