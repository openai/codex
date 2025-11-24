/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/err.h>
#include <openssl/opensslconf.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "internal/thread_once.h"
#include "crypto/rand.h"
#include "crypto/cryptlib.h"
#include "rand_local.h"
#include "crypto/context.h"
#include "internal/provider.h"

#ifndef OPENSSL_DEFAULT_SEED_SRC
# define OPENSSL_DEFAULT_SEED_SRC SEED-SRC
#endif

typedef struct rand_global_st {
    /*
     * The three shared DRBG instances
     *
     * There are three shared DRBG instances: <primary>, <public>, and
     * <private>.  The <public> and <private> DRBGs are secondary ones.
     * These are used for non-secret (e.g. nonces) and secret
     * (e.g. private keys) data respectively.
     */
    CRYPTO_RWLOCK *lock;

    EVP_RAND_CTX *seed;

    /*
     * The <primary> DRBG
     *
     * Not used directly by the application, only for reseeding the two other
     * DRBGs. It reseeds itself by pulling either randomness from os entropy
     * sources or by consuming randomness which was added by RAND_add().
     *
     * The <primary> DRBG is a global instance which is accessed concurrently by
     * all threads. The necessary locking is managed automatically by its child
     * DRBG instances during reseeding.
     */
    EVP_RAND_CTX *primary;

    /*
     * The provider which we'll use to generate randomness.
     */
#ifndef FIPS_MODULE
    OSSL_PROVIDER *random_provider;
    char *random_provider_name;
#endif      /* !FIPS_MODULE */

    /*
     * The <public> DRBG
     *
     * Used by default for generating random bytes using RAND_bytes().
     *
     * The <public> secondary DRBG is thread-local, i.e., there is one instance
     * per thread.
     */
    CRYPTO_THREAD_LOCAL public;

    /*
     * The <private> DRBG
     *
     * Used by default for generating private keys using RAND_priv_bytes()
     *
     * The <private> secondary DRBG is thread-local, i.e., there is one
     * instance per thread.
     */
    CRYPTO_THREAD_LOCAL private;

    /* Which RNG is being used by default and it's configuration settings */
    char *rng_name;
    char *rng_cipher;
    char *rng_digest;
    char *rng_propq;

    /* Allow the randomness source to be changed */
    char *seed_name;
    char *seed_propq;
} RAND_GLOBAL;

static EVP_RAND_CTX *rand_get0_primary(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl);
static EVP_RAND_CTX *rand_get0_public(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl);
static EVP_RAND_CTX *rand_get0_private(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl);

static RAND_GLOBAL *rand_get_global(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_DRBG_INDEX);
}

#ifndef FIPS_MODULE
# include <stdio.h>
# include <time.h>
# include <limits.h>
# include <openssl/conf.h>
# include <openssl/trace.h>
# include <openssl/engine.h>
# include "crypto/rand_pool.h"
# include "prov/seeding.h"
# include "internal/e_os.h"
# include "internal/property.h"

/*
 * The default name for the random provider.
 * This ensures that the FIPS provider will supply libcrypto's random byte
 * requirements.
 */
static const char random_provider_fips_name[] = "fips";

static int set_random_provider_name(RAND_GLOBAL *dgbl, const char *name)
{
    if (dgbl->random_provider_name != NULL
            && OPENSSL_strcasecmp(dgbl->random_provider_name, name) == 0)
        return 1;

    OPENSSL_free(dgbl->random_provider_name);
    dgbl->random_provider_name = OPENSSL_strdup(name);
    return dgbl->random_provider_name != NULL;
}

# ifndef OPENSSL_NO_ENGINE
/* non-NULL if default_RAND_meth is ENGINE-provided */
static ENGINE *funct_ref;
static CRYPTO_RWLOCK *rand_engine_lock;
# endif     /* !OPENSSL_NO_ENGINE */
# ifndef OPENSSL_NO_DEPRECATED_3_0
static CRYPTO_RWLOCK *rand_meth_lock;
static const RAND_METHOD *default_RAND_meth;
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */
static CRYPTO_ONCE rand_init = CRYPTO_ONCE_STATIC_INIT;

static int rand_inited = 0;

DEFINE_RUN_ONCE_STATIC(do_rand_init)
{
# ifndef OPENSSL_NO_ENGINE
    rand_engine_lock = CRYPTO_THREAD_lock_new();
    if (rand_engine_lock == NULL)
        return 0;
# endif     /* !OPENSSL_NO_ENGINE */

# ifndef OPENSSL_NO_DEPRECATED_3_0
    rand_meth_lock = CRYPTO_THREAD_lock_new();
    if (rand_meth_lock == NULL)
        goto err;
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */

    if (!ossl_rand_pool_init())
        goto err;

    rand_inited = 1;
    return 1;

 err:
# ifndef OPENSSL_NO_DEPRECATED_3_0
    CRYPTO_THREAD_lock_free(rand_meth_lock);
    rand_meth_lock = NULL;
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */
# ifndef OPENSSL_NO_ENGINE
    CRYPTO_THREAD_lock_free(rand_engine_lock);
    rand_engine_lock = NULL;
# endif     /* !OPENSSL_NO_ENGINE */
    return 0;
}

