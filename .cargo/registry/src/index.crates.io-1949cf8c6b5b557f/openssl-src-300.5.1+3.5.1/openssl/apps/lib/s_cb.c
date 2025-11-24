/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * callback functions used by s_client, s_server, and s_time,
 * as well as other common logic for those apps
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h> /* for memcpy() and strcmp() */
#include "apps.h"
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/x509.h>
#include <openssl/ssl.h>
#include <openssl/bn.h>
#ifndef OPENSSL_NO_DH
# include <openssl/dh.h>
#endif
#include "s_apps.h"

#define COOKIE_SECRET_LENGTH    16

VERIFY_CB_ARGS verify_args = { -1, 0, X509_V_OK, 0 };

#ifndef OPENSSL_NO_SOCK
static unsigned char cookie_secret[COOKIE_SECRET_LENGTH];
static int cookie_initialized = 0;
#endif
static BIO *bio_keylog = NULL;

static const char *lookup(int val, const STRINT_PAIR* list, const char* def)
{
    for ( ; list->name; ++list)
        if (list->retval == val)
            return list->name;
    return def;
}

int verify_callback(int ok, X509_STORE_CTX *ctx)
{
    X509 *err_cert;
    int err, depth;

    err_cert = X509_STORE_CTX_get_current_cert(ctx);
    err = X509_STORE_CTX_get_error(ctx);
    depth = X509_STORE_CTX_get_error_depth(ctx);

    if (!verify_args.quiet || !ok) {
        BIO_printf(bio_err, "depth=%d ", depth);
        if (err_cert != NULL) {
            X509_NAME_print_ex(bio_err,
                               X509_get_subject_name(err_cert),
                               0, get_nameopt());
            BIO_puts(bio_err, "\n");
        } else {
            BIO_puts(bio_err, "<no cert>\n");
        }
    }
    if (!ok) {
        BIO_printf(bio_err, "verify error:num=%d:%s\n", err,
                   X509_verify_cert_error_string(err));
        if (verify_args.depth < 0 || verify_args.depth >= depth) {
            if (!verify_args.return_error)
                ok = 1;
            verify_args.error = err;
        } else {
            ok = 0;
            verify_args.error = X509_V_ERR_CERT_CHAIN_TOO_LONG;
        }
    }
    switch (err) {
    case X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT:
        if (err_cert != NULL) {
            BIO_puts(bio_err, "issuer= ");
            X509_NAME_print_ex(bio_err, X509_get_issuer_name(err_cert),
                               0, get_nameopt());
            BIO_puts(bio_err, "\n");
        }
        break;
    case X509_V_ERR_CERT_NOT_YET_VALID:
    case X509_V_ERR_ERROR_IN_CERT_NOT_BEFORE_FIELD:
        if (err_cert != NULL) {
            BIO_printf(bio_err, "notBefore=");
            ASN1_TIME_print(bio_err, X509_get0_notBefore(err_cert));
            BIO_printf(bio_err, "\n");
        }
        break;
    case X509_V_ERR_CERT_HAS_EXPIRED:
    case X509_V_ERR_ERROR_IN_CERT_NOT_AFTER_FIELD:
        if (err_cert != NULL) {
            BIO_printf(bio_err, "notAfter=");
            ASN1_TIME_print(bio_err, X509_get0_notAfter(err_cert));
            BIO_printf(bio_err, "\n");
        }
        break;
    case X509_V_ERR_NO_EXPLICIT_POLICY:
        if (!verify_args.quiet)
            policies_print(ctx);
        break;
    }
    if (err == X509_V_OK && ok == 2 && !verify_args.quiet)
        policies_print(ctx);
    if (ok && !verify_args.quiet)
        BIO_printf(bio_err, "verify return:%d\n", ok);
    return ok;
}

int set_cert_stuff(SSL_CTX *ctx, char *cert_file, char *key_file)
{
    if (cert_file != NULL) {
        if (SSL_CTX_use_certificate_file(ctx, cert_file,
                                         SSL_FILETYPE_PEM) <= 0) {
            BIO_printf(bio_err, "unable to get certificate from '%s'\n",
                       cert_file);
            ERR_print_errors(bio_err);
            return 0;
        }
        if (key_file == NULL)
            key_file = cert_file;
        if (SSL_CTX_use_PrivateKey_file(ctx, key_file, SSL_FILETYPE_PEM) <= 0) {
            BIO_printf(bio_err, "unable to get private key from '%s'\n",
                       key_file);
            ERR_print_errors(bio_err);
            return 0;
        }

        /*
         * If we are using DSA, we can copy the parameters from the private
         * key
         */

        /*
         * Now we know that a key and cert have been set against the SSL
         * context
         */
        if (!SSL_CTX_check_private_key(ctx)) {
            BIO_printf(bio_err,
                       "Private key does not match the certificate public key\n");
            return 0;
        }
    }
    return 1;
}

int set_cert_key_stuff(SSL_CTX *ctx, X509 *cert, EVP_PKEY *key,
                       STACK_OF(X509) *chain, int build_chain)
{
    int chflags = chain ? SSL_BUILD_CHAIN_FLAG_CHECK : 0;

    if (cert == NULL)
        return 1;
    if (SSL_CTX_use_certificate(ctx, cert) <= 0) {
        BIO_printf(bio_err, "error setting certificate\n");
        ERR_print_errors(bio_err);
        return 0;
    }

    if (SSL_CTX_use_PrivateKey(ctx, key) <= 0) {
        BIO_printf(bio_err, "error setting private key\n");
        ERR_print_errors(bio_err);
        return 0;
    }

    /*
     * Now we know that a key and cert have been set against the SSL context
     */
    if (!SSL_CTX_check_private_key(ctx)) {
        BIO_printf(bio_err,
                   "Private key does not match the certificate public key\n");
        return 0;
    }
    if (chain && !SSL_CTX_set1_chain(ctx, chain)) {
        BIO_printf(bio_err, "error setting certificate chain\n");
        ERR_print_errors(bio_err);
        return 0;
    }
    if (build_chain && !SSL_CTX_build_cert_chain(ctx, chflags)) {
        BIO_printf(bio_err, "error building certificate chain\n");
        ERR_print_errors(bio_err);
        return 0;
    }
    return 1;
}

static STRINT_PAIR cert_type_list[] = {
    {"RSA sign", TLS_CT_RSA_SIGN},
    {"DSA sign", TLS_CT_DSS_SIGN},
    {"RSA fixed DH", TLS_CT_RSA_FIXED_DH},
    {"DSS fixed DH", TLS_CT_DSS_FIXED_DH},
    {"ECDSA sign", TLS_CT_ECDSA_SIGN},
    {"RSA fixed ECDH", TLS_CT_RSA_FIXED_ECDH},
    {"ECDSA fixed ECDH", TLS_CT_ECDSA_FIXED_ECDH},
    {"GOST01 Sign", TLS_CT_GOST01_SIGN},
    {"GOST12 Sign", TLS_CT_GOST12_IANA_SIGN},
    {NULL}
};

static void ssl_print_client_cert_types(BIO *bio, SSL *s)
{
    const unsigned char *p;
    int i;
    int cert_type_num = SSL_get0_certificate_types(s, &p);

    if (!cert_type_num)
        return;
    BIO_puts(bio, "Client Certificate Types: ");
    for (i = 0; i < cert_type_num; i++) {
        unsigned char cert_type = p[i];
        const char *cname = lookup((int)cert_type, cert_type_list, NULL);

        if (i)
            BIO_puts(bio, ", ");
        if (cname != NULL)
            BIO_puts(bio, cname);
        else
            BIO_printf(bio, "UNKNOWN (%d),", cert_type);
    }
    BIO_puts(bio, "\n");
}

