/*
 * Copyright 2011-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * All low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include "cipher_aes_cbc_hmac_sha.h"

#if !defined(AES_CBC_HMAC_SHA_CAPABLE) || !defined(AESNI_CAPABLE)
int ossl_cipher_capable_aes_cbc_hmac_sha1(void)
{
    return 0;
}

const PROV_CIPHER_HW_AES_HMAC_SHA *ossl_prov_cipher_hw_aes_cbc_hmac_sha1(void)
{
    return NULL;
}
#else

# include <openssl/rand.h>
# include "crypto/evp.h"
# include "internal/constant_time.h"

void sha1_block_data_order(void *c, const void *p, size_t len);
void aesni_cbc_sha1_enc(const void *inp, void *out, size_t blocks,
                        const AES_KEY *key, unsigned char iv[16],
                        SHA_CTX *ctx, const void *in0);

int ossl_cipher_capable_aes_cbc_hmac_sha1(void)
{
    return AESNI_CBC_HMAC_SHA_CAPABLE;
}

static int aesni_cbc_hmac_sha1_init_key(PROV_CIPHER_CTX *vctx,
                                        const unsigned char *key, size_t keylen)
{
    int ret;
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_AES_HMAC_SHA1_CTX *sctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;

    if (ctx->base.enc)
        ret = aesni_set_encrypt_key(key, keylen * 8, &ctx->ks);
    else
        ret = aesni_set_decrypt_key(key, keylen * 8, &ctx->ks);

    SHA1_Init(&sctx->head);      /* handy when benchmarking */
    sctx->tail = sctx->head;
    sctx->md = sctx->head;

    ctx->payload_length = NO_PAYLOAD_LENGTH;

    vctx->removetlspad = 1;
    vctx->removetlsfixed = SHA_DIGEST_LENGTH + AES_BLOCK_SIZE;

    return ret < 0 ? 0 : 1;
}

static void sha1_update(SHA_CTX *c, const void *data, size_t len)
{
    const unsigned char *ptr = data;
    size_t res;

    if ((res = c->num)) {
        res = SHA_CBLOCK - res;
        if (len < res)
            res = len;
        SHA1_Update(c, ptr, res);
        ptr += res;
        len -= res;
    }

    res = len % SHA_CBLOCK;
    len -= res;

    if (len) {
        sha1_block_data_order(c, ptr, len / SHA_CBLOCK);

        ptr += len;
        c->Nh += len >> 29;
        c->Nl += len <<= 3;
        if (c->Nl < (unsigned int)len)
            c->Nh++;
    }

    if (res)
        SHA1_Update(c, ptr, res);
}

# if !defined(OPENSSL_NO_MULTIBLOCK)

typedef struct {
    unsigned int A[8], B[8], C[8], D[8], E[8];
} SHA1_MB_CTX;

typedef struct {
    const unsigned char *ptr;
    int blocks;
} HASH_DESC;

typedef struct {
    const unsigned char *inp;
    unsigned char *out;
    int blocks;
    u64 iv[2];
} CIPH_DESC;

void sha1_multi_block(SHA1_MB_CTX *, const HASH_DESC *, int);
void aesni_multi_cbc_encrypt(CIPH_DESC *, void *, int);

