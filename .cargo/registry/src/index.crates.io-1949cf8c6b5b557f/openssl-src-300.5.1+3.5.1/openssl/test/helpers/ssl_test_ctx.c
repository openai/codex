/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>

#include <openssl/e_os2.h>
#include <openssl/crypto.h>

#include "internal/nelem.h"
#include "ssl_test_ctx.h"
#include "../testutil.h"

static const int default_app_data_size = 256;
/* Default set to be as small as possible to exercise fragmentation. */
static const int default_max_fragment_size = 512;

static int parse_boolean(const char *value, int *result)
{
    if (OPENSSL_strcasecmp(value, "Yes") == 0) {
        *result = 1;
        return 1;
    }
    else if (OPENSSL_strcasecmp(value, "No") == 0) {
        *result = 0;
        return 1;
    }
    TEST_error("parse_boolean given: '%s'", value);
    return 0;
}

#define IMPLEMENT_SSL_TEST_BOOL_OPTION(struct_type, name, field)        \
    static int parse_##name##_##field(struct_type *ctx, const char *value) \
    {                                                                   \
        return parse_boolean(value, &ctx->field);                       \
    }

#define IMPLEMENT_SSL_TEST_STRING_OPTION(struct_type, name, field)      \
    static int parse_##name##_##field(struct_type *ctx, const char *value) \
    {                                                                   \
        OPENSSL_free(ctx->field);                                       \
        ctx->field = OPENSSL_strdup(value);                             \
        return TEST_ptr(ctx->field);                                    \
    }

#define IMPLEMENT_SSL_TEST_INT_OPTION(struct_type, name, field)        \
    static int parse_##name##_##field(struct_type *ctx, const char *value) \
    {                                                                   \
        ctx->field = atoi(value);                                       \
        return 1;                                                       \
    }

/* True enums and other test configuration values that map to an int. */
typedef struct {
    const char *name;
    int value;
} test_enum;


__owur static int parse_enum(const test_enum *enums, size_t num_enums,
                             int *value, const char *name)
{
    size_t i;
    for (i = 0; i < num_enums; i++) {
        if (strcmp(enums[i].name, name) == 0) {
            *value = enums[i].value;
            return 1;
        }
    }
    return 0;
}

static const char *enum_name(const test_enum *enums, size_t num_enums,
                             int value)
{
    size_t i;
    for (i = 0; i < num_enums; i++) {
        if (enums[i].value == value) {
            return enums[i].name;
        }
    }
    return "InvalidValue";
}


/* ExpectedResult */

static const test_enum ssl_test_results[] = {
    {"Success", SSL_TEST_SUCCESS},
    {"ServerFail", SSL_TEST_SERVER_FAIL},
    {"ClientFail", SSL_TEST_CLIENT_FAIL},
    {"InternalError", SSL_TEST_INTERNAL_ERROR},
    {"FirstHandshakeFailed", SSL_TEST_FIRST_HANDSHAKE_FAILED},
};

__owur static int parse_expected_result(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_test_results, OSSL_NELEM(ssl_test_results),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->expected_result = ret_value;
    return 1;
}

const char *ssl_test_result_name(ssl_test_result_t result)
{
    return enum_name(ssl_test_results, OSSL_NELEM(ssl_test_results), result);
}

/* ExpectedClientAlert / ExpectedServerAlert */

static const test_enum ssl_alerts[] = {
    {"UnknownCA", SSL_AD_UNKNOWN_CA},
    {"HandshakeFailure", SSL_AD_HANDSHAKE_FAILURE},
    {"UnrecognizedName", SSL_AD_UNRECOGNIZED_NAME},
    {"NoRenegotiation", SSL_AD_NO_RENEGOTIATION},
    {"BadCertificate", SSL_AD_BAD_CERTIFICATE},
    {"NoApplicationProtocol", SSL_AD_NO_APPLICATION_PROTOCOL},
    {"CertificateRequired", SSL_AD_CERTIFICATE_REQUIRED},
};

