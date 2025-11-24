/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Helper functions for 128 bit CBC CTS ciphers (Currently AES and Camellia).
 *
 * The function dispatch tables are embedded into cipher_aes.c
 * and cipher_camellia.c using cipher_aes_cts.inc and cipher_camellia_cts.inc
 */

/*
 * Refer to SP800-38A-Addendum
 *
 * Ciphertext stealing encrypts plaintext using a block cipher, without padding
 * the message to a multiple of the block size, so the ciphertext is the same
 * size as the plaintext.
 * It does this by altering processing of the last two blocks of the message.
 * The processing of all but the last two blocks is unchanged, but a portion of
 * the second-last block's ciphertext is "stolen" to pad the last plaintext
 * block. The padded final block is then encrypted as usual.
 * The final ciphertext for the last two blocks, consists of the partial block
 * (with the "stolen" portion omitted) plus the full final block,
 * which are the same size as the original plaintext.
 * Decryption requires decrypting the final block first, then restoring the
 * stolen ciphertext to the partial block, which can then be decrypted as usual.

 * AES_CBC_CTS has 3 variants:
 *  (1) CS1 The NIST variant.
 *      If the length is a multiple of the blocksize it is the same as CBC mode.
 *      otherwise it produces C1||C2||(C(n-1))*||Cn.
 *      Where C(n-1)* is a partial block.
 *  (2) CS2
 *      If the length is a multiple of the blocksize it is the same as CBC mode.
 *      otherwise it produces C1||C2||Cn||(C(n-1))*.
 *      Where C(n-1)* is a partial block.
 *  (3) CS3 The Kerberos5 variant.
 *      Produces C1||C2||Cn||(C(n-1))* regardless of the length.
 *      If the length is a multiple of the blocksize it looks similar to CBC mode
 *      with the last 2 blocks swapped.
 *      Otherwise it is the same as CS2.
 */

#include <openssl/core_names.h>
#include "prov/ciphercommon.h"
#include "internal/nelem.h"
#include "cipher_cts.h"

/* The value assigned to 0 is the default */
#define CTS_CS1 0
#define CTS_CS2 1
#define CTS_CS3 2

#define CTS_BLOCK_SIZE 16

typedef union {
    size_t align;
    unsigned char c[CTS_BLOCK_SIZE];
} aligned_16bytes;

typedef struct cts_mode_name2id_st {
    unsigned int id;
    const char *name;
} CTS_MODE_NAME2ID;

static CTS_MODE_NAME2ID cts_modes[] = {
    { CTS_CS1, OSSL_CIPHER_CTS_MODE_CS1 },
    { CTS_CS2, OSSL_CIPHER_CTS_MODE_CS2 },
    { CTS_CS3, OSSL_CIPHER_CTS_MODE_CS3 },
};

const char *ossl_cipher_cbc_cts_mode_id2name(unsigned int id)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(cts_modes); ++i) {
        if (cts_modes[i].id == id)
            return cts_modes[i].name;
    }
    return NULL;
}

int ossl_cipher_cbc_cts_mode_name2id(const char *name)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(cts_modes); ++i) {
        if (OPENSSL_strcasecmp(name, cts_modes[i].name) == 0)
            return (int)cts_modes[i].id;
    }
    return -1;
}

static size_t cts128_cs1_encrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    aligned_16bytes tmp_in;
    size_t residue;

    residue = len % CTS_BLOCK_SIZE;
    len -= residue;
    if (!ctx->hw->cipher(ctx, out, in, len))
        return 0;

    if (residue == 0)
        return len;

    in += len;
    out += len;

    memset(tmp_in.c, 0, sizeof(tmp_in));
    memcpy(tmp_in.c, in, residue);
    if (!ctx->hw->cipher(ctx, out - CTS_BLOCK_SIZE + residue, tmp_in.c,
                         CTS_BLOCK_SIZE))
        return 0;
    return len + residue;
}

static void do_xor(const unsigned char *in1, const unsigned char *in2,
                   size_t len, unsigned char *out)
{
    size_t i;

    for (i = 0; i < len; ++i)
        out[i] = in1[i] ^ in2[i];
}

