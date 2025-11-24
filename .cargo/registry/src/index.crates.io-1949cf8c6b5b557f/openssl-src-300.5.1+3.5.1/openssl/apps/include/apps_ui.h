/*
 * Copyright 2018-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_APPS_UI_H
# define OSSL_APPS_UI_H


# define PW_MIN_LENGTH 4
typedef struct pw_cb_data {
    const void *password;
    const char *prompt_info;
} PW_CB_DATA;

int password_callback(char *buf, int bufsiz, int verify, PW_CB_DATA *cb_data);

int setup_ui_method(void);
void destroy_ui_method(void);
int set_base_ui_method(const UI_METHOD *ui_method);
const UI_METHOD *get_ui_method(void);

extern BIO *bio_err;

#endif
