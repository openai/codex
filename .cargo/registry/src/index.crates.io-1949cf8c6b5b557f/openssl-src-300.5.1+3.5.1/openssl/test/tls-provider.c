/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/rand.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/pkcs12.h>
#include <openssl/provider.h>
#include <assert.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <openssl/core_object.h>
#include "internal/asn1.h"
/* For TLS1_3_VERSION */
#include <openssl/ssl.h>
#include "internal/nelem.h"
#include "internal/refcount.h"

/* error codes */

/* xorprovider error codes */
#define XORPROV_R_INVALID_DIGEST                            1
#define XORPROV_R_INVALID_SIZE                              2
#define XORPROV_R_INVALID_KEY                               3
#define XORPROV_R_UNSUPPORTED                               4
#define XORPROV_R_MISSING_OID                               5
#define XORPROV_R_OBJ_CREATE_ERR                            6
#define XORPROV_R_INVALID_ENCODING                          7
#define XORPROV_R_SIGN_ERROR                                8
#define XORPROV_R_LIB_CREATE_ERR                            9
#define XORPROV_R_NO_PRIVATE_KEY                            10
#define XORPROV_R_BUFFER_LENGTH_WRONG                       11
#define XORPROV_R_SIGNING_FAILED                            12
#define XORPROV_R_WRONG_PARAMETERS                          13
#define XORPROV_R_VERIFY_ERROR                              14
#define XORPROV_R_EVPINFO_MISSING                           15

static OSSL_FUNC_keymgmt_import_fn xor_import;
static OSSL_FUNC_keymgmt_import_types_fn xor_import_types;
static OSSL_FUNC_keymgmt_import_types_ex_fn xor_import_types_ex;
static OSSL_FUNC_keymgmt_export_fn xor_export;
static OSSL_FUNC_keymgmt_export_types_fn xor_export_types;
static OSSL_FUNC_keymgmt_export_types_ex_fn xor_export_types_ex;

int tls_provider_init(const OSSL_CORE_HANDLE *handle,
                      const OSSL_DISPATCH *in,
                      const OSSL_DISPATCH **out,
                      void **provctx);

#define XOR_KEY_SIZE 32

/*
 * Top secret. This algorithm only works if no one knows what this number is.
 * Please don't tell anyone what it is.
 *
 * This algorithm is for testing only - don't really use it!
 */
static const unsigned char private_constant[XOR_KEY_SIZE] = {
    0xd3, 0x6b, 0x54, 0xec, 0x5b, 0xac, 0x89, 0x96, 0x8c, 0x2c, 0x66, 0xa5,
    0x67, 0x0d, 0xe3, 0xdd, 0x43, 0x69, 0xbc, 0x83, 0x3d, 0x60, 0xc7, 0xb8,
    0x2b, 0x1c, 0x5a, 0xfd, 0xb5, 0xcd, 0xd0, 0xf8
};

typedef struct xorkey_st {
    unsigned char privkey[XOR_KEY_SIZE];
    unsigned char pubkey[XOR_KEY_SIZE];
    int hasprivkey;
    int haspubkey;
    char *tls_name;
    CRYPTO_REF_COUNT references;
} XORKEY;

/* Key Management for the dummy XOR KEX, KEM and signature algorithms */

static OSSL_FUNC_keymgmt_new_fn xor_newkey;
static OSSL_FUNC_keymgmt_free_fn xor_freekey;
static OSSL_FUNC_keymgmt_has_fn xor_has;
static OSSL_FUNC_keymgmt_dup_fn xor_dup;
static OSSL_FUNC_keymgmt_gen_init_fn xor_gen_init;
static OSSL_FUNC_keymgmt_gen_set_params_fn xor_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn xor_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_fn xor_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn xor_gen_cleanup;
static OSSL_FUNC_keymgmt_load_fn xor_load;
static OSSL_FUNC_keymgmt_get_params_fn xor_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn xor_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn xor_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn xor_settable_params;

/*
 * Dummy "XOR" Key Exchange algorithm. We just xor the private and public keys
 * together. Don't use this!
 */

static OSSL_FUNC_keyexch_newctx_fn xor_newkemkexctx;
static OSSL_FUNC_keyexch_init_fn xor_init;
static OSSL_FUNC_keyexch_set_peer_fn xor_set_peer;
static OSSL_FUNC_keyexch_derive_fn xor_derive;
static OSSL_FUNC_keyexch_freectx_fn xor_freectx;
static OSSL_FUNC_keyexch_dupctx_fn xor_dupctx;

/*
 * Dummy "XOR" Key Encapsulation Method. We just build a KEM over the xor KEX.
 * Don't use this!
 */

static OSSL_FUNC_kem_newctx_fn xor_newkemkexctx;
static OSSL_FUNC_kem_freectx_fn xor_freectx;
static OSSL_FUNC_kem_dupctx_fn xor_dupctx;
static OSSL_FUNC_kem_encapsulate_init_fn xor_init;
static OSSL_FUNC_kem_encapsulate_fn xor_encapsulate;
static OSSL_FUNC_kem_decapsulate_init_fn xor_init;
static OSSL_FUNC_kem_decapsulate_fn xor_decapsulate;

/*
 * Common key management table access functions
 */
static OSSL_FUNC_keymgmt_new_fn *
xor_prov_get_keymgmt_new(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_NEW)
            return OSSL_FUNC_keymgmt_new(fns);

    return NULL;
}

static OSSL_FUNC_keymgmt_free_fn *
xor_prov_get_keymgmt_free(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_FREE)
            return OSSL_FUNC_keymgmt_free(fns);

    return NULL;
}

static OSSL_FUNC_keymgmt_import_fn *
xor_prov_get_keymgmt_import(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_IMPORT)
            return OSSL_FUNC_keymgmt_import(fns);

    return NULL;
}

static OSSL_FUNC_keymgmt_export_fn *
xor_prov_get_keymgmt_export(const OSSL_DISPATCH *fns)
{
    /* Pilfer the keymgmt dispatch table */
    for (; fns->function_id != 0; fns++)
        if (fns->function_id == OSSL_FUNC_KEYMGMT_EXPORT)
            return OSSL_FUNC_keymgmt_export(fns);

    return NULL;
}

static void *xor_prov_import_key(const OSSL_DISPATCH *fns, void *provctx,
                           int selection, const OSSL_PARAM params[])
{
    OSSL_FUNC_keymgmt_new_fn *kmgmt_new = xor_prov_get_keymgmt_new(fns);
    OSSL_FUNC_keymgmt_free_fn *kmgmt_free = xor_prov_get_keymgmt_free(fns);
    OSSL_FUNC_keymgmt_import_fn *kmgmt_import =
        xor_prov_get_keymgmt_import(fns);
    void *key = NULL;

    if (kmgmt_new != NULL && kmgmt_import != NULL && kmgmt_free != NULL) {
        if ((key = kmgmt_new(provctx)) == NULL
            || !kmgmt_import(key, selection, params)) {
            kmgmt_free(key);
            key = NULL;
        }
    }
    return key;
}

static void xor_prov_free_key(const OSSL_DISPATCH *fns, void *key)
{
    OSSL_FUNC_keymgmt_free_fn *kmgmt_free = xor_prov_get_keymgmt_free(fns);

    if (kmgmt_free != NULL)
        kmgmt_free(key);
}

/*
 * We define 2 dummy TLS groups called "xorgroup" and "xorkemgroup" for test
 * purposes
 */
struct tls_group_st {
    unsigned int group_id; /* for "tls-group-id", see provider-base(7) */
    unsigned int secbits;
    unsigned int mintls;
    unsigned int maxtls;
    unsigned int mindtls;
    unsigned int maxdtls;
    unsigned int is_kem; /* boolean */
};

#define XORGROUP_NAME "xorgroup"
#define XORGROUP_NAME_INTERNAL "xorgroup-int"
static struct tls_group_st xor_group = {
    0,                  /* group_id, set by randomize_tls_alg_id() */
    128,                /* secbits */
    TLS1_3_VERSION,     /* mintls */
    0,                  /* maxtls */
    -1,                 /* mindtls */
    -1,                 /* maxdtls */
    0                   /* is_kem */
};

#define XORKEMGROUP_NAME "xorkemgroup"
#define XORKEMGROUP_NAME_INTERNAL "xorkemgroup-int"
static struct tls_group_st xor_kemgroup = {
    0,                  /* group_id, set by randomize_tls_alg_id() */
    128,                /* secbits */
    TLS1_3_VERSION,     /* mintls */
    0,                  /* maxtls */
    -1,                 /* mindtls */
    -1,                 /* maxdtls */
    1                   /* is_kem */
};

#define ALGORITHM "XOR"

static const OSSL_PARAM xor_group_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_NAME,
                           XORGROUP_NAME, sizeof(XORGROUP_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_NAME_INTERNAL,
                           XORGROUP_NAME_INTERNAL,
                           sizeof(XORGROUP_NAME_INTERNAL)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_ALG, ALGORITHM,
                           sizeof(ALGORITHM)),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_ID, &xor_group.group_id),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_SECURITY_BITS,
                    &xor_group.secbits),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MIN_TLS, &xor_group.mintls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MAX_TLS, &xor_group.maxtls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MIN_DTLS, &xor_group.mindtls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MAX_DTLS, &xor_group.maxdtls),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_IS_KEM, &xor_group.is_kem),
    OSSL_PARAM_END
};

static const OSSL_PARAM xor_kemgroup_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_NAME,
                           XORKEMGROUP_NAME, sizeof(XORKEMGROUP_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_NAME_INTERNAL,
                           XORKEMGROUP_NAME_INTERNAL,
                           sizeof(XORKEMGROUP_NAME_INTERNAL)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_GROUP_ALG, ALGORITHM,
                           sizeof(ALGORITHM)),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_ID, &xor_kemgroup.group_id),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_SECURITY_BITS,
                    &xor_kemgroup.secbits),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MIN_TLS, &xor_kemgroup.mintls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MAX_TLS, &xor_kemgroup.maxtls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MIN_DTLS, &xor_kemgroup.mindtls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_GROUP_MAX_DTLS, &xor_kemgroup.maxdtls),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_GROUP_IS_KEM, &xor_kemgroup.is_kem),
    OSSL_PARAM_END
};

#define NUM_DUMMY_GROUPS 50
static char *dummy_group_names[NUM_DUMMY_GROUPS];

/*
 * We define a dummy TLS sigalg called for test purposes
 */
struct tls_sigalg_st {
    unsigned int code_point; /* for "tls-sigalg-alg", see provider-base(7) */
    unsigned int secbits;
    unsigned int mintls;
    unsigned int maxtls;
};

#define XORSIGALG_NAME "xorhmacsig"
#define XORSIGALG_OID "1.3.6.1.4.1.16604.998888.1"
#define XORSIGALG_HASH_NAME "xorhmacsha2sig"
#define XORSIGALG_HASH "SHA256"
#define XORSIGALG_HASH_OID "1.3.6.1.4.1.16604.998888.2"
#define XORSIGALG12_NAME "xorhmacsig12"
#define XORSIGALG12_OID "1.3.6.1.4.1.16604.998888.3"

static struct tls_sigalg_st xor_sigalg = {
    0,                  /* alg id, set by randomize_tls_alg_id() */
    128,                /* secbits */
    TLS1_3_VERSION,     /* mintls */
    0,                  /* maxtls */
};

static struct tls_sigalg_st xor_sigalg_hash = {
    0,                  /* alg id, set by randomize_tls_alg_id() */
    128,                /* secbits */
    TLS1_3_VERSION,     /* mintls */
    0,                  /* maxtls */
};

static struct tls_sigalg_st xor_sigalg12 = {
    0,                  /* alg id, set by randomize_tls_alg_id() */
    128,                /* secbits */
    TLS1_2_VERSION,     /* mintls */
    TLS1_2_VERSION,     /* maxtls */
};

static const OSSL_PARAM xor_sig_nohash_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_IANA_NAME,
                           XORSIGALG_NAME, sizeof(XORSIGALG_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_NAME,
                           XORSIGALG_NAME,
                           sizeof(XORSIGALG_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_OID,
                           XORSIGALG_OID, sizeof(XORSIGALG_OID)),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_CODE_POINT,
                    &xor_sigalg.code_point),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_SECURITY_BITS,
                    &xor_sigalg.secbits),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MIN_TLS,
                   &xor_sigalg.mintls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MAX_TLS,
                   &xor_sigalg.maxtls),
    OSSL_PARAM_END
};

static const OSSL_PARAM xor_sig_hash_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_IANA_NAME,
                           XORSIGALG_HASH_NAME, sizeof(XORSIGALG_HASH_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_NAME,
                           XORSIGALG_HASH_NAME,
                           sizeof(XORSIGALG_HASH_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_HASH_NAME,
                           XORSIGALG_HASH, sizeof(XORSIGALG_HASH)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_OID,
                           XORSIGALG_HASH_OID, sizeof(XORSIGALG_HASH_OID)),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_CODE_POINT,
                    &xor_sigalg_hash.code_point),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_SECURITY_BITS,
                    &xor_sigalg_hash.secbits),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MIN_TLS,
                   &xor_sigalg_hash.mintls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MAX_TLS,
                   &xor_sigalg_hash.maxtls),
    OSSL_PARAM_END
};