static size_t cts128_cs1_decrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    aligned_16bytes mid_iv, ct_mid, cn, pt_last;
    size_t residue;

    residue = len % CTS_BLOCK_SIZE;
    if (residue == 0) {
        /* If there are no partial blocks then it is the same as CBC mode */
        if (!ctx->hw->cipher(ctx, out, in, len))
            return 0;
        return len;
    }
    /* Process blocks at the start - but leave the last 2 blocks */
    len -= CTS_BLOCK_SIZE + residue;
    if (len > 0) {
        if (!ctx->hw->cipher(ctx, out, in, len))
            return 0;
        in += len;
        out += len;
    }
    /* Save the iv that will be used by the second last block */
    memcpy(mid_iv.c, ctx->iv, CTS_BLOCK_SIZE);
    /* Save the C(n) block */
    memcpy(cn.c, in + residue, CTS_BLOCK_SIZE);

    /* Decrypt the last block first using an iv of zero */
    memset(ctx->iv, 0, CTS_BLOCK_SIZE);
    if (!ctx->hw->cipher(ctx, pt_last.c, in + residue, CTS_BLOCK_SIZE))
        return 0;

    /*
     * Rebuild the ciphertext of the second last block as a combination of
     * the decrypted last block + replace the start with the ciphertext bytes
     * of the partial second last block.
     */
    memcpy(ct_mid.c, in, residue);
    memcpy(ct_mid.c + residue, pt_last.c + residue, CTS_BLOCK_SIZE - residue);
    /*
     * Restore the last partial ciphertext block.
     * Now that we have the cipher text of the second last block, apply
     * that to the partial plaintext end block. We have already decrypted the
     * block using an IV of zero. For decryption the IV is just XORed after
     * doing an Cipher CBC block - so just XOR in the cipher text.
     */
    do_xor(ct_mid.c, pt_last.c, residue, out + CTS_BLOCK_SIZE);

    /* Restore the iv needed by the second last block */
    memcpy(ctx->iv, mid_iv.c, CTS_BLOCK_SIZE);

    /*
     * Decrypt the second last plaintext block now that we have rebuilt the
     * ciphertext.
     */
    if (!ctx->hw->cipher(ctx, out, ct_mid.c, CTS_BLOCK_SIZE))
        return 0;

    /* The returned iv is the C(n) block */
    memcpy(ctx->iv, cn.c, CTS_BLOCK_SIZE);
    return len + CTS_BLOCK_SIZE + residue;
}

static size_t cts128_cs3_encrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    aligned_16bytes tmp_in;
    size_t residue;

    if (len < CTS_BLOCK_SIZE)  /* CS3 requires at least one block */
        return 0;

    /* If we only have one block then just process the aligned block */
    if (len == CTS_BLOCK_SIZE)
        return ctx->hw->cipher(ctx, out, in, len) ? len : 0;

    residue = len % CTS_BLOCK_SIZE;
    if (residue == 0)
        residue = CTS_BLOCK_SIZE;
    len -= residue;

    if (!ctx->hw->cipher(ctx, out, in, len))
        return 0;

    in += len;
    out += len;

    memset(tmp_in.c, 0, sizeof(tmp_in));
    memcpy(tmp_in.c, in, residue);
    memcpy(out, out - CTS_BLOCK_SIZE, residue);
    if (!ctx->hw->cipher(ctx, out - CTS_BLOCK_SIZE, tmp_in.c, CTS_BLOCK_SIZE))
        return 0;
    return len + residue;
}

/*
 * Note:
 *  The cipher text (in) is of the form C(0), C(1), ., C(n), C(n-1)* where
 *  C(n) is a full block and C(n-1)* can be a partial block
 *  (but could be a full block).
 *  This means that the output plaintext (out) needs to swap the plaintext of
 *  the last two decoded ciphertext blocks.
 */
