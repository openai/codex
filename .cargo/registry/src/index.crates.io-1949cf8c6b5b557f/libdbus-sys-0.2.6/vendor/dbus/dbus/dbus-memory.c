/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-memory.c  D-Bus memory handling
 *
 * Copyright (C) 2002, 2003  Red Hat Inc.
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

#include <config.h>
#include "dbus-memory.h"
#include "dbus-internals.h"
#include "dbus-sysdeps.h"
#include "dbus-list.h"
#include "dbus-threads.h"
#include <dbus/dbus-test-tap.h>
#include <stdlib.h>

/**
 * @defgroup DBusMemory Memory Allocation
 * @ingroup  DBus
 * @brief dbus_malloc(), dbus_free(), etc.
 *
 * Functions and macros related to allocating and releasing
 * blocks of memory.
 *
 */

/**
 * @defgroup DBusMemoryInternals Memory allocation implementation details
 * @ingroup  DBusInternals
 * @brief internals of dbus_malloc() etc.
 *
 * Implementation details related to allocating and releasing blocks
 * of memory.
 */

/**
 * @addtogroup DBusMemory
 *
 * @{
 */

/**
 * @def dbus_new
 *
 * Safe macro for using dbus_malloc(). Accepts the type
 * to allocate and the number of type instances to
 * allocate as arguments, and returns a memory block
 * cast to the desired type, instead of as a void*.
 *
 * @param type type name to allocate
 * @param count number of instances in the allocated array
 * @returns the new memory block or #NULL on failure
 */

/**
 * @def dbus_new0
 *
 * Safe macro for using dbus_malloc0(). Accepts the type
 * to allocate and the number of type instances to
 * allocate as arguments, and returns a memory block
 * cast to the desired type, instead of as a void*.
 * The allocated array is initialized to all-bits-zero.
 *
 * @param type type name to allocate
 * @param count number of instances in the allocated array
 * @returns the new memory block or #NULL on failure
 */

/**
 * @typedef DBusFreeFunction
 *
 * The type of a function which frees a block of memory.
 *
 * @param memory the memory to free
 */

/** @} */ /* end of public API docs */

/**
 * @addtogroup DBusMemoryInternals
 *
 * @{
 */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/* Test-only, does not need to be thread-safe */
static dbus_bool_t debug_initialized = FALSE;
static int fail_nth = -1;
static size_t fail_size = 0;
static int fail_alloc_counter = _DBUS_INT_MAX;
static int n_failures_per_failure = 1;
static int n_failures_this_failure = 0;
static dbus_bool_t guards = FALSE;
static dbus_bool_t disable_mem_pools = FALSE;
static dbus_bool_t backtrace_on_fail_alloc = FALSE;
static dbus_bool_t malloc_cannot_fail = FALSE;
static DBusAtomic n_blocks_outstanding = {0};

/** value stored in guard padding for debugging buffer overrun */
#define GUARD_VALUE 0xdeadbeef
/** size of the information about the block stored in guard mode */
#define GUARD_INFO_SIZE 8
/** size of the GUARD_VALUE-filled padding after the header info  */
#define GUARD_START_PAD 16
/** size of the GUARD_VALUE-filled padding at the end of the block */
#define GUARD_END_PAD 16
/** size of stuff at start of block */
#define GUARD_START_OFFSET (GUARD_START_PAD + GUARD_INFO_SIZE)
/** total extra size over the requested allocation for guard stuff */
#define GUARD_EXTRA_SIZE (GUARD_START_OFFSET + GUARD_END_PAD)

