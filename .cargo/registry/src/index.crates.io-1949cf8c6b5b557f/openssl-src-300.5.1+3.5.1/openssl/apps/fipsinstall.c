/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/provider.h>
#include <openssl/params.h>
#include <openssl/fips_names.h>
#include <openssl/core_names.h>
#include <openssl/self_test.h>
#include <openssl/fipskey.h>
#include "apps.h"
#include "progs.h"

#define BUFSIZE 4096

/* Configuration file values */
#define VERSION_KEY  "version"
#define VERSION_VAL  "1"
#define INSTALL_STATUS_VAL "INSTALL_SELF_TEST_KATS_RUN"

static OSSL_CALLBACK self_test_events;
static char *self_test_corrupt_desc = NULL;
static char *self_test_corrupt_type = NULL;
static int self_test_log = 1;
static int quiet = 0;

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_IN, OPT_OUT, OPT_MODULE, OPT_PEDANTIC,
    OPT_PROV_NAME, OPT_SECTION_NAME, OPT_MAC_NAME, OPT_MACOPT, OPT_VERIFY,
    OPT_NO_LOG, OPT_CORRUPT_DESC, OPT_CORRUPT_TYPE, OPT_QUIET, OPT_CONFIG,
    OPT_NO_CONDITIONAL_ERRORS,
    OPT_NO_SECURITY_CHECKS,
    OPT_TLS_PRF_EMS_CHECK, OPT_NO_SHORT_MAC,
    OPT_DISALLOW_PKCS15_PADDING, OPT_RSA_PSS_SALTLEN_CHECK,
    OPT_DISALLOW_SIGNATURE_X931_PADDING,
    OPT_HMAC_KEY_CHECK, OPT_KMAC_KEY_CHECK,
    OPT_DISALLOW_DRGB_TRUNC_DIGEST,
    OPT_SIGNATURE_DIGEST_CHECK,
    OPT_HKDF_DIGEST_CHECK,
    OPT_TLS13_KDF_DIGEST_CHECK,
    OPT_TLS1_PRF_DIGEST_CHECK,
    OPT_SSHKDF_DIGEST_CHECK,
    OPT_SSKDF_DIGEST_CHECK,
    OPT_X963KDF_DIGEST_CHECK,
    OPT_DISALLOW_DSA_SIGN,
    OPT_DISALLOW_TDES_ENCRYPT,
    OPT_HKDF_KEY_CHECK,
    OPT_KBKDF_KEY_CHECK,
    OPT_TLS13_KDF_KEY_CHECK,
    OPT_TLS1_PRF_KEY_CHECK,
    OPT_SSHKDF_KEY_CHECK,
    OPT_SSKDF_KEY_CHECK,
    OPT_X963KDF_KEY_CHECK,
    OPT_X942KDF_KEY_CHECK,
    OPT_NO_PBKDF2_LOWER_BOUND_CHECK,
    OPT_ECDH_COFACTOR_CHECK,
    OPT_SELF_TEST_ONLOAD, OPT_SELF_TEST_ONINSTALL
} OPTION_CHOICE;

