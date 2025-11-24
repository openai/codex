/*
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2015-2022
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * A collection of test cases where check-format.pl should not report issues.
 * There are some known false positives, though, which are marked below using /*@
 */

#include <errno.h> /* should not report whitespace nits within <...> */
#define F                                       \
    void f()                                    \
    {                                           \
        int i;                                  \
        int j;                                  \
                                                \
        return;                                 \
    }

/* allow extra  SPC in single-line comment */
/*
 * allow extra  SPC in regular multi-line comment
 */
/*-
 * allow extra  SPC in format-tagged multi-line comment
 */
/** allow extra '*' in comment opening */
/*! allow extra '!' in comment opening */
/*
 ** allow "**" as first non-space chars of a line within multi-line comment
 */

int f(void) /*
             * trailing multi-line comment
             */
{
    typedef int INT;
    void v;
    short b;
    char c;
    signed s;
    unsigned u;
    int i;
    long l;
    float f;
    double d;
    enum {} enu;
    struct {} stru;
    union {} un;
    auto a;
    extern e;
    static int stat;
    const int con;
    volatile int vola;
    register int reg;
    OSSL_x y, *p = params;
    int params[];
    OSSL_PARAM * (* params []) [MAX + 1];
    XY *(* fn)(int a, char b);
    /*
     * multi-line comment should not disturb detection of local decls
     */
    BIO1 ***b;
    /* intra-line comment should not disturb detection of local decls */
    unsigned k;

    /* intra-line comment should not disturb detection of end of local decls */

    {
        int x; /* just decls in block */
    }
    if (p != (unsigned char *)
        &(ctx->tmp[0])) {
        i -= (p - (unsigned char *) /* do not confuse with var decl */
              &(ctx->tmp[0]));
    }
    {
        ctx->buf_off = 0; /* do not confuse with var decl */
        return 0;
    }
    {
        ctx->buf_len = EVP_EncodeBlock((unsigned char *)ctx->buf,
                                       (unsigned char *)ctx->tmp, /* no decl */
                                       ctx->tmp_len);
    }
    {
        EVP_EncodeFinal(ctx->base64,
                        (unsigned char *)ctx->buf, &(ctx->len)); /* no decl */
        /* push out the bytes */
        goto again;
    }
    {
        f(1, (unsigned long)2); /* no decl */
        x;
    }
    {
        char *pass_str = get_passwd(opt_srv_secret, "x");

        if (pass_str != NULL) {
            cleanse(opt_srv_secret);
            res = OSSL_CMP_CTX_set1_secretValue(ctx, (unsigned char *)pass_str,
                                                strlen(pass_str));
            clear_free(pass_str);
        }
    }
}

int g(void)
{
    if (ctx == NULL) {    /* non-leading end-of-line comment */
        if (/* comment after '(' */ pem_name != NULL /* comment before ')' */)
            /* entire-line comment indent usually like for the following line */
            return NULL; /* hanging indent also for this line after comment */
        /* leading comment has same indentation as normal code */ stmt;
        /* entire-line comment may have same indent as normal code */
    }
    for (i = 0; i < n; i++)
        for (; i < n; i++)
            for (i = 0; ; i++)
                for (i = 0;; i++)
                    for (i = 0; i < n; )
                        for (i = 0; i < n;)
                            ;
    for (i = 0; ; )
        for (i = 0; ;)
            for (i = 0;; )
                for (i = 0;;)
                    for (; i < n; )
                        for (; j < n;)
                            for (; ; i++)
                                for (;; i++)
                                    ;
    for (;;) /* the only variant allowed in case of "empty" for (...) */
        ;
    for (;;) ; /* should not trigger: space before ';' */
 lab: ;  /* should not trigger: space before ';' */

#if X
    if (1) /* bad style: just part of control structure depends on #if */
#else
    if (2) /*@ resulting false positive */
#endif
        c; /*@ resulting false positive */

    if (1)
        if (2)
            c;
        else
            e;
    else
        f;
    do
        do
            2;
        while (1);
    while (2);

    if (pcrl != NULL) {
        1;
        2;
    } else if (pcrls != NULL) {
        1;
    }

    if (1)
        f(a, b);
    do
        1; while (2); /*@ more than one stmt just to construct case */
    if (1)
        f(a, b);
    else
        do
            1;
        while (2);
    if (1)
        f(a, b);
    else do /*@ (non-brace) code before 'do' just to construct case */
             1;
        while (2);
    f1234(a,
          b); do /*@ (non-brace) code before 'do' just to construct case */
                  1;
    while (2);
    if (1)
        f(a,
          b); do /*@ (non-brace) code before 'do' just to construct case */
                  1;
    while (2);
    if (1)
        f(a, b);
    else
        do f(c, c); /*@ (non-brace) code after 'do' just to construct case */
        while (2);

    if (1)
        f(a, b);
    else
        return;
    if (1)
        f(a,
          b); else /*@ (non-brace) code before 'else' just to construct case */
        do
            1;
        while (2);

    if (1)
    { /*@ brace after 'if' not on same line just to construct case */
        c;
        d;
    }
    /* this comment is correctly indented if it refers to the following line */
    d;

    if (1) {
        2;
    } else /*@ no brace after 'else' just to construct case */
        3;
    do {
    } while (x);
    if (1) {
        2;
    } else {
        3;
    }
    if (4)
        5;
    else
        6;

    if (1) {
        if (2) {
        case MAC_TYPE_MAC:
            {
                EVP_MAC_CTX *new_mac_ctx;

                if (ctx->pkey == NULL)
                    return 0;
            }
            break;
        case 1: {
            ;
        }
        default:
            /* This should be dead code */
            return 0;
        }
    }
    if (expr_line1
        == expr_line2
            && expr_line3) {
        c1;
    } else {
        c;
        d;
    }
    if (expr_line1
        == expr_line2
            && expr_line3)
        hanging_stmt;
}

