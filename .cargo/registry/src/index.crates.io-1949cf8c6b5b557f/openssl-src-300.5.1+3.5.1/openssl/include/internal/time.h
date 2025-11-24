/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_TIME_H
# define OSSL_INTERNAL_TIME_H
# pragma once

# include <openssl/e_os2.h>     /* uint64_t */
# include "internal/e_os.h"
# include "internal/e_winsock.h" /* for struct timeval */
# include "internal/safe_math.h"

/*
 * Internal type defining a time.
 * This should be treated as an opaque structure.
 *
 * The time datum is Unix's 1970 and at nanosecond precision, this gives
 * a range of 584 years roughly.
 */
typedef struct {
    uint64_t t;     /* Ticks since the epoch */
} OSSL_TIME;

/* The precision of times allows this many values per second */
# define OSSL_TIME_SECOND ((uint64_t)1000000000)

/* One millisecond. */
# define OSSL_TIME_MS     (OSSL_TIME_SECOND / 1000)

/* One microsecond. */
# define OSSL_TIME_US     (OSSL_TIME_MS     / 1000)

/* One nanosecond. */
# define OSSL_TIME_NS     (OSSL_TIME_US     / 1000)

#define ossl_seconds2time(s) ossl_ticks2time((s) * OSSL_TIME_SECOND)
#define ossl_time2seconds(t) (ossl_time2ticks(t) / OSSL_TIME_SECOND)
#define ossl_ms2time(ms) ossl_ticks2time((ms) * OSSL_TIME_MS)
#define ossl_time2ms(t) (ossl_time2ticks(t) / OSSL_TIME_MS)
#define ossl_us2time(us) ossl_ticks2time((us) * OSSL_TIME_US)
#define ossl_time2us(t) (ossl_time2ticks(t) / OSSL_TIME_US)

/*
 * Arithmetic operations on times.
 * These operations are saturating, in that an overflow or underflow returns
 * the largest or smallest value respectively.
 */
OSSL_SAFE_MATH_UNSIGNED(time, uint64_t)

/* Convert a tick count into a time */
static ossl_unused ossl_inline
OSSL_TIME ossl_ticks2time(uint64_t ticks)
{
    OSSL_TIME r;

    r.t = ticks;
    return r;
}

/* Convert a time to a tick count */
static ossl_unused ossl_inline
uint64_t ossl_time2ticks(OSSL_TIME t)
{
    return t.t;
}

/* Get current time */
OSSL_TIME ossl_time_now(void);

/* The beginning and end of the time range */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_zero(void)
{
    return ossl_ticks2time(0);
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_infinite(void)
{
    return ossl_ticks2time(~(uint64_t)0);
}


/* Convert time to timeval */
static ossl_unused ossl_inline
struct timeval ossl_time_to_timeval(OSSL_TIME t)
{
    struct timeval tv;
    int err = 0;

    /*
     * Round up any nano secs which struct timeval doesn't support. Ensures that
     * we never return a zero time if the input time is non zero
     */
    t.t = safe_add_time(t.t, OSSL_TIME_US - 1, &err);
    if (err)
        t = ossl_time_infinite();

#ifdef _WIN32
    tv.tv_sec = (long int)(t.t / OSSL_TIME_SECOND);
#else
    tv.tv_sec = (time_t)(t.t / OSSL_TIME_SECOND);
#endif
    tv.tv_usec = (t.t % OSSL_TIME_SECOND) / OSSL_TIME_US;
    return tv;
}

/* Convert timeval to time */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_from_timeval(struct timeval tv)
{
    OSSL_TIME t;

#ifndef __DJGPP__ /* tv_sec is unsigned on djgpp. */
    if (tv.tv_sec < 0)
        return ossl_time_zero();
#endif
    t.t = tv.tv_sec * OSSL_TIME_SECOND + tv.tv_usec * OSSL_TIME_US;
    return t;
}

/* Convert OSSL_TIME to time_t */
static ossl_unused ossl_inline
time_t ossl_time_to_time_t(OSSL_TIME t)
{
    return (time_t)(t.t / OSSL_TIME_SECOND);
}

/* Convert time_t to OSSL_TIME */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_from_time_t(time_t t)
{
    OSSL_TIME ot;

    ot.t = t;
    ot.t *= OSSL_TIME_SECOND;
    return ot;
}

/* Compare two time values, return -1 if less, 1 if greater and 0 if equal */
static ossl_unused ossl_inline
int ossl_time_compare(OSSL_TIME a, OSSL_TIME b)
{
    if (a.t > b.t)
        return 1;
    if (a.t < b.t)
        return -1;
    return 0;
}

/* Returns true if an OSSL_TIME is ossl_time_zero(). */
static ossl_unused ossl_inline
int ossl_time_is_zero(OSSL_TIME t)
{
    return ossl_time_compare(t, ossl_time_zero()) == 0;
}

/* Returns true if an OSSL_TIME is ossl_time_infinite(). */
static ossl_unused ossl_inline
int ossl_time_is_infinite(OSSL_TIME t)
{
    return ossl_time_compare(t, ossl_time_infinite()) == 0;
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_add(OSSL_TIME a, OSSL_TIME b)
{
    OSSL_TIME r;
    int err = 0;

    r.t = safe_add_time(a.t, b.t, &err);
    return err ? ossl_time_infinite() : r;
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_subtract(OSSL_TIME a, OSSL_TIME b)
{
    OSSL_TIME r;
    int err = 0;

    r.t = safe_sub_time(a.t, b.t, &err);
    return err ? ossl_time_zero() : r;
}

/* Returns |a - b|. */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_abs_difference(OSSL_TIME a, OSSL_TIME b)
{
    return a.t > b.t ? ossl_time_subtract(a, b)
                     : ossl_time_subtract(b, a);
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_multiply(OSSL_TIME a, uint64_t b)
{
    OSSL_TIME r;
    int err = 0;

    r.t = safe_mul_time(a.t, b, &err);
    return err ? ossl_time_infinite() : r;
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_divide(OSSL_TIME a, uint64_t b)
{
    OSSL_TIME r;
    int err = 0;

    r.t = safe_div_time(a.t, b, &err);
    return err ? ossl_time_zero() : r;
}

static ossl_unused ossl_inline
OSSL_TIME ossl_time_muldiv(OSSL_TIME a, uint64_t b, uint64_t c)
{
    OSSL_TIME r;
    int err = 0;

    r.t = safe_muldiv_time(a.t, b, c, &err);
    return err ? ossl_time_zero() : r;
}

/* Return higher of the two given time values. */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_max(OSSL_TIME a, OSSL_TIME b)
{
    return a.t > b.t ? a : b;
}

/* Return the lower of the two given time values. */
static ossl_unused ossl_inline
OSSL_TIME ossl_time_min(OSSL_TIME a, OSSL_TIME b)
{
    return a.t < b.t ? a : b;
}

#endif
