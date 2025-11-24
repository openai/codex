/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/self_test.h>
#include "internal/param_build_set.h"
#include <openssl/param_build.h>
#include "crypto/ecx.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/ecx.h"
#include "prov/securitycheck.h"
#ifdef S390X_EC_ASM
# include "s390x_arch.h"
# include <openssl/sha.h>   /* For SHA512_DIGEST_LENGTH */
#endif

static OSSL_FUNC_keymgmt_new_fn x25519_new_key;
static OSSL_FUNC_keymgmt_new_fn x448_new_key;
static OSSL_FUNC_keymgmt_new_fn ed25519_new_key;
static OSSL_FUNC_keymgmt_new_fn ed448_new_key;
static OSSL_FUNC_keymgmt_gen_init_fn x25519_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn x448_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn ed25519_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn ed448_gen_init;
static OSSL_FUNC_keymgmt_gen_fn x25519_gen;
static OSSL_FUNC_keymgmt_gen_fn x448_gen;
static OSSL_FUNC_keymgmt_gen_fn ed25519_gen;
static OSSL_FUNC_keymgmt_gen_fn ed448_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn ecx_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn ecx_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn ecx_gen_settable_params;
static OSSL_FUNC_keymgmt_load_fn ecx_load;
static OSSL_FUNC_keymgmt_get_params_fn x25519_get_params;
static OSSL_FUNC_keymgmt_get_params_fn x448_get_params;
static OSSL_FUNC_keymgmt_get_params_fn ed25519_get_params;
static OSSL_FUNC_keymgmt_get_params_fn ed448_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn x25519_gettable_params;
static OSSL_FUNC_keymgmt_gettable_params_fn x448_gettable_params;
static OSSL_FUNC_keymgmt_gettable_params_fn ed25519_gettable_params;
static OSSL_FUNC_keymgmt_gettable_params_fn ed448_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn x25519_set_params;
static OSSL_FUNC_keymgmt_set_params_fn x448_set_params;
static OSSL_FUNC_keymgmt_set_params_fn ed25519_set_params;
static OSSL_FUNC_keymgmt_set_params_fn ed448_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn x25519_settable_params;
static OSSL_FUNC_keymgmt_settable_params_fn x448_settable_params;
static OSSL_FUNC_keymgmt_settable_params_fn ed25519_settable_params;
static OSSL_FUNC_keymgmt_settable_params_fn ed448_settable_params;
static OSSL_FUNC_keymgmt_has_fn ecx_has;
static OSSL_FUNC_keymgmt_match_fn ecx_match;
static OSSL_FUNC_keymgmt_validate_fn x25519_validate;
static OSSL_FUNC_keymgmt_validate_fn x448_validate;
static OSSL_FUNC_keymgmt_validate_fn ed25519_validate;
static OSSL_FUNC_keymgmt_validate_fn ed448_validate;
static OSSL_FUNC_keymgmt_import_fn ecx_import;
static OSSL_FUNC_keymgmt_import_types_fn ecx_imexport_types;
static OSSL_FUNC_keymgmt_export_fn ecx_export;
static OSSL_FUNC_keymgmt_export_types_fn ecx_imexport_types;
static OSSL_FUNC_keymgmt_dup_fn ecx_dup;

#define ECX_POSSIBLE_SELECTIONS (OSSL_KEYMGMT_SELECT_KEYPAIR)

struct ecx_gen_ctx {
    OSSL_LIB_CTX *libctx;
    char *propq;
    ECX_KEY_TYPE type;
    int selection;
    unsigned char *dhkem_ikm;
    size_t dhkem_ikmlen;
};

#ifdef S390X_EC_ASM
static void *s390x_ecx_keygen25519(struct ecx_gen_ctx *gctx);
static void *s390x_ecx_keygen448(struct ecx_gen_ctx *gctx);
static void *s390x_ecd_keygen25519(struct ecx_gen_ctx *gctx);
static void *s390x_ecd_keygen448(struct ecx_gen_ctx *gctx);
#endif

static void *x25519_new_key(void *provctx)
{
    if (!ossl_prov_is_running())
        return 0;
    return ossl_ecx_key_new(PROV_LIBCTX_OF(provctx), ECX_KEY_TYPE_X25519, 0,
                            NULL);
}

static void *x448_new_key(void *provctx)
{
    if (!ossl_prov_is_running())
        return 0;
    return ossl_ecx_key_new(PROV_LIBCTX_OF(provctx), ECX_KEY_TYPE_X448, 0,
                            NULL);
}

