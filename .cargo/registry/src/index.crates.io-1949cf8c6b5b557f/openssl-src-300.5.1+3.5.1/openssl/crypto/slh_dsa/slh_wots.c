/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"

/* For the parameter sets defined there is only one w value */
#define SLH_WOTS_LOGW 4
#define SLH_WOTS_W 16
#define SLH_WOTS_LEN1(n) (2 * (n))
#define SLH_WOTS_LEN2 3
#define SLH_WOTS_CHECKSUM_LEN ((SLH_WOTS_LEN2 + SLH_WOTS_LOGW + 7) / 8)
#define SLH_WOTS_LEN_MAX SLH_WOTS_LEN(SLH_MAX_N)
#define NIBBLE_MASK 15
#define NIBBLE_SHIFT 4

/*
 * @brief Convert a byte array to a byte array of (4 bit) nibbles
 * This is a Variant of the FIPS 205 Algorithm 4 base_2^b function.
 *
 * @param in A byte message to convert
 * @param in_len The size of |in|.
 * @param out The returned array of nibbles, with a size of 2*|in_len|
 */
static ossl_inline void slh_bytes_to_nibbles(const uint8_t *in, size_t in_len,
                                             uint8_t *out)
{
    size_t consumed = 0;

    for (consumed = 0; consumed < in_len; consumed++) {
        *out++ = (*in >> NIBBLE_SHIFT);
        *out++ = (*in++ & NIBBLE_MASK);
    }
}

/*
 * With w = 16 the maximum checksum is 0xF * n which fits into 12 bits
 * which is 3 nibbles.
 *
 * This is effectively a cutdown version of Algorithm 7: steps 3 to 6
 * which does a complicated base2^b(tobyte()) operation.
 */
static ossl_inline void compute_checksum_nibbles(const uint8_t *in, size_t in_len,
                                                 uint8_t *out)
{
    size_t i;
    uint16_t csum = 0;

    /* Compute checksum */
    for (i = 0; i < in_len; ++i)
        csum += in[i];
    /*
     * This line is effectively the same as doing csum += NIBBLE_MASK - in[i]
     * in the loop above.
     */
    csum = (uint16_t)(NIBBLE_MASK * in_len) - csum;

    /* output checksum as 3 nibbles */
    out[0] = (csum >> (2 * NIBBLE_SHIFT)) & NIBBLE_MASK;
    out[1] = (csum >> NIBBLE_SHIFT) & NIBBLE_MASK;
    out[2] = csum & NIBBLE_MASK;
}

/**
 * @brief WOTS+ Chaining function
 * See FIPS 205 Section 5 Algorithm 5
 *
 * Iterates using a hash function on the input |steps| times starting at index
 * |start|. (Internally the |adrs| hash address is used to update the chaining
 * index).
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param in An input string of |n| bytes
 * @param start_index The chaining start index
 * @param steps The number of iterations starting from |start_index|
 *              Note |start_index| + |steps| < w
 *              (where w = 16 indicates the length of the hash chains)
 * @param pk_seed A public key seed (which is added to the hash)
 * @param adrs An ADRS object which has a type of WOTS_HASH, and has a layer
 *             address, tree address, key pair address and chain address
 * @params wpkt A WPACKET object to write the hash chain to (n bytes are written)
 * @returns 1 on success, or 0 on error.
 */
static int slh_wots_chain(SLH_DSA_HASH_CTX *ctx, const uint8_t *in,
                          uint8_t start_index, uint8_t steps,
                          const uint8_t *pk_seed, uint8_t *adrs, WPACKET *wpkt)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_HASH_FN_DECLARE(hashf, F);
    SLH_ADRS_FN_DECLARE(adrsf, set_hash_address);
    size_t j = start_index, end_index;
    size_t n = key->params->n;
    uint8_t *tmp; /* Pointer into the |wpkt| buffer */
    size_t tmp_len = n;

    if (steps == 0)
        return WPACKET_memcpy(wpkt, in, n);

    if (!WPACKET_allocate_bytes(wpkt, tmp_len, &tmp))
        return 0;

    set_hash_address(adrs, j++);
    if (!F(ctx, pk_seed, adrs, in, n, tmp, tmp_len))
        return 0;

    end_index = start_index + steps;
    for (; j < end_index; ++j) {
        set_hash_address(adrs, j);
        if (!F(ctx, pk_seed, adrs, tmp, n, tmp, tmp_len))
            return 0;
    }
    return 1;
}

