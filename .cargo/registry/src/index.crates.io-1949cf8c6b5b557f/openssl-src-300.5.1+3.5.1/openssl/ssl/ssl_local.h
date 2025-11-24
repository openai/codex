/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_SSL_LOCAL_H
# define OSSL_SSL_LOCAL_H

# include <stdlib.h>
# include <time.h>
# include <errno.h>
# include "internal/common.h" /* for HAS_PREFIX */

# include <openssl/buffer.h>
# include <openssl/bio.h>
# include <openssl/comp.h>
# include <openssl/dsa.h>
# include <openssl/err.h>
# include <openssl/ssl.h>
# include <openssl/async.h>
# include <openssl/symhacks.h>
# include <openssl/ct.h>
# include "internal/recordmethod.h"
# include "internal/statem.h"
# include "internal/packet.h"
# include "internal/dane.h"
# include "internal/refcount.h"
# include "internal/tsan_assist.h"
# include "internal/bio.h"
# include "internal/ktls.h"
# include "internal/time.h"
# include "internal/ssl.h"
# include "internal/cryptlib.h"
# include "internal/quic_predef.h"
# include "record/record.h"
# include "internal/quic_predef.h"
# include "internal/quic_tls.h"

# ifdef OPENSSL_BUILD_SHLIBSSL
#  undef OPENSSL_EXTERN
#  define OPENSSL_EXTERN OPENSSL_EXPORT
# endif

# define TLS_MAX_VERSION_INTERNAL TLS1_3_VERSION
# define DTLS_MAX_VERSION_INTERNAL DTLS1_2_VERSION

/*
 * DTLS version numbers are strange because they're inverted. Except for
 * DTLS1_BAD_VER, which should be considered "lower" than the rest.
 */
# define dtls_ver_ordinal(v1) (((v1) == DTLS1_BAD_VER) ? 0xff00 : (v1))
# define DTLS_VERSION_GT(v1, v2) (dtls_ver_ordinal(v1) < dtls_ver_ordinal(v2))
# define DTLS_VERSION_GE(v1, v2) (dtls_ver_ordinal(v1) <= dtls_ver_ordinal(v2))
# define DTLS_VERSION_LT(v1, v2) (dtls_ver_ordinal(v1) > dtls_ver_ordinal(v2))
# define DTLS_VERSION_LE(v1, v2) (dtls_ver_ordinal(v1) >= dtls_ver_ordinal(v2))

# define SSL_AD_NO_ALERT    -1

/*
 * Define the Bitmasks for SSL_CIPHER.algorithms.
 * This bits are used packed as dense as possible. If new methods/ciphers
 * etc will be added, the bits a likely to change, so this information
 * is for internal library use only, even though SSL_CIPHER.algorithms
 * can be publicly accessed.
 * Use the according functions for cipher management instead.
 *
 * The bit mask handling in the selection and sorting scheme in
 * ssl_create_cipher_list() has only limited capabilities, reflecting
 * that the different entities within are mutually exclusive:
 * ONLY ONE BIT PER MASK CAN BE SET AT A TIME.
 */

/* Bits for algorithm_mkey (key exchange algorithm) */
/* RSA key exchange */
# define SSL_kRSA                0x00000001U
/* tmp DH key no DH cert */
# define SSL_kDHE                0x00000002U
/* ephemeral ECDH */
# define SSL_kECDHE              0x00000004U
/* PSK */
# define SSL_kPSK                0x00000008U
/* GOST key exchange */
# define SSL_kGOST               0x00000010U
/* SRP */
# define SSL_kSRP                0x00000020U

# define SSL_kRSAPSK             0x00000040U
# define SSL_kECDHEPSK           0x00000080U
# define SSL_kDHEPSK             0x00000100U
/* GOST KDF key exchange, draft-smyshlyaev-tls12-gost-suites */
# define SSL_kGOST18             0x00000200U

/* all PSK */

# define SSL_PSK     (SSL_kPSK | SSL_kRSAPSK | SSL_kECDHEPSK | SSL_kDHEPSK)

/* Any appropriate key exchange algorithm (for TLS 1.3 ciphersuites) */
# define SSL_kANY                0x00000000U

/* Bits for algorithm_auth (server authentication) */
/* RSA auth */
# define SSL_aRSA                0x00000001U
/* DSS auth */
# define SSL_aDSS                0x00000002U
/* no auth (i.e. use ADH or AECDH) */
# define SSL_aNULL               0x00000004U
/* ECDSA auth*/
# define SSL_aECDSA              0x00000008U
/* PSK auth */
# define SSL_aPSK                0x00000010U
/* GOST R 34.10-2001 signature auth */
# define SSL_aGOST01             0x00000020U
/* SRP auth */
# define SSL_aSRP                0x00000040U
/* GOST R 34.10-2012 signature auth */
# define SSL_aGOST12             0x00000080U
/* Any appropriate signature auth (for TLS 1.3 ciphersuites) */
# define SSL_aANY                0x00000000U
/* All bits requiring a certificate */
#define SSL_aCERT \
    (SSL_aRSA | SSL_aDSS | SSL_aECDSA | SSL_aGOST01 | SSL_aGOST12)

/* Bits for algorithm_enc (symmetric encryption) */
# define SSL_DES                 0x00000001U
# define SSL_3DES                0x00000002U
# define SSL_RC4                 0x00000004U
# define SSL_RC2                 0x00000008U
# define SSL_IDEA                0x00000010U
# define SSL_eNULL               0x00000020U
# define SSL_AES128              0x00000040U
# define SSL_AES256              0x00000080U
# define SSL_CAMELLIA128         0x00000100U
# define SSL_CAMELLIA256         0x00000200U
# define SSL_eGOST2814789CNT     0x00000400U
# define SSL_SEED                0x00000800U
# define SSL_AES128GCM           0x00001000U
# define SSL_AES256GCM           0x00002000U
# define SSL_AES128CCM           0x00004000U
# define SSL_AES256CCM           0x00008000U
# define SSL_AES128CCM8          0x00010000U
# define SSL_AES256CCM8          0x00020000U
# define SSL_eGOST2814789CNT12   0x00040000U
# define SSL_CHACHA20POLY1305    0x00080000U
# define SSL_ARIA128GCM          0x00100000U
# define SSL_ARIA256GCM          0x00200000U
# define SSL_MAGMA               0x00400000U
# define SSL_KUZNYECHIK          0x00800000U

# define SSL_AESGCM              (SSL_AES128GCM | SSL_AES256GCM)
# define SSL_AESCCM              (SSL_AES128CCM | SSL_AES256CCM | SSL_AES128CCM8 | SSL_AES256CCM8)
# define SSL_AES                 (SSL_AES128|SSL_AES256|SSL_AESGCM|SSL_AESCCM)
# define SSL_CAMELLIA            (SSL_CAMELLIA128|SSL_CAMELLIA256)
# define SSL_CHACHA20            (SSL_CHACHA20POLY1305)
# define SSL_ARIAGCM             (SSL_ARIA128GCM | SSL_ARIA256GCM)
# define SSL_ARIA                (SSL_ARIAGCM)
# define SSL_CBC                 (SSL_DES | SSL_3DES | SSL_RC2 | SSL_IDEA \
                                  | SSL_AES128 | SSL_AES256 | SSL_CAMELLIA128 \
                                  | SSL_CAMELLIA256 | SSL_SEED)

/* Bits for algorithm_mac (symmetric authentication) */

# define SSL_MD5                 0x00000001U
# define SSL_SHA1                0x00000002U
# define SSL_GOST94      0x00000004U
# define SSL_GOST89MAC   0x00000008U
# define SSL_SHA256              0x00000010U
# define SSL_SHA384              0x00000020U
/* Not a real MAC, just an indication it is part of cipher */
# define SSL_AEAD                0x00000040U
# define SSL_GOST12_256          0x00000080U
# define SSL_GOST89MAC12         0x00000100U
# define SSL_GOST12_512          0x00000200U
# define SSL_MAGMAOMAC           0x00000400U
# define SSL_KUZNYECHIKOMAC      0x00000800U

/*
 * When adding new digest in the ssl_ciph.c and increment SSL_MD_NUM_IDX make
 * sure to update this constant too
 */

# define SSL_MD_MD5_IDX  0
# define SSL_MD_SHA1_IDX 1
# define SSL_MD_GOST94_IDX 2
# define SSL_MD_GOST89MAC_IDX 3
# define SSL_MD_SHA256_IDX 4
# define SSL_MD_SHA384_IDX 5
# define SSL_MD_GOST12_256_IDX  6
# define SSL_MD_GOST89MAC12_IDX 7
# define SSL_MD_GOST12_512_IDX  8
# define SSL_MD_MD5_SHA1_IDX 9
# define SSL_MD_SHA224_IDX 10
# define SSL_MD_SHA512_IDX 11
# define SSL_MD_MAGMAOMAC_IDX 12
# define SSL_MD_KUZNYECHIKOMAC_IDX 13
# define SSL_MAX_DIGEST 14

#define SSL_MD_NUM_IDX  SSL_MAX_DIGEST

/* Bits for algorithm2 (handshake digests and other extra flags) */

/* Bits 0-7 are handshake MAC */
# define SSL_HANDSHAKE_MAC_MASK  0xFF
# define SSL_HANDSHAKE_MAC_MD5_SHA1 SSL_MD_MD5_SHA1_IDX
# define SSL_HANDSHAKE_MAC_SHA256   SSL_MD_SHA256_IDX
# define SSL_HANDSHAKE_MAC_SHA384   SSL_MD_SHA384_IDX
# define SSL_HANDSHAKE_MAC_GOST94 SSL_MD_GOST94_IDX
# define SSL_HANDSHAKE_MAC_GOST12_256 SSL_MD_GOST12_256_IDX
# define SSL_HANDSHAKE_MAC_GOST12_512 SSL_MD_GOST12_512_IDX
# define SSL_HANDSHAKE_MAC_DEFAULT  SSL_HANDSHAKE_MAC_MD5_SHA1

/* Bits 8-15 bits are PRF */
# define TLS1_PRF_DGST_SHIFT 8
# define TLS1_PRF_SHA1_MD5 (SSL_MD_MD5_SHA1_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF_SHA256 (SSL_MD_SHA256_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF_SHA384 (SSL_MD_SHA384_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF_GOST94 (SSL_MD_GOST94_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF_GOST12_256 (SSL_MD_GOST12_256_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF_GOST12_512 (SSL_MD_GOST12_512_IDX << TLS1_PRF_DGST_SHIFT)
# define TLS1_PRF            (SSL_MD_MD5_SHA1_IDX << TLS1_PRF_DGST_SHIFT)

/*
 * Stream MAC for GOST ciphersuites from cryptopro draft (currently this also
 * goes into algorithm2)
 */
# define TLS1_STREAM_MAC 0x10000
/*
 * TLSTREE cipher/mac key derivation from draft-smyshlyaev-tls12-gost-suites
 * (currently this also  goes into algorithm2)
 */
# define TLS1_TLSTREE 0x20000

/* Ciphersuite supported in QUIC */
# define SSL_QUIC                0x00040000U

# define SSL_STRONG_MASK         0x0000001FU
# define SSL_DEFAULT_MASK        0X00000020U

# define SSL_STRONG_NONE         0x00000001U
# define SSL_LOW                 0x00000002U
# define SSL_MEDIUM              0x00000004U
# define SSL_HIGH                0x00000008U
# define SSL_FIPS                0x00000010U
# define SSL_NOT_DEFAULT         0x00000020U

/* we have used 0000003f - 26 bits left to go */

/* Flag used on OpenSSL ciphersuite ids to indicate they are for SSLv3+ */
# define SSL3_CK_CIPHERSUITE_FLAG                0x03000000

/* Check if an SSL structure is using DTLS */
# define SSL_CONNECTION_IS_DTLS(s) \
    (SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->enc_flags & SSL_ENC_FLAG_DTLS)

/* Check if an SSL_CTX structure is using DTLS */
# define SSL_CTX_IS_DTLS(ctx) \
    (ctx->method->ssl3_enc->enc_flags & SSL_ENC_FLAG_DTLS)

/* Check if we are using TLSv1.3 */
# define SSL_CONNECTION_IS_TLS13(s) (!SSL_CONNECTION_IS_DTLS(s) \
    && SSL_CONNECTION_GET_SSL(s)->method->version >= TLS1_3_VERSION \
    && SSL_CONNECTION_GET_SSL(s)->method->version != TLS_ANY_VERSION)

# define SSL_CONNECTION_TREAT_AS_TLS13(s) \
    (SSL_CONNECTION_IS_TLS13(s) \
     || (s)->early_data_state == SSL_EARLY_DATA_CONNECTING \
     || (s)->early_data_state == SSL_EARLY_DATA_CONNECT_RETRY \
     || (s)->early_data_state == SSL_EARLY_DATA_WRITING \
     || (s)->early_data_state == SSL_EARLY_DATA_WRITE_RETRY \
     || (s)->hello_retry_request == SSL_HRR_PENDING)

# define SSL_IS_FIRST_HANDSHAKE(s) ((s)->s3.tmp.finish_md_len == 0 \
                                    || (s)->s3.tmp.peer_finish_md_len == 0)

/*
 * See if we use signature algorithms extension and signature algorithm
 * before signatures.
 */
# define SSL_USE_SIGALGS(s)      \
    (SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->enc_flags & SSL_ENC_FLAG_SIGALGS)
/*
 * Allow TLS 1.2 ciphersuites: applies to DTLS 1.2 as well as TLS 1.2: may
 * apply to others in future.
 */
# define SSL_USE_TLS1_2_CIPHERS(s)       \
    (SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->enc_flags & SSL_ENC_FLAG_TLS1_2_CIPHERS)

# define IS_MAX_FRAGMENT_LENGTH_EXT_VALID(value) \
    (((value) >= TLSEXT_max_fragment_length_512) && \
     ((value) <= TLSEXT_max_fragment_length_4096))
# define USE_MAX_FRAGMENT_LENGTH_EXT(session) \
    IS_MAX_FRAGMENT_LENGTH_EXT_VALID(session->ext.max_fragment_len_mode)
# define GET_MAX_FRAGMENT_LENGTH(session) \
    (512U << (session->ext.max_fragment_len_mode - 1))

# define SSL_READ_ETM(s) (s->s3.flags & TLS1_FLAGS_ENCRYPT_THEN_MAC_READ)
# define SSL_WRITE_ETM(s) (s->s3.flags & TLS1_FLAGS_ENCRYPT_THEN_MAC_WRITE)

# define SSL_IS_QUIC_HANDSHAKE(s) (((s)->s3.flags & TLS1_FLAGS_QUIC) != 0)
# define SSL_IS_QUIC_INT_HANDSHAKE(s) (((s)->s3.flags & TLS1_FLAGS_QUIC_INTERNAL) != 0)

/* no end of early data */
# define SSL_NO_EOED(s) SSL_IS_QUIC_HANDSHAKE(s)

/* alert_dispatch values */

/* No alert pending */
# define SSL_ALERT_DISPATCH_NONE    0
/* Alert pending */
# define SSL_ALERT_DISPATCH_PENDING 1
/* Pending alert write needs to be retried */
# define SSL_ALERT_DISPATCH_RETRY   2

/* Mostly for SSLv3 */
# define SSL_PKEY_RSA            0
# define SSL_PKEY_RSA_PSS_SIGN   1
# define SSL_PKEY_DSA_SIGN       2
# define SSL_PKEY_ECC            3
# define SSL_PKEY_GOST01         4
# define SSL_PKEY_GOST12_256     5
# define SSL_PKEY_GOST12_512     6
# define SSL_PKEY_ED25519        7
# define SSL_PKEY_ED448          8
# define SSL_PKEY_NUM            9

# define SSL_ENC_DES_IDX         0
# define SSL_ENC_3DES_IDX        1
# define SSL_ENC_RC4_IDX         2
# define SSL_ENC_RC2_IDX         3
# define SSL_ENC_IDEA_IDX        4
# define SSL_ENC_NULL_IDX        5
# define SSL_ENC_AES128_IDX      6
# define SSL_ENC_AES256_IDX      7
# define SSL_ENC_CAMELLIA128_IDX 8
# define SSL_ENC_CAMELLIA256_IDX 9
# define SSL_ENC_GOST89_IDX      10
# define SSL_ENC_SEED_IDX        11
# define SSL_ENC_AES128GCM_IDX   12
# define SSL_ENC_AES256GCM_IDX   13
# define SSL_ENC_AES128CCM_IDX   14
# define SSL_ENC_AES256CCM_IDX   15
# define SSL_ENC_AES128CCM8_IDX  16
# define SSL_ENC_AES256CCM8_IDX  17
# define SSL_ENC_GOST8912_IDX    18
# define SSL_ENC_CHACHA_IDX      19
# define SSL_ENC_ARIA128GCM_IDX  20
# define SSL_ENC_ARIA256GCM_IDX  21
# define SSL_ENC_MAGMA_IDX       22
# define SSL_ENC_KUZNYECHIK_IDX  23
# define SSL_ENC_NUM_IDX         24

/*-
 * SSL_kRSA <- RSA_ENC
 * SSL_kDH  <- DH_ENC & (RSA_ENC | RSA_SIGN | DSA_SIGN)
 * SSL_kDHE <- RSA_ENC | RSA_SIGN | DSA_SIGN
 * SSL_aRSA <- RSA_ENC | RSA_SIGN
 * SSL_aDSS <- DSA_SIGN
 */

/* Certificate Type State */
# define OSSL_CERT_TYPE_CTOS_NONE    0
# define OSSL_CERT_TYPE_CTOS_GOOD    1
# define OSSL_CERT_TYPE_CTOS_ERROR   2

