/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <limits.h>
#include "internal/cryptlib.h"
#include "internal/endian.h"
#include "bn_local.h"
#include <openssl/opensslconf.h>
#include "internal/constant_time.h"

/* This stuff appears to be completely unused, so is deprecated */
#ifndef OPENSSL_NO_DEPRECATED_0_9_8
/*-
 * For a 32 bit machine
 * 2 -   4 ==  128
 * 3 -   8 ==  256
 * 4 -  16 ==  512
 * 5 -  32 == 1024
 * 6 -  64 == 2048
 * 7 - 128 == 4096
 * 8 - 256 == 8192
 */
static int bn_limit_bits = 0;
static int bn_limit_num = 8;    /* (1<<bn_limit_bits) */
static int bn_limit_bits_low = 0;
static int bn_limit_num_low = 8; /* (1<<bn_limit_bits_low) */
static int bn_limit_bits_high = 0;
static int bn_limit_num_high = 8; /* (1<<bn_limit_bits_high) */
static int bn_limit_bits_mont = 0;
static int bn_limit_num_mont = 8; /* (1<<bn_limit_bits_mont) */

void BN_set_params(int mult, int high, int low, int mont)
{
    if (mult >= 0) {
        if (mult > (int)(sizeof(int) * 8) - 1)
            mult = sizeof(int) * 8 - 1;
        bn_limit_bits = mult;
        bn_limit_num = 1 << mult;
    }
    if (high >= 0) {
        if (high > (int)(sizeof(int) * 8) - 1)
            high = sizeof(int) * 8 - 1;
        bn_limit_bits_high = high;
        bn_limit_num_high = 1 << high;
    }
    if (low >= 0) {
        if (low > (int)(sizeof(int) * 8) - 1)
            low = sizeof(int) * 8 - 1;
        bn_limit_bits_low = low;
        bn_limit_num_low = 1 << low;
    }
    if (mont >= 0) {
        if (mont > (int)(sizeof(int) * 8) - 1)
            mont = sizeof(int) * 8 - 1;
        bn_limit_bits_mont = mont;
        bn_limit_num_mont = 1 << mont;
    }
}

int BN_get_params(int which)
{
    if (which == 0)
        return bn_limit_bits;
    else if (which == 1)
        return bn_limit_bits_high;
    else if (which == 2)
        return bn_limit_bits_low;
    else if (which == 3)
        return bn_limit_bits_mont;
    else
        return 0;
}
#endif

const BIGNUM *BN_value_one(void)
{
    static const BN_ULONG data_one = 1L;
    static const BIGNUM const_one = {
        (BN_ULONG *)&data_one, 1, 1, 0, BN_FLG_STATIC_DATA
    };

    return &const_one;
}

/*
 * Old Visual Studio ARM compiler miscompiles BN_num_bits_word()
 * https://mta.openssl.org/pipermail/openssl-users/2018-August/008465.html
 */
#if defined(_MSC_VER) && defined(_ARM_) && defined(_WIN32_WCE) \
    && _MSC_VER>=1400 && _MSC_VER<1501
# define MS_BROKEN_BN_num_bits_word
# pragma optimize("", off)
#endif
int BN_num_bits_word(BN_ULONG l)
{
    BN_ULONG x, mask;
    int bits = (l != 0);

#if BN_BITS2 > 32
    x = l >> 32;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 32 & mask;
    l ^= (x ^ l) & mask;
#endif

    x = l >> 16;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 16 & mask;
    l ^= (x ^ l) & mask;

    x = l >> 8;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 8 & mask;
    l ^= (x ^ l) & mask;

    x = l >> 4;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 4 & mask;
    l ^= (x ^ l) & mask;

    x = l >> 2;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 2 & mask;
    l ^= (x ^ l) & mask;

    x = l >> 1;
    mask = (0 - x) & BN_MASK2;
    mask = (0 - (mask >> (BN_BITS2 - 1)));
    bits += 1 & mask;

    return bits;
}
#ifdef MS_BROKEN_BN_num_bits_word
# pragma optimize("", on)
#endif

/*
 * This function still leaks `a->dmax`: it's caller's responsibility to
 * expand the input `a` in advance to a public length.
 */
