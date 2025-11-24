/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/err.h>
#include <openssl/ui.h>
#include <openssl/core_names.h>
#include "internal/cryptlib.h"
#include "internal/passphrase.h"

void ossl_pw_clear_passphrase_data(struct ossl_passphrase_data_st *data)
{
    if (data != NULL) {
        if (data->type == is_expl_passphrase)
            OPENSSL_clear_free(data->_.expl_passphrase.passphrase_copy,
                               data->_.expl_passphrase.passphrase_len);
        ossl_pw_clear_passphrase_cache(data);
        memset(data, 0, sizeof(*data));
    }
}

void ossl_pw_clear_passphrase_cache(struct ossl_passphrase_data_st *data)
{
    OPENSSL_clear_free(data->cached_passphrase, data->cached_passphrase_len);
    data->cached_passphrase = NULL;
}

int ossl_pw_set_passphrase(struct ossl_passphrase_data_st *data,
                           const unsigned char *passphrase,
                           size_t passphrase_len)
{
    if (!ossl_assert(data != NULL && passphrase != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ossl_pw_clear_passphrase_data(data);
    data->type = is_expl_passphrase;
    data->_.expl_passphrase.passphrase_copy =
        passphrase_len != 0 ? OPENSSL_memdup(passphrase, passphrase_len)
                            : OPENSSL_malloc(1);
    if (data->_.expl_passphrase.passphrase_copy == NULL)
        return 0;
    data->_.expl_passphrase.passphrase_len = passphrase_len;
    return 1;
}

int ossl_pw_set_pem_password_cb(struct ossl_passphrase_data_st *data,
                                pem_password_cb *cb, void *cbarg)
{
    if (!ossl_assert(data != NULL && cb != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ossl_pw_clear_passphrase_data(data);
    data->type = is_pem_password;
    data->_.pem_password.password_cb = cb;
    data->_.pem_password.password_cbarg = cbarg;
    return 1;
}

int ossl_pw_set_ossl_passphrase_cb(struct ossl_passphrase_data_st *data,
                                   OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg)
{
    if (!ossl_assert(data != NULL && cb != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ossl_pw_clear_passphrase_data(data);
    data->type = is_ossl_passphrase;
    data->_.ossl_passphrase.passphrase_cb = cb;
    data->_.ossl_passphrase.passphrase_cbarg = cbarg;
    return 1;
}

int ossl_pw_set_ui_method(struct ossl_passphrase_data_st *data,
                          const UI_METHOD *ui_method, void *ui_data)
{
    if (!ossl_assert(data != NULL && ui_method != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    ossl_pw_clear_passphrase_data(data);
    data->type = is_ui_method;
    data->_.ui_method.ui_method = ui_method;
    data->_.ui_method.ui_method_data = ui_data;
    return 1;
}

int ossl_pw_enable_passphrase_caching(struct ossl_passphrase_data_st *data)
{
    data->flag_cache_passphrase = 1;
    return 1;
}

int ossl_pw_disable_passphrase_caching(struct ossl_passphrase_data_st *data)
{
    data->flag_cache_passphrase = 0;
    return 1;
}


/*-
 * UI_METHOD processor.  It differs from UI_UTIL_read_pw() like this:
 *
 * 1.  It constructs a prompt on its own, based on |prompt_info|.
 * 2.  It allocates a buffer for password and verification on its own
 *     to compensate for NUL terminator in UI password strings.
 * 3.  It raises errors.
 * 4.  It reports back the length of the prompted pass phrase.
 */
static int do_ui_passphrase(char *pass, size_t pass_size, size_t *pass_len,
                            const char *prompt_info, int verify,
                            const UI_METHOD *ui_method, void *ui_data)
{
    char *prompt = NULL, *ipass = NULL, *vpass = NULL;
    int prompt_idx = -1, verify_idx = -1, res;
    UI *ui = NULL;
    int ret = 0;

    if (!ossl_assert(pass != NULL && pass_size != 0 && pass_len != NULL)) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((ui = UI_new()) == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
        return 0;
    }

    if (ui_method != NULL) {
        UI_set_method(ui, ui_method);
        if (ui_data != NULL)
            UI_add_user_data(ui, ui_data);
    }

    /* Get an application constructed prompt */
    prompt = UI_construct_prompt(ui, "pass phrase", prompt_info);
    if (prompt == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
        goto end;
    }

    /* Get a buffer for verification prompt */
    ipass = OPENSSL_zalloc(pass_size + 1);
    if (ipass == NULL)
        goto end;

    prompt_idx = UI_add_input_string(ui, prompt,
                                     UI_INPUT_FLAG_DEFAULT_PWD,
                                     ipass, 0, pass_size) - 1;
    if (prompt_idx < 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
        goto end;
    }

    if (verify) {
        /* Get a buffer for verification prompt */
        vpass = OPENSSL_zalloc(pass_size + 1);
        if (vpass == NULL)
            goto end;
        verify_idx = UI_add_verify_string(ui, prompt,
                                          UI_INPUT_FLAG_DEFAULT_PWD,
                                          vpass, 0, pass_size,
                                          ipass) - 1;
        if (verify_idx < 0) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
            goto end;
        }
    }

    switch (UI_process(ui)) {
    case -2:
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERRUPTED_OR_CANCELLED);
        break;
    case -1:
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
        break;
    default:
        res = UI_get_result_length(ui, prompt_idx);
        if (res < 0) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
            break;
        }
        *pass_len = (size_t)res;
        memcpy(pass, ipass, *pass_len);
        ret = 1;
        break;
    }

 end:
    OPENSSL_clear_free(vpass, pass_size + 1);
    OPENSSL_clear_free(ipass, pass_size + 1);
    OPENSSL_free(prompt);
    UI_free(ui);
    return ret;
}

/* Central pw prompting dispatcher */
int ossl_pw_get_passphrase(char *pass, size_t pass_size, size_t *pass_len,
                           const OSSL_PARAM params[], int verify,
                           struct ossl_passphrase_data_st *data)
{
    const char *source = NULL;
    size_t source_len = 0;
    const char *prompt_info = NULL;
    const UI_METHOD *ui_method = NULL;
    UI_METHOD *allocated_ui_method = NULL;
    void *ui_data = NULL;
    const OSSL_PARAM *p = NULL;
    int ret;

    /* Handle explicit and cached passphrases */

    if (data->type == is_expl_passphrase) {
        source = data->_.expl_passphrase.passphrase_copy;
        source_len = data->_.expl_passphrase.passphrase_len;
    } else if (data->flag_cache_passphrase && data->cached_passphrase != NULL) {
        source = data->cached_passphrase;
        source_len = data->cached_passphrase_len;
    }

    if (source != NULL) {
        if (source_len > pass_size)
            source_len = pass_size;
        memcpy(pass, source, source_len);
        *pass_len = source_len;
        return 1;
    }

    /* Handle the is_ossl_passphrase case...  that's pretty direct */

    if (data->type == is_ossl_passphrase) {
        OSSL_PASSPHRASE_CALLBACK *cb = data->_.ossl_passphrase.passphrase_cb;
        void *cbarg = data->_.ossl_passphrase.passphrase_cbarg;

        ret = cb(pass, pass_size, pass_len, params, cbarg);
        goto do_cache;
    }

    /* Handle the is_pem_password and is_ui_method cases */

    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_PASSPHRASE_PARAM_INFO)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING) {
            ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT,
                           "Prompt info data type incorrect");
            return 0;
        }
        prompt_info = p->data;
    }

    if (data->type == is_pem_password) {
        /* We use a UI wrapper for PEM */
        pem_password_cb *cb = data->_.pem_password.password_cb;

        ui_method = allocated_ui_method =
            UI_UTIL_wrap_read_pem_callback(cb, verify);
        ui_data = data->_.pem_password.password_cbarg;

        if (ui_method == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_UI_LIB);
            return 0;
        }
    } else if (data->type == is_ui_method) {
        ui_method = data->_.ui_method.ui_method;
        ui_data = data->_.ui_method.ui_method_data;
    }

    if (ui_method == NULL) {
        ERR_raise_data(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT,
                       "No password method specified");
        return 0;
    }

    ret = do_ui_passphrase(pass, pass_size, pass_len, prompt_info, verify,
                           ui_method, ui_data);

    UI_destroy_method(allocated_ui_method);

 do_cache:
    if (ret && data->flag_cache_passphrase) {
        if (data->cached_passphrase == NULL
            || *pass_len > data->cached_passphrase_len) {
            void *new_cache =
                OPENSSL_clear_realloc(data->cached_passphrase,
                                      data->cached_passphrase_len,
                                      *pass_len + 1);

            if (new_cache == NULL) {
                OPENSSL_cleanse(pass, *pass_len);
                return 0;
            }
            data->cached_passphrase = new_cache;
        }
        memcpy(data->cached_passphrase, pass, *pass_len);
        data->cached_passphrase[*pass_len] = '\0';
        data->cached_passphrase_len = *pass_len;
    }

    return ret;
}