/* Post-Handshake Authentication state */
typedef enum {
    SSL_PHA_NONE = 0,
    SSL_PHA_EXT_SENT,        /* client-side only: extension sent */
    SSL_PHA_EXT_RECEIVED,    /* server-side only: extension received */
    SSL_PHA_REQUEST_PENDING, /* server-side only: request pending */
    SSL_PHA_REQUESTED        /* request received by client, or sent by server */
} SSL_PHA_STATE;

/* CipherSuite length. SSLv3 and all TLS versions. */
# define TLS_CIPHER_LEN 2
/* used to hold info on the particular ciphers used */
struct ssl_cipher_st {
    uint32_t valid;
    const char *name;           /* text name */
    const char *stdname;        /* RFC name */
    uint32_t id;                /* id, 4 bytes, first is version */
    /*
     * changed in 1.0.0: these four used to be portions of a single value
     * 'algorithms'
     */
    uint32_t algorithm_mkey;    /* key exchange algorithm */
    uint32_t algorithm_auth;    /* server authentication */
    uint32_t algorithm_enc;     /* symmetric encryption */
    uint32_t algorithm_mac;     /* symmetric authentication */
    int min_tls;                /* minimum SSL/TLS protocol version */
    int max_tls;                /* maximum SSL/TLS protocol version */
    int min_dtls;               /* minimum DTLS protocol version */
    int max_dtls;               /* maximum DTLS protocol version */
    uint32_t algo_strength;     /* strength and export flags */
    uint32_t algorithm2;        /* Extra flags */
    int32_t strength_bits;      /* Number of bits really used */
    uint32_t alg_bits;          /* Number of bits for algorithm */
};

/* Used to hold SSL/TLS functions */
struct ssl_method_st {
    int version;
    unsigned flags;
    uint64_t mask;
    SSL *(*ssl_new) (SSL_CTX *ctx);
    void (*ssl_free) (SSL *s);
    int (*ssl_reset) (SSL *s);
    int (*ssl_init) (SSL *s);
    int (*ssl_clear) (SSL *s);
    void (*ssl_deinit) (SSL *s);
    int (*ssl_accept) (SSL *s);
    int (*ssl_connect) (SSL *s);
    int (*ssl_read) (SSL *s, void *buf, size_t len, size_t *readbytes);
    int (*ssl_peek) (SSL *s, void *buf, size_t len, size_t *readbytes);
    int (*ssl_write) (SSL *s, const void *buf, size_t len, size_t *written);
    int (*ssl_shutdown) (SSL *s);
    int (*ssl_renegotiate) (SSL *s);
    int (*ssl_renegotiate_check) (SSL *s, int);
    int (*ssl_read_bytes) (SSL *s, uint8_t type, uint8_t *recvd_type,
                           unsigned char *buf, size_t len, int peek,
                           size_t *readbytes);
    int (*ssl_write_bytes) (SSL *s, uint8_t type, const void *buf_, size_t len,
                            size_t *written);
    int (*ssl_dispatch_alert) (SSL *s);
    long (*ssl_ctrl) (SSL *s, int cmd, long larg, void *parg);
    long (*ssl_ctx_ctrl) (SSL_CTX *ctx, int cmd, long larg, void *parg);
    const SSL_CIPHER *(*get_cipher_by_char) (const unsigned char *ptr);
    int (*put_cipher_by_char) (const SSL_CIPHER *cipher, WPACKET *pkt,
                               size_t *len);
    size_t (*ssl_pending) (const SSL *s);
    int (*num_ciphers) (void);
    const SSL_CIPHER *(*get_cipher) (unsigned ncipher);
    OSSL_TIME (*get_timeout) (void);
    const struct ssl3_enc_method *ssl3_enc; /* Extra SSLv3/TLS stuff */
    int (*ssl_version) (void);
    long (*ssl_callback_ctrl) (SSL *s, int cb_id, void (*fp) (void));
    long (*ssl_ctx_callback_ctrl) (SSL_CTX *s, int cb_id, void (*fp) (void));
};

/*
 * Matches the length of PSK_MAX_PSK_LEN. We keep it the same value for
 * consistency, even in the event of OPENSSL_NO_PSK being defined.
 */
# define TLS13_MAX_RESUMPTION_PSK_LENGTH      512

/*-
 * Lets make this into an ASN.1 type structure as follows
 * SSL_SESSION_ID ::= SEQUENCE {
 *      version                 INTEGER,        -- structure version number
 *      SSLversion              INTEGER,        -- SSL version number
 *      Cipher                  OCTET STRING,   -- the 3 byte cipher ID
 *      Session_ID              OCTET STRING,   -- the Session ID
 *      Master_key              OCTET STRING,   -- the master key
 *      Key_Arg [ 0 ] IMPLICIT  OCTET STRING,   -- the optional Key argument
 *      Time [ 1 ] EXPLICIT     INTEGER,        -- optional Start Time
 *      Timeout [ 2 ] EXPLICIT  INTEGER,        -- optional Timeout ins seconds
 *      Peer [ 3 ] EXPLICIT     X509,           -- optional Peer Certificate
 *      Session_ID_context [ 4 ] EXPLICIT OCTET STRING,   -- the Session ID context
 *      Verify_result [ 5 ] EXPLICIT INTEGER,   -- X509_V_... code for `Peer'
 *      HostName [ 6 ] EXPLICIT OCTET STRING,   -- optional HostName from servername TLS extension
 *      PSK_identity_hint [ 7 ] EXPLICIT OCTET STRING, -- optional PSK identity hint
 *      PSK_identity [ 8 ] EXPLICIT OCTET STRING,  -- optional PSK identity
 *      Ticket_lifetime_hint [9] EXPLICIT INTEGER, -- server's lifetime hint for session ticket
 *      Ticket [10]             EXPLICIT OCTET STRING, -- session ticket (clients only)
 *      Compression_meth [11]   EXPLICIT OCTET STRING, -- optional compression method
 *      SRP_username [ 12 ] EXPLICIT OCTET STRING -- optional SRP username
 *      flags [ 13 ] EXPLICIT INTEGER -- optional flags
 *      }
 * Look in ssl/ssl_asn1.c for more details
 * I'm using EXPLICIT tags so I can read the damn things using asn1parse :-).
 */
struct ssl_session_st {
    int ssl_version;            /* what ssl version session info is being kept
                                 * in here? */
    size_t master_key_length;

    /* TLSv1.3 early_secret used for external PSKs */
    unsigned char early_secret[EVP_MAX_MD_SIZE];
    /*
     * For <=TLS1.2 this is the master_key. For TLS1.3 this is the resumption
     * PSK
     */
    unsigned char master_key[TLS13_MAX_RESUMPTION_PSK_LENGTH];
    /* session_id - valid? */
    size_t session_id_length;
    unsigned char session_id[SSL_MAX_SSL_SESSION_ID_LENGTH];
    /*
     * this is used to determine whether the session is being reused in the
     * appropriate context. It is up to the application to set this, via
     * SSL_new
     */
    size_t sid_ctx_length;
    unsigned char sid_ctx[SSL_MAX_SID_CTX_LENGTH];
# ifndef OPENSSL_NO_PSK
    char *psk_identity_hint;
    char *psk_identity;
# endif
    /*
     * Used to indicate that session resumption is not allowed. Applications
     * can also set this bit for a new session via not_resumable_session_cb
     * to disable session caching and tickets.
     */
    int not_resumable;
    /* Peer raw public key, if available */
    EVP_PKEY *peer_rpk;
    /* This is the cert and type for the other end. */
    X509 *peer;
    /* Certificate chain peer sent. */
    STACK_OF(X509) *peer_chain;
    /*
     * when app_verify_callback accepts a session where the peer's
     * certificate is not ok, we must remember the error for session reuse:
     */
    long verify_result;         /* only for servers */
    OSSL_TIME timeout;
    OSSL_TIME time;
    OSSL_TIME calc_timeout;
    unsigned int compress_meth; /* Need to lookup the method */
    const SSL_CIPHER *cipher;
    unsigned long cipher_id;    /* when ASN.1 loaded, this needs to be used to
                                 * load the 'cipher' structure */
    unsigned int kex_group;      /* TLS group from key exchange */
    CRYPTO_EX_DATA ex_data;     /* application specific data */

    struct {
        char *hostname;
        /* RFC4507 info */
        unsigned char *tick; /* Session ticket */
        size_t ticklen;      /* Session ticket length */
        /* Session lifetime hint in seconds */
        unsigned long tick_lifetime_hint;
        uint32_t tick_age_add;
        /* Max number of bytes that can be sent as early data */
        uint32_t max_early_data;
        /* The ALPN protocol selected for this session */
        unsigned char *alpn_selected;
        size_t alpn_selected_len;
        /*
         * Maximum Fragment Length as per RFC 4366.
         * If this value does not contain RFC 4366 allowed values (1-4) then
         * either the Maximum Fragment Length Negotiation failed or was not
         * performed at all.
         */
        uint8_t max_fragment_len_mode;
    } ext;
# ifndef OPENSSL_NO_SRP
    char *srp_username;
# endif
    unsigned char *ticket_appdata;
    size_t ticket_appdata_len;
    uint32_t flags;
    SSL_CTX *owner;

    /*
     * These are used to make removal of session-ids more efficient and to
     * implement a maximum cache size. Access requires protection of ctx->lock.
     */
    struct ssl_session_st *prev, *next;
    CRYPTO_REF_COUNT references;
};

/* Extended master secret support */
# define SSL_SESS_FLAG_EXTMS             0x1

# ifndef OPENSSL_NO_SRP

typedef struct srp_ctx_st {
    /* param for all the callbacks */
    void *SRP_cb_arg;
    /* set client Hello login callback */
    int (*TLS_ext_srp_username_callback) (SSL *, int *, void *);
    /* set SRP N/g param callback for verification */
    int (*SRP_verify_param_callback) (SSL *, void *);
    /* set SRP client passwd callback */
    char *(*SRP_give_srp_client_pwd_callback) (SSL *, void *);
    char *login;
    BIGNUM *N, *g, *s, *B, *A;
    BIGNUM *a, *b, *v;
    char *info;
    int strength;
    unsigned long srp_Mask;
} SRP_CTX;

# endif

typedef enum {
    SSL_EARLY_DATA_NONE = 0,
    SSL_EARLY_DATA_CONNECT_RETRY,
    SSL_EARLY_DATA_CONNECTING,
    SSL_EARLY_DATA_WRITE_RETRY,
    SSL_EARLY_DATA_WRITING,
    SSL_EARLY_DATA_WRITE_FLUSH,
    SSL_EARLY_DATA_UNAUTH_WRITING,
    SSL_EARLY_DATA_FINISHED_WRITING,
    SSL_EARLY_DATA_ACCEPT_RETRY,
    SSL_EARLY_DATA_ACCEPTING,
    SSL_EARLY_DATA_READ_RETRY,
    SSL_EARLY_DATA_READING,
    SSL_EARLY_DATA_FINISHED_READING
} SSL_EARLY_DATA_STATE;

/*
 * We check that the amount of unreadable early data doesn't exceed
 * max_early_data. max_early_data is given in plaintext bytes. However if it is
 * unreadable then we only know the number of ciphertext bytes. We also don't
 * know how much the overhead should be because it depends on the ciphersuite.
 * We make a small allowance. We assume 5 records of actual data plus the end
 * of early data alert record. Each record has a tag and a content type byte.
 * The longest tag length we know of is EVP_GCM_TLS_TAG_LEN. We don't count the
 * content of the alert record either which is 2 bytes.
 */
# define EARLY_DATA_CIPHERTEXT_OVERHEAD ((6 * (EVP_GCM_TLS_TAG_LEN + 1)) + 2)

/*
 * The allowance we have between the client's calculated ticket age and our own.
 * We allow for 10 seconds. If a ticket is presented and the
 * client's age calculation is different by more than this than our own then we
 * do not allow that ticket for early_data.
 */
# define TICKET_AGE_ALLOWANCE   ossl_seconds2time(10)

#define MAX_COMPRESSIONS_SIZE   255


typedef struct raw_extension_st {
    /* Raw packet data for the extension */
    PACKET data;
    /* Set to 1 if the extension is present or 0 otherwise */
    int present;
    /* Set to 1 if we have already parsed the extension or 0 otherwise */
    int parsed;
    /* The type of this extension, i.e. a TLSEXT_TYPE_* value */
    unsigned int type;
    /* Track what order extensions are received in (0-based). */
    size_t received_order;
} RAW_EXTENSION;

typedef struct {
    unsigned int isv2;
    unsigned int legacy_version;
    unsigned char random[SSL3_RANDOM_SIZE];
    size_t session_id_len;
    unsigned char session_id[SSL_MAX_SSL_SESSION_ID_LENGTH];
    size_t dtls_cookie_len;
    unsigned char dtls_cookie[DTLS1_COOKIE_LENGTH];
    PACKET ciphersuites;
    size_t compressions_len;
    unsigned char compressions[MAX_COMPRESSIONS_SIZE];
    PACKET extensions;
    size_t pre_proc_exts_len;
    RAW_EXTENSION *pre_proc_exts;
} CLIENTHELLO_MSG;

/*
 * Extension index values NOTE: Any updates to these defines should be mirrored
 * with equivalent updates to ext_defs in extensions.c
 */
typedef enum tlsext_index_en {
    TLSEXT_IDX_renegotiate,
    TLSEXT_IDX_server_name,
    TLSEXT_IDX_max_fragment_length,
    TLSEXT_IDX_srp,
    TLSEXT_IDX_ec_point_formats,
    TLSEXT_IDX_supported_groups,
    TLSEXT_IDX_session_ticket,
    TLSEXT_IDX_status_request,
    TLSEXT_IDX_next_proto_neg,
    TLSEXT_IDX_application_layer_protocol_negotiation,
    TLSEXT_IDX_use_srtp,
    TLSEXT_IDX_encrypt_then_mac,
    TLSEXT_IDX_signed_certificate_timestamp,
    TLSEXT_IDX_extended_master_secret,
    TLSEXT_IDX_signature_algorithms_cert,
    TLSEXT_IDX_post_handshake_auth,
    TLSEXT_IDX_client_cert_type,
    TLSEXT_IDX_server_cert_type,
    TLSEXT_IDX_signature_algorithms,
    TLSEXT_IDX_supported_versions,
    TLSEXT_IDX_psk_kex_modes,
    TLSEXT_IDX_key_share,
    TLSEXT_IDX_cookie,
    TLSEXT_IDX_cryptopro_bug,
    TLSEXT_IDX_compress_certificate,
    TLSEXT_IDX_early_data,
    TLSEXT_IDX_certificate_authorities,
    TLSEXT_IDX_padding,
    TLSEXT_IDX_psk,
    /* Dummy index - must always be the last entry */
    TLSEXT_IDX_num_builtins
} TLSEXT_INDEX;

DEFINE_LHASH_OF_EX(SSL_SESSION);
/* Needed in ssl_cert.c */
DEFINE_LHASH_OF_EX(X509_NAME);

# define TLSEXT_KEYNAME_LENGTH  16
# define TLSEXT_TICK_KEY_LENGTH 32

typedef struct ssl_ctx_ext_secure_st {
    unsigned char tick_hmac_key[TLSEXT_TICK_KEY_LENGTH];
    unsigned char tick_aes_key[TLSEXT_TICK_KEY_LENGTH];
} SSL_CTX_EXT_SECURE;

/*
 * Helper function for HMAC
 * The structure should be considered opaque, it will change once the low
 * level deprecated calls are removed.  At that point it can be replaced
 * by EVP_MAC_CTX and most of the functions converted to macros or inlined
 * directly.
 */
typedef struct ssl_hmac_st {
    EVP_MAC_CTX *ctx;
# ifndef OPENSSL_NO_DEPRECATED_3_0
    HMAC_CTX *old_ctx;
# endif
} SSL_HMAC;

SSL_HMAC *ssl_hmac_new(const SSL_CTX *ctx);
void ssl_hmac_free(SSL_HMAC *ctx);
# ifndef OPENSSL_NO_DEPRECATED_3_0
HMAC_CTX *ssl_hmac_get0_HMAC_CTX(SSL_HMAC *ctx);
# endif
EVP_MAC_CTX *ssl_hmac_get0_EVP_MAC_CTX(SSL_HMAC *ctx);
int ssl_hmac_init(SSL_HMAC *ctx, void *key, size_t len, char *md);
int ssl_hmac_update(SSL_HMAC *ctx, const unsigned char *data, size_t len);
int ssl_hmac_final(SSL_HMAC *ctx, unsigned char *md, size_t *len,
                   size_t max_size);
size_t ssl_hmac_size(const SSL_HMAC *ctx);

int ssl_get_EC_curve_nid(const EVP_PKEY *pkey);
__owur int tls13_set_encoded_pub_key(EVP_PKEY *pkey,
                                     const unsigned char *enckey,
                                     size_t enckeylen);

typedef struct tls_group_info_st {
    char *tlsname;           /* Curve Name as in TLS specs */
    char *realname;          /* Curve Name according to provider */
    char *algorithm;         /* Algorithm name to fetch */
    unsigned int secbits;    /* Bits of security (from SP800-57) */
    uint16_t group_id;       /* Group ID */
    int mintls;              /* Minimum TLS version, -1 unsupported */
    int maxtls;              /* Maximum TLS version (or 0 for undefined) */
    int mindtls;             /* Minimum DTLS version, -1 unsupported */
    int maxdtls;             /* Maximum DTLS version (or 0 for undefined) */
    char is_kem;             /* Mode for this Group: 0 is KEX, 1 is KEM */
} TLS_GROUP_INFO;

