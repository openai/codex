/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/bn.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include "crypto/asn1.h"
#include "crypto/x509.h"

void OSSL_STACK_OF_X509_free(STACK_OF(X509) *certs)
{
    sk_X509_pop_free(certs, X509_free);
}

#ifndef OPENSSL_NO_STDIO
int X509_print_fp(FILE *fp, X509 *x)
{
    return X509_print_ex_fp(fp, x, XN_FLAG_COMPAT, X509_FLAG_COMPAT);
}

int X509_print_ex_fp(FILE *fp, X509 *x, unsigned long nmflag,
                     unsigned long cflag)
{
    BIO *b;
    int ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_BUF_LIB);
        return 0;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = X509_print_ex(b, x, nmflag, cflag);
    BIO_free(b);
    return ret;
}
#endif

int X509_print(BIO *bp, X509 *x)
{
    return X509_print_ex(bp, x, XN_FLAG_COMPAT, X509_FLAG_COMPAT);
}

int X509_print_ex(BIO *bp, X509 *x, unsigned long nmflags,
                  unsigned long cflag)
{
    long l;
    int ret = 0;
    char mlch = ' ';
    int nmindent = 0, printok = 0;
    EVP_PKEY *pkey = NULL;

    if ((nmflags & XN_FLAG_SEP_MASK) == XN_FLAG_SEP_MULTILINE) {
        mlch = '\n';
        nmindent = 12;
    }

    if (nmflags == XN_FLAG_COMPAT)
        printok = 1;

    if (!(cflag & X509_FLAG_NO_HEADER)) {
        if (BIO_write(bp, "Certificate:\n", 13) <= 0)
            goto err;
        if (BIO_write(bp, "    Data:\n", 10) <= 0)
            goto err;
    }
    if (!(cflag & X509_FLAG_NO_VERSION)) {
        l = X509_get_version(x);
        if (l >= X509_VERSION_1 && l <= X509_VERSION_3) {
            if (BIO_printf(bp, "%8sVersion: %ld (0x%lx)\n", "", l + 1, (unsigned long)l) <= 0)
                goto err;
        } else {
            if (BIO_printf(bp, "%8sVersion: Unknown (%ld)\n", "", l) <= 0)
                goto err;
        }
    }
    if (!(cflag & X509_FLAG_NO_SERIAL)) {
        const ASN1_INTEGER *bs = X509_get0_serialNumber(x);

        if (BIO_write(bp, "        Serial Number:", 22) <= 0)
            goto err;
        if (ossl_serial_number_print(bp, bs, 12) != 0)
            goto err;
        if (BIO_puts(bp, "\n") <= 0)
            goto err;
    }

    if (!(cflag & X509_FLAG_NO_SIGNAME)) {
        const X509_ALGOR *tsig_alg = X509_get0_tbs_sigalg(x);

        if (BIO_puts(bp, "    ") <= 0)
            goto err;
        if (X509_signature_print(bp, tsig_alg, NULL) <= 0)
            goto err;
    }

    if (!(cflag & X509_FLAG_NO_ISSUER)) {
        if (BIO_printf(bp, "        Issuer:%c", mlch) <= 0)
            goto err;
        if (X509_NAME_print_ex(bp, X509_get_issuer_name(x), nmindent, nmflags)
            < printok)
            goto err;
        if (BIO_write(bp, "\n", 1) <= 0)
            goto err;
    }
    if (!(cflag & X509_FLAG_NO_VALIDITY)) {
        if (BIO_write(bp, "        Validity\n", 17) <= 0)
            goto err;
        if (BIO_write(bp, "            Not Before: ", 24) <= 0)
            goto err;
        if (ossl_asn1_time_print_ex(bp, X509_get0_notBefore(x), ASN1_DTFLGS_RFC822) == 0)
            goto err;
        if (BIO_write(bp, "\n            Not After : ", 25) <= 0)
            goto err;
        if (ossl_asn1_time_print_ex(bp, X509_get0_notAfter(x), ASN1_DTFLGS_RFC822) == 0)
            goto err;
        if (BIO_write(bp, "\n", 1) <= 0)
            goto err;
    }
    if (!(cflag & X509_FLAG_NO_SUBJECT)) {
        if (BIO_printf(bp, "        Subject:%c", mlch) <= 0)
            goto err;
        if (X509_NAME_print_ex
            (bp, X509_get_subject_name(x), nmindent, nmflags) < printok)
            goto err;
        if (BIO_write(bp, "\n", 1) <= 0)
            goto err;
    }
    if (!(cflag & X509_FLAG_NO_PUBKEY)) {
        X509_PUBKEY *xpkey = X509_get_X509_PUBKEY(x);
        ASN1_OBJECT *xpoid;
        X509_PUBKEY_get0_param(&xpoid, NULL, NULL, NULL, xpkey);
        if (BIO_write(bp, "        Subject Public Key Info:\n", 33) <= 0)
            goto err;
        if (BIO_printf(bp, "%12sPublic Key Algorithm: ", "") <= 0)
            goto err;
        if (i2a_ASN1_OBJECT(bp, xpoid) <= 0)
            goto err;
        if (BIO_puts(bp, "\n") <= 0)
            goto err;

        pkey = X509_get0_pubkey(x);
        if (pkey == NULL) {
            BIO_printf(bp, "%12sUnable to load Public Key\n", "");
            ERR_print_errors(bp);
        } else {
            EVP_PKEY_print_public(bp, pkey, 16, NULL);
        }
    }

    if (!(cflag & X509_FLAG_NO_IDS)) {
        const ASN1_BIT_STRING *iuid, *suid;
        X509_get0_uids(x, &iuid, &suid);
        if (iuid != NULL) {
            if (BIO_printf(bp, "%8sIssuer Unique ID: ", "") <= 0)
                goto err;
            if (!X509_signature_dump(bp, iuid, 12))
                goto err;
        }
        if (suid != NULL) {
            if (BIO_printf(bp, "%8sSubject Unique ID: ", "") <= 0)
                goto err;
            if (!X509_signature_dump(bp, suid, 12))
                goto err;
        }
    }

    if (!(cflag & X509_FLAG_NO_EXTENSIONS)
        && !X509V3_extensions_print(bp, "X509v3 extensions",
                                    X509_get0_extensions(x), cflag, 8))
        goto err;

    if (!(cflag & X509_FLAG_NO_SIGDUMP)) {
        const X509_ALGOR *sig_alg;
        const ASN1_BIT_STRING *sig;
        X509_get0_signature(&sig, &sig_alg, x);
        if (X509_signature_print(bp, sig_alg, sig) <= 0)
            goto err;
    }
    if (!(cflag & X509_FLAG_NO_AUX)) {
        if (!X509_aux_print(bp, x, 0))
            goto err;
    }
    ret = 1;
 err:
    return ret;
}