/**
 * @brief WOTS+ Public key generation.
 * See FIPS 205 Section 5.1 Algorithm 6
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param sk_seed A private key seed of size |n|
 * @param pk_seed A public key seed of size |n|
 * @param adrs An ADRS object containing the layer address, tree address and
 *             keypair address of the WOTS+ public key to generate.
 * @param pk_out The generated public key of size |n|
 * @param pk_out_len The maximum size of |pk_out|
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_wots_pk_gen(SLH_DSA_HASH_CTX *ctx,
                         const uint8_t *sk_seed, const uint8_t *pk_seed,
                         uint8_t *adrs, uint8_t *pk_out, size_t pk_out_len)
{
    int ret = 0;
    const SLH_DSA_KEY *key = ctx->key;
    size_t n = key->params->n;
    size_t i, len = SLH_WOTS_LEN(n); /* 2 * n + 3 */
    uint8_t sk[SLH_MAX_N];
    uint8_t tmp[SLH_WOTS_LEN_MAX * SLH_MAX_N];
    WPACKET pkt, *tmp_wpkt = &pkt; /* Points to the |tmp| buffer */
    size_t tmp_len = 0;

    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_HASH_FN_DECLARE(hashf, PRF);
    SLH_ADRS_FN_DECLARE(adrsf, set_chain_address);
    SLH_ADRS_DECLARE(sk_adrs);
    SLH_ADRS_DECLARE(wots_pk_adrs);

    if (!WPACKET_init_static_len(tmp_wpkt, tmp, sizeof(tmp), 0))
        return 0;
    adrsf->copy(sk_adrs, adrs);
    adrsf->set_type_and_clear(sk_adrs, SLH_ADRS_TYPE_WOTS_PRF);
    adrsf->copy_keypair_address(sk_adrs, adrs);

    for (i = 0; i < len; ++i) { /* len = 2n + 3 */
        set_chain_address(sk_adrs, i);
        if (!PRF(ctx, pk_seed, sk_seed, sk_adrs, sk, sizeof(sk)))
            goto end;

        set_chain_address(adrs, i);
        if (!slh_wots_chain(ctx, sk, 0, NIBBLE_MASK, pk_seed, adrs, tmp_wpkt))
            goto end;
    }

    if (!WPACKET_get_total_written(tmp_wpkt, &tmp_len)) /* should be n * (2 * n + 3) */
        goto end;
    adrsf->copy(wots_pk_adrs, adrs);
    adrsf->set_type_and_clear(wots_pk_adrs, SLH_ADRS_TYPE_WOTS_PK);
    adrsf->copy_keypair_address(wots_pk_adrs, adrs);
    ret = hashf->T(ctx, pk_seed, wots_pk_adrs, tmp, tmp_len, pk_out, pk_out_len);
end:
    WPACKET_finish(tmp_wpkt);
    OPENSSL_cleanse(tmp, sizeof(tmp));
    OPENSSL_cleanse(sk, n);
    return ret;
}

/**
 * @brief WOTS+ Signature generation
 * See FIPS 205 Section 5.2 Algorithm 7
 *
 * The returned signature size is len * |n| bytes (where len = 2 * |n| + 3).
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param msg An input message of size |n| bytes.
 *            The message is either an XMSS or FORS public key
 * @param sk_seed The private key seed of size |n| bytes
 * @param pk_seed The public key seed  of size |n| bytes
 * @param adrs An address containing the layer address, tree address and key
 *             pair address. The size is either 32 or 22 bytes.
 * @param sig_wpkt A WPACKET object to write the signature to.
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_wots_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                       const uint8_t *sk_seed, const uint8_t *pk_seed,
                       uint8_t *adrs, WPACKET *sig_wpkt)
{
    int ret = 0;
    const SLH_DSA_KEY *key = ctx->key;
    uint8_t msg_and_csum_nibbles[SLH_WOTS_LEN_MAX]; /* size is >= 2 * n + 3 */
    uint8_t sk[SLH_MAX_N];
    size_t i;
    size_t n = key->params->n;
    size_t len1 = SLH_WOTS_LEN1(n); /* 2 * n = the msg length in nibbles */
    size_t len = len1 + SLH_WOTS_LEN2;  /* 2 * n + 3 (3 checksum nibbles) */

    SLH_ADRS_DECLARE(sk_adrs);
    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_HASH_FN_DECLARE(hashf, PRF);
    SLH_ADRS_FN_DECLARE(adrsf, set_chain_address);

    /*
     * Convert n message bytes to 2*n base w=16 integers
     * i.e. Convert message to an array of 2*n nibbles.
     */
    slh_bytes_to_nibbles(msg, n, msg_and_csum_nibbles);
    /* Compute a 12 bit checksum and add it to the end */
    compute_checksum_nibbles(msg_and_csum_nibbles, len1, msg_and_csum_nibbles + len1);

    adrsf->copy(sk_adrs, adrs);
    adrsf->set_type_and_clear(sk_adrs, SLH_ADRS_TYPE_WOTS_PRF);
    adrsf->copy_keypair_address(sk_adrs, adrs);

    for (i = 0; i < len; ++i) {
        set_chain_address(sk_adrs, i);
        /* compute chain i secret */
        if (!PRF(ctx, pk_seed, sk_seed, sk_adrs, sk, sizeof(sk)))
            goto err;
        set_chain_address(adrs, i);
        /* compute chain i signature */
        if (!slh_wots_chain(ctx, sk, 0, msg_and_csum_nibbles[i],
                            pk_seed, adrs, sig_wpkt))
            goto err;
    }
    ret = 1;
