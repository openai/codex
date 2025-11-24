/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/evp.h>
#include <openssl/params.h>
#include <openssl/crypto.h>
#include "internal/cryptlib.h"
#include <openssl/fipskey.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/rand.h>
#include "internal/e_os.h"
#include "internal/fips.h"
#include "internal/tsan_assist.h"
#include "prov/providercommon.h"
#include "crypto/rand.h"

/*
 * We're cheating here. Normally we don't allow RUN_ONCE usage inside the FIPS
 * module because all such initialisation should be associated with an
 * individual OSSL_LIB_CTX. That doesn't work with the self test though because
 * it should be run once regardless of the number of OSSL_LIB_CTXs we have.
 */
#define ALLOW_RUN_ONCE_IN_FIPS
#include "internal/thread_once.h"
#include "self_test.h"

#define FIPS_STATE_INIT     0
#define FIPS_STATE_SELFTEST 1
#define FIPS_STATE_RUNNING  2
#define FIPS_STATE_ERROR    3

/*
 * The number of times the module will report it is in the error state
 * before going quiet.
 */
#define FIPS_ERROR_REPORTING_RATE_LIMIT     10

/* The size of a temp buffer used to read in data */
#define INTEGRITY_BUF_SIZE (4096)
#define MAX_MD_SIZE 64
#define MAC_NAME    "HMAC"
#define DIGEST_NAME "SHA256"

static int FIPS_conditional_error_check = 1;
static CRYPTO_RWLOCK *self_test_lock = NULL;

static CRYPTO_ONCE fips_self_test_init = CRYPTO_ONCE_STATIC_INIT;
#if !defined(OPENSSL_NO_FIPS_POST)
static unsigned char fixed_key[32] = { FIPS_KEY_ELEMENTS };
#endif

DEFINE_RUN_ONCE_STATIC(do_fips_self_test_init)
{
    /*
     * These locks get freed in platform specific ways that may occur after we
     * do mem leak checking. If we don't know how to free it for a particular
     * platform then we just leak it deliberately.
     */
    self_test_lock = CRYPTO_THREAD_lock_new();
    return self_test_lock != NULL;
}

/*
 * Declarations for the DEP entry/exit points.
 * Ones not required or incorrect need to be undefined or redefined respectively.
 */
#define DEP_INITIAL_STATE   FIPS_STATE_INIT
#define DEP_INIT_ATTRIBUTE  static
#define DEP_FINI_ATTRIBUTE  static

static void init(void);
static void cleanup(void);

/*
 * This is the Default Entry Point (DEP) code.
 * See FIPS 140-2 IG 9.10
 */
#if defined(_WIN32) || defined(__CYGWIN__)
# ifdef __CYGWIN__
/* pick DLL_[PROCESS|THREAD]_[ATTACH|DETACH] definitions */
#  include <windows.h>
/*
 * this has side-effect of _WIN32 getting defined, which otherwise is
 * mutually exclusive with __CYGWIN__...
 */
# endif

BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved);
BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved)
{
    switch (fdwReason) {
    case DLL_PROCESS_ATTACH:
        init();
        break;
    case DLL_PROCESS_DETACH:
        cleanup();
        break;
    default:
        break;
    }
    return TRUE;
}

#elif defined(__GNUC__) && !defined(_AIX)
# undef DEP_INIT_ATTRIBUTE
# undef DEP_FINI_ATTRIBUTE
# define DEP_INIT_ATTRIBUTE static __attribute__((constructor))
# define DEP_FINI_ATTRIBUTE static __attribute__((destructor))

#elif defined(__sun)
# pragma init(init)
# pragma fini(cleanup)

#elif defined(_AIX) && !defined(__GNUC__)
void _init(void);
void _cleanup(void);
# pragma init(_init)
# pragma fini(_cleanup)
void _init(void)
{
    init();
}
void _cleanup(void)
{
    cleanup();
}

#elif defined(__hpux)
# pragma init "init"
# pragma fini "cleanup"