void ossl_rand_cleanup_int(void)
{
# ifndef OPENSSL_NO_DEPRECATED_3_0
    const RAND_METHOD *meth = default_RAND_meth;

    if (!rand_inited)
        return;

    if (meth != NULL && meth->cleanup != NULL)
        meth->cleanup();
    RAND_set_rand_method(NULL);
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */
    ossl_rand_pool_cleanup();
# ifndef OPENSSL_NO_ENGINE
    CRYPTO_THREAD_lock_free(rand_engine_lock);
    rand_engine_lock = NULL;
# endif     /* !OPENSSL_NO_ENGINE */
# ifndef OPENSSL_NO_DEPRECATED_3_0
    CRYPTO_THREAD_lock_free(rand_meth_lock);
    rand_meth_lock = NULL;
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */
    ossl_release_default_drbg_ctx();
    rand_inited = 0;
}

/*
 * RAND_close_seed_files() ensures that any seed file descriptors are
 * closed after use.  This only applies to libcrypto/default provider,
 * it does not apply to other providers.
 */
void RAND_keep_random_devices_open(int keep)
{
    if (RUN_ONCE(&rand_init, do_rand_init))
        ossl_rand_pool_keep_random_devices_open(keep);
}

/*
 * RAND_poll() reseeds the default RNG using random input
 *
 * The random input is obtained from polling various entropy
 * sources which depend on the operating system and are
 * configurable via the --with-rand-seed configure option.
 */
int RAND_poll(void)
{
    static const char salt[] = "polling";

# ifndef OPENSSL_NO_DEPRECATED_3_0
    const RAND_METHOD *meth = RAND_get_rand_method();
    int ret = meth == RAND_OpenSSL();

    if (meth == NULL)
        return 0;

    if (!ret) {
        /* fill random pool and seed the current legacy RNG */
        RAND_POOL *pool = ossl_rand_pool_new(RAND_DRBG_STRENGTH, 1,
                                             (RAND_DRBG_STRENGTH + 7) / 8,
                                             RAND_POOL_MAX_LENGTH);

        if (pool == NULL)
            return 0;

        if (ossl_pool_acquire_entropy(pool) == 0)
            goto err;

        if (meth->add == NULL
            || meth->add(ossl_rand_pool_buffer(pool),
                         ossl_rand_pool_length(pool),
                         (ossl_rand_pool_entropy(pool) / 8.0)) == 0)
            goto err;

        ret = 1;
     err:
        ossl_rand_pool_free(pool);
        return ret;
    }
# endif     /* !OPENSSL_NO_DEPRECATED_3_0 */

    RAND_seed(salt, sizeof(salt));
    return 1;
}

# ifndef OPENSSL_NO_DEPRECATED_3_0
static int rand_set_rand_method_internal(const RAND_METHOD *meth,
                                         ossl_unused ENGINE *e)
{
    if (!RUN_ONCE(&rand_init, do_rand_init))
        return 0;

    if (!CRYPTO_THREAD_write_lock(rand_meth_lock))
        return 0;
#  ifndef OPENSSL_NO_ENGINE
    ENGINE_finish(funct_ref);
    funct_ref = e;
#  endif
    default_RAND_meth = meth;
    CRYPTO_THREAD_unlock(rand_meth_lock);
    return 1;
}

int RAND_set_rand_method(const RAND_METHOD *meth)
{
    return rand_set_rand_method_internal(meth, NULL);
}

const RAND_METHOD *RAND_get_rand_method(void)
{
    const RAND_METHOD *tmp_meth = NULL;

    if (!RUN_ONCE(&rand_init, do_rand_init))
        return NULL;

    if (rand_meth_lock == NULL)
        return NULL;

    if (!CRYPTO_THREAD_read_lock(rand_meth_lock))
        return NULL;
    tmp_meth = default_RAND_meth;
    CRYPTO_THREAD_unlock(rand_meth_lock);
    if (tmp_meth != NULL)
        return tmp_meth;

    if (!CRYPTO_THREAD_write_lock(rand_meth_lock))
        return NULL;
    if (default_RAND_meth == NULL) {
#  ifndef OPENSSL_NO_ENGINE
        ENGINE *e;

        /* If we have an engine that can do RAND, use it. */
        if ((e = ENGINE_get_default_RAND()) != NULL
                && (tmp_meth = ENGINE_get_RAND(e)) != NULL) {
            funct_ref = e;
            default_RAND_meth = tmp_meth;
        } else {
            ENGINE_finish(e);
            default_RAND_meth = &ossl_rand_meth;
        }
#  else
        default_RAND_meth = &ossl_rand_meth;
#  endif
    }
    tmp_meth = default_RAND_meth;
    CRYPTO_THREAD_unlock(rand_meth_lock);
    return tmp_meth;
}

