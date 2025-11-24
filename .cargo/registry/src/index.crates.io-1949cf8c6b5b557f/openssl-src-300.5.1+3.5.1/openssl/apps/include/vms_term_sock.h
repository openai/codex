/*
 * Copyright 2016 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2016 VMS Software, Inc. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_APPS_VMS_TERM_SOCK_H
# define OSSL_APPS_VMS_TERM_SOCK_H

/*
** Terminal Socket Function Codes
*/
# define TERM_SOCK_CREATE       1
# define TERM_SOCK_DELETE       2

/*
** Terminal Socket Status Codes
*/
# define TERM_SOCK_FAILURE      0
# define TERM_SOCK_SUCCESS      1

/*
** Terminal Socket Prototype
*/
int TerminalSocket (int FunctionCode, int *ReturnSocket);

#endif