static size_t tls1_multi_block_encrypt(void *vctx,
                                       unsigned char *out,
                                       const unsigned char *inp,
                                       size_t inp_len, int n4x)
{                               /* n4x is 1 or 2 */
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_AES_HMAC_SHA1_CTX *sctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;
    HASH_DESC hash_d[8], edges[8];
    CIPH_DESC ciph_d[8];
    unsigned char storage[sizeof(SHA1_MB_CTX) + 32];
    union {
        u64 q[16];
        u32 d[32];
        u8 c[128];
    } blocks[8];
    SHA1_MB_CTX *mctx;
    unsigned int frag, last, packlen, i;
    unsigned int x4 = 4 * n4x, minblocks, processed = 0;
    size_t ret = 0;
    u8 *IVs;
#  if defined(BSWAP8)
    u64 seqnum;
#  endif

    /* ask for IVs in bulk */
    if (RAND_bytes_ex(ctx->base.libctx, (IVs = blocks[0].c), 16 * x4, 0) <= 0)
        return 0;

    mctx = (SHA1_MB_CTX *) (storage + 32 - ((size_t)storage % 32)); /* align */

    frag = (unsigned int)inp_len >> (1 + n4x);
    last = (unsigned int)inp_len + frag - (frag << (1 + n4x));
    if (last > frag && ((last + 13 + 9) % 64) < (x4 - 1)) {
        frag++;
        last -= x4 - 1;
    }

    packlen = 5 + 16 + ((frag + 20 + 16) & -16);

    /* populate descriptors with pointers and IVs */
    hash_d[0].ptr = inp;
    ciph_d[0].inp = inp;
    /* 5+16 is place for header and explicit IV */
    ciph_d[0].out = out + 5 + 16;
    memcpy(ciph_d[0].out - 16, IVs, 16);
    memcpy(ciph_d[0].iv, IVs, 16);
    IVs += 16;

    for (i = 1; i < x4; i++) {
        ciph_d[i].inp = hash_d[i].ptr = hash_d[i - 1].ptr + frag;
        ciph_d[i].out = ciph_d[i - 1].out + packlen;
        memcpy(ciph_d[i].out - 16, IVs, 16);
        memcpy(ciph_d[i].iv, IVs, 16);
        IVs += 16;
    }

#  if defined(BSWAP8)
    memcpy(blocks[0].c, sctx->md.data, 8);
    seqnum = BSWAP8(blocks[0].q[0]);
#  endif
    for (i = 0; i < x4; i++) {
        unsigned int len = (i == (x4 - 1) ? last : frag);
#  if !defined(BSWAP8)
        unsigned int carry, j;
#  endif

        mctx->A[i] = sctx->md.h0;
        mctx->B[i] = sctx->md.h1;
        mctx->C[i] = sctx->md.h2;
        mctx->D[i] = sctx->md.h3;
        mctx->E[i] = sctx->md.h4;

        /* fix seqnum */
#  if defined(BSWAP8)
        blocks[i].q[0] = BSWAP8(seqnum + i);
#  else
        for (carry = i, j = 8; j--;) {
            blocks[i].c[j] = ((u8 *)sctx->md.data)[j] + carry;
            carry = (blocks[i].c[j] - carry) >> (sizeof(carry) * 8 - 1);
        }
#  endif
        blocks[i].c[8] = ((u8 *)sctx->md.data)[8];
        blocks[i].c[9] = ((u8 *)sctx->md.data)[9];
        blocks[i].c[10] = ((u8 *)sctx->md.data)[10];
        /* fix length */
        blocks[i].c[11] = (u8)(len >> 8);
        blocks[i].c[12] = (u8)(len);

        memcpy(blocks[i].c + 13, hash_d[i].ptr, 64 - 13);
        hash_d[i].ptr += 64 - 13;
        hash_d[i].blocks = (len - (64 - 13)) / 64;

        edges[i].ptr = blocks[i].c;
        edges[i].blocks = 1;
    }

    /* hash 13-byte headers and first 64-13 bytes of inputs */
    sha1_multi_block(mctx, edges, n4x);
    /* hash bulk inputs */
#  define MAXCHUNKSIZE    2048
#  if     MAXCHUNKSIZE%64
#   error  "MAXCHUNKSIZE is not divisible by 64"
#  elif   MAXCHUNKSIZE
    /*
     * goal is to minimize pressure on L1 cache by moving in shorter steps,
     * so that hashed data is still in the cache by the time we encrypt it
     */
    minblocks = ((frag <= last ? frag : last) - (64 - 13)) / 64;
    if (minblocks > MAXCHUNKSIZE / 64) {
        for (i = 0; i < x4; i++) {
            edges[i].ptr = hash_d[i].ptr;
            edges[i].blocks = MAXCHUNKSIZE / 64;
            ciph_d[i].blocks = MAXCHUNKSIZE / 16;
        }
        do {
            sha1_multi_block(mctx, edges, n4x);
            aesni_multi_cbc_encrypt(ciph_d, &ctx->ks, n4x);

            for (i = 0; i < x4; i++) {
                edges[i].ptr = hash_d[i].ptr += MAXCHUNKSIZE;
                hash_d[i].blocks -= MAXCHUNKSIZE / 64;
                edges[i].blocks = MAXCHUNKSIZE / 64;
                ciph_d[i].inp += MAXCHUNKSIZE;
                ciph_d[i].out += MAXCHUNKSIZE;
                ciph_d[i].blocks = MAXCHUNKSIZE / 16;
                memcpy(ciph_d[i].iv, ciph_d[i].out - 16, 16);
            }
            processed += MAXCHUNKSIZE;
            minblocks -= MAXCHUNKSIZE / 64;
        } while (minblocks > MAXCHUNKSIZE / 64);
    }
#  endif
#  undef  MAXCHUNKSIZE
    sha1_multi_block(mctx, hash_d, n4x);

    memset(blocks, 0, sizeof(blocks));
    for (i = 0; i < x4; i++) {
        unsigned int len = (i == (x4 - 1) ? last : frag),
            off = hash_d[i].blocks * 64;
        const unsigned char *ptr = hash_d[i].ptr + off;

        off = (len - processed) - (64 - 13) - off; /* remainder actually */
        memcpy(blocks[i].c, ptr, off);
        blocks[i].c[off] = 0x80;
        len += 64 + 13;         /* 64 is HMAC header */
        len *= 8;               /* convert to bits */
        if (off < (64 - 8)) {
#  ifdef BSWAP4
            blocks[i].d[15] = BSWAP4(len);
#  else
            PUTU32(blocks[i].c + 60, len);
#  endif
            edges[i].blocks = 1;
        } else {
#  ifdef BSWAP4
            blocks[i].d[31] = BSWAP4(len);
#  else
            PUTU32(blocks[i].c + 124, len);
#  endif
            edges[i].blocks = 2;
        }
        edges[i].ptr = blocks[i].c;
    }

    /* hash input tails and finalize */
    sha1_multi_block(mctx, edges, n4x);

    memset(blocks, 0, sizeof(blocks));
    for (i = 0; i < x4; i++) {
#  ifdef BSWAP4
        blocks[i].d[0] = BSWAP4(mctx->A[i]);
        mctx->A[i] = sctx->tail.h0;
        blocks[i].d[1] = BSWAP4(mctx->B[i]);
        mctx->B[i] = sctx->tail.h1;
        blocks[i].d[2] = BSWAP4(mctx->C[i]);
        mctx->C[i] = sctx->tail.h2;
        blocks[i].d[3] = BSWAP4(mctx->D[i]);
        mctx->D[i] = sctx->tail.h3;
        blocks[i].d[4] = BSWAP4(mctx->E[i]);
        mctx->E[i] = sctx->tail.h4;
        blocks[i].c[20] = 0x80;
        blocks[i].d[15] = BSWAP4((64 + 20) * 8);
#  else
        PUTU32(blocks[i].c + 0, mctx->A[i]);
        mctx->A[i] = sctx->tail.h0;
        PUTU32(blocks[i].c + 4, mctx->B[i]);
        mctx->B[i] = sctx->tail.h1;
        PUTU32(blocks[i].c + 8, mctx->C[i]);
        mctx->C[i] = sctx->tail.h2;
        PUTU32(blocks[i].c + 12, mctx->D[i]);
        mctx->D[i] = sctx->tail.h3;
        PUTU32(blocks[i].c + 16, mctx->E[i]);
        mctx->E[i] = sctx->tail.h4;
        blocks[i].c[20] = 0x80;
        PUTU32(blocks[i].c + 60, (64 + 20) * 8);
#  endif /* BSWAP */
        edges[i].ptr = blocks[i].c;
        edges[i].blocks = 1;
    }

    /* finalize MACs */
    sha1_multi_block(mctx, edges, n4x);

    for (i = 0; i < x4; i++) {
        unsigned int len = (i == (x4 - 1) ? last : frag), pad, j;
        unsigned char *out0 = out;

        memcpy(ciph_d[i].out, ciph_d[i].inp, len - processed);
        ciph_d[i].inp = ciph_d[i].out;

        out += 5 + 16 + len;

        /* write MAC */
        PUTU32(out + 0, mctx->A[i]);
        PUTU32(out + 4, mctx->B[i]);
        PUTU32(out + 8, mctx->C[i]);
        PUTU32(out + 12, mctx->D[i]);
        PUTU32(out + 16, mctx->E[i]);
        out += 20;
        len += 20;

        /* pad */
        pad = 15 - len % 16;
        for (j = 0; j <= pad; j++)
            *(out++) = pad;
        len += pad + 1;

        ciph_d[i].blocks = (len - processed) / 16;
        len += 16;              /* account for explicit iv */

        /* arrange header */
        out0[0] = ((u8 *)sctx->md.data)[8];
        out0[1] = ((u8 *)sctx->md.data)[9];
        out0[2] = ((u8 *)sctx->md.data)[10];
        out0[3] = (u8)(len >> 8);
        out0[4] = (u8)(len);

        ret += len + 5;
        inp += frag;
    }

    aesni_multi_cbc_encrypt(ciph_d, &ctx->ks, n4x);

    OPENSSL_cleanse(blocks, sizeof(blocks));
    OPENSSL_cleanse(mctx, sizeof(*mctx));

    ctx->multiblock_encrypt_len = ret;
    return ret;
}
# endif /* OPENSSL_NO_MULTIBLOCK */

