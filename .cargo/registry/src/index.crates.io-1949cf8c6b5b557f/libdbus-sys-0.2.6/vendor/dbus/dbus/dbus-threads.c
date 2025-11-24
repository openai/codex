/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-threads.h  D-Bus threads handling
 *
 * Copyright (C) 2002, 2003, 2006 Red Hat Inc.
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
#include "dbus-threads.h"
#include "dbus-internals.h"
#include "dbus-threads-internal.h"
#include "dbus-list.h"

/* Protected by _dbus_threads_lock_platform_specific() */
static int thread_init_generation = 0;

/**
 * @defgroup DBusThreadsInternals Thread functions
 * @ingroup  DBusInternals
 * @brief _dbus_rmutex_lock(), etc.
 *
 * Functions and macros related to threads and thread locks.
 *
 * @{
 */

/**
 * Creates a new mutex
 * or creates a no-op mutex if threads are not initialized.
 * May return #NULL even if threads are initialized, indicating
 * out-of-memory.
 *
 * If possible, the mutex returned by this function is recursive, to
 * avoid deadlocks. However, that cannot be relied on.
 *
 * @param location_p the location of the new mutex, can return #NULL on OOM
 */
void
_dbus_rmutex_new_at_location (DBusRMutex **location_p)
{
  _dbus_assert (location_p != NULL);

  if (!dbus_threads_init_default ())
    {
      *location_p = NULL;
      return;
    }

  *location_p = _dbus_platform_rmutex_new ();
}

/**
 * Creates a new mutex
 * or creates a no-op mutex if threads are not initialized.
 * May return #NULL even if threads are initialized, indicating
 * out-of-memory.
 *
 * The returned mutex is suitable for use with condition variables.
 *
 * @param location_p the location of the new mutex, can return #NULL on OOM
 */
void
_dbus_cmutex_new_at_location (DBusCMutex **location_p)
{
  _dbus_assert (location_p != NULL);

  if (!dbus_threads_init_default ())
    {
      *location_p = NULL;
      return;
    }

  *location_p = _dbus_platform_cmutex_new ();
}

/**
 * Frees a DBusRMutex; does nothing if passed a #NULL pointer.
 */
void
_dbus_rmutex_free_at_location (DBusRMutex **location_p)
{
  if (location_p == NULL)
    return;

  if (*location_p != NULL)
    _dbus_platform_rmutex_free (*location_p);
}

/**
 * Frees a DBusCMutex; does nothing if passed a #NULL pointer.
 */
void
_dbus_cmutex_free_at_location (DBusCMutex **location_p)
{
  if (location_p == NULL)
    return;

  if (*location_p != NULL)
    _dbus_platform_cmutex_free (*location_p);
}

/**
 * Locks a mutex. Does nothing if passed a #NULL pointer.
 * Locks may be recursive if threading implementation initialized
 * recursive locks.
 */
void
_dbus_rmutex_lock (DBusRMutex *mutex)
{
  if (mutex == NULL)
    return;

  _dbus_platform_rmutex_lock (mutex);
}

/**
 * Locks a mutex. Does nothing if passed a #NULL pointer.
 * Locks may be recursive if threading implementation initialized
 * recursive locks.
 */
void
_dbus_cmutex_lock (DBusCMutex *mutex)
{
  if (mutex == NULL)
    return;

  _dbus_platform_cmutex_lock (mutex);
}

/**
 * Unlocks a mutex. Does nothing if passed a #NULL pointer.
 *
 * @returns #TRUE on success
 */
void
_dbus_rmutex_unlock (DBusRMutex *mutex)
{
  if (mutex == NULL)
    return;

  _dbus_platform_rmutex_unlock (mutex);
}

/**
 * Unlocks a mutex. Does nothing if passed a #NULL pointer.
 *
 * @returns #TRUE on success
 */
