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

/* k = 14, 17, 22, 33, 35 (number of trees) */
#define SLH_MAX_K           35
/* a = 6, 8, 9, 12 or 14  - There are (2^a) merkle trees */
#define SLH_MAX_A           9

#define SLH_MAX_K_TIMES_A      (SLH_MAX_A * SLH_MAX_K)
#define SLH_MAX_ROOTS          (SLH_MAX_K_TIMES_A * SLH_MAX_N)

static void slh_base_2b(const uint8_t *in, uint32_t b, uint32_t *out, size_t out_len);

/**
 * @brief Generate FORS secret values
 * See FIPS 205 Section 8.1 Algorithm 14.
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param sk_seed A private key seed of size |n|
 * @param pk_seed A public key seed of size |n|
 * @param adrs An ADRS object containing the layer address of zero, with the
 *             tree address and key pair address set to the index of the WOTS+
 *             key within the XMSS tree that signs the FORS key.
 * @param id The index of the FORS secret value within the sets of FORS trees.
 *               (which must be < 2^(hm - height)
 * @param pk_out The generated FORS secret value of size |n|
 * @param pk_out_len The maximum size of |pk_out|
 * @returns 1 on success, or 0 on error.
 */
static int slh_fors_sk_gen(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_seed,
                           const uint8_t *pk_seed, uint8_t *adrs, uint32_t id,
                           uint8_t *pk_out, size_t pk_out_len)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_ADRS_DECLARE(sk_adrs);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);

    adrsf->copy(sk_adrs, adrs);
    adrsf->set_type_and_clear(sk_adrs, SLH_ADRS_TYPE_FORS_PRF);
    adrsf->copy_keypair_address(sk_adrs, adrs);
    adrsf->set_tree_index(sk_adrs, id);
    return key->hash_func->PRF(ctx, pk_seed, sk_seed, sk_adrs, pk_out, pk_out_len);
}

/**
 * @brief Computes the nodes of a Merkle tree.
 * See FIPS 205 Section 8.2 Algorithm 18
 *
 * The leaf nodes are hashes of FORS secret values.
 * Each parent node is a hash of its 2 children.
 * Note this is a recursive function.
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param sk_seed A SLH_DSA private key seed of size |n|
 * @param pk_seed A SLH_DSA public key seed of size |n|
 * @param adrs The ADRS object must have a layer address of zero, and the
 *             tree address set to the XMSS tree that signs the FORS key,
 *             the type set to FORS_TREE, and the keypair address set to the
 *             index of the WOTS+ key that signs the FORS key.
 * @param node_id The target node index
 * @param height The target node height
 * @param node The returned hash for a node of size|n|
 * @param node_len The maximum size of |node|
 * @returns 1 on success, or 0 on error.
 */
static int slh_fors_node(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_seed,
                         const uint8_t *pk_seed, uint8_t *adrs, uint32_t node_id,
                         uint32_t height, uint8_t *node, size_t node_len)
{
    int ret = 0;
    const SLH_DSA_KEY *key = ctx->key;
    uint8_t sk[SLH_MAX_N], lnode[SLH_MAX_N], rnode[SLH_MAX_N];
    uint32_t n = key->params->n;

    SLH_ADRS_FUNC_DECLARE(key, adrsf);

    if (height == 0) {
        /* Gets here for leaf nodes */
        if (!slh_fors_sk_gen(ctx, sk_seed, pk_seed, adrs, node_id, sk, sizeof(sk)))
            return 0;
        adrsf->set_tree_height(adrs, 0);
        adrsf->set_tree_index(adrs, node_id);
        ret = key->hash_func->F(ctx, pk_seed, adrs, sk, n, node, node_len);
        OPENSSL_cleanse(sk, n);
        return ret;
    } else {
        if (!slh_fors_node(ctx, sk_seed, pk_seed, adrs, 2 * node_id, height - 1,
                           lnode, sizeof(rnode))
                || !slh_fors_node(ctx, sk_seed, pk_seed, adrs, 2 * node_id + 1,
                                  height - 1, rnode, sizeof(rnode)))
            return 0;
        adrsf->set_tree_height(adrs, height);
        adrsf->set_tree_index(adrs, node_id);
        if (!key->hash_func->H(ctx, pk_seed, adrs, lnode, rnode, node, node_len))
            return 0;
    }
    return 1;
}

