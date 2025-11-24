/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <openssl/err.h>
#include "internal/propertyerr.h"
#include "internal/property.h"
#include "internal/numbers.h"
#include "crypto/ctype.h"
#include "internal/nelem.h"
#include "property_local.h"
#include "internal/e_os.h"

DEFINE_STACK_OF(OSSL_PROPERTY_DEFINITION)

static const char *skip_space(const char *s)
{
    while (ossl_isspace(*s))
        s++;
    return s;
}

static int match_ch(const char *t[], char m)
{
    const char *s = *t;

    if (*s == m) {
        *t = skip_space(s + 1);
        return 1;
    }
    return 0;
}

#define MATCH(s, m) match(s, m, sizeof(m) - 1)

static int match(const char *t[], const char m[], size_t m_len)
{
    const char *s = *t;

    if (OPENSSL_strncasecmp(s, m, m_len) == 0) {
        *t = skip_space(s + m_len);
        return 1;
    }
    return 0;
}

static int parse_name(OSSL_LIB_CTX *ctx, const char *t[], int create,
                      OSSL_PROPERTY_IDX *idx)
{
    char name[100];
    int err = 0;
    size_t i = 0;
    const char *s = *t;
    int user_name = 0;

    for (;;) {
        if (!ossl_isalpha(*s)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_IDENTIFIER,
                           "HERE-->%s", *t);
            return 0;
        }
        do {
            if (i < sizeof(name) - 1)
                name[i++] = ossl_tolower(*s);
            else
                err = 1;
        } while (*++s == '_' || ossl_isalnum(*s));
        if (*s != '.')
            break;
        user_name = 1;
        if (i < sizeof(name) - 1)
            name[i++] = *s;
        else
            err = 1;
        s++;
    }
    name[i] = '\0';
    if (err) {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NAME_TOO_LONG, "HERE-->%s", *t);
        return 0;
    }
    *t = skip_space(s);
    *idx = ossl_property_name(ctx, name, user_name && create);
    return 1;
}

static int parse_number(const char *t[], OSSL_PROPERTY_DEFINITION *res)
{
    const char *s = *t;
    int64_t v = 0;

    do {
        if (!ossl_isdigit(*s)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_A_DECIMAL_DIGIT,
                           "HERE-->%s", *t);
            return 0;
        }
        /* overflow check */
        if (v > ((INT64_MAX - (*s - '0')) / 10)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_PARSE_FAILED,
                           "Property %s overflows", *t);
            return 0;
        }
        v = v * 10 + (*s++ - '0');
    } while (ossl_isdigit(*s));
    if (!ossl_isspace(*s) && *s != '\0' && *s != ',') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_A_DECIMAL_DIGIT,
                       "HERE-->%s", *t);
        return 0;
    }
    *t = skip_space(s);
    res->type = OSSL_PROPERTY_TYPE_NUMBER;
    res->v.int_val = v;
    return 1;
}

static int parse_hex(const char *t[], OSSL_PROPERTY_DEFINITION *res)
{
    const char *s = *t;
    int64_t v = 0;
    int sval;

    do {
        if (ossl_isdigit(*s)) {
            sval = *s - '0';
        } else if (ossl_isxdigit(*s)) {
            sval = ossl_tolower(*s) - 'a' + 10;
        } else {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_HEXADECIMAL_DIGIT,
                           "%s", *t);
            return 0;
        }

        if (v > ((INT64_MAX - sval) / 16)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_PARSE_FAILED,
                           "Property %s overflows", *t);
            return 0;
        }

        v <<= 4;
        v += sval;
    } while (ossl_isxdigit(*++s));
    if (!ossl_isspace(*s) && *s != '\0' && *s != ',') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_HEXADECIMAL_DIGIT,
                       "HERE-->%s", *t);
        return 0;
    }
    *t = skip_space(s);
    res->type = OSSL_PROPERTY_TYPE_NUMBER;
    res->v.int_val = v;
    return 1;
}

