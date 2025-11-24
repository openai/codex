/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/objects.h>
#include <openssl/asn1.h>

#ifndef ASN1_PARSE_MAXDEPTH
#define ASN1_PARSE_MAXDEPTH 128
#endif

static int asn1_parse2(BIO *bp, const unsigned char **pp, long length,
                       int offset, int depth, int indent, int dump);
static int asn1_print_info(BIO *bp, long offset, int depth, int hl, long len,
                           int tag, int xclass, int constructed, int indent)
{
    char str[128];
    const char *p;
    int pop_f_prefix = 0;
    long saved_indent = -1;
    int i = 0;
    BIO *bio = NULL;

    if (constructed & V_ASN1_CONSTRUCTED)
        p = "cons: ";
    else
        p = "prim: ";
    if (constructed != (V_ASN1_CONSTRUCTED | 1)) {
        if (BIO_snprintf(str, sizeof(str), "%5ld:d=%-2d hl=%ld l=%4ld %s",
                         offset, depth, (long)hl, len, p) <= 0)
            goto err;
    } else {
        if (BIO_snprintf(str, sizeof(str), "%5ld:d=%-2d hl=%ld l=inf  %s",
                         offset, depth, (long)hl, p) <= 0)
            goto err;
    }
    if (bp != NULL) {
        if (BIO_set_prefix(bp, str) <= 0) {
            if ((bio = BIO_new(BIO_f_prefix())) == NULL
                    || (bp = BIO_push(bio, bp)) == NULL)
                goto err;
            pop_f_prefix = 1;
        }
        saved_indent = BIO_get_indent(bp);
        if (BIO_set_prefix(bp, str) <= 0 || BIO_set_indent(bp, indent) <= 0)
            goto err;
    }

    /*
     * BIO_set_prefix made a copy of |str|, so we can safely use it for
     * something else, ASN.1 tag printout.
     */
    p = str;
    if ((xclass & V_ASN1_PRIVATE) == V_ASN1_PRIVATE)
        BIO_snprintf(str, sizeof(str), "priv [ %d ] ", tag);
    else if ((xclass & V_ASN1_CONTEXT_SPECIFIC) == V_ASN1_CONTEXT_SPECIFIC)
        BIO_snprintf(str, sizeof(str), "cont [ %d ]", tag);
    else if ((xclass & V_ASN1_APPLICATION) == V_ASN1_APPLICATION)
        BIO_snprintf(str, sizeof(str), "appl [ %d ]", tag);
    else if (tag > 30)
        BIO_snprintf(str, sizeof(str), "<ASN1 %d>", tag);
    else
        p = ASN1_tag2str(tag);

    i = (BIO_printf(bp, "%-18s", p) > 0);
 err:
    if (saved_indent >= 0)
        BIO_set_indent(bp, saved_indent);
    if (pop_f_prefix)
        BIO_pop(bp);
    BIO_free(bio);
    return i;
}

int ASN1_parse(BIO *bp, const unsigned char *pp, long len, int indent)
{
    return asn1_parse2(bp, &pp, len, 0, 0, indent, 0);
}

int ASN1_parse_dump(BIO *bp, const unsigned char *pp, long len, int indent,
                    int dump)
{
    return asn1_parse2(bp, &pp, len, 0, 0, indent, dump);
}