static const OSSL_PARAM xor_sig_12_params[] = {
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_IANA_NAME,
                           XORSIGALG12_NAME, sizeof(XORSIGALG12_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_NAME,
                           XORSIGALG12_NAME,
                           sizeof(XORSIGALG12_NAME)),
    OSSL_PARAM_utf8_string(OSSL_CAPABILITY_TLS_SIGALG_OID,
                           XORSIGALG12_OID, sizeof(XORSIGALG12_OID)),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_CODE_POINT,
                    &xor_sigalg12.code_point),
    OSSL_PARAM_uint(OSSL_CAPABILITY_TLS_SIGALG_SECURITY_BITS,
                    &xor_sigalg12.secbits),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MIN_TLS,
                   &xor_sigalg12.mintls),
    OSSL_PARAM_int(OSSL_CAPABILITY_TLS_SIGALG_MAX_TLS,
                   &xor_sigalg12.maxtls),
    OSSL_PARAM_END
};

static int tls_prov_get_capabilities(void *provctx, const char *capability,
                                     OSSL_CALLBACK *cb, void *arg)
{
    int ret = 0;
    int i;
    const char *dummy_base = "dummy";
    const size_t dummy_name_max_size = strlen(dummy_base) + 3;

    if (strcmp(capability, "TLS-GROUP") == 0) {
        /* Register our 2 groups */
        OPENSSL_assert(xor_group.group_id >= 65024
                       && xor_group.group_id < 65279 - NUM_DUMMY_GROUPS);
        ret = cb(xor_group_params, arg);
        ret &= cb(xor_kemgroup_params, arg);

        /*
         * Now register some dummy groups > GROUPLIST_INCREMENT (== 40) as defined
         * in ssl/t1_lib.c, to make sure we exercise the code paths for registering
         * large numbers of groups.
         */

        for (i = 0; i < NUM_DUMMY_GROUPS; i++) {
            OSSL_PARAM dummygroup[OSSL_NELEM(xor_group_params)];
            unsigned int dummygroup_id;

            memcpy(dummygroup, xor_group_params, sizeof(xor_group_params));

            /* Give the dummy group a unique name */
            if (dummy_group_names[i] == NULL) {
                dummy_group_names[i] = OPENSSL_zalloc(dummy_name_max_size);
                if (dummy_group_names[i] == NULL)
                    return 0;
                BIO_snprintf(dummy_group_names[i],
                         dummy_name_max_size,
                         "%s%d", dummy_base, i);
            }
            dummygroup[0].data = dummy_group_names[i];
            dummygroup[0].data_size = strlen(dummy_group_names[i]) + 1;
            /* assign unique group IDs also to dummy groups for registration */
            dummygroup_id = 65279 - NUM_DUMMY_GROUPS + i;
            dummygroup[3].data = (unsigned char*)&dummygroup_id;
            ret &= cb(dummygroup, arg);
        }
    }

    if (strcmp(capability, "TLS-SIGALG") == 0) {
        ret = cb(xor_sig_nohash_params, arg);
        ret &= cb(xor_sig_hash_params, arg);
        ret &= cb(xor_sig_12_params, arg);
    }
    return ret;
}

typedef struct {
    OSSL_LIB_CTX *libctx;
} PROV_XOR_CTX;

static PROV_XOR_CTX *xor_newprovctx(OSSL_LIB_CTX *libctx)
{
    PROV_XOR_CTX* prov_ctx = OPENSSL_malloc(sizeof(PROV_XOR_CTX));

    if (prov_ctx == NULL)
        return NULL;

    if (libctx == NULL) {
        OPENSSL_free(prov_ctx);
        return NULL;
    }
    prov_ctx->libctx = libctx;
    return prov_ctx;
}



#define PROV_XOR_LIBCTX_OF(provctx) (((PROV_XOR_CTX *)provctx)->libctx)

/*
 * Dummy "XOR" Key Exchange and signature algorithm. We just xor the
 * private and public keys together. Don't use this!
 */

typedef struct {
    XORKEY *key;
    XORKEY *peerkey;
    void *provctx;
} PROV_XORKEMKEX_CTX;

static void *xor_newkemkexctx(void *provctx)
{
    PROV_XORKEMKEX_CTX *pxorctx = OPENSSL_zalloc(sizeof(PROV_XORKEMKEX_CTX));

    if (pxorctx == NULL)
        return NULL;

    pxorctx->provctx = provctx;

    return pxorctx;
}

static int xor_init(void *vpxorctx, void *vkey,
                    ossl_unused const OSSL_PARAM params[])
{
    PROV_XORKEMKEX_CTX *pxorctx = (PROV_XORKEMKEX_CTX *)vpxorctx;

    if (pxorctx == NULL || vkey == NULL)
        return 0;
    pxorctx->key = vkey;
    return 1;
}

static int xor_set_peer(void *vpxorctx, void *vpeerkey)
{
    PROV_XORKEMKEX_CTX *pxorctx = (PROV_XORKEMKEX_CTX *)vpxorctx;

    if (pxorctx == NULL || vpeerkey == NULL)
        return 0;
    pxorctx->peerkey = vpeerkey;
    return 1;
}

static int xor_derive(void *vpxorctx, unsigned char *secret, size_t *secretlen,
                      size_t outlen)
{
    PROV_XORKEMKEX_CTX *pxorctx = (PROV_XORKEMKEX_CTX *)vpxorctx;
    int i;

    if (pxorctx->key == NULL || pxorctx->peerkey == NULL)
        return 0;

    *secretlen = XOR_KEY_SIZE;
    if (secret == NULL)
        return 1;

    if (outlen < XOR_KEY_SIZE)
        return 0;

    for (i = 0; i < XOR_KEY_SIZE; i++)
        secret[i] = pxorctx->key->privkey[i] ^ pxorctx->peerkey->pubkey[i];

    return 1;
}

static void xor_freectx(void *pxorctx)
{
    OPENSSL_free(pxorctx);
}

static void *xor_dupctx(void *vpxorctx)
{
    PROV_XORKEMKEX_CTX *srcctx = (PROV_XORKEMKEX_CTX *)vpxorctx;
    PROV_XORKEMKEX_CTX *dstctx;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;

    return dstctx;
}

static const OSSL_DISPATCH xor_keyexch_functions[] = {
    { OSSL_FUNC_KEYEXCH_NEWCTX, (void (*)(void))xor_newkemkexctx },
    { OSSL_FUNC_KEYEXCH_INIT, (void (*)(void))xor_init },
    { OSSL_FUNC_KEYEXCH_DERIVE, (void (*)(void))xor_derive },
    { OSSL_FUNC_KEYEXCH_SET_PEER, (void (*)(void))xor_set_peer },
    { OSSL_FUNC_KEYEXCH_FREECTX, (void (*)(void))xor_freectx },
    { OSSL_FUNC_KEYEXCH_DUPCTX, (void (*)(void))xor_dupctx },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM tls_prov_keyexch[] = {
    /*
     * Obviously this is not FIPS approved, but in order to test in conjunction
     * with the FIPS provider we pretend that it is.
     */
    { "XOR", "provider=tls-provider,fips=yes", xor_keyexch_functions },
    { NULL, NULL, NULL }
};

/*
 * Dummy "XOR" Key Encapsulation Method. We just build a KEM over the xor KEX.
 * Don't use this!
 */

static int xor_encapsulate(void *vpxorctx,
                           unsigned char *ct, size_t *ctlen,
                           unsigned char *ss, size_t *sslen)
{
    /*
     * We are building this around a KEX:
     *
     * 1. we generate ephemeral keypair
     * 2. we encode our ephemeral pubkey as the outgoing ct
     * 3. we derive using our ephemeral privkey in combination with the peer
     *    pubkey from the ctx; the result is our ss.
     */
    int rv = 0;
    void *genctx = NULL, *derivectx = NULL;
    XORKEY *ourkey = NULL;
    PROV_XORKEMKEX_CTX *pxorctx = vpxorctx;

    if (ct == NULL || ss == NULL) {
        /* Just return sizes */

        if (ctlen == NULL && sslen == NULL)
            return 0;
        if (ctlen != NULL)
            *ctlen = XOR_KEY_SIZE;
        if (sslen != NULL)
            *sslen = XOR_KEY_SIZE;
        return 1;
    }

    /* 1. Generate keypair */
    genctx = xor_gen_init(pxorctx->provctx, OSSL_KEYMGMT_SELECT_KEYPAIR, NULL);
    if (genctx == NULL)
        goto end;
    ourkey = xor_gen(genctx, NULL, NULL);
    if (ourkey == NULL)
        goto end;

    /* 2. Encode ephemeral pubkey as ct */
    memcpy(ct, ourkey->pubkey, XOR_KEY_SIZE);
    *ctlen = XOR_KEY_SIZE;

    /* 3. Derive ss via KEX */
    derivectx = xor_newkemkexctx(pxorctx->provctx);
    if (derivectx == NULL
            || !xor_init(derivectx, ourkey, NULL)
            || !xor_set_peer(derivectx, pxorctx->key)
            || !xor_derive(derivectx, ss, sslen, XOR_KEY_SIZE))
        goto end;

    rv = 1;

 end:
    xor_gen_cleanup(genctx);
    xor_freekey(ourkey);
    xor_freectx(derivectx);
    return rv;
}

static int xor_decapsulate(void *vpxorctx,
                           unsigned char *ss, size_t *sslen,
                           const unsigned char *ct, size_t ctlen)
{
    /*
     * We are building this around a KEX:
     *
     * - ct is our peer's pubkey
     * - decapsulate is just derive.
     */
    int rv = 0;
    void *derivectx = NULL;
    XORKEY *peerkey = NULL;
    PROV_XORKEMKEX_CTX *pxorctx = vpxorctx;

    if (ss == NULL) {
        /* Just return size */
        if (sslen == NULL)
            return 0;
        *sslen = XOR_KEY_SIZE;
        return 1;
    }

    if (ctlen != XOR_KEY_SIZE)
        return 0;
    peerkey = xor_newkey(pxorctx->provctx);
    if (peerkey == NULL)
        goto end;
    memcpy(peerkey->pubkey, ct, XOR_KEY_SIZE);

    /* Derive ss via KEX */
    derivectx = xor_newkemkexctx(pxorctx->provctx);
    if (derivectx == NULL
            || !xor_init(derivectx, pxorctx->key, NULL)
            || !xor_set_peer(derivectx, peerkey)
            || !xor_derive(derivectx, ss, sslen, XOR_KEY_SIZE))
        goto end;

    rv = 1;

 end:
    xor_freekey(peerkey);
    xor_freectx(derivectx);
    return rv;
}

static const OSSL_DISPATCH xor_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (void (*)(void))xor_newkemkexctx },
    { OSSL_FUNC_KEM_FREECTX, (void (*)(void))xor_freectx },
    { OSSL_FUNC_KEM_DUPCTX, (void (*)(void))xor_dupctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT, (void (*)(void))xor_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (void (*)(void))xor_encapsulate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT, (void (*)(void))xor_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (void (*)(void))xor_decapsulate },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM tls_prov_kem[] = {
    /*
     * Obviously this is not FIPS approved, but in order to test in conjunction
     * with the FIPS provider we pretend that it is.
     */
    { "XOR", "provider=tls-provider,fips=yes", xor_kem_functions },
    { NULL, NULL, NULL }
};

/* Key Management for the dummy XOR key exchange algorithm */

static void *xor_newkey(void *provctx)
{
    XORKEY *ret = OPENSSL_zalloc(sizeof(XORKEY));

    if (ret == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&ret->references, 1)) {
        OPENSSL_free(ret);
        return NULL;
    }

    return ret;
}

static void xor_freekey(void *keydata)
{
    XORKEY* key = (XORKEY *)keydata;
    int refcnt;

    if (key == NULL)
        return;

    if (CRYPTO_DOWN_REF(&key->references, &refcnt) <= 0)
        return;

    if (refcnt > 0)
        return;
    assert(refcnt == 0);

    if (key != NULL) {
        OPENSSL_free(key->tls_name);
        key->tls_name = NULL;
    }
    CRYPTO_FREE_REF(&key->references);
    OPENSSL_free(key);
}

static int xor_key_up_ref(XORKEY *key)
{
    int refcnt;

    if (CRYPTO_UP_REF(&key->references, &refcnt) <= 0)
        return 0;

    assert(refcnt > 1);
    return (refcnt > 1);
}

static int xor_has(const void *vkey, int selection)
{
    const XORKEY *key = vkey;
    int ok = 0;

    if (key != NULL) {
        ok = 1;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
            ok = ok && key->haspubkey;
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
            ok = ok && key->hasprivkey;
    }
    return ok;
}

static void *xor_dup(const void *vfromkey, int selection)
{
    XORKEY *tokey = xor_newkey(NULL);
    const XORKEY *fromkey = vfromkey;
    int ok = 0;

    if (tokey != NULL && fromkey != NULL) {
        ok = 1;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            if (fromkey->haspubkey) {
                memcpy(tokey->pubkey, fromkey->pubkey, XOR_KEY_SIZE);
                tokey->haspubkey = 1;
            } else {
                tokey->haspubkey = 0;
            }
        }
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            if (fromkey->hasprivkey) {
                memcpy(tokey->privkey, fromkey->privkey, XOR_KEY_SIZE);
                tokey->hasprivkey = 1;
            } else {
                tokey->hasprivkey = 0;
            }
        }
        if (fromkey->tls_name != NULL)
            tokey->tls_name = OPENSSL_strdup(fromkey->tls_name);
    }
    if (!ok) {
        xor_freekey(tokey);
        tokey = NULL;
    }
    return tokey;
}