const OPTIONS fipsinstall_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"pedantic", OPT_PEDANTIC, '-', "Set options for strict FIPS compliance"},
    {"verify", OPT_VERIFY, '-',
     "Verify a config file instead of generating one"},
    {"module", OPT_MODULE, '<', "File name of the provider module"},
    {"provider_name", OPT_PROV_NAME, 's', "FIPS provider name"},
    {"section_name", OPT_SECTION_NAME, 's',
     "FIPS Provider config section name (optional)"},
    {"no_conditional_errors", OPT_NO_CONDITIONAL_ERRORS, '-',
     "Disable the ability of the fips module to enter an error state if"
     " any conditional self tests fail"},
    {"no_security_checks", OPT_NO_SECURITY_CHECKS, '-',
     "Disable the run-time FIPS security checks in the module"},
    {"self_test_onload", OPT_SELF_TEST_ONLOAD, '-',
     "Forces self tests to always run on module load"},
    {"self_test_oninstall", OPT_SELF_TEST_ONINSTALL, '-',
     "Forces self tests to run once on module installation"},
    {"ems_check", OPT_TLS_PRF_EMS_CHECK, '-',
     "Enable the run-time FIPS check for EMS during TLS1_PRF"},
    {"no_short_mac", OPT_NO_SHORT_MAC, '-', "Disallow short MAC output"},
    {"no_drbg_truncated_digests", OPT_DISALLOW_DRGB_TRUNC_DIGEST, '-',
     "Disallow truncated digests with Hash and HMAC DRBGs"},
    {"signature_digest_check", OPT_SIGNATURE_DIGEST_CHECK, '-',
     "Enable checking for approved digests for signatures"},
    {"hmac_key_check", OPT_HMAC_KEY_CHECK, '-', "Enable key check for HMAC"},
    {"kmac_key_check", OPT_KMAC_KEY_CHECK, '-', "Enable key check for KMAC"},
    {"hkdf_digest_check", OPT_HKDF_DIGEST_CHECK, '-',
     "Enable digest check for HKDF"},
    {"tls13_kdf_digest_check", OPT_TLS13_KDF_DIGEST_CHECK, '-',
     "Enable digest check for TLS13-KDF"},
    {"tls1_prf_digest_check", OPT_TLS1_PRF_DIGEST_CHECK, '-',
     "Enable digest check for TLS1-PRF"},
    {"sshkdf_digest_check", OPT_SSHKDF_DIGEST_CHECK, '-',
     "Enable digest check for SSHKDF"},
    {"sskdf_digest_check", OPT_SSKDF_DIGEST_CHECK, '-',
     "Enable digest check for SSKDF"},
    {"x963kdf_digest_check", OPT_X963KDF_DIGEST_CHECK, '-',
     "Enable digest check for X963KDF"},
    {"dsa_sign_disabled", OPT_DISALLOW_DSA_SIGN, '-',
     "Disallow DSA signing"},
    {"tdes_encrypt_disabled", OPT_DISALLOW_TDES_ENCRYPT, '-',
     "Disallow Triple-DES encryption"},
    {"rsa_pkcs15_padding_disabled", OPT_DISALLOW_PKCS15_PADDING, '-',
     "Disallow PKCS#1 version 1.5 padding for RSA encryption"},
    {"rsa_pss_saltlen_check", OPT_RSA_PSS_SALTLEN_CHECK, '-',
     "Enable salt length check for RSA-PSS signature operations"},
    {"rsa_sign_x931_disabled", OPT_DISALLOW_SIGNATURE_X931_PADDING, '-',
     "Disallow X931 Padding for RSA signing"},
    {"hkdf_key_check", OPT_HKDF_KEY_CHECK, '-',
     "Enable key check for HKDF"},
    {"kbkdf_key_check", OPT_KBKDF_KEY_CHECK, '-',
     "Enable key check for KBKDF"},
    {"tls13_kdf_key_check", OPT_TLS13_KDF_KEY_CHECK, '-',
     "Enable key check for TLS13-KDF"},
    {"tls1_prf_key_check", OPT_TLS1_PRF_KEY_CHECK, '-',
     "Enable key check for TLS1-PRF"},
    {"sshkdf_key_check", OPT_SSHKDF_KEY_CHECK, '-',
     "Enable key check for SSHKDF"},
    {"sskdf_key_check", OPT_SSKDF_KEY_CHECK, '-',
     "Enable key check for SSKDF"},
    {"x963kdf_key_check", OPT_X963KDF_KEY_CHECK, '-',
     "Enable key check for X963KDF"},
    {"x942kdf_key_check", OPT_X942KDF_KEY_CHECK, '-',
     "Enable key check for X942KDF"},
    {"no_pbkdf2_lower_bound_check", OPT_NO_PBKDF2_LOWER_BOUND_CHECK, '-',
     "Disable lower bound check for PBKDF2"},
    {"ecdh_cofactor_check", OPT_ECDH_COFACTOR_CHECK, '-',
     "Enable Cofactor check for ECDH"},
    OPT_SECTION("Input"),
    {"in", OPT_IN, '<', "Input config file, used when verifying"},

    OPT_SECTION("Output"),
    {"out", OPT_OUT, '>', "Output config file, used when generating"},
    {"mac_name", OPT_MAC_NAME, 's', "MAC name"},
    {"macopt", OPT_MACOPT, 's', "MAC algorithm parameters in n:v form."},
    {OPT_MORE_STR, 0, 0, "See 'PARAMETER NAMES' in the EVP_MAC_ docs"},
    {"noout", OPT_NO_LOG, '-', "Disable logging of self test events"},
    {"corrupt_desc", OPT_CORRUPT_DESC, 's', "Corrupt a self test by description"},
    {"corrupt_type", OPT_CORRUPT_TYPE, 's', "Corrupt a self test by type"},
    {"config", OPT_CONFIG, '<', "The parent config to verify"},
    {"quiet", OPT_QUIET, '-', "No messages, just exit status"},
    {NULL}
};

typedef struct {
    unsigned int self_test_onload : 1;
    unsigned int conditional_errors : 1;
    unsigned int security_checks : 1;
    unsigned int hmac_key_check : 1;
    unsigned int kmac_key_check : 1;
    unsigned int tls_prf_ems_check : 1;
    unsigned int no_short_mac : 1;
    unsigned int drgb_no_trunc_dgst : 1;
    unsigned int signature_digest_check : 1;
    unsigned int hkdf_digest_check : 1;
    unsigned int tls13_kdf_digest_check : 1;
    unsigned int tls1_prf_digest_check : 1;
    unsigned int sshkdf_digest_check : 1;
    unsigned int sskdf_digest_check : 1;
    unsigned int x963kdf_digest_check : 1;
    unsigned int dsa_sign_disabled : 1;
    unsigned int tdes_encrypt_disabled : 1;
    unsigned int rsa_pkcs15_padding_disabled : 1;
    unsigned int rsa_pss_saltlen_check : 1;
    unsigned int sign_x931_padding_disabled : 1;
    unsigned int hkdf_key_check : 1;
    unsigned int kbkdf_key_check : 1;
    unsigned int tls13_kdf_key_check : 1;
    unsigned int tls1_prf_key_check : 1;
    unsigned int sshkdf_key_check : 1;
    unsigned int sskdf_key_check : 1;
    unsigned int x963kdf_key_check : 1;
    unsigned int x942kdf_key_check : 1;
    unsigned int pbkdf2_lower_bound_check : 1;
    unsigned int ecdh_cofactor_check : 1;
} FIPS_OPTS;

