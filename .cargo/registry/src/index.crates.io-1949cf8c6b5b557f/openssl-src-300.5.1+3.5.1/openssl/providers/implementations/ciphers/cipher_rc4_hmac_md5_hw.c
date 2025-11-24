/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* RC4_HMAC_MD5 cipher implementation */

/*
 * MD5 and RC4 low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "cipher_rc4_hmac_md5.h"

#define NO_PAYLOAD_LENGTH ((size_t)-1)

#if defined(RC4_ASM)                                                           \
    && defined(MD5_ASM)                                                        \
    && (defined(__x86_64)                                                      \
        || defined(__x86_64__)                                                 \
        || defined(_M_AMD64)                                                   \
        || defined(_M_X64))
# define STITCHED_CALL
# define MOD 32 /* 32 is $MOD from rc4_md5-x86_64.pl */
#else
# define rc4_off 0
# define md5_off 0
#endif

static int cipher_hw_rc4_hmac_md5_initkey(PROV_CIPHER_CTX *bctx,
                                          const uint8_t *key, size_t keylen)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)bctx;

    RC4_set_key(&ctx->ks.ks, keylen, key);
    MD5_Init(&ctx->head);       /* handy when benchmarking */
    ctx->tail = ctx->head;
    ctx->md = ctx->head;
    ctx->payload_length = NO_PAYLOAD_LENGTH;
    bctx->removetlsfixed = MD5_DIGEST_LENGTH;
    return 1;
}

static int cipher_hw_rc4_hmac_md5_cipher(PROV_CIPHER_CTX *bctx,
                                         unsigned char *out,
                                         const unsigned char *in, size_t len)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)bctx;
    RC4_KEY *ks = &ctx->ks.ks;

#if defined(STITCHED_CALL)
    size_t rc4_off = MOD - 1 - (ks->x & (MOD - 1));
    size_t md5_off = MD5_CBLOCK - ctx->md.num, blocks;
    unsigned int l;
#endif
    size_t plen = ctx->payload_length;

    if (plen != NO_PAYLOAD_LENGTH && len != (plen + MD5_DIGEST_LENGTH))
        return 0;

    if (ctx->base.enc) {
        if (plen == NO_PAYLOAD_LENGTH)
            plen = len;
#if defined(STITCHED_CALL)
        /* cipher has to "fall behind" */
        if (rc4_off > md5_off)
            md5_off += MD5_CBLOCK;

        if (plen > md5_off
                && (blocks = (plen - md5_off) / MD5_CBLOCK)
                && (OPENSSL_ia32cap_P[0] & (1 << 20)) == 0) {
            MD5_Update(&ctx->md, in, md5_off);
            RC4(ks, rc4_off, in, out);

            rc4_md5_enc(ks, in + rc4_off, out + rc4_off,
                        &ctx->md, in + md5_off, blocks);
            blocks *= MD5_CBLOCK;
            rc4_off += blocks;
            md5_off += blocks;
            ctx->md.Nh += blocks >> 29;
            ctx->md.Nl += blocks <<= 3;
            if (ctx->md.Nl < (unsigned int)blocks)
                ctx->md.Nh++;
        } else {
            rc4_off = 0;
            md5_off = 0;
        }
#endif
        MD5_Update(&ctx->md, in + md5_off, plen - md5_off);

        if (plen != len) {      /* "TLS" mode of operation */
            if (in != out)
                memcpy(out + rc4_off, in + rc4_off, plen - rc4_off);

            /* calculate HMAC and append it to payload */
            MD5_Final(out + plen, &ctx->md);
            ctx->md = ctx->tail;
            MD5_Update(&ctx->md, out + plen, MD5_DIGEST_LENGTH);
            MD5_Final(out + plen, &ctx->md);
            /* encrypt HMAC at once */
            RC4(ks, len - rc4_off, out + rc4_off, out + rc4_off);
        } else {
            RC4(ks, len - rc4_off, in + rc4_off, out + rc4_off);
        }
    } else {
        unsigned char mac[MD5_DIGEST_LENGTH];

#if defined(STITCHED_CALL)
        /* digest has to "fall behind" */
        if (md5_off > rc4_off)
            rc4_off += 2 * MD5_CBLOCK;
        else
            rc4_off += MD5_CBLOCK;

        if (len > rc4_off
                && (blocks = (len - rc4_off) / MD5_CBLOCK)
                && (OPENSSL_ia32cap_P[0] & (1 << 20)) == 0) {
            RC4(ks, rc4_off, in, out);
            MD5_Update(&ctx->md, out, md5_off);

            rc4_md5_enc(ks, in + rc4_off, out + rc4_off,
                        &ctx->md, out + md5_off, blocks);
            blocks *= MD5_CBLOCK;
            rc4_off += blocks;
            md5_off += blocks;
            l = (ctx->md.Nl + (blocks << 3)) & 0xffffffffU;
            if (l < ctx->md.Nl)
                ctx->md.Nh++;
            ctx->md.Nl = l;
            ctx->md.Nh += blocks >> 29;
        } else {
            md5_off = 0;
            rc4_off = 0;
        }
#endif
        /* decrypt HMAC at once */
        RC4(ks, len - rc4_off, in + rc4_off, out + rc4_off);
        if (plen != NO_PAYLOAD_LENGTH) {
            /* "TLS" mode of operation */
            MD5_Update(&ctx->md, out + md5_off, plen - md5_off);

            /* calculate HMAC and verify it */
            MD5_Final(mac, &ctx->md);
            ctx->md = ctx->tail;
            MD5_Update(&ctx->md, mac, MD5_DIGEST_LENGTH);
            MD5_Final(mac, &ctx->md);

            if (CRYPTO_memcmp(out + plen, mac, MD5_DIGEST_LENGTH))
                return 0;
        } else {
            MD5_Update(&ctx->md, out + md5_off, len - md5_off);
        }
    }

    ctx->payload_length = NO_PAYLOAD_LENGTH;

    return 1;
}

