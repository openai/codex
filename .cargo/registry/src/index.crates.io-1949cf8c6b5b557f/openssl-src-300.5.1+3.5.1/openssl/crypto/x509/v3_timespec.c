/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/asn1t.h>
#include <openssl/x509v3.h>
#include <crypto/asn1.h>
#include "ext_dat.h"

static const char *WEEKDAY_NAMES[7] = {
    "SUN",
    "MON",
    "TUE",
    "WED",
    "THU",
    "FRI",
    "SAT"
};

static const char *WEEK_NAMES[5] = {
    "first",
    "second",
    "third",
    "fourth",
    "final"
};

static const char *MONTH_NAMES[12] = {
    "JAN",
    "FEB",
    "MAR",
    "APR",
    "MAY",
    "JUN",
    "JUL",
    "AUG",
    "SEPT",
    "OCT",
    "NOV",
    "DEC"
};

ASN1_SEQUENCE(OSSL_TIME_SPEC_ABSOLUTE) = {
    ASN1_EXP_OPT(OSSL_TIME_SPEC_ABSOLUTE, startTime, ASN1_GENERALIZEDTIME, 0),
    ASN1_EXP_OPT(OSSL_TIME_SPEC_ABSOLUTE, endTime, ASN1_GENERALIZEDTIME, 1),
} ASN1_SEQUENCE_END(OSSL_TIME_SPEC_ABSOLUTE)

ASN1_SEQUENCE(OSSL_DAY_TIME) = {
    ASN1_EXP_OPT(OSSL_DAY_TIME, hour, ASN1_INTEGER, 0),
    ASN1_EXP_OPT(OSSL_DAY_TIME, minute, ASN1_INTEGER, 1),
    ASN1_EXP_OPT(OSSL_DAY_TIME, second, ASN1_INTEGER, 2),
} ASN1_SEQUENCE_END(OSSL_DAY_TIME)

ASN1_SEQUENCE(OSSL_DAY_TIME_BAND) = {
    ASN1_EXP_OPT(OSSL_DAY_TIME_BAND, startDayTime, OSSL_DAY_TIME, 0),
    ASN1_EXP_OPT(OSSL_DAY_TIME_BAND, endDayTime, OSSL_DAY_TIME, 1),
} ASN1_SEQUENCE_END(OSSL_DAY_TIME_BAND)

ASN1_CHOICE(OSSL_NAMED_DAY) = {
    ASN1_SET_OF(OSSL_NAMED_DAY, choice.intNamedDays, ASN1_ENUMERATED),
    ASN1_SIMPLE(OSSL_NAMED_DAY, choice.bitNamedDays, ASN1_BIT_STRING),
} ASN1_CHOICE_END(OSSL_NAMED_DAY)

ASN1_CHOICE(OSSL_TIME_SPEC_X_DAY_OF) = {
    ASN1_EXP(OSSL_TIME_SPEC_X_DAY_OF, choice.first, OSSL_NAMED_DAY, 1),
    ASN1_EXP(OSSL_TIME_SPEC_X_DAY_OF, choice.second, OSSL_NAMED_DAY, 2),
    ASN1_EXP(OSSL_TIME_SPEC_X_DAY_OF, choice.third, OSSL_NAMED_DAY, 3),
    ASN1_EXP(OSSL_TIME_SPEC_X_DAY_OF, choice.fourth, OSSL_NAMED_DAY, 4),
    ASN1_EXP(OSSL_TIME_SPEC_X_DAY_OF, choice.fifth, OSSL_NAMED_DAY, 5),
} ASN1_CHOICE_END(OSSL_TIME_SPEC_X_DAY_OF)

ASN1_CHOICE(OSSL_TIME_SPEC_DAY) = {
    ASN1_SET_OF(OSSL_TIME_SPEC_DAY, choice.intDay, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_TIME_SPEC_DAY, choice.bitDay, ASN1_BIT_STRING),
    ASN1_SIMPLE(OSSL_TIME_SPEC_DAY, choice.dayOf, OSSL_TIME_SPEC_X_DAY_OF),
} ASN1_CHOICE_END(OSSL_TIME_SPEC_DAY)