__owur static int parse_alert(int *alert, const char *value)
{
    return parse_enum(ssl_alerts, OSSL_NELEM(ssl_alerts), alert, value);
}

__owur static int parse_client_alert(SSL_TEST_CTX *test_ctx, const char *value)
{
    return parse_alert(&test_ctx->expected_client_alert, value);
}

__owur static int parse_server_alert(SSL_TEST_CTX *test_ctx, const char *value)
{
    return parse_alert(&test_ctx->expected_server_alert, value);
}

const char *ssl_alert_name(int alert)
{
    return enum_name(ssl_alerts, OSSL_NELEM(ssl_alerts), alert);
}

/* ExpectedProtocol */

static const test_enum ssl_protocols[] = {
     {"TLSv1.3", TLS1_3_VERSION},
     {"TLSv1.2", TLS1_2_VERSION},
     {"TLSv1.1", TLS1_1_VERSION},
     {"TLSv1", TLS1_VERSION},
     {"SSLv3", SSL3_VERSION},
     {"DTLSv1", DTLS1_VERSION},
     {"DTLSv1.2", DTLS1_2_VERSION},
};

__owur static int parse_protocol(SSL_TEST_CTX *test_ctx, const char *value)
{
    return parse_enum(ssl_protocols, OSSL_NELEM(ssl_protocols),
                      &test_ctx->expected_protocol, value);
}

const char *ssl_protocol_name(int protocol)
{
    return enum_name(ssl_protocols, OSSL_NELEM(ssl_protocols), protocol);
}

/* VerifyCallback */

static const test_enum ssl_verify_callbacks[] = {
    {"None", SSL_TEST_VERIFY_NONE},
    {"AcceptAll", SSL_TEST_VERIFY_ACCEPT_ALL},
    {"RetryOnce", SSL_TEST_VERIFY_RETRY_ONCE},
    {"RejectAll", SSL_TEST_VERIFY_REJECT_ALL},
};

__owur static int parse_client_verify_callback(SSL_TEST_CLIENT_CONF *client_conf,
                                               const char *value)
{
    int ret_value;

    if (!parse_enum(ssl_verify_callbacks, OSSL_NELEM(ssl_verify_callbacks),
                    &ret_value, value)) {
        return 0;
    }
    client_conf->verify_callback = ret_value;
    return 1;
}

const char *ssl_verify_callback_name(ssl_verify_callback_t callback)
{
    return enum_name(ssl_verify_callbacks, OSSL_NELEM(ssl_verify_callbacks),
                     callback);
}

/* ServerName */

static const test_enum ssl_servername[] = {
    {"None", SSL_TEST_SERVERNAME_NONE},
    {"server1", SSL_TEST_SERVERNAME_SERVER1},
    {"server2", SSL_TEST_SERVERNAME_SERVER2},
    {"invalid", SSL_TEST_SERVERNAME_INVALID},
};

__owur static int parse_servername(SSL_TEST_CLIENT_CONF *client_conf,
                                   const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_servername, OSSL_NELEM(ssl_servername),
                    &ret_value, value)) {
        return 0;
    }
    client_conf->servername = ret_value;
    return 1;
}

__owur static int parse_expected_servername(SSL_TEST_CTX *test_ctx,
                                            const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_servername, OSSL_NELEM(ssl_servername),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->expected_servername = ret_value;
    return 1;
}

const char *ssl_servername_name(ssl_servername_t server)
{
    return enum_name(ssl_servername, OSSL_NELEM(ssl_servername),
                     server);
}

/* ServerNameCallback */

static const test_enum ssl_servername_callbacks[] = {
    {"None", SSL_TEST_SERVERNAME_CB_NONE},
    {"IgnoreMismatch", SSL_TEST_SERVERNAME_IGNORE_MISMATCH},
    {"RejectMismatch", SSL_TEST_SERVERNAME_REJECT_MISMATCH},
    {"ClientHelloIgnoreMismatch",
     SSL_TEST_SERVERNAME_CLIENT_HELLO_IGNORE_MISMATCH},
    {"ClientHelloRejectMismatch",
     SSL_TEST_SERVERNAME_CLIENT_HELLO_REJECT_MISMATCH},
    {"ClientHelloNoV12", SSL_TEST_SERVERNAME_CLIENT_HELLO_NO_V12},
};

