#include "quic_record_shared.h"
#include "internal/quic_record_util.h"
#include "internal/common.h"
#include "../ssl_local.h"

/* Constants used for key derivation in QUIC v1. */
static const unsigned char quic_v1_iv_label[] = {
    0x71, 0x75, 0x69, 0x63, 0x20, 0x69, 0x76 /* "quic iv" */
};
static const unsigned char quic_v1_key_label[] = {
    0x71, 0x75, 0x69, 0x63, 0x20, 0x6b, 0x65, 0x79 /* "quic key" */
};
static const unsigned char quic_v1_hp_label[] = {
    0x71, 0x75, 0x69, 0x63, 0x20, 0x68, 0x70 /* "quic hp" */
};
static const unsigned char quic_v1_ku_label[] = {
    0x71, 0x75, 0x69, 0x63, 0x20, 0x6b, 0x75 /* "quic ku" */
};

OSSL_QRL_ENC_LEVEL *ossl_qrl_enc_level_set_get(OSSL_QRL_ENC_LEVEL_SET *els,
                                               uint32_t enc_level,
                                               int require_prov)
{
    OSSL_QRL_ENC_LEVEL *el;

    if (!ossl_assert(enc_level < QUIC_ENC_LEVEL_NUM))
        return NULL;

    el = &els->el[enc_level];

    if (require_prov)
        switch (el->state) {
            case QRL_EL_STATE_PROV_NORMAL:
            case QRL_EL_STATE_PROV_UPDATING:
            case QRL_EL_STATE_PROV_COOLDOWN:
                break;
            default:
                return NULL;
        }

    return el;
}

int ossl_qrl_enc_level_set_have_el(OSSL_QRL_ENC_LEVEL_SET *els,
                                  uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);

    switch (el->state) {
        case QRL_EL_STATE_UNPROV:
            return 0;
        case QRL_EL_STATE_PROV_NORMAL:
        case QRL_EL_STATE_PROV_UPDATING:
        case QRL_EL_STATE_PROV_COOLDOWN:
            return 1;
        default:
        case QRL_EL_STATE_DISCARDED:
            return -1;
    }
}

int ossl_qrl_enc_level_set_has_keyslot(OSSL_QRL_ENC_LEVEL_SET *els,
                                       uint32_t enc_level,
                                       unsigned char tgt_state,
                                       size_t keyslot)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);

    if (!ossl_assert(el != NULL && keyslot < 2))
        return 0;

    switch (tgt_state) {
        case QRL_EL_STATE_PROV_NORMAL:
        case QRL_EL_STATE_PROV_UPDATING:
            return enc_level == QUIC_ENC_LEVEL_1RTT || keyslot == 0;
        case QRL_EL_STATE_PROV_COOLDOWN:
            assert(enc_level == QUIC_ENC_LEVEL_1RTT);
            return keyslot == (el->key_epoch & 1);
        default:
            return 0;
    }
}

static void el_teardown_keyslot(OSSL_QRL_ENC_LEVEL_SET *els,
                                uint32_t enc_level,
                                size_t keyslot)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);

    if (!ossl_qrl_enc_level_set_has_keyslot(els, enc_level, el->state, keyslot))
        return;

    if (el->cctx[keyslot] != NULL) {
        EVP_CIPHER_CTX_free(el->cctx[keyslot]);
        el->cctx[keyslot] = NULL;
    }

    OPENSSL_cleanse(el->iv[keyslot], sizeof(el->iv[keyslot]));
}