static ossl_inline
int bn_num_bits_consttime(const BIGNUM *a)
{
    int j, ret;
    unsigned int mask, past_i;
    int i = a->top - 1;
    bn_check_top(a);

    for (j = 0, past_i = 0, ret = 0; j < a->dmax; j++) {
        mask = constant_time_eq_int(i, j); /* 0xff..ff if i==j, 0x0 otherwise */

        ret += BN_BITS2 & (~mask & ~past_i);
        ret += BN_num_bits_word(a->d[j]) & mask;

        past_i |= mask; /* past_i will become 0xff..ff after i==j */
    }

    /*
     * if BN_is_zero(a) => i is -1 and ret contains garbage, so we mask the
     * final result.
     */
    mask = ~(constant_time_eq_int(i, ((int)-1)));

    return ret & mask;
}

int BN_num_bits(const BIGNUM *a)
{
    int i = a->top - 1;
    bn_check_top(a);

    if (a->flags & BN_FLG_CONSTTIME) {
        /*
         * We assume that BIGNUMs flagged as CONSTTIME have also been expanded
         * so that a->dmax is not leaking secret information.
         *
         * In other words, it's the caller's responsibility to ensure `a` has
         * been preallocated in advance to a public length if we hit this
         * branch.
         *
         */
        return bn_num_bits_consttime(a);
    }

    if (BN_is_zero(a))
        return 0;

    return ((i * BN_BITS2) + BN_num_bits_word(a->d[i]));
}

static void bn_free_d(BIGNUM *a, int clear)
{
    if (BN_get_flags(a, BN_FLG_SECURE))
        OPENSSL_secure_clear_free(a->d, a->dmax * sizeof(a->d[0]));
    else if (clear != 0)
        OPENSSL_clear_free(a->d, a->dmax * sizeof(a->d[0]));
    else
        OPENSSL_free(a->d);
}


void BN_clear_free(BIGNUM *a)
{
    if (a == NULL)
        return;
    if (a->d != NULL && !BN_get_flags(a, BN_FLG_STATIC_DATA))
        bn_free_d(a, 1);
    if (BN_get_flags(a, BN_FLG_MALLOCED)) {
        OPENSSL_cleanse(a, sizeof(*a));
        OPENSSL_free(a);
    }
}

void BN_free(BIGNUM *a)
{
    if (a == NULL)
        return;
    if (!BN_get_flags(a, BN_FLG_STATIC_DATA))
        bn_free_d(a, 0);
    if (a->flags & BN_FLG_MALLOCED)
        OPENSSL_free(a);
}

void bn_init(BIGNUM *a)
{
    static BIGNUM nilbn;

    *a = nilbn;
    bn_check_top(a);
}

BIGNUM *BN_new(void)
{
    BIGNUM *ret;

    if ((ret = OPENSSL_zalloc(sizeof(*ret))) == NULL)
        return NULL;
    ret->flags = BN_FLG_MALLOCED;
    bn_check_top(ret);
    return ret;
}

BIGNUM *BN_secure_new(void)
{
    BIGNUM *ret = BN_new();

    if (ret != NULL)
        ret->flags |= BN_FLG_SECURE;
    return ret;
}

/* This is used by bn_expand2() */
/* The caller MUST check that words > b->dmax before calling this */
static BN_ULONG *bn_expand_internal(const BIGNUM *b, int words)
{
    BN_ULONG *a = NULL;

    if (words > (INT_MAX / (4 * BN_BITS2))) {
        ERR_raise(ERR_LIB_BN, BN_R_BIGNUM_TOO_LONG);
        return NULL;
    }
    if (BN_get_flags(b, BN_FLG_STATIC_DATA)) {
        ERR_raise(ERR_LIB_BN, BN_R_EXPAND_ON_STATIC_BIGNUM_DATA);
        return NULL;
    }
    if (BN_get_flags(b, BN_FLG_SECURE))
        a = OPENSSL_secure_zalloc(words * sizeof(*a));
    else
        a = OPENSSL_zalloc(words * sizeof(*a));
    if (a == NULL)
        return NULL;

    assert(b->top <= words);
    if (b->top > 0)
        memcpy(a, b->d, sizeof(*a) * b->top);

    return a;
}

