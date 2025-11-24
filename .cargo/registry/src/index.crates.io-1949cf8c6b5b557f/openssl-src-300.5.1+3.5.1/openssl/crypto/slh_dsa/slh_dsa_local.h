/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/slh_dsa.h"
#include "slh_hash.h"
#include "slh_params.h"

/*
 * Maximum size of the security parameter |n| in FIPS 205 Section 11. Table 2.
 * This indicates the length in bytes of a message that can be signed.
 * It is the size used by WOTS+ public and private key elements as well as
 * signature elements.
 */
#define SLH_MAX_N 32
/*
 * For the given standard w=16 for all parameter sets.
 * A n byte message is converted into 2 * n base 16 Integers followed
 * by 3 Integers for the checksum of these values.
 */
#define SLH_WOTS_LEN(n) (2 * (n) + 3)

/*
 * FIPS 205 SLH-DSA algorithms have many different parameters which includes
 * the following constants that are stored into a |key|:
 *   - A set of constants (Section 11. contains 12 parameter sets)
 *     such as tree heights and security parameters associated with a algorithm
 *     name such as SLH-DSA-SHA2-128s.
 *   - ADRS functions (such as set_layer_address() in Section 4.3 & 11.2)
 *   - Hash Functions (such as H_MSG() & PRF()) See Sections 11.1, 11.2.1 & 11.2.2.
 *   - prefetched EVP_MD objects used for hashing.
 *
 * When performing operations multiple Hash related objects are also needed
 * such as EVP_MD_CTX and EVP_MAC_CTX (these are independent of the |key|)
 *
 * SLH_DSA_HASH_CTX is a container to hold all of these objects. This object is
 * resolved early and is then passed to most SLH_DSA related functions, since
 * there are many nested layers of calls that require these values.
 *
 * NOTE: Any changes to this structure will need updating in
 * ossl_slh_dsa_hash_ctx_dup().
 */
struct slh_dsa_hash_ctx_st {
    const SLH_DSA_KEY *key; /* This key is not owned by this object */
    EVP_MD_CTX *md_ctx;     /* Either SHAKE OR SHA-256 */
    EVP_MD_CTX *md_big_ctx; /* Either SHA-512 or points to |md_ctx| for SHA-256*/
    EVP_MAC_CTX *hmac_ctx;  /* required by SHA algorithms for PRFmsg() */
    int hmac_digest_used;   /* Used for lazy init of hmac_ctx digest */
};

__owur int ossl_slh_wots_pk_gen(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_seed,
                                const uint8_t *pk_seed, uint8_t *adrs,
                                uint8_t *pk_out, size_t pk_out_len);
__owur int ossl_slh_wots_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                              const uint8_t *sk_seed, const uint8_t *pk_seed,
                              uint8_t *adrs, WPACKET *sig_wpkt);
__owur int ossl_slh_wots_pk_from_sig(SLH_DSA_HASH_CTX *ctx,
                                     PACKET *sig_rpkt, const uint8_t *msg,
                                     const uint8_t *pk_seed, uint8_t *adrs,
                                     uint8_t *pk_out, size_t pk_out_len);

__owur int ossl_slh_xmss_node(SLH_DSA_HASH_CTX *ctx, const uint8_t *sk_seed,
                              uint32_t node_id, uint32_t height,
                              const uint8_t *pk_seed, uint8_t *adrs,
                              uint8_t *pk_out, size_t pk_out_len);
__owur int ossl_slh_xmss_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                              const uint8_t *sk_seed, uint32_t node_id,
                              const uint8_t *pk_seed, uint8_t *adrs,
                              WPACKET *sig_wpkt);
__owur int ossl_slh_xmss_pk_from_sig(SLH_DSA_HASH_CTX *ctx, uint32_t node_id,
                                     PACKET *sig_rpkt, const uint8_t *msg,
                                     const uint8_t *pk_seed, uint8_t *adrs,
                                     uint8_t *pk_out, size_t pk_out_len);

__owur int ossl_slh_ht_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                            const uint8_t *sk_seed, const uint8_t *pk_seed,
                            uint64_t tree_id, uint32_t leaf_id,
                            WPACKET *sig_wpkt);
__owur int ossl_slh_ht_verify(SLH_DSA_HASH_CTX *ctx, const uint8_t *msg,
                              PACKET *sig_rpkt, const uint8_t *pk_seed,
                              uint64_t tree_id, uint32_t leaf_id,
                              const uint8_t *pk_root);

__owur int ossl_slh_fors_sign(SLH_DSA_HASH_CTX *ctx, const uint8_t *md,
                              const uint8_t *sk_seed, const uint8_t *pk_seed,
                              uint8_t *adrs, WPACKET *sig_wpkt);
__owur int ossl_slh_fors_pk_from_sig(SLH_DSA_HASH_CTX *ctx, PACKET *sig_rpkt,
                                     const uint8_t *md, const uint8_t *pk_seed,
                                     uint8_t *adrs,
                                     uint8_t *pk_out, size_t pk_out_len);