static void *ed25519_new_key(void *provctx)
{
    if (!ossl_prov_is_running())
        return 0;
    return ossl_ecx_key_new(PROV_LIBCTX_OF(provctx), ECX_KEY_TYPE_ED25519, 0,
                            NULL);
}

static void *ed448_new_key(void *provctx)
{
    if (!ossl_prov_is_running())
        return 0;
    return ossl_ecx_key_new(PROV_LIBCTX_OF(provctx), ECX_KEY_TYPE_ED448, 0,
                            NULL);
}

static int ecx_has(const void *keydata, int selection)
{
    const ECX_KEY *key = keydata;
    int ok = 0;

    if (ossl_prov_is_running() && key != NULL) {
        /*
         * ECX keys always have all the parameters they need (i.e. none).
         * Therefore we always return with 1, if asked about parameters.
         */
        ok = 1;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
            ok = ok && key->haspubkey;

        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
            ok = ok && key->privkey != NULL;
    }
    return ok;
}

static int ecx_match(const void *keydata1, const void *keydata2, int selection)
{
    const ECX_KEY *key1 = keydata1;
    const ECX_KEY *key2 = keydata2;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && key1->type == key2->type;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            const unsigned char *pa = key1->haspubkey ? key1->pubkey : NULL;
            const unsigned char *pb = key2->haspubkey ? key2->pubkey : NULL;
            size_t pal = key1->keylen;
            size_t pbl = key2->keylen;

            if (pa != NULL && pb != NULL) {
                ok = ok
                    && key1->type == key2->type
                    && pal == pbl
                    && CRYPTO_memcmp(pa, pb, pal) == 0;
                key_checked = 1;
            }
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            const unsigned char *pa = key1->privkey;
            const unsigned char *pb = key2->privkey;
            size_t pal = key1->keylen;
            size_t pbl = key2->keylen;

            if (pa != NULL && pb != NULL) {
                ok = ok
                    && key1->type == key2->type
                    && pal == pbl
                    && CRYPTO_memcmp(pa, pb, pal) == 0;
                key_checked = 1;
            }
        }
        ok = ok && key_checked;
    }
    return ok;
}

static int ecx_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    ECX_KEY *key = keydata;
    int ok = 1;
    int include_private;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_private = selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;
    ok = ok && ossl_ecx_key_fromdata(key, params, include_private);

    return ok;
}

static int key_to_params(ECX_KEY *key, OSSL_PARAM_BLD *tmpl,
                         OSSL_PARAM params[], int include_private)
{
    if (key == NULL)
        return 0;

    if (!ossl_param_build_set_octet_string(tmpl, params,
                                           OSSL_PKEY_PARAM_PUB_KEY,
                                           key->pubkey, key->keylen))
        return 0;

    if (include_private
        && key->privkey != NULL
        && !ossl_param_build_set_octet_string(tmpl, params,
                                              OSSL_PKEY_PARAM_PRIV_KEY,
                                              key->privkey, key->keylen))
        return 0;

    return 1;
}

static int ecx_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
                      void *cbarg)
{
    ECX_KEY *key = keydata;
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ret = 0;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private = ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0);

        if (!key_to_params(key, tmpl, NULL, include_private))
            goto err;
    }

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return ret;
}

#define ECX_KEY_TYPES()                                                        \
OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),                     \
OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)

static const OSSL_PARAM ecx_key_types[] = {
    ECX_KEY_TYPES(),
    OSSL_PARAM_END
};
static const OSSL_PARAM *ecx_imexport_types(int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return ecx_key_types;
    return NULL;
}

static int ecx_get_params(void *key, OSSL_PARAM params[], int bits, int secbits,
                          int size)
{
    ECX_KEY *ecx = key;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, bits))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, secbits))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && !OSSL_PARAM_set_int(p, size))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY)) != NULL
            && (ecx->type == ECX_KEY_TYPE_X25519
                || ecx->type == ECX_KEY_TYPE_X448)) {
        if (!OSSL_PARAM_set_octet_string(p, ecx->pubkey, ecx->keylen))
            return 0;
    }
#ifdef FIPS_MODULE
    {
        /* X25519 and X448 are not approved */
        int approved = 0;

        p = OSSL_PARAM_locate(params, OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR);
        if (p != NULL && !OSSL_PARAM_set_int(p, approved))
            return 0;
    }
#endif

    return key_to_params(ecx, NULL, params, 1);
}