static ossl_inline int xor_get_params(void *vkey, OSSL_PARAM params[])
{
    XORKEY *key = vkey;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, XOR_KEY_SIZE))
        return 0;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, xor_group.secbits))
        return 0;

    if ((p = OSSL_PARAM_locate(params,
                               OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY)) != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;
        p->return_size = XOR_KEY_SIZE;
        if (p->data != NULL && p->data_size >= XOR_KEY_SIZE)
            memcpy(p->data, key->pubkey, XOR_KEY_SIZE);
    }

    return 1;
}

static const OSSL_PARAM xor_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *xor_gettable_params(void *provctx)
{
    return xor_params;
}

static int xor_set_params(void *vkey, const OSSL_PARAM params[])
{
    XORKEY *key = vkey;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING
                || p->data_size != XOR_KEY_SIZE)
            return 0;
        memcpy(key->pubkey, p->data, XOR_KEY_SIZE);
        key->haspubkey = 1;
    }

    return 1;
}

static const OSSL_PARAM xor_known_settable_params[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static void *xor_load(const void *reference, size_t reference_sz)
{
    XORKEY *key = NULL;

    if (reference_sz == sizeof(key)) {
        /* The contents of the reference is the address to our object */
        key = *(XORKEY **)reference;
        /* We grabbed, so we detach it */
        *(XORKEY **)reference = NULL;
        return key;
    }
    return NULL;
}

/* check one key is the "XOR complement" of the other */
static int xor_recreate(const unsigned char *kd1, const unsigned char *kd2) {
    int i;

    for (i = 0; i < XOR_KEY_SIZE; i++) {
        if ((kd1[i] & 0xff) != ((kd2[i] ^ private_constant[i]) & 0xff))
            return 0;
    }
    return 1;
}

static int xor_match(const void *keydata1, const void *keydata2, int selection)
{
    const XORKEY *key1 = keydata1;
    const XORKEY *key2 = keydata2;
    int ok = 1;

    if (key1->tls_name != NULL && key2->tls_name != NULL)
        ok = ok & (strcmp(key1->tls_name, key2->tls_name) == 0);

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)  {
        if (key1->hasprivkey) {
            if (key2->hasprivkey)
                ok = ok & (CRYPTO_memcmp(key1->privkey, key2->privkey,
                                         XOR_KEY_SIZE) == 0);
            else
                ok = ok & xor_recreate(key1->privkey, key2->pubkey);
        } else {
            if (key2->hasprivkey)
                ok = ok & xor_recreate(key2->privkey, key1->pubkey);
            else
                ok = 0;
        }
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)  {
        if (key1->haspubkey) {
            if (key2->haspubkey)
                ok = ok & (CRYPTO_memcmp(key1->pubkey, key2->pubkey, XOR_KEY_SIZE) == 0);
            else
                ok = ok & xor_recreate(key1->pubkey, key2->privkey);
        } else {
            if (key2->haspubkey)
                ok = ok & xor_recreate(key2->pubkey, key1->privkey);
            else
                ok = 0;
        }
    }

    return ok;
}

static const OSSL_PARAM *xor_settable_params(void *provctx)
{
    return xor_known_settable_params;
}

struct xor_gen_ctx {
    int selection;
    OSSL_LIB_CTX *libctx;
};

static void *xor_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[])
{
    struct xor_gen_ctx *gctx = NULL;

    if ((selection & (OSSL_KEYMGMT_SELECT_KEYPAIR
                      | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS)) == 0)
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) == NULL)
        return NULL;

    gctx->selection = selection;
    gctx->libctx = PROV_XOR_LIBCTX_OF(provctx);

    if (!xor_gen_set_params(gctx, params)) {
        OPENSSL_free(gctx);
        return NULL;
    }
    return gctx;
}

static int xor_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct xor_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_GROUP_NAME);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
                || (strcmp(p->data, XORGROUP_NAME_INTERNAL) != 0
                    &&  strcmp(p->data, XORKEMGROUP_NAME_INTERNAL) != 0))
            return 0;
    }

    return 1;
}

static const OSSL_PARAM *xor_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static void *xor_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct xor_gen_ctx *gctx = genctx;
    XORKEY *key = xor_newkey(NULL);
    size_t i;

    if (key == NULL)
        return NULL;

    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if (RAND_bytes_ex(gctx->libctx, key->privkey, XOR_KEY_SIZE, 0) <= 0) {
            OPENSSL_free(key);
            return NULL;
        }
        for (i = 0; i < XOR_KEY_SIZE; i++)
            key->pubkey[i] = key->privkey[i] ^ private_constant[i];
        key->hasprivkey = 1;
        key->haspubkey = 1;
    }

    return key;
}

/* IMPORT + EXPORT */

static int xor_import(void *vkey, int select, const OSSL_PARAM params[])
{
    XORKEY *key = vkey;
    const OSSL_PARAM *param_priv_key, *param_pub_key;
    unsigned char privkey[XOR_KEY_SIZE];
    unsigned char pubkey[XOR_KEY_SIZE];
    void *pprivkey = privkey, *ppubkey = pubkey;
    size_t priv_len = 0, pub_len = 0;
    int res = 0;

    if (key == NULL || (select & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    memset(privkey, 0, sizeof(privkey));
    memset(pubkey, 0, sizeof(pubkey));
    param_priv_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    param_pub_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);

    if ((param_priv_key != NULL
         && !OSSL_PARAM_get_octet_string(param_priv_key, &pprivkey,
                                         sizeof(privkey), &priv_len))
        || (param_pub_key != NULL
            && !OSSL_PARAM_get_octet_string(param_pub_key, &ppubkey,
                                            sizeof(pubkey), &pub_len)))
        goto err;

    if (priv_len > 0) {
        memcpy(key->privkey, privkey, priv_len);
        key->hasprivkey = 1;
    }
    if (pub_len > 0) {
        memcpy(key->pubkey, pubkey, pub_len);
        key->haspubkey = 1;
    }
    res = 1;
 err:
    return res;
}

static int xor_export(void *vkey, int select, OSSL_CALLBACK *param_cb,
                      void *cbarg)
{
    XORKEY *key = vkey;
    OSSL_PARAM params[3], *p = params;

    if (key == NULL || (select & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PRIV_KEY,
                                             key->privkey,
                                             sizeof(key->privkey));
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PUB_KEY,
                                             key->pubkey, sizeof(key->pubkey));
    *p++ = OSSL_PARAM_construct_end();

    return param_cb(params, cbarg);
}

static const OSSL_PARAM xor_key_types[] = {
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *xor_import_types(int select)
{
    return (select & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0 ? xor_key_types : NULL;
}

static const OSSL_PARAM *xor_import_types_ex(void *provctx, int select)
{
    if (provctx == NULL)
        return NULL;

    return xor_import_types(select);
}

static const OSSL_PARAM *xor_export_types(int select)
{
    return (select & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0 ? xor_key_types : NULL;
}

static const OSSL_PARAM *xor_export_types_ex(void *provctx, int select)
{
    if (provctx == NULL)
        return NULL;

    return xor_export_types(select);
}

static void xor_gen_cleanup(void *genctx)
{
    OPENSSL_free(genctx);
}

static const OSSL_DISPATCH xor_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))xor_newkey },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))xor_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))xor_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))xor_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))xor_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))xor_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))xor_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))xor_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))xor_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))xor_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))xor_has },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))xor_dup },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))xor_freekey },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))xor_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))xor_import_types },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES_EX, (void (*)(void))xor_import_types_ex },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))xor_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))xor_export_types },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES_EX, (void (*)(void))xor_export_types_ex },
    OSSL_DISPATCH_END
};

/* We're reusing most XOR keymgmt functions also for signature operations: */
static void *xor_xorhmacsig_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    XORKEY *k = xor_gen(genctx, osslcb, cbarg);

    if (k == NULL)
        return NULL;
    k->tls_name = OPENSSL_strdup(XORSIGALG_NAME);
    if (k->tls_name == NULL) {
        xor_freekey(k);
        return NULL;
    }
    return k;
}

static void *xor_xorhmacsha2sig_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    XORKEY* k = xor_gen(genctx, osslcb, cbarg);

    if (k == NULL)
        return NULL;
    k->tls_name = OPENSSL_strdup(XORSIGALG_HASH_NAME);
    if (k->tls_name == NULL) {
        xor_freekey(k);
        return NULL;
    }
    return k;
}


static const OSSL_DISPATCH xor_xorhmacsig_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))xor_newkey },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))xor_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))xor_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))xor_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))xor_xorhmacsig_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))xor_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))xor_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))xor_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))xor_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))xor_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))xor_has },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))xor_dup },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))xor_freekey },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))xor_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))xor_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))xor_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))xor_export_types },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))xor_load },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))xor_match },
    OSSL_DISPATCH_END
};

static const OSSL_DISPATCH xor_xorhmacsha2sig_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))xor_newkey },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))xor_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))xor_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))xor_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))xor_xorhmacsha2sig_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))xor_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))xor_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))xor_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))xor_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))xor_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))xor_has },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))xor_dup },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))xor_freekey },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))xor_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))xor_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))xor_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))xor_export_types },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))xor_load },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))xor_match },
    OSSL_DISPATCH_END
};

typedef enum {
    KEY_OP_PUBLIC,
    KEY_OP_PRIVATE,
    KEY_OP_KEYGEN
} xor_key_op_t;

/* Re-create XORKEY from encoding(s): Same end-state as after key-gen */
static XORKEY *xor_key_op(const X509_ALGOR *palg,
                          const unsigned char *p, int plen,
                          xor_key_op_t op,
                          OSSL_LIB_CTX *libctx, const char *propq)
{
    XORKEY *key = NULL;
    int nid = NID_undef;

    if (palg != NULL) {
        int ptype;

        /* Algorithm parameters must be absent */
        X509_ALGOR_get0(NULL, &ptype, NULL, palg);
        if (ptype != V_ASN1_UNDEF || palg->algorithm == NULL) {
            ERR_raise(ERR_LIB_USER, XORPROV_R_INVALID_ENCODING);
            return 0;
        }
        nid = OBJ_obj2nid(palg->algorithm);
    }

    if (p == NULL || nid == EVP_PKEY_NONE || nid == NID_undef) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_INVALID_ENCODING);
        return 0;
    }

    key = xor_newkey(NULL);
    if (key == NULL) {
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
        return 0;
    }

    if (XOR_KEY_SIZE != plen) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_INVALID_ENCODING);
        goto err;
    }

    if (op == KEY_OP_PUBLIC) {
        memcpy(key->pubkey, p, plen);
        key->haspubkey = 1;
    } else {
        memcpy(key->privkey, p, plen);
        key->hasprivkey = 1;
    }

    key->tls_name = OPENSSL_strdup(OBJ_nid2sn(nid));
    if (key->tls_name == NULL)
        goto err;
    return key;

 err:
    xor_freekey(key);
    return NULL;
}

static XORKEY *xor_key_from_x509pubkey(const X509_PUBKEY *xpk,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *p;
    int plen;
    X509_ALGOR *palg;

    if (!xpk || (!X509_PUBKEY_get0_param(NULL, &p, &plen, &palg, xpk))) {
        return NULL;
    }
    return xor_key_op(palg, p, plen, KEY_OP_PUBLIC, libctx, propq);
}

static XORKEY *xor_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    XORKEY *xork = NULL;
    const unsigned char *p;
    int plen;
    ASN1_OCTET_STRING *oct = NULL;
    const X509_ALGOR *palg;

    if (!PKCS8_pkey_get0(NULL, &p, &plen, &palg, p8inf))
        return 0;

    oct = d2i_ASN1_OCTET_STRING(NULL, &p, plen);
    if (oct == NULL) {
        p = NULL;
        plen = 0;
    } else {
        p = ASN1_STRING_get0_data(oct);
        plen = ASN1_STRING_length(oct);
    }

    xork = xor_key_op(palg, p, plen, KEY_OP_PRIVATE,
                      libctx, propq);
    ASN1_OCTET_STRING_free(oct);
    return xork;
}

static const OSSL_ALGORITHM tls_prov_keymgmt[] = {
    /*
     * Obviously this is not FIPS approved, but in order to test in conjunction
     * with the FIPS provider we pretend that it is.
     */
    { "XOR", "provider=tls-provider,fips=yes",
             xor_keymgmt_functions },
    { XORSIGALG_NAME, "provider=tls-provider,fips=yes",
             xor_xorhmacsig_keymgmt_functions },
    { XORSIGALG_HASH_NAME,
    "provider=tls-provider,fips=yes",
             xor_xorhmacsha2sig_keymgmt_functions },
    { NULL, NULL, NULL }
};

struct key2any_ctx_st {
    PROV_XOR_CTX *provctx;

    /* Set to 0 if parameters should not be saved (dsa only) */
    int save_parameters;

    /* Set to 1 if intending to encrypt/decrypt, otherwise 0 */
    int cipher_intent;

    EVP_CIPHER *cipher;

    OSSL_PASSPHRASE_CALLBACK *pwcb;
    void *pwcbarg;
};

typedef int check_key_type_fn(const void *key, int nid);
typedef int key_to_paramstring_fn(const void *key, int nid, int save,
                                  void **str, int *strtype);
typedef int key_to_der_fn(BIO *out, const void *key,
                          int key_nid, const char *pemname,
                          key_to_paramstring_fn *p2s, i2d_of_void *k2d,
                          struct key2any_ctx_st *ctx);
typedef int write_bio_of_void_fn(BIO *bp, const void *x);


/* Free the blob allocated during key_to_paramstring_fn */
static void free_asn1_data(int type, void *data)
{
    switch(type) {
    case V_ASN1_OBJECT:
        ASN1_OBJECT_free(data);
        break;
    case V_ASN1_SEQUENCE:
        ASN1_STRING_free(data);
        break;
    }
}