/*
 * This is an internal function that should not be used in applications. It
 * ensures that 'b' has enough room for a 'words' word number and initialises
 * any unused part of b->d with leading zeros. It is mostly used by the
 * various BIGNUM routines. If there is an error, NULL is returned. If not,
 * 'b' is returned.
 */

BIGNUM *bn_expand2(BIGNUM *b, int words)
{
    if (words > b->dmax) {
        BN_ULONG *a = bn_expand_internal(b, words);
        if (!a)
            return NULL;
        if (b->d != NULL)
            bn_free_d(b, 1);
        b->d = a;
        b->dmax = words;
    }

    return b;
}

BIGNUM *BN_dup(const BIGNUM *a)
{
    BIGNUM *t;

    if (a == NULL)
        return NULL;
    bn_check_top(a);

    t = BN_get_flags(a, BN_FLG_SECURE) ? BN_secure_new() : BN_new();
    if (t == NULL)
        return NULL;
    if (!BN_copy(t, a)) {
        BN_free(t);
        return NULL;
    }
    bn_check_top(t);
    return t;
}

BIGNUM *BN_copy(BIGNUM *a, const BIGNUM *b)
{
    int bn_words;

    bn_check_top(b);

    bn_words = BN_get_flags(b, BN_FLG_CONSTTIME) ? b->dmax : b->top;

    if (a == b)
        return a;
    if (bn_wexpand(a, bn_words) == NULL)
        return NULL;

    if (b->top > 0)
        memcpy(a->d, b->d, sizeof(b->d[0]) * bn_words);

    a->neg = b->neg;
    a->top = b->top;
    a->flags |= b->flags & BN_FLG_FIXED_TOP;
    bn_check_top(a);
    return a;
}

#define FLAGS_DATA(flags) ((flags) & (BN_FLG_STATIC_DATA \
                                    | BN_FLG_CONSTTIME   \
                                    | BN_FLG_SECURE      \
                                    | BN_FLG_FIXED_TOP))
#define FLAGS_STRUCT(flags) ((flags) & (BN_FLG_MALLOCED))

void BN_swap(BIGNUM *a, BIGNUM *b)
{
    int flags_old_a, flags_old_b;
    BN_ULONG *tmp_d;
    int tmp_top, tmp_dmax, tmp_neg;

    bn_check_top(a);
    bn_check_top(b);

    flags_old_a = a->flags;
    flags_old_b = b->flags;

    tmp_d = a->d;
    tmp_top = a->top;
    tmp_dmax = a->dmax;
    tmp_neg = a->neg;

    a->d = b->d;
    a->top = b->top;
    a->dmax = b->dmax;
    a->neg = b->neg;

    b->d = tmp_d;
    b->top = tmp_top;
    b->dmax = tmp_dmax;
    b->neg = tmp_neg;

    a->flags = FLAGS_STRUCT(flags_old_a) | FLAGS_DATA(flags_old_b);
    b->flags = FLAGS_STRUCT(flags_old_b) | FLAGS_DATA(flags_old_a);
    bn_check_top(a);
    bn_check_top(b);
}

void BN_clear(BIGNUM *a)
{
    if (a == NULL)
        return;
    bn_check_top(a);
    if (a->d != NULL)
        OPENSSL_cleanse(a->d, sizeof(*a->d) * a->dmax);
    a->neg = 0;
    a->top = 0;
    a->flags &= ~BN_FLG_FIXED_TOP;
}

BN_ULONG BN_get_word(const BIGNUM *a)
{
    if (a->top > 1)
        return BN_MASK2;
    else if (a->top == 1)
        return a->d[0];
    /* a->top == 0 */
    return 0;
}

int BN_set_word(BIGNUM *a, BN_ULONG w)
{
    bn_check_top(a);
    if (bn_expand(a, (int)sizeof(BN_ULONG) * 8) == NULL)
        return 0;
    a->neg = 0;
    a->d[0] = w;
    a->top = (w ? 1 : 0);
    a->flags &= ~BN_FLG_FIXED_TOP;
    bn_check_top(a);
    return 1;
}

typedef enum {BIG, LITTLE} endianness_t;
typedef enum {SIGNED, UNSIGNED} signedness_t;

