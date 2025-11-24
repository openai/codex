/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/fips_names.h>
#include <openssl/fipskey.h>
#include <openssl/rand.h> /* RAND_get0_public() */
#include <openssl/proverr.h>
#include <openssl/indicator.h>
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "prov/implementations.h"
#include "prov/names.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/provider_util.h"
#include "prov/seeding.h"
#include "internal/nelem.h"
#include "self_test.h"
#include "crypto/context.h"
#include "fipscommon.h"
#include "internal/core.h"

static const char FIPS_DEFAULT_PROPERTIES[] = "provider=fips,fips=yes";
static const char FIPS_UNAPPROVED_PROPERTIES[] = "provider=fips,fips=no";

/*
 * Forward declarations to ensure that interface functions are correctly
 * defined.
 */
static OSSL_FUNC_provider_teardown_fn fips_teardown;
static OSSL_FUNC_provider_gettable_params_fn fips_gettable_params;
static OSSL_FUNC_provider_get_params_fn fips_get_params;
static OSSL_FUNC_provider_query_operation_fn fips_query;
static OSSL_FUNC_provider_query_operation_fn fips_query_internal;
static OSSL_FUNC_provider_random_bytes_fn fips_random_bytes;

#define ALGC(NAMES, FUNC, CHECK)                \
    { { NAMES, FIPS_DEFAULT_PROPERTIES, FUNC }, CHECK }
#define ALG(NAMES, FUNC) ALGC(NAMES, FUNC, NULL)

extern OSSL_FUNC_core_thread_start_fn *c_thread_start;

/*
 * Should these function pointers be stored in the provider side provctx? Could
 * they ever be different from one init to the next? We assume not for now.
 */

/* Functions provided by the core */
static OSSL_FUNC_core_gettable_params_fn *c_gettable_params;
static OSSL_FUNC_core_get_params_fn *c_get_params;
OSSL_FUNC_core_thread_start_fn *c_thread_start;
static OSSL_FUNC_core_new_error_fn *c_new_error;
static OSSL_FUNC_core_set_error_debug_fn *c_set_error_debug;
static OSSL_FUNC_core_vset_error_fn *c_vset_error;
static OSSL_FUNC_core_set_error_mark_fn *c_set_error_mark;
static OSSL_FUNC_core_clear_last_error_mark_fn *c_clear_last_error_mark;
static OSSL_FUNC_core_pop_error_to_mark_fn *c_pop_error_to_mark;
static OSSL_FUNC_CRYPTO_malloc_fn *c_CRYPTO_malloc;
static OSSL_FUNC_CRYPTO_zalloc_fn *c_CRYPTO_zalloc;
static OSSL_FUNC_CRYPTO_free_fn *c_CRYPTO_free;
static OSSL_FUNC_CRYPTO_clear_free_fn *c_CRYPTO_clear_free;
static OSSL_FUNC_CRYPTO_realloc_fn *c_CRYPTO_realloc;
static OSSL_FUNC_CRYPTO_clear_realloc_fn *c_CRYPTO_clear_realloc;
static OSSL_FUNC_CRYPTO_secure_malloc_fn *c_CRYPTO_secure_malloc;
static OSSL_FUNC_CRYPTO_secure_zalloc_fn *c_CRYPTO_secure_zalloc;
static OSSL_FUNC_CRYPTO_secure_free_fn *c_CRYPTO_secure_free;
static OSSL_FUNC_CRYPTO_secure_clear_free_fn *c_CRYPTO_secure_clear_free;
static OSSL_FUNC_CRYPTO_secure_allocated_fn *c_CRYPTO_secure_allocated;
static OSSL_FUNC_BIO_vsnprintf_fn *c_BIO_vsnprintf;
static OSSL_FUNC_self_test_cb_fn *c_stcbfn = NULL;
static OSSL_FUNC_indicator_cb_fn *c_indcbfn = NULL;
static OSSL_FUNC_core_get_libctx_fn *c_get_libctx = NULL;

typedef struct {
    const char *option;
    unsigned char enabled;
} FIPS_OPTION;

typedef struct fips_global_st {
    const OSSL_CORE_HANDLE *handle;
    SELF_TEST_POST_PARAMS selftest_params;

#define OSSL_FIPS_PARAM(structname, paramname, initvalue) \
    FIPS_OPTION fips_##structname;
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

} FIPS_GLOBAL;

static void init_fips_option(FIPS_OPTION *opt, int enabled)
{
    opt->enabled = enabled;
    opt->option = enabled ? "1" : "0";
}

