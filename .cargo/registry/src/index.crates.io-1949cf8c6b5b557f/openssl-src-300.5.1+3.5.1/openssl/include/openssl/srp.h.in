/*
 * {- join("\n * ", @autowarntext) -}
 *
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

{-
use OpenSSL::stackhash qw(generate_stack_macros);
-}

#ifndef OPENSSL_SRP_H
# define OPENSSL_SRP_H
# pragma once

# include <openssl/macros.h>
# ifndef OPENSSL_NO_DEPRECATED_3_0
#  define HEADER_SRP_H
# endif

#include <openssl/opensslconf.h>

#ifndef OPENSSL_NO_SRP
# include <stdio.h>
# include <string.h>
# include <openssl/safestack.h>
# include <openssl/bn.h>
# include <openssl/crypto.h>

# ifdef  __cplusplus
extern "C" {
# endif

# ifndef OPENSSL_NO_DEPRECATED_3_0

typedef struct SRP_gN_cache_st {
    char *b64_bn;
    BIGNUM *bn;
} SRP_gN_cache;
{-
    generate_stack_macros("SRP_gN_cache");
-}


typedef struct SRP_user_pwd_st {
    /* Owned by us. */
    char *id;
    BIGNUM *s;
    BIGNUM *v;
    /* Not owned by us. */
    const BIGNUM *g;
    const BIGNUM *N;
    /* Owned by us. */
    char *info;
} SRP_user_pwd;
{-
    generate_stack_macros("SRP_user_pwd");
-}

OSSL_DEPRECATEDIN_3_0
SRP_user_pwd *SRP_user_pwd_new(void);
OSSL_DEPRECATEDIN_3_0
void SRP_user_pwd_free(SRP_user_pwd *user_pwd);

OSSL_DEPRECATEDIN_3_0
void SRP_user_pwd_set_gN(SRP_user_pwd *user_pwd, const BIGNUM *g,
                         const BIGNUM *N);
OSSL_DEPRECATEDIN_3_0
int SRP_user_pwd_set1_ids(SRP_user_pwd *user_pwd, const char *id,
                          const char *info);
OSSL_DEPRECATEDIN_3_0
int SRP_user_pwd_set0_sv(SRP_user_pwd *user_pwd, BIGNUM *s, BIGNUM *v);

typedef struct SRP_VBASE_st {
    STACK_OF(SRP_user_pwd) *users_pwd;
    STACK_OF(SRP_gN_cache) *gN_cache;
/* to simulate a user */
    char *seed_key;
    const BIGNUM *default_g;
    const BIGNUM *default_N;
} SRP_VBASE;

/*
 * Internal structure storing N and g pair
 */
typedef struct SRP_gN_st {
    char *id;
    const BIGNUM *g;
    const BIGNUM *N;
} SRP_gN;
{-
    generate_stack_macros("SRP_gN");
-}


OSSL_DEPRECATEDIN_3_0
SRP_VBASE *SRP_VBASE_new(char *seed_key);
OSSL_DEPRECATEDIN_3_0
void SRP_VBASE_free(SRP_VBASE *vb);
OSSL_DEPRECATEDIN_3_0
int SRP_VBASE_init(SRP_VBASE *vb, char *verifier_file);

OSSL_DEPRECATEDIN_3_0
int SRP_VBASE_add0_user(SRP_VBASE *vb, SRP_user_pwd *user_pwd);

/* NOTE: unlike in SRP_VBASE_get_by_user, caller owns the returned pointer.*/
OSSL_DEPRECATEDIN_3_0
SRP_user_pwd *SRP_VBASE_get1_by_user(SRP_VBASE *vb, char *username);

OSSL_DEPRECATEDIN_3_0
char *SRP_create_verifier_ex(const char *user, const char *pass, char **salt,
                             char **verifier, const char *N, const char *g,
                             OSSL_LIB_CTX *libctx, const char *propq);