static BIGNUM *bin2bn(const unsigned char *s, int len, BIGNUM *ret,
                      endianness_t endianness, signedness_t signedness)
{
    int inc;
    const unsigned char *s2;
    int inc2;
    int neg = 0, xor = 0, carry = 0;
    unsigned int i;
    unsigned int n;
    BIGNUM *bn = NULL;

    /* Negative length is not acceptable */
    if (len < 0)
        return NULL;

    if (ret == NULL)
        ret = bn = BN_new();
    if (ret == NULL)
        return NULL;
    bn_check_top(ret);

    /*
     * If the input has no bits, the number is considered zero.
     * This makes calls with s==NULL and len==0 safe.
     */
    if (len == 0) {
        BN_clear(ret);
        return ret;
    }

    /*
     * The loop that does the work iterates from least to most
     * significant BIGNUM chunk, so we adapt parameters to transfer
     * input bytes accordingly.
     */
    if (endianness == LITTLE) {
        s2 = s + len - 1;
        inc2 = -1;
        inc = 1;
    } else {
        s2 = s;
        inc2 = 1;
        inc = -1;
        s += len - 1;
    }

    /* Take note of the signedness of the input bytes*/
    if (signedness == SIGNED) {
        neg = !!(*s2 & 0x80);
        xor = neg ? 0xff : 0x00;
        carry = neg;
    }

    /*
     * Skip leading sign extensions (the value of |xor|).
     * This is the only spot where |s2| and |inc2| are used.
     */
    for ( ; len > 0 && *s2 == xor; s2 += inc2, len--)
        continue;

    /*
     * If there was a set of 0xff, we backtrack one byte unless the next
     * one has a sign bit, as the last 0xff is then part of the actual
     * number, rather then a mere sign extension.
     */
    if (xor == 0xff) {
        if (len == 0 || !(*s2 & 0x80))
            len++;
    }
    /* If it was all zeros, we're done */
    if (len == 0) {
        ret->top = 0;
        return ret;
    }
    n = ((len - 1) / BN_BYTES) + 1; /* Number of resulting bignum chunks */
    if (bn_wexpand(ret, (int)n) == NULL) {
        BN_free(bn);
        return NULL;
    }
    ret->top = n;
    ret->neg = neg;
    for (i = 0; n-- > 0; i++) {
        BN_ULONG l = 0;        /* Accumulator */
        unsigned int m = 0;    /* Offset in a bignum chunk, in bits */

        for (; len > 0 && m < BN_BYTES * 8; len--, s += inc, m += 8) {
            BN_ULONG byte_xored = *s ^ xor;
            BN_ULONG byte = (byte_xored + carry) & 0xff;

            carry = byte_xored > byte; /* Implicit 1 or 0 */
            l |= (byte << m);
        }
        ret->d[i] = l;
    }
    /*
     * need to call this due to clear byte at top if avoiding having the top
     * bit set (-ve number)
     */
    bn_correct_top(ret);
    return ret;
}

BIGNUM *BN_bin2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    return bin2bn(s, len, ret, BIG, UNSIGNED);
}

BIGNUM *BN_signed_bin2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    return bin2bn(s, len, ret, BIG, SIGNED);
}