#  if !defined(OPENSSL_NO_ENGINE)
int RAND_set_rand_engine(ENGINE *engine)
{
    const RAND_METHOD *tmp_meth = NULL;

    if (!RUN_ONCE(&rand_init, do_rand_init))
        return 0;

    if (engine != NULL) {
        if (!ENGINE_init(engine))
            return 0;
        tmp_meth = ENGINE_get_RAND(engine);
        if (tmp_meth == NULL) {
            ENGINE_finish(engine);
            return 0;
        }
    }
    if (!CRYPTO_THREAD_write_lock(rand_engine_lock)) {
        ENGINE_finish(engine);
        return 0;
    }

    /* This function releases any prior ENGINE so call it first */
    rand_set_rand_method_internal(tmp_meth, engine);
    CRYPTO_THREAD_unlock(rand_engine_lock);
    return 1;
}
#  endif
# endif /* OPENSSL_NO_DEPRECATED_3_0 */

void RAND_seed(const void *buf, int num)
{
    EVP_RAND_CTX *drbg;
# ifndef OPENSSL_NO_DEPRECATED_3_0
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth->seed != NULL) {
        meth->seed(buf, num);
        return;
    }
# endif

    drbg = RAND_get0_primary(NULL);
    if (drbg != NULL && num > 0)
        EVP_RAND_reseed(drbg, 0, NULL, 0, buf, num);
}

void RAND_add(const void *buf, int num, double randomness)
{
    EVP_RAND_CTX *drbg;
# ifndef OPENSSL_NO_DEPRECATED_3_0
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth->add != NULL) {
        meth->add(buf, num, randomness);
        return;
    }
# endif
    drbg = RAND_get0_primary(NULL);
    if (drbg != NULL && num > 0)
# ifdef OPENSSL_RAND_SEED_NONE
        /* Without an entropy source, we have to rely on the user */
        EVP_RAND_reseed(drbg, 0, buf, num, NULL, 0);
# else
        /* With an entropy source, we downgrade this to additional input */
        EVP_RAND_reseed(drbg, 0, NULL, 0, buf, num);
# endif
}

# if !defined(OPENSSL_NO_DEPRECATED_1_1_0)
int RAND_pseudo_bytes(unsigned char *buf, int num)
{
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth->pseudorand != NULL)
        return meth->pseudorand(buf, num);
    ERR_raise(ERR_LIB_RAND, RAND_R_FUNC_NOT_IMPLEMENTED);
    return -1;
}
# endif

int RAND_status(void)
{
    EVP_RAND_CTX *rand;
# ifndef OPENSSL_NO_DEPRECATED_3_0
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth != RAND_OpenSSL())
        return meth->status != NULL ? meth->status() : 0;
# endif

    if ((rand = RAND_get0_primary(NULL)) == NULL)
        return 0;
    return EVP_RAND_get_state(rand) == EVP_RAND_STATE_READY;
}
# else  /* !FIPS_MODULE */

# ifndef OPENSSL_NO_DEPRECATED_3_0
const RAND_METHOD *RAND_get_rand_method(void)
{
    return NULL;
}
# endif
#endif /* !FIPS_MODULE */

/*
 * This function is not part of RAND_METHOD, so if we're not using
 * the default method, then just call RAND_bytes().  Otherwise make
 * sure we're instantiated and use the private DRBG.
 */
int RAND_priv_bytes_ex(OSSL_LIB_CTX *ctx, unsigned char *buf, size_t num,
                       unsigned int strength)
{
    RAND_GLOBAL *dgbl;
    EVP_RAND_CTX *rand;
#if !defined(OPENSSL_NO_DEPRECATED_3_0) && !defined(FIPS_MODULE)
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth != RAND_OpenSSL()) {
        if (meth->bytes != NULL)
            return meth->bytes(buf, num);
        ERR_raise(ERR_LIB_RAND, RAND_R_FUNC_NOT_IMPLEMENTED);
        return -1;
    }
#endif

    dgbl = rand_get_global(ctx);
    if (dgbl == NULL)
        return 0;
#ifndef FIPS_MODULE
    if (dgbl->random_provider != NULL)
        return ossl_provider_random_bytes(dgbl->random_provider,
                                          OSSL_PROV_RANDOM_PRIVATE,
                                          buf, num, strength);
