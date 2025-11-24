/*
* Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_SRTM_H
# define OSSL_INTERNAL_QUIC_SRTM_H
# pragma once

# include "internal/e_os.h"
# include "internal/time.h"
# include "internal/quic_types.h"
# include "internal/quic_wire.h"
# include "internal/quic_predef.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Stateless Reset Token Manager
 * ==================================
 *
 * The stateless reset token manager is responsible for mapping stateless reset
 * tokens to connections. It is used to identify stateless reset tokens in
 * incoming packets. In this regard it can be considered an alternate "routing"
 * mechanism for incoming packets, and is somewhat analogous with the LCIDM,
 * except that it uses SRTs to route rather than DCIDs.
 *
 * The SRTM specifically stores a bidirectional mapping of the form
 *
 *   (opaque pointer, sequence number) [1] <-> [0..n] SRT
 *
 * The (opaque pointer, sequence number) tuple is used to refer to an entry (for
 * example for the purposes of removing it later when it is no longer needed).
 * Likewise, an entry can be looked up using SRT to get the opaque pointer and
 * sequence number.
 *
 * It is important to note that the same SRT may exist multiple times and map to
 * multiple (opaque pointer, sequence number) tuples, for example, if we
 * initiate multiple connections to the same peer using the same local QUIC_PORT
 * and the peer decides to behave bizarrely and issue the same SRT for both
 * connections. It should not do this, but we have to be resilient against
 * byzantine peer behaviour. Thus we are capable of storing multiple identical
 * SRTs for different (opaque pointer, sequence number) keys.
 *
 * The SRTM supports arbitrary insertion, arbitrary deletion of specific keys
 * identified by a (opaque pointer, sequence number) key, and mass deletion of
 * all entries under a specific opaque pointer. It supports lookup by SRT to
 * identify zero or more corresponding (opaque pointer, sequence number) tuples.
 *
 * The opaque pointer may be used for any purpose but is intended to represent a
 * connection identity and must therefore be consistent (usefully comparable).
 */

/* Creates a new empty SRTM instance. */
QUIC_SRTM *ossl_quic_srtm_new(OSSL_LIB_CTX *libctx, const char *propq);

/* Frees a SRTM instance. No-op if srtm is NULL. */
void ossl_quic_srtm_free(QUIC_SRTM *srtm);

/*
 * Add a (opaque, seq_num) -> SRT entry to the SRTM. This operation fails if a
 * SRT entry already exists with the same (opaque, seq_num) tuple. The token is
 * copied. Returns 1 on success or 0 on failure.
 */
int ossl_quic_srtm_add(QUIC_SRTM *srtm, void *opaque, uint64_t seq_num,
                       const QUIC_STATELESS_RESET_TOKEN *token);

/*
 * Removes an entry by identifying it via its (opaque, seq_num) tuple.
 * Returns 1 if the entry was found and removed, and 0 if it was not found.
 */
int ossl_quic_srtm_remove(QUIC_SRTM *srtm, void *opaque, uint64_t seq_num);

/*
 * Removes all entries (opaque, *) with the given opaque pointer.
 *
 * Returns 1 on success and 0 on failure. If no entries with the given opaque
 * pointer were found, this is considered a success condition.
 */
int ossl_quic_srtm_cull(QUIC_SRTM *strm, void *opaque);

/*
 * Looks up a SRT to find the corresponding opaque pointer and sequence number.
 * An output field pointer can be set to NULL if it is not required.
 *
 * This function is designed to avoid exposing timing channels on token values
 * or the contents of the SRT mapping.
 *
 * If there are several identical SRTs, idx can be used to get the nth entry.
 * Call this function with idx set to 0 first, and keep calling it after
 * incrementing idx until it returns 0.
 *
 * Returns 1 if an entry was found and 0 otherwise.
 */
int ossl_quic_srtm_lookup(QUIC_SRTM *srtm,
                          const QUIC_STATELESS_RESET_TOKEN *token,
                          size_t idx,
                          void **opaque, uint64_t *seq_num);

/* Verify internal invariants and assert if they are not met. */
void ossl_quic_srtm_check(const QUIC_SRTM *srtm);

# endif

#endif
