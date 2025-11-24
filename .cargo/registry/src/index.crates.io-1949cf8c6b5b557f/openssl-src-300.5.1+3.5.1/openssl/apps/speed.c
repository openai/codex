/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#undef SECONDS
#define SECONDS          3
#define PKEY_SECONDS    10

#define RSA_SECONDS     PKEY_SECONDS
#define DSA_SECONDS     PKEY_SECONDS
#define ECDSA_SECONDS   PKEY_SECONDS
#define ECDH_SECONDS    PKEY_SECONDS
#define EdDSA_SECONDS   PKEY_SECONDS
#define SM2_SECONDS     PKEY_SECONDS
#define FFDH_SECONDS    PKEY_SECONDS
#define KEM_SECONDS     PKEY_SECONDS
#define SIG_SECONDS     PKEY_SECONDS

#define MAX_ALGNAME_SUFFIX 100

/* We need to use some deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED
#include "internal/e_os.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include "apps.h"
#include "progs.h"
#include "internal/nelem.h"
#include "internal/numbers.h"
#include <openssl/crypto.h>
#include <openssl/rand.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/core_names.h>
#include <openssl/async.h>
#include <openssl/provider.h>
#if !defined(OPENSSL_SYS_MSDOS)
# include <unistd.h>
#endif

#if defined(_WIN32)
# include <windows.h>
/*
 * While VirtualLock is available under the app partition (e.g. UWP),
 * the headers do not define the API. Define it ourselves instead.
 */
WINBASEAPI
BOOL
WINAPI
VirtualLock(
    _In_ LPVOID lpAddress,
    _In_ SIZE_T dwSize
    );
#endif

#if defined(OPENSSL_SYS_LINUX)
# include <sys/mman.h>
#endif

#include <openssl/bn.h>
#include <openssl/rsa.h>
#include "./testrsa.h"
#ifndef OPENSSL_NO_DH
# include <openssl/dh.h>
#endif
#include <openssl/x509.h>
#include <openssl/dsa.h>
#include "./testdsa.h"
#include <openssl/modes.h>

#ifndef HAVE_FORK
# if defined(OPENSSL_SYS_VMS) || defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_VXWORKS)
#  define HAVE_FORK 0
# else
#  define HAVE_FORK 1
#  include <sys/wait.h>
# endif
#endif

#if HAVE_FORK
# undef NO_FORK
#else
# define NO_FORK
#endif

#define MAX_MISALIGNMENT 63
#define MAX_ECDH_SIZE   256
#define MISALIGN        64
#define MAX_FFDH_SIZE 1024

#ifndef RSA_DEFAULT_PRIME_NUM
# define RSA_DEFAULT_PRIME_NUM 2
#endif

typedef struct openssl_speed_sec_st {
    int sym;
    int rsa;
    int dsa;
    int ecdsa;
    int ecdh;
    int eddsa;
    int sm2;
    int ffdh;
    int kem;
    int sig;
} openssl_speed_sec_t;

static volatile int run = 0;

static int mr = 0;  /* machine-readeable output format to merge fork results */
static int usertime = 1;

static double Time_F(int s);
static void print_message(const char *s, int length, int tm);
static void pkey_print_message(const char *str, const char *str2,
                               unsigned int bits, int sec);
static void kskey_print_message(const char *str, const char *str2, int tm);
static void print_result(int alg, int run_no, int count, double time_used);
#ifndef NO_FORK
static int do_multi(int multi, int size_num);
#endif

static int domlock = 0;
static int testmode = 0;
static int testmoderesult = 0;

static const int lengths_list[] = {
    16, 64, 256, 1024, 8 * 1024, 16 * 1024
};
#define SIZE_NUM         OSSL_NELEM(lengths_list)
static const int *lengths = lengths_list;

static const int aead_lengths_list[] = {
    2, 31, 136, 1024, 8 * 1024, 16 * 1024
};

#define START   0
#define STOP    1

#ifdef SIGALRM

static void alarmed(ossl_unused int sig)
{
    signal(SIGALRM, alarmed);
    run = 0;
}

static double Time_F(int s)
{
    double ret = app_tminterval(s, usertime);
    if (s == STOP)
        alarm(0);
    return ret;
}

#elif defined(_WIN32)

# define SIGALRM -1

static unsigned int lapse;
static volatile unsigned int schlock;
static void alarm_win32(unsigned int secs)
{
    lapse = secs * 1000;
}

# define alarm alarm_win32

static DWORD WINAPI sleepy(VOID * arg)
{
    schlock = 1;
    Sleep(lapse);
    run = 0;
    return 0;
}

static double Time_F(int s)
{
    double ret;
    static HANDLE thr;

    if (s == START) {
        schlock = 0;
        thr = CreateThread(NULL, 4096, sleepy, NULL, 0, NULL);
        if (thr == NULL) {
            DWORD err = GetLastError();
            BIO_printf(bio_err, "unable to CreateThread (%lu)", err);
            ExitProcess(err);
        }
        while (!schlock)
            Sleep(0);           /* scheduler spinlock */
        ret = app_tminterval(s, usertime);
    } else {
        ret = app_tminterval(s, usertime);
        if (run)
            TerminateThread(thr, 0);
        CloseHandle(thr);
    }

    return ret;
}
#else
# error "SIGALRM not defined and the platform is not Windows"
#endif

static void multiblock_speed(const EVP_CIPHER *evp_cipher, int lengths_single,
                             const openssl_speed_sec_t *seconds);

static int opt_found(const char *name, unsigned int *result,
                     const OPT_PAIR pairs[], unsigned int nbelem)
{
    unsigned int idx;

    for (idx = 0; idx < nbelem; ++idx, pairs++)
        if (strcmp(name, pairs->name) == 0) {
            *result = pairs->retval;
            return 1;
        }
    return 0;
}
#define opt_found(value, pairs, result)\
    opt_found(value, result, pairs, OSSL_NELEM(pairs))

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_ELAPSED, OPT_EVP, OPT_HMAC, OPT_DECRYPT, OPT_ENGINE, OPT_MULTI,
    OPT_MR, OPT_MB, OPT_MISALIGN, OPT_ASYNCJOBS, OPT_R_ENUM, OPT_PROV_ENUM,
    OPT_CONFIG, OPT_PRIMES, OPT_SECONDS, OPT_BYTES, OPT_AEAD, OPT_CMAC,
    OPT_MLOCK, OPT_TESTMODE, OPT_KEM, OPT_SIG
} OPTION_CHOICE;

const OPTIONS speed_options[] = {
    {OPT_HELP_STR, 1, '-',
     "Usage: %s [options] [algorithm...]\n"
     "All +int options consider prefix '0' as base-8 input, "
     "prefix '0x'/'0X' as base-16 input.\n"
    },

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},
    {"mb", OPT_MB, '-',
     "Enable (tls1>=1) multi-block mode on EVP-named cipher"},
    {"mr", OPT_MR, '-', "Produce machine readable output"},
#ifndef NO_FORK
    {"multi", OPT_MULTI, 'p', "Run benchmarks in parallel"},
#endif
#ifndef OPENSSL_NO_ASYNC
    {"async_jobs", OPT_ASYNCJOBS, 'p',
     "Enable async mode and start specified number of jobs"},
#endif
#ifndef OPENSSL_NO_ENGINE
    {"engine", OPT_ENGINE, 's', "Use engine, possibly a hardware device"},
#endif
    {"primes", OPT_PRIMES, 'p', "Specify number of primes (for RSA only)"},
    {"mlock", OPT_MLOCK, '-', "Lock memory for better result determinism"},
    {"testmode", OPT_TESTMODE, '-', "Run the speed command in test mode"},
    OPT_CONFIG_OPTION,

    OPT_SECTION("Selection"),
    {"evp", OPT_EVP, 's', "Use EVP-named cipher or digest"},
    {"hmac", OPT_HMAC, 's', "HMAC using EVP-named digest"},
    {"cmac", OPT_CMAC, 's', "CMAC using EVP-named cipher"},
    {"decrypt", OPT_DECRYPT, '-',
     "Time decryption instead of encryption (only EVP)"},
    {"aead", OPT_AEAD, '-',
     "Benchmark EVP-named AEAD cipher in TLS-like sequence"},
    {"kem-algorithms", OPT_KEM, '-',
     "Benchmark KEM algorithms"},
    {"signature-algorithms", OPT_SIG, '-',
     "Benchmark signature algorithms"},

    OPT_SECTION("Timing"),
    {"elapsed", OPT_ELAPSED, '-',
     "Use wall-clock time instead of CPU user time as divisor"},
    {"seconds", OPT_SECONDS, 'p',
     "Run benchmarks for specified amount of seconds"},
    {"bytes", OPT_BYTES, 'p',
     "Run [non-PKI] benchmarks on custom-sized buffer"},
    {"misalign", OPT_MISALIGN, 'p',
     "Use specified offset to mis-align buffers"},

    OPT_R_OPTIONS,
    OPT_PROV_OPTIONS,

    OPT_PARAMETERS(),
    {"algorithm", 0, 0, "Algorithm(s) to test (optional; otherwise tests all)"},
    {NULL}
};

enum {
    D_MD2, D_MDC2, D_MD4, D_MD5, D_SHA1, D_RMD160,
    D_SHA256, D_SHA512, D_WHIRLPOOL, D_HMAC,
    D_CBC_DES, D_EDE3_DES, D_RC4, D_CBC_IDEA, D_CBC_SEED,
    D_CBC_RC2, D_CBC_RC5, D_CBC_BF, D_CBC_CAST,
    D_CBC_128_AES, D_CBC_192_AES, D_CBC_256_AES,
    D_CBC_128_CML, D_CBC_192_CML, D_CBC_256_CML,
    D_EVP, D_GHASH, D_RAND, D_EVP_CMAC, D_KMAC128, D_KMAC256,
    ALGOR_NUM
};
/* name of algorithms to test. MUST BE KEEP IN SYNC with above enum ! */
static const char *names[ALGOR_NUM] = {
    "md2", "mdc2", "md4", "md5", "sha1", "rmd160",
    "sha256", "sha512", "whirlpool", "hmac(sha256)",
    "des-cbc", "des-ede3", "rc4", "idea-cbc", "seed-cbc",
    "rc2-cbc", "rc5-cbc", "blowfish", "cast-cbc",
    "aes-128-cbc", "aes-192-cbc", "aes-256-cbc",
    "camellia-128-cbc", "camellia-192-cbc", "camellia-256-cbc",
    "evp", "ghash", "rand", "cmac", "kmac128", "kmac256"
};

/* list of configured algorithm (remaining), with some few alias */
static const OPT_PAIR doit_choices[] = {
    {"md2", D_MD2},
    {"mdc2", D_MDC2},
    {"md4", D_MD4},
    {"md5", D_MD5},
    {"hmac", D_HMAC},
    {"sha1", D_SHA1},
    {"sha256", D_SHA256},
    {"sha512", D_SHA512},
    {"whirlpool", D_WHIRLPOOL},
    {"ripemd", D_RMD160},
    {"rmd160", D_RMD160},
    {"ripemd160", D_RMD160},
    {"rc4", D_RC4},
    {"des-cbc", D_CBC_DES},
    {"des-ede3", D_EDE3_DES},
    {"aes-128-cbc", D_CBC_128_AES},
    {"aes-192-cbc", D_CBC_192_AES},
    {"aes-256-cbc", D_CBC_256_AES},
    {"camellia-128-cbc", D_CBC_128_CML},
    {"camellia-192-cbc", D_CBC_192_CML},
    {"camellia-256-cbc", D_CBC_256_CML},
    {"rc2-cbc", D_CBC_RC2},
    {"rc2", D_CBC_RC2},
    {"rc5-cbc", D_CBC_RC5},
    {"rc5", D_CBC_RC5},
    {"idea-cbc", D_CBC_IDEA},
    {"idea", D_CBC_IDEA},
    {"seed-cbc", D_CBC_SEED},
    {"seed", D_CBC_SEED},
    {"bf-cbc", D_CBC_BF},
    {"blowfish", D_CBC_BF},
    {"bf", D_CBC_BF},
    {"cast-cbc", D_CBC_CAST},
    {"cast", D_CBC_CAST},
    {"cast5", D_CBC_CAST},
    {"ghash", D_GHASH},
    {"rand", D_RAND},
    {"kmac128", D_KMAC128},
    {"kmac256", D_KMAC256},
};

static double results[ALGOR_NUM][SIZE_NUM];

#ifndef OPENSSL_NO_DSA
enum { R_DSA_1024, R_DSA_2048, DSA_NUM };
static const OPT_PAIR dsa_choices[DSA_NUM] = {
    {"dsa1024", R_DSA_1024},
    {"dsa2048", R_DSA_2048}
};
static double dsa_results[DSA_NUM][2];  /* 2 ops: sign then verify */
#endif /* OPENSSL_NO_DSA */

enum {
    R_RSA_512, R_RSA_1024, R_RSA_2048, R_RSA_3072, R_RSA_4096, R_RSA_7680,
    R_RSA_15360, RSA_NUM
};
static const OPT_PAIR rsa_choices[RSA_NUM] = {
    {"rsa512", R_RSA_512},
    {"rsa1024", R_RSA_1024},
    {"rsa2048", R_RSA_2048},
    {"rsa3072", R_RSA_3072},
    {"rsa4096", R_RSA_4096},
    {"rsa7680", R_RSA_7680},
    {"rsa15360", R_RSA_15360}
};

static double rsa_results[RSA_NUM][4];  /* 4 ops: sign, verify, encrypt, decrypt */

#ifndef OPENSSL_NO_DH
enum ff_params_t {
    R_FFDH_2048, R_FFDH_3072, R_FFDH_4096, R_FFDH_6144, R_FFDH_8192, FFDH_NUM
};

static const OPT_PAIR ffdh_choices[FFDH_NUM] = {
    {"ffdh2048", R_FFDH_2048},
    {"ffdh3072", R_FFDH_3072},
    {"ffdh4096", R_FFDH_4096},
    {"ffdh6144", R_FFDH_6144},
    {"ffdh8192", R_FFDH_8192},
};

static double ffdh_results[FFDH_NUM][1];  /* 1 op: derivation */
#endif /* OPENSSL_NO_DH */

enum ec_curves_t {
    R_EC_P160, R_EC_P192, R_EC_P224, R_EC_P256, R_EC_P384, R_EC_P521,
#ifndef OPENSSL_NO_EC2M
    R_EC_K163, R_EC_K233, R_EC_K283, R_EC_K409, R_EC_K571,
    R_EC_B163, R_EC_B233, R_EC_B283, R_EC_B409, R_EC_B571,
#endif
    R_EC_BRP256R1, R_EC_BRP256T1, R_EC_BRP384R1, R_EC_BRP384T1,
    R_EC_BRP512R1, R_EC_BRP512T1, ECDSA_NUM
};
/* list of ecdsa curves */
static const OPT_PAIR ecdsa_choices[ECDSA_NUM] = {
    {"ecdsap160", R_EC_P160},
    {"ecdsap192", R_EC_P192},
    {"ecdsap224", R_EC_P224},
    {"ecdsap256", R_EC_P256},
    {"ecdsap384", R_EC_P384},
    {"ecdsap521", R_EC_P521},
#ifndef OPENSSL_NO_EC2M
    {"ecdsak163", R_EC_K163},
    {"ecdsak233", R_EC_K233},
    {"ecdsak283", R_EC_K283},
    {"ecdsak409", R_EC_K409},
    {"ecdsak571", R_EC_K571},
    {"ecdsab163", R_EC_B163},
    {"ecdsab233", R_EC_B233},
    {"ecdsab283", R_EC_B283},
    {"ecdsab409", R_EC_B409},
    {"ecdsab571", R_EC_B571},
#endif
    {"ecdsabrp256r1", R_EC_BRP256R1},
    {"ecdsabrp256t1", R_EC_BRP256T1},
    {"ecdsabrp384r1", R_EC_BRP384R1},
    {"ecdsabrp384t1", R_EC_BRP384T1},
    {"ecdsabrp512r1", R_EC_BRP512R1},
    {"ecdsabrp512t1", R_EC_BRP512T1}
};
enum {
#ifndef OPENSSL_NO_ECX
    R_EC_X25519 = ECDSA_NUM, R_EC_X448, EC_NUM
#else
    EC_NUM = ECDSA_NUM
#endif
};
/* list of ecdh curves, extension of |ecdsa_choices| list above */
static const OPT_PAIR ecdh_choices[EC_NUM] = {
    {"ecdhp160", R_EC_P160},
    {"ecdhp192", R_EC_P192},
    {"ecdhp224", R_EC_P224},
    {"ecdhp256", R_EC_P256},
    {"ecdhp384", R_EC_P384},
    {"ecdhp521", R_EC_P521},
#ifndef OPENSSL_NO_EC2M
    {"ecdhk163", R_EC_K163},
    {"ecdhk233", R_EC_K233},
    {"ecdhk283", R_EC_K283},
    {"ecdhk409", R_EC_K409},
    {"ecdhk571", R_EC_K571},
    {"ecdhb163", R_EC_B163},
    {"ecdhb233", R_EC_B233},
    {"ecdhb283", R_EC_B283},
    {"ecdhb409", R_EC_B409},
    {"ecdhb571", R_EC_B571},
#endif
    {"ecdhbrp256r1", R_EC_BRP256R1},
    {"ecdhbrp256t1", R_EC_BRP256T1},
    {"ecdhbrp384r1", R_EC_BRP384R1},
    {"ecdhbrp384t1", R_EC_BRP384T1},
    {"ecdhbrp512r1", R_EC_BRP512R1},
    {"ecdhbrp512t1", R_EC_BRP512T1},
#ifndef OPENSSL_NO_ECX
    {"ecdhx25519", R_EC_X25519},
    {"ecdhx448", R_EC_X448}
#endif
};

static double ecdh_results[EC_NUM][1];      /* 1 op: derivation */
static double ecdsa_results[ECDSA_NUM][2];  /* 2 ops: sign then verify */

#ifndef OPENSSL_NO_ECX
enum { R_EC_Ed25519, R_EC_Ed448, EdDSA_NUM };
static const OPT_PAIR eddsa_choices[EdDSA_NUM] = {
    {"ed25519", R_EC_Ed25519},
    {"ed448", R_EC_Ed448}

};
static double eddsa_results[EdDSA_NUM][2];    /* 2 ops: sign then verify */
#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_SM2
enum { R_EC_CURVESM2, SM2_NUM };
static const OPT_PAIR sm2_choices[SM2_NUM] = {
    {"curveSM2", R_EC_CURVESM2}
};
# define SM2_ID        "TLSv1.3+GM+Cipher+Suite"
# define SM2_ID_LEN    sizeof("TLSv1.3+GM+Cipher+Suite") - 1
static double sm2_results[SM2_NUM][2];    /* 2 ops: sign then verify */
#endif /* OPENSSL_NO_SM2 */

#define MAX_KEM_NUM 111
static size_t kems_algs_len = 0;
static char *kems_algname[MAX_KEM_NUM] = { NULL };
static double kems_results[MAX_KEM_NUM][3];  /* keygen, encaps, decaps */

#define MAX_SIG_NUM 256
static size_t sigs_algs_len = 0;
static char *sigs_algname[MAX_SIG_NUM] = { NULL };
static double sigs_results[MAX_SIG_NUM][3];  /* keygen, sign, verify */

#define COND(unused_cond) (run && count < (testmode ? 1 : INT_MAX))
#define COUNT(d) (count)

#define TAG_LEN 16 /* 16 bytes tag length works for all AEAD modes */
#define AEAD_IVLEN 12 /* 12 bytes iv length works for all AEAD modes */

static unsigned int mode_op; /* AE Mode of operation */
static unsigned int aead = 0; /* AEAD flag */
static unsigned char aead_iv[AEAD_IVLEN]; /* For AEAD modes */
static unsigned char aad[EVP_AEAD_TLS1_AAD_LEN] = { 0xcc };

typedef struct loopargs_st {
    ASYNC_JOB *inprogress_job;
    ASYNC_WAIT_CTX *wait_ctx;
    unsigned char *buf;
    unsigned char *buf2;
    unsigned char *buf_malloc;
    unsigned char *buf2_malloc;
    unsigned char *key;
    unsigned char tag[TAG_LEN];
    size_t buflen;
    size_t sigsize;
    size_t encsize;
    EVP_PKEY_CTX *rsa_sign_ctx[RSA_NUM];
    EVP_PKEY_CTX *rsa_verify_ctx[RSA_NUM];
    EVP_PKEY_CTX *rsa_encrypt_ctx[RSA_NUM];
    EVP_PKEY_CTX *rsa_decrypt_ctx[RSA_NUM];
#ifndef OPENSSL_NO_DSA
    EVP_PKEY_CTX *dsa_sign_ctx[DSA_NUM];
    EVP_PKEY_CTX *dsa_verify_ctx[DSA_NUM];
#endif
    EVP_PKEY_CTX *ecdsa_sign_ctx[ECDSA_NUM];
    EVP_PKEY_CTX *ecdsa_verify_ctx[ECDSA_NUM];
    EVP_PKEY_CTX *ecdh_ctx[EC_NUM];
#ifndef OPENSSL_NO_ECX
    EVP_MD_CTX *eddsa_ctx[EdDSA_NUM];
    EVP_MD_CTX *eddsa_ctx2[EdDSA_NUM];
#endif /* OPENSSL_NO_ECX */
#ifndef OPENSSL_NO_SM2
    EVP_MD_CTX *sm2_ctx[SM2_NUM];
    EVP_MD_CTX *sm2_vfy_ctx[SM2_NUM];
    EVP_PKEY *sm2_pkey[SM2_NUM];
#endif
    unsigned char *secret_a;
    unsigned char *secret_b;
    size_t outlen[EC_NUM];
#ifndef OPENSSL_NO_DH
    EVP_PKEY_CTX *ffdh_ctx[FFDH_NUM];
    unsigned char *secret_ff_a;
    unsigned char *secret_ff_b;
#endif
    EVP_CIPHER_CTX *ctx;
    EVP_MAC_CTX *mctx;
    EVP_PKEY_CTX *kem_gen_ctx[MAX_KEM_NUM];
    EVP_PKEY_CTX *kem_encaps_ctx[MAX_KEM_NUM];
    EVP_PKEY_CTX *kem_decaps_ctx[MAX_KEM_NUM];
    size_t kem_out_len[MAX_KEM_NUM];
    size_t kem_secret_len[MAX_KEM_NUM];
    unsigned char *kem_out[MAX_KEM_NUM];
    unsigned char *kem_send_secret[MAX_KEM_NUM];
    unsigned char *kem_rcv_secret[MAX_KEM_NUM];
    EVP_PKEY_CTX *sig_gen_ctx[MAX_SIG_NUM];
    EVP_PKEY_CTX *sig_sign_ctx[MAX_SIG_NUM];
    EVP_PKEY_CTX *sig_verify_ctx[MAX_SIG_NUM];
    size_t sig_max_sig_len[MAX_SIG_NUM];
    size_t sig_act_sig_len[MAX_SIG_NUM];
    unsigned char *sig_sig[MAX_SIG_NUM];
} loopargs_t;
static int run_benchmark(int async_jobs, int (*loop_function) (void *),
                         loopargs_t *loopargs);