static const char *get_sigtype(int nid)
{
    switch (nid) {
    case EVP_PKEY_RSA:
        return "RSA";

    case EVP_PKEY_RSA_PSS:
        return "RSA-PSS";

    case EVP_PKEY_DSA:
        return "DSA";

    case EVP_PKEY_EC:
        return "ECDSA";

    case NID_ED25519:
        return "ed25519";

    case NID_ED448:
        return "ed448";

    case NID_id_GostR3410_2001:
        return "gost2001";

    case NID_id_GostR3410_2012_256:
        return "gost2012_256";

    case NID_id_GostR3410_2012_512:
        return "gost2012_512";

    default:
        /* Try to output provider-registered sig alg name */
        return OBJ_nid2sn(nid);
    }
}

static int do_print_sigalgs(BIO *out, SSL *s, int shared)
{
    int i, nsig, client;

    client = SSL_is_server(s) ? 0 : 1;
    if (shared)
        nsig = SSL_get_shared_sigalgs(s, 0, NULL, NULL, NULL, NULL, NULL);
    else
        nsig = SSL_get_sigalgs(s, -1, NULL, NULL, NULL, NULL, NULL);
    if (nsig == 0)
        return 1;

    if (shared)
        BIO_puts(out, "Shared ");

    if (client)
        BIO_puts(out, "Requested ");
    BIO_puts(out, "Signature Algorithms: ");
    for (i = 0; i < nsig; i++) {
        int hash_nid, sign_nid;
        unsigned char rhash, rsign;
        const char *sstr = NULL;
        if (shared)
            SSL_get_shared_sigalgs(s, i, &sign_nid, &hash_nid, NULL,
                                   &rsign, &rhash);
        else
            SSL_get_sigalgs(s, i, &sign_nid, &hash_nid, NULL, &rsign, &rhash);
        if (i)
            BIO_puts(out, ":");
        switch (rsign | rhash << 8) {
        case 0x0809:
            BIO_puts(out, "rsa_pss_pss_sha256");
            continue;
        case 0x080a:
            BIO_puts(out, "rsa_pss_pss_sha384");
            continue;
        case 0x080b:
            BIO_puts(out, "rsa_pss_pss_sha512");
            continue;
        case 0x081a:
            BIO_puts(out, "ecdsa_brainpoolP256r1_sha256");
            continue;
        case 0x081b:
            BIO_puts(out, "ecdsa_brainpoolP384r1_sha384");
            continue;
        case 0x081c:
            BIO_puts(out, "ecdsa_brainpoolP512r1_sha512");
            continue;
        }
        sstr = get_sigtype(sign_nid);
        if (sstr)
            BIO_printf(out, "%s", sstr);
        else
            BIO_printf(out, "0x%02X", (int)rsign);
        if (hash_nid != NID_undef)
            BIO_printf(out, "+%s", OBJ_nid2sn(hash_nid));
        else if (sstr == NULL)
            BIO_printf(out, "+0x%02X", (int)rhash);
    }
    BIO_puts(out, "\n");
    return 1;
}

int ssl_print_sigalgs(BIO *out, SSL *s)
{
    const char *name;
    int nid;

    if (!SSL_is_server(s))
        ssl_print_client_cert_types(out, s);
    do_print_sigalgs(out, s, 0);
    do_print_sigalgs(out, s, 1);
    if (SSL_get_peer_signature_nid(s, &nid) && nid != NID_undef)
        BIO_printf(out, "Peer signing digest: %s\n", OBJ_nid2sn(nid));
    if (SSL_get0_peer_signature_name(s, &name))
        BIO_printf(out, "Peer signature type: %s\n", name);
    else if (SSL_get_peer_signature_type_nid(s, &nid))
        BIO_printf(out, "Peer signature type: %s\n", get_sigtype(nid));
    return 1;
}

#ifndef OPENSSL_NO_EC
int ssl_print_point_formats(BIO *out, SSL *s)
{
    int i, nformats;
    const char *pformats;

    nformats = SSL_get0_ec_point_formats(s, &pformats);
    if (nformats <= 0)
        return 1;
    BIO_puts(out, "Supported Elliptic Curve Point Formats: ");
    for (i = 0; i < nformats; i++, pformats++) {
        if (i)
            BIO_puts(out, ":");
        switch (*pformats) {
        case TLSEXT_ECPOINTFORMAT_uncompressed:
            BIO_puts(out, "uncompressed");
            break;

        case TLSEXT_ECPOINTFORMAT_ansiX962_compressed_prime:
            BIO_puts(out, "ansiX962_compressed_prime");
            break;

        case TLSEXT_ECPOINTFORMAT_ansiX962_compressed_char2:
            BIO_puts(out, "ansiX962_compressed_char2");
            break;

        default:
            BIO_printf(out, "unknown(%d)", (int)*pformats);
            break;

        }
    }
    BIO_puts(out, "\n");
    return 1;
}

int ssl_print_groups(BIO *out, SSL *s, int noshared)
{
    int i, ngroups, *groups, nid;

    ngroups = SSL_get1_groups(s, NULL);
    if (ngroups <= 0)
        return 1;
    groups = app_malloc(ngroups * sizeof(int), "groups to print");
    SSL_get1_groups(s, groups);

    BIO_puts(out, "Supported groups: ");
    for (i = 0; i < ngroups; i++) {
        if (i)
            BIO_puts(out, ":");
        nid = groups[i];
        BIO_printf(out, "%s", SSL_group_to_name(s, nid));
    }
    OPENSSL_free(groups);
    if (noshared) {
        BIO_puts(out, "\n");
        return 1;
    }
    BIO_puts(out, "\nShared groups: ");
    ngroups = SSL_get_shared_group(s, -1);
    for (i = 0; i < ngroups; i++) {
        if (i)
            BIO_puts(out, ":");
        nid = SSL_get_shared_group(s, i);
        BIO_printf(out, "%s", SSL_group_to_name(s, nid));
    }
    if (ngroups == 0)
        BIO_puts(out, "NONE");
    BIO_puts(out, "\n");
    return 1;
}
#endif

int ssl_print_tmp_key(BIO *out, SSL *s)
{
    const char *keyname;
    EVP_PKEY *key;

    if (!SSL_get_peer_tmp_key(s, &key)) {
        if (SSL_version(s) == TLS1_3_VERSION)
            BIO_printf(out, "Negotiated TLS1.3 group: %s\n",
                       SSL_group_to_name(s, SSL_get_negotiated_group(s)));
        return 1;
    }

    BIO_puts(out, "Peer Temp Key: ");
    switch (EVP_PKEY_get_id(key)) {
    case EVP_PKEY_RSA:
        BIO_printf(out, "RSA, %d bits\n", EVP_PKEY_get_bits(key));
        break;

    case EVP_PKEY_KEYMGMT:
        if ((keyname = EVP_PKEY_get0_type_name(key)) == NULL)
            keyname = "?";
        BIO_printf(out, "%s\n", keyname);
        break;

    case EVP_PKEY_DH:
        BIO_printf(out, "DH, %d bits\n", EVP_PKEY_get_bits(key));
        break;
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        {
            char name[80];
            size_t name_len;

            if (!EVP_PKEY_get_utf8_string_param(key, OSSL_PKEY_PARAM_GROUP_NAME,
                                                name, sizeof(name), &name_len))
                strcpy(name, "?");
            BIO_printf(out, "ECDH, %s, %d bits\n", name, EVP_PKEY_get_bits(key));
        }
    break;
#endif
    default:
        BIO_printf(out, "%s, %d bits\n", OBJ_nid2sn(EVP_PKEY_get_id(key)),
                   EVP_PKEY_get_bits(key));
    }
    EVP_PKEY_free(key);
    return 1;
}

