/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This file is to enable backwards compatibility for the SRP features of
 * s_client, s_server and ciphers. All of those features are deprecated and will
 * eventually disappear. In the meantime, to continue to support them, we
 * need to access deprecated SRP APIs.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/bn.h>
#include <openssl/bio.h>
#include <openssl/ssl.h>
#include <openssl/srp.h>
#include "apps_ui.h"
#include "apps.h"
#include "s_apps.h"

static int srp_Verify_N_and_g(const BIGNUM *N, const BIGNUM *g)
{
    BN_CTX *bn_ctx = BN_CTX_new();
    BIGNUM *p = BN_new();
    BIGNUM *r = BN_new();
    int ret =
        g != NULL && N != NULL && bn_ctx != NULL && BN_is_odd(N) &&
        BN_check_prime(N, bn_ctx, NULL) == 1 &&
        p != NULL && BN_rshift1(p, N) &&
        /* p = (N-1)/2 */
        BN_check_prime(p, bn_ctx, NULL) == 1 &&
        r != NULL &&
        /* verify g^((N-1)/2) == -1 (mod N) */
        BN_mod_exp(r, g, p, N, bn_ctx) &&
        BN_add_word(r, 1) && BN_cmp(r, N) == 0;

    BN_free(r);
    BN_free(p);
    BN_CTX_free(bn_ctx);
    return ret;
}

/*-
 * This callback is used here for two purposes:
 * - extended debugging
 * - making some primality tests for unknown groups
 * The callback is only called for a non default group.
 *
 * An application does not need the call back at all if
 * only the standard groups are used.  In real life situations,
 * client and server already share well known groups,
 * thus there is no need to verify them.
 * Furthermore, in case that a server actually proposes a group that
 * is not one of those defined in RFC 5054, it is more appropriate
 * to add the group to a static list and then compare since
 * primality tests are rather cpu consuming.
 */

static int ssl_srp_verify_param_cb(SSL *s, void *arg)
{
    SRP_ARG *srp_arg = (SRP_ARG *)arg;
    BIGNUM *N = NULL, *g = NULL;

    if (((N = SSL_get_srp_N(s)) == NULL) || ((g = SSL_get_srp_g(s)) == NULL))
        return 0;
    if (srp_arg->debug || srp_arg->msg || srp_arg->amp == 1) {
        BIO_printf(bio_err, "SRP parameters:\n");
        BIO_printf(bio_err, "\tN=");
        BN_print(bio_err, N);
        BIO_printf(bio_err, "\n\tg=");
        BN_print(bio_err, g);
        BIO_printf(bio_err, "\n");
    }

    if (SRP_check_known_gN_param(g, N))
        return 1;

    if (srp_arg->amp == 1) {
        if (srp_arg->debug)
            BIO_printf(bio_err,
                       "SRP param N and g are not known params, going to check deeper.\n");

        /*
         * The srp_moregroups is a real debugging feature. Implementers
         * should rather add the value to the known ones. The minimal size
         * has already been tested.
         */
        if (BN_num_bits(g) <= BN_BITS && srp_Verify_N_and_g(N, g))
            return 1;
    }
    BIO_printf(bio_err, "SRP param N and g rejected.\n");
    return 0;
}

#define PWD_STRLEN 1024

static char *ssl_give_srp_client_pwd_cb(SSL *s, void *arg)
{
    SRP_ARG *srp_arg = (SRP_ARG *)arg;
    char *pass = app_malloc(PWD_STRLEN + 1, "SRP password buffer");
    PW_CB_DATA cb_tmp;
    int l;

    cb_tmp.password = (char *)srp_arg->srppassin;
    cb_tmp.prompt_info = "SRP user";
    if ((l = password_callback(pass, PWD_STRLEN, 0, &cb_tmp)) < 0) {
        BIO_printf(bio_err, "Can't read Password\n");
        OPENSSL_free(pass);
        return NULL;
    }
    *(pass + l) = '\0';

    return pass;
}