static void
_dbus_initialize_malloc_debug (void)
{
  if (!debug_initialized)
    {
      debug_initialized = TRUE;
      
      if (_dbus_getenv ("DBUS_MALLOC_FAIL_NTH") != NULL)
	{
	  fail_nth = atoi (_dbus_getenv ("DBUS_MALLOC_FAIL_NTH"));
          fail_alloc_counter = fail_nth;
          _dbus_verbose ("Will fail dbus_malloc every %d times\n", fail_nth);
	}
      
      if (_dbus_getenv ("DBUS_MALLOC_FAIL_GREATER_THAN") != NULL)
        {
          fail_size = atoi (_dbus_getenv ("DBUS_MALLOC_FAIL_GREATER_THAN"));
          _dbus_verbose ("Will fail mallocs over %ld bytes\n",
                         (long) fail_size);
        }

      if (_dbus_getenv ("DBUS_MALLOC_GUARDS") != NULL)
        {
          guards = TRUE;
          _dbus_verbose ("Will use dbus_malloc guards\n");
        }

      if (_dbus_getenv ("DBUS_DISABLE_MEM_POOLS") != NULL)
        {
          disable_mem_pools = TRUE;
          _dbus_verbose ("Will disable memory pools\n");
        }

      if (_dbus_getenv ("DBUS_MALLOC_BACKTRACES") != NULL)
        {
          backtrace_on_fail_alloc = TRUE;
          _dbus_verbose ("Will backtrace on failing a dbus_malloc\n");
        }

      if (_dbus_getenv ("DBUS_MALLOC_CANNOT_FAIL") != NULL)
        {
          malloc_cannot_fail = TRUE;
          _dbus_verbose ("Will abort if system malloc() and friends fail\n");
        }
    }
}

/**
 * Whether to turn off mem pools, useful for leak checking.
 *
 * @returns #TRUE if mempools should not be used.
 */
dbus_bool_t
_dbus_disable_mem_pools (void)
{
  _dbus_initialize_malloc_debug ();
  return disable_mem_pools;
}

/**
 * Sets the number of allocations until we simulate a failed
 * allocation. If set to 0, the next allocation to run
 * fails; if set to 1, one succeeds then the next fails; etc.
 * Set to _DBUS_INT_MAX to not fail anything. 
 *
 * @param until_next_fail number of successful allocs before one fails
 */
void
_dbus_set_fail_alloc_counter (int until_next_fail)
{
  _dbus_initialize_malloc_debug ();

  fail_alloc_counter = until_next_fail;

#if 0
  _dbus_verbose ("Set fail alloc counter = %d\n", fail_alloc_counter);
#endif
}

/**
 * Gets the number of successful allocs until we'll simulate
 * a failed alloc.
 *
 * @returns current counter value
 */
int
_dbus_get_fail_alloc_counter (void)
{
  _dbus_initialize_malloc_debug ();

  return fail_alloc_counter;
}

/**
 * Sets how many mallocs to fail when the fail alloc counter reaches
 * 0.
 *
 * @param failures_per_failure number to fail
 */
void
_dbus_set_fail_alloc_failures (int failures_per_failure)
{
  n_failures_per_failure = failures_per_failure;
}

/**
 * Gets the number of failures we'll have when the fail malloc
 * counter reaches 0.
 *
 * @returns number of failures planned
 */