static int ed_get_params(void *key, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params,
                               OSSL_PKEY_PARAM_MANDATORY_DIGEST)) != NULL
        && !OSSL_PARAM_set_utf8_string(p, ""))
        return 0;
    return 1;
}

static int x25519_get_params(void *key, OSSL_PARAM params[])
{
    return ecx_get_params(key, params, X25519_BITS, X25519_SECURITY_BITS,
                          X25519_KEYLEN);
}

static int x448_get_params(void *key, OSSL_PARAM params[])
{
    return ecx_get_params(key, params, X448_BITS, X448_SECURITY_BITS,
                          X448_KEYLEN);
}

static int ed25519_get_params(void *key, OSSL_PARAM params[])
{
    return ecx_get_params(key, params, ED25519_BITS, ED25519_SECURITY_BITS,
                          ED25519_SIGSIZE)
        && ed_get_params(key, params);
}

static int ed448_get_params(void *key, OSSL_PARAM params[])
{
    return ecx_get_params(key, params, ED448_BITS, ED448_SECURITY_BITS,
                          ED448_SIGSIZE)
        && ed_get_params(key, params);
}

static const OSSL_PARAM ecx_gettable_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    ECX_KEY_TYPES(),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};

static const OSSL_PARAM ed_gettable_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_MANDATORY_DIGEST, NULL, 0),
    ECX_KEY_TYPES(),
    OSSL_PARAM_END
};

static const OSSL_PARAM *x25519_gettable_params(void *provctx)
{
    return ecx_gettable_params;
}

static const OSSL_PARAM *x448_gettable_params(void *provctx)
{
    return ecx_gettable_params;
}

static const OSSL_PARAM *ed25519_gettable_params(void *provctx)
{
    return ed_gettable_params;
}

static const OSSL_PARAM *ed448_gettable_params(void *provctx)
{
    return ed_gettable_params;
}

static int set_property_query(ECX_KEY *ecxkey, const char *propq)
{
    OPENSSL_free(ecxkey->propq);
    ecxkey->propq = NULL;
    if (propq != NULL) {
        ecxkey->propq = OPENSSL_strdup(propq);
        if (ecxkey->propq == NULL)
            return 0;
    }
    return 1;
}

static int ecx_set_params(void *key, const OSSL_PARAM params[])
{
    ECX_KEY *ecxkey = key;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL) {
        void *buf = ecxkey->pubkey;

        if (p->data_size != ecxkey->keylen
                || !OSSL_PARAM_get_octet_string(p, &buf, sizeof(ecxkey->pubkey),
                                                NULL))
            return 0;
        OPENSSL_clear_free(ecxkey->privkey, ecxkey->keylen);
        ecxkey->privkey = NULL;
        ecxkey->haspubkey = 1;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || !set_property_query(ecxkey, p->data))
            return 0;
    }

    return 1;
}

static int x25519_set_params(void *key, const OSSL_PARAM params[])
{
    return ecx_set_params(key, params);
}

static int x448_set_params(void *key, const OSSL_PARAM params[])
{
    return ecx_set_params(key, params);
}

static int ed25519_set_params(void *key, const OSSL_PARAM params[])
{
    return 1;
}

static int ed448_set_params(void *key, const OSSL_PARAM params[])
{
    return 1;
}

static const OSSL_PARAM ecx_settable_params[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM ed_settable_params[] = {
    OSSL_PARAM_END
};

static const OSSL_PARAM *x25519_settable_params(void *provctx)
{
    return ecx_settable_params;
}

static const OSSL_PARAM *x448_settable_params(void *provctx)
{
    return ecx_settable_params;
}

static const OSSL_PARAM *ed25519_settable_params(void *provctx)
{
    return ed_settable_params;
}

static const OSSL_PARAM *ed448_settable_params(void *provctx)
{
    return ed_settable_params;
}

static void *ecx_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[], ECX_KEY_TYPE type,
                          const char *algdesc)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct ecx_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->libctx = libctx;
        gctx->type = type;
        gctx->selection = selection;
#ifdef FIPS_MODULE
        /* X25519/X448 are not FIPS approved, (ED25519/ED448 are approved) */
        if (algdesc != NULL
                && !ossl_FIPS_IND_callback(libctx, algdesc, "KeyGen Init")) {
            OPENSSL_free(gctx);
            return NULL;
        }
#endif
    } else {
        return NULL;
    }
    if (!ecx_gen_set_params(gctx, params)) {
        ecx_gen_cleanup(gctx);
        gctx = NULL;
    }
    return gctx;
}