static unsigned int testnum;

static char *evp_mac_mdname = "sha256";
static char *evp_hmac_name = NULL;
static const char *evp_md_name = NULL;
static char *evp_mac_ciphername = "aes-128-cbc";
static char *evp_cmac_name = NULL;

static void dofail(void)
{
    ERR_print_errors(bio_err);
    testmoderesult = 1;
}

static int have_md(const char *name)
{
    int ret = 0;
    EVP_MD *md = NULL;

    if (opt_md_silent(name, &md)) {
        EVP_MD_CTX *ctx = EVP_MD_CTX_new();

        if (ctx != NULL && EVP_DigestInit(ctx, md) > 0)
            ret = 1;
        EVP_MD_CTX_free(ctx);
        EVP_MD_free(md);
    }
    return ret;
}

static int have_cipher(const char *name)
{
    int ret = 0;
    EVP_CIPHER *cipher = NULL;

    if (opt_cipher_silent(name, &cipher)) {
        EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();

        if (ctx != NULL
            && EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, 1) > 0)
            ret = 1;
        EVP_CIPHER_CTX_free(ctx);
        EVP_CIPHER_free(cipher);
    }
    return ret;
}

static int EVP_Digest_loop(const char *mdname, ossl_unused int algindex, void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char digest[EVP_MAX_MD_SIZE];
    int count;
    EVP_MD *md = NULL;
    EVP_MD_CTX *ctx = NULL;

    if (!opt_md_silent(mdname, &md))
        return -1;
    if (EVP_MD_xof(md)) {
        ctx = EVP_MD_CTX_new();
        if (ctx == NULL) {
            count = -1;
            goto out;
        }

        for (count = 0; COND(c[algindex][testnum]); count++) {
             if (!EVP_DigestInit_ex2(ctx, md, NULL)
                 || !EVP_DigestUpdate(ctx, buf, (size_t)lengths[testnum])
                 || !EVP_DigestFinalXOF(ctx, digest, sizeof(digest))) {
                count = -1;
                break;
            }
        }
    } else {
        for (count = 0; COND(c[algindex][testnum]); count++) {
            if (!EVP_Digest(buf, (size_t)lengths[testnum], digest, NULL, md,
                            NULL)) {
                count = -1;
                break;
            }
        }
    }
out:
    EVP_MD_free(md);
    EVP_MD_CTX_free(ctx);
    return count;
}

static int EVP_Digest_md_loop(void *args)
{
    return EVP_Digest_loop(evp_md_name, D_EVP, args);
}

static int EVP_Digest_MD2_loop(void *args)
{
    return EVP_Digest_loop("md2", D_MD2, args);
}

static int EVP_Digest_MDC2_loop(void *args)
{
    return EVP_Digest_loop("mdc2", D_MDC2, args);
}

static int EVP_Digest_MD4_loop(void *args)
{
    return EVP_Digest_loop("md4", D_MD4, args);
}

static int MD5_loop(void *args)
{
    return EVP_Digest_loop("md5", D_MD5, args);
}

static int mac_setup(const char *name,
                     EVP_MAC **mac, OSSL_PARAM params[],
                     loopargs_t *loopargs, unsigned int loopargs_len)
{
    unsigned int i;

    *mac = EVP_MAC_fetch(app_get0_libctx(), name, app_get0_propq());
    if (*mac == NULL)
        return 0;

    for (i = 0; i < loopargs_len; i++) {
        loopargs[i].mctx = EVP_MAC_CTX_new(*mac);
        if (loopargs[i].mctx == NULL)
            return 0;

        if (!EVP_MAC_CTX_set_params(loopargs[i].mctx, params))
            return 0;
    }

    return 1;
}

static void mac_teardown(EVP_MAC **mac,
                         loopargs_t *loopargs, unsigned int loopargs_len)
{
    unsigned int i;

    for (i = 0; i < loopargs_len; i++)
        EVP_MAC_CTX_free(loopargs[i].mctx);
    EVP_MAC_free(*mac);
    *mac = NULL;

    return;
}

static int EVP_MAC_loop(ossl_unused int algindex, void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MAC_CTX *mctx = tempargs->mctx;
    unsigned char mac[EVP_MAX_MD_SIZE];
    int count;

    for (count = 0; COND(c[algindex][testnum]); count++) {
        size_t outl;

        if (!EVP_MAC_init(mctx, NULL, 0, NULL)
            || !EVP_MAC_update(mctx, buf, lengths[testnum])
            || !EVP_MAC_final(mctx, mac, &outl, sizeof(mac)))
            return -1;
    }
    return count;
}

static int HMAC_loop(void *args)
{
    return EVP_MAC_loop(D_HMAC, args);
}

static int CMAC_loop(void *args)
{
    return EVP_MAC_loop(D_EVP_CMAC, args);
}

static int KMAC128_loop(void *args)
{
    return EVP_MAC_loop(D_KMAC128, args);
}

static int KMAC256_loop(void *args)
{
    return EVP_MAC_loop(D_KMAC256, args);
}

static int SHA1_loop(void *args)
{
    return EVP_Digest_loop("sha1", D_SHA1, args);
}

static int SHA256_loop(void *args)
{
    return EVP_Digest_loop("sha256", D_SHA256, args);
}

static int SHA512_loop(void *args)
{
    return EVP_Digest_loop("sha512", D_SHA512, args);
}

static int WHIRLPOOL_loop(void *args)
{
    return EVP_Digest_loop("whirlpool", D_WHIRLPOOL, args);
}

static int EVP_Digest_RMD160_loop(void *args)
{
    return EVP_Digest_loop("ripemd160", D_RMD160, args);
}

static int algindex;

static int EVP_Cipher_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    int count;

    if (tempargs->ctx == NULL)
        return -1;
    for (count = 0; COND(c[algindex][testnum]); count++)
        if (EVP_Cipher(tempargs->ctx, buf, buf, (size_t)lengths[testnum]) <= 0)
            return -1;
    return count;
}

static int GHASH_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MAC_CTX *mctx = tempargs->mctx;
    int count;

    /* just do the update in the loop to be comparable with 1.1.1 */
    for (count = 0; COND(c[D_GHASH][testnum]); count++) {
        if (!EVP_MAC_update(mctx, buf, lengths[testnum]))
            return -1;
    }
    return count;
}

#define MAX_BLOCK_SIZE 128

static unsigned char iv[2 * MAX_BLOCK_SIZE / 8];

static EVP_CIPHER_CTX *init_evp_cipher_ctx(const char *ciphername,
                                           const unsigned char *key,
                                           int keylen)
{
    EVP_CIPHER_CTX *ctx = NULL;
    EVP_CIPHER *cipher = NULL;

    if (!opt_cipher_silent(ciphername, &cipher))
        return NULL;

    if ((ctx = EVP_CIPHER_CTX_new()) == NULL)
        goto end;

    if (!EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, 1)) {
        EVP_CIPHER_CTX_free(ctx);
        ctx = NULL;
        goto end;
    }

    if (EVP_CIPHER_CTX_set_key_length(ctx, keylen) <= 0) {
        EVP_CIPHER_CTX_free(ctx);
        ctx = NULL;
        goto end;
    }

    if (!EVP_CipherInit_ex(ctx, NULL, NULL, key, iv, 1)) {
        EVP_CIPHER_CTX_free(ctx);
        ctx = NULL;
        goto end;
    }

end:
    EVP_CIPHER_free(cipher);
    return ctx;
}

static int RAND_bytes_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    int count;

    for (count = 0; COND(c[D_RAND][testnum]); count++)
        RAND_bytes(buf, lengths[testnum]);
    return count;
}

static int decrypt = 0;
static int EVP_Update_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_CIPHER_CTX *ctx = tempargs->ctx;
    int outl, count, rc;

    if (decrypt) {
        for (count = 0; COND(c[D_EVP][testnum]); count++) {
            rc = EVP_DecryptUpdate(ctx, buf, &outl, buf, lengths[testnum]);
            if (rc != 1) {
                /* reset iv in case of counter overflow */
                rc = EVP_CipherInit_ex(ctx, NULL, NULL, NULL, iv, -1);
            }
        }
    } else {
        for (count = 0; COND(c[D_EVP][testnum]); count++) {
            rc = EVP_EncryptUpdate(ctx, buf, &outl, buf, lengths[testnum]);
            if (rc != 1) {
                /* reset iv in case of counter overflow */
                rc = EVP_CipherInit_ex(ctx, NULL, NULL, NULL, iv, -1);
            }
        }
    }
    if (decrypt)
        rc = EVP_DecryptFinal_ex(ctx, buf, &outl);
    else
        rc = EVP_EncryptFinal_ex(ctx, buf, &outl);

    if (rc == 0)
        BIO_printf(bio_err, "Error finalizing cipher loop\n");
    return count;
}

/*
 * To make AEAD benchmarking more relevant perform TLS-like operations,
 * 13-byte AAD followed by payload. But don't use TLS-formatted AAD, as
 * payload length is not actually limited by 16KB...
 * CCM does not support streaming. For the purpose of performance measurement,
 * each message is encrypted using the same (key,iv)-pair. Do not use this
 * code in your application.
 */
static int EVP_Update_loop_aead_enc(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *key = tempargs->key;
    EVP_CIPHER_CTX *ctx = tempargs->ctx;
    int outl, count, realcount = 0;

    for (count = 0; COND(c[D_EVP][testnum]); count++) {
        /* Set length of iv (Doesn't apply to SIV mode) */
        if (mode_op != EVP_CIPH_SIV_MODE) {
            if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN,
                                     sizeof(aead_iv), NULL)) {
                BIO_printf(bio_err, "\nFailed to set iv length\n");
                dofail();
                exit(1);
            }
        }
        /* Set tag_len (Not for GCM/SIV at encryption stage) */
        if (mode_op != EVP_CIPH_GCM_MODE
            && mode_op != EVP_CIPH_SIV_MODE
            && mode_op != EVP_CIPH_GCM_SIV_MODE) {
            if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                     TAG_LEN, NULL)) {
                BIO_printf(bio_err, "\nFailed to set tag length\n");
                dofail();
                exit(1);
            }
        }
        if (!EVP_CipherInit_ex(ctx, NULL, NULL, key, aead_iv, -1)) {
            BIO_printf(bio_err, "\nFailed to set key and iv\n");
            dofail();
            exit(1);
        }
        /* Set total length of input. Only required for CCM */
        if (mode_op == EVP_CIPH_CCM_MODE) {
            if (!EVP_EncryptUpdate(ctx, NULL, &outl,
                                   NULL, lengths[testnum])) {
                BIO_printf(bio_err, "\nCouldn't set input text length\n");
                dofail();
                exit(1);
            }
        }
        if (aead) {
            if (!EVP_EncryptUpdate(ctx, NULL, &outl, aad, sizeof(aad))) {
                BIO_printf(bio_err, "\nCouldn't insert AAD when encrypting\n");
                dofail();
                exit(1);
            }
        }
        if (!EVP_EncryptUpdate(ctx, buf, &outl, buf, lengths[testnum])) {
            BIO_printf(bio_err, "\nFailed to encrypt the data\n");
            dofail();
            exit(1);
        }
        if (EVP_EncryptFinal_ex(ctx, buf, &outl))
            realcount++;
    }
    return realcount;
}

/*
 * To make AEAD benchmarking more relevant perform TLS-like operations,
 * 13-byte AAD followed by payload. But don't use TLS-formatted AAD, as
 * payload length is not actually limited by 16KB...
 * CCM does not support streaming. For the purpose of performance measurement,
 * each message is decrypted using the same (key,iv)-pair. Do not use this
 * code in your application.
 * For decryption, we will use buf2 to preserve the input text in buf.
 */
static int EVP_Update_loop_aead_dec(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *outbuf = tempargs->buf2;
    unsigned char *key = tempargs->key;
    unsigned char tag[TAG_LEN];
    EVP_CIPHER_CTX *ctx = tempargs->ctx;
    int outl, count, realcount = 0;

    for (count = 0; COND(c[D_EVP][testnum]); count++) {
        /* Set the length of iv (Doesn't apply to SIV mode) */
        if (mode_op != EVP_CIPH_SIV_MODE) {
            if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN,
                                     sizeof(aead_iv), NULL)) {
                BIO_printf(bio_err, "\nFailed to set iv length\n");
                dofail();
                exit(1);
            }
        }

        /* Set the tag length (Doesn't apply to SIV mode) */
        if (mode_op != EVP_CIPH_SIV_MODE
            && mode_op != EVP_CIPH_GCM_MODE
            && mode_op != EVP_CIPH_GCM_SIV_MODE) {
            if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                     TAG_LEN, NULL)) {
                BIO_printf(bio_err, "\nFailed to set tag length\n");
                dofail();
                exit(1);
            }
        }
        if (!EVP_CipherInit_ex(ctx, NULL, NULL, key, aead_iv, -1)) {
            BIO_printf(bio_err, "\nFailed to set key and iv\n");
            dofail();
            exit(1);
        }
        /* Set iv before decryption (Doesn't apply to SIV mode) */
        if (mode_op != EVP_CIPH_SIV_MODE) {
            if (!EVP_DecryptInit_ex(ctx, NULL, NULL, NULL, aead_iv)) {
                BIO_printf(bio_err, "\nFailed to set iv\n");
                dofail();
                exit(1);
            }
        }
        memcpy(tag, tempargs->tag, TAG_LEN);

        if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                 TAG_LEN, tag)) {
            BIO_printf(bio_err, "\nFailed to set tag\n");
            dofail();
            exit(1);
        }
        /* Set the total length of cipher text. Only required for CCM */
        if (mode_op == EVP_CIPH_CCM_MODE) {
            if (!EVP_DecryptUpdate(ctx, NULL, &outl,
                                   NULL, lengths[testnum])) {
                BIO_printf(bio_err, "\nCouldn't set cipher text length\n");
                dofail();
                exit(1);
            }
        }
        if (aead) {
            if (!EVP_DecryptUpdate(ctx, NULL, &outl, aad, sizeof(aad))) {
                BIO_printf(bio_err, "\nCouldn't insert AAD when decrypting\n");
                dofail();
                exit(1);
            }
        }
        if (!EVP_DecryptUpdate(ctx, outbuf, &outl, buf, lengths[testnum])) {
            BIO_printf(bio_err, "\nFailed to decrypt the data\n");
            dofail();
            exit(1);
        }
        if (EVP_DecryptFinal_ex(ctx, outbuf, &outl))
            realcount++;
    }
    return realcount;
}