static int bn2binpad(const BIGNUM *a, unsigned char *to, int tolen,
                     endianness_t endianness, signedness_t signedness)
{
    int inc;
    int n, n8;
    int xor = 0, carry = 0, ext = 0;
    size_t i, lasti, j, atop, mask;
    BN_ULONG l;

    /*
     * In case |a| is fixed-top, BN_num_bits can return bogus length,
     * but it's assumed that fixed-top inputs ought to be "nominated"
     * even for padded output, so it works out...
     */
    n8 = BN_num_bits(a);
    n = (n8 + 7) / 8;           /* This is what BN_num_bytes() does */

    /* Take note of the signedness of the bignum */
    if (signedness == SIGNED) {
        xor = a->neg ? 0xff : 0x00;
        carry = a->neg;

        /*
         * if |n * 8 == n|, then the MSbit is set, otherwise unset.
         * We must compensate with one extra byte if that doesn't
         * correspond to the signedness of the bignum with regards
         * to 2's complement.
         */
        ext = (n * 8 == n8)
            ? !a->neg            /* MSbit set on nonnegative bignum */
            : a->neg;            /* MSbit unset on negative bignum */
    }

    if (tolen == -1) {
        tolen = n + ext;
    } else if (tolen < n + ext) { /* uncommon/unlike case */
        BIGNUM temp = *a;

        bn_correct_top(&temp);
        n8 = BN_num_bits(&temp);
        n = (n8 + 7) / 8;       /* This is what BN_num_bytes() does */
        if (tolen < n + ext)
            return -1;
    }

    /* Swipe through whole available data and don't give away padded zero. */
    atop = a->dmax * BN_BYTES;
    if (atop == 0) {
        if (tolen != 0)
            memset(to, '\0', tolen);
        return tolen;
    }

    /*
     * The loop that does the work iterates from least significant
     * to most significant BIGNUM limb, so we adapt parameters to
     * transfer output bytes accordingly.
     */
    if (endianness == LITTLE) {
        inc = 1;
    } else {
        inc = -1;
        to += tolen - 1;         /* Move to the last byte, not beyond */
    }

    lasti = atop - 1;
    atop = a->top * BN_BYTES;
    for (i = 0, j = 0; j < (size_t)tolen; j++) {
        unsigned char byte, byte_xored;

        l = a->d[i / BN_BYTES];
        mask = 0 - ((j - atop) >> (8 * sizeof(i) - 1));
        byte = (unsigned char)(l >> (8 * (i % BN_BYTES)) & mask);
        byte_xored = byte ^ xor;
        *to = (unsigned char)(byte_xored + carry);
        carry = byte_xored > *to; /* Implicit 1 or 0 */
        to += inc;
        i += (i - lasti) >> (8 * sizeof(i) - 1); /* stay on last limb */
    }

    return tolen;
}

int BN_bn2binpad(const BIGNUM *a, unsigned char *to, int tolen)
{
    if (tolen < 0)
        return -1;
    return bn2binpad(a, to, tolen, BIG, UNSIGNED);
}

int BN_signed_bn2bin(const BIGNUM *a, unsigned char *to, int tolen)
{
    if (tolen < 0)
        return -1;
    return bn2binpad(a, to, tolen, BIG, SIGNED);
}

int BN_bn2bin(const BIGNUM *a, unsigned char *to)
{
    return bn2binpad(a, to, -1, BIG, UNSIGNED);
}

BIGNUM *BN_lebin2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    return bin2bn(s, len, ret, LITTLE, UNSIGNED);
}

BIGNUM *BN_signed_lebin2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    return bin2bn(s, len, ret, LITTLE, SIGNED);
}

int BN_bn2lebinpad(const BIGNUM *a, unsigned char *to, int tolen)
{
    if (tolen < 0)
        return -1;
    return bn2binpad(a, to, tolen, LITTLE, UNSIGNED);
}

int BN_signed_bn2lebin(const BIGNUM *a, unsigned char *to, int tolen)
{
    if (tolen < 0)
        return -1;
    return bn2binpad(a, to, tolen, LITTLE, SIGNED);
}

BIGNUM *BN_native2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN)
        return BN_lebin2bn(s, len, ret);
    return BN_bin2bn(s, len, ret);
}

BIGNUM *BN_signed_native2bn(const unsigned char *s, int len, BIGNUM *ret)
{
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN)
        return BN_signed_lebin2bn(s, len, ret);
    return BN_signed_bin2bn(s, len, ret);
}

int BN_bn2nativepad(const BIGNUM *a, unsigned char *to, int tolen)
{
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN)
        return BN_bn2lebinpad(a, to, tolen);
    return BN_bn2binpad(a, to, tolen);
}

int BN_signed_bn2native(const BIGNUM *a, unsigned char *to, int tolen)
{
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN)
        return BN_signed_bn2lebin(a, to, tolen);
    return BN_signed_bn2bin(a, to, tolen);
}

int BN_ucmp(const BIGNUM *a, const BIGNUM *b)
{
    int i;
    BN_ULONG t1, t2, *ap, *bp;

    ap = a->d;
    bp = b->d;

    if (BN_get_flags(a, BN_FLG_CONSTTIME)
            && a->top == b->top) {
        int res = 0;

        for (i = 0; i < b->top; i++) {
            res = constant_time_select_int(constant_time_lt_bn(ap[i], bp[i]),
                                           -1, res);
            res = constant_time_select_int(constant_time_lt_bn(bp[i], ap[i]),
                                           1, res);
        }
        return res;
    }

    bn_check_top(a);
    bn_check_top(b);

    i = a->top - b->top;
    if (i != 0)
        return i;

    for (i = a->top - 1; i >= 0; i--) {
        t1 = ap[i];
        t2 = bp[i];
        if (t1 != t2)
            return ((t1 > t2) ? 1 : -1);
    }
    return 0;
}