#elif defined(__TANDEM)
/* Method automatically called by the NonStop OS when the DLL loads */
void __INIT__init(void) {
    init();
}

/* Method automatically called by the NonStop OS prior to unloading the DLL */
void __TERM__cleanup(void) {
    cleanup();
}

#else
/*
 * This build does not support any kind of DEP.
 * We force the self-tests to run as part of the FIPS provider initialisation
 * rather than being triggered by the DEP.
 */
# undef DEP_INIT_ATTRIBUTE
# undef DEP_FINI_ATTRIBUTE
# undef DEP_INITIAL_STATE
# define DEP_INITIAL_STATE  FIPS_STATE_SELFTEST
#endif

static TSAN_QUALIFIER int FIPS_state = DEP_INITIAL_STATE;

#if defined(DEP_INIT_ATTRIBUTE)
DEP_INIT_ATTRIBUTE void init(void)
{
    tsan_store(&FIPS_state, FIPS_STATE_SELFTEST);
}
#endif

#if defined(DEP_FINI_ATTRIBUTE)
DEP_FINI_ATTRIBUTE void cleanup(void)
{
    CRYPTO_THREAD_lock_free(self_test_lock);
}
#endif

#if !defined(OPENSSL_NO_FIPS_POST)
/*
 * We need an explicit HMAC-SHA-256 KAT even though it is also
 * checked as part of the KDF KATs.  Refer IG 10.3.
 */
static const unsigned char hmac_kat_pt[] = {
    0xdd, 0x0c, 0x30, 0x33, 0x35, 0xf9, 0xe4, 0x2e,
    0xc2, 0xef, 0xcc, 0xbf, 0x07, 0x95, 0xee, 0xa2
};
static const unsigned char hmac_kat_key[] = {
    0xf4, 0x55, 0x66, 0x50, 0xac, 0x31, 0xd3, 0x54,
    0x61, 0x61, 0x0b, 0xac, 0x4e, 0xd8, 0x1b, 0x1a,
    0x18, 0x1b, 0x2d, 0x8a, 0x43, 0xea, 0x28, 0x54,
    0xcb, 0xae, 0x22, 0xca, 0x74, 0x56, 0x08, 0x13
};
static const unsigned char hmac_kat_digest[] = {
    0xf5, 0xf5, 0xe5, 0xf2, 0x66, 0x49, 0xe2, 0x40,
    0xfc, 0x9e, 0x85, 0x7f, 0x2b, 0x9a, 0xbe, 0x28,
    0x20, 0x12, 0x00, 0x92, 0x82, 0x21, 0x3e, 0x51,
    0x44, 0x5d, 0xe3, 0x31, 0x04, 0x01, 0x72, 0x6b
};

static int integrity_self_test(OSSL_SELF_TEST *ev, OSSL_LIB_CTX *libctx)
{
    int ok = 0;
    unsigned char out[EVP_MAX_MD_SIZE];
    size_t out_len = 0;

    OSSL_PARAM   params[2];
    EVP_MAC     *mac = EVP_MAC_fetch(libctx, MAC_NAME, NULL);
    EVP_MAC_CTX *ctx = EVP_MAC_CTX_new(mac);

    OSSL_SELF_TEST_onbegin(ev, OSSL_SELF_TEST_TYPE_KAT_INTEGRITY,
                               OSSL_SELF_TEST_DESC_INTEGRITY_HMAC);

    params[0] = OSSL_PARAM_construct_utf8_string("digest", DIGEST_NAME, 0);
    params[1] = OSSL_PARAM_construct_end();

    if (ctx == NULL
            || mac == NULL
            || !EVP_MAC_init(ctx, hmac_kat_key, sizeof(hmac_kat_key), params)
            || !EVP_MAC_update(ctx, hmac_kat_pt, sizeof(hmac_kat_pt))
            || !EVP_MAC_final(ctx, out, &out_len, MAX_MD_SIZE))
        goto err;

    /* Optional corruption */
    OSSL_SELF_TEST_oncorrupt_byte(ev, out);

    if (out_len != sizeof(hmac_kat_digest)
            || memcmp(out, hmac_kat_digest, out_len) != 0)
        goto err;
    ok = 1;
err:
    OSSL_SELF_TEST_onend(ev, ok);
    EVP_MAC_free(mac);
    EVP_MAC_CTX_free(ctx);
    return ok;
}