static int cipher_hw_rc4_hmac_md5_tls_init(PROV_CIPHER_CTX *bctx,
                                           unsigned char *aad, size_t aad_len)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)bctx;
    unsigned int len;

    if (aad_len != EVP_AEAD_TLS1_AAD_LEN)
        return 0;

    len = aad[aad_len - 2] << 8 | aad[aad_len - 1];

    if (!bctx->enc) {
        if (len < MD5_DIGEST_LENGTH)
            return 0;
        len -= MD5_DIGEST_LENGTH;
        aad[aad_len - 2] = len >> 8;
        aad[aad_len - 1] = len;
    }
    ctx->payload_length = len;
    ctx->md = ctx->head;
    MD5_Update(&ctx->md, aad, aad_len);

    return MD5_DIGEST_LENGTH;
}

static void cipher_hw_rc4_hmac_md5_init_mackey(PROV_CIPHER_CTX *bctx,
                                               const unsigned char *key,
                                               size_t len)
{
    PROV_RC4_HMAC_MD5_CTX *ctx = (PROV_RC4_HMAC_MD5_CTX *)bctx;
    unsigned int i;
    unsigned char hmac_key[64];

    memset(hmac_key, 0, sizeof(hmac_key));

    if (len > (int)sizeof(hmac_key)) {
        MD5_Init(&ctx->head);
        MD5_Update(&ctx->head, key, len);
        MD5_Final(hmac_key, &ctx->head);
    } else {
        memcpy(hmac_key, key, len);
    }

    for (i = 0; i < sizeof(hmac_key); i++)
        hmac_key[i] ^= 0x36; /* ipad */
    MD5_Init(&ctx->head);
    MD5_Update(&ctx->head, hmac_key, sizeof(hmac_key));

    for (i = 0; i < sizeof(hmac_key); i++)
        hmac_key[i] ^= 0x36 ^ 0x5c; /* opad */
    MD5_Init(&ctx->tail);
    MD5_Update(&ctx->tail, hmac_key, sizeof(hmac_key));

    OPENSSL_cleanse(hmac_key, sizeof(hmac_key));
}

static const PROV_CIPHER_HW_RC4_HMAC_MD5 rc4_hmac_md5_hw = {
    {
      cipher_hw_rc4_hmac_md5_initkey,
      cipher_hw_rc4_hmac_md5_cipher
    },
    cipher_hw_rc4_hmac_md5_tls_init,
    cipher_hw_rc4_hmac_md5_init_mackey
};

const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc4_hmac_md5(size_t keybits)
{
    return (PROV_CIPHER_HW *)&rc4_hmac_md5_hw;
}