long bio_dump_callback(BIO *bio, int cmd, const char *argp, size_t len,
                       int argi, long argl, int ret, size_t *processed)
{
    BIO *out;
    BIO_MMSG_CB_ARGS *mmsgargs;
    size_t i;

    out = (BIO *)BIO_get_callback_arg(bio);
    if (out == NULL)
        return ret;

    switch (cmd) {
    case (BIO_CB_READ | BIO_CB_RETURN):
        if (ret > 0 && processed != NULL) {
            BIO_printf(out, "read from %p [%p] (%zu bytes => %zu (0x%zX))\n",
                       (void *)bio, (void *)argp, len, *processed, *processed);
            BIO_dump(out, argp, (int)*processed);
        } else {
            BIO_printf(out, "read from %p [%p] (%zu bytes => %d)\n",
                       (void *)bio, (void *)argp, len, ret);
        }
        break;

    case (BIO_CB_WRITE | BIO_CB_RETURN):
        if (ret > 0 && processed != NULL) {
            BIO_printf(out, "write to %p [%p] (%zu bytes => %zu (0x%zX))\n",
                       (void *)bio, (void *)argp, len, *processed, *processed);
            BIO_dump(out, argp, (int)*processed);
        } else {
            BIO_printf(out, "write to %p [%p] (%zu bytes => %d)\n",
                       (void *)bio, (void *)argp, len, ret);
        }
        break;

    case (BIO_CB_RECVMMSG | BIO_CB_RETURN):
        mmsgargs = (BIO_MMSG_CB_ARGS *)argp;
        if (ret > 0) {
            for (i = 0; i < *(mmsgargs->msgs_processed); i++) {
                BIO_MSG *msg = (BIO_MSG *)((char *)mmsgargs->msg
                                           + (i * mmsgargs->stride));

                BIO_printf(out, "read from %p [%p] (%zu bytes => %zu (0x%zX))\n",
                           (void *)bio, (void *)msg->data, msg->data_len,
                           msg->data_len, msg->data_len);
                BIO_dump(out, msg->data, msg->data_len);
            }
        } else if (mmsgargs->num_msg > 0) {
            BIO_MSG *msg = mmsgargs->msg;

            BIO_printf(out, "read from %p [%p] (%zu bytes => %d)\n",
                       (void *)bio, (void *)msg->data, msg->data_len, ret);
        }
        break;

    case (BIO_CB_SENDMMSG | BIO_CB_RETURN):
        mmsgargs = (BIO_MMSG_CB_ARGS *)argp;
        if (ret > 0) {
            for (i = 0; i < *(mmsgargs->msgs_processed); i++) {
                BIO_MSG *msg = (BIO_MSG *)((char *)mmsgargs->msg
                                           + (i * mmsgargs->stride));

                BIO_printf(out, "write to %p [%p] (%zu bytes => %zu (0x%zX))\n",
                           (void *)bio, (void *)msg->data, msg->data_len,
                           msg->data_len, msg->data_len);
                BIO_dump(out, msg->data, msg->data_len);
            }
        } else if (mmsgargs->num_msg > 0) {
            BIO_MSG *msg = mmsgargs->msg;

            BIO_printf(out, "write to %p [%p] (%zu bytes => %d)\n",
                       (void *)bio, (void *)msg->data, msg->data_len, ret);
        }
        break;

    default:
        /* do nothing */
        break;
    }
    return ret;
}

void apps_ssl_info_callback(const SSL *s, int where, int ret)
{
    const char *str;
    int w;

    w = where & ~SSL_ST_MASK;

    if (w & SSL_ST_CONNECT)
        str = "SSL_connect";
    else if (w & SSL_ST_ACCEPT)
        str = "SSL_accept";
    else
        str = "undefined";

    if (where & SSL_CB_LOOP) {
        BIO_printf(bio_err, "%s:%s\n", str, SSL_state_string_long(s));
    } else if (where & SSL_CB_ALERT) {
        str = (where & SSL_CB_READ) ? "read" : "write";
        BIO_printf(bio_err, "SSL3 alert %s:%s:%s\n",
                   str,
                   SSL_alert_type_string_long(ret),
                   SSL_alert_desc_string_long(ret));
    } else if (where & SSL_CB_EXIT) {
        if (ret == 0)
            BIO_printf(bio_err, "%s:failed in %s\n",
                       str, SSL_state_string_long(s));
        else if (ret < 0)
            BIO_printf(bio_err, "%s:error in %s\n",
                       str, SSL_state_string_long(s));
    }
}

static STRINT_PAIR ssl_versions[] = {
    {"SSL 3.0", SSL3_VERSION},
    {"TLS 1.0", TLS1_VERSION},
    {"TLS 1.1", TLS1_1_VERSION},
    {"TLS 1.2", TLS1_2_VERSION},
    {"TLS 1.3", TLS1_3_VERSION},
    {"DTLS 1.0", DTLS1_VERSION},
    {"DTLS 1.0 (bad)", DTLS1_BAD_VER},
    {NULL}
};

static STRINT_PAIR alert_types[] = {
    {" close_notify", 0},
    {" end_of_early_data", 1},
    {" unexpected_message", 10},
    {" bad_record_mac", 20},
    {" decryption_failed", 21},
    {" record_overflow", 22},
    {" decompression_failure", 30},
    {" handshake_failure", 40},
    {" bad_certificate", 42},
    {" unsupported_certificate", 43},
    {" certificate_revoked", 44},
    {" certificate_expired", 45},
    {" certificate_unknown", 46},
    {" illegal_parameter", 47},
    {" unknown_ca", 48},
    {" access_denied", 49},
    {" decode_error", 50},
    {" decrypt_error", 51},
    {" export_restriction", 60},
    {" protocol_version", 70},
    {" insufficient_security", 71},
    {" internal_error", 80},
    {" inappropriate_fallback", 86},
    {" user_canceled", 90},
    {" no_renegotiation", 100},
    {" missing_extension", 109},
    {" unsupported_extension", 110},
    {" certificate_unobtainable", 111},
    {" unrecognized_name", 112},
    {" bad_certificate_status_response", 113},
    {" bad_certificate_hash_value", 114},
    {" unknown_psk_identity", 115},
    {" certificate_required", 116},
    {NULL}
};

static STRINT_PAIR handshakes[] = {
    {", HelloRequest", SSL3_MT_HELLO_REQUEST},
    {", ClientHello", SSL3_MT_CLIENT_HELLO},
    {", ServerHello", SSL3_MT_SERVER_HELLO},
    {", HelloVerifyRequest", DTLS1_MT_HELLO_VERIFY_REQUEST},
    {", NewSessionTicket", SSL3_MT_NEWSESSION_TICKET},
    {", EndOfEarlyData", SSL3_MT_END_OF_EARLY_DATA},
    {", EncryptedExtensions", SSL3_MT_ENCRYPTED_EXTENSIONS},
    {", Certificate", SSL3_MT_CERTIFICATE},
    {", ServerKeyExchange", SSL3_MT_SERVER_KEY_EXCHANGE},
    {", CertificateRequest", SSL3_MT_CERTIFICATE_REQUEST},
    {", ServerHelloDone", SSL3_MT_SERVER_DONE},
    {", CertificateVerify", SSL3_MT_CERTIFICATE_VERIFY},
    {", ClientKeyExchange", SSL3_MT_CLIENT_KEY_EXCHANGE},
    {", Finished", SSL3_MT_FINISHED},
    {", CertificateUrl", SSL3_MT_CERTIFICATE_URL},
    {", CertificateStatus", SSL3_MT_CERTIFICATE_STATUS},
    {", SupplementalData", SSL3_MT_SUPPLEMENTAL_DATA},
    {", KeyUpdate", SSL3_MT_KEY_UPDATE},
    {", CompressedCertificate", SSL3_MT_COMPRESSED_CERTIFICATE},
#ifndef OPENSSL_NO_NEXTPROTONEG
    {", NextProto", SSL3_MT_NEXT_PROTO},
#endif
    {", MessageHash", SSL3_MT_MESSAGE_HASH},
    {NULL}
};

