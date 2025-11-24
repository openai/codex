/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_statm.h"

void ossl_statm_update_rtt(OSSL_STATM *statm,
                           OSSL_TIME ack_delay,
                           OSSL_TIME override_latest_rtt)
{
    OSSL_TIME adjusted_rtt, latest_rtt = override_latest_rtt;

    /* Use provided RTT value, or else last RTT value. */
    if (ossl_time_is_zero(latest_rtt))
        latest_rtt = statm->latest_rtt;
    else
        statm->latest_rtt = latest_rtt;

    if (!statm->have_first_sample) {
        statm->min_rtt              = latest_rtt;
        statm->smoothed_rtt         = latest_rtt;
        statm->rtt_variance         = ossl_time_divide(latest_rtt, 2);
        statm->have_first_sample    = 1;
        return;
    }

    /* Update minimum RTT. */
    if (ossl_time_compare(latest_rtt, statm->min_rtt) < 0)
        statm->min_rtt = latest_rtt;

    /*
     * Enforcement of max_ack_delay is the responsibility of
     * the caller as it is context-dependent.
     */

    adjusted_rtt = latest_rtt;
    if (ossl_time_compare(latest_rtt, ossl_time_add(statm->min_rtt, ack_delay)) >= 0)
        adjusted_rtt = ossl_time_subtract(latest_rtt, ack_delay);

    statm->rtt_variance = ossl_time_divide(ossl_time_add(ossl_time_multiply(statm->rtt_variance, 3),
                                                         ossl_time_abs_difference(statm->smoothed_rtt,
                                                                              adjusted_rtt)), 4);
    statm->smoothed_rtt = ossl_time_divide(ossl_time_add(ossl_time_multiply(statm->smoothed_rtt, 7),
                                                         adjusted_rtt), 8);
}

/* RFC 9002 kInitialRtt value. RFC recommended value. */
#define K_INITIAL_RTT               ossl_ms2time(333)

int ossl_statm_init(OSSL_STATM *statm)
{
    statm->smoothed_rtt             = K_INITIAL_RTT;
    statm->latest_rtt               = ossl_time_zero();
    statm->min_rtt                  = ossl_time_infinite();
    statm->rtt_variance             = ossl_time_divide(K_INITIAL_RTT, 2);
    statm->have_first_sample        = 0;
    return 1;
}

void ossl_statm_destroy(OSSL_STATM *statm)
{
    /* No-op. */
}

void ossl_statm_get_rtt_info(OSSL_STATM *statm, OSSL_RTT_INFO *rtt_info)
{
    rtt_info->min_rtt           = statm->min_rtt;
    rtt_info->latest_rtt        = statm->latest_rtt;
    rtt_info->smoothed_rtt      = statm->smoothed_rtt;
    rtt_info->rtt_variance      = statm->rtt_variance;
}