#define m1                           \
    if (ctx == NULL)                 \
        return 0;                    \
    if (ossl_param_is_empty(params)) \
        return 1;                    \

#define m2                                                               \
    do { /* should not be confused with function header followed by '{' */ \
    } while (0)

/* should not trigger: constant on LHS of comparison or assignment operator */
X509 *x509 = NULL;
int y = a + 1 < b;
int ret, was_NULL = *certs == NULL;

/* should not trigger: missing space before ... */
float z = 1e-6 * (-1) * b[+6] * 1e+1 * (a)->f * (long)+1
    - (tmstart.tv_sec + tmstart.tv_nsec * 1e-9);
struct st = {-1, 0};
int x = (y <<= 1) + (z <= 5.0);

const OPTIONS passwd_options[] = {
    {"aixmd5", OPT_AIXMD5, '-', "AIX MD5-based password algorithm"},
#if !defined(OPENSSL_NO_DES) && !defined(OPENSSL_NO_DEPRECATED_3_0)
    {"crypt", OPT_CRYPT, '-', "Standard Unix password algorithm (default)"},
#endif
    OPT_R_OPTIONS,

    {NULL}
};

typedef bool (*LOG_cb_t)(int lineno, severity level, const char *msg);
typedef * d(int)
    x;
typedef (int)
x;
typedef (int)*()
    x;
typedef *int *
x;
typedef OSSL_CMP_MSG *(*cmp_srv_process_cb_t)
    (OSSL_CMP_SRV_CTX *ctx, OSSL_CMP_MSG *msg)
    xx;

#define IF(cond) if (cond)

_Pragma("GCC diagnostic push")
_Pragma("GCC diagnostic pop")

#define CB_ERR_IF(cond, ctx, cert, depth, err) \
    if ((cond) && ((depth) < 0 || verify_cb_cert(ctx, cert, depth, err) == 0)) \
        return err
static int verify_cb_crl(X509_STORE_CTX *ctx, int err)
{
    ctx->error = err;
    return ctx->verify_cb(0, ctx);
}

#ifdef CMP_FALLBACK_EST
# define CMP_FALLBACK_CERT_FILE "cert.pem"
#endif

#define X509_OBJECT_get0_X509(obj)                                      \
    ((obj) == NULL || (obj)->type != X509_LU_X509 ? NULL : (obj)->data.x509)
#define X509_STORE_CTX_set_current_cert(ctx, x) { (ctx)->current_cert = (x); }
#define X509_STORE_set_ex_data(ctx, idx, data) \
    CRYPTO_set_ex_data(&(ctx)->ex_data, (idx), (data))

typedef int (*X509_STORE_CTX_check_revocation_fn)(X509_STORE_CTX *ctx);
#define X509_STORE_CTX_set_error_depth(ctx, depth) \
    { (ctx)->error_depth = (depth); }
#define EVP_PKEY_up_ref(x) ((x)->references++)
/* should not report missing blank line: */
DECLARE_STACK_OF(OPENSSL_CSTRING)
bool UTIL_iterate_dir(int (*fn)(const char *file, void *arg), void *arg,
                      const char *path, bool recursive);
size_t UTIL_url_encode(
                       size_t *size_needed
                       );