void msg_cb(int write_p, int version, int content_type, const void *buf,
            size_t len, SSL *ssl, void *arg)
{
    BIO *bio = arg;
    const char *str_write_p = write_p ? ">>>" : "<<<";
    char tmpbuf[128];
    const char *str_version, *str_content_type = "", *str_details1 = "", *str_details2 = "";
    const unsigned char* bp = buf;

    if (version == SSL3_VERSION ||
        version == TLS1_VERSION ||
        version == TLS1_1_VERSION ||
        version == TLS1_2_VERSION ||
        version == TLS1_3_VERSION ||
        version == DTLS1_VERSION || version == DTLS1_BAD_VER) {
        str_version = lookup(version, ssl_versions, "???");
        switch (content_type) {
        case SSL3_RT_CHANGE_CIPHER_SPEC:
            /* type 20 */
            str_content_type = ", ChangeCipherSpec";
            break;
        case SSL3_RT_ALERT:
            /* type 21 */
            str_content_type = ", Alert";
            str_details1 = ", ???";
            if (len == 2) {
                switch (bp[0]) {
                case 1:
                    str_details1 = ", warning";
                    break;
                case 2:
                    str_details1 = ", fatal";
                    break;
                }
                str_details2 = lookup((int)bp[1], alert_types, " ???");
            }
            break;
        case SSL3_RT_HANDSHAKE:
            /* type 22 */
            str_content_type = ", Handshake";
            str_details1 = "???";
            if (len > 0)
                str_details1 = lookup((int)bp[0], handshakes, "???");
            break;
        case SSL3_RT_APPLICATION_DATA:
            /* type 23 */
            str_content_type = ", ApplicationData";
            break;
        case SSL3_RT_HEADER:
            /* type 256 */
            str_content_type = ", RecordHeader";
            break;
        case SSL3_RT_INNER_CONTENT_TYPE:
            /* type 257 */
            str_content_type = ", InnerContent";
            break;
        default:
            BIO_snprintf(tmpbuf, sizeof(tmpbuf)-1, ", Unknown (content_type=%d)", content_type);
            str_content_type = tmpbuf;
        }
    } else {
        BIO_snprintf(tmpbuf, sizeof(tmpbuf)-1, "Not TLS data or unknown version (version=%d, content_type=%d)", version, content_type);
        str_version = tmpbuf;
    }

    BIO_printf(bio, "%s %s%s [length %04lx]%s%s\n", str_write_p, str_version,
               str_content_type, (unsigned long)len, str_details1,
               str_details2);

    if (len > 0) {
        size_t num, i;

        BIO_printf(bio, "   ");
        num = len;
        for (i = 0; i < num; i++) {
            if (i % 16 == 0 && i > 0)
                BIO_printf(bio, "\n   ");
            BIO_printf(bio, " %02x", ((const unsigned char *)buf)[i]);
        }
        if (i < len)
            BIO_printf(bio, " ...");
        BIO_printf(bio, "\n");
    }
    (void)BIO_flush(bio);
}

static const STRINT_PAIR tlsext_types[] = {
    {"server name", TLSEXT_TYPE_server_name},
    {"max fragment length", TLSEXT_TYPE_max_fragment_length},
    {"client certificate URL", TLSEXT_TYPE_client_certificate_url},
    {"trusted CA keys", TLSEXT_TYPE_trusted_ca_keys},
    {"truncated HMAC", TLSEXT_TYPE_truncated_hmac},
    {"status request", TLSEXT_TYPE_status_request},
    {"user mapping", TLSEXT_TYPE_user_mapping},
    {"client authz", TLSEXT_TYPE_client_authz},
    {"server authz", TLSEXT_TYPE_server_authz},
    {"cert type", TLSEXT_TYPE_cert_type},
    {"supported_groups", TLSEXT_TYPE_supported_groups},
    {"EC point formats", TLSEXT_TYPE_ec_point_formats},
    {"SRP", TLSEXT_TYPE_srp},
    {"signature algorithms", TLSEXT_TYPE_signature_algorithms},
    {"use SRTP", TLSEXT_TYPE_use_srtp},
    {"session ticket", TLSEXT_TYPE_session_ticket},
    {"renegotiation info", TLSEXT_TYPE_renegotiate},
    {"signed certificate timestamps", TLSEXT_TYPE_signed_certificate_timestamp},
    {"client cert type", TLSEXT_TYPE_client_cert_type},
    {"server cert type", TLSEXT_TYPE_server_cert_type},
    {"TLS padding", TLSEXT_TYPE_padding},
#ifdef TLSEXT_TYPE_next_proto_neg
    {"next protocol", TLSEXT_TYPE_next_proto_neg},
#endif
#ifdef TLSEXT_TYPE_encrypt_then_mac
    {"encrypt-then-mac", TLSEXT_TYPE_encrypt_then_mac},
#endif
#ifdef TLSEXT_TYPE_application_layer_protocol_negotiation
    {"application layer protocol negotiation",
     TLSEXT_TYPE_application_layer_protocol_negotiation},
#endif
#ifdef TLSEXT_TYPE_extended_master_secret
    {"extended master secret", TLSEXT_TYPE_extended_master_secret},
#endif
    {"compress certificate", TLSEXT_TYPE_compress_certificate},
    {"key share", TLSEXT_TYPE_key_share},
    {"supported versions", TLSEXT_TYPE_supported_versions},
    {"psk", TLSEXT_TYPE_psk},
    {"psk kex modes", TLSEXT_TYPE_psk_kex_modes},
    {"certificate authorities", TLSEXT_TYPE_certificate_authorities},
    {"post handshake auth", TLSEXT_TYPE_post_handshake_auth},
    {"early_data", TLSEXT_TYPE_early_data},
    {NULL}
};

/* from rfc8446 4.2.3. + gost (https://tools.ietf.org/id/draft-smyshlyaev-tls12-gost-suites-04.html) */
static STRINT_PAIR signature_tls13_scheme_list[] = {
    {"rsa_pkcs1_sha1",         0x0201 /* TLSEXT_SIGALG_rsa_pkcs1_sha1 */},
    {"ecdsa_sha1",             0x0203 /* TLSEXT_SIGALG_ecdsa_sha1 */},
/*  {"rsa_pkcs1_sha224",       0x0301    TLSEXT_SIGALG_rsa_pkcs1_sha224}, not in rfc8446 */
/*  {"ecdsa_sha224",           0x0303    TLSEXT_SIGALG_ecdsa_sha224}      not in rfc8446 */
    {"rsa_pkcs1_sha256",       0x0401 /* TLSEXT_SIGALG_rsa_pkcs1_sha256 */},
    {"ecdsa_secp256r1_sha256", 0x0403 /* TLSEXT_SIGALG_ecdsa_secp256r1_sha256 */},
    {"rsa_pkcs1_sha384",       0x0501 /* TLSEXT_SIGALG_rsa_pkcs1_sha384 */},
    {"ecdsa_secp384r1_sha384", 0x0503 /* TLSEXT_SIGALG_ecdsa_secp384r1_sha384 */},
    {"rsa_pkcs1_sha512",       0x0601 /* TLSEXT_SIGALG_rsa_pkcs1_sha512 */},
    {"ecdsa_secp521r1_sha512", 0x0603 /* TLSEXT_SIGALG_ecdsa_secp521r1_sha512 */},
    {"rsa_pss_rsae_sha256",    0x0804 /* TLSEXT_SIGALG_rsa_pss_rsae_sha256 */},
    {"rsa_pss_rsae_sha384",    0x0805 /* TLSEXT_SIGALG_rsa_pss_rsae_sha384 */},
    {"rsa_pss_rsae_sha512",    0x0806 /* TLSEXT_SIGALG_rsa_pss_rsae_sha512 */},
    {"ed25519",                0x0807 /* TLSEXT_SIGALG_ed25519 */},
    {"ed448",                  0x0808 /* TLSEXT_SIGALG_ed448 */},
    {"rsa_pss_pss_sha256",     0x0809 /* TLSEXT_SIGALG_rsa_pss_pss_sha256 */},
    {"rsa_pss_pss_sha384",     0x080a /* TLSEXT_SIGALG_rsa_pss_pss_sha384 */},
    {"rsa_pss_pss_sha512",     0x080b /* TLSEXT_SIGALG_rsa_pss_pss_sha512 */},
    {"gostr34102001",          0xeded /* TLSEXT_SIGALG_gostr34102001_gostr3411 */},
    {"gostr34102012_256",      0xeeee /* TLSEXT_SIGALG_gostr34102012_256_gostr34112012_256 */},
    {"gostr34102012_512",      0xefef /* TLSEXT_SIGALG_gostr34102012_512_gostr34112012_512 */},
    {NULL}
};

