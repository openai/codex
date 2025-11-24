/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rand.h>
#include <openssl/evp.h>

#include "internal/constant_time.h"
#include "internal/cryptlib.h"
#include "internal/ssl3_cbc.h"

/*
 * This file has no dependencies on the rest of libssl because it is shared
 * with the providers. It contains functions for low level CBC TLS padding
 * removal. Responsibility for this lies with the cipher implementations in the
 * providers. However there are legacy code paths in libssl which also need to
 * do this. In time those legacy code paths can be removed and this file can be
 * moved out of libssl.
 */

static int ssl3_cbc_copy_mac(size_t *reclen,
                             size_t origreclen,
                             unsigned char *recdata,
                             unsigned char **mac,
                             int *alloced,
                             size_t block_size,
                             size_t mac_size,
                             size_t good,
                             OSSL_LIB_CTX *libctx);

/*-
 * ssl3_cbc_remove_padding removes padding from the decrypted, SSLv3, CBC
 * record in |recdata| by updating |reclen| in constant time. It also extracts
 * the MAC from the underlying record and places a pointer to it in |mac|. The
 * MAC data can either be newly allocated memory, or a pointer inside the
 * |recdata| buffer. If allocated then |*alloced| is set to 1, otherwise it is
 * set to 0.
 *
 * origreclen: the original record length before any changes were made
 * block_size: the block size of the cipher used to encrypt the record.
 * mac_size: the size of the MAC to be extracted
 * aead: 1 if an AEAD cipher is in use, or 0 otherwise
 * returns:
 *   0: if the record is publicly invalid.
 *   1: if the record is publicly valid. If the padding removal fails then the
 *      MAC returned is random.
 */
int ssl3_cbc_remove_padding_and_mac(size_t *reclen,
                                    size_t origreclen,
                                    unsigned char *recdata,
                                    unsigned char **mac,
                                    int *alloced,
                                    size_t block_size, size_t mac_size,
                                    OSSL_LIB_CTX *libctx)
{
    size_t padding_length;
    size_t good;
    const size_t overhead = 1 /* padding length byte */  + mac_size;

    /*
     * These lengths are all public so we can test them in non-constant time.
     */
    if (overhead > *reclen)
        return 0;

    padding_length = recdata[*reclen - 1];
    good = constant_time_ge_s(*reclen, padding_length + overhead);
    /* SSLv3 requires that the padding is minimal. */
    good &= constant_time_ge_s(block_size, padding_length + 1);
    *reclen -= good & (padding_length + 1);

    return ssl3_cbc_copy_mac(reclen, origreclen, recdata, mac, alloced,
                             block_size, mac_size, good, libctx);
}

/*-
 * tls1_cbc_remove_padding_and_mac removes padding from the decrypted, TLS, CBC
 * record in |recdata| by updating |reclen| in constant time. It also extracts
 * the MAC from the underlying record and places a pointer to it in |mac|. The
 * MAC data can either be newly allocated memory, or a pointer inside the
 * |recdata| buffer. If allocated then |*alloced| is set to 1, otherwise it is
 * set to 0.
 *
 * origreclen: the original record length before any changes were made
 * block_size: the block size of the cipher used to encrypt the record.
 * mac_size: the size of the MAC to be extracted
 * aead: 1 if an AEAD cipher is in use, or 0 otherwise
 * returns:
 *   0: if the record is publicly invalid.
 *   1: if the record is publicly valid. If the padding removal fails then the
 *      MAC returned is random.
 */
int tls1_cbc_remove_padding_and_mac(size_t *reclen,
                                    size_t origreclen,
                                    unsigned char *recdata,
                                    unsigned char **mac,
                                    int *alloced,
                                    size_t block_size, size_t mac_size,
                                    int aead,
                                    OSSL_LIB_CTX *libctx)
{
    size_t good = -1;
    size_t padding_length, to_check, i;
    size_t overhead = ((block_size == 1) ? 0 : 1) /* padding length byte */
                      + mac_size;

    /*
     * These lengths are all public so we can test them in non-constant
     * time.
     */
    if (overhead > *reclen)
        return 0;

    if (block_size != 1) {

        padding_length = recdata[*reclen - 1];

        if (aead) {
            /* padding is already verified and we don't need to check the MAC */
            *reclen -= padding_length + 1 + mac_size;
            return 1;
        }

        good = constant_time_ge_s(*reclen, overhead + padding_length);
        /*
         * The padding consists of a length byte at the end of the record and
         * then that many bytes of padding, all with the same value as the
         * length byte. Thus, with the length byte included, there are i+1 bytes
         * of padding. We can't check just |padding_length+1| bytes because that
         * leaks decrypted information. Therefore we always have to check the
         * maximum amount of padding possible. (Again, the length of the record
         * is public information so we can use it.)
         */
        to_check = 256;        /* maximum amount of padding, inc length byte. */
        if (to_check > *reclen)
            to_check = *reclen;

        for (i = 0; i < to_check; i++) {
            unsigned char mask = constant_time_ge_8_s(padding_length, i);
            unsigned char b = recdata[*reclen - 1 - i];
            /*
             * The final |padding_length+1| bytes should all have the value
             * |padding_length|. Therefore the XOR should be zero.
             */
            good &= ~(mask & (padding_length ^ b));
        }

        /*
         * If any of the final |padding_length+1| bytes had the wrong value, one
         * or more of the lower eight bits of |good| will be cleared.
         */
        good = constant_time_eq_s(0xff, good & 0xff);
        *reclen -= good & (padding_length + 1);
    }

    return ssl3_cbc_copy_mac(reclen, origreclen, recdata, mac, alloced,
                             block_size, mac_size, good, libctx);
}

