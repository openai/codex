#include <stdio.h>
#include <stdarg.h>

typedef void (wl_log_func_t)(const char *, va_list);

void wl_log_trampoline_to_rust_client(char const *fmt, va_list list);

void wl_log_rust_logger_client(char const *msg);

void wl_log_trampoline_to_rust_client(char const *fmt, va_list list) {
    char buffer[256];
    int ret = vsnprintf(buffer, 256, fmt, list);
    if (ret <= 0) {
        // forward the unformatted message, in a best-effort attempt
        wl_log_rust_logger_client(fmt);
    } else {
        wl_log_rust_logger_client(buffer);
    }
}