/* from rfc5246 7.4.1.4.1. */
static STRINT_PAIR signature_tls12_alg_list[] = {
    {"anonymous", TLSEXT_signature_anonymous /* 0 */},
    {"RSA",       TLSEXT_signature_rsa       /* 1 */},
    {"DSA",       TLSEXT_signature_dsa       /* 2 */},
    {"ECDSA",     TLSEXT_signature_ecdsa     /* 3 */},
    {NULL}
};

/* from rfc5246 7.4.1.4.1. */
static STRINT_PAIR signature_tls12_hash_list[] = {
    {"none",   TLSEXT_hash_none   /* 0 */},
    {"MD5",    TLSEXT_hash_md5    /* 1 */},
    {"SHA1",   TLSEXT_hash_sha1   /* 2 */},
    {"SHA224", TLSEXT_hash_sha224 /* 3 */},
    {"SHA256", TLSEXT_hash_sha256 /* 4 */},
    {"SHA384", TLSEXT_hash_sha384 /* 5 */},
    {"SHA512", TLSEXT_hash_sha512 /* 6 */},
    {NULL}
};

void tlsext_cb(SSL *s, int client_server, int type,
               const unsigned char *data, int len, void *arg)
{
    BIO *bio = arg;
    const char *extname = lookup(type, tlsext_types, "unknown");

    BIO_printf(bio, "TLS %s extension \"%s\" (id=%d), len=%d\n",
               client_server ? "server" : "client", extname, type, len);
    BIO_dump(bio, (const char *)data, len);
    (void)BIO_flush(bio);
}

#ifndef OPENSSL_NO_SOCK
int generate_stateless_cookie_callback(SSL *ssl, unsigned char *cookie,
                                       size_t *cookie_len)
{
    unsigned char *buffer = NULL;
    size_t length = 0;
    unsigned short port;
    BIO_ADDR *lpeer = NULL, *peer = NULL;
    int res = 0;

    /* Initialize a random secret */
    if (!cookie_initialized) {
        if (RAND_bytes(cookie_secret, COOKIE_SECRET_LENGTH) <= 0) {
            BIO_printf(bio_err, "error setting random cookie secret\n");
            return 0;
        }
        cookie_initialized = 1;
    }

    if (SSL_is_dtls(ssl)) {
        lpeer = peer = BIO_ADDR_new();
        if (peer == NULL) {
            BIO_printf(bio_err, "memory full\n");
            return 0;
        }

        /* Read peer information */
        (void)BIO_dgram_get_peer(SSL_get_rbio(ssl), peer);
    } else {
        peer = ourpeer;
    }

    /* Create buffer with peer's address and port */
    if (!BIO_ADDR_rawaddress(peer, NULL, &length)) {
        BIO_printf(bio_err, "Failed getting peer address\n");
        BIO_ADDR_free(lpeer);
        return 0;
    }
    OPENSSL_assert(length != 0);
    port = BIO_ADDR_rawport(peer);
    length += sizeof(port);
    buffer = app_malloc(length, "cookie generate buffer");

    memcpy(buffer, &port, sizeof(port));
    BIO_ADDR_rawaddress(peer, buffer + sizeof(port), NULL);

    if (EVP_Q_mac(NULL, "HMAC", NULL, "SHA1", NULL,
                  cookie_secret, COOKIE_SECRET_LENGTH, buffer, length,
                  cookie, DTLS1_COOKIE_LENGTH, cookie_len) == NULL) {
        BIO_printf(bio_err,
                   "Error calculating HMAC-SHA1 of buffer with secret\n");
        goto end;
    }
    res = 1;
end:
    OPENSSL_free(buffer);
    BIO_ADDR_free(lpeer);

    return res;
}

int verify_stateless_cookie_callback(SSL *ssl, const unsigned char *cookie,
                                     size_t cookie_len)
{
    unsigned char result[EVP_MAX_MD_SIZE];
    size_t resultlength;

    /* Note: we check cookie_initialized because if it's not,
     * it cannot be valid */
    if (cookie_initialized
        && generate_stateless_cookie_callback(ssl, result, &resultlength)
        && cookie_len == resultlength
        && memcmp(result, cookie, resultlength) == 0)
        return 1;

    return 0;
}

int generate_cookie_callback(SSL *ssl, unsigned char *cookie,
                             unsigned int *cookie_len)
{
    size_t temp = 0;
    int res = generate_stateless_cookie_callback(ssl, cookie, &temp);

    if (res != 0)
        *cookie_len = (unsigned int)temp;
    return res;
}

int verify_cookie_callback(SSL *ssl, const unsigned char *cookie,
                           unsigned int cookie_len)
{
    return verify_stateless_cookie_callback(ssl, cookie, cookie_len);
}

#endif

/*
 * Example of extended certificate handling. Where the standard support of
 * one certificate per algorithm is not sufficient an application can decide
 * which certificate(s) to use at runtime based on whatever criteria it deems
 * appropriate.
 */

/* Linked list of certificates, keys and chains */
struct ssl_excert_st {
    int certform;
    const char *certfile;
    int keyform;
    const char *keyfile;
    const char *chainfile;
    X509 *cert;
    EVP_PKEY *key;
    STACK_OF(X509) *chain;
    int build_chain;
    struct ssl_excert_st *next, *prev;
};

static STRINT_PAIR chain_flags[] = {
    {"Overall Validity", CERT_PKEY_VALID},
    {"Sign with EE key", CERT_PKEY_SIGN},
    {"EE signature", CERT_PKEY_EE_SIGNATURE},
    {"CA signature", CERT_PKEY_CA_SIGNATURE},
    {"EE key parameters", CERT_PKEY_EE_PARAM},
    {"CA key parameters", CERT_PKEY_CA_PARAM},
    {"Explicitly sign with EE key", CERT_PKEY_EXPLICIT_SIGN},
    {"Issuer Name", CERT_PKEY_ISSUER_NAME},
    {"Certificate Type", CERT_PKEY_CERT_TYPE},
    {NULL}
};

static void print_chain_flags(SSL *s, int flags)
{
    STRINT_PAIR *pp;

    for (pp = chain_flags; pp->name; ++pp)
        BIO_printf(bio_err, "\t%s: %s\n",
                   pp->name,
                   (flags & pp->retval) ? "OK" : "NOT OK");
    BIO_printf(bio_err, "\tSuite B: ");
    if (SSL_set_cert_flags(s, 0) & SSL_CERT_FLAG_SUITEB_128_LOS)
        BIO_puts(bio_err, flags & CERT_PKEY_SUITEB ? "OK\n" : "NOT OK\n");
    else
        BIO_printf(bio_err, "not tested\n");
}

/*
 * Very basic selection callback: just use any certificate chain reported as
 * valid. More sophisticated could prioritise according to local policy.
 */
static int set_cert_cb(SSL *ssl, void *arg)
{
    int i, rv;
    SSL_EXCERT *exc = arg;
#ifdef CERT_CB_TEST_RETRY
    static int retry_cnt;

    if (retry_cnt < 5) {
        retry_cnt++;
        BIO_printf(bio_err,
                   "Certificate callback retry test: count %d\n",
                   retry_cnt);
        return -1;
    }
#endif
    SSL_certs_clear(ssl);

    if (exc == NULL)
        return 1;

    /*
     * Go to end of list and traverse backwards since we prepend newer
     * entries this retains the original order.
     */
    while (exc->next != NULL)
        exc = exc->next;

    i = 0;

    while (exc != NULL) {
        i++;
        rv = SSL_check_chain(ssl, exc->cert, exc->key, exc->chain);
        BIO_printf(bio_err, "Checking cert chain %d:\nSubject: ", i);
        X509_NAME_print_ex(bio_err, X509_get_subject_name(exc->cert), 0,
                           get_nameopt());
        BIO_puts(bio_err, "\n");
        print_chain_flags(ssl, rv);
        if (rv & CERT_PKEY_VALID) {
            if (!SSL_use_certificate(ssl, exc->cert)
                    || !SSL_use_PrivateKey(ssl, exc->key)) {
                return 0;
            }
            /*
             * NB: we wouldn't normally do this as it is not efficient
             * building chains on each connection better to cache the chain
             * in advance.
             */
            if (exc->build_chain) {
                if (!SSL_build_cert_chain(ssl, 0))
                    return 0;
            } else if (exc->chain != NULL) {
                if (!SSL_set1_chain(ssl, exc->chain))
                    return 0;
            }
        }
        exc = exc->prev;
    }
    return 1;
}