static void *x25519_gen_init(void *provctx, int selection,
                             const OSSL_PARAM params[])
{
    return ecx_gen_init(provctx, selection, params, ECX_KEY_TYPE_X25519, "X25519");
}

static void *x448_gen_init(void *provctx, int selection,
                           const OSSL_PARAM params[])
{
    return ecx_gen_init(provctx, selection, params, ECX_KEY_TYPE_X448, "X448");
}

static void *ed25519_gen_init(void *provctx, int selection,
                              const OSSL_PARAM params[])
{
    return ecx_gen_init(provctx, selection, params, ECX_KEY_TYPE_ED25519, NULL);
}

static void *ed448_gen_init(void *provctx, int selection,
                            const OSSL_PARAM params[])
{
    return ecx_gen_init(provctx, selection, params, ECX_KEY_TYPE_ED448, NULL);
}

static int ecx_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct ecx_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_GROUP_NAME);
    if (p != NULL) {
        const char *groupname = NULL;

        /*
         * We optionally allow setting a group name - but each algorithm only
         * support one such name, so all we do is verify that it is the one we
         * expected.
         */
        switch (gctx->type) {
            case ECX_KEY_TYPE_X25519:
                groupname = "x25519";
                break;
            case ECX_KEY_TYPE_X448:
                groupname = "x448";
                break;
            default:
                /* We only support this for key exchange at the moment */
                break;
        }
        if (p->data_type != OSSL_PARAM_UTF8_STRING
                || groupname == NULL
                || OPENSSL_strcasecmp(p->data, groupname) != 0) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->propq);
        gctx->propq = OPENSSL_strdup(p->data);
        if (gctx->propq == NULL)
            return 0;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_DHKEM_IKM);
    if (p != NULL) {
        if (p->data_size != 0 && p->data != NULL) {
            OPENSSL_free(gctx->dhkem_ikm);
            gctx->dhkem_ikm = NULL;
            if (!OSSL_PARAM_get_octet_string(p, (void **)&gctx->dhkem_ikm, 0,
                                             &gctx->dhkem_ikmlen))
                return 0;
        }
    }

    return 1;
}

static const OSSL_PARAM *ecx_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

#ifdef FIPS_MODULE
/*
 * Refer: FIPS 140-3 IG 10.3.A Additional Comment 1
 * Perform a pairwise test for EDDSA by signing and verifying signature.
 *
 * The parameter `self_test` is used to indicate whether to create OSSL_SELF_TEST
 * instance.
 */
static int ecd_fips140_pairwise_test(const ECX_KEY *ecx, int type, int self_test)
{
    int ret = 0;
    OSSL_SELF_TEST *st = NULL;
    OSSL_CALLBACK *cb = NULL;
    void *cbarg = NULL;

    unsigned char msg[16] = {0};
    size_t msg_len = sizeof(msg);
    unsigned char sig[ED448_SIGSIZE] = {0};

    int is_ed25519 = (type == ECX_KEY_TYPE_ED25519) ? 1 : 0;
    int operation_result = 0;

    /*
     * The functions `OSSL_SELF_TEST_*` will return directly if parameter `st`
     * is NULL.
     */
    if (self_test)  {
        OSSL_SELF_TEST_get_callback(ecx->libctx, &cb, &cbarg);

        st = OSSL_SELF_TEST_new(cb, cbarg);
        if (st == NULL)
            return 0;
    }

    OSSL_SELF_TEST_onbegin(st, OSSL_SELF_TEST_TYPE_PCT,
                           OSSL_SELF_TEST_DESC_PCT_EDDSA);

    if (is_ed25519)
        operation_result = ossl_ed25519_sign(sig, msg, msg_len, ecx->pubkey,
                                             ecx->privkey, 0, 0, 0, NULL, 0,
                                             ecx->libctx, ecx->propq);
    else
        operation_result = ossl_ed448_sign(ecx->libctx, sig, msg, msg_len,
                                           ecx->pubkey, ecx->privkey, NULL, 0,
                                           0, ecx->propq);
    if (operation_result != 1)
        goto err;

    OSSL_SELF_TEST_oncorrupt_byte(st, sig);

    if (is_ed25519)
        operation_result = ossl_ed25519_verify(msg, msg_len, sig, ecx->pubkey,
                                               0, 0, 0, NULL, 0, ecx->libctx,
                                               ecx->propq);
    else
        operation_result = ossl_ed448_verify(ecx->libctx, msg, msg_len, sig,
                                             ecx->pubkey, NULL, 0, 0, ecx->propq);
    if (operation_result != 1)
        goto err;

    ret = 1;
err:
    OSSL_SELF_TEST_onend(st, ret);
    OSSL_SELF_TEST_free(st);
    return ret;
}
#endif

