/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <windows.h>
#include <stdlib.h>
#include <string.h>
#include <malloc.h>

#if defined(CP_UTF8)

static UINT saved_cp;
static int newargc;
static char **newargv;

static void cleanup(void)
{
    int i;

    SetConsoleOutputCP(saved_cp);

    for (i = 0; i < newargc; i++)
        free(newargv[i]);

    free(newargv);
}

/*
 * Incrementally [re]allocate newargv and keep it NULL-terminated.
 */
static int validate_argv(int argc)
{
    static int size = 0;

    if (argc >= size) {
        char **ptr;

        while (argc >= size)
            size += 64;

        ptr = realloc(newargv, size * sizeof(newargv[0]));
        if (ptr == NULL)
            return 0;

        (newargv = ptr)[argc] = NULL;
    } else {
        newargv[argc] = NULL;
    }

    return 1;
}

static int process_glob(WCHAR *wstr, int wlen)
{
    int i, slash, udlen;
    WCHAR saved_char;
    WIN32_FIND_DATAW data;
    HANDLE h;

    /*
     * Note that we support wildcard characters only in filename part
     * of the path, and not in directories. Windows users are used to
     * this, that's why recursive glob processing is not implemented.
     */
    /*
     * Start by looking for last slash or backslash, ...
     */
    for (slash = 0, i = 0; i < wlen; i++)
        if (wstr[i] == L'/' || wstr[i] == L'\\')
            slash = i + 1;
    /*
     * ... then look for asterisk or question mark in the file name.
     */
    for (i = slash; i < wlen; i++)
        if (wstr[i] == L'*' || wstr[i] == L'?')
            break;

    if (i == wlen)
        return 0;   /* definitely not a glob */

    saved_char = wstr[wlen];
    wstr[wlen] = L'\0';
    h = FindFirstFileW(wstr, &data);
    wstr[wlen] = saved_char;
    if (h == INVALID_HANDLE_VALUE)
        return 0;   /* not a valid glob, just pass... */

    if (slash)
        udlen = WideCharToMultiByte(CP_UTF8, 0, wstr, slash,
                                    NULL, 0, NULL, NULL);
    else
        udlen = 0;

    do {
        int uflen;
        char *arg;

        /*
         * skip over . and ..
         */
        if (data.cFileName[0] == L'.') {
            if ((data.cFileName[1] == L'\0') ||
                (data.cFileName[1] == L'.' && data.cFileName[2] == L'\0'))
                continue;
        }

        if (!validate_argv(newargc + 1))
            break;

        /*
         * -1 below means "scan for trailing '\0' *and* count it",
         * so that |uflen| covers even trailing '\0'.
         */
        uflen = WideCharToMultiByte(CP_UTF8, 0, data.cFileName, -1,
                                    NULL, 0, NULL, NULL);

        arg = malloc(udlen + uflen);
        if (arg == NULL)
            break;

        if (udlen)
            WideCharToMultiByte(CP_UTF8, 0, wstr, slash,
                                arg, udlen, NULL, NULL);

        WideCharToMultiByte(CP_UTF8, 0, data.cFileName, -1,
                            arg + udlen, uflen, NULL, NULL);

        newargv[newargc++] = arg;
    } while (FindNextFileW(h, &data));

    CloseHandle(h);

    return 1;
}

