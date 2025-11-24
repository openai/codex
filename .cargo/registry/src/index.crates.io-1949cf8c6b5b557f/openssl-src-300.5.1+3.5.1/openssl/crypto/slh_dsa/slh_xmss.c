/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"

/**
 * @brief Compute the root Public key of a XMSS tree.
 * See FIPS 205 Section 6.1 Algorithm 9.
 * This is a recursive function that starts at an leaf index, that calculates
 * the hash of each parent using 2 child nodes.
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param sk_seed A SLH-DSA private key seed of size |n|
 * @param nodeid The index of the target node being computed
 *               (which must be < 2^(hm - height)
 * @param h The height within the tree of the node being computed.
 *          (which must be <= hm) (hm is one of 3, 4, 8 or 9)
 *          At height=0 There are 2^hm leaf nodes,
 *          and the root node is at height = hm)
 * @param pk_seed A SLH-DSA public key seed of size |n|
 * @param adrs An ADRS object containing the layer address and tree address set
 *             to the XMSS tree within which the XMSS tree is being computed.
 * @param pk_out The generated public key of size |n|
 * @param pk_out_len The maximum size of |pk_out|
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_xmss_node(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_seed,
                       uint32_t node_id, uint32_t h,
                       const uint8_t *pk_seed, uint8_t *adrs,
                       uint8_t *pk_out, size_t pk_out_len)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_ADRS_FUNC_DECLARE(key, adrsf);

    if (h == 0) {
        /* For leaf nodes generate the public key */
        adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_WOTS_HASH);
        adrsf->set_keypair_address(adrs, node_id);
        if (!ossl_slh_wots_pk_gen(ctx, sk_seed, pk_seed, adrs,
                                  pk_out, pk_out_len))
            return 0;
    } else {
        uint8_t lnode[SLH_MAX_N], rnode[SLH_MAX_N];

        if (!ossl_slh_xmss_node(ctx, sk_seed, 2 * node_id, h - 1, pk_seed, adrs,
                                lnode, sizeof(lnode))
                || !ossl_slh_xmss_node(ctx, sk_seed, 2 * node_id + 1, h - 1,
                                       pk_seed, adrs, rnode, sizeof(rnode)))
            return 0;
        adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_TREE);
        adrsf->set_tree_height(adrs, h);
        adrsf->set_tree_index(adrs, node_id);
        if (!key->hash_func->H(ctx, pk_seed, adrs, lnode, rnode, pk_out, pk_out_len))
            return 0;
    }
    return 1;
}

/**
 * @brief Generate an XMSS signature using a message and key.
 * See FIPS 205 Section 6.2 Algorithm 10
 *
 * The generated signature consists of:
 *  - A WOTS+ signature of size (2 * n + 3) * n
 *  - An array of authentication paths of size (XMSS tree_height) * n.
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param msg A message of size |n| bytes to sign
 * @param sk_seed A private key seed of size |n|
 * @param node_id The index of a WOTS+ key within the XMSS tree to use for signing.
 * @param pk_seed A public key seed f size |n|
 * @param adrs An ADRS object containing the layer address and tree address set
 *              to the XMSS key being used to sign the message.
 * @param sig_wpkt A WPACKET object to write the generated XMSS signature to.
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_xmss_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                       const uint8_t *sk_seed, uint32_t node_id,
                       const uint8_t *pk_seed, uint8_t *adrs, WPACKET *sig_wpkt)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_ADRS_DECLARE(tmp_adrs);
    size_t n = key->params->n;
    uint32_t h, hm = key->params->hm;
    uint32_t id = node_id;
    uint8_t *auth_path; /* Pointer to a buffer offset inside |sig_wpkt| */
    size_t auth_path_len = n;

    /*
     * This code reverses the order of the FIPS 205 code so that it does the
     * sign first. This simplifies the WPACKET writing.
     */
    adrsf->copy(tmp_adrs, adrs);
    adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_WOTS_HASH);
    adrsf->set_keypair_address(adrs, node_id);
    if (!ossl_slh_wots_sign(ctx, msg, sk_seed, pk_seed, adrs, sig_wpkt))
        return 0;

    adrsf->copy(adrs, tmp_adrs);
    for (h = 0; h < hm; ++h) {
        if (!WPACKET_allocate_bytes(sig_wpkt, auth_path_len, &auth_path)
                || !ossl_slh_xmss_node(ctx, sk_seed, id ^ 1, h, pk_seed, adrs,
                                       auth_path, auth_path_len))
            return 0;
        id >>= 1;
    }
    return 1;
}

/**
 * @brief Compute a candidate XMSS public key from a message and XMSS signature
 * See FIPS 205 Section 6.3 Algorithm 11
 *
 * * The signature consists of:
 *  - A WOTS+ signature of size (2 * n + 3) * n
 *  - An array of authentication paths of size (XMSS tree height) * n.
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants.
 * @param node_id Must be set to the |node_id| used in xmss_sign().
 * @param sig_rpkt A Packet to read a XMSS signature from.
 * @param msg A message of size |n| bytes
 * @param sk_seed A private key seed of size |n|
 * @param pk_seed A public key seed of size |n|
 * @param adrs An ADRS object containing a layer address and tree address of an
 *             XMSS key used for signing the message.
 * @param pk_out The returned candidate XMSS public key of size |n|.
 * @param pk_out_len The maximum size of |pk_out|.
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_xmss_pk_from_sig(SLH_DSA_HASH_CTX *ctx, uint32_t node_id,
                              PACKET *sig_rpkt, const uint8_t *msg,
                              const uint8_t *pk_seed, uint8_t *adrs,
                              uint8_t *pk_out, size_t pk_out_len)
{
    const SLH_DSA_KEY *key = ctx->key;
    SLH_HASH_FUNC_DECLARE(key, hashf);
    SLH_ADRS_FUNC_DECLARE(key, adrsf);
    SLH_HASH_FN_DECLARE(hashf, H);
    SLH_ADRS_FN_DECLARE(adrsf, set_tree_index);
    SLH_ADRS_FN_DECLARE(adrsf, set_tree_height);
    uint32_t k;
    size_t n = key->params->n;
    uint32_t hm = key->params->hm;
    uint8_t *node = pk_out;
    const uint8_t *auth_path; /* Pointer to buffer offset in |pkt_sig| */

    adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_WOTS_HASH);
    adrsf->set_keypair_address(adrs, node_id);
    if (!ossl_slh_wots_pk_from_sig(ctx, sig_rpkt, msg, pk_seed, adrs,
                                   node, pk_out_len))
        return 0;

    adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_TREE);

    for (k = 0; k < hm; ++k) {
        if (!PACKET_get_bytes(sig_rpkt, &auth_path, n))
            return 0;
        set_tree_height(adrs, k + 1);
        if ((node_id & 1) == 0) { /* even */
            node_id >>= 1;
            set_tree_index(adrs, node_id);
            if (!H(ctx, pk_seed, adrs, node, auth_path, node, pk_out_len))
                return 0;
        } else { /* odd */
            node_id = (node_id - 1) >> 1;
            set_tree_index(adrs, node_id);
            if (!H(ctx, pk_seed, adrs, auth_path, node, node, pk_out_len))
                return 0;
        }
    }
    return 1;
}