static PKCS8_PRIV_KEY_INFO *key_to_p8info(const void *key, int key_nid,
                                          void *params, int params_type,
                                          i2d_of_void *k2d)
{
    /* der, derlen store the key DER output and its length */
    unsigned char *der = NULL;
    int derlen;
    /* The final PKCS#8 info */
    PKCS8_PRIV_KEY_INFO *p8info = NULL;

    if ((p8info = PKCS8_PRIV_KEY_INFO_new()) == NULL
        || (derlen = k2d(key, &der)) <= 0
        || !PKCS8_pkey_set0(p8info, OBJ_nid2obj(key_nid), 0,
                            V_ASN1_UNDEF, NULL,
                            der, derlen)) {
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
        PKCS8_PRIV_KEY_INFO_free(p8info);
        OPENSSL_free(der);
        p8info = NULL;
    }

    return p8info;
}

static X509_SIG *p8info_to_encp8(PKCS8_PRIV_KEY_INFO *p8info,
                                 struct key2any_ctx_st *ctx)
{
    X509_SIG *p8 = NULL;
    char kstr[PEM_BUFSIZE];
    size_t klen = 0;
    OSSL_LIB_CTX *libctx = PROV_XOR_LIBCTX_OF(ctx->provctx);

    if (ctx->cipher == NULL || ctx->pwcb == NULL)
        return NULL;

    if (!ctx->pwcb(kstr, PEM_BUFSIZE, &klen, NULL, ctx->pwcbarg)) {
        ERR_raise(ERR_LIB_USER, PROV_R_UNABLE_TO_GET_PASSPHRASE);
        return NULL;
    }
    /* First argument == -1 means "standard" */
    p8 = PKCS8_encrypt_ex(-1, ctx->cipher, kstr, klen, NULL, 0, 0, p8info, libctx, NULL);
    OPENSSL_cleanse(kstr, klen);
    return p8;
}

static X509_SIG *key_to_encp8(const void *key, int key_nid,
                              void *params, int params_type,
                              i2d_of_void *k2d, struct key2any_ctx_st *ctx)
{
    PKCS8_PRIV_KEY_INFO *p8info =
        key_to_p8info(key, key_nid, params, params_type, k2d);
    X509_SIG *p8 = NULL;

    if (p8info == NULL) {
        free_asn1_data(params_type, params);
    } else {
        p8 = p8info_to_encp8(p8info, ctx);
        PKCS8_PRIV_KEY_INFO_free(p8info);
    }
    return p8;
}

static X509_PUBKEY *xorx_key_to_pubkey(const void *key, int key_nid,
                                  void *params, int params_type,
                                  i2d_of_void k2d)
{
    /* der, derlen store the key DER output and its length */
    unsigned char *der = NULL;
    int derlen;
    /* The final X509_PUBKEY */
    X509_PUBKEY *xpk = NULL;

    if ((xpk = X509_PUBKEY_new()) == NULL
        || (derlen = k2d(key, &der)) <= 0
        || !X509_PUBKEY_set0_param(xpk, OBJ_nid2obj(key_nid),
                        V_ASN1_UNDEF, NULL,
                        der, derlen)) {
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
        X509_PUBKEY_free(xpk);
        OPENSSL_free(der);
        xpk = NULL;
    }

    return xpk;
}

/*
 * key_to_epki_* produce encoded output with the private key data in a
 * EncryptedPrivateKeyInfo structure (defined by PKCS#8).  They require
 * that there's an intent to encrypt, anything else is an error.
 *
 * key_to_pki_* primarily produce encoded output with the private key data
 * in a PrivateKeyInfo structure (also defined by PKCS#8).  However, if
 * there is an intent to encrypt the data, the corresponding key_to_epki_*
 * function is used instead.
 *
 * key_to_spki_* produce encoded output with the public key data in an
 * X.509 SubjectPublicKeyInfo.
 *
 * Key parameters don't have any defined envelopment of this kind, but are
 * included in some manner in the output from the functions described above,
 * either in the AlgorithmIdentifier's parameter field, or as part of the
 * key data itself.
 */

static int key_to_epki_der_priv_bio(BIO *out, const void *key,
                                    int key_nid,
                                    ossl_unused const char *pemname,
                                    key_to_paramstring_fn *p2s,
                                    i2d_of_void *k2d,
                                    struct key2any_ctx_st *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_SIG *p8;

    if (!ctx->cipher_intent)
        return 0;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8 = key_to_encp8(key, key_nid, str, strtype, k2d, ctx);
    if (p8 != NULL)
        ret = i2d_PKCS8_bio(out, p8);

    X509_SIG_free(p8);

    return ret;
}

static int key_to_epki_pem_priv_bio(BIO *out, const void *key,
                                    int key_nid,
                                    ossl_unused const char *pemname,
                                    key_to_paramstring_fn *p2s,
                                    i2d_of_void *k2d,
                                    struct key2any_ctx_st *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_SIG *p8;

    if (!ctx->cipher_intent)
        return 0;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8 = key_to_encp8(key, key_nid, str, strtype, k2d, ctx);
    if (p8 != NULL)
        ret = PEM_write_bio_PKCS8(out, p8);

    X509_SIG_free(p8);

    return ret;
}

static int key_to_pki_der_priv_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   i2d_of_void *k2d,
                                   struct key2any_ctx_st *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    PKCS8_PRIV_KEY_INFO *p8info;

    if (ctx->cipher_intent)
        return key_to_epki_der_priv_bio(out, key, key_nid, pemname,
                                        p2s, k2d, ctx);

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8info = key_to_p8info(key, key_nid, str, strtype, k2d);

    if (p8info != NULL)
        ret = i2d_PKCS8_PRIV_KEY_INFO_bio(out, p8info);
    else
        free_asn1_data(strtype, str);

    PKCS8_PRIV_KEY_INFO_free(p8info);

    return ret;
}

static int key_to_pki_pem_priv_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   i2d_of_void *k2d,
                                   struct key2any_ctx_st *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    PKCS8_PRIV_KEY_INFO *p8info;

    if (ctx->cipher_intent)
        return key_to_epki_pem_priv_bio(out, key, key_nid, pemname,
                                        p2s, k2d, ctx);

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8info = key_to_p8info(key, key_nid, str, strtype, k2d);

    if (p8info != NULL)
        ret = PEM_write_bio_PKCS8_PRIV_KEY_INFO(out, p8info);
    else
        free_asn1_data(strtype, str);

    PKCS8_PRIV_KEY_INFO_free(p8info);

    return ret;
}

static int key_to_spki_der_pub_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   i2d_of_void *k2d,
                                   struct key2any_ctx_st *ctx)
{
    int ret = 0;
    X509_PUBKEY *xpk = NULL;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    xpk = xorx_key_to_pubkey(key, key_nid, str, strtype, k2d);

    if (xpk != NULL)
        ret = i2d_X509_PUBKEY_bio(out, xpk);

    X509_PUBKEY_free(xpk);
    return ret;
}

static int key_to_spki_pem_pub_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   i2d_of_void *k2d,
                                   struct key2any_ctx_st *ctx)
{
    int ret = 0;
    X509_PUBKEY *xpk = NULL;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    xpk = xorx_key_to_pubkey(key, key_nid, str, strtype, k2d);

    if (xpk != NULL)
        ret = PEM_write_bio_X509_PUBKEY(out, xpk);
    else
        free_asn1_data(strtype, str);

    /* Also frees |str| */
    X509_PUBKEY_free(xpk);
    return ret;
}

/* ---------------------------------------------------------------------- */

static int prepare_xorx_params(const void *xorxkey, int nid, int save,
                             void **pstr, int *pstrtype)
{
    ASN1_OBJECT *params = NULL;
    XORKEY *k = (XORKEY*)xorxkey;

    if (k->tls_name && OBJ_sn2nid(k->tls_name) != nid) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_INVALID_KEY);
        return 0;
    }

    if (nid == NID_undef) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_MISSING_OID);
        return 0;
    }

    params = OBJ_nid2obj(nid);

    if (params == NULL || OBJ_length(params) == 0) {
        /* unexpected error */
        ERR_raise(ERR_LIB_USER, XORPROV_R_MISSING_OID);
        ASN1_OBJECT_free(params);
        return 0;
    }
    *pstr = params;
    *pstrtype = V_ASN1_OBJECT;
    return 1;
}

static int xorx_spki_pub_to_der(const void *vecxkey, unsigned char **pder)
{
    const XORKEY *xorxkey = vecxkey;
    unsigned char *keyblob;
    int retlen;

    if (xorxkey == NULL) {
        ERR_raise(ERR_LIB_USER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    keyblob = OPENSSL_memdup(xorxkey->pubkey, retlen = XOR_KEY_SIZE);
    if (keyblob == NULL) {
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
        return 0;
    }

    *pder = keyblob;
    return retlen;
}

static int xorx_pki_priv_to_der(const void *vecxkey, unsigned char **pder)
{
    XORKEY *xorxkey = (XORKEY *)vecxkey;
    unsigned char* buf = NULL;
    ASN1_OCTET_STRING oct;
    int keybloblen;

    if (xorxkey == NULL) {
        ERR_raise(ERR_LIB_USER, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    buf = OPENSSL_secure_malloc(XOR_KEY_SIZE);
    memcpy(buf, xorxkey->privkey, XOR_KEY_SIZE);

    oct.data = buf;
    oct.length = XOR_KEY_SIZE;
    oct.flags = 0;

    keybloblen = i2d_ASN1_OCTET_STRING(&oct, pder);
    if (keybloblen < 0) {
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
        keybloblen = 0;
    }

    OPENSSL_secure_clear_free(buf, XOR_KEY_SIZE);
    return keybloblen;
}

# define xorx_epki_priv_to_der xorx_pki_priv_to_der

/*
 * XORX only has PKCS#8 / SubjectPublicKeyInfo
 * representation, so we don't define xorx_type_specific_[priv,pub,params]_to_der.
 */

# define xorx_check_key_type            NULL

# define xorhmacsig_evp_type            0
# define xorhmacsig_input_type          XORSIGALG_NAME
# define xorhmacsig_pem_type            XORSIGALG_NAME
# define xorhmacsha2sig_evp_type        0
# define xorhmacsha2sig_input_type      XORSIGALG_HASH_NAME
# define xorhmacsha2sig_pem_type        XORSIGALG_HASH_NAME

/* ---------------------------------------------------------------------- */

static OSSL_FUNC_decoder_newctx_fn key2any_newctx;
static OSSL_FUNC_decoder_freectx_fn key2any_freectx;

static void *key2any_newctx(void *provctx)
{
    struct key2any_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->provctx = provctx;
        ctx->save_parameters = 1;
    }

    return ctx;
}

static void key2any_freectx(void *vctx)
{
    struct key2any_ctx_st *ctx = vctx;

    EVP_CIPHER_free(ctx->cipher);
    OPENSSL_free(ctx);
}

static const OSSL_PARAM *key2any_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_ENCODER_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_ENCODER_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END,
    };

    return settables;
}

static int key2any_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct key2any_ctx_st *ctx = vctx;
    OSSL_LIB_CTX *libctx = PROV_XOR_LIBCTX_OF(ctx->provctx);
    const OSSL_PARAM *cipherp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_CIPHER);
    const OSSL_PARAM *propsp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_PROPERTIES);
    const OSSL_PARAM *save_paramsp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_SAVE_PARAMETERS);

    if (cipherp != NULL) {
        const char *ciphername = NULL;
        const char *props = NULL;

        if (!OSSL_PARAM_get_utf8_string_ptr(cipherp, &ciphername))
            return 0;
        if (propsp != NULL && !OSSL_PARAM_get_utf8_string_ptr(propsp, &props))
            return 0;

        EVP_CIPHER_free(ctx->cipher);
        ctx->cipher = NULL;
        ctx->cipher_intent = ciphername != NULL;
        if (ciphername != NULL
            && ((ctx->cipher =
                 EVP_CIPHER_fetch(libctx, ciphername, props)) == NULL)) {
            return 0;
        }
    }

    if (save_paramsp != NULL) {
        if (!OSSL_PARAM_get_int(save_paramsp, &ctx->save_parameters)) {
            return 0;
        }
    }
    return 1;
}

static int key2any_check_selection(int selection, int selection_mask)
{
    /*
     * The selections are kinda sorta "levels", i.e. each selection given
     * here is assumed to include those following.
     */
    int checks[] = {
        OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
        OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
        OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
    };
    size_t i;

    /* The decoder implementations made here support guessing */
    if (selection == 0)
        return 1;

    for (i = 0; i < OSSL_NELEM(checks); i++) {
        int check1 = (selection & checks[i]) != 0;
        int check2 = (selection_mask & checks[i]) != 0;

        /*
         * If the caller asked for the currently checked bit(s), return
         * whether the decoder description says it's supported.
         */
        if (check1)
            return check2;
    }

    /* This should be dead code, but just to be safe... */
    return 0;
}

static int key2any_encode(struct key2any_ctx_st *ctx, OSSL_CORE_BIO *cout,
                          const void *key, const char* typestr, const char *pemname,
                          key_to_der_fn *writer,
                          OSSL_PASSPHRASE_CALLBACK *pwcb, void *pwcbarg,
                          key_to_paramstring_fn *key2paramstring,
                          i2d_of_void *key2der)
{
    int ret = 0;
    int type = OBJ_sn2nid(typestr);

    if (key == NULL || type <= 0) {
        ERR_raise(ERR_LIB_USER, ERR_R_PASSED_NULL_PARAMETER);
    } else if (writer != NULL) {
        BIO *out = BIO_new_from_core_bio(ctx->provctx->libctx, cout);

        if (out != NULL) {
            ctx->pwcb = pwcb;
            ctx->pwcbarg = pwcbarg;

            ret = writer(out, key, type, pemname, key2paramstring, key2der, ctx);
        }

        BIO_free(out);
    } else {
        ERR_raise(ERR_LIB_USER, ERR_R_PASSED_INVALID_ARGUMENT);
    }
    return ret;
}

