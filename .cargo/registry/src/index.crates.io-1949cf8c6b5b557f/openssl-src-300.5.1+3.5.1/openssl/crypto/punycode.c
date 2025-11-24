/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stddef.h>
#include <stdio.h>
#include <openssl/e_os2.h>
#include "crypto/punycode.h"
#include "internal/common.h" /* for HAS_PREFIX */
#include "internal/packet.h" /* for WPACKET */

static const unsigned int base = 36;
static const unsigned int tmin = 1;
static const unsigned int tmax = 26;
static const unsigned int skew = 38;
static const unsigned int damp = 700;
static const unsigned int initial_bias = 72;
static const unsigned int initial_n = 0x80;
static const unsigned int maxint = 0xFFFFFFFF;
static const char delimiter = '-';

#define LABEL_BUF_SIZE 512

/*-
 * Pseudocode:
 *
 * function adapt(delta,numpoints,firsttime):
 *  if firsttime then let delta = delta div damp
 *  else let delta = delta div 2
 *  let delta = delta + (delta div numpoints)
 *  let k = 0
 *  while delta > ((base - tmin) * tmax) div 2 do begin
 *    let delta = delta div (base - tmin)
 *    let k = k + base
 *  end
 *  return k + (((base - tmin + 1) * delta) div (delta + skew))
 */

static int adapt(unsigned int delta, unsigned int numpoints,
                 unsigned int firsttime)
{
    unsigned int k = 0;

    delta = (firsttime) ? delta / damp : delta / 2;
    delta = delta + delta / numpoints;

    while (delta > ((base - tmin) * tmax) / 2) {
        delta = delta / (base - tmin);
        k = k + base;
    }

    return k + (((base - tmin + 1) * delta) / (delta + skew));
}

static ossl_inline int is_basic(unsigned int a)
{
    return (a < 0x80) ? 1 : 0;
}

/*-
 * code points    digit-values
 * ------------   ----------------------
 * 41..5A (A-Z) =  0 to 25, respectively
 * 61..7A (a-z) =  0 to 25, respectively
 * 30..39 (0-9) = 26 to 35, respectively
 */
static ossl_inline int digit_decoded(const unsigned char a)
{
    if (a >= 0x41 && a <= 0x5A)
        return a - 0x41;

    if (a >= 0x61 && a <= 0x7A)
        return a - 0x61;

    if (a >= 0x30 && a <= 0x39)
        return a - 0x30 + 26;

    return -1;
}

/*-
 * Pseudocode:
 *
 * function ossl_punycode_decode
 *  let n = initial_n
 *  let i = 0
 *  let bias = initial_bias
 *  let output = an empty string indexed from 0
 *  consume all code points before the last delimiter (if there is one)
 *    and copy them to output, fail on any non-basic code point
 *  if more than zero code points were consumed then consume one more
 *    (which will be the last delimiter)
 *  while the input is not exhausted do begin
 *    let oldi = i
 *    let w = 1
 *    for k = base to infinity in steps of base do begin
 *      consume a code point, or fail if there was none to consume
 *      let digit = the code point's digit-value, fail if it has none
 *      let i = i + digit * w, fail on overflow
 *      let t = tmin if k <= bias {+ tmin}, or
 *              tmax if k >= bias + tmax, or k - bias otherwise
 *      if digit < t then break
 *      let w = w * (base - t), fail on overflow
 *    end
 *    let bias = adapt(i - oldi, length(output) + 1, test oldi is 0?)
 *    let n = n + i div (length(output) + 1), fail on overflow
 *    let i = i mod (length(output) + 1)
 *    {if n is a basic code point then fail}
 *    insert n into output at position i
 *    increment i
 *  end
 */

int ossl_punycode_decode(const char *pEncoded, const size_t enc_len,
                         unsigned int *pDecoded, unsigned int *pout_length)
{
    unsigned int n = initial_n;
    unsigned int i = 0;
    unsigned int bias = initial_bias;
    size_t processed_in = 0, written_out = 0;
    unsigned int max_out = *pout_length;
    unsigned int basic_count = 0;
    unsigned int loop;

    for (loop = 0; loop < enc_len; loop++) {
        if (pEncoded[loop] == delimiter)
            basic_count = loop;
    }

    if (basic_count > 0) {
        if (basic_count > max_out)
            return 0;

        for (loop = 0; loop < basic_count; loop++) {
            if (is_basic(pEncoded[loop]) == 0)
                return 0;

            pDecoded[loop] = pEncoded[loop];
            written_out++;
        }
        processed_in = basic_count + 1;
    }

    for (loop = processed_in; loop < enc_len;) {
        unsigned int oldi = i;
        unsigned int w = 1;
        unsigned int k, t;
        int digit;

        for (k = base;; k += base) {
            if (loop >= enc_len)
                return 0;

            digit = digit_decoded(pEncoded[loop]);
            loop++;

            if (digit < 0)
                return 0;
            if ((unsigned int)digit > (maxint - i) / w)
                return 0;

            i = i + digit * w;
            t = (k <= bias) ? tmin : (k >= bias + tmax) ? tmax : k - bias;

            if ((unsigned int)digit < t)
                break;

            if (w > maxint / (base - t))
                return 0;
            w = w * (base - t);
        }

        bias = adapt(i - oldi, written_out + 1, (oldi == 0));
        if (i / (written_out + 1) > maxint - n)
            return 0;
        n = n + i / (written_out + 1);
        i %= (written_out + 1);

        if (written_out >= max_out)
            return 0;

        memmove(pDecoded + i + 1, pDecoded + i,
                (written_out - i) * sizeof(*pDecoded));
        pDecoded[i] = n;
        i++;
        written_out++;
    }

    *pout_length = written_out;
    return 1;
}

