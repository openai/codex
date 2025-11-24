/*
 * Copyright 1995-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/err.h>
#include <openssl/ui.h>
#include "apps_ui.h"

static UI_METHOD *ui_method = NULL;
static const UI_METHOD *ui_base_method = NULL;

static int ui_open(UI *ui)
{
    int (*opener)(UI *ui) = UI_method_get_opener(ui_base_method);

    if (opener != NULL)
        return opener(ui);
    return 1;
}

static int ui_read(UI *ui, UI_STRING *uis)
{
    int (*reader)(UI *ui, UI_STRING *uis) = NULL;

    if (UI_get_input_flags(uis) & UI_INPUT_FLAG_DEFAULT_PWD
        && UI_get0_user_data(ui)) {
        switch (UI_get_string_type(uis)) {
        case UIT_PROMPT:
        case UIT_VERIFY:
            {
                const char *password =
                    ((PW_CB_DATA *)UI_get0_user_data(ui))->password;

                if (password != NULL) {
                    UI_set_result(ui, uis, password);
                    return 1;
                }
            }
            break;
        case UIT_NONE:
        case UIT_BOOLEAN:
        case UIT_INFO:
        case UIT_ERROR:
            break;
        }
    }

    reader = UI_method_get_reader(ui_base_method);
    if (reader != NULL)
        return reader(ui, uis);
    /* Default to the empty password if we've got nothing better */
    UI_set_result(ui, uis, "");
    return 1;
}

static int ui_write(UI *ui, UI_STRING *uis)
{
    int (*writer)(UI *ui, UI_STRING *uis) = NULL;

    if (UI_get_input_flags(uis) & UI_INPUT_FLAG_DEFAULT_PWD
        && UI_get0_user_data(ui)) {
        switch (UI_get_string_type(uis)) {
        case UIT_PROMPT:
        case UIT_VERIFY:
            {
                const char *password =
                    ((PW_CB_DATA *)UI_get0_user_data(ui))->password;

                if (password != NULL)
                    return 1;
            }
            break;
        case UIT_NONE:
        case UIT_BOOLEAN:
        case UIT_INFO:
        case UIT_ERROR:
            break;
        }
    }

    writer = UI_method_get_writer(ui_base_method);
    if (writer != NULL)
        return writer(ui, uis);
    return 1;
}

static int ui_close(UI *ui)
{
    int (*closer)(UI *ui) = UI_method_get_closer(ui_base_method);

    if (closer != NULL)
        return closer(ui);
    return 1;
}

/* object_name defaults to prompt_info from ui user data if present */
static char *ui_prompt_construct(UI *ui, const char *phrase_desc,
                                 const char *object_name)
{
    PW_CB_DATA *cb_data = (PW_CB_DATA *)UI_get0_user_data(ui);

    if (phrase_desc == NULL)
        phrase_desc = "pass phrase";
    if (object_name == NULL && cb_data != NULL)
        object_name = cb_data->prompt_info;
    return UI_construct_prompt(NULL, phrase_desc, object_name);
}

int set_base_ui_method(const UI_METHOD *ui_meth)
{
    if (ui_meth == NULL)
        ui_meth = UI_null();
    ui_base_method = ui_meth;
    return 1;
}

int setup_ui_method(void)
{
    ui_base_method = UI_null();
#ifndef OPENSSL_NO_UI_CONSOLE
    ui_base_method = UI_OpenSSL();
#endif
    ui_method = UI_create_method("OpenSSL application user interface");
    return ui_method != NULL
        && 0 == UI_method_set_opener(ui_method, ui_open)
        && 0 == UI_method_set_reader(ui_method, ui_read)
        && 0 == UI_method_set_writer(ui_method, ui_write)
        && 0 == UI_method_set_closer(ui_method, ui_close)
        && 0 == UI_method_set_prompt_constructor(ui_method,
                                                 ui_prompt_construct);
}

void destroy_ui_method(void)
{
    if (ui_method != NULL) {
        UI_destroy_method(ui_method);
        ui_method = NULL;
    }
}

const UI_METHOD *get_ui_method(void)
{
    return ui_method;
}

static void *ui_malloc(int sz, const char *what)
{
    void *vp = OPENSSL_malloc(sz);

    if (vp == NULL) {
        BIO_printf(bio_err, "Could not allocate %d bytes for %s\n", sz, what);
        ERR_print_errors(bio_err);
        exit(1);
    }
    return vp;
}

int password_callback(char *buf, int bufsiz, int verify, PW_CB_DATA *cb_data)
{
    int res = 0;
    UI *ui;
    int ok = 0;
    char *buff = NULL;
    int ui_flags = 0;
    const char *prompt_info = NULL;
    char *prompt;

    if ((ui = UI_new_method(ui_method)) == NULL)
        return 0;

    if (cb_data != NULL && cb_data->prompt_info != NULL)
        prompt_info = cb_data->prompt_info;
    prompt = UI_construct_prompt(ui, "pass phrase", prompt_info);
    if (prompt == NULL) {
        BIO_printf(bio_err, "Out of memory\n");
        UI_free(ui);
        return 0;
    }

    ui_flags |= UI_INPUT_FLAG_DEFAULT_PWD;
    UI_ctrl(ui, UI_CTRL_PRINT_ERRORS, 1, 0, 0);

    /* We know that there is no previous user data to return to us */
    (void)UI_add_user_data(ui, cb_data);

    ok = UI_add_input_string(ui, prompt, ui_flags, buf,
                             PW_MIN_LENGTH, bufsiz - 1);

    if (ok >= 0 && verify) {
        buff = ui_malloc(bufsiz, "password buffer");
        ok = UI_add_verify_string(ui, prompt, ui_flags, buff,
                                  PW_MIN_LENGTH, bufsiz - 1, buf);
    }
    if (ok >= 0)
        do {
            ok = UI_process(ui);
        } while (ok < 0 && UI_ctrl(ui, UI_CTRL_IS_REDOABLE, 0, 0, 0));

    OPENSSL_clear_free(buff, (unsigned int)bufsiz);

    if (ok >= 0)
        res = strlen(buf);
    if (ok == -1) {
        BIO_printf(bio_err, "User interface error\n");
        ERR_print_errors(bio_err);
        OPENSSL_cleanse(buf, (unsigned int)bufsiz);
        res = 0;
    }
    if (ok == -2) {
        BIO_printf(bio_err, "aborted!\n");
        OPENSSL_cleanse(buf, (unsigned int)bufsiz);
        res = 0;
    }
    UI_free(ui);
    OPENSSL_free(prompt);
    return res;
}
