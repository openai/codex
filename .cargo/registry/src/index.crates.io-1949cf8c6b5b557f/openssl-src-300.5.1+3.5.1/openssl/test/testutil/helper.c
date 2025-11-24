/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <time.h>
#include <openssl/asn1t.h>
#include "../testutil.h"

/*
 * tweak for Windows
 */
#ifdef WIN32
# define timezone _timezone
#endif

#if defined(__FreeBSD__) || defined(__wasi__)
# define USE_TIMEGM
#endif

time_t test_asn1_string_to_time_t(const char *asn1_string)
{
    ASN1_TIME *timestamp_asn1 = NULL;
    struct tm *timestamp_tm = NULL;
#if defined(__DJGPP__)
    char *tz = NULL;
#elif !defined(USE_TIMEGM)
    time_t timestamp_local;
#endif
    time_t timestamp_utc;

    timestamp_asn1 = ASN1_TIME_new();
    if(timestamp_asn1 == NULL)
        return -1;
    if (!ASN1_TIME_set_string(timestamp_asn1, asn1_string))
    {
        ASN1_TIME_free(timestamp_asn1);
        return -1;
    }

    timestamp_tm = OPENSSL_malloc(sizeof(*timestamp_tm));
    if (timestamp_tm == NULL) {
        ASN1_TIME_free(timestamp_asn1);
        return -1;
    }
    if (!(ASN1_TIME_to_tm(timestamp_asn1, timestamp_tm))) {
        OPENSSL_free(timestamp_tm);
        ASN1_TIME_free(timestamp_asn1);
        return -1;
    }
    ASN1_TIME_free(timestamp_asn1);

#if defined(__DJGPP__)
    /*
     * This is NOT thread-safe.  Do not use this method for platforms other
     * than djgpp.
     */
    tz = getenv("TZ");
    if (tz != NULL) {
        tz = OPENSSL_strdup(tz);
        if (tz == NULL) {
            OPENSSL_free(timestamp_tm);
            return -1;
        }
    }
    setenv("TZ", "UTC", 1);

    timestamp_utc = mktime(timestamp_tm);

    if (tz != NULL) {
        setenv("TZ", tz, 1);
        OPENSSL_free(tz);
    } else {
        unsetenv("TZ");
    }
#elif defined(USE_TIMEGM)
    timestamp_utc = timegm(timestamp_tm);
#else
    timestamp_local = mktime(timestamp_tm);
    timestamp_utc = timestamp_local - timezone;
#endif
    OPENSSL_free(timestamp_tm);

    return timestamp_utc;
}
