/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_APPS_LOG_H
# define OSSL_APPS_LOG_H

# include <openssl/bio.h>
# if !defined(OPENSSL_SYS_VMS) && !defined(OPENSSL_SYS_WINDOWS) \
    && !defined(OPENSSL_NO_SOCK) && !defined(OPENSSL_NO_POSIX_IO)
#  include <syslog.h>
# else
#  define LOG_EMERG   0
#  define LOG_ALERT   1
#  define LOG_CRIT    2
#  define LOG_ERR     3
#  define LOG_WARNING 4
#  define LOG_NOTICE  5
#  define LOG_INFO    6
#  define LOG_DEBUG   7
# endif

# undef LOG_TRACE
# define LOG_TRACE (LOG_DEBUG + 1)

int log_set_verbosity(const char *prog, int level);
int log_get_verbosity(void);

/*-
 * Output a message using the trace API with the given category
 * if the category is >= 0 and tracing is enabled.
 * Log the message to syslog if multi-threaded HTTP_DAEMON, else to bio_err
 * if the verbosity is sufficient for the given level of severity.
 * Yet cannot do both types of output in strict ANSI mode.
 * category: trace category as defined in trace.h, or -1
 * prog: the name of the current app, or NULL
 * level: the severity of the message, e.g., LOG_ERR
 * fmt: message format, which should not include a trailing newline
 * ...: potential extra parameters like with printf()
 * returns nothing
 */
void trace_log_message(int category,
                       const char *prog, int level, const char *fmt, ...);

#endif /* OSSL_APPS_LOG_H */