ASN1_CHOICE(OSSL_TIME_SPEC_WEEKS) = {
    ASN1_SIMPLE(OSSL_TIME_SPEC_WEEKS, choice.allWeeks, ASN1_NULL),
    ASN1_SET_OF(OSSL_TIME_SPEC_WEEKS, choice.intWeek, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_TIME_SPEC_WEEKS, choice.bitWeek, ASN1_BIT_STRING),
} ASN1_CHOICE_END(OSSL_TIME_SPEC_WEEKS)

ASN1_CHOICE(OSSL_TIME_SPEC_MONTH) = {
    ASN1_SIMPLE(OSSL_TIME_SPEC_MONTH, choice.allMonths, ASN1_NULL),
    ASN1_SET_OF(OSSL_TIME_SPEC_MONTH, choice.intMonth, ASN1_INTEGER),
    ASN1_SIMPLE(OSSL_TIME_SPEC_MONTH, choice.bitMonth, ASN1_BIT_STRING),
} ASN1_CHOICE_END(OSSL_TIME_SPEC_MONTH)

ASN1_SEQUENCE(OSSL_TIME_PERIOD) = {
    ASN1_EXP_SET_OF_OPT(OSSL_TIME_PERIOD, timesOfDay, OSSL_DAY_TIME_BAND, 0),
    ASN1_EXP_OPT(OSSL_TIME_PERIOD, days, OSSL_TIME_SPEC_DAY, 1),
    ASN1_EXP_OPT(OSSL_TIME_PERIOD, weeks, OSSL_TIME_SPEC_WEEKS, 2),
    ASN1_EXP_OPT(OSSL_TIME_PERIOD, months, OSSL_TIME_SPEC_MONTH, 3),
    ASN1_EXP_SET_OF_OPT(OSSL_TIME_PERIOD, years, ASN1_INTEGER, 4),
} ASN1_SEQUENCE_END(OSSL_TIME_PERIOD)

ASN1_CHOICE(OSSL_TIME_SPEC_TIME) = {
    ASN1_SIMPLE(OSSL_TIME_SPEC_TIME, choice.absolute, OSSL_TIME_SPEC_ABSOLUTE),
    ASN1_SET_OF(OSSL_TIME_SPEC_TIME, choice.periodic, OSSL_TIME_PERIOD)
} ASN1_CHOICE_END(OSSL_TIME_SPEC_TIME)

ASN1_SEQUENCE(OSSL_TIME_SPEC) = {
    ASN1_SIMPLE(OSSL_TIME_SPEC, time, OSSL_TIME_SPEC_TIME),
    ASN1_OPT(OSSL_TIME_SPEC, notThisTime, ASN1_FBOOLEAN),
    ASN1_OPT(OSSL_TIME_SPEC, timeZone, ASN1_INTEGER),
} ASN1_SEQUENCE_END(OSSL_TIME_SPEC)

IMPLEMENT_ASN1_FUNCTIONS(OSSL_DAY_TIME)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_DAY_TIME_BAND)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_DAY)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_WEEKS)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_MONTH)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_NAMED_DAY)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_X_DAY_OF)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_ABSOLUTE)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC_TIME)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_SPEC)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_TIME_PERIOD)

static int i2r_OSSL_TIME_SPEC_ABSOLUTE(X509V3_EXT_METHOD *method,
                                       OSSL_TIME_SPEC_ABSOLUTE *time,
                                       BIO *out, int indent)
{
    if (time->startTime != NULL && time->endTime != NULL) {
        if (!BIO_puts(out, "Any time between "))
            return 0;
        if (!ossl_asn1_time_print_ex(out, time->startTime, 0))
            return 0;
        if (!BIO_puts(out, " and "))
            return 0;
        if (!ossl_asn1_time_print_ex(out, time->endTime, 0))
            return 0;
    } else if (time->startTime != NULL) {
        if (!BIO_puts(out, "Any time after "))
            return 0;
        if (!ossl_asn1_time_print_ex(out, time->startTime, 0))
            return 0;
        if (BIO_printf(out, "%.*s", time->startTime->length, time->startTime->data) <= 0)
            return 0;
    } else if (time->endTime != NULL) {
        if (!BIO_puts(out, "Any time until "))
            return 0;
        if (!ossl_asn1_time_print_ex(out, time->endTime, 0))
            return 0;
    } else { /* Invalid: there must be SOME time specified. */
        return BIO_puts(out, "INVALID (EMPTY)");
    }
    return 1;
}