static int parse_oct(const char *t[], OSSL_PROPERTY_DEFINITION *res)
{
    const char *s = *t;
    int64_t v = 0;

    do {
        if (*s == '9' || *s == '8' || !ossl_isdigit(*s)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_OCTAL_DIGIT,
                           "HERE-->%s", *t);
            return 0;
        }
        if (v > ((INT64_MAX - (*s - '0')) / 8)) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_PARSE_FAILED,
                           "Property %s overflows", *t);
            return 0;
        }

        v = (v << 3) + (*s - '0');
    } while (ossl_isdigit(*++s) && *s != '9' && *s != '8');
    if (!ossl_isspace(*s) && *s != '\0' && *s != ',') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_OCTAL_DIGIT,
                       "HERE-->%s", *t);
        return 0;
    }
    *t = skip_space(s);
    res->type = OSSL_PROPERTY_TYPE_NUMBER;
    res->v.int_val = v;
    return 1;
}

static int parse_string(OSSL_LIB_CTX *ctx, const char *t[], char delim,
                        OSSL_PROPERTY_DEFINITION *res, const int create)
{
    char v[1000];
    const char *s = *t;
    size_t i = 0;
    int err = 0;

    while (*s != '\0' && *s != delim) {
        if (i < sizeof(v) - 1)
            v[i++] = *s;
        else
            err = 1;
        s++;
    }
    if (*s == '\0') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NO_MATCHING_STRING_DELIMITER,
                       "HERE-->%c%s", delim, *t);
        return 0;
    }
    v[i] = '\0';
    if (err) {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_STRING_TOO_LONG, "HERE-->%s", *t);
    } else {
        res->v.str_val = ossl_property_value(ctx, v, create);
    }
    *t = skip_space(s + 1);
    res->type = OSSL_PROPERTY_TYPE_STRING;
    return !err;
}

static int parse_unquoted(OSSL_LIB_CTX *ctx, const char *t[],
                          OSSL_PROPERTY_DEFINITION *res, const int create)
{
    char v[1000];
    const char *s = *t;
    size_t i = 0;
    int err = 0;

    if (*s == '\0' || *s == ',')
        return 0;
    while (ossl_isprint(*s) && !ossl_isspace(*s) && *s != ',') {
        if (i < sizeof(v) - 1)
            v[i++] = ossl_tolower(*s);
        else
            err = 1;
        s++;
    }
    if (!ossl_isspace(*s) && *s != '\0' && *s != ',') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_NOT_AN_ASCII_CHARACTER,
                       "HERE-->%s", s);
        return 0;
    }
    v[i] = 0;
    if (err)
        ERR_raise_data(ERR_LIB_PROP, PROP_R_STRING_TOO_LONG, "HERE-->%s", *t);
    else if ((res->v.str_val = ossl_property_value(ctx, v, create)) == 0)
        err = 1;
    *t = skip_space(s);
    res->type = OSSL_PROPERTY_TYPE_STRING;
    return !err;
}

static int parse_value(OSSL_LIB_CTX *ctx, const char *t[],
                       OSSL_PROPERTY_DEFINITION *res, int create)
{
    const char *s = *t;
    int r = 0;

    if (*s == '"' || *s == '\'') {
        s++;
        r = parse_string(ctx, &s, s[-1], res, create);
    } else if (*s == '+') {
        s++;
        r = parse_number(&s, res);
    } else if (*s == '-') {
        s++;
        r = parse_number(&s, res);
        res->v.int_val = -res->v.int_val;
    } else if (*s == '0' && s[1] == 'x') {
        s += 2;
        r = parse_hex(&s, res);
    } else if (*s == '0' && ossl_isdigit(s[1])) {
        s++;
        r = parse_oct(&s, res);
    } else if (ossl_isdigit(*s)) {
        return parse_number(t, res);
    } else if (ossl_isalpha(*s))
        return parse_unquoted(ctx, t, res, create);
    if (r)
        *t = s;
    return r;
}