#endif      /* !FIPS_MODULE */
    rand = rand_get0_private(ctx, dgbl);
    if (rand != NULL)
        return EVP_RAND_generate(rand, buf, num, strength, 0, NULL, 0);

    return 0;
}

int RAND_priv_bytes(unsigned char *buf, int num)
{
    if (num < 0)
        return 0;
    return RAND_priv_bytes_ex(NULL, buf, (size_t)num, 0);
}

int RAND_bytes_ex(OSSL_LIB_CTX *ctx, unsigned char *buf, size_t num,
                  unsigned int strength)
{
    RAND_GLOBAL *dgbl;
    EVP_RAND_CTX *rand;
#if !defined(OPENSSL_NO_DEPRECATED_3_0) && !defined(FIPS_MODULE)
    const RAND_METHOD *meth = RAND_get_rand_method();

    if (meth != NULL && meth != RAND_OpenSSL()) {
        if (meth->bytes != NULL)
            return meth->bytes(buf, num);
        ERR_raise(ERR_LIB_RAND, RAND_R_FUNC_NOT_IMPLEMENTED);
        return -1;
    }
#endif

    dgbl = rand_get_global(ctx);
    if (dgbl == NULL)
        return 0;
#ifndef FIPS_MODULE
    if (dgbl->random_provider != NULL)
        return ossl_provider_random_bytes(dgbl->random_provider,
                                          OSSL_PROV_RANDOM_PUBLIC,
                                          buf, num, strength);
#endif      /* !FIPS_MODULE */

    rand = rand_get0_public(ctx, dgbl);
    if (rand != NULL)
        return EVP_RAND_generate(rand, buf, num, strength, 0, NULL, 0);

    return 0;
}

int RAND_bytes(unsigned char *buf, int num)
{
    if (num < 0)
        return 0;
    return RAND_bytes_ex(NULL, buf, (size_t)num, 0);
}

/*
 * Initialize the OSSL_LIB_CTX global DRBGs on first use.
 * Returns the allocated global data on success or NULL on failure.
 */
void *ossl_rand_ctx_new(OSSL_LIB_CTX *libctx)
{
    RAND_GLOBAL *dgbl = OPENSSL_zalloc(sizeof(*dgbl));

    if (dgbl == NULL)
        return NULL;

#ifndef FIPS_MODULE
    /*
     * We need to ensure that base libcrypto thread handling has been
     * initialised.
     */
    OPENSSL_init_crypto(OPENSSL_INIT_BASE_ONLY, NULL);

    /* Prepopulate the random provider name */
    dgbl->random_provider_name = OPENSSL_strdup(random_provider_fips_name);
    if (dgbl->random_provider_name == NULL)
        goto err0;
#endif

    dgbl->lock = CRYPTO_THREAD_lock_new();
    if (dgbl->lock == NULL)
        goto err1;

    if (!CRYPTO_THREAD_init_local(&dgbl->private, NULL))
        goto err1;

    if (!CRYPTO_THREAD_init_local(&dgbl->public, NULL))
        goto err2;

    return dgbl;

 err2:
    CRYPTO_THREAD_cleanup_local(&dgbl->private);
 err1:
    CRYPTO_THREAD_lock_free(dgbl->lock);
#ifndef FIPS_MODULE
 err0:
    OPENSSL_free(dgbl->random_provider_name);
#endif
    OPENSSL_free(dgbl);
    return NULL;
}

void ossl_rand_ctx_free(void *vdgbl)
{
    RAND_GLOBAL *dgbl = vdgbl;

    if (dgbl == NULL)
        return;

    CRYPTO_THREAD_lock_free(dgbl->lock);
    CRYPTO_THREAD_cleanup_local(&dgbl->private);
    CRYPTO_THREAD_cleanup_local(&dgbl->public);
    EVP_RAND_CTX_free(dgbl->primary);
    EVP_RAND_CTX_free(dgbl->seed);
#ifndef FIPS_MODULE
    OPENSSL_free(dgbl->random_provider_name);
#endif      /* !FIPS_MODULE */
    OPENSSL_free(dgbl->rng_name);
    OPENSSL_free(dgbl->rng_cipher);
    OPENSSL_free(dgbl->rng_digest);
    OPENSSL_free(dgbl->rng_propq);
    OPENSSL_free(dgbl->seed_name);
    OPENSSL_free(dgbl->seed_propq);

    OPENSSL_free(dgbl);
}

static void rand_delete_thread_state(void *arg)
{
    OSSL_LIB_CTX *ctx = arg;
    RAND_GLOBAL *dgbl = rand_get_global(ctx);
    EVP_RAND_CTX *rand;

    if (dgbl == NULL)
        return;

    rand = CRYPTO_THREAD_get_local(&dgbl->public);
    CRYPTO_THREAD_set_local(&dgbl->public, NULL);
    EVP_RAND_CTX_free(rand);

    rand = CRYPTO_THREAD_get_local(&dgbl->private);
    CRYPTO_THREAD_set_local(&dgbl->private, NULL);
    EVP_RAND_CTX_free(rand);
}

