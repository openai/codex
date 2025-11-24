/*
 * Copyright 2005-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/objects.h>

/*
 * X9.31 Embeds the hash inside the following data structure
 *
 * header (4 bits = 0x6)
 * padding (consisting of zero or more 4 bit values of a sequence of 0xB,
 *          ending with the terminator 0xA)
 * hash(Msg) hash of a message (the output size is related to the hash function)
 * trailer (consists of 2 bytes)
 *         The 1st byte is related to a part number for a hash algorithm
 *         (See RSA_X931_hash_id()), followed by the fixed value 0xCC
 *
 * The RSA modulus size n (which for X9.31 is 1024 + 256*s) is the size of the data
 * structure, which determines the padding size.
 * i.e. len(padding) = n - len(header) - len(hash) - len(trailer)
 *
 * Params:
 *     to The output buffer to write the data structure to.
 *     tolen The size of 'to' in bytes (it is the size of the n)
 *     from The input hash followed by the 1st byte of the trailer.
 *     flen The size of the input hash + 1 (trailer byte)
 */
int RSA_padding_add_X931(unsigned char *to, int tlen,
                         const unsigned char *from, int flen)
{
    int j;
    unsigned char *p;

    /*
     * We need at least 1 byte for header + padding (0x6A)
     * And 2 trailer bytes (but we subtract 1 since flen includes 1 trailer byte)
     */
    j = tlen - flen - 2;

    if (j < 0) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE_FOR_KEY_SIZE);
        return -1;
    }

    p = (unsigned char *)to;

    /* If no padding start and end nibbles are in one byte */
    if (j == 0) {
        *p++ = 0x6A;
    } else {
        *p++ = 0x6B;
        if (j > 1) {
            memset(p, 0xBB, j - 1);
            p += j - 1;
        }
        *p++ = 0xBA;
    }
    memcpy(p, from, (unsigned int)flen);
    p += flen;
    *p = 0xCC;
    return 1;
}

int RSA_padding_check_X931(unsigned char *to, int tlen,
                           const unsigned char *from, int flen, int num)
{
    int i = 0, j;
    const unsigned char *p;

    p = from;
    if ((num != flen) || ((*p != 0x6A) && (*p != 0x6B))) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_HEADER);
        return -1;
    }

    if (*p++ == 0x6B) {
        j = flen - 3;
        for (i = 0; i < j; i++) {
            unsigned char c = *p++;
            if (c == 0xBA)
                break;
            if (c != 0xBB) {
                ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING);
                return -1;
            }
        }

        j -= i;

        if (i == 0) {
            ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_PADDING);
            return -1;
        }

    } else {
        j = flen - 2;
    }

    if (p[j] != 0xCC) {
        ERR_raise(ERR_LIB_RSA, RSA_R_INVALID_TRAILER);
        return -1;
    }

    memcpy(to, p, (unsigned int)j);

    return j;
}

/*
 * Translate between X9.31 hash ids and NIDs
 * The returned values relate to ISO/IEC 10118 part numbers which consist of
 * a hash algorithm and hash number. The returned values are used as the
 * first byte of the 'trailer'.
 */

int RSA_X931_hash_id(int nid)
{
    switch (nid) {
    case NID_sha1:
        return 0x33;

    case NID_sha256:
        return 0x34;

    case NID_sha384:
        return 0x36;

    case NID_sha512:
        return 0x35;

    }
    return -1;
}