OSSL_DEPRECATEDIN_3_0
char *SRP_create_verifier(const char *user, const char *pass, char **salt,
                          char **verifier, const char *N, const char *g);
OSSL_DEPRECATEDIN_3_0
int SRP_create_verifier_BN_ex(const char *user, const char *pass, BIGNUM **salt,
                              BIGNUM **verifier, const BIGNUM *N,
                              const BIGNUM *g, OSSL_LIB_CTX *libctx,
                              const char *propq);
OSSL_DEPRECATEDIN_3_0
int SRP_create_verifier_BN(const char *user, const char *pass, BIGNUM **salt,
                           BIGNUM **verifier, const BIGNUM *N,
                           const BIGNUM *g);

#  define SRP_NO_ERROR 0
#  define SRP_ERR_VBASE_INCOMPLETE_FILE 1
#  define SRP_ERR_VBASE_BN_LIB 2
#  define SRP_ERR_OPEN_FILE 3
#  define SRP_ERR_MEMORY 4

#  define DB_srptype      0
#  define DB_srpverifier  1
#  define DB_srpsalt      2
#  define DB_srpid        3
#  define DB_srpgN        4
#  define DB_srpinfo      5
#  undef  DB_NUMBER
#  define DB_NUMBER       6

#  define DB_SRP_INDEX    'I'
#  define DB_SRP_VALID    'V'
#  define DB_SRP_REVOKED  'R'
#  define DB_SRP_MODIF    'v'

/* see srp.c */
OSSL_DEPRECATEDIN_3_0
char *SRP_check_known_gN_param(const BIGNUM *g, const BIGNUM *N);
OSSL_DEPRECATEDIN_3_0
SRP_gN *SRP_get_default_gN(const char *id);

/* server side .... */
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_server_key(const BIGNUM *A, const BIGNUM *v, const BIGNUM *u,
                            const BIGNUM *b, const BIGNUM *N);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_B_ex(const BIGNUM *b, const BIGNUM *N, const BIGNUM *g,
                      const BIGNUM *v, OSSL_LIB_CTX *libctx, const char *propq);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_B(const BIGNUM *b, const BIGNUM *N, const BIGNUM *g,
                   const BIGNUM *v);

OSSL_DEPRECATEDIN_3_0
int SRP_Verify_A_mod_N(const BIGNUM *A, const BIGNUM *N);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_u_ex(const BIGNUM *A, const BIGNUM *B, const BIGNUM *N,
                      OSSL_LIB_CTX *libctx, const char *propq);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_u(const BIGNUM *A, const BIGNUM *B, const BIGNUM *N);

/* client side .... */

OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_x_ex(const BIGNUM *s, const char *user, const char *pass,
                      OSSL_LIB_CTX *libctx, const char *propq);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_x(const BIGNUM *s, const char *user, const char *pass);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_A(const BIGNUM *a, const BIGNUM *N, const BIGNUM *g);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_client_key_ex(const BIGNUM *N, const BIGNUM *B, const BIGNUM *g,
                            const BIGNUM *x, const BIGNUM *a, const BIGNUM *u,
                            OSSL_LIB_CTX *libctx, const char *propq);
OSSL_DEPRECATEDIN_3_0
BIGNUM *SRP_Calc_client_key(const BIGNUM *N, const BIGNUM *B, const BIGNUM *g,
                            const BIGNUM *x, const BIGNUM *a, const BIGNUM *u);
OSSL_DEPRECATEDIN_3_0
int SRP_Verify_B_mod_N(const BIGNUM *B, const BIGNUM *N);

#  define SRP_MINIMAL_N 1024

# endif /* OPENSSL_NO_DEPRECATED_3_0 */

/* This method ignores the configured seed and fails for an unknown user. */
# ifndef OPENSSL_NO_DEPRECATED_1_1_0
OSSL_DEPRECATEDIN_1_1_0
SRP_user_pwd *SRP_VBASE_get_by_user(SRP_VBASE *vb, char *username);
# endif

# ifdef  __cplusplus
}
# endif
# endif

#endif
