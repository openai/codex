/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/unicode.h"
#include <openssl/asn1.h>

/* UTF8 utilities */

/*-
 * This parses a UTF8 string one character at a time. It is passed a pointer
 * to the string and the length of the string. It sets 'value' to the value of
 * the current character. It returns the number of characters read or a
 * negative error code:
 * -1 = string too short
 * -2 = illegal character
 * -3 = subsequent characters not of the form 10xxxxxx
 * -4 = character encoded incorrectly (not minimal length).
 */

int UTF8_getc(const unsigned char *str, int len, unsigned long *val)
{
    const unsigned char *p;
    unsigned long value;
    int ret;
    if (len <= 0)
        return 0;
    p = str;

    /* Check syntax and work out the encoded value (if correct) */
    if ((*p & 0x80) == 0) {
        value = *p++ & 0x7f;
        ret = 1;
    } else if ((*p & 0xe0) == 0xc0) {
        if (len < 2)
            return -1;
        if ((p[1] & 0xc0) != 0x80)
            return -3;
        value = (*p++ & 0x1f) << 6;
        value |= *p++ & 0x3f;
        if (value < 0x80)
            return -4;
        ret = 2;
    } else if ((*p & 0xf0) == 0xe0) {
        if (len < 3)
            return -1;
        if (((p[1] & 0xc0) != 0x80)
            || ((p[2] & 0xc0) != 0x80))
            return -3;
        value = (*p++ & 0xf) << 12;
        value |= (*p++ & 0x3f) << 6;
        value |= *p++ & 0x3f;
        if (value < 0x800)
            return -4;
        if (is_unicode_surrogate(value))
            return -2;
        ret = 3;
    } else if ((*p & 0xf8) == 0xf0) {
        if (len < 4)
            return -1;
        if (((p[1] & 0xc0) != 0x80)
            || ((p[2] & 0xc0) != 0x80)
            || ((p[3] & 0xc0) != 0x80))
            return -3;
        value = ((unsigned long)(*p++ & 0x7)) << 18;
        value |= (*p++ & 0x3f) << 12;
        value |= (*p++ & 0x3f) << 6;
        value |= *p++ & 0x3f;
        if (value < 0x10000)
            return -4;
        ret = 4;
    } else
        return -2;
    *val = value;
    return ret;
}

/*
 * This takes a character 'value' and writes the UTF8 encoded value in 'str'
 * where 'str' is a buffer containing 'len' characters. Returns the number of
 * characters written, -1 if 'len' is too small or -2 if 'value' is out of
 * range. 'str' can be set to NULL in which case it just returns the number of
 * characters. It will need at most 4 characters.
 */

int UTF8_putc(unsigned char *str, int len, unsigned long value)
{
    if (!str)
        len = 4;                /* Maximum we will need */
    else if (len <= 0)
        return -1;
    if (value < 0x80) {
        if (str)
            *str = (unsigned char)value;
        return 1;
    }
    if (value < 0x800) {
        if (len < 2)
            return -1;
        if (str) {
            *str++ = (unsigned char)(((value >> 6) & 0x1f) | 0xc0);
            *str = (unsigned char)((value & 0x3f) | 0x80);
        }
        return 2;
    }
    if (value < 0x10000) {
        if (is_unicode_surrogate(value))
            return -2;
        if (len < 3)
            return -1;
        if (str) {
            *str++ = (unsigned char)(((value >> 12) & 0xf) | 0xe0);
            *str++ = (unsigned char)(((value >> 6) & 0x3f) | 0x80);
            *str = (unsigned char)((value & 0x3f) | 0x80);
        }
        return 3;
    }
    if (value < UNICODE_LIMIT) {
        if (len < 4)
            return -1;
        if (str) {
            *str++ = (unsigned char)(((value >> 18) & 0x7) | 0xf0);
            *str++ = (unsigned char)(((value >> 12) & 0x3f) | 0x80);
            *str++ = (unsigned char)(((value >> 6) & 0x3f) | 0x80);
            *str = (unsigned char)((value & 0x3f) | 0x80);
        }
        return 4;
    }
    return -2;
}
