/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/buffer.h>
#include "crypto/x509.h"
#include "crypto/ctype.h"

/*
 * Limit to ensure we don't overflow: much greater than
 * anything encountered in practice.
 */

#define NAME_ONELINE_MAX    (1024 * 1024)

char *X509_NAME_oneline(const X509_NAME *a, char *buf, int len)
{
    const X509_NAME_ENTRY *ne;
    int i;
    int n, lold, l, l1, l2, num, j, type;
    int prev_set = -1;
    const char *s;
    char *p;
    unsigned char *q;
    BUF_MEM *b = NULL;
    int gs_doit[4];
    char tmp_buf[80];
#ifdef CHARSET_EBCDIC
    unsigned char ebcdic_buf[1024];
#endif

    if (buf == NULL) {
        if ((b = BUF_MEM_new()) == NULL)
            goto buferr;
        if (!BUF_MEM_grow(b, 200))
            goto buferr;
        b->data[0] = '\0';
        len = 200;
    } else if (len == 0) {
        return NULL;
    }
    if (a == NULL) {
        if (b) {
            buf = b->data;
            OPENSSL_free(b);
        }
        strncpy(buf, "NO X509_NAME", len);
        buf[len - 1] = '\0';
        return buf;
    }

    len--;                      /* space for '\0' */
    l = 0;
    for (i = 0; i < sk_X509_NAME_ENTRY_num(a->entries); i++) {
        ne = sk_X509_NAME_ENTRY_value(a->entries, i);
        n = OBJ_obj2nid(ne->object);
        if ((n == NID_undef) || ((s = OBJ_nid2sn(n)) == NULL)) {
            i2t_ASN1_OBJECT(tmp_buf, sizeof(tmp_buf), ne->object);
            s = tmp_buf;
        }
        l1 = strlen(s);

        type = ne->value->type;
        num = ne->value->length;
        if (num > NAME_ONELINE_MAX) {
            ERR_raise(ERR_LIB_X509, X509_R_NAME_TOO_LONG);
            goto end;
        }
        q = ne->value->data;
#ifdef CHARSET_EBCDIC
        if (type == V_ASN1_GENERALSTRING ||
            type == V_ASN1_VISIBLESTRING ||
            type == V_ASN1_PRINTABLESTRING ||
            type == V_ASN1_TELETEXSTRING ||
            type == V_ASN1_IA5STRING) {
            if (num > (int)sizeof(ebcdic_buf))
                num = sizeof(ebcdic_buf);
            ascii2ebcdic(ebcdic_buf, q, num);
            q = ebcdic_buf;
        }
#endif

        if ((type == V_ASN1_GENERALSTRING) && ((num % 4) == 0)) {
            gs_doit[0] = gs_doit[1] = gs_doit[2] = gs_doit[3] = 0;
            for (j = 0; j < num; j++)
                if (q[j] != 0)
                    gs_doit[j & 3] = 1;

            if (gs_doit[0] | gs_doit[1] | gs_doit[2])
                gs_doit[0] = gs_doit[1] = gs_doit[2] = gs_doit[3] = 1;
            else {
                gs_doit[0] = gs_doit[1] = gs_doit[2] = 0;
                gs_doit[3] = 1;
            }
        } else
            gs_doit[0] = gs_doit[1] = gs_doit[2] = gs_doit[3] = 1;

        for (l2 = j = 0; j < num; j++) {
            if (!gs_doit[j & 3])
                continue;
            l2++;
            if (q[j] == '/' || q[j] == '+')
                l2++; /* char needs to be escaped */
            else if ((ossl_toascii(q[j]) < ossl_toascii(' ')) ||
                     (ossl_toascii(q[j]) > ossl_toascii('~')))
                l2 += 3;
        }

        lold = l;
        l += 1 + l1 + 1 + l2;
        if (l > NAME_ONELINE_MAX) {
            ERR_raise(ERR_LIB_X509, X509_R_NAME_TOO_LONG);
            goto end;
        }
        if (b != NULL) {
            if (!BUF_MEM_grow(b, l + 1))
                goto buferr;
            p = &(b->data[lold]);
        } else if (l > len) {
            break;
        } else
            p = &(buf[lold]);
        *(p++) = prev_set == ne->set ? '+' : '/';
        memcpy(p, s, (unsigned int)l1);
        p += l1;
        *(p++) = '=';

#ifndef CHARSET_EBCDIC          /* q was assigned above already. */
        q = ne->value->data;
#endif

        for (j = 0; j < num; j++) {
            if (!gs_doit[j & 3])
                continue;
#ifndef CHARSET_EBCDIC
            n = q[j];
            if ((n < ' ') || (n > '~')) {
                *(p++) = '\\';
                *(p++) = 'x';
                p += ossl_to_hex(p, n);
            } else {
                if (n == '/' || n == '+')
                    *(p++) = '\\';
                *(p++) = n;
            }
#else
            n = os_toascii[q[j]];
            if ((n < os_toascii[' ']) || (n > os_toascii['~'])) {
                *(p++) = '\\';
                *(p++) = 'x';
                p += ossl_to_hex(p, n);
            } else {
                if (n == os_toascii['/'] || n == os_toascii['+'])
                    *(p++) = '\\';
                *(p++) = q[j];
            }
#endif
        }
        *p = '\0';
        prev_set = ne->set;
    }
    if (b != NULL) {
        p = b->data;
        OPENSSL_free(b);
    } else
        p = buf;
    if (i == 0)
        *p = '\0';
    return p;
 buferr:
    ERR_raise(ERR_LIB_X509, ERR_R_BUF_LIB);
 end:
    BUF_MEM_free(b);
    return NULL;
}