int X509_ocspid_print(BIO *bp, X509 *x)
{
    unsigned char *der = NULL;
    unsigned char *dertmp;
    int derlen;
    int i;
    unsigned char SHA1md[SHA_DIGEST_LENGTH];
    ASN1_BIT_STRING *keybstr;
    const X509_NAME *subj;
    EVP_MD *md = NULL;

    if (x == NULL || bp == NULL)
        return 0;
    /*
     * display the hash of the subject as it would appear in OCSP requests
     */
    if (BIO_printf(bp, "        Subject OCSP hash: ") <= 0)
        goto err;
    subj = X509_get_subject_name(x);
    derlen = i2d_X509_NAME(subj, NULL);
    if (derlen <= 0)
        goto err;
    if ((der = dertmp = OPENSSL_malloc(derlen)) == NULL)
        goto err;
    i2d_X509_NAME(subj, &dertmp);

    md = EVP_MD_fetch(x->libctx, SN_sha1, x->propq);
    if (md == NULL)
        goto err;
    if (!EVP_Digest(der, derlen, SHA1md, NULL, md, NULL))
        goto err;
    for (i = 0; i < SHA_DIGEST_LENGTH; i++) {
        if (BIO_printf(bp, "%02X", SHA1md[i]) <= 0)
            goto err;
    }
    OPENSSL_free(der);
    der = NULL;

    /*
     * display the hash of the public key as it would appear in OCSP requests
     */
    if (BIO_printf(bp, "\n        Public key OCSP hash: ") <= 0)
        goto err;

    keybstr = X509_get0_pubkey_bitstr(x);

    if (keybstr == NULL)
        goto err;

    if (!EVP_Digest(ASN1_STRING_get0_data(keybstr),
                    ASN1_STRING_length(keybstr), SHA1md, NULL, md, NULL))
        goto err;
    for (i = 0; i < SHA_DIGEST_LENGTH; i++) {
        if (BIO_printf(bp, "%02X", SHA1md[i]) <= 0)
            goto err;
    }
    BIO_printf(bp, "\n");
    EVP_MD_free(md);

    return 1;
 err:
    OPENSSL_free(der);
    EVP_MD_free(md);
    return 0;
}

