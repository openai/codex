/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_APPS_OPT_H
#define OSSL_APPS_OPT_H

#include <sys/types.h>
#include <openssl/e_os2.h>
#include <openssl/types.h>
#include <stdarg.h>

#define OPT_COMMON OPT_ERR = -1, OPT_EOF = 0, OPT_HELP

/*
 * Common verification options.
 */
# define OPT_V_ENUM \
        OPT_V__FIRST=2000, \
        OPT_V_POLICY, OPT_V_PURPOSE, OPT_V_VERIFY_NAME, OPT_V_VERIFY_DEPTH, \
        OPT_V_ATTIME, OPT_V_VERIFY_HOSTNAME, OPT_V_VERIFY_EMAIL, \
        OPT_V_VERIFY_IP, OPT_V_IGNORE_CRITICAL, OPT_V_ISSUER_CHECKS, \
        OPT_V_CRL_CHECK, OPT_V_CRL_CHECK_ALL, OPT_V_POLICY_CHECK, \
        OPT_V_EXPLICIT_POLICY, OPT_V_INHIBIT_ANY, OPT_V_INHIBIT_MAP, \
        OPT_V_X509_STRICT, OPT_V_EXTENDED_CRL, OPT_V_USE_DELTAS, \
        OPT_V_POLICY_PRINT, OPT_V_CHECK_SS_SIG, OPT_V_TRUSTED_FIRST, \
        OPT_V_SUITEB_128_ONLY, OPT_V_SUITEB_128, OPT_V_SUITEB_192, \
        OPT_V_PARTIAL_CHAIN, OPT_V_NO_ALT_CHAINS, OPT_V_NO_CHECK_TIME, \
        OPT_V_VERIFY_AUTH_LEVEL, OPT_V_ALLOW_PROXY_CERTS, \
        OPT_V__LAST

# define OPT_V_OPTIONS \
        OPT_SECTION("Validation"), \
        { "policy", OPT_V_POLICY, 's', "adds policy to the acceptable policy set"}, \
        { "purpose", OPT_V_PURPOSE, 's', \
            "certificate chain purpose"}, \
        { "verify_name", OPT_V_VERIFY_NAME, 's', "verification policy name"}, \
        { "verify_depth", OPT_V_VERIFY_DEPTH, 'n', \
            "chain depth limit" }, \
        { "auth_level", OPT_V_VERIFY_AUTH_LEVEL, 'n', \
            "chain authentication security level" }, \
        { "attime", OPT_V_ATTIME, 'M', "verification epoch time" }, \
        { "verify_hostname", OPT_V_VERIFY_HOSTNAME, 's', \
            "expected peer hostname" }, \
        { "verify_email", OPT_V_VERIFY_EMAIL, 's', \
            "expected peer email" }, \
        { "verify_ip", OPT_V_VERIFY_IP, 's', \
            "expected peer IP address" }, \
        { "ignore_critical", OPT_V_IGNORE_CRITICAL, '-', \
            "permit unhandled critical extensions"}, \
        { "issuer_checks", OPT_V_ISSUER_CHECKS, '-', "(deprecated)"}, \
        { "crl_check", OPT_V_CRL_CHECK, '-', "check leaf certificate revocation" }, \
        { "crl_check_all", OPT_V_CRL_CHECK_ALL, '-', "check full chain revocation" }, \
        { "policy_check", OPT_V_POLICY_CHECK, '-', "perform rfc5280 policy checks"}, \
        { "explicit_policy", OPT_V_EXPLICIT_POLICY, '-', \
            "set policy variable require-explicit-policy"}, \
        { "inhibit_any", OPT_V_INHIBIT_ANY, '-', \
            "set policy variable inhibit-any-policy"}, \
        { "inhibit_map", OPT_V_INHIBIT_MAP, '-', \
            "set policy variable inhibit-policy-mapping"}, \
        { "x509_strict", OPT_V_X509_STRICT, '-', \
            "disable certificate compatibility work-arounds"}, \
        { "extended_crl", OPT_V_EXTENDED_CRL, '-', \
            "enable extended CRL features"}, \
        { "use_deltas", OPT_V_USE_DELTAS, '-', \
            "use delta CRLs"}, \
        { "policy_print", OPT_V_POLICY_PRINT, '-', \
            "print policy processing diagnostics"}, \
        { "check_ss_sig", OPT_V_CHECK_SS_SIG, '-', \
            "check root CA self-signatures"}, \
        { "trusted_first", OPT_V_TRUSTED_FIRST, '-', \
            "search trust store first (default)" }, \
        { "suiteB_128_only", OPT_V_SUITEB_128_ONLY, '-', "Suite B 128-bit-only mode"}, \
        { "suiteB_128", OPT_V_SUITEB_128, '-', \
            "Suite B 128-bit mode allowing 192-bit algorithms"}, \
        { "suiteB_192", OPT_V_SUITEB_192, '-', "Suite B 192-bit-only mode" }, \
        { "partial_chain", OPT_V_PARTIAL_CHAIN, '-', \
            "accept chains anchored by intermediate trust-store CAs"}, \
        { "no_alt_chains", OPT_V_NO_ALT_CHAINS, '-', "(deprecated)" }, \
        { "no_check_time", OPT_V_NO_CHECK_TIME, '-', "ignore certificate validity time" }, \
        { "allow_proxy_certs", OPT_V_ALLOW_PROXY_CERTS, '-', "allow the use of proxy certificates" }

