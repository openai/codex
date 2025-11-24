/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-macros.h  generic macros
 *
 * Copyright (C) 2002  Red Hat Inc.
 *
 * Licensed under the Academic Free License version 2.1
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 *
 */
#if !defined (DBUS_INSIDE_DBUS_H) && !defined (DBUS_COMPILATION)
#error "Only <dbus/dbus.h> can be included directly, this file may disappear or change contents."
#endif

#ifndef DBUS_MACROS_H
#define DBUS_MACROS_H

#ifdef  __cplusplus
#  define DBUS_BEGIN_DECLS  extern "C" {
#  define DBUS_END_DECLS    }
#else
#  define DBUS_BEGIN_DECLS
#  define DBUS_END_DECLS
#endif

#ifndef TRUE
#  define TRUE 1
#endif
#ifndef FALSE
#  define FALSE 0
#endif

#ifndef NULL
#  ifdef __cplusplus
#    define NULL        (0L)
#  else /* !__cplusplus */
#    define NULL        ((void*) 0)
#  endif /* !__cplusplus */
#endif

#if  __GNUC__ > 3 || (__GNUC__ == 3 && __GNUC_MINOR__ >= 1)
#  define DBUS_DEPRECATED __attribute__ ((__deprecated__))
#elif defined(_MSC_VER) && (_MSC_VER >= 1300)
#  define DBUS_DEPRECATED __declspec(deprecated)
#else
#  define DBUS_DEPRECATED
#endif

#if __GNUC__ > 2 || (__GNUC__ == 2 && __GNUC_MINOR__ >= 8)
#  define _DBUS_GNUC_EXTENSION __extension__
#else
#  define _DBUS_GNUC_EXTENSION
#endif

#if     (__GNUC__ > 2 || (__GNUC__ == 2 && __GNUC_MINOR__ > 4)) || \
         defined(__clang__)
#define _DBUS_GNUC_PRINTF( format_idx, arg_idx )    \
  __attribute__((__format__ (__printf__, format_idx, arg_idx)))
#define _DBUS_GNUC_NORETURN                         \
  __attribute__((__noreturn__))
#define _DBUS_GNUC_UNUSED                           \
  __attribute__((__unused__))
#else   /* !__GNUC__ */
#define _DBUS_GNUC_PRINTF( format_idx, arg_idx )
#define _DBUS_GNUC_NORETURN
#define _DBUS_GNUC_UNUSED
#endif  /* !__GNUC__ */

#if    __GNUC__ > 2 || (__GNUC__ == 2 && __GNUC_MINOR__ >= 96)
#define DBUS_MALLOC     __attribute__((__malloc__))
#else
#define DBUS_MALLOC
#endif

#if     (__GNUC__ > 4) || (__GNUC__ == 4 && __GNUC_MINOR__ >= 3)
#define DBUS_ALLOC_SIZE(x) __attribute__((__alloc_size__(x)))
#define DBUS_ALLOC_SIZE2(x,y) __attribute__((__alloc_size__(x,y)))
#else
#define DBUS_ALLOC_SIZE(x)
#define DBUS_ALLOC_SIZE2(x,y)
#endif

/** @def _DBUS_WARN_UNUSED_RESULT
 *
 * An attribute for functions whose result must be checked by the caller.
 *
 * This macro is used in function declarations. Unlike gcc-specific
 * attributes, to avoid compilation failure with MSVC it must appear
 * somewhere before the function name in the declaration. Our preferred
 * coding style is to place it before the return type, for example:
 *
 * DBUS_PRIVATE_EXPORT _DBUS_WARN_UNUSED_RESULT
 * dbus_bool_t _dbus_user_database_lock_system (void);
 */
#if     defined(_MSC_VER) && (_MSC_VER >= 1700)
#define _DBUS_WARN_UNUSED_RESULT _Must_inspect_result_
#elif    (__GNUC__ > 3) || (__GNUC__ == 3 && __GNUC_MINOR__ >= 4)
#define _DBUS_WARN_UNUSED_RESULT __attribute__((warn_unused_result))
#else
#define _DBUS_WARN_UNUSED_RESULT
#endif

/** @def _DBUS_GNUC_PRINTF
 * used to tell gcc about printf format strings
 */