#if !defined(FIPS_MODULE) || !defined(OPENSSL_NO_FIPS_JITTER)
static EVP_RAND_CTX *rand_new_seed(OSSL_LIB_CTX *libctx)
{
    EVP_RAND *rand;
    const char *propq;
    char *name;
    EVP_RAND_CTX *ctx = NULL;
# ifdef OPENSSL_NO_FIPS_JITTER
    RAND_GLOBAL *dgbl = rand_get_global(libctx);

    if (dgbl == NULL)
        return NULL;
    propq = dgbl->seed_propq;
    name = dgbl->seed_name != NULL ? dgbl->seed_name
                                   : OPENSSL_MSTR(OPENSSL_DEFAULT_SEED_SRC);
# else /* !OPENSSL_NO_FIPS_JITTER */
    name = "JITTER";
    propq = "";
# endif /* OPENSSL_NO_FIPS_JITTER */

    rand = EVP_RAND_fetch(libctx, name, propq);
    if (rand == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_FETCH_DRBG);
        goto err;
    }
    ctx = EVP_RAND_CTX_new(rand, NULL);
    EVP_RAND_free(rand);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_CREATE_DRBG);
        goto err;
    }
    if (!EVP_RAND_instantiate(ctx, 0, 0, NULL, 0, NULL)) {
        ERR_raise(ERR_LIB_RAND, RAND_R_ERROR_INSTANTIATING_DRBG);
        goto err;
    }
    return ctx;
 err:
    EVP_RAND_CTX_free(ctx);
    return NULL;
}
#endif  /* !FIPS_MODULE || !OPENSSL_NO_FIPS_JITTER */

#ifndef FIPS_MODULE
EVP_RAND_CTX *ossl_rand_get0_seed_noncreating(OSSL_LIB_CTX *ctx)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);
    EVP_RAND_CTX *ret;

    if (dgbl == NULL)
        return NULL;

    if (!CRYPTO_THREAD_read_lock(dgbl->lock))
        return NULL;
    ret = dgbl->seed;
    CRYPTO_THREAD_unlock(dgbl->lock);
    return ret;
}
#endif  /* !FIPS_MODULE */

static EVP_RAND_CTX *rand_new_drbg(OSSL_LIB_CTX *libctx, EVP_RAND_CTX *parent,
                                   unsigned int reseed_interval,
                                   time_t reseed_time_interval)
{
    EVP_RAND *rand;
    RAND_GLOBAL *dgbl = rand_get_global(libctx);
    EVP_RAND_CTX *ctx;
    OSSL_PARAM params[9], *p = params;
    const OSSL_PARAM *settables;
    const char *prov_name;
    char *name, *cipher;
    int use_df = 1;

    if (dgbl == NULL)
        return NULL;
    name = dgbl->rng_name != NULL ? dgbl->rng_name : "CTR-DRBG";
    rand = EVP_RAND_fetch(libctx, name, dgbl->rng_propq);
    if (rand == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_FETCH_DRBG);
        return NULL;
    }
    prov_name = ossl_provider_name(EVP_RAND_get0_provider(rand));
    ctx = EVP_RAND_CTX_new(rand, parent);
    EVP_RAND_free(rand);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_CREATE_DRBG);
        return NULL;
    }

    settables = EVP_RAND_CTX_settable_params(ctx);
    if (OSSL_PARAM_locate_const(settables, OSSL_DRBG_PARAM_CIPHER)) {
        cipher = dgbl->rng_cipher != NULL ? dgbl->rng_cipher : "AES-256-CTR";
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_CIPHER,
                                                cipher, 0);
    }
    if (dgbl->rng_digest != NULL
            && OSSL_PARAM_locate_const(settables, OSSL_DRBG_PARAM_DIGEST))
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_DIGEST,
                                                dgbl->rng_digest, 0);
    if (prov_name != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PROV_PARAM_CORE_PROV_NAME,
                                                (char *)prov_name, 0);
    if (dgbl->rng_propq != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_PROPERTIES,
                                                dgbl->rng_propq, 0);
    if (OSSL_PARAM_locate_const(settables, OSSL_ALG_PARAM_MAC))
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_MAC, "HMAC", 0);
    if (OSSL_PARAM_locate_const(settables, OSSL_DRBG_PARAM_USE_DF))
        *p++ = OSSL_PARAM_construct_int(OSSL_DRBG_PARAM_USE_DF, &use_df);
    *p++ = OSSL_PARAM_construct_uint(OSSL_DRBG_PARAM_RESEED_REQUESTS,
                                     &reseed_interval);
    *p++ = OSSL_PARAM_construct_time_t(OSSL_DRBG_PARAM_RESEED_TIME_INTERVAL,
                                       &reseed_time_interval);
    *p = OSSL_PARAM_construct_end();
    if (!EVP_RAND_instantiate(ctx, 0, 0, NULL, 0, params)) {
        ERR_raise(ERR_LIB_RAND, RAND_R_ERROR_INSTANTIATING_DRBG);
        EVP_RAND_CTX_free(ctx);
        return NULL;
    }
    return ctx;
}