/* Pedantic FIPS compliance */
static const FIPS_OPTS pedantic_opts = {
    1,      /* self_test_onload */
    1,      /* conditional_errors */
    1,      /* security_checks */
    1,      /* hmac_key_check */
    1,      /* kmac_key_check */
    1,      /* tls_prf_ems_check */
    1,      /* no_short_mac */
    1,      /* drgb_no_trunc_dgst */
    1,      /* signature_digest_check */
    1,      /* hkdf_digest_check */
    1,      /* tls13_kdf_digest_check */
    1,      /* tls1_prf_digest_check */
    1,      /* sshkdf_digest_check */
    1,      /* sskdf_digest_check */
    1,      /* x963kdf_digest_check */
    1,      /* dsa_sign_disabled */
    1,      /* tdes_encrypt_disabled */
    1,      /* rsa_pkcs15_padding_disabled */
    1,      /* rsa_pss_saltlen_check */
    1,      /* sign_x931_padding_disabled */
    1,      /* hkdf_key_check */
    1,      /* kbkdf_key_check */
    1,      /* tls13_kdf_key_check */
    1,      /* tls1_prf_key_check */
    1,      /* sshkdf_key_check */
    1,      /* sskdf_key_check */
    1,      /* x963kdf_key_check */
    1,      /* x942kdf_key_check */
    1,      /* pbkdf2_lower_bound_check */
    1,      /* ecdh_cofactor_check */
};

/* Default FIPS settings for backward compatibility */
static FIPS_OPTS fips_opts = {
    1,      /* self_test_onload */
    1,      /* conditional_errors */
    1,      /* security_checks */
    0,      /* hmac_key_check */
    0,      /* kmac_key_check */
    0,      /* tls_prf_ems_check */
    0,      /* no_short_mac */
    0,      /* drgb_no_trunc_dgst */
    0,      /* signature_digest_check */
    0,      /* hkdf_digest_check */
    0,      /* tls13_kdf_digest_check */
    0,      /* tls1_prf_digest_check */
    0,      /* sshkdf_digest_check */
    0,      /* sskdf_digest_check */
    0,      /* x963kdf_digest_check */
    0,      /* dsa_sign_disabled */
    0,      /* tdes_encrypt_disabled */
    0,      /* rsa_pkcs15_padding_disabled */
    0,      /* rsa_pss_saltlen_check */
    0,      /* sign_x931_padding_disabled */
    0,      /* hkdf_key_check */
    0,      /* kbkdf_key_check */
    0,      /* tls13_kdf_key_check */
    0,      /* tls1_prf_key_check */
    0,      /* sshkdf_key_check */
    0,      /* sskdf_key_check */
    0,      /* x963kdf_key_check */
    0,      /* x942kdf_key_check */
    1,      /* pbkdf2_lower_bound_check */
    0,      /* ecdh_cofactor_check */
};

static int check_non_pedantic_fips(int pedantic, const char *name)
{
    if (pedantic) {
        BIO_printf(bio_err, "Cannot specify -%s after -pedantic\n", name);
        return 0;
    }
    return 1;
}

static int do_mac(EVP_MAC_CTX *ctx, unsigned char *tmp, BIO *in,
                  unsigned char *out, size_t *out_len)
{
    int ret = 0;
    int i;
    size_t outsz = *out_len;

    if (!EVP_MAC_init(ctx, NULL, 0, NULL))
        goto err;
    if (EVP_MAC_CTX_get_mac_size(ctx) > outsz)
        goto end;
    while ((i = BIO_read(in, (char *)tmp, BUFSIZE)) != 0) {
        if (i < 0 || !EVP_MAC_update(ctx, tmp, i))
            goto err;
    }
end:
    if (!EVP_MAC_final(ctx, out, out_len, outsz))
        goto err;
    ret = 1;
err:
    return ret;
}