static int pd_compare(const OSSL_PROPERTY_DEFINITION *const *p1,
                      const OSSL_PROPERTY_DEFINITION *const *p2)
{
    const OSSL_PROPERTY_DEFINITION *pd1 = *p1;
    const OSSL_PROPERTY_DEFINITION *pd2 = *p2;

    if (pd1->name_idx < pd2->name_idx)
        return -1;
    if (pd1->name_idx > pd2->name_idx)
        return 1;
    return 0;
}

static void pd_free(OSSL_PROPERTY_DEFINITION *pd)
{
    OPENSSL_free(pd);
}

/*
 * Convert a stack of property definitions and queries into a fixed array.
 * The items are sorted for efficient query.  The stack is not freed.
 * This function also checks for duplicated names and returns an error if
 * any exist.
 */
static OSSL_PROPERTY_LIST *
stack_to_property_list(OSSL_LIB_CTX *ctx,
                       STACK_OF(OSSL_PROPERTY_DEFINITION) *sk)
{
    const int n = sk_OSSL_PROPERTY_DEFINITION_num(sk);
    OSSL_PROPERTY_LIST *r;
    OSSL_PROPERTY_IDX prev_name_idx = 0;
    int i;

    r = OPENSSL_malloc(sizeof(*r)
                       + (n <= 0 ? 0 : n - 1) * sizeof(r->properties[0]));
    if (r != NULL) {
        sk_OSSL_PROPERTY_DEFINITION_sort(sk);

        r->has_optional = 0;
        for (i = 0; i < n; i++) {
            r->properties[i] = *sk_OSSL_PROPERTY_DEFINITION_value(sk, i);
            r->has_optional |= r->properties[i].optional;

            /* Check for duplicated names */
            if (i > 0 && r->properties[i].name_idx == prev_name_idx) {
                OPENSSL_free(r);
                ERR_raise_data(ERR_LIB_PROP, PROP_R_PARSE_FAILED,
                               "Duplicated name `%s'",
                               ossl_property_name_str(ctx, prev_name_idx));
                return NULL;
            }
            prev_name_idx = r->properties[i].name_idx;
        }
        r->num_properties = n;
    }
    return r;
}

OSSL_PROPERTY_LIST *ossl_parse_property(OSSL_LIB_CTX *ctx, const char *defn)
{
    OSSL_PROPERTY_DEFINITION *prop = NULL;
    OSSL_PROPERTY_LIST *res = NULL;
    STACK_OF(OSSL_PROPERTY_DEFINITION) *sk;
    const char *s = defn;
    int done;

    if (s == NULL || (sk = sk_OSSL_PROPERTY_DEFINITION_new(&pd_compare)) == NULL)
        return NULL;

    s = skip_space(s);
    done = *s == '\0';
    while (!done) {
        const char *start = s;

        prop = OPENSSL_malloc(sizeof(*prop));
        if (prop == NULL)
            goto err;
        memset(&prop->v, 0, sizeof(prop->v));
        prop->optional = 0;
        if (!parse_name(ctx, &s, 1, &prop->name_idx))
            goto err;
        prop->oper = OSSL_PROPERTY_OPER_EQ;
        if (prop->name_idx == 0) {
            ERR_raise_data(ERR_LIB_PROP, PROP_R_PARSE_FAILED,
                           "Unknown name HERE-->%s", start);
            goto err;
        }
        if (match_ch(&s, '=')) {
            if (!parse_value(ctx, &s, prop, 1)) {
                ERR_raise_data(ERR_LIB_PROP, PROP_R_NO_VALUE,
                               "HERE-->%s", start);
                goto err;
            }
        } else {
            /* A name alone means a true Boolean */
            prop->type = OSSL_PROPERTY_TYPE_STRING;
            prop->v.str_val = OSSL_PROPERTY_TRUE;
        }

        if (!sk_OSSL_PROPERTY_DEFINITION_push(sk, prop))
            goto err;
        prop = NULL;
        done = !match_ch(&s, ',');
    }
    if (*s != '\0') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_TRAILING_CHARACTERS,
                       "HERE-->%s", s);
        goto err;
    }
    res = stack_to_property_list(ctx, sk);