static int el_setup_keyslot(OSSL_QRL_ENC_LEVEL_SET *els,
                            uint32_t enc_level,
                            unsigned char tgt_state,
                            size_t keyslot,
                            const unsigned char *secret,
                            size_t secret_len)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);
    unsigned char key[EVP_MAX_KEY_LENGTH];
    size_t key_len = 0, iv_len = 0;
    const char *cipher_name = NULL;
    EVP_CIPHER *cipher = NULL;
    EVP_CIPHER_CTX *cctx = NULL;

    if (!ossl_assert(el != NULL
                     && ossl_qrl_enc_level_set_has_keyslot(els, enc_level,
                                                           tgt_state, keyslot))) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    cipher_name = ossl_qrl_get_suite_cipher_name(el->suite_id);
    iv_len      = ossl_qrl_get_suite_cipher_iv_len(el->suite_id);
    key_len     = ossl_qrl_get_suite_cipher_key_len(el->suite_id);
    if (cipher_name == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (secret_len != ossl_qrl_get_suite_secret_len(el->suite_id)
        || secret_len > EVP_MAX_KEY_LENGTH) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    assert(el->cctx[keyslot] == NULL);

    /* Derive "quic iv" key. */
    if (!tls13_hkdf_expand_ex(el->libctx, el->propq,
                              el->md,
                              secret,
                              quic_v1_iv_label,
                              sizeof(quic_v1_iv_label),
                              NULL, 0,
                              el->iv[keyslot], iv_len, 1))
        goto err;

    /* Derive "quic key" key. */
    if (!tls13_hkdf_expand_ex(el->libctx, el->propq,
                              el->md,
                              secret,
                              quic_v1_key_label,
                              sizeof(quic_v1_key_label),
                              NULL, 0,
                              key, key_len, 1))
        goto err;

    /* Create and initialise cipher context. */
    if ((cipher = EVP_CIPHER_fetch(el->libctx, cipher_name, el->propq)) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if ((cctx = EVP_CIPHER_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    if (!ossl_assert(iv_len == (size_t)EVP_CIPHER_get_iv_length(cipher))
        || !ossl_assert(key_len == (size_t)EVP_CIPHER_get_key_length(cipher))) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        goto err;
    }

    /* IV will be changed on RX/TX so we don't need to use a real value here. */
    if (!EVP_CipherInit_ex(cctx, cipher, NULL, key, el->iv[keyslot], 0)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        goto err;
    }

    el->cctx[keyslot] = cctx;

    /* Zeroize intermediate keys. */
    OPENSSL_cleanse(key, sizeof(key));
    EVP_CIPHER_free(cipher);
    return 1;

 err:
    EVP_CIPHER_CTX_free(cctx);
    EVP_CIPHER_free(cipher);
    OPENSSL_cleanse(el->iv[keyslot], sizeof(el->iv[keyslot]));
    OPENSSL_cleanse(key, sizeof(key));
    return 0;
}

int ossl_qrl_enc_level_set_provide_secret(OSSL_QRL_ENC_LEVEL_SET *els,
                                          OSSL_LIB_CTX *libctx,
                                          const char *propq,
                                          uint32_t enc_level,
                                          uint32_t suite_id,
                                          EVP_MD *md,
                                          const unsigned char *secret,
                                          size_t secret_len,
                                          unsigned char init_key_phase_bit,
                                          int is_tx)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);
    unsigned char ku_key[EVP_MAX_KEY_LENGTH], hpr_key[EVP_MAX_KEY_LENGTH];
    int have_ks0 = 0, have_ks1 = 0, own_md = 0;
    const char *md_name = ossl_qrl_get_suite_md_name(suite_id);
    size_t hpr_key_len, init_keyslot;

    if (el == NULL
        || md_name == NULL
        || init_key_phase_bit > 1 || is_tx < 0 || is_tx > 1
        || (init_key_phase_bit > 0 && enc_level != QUIC_ENC_LEVEL_1RTT)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (enc_level == QUIC_ENC_LEVEL_INITIAL
        && el->state == QRL_EL_STATE_PROV_NORMAL) {
        /*
         * Sometimes the INITIAL EL needs to be reprovisioned, namely if a
         * connection retry occurs. Exceptionally, if the caller wants to
         * reprovision the INITIAL EL, tear it down as usual and then override
         * the state so it can be provisioned again.
         */
        ossl_qrl_enc_level_set_discard(els, enc_level);
        el->state = QRL_EL_STATE_UNPROV;
    }

    if (el->state != QRL_EL_STATE_UNPROV) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    init_keyslot = is_tx ? 0 : init_key_phase_bit;
    hpr_key_len = ossl_qrl_get_suite_hdr_prot_key_len(suite_id);
    if (hpr_key_len == 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (md == NULL) {
        md = EVP_MD_fetch(libctx, md_name, propq);
        if (md == NULL) {
            ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
            return 0;
        }

        own_md = 1;
    }

    el->libctx      = libctx;
    el->propq       = propq;
    el->md          = md;
    el->suite_id    = suite_id;
    el->tag_len     = ossl_qrl_get_suite_cipher_tag_len(suite_id);
    el->op_count    = 0;
    el->key_epoch   = (uint64_t)init_key_phase_bit;
    el->is_tx       = (unsigned char)is_tx;

    /* Derive "quic hp" key. */
    if (!tls13_hkdf_expand_ex(libctx, propq,
                              md,
                              secret,
                              quic_v1_hp_label,
                              sizeof(quic_v1_hp_label),
                              NULL, 0,
                              hpr_key, hpr_key_len, 1))
        goto err;

    /* Setup KS0 (or KS1 if init_key_phase_bit), our initial keyslot. */
    if (!el_setup_keyslot(els, enc_level, QRL_EL_STATE_PROV_NORMAL,
                          init_keyslot, secret, secret_len))
        goto err;

    have_ks0 = 1;

    if (enc_level == QUIC_ENC_LEVEL_1RTT) {
        /* Derive "quic ku" key (the epoch 1 secret). */
        if (!tls13_hkdf_expand_ex(libctx, propq,
                                  md,
                                  secret,
                                  quic_v1_ku_label,
                                  sizeof(quic_v1_ku_label),
                                  NULL, 0,
                                  is_tx ? el->ku : ku_key, secret_len, 1))
            goto err;

        if (!is_tx) {
            /* Setup KS1 (or KS0 if init_key_phase_bit), our next keyslot. */
            if (!el_setup_keyslot(els, enc_level, QRL_EL_STATE_PROV_NORMAL,
                                  !init_keyslot, ku_key, secret_len))
                goto err;

            have_ks1 = 1;

            /* Derive NEXT "quic ku" key (the epoch 2 secret). */
            if (!tls13_hkdf_expand_ex(libctx, propq,
                                      md,
                                      ku_key,
                                      quic_v1_ku_label,
                                      sizeof(quic_v1_ku_label),
                                      NULL, 0,
                                      el->ku, secret_len, 1))
                goto err;
        }
    }

    /* Setup header protection context. */
    if (!ossl_quic_hdr_protector_init(&el->hpr,
                                      libctx, propq,
                                      ossl_qrl_get_suite_hdr_prot_cipher_id(suite_id),
                                      hpr_key, hpr_key_len))
        goto err;

    /*
     * We are now provisioned: KS0 has our current key (for key epoch 0), KS1
     * has our next key (for key epoch 1, in the case of the 1-RTT EL only), and
     * el->ku has the secret which will be used to generate keys for key epoch
     * 2.
     */
    OPENSSL_cleanse(hpr_key, sizeof(hpr_key));
    OPENSSL_cleanse(ku_key, sizeof(ku_key));
    el->state = QRL_EL_STATE_PROV_NORMAL;
    return 1;

 err:
    el->suite_id = 0;
    el->md = NULL;
    OPENSSL_cleanse(hpr_key, sizeof(hpr_key));
    OPENSSL_cleanse(ku_key, sizeof(ku_key));
    OPENSSL_cleanse(el->ku, sizeof(el->ku));
    if (have_ks0)
        el_teardown_keyslot(els, enc_level, init_keyslot);
    if (have_ks1)
        el_teardown_keyslot(els, enc_level, !init_keyslot);
    if (own_md)
        EVP_MD_free(md);
    return 0;
}

int ossl_qrl_enc_level_set_key_update(OSSL_QRL_ENC_LEVEL_SET *els,
                                      uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);
    size_t secret_len;
    unsigned char new_ku[EVP_MAX_KEY_LENGTH];

    if (el == NULL || !ossl_assert(enc_level == QUIC_ENC_LEVEL_1RTT)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (el->state != QRL_EL_STATE_PROV_NORMAL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (!el->is_tx) {
        /*
         * We already have the key for the next epoch, so just move to using it.
         */
        ++el->key_epoch;
        el->state = QRL_EL_STATE_PROV_UPDATING;
        return 1;
    }

    /*
     * TX case. For the TX side we use only keyslot 0; it replaces the old key
     * immediately.
     */
    secret_len = ossl_qrl_get_suite_secret_len(el->suite_id);

    /* Derive NEXT "quic ku" key (the epoch n+1 secret). */
    if (!tls13_hkdf_expand_ex(el->libctx, el->propq,
                              el->md, el->ku,
                              quic_v1_ku_label,
                              sizeof(quic_v1_ku_label),
                              NULL, 0,
                              new_ku, secret_len, 1))
        return 0;

    el_teardown_keyslot(els, enc_level, 0);

    /* Setup keyslot for CURRENT "quic ku" key. */
    if (!el_setup_keyslot(els, enc_level, QRL_EL_STATE_PROV_NORMAL,
                          0, el->ku, secret_len))
        return 0;

    ++el->key_epoch;
    el->op_count = 0;
    memcpy(el->ku, new_ku, secret_len);
    /* Remain in PROV_NORMAL state */
    return 1;
}

/* Transitions from PROV_UPDATING to PROV_COOLDOWN. */
int ossl_qrl_enc_level_set_key_update_done(OSSL_QRL_ENC_LEVEL_SET *els,
                                           uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);

    if (el == NULL || !ossl_assert(enc_level == QUIC_ENC_LEVEL_1RTT)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    /* No new key yet, but erase key material to aid PFS. */
    el_teardown_keyslot(els, enc_level, ~el->key_epoch & 1);
    el->state = QRL_EL_STATE_PROV_COOLDOWN;
    return 1;
}

/*
 * Transitions from PROV_COOLDOWN to PROV_NORMAL. (If in PROV_UPDATING,
 * auto-transitions to PROV_COOLDOWN first.)
 */
int ossl_qrl_enc_level_set_key_cooldown_done(OSSL_QRL_ENC_LEVEL_SET *els,
                                             uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);
    size_t secret_len;
    unsigned char new_ku[EVP_MAX_KEY_LENGTH];

    if (el == NULL || !ossl_assert(enc_level == QUIC_ENC_LEVEL_1RTT)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (el->state == QRL_EL_STATE_PROV_UPDATING
        && !ossl_qrl_enc_level_set_key_update_done(els, enc_level)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    if (el->state != QRL_EL_STATE_PROV_COOLDOWN) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    secret_len = ossl_qrl_get_suite_secret_len(el->suite_id);

    if (!el_setup_keyslot(els, enc_level, QRL_EL_STATE_PROV_NORMAL,
                          ~el->key_epoch & 1, el->ku, secret_len))
        return 0;

    /* Derive NEXT "quic ku" key (the epoch n+1 secret). */
    if (!tls13_hkdf_expand_ex(el->libctx, el->propq,
                              el->md,
                              el->ku,
                              quic_v1_ku_label,
                              sizeof(quic_v1_ku_label),
                              NULL, 0,
                              new_ku, secret_len, 1)) {
        el_teardown_keyslot(els, enc_level, ~el->key_epoch & 1);
        return 0;
    }

    memcpy(el->ku, new_ku, secret_len);
    el->state = QRL_EL_STATE_PROV_NORMAL;
    return 1;
}

/*
 * Discards keying material for a given encryption level. Transitions from any
 * state to DISCARDED.
 */
void ossl_qrl_enc_level_set_discard(OSSL_QRL_ENC_LEVEL_SET *els,
                                    uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(els, enc_level, 0);

    if (el == NULL || el->state == QRL_EL_STATE_DISCARDED)
        return;

    if (ossl_qrl_enc_level_set_have_el(els, enc_level) == 1) {
        ossl_quic_hdr_protector_cleanup(&el->hpr);

        el_teardown_keyslot(els, enc_level, 0);
        el_teardown_keyslot(els, enc_level, 1);
    }

    EVP_MD_free(el->md);
    el->md      = NULL;
    el->state   = QRL_EL_STATE_DISCARDED;
}