/*
 * Encode a code point using UTF-8
 * return number of bytes on success, 0 on failure
 * (also produces U+FFFD, which uses 3 bytes on failure)
 */
static int codepoint2utf8(unsigned char *out, unsigned long utf)
{
    if (utf <= 0x7F) {
        /* Plain ASCII */
        out[0] = (unsigned char)utf;
        out[1] = 0;
        return 1;
    } else if (utf <= 0x07FF) {
        /* 2-byte unicode */
        out[0] = (unsigned char)(((utf >> 6) & 0x1F) | 0xC0);
        out[1] = (unsigned char)(((utf >> 0) & 0x3F) | 0x80);
        out[2] = 0;
        return 2;
    } else if (utf <= 0xFFFF) {
        /* 3-byte unicode */
        out[0] = (unsigned char)(((utf >> 12) & 0x0F) | 0xE0);
        out[1] = (unsigned char)(((utf >> 6) & 0x3F) | 0x80);
        out[2] = (unsigned char)(((utf >> 0) & 0x3F) | 0x80);
        out[3] = 0;
        return 3;
    } else if (utf <= 0x10FFFF) {
        /* 4-byte unicode */
        out[0] = (unsigned char)(((utf >> 18) & 0x07) | 0xF0);
        out[1] = (unsigned char)(((utf >> 12) & 0x3F) | 0x80);
        out[2] = (unsigned char)(((utf >> 6) & 0x3F) | 0x80);
        out[3] = (unsigned char)(((utf >> 0) & 0x3F) | 0x80);
        out[4] = 0;
        return 4;
    } else {
        /* error - use replacement character */
        out[0] = (unsigned char)0xEF;
        out[1] = (unsigned char)0xBF;
        out[2] = (unsigned char)0xBD;
        out[3] = 0;
        return 0;
    }
}

/*-
 * Return values:
 * 1 - ok
 * 0 - ok but buf was too short
 * -1 - bad string passed or other error
 */

int ossl_a2ulabel(const char *in, char *out, size_t outlen)
{
    /*-
     * Domain name has some parts consisting of ASCII chars joined with dot.
     * If a part is shorter than 5 chars, it becomes U-label as is.
     * If it does not start with xn--,    it becomes U-label as is.
     * Otherwise we try to decode it.
     */
    const char *inptr = in;
    int result = 1;
    unsigned int i;
    unsigned int buf[LABEL_BUF_SIZE];      /* It's a hostname */
    WPACKET pkt;

    /* Internal API, so should not fail */
    if (!ossl_assert(out != NULL))
        return -1;

    if (!WPACKET_init_static_len(&pkt, (unsigned char *)out, outlen, 0))
        return -1;

    while (1) {
        char *tmpptr = strchr(inptr, '.');
        size_t delta = tmpptr != NULL ? (size_t)(tmpptr - inptr) : strlen(inptr);

        if (!HAS_PREFIX(inptr, "xn--")) {
            if (!WPACKET_memcpy(&pkt, inptr, delta))
                result = 0;
        } else {
            unsigned int bufsize = LABEL_BUF_SIZE;

            if (ossl_punycode_decode(inptr + 4, delta - 4, buf, &bufsize) <= 0) {
                result = -1;
                goto end;
            }

            for (i = 0; i < bufsize; i++) {
                unsigned char seed[6];
                size_t utfsize = codepoint2utf8(seed, buf[i]);

                if (utfsize == 0) {
                    result = -1;
                    goto end;
                }

                if (!WPACKET_memcpy(&pkt, seed, utfsize))
                    result = 0;
            }
        }

        if (tmpptr == NULL)
            break;

        if (!WPACKET_put_bytes_u8(&pkt, '.'))
            result = 0;

        inptr = tmpptr + 1;
    }

    if (!WPACKET_put_bytes_u8(&pkt, '\0'))
        result = 0;
 end:
    WPACKET_cleanup(&pkt);
    return result;
}