void ssl_ctx_set_excert(SSL_CTX *ctx, SSL_EXCERT *exc)
{
    SSL_CTX_set_cert_cb(ctx, set_cert_cb, exc);
}

static int ssl_excert_prepend(SSL_EXCERT **pexc)
{
    SSL_EXCERT *exc = app_malloc(sizeof(*exc), "prepend cert");

    memset(exc, 0, sizeof(*exc));

    exc->next = *pexc;
    *pexc = exc;

    if (exc->next) {
        exc->certform = exc->next->certform;
        exc->keyform = exc->next->keyform;
        exc->next->prev = exc;
    } else {
        exc->certform = FORMAT_PEM;
        exc->keyform = FORMAT_PEM;
    }
    return 1;

}

void ssl_excert_free(SSL_EXCERT *exc)
{
    SSL_EXCERT *curr;

    if (exc == NULL)
        return;
    while (exc) {
        X509_free(exc->cert);
        EVP_PKEY_free(exc->key);
        OSSL_STACK_OF_X509_free(exc->chain);
        curr = exc;
        exc = exc->next;
        OPENSSL_free(curr);
    }
}

int load_excert(SSL_EXCERT **pexc)
{
    SSL_EXCERT *exc = *pexc;

    if (exc == NULL)
        return 1;
    /* If nothing in list, free and set to NULL */
    if (exc->certfile == NULL && exc->next == NULL) {
        ssl_excert_free(exc);
        *pexc = NULL;
        return 1;
    }
    for (; exc; exc = exc->next) {
        if (exc->certfile == NULL) {
            BIO_printf(bio_err, "Missing filename\n");
            return 0;
        }
        exc->cert = load_cert(exc->certfile, exc->certform,
                              "Server Certificate");
        if (exc->cert == NULL)
            return 0;
        if (exc->keyfile != NULL) {
            exc->key = load_key(exc->keyfile, exc->keyform,
                                0, NULL, NULL, "server key");
        } else {
            exc->key = load_key(exc->certfile, exc->certform,
                                0, NULL, NULL, "server key");
        }
        if (exc->key == NULL)
            return 0;
        if (exc->chainfile != NULL) {
            if (!load_certs(exc->chainfile, 0, &exc->chain, NULL, "server chain"))
                return 0;
        }
    }
    return 1;
}

enum range { OPT_X_ENUM };

int args_excert(int opt, SSL_EXCERT **pexc)
{
    SSL_EXCERT *exc = *pexc;

    assert(opt > OPT_X__FIRST);
    assert(opt < OPT_X__LAST);

    if (exc == NULL) {
        if (!ssl_excert_prepend(&exc)) {
            BIO_printf(bio_err, " %s: Error initialising xcert\n",
                       opt_getprog());
            goto err;
        }
        *pexc = exc;
    }

    switch ((enum range)opt) {
    case OPT_X__FIRST:
    case OPT_X__LAST:
        return 0;
    case OPT_X_CERT:
        if (exc->certfile != NULL && !ssl_excert_prepend(&exc)) {
            BIO_printf(bio_err, "%s: Error adding xcert\n", opt_getprog());
            goto err;
        }
        *pexc = exc;
        exc->certfile = opt_arg();
        break;
    case OPT_X_KEY:
        if (exc->keyfile != NULL) {
            BIO_printf(bio_err, "%s: Key already specified\n", opt_getprog());
            goto err;
        }
        exc->keyfile = opt_arg();
        break;
    case OPT_X_CHAIN:
        if (exc->chainfile != NULL) {
            BIO_printf(bio_err, "%s: Chain already specified\n",
                       opt_getprog());
            goto err;
        }
        exc->chainfile = opt_arg();
        break;
    case OPT_X_CHAIN_BUILD:
        exc->build_chain = 1;
        break;
    case OPT_X_CERTFORM:
        if (!opt_format(opt_arg(), OPT_FMT_ANY, &exc->certform))
            return 0;
        break;
    case OPT_X_KEYFORM:
        if (!opt_format(opt_arg(), OPT_FMT_ANY, &exc->keyform))
            return 0;
        break;
    }
    return 1;

 err:
    ERR_print_errors(bio_err);
    ssl_excert_free(exc);
    *pexc = NULL;
    return 0;
}

static void print_raw_cipherlist(SSL *s)
{
    const unsigned char *rlist;
    static const unsigned char scsv_id[] = { 0, 0xFF };
    size_t i, rlistlen, num;

    if (!SSL_is_server(s))
        return;
    num = SSL_get0_raw_cipherlist(s, NULL);
    OPENSSL_assert(num == 2);
    rlistlen = SSL_get0_raw_cipherlist(s, &rlist);
    BIO_puts(bio_err, "Client cipher list: ");
    for (i = 0; i < rlistlen; i += num, rlist += num) {
        const SSL_CIPHER *c = SSL_CIPHER_find(s, rlist);
        if (i)
            BIO_puts(bio_err, ":");
        if (c != NULL) {
            BIO_puts(bio_err, SSL_CIPHER_get_name(c));
        } else if (memcmp(rlist, scsv_id, num) == 0) {
            BIO_puts(bio_err, "SCSV");
        } else {
            size_t j;
            BIO_puts(bio_err, "0x");
            for (j = 0; j < num; j++)
                BIO_printf(bio_err, "%02X", rlist[j]);
        }
    }
    BIO_puts(bio_err, "\n");
}

/*
 * Hex encoder for TLSA RRdata, not ':' delimited.
 */
static char *hexencode(const unsigned char *data, size_t len)
{
    static const char *hex = "0123456789abcdef";
    char *out;
    char *cp;
    size_t outlen = 2 * len + 1;
    int ilen = (int) outlen;

    if (outlen < len || ilen < 0 || outlen != (size_t)ilen) {
        BIO_printf(bio_err, "%s: %zu-byte buffer too large to hexencode\n",
                   opt_getprog(), len);
        exit(1);
    }
    cp = out = app_malloc(ilen, "TLSA hex data buffer");

    while (len-- > 0) {
        *cp++ = hex[(*data >> 4) & 0x0f];
        *cp++ = hex[*data++ & 0x0f];
    }
    *cp = '\0';
    return out;
}

void print_verify_detail(SSL *s, BIO *bio)
{
    int mdpth;
    EVP_PKEY *mspki = NULL;
    long verify_err = SSL_get_verify_result(s);

    if (verify_err == X509_V_OK) {
        const char *peername = SSL_get0_peername(s);

        BIO_printf(bio, "Verification: OK\n");
        if (peername != NULL)
            BIO_printf(bio, "Verified peername: %s\n", peername);
    } else {
        const char *reason = X509_verify_cert_error_string(verify_err);

        BIO_printf(bio, "Verification error: %s\n", reason);
    }

    if ((mdpth = SSL_get0_dane_authority(s, NULL, &mspki)) >= 0) {
        uint8_t usage, selector, mtype;
        const unsigned char *data = NULL;
        size_t dlen = 0;
        char *hexdata;

        mdpth = SSL_get0_dane_tlsa(s, &usage, &selector, &mtype, &data, &dlen);

        /*
         * The TLSA data field can be quite long when it is a certificate,
         * public key or even a SHA2-512 digest.  Because the initial octets of
         * ASN.1 certificates and public keys contain mostly boilerplate OIDs
         * and lengths, we show the last 12 bytes of the data instead, as these
         * are more likely to distinguish distinct TLSA records.
         */
#define TLSA_TAIL_SIZE 12
        if (dlen > TLSA_TAIL_SIZE)
            hexdata = hexencode(data + dlen - TLSA_TAIL_SIZE, TLSA_TAIL_SIZE);
        else
            hexdata = hexencode(data, dlen);
        BIO_printf(bio, "DANE TLSA %d %d %d %s%s ",
                   usage, selector, mtype,
                   (dlen > TLSA_TAIL_SIZE) ? "..." : "", hexdata);
        if (SSL_get0_peer_rpk(s) == NULL)
            BIO_printf(bio, "%s certificate at depth %d\n",
                       (mspki != NULL) ? "signed the peer" :
                       mdpth ? "matched the TA" : "matched the EE", mdpth);
        else
            BIO_printf(bio, "matched the peer raw public key\n");
        OPENSSL_free(hexdata);
    }
}