static int asn1_parse2(BIO *bp, const unsigned char **pp, long length,
                       int offset, int depth, int indent, int dump)
{
    const unsigned char *p, *ep, *tot, *op, *opp;
    long len;
    int tag, xclass, ret = 0;
    int nl, hl, j, r;
    ASN1_OBJECT *o = NULL;
    ASN1_OCTET_STRING *os = NULL;
    ASN1_INTEGER *ai = NULL;
    ASN1_ENUMERATED *ae = NULL;
    /* ASN1_BMPSTRING *bmp=NULL; */
    int dump_indent, dump_cont = 0;

    if (depth > ASN1_PARSE_MAXDEPTH) {
        BIO_puts(bp, "BAD RECURSION DEPTH\n");
        return 0;
    }

    dump_indent = 6;            /* Because we know BIO_dump_indent() */
    p = *pp;
    tot = p + length;
    while (length > 0) {
        op = p;
        j = ASN1_get_object(&p, &len, &tag, &xclass, length);
        if (j & 0x80) {
            BIO_puts(bp, "Error in encoding\n");
            goto end;
        }
        hl = (p - op);
        length -= hl;
        /*
         * if j == 0x21 it is a constructed indefinite length object
         */
        if (!asn1_print_info(bp, (long)offset + (long)(op - *pp), depth,
                             hl, len, tag, xclass, j, (indent) ? depth : 0))
            goto end;
        if (j & V_ASN1_CONSTRUCTED) {
            const unsigned char *sp = p;

            ep = p + len;
            if (BIO_write(bp, "\n", 1) <= 0)
                goto end;
            if (len > length) {
                BIO_printf(bp, "length is greater than %ld\n", length);
                goto end;
            }
            if ((j == 0x21) && (len == 0)) {
                for (;;) {
                    r = asn1_parse2(bp, &p, (long)(tot - p),
                                    offset + (p - *pp), depth + 1,
                                    indent, dump);
                    if (r == 0)
                        goto end;
                    if ((r == 2) || (p >= tot)) {
                        len = p - sp;
                        break;
                    }
                }
            } else {
                long tmp = len;

                while (p < ep) {
                    sp = p;
                    r = asn1_parse2(bp, &p, tmp,
                                    offset + (p - *pp), depth + 1,
                                    indent, dump);
                    if (r == 0)
                        goto end;
                    tmp -= p - sp;
                }
            }
        } else if (xclass != 0) {
            p += len;
            if (BIO_write(bp, "\n", 1) <= 0)
                goto end;
        } else {
            nl = 0;
            if ((tag == V_ASN1_PRINTABLESTRING) ||
                (tag == V_ASN1_T61STRING) ||
                (tag == V_ASN1_IA5STRING) ||
                (tag == V_ASN1_VISIBLESTRING) ||
                (tag == V_ASN1_NUMERICSTRING) ||
                (tag == V_ASN1_UTF8STRING) ||
                (tag == V_ASN1_UTCTIME) || (tag == V_ASN1_GENERALIZEDTIME)) {
                if (BIO_write(bp, ":", 1) <= 0)
                    goto end;
                if ((len > 0) && BIO_write(bp, (const char *)p, (int)len)
                    != (int)len)
                    goto end;
            } else if (tag == V_ASN1_OBJECT) {
                opp = op;
                if (d2i_ASN1_OBJECT(&o, &opp, len + hl) != NULL) {
                    if (BIO_write(bp, ":", 1) <= 0)
                        goto end;
                    i2a_ASN1_OBJECT(bp, o);
                } else {
                    if (BIO_puts(bp, ":BAD OBJECT") <= 0)
                        goto end;
                    dump_cont = 1;
                }
            } else if (tag == V_ASN1_BOOLEAN) {
                if (len != 1) {
                    if (BIO_puts(bp, ":BAD BOOLEAN") <= 0)
                        goto end;
                    dump_cont = 1;
                }
                if (len > 0)
                    BIO_printf(bp, ":%u", p[0]);
            } else if (tag == V_ASN1_BMPSTRING) {
                /* do the BMP thang */
            } else if (tag == V_ASN1_OCTET_STRING) {
                int i, printable = 1;

                opp = op;
                os = d2i_ASN1_OCTET_STRING(NULL, &opp, len + hl);
                if (os != NULL && os->length > 0) {
                    opp = os->data;
                    /*
                     * testing whether the octet string is printable
                     */
                    for (i = 0; i < os->length; i++) {
                        if (((opp[i] < ' ') &&
                             (opp[i] != '\n') &&
                             (opp[i] != '\r') &&
                             (opp[i] != '\t')) || (opp[i] > '~')) {
                            printable = 0;
                            break;
                        }
                    }
                    if (printable)
                        /* printable string */
                    {
                        if (BIO_write(bp, ":", 1) <= 0)
                            goto end;
                        if (BIO_write(bp, (const char *)opp, os->length) <= 0)
                            goto end;
                    } else if (!dump)
                        /*
                         * not printable => print octet string as hex dump
                         */
                    {
                        if (BIO_write(bp, "[HEX DUMP]:", 11) <= 0)
                            goto end;
                        for (i = 0; i < os->length; i++) {
                            if (BIO_printf(bp, "%02X", opp[i]) <= 0)
                                goto end;
                        }
                    } else
                        /* print the normal dump */
                    {
                        if (!nl) {
                            if (BIO_write(bp, "\n", 1) <= 0)
                                goto end;
                        }
                        if (BIO_dump_indent(bp,
                                            (const char *)opp,
                                            ((dump == -1 || dump >
                                              os->
                                              length) ? os->length : dump),
                                            dump_indent) <= 0)
                            goto end;
                        nl = 1;
                    }
                }
                ASN1_OCTET_STRING_free(os);
                os = NULL;
            } else if (tag == V_ASN1_INTEGER) {
                int i;

                opp = op;
                ai = d2i_ASN1_INTEGER(NULL, &opp, len + hl);
                if (ai != NULL) {
                    if (BIO_write(bp, ":", 1) <= 0)
                        goto end;
                    if (ai->type == V_ASN1_NEG_INTEGER)
                        if (BIO_write(bp, "-", 1) <= 0)
                            goto end;
                    for (i = 0; i < ai->length; i++) {
                        if (BIO_printf(bp, "%02X", ai->data[i]) <= 0)
                            goto end;
                    }
                    if (ai->length == 0) {
                        if (BIO_write(bp, "00", 2) <= 0)
                            goto end;
                    }
                } else {
                    if (BIO_puts(bp, ":BAD INTEGER") <= 0)
                        goto end;
                    dump_cont = 1;
                }
                ASN1_INTEGER_free(ai);
                ai = NULL;
            } else if (tag == V_ASN1_ENUMERATED) {
                int i;

                opp = op;
                ae = d2i_ASN1_ENUMERATED(NULL, &opp, len + hl);
                if (ae != NULL) {
                    if (BIO_write(bp, ":", 1) <= 0)
                        goto end;
                    if (ae->type == V_ASN1_NEG_ENUMERATED)
                        if (BIO_write(bp, "-", 1) <= 0)
                            goto end;
                    for (i = 0; i < ae->length; i++) {
                        if (BIO_printf(bp, "%02X", ae->data[i]) <= 0)
                            goto end;
                    }
                    if (ae->length == 0) {
                        if (BIO_write(bp, "00", 2) <= 0)
                            goto end;
                    }
                } else {
                    if (BIO_puts(bp, ":BAD ENUMERATED") <= 0)
                        goto end;
                    dump_cont = 1;
                }
                ASN1_ENUMERATED_free(ae);
                ae = NULL;
            } else if (len > 0 && dump) {
                if (!nl) {
                    if (BIO_write(bp, "\n", 1) <= 0)
                        goto end;
                }
                if (BIO_dump_indent(bp, (const char *)p,
                                    ((dump == -1 || dump > len) ? len : dump),
                                    dump_indent) <= 0)
                    goto end;
                nl = 1;
            }
            if (dump_cont) {
                int i;
                const unsigned char *tmp = op + hl;
                if (BIO_puts(bp, ":[") <= 0)
                    goto end;
                for (i = 0; i < len; i++) {
                    if (BIO_printf(bp, "%02X", tmp[i]) <= 0)
                        goto end;
                }
                if (BIO_puts(bp, "]") <= 0)
                    goto end;
                dump_cont = 0;
            }

            if (!nl) {
                if (BIO_write(bp, "\n", 1) <= 0)
                    goto end;
            }
            p += len;
            if ((tag == V_ASN1_EOC) && (xclass == 0)) {
                ret = 2;        /* End of sequence */
                goto end;
            }
        }
        length -= len;
    }
    ret = 1;
 end:
    ASN1_OBJECT_free(o);
    ASN1_OCTET_STRING_free(os);
    ASN1_INTEGER_free(ai);
    ASN1_ENUMERATED_free(ae);
    *pp = p;
    return ret;
}

