/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_APPS_H
# define OSSL_APPS_H

# include "internal/common.h" /* for HAS_PREFIX */
# include "internal/nelem.h"
# include <assert.h>

# include <stdarg.h>
# include <sys/types.h>
# ifndef OPENSSL_NO_POSIX_IO
#  include <sys/stat.h>
#  include <fcntl.h>
# endif

# include <openssl/e_os2.h>
# include <openssl/types.h>
# include <openssl/bio.h>
# include <openssl/x509.h>
# include <openssl/conf.h>
# include <openssl/txt_db.h>
# include <openssl/engine.h>
# include <openssl/ocsp.h>
# include <openssl/http.h>
# include <signal.h>
# include "apps_ui.h"
# include "opt.h"
# include "fmt.h"
# include "platform.h"
# include "engine_loader.h"
# include "app_libctx.h"

/*
 * quick macro when you need to pass an unsigned char instead of a char.
 * this is true for some implementations of the is*() functions, for
 * example.
 */
# define _UC(c) ((unsigned char)(c))

void app_RAND_load_conf(CONF *c, const char *section);
int app_RAND_write(void);
int app_RAND_load(void);

extern char *default_config_file; /* may be "" */
extern BIO *bio_in;
extern BIO *bio_out;
extern BIO *bio_err;
extern const unsigned char tls13_aes128gcmsha256_id[];
extern const unsigned char tls13_aes256gcmsha384_id[];
extern BIO_ADDR *ourpeer;

BIO *dup_bio_in(int format);
BIO *dup_bio_out(int format);
BIO *dup_bio_err(int format);
BIO *bio_open_owner(const char *filename, int format, int private);
BIO *bio_open_default(const char *filename, char mode, int format);
BIO *bio_open_default_quiet(const char *filename, char mode, int format);
int mem_bio_to_file(BIO *in, const char *filename, int format, int private);
char *app_conf_try_string(const CONF *cnf, const char *group, const char *name);
int app_conf_try_number(const CONF *conf, const char *group, const char *name,
                        long *result);
CONF *app_load_config_bio(BIO *in, const char *filename);
# define app_load_config(filename) app_load_config_internal(filename, 0)
# define app_load_config_quiet(filename) app_load_config_internal(filename, 1)
CONF *app_load_config_internal(const char *filename, int quiet);
CONF *app_load_config_verbose(const char *filename, int verbose);
int app_load_modules(const CONF *config);
CONF *app_load_config_modules(const char *configfile);
void unbuffer(FILE *fp);
void wait_for_async(SSL *s);
# if defined(OPENSSL_SYS_MSDOS)
int has_stdin_waiting(void);
# endif

void corrupt_signature(const ASN1_STRING *signature);

/* Helpers for setting X509v3 certificate fields notBefore and notAfter */
int check_cert_time_string(const char *time, const char *desc);
int set_cert_times(X509 *x, const char *startdate, const char *enddate,
                   int days, int strict_compare_times);

int set_crl_lastupdate(X509_CRL *crl, const char *lastupdate);
int set_crl_nextupdate(X509_CRL *crl, const char *nextupdate,
                       long days, long hours, long secs);

typedef struct args_st {
    int size;
    int argc;
    char **argv;
} ARGS;

/* We need both wrap and the "real" function because libcrypto uses both. */
int wrap_password_callback(char *buf, int bufsiz, int verify, void *cb_data);

/* progress callback for dsaparam, dhparam, req, genpkey, etc. */
int progress_cb(EVP_PKEY_CTX *ctx);

int chopup_args(ARGS *arg, char *buf);
void dump_cert_text(BIO *out, X509 *x);
void print_name(BIO *out, const char *title, const X509_NAME *nm);
void print_bignum_var(BIO *, const BIGNUM *, const char *,
                      int, unsigned char *);