int
_dbus_get_fail_alloc_failures (void)
{
  return n_failures_per_failure;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Called when about to alloc some memory; if
 * it returns #TRUE, then the allocation should
 * fail. If it returns #FALSE, then the allocation
 * should not fail.
 *
 * @returns #TRUE if this alloc should fail
 */
dbus_bool_t
_dbus_decrement_fail_alloc_counter (void)
{
  _dbus_initialize_malloc_debug ();

  if (fail_alloc_counter <= 0)
    {
      if (backtrace_on_fail_alloc)
        _dbus_print_backtrace ();

      _dbus_verbose ("failure %d\n", n_failures_this_failure);
      
      n_failures_this_failure += 1;
      if (n_failures_this_failure >= n_failures_per_failure)
        {
          if (fail_nth >= 0)
            fail_alloc_counter = fail_nth;
          else
            fail_alloc_counter = _DBUS_INT_MAX;

          n_failures_this_failure = 0;

          _dbus_verbose ("reset fail alloc counter to %d\n", fail_alloc_counter);
        }
      
      return TRUE;
    }
  else
    {
      fail_alloc_counter -= 1;
      return FALSE;
    }
}
#endif /* DBUS_ENABLE_EMBEDDED_TESTS */

/**
 * Get the number of outstanding malloc()'d blocks.
 *
 * @returns number of blocks
 */
int
_dbus_get_malloc_blocks_outstanding (void)
{
  return _dbus_atomic_get (&n_blocks_outstanding);
}

/**
 * Where the block came from.
 */
typedef enum
{
  SOURCE_UNKNOWN,
  SOURCE_MALLOC,
  SOURCE_REALLOC,
  SOURCE_MALLOC_ZERO,
  SOURCE_REALLOC_NULL
} BlockSource;

static const char*
source_string (BlockSource source)
{
  switch (source)
    {
    case SOURCE_UNKNOWN:
      return "unknown";
    case SOURCE_MALLOC:
      return "malloc";
    case SOURCE_REALLOC:
      return "realloc";
    case SOURCE_MALLOC_ZERO:
      return "malloc0";
    case SOURCE_REALLOC_NULL:
      return "realloc(NULL)";
    default:
      _dbus_assert_not_reached ("Invalid malloc block source ID");
      return "invalid!";
    }
}

static void
check_guards (void       *free_block,
              dbus_bool_t overwrite)
{
  if (free_block != NULL)
    {
      unsigned char *block = ((unsigned char*)free_block) - GUARD_START_OFFSET;
      size_t requested_bytes = *(dbus_uint32_t*)block;
      BlockSource source = *(dbus_uint32_t*)(block + 4);
      unsigned int i;
      dbus_bool_t failed;

      failed = FALSE;

#if 0
      _dbus_verbose ("Checking %d bytes request from source %s\n",
                     requested_bytes, source_string (source));
#endif
      
      i = GUARD_INFO_SIZE;
      while (i < GUARD_START_OFFSET)
        {
          dbus_uint32_t value = *(dbus_uint32_t*) &block[i];
          if (value != GUARD_VALUE)
            {
              _dbus_warn ("Block of %lu bytes from %s had start guard value 0x%ux at %d expected 0x%x",
                          (long) requested_bytes, source_string (source),
                          value, i, GUARD_VALUE);
              failed = TRUE;
            }
          
          i += 4;
        }

      i = GUARD_START_OFFSET + requested_bytes;
      while (i < (GUARD_START_OFFSET + requested_bytes + GUARD_END_PAD))
        {
          dbus_uint32_t value = *(dbus_uint32_t*) &block[i];
          if (value != GUARD_VALUE)
            {
              _dbus_warn ("Block of %lu bytes from %s had end guard value 0x%ux at %d expected 0x%x",
                          (long) requested_bytes, source_string (source),
                          value, i, GUARD_VALUE);
              failed = TRUE;
            }
          
          i += 4;
        }

      /* set memory to anything but nul bytes */
      if (overwrite)
        memset (free_block, 'g', requested_bytes);
      
      if (failed)
        _dbus_assert_not_reached ("guard value corruption");
    }
}

static void*
set_guards (void       *real_block,
            size_t      requested_bytes,
            BlockSource source)
{
  unsigned char *block = real_block;
  unsigned int i;
  
  if (block == NULL)
    return NULL;

  _dbus_assert (GUARD_START_OFFSET + GUARD_END_PAD == GUARD_EXTRA_SIZE);
  
  *((dbus_uint32_t*)block) = requested_bytes;
  *((dbus_uint32_t*)(block + 4)) = source;

  i = GUARD_INFO_SIZE;
  while (i < GUARD_START_OFFSET)
    {
      (*(dbus_uint32_t*) &block[i]) = GUARD_VALUE;
      
      i += 4;
    }

  i = GUARD_START_OFFSET + requested_bytes;
  while (i < (GUARD_START_OFFSET + requested_bytes + GUARD_END_PAD))
    {
      (*(dbus_uint32_t*) &block[i]) = GUARD_VALUE;
      
      i += 4;
    }
  
  check_guards (block + GUARD_START_OFFSET, FALSE);
  
  return block + GUARD_START_OFFSET;
}

#endif

/** @} */ /* End of internals docs */


/**
 * @addtogroup DBusMemory
 *
 * @{
 */

/**
 * Allocates the given number of bytes, as with standard
 * malloc(). Guaranteed to return #NULL if bytes is zero
 * on all platforms. Returns #NULL if the allocation fails.
 * The memory must be released with dbus_free().
 *
 * dbus_malloc() memory is NOT safe to free with regular free() from
 * the C library. Free it with dbus_free() only.
 *
 * @param bytes number of bytes to allocate
 * @return allocated memory, or #NULL if the allocation fails.
 */
void*
dbus_malloc (size_t bytes)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  _dbus_initialize_malloc_debug ();
  
  if (_dbus_decrement_fail_alloc_counter ())
    {
      _dbus_verbose (" FAILING malloc of %ld bytes\n", (long) bytes);
      return NULL;
    }
#endif

  if (bytes == 0) /* some system mallocs handle this, some don't */
    return NULL;
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  else if (fail_size != 0 && bytes > fail_size)
    return NULL;
  else if (guards)
    {
      void *block;

      block = malloc (bytes + GUARD_EXTRA_SIZE);
      if (block)
        {
          _dbus_atomic_inc (&n_blocks_outstanding);
        }
      else if (malloc_cannot_fail)
        {
          _dbus_warn ("out of memory: malloc (%ld + %ld)",
              (long) bytes, (long) GUARD_EXTRA_SIZE);
          _dbus_abort ();
        }
      
      return set_guards (block, bytes, SOURCE_MALLOC);
    }
#endif
  else
    {
      void *mem;
      mem = malloc (bytes);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (mem)
        {
          _dbus_atomic_inc (&n_blocks_outstanding);
        }
      else if (malloc_cannot_fail)
        {
          _dbus_warn ("out of memory: malloc (%ld)", (long) bytes);
          _dbus_abort ();
        }
#endif

      return mem;
    }
}

