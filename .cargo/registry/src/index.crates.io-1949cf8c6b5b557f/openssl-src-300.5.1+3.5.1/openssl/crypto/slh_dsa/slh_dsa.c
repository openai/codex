/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stddef.h>
#include <string.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "slh_dsa_local.h"
#include "slh_dsa_key.h"

#define SLH_MAX_M 49 /* See slh_params.c */
/* The size of md is (21..40 bytes) - since a is in bits round up to nearest byte */
#define MD_LEN(params) (((params)->k * (params)->a + 7) >> 3)

static int get_tree_ids(PACKET *pkt, const SLH_DSA_PARAMS *params,
                        uint64_t *tree_id, uint32_t *leaf_id);

/**
 * @brief SLH-DSA Signature generation
 * See FIPS 205 Section 9.2 Algorithm 19
 *
 * A signature consists of
 *   r[n] random bytes
 *   [k]*[1+a][n] FORS signature bytes
 *   [h + d*len][n] Hyper tree signature bytes
 *
 * @param ctx Contains SLH_DSA algorithm functions and constants, and the
 *            private SLH_DSA key to use for signing.
 * @param msg The message to sign. This may be encoded beforehand.
 * @param msg_len The size of |msg|
 * @param sig The returned signature
 * @param sig_len The size of the returned |sig|
 * @param sig_size The maximum size of |sig|
 * @param opt_rand An optional random value to use of size |n|. It can be NULL.
 * @returns 1 if the signature generation succeeded or 0 otherwise.
 */
static int slh_sign_internal(SLH_DSA_HASH_CTX *hctx,
                             const uint8_t *msg, size_t msg_len,
                             uint8_t *sig, size_t *sig_len, size_t sig_size,
                             const uint8_t *opt_rand)
{
    int ret = 0;
    const SLH_DSA_KEY *priv = hctx->key;
    const SLH_DSA_PARAMS *params = priv->params;
    size_t sig_len_expected = params->sig_len;
    uint8_t m_digest[SLH_MAX_M];
    const uint8_t *md; /* The first md_len bytes of m_digest */
    size_t md_len = MD_LEN(params); /* The size of the digest |md| */
    /* Points to |m_digest| buffer, it is also reused to point to |sig_fors| */
    PACKET r_packet, *rpkt = &r_packet;
    uint8_t *r, *sig_fors; /* Pointers into buffer inside |wpkt| */
    WPACKET w_packet, *wpkt = &w_packet; /* Points to output |sig| buffer */
    const uint8_t *pk_seed, *sk_seed; /* pointers to elements within |priv| */
    uint8_t pk_fors[SLH_MAX_N];
    uint64_t tree_id;
    uint32_t leaf_id;

    SLH_ADRS_DECLARE(adrs);
    SLH_HASH_FUNC_DECLARE(priv, hashf);
    SLH_ADRS_FUNC_DECLARE(priv, adrsf);

    if (sig == NULL) {
        *sig_len = sig_len_expected;
        return 1;
    }

    if (sig_size < sig_len_expected) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_SIGNATURE_SIZE,
                       "is %zu, should be at least %zu", sig_size, sig_len_expected);
        return 0;
    }
    /* Exit if private key is not set */
    if (priv->has_priv == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    if (!WPACKET_init_static_len(wpkt, sig, sig_len_expected, 0))
        return 0;
    if (!PACKET_buf_init(rpkt, m_digest, params->m))
        return 0;

    pk_seed = SLH_DSA_PK_SEED(priv);
    sk_seed = SLH_DSA_SK_SEED(priv);

    if (opt_rand == NULL)
        opt_rand = pk_seed;

    adrsf->zero(adrs);
    /* calculate Randomness value r, and output to the SLH-DSA signature */
    r = WPACKET_get_curr(wpkt);
    if (!hashf->PRF_MSG(hctx, SLH_DSA_SK_PRF(priv), opt_rand, msg, msg_len, wpkt)
            /* generate a digest of size |params->m| bytes where m is (30..49) */
            || !hashf->H_MSG(hctx, r, pk_seed, SLH_DSA_PK_ROOT(priv), msg, msg_len,
                             m_digest, sizeof(m_digest))
            /* Grab the first md_len bytes of m_digest to use in fors_sign() */
            || !PACKET_get_bytes(rpkt, &md, md_len)
            /* Grab remaining bytes from m_digest to select tree and leaf id's */
            || !get_tree_ids(rpkt, params, &tree_id, &leaf_id))
        goto err;

    adrsf->set_tree_address(adrs, tree_id);
    adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_FORS_TREE);
    adrsf->set_keypair_address(adrs, leaf_id);

    sig_fors = WPACKET_get_curr(wpkt);
    /* generate the FORS signature and append it to the SLH-DSA signature */
    ret = ossl_slh_fors_sign(hctx, md, sk_seed, pk_seed, adrs, wpkt)
        /* Reuse rpkt to point to the FORS signature that was just generated */
        && PACKET_buf_init(rpkt, sig_fors, WPACKET_get_curr(wpkt) - sig_fors)
        /* Calculate the FORS public key using the generated FORS signature */
        && ossl_slh_fors_pk_from_sig(hctx, rpkt, md, pk_seed, adrs,
                                     pk_fors, sizeof(pk_fors))
        /* Generate ht signature and append to the SLH-DSA signature */
        && ossl_slh_ht_sign(hctx, pk_fors, sk_seed, pk_seed, tree_id, leaf_id,
                            wpkt);
    *sig_len = sig_len_expected;
    ret = 1;
 err:
    if (!WPACKET_finish(wpkt))
        ret = 0;
    return ret;
}