static int RSA_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t *rsa_num = &tempargs->sigsize;
    EVP_PKEY_CTX **rsa_sign_ctx = tempargs->rsa_sign_ctx;
    int ret, count;

    for (count = 0; COND(rsa_c[testnum][0]); count++) {
        *rsa_num = tempargs->buflen;
        ret = EVP_PKEY_sign(rsa_sign_ctx[testnum], buf2, rsa_num, buf, 36);
        if (ret <= 0) {
            BIO_printf(bio_err, "RSA sign failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int RSA_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t rsa_num = tempargs->sigsize;
    EVP_PKEY_CTX **rsa_verify_ctx = tempargs->rsa_verify_ctx;
    int ret, count;

    for (count = 0; COND(rsa_c[testnum][1]); count++) {
        ret = EVP_PKEY_verify(rsa_verify_ctx[testnum], buf2, rsa_num, buf, 36);
        if (ret <= 0) {
            BIO_printf(bio_err, "RSA verify failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int RSA_encrypt_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t *rsa_num = &tempargs->encsize;
    EVP_PKEY_CTX **rsa_encrypt_ctx = tempargs->rsa_encrypt_ctx;
    int ret, count;

    for (count = 0; COND(rsa_c[testnum][2]); count++) {
        *rsa_num = tempargs->buflen;
        ret = EVP_PKEY_encrypt(rsa_encrypt_ctx[testnum], buf2, rsa_num, buf, 36);
        if (ret <= 0) {
            BIO_printf(bio_err, "RSA encrypt failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int RSA_decrypt_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t rsa_num;
    EVP_PKEY_CTX **rsa_decrypt_ctx = tempargs->rsa_decrypt_ctx;
    int ret, count;

    for (count = 0; COND(rsa_c[testnum][3]); count++) {
        rsa_num = tempargs->buflen;
        ret = EVP_PKEY_decrypt(rsa_decrypt_ctx[testnum], buf, &rsa_num, buf2, tempargs->encsize);
        if (ret <= 0) {
            BIO_printf(bio_err, "RSA decrypt failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

#ifndef OPENSSL_NO_DH

static int FFDH_derive_key_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ffdh_ctx = tempargs->ffdh_ctx[testnum];
    unsigned char *derived_secret = tempargs->secret_ff_a;
    int count;

    for (count = 0; COND(ffdh_c[testnum][0]); count++) {
        /* outlen can be overwritten with a too small value (no padding used) */
        size_t outlen = MAX_FFDH_SIZE;

        EVP_PKEY_derive(ffdh_ctx, derived_secret, &outlen);
    }
    return count;
}
#endif /* OPENSSL_NO_DH */

#ifndef OPENSSL_NO_DSA
static int DSA_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t *dsa_num = &tempargs->sigsize;
    EVP_PKEY_CTX **dsa_sign_ctx = tempargs->dsa_sign_ctx;
    int ret, count;

    for (count = 0; COND(dsa_c[testnum][0]); count++) {
        *dsa_num = tempargs->buflen;
        ret = EVP_PKEY_sign(dsa_sign_ctx[testnum], buf2, dsa_num, buf, 20);
        if (ret <= 0) {
            BIO_printf(bio_err, "DSA sign failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int DSA_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t dsa_num = tempargs->sigsize;
    EVP_PKEY_CTX **dsa_verify_ctx = tempargs->dsa_verify_ctx;
    int ret, count;

    for (count = 0; COND(dsa_c[testnum][1]); count++) {
        ret = EVP_PKEY_verify(dsa_verify_ctx[testnum], buf2, dsa_num, buf, 20);
        if (ret <= 0) {
            BIO_printf(bio_err, "DSA verify failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}
#endif /* OPENSSL_NO_DSA */

static int ECDSA_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t *ecdsa_num = &tempargs->sigsize;
    EVP_PKEY_CTX **ecdsa_sign_ctx = tempargs->ecdsa_sign_ctx;
    int ret, count;

    for (count = 0; COND(ecdsa_c[testnum][0]); count++) {
        *ecdsa_num = tempargs->buflen;
        ret = EVP_PKEY_sign(ecdsa_sign_ctx[testnum], buf2, ecdsa_num, buf, 20);
        if (ret <= 0) {
            BIO_printf(bio_err, "ECDSA sign failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int ECDSA_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    unsigned char *buf2 = tempargs->buf2;
    size_t ecdsa_num = tempargs->sigsize;
    EVP_PKEY_CTX **ecdsa_verify_ctx = tempargs->ecdsa_verify_ctx;
    int ret, count;

    for (count = 0; COND(ecdsa_c[testnum][1]); count++) {
        ret = EVP_PKEY_verify(ecdsa_verify_ctx[testnum], buf2, ecdsa_num,
                              buf, 20);
        if (ret <= 0) {
            BIO_printf(bio_err, "ECDSA verify failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

/* ******************************************************************** */

static int ECDH_EVP_derive_key_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->ecdh_ctx[testnum];
    unsigned char *derived_secret = tempargs->secret_a;
    int count;
    size_t *outlen = &(tempargs->outlen[testnum]);

    for (count = 0; COND(ecdh_c[testnum][0]); count++)
        EVP_PKEY_derive(ctx, derived_secret, outlen);

    return count;
}

#ifndef OPENSSL_NO_ECX
static int EdDSA_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MD_CTX **edctx = tempargs->eddsa_ctx;
    unsigned char *eddsasig = tempargs->buf2;
    size_t *eddsasigsize = &tempargs->sigsize;
    int ret, count;

    for (count = 0; COND(eddsa_c[testnum][0]); count++) {
        ret = EVP_DigestSignInit(edctx[testnum], NULL, NULL, NULL, NULL);
        if (ret == 0) {
            BIO_printf(bio_err, "EdDSA sign init failure\n");
            dofail();
            count = -1;
            break;
        }
        ret = EVP_DigestSign(edctx[testnum], eddsasig, eddsasigsize, buf, 20);
        if (ret == 0) {
            BIO_printf(bio_err, "EdDSA sign failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}

static int EdDSA_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MD_CTX **edctx = tempargs->eddsa_ctx2;
    unsigned char *eddsasig = tempargs->buf2;
    size_t eddsasigsize = tempargs->sigsize;
    int ret, count;

    for (count = 0; COND(eddsa_c[testnum][1]); count++) {
        ret = EVP_DigestVerifyInit(edctx[testnum], NULL, NULL, NULL, NULL);
        if (ret == 0) {
            BIO_printf(bio_err, "EdDSA verify init failure\n");
            dofail();
            count = -1;
            break;
        }
        ret = EVP_DigestVerify(edctx[testnum], eddsasig, eddsasigsize, buf, 20);
        if (ret != 1) {
            BIO_printf(bio_err, "EdDSA verify failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}
#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_SM2
static int SM2_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MD_CTX **sm2ctx = tempargs->sm2_ctx;
    unsigned char *sm2sig = tempargs->buf2;
    size_t sm2sigsize;
    int ret, count;
    EVP_PKEY **sm2_pkey = tempargs->sm2_pkey;
    const size_t max_size = EVP_PKEY_get_size(sm2_pkey[testnum]);

    for (count = 0; COND(sm2_c[testnum][0]); count++) {
        sm2sigsize = max_size;

        if (!EVP_DigestSignInit(sm2ctx[testnum], NULL, EVP_sm3(),
                                NULL, sm2_pkey[testnum])) {
            BIO_printf(bio_err, "SM2 init sign failure\n");
            dofail();
            count = -1;
            break;
        }
        ret = EVP_DigestSign(sm2ctx[testnum], sm2sig, &sm2sigsize,
                             buf, 20);
        if (ret == 0) {
            BIO_printf(bio_err, "SM2 sign failure\n");
            dofail();
            count = -1;
            break;
        }
        /* update the latest returned size and always use the fixed buffer size */
        tempargs->sigsize = sm2sigsize;
    }

    return count;
}

static int SM2_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    unsigned char *buf = tempargs->buf;
    EVP_MD_CTX **sm2ctx = tempargs->sm2_vfy_ctx;
    unsigned char *sm2sig = tempargs->buf2;
    size_t sm2sigsize = tempargs->sigsize;
    int ret, count;
    EVP_PKEY **sm2_pkey = tempargs->sm2_pkey;

    for (count = 0; COND(sm2_c[testnum][1]); count++) {
        if (!EVP_DigestVerifyInit(sm2ctx[testnum], NULL, EVP_sm3(),
                                  NULL, sm2_pkey[testnum])) {
            BIO_printf(bio_err, "SM2 verify init failure\n");
            dofail();
            count = -1;
            break;
        }
        ret = EVP_DigestVerify(sm2ctx[testnum], sm2sig, sm2sigsize,
                               buf, 20);
        if (ret != 1) {
            BIO_printf(bio_err, "SM2 verify failure\n");
            dofail();
            count = -1;
            break;
        }
    }
    return count;
}
#endif                         /* OPENSSL_NO_SM2 */

static int KEM_keygen_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->kem_gen_ctx[testnum];
    EVP_PKEY *pkey = NULL;
    int count;

    for (count = 0; COND(kems_c[testnum][0]); count++) {
        if (EVP_PKEY_keygen(ctx, &pkey) <= 0)
            return -1;
        /*
         * runtime defined to quite some degree by randomness,
         * so performance overhead of _free doesn't impact
         * results significantly. In any case this test is
         * meant to permit relative algorithm performance
         * comparison.
         */
        EVP_PKEY_free(pkey);
        pkey = NULL;
    }
    return count;
}

static int KEM_encaps_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->kem_encaps_ctx[testnum];
    size_t out_len = tempargs->kem_out_len[testnum];
    size_t secret_len = tempargs->kem_secret_len[testnum];
    unsigned char *out = tempargs->kem_out[testnum];
    unsigned char *secret = tempargs->kem_send_secret[testnum];
    int count;

    for (count = 0; COND(kems_c[testnum][1]); count++) {
        if (EVP_PKEY_encapsulate(ctx, out, &out_len, secret, &secret_len) <= 0)
            return -1;
    }
    return count;
}

static int KEM_decaps_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->kem_decaps_ctx[testnum];
    size_t out_len = tempargs->kem_out_len[testnum];
    size_t secret_len = tempargs->kem_secret_len[testnum];
    unsigned char *out = tempargs->kem_out[testnum];
    unsigned char *secret = tempargs->kem_send_secret[testnum];
    int count;

    for (count = 0; COND(kems_c[testnum][2]); count++) {
        if (EVP_PKEY_decapsulate(ctx, secret, &secret_len, out, out_len) <= 0)
            return -1;
    }
    return count;
}

static int SIG_keygen_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->sig_gen_ctx[testnum];
    EVP_PKEY *pkey = NULL;
    int count;

    for (count = 0; COND(kems_c[testnum][0]); count++) {
        EVP_PKEY_keygen(ctx, &pkey);
        /* TBD: How much does free influence runtime? */
        EVP_PKEY_free(pkey);
        pkey = NULL;
    }
    return count;
}

static int SIG_sign_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->sig_sign_ctx[testnum];
    /* be sure to not change stored sig: */
    unsigned char *sig = app_malloc(tempargs->sig_max_sig_len[testnum],
                                    "sig sign loop");
    unsigned char md[SHA256_DIGEST_LENGTH] = { 0 };
    size_t md_len = SHA256_DIGEST_LENGTH;
    int count;

    for (count = 0; COND(kems_c[testnum][1]); count++) {
        size_t sig_len = tempargs->sig_max_sig_len[testnum];
        int ret = EVP_PKEY_sign(ctx, sig, &sig_len, md, md_len);

        if (ret <= 0) {
            BIO_printf(bio_err, "SIG sign failure at count %d\n", count);
            dofail();
            count = -1;
            break;
        }
    }
    OPENSSL_free(sig);
    return count;
}

static int SIG_verify_loop(void *args)
{
    loopargs_t *tempargs = *(loopargs_t **) args;
    EVP_PKEY_CTX *ctx = tempargs->sig_verify_ctx[testnum];
    size_t sig_len = tempargs->sig_act_sig_len[testnum];
    unsigned char *sig = tempargs->sig_sig[testnum];
    unsigned char md[SHA256_DIGEST_LENGTH] = { 0 };
    size_t md_len = SHA256_DIGEST_LENGTH;
    int count;

    for (count = 0; COND(kems_c[testnum][2]); count++) {
        int ret = EVP_PKEY_verify(ctx, sig, sig_len, md, md_len);

        if (ret <= 0) {
            BIO_printf(bio_err, "SIG verify failure at count %d\n", count);
            dofail();
            count = -1;
            break;
        }

    }
    return count;
}

static int check_block_size(EVP_CIPHER_CTX *ctx, int length)
{
    const EVP_CIPHER *ciph = EVP_CIPHER_CTX_get0_cipher(ctx);
    int blocksize = EVP_CIPHER_CTX_get_block_size(ctx);

    if (ciph == NULL || blocksize <= 0) {
        BIO_printf(bio_err, "\nInvalid cipher!\n");
        return 0;
    }
    if (length % blocksize != 0) {
        BIO_printf(bio_err,
                   "\nRequested encryption length not a multiple of block size for %s!\n",
                   EVP_CIPHER_get0_name(ciph));
        return 0;
    }
    return 1;
}

static int run_benchmark(int async_jobs,
                         int (*loop_function) (void *), loopargs_t *loopargs)
{
    int job_op_count = 0;
    int total_op_count = 0;
    int num_inprogress = 0;
    int error = 0, i = 0, ret = 0;
    OSSL_ASYNC_FD job_fd = 0;
    size_t num_job_fds = 0;

    if (async_jobs == 0) {
        return loop_function((void *)&loopargs);
    }

    for (i = 0; i < async_jobs && !error; i++) {
        loopargs_t *looparg_item = loopargs + i;

        /* Copy pointer content (looparg_t item address) into async context */
        ret = ASYNC_start_job(&loopargs[i].inprogress_job, loopargs[i].wait_ctx,
                              &job_op_count, loop_function,
                              (void *)&looparg_item, sizeof(looparg_item));
        switch (ret) {
        case ASYNC_PAUSE:
            ++num_inprogress;
            break;
        case ASYNC_FINISH:
            if (job_op_count == -1) {
                error = 1;
            } else {
                total_op_count += job_op_count;
            }
            break;
        case ASYNC_NO_JOBS:
        case ASYNC_ERR:
            BIO_printf(bio_err, "Failure in the job\n");
            dofail();
            error = 1;
            break;
        }
    }

    while (num_inprogress > 0) {
#if defined(OPENSSL_SYS_WINDOWS)
        DWORD avail = 0;
#elif defined(OPENSSL_SYS_UNIX)
        int select_result = 0;
        OSSL_ASYNC_FD max_fd = 0;
        fd_set waitfdset;

        FD_ZERO(&waitfdset);

        for (i = 0; i < async_jobs && num_inprogress > 0; i++) {
            if (loopargs[i].inprogress_job == NULL)
                continue;

            if (!ASYNC_WAIT_CTX_get_all_fds
                (loopargs[i].wait_ctx, NULL, &num_job_fds)
                || num_job_fds > 1) {
                BIO_printf(bio_err, "Too many fds in ASYNC_WAIT_CTX\n");
                dofail();
                error = 1;
                break;
            }
            ASYNC_WAIT_CTX_get_all_fds(loopargs[i].wait_ctx, &job_fd,
                                       &num_job_fds);
            FD_SET(job_fd, &waitfdset);
            if (job_fd > max_fd)
                max_fd = job_fd;
        }

        if (max_fd >= (OSSL_ASYNC_FD)FD_SETSIZE) {
            BIO_printf(bio_err,
                       "Error: max_fd (%d) must be smaller than FD_SETSIZE (%d). "
                       "Decrease the value of async_jobs\n",
                       max_fd, FD_SETSIZE);
            dofail();
            error = 1;
            break;
        }

        select_result = select(max_fd + 1, &waitfdset, NULL, NULL, NULL);
        if (select_result == -1 && errno == EINTR)
            continue;

        if (select_result == -1) {
            BIO_printf(bio_err, "Failure in the select\n");
            dofail();
            error = 1;
            break;
        }

        if (select_result == 0)
            continue;
#endif

        for (i = 0; i < async_jobs; i++) {
            if (loopargs[i].inprogress_job == NULL)
                continue;

            if (!ASYNC_WAIT_CTX_get_all_fds
                (loopargs[i].wait_ctx, NULL, &num_job_fds)
                || num_job_fds > 1) {
                BIO_printf(bio_err, "Too many fds in ASYNC_WAIT_CTX\n");
                dofail();
                error = 1;
                break;
            }
            ASYNC_WAIT_CTX_get_all_fds(loopargs[i].wait_ctx, &job_fd,
                                       &num_job_fds);

#if defined(OPENSSL_SYS_UNIX)
            if (num_job_fds == 1 && !FD_ISSET(job_fd, &waitfdset))
                continue;
#elif defined(OPENSSL_SYS_WINDOWS)
            if (num_job_fds == 1
                && !PeekNamedPipe(job_fd, NULL, 0, NULL, &avail, NULL)
                && avail > 0)
                continue;
#endif

            ret = ASYNC_start_job(&loopargs[i].inprogress_job,
                                  loopargs[i].wait_ctx, &job_op_count,
                                  loop_function, (void *)(loopargs + i),
                                  sizeof(loopargs_t));
            switch (ret) {
            case ASYNC_PAUSE:
                break;
            case ASYNC_FINISH:
                if (job_op_count == -1) {
                    error = 1;
                } else {
                    total_op_count += job_op_count;
                }
                --num_inprogress;
                loopargs[i].inprogress_job = NULL;
                break;
            case ASYNC_NO_JOBS:
            case ASYNC_ERR:
                --num_inprogress;
                loopargs[i].inprogress_job = NULL;
                BIO_printf(bio_err, "Failure in the job\n");
                dofail();
                error = 1;
                break;
            }
        }
    }

    return error ? -1 : total_op_count;
}

typedef struct ec_curve_st {
    const char *name;
    unsigned int nid;
    unsigned int bits;
    size_t sigsize; /* only used for EdDSA curves */
} EC_CURVE;

static EVP_PKEY *get_ecdsa(const EC_CURVE *curve)
{
    EVP_PKEY_CTX *kctx = NULL;
    EVP_PKEY *key = NULL;

    /* Ensure that the error queue is empty */
    if (ERR_peek_error()) {
        BIO_printf(bio_err,
                   "WARNING: the error queue contains previous unhandled errors.\n");
        dofail();
    }

    /*
     * Let's try to create a ctx directly from the NID: this works for
     * curves like Curve25519 that are not implemented through the low
     * level EC interface.
     * If this fails we try creating a EVP_PKEY_EC generic param ctx,
     * then we set the curve by NID before deriving the actual keygen
     * ctx for that specific curve.
     */
    kctx = EVP_PKEY_CTX_new_id(curve->nid, NULL);
    if (kctx == NULL) {
        EVP_PKEY_CTX *pctx = NULL;
        EVP_PKEY *params = NULL;
        /*
         * If we reach this code EVP_PKEY_CTX_new_id() failed and a
         * "int_ctx_new:unsupported algorithm" error was added to the
         * error queue.
         * We remove it from the error queue as we are handling it.
         */
        unsigned long error = ERR_peek_error();

        if (error == ERR_peek_last_error() /* oldest and latest errors match */
            /* check that the error origin matches */
            && ERR_GET_LIB(error) == ERR_LIB_EVP
            && (ERR_GET_REASON(error) == EVP_R_UNSUPPORTED_ALGORITHM
                || ERR_GET_REASON(error) == ERR_R_UNSUPPORTED))
            ERR_get_error(); /* pop error from queue */
        if (ERR_peek_error()) {
            BIO_printf(bio_err,
                       "Unhandled error in the error queue during EC key setup.\n");
            dofail();
            return NULL;
        }

        /* Create the context for parameter generation */
        if ((pctx = EVP_PKEY_CTX_new_from_name(NULL, "EC", NULL)) == NULL
            || EVP_PKEY_paramgen_init(pctx) <= 0
            || EVP_PKEY_CTX_set_ec_paramgen_curve_nid(pctx,
                                                      curve->nid) <= 0
            || EVP_PKEY_paramgen(pctx, &params) <= 0) {
            BIO_printf(bio_err, "EC params init failure.\n");
            dofail();
            EVP_PKEY_CTX_free(pctx);
            return NULL;
        }
        EVP_PKEY_CTX_free(pctx);

        /* Create the context for the key generation */
        kctx = EVP_PKEY_CTX_new(params, NULL);
        EVP_PKEY_free(params);
    }
    if (kctx == NULL
        || EVP_PKEY_keygen_init(kctx) <= 0
        || EVP_PKEY_keygen(kctx, &key) <= 0) {
        BIO_printf(bio_err, "EC key generation failure.\n");
        dofail();
        key = NULL;
    }
    EVP_PKEY_CTX_free(kctx);
    return key;
}

#define stop_it(do_it, test_num)\
    memset(do_it + test_num, 0, OSSL_NELEM(do_it) - test_num);

/* Checks to see if algorithms are fetchable */
#define IS_FETCHABLE(type, TYPE)                                \
    static int is_ ## type ## _fetchable(const TYPE *alg)       \
    {                                                           \
        TYPE *impl;                                             \
        const char *propq = app_get0_propq();                   \
        OSSL_LIB_CTX *libctx = app_get0_libctx();               \
        const char *name = TYPE ## _get0_name(alg);             \
                                                                \
        ERR_set_mark();                                         \
        impl = TYPE ## _fetch(libctx, name, propq);             \
        ERR_pop_to_mark();                                      \
        if (impl == NULL)                                       \
            return 0;                                           \
        TYPE ## _free(impl);                                    \
        return 1;                                               \
    }

IS_FETCHABLE(signature, EVP_SIGNATURE)
IS_FETCHABLE(kem, EVP_KEM)

DEFINE_STACK_OF(EVP_KEM)

static int kems_cmp(const EVP_KEM * const *a,
                    const EVP_KEM * const *b)
{
    return strcmp(OSSL_PROVIDER_get0_name(EVP_KEM_get0_provider(*a)),
                  OSSL_PROVIDER_get0_name(EVP_KEM_get0_provider(*b)));
}

static void collect_kem(EVP_KEM *kem, void *stack)
{
    STACK_OF(EVP_KEM) *kem_stack = stack;

    if (is_kem_fetchable(kem)
            && EVP_KEM_up_ref(kem)
            && sk_EVP_KEM_push(kem_stack, kem) <= 0)
        EVP_KEM_free(kem); /* up-ref successful but push to stack failed */
}

static int kem_locate(const char *algo, unsigned int *idx)
{
    unsigned int i;

    for (i = 0; i < kems_algs_len; i++) {
        if (strcmp(kems_algname[i], algo) == 0) {
            *idx = i;
            return 1;
        }
    }
    return 0;
}

DEFINE_STACK_OF(EVP_SIGNATURE)

static int signatures_cmp(const EVP_SIGNATURE * const *a,
                          const EVP_SIGNATURE * const *b)
{
    return strcmp(OSSL_PROVIDER_get0_name(EVP_SIGNATURE_get0_provider(*a)),
                  OSSL_PROVIDER_get0_name(EVP_SIGNATURE_get0_provider(*b)));
}

static void collect_signatures(EVP_SIGNATURE *sig, void *stack)
{
    STACK_OF(EVP_SIGNATURE) *sig_stack = stack;

    if (is_signature_fetchable(sig)
            && EVP_SIGNATURE_up_ref(sig)
            && sk_EVP_SIGNATURE_push(sig_stack, sig) <= 0)
        EVP_SIGNATURE_free(sig); /* up-ref successful but push to stack failed */
}

static int sig_locate(const char *algo, unsigned int *idx)
{
    unsigned int i;

    for (i = 0; i < sigs_algs_len; i++) {
        if (strcmp(sigs_algname[i], algo) == 0) {
            *idx = i;
            return 1;
        }
    }
    return 0;
}

static int get_max(const uint8_t doit[], size_t algs_len) {
    size_t i = 0;
    int maxcnt = 0;

    for (i = 0; i < algs_len; i++)
        if (maxcnt < doit[i]) maxcnt = doit[i];
    return maxcnt;
}

int speed_main(int argc, char **argv)
{
    CONF *conf = NULL;
    ENGINE *e = NULL;
    loopargs_t *loopargs = NULL;
    const char *prog;
    const char *engine_id = NULL;
    EVP_CIPHER *evp_cipher = NULL;
    EVP_MAC *mac = NULL;
    double d = 0.0;
    OPTION_CHOICE o;
    int async_init = 0, multiblock = 0, pr_header = 0;
    uint8_t doit[ALGOR_NUM] = { 0 };
    int ret = 1, misalign = 0, lengths_single = 0;
    STACK_OF(EVP_KEM) *kem_stack = NULL;
    STACK_OF(EVP_SIGNATURE) *sig_stack = NULL;
    long count = 0;
    unsigned int size_num = SIZE_NUM;
    unsigned int i, k, loopargs_len = 0, async_jobs = 0;
    unsigned int idx;
    int keylen = 0;
    int buflen;
    size_t declen;
    BIGNUM *bn = NULL;
    EVP_PKEY_CTX *genctx = NULL;
#ifndef NO_FORK
    int multi = 0;
#endif
    long op_count = 1;
    openssl_speed_sec_t seconds = { SECONDS, RSA_SECONDS, DSA_SECONDS,
                                    ECDSA_SECONDS, ECDH_SECONDS,
                                    EdDSA_SECONDS, SM2_SECONDS,
                                    FFDH_SECONDS, KEM_SECONDS,
                                    SIG_SECONDS };

    static const unsigned char key32[32] = {
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
        0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12,
        0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34,
        0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56
    };
    static const unsigned char deskey[] = {
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, /* key1 */
        0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, /* key2 */
        0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34  /* key3 */
    };
    static const struct {
        const unsigned char *data;
        unsigned int length;
        unsigned int bits;
    } rsa_keys[] = {
        {   test512,   sizeof(test512),   512 },
        {  test1024,  sizeof(test1024),  1024 },
        {  test2048,  sizeof(test2048),  2048 },
        {  test3072,  sizeof(test3072),  3072 },
        {  test4096,  sizeof(test4096),  4096 },
        {  test7680,  sizeof(test7680),  7680 },
        { test15360, sizeof(test15360), 15360 }
    };
    uint8_t rsa_doit[RSA_NUM] = { 0 };
    int primes = RSA_DEFAULT_PRIME_NUM;
#ifndef OPENSSL_NO_DH
    typedef struct ffdh_params_st {
        const char *name;
        unsigned int nid;
        unsigned int bits;
    } FFDH_PARAMS;

    static const FFDH_PARAMS ffdh_params[FFDH_NUM] = {
        {"ffdh2048", NID_ffdhe2048, 2048},
        {"ffdh3072", NID_ffdhe3072, 3072},
        {"ffdh4096", NID_ffdhe4096, 4096},
        {"ffdh6144", NID_ffdhe6144, 6144},
        {"ffdh8192", NID_ffdhe8192, 8192}
    };
    uint8_t ffdh_doit[FFDH_NUM] = { 0 };

#endif /* OPENSSL_NO_DH */
#ifndef OPENSSL_NO_DSA
    static const unsigned int dsa_bits[DSA_NUM] = { 1024, 2048 };
    uint8_t dsa_doit[DSA_NUM] = { 0 };
#endif /* OPENSSL_NO_DSA */
    /*
     * We only test over the following curves as they are representative, To
     * add tests over more curves, simply add the curve NID and curve name to
     * the following arrays and increase the |ecdh_choices| and |ecdsa_choices|
     * lists accordingly.
     */
    static const EC_CURVE ec_curves[EC_NUM] = {
        /* Prime Curves */
        {"secp160r1", NID_secp160r1, 160},
        {"nistp192", NID_X9_62_prime192v1, 192},
        {"nistp224", NID_secp224r1, 224},
        {"nistp256", NID_X9_62_prime256v1, 256},
        {"nistp384", NID_secp384r1, 384},
        {"nistp521", NID_secp521r1, 521},
#ifndef OPENSSL_NO_EC2M
        /* Binary Curves */
        {"nistk163", NID_sect163k1, 163},
        {"nistk233", NID_sect233k1, 233},
        {"nistk283", NID_sect283k1, 283},
        {"nistk409", NID_sect409k1, 409},
        {"nistk571", NID_sect571k1, 571},
        {"nistb163", NID_sect163r2, 163},
        {"nistb233", NID_sect233r1, 233},
        {"nistb283", NID_sect283r1, 283},
        {"nistb409", NID_sect409r1, 409},
        {"nistb571", NID_sect571r1, 571},
#endif
        {"brainpoolP256r1", NID_brainpoolP256r1, 256},
        {"brainpoolP256t1", NID_brainpoolP256t1, 256},
        {"brainpoolP384r1", NID_brainpoolP384r1, 384},
        {"brainpoolP384t1", NID_brainpoolP384t1, 384},
        {"brainpoolP512r1", NID_brainpoolP512r1, 512},
        {"brainpoolP512t1", NID_brainpoolP512t1, 512},
#ifndef OPENSSL_NO_ECX
        /* Other and ECDH only ones */
        {"X25519", NID_X25519, 253},
        {"X448", NID_X448, 448}
#endif
    };
#ifndef OPENSSL_NO_ECX
    static const EC_CURVE ed_curves[EdDSA_NUM] = {
        /* EdDSA */
        {"Ed25519", NID_ED25519, 253, 64},
        {"Ed448", NID_ED448, 456, 114}
    };
#endif /* OPENSSL_NO_ECX */
#ifndef OPENSSL_NO_SM2
    static const EC_CURVE sm2_curves[SM2_NUM] = {
        /* SM2 */
        {"CurveSM2", NID_sm2, 256}
    };
    uint8_t sm2_doit[SM2_NUM] = { 0 };
#endif
    uint8_t ecdsa_doit[ECDSA_NUM] = { 0 };
    uint8_t ecdh_doit[EC_NUM] = { 0 };
#ifndef OPENSSL_NO_ECX
    uint8_t eddsa_doit[EdDSA_NUM] = { 0 };
#endif /* OPENSSL_NO_ECX */

    uint8_t kems_doit[MAX_KEM_NUM] = { 0 };
    uint8_t sigs_doit[MAX_SIG_NUM] = { 0 };

    uint8_t do_kems = 0;
    uint8_t do_sigs = 0;

    /* checks declared curves against choices list. */
#ifndef OPENSSL_NO_ECX
    OPENSSL_assert(ed_curves[EdDSA_NUM - 1].nid == NID_ED448);
    OPENSSL_assert(strcmp(eddsa_choices[EdDSA_NUM - 1].name, "ed448") == 0);

    OPENSSL_assert(ec_curves[EC_NUM - 1].nid == NID_X448);
    OPENSSL_assert(strcmp(ecdh_choices[EC_NUM - 1].name, "ecdhx448") == 0);

    OPENSSL_assert(ec_curves[ECDSA_NUM - 1].nid == NID_brainpoolP512t1);
    OPENSSL_assert(strcmp(ecdsa_choices[ECDSA_NUM - 1].name, "ecdsabrp512t1") == 0);
#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_SM2
    OPENSSL_assert(sm2_curves[SM2_NUM - 1].nid == NID_sm2);
    OPENSSL_assert(strcmp(sm2_choices[SM2_NUM - 1].name, "curveSM2") == 0);
#endif

    prog = opt_init(argc, argv, speed_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
 opterr:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(speed_options);
            ret = 0;
            goto end;
        case OPT_ELAPSED:
            usertime = 0;
            break;
        case OPT_EVP:
            if (doit[D_EVP]) {
                BIO_printf(bio_err, "%s: -evp option cannot be used more than once\n", prog);
                goto opterr;
            }
            ERR_set_mark();
            if (!opt_cipher_silent(opt_arg(), &evp_cipher)) {
                if (have_md(opt_arg()))
                    evp_md_name = opt_arg();
            }
            if (evp_cipher == NULL && evp_md_name == NULL) {
                ERR_clear_last_mark();
                BIO_printf(bio_err,
                           "%s: %s is an unknown cipher or digest\n",
                           prog, opt_arg());
                goto end;
            }
            ERR_pop_to_mark();
            doit[D_EVP] = 1;
            break;
        case OPT_HMAC:
            if (!have_md(opt_arg())) {
                BIO_printf(bio_err, "%s: %s is an unknown digest\n",
                           prog, opt_arg());
                goto end;
            }
            evp_mac_mdname = opt_arg();
            doit[D_HMAC] = 1;
            break;
        case OPT_CMAC:
            if (!have_cipher(opt_arg())) {
                BIO_printf(bio_err, "%s: %s is an unknown cipher\n",
                           prog, opt_arg());
                goto end;
            }
            evp_mac_ciphername = opt_arg();
            doit[D_EVP_CMAC] = 1;
            break;
        case OPT_DECRYPT:
            decrypt = 1;
            break;
        case OPT_ENGINE:
            /*
             * In a forked execution, an engine might need to be
             * initialised by each child process, not by the parent.
             * So store the name here and run setup_engine() later on.
             */
            engine_id = opt_arg();
            break;
        case OPT_MULTI:
#ifndef NO_FORK
            multi = opt_int_arg();
            if ((size_t)multi >= SIZE_MAX / sizeof(int)) {
                BIO_printf(bio_err, "%s: multi argument too large\n", prog);
                return 0;
            }
#endif
            break;
        case OPT_ASYNCJOBS:
#ifndef OPENSSL_NO_ASYNC
            async_jobs = opt_int_arg();
            if (async_jobs > 99999) {
                BIO_printf(bio_err, "%s: too many async_jobs\n", prog);
                goto opterr;
            }
            if (!ASYNC_is_capable()) {
                BIO_printf(bio_err,
                           "%s: async_jobs specified but async not supported\n",
                           prog);
                if (testmode)
                    /* Return success in the testmode. */
                    return 0;
                goto opterr;
            }
#endif
            break;
        case OPT_MISALIGN:
            misalign = opt_int_arg();
            if (misalign > MISALIGN) {
                BIO_printf(bio_err,
                           "%s: Maximum offset is %d\n", prog, MISALIGN);
                goto opterr;
            }
            break;
        case OPT_MR:
            mr = 1;
            break;
        case OPT_MB:
            multiblock = 1;
#ifdef OPENSSL_NO_MULTIBLOCK
            BIO_printf(bio_err,
                       "%s: -mb specified but multi-block support is disabled\n",
                       prog);
            goto end;
#endif
            break;
        case OPT_R_CASES:
            if (!opt_rand(o))
                goto end;
            break;
        case OPT_PROV_CASES:
            if (!opt_provider(o))
                goto end;
            break;
        case OPT_CONFIG:
            conf = app_load_config_modules(opt_arg());
            if (conf == NULL)
                goto end;
            break;
        case OPT_PRIMES:
            primes = opt_int_arg();
            break;
        case OPT_SECONDS:
            seconds.sym = seconds.rsa = seconds.dsa = seconds.ecdsa
                        = seconds.ecdh = seconds.eddsa
                        = seconds.sm2 = seconds.ffdh
                        = seconds.kem = seconds.sig = opt_int_arg();
            break;
        case OPT_BYTES:
            lengths_single = opt_int_arg();
            lengths = &lengths_single;
            size_num = 1;
            break;
        case OPT_AEAD:
            aead = 1;
            break;
        case OPT_KEM:
            do_kems = 1;
            break;
        case OPT_SIG:
            do_sigs = 1;
            break;
        case OPT_MLOCK:
            domlock = 1;
#if !defined(_WIN32) && !defined(OPENSSL_SYS_LINUX)
            BIO_printf(bio_err,
                       "%s: -mlock not supported on this platform\n",
                       prog);
            goto end;
#endif
            break;
        case OPT_TESTMODE:
            testmode = 1;
            break;
        }
    }

    /* find all KEMs currently available */
    kem_stack = sk_EVP_KEM_new(kems_cmp);
    EVP_KEM_do_all_provided(app_get0_libctx(), collect_kem, kem_stack);

    kems_algs_len = 0;

    for (idx = 0; idx < (unsigned int)sk_EVP_KEM_num(kem_stack); idx++) {
        EVP_KEM *kem = sk_EVP_KEM_value(kem_stack, idx);

        if (strcmp(EVP_KEM_get0_name(kem), "RSA") == 0) {
            if (kems_algs_len + OSSL_NELEM(rsa_choices) >= MAX_KEM_NUM) {
                BIO_printf(bio_err,
                           "Too many KEMs registered. Change MAX_KEM_NUM.\n");
                goto end;
            }
            for (i = 0; i < OSSL_NELEM(rsa_choices); i++) {
                kems_doit[kems_algs_len] = 1;
                kems_algname[kems_algs_len++] = OPENSSL_strdup(rsa_choices[i].name);
            }
        } else if (strcmp(EVP_KEM_get0_name(kem), "EC") == 0) {
            if (kems_algs_len + 3 >= MAX_KEM_NUM) {
                BIO_printf(bio_err,
                           "Too many KEMs registered. Change MAX_KEM_NUM.\n");
                goto end;
            }
            kems_doit[kems_algs_len] = 1;
            kems_algname[kems_algs_len++] = OPENSSL_strdup("ECP-256");
            kems_doit[kems_algs_len] = 1;
            kems_algname[kems_algs_len++] = OPENSSL_strdup("ECP-384");
            kems_doit[kems_algs_len] = 1;
            kems_algname[kems_algs_len++] = OPENSSL_strdup("ECP-521");
        } else {
            if (kems_algs_len + 1 >= MAX_KEM_NUM) {
                BIO_printf(bio_err,
                           "Too many KEMs registered. Change MAX_KEM_NUM.\n");
                goto end;
            }
            kems_doit[kems_algs_len] = 1;
            kems_algname[kems_algs_len++] = OPENSSL_strdup(EVP_KEM_get0_name(kem));
        }
    }
    sk_EVP_KEM_pop_free(kem_stack, EVP_KEM_free);
    kem_stack = NULL;

    /* find all SIGNATUREs currently available */
    sig_stack = sk_EVP_SIGNATURE_new(signatures_cmp);
    EVP_SIGNATURE_do_all_provided(app_get0_libctx(), collect_signatures, sig_stack);

    sigs_algs_len = 0;

    for (idx = 0; idx < (unsigned int)sk_EVP_SIGNATURE_num(sig_stack); idx++) {
        EVP_SIGNATURE *s = sk_EVP_SIGNATURE_value(sig_stack, idx);
        const char *sig_name = EVP_SIGNATURE_get0_name(s);

        if (strcmp(sig_name, "RSA") == 0) {
            if (sigs_algs_len + OSSL_NELEM(rsa_choices) >= MAX_SIG_NUM) {
                BIO_printf(bio_err,
                           "Too many signatures registered. Change MAX_SIG_NUM.\n");
                goto end;
            }
            for (i = 0; i < OSSL_NELEM(rsa_choices); i++) {
                sigs_doit[sigs_algs_len] = 1;
                sigs_algname[sigs_algs_len++] = OPENSSL_strdup(rsa_choices[i].name);
            }
        }
#ifndef OPENSSL_NO_DSA
        else if (strcmp(sig_name, "DSA") == 0) {
            if (sigs_algs_len + DSA_NUM >= MAX_SIG_NUM) {
                BIO_printf(bio_err,
                           "Too many signatures registered. Change MAX_SIG_NUM.\n");
                goto end;
            }
            for (i = 0; i < DSA_NUM; i++) {
                sigs_doit[sigs_algs_len] = 1;
                sigs_algname[sigs_algs_len++] = OPENSSL_strdup(dsa_choices[i].name);
            }
        }
#endif /* OPENSSL_NO_DSA */
        /* skipping these algs as tested elsewhere - and b/o setup is a pain */
        else if (strcmp(sig_name, "ED25519") &&
                 strcmp(sig_name, "ED448") &&
                 strcmp(sig_name, "ECDSA") &&
                 strcmp(sig_name, "HMAC") &&
                 strcmp(sig_name, "SIPHASH") &&
                 strcmp(sig_name, "POLY1305") &&
                 strcmp(sig_name, "CMAC") &&
                 strcmp(sig_name, "SM2")) { /* skip alg */
            if (sigs_algs_len + 1 >= MAX_SIG_NUM) {
                BIO_printf(bio_err,
                           "Too many signatures registered. Change MAX_SIG_NUM.\n");
                goto end;
            }
            /* activate this provider algorithm */
            sigs_doit[sigs_algs_len] = 1;
            sigs_algname[sigs_algs_len++] = OPENSSL_strdup(sig_name);
        }
    }
    sk_EVP_SIGNATURE_pop_free(sig_stack, EVP_SIGNATURE_free);
    sig_stack = NULL;

    /* Remaining arguments are algorithms. */
    argc = opt_num_rest();
    argv = opt_rest();

    if (!app_RAND_load())
        goto end;

    for (; *argv; argv++) {
        const char *algo = *argv;
        int algo_found = 0;

        if (opt_found(algo, doit_choices, &i)) {
            doit[i] = 1;
            algo_found = 1;
        }
        if (strcmp(algo, "des") == 0) {
            doit[D_CBC_DES] = doit[D_EDE3_DES] = 1;
            algo_found = 1;
        }
        if (strcmp(algo, "sha") == 0) {
            doit[D_SHA1] = doit[D_SHA256] = doit[D_SHA512] = 1;
            algo_found = 1;
        }
#ifndef OPENSSL_NO_DEPRECATED_3_0
        if (strcmp(algo, "openssl") == 0) /* just for compatibility */
            algo_found = 1;
#endif
        if (HAS_PREFIX(algo, "rsa")) {
            if (algo[sizeof("rsa") - 1] == '\0') {
                memset(rsa_doit, 1, sizeof(rsa_doit));
                algo_found = 1;
            }
            if (opt_found(algo, rsa_choices, &i)) {
                rsa_doit[i] = 1;
                algo_found = 1;
            }
        }
#ifndef OPENSSL_NO_DH
        if (HAS_PREFIX(algo, "ffdh")) {
            if (algo[sizeof("ffdh") - 1] == '\0') {
                memset(ffdh_doit, 1, sizeof(ffdh_doit));
                algo_found = 1;
            }
            if (opt_found(algo, ffdh_choices, &i)) {
                ffdh_doit[i] = 2;
                algo_found = 1;
            }
        }
#endif
#ifndef OPENSSL_NO_DSA
        if (HAS_PREFIX(algo, "dsa")) {
            if (algo[sizeof("dsa") - 1] == '\0') {
                memset(dsa_doit, 1, sizeof(dsa_doit));
                algo_found = 1;
            }
            if (opt_found(algo, dsa_choices, &i)) {
                dsa_doit[i] = 2;
                algo_found = 1;
            }
        }
#endif
        if (strcmp(algo, "aes") == 0) {
            doit[D_CBC_128_AES] = doit[D_CBC_192_AES] = doit[D_CBC_256_AES] = 1;
            algo_found = 1;
        }
        if (strcmp(algo, "camellia") == 0) {
            doit[D_CBC_128_CML] = doit[D_CBC_192_CML] = doit[D_CBC_256_CML] = 1;
            algo_found = 1;
        }
        if (HAS_PREFIX(algo, "ecdsa")) {
            if (algo[sizeof("ecdsa") - 1] == '\0') {
                memset(ecdsa_doit, 1, sizeof(ecdsa_doit));
                algo_found = 1;
            }
            if (opt_found(algo, ecdsa_choices, &i)) {
                ecdsa_doit[i] = 2;
                algo_found = 1;
            }
        }
        if (HAS_PREFIX(algo, "ecdh")) {
            if (algo[sizeof("ecdh") - 1] == '\0') {
                memset(ecdh_doit, 1, sizeof(ecdh_doit));
                algo_found = 1;
            }
            if (opt_found(algo, ecdh_choices, &i)) {
                ecdh_doit[i] = 2;
                algo_found = 1;
            }
        }
#ifndef OPENSSL_NO_ECX
        if (strcmp(algo, "eddsa") == 0) {
            memset(eddsa_doit, 1, sizeof(eddsa_doit));
            algo_found = 1;
        }
        if (opt_found(algo, eddsa_choices, &i)) {
            eddsa_doit[i] = 2;
            algo_found = 1;
        }
#endif /* OPENSSL_NO_ECX */
#ifndef OPENSSL_NO_SM2
        if (strcmp(algo, "sm2") == 0) {
            memset(sm2_doit, 1, sizeof(sm2_doit));
            algo_found = 1;
        }
        if (opt_found(algo, sm2_choices, &i)) {
            sm2_doit[i] = 2;
            algo_found = 1;
        }
#endif
        if (kem_locate(algo, &idx)) {
            kems_doit[idx]++;
            do_kems = 1;
            algo_found = 1;
        }
        if (sig_locate(algo, &idx)) {
            sigs_doit[idx]++;
            do_sigs = 1;
            algo_found = 1;
        }
        if (strcmp(algo, "kmac") == 0) {
            doit[D_KMAC128] = doit[D_KMAC256] = 1;
            algo_found = 1;
        }
        if (strcmp(algo, "cmac") == 0) {
            doit[D_EVP_CMAC] = 1;
            algo_found = 1;
        }

        if (!algo_found) {
            BIO_printf(bio_err, "%s: Unknown algorithm %s\n", prog, algo);
            goto end;
        }
    }

    /* Sanity checks */
    if (aead) {
        if (evp_cipher == NULL) {
            BIO_printf(bio_err, "-aead can be used only with an AEAD cipher\n");
            goto end;
        } else if (!(EVP_CIPHER_get_flags(evp_cipher) &
                     EVP_CIPH_FLAG_AEAD_CIPHER)) {
            BIO_printf(bio_err, "%s is not an AEAD cipher\n",
                       EVP_CIPHER_get0_name(evp_cipher));
            goto end;
        }
    }
    if (kems_algs_len > 0) {
        int maxcnt = get_max(kems_doit, kems_algs_len);

        if (maxcnt > 1) {
            /* some algs explicitly selected */
            for (i = 0; i < kems_algs_len; i++) {
                /* disable the rest */
                kems_doit[i]--;
            }
        }
    }
    if (sigs_algs_len > 0) {
        int maxcnt = get_max(sigs_doit, sigs_algs_len);

        if (maxcnt > 1) {
            /* some algs explicitly selected */
            for (i = 0; i < sigs_algs_len; i++) {
                /* disable the rest */
                sigs_doit[i]--;
            }
        }
    }
    if (multiblock) {
        if (evp_cipher == NULL) {
            BIO_printf(bio_err, "-mb can be used only with a multi-block"
                                " capable cipher\n");
            goto end;
        } else if (!(EVP_CIPHER_get_flags(evp_cipher) &
                     EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK)) {
            BIO_printf(bio_err, "%s is not a multi-block capable\n",
                       EVP_CIPHER_get0_name(evp_cipher));
            goto end;
        } else if (async_jobs > 0) {
            BIO_printf(bio_err, "Async mode is not supported with -mb");
            goto end;
        }
    }

    /* Initialize the job pool if async mode is enabled */
    if (async_jobs > 0) {
        async_init = ASYNC_init_thread(async_jobs, async_jobs);
        if (!async_init) {
            BIO_printf(bio_err, "Error creating the ASYNC job pool\n");
            goto end;
        }
    }

    loopargs_len = (async_jobs == 0 ? 1 : async_jobs);
    loopargs =
        app_malloc(loopargs_len * sizeof(loopargs_t), "array of loopargs");
    memset(loopargs, 0, loopargs_len * sizeof(loopargs_t));

    buflen = lengths[size_num - 1];
    if (buflen < 36)    /* size of random vector in RSA benchmark */
        buflen = 36;
    if (INT_MAX - (MAX_MISALIGNMENT + 1) < buflen) {
        BIO_printf(bio_err, "Error: buffer size too large\n");
        goto end;
    }
    buflen += MAX_MISALIGNMENT + 1;
    for (i = 0; i < loopargs_len; i++) {
        if (async_jobs > 0) {
            loopargs[i].wait_ctx = ASYNC_WAIT_CTX_new();
            if (loopargs[i].wait_ctx == NULL) {
                BIO_printf(bio_err, "Error creating the ASYNC_WAIT_CTX\n");
                goto end;
            }
        }

        loopargs[i].buf_malloc = app_malloc(buflen, "input buffer");
        loopargs[i].buf2_malloc = app_malloc(buflen, "input buffer");

        /* Align the start of buffers on a 64 byte boundary */
        loopargs[i].buf = loopargs[i].buf_malloc + misalign;
        loopargs[i].buf2 = loopargs[i].buf2_malloc + misalign;
        loopargs[i].buflen = buflen - misalign;
        loopargs[i].sigsize = buflen - misalign;
        loopargs[i].secret_a = app_malloc(MAX_ECDH_SIZE, "ECDH secret a");
        loopargs[i].secret_b = app_malloc(MAX_ECDH_SIZE, "ECDH secret b");
#ifndef OPENSSL_NO_DH
        loopargs[i].secret_ff_a = app_malloc(MAX_FFDH_SIZE, "FFDH secret a");
        loopargs[i].secret_ff_b = app_malloc(MAX_FFDH_SIZE, "FFDH secret b");
#endif
    }

#ifndef NO_FORK
    if (multi && do_multi(multi, size_num))
        goto show_res;
#endif

    for (i = 0; i < loopargs_len; ++i) {
        if (domlock) {
#if defined(_WIN32)
            (void)VirtualLock(loopargs[i].buf_malloc, buflen);
            (void)VirtualLock(loopargs[i].buf2_malloc, buflen);
#elif defined(OPENSSL_SYS_LINUX)
            (void)mlock(loopargs[i].buf_malloc, buflen);
            (void)mlock(loopargs[i].buf_malloc, buflen);
#endif
        }
        memset(loopargs[i].buf_malloc, 0, buflen);
        memset(loopargs[i].buf2_malloc, 0, buflen);
    }

    /* Initialize the engine after the fork */
    e = setup_engine(engine_id, 0);

    /* No parameters; turn on everything. */
    if (argc == 0 && !doit[D_EVP] && !doit[D_HMAC]
        && !doit[D_EVP_CMAC] && !do_kems && !do_sigs) {
        memset(doit, 1, sizeof(doit));
        doit[D_EVP] = doit[D_EVP_CMAC] = 0;
        ERR_set_mark();
        for (i = D_MD2; i <= D_WHIRLPOOL; i++) {
            if (!have_md(names[i]))
                doit[i] = 0;
        }
        for (i = D_CBC_DES; i <= D_CBC_256_CML; i++) {
            if (!have_cipher(names[i]))
                doit[i] = 0;
        }
        if ((mac = EVP_MAC_fetch(app_get0_libctx(), "GMAC",
                                 app_get0_propq())) != NULL) {
            EVP_MAC_free(mac);
            mac = NULL;
        } else {
            doit[D_GHASH] = 0;
        }
        if ((mac = EVP_MAC_fetch(app_get0_libctx(), "HMAC",
                                 app_get0_propq())) != NULL) {
            EVP_MAC_free(mac);
            mac = NULL;
        } else {
            doit[D_HMAC] = 0;
        }
        ERR_pop_to_mark();
        memset(rsa_doit, 1, sizeof(rsa_doit));
#ifndef OPENSSL_NO_DH
        memset(ffdh_doit, 1, sizeof(ffdh_doit));
#endif
#ifndef OPENSSL_NO_DSA
        memset(dsa_doit, 1, sizeof(dsa_doit));
#endif
#ifndef OPENSSL_NO_ECX
        memset(ecdsa_doit, 1, sizeof(ecdsa_doit));
        memset(ecdh_doit, 1, sizeof(ecdh_doit));
        memset(eddsa_doit, 1, sizeof(eddsa_doit));
#endif /* OPENSSL_NO_ECX */
#ifndef OPENSSL_NO_SM2
        memset(sm2_doit, 1, sizeof(sm2_doit));
#endif
        memset(kems_doit, 1, sizeof(kems_doit));
        do_kems = 1;
        memset(sigs_doit, 1, sizeof(sigs_doit));
        do_sigs = 1;
    }
    for (i = 0; i < ALGOR_NUM; i++)
        if (doit[i])
            pr_header++;

    if (usertime == 0 && !mr)
        BIO_printf(bio_err,
                   "You have chosen to measure elapsed time "
                   "instead of user CPU time.\n");

#if SIGALRM > 0
    signal(SIGALRM, alarmed);
#endif

    if (doit[D_MD2]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_MD2], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, EVP_Digest_MD2_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_MD2, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_MDC2]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_MDC2], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, EVP_Digest_MDC2_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_MDC2, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_MD4]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_MD4], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, EVP_Digest_MD4_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_MD4, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_MD5]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_MD5], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, MD5_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_MD5, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_SHA1]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_SHA1], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, SHA1_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_SHA1, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_SHA256]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_SHA256], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, SHA256_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_SHA256, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_SHA512]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_SHA512], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, SHA512_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_SHA512, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_WHIRLPOOL]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_WHIRLPOOL], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, WHIRLPOOL_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_WHIRLPOOL, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_RMD160]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_RMD160], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, EVP_Digest_RMD160_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_RMD160, testnum, count, d);
            if (count < 0)
                break;
        }
    }

    if (doit[D_HMAC]) {
        static const char hmac_key[] = "This is a key...";
        int len = strlen(hmac_key);
        size_t hmac_name_len = sizeof("hmac()") + strlen(evp_mac_mdname);
        OSSL_PARAM params[3];

        if (evp_mac_mdname == NULL)
            goto end;
        evp_hmac_name = app_malloc(hmac_name_len, "HMAC name");
        BIO_snprintf(evp_hmac_name, hmac_name_len, "hmac(%s)", evp_mac_mdname);
        names[D_HMAC] = evp_hmac_name;

        params[0] =
            OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                             evp_mac_mdname, 0);
        params[1] =
            OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                              (char *)hmac_key, len);
        params[2] = OSSL_PARAM_construct_end();

        if (mac_setup("HMAC", &mac, params, loopargs, loopargs_len) < 1)
            goto end;
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_HMAC], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, HMAC_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_HMAC, testnum, count, d);
            if (count < 0)
                break;
        }
        mac_teardown(&mac, loopargs, loopargs_len);
    }

    if (doit[D_CBC_DES]) {
        int st = 1;

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].ctx = init_evp_cipher_ctx("des-cbc", deskey,
                                                  sizeof(deskey) / 3);
            st = loopargs[i].ctx != NULL;
        }
        algindex = D_CBC_DES;
        for (testnum = 0; st && testnum < size_num; testnum++) {
            if (!check_block_size(loopargs[0].ctx, lengths[testnum]))
                break;
            print_message(names[D_CBC_DES], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, EVP_Cipher_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_CBC_DES, testnum, count, d);
        }
        for (i = 0; i < loopargs_len; i++)
            EVP_CIPHER_CTX_free(loopargs[i].ctx);
    }

    if (doit[D_EDE3_DES]) {
        int st = 1;

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].ctx = init_evp_cipher_ctx("des-ede3-cbc", deskey,
                                                  sizeof(deskey));
            st = loopargs[i].ctx != NULL;
        }
        algindex = D_EDE3_DES;
        for (testnum = 0; st && testnum < size_num; testnum++) {
            if (!check_block_size(loopargs[0].ctx, lengths[testnum]))
                break;
            print_message(names[D_EDE3_DES], lengths[testnum], seconds.sym);
            Time_F(START);
            count =
                run_benchmark(async_jobs, EVP_Cipher_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_EDE3_DES, testnum, count, d);
        }
        for (i = 0; i < loopargs_len; i++)
            EVP_CIPHER_CTX_free(loopargs[i].ctx);
    }

    for (k = 0; k < 3; k++) {
        algindex = D_CBC_128_AES + k;
        if (doit[algindex]) {
            int st = 1;

            keylen = 16 + k * 8;
            for (i = 0; st && i < loopargs_len; i++) {
                loopargs[i].ctx = init_evp_cipher_ctx(names[algindex],
                                                      key32, keylen);
                st = loopargs[i].ctx != NULL;
            }

            for (testnum = 0; st && testnum < size_num; testnum++) {
                if (!check_block_size(loopargs[0].ctx, lengths[testnum]))
                    break;
                print_message(names[algindex], lengths[testnum], seconds.sym);
                Time_F(START);
                count =
                    run_benchmark(async_jobs, EVP_Cipher_loop, loopargs);
                d = Time_F(STOP);
                print_result(algindex, testnum, count, d);
            }
            for (i = 0; i < loopargs_len; i++)
                EVP_CIPHER_CTX_free(loopargs[i].ctx);
        }
    }

    for (k = 0; k < 3; k++) {
        algindex = D_CBC_128_CML + k;
        if (doit[algindex]) {
            int st = 1;

            keylen = 16 + k * 8;
            for (i = 0; st && i < loopargs_len; i++) {
                loopargs[i].ctx = init_evp_cipher_ctx(names[algindex],
                                                      key32, keylen);
                st = loopargs[i].ctx != NULL;
            }

            for (testnum = 0; st && testnum < size_num; testnum++) {
                if (!check_block_size(loopargs[0].ctx, lengths[testnum]))
                    break;
                print_message(names[algindex], lengths[testnum], seconds.sym);
                Time_F(START);
                count =
                    run_benchmark(async_jobs, EVP_Cipher_loop, loopargs);
                d = Time_F(STOP);
                print_result(algindex, testnum, count, d);
            }
            for (i = 0; i < loopargs_len; i++)
                EVP_CIPHER_CTX_free(loopargs[i].ctx);
        }
    }

    for (algindex = D_RC4; algindex <= D_CBC_CAST; algindex++) {
        if (doit[algindex]) {
            int st = 1;

            keylen = 16;
            for (i = 0; st && i < loopargs_len; i++) {
                loopargs[i].ctx = init_evp_cipher_ctx(names[algindex],
                                                      key32, keylen);
                st = loopargs[i].ctx != NULL;
            }

            for (testnum = 0; st && testnum < size_num; testnum++) {
                if (!check_block_size(loopargs[0].ctx, lengths[testnum]))
                    break;
                print_message(names[algindex], lengths[testnum], seconds.sym);
                Time_F(START);
                count =
                    run_benchmark(async_jobs, EVP_Cipher_loop, loopargs);
                d = Time_F(STOP);
                print_result(algindex, testnum, count, d);
            }
            for (i = 0; i < loopargs_len; i++)
                EVP_CIPHER_CTX_free(loopargs[i].ctx);
        }
    }
    if (doit[D_GHASH]) {
        static const char gmac_iv[] = "0123456789ab";
        OSSL_PARAM params[4];

        params[0] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_CIPHER,
                                                     "aes-128-gcm", 0);
        params[1] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_IV,
                                                      (char *)gmac_iv,
                                                      sizeof(gmac_iv) - 1);
        params[2] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                      (void *)key32, 16);
        params[3] = OSSL_PARAM_construct_end();

        if (mac_setup("GMAC", &mac, params, loopargs, loopargs_len) < 1)
            goto end;
        /* b/c of the definition of GHASH_loop(), init() calls are needed here */
        for (i = 0; i < loopargs_len; i++) {
            if (!EVP_MAC_init(loopargs[i].mctx, NULL, 0, NULL))
                goto end;
        }
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_GHASH], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, GHASH_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_GHASH, testnum, count, d);
            if (count < 0)
                break;
        }
        mac_teardown(&mac, loopargs, loopargs_len);
    }

    if (doit[D_RAND]) {
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_RAND], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, RAND_bytes_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_RAND, testnum, count, d);
        }
    }

    /*-
     * There are three scenarios for D_EVP:
     * 1- Using authenticated encryption (AE) e.g. CCM, GCM, OCB etc.
     * 2- Using AE + associated data (AD) i.e. AEAD using CCM, GCM, OCB etc.
     * 3- Not using AE or AD e.g. ECB, CBC, CFB etc.
     */
    if (doit[D_EVP]) {
        if (evp_cipher != NULL) {
            int (*loopfunc) (void *);
            int outlen = 0;
            unsigned int ae_mode = 0;

            if (multiblock && (EVP_CIPHER_get_flags(evp_cipher)
                               & EVP_CIPH_FLAG_TLS1_1_MULTIBLOCK)) {
                multiblock_speed(evp_cipher, lengths_single, &seconds);
                ret = 0;
                goto end;
            }

            names[D_EVP] = EVP_CIPHER_get0_name(evp_cipher);

            mode_op = EVP_CIPHER_get_mode(evp_cipher);

            if (aead) {
                if (lengths == lengths_list) {
                    lengths = aead_lengths_list;
                    size_num = OSSL_NELEM(aead_lengths_list);
                }
            }
            if (mode_op == EVP_CIPH_GCM_MODE
                || mode_op == EVP_CIPH_CCM_MODE
                || mode_op == EVP_CIPH_OCB_MODE
                || mode_op == EVP_CIPH_SIV_MODE
                || mode_op == EVP_CIPH_GCM_SIV_MODE) {
                ae_mode = 1;
                if (decrypt)
                    loopfunc = EVP_Update_loop_aead_dec;
                else
                    loopfunc = EVP_Update_loop_aead_enc;
            } else {
                loopfunc = EVP_Update_loop;
            }

            for (testnum = 0; testnum < size_num; testnum++) {
                print_message(names[D_EVP], lengths[testnum], seconds.sym);

                for (k = 0; k < loopargs_len; k++) {
                    loopargs[k].ctx = EVP_CIPHER_CTX_new();
                    if (loopargs[k].ctx == NULL) {
                        BIO_printf(bio_err, "\nEVP_CIPHER_CTX_new failure\n");
                        exit(1);
                    }

                    /*
                     * For AE modes, we must first encrypt the data to get
                     * a valid tag that enables us to decrypt. If we don't
                     * encrypt first, we won't have a valid tag that enables
                     * authenticity and hence decryption will fail.
                     */
                    if (!EVP_CipherInit_ex(loopargs[k].ctx, evp_cipher, NULL,
                                           NULL, NULL, ae_mode ? 1 : !decrypt)) {
                        BIO_printf(bio_err, "\nCouldn't init the context\n");
                        dofail();
                        exit(1);
                    }

                    /* Padding isn't needed */
                    EVP_CIPHER_CTX_set_padding(loopargs[k].ctx, 0);

                    keylen = EVP_CIPHER_CTX_get_key_length(loopargs[k].ctx);
                    loopargs[k].key = app_malloc(keylen, "evp_cipher key");
                    EVP_CIPHER_CTX_rand_key(loopargs[k].ctx, loopargs[k].key);

                    if (!ae_mode) {
                        if (!EVP_CipherInit_ex(loopargs[k].ctx, NULL, NULL,
                                               loopargs[k].key, iv, -1)) {
                            BIO_printf(bio_err, "\nFailed to set the key\n");
                            dofail();
                            exit(1);
                        }
                    } else if (mode_op == EVP_CIPH_SIV_MODE
                               || mode_op == EVP_CIPH_GCM_SIV_MODE) {
                        EVP_CIPHER_CTX_ctrl(loopargs[k].ctx,
                                            EVP_CTRL_SET_SPEED, 1, NULL);
                    }
                    if (ae_mode && decrypt) {
                        /* Set length of iv (Doesn't apply to SIV mode) */
                        if (mode_op != EVP_CIPH_SIV_MODE) {
                            if (!EVP_CIPHER_CTX_ctrl(loopargs[k].ctx,
                                                     EVP_CTRL_AEAD_SET_IVLEN,
                                                     sizeof(aead_iv), NULL)) {
                                BIO_printf(bio_err, "\nFailed to set iv length\n");
                                dofail();
                                exit(1);
                            }
                        }
                        /* Set tag_len (Not for GCM/SIV at encryption stage) */
                        if (mode_op != EVP_CIPH_GCM_MODE
                            && mode_op != EVP_CIPH_SIV_MODE
                            && mode_op != EVP_CIPH_GCM_SIV_MODE) {
                            if (!EVP_CIPHER_CTX_ctrl(loopargs[k].ctx,
                                                     EVP_CTRL_AEAD_SET_TAG,
                                                     TAG_LEN, NULL)) {
                                BIO_printf(bio_err,
                                           "\nFailed to set tag length\n");
                                dofail();
                                exit(1);
                            }
                        }
                        if (!EVP_CipherInit_ex(loopargs[k].ctx, NULL, NULL,
                                               loopargs[k].key, aead_iv, -1)) {
                            BIO_printf(bio_err, "\nFailed to set the key\n");
                            dofail();
                            exit(1);
                        }
                        /* Set total length of input. Only required for CCM */
                        if (mode_op == EVP_CIPH_CCM_MODE) {
                            if (!EVP_EncryptUpdate(loopargs[k].ctx, NULL,
                                                   &outlen, NULL,
                                                   lengths[testnum])) {
                                BIO_printf(bio_err,
                                           "\nCouldn't set input text length\n");
                                dofail();
                                exit(1);
                            }
                        }
                        if (aead) {
                            if (!EVP_EncryptUpdate(loopargs[k].ctx, NULL,
                                                   &outlen, aad, sizeof(aad))) {
                                BIO_printf(bio_err,
                                           "\nCouldn't insert AAD when encrypting\n");
                                dofail();
                                exit(1);
                            }
                        }
                        if (!EVP_EncryptUpdate(loopargs[k].ctx, loopargs[k].buf,
                                               &outlen, loopargs[k].buf,
                                               lengths[testnum])) {
                            BIO_printf(bio_err,
                                       "\nFailed to to encrypt the data\n");
                            dofail();
                            exit(1);
                        }

                        if (!EVP_EncryptFinal_ex(loopargs[k].ctx,
                                                 loopargs[k].buf, &outlen)) {
                            BIO_printf(bio_err,
                                       "\nFailed finalize the encryption\n");
                            dofail();
                            exit(1);
                        }

                        if (!EVP_CIPHER_CTX_ctrl(loopargs[k].ctx, EVP_CTRL_AEAD_GET_TAG,
                                                 TAG_LEN, &loopargs[k].tag)) {
                            BIO_printf(bio_err, "\nFailed to get the tag\n");
                            dofail();
                            exit(1);
                        }

                        EVP_CIPHER_CTX_free(loopargs[k].ctx);
                        loopargs[k].ctx = EVP_CIPHER_CTX_new();
                        if (loopargs[k].ctx == NULL) {
                            BIO_printf(bio_err,
                                       "\nEVP_CIPHER_CTX_new failure\n");
                            exit(1);
                        }
                        if (!EVP_CipherInit_ex(loopargs[k].ctx, evp_cipher,
                                               NULL, NULL, NULL, 0)) {
                            BIO_printf(bio_err,
                                       "\nFailed initializing the context\n");
                            dofail();
                            exit(1);
                        }

                        EVP_CIPHER_CTX_set_padding(loopargs[k].ctx, 0);

                        /* GCM-SIV/SIV only allows for a single Update operation */
                        if (mode_op == EVP_CIPH_SIV_MODE
                            || mode_op == EVP_CIPH_GCM_SIV_MODE)
                            EVP_CIPHER_CTX_ctrl(loopargs[k].ctx,
                                                EVP_CTRL_SET_SPEED, 1, NULL);
                    }
                }

                Time_F(START);
                count = run_benchmark(async_jobs, loopfunc, loopargs);
                d = Time_F(STOP);
                for (k = 0; k < loopargs_len; k++) {
                    OPENSSL_clear_free(loopargs[k].key, keylen);
                    EVP_CIPHER_CTX_free(loopargs[k].ctx);
                }
                print_result(D_EVP, testnum, count, d);
            }
        } else if (evp_md_name != NULL) {
            names[D_EVP] = evp_md_name;

            for (testnum = 0; testnum < size_num; testnum++) {
                print_message(names[D_EVP], lengths[testnum], seconds.sym);
                Time_F(START);
                count = run_benchmark(async_jobs, EVP_Digest_md_loop, loopargs);
                d = Time_F(STOP);
                print_result(D_EVP, testnum, count, d);
                if (count < 0)
                    break;
            }
        }
    }

    if (doit[D_EVP_CMAC]) {
        size_t len = sizeof("cmac()") + strlen(evp_mac_ciphername);
        OSSL_PARAM params[3];
        EVP_CIPHER *cipher = NULL;

        if (!opt_cipher(evp_mac_ciphername, &cipher))
            goto end;

        keylen = EVP_CIPHER_get_key_length(cipher);
        EVP_CIPHER_free(cipher);
        if (keylen <= 0 || keylen > (int)sizeof(key32)) {
            BIO_printf(bio_err, "\nRequested CMAC cipher with unsupported key length.\n");
            goto end;
        }
        evp_cmac_name = app_malloc(len, "CMAC name");
        BIO_snprintf(evp_cmac_name, len, "cmac(%s)", evp_mac_ciphername);
        names[D_EVP_CMAC] = evp_cmac_name;

        params[0] = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_CIPHER,
                                                     evp_mac_ciphername, 0);
        params[1] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                      (char *)key32, keylen);
        params[2] = OSSL_PARAM_construct_end();

        if (mac_setup("CMAC", &mac, params, loopargs, loopargs_len) < 1)
            goto end;
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_EVP_CMAC], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, CMAC_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_EVP_CMAC, testnum, count, d);
            if (count < 0)
                break;
        }
        mac_teardown(&mac, loopargs, loopargs_len);
    }

    if (doit[D_KMAC128]) {
        OSSL_PARAM params[2];

        params[0] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                      (void *)key32, 16);
        params[1] = OSSL_PARAM_construct_end();

        if (mac_setup("KMAC-128", &mac, params, loopargs, loopargs_len) < 1)
            goto end;
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_KMAC128], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, KMAC128_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_KMAC128, testnum, count, d);
            if (count < 0)
                break;
        }
        mac_teardown(&mac, loopargs, loopargs_len);
    }

    if (doit[D_KMAC256]) {
        OSSL_PARAM params[2];

        params[0] = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                      (void *)key32, 32);
        params[1] = OSSL_PARAM_construct_end();

        if (mac_setup("KMAC-256", &mac, params, loopargs, loopargs_len) < 1)
            goto end;
        for (testnum = 0; testnum < size_num; testnum++) {
            print_message(names[D_KMAC256], lengths[testnum], seconds.sym);
            Time_F(START);
            count = run_benchmark(async_jobs, KMAC256_loop, loopargs);
            d = Time_F(STOP);
            print_result(D_KMAC256, testnum, count, d);
            if (count < 0)
                break;
        }
        mac_teardown(&mac, loopargs, loopargs_len);
    }

    for (i = 0; i < loopargs_len; i++)
        if (RAND_bytes(loopargs[i].buf, 36) <= 0)
            goto end;

    for (testnum = 0; testnum < RSA_NUM; testnum++) {
        EVP_PKEY *rsa_key = NULL;
        int st = 0;

        if (!rsa_doit[testnum])
            continue;

        if (primes > RSA_DEFAULT_PRIME_NUM) {
            /* we haven't set keys yet,  generate multi-prime RSA keys */
            bn = BN_new();
            st = bn != NULL
                && BN_set_word(bn, RSA_F4)
                && init_gen_str(&genctx, "RSA", NULL, 0, NULL, NULL)
                && EVP_PKEY_CTX_set_rsa_keygen_bits(genctx, rsa_keys[testnum].bits) > 0
                && EVP_PKEY_CTX_set1_rsa_keygen_pubexp(genctx, bn) > 0
                && EVP_PKEY_CTX_set_rsa_keygen_primes(genctx, primes) > 0
                && EVP_PKEY_keygen(genctx, &rsa_key) > 0;
            BN_free(bn);
            bn = NULL;
            EVP_PKEY_CTX_free(genctx);
            genctx = NULL;
        } else {
            const unsigned char *p = rsa_keys[testnum].data;

            st = (rsa_key = d2i_PrivateKey(EVP_PKEY_RSA, NULL, &p,
                                           rsa_keys[testnum].length)) != NULL;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].rsa_sign_ctx[testnum] = EVP_PKEY_CTX_new(rsa_key, NULL);
            loopargs[i].sigsize = loopargs[i].buflen;
            if (loopargs[i].rsa_sign_ctx[testnum] == NULL
                || EVP_PKEY_sign_init(loopargs[i].rsa_sign_ctx[testnum]) <= 0
                || EVP_PKEY_sign(loopargs[i].rsa_sign_ctx[testnum],
                                 loopargs[i].buf2,
                                 &loopargs[i].sigsize,
                                 loopargs[i].buf, 36) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "RSA sign setup failure.  No RSA sign will be done.\n");
            dofail();
            op_count = 1;
        } else {
            pkey_print_message("private", "rsa sign",
                               rsa_keys[testnum].bits, seconds.rsa);
            /* RSA_blinding_on(rsa_key[testnum],NULL); */
            Time_F(START);
            count = run_benchmark(async_jobs, RSA_sign_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R1:%ld:%d:%.2f\n"
                       : "%ld %u bits private RSA sign ops in %.2fs\n",
                       count, rsa_keys[testnum].bits, d);
            rsa_results[testnum][0] = (double)count / d;
            op_count = count;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].rsa_verify_ctx[testnum] = EVP_PKEY_CTX_new(rsa_key,
                                                                   NULL);
            if (loopargs[i].rsa_verify_ctx[testnum] == NULL
                || EVP_PKEY_verify_init(loopargs[i].rsa_verify_ctx[testnum]) <= 0
                || EVP_PKEY_verify(loopargs[i].rsa_verify_ctx[testnum],
                                   loopargs[i].buf2,
                                   loopargs[i].sigsize,
                                   loopargs[i].buf, 36) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "RSA verify setup failure.  No RSA verify will be done.\n");
            dofail();
            rsa_doit[testnum] = 0;
        } else {
            pkey_print_message("public", "rsa verify",
                               rsa_keys[testnum].bits, seconds.rsa);
            Time_F(START);
            count = run_benchmark(async_jobs, RSA_verify_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R2:%ld:%d:%.2f\n"
                       : "%ld %u bits public RSA verify ops in %.2fs\n",
                       count, rsa_keys[testnum].bits, d);
            rsa_results[testnum][1] = (double)count / d;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].rsa_encrypt_ctx[testnum] = EVP_PKEY_CTX_new(rsa_key, NULL);
            loopargs[i].encsize = loopargs[i].buflen;
            if (loopargs[i].rsa_encrypt_ctx[testnum] == NULL
                || EVP_PKEY_encrypt_init(loopargs[i].rsa_encrypt_ctx[testnum]) <= 0
                || EVP_PKEY_encrypt(loopargs[i].rsa_encrypt_ctx[testnum],
                                    loopargs[i].buf2,
                                    &loopargs[i].encsize,
                                    loopargs[i].buf, 36) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "RSA encrypt setup failure.  No RSA encrypt will be done.\n");
            dofail();
            op_count = 1;
        } else {
            pkey_print_message("public", "rsa encrypt",
                               rsa_keys[testnum].bits, seconds.rsa);
            /* RSA_blinding_on(rsa_key[testnum],NULL); */
            Time_F(START);
            count = run_benchmark(async_jobs, RSA_encrypt_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R3:%ld:%d:%.2f\n"
                       : "%ld %u bits public RSA encrypt ops in %.2fs\n",
                       count, rsa_keys[testnum].bits, d);
            rsa_results[testnum][2] = (double)count / d;
            op_count = count;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].rsa_decrypt_ctx[testnum] = EVP_PKEY_CTX_new(rsa_key, NULL);
            declen = loopargs[i].buflen;
            if (loopargs[i].rsa_decrypt_ctx[testnum] == NULL
                || EVP_PKEY_decrypt_init(loopargs[i].rsa_decrypt_ctx[testnum]) <= 0
                || EVP_PKEY_decrypt(loopargs[i].rsa_decrypt_ctx[testnum],
                                    loopargs[i].buf,
                                    &declen,
                                    loopargs[i].buf2,
                                    loopargs[i].encsize) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "RSA decrypt setup failure.  No RSA decrypt will be done.\n");
            dofail();
            op_count = 1;
        } else {
            pkey_print_message("private", "rsa decrypt",
                               rsa_keys[testnum].bits, seconds.rsa);
            /* RSA_blinding_on(rsa_key[testnum],NULL); */
            Time_F(START);
            count = run_benchmark(async_jobs, RSA_decrypt_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R4:%ld:%d:%.2f\n"
                       : "%ld %u bits private RSA decrypt ops in %.2fs\n",
                       count, rsa_keys[testnum].bits, d);
            rsa_results[testnum][3] = (double)count / d;
            op_count = count;
        }

        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(rsa_doit, testnum);
        }
        EVP_PKEY_free(rsa_key);
    }