#if defined(FIPS_MODULE)
static EVP_RAND_CTX *rand_new_crngt(OSSL_LIB_CTX *libctx, EVP_RAND_CTX *parent)
{
    EVP_RAND *rand;
    EVP_RAND_CTX *ctx;

    rand = EVP_RAND_fetch(libctx, "CRNG-TEST", "-fips");
    if (rand == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_FETCH_DRBG);
        return NULL;
    }
    ctx = EVP_RAND_CTX_new(rand, parent);
    EVP_RAND_free(rand);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_RAND, RAND_R_UNABLE_TO_CREATE_DRBG);
        return NULL;
    }

    if (!EVP_RAND_instantiate(ctx, 0, 0, NULL, 0, NULL)) {
        ERR_raise(ERR_LIB_RAND, RAND_R_ERROR_INSTANTIATING_DRBG);
        EVP_RAND_CTX_free(ctx);
        return NULL;
    }
    return ctx;
}
#endif  /* FIPS_MODULE */

/*
 * Get the primary random generator.
 * Returns pointer to its EVP_RAND_CTX on success, NULL on failure.
 *
 */
static EVP_RAND_CTX *rand_get0_primary(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl)
{
    EVP_RAND_CTX *ret, *seed, *newseed = NULL, *primary;

    if (dgbl == NULL)
        return NULL;

    if (!CRYPTO_THREAD_read_lock(dgbl->lock))
        return NULL;

    ret = dgbl->primary;
    seed = dgbl->seed;
    CRYPTO_THREAD_unlock(dgbl->lock);

    if (ret != NULL)
        return ret;

#if !defined(FIPS_MODULE) || !defined(OPENSSL_NO_FIPS_JITTER)
    /* Create a seed source for libcrypto or jitter enabled FIPS provider */
    if (seed == NULL) {
        ERR_set_mark();
        seed = newseed = rand_new_seed(ctx);
        ERR_pop_to_mark();
    }
#endif  /* !FIPS_MODULE || !OPENSSL_NO_FIPS_JITTER */

#if defined(FIPS_MODULE)
    /* The FIPS provider has entropy health tests instead of the primary */
    ret = rand_new_crngt(ctx, seed);
#else   /* FIPS_MODULE */
    ret = rand_new_drbg(ctx, seed, PRIMARY_RESEED_INTERVAL,
                        PRIMARY_RESEED_TIME_INTERVAL);
#endif  /* FIPS_MODULE */

    /*
     * The primary DRBG may be shared between multiple threads so we must
     * enable locking.
     */
    if (ret == NULL || !EVP_RAND_enable_locking(ret)) {
        if (ret != NULL) {
            ERR_raise(ERR_LIB_EVP, EVP_R_UNABLE_TO_ENABLE_LOCKING);
            EVP_RAND_CTX_free(ret);
        }
        if (newseed == NULL)
            return NULL;
        /* else carry on and store seed */
        ret = NULL;
    }

    if (!CRYPTO_THREAD_write_lock(dgbl->lock))
        return NULL;

    primary = dgbl->primary;
    if (primary != NULL) {
        CRYPTO_THREAD_unlock(dgbl->lock);
        EVP_RAND_CTX_free(ret);
        EVP_RAND_CTX_free(newseed);
        return primary;
    }
    if (newseed != NULL)
        dgbl->seed = newseed;
    dgbl->primary = ret;
    CRYPTO_THREAD_unlock(dgbl->lock);

    return ret;
}

/*
 * Get the primary random generator.
 * Returns pointer to its EVP_RAND_CTX on success, NULL on failure.
 *
 */
EVP_RAND_CTX *RAND_get0_primary(OSSL_LIB_CTX *ctx)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    return dgbl == NULL ? NULL : rand_get0_primary(ctx, dgbl);
}

