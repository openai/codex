/*
 * Copyright 2001-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * PPC support for AES GCM.
 * This file is included by cipher_aes_gcm_hw.c
 */

static int aes_ppc_gcm_initkey(PROV_GCM_CTX *ctx, const unsigned char *key,
                               size_t keylen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    AES_KEY *ks = &actx->ks.ks;

    GCM_HW_SET_KEY_CTR_FN(ks, aes_p8_set_encrypt_key, aes_p8_encrypt,
                          aes_p8_ctr32_encrypt_blocks);
    return 1;
}

static inline u32 UTO32(unsigned char *buf)
{
    return ((u32) buf[0] << 24) | ((u32) buf[1] << 16) | ((u32) buf[2] << 8) | ((u32) buf[3]);
}

static inline u32 add32TOU(unsigned char buf[4], u32 n)
{
    u32 r;

    r = UTO32(buf);
    r += n;
    buf[0] = (unsigned char) (r >> 24) & 0xFF;
    buf[1] = (unsigned char) (r >> 16) & 0xFF;
    buf[2] = (unsigned char) (r >> 8) & 0xFF;
    buf[3] = (unsigned char) r & 0xFF;
    return r;
}

static size_t ppc_aes_gcm_crypt(const unsigned char *in, unsigned char *out, size_t len,
                                const void *key, unsigned char ivec[16], u64 *Xi, int encrypt)
{
    int s = 0;
    int ndone = 0;
    int ctr_reset = 0;
    u64 blocks_unused;
    u64 nb = len / 16;
    u64 next_ctr = 0;
    unsigned char ctr_saved[12];

    memcpy(ctr_saved, ivec, 12);

    while (nb) {
        blocks_unused = (u64) 0xffffffffU + 1 - (u64) UTO32 (ivec + 12);
        if (nb > blocks_unused) {
            len = blocks_unused * 16;
            nb -= blocks_unused;
            next_ctr = blocks_unused;
            ctr_reset = 1;
        } else {
            len = nb * 16;
            next_ctr = nb;
            nb = 0;
        }

        s = encrypt ? ppc_aes_gcm_encrypt(in, out, len, key, ivec, Xi)
                    : ppc_aes_gcm_decrypt(in, out, len, key, ivec, Xi);

        /* add counter to ivec */
        add32TOU(ivec + 12, (u32) next_ctr);
        if (ctr_reset) {
            ctr_reset = 0;
            in += len;
            out += len;
        }
        memcpy(ivec, ctr_saved, 12);
        ndone += s;
    }

    return ndone;
}

static int ppc_aes_gcm_cipher_update(PROV_GCM_CTX *ctx, const unsigned char *in,
                                     size_t len, unsigned char *out)
{
    if (ctx->enc) {
        if (ctx->ctr != NULL) {
            size_t bulk = 0;

            if (len >= AES_GCM_ENC_BYTES && AES_GCM_ASM_PPC(ctx)) {
                size_t res = (16 - ctx->gcm.mres) % 16;

                if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, res))
                    return 0;

                bulk = ppc_aes_gcm_crypt(in + res, out + res, len - res,
                                         ctx->gcm.key,
                                         ctx->gcm.Yi.c, ctx->gcm.Xi.u, 1);

                ctx->gcm.len.u[1] += bulk;
                bulk += res;
            }
            if (CRYPTO_gcm128_encrypt_ctr32(&ctx->gcm, in + bulk, out + bulk,
                                            len - bulk, ctx->ctr))
                return 0;
        } else {
            if (CRYPTO_gcm128_encrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    } else {
        if (ctx->ctr != NULL) {
            size_t bulk = 0;

            if (len >= AES_GCM_DEC_BYTES && AES_GCM_ASM_PPC(ctx)) {
                size_t res = (16 - ctx->gcm.mres) % 16;

                if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, res))
                    return -1;

                bulk = ppc_aes_gcm_crypt(in + res, out + res, len - res,
                                         ctx->gcm.key,
                                         ctx->gcm.Yi.c, ctx->gcm.Xi.u, 0);

                ctx->gcm.len.u[1] += bulk;
                bulk += res;
            }
            if (CRYPTO_gcm128_decrypt_ctr32(&ctx->gcm, in + bulk, out + bulk,
                                            len - bulk, ctx->ctr))
                return 0;
        } else {
            if (CRYPTO_gcm128_decrypt(&ctx->gcm, in, out, len))
                return 0;
        }
    }
    return 1;
}

static const PROV_GCM_HW aes_ppc_gcm = {
    aes_ppc_gcm_initkey,
    ossl_gcm_setiv,
    ossl_gcm_aad_update,
    ppc_aes_gcm_cipher_update,
    ossl_gcm_cipher_final,
    ossl_gcm_one_shot
};

const PROV_GCM_HW *ossl_prov_aes_hw_gcm(size_t keybits)
{
    return PPC_AES_GCM_CAPABLE ? &aes_ppc_gcm : &aes_gcm;
}