size_t UTIL_url_encode(const char  *source,
                       char        *destination,
                       size_t      destination_len,
                       size_t      *size_needed);
#error well. oops.

int f()
{
    c;
    if (1)
        c;
    c;
    if (1)
        if (2)
        { /*@ brace after 'if' not on same line just to construct case */
            c;
        }
    e;
    const usign = {
                   0xDF,
                   {
                    dd
                   },
                   dd
    };
    const unsign = {
                    0xDF, {
                           dd
                    },
                    dd
    };
}
const unsigned char trans_id[OSSL_CMP_TRANSACTIONID_LENGTH] = {
                                                               0xDF,
};
const unsigned char trans_id[OSSL_CMP_TRANSACTIONID_LENGTH] =
    {
     0xDF,
    };
typedef
int
a;

typedef
struct
{
    int a;
} b;
typedef enum {
              w = 0
} e_type;
typedef struct {
    enum {
          w = 0
    } e_type;
    enum {
          w = 0
    } e_type;
} e;
struct s_type {
    enum e_type {
                 w = 0
    };
};
struct s_type
{
    enum e_type {
                 w = 0
    };
    enum e2_type {
                  w = 0
    };
};

#define X  1          + 1
#define Y  /* .. */ 2 + 2
#define Z  3          + 3 * (*a++)

static varref cmp_vars[] = { /* comment.  comment?  comment!  */
    {&opt_config}, {&opt_section},

    {&opt_server}, {&opt_proxy}, {&opt_path},
};

#define SWITCH(x)                               \
    switch (x) {                                \
    case 0:                                     \
        break;                                  \
    default:                                    \
        break;                                  \
    }

#define DEFINE_SET_GET_BASE_TEST(PREFIX, SETN, GETN, DUP, FIELD, TYPE, ERR, \
                                 DEFAULT, NEW, FREE) \
    static int execute_CTX_##SETN##_##GETN##_##FIELD( \
                                                     TEST_FIXTURE *fixture) \
    { \
        CTX *ctx = fixture->ctx; \
        int (*set_fn)(CTX *ctx, TYPE) = \
            (int (*)(CTX *ctx, TYPE))PREFIX##_##SETN##_##FIELD; \
        /* comment */ \
    }

union un var; /* struct/union/enum in variable type */
struct provider_store_st *f() /* struct/union/enum in function return type */
{
}
static void f(struct pem_pass_data *data) /* struct/union/enum in arg list */
{
}

static void *fun(void)
{
    if (pem_name != NULL)
        /* comment */
        return NULL;

label0:
 label1: /* allow special indent 1 for label at outermost level in body */
    do {
    label2:
        size_t available_len, data_len;
        const char *curr = txt, *next = txt;
        char *tmp;

        {
        label3:
        }
    } while (1);

    char *intraline_string_with_comment_delimiters_and_dbl_space = "1  /*1";
    char *multiline_string_with_comment_delimiters_and_dbl_space = "1  /*1\
2222222\'22222222222222222\"222222222" "33333  /*3333333333" "44  /*44444444444\
55555555555555\
6666";
}

ASN1_CHOICE(OSSL_CRMF_POPO) = {
    ASN1_IMP(OSSL_CRMF_POPO, value.raVerified, ASN1_NULL, 0),
    ASN1_EXP(OSSL_CRMF_POPO, value.keyAgreement, OSSL_CRMF_POPOPRIVKEY, 3)
} ASN1_CHOICE_END(OSSL_CRMF_POPO)
IMPLEMENT_ASN1_FUNCTIONS(OSSL_CRMF_POPO)

ASN1_ADB(OSSL_CRMF_ATTRIBUTETYPEANDVALUE) = {
    ADB_ENTRY(NID_id_regCtrl_regToken,
              ASN1_SIMPLE(OSSL_CRMF_ATTRIBUTETYPEANDVALUE,
                          value.regToken, ASN1_UTF8STRING)),
} ASN1_ADB_END(OSSL_CRMF_ATTRIBUTETYPEANDVALUE, 0, type, 0,
               &attributetypeandvalue_default_tt, NULL);

ASN1_ITEM_TEMPLATE(OSSL_CRMF_MSGS) =
    ASN1_EX_TEMPLATE_TYPE(ASN1_TFLG_SEQUENCE_OF, 0,
                          OSSL_CRMF_MSGS, OSSL_CRMF_MSG)
ASN1_ITEM_TEMPLATE_END(OSSL_CRMF_MSGS)

void f_looong_body_200()
{ /* function body length up to 200 lines accepted */
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
}

void f_looong_body_201()
{ /* function body length > 200 lines, but LONG BODY marker present */
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
}