#define DO_ENC_PRIVATE_KEY_selection_mask OSSL_KEYMGMT_SELECT_PRIVATE_KEY
#define DO_ENC_PRIVATE_KEY(impl, type, kind, output)                            \
    if ((selection & DO_ENC_PRIVATE_KEY_selection_mask) != 0)                   \
        return key2any_encode(ctx, cout, key, impl##_pem_type,              \
                              impl##_pem_type " PRIVATE KEY",               \
                              key_to_##kind##_##output##_priv_bio,          \
                              cb, cbarg, prepare_##type##_params,           \
                              type##_##kind##_priv_to_der);

#define DO_ENC_PUBLIC_KEY_selection_mask OSSL_KEYMGMT_SELECT_PUBLIC_KEY
#define DO_ENC_PUBLIC_KEY(impl, type, kind, output)                             \
    if ((selection & DO_ENC_PUBLIC_KEY_selection_mask) != 0)                    \
        return key2any_encode(ctx, cout, key, impl##_pem_type,              \
                              impl##_pem_type " PUBLIC KEY",                \
                              key_to_##kind##_##output##_pub_bio,           \
                              cb, cbarg, prepare_##type##_params,           \
                              type##_##kind##_pub_to_der);

#define DO_ENC_PARAMETERS_selection_mask OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
#define DO_ENC_PARAMETERS(impl, type, kind, output)                             \
    if ((selection & DO_ENC_PARAMETERS_selection_mask) != 0)                    \
        return key2any_encode(ctx, cout, key, impl##_pem_type,              \
                              impl##_pem_type " PARAMETERS",                \
                              key_to_##kind##_##output##_param_bio,         \
                              NULL, NULL, NULL,                             \
                              type##_##kind##_params_to_der);

/*-
 * Implement the kinds of output structure that can be produced.  They are
 * referred to by name, and for each name, the following macros are defined
 * (braces not included):
 *
 * DO_{kind}_selection_mask
 *
 *      A mask of selection bits that must not be zero.  This is used as a
 *      selection criterion for each implementation.
 *      This mask must never be zero.
 *
 * DO_{kind}
 *
 *      The performing macro.  It must use the DO_ macros defined above,
 *      always in this order:
 *
 *      - DO_PRIVATE_KEY
 *      - DO_PUBLIC_KEY
 *      - DO_PARAMETERS
 *
 *      Any of those may be omitted, but the relative order must still be
 *      the same.
 */

/*
 * PKCS#8 defines two structures for private keys only:
 * - PrivateKeyInfo             (raw unencrypted form)
 * - EncryptedPrivateKeyInfo    (encrypted wrapping)
 *
 * To allow a certain amount of flexibility, we allow the routines
 * for PrivateKeyInfo to also produce EncryptedPrivateKeyInfo if a
 * passphrase callback has been passed to them.
 */
#define DO_ENC_PrivateKeyInfo_selection_mask DO_ENC_PRIVATE_KEY_selection_mask
#define DO_ENC_PrivateKeyInfo(impl, type, output)                               \
    DO_ENC_PRIVATE_KEY(impl, type, pki, output)

#define DO_ENC_EncryptedPrivateKeyInfo_selection_mask DO_ENC_PRIVATE_KEY_selection_mask
#define DO_ENC_EncryptedPrivateKeyInfo(impl, type, output)                      \
    DO_ENC_PRIVATE_KEY(impl, type, epki, output)

/* SubjectPublicKeyInfo is a structure for public keys only */
#define DO_ENC_SubjectPublicKeyInfo_selection_mask DO_ENC_PUBLIC_KEY_selection_mask
#define DO_ENC_SubjectPublicKeyInfo(impl, type, output)                         \
    DO_ENC_PUBLIC_KEY(impl, type, spki, output)

/*
 * MAKE_ENCODER is the single driver for creating OSSL_DISPATCH tables.
 * It takes the following arguments:
 *
 * impl         This is the key type name that's being implemented.
 * type         This is the type name for the set of functions that implement
 *              the key type.  For example, ed25519, ed448, x25519 and x448
 *              are all implemented with the exact same set of functions.
 * kind         What kind of support to implement.  These translate into
 *              the DO_##kind macros above.
 * output       The output type to implement.  may be der or pem.
 *
 * The resulting OSSL_DISPATCH array gets the following name (expressed in
 * C preprocessor terms) from those arguments:
 *
 * xor_##impl##_to_##kind##_##output##_encoder_functions
 */
#define MAKE_ENCODER(impl, type, kind, output)                              \
    static OSSL_FUNC_encoder_import_object_fn                               \
    impl##_to_##kind##_##output##_import_object;                            \
    static OSSL_FUNC_encoder_free_object_fn                                 \
    impl##_to_##kind##_##output##_free_object;                              \
    static OSSL_FUNC_encoder_encode_fn                                      \
    impl##_to_##kind##_##output##_encode;                                   \
                                                                            \
    static void *                                                           \
    impl##_to_##kind##_##output##_import_object(void *vctx, int selection,  \
                                                const OSSL_PARAM params[])  \
    {                                                                       \
        struct key2any_ctx_st *ctx = vctx;                                  \
                                                                            \
        return xor_prov_import_key(xor_##impl##_keymgmt_functions,          \
                                    ctx->provctx, selection, params);       \
    }                                                                       \
    static void impl##_to_##kind##_##output##_free_object(void *key)        \
    {                                                                       \
        xor_prov_free_key(xor_##impl##_keymgmt_functions, key);             \
    }                                                                       \
    static int impl##_to_##kind##_##output##_does_selection(void *ctx,      \
                                                            int selection)  \
    {                                                                       \
        return key2any_check_selection(selection,                           \
                                       DO_ENC_##kind##_selection_mask);     \
    }                                                                       \
    static int                                                              \
    impl##_to_##kind##_##output##_encode(void *ctx, OSSL_CORE_BIO *cout,    \
                                         const void *key,                   \
                                         const OSSL_PARAM key_abstract[],   \
                                         int selection,                     \
                                         OSSL_PASSPHRASE_CALLBACK *cb,      \
                                         void *cbarg)                       \
    {                                                                       \
        /* We don't deal with abstract objects */                           \
        if (key_abstract != NULL) {                                         \
            ERR_raise(ERR_LIB_USER, ERR_R_PASSED_INVALID_ARGUMENT);         \
            return 0;                                                       \
        }                                                                   \
        DO_ENC_##kind(impl, type, output)                                   \
                                                                            \
        ERR_raise(ERR_LIB_USER, ERR_R_PASSED_INVALID_ARGUMENT);             \
        return 0;                                                           \
    }                                                                       \
    static const OSSL_DISPATCH                                              \
    xor_##impl##_to_##kind##_##output##_encoder_functions[] = {             \
        { OSSL_FUNC_ENCODER_NEWCTX,                                         \
          (void (*)(void))key2any_newctx },                                 \
        { OSSL_FUNC_ENCODER_FREECTX,                                        \
          (void (*)(void))key2any_freectx },                                \
        { OSSL_FUNC_ENCODER_SETTABLE_CTX_PARAMS,                            \
          (void (*)(void))key2any_settable_ctx_params },                    \
        { OSSL_FUNC_ENCODER_SET_CTX_PARAMS,                                 \
          (void (*)(void))key2any_set_ctx_params },                         \
        { OSSL_FUNC_ENCODER_DOES_SELECTION,                                 \
          (void (*)(void))impl##_to_##kind##_##output##_does_selection },   \
        { OSSL_FUNC_ENCODER_IMPORT_OBJECT,                                  \
          (void (*)(void))impl##_to_##kind##_##output##_import_object },    \
        { OSSL_FUNC_ENCODER_FREE_OBJECT,                                    \
          (void (*)(void))impl##_to_##kind##_##output##_free_object },      \
        { OSSL_FUNC_ENCODER_ENCODE,                                         \
          (void (*)(void))impl##_to_##kind##_##output##_encode },           \
        OSSL_DISPATCH_END                                                   \
    }

/*
 * Replacements for i2d_{TYPE}PrivateKey, i2d_{TYPE}PublicKey,
 * i2d_{TYPE}params, as they exist.
 */

/*
 * PKCS#8 and SubjectPublicKeyInfo support.  This may duplicate some of the
 * implementations specified above, but are more specific.
 * The SubjectPublicKeyInfo implementations also replace the
 * PEM_write_bio_{TYPE}_PUBKEY functions.
 * For PEM, these are expected to be used by PEM_write_bio_PrivateKey(),
 * PEM_write_bio_PUBKEY() and PEM_write_bio_Parameters().
 */

MAKE_ENCODER(xorhmacsig, xorx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(xorhmacsig, xorx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(xorhmacsig, xorx, PrivateKeyInfo, der);
MAKE_ENCODER(xorhmacsig, xorx, PrivateKeyInfo, pem);
MAKE_ENCODER(xorhmacsig, xorx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(xorhmacsig, xorx, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(xorhmacsha2sig, xorx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(xorhmacsha2sig, xorx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(xorhmacsha2sig, xorx, PrivateKeyInfo, der);
MAKE_ENCODER(xorhmacsha2sig, xorx, PrivateKeyInfo, pem);
MAKE_ENCODER(xorhmacsha2sig, xorx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(xorhmacsha2sig, xorx, SubjectPublicKeyInfo, pem);

static const OSSL_ALGORITHM tls_prov_encoder[] = {
#define ENCODER_PROVIDER "tls-provider"
#ifndef ENCODER_PROVIDER
# error Macro ENCODER_PROVIDER undefined
#endif

#define ENCODER_STRUCTURE_PKCS8                         "pkcs8"
#define ENCODER_STRUCTURE_SubjectPublicKeyInfo          "SubjectPublicKeyInfo"
#define ENCODER_STRUCTURE_PrivateKeyInfo                "PrivateKeyInfo"
#define ENCODER_STRUCTURE_EncryptedPrivateKeyInfo       "EncryptedPrivateKeyInfo"
#define ENCODER_STRUCTURE_PKCS1                         "pkcs1"
#define ENCODER_STRUCTURE_PKCS3                         "pkcs3"

/* Arguments are prefixed with '_' to avoid build breaks on certain platforms */
/*
 * Obviously this is not FIPS approved, but in order to test in conjunction
 * with the FIPS provider we pretend that it is.
 */
#define ENCODER_TEXT(_name, _sym)                                \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=yes,output=text",      \
      (xor_##_sym##_to_text_encoder_functions) }
#define ENCODER(_name, _sym, _fips, _output)                            \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=yes,output=" #_output, \
      (xor_##_sym##_to_##_output##_encoder_functions) }

#define ENCODER_w_structure(_name, _sym, _output, _structure)    \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=yes,output=" #_output  \
      ",structure=" ENCODER_STRUCTURE_##_structure,                     \
      (xor_##_sym##_to_##_structure##_##_output##_encoder_functions) }

/*
 * Entries for human text "encoders"
 */

/*
 * Entries for PKCS#8 and SubjectPublicKeyInfo.
 * The "der" ones are added convenience for any user that wants to use
 * OSSL_ENCODER directly.
 * The "pem" ones also support PEM_write_bio_PrivateKey() and
 * PEM_write_bio_PUBKEY().
 */

ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, der, PrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, pem, PrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, der, SubjectPublicKeyInfo),
ENCODER_w_structure(XORSIGALG_NAME, xorhmacsig, pem, SubjectPublicKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    der, PrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    pem, PrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    der, EncryptedPrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    der, SubjectPublicKeyInfo),
ENCODER_w_structure(XORSIGALG_HASH_NAME, xorhmacsha2sig,
                    pem, SubjectPublicKeyInfo),
#undef ENCODER_PROVIDER
    { NULL, NULL, NULL }
};

struct der2key_ctx_st;           /* Forward declaration */
typedef int check_key_fn(void *, struct der2key_ctx_st *ctx);
typedef void adjust_key_fn(void *, struct der2key_ctx_st *ctx);
typedef void free_key_fn(void *);
typedef void *d2i_PKCS8_fn(void **, const unsigned char **, long,
                           struct der2key_ctx_st *);
struct keytype_desc_st {
    const char *keytype_name;
    const OSSL_DISPATCH *fns; /* Keymgmt (to pilfer functions from) */

    /* The input structure name */
    const char *structure_name;

    /*
     * The EVP_PKEY_xxx type macro.  Should be zero for type specific
     * structures, non-zero when the outermost structure is PKCS#8 or
     * SubjectPublicKeyInfo.  This determines which of the function
     * pointers below will be used.
     */
    int evp_type;

    /* The selection mask for OSSL_FUNC_decoder_does_selection() */
    int selection_mask;

    /* For type specific decoders, we use the corresponding d2i */
    d2i_of_void *d2i_private_key; /* From type-specific DER */
    d2i_of_void *d2i_public_key;  /* From type-specific DER */
    d2i_of_void *d2i_key_params;  /* From type-specific DER */
    d2i_PKCS8_fn *d2i_PKCS8;      /* Wrapped in a PrivateKeyInfo */
    d2i_of_void *d2i_PUBKEY;      /* Wrapped in a SubjectPublicKeyInfo */

    /*
     * For any key, we may need to check that the key meets expectations.
     * This is useful when the same functions can decode several variants
     * of a key.
     */
    check_key_fn *check_key;

    /*
     * For any key, we may need to make provider specific adjustments, such
     * as ensure the key carries the correct library context.
     */
    adjust_key_fn *adjust_key;
    /* {type}_free() */
    free_key_fn *free_key;
};

/*
 * Start blatant code steal. Alternative: Open up d2i_X509_PUBKEY_INTERNAL
 * as per https://github.com/openssl/openssl/issues/16697 (TBD)
 * Code from openssl/crypto/x509/x_pubkey.c as
 * ossl_d2i_X509_PUBKEY_INTERNAL is presently not public
 */
struct X509_pubkey_st {
    X509_ALGOR *algor;
    ASN1_BIT_STRING *public_key;

    EVP_PKEY *pkey;

    /* extra data for the callback, used by d2i_PUBKEY_ex */
    OSSL_LIB_CTX *libctx;
    char *propq;
};

ASN1_SEQUENCE(X509_PUBKEY_INTERNAL) = {
        ASN1_SIMPLE(X509_PUBKEY, algor, X509_ALGOR),
        ASN1_SIMPLE(X509_PUBKEY, public_key, ASN1_BIT_STRING)
} static_ASN1_SEQUENCE_END_name(X509_PUBKEY, X509_PUBKEY_INTERNAL)

static X509_PUBKEY *xorx_d2i_X509_PUBKEY_INTERNAL(const unsigned char **pp,
                                           long len, OSSL_LIB_CTX *libctx)
{
    X509_PUBKEY *xpub = OPENSSL_zalloc(sizeof(*xpub));

    if (xpub == NULL)
        return NULL;
    return (X509_PUBKEY *)ASN1_item_d2i_ex((ASN1_VALUE **)&xpub, pp, len,
                                           ASN1_ITEM_rptr(X509_PUBKEY_INTERNAL),
                                           libctx, NULL);
}
/* end steal https://github.com/openssl/openssl/issues/16697 */

/*
 * Context used for DER to key decoding.
 */
struct der2key_ctx_st {
    PROV_XOR_CTX *provctx;
    struct keytype_desc_st *desc;
    /* The selection that is passed to xor_der2key_decode() */
    int selection;
    /* Flag used to signal that a failure is fatal */
    unsigned int flag_fatal : 1;
};

static int xor_read_der(PROV_XOR_CTX *provctx, OSSL_CORE_BIO *cin,
                        unsigned char **data, long *len)
{
    BUF_MEM *mem = NULL;
    BIO *in = BIO_new_from_core_bio(provctx->libctx, cin);
    int ok = (asn1_d2i_read_bio(in, &mem) >= 0);

    if (ok) {
        *data = (unsigned char *)mem->data;
        *len = (long)mem->length;
        OPENSSL_free(mem);
    }
    BIO_free(in);
    return ok;
}

typedef void *key_from_pkcs8_t(const PKCS8_PRIV_KEY_INFO *p8inf,
                               OSSL_LIB_CTX *libctx, const char *propq);
static void *xor_der2key_decode_p8(const unsigned char **input_der,
                               long input_der_len, struct der2key_ctx_st *ctx,
                               key_from_pkcs8_t *key_from_pkcs8)
{
    PKCS8_PRIV_KEY_INFO *p8inf = NULL;
    const X509_ALGOR *alg = NULL;
    void *key = NULL;

    if ((p8inf = d2i_PKCS8_PRIV_KEY_INFO(NULL, input_der, input_der_len)) != NULL
        && PKCS8_pkey_get0(NULL, NULL, NULL, &alg, p8inf)
        && OBJ_obj2nid(alg->algorithm) == ctx->desc->evp_type)
        key = key_from_pkcs8(p8inf, PROV_XOR_LIBCTX_OF(ctx->provctx), NULL);
    PKCS8_PRIV_KEY_INFO_free(p8inf);

    return key;
}

static XORKEY *xor_d2i_PUBKEY(XORKEY **a,
                               const unsigned char **pp, long length)
{
    XORKEY *key = NULL;
    X509_PUBKEY *xpk;

    xpk = xorx_d2i_X509_PUBKEY_INTERNAL(pp, length, NULL);

    key = xor_key_from_x509pubkey(xpk, NULL, NULL);

    if (key == NULL)
        goto err_exit;

    if (a != NULL) {
        xor_freekey(*a);
        *a = key;
    }

    err_exit:
    X509_PUBKEY_free(xpk);
    return key;
}


/* ---------------------------------------------------------------------- */

static OSSL_FUNC_decoder_freectx_fn der2key_freectx;
static OSSL_FUNC_decoder_decode_fn xor_der2key_decode;
static OSSL_FUNC_decoder_export_object_fn der2key_export_object;

static struct der2key_ctx_st *
der2key_newctx(void *provctx, struct keytype_desc_st *desc, const char* tls_name)
{
    struct der2key_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->provctx = provctx;
        ctx->desc = desc;
        if (desc->evp_type == 0) {
           ctx->desc->evp_type = OBJ_sn2nid(tls_name);
        }
    }
    return ctx;
}

static void der2key_freectx(void *vctx)
{
    struct der2key_ctx_st *ctx = vctx;

    OPENSSL_free(ctx);
}

static int der2key_check_selection(int selection,
                                   const struct keytype_desc_st *desc)
{
    /*
     * The selections are kinda sorta "levels", i.e. each selection given
     * here is assumed to include those following.
     */
    int checks[] = {
        OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
        OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
        OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
    };
    size_t i;

    /* The decoder implementations made here support guessing */
    if (selection == 0)
        return 1;

    for (i = 0; i < OSSL_NELEM(checks); i++) {
        int check1 = (selection & checks[i]) != 0;
        int check2 = (desc->selection_mask & checks[i]) != 0;

        /*
         * If the caller asked for the currently checked bit(s), return
         * whether the decoder description says it's supported.
         */
        if (check1)
            return check2;
    }

    /* This should be dead code, but just to be safe... */
    return 0;
}

static int xor_der2key_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                          OSSL_CALLBACK *data_cb, void *data_cbarg,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct der2key_ctx_st *ctx = vctx;
    unsigned char *der = NULL;
    const unsigned char *derp;
    long der_len = 0;
    void *key = NULL;
    int ok = 0;

    ctx->selection = selection;
    /*
     * The caller is allowed to specify 0 as a selection mark, to have the
     * structure and key type guessed.  For type-specific structures, this
     * is not recommended, as some structures are very similar.
     * Note that 0 isn't the same as OSSL_KEYMGMT_SELECT_ALL, as the latter
     * signifies a private key structure, where everything else is assumed
     * to be present as well.
     */
    if (selection == 0)
        selection = ctx->desc->selection_mask;
    if ((selection & ctx->desc->selection_mask) == 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    ok = xor_read_der(ctx->provctx, cin, &der, &der_len);
    if (!ok)
        goto next;

    ok = 0;                      /* Assume that we fail */

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        derp = der;
        if (ctx->desc->d2i_PKCS8 != NULL) {
            key = ctx->desc->d2i_PKCS8(NULL, &derp, der_len, ctx);
            if (ctx->flag_fatal)
                goto end;
        } else if (ctx->desc->d2i_private_key != NULL) {
            key = ctx->desc->d2i_private_key(NULL, &derp, der_len);
        }
        if (key == NULL && ctx->selection != 0)
            goto next;
    }
    if (key == NULL && (selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        derp = der;
        if (ctx->desc->d2i_PUBKEY != NULL)
            key = ctx->desc->d2i_PUBKEY(NULL, &derp, der_len);
        else
            key = ctx->desc->d2i_public_key(NULL, &derp, der_len);
        if (key == NULL && ctx->selection != 0)
            goto next;
    }
    if (key == NULL && (selection & OSSL_KEYMGMT_SELECT_ALL_PARAMETERS) != 0) {
        derp = der;
        if (ctx->desc->d2i_key_params != NULL)
            key = ctx->desc->d2i_key_params(NULL, &derp, der_len);
        if (key == NULL && ctx->selection != 0)
            goto next;
    }

    /*
     * Last minute check to see if this was the correct type of key.  This
     * should never lead to a fatal error, i.e. the decoding itself was
     * correct, it was just an unexpected key type.  This is generally for
     * classes of key types that have subtle variants, like RSA-PSS keys as
     * opposed to plain RSA keys.
     */
    if (key != NULL
        && ctx->desc->check_key != NULL
        && !ctx->desc->check_key(key, ctx)) {
        ctx->desc->free_key(key);
        key = NULL;
    }

    if (key != NULL && ctx->desc->adjust_key != NULL)
        ctx->desc->adjust_key(key, ctx);

 next:
    /*
     * Indicated that we successfully decoded something, or not at all.
     * Ending up "empty handed" is not an error.
     */
    ok = 1;

    /*
     * We free memory here so it's not held up during the callback, because
     * we know the process is recursive and the allocated chunks of memory
     * add up.
     */
    OPENSSL_free(der);
    der = NULL;

    if (key != NULL) {
        OSSL_PARAM params[4];
        int object_type = OSSL_OBJECT_PKEY;

        params[0] =
            OSSL_PARAM_construct_int(OSSL_OBJECT_PARAM_TYPE, &object_type);
        params[1] =
            OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                             (char *)ctx->desc->keytype_name,
                                             0);
        /* The address of the key becomes the octet string */
        params[2] =
            OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_REFERENCE,
                                              &key, sizeof(key));
        params[3] = OSSL_PARAM_construct_end();

        ok = data_cb(params, data_cbarg);
    }

 end:
    ctx->desc->free_key(key);
    OPENSSL_free(der);

    return ok;
}

static int der2key_export_object(void *vctx,
                                 const void *reference, size_t reference_sz,
                                 OSSL_CALLBACK *export_cb, void *export_cbarg)
{
    struct der2key_ctx_st *ctx = vctx;
    OSSL_FUNC_keymgmt_export_fn *export =
        xor_prov_get_keymgmt_export(ctx->desc->fns);
    void *keydata;

    if (reference_sz == sizeof(keydata) && export != NULL) {
        /* The contents of the reference is the address to our object */
        keydata = *(void **)reference;

        return export(keydata, ctx->selection, export_cb, export_cbarg);
    }
    return 0;
}

/* ---------------------------------------------------------------------- */

static void *xorx_d2i_PKCS8(void **key, const unsigned char **der, long der_len,
                           struct der2key_ctx_st *ctx)
{
    return xor_der2key_decode_p8(der, der_len, ctx,
                             (key_from_pkcs8_t *)xor_key_from_pkcs8);
}

static void xorx_key_adjust(void *key, struct der2key_ctx_st *ctx)
{
}

/* ---------------------------------------------------------------------- */

#define DO_PrivateKeyInfo(keytype)                      \
    "PrivateKeyInfo", 0,                                \
        ( OSSL_KEYMGMT_SELECT_PRIVATE_KEY ),            \
        NULL,                                           \
        NULL,                                           \
        NULL,                                           \
        xorx_d2i_PKCS8,                                 \
        NULL,                                           \
        NULL,                                           \
        xorx_key_adjust,                                \
        (free_key_fn *)xor_freekey

#define DO_SubjectPublicKeyInfo(keytype)                \
    "SubjectPublicKeyInfo", 0,                          \
        ( OSSL_KEYMGMT_SELECT_PUBLIC_KEY ),             \
        NULL,                                           \
        NULL,                                           \
        NULL,                                           \
        NULL,                                           \
        (d2i_of_void *)xor_d2i_PUBKEY,                  \
        NULL,                                           \
        xorx_key_adjust,                                \
        (free_key_fn *)xor_freekey

/*
 * MAKE_DECODER is the single driver for creating OSSL_DISPATCH tables.
 * It takes the following arguments:
 *
 * keytype_name The implementation key type as a string.
 * keytype      The implementation key type.  This must correspond exactly
 *              to our existing keymgmt keytype names...  in other words,
 *              there must exist an ossl_##keytype##_keymgmt_functions.
 * type         The type name for the set of functions that implement the
 *              decoder for the key type.  This isn't necessarily the same
 *              as keytype.  For example, the key types ed25519, ed448,
 *              x25519 and x448 are all handled by the same functions with
 *              the common type name ecx.
 * kind         The kind of support to implement.  This translates into
 *              the DO_##kind macros above, to populate the keytype_desc_st
 *              structure.
 */
#define MAKE_DECODER(keytype_name, keytype, type, kind)                 \
    static struct keytype_desc_st kind##_##keytype##_desc =             \
        { keytype_name, xor_##keytype##_keymgmt_functions,              \
          DO_##kind(keytype) };                                         \
                                                                        \
    static OSSL_FUNC_decoder_newctx_fn kind##_der2##keytype##_newctx;   \
                                                                        \
    static void *kind##_der2##keytype##_newctx(void *provctx)           \
    {                                                                   \
        return der2key_newctx(provctx, &kind##_##keytype##_desc, keytype_name );\
    }                                                                   \
    static int kind##_der2##keytype##_does_selection(void *provctx,     \
                                                     int selection)     \
    {                                                                   \
        return der2key_check_selection(selection,                       \
                                       &kind##_##keytype##_desc);       \
    }                                                                   \
    static const OSSL_DISPATCH                                          \
    xor_##kind##_der_to_##keytype##_decoder_functions[] = {             \
        { OSSL_FUNC_DECODER_NEWCTX,                                     \
          (void (*)(void))kind##_der2##keytype##_newctx },              \
        { OSSL_FUNC_DECODER_FREECTX,                                    \
          (void (*)(void))der2key_freectx },                            \
        { OSSL_FUNC_DECODER_DOES_SELECTION,                             \
          (void (*)(void))kind##_der2##keytype##_does_selection },      \
        { OSSL_FUNC_DECODER_DECODE,                                     \
          (void (*)(void))xor_der2key_decode },                         \
        { OSSL_FUNC_DECODER_EXPORT_OBJECT,                              \
          (void (*)(void))der2key_export_object },                      \
        OSSL_DISPATCH_END                                               \
    }

MAKE_DECODER(XORSIGALG_NAME, xorhmacsig, xor, PrivateKeyInfo);
MAKE_DECODER(XORSIGALG_NAME, xorhmacsig, xor, SubjectPublicKeyInfo);
MAKE_DECODER(XORSIGALG_HASH_NAME, xorhmacsha2sig, xor, PrivateKeyInfo);
MAKE_DECODER(XORSIGALG_HASH_NAME, xorhmacsha2sig, xor, SubjectPublicKeyInfo);

static const OSSL_ALGORITHM tls_prov_decoder[] = {
#define DECODER_PROVIDER "tls-provider"
#define DECODER_STRUCTURE_SubjectPublicKeyInfo          "SubjectPublicKeyInfo"
#define DECODER_STRUCTURE_PrivateKeyInfo                "PrivateKeyInfo"

/* Arguments are prefixed with '_' to avoid build breaks on certain platforms */
/*
 * Obviously this is not FIPS approved, but in order to test in conjunction
 * with the FIPS provider we pretend that it is.
 */

#define DECODER(_name, _input, _output)                          \
    { _name,                                                            \
      "provider=" DECODER_PROVIDER ",fips=yes,input=" #_input,   \
      (xor_##_input##_to_##_output##_decoder_functions) }
#define DECODER_w_structure(_name, _input, _structure, _output)  \
    { _name,                                                            \
      "provider=" DECODER_PROVIDER ",fips=yes,input=" #_input    \
      ",structure=" DECODER_STRUCTURE_##_structure,                     \
      (xor_##_structure##_##_input##_to_##_output##_decoder_functions) }

DECODER_w_structure(XORSIGALG_NAME, der, PrivateKeyInfo, xorhmacsig),
DECODER_w_structure(XORSIGALG_NAME, der, SubjectPublicKeyInfo, xorhmacsig),
DECODER_w_structure(XORSIGALG_HASH_NAME, der, PrivateKeyInfo, xorhmacsha2sig),
DECODER_w_structure(XORSIGALG_HASH_NAME, der, SubjectPublicKeyInfo, xorhmacsha2sig),
#undef DECODER_PROVIDER
    { NULL, NULL, NULL }
};

#define OSSL_MAX_NAME_SIZE 50
#define OSSL_MAX_PROPQUERY_SIZE     256 /* Property query strings */

static OSSL_FUNC_signature_newctx_fn xor_sig_newctx;
static OSSL_FUNC_signature_sign_init_fn xor_sig_sign_init;
static OSSL_FUNC_signature_verify_init_fn xor_sig_verify_init;
static OSSL_FUNC_signature_sign_fn xor_sig_sign;
static OSSL_FUNC_signature_verify_fn xor_sig_verify;
static OSSL_FUNC_signature_digest_sign_init_fn xor_sig_digest_sign_init;
static OSSL_FUNC_signature_digest_sign_update_fn xor_sig_digest_signverify_update;
static OSSL_FUNC_signature_digest_sign_final_fn xor_sig_digest_sign_final;
static OSSL_FUNC_signature_digest_verify_init_fn xor_sig_digest_verify_init;
static OSSL_FUNC_signature_digest_verify_update_fn xor_sig_digest_signverify_update;
static OSSL_FUNC_signature_digest_verify_final_fn xor_sig_digest_verify_final;
static OSSL_FUNC_signature_freectx_fn xor_sig_freectx;
static OSSL_FUNC_signature_dupctx_fn xor_sig_dupctx;
static OSSL_FUNC_signature_get_ctx_params_fn xor_sig_get_ctx_params;
static OSSL_FUNC_signature_gettable_ctx_params_fn xor_sig_gettable_ctx_params;
static OSSL_FUNC_signature_set_ctx_params_fn xor_sig_set_ctx_params;
static OSSL_FUNC_signature_settable_ctx_params_fn xor_sig_settable_ctx_params;
static OSSL_FUNC_signature_get_ctx_md_params_fn xor_sig_get_ctx_md_params;
static OSSL_FUNC_signature_gettable_ctx_md_params_fn xor_sig_gettable_ctx_md_params;
static OSSL_FUNC_signature_set_ctx_md_params_fn xor_sig_set_ctx_md_params;
static OSSL_FUNC_signature_settable_ctx_md_params_fn xor_sig_settable_ctx_md_params;

static int xor_get_aid(unsigned char** oidbuf, const char *tls_name) {
   X509_ALGOR *algor = X509_ALGOR_new();
   int aidlen = 0;

   X509_ALGOR_set0(algor, OBJ_txt2obj(tls_name, 0), V_ASN1_UNDEF, NULL);

   aidlen = i2d_X509_ALGOR(algor, oidbuf);
   X509_ALGOR_free(algor);
   return(aidlen);
}

/*
 * What's passed as an actual key is defined by the KEYMGMT interface.
 */
typedef struct {
    OSSL_LIB_CTX *libctx;
    char *propq;
    XORKEY *sig;

    /*
     * Flag to determine if the hash function can be changed (1) or not (0)
     * Because it's dangerous to change during a DigestSign or DigestVerify
     * operation, this flag is cleared by their Init function, and set again
     * by their Final function.
     */
    unsigned int flag_allow_md : 1;

    char mdname[OSSL_MAX_NAME_SIZE];

    /* The Algorithm Identifier of the combined signature algorithm */
    unsigned char *aid;
    size_t  aid_len;

    /* main digest */
    EVP_MD *md;
    EVP_MD_CTX *mdctx;
    int operation;
} PROV_XORSIG_CTX;

static void *xor_sig_newctx(void *provctx, const char *propq)
{
    PROV_XORSIG_CTX *pxor_sigctx;

    pxor_sigctx = OPENSSL_zalloc(sizeof(PROV_XORSIG_CTX));
    if (pxor_sigctx == NULL)
        return NULL;

    pxor_sigctx->libctx = ((PROV_XOR_CTX*)provctx)->libctx;
    pxor_sigctx->flag_allow_md = 0;
    if (propq != NULL && (pxor_sigctx->propq = OPENSSL_strdup(propq)) == NULL) {
        OPENSSL_free(pxor_sigctx);
        pxor_sigctx = NULL;
        ERR_raise(ERR_LIB_USER, ERR_R_MALLOC_FAILURE);
    }
    return pxor_sigctx;
}

static int xor_sig_setup_md(PROV_XORSIG_CTX *ctx,
                        const char *mdname, const char *mdprops)
{
    EVP_MD *md;

    if (mdprops == NULL)
        mdprops = ctx->propq;

    md = EVP_MD_fetch(ctx->libctx, mdname, mdprops);

    if ((md == NULL) || (EVP_MD_nid(md)==NID_undef)) {
        if (md == NULL)
            ERR_raise_data(ERR_LIB_USER, XORPROV_R_INVALID_DIGEST,
                           "%s could not be fetched", mdname);
        EVP_MD_free(md);
        return 0;
    }

    EVP_MD_CTX_free(ctx->mdctx);
    ctx->mdctx = NULL;
    EVP_MD_free(ctx->md);
    ctx->md = NULL;

    OPENSSL_free(ctx->aid);
    ctx->aid = NULL;
    ctx->aid_len = xor_get_aid(&(ctx->aid), ctx->sig->tls_name);
    if (ctx->aid_len <= 0) {
        EVP_MD_free(md);
        return 0;
    }

    ctx->mdctx = NULL;
    ctx->md = md;
    OPENSSL_strlcpy(ctx->mdname, mdname, sizeof(ctx->mdname));
    return 1;
}

static int xor_sig_signverify_init(void *vpxor_sigctx, void *vxorsig,
                                   int operation)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx == NULL || vxorsig == NULL)
        return 0;
    xor_freekey(pxor_sigctx->sig);
    if (!xor_key_up_ref(vxorsig))
        return 0;
    pxor_sigctx->sig = vxorsig;
    pxor_sigctx->operation = operation;
    if ((operation==EVP_PKEY_OP_SIGN && pxor_sigctx->sig == NULL)
        || (operation==EVP_PKEY_OP_VERIFY && pxor_sigctx->sig == NULL)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_INVALID_KEY);
        return 0;
    }
    return 1;
}

static int xor_sig_sign_init(void *vpxor_sigctx, void *vxorsig,
                             const OSSL_PARAM params[])
{
    return xor_sig_signverify_init(vpxor_sigctx, vxorsig, EVP_PKEY_OP_SIGN);
}

static int xor_sig_verify_init(void *vpxor_sigctx, void *vxorsig,
                               const OSSL_PARAM params[])
{
    return xor_sig_signverify_init(vpxor_sigctx, vxorsig, EVP_PKEY_OP_VERIFY);
}

static int xor_sig_sign(void *vpxor_sigctx, unsigned char *sig, size_t *siglen,
                    size_t sigsize, const unsigned char *tbs, size_t tbslen)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    XORKEY *xorkey = pxor_sigctx->sig;

    size_t max_sig_len = EVP_MAX_MD_SIZE;
    size_t xor_sig_len = 0;
    int rv = 0;

    if (xorkey == NULL || !xorkey->hasprivkey) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_NO_PRIVATE_KEY);
        return rv;
    }

    if (sig == NULL) {
        *siglen = max_sig_len;
        return 1;
    }
    if (*siglen < max_sig_len) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_BUFFER_LENGTH_WRONG);
        return rv;
    }

    /*
     * create HMAC using XORKEY as key and hash as data:
     * No real crypto, just for test, don't do this at home!
     */
    if (!EVP_Q_mac(pxor_sigctx->libctx, "HMAC", NULL, "sha1", NULL,
                   xorkey->privkey, XOR_KEY_SIZE, tbs, tbslen,
                   &sig[0], EVP_MAX_MD_SIZE, &xor_sig_len)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_SIGNING_FAILED);
        goto endsign;
    }

    *siglen = xor_sig_len;
    rv = 1; /* success */

 endsign:
    return rv;
}

static int xor_sig_verify(void *vpxor_sigctx,
    const unsigned char *sig, size_t siglen,
                          const unsigned char *tbs, size_t tbslen)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    XORKEY *xorkey = pxor_sigctx->sig;
    unsigned char resignature[EVP_MAX_MD_SIZE];
    size_t resiglen;
    int i;

    if (xorkey == NULL || sig == NULL || tbs == NULL) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_WRONG_PARAMETERS);
        return 0;
    }

    /*
     * This is no real verify: just re-sign and compare:
     * Don't do this at home! Not fit for real use!
     */
    /* First re-create private key from public key: */
    for (i = 0; i < XOR_KEY_SIZE; i++)
        xorkey->privkey[i] = xorkey->pubkey[i] ^ private_constant[i];

    /* Now re-create signature */
    if (!EVP_Q_mac(pxor_sigctx->libctx, "HMAC", NULL, "sha1", NULL,
                   xorkey->privkey, XOR_KEY_SIZE, tbs, tbslen,
                   &resignature[0], EVP_MAX_MD_SIZE, &resiglen)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_VERIFY_ERROR);
        return 0;
    }

    /* Now compare with signature passed */
    if (siglen != resiglen || memcmp(resignature, sig, siglen) != 0) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_VERIFY_ERROR);
        return 0;
    }
    return 1;
}

static int xor_sig_digest_signverify_init(void *vpxor_sigctx, const char *mdname,
                                      void *vxorsig, int operation)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    char *rmdname = (char *)mdname;

    if (rmdname == NULL)
        rmdname = "sha256";

    pxor_sigctx->flag_allow_md = 0;
    if (!xor_sig_signverify_init(vpxor_sigctx, vxorsig, operation))
        return 0;

    if (!xor_sig_setup_md(pxor_sigctx, rmdname, NULL))
        return 0;

    pxor_sigctx->mdctx = EVP_MD_CTX_new();
    if (pxor_sigctx->mdctx == NULL)
        goto error;

    if (!EVP_DigestInit_ex(pxor_sigctx->mdctx, pxor_sigctx->md, NULL))
        goto error;

    return 1;

 error:
    EVP_MD_CTX_free(pxor_sigctx->mdctx);
    EVP_MD_free(pxor_sigctx->md);
    pxor_sigctx->mdctx = NULL;
    pxor_sigctx->md = NULL;
    return 0;
}

static int xor_sig_digest_sign_init(void *vpxor_sigctx, const char *mdname,
                                      void *vxorsig, const OSSL_PARAM params[])
{
    return xor_sig_digest_signverify_init(vpxor_sigctx, mdname, vxorsig,
                                          EVP_PKEY_OP_SIGN);
}

static int xor_sig_digest_verify_init(void *vpxor_sigctx, const char *mdname, void *vxorsig, const OSSL_PARAM params[])
{
    return xor_sig_digest_signverify_init(vpxor_sigctx, mdname,
                                          vxorsig, EVP_PKEY_OP_VERIFY);
}

int xor_sig_digest_signverify_update(void *vpxor_sigctx,
                                     const unsigned char *data,
                                     size_t datalen)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx == NULL || pxor_sigctx->mdctx == NULL)
        return 0;

    return EVP_DigestUpdate(pxor_sigctx->mdctx, data, datalen);
}