void *ossl_fips_prov_ossl_ctx_new(OSSL_LIB_CTX *libctx)
{
    FIPS_GLOBAL *fgbl = OPENSSL_zalloc(sizeof(*fgbl));

    if (fgbl == NULL)
        return NULL;

#define OSSL_FIPS_PARAM(structname, paramname, initvalue) \
    init_fips_option(&fgbl->fips_##structname, initvalue);
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

    return fgbl;
}

void ossl_fips_prov_ossl_ctx_free(void *fgbl)
{
    OPENSSL_free(fgbl);
}

static int fips_random_bytes(ossl_unused void *vprov, int which,
                             void *buf, size_t n, unsigned int strength)
{
    OSSL_LIB_CTX *libctx;
    PROV_CTX *prov = (PROV_CTX *)vprov;

    if (prov == NULL)
        return 0;
    libctx = ossl_prov_ctx_get0_libctx(prov);
    if (which == OSSL_PROV_RANDOM_PRIVATE)
        return RAND_priv_bytes_ex(libctx, buf, n, strength);
    return RAND_bytes_ex(libctx, buf, n, strength);
}

/*
 * Parameters to retrieve from the core provider
 * NOTE: inside core_get_params() these will be loaded from config items
 * stored inside prov->parameters
 */
static int fips_get_params_from_core(FIPS_GLOBAL *fgbl)
{
    OSSL_PARAM core_params[32], *p = core_params;

#define OSSL_FIPS_PARAM(structname, paramname)                                 \
    *p++ = OSSL_PARAM_construct_utf8_ptr(                                      \
            paramname, (char **)&fgbl->selftest_params.structname,             \
            sizeof(fgbl->selftest_params.structname));

/* Parameters required for self testing */
#include "fips_selftest_params.inc"
#undef OSSL_FIPS_PARAM

/* FIPS indicator options can be enabled or disabled independently */
#define OSSL_FIPS_PARAM(structname, paramname, initvalue)                      \
    *p++ = OSSL_PARAM_construct_utf8_ptr(                                      \
            OSSL_PROV_PARAM_##paramname,                                       \
            (char **)&fgbl->fips_##structname.option,                          \
            sizeof(fgbl->fips_##structname.option));
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

    *p = OSSL_PARAM_construct_end();

    if (!c_get_params(fgbl->handle, core_params)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }

    return 1;
}

static const OSSL_PARAM *fips_gettable_params(void *provctx)
{
    /* Parameters we provide to the core */
    static const OSSL_PARAM fips_param_types[] = {
        OSSL_PARAM_DEFN(OSSL_PROV_PARAM_NAME, OSSL_PARAM_UTF8_PTR, NULL, 0),
        OSSL_PARAM_DEFN(OSSL_PROV_PARAM_VERSION, OSSL_PARAM_UTF8_PTR, NULL, 0),
        OSSL_PARAM_DEFN(OSSL_PROV_PARAM_BUILDINFO, OSSL_PARAM_UTF8_PTR, NULL, 0),
        OSSL_PARAM_DEFN(OSSL_PROV_PARAM_STATUS, OSSL_PARAM_INTEGER, NULL, 0),

#define OSSL_FIPS_PARAM(structname, paramname, initvalue) \
        OSSL_PARAM_DEFN(OSSL_PROV_PARAM_##paramname, OSSL_PARAM_INTEGER, NULL, 0),
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

        OSSL_PARAM_END
    };
    return fips_param_types;
}

static int fips_get_params(void *provctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;
    FIPS_GLOBAL *fgbl = ossl_lib_ctx_get_data(ossl_prov_ctx_get0_libctx(provctx),
                                              OSSL_LIB_CTX_FIPS_PROV_INDEX);

    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_NAME);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, FIPS_VENDOR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_VERSION);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_BUILDINFO);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_FULL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_STATUS);
    if (p != NULL && !OSSL_PARAM_set_int(p, ossl_prov_is_running()))
        return 0;

#define OSSL_FIPS_PARAM(structname, paramname, initvalue)                      \
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_##paramname);                \
    if (p != NULL && !OSSL_PARAM_set_int(p, fgbl->fips_##structname.enabled))  \
        return 0;
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

    return 1;
}

static void set_self_test_cb(FIPS_GLOBAL *fgbl)
{
    const OSSL_CORE_HANDLE *handle =
        FIPS_get_core_handle(fgbl->selftest_params.libctx);

    if (c_stcbfn != NULL && c_get_libctx != NULL) {
        c_stcbfn(c_get_libctx(handle), &fgbl->selftest_params.cb,
                              &fgbl->selftest_params.cb_arg);
    } else {
        fgbl->selftest_params.cb = NULL;
        fgbl->selftest_params.cb_arg = NULL;
    }
}

static int fips_self_test(void *provctx)
{
    FIPS_GLOBAL *fgbl = ossl_lib_ctx_get_data(ossl_prov_ctx_get0_libctx(provctx),
                                              OSSL_LIB_CTX_FIPS_PROV_INDEX);

    set_self_test_cb(fgbl);
    return SELF_TEST_post(&fgbl->selftest_params, 1) ? 1 : 0;
}

/*
 * For the algorithm names, we use the following formula for our primary
 * names:
 *
 *     ALGNAME[VERSION?][-SUBNAME[VERSION?]?][-SIZE?][-MODE?]
 *
 *     VERSION is only present if there are multiple versions of
 *     an alg (MD2, MD4, MD5).  It may be omitted if there is only
 *     one version (if a subsequent version is released in the future,
 *     we can always change the canonical name, and add the old name
 *     as an alias).
 *
 *     SUBNAME may be present where we are combining multiple
 *     algorithms together, e.g. MD5-SHA1.
 *
 *     SIZE is only present if multiple versions of an algorithm exist
 *     with different sizes (e.g. AES-128-CBC, AES-256-CBC)
 *
 *     MODE is only present where applicable.
 *
 * We add diverse other names where applicable, such as the names that
 * NIST uses, or that are used for ASN.1 OBJECT IDENTIFIERs, or names
 * we have used historically.
 */
static const OSSL_ALGORITHM fips_digests[] = {
    /* Our primary name:NiST name[:our older names] */
    { PROV_NAMES_SHA1, FIPS_DEFAULT_PROPERTIES, ossl_sha1_functions },
    { PROV_NAMES_SHA2_224, FIPS_DEFAULT_PROPERTIES, ossl_sha224_functions },
    { PROV_NAMES_SHA2_256, FIPS_DEFAULT_PROPERTIES, ossl_sha256_functions },
    { PROV_NAMES_SHA2_384, FIPS_DEFAULT_PROPERTIES, ossl_sha384_functions },
    { PROV_NAMES_SHA2_512, FIPS_DEFAULT_PROPERTIES, ossl_sha512_functions },
    { PROV_NAMES_SHA2_512_224, FIPS_DEFAULT_PROPERTIES,
      ossl_sha512_224_functions },
    { PROV_NAMES_SHA2_512_256, FIPS_DEFAULT_PROPERTIES,
      ossl_sha512_256_functions },

    /* We agree with NIST here, so one name only */
    { PROV_NAMES_SHA3_224, FIPS_DEFAULT_PROPERTIES, ossl_sha3_224_functions },
    { PROV_NAMES_SHA3_256, FIPS_DEFAULT_PROPERTIES, ossl_sha3_256_functions },
    { PROV_NAMES_SHA3_384, FIPS_DEFAULT_PROPERTIES, ossl_sha3_384_functions },
    { PROV_NAMES_SHA3_512, FIPS_DEFAULT_PROPERTIES, ossl_sha3_512_functions },

    { PROV_NAMES_SHAKE_128, FIPS_DEFAULT_PROPERTIES, ossl_shake_128_functions },
    { PROV_NAMES_SHAKE_256, FIPS_DEFAULT_PROPERTIES, ossl_shake_256_functions },

    /*
     * KECCAK-KMAC-128 and KECCAK-KMAC-256 as hashes are mostly useful for
     * KMAC128 and KMAC256.
     */
    { PROV_NAMES_KECCAK_KMAC_128, FIPS_DEFAULT_PROPERTIES,
      ossl_keccak_kmac_128_functions },
    { PROV_NAMES_KECCAK_KMAC_256, FIPS_DEFAULT_PROPERTIES,
      ossl_keccak_kmac_256_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM_CAPABLE fips_ciphers[] = {
    /* Our primary name[:ASN.1 OID name][:our older names] */
    ALG(PROV_NAMES_AES_256_ECB, ossl_aes256ecb_functions),
    ALG(PROV_NAMES_AES_192_ECB, ossl_aes192ecb_functions),
    ALG(PROV_NAMES_AES_128_ECB, ossl_aes128ecb_functions),
    ALG(PROV_NAMES_AES_256_CBC, ossl_aes256cbc_functions),
    ALG(PROV_NAMES_AES_192_CBC, ossl_aes192cbc_functions),
    ALG(PROV_NAMES_AES_128_CBC, ossl_aes128cbc_functions),
    ALG(PROV_NAMES_AES_256_CBC_CTS, ossl_aes256cbc_cts_functions),
    ALG(PROV_NAMES_AES_192_CBC_CTS, ossl_aes192cbc_cts_functions),
    ALG(PROV_NAMES_AES_128_CBC_CTS, ossl_aes128cbc_cts_functions),
    ALG(PROV_NAMES_AES_256_OFB, ossl_aes256ofb_functions),
    ALG(PROV_NAMES_AES_192_OFB, ossl_aes192ofb_functions),
    ALG(PROV_NAMES_AES_128_OFB, ossl_aes128ofb_functions),
    ALG(PROV_NAMES_AES_256_CFB, ossl_aes256cfb_functions),
    ALG(PROV_NAMES_AES_192_CFB, ossl_aes192cfb_functions),
    ALG(PROV_NAMES_AES_128_CFB, ossl_aes128cfb_functions),
    ALG(PROV_NAMES_AES_256_CFB1, ossl_aes256cfb1_functions),
    ALG(PROV_NAMES_AES_192_CFB1, ossl_aes192cfb1_functions),
    ALG(PROV_NAMES_AES_128_CFB1, ossl_aes128cfb1_functions),
    ALG(PROV_NAMES_AES_256_CFB8, ossl_aes256cfb8_functions),
    ALG(PROV_NAMES_AES_192_CFB8, ossl_aes192cfb8_functions),
    ALG(PROV_NAMES_AES_128_CFB8, ossl_aes128cfb8_functions),
    ALG(PROV_NAMES_AES_256_CTR, ossl_aes256ctr_functions),
    ALG(PROV_NAMES_AES_192_CTR, ossl_aes192ctr_functions),
    ALG(PROV_NAMES_AES_128_CTR, ossl_aes128ctr_functions),
    ALG(PROV_NAMES_AES_256_XTS, ossl_aes256xts_functions),
    ALG(PROV_NAMES_AES_128_XTS, ossl_aes128xts_functions),
    ALG(PROV_NAMES_AES_256_GCM, ossl_aes256gcm_functions),
    ALG(PROV_NAMES_AES_192_GCM, ossl_aes192gcm_functions),
    ALG(PROV_NAMES_AES_128_GCM, ossl_aes128gcm_functions),
    ALG(PROV_NAMES_AES_256_CCM, ossl_aes256ccm_functions),
    ALG(PROV_NAMES_AES_192_CCM, ossl_aes192ccm_functions),
    ALG(PROV_NAMES_AES_128_CCM, ossl_aes128ccm_functions),
    ALG(PROV_NAMES_AES_256_WRAP, ossl_aes256wrap_functions),
    ALG(PROV_NAMES_AES_192_WRAP, ossl_aes192wrap_functions),
    ALG(PROV_NAMES_AES_128_WRAP, ossl_aes128wrap_functions),
    ALG(PROV_NAMES_AES_256_WRAP_PAD, ossl_aes256wrappad_functions),
    ALG(PROV_NAMES_AES_192_WRAP_PAD, ossl_aes192wrappad_functions),
    ALG(PROV_NAMES_AES_128_WRAP_PAD, ossl_aes128wrappad_functions),
    ALG(PROV_NAMES_AES_256_WRAP_INV, ossl_aes256wrapinv_functions),
    ALG(PROV_NAMES_AES_192_WRAP_INV, ossl_aes192wrapinv_functions),
    ALG(PROV_NAMES_AES_128_WRAP_INV, ossl_aes128wrapinv_functions),
    ALG(PROV_NAMES_AES_256_WRAP_PAD_INV, ossl_aes256wrappadinv_functions),
    ALG(PROV_NAMES_AES_192_WRAP_PAD_INV, ossl_aes192wrappadinv_functions),
    ALG(PROV_NAMES_AES_128_WRAP_PAD_INV, ossl_aes128wrappadinv_functions),
    ALGC(PROV_NAMES_AES_128_CBC_HMAC_SHA1, ossl_aes128cbc_hmac_sha1_functions,
         ossl_cipher_capable_aes_cbc_hmac_sha1),
    ALGC(PROV_NAMES_AES_256_CBC_HMAC_SHA1, ossl_aes256cbc_hmac_sha1_functions,
         ossl_cipher_capable_aes_cbc_hmac_sha1),
    ALGC(PROV_NAMES_AES_128_CBC_HMAC_SHA256, ossl_aes128cbc_hmac_sha256_functions,
         ossl_cipher_capable_aes_cbc_hmac_sha256),
    ALGC(PROV_NAMES_AES_256_CBC_HMAC_SHA256, ossl_aes256cbc_hmac_sha256_functions,
         ossl_cipher_capable_aes_cbc_hmac_sha256),
#ifndef OPENSSL_NO_DES
    ALG(PROV_NAMES_DES_EDE3_ECB, ossl_tdes_ede3_ecb_functions),
    ALG(PROV_NAMES_DES_EDE3_CBC, ossl_tdes_ede3_cbc_functions),
#endif  /* OPENSSL_NO_DES */
    { { NULL, NULL, NULL }, NULL }
};
static OSSL_ALGORITHM exported_fips_ciphers[OSSL_NELEM(fips_ciphers)];

static const OSSL_ALGORITHM fips_macs[] = {
#ifndef OPENSSL_NO_CMAC
    { PROV_NAMES_CMAC, FIPS_DEFAULT_PROPERTIES, ossl_cmac_functions },
#endif
    { PROV_NAMES_GMAC, FIPS_DEFAULT_PROPERTIES, ossl_gmac_functions },
    { PROV_NAMES_HMAC, FIPS_DEFAULT_PROPERTIES, ossl_hmac_functions },
    { PROV_NAMES_KMAC_128, FIPS_DEFAULT_PROPERTIES, ossl_kmac128_functions },
    { PROV_NAMES_KMAC_256, FIPS_DEFAULT_PROPERTIES, ossl_kmac256_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_macs_internal[] = {
#ifndef OPENSSL_NO_CMAC
    { PROV_NAMES_CMAC, FIPS_DEFAULT_PROPERTIES, ossl_cmac_functions },
#endif
    { PROV_NAMES_HMAC, FIPS_DEFAULT_PROPERTIES, ossl_hmac_internal_functions },
    { PROV_NAMES_KMAC_128, FIPS_DEFAULT_PROPERTIES, ossl_kmac128_internal_functions },
    { PROV_NAMES_KMAC_256, FIPS_DEFAULT_PROPERTIES, ossl_kmac256_internal_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_kdfs[] = {
    { PROV_NAMES_HKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_hkdf_functions },
    { PROV_NAMES_TLS1_3_KDF, FIPS_DEFAULT_PROPERTIES,
      ossl_kdf_tls1_3_kdf_functions },
    { PROV_NAMES_SSKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_sskdf_functions },
    { PROV_NAMES_PBKDF2, FIPS_DEFAULT_PROPERTIES, ossl_kdf_pbkdf2_functions },
    { PROV_NAMES_SSHKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_sshkdf_functions },
    { PROV_NAMES_X963KDF, FIPS_DEFAULT_PROPERTIES,
      ossl_kdf_x963_kdf_functions },
    { PROV_NAMES_X942KDF_ASN1, FIPS_DEFAULT_PROPERTIES,
      ossl_kdf_x942_kdf_functions },
    { PROV_NAMES_TLS1_PRF, FIPS_DEFAULT_PROPERTIES,
      ossl_kdf_tls1_prf_functions },
    { PROV_NAMES_KBKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_kbkdf_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_rands[] = {
    { PROV_NAMES_CRNG_TEST, FIPS_UNAPPROVED_PROPERTIES, ossl_crng_test_functions },
    { PROV_NAMES_CTR_DRBG, FIPS_DEFAULT_PROPERTIES, ossl_drbg_ctr_functions },
    { PROV_NAMES_HASH_DRBG, FIPS_DEFAULT_PROPERTIES, ossl_drbg_hash_functions },
    { PROV_NAMES_HMAC_DRBG, FIPS_DEFAULT_PROPERTIES, ossl_drbg_ossl_hmac_functions },
#ifndef OPENSSL_NO_FIPS_JITTER
    { PROV_NAMES_JITTER, FIPS_DEFAULT_PROPERTIES, ossl_jitter_functions },
#endif
    { PROV_NAMES_TEST_RAND, FIPS_UNAPPROVED_PROPERTIES, ossl_test_rng_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_keyexch[] = {
#ifndef OPENSSL_NO_DH
    { PROV_NAMES_DH, FIPS_DEFAULT_PROPERTIES, ossl_dh_keyexch_functions },
#endif
#ifndef OPENSSL_NO_EC
    { PROV_NAMES_ECDH, FIPS_DEFAULT_PROPERTIES, ossl_ecdh_keyexch_functions },
# ifndef OPENSSL_NO_ECX
    { PROV_NAMES_X25519, FIPS_UNAPPROVED_PROPERTIES, ossl_x25519_keyexch_functions },
    { PROV_NAMES_X448, FIPS_UNAPPROVED_PROPERTIES, ossl_x448_keyexch_functions },
# endif
#endif
    { PROV_NAMES_TLS1_PRF, FIPS_DEFAULT_PROPERTIES,
      ossl_kdf_tls1_prf_keyexch_functions },
    { PROV_NAMES_HKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_hkdf_keyexch_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_signature[] = {
#ifndef OPENSSL_NO_DSA
    { PROV_NAMES_DSA, FIPS_DEFAULT_PROPERTIES, ossl_dsa_signature_functions },
    { PROV_NAMES_DSA_SHA1, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha1_signature_functions },
    { PROV_NAMES_DSA_SHA224, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha224_signature_functions },
    { PROV_NAMES_DSA_SHA256, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha256_signature_functions },
    { PROV_NAMES_DSA_SHA384, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha384_signature_functions },
    { PROV_NAMES_DSA_SHA512, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha512_signature_functions },
    { PROV_NAMES_DSA_SHA3_224, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha3_224_signature_functions },
    { PROV_NAMES_DSA_SHA3_256, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha3_256_signature_functions },
    { PROV_NAMES_DSA_SHA3_384, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha3_384_signature_functions },
    { PROV_NAMES_DSA_SHA3_512, FIPS_DEFAULT_PROPERTIES, ossl_dsa_sha3_512_signature_functions },
#endif
    { PROV_NAMES_RSA, FIPS_DEFAULT_PROPERTIES, ossl_rsa_signature_functions },
    { PROV_NAMES_RSA_SHA1, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha1_signature_functions },
    { PROV_NAMES_RSA_SHA224, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha224_signature_functions },
    { PROV_NAMES_RSA_SHA256, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha256_signature_functions },
    { PROV_NAMES_RSA_SHA384, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha384_signature_functions },
    { PROV_NAMES_RSA_SHA512, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha512_signature_functions },
    { PROV_NAMES_RSA_SHA512_224, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha512_224_signature_functions },
    { PROV_NAMES_RSA_SHA512_256, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha512_256_signature_functions },
    { PROV_NAMES_RSA_SHA3_224, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha3_224_signature_functions },
    { PROV_NAMES_RSA_SHA3_256, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha3_256_signature_functions },
    { PROV_NAMES_RSA_SHA3_384, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha3_384_signature_functions },
    { PROV_NAMES_RSA_SHA3_512, FIPS_DEFAULT_PROPERTIES,
      ossl_rsa_sha3_512_signature_functions },
#ifndef OPENSSL_NO_EC
# ifndef OPENSSL_NO_ECX
    { PROV_NAMES_ED25519, FIPS_DEFAULT_PROPERTIES,
      ossl_ed25519_signature_functions },
    { PROV_NAMES_ED25519ph, FIPS_DEFAULT_PROPERTIES,
      ossl_ed25519ph_signature_functions },
    { PROV_NAMES_ED25519ctx, FIPS_DEFAULT_PROPERTIES,
      ossl_ed25519ctx_signature_functions },
    { PROV_NAMES_ED448, FIPS_DEFAULT_PROPERTIES,
      ossl_ed448_signature_functions },
    { PROV_NAMES_ED448ph, FIPS_DEFAULT_PROPERTIES,
      ossl_ed448ph_signature_functions },
# endif
    { PROV_NAMES_ECDSA, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_signature_functions },
    { PROV_NAMES_ECDSA_SHA1, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha1_signature_functions },
    { PROV_NAMES_ECDSA_SHA224, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha224_signature_functions },
    { PROV_NAMES_ECDSA_SHA256, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha256_signature_functions },
    { PROV_NAMES_ECDSA_SHA384, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha384_signature_functions },
    { PROV_NAMES_ECDSA_SHA512, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha512_signature_functions },
    { PROV_NAMES_ECDSA_SHA3_224, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha3_224_signature_functions },
    { PROV_NAMES_ECDSA_SHA3_256, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha3_256_signature_functions },
    { PROV_NAMES_ECDSA_SHA3_384, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha3_384_signature_functions },
    { PROV_NAMES_ECDSA_SHA3_512, FIPS_DEFAULT_PROPERTIES, ossl_ecdsa_sha3_512_signature_functions },
#endif
#ifndef OPENSSL_NO_ML_DSA
    { PROV_NAMES_ML_DSA_44, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_44_signature_functions },
    { PROV_NAMES_ML_DSA_65, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_65_signature_functions },
    { PROV_NAMES_ML_DSA_87, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_87_signature_functions },
#endif
    { PROV_NAMES_HMAC, FIPS_DEFAULT_PROPERTIES,
      ossl_mac_legacy_hmac_signature_functions },
#ifndef OPENSSL_NO_CMAC
    { PROV_NAMES_CMAC, FIPS_DEFAULT_PROPERTIES,
      ossl_mac_legacy_cmac_signature_functions },
#endif
#ifndef OPENSSL_NO_SLH_DSA
    { PROV_NAMES_SLH_DSA_SHA2_128S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_128s_signature_functions, PROV_DESCS_SLH_DSA_SHA2_128S },
    { PROV_NAMES_SLH_DSA_SHA2_128F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_128f_signature_functions, PROV_DESCS_SLH_DSA_SHA2_128F },
    { PROV_NAMES_SLH_DSA_SHA2_192S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_192s_signature_functions, PROV_DESCS_SLH_DSA_SHA2_192S },
    { PROV_NAMES_SLH_DSA_SHA2_192F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_192f_signature_functions, PROV_DESCS_SLH_DSA_SHA2_192F },
    { PROV_NAMES_SLH_DSA_SHA2_256S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_256s_signature_functions, PROV_DESCS_SLH_DSA_SHA2_256S },
    { PROV_NAMES_SLH_DSA_SHA2_256F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_256f_signature_functions, PROV_DESCS_SLH_DSA_SHA2_256F },
    { PROV_NAMES_SLH_DSA_SHAKE_128S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_128s_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_128S },
    { PROV_NAMES_SLH_DSA_SHAKE_128F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_128f_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_128F },
    { PROV_NAMES_SLH_DSA_SHAKE_192S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_192s_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_192S },
    { PROV_NAMES_SLH_DSA_SHAKE_192F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_192f_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_192F },
    { PROV_NAMES_SLH_DSA_SHAKE_256S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_256s_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_256S },
    { PROV_NAMES_SLH_DSA_SHAKE_256F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_256f_signature_functions, PROV_DESCS_SLH_DSA_SHAKE_256F },
#endif /* OPENSSL_NO_SLH_DSA */
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_asym_cipher[] = {
    { PROV_NAMES_RSA, FIPS_DEFAULT_PROPERTIES, ossl_rsa_asym_cipher_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_asym_kem[] = {
    { PROV_NAMES_RSA, FIPS_DEFAULT_PROPERTIES, ossl_rsa_asym_kem_functions },
#ifndef OPENSSL_NO_ML_KEM
    { PROV_NAMES_ML_KEM_512, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_asym_kem_functions },
    { PROV_NAMES_ML_KEM_768, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_asym_kem_functions },
    { PROV_NAMES_ML_KEM_1024, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_asym_kem_functions },
# if !defined(OPENSSL_NO_ECX)
    { "X25519MLKEM768", FIPS_DEFAULT_PROPERTIES, ossl_mlx_kem_asym_kem_functions },
    { "X448MLKEM1024", FIPS_DEFAULT_PROPERTIES, ossl_mlx_kem_asym_kem_functions },
# endif
# if !defined(OPENSSL_NO_EC)
    { "SecP256r1MLKEM768", FIPS_DEFAULT_PROPERTIES, ossl_mlx_kem_asym_kem_functions },
    { "SecP384r1MLKEM1024", FIPS_DEFAULT_PROPERTIES, ossl_mlx_kem_asym_kem_functions },
# endif
#endif
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM fips_keymgmt[] = {
#ifndef OPENSSL_NO_DH
    { PROV_NAMES_DH, FIPS_DEFAULT_PROPERTIES, ossl_dh_keymgmt_functions,
      PROV_DESCS_DH },
    { PROV_NAMES_DHX, FIPS_DEFAULT_PROPERTIES, ossl_dhx_keymgmt_functions,
      PROV_DESCS_DHX },
#endif
#ifndef OPENSSL_NO_DSA
    { PROV_NAMES_DSA, FIPS_DEFAULT_PROPERTIES, ossl_dsa_keymgmt_functions,
      PROV_DESCS_DSA },
#endif
    { PROV_NAMES_RSA, FIPS_DEFAULT_PROPERTIES, ossl_rsa_keymgmt_functions,
      PROV_DESCS_RSA },
    { PROV_NAMES_RSA_PSS, FIPS_DEFAULT_PROPERTIES,
      ossl_rsapss_keymgmt_functions, PROV_DESCS_RSA_PSS },
#ifndef OPENSSL_NO_EC
    { PROV_NAMES_EC, FIPS_DEFAULT_PROPERTIES, ossl_ec_keymgmt_functions,
      PROV_DESCS_EC },
# ifndef OPENSSL_NO_ECX
    { PROV_NAMES_X25519, FIPS_UNAPPROVED_PROPERTIES, ossl_x25519_keymgmt_functions,
      PROV_DESCS_X25519 },
    { PROV_NAMES_X448, FIPS_UNAPPROVED_PROPERTIES, ossl_x448_keymgmt_functions,
      PROV_DESCS_X448 },
    { PROV_NAMES_ED25519, FIPS_DEFAULT_PROPERTIES, ossl_ed25519_keymgmt_functions,
      PROV_DESCS_ED25519 },
    { PROV_NAMES_ED448, FIPS_DEFAULT_PROPERTIES, ossl_ed448_keymgmt_functions,
      PROV_DESCS_ED448 },
# endif
#endif
#ifndef OPENSSL_NO_ML_DSA
    { PROV_NAMES_ML_DSA_44, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_44_keymgmt_functions,
      PROV_DESCS_ML_DSA_44 },
    { PROV_NAMES_ML_DSA_65, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_65_keymgmt_functions,
      PROV_DESCS_ML_DSA_65 },
    { PROV_NAMES_ML_DSA_87, FIPS_DEFAULT_PROPERTIES, ossl_ml_dsa_87_keymgmt_functions,
      PROV_DESCS_ML_DSA_87 },
#endif /* OPENSSL_NO_ML_DSA */
    { PROV_NAMES_TLS1_PRF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_keymgmt_functions,
      PROV_DESCS_TLS1_PRF_SIGN },
    { PROV_NAMES_HKDF, FIPS_DEFAULT_PROPERTIES, ossl_kdf_keymgmt_functions,
      PROV_DESCS_HKDF_SIGN },
    { PROV_NAMES_HMAC, FIPS_DEFAULT_PROPERTIES, ossl_mac_legacy_keymgmt_functions,
      PROV_DESCS_HMAC_SIGN },
#ifndef OPENSSL_NO_CMAC
    { PROV_NAMES_CMAC, FIPS_DEFAULT_PROPERTIES,
      ossl_cmac_legacy_keymgmt_functions, PROV_DESCS_CMAC_SIGN },
#endif
#ifndef OPENSSL_NO_ML_KEM
    { PROV_NAMES_ML_KEM_512, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_512_keymgmt_functions,
      PROV_DESCS_ML_KEM_512 },
    { PROV_NAMES_ML_KEM_768, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_768_keymgmt_functions,
      PROV_DESCS_ML_KEM_768 },
    { PROV_NAMES_ML_KEM_1024, FIPS_DEFAULT_PROPERTIES, ossl_ml_kem_1024_keymgmt_functions,
      PROV_DESCS_ML_KEM_1024 },
# if !defined(OPENSSL_NO_ECX)
    { PROV_NAMES_X25519MLKEM768, FIPS_DEFAULT_PROPERTIES, ossl_mlx_x25519_kem_kmgmt_functions,
      PROV_DESCS_X25519MLKEM768 },
    { PROV_NAMES_X448MLKEM1024, FIPS_DEFAULT_PROPERTIES, ossl_mlx_x448_kem_kmgmt_functions,
      PROV_DESCS_X448MLKEM1024 },
# endif
# if !defined(OPENSSL_NO_EC)
    { PROV_NAMES_SecP256r1MLKEM768, FIPS_DEFAULT_PROPERTIES, ossl_mlx_p256_kem_kmgmt_functions,
      PROV_DESCS_SecP256r1MLKEM768 },
    { PROV_NAMES_SecP384r1MLKEM1024, FIPS_DEFAULT_PROPERTIES, ossl_mlx_p384_kem_kmgmt_functions,
      PROV_DESCS_SecP384r1MLKEM1024 },
# endif
#endif
#ifndef OPENSSL_NO_SLH_DSA
    { PROV_NAMES_SLH_DSA_SHA2_128S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_128s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_128S },
    { PROV_NAMES_SLH_DSA_SHA2_128F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_128f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_128F },
    { PROV_NAMES_SLH_DSA_SHA2_192S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_192s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_192S },
    { PROV_NAMES_SLH_DSA_SHA2_192F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_192f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_192F },
    { PROV_NAMES_SLH_DSA_SHA2_256S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_256s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_256S },
    { PROV_NAMES_SLH_DSA_SHA2_256F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_sha2_256f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHA2_256F },
    { PROV_NAMES_SLH_DSA_SHAKE_128S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_128s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_128S },
    { PROV_NAMES_SLH_DSA_SHAKE_128F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_128f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_128F },
    { PROV_NAMES_SLH_DSA_SHAKE_192S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_192s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_192S },
    { PROV_NAMES_SLH_DSA_SHAKE_192F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_192f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_192F },
    { PROV_NAMES_SLH_DSA_SHAKE_256S, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_256s_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_256S },
    { PROV_NAMES_SLH_DSA_SHAKE_256F, FIPS_DEFAULT_PROPERTIES,
      ossl_slh_dsa_shake_256f_keymgmt_functions, PROV_DESCS_SLH_DSA_SHAKE_256F },
#endif /* OPENSSL_NO_SLH_DSA */
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *fips_query(void *provctx, int operation_id,
                                        int *no_cache)
{
    *no_cache = 0;

    if (!ossl_prov_is_running())
        return NULL;

    switch (operation_id) {
    case OSSL_OP_DIGEST:
        return fips_digests;
    case OSSL_OP_CIPHER:
        return exported_fips_ciphers;
    case OSSL_OP_MAC:
        return fips_macs;
    case OSSL_OP_KDF:
        return fips_kdfs;
    case OSSL_OP_RAND:
        return fips_rands;
    case OSSL_OP_KEYMGMT:
        return fips_keymgmt;
    case OSSL_OP_KEYEXCH:
        return fips_keyexch;
    case OSSL_OP_SIGNATURE:
        return fips_signature;
    case OSSL_OP_ASYM_CIPHER:
        return fips_asym_cipher;
    case OSSL_OP_KEM:
        return fips_asym_kem;
    }
    return NULL;
}

static const OSSL_ALGORITHM *fips_query_internal(void *provctx, int operation_id,
                                                 int *no_cache)
{
    if (operation_id == OSSL_OP_MAC) {
        *no_cache = 0;
        if (!ossl_prov_is_running())
            return NULL;
        return fips_macs_internal;
    }
    return fips_query(provctx, operation_id, no_cache);
}

static void fips_teardown(void *provctx)
{
    OSSL_LIB_CTX_free(PROV_LIBCTX_OF(provctx));
    ossl_prov_ctx_free(provctx);
}

static void fips_intern_teardown(void *provctx)
{
    /*
     * We know that the library context is the same as for the outer provider,
     * so no need to destroy it here.
     */
    ossl_prov_ctx_free(provctx);
}

/* Functions we provide to the core */
static const OSSL_DISPATCH fips_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))fips_teardown },
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS, (void (*)(void))fips_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))fips_get_params },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fips_query },
    { OSSL_FUNC_PROVIDER_GET_CAPABILITIES,
      (void (*)(void))ossl_prov_get_capabilities },
    { OSSL_FUNC_PROVIDER_SELF_TEST, (void (*)(void))fips_self_test },
    { OSSL_FUNC_PROVIDER_RANDOM_BYTES, (void (*)(void))fips_random_bytes },
    OSSL_DISPATCH_END
};

/* Functions we provide to ourself */
static const OSSL_DISPATCH intern_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))fips_intern_teardown },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fips_query_internal },
    OSSL_DISPATCH_END
};

/*
 * On VMS, the provider init function name is expected to be uppercase,
 * see the pragmas in <openssl/core.h>.  Let's do the same with this
 * internal name.  This is how symbol names are treated by default
 * by the compiler if nothing else is said, but since this is part
 * of libfips, and we build our libraries with mixed case symbol names,
 * we must switch back to this default explicitly here.
 */
#ifdef __VMS
# pragma names save
# pragma names uppercase,truncated
#endif
OSSL_provider_init_fn OSSL_provider_init_int;
#ifdef __VMS
# pragma names restore
#endif
int OSSL_provider_init_int(const OSSL_CORE_HANDLE *handle,
                           const OSSL_DISPATCH *in,
                           const OSSL_DISPATCH **out,
                           void **provctx)
{
    FIPS_GLOBAL *fgbl;
    OSSL_LIB_CTX *libctx = NULL;
    SELF_TEST_POST_PARAMS selftest_params;

    memset(&selftest_params, 0, sizeof(selftest_params));

    if (!ossl_prov_seeding_from_dispatch(in))
        goto err;
    for (; in->function_id != 0; in++) {
        /*
         * We do not support the scenario of an application linked against
         * multiple versions of libcrypto (e.g. one static and one dynamic), but
         * sharing a single fips.so. We do a simple sanity check here.
         */
#define set_func(c, f) if (c == NULL) c = f; else if (c != f) return 0;
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GET_LIBCTX:
            set_func(c_get_libctx, OSSL_FUNC_core_get_libctx(in));
            break;
        case OSSL_FUNC_CORE_GETTABLE_PARAMS:
            set_func(c_gettable_params, OSSL_FUNC_core_gettable_params(in));
            break;
        case OSSL_FUNC_CORE_GET_PARAMS:
            set_func(c_get_params, OSSL_FUNC_core_get_params(in));
            break;
        case OSSL_FUNC_CORE_THREAD_START:
            set_func(c_thread_start, OSSL_FUNC_core_thread_start(in));
            break;
        case OSSL_FUNC_CORE_NEW_ERROR:
            set_func(c_new_error, OSSL_FUNC_core_new_error(in));
            break;
        case OSSL_FUNC_CORE_SET_ERROR_DEBUG:
            set_func(c_set_error_debug, OSSL_FUNC_core_set_error_debug(in));
            break;
        case OSSL_FUNC_CORE_VSET_ERROR:
            set_func(c_vset_error, OSSL_FUNC_core_vset_error(in));
            break;
        case OSSL_FUNC_CORE_SET_ERROR_MARK:
            set_func(c_set_error_mark, OSSL_FUNC_core_set_error_mark(in));
            break;
        case OSSL_FUNC_CORE_CLEAR_LAST_ERROR_MARK:
            set_func(c_clear_last_error_mark,
                     OSSL_FUNC_core_clear_last_error_mark(in));
            break;
        case OSSL_FUNC_CORE_POP_ERROR_TO_MARK:
            set_func(c_pop_error_to_mark, OSSL_FUNC_core_pop_error_to_mark(in));
            break;
        case OSSL_FUNC_CRYPTO_MALLOC:
            set_func(c_CRYPTO_malloc, OSSL_FUNC_CRYPTO_malloc(in));
            break;
        case OSSL_FUNC_CRYPTO_ZALLOC:
            set_func(c_CRYPTO_zalloc, OSSL_FUNC_CRYPTO_zalloc(in));
            break;
        case OSSL_FUNC_CRYPTO_FREE:
            set_func(c_CRYPTO_free, OSSL_FUNC_CRYPTO_free(in));
            break;
        case OSSL_FUNC_CRYPTO_CLEAR_FREE:
            set_func(c_CRYPTO_clear_free, OSSL_FUNC_CRYPTO_clear_free(in));
            break;
        case OSSL_FUNC_CRYPTO_REALLOC:
            set_func(c_CRYPTO_realloc, OSSL_FUNC_CRYPTO_realloc(in));
            break;
        case OSSL_FUNC_CRYPTO_CLEAR_REALLOC:
            set_func(c_CRYPTO_clear_realloc,
                     OSSL_FUNC_CRYPTO_clear_realloc(in));
            break;
        case OSSL_FUNC_CRYPTO_SECURE_MALLOC:
            set_func(c_CRYPTO_secure_malloc,
                     OSSL_FUNC_CRYPTO_secure_malloc(in));
            break;
        case OSSL_FUNC_CRYPTO_SECURE_ZALLOC:
            set_func(c_CRYPTO_secure_zalloc,
                     OSSL_FUNC_CRYPTO_secure_zalloc(in));
            break;
        case OSSL_FUNC_CRYPTO_SECURE_FREE:
            set_func(c_CRYPTO_secure_free,
                     OSSL_FUNC_CRYPTO_secure_free(in));
            break;
        case OSSL_FUNC_CRYPTO_SECURE_CLEAR_FREE:
            set_func(c_CRYPTO_secure_clear_free,
                     OSSL_FUNC_CRYPTO_secure_clear_free(in));
            break;
        case OSSL_FUNC_CRYPTO_SECURE_ALLOCATED:
            set_func(c_CRYPTO_secure_allocated,
                     OSSL_FUNC_CRYPTO_secure_allocated(in));
            break;
        case OSSL_FUNC_BIO_NEW_FILE:
            set_func(selftest_params.bio_new_file_cb,
                     OSSL_FUNC_BIO_new_file(in));
            break;
        case OSSL_FUNC_BIO_NEW_MEMBUF:
            set_func(selftest_params.bio_new_buffer_cb,
                     OSSL_FUNC_BIO_new_membuf(in));
            break;
        case OSSL_FUNC_BIO_READ_EX:
            set_func(selftest_params.bio_read_ex_cb,
                     OSSL_FUNC_BIO_read_ex(in));
            break;
        case OSSL_FUNC_BIO_FREE:
            set_func(selftest_params.bio_free_cb, OSSL_FUNC_BIO_free(in));
            break;
        case OSSL_FUNC_BIO_VSNPRINTF:
            set_func(c_BIO_vsnprintf, OSSL_FUNC_BIO_vsnprintf(in));
            break;
        case OSSL_FUNC_SELF_TEST_CB:
            set_func(c_stcbfn, OSSL_FUNC_self_test_cb(in));
            break;
        case OSSL_FUNC_INDICATOR_CB:
            set_func(c_indcbfn, OSSL_FUNC_indicator_cb(in));
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    OPENSSL_cpuid_setup();

    /*  Create a context. */
    if ((*provctx = ossl_prov_ctx_new()) == NULL
            || (libctx = OSSL_LIB_CTX_new()) == NULL)
        goto err;

    if ((fgbl = ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_FIPS_PROV_INDEX)) == NULL)
        goto err;

    fgbl->handle = handle;

    /*
     * We need to register this thread to receive thread lifecycle callbacks.
     * This wouldn't matter if the current thread is also the same thread that
     * closes the FIPS provider down. But if that happens on a different thread
     * then memory leaks could otherwise occur.
     */
    if (!ossl_thread_register_fips(libctx))
        goto err;

    /*
     * Ensure our internal provider is loaded. We use this whenever the FIPS
     * provider internally uses the EVP API. We proactively load this now
     * rather than waiting for lazy loading to ensure it is always present when
     * we need it.
     */
    if (!ossl_provider_activate_fallbacks(libctx))
        goto err;

    /*
     * We did initial set up of selftest_params in a local copy, because we
     * could not create fgbl until c_CRYPTO_zalloc was defined in the loop
     * above.
     */
    fgbl->selftest_params = selftest_params;

    fgbl->selftest_params.libctx = libctx;

    set_self_test_cb(fgbl);

    if (!fips_get_params_from_core(fgbl)) {
        /* Error already raised */
        goto err;
    }
    /*
     * Disable the conditional error check if it's disabled in the fips config
     * file.
     */
    if (fgbl->selftest_params.conditional_error_check != NULL
        && strcmp(fgbl->selftest_params.conditional_error_check, "0") == 0)
        SELF_TEST_disable_conditional_error_state();

    /* Enable or disable FIPS provider options */
#define OSSL_FIPS_PARAM(structname, paramname, unused)                         \
    if (fgbl->fips_##structname.option != NULL) {                              \
        if (strcmp(fgbl->fips_##structname.option, "1") == 0)                  \
            fgbl->fips_##structname.enabled = 1;                               \
        else if (strcmp(fgbl->fips_##structname.option, "0") == 0)             \
            fgbl->fips_##structname.enabled = 0;                               \
        else                                                                   \
            goto err;                                                          \
    }
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

    ossl_prov_cache_exported_algorithms(fips_ciphers, exported_fips_ciphers);

    if (!SELF_TEST_post(&fgbl->selftest_params, 0)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_SELF_TEST_POST_FAILURE);
        goto err;
    }

    ossl_prov_ctx_set0_libctx(*provctx, libctx);
    ossl_prov_ctx_set0_core_get_params(*provctx, c_get_params);
    ossl_prov_ctx_set0_handle(*provctx, handle);

    *out = fips_dispatch_table;
    return 1;
 err:
    fips_teardown(*provctx);
    OSSL_LIB_CTX_free(libctx);
    *provctx = NULL;
    return 0;
}

/*
 * The internal init function used when the FIPS module uses EVP to call
 * another algorithm also in the FIPS module. This is a recursive call that has
 * been made from within the FIPS module itself. To make this work, we populate
 * the provider context of this inner instance with the same library context
 * that was used in the EVP call that initiated this recursive call.
 */
OSSL_provider_init_fn ossl_fips_intern_provider_init;
int ossl_fips_intern_provider_init(const OSSL_CORE_HANDLE *handle,
                                   const OSSL_DISPATCH *in,
                                   const OSSL_DISPATCH **out,
                                   void **provctx)
{
    OSSL_FUNC_core_get_libctx_fn *c_internal_get_libctx = NULL;

    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GET_LIBCTX:
            c_internal_get_libctx = OSSL_FUNC_core_get_libctx(in);
            break;
        default:
            break;
        }
    }

    if (c_internal_get_libctx == NULL)
        return 0;

    if ((*provctx = ossl_prov_ctx_new()) == NULL)
        return 0;

    /*
     * Using the parent library context only works because we are a built-in
     * internal provider. This is not something that most providers would be
     * able to do.
     */
    ossl_prov_ctx_set0_libctx(*provctx,
                              (OSSL_LIB_CTX *)c_internal_get_libctx(handle));
    ossl_prov_ctx_set0_handle(*provctx, handle);

    *out = intern_dispatch_table;
    return 1;
}

void ERR_new(void)
{
    c_new_error(NULL);
}

void ERR_set_debug(const char *file, int line, const char *func)
{
    c_set_error_debug(NULL, file, line, func);
}

void ERR_set_error(int lib, int reason, const char *fmt, ...)
{
    va_list args;

    va_start(args, fmt);
    c_vset_error(NULL, ERR_PACK(lib, 0, reason), fmt, args);
    va_end(args);
}

void ERR_vset_error(int lib, int reason, const char *fmt, va_list args)
{
    c_vset_error(NULL, ERR_PACK(lib, 0, reason), fmt, args);
}

int ERR_set_mark(void)
{
    return c_set_error_mark(NULL);
}

int ERR_clear_last_mark(void)
{
    return c_clear_last_error_mark(NULL);
}

int ERR_pop_to_mark(void)
{
    return c_pop_error_to_mark(NULL);
}

/*
 * This must take a library context, since it's called from the depths
 * of crypto/initthread.c code, where it's (correctly) assumed that the
 * passed caller argument is an OSSL_LIB_CTX pointer (since the same routine
 * is also called from other parts of libcrypto, which all pass around a
 * OSSL_LIB_CTX pointer)
 */
const OSSL_CORE_HANDLE *FIPS_get_core_handle(OSSL_LIB_CTX *libctx)
{
    FIPS_GLOBAL *fgbl = ossl_lib_ctx_get_data(libctx,
                                              OSSL_LIB_CTX_FIPS_PROV_INDEX);

    if (fgbl == NULL)
        return NULL;

    return fgbl->handle;
}

void *CRYPTO_malloc(size_t num, const char *file, int line)
{
    return c_CRYPTO_malloc(num, file, line);
}

void *CRYPTO_zalloc(size_t num, const char *file, int line)
{
    return c_CRYPTO_zalloc(num, file, line);
}

void CRYPTO_free(void *ptr, const char *file, int line)
{
    c_CRYPTO_free(ptr, file, line);
}

void CRYPTO_clear_free(void *ptr, size_t num, const char *file, int line)
{
    c_CRYPTO_clear_free(ptr, num, file, line);
}

void *CRYPTO_realloc(void *addr, size_t num, const char *file, int line)
{
    return c_CRYPTO_realloc(addr, num, file, line);
}

void *CRYPTO_clear_realloc(void *addr, size_t old_num, size_t num,
                           const char *file, int line)
{
    return c_CRYPTO_clear_realloc(addr, old_num, num, file, line);
}

void *CRYPTO_secure_malloc(size_t num, const char *file, int line)
{
    return c_CRYPTO_secure_malloc(num, file, line);
}

void *CRYPTO_secure_zalloc(size_t num, const char *file, int line)
{
    return c_CRYPTO_secure_zalloc(num, file, line);
}

void CRYPTO_secure_free(void *ptr, const char *file, int line)
{
    c_CRYPTO_secure_free(ptr, file, line);
}

void CRYPTO_secure_clear_free(void *ptr, size_t num, const char *file, int line)
{
    c_CRYPTO_secure_clear_free(ptr, num, file, line);
}

int CRYPTO_secure_allocated(const void *ptr)
{
    return c_CRYPTO_secure_allocated(ptr);
}

void *CRYPTO_aligned_alloc(size_t num, size_t align, void **freeptr,
                           const char *file, int line)
{
    return NULL;
}

int BIO_snprintf(char *buf, size_t n, const char *format, ...)
{
    va_list args;
    int ret;

    va_start(args, format);
    ret = c_BIO_vsnprintf(buf, n, format, args);
    va_end(args);
    return ret;
}

#define OSSL_FIPS_PARAM(structname, paramname, unused)                         \
    int ossl_fips_config_##structname(OSSL_LIB_CTX *libctx)                    \
    {                                                                          \
        FIPS_GLOBAL *fgbl =                                                    \
            ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_FIPS_PROV_INDEX);       \
                                                                               \
        return fgbl->fips_##structname.enabled;                                \
    }
#include "fips_indicator_params.inc"
#undef OSSL_FIPS_PARAM

void OSSL_SELF_TEST_get_callback(OSSL_LIB_CTX *libctx, OSSL_CALLBACK **cb,
                                 void **cbarg)
{
    assert(libctx != NULL);

    if (c_stcbfn != NULL && c_get_libctx != NULL) {
        /* Get the parent libctx */
        c_stcbfn(c_get_libctx(FIPS_get_core_handle(libctx)), cb, cbarg);
    } else {
        if (cb != NULL)
            *cb = NULL;
        if (cbarg != NULL)
            *cbarg = NULL;
    }
}

void OSSL_INDICATOR_get_callback(OSSL_LIB_CTX *libctx,
                                 OSSL_INDICATOR_CALLBACK **cb)
{
    assert(libctx != NULL);

    if (c_indcbfn != NULL && c_get_libctx != NULL) {
        /* Get the parent libctx */
        c_indcbfn(c_get_libctx(FIPS_get_core_handle(libctx)), cb);
    } else {
        if (cb != NULL)
            *cb = NULL;
    }
}