/*
 * Calculate the HMAC SHA256 of data read using a BIO and read_cb, and verify
 * the result matches the expected value.
 * Return 1 if verified, or 0 if it fails.
 */
static int verify_integrity(OSSL_CORE_BIO *bio, OSSL_FUNC_BIO_read_ex_fn read_ex_cb,
                            unsigned char *expected, size_t expected_len,
                            OSSL_LIB_CTX *libctx, OSSL_SELF_TEST *ev,
                            const char *event_type)
{
    int ret = 0, status;
    unsigned char out[MAX_MD_SIZE];
    unsigned char buf[INTEGRITY_BUF_SIZE];
    size_t bytes_read = 0, out_len = 0;
    EVP_MAC *mac = NULL;
    EVP_MAC_CTX *ctx = NULL;
    OSSL_PARAM params[2], *p = params;

    if (!integrity_self_test(ev, libctx))
        goto err;

    OSSL_SELF_TEST_onbegin(ev, event_type, OSSL_SELF_TEST_DESC_INTEGRITY_HMAC);

    mac = EVP_MAC_fetch(libctx, MAC_NAME, NULL);
    if (mac == NULL)
        goto err;
    ctx = EVP_MAC_CTX_new(mac);
    if (ctx == NULL)
        goto err;

    *p++ = OSSL_PARAM_construct_utf8_string("digest", DIGEST_NAME, 0);
    *p = OSSL_PARAM_construct_end();

    if (!EVP_MAC_init(ctx, fixed_key, sizeof(fixed_key), params))
        goto err;

    while (1) {
        status = read_ex_cb(bio, buf, sizeof(buf), &bytes_read);
        if (status != 1)
            break;
        if (!EVP_MAC_update(ctx, buf, bytes_read))
            goto err;
    }
    if (!EVP_MAC_final(ctx, out, &out_len, sizeof(out)))
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(ev, out);
    if (expected_len != out_len
            || memcmp(expected, out, out_len) != 0)
        goto err;
    ret = 1;
err:
    OSSL_SELF_TEST_onend(ev, ret);
    EVP_MAC_CTX_free(ctx);
    EVP_MAC_free(mac);
# ifdef OPENSSL_PEDANTIC_ZEROIZATION
    OPENSSL_cleanse(out, sizeof(out));
# endif
    return ret;
}
#endif /* OPENSSL_NO_FIPS_POST */

static void set_fips_state(int state)
{
    tsan_store(&FIPS_state, state);
}

/* Return 1 if the FIPS self tests are running and 0 otherwise */
int ossl_fips_self_testing(void)
{
    return tsan_load(&FIPS_state) == FIPS_STATE_SELFTEST;
}

/* This API is triggered either on loading of the FIPS module or on demand */
int SELF_TEST_post(SELF_TEST_POST_PARAMS *st, int on_demand_test)
{
    int loclstate;
#if !defined(OPENSSL_NO_FIPS_POST)
    int ok = 0;
    long checksum_len;
    OSSL_CORE_BIO *bio_module = NULL;
    unsigned char *module_checksum = NULL;
    OSSL_SELF_TEST *ev = NULL;
    EVP_RAND *testrand = NULL;
    EVP_RAND_CTX *rng;
#endif

    if (!RUN_ONCE(&fips_self_test_init, do_fips_self_test_init))
        return 0;

    loclstate = tsan_load(&FIPS_state);

    if (loclstate == FIPS_STATE_RUNNING) {
        if (!on_demand_test)
            return 1;
    } else if (loclstate != FIPS_STATE_SELFTEST) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_STATE);
        return 0;
    }

    if (!CRYPTO_THREAD_write_lock(self_test_lock))
        return 0;