# define OPT_V_CASES \
        OPT_V__FIRST: case OPT_V__LAST: break; \
        case OPT_V_POLICY: \
        case OPT_V_PURPOSE: \
        case OPT_V_VERIFY_NAME: \
        case OPT_V_VERIFY_DEPTH: \
        case OPT_V_VERIFY_AUTH_LEVEL: \
        case OPT_V_ATTIME: \
        case OPT_V_VERIFY_HOSTNAME: \
        case OPT_V_VERIFY_EMAIL: \
        case OPT_V_VERIFY_IP: \
        case OPT_V_IGNORE_CRITICAL: \
        case OPT_V_ISSUER_CHECKS: \
        case OPT_V_CRL_CHECK: \
        case OPT_V_CRL_CHECK_ALL: \
        case OPT_V_POLICY_CHECK: \
        case OPT_V_EXPLICIT_POLICY: \
        case OPT_V_INHIBIT_ANY: \
        case OPT_V_INHIBIT_MAP: \
        case OPT_V_X509_STRICT: \
        case OPT_V_EXTENDED_CRL: \
        case OPT_V_USE_DELTAS: \
        case OPT_V_POLICY_PRINT: \
        case OPT_V_CHECK_SS_SIG: \
        case OPT_V_TRUSTED_FIRST: \
        case OPT_V_SUITEB_128_ONLY: \
        case OPT_V_SUITEB_128: \
        case OPT_V_SUITEB_192: \
        case OPT_V_PARTIAL_CHAIN: \
        case OPT_V_NO_ALT_CHAINS: \
        case OPT_V_NO_CHECK_TIME: \
        case OPT_V_ALLOW_PROXY_CERTS

/*
 * Common "extended validation" options.
 */
# define OPT_X_ENUM \
        OPT_X__FIRST=1000, \
        OPT_X_KEY, OPT_X_CERT, OPT_X_CHAIN, OPT_X_CHAIN_BUILD, \
        OPT_X_CERTFORM, OPT_X_KEYFORM, \
        OPT_X__LAST

# define OPT_X_OPTIONS \
        OPT_SECTION("Extended certificate"), \
        { "xkey", OPT_X_KEY, '<', "key for Extended certificates"}, \
        { "xcert", OPT_X_CERT, '<', "cert for Extended certificates"}, \
        { "xchain", OPT_X_CHAIN, '<', "chain for Extended certificates"}, \
        { "xchain_build", OPT_X_CHAIN_BUILD, '-', \
            "build certificate chain for the extended certificates"}, \
        { "xcertform", OPT_X_CERTFORM, 'F', \
            "format of Extended certificate (PEM/DER/P12); has no effect" }, \
        { "xkeyform", OPT_X_KEYFORM, 'F', \
            "format of Extended certificate's key (DER/PEM/P12); has no effect"}