/**
 * Allocates the given number of bytes, as with standard malloc(), but
 * all bytes are initialized to zero as with calloc(). Guaranteed to
 * return #NULL if bytes is zero on all platforms. Returns #NULL if the
 * allocation fails.  The memory must be released with dbus_free().
 *
 * dbus_malloc0() memory is NOT safe to free with regular free() from
 * the C library. Free it with dbus_free() only.
 *
 * @param bytes number of bytes to allocate
 * @return allocated memory, or #NULL if the allocation fails.
 */
void*
dbus_malloc0 (size_t bytes)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  _dbus_initialize_malloc_debug ();
  
  if (_dbus_decrement_fail_alloc_counter ())
    {
      _dbus_verbose (" FAILING malloc0 of %ld bytes\n", (long) bytes);
      
      return NULL;
    }
#endif
  
  if (bytes == 0)
    return NULL;
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  else if (fail_size != 0 && bytes > fail_size)
    return NULL;
  else if (guards)
    {
      void *block;

      block = calloc (bytes + GUARD_EXTRA_SIZE, 1);

      if (block)
        {
          _dbus_atomic_inc (&n_blocks_outstanding);
        }
      else if (malloc_cannot_fail)
        {
          _dbus_warn ("out of memory: calloc (%ld + %ld, 1)",
              (long) bytes, (long) GUARD_EXTRA_SIZE);
          _dbus_abort ();
        }

      return set_guards (block, bytes, SOURCE_MALLOC_ZERO);
    }
#endif
  else
    {
      void *mem;
      mem = calloc (bytes, 1);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (mem)
        {
          _dbus_atomic_inc (&n_blocks_outstanding);
        }
      else if (malloc_cannot_fail)
        {
          _dbus_warn ("out of memory: calloc (%ld)", (long) bytes);
          _dbus_abort ();
        }
#endif

      return mem;
    }
}

/**
 * Resizes a block of memory previously allocated by dbus_malloc() or
 * dbus_malloc0(). Guaranteed to free the memory and return #NULL if bytes
 * is zero on all platforms. Returns #NULL if the resize fails.
 * If the resize fails, the memory is not freed.
 *
 * @param memory block to be resized
 * @param bytes new size of the memory block
 * @return allocated memory, or #NULL if the resize fails.
 */
void*
dbus_realloc (void  *memory,
              size_t bytes)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  _dbus_initialize_malloc_debug ();
  
  if (_dbus_decrement_fail_alloc_counter ())
    {
      _dbus_verbose (" FAILING realloc of %ld bytes\n", (long) bytes);
      
      return NULL;
    }
