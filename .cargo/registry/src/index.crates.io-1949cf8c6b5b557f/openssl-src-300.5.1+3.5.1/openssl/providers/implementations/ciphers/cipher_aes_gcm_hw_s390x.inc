/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * IBM S390X support for AES GCM.
 * This file is included by cipher_aes_gcm_hw.c
 */

/* iv + padding length for iv lengths != 12 */
#define S390X_gcm_ivpadlen(i)  ((((i) + 15) >> 4 << 4) + 16)

/* Additional flag or'ed to fc for decryption */
#define S390X_gcm_decrypt_flag(ctx) (((ctx)->enc) ? 0 : S390X_DECRYPT)

#define S390X_gcm_fc(A,C) ((A)->plat.s390x.fc | (A)->plat.s390x.hsflag |\
                            S390X_gcm_decrypt_flag((C)))

static int s390x_aes_gcm_initkey(PROV_GCM_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;

    ctx->key_set = 1;
    memcpy(&actx->plat.s390x.param.kma.k, key, keylen);
    actx->plat.s390x.fc = S390X_AES_FC(keylen);
    return 1;
}

static int s390x_aes_gcm_setiv(PROV_GCM_CTX *ctx, const unsigned char *iv,
                               size_t ivlen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    S390X_KMA_PARAMS *kma = &actx->plat.s390x.param.kma;

    kma->t.g[0] = 0;
    kma->t.g[1] = 0;
    kma->tpcl = 0;
    kma->taadl = 0;
    actx->plat.s390x.mreslen = 0;
    actx->plat.s390x.areslen = 0;
    actx->plat.s390x.kreslen = 0;

    if (ivlen == GCM_IV_DEFAULT_SIZE) {
        memcpy(&kma->j0, iv, ivlen);
        kma->j0.w[3] = 1;
        kma->cv.w = 1;
        actx->plat.s390x.hsflag = 0;
    } else {
        unsigned long long ivbits = ivlen << 3;
        size_t len = S390X_gcm_ivpadlen(ivlen);
        unsigned char iv_zero_pad[S390X_gcm_ivpadlen(GCM_IV_MAX_SIZE)];
        /*
         * The IV length needs to be zero padded to be a multiple of 16 bytes
         * followed by 8 bytes of zeros and 8 bytes for the IV length.
         * The GHASH of this value can then be calculated.
         */
        memcpy(iv_zero_pad, iv, ivlen);
        memset(iv_zero_pad + ivlen, 0, len - ivlen);
        memcpy(iv_zero_pad + len - sizeof(ivbits), &ivbits, sizeof(ivbits));
        /*
         * Calculate the ghash of the iv - the result is stored into the tag
         * param.
         */
        s390x_kma(iv_zero_pad, len, NULL, 0, NULL, actx->plat.s390x.fc, kma);
        actx->plat.s390x.hsflag = S390X_KMA_HS; /* The hash subkey is set */

        /* Copy the 128 bit GHASH result into J0 and clear the tag */
        kma->j0.g[0] = kma->t.g[0];
        kma->j0.g[1] = kma->t.g[1];
        kma->t.g[0] = 0;
        kma->t.g[1] = 0;
        /* Set the 32 bit counter */
        kma->cv.w = kma->j0.w[3];
    }
    return 1;
}

static int s390x_aes_gcm_cipher_final(PROV_GCM_CTX *ctx, unsigned char *tag)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    S390X_KMA_PARAMS *kma = &actx->plat.s390x.param.kma;
    unsigned char out[AES_BLOCK_SIZE];
    unsigned int fc;
    int rc;

    kma->taadl <<= 3;
    kma->tpcl <<= 3;
    fc = S390X_gcm_fc(actx, ctx) | S390X_KMA_LAAD | S390X_KMA_LPC;
    s390x_kma(actx->plat.s390x.ares, actx->plat.s390x.areslen,
              actx->plat.s390x.mres, actx->plat.s390x.mreslen, out,
              fc, kma);

    /* gctx->mres already returned to the caller */
    OPENSSL_cleanse(out, actx->plat.s390x.mreslen);

    if (ctx->enc) {
        ctx->taglen = GCM_TAG_MAX_SIZE;
        memcpy(tag, kma->t.b, ctx->taglen);
        rc = 1;
    } else {
        rc = (CRYPTO_memcmp(tag, kma->t.b, ctx->taglen) == 0);
    }
    return rc;
}

static int s390x_aes_gcm_one_shot(PROV_GCM_CTX *ctx,
                                  unsigned char *aad, size_t aad_len,
                                  const unsigned char *in, size_t in_len,
                                  unsigned char *out,
                                  unsigned char *tag, size_t taglen)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    S390X_KMA_PARAMS *kma = &actx->plat.s390x.param.kma;
    unsigned int fc;
    int rc;

    kma->taadl = aad_len << 3;
    kma->tpcl = in_len << 3;
    fc = S390X_gcm_fc(actx, ctx) | S390X_KMA_LAAD | S390X_KMA_LPC;
    s390x_kma(aad, aad_len, in, in_len, out, fc, kma);

    if (ctx->enc) {
        memcpy(tag, kma->t.b, taglen);
        rc = 1;
    } else {
        rc = (CRYPTO_memcmp(tag, kma->t.b, taglen) == 0);
    }
    return rc;
}

/*
 * Process additional authenticated data. Returns 1 on success. Code is
 * big-endian.
 */