const char *ASN1_tag2str(int tag)
{
    static const char *const tag2str[] = {
        /* 0-4 */
        "EOC", "BOOLEAN", "INTEGER", "BIT STRING", "OCTET STRING",
        /* 5-9 */
        "NULL", "OBJECT", "OBJECT DESCRIPTOR", "EXTERNAL", "REAL",
        /* 10-13 */
        "ENUMERATED", "<ASN1 11>", "UTF8STRING", "<ASN1 13>",
        /* 15-17 */
        "<ASN1 14>", "<ASN1 15>", "SEQUENCE", "SET",
        /* 18-20 */
        "NUMERICSTRING", "PRINTABLESTRING", "T61STRING",
        /* 21-24 */
        "VIDEOTEXSTRING", "IA5STRING", "UTCTIME", "GENERALIZEDTIME",
        /* 25-27 */
        "GRAPHICSTRING", "VISIBLESTRING", "GENERALSTRING",
        /* 28-30 */
        "UNIVERSALSTRING", "<ASN1 29>", "BMPSTRING"
    };

    if ((tag == V_ASN1_NEG_INTEGER) || (tag == V_ASN1_NEG_ENUMERATED))
        tag &= ~0x100;

    if (tag < 0 || tag > 30)
        return "(unknown)";
    return tag2str[tag];
}
