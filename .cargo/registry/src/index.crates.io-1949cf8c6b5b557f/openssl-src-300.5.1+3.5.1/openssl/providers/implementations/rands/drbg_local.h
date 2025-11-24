/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_PROV_LOCAL_H
# define OSSL_CRYPTO_PROV_LOCAL_H

# include <openssl/evp.h>
# include <openssl/core_dispatch.h>
# include <openssl/core_names.h>
# include <openssl/params.h>
# include "internal/tsan_assist.h"
# include "internal/nelem.h"
# include "internal/numbers.h"
# include "prov/provider_ctx.h"
# include "prov/securitycheck.h"

/* How many times to read the TSC as a randomness source. */
# define TSC_READ_COUNT                 4

/* Maximum reseed intervals */
# define MAX_RESEED_INTERVAL                     (1 << 24)
# define MAX_RESEED_TIME_INTERVAL                (1 << 20) /* approx. 12 days */

/* Default reseed intervals */
# define RESEED_INTERVAL                         (1 << 8)
# define TIME_INTERVAL                           (60*60)   /* 1 hour */

/*
 * Maximum input size for the DRBG (entropy, nonce, personalization string)
 *
 * NIST SP800 90Ar1 allows a maximum of (1 << 35) bits i.e., (1 << 32) bytes.
 *
 * We lower it to 'only' INT32_MAX bytes, which is equivalent to 2 gigabytes.
 */
# define DRBG_MAX_LENGTH                         INT32_MAX

/* The default nonce */
/* ASCII: "OpenSSL NIST SP 800-90A DRBG", in hex for EBCDIC compatibility */
#define DRBG_DEFAULT_PERS_STRING "\x4f\x70\x65\x6e\x53\x53\x4c\x20\x4e\x49\x53\x54\x20\x53\x50\x20\x38\x30\x30\x2d\x39\x30\x41\x20\x44\x52\x42\x47"

typedef struct prov_drbg_st PROV_DRBG;

/* DRBG status values */
typedef enum drbg_status_e {
    DRBG_UNINITIALISED,
    DRBG_READY,
    DRBG_ERROR
} DRBG_STATUS;

/*
 * The state of all types of DRBGs.
 */
struct prov_drbg_st {
    CRYPTO_RWLOCK *lock;
    PROV_CTX *provctx;

    /* Virtual functions are cached here */
    int (*instantiate)(PROV_DRBG *drbg,
                       const unsigned char *entropy, size_t entropylen,
                       const unsigned char *nonce, size_t noncelen,
                       const unsigned char *pers, size_t perslen);
    int (*uninstantiate)(PROV_DRBG *ctx);
    int (*reseed)(PROV_DRBG *drbg, const unsigned char *ent, size_t ent_len,
                  const unsigned char *adin, size_t adin_len);
    int (*generate)(PROV_DRBG *, unsigned char *out, size_t outlen,
                    const unsigned char *adin, size_t adin_len);

    /* Parent PROV_RAND and its dispatch table functions */
    void *parent;
    OSSL_FUNC_rand_enable_locking_fn *parent_enable_locking;
    OSSL_FUNC_rand_lock_fn *parent_lock;
    OSSL_FUNC_rand_unlock_fn *parent_unlock;
    OSSL_FUNC_rand_get_ctx_params_fn *parent_get_ctx_params;
    OSSL_FUNC_rand_nonce_fn *parent_nonce;
    OSSL_FUNC_rand_get_seed_fn *parent_get_seed;
    OSSL_FUNC_rand_clear_seed_fn *parent_clear_seed;

    /*
     * Stores the return value of openssl_get_fork_id() as of when we last
     * reseeded.  The DRBG reseeds automatically whenever drbg->fork_id !=
     * openssl_get_fork_id().  Used to provide fork-safety and reseed this
     * DRBG in the child process.
     */
    int fork_id;
    unsigned short flags; /* various external flags */

    /*
     * The following parameters are setup by the per-type "init" function.
     *
     * The supported types and their init functions are:
     *    (1) CTR_DRBG:  drbg_ctr_init().
     *    (2) HMAC_DRBG: drbg_hmac_init().
     *    (3) HASH_DRBG: drbg_hash_init().
     *
     * The parameters are closely related to the ones described in
     * section '10.2.1 CTR_DRBG' of [NIST SP 800-90Ar1], with one
     * crucial difference: In the NIST standard, all counts are given
     * in bits, whereas in OpenSSL entropy counts are given in bits
     * and buffer lengths are given in bytes.
     *
     * Since this difference has lead to some confusion in the past,
     * (see [GitHub Issue #2443], formerly [rt.openssl.org #4055])
     * the 'len' suffix has been added to all buffer sizes for
     * clarification.
     */

    unsigned int strength;
    size_t max_request;
    size_t min_entropylen, max_entropylen;
    size_t min_noncelen, max_noncelen;
    size_t max_perslen, max_adinlen;