static size_t cts128_cs3_decrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    aligned_16bytes mid_iv, ct_mid, cn, pt_last;
    size_t residue;

    if (len < CTS_BLOCK_SIZE) /* CS3 requires at least one block */
        return 0;

    /* If we only have one block then just process the aligned block */
    if (len == CTS_BLOCK_SIZE)
        return ctx->hw->cipher(ctx, out, in, len) ? len : 0;

    /* Process blocks at the start - but leave the last 2 blocks */
    residue = len % CTS_BLOCK_SIZE;
    if (residue == 0)
        residue = CTS_BLOCK_SIZE;
    len -= CTS_BLOCK_SIZE + residue;

    if (len > 0) {
        if (!ctx->hw->cipher(ctx, out, in, len))
            return 0;
        in += len;
        out += len;
    }
    /* Save the iv that will be used by the second last block */
    memcpy(mid_iv.c, ctx->iv, CTS_BLOCK_SIZE);
    /* Save the C(n) block : For CS3 it is C(1)||...||C(n-2)||C(n)||C(n-1)* */
    memcpy(cn.c, in, CTS_BLOCK_SIZE);

    /* Decrypt the C(n) block first using an iv of zero */
    memset(ctx->iv, 0, CTS_BLOCK_SIZE);
    if (!ctx->hw->cipher(ctx, pt_last.c, in, CTS_BLOCK_SIZE))
        return 0;

    /*
     * Rebuild the ciphertext of C(n-1) as a combination of
     * the decrypted C(n) block + replace the start with the ciphertext bytes
     * of the partial last block.
     */
    memcpy(ct_mid.c, in + CTS_BLOCK_SIZE, residue);
    if (residue != CTS_BLOCK_SIZE)
        memcpy(ct_mid.c + residue, pt_last.c + residue, CTS_BLOCK_SIZE - residue);
    /*
     * Restore the last partial ciphertext block.
     * Now that we have the cipher text of the second last block, apply
     * that to the partial plaintext end block. We have already decrypted the
     * block using an IV of zero. For decryption the IV is just XORed after
     * doing an AES block - so just XOR in the ciphertext.
     */
    do_xor(ct_mid.c, pt_last.c, residue, out + CTS_BLOCK_SIZE);

    /* Restore the iv needed by the second last block */
    memcpy(ctx->iv, mid_iv.c, CTS_BLOCK_SIZE);
    /*
     * Decrypt the second last plaintext block now that we have rebuilt the
     * ciphertext.
     */
    if (!ctx->hw->cipher(ctx, out, ct_mid.c, CTS_BLOCK_SIZE))
        return 0;

    /* The returned iv is the C(n) block */
    memcpy(ctx->iv, cn.c, CTS_BLOCK_SIZE);
    return len + CTS_BLOCK_SIZE + residue;
}

static size_t cts128_cs2_encrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    if (len % CTS_BLOCK_SIZE == 0) {
        /* If there are no partial blocks then it is the same as CBC mode */
        if (!ctx->hw->cipher(ctx, out, in, len))
            return 0;
        return len;
    }
    /* For partial blocks CS2 is equivalent to CS3 */
    return cts128_cs3_encrypt(ctx, in, out, len);
}

static size_t cts128_cs2_decrypt(PROV_CIPHER_CTX *ctx, const unsigned char *in,
                                 unsigned char *out, size_t len)
{
    if (len % CTS_BLOCK_SIZE == 0) {
        /* If there are no partial blocks then it is the same as CBC mode */
        if (!ctx->hw->cipher(ctx, out, in, len))
            return 0;
        return len;
    }
    /* For partial blocks CS2 is equivalent to CS3 */
    return cts128_cs3_decrypt(ctx, in, out, len);
}

int ossl_cipher_cbc_cts_block_update(void *vctx, unsigned char *out, size_t *outl,
                                     size_t outsize, const unsigned char *in,
                                     size_t inl)
{
    PROV_CIPHER_CTX *ctx = (PROV_CIPHER_CTX *)vctx;
    size_t sz = 0;

    if (inl < CTS_BLOCK_SIZE) /* There must be at least one block for CTS mode */
        return 0;
    if (outsize < inl)
        return 0;
    if (out == NULL) {
        *outl = inl;
        return 1;
    }

    /*
     * Return an error if the update is called multiple times, only one shot
     * is supported.
     */
    if (ctx->updated == 1)
        return 0;

    if (ctx->enc) {
        if (ctx->cts_mode == CTS_CS1)
            sz = cts128_cs1_encrypt(ctx, in, out, inl);
        else if (ctx->cts_mode == CTS_CS2)
            sz = cts128_cs2_encrypt(ctx, in, out, inl);
        else if (ctx->cts_mode == CTS_CS3)
            sz = cts128_cs3_encrypt(ctx, in, out, inl);
    } else {
        if (ctx->cts_mode == CTS_CS1)
            sz = cts128_cs1_decrypt(ctx, in, out, inl);
        else if (ctx->cts_mode == CTS_CS2)
            sz = cts128_cs2_decrypt(ctx, in, out, inl);
        else if (ctx->cts_mode == CTS_CS3)
            sz = cts128_cs3_decrypt(ctx, in, out, inl);
    }
    if (sz == 0)
        return 0;
    ctx->updated = 1; /* Stop multiple updates being allowed */
    *outl = sz;
    return 1;
}

int ossl_cipher_cbc_cts_block_final(void *vctx, unsigned char *out, size_t *outl,
                                    size_t outsize)
{
    *outl = 0;
    return 1;
}
