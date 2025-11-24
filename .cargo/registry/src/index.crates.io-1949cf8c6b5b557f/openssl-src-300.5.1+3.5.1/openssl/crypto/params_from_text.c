/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/common.h" /* for HAS_PREFIX */
#include <openssl/ebcdic.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/buffer.h>

/*
 * When processing text to params, we're trying to be smart with numbers.
 * Instead of handling each specific separate integer type, we use a bignum
 * and ensure that it isn't larger than the expected size, and we then make
 * sure it is the expected size...  if there is one given.
 * (if the size can be arbitrary, then we give whatever we have)
 */

static int prepare_from_text(const OSSL_PARAM *paramdefs, const char *key,
                             const char *value, size_t value_n,
                             /* Output parameters */
                             const OSSL_PARAM **paramdef, int *ishex,
                             size_t *buf_n, BIGNUM **tmpbn, int *found)
{
    const OSSL_PARAM *p;
    size_t buf_bits;
    int r;

    /*
     * ishex is used to translate legacy style string controls in hex format
     * to octet string parameters.
     */
    *ishex = CHECK_AND_SKIP_PREFIX(key, "hex");

    p = *paramdef = OSSL_PARAM_locate_const(paramdefs, key);
    if (found != NULL)
        *found = p != NULL;
    if (p == NULL)
        return 0;

    switch (p->data_type) {
    case OSSL_PARAM_INTEGER:
    case OSSL_PARAM_UNSIGNED_INTEGER:
        if (*ishex)
            r = BN_hex2bn(tmpbn, value);
        else
            r = BN_asc2bn(tmpbn, value);

        if (r == 0 || *tmpbn == NULL)
            return 0;

        if (p->data_type == OSSL_PARAM_UNSIGNED_INTEGER
            && BN_is_negative(*tmpbn)) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_INVALID_NEGATIVE_VALUE);
            return 0;
        }

        /*
         * 2's complement negate, part 1
         *
         * BN_bn2nativepad puts the absolute value of the number in the
         * buffer, i.e. if it's negative, we need to deal with it.  We do
         * it by subtracting 1 here and inverting the bytes in
         * construct_from_text() below.
         * To subtract 1 from an absolute value of a negative number we
         * actually have to add 1: -3 - 1 = -4, |-3| = 3 + 1 = 4.
         */
        if (p->data_type == OSSL_PARAM_INTEGER && BN_is_negative(*tmpbn)
            && !BN_add_word(*tmpbn, 1)) {
            return 0;
        }

        buf_bits = (size_t)BN_num_bits(*tmpbn);

        /*
         * Compensate for cases where the most significant bit in
         * the resulting OSSL_PARAM buffer will be set after the
         * BN_bn2nativepad() call, as the implied sign may not be
         * correct after the second part of the 2's complement
         * negation has been performed.
         * We fix these cases by extending the buffer by one byte
         * (8 bits), which will give some padding.  The second part
         * of the 2's complement negation will do the rest.
         */
        if (p->data_type == OSSL_PARAM_INTEGER && buf_bits % 8 == 0)
            buf_bits += 8;

        *buf_n = (buf_bits + 7) / 8;

        /*
         * A zero data size means "arbitrary size", so only do the
         * range checking if a size is specified.
         */
        if (p->data_size > 0) {
            if (buf_bits > p->data_size * 8) {
                ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_SMALL_BUFFER);
                /* Since this is a different error, we don't break */
                return 0;
            }
            /* Change actual size to become the desired size. */
            *buf_n = p->data_size;
        }
        break;
    case OSSL_PARAM_UTF8_STRING:
        if (*ishex) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
        *buf_n = strlen(value) + 1;
        break;
    case OSSL_PARAM_OCTET_STRING:
        if (*ishex) {
            size_t hexdigits = strlen(value);
            if ((hexdigits % 2) != 0) {
                /* We don't accept an odd number of hex digits */
                ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_ODD_NUMBER_OF_DIGITS);
                return 0;
            }
            *buf_n = hexdigits >> 1;
        } else {
            *buf_n = value_n;
        }
        break;
    }

    return 1;
}

static int construct_from_text(OSSL_PARAM *to, const OSSL_PARAM *paramdef,
                               const char *value, size_t value_n, int ishex,
                               void *buf, size_t buf_n, BIGNUM *tmpbn)
{
    if (buf == NULL)
        return 0;

    if (buf_n > 0) {
        switch (paramdef->data_type) {
        case OSSL_PARAM_INTEGER:
        case OSSL_PARAM_UNSIGNED_INTEGER:
            /*
            {
                if ((new_value = OPENSSL_malloc(new_value_n)) == NULL) {
                    BN_free(a);
                    break;
                }
            */

            BN_bn2nativepad(tmpbn, buf, buf_n);

            /*
             * 2's complement negation, part two.
             *
             * Because we did the first part on the BIGNUM itself, we can just
             * invert all the bytes here and be done with it.
             */
            if (paramdef->data_type == OSSL_PARAM_INTEGER
                && BN_is_negative(tmpbn)) {
                unsigned char *cp;
                size_t i = buf_n;

                for (cp = buf; i-- > 0; cp++)
                    *cp ^= 0xFF;
            }
            break;
        case OSSL_PARAM_UTF8_STRING:
#ifdef CHARSET_EBCDIC
            ebcdic2ascii(buf, value, buf_n);
#else
            strncpy(buf, value, buf_n);
#endif
            /* Don't count the terminating NUL byte as data */
            buf_n--;
            break;
        case OSSL_PARAM_OCTET_STRING:
            if (ishex) {
                size_t l = 0;

                if (!OPENSSL_hexstr2buf_ex(buf, buf_n, &l, value, ':'))
                    return 0;
            } else {
                memcpy(buf, value, buf_n);
            }
            break;
        }
    }

    *to = *paramdef;
    to->data = buf;
    to->data_size = buf_n;
    to->return_size = OSSL_PARAM_UNMODIFIED;

    return 1;
}