    /*
     * Counts the number of generate requests since the last reseed
     * (Starts at 1). This value is the reseed_counter as defined in
     * NIST SP 800-90Ar1
     */
    unsigned int generate_counter;
    /*
     * Maximum number of generate requests until a reseed is required.
     * This value is ignored if it is zero.
     */
    unsigned int reseed_interval;
    /* Stores the time when the last reseeding occurred */
    time_t reseed_time;
    /*
     * Specifies the maximum time interval (in seconds) between reseeds.
     * This value is ignored if it is zero.
     */
    time_t reseed_time_interval;
    /*
     * Counts the number of reseeds since instantiation.
     * This value is ignored if it is zero.
     *
     * This counter is used only for seed propagation from the <master> DRBG
     * to its two children, the <public> and <private> DRBG. This feature is
     * very special and its sole purpose is to ensure that any randomness which
     * is added by PROV_add() or PROV_seed() will have an immediate effect on
     * the output of PROV_bytes() resp. PROV_priv_bytes().
     */
    TSAN_QUALIFIER unsigned int reseed_counter;
    unsigned int reseed_next_counter;
    unsigned int parent_reseed_counter;

    size_t seedlen;
    DRBG_STATUS state;

    /* DRBG specific data */
    void *data;

    /* Entropy and nonce gathering callbacks */
    void *callback_arg;
    OSSL_INOUT_CALLBACK *get_entropy_fn;
    OSSL_CALLBACK *cleanup_entropy_fn;
    OSSL_INOUT_CALLBACK *get_nonce_fn;
    OSSL_CALLBACK *cleanup_nonce_fn;

    OSSL_FIPS_IND_DECLARE
};

PROV_DRBG *ossl_rand_drbg_new
    (void *provctx, void *parent, const OSSL_DISPATCH *parent_dispatch,
     int (*dnew)(PROV_DRBG *ctx),
     void (*dfree)(void *vctx),
     int (*instantiate)(PROV_DRBG *drbg,
                        const unsigned char *entropy, size_t entropylen,
                        const unsigned char *nonce, size_t noncelen,
                        const unsigned char *pers, size_t perslen),
     int (*uninstantiate)(PROV_DRBG *ctx),
     int (*reseed)(PROV_DRBG *drbg, const unsigned char *ent, size_t ent_len,
                   const unsigned char *adin, size_t adin_len),
     int (*generate)(PROV_DRBG *, unsigned char *out, size_t outlen,
                     const unsigned char *adin, size_t adin_len));
void ossl_rand_drbg_free(PROV_DRBG *drbg);

int ossl_prov_drbg_instantiate(PROV_DRBG *drbg, unsigned int strength,
                               int prediction_resistance,
                               const unsigned char *pers, size_t perslen);

int ossl_prov_drbg_uninstantiate(PROV_DRBG *drbg);

int ossl_prov_drbg_reseed(PROV_DRBG *drbg, int prediction_resistance,
                          const unsigned char *ent, size_t ent_len,
                          const unsigned char *adin, size_t adinlen);

int ossl_prov_drbg_generate(PROV_DRBG *drbg, unsigned char *out, size_t outlen,
                            unsigned int strength, int prediction_resistance,
                            const unsigned char *adin, size_t adinlen);

/* Seeding api */
OSSL_FUNC_rand_get_seed_fn ossl_drbg_get_seed;
OSSL_FUNC_rand_clear_seed_fn ossl_drbg_clear_seed;

/* Verify that an array of numeric values is all zero */
#define PROV_DRBG_VERIFY_ZEROIZATION(v)     \
    {                                       \
        size_t i;                           \
                                            \
        for (i = 0; i < OSSL_NELEM(v); i++) \
            if ((v)[i] != 0)                \
                goto err;                   \
    }

/* locking api */
OSSL_FUNC_rand_enable_locking_fn ossl_drbg_enable_locking;
OSSL_FUNC_rand_lock_fn ossl_drbg_lock;
OSSL_FUNC_rand_unlock_fn ossl_drbg_unlock;

/* Common parameters for all of our DRBGs */
int ossl_drbg_get_ctx_params(PROV_DRBG *drbg, OSSL_PARAM params[]);
int ossl_drbg_get_ctx_params_no_lock(PROV_DRBG *drbg, OSSL_PARAM params[],
                                     int *complete);
int ossl_drbg_set_ctx_params(PROV_DRBG *drbg, const OSSL_PARAM params[]);

#define OSSL_PARAM_DRBG_SETTABLE_CTX_COMMON                             \
    OSSL_PARAM_uint(OSSL_DRBG_PARAM_RESEED_REQUESTS, NULL),             \
    OSSL_PARAM_uint64(OSSL_DRBG_PARAM_RESEED_TIME_INTERVAL, NULL)

#define OSSL_PARAM_DRBG_GETTABLE_CTX_COMMON                             \
    OSSL_PARAM_int(OSSL_RAND_PARAM_STATE, NULL),                        \
    OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),                    \
    OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),               \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MIN_ENTROPYLEN, NULL),            \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MAX_ENTROPYLEN, NULL),            \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MIN_NONCELEN, NULL),              \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MAX_NONCELEN, NULL),              \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MAX_PERSLEN, NULL),               \
    OSSL_PARAM_size_t(OSSL_DRBG_PARAM_MAX_ADINLEN, NULL),               \
    OSSL_PARAM_uint(OSSL_DRBG_PARAM_RESEED_COUNTER, NULL),              \
    OSSL_PARAM_time_t(OSSL_DRBG_PARAM_RESEED_TIME, NULL),               \
    OSSL_PARAM_uint(OSSL_DRBG_PARAM_RESEED_REQUESTS, NULL),             \
    OSSL_PARAM_uint64(OSSL_DRBG_PARAM_RESEED_TIME_INTERVAL, NULL)

/* Confirm digest is allowed to be used with a DRBG */
int ossl_drbg_verify_digest(PROV_DRBG *drbg, OSSL_LIB_CTX *libctx, const EVP_MD *md);

#endif