void
_dbus_cmutex_unlock (DBusCMutex *mutex)
{
  if (mutex == NULL)
    return;

  _dbus_platform_cmutex_unlock (mutex);
}

/**
 * Creates a new condition variable using the function supplied
 * to dbus_threads_init(), or creates a no-op condition variable
 * if threads are not initialized. May return #NULL even if
 * threads are initialized, indicating out-of-memory.
 *
 * @returns new mutex or #NULL
 */
DBusCondVar *
_dbus_condvar_new (void)
{
  if (!dbus_threads_init_default ())
    return NULL;

  return _dbus_platform_condvar_new ();
}


/**
 * This does the same thing as _dbus_condvar_new.  It however
 * gives another level of indirection by allocating a pointer
 * to point to the condvar location; this used to be useful.
 *
 * @returns the location of a new condvar or #NULL on OOM
 */

void 
_dbus_condvar_new_at_location (DBusCondVar **location_p)
{
  _dbus_assert (location_p != NULL);

  *location_p = _dbus_condvar_new();
}


/**
 * Frees a conditional variable created with dbus_condvar_new(); does
 * nothing if passed a #NULL pointer.
 */
void
_dbus_condvar_free (DBusCondVar *cond)
{
  if (cond == NULL)
    return;

  _dbus_platform_condvar_free (cond);
}

/**
 * Frees a condition variable; does nothing if passed a #NULL pointer.
 */
void
_dbus_condvar_free_at_location (DBusCondVar **location_p)
{
  if (location_p == NULL)
    return;

  if (*location_p != NULL)
    _dbus_platform_condvar_free (*location_p);
}

/**
 * Atomically unlocks the mutex and waits for the conditions
 * variable to be signalled. Locks the mutex again before
 * returning.
 * Does nothing if passed a #NULL pointer.
 */
void
_dbus_condvar_wait (DBusCondVar *cond,
                    DBusCMutex  *mutex)
{
  if (cond == NULL || mutex == NULL)
    return;

  _dbus_platform_condvar_wait (cond, mutex);
}

/**
 * Atomically unlocks the mutex and waits for the conditions variable
 * to be signalled, or for a timeout. Locks the mutex again before
 * returning.  Does nothing if passed a #NULL pointer.  Return value
 * is #FALSE if we timed out, #TRUE otherwise.
 *
 * @param cond the condition variable
 * @param mutex the mutex
 * @param timeout_milliseconds the maximum time to wait
 * @returns #FALSE if the timeout occurred, #TRUE if not
 */
dbus_bool_t
_dbus_condvar_wait_timeout (DBusCondVar               *cond,
                            DBusCMutex                *mutex,
                            int                        timeout_milliseconds)
{
  if (cond == NULL || mutex == NULL)
    return TRUE;

  return _dbus_platform_condvar_wait_timeout (cond, mutex,
      timeout_milliseconds);
}

/**
 * If there are threads waiting on the condition variable, wake
 * up exactly one. 
 * Does nothing if passed a #NULL pointer.
 */
void
_dbus_condvar_wake_one (DBusCondVar *cond)
{
  if (cond == NULL)
    return;

  _dbus_platform_condvar_wake_one (cond);
}

/* Protected by _dbus_threads_lock_platform_specific() */
static DBusRMutex *global_locks[_DBUS_N_GLOBAL_LOCKS] = { NULL };

static void
shutdown_global_locks (void *nil)
{
  int i;

  for (i = 0; i < _DBUS_N_GLOBAL_LOCKS; i++)
    {
      _dbus_assert (global_locks[i] != NULL);
      _dbus_platform_rmutex_free (global_locks[i]);
      global_locks[i] = NULL;
    }
}