typedef struct tls_sigalg_info_st {
    char *name;              /* name as in IANA TLS specs */
    uint16_t code_point;     /* IANA-specified code point of sigalg-name */
    char *sigalg_name;       /* (combined) sigalg name */
    char *sigalg_oid;        /* (combined) sigalg OID */
    char *sig_name;          /* pure signature algorithm name */
    char *sig_oid;           /* pure signature algorithm OID */
    char *hash_name;         /* hash algorithm name */
    char *hash_oid;          /* hash algorithm OID */
    char *keytype;           /* keytype name */
    char *keytype_oid;       /* keytype OID */
    unsigned int secbits;    /* Bits of security (from SP800-57) */
    int mintls;              /* Minimum TLS version, -1 unsupported */
    int maxtls;              /* Maximum TLS version (or 0 for undefined) */
    int mindtls;             /* Minimum DTLS version, -1 unsupported */
    int maxdtls;             /* Maximum DTLS version (or 0 for undefined) */
} TLS_SIGALG_INFO;

/*
 * Structure containing table entry of certificate info corresponding to
 * CERT_PKEY entries
 */
typedef struct {
    int nid; /* NID of public key algorithm */
    uint32_t amask; /* authmask corresponding to key type */
} SSL_CERT_LOOKUP;

/* flags values */
# define TLS_GROUP_TYPE             0x0000000FU /* Mask for group type */
# define TLS_GROUP_CURVE_PRIME      0x00000001U
# define TLS_GROUP_CURVE_CHAR2      0x00000002U
# define TLS_GROUP_CURVE_CUSTOM     0x00000004U
# define TLS_GROUP_FFDHE            0x00000008U
# define TLS_GROUP_ONLY_FOR_TLS1_3  0x00000010U

# define TLS_GROUP_FFDHE_FOR_TLS1_3 (TLS_GROUP_FFDHE|TLS_GROUP_ONLY_FOR_TLS1_3)

/* We limit the number of key shares sent */
# ifndef OPENSSL_CLIENT_MAX_KEY_SHARES
#  define OPENSSL_CLIENT_MAX_KEY_SHARES 4
# endif

struct ssl_ctx_st {
    OSSL_LIB_CTX *libctx;

    const SSL_METHOD *method;
    STACK_OF(SSL_CIPHER) *cipher_list;
    /* same as above but sorted for lookup */
    STACK_OF(SSL_CIPHER) *cipher_list_by_id;
    /* TLSv1.3 specific ciphersuites */
    STACK_OF(SSL_CIPHER) *tls13_ciphersuites;
    struct x509_store_st /* X509_STORE */ *cert_store;
    LHASH_OF(SSL_SESSION) *sessions;
    /*
     * Most session-ids that will be cached, default is
     * SSL_SESSION_CACHE_MAX_SIZE_DEFAULT. 0 is unlimited.
     */
    size_t session_cache_size;
    struct ssl_session_st *session_cache_head;
    struct ssl_session_st *session_cache_tail;
    /*
     * This can have one of 2 values, ored together, SSL_SESS_CACHE_CLIENT,
     * SSL_SESS_CACHE_SERVER, Default is SSL_SESSION_CACHE_SERVER, which
     * means only SSL_accept will cache SSL_SESSIONS.
     */
    uint32_t session_cache_mode;
    /*
     * If timeout is not 0, it is the default timeout value set when
     * SSL_new() is called.  This has been put in to make life easier to set
     * things up
     */
    OSSL_TIME session_timeout;
    /*
     * If this callback is not null, it will be called each time a session id
     * is added to the cache.  If this function returns 1, it means that the
     * callback will do an SSL_SESSION_free() when it has finished using it.
     * Otherwise, on 0, it means the callback has finished with it. If
     * remove_session_cb is not null, it will be called when a session-id is
     * removed from the cache.  After the call, OpenSSL will
     * SSL_SESSION_free() it.
     */
    int (*new_session_cb) (struct ssl_st *ssl, SSL_SESSION *sess);
    void (*remove_session_cb) (struct ssl_ctx_st *ctx, SSL_SESSION *sess);
    SSL_SESSION *(*get_session_cb) (struct ssl_st *ssl,
                                    const unsigned char *data, int len,
                                    int *copy);
    struct {
        TSAN_QUALIFIER int sess_connect;       /* SSL new conn - started */
        TSAN_QUALIFIER int sess_connect_renegotiate; /* SSL reneg - requested */
        TSAN_QUALIFIER int sess_connect_good;  /* SSL new conne/reneg - finished */
        TSAN_QUALIFIER int sess_accept;        /* SSL new accept - started */
        TSAN_QUALIFIER int sess_accept_renegotiate; /* SSL reneg - requested */
        TSAN_QUALIFIER int sess_accept_good;   /* SSL accept/reneg - finished */
        TSAN_QUALIFIER int sess_miss;          /* session lookup misses */
        TSAN_QUALIFIER int sess_timeout;       /* reuse attempt on timeouted session */
        TSAN_QUALIFIER int sess_cache_full;    /* session removed due to full cache */
        TSAN_QUALIFIER int sess_hit;           /* session reuse actually done */
        TSAN_QUALIFIER int sess_cb_hit;        /* session-id that was not in
                                                * the cache was passed back via
                                                * the callback. This indicates
                                                * that the application is
                                                * supplying session-id's from
                                                * other processes - spooky
                                                * :-) */
    } stats;
#ifdef TSAN_REQUIRES_LOCKING
    CRYPTO_RWLOCK *tsan_lock;
#endif

    CRYPTO_REF_COUNT references;

    /* if defined, these override the X509_verify_cert() calls */
    int (*app_verify_callback) (X509_STORE_CTX *, void *);
    void *app_verify_arg;
    /*
     * before OpenSSL 0.9.7, 'app_verify_arg' was ignored
     * ('app_verify_callback' was called with just one argument)
     */

    /* Default password callback. */
    pem_password_cb *default_passwd_callback;

    /* Default password callback user data. */
    void *default_passwd_callback_userdata;

    /* get client cert callback */
    int (*client_cert_cb) (SSL *ssl, X509 **x509, EVP_PKEY **pkey);

    /* cookie generate callback */
    int (*app_gen_cookie_cb) (SSL *ssl, unsigned char *cookie,
                              unsigned int *cookie_len);

    /* verify cookie callback */
    int (*app_verify_cookie_cb) (SSL *ssl, const unsigned char *cookie,
                                 unsigned int cookie_len);

    /* TLS1.3 app-controlled cookie generate callback */
    int (*gen_stateless_cookie_cb) (SSL *ssl, unsigned char *cookie,
                                    size_t *cookie_len);

    /* TLS1.3 verify app-controlled cookie callback */
    int (*verify_stateless_cookie_cb) (SSL *ssl, const unsigned char *cookie,
                                       size_t cookie_len);

    CRYPTO_EX_DATA ex_data;

    const EVP_MD *md5;          /* For SSLv3/TLSv1 'ssl3-md5' */
    const EVP_MD *sha1;         /* For SSLv3/TLSv1 'ssl3-sha1' */

    STACK_OF(X509) *extra_certs;
    STACK_OF(SSL_COMP) *comp_methods; /* stack of SSL_COMP, SSLv3/TLSv1 */

    /* Default values used when no per-SSL value is defined follow */

    /* used if SSL's info_callback is NULL */
    void (*info_callback) (const SSL *ssl, int type, int val);

    /*
     * What we put in certificate_authorities extension for TLS 1.3
     * (ClientHello and CertificateRequest) or just client cert requests for
     * earlier versions. If client_ca_names is populated then it is only used
     * for client cert requests, and in preference to ca_names.
     */
    STACK_OF(X509_NAME) *ca_names;
    STACK_OF(X509_NAME) *client_ca_names;

    /*
     * Default values to use in SSL structures follow (these are copied by
     * SSL_new)
     */

    uint64_t options;
    uint32_t mode;
    int min_proto_version;
    int max_proto_version;
    size_t max_cert_list;

    struct cert_st /* CERT */ *cert;
    SSL_CERT_LOOKUP *ssl_cert_info;
    int read_ahead;

    /* callback that allows applications to peek at protocol messages */
    ossl_msg_cb msg_callback;
    void *msg_callback_arg;

    uint32_t verify_mode;
    size_t sid_ctx_length;
    unsigned char sid_ctx[SSL_MAX_SID_CTX_LENGTH];
    /* called 'verify_callback' in the SSL */
    int (*default_verify_callback) (int ok, X509_STORE_CTX *ctx);

    /* Default generate session ID callback. */
    GEN_SESSION_CB generate_session_id;

    X509_VERIFY_PARAM *param;

    int quiet_shutdown;

# ifndef OPENSSL_NO_CT
    CTLOG_STORE *ctlog_store;   /* CT Log Store */
    /*
     * Validates that the SCTs (Signed Certificate Timestamps) are sufficient.
     * If they are not, the connection should be aborted.
     */
    ssl_ct_validation_cb ct_validation_callback;
    void *ct_validation_callback_arg;
# endif

    /*
     * If we're using more than one pipeline how should we divide the data
     * up between the pipes?
     */
    size_t split_send_fragment;
    /*
     * Maximum amount of data to send in one fragment. actual record size can
     * be more than this due to padding and MAC overheads.
     */
    size_t max_send_fragment;

    /* Up to how many pipelines should we use? If 0 then 1 is assumed */
    size_t max_pipelines;

    /* The default read buffer length to use (0 means not set) */
    size_t default_read_buf_len;

# ifndef OPENSSL_NO_ENGINE
    /*
     * Engine to pass requests for client certs to
     */
    ENGINE *client_cert_engine;
# endif

    /* ClientHello callback.  Mostly for extensions, but not entirely. */
    SSL_client_hello_cb_fn client_hello_cb;
    void *client_hello_cb_arg;

    /* Callback to announce new pending ssl objects in the accept queue */
    SSL_new_pending_conn_cb_fn new_pending_conn_cb;
    void *new_pending_conn_arg;

    /* TLS extensions. */
    struct {
        /* TLS extensions servername callback */
        int (*servername_cb) (SSL *, int *, void *);
        void *servername_arg;
        /* RFC 4507 session ticket keys */
        unsigned char tick_key_name[TLSEXT_KEYNAME_LENGTH];
        SSL_CTX_EXT_SECURE *secure;
# ifndef OPENSSL_NO_DEPRECATED_3_0
        /* Callback to support customisation of ticket key setting */
        int (*ticket_key_cb) (SSL *ssl,
                              unsigned char *name, unsigned char *iv,
                              EVP_CIPHER_CTX *ectx, HMAC_CTX *hctx, int enc);
#endif
        int (*ticket_key_evp_cb) (SSL *ssl,
                                  unsigned char *name, unsigned char *iv,
                                  EVP_CIPHER_CTX *ectx, EVP_MAC_CTX *hctx,
                                  int enc);

        /* certificate status request info */
        /* Callback for status request */
        int (*status_cb) (SSL *ssl, void *arg);
        void *status_arg;
        /* ext status type used for CSR extension (OCSP Stapling) */
        int status_type;
        /* RFC 4366 Maximum Fragment Length Negotiation */
        uint8_t max_fragment_len_mode;

        /* EC extension values inherited by SSL structure */
        size_t ecpointformats_len;
        unsigned char *ecpointformats;

        size_t supportedgroups_len;
        uint16_t *supportedgroups;

        size_t keyshares_len;
        uint16_t *keyshares;

        size_t tuples_len; /* Number of group tuples */
        size_t *tuples; /* Number of groups in each group tuple */

        /*
         * ALPN information (we are in the process of transitioning from NPN to
         * ALPN.)
         */

        /*-
         * For a server, this contains a callback function that allows the
         * server to select the protocol for the connection.
         *   out: on successful return, this must point to the raw protocol
         *        name (without the length prefix).
         *   outlen: on successful return, this contains the length of |*out|.
         *   in: points to the client's list of supported protocols in
         *       wire-format.
         *   inlen: the length of |in|.
         */
        int (*alpn_select_cb) (SSL *s,
                               const unsigned char **out,
                               unsigned char *outlen,
                               const unsigned char *in,
                               unsigned int inlen, void *arg);
        void *alpn_select_cb_arg;

        /*
         * For a client, this contains the list of supported protocols in wire
         * format.
         */
        unsigned char *alpn;
        size_t alpn_len;

# ifndef OPENSSL_NO_NEXTPROTONEG
        /* Next protocol negotiation information */

        /*
         * For a server, this contains a callback function by which the set of
         * advertised protocols can be provided.
         */
        SSL_CTX_npn_advertised_cb_func npn_advertised_cb;
        void *npn_advertised_cb_arg;
        /*
         * For a client, this contains a callback function that selects the next
         * protocol from the list provided by the server.
         */
        SSL_CTX_npn_select_cb_func npn_select_cb;
        void *npn_select_cb_arg;
# endif

        unsigned char cookie_hmac_key[SHA256_DIGEST_LENGTH];
    } ext;

# ifndef OPENSSL_NO_PSK
    SSL_psk_client_cb_func psk_client_callback;
    SSL_psk_server_cb_func psk_server_callback;
# endif
    SSL_psk_find_session_cb_func psk_find_session_cb;
    SSL_psk_use_session_cb_func psk_use_session_cb;

# ifndef OPENSSL_NO_SRP
    SRP_CTX srp_ctx;            /* ctx for SRP authentication */
# endif

    /* Shared DANE context */
    struct dane_ctx_st dane;

# ifndef OPENSSL_NO_SRTP
    /* SRTP profiles we are willing to do from RFC 5764 */
    STACK_OF(SRTP_PROTECTION_PROFILE) *srtp_profiles;
# endif
    /*
     * Callback for disabling session caching and ticket support on a session
     * basis, depending on the chosen cipher.
     */
    int (*not_resumable_session_cb) (SSL *ssl, int is_forward_secure);

    CRYPTO_RWLOCK *lock;

    /*
     * Callback for logging key material for use with debugging tools like
     * Wireshark. The callback should log `line` followed by a newline.
     */
    SSL_CTX_keylog_cb_func keylog_callback;

    /*
     * Private flag for internal key logging based on SSLKEYLOG env
     */
# ifndef OPENSSL_NO_SSLKEYLOG
    uint32_t do_sslkeylog;
# endif

    /*
     * The maximum number of bytes advertised in session tickets that can be
     * sent as early data.
     */
    uint32_t max_early_data;

    /*
     * The maximum number of bytes of early data that a server will tolerate
     * (which should be at least as much as max_early_data).
     */
    uint32_t recv_max_early_data;

    /* TLS1.3 padding callback */
    size_t (*record_padding_cb)(SSL *s, int type, size_t len, void *arg);
    void *record_padding_arg;
    size_t block_padding;
    size_t hs_padding;

    /* Session ticket appdata */
    SSL_CTX_generate_session_ticket_fn generate_ticket_cb;
    SSL_CTX_decrypt_session_ticket_fn decrypt_ticket_cb;
    void *ticket_cb_data;

    /* The number of TLS1.3 tickets to automatically send */
    size_t num_tickets;

    /* Callback to determine if early_data is acceptable or not */
    SSL_allow_early_data_cb_fn allow_early_data_cb;
    void *allow_early_data_cb_data;

    /* Do we advertise Post-handshake auth support? */
    int pha_enabled;

    /* Callback for SSL async handling */
    SSL_async_callback_fn async_cb;
    void *async_cb_arg;

    char *propq;

    int ssl_mac_pkey_id[SSL_MD_NUM_IDX];
    const EVP_CIPHER *ssl_cipher_methods[SSL_ENC_NUM_IDX];
    const EVP_MD *ssl_digest_methods[SSL_MD_NUM_IDX];
    size_t ssl_mac_secret_size[SSL_MD_NUM_IDX];

    size_t sigalg_lookup_cache_len;
    size_t tls12_sigalgs_len;
    /* Cache of all sigalgs we know and whether they are available or not */
    struct sigalg_lookup_st *sigalg_lookup_cache;
    /* List of all sigalgs (code points) available, incl. from providers */
    uint16_t *tls12_sigalgs;

    TLS_GROUP_INFO *group_list;
    size_t group_list_len;
    size_t group_list_max_len;

    TLS_SIGALG_INFO *sigalg_list;
    size_t sigalg_list_len;
    size_t sigalg_list_max_len;

    /* masks of disabled algorithms */
    uint32_t disabled_enc_mask;
    uint32_t disabled_mac_mask;
    uint32_t disabled_mkey_mask;
    uint32_t disabled_auth_mask;

#ifndef OPENSSL_NO_COMP_ALG
    /* certificate compression preferences */
    int cert_comp_prefs[TLSEXT_comp_cert_limit];
#endif

    /* Certificate Type stuff - for RPK vs X.509 */
    unsigned char *client_cert_type;
    size_t client_cert_type_len;
    unsigned char *server_cert_type;
    size_t server_cert_type_len;

# ifndef OPENSSL_NO_QUIC
    uint64_t domain_flags;
    SSL_TOKEN_STORE *tokencache;
# endif

# ifndef OPENSSL_NO_QLOG
    char *qlog_title; /* Session title for qlog */
# endif
};