err:
    OPENSSL_free(prop);
    sk_OSSL_PROPERTY_DEFINITION_pop_free(sk, &pd_free);
    return res;
}

OSSL_PROPERTY_LIST *ossl_parse_query(OSSL_LIB_CTX *ctx, const char *s,
                                     int create_values)
{
    STACK_OF(OSSL_PROPERTY_DEFINITION) *sk;
    OSSL_PROPERTY_LIST *res = NULL;
    OSSL_PROPERTY_DEFINITION *prop = NULL;
    int done;

    if (s == NULL || (sk = sk_OSSL_PROPERTY_DEFINITION_new(&pd_compare)) == NULL)
        return NULL;

    s = skip_space(s);
    done = *s == '\0';
    while (!done) {
        prop = OPENSSL_malloc(sizeof(*prop));
        if (prop == NULL)
            goto err;
        memset(&prop->v, 0, sizeof(prop->v));

        if (match_ch(&s, '-')) {
            prop->oper = OSSL_PROPERTY_OVERRIDE;
            prop->optional = 0;
            if (!parse_name(ctx, &s, 1, &prop->name_idx))
                goto err;
            goto skip_value;
        }
        prop->optional = match_ch(&s, '?');
        if (!parse_name(ctx, &s, 1, &prop->name_idx))
            goto err;

        if (match_ch(&s, '=')) {
            prop->oper = OSSL_PROPERTY_OPER_EQ;
        } else if (MATCH(&s, "!=")) {
            prop->oper = OSSL_PROPERTY_OPER_NE;
        } else {
            /* A name alone is a Boolean comparison for true */
            prop->oper = OSSL_PROPERTY_OPER_EQ;
            prop->type = OSSL_PROPERTY_TYPE_STRING;
            prop->v.str_val = OSSL_PROPERTY_TRUE;
            goto skip_value;
        }
        if (!parse_value(ctx, &s, prop, create_values))
            prop->type = OSSL_PROPERTY_TYPE_VALUE_UNDEFINED;

skip_value:
        if (!sk_OSSL_PROPERTY_DEFINITION_push(sk, prop))
            goto err;
        prop = NULL;
        done = !match_ch(&s, ',');
    }
    if (*s != '\0') {
        ERR_raise_data(ERR_LIB_PROP, PROP_R_TRAILING_CHARACTERS,
                       "HERE-->%s", s);
        goto err;
    }
    res = stack_to_property_list(ctx, sk);

err:
    OPENSSL_free(prop);
    sk_OSSL_PROPERTY_DEFINITION_pop_free(sk, &pd_free);
    return res;
}

/*
 * Compare a query against a definition.
 * Return the number of clauses matched or -1 if a mandatory clause is false.
 */