__owur static int parse_servername_callback(SSL_TEST_SERVER_CONF *server_conf,
                                            const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_servername_callbacks,
                    OSSL_NELEM(ssl_servername_callbacks), &ret_value, value)) {
        return 0;
    }
    server_conf->servername_callback = ret_value;
    return 1;
}

const char *ssl_servername_callback_name(ssl_servername_callback_t callback)
{
    return enum_name(ssl_servername_callbacks,
                     OSSL_NELEM(ssl_servername_callbacks), callback);
}

/* SessionTicketExpected */

static const test_enum ssl_session_ticket[] = {
    {"Ignore", SSL_TEST_SESSION_TICKET_IGNORE},
    {"Yes", SSL_TEST_SESSION_TICKET_YES},
    {"No", SSL_TEST_SESSION_TICKET_NO},
};

__owur static int parse_session_ticket(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_session_ticket, OSSL_NELEM(ssl_session_ticket),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->session_ticket_expected = ret_value;
    return 1;
}

const char *ssl_session_ticket_name(ssl_session_ticket_t server)
{
    return enum_name(ssl_session_ticket,
                     OSSL_NELEM(ssl_session_ticket),
                     server);
}

/* CompressionExpected */

IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, compression_expected)

/* SessionIdExpected */

static const test_enum ssl_session_id[] = {
    {"Ignore", SSL_TEST_SESSION_ID_IGNORE},
    {"Yes", SSL_TEST_SESSION_ID_YES},
    {"No", SSL_TEST_SESSION_ID_NO},
};

__owur static int parse_session_id(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_session_id, OSSL_NELEM(ssl_session_id),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->session_id_expected = ret_value;
    return 1;
}

const char *ssl_session_id_name(ssl_session_id_t server)
{
    return enum_name(ssl_session_id,
                     OSSL_NELEM(ssl_session_id),
                     server);
}

/* Method */

static const test_enum ssl_test_methods[] = {
    {"TLS", SSL_TEST_METHOD_TLS},
    {"DTLS", SSL_TEST_METHOD_DTLS},
    {"QUIC", SSL_TEST_METHOD_QUIC}
};

__owur static int parse_test_method(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_test_methods, OSSL_NELEM(ssl_test_methods),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->method = ret_value;
    return 1;
}

const char *ssl_test_method_name(ssl_test_method_t method)
{
    return enum_name(ssl_test_methods, OSSL_NELEM(ssl_test_methods), method);
}

/* NPN and ALPN options */

IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CLIENT_CONF, client, npn_protocols)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_SERVER_CONF, server, npn_protocols)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CTX, test, expected_npn_protocol)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CLIENT_CONF, client, alpn_protocols)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_SERVER_CONF, server, alpn_protocols)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CTX, test, expected_alpn_protocol)

/* SRP options */
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CLIENT_CONF, client, srp_user)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_SERVER_CONF, server, srp_user)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CLIENT_CONF, client, srp_password)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_SERVER_CONF, server, srp_password)

/* Session Ticket App Data options */
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CTX, test, expected_session_ticket_app_data)
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_SERVER_CONF, server, session_ticket_app_data)

/* Handshake mode */

static const test_enum ssl_handshake_modes[] = {
    {"Simple", SSL_TEST_HANDSHAKE_SIMPLE},
    {"Resume", SSL_TEST_HANDSHAKE_RESUME},
    {"RenegotiateServer", SSL_TEST_HANDSHAKE_RENEG_SERVER},
    {"RenegotiateClient", SSL_TEST_HANDSHAKE_RENEG_CLIENT},
    {"KeyUpdateServer", SSL_TEST_HANDSHAKE_KEY_UPDATE_SERVER},
    {"KeyUpdateClient", SSL_TEST_HANDSHAKE_KEY_UPDATE_CLIENT},
    {"PostHandshakeAuth", SSL_TEST_HANDSHAKE_POST_HANDSHAKE_AUTH},
};