void print_ssl_summary(SSL *s)
{
    const char *sigalg;
    const SSL_CIPHER *c;
    X509 *peer = SSL_get0_peer_certificate(s);
    EVP_PKEY *peer_rpk = SSL_get0_peer_rpk(s);
    int nid;

    BIO_printf(bio_err, "Protocol version: %s\n", SSL_get_version(s));
    print_raw_cipherlist(s);
    c = SSL_get_current_cipher(s);
    BIO_printf(bio_err, "Ciphersuite: %s\n", SSL_CIPHER_get_name(c));
    do_print_sigalgs(bio_err, s, 0);
    if (peer != NULL) {
        BIO_puts(bio_err, "Peer certificate: ");
        X509_NAME_print_ex(bio_err, X509_get_subject_name(peer),
                           0, get_nameopt());
        BIO_puts(bio_err, "\n");
        if (SSL_get_peer_signature_nid(s, &nid))
            BIO_printf(bio_err, "Hash used: %s\n", OBJ_nid2sn(nid));
        if (SSL_get0_peer_signature_name(s, &sigalg))
            BIO_printf(bio_err, "Signature type: %s\n", sigalg);
        print_verify_detail(s, bio_err);
    } else if (peer_rpk != NULL) {
        BIO_printf(bio_err, "Peer used raw public key\n");
        if (SSL_get0_peer_signature_name(s, &sigalg))
            BIO_printf(bio_err, "Signature type: %s\n", sigalg);
        print_verify_detail(s, bio_err);
    } else {
        BIO_puts(bio_err, "No peer certificate or raw public key\n");
    }
#ifndef OPENSSL_NO_EC
    ssl_print_point_formats(bio_err, s);
    if (SSL_is_server(s))
        ssl_print_groups(bio_err, s, 1);
#endif
    ssl_print_tmp_key(bio_err, s);
}

int config_ctx(SSL_CONF_CTX *cctx, STACK_OF(OPENSSL_STRING) *str,
               SSL_CTX *ctx)
{
    int i;

    SSL_CONF_CTX_set_ssl_ctx(cctx, ctx);
    for (i = 0; i < sk_OPENSSL_STRING_num(str); i += 2) {
        const char *flag = sk_OPENSSL_STRING_value(str, i);
        const char *arg = sk_OPENSSL_STRING_value(str, i + 1);

        if (SSL_CONF_cmd(cctx, flag, arg) <= 0) {
            BIO_printf(bio_err, "Call to SSL_CONF_cmd(%s, %s) failed\n",
                       flag, arg == NULL ? "<NULL>" : arg);
            ERR_print_errors(bio_err);
            return 0;
        }
    }
    if (!SSL_CONF_CTX_finish(cctx)) {
        BIO_puts(bio_err, "Error finishing context\n");
        ERR_print_errors(bio_err);
        return 0;
    }
    return 1;
}

static int add_crls_store(X509_STORE *st, STACK_OF(X509_CRL) *crls)
{
    X509_CRL *crl;
    int i, ret = 1;

    for (i = 0; i < sk_X509_CRL_num(crls); i++) {
        crl = sk_X509_CRL_value(crls, i);
        if (!X509_STORE_add_crl(st, crl))
            ret = 0;
    }
    return ret;
}

int ssl_ctx_add_crls(SSL_CTX *ctx, STACK_OF(X509_CRL) *crls, int crl_download)
{
    X509_STORE *st;

    st = SSL_CTX_get_cert_store(ctx);
    add_crls_store(st, crls);
    if (crl_download)
        store_setup_crl_download(st);
    return 1;
}

int ssl_load_stores(SSL_CTX *ctx,
                    const char *vfyCApath, const char *vfyCAfile,
                    const char *vfyCAstore,
                    const char *chCApath, const char *chCAfile,
                    const char *chCAstore,
                    STACK_OF(X509_CRL) *crls, int crl_download)
{
    X509_STORE *vfy = NULL, *ch = NULL;
    int rv = 0;

    if (vfyCApath != NULL || vfyCAfile != NULL || vfyCAstore != NULL) {
        vfy = X509_STORE_new();
        if (vfy == NULL)
            goto err;
        if (vfyCAfile != NULL && !X509_STORE_load_file(vfy, vfyCAfile))
            goto err;
        if (vfyCApath != NULL && !X509_STORE_load_path(vfy, vfyCApath))
            goto err;
        if (vfyCAstore != NULL && !X509_STORE_load_store(vfy, vfyCAstore))
            goto err;
        add_crls_store(vfy, crls);
        if (SSL_CTX_set1_verify_cert_store(ctx, vfy) == 0)
            goto err;
        if (crl_download)
            store_setup_crl_download(vfy);
    }
    if (chCApath != NULL || chCAfile != NULL || chCAstore != NULL) {
        ch = X509_STORE_new();
        if (ch == NULL)
            goto err;
        if (chCAfile != NULL && !X509_STORE_load_file(ch, chCAfile))
            goto err;
        if (chCApath != NULL && !X509_STORE_load_path(ch, chCApath))
            goto err;
        if (chCAstore != NULL && !X509_STORE_load_store(ch, chCAstore))
            goto err;
        if (SSL_CTX_set1_chain_cert_store(ctx, ch) == 0)
            goto err;
    }
    rv = 1;
 err:
    X509_STORE_free(vfy);
    X509_STORE_free(ch);
    return rv;
}

/* Verbose print out of security callback */

typedef struct {
    BIO *out;
    int verbose;
    int (*old_cb) (const SSL *s, const SSL_CTX *ctx, int op, int bits, int nid,
                   void *other, void *ex);
} security_debug_ex;

static STRINT_PAIR callback_types[] = {
    {"Supported Ciphersuite", SSL_SECOP_CIPHER_SUPPORTED},
    {"Shared Ciphersuite", SSL_SECOP_CIPHER_SHARED},
    {"Check Ciphersuite", SSL_SECOP_CIPHER_CHECK},
#ifndef OPENSSL_NO_DH
    {"Temp DH key bits", SSL_SECOP_TMP_DH},
#endif
    {"Supported Curve", SSL_SECOP_CURVE_SUPPORTED},
    {"Shared Curve", SSL_SECOP_CURVE_SHARED},
    {"Check Curve", SSL_SECOP_CURVE_CHECK},
    {"Supported Signature Algorithm", SSL_SECOP_SIGALG_SUPPORTED},
    {"Shared Signature Algorithm", SSL_SECOP_SIGALG_SHARED},
    {"Check Signature Algorithm", SSL_SECOP_SIGALG_CHECK},
    {"Signature Algorithm mask", SSL_SECOP_SIGALG_MASK},
    {"Certificate chain EE key", SSL_SECOP_EE_KEY},
    {"Certificate chain CA key", SSL_SECOP_CA_KEY},
    {"Peer Chain EE key", SSL_SECOP_PEER_EE_KEY},
    {"Peer Chain CA key", SSL_SECOP_PEER_CA_KEY},
    {"Certificate chain CA digest", SSL_SECOP_CA_MD},
    {"Peer chain CA digest", SSL_SECOP_PEER_CA_MD},
    {"SSL compression", SSL_SECOP_COMPRESSION},
    {"Session ticket", SSL_SECOP_TICKET},
    {NULL}
};

