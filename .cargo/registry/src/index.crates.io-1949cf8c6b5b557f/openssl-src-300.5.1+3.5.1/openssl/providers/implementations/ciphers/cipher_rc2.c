/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Dispatch functions for RC2 cipher modes ecb, cbc, ofb, cfb */

/*
 * RC2 low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <openssl/proverr.h>
#include "cipher_rc2.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"

#define RC2_40_MAGIC    0xa0
#define RC2_64_MAGIC    0x78
#define RC2_128_MAGIC   0x3a
#define RC2_FLAGS       PROV_CIPHER_FLAG_VARIABLE_LENGTH

static OSSL_FUNC_cipher_encrypt_init_fn rc2_einit;
static OSSL_FUNC_cipher_decrypt_init_fn rc2_dinit;
static OSSL_FUNC_cipher_freectx_fn rc2_freectx;
static OSSL_FUNC_cipher_dupctx_fn rc2_dupctx;
static OSSL_FUNC_cipher_gettable_ctx_params_fn rc2_gettable_ctx_params;
static OSSL_FUNC_cipher_settable_ctx_params_fn rc2_settable_ctx_params;
static OSSL_FUNC_cipher_set_ctx_params_fn rc2_set_ctx_params;

static void rc2_freectx(void *vctx)
{
    PROV_RC2_CTX *ctx = (PROV_RC2_CTX *)vctx;

    ossl_cipher_generic_reset_ctx((PROV_CIPHER_CTX *)vctx);
    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void *rc2_dupctx(void *ctx)
{
    PROV_RC2_CTX *in = (PROV_RC2_CTX *)ctx;
    PROV_RC2_CTX *ret;

    if (!ossl_prov_is_running())
        return NULL;

    ret = OPENSSL_malloc(sizeof(*ret));
    if (ret == NULL)
        return NULL;
    *ret = *in;

    return ret;
}

static int rc2_keybits_to_magic(int keybits)
{
    switch (keybits) {
    case 128:
        return RC2_128_MAGIC;
    case 64:
        return RC2_64_MAGIC;
    case 40:
        return RC2_40_MAGIC;
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_UNSUPPORTED_KEY_SIZE);
    return 0;
}

static int rc2_magic_to_keybits(int magic)
{
    switch (magic) {
    case RC2_128_MAGIC:
        return 128;
    case RC2_64_MAGIC:
        return 64;
    case RC2_40_MAGIC:
        return 40;
    }
    ERR_raise(ERR_LIB_PROV, PROV_R_UNSUPPORTED_KEY_SIZE);
    return 0;
}

static int rc2_einit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_einit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc2_set_ctx_params(ctx, params);
}

static int rc2_dinit(void *ctx, const unsigned char *key, size_t keylen,
                          const unsigned char *iv, size_t ivlen,
                          const OSSL_PARAM params[])
{
    if (!ossl_cipher_generic_dinit(ctx, key, keylen, iv, ivlen, NULL))
        return 0;
    return rc2_set_ctx_params(ctx, params);
}

static int rc2_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    PROV_RC2_CTX *ctx = (PROV_RC2_CTX *)vctx;
    OSSL_PARAM *p, *p1, *p2;

    if (!ossl_cipher_generic_get_ctx_params(vctx, params))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_RC2_KEYBITS);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->key_bits)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
        return 0;
    }
    p1 = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS);
    p2 = OSSL_PARAM_locate(params, OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS_OLD);
    if (p1 != NULL || p2 != NULL) {
        long num;
        int i;
        ASN1_TYPE *type;
        unsigned char *d1 = (p1 == NULL) ? NULL : p1->data;
        unsigned char *d2 = (p2 == NULL) ? NULL : p2->data;
        unsigned char **dd1 = d1 == NULL ? NULL : &d1;
        unsigned char **dd2 = d2 == NULL ? NULL : &d2;

        if ((p1 != NULL && p1->data_type != OSSL_PARAM_OCTET_STRING)
            || (p2 != NULL && p2->data_type != OSSL_PARAM_OCTET_STRING)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
        if ((type = ASN1_TYPE_new()) == NULL) {
            ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
            return 0;
        }

        /* Is this the original IV or the running IV? */
        num = rc2_keybits_to_magic(ctx->key_bits);
        if (!ASN1_TYPE_set_int_octetstring(type, num,
                                           ctx->base.iv, ctx->base.ivlen)) {
            ASN1_TYPE_free(type);
            ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
            return 0;
        }

        /*
         * IF the caller has a buffer, we pray to the gods they got the
         * size right.  There's no way to tell the i2d functions...
         */
        i = i2d_ASN1_TYPE(type, dd1);
        if (p1 != NULL && i >= 0)
            p1->return_size = (size_t)i;

        /*
         * If the buffers differ, redo the i2d on the second buffer.
         * Otherwise, just use |i| as computed above
         */
        if (d1 != d2)
            i = i2d_ASN1_TYPE(type, dd2);
        if (p2 != NULL && i >= 0)
            p2->return_size = (size_t)i;

        ASN1_TYPE_free(type);
        if (i < 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            return 0;
        }
    }
    return 1;
}

