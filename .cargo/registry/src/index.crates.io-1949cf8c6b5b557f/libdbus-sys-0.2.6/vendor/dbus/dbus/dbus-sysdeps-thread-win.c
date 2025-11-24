/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-pthread.c Implements threads using Windows threads (internal to libdbus)
 *
 * Copyright (C) 2006  Red Hat, Inc.
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
#include "dbus-internals.h"
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-win.h"
#include "dbus-threads.h"
#include "dbus-list.h"

#include <stdio.h>

#include <windows.h>

/* Protected by DllMain lock, effectively */
static dbus_bool_t global_init_done = FALSE;
static CRITICAL_SECTION init_lock;

/* Called from C++ code in dbus-init-win.cpp. */
void
_dbus_threads_windows_init_global (void)
{
  /* this ensures that the object that acts as our global constructor
   * actually gets linked in when we're linked statically */
  _dbus_threads_windows_ensure_ctor_linked ();

  InitializeCriticalSection (&init_lock);
  global_init_done = TRUE;
}

struct DBusCondVar {
  DBusList *list;        /**< list thread-local-stored events waiting on the cond variable */
  CRITICAL_SECTION lock; /**< lock protecting the list */
};

static DWORD dbus_cond_event_tls = TLS_OUT_OF_INDEXES;

/* Protected by DllMain lock, effectively */
static HMODULE dbus_dll_hmodule;

void *
_dbus_win_get_dll_hmodule (void)
{
  return dbus_dll_hmodule;
}

#ifdef DBUS_WINCE
#define hinst_t HANDLE
#else
#define hinst_t HINSTANCE
#endif

BOOL WINAPI DllMain (hinst_t, DWORD, LPVOID);

/* We need this to free the TLS events on thread exit */
BOOL WINAPI
DllMain (hinst_t hinstDLL,
	 DWORD     fdwReason,
	 LPVOID    lpvReserved)
{
  HANDLE event;
  switch (fdwReason) 
    { 
    case DLL_PROCESS_ATTACH:
      dbus_dll_hmodule = hinstDLL;
      break;
    case DLL_THREAD_DETACH:
      if (dbus_cond_event_tls != TLS_OUT_OF_INDEXES)
	{
	  event = TlsGetValue(dbus_cond_event_tls);
	  CloseHandle (event);
	  TlsSetValue(dbus_cond_event_tls, NULL);
	}
      break;
    case DLL_PROCESS_DETACH: 
      if (dbus_cond_event_tls != TLS_OUT_OF_INDEXES)
	{
	  event = TlsGetValue(dbus_cond_event_tls);
	  CloseHandle (event);
	  TlsSetValue(dbus_cond_event_tls, NULL);

	  TlsFree(dbus_cond_event_tls); 
	}
      break;
    default: 
      break; 
    }
  return TRUE;
}

DBusCMutex *
_dbus_platform_cmutex_new (void)
{
  HANDLE handle;
  handle = CreateMutex (NULL, FALSE, NULL);
  return (DBusCMutex *) handle;
}

DBusRMutex *
_dbus_platform_rmutex_new (void)
{
  HANDLE handle;
  handle = CreateMutex (NULL, FALSE, NULL);
  return (DBusRMutex *) handle;
}

void
_dbus_platform_cmutex_free (DBusCMutex *mutex)
{
  CloseHandle ((HANDLE *) mutex);
}

void
_dbus_platform_rmutex_free (DBusRMutex *mutex)
{
  CloseHandle ((HANDLE *) mutex);
}

void
_dbus_platform_cmutex_lock (DBusCMutex *mutex)
{
  WaitForSingleObject ((HANDLE *) mutex, INFINITE);
}

void
_dbus_platform_rmutex_lock (DBusRMutex *mutex)
{
  WaitForSingleObject ((HANDLE *) mutex, INFINITE);
}

void
_dbus_platform_cmutex_unlock (DBusCMutex *mutex)
{
  ReleaseMutex ((HANDLE *) mutex);
}

void
_dbus_platform_rmutex_unlock (DBusRMutex *mutex)
{
  ReleaseMutex ((HANDLE *) mutex);
}

