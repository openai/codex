/*
 * Copyright 2001-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * S390X support for AES CCM.
 * This file is included by cipher_aes_ccm_hw.c
 */

#define S390X_CCM_AAD_FLAG 0x40

static int s390x_aes_ccm_initkey(PROV_CCM_CTX *ctx,
                                 const unsigned char *key, size_t keylen)
{
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;

    sctx->ccm.s390x.fc = S390X_AES_FC(keylen);
    memcpy(&sctx->ccm.s390x.kmac.k, key, keylen);
    /* Store encoded m and l. */
    sctx->ccm.s390x.nonce.b[0] = ((ctx->l - 1) & 0x7)
                                | (((ctx->m - 2) >> 1) & 0x7) << 3;
    memset(sctx->ccm.s390x.nonce.b + 1, 0, sizeof(sctx->ccm.s390x.nonce.b));
    sctx->ccm.s390x.blocks = 0;
    ctx->key_set = 1;
    return 1;
}

static int s390x_aes_ccm_setiv(PROV_CCM_CTX *ctx,
                               const unsigned char *nonce, size_t noncelen,
                               size_t mlen)
{
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;

    sctx->ccm.s390x.nonce.b[0] &= ~S390X_CCM_AAD_FLAG;
    sctx->ccm.s390x.nonce.g[1] = mlen;
    memcpy(sctx->ccm.s390x.nonce.b + 1, nonce, 15 - ctx->l);
    return 1;
}

/*-
 * Process additional authenticated data. Code is big-endian.
 */
static int s390x_aes_ccm_setaad(PROV_CCM_CTX *ctx,
                                const unsigned char *aad, size_t alen)
{
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;
    unsigned char *ptr;
    int i, rem;

    if (!alen)
        return 1;

    sctx->ccm.s390x.nonce.b[0] |= S390X_CCM_AAD_FLAG;

    /* Suppress 'type-punned pointer dereference' warning. */
    ptr = sctx->ccm.s390x.buf.b;

    if (alen < ((1 << 16) - (1 << 8))) {
        *(uint16_t *)ptr = alen;
        i = 2;
    } else if (sizeof(alen) == 8
               && alen >= (size_t)1 << (32 % (sizeof(alen) * 8))) {
        *(uint16_t *)ptr = 0xffff;
        *(uint64_t *)(ptr + 2) = alen;
        i = 10;
    } else {
        *(uint16_t *)ptr = 0xfffe;
        *(uint32_t *)(ptr + 2) = alen;
        i = 6;
    }

    while (i < 16 && alen) {
        sctx->ccm.s390x.buf.b[i] = *aad;
        ++aad;
        --alen;
        ++i;
    }
    while (i < 16) {
        sctx->ccm.s390x.buf.b[i] = 0;
        ++i;
    }

    sctx->ccm.s390x.kmac.icv.g[0] = 0;
    sctx->ccm.s390x.kmac.icv.g[1] = 0;
    s390x_kmac(sctx->ccm.s390x.nonce.b, 32, sctx->ccm.s390x.fc,
               &sctx->ccm.s390x.kmac);
    sctx->ccm.s390x.blocks += 2;

    rem = alen & 0xf;
    alen &= ~(size_t)0xf;
    if (alen) {
        s390x_kmac(aad, alen, sctx->ccm.s390x.fc, &sctx->ccm.s390x.kmac);
        sctx->ccm.s390x.blocks += alen >> 4;
        aad += alen;
    }
    if (rem) {
        for (i = 0; i < rem; i++)
            sctx->ccm.s390x.kmac.icv.b[i] ^= aad[i];

        s390x_km(sctx->ccm.s390x.kmac.icv.b, 16,
                 sctx->ccm.s390x.kmac.icv.b, sctx->ccm.s390x.fc,
                 sctx->ccm.s390x.kmac.k);
        sctx->ccm.s390x.blocks++;
    }
    return 1;
}

/*-
 * En/de-crypt plain/cipher-text. Compute tag from plaintext. Returns 1 for
 * success.
 */