int xor_sig_digest_sign_final(void *vpxor_sigctx,
                              unsigned char *sig, size_t *siglen,
                              size_t sigsize)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (sig != NULL) {
        if (pxor_sigctx == NULL || pxor_sigctx->mdctx == NULL)
            return 0;

        if (!EVP_DigestFinal_ex(pxor_sigctx->mdctx, digest, &dlen))
            return 0;

        pxor_sigctx->flag_allow_md = 1;
    }

    return xor_sig_sign(vpxor_sigctx, sig, siglen, sigsize, digest, (size_t)dlen);

}

int xor_sig_digest_verify_final(void *vpxor_sigctx, const unsigned char *sig,
                            size_t siglen)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;

    if (pxor_sigctx == NULL || pxor_sigctx->mdctx == NULL)
        return 0;

    if (!EVP_DigestFinal_ex(pxor_sigctx->mdctx, digest, &dlen))
        return 0;

    pxor_sigctx->flag_allow_md = 1;

    return xor_sig_verify(vpxor_sigctx, sig, siglen, digest, (size_t)dlen);
}

static void xor_sig_freectx(void *vpxor_sigctx)
{
    PROV_XORSIG_CTX *ctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    OPENSSL_free(ctx->propq);
    EVP_MD_CTX_free(ctx->mdctx);
    EVP_MD_free(ctx->md);
    ctx->propq = NULL;
    ctx->mdctx = NULL;
    ctx->md = NULL;
    xor_freekey(ctx->sig);
    ctx->sig = NULL;
    OPENSSL_free(ctx->aid);
    OPENSSL_free(ctx);
}

