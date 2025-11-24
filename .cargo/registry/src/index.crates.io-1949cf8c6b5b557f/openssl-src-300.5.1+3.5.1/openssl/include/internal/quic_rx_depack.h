/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_RX_DEPACK_H
# define OSSL_QUIC_RX_DEPACK_H

# include "internal/quic_channel.h"

# ifndef OPENSSL_NO_QUIC

int ossl_quic_handle_frames(QUIC_CHANNEL *qc, OSSL_QRX_PKT *qpacket);

# endif

#endif
