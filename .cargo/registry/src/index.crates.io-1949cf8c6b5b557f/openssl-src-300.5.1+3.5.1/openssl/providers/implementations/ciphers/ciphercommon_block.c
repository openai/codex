/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
/* For SSL3_VERSION, TLS1_VERSION etc */
#include <openssl/prov_ssl.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "internal/constant_time.h"
#include "internal/ssl3_cbc.h"
#include "ciphercommon_local.h"

/*
 * Fills a single block of buffered data from the input, and returns the amount
 * of data remaining in the input that is a multiple of the blocksize. The buffer
 * is only filled if it already has some data in it, isn't full already or we
 * don't have at least one block in the input.
 *
 * buf: a buffer of blocksize bytes
 * buflen: contains the amount of data already in buf on entry. Updated with the
 *         amount of data in buf at the end. On entry *buflen must always be
 *         less than the blocksize
 * blocksize: size of a block. Must be greater than 0 and a power of 2
 * in: pointer to a pointer containing the input data
 * inlen: amount of input data available
 *
 * On return buf is filled with as much data as possible up to a full block,
 * *buflen is updated containing the amount of data in buf. *in is updated to
 * the new location where input data should be read from, *inlen is updated with
 * the remaining amount of data in *in. Returns the largest value <= *inlen
 * which is a multiple of the blocksize.
 */
size_t ossl_cipher_fillblock(unsigned char *buf, size_t *buflen,
                             size_t blocksize,
                             const unsigned char **in, size_t *inlen)
{
    size_t blockmask = ~(blocksize - 1);
    size_t bufremain = blocksize - *buflen;

    assert(*buflen <= blocksize);
    assert(blocksize > 0 && (blocksize & (blocksize - 1)) == 0);

    if (*inlen < bufremain)
        bufremain = *inlen;
    memcpy(buf + *buflen, *in, bufremain);
    *in += bufremain;
    *inlen -= bufremain;
    *buflen += bufremain;

    return *inlen & blockmask;
}

/*
 * Fills the buffer with trailing data from an encryption/decryption that didn't
 * fit into a full block.
 */
int ossl_cipher_trailingdata(unsigned char *buf, size_t *buflen, size_t blocksize,
                             const unsigned char **in, size_t *inlen)
{
    if (*inlen == 0)
        return 1;

    if (*buflen + *inlen > blocksize) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    memcpy(buf + *buflen, *in, *inlen);
    *buflen += *inlen;
    *inlen = 0;

    return 1;
}

/* Pad the final block for encryption */
void ossl_cipher_padblock(unsigned char *buf, size_t *buflen, size_t blocksize)
{
    size_t i;
    unsigned char pad = (unsigned char)(blocksize - *buflen);

    for (i = *buflen; i < blocksize; i++)
        buf[i] = pad;
}

int ossl_cipher_unpadblock(unsigned char *buf, size_t *buflen, size_t blocksize)
{
    size_t pad, i;
    size_t len = *buflen;

    if (len != blocksize) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /*
     * The following assumes that the ciphertext has been authenticated.
     * Otherwise it provides a padding oracle.
     */
    pad = buf[blocksize - 1];
    if (pad == 0 || pad > blocksize) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BAD_DECRYPT);
        return 0;
    }
    for (i = 0; i < pad; i++) {
        if (buf[--len] != pad) {
            ERR_raise(ERR_LIB_PROV, PROV_R_BAD_DECRYPT);
            return 0;
        }
    }
    *buflen = len;
    return 1;
}

/*-
 * ossl_cipher_tlsunpadblock removes the CBC padding from the decrypted, TLS, CBC
 * record in constant time. Also removes the MAC from the record in constant
 * time.
 *
 * libctx: Our library context
 * tlsversion: The TLS version in use, e.g. SSL3_VERSION, TLS1_VERSION, etc
 * buf: The decrypted TLS record data
 * buflen: The length of the decrypted TLS record data. Updated with the new
 *         length after the padding is removed
 * block_size: the block size of the cipher used to encrypt the record.
 * mac: Location to store the pointer to the MAC
 * alloced: Whether the MAC is stored in a newly allocated buffer, or whether
 *          *mac points into *buf
 * macsize: the size of the MAC inside the record (or 0 if there isn't one)
 * aead: whether this is an aead cipher
 * returns:
 *   0: (in non-constant time) if the record is publicly invalid.
 *   1: (in constant time) Record is publicly valid. If padding is invalid then
 *      the mac is random
 */
int ossl_cipher_tlsunpadblock(OSSL_LIB_CTX *libctx, unsigned int tlsversion,
                              unsigned char *buf, size_t *buflen,
                              size_t blocksize,
                              unsigned char **mac, int *alloced, size_t macsize,
                              int aead)
{
    int ret;

    switch (tlsversion) {
    case SSL3_VERSION:
        return ssl3_cbc_remove_padding_and_mac(buflen, *buflen, buf, mac,
                                               alloced, blocksize, macsize,
                                               libctx);

    case TLS1_2_VERSION:
    case DTLS1_2_VERSION:
    case TLS1_1_VERSION:
    case DTLS1_VERSION:
    case DTLS1_BAD_VER:
        /* Remove the explicit IV */
        buf += blocksize;
        *buflen -= blocksize;
        /* Fall through */
    case TLS1_VERSION:
        ret = tls1_cbc_remove_padding_and_mac(buflen, *buflen, buf, mac,
                                              alloced, blocksize, macsize,
                                              aead, libctx);
        return ret;

    default:
        return 0;
    }
}