static int i2r_OSSL_DAY_TIME(X509V3_EXT_METHOD *method,
                             OSSL_DAY_TIME *dt,
                             BIO *out, int indent)
{
    int64_t h = 0;
    int64_t m = 0;
    int64_t s = 0;

    if (!dt->hour || !ASN1_INTEGER_get_int64(&h, dt->hour))
        return 0;
    if (dt->minute && !ASN1_INTEGER_get_int64(&m, dt->minute))
        return 0;
    if (dt->minute && !ASN1_INTEGER_get_int64(&s, dt->second))
        return 0;
    return BIO_printf(out, "%02lld:%02lld:%02lld",
                      (long long int)h, (long long int)m, (long long int)s) > 0;
}

static int i2r_OSSL_DAY_TIME_BAND(X509V3_EXT_METHOD *method,
                                  OSSL_DAY_TIME_BAND *band,
                                  BIO *out, int indent)
{
    if (band->startDayTime) {
        if (!i2r_OSSL_DAY_TIME(method, band->startDayTime, out, indent))
            return 0;
    } else if (!BIO_puts(out, "00:00:00")) {
        return 0;
    }
    if (!BIO_puts(out, " - "))
        return 0;
    if (band->endDayTime) {
        if (!i2r_OSSL_DAY_TIME(method, band->endDayTime, out, indent))
            return 0;
    } else if (!BIO_puts(out, "23:59:59")) {
        return 0;
    }
    return 1;
}

static int print_int_month(BIO *out, int64_t month)
{
    switch (month) {
    case (OSSL_TIME_SPEC_INT_MONTH_JAN):
        return BIO_puts(out, "JAN");
    case (OSSL_TIME_SPEC_INT_MONTH_FEB):
        return BIO_puts(out, "FEB");
    case (OSSL_TIME_SPEC_INT_MONTH_MAR):
        return BIO_puts(out, "MAR");
    case (OSSL_TIME_SPEC_INT_MONTH_APR):
        return BIO_puts(out, "APR");
    case (OSSL_TIME_SPEC_INT_MONTH_MAY):
        return BIO_puts(out, "MAY");
    case (OSSL_TIME_SPEC_INT_MONTH_JUN):
        return BIO_puts(out, "JUN");
    case (OSSL_TIME_SPEC_INT_MONTH_JUL):
        return BIO_puts(out, "JUL");
    case (OSSL_TIME_SPEC_INT_MONTH_AUG):
        return BIO_puts(out, "AUG");
    case (OSSL_TIME_SPEC_INT_MONTH_SEP):
        return BIO_puts(out, "SEP");
    case (OSSL_TIME_SPEC_INT_MONTH_OCT):
        return BIO_puts(out, "OCT");
    case (OSSL_TIME_SPEC_INT_MONTH_NOV):
        return BIO_puts(out, "NOV");
    case (OSSL_TIME_SPEC_INT_MONTH_DEC):
        return BIO_puts(out, "DEC");
    default:
        return 0;
    }
    return 0;
}

static int print_bit_month(BIO *out, ASN1_BIT_STRING *bs)
{
    int i = OSSL_TIME_SPEC_BIT_MONTH_JAN;
    int j = 0;

    for (; i <= OSSL_TIME_SPEC_BIT_MONTH_DEC; i++) {
        if (ASN1_BIT_STRING_get_bit(bs, i)) {
            if (j > 0 && !BIO_puts(out, ", "))
                return 0;
            j++;
            if (!BIO_puts(out, MONTH_NAMES[i]))
                return 0;
        }
    }
    return 1;
}

/*
 * It might seem like you could just print the bits of the string numerically,
 * but the fifth bit has the special meaning of "the final week" imputed to it
 * by the text of ITU-T Recommendation X.520.
 */
static int print_bit_week(BIO *out, ASN1_BIT_STRING *bs)
{
    int i = OSSL_TIME_SPEC_BIT_WEEKS_1;
    int j = 0;

    for (; i <= OSSL_TIME_SPEC_BIT_WEEKS_5; i++) {
        if (ASN1_BIT_STRING_get_bit(bs, i)) {
            if (j > 0 && !BIO_puts(out, ", "))
                return 0;
            j++;
            if (!BIO_puts(out, WEEK_NAMES[i]))
                return 0;
        }
    }
    return 1;
}