/*-
 * ssl3_cbc_copy_mac copies |md_size| bytes from the end of the record in
 * |recdata| to |*mac| in constant time (independent of the concrete value of
 * the record length |reclen|, which may vary within a 256-byte window).
 *
 * On entry:
 *   origreclen >= mac_size
 *   mac_size <= EVP_MAX_MD_SIZE
 *
 * If CBC_MAC_ROTATE_IN_PLACE is defined then the rotation is performed with
 * variable accesses in a 64-byte-aligned buffer. Assuming that this fits into
 * a single or pair of cache-lines, then the variable memory accesses don't
 * actually affect the timing. CPUs with smaller cache-lines [if any] are
 * not multi-core and are not considered vulnerable to cache-timing attacks.
 */
#define CBC_MAC_ROTATE_IN_PLACE

static int ssl3_cbc_copy_mac(size_t *reclen,
                             size_t origreclen,
                             unsigned char *recdata,
                             unsigned char **mac,
                             int *alloced,
                             size_t block_size,
                             size_t mac_size,
                             size_t good,
                             OSSL_LIB_CTX *libctx)
{
#if defined(CBC_MAC_ROTATE_IN_PLACE)
    unsigned char rotated_mac_buf[64 + EVP_MAX_MD_SIZE];
    unsigned char *rotated_mac;
    char aux1, aux2, aux3, mask;
#else
    unsigned char rotated_mac[EVP_MAX_MD_SIZE];
#endif
    unsigned char randmac[EVP_MAX_MD_SIZE];
    unsigned char *out;

    /*
     * mac_end is the index of |recdata| just after the end of the MAC.
     */
    size_t mac_end = *reclen;
    size_t mac_start = mac_end - mac_size;
    size_t in_mac;
    /*
     * scan_start contains the number of bytes that we can ignore because the
     * MAC's position can only vary by 255 bytes.
     */
    size_t scan_start = 0;
    size_t i, j;
    size_t rotate_offset;

    if (!ossl_assert(origreclen >= mac_size
                     && mac_size <= EVP_MAX_MD_SIZE))
        return 0;

    /* If no MAC then nothing to be done */
    if (mac_size == 0) {
        /* No MAC so we can do this in non-constant time */
        if (good == 0)
            return 0;
        return 1;
    }

    *reclen -= mac_size;

    if (block_size == 1) {
        /* There's no padding so the position of the MAC is fixed */
        if (mac != NULL)
            *mac = &recdata[*reclen];
        if (alloced != NULL)
            *alloced = 0;
        return 1;
    }

    /* Create the random MAC we will emit if padding is bad */
    if (RAND_bytes_ex(libctx, randmac, mac_size, 0) <= 0)
        return 0;

    if (!ossl_assert(mac != NULL && alloced != NULL))
        return 0;
    *mac = out = OPENSSL_malloc(mac_size);
    if (*mac == NULL)
        return 0;
    *alloced = 1;

#if defined(CBC_MAC_ROTATE_IN_PLACE)
    rotated_mac = rotated_mac_buf + ((0 - (size_t)rotated_mac_buf) & 63);
#endif

    /* This information is public so it's safe to branch based on it. */
    if (origreclen > mac_size + 255 + 1)
        scan_start = origreclen - (mac_size + 255 + 1);

    in_mac = 0;
    rotate_offset = 0;
    memset(rotated_mac, 0, mac_size);
    for (i = scan_start, j = 0; i < origreclen; i++) {
        size_t mac_started = constant_time_eq_s(i, mac_start);
        size_t mac_ended = constant_time_lt_s(i, mac_end);
        unsigned char b = recdata[i];

        in_mac |= mac_started;
        in_mac &= mac_ended;
        rotate_offset |= j & mac_started;
        rotated_mac[j++] |= b & in_mac;
        j &= constant_time_lt_s(j, mac_size);
    }

    /* Now rotate the MAC */
#if defined(CBC_MAC_ROTATE_IN_PLACE)
    j = 0;
    for (i = 0; i < mac_size; i++) {
        /*
         * in case cache-line is 32 bytes,
         * load from both lines and select appropriately
         */
        aux1 = rotated_mac[rotate_offset & ~32];
        aux2 = rotated_mac[rotate_offset | 32];
        mask = constant_time_eq_8(rotate_offset & ~32, rotate_offset);
        aux3 = constant_time_select_8(mask, aux1, aux2);
        rotate_offset++;

        /* If the padding wasn't good we emit a random MAC */
        out[j++] = constant_time_select_8((unsigned char)(good & 0xff),
                                          aux3,
                                          randmac[i]);
        rotate_offset &= constant_time_lt_s(rotate_offset, mac_size);
    }
#else
    memset(out, 0, mac_size);
    rotate_offset = mac_size - rotate_offset;
    rotate_offset &= constant_time_lt_s(rotate_offset, mac_size);
    for (i = 0; i < mac_size; i++) {
        for (j = 0; j < mac_size; j++)
            out[j] |= rotated_mac[i] & constant_time_eq_8_s(j, rotate_offset);
        rotate_offset++;
        rotate_offset &= constant_time_lt_s(rotate_offset, mac_size);

        /* If the padding wasn't good we emit a random MAC */
        out[i] = constant_time_select_8((unsigned char)(good & 0xff), out[i],
                                        randmac[i]);
    }
#endif

    return 1;
}