int X509_signature_dump(BIO *bp, const ASN1_STRING *sig, int indent)
{
    const unsigned char *s;
    int i, n;

    n = sig->length;
    s = sig->data;
    for (i = 0; i < n; i++) {
        if ((i % 18) == 0) {
            if (i > 0 && BIO_write(bp, "\n", 1) <= 0)
                return 0;
            if (BIO_indent(bp, indent, indent) <= 0)
                return 0;
        }
        if (BIO_printf(bp, "%02x%s", s[i], ((i + 1) == n) ? "" : ":") <= 0)
            return 0;
    }
    if (BIO_write(bp, "\n", 1) != 1)
        return 0;

    return 1;
}

int X509_signature_print(BIO *bp, const X509_ALGOR *sigalg,
                         const ASN1_STRING *sig)
{
    int sig_nid;
    int indent = 4;
    if (BIO_printf(bp, "%*sSignature Algorithm: ", indent, "") <= 0)
        return 0;
    if (i2a_ASN1_OBJECT(bp, sigalg->algorithm) <= 0)
        return 0;

    if (sig && BIO_printf(bp, "\n%*sSignature Value:", indent, "") <= 0)
        return 0;
    sig_nid = OBJ_obj2nid(sigalg->algorithm);
    if (sig_nid != NID_undef) {
        int pkey_nid, dig_nid;
        const EVP_PKEY_ASN1_METHOD *ameth;
        if (OBJ_find_sigid_algs(sig_nid, &dig_nid, &pkey_nid)) {
            ameth = EVP_PKEY_asn1_find(NULL, pkey_nid);
            if (ameth && ameth->sig_print)
                return ameth->sig_print(bp, sigalg, sig, indent + 4, 0);
        }
    }
    if (BIO_write(bp, "\n", 1) != 1)
        return 0;
    if (sig)
        return X509_signature_dump(bp, sig, indent + 4);
    return 1;
}

int X509_aux_print(BIO *out, X509 *x, int indent)
{
    char oidstr[80], first;
    STACK_OF(ASN1_OBJECT) *trust, *reject;
    const unsigned char *alias, *keyid;
    int keyidlen;
    int i;
    if (X509_trusted(x) == 0)
        return 1;
    trust = X509_get0_trust_objects(x);
    reject = X509_get0_reject_objects(x);
    if (trust) {
        first = 1;
        BIO_printf(out, "%*sTrusted Uses:\n%*s", indent, "", indent + 2, "");
        for (i = 0; i < sk_ASN1_OBJECT_num(trust); i++) {
            if (!first)
                BIO_puts(out, ", ");
            else
                first = 0;
            OBJ_obj2txt(oidstr, sizeof(oidstr),
                        sk_ASN1_OBJECT_value(trust, i), 0);
            BIO_puts(out, oidstr);
        }
        BIO_puts(out, "\n");
    } else
        BIO_printf(out, "%*sNo Trusted Uses.\n", indent, "");
    if (reject) {
        first = 1;
        BIO_printf(out, "%*sRejected Uses:\n%*s", indent, "", indent + 2, "");
        for (i = 0; i < sk_ASN1_OBJECT_num(reject); i++) {
            if (!first)
                BIO_puts(out, ", ");
            else
                first = 0;
            OBJ_obj2txt(oidstr, sizeof(oidstr),
                        sk_ASN1_OBJECT_value(reject, i), 0);
            BIO_puts(out, oidstr);
        }
        BIO_puts(out, "\n");
    } else
        BIO_printf(out, "%*sNo Rejected Uses.\n", indent, "");
    alias = X509_alias_get0(x, &i);
    if (alias)
        BIO_printf(out, "%*sAlias: %.*s\n", indent, "", i, alias);
    keyid = X509_keyid_get0(x, &keyidlen);
    if (keyid) {
        BIO_printf(out, "%*sKey Id: ", indent, "");
        for (i = 0; i < keyidlen; i++)
            BIO_printf(out, "%s%02X", i ? ":" : "", keyid[i]);
        BIO_write(out, "\n", 1);
    }
    return 1;
}