/**
 * @brief SLH-DSA Signature verification
 * See FIPS 205 Section 9.3 Algorithm 20
 *
 * A signature consists of
 *   r[n] random bytes
 *   [k]*[1+a][n] FORS signature bytes
 *   [h + d*len][n] Hyper tree signature bytes
 *
 * @param hctx Contains SLH_DSA algorithm functions and constants and the
 *             public SLH_DSA key to use for verification.
 * @param msg The message to verify. This may be encoded beforehand.
 * @param msg_len The size of |msg|
 * @param sig A signature to verify
 * @param sig_len The size of |sig|
 * @returns 1 if the signature verification succeeded or 0 otherwise.
 */
static int slh_verify_internal(SLH_DSA_HASH_CTX *hctx,
                               const uint8_t *msg, size_t msg_len,
                               const uint8_t *sig, size_t sig_len)
{
    const SLH_DSA_KEY *pub = hctx->key;
    SLH_HASH_FUNC_DECLARE(pub, hashf);
    SLH_ADRS_FUNC_DECLARE(pub, adrsf);
    SLH_ADRS_DECLARE(adrs);
    const SLH_DSA_PARAMS *params = pub->params;
    uint32_t n = params->n;
    const uint8_t *pk_seed, *pk_root; /* Pointers to elements in |pub| */
    PACKET pkt, *sig_rpkt = &pkt; /* Points to the |sig| buffer */
    uint8_t m_digest[SLH_MAX_M];
    const uint8_t *md; /* This is a pointer into the buffer in m_digest_rpkt */
    size_t md_len = MD_LEN(params); /* 21..40 bytes */
    PACKET pkt2, *m_digest_rpkt = &pkt2; /* Points to m_digest buffer */
    const uint8_t *r; /* Pointer to |sig_rpkt| buffer */
    uint8_t pk_fors[SLH_MAX_N];
    uint64_t tree_id;
    uint32_t leaf_id;

    /* Exit if public key is not set */
    if (pub->pub == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    /* Exit if signature is invalid size */
    if (sig_len != params->sig_len
            || !PACKET_buf_init(sig_rpkt, sig, sig_len))
        return 0;
    if (!PACKET_get_bytes(sig_rpkt, &r, n))
        return 0;

    adrsf->zero(adrs);

    pk_seed = SLH_DSA_PK_SEED(pub);
    pk_root = SLH_DSA_PK_ROOT(pub);

    if (!hashf->H_MSG(hctx, r, pk_seed, pk_root, msg, msg_len,
                      m_digest, sizeof(m_digest)))
        return 0;

    /*
     * Get md (the first md_len bytes of m_digest to use in
     * ossl_slh_fors_pk_from_sig(), and then retrieve the tree id and leaf id
     * from the remaining bytes in m_digest.
     */
    if (!PACKET_buf_init(m_digest_rpkt, m_digest, sizeof(m_digest))
            || !PACKET_get_bytes(m_digest_rpkt, &md, md_len)
            || !get_tree_ids(m_digest_rpkt, params, &tree_id, &leaf_id))
        return 0;

    adrsf->set_tree_address(adrs, tree_id);
    adrsf->set_type_and_clear(adrs, SLH_ADRS_TYPE_FORS_TREE);
    adrsf->set_keypair_address(adrs, leaf_id);
    return ossl_slh_fors_pk_from_sig(hctx, sig_rpkt, md, pk_seed, adrs,
                                     pk_fors, sizeof(pk_fors))
        && ossl_slh_ht_verify(hctx, pk_fors, sig_rpkt, pk_seed,
                              tree_id, leaf_id, pk_root)
        && PACKET_remaining(sig_rpkt) == 0;
}

/**
 * @brief Encode a message
 * See FIPS 205 Algorithm 22 Step 8 (and algorithm 24 Step 4).
 *
 * SLH_DSA pure signatures are encoded as M' = 00 || ctx_len || ctx || msg
 * Where ctx is the empty string by default and ctx_len <= 255.
 *
 * @param msg A message to encode
 * @param msg_len The size of |msg|
 * @param ctx An optional context to add to the message encoding.
 * @param ctx_len The size of |ctx|. It must be in the range 0..255
 * @param encode Use the Pure signature encoding if this is 1, and dont encode
 *               if this value is 0.
 * @param tmp A small buffer that may be used if the message is small.
 * @param tmp_len The size of |tmp|
 * @param out_len The size of the returned encoded buffer.
 * @returns A buffer containing the encoded message. If the passed in
 * |tmp| buffer is big enough to hold the encoded message then it returns |tmp|
 * otherwise it allocates memory which must be freed by the caller. If |encode|
 * is 0 then it returns |msg|. NULL is returned if there is a failure.
 */
static uint8_t *msg_encode(const uint8_t *msg, size_t msg_len,
                           const uint8_t *ctx, size_t ctx_len, int encode,
                           uint8_t *tmp, size_t tmp_len, size_t *out_len)
{
    uint8_t *encoded = NULL;
    size_t encoded_len;

    if (encode == 0) {
        /* Raw message */
        *out_len = msg_len;
        return (uint8_t *)msg;
    }
    if (ctx_len > SLH_DSA_MAX_CONTEXT_STRING_LEN)
        return NULL;

    /* Pure encoding */
    encoded_len = 1 + 1 + ctx_len + msg_len;
    *out_len = encoded_len;
    if (encoded_len <= tmp_len) {
        encoded = tmp;
    } else {
        encoded = OPENSSL_zalloc(encoded_len);
        if (encoded == NULL)
            return NULL;
    }
    encoded[0] = 0;
    encoded[1] = (uint8_t)ctx_len;
    memcpy(&encoded[2], ctx, ctx_len);
    memcpy(&encoded[2 + ctx_len], msg, msg_len);
    return encoded;
}

/**
 * See FIPS 205 Section 10.2.1 Algorithm 22
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_dsa_sign(SLH_DSA_HASH_CTX *slh_ctx,
                      const uint8_t *msg, size_t msg_len,
                      const uint8_t *ctx, size_t ctx_len,
                      const uint8_t *add_rand, int encode,
                      unsigned char *sig, size_t *siglen, size_t sigsize)
{
    uint8_t m_tmp[1024], *m = m_tmp;
    size_t m_len = 0;
    int ret = 0;

    if (sig != NULL) {
        m = msg_encode(msg, msg_len, ctx, ctx_len, encode, m_tmp, sizeof(m_tmp),
                       &m_len);
        if (m == NULL)
            return 0;
    }
    ret = slh_sign_internal(slh_ctx, m, m_len, sig, siglen, sigsize, add_rand);
    if (m != msg && m != m_tmp)
        OPENSSL_free(m);
    return ret;
}

/**
 * See FIPS 205 Section 10.3 Algorithm 24
 * @returns 1 on success, or 0 on error.
 */
int ossl_slh_dsa_verify(SLH_DSA_HASH_CTX *slh_ctx,
                        const uint8_t *msg, size_t msg_len,
                        const uint8_t *ctx, size_t ctx_len, int encode,
                        const uint8_t *sig, size_t sig_len)
{
    uint8_t *m;
    size_t m_len;
    uint8_t m_tmp[1024];
    int ret = 0;

    m = msg_encode(msg, msg_len, ctx, ctx_len, encode, m_tmp, sizeof(m_tmp),
                   &m_len);
    if (m == NULL)
        return 0;

    ret = slh_verify_internal(slh_ctx, m, m_len, sig, sig_len);
    if (m != msg && m != m_tmp)
        OPENSSL_free(m);
    return ret;
}

/*
 * See FIPS 205 Algorithm 2 toInt(X, n)
 * OPENSSL_load_u64_be() cant be used here as the |in_len| may be < 8
 */
static uint64_t bytes_to_u64_be(const uint8_t *in, size_t in_len)
{

    size_t i;
    uint64_t total = 0;

    for (i = 0; i < in_len; i++)
        total = (total << 8) + *in++;
    return total;
}

/*
 * See Algorithm 19 Steps 7..10 (also Algorithm 20 Step 10..13).
 * Converts digested bytes into a tree index, and leaf index within the tree.
 * The sizes are determined by the |params| parameter set.
 */
static int get_tree_ids(PACKET *rpkt, const SLH_DSA_PARAMS *params,
                        uint64_t *tree_id, uint32_t *leaf_id)
{
    const uint8_t *tree_id_bytes, *leaf_id_bytes;
    uint32_t tree_id_len, leaf_id_len;
    uint64_t tree_id_mask, leaf_id_mask;

    tree_id_len = ((params->h - params->hm + 7) >> 3); /* 7 or 8 bytes */
    leaf_id_len = ((params->hm + 7) >> 3); /* 1 or 2 bytes */

    if (!PACKET_get_bytes(rpkt, &tree_id_bytes, tree_id_len)
            || !PACKET_get_bytes(rpkt, &leaf_id_bytes, leaf_id_len))
        return 0;

    /*
     * In order to calculate A mod (2^X) where X is in the range of (54..64)
     * This is equivalent to A & (2^x - 1) which is just a sequence of X ones
     * that must fit into a 64 bit value.
     * e.g when X = 64 it would be A & (0xFFFF_FFFF_FFFF_FFFF)
     *     when X = 54 it would be A & (0x3F_FFFF_FFFF_FFFF)
     * i.e. A & (0xFFFF_FFFF_FFFF_FFFF >> (64 - X))
     */
    tree_id_mask = (~(uint64_t)0) >> (64 - (params->h - params->hm));
    leaf_id_mask = ((uint64_t)1 << params->hm) - 1; /* max value is 0x1FF when hm = 9 */
    *tree_id = bytes_to_u64_be(tree_id_bytes, tree_id_len) & tree_id_mask;
    *leaf_id = (uint32_t)(bytes_to_u64_be(leaf_id_bytes, leaf_id_len) & leaf_id_mask);
    return 1;
}