# define OPT_X_CASES \
        OPT_X__FIRST: case OPT_X__LAST: break; \
        case OPT_X_KEY: \
        case OPT_X_CERT: \
        case OPT_X_CHAIN: \
        case OPT_X_CHAIN_BUILD: \
        case OPT_X_CERTFORM: \
        case OPT_X_KEYFORM

/*
 * Common SSL options.
 * Any changes here must be coordinated with ../ssl/ssl_conf.c
 */
# define OPT_S_ENUM \
        OPT_S__FIRST=3000, \
        OPT_S_NOSSL3, OPT_S_NOTLS1, OPT_S_NOTLS1_1, OPT_S_NOTLS1_2, \
        OPT_S_NOTLS1_3, OPT_S_BUGS, OPT_S_NO_COMP, OPT_S_NOTICKET, \
        OPT_S_SERVERPREF, OPT_S_LEGACYRENEG, OPT_S_CLIENTRENEG, \
        OPT_S_LEGACYCONN, \
        OPT_S_ONRESUMP, OPT_S_NOLEGACYCONN, \
        OPT_S_ALLOW_NO_DHE_KEX, OPT_S_PREFER_NO_DHE_KEX, \
        OPT_S_PRIORITIZE_CHACHA, \
        OPT_S_STRICT, OPT_S_SIGALGS, OPT_S_CLIENTSIGALGS, OPT_S_GROUPS, \
        OPT_S_CURVES, OPT_S_NAMEDCURVE, OPT_S_CIPHER, OPT_S_CIPHERSUITES, \
        OPT_S_RECORD_PADDING, OPT_S_DEBUGBROKE, OPT_S_COMP, \
        OPT_S_MINPROTO, OPT_S_MAXPROTO, \
        OPT_S_NO_RENEGOTIATION, OPT_S_NO_MIDDLEBOX, OPT_S_NO_ETM, \
        OPT_S_NO_EMS, \
        OPT_S_NO_TX_CERT_COMP, \
        OPT_S_NO_RX_CERT_COMP, \
        OPT_S__LAST