static int aesni_cbc_hmac_sha1_cipher(PROV_CIPHER_CTX *vctx,
                                      unsigned char *out,
                                      const unsigned char *in, size_t len)
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_AES_HMAC_SHA1_CTX *sctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;
    unsigned int l;
    size_t plen = ctx->payload_length;
    size_t iv = 0; /* explicit IV in TLS 1.1 and later */
    size_t aes_off = 0, blocks;
    size_t sha_off = SHA_CBLOCK - sctx->md.num;

    ctx->payload_length = NO_PAYLOAD_LENGTH;

    if (len % AES_BLOCK_SIZE)
        return 0;

    if (ctx->base.enc) {
        if (plen == NO_PAYLOAD_LENGTH)
            plen = len;
        else if (len !=
                 ((plen + SHA_DIGEST_LENGTH +
                   AES_BLOCK_SIZE) & -AES_BLOCK_SIZE))
            return 0;
        else if (ctx->aux.tls_ver >= TLS1_1_VERSION)
            iv = AES_BLOCK_SIZE;

        if (plen > (sha_off + iv)
                && (blocks = (plen - (sha_off + iv)) / SHA_CBLOCK)) {
            sha1_update(&sctx->md, in + iv, sha_off);

            aesni_cbc_sha1_enc(in, out, blocks, &ctx->ks, ctx->base.iv,
                               &sctx->md, in + iv + sha_off);
            blocks *= SHA_CBLOCK;
            aes_off += blocks;
            sha_off += blocks;
            sctx->md.Nh += blocks >> 29;
            sctx->md.Nl += blocks <<= 3;
            if (sctx->md.Nl < (unsigned int)blocks)
                sctx->md.Nh++;
        } else {
            sha_off = 0;
        }
        sha_off += iv;
        sha1_update(&sctx->md, in + sha_off, plen - sha_off);

        if (plen != len) {      /* "TLS" mode of operation */
            if (in != out)
                memcpy(out + aes_off, in + aes_off, plen - aes_off);

            /* calculate HMAC and append it to payload */
            SHA1_Final(out + plen, &sctx->md);
            sctx->md = sctx->tail;
            sha1_update(&sctx->md, out + plen, SHA_DIGEST_LENGTH);
            SHA1_Final(out + plen, &sctx->md);

            /* pad the payload|hmac */
            plen += SHA_DIGEST_LENGTH;
            for (l = len - plen - 1; plen < len; plen++)
                out[plen] = l;
            /* encrypt HMAC|padding at once */
            aesni_cbc_encrypt(out + aes_off, out + aes_off, len - aes_off,
                              &ctx->ks, ctx->base.iv, 1);
        } else {
            aesni_cbc_encrypt(in + aes_off, out + aes_off, len - aes_off,
                              &ctx->ks, ctx->base.iv, 1);
        }
    } else {
        union {
            unsigned int u[SHA_DIGEST_LENGTH / sizeof(unsigned int)];
            unsigned char c[32 + SHA_DIGEST_LENGTH];
        } mac, *pmac;

        /* arrange cache line alignment */
        pmac = (void *)(((size_t)mac.c + 31) & ((size_t)0 - 32));

        if (plen != NO_PAYLOAD_LENGTH) { /* "TLS" mode of operation */
            size_t inp_len, mask, j, i;
            unsigned int res, maxpad, pad, bitlen;
            int ret = 1;
            union {
                unsigned int u[SHA_LBLOCK];
                unsigned char c[SHA_CBLOCK];
            } *data = (void *)sctx->md.data;

            if ((ctx->aux.tls_aad[plen - 4] << 8 | ctx->aux.tls_aad[plen - 3])
                >= TLS1_1_VERSION) {
                if (len < (AES_BLOCK_SIZE + SHA_DIGEST_LENGTH + 1))
                    return 0;

                /* omit explicit iv */
                memcpy(ctx->base.iv, in, AES_BLOCK_SIZE);

                in += AES_BLOCK_SIZE;
                out += AES_BLOCK_SIZE;
                len -= AES_BLOCK_SIZE;
            } else if (len < (SHA_DIGEST_LENGTH + 1))
                return 0;

            /* decrypt HMAC|padding at once */
            aesni_cbc_encrypt(in, out, len, &ctx->ks, ctx->base.iv, 0);

            /* figure out payload length */
            pad = out[len - 1];
            maxpad = len - (SHA_DIGEST_LENGTH + 1);
            maxpad |= (255 - maxpad) >> (sizeof(maxpad) * 8 - 8);
            maxpad &= 255;

            mask = constant_time_ge(maxpad, pad);
            ret &= mask;
            /*
             * If pad is invalid then we will fail the above test but we must
             * continue anyway because we are in constant time code. However,
             * we'll use the maxpad value instead of the supplied pad to make
             * sure we perform well defined pointer arithmetic.
             */
            pad = constant_time_select(mask, pad, maxpad);

            inp_len = len - (SHA_DIGEST_LENGTH + pad + 1);

            ctx->aux.tls_aad[plen - 2] = inp_len >> 8;
            ctx->aux.tls_aad[plen - 1] = inp_len;

            /* calculate HMAC */
            sctx->md = sctx->head;
            sha1_update(&sctx->md, ctx->aux.tls_aad, plen);

            /* code containing lucky-13 fix */
            len -= SHA_DIGEST_LENGTH; /* amend mac */
            if (len >= (256 + SHA_CBLOCK)) {
                j = (len - (256 + SHA_CBLOCK)) & (0 - SHA_CBLOCK);
                j += SHA_CBLOCK - sctx->md.num;
                sha1_update(&sctx->md, out, j);
                out += j;
                len -= j;
                inp_len -= j;
            }

            /* but pretend as if we hashed padded payload */
            bitlen = sctx->md.Nl + (inp_len << 3); /* at most 18 bits */
# ifdef BSWAP4
            bitlen = BSWAP4(bitlen);
# else
            mac.c[0] = 0;
            mac.c[1] = (unsigned char)(bitlen >> 16);
            mac.c[2] = (unsigned char)(bitlen >> 8);
            mac.c[3] = (unsigned char)bitlen;
            bitlen = mac.u[0];
# endif /* BSWAP */

            pmac->u[0] = 0;
            pmac->u[1] = 0;
            pmac->u[2] = 0;
            pmac->u[3] = 0;
            pmac->u[4] = 0;

            for (res = sctx->md.num, j = 0; j < len; j++) {
                size_t c = out[j];
                mask = (j - inp_len) >> (sizeof(j) * 8 - 8);
                c &= mask;
                c |= 0x80 & ~mask & ~((inp_len - j) >> (sizeof(j) * 8 - 8));
                data->c[res++] = (unsigned char)c;

                if (res != SHA_CBLOCK)
                    continue;

                /* j is not incremented yet */
                mask = 0 - ((inp_len + 7 - j) >> (sizeof(j) * 8 - 1));
                data->u[SHA_LBLOCK - 1] |= bitlen & mask;
                sha1_block_data_order(&sctx->md, data, 1);
                mask &= 0 - ((j - inp_len - 72) >> (sizeof(j) * 8 - 1));
                pmac->u[0] |= sctx->md.h0 & mask;
                pmac->u[1] |= sctx->md.h1 & mask;
                pmac->u[2] |= sctx->md.h2 & mask;
                pmac->u[3] |= sctx->md.h3 & mask;
                pmac->u[4] |= sctx->md.h4 & mask;
                res = 0;
            }

            for (i = res; i < SHA_CBLOCK; i++, j++)
                data->c[i] = 0;

            if (res > SHA_CBLOCK - 8) {
                mask = 0 - ((inp_len + 8 - j) >> (sizeof(j) * 8 - 1));
                data->u[SHA_LBLOCK - 1] |= bitlen & mask;
                sha1_block_data_order(&sctx->md, data, 1);
                mask &= 0 - ((j - inp_len - 73) >> (sizeof(j) * 8 - 1));
                pmac->u[0] |= sctx->md.h0 & mask;
                pmac->u[1] |= sctx->md.h1 & mask;
                pmac->u[2] |= sctx->md.h2 & mask;
                pmac->u[3] |= sctx->md.h3 & mask;
                pmac->u[4] |= sctx->md.h4 & mask;

                memset(data, 0, SHA_CBLOCK);
                j += 64;
            }
            data->u[SHA_LBLOCK - 1] = bitlen;
            sha1_block_data_order(&sctx->md, data, 1);
            mask = 0 - ((j - inp_len - 73) >> (sizeof(j) * 8 - 1));
            pmac->u[0] |= sctx->md.h0 & mask;
            pmac->u[1] |= sctx->md.h1 & mask;
            pmac->u[2] |= sctx->md.h2 & mask;
            pmac->u[3] |= sctx->md.h3 & mask;
            pmac->u[4] |= sctx->md.h4 & mask;

# ifdef BSWAP4
            pmac->u[0] = BSWAP4(pmac->u[0]);
            pmac->u[1] = BSWAP4(pmac->u[1]);
            pmac->u[2] = BSWAP4(pmac->u[2]);
            pmac->u[3] = BSWAP4(pmac->u[3]);
            pmac->u[4] = BSWAP4(pmac->u[4]);
# else
            for (i = 0; i < 5; i++) {
                res = pmac->u[i];
                pmac->c[4 * i + 0] = (unsigned char)(res >> 24);
                pmac->c[4 * i + 1] = (unsigned char)(res >> 16);
                pmac->c[4 * i + 2] = (unsigned char)(res >> 8);
                pmac->c[4 * i + 3] = (unsigned char)res;
            }
# endif /* BSWAP4 */
            len += SHA_DIGEST_LENGTH;
            sctx->md = sctx->tail;
            sha1_update(&sctx->md, pmac->c, SHA_DIGEST_LENGTH);
            SHA1_Final(pmac->c, &sctx->md);

            /* verify HMAC */
            out += inp_len;
            len -= inp_len;
            /* version of code with lucky-13 fix */
            {
                unsigned char *p = out + len - 1 - maxpad - SHA_DIGEST_LENGTH;
                size_t off = out - p;
                unsigned int c, cmask;

                for (res = 0, i = 0, j = 0; j < maxpad + SHA_DIGEST_LENGTH; j++) {
                    c = p[j];
                    cmask =
                        ((int)(j - off - SHA_DIGEST_LENGTH)) >> (sizeof(int) *
                                                                 8 - 1);
                    res |= (c ^ pad) & ~cmask; /* ... and padding */
                    cmask &= ((int)(off - 1 - j)) >> (sizeof(int) * 8 - 1);
                    res |= (c ^ pmac->c[i]) & cmask;
                    i += 1 & cmask;
                }

                res = 0 - ((0 - res) >> (sizeof(res) * 8 - 1));
                ret &= (int)~res;
            }
            return ret;
        } else {
            /* decrypt HMAC|padding at once */
            aesni_cbc_encrypt(in, out, len, &ctx->ks, ctx->base.iv, 0);
            sha1_update(&sctx->md, out, len);
        }
    }

    return 1;
}