static void *xor_sig_dupctx(void *vpxor_sigctx)
{
    PROV_XORSIG_CTX *srcctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    PROV_XORSIG_CTX *dstctx;

    dstctx = OPENSSL_zalloc(sizeof(*srcctx));
    if (dstctx == NULL)
        return NULL;

    *dstctx = *srcctx;
    dstctx->sig = NULL;
    dstctx->md = NULL;
    dstctx->mdctx = NULL;
    dstctx->aid = NULL;

    if ((srcctx->sig != NULL) && !xor_key_up_ref(srcctx->sig))
        goto err;
    dstctx->sig = srcctx->sig;

    if (srcctx->md != NULL && !EVP_MD_up_ref(srcctx->md))
        goto err;
    dstctx->md = srcctx->md;

    if (srcctx->mdctx != NULL) {
        dstctx->mdctx = EVP_MD_CTX_new();
        if (dstctx->mdctx == NULL
                || !EVP_MD_CTX_copy_ex(dstctx->mdctx, srcctx->mdctx))
            goto err;
    }

    return dstctx;
 err:
    xor_sig_freectx(dstctx);
    return NULL;
}

static int xor_sig_get_ctx_params(void *vpxor_sigctx, OSSL_PARAM *params)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    OSSL_PARAM *p;

    if (pxor_sigctx == NULL || params == NULL)
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_ALGORITHM_ID);

    if (pxor_sigctx->aid == NULL)
        pxor_sigctx->aid_len = xor_get_aid(&(pxor_sigctx->aid), pxor_sigctx->sig->tls_name);

    if (p != NULL
        && !OSSL_PARAM_set_octet_string(p, pxor_sigctx->aid, pxor_sigctx->aid_len))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_SIGNATURE_PARAM_DIGEST);
    if (p != NULL && !OSSL_PARAM_set_utf8_string(p, pxor_sigctx->mdname))
        return 0;

    return 1;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_octet_string(OSSL_SIGNATURE_PARAM_ALGORITHM_ID, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *xor_sig_gettable_ctx_params(ossl_unused void *vpxor_sigctx, ossl_unused void *vctx)
{
    return known_gettable_ctx_params;
}

static int xor_sig_set_ctx_params(void *vpxor_sigctx, const OSSL_PARAM params[])
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;
    const OSSL_PARAM *p;

    if (pxor_sigctx == NULL || params == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_SIGNATURE_PARAM_DIGEST);
    /* Not allowed during certain operations */
    if (p != NULL && !pxor_sigctx->flag_allow_md)
        return 0;
    if (p != NULL) {
        char mdname[OSSL_MAX_NAME_SIZE] = "", *pmdname = mdname;
        char mdprops[OSSL_MAX_PROPQUERY_SIZE] = "", *pmdprops = mdprops;
        const OSSL_PARAM *propsp =
            OSSL_PARAM_locate_const(params,
                                    OSSL_SIGNATURE_PARAM_PROPERTIES);

        if (!OSSL_PARAM_get_utf8_string(p, &pmdname, sizeof(mdname)))
            return 0;
        if (propsp != NULL
            && !OSSL_PARAM_get_utf8_string(propsp, &pmdprops, sizeof(mdprops)))
            return 0;
        if (!xor_sig_setup_md(pxor_sigctx, mdname, mdprops))
            return 0;
    }

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_DIGEST, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_SIGNATURE_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *xor_sig_settable_ctx_params(ossl_unused void *vpsm2ctx,
                                                     ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

static int xor_sig_get_ctx_md_params(void *vpxor_sigctx, OSSL_PARAM *params)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_get_params(pxor_sigctx->mdctx, params);
}

static const OSSL_PARAM *xor_sig_gettable_ctx_md_params(void *vpxor_sigctx)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx->md == NULL)
        return 0;

    return EVP_MD_gettable_ctx_params(pxor_sigctx->md);
}