err:
    return ret;
}

/**
 * @brief Compute a candidate WOTS+ public key from a message and signature
 * See FIPS 205 Section 5.3 Algorithm 8
 *
 * The size of the signature is len * |n| bytes (where len = 2 * |n| + 3).
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param sig_rpkt A PACKET object to read a WOTS+ signature from
 * @param msg A message of size |n| bytes.
 * @param pk_seed The public key seed of size |n|.
 * @param adrs An ADRS object containing the layer address, tree address and
 *             key pair address that of the WOTS+ key used to sign the message.
 * @param pk_out The returned public key candidate of size |n|
 * @param pk_out_len The maximum size of |pk_out|
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_wots_pk_from_sig(SLH_DSA_HASH_CTX *ctx,
                              PACKET *sig_rpkt, const uint8_t *msg,
                              const uint8_t *pk_seed, uint8_t *adrs,
                              uint8_t *pk_out, size_t pk_out_len)
{
    int ret = 0;
    const SLH_DSA_KEY *key = ctx->key;
    uint8_t msg_and_csum_nibbles[SLH_WOTS_LEN_MAX];
    size_t i;
    size_t n = key->params->n;
    size_t len1 = SLH_WOTS_LEN1(n);
    size_t len = len1 + SLH_WOTS_LEN2; /* 2n + 3 */
    const uint8_t *sig_i;  /* Pointer into |sig_rpkt| buffer */
    uint8_t tmp[SLH_WOTS_LEN_MAX * SLH_MAX_N];
    WPACKET pkt, *tmp_pkt = &pkt;
    size_t tmp_len = 0;

    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_ADRS_FN_DECLARE(adrsf, set_chain_address);
    SLH_ADRS_DECLARE(wots_pk_adrs);

    if (!WPACKET_init_static_len(tmp_pkt, tmp, sizeof(tmp), 0))
        return 0;

    slh_bytes_to_nibbles(msg, n, msg_and_csum_nibbles);
    compute_checksum_nibbles(msg_and_csum_nibbles, len1, msg_and_csum_nibbles + len1);

    /* Compute the end nodes for each of the chains */
    for (i = 0; i < len; ++i) {
        set_chain_address(adrs, i);
        if (!PACKET_get_bytes(sig_rpkt, &sig_i, n)
                || !slh_wots_chain(ctx, sig_i, msg_and_csum_nibbles[i],
                                   NIBBLE_MASK - msg_and_csum_nibbles[i],
                                   pk_seed, adrs, tmp_pkt))
            goto err;
    }
    /* compress the computed public key value */
    adrsf->copy(wots_pk_adrs, adrs);
    adrsf->set_type_and_clear(wots_pk_adrs, SLH_ADRS_TYPE_WOTS_PK);
    adrsf->copy_keypair_address(wots_pk_adrs, adrs);
    if (!WPACKET_get_total_written(tmp_pkt, &tmp_len))
        goto err;
    ret = hashf->T(ctx, pk_seed, wots_pk_adrs, tmp, tmp_len,
                   pk_out, pk_out_len);
 err:
    if (!WPACKET_finish(tmp_pkt))
        ret = 0;
    return ret;
}
