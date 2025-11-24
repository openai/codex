/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ocsp.h>
#include <openssl/http.h>

#ifndef OPENSSL_NO_OCSP

OSSL_HTTP_REQ_CTX *OCSP_sendreq_new(BIO *io, const char *path,
                                    const OCSP_REQUEST *req, int buf_size)
{
    OSSL_HTTP_REQ_CTX *rctx = OSSL_HTTP_REQ_CTX_new(io, io, buf_size);

    if (rctx == NULL)
        return NULL;
    /*-
     * by default:
     * no bio_update_fn (and consequently no arg)
     * no ssl
     * no proxy
     * no timeout (blocking indefinitely)
     * no expected content type
     * max_resp_len = 100 KiB
     */
    if (!OSSL_HTTP_REQ_CTX_set_request_line(rctx, 1 /* POST */,
                                            NULL, NULL, path))
        goto err;
    /* by default, no extra headers */
    if (!OSSL_HTTP_REQ_CTX_set_expected(rctx,
                                        NULL /* content_type */, 1 /* asn1 */,
                                        0 /* timeout */, 0 /* keep_alive */))
        goto err;
    if (req != NULL
        && !OSSL_HTTP_REQ_CTX_set1_req(rctx, "application/ocsp-request",
                                       ASN1_ITEM_rptr(OCSP_REQUEST),
                                       (const ASN1_VALUE *)req))
        goto err;
    return rctx;

 err:
    OSSL_HTTP_REQ_CTX_free(rctx);
    return NULL;
}

OCSP_RESPONSE *OCSP_sendreq_bio(BIO *b, const char *path, OCSP_REQUEST *req)
{
    OCSP_RESPONSE *resp = NULL;
    OSSL_HTTP_REQ_CTX *ctx;
    BIO *mem;

    ctx = OCSP_sendreq_new(b, path, req, 0 /* default buf_size */);
    if (ctx == NULL)
        return NULL;
    mem = OSSL_HTTP_REQ_CTX_exchange(ctx);
    /* ASN1_item_d2i_bio handles NULL bio gracefully */
    resp = (OCSP_RESPONSE *)ASN1_item_d2i_bio(ASN1_ITEM_rptr(OCSP_RESPONSE),
                                              mem, NULL);

    OSSL_HTTP_REQ_CTX_free(ctx);
    return resp;
}
#endif /* !defined(OPENSSL_NO_OCSP) */