typedef struct ossl_quic_tls_callbacks_st {
    int (*crypto_send_cb)(SSL *s, const unsigned char *buf, size_t buf_len,
                          size_t *consumed, void *arg);
    int (*crypto_recv_rcd_cb)(SSL *s, const unsigned char **buf,
                              size_t *bytes_read, void *arg);
    int (*crypto_release_rcd_cb)(SSL *s, size_t bytes_read, void *arg);
    int (*yield_secret_cb)(SSL *s, uint32_t prot_level, int direction,
                           const unsigned char *secret, size_t secret_len,
                           void *arg);
    int (*got_transport_params_cb)(SSL *s, const unsigned char *params,
                                   size_t params_len,
                                   void *arg);
    int (*alert_cb)(SSL *s, unsigned char alert_code, void *arg);
} OSSL_QUIC_TLS_CALLBACKS;

typedef struct cert_pkey_st CERT_PKEY;

#define SSL_TYPE_SSL_CONNECTION     0
#define SSL_TYPE_QUIC_CONNECTION    0x80
#define SSL_TYPE_QUIC_XSO           0x81
#define SSL_TYPE_QUIC_LISTENER      0x82
#define SSL_TYPE_QUIC_DOMAIN        0x83

#define SSL_TYPE_IS_QUIC(x)         (((x) & 0x80) != 0)

struct ssl_st {
    int type;
    SSL_CTX *ctx;
    const SSL_METHOD *defltmeth;
    const SSL_METHOD *method;
    CRYPTO_REF_COUNT references;
    CRYPTO_RWLOCK *lock;
    /* extra application data */
    CRYPTO_EX_DATA ex_data;
};

struct ssl_connection_st {
    /* type identifier and common data */
    struct ssl_st ssl;

    /*
     * The actual end user's SSL object. Could be different to this one for
     * QUIC
     */
    SSL *user_ssl;

    /*
     * protocol version (one of SSL2_VERSION, SSL3_VERSION, TLS1_VERSION,
     * DTLS1_VERSION)
     */
    int version;
    /*
     * There are 2 BIO's even though they are normally both the same.  This
     * is so data can be read and written to different handlers
     */
    /* used by SSL_read */
    BIO *rbio;
    /* used by SSL_write */
    BIO *wbio;
    /* used during session-id reuse to concatenate messages */
    BIO *bbio;
    /*
     * This holds a variable that indicates what we were doing when a 0 or -1
     * is returned.  This is needed for non-blocking IO so we know what
     * request needs re-doing when in SSL_accept or SSL_connect
     */
    int rwstate;
    int (*handshake_func) (SSL *);
    /*
     * Imagine that here's a boolean member "init" that is switched as soon
     * as SSL_set_{accept/connect}_state is called for the first time, so
     * that "state" and "handshake_func" are properly initialized.  But as
     * handshake_func is == 0 until then, we use this test instead of an
     * "init" member.
     */
    /* are we the server side? */
    int server;
    /*
     * Generate a new session or reuse an old one.
     * NB: For servers, the 'new' session may actually be a previously
     * cached session or even the previous session unless
     * SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION is set
     */
    int new_session;
    /* don't send shutdown packets */
    int quiet_shutdown;
    /* we have shut things down, 0x01 sent, 0x02 for received */
    int shutdown;
    /* Timestamps used to calculate the handshake RTT */
    OSSL_TIME ts_msg_write;
    OSSL_TIME ts_msg_read;
    /* where we are */
    OSSL_STATEM statem;
    SSL_EARLY_DATA_STATE early_data_state;
    BUF_MEM *init_buf;          /* buffer used during init */
    void *init_msg;             /* pointer to handshake message body, set by
                                 * tls_get_message_header() */
    size_t init_num;               /* amount read/written */
    size_t init_off;               /* amount read/written */

    size_t ssl_pkey_num;

    /* QUIC TLS fields */
    OSSL_QUIC_TLS_CALLBACKS qtcb;
    void *qtarg;
    QUIC_TLS *qtls;

    struct {
        long flags;
        unsigned char server_random[SSL3_RANDOM_SIZE];
        unsigned char client_random[SSL3_RANDOM_SIZE];

        /* used during startup, digest all incoming/outgoing packets */
        BIO *handshake_buffer;
        /*
         * When handshake digest is determined, buffer is hashed and
         * freed and MD_CTX for the required digest is stored here.
         */
        EVP_MD_CTX *handshake_dgst;
        /*
         * Set whenever an expected ChangeCipherSpec message is processed.
         * Unset when the peer's Finished message is received.
         * Unexpected ChangeCipherSpec messages trigger a fatal alert.
         */
        int change_cipher_spec;
        int warn_alert;
        int fatal_alert;
        /*
         * we allow one fatal and one warning alert to be outstanding, send close
         * alert via the warning alert
         */
        int alert_dispatch;
        unsigned char send_alert[2];
        /*
         * This flag is set when we should renegotiate ASAP, basically when there
         * is no more data in the read or write buffers
         */
        int renegotiate;
        int total_renegotiations;
        int num_renegotiations;
        int in_read_app_data;

        struct {
            /* actually only need to be 16+20 for SSLv3 and 12 for TLS */
            unsigned char finish_md[EVP_MAX_MD_SIZE * 2];
            size_t finish_md_len;
            unsigned char peer_finish_md[EVP_MAX_MD_SIZE * 2];
            size_t peer_finish_md_len;
            size_t message_size;
            int message_type;
            /* used to hold the new cipher we are going to use */
            const SSL_CIPHER *new_cipher;
            EVP_PKEY *pkey;         /* holds short lived key exchange key */
            /* holds the array of short lived key exchange key (pointers) */
            EVP_PKEY *ks_pkey[OPENSSL_CLIENT_MAX_KEY_SHARES];
            uint16_t ks_group_id[OPENSSL_CLIENT_MAX_KEY_SHARES]; /* The IDs of the keyshare keys */
            size_t num_ks_pkey; /* how many keyshares are there */
            /* used for certificate requests */
            int cert_req;
            /* Certificate types in certificate request message. */
            uint8_t *ctype;
            size_t ctype_len;
            /* Certificate authorities list peer sent */
            STACK_OF(X509_NAME) *peer_ca_names;
            size_t key_block_length;
            unsigned char *key_block;
            const EVP_CIPHER *new_sym_enc;
            const EVP_MD *new_hash;
            int new_mac_pkey_type;
            size_t new_mac_secret_size;
# ifndef OPENSSL_NO_COMP
            const SSL_COMP *new_compression;
# else
            char *new_compression;
# endif
            int cert_request;
            /* Raw values of the cipher list from a client */
            unsigned char *ciphers_raw;
            size_t ciphers_rawlen;
            /* Temporary storage for premaster secret */
            unsigned char *pms;
            size_t pmslen;
# ifndef OPENSSL_NO_PSK
            /* Temporary storage for PSK key */
            unsigned char *psk;
            size_t psklen;
# endif
            /* Signature algorithm we actually use */
            const struct sigalg_lookup_st *sigalg;
            /* Pointer to certificate we use */
            CERT_PKEY *cert;
            /*
             * signature algorithms peer reports: e.g. supported signature
             * algorithms extension for server or as part of a certificate
             * request for client.
             * Keep track of the algorithms for TLS and X.509 usage separately.
             */
            uint16_t *peer_sigalgs;
            uint16_t *peer_cert_sigalgs;
            /* Size of above arrays */
            size_t peer_sigalgslen;
            size_t peer_cert_sigalgslen;
            /* Sigalg peer actually uses */
            const struct sigalg_lookup_st *peer_sigalg;
            /*
             * Set if corresponding CERT_PKEY can be used with current
             * SSL session: e.g. appropriate curve, signature algorithms etc.
             * If zero it can't be used at all.
             */
            uint32_t *valid_flags;
            /*
             * For servers the following masks are for the key and auth algorithms
             * that are supported by the certs below. For clients they are masks of
             * *disabled* algorithms based on the current session.
             */
            uint32_t mask_k;
            uint32_t mask_a;
            /*
             * The following are used by the client to see if a cipher is allowed or
             * not.  It contains the minimum and maximum version the client's using
             * based on what it knows so far.
             */
            int min_ver;
            int max_ver;
        } tmp;

        /* Connection binding to prevent renegotiation attacks */
        unsigned char previous_client_finished[EVP_MAX_MD_SIZE];
        size_t previous_client_finished_len;
        unsigned char previous_server_finished[EVP_MAX_MD_SIZE];
        size_t previous_server_finished_len;
        int send_connection_binding;

# ifndef OPENSSL_NO_NEXTPROTONEG
        /*
         * Set if we saw the Next Protocol Negotiation extension from our peer.
         */
        int npn_seen;
# endif

        /*
         * ALPN information (we are in the process of transitioning from NPN to
         * ALPN.)
         */

        /*
         * In a server these point to the selected ALPN protocol after the
         * ClientHello has been processed. In a client these contain the protocol
         * that the server selected once the ServerHello has been processed.
         */
        unsigned char *alpn_selected;
        size_t alpn_selected_len;
        /* used by the server to know what options were proposed */
        unsigned char *alpn_proposed;
        size_t alpn_proposed_len;
        /* used by the client to know if it actually sent alpn */
        int alpn_sent;

        /*
         * This is set to true if we believe that this is a version of Safari
         * running on OS X 10.6 or newer. We wish to know this because Safari on
         * 10.8 .. 10.8.3 has broken ECDHE-ECDSA support.
         */
        char is_probably_safari;

        /*
         * Track whether we did a key exchange this handshake or not, so
         * SSL_get_negotiated_group() knows whether to fall back to the
         * value in the SSL_SESSION.
         */
        char did_kex;
        /* For clients: peer temporary key */
        /* The group_id for the key exchange key */
        uint16_t group_id;
        EVP_PKEY *peer_tmp;
        /* The cached group_id candidate for the key exchange key */
        uint16_t group_id_candidate;
    } s3;

    struct dtls1_state_st *d1;  /* DTLSv1 variables */
    /* callback that allows applications to peek at protocol messages */
    void (*msg_callback) (int write_p, int version, int content_type,
                          const void *buf, size_t len, SSL *ssl, void *arg);
    void *msg_callback_arg;
    int hit;                    /* reusing a previous session */
    X509_VERIFY_PARAM *param;
    /* Per connection DANE state */
    SSL_DANE dane;
    /* crypto */
    STACK_OF(SSL_CIPHER) *peer_ciphers;
    STACK_OF(SSL_CIPHER) *cipher_list;
    STACK_OF(SSL_CIPHER) *cipher_list_by_id;
    /* TLSv1.3 specific ciphersuites */
    STACK_OF(SSL_CIPHER) *tls13_ciphersuites;
    /*
     * These are the ones being used, the ones in SSL_SESSION are the ones to
     * be 'copied' into these ones
     */
    uint32_t mac_flags;
    /*
     * The TLS1.3 secrets.
     */
    unsigned char early_secret[EVP_MAX_MD_SIZE];
    unsigned char handshake_secret[EVP_MAX_MD_SIZE];
    unsigned char master_secret[EVP_MAX_MD_SIZE];
    unsigned char resumption_master_secret[EVP_MAX_MD_SIZE];
    unsigned char client_finished_secret[EVP_MAX_MD_SIZE];
    unsigned char server_finished_secret[EVP_MAX_MD_SIZE];
    unsigned char server_finished_hash[EVP_MAX_MD_SIZE];
    unsigned char handshake_traffic_hash[EVP_MAX_MD_SIZE];
    unsigned char client_app_traffic_secret[EVP_MAX_MD_SIZE];
    unsigned char server_app_traffic_secret[EVP_MAX_MD_SIZE];
    unsigned char exporter_master_secret[EVP_MAX_MD_SIZE];
    unsigned char early_exporter_master_secret[EVP_MAX_MD_SIZE];

    /* session info */
    /* client cert? */
    /* This is used to hold the server certificate used */
    struct cert_st /* CERT */ *cert;

    /*
     * The hash of all messages prior to the CertificateVerify, and the length
     * of that hash.
     */
    unsigned char cert_verify_hash[EVP_MAX_MD_SIZE];
    size_t cert_verify_hash_len;

    /* Flag to indicate whether we should send a HelloRetryRequest or not */
    enum {SSL_HRR_NONE = 0, SSL_HRR_PENDING, SSL_HRR_COMPLETE}
        hello_retry_request;

    /*
     * the session_id_context is used to ensure sessions are only reused in
     * the appropriate context
     */
    size_t sid_ctx_length;
    unsigned char sid_ctx[SSL_MAX_SID_CTX_LENGTH];
    /* This can also be in the session once a session is established */
    SSL_SESSION *session;
    /* TLSv1.3 PSK session */
    SSL_SESSION *psksession;
    unsigned char *psksession_id;
    size_t psksession_id_len;
    /* Default generate session ID callback. */
    GEN_SESSION_CB generate_session_id;
    /*
     * The temporary TLSv1.3 session id. This isn't really a session id at all
     * but is a random value sent in the legacy session id field.
     */
    unsigned char tmp_session_id[SSL_MAX_SSL_SESSION_ID_LENGTH];
    size_t tmp_session_id_len;
    /* Used in SSL3 */
    /*
     * 0 don't care about verify failure.
     * 1 fail if verify fails
     */
    uint32_t verify_mode;
    /* fail if callback returns 0 */
    int (*verify_callback) (int ok, X509_STORE_CTX *ctx);
    /* optional informational callback */
    void (*info_callback) (const SSL *ssl, int type, int val);
    /* error bytes to be written */
    int error;
    /* actual code */
    int error_code;
# ifndef OPENSSL_NO_PSK
    SSL_psk_client_cb_func psk_client_callback;
    SSL_psk_server_cb_func psk_server_callback;
# endif
    SSL_psk_find_session_cb_func psk_find_session_cb;
    SSL_psk_use_session_cb_func psk_use_session_cb;

    /* Verified chain of peer */
    STACK_OF(X509) *verified_chain;
    long verify_result;
    /*
     * What we put in certificate_authorities extension for TLS 1.3
     * (ClientHello and CertificateRequest) or just client cert requests for
     * earlier versions. If client_ca_names is populated then it is only used
     * for client cert requests, and in preference to ca_names.
     */
    STACK_OF(X509_NAME) *ca_names;
    STACK_OF(X509_NAME) *client_ca_names;
    /* protocol behaviour */
    uint64_t options;
    /* API behaviour */
    uint32_t mode;
    int min_proto_version;
    int max_proto_version;
    size_t max_cert_list;
    int first_packet;
    /*
     * What was passed in ClientHello.legacy_version. Used for RSA pre-master
     * secret and SSLv3/TLS (<=1.2) rollback check
     */
    int client_version;
    /*
     * If we're using more than one pipeline how should we divide the data
     * up between the pipes?
     */
    size_t split_send_fragment;
    /*
     * Maximum amount of data to send in one fragment. actual record size can
     * be more than this due to padding and MAC overheads.
     */
    size_t max_send_fragment;
    /* Up to how many pipelines should we use? If 0 then 1 is assumed */
    size_t max_pipelines;

    struct {
        /* Built-in extension flags */
        uint8_t extflags[TLSEXT_IDX_num_builtins];
        /* TLS extension debug callback */
        void (*debug_cb)(SSL *s, int client_server, int type,
                         const unsigned char *data, int len, void *arg);
        void *debug_arg;
        char *hostname;
        /* certificate status request info */
        /* Status type or -1 if no status type */
        int status_type;
        /* Raw extension data, if seen */
        unsigned char *scts;
        /* Length of raw extension data, if seen */
        uint16_t scts_len;
        /* Expect OCSP CertificateStatus message */
        int status_expected;

        struct {
            /* OCSP status request only */
            STACK_OF(OCSP_RESPID) *ids;
            X509_EXTENSIONS *exts;
            /* OCSP response received or to be sent */
            unsigned char *resp;
            size_t resp_len;
        } ocsp;

        /* RFC4507 session ticket expected to be received or sent */
        int ticket_expected;
        /* TLS 1.3 tickets requested by the application. */
        int extra_tickets_expected;

        /* our list */
        size_t ecpointformats_len;
        unsigned char *ecpointformats;
        /* peer's list */
        size_t peer_ecpointformats_len;
        unsigned char *peer_ecpointformats;

        /* our list */
        size_t supportedgroups_len;
        uint16_t *supportedgroups;
        /* peer's list */
        size_t peer_supportedgroups_len;
        uint16_t *peer_supportedgroups;

        /* key shares */
        size_t keyshares_len;
        uint16_t *keyshares;
        /* supported groups tuples */
        size_t tuples_len;
        size_t *tuples;

        /* TLS Session Ticket extension override */
        TLS_SESSION_TICKET_EXT *session_ticket;
        /* TLS Session Ticket extension callback */
        tls_session_ticket_ext_cb_fn session_ticket_cb;
        void *session_ticket_cb_arg;
        /* TLS pre-shared secret session resumption */
        tls_session_secret_cb_fn session_secret_cb;
        void *session_secret_cb_arg;
        /*
         * For a client, this contains the list of supported protocols in wire
         * format.
         */
        unsigned char *alpn;
        size_t alpn_len;
        /*
         * Next protocol negotiation. For the client, this is the protocol that
         * we sent in NextProtocol and is set when handling ServerHello
         * extensions. For a server, this is the client's selected_protocol from
         * NextProtocol and is set when handling the NextProtocol message, before
         * the Finished message.
         */
        unsigned char *npn;
        size_t npn_len;

        /* The available PSK key exchange modes */
        int psk_kex_mode;

        /* Set to one if we have negotiated ETM */
        int use_etm;

        /* Are we expecting to receive early data? */
        int early_data;
        /* Is the session suitable for early data? */
        int early_data_ok;

        /* May be sent by a server in HRR. Must be echoed back in ClientHello */
        unsigned char *tls13_cookie;
        size_t tls13_cookie_len;
        /* Have we received a cookie from the client? */
        int cookieok;