static int s390x_aes_gcm_aad_update(PROV_GCM_CTX *ctx,
                                    const unsigned char *aad, size_t len)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    S390X_KMA_PARAMS *kma = &actx->plat.s390x.param.kma;
    unsigned long long alen;
    unsigned int fc;
    int n, rem;

    /* If already processed pt/ct then error */
    if (kma->tpcl != 0)
        return 0;

    /* update the total aad length */
    alen = kma->taadl + len;
    if (alen > (U64(1) << 61) || (sizeof(len) == 8 && alen < len))
        return 0;
    kma->taadl = alen;

    /* check if there is any existing aad data from a previous add */
    n = actx->plat.s390x.areslen;
    if (n) {
        /* add additional data to a buffer until it has 16 bytes */
        while (n && len) {
            actx->plat.s390x.ares[n] = *aad;
            ++aad;
            --len;
            n = (n + 1) & 0xf;
        }
        /* ctx->ares contains a complete block if offset has wrapped around */
        if (!n) {
            fc = S390X_gcm_fc(actx, ctx);
            s390x_kma(actx->plat.s390x.ares, 16, NULL, 0, NULL, fc, kma);
            actx->plat.s390x.hsflag = S390X_KMA_HS;
        }
        actx->plat.s390x.areslen = n;
    }

    /* If there are leftover bytes (< 128 bits) save them for next time */
    rem = len & 0xf;
    /* Add any remaining 16 byte blocks (128 bit each) */
    len &= ~(size_t)0xf;
    if (len) {
        fc = S390X_gcm_fc(actx, ctx);
        s390x_kma(aad, len, NULL, 0, NULL, fc, kma);
        actx->plat.s390x.hsflag = S390X_KMA_HS;
        aad += len;
    }

    if (rem) {
        actx->plat.s390x.areslen = rem;

        do {
            --rem;
            actx->plat.s390x.ares[rem] = aad[rem];
        } while (rem);
    }
    return 1;
}

/*-
 * En/de-crypt plain/cipher-text and authenticate ciphertext. Returns 1 for
 * success. Code is big-endian.
 */
static int s390x_aes_gcm_cipher_update(PROV_GCM_CTX *ctx,
                                       const unsigned char *in, size_t len,
                                       unsigned char *out)
{
    PROV_AES_GCM_CTX *actx = (PROV_AES_GCM_CTX *)ctx;
    S390X_KMA_PARAMS *kma = &actx->plat.s390x.param.kma;
    const unsigned char *inptr;
    unsigned long long mlen;
    unsigned int fc;
    union {
        unsigned int w[4];
        unsigned char b[16];
    } buf;
    size_t inlen;
    int n, rem, i;

    mlen = kma->tpcl + len;
    if (mlen > ((U64(1) << 36) - 32) || (sizeof(len) == 8 && mlen < len))
        return 0;
    kma->tpcl = mlen;

    fc = S390X_gcm_fc(actx, ctx) | S390X_KMA_LAAD;
    n = actx->plat.s390x.mreslen;
    if (n) {
        inptr = in;
        inlen = len;
        while (n && inlen) {
            actx->plat.s390x.mres[n] = *inptr;
            n = (n + 1) & 0xf;
            ++inptr;
            --inlen;
        }
        /* ctx->mres contains a complete block if offset has wrapped around */
        if (!n) {
            s390x_kma(actx->plat.s390x.ares, actx->plat.s390x.areslen,
                      actx->plat.s390x.mres, 16, buf.b, fc, kma);
            actx->plat.s390x.hsflag = S390X_KMA_HS;
            fc |= S390X_KMA_HS;
            actx->plat.s390x.areslen = 0;

            /* previous call already encrypted/decrypted its remainder,
             * see comment below */
            n = actx->plat.s390x.mreslen;
            while (n) {
                *out = buf.b[n];
                n = (n + 1) & 0xf;
                ++out;
                ++in;
                --len;
            }
            actx->plat.s390x.mreslen = 0;
        }
    }

    rem = len & 0xf;

    len &= ~(size_t)0xf;
    if (len) {
        s390x_kma(actx->plat.s390x.ares, actx->plat.s390x.areslen, in, len, out,
                  fc, kma);
        in += len;
        out += len;
        actx->plat.s390x.hsflag = S390X_KMA_HS;
        actx->plat.s390x.areslen = 0;
    }

    /*-
     * If there is a remainder, it has to be saved such that it can be
     * processed by kma later. However, we also have to do the for-now
     * unauthenticated encryption/decryption part here and now...
     */
    if (rem) {
        if (!actx->plat.s390x.mreslen) {
            buf.w[0] = kma->j0.w[0];
            buf.w[1] = kma->j0.w[1];
            buf.w[2] = kma->j0.w[2];
            buf.w[3] = kma->cv.w + 1;
            s390x_km(buf.b, 16, actx->plat.s390x.kres,
                     fc & 0x1f, &kma->k);
        }

        n = actx->plat.s390x.mreslen;
        for (i = 0; i < rem; i++) {
            actx->plat.s390x.mres[n + i] = in[i];
            out[i] = in[i] ^ actx->plat.s390x.kres[n + i];
        }
        actx->plat.s390x.mreslen += rem;
    }
    return 1;
}

static const PROV_GCM_HW s390x_aes_gcm = {
    s390x_aes_gcm_initkey,
    s390x_aes_gcm_setiv,
    s390x_aes_gcm_aad_update,
    s390x_aes_gcm_cipher_update,
    s390x_aes_gcm_cipher_final,
    s390x_aes_gcm_one_shot
};

const PROV_GCM_HW *ossl_prov_aes_hw_gcm(size_t keybits)
{
    if ((keybits == 128 && S390X_aes_128_gcm_CAPABLE)
         || (keybits == 192 && S390X_aes_192_gcm_CAPABLE)
         || (keybits == 256 && S390X_aes_256_gcm_CAPABLE))
        return &s390x_aes_gcm;
    return &aes_gcm;
}
