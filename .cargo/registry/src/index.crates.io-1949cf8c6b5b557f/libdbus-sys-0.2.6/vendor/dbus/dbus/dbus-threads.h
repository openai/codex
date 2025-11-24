/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-threads.h  D-Bus threads handling
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

#ifndef DBUS_THREADS_H
#define DBUS_THREADS_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusThreads
 * @{
 */

/** An opaque mutex type provided by the #DBusThreadFunctions implementation installed by dbus_threads_init(). */
typedef struct DBusMutex DBusMutex;
/** An opaque condition variable type provided by the #DBusThreadFunctions implementation installed by dbus_threads_init(). */
typedef struct DBusCondVar DBusCondVar;

/** Deprecated, provide DBusRecursiveMutexNewFunction instead. */
typedef DBusMutex*  (* DBusMutexNewFunction)    (void);
/** Deprecated, provide DBusRecursiveMutexFreeFunction instead. */
typedef void        (* DBusMutexFreeFunction)   (DBusMutex *mutex);
/** Deprecated, provide DBusRecursiveMutexLockFunction instead. Return value is lock success, but gets ignored in practice. */
typedef dbus_bool_t (* DBusMutexLockFunction)   (DBusMutex *mutex);
/** Deprecated, provide DBusRecursiveMutexUnlockFunction instead. Return value is unlock success, but gets ignored in practice. */
typedef dbus_bool_t (* DBusMutexUnlockFunction) (DBusMutex *mutex);

/** Creates a new recursively-lockable mutex, or returns #NULL if not
 * enough memory.  Can only fail due to lack of memory.  Found in
 * #DBusThreadFunctions. Do not just use PTHREAD_MUTEX_RECURSIVE for
 * this, because it does not save/restore the recursion count when
 * waiting on a condition. libdbus requires the Java-style behavior
 * where the mutex is fully unlocked to wait on a condition.
 */
typedef DBusMutex*  (* DBusRecursiveMutexNewFunction)    (void);
/** Frees a recursively-lockable mutex.  Found in #DBusThreadFunctions.
 */
typedef void        (* DBusRecursiveMutexFreeFunction)   (DBusMutex *mutex);
/** Locks a recursively-lockable mutex.  Found in #DBusThreadFunctions.
 * Can only fail due to lack of memory.
 */
typedef void        (* DBusRecursiveMutexLockFunction)   (DBusMutex *mutex);
/** Unlocks a recursively-lockable mutex.  Found in #DBusThreadFunctions.
 * Can only fail due to lack of memory.
 */
typedef void        (* DBusRecursiveMutexUnlockFunction) (DBusMutex *mutex);

/** Creates a new condition variable.  Found in #DBusThreadFunctions.
 * Can only fail (returning #NULL) due to lack of memory.
 */
typedef DBusCondVar*  (* DBusCondVarNewFunction)         (void);
/** Frees a condition variable.  Found in #DBusThreadFunctions.
 */
typedef void          (* DBusCondVarFreeFunction)        (DBusCondVar *cond);

/** Waits on a condition variable.  Found in
 * #DBusThreadFunctions. Must work with either a recursive or
 * nonrecursive mutex, whichever the thread implementation
 * provides. Note that PTHREAD_MUTEX_RECURSIVE does not work with
 * condition variables (does not save/restore the recursion count) so
 * don't try using simply pthread_cond_wait() and a
 * PTHREAD_MUTEX_RECURSIVE to implement this, it won't work right.
 *
 * Has no error conditions. Must succeed if it returns.
 */
typedef void          (* DBusCondVarWaitFunction)        (DBusCondVar *cond,
							  DBusMutex   *mutex);

/** Waits on a condition variable with a timeout.  Found in
 *  #DBusThreadFunctions. Returns #TRUE if the wait did not
 *  time out, and #FALSE if it did.
 *
 * Has no error conditions. Must succeed if it returns. 
 */
typedef dbus_bool_t   (* DBusCondVarWaitTimeoutFunction) (DBusCondVar *cond,
							  DBusMutex   *mutex,
							  int          timeout_milliseconds);
/** Wakes one waiting thread on a condition variable.  Found in #DBusThreadFunctions.
 *
 * Has no error conditions. Must succeed if it returns.
 */
typedef void          (* DBusCondVarWakeOneFunction) (DBusCondVar *cond);

/** Wakes all waiting threads on a condition variable.  Found in #DBusThreadFunctions.
 *
 * Has no error conditions. Must succeed if it returns.
 */
typedef void          (* DBusCondVarWakeAllFunction) (DBusCondVar *cond);

/**
 * Flags indicating which functions are present in #DBusThreadFunctions. Used to allow
 * the library to detect older callers of dbus_threads_init() if new possible functions
 * are added to #DBusThreadFunctions.
 */
typedef enum 
{
  DBUS_THREAD_FUNCTIONS_MUTEX_NEW_MASK      = 1 << 0,
  DBUS_THREAD_FUNCTIONS_MUTEX_FREE_MASK     = 1 << 1,
  DBUS_THREAD_FUNCTIONS_MUTEX_LOCK_MASK     = 1 << 2,
  DBUS_THREAD_FUNCTIONS_MUTEX_UNLOCK_MASK   = 1 << 3,
  DBUS_THREAD_FUNCTIONS_CONDVAR_NEW_MASK    = 1 << 4,
  DBUS_THREAD_FUNCTIONS_CONDVAR_FREE_MASK   = 1 << 5,
  DBUS_THREAD_FUNCTIONS_CONDVAR_WAIT_MASK   = 1 << 6,
  DBUS_THREAD_FUNCTIONS_CONDVAR_WAIT_TIMEOUT_MASK   = 1 << 7,
  DBUS_THREAD_FUNCTIONS_CONDVAR_WAKE_ONE_MASK = 1 << 8,
  DBUS_THREAD_FUNCTIONS_CONDVAR_WAKE_ALL_MASK = 1 << 9,
  DBUS_THREAD_FUNCTIONS_RECURSIVE_MUTEX_NEW_MASK    = 1 << 10,
  DBUS_THREAD_FUNCTIONS_RECURSIVE_MUTEX_FREE_MASK   = 1 << 11,
  DBUS_THREAD_FUNCTIONS_RECURSIVE_MUTEX_LOCK_MASK   = 1 << 12,
  DBUS_THREAD_FUNCTIONS_RECURSIVE_MUTEX_UNLOCK_MASK = 1 << 13,
  DBUS_THREAD_FUNCTIONS_ALL_MASK     = (1 << 14) - 1
} DBusThreadFunctionsMask;

/**
 * Functions that must be implemented to make the D-Bus library
 * thread-aware.
 *
 * If you supply both recursive and non-recursive mutexes,
 * libdbus will use the non-recursive version for condition variables,
 * and the recursive version in other contexts.
 *
 * The condition variable functions have to work with nonrecursive
 * mutexes if you provide those, or with recursive mutexes if you
 * don't.
 */
typedef struct
{
  unsigned int mask; /**< Mask indicating which functions are present. */

  DBusMutexNewFunction mutex_new; /**< Function to create a mutex; optional and deprecated. */
  DBusMutexFreeFunction mutex_free; /**< Function to free a mutex; optional and deprecated. */
  DBusMutexLockFunction mutex_lock; /**< Function to lock a mutex; optional and deprecated. */
  DBusMutexUnlockFunction mutex_unlock; /**< Function to unlock a mutex; optional and deprecated. */

  DBusCondVarNewFunction condvar_new; /**< Function to create a condition variable */
  DBusCondVarFreeFunction condvar_free; /**< Function to free a condition variable */
  DBusCondVarWaitFunction condvar_wait; /**< Function to wait on a condition */
  DBusCondVarWaitTimeoutFunction condvar_wait_timeout; /**< Function to wait on a condition with a timeout */
  DBusCondVarWakeOneFunction condvar_wake_one; /**< Function to wake one thread waiting on the condition */
  DBusCondVarWakeAllFunction condvar_wake_all; /**< Function to wake all threads waiting on the condition */
 
  DBusRecursiveMutexNewFunction recursive_mutex_new; /**< Function to create a recursive mutex */
  DBusRecursiveMutexFreeFunction recursive_mutex_free; /**< Function to free a recursive mutex */
  DBusRecursiveMutexLockFunction recursive_mutex_lock; /**< Function to lock a recursive mutex */
  DBusRecursiveMutexUnlockFunction recursive_mutex_unlock; /**< Function to unlock a recursive mutex */

  void (* padding1) (void); /**< Reserved for future expansion */
  void (* padding2) (void); /**< Reserved for future expansion */
  void (* padding3) (void); /**< Reserved for future expansion */
  void (* padding4) (void); /**< Reserved for future expansion */
  
} DBusThreadFunctions;

DBUS_EXPORT
dbus_bool_t  dbus_threads_init         (const DBusThreadFunctions *functions);
DBUS_EXPORT
dbus_bool_t  dbus_threads_init_default (void);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_THREADS_H */