        /*
         * Maximum Fragment Length as per RFC 4366.
         * If this member contains one of the allowed values (1-4)
         * then we should include Maximum Fragment Length Negotiation
         * extension in Client Hello.
         * Please note that value of this member does not have direct
         * effect. The actual (binding) value is stored in SSL_SESSION,
         * as this extension is optional on server side.
         */
        uint8_t max_fragment_len_mode;

        /*
         * On the client side the number of ticket identities we sent in the
         * ClientHello. On the server side the identity of the ticket we
         * selected.
         */
        int tick_identity;

        /* This is the list of algorithms the peer supports that we also support */
        int compress_certificate_from_peer[TLSEXT_comp_cert_limit];
        /* indicate that we sent the extension, so we'll accept it */
        int compress_certificate_sent;

        uint8_t client_cert_type;
        uint8_t client_cert_type_ctos;
        uint8_t server_cert_type;
        uint8_t server_cert_type_ctos;
    } ext;

    /*
     * Parsed form of the ClientHello, kept around across client_hello_cb
     * calls.
     */
    CLIENTHELLO_MSG *clienthello;

    /*-
     * no further mod of servername
     * 0 : call the servername extension callback.
     * 1 : prepare 2, allow last ack just after in server callback.
     * 2 : don't call servername callback, no ack in server hello
     */
    int servername_done;
# ifndef OPENSSL_NO_CT
    /*
     * Validates that the SCTs (Signed Certificate Timestamps) are sufficient.
     * If they are not, the connection should be aborted.
     */
    ssl_ct_validation_cb ct_validation_callback;
    /* User-supplied argument that is passed to the ct_validation_callback */
    void *ct_validation_callback_arg;
    /*
     * Consolidated stack of SCTs from all sources.
     * Lazily populated by CT_get_peer_scts(SSL*)
     */
    STACK_OF(SCT) *scts;
    /* Have we attempted to find/parse SCTs yet? */
    int scts_parsed;
# endif
    SSL_CTX *session_ctx;       /* initial ctx, used to store sessions */
# ifndef OPENSSL_NO_SRTP
    /* What we'll do */
    STACK_OF(SRTP_PROTECTION_PROFILE) *srtp_profiles;
    /* What's been chosen */
    SRTP_PROTECTION_PROFILE *srtp_profile;
# endif
    /*-
     * 1 if we are renegotiating.
     * 2 if we are a server and are inside a handshake
     * (i.e. not just sending a HelloRequest)
     */
    int renegotiate;
    /* If sending a KeyUpdate is pending */
    int key_update;
    /* Post-handshake authentication state */
    SSL_PHA_STATE post_handshake_auth;
    int pha_enabled;
    uint8_t* pha_context;
    size_t pha_context_len;
    int certreqs_sent;
    EVP_MD_CTX *pha_dgst; /* this is just the digest through ClientFinished */

# ifndef OPENSSL_NO_SRP
    /* ctx for SRP authentication */
    SRP_CTX srp_ctx;
# endif
    /*
     * Callback for disabling session caching and ticket support on a session
     * basis, depending on the chosen cipher.
     */
    int (*not_resumable_session_cb) (SSL *ssl, int is_forward_secure);

    /* Record layer data */
    RECORD_LAYER rlayer;

    /* Default password callback. */
    pem_password_cb *default_passwd_callback;
    /* Default password callback user data. */
    void *default_passwd_callback_userdata;
    /* Async Job info */
    ASYNC_JOB *job;
    ASYNC_WAIT_CTX *waitctx;
    size_t asyncrw;

    /*
     * The maximum number of bytes advertised in session tickets that can be
     * sent as early data.
     */
    uint32_t max_early_data;
    /*
     * The maximum number of bytes of early data that a server will tolerate
     * (which should be at least as much as max_early_data).
     */
    uint32_t recv_max_early_data;

    /*
     * The number of bytes of early data received so far. If we accepted early
     * data then this is a count of the plaintext bytes. If we rejected it then
     * this is a count of the ciphertext bytes.
     */
    uint32_t early_data_count;

    /* The number of TLS1.3 tickets to automatically send */
    size_t num_tickets;
    /* The number of TLS1.3 tickets actually sent so far */
    size_t sent_tickets;
    /* The next nonce value to use when we send a ticket on this connection */
    uint64_t next_ticket_nonce;

    /* Callback to determine if early_data is acceptable or not */
    SSL_allow_early_data_cb_fn allow_early_data_cb;
    void *allow_early_data_cb_data;

    /* Callback for SSL async handling */
    SSL_async_callback_fn async_cb;
    void *async_cb_arg;

    /*
     * Signature algorithms shared by client and server: cached because these
     * are used most often.
     */
    const struct sigalg_lookup_st **shared_sigalgs;
    size_t shared_sigalgslen;

#ifndef OPENSSL_NO_COMP_ALG
    /* certificate compression preferences */
    int cert_comp_prefs[TLSEXT_comp_cert_limit];
#endif

    /* Certificate Type stuff - for RPK vs X.509 */
    unsigned char *client_cert_type;
    size_t client_cert_type_len;
    unsigned char *server_cert_type;
    size_t server_cert_type_len;
};

/*
 * Structure containing table entry of values associated with the signature
 * algorithms (signature scheme) extension
*/
typedef struct sigalg_lookup_st {
    /* TLS 1.3 signature scheme name */
    const char *name;
    /* TLS 1.2 signature scheme name */
    const char *name12;
    /* Raw value used in extension */
    uint16_t sigalg;
    /* NID of hash algorithm or NID_undef if no hash */
    int hash;
    /* Index of hash algorithm or -1 if no hash algorithm */
    int hash_idx;
    /* NID of signature algorithm */
    int sig;
    /* Index of signature algorithm */
    int sig_idx;
    /* Combined hash and signature NID, if any */
    int sigandhash;
    /* Required public key curve (ECDSA only) */
    int curve;
    /* Whether this signature algorithm is actually available for use */
    int available;
    /* Whether this signature algorithm is by default advertised */
    int advertise;
    /* Supported protocol ranges */
    int mintls;
    int maxtls;
    int mindtls;
    int maxdtls;
} SIGALG_LOOKUP;

/* DTLS structures */

# ifndef OPENSSL_NO_SCTP
#  define DTLS1_SCTP_AUTH_LABEL   "EXPORTER_DTLS_OVER_SCTP"
# endif

/* Max MTU overhead we know about so far is 40 for IPv6 + 8 for UDP */
# define DTLS1_MAX_MTU_OVERHEAD                   48

struct dtls1_retransmit_state {
    const OSSL_RECORD_METHOD *wrlmethod;
    OSSL_RECORD_LAYER *wrl;
};

struct hm_header_st {
    unsigned char type;
    size_t msg_len;
    unsigned short seq;
    size_t frag_off;
    size_t frag_len;
    unsigned int is_ccs;
    struct dtls1_retransmit_state saved_retransmit_state;
};

typedef struct hm_fragment_st {
    struct hm_header_st msg_header;
    unsigned char *fragment;
    unsigned char *reassembly;
} hm_fragment;

typedef struct pqueue_st pqueue;
typedef struct pitem_st pitem;

struct pitem_st {
    unsigned char priority[8];  /* 64-bit value in big-endian encoding */
    void *data;
    pitem *next;
};

typedef struct pitem_st *piterator;

pitem *pitem_new(unsigned char *prio64be, void *data);
void pitem_free(pitem *item);
pqueue *pqueue_new(void);
void pqueue_free(pqueue *pq);
pitem *pqueue_insert(pqueue *pq, pitem *item);
pitem *pqueue_peek(pqueue *pq);
pitem *pqueue_pop(pqueue *pq);
pitem *pqueue_find(pqueue *pq, unsigned char *prio64be);
pitem *pqueue_iterator(pqueue *pq);
pitem *pqueue_next(piterator *iter);
size_t pqueue_size(pqueue *pq);

typedef struct dtls1_state_st {
    unsigned char cookie[DTLS1_COOKIE_LENGTH];
    size_t cookie_len;
    unsigned int cookie_verified;
    /* handshake message numbers */
    unsigned short handshake_write_seq;
    unsigned short next_handshake_write_seq;
    unsigned short handshake_read_seq;
    /* Buffered handshake messages */
    pqueue *buffered_messages;
    /* Buffered (sent) handshake records */
    pqueue *sent_messages;
    size_t link_mtu;      /* max on-the-wire DTLS packet size */
    size_t mtu;           /* max DTLS packet size */
    struct hm_header_st w_msg_hdr;
    struct hm_header_st r_msg_hdr;
    /* Number of alerts received so far */
    unsigned int timeout_num_alerts;
    /*
     * Indicates when the last handshake msg sent will timeout
     */
    OSSL_TIME next_timeout;
    /* Timeout duration */
    unsigned int timeout_duration_us;

    unsigned int retransmitting;
# ifndef OPENSSL_NO_SCTP
    int shutdown_received;
# endif

    DTLS_timer_cb timer_cb;

} DTLS1_STATE;

/*
 * From ECC-TLS draft, used in encoding the curve type in ECParameters
 */
#  define EXPLICIT_PRIME_CURVE_TYPE  1
#  define EXPLICIT_CHAR2_CURVE_TYPE  2
#  define NAMED_CURVE_TYPE           3

# ifndef OPENSSL_NO_COMP_ALG
struct ossl_comp_cert_st {
    unsigned char *data;
    size_t len;
    size_t orig_len;
    CRYPTO_REF_COUNT references;
    int alg;
};
typedef struct ossl_comp_cert_st OSSL_COMP_CERT;

void OSSL_COMP_CERT_free(OSSL_COMP_CERT *c);
int OSSL_COMP_CERT_up_ref(OSSL_COMP_CERT *c);
# endif

struct cert_pkey_st {
    X509 *x509;
    EVP_PKEY *privatekey;
    /* Chain for this certificate */
    STACK_OF(X509) *chain;
    /*-
     * serverinfo data for this certificate.  The data is in TLS Extension
     * wire format, specifically it's a series of records like:
     *   uint16_t extension_type; // (RFC 5246, 7.4.1.4, Extension)
     *   uint16_t length;
     *   uint8_t data[length];
     */
    unsigned char *serverinfo;
    size_t serverinfo_length;
# ifndef OPENSSL_NO_COMP_ALG
    /* Compressed certificate data - index 0 is unused */
    OSSL_COMP_CERT *comp_cert[TLSEXT_comp_cert_limit];
    int cert_comp_used;
# endif
};
/* Retrieve Suite B flags */
# define tls1_suiteb(s)  (s->cert->cert_flags & SSL_CERT_FLAG_SUITEB_128_LOS)
/* Uses to check strict mode: suite B modes are always strict */
# define SSL_CERT_FLAGS_CHECK_TLS_STRICT \
        (SSL_CERT_FLAG_SUITEB_128_LOS|SSL_CERT_FLAG_TLS_STRICT)

typedef enum {
    ENDPOINT_CLIENT = 0,
    ENDPOINT_SERVER,
    ENDPOINT_BOTH
} ENDPOINT;


typedef struct {
    unsigned short ext_type;
    ENDPOINT role;
    /* The context which this extension applies to */
    unsigned int context;
    /*
     * Per-connection flags relating to this extension type: not used if
     * part of an SSL_CTX structure.
     */
    uint32_t ext_flags;
    SSL_custom_ext_add_cb_ex add_cb;
    SSL_custom_ext_free_cb_ex free_cb;
    void *add_arg;
    SSL_custom_ext_parse_cb_ex parse_cb;
    void *parse_arg;
} custom_ext_method;

/* ext_flags values */

/*
 * Indicates an extension has been received. Used to check for unsolicited or
 * duplicate extensions.
 */
# define SSL_EXT_FLAG_RECEIVED   0x1
/*
 * Indicates an extension has been sent: used to enable sending of
 * corresponding ServerHello extension.
 */
# define SSL_EXT_FLAG_SENT       0x2
/*
 * Indicates an extension that was set on SSL object and needs to be
 * preserved when switching SSL contexts.
 */
# define SSL_EXT_FLAG_CONN       0x4

typedef struct {
    custom_ext_method *meths;
    size_t meths_count;
} custom_ext_methods;

typedef struct cert_st {
    /* Current active set */
    /*
     * ALWAYS points to an element of the pkeys array
     * Probably it would make more sense to store
     * an index, not a pointer.
     */
    CERT_PKEY *key;

    EVP_PKEY *dh_tmp;
    DH *(*dh_tmp_cb) (SSL *ssl, int is_export, int keysize);
    int dh_tmp_auto;
    /* Flags related to certificates */
    uint32_t cert_flags;
    CERT_PKEY *pkeys;
    size_t ssl_pkey_num;
    /* Custom certificate types sent in certificate request message. */
    uint8_t *ctype;
    size_t ctype_len;
    /*
     * supported signature algorithms. When set on a client this is sent in
     * the client hello as the supported signature algorithms extension. For
     * servers it represents the signature algorithms we are willing to use.
     */
    uint16_t *conf_sigalgs;
    /* Size of above array */
    size_t conf_sigalgslen;
    /*
     * Client authentication signature algorithms, if not set then uses
     * conf_sigalgs. On servers these will be the signature algorithms sent
     * to the client in a certificate request for TLS 1.2. On a client this
     * represents the signature algorithms we are willing to use for client
     * authentication.
     */
    uint16_t *client_sigalgs;
    /* Size of above array */
    size_t client_sigalgslen;
    /*
     * Certificate setup callback: if set is called whenever a certificate
     * may be required (client or server). the callback can then examine any
     * appropriate parameters and setup any certificates required. This
     * allows advanced applications to select certificates on the fly: for
     * example based on supported signature algorithms or curves.
     */
    int (*cert_cb) (SSL *ssl, void *arg);
    void *cert_cb_arg;
    /*
     * Optional X509_STORE for chain building or certificate validation If
     * NULL the parent SSL_CTX store is used instead.
     */
    X509_STORE *chain_store;
    X509_STORE *verify_store;
    /* Custom extensions */
    custom_ext_methods custext;
    /* Security callback */
    int (*sec_cb) (const SSL *s, const SSL_CTX *ctx, int op, int bits, int nid,
                   void *other, void *ex);
    /* Security level */
    int sec_level;
    void *sec_ex;
# ifndef OPENSSL_NO_PSK
    /* If not NULL psk identity hint to use for servers */
    char *psk_identity_hint;
# endif
    CRYPTO_REF_COUNT references;             /* >1 only if SSL_copy_session_id is used */
} CERT;

/*
 * This is for the SSLv3/TLSv1.0 differences in crypto/hash stuff It is a bit
 * of a mess of functions, but hell, think of it as an opaque structure :-)
 */
typedef struct ssl3_enc_method {
    int (*setup_key_block) (SSL_CONNECTION *);
    int (*generate_master_secret) (SSL_CONNECTION *, unsigned char *,
                                   unsigned char *, size_t, size_t *);
    int (*change_cipher_state) (SSL_CONNECTION *, int);
    size_t (*final_finish_mac) (SSL_CONNECTION *, const char *, size_t,
                                unsigned char *);
    const char *client_finished_label;
    size_t client_finished_label_len;
    const char *server_finished_label;
    size_t server_finished_label_len;
    int (*alert_value) (int);
    int (*export_keying_material) (SSL_CONNECTION *, unsigned char *, size_t,
                                   const char *, size_t,
                                   const unsigned char *, size_t,
                                   int use_context);
    /* Various flags indicating protocol version requirements */
    uint32_t enc_flags;
    /* Set the handshake header */
    int (*set_handshake_header) (SSL_CONNECTION *s, WPACKET *pkt, int type);
    /* Close construction of the handshake message */
    int (*close_construct_packet) (SSL_CONNECTION *s, WPACKET *pkt, int htype);
    /* Write out handshake message */
    int (*do_write) (SSL_CONNECTION *s);
} SSL3_ENC_METHOD;

# define ssl_set_handshake_header(s, pkt, htype) \
    SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->set_handshake_header((s), (pkt), (htype))
# define ssl_close_construct_packet(s, pkt, htype) \
    SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->close_construct_packet((s), (pkt), (htype))
# define ssl_do_write(s)  SSL_CONNECTION_GET_SSL(s)->method->ssl3_enc->do_write(s)

/* Values for enc_flags */

/* Uses signature algorithms extension */
# define SSL_ENC_FLAG_SIGALGS            0x2
/* Uses SHA256 default PRF */
# define SSL_ENC_FLAG_SHA256_PRF         0x4
/* Is DTLS */
# define SSL_ENC_FLAG_DTLS               0x8
/*
 * Allow TLS 1.2 ciphersuites: applies to DTLS 1.2 as well as TLS 1.2: may
 * apply to others in future.
 */
# define SSL_ENC_FLAG_TLS1_2_CIPHERS     0x10

typedef enum downgrade_en {
    DOWNGRADE_NONE,
    DOWNGRADE_TO_1_2,
    DOWNGRADE_TO_1_1
} DOWNGRADE;

/*
 * Dummy status type for the status_type extension. Indicates no status type
 * set
 */
#define TLSEXT_STATUSTYPE_nothing  -1

