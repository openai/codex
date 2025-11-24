/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#ifndef OSSL_CRYPTO_RAND_H
# define OSSL_CRYPTO_RAND_H
# pragma once

# include <openssl/rand.h>
# include "crypto/rand_pool.h"

# if defined(__APPLE__) && !defined(OPENSSL_NO_APPLE_CRYPTO_RANDOM)
#  include <Availability.h>
#  if (defined(__MAC_OS_X_VERSION_MIN_REQUIRED) && __MAC_OS_X_VERSION_MIN_REQUIRED >= 101200) || \
     (defined(__IPHONE_OS_VERSION_MIN_REQUIRED) && __IPHONE_OS_VERSION_MIN_REQUIRED >= 80000)
#   define OPENSSL_APPLE_CRYPTO_RANDOM 1
#   include <CommonCrypto/CommonCryptoError.h>
#   include <CommonCrypto/CommonRandom.h>
#  endif
# endif

/*
 * Defines related to seed sources
 */
#ifndef DEVRANDOM
/*
 * set this to a comma-separated list of 'random' device files to try out. By
 * default, we will try to read at least one of these files
 */
# define DEVRANDOM "/dev/urandom", "/dev/random", "/dev/hwrng", "/dev/srandom"
# if defined(__linux) && !defined(__ANDROID__)
#  ifndef DEVRANDOM_WAIT
#   define DEVRANDOM_WAIT   "/dev/random"
#  endif
/*
 * Linux kernels 4.8 and later changes how their random device works and there
 * is no reliable way to tell that /dev/urandom has been seeded -- getentropy(2)
 * should be used instead.
 */
#  ifndef DEVRANDOM_SAFE_KERNEL
#   define DEVRANDOM_SAFE_KERNEL        4, 8
#  endif
/*
 * Some operating systems do not permit select(2) on their random devices,
 * defining this to zero will force the use of read(2) to extract one byte
 * from /dev/random.
 */
#  ifndef DEVRANDM_WAIT_USE_SELECT
#   define DEVRANDM_WAIT_USE_SELECT     1
#  endif
/*
 * Define the shared memory identifier used to indicate if the operating
 * system has properly seeded the DEVRANDOM source.
 */
#  ifndef OPENSSL_RAND_SEED_DEVRANDOM_SHM_ID
#   define OPENSSL_RAND_SEED_DEVRANDOM_SHM_ID 114
#  endif

# endif
#endif

#if !defined(OPENSSL_NO_EGD) && !defined(DEVRANDOM_EGD)
/*
 * set this to a comma-separated list of 'egd' sockets to try out. These
 * sockets will be tried in the order listed in case accessing the device
 * files listed in DEVRANDOM did not return enough randomness.
 */
# define DEVRANDOM_EGD "/var/run/egd-pool", "/dev/egd-pool", "/etc/egd-pool", "/etc/entropy"
#endif

void ossl_rand_cleanup_int(void);

/*
 * Initialise the random pool reseeding sources.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_rand_pool_init(void);

/*
 * Finalise the random pool reseeding sources.
 */
void ossl_rand_pool_cleanup(void);

/*
 * Control the random pool use of open file descriptors.
 */
void ossl_rand_pool_keep_random_devices_open(int keep);

/*
 * Configuration
 */
void ossl_random_add_conf_module(void);

/*
 * Get and cleanup random seed material.
 */
size_t ossl_rand_get_entropy(OSSL_LIB_CTX *ctx,
                             unsigned char **pout, int entropy,
                             size_t min_len, size_t max_len);
size_t ossl_rand_get_user_entropy(OSSL_LIB_CTX *ctx,
                                  unsigned char **pout, int entropy,
                                  size_t min_len, size_t max_len);
void ossl_rand_cleanup_entropy(OSSL_LIB_CTX *ctx,
                               unsigned char *buf, size_t len);
void ossl_rand_cleanup_user_entropy(OSSL_LIB_CTX *ctx,
                                    unsigned char *buf, size_t len);
size_t ossl_rand_get_nonce(OSSL_LIB_CTX *ctx,
                           unsigned char **pout, size_t min_len, size_t max_len,
                           const void *salt, size_t salt_len);
size_t ossl_rand_get_user_nonce(OSSL_LIB_CTX *ctx, unsigned char **pout,
                                size_t min_len, size_t max_len,
                                const void *salt, size_t salt_len);
void ossl_rand_cleanup_nonce(OSSL_LIB_CTX *ctx,
                             unsigned char *buf, size_t len);
void ossl_rand_cleanup_user_nonce(OSSL_LIB_CTX *ctx,
                                  unsigned char *buf, size_t len);

/*
 * Get seeding material from the operating system sources.
 */
size_t ossl_pool_acquire_entropy(RAND_POOL *pool);
int ossl_pool_add_nonce_data(RAND_POOL *pool);

# ifdef FIPS_MODULE
EVP_RAND_CTX *ossl_rand_get0_private_noncreating(OSSL_LIB_CTX *ctx);
# else
EVP_RAND_CTX *ossl_rand_get0_seed_noncreating(OSSL_LIB_CTX *ctx);
# endif

/* Generate a uniformly distributed random integer in the interval [0, upper) */
uint32_t ossl_rand_uniform_uint32(OSSL_LIB_CTX *ctx, uint32_t upper, int *err);

/*
 * Generate a uniformly distributed random integer in the interval
 * [lower, upper).
 */
uint32_t ossl_rand_range_uint32(OSSL_LIB_CTX *ctx, uint32_t lower, uint32_t upper,
                                int *err);

/*
 * Check if the named provider is the nominated entropy/random provider.
 * If it is, use it.
 */
# ifndef FIPS_MODULE
int ossl_rand_check_random_provider_on_load(OSSL_LIB_CTX *ctx,
                                            OSSL_PROVIDER *prov);
int ossl_rand_check_random_provider_on_unload(OSSL_LIB_CTX *ctx,
                                              OSSL_PROVIDER *prov);
# endif     /* FIPS_MODULE */

#endif