/* EVP_CTRL_AEAD_SET_MAC_KEY */
static void aesni_cbc_hmac_sha1_set_mac_key(void *vctx,
                                            const unsigned char *mac, size_t len)
{
    PROV_AES_HMAC_SHA1_CTX *ctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;
    unsigned int i;
    unsigned char hmac_key[64];

    memset(hmac_key, 0, sizeof(hmac_key));

    if (len > (int)sizeof(hmac_key)) {
        SHA1_Init(&ctx->head);
        sha1_update(&ctx->head, mac, len);
        SHA1_Final(hmac_key, &ctx->head);
    } else {
        memcpy(hmac_key, mac, len);
    }

    for (i = 0; i < sizeof(hmac_key); i++)
        hmac_key[i] ^= 0x36; /* ipad */
    SHA1_Init(&ctx->head);
    sha1_update(&ctx->head, hmac_key, sizeof(hmac_key));

    for (i = 0; i < sizeof(hmac_key); i++)
        hmac_key[i] ^= 0x36 ^ 0x5c; /* opad */
    SHA1_Init(&ctx->tail);
    sha1_update(&ctx->tail, hmac_key, sizeof(hmac_key));

    OPENSSL_cleanse(hmac_key, sizeof(hmac_key));
}

/* EVP_CTRL_AEAD_TLS1_AAD */
static int aesni_cbc_hmac_sha1_set_tls1_aad(void *vctx,
                                            unsigned char *aad_rec, int aad_len)
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_AES_HMAC_SHA1_CTX *sctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;
    unsigned char *p = aad_rec;
    unsigned int len;

    if (aad_len != EVP_AEAD_TLS1_AAD_LEN)
        return -1;

    len = p[aad_len - 2] << 8 | p[aad_len - 1];

    if (ctx->base.enc) {
        ctx->payload_length = len;
        if ((ctx->aux.tls_ver =
             p[aad_len - 4] << 8 | p[aad_len - 3]) >= TLS1_1_VERSION) {
            if (len < AES_BLOCK_SIZE)
                return 0;
            len -= AES_BLOCK_SIZE;
            p[aad_len - 2] = len >> 8;
            p[aad_len - 1] = len;
        }
        sctx->md = sctx->head;
        sha1_update(&sctx->md, p, aad_len);
        ctx->tls_aad_pad = (int)(((len + SHA_DIGEST_LENGTH +
                       AES_BLOCK_SIZE) & -AES_BLOCK_SIZE)
                     - len);
        return 1;
    } else {
        memcpy(ctx->aux.tls_aad, aad_rec, aad_len);
        ctx->payload_length = aad_len;
        ctx->tls_aad_pad = SHA_DIGEST_LENGTH;
        return 1;
    }
}

