/*
* Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_VLINT_H
# define OSSL_INTERNAL_QUIC_VLINT_H
# pragma once

# include "internal/e_os.h"

# ifndef OPENSSL_NO_QUIC

/* The smallest value requiring a 1, 2, 4, or 8-byte representation. */
#define OSSL_QUIC_VLINT_1B_MIN 0
#define OSSL_QUIC_VLINT_2B_MIN 64
#define OSSL_QUIC_VLINT_4B_MIN 16384
#define OSSL_QUIC_VLINT_8B_MIN 1073741824

/* The largest value representable in a given number of bytes. */
#define OSSL_QUIC_VLINT_1B_MAX (OSSL_QUIC_VLINT_2B_MIN - 1)
#define OSSL_QUIC_VLINT_2B_MAX (OSSL_QUIC_VLINT_4B_MIN - 1)
#define OSSL_QUIC_VLINT_4B_MAX (OSSL_QUIC_VLINT_8B_MIN - 1)
#define OSSL_QUIC_VLINT_8B_MAX (((uint64_t)1 << 62) - 1)

/* The largest value representable as a variable-length integer. */
#define OSSL_QUIC_VLINT_MAX    OSSL_QUIC_VLINT_8B_MAX

/*
 * Returns the number of bytes needed to encode v in the QUIC variable-length
 * integer encoding.
 *
 * Returns 0 if v exceeds OSSL_QUIC_VLINT_MAX.
 */
static ossl_unused ossl_inline size_t ossl_quic_vlint_encode_len(uint64_t v)
{
    if (v < OSSL_QUIC_VLINT_2B_MIN)
        return 1;

    if (v < OSSL_QUIC_VLINT_4B_MIN)
        return 2;

    if (v < OSSL_QUIC_VLINT_8B_MIN)
        return 4;

    if (v <= OSSL_QUIC_VLINT_MAX)
        return 8;

    return 0;
}

/*
 * This function writes a QUIC varable-length encoded integer to buf.
 * The smallest usable representation is used.
 *
 * It is the caller's responsibility to ensure that the buffer is big enough by
 * calling ossl_quic_vlint_encode_len(v) before calling this function.
 *
 * Precondition: buf is at least ossl_quic_vlint_enc_len(v) bytes in size
 *   (unchecked)
 * Precondition: v does not exceed OSSL_QUIC_VLINT_MAX
 *   (unchecked)
 */
void ossl_quic_vlint_encode(unsigned char *buf, uint64_t v);

/*
 * This function writes a QUIC variable-length encoded integer to buf. The
 * specified number of bytes n are used for the encoding, which means that the
 * encoded value may take up more space than necessary.
 *
 * It is the caller's responsibility to ensure that the buffer is of at least n
 * bytes, and that v is representable by a n-byte QUIC variable-length integer.
 * The representable ranges are:
 *
 *   1-byte encoding: [0, 2** 6-1]
 *   2-byte encoding: [0, 2**14-1]
 *   4-byte encoding: [0, 2**30-1]
 *   8-byte encoding: [0, 2**62-1]
 *
 * Precondition: buf is at least n bytes in size (unchecked)
 * Precondition: v does not exceed the representable range
 *   (ossl_quic_vlint_encode_len(v) <= n) (unchecked)
 * Precondition: v does not exceed OSSL_QUIC_VLINT_MAX
 *   (unchecked)
 */
void ossl_quic_vlint_encode_n(unsigned char *buf, uint64_t v, int n);

/*
 * Given the first byte of an encoded QUIC variable-length integer, returns
 * the number of bytes comprising the encoded integer, including the first
 * byte.
 */
static ossl_unused ossl_inline size_t ossl_quic_vlint_decode_len(uint8_t first_byte)
{
    return 1U << ((first_byte & 0xC0) >> 6);
}

/*
 * Given a buffer containing an encoded QUIC variable-length integer, returns
 * the decoded value. The buffer must be of at least
 * ossl_quic_vlint_decode_len(buf[0]) bytes in size, and the caller is responsible
 * for checking this.
 *
 * Precondition: buf is at least ossl_quic_vlint_decode_len(buf[0]) bytes in size
 *   (unchecked)
 */
uint64_t ossl_quic_vlint_decode_unchecked(const unsigned char *buf);

/*
 * Given a buffer buf of buf_len bytes in length, attempts to decode an encoded
 * QUIC variable-length integer at the start of the buffer and writes the result
 * to *v. If buf_len is inadequate, suggesting a truncated encoded integer, the
 * function fails and 0 is returned. Otherwise, returns the number of bytes
 * consumed.
 *
 * Precondition: buf is at least buf_len bytes in size
 * Precondition: v (unchecked)
 */
int ossl_quic_vlint_decode(const unsigned char *buf, size_t buf_len, uint64_t *v);

# endif

#endif
