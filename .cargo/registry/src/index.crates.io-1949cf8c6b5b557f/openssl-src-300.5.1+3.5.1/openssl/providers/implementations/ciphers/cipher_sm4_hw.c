/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "cipher_sm4.h"

static int cipher_hw_sm4_initkey(PROV_CIPHER_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_SM4_CTX *sctx =  (PROV_SM4_CTX *)ctx;
    SM4_KEY *ks = &sctx->ks.ks;

    ctx->ks = ks;
    if (ctx->enc
            || (ctx->mode != EVP_CIPH_ECB_MODE
                && ctx->mode != EVP_CIPH_CBC_MODE)) {
#ifdef HWSM4_CAPABLE
        if (HWSM4_CAPABLE) {
            HWSM4_set_encrypt_key(key, ks);
            ctx->block = (block128_f)HWSM4_encrypt;
            ctx->stream.cbc = NULL;
#ifdef HWSM4_cbc_encrypt
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)HWSM4_cbc_encrypt;
            else
#endif
#ifdef HWSM4_ecb_encrypt
            if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)HWSM4_ecb_encrypt;
            else
#endif
#ifdef HWSM4_ctr32_encrypt_blocks
            if (ctx->mode == EVP_CIPH_CTR_MODE)
                ctx->stream.ctr = (ctr128_f)HWSM4_ctr32_encrypt_blocks;
            else
#endif
            (void)0;            /* terminate potentially open 'else' */
        } else
#endif
#ifdef VPSM4_EX_CAPABLE
        if (VPSM4_EX_CAPABLE) {
            vpsm4_ex_set_encrypt_key(key, ks);
            ctx->block = (block128_f)vpsm4_ex_encrypt;
            ctx->stream.cbc = NULL;
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)vpsm4_ex_cbc_encrypt;
            else if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)vpsm4_ex_ecb_encrypt;
            else if (ctx->mode == EVP_CIPH_CTR_MODE)
                ctx->stream.ctr = (ctr128_f)vpsm4_ex_ctr32_encrypt_blocks;
        } else
#endif
#ifdef VPSM4_CAPABLE
        if (VPSM4_CAPABLE) {
            vpsm4_set_encrypt_key(key, ks);
            ctx->block = (block128_f)vpsm4_encrypt;
            ctx->stream.cbc = NULL;
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)vpsm4_cbc_encrypt;
            else if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)vpsm4_ecb_encrypt;
            else if (ctx->mode == EVP_CIPH_CTR_MODE)
                ctx->stream.ctr = (ctr128_f)vpsm4_ctr32_encrypt_blocks;
        } else
#endif
        {
            ossl_sm4_set_key(key, ks);
            ctx->block = (block128_f)ossl_sm4_encrypt;
        }
    } else {
#ifdef HWSM4_CAPABLE
        if (HWSM4_CAPABLE) {
            HWSM4_set_decrypt_key(key, ks);
            ctx->block = (block128_f)HWSM4_decrypt;
            ctx->stream.cbc = NULL;
#ifdef HWSM4_cbc_encrypt
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)HWSM4_cbc_encrypt;
#endif
#ifdef HWSM4_ecb_encrypt
            if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)HWSM4_ecb_encrypt;
#endif
        } else
#endif
#ifdef VPSM4_EX_CAPABLE
        if (VPSM4_EX_CAPABLE) {
            vpsm4_ex_set_decrypt_key(key, ks);
            ctx->block = (block128_f)vpsm4_ex_decrypt;
            ctx->stream.cbc = NULL;
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)vpsm4_ex_cbc_encrypt;
            else if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)vpsm4_ex_ecb_encrypt;
        } else
#endif
#ifdef VPSM4_CAPABLE
        if (VPSM4_CAPABLE) {
            vpsm4_set_decrypt_key(key, ks);
            ctx->block = (block128_f)vpsm4_decrypt;
            ctx->stream.cbc = NULL;
            if (ctx->mode == EVP_CIPH_CBC_MODE)
                ctx->stream.cbc = (cbc128_f)vpsm4_cbc_encrypt;
            else if (ctx->mode == EVP_CIPH_ECB_MODE)
                ctx->stream.ecb = (ecb128_f)vpsm4_ecb_encrypt;
        } else
#endif
        {
            ossl_sm4_set_key(key, ks);
            ctx->block = (block128_f)ossl_sm4_decrypt;
        }
    }

    return 1;
}

IMPLEMENT_CIPHER_HW_COPYCTX(cipher_hw_sm4_copyctx, PROV_SM4_CTX)

# define PROV_CIPHER_HW_sm4_mode(mode)                                         \
static const PROV_CIPHER_HW sm4_##mode = {                                     \
    cipher_hw_sm4_initkey,                                                     \
    ossl_cipher_hw_generic_##mode,                                             \
    cipher_hw_sm4_copyctx                                                      \
};                                                                             \
PROV_CIPHER_HW_declare(mode)                                                   \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_sm4_##mode(size_t keybits)           \
{                                                                              \
    PROV_CIPHER_HW_select(mode)                                                \
    return &sm4_##mode;                                                        \
}

#if defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
# include "cipher_sm4_hw_rv64i.inc"
#else
/* The generic case */
# define PROV_CIPHER_HW_declare(mode)
# define PROV_CIPHER_HW_select(mode)
#endif

PROV_CIPHER_HW_sm4_mode(cbc)
PROV_CIPHER_HW_sm4_mode(ecb)
PROV_CIPHER_HW_sm4_mode(ofb128)
PROV_CIPHER_HW_sm4_mode(cfb128)
PROV_CIPHER_HW_sm4_mode(ctr)