static int load_fips_prov_and_run_self_test(const char *prov_name,
                                            int *is_fips_140_2_prov)
{
    int ret = 0;
    OSSL_PROVIDER *prov = NULL;
    OSSL_PARAM params[4], *p = params;
    char *name = "", *vers = "", *build = "";

    prov = OSSL_PROVIDER_load(NULL, prov_name);
    if (prov == NULL) {
        BIO_printf(bio_err, "Failed to load FIPS module\n");
        goto end;
    }
    if (!quiet) {
        *p++ = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_NAME,
                                             &name, sizeof(name));
        *p++ = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_VERSION,
                                             &vers, sizeof(vers));
        *p++ = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_BUILDINFO,
                                             &build, sizeof(build));
        *p = OSSL_PARAM_construct_end();
        if (!OSSL_PROVIDER_get_params(prov, params)) {
            BIO_printf(bio_err, "Failed to query FIPS module parameters\n");
            goto end;
        }
        if (OSSL_PARAM_modified(params))
            BIO_printf(bio_err, "\t%-10s\t%s\n", "name:", name);
        if (OSSL_PARAM_modified(params + 1))
            BIO_printf(bio_err, "\t%-10s\t%s\n", "version:", vers);
        if (OSSL_PARAM_modified(params + 2))
            BIO_printf(bio_err, "\t%-10s\t%s\n", "build:", build);
    } else {
        *p++ = OSSL_PARAM_construct_utf8_ptr(OSSL_PROV_PARAM_VERSION,
                                             &vers, sizeof(vers));
        *p = OSSL_PARAM_construct_end();
        if (!OSSL_PROVIDER_get_params(prov, params)) {
            BIO_printf(bio_err, "Failed to query FIPS module parameters\n");
            goto end;
        }
    }
    *is_fips_140_2_prov = (strncmp("3.0.", vers, 4) == 0);
    ret = 1;
end:
    OSSL_PROVIDER_unload(prov);
    return ret;
}

static int print_mac(BIO *bio, const char *label, const unsigned char *mac,
                     size_t len)
{
    int ret;
    char *hexstr = NULL;

    hexstr = OPENSSL_buf2hexstr(mac, (long)len);
    if (hexstr == NULL)
        return 0;
    ret = BIO_printf(bio, "%s = %s\n", label, hexstr);
    OPENSSL_free(hexstr);
    return ret;
}

static int write_config_header(BIO *out, const char *prov_name,
                               const char *section)
{
    return BIO_printf(out, "openssl_conf = openssl_init\n\n")
           && BIO_printf(out, "[openssl_init]\n")
           && BIO_printf(out, "providers = provider_section\n\n")
           && BIO_printf(out, "[provider_section]\n")
           && BIO_printf(out, "%s = %s\n\n", prov_name, section);
}

/*
 * Outputs a fips related config file that contains entries for the fips
 * module checksum, installation indicator checksum and the options
 * conditional_errors and security_checks.
 *
 * Returns 1 if the config file is written otherwise it returns 0 on error.
 */
static int write_config_fips_section(BIO *out, const char *section,
                                     unsigned char *module_mac,
                                     size_t module_mac_len,
                                     const FIPS_OPTS *opts,
                                     unsigned char *install_mac,
                                     size_t install_mac_len)
{
    int ret = 0;

    if (BIO_printf(out, "[%s]\n", section) <= 0
        || BIO_printf(out, "activate = 1\n") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_FIPS_PARAM_INSTALL_VERSION,
                      VERSION_VAL) <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_FIPS_PARAM_CONDITIONAL_ERRORS,
                      opts->conditional_errors ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_SECURITY_CHECKS,
                      opts->security_checks ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_HMAC_KEY_CHECK,
                      opts->hmac_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_KMAC_KEY_CHECK,
                      opts->kmac_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_TLS1_PRF_EMS_CHECK,
                      opts->tls_prf_ems_check ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_NO_SHORT_MAC,
                      opts->no_short_mac ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_DRBG_TRUNC_DIGEST,
                      opts->drgb_no_trunc_dgst ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_SIGNATURE_DIGEST_CHECK,
                      opts->signature_digest_check ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_HKDF_DIGEST_CHECK,
                      opts->hkdf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_TLS13_KDF_DIGEST_CHECK,
                      opts->tls13_kdf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_TLS1_PRF_DIGEST_CHECK,
                      opts->tls1_prf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_SSHKDF_DIGEST_CHECK,
                      opts->sshkdf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_SSKDF_DIGEST_CHECK,
                      opts->sskdf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_X963KDF_DIGEST_CHECK,
                      opts->x963kdf_digest_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_DSA_SIGN_DISABLED,
                      opts->dsa_sign_disabled ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_TDES_ENCRYPT_DISABLED,
                      opts->tdes_encrypt_disabled ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_RSA_PKCS15_PAD_DISABLED,
                      opts->rsa_pkcs15_padding_disabled ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_RSA_PSS_SALTLEN_CHECK,
                      opts->rsa_pss_saltlen_check ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_RSA_SIGN_X931_PAD_DISABLED,
                      opts->sign_x931_padding_disabled ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_HKDF_KEY_CHECK,
                      opts->hkdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_KBKDF_KEY_CHECK,
                      opts->kbkdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_TLS13_KDF_KEY_CHECK,
                      opts->tls13_kdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_TLS1_PRF_KEY_CHECK,
                      opts->tls1_prf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_SSHKDF_KEY_CHECK,
                      opts->sshkdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_SSKDF_KEY_CHECK,
                      opts->sskdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_X963KDF_KEY_CHECK,
                      opts->x963kdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_X942KDF_KEY_CHECK,
                      opts->x942kdf_key_check ? "1": "0") <= 0
        || BIO_printf(out, "%s = %s\n",
                      OSSL_PROV_PARAM_PBKDF2_LOWER_BOUND_CHECK,
                      opts->pbkdf2_lower_bound_check ? "1" : "0") <= 0
        || BIO_printf(out, "%s = %s\n", OSSL_PROV_PARAM_ECDH_COFACTOR_CHECK,
                      opts->ecdh_cofactor_check ? "1": "0") <= 0
        || !print_mac(out, OSSL_PROV_FIPS_PARAM_MODULE_MAC, module_mac,
                      module_mac_len))
        goto end;

    if (install_mac != NULL
            && install_mac_len > 0
            && opts->self_test_onload == 0) {
        if (!print_mac(out, OSSL_PROV_FIPS_PARAM_INSTALL_MAC, install_mac,
                       install_mac_len)
            || BIO_printf(out, "%s = %s\n", OSSL_PROV_FIPS_PARAM_INSTALL_STATUS,
                          INSTALL_STATUS_VAL) <= 0)
            goto end;
    }
    ret = 1;