/** @def _DBUS_GNUC_NORETURN
 * used to tell gcc about functions that never return, such as _dbus_abort()
 */

/* Normally docs are in .c files, but there isn't a .c file for this. */
/**
 * @defgroup DBusMacros Utility macros
 * @ingroup  DBus
 * @brief #TRUE, #FALSE, #NULL, and so on
 *
 * Utility macros.
 *
 * @{
 */

/**
 * @def DBUS_BEGIN_DECLS
 *
 * Macro used prior to declaring functions in the D-Bus header
 * files. Expands to "extern "C"" when using a C++ compiler,
 * and expands to nothing when using a C compiler.
 *
 * Please don't use this in your own code, consider it
 * D-Bus internal.
 */
/**
 * @def DBUS_END_DECLS
 *
 * Macro used after declaring functions in the D-Bus header
 * files. Expands to "}" when using a C++ compiler,
 * and expands to nothing when using a C compiler.
 *
 * Please don't use this in your own code, consider it
 * D-Bus internal.
 */
/**
 * @def TRUE
 *
 * Expands to "1"
 */
/**
 * @def FALSE
 *
 * Expands to "0"
 */
/**
 * @def NULL
 *
 * A null pointer, defined appropriately for C or C++.
 */
/**
 * @def DBUS_DEPRECATED
 *
 * Tells the compiler to warn about a function or type if it's used.
 * Code marked in this way should also be enclosed in
 * @code
 * #ifndef DBUS_DISABLE_DEPRECATED
 *  deprecated stuff here
 * #endif
 * @endcode
 *
 * Please don't use this in your own code, consider it
 * D-Bus internal.
 */
/**
 * @def _DBUS_GNUC_EXTENSION
 *
 * Tells gcc not to warn about extensions to the C standard in the
 * following expression, even if compiling with -pedantic. Do not use
 * this macro in your own code; please consider it to be internal to libdbus.
 */

/*
 * @def DBUS_EXPORT
 *
 * Declare the following symbol as public.  This is currently a noop on
 * platforms other than Windows.
 */

#if defined(DBUS_EXPORT)
  /* value forced by compiler command line, don't redefine */
#elif defined(_WIN32)
#  if defined(DBUS_STATIC_BUILD)
#  define DBUS_EXPORT
#  elif defined(dbus_1_EXPORTS)
#  define DBUS_EXPORT __declspec(dllexport)
#  else
#  define DBUS_EXPORT __declspec(dllimport)
#  endif
#elif defined(__GNUC__) && __GNUC__ >= 4
#  define DBUS_EXPORT __attribute__ ((__visibility__ ("default")))
#else
#define DBUS_EXPORT
#endif

#if defined(DBUS_PRIVATE_EXPORT)
  /* value forced by compiler command line, don't redefine */
#elif defined(_WIN32)
#  if defined(DBUS_STATIC_BUILD)
#    define DBUS_PRIVATE_EXPORT /* no decoration */
#  elif defined(dbus_1_EXPORTS)
#    define DBUS_PRIVATE_EXPORT __declspec(dllexport)
#  else
#    define DBUS_PRIVATE_EXPORT __declspec(dllimport)
#  endif
#elif defined(__GNUC__) && __GNUC__ >= 4
#  define DBUS_PRIVATE_EXPORT __attribute__ ((__visibility__ ("default")))
#else
#  define DBUS_PRIVATE_EXPORT /* no decoration */
#endif

/* Implementation for dbus_clear_message() etc. This is not API,
 * do not use it directly.
 *
 * We're using a specific type (T ** and T *) instead of void ** and
 * void * partly for type-safety, partly to be strict-aliasing-compliant,
 * and partly to keep C++ compilers happy. This code is inlined into
 * users of libdbus, so we can't rely on it having dbus' own compiler
 * settings. */
#define _dbus_clear_pointer_impl(T, pointer_to_pointer, destroy) \
  do { \
    T **_pp = (pointer_to_pointer); \
    T *_value = *_pp; \
    \
    *_pp = NULL; \
    \
    if (_value != NULL) \
      destroy (_value); \
  } while (0)
/* Not (destroy) (_value) in case destroy() is a function-like macro */

/** @} */

#endif /* DBUS_MACROS_H */