int set_up_srp_arg(SSL_CTX *ctx, SRP_ARG *srp_arg, int srp_lateuser, int c_msg,
                   int c_debug)
{
    if (!srp_lateuser && !SSL_CTX_set_srp_username(ctx, srp_arg->srplogin)) {
        BIO_printf(bio_err, "Unable to set SRP username\n");
        return 0;
    }
    srp_arg->msg = c_msg;
    srp_arg->debug = c_debug;
    SSL_CTX_set_srp_cb_arg(ctx, &srp_arg);
    SSL_CTX_set_srp_client_pwd_callback(ctx, ssl_give_srp_client_pwd_cb);
    SSL_CTX_set_srp_strength(ctx, srp_arg->strength);
    if (c_msg || c_debug || srp_arg->amp == 0)
        SSL_CTX_set_srp_verify_param_callback(ctx, ssl_srp_verify_param_cb);

    return 1;
}

static char *dummy_srp(SSL *ssl, void *arg)
{
    return "";
}

void set_up_dummy_srp(SSL_CTX *ctx)
{
        SSL_CTX_set_srp_client_pwd_callback(ctx, dummy_srp);
}

/*
 * This callback pretends to require some asynchronous logic in order to
 * obtain a verifier. When the callback is called for a new connection we
 * return with a negative value. This will provoke the accept etc to return
 * with an LOOKUP_X509. The main logic of the reinvokes the suspended call
 * (which would normally occur after a worker has finished) and we set the
 * user parameters.
 */
static int ssl_srp_server_param_cb(SSL *s, int *ad, void *arg)
{
    srpsrvparm *p = (srpsrvparm *) arg;
    int ret = SSL3_AL_FATAL;

    if (p->login == NULL && p->user == NULL) {
        p->login = SSL_get_srp_username(s);
        BIO_printf(bio_err, "SRP username = \"%s\"\n", p->login);
        return -1;
    }

    if (p->user == NULL) {
        BIO_printf(bio_err, "User %s doesn't exist\n", p->login);
        goto err;
    }

    if (SSL_set_srp_server_param
        (s, p->user->N, p->user->g, p->user->s, p->user->v,
         p->user->info) < 0) {
        *ad = SSL_AD_INTERNAL_ERROR;
        goto err;
    }
    BIO_printf(bio_err,
               "SRP parameters set: username = \"%s\" info=\"%s\"\n",
               p->login, p->user->info);
    ret = SSL_ERROR_NONE;

 err:
    SRP_user_pwd_free(p->user);
    p->user = NULL;
    p->login = NULL;
    return ret;
}

int set_up_srp_verifier_file(SSL_CTX *ctx, srpsrvparm *srp_callback_parm,
                             char *srpuserseed, char *srp_verifier_file)
{
    int ret;

    srp_callback_parm->vb = SRP_VBASE_new(srpuserseed);
    srp_callback_parm->user = NULL;
    srp_callback_parm->login = NULL;

    if (srp_callback_parm->vb == NULL) {
        BIO_printf(bio_err, "Failed to initialize SRP verifier file\n");
        return 0;
    }
    if ((ret =
            SRP_VBASE_init(srp_callback_parm->vb,
                           srp_verifier_file)) != SRP_NO_ERROR) {
        BIO_printf(bio_err,
                    "Cannot initialize SRP verifier file \"%s\":ret=%d\n",
                    srp_verifier_file, ret);
        return 0;
    }
    SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, verify_callback);
    SSL_CTX_set_srp_cb_arg(ctx, &srp_callback_parm);
    SSL_CTX_set_srp_username_callback(ctx, ssl_srp_server_param_cb);

    return 1;
}

void lookup_srp_user(srpsrvparm *srp_callback_parm, BIO *bio_s_out)
{
    SRP_user_pwd_free(srp_callback_parm->user);
    srp_callback_parm->user = SRP_VBASE_get1_by_user(srp_callback_parm->vb,
                                                     srp_callback_parm->login);

    if (srp_callback_parm->user != NULL)
        BIO_printf(bio_s_out, "LOOKUP done %s\n",
                    srp_callback_parm->user->info);
    else
        BIO_printf(bio_s_out, "LOOKUP not successful\n");
}
