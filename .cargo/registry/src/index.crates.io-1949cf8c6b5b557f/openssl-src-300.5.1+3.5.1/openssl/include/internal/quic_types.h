/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_TYPES_H
# define OSSL_QUIC_TYPES_H

# include <openssl/ssl.h>
# include <internal/ssl.h>
# include <assert.h>
# include <string.h>

# ifndef OPENSSL_NO_QUIC

/* QUIC encryption levels. */
enum {
    QUIC_ENC_LEVEL_INITIAL = 0,
    QUIC_ENC_LEVEL_0RTT,
    QUIC_ENC_LEVEL_HANDSHAKE,
    QUIC_ENC_LEVEL_1RTT,
    QUIC_ENC_LEVEL_NUM       /* Must be the ultimate entry */
};

/* QUIC packet number spaces. */
enum {
    QUIC_PN_SPACE_INITIAL = 0,
    QUIC_PN_SPACE_HANDSHAKE,
    /* New entries must go here, so that QUIC_PN_SPACE_APP is the penultimate */
    QUIC_PN_SPACE_APP,
    QUIC_PN_SPACE_NUM       /* Must be the ultimate entry */
};

static ossl_unused ossl_inline uint32_t
ossl_quic_enc_level_to_pn_space(uint32_t enc_level)
{
    switch (enc_level) {
    case QUIC_ENC_LEVEL_INITIAL:
        return QUIC_PN_SPACE_INITIAL;
    case QUIC_ENC_LEVEL_HANDSHAKE:
        return QUIC_PN_SPACE_HANDSHAKE;
    case QUIC_ENC_LEVEL_0RTT:
    case QUIC_ENC_LEVEL_1RTT:
        return QUIC_PN_SPACE_APP;
    default:
        assert(0);
        return QUIC_PN_SPACE_APP;
    }
}

/* QUIC packet number representation. */
typedef uint64_t QUIC_PN;
#  define QUIC_PN_INVALID            UINT64_MAX

static ossl_unused ossl_inline QUIC_PN ossl_quic_pn_max(QUIC_PN a, QUIC_PN b)
{
    return a > b ? a : b;
}

static ossl_unused ossl_inline QUIC_PN ossl_quic_pn_min(QUIC_PN a, QUIC_PN b)
{
    return a < b ? a : b;
}

static ossl_unused ossl_inline int ossl_quic_pn_valid(QUIC_PN pn)
{
    return pn < (((QUIC_PN)1) << 62);
}

/* QUIC connection ID representation. */
#  define QUIC_MAX_CONN_ID_LEN   20
#  define QUIC_MIN_ODCID_LEN     8   /* RFC 9000 s. 7.2 */

typedef struct quic_conn_id_st {
    unsigned char id_len, id[QUIC_MAX_CONN_ID_LEN];
} QUIC_CONN_ID;

static ossl_unused ossl_inline int ossl_quic_conn_id_eq(const QUIC_CONN_ID *a,
                                                        const QUIC_CONN_ID *b)
{
    if (a->id_len != b->id_len || a->id_len > QUIC_MAX_CONN_ID_LEN)
        return 0;
    return memcmp(a->id, b->id, a->id_len) == 0;
}

/*
 * Generates a random CID of the given length. libctx may be NULL.
 * Returns 1 on success or 0 on failure.
 */
int ossl_quic_gen_rand_conn_id(OSSL_LIB_CTX *libctx, size_t len,
                               QUIC_CONN_ID *cid);

#  define QUIC_MIN_INITIAL_DGRAM_LEN  1200

#  define QUIC_DEFAULT_ACK_DELAY_EXP  3
#  define QUIC_MAX_ACK_DELAY_EXP      20

#  define QUIC_DEFAULT_MAX_ACK_DELAY  25

#  define QUIC_MIN_ACTIVE_CONN_ID_LIMIT   2

/* Arbitrary choice of default idle timeout (not an RFC value). */
#  define QUIC_DEFAULT_IDLE_TIMEOUT   30000

#  define QUIC_STATELESS_RESET_TOKEN_LEN    16

typedef struct {
    unsigned char token[QUIC_STATELESS_RESET_TOKEN_LEN];
} QUIC_STATELESS_RESET_TOKEN;

/*
 * An encoded preferred_addr transport parameter cannot be shorter or longer
 * than these lengths in bytes.
 */
#  define QUIC_MIN_ENCODED_PREFERRED_ADDR_LEN   41
#  define QUIC_MAX_ENCODED_PREFERRED_ADDR_LEN   61

# endif

#endif