# if !defined(OPENSSL_NO_MULTIBLOCK)

/* EVP_CTRL_TLS1_1_MULTIBLOCK_MAX_BUFSIZE */
static int aesni_cbc_hmac_sha1_tls1_multiblock_max_bufsize(void *vctx)
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;

    OPENSSL_assert(ctx->multiblock_max_send_fragment != 0);
    return (int)(5 + 16
                 + (((int)ctx->multiblock_max_send_fragment + 20 + 16) & -16));
}

/* EVP_CTRL_TLS1_1_MULTIBLOCK_AAD */
static int aesni_cbc_hmac_sha1_tls1_multiblock_aad(
    void *vctx, EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *param)
{
    PROV_AES_HMAC_SHA_CTX *ctx = (PROV_AES_HMAC_SHA_CTX *)vctx;
    PROV_AES_HMAC_SHA1_CTX *sctx = (PROV_AES_HMAC_SHA1_CTX *)vctx;
    unsigned int n4x = 1, x4;
    unsigned int frag, last, packlen, inp_len;

    inp_len = param->inp[11] << 8 | param->inp[12];
    ctx->multiblock_interleave = param->interleave;

    if (ctx->base.enc) {
        if ((param->inp[9] << 8 | param->inp[10]) < TLS1_1_VERSION)
            return -1;

        if (inp_len) {
            if (inp_len < 4096)
                return 0; /* too short */

            if (inp_len >= 8192 && OPENSSL_ia32cap_P[2] & (1 << 5))
                n4x = 2; /* AVX2 */
        } else if ((n4x = param->interleave / 4) && n4x <= 2)
            inp_len = param->len;
        else
            return -1;

        sctx->md = sctx->head;
        sha1_update(&sctx->md, param->inp, 13);

        x4 = 4 * n4x;
        n4x += 1;

        frag = inp_len >> n4x;
        last = inp_len + frag - (frag << n4x);
        if (last > frag && ((last + 13 + 9) % 64 < (x4 - 1))) {
            frag++;
            last -= x4 - 1;
        }

        packlen = 5 + 16 + ((frag + 20 + 16) & -16);
        packlen = (packlen << n4x) - packlen;
        packlen += 5 + 16 + ((last + 20 + 16) & -16);

        param->interleave = x4;
        /* The returned values used by get need to be stored */
        ctx->multiblock_interleave = x4;
        ctx->multiblock_aad_packlen = packlen;
        return 1;
    }
    return -1;      /* not yet */
}