static void *ecx_gen(struct ecx_gen_ctx *gctx)
{
    ECX_KEY *key;
    unsigned char *privkey;

    if (gctx == NULL)
        return NULL;
    if ((key = ossl_ecx_key_new(gctx->libctx, gctx->type, 0,
                                gctx->propq)) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        return NULL;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    if ((privkey = ossl_ecx_key_allocate_privkey(key)) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }
#ifndef FIPS_MODULE
    if (gctx->dhkem_ikm != NULL && gctx->dhkem_ikmlen != 0) {
        if (gctx->type == ECX_KEY_TYPE_ED25519
                || gctx->type == ECX_KEY_TYPE_ED448)
            goto err;
        if (!ossl_ecx_dhkem_derive_private(key, privkey,
                                           gctx->dhkem_ikm, gctx->dhkem_ikmlen))
            goto err;
    } else
#endif
    {
        if (RAND_priv_bytes_ex(gctx->libctx, privkey, key->keylen, 0) <= 0)
            goto err;
    }

    switch (gctx->type) {
    case ECX_KEY_TYPE_X25519:
        privkey[0] &= 248;
        privkey[X25519_KEYLEN - 1] &= 127;
        privkey[X25519_KEYLEN - 1] |= 64;
        ossl_x25519_public_from_private(key->pubkey, privkey);
        break;
    case ECX_KEY_TYPE_X448:
        privkey[0] &= 252;
        privkey[X448_KEYLEN - 1] |= 128;
        ossl_x448_public_from_private(key->pubkey, privkey);
        break;
    case ECX_KEY_TYPE_ED25519:
        if (!ossl_ed25519_public_from_private(gctx->libctx, key->pubkey, privkey,
                                              gctx->propq))
            goto err;
        break;
    case ECX_KEY_TYPE_ED448:
        if (!ossl_ed448_public_from_private(gctx->libctx, key->pubkey, privkey,
                                            gctx->propq))
            goto err;
        break;
    }
    key->haspubkey = 1;
    return key;
err:
    ossl_ecx_key_free(key);
    return NULL;
}

static void *x25519_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct ecx_gen_ctx *gctx = genctx;

    if (!ossl_prov_is_running())
        return 0;

#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X25519))
        return s390x_ecx_keygen25519(gctx);
#endif
    return ecx_gen(gctx);
}

static void *x448_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct ecx_gen_ctx *gctx = genctx;

    if (!ossl_prov_is_running())
        return 0;

#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X448))
        return s390x_ecx_keygen448(gctx);
#endif
    return ecx_gen(gctx);
}

static void *ed25519_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    ECX_KEY *key = NULL;
    struct ecx_gen_ctx *gctx = genctx;

    if (!ossl_prov_is_running())
        return 0;

#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED25519)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_SIGN_ED25519)
        && OPENSSL_s390xcap_P.kdsa[0]
            & S390X_CAPBIT(S390X_EDDSA_VERIFY_ED25519)) {
        key = s390x_ecd_keygen25519(gctx);
    } else
#endif
    {
        key = ecx_gen(gctx);
    }

#ifdef FIPS_MODULE
    /* Exit if keygen failed OR we are doing parameter generation (blank key) */
    if (!key || ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0))
        return key;
    if (ecd_fips140_pairwise_test(key, ECX_KEY_TYPE_ED25519, 1) != 1) {
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
        ossl_ecx_key_free(key);
        return NULL;
    }
#endif

    return key;
}

static void *ed448_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    ECX_KEY *key = NULL;
    struct ecx_gen_ctx *gctx = genctx;

    if (!ossl_prov_is_running())
        return 0;

#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED448)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_SIGN_ED448)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_VERIFY_ED448)) {
        key = s390x_ecd_keygen448(gctx);
    } else
#endif
    {
        key = ecx_gen(gctx);
    }

#ifdef FIPS_MODULE
    /* Exit if keygen failed OR we are doing parameter generation (blank key) */
    if (!key || ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0))
        return key;
    if (ecd_fips140_pairwise_test(key, ECX_KEY_TYPE_ED448, 1) != 1) {
        ossl_set_error_state(OSSL_SELF_TEST_TYPE_PCT);
        ossl_ecx_key_free(key);
        return NULL;
    }
#endif

    return key;
}