/* Sigalgs values */
#define TLSEXT_SIGALG_ecdsa_secp256r1_sha256                    0x0403
#define TLSEXT_SIGALG_ecdsa_secp384r1_sha384                    0x0503
#define TLSEXT_SIGALG_ecdsa_secp521r1_sha512                    0x0603
#define TLSEXT_SIGALG_ecdsa_sha224                              0x0303
#define TLSEXT_SIGALG_ecdsa_sha1                                0x0203
#define TLSEXT_SIGALG_rsa_pss_rsae_sha256                       0x0804
#define TLSEXT_SIGALG_rsa_pss_rsae_sha384                       0x0805
#define TLSEXT_SIGALG_rsa_pss_rsae_sha512                       0x0806
#define TLSEXT_SIGALG_rsa_pss_pss_sha256                        0x0809
#define TLSEXT_SIGALG_rsa_pss_pss_sha384                        0x080a
#define TLSEXT_SIGALG_rsa_pss_pss_sha512                        0x080b
#define TLSEXT_SIGALG_rsa_pkcs1_sha256                          0x0401
#define TLSEXT_SIGALG_rsa_pkcs1_sha384                          0x0501
#define TLSEXT_SIGALG_rsa_pkcs1_sha512                          0x0601
#define TLSEXT_SIGALG_rsa_pkcs1_sha224                          0x0301
#define TLSEXT_SIGALG_rsa_pkcs1_sha1                            0x0201
#define TLSEXT_SIGALG_dsa_sha256                                0x0402
#define TLSEXT_SIGALG_dsa_sha384                                0x0502
#define TLSEXT_SIGALG_dsa_sha512                                0x0602
#define TLSEXT_SIGALG_dsa_sha224                                0x0302
#define TLSEXT_SIGALG_dsa_sha1                                  0x0202
#define TLSEXT_SIGALG_gostr34102012_256_intrinsic               0x0840
#define TLSEXT_SIGALG_gostr34102012_512_intrinsic               0x0841
#define TLSEXT_SIGALG_gostr34102012_256_gostr34112012_256       0xeeee
#define TLSEXT_SIGALG_gostr34102012_512_gostr34112012_512       0xefef
#define TLSEXT_SIGALG_gostr34102001_gostr3411                   0xeded

#define TLSEXT_SIGALG_ed25519                                   0x0807
#define TLSEXT_SIGALG_ed448                                     0x0808
#define TLSEXT_SIGALG_ecdsa_brainpoolP256r1_sha256              0x081a
#define TLSEXT_SIGALG_ecdsa_brainpoolP384r1_sha384              0x081b
#define TLSEXT_SIGALG_ecdsa_brainpoolP512r1_sha512              0x081c
#define TLSEXT_SIGALG_mldsa44                                   0x0904
#define TLSEXT_SIGALG_mldsa65                                   0x0905
#define TLSEXT_SIGALG_mldsa87                                   0x0906

/* Sigalgs names */
#define TLSEXT_SIGALG_ecdsa_secp256r1_sha256_name                    "ecdsa_secp256r1_sha256"
#define TLSEXT_SIGALG_ecdsa_secp384r1_sha384_name                    "ecdsa_secp384r1_sha384"
#define TLSEXT_SIGALG_ecdsa_secp521r1_sha512_name                    "ecdsa_secp521r1_sha512"
#define TLSEXT_SIGALG_ecdsa_sha224_name                              "ecdsa_sha224"
#define TLSEXT_SIGALG_ecdsa_sha1_name                                "ecdsa_sha1"
#define TLSEXT_SIGALG_rsa_pss_rsae_sha256_name                       "rsa_pss_rsae_sha256"
#define TLSEXT_SIGALG_rsa_pss_rsae_sha384_name                       "rsa_pss_rsae_sha384"
#define TLSEXT_SIGALG_rsa_pss_rsae_sha512_name                       "rsa_pss_rsae_sha512"
#define TLSEXT_SIGALG_rsa_pss_pss_sha256_name                        "rsa_pss_pss_sha256"
#define TLSEXT_SIGALG_rsa_pss_pss_sha384_name                        "rsa_pss_pss_sha384"
#define TLSEXT_SIGALG_rsa_pss_pss_sha512_name                        "rsa_pss_pss_sha512"
#define TLSEXT_SIGALG_rsa_pkcs1_sha256_name                          "rsa_pkcs1_sha256"
#define TLSEXT_SIGALG_rsa_pkcs1_sha384_name                          "rsa_pkcs1_sha384"
#define TLSEXT_SIGALG_rsa_pkcs1_sha512_name                          "rsa_pkcs1_sha512"
#define TLSEXT_SIGALG_rsa_pkcs1_sha224_name                          "rsa_pkcs1_sha224"
#define TLSEXT_SIGALG_rsa_pkcs1_sha1_name                            "rsa_pkcs1_sha1"
#define TLSEXT_SIGALG_dsa_sha256_name                                "dsa_sha256"
#define TLSEXT_SIGALG_dsa_sha384_name                                "dsa_sha384"
#define TLSEXT_SIGALG_dsa_sha512_name                                "dsa_sha512"
#define TLSEXT_SIGALG_dsa_sha224_name                                "dsa_sha224"
#define TLSEXT_SIGALG_dsa_sha1_name                                  "dsa_sha1"
#define TLSEXT_SIGALG_gostr34102012_256_intrinsic_name               "gostr34102012_256"
#define TLSEXT_SIGALG_gostr34102012_512_intrinsic_name               "gostr34102012_512"
#define TLSEXT_SIGALG_gostr34102012_256_intrinsic_alias              "gost2012_256"
#define TLSEXT_SIGALG_gostr34102012_512_intrinsic_alias              "gost2012_512"
#define TLSEXT_SIGALG_gostr34102012_256_gostr34112012_256_name       "gost2012_256"
#define TLSEXT_SIGALG_gostr34102012_512_gostr34112012_512_name       "gost2012_512"
#define TLSEXT_SIGALG_gostr34102001_gostr3411_name                   "gost2001_gost94"

#define TLSEXT_SIGALG_ed25519_name                                   "ed25519"
#define TLSEXT_SIGALG_ed448_name                                     "ed448"
#define TLSEXT_SIGALG_ecdsa_brainpoolP256r1_sha256_name              "ecdsa_brainpoolP256r1tls13_sha256"
#define TLSEXT_SIGALG_ecdsa_brainpoolP384r1_sha384_name              "ecdsa_brainpoolP384r1tls13_sha384"
#define TLSEXT_SIGALG_ecdsa_brainpoolP512r1_sha512_name              "ecdsa_brainpoolP512r1tls13_sha512"
#define TLSEXT_SIGALG_ecdsa_brainpoolP256r1_sha256_alias             "ecdsa_brainpoolP256r1_sha256"
#define TLSEXT_SIGALG_ecdsa_brainpoolP384r1_sha384_alias             "ecdsa_brainpoolP384r1_sha384"
#define TLSEXT_SIGALG_ecdsa_brainpoolP512r1_sha512_alias             "ecdsa_brainpoolP512r1_sha512"
#define TLSEXT_SIGALG_mldsa44_name                                   "mldsa44"
#define TLSEXT_SIGALG_mldsa65_name                                   "mldsa65"
#define TLSEXT_SIGALG_mldsa87_name                                   "mldsa87"

/* Known PSK key exchange modes */
#define TLSEXT_KEX_MODE_KE                                      0x00
#define TLSEXT_KEX_MODE_KE_DHE                                  0x01

/*
 * Internal representations of key exchange modes
 */
#define TLSEXT_KEX_MODE_FLAG_NONE                               0
#define TLSEXT_KEX_MODE_FLAG_KE                                 1
#define TLSEXT_KEX_MODE_FLAG_KE_DHE                             2

#define SSL_USE_PSS(s) (s->s3.tmp.peer_sigalg != NULL && \
                        s->s3.tmp.peer_sigalg->sig == EVP_PKEY_RSA_PSS)

/* TLSv1.3 downgrade protection sentinel values */
extern const unsigned char tls11downgrade[8];
extern const unsigned char tls12downgrade[8];

extern const SSL3_ENC_METHOD ssl3_undef_enc_method;

__owur const SSL_METHOD *sslv3_method(void);
__owur const SSL_METHOD *sslv3_server_method(void);
__owur const SSL_METHOD *sslv3_client_method(void);
__owur const SSL_METHOD *tlsv1_method(void);
__owur const SSL_METHOD *tlsv1_server_method(void);
__owur const SSL_METHOD *tlsv1_client_method(void);
__owur const SSL_METHOD *tlsv1_1_method(void);
__owur const SSL_METHOD *tlsv1_1_server_method(void);
__owur const SSL_METHOD *tlsv1_1_client_method(void);
__owur const SSL_METHOD *tlsv1_2_method(void);
__owur const SSL_METHOD *tlsv1_2_server_method(void);
__owur const SSL_METHOD *tlsv1_2_client_method(void);
__owur const SSL_METHOD *tlsv1_3_method(void);
__owur const SSL_METHOD *tlsv1_3_server_method(void);
__owur const SSL_METHOD *tlsv1_3_client_method(void);
__owur const SSL_METHOD *dtlsv1_method(void);
__owur const SSL_METHOD *dtlsv1_server_method(void);
__owur const SSL_METHOD *dtlsv1_client_method(void);
__owur const SSL_METHOD *dtls_bad_ver_client_method(void);
__owur const SSL_METHOD *dtlsv1_2_method(void);
__owur const SSL_METHOD *dtlsv1_2_server_method(void);
__owur const SSL_METHOD *dtlsv1_2_client_method(void);

extern const SSL3_ENC_METHOD TLSv1_enc_data;
extern const SSL3_ENC_METHOD TLSv1_1_enc_data;
extern const SSL3_ENC_METHOD TLSv1_2_enc_data;
extern const SSL3_ENC_METHOD TLSv1_3_enc_data;
extern const SSL3_ENC_METHOD SSLv3_enc_data;
extern const SSL3_ENC_METHOD DTLSv1_enc_data;
extern const SSL3_ENC_METHOD DTLSv1_2_enc_data;

/*
 * Flags for SSL methods
 */
# define SSL_METHOD_NO_FIPS      (1U<<0)
# define SSL_METHOD_NO_SUITEB    (1U<<1)

# define IMPLEMENT_tls_meth_func(version, flags, mask, func_name, s_accept, \
                                 s_connect, enc_data) \
const SSL_METHOD *func_name(void)  \
        { \
        static const SSL_METHOD func_name##_data= { \
                version, \
                flags, \
                mask, \
                ossl_ssl_connection_new, \
                ossl_ssl_connection_free, \
                ossl_ssl_connection_reset, \
                tls1_new, \
                tls1_clear, \
                tls1_free, \
                s_accept, \
                s_connect, \
                ssl3_read, \
                ssl3_peek, \
                ssl3_write, \
                ssl3_shutdown, \
                ssl3_renegotiate, \
                ssl3_renegotiate_check, \
                ssl3_read_bytes, \
                ssl3_write_bytes, \
                ssl3_dispatch_alert, \
                ssl3_ctrl, \
                ssl3_ctx_ctrl, \
                ssl3_get_cipher_by_char, \
                ssl3_put_cipher_by_char, \
                ssl3_pending, \
                ssl3_num_ciphers, \
                ssl3_get_cipher, \
                tls1_default_timeout, \
                &enc_data, \
                ssl_undefined_void_function, \
                ssl3_callback_ctrl, \
                ssl3_ctx_callback_ctrl, \
        }; \
        return &func_name##_data; \
        }

# define IMPLEMENT_ssl3_meth_func(func_name, s_accept, s_connect) \
const SSL_METHOD *func_name(void)  \
        { \
        static const SSL_METHOD func_name##_data= { \
                SSL3_VERSION, \
                SSL_METHOD_NO_FIPS | SSL_METHOD_NO_SUITEB, \
                SSL_OP_NO_SSLv3, \
                ossl_ssl_connection_new, \
                ossl_ssl_connection_free, \
                ossl_ssl_connection_reset, \
                ssl3_new, \
                ssl3_clear, \
                ssl3_free, \
                s_accept, \
                s_connect, \
                ssl3_read, \
                ssl3_peek, \
                ssl3_write, \
                ssl3_shutdown, \
                ssl3_renegotiate, \
                ssl3_renegotiate_check, \
                ssl3_read_bytes, \
                ssl3_write_bytes, \
                ssl3_dispatch_alert, \
                ssl3_ctrl, \
                ssl3_ctx_ctrl, \
                ssl3_get_cipher_by_char, \
                ssl3_put_cipher_by_char, \
                ssl3_pending, \
                ssl3_num_ciphers, \
                ssl3_get_cipher, \
                ssl3_default_timeout, \
                &SSLv3_enc_data, \
                ssl_undefined_void_function, \
                ssl3_callback_ctrl, \
                ssl3_ctx_callback_ctrl, \
        }; \
        return &func_name##_data; \
        }

# define IMPLEMENT_dtls1_meth_func(version, flags, mask, func_name, s_accept, \
                                        s_connect, enc_data) \
const SSL_METHOD *func_name(void)  \
        { \
        static const SSL_METHOD func_name##_data= { \
                version, \
                flags, \
                mask, \
                ossl_ssl_connection_new, \
                ossl_ssl_connection_free, \
                ossl_ssl_connection_reset, \
                dtls1_new, \
                dtls1_clear, \
                dtls1_free, \
                s_accept, \
                s_connect, \
                ssl3_read, \
                ssl3_peek, \
                ssl3_write, \
                dtls1_shutdown, \
                ssl3_renegotiate, \
                ssl3_renegotiate_check, \
                dtls1_read_bytes, \
                dtls1_write_app_data_bytes, \
                dtls1_dispatch_alert, \
                dtls1_ctrl, \
                ssl3_ctx_ctrl, \
                ssl3_get_cipher_by_char, \
                ssl3_put_cipher_by_char, \
                ssl3_pending, \
                ssl3_num_ciphers, \
                ssl3_get_cipher, \
                dtls1_default_timeout, \
                &enc_data, \
                ssl_undefined_void_function, \
                ssl3_callback_ctrl, \
                ssl3_ctx_callback_ctrl, \
        }; \
        return &func_name##_data; \
        }

struct openssl_ssl_test_functions {
    int (*p_ssl_init_wbio_buffer) (SSL_CONNECTION *s);
};

const char *ssl_protocol_to_string(int version);

static ossl_inline int tls12_rpk_and_privkey(const SSL_CONNECTION *sc, int idx)
{
    /*
     * This is to check for special cases when using RPK with just
     * a private key, and NO CERTIFICATE
     */
    return ((sc->server && sc->ext.server_cert_type == TLSEXT_cert_type_rpk)
            || (!sc->server && sc->ext.client_cert_type == TLSEXT_cert_type_rpk))
        && sc->cert->pkeys[idx].privatekey != NULL
        && sc->cert->pkeys[idx].x509 == NULL;
}

static ossl_inline int ssl_has_cert_type(const SSL_CONNECTION *sc, unsigned char ct)
{
    unsigned char *ptr;
    size_t len;

    if (sc->server) {
        ptr = sc->server_cert_type;
        len = sc->server_cert_type_len;
    } else {
        ptr = sc->client_cert_type;
        len = sc->client_cert_type_len;
    }

    if (ptr == NULL)
        return 0;

    return memchr(ptr, ct, len) != NULL;
}

/* Returns true if certificate and private key for 'idx' are present */
static ossl_inline int ssl_has_cert(const SSL_CONNECTION *s, int idx)
{
    if (idx < 0 || idx >= (int)s->ssl_pkey_num)
        return 0;

    /* If RPK is enabled for this SSL... only require private key */
    if (ssl_has_cert_type(s, TLSEXT_cert_type_rpk))
        return s->cert->pkeys[idx].privatekey != NULL;

    return s->cert->pkeys[idx].x509 != NULL
        && s->cert->pkeys[idx].privatekey != NULL;
}

static ossl_inline void tls1_get_peer_groups(SSL_CONNECTION *s,
                                             const uint16_t **pgroups,
                                             size_t *pgroupslen)
{
    *pgroups = s->ext.peer_supportedgroups;
    *pgroupslen = s->ext.peer_supportedgroups_len;
}

# ifndef OPENSSL_UNIT_TEST

__owur int ossl_ssl_init(SSL *ssl, SSL_CTX *ctx, const SSL_METHOD *method,
                         int type);
__owur SSL *ossl_ssl_connection_new_int(SSL_CTX *ctx, SSL *user_ssl,
                                        const SSL_METHOD *method);
__owur SSL *ossl_ssl_connection_new(SSL_CTX *ctx);
void ossl_ssl_connection_free(SSL *ssl);
__owur int ossl_ssl_connection_reset(SSL *ssl);

__owur int ssl_read_internal(SSL *s, void *buf, size_t num, size_t *readbytes);
__owur int ssl_write_internal(SSL *s, const void *buf, size_t num,
                              uint64_t flags, size_t *written);
int ssl_clear_bad_session(SSL_CONNECTION *s);
__owur CERT *ssl_cert_new(size_t ssl_pkey_num);
__owur CERT *ssl_cert_dup(CERT *cert);
void ssl_cert_clear_certs(CERT *c);
void ssl_cert_free(CERT *c);
__owur int ssl_generate_session_id(SSL_CONNECTION *s, SSL_SESSION *ss);
__owur int ssl_get_new_session(SSL_CONNECTION *s, int session);
__owur SSL_SESSION *lookup_sess_in_cache(SSL_CONNECTION *s,
                                         const unsigned char *sess_id,
                                         size_t sess_id_len);
__owur int ssl_get_prev_session(SSL_CONNECTION *s, CLIENTHELLO_MSG *hello);
__owur SSL_SESSION *ssl_session_dup(const SSL_SESSION *src, int ticket);
__owur int ssl_cipher_id_cmp(const SSL_CIPHER *a, const SSL_CIPHER *b);
DECLARE_OBJ_BSEARCH_GLOBAL_CMP_FN(SSL_CIPHER, SSL_CIPHER, ssl_cipher_id);
__owur int ssl_cipher_ptr_id_cmp(const SSL_CIPHER *const *ap,
                                 const SSL_CIPHER *const *bp);
