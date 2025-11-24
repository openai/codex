/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "cipher_sm4_xts.h"

#define XTS_SET_KEY_FN(fn_set_enc_key, fn_set_dec_key,                         \
                       fn_block_enc, fn_block_dec,                             \
                       fn_stream, fn_stream_gb) {                              \
    size_t bytes = keylen / 2;                                                 \
                                                                               \
    if (ctx->enc) {                                                            \
        fn_set_enc_key(key, &xctx->ks1.ks);                                    \
        xctx->xts.block1 = (block128_f)fn_block_enc;                           \
    } else {                                                                   \
        fn_set_dec_key(key, &xctx->ks1.ks);                                    \
        xctx->xts.block1 = (block128_f)fn_block_dec;                           \
    }                                                                          \
    fn_set_enc_key(key + bytes, &xctx->ks2.ks);                                \
    xctx->xts.block2 = (block128_f)fn_block_enc;                               \
    xctx->xts.key1 = &xctx->ks1;                                               \
    xctx->xts.key2 = &xctx->ks2;                                               \
    xctx->stream = fn_stream;                                                  \
    xctx->stream_gb = fn_stream_gb;                                            \
}

static int cipher_hw_sm4_xts_generic_initkey(PROV_CIPHER_CTX *ctx,
                                             const unsigned char *key,
                                             size_t keylen)
{
    PROV_SM4_XTS_CTX *xctx = (PROV_SM4_XTS_CTX *)ctx;
    OSSL_xts_stream_fn stream = NULL;
    OSSL_xts_stream_fn stream_gb = NULL;
#ifdef HWSM4_CAPABLE
    if (HWSM4_CAPABLE) {
        XTS_SET_KEY_FN(HWSM4_set_encrypt_key, HWSM4_set_decrypt_key,
                       HWSM4_encrypt, HWSM4_decrypt, stream, stream_gb);
        return 1;
    } else
#endif /* HWSM4_CAPABLE */
#ifdef VPSM4_EX_CAPABLE
    if (VPSM4_EX_CAPABLE) {
        stream = vpsm4_ex_xts_encrypt;
        stream_gb = vpsm4_ex_xts_encrypt_gb;
        XTS_SET_KEY_FN(vpsm4_ex_set_encrypt_key, vpsm4_ex_set_decrypt_key,
                       vpsm4_ex_encrypt, vpsm4_ex_decrypt, stream, stream_gb);
        return 1;
    } else
#endif /* VPSM4_EX_CAPABLE */
#ifdef VPSM4_CAPABLE
    if (VPSM4_CAPABLE) {
        stream = vpsm4_xts_encrypt;
        stream_gb = vpsm4_xts_encrypt_gb;
        XTS_SET_KEY_FN(vpsm4_set_encrypt_key, vpsm4_set_decrypt_key,
                       vpsm4_encrypt, vpsm4_decrypt, stream, stream_gb);
        return 1;
    } else
#endif /* VPSM4_CAPABLE */
    {
        (void)0;
    }
    {
        XTS_SET_KEY_FN(ossl_sm4_set_key, ossl_sm4_set_key, ossl_sm4_encrypt,
                       ossl_sm4_decrypt, stream, stream_gb);
    }
    return 1;
}

static void cipher_hw_sm4_xts_copyctx(PROV_CIPHER_CTX *dst,
                                      const PROV_CIPHER_CTX *src)
{
    PROV_SM4_XTS_CTX *sctx = (PROV_SM4_XTS_CTX *)src;
    PROV_SM4_XTS_CTX *dctx = (PROV_SM4_XTS_CTX *)dst;

    *dctx = *sctx;
    dctx->xts.key1 = &dctx->ks1.ks;
    dctx->xts.key2 = &dctx->ks2.ks;
}


static const PROV_CIPHER_HW sm4_generic_xts = {
    cipher_hw_sm4_xts_generic_initkey,
    NULL,
    cipher_hw_sm4_xts_copyctx
};

#if defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_sm4_xts_hw_rv64i.inc"
#else
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_xts(size_t keybits)
{
    return &sm4_generic_xts;
}
#endif