static int s390x_aes_ccm_auth_encdec(PROV_CCM_CTX *ctx,
                                     const unsigned char *in,
                                     unsigned char *out, size_t len, int enc)
{
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;
    size_t n, rem;
    unsigned int i, l, num;
    unsigned char flags;

    flags = sctx->ccm.s390x.nonce.b[0];
    if (!(flags & S390X_CCM_AAD_FLAG)) {
        s390x_km(sctx->ccm.s390x.nonce.b, 16, sctx->ccm.s390x.kmac.icv.b,
                 sctx->ccm.s390x.fc, sctx->ccm.s390x.kmac.k);
        sctx->ccm.s390x.blocks++;
    }
    l = flags & 0x7;
    sctx->ccm.s390x.nonce.b[0] = l;

    /*-
     * Reconstruct length from encoded length field
     * and initialize it with counter value.
     */
    n = 0;
    for (i = 15 - l; i < 15; i++) {
        n |= sctx->ccm.s390x.nonce.b[i];
        sctx->ccm.s390x.nonce.b[i] = 0;
        n <<= 8;
    }
    n |= sctx->ccm.s390x.nonce.b[15];
    sctx->ccm.s390x.nonce.b[15] = 1;

    if (n != len)
        return 0;      /* length mismatch */

    if (enc) {
        /* Two operations per block plus one for tag encryption */
        sctx->ccm.s390x.blocks += (((len + 15) >> 4) << 1) + 1;
        if (sctx->ccm.s390x.blocks > (1ULL << 61))
            return 0;      /* too much data */
    }

    num = 0;
    rem = len & 0xf;
    len &= ~(size_t)0xf;

    if (enc) {
        /* mac-then-encrypt */
        if (len)
            s390x_kmac(in, len, sctx->ccm.s390x.fc, &sctx->ccm.s390x.kmac);
        if (rem) {
            for (i = 0; i < rem; i++)
                sctx->ccm.s390x.kmac.icv.b[i] ^= in[len + i];

            s390x_km(sctx->ccm.s390x.kmac.icv.b, 16,
                     sctx->ccm.s390x.kmac.icv.b,
                     sctx->ccm.s390x.fc, sctx->ccm.s390x.kmac.k);
        }

        CRYPTO_ctr128_encrypt_ctr32(in, out, len + rem, &sctx->ccm.ks.ks,
                                    sctx->ccm.s390x.nonce.b, sctx->ccm.s390x.buf.b,
                                    &num, (ctr128_f)AES_ctr32_encrypt);
    } else {
        /* decrypt-then-mac */
        CRYPTO_ctr128_encrypt_ctr32(in, out, len + rem, &sctx->ccm.ks.ks,
                                    sctx->ccm.s390x.nonce.b, sctx->ccm.s390x.buf.b,
                                    &num, (ctr128_f)AES_ctr32_encrypt);

        if (len)
            s390x_kmac(out, len, sctx->ccm.s390x.fc, &sctx->ccm.s390x.kmac);
        if (rem) {
            for (i = 0; i < rem; i++)
                sctx->ccm.s390x.kmac.icv.b[i] ^= out[len + i];

            s390x_km(sctx->ccm.s390x.kmac.icv.b, 16,
                     sctx->ccm.s390x.kmac.icv.b,
                     sctx->ccm.s390x.fc, sctx->ccm.s390x.kmac.k);
        }
    }
    /* encrypt tag */
    for (i = 15 - l; i < 16; i++)
        sctx->ccm.s390x.nonce.b[i] = 0;

    s390x_km(sctx->ccm.s390x.nonce.b, 16, sctx->ccm.s390x.buf.b,
             sctx->ccm.s390x.fc, sctx->ccm.s390x.kmac.k);
    sctx->ccm.s390x.kmac.icv.g[0] ^= sctx->ccm.s390x.buf.g[0];
    sctx->ccm.s390x.kmac.icv.g[1] ^= sctx->ccm.s390x.buf.g[1];

    sctx->ccm.s390x.nonce.b[0] = flags;    /* restore flags field */
    return 1;
}


static int s390x_aes_ccm_gettag(PROV_CCM_CTX *ctx,
                                unsigned char *tag, size_t tlen)
{
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;

    if (tlen > ctx->m)
        return 0;
    memcpy(tag, sctx->ccm.s390x.kmac.icv.b, tlen);
    return 1;
}

static int s390x_aes_ccm_auth_encrypt(PROV_CCM_CTX *ctx,
                                      const unsigned char *in,
                                      unsigned char *out, size_t len,
                                      unsigned char *tag, size_t taglen)
{
    int rv;

    rv = s390x_aes_ccm_auth_encdec(ctx, in, out, len, 1);
    if (rv && tag != NULL)
        rv = s390x_aes_ccm_gettag(ctx, tag, taglen);
    return rv;
}

static int s390x_aes_ccm_auth_decrypt(PROV_CCM_CTX *ctx,
                                      const unsigned char *in,
                                      unsigned char *out, size_t len,
                                      unsigned char *expected_tag,
                                      size_t taglen)
{
    int rv = 0;
    PROV_AES_CCM_CTX *sctx = (PROV_AES_CCM_CTX *)ctx;

    rv = s390x_aes_ccm_auth_encdec(ctx, in, out, len, 0);
    if (rv) {
        if (CRYPTO_memcmp(sctx->ccm.s390x.kmac.icv.b, expected_tag, ctx->m) != 0)
            rv = 0;
    }
    if (rv == 0)
        OPENSSL_cleanse(out, len);
    return rv;
}

static const PROV_CCM_HW s390x_aes_ccm = {
    s390x_aes_ccm_initkey,
    s390x_aes_ccm_setiv,
    s390x_aes_ccm_setaad,
    s390x_aes_ccm_auth_encrypt,
    s390x_aes_ccm_auth_decrypt,
    s390x_aes_ccm_gettag
};

const PROV_CCM_HW *ossl_prov_aes_hw_ccm(size_t keybits)
{
    if ((keybits == 128 && S390X_aes_128_ccm_CAPABLE)
         || (keybits == 192 && S390X_aes_192_ccm_CAPABLE)
         || (keybits == 256 && S390X_aes_256_ccm_CAPABLE))
        return &s390x_aes_ccm;
    return &aes_ccm;
}