void print_array(BIO *, const char *, int, const unsigned char *);
int set_nameopt(const char *arg);
unsigned long get_nameopt(void);
int set_dateopt(unsigned long *dateopt, const char *arg);
int set_cert_ex(unsigned long *flags, const char *arg);
int set_name_ex(unsigned long *flags, const char *arg);
int set_ext_copy(int *copy_type, const char *arg);
int copy_extensions(X509 *x, X509_REQ *req, int copy_type);
char *get_passwd(const char *pass, const char *desc);
int app_passwd(const char *arg1, const char *arg2, char **pass1, char **pass2);
int add_oid_section(CONF *conf);
X509_REQ *load_csr(const char *file, int format, const char *desc);
X509_REQ *load_csr_autofmt(const char *infile, int format,
                           STACK_OF(OPENSSL_STRING) *vfyopts, const char *desc);
X509 *load_cert_pass(const char *uri, int format, int maybe_stdin,
                     const char *pass, const char *desc);
# define load_cert(uri, format, desc) load_cert_pass(uri, format, 1, NULL, desc)
X509_CRL *load_crl(const char *uri, int format, int maybe_stdin,
                   const char *desc);
void cleanse(char *str);
void clear_free(char *str);
EVP_PKEY *load_key(const char *uri, int format, int maybe_stdin,
                   const char *pass, ENGINE *e, const char *desc);
/* first try reading public key, on failure resort to loading private key */
EVP_PKEY *load_pubkey(const char *uri, int format, int maybe_stdin,
                      const char *pass, ENGINE *e, const char *desc);
EVP_PKEY *load_keyparams(const char *uri, int format, int maybe_stdin,
                         const char *keytype, const char *desc);
EVP_PKEY *load_keyparams_suppress(const char *uri, int format, int maybe_stdin,
                                  const char *keytype, const char *desc,
                                  int suppress_decode_errors);
char *next_item(char *opt); /* in list separated by comma and/or space */
int load_cert_certs(const char *uri,
                    X509 **pcert, STACK_OF(X509) **pcerts,
                    int exclude_http, const char *pass, const char *desc,
                    X509_VERIFY_PARAM *vpm);
STACK_OF(X509) *load_certs_multifile(char *files, const char *pass,
                                     const char *desc, X509_VERIFY_PARAM *vpm);
X509_STORE *load_certstore(char *input, const char *pass, const char *desc,
                           X509_VERIFY_PARAM *vpm);
int load_certs(const char *uri, int maybe_stdin, STACK_OF(X509) **certs,
               const char *pass, const char *desc);
int load_crls(const char *uri, STACK_OF(X509_CRL) **crls,
              const char *pass, const char *desc);
int load_key_certs_crls(const char *uri, int format, int maybe_stdin,
                        const char *pass, const char *desc, int quiet,
                        EVP_PKEY **ppkey, EVP_PKEY **ppubkey,
                        EVP_PKEY **pparams,
                        X509 **pcert, STACK_OF(X509) **pcerts,
                        X509_CRL **pcrl, STACK_OF(X509_CRL) **pcrls);
X509_STORE *setup_verify(const char *CAfile, int noCAfile,
                         const char *CApath, int noCApath,
                         const char *CAstore, int noCAstore);
__owur int ctx_set_verify_locations(SSL_CTX *ctx,
                                    const char *CAfile, int noCAfile,
                                    const char *CApath, int noCApath,
                                    const char *CAstore, int noCAstore);

# ifndef OPENSSL_NO_CT

/*
 * Sets the file to load the Certificate Transparency log list from.
 * If path is NULL, loads from the default file path.
 * Returns 1 on success, 0 otherwise.
 */
__owur int ctx_set_ctlog_list_file(SSL_CTX *ctx, const char *path);

# endif

ENGINE *setup_engine_methods(const char *id, unsigned int methods, int debug);
# define setup_engine(e, debug) setup_engine_methods(e, (unsigned int)-1, debug)
void release_engine(ENGINE *e);
int init_engine(ENGINE *e);
int finish_engine(ENGINE *e);
char *make_engine_uri(ENGINE *e, const char *key_id, const char *desc);