#ifndef OPENSSL_NO_DSA
    for (testnum = 0; testnum < DSA_NUM; testnum++) {
        EVP_PKEY *dsa_key = NULL;
        int st;

        if (!dsa_doit[testnum])
            continue;

        st = (dsa_key = get_dsa(dsa_bits[testnum])) != NULL;

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].dsa_sign_ctx[testnum] = EVP_PKEY_CTX_new(dsa_key,
                                                                 NULL);
            loopargs[i].sigsize = loopargs[i].buflen;
            if (loopargs[i].dsa_sign_ctx[testnum] == NULL
                || EVP_PKEY_sign_init(loopargs[i].dsa_sign_ctx[testnum]) <= 0
                || EVP_PKEY_sign(loopargs[i].dsa_sign_ctx[testnum],
                                 loopargs[i].buf2,
                                 &loopargs[i].sigsize,
                                 loopargs[i].buf, 20) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "DSA sign setup failure.  No DSA sign will be done.\n");
            dofail();
            op_count = 1;
        } else {
            pkey_print_message("sign", "dsa",
                               dsa_bits[testnum], seconds.dsa);
            Time_F(START);
            count = run_benchmark(async_jobs, DSA_sign_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R5:%ld:%u:%.2f\n"
                       : "%ld %u bits DSA sign ops in %.2fs\n",
                       count, dsa_bits[testnum], d);
            dsa_results[testnum][0] = (double)count / d;
            op_count = count;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].dsa_verify_ctx[testnum] = EVP_PKEY_CTX_new(dsa_key,
                                                                   NULL);
            if (loopargs[i].dsa_verify_ctx[testnum] == NULL
                || EVP_PKEY_verify_init(loopargs[i].dsa_verify_ctx[testnum]) <= 0
                || EVP_PKEY_verify(loopargs[i].dsa_verify_ctx[testnum],
                                   loopargs[i].buf2,
                                   loopargs[i].sigsize,
                                   loopargs[i].buf, 36) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "DSA verify setup failure.  No DSA verify will be done.\n");
            dofail();
            dsa_doit[testnum] = 0;
        } else {
            pkey_print_message("verify", "dsa",
                               dsa_bits[testnum], seconds.dsa);
            Time_F(START);
            count = run_benchmark(async_jobs, DSA_verify_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R6:%ld:%u:%.2f\n"
                       : "%ld %u bits DSA verify ops in %.2fs\n",
                       count, dsa_bits[testnum], d);
            dsa_results[testnum][1] = (double)count / d;
        }

        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(dsa_doit, testnum);
        }
        EVP_PKEY_free(dsa_key);
    }