static void ecx_gen_cleanup(void *genctx)
{
    struct ecx_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_clear_free(gctx->dhkem_ikm, gctx->dhkem_ikmlen);
    OPENSSL_free(gctx->propq);
    OPENSSL_free(gctx);
}

void *ecx_load(const void *reference, size_t reference_sz)
{
    ECX_KEY *key = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(key)) {
        /* The contents of the reference is the address to our object */
        key = *(ECX_KEY **)reference;
        /* We grabbed, so we detach it */
        *(ECX_KEY **)reference = NULL;
        return key;
    }
    return NULL;
}

static void *ecx_dup(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_ecx_key_dup(keydata_from, selection);
    return NULL;
}

static int ecx_key_pairwise_check(const ECX_KEY *ecx, int type)
{
    uint8_t pub[64];

    switch (type) {
    case ECX_KEY_TYPE_X25519:
        ossl_x25519_public_from_private(pub, ecx->privkey);
        break;
    case ECX_KEY_TYPE_X448:
        ossl_x448_public_from_private(pub, ecx->privkey);
        break;
    default:
        return 0;
    }
    return CRYPTO_memcmp(ecx->pubkey, pub, ecx->keylen) == 0;
}

#ifdef FIPS_MODULE
/*
 * FIPS ACVP testing requires the ability to check if the public key is valid
 * This is not required normally since the ED signature verify does the test
 * internally.
 */
static int ecd_key_pub_check(const ECX_KEY *ecx, int type)
{
    switch (type) {
    case ECX_KEY_TYPE_ED25519:
        return ossl_ed25519_pubkey_verify(ecx->pubkey, ecx->keylen);
    case ECX_KEY_TYPE_ED448:
        return ossl_ed448_pubkey_verify(ecx->pubkey, ecx->keylen);
    default:
        return 1;
    }
}
#endif

#ifdef FIPS_MODULE
static int ecd_key_pairwise_check(const ECX_KEY *ecx, int type)
{
    return ecd_fips140_pairwise_test(ecx, type, 0);
}
#else
static int ecd_key_pairwise_check(const ECX_KEY *ecx, int type)
{
    uint8_t pub[64];

    switch (type) {
    case ECX_KEY_TYPE_ED25519:
        if (!ossl_ed25519_public_from_private(ecx->libctx, pub, ecx->privkey,
                                              ecx->propq))
            return 0;
        break;
    case ECX_KEY_TYPE_ED448:
        if (!ossl_ed448_public_from_private(ecx->libctx, pub, ecx->privkey,
                                            ecx->propq))
            return 0;
        break;
    default:
        return 0;
    }
    return CRYPTO_memcmp(ecx->pubkey, pub, ecx->keylen) == 0;
}
#endif

static int ecx_validate(const void *keydata, int selection, int type,
                        size_t keylen)
{
    const ECX_KEY *ecx = keydata;
    int ok = keylen == ecx->keylen;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & ECX_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    if (!ok) {
        ERR_raise(ERR_LIB_PROV, PROV_R_ALGORITHM_MISMATCH);
        return 0;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        ok = ok && ecx->haspubkey;
#ifdef FIPS_MODULE
        ok = ok && ecd_key_pub_check(ecx, type);
#endif
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && ecx->privkey != NULL;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != OSSL_KEYMGMT_SELECT_KEYPAIR)
        return ok;

    if (type == ECX_KEY_TYPE_ED25519 || type == ECX_KEY_TYPE_ED448)
        ok = ok && ecd_key_pairwise_check(ecx, type);
    else
        ok = ok && ecx_key_pairwise_check(ecx, type);

    return ok;
}

static int x25519_validate(const void *keydata, int selection, int checktype)
{
    return ecx_validate(keydata, selection, ECX_KEY_TYPE_X25519, X25519_KEYLEN);
}

static int x448_validate(const void *keydata, int selection, int checktype)
{
    return ecx_validate(keydata, selection, ECX_KEY_TYPE_X448, X448_KEYLEN);
}

static int ed25519_validate(const void *keydata, int selection, int checktype)
{
    return ecx_validate(keydata, selection, ECX_KEY_TYPE_ED25519, ED25519_KEYLEN);
}

static int ed448_validate(const void *keydata, int selection, int checktype)
{
    return ecx_validate(keydata, selection, ECX_KEY_TYPE_ED448, ED448_KEYLEN);
}