__owur static int parse_handshake_mode(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_handshake_modes, OSSL_NELEM(ssl_handshake_modes),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->handshake_mode = ret_value;
    return 1;
}

const char *ssl_handshake_mode_name(ssl_handshake_mode_t mode)
{
    return enum_name(ssl_handshake_modes, OSSL_NELEM(ssl_handshake_modes),
                     mode);
}

/* Renegotiation Ciphersuites */

IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CLIENT_CONF, client, reneg_ciphers)

/* KeyUpdateType */

static const test_enum ssl_key_update_types[] = {
    {"KeyUpdateRequested", SSL_KEY_UPDATE_REQUESTED},
    {"KeyUpdateNotRequested", SSL_KEY_UPDATE_NOT_REQUESTED},
};

__owur static int parse_key_update_type(SSL_TEST_CTX *test_ctx, const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_key_update_types, OSSL_NELEM(ssl_key_update_types),
                    &ret_value, value)) {
        return 0;
    }
    test_ctx->key_update_type = ret_value;
    return 1;
}

/* CT Validation */

static const test_enum ssl_ct_validation_modes[] = {
    {"None", SSL_TEST_CT_VALIDATION_NONE},
    {"Permissive", SSL_TEST_CT_VALIDATION_PERMISSIVE},
    {"Strict", SSL_TEST_CT_VALIDATION_STRICT},
};

__owur static int parse_ct_validation(SSL_TEST_CLIENT_CONF *client_conf,
                                      const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_ct_validation_modes, OSSL_NELEM(ssl_ct_validation_modes),
                    &ret_value, value)) {
        return 0;
    }
    client_conf->ct_validation = ret_value;
    return 1;
}

const char *ssl_ct_validation_name(ssl_ct_validation_t mode)
{
    return enum_name(ssl_ct_validation_modes, OSSL_NELEM(ssl_ct_validation_modes),
                     mode);
}

IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, resumption_expected)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_SERVER_CONF, server, broken_session_ticket)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, use_sctp)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, compress_certificates)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, enable_client_sctp_label_bug)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CTX, test, enable_server_sctp_label_bug)

/* CertStatus */

static const test_enum ssl_certstatus[] = {
    {"None", SSL_TEST_CERT_STATUS_NONE},
    {"GoodResponse", SSL_TEST_CERT_STATUS_GOOD_RESPONSE},
    {"BadResponse", SSL_TEST_CERT_STATUS_BAD_RESPONSE}
};

__owur static int parse_certstatus(SSL_TEST_SERVER_CONF *server_conf,
                                            const char *value)
{
    int ret_value;
    if (!parse_enum(ssl_certstatus, OSSL_NELEM(ssl_certstatus), &ret_value,
                    value)) {
        return 0;
    }
    server_conf->cert_status = ret_value;
    return 1;
}

const char *ssl_certstatus_name(ssl_cert_status_t cert_status)
{
    return enum_name(ssl_certstatus,
                     OSSL_NELEM(ssl_certstatus), cert_status);
}

/* ApplicationData */

IMPLEMENT_SSL_TEST_INT_OPTION(SSL_TEST_CTX, test, app_data_size)


/* MaxFragmentSize */

IMPLEMENT_SSL_TEST_INT_OPTION(SSL_TEST_CTX, test, max_fragment_size)

/* Maximum-Fragment-Length TLS extension mode */
static const test_enum ssl_max_fragment_len_mode[] = {
    {"None", TLSEXT_max_fragment_length_DISABLED},
    { "512", TLSEXT_max_fragment_length_512},
    {"1024", TLSEXT_max_fragment_length_1024},
    {"2048", TLSEXT_max_fragment_length_2048},
    {"4096", TLSEXT_max_fragment_length_4096}
};