__owur STACK_OF(SSL_CIPHER) *ssl_create_cipher_list(SSL_CTX *ctx,
                                                    STACK_OF(SSL_CIPHER) *tls13_ciphersuites,
                                                    STACK_OF(SSL_CIPHER) **cipher_list,
                                                    STACK_OF(SSL_CIPHER) **cipher_list_by_id,
                                                    const char *rule_str,
                                                    CERT *c);
__owur int ssl_cache_cipherlist(SSL_CONNECTION *s, PACKET *cipher_suites,
                                int sslv2format);
__owur int ossl_bytes_to_cipher_list(SSL_CONNECTION *s, PACKET *cipher_suites,
                                     STACK_OF(SSL_CIPHER) **skp,
                                     STACK_OF(SSL_CIPHER) **scsvs, int sslv2format,
                                     int fatal);
void ssl_update_cache(SSL_CONNECTION *s, int mode);
__owur int ssl_cipher_get_evp_cipher(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                                     const EVP_CIPHER **enc);
__owur int ssl_cipher_get_evp_md_mac(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                                     const EVP_MD **md,
                                     int *mac_pkey_type, size_t *mac_secret_size);
__owur int ssl_cipher_get_evp(SSL_CTX *ctxc, const SSL_SESSION *s,
                              const EVP_CIPHER **enc, const EVP_MD **md,
                              int *mac_pkey_type, size_t *mac_secret_size,
                              SSL_COMP **comp, int use_etm);
__owur int ssl_cipher_get_overhead(const SSL_CIPHER *c, size_t *mac_overhead,
                                   size_t *int_overhead, size_t *blocksize,
                                   size_t *ext_overhead);
__owur int ssl_cert_is_disabled(SSL_CTX *ctx, size_t idx);
__owur const SSL_CIPHER *ssl_get_cipher_by_char(SSL_CONNECTION *ssl,
                                                const unsigned char *ptr,
                                                int all);
__owur int ssl_cert_set0_chain(SSL_CONNECTION *s, SSL_CTX *ctx,
                               STACK_OF(X509) *chain);
__owur int ssl_cert_set1_chain(SSL_CONNECTION *s, SSL_CTX *ctx,
                               STACK_OF(X509) *chain);
__owur int ssl_cert_add0_chain_cert(SSL_CONNECTION *s, SSL_CTX *ctx, X509 *x);
__owur int ssl_cert_add1_chain_cert(SSL_CONNECTION *s, SSL_CTX *ctx, X509 *x);
__owur int ssl_cert_select_current(CERT *c, X509 *x);
__owur int ssl_cert_set_current(CERT *c, long arg);
void ssl_cert_set_cert_cb(CERT *c, int (*cb) (SSL *ssl, void *arg), void *arg);

__owur int ssl_verify_cert_chain(SSL_CONNECTION *s, STACK_OF(X509) *sk);
__owur int ssl_verify_rpk(SSL_CONNECTION *s, EVP_PKEY *rpk);
__owur int ssl_build_cert_chain(SSL_CONNECTION *s, SSL_CTX *ctx, int flags);
__owur int ssl_cert_set_cert_store(CERT *c, X509_STORE *store, int chain,
                                   int ref);
__owur int ssl_cert_get_cert_store(CERT *c, X509_STORE **pstore, int chain);

__owur int ssl_security(const SSL_CONNECTION *s, int op, int bits, int nid,
                        void *other);
__owur int ssl_ctx_security(const SSL_CTX *ctx, int op, int bits, int nid,
                            void *other);
int ssl_get_security_level_bits(const SSL *s, const SSL_CTX *ctx, int *levelp);

__owur int ssl_cert_lookup_by_nid(int nid, size_t *pidx, SSL_CTX *ctx);
__owur const SSL_CERT_LOOKUP *ssl_cert_lookup_by_pkey(const EVP_PKEY *pk,
                                                      size_t *pidx,
                                                      SSL_CTX *ctx);
__owur const SSL_CERT_LOOKUP *ssl_cert_lookup_by_idx(size_t idx, SSL_CTX *ctx);

int ssl_undefined_function(SSL *s);
__owur int ssl_undefined_void_function(void);
__owur int ssl_get_server_cert_serverinfo(SSL_CONNECTION *s,
                                          const unsigned char **serverinfo,
                                          size_t *serverinfo_length);
void ssl_set_masks(SSL_CONNECTION *s);
__owur STACK_OF(SSL_CIPHER) *ssl_get_ciphers_by_id(SSL_CONNECTION *sc);
__owur int ssl_x509err2alert(int type);
void ssl_sort_cipher_list(void);
int ssl_load_ciphers(SSL_CTX *ctx);
__owur int ssl_setup_sigalgs(SSL_CTX *ctx);
int ssl_load_groups(SSL_CTX *ctx);
int ssl_load_sigalgs(SSL_CTX *ctx);
__owur int ssl_fill_hello_random(SSL_CONNECTION *s, int server,
                                 unsigned char *field, size_t len,
                                 DOWNGRADE dgrd);
__owur int ssl_generate_master_secret(SSL_CONNECTION *s, unsigned char *pms,
                                      size_t pmslen, int free_pms);
__owur EVP_PKEY *ssl_generate_pkey(SSL_CONNECTION *s, EVP_PKEY *pm);
__owur int ssl_gensecret(SSL_CONNECTION *s, unsigned char *pms, size_t pmslen);
__owur int ssl_derive(SSL_CONNECTION *s, EVP_PKEY *privkey, EVP_PKEY *pubkey,
                      int genmaster);
__owur int ssl_decapsulate(SSL_CONNECTION *s, EVP_PKEY *privkey,
                           const unsigned char *ct, size_t ctlen,
                           int gensecret);
__owur int ssl_encapsulate(SSL_CONNECTION *s, EVP_PKEY *pubkey,
                           unsigned char **ctp, size_t *ctlenp,
                           int gensecret);
__owur EVP_PKEY *ssl_dh_to_pkey(DH *dh);
__owur int ssl_set_tmp_ecdh_groups(uint16_t **pext, size_t *pextlen,
                                   uint16_t **ksext, size_t *ksextlen,
                                   size_t **tplext, size_t *tplextlen,
                                   void *key);
__owur unsigned int ssl_get_max_send_fragment(const SSL_CONNECTION *sc);
__owur unsigned int ssl_get_split_send_fragment(const SSL_CONNECTION *sc);

__owur const SSL_CIPHER *ssl3_get_cipher_by_id(uint32_t id);
__owur const SSL_CIPHER *ssl3_get_cipher_by_std_name(const char *stdname);
__owur const SSL_CIPHER *ssl3_get_cipher_by_char(const unsigned char *p);
__owur int ssl3_put_cipher_by_char(const SSL_CIPHER *c, WPACKET *pkt,
                                   size_t *len);
int ssl3_init_finished_mac(SSL_CONNECTION *s);
__owur int ssl3_setup_key_block(SSL_CONNECTION *s);
__owur int ssl3_change_cipher_state(SSL_CONNECTION *s, int which);
void ssl3_cleanup_key_block(SSL_CONNECTION *s);
__owur int ssl3_do_write(SSL_CONNECTION *s, uint8_t type);
int ssl3_send_alert(SSL_CONNECTION *s, int level, int desc);
__owur int ssl3_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                       unsigned char *p, size_t len,
                                       size_t *secret_size);
__owur int ssl3_get_req_cert_type(SSL_CONNECTION *s, WPACKET *pkt);
__owur int ssl3_num_ciphers(void);
__owur const SSL_CIPHER *ssl3_get_cipher(unsigned int u);
int ssl3_renegotiate(SSL *ssl);
int ssl3_renegotiate_check(SSL *ssl, int initok);
void ssl3_digest_master_key_set_params(const SSL_SESSION *session,
                                       OSSL_PARAM params[]);
__owur int ssl3_dispatch_alert(SSL *s);
__owur size_t ssl3_final_finish_mac(SSL_CONNECTION *s, const char *sender,
                                    size_t slen, unsigned char *p);
__owur int ssl3_finish_mac(SSL_CONNECTION *s, const unsigned char *buf,
                           size_t len);
void ssl3_free_digest_list(SSL_CONNECTION *s);
__owur unsigned long ssl3_output_cert_chain(SSL_CONNECTION *s, WPACKET *pkt,
                                            CERT_PKEY *cpk, int for_comp);
__owur const SSL_CIPHER *ssl3_choose_cipher(SSL_CONNECTION *s,
                                            STACK_OF(SSL_CIPHER) *clnt,
                                            STACK_OF(SSL_CIPHER) *srvr);
__owur int ssl3_digest_cached_records(SSL_CONNECTION *s, int keep);
__owur int ssl3_new(SSL *s);
void ssl3_free(SSL *s);
__owur int ssl3_read(SSL *s, void *buf, size_t len, size_t *readbytes);
__owur int ssl3_peek(SSL *s, void *buf, size_t len, size_t *readbytes);
__owur int ssl3_write(SSL *s, const void *buf, size_t len, size_t *written);
__owur int ssl3_shutdown(SSL *s);
int ssl3_clear(SSL *s);
__owur long ssl3_ctrl(SSL *s, int cmd, long larg, void *parg);
__owur long ssl3_ctx_ctrl(SSL_CTX *s, int cmd, long larg, void *parg);
__owur long ssl3_callback_ctrl(SSL *s, int cmd, void (*fp) (void));
__owur long ssl3_ctx_callback_ctrl(SSL_CTX *s, int cmd, void (*fp) (void));

__owur int ssl3_do_change_cipher_spec(SSL_CONNECTION *s);
__owur OSSL_TIME ssl3_default_timeout(void);

__owur int ssl3_set_handshake_header(SSL_CONNECTION *s, WPACKET *pkt,
                                     int htype);
__owur int tls_close_construct_packet(SSL_CONNECTION *s, WPACKET *pkt, int htype);
__owur int tls_setup_handshake(SSL_CONNECTION *s);
__owur int dtls1_set_handshake_header(SSL_CONNECTION *s, WPACKET *pkt, int htype);
__owur int dtls1_close_construct_packet(SSL_CONNECTION *s, WPACKET *pkt, int htype);
__owur int ssl3_handshake_write(SSL_CONNECTION *s);

__owur int ssl_allow_compression(SSL_CONNECTION *s);

__owur int ssl_version_cmp(const SSL_CONNECTION *s, int versiona, int versionb);
__owur int ssl_version_supported(const SSL_CONNECTION *s, int version,
                                 const SSL_METHOD **meth);

__owur int ssl_set_client_hello_version(SSL_CONNECTION *s);
__owur int ssl_check_version_downgrade(SSL_CONNECTION *s);
__owur int ssl_set_version_bound(int method_version, int version, int *bound);
__owur int ssl_choose_server_version(SSL_CONNECTION *s, CLIENTHELLO_MSG *hello,
                                     DOWNGRADE *dgrd);
__owur int ssl_choose_client_version(SSL_CONNECTION *s, int version,
                                     RAW_EXTENSION *extensions);
__owur int ssl_get_min_max_version(const SSL_CONNECTION *s, int *min_version,
                                   int *max_version, int *real_max);

__owur OSSL_TIME tls1_default_timeout(void);
__owur int dtls1_do_write(SSL_CONNECTION *s, uint8_t type);
void dtls1_set_message_header(SSL_CONNECTION *s,
                              unsigned char mt,
                              size_t len,
                              size_t frag_off, size_t frag_len);

int dtls1_write_app_data_bytes(SSL *s, uint8_t type, const void *buf_,
                               size_t len, size_t *written);

__owur int dtls1_read_failed(SSL_CONNECTION *s, int code);
__owur int dtls1_buffer_message(SSL_CONNECTION *s, int ccs);
__owur int dtls1_retransmit_message(SSL_CONNECTION *s, unsigned short seq,
                                    int *found);
__owur int dtls1_get_queue_priority(unsigned short seq, int is_ccs);
int dtls1_retransmit_buffered_messages(SSL_CONNECTION *s);
void dtls1_clear_received_buffer(SSL_CONNECTION *s);
void dtls1_clear_sent_buffer(SSL_CONNECTION *s);
void dtls1_get_message_header(const unsigned char *data,
                              struct hm_header_st *msg_hdr);
__owur OSSL_TIME dtls1_default_timeout(void);
__owur int dtls1_get_timeout(const SSL_CONNECTION *s, OSSL_TIME *timeleft);
__owur int dtls1_check_timeout_num(SSL_CONNECTION *s);
__owur int dtls1_handle_timeout(SSL_CONNECTION *s);
void dtls1_start_timer(SSL_CONNECTION *s);
void dtls1_stop_timer(SSL_CONNECTION *s);
__owur int dtls1_is_timer_expired(SSL_CONNECTION *s);
__owur int dtls_raw_hello_verify_request(WPACKET *pkt, unsigned char *cookie,
                                         size_t cookie_len);
__owur size_t dtls1_min_mtu(SSL_CONNECTION *s);
void dtls1_hm_fragment_free(hm_fragment *frag);
__owur int dtls1_query_mtu(SSL_CONNECTION *s);

__owur int tls1_new(SSL *s);
void tls1_free(SSL *s);
int tls1_clear(SSL *s);

__owur int dtls1_new(SSL *s);
void dtls1_free(SSL *s);
int dtls1_clear(SSL *s);
long dtls1_ctrl(SSL *s, int cmd, long larg, void *parg);
__owur int dtls1_shutdown(SSL *s);

__owur int dtls1_dispatch_alert(SSL *s);

__owur int ssl_init_wbio_buffer(SSL_CONNECTION *s);
int ssl_free_wbio_buffer(SSL_CONNECTION *s);

__owur int tls1_change_cipher_state(SSL_CONNECTION *s, int which);
__owur int tls1_setup_key_block(SSL_CONNECTION *s);
__owur size_t tls1_final_finish_mac(SSL_CONNECTION *s, const char *str,
                                    size_t slen, unsigned char *p);
__owur int tls1_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                       unsigned char *p, size_t len,
                                       size_t *secret_size);
__owur int tls13_setup_key_block(SSL_CONNECTION *s);
__owur size_t tls13_final_finish_mac(SSL_CONNECTION *s, const char *str, size_t slen,
                                     unsigned char *p);
__owur int tls13_store_handshake_traffic_hash(SSL_CONNECTION *s);
__owur int tls13_store_server_finished_hash(SSL_CONNECTION *s);
__owur int tls13_change_cipher_state(SSL_CONNECTION *s, int which);
__owur int tls13_update_key(SSL_CONNECTION *s, int send);
__owur int tls13_hkdf_expand(SSL_CONNECTION *s,
                             const EVP_MD *md,
                             const unsigned char *secret,
                             const unsigned char *label, size_t labellen,
                             const unsigned char *data, size_t datalen,
                             unsigned char *out, size_t outlen, int fatal);
__owur int tls13_hkdf_expand_ex(OSSL_LIB_CTX *libctx, const char *propq,
                                const EVP_MD *md,
                                const unsigned char *secret,
                                const unsigned char *label, size_t labellen,
                                const unsigned char *data, size_t datalen,
                                unsigned char *out, size_t outlen,
                                int raise_error);
__owur int tls13_derive_key(SSL_CONNECTION *s, const EVP_MD *md,
                            const unsigned char *secret, unsigned char *key,
                            size_t keylen);
__owur int tls13_derive_iv(SSL_CONNECTION *s, const EVP_MD *md,
                           const unsigned char *secret, unsigned char *iv,
                           size_t ivlen);
__owur int tls13_derive_finishedkey(SSL_CONNECTION *s, const EVP_MD *md,
                                    const unsigned char *secret,
                                    unsigned char *fin, size_t finlen);
int tls13_generate_secret(SSL_CONNECTION *s, const EVP_MD *md,
                          const unsigned char *prevsecret,
                          const unsigned char *insecret,
                          size_t insecretlen,
                          unsigned char *outsecret);
__owur int tls13_generate_handshake_secret(SSL_CONNECTION *s,
                                           const unsigned char *insecret,
                                           size_t insecretlen);
__owur int tls13_generate_master_secret(SSL_CONNECTION *s, unsigned char *out,
                                        unsigned char *prev, size_t prevlen,
                                        size_t *secret_size);
__owur int tls1_export_keying_material(SSL_CONNECTION *s,
                                       unsigned char *out, size_t olen,
                                       const char *label, size_t llen,
                                       const unsigned char *p, size_t plen,
                                       int use_context);
__owur int tls13_export_keying_material(SSL_CONNECTION *s,
                                        unsigned char *out, size_t olen,
                                        const char *label, size_t llen,
                                        const unsigned char *context,
                                        size_t contextlen, int use_context);
__owur int tls13_export_keying_material_early(SSL_CONNECTION *s,
                                              unsigned char *out, size_t olen,
                                              const char *label, size_t llen,
                                              const unsigned char *context,
                                              size_t contextlen);
__owur int tls1_alert_code(int code);
__owur int tls13_alert_code(int code);
__owur int ssl3_alert_code(int code);

__owur int ssl_check_srvr_ecc_cert_and_alg(X509 *x, SSL_CONNECTION *s);

SSL_COMP *ssl3_comp_find(STACK_OF(SSL_COMP) *sk, int n);