static dbus_bool_t
init_global_locks (void)
{
  int i;
  dbus_bool_t ok;

  for (i = 0; i < _DBUS_N_GLOBAL_LOCKS; i++)
    {
      _dbus_assert (global_locks[i] == NULL);

      global_locks[i] = _dbus_platform_rmutex_new ();

      if (global_locks[i] == NULL)
        goto failed;
    }

  _dbus_platform_rmutex_lock (global_locks[_DBUS_LOCK_shutdown_funcs]);
  ok = _dbus_register_shutdown_func_unlocked (shutdown_global_locks, NULL);
  _dbus_platform_rmutex_unlock (global_locks[_DBUS_LOCK_shutdown_funcs]);

  if (!ok)
    goto failed;

  return TRUE;

 failed:
  for (i = i - 1; i >= 0; i--)
    {
      _dbus_platform_rmutex_free (global_locks[i]);
      global_locks[i] = NULL;
    }

  return FALSE;
}

dbus_bool_t
_dbus_lock (DBusGlobalLock lock)
{
  _dbus_assert (lock >= 0);
  _dbus_assert (lock < _DBUS_N_GLOBAL_LOCKS);

  if (thread_init_generation != _dbus_current_generation &&
      !dbus_threads_init_default ())
    return FALSE;

  _dbus_platform_rmutex_lock (global_locks[lock]);
  return TRUE;
}

void
_dbus_unlock (DBusGlobalLock lock)
{
  _dbus_assert (lock >= 0);
  _dbus_assert (lock < _DBUS_N_GLOBAL_LOCKS);

  _dbus_platform_rmutex_unlock (global_locks[lock]);
}

/** @} */ /* end of internals */

/**
 * @defgroup DBusThreads Thread functions
 * @ingroup  DBus
 * @brief dbus_threads_init() and dbus_threads_init_default()
 *
 * Functions and macros related to threads and thread locks.
 *
 * If threads are initialized, the D-Bus library has locks on all
 * global data structures.  In addition, each #DBusConnection has a
 * lock, so only one thread at a time can touch the connection.  (See
 * @ref DBusConnection for more on connection locking.)
 *
 * Most other objects, however, do not have locks - they can only be
 * used from a single thread at a time, unless you lock them yourself.
 * For example, a #DBusMessage can't be modified from two threads
 * at once.
 * 
 * @{
 */

/**
 * Initializes threads, like dbus_threads_init_default().
 * This version previously allowed user-specified threading
 * primitives, but since D-Bus 1.6 it ignores them and behaves
 * exactly like dbus_threads_init_default().
 *
 * @param functions ignored, formerly functions for using threads
 * @returns #TRUE on success, #FALSE if no memory
 */
dbus_bool_t
dbus_threads_init (const DBusThreadFunctions *functions)
{
  _dbus_threads_lock_platform_specific ();

  if (thread_init_generation == _dbus_current_generation)
    {
      _dbus_threads_unlock_platform_specific ();
      return TRUE;
    }

  if (!_dbus_threads_init_platform_specific() ||
      !init_global_locks ())
    {
      _dbus_threads_unlock_platform_specific ();
      return FALSE;
    }

  thread_init_generation = _dbus_current_generation;

  _dbus_threads_unlock_platform_specific ();
  return TRUE;
}



/* Default thread implemenation */

/**
 * Initializes threads. If this function is not called, the D-Bus
 * library will not lock any data structures.  If it is called, D-Bus
 * will do locking, at some cost in efficiency.
 *
 * Since D-Bus 1.7 it is safe to call this function from any thread,
 * any number of times (but it must be called before any other
 * libdbus API is used).
 *
 * In D-Bus 1.6 or older, this function must be called in the main thread
 * before any other thread starts. As a result, it is not sufficient to
 * call this function in a library or plugin, unless the library or plugin
 * imposes a similar requirement on its callers.
 *
 * dbus_shutdown() reverses the effects of this function when it
 * resets all global state in libdbus.
 * 
 * @returns #TRUE on success, #FALSE if not enough memory
 */
dbus_bool_t
dbus_threads_init_default (void)
{
  return dbus_threads_init (NULL);
}


/** @} */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