__owur static int parse_max_fragment_len_mode(SSL_TEST_CLIENT_CONF *client_conf,
                                              const char *value)
{
    int ret_value;

    if (!parse_enum(ssl_max_fragment_len_mode,
                    OSSL_NELEM(ssl_max_fragment_len_mode), &ret_value, value)) {
        return 0;
    }
    client_conf->max_fragment_len_mode = ret_value;
    return 1;
}

const char *ssl_max_fragment_len_name(int MFL_mode)
{
    return enum_name(ssl_max_fragment_len_mode,
                     OSSL_NELEM(ssl_max_fragment_len_mode), MFL_mode);
}


/* Expected key and signature types */

__owur static int parse_expected_key_type(int *ptype, const char *value)
{
    int nid;
    const EVP_PKEY_ASN1_METHOD *ameth;

    if (value == NULL)
        return 0;
    ameth = EVP_PKEY_asn1_find_str(NULL, value, -1);
    if (ameth != NULL)
        EVP_PKEY_asn1_get0_info(&nid, NULL, NULL, NULL, NULL, ameth);
    else
        nid = OBJ_sn2nid(value);
    if (nid == NID_undef)
        nid = OBJ_ln2nid(value);
#ifndef OPENSSL_NO_EC
    if (nid == NID_undef)
        nid = EC_curve_nist2nid(value);
#endif
    switch (nid) {
    case NID_brainpoolP256r1tls13:
        nid = NID_brainpoolP256r1;
        break;
    case NID_brainpoolP384r1tls13:
        nid = NID_brainpoolP384r1;
        break;
    case NID_brainpoolP512r1tls13:
        nid = NID_brainpoolP512r1;
        break;
    }
    if (nid == NID_undef)
        return 0;
    *ptype = nid;
    return 1;
}

__owur static int parse_expected_tmp_key_type(SSL_TEST_CTX *test_ctx,
                                              const char *value)
{
    return parse_expected_key_type(&test_ctx->expected_tmp_key_type, value);
}

__owur static int parse_expected_server_cert_type(SSL_TEST_CTX *test_ctx,
                                                  const char *value)
{
    return parse_expected_key_type(&test_ctx->expected_server_cert_type,
                                   value);
}

__owur static int parse_expected_server_sign_type(SSL_TEST_CTX *test_ctx,
                                                 const char *value)
{
    return parse_expected_key_type(&test_ctx->expected_server_sign_type,
                                   value);
}

__owur static int parse_expected_client_cert_type(SSL_TEST_CTX *test_ctx,
                                                  const char *value)
{
    return parse_expected_key_type(&test_ctx->expected_client_cert_type,
                                   value);
}

__owur static int parse_expected_client_sign_type(SSL_TEST_CTX *test_ctx,
                                                 const char *value)
{
    return parse_expected_key_type(&test_ctx->expected_client_sign_type,
                                   value);
}


/* Expected signing hash */

__owur static int parse_expected_sign_hash(int *ptype, const char *value)
{
    int nid;

    if (value == NULL)
        return 0;
    nid = OBJ_sn2nid(value);
    if (nid == NID_undef)
        nid = OBJ_ln2nid(value);
    if (nid == NID_undef)
        return 0;
    *ptype = nid;
    return 1;
}

__owur static int parse_expected_server_sign_hash(SSL_TEST_CTX *test_ctx,
                                                  const char *value)
{
    return parse_expected_sign_hash(&test_ctx->expected_server_sign_hash,
                                    value);
}

__owur static int parse_expected_client_sign_hash(SSL_TEST_CTX *test_ctx,
                                                  const char *value)
{
    return parse_expected_sign_hash(&test_ctx->expected_client_sign_hash,
                                    value);
}