void win32_utf8argv(int *argc, char **argv[])
{
    const WCHAR *wcmdline;
    WCHAR *warg, *wend, *p;
    int wlen, ulen, valid = 1;
    char *arg;

    if (GetEnvironmentVariableW(L"OPENSSL_WIN32_UTF8", NULL, 0) == 0)
        return;

    newargc = 0;
    newargv = NULL;
    if (!validate_argv(newargc))
        return;

    wcmdline = GetCommandLineW();
    if (wcmdline == NULL) return;

    /*
     * make a copy of the command line, since we might have to modify it...
     */
    wlen = wcslen(wcmdline);
    p = _alloca((wlen + 1) * sizeof(WCHAR));
    wcscpy(p, wcmdline);

    while (*p != L'\0') {
        int in_quote = 0;

        if (*p == L' ' || *p == L'\t') {
            p++; /* skip over whitespace */
            continue;
        }

        /*
         * Note: because we may need to fiddle with the number of backslashes,
         * the argument string is copied into itself.  This is safe because
         * the number of characters will never expand.
         */
        warg = wend = p;
        while (*p != L'\0'
               && (in_quote || (*p != L' ' && *p != L'\t'))) {
            switch (*p) {
            case L'\\':
                /*
                 * Microsoft documentation on how backslashes are treated
                 * is:
                 *
                 * + Backslashes are interpreted literally, unless they
                 *   immediately precede a double quotation mark.
                 * + If an even number of backslashes is followed by a double
                 *   quotation mark, one backslash is placed in the argv array
                 *   for every pair of backslashes, and the double quotation
                 *   mark is interpreted as a string delimiter.
                 * + If an odd number of backslashes is followed by a double
                 *   quotation mark, one backslash is placed in the argv array
                 *   for every pair of backslashes, and the double quotation
                 *   mark is "escaped" by the remaining backslash, causing a
                 *   literal double quotation mark (") to be placed in argv.
                 *
                 * Ref: https://msdn.microsoft.com/en-us/library/17w5ykft.aspx
                 *
                 * Though referred page doesn't mention it, multiple qouble
                 * quotes are also special. Pair of double quotes in quoted
                 * string is counted as single double quote.
                 */
                {
                    const WCHAR *q = p;
                    int i;

                    while (*p == L'\\')
                        p++;

                    if (*p == L'"') {
                        int i;

                        for (i = (p - q) / 2; i > 0; i--)
                            *wend++ = L'\\';

                        /*
                         * if odd amount of backslashes before the quote,
                         * said quote is part of the argument, not a delimiter
                         */
                        if ((p - q) % 2 == 1)
                            *wend++ = *p++;
                    } else {
                        for (i = p - q; i > 0; i--)
                            *wend++ = L'\\';
                    }
                }
                break;
            case L'"':
                /*
                 * Without the preceding backslash (or when preceded with an
                 * even number of backslashes), the double quote is a simple
                 * string delimiter and just slightly change the parsing state
                 */
                if (in_quote && p[1] == L'"')
                    *wend++ = *p++;
                else
                    in_quote = !in_quote;
                p++;
                break;
            default:
                /*
                 * Any other non-delimiter character is just taken verbatim
                 */
                *wend++ = *p++;
            }
        }

        wlen = wend - warg;

        if (wlen == 0 || !process_glob(warg, wlen)) {
            if (!validate_argv(newargc + 1)) {
                valid = 0;
                break;
            }

            ulen = 0;
            if (wlen > 0) {
                ulen = WideCharToMultiByte(CP_UTF8, 0, warg, wlen,
                                           NULL, 0, NULL, NULL);
                if (ulen <= 0)
                    continue;
            }

            arg = malloc(ulen + 1);
            if (arg == NULL) {
                valid = 0;
                break;
            }

            if (wlen > 0)
                WideCharToMultiByte(CP_UTF8, 0, warg, wlen,
                                    arg, ulen, NULL, NULL);
            arg[ulen] = '\0';

            newargv[newargc++] = arg;
        }
    }

    if (valid) {
        saved_cp = GetConsoleOutputCP();
        SetConsoleOutputCP(CP_UTF8);

        *argc = newargc;
        *argv = newargv;

        atexit(cleanup);
    } else if (newargv != NULL) {
        int i;

        for (i = 0; i < newargc; i++)
            free(newargv[i]);

        free(newargv);

        newargc = 0;
        newargv = NULL;
    }

    return;
}
#else
void win32_utf8argv(int *argc, char **argv[])
{   return;   }
#endif