/**
 * OSSL_PARAM_print_to_bio - Print OSSL_PARAM array to a bio 
 *
 * @p:        Array of OSSL_PARAM structures containing keys and values.
 * @bio:      Pointer to bio where the formatted output will be written.
 * @print_values: If non-zero, prints both keys and values. If zero, only keys
 *                are printed.
 *
 * This function iterates through the given array of OSSL_PARAM structures,
 * printing each key to an in-memory buffer, and optionally printing its
 * value based on the provided data type. Supported types include integers,
 * strings, octet strings, and real numbers.
 *
 * Return:    1 on success, 0 on failure.
 */
int OSSL_PARAM_print_to_bio(const OSSL_PARAM *p, BIO *bio, int print_values)
{
    int64_t i;
    uint64_t u;
    BIGNUM *bn;
#ifndef OPENSSL_SYS_UEFI
    double d;
    int dok;
#endif
    int ok = -1;

    /*
     * Iterate through each key in the array printing its key and value
     */
    for (; p->key != NULL; p++) {
        ok = -1;
        ok = BIO_printf(bio, "%s: ", p->key);

        if (ok == -1)
            goto end;

        /*
         * if printing of values was not requested, just move on
         * to the next param, after adding a newline to the buffer
         */
        if (print_values == 0) {
            BIO_printf(bio, "\n");
            continue;
        }

        switch (p->data_type) {
        case OSSL_PARAM_UNSIGNED_INTEGER:
            if (p->data_size > sizeof(int64_t)) {
                if (OSSL_PARAM_get_BN(p, &bn))
                    ok = BN_print(bio, bn);
                else
                    ok = BIO_printf(bio, "error getting value\n");
            } else {
                if (OSSL_PARAM_get_uint64(p, &u))
                    ok = BIO_printf(bio, "%llu\n", (unsigned long long int)u);
                else
                    ok = BIO_printf(bio, "error getting value\n");
            }
            break;
        case OSSL_PARAM_INTEGER:
            if (p->data_size > sizeof(int64_t)) {
                if (OSSL_PARAM_get_BN(p, &bn))
                    ok = BN_print(bio, bn);
                else
                    ok = BIO_printf(bio, "error getting value\n");
            } else {
                if (OSSL_PARAM_get_int64(p, &i))
                    ok = BIO_printf(bio, "%lld\n", (long long int)i);
                else
                    ok = BIO_printf(bio, "error getting value\n");
            }
            break;
        case OSSL_PARAM_UTF8_PTR:
            ok = BIO_dump(bio, p->data, p->data_size);
            break;
        case OSSL_PARAM_UTF8_STRING:
            ok = BIO_dump(bio, (char *)p->data, p->data_size);
            break;
        case OSSL_PARAM_OCTET_PTR:
        case OSSL_PARAM_OCTET_STRING:
            ok = BIO_dump(bio, (char *)p->data, p->data_size);
            break;
#ifndef OPENSSL_SYS_UEFI
        case OSSL_PARAM_REAL:
            dok = 0;
            dok = OSSL_PARAM_get_double(p, &d);
            if (dok == 1)
                ok = BIO_printf(bio, "%f\n", d);
            else
                ok = BIO_printf(bio, "error getting value\n");
            break;
#endif
        default:
            ok = BIO_printf(bio, "unknown type (%u) of %zu bytes\n",
                            p->data_type, p->data_size);
            break;
        }
        if (ok == -1)
            goto end;
    }

end:
    return ok == -1 ? 0 : 1;
}

int OSSL_PARAM_allocate_from_text(OSSL_PARAM *to,
                                  const OSSL_PARAM *paramdefs,
                                  const char *key, const char *value,
                                  size_t value_n, int *found)
{
    const OSSL_PARAM *paramdef = NULL;
    int ishex = 0;
    void *buf = NULL;
    size_t buf_n = 0;
    BIGNUM *tmpbn = NULL;
    int ok = 0;

    if (to == NULL || paramdefs == NULL)
        return 0;

    if (!prepare_from_text(paramdefs, key, value, value_n,
                           &paramdef, &ishex, &buf_n, &tmpbn, found))
        goto err;

    if ((buf = OPENSSL_zalloc(buf_n > 0 ? buf_n : 1)) == NULL)
        goto err;

    ok = construct_from_text(to, paramdef, value, value_n, ishex,
                             buf, buf_n, tmpbn);
    BN_free(tmpbn);
    if (!ok)
        OPENSSL_free(buf);
    return ok;
 err:
    BN_free(tmpbn);
    return 0;
}