__owur static int parse_expected_ca_names(STACK_OF(X509_NAME) **pnames,
                                          const char *value,
                                          OSSL_LIB_CTX *libctx)
{
    if (value == NULL)
        return 0;
    if (!strcmp(value, "empty"))
        *pnames = sk_X509_NAME_new_null();
    else
        *pnames = SSL_load_client_CA_file_ex(value, libctx, NULL);
    return *pnames != NULL;
}
__owur static int parse_expected_server_ca_names(SSL_TEST_CTX *test_ctx,
                                                 const char *value)
{
    return parse_expected_ca_names(&test_ctx->expected_server_ca_names, value,
                                   test_ctx->libctx);
}
__owur static int parse_expected_client_ca_names(SSL_TEST_CTX *test_ctx,
                                                 const char *value)
{
    return parse_expected_ca_names(&test_ctx->expected_client_ca_names, value,
                                   test_ctx->libctx);
}

/* ExpectedCipher */

IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CTX, test, expected_cipher)

/* Client and Server PHA */

IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CLIENT_CONF, client, enable_pha)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_SERVER_CONF, server, force_pha)
IMPLEMENT_SSL_TEST_BOOL_OPTION(SSL_TEST_CLIENT_CONF, client, no_extms_on_reneg)

/* FIPS provider version limiting */
IMPLEMENT_SSL_TEST_STRING_OPTION(SSL_TEST_CTX, test, fips_version)

/* Known test options and their corresponding parse methods. */

/* Top-level options. */
typedef struct {
    const char *name;
    int (*parse)(SSL_TEST_CTX *test_ctx, const char *value);
} ssl_test_ctx_option;

static const ssl_test_ctx_option ssl_test_ctx_options[] = {
    { "ExpectedResult", &parse_expected_result },
    { "ExpectedClientAlert", &parse_client_alert },
    { "ExpectedServerAlert", &parse_server_alert },
    { "ExpectedProtocol", &parse_protocol },
    { "ExpectedServerName", &parse_expected_servername },
    { "SessionTicketExpected", &parse_session_ticket },
    { "CompressionExpected", &parse_test_compression_expected },
    { "SessionIdExpected", &parse_session_id },
    { "Method", &parse_test_method },
    { "ExpectedNPNProtocol", &parse_test_expected_npn_protocol },
    { "ExpectedALPNProtocol", &parse_test_expected_alpn_protocol },
    { "HandshakeMode", &parse_handshake_mode },
    { "KeyUpdateType", &parse_key_update_type },
    { "ResumptionExpected", &parse_test_resumption_expected },
    { "ApplicationData", &parse_test_app_data_size },
    { "MaxFragmentSize", &parse_test_max_fragment_size },
    { "ExpectedTmpKeyType", &parse_expected_tmp_key_type },
    { "ExpectedServerCertType", &parse_expected_server_cert_type },
    { "ExpectedServerSignHash", &parse_expected_server_sign_hash },
    { "ExpectedServerSignType", &parse_expected_server_sign_type },
    { "ExpectedServerCANames", &parse_expected_server_ca_names },
    { "ExpectedClientCertType", &parse_expected_client_cert_type },
    { "ExpectedClientSignHash", &parse_expected_client_sign_hash },
    { "ExpectedClientSignType", &parse_expected_client_sign_type },
    { "ExpectedClientCANames", &parse_expected_client_ca_names },
    { "UseSCTP", &parse_test_use_sctp },
    { "CompressCertificates", &parse_test_compress_certificates },
    { "EnableClientSCTPLabelBug", &parse_test_enable_client_sctp_label_bug },
    { "EnableServerSCTPLabelBug", &parse_test_enable_server_sctp_label_bug },
    { "ExpectedCipher", &parse_test_expected_cipher },
    { "ExpectedSessionTicketAppData", &parse_test_expected_session_ticket_app_data },
    { "FIPSversion", &parse_test_fips_version },
};

/* Nested client options. */
typedef struct {
    const char *name;
    int (*parse)(SSL_TEST_CLIENT_CONF *conf, const char *value);
} ssl_test_client_option;

