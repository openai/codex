/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_RCU_INTERNAL_H
# define OPENSSL_RCU_INTERNAL_H
# pragma once

struct rcu_qp;

struct rcu_cb_item {
    rcu_cb_fn fn;
    void *data;
    struct rcu_cb_item *next;
};

#endif