/**
 * @brief Generate an FORS signature
 * See FIPS 205 Section 8.3 Algorithm 16
 *
 * A FORS signature has a size of (k * (1 + a) * n) bytes
 * There are k trees, each of which have a private key value of size |n| followed
 * by an authentication path of size |a| (where each path is size |n|)
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param md A message digest of size |(k * a + 7) / 8| bytes to sign
 * @param sk_seed A private key seed of size |n|
 * @param pk_seed A public key seed of size |n|
 * @param adrs The ADRS object must have a layer address of zero, and the
 *             tree address set to the XMSS tree that signs the FORS key,
 *             the type set to FORS_TREE, and the keypair address set to the
 *             index of the WOTS+ key that signs the FORS key.
 * @param sig_wpkt A WPACKET object to write the generated XMSS signature to
 * @param sig_len  The size of |sig| which is (2 * n + 3) * n + tree_height * n.
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_fors_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *md,
                       const uint8_t *sk_seed, const uint8_t *pk_seed,
                       uint8_t *adrs, WPACKET *sig_wpkt)
{
    const SLH_DSA_KEY *key = ctx->key;
    uint32_t tree_id, layer, s, tree_offset;
    uint32_t ids[SLH_MAX_K];
    const SLH_DSA_PARAMS *params = key->params;
    uint32_t n = params->n;
    uint32_t k = params->k; /* number of trees */
    uint32_t a = params->a;
    uint32_t two_power_a = (1 << a); /* this is t in FIPS 205 */
    uint32_t tree_id_times_two_power_a = 0;
    uint8_t out[SLH_MAX_N];

    /*
     * Split md into k a-bit values e.g with k = 14, a = 12
     * ids[0..13] = 12 bits each of md
     */
    slh_base_2b(md, a, ids, k);

    for (tree_id = 0; tree_id < k; ++tree_id) {
        /* Get the tree[i] leaf id */
        uint32_t node_id = ids[tree_id]; /* |id| = |a| bits */

        /*
         * Give each of the k trees a unique range at each level.
         * e.g. If we have 4096 leaf nodes (2^a = 2^12) for each tree
         * tree i will use indexes from 4096 * i + (0..4095) for its bottom level.
         * For the next level up from the bottom there would be 2048 nodes
         * (so tree i uses indexes 2048 * i + (0...2047) for this level)
         */
        tree_offset = tree_id_times_two_power_a;

        if (!slh_fors_sk_gen(ctx, sk_seed, pk_seed, adrs,
                             node_id + tree_id_times_two_power_a, out, sizeof(out))
                || !WPACKET_memcpy(sig_wpkt, out, n))
            return 0;

        /*
         * Traverse from the bottom of the tree (layer = 0)
         * up to the root (layer = a - 1).
         * NOTE: This is a really inefficient way of doing this, since at
         * layer a - 1 it calculates most of the hashes of the entire tree as
         * well as all the leaf nodes. So it is calculating nodes multiple times.
         */
        for (layer = 0; layer < a; ++layer) {
            s = node_id ^ 1; /* XOR gets the index of the other child in a binary tree */
            if (!slh_fors_node(ctx, sk_seed, pk_seed, adrs,
                               s + tree_offset, layer, out, sizeof(out)))
                return 0;
            node_id >>= 1; /* Get the parent node id */
            tree_offset >>= 1; /* Each layer up has half as many nodes */
            if (!WPACKET_memcpy(sig_wpkt, out, n))
                return 0;
        }
        tree_id_times_two_power_a += two_power_a;
    }
    return 1;
}