static int ossl_pw_get_password(char *buf, int size, int rwflag,
                                void *userdata, const char *info)
{
    size_t password_len = 0;
    OSSL_PARAM params[] = {
        OSSL_PARAM_utf8_string(OSSL_PASSPHRASE_PARAM_INFO, NULL, 0),
        OSSL_PARAM_END
    };

    params[0].data = (void *)info;
    if (ossl_pw_get_passphrase(buf, (size_t)size, &password_len, params,
                               rwflag, userdata))
        return (int)password_len;
    return -1;
}

int ossl_pw_pem_password(char *buf, int size, int rwflag, void *userdata)
{
    return ossl_pw_get_password(buf, size, rwflag, userdata, "PEM");
}

int ossl_pw_pvk_password(char *buf, int size, int rwflag, void *userdata)
{
    return ossl_pw_get_password(buf, size, rwflag, userdata, "PVK");
}

int ossl_pw_passphrase_callback_enc(char *pass, size_t pass_size,
                                    size_t *pass_len,
                                    const OSSL_PARAM params[], void *arg)
{
    return ossl_pw_get_passphrase(pass, pass_size, pass_len, params, 1, arg);
}

int ossl_pw_passphrase_callback_dec(char *pass, size_t pass_size,
                                    size_t *pass_len,
                                    const OSSL_PARAM params[], void *arg)
{
    return ossl_pw_get_passphrase(pass, pass_size, pass_len, params, 0, arg);
}