#if !defined(OPENSSL_NO_FIPS_POST)
    loclstate = tsan_load(&FIPS_state);
    if (loclstate == FIPS_STATE_RUNNING) {
        if (!on_demand_test) {
            CRYPTO_THREAD_unlock(self_test_lock);
            return 1;
        }
        set_fips_state(FIPS_STATE_SELFTEST);
    } else if (loclstate != FIPS_STATE_SELFTEST) {
        CRYPTO_THREAD_unlock(self_test_lock);
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_STATE);
        return 0;
    }

    if (st == NULL
            || st->module_checksum_data == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CONFIG_DATA);
        goto end;
    }

    ev = OSSL_SELF_TEST_new(st->cb, st->cb_arg);
    if (ev == NULL)
        goto end;

    module_checksum = OPENSSL_hexstr2buf(st->module_checksum_data,
                                         &checksum_len);
    if (module_checksum == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CONFIG_DATA);
        goto end;
    }
    bio_module = (*st->bio_new_file_cb)(st->module_filename, "rb");

    /* Always check the integrity of the fips module */
    if (bio_module == NULL
            || !verify_integrity(bio_module, st->bio_read_ex_cb,
                                 module_checksum, checksum_len, st->libctx,
                                 ev, OSSL_SELF_TEST_TYPE_MODULE_INTEGRITY)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MODULE_INTEGRITY_FAILURE);
        goto end;
    }

    if (!SELF_TEST_kats(ev, st->libctx)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_SELF_TEST_KAT_FAILURE);
        goto end;
    }

    /* Verify that the RNG has been restored properly */
    rng = ossl_rand_get0_private_noncreating(st->libctx);
    if (rng != NULL)
        if ((testrand = EVP_RAND_fetch(st->libctx, "TEST-RAND", NULL)) == NULL
                || strcmp(EVP_RAND_get0_name(EVP_RAND_CTX_get0_rand(rng)),
                          EVP_RAND_get0_name(testrand)) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_SELF_TEST_KAT_FAILURE);
            goto end;
        }

    ok = 1;
end:
    EVP_RAND_free(testrand);
    OSSL_SELF_TEST_free(ev);
    OPENSSL_free(module_checksum);

    if (st != NULL)
        (*st->bio_free_cb)(bio_module);

    if (ok)
        set_fips_state(FIPS_STATE_RUNNING);
    else
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_NONE);
    CRYPTO_THREAD_unlock(self_test_lock);

    return ok;
#else
    set_fips_state(FIPS_STATE_RUNNING);
    CRYPTO_THREAD_unlock(self_test_lock);
    return 1;
#endif /* !defined(OPENSSL_NO_FIPS_POST) */
}

void SELF_TEST_disable_conditional_error_state(void)
{
    FIPS_conditional_error_check = 0;
}

void ossl_set_error_state(const char *type)
{
    int cond_test = (type != NULL && strcmp(type, OSSL_SELF_TEST_TYPE_PCT) == 0);

    if (!cond_test || (FIPS_conditional_error_check == 1)) {
        set_fips_state(FIPS_STATE_ERROR);
        ERR_raise(ERR_LIB_PROV, PROV_R_FIPS_MODULE_ENTERING_ERROR_STATE);
    } else {
        ERR_raise(ERR_LIB_PROV, PROV_R_FIPS_MODULE_CONDITIONAL_ERROR);
    }
}

int ossl_prov_is_running(void)
{
    int res, loclstate;
    static TSAN_QUALIFIER unsigned int rate_limit = 0;

    loclstate = tsan_load(&FIPS_state);
    res = loclstate == FIPS_STATE_RUNNING || loclstate == FIPS_STATE_SELFTEST;
    if (loclstate == FIPS_STATE_ERROR)
        if (tsan_counter(&rate_limit) < FIPS_ERROR_REPORTING_RATE_LIMIT)
            ERR_raise(ERR_LIB_PROV, PROV_R_FIPS_MODULE_IN_ERROR_STATE);
    return res;
}