/* EVP_CTRL_TLS1_1_MULTIBLOCK_ENCRYPT */
static int aesni_cbc_hmac_sha1_tls1_multiblock_encrypt(
    void *ctx, EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM *param)
{
    return (int)tls1_multi_block_encrypt(ctx, param->out,
                                         param->inp, param->len,
                                         param->interleave / 4);
}

# endif /* OPENSSL_NO_MULTIBLOCK */

static const PROV_CIPHER_HW_AES_HMAC_SHA cipher_hw_aes_hmac_sha1 = {
    {
      aesni_cbc_hmac_sha1_init_key,
      aesni_cbc_hmac_sha1_cipher
    },
    aesni_cbc_hmac_sha1_set_mac_key,
    aesni_cbc_hmac_sha1_set_tls1_aad,
# if !defined(OPENSSL_NO_MULTIBLOCK)
    aesni_cbc_hmac_sha1_tls1_multiblock_max_bufsize,
    aesni_cbc_hmac_sha1_tls1_multiblock_aad,
    aesni_cbc_hmac_sha1_tls1_multiblock_encrypt
# endif
};

const PROV_CIPHER_HW_AES_HMAC_SHA *ossl_prov_cipher_hw_aes_cbc_hmac_sha1(void)
{
    return &cipher_hw_aes_hmac_sha1;
}

#endif /* !defined(AES_CBC_HMAC_SHA_CAPABLE) || !defined(AESNI_CAPABLE) */