end:
    return ret;
}

static CONF *generate_config_and_load(const char *prov_name,
                                      const char *section,
                                      unsigned char *module_mac,
                                      size_t module_mac_len,
                                      const FIPS_OPTS *opts)
{
    BIO *mem_bio = NULL;
    CONF *conf = NULL;

    mem_bio = BIO_new(BIO_s_mem());
    if (mem_bio == NULL)
        return 0;
    if (!write_config_header(mem_bio, prov_name, section)
        || !write_config_fips_section(mem_bio, section,
                                      module_mac, module_mac_len,
                                      opts, NULL, 0))
        goto end;

    conf = app_load_config_bio(mem_bio, NULL);
    if (conf == NULL)
        goto end;

    if (CONF_modules_load(conf, NULL, 0) <= 0)
        goto end;
    BIO_free(mem_bio);
    return conf;
end:
    NCONF_free(conf);
    BIO_free(mem_bio);
    return NULL;
}

static void free_config_and_unload(CONF *conf)
{
    if (conf != NULL) {
        NCONF_free(conf);
        CONF_modules_unload(1);
    }
}

static int verify_module_load(const char *parent_config_file)
{
    return OSSL_LIB_CTX_load_config(NULL, parent_config_file);
}

/*
 * Returns 1 if the config file entries match the passed in module_mac and
 * install_mac values, otherwise it returns 0.
 */
static int verify_config(const char *infile, const char *section,
                         unsigned char *module_mac, size_t module_mac_len,
                         unsigned char *install_mac, size_t install_mac_len)
{
    int ret = 0;
    char *s = NULL;
    unsigned char *buf1 = NULL, *buf2 = NULL;
    long len;
    CONF *conf = NULL;

    /* read in the existing values and check they match the saved values */
    conf = app_load_config(infile);
    if (conf == NULL)
        goto end;

    s = NCONF_get_string(conf, section, OSSL_PROV_FIPS_PARAM_INSTALL_VERSION);
    if (s == NULL || strcmp(s, VERSION_VAL) != 0) {
        BIO_printf(bio_err, "version not found\n");
        goto end;
    }
    s = NCONF_get_string(conf, section, OSSL_PROV_FIPS_PARAM_MODULE_MAC);
    if (s == NULL) {
        BIO_printf(bio_err, "Module integrity MAC not found\n");
        goto end;
    }
    buf1 = OPENSSL_hexstr2buf(s, &len);
    if (buf1 == NULL
            || (size_t)len != module_mac_len
            || memcmp(module_mac, buf1, module_mac_len) != 0) {
        BIO_printf(bio_err, "Module integrity mismatch\n");
        goto end;
    }
    if (install_mac != NULL && install_mac_len > 0) {
        s = NCONF_get_string(conf, section, OSSL_PROV_FIPS_PARAM_INSTALL_STATUS);
        if (s == NULL || strcmp(s, INSTALL_STATUS_VAL) != 0) {
            BIO_printf(bio_err, "install status not found\n");
            goto end;
        }
        s = NCONF_get_string(conf, section, OSSL_PROV_FIPS_PARAM_INSTALL_MAC);
        if (s == NULL) {
            BIO_printf(bio_err, "Install indicator MAC not found\n");
            goto end;
        }
        buf2 = OPENSSL_hexstr2buf(s, &len);
        if (buf2 == NULL
                || (size_t)len != install_mac_len
                || memcmp(install_mac, buf2, install_mac_len) != 0) {
            BIO_printf(bio_err, "Install indicator status mismatch\n");
            goto end;
        }
    }
    ret = 1;
end:
    OPENSSL_free(buf1);
    OPENSSL_free(buf2);
    NCONF_free(conf);
    return ret;
}

