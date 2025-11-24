/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/s390x_arch.h"

static OSSL_FUNC_cipher_encrypt_init_fn s390x_aes_xts_einit;
static OSSL_FUNC_cipher_decrypt_init_fn s390x_aes_xts_dinit;
static OSSL_FUNC_cipher_cipher_fn s390x_aes_xts_cipher;
static OSSL_FUNC_cipher_dupctx_fn s390x_aes_xts_dupctx;

static int s390x_aes_xts_init(void *vctx, const unsigned char *key,
                              size_t keylen, const unsigned char *iv,
                              size_t ivlen, const OSSL_PARAM params[],
                              unsigned int dec)
{
    PROV_AES_XTS_CTX *xctx = (PROV_AES_XTS_CTX *)vctx;
    S390X_KM_XTS_PARAMS *km = &xctx->plat.s390x.param.km;
    unsigned int fc, offs;

    switch (xctx->base.keylen) {
    case 128 / 8 * 2:
        fc = S390X_XTS_AES_128_MSA10;
        offs = 32;
        break;
    case 256 / 8 * 2:
        fc = S390X_XTS_AES_256_MSA10;
        offs = 0;
        break;
    default:
        goto not_supported;
    }

    if (!(OPENSSL_s390xcap_P.km[1] && S390X_CAPBIT(fc)))
        goto not_supported;

    if (iv != NULL) {
        if (ivlen != xctx->base.ivlen
                || ivlen > sizeof(km->tweak)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_IV_LENGTH);
            return 0;
        }
        memcpy(km->tweak, iv, ivlen);
        xctx->plat.s390x.iv_set = 1;
    }

    if (key != NULL) {
        if (keylen != xctx->base.keylen) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
        if (!aes_xts_check_keys_differ(key, keylen / 2, !dec))
            return 0;

        memcpy(km->key + offs, key, keylen);
        xctx->plat.s390x.key_set = 1;
    }

    xctx->plat.s390x.fc = fc | dec;
    xctx->plat.s390x.offset = offs;

    memset(km->nap, 0, sizeof(km->nap));
    km->nap[0] = 0x1;

    return aes_xts_set_ctx_params(xctx, params);

not_supported:
    xctx->plat.s390x.fc = 0;
    xctx->plat.s390x.offset = 0;
    return 0;
}

static int s390x_aes_xts_einit(void *vctx, const unsigned char *key,
                               size_t keylen, const unsigned char *iv,
                               size_t ivlen, const OSSL_PARAM params[])
{
    return s390x_aes_xts_init(vctx, key, keylen, iv, ivlen, params, 0);
}

static int s390x_aes_xts_dinit(void *vctx, const unsigned char *key,
                               size_t keylen, const unsigned char *iv,
                               size_t ivlen, const OSSL_PARAM params[])
{
    return s390x_aes_xts_init(vctx, key, keylen, iv, ivlen, params,
                              S390X_DECRYPT);
}

static void *s390x_aes_xts_dupctx(void *vctx)
{
    PROV_AES_XTS_CTX *in = (PROV_AES_XTS_CTX *)vctx;
    PROV_AES_XTS_CTX *ret = OPENSSL_zalloc(sizeof(*in));

    if (ret != NULL)
        *ret = *in;

    return ret;
}

static int s390x_aes_xts_cipher(void *vctx, unsigned char *out, size_t *outl,
                                size_t outsize, const unsigned char *in,
                                size_t inl)
{
    PROV_AES_XTS_CTX *xctx = (PROV_AES_XTS_CTX *)vctx;
    S390X_KM_XTS_PARAMS *km = &xctx->plat.s390x.param.km;
    unsigned char *param = (unsigned char *)km + xctx->plat.s390x.offset;
    unsigned int fc = xctx->plat.s390x.fc;
    unsigned char tmp[2][AES_BLOCK_SIZE];
    unsigned char nap_n1[AES_BLOCK_SIZE];
    unsigned char drop[AES_BLOCK_SIZE];
    size_t len_incomplete, len_complete;

    if (!ossl_prov_is_running()
            || inl < AES_BLOCK_SIZE
            || in == NULL
            || out == NULL
            || !xctx->plat.s390x.iv_set
            || !xctx->plat.s390x.key_set)
        return 0;

    /*
     * Impose a limit of 2^20 blocks per data unit as specified by
     * IEEE Std 1619-2018.  The earlier and obsolete IEEE Std 1619-2007
     * indicated that this was a SHOULD NOT rather than a MUST NOT.
     * NIST SP 800-38E mandates the same limit.
     */
    if (inl > XTS_MAX_BLOCKS_PER_DATA_UNIT * AES_BLOCK_SIZE) {
        ERR_raise(ERR_LIB_PROV, PROV_R_XTS_DATA_UNIT_IS_TOO_LARGE);
        return 0;
    }

    len_incomplete = inl % AES_BLOCK_SIZE;
    len_complete = (len_incomplete == 0) ? inl :
                       (inl / AES_BLOCK_SIZE - 1) * AES_BLOCK_SIZE;

    if (len_complete > 0)
        s390x_km(in, len_complete, out, fc, param);
    if (len_incomplete == 0)
       goto out;

    memcpy(tmp, in + len_complete, AES_BLOCK_SIZE + len_incomplete);
    /* swap NAP for decrypt */
    if (fc & S390X_DECRYPT) {
        memcpy(nap_n1, km->nap, AES_BLOCK_SIZE);
        s390x_km(tmp[0], AES_BLOCK_SIZE, drop, fc, param);
    }
    s390x_km(tmp[0], AES_BLOCK_SIZE, tmp[0], fc, param);
    if (fc & S390X_DECRYPT)
        memcpy(km->nap, nap_n1, AES_BLOCK_SIZE);

    memcpy(tmp[1] + len_incomplete, tmp[0] + len_incomplete,
           AES_BLOCK_SIZE - len_incomplete);
    s390x_km(tmp[1], AES_BLOCK_SIZE, out + len_complete, fc, param);
    memcpy(out + len_complete + AES_BLOCK_SIZE, tmp[0], len_incomplete);

    /* do not expose temporary data */
    OPENSSL_cleanse(tmp, sizeof(tmp));
out:
    memcpy(xctx->base.iv, km->tweak, AES_BLOCK_SIZE);
    *outl = inl;

    return 1;
}