# define OPT_S_OPTIONS \
        OPT_SECTION("TLS/SSL"), \
        {"no_ssl3", OPT_S_NOSSL3, '-',"Just disable SSLv3" }, \
        {"no_tls1", OPT_S_NOTLS1, '-', "Just disable TLSv1"}, \
        {"no_tls1_1", OPT_S_NOTLS1_1, '-', "Just disable TLSv1.1" }, \
        {"no_tls1_2", OPT_S_NOTLS1_2, '-', "Just disable TLSv1.2"}, \
        {"no_tls1_3", OPT_S_NOTLS1_3, '-', "Just disable TLSv1.3"}, \
        {"bugs", OPT_S_BUGS, '-', "Turn on SSL bug compatibility"}, \
        {"no_comp", OPT_S_NO_COMP, '-', "Disable SSL/TLS compression (default)" }, \
        {"comp", OPT_S_COMP, '-', "Use SSL/TLS-level compression" }, \
        {"no_tx_cert_comp", OPT_S_NO_TX_CERT_COMP, '-', "Disable sending TLSv1.3 compressed certificates" }, \
        {"no_rx_cert_comp", OPT_S_NO_RX_CERT_COMP, '-', "Disable receiving TLSv1.3 compressed certificates" }, \
        {"no_ticket", OPT_S_NOTICKET, '-', \
            "Disable use of TLS session tickets"}, \
        {"serverpref", OPT_S_SERVERPREF, '-', "Use server's cipher preferences"}, \
        {"legacy_renegotiation", OPT_S_LEGACYRENEG, '-', \
            "Enable use of legacy renegotiation (dangerous)"}, \
        {"client_renegotiation", OPT_S_CLIENTRENEG, '-', \
            "Allow client-initiated renegotiation" }, \
        {"no_renegotiation", OPT_S_NO_RENEGOTIATION, '-', \
            "Disable all renegotiation."}, \
        {"legacy_server_connect", OPT_S_LEGACYCONN, '-', \
            "Allow initial connection to servers that don't support RI"}, \
        {"no_resumption_on_reneg", OPT_S_ONRESUMP, '-', \
            "Disallow session resumption on renegotiation"}, \
        {"no_legacy_server_connect", OPT_S_NOLEGACYCONN, '-', \
            "Disallow initial connection to servers that don't support RI"}, \
        {"allow_no_dhe_kex", OPT_S_ALLOW_NO_DHE_KEX, '-', \
            "In TLSv1.3 allow non-(ec)dhe based key exchange on resumption"}, \
        {"prefer_no_dhe_kex", OPT_S_PREFER_NO_DHE_KEX, '-', \
            "In TLSv1.3 prefer non-(ec)dhe over (ec)dhe-based key exchange on resumption"}, \
        {"prioritize_chacha", OPT_S_PRIORITIZE_CHACHA, '-', \
            "Prioritize ChaCha ciphers when preferred by clients"}, \
        {"strict", OPT_S_STRICT, '-', \
            "Enforce strict certificate checks as per TLS standard"}, \
        {"sigalgs", OPT_S_SIGALGS, 's', \
            "Signature algorithms to support (colon-separated list)" }, \
        {"client_sigalgs", OPT_S_CLIENTSIGALGS, 's', \
            "Signature algorithms to support for client certificate" \
            " authentication (colon-separated list)" }, \
        {"groups", OPT_S_GROUPS, 's', \
            "Groups to advertise (colon-separated list)" }, \
        {"curves", OPT_S_CURVES, 's', \
            "Groups to advertise (colon-separated list)" }, \
        {"named_curve", OPT_S_NAMEDCURVE, 's', \
            "Elliptic curve used for ECDHE (server-side only)" }, \
        {"cipher", OPT_S_CIPHER, 's', "Specify TLSv1.2 and below cipher list to be used"}, \
        {"ciphersuites", OPT_S_CIPHERSUITES, 's', "Specify TLSv1.3 ciphersuites to be used"}, \
        {"min_protocol", OPT_S_MINPROTO, 's', "Specify the minimum protocol version to be used"}, \
        {"max_protocol", OPT_S_MAXPROTO, 's', "Specify the maximum protocol version to be used"}, \
        {"record_padding", OPT_S_RECORD_PADDING, 's', \
            "Block size to pad TLS 1.3 records to."}, \
        {"debug_broken_protocol", OPT_S_DEBUGBROKE, '-', \
            "Perform all sorts of protocol violations for testing purposes"}, \
        {"no_middlebox", OPT_S_NO_MIDDLEBOX, '-', \
            "Disable TLSv1.3 middlebox compat mode" }, \
        {"no_etm", OPT_S_NO_ETM, '-', \
            "Disable Encrypt-then-Mac extension"}, \
        {"no_ems", OPT_S_NO_EMS, '-', \
            "Disable Extended master secret extension"}

# define OPT_S_CASES \
        OPT_S__FIRST: case OPT_S__LAST: break; \
        case OPT_S_NOSSL3: \
        case OPT_S_NOTLS1: \
        case OPT_S_NOTLS1_1: \
        case OPT_S_NOTLS1_2: \
        case OPT_S_NOTLS1_3: \
        case OPT_S_BUGS: \
        case OPT_S_NO_COMP: \
        case OPT_S_COMP: \
        case OPT_S_NO_TX_CERT_COMP: \
        case OPT_S_NO_RX_CERT_COMP: \
        case OPT_S_NOTICKET: \
        case OPT_S_SERVERPREF: \
        case OPT_S_LEGACYRENEG: \
        case OPT_S_CLIENTRENEG: \
        case OPT_S_LEGACYCONN: \
        case OPT_S_ONRESUMP: \
        case OPT_S_NOLEGACYCONN: \
        case OPT_S_ALLOW_NO_DHE_KEX: \
        case OPT_S_PREFER_NO_DHE_KEX: \
        case OPT_S_PRIORITIZE_CHACHA: \
        case OPT_S_STRICT: \
        case OPT_S_SIGALGS: \
        case OPT_S_CLIENTSIGALGS: \
        case OPT_S_GROUPS: \
        case OPT_S_CURVES: \
        case OPT_S_NAMEDCURVE: \
        case OPT_S_CIPHER: \
        case OPT_S_CIPHERSUITES: \
        case OPT_S_RECORD_PADDING: \
        case OPT_S_NO_RENEGOTIATION: \
        case OPT_S_MINPROTO: \
        case OPT_S_MAXPROTO: \
        case OPT_S_DEBUGBROKE: \
        case OPT_S_NO_MIDDLEBOX: \
        case OPT_S_NO_ETM: \
        case OPT_S_NO_EMS