#endif /* OPENSSL_NO_DSA */

    for (testnum = 0; testnum < ECDSA_NUM; testnum++) {
        EVP_PKEY *ecdsa_key = NULL;
        int st;

        if (!ecdsa_doit[testnum])
            continue;

        st = (ecdsa_key = get_ecdsa(&ec_curves[testnum])) != NULL;

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].ecdsa_sign_ctx[testnum] = EVP_PKEY_CTX_new(ecdsa_key,
                                                                   NULL);
            loopargs[i].sigsize = loopargs[i].buflen;
            if (loopargs[i].ecdsa_sign_ctx[testnum] == NULL
                || EVP_PKEY_sign_init(loopargs[i].ecdsa_sign_ctx[testnum]) <= 0
                || EVP_PKEY_sign(loopargs[i].ecdsa_sign_ctx[testnum],
                                 loopargs[i].buf2,
                                 &loopargs[i].sigsize,
                                 loopargs[i].buf, 20) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "ECDSA sign setup failure.  No ECDSA sign will be done.\n");
            dofail();
            op_count = 1;
        } else {
            pkey_print_message("sign", "ecdsa",
                               ec_curves[testnum].bits, seconds.ecdsa);
            Time_F(START);
            count = run_benchmark(async_jobs, ECDSA_sign_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R7:%ld:%u:%.2f\n"
                       : "%ld %u bits ECDSA sign ops in %.2fs\n",
                       count, ec_curves[testnum].bits, d);
            ecdsa_results[testnum][0] = (double)count / d;
            op_count = count;
        }

        for (i = 0; st && i < loopargs_len; i++) {
            loopargs[i].ecdsa_verify_ctx[testnum] = EVP_PKEY_CTX_new(ecdsa_key,
                                                                     NULL);
            if (loopargs[i].ecdsa_verify_ctx[testnum] == NULL
                || EVP_PKEY_verify_init(loopargs[i].ecdsa_verify_ctx[testnum]) <= 0
                || EVP_PKEY_verify(loopargs[i].ecdsa_verify_ctx[testnum],
                                   loopargs[i].buf2,
                                   loopargs[i].sigsize,
                                   loopargs[i].buf, 20) <= 0)
                st = 0;
        }
        if (!st) {
            BIO_printf(bio_err,
                       "ECDSA verify setup failure.  No ECDSA verify will be done.\n");
            dofail();
            ecdsa_doit[testnum] = 0;
        } else {
            pkey_print_message("verify", "ecdsa",
                               ec_curves[testnum].bits, seconds.ecdsa);
            Time_F(START);
            count = run_benchmark(async_jobs, ECDSA_verify_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R8:%ld:%u:%.2f\n"
                       : "%ld %u bits ECDSA verify ops in %.2fs\n",
                       count, ec_curves[testnum].bits, d);
            ecdsa_results[testnum][1] = (double)count / d;
        }

        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(ecdsa_doit, testnum);
        }
        EVP_PKEY_free(ecdsa_key);
    }

    for (testnum = 0; testnum < EC_NUM; testnum++) {
        int ecdh_checks = 1;

        if (!ecdh_doit[testnum])
            continue;

        for (i = 0; i < loopargs_len; i++) {
            EVP_PKEY_CTX *test_ctx = NULL;
            EVP_PKEY_CTX *ctx = NULL;
            EVP_PKEY *key_A = NULL;
            EVP_PKEY *key_B = NULL;
            size_t outlen;
            size_t test_outlen;

            if ((key_A = get_ecdsa(&ec_curves[testnum])) == NULL /* generate secret key A */
                || (key_B = get_ecdsa(&ec_curves[testnum])) == NULL /* generate secret key B */
                || (ctx = EVP_PKEY_CTX_new(key_A, NULL)) == NULL /* derivation ctx from skeyA */
                || EVP_PKEY_derive_init(ctx) <= 0 /* init derivation ctx */
                || EVP_PKEY_derive_set_peer(ctx, key_B) <= 0 /* set peer pubkey in ctx */
                || EVP_PKEY_derive(ctx, NULL, &outlen) <= 0 /* determine max length */
                || outlen == 0 /* ensure outlen is a valid size */
                || outlen > MAX_ECDH_SIZE /* avoid buffer overflow */) {
                ecdh_checks = 0;
                BIO_printf(bio_err, "ECDH key generation failure.\n");
                dofail();
                op_count = 1;
                break;
            }

            /*
             * Here we perform a test run, comparing the output of a*B and b*A;
             * we try this here and assume that further EVP_PKEY_derive calls
             * never fail, so we can skip checks in the actually benchmarked
             * code, for maximum performance.
             */
            if ((test_ctx = EVP_PKEY_CTX_new(key_B, NULL)) == NULL /* test ctx from skeyB */
                || EVP_PKEY_derive_init(test_ctx) <= 0 /* init derivation test_ctx */
                || EVP_PKEY_derive_set_peer(test_ctx, key_A) <= 0 /* set peer pubkey in test_ctx */
                || EVP_PKEY_derive(test_ctx, NULL, &test_outlen) <= 0 /* determine max length */
                || EVP_PKEY_derive(ctx, loopargs[i].secret_a, &outlen) <= 0 /* compute a*B */
                || EVP_PKEY_derive(test_ctx, loopargs[i].secret_b, &test_outlen) <= 0 /* compute b*A */
                || test_outlen != outlen /* compare output length */) {
                ecdh_checks = 0;
                BIO_printf(bio_err, "ECDH computation failure.\n");
                dofail();
                op_count = 1;
                break;
            }

            /* Compare the computation results: CRYPTO_memcmp() returns 0 if equal */
            if (CRYPTO_memcmp(loopargs[i].secret_a,
                              loopargs[i].secret_b, outlen)) {
                ecdh_checks = 0;
                BIO_printf(bio_err, "ECDH computations don't match.\n");
                dofail();
                op_count = 1;
                break;
            }

            loopargs[i].ecdh_ctx[testnum] = ctx;
            loopargs[i].outlen[testnum] = outlen;

            EVP_PKEY_free(key_A);
            EVP_PKEY_free(key_B);
            EVP_PKEY_CTX_free(test_ctx);
            test_ctx = NULL;
        }
        if (ecdh_checks != 0) {
            pkey_print_message("", "ecdh",
                               ec_curves[testnum].bits, seconds.ecdh);
            Time_F(START);
            count =
                run_benchmark(async_jobs, ECDH_EVP_derive_key_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R9:%ld:%d:%.2f\n" :
                       "%ld %u-bits ECDH ops in %.2fs\n", count,
                       ec_curves[testnum].bits, d);
            ecdh_results[testnum][0] = (double)count / d;
            op_count = count;
        }

        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(ecdh_doit, testnum);
        }
    }