int BN_cmp(const BIGNUM *a, const BIGNUM *b)
{
    int i;
    int gt, lt;
    BN_ULONG t1, t2;

    if ((a == NULL) || (b == NULL)) {
        if (a != NULL)
            return -1;
        else if (b != NULL)
            return 1;
        else
            return 0;
    }

    bn_check_top(a);
    bn_check_top(b);

    if (a->neg != b->neg) {
        if (a->neg)
            return -1;
        else
            return 1;
    }
    if (a->neg == 0) {
        gt = 1;
        lt = -1;
    } else {
        gt = -1;
        lt = 1;
    }

    if (a->top > b->top)
        return gt;
    if (a->top < b->top)
        return lt;
    for (i = a->top - 1; i >= 0; i--) {
        t1 = a->d[i];
        t2 = b->d[i];
        if (t1 > t2)
            return gt;
        if (t1 < t2)
            return lt;
    }
    return 0;
}

int BN_set_bit(BIGNUM *a, int n)
{
    int i, j, k;

    if (n < 0)
        return 0;

    i = n / BN_BITS2;
    j = n % BN_BITS2;
    if (a->top <= i) {
        if (bn_wexpand(a, i + 1) == NULL)
            return 0;
        for (k = a->top; k < i + 1; k++)
            a->d[k] = 0;
        a->top = i + 1;
        a->flags &= ~BN_FLG_FIXED_TOP;
    }

    a->d[i] |= (((BN_ULONG)1) << j);
    bn_check_top(a);
    return 1;
}

int BN_clear_bit(BIGNUM *a, int n)
{
    int i, j;

    bn_check_top(a);
    if (n < 0)
        return 0;

    i = n / BN_BITS2;
    j = n % BN_BITS2;
    if (a->top <= i)
        return 0;

    a->d[i] &= (~(((BN_ULONG)1) << j));
    bn_correct_top(a);
    return 1;
}

int BN_is_bit_set(const BIGNUM *a, int n)
{
    int i, j;

    bn_check_top(a);
    if (n < 0)
        return 0;
    i = n / BN_BITS2;
    j = n % BN_BITS2;
    if (a->top <= i)
        return 0;
    return (int)(((a->d[i]) >> j) & ((BN_ULONG)1));
}

int ossl_bn_mask_bits_fixed_top(BIGNUM *a, int n)
{
    int b, w;

    if (n < 0)
        return 0;

    w = n / BN_BITS2;
    b = n % BN_BITS2;
    if (w >= a->top)
        return 0;
    if (b == 0)
        a->top = w;
    else {
        a->top = w + 1;
        a->d[w] &= ~(BN_MASK2 << b);
    }
    a->flags |= BN_FLG_FIXED_TOP;
    return 1;
}

int BN_mask_bits(BIGNUM *a, int n)
{
    int ret;

    bn_check_top(a);
    ret = ossl_bn_mask_bits_fixed_top(a, n);
    if (ret)
        bn_correct_top(a);
    return ret;
}

void BN_set_negative(BIGNUM *a, int b)
{
    if (b && !BN_is_zero(a))
        a->neg = 1;
    else
        a->neg = 0;
}

int bn_cmp_words(const BN_ULONG *a, const BN_ULONG *b, int n)
{
    int i;
    BN_ULONG aa, bb;

    if (n == 0)
        return 0;

    aa = a[n - 1];
    bb = b[n - 1];
    if (aa != bb)
        return ((aa > bb) ? 1 : -1);
    for (i = n - 2; i >= 0; i--) {
        aa = a[i];
        bb = b[i];
        if (aa != bb)
            return ((aa > bb) ? 1 : -1);
    }
    return 0;
}

/*
 * Here follows a specialised variants of bn_cmp_words().  It has the
 * capability of performing the operation on arrays of different sizes. The
 * sizes of those arrays is expressed through cl, which is the common length
 * ( basically, min(len(a),len(b)) ), and dl, which is the delta between the
 * two lengths, calculated as len(a)-len(b). All lengths are the number of
 * BN_ULONGs...
 */