/*
 * Helper functions for improving certificate verification error diagnostics
 */

int ossl_x509_print_ex_brief(BIO *bio, X509 *cert, unsigned long neg_cflags)
{
    unsigned long flags = ASN1_STRFLGS_RFC2253 | ASN1_STRFLGS_ESC_QUOTE |
        XN_FLAG_SEP_CPLUS_SPC | XN_FLAG_FN_SN;

    if (cert == NULL)
        return BIO_printf(bio, "    (no certificate)\n") > 0;
    if (BIO_printf(bio, "    certificate\n") <= 0
            || !X509_print_ex(bio, cert, flags, ~X509_FLAG_NO_SUBJECT))
        return 0;
    if (X509_check_issued((X509 *)cert, cert) == X509_V_OK) {
        if (BIO_printf(bio, "        self-issued\n") <= 0)
            return 0;
    } else {
        if (BIO_printf(bio, " ") <= 0
            || !X509_print_ex(bio, cert, flags, ~X509_FLAG_NO_ISSUER))
            return 0;
    }
    if (!X509_print_ex(bio, cert, flags,
                       ~(X509_FLAG_NO_SERIAL | X509_FLAG_NO_VALIDITY)))
        return 0;
    if (X509_cmp_current_time(X509_get0_notBefore(cert)) > 0)
        if (BIO_printf(bio, "        not yet valid\n") <= 0)
            return 0;
    if (X509_cmp_current_time(X509_get0_notAfter(cert)) < 0)
        if (BIO_printf(bio, "        no more valid\n") <= 0)
            return 0;
    return X509_print_ex(bio, cert, flags,
                         ~neg_cflags & ~X509_FLAG_EXTENSIONS_ONLY_KID);
}

static int print_certs(BIO *bio, const STACK_OF(X509) *certs)
{
    int i;

    if (certs == NULL || sk_X509_num(certs) <= 0)
        return BIO_printf(bio, "    (no certificates)\n") >= 0;

    for (i = 0; i < sk_X509_num(certs); i++) {
        X509 *cert = sk_X509_value(certs, i);

        if (cert != NULL) {
            if (!ossl_x509_print_ex_brief(bio, cert, 0))
                return 0;
            if (!X509V3_extensions_print(bio, NULL,
                                         X509_get0_extensions(cert),
                                         X509_FLAG_EXTENSIONS_ONLY_KID, 8))
                return 0;
            }
    }
    return 1;
}

static int print_store_certs(BIO *bio, X509_STORE *store)
{
    if (store != NULL) {
        STACK_OF(X509) *certs = X509_STORE_get1_all_certs(store);
        int ret = print_certs(bio, certs);

        OSSL_STACK_OF_X509_free(certs);
        return ret;
    } else {
        return BIO_printf(bio, "    (no trusted store)\n") >= 0;
    }
}