#ifndef OPENSSL_NO_ECX
    for (testnum = 0; testnum < EdDSA_NUM; testnum++) {
        int st = 1;
        EVP_PKEY *ed_pkey = NULL;
        EVP_PKEY_CTX *ed_pctx = NULL;

        if (!eddsa_doit[testnum])
            continue;           /* Ignore Curve */
        for (i = 0; i < loopargs_len; i++) {
            loopargs[i].eddsa_ctx[testnum] = EVP_MD_CTX_new();
            if (loopargs[i].eddsa_ctx[testnum] == NULL) {
                st = 0;
                break;
            }
            loopargs[i].eddsa_ctx2[testnum] = EVP_MD_CTX_new();
            if (loopargs[i].eddsa_ctx2[testnum] == NULL) {
                st = 0;
                break;
            }

            if ((ed_pctx = EVP_PKEY_CTX_new_id(ed_curves[testnum].nid,
                                               NULL)) == NULL
                || EVP_PKEY_keygen_init(ed_pctx) <= 0
                || EVP_PKEY_keygen(ed_pctx, &ed_pkey) <= 0) {
                st = 0;
                EVP_PKEY_CTX_free(ed_pctx);
                break;
            }
            EVP_PKEY_CTX_free(ed_pctx);

            if (!EVP_DigestSignInit(loopargs[i].eddsa_ctx[testnum], NULL, NULL,
                                    NULL, ed_pkey)) {
                st = 0;
                EVP_PKEY_free(ed_pkey);
                break;
            }
            if (!EVP_DigestVerifyInit(loopargs[i].eddsa_ctx2[testnum], NULL,
                                      NULL, NULL, ed_pkey)) {
                st = 0;
                EVP_PKEY_free(ed_pkey);
                break;
            }

            EVP_PKEY_free(ed_pkey);
            ed_pkey = NULL;
        }
        if (st == 0) {
            BIO_printf(bio_err, "EdDSA failure.\n");
            dofail();
            op_count = 1;
        } else {
            for (i = 0; i < loopargs_len; i++) {
                /* Perform EdDSA signature test */
                loopargs[i].sigsize = ed_curves[testnum].sigsize;
                st = EVP_DigestSign(loopargs[i].eddsa_ctx[testnum],
                                    loopargs[i].buf2, &loopargs[i].sigsize,
                                    loopargs[i].buf, 20);
                if (st == 0)
                    break;
            }
            if (st == 0) {
                BIO_printf(bio_err,
                           "EdDSA sign failure.  No EdDSA sign will be done.\n");
                dofail();
                op_count = 1;
            } else {
                pkey_print_message("sign", ed_curves[testnum].name,
                                   ed_curves[testnum].bits, seconds.eddsa);
                Time_F(START);
                count = run_benchmark(async_jobs, EdDSA_sign_loop, loopargs);
                d = Time_F(STOP);

                BIO_printf(bio_err,
                           mr ? "+R10:%ld:%u:%s:%.2f\n" :
                           "%ld %u bits %s sign ops in %.2fs \n",
                           count, ed_curves[testnum].bits,
                           ed_curves[testnum].name, d);
                eddsa_results[testnum][0] = (double)count / d;
                op_count = count;
            }
            /* Perform EdDSA verification test */
            for (i = 0; i < loopargs_len; i++) {
                st = EVP_DigestVerify(loopargs[i].eddsa_ctx2[testnum],
                                      loopargs[i].buf2, loopargs[i].sigsize,
                                      loopargs[i].buf, 20);
                if (st != 1)
                    break;
            }
            if (st != 1) {
                BIO_printf(bio_err,
                           "EdDSA verify failure.  No EdDSA verify will be done.\n");
                dofail();
                eddsa_doit[testnum] = 0;
            } else {
                pkey_print_message("verify", ed_curves[testnum].name,
                                   ed_curves[testnum].bits, seconds.eddsa);
                Time_F(START);
                count = run_benchmark(async_jobs, EdDSA_verify_loop, loopargs);
                d = Time_F(STOP);
                BIO_printf(bio_err,
                           mr ? "+R11:%ld:%u:%s:%.2f\n"
                           : "%ld %u bits %s verify ops in %.2fs\n",
                           count, ed_curves[testnum].bits,
                           ed_curves[testnum].name, d);
                eddsa_results[testnum][1] = (double)count / d;
            }

            if (op_count <= 1) {
                /* if longer than 10s, don't do any more */
                stop_it(eddsa_doit, testnum);
            }
        }
    }
#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_SM2
    for (testnum = 0; testnum < SM2_NUM; testnum++) {
        int st = 1;
        EVP_PKEY *sm2_pkey = NULL;

        if (!sm2_doit[testnum])
            continue;           /* Ignore Curve */
        /* Init signing and verification */
        for (i = 0; i < loopargs_len; i++) {
            EVP_PKEY_CTX *sm2_pctx = NULL;
            EVP_PKEY_CTX *sm2_vfy_pctx = NULL;
            EVP_PKEY_CTX *pctx = NULL;
            st = 0;

            loopargs[i].sm2_ctx[testnum] = EVP_MD_CTX_new();
            loopargs[i].sm2_vfy_ctx[testnum] = EVP_MD_CTX_new();
            if (loopargs[i].sm2_ctx[testnum] == NULL
                    || loopargs[i].sm2_vfy_ctx[testnum] == NULL)
                break;

            sm2_pkey = NULL;

            st = !((pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_SM2, NULL)) == NULL
                || EVP_PKEY_keygen_init(pctx) <= 0
                || EVP_PKEY_CTX_set_ec_paramgen_curve_nid(pctx,
                    sm2_curves[testnum].nid) <= 0
                || EVP_PKEY_keygen(pctx, &sm2_pkey) <= 0);
            EVP_PKEY_CTX_free(pctx);
            if (st == 0)
                break;

            st = 0; /* set back to zero */
            /* attach it sooner to rely on main final cleanup */
            loopargs[i].sm2_pkey[testnum] = sm2_pkey;
            loopargs[i].sigsize = EVP_PKEY_get_size(sm2_pkey);

            sm2_pctx = EVP_PKEY_CTX_new(sm2_pkey, NULL);
            sm2_vfy_pctx = EVP_PKEY_CTX_new(sm2_pkey, NULL);
            if (sm2_pctx == NULL || sm2_vfy_pctx == NULL) {
                EVP_PKEY_CTX_free(sm2_vfy_pctx);
                break;
            }

            /* attach them directly to respective ctx */
            EVP_MD_CTX_set_pkey_ctx(loopargs[i].sm2_ctx[testnum], sm2_pctx);
            EVP_MD_CTX_set_pkey_ctx(loopargs[i].sm2_vfy_ctx[testnum], sm2_vfy_pctx);

            /*
             * No need to allow user to set an explicit ID here, just use
             * the one defined in the 'draft-yang-tls-tl13-sm-suites' I-D.
             */
            if (EVP_PKEY_CTX_set1_id(sm2_pctx, SM2_ID, SM2_ID_LEN) != 1
                || EVP_PKEY_CTX_set1_id(sm2_vfy_pctx, SM2_ID, SM2_ID_LEN) != 1)
                break;

            if (!EVP_DigestSignInit(loopargs[i].sm2_ctx[testnum], NULL,
                                    EVP_sm3(), NULL, sm2_pkey))
                break;
            if (!EVP_DigestVerifyInit(loopargs[i].sm2_vfy_ctx[testnum], NULL,
                                      EVP_sm3(), NULL, sm2_pkey))
                break;
            st = 1;         /* mark loop as succeeded */
        }
        if (st == 0) {
            BIO_printf(bio_err, "SM2 init failure.\n");
            dofail();
            op_count = 1;
        } else {
            for (i = 0; i < loopargs_len; i++) {
                /* Perform SM2 signature test */
                st = EVP_DigestSign(loopargs[i].sm2_ctx[testnum],
                                    loopargs[i].buf2, &loopargs[i].sigsize,
                                    loopargs[i].buf, 20);
                if (st == 0)
                    break;
            }
            if (st == 0) {
                BIO_printf(bio_err,
                           "SM2 sign failure.  No SM2 sign will be done.\n");
                dofail();
                op_count = 1;
            } else {
                pkey_print_message("sign", sm2_curves[testnum].name,
                                   sm2_curves[testnum].bits, seconds.sm2);
                Time_F(START);
                count = run_benchmark(async_jobs, SM2_sign_loop, loopargs);
                d = Time_F(STOP);

                BIO_printf(bio_err,
                           mr ? "+R12:%ld:%u:%s:%.2f\n" :
                           "%ld %u bits %s sign ops in %.2fs \n",
                           count, sm2_curves[testnum].bits,
                           sm2_curves[testnum].name, d);
                sm2_results[testnum][0] = (double)count / d;
                op_count = count;
            }

            /* Perform SM2 verification test */
            for (i = 0; i < loopargs_len; i++) {
                st = EVP_DigestVerify(loopargs[i].sm2_vfy_ctx[testnum],
                                      loopargs[i].buf2, loopargs[i].sigsize,
                                      loopargs[i].buf, 20);
                if (st != 1)
                    break;
            }
            if (st != 1) {
                BIO_printf(bio_err,
                           "SM2 verify failure.  No SM2 verify will be done.\n");
                dofail();
                sm2_doit[testnum] = 0;
            } else {
                pkey_print_message("verify", sm2_curves[testnum].name,
                                   sm2_curves[testnum].bits, seconds.sm2);
                Time_F(START);
                count = run_benchmark(async_jobs, SM2_verify_loop, loopargs);
                d = Time_F(STOP);
                BIO_printf(bio_err,
                           mr ? "+R13:%ld:%u:%s:%.2f\n"
                           : "%ld %u bits %s verify ops in %.2fs\n",
                           count, sm2_curves[testnum].bits,
                           sm2_curves[testnum].name, d);
                sm2_results[testnum][1] = (double)count / d;
            }

            if (op_count <= 1) {
                /* if longer than 10s, don't do any more */
                for (testnum++; testnum < SM2_NUM; testnum++)
                    sm2_doit[testnum] = 0;
            }
        }
    }
#endif                         /* OPENSSL_NO_SM2 */

