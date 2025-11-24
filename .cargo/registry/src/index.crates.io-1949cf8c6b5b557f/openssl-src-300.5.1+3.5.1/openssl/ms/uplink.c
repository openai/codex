/*
 * Copyright 2004-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if (defined(_WIN64) || defined(_WIN32_WCE)) && !defined(UNICODE)
# define UNICODE
#endif
#if defined(UNICODE) && !defined(_UNICODE)
# define _UNICODE
#endif
#if defined(_UNICODE) && !defined(UNICODE)
# define UNICODE
#endif

#include <windows.h>
#include <tchar.h>
#include <stdio.h>
#include "uplink.h"
void OPENSSL_showfatal(const char *, ...);

static TCHAR msg[128];

static void unimplemented(void)
{
    OPENSSL_showfatal(sizeof(TCHAR) == sizeof(char) ? "%s\n" : "%S\n", msg);
    TerminateProcess(GetCurrentProcess(), 1);
}

void OPENSSL_Uplink(volatile void **table, int index)
{
    static HMODULE volatile apphandle = NULL;
    static void **volatile applinktable = NULL;
    int len;
    void (*func) (void) = unimplemented;
    HANDLE h;
    void **p;

    /*
     * Note that the below code is not MT-safe in respect to msg buffer, but
     * what's the worst thing that can happen? Error message might be
     * misleading or corrupted. As error condition is fatal and should never
     * be risen, I accept the risk...
     */
    /*
     * One can argue that I should have used InterlockedExchangePointer or
     * something to update static variables and table[]. Well, store
     * instructions are as atomic as they can get and assigned values are
     * effectively constant... So that volatile qualifier should be
     * sufficient [it prohibits compiler to reorder memory access
     * instructions].
     */
    do {
        len = _sntprintf(msg, sizeof(msg) / sizeof(TCHAR),
                         _T("OPENSSL_Uplink(%p,%02X): "), table, index);
        _tcscpy(msg + len, _T("unimplemented function"));

        if ((h = apphandle) == NULL) {
            if ((h = GetModuleHandle(NULL)) == NULL) {
                apphandle = (HMODULE) - 1;
                _tcscpy(msg + len, _T("no host application"));
                break;
            }
            apphandle = h;
        }
        if ((h = apphandle) == (HMODULE) - 1) /* revalidate */
            break;

        if (applinktable == NULL) {
            void **(*applink) ();

            applink = (void **(*)())GetProcAddress(h, "OPENSSL_Applink");
            if (applink == NULL) {
                apphandle = (HMODULE) - 1;
                _tcscpy(msg + len, _T("no OPENSSL_Applink"));
                break;
            }
            p = (*applink) ();
            if (p == NULL) {
                apphandle = (HMODULE) - 1;
                _tcscpy(msg + len, _T("no ApplinkTable"));
                break;
            }
            applinktable = p;
        } else
            p = applinktable;

        if (index > (int)p[0])
            break;

        if (p[index])
            func = p[index];
    } while (0);

    table[index] = func;
}

#if (defined(_MSC_VER) || defined(__BORLANDC__)) && defined(_M_IX86)
# if defined(_MSC_VER)
#  define LAZY(i)         \
__declspec(naked) static void lazy##i (void) {  \
        _asm    push i                          \
        _asm    push OFFSET OPENSSL_UplinkTable \
        _asm    call OPENSSL_Uplink             \
        _asm    add  esp,8                      \
        _asm    jmp  OPENSSL_UplinkTable+4*i    }
# elif defined(__BORLANDC__) && defined(__clang__)
void *OPENSSL_UplinkTable[26]; /* C++Builder requires declaration before use */
#  define LAZY(i)         \
__declspec(naked) static void lazy##i (void) { \
    __asm__("pushl $" #i "; "                  \
            "pushl %0; "                       \
            "call  %P1; "                      \
            "addl  $8, %%esp; "                \
            "jmp   *%2 "                       \
            : /* no outputs */                 \
            : "i" (OPENSSL_UplinkTable),       \
              "i" (OPENSSL_Uplink),            \
              "m" (OPENSSL_UplinkTable[i]));   }
# endif

# if APPLINK_MAX>25
#  error "Add more stubs..."
# endif
/* make some in advance... */
LAZY(1) LAZY(2) LAZY(3) LAZY(4) LAZY(5)
    LAZY(6) LAZY(7) LAZY(8) LAZY(9) LAZY(10)
    LAZY(11) LAZY(12) LAZY(13) LAZY(14) LAZY(15)
    LAZY(16) LAZY(17) LAZY(18) LAZY(19) LAZY(20)
    LAZY(21) LAZY(22) LAZY(23) LAZY(24) LAZY(25)
void *OPENSSL_UplinkTable[] = {
    (void *)APPLINK_MAX,
    lazy1, lazy2, lazy3, lazy4, lazy5,
    lazy6, lazy7, lazy8, lazy9, lazy10,
    lazy11, lazy12, lazy13, lazy14, lazy15,
    lazy16, lazy17, lazy18, lazy19, lazy20,
    lazy21, lazy22, lazy23, lazy24, lazy25,
};
#endif

#ifdef SELFTEST
main()
{
    UP_fprintf(UP_stdout, "hello, world!\n");
}
#endif