/* Extend the error queue with details on a failed cert verification */
int X509_STORE_CTX_print_verify_cb(int ok, X509_STORE_CTX *ctx)
{
    if (ok == 0 && ctx != NULL) {
        int cert_error = X509_STORE_CTX_get_error(ctx);
        BIO *bio = BIO_new(BIO_s_mem()); /* may be NULL */

        if (bio == NULL)
            return 0;
        BIO_printf(bio, "%s at depth = %d error = %d (%s)\n",
                   X509_STORE_CTX_get0_parent_ctx(ctx) != NULL
                   ? "CRL path validation"
                   : "Certificate verification",
                   X509_STORE_CTX_get_error_depth(ctx),
                   cert_error, X509_verify_cert_error_string(cert_error));
        {
            X509_STORE *ts = X509_STORE_CTX_get0_store(ctx);
            X509_VERIFY_PARAM *vpm = X509_STORE_get0_param(ts);
            char *str;
            int idx = 0;

            switch (cert_error) {
            case X509_V_ERR_HOSTNAME_MISMATCH:
                BIO_printf(bio, "Expected hostname(s) = ");
                while ((str = X509_VERIFY_PARAM_get0_host(vpm, idx++)) != NULL)
                    BIO_printf(bio, "%s%s", idx == 1 ? "" : ", ", str);
                BIO_printf(bio, "\n");
                break;
            case X509_V_ERR_EMAIL_MISMATCH:
                str = X509_VERIFY_PARAM_get0_email(vpm);
                if (str != NULL)
                    BIO_printf(bio, "Expected email address = %s\n", str);
                break;
            case X509_V_ERR_IP_ADDRESS_MISMATCH:
                str = X509_VERIFY_PARAM_get1_ip_asc(vpm);
                if (str != NULL)
                    BIO_printf(bio, "Expected IP address = %s\n", str);
                OPENSSL_free(str);
                break;
            default:
                break;
            }
        }

        BIO_printf(bio, "Failure for:\n");
        ossl_x509_print_ex_brief(bio, X509_STORE_CTX_get_current_cert(ctx),
                                 X509_FLAG_NO_EXTENSIONS);
        if (cert_error == X509_V_ERR_CERT_UNTRUSTED
                || cert_error == X509_V_ERR_DEPTH_ZERO_SELF_SIGNED_CERT
                || cert_error == X509_V_ERR_SELF_SIGNED_CERT_IN_CHAIN
                || cert_error == X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT
                || cert_error == X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY
                || cert_error == X509_V_ERR_UNABLE_TO_GET_CRL_ISSUER
                || cert_error == X509_V_ERR_STORE_LOOKUP) {
            BIO_printf(bio, "Non-trusted certs:\n");
            print_certs(bio, X509_STORE_CTX_get0_untrusted(ctx));
            BIO_printf(bio, "Certs in trust store:\n");
            print_store_certs(bio, X509_STORE_CTX_get0_store(ctx));
        }
        ERR_raise(ERR_LIB_X509, X509_R_CERTIFICATE_VERIFICATION_FAILED);
        ERR_add_error_mem_bio("\n", bio);
        BIO_free(bio);
    }

    return ok;
}

/*
 * Prints serial numbers in decimal and hexadecimal. The indent argument is only
 * used if the serial number is too large to fit in an int64_t.
 */
int ossl_serial_number_print(BIO *out, const ASN1_INTEGER *bs, int indent)
{
    int i, ok;
    int64_t l;
    uint64_t ul;
    const char *neg;

    if (bs->length == 0) {
        if (BIO_puts(out, " (Empty)") <= 0)
            return -1;
        return 0;
    }

    ERR_set_mark();
    ok = ASN1_INTEGER_get_int64(&l, bs);
    ERR_pop_to_mark();

    if (ok) { /* Reading an int64_t succeeded: print decimal and hex. */
        if (bs->type == V_ASN1_NEG_INTEGER) {
            ul = 0 - (uint64_t)l;
            neg = "-";
        } else {
            ul = l;
            neg = "";
        }
        if (BIO_printf(out, " %s%ju (%s0x%jx)", neg, ul, neg, ul) <= 0)
            return -1;
    } else { /* Reading an int64_t failed: just print hex. */
        neg = (bs->type == V_ASN1_NEG_INTEGER) ? " (Negative)" : "";
        if (BIO_printf(out, "\n%*s%s", indent, "", neg) <= 0)
            return -1;

        for (i = 0; i < bs->length - 1; i++) {
            if (BIO_printf(out, "%02x%c", bs->data[i], ':') <= 0)
                return -1;
        }
        if (BIO_printf(out, "%02x", bs->data[i]) <= 0)
            return -1;
    }
    return 0;
}
