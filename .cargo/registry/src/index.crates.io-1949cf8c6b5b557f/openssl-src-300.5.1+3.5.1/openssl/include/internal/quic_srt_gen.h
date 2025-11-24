/*
* Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_SRT_GEN_H
# define OSSL_INTERNAL_QUIC_SRT_GEN_H
# pragma once

# include "internal/e_os.h"
# include "internal/time.h"
# include "internal/quic_types.h"
# include "internal/quic_wire.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Stateless Reset Token Generator
 * ====================================
 *
 * This generates 16-byte QUIC Stateless Reset Tokens given a secret symmetric
 * key and a DCID. Because the output is deterministic with regards to these
 * inputs, assuming the same key is used between invocations of a process, we
 * are able to generate the same stateless reset token in a subsequent process,
 * thereby allowing us to achieve stateless reset of a peer which still thinks
 * it is connected to a past process at the same UDP address.
 */
typedef struct quic_srt_gen_st QUIC_SRT_GEN;

/*
 * Create a new stateless reset token generator using the given key as input.
 * The key may be of arbitrary length.
 *
 * The caller is responsible for performing domain separation with regards to
 * the key; i.e., the caller is responsible for ensuring the key is never used
 * in any other context.
 */
QUIC_SRT_GEN *ossl_quic_srt_gen_new(OSSL_LIB_CTX *libctx, const char *propq,
                                    const unsigned char *key, size_t key_len);

/* Free the stateless reset token generator. No-op if srt_gen is NULL. */
void ossl_quic_srt_gen_free(QUIC_SRT_GEN *srt_gen);

/*
 * Calculates a token using the given DCID and writes it to *token. Returns 0 on
 * failure.
 */
int ossl_quic_srt_gen_calculate_token(QUIC_SRT_GEN *srt_gen,
                                      const QUIC_CONN_ID *dcid,
                                      QUIC_STATELESS_RESET_TOKEN *token);

# endif
#endif