#endif
  
  if (bytes == 0) /* guarantee this is safe */
    {
      dbus_free (memory);
      return NULL;
    }
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  else if (fail_size != 0 && bytes > fail_size)
    return NULL;
  else if (guards)
    {
      if (memory)
        {
          size_t old_bytes;
          void *block;
          
          check_guards (memory, FALSE);
          
          block = realloc (((unsigned char*)memory) - GUARD_START_OFFSET,
                           bytes + GUARD_EXTRA_SIZE);

          if (block == NULL)
            {
              if (malloc_cannot_fail)
                {
                  _dbus_warn ("out of memory: realloc (%p, %ld + %ld)",
                      memory, (long) bytes, (long) GUARD_EXTRA_SIZE);
                  _dbus_abort ();
                }

              return NULL;
            }

          old_bytes = *(dbus_uint32_t*)block;
          if (bytes >= old_bytes)
            /* old guards shouldn't have moved */
            check_guards (((unsigned char*)block) + GUARD_START_OFFSET, FALSE);
          
          return set_guards (block, bytes, SOURCE_REALLOC);
        }
      else
        {
          void *block;
          
          block = malloc (bytes + GUARD_EXTRA_SIZE);

          if (block)
            {
              _dbus_atomic_inc (&n_blocks_outstanding);
            }
          else if (malloc_cannot_fail)
            {
              _dbus_warn ("out of memory: malloc (%ld + %ld)",
                  (long) bytes, (long) GUARD_EXTRA_SIZE);
              _dbus_abort ();
            }

          return set_guards (block, bytes, SOURCE_REALLOC_NULL);   
        }
    }
#endif
  else
    {
      void *mem;
      mem = realloc (memory, bytes);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (mem == NULL && malloc_cannot_fail)
        {
          _dbus_warn ("out of memory: malloc (%ld)", (long) bytes);
          _dbus_abort ();
        }

      if (memory == NULL && mem != NULL)
	    _dbus_atomic_inc (&n_blocks_outstanding);
#endif
      return mem;
    }
}

/**
 * Frees a block of memory previously allocated by dbus_malloc() or
 * dbus_malloc0(). If passed #NULL, does nothing.
 * 
 * @param memory block to be freed
 */
void
dbus_free (void  *memory)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (guards)
    {
      check_guards (memory, TRUE);
      if (memory)
        {
#ifdef DBUS_DISABLE_ASSERT
          _dbus_atomic_dec (&n_blocks_outstanding);
#else
          dbus_int32_t old_value;

          old_value = _dbus_atomic_dec (&n_blocks_outstanding);
          _dbus_assert (old_value >= 1);
#endif

          free (((unsigned char*)memory) - GUARD_START_OFFSET);
        }
      
      return;
    }
#endif
    
  if (memory) /* we guarantee it's safe to free (NULL) */
    {
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#ifdef DBUS_DISABLE_ASSERT
      _dbus_atomic_dec (&n_blocks_outstanding);
#else
      dbus_int32_t old_value;

      old_value = _dbus_atomic_dec (&n_blocks_outstanding);
      _dbus_assert (old_value >= 1);
#endif
#endif

      free (memory);
    }
}

/**
 * Frees a #NULL-terminated array of strings.
 * If passed #NULL, does nothing.
 *
 * @param str_array the array to be freed
 */
void
dbus_free_string_array (char **str_array)
{
  if (str_array)
    {
      int i;

      i = 0;
      while (str_array[i])
	{
	  dbus_free (str_array[i]);
	  i++;
	}

      dbus_free (str_array);
    }
}

/** @} */ /* End of public API docs block */


/**
 * @addtogroup DBusMemoryInternals
 *
 * @{
 */

/**
 * _dbus_current_generation is used to track each
 * time that dbus_shutdown() is called, so we can
 * reinit things after it's been called. It is simply
 * incremented each time we shut down.
 */
int _dbus_current_generation = 1;

/**
 * Represents a function to be called on shutdown.
 */
typedef struct ShutdownClosure ShutdownClosure;

/**
 * This struct represents a function to be called on shutdown.
 */
struct ShutdownClosure
{
  ShutdownClosure *next;     /**< Next ShutdownClosure */
  DBusShutdownFunction func; /**< Function to call */
  void *data;                /**< Data for function */
};

/* Protected by _DBUS_LOCK (shutdown_funcs) */
static ShutdownClosure *registered_globals = NULL;

/**
 * Register a cleanup function to be called exactly once
 * the next time dbus_shutdown() is called.
 *
 * @param func the function
 * @param data data to pass to the function
 * @returns #FALSE on not enough memory
 */