int bn_cmp_part_words(const BN_ULONG *a, const BN_ULONG *b, int cl, int dl)
{
    int n, i;
    n = cl - 1;

    if (dl < 0) {
        for (i = dl; i < 0; i++) {
            if (b[n - i] != 0)
                return -1;      /* a < b */
        }
    }
    if (dl > 0) {
        for (i = dl; i > 0; i--) {
            if (a[n + i] != 0)
                return 1;       /* a > b */
        }
    }
    return bn_cmp_words(a, b, cl);
}

/*-
 * Constant-time conditional swap of a and b.
 * a and b are swapped if condition is not 0.
 * nwords is the number of words to swap.
 * Assumes that at least nwords are allocated in both a and b.
 * Assumes that no more than nwords are used by either a or b.
 */
void BN_consttime_swap(BN_ULONG condition, BIGNUM *a, BIGNUM *b, int nwords)
{
    BN_ULONG t;
    int i;

    bn_wcheck_size(a, nwords);
    bn_wcheck_size(b, nwords);

    condition = ((~condition & ((condition - 1))) >> (BN_BITS2 - 1)) - 1;

    t = (a->top ^ b->top) & condition;
    a->top ^= t;
    b->top ^= t;

    t = (a->neg ^ b->neg) & condition;
    a->neg ^= t;
    b->neg ^= t;

    /*-
     * BN_FLG_STATIC_DATA: indicates that data may not be written to. Intention
     * is actually to treat it as it's read-only data, and some (if not most)
     * of it does reside in read-only segment. In other words observation of
     * BN_FLG_STATIC_DATA in BN_consttime_swap should be treated as fatal
     * condition. It would either cause SEGV or effectively cause data
     * corruption.
     *
     * BN_FLG_MALLOCED: refers to BN structure itself, and hence must be
     * preserved.
     *
     * BN_FLG_SECURE: must be preserved, because it determines how x->d was
     * allocated and hence how to free it.
     *
     * BN_FLG_CONSTTIME: sufficient to mask and swap
     *
     * BN_FLG_FIXED_TOP: indicates that we haven't called bn_correct_top() on
     * the data, so the d array may be padded with additional 0 values (i.e.
     * top could be greater than the minimal value that it could be). We should
     * be swapping it
     */

#define BN_CONSTTIME_SWAP_FLAGS (BN_FLG_CONSTTIME | BN_FLG_FIXED_TOP)

    t = ((a->flags ^ b->flags) & BN_CONSTTIME_SWAP_FLAGS) & condition;
    a->flags ^= t;
    b->flags ^= t;

    /* conditionally swap the data */
    for (i = 0; i < nwords; i++) {
        t = (a->d[i] ^ b->d[i]) & condition;
        a->d[i] ^= t;
        b->d[i] ^= t;
    }
}

#undef BN_CONSTTIME_SWAP_FLAGS

/* Bits of security, see SP800-57 */

int BN_security_bits(int L, int N)
{
    int secbits, bits;
    if (L >= 15360)
        secbits = 256;
    else if (L >= 7680)
        secbits = 192;
    else if (L >= 3072)
        secbits = 128;
    else if (L >= 2048)
        secbits = 112;
    else if (L >= 1024)
        secbits = 80;
    else
        return 0;
    if (N == -1)
        return secbits;
    bits = N / 2;
    if (bits < 80)
        return 0;
    return bits >= secbits ? secbits : bits;
}

void BN_zero_ex(BIGNUM *a)
{
    a->neg = 0;
    a->top = 0;
    a->flags &= ~BN_FLG_FIXED_TOP;
}

int BN_abs_is_word(const BIGNUM *a, const BN_ULONG w)
{
    return ((a->top == 1) && (a->d[0] == w)) || ((w == 0) && (a->top == 0));
}

int BN_is_zero(const BIGNUM *a)
{
    return a->top == 0;
}

int BN_is_one(const BIGNUM *a)
{
    return BN_abs_is_word(a, 1) && !a->neg;
}

int BN_is_word(const BIGNUM *a, const BN_ULONG w)
{
    return BN_abs_is_word(a, w) && (!w || !a->neg);
}