static int print_day_of_week(BIO *out, ASN1_BIT_STRING *bs)
{
    int i = OSSL_TIME_SPEC_DAY_BIT_SUN;
    int j = 0;

    for (; i <= OSSL_TIME_SPEC_DAY_BIT_SAT; i++) {
        if (ASN1_BIT_STRING_get_bit(bs, i)) {
            if (j > 0 && !BIO_puts(out, ", "))
                return 0;
            j++;
            if (!BIO_puts(out, WEEKDAY_NAMES[i]))
                return 0;
        }
    }
    return 1;
}

static int print_int_day_of_week(BIO *out, int64_t dow)
{
    switch (dow) {
    case (OSSL_TIME_SPEC_DAY_INT_SUN):
        return BIO_puts(out, "SUN");
    case (OSSL_TIME_SPEC_DAY_INT_MON):
        return BIO_puts(out, "MON");
    case (OSSL_TIME_SPEC_DAY_INT_TUE):
        return BIO_puts(out, "TUE");
    case (OSSL_TIME_SPEC_DAY_INT_WED):
        return BIO_puts(out, "WED");
    case (OSSL_TIME_SPEC_DAY_INT_THU):
        return BIO_puts(out, "THU");
    case (OSSL_TIME_SPEC_DAY_INT_FRI):
        return BIO_puts(out, "FRI");
    case (OSSL_TIME_SPEC_DAY_INT_SAT):
        return BIO_puts(out, "SAT");
    default:
        return 0;
    }
    return 0;
}

static int print_int_named_day(BIO *out, int64_t nd)
{
    switch (nd) {
    case (OSSL_NAMED_DAY_INT_SUN):
        return BIO_puts(out, "SUN");
    case (OSSL_NAMED_DAY_INT_MON):
        return BIO_puts(out, "MON");
    case (OSSL_NAMED_DAY_INT_TUE):
        return BIO_puts(out, "TUE");
    case (OSSL_NAMED_DAY_INT_WED):
        return BIO_puts(out, "WED");
    case (OSSL_NAMED_DAY_INT_THU):
        return BIO_puts(out, "THU");
    case (OSSL_NAMED_DAY_INT_FRI):
        return BIO_puts(out, "FRI");
    case (OSSL_NAMED_DAY_INT_SAT):
        return BIO_puts(out, "SAT");
    default:
        return 0;
    }
    return 0;
}

static int print_bit_named_day(BIO *out, ASN1_BIT_STRING *bs)
{
    return print_day_of_week(out, bs);
}

