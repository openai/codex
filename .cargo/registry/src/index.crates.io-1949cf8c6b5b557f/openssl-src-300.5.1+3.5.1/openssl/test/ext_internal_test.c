/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/nelem.h"
#include "../ssl/ssl_local.h"
#include "../ssl/statem/statem_local.h"
#include "testutil.h"

#define EXT_ENTRY(name) { TLSEXT_IDX_##name, TLSEXT_TYPE_##name, #name }
#define EXT_EXCEPTION(name) { TLSEXT_IDX_##name, TLSEXT_TYPE_invalid, #name }
#define EXT_END(name) { TLSEXT_IDX_##name, TLSEXT_TYPE_out_of_range, #name }

typedef struct {
    size_t idx;
    unsigned int type;
    char *name;
} EXT_LIST;

/* The order here does matter! */
static EXT_LIST ext_list[] = {

    EXT_ENTRY(renegotiate),
    EXT_ENTRY(server_name),
    EXT_ENTRY(max_fragment_length),
#ifndef OPENSSL_NO_SRP
    EXT_ENTRY(srp),
#else
    EXT_EXCEPTION(srp),
#endif
    EXT_ENTRY(ec_point_formats),
    EXT_ENTRY(supported_groups),
    EXT_ENTRY(session_ticket),
#ifndef OPENSSL_NO_OCSP
    EXT_ENTRY(status_request),
#else
    EXT_EXCEPTION(status_request),
#endif
#ifndef OPENSSL_NO_NEXTPROTONEG
    EXT_ENTRY(next_proto_neg),
#else
    EXT_EXCEPTION(next_proto_neg),
#endif
    EXT_ENTRY(application_layer_protocol_negotiation),
#ifndef OPENSSL_NO_SRTP
    EXT_ENTRY(use_srtp),
#else
    EXT_EXCEPTION(use_srtp),
#endif
    EXT_ENTRY(encrypt_then_mac),
#ifndef OPENSSL_NO_CT
    EXT_ENTRY(signed_certificate_timestamp),
#else
    EXT_EXCEPTION(signed_certificate_timestamp),
#endif
    EXT_ENTRY(extended_master_secret),
    EXT_ENTRY(signature_algorithms_cert),
    EXT_ENTRY(post_handshake_auth),
    EXT_ENTRY(client_cert_type),
    EXT_ENTRY(server_cert_type),
    EXT_ENTRY(signature_algorithms),
    EXT_ENTRY(supported_versions),
    EXT_ENTRY(psk_kex_modes),
    EXT_ENTRY(key_share),
    EXT_ENTRY(cookie),
    EXT_ENTRY(cryptopro_bug),
    EXT_ENTRY(compress_certificate),
    EXT_ENTRY(early_data),
    EXT_ENTRY(certificate_authorities),
    EXT_ENTRY(padding),
    EXT_ENTRY(psk),
    EXT_END(num_builtins)
};

static int test_extension_list(void)
{
    size_t n = OSSL_NELEM(ext_list);
    size_t i;
    unsigned int type;
    int retval = 1;

    for (i = 0; i < n; i++) {
        if (!TEST_size_t_eq(i, ext_list[i].idx)) {
            retval = 0;
            TEST_error("TLSEXT_IDX_%s=%zd, found at=%zd\n",
                       ext_list[i].name, ext_list[i].idx, i);
        }
        type = ossl_get_extension_type(ext_list[i].idx);
        if (!TEST_uint_eq(type, ext_list[i].type)) {
            retval = 0;
            TEST_error("TLSEXT_IDX_%s=%zd expected=0x%05X got=0x%05X",
                       ext_list[i].name, ext_list[i].idx, ext_list[i].type,
                       type);
        }
    }
    return retval;
}

int setup_tests(void)
{
    ADD_TEST(test_extension_list);
    return 1;
}