int ossl_property_match_count(const OSSL_PROPERTY_LIST *query,
                              const OSSL_PROPERTY_LIST *defn)
{
    const OSSL_PROPERTY_DEFINITION *const q = query->properties;
    const OSSL_PROPERTY_DEFINITION *const d = defn->properties;
    int i = 0, j = 0, matches = 0;
    OSSL_PROPERTY_OPER oper;

    while (i < query->num_properties) {
        if ((oper = q[i].oper) == OSSL_PROPERTY_OVERRIDE) {
            i++;
            continue;
        }
        if (j < defn->num_properties) {
            if (q[i].name_idx > d[j].name_idx) {  /* skip defn, not in query */
                j++;
                continue;
            }
            if (q[i].name_idx == d[j].name_idx) { /* both in defn and query */
                const int eq = q[i].type == d[j].type
                               && memcmp(&q[i].v, &d[j].v, sizeof(q[i].v)) == 0;

                if ((eq && oper == OSSL_PROPERTY_OPER_EQ)
                    || (!eq && oper == OSSL_PROPERTY_OPER_NE))
                    matches++;
                else if (!q[i].optional)
                    return -1;
                i++;
                j++;
                continue;
            }
        }

        /*
         * Handle the cases of a missing value and a query with no corresponding
         * definition.  The former fails for any comparison except inequality,
         * the latter is treated as a comparison against the Boolean false.
         */
        if (q[i].type == OSSL_PROPERTY_TYPE_VALUE_UNDEFINED) {
            if (oper == OSSL_PROPERTY_OPER_NE)
                matches++;
            else if (!q[i].optional)
                return -1;
        } else if (q[i].type != OSSL_PROPERTY_TYPE_STRING
                   || (oper == OSSL_PROPERTY_OPER_EQ
                       && q[i].v.str_val != OSSL_PROPERTY_FALSE)
                   || (oper == OSSL_PROPERTY_OPER_NE
                       && q[i].v.str_val == OSSL_PROPERTY_FALSE)) {
            if (!q[i].optional)
                return -1;
        } else {
            matches++;
        }
        i++;
    }
    return matches;
}

void ossl_property_free(OSSL_PROPERTY_LIST *p)
{
    OPENSSL_free(p);
}

/*
 * Merge two property lists.
 * If there is a common name, the one from the first list is used.
 */
OSSL_PROPERTY_LIST *ossl_property_merge(const OSSL_PROPERTY_LIST *a,
                                        const OSSL_PROPERTY_LIST *b)
{
    const OSSL_PROPERTY_DEFINITION *const ap = a->properties;
    const OSSL_PROPERTY_DEFINITION *const bp = b->properties;
    const OSSL_PROPERTY_DEFINITION *copy;
    OSSL_PROPERTY_LIST *r;
    int i, j, n;
    const int t = a->num_properties + b->num_properties;

    r = OPENSSL_malloc(sizeof(*r)
                       + (t == 0 ? 0 : t - 1) * sizeof(r->properties[0]));
    if (r == NULL)
        return NULL;

    r->has_optional = 0;
    for (i = j = n = 0; i < a->num_properties || j < b->num_properties; n++) {
        if (i >= a->num_properties) {
            copy = &bp[j++];
        } else if (j >= b->num_properties) {
            copy = &ap[i++];
        } else if (ap[i].name_idx <= bp[j].name_idx) {
            if (ap[i].name_idx == bp[j].name_idx)
                j++;
            copy = &ap[i++];
        } else {
            copy = &bp[j++];
        }
        memcpy(r->properties + n, copy, sizeof(r->properties[0]));
        r->has_optional |= copy->optional;
    }
    r->num_properties = n;

    return r;
}

int ossl_property_parse_init(OSSL_LIB_CTX *ctx)
{
    static const char *const predefined_names[] = {
        "provider",     /* Name of provider (default, legacy, fips) */
        "version",      /* Version number of this provider */
        "fips",         /* FIPS validated or FIPS supporting algorithm */
        "output",       /* Output type for encoders */
        "input",        /* Input type for decoders */
        "structure",    /* Structure name for encoders and decoders */
    };
    size_t i;

    for (i = 0; i < OSSL_NELEM(predefined_names); i++)
        if (ossl_property_name(ctx, predefined_names[i], 1) == 0)
            goto err;

    /*
     * Pre-populate the two Boolean values. We must do them before any other
     * values and in this order so that we get the same index as the global
     * OSSL_PROPERTY_TRUE and OSSL_PROPERTY_FALSE values
     */
    if ((ossl_property_value(ctx, "yes", 1) != OSSL_PROPERTY_TRUE)
        || (ossl_property_value(ctx, "no", 1) != OSSL_PROPERTY_FALSE))
        goto err;

    return 1;
err:
    return 0;
}

static void put_char(char ch, char **buf, size_t *remain, size_t *needed)
{
    if (*remain == 0) {
        ++*needed;
        return;
    }
    if (*remain == 1)
        **buf = '\0';
    else
        **buf = ch;
    ++*buf;
    ++*needed;
    --*remain;
}