static int xor_sig_set_ctx_md_params(void *vpxor_sigctx, const OSSL_PARAM params[])
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx->mdctx == NULL)
        return 0;

    return EVP_MD_CTX_set_params(pxor_sigctx->mdctx, params);
}

static const OSSL_PARAM *xor_sig_settable_ctx_md_params(void *vpxor_sigctx)
{
    PROV_XORSIG_CTX *pxor_sigctx = (PROV_XORSIG_CTX *)vpxor_sigctx;

    if (pxor_sigctx->md == NULL)
        return 0;

    return EVP_MD_settable_ctx_params(pxor_sigctx->md);
}

static const OSSL_DISPATCH xor_signature_functions[] = {
    { OSSL_FUNC_SIGNATURE_NEWCTX, (void (*)(void))xor_sig_newctx },
    { OSSL_FUNC_SIGNATURE_SIGN_INIT, (void (*)(void))xor_sig_sign_init },
    { OSSL_FUNC_SIGNATURE_SIGN, (void (*)(void))xor_sig_sign },
    { OSSL_FUNC_SIGNATURE_VERIFY_INIT, (void (*)(void))xor_sig_verify_init },
    { OSSL_FUNC_SIGNATURE_VERIFY, (void (*)(void))xor_sig_verify },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_INIT,
      (void (*)(void))xor_sig_digest_sign_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_UPDATE,
      (void (*)(void))xor_sig_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_SIGN_FINAL,
      (void (*)(void))xor_sig_digest_sign_final },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_INIT,
      (void (*)(void))xor_sig_digest_verify_init },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_UPDATE,
      (void (*)(void))xor_sig_digest_signverify_update },
    { OSSL_FUNC_SIGNATURE_DIGEST_VERIFY_FINAL,
      (void (*)(void))xor_sig_digest_verify_final },
    { OSSL_FUNC_SIGNATURE_FREECTX, (void (*)(void))xor_sig_freectx },
    { OSSL_FUNC_SIGNATURE_DUPCTX, (void (*)(void))xor_sig_dupctx },
    { OSSL_FUNC_SIGNATURE_GET_CTX_PARAMS, (void (*)(void))xor_sig_get_ctx_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_PARAMS,
      (void (*)(void))xor_sig_gettable_ctx_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_PARAMS, (void (*)(void))xor_sig_set_ctx_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_PARAMS,
      (void (*)(void))xor_sig_settable_ctx_params },
    { OSSL_FUNC_SIGNATURE_GET_CTX_MD_PARAMS,
      (void (*)(void))xor_sig_get_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_GETTABLE_CTX_MD_PARAMS,
      (void (*)(void))xor_sig_gettable_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SET_CTX_MD_PARAMS,
      (void (*)(void))xor_sig_set_ctx_md_params },
    { OSSL_FUNC_SIGNATURE_SETTABLE_CTX_MD_PARAMS,
      (void (*)(void))xor_sig_settable_ctx_md_params },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM tls_prov_signature[] = {
    /*
     * Obviously this is not FIPS approved, but in order to test in conjunction
     * with the FIPS provider we pretend that it is.
     */
    { XORSIGALG_NAME, "provider=tls-provider,fips=yes",
                           xor_signature_functions },
    { XORSIGALG_HASH_NAME, "provider=tls-provider,fips=yes",
                           xor_signature_functions },
    { XORSIGALG12_NAME, "provider=tls-provider,fips=yes",
                           xor_signature_functions },
    { NULL, NULL, NULL }
};


static const OSSL_ALGORITHM *tls_prov_query(void *provctx, int operation_id,
                                            int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_KEYMGMT:
        return tls_prov_keymgmt;
    case OSSL_OP_KEYEXCH:
        return tls_prov_keyexch;
    case OSSL_OP_KEM:
        return tls_prov_kem;
    case OSSL_OP_ENCODER:
        return tls_prov_encoder;
    case OSSL_OP_DECODER:
        return tls_prov_decoder;
    case OSSL_OP_SIGNATURE:
        return tls_prov_signature;
    }
    return NULL;
}

static void tls_prov_teardown(void *provctx)
{
    int i;
    PROV_XOR_CTX *pctx = (PROV_XOR_CTX*)provctx;

    OSSL_LIB_CTX_free(pctx->libctx);

    for (i = 0; i < NUM_DUMMY_GROUPS; i++) {
        OPENSSL_free(dummy_group_names[i]);
        dummy_group_names[i] = NULL;
    }
    OPENSSL_free(pctx);
}

/* Functions we provide to the core */
static const OSSL_DISPATCH tls_prov_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))tls_prov_teardown },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))tls_prov_query },
    { OSSL_FUNC_PROVIDER_GET_CAPABILITIES, (void (*)(void))tls_prov_get_capabilities },
    OSSL_DISPATCH_END
};

static
unsigned int randomize_tls_alg_id(OSSL_LIB_CTX *libctx)
{
    /*
     * Randomise the id we're going to use to ensure we don't interoperate
     * with anything but ourselves.
     */
    unsigned int id;
    static unsigned int mem[10] = { 0 };
    static int in_mem = 0;
    int i;

 retry:
    if (RAND_bytes_ex(libctx, (unsigned char *)&id, sizeof(id), 0) <= 0)
        return 0;
    /*
     * Ensure id is within the IANA Reserved for private use range
     * (65024-65279).
     * Carve out NUM_DUMMY_GROUPS ids for properly registering those.
     */
    id %= 65279 - NUM_DUMMY_GROUPS - 65024;
    id += 65024;

    /* Ensure we did not already issue this id */
    for (i = 0; i < in_mem; i++)
        if (mem[i] == id)
            goto retry;

    /* Add this id to the list of ids issued by this function */
    mem[in_mem++] = id;

    return id;
}

int tls_provider_init(const OSSL_CORE_HANDLE *handle,
                      const OSSL_DISPATCH *in,
                      const OSSL_DISPATCH **out,
                      void **provctx)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new_from_dispatch(handle, in);
    OSSL_FUNC_core_obj_create_fn *c_obj_create= NULL;
    OSSL_FUNC_core_obj_add_sigid_fn *c_obj_add_sigid= NULL;
    PROV_XOR_CTX *xor_prov_ctx = xor_newprovctx(libctx);

    if (libctx == NULL || xor_prov_ctx == NULL)
        goto err;

    *provctx = xor_prov_ctx;

    /*
     * Randomise the group_id and code_points we're going to use to ensure we
     * don't interoperate with anything but ourselves.
     */
    xor_group.group_id = randomize_tls_alg_id(libctx);
    xor_kemgroup.group_id = randomize_tls_alg_id(libctx);
    xor_sigalg.code_point = randomize_tls_alg_id(libctx);
    xor_sigalg_hash.code_point = randomize_tls_alg_id(libctx);

    /* Retrieve registration functions */
    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_OBJ_CREATE:
            c_obj_create = OSSL_FUNC_core_obj_create(in);
            break;
        case OSSL_FUNC_CORE_OBJ_ADD_SIGID:
            c_obj_add_sigid = OSSL_FUNC_core_obj_add_sigid(in);
            break;
        /* Just ignore anything we don't understand */
        default:
            break;
        }
    }

    /*
     * Register algorithms manually as add_provider_sigalgs is
     * only called during session establishment -- too late for
     * key & cert generation...
     */
    if (!c_obj_create(handle, XORSIGALG_OID, XORSIGALG_NAME, XORSIGALG_NAME)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_OBJ_CREATE_ERR);
        goto err;
    }

    if (!c_obj_add_sigid(handle, XORSIGALG_OID, "", XORSIGALG_OID)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_OBJ_CREATE_ERR);
        goto err;
    }
    if (!c_obj_create(handle, XORSIGALG_HASH_OID, XORSIGALG_HASH_NAME, NULL)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_OBJ_CREATE_ERR);
        goto err;
    }

    if (!c_obj_add_sigid(handle, XORSIGALG_HASH_OID, XORSIGALG_HASH, XORSIGALG_HASH_OID)) {
        ERR_raise(ERR_LIB_USER, XORPROV_R_OBJ_CREATE_ERR);
        goto err;
    }

    *out = tls_prov_dispatch_table;
    return 1;

err:
    OPENSSL_free(xor_prov_ctx);
    *provctx = NULL;
    OSSL_LIB_CTX_free(libctx);
    return 0;
}