#define MAKE_KEYMGMT_FUNCTIONS(alg) \
    const OSSL_DISPATCH ossl_##alg##_keymgmt_functions[] = { \
        { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))alg##_new_key }, \
        { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))ossl_ecx_key_free }, \
        { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))alg##_get_params }, \
        { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))alg##_gettable_params }, \
        { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))alg##_set_params }, \
        { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))alg##_settable_params }, \
        { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))ecx_has }, \
        { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))ecx_match }, \
        { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))alg##_validate }, \
        { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))ecx_import }, \
        { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))ecx_imexport_types }, \
        { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))ecx_export }, \
        { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))ecx_imexport_types }, \
        { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))alg##_gen_init }, \
        { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))ecx_gen_set_params }, \
        { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS, \
          (void (*)(void))ecx_gen_settable_params }, \
        { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))alg##_gen }, \
        { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))ecx_gen_cleanup }, \
        { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))ecx_load }, \
        { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))ecx_dup }, \
        OSSL_DISPATCH_END \
    };

MAKE_KEYMGMT_FUNCTIONS(x25519)
MAKE_KEYMGMT_FUNCTIONS(x448)
MAKE_KEYMGMT_FUNCTIONS(ed25519)
MAKE_KEYMGMT_FUNCTIONS(ed448)

#ifdef S390X_EC_ASM
# include "s390x_arch.h"

