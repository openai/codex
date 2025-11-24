/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_STATS_H
# define OSSL_QUIC_STATS_H

# include <openssl/ssl.h>
# include "internal/time.h"
# include "internal/quic_predef.h"

# ifndef OPENSSL_NO_QUIC

struct ossl_statm_st {
    OSSL_TIME smoothed_rtt, latest_rtt, min_rtt, rtt_variance;
    char      have_first_sample;
};

typedef struct ossl_rtt_info_st {
    /* As defined in RFC 9002. */
    OSSL_TIME smoothed_rtt, latest_rtt, rtt_variance, min_rtt;
} OSSL_RTT_INFO;

int ossl_statm_init(OSSL_STATM *statm);

void ossl_statm_destroy(OSSL_STATM *statm);

void ossl_statm_get_rtt_info(OSSL_STATM *statm, OSSL_RTT_INFO *rtt_info);

void ossl_statm_update_rtt(OSSL_STATM *statm,
                           OSSL_TIME ack_delay,
                           OSSL_TIME override_latest_rtt);

# endif

#endif