#ifndef OPENSSL_NO_DH
    for (testnum = 0; testnum < FFDH_NUM; testnum++) {
        int ffdh_checks = 1;

        if (!ffdh_doit[testnum])
            continue;

        for (i = 0; i < loopargs_len; i++) {
            EVP_PKEY *pkey_A = NULL;
            EVP_PKEY *pkey_B = NULL;
            EVP_PKEY_CTX *ffdh_ctx = NULL;
            EVP_PKEY_CTX *test_ctx = NULL;
            size_t secret_size;
            size_t test_out;

            /* Ensure that the error queue is empty */
            if (ERR_peek_error()) {
                BIO_printf(bio_err,
                           "WARNING: the error queue contains previous unhandled errors.\n");
                dofail();
            }

            pkey_A = EVP_PKEY_new();
            if (!pkey_A) {
                BIO_printf(bio_err, "Error while initialising EVP_PKEY (out of memory?).\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            pkey_B = EVP_PKEY_new();
            if (!pkey_B) {
                BIO_printf(bio_err, "Error while initialising EVP_PKEY (out of memory?).\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            ffdh_ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_DH, NULL);
            if (!ffdh_ctx) {
                BIO_printf(bio_err, "Error while allocating EVP_PKEY_CTX.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            if (EVP_PKEY_keygen_init(ffdh_ctx) <= 0) {
                BIO_printf(bio_err, "Error while initialising EVP_PKEY_CTX.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_CTX_set_dh_nid(ffdh_ctx, ffdh_params[testnum].nid) <= 0) {
                BIO_printf(bio_err, "Error setting DH key size for keygen.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            if (EVP_PKEY_keygen(ffdh_ctx, &pkey_A) <= 0 ||
                EVP_PKEY_keygen(ffdh_ctx, &pkey_B) <= 0) {
                BIO_printf(bio_err, "FFDH key generation failure.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            EVP_PKEY_CTX_free(ffdh_ctx);

            /*
             * check if the derivation works correctly both ways so that
             * we know if future derive calls will fail, and we can skip
             * error checking in benchmarked code
             */
            ffdh_ctx = EVP_PKEY_CTX_new(pkey_A, NULL);
            if (ffdh_ctx == NULL) {
                BIO_printf(bio_err, "Error while allocating EVP_PKEY_CTX.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_derive_init(ffdh_ctx) <= 0) {
                BIO_printf(bio_err, "FFDH derivation context init failure.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_derive_set_peer(ffdh_ctx, pkey_B) <= 0) {
                BIO_printf(bio_err, "Assigning peer key for derivation failed.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_derive(ffdh_ctx, NULL, &secret_size) <= 0) {
                BIO_printf(bio_err, "Checking size of shared secret failed.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (secret_size > MAX_FFDH_SIZE) {
                BIO_printf(bio_err, "Assertion failure: shared secret too large.\n");
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_derive(ffdh_ctx,
                                loopargs[i].secret_ff_a,
                                &secret_size) <= 0) {
                BIO_printf(bio_err, "Shared secret derive failure.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            /* Now check from side B */
            test_ctx = EVP_PKEY_CTX_new(pkey_B, NULL);
            if (!test_ctx) {
                BIO_printf(bio_err, "Error while allocating EVP_PKEY_CTX.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }
            if (EVP_PKEY_derive_init(test_ctx) <= 0 ||
                EVP_PKEY_derive_set_peer(test_ctx, pkey_A) <= 0 ||
                EVP_PKEY_derive(test_ctx, NULL, &test_out) <= 0 ||
                EVP_PKEY_derive(test_ctx, loopargs[i].secret_ff_b, &test_out) <= 0 ||
                test_out != secret_size) {
                BIO_printf(bio_err, "FFDH computation failure.\n");
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            /* compare the computed secrets */
            if (CRYPTO_memcmp(loopargs[i].secret_ff_a,
                              loopargs[i].secret_ff_b, secret_size)) {
                BIO_printf(bio_err, "FFDH computations don't match.\n");
                dofail();
                op_count = 1;
                ffdh_checks = 0;
                break;
            }

            loopargs[i].ffdh_ctx[testnum] = ffdh_ctx;

            EVP_PKEY_free(pkey_A);
            pkey_A = NULL;
            EVP_PKEY_free(pkey_B);
            pkey_B = NULL;
            EVP_PKEY_CTX_free(test_ctx);
            test_ctx = NULL;
        }
        if (ffdh_checks != 0) {
            pkey_print_message("", "ffdh",
                               ffdh_params[testnum].bits, seconds.ffdh);
            Time_F(START);
            count =
                run_benchmark(async_jobs, FFDH_derive_key_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R14:%ld:%d:%.2f\n" :
                       "%ld %u-bits FFDH ops in %.2fs\n", count,
                       ffdh_params[testnum].bits, d);
            ffdh_results[testnum][0] = (double)count / d;
            op_count = count;
        }
        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(ffdh_doit, testnum);
        }
    }
#endif  /* OPENSSL_NO_DH */

    for (testnum = 0; testnum < kems_algs_len; testnum++) {
        int kem_checks = 1;
        const char *kem_name = kems_algname[testnum];

        if (!kems_doit[testnum] || !do_kems)
            continue;

        for (i = 0; i < loopargs_len; i++) {
            EVP_PKEY *pkey = NULL;
            EVP_PKEY_CTX *kem_gen_ctx = NULL;
            EVP_PKEY_CTX *kem_encaps_ctx = NULL;
            EVP_PKEY_CTX *kem_decaps_ctx = NULL;
            size_t send_secret_len, out_len;
            size_t rcv_secret_len;
            unsigned char *out = NULL, *send_secret = NULL, *rcv_secret;
            unsigned int bits;
            char *name;
            char sfx[MAX_ALGNAME_SUFFIX];
            OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END };
            int use_params = 0;
            enum kem_type_t { KEM_RSA = 1, KEM_EC, KEM_X25519, KEM_X448 } kem_type;

            /* no string after rsa<bitcnt> permitted: */
            if (strlen(kem_name) < MAX_ALGNAME_SUFFIX + 4 /* rsa+digit */
                && sscanf(kem_name, "rsa%u%s", &bits, sfx) == 1)
                kem_type = KEM_RSA;
            else if (strncmp(kem_name, "EC", 2) == 0)
                kem_type = KEM_EC;
            else if (strcmp(kem_name, "X25519") == 0)
                kem_type = KEM_X25519;
            else if (strcmp(kem_name, "X448") == 0)
                kem_type = KEM_X448;
            else kem_type = 0;

            if (ERR_peek_error()) {
                BIO_printf(bio_err,
                           "WARNING: the error queue contains previous unhandled errors.\n");
                dofail();
            }

            if (kem_type == KEM_RSA) {
                params[0] = OSSL_PARAM_construct_uint(OSSL_PKEY_PARAM_RSA_BITS,
                                                      &bits);
                use_params = 1;
            } else if (kem_type == KEM_EC) {
                name = (char *)(kem_name + 2);
                params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                  name, 0);
                use_params = 1;
            }

            kem_gen_ctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(),
                                               (kem_type == KEM_RSA) ? "RSA":
                                                (kem_type == KEM_EC) ? "EC":
                                                 kem_name,
                                               app_get0_propq());

            if ((!kem_gen_ctx || EVP_PKEY_keygen_init(kem_gen_ctx) <= 0)
                || (use_params
                    && EVP_PKEY_CTX_set_params(kem_gen_ctx, params) <= 0)) {
                BIO_printf(bio_err, "Error initializing keygen ctx for %s.\n",
                           kem_name);
                goto kem_err_break;
            }
            if (EVP_PKEY_keygen(kem_gen_ctx, &pkey) <= 0) {
                BIO_printf(bio_err, "Error while generating KEM EVP_PKEY.\n");
                goto kem_err_break;
            }
            /* Now prepare encaps data structs */
            kem_encaps_ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(),
                                                        pkey,
                                                        app_get0_propq());
            if (kem_encaps_ctx == NULL
                || EVP_PKEY_encapsulate_init(kem_encaps_ctx, NULL) <= 0
                || (kem_type == KEM_RSA
                    && EVP_PKEY_CTX_set_kem_op(kem_encaps_ctx, "RSASVE") <= 0)
                || ((kem_type == KEM_EC
                    || kem_type == KEM_X25519
                    || kem_type == KEM_X448)
                   && EVP_PKEY_CTX_set_kem_op(kem_encaps_ctx, "DHKEM") <= 0)
                || EVP_PKEY_encapsulate(kem_encaps_ctx, NULL, &out_len,
                                      NULL, &send_secret_len) <= 0) {
                BIO_printf(bio_err,
                           "Error while initializing encaps data structs for %s.\n",
                           kem_name);
                goto kem_err_break;
            }
            out = app_malloc(out_len, "encaps result");
            send_secret = app_malloc(send_secret_len, "encaps secret");
            if (out == NULL || send_secret == NULL) {
                BIO_printf(bio_err, "MemAlloc error in encaps for %s.\n", kem_name);
                goto kem_err_break;
            }
            if (EVP_PKEY_encapsulate(kem_encaps_ctx, out, &out_len,
                                     send_secret, &send_secret_len) <= 0) {
                BIO_printf(bio_err, "Encaps error for %s.\n", kem_name);
                goto kem_err_break;
            }
            /* Now prepare decaps data structs */
            kem_decaps_ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(),
                                                        pkey,
                                                        app_get0_propq());
            if (kem_decaps_ctx == NULL
                || EVP_PKEY_decapsulate_init(kem_decaps_ctx, NULL) <= 0
                || (kem_type == KEM_RSA
                  && EVP_PKEY_CTX_set_kem_op(kem_decaps_ctx, "RSASVE") <= 0)
                || ((kem_type == KEM_EC
                     || kem_type == KEM_X25519
                     || kem_type == KEM_X448)
                  && EVP_PKEY_CTX_set_kem_op(kem_decaps_ctx, "DHKEM") <= 0)
                || EVP_PKEY_decapsulate(kem_decaps_ctx, NULL, &rcv_secret_len,
                                        out, out_len) <= 0) {
                BIO_printf(bio_err,
                           "Error while initializing decaps data structs for %s.\n",
                           kem_name);
                goto kem_err_break;
            }
            rcv_secret = app_malloc(rcv_secret_len, "KEM decaps secret");
            if (rcv_secret == NULL) {
                BIO_printf(bio_err, "MemAlloc failure in decaps for %s.\n",
                           kem_name);
                goto kem_err_break;
            }
            if (EVP_PKEY_decapsulate(kem_decaps_ctx, rcv_secret,
                                     &rcv_secret_len, out, out_len) <= 0
                || rcv_secret_len != send_secret_len
                || memcmp(send_secret, rcv_secret, send_secret_len)) {
                BIO_printf(bio_err, "Decaps error for %s.\n", kem_name);
                goto kem_err_break;
            }
            loopargs[i].kem_gen_ctx[testnum] = kem_gen_ctx;
            loopargs[i].kem_encaps_ctx[testnum] = kem_encaps_ctx;
            loopargs[i].kem_decaps_ctx[testnum] = kem_decaps_ctx;
            loopargs[i].kem_out_len[testnum] = out_len;
            loopargs[i].kem_secret_len[testnum] = send_secret_len;
            loopargs[i].kem_out[testnum] = out;
            loopargs[i].kem_send_secret[testnum] = send_secret;
            loopargs[i].kem_rcv_secret[testnum] = rcv_secret;
            EVP_PKEY_free(pkey);
            pkey = NULL;
            continue;

        kem_err_break:
            dofail();
            EVP_PKEY_free(pkey);
            op_count = 1;
            kem_checks = 0;
            break;
        }
        if (kem_checks != 0) {
            kskey_print_message(kem_name, "keygen", seconds.kem);
            Time_F(START);
            count =
                run_benchmark(async_jobs, KEM_keygen_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R15:%ld:%s:%.2f\n" :
                       "%ld %s KEM keygen ops in %.2fs\n", count,
                       kem_name, d);
            kems_results[testnum][0] = (double)count / d;
            op_count = count;
            kskey_print_message(kem_name, "encaps", seconds.kem);
            Time_F(START);
            count =
                run_benchmark(async_jobs, KEM_encaps_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R16:%ld:%s:%.2f\n" :
                       "%ld %s KEM encaps ops in %.2fs\n", count,
                       kem_name, d);
            kems_results[testnum][1] = (double)count / d;
            op_count = count;
            kskey_print_message(kem_name, "decaps", seconds.kem);
            Time_F(START);
            count =
                run_benchmark(async_jobs, KEM_decaps_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R17:%ld:%s:%.2f\n" :
                       "%ld %s KEM decaps ops in %.2fs\n", count,
                       kem_name, d);
            kems_results[testnum][2] = (double)count / d;
            op_count = count;
        }
        if (op_count <= 1) {
            /* if longer than 10s, don't do any more */
            stop_it(kems_doit, testnum);
        }
    }

    for (testnum = 0; testnum < sigs_algs_len; testnum++) {
        int sig_checks = 1;
        const char *sig_name = sigs_algname[testnum];

        if (!sigs_doit[testnum] || !do_sigs)
            continue;

        for (i = 0; i < loopargs_len; i++) {
            EVP_PKEY *pkey = NULL;
            EVP_PKEY_CTX *ctx_params = NULL;
            EVP_PKEY* pkey_params = NULL;
            EVP_PKEY_CTX *sig_gen_ctx = NULL;
            EVP_PKEY_CTX *sig_sign_ctx = NULL;
            EVP_PKEY_CTX *sig_verify_ctx = NULL;
            unsigned char md[SHA256_DIGEST_LENGTH];
            unsigned char *sig;
            char sfx[MAX_ALGNAME_SUFFIX];
            size_t md_len = SHA256_DIGEST_LENGTH;
            size_t max_sig_len, sig_len;
            unsigned int bits;
            OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END };
            int use_params = 0;

            /* only sign little data to avoid measuring digest performance */
            memset(md, 0, SHA256_DIGEST_LENGTH);

            if (ERR_peek_error()) {
                BIO_printf(bio_err,
                           "WARNING: the error queue contains previous unhandled errors.\n");
                dofail();
            }

            /* no string after rsa<bitcnt> permitted: */
            if (strlen(sig_name) < MAX_ALGNAME_SUFFIX + 4 /* rsa+digit */
                && sscanf(sig_name, "rsa%u%s", &bits, sfx) == 1) {
                params[0] = OSSL_PARAM_construct_uint(OSSL_PKEY_PARAM_RSA_BITS,
                                                      &bits);
                use_params = 1;
            }

            if (strncmp(sig_name, "dsa", 3) == 0) {
                ctx_params = EVP_PKEY_CTX_new_id(EVP_PKEY_DSA, NULL);
                if (ctx_params == NULL
                    || EVP_PKEY_paramgen_init(ctx_params) <= 0
                    || EVP_PKEY_CTX_set_dsa_paramgen_bits(ctx_params,
                                                        atoi(sig_name + 3)) <= 0
                    || EVP_PKEY_paramgen(ctx_params, &pkey_params) <= 0
                    || (sig_gen_ctx = EVP_PKEY_CTX_new(pkey_params, NULL)) == NULL
                    || EVP_PKEY_keygen_init(sig_gen_ctx) <= 0) {
                    BIO_printf(bio_err,
                               "Error initializing classic keygen ctx for %s.\n",
                               sig_name);
                    goto sig_err_break;
                }
            }

            if (sig_gen_ctx == NULL)
                sig_gen_ctx = EVP_PKEY_CTX_new_from_name(app_get0_libctx(),
                                      use_params == 1 ? "RSA" : sig_name,
                                      app_get0_propq());

            if (!sig_gen_ctx || EVP_PKEY_keygen_init(sig_gen_ctx) <= 0
                || (use_params &&
                    EVP_PKEY_CTX_set_params(sig_gen_ctx, params) <= 0)) {
                BIO_printf(bio_err, "Error initializing keygen ctx for %s.\n",
                           sig_name);
                goto sig_err_break;
            }
            if (EVP_PKEY_keygen(sig_gen_ctx, &pkey) <= 0) {
                BIO_printf(bio_err,
                           "Error while generating signature EVP_PKEY for %s.\n",
                           sig_name);
                goto sig_err_break;
            }
            /* Now prepare signature data structs */
            sig_sign_ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(),
                                                      pkey,
                                                      app_get0_propq());
            if (sig_sign_ctx == NULL
                || EVP_PKEY_sign_init(sig_sign_ctx) <= 0
                || (use_params == 1
                    && (EVP_PKEY_CTX_set_rsa_padding(sig_sign_ctx,
                                                     RSA_PKCS1_PADDING) <= 0))
                || EVP_PKEY_sign(sig_sign_ctx, NULL, &max_sig_len,
                                 md, md_len) <= 0) {
                    BIO_printf(bio_err,
                               "Error while initializing signing data structs for %s.\n",
                               sig_name);
                    goto sig_err_break;
            }
            sig = app_malloc(sig_len = max_sig_len, "signature buffer");
            if (sig == NULL) {
                BIO_printf(bio_err, "MemAlloc error in sign for %s.\n", sig_name);
                goto sig_err_break;
            }
            if (EVP_PKEY_sign(sig_sign_ctx, sig, &sig_len, md, md_len) <= 0) {
                BIO_printf(bio_err, "Signing error for %s.\n", sig_name);
                goto sig_err_break;
            }
            /* Now prepare verify data structs */
            memset(md, 0, SHA256_DIGEST_LENGTH);
            sig_verify_ctx = EVP_PKEY_CTX_new_from_pkey(app_get0_libctx(),
                                                        pkey,
                                                        app_get0_propq());
            if (sig_verify_ctx == NULL
                || EVP_PKEY_verify_init(sig_verify_ctx) <= 0
                || (use_params == 1
                  && (EVP_PKEY_CTX_set_rsa_padding(sig_verify_ctx,
                                                   RSA_PKCS1_PADDING) <= 0))) {
                BIO_printf(bio_err,
                           "Error while initializing verify data structs for %s.\n",
                           sig_name);
                goto sig_err_break;
            }
            if (EVP_PKEY_verify(sig_verify_ctx, sig, sig_len, md, md_len) <= 0) {
                BIO_printf(bio_err, "Verify error for %s.\n", sig_name);
                goto sig_err_break;
            }
            if (EVP_PKEY_verify(sig_verify_ctx, sig, sig_len, md, md_len) <= 0) {
                BIO_printf(bio_err, "Verify 2 error for %s.\n", sig_name);
                goto sig_err_break;
            }
            loopargs[i].sig_gen_ctx[testnum] = sig_gen_ctx;
            loopargs[i].sig_sign_ctx[testnum] = sig_sign_ctx;
            loopargs[i].sig_verify_ctx[testnum] = sig_verify_ctx;
            loopargs[i].sig_max_sig_len[testnum] = max_sig_len;
            loopargs[i].sig_act_sig_len[testnum] = sig_len;
            loopargs[i].sig_sig[testnum] = sig;
            EVP_PKEY_free(pkey);
            pkey = NULL;
            continue;

        sig_err_break:
            dofail();
            EVP_PKEY_free(pkey);
            op_count = 1;
            sig_checks = 0;
            break;
        }

        if (sig_checks != 0) {
            kskey_print_message(sig_name, "keygen", seconds.sig);
            Time_F(START);
            count = run_benchmark(async_jobs, SIG_keygen_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R18:%ld:%s:%.2f\n" :
                       "%ld %s signature keygen ops in %.2fs\n", count,
                       sig_name, d);
            sigs_results[testnum][0] = (double)count / d;
            op_count = count;
            kskey_print_message(sig_name, "signs", seconds.sig);
            Time_F(START);
            count =
                run_benchmark(async_jobs, SIG_sign_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R19:%ld:%s:%.2f\n" :
                       "%ld %s signature sign ops in %.2fs\n", count,
                       sig_name, d);
            sigs_results[testnum][1] = (double)count / d;
            op_count = count;

            kskey_print_message(sig_name, "verify", seconds.sig);
            Time_F(START);
            count =
                run_benchmark(async_jobs, SIG_verify_loop, loopargs);
            d = Time_F(STOP);
            BIO_printf(bio_err,
                       mr ? "+R20:%ld:%s:%.2f\n" :
                       "%ld %s signature verify ops in %.2fs\n", count,
                       sig_name, d);
            sigs_results[testnum][2] = (double)count / d;
            op_count = count;
        }
        if (op_count <= 1)
            stop_it(sigs_doit, testnum);
    }

#ifndef NO_FORK
 show_res:
#endif
    if (!mr) {
        printf("version: %s\n", OpenSSL_version(OPENSSL_FULL_VERSION_STRING));
        printf("%s\n", OpenSSL_version(OPENSSL_BUILT_ON));
        printf("options: %s\n", BN_options());
        printf("%s\n", OpenSSL_version(OPENSSL_CFLAGS));
        printf("%s\n", OpenSSL_version(OPENSSL_CPU_INFO));
    }

    if (pr_header) {
        if (mr) {
            printf("+H");
        } else {
            printf("The 'numbers' are in 1000s of bytes per second processed.\n");
            printf("type        ");
        }
        for (testnum = 0; testnum < size_num; testnum++)
            printf(mr ? ":%d" : "%7d bytes", lengths[testnum]);
        printf("\n");
    }

    for (k = 0; k < ALGOR_NUM; k++) {
        const char *alg_name = names[k];

        if (!doit[k])
            continue;

        if (k == D_EVP) {
            if (evp_cipher == NULL)
                alg_name = evp_md_name;
            else if ((alg_name = EVP_CIPHER_get0_name(evp_cipher)) == NULL)
                app_bail_out("failed to get name of cipher '%s'\n", evp_cipher);
        }

        if (mr)
            printf("+F:%u:%s", k, alg_name);
        else
            printf("%-13s", alg_name);
        for (testnum = 0; testnum < size_num; testnum++) {
            if (results[k][testnum] > 10000 && !mr)
                printf(" %11.2fk", results[k][testnum] / 1e3);
            else
                printf(mr ? ":%.2f" : " %11.2f ", results[k][testnum]);
        }
        printf("\n");
    }
    testnum = 1;
    for (k = 0; k < RSA_NUM; k++) {
        if (!rsa_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%19ssign    verify    encrypt   decrypt   sign/s verify/s  encr./s  decr./s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F2:%u:%u:%f:%f:%f:%f\n",
                   k, rsa_keys[k].bits, rsa_results[k][0], rsa_results[k][1],
                   rsa_results[k][2], rsa_results[k][3]);
        else
            printf("rsa %5u bits %8.6fs %8.6fs %8.6fs %8.6fs %8.1f %8.1f %8.1f %8.1f\n",
                   rsa_keys[k].bits, 1.0 / rsa_results[k][0],
                   1.0 / rsa_results[k][1], 1.0 / rsa_results[k][2],
                   1.0 / rsa_results[k][3],
                   rsa_results[k][0], rsa_results[k][1],
                   rsa_results[k][2], rsa_results[k][3]);
    }
    testnum = 1;
#ifndef OPENSSL_NO_DSA
    for (k = 0; k < DSA_NUM; k++) {
        if (!dsa_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%18ssign    verify    sign/s verify/s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F3:%u:%u:%f:%f\n",
                   k, dsa_bits[k], dsa_results[k][0], dsa_results[k][1]);
        else
            printf("dsa %4u bits %8.6fs %8.6fs %8.1f %8.1f\n",
                   dsa_bits[k], 1.0 / dsa_results[k][0], 1.0 / dsa_results[k][1],
                   dsa_results[k][0], dsa_results[k][1]);
    }
#endif /* OPENSSL_NO_DSA */
    testnum = 1;
    for (k = 0; k < OSSL_NELEM(ecdsa_doit); k++) {
        if (!ecdsa_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%30ssign    verify    sign/s verify/s\n", " ");
            testnum = 0;
        }

        if (mr)
            printf("+F4:%u:%u:%f:%f\n",
                   k, ec_curves[k].bits,
                   ecdsa_results[k][0], ecdsa_results[k][1]);
        else
            printf("%4u bits ecdsa (%s) %8.4fs %8.4fs %8.1f %8.1f\n",
                   ec_curves[k].bits, ec_curves[k].name,
                   1.0 / ecdsa_results[k][0], 1.0 / ecdsa_results[k][1],
                   ecdsa_results[k][0], ecdsa_results[k][1]);
    }

    testnum = 1;
    for (k = 0; k < EC_NUM; k++) {
        if (!ecdh_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%30sop      op/s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F5:%u:%u:%f:%f\n",
                   k, ec_curves[k].bits,
                   ecdh_results[k][0], 1.0 / ecdh_results[k][0]);

        else
            printf("%4u bits ecdh (%s) %8.4fs %8.1f\n",
                   ec_curves[k].bits, ec_curves[k].name,
                   1.0 / ecdh_results[k][0], ecdh_results[k][0]);
    }

#ifndef OPENSSL_NO_ECX
    testnum = 1;
    for (k = 0; k < OSSL_NELEM(eddsa_doit); k++) {
        if (!eddsa_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%30ssign    verify    sign/s verify/s\n", " ");
            testnum = 0;
        }

        if (mr)
            printf("+F6:%u:%u:%s:%f:%f\n",
                   k, ed_curves[k].bits, ed_curves[k].name,
                   eddsa_results[k][0], eddsa_results[k][1]);
        else
            printf("%4u bits EdDSA (%s) %8.4fs %8.4fs %8.1f %8.1f\n",
                   ed_curves[k].bits, ed_curves[k].name,
                   1.0 / eddsa_results[k][0], 1.0 / eddsa_results[k][1],
                   eddsa_results[k][0], eddsa_results[k][1]);
    }
#endif /* OPENSSL_NO_ECX */

#ifndef OPENSSL_NO_SM2
    testnum = 1;
    for (k = 0; k < OSSL_NELEM(sm2_doit); k++) {
        if (!sm2_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%30ssign    verify    sign/s verify/s\n", " ");
            testnum = 0;
        }

        if (mr)
            printf("+F7:%u:%u:%s:%f:%f\n",
                   k, sm2_curves[k].bits, sm2_curves[k].name,
                   sm2_results[k][0], sm2_results[k][1]);
        else
            printf("%4u bits SM2 (%s) %8.4fs %8.4fs %8.1f %8.1f\n",
                   sm2_curves[k].bits, sm2_curves[k].name,
                   1.0 / sm2_results[k][0], 1.0 / sm2_results[k][1],
                   sm2_results[k][0], sm2_results[k][1]);
    }
#endif
#ifndef OPENSSL_NO_DH
    testnum = 1;
    for (k = 0; k < FFDH_NUM; k++) {
        if (!ffdh_doit[k])
            continue;
        if (testnum && !mr) {
            printf("%23sop     op/s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F8:%u:%u:%f:%f\n",
                   k, ffdh_params[k].bits,
                   ffdh_results[k][0], 1.0 / ffdh_results[k][0]);

        else
            printf("%4u bits ffdh %8.4fs %8.1f\n",
                   ffdh_params[k].bits,
                   1.0 / ffdh_results[k][0], ffdh_results[k][0]);
    }
#endif /* OPENSSL_NO_DH */

    testnum = 1;
    for (k = 0; k < kems_algs_len; k++) {
        const char *kem_name = kems_algname[k];

        if (!kems_doit[k] || !do_kems)
            continue;
        if (testnum && !mr) {
            printf("%31skeygen    encaps    decaps keygens/s  encaps/s  decaps/s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F9:%u:%f:%f:%f\n",
                   k, kems_results[k][0], kems_results[k][1],
                   kems_results[k][2]);
        else
            printf("%27s %8.6fs %8.6fs %8.6fs %9.1f %9.1f %9.1f\n", kem_name,
                   1.0 / kems_results[k][0],
                   1.0 / kems_results[k][1], 1.0 / kems_results[k][2],
                   kems_results[k][0], kems_results[k][1], kems_results[k][2]);
    }
    ret = 0;

    testnum = 1;
    for (k = 0; k < sigs_algs_len; k++) {
        const char *sig_name = sigs_algname[k];

        if (!sigs_doit[k] || !do_sigs)
            continue;
        if (testnum && !mr) {
            printf("%31skeygen     signs    verify keygens/s    sign/s  verify/s\n", " ");
            testnum = 0;
        }
        if (mr)
            printf("+F10:%u:%f:%f:%f\n",
                   k, sigs_results[k][0], sigs_results[k][1],
                   sigs_results[k][2]);
        else
            printf("%27s %8.6fs %8.6fs %8.6fs %9.1f %9.1f %9.1f\n", sig_name,
                   1.0 / sigs_results[k][0], 1.0 / sigs_results[k][1],
                   1.0 / sigs_results[k][2], sigs_results[k][0],
                   sigs_results[k][1], sigs_results[k][2]);
    }
    ret = 0;

 end:
    if (ret == 0 && testmode)
        ret = testmoderesult;
    ERR_print_errors(bio_err);
    for (i = 0; i < loopargs_len; i++) {
        OPENSSL_free(loopargs[i].buf_malloc);
        OPENSSL_free(loopargs[i].buf2_malloc);

        BN_free(bn);
        EVP_PKEY_CTX_free(genctx);
        for (k = 0; k < RSA_NUM; k++) {
            EVP_PKEY_CTX_free(loopargs[i].rsa_sign_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].rsa_verify_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].rsa_encrypt_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].rsa_decrypt_ctx[k]);
        }
#ifndef OPENSSL_NO_DH
        OPENSSL_free(loopargs[i].secret_ff_a);
        OPENSSL_free(loopargs[i].secret_ff_b);
        for (k = 0; k < FFDH_NUM; k++)
            EVP_PKEY_CTX_free(loopargs[i].ffdh_ctx[k]);
#endif
#ifndef OPENSSL_NO_DSA
        for (k = 0; k < DSA_NUM; k++) {
            EVP_PKEY_CTX_free(loopargs[i].dsa_sign_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].dsa_verify_ctx[k]);
        }
#endif
        for (k = 0; k < ECDSA_NUM; k++) {
            EVP_PKEY_CTX_free(loopargs[i].ecdsa_sign_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].ecdsa_verify_ctx[k]);
        }
        for (k = 0; k < EC_NUM; k++)
            EVP_PKEY_CTX_free(loopargs[i].ecdh_ctx[k]);
#ifndef OPENSSL_NO_ECX
        for (k = 0; k < EdDSA_NUM; k++) {
            EVP_MD_CTX_free(loopargs[i].eddsa_ctx[k]);
            EVP_MD_CTX_free(loopargs[i].eddsa_ctx2[k]);
        }
#endif /* OPENSSL_NO_ECX */
#ifndef OPENSSL_NO_SM2
        for (k = 0; k < SM2_NUM; k++) {
            EVP_PKEY_CTX *pctx = NULL;

            /* free signing ctx */
            if (loopargs[i].sm2_ctx[k] != NULL
                && (pctx = EVP_MD_CTX_get_pkey_ctx(loopargs[i].sm2_ctx[k])) != NULL)
                EVP_PKEY_CTX_free(pctx);
            EVP_MD_CTX_free(loopargs[i].sm2_ctx[k]);
            /* free verification ctx */
            if (loopargs[i].sm2_vfy_ctx[k] != NULL
                && (pctx = EVP_MD_CTX_get_pkey_ctx(loopargs[i].sm2_vfy_ctx[k])) != NULL)
                EVP_PKEY_CTX_free(pctx);
            EVP_MD_CTX_free(loopargs[i].sm2_vfy_ctx[k]);
            /* free pkey */
            EVP_PKEY_free(loopargs[i].sm2_pkey[k]);
        }
#endif
        for (k = 0; k < kems_algs_len; k++) {
            EVP_PKEY_CTX_free(loopargs[i].kem_gen_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].kem_encaps_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].kem_decaps_ctx[k]);
            OPENSSL_free(loopargs[i].kem_out[k]);
            OPENSSL_free(loopargs[i].kem_send_secret[k]);
            OPENSSL_free(loopargs[i].kem_rcv_secret[k]);
        }
        for (k = 0; k < sigs_algs_len; k++) {
            EVP_PKEY_CTX_free(loopargs[i].sig_gen_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].sig_sign_ctx[k]);
            EVP_PKEY_CTX_free(loopargs[i].sig_verify_ctx[k]);
            OPENSSL_free(loopargs[i].sig_sig[k]);
        }
        OPENSSL_free(loopargs[i].secret_a);
        OPENSSL_free(loopargs[i].secret_b);
    }
    OPENSSL_free(evp_hmac_name);
    OPENSSL_free(evp_cmac_name);
    for (k = 0; k < kems_algs_len; k++)
        OPENSSL_free(kems_algname[k]);
    if (kem_stack != NULL)
        sk_EVP_KEM_pop_free(kem_stack, EVP_KEM_free);
    for (k = 0; k < sigs_algs_len; k++)
        OPENSSL_free(sigs_algname[k]);
    if (sig_stack != NULL)
        sk_EVP_SIGNATURE_pop_free(sig_stack, EVP_SIGNATURE_free);

    if (async_jobs > 0) {
        for (i = 0; i < loopargs_len; i++)
            ASYNC_WAIT_CTX_free(loopargs[i].wait_ctx);
    }

    if (async_init) {
        ASYNC_cleanup_thread();
    }
    OPENSSL_free(loopargs);
    release_engine(e);
    EVP_CIPHER_free(evp_cipher);
    EVP_MAC_free(mac);
    NCONF_free(conf);
    return ret;
}

static void print_message(const char *s, int length, int tm)
{
    BIO_printf(bio_err,
               mr ? "+DT:%s:%d:%d\n"
               : "Doing %s ops for %ds on %d size blocks: ", s, tm, length);
    (void)BIO_flush(bio_err);
    run = 1;
    alarm(tm);
}

static void pkey_print_message(const char *str, const char *str2, unsigned int bits,
                               int tm)
{
    BIO_printf(bio_err,
               mr ? "+DTP:%d:%s:%s:%d\n"
               : "Doing %u bits %s %s ops for %ds: ", bits, str, str2, tm);
    (void)BIO_flush(bio_err);
    run = 1;
    alarm(tm);
}

static void kskey_print_message(const char *str, const char *str2, int tm)
{
    BIO_printf(bio_err,
               mr ? "+DTP:%s:%s:%d\n"
               : "Doing %s %s ops for %ds: ", str, str2, tm);
    (void)BIO_flush(bio_err);
    run = 1;
    alarm(tm);
}

static void print_result(int alg, int run_no, int count, double time_used)
{
    if (count == -1) {
        BIO_printf(bio_err, "%s error!\n", names[alg]);
        dofail();
        return;
    }
    BIO_printf(bio_err,
               mr ? "+R:%d:%s:%f\n"
               : "%d %s ops in %.2fs\n", count, names[alg], time_used);
    results[alg][run_no] = ((double)count) / time_used * lengths[run_no];
}

#ifndef NO_FORK
static char *sstrsep(char **string, const char *delim)
{
    char isdelim[256];
    char *token = *string;

    memset(isdelim, 0, sizeof(isdelim));
    isdelim[0] = 1;

    while (*delim) {
        isdelim[(unsigned char)(*delim)] = 1;
        delim++;
    }

    while (!isdelim[(unsigned char)(**string)])
        (*string)++;

    if (**string) {
        **string = 0;
        (*string)++;
    }

    return token;
}

static int strtoint(const char *str, const int min_val, const int upper_val,
                    int *res)
{
    char *end = NULL;
    long int val = 0;

    errno = 0;
    val = strtol(str, &end, 10);
    if (errno == 0 && end != str && *end == 0
        && min_val <= val && val < upper_val) {
        *res = (int)val;
        return 1;
    } else {
        return 0;
    }
}

static int do_multi(int multi, int size_num)
{
    int n;
    int fd[2];
    int *fds;
    int status;
    static char sep[] = ":";

    fds = app_malloc(sizeof(*fds) * multi, "fd buffer for do_multi");
    for (n = 0; n < multi; ++n) {
        if (pipe(fd) == -1) {
            BIO_printf(bio_err, "pipe failure\n");
            exit(1);
        }
        fflush(stdout);
        (void)BIO_flush(bio_err);
        if (fork()) {
            close(fd[1]);
            fds[n] = fd[0];
        } else {
            close(fd[0]);
            close(1);
            if (dup(fd[1]) == -1) {
                BIO_printf(bio_err, "dup failed\n");
                exit(1);
            }
            close(fd[1]);
            mr = 1;
            usertime = 0;
            OPENSSL_free(fds);
            return 0;
        }
        printf("Forked child %d\n", n);
    }

    /* for now, assume the pipe is long enough to take all the output */
    for (n = 0; n < multi; ++n) {
        FILE *f;
        char buf[1024];
        char *p;
        char *tk;
        int k;
        double d;

        if ((f = fdopen(fds[n], "r")) == NULL) {
            BIO_printf(bio_err, "fdopen failure with 0x%x\n",
                       errno);
            OPENSSL_free(fds);
            return 1;
        }
        while (fgets(buf, sizeof(buf), f)) {
            p = strchr(buf, '\n');
            if (p)
                *p = '\0';
            if (buf[0] != '+') {
                BIO_printf(bio_err,
                           "Don't understand line '%s' from child %d\n", buf,
                           n);
                continue;
            }
            printf("Got: %s from %d\n", buf, n);
            p = buf;
            if (CHECK_AND_SKIP_PREFIX(p, "+F:")) {
                int alg;
                int j;

                if (strtoint(sstrsep(&p, sep), 0, ALGOR_NUM, &alg)) {
                    sstrsep(&p, sep);
                    for (j = 0; j < size_num; ++j)
                        results[alg][j] += atof(sstrsep(&p, sep));
                }
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F2:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(rsa_results), &k)) {
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    rsa_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    rsa_results[k][1] += d;

                    d = atof(sstrsep(&p, sep));
                    rsa_results[k][2] += d;

                    d = atof(sstrsep(&p, sep));
                    rsa_results[k][3] += d;
                }
# ifndef OPENSSL_NO_DSA
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F3:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(dsa_results), &k)) {
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    dsa_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    dsa_results[k][1] += d;
                }
# endif /* OPENSSL_NO_DSA */
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F4:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(ecdsa_results), &k)) {
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    ecdsa_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    ecdsa_results[k][1] += d;
                }
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F5:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(ecdh_results), &k)) {
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    ecdh_results[k][0] += d;
                }