static EVP_RAND_CTX *rand_get0_public(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl)
{
    EVP_RAND_CTX *rand, *primary;

    if (dgbl == NULL)
        return NULL;

    rand = CRYPTO_THREAD_get_local(&dgbl->public);
    if (rand == NULL) {
        primary = rand_get0_primary(ctx, dgbl);
        if (primary == NULL)
            return NULL;

        ctx = ossl_lib_ctx_get_concrete(ctx);

        if (ctx == NULL)
            return NULL;
        /*
         * If the private is also NULL then this is the first time we've
         * used this thread.
         */
        if (CRYPTO_THREAD_get_local(&dgbl->private) == NULL
                && !ossl_init_thread_start(NULL, ctx, rand_delete_thread_state))
            return NULL;
        rand = rand_new_drbg(ctx, primary, SECONDARY_RESEED_INTERVAL,
                             SECONDARY_RESEED_TIME_INTERVAL);
        CRYPTO_THREAD_set_local(&dgbl->public, rand);
    }
    return rand;
}

/*
 * Get the public random generator.
 * Returns pointer to its EVP_RAND_CTX on success, NULL on failure.
 */
EVP_RAND_CTX *RAND_get0_public(OSSL_LIB_CTX *ctx)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    return dgbl == NULL ? NULL : rand_get0_public(ctx, dgbl);
}

static EVP_RAND_CTX *rand_get0_private(OSSL_LIB_CTX *ctx, RAND_GLOBAL *dgbl)
{
    EVP_RAND_CTX *rand, *primary;

    rand = CRYPTO_THREAD_get_local(&dgbl->private);
    if (rand == NULL) {
        primary = rand_get0_primary(ctx, dgbl);
        if (primary == NULL)
            return NULL;

        ctx = ossl_lib_ctx_get_concrete(ctx);

        if (ctx == NULL)
            return NULL;
        /*
         * If the public is also NULL then this is the first time we've
         * used this thread.
         */
        if (CRYPTO_THREAD_get_local(&dgbl->public) == NULL
                && !ossl_init_thread_start(NULL, ctx, rand_delete_thread_state))
            return NULL;
        rand = rand_new_drbg(ctx, primary, SECONDARY_RESEED_INTERVAL,
                             SECONDARY_RESEED_TIME_INTERVAL);
        CRYPTO_THREAD_set_local(&dgbl->private, rand);
    }
    return rand;
}

/*
 * Get the private random generator.
 * Returns pointer to its EVP_RAND_CTX on success, NULL on failure.
 */
EVP_RAND_CTX *RAND_get0_private(OSSL_LIB_CTX *ctx)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    return dgbl == NULL ? NULL : rand_get0_private(ctx, dgbl);
}

#ifdef FIPS_MODULE
EVP_RAND_CTX *ossl_rand_get0_private_noncreating(OSSL_LIB_CTX *ctx)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return NULL;

    return CRYPTO_THREAD_get_local(&dgbl->private);
}
#endif

int RAND_set0_public(OSSL_LIB_CTX *ctx, EVP_RAND_CTX *rand)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);
    EVP_RAND_CTX *old;
    int r;

    if (dgbl == NULL)
        return 0;
    old = CRYPTO_THREAD_get_local(&dgbl->public);
    if ((r = CRYPTO_THREAD_set_local(&dgbl->public, rand)) > 0)
        EVP_RAND_CTX_free(old);
    return r;
}

int RAND_set0_private(OSSL_LIB_CTX *ctx, EVP_RAND_CTX *rand)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);
    EVP_RAND_CTX *old;
    int r;

    if (dgbl == NULL)
        return 0;
    old = CRYPTO_THREAD_get_local(&dgbl->private);
    if ((r = CRYPTO_THREAD_set_local(&dgbl->private, rand)) > 0)
        EVP_RAND_CTX_free(old);
    return r;
}

#ifndef FIPS_MODULE
static int random_set_string(char **p, const char *s)
{
    char *d = NULL;

    if (s != NULL) {
        d = OPENSSL_strdup(s);
        if (d == NULL)
            return 0;
    }
    OPENSSL_free(*p);
    *p = d;
    return 1;
}

/*
 * Load the DRBG definitions from a configuration file.
 */