int get_legacy_pkey_id(OSSL_LIB_CTX *libctx, const char *algname, ENGINE *e);
const EVP_MD *get_digest_from_engine(const char *name);
const EVP_CIPHER *get_cipher_from_engine(const char *name);

# ifndef OPENSSL_NO_OCSP
OCSP_RESPONSE *process_responder(OCSP_REQUEST *req, const char *host,
                                 const char *port, const char *path,
                                 const char *proxy, const char *no_proxy,
                                 int use_ssl, STACK_OF(CONF_VALUE) *headers,
                                 int req_timeout);
# endif

/* Functions defined in ca.c and also used in ocsp.c */
int unpack_revinfo(ASN1_TIME **prevtm, int *preason, ASN1_OBJECT **phold,
                   ASN1_GENERALIZEDTIME **pinvtm, const char *str);

# define DB_type         0
# define DB_exp_date     1
# define DB_rev_date     2
# define DB_serial       3 /* index - unique */
# define DB_file         4
# define DB_name         5 /* index - unique when active and not disabled */
# define DB_NUMBER       6

# define DB_TYPE_REV     'R'    /* Revoked  */
# define DB_TYPE_EXP     'E'    /* Expired  */
# define DB_TYPE_VAL     'V'    /* Valid ; inserted with: ca ... -valid */
# define DB_TYPE_SUSP    'S'    /* Suspended  */

typedef struct db_attr_st {
    int unique_subject;
} DB_ATTR;
typedef struct ca_db_st {
    DB_ATTR attributes;
    TXT_DB *db;
    char *dbfname;
# ifndef OPENSSL_NO_POSIX_IO
    struct stat dbst;
# endif
} CA_DB;

extern int do_updatedb(CA_DB *db, time_t *now);

void app_bail_out(char *fmt, ...);
void *app_malloc(size_t sz, const char *what);

/* load_serial, save_serial, and rotate_serial are also used for CRL numbers */
BIGNUM *load_serial(const char *serialfile, int *exists, int create,
                    ASN1_INTEGER **retai);
int save_serial(const char *serialfile, const char *suffix,
                const BIGNUM *serial, ASN1_INTEGER **retai);
int rotate_serial(const char *serialfile, const char *new_suffix,
                  const char *old_suffix);
int rand_serial(BIGNUM *b, ASN1_INTEGER *ai);

CA_DB *load_index(const char *dbfile, DB_ATTR *dbattr);
int index_index(CA_DB *db);
int save_index(const char *dbfile, const char *suffix, CA_DB *db);
int rotate_index(const char *dbfile, const char *new_suffix,
                 const char *old_suffix);
void free_index(CA_DB *db);
# define index_name_cmp_noconst(a, b) \
    index_name_cmp((const OPENSSL_CSTRING *)CHECKED_PTR_OF(OPENSSL_STRING, a), \
                   (const OPENSSL_CSTRING *)CHECKED_PTR_OF(OPENSSL_STRING, b))
int index_name_cmp(const OPENSSL_CSTRING *a, const OPENSSL_CSTRING *b);
int parse_yesno(const char *str, int def);

X509_NAME *parse_name(const char *str, int chtype, int multirdn,
                      const char *desc);
void policies_print(X509_STORE_CTX *ctx);
int bio_to_mem(unsigned char **out, int maxlen, BIO *in);
int pkey_ctrl_string(EVP_PKEY_CTX *ctx, const char *value);
int x509_ctrl_string(X509 *x, const char *value);
int x509_req_ctrl_string(X509_REQ *x, const char *value);
int init_gen_str(EVP_PKEY_CTX **pctx,
                 const char *algname, ENGINE *e, int do_param,
                 OSSL_LIB_CTX *libctx, const char *propq);
