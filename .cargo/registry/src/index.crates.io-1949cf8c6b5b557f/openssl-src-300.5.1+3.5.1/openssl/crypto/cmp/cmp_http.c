/*
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Nokia 2007-2019
 * Copyright Siemens AG 2015-2019
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>

#include <openssl/asn1t.h>
#include <openssl/http.h>

#include <openssl/cmp.h>
#include "cmp_local.h"

/* explicit #includes not strictly needed since implied by the above: */
#include <ctype.h>
#include <fcntl.h>
#include <stdlib.h>
#include <openssl/bio.h>
#include <openssl/buffer.h>
#include <openssl/err.h>

static int keep_alive(int keep_alive, int body_type, BIO **bios)
{
    if (keep_alive != 0 && bios == NULL
        /*
         * Ask for persistent connection only if may need more round trips.
         * Do so even with disableConfirm because polling might be needed.
         */
            && body_type != OSSL_CMP_PKIBODY_IR
            && body_type != OSSL_CMP_PKIBODY_CR
            && body_type != OSSL_CMP_PKIBODY_P10CR
            && body_type != OSSL_CMP_PKIBODY_KUR
            && body_type != OSSL_CMP_PKIBODY_POLLREQ)
        keep_alive = 0;
    return keep_alive;
}

/*
 * Send the PKIMessage req and on success return the response, else NULL.
 */
OSSL_CMP_MSG *OSSL_CMP_MSG_http_perform(OSSL_CMP_CTX *ctx,
                                        const OSSL_CMP_MSG *req)
{
    char server_port[32] = { '\0' };
    STACK_OF(CONF_VALUE) *headers = NULL;
    const char content_type_pkix[] = "application/pkixcmp";
    int tls_used;
    const ASN1_ITEM *it = ASN1_ITEM_rptr(OSSL_CMP_MSG);
    BIO *req_mem, *rsp;
    BIO **bios; /* optionally used as bio and rbio */
    OSSL_CMP_MSG *res = NULL;

    if (ctx == NULL || req == NULL) {
        ERR_raise(ERR_LIB_CMP, CMP_R_NULL_ARGUMENT);
        return NULL;
    }

    if (!X509V3_add_value("Pragma", "no-cache", &headers))
        return NULL;
    if ((req_mem = ASN1_item_i2d_mem_bio(it, (const ASN1_VALUE *)req)) == NULL)
        goto err;

    bios = OSSL_CMP_CTX_get_transfer_cb_arg(ctx);
    if (ctx->serverPort != 0)
        BIO_snprintf(server_port, sizeof(server_port), "%d", ctx->serverPort);
    tls_used = ctx->tls_used >= 0 ? ctx->tls_used != 0
        : OSSL_CMP_CTX_get_http_cb_arg(ctx) != NULL; /* backward compat */
    if (ctx->http_ctx == NULL) { /* using existing connection or yet not set up own connection */
        const char *path = ctx->serverPath;

        if (path == NULL)
            path = "";
        if (*path == '/')
            path++;
        if (bios == NULL)
            ossl_cmp_log4(DEBUG, ctx,
                          "connecting to CMP server via http%s://%s:%s/%s",
                          tls_used ? "s" : "", ctx->server, server_port, path);
        else
            ossl_cmp_log3(DEBUG, ctx,
                          "using existing connection with CMP server %s:%s and HTTP path /%s",
                          ctx->server, server_port, path);
    }

    rsp = OSSL_HTTP_transfer(&ctx->http_ctx, ctx->server, server_port,
                             ctx->serverPath, tls_used,
                             ctx->proxy, ctx->no_proxy,
                             bios == NULL ? NULL : bios[0] /* bio */,
                             bios == NULL ? NULL : bios[1] /* rbio */,
                             ctx->http_cb, OSSL_CMP_CTX_get_http_cb_arg(ctx),
                             0 /* buf_size */, headers,
                             content_type_pkix, req_mem,
                             content_type_pkix, 1 /* expect_asn1 */,
                             OSSL_HTTP_DEFAULT_MAX_RESP_LEN,
                             ctx->msg_timeout,
                             keep_alive(ctx->keep_alive, req->body->type, bios));
    BIO_free(req_mem);
    res = (OSSL_CMP_MSG *)ASN1_item_d2i_bio(it, rsp, NULL);
    BIO_free(rsp);

    if (ctx->http_ctx == NULL)
        ossl_cmp_debug(ctx, "disconnected from CMP server");
    /*
     * Note that on normal successful end of the transaction the
     * HTTP connection is not closed at this level if keep_alive(...) != 0.
     * It should be closed by the CMP client application
     * using OSSL_CMP_CTX_free() or OSSL_CMP_CTX_reinit().
     * Any pre-existing bio (== ctx->transfer_cb_arg) is not freed.
     */
    if (res != NULL)
        ossl_cmp_debug(ctx, "finished reading response from CMP server");
 err:
    sk_CONF_VALUE_pop_free(headers, X509V3_conf_free);
    return res;
}