static const ssl_test_client_option ssl_test_client_options[] = {
    { "VerifyCallback", &parse_client_verify_callback },
    { "ServerName", &parse_servername },
    { "NPNProtocols", &parse_client_npn_protocols },
    { "ALPNProtocols", &parse_client_alpn_protocols },
    { "CTValidation", &parse_ct_validation },
    { "RenegotiateCiphers", &parse_client_reneg_ciphers},
    { "SRPUser", &parse_client_srp_user },
    { "SRPPassword", &parse_client_srp_password },
    { "MaxFragmentLenExt", &parse_max_fragment_len_mode },
    { "EnablePHA", &parse_client_enable_pha },
    { "RenegotiateNoExtms", &parse_client_no_extms_on_reneg },
};

/* Nested server options. */
typedef struct {
    const char *name;
    int (*parse)(SSL_TEST_SERVER_CONF *conf, const char *value);
} ssl_test_server_option;

static const ssl_test_server_option ssl_test_server_options[] = {
    { "ServerNameCallback", &parse_servername_callback },
    { "NPNProtocols", &parse_server_npn_protocols },
    { "ALPNProtocols", &parse_server_alpn_protocols },
    { "BrokenSessionTicket", &parse_server_broken_session_ticket },
    { "CertStatus", &parse_certstatus },
    { "SRPUser", &parse_server_srp_user },
    { "SRPPassword", &parse_server_srp_password },
    { "ForcePHA", &parse_server_force_pha },
    { "SessionTicketAppData", &parse_server_session_ticket_app_data },
};

SSL_TEST_CTX *SSL_TEST_CTX_new(OSSL_LIB_CTX *libctx)
{
    SSL_TEST_CTX *ret;

    /* The return code is checked by caller */
    if ((ret = OPENSSL_zalloc(sizeof(*ret))) != NULL) {
        ret->libctx = libctx;
        ret->app_data_size = default_app_data_size;
        ret->max_fragment_size = default_max_fragment_size;
    }
    return ret;
}

static void ssl_test_extra_conf_free_data(SSL_TEST_EXTRA_CONF *conf)
{
    OPENSSL_free(conf->client.npn_protocols);
    OPENSSL_free(conf->server.npn_protocols);
    OPENSSL_free(conf->server2.npn_protocols);
    OPENSSL_free(conf->client.alpn_protocols);
    OPENSSL_free(conf->server.alpn_protocols);
    OPENSSL_free(conf->server2.alpn_protocols);
    OPENSSL_free(conf->client.reneg_ciphers);
    OPENSSL_free(conf->server.srp_user);
    OPENSSL_free(conf->server.srp_password);
    OPENSSL_free(conf->server2.srp_user);
    OPENSSL_free(conf->server2.srp_password);
    OPENSSL_free(conf->client.srp_user);
    OPENSSL_free(conf->client.srp_password);
    OPENSSL_free(conf->server.session_ticket_app_data);
    OPENSSL_free(conf->server2.session_ticket_app_data);
}

static void ssl_test_ctx_free_extra_data(SSL_TEST_CTX *ctx)
{
    ssl_test_extra_conf_free_data(&ctx->extra);
    ssl_test_extra_conf_free_data(&ctx->resume_extra);
}

void SSL_TEST_CTX_free(SSL_TEST_CTX *ctx)
{
    if (ctx == NULL)
        return;
    ssl_test_ctx_free_extra_data(ctx);
    OPENSSL_free(ctx->expected_npn_protocol);
    OPENSSL_free(ctx->expected_alpn_protocol);
    OPENSSL_free(ctx->expected_session_ticket_app_data);
    sk_X509_NAME_pop_free(ctx->expected_server_ca_names, X509_NAME_free);
    sk_X509_NAME_pop_free(ctx->expected_client_ca_names, X509_NAME_free);
    OPENSSL_free(ctx->expected_cipher);
    OPENSSL_free(ctx->fips_version);
    OPENSSL_free(ctx);
}