int cert_matches_key(const X509 *cert, const EVP_PKEY *pkey);
int do_X509_sign(X509 *x, int force_v1, EVP_PKEY *pkey, const char *md,
                 STACK_OF(OPENSSL_STRING) *sigopts, X509V3_CTX *ext_ctx);
int do_X509_verify(X509 *x, EVP_PKEY *pkey, STACK_OF(OPENSSL_STRING) *vfyopts);
int do_X509_REQ_sign(X509_REQ *x, EVP_PKEY *pkey, const char *md,
                     STACK_OF(OPENSSL_STRING) *sigopts);
int do_X509_REQ_verify(X509_REQ *x, EVP_PKEY *pkey,
                       STACK_OF(OPENSSL_STRING) *vfyopts);
int do_X509_CRL_sign(X509_CRL *x, EVP_PKEY *pkey, const char *md,
                     STACK_OF(OPENSSL_STRING) *sigopts);

extern char *psk_key;

unsigned char *next_protos_parse(size_t *outlen, const char *in);

int check_cert_attributes(BIO *bio, X509 *x,
                          const char *checkhost, const char *checkemail,
                          const char *checkip, int print);

void store_setup_crl_download(X509_STORE *st);

typedef struct app_http_tls_info_st {
    const char *server;
    const char *port;
    int use_proxy;
    long timeout;
    SSL_CTX *ssl_ctx;
} APP_HTTP_TLS_INFO;
BIO *app_http_tls_cb(BIO *hbio, /* APP_HTTP_TLS_INFO */ void *arg,
                     int connect, int detail);
void APP_HTTP_TLS_INFO_free(APP_HTTP_TLS_INFO *info);
# ifndef OPENSSL_NO_SOCK
ASN1_VALUE *app_http_get_asn1(const char *url, const char *proxy,
                              const char *no_proxy, SSL_CTX *ssl_ctx,
                              const STACK_OF(CONF_VALUE) *headers,
                              long timeout, const char *expected_content_type,
                              const ASN1_ITEM *it);
ASN1_VALUE *app_http_post_asn1(const char *host, const char *port,
                               const char *path, const char *proxy,
                               const char *no_proxy, SSL_CTX *ctx,
                               const STACK_OF(CONF_VALUE) *headers,
                               const char *content_type,
                               ASN1_VALUE *req, const ASN1_ITEM *req_it,
                               const char *expected_content_type,
                               long timeout, const ASN1_ITEM *rsp_it);
# endif

# define EXT_COPY_NONE   0
# define EXT_COPY_ADD    1
# define EXT_COPY_ALL    2

# define NETSCAPE_CERT_HDR "certificate"

# define APP_PASS_LEN 1024

/*
 * IETF RFC 5280 says serial number must be <= 20 bytes. Use 159 bits
 * so that the first bit will never be one, so that the DER encoding
 * rules won't force a leading octet.
 */
# define SERIAL_RAND_BITS 159

int app_isdir(const char *);
int app_access(const char *, int flag);
int fileno_stdin(void);
int fileno_stdout(void);
int raw_read_stdin(void *, int);
int raw_write_stdout(const void *, int);

# define TM_START        0
# define TM_STOP         1
double app_tminterval(int stop, int usertime);

void make_uppercase(char *string);

typedef struct verify_options_st {
    int depth;
    int quiet;
    int error;
    int return_error;
} VERIFY_CB_ARGS;

extern VERIFY_CB_ARGS verify_args;

OSSL_PARAM *app_params_new_from_opts(STACK_OF(OPENSSL_STRING) *opts,
                                     const OSSL_PARAM *paramdefs);
void app_params_free(OSSL_PARAM *params);
int app_provider_load(OSSL_LIB_CTX *libctx, const char *provider_name);
void app_providers_cleanup(void);

EVP_PKEY *app_keygen(EVP_PKEY_CTX *ctx, const char *alg, int bits, int verbose);
EVP_PKEY *app_paramgen(EVP_PKEY_CTX *ctx, const char *alg);

#endif