/**
 * @brief Compute a candidate FORS public key from a message and signature.
 * See FIPS 205 Section 8.4 Algorithm 17.
 *
 * A FORS signature has a size of (k * (a + 1) * n) bytes
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param fors_sig_rpkt A PACKET object to read a FORS signature from
 * @param md A message digest of size (k * a / 8) bytes
 * @param pk_seed A public key seed of size |n|
 * @param adrs The ADRS object must have a layer address of zero, and the
 *             tree address set to the XMSS tree that signs the FORS key,
 *             the type set to FORS_TREE, and the keypair address set to the
 *             index of the WOTS+ key that signs the FORS key.
 * @param pk_out The returned candidate FORS public key of size |n|
 * @param pk_out_len The maximum size of |pk_out|
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_fors_pk_from_sig(SLH_DSA_HASH_CTX *ctx, PACKET *fors_sig_rpkt,
                              const uint8_t *md, const uint8_t *pk_seed,
                              uint8_t *adrs, uint8_t *pk_out, size_t pk_out_len)
{
    const SLH_DSA_KEY *key = ctx->key;
    int ret = 0;
    uint32_t i, j, aoff = 0;
    uint32_t ids[SLH_MAX_K];
    const SLH_DSA_PARAMS *params = key->params;
    uint32_t a = params->a;
    uint32_t k = params->k;
    uint32_t n = params->n;
    uint32_t two_power_a = (1 << a);
    const uint8_t *sk, *authj; /* Pointers to |sig| buffer inside fors_sig_rpkt */
    uint8_t roots[SLH_MAX_ROOTS];
    size_t roots_len = 0; /* The size of |roots| */
    uint8_t *node0, *node1; /* Pointers into roots[] */
    WPACKET root_pkt, *wroot_pkt = &root_pkt; /* Points to |roots| buffer */

    SLH_ADRS_DECLARE(pk_adrs);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_ADRS_FN_DECLARE(adrsf, set_tree_index);
    SLH_ADRS_FN_DECLARE(adrsf, set_tree_height);
    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_HASH_FN_DECLARE(hashf, F);
    SLH_HASH_FN_DECLARE(hashf, H);

    if (!WPACKET_init_static_len(wroot_pkt, roots, sizeof(roots), 0))
        return 0;

    /* Split md into k a-bit values e.g ids[0..k-1] = 12 bits each of md */
    slh_base_2b(md, a, ids, k);

    /* Compute the roots of k Merkle trees */
    for (i = 0; i < k; ++i) {
        uint32_t id = ids[i];
        uint32_t node_id = id + aoff;

        set_tree_height(adrs, 0);
        set_tree_index(adrs, node_id);

        /* Regenerate the public key of the leaf */
        if (!PACKET_get_bytes(fors_sig_rpkt, &sk, n)
                || !WPACKET_allocate_bytes(wroot_pkt, n, &node0)
                || !F(ctx, pk_seed, adrs, sk, n, node0, n))
            goto err;

        /* This omits the copying of the nodes that the FIPS 205 code does */
        node1 = node0;
        for (j = 0; j < a; ++j) {
            /* Get this layers other child public key */
            if (!PACKET_get_bytes(fors_sig_rpkt, &authj, n))
                goto err;
            /* Hash the children together to get the parent nodes public key */
            set_tree_height(adrs, j + 1);
            if ((id & 1) == 0) {
                node_id >>= 1;
                set_tree_index(adrs, node_id);
                if (!H(ctx, pk_seed, adrs, node0, authj, node1, n))
                    goto err;
            } else {
                node_id = (node_id - 1) >> 1;
                set_tree_index(adrs, node_id);
                if (!H(ctx, pk_seed, adrs, authj, node0, node1, n))
                    goto err;
            }
            id >>= 1;
        }
        aoff += two_power_a;
    }
    if (!WPACKET_get_total_written(wroot_pkt, &roots_len))
        goto err;

    /* The public key is the hash of all the roots of the k trees */
    adrsf->copy(pk_adrs, adrs);
    adrsf->set_type_and_clear(pk_adrs, SLH_ADRS_TYPE_FORS_ROOTS);
    adrsf->copy_keypair_address(pk_adrs, adrs);
    ret = hashf->T(ctx, pk_seed, pk_adrs, roots, roots_len, pk_out, pk_out_len);
 err:
    if (!WPACKET_finish(wroot_pkt))
        ret = 0;
    return ret;
}

/**
 * @brief Convert a byte string into a base 2^b representation
 * See FIPS 205 Algorithm 4
 *
 * @param in An input byte stream with a size >= |outlen * b / 8|
 * @param b The bit size to divide |in| into
 *          This is one of 6, 8, 9, 12 or 14 for FORS.
 * @param out The array of returned base 2^b integers that represents the first
 *            |outlen|*|b| bits of |in|
 * @param out_len The size of |out|
 */
static void slh_base_2b(const uint8_t *in, uint32_t b,
                        uint32_t *out, size_t out_len)
{
    size_t consumed = 0;
    uint32_t bits = 0;
    uint32_t total = 0;
    uint32_t mask = (1 << b) - 1;

    for (consumed = 0; consumed < out_len; consumed++) {
        while (bits < b) {
            total <<= 8;
            total += *in++;
            bits += 8;
        }
        bits -= b;
        *out++ = (total >> bits) & mask;
    }
}