dbus_bool_t
_dbus_register_shutdown_func (DBusShutdownFunction  func,
                              void                 *data)
{
  dbus_bool_t ok;

  if (!_DBUS_LOCK (shutdown_funcs))
    return FALSE;

  ok = _dbus_register_shutdown_func_unlocked (func, data);
  _DBUS_UNLOCK (shutdown_funcs);
  return ok;
}

dbus_bool_t
_dbus_register_shutdown_func_unlocked (DBusShutdownFunction  func,
                                       void                 *data)
{
  ShutdownClosure *c;

  c = dbus_new (ShutdownClosure, 1);

  if (c == NULL)
    return FALSE;

  c->func = func;
  c->data = data;

  c->next = registered_globals;
  registered_globals = c;

  return TRUE;
}

/** @} */ /* End of private API docs block */


/**
 * @addtogroup DBusMemory
 *
 * @{
 */

/**
 * Frees all memory allocated internally by libdbus and
 * reverses the effects of dbus_threads_init(). libdbus keeps internal
 * global variables, for example caches and thread locks, and it
 * can be useful to free these internal data structures.
 *
 * dbus_shutdown() does NOT free memory that was returned
 * to the application. It only frees libdbus-internal
 * data structures.
 *
 * You MUST free all memory and release all reference counts
 * returned to you by libdbus prior to calling dbus_shutdown().
 *
 * If a shared connection is open, calling dbus_shutdown() will
 * drain its queue of messages and disconnect it. In particular,
 * this will result in processing of the special Disconnected
 * signal, which may result in a call to _exit(), unless you
 * have used dbus_connection_set_exit_on_disconnect() to disable
 * that behaviour.
 *
 * You can't continue to use any D-Bus objects, such as connections,
 * that were allocated prior to dbus_shutdown(). You can, however,
 * start over; call dbus_threads_init() again, create new connections,
 * and so forth.
 *
 * WARNING: dbus_shutdown() is NOT thread safe, it must be called
 * while NO other threads are using D-Bus. (Remember, you have to free
 * all D-Bus objects and memory before you call dbus_shutdown(), so no
 * thread can be using libdbus.)
 *
 * The purpose of dbus_shutdown() is to allow applications to get
 * clean output from memory leak checkers. dbus_shutdown() may also be
 * useful if you want to dlopen() libdbus instead of linking to it,
 * and want to be able to unload the library again.
 *
 * There is absolutely no requirement to call dbus_shutdown() - in fact,
 * most applications won't bother and should not feel guilty.
 * 
 * You have to know that nobody is using libdbus in your application's
 * process before you can call dbus_shutdown(). One implication of this
 * is that calling dbus_shutdown() from a library is almost certainly
 * wrong, since you don't know what the rest of the app is up to.
 * 
 */
void
dbus_shutdown (void)
{
  while (registered_globals != NULL)
    {
      ShutdownClosure *c;

      c = registered_globals;
      registered_globals = c->next;
      
      (* c->func) (c->data);
      
      dbus_free (c);
    }

  /* We wrap this in the thread-initialization lock because
   * dbus_threads_init() uses the current generation to tell whether
   * we're initialized, so we need to make sure that un-initializing
   * propagates into all threads. */
  _dbus_threads_lock_platform_specific ();
  _dbus_current_generation += 1;
  _dbus_threads_unlock_platform_specific ();
}

/** @} */ /** End of public API docs block */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "dbus-test.h"

/**
 * @ingroup DBusMemoryInternals
 * Unit test for DBusMemory
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_memory_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  dbus_bool_t old_guards;
  void *p;
  size_t size;

  old_guards = guards;
  guards = TRUE;
  p = dbus_malloc (4);
  if (p == NULL)
    _dbus_test_fatal ("no memory");
  for (size = 4; size < 256; size += 4)
    {
      p = dbus_realloc (p, size);
      if (p == NULL)
        _dbus_test_fatal ("no memory");
    }
  for (size = 256; size != 0; size -= 4)
    {
      p = dbus_realloc (p, size);
      if (p == NULL)
        _dbus_test_fatal ("no memory");
    }
  dbus_free (p);
  guards = old_guards;
  return TRUE;
}

#endif