static int i2r_OSSL_PERIOD(X509V3_EXT_METHOD *method,
                           OSSL_TIME_PERIOD *p,
                           BIO *out, int indent)
{
    int i;
    OSSL_DAY_TIME_BAND *band;
    ASN1_INTEGER *big_val;
    int64_t small_val;
    OSSL_NAMED_DAY *nd;

    if (BIO_printf(out, "%*sPeriod:\n", indent, "") <= 0)
        return 0;
    if (p->timesOfDay) {
        if (BIO_printf(out, "%*sDaytime bands:\n", indent + 4, "") <= 0)
            return 0;
        for (i = 0; i < sk_OSSL_DAY_TIME_BAND_num(p->timesOfDay); i++) {
            band = sk_OSSL_DAY_TIME_BAND_value(p->timesOfDay, i);
            if (BIO_printf(out, "%*s", indent + 8, "") <= 0)
                return 0;
            if (!i2r_OSSL_DAY_TIME_BAND(method, band, out, indent + 8))
                return 0;
            if (!BIO_puts(out, "\n"))
                return 0;
        }
    }
    if (p->days) {
        if (p->days->type == OSSL_TIME_SPEC_DAY_TYPE_INT) {
            if (p->weeks != NULL) {
                if (BIO_printf(out, "%*sDays of the week: ", indent + 4, "") <= 0)
                    return 0;
            } else if (p->months != NULL) {
                if (BIO_printf(out, "%*sDays of the month: ", indent + 4, "") <= 0)
                    return 0;
            } else if (p->years != NULL) {
                if (BIO_printf(out, "%*sDays of the year: ", indent + 4, "") <= 0)
                    return 0;
            }
        } else {
            if (BIO_printf(out, "%*sDays: ", indent + 4, "") <= 0)
                return 0;
        }

        switch (p->days->type) {
        case (OSSL_TIME_SPEC_DAY_TYPE_INT):
            for (i = 0; i < sk_ASN1_INTEGER_num(p->days->choice.intDay); i++) {
                big_val = sk_ASN1_INTEGER_value(p->days->choice.intDay, i);
                if (!ASN1_INTEGER_get_int64(&small_val, big_val))
                    return 0;
                if (i > 0 && !BIO_puts(out, ", "))
                    return 0;
                /* If weeks is defined, then print day of week by name. */
                if (p->weeks != NULL) {
                    if (!print_int_day_of_week(out, small_val))
                        return 0;
                } else if (BIO_printf(out, "%lld", (long long int)small_val) <= 0) {
                    return 0;
                }
            }
            break;
        case (OSSL_TIME_SPEC_DAY_TYPE_BIT):
            if (!print_day_of_week(out, p->days->choice.bitDay))
                return 0;
            break;
        case (OSSL_TIME_SPEC_DAY_TYPE_DAY_OF):
            switch (p->days->choice.dayOf->type) {
            case (OSSL_TIME_SPEC_X_DAY_OF_FIRST):
                if (!BIO_puts(out, "FIRST "))
                    return 0;
                nd = p->days->choice.dayOf->choice.first;
                break;
            case (OSSL_TIME_SPEC_X_DAY_OF_SECOND):
                if (!BIO_puts(out, "SECOND "))
                    return 0;
                nd = p->days->choice.dayOf->choice.second;
                break;
            case (OSSL_TIME_SPEC_X_DAY_OF_THIRD):
                if (!BIO_puts(out, "THIRD "))
                    return 0;
                nd = p->days->choice.dayOf->choice.third;
                break;
            case (OSSL_TIME_SPEC_X_DAY_OF_FOURTH):
                if (!BIO_puts(out, "FOURTH "))
                    return 0;
                nd = p->days->choice.dayOf->choice.fourth;
                break;
            case (OSSL_TIME_SPEC_X_DAY_OF_FIFTH):
                if (!BIO_puts(out, "FIFTH "))
                    return 0;
                nd = p->days->choice.dayOf->choice.fifth;
                break;
            default:
                return 0;
            }
            switch (nd->type) {
            case (OSSL_NAMED_DAY_TYPE_INT):
                if (!ASN1_INTEGER_get_int64(&small_val, nd->choice.intNamedDays))
                    return 0;
                if (!print_int_named_day(out, small_val))
                    return 0;
                break;
            case (OSSL_NAMED_DAY_TYPE_BIT):
                if (!print_bit_named_day(out, nd->choice.bitNamedDays))
                    return 0;
                break;
            default:
                return 0;
            }
            break;
        default:
            return 0;
        }
        if (!BIO_puts(out, "\n"))
            return 0;
    }
    if (p->weeks) {
        if (p->weeks->type == OSSL_TIME_SPEC_WEEKS_TYPE_INT) {
            if (p->months != NULL) {
                if (BIO_printf(out, "%*sWeeks of the month: ", indent + 4, "") <= 0)
                    return 0;
            } else if (p->years != NULL) {
                if (BIO_printf(out, "%*sWeeks of the year: ", indent + 4, "") <= 0)
                    return 0;
            }
        } else {
            if (BIO_printf(out, "%*sWeeks: ", indent + 4, "") <= 0)
                return 0;
        }

        switch (p->weeks->type) {
        case (OSSL_TIME_SPEC_WEEKS_TYPE_ALL):
            if (!BIO_puts(out, "ALL"))
                return 0;
            break;
        case (OSSL_TIME_SPEC_WEEKS_TYPE_INT):
            for (i = 0; i < sk_ASN1_INTEGER_num(p->weeks->choice.intWeek); i++) {
                big_val = sk_ASN1_INTEGER_value(p->weeks->choice.intWeek, i);
                if (!ASN1_INTEGER_get_int64(&small_val, big_val))
                    return 0;
                if (i > 0 && !BIO_puts(out, ", "))
                    return 0;
                if (!BIO_printf(out, "%lld", (long long int)small_val))
                    return 0;
            }
            break;
        case (OSSL_TIME_SPEC_WEEKS_TYPE_BIT):
            if (!print_bit_week(out, p->weeks->choice.bitWeek))
                return 0;
            break;
        default:
            return 0;
        }
        if (!BIO_puts(out, "\n"))
            return 0;
    }
    if (p->months) {
        if (BIO_printf(out, "%*sMonths: ", indent + 4, "") <= 0)
            return 0;
        switch (p->months->type) {
        case (OSSL_TIME_SPEC_MONTH_TYPE_ALL):
            if (!BIO_puts(out, "ALL"))
                return 0;
            break;
        case (OSSL_TIME_SPEC_MONTH_TYPE_INT):
            for (i = 0; i < sk_ASN1_INTEGER_num(p->months->choice.intMonth); i++) {
                big_val = sk_ASN1_INTEGER_value(p->months->choice.intMonth, i);
                if (!ASN1_INTEGER_get_int64(&small_val, big_val))
                    return 0;
                if (i > 0 && !BIO_puts(out, ", "))
                    return 0;
                if (!print_int_month(out, small_val))
                    return 0;
            }
            break;
        case (OSSL_TIME_SPEC_MONTH_TYPE_BIT):
            if (!print_bit_month(out, p->months->choice.bitMonth))
                return 0;
            break;
        default:
            return 0;
        }
        if (!BIO_puts(out, "\n"))
            return 0;
    }
    if (p->years) {
        if (BIO_printf(out, "%*sYears: ", indent + 4, "") <= 0)
            return 0;
        for (i = 0; i < sk_ASN1_INTEGER_num(p->years); i++) {
            big_val = sk_ASN1_INTEGER_value(p->years, i);
            if (!ASN1_INTEGER_get_int64(&small_val, big_val))
                return 0;
            if (i > 0 && !BIO_puts(out, ", "))
                return 0;
            if (BIO_printf(out, "%04lld", (long long int)small_val) <= 0)
                return 0;
        }
    }
    return 1;
}