DBusCondVar *
_dbus_platform_condvar_new (void)
{
  DBusCondVar *cond;
    
  cond = dbus_new (DBusCondVar, 1);
  if (cond == NULL)
    return NULL;
  
  cond->list = NULL;
  
  InitializeCriticalSection (&cond->lock);
  return cond;
}

void
_dbus_platform_condvar_free (DBusCondVar *cond)
{
  DeleteCriticalSection (&cond->lock);
  _dbus_list_clear (&cond->list);
  dbus_free (cond);
}

static dbus_bool_t
_dbus_condvar_wait_win32 (DBusCondVar *cond,
			  DBusCMutex *mutex,
			  int milliseconds)
{
  DWORD retval;
  dbus_bool_t ret;
  HANDLE event = TlsGetValue (dbus_cond_event_tls);

  if (!event)
    {
      event = CreateEvent (0, FALSE, FALSE, NULL);
      if (event == 0)
	return FALSE;
      TlsSetValue (dbus_cond_event_tls, event);
    }

  EnterCriticalSection (&cond->lock);

  /* The event must not be signaled. Check this */
  _dbus_assert (WaitForSingleObject (event, 0) == WAIT_TIMEOUT);

  ret = _dbus_list_append (&cond->list, event);
  
  LeaveCriticalSection (&cond->lock);
  
  if (!ret)
    return FALSE; /* Prepend failed */

  _dbus_platform_cmutex_unlock (mutex);
  retval = WaitForSingleObject (event, milliseconds);
  _dbus_platform_cmutex_lock (mutex);
  
  if (retval == WAIT_TIMEOUT)
    {
      EnterCriticalSection (&cond->lock);
      _dbus_list_remove (&cond->list, event);

      /* In the meantime we could have been signaled, so we must again
       * wait for the signal, this time with no timeout, to reset
       * it. retval is set again to honour the late arrival of the
       * signal */
      retval = WaitForSingleObject (event, 0);

      LeaveCriticalSection (&cond->lock);
    }

#ifndef DBUS_DISABLE_ASSERT
  EnterCriticalSection (&cond->lock);

  /* Now event must not be inside the array, check this */
  _dbus_assert (_dbus_list_remove (&cond->list, event) == FALSE);

  LeaveCriticalSection (&cond->lock);
#endif /* !G_DISABLE_ASSERT */

  return retval != WAIT_TIMEOUT;
}

void
_dbus_platform_condvar_wait (DBusCondVar *cond,
                             DBusCMutex  *mutex)
{
  _dbus_condvar_wait_win32 (cond, mutex, INFINITE);
}

dbus_bool_t
_dbus_platform_condvar_wait_timeout (DBusCondVar               *cond,
				     DBusCMutex                *mutex,
				     int                        timeout_milliseconds)
{
  return _dbus_condvar_wait_win32 (cond, mutex, timeout_milliseconds);
}

void
_dbus_platform_condvar_wake_one (DBusCondVar *cond)
{
  EnterCriticalSection (&cond->lock);
  
  if (cond->list != NULL)
    {
      SetEvent (_dbus_list_pop_first (&cond->list));
      /* Avoid live lock by pushing the waiter to the mutex lock
         instruction, which is fair.  If we don't do this, we could
         acquire the condition variable again before the waiter has a
         chance itself, leading to starvation.  */
      Sleep (0);
    }
  LeaveCriticalSection (&cond->lock);
}

dbus_bool_t
_dbus_threads_init_platform_specific (void)
{
  /* We reuse this over several generations, because we can't
   * free the events once they are in use
   */
  if (dbus_cond_event_tls == TLS_OUT_OF_INDEXES)
    {
      dbus_cond_event_tls = TlsAlloc ();
      if (dbus_cond_event_tls == TLS_OUT_OF_INDEXES)
	return FALSE;
    }

  return TRUE;
}

void
_dbus_threads_lock_platform_specific (void)
{
  _dbus_assert (global_init_done);
  EnterCriticalSection (&init_lock);
}

void
_dbus_threads_unlock_platform_specific (void)
{
  _dbus_assert (global_init_done);
  LeaveCriticalSection (&init_lock);
}

#ifdef DBUS_ENABLE_VERBOSE_MODE
void
_dbus_print_thread (void)
{
  fprintf (stderr, "%lu: 0x%04lx: ", _dbus_pid_for_log (), GetCurrentThreadId ());
}
#endif