static void put_str(const char *str, char **buf, size_t *remain, size_t *needed)
{
    size_t olen, len, i;
    char quote = '\0';
    int quotes;

    len = olen = strlen(str);
    *needed += len;

    /*
     * Check to see if we need quotes or not.
     * Characters that are legal in a PropertyName don't need quoting.
     * We simply assume all others require quotes.
     */
    for (i = 0; i < len; i++)
        if (!ossl_isalnum(str[i]) && str[i] != '.' && str[i] != '_') {
            /* Default to single quotes ... */
            if (quote == '\0')
                quote = '\'';
            /* ... but use double quotes if a single is present */
            if (str[i] == '\'')
                quote = '"';
        }

    quotes = quote != '\0';
    if (*remain == 0) {
        *needed += 2 * quotes;
        return;
    }

    if (quotes)
        put_char(quote, buf, remain, needed);

    if (*remain < len + 1 + quotes)
        len = *remain - 1;

    if (len > 0) {
        memcpy(*buf, str, len);
        *buf += len;
        *remain -= len;
    }

    if (quotes)
        put_char(quote, buf, remain, needed);

    if (len < olen && *remain == 1) {
        **buf = '\0';
        ++*buf;
        --*remain;
    }
}

static void put_num(int64_t val, char **buf, size_t *remain, size_t *needed)
{
    int64_t tmpval = val;
    size_t len = 1;

    if (tmpval < 0) {
        len++;
        tmpval = -tmpval;
    }
    for (; tmpval > 9; len++, tmpval /= 10);

    *needed += len;

    if (*remain == 0)
        return;

    BIO_snprintf(*buf, *remain, "%lld", (long long int)val);
    if (*remain < len) {
        *buf += *remain;
        *remain = 0;
    } else {
        *buf += len;
        *remain -= len;
    }
}

size_t ossl_property_list_to_string(OSSL_LIB_CTX *ctx,
                                    const OSSL_PROPERTY_LIST *list, char *buf,
                                    size_t bufsize)
{
    int i;
    const OSSL_PROPERTY_DEFINITION *prop = NULL;
    size_t needed = 0;
    const char *val;

    if (list == NULL) {
        if (bufsize > 0)
            *buf = '\0';
        return 1;
    }
    if (list->num_properties != 0)
        prop = &list->properties[list->num_properties - 1];
    for (i = 0; i < list->num_properties; i++, prop--) {
        /* Skip invalid names */
        if (prop->name_idx == 0)
            continue;

        if (needed > 0)
            put_char(',', &buf, &bufsize, &needed);

        if (prop->optional)
            put_char('?', &buf, &bufsize, &needed);
        else if (prop->oper == OSSL_PROPERTY_OVERRIDE)
            put_char('-', &buf, &bufsize, &needed);

        val = ossl_property_name_str(ctx, prop->name_idx);
        if (val == NULL)
            return 0;
        put_str(val, &buf, &bufsize, &needed);

        switch (prop->oper) {
            case OSSL_PROPERTY_OPER_NE:
                put_char('!', &buf, &bufsize, &needed);
                /* fall through */
            case OSSL_PROPERTY_OPER_EQ:
                put_char('=', &buf, &bufsize, &needed);
                /* put value */
                switch (prop->type) {
                case OSSL_PROPERTY_TYPE_STRING:
                    val = ossl_property_value_str(ctx, prop->v.str_val);
                    if (val == NULL)
                        return 0;
                    put_str(val, &buf, &bufsize, &needed);
                    break;

                case OSSL_PROPERTY_TYPE_NUMBER:
                    put_num(prop->v.int_val, &buf, &bufsize, &needed);
                    break;

                default:
                    return 0;
                }
                break;
            default:
                /* do nothing */
                break;
        }
    }

    put_char('\0', &buf, &bufsize, &needed);
    return needed;
}