# ifndef OPENSSL_NO_ECX
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F6:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(eddsa_results), &k)) {
                    sstrsep(&p, sep);
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    eddsa_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    eddsa_results[k][1] += d;
                }
# endif /* OPENSSL_NO_ECX */
# ifndef OPENSSL_NO_SM2
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F7:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(sm2_results), &k)) {
                    sstrsep(&p, sep);
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    sm2_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    sm2_results[k][1] += d;
                }
# endif /* OPENSSL_NO_SM2 */
# ifndef OPENSSL_NO_DH
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F8:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(ffdh_results), &k)) {
                    sstrsep(&p, sep);

                    d = atof(sstrsep(&p, sep));
                    ffdh_results[k][0] += d;
                }
# endif /* OPENSSL_NO_DH */
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F9:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(kems_results), &k)) {
                    d = atof(sstrsep(&p, sep));
                    kems_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    kems_results[k][1] += d;

                    d = atof(sstrsep(&p, sep));
                    kems_results[k][2] += d;
                }
            } else if (CHECK_AND_SKIP_PREFIX(p, "+F10:")) {
                tk = sstrsep(&p, sep);
                if (strtoint(tk, 0, OSSL_NELEM(sigs_results), &k)) {
                    d = atof(sstrsep(&p, sep));
                    sigs_results[k][0] += d;

                    d = atof(sstrsep(&p, sep));
                    sigs_results[k][1] += d;

                    d = atof(sstrsep(&p, sep));
                    sigs_results[k][2] += d;
                }
            } else if (!HAS_PREFIX(buf, "+H:")) {
                BIO_printf(bio_err, "Unknown type '%s' from child %d\n", buf,
                           n);
            }
        }

        fclose(f);
    }
    OPENSSL_free(fds);
    for (n = 0; n < multi; ++n) {
        while (wait(&status) == -1)
            if (errno != EINTR) {
                BIO_printf(bio_err, "Waitng for child failed with 0x%x\n",
                           errno);
                return 1;
            }
        if (WIFEXITED(status) && WEXITSTATUS(status)) {
            BIO_printf(bio_err, "Child exited with %d\n", WEXITSTATUS(status));
        } else if (WIFSIGNALED(status)) {
            BIO_printf(bio_err, "Child terminated by signal %d\n",
                       WTERMSIG(status));
        }
    }
    return 1;
}
#endif

static void multiblock_speed(const EVP_CIPHER *evp_cipher, int lengths_single,
                             const openssl_speed_sec_t *seconds)
{
    static const int mblengths_list[] = {
        8 * 1024, 2 * 8 * 1024, 4 * 8 * 1024, 8 * 8 * 1024, 8 * 16 * 1024
    };
    const int *mblengths = mblengths_list;
    int j, count, keylen, num = OSSL_NELEM(mblengths_list), ciph_success = 1;
    const char *alg_name;
    unsigned char *inp = NULL, *out = NULL, *key, no_key[32], no_iv[16];
    EVP_CIPHER_CTX *ctx = NULL;
    double d = 0.0;

    if (lengths_single) {
        mblengths = &lengths_single;
        num = 1;
    }

    inp = app_malloc(mblengths[num - 1], "multiblock input buffer");
    out = app_malloc(mblengths[num - 1] + 1024, "multiblock output buffer");
    if ((ctx = EVP_CIPHER_CTX_new()) == NULL)
        app_bail_out("failed to allocate cipher context\n");
    if (!EVP_EncryptInit_ex(ctx, evp_cipher, NULL, NULL, no_iv))
        app_bail_out("failed to initialise cipher context\n");

    if ((keylen = EVP_CIPHER_CTX_get_key_length(ctx)) < 0) {
        BIO_printf(bio_err, "Impossible negative key length: %d\n", keylen);
        goto err;
    }
    key = app_malloc(keylen, "evp_cipher key");
    if (EVP_CIPHER_CTX_rand_key(ctx, key) <= 0)
        app_bail_out("failed to generate random cipher key\n");
    if (!EVP_EncryptInit_ex(ctx, NULL, NULL, key, NULL))
        app_bail_out("failed to set cipher key\n");
    OPENSSL_clear_free(key, keylen);

    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_MAC_KEY,
                             sizeof(no_key), no_key) <= 0)
        app_bail_out("failed to set AEAD key\n");
    if ((alg_name = EVP_CIPHER_get0_name(evp_cipher)) == NULL)
        app_bail_out("failed to get cipher name\n");

    for (j = 0; j < num; j++) {
        print_message(alg_name, mblengths[j], seconds->sym);
        Time_F(START);
        for (count = 0; run && COND(count); count++) {
            EVP_CTRL_TLS1_1_MULTIBLOCK_PARAM mb_param;
            size_t len = mblengths[j];
            int packlen;

            memset(aad, 0, 8);  /* avoid uninitialized values */
            aad[8] = 23;        /* SSL3_RT_APPLICATION_DATA */
            aad[9] = 3;         /* version */
            aad[10] = 2;
            aad[11] = 0;        /* length */
            aad[12] = 0;
            mb_param.out = NULL;
            mb_param.inp = aad;
            mb_param.len = len;
            mb_param.interleave = 8;

            packlen = EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_TLS1_1_MULTIBLOCK_AAD,
                                          sizeof(mb_param), &mb_param);

            if (packlen > 0) {
                mb_param.out = out;
                mb_param.inp = inp;
                mb_param.len = len;
                (void)EVP_CIPHER_CTX_ctrl(ctx,
                                          EVP_CTRL_TLS1_1_MULTIBLOCK_ENCRYPT,
                                          sizeof(mb_param), &mb_param);
            } else {
                int pad;

                if (RAND_bytes(inp, 16) <= 0)
                    app_bail_out("error setting random bytes\n");
                len += 16;
                aad[11] = (unsigned char)(len >> 8);
                aad[12] = (unsigned char)(len);
                pad = EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_TLS1_AAD,
                                          EVP_AEAD_TLS1_AAD_LEN, aad);
                ciph_success = EVP_Cipher(ctx, out, inp, len + pad);
            }
        }
        d = Time_F(STOP);
        BIO_printf(bio_err, mr ? "+R:%d:%s:%f\n"
                   : "%d %s ops in %.2fs\n", count, "evp", d);
        if ((ciph_success <= 0) && (mr == 0))
            BIO_printf(bio_err, "Error performing cipher op\n");
        results[D_EVP][j] = ((double)count) / d * mblengths[j];
    }

    if (mr) {
        fprintf(stdout, "+H");
        for (j = 0; j < num; j++)
            fprintf(stdout, ":%d", mblengths[j]);
        fprintf(stdout, "\n");
        fprintf(stdout, "+F:%d:%s", D_EVP, alg_name);
        for (j = 0; j < num; j++)
            fprintf(stdout, ":%.2f", results[D_EVP][j]);
        fprintf(stdout, "\n");
    } else {
        fprintf(stdout,
                "The 'numbers' are in 1000s of bytes per second processed.\n");
        fprintf(stdout, "type                    ");
        for (j = 0; j < num; j++)
            fprintf(stdout, "%7d bytes", mblengths[j]);
        fprintf(stdout, "\n");
        fprintf(stdout, "%-24s", alg_name);

        for (j = 0; j < num; j++) {
            if (results[D_EVP][j] > 10000)
                fprintf(stdout, " %11.2fk", results[D_EVP][j] / 1e3);
            else
                fprintf(stdout, " %11.2f ", results[D_EVP][j]);
        }
        fprintf(stdout, "\n");
    }

 err:
    OPENSSL_free(inp);
    OPENSSL_free(out);
    EVP_CIPHER_CTX_free(ctx);
}