__owur const TLS_GROUP_INFO *tls1_group_id_lookup(SSL_CTX *ctx, uint16_t curve_id);
__owur const char *tls1_group_id2name(SSL_CTX *ctx, uint16_t group_id);
__owur int tls1_group_id2nid(uint16_t group_id, int include_unknown);
__owur uint16_t tls1_nid2group_id(int nid);
__owur int tls1_check_group_id(SSL_CONNECTION *s, uint16_t group_id,
                               int check_own_curves);
__owur int tls1_get0_implemented_groups(int min_proto_version,
                                        int max_proto_version,
                                        TLS_GROUP_INFO *grps,
                                        size_t num, long all,
                                        STACK_OF(OPENSSL_CSTRING) *out);
__owur uint16_t tls1_shared_group(SSL_CONNECTION *s, int nmatch);
__owur int tls1_set_groups(uint16_t **grpext, size_t *grpextlen,
                           uint16_t **ksext, size_t *ksextlen,
                           size_t **tplext, size_t *tplextlen,
                           int *curves, size_t ncurves);
__owur int tls1_set_groups_list(SSL_CTX *ctx,
                                uint16_t **grpext, size_t *grpextlen,
                                uint16_t **ksext, size_t *ksextlen,
                                size_t **tplext, size_t *tplextlen,
                                const char *str);
__owur EVP_PKEY *ssl_generate_pkey_group(SSL_CONNECTION *s, uint16_t id);
__owur int tls_valid_group(SSL_CONNECTION *s, uint16_t group_id, int minversion,
                           int maxversion, int isec, int *okfortls13);
__owur EVP_PKEY *ssl_generate_param_group(SSL_CONNECTION *s, uint16_t id);
void tls1_get_formatlist(SSL_CONNECTION *s, const unsigned char **pformats,
                         size_t *num_formats);
__owur int tls1_check_ec_tmp_key(SSL_CONNECTION *s, unsigned long id);

__owur int tls_group_allowed(SSL_CONNECTION *s, uint16_t curve, int op);
void tls1_get_supported_groups(SSL_CONNECTION *s, const uint16_t **pgroups,
                               size_t *pgroupslen);
void tls1_get_requested_keyshare_groups(SSL_CONNECTION *s, const uint16_t **pgroups,
                                        size_t *pgroupslen);
void tls1_get_group_tuples(SSL_CONNECTION *s, const size_t **ptuples,
                           size_t *ptupleslen);

__owur int tls1_set_server_sigalgs(SSL_CONNECTION *s);

__owur SSL_TICKET_STATUS tls_get_ticket_from_client(SSL_CONNECTION *s,
                                                    CLIENTHELLO_MSG *hello,
                                                    SSL_SESSION **ret);
__owur SSL_TICKET_STATUS tls_decrypt_ticket(SSL_CONNECTION *s,
                                            const unsigned char *etick,
                                            size_t eticklen,
                                            const unsigned char *sess_id,
                                            size_t sesslen, SSL_SESSION **psess);

__owur int tls_use_ticket(SSL_CONNECTION *s);

void ssl_set_sig_mask(uint32_t *pmask_a, SSL_CONNECTION *s, int op);

__owur int tls1_set_sigalgs_list(SSL_CTX *ctx, CERT *c, const char *str, int client);
__owur int tls1_set_raw_sigalgs(CERT *c, const uint16_t *psigs, size_t salglen,
                                int client);
__owur int tls1_set_sigalgs(CERT *c, const int *salg, size_t salglen,
                            int client);
int tls1_check_chain(SSL_CONNECTION *s, X509 *x, EVP_PKEY *pk,
                     STACK_OF(X509) *chain, int idx);
void tls1_set_cert_validity(SSL_CONNECTION *s);

#  ifndef OPENSSL_NO_CT
__owur int ssl_validate_ct(SSL_CONNECTION *s);
#  endif

__owur EVP_PKEY *ssl_get_auto_dh(SSL_CONNECTION *s);

__owur int ssl_security_cert(SSL_CONNECTION *s, SSL_CTX *ctx, X509 *x, int vfy,
                             int is_ee);
__owur int ssl_security_cert_chain(SSL_CONNECTION *s, STACK_OF(X509) *sk,
                                   X509 *ex, int vfy);

int tls_choose_sigalg(SSL_CONNECTION *s, int fatalerrs);

__owur long ssl_get_algorithm2(SSL_CONNECTION *s);
__owur int tls12_copy_sigalgs(SSL_CONNECTION *s, WPACKET *pkt,
                              const uint16_t *psig, size_t psiglen);
__owur int tls1_save_u16(PACKET *pkt, uint16_t **pdest, size_t *pdestlen);
__owur int tls1_save_sigalgs(SSL_CONNECTION *s, PACKET *pkt, int cert);
__owur int tls1_process_sigalgs(SSL_CONNECTION *s);
__owur int tls1_set_peer_legacy_sigalg(SSL_CONNECTION *s, const EVP_PKEY *pkey);
__owur int tls1_lookup_md(SSL_CTX *ctx, const SIGALG_LOOKUP *lu,
                          const EVP_MD **pmd);
__owur size_t tls12_get_psigalgs(SSL_CONNECTION *s, int sent,
                                 const uint16_t **psigs);
__owur int tls_check_sigalg_curve(const SSL_CONNECTION *s, int curve);
__owur int tls12_check_peer_sigalg(SSL_CONNECTION *s, uint16_t, EVP_PKEY *pkey);
__owur int ssl_set_client_disabled(SSL_CONNECTION *s);
__owur int ssl_cipher_disabled(const SSL_CONNECTION *s, const SSL_CIPHER *c,
                               int op, int echde);

__owur int ssl_handshake_hash(SSL_CONNECTION *s,
                              unsigned char *out, size_t outlen,
                              size_t *hashlen);
__owur const EVP_MD *ssl_md(SSL_CTX *ctx, int idx);
int ssl_get_md_idx(int md_nid);
__owur const EVP_MD *ssl_handshake_md(SSL_CONNECTION *s);
__owur const EVP_MD *ssl_prf_md(SSL_CONNECTION *s);

__owur int ossl_adjust_domain_flags(uint64_t domain_flags,
                                    uint64_t *p_domain_flags);

/*
 * ssl_log_rsa_client_key_exchange logs |premaster| to the SSL_CTX associated
 * with |ssl|, if logging is enabled. It returns one on success and zero on
 * failure. The entry is identified by the first 8 bytes of
 * |encrypted_premaster|.
 */
__owur int ssl_log_rsa_client_key_exchange(SSL_CONNECTION *s,
                                           const uint8_t *encrypted_premaster,
                                           size_t encrypted_premaster_len,
                                           const uint8_t *premaster,
                                           size_t premaster_len);

/*
 * ssl_log_secret logs |secret| to the SSL_CTX associated with |ssl|, if
 * logging is available. It returns one on success and zero on failure. It tags
 * the entry with |label|.
 */
__owur int ssl_log_secret(SSL_CONNECTION *s, const char *label,
                          const uint8_t *secret, size_t secret_len);

#define MASTER_SECRET_LABEL "CLIENT_RANDOM"
#define CLIENT_EARLY_LABEL "CLIENT_EARLY_TRAFFIC_SECRET"
#define CLIENT_HANDSHAKE_LABEL "CLIENT_HANDSHAKE_TRAFFIC_SECRET"
#define SERVER_HANDSHAKE_LABEL "SERVER_HANDSHAKE_TRAFFIC_SECRET"
#define CLIENT_APPLICATION_LABEL "CLIENT_TRAFFIC_SECRET_0"
#define CLIENT_APPLICATION_N_LABEL "CLIENT_TRAFFIC_SECRET_N"
#define SERVER_APPLICATION_LABEL "SERVER_TRAFFIC_SECRET_0"
#define SERVER_APPLICATION_N_LABEL "SERVER_TRAFFIC_SECRET_N"
#define EARLY_EXPORTER_SECRET_LABEL "EARLY_EXPORTER_SECRET"
#define EXPORTER_SECRET_LABEL "EXPORTER_SECRET"

__owur int srp_generate_server_master_secret(SSL_CONNECTION *s);
__owur int srp_generate_client_master_secret(SSL_CONNECTION *s);
__owur int srp_verify_server_param(SSL_CONNECTION *s);

/* statem/statem_srvr.c */

__owur int send_certificate_request(SSL_CONNECTION *s);

/* statem/extensions_cust.c */

custom_ext_method *custom_ext_find(const custom_ext_methods *exts,
                                   ENDPOINT role, unsigned int ext_type,
                                   size_t *idx);

void custom_ext_init(custom_ext_methods *meths);

int ossl_tls_add_custom_ext_intern(SSL_CTX *ctx, custom_ext_methods *exts,
                                   ENDPOINT role, unsigned int ext_type,
                                   unsigned int context,
                                   SSL_custom_ext_add_cb_ex add_cb,
                                   SSL_custom_ext_free_cb_ex free_cb,
                                   void *add_arg,
                                   SSL_custom_ext_parse_cb_ex parse_cb,
                                   void *parse_arg);
__owur int custom_ext_parse(SSL_CONNECTION *s, unsigned int context,
                            unsigned int ext_type,
                            const unsigned char *ext_data, size_t ext_size,
                            X509 *x, size_t chainidx);
__owur int custom_ext_add(SSL_CONNECTION *s, int context, WPACKET *pkt, X509 *x,
                          size_t chainidx, int maxversion);

__owur int custom_exts_copy(custom_ext_methods *dst,
                            const custom_ext_methods *src);
__owur int custom_exts_copy_conn(custom_ext_methods *dst,
                                 const custom_ext_methods *src);
__owur int custom_exts_copy_flags(custom_ext_methods *dst,
                                  const custom_ext_methods *src);
void custom_exts_free(custom_ext_methods *exts);

/* ssl_mcnf.c */
int ssl_ctx_system_config(SSL_CTX *ctx);

const EVP_CIPHER *ssl_evp_cipher_fetch(OSSL_LIB_CTX *libctx,
                                       int nid,
                                       const char *properties);
int ssl_evp_cipher_up_ref(const EVP_CIPHER *cipher);
void ssl_evp_cipher_free(const EVP_CIPHER *cipher);
const EVP_MD *ssl_evp_md_fetch(OSSL_LIB_CTX *libctx,
                               int nid,
                               const char *properties);
int ssl_evp_md_up_ref(const EVP_MD *md);
void ssl_evp_md_free(const EVP_MD *md);

void tls_engine_finish(ENGINE *e);
const EVP_CIPHER *tls_get_cipher_from_engine(int nid);
const EVP_MD *tls_get_digest_from_engine(int nid);
int tls_engine_load_ssl_client_cert(SSL_CONNECTION *s, X509 **px509,
                                    EVP_PKEY **ppkey);
int ssl_hmac_old_new(SSL_HMAC *ret);
void ssl_hmac_old_free(SSL_HMAC *ctx);
int ssl_hmac_old_init(SSL_HMAC *ctx, void *key, size_t len, char *md);
int ssl_hmac_old_update(SSL_HMAC *ctx, const unsigned char *data, size_t len);
int ssl_hmac_old_final(SSL_HMAC *ctx, unsigned char *md, size_t *len);
size_t ssl_hmac_old_size(const SSL_HMAC *ctx);

int ssl_ctx_srp_ctx_free_intern(SSL_CTX *ctx);
int ssl_ctx_srp_ctx_init_intern(SSL_CTX *ctx);
int ssl_srp_ctx_free_intern(SSL_CONNECTION *s);
int ssl_srp_ctx_init_intern(SSL_CONNECTION *s);

int ssl_srp_calc_a_param_intern(SSL_CONNECTION *s);
int ssl_srp_server_param_with_username_intern(SSL_CONNECTION *s, int *ad);

void ssl_session_calculate_timeout(SSL_SESSION *ss);

# else /* OPENSSL_UNIT_TEST */

#  define ssl_init_wbio_buffer SSL_test_functions()->p_ssl_init_wbio_buffer

# endif

/* Some helper routines to support TSAN operations safely */
static ossl_unused ossl_inline int ssl_tsan_lock(const SSL_CTX *ctx)
{
#ifdef TSAN_REQUIRES_LOCKING
    if (!CRYPTO_THREAD_write_lock(ctx->tsan_lock))
        return 0;
#endif
    return 1;
}

static ossl_unused ossl_inline void ssl_tsan_unlock(const SSL_CTX *ctx)
{
#ifdef TSAN_REQUIRES_LOCKING
    CRYPTO_THREAD_unlock(ctx->tsan_lock);
#endif
}

static ossl_unused ossl_inline void ssl_tsan_counter(const SSL_CTX *ctx,
                                                     TSAN_QUALIFIER int *stat)
{
    if (ssl_tsan_lock(ctx)) {
        tsan_counter(stat);
        ssl_tsan_unlock(ctx);
    }
}

int ossl_comp_has_alg(int a);
size_t ossl_calculate_comp_expansion(int alg, size_t length);

void ossl_ssl_set_custom_record_layer(SSL_CONNECTION *s,
                                      const OSSL_RECORD_METHOD *meth,
                                      void *rlarg);

long ossl_ctrl_internal(SSL *s, int cmd, long larg, void *parg, int no_quic);

/*
 * Options which no longer have any effect, but which can be implemented
 * as no-ops for QUIC.
 */
#define OSSL_LEGACY_SSL_OPTIONS                 \
    (SSL_OP_NETSCAPE_REUSE_CIPHER_CHANGE_BUG  | \
     SSL_OP_MICROSOFT_BIG_SSLV3_BUFFER        | \
     SSL_OP_SSLEAY_080_CLIENT_DH_BUG          | \
     SSL_OP_TLS_D5_BUG                        | \
     SSL_OP_TLS_BLOCK_PADDING_BUG             | \
     SSL_OP_MSIE_SSLV2_RSA_PADDING            | \
     SSL_OP_SSLREF2_REUSE_CERT_TYPE_BUG       | \
     SSL_OP_MICROSOFT_SESS_ID_BUG             | \
     SSL_OP_NETSCAPE_CHALLENGE_BUG            | \
     SSL_OP_PKCS1_CHECK_1                     | \
     SSL_OP_PKCS1_CHECK_2                     | \
     SSL_OP_SINGLE_DH_USE                     | \
     SSL_OP_SINGLE_ECDH_USE                   | \
     SSL_OP_EPHEMERAL_RSA                     )

/* This option is undefined in public headers with no-dtls1-method. */
#ifndef SSL_OP_CISCO_ANYCONNECT
# define SSL_OP_CISCO_ANYCONNECT 0
#endif
/*
 * Options which are no-ops under QUIC or TLSv1.3 and which are therefore
 * allowed but ignored under QUIC.
 */
#define OSSL_TLS1_2_OPTIONS                     \
    (SSL_OP_CRYPTOPRO_TLSEXT_BUG              | \
     SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS       | \
     SSL_OP_ALLOW_CLIENT_RENEGOTIATION        | \
     SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION | \
     SSL_OP_NO_COMPRESSION                    | \
     SSL_OP_NO_SSLv3                          | \
     SSL_OP_NO_TLSv1                          | \
     SSL_OP_NO_TLSv1_1                        | \
     SSL_OP_NO_TLSv1_2                        | \
     SSL_OP_NO_DTLSv1                         | \
     SSL_OP_NO_DTLSv1_2                       | \
     SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION | \
     SSL_OP_CISCO_ANYCONNECT                  | \
     SSL_OP_NO_RENEGOTIATION                  | \
     SSL_OP_NO_EXTENDED_MASTER_SECRET         | \
     SSL_OP_NO_ENCRYPT_THEN_MAC               | \
     SSL_OP_COOKIE_EXCHANGE                   | \
     SSL_OP_LEGACY_SERVER_CONNECT             | \
     SSL_OP_IGNORE_UNEXPECTED_EOF             )

/* Total mask of connection-level options permitted or ignored under QUIC. */
#define OSSL_QUIC_PERMITTED_OPTIONS_CONN        \
    (OSSL_LEGACY_SSL_OPTIONS                  | \
     OSSL_TLS1_2_OPTIONS                      | \
     SSL_OP_CIPHER_SERVER_PREFERENCE          | \
     SSL_OP_DISABLE_TLSEXT_CA_NAMES           | \
     SSL_OP_NO_TX_CERTIFICATE_COMPRESSION     | \
     SSL_OP_NO_RX_CERTIFICATE_COMPRESSION     | \
     SSL_OP_PRIORITIZE_CHACHA                 | \
     SSL_OP_NO_QUERY_MTU                      | \
     SSL_OP_NO_TICKET                         | \
     SSL_OP_NO_ANTI_REPLAY                    )

/* Total mask of stream-level options permitted or ignored under QUIC. */
#define OSSL_QUIC_PERMITTED_OPTIONS_STREAM      \
    (OSSL_LEGACY_SSL_OPTIONS                  | \
     OSSL_TLS1_2_OPTIONS                      | \
     SSL_OP_CLEANSE_PLAINTEXT                 )

/* Total mask of options permitted on either connections or streams. */
#define OSSL_QUIC_PERMITTED_OPTIONS             \
    (OSSL_QUIC_PERMITTED_OPTIONS_CONN |         \
     OSSL_QUIC_PERMITTED_OPTIONS_STREAM)

/* Total mask of domain flags supported on a QUIC SSL_CTX. */
#define OSSL_QUIC_SUPPORTED_DOMAIN_FLAGS        \
    (SSL_DOMAIN_FLAG_SINGLE_THREAD |            \
     SSL_DOMAIN_FLAG_MULTI_THREAD |             \
     SSL_DOMAIN_FLAG_THREAD_ASSISTED |          \
     SSL_DOMAIN_FLAG_BLOCKING |                 \
     SSL_DOMAIN_FLAG_LEGACY_BLOCKING)

#endif