static int rc2_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_RC2_CTX *ctx = (PROV_RC2_CTX *)vctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_cipher_var_keylen_set_ctx_params(vctx, params))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_RC2_KEYBITS);
    if (p != NULL) {
         if (!OSSL_PARAM_get_size_t(p, &ctx->key_bits)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
            return 0;
        }
    }
    p = OSSL_PARAM_locate_const(params, OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS);
    if (p != NULL) {
        ASN1_TYPE *type = NULL;
        long num = 0;
        const unsigned char *d = p->data;
        int ret = 1;
        unsigned char iv[16];

        if (p->data_type != OSSL_PARAM_OCTET_STRING
            || ctx->base.ivlen > sizeof(iv)
            || (type = d2i_ASN1_TYPE(NULL, &d, p->data_size)) == NULL
            || ((size_t)ASN1_TYPE_get_int_octetstring(type, &num, iv,
                                                      ctx->base.ivlen)
                != ctx->base.ivlen)
            || !ossl_cipher_generic_initiv(&ctx->base, iv, ctx->base.ivlen)
            || (ctx->key_bits = rc2_magic_to_keybits(num)) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_SET_PARAMETER);
            ret = 0;
        }
        ASN1_TYPE_free(type);
        if (ret == 0)
            return 0;
        /*
         * This code assumes that the caller will call
         * EVP_CipherInit_ex() with a non NULL key in order to setup a key that
         * uses the keylen and keybits that were set here.
         */
        ctx->base.keylen = ctx->key_bits / 8;
    }
    return 1;
}

CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(rc2)
OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_RC2_KEYBITS, NULL),
OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS, NULL, 0),
CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(rc2)

CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(rc2)
OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),
OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_RC2_KEYBITS, NULL),
OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_ALGORITHM_ID_PARAMS, NULL, 0),
CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(rc2)

#define IMPLEMENT_cipher(alg, UCALG, lcmode, UCMODE, flags, kbits, blkbits,    \
                         ivbits, typ)                                          \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lcmode##_get_params;   \
static int alg##_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])          \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn alg##_##kbits##_##lcmode##_newctx;           \
static void *alg##_##kbits##_##lcmode##_newctx(void *provctx)                  \
{                                                                              \
     PROV_##UCALG##_CTX *ctx;                                                  \
     if (!ossl_prov_is_running())                                              \
        return NULL;                                                           \
     ctx = OPENSSL_zalloc(sizeof(*ctx));                                       \
     if (ctx != NULL) {                                                        \
         ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits,              \
                                EVP_CIPH_##UCMODE##_MODE, flags,               \
                                ossl_prov_cipher_hw_##alg##_##lcmode(kbits),   \
                                NULL);                                         \
         ctx->key_bits = kbits;                                                \
     }                                                                         \
     return ctx;                                                               \
}                                                                              \
const OSSL_DISPATCH ossl_##alg##kbits##lcmode##_functions[] = {                \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_##lcmode##_newctx },                    \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))rc2_einit },              \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))rc2_dinit },              \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_cipher_generic_##typ##_update },\
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##typ##_final },  \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lcmode##_get_params },                \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))rc2_get_ctx_params },                                    \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))rc2_gettable_ctx_params },                               \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))rc2_set_ctx_params },                                    \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))rc2_settable_ctx_params },                                \
    OSSL_DISPATCH_END                                                          \
};

/* ossl_rc2128ecb_functions */
IMPLEMENT_cipher(rc2, RC2, ecb, ECB, RC2_FLAGS, 128, 64, 0, block)
/* ossl_rc2128cbc_functions */
IMPLEMENT_cipher(rc2, RC2, cbc, CBC, RC2_FLAGS, 128, 64, 64, block)
/* ossl_rc240cbc_functions */
IMPLEMENT_cipher(rc2, RC2, cbc, CBC, RC2_FLAGS, 40, 64, 64, block)
/* ossl_rc264cbc_functions */
IMPLEMENT_cipher(rc2, RC2, cbc, CBC, RC2_FLAGS, 64, 64, 64, block)

/* ossl_rc2128ofb128_functions */
IMPLEMENT_cipher(rc2, RC2, ofb128, OFB, RC2_FLAGS, 128, 8, 64, stream)
/* ossl_rc2128cfb128_functions */
IMPLEMENT_cipher(rc2, RC2, cfb128, CFB, RC2_FLAGS, 128, 8, 64, stream)
