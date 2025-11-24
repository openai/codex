/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_RECORD_SHARED_H
# define OSSL_QUIC_RECORD_SHARED_H

# include <openssl/ssl.h>
# include "internal/quic_types.h"
# include "internal/quic_wire_pkt.h"

/*
 * QUIC Record Layer EL Management Utilities
 * =========================================
 *
 * This defines a structure for managing the cryptographic state at a given
 * encryption level, as this functionality is shared between QRX and QTX. For
 * QRL use only.
 */

/*
 * States an EL can be in. The Updating and Cooldown states are used by RX only;
 * a TX EL in the Provisioned state is always in the Normal substate.
 *
 * Key material is available if in the Provisioned state.
 */
#define QRL_EL_STATE_UNPROV         0   /* Unprovisioned (initial state) */
#define QRL_EL_STATE_PROV_NORMAL    1   /* Provisioned - Normal */
#define QRL_EL_STATE_PROV_UPDATING  2   /* Provisioned - Updating */
#define QRL_EL_STATE_PROV_COOLDOWN  3   /* Provisioned - Cooldown */
#define QRL_EL_STATE_DISCARDED      4   /* Discarded (terminal state) */

typedef struct ossl_qrl_enc_level_st {
    /*
     * Cryptographic context used to apply and remove header protection from
     * packet headers.
     */
    QUIC_HDR_PROTECTOR          hpr;

    /* Hash function used for key derivation. */
    EVP_MD                     *md;

    /* Context used for packet body ciphering. One for each keyslot. */
    EVP_CIPHER_CTX             *cctx[2];

    OSSL_LIB_CTX               *libctx;
    const char                 *propq;

    /*
     * Key epoch, essentially the number of times we have done a key update.
     *
     * The least significant bit of this is therefore by definition the current
     * Key Phase bit value.
     */
    uint64_t                    key_epoch;

    /* Usage counter. The caller maintains this. Used by TX side only. */
    uint64_t                    op_count;

    /* QRL_SUITE_* value. */
    uint32_t                    suite_id;

    /* Length of authentication tag. */
    uint32_t                    tag_len;

    /* Current EL state. */
    unsigned char               state; /* QRL_EL_STATE_* */

    /* 1 if for TX, else RX. Initialised when secret provided. */
    unsigned char               is_tx;

    /* IV used to construct nonces used for AEAD packet body ciphering. */
    unsigned char               iv[2][EVP_MAX_IV_LENGTH];

    /*
     * Secret for next key epoch.
     */
    unsigned char               ku[EVP_MAX_KEY_LENGTH];
} OSSL_QRL_ENC_LEVEL;

typedef struct ossl_qrl_enc_level_set_st {
    OSSL_QRL_ENC_LEVEL el[QUIC_ENC_LEVEL_NUM];
} OSSL_QRL_ENC_LEVEL_SET;

/*
 * Returns 1 if we have key material for a given encryption level (that is, if
 * we are in the PROVISIONED state), 0 if we do not yet have material (we are in
 * the UNPROVISIONED state) and -1 if the EL is discarded (we are in the
 * DISCARDED state).
 */
int ossl_qrl_enc_level_set_have_el(OSSL_QRL_ENC_LEVEL_SET *els,
                                   uint32_t enc_level);

/*
 * Returns EL in a set. If enc_level is not a valid QUIC_ENC_LEVEL_* value,
 * returns NULL. If require_prov is 1, returns NULL if the EL is not in
 * the PROVISIONED state; otherwise, the returned EL may be in any state.
 */
OSSL_QRL_ENC_LEVEL *ossl_qrl_enc_level_set_get(OSSL_QRL_ENC_LEVEL_SET *els,
                                               uint32_t enc_level,
                                               int require_prov);

/* Provide secret to an EL. md may be NULL. */
int ossl_qrl_enc_level_set_provide_secret(OSSL_QRL_ENC_LEVEL_SET *els,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq,
                                          uint32_t enc_level,
                                          uint32_t suite_id,
                                          EVP_MD *md,
                                          const unsigned char *secret,
                                          size_t secret_len,
                                          unsigned char init_key_phase_bit,
                                          int is_tx);

/*
 * Returns 1 if the given keyslot index is currently valid for a given EL and EL
 * state.
 */
int ossl_qrl_enc_level_set_has_keyslot(OSSL_QRL_ENC_LEVEL_SET *els,
                                       uint32_t enc_level,
                                       unsigned char tgt_state,
                                       size_t keyslot);

/* Perform a key update. Transitions from PROV_NORMAL to PROV_UPDATING. */
int ossl_qrl_enc_level_set_key_update(OSSL_QRL_ENC_LEVEL_SET *els,
                                      uint32_t enc_level);

/* Transitions from PROV_UPDATING to PROV_COOLDOWN. */
int ossl_qrl_enc_level_set_key_update_done(OSSL_QRL_ENC_LEVEL_SET *els,
                                           uint32_t enc_level);

/*
 * Transitions from PROV_COOLDOWN to PROV_NORMAL. (If in PROV_UPDATING,
 * auto-transitions to PROV_COOLDOWN first.)
 */
int ossl_qrl_enc_level_set_key_cooldown_done(OSSL_QRL_ENC_LEVEL_SET *els,
                                             uint32_t enc_level);

/*
 * Discard an EL. No secret can be provided for the EL ever again.
 */
void ossl_qrl_enc_level_set_discard(OSSL_QRL_ENC_LEVEL_SET *els,
                                    uint32_t enc_level);

#endif