#define IS_NO_PROT_FLAG(o) \
 (o == OPT_S_NOSSL3 || o == OPT_S_NOTLS1 || o == OPT_S_NOTLS1_1 \
  || o == OPT_S_NOTLS1_2 || o == OPT_S_NOTLS1_3)

/*
 * Random state options.
 */
# define OPT_R_ENUM \
        OPT_R__FIRST=1500, OPT_R_RAND, OPT_R_WRITERAND, OPT_R__LAST

# define OPT_R_OPTIONS \
    OPT_SECTION("Random state"), \
    {"rand", OPT_R_RAND, 's', "Load the given file(s) into the random number generator"}, \
    {"writerand", OPT_R_WRITERAND, '>', "Write random data to the specified file"}

# define OPT_R_CASES \
        OPT_R__FIRST: case OPT_R__LAST: break; \
        case OPT_R_RAND: case OPT_R_WRITERAND

/*
 * Provider options.
 */
# define OPT_PROV_ENUM \
        OPT_PROV__FIRST=1600, \
        OPT_PROV_PROVIDER, OPT_PROV_PROVIDER_PATH, OPT_PROV_PROPQUERY, \
        OPT_PROV_PARAM, \
        OPT_PROV__LAST

# define OPT_CONFIG_OPTION \
        { "config", OPT_CONFIG, '<', "Load a configuration file (this may load modules)" }

# define OPT_PROV_OPTIONS \
        OPT_SECTION("Provider"), \
        { "provider-path", OPT_PROV_PROVIDER_PATH, 's', "Provider load path (must be before 'provider' argument if required)" }, \
        { "provider", OPT_PROV_PROVIDER, 's', "Provider to load (can be specified multiple times)" }, \
        { "provparam", OPT_PROV_PARAM, 's', "Set a provider key-value parameter" }, \
        { "propquery", OPT_PROV_PROPQUERY, 's', "Property query used when fetching algorithms" }

# define OPT_PROV_CASES \
        OPT_PROV__FIRST: case OPT_PROV__LAST: break; \
        case OPT_PROV_PROVIDER: \
        case OPT_PROV_PROVIDER_PATH: \
        case OPT_PROV_PARAM: \
        case OPT_PROV_PROPQUERY

/*
 * Option parsing.
 */
extern const char OPT_HELP_STR[];
extern const char OPT_MORE_STR[];
extern const char OPT_SECTION_STR[];
extern const char OPT_PARAM_STR[];

typedef struct options_st {
    const char *name;
    int retval;
    /*-
     * value type:
     *
     *   '-' no value (also the value zero)
     *   'n' number (type 'int')
     *   'p' positive number (type 'int')
     *   'u' unsigned number (type 'unsigned long')
     *   'l' number (type 'unsigned long')
     *   'M' number (type 'intmax_t')
     *   'U' unsigned number (type 'uintmax_t')
     *   's' string
     *   '<' input file
     *   '>' output file
     *   '/' directory
     *   'f' any format                    [OPT_FMT_ANY]
     *   'F' der/pem format                [OPT_FMT_PEMDER]
     *   'A' any ASN1, der/pem/b64 format  [OPT_FMT_ASN1]
     *   'E' der/pem/engine format         [OPT_FMT_PDE]
     *   'c' pem/der/smime format          [OPT_FMT_PDS]
     *
     * The 'l', 'n' and 'u' value types include the values zero,
     * the 'p' value type does not.
     */
    int valtype;
    const char *helpstr;
} OPTIONS;
/* Special retval values: */
#define OPT_PARAM 0 /* same as OPT_EOF usually defined in apps */
#define OPT_DUP -2 /* marks duplicate occurrence of option in help output */