static int parse_client_options(SSL_TEST_CLIENT_CONF *client, const CONF *conf,
                                const char *client_section)
{
    STACK_OF(CONF_VALUE) *sk_conf;
    int i;
    size_t j;

    if (!TEST_ptr(sk_conf = NCONF_get_section(conf, client_section)))
        return 0;

    for (i = 0; i < sk_CONF_VALUE_num(sk_conf); i++) {
        int found = 0;
        const CONF_VALUE *option = sk_CONF_VALUE_value(sk_conf, i);
        for (j = 0; j < OSSL_NELEM(ssl_test_client_options); j++) {
            if (strcmp(option->name, ssl_test_client_options[j].name) == 0) {
                if (!ssl_test_client_options[j].parse(client, option->value)) {
                    TEST_info("Bad value %s for option %s",
                              option->value, option->name);
                    return 0;
                }
                found = 1;
                break;
            }
        }
        if (!found) {
            TEST_info("Unknown test option: %s", option->name);
            return 0;
        }
    }

    return 1;
}

static int parse_server_options(SSL_TEST_SERVER_CONF *server, const CONF *conf,
                                const char *server_section)
{
    STACK_OF(CONF_VALUE) *sk_conf;
    int i;
    size_t j;

    if (!TEST_ptr(sk_conf = NCONF_get_section(conf, server_section)))
        return 0;

    for (i = 0; i < sk_CONF_VALUE_num(sk_conf); i++) {
        int found = 0;
        const CONF_VALUE *option = sk_CONF_VALUE_value(sk_conf, i);
        for (j = 0; j < OSSL_NELEM(ssl_test_server_options); j++) {
            if (strcmp(option->name, ssl_test_server_options[j].name) == 0) {
                if (!ssl_test_server_options[j].parse(server, option->value)) {
                    TEST_info("Bad value %s for option %s",
                               option->value, option->name);
                    return 0;
                }
                found = 1;
                break;
            }
        }
        if (!found) {
            TEST_info("Unknown test option: %s", option->name);
            return 0;
        }
    }

    return 1;
}

SSL_TEST_CTX *SSL_TEST_CTX_create(const CONF *conf, const char *test_section,
                                  OSSL_LIB_CTX *libctx)
{
    STACK_OF(CONF_VALUE) *sk_conf = NULL;
    SSL_TEST_CTX *ctx = NULL;
    int i;
    size_t j;

    if (!TEST_ptr(sk_conf = NCONF_get_section(conf, test_section))
            || !TEST_ptr(ctx = SSL_TEST_CTX_new(libctx)))
        goto err;

    for (i = 0; i < sk_CONF_VALUE_num(sk_conf); i++) {
        int found = 0;
        const CONF_VALUE *option = sk_CONF_VALUE_value(sk_conf, i);

        /* Subsections */
        if (strcmp(option->name, "client") == 0) {
            if (!parse_client_options(&ctx->extra.client, conf, option->value))
                goto err;
        } else if (strcmp(option->name, "server") == 0) {
            if (!parse_server_options(&ctx->extra.server, conf, option->value))
                goto err;
        } else if (strcmp(option->name, "server2") == 0) {
            if (!parse_server_options(&ctx->extra.server2, conf, option->value))
                goto err;
        } else if (strcmp(option->name, "resume-client") == 0) {
            if (!parse_client_options(&ctx->resume_extra.client, conf,
                                      option->value))
                goto err;
        } else if (strcmp(option->name, "resume-server") == 0) {
            if (!parse_server_options(&ctx->resume_extra.server, conf,
                                      option->value))
                goto err;
        } else if (strcmp(option->name, "resume-server2") == 0) {
            if (!parse_server_options(&ctx->resume_extra.server2, conf,
                                      option->value))
                goto err;
        } else {
            for (j = 0; j < OSSL_NELEM(ssl_test_ctx_options); j++) {
                if (strcmp(option->name, ssl_test_ctx_options[j].name) == 0) {
                    if (!ssl_test_ctx_options[j].parse(ctx, option->value)) {
                        TEST_info("Bad value %s for option %s",
                                   option->value, option->name);
                        goto err;
                    }
                    found = 1;
                    break;
                }
            }
            if (!found) {
                TEST_info("Unknown test option: %s", option->name);
                goto err;
            }
        }
    }

    goto done;

 err:
    SSL_TEST_CTX_free(ctx);
    ctx = NULL;
 done:
    return ctx;
}