int fipsinstall_main(int argc, char **argv)
{
    int ret = 1, verify = 0, gotkey = 0, gotdigest = 0, pedantic = 0;
    int is_fips_140_2_prov = 0, set_selftest_onload_option = 0;
    const char *section_name = "fips_sect";
    const char *mac_name = "HMAC";
    const char *prov_name = "fips";
    BIO *module_bio = NULL, *mem_bio = NULL, *fout = NULL;
    char *in_fname = NULL, *out_fname = NULL, *prog;
    char *module_fname = NULL, *parent_config = NULL, *module_path = NULL;
    const char *tail;
    EVP_MAC_CTX *ctx = NULL, *ctx2 = NULL;
    STACK_OF(OPENSSL_STRING) *opts = NULL;
    OPTION_CHOICE o;
    unsigned char *read_buffer = NULL;
    unsigned char module_mac[EVP_MAX_MD_SIZE];
    size_t module_mac_len = EVP_MAX_MD_SIZE;
    unsigned char install_mac[EVP_MAX_MD_SIZE];
    size_t install_mac_len = EVP_MAX_MD_SIZE;
    EVP_MAC *mac = NULL;
    CONF *conf = NULL;

    if ((opts = sk_OPENSSL_STRING_new_null()) == NULL)
        goto end;

    prog = opt_init(argc, argv, fipsinstall_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto cleanup;
        case OPT_HELP:
            opt_help(fipsinstall_options);
            ret = 0;
            goto end;
        case OPT_IN:
            in_fname = opt_arg();
            break;
        case OPT_OUT:
            out_fname = opt_arg();
            break;
        case OPT_PEDANTIC:
            fips_opts = pedantic_opts;
            pedantic = 1;
            break;
        case OPT_NO_CONDITIONAL_ERRORS:
            if (!check_non_pedantic_fips(pedantic, "no_conditional_errors"))
                goto end;
            fips_opts.conditional_errors = 0;
            break;
        case OPT_NO_SECURITY_CHECKS:
            if (!check_non_pedantic_fips(pedantic, "no_security_checks"))
                goto end;
            fips_opts.security_checks = 0;
            break;
        case OPT_HMAC_KEY_CHECK:
            fips_opts.hmac_key_check = 1;
            break;
        case OPT_KMAC_KEY_CHECK:
            fips_opts.kmac_key_check = 1;
            break;
        case OPT_TLS_PRF_EMS_CHECK:
            fips_opts.tls_prf_ems_check = 1;
            break;
        case OPT_NO_SHORT_MAC:
            fips_opts.no_short_mac = 1;
            break;
        case OPT_DISALLOW_DRGB_TRUNC_DIGEST:
            fips_opts.drgb_no_trunc_dgst = 1;
            break;
        case OPT_SIGNATURE_DIGEST_CHECK:
            fips_opts.signature_digest_check = 1;
            break;
        case OPT_HKDF_DIGEST_CHECK:
            fips_opts.hkdf_digest_check = 1;
            break;
        case OPT_TLS13_KDF_DIGEST_CHECK:
            fips_opts.tls13_kdf_digest_check = 1;
            break;
        case OPT_TLS1_PRF_DIGEST_CHECK:
            fips_opts.tls1_prf_digest_check = 1;
            break;
        case OPT_SSHKDF_DIGEST_CHECK:
            fips_opts.sshkdf_digest_check = 1;
            break;
        case OPT_SSKDF_DIGEST_CHECK:
            fips_opts.sskdf_digest_check = 1;
            break;
        case OPT_X963KDF_DIGEST_CHECK:
            fips_opts.x963kdf_digest_check = 1;
            break;
        case OPT_DISALLOW_DSA_SIGN:
            fips_opts.dsa_sign_disabled = 1;
            break;
        case OPT_DISALLOW_TDES_ENCRYPT:
            fips_opts.tdes_encrypt_disabled = 1;
            break;
        case OPT_RSA_PSS_SALTLEN_CHECK:
            fips_opts.rsa_pss_saltlen_check = 1;
            break;
        case OPT_DISALLOW_SIGNATURE_X931_PADDING:
            fips_opts.sign_x931_padding_disabled = 1;
            break;
        case OPT_DISALLOW_PKCS15_PADDING:
            fips_opts.rsa_pkcs15_padding_disabled = 1;
            break;
        case OPT_HKDF_KEY_CHECK:
            fips_opts.hkdf_key_check = 1;
            break;
        case OPT_KBKDF_KEY_CHECK:
            fips_opts.kbkdf_key_check = 1;
            break;
        case OPT_TLS13_KDF_KEY_CHECK:
            fips_opts.tls13_kdf_key_check = 1;
            break;
        case OPT_TLS1_PRF_KEY_CHECK:
            fips_opts.tls1_prf_key_check = 1;
            break;
        case OPT_SSHKDF_KEY_CHECK:
            fips_opts.sshkdf_key_check = 1;
            break;
        case OPT_SSKDF_KEY_CHECK:
            fips_opts.sskdf_key_check = 1;
            break;
        case OPT_X963KDF_KEY_CHECK:
            fips_opts.x963kdf_key_check = 1;
            break;
        case OPT_X942KDF_KEY_CHECK:
            fips_opts.x942kdf_key_check = 1;
            break;
        case OPT_NO_PBKDF2_LOWER_BOUND_CHECK:
            if (!check_non_pedantic_fips(pedantic, "no_pbkdf2_lower_bound_check"))
                goto end;
            fips_opts.pbkdf2_lower_bound_check = 0;
            break;
        case OPT_ECDH_COFACTOR_CHECK:
            fips_opts.ecdh_cofactor_check = 1;
            break;
        case OPT_QUIET:
            quiet = 1;
            /* FALLTHROUGH */
        case OPT_NO_LOG:
            self_test_log = 0;
            break;
        case OPT_CORRUPT_DESC:
            self_test_corrupt_desc = opt_arg();
            break;
        case OPT_CORRUPT_TYPE:
            self_test_corrupt_type = opt_arg();
            break;
        case OPT_PROV_NAME:
            prov_name = opt_arg();
            break;
        case OPT_MODULE:
            module_fname = opt_arg();
            break;
        case OPT_SECTION_NAME:
            section_name = opt_arg();
            break;
        case OPT_MAC_NAME:
            mac_name = opt_arg();
            break;
        case OPT_CONFIG:
            parent_config = opt_arg();
            break;
        case OPT_MACOPT:
            if (!sk_OPENSSL_STRING_push(opts, opt_arg()))
                goto opthelp;
            if (HAS_PREFIX(opt_arg(), "hexkey:"))
                gotkey = 1;
            else if (HAS_PREFIX(opt_arg(), "digest:"))
                gotdigest = 1;
            break;
        case OPT_VERIFY:
            verify = 1;
            break;
        case OPT_SELF_TEST_ONLOAD:
            set_selftest_onload_option = 1;
            fips_opts.self_test_onload = 1;
            break;
        case OPT_SELF_TEST_ONINSTALL:
            if (!check_non_pedantic_fips(pedantic, "self_test_oninstall"))
                goto end;
            set_selftest_onload_option = 1;
            fips_opts.self_test_onload = 0;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;
    if (verify && in_fname == NULL) {
        BIO_printf(bio_err, "Missing -in option for -verify\n");
        goto opthelp;
    }

    if (parent_config != NULL) {
        /* Test that a parent config can load the module */
        if (verify_module_load(parent_config)) {
            ret = OSSL_PROVIDER_available(NULL, prov_name) ? 0 : 1;
            if (!quiet) {
                BIO_printf(bio_err, "FIPS provider is %s\n",
                           ret == 0 ? "available" : "not available");
            }
        }
        goto end;
    }
    if (module_fname == NULL)
        goto opthelp;

    tail = opt_path_end(module_fname);
    if (tail != NULL) {
        module_path = OPENSSL_strdup(module_fname);
        if (module_path == NULL)
            goto end;
        module_path[tail - module_fname] = '\0';
        if (!OSSL_PROVIDER_set_default_search_path(NULL, module_path))
            goto end;
    }

    if (self_test_log
            || self_test_corrupt_desc != NULL
            || self_test_corrupt_type != NULL)
        OSSL_SELF_TEST_set_callback(NULL, self_test_events, NULL);

    /* Use the default FIPS HMAC digest and key if not specified. */
    if (!gotdigest && !sk_OPENSSL_STRING_push(opts, "digest:SHA256"))
        goto end;
    if (!gotkey && !sk_OPENSSL_STRING_push(opts, "hexkey:" FIPS_KEY_STRING))
        goto end;

    module_bio = bio_open_default(module_fname, 'r', FORMAT_BINARY);
    if (module_bio == NULL) {
        BIO_printf(bio_err, "Failed to open module file\n");
        goto end;
    }

    read_buffer = app_malloc(BUFSIZE, "I/O buffer");
    if (read_buffer == NULL)
        goto end;

    mac = EVP_MAC_fetch(app_get0_libctx(), mac_name, app_get0_propq());
    if (mac == NULL) {
        BIO_printf(bio_err, "Unable to get MAC of type %s\n", mac_name);
        goto end;
    }

    ctx = EVP_MAC_CTX_new(mac);
    if (ctx == NULL) {
        BIO_printf(bio_err, "Unable to create MAC CTX for module check\n");
        goto end;
    }

    if (opts != NULL) {
        int ok = 1;
        OSSL_PARAM *params =
            app_params_new_from_opts(opts, EVP_MAC_settable_ctx_params(mac));

        if (params == NULL)
            goto end;

        if (!EVP_MAC_CTX_set_params(ctx, params)) {
            BIO_printf(bio_err, "MAC parameter error\n");
            ERR_print_errors(bio_err);
            ok = 0;
        }
        app_params_free(params);
        if (!ok)
            goto end;
    }

    ctx2 = EVP_MAC_CTX_dup(ctx);
    if (ctx2 == NULL) {
        BIO_printf(bio_err, "Unable to create MAC CTX for install indicator\n");
        goto end;
    }

    if (!do_mac(ctx, read_buffer, module_bio, module_mac, &module_mac_len))
        goto end;

    /* Calculate the MAC for the indicator status - it may not be used */
    mem_bio = BIO_new_mem_buf((const void *)INSTALL_STATUS_VAL,
                              strlen(INSTALL_STATUS_VAL));
    if (mem_bio == NULL) {
        BIO_printf(bio_err, "Unable to create memory BIO\n");
        goto end;
    }
    if (!do_mac(ctx2, read_buffer, mem_bio, install_mac, &install_mac_len))
        goto end;

    if (verify) {
        if (fips_opts.self_test_onload == 1)
            install_mac_len = 0;
        if (!verify_config(in_fname, section_name, module_mac, module_mac_len,
                           install_mac, install_mac_len))
            goto end;
        if (!quiet)
            BIO_printf(bio_err, "VERIFY PASSED\n");
    } else {
        conf = generate_config_and_load(prov_name, section_name, module_mac,
                                        module_mac_len, &fips_opts);
        if (conf == NULL)
            goto end;
        if (!load_fips_prov_and_run_self_test(prov_name, &is_fips_140_2_prov))
            goto end;

        /*
         * In OpenSSL 3.1 the code was changed so that the status indicator is
         * not written out by default since this is a FIPS 140-3 requirement.
         * For backwards compatibility - if the detected FIPS provider is 3.0.X
         * (Which was a FIPS 140-2 validation), then the indicator status will
         * be written to the config file unless 'self_test_onload' is set on the
         * command line.
         */
        if (set_selftest_onload_option == 0 && is_fips_140_2_prov)
            fips_opts.self_test_onload = 0;

        fout =
            out_fname == NULL ? dup_bio_out(FORMAT_TEXT)
                              : bio_open_default(out_fname, 'w', FORMAT_TEXT);
        if (fout == NULL) {
            BIO_printf(bio_err, "Failed to open file\n");
            goto end;
        }

        if (!write_config_fips_section(fout, section_name,
                                       module_mac, module_mac_len, &fips_opts,
                                       install_mac, install_mac_len))
            goto end;
        if (!quiet)
            BIO_printf(bio_err, "INSTALL PASSED\n");
    }

    ret = 0;
end:
    if (ret == 1) {
        if (!quiet)
            BIO_printf(bio_err, "%s FAILED\n", verify ? "VERIFY" : "INSTALL");
        ERR_print_errors(bio_err);
    }

cleanup:
    OPENSSL_free(module_path);
    BIO_free(fout);
    BIO_free(mem_bio);
    BIO_free(module_bio);
    sk_OPENSSL_STRING_free(opts);
    EVP_MAC_free(mac);
    EVP_MAC_CTX_free(ctx2);
    EVP_MAC_CTX_free(ctx);
    OPENSSL_free(read_buffer);
    free_config_and_unload(conf);
    return ret;
}

static int self_test_events(const OSSL_PARAM params[], void *arg)
{
    const OSSL_PARAM *p = NULL;
    const char *phase = NULL, *type = NULL, *desc = NULL;
    int ret = 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_PHASE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    phase = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_DESC);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    desc = (const char *)p->data;

    p = OSSL_PARAM_locate_const(params, OSSL_PROV_PARAM_SELF_TEST_TYPE);
    if (p == NULL || p->data_type != OSSL_PARAM_UTF8_STRING)
        goto err;
    type = (const char *)p->data;

    if (self_test_log) {
        if (strcmp(phase, OSSL_SELF_TEST_PHASE_START) == 0)
            BIO_printf(bio_err, "%s : (%s) : ", desc, type);
        else if (strcmp(phase, OSSL_SELF_TEST_PHASE_PASS) == 0
                 || strcmp(phase, OSSL_SELF_TEST_PHASE_FAIL) == 0)
            BIO_printf(bio_err, "%s\n", phase);
    }
    /*
     * The self test code will internally corrupt the KAT test result if an
     * error is returned during the corrupt phase.
     */
    if (strcmp(phase, OSSL_SELF_TEST_PHASE_CORRUPT) == 0
            && (self_test_corrupt_desc != NULL
                || self_test_corrupt_type != NULL)) {
        if (self_test_corrupt_desc != NULL
                && strcmp(self_test_corrupt_desc, desc) != 0)
            goto end;
        if (self_test_corrupt_type != NULL
                && strcmp(self_test_corrupt_type, type) != 0)
            goto end;
        BIO_printf(bio_err, "%s ", phase);
        goto err;
    }
end:
    ret = 1;
err:
    return ret;
}