static void *s390x_ecx_keygen25519(struct ecx_gen_ctx *gctx)
{
    static const unsigned char generator[] = {
        0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    ECX_KEY *key = ossl_ecx_key_new(gctx->libctx, ECX_KEY_TYPE_X25519, 1,
                                    gctx->propq);
    unsigned char *privkey = NULL, *pubkey;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

#ifndef FIPS_MODULE
    if (gctx->dhkem_ikm != NULL && gctx->dhkem_ikmlen != 0) {
        if (gctx->type != ECX_KEY_TYPE_X25519)
            goto err;
        if (!ossl_ecx_dhkem_derive_private(key, privkey,
                                           gctx->dhkem_ikm, gctx->dhkem_ikmlen))
            goto err;
    } else
#endif
    {
        if (RAND_priv_bytes_ex(gctx->libctx, privkey, X25519_KEYLEN, 0) <= 0)
            goto err;
    }

    privkey[0] &= 248;
    privkey[31] &= 127;
    privkey[31] |= 64;

    if (s390x_x25519_mul(pubkey, generator, privkey) != 1)
        goto err;
    key->haspubkey = 1;
    return key;
 err:
    ossl_ecx_key_free(key);
    return NULL;
}

static void *s390x_ecx_keygen448(struct ecx_gen_ctx *gctx)
{
    static const unsigned char generator[] = {
        0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    ECX_KEY *key = ossl_ecx_key_new(gctx->libctx, ECX_KEY_TYPE_X448, 1,
                                    gctx->propq);
    unsigned char *privkey = NULL, *pubkey;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

#ifndef FIPS_MODULE
    if (gctx->dhkem_ikm != NULL && gctx->dhkem_ikmlen != 0) {
        if (gctx->type != ECX_KEY_TYPE_X448)
            goto err;
        if (!ossl_ecx_dhkem_derive_private(key, privkey,
                                           gctx->dhkem_ikm, gctx->dhkem_ikmlen))
            goto err;
    } else
#endif
    {
        if (RAND_priv_bytes_ex(gctx->libctx, privkey, X448_KEYLEN, 0) <= 0)
            goto err;
    }

    privkey[0] &= 252;
    privkey[55] |= 128;

    if (s390x_x448_mul(pubkey, generator, privkey) != 1)
        goto err;
    key->haspubkey = 1;
    return key;
 err:
    ossl_ecx_key_free(key);
    return NULL;
}

static void *s390x_ecd_keygen25519(struct ecx_gen_ctx *gctx)
{
    static const unsigned char generator_x[] = {
        0x1a, 0xd5, 0x25, 0x8f, 0x60, 0x2d, 0x56, 0xc9, 0xb2, 0xa7, 0x25, 0x95,
        0x60, 0xc7, 0x2c, 0x69, 0x5c, 0xdc, 0xd6, 0xfd, 0x31, 0xe2, 0xa4, 0xc0,
        0xfe, 0x53, 0x6e, 0xcd, 0xd3, 0x36, 0x69, 0x21
    };
    static const unsigned char generator_y[] = {
        0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    };
    unsigned char x_dst[32], buff[SHA512_DIGEST_LENGTH];
    ECX_KEY *key = ossl_ecx_key_new(gctx->libctx, ECX_KEY_TYPE_ED25519, 1,
                                    gctx->propq);
    unsigned char *privkey = NULL, *pubkey;
    unsigned int sz;
    EVP_MD *sha = NULL;
    int j;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    if (RAND_priv_bytes_ex(gctx->libctx, privkey, ED25519_KEYLEN, 0) <= 0)
        goto err;

    sha = EVP_MD_fetch(gctx->libctx, "SHA512", gctx->propq);
    if (sha == NULL)
        goto err;
    j = EVP_Digest(privkey, 32, buff, &sz, sha, NULL);
    EVP_MD_free(sha);
    if (!j)
        goto err;

    buff[0] &= 248;
    buff[31] &= 63;
    buff[31] |= 64;

    if (s390x_ed25519_mul(x_dst, pubkey,
                          generator_x, generator_y, buff) != 1)
        goto err;

    pubkey[31] |= ((x_dst[0] & 0x01) << 7);
    key->haspubkey = 1;
    return key;
 err:
    ossl_ecx_key_free(key);
    return NULL;
}

static void *s390x_ecd_keygen448(struct ecx_gen_ctx *gctx)
{
    static const unsigned char generator_x[] = {
        0x5e, 0xc0, 0x0c, 0xc7, 0x2b, 0xa8, 0x26, 0x26, 0x8e, 0x93, 0x00, 0x8b,
        0xe1, 0x80, 0x3b, 0x43, 0x11, 0x65, 0xb6, 0x2a, 0xf7, 0x1a, 0xae, 0x12,
        0x64, 0xa4, 0xd3, 0xa3, 0x24, 0xe3, 0x6d, 0xea, 0x67, 0x17, 0x0f, 0x47,
        0x70, 0x65, 0x14, 0x9e, 0xda, 0x36, 0xbf, 0x22, 0xa6, 0x15, 0x1d, 0x22,
        0xed, 0x0d, 0xed, 0x6b, 0xc6, 0x70, 0x19, 0x4f, 0x00
    };
    static const unsigned char generator_y[] = {
        0x14, 0xfa, 0x30, 0xf2, 0x5b, 0x79, 0x08, 0x98, 0xad, 0xc8, 0xd7, 0x4e,
        0x2c, 0x13, 0xbd, 0xfd, 0xc4, 0x39, 0x7c, 0xe6, 0x1c, 0xff, 0xd3, 0x3a,
        0xd7, 0xc2, 0xa0, 0x05, 0x1e, 0x9c, 0x78, 0x87, 0x40, 0x98, 0xa3, 0x6c,
        0x73, 0x73, 0xea, 0x4b, 0x62, 0xc7, 0xc9, 0x56, 0x37, 0x20, 0x76, 0x88,
        0x24, 0xbc, 0xb6, 0x6e, 0x71, 0x46, 0x3f, 0x69, 0x00
    };
    unsigned char x_dst[57], buff[114];
    ECX_KEY *key = ossl_ecx_key_new(gctx->libctx, ECX_KEY_TYPE_ED448, 1,
                                    gctx->propq);
    unsigned char *privkey = NULL, *pubkey;
    EVP_MD_CTX *hashctx = NULL;
    EVP_MD *shake = NULL;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_EC_LIB);
        goto err;
    }

    shake = EVP_MD_fetch(gctx->libctx, "SHAKE256", gctx->propq);
    if (shake == NULL)
        goto err;
    if (RAND_priv_bytes_ex(gctx->libctx, privkey, ED448_KEYLEN, 0) <= 0)
        goto err;

    hashctx = EVP_MD_CTX_new();
    if (hashctx == NULL)
        goto err;
    if (EVP_DigestInit_ex(hashctx, shake, NULL) != 1)
        goto err;
    if (EVP_DigestUpdate(hashctx, privkey, 57) != 1)
        goto err;
    if (EVP_DigestFinalXOF(hashctx, buff, sizeof(buff)) != 1)
        goto err;

    buff[0] &= -4;
    buff[55] |= 0x80;
    buff[56] = 0;

    if (s390x_ed448_mul(x_dst, pubkey,
                        generator_x, generator_y, buff) != 1)
        goto err;

    pubkey[56] |= ((x_dst[0] & 0x01) << 7);
    EVP_MD_CTX_free(hashctx);
    EVP_MD_free(shake);
    key->haspubkey = 1;
    return key;
 err:
    ossl_ecx_key_free(key);
    EVP_MD_CTX_free(hashctx);
    EVP_MD_free(shake);
    return NULL;
}
#endif