int ossl_bn_is_word_fixed_top(const BIGNUM *a, const BN_ULONG w)
{
    int res, i;
    const BN_ULONG *ap = a->d;

    if (a->neg || a->top == 0)
        return 0;

    res = constant_time_select_int(constant_time_eq_bn(ap[0], w), 1, 0);

    for (i = 1; i < a->top; i++)
        res = constant_time_select_int(constant_time_is_zero_bn(ap[i]),
                                       res, 0);
    return res;
}

int BN_is_odd(const BIGNUM *a)
{
    return (a->top > 0) && (a->d[0] & 1);
}

int BN_is_negative(const BIGNUM *a)
{
    return (a->neg != 0);
}

int BN_to_montgomery(BIGNUM *r, const BIGNUM *a, BN_MONT_CTX *mont,
                     BN_CTX *ctx)
{
    return BN_mod_mul_montgomery(r, a, &(mont->RR), mont, ctx);
}

void BN_with_flags(BIGNUM *dest, const BIGNUM *b, int flags)
{
    dest->d = b->d;
    dest->top = b->top;
    dest->dmax = b->dmax;
    dest->neg = b->neg;
    dest->flags = ((dest->flags & BN_FLG_MALLOCED)
                   | (b->flags & ~BN_FLG_MALLOCED)
                   | BN_FLG_STATIC_DATA | flags);
}

BN_GENCB *BN_GENCB_new(void)
{
    BN_GENCB *ret;

    if ((ret = OPENSSL_malloc(sizeof(*ret))) == NULL)
        return NULL;

    return ret;
}

void BN_GENCB_free(BN_GENCB *cb)
{
    if (cb == NULL)
        return;
    OPENSSL_free(cb);
}

void BN_set_flags(BIGNUM *b, int n)
{
    b->flags |= n;
}

int BN_get_flags(const BIGNUM *b, int n)
{
    return b->flags & n;
}

/* Populate a BN_GENCB structure with an "old"-style callback */
void BN_GENCB_set_old(BN_GENCB *gencb, void (*callback) (int, int, void *),
                      void *cb_arg)
{
    BN_GENCB *tmp_gencb = gencb;
    tmp_gencb->ver = 1;
    tmp_gencb->arg = cb_arg;
    tmp_gencb->cb.cb_1 = callback;
}

/* Populate a BN_GENCB structure with a "new"-style callback */
void BN_GENCB_set(BN_GENCB *gencb, int (*callback) (int, int, BN_GENCB *),
                  void *cb_arg)
{
    BN_GENCB *tmp_gencb = gencb;
    tmp_gencb->ver = 2;
    tmp_gencb->arg = cb_arg;
    tmp_gencb->cb.cb_2 = callback;
}

void *BN_GENCB_get_arg(BN_GENCB *cb)
{
    return cb->arg;
}

BIGNUM *bn_wexpand(BIGNUM *a, int words)
{
    return (words <= a->dmax) ? a : bn_expand2(a, words);
}

void bn_correct_top_consttime(BIGNUM *a)
{
    int j, atop;
    BN_ULONG limb;
    unsigned int mask;

    for (j = 0, atop = 0; j < a->dmax; j++) {
        limb = a->d[j];
        limb |= 0 - limb;
        limb >>= BN_BITS2 - 1;
        limb = 0 - limb;
        mask = (unsigned int)limb;
        mask &= constant_time_msb(j - a->top);
        atop = constant_time_select_int(mask, j + 1, atop);
    }

    mask = constant_time_eq_int(atop, 0);
    a->top = atop;
    a->neg = constant_time_select_int(mask, 0, a->neg);
    a->flags &= ~BN_FLG_FIXED_TOP;
}

void bn_correct_top(BIGNUM *a)
{
    BN_ULONG *ftl;
    int tmp_top = a->top;

    if (tmp_top > 0) {
        for (ftl = &(a->d[tmp_top]); tmp_top > 0; tmp_top--) {
            ftl--;
            if (*ftl != 0)
                break;
        }
        a->top = tmp_top;
    }
    if (a->top == 0)
        a->neg = 0;
    a->flags &= ~BN_FLG_FIXED_TOP;
    bn_pollute(a);
}
