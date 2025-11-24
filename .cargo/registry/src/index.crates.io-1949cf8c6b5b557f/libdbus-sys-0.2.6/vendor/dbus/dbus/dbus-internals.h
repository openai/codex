/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-internals.h  random utility stuff (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
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
#ifdef DBUS_INSIDE_DBUS_H
#error "You can't include dbus-internals.h in the public header dbus.h"
#endif

#ifndef DBUS_INTERNALS_H
#define DBUS_INTERNALS_H

#include <dbus/dbus-memory.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-sysdeps.h>
#include <dbus/dbus-threads-internal.h>

DBUS_BEGIN_DECLS

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#define DBUS_EMBEDDED_TESTS_EXPORT DBUS_PRIVATE_EXPORT
#else
#define DBUS_EMBEDDED_TESTS_EXPORT /* nothing */
#endif

DBUS_PRIVATE_EXPORT
void _dbus_warn               (const char *format,
                               ...) _DBUS_GNUC_PRINTF (1, 2);

DBUS_PRIVATE_EXPORT
void _dbus_warn_check_failed  (const char *format,
                               ...) _DBUS_GNUC_PRINTF (1, 2);
DBUS_PRIVATE_EXPORT
void _dbus_warn_return_if_fail (const char *function,
                                const char *assertion,
                                const char *file,
                                int line);

#if defined (__STDC_VERSION__) && (__STDC_VERSION__ >= 199901L)
#define _DBUS_FUNCTION_NAME __func__
#elif defined(__GNUC__) || defined(_MSC_VER)
#define _DBUS_FUNCTION_NAME __FUNCTION__
#else
#define _DBUS_FUNCTION_NAME "unknown function"
#endif

/*
 * (code from GLib)
 * 
 * The _DBUS_LIKELY and _DBUS_UNLIKELY macros let the programmer give hints to 
 * the compiler about the expected result of an expression. Some compilers
 * can use this information for optimizations.
 *
 * The _DBUS_BOOLEAN_EXPR macro is intended to trigger a gcc warning when
 * putting assignments in the macro arg
 */
#if defined(__GNUC__) && (__GNUC__ > 2) && defined(__OPTIMIZE__)
#define _DBUS_BOOLEAN_EXPR(expr)                \
 __extension__ ({                               \
   int _dbus_boolean_var_;                      \
   if (expr)                                    \
      _dbus_boolean_var_ = 1;                   \
   else                                         \
      _dbus_boolean_var_ = 0;                   \
   _dbus_boolean_var_;                          \
})
#define _DBUS_LIKELY(expr) (__builtin_expect (_DBUS_BOOLEAN_EXPR(expr), 1))
#define _DBUS_UNLIKELY(expr) (__builtin_expect (_DBUS_BOOLEAN_EXPR(expr), 0))
#else
#define _DBUS_LIKELY(expr) (expr)
#define _DBUS_UNLIKELY(expr) (expr)
#endif

#ifdef DBUS_ENABLE_VERBOSE_MODE

/*
 at least gnu cc and msvc compiler are known to 
 have support for variable macro argument lists
 add other compilers is required
*/
#if defined(__GNUC__) || defined(_MSC_VER) 
#define DBUS_CPP_SUPPORTS_VARIABLE_MACRO_ARGUMENTS
#endif

#ifdef DBUS_CPP_SUPPORTS_VARIABLE_MACRO_ARGUMENTS
DBUS_PRIVATE_EXPORT
void _dbus_verbose_real       (const char *file, const int line, const char *function, 
                               const char *format,...) _DBUS_GNUC_PRINTF (4, 5);
#  define _dbus_verbose(fmt,...) _dbus_verbose_real( __FILE__,__LINE__,__FUNCTION__,fmt, ## __VA_ARGS__)
#else
DBUS_PRIVATE_EXPORT
void _dbus_verbose_real       (const char *format,
                               ...) _DBUS_GNUC_PRINTF (1, 2);
#  define _dbus_verbose _dbus_verbose_real
#endif
DBUS_PRIVATE_EXPORT
void _dbus_verbose_reset_real (void);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_is_verbose_real (void);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_get_verbose (void);
DBUS_PRIVATE_EXPORT
void _dbus_set_verbose (dbus_bool_t state);
void _dbus_verbose_raw (const char *s);

#  define _dbus_verbose_reset _dbus_verbose_reset_real
#  define _dbus_is_verbose _dbus_is_verbose_real
#else
#  define _dbus_verbose(...) do { } while (0)
#  define _dbus_verbose_reset() do { } while (0)
#  define _dbus_is_verbose() FALSE 
#endif /* !DBUS_ENABLE_VERBOSE_MODE */

DBUS_PRIVATE_EXPORT
void _dbus_trace_ref (const char *obj_name,
                      void       *obj,
                      int         old_refcount,
                      int         new_refcount,
                      const char *why,
                      const char *env_var,
                      int        *enabled);

DBUS_PRIVATE_EXPORT
const char* _dbus_strerror (int error_number);

#ifdef DBUS_DISABLE_ASSERT
#define _dbus_assert(condition) do { } while (0)
#else
DBUS_PRIVATE_EXPORT
void _dbus_real_assert (dbus_bool_t  condition,
                        const char  *condition_text,
                        const char  *file,
                        int          line,
                        const char  *func);
#define _dbus_assert(condition)                                         \
  _dbus_real_assert ((condition) != 0, #condition, __FILE__, __LINE__, _DBUS_FUNCTION_NAME)
#endif /* !DBUS_DISABLE_ASSERT */

#ifdef DBUS_DISABLE_ASSERT
#define _dbus_assert_not_reached(explanation) do { } while (0)
#else
DBUS_PRIVATE_EXPORT
void _dbus_real_assert_not_reached (const char *explanation,
                                    const char *file,
                                    int         line) _DBUS_GNUC_NORETURN;
#define _dbus_assert_not_reached(explanation)                                   \
  _dbus_real_assert_not_reached (explanation, __FILE__, __LINE__)
#endif /* !DBUS_DISABLE_ASSERT */

#ifdef DBUS_DISABLE_CHECKS
#define _dbus_return_if_fail(condition)
#define _dbus_return_val_if_fail(condition, val)
#else

#define _dbus_return_if_fail(condition) do {                                       \
   _dbus_assert ((*(const char*)_DBUS_FUNCTION_NAME) != '_');                      \
  if (!(condition)) {                                                              \
    _dbus_warn_return_if_fail (_DBUS_FUNCTION_NAME, #condition, __FILE__, __LINE__); \
    return;                                                                        \
  } } while (0)

#define _dbus_return_val_if_fail(condition, val) do {                                   \
   _dbus_assert ((*(const char*)_DBUS_FUNCTION_NAME) != '_');                           \
  if (!(condition)) {                                                                   \
    _dbus_warn_return_if_fail (_DBUS_FUNCTION_NAME, #condition, __FILE__, __LINE__); \
    return (val);                                                                       \
  } } while (0)

#endif /* !DBUS_DISABLE_ASSERT */

#define _DBUS_N_ELEMENTS(array) ((int) (sizeof ((array)) / sizeof ((array)[0])))

#define _DBUS_POINTER_TO_INT(pointer) ((intptr_t)(pointer))
#define _DBUS_INT_TO_POINTER(integer) ((void*)((intptr_t)(integer)))

#define _DBUS_ZERO(object) (memset (&(object), '\0', sizeof ((object))))

#ifdef offsetof
#define _DBUS_STRUCT_OFFSET(struct_type, member) \
    (offsetof (struct_type, member))
#else
#define _DBUS_STRUCT_OFFSET(struct_type, member)	\
    ((intptr_t) ((unsigned char*) &((struct_type*) 0)->member))
#endif

#define _DBUS_ALIGNOF(type) \
    (_DBUS_STRUCT_OFFSET (struct { char _1; type _2; }, _2))

#if defined(DBUS_DISABLE_CHECKS) || defined(DBUS_DISABLE_ASSERT)
/* this is an assert and not an error, but in the typical --disable-checks case (you're trying
 * to really minimize code size), disabling these assertions makes sense.
 */
#define _DBUS_ASSERT_ERROR_IS_SET(error) do { } while (0)
#define _DBUS_ASSERT_ERROR_IS_CLEAR(error) do { } while (0)
#define _DBUS_ASSERT_ERROR_XOR_BOOL(error, retval) do { } while (0)
#else
static inline void
_dbus_assert_error_is_set (const DBusError *error,
                           const char      *file,
                           int              line,
                           const char      *func)
{
  _dbus_real_assert (error == NULL || dbus_error_is_set (error),
                     "error is set", file, line, func);
}

static inline void
_dbus_assert_error_is_clear (const DBusError *error,
                           const char      *file,
                           int              line,
                           const char      *func)
{
  _dbus_real_assert (error == NULL || !dbus_error_is_set (error),
                     "error is clear", file, line, func);
}

static inline void
_dbus_assert_error_xor_bool (const DBusError *error,
                             dbus_bool_t      retval,
                             const char      *file,
                             int              line,
                             const char      *func)
{
  _dbus_real_assert (error == NULL || dbus_error_is_set (error) == !retval,
                     "error is consistent with boolean result", file, line, func);
}

/**
 * Assert that error is set, unless it is NULL in which case we cannot
 * tell whether it would have been set.
 */
#define _DBUS_ASSERT_ERROR_IS_SET(error) _dbus_assert_error_is_set (error, __FILE__, __LINE__, _DBUS_FUNCTION_NAME)

/**
 * Assert that error is not set, unless it is NULL in which case we cannot
 * tell whether it would have been set.
 */
#define _DBUS_ASSERT_ERROR_IS_CLEAR(error) _dbus_assert_error_is_clear (error, __FILE__, __LINE__, _DBUS_FUNCTION_NAME)

/**
 * Assert that error is consistent with retval: if error is not NULL,
 * it must be set if and only if retval is false.
 *
 * retval can be a boolean expression like "result != NULL".
 */
#define _DBUS_ASSERT_ERROR_XOR_BOOL(error, retval) _dbus_assert_error_xor_bool (error, retval, __FILE__, __LINE__, _DBUS_FUNCTION_NAME)
#endif

#define _dbus_return_if_error_is_set(error) _dbus_return_if_fail ((error) == NULL || !dbus_error_is_set ((error)))
#define _dbus_return_val_if_error_is_set(error, val) _dbus_return_val_if_fail ((error) == NULL || !dbus_error_is_set ((error)), (val))

/* This alignment thing is from ORBit2 */
/* Align a value upward to a boundary, expressed as a number of bytes.
 * E.g. align to an 8-byte boundary with argument of 8.
 */

/*
 *   (this + boundary - 1)
 *          &
 *    ~(boundary - 1)
 */

#define _DBUS_ALIGN_VALUE(this, boundary) \
  (( ((uintptr_t)(this)) + (((uintptr_t)(boundary)) -1)) & (~(((uintptr_t)(boundary))-1)))

#define _DBUS_ALIGN_ADDRESS(this, boundary) \
  ((void*)_DBUS_ALIGN_VALUE(this, boundary))


DBUS_PRIVATE_EXPORT
char*       _dbus_strdup                (const char  *str);
void*       _dbus_memdup                (const void  *mem,
                                         size_t       n_bytes);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_string_array_contains (const char **array,
                                         const char  *str);
DBUS_PRIVATE_EXPORT
size_t      _dbus_string_array_length   (const char **array);
char**      _dbus_dup_string_array      (const char **array);

#define _DBUS_INT16_MIN	 ((dbus_int16_t) 0x8000)
#define _DBUS_INT16_MAX	 ((dbus_int16_t) 0x7fff)
#define _DBUS_UINT16_MAX ((dbus_uint16_t)0xffff)
#define _DBUS_INT32_MIN	 ((dbus_int32_t) 0x80000000)
#define _DBUS_INT32_MAX	 ((dbus_int32_t) 0x7fffffff)
#define _DBUS_UINT32_MAX ((dbus_uint32_t)0xffffffff)
/* using 32-bit here is sort of bogus */
#define _DBUS_INT_MIN	 _DBUS_INT32_MIN
#define _DBUS_INT_MAX	 _DBUS_INT32_MAX
#define _DBUS_UINT_MAX	 _DBUS_UINT32_MAX
#define _DBUS_INT64_MAX	 DBUS_INT64_CONSTANT  (0x7fffffffffffffff)
#define _DBUS_UINT64_MAX DBUS_UINT64_CONSTANT (0xffffffffffffffff)
#define _DBUS_ONE_KILOBYTE 1024
#define _DBUS_ONE_MEGABYTE 1024 * _DBUS_ONE_KILOBYTE
#define _DBUS_ONE_HOUR_IN_MILLISECONDS (1000 * 60 * 60)
#define _DBUS_USEC_PER_SECOND          (1000000)

#undef	MAX
#define MAX(a, b)  (((a) > (b)) ? (a) : (b))

#undef	MIN
#define MIN(a, b)  (((a) < (b)) ? (a) : (b))

#undef	ABS
#define ABS(a)	   (((a) < 0) ? -(a) : (a))

#define _DBUS_ISASCII(c) ((c) != '\0' && (((c) & ~0x7f) == 0))

typedef void (* DBusForeachFunction) (void *element,
                                      void *data);

void _dbus_verbose_bytes           (const unsigned char *data,
                                    int                  len,
                                    int                  offset);
DBUS_PRIVATE_EXPORT
void _dbus_verbose_bytes_of_string (const DBusString    *str,
                                    int                  start,
                                    int                  len);

DBUS_PRIVATE_EXPORT
extern const char *_dbus_no_memory_message;
#define _DBUS_SET_OOM(error) dbus_set_error_const ((error), DBUS_ERROR_NO_MEMORY, _dbus_no_memory_message)
DBUS_PRIVATE_EXPORT
void _dbus_set_error_valist (DBusError  *error,
                             const char *name,
                             const char *format,
                             va_list     args) _DBUS_GNUC_PRINTF (3, 0);

typedef dbus_bool_t (* DBusTestMemoryFunction)  (void        *data,
                                                 dbus_bool_t  have_memory);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/* Memory debugging */
void        _dbus_set_fail_alloc_counter        (int  until_next_fail);
int         _dbus_get_fail_alloc_counter        (void);
void        _dbus_set_fail_alloc_failures       (int  failures_per_failure);
int         _dbus_get_fail_alloc_failures       (void);
dbus_bool_t _dbus_decrement_fail_alloc_counter  (void);
dbus_bool_t _dbus_disable_mem_pools             (void);
DBUS_PRIVATE_EXPORT
int         _dbus_get_malloc_blocks_outstanding (void);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_test_oom_handling (const char             *description,
                                     DBusTestMemoryFunction  func,
                                     void                   *data);
#else
#define _dbus_set_fail_alloc_counter(n)
#define _dbus_get_fail_alloc_counter _DBUS_INT_MAX

/* These are constant expressions so that blocks
 * they protect should be optimized away
 */
#define _dbus_decrement_fail_alloc_counter() (FALSE)
#define _dbus_disable_mem_pools()            (FALSE)
#define _dbus_get_malloc_blocks_outstanding() (0)

#define _dbus_test_oom_handling(description, func, data) ((*func) (data, TRUE))
#endif /* !DBUS_ENABLE_EMBEDDED_TESTS */

typedef void (* DBusShutdownFunction) (void *data);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_register_shutdown_func          (DBusShutdownFunction  function,
                                                   void                 *data);
dbus_bool_t _dbus_register_shutdown_func_unlocked (DBusShutdownFunction  function,
                                                   void                 *data);

extern int _dbus_current_generation;

/* The weird case convention is to avoid having to change all the callers,
 * which would be quite a mega-patch. */
typedef enum
{
  /* index 0-4 */
  _DBUS_LOCK_list,
  _DBUS_LOCK_connection_slots,
  _DBUS_LOCK_pending_call_slots,
  _DBUS_LOCK_server_slots,
  _DBUS_LOCK_message_slots,
  /* index 5-9 */
  _DBUS_LOCK_bus,
  _DBUS_LOCK_bus_datas,
  _DBUS_LOCK_shutdown_funcs,
  _DBUS_LOCK_system_users,
  _DBUS_LOCK_message_cache,
  /* index 10-12 */
  _DBUS_LOCK_shared_connections,
  _DBUS_LOCK_machine_uuid,
  _DBUS_LOCK_sysdeps,

  _DBUS_N_GLOBAL_LOCKS
} DBusGlobalLock;

_DBUS_WARN_UNUSED_RESULT
dbus_bool_t _dbus_lock   (DBusGlobalLock lock);
void        _dbus_unlock (DBusGlobalLock lock);

#define _DBUS_LOCK_NAME(name)           _DBUS_LOCK_##name
#define _DBUS_LOCK(name)                _dbus_lock   (_DBUS_LOCK_##name)
#define _DBUS_UNLOCK(name)              _dbus_unlock (_DBUS_LOCK_##name)

DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_address_append_escaped (DBusString       *escaped,
                                            const DBusString *unescaped);

void          _dbus_set_bad_address        (DBusError         *error,
                                            const char        *address_problem_type,
                                            const char        *address_problem_field,
                                            const char        *address_problem_other);

#define DBUS_UUID_LENGTH_BYTES 16
#define DBUS_UUID_LENGTH_WORDS (DBUS_UUID_LENGTH_BYTES / 4)
#define DBUS_UUID_LENGTH_HEX   (DBUS_UUID_LENGTH_BYTES * 2)

/**
 * A globally unique ID ; we have one for each DBusServer, and also one for each
 * machine with libdbus installed on it.
 */
union DBusGUID
{
  dbus_uint32_t as_uint32s[DBUS_UUID_LENGTH_WORDS];     /**< guid as four uint32 values */
  char as_bytes[DBUS_UUID_LENGTH_BYTES];                /**< guid as 16 single-byte values */
};

DBUS_PRIVATE_EXPORT _DBUS_WARN_UNUSED_RESULT
dbus_bool_t _dbus_generate_uuid  (DBusGUID         *uuid,
                                  DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_uuid_encode    (const DBusGUID   *uuid,
                                  DBusString       *encoded);
dbus_bool_t _dbus_read_uuid_file (const DBusString *filename,
                                  DBusGUID         *uuid,
                                  dbus_bool_t       create_if_not_found,
                                  DBusError        *error);

dbus_bool_t _dbus_write_uuid_file (const DBusString *filename,
                                   const DBusGUID   *uuid,
                                   DBusError        *error);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_get_local_machine_uuid_encoded (DBusString *uuid_str,
                                                  DBusError  *error);

#define _DBUS_PASTE2(a, b) a ## b
#define _DBUS_PASTE(a, b) _DBUS_PASTE2 (a, b)
#define _DBUS_STATIC_ASSERT(expr) \
  typedef struct { char _assertion[(expr) ? 1 : -1]; } \
  _DBUS_PASTE (_DBUS_STATIC_ASSERT_, __LINE__) _DBUS_GNUC_UNUSED

#define _DBUS_STRINGIFY(x) #x
#define _DBUS_FILE_LINE __FILE__ ":" _DBUS_STRINGIFY(__LINE__)

DBUS_END_DECLS

#endif /* DBUS_INTERNALS_H */