static int random_conf_init(CONF_IMODULE *md, const CONF *cnf)
{
    STACK_OF(CONF_VALUE) *elist;
    CONF_VALUE *cval;
    OSSL_LIB_CTX *libctx = NCONF_get0_libctx((CONF *)cnf);
    RAND_GLOBAL *dgbl = rand_get_global(libctx);
    int i, r = 1;

    OSSL_TRACE1(CONF, "Loading random module: section %s\n",
                CONF_imodule_get_value(md));

    /* Value is a section containing RANDOM configuration */
    elist = NCONF_get_section(cnf, CONF_imodule_get_value(md));
    if (elist == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_RANDOM_SECTION_ERROR);
        return 0;
    }

    if (dgbl == NULL)
        return 0;

    for (i = 0; i < sk_CONF_VALUE_num(elist); i++) {
        cval = sk_CONF_VALUE_value(elist, i);
        if (OPENSSL_strcasecmp(cval->name, "random") == 0) {
            if (!random_set_string(&dgbl->rng_name, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "cipher") == 0) {
            if (!random_set_string(&dgbl->rng_cipher, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "digest") == 0) {
            if (!random_set_string(&dgbl->rng_digest, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "properties") == 0) {
            if (!random_set_string(&dgbl->rng_propq, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "seed") == 0) {
            if (!random_set_string(&dgbl->seed_name, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "seed_properties") == 0) {
            if (!random_set_string(&dgbl->seed_propq, cval->value))
                return 0;
        } else if (OPENSSL_strcasecmp(cval->name, "random_provider") == 0) {
# ifndef FIPS_MODULE
            OSSL_PROVIDER *prov = ossl_provider_find(libctx, cval->value, 0);

            if (prov != NULL) {
                if (!RAND_set1_random_provider(libctx, prov)) {
                    ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
                    OSSL_PROVIDER_unload(prov);
                    return 0;
                }
                /*
                 * We need to release the reference from ossl_provider_find because
                 * we don't want to keep a reference counted handle to the provider.
                 *
                 * The provider unload code checks for the random provider and,
                 * if present, our reference will be NULLed when it is fully freed.
                 * The provider load code, conversely, checks the provider name
                 * and re-hooks our reference if required.  This means that a load,
                 * hook random provider, use, unload, reload, reuse sequence will
                 * work as expected.
                 */
                OSSL_PROVIDER_unload(prov);
            } else if (!set_random_provider_name(dgbl, cval->value))
                return 0;
# endif
        } else {
            ERR_raise_data(ERR_LIB_CRYPTO,
                           CRYPTO_R_UNKNOWN_NAME_IN_RANDOM_SECTION,
                           "name=%s, value=%s", cval->name, cval->value);
            r = 0;
        }
    }
    return r;
}


static void random_conf_deinit(CONF_IMODULE *md)
{
    OSSL_TRACE(CONF, "Cleaned up random\n");
}

void ossl_random_add_conf_module(void)
{
    OSSL_TRACE(CONF, "Adding config module 'random'\n");
    CONF_module_add("random", random_conf_init, random_conf_deinit);
}

int RAND_set_DRBG_type(OSSL_LIB_CTX *ctx, const char *drbg, const char *propq,
                       const char *cipher, const char *digest)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return 0;
    if (dgbl->primary != NULL) {
        ERR_raise(ERR_LIB_CRYPTO, RAND_R_ALREADY_INSTANTIATED);
        return 0;
    }
    return random_set_string(&dgbl->rng_name, drbg)
        && random_set_string(&dgbl->rng_propq, propq)
        && random_set_string(&dgbl->rng_cipher, cipher)
        && random_set_string(&dgbl->rng_digest, digest);
}

int RAND_set_seed_source_type(OSSL_LIB_CTX *ctx, const char *seed,
                              const char *propq)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return 0;
    if (dgbl->seed != NULL) {
        ERR_raise(ERR_LIB_CRYPTO, RAND_R_ALREADY_INSTANTIATED);
        return 0;
    }
    return random_set_string(&dgbl->seed_name, seed)
        && random_set_string(&dgbl->seed_propq, propq);
}

int RAND_set1_random_provider(OSSL_LIB_CTX *ctx, OSSL_PROVIDER *prov)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return 0;

    if (prov == NULL) {
        OPENSSL_free(dgbl->random_provider_name);
        dgbl->random_provider_name = NULL;
        dgbl->random_provider = NULL;
        return 1;
    }

    if (dgbl->random_provider == prov)
        return 1;

    if (!set_random_provider_name(dgbl, OSSL_PROVIDER_get0_name(prov)))
        return 0;

    dgbl->random_provider = prov;
    return 1;
}

/*
 * When a new provider is loaded, we need to check to see if it is the
 * designated randomness provider and register it if it is.
 */
int ossl_rand_check_random_provider_on_load(OSSL_LIB_CTX *ctx,
                                            OSSL_PROVIDER *prov)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return 0;

    /* No random provider name specified, or one is installed already */
    if (dgbl->random_provider_name == NULL || dgbl->random_provider != NULL)
        return 1;

    /* Does this provider match the name we're using? */
    if (strcmp(dgbl->random_provider_name, OSSL_PROVIDER_get0_name(prov)) != 0)
        return 1;

    dgbl->random_provider = prov;
    return 1;
}

/*
 * When a provider is being unloaded, if it is the randomness provider,
 * we need to deregister it.
 */
int ossl_rand_check_random_provider_on_unload(OSSL_LIB_CTX *ctx,
                                              OSSL_PROVIDER *prov)
{
    RAND_GLOBAL *dgbl = rand_get_global(ctx);

    if (dgbl == NULL)
        return 0;

    if (dgbl->random_provider == prov)
        dgbl->random_provider = NULL;
    return 1;
}

#endif      /* !FIPS_MODULE */