static int i2r_OSSL_TIME_SPEC_TIME(X509V3_EXT_METHOD *method,
                                   OSSL_TIME_SPEC_TIME *time,
                                   BIO *out, int indent)
{
    OSSL_TIME_PERIOD *tp;
    int i;

    switch (time->type) {
    case (OSSL_TIME_SPEC_TIME_TYPE_ABSOLUTE):
        if (BIO_printf(out, "%*sAbsolute: ", indent, "") <= 0)
            return 0;
        if (i2r_OSSL_TIME_SPEC_ABSOLUTE(method, time->choice.absolute, out, indent + 4) <= 0)
            return 0;
        return BIO_puts(out, "\n");
    case (OSSL_TIME_SPEC_TIME_TYPE_PERIODIC):
        if (BIO_printf(out, "%*sPeriodic:\n", indent, "") <= 0)
            return 0;
        for (i = 0; i < sk_OSSL_TIME_PERIOD_num(time->choice.periodic); i++) {
            if (i > 0 && !BIO_puts(out, "\n"))
                return 0;
            tp = sk_OSSL_TIME_PERIOD_value(time->choice.periodic, i);
            if (!i2r_OSSL_PERIOD(method, tp, out, indent + 4))
                return 0;
        }
        return BIO_puts(out, "\n");
    default:
        return 0;
    }
    return 0;
}

static int i2r_OSSL_TIME_SPEC(X509V3_EXT_METHOD *method,
                              OSSL_TIME_SPEC *time,
                              BIO *out, int indent)
{
    int64_t tz;

    if (time->timeZone) {
        if (ASN1_INTEGER_get_int64(&tz, time->timeZone) != 1)
            return 0;
        if (BIO_printf(out, "%*sTimezone: UTC%+03lld:00\n", indent, "", (long long int)tz) <= 0)
            return 0;
    }
    if (time->notThisTime > 0) {
        if (BIO_printf(out, "%*sNOT this time:\n", indent, "") <= 0)
            return 0;
    } else if (BIO_printf(out, "%*sTime:\n", indent, "") <= 0) {
        return 0;
    }
    return i2r_OSSL_TIME_SPEC_TIME(method, time->time, out, indent + 4);
}

const X509V3_EXT_METHOD ossl_v3_time_specification = {
    NID_time_specification, X509V3_EXT_MULTILINE,
    ASN1_ITEM_ref(OSSL_TIME_SPEC),
    0, 0, 0, 0,
    0, 0,
    0,
    0,
    (X509V3_EXT_I2R)i2r_OSSL_TIME_SPEC,
    NULL,
    NULL
};
