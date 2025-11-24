/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <string.h>
#include <openssl/opensslconf.h>
#include <openssl/err.h>
#include "apps_ui.h"
#include "testutil.h"


#include <openssl/ui.h>

/* Old style PEM password callback */
static int test_pem_password_cb(char *buf, int size, int rwflag, void *userdata)
{
    OPENSSL_strlcpy(buf, (char *)userdata, (size_t)size);
    return strlen(buf);
}

/*
 * Test wrapping old style PEM password callback in a UI method through the
 * use of UI utility functions
 */
static int test_old(void)
{
    UI_METHOD *ui_method = NULL;
    UI *ui = NULL;
    char defpass[] = "password";
    char pass[16];
    int ok = 0;

    if (!TEST_ptr(ui_method =
                  UI_UTIL_wrap_read_pem_callback(test_pem_password_cb, 0))
            || !TEST_ptr(ui = UI_new_method(ui_method)))
        goto err;

    /* The wrapper passes the UI userdata as the callback userdata param */
    UI_add_user_data(ui, defpass);

    if (UI_add_input_string(ui, "prompt", UI_INPUT_FLAG_DEFAULT_PWD,
                             pass, 0, sizeof(pass) - 1) <= 0)
        goto err;

    switch (UI_process(ui)) {
    case -2:
        TEST_info("test_old: UI process interrupted or cancelled");
        /* fall through */
    case -1:
        goto err;
    default:
        break;
    }

    if (TEST_str_eq(pass, defpass))
        ok = 1;

 err:
    UI_free(ui);
    UI_destroy_method(ui_method);

    return ok;
}

/* Test of UI.  This uses the UI method defined in apps/apps.c */
static int test_new_ui(void)
{
    PW_CB_DATA cb_data = {
        "password",
        "prompt"
    };
    char pass[16];
    int ok = 0;

    (void)setup_ui_method();
    if (TEST_int_gt(password_callback(pass, sizeof(pass), 0, &cb_data), 0)
            && TEST_str_eq(pass, cb_data.password))
        ok = 1;
    destroy_ui_method();
    return ok;
}

int setup_tests(void)
{
    ADD_TEST(test_old);
    ADD_TEST(test_new_ui);
    return 1;
}