/*
 * A string/int pairing; widely use for option value lookup, hence the
 * name OPT_PAIR. But that name is misleading in s_cb.c, so we also use
 * the "generic" name STRINT_PAIR.
 */
typedef struct string_int_pair_st {
    const char *name;
    int retval;
} OPT_PAIR, STRINT_PAIR;

/* Flags to pass into opt_format; see FORMAT_xxx, below. */
# define OPT_FMT_PEM             (1L <<  1)
# define OPT_FMT_DER             (1L <<  2)
# define OPT_FMT_B64             (1L <<  3)
# define OPT_FMT_PKCS12          (1L <<  4)
# define OPT_FMT_SMIME           (1L <<  5)
# define OPT_FMT_ENGINE          (1L <<  6)
# define OPT_FMT_MSBLOB          (1L <<  7)
# define OPT_FMT_NSS             (1L <<  8)
# define OPT_FMT_TEXT            (1L <<  9)
# define OPT_FMT_HTTP            (1L << 10)
# define OPT_FMT_PVK             (1L << 11)

# define OPT_FMT_PEMDER  (OPT_FMT_PEM | OPT_FMT_DER)
# define OPT_FMT_ASN1    (OPT_FMT_PEM | OPT_FMT_DER | OPT_FMT_B64)
# define OPT_FMT_PDE     (OPT_FMT_PEMDER | OPT_FMT_ENGINE)
# define OPT_FMT_PDS     (OPT_FMT_PEMDER | OPT_FMT_SMIME)
# define OPT_FMT_ANY     ( \
        OPT_FMT_PEM | OPT_FMT_DER | OPT_FMT_B64 | \
        OPT_FMT_PKCS12 | OPT_FMT_SMIME |                     \
        OPT_FMT_ENGINE | OPT_FMT_MSBLOB | OPT_FMT_NSS | \
        OPT_FMT_TEXT | OPT_FMT_HTTP | OPT_FMT_PVK)

/* Divide options into sections when displaying usage */
#define OPT_SECTION(sec) { OPT_SECTION_STR, 1, '-', sec " options:\n" }
#define OPT_PARAMETERS() { OPT_PARAM_STR, 1, '-', "Parameters:\n" }

const char *opt_path_end(const char *filename);
char *opt_init(int ac, char **av, const OPTIONS *o);
char *opt_progname(const char *argv0);
char *opt_appname(const char *argv0);
char *opt_getprog(void);
void opt_help(const OPTIONS *list);

void opt_begin(void);
int opt_next(void);
char *opt_flag(void);
char *opt_arg(void);
char *opt_unknown(void);
void reset_unknown(void);
int opt_cipher(const char *name, EVP_CIPHER **cipherp);
int opt_cipher_any(const char *name, EVP_CIPHER **cipherp);
int opt_cipher_silent(const char *name, EVP_CIPHER **cipherp);
int opt_check_md(const char *name);
int opt_md(const char *name, EVP_MD **mdp);
int opt_md_silent(const char *name, EVP_MD **mdp);

int opt_int(const char *arg, int *result);
void opt_set_unknown_name(const char *name);
int opt_int_arg(void);
int opt_long(const char *arg, long *result);
int opt_ulong(const char *arg, unsigned long *result);
int opt_intmax(const char *arg, ossl_intmax_t *result);
int opt_uintmax(const char *arg, ossl_uintmax_t *result);

int opt_isdir(const char *name);
int opt_format(const char *s, unsigned long flags, int *result);
void print_format_error(int format, unsigned long flags);
int opt_printf_stderr(const char *fmt, ...);
int opt_string(const char *name, const char **options);
int opt_pair(const char *arg, const OPT_PAIR *pairs, int *result);

int opt_verify(int i, X509_VERIFY_PARAM *vpm);
int opt_rand(int i);
int opt_provider(int i);
int opt_provider_option_given(void);

char **opt_rest(void);
int opt_num_rest(void);
int opt_check_rest_arg(const char *expected);

/* Returns non-zero if legacy paths are still available */
int opt_legacy_okay(void);


#endif /* OSSL_APPS_OPT_H */