static int security_callback_debug(const SSL *s, const SSL_CTX *ctx,
                                   int op, int bits, int nid,
                                   void *other, void *ex)
{
    security_debug_ex *sdb = ex;
    int rv, show_bits = 1, cert_md = 0;
    const char *nm;
    int show_nm;

    rv = sdb->old_cb(s, ctx, op, bits, nid, other, ex);
    if (rv == 1 && sdb->verbose < 2)
        return 1;
    BIO_puts(sdb->out, "Security callback: ");

    nm = lookup(op, callback_types, NULL);
    show_nm = nm != NULL;
    switch (op) {
    case SSL_SECOP_TICKET:
    case SSL_SECOP_COMPRESSION:
        show_bits = 0;
        show_nm = 0;
        break;
    case SSL_SECOP_VERSION:
        BIO_printf(sdb->out, "Version=%s", lookup(nid, ssl_versions, "???"));
        show_bits = 0;
        show_nm = 0;
        break;
    case SSL_SECOP_CA_MD:
    case SSL_SECOP_PEER_CA_MD:
        cert_md = 1;
        break;
    case SSL_SECOP_SIGALG_SUPPORTED:
    case SSL_SECOP_SIGALG_SHARED:
    case SSL_SECOP_SIGALG_CHECK:
    case SSL_SECOP_SIGALG_MASK:
        show_nm = 0;
        break;
    }
    if (show_nm)
        BIO_printf(sdb->out, "%s=", nm);

    switch (op & SSL_SECOP_OTHER_TYPE) {

    case SSL_SECOP_OTHER_CIPHER:
        BIO_puts(sdb->out, SSL_CIPHER_get_name(other));
        break;

#ifndef OPENSSL_NO_EC
    case SSL_SECOP_OTHER_CURVE:
        {
            const char *cname;
            cname = EC_curve_nid2nist(nid);
            if (cname == NULL)
                cname = OBJ_nid2sn(nid);
            BIO_puts(sdb->out, cname);
        }
        break;
#endif
    case SSL_SECOP_OTHER_CERT:
        {
            if (cert_md) {
                int sig_nid = X509_get_signature_nid(other);

                BIO_puts(sdb->out, OBJ_nid2sn(sig_nid));
            } else {
                EVP_PKEY *pkey = X509_get0_pubkey(other);

                if (pkey == NULL) {
                    BIO_printf(sdb->out, "Public key missing");
                } else {
                    const char *algname = "";

                    EVP_PKEY_asn1_get0_info(NULL, NULL, NULL, NULL,
                                            &algname, EVP_PKEY_get0_asn1(pkey));
                    BIO_printf(sdb->out, "%s, bits=%d",
                            algname, EVP_PKEY_get_bits(pkey));
                }
            }
            break;
        }
    case SSL_SECOP_OTHER_SIGALG:
        {
            const unsigned char *salg = other;
            const char *sname = NULL;
            int raw_sig_code = (salg[0] << 8) + salg[1]; /* always big endian (msb, lsb) */
                /* raw_sig_code: signature_scheme from tls1.3, or signature_and_hash from tls1.2 */

            if (nm != NULL)
                BIO_printf(sdb->out, "%s", nm);
            else
                BIO_printf(sdb->out, "s_cb.c:security_callback_debug op=0x%x", op);

            sname = lookup(raw_sig_code, signature_tls13_scheme_list, NULL);
            if (sname != NULL) {
                BIO_printf(sdb->out, " scheme=%s", sname);
            } else {
                int alg_code = salg[1];
                int hash_code = salg[0];
                const char *alg_str = lookup(alg_code, signature_tls12_alg_list, NULL);
                const char *hash_str = lookup(hash_code, signature_tls12_hash_list, NULL);

                if (alg_str != NULL && hash_str != NULL)
                    BIO_printf(sdb->out, " digest=%s, algorithm=%s", hash_str, alg_str);
                else
                    BIO_printf(sdb->out, " scheme=unknown(0x%04x)", raw_sig_code);
            }
        }

    }

    if (show_bits)
        BIO_printf(sdb->out, ", security bits=%d", bits);
    BIO_printf(sdb->out, ": %s\n", rv ? "yes" : "no");
    return rv;
}

void ssl_ctx_security_debug(SSL_CTX *ctx, int verbose)
{
    static security_debug_ex sdb;

    sdb.out = bio_err;
    sdb.verbose = verbose;
    sdb.old_cb = SSL_CTX_get_security_callback(ctx);
    SSL_CTX_set_security_callback(ctx, security_callback_debug);
    SSL_CTX_set0_security_ex_data(ctx, &sdb);
}

static void keylog_callback(const SSL *ssl, const char *line)
{
    if (bio_keylog == NULL) {
        BIO_printf(bio_err, "Keylog callback is invoked without valid file!\n");
        return;
    }

    /*
     * There might be concurrent writers to the keylog file, so we must ensure
     * that the given line is written at once.
     */
    BIO_printf(bio_keylog, "%s\n", line);
    (void)BIO_flush(bio_keylog);
}

int set_keylog_file(SSL_CTX *ctx, const char *keylog_file)
{
    /* Close any open files */
    BIO_free_all(bio_keylog);
    bio_keylog = NULL;

    if (ctx == NULL || keylog_file == NULL) {
        /* Keylogging is disabled, OK. */
        return 0;
    }

    /*
     * Append rather than write in order to allow concurrent modification.
     * Furthermore, this preserves existing keylog files which is useful when
     * the tool is run multiple times.
     */
    bio_keylog = BIO_new_file(keylog_file, "a");
    if (bio_keylog == NULL) {
        BIO_printf(bio_err, "Error writing keylog file %s\n", keylog_file);
        return 1;
    }

    /* Write a header for seekable, empty files (this excludes pipes). */
    if (BIO_tell(bio_keylog) == 0) {
        BIO_puts(bio_keylog,
                 "# SSL/TLS secrets log file, generated by OpenSSL\n");
        (void)BIO_flush(bio_keylog);
    }
    SSL_CTX_set_keylog_callback(ctx, keylog_callback);
    return 0;
}

void print_ca_names(BIO *bio, SSL *s)
{
    const char *cs = SSL_is_server(s) ? "server" : "client";
    const STACK_OF(X509_NAME) *sk = SSL_get0_peer_CA_list(s);
    int i;

    if (sk == NULL || sk_X509_NAME_num(sk) == 0) {
        if (!SSL_is_server(s))
            BIO_printf(bio, "---\nNo %s certificate CA names sent\n", cs);
        return;
    }

    BIO_printf(bio, "---\nAcceptable %s certificate CA names\n", cs);
    for (i = 0; i < sk_X509_NAME_num(sk); i++) {
        X509_NAME_print_ex(bio, sk_X509_NAME_value(sk, i), 0, get_nameopt());
        BIO_write(bio, "\n", 1);
    }
}

void ssl_print_secure_renegotiation_notes(BIO *bio, SSL *s)
{
    if (SSL_VERSION_ALLOWS_RENEGOTIATION(s)) {
        BIO_printf(bio, "Secure Renegotiation IS%s supported\n",
                   SSL_get_secure_renegotiation_support(s) ? "" : " NOT");
    } else {
        BIO_printf(bio, "This TLS version forbids renegotiation.\n");
    }
}

int progress_cb(EVP_PKEY_CTX *ctx)
{
    BIO *b = EVP_PKEY_CTX_get_app_data(ctx);
    int p = EVP_PKEY_CTX_get_keygen_info(ctx, 0);
    static const char symbols[] = ".+*\n";
    char c = (p >= 0 && (size_t)p <= sizeof(symbols) - 1) ? symbols[p] : '?';

    BIO_write(b, &c, 1);
    (void)BIO_flush(b);
    return 1;
}
