#include <config.h>

//#define SPAWN_DEBUG

#if !defined(SPAWN_DEBUG) || defined(_MSC_VER)
#define PING()
#else
#define PING() fprintf (stderr, "%s:%s:%d\n", __FILE__, __FUNCTION__, __LINE__); fflush (stderr)
#endif

#include <stdio.h>

/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-spawn-win32.c Wrapper around g_spawn
 *
 * Copyright (C) 2002, 2003, 2004  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
 * Copyright (C) 2005 Novell, Inc.
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
#include "dbus-spawn.h"
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-win.h"
#include "dbus-internals.h"
#include "dbus-test.h"
#include "dbus-protocol.h"

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
//#define STRICT
//#include <windows.h>
//#undef STRICT
#include <winsock2.h>
#undef interface

#include <stdlib.h>

#ifndef DBUS_WINCE
#include <process.h>
#endif

/**
 * Babysitter implementation details
 */
struct DBusBabysitter
  {
    DBusAtomic refcount;
    char *log_name;

    HANDLE thread_handle;
    HANDLE child_handle;
    DBusSocket socket_to_babysitter;	/* Connection to the babysitter thread */
    DBusSocket socket_to_main;

    DBusWatchList *watches;
    DBusWatch *sitter_watch;
    DBusBabysitterFinishedFunc finished_cb;
    void *finished_data;

    dbus_bool_t have_spawn_errno;
    int spawn_errno;
    dbus_bool_t have_child_status;
    int child_status;
  };

static void
_dbus_babysitter_trace_ref (DBusBabysitter *sitter,
    int old_refcount,
    int new_refcount,
    const char *why)
{
#ifdef DBUS_ENABLE_VERBOSE_MODE
  static int enabled = -1;

  _dbus_trace_ref ("DBusBabysitter", sitter, old_refcount, new_refcount, why,
      "DBUS_BABYSITTER_TRACE", &enabled);
#endif
}

static DBusBabysitter*
_dbus_babysitter_new (void)
{
  DBusBabysitter *sitter;
  dbus_int32_t old_refcount;

  sitter = dbus_new0 (DBusBabysitter, 1);
  if (sitter == NULL)
    return NULL;

  old_refcount = _dbus_atomic_inc (&sitter->refcount);

  _dbus_babysitter_trace_ref (sitter, old_refcount, old_refcount+1, __FUNCTION__);

  sitter->child_handle = NULL;

  sitter->socket_to_babysitter = sitter->socket_to_main = _dbus_socket_get_invalid ();

  sitter->watches = _dbus_watch_list_new ();
  if (sitter->watches == NULL)
    {
      _dbus_babysitter_unref (sitter);
      return NULL;
    }

  sitter->have_spawn_errno = FALSE;
  sitter->have_child_status = FALSE;

  return sitter;
}

/**
 * Increment the reference count on the babysitter object.
 *
 * @param sitter the babysitter
 * @returns the babysitter
 */
DBusBabysitter *
_dbus_babysitter_ref (DBusBabysitter *sitter)
{
  dbus_int32_t old_refcount;
  PING();
  _dbus_assert (sitter != NULL);

  old_refcount = _dbus_atomic_inc (&sitter->refcount);
  _dbus_assert (old_refcount > 0);
  _dbus_babysitter_trace_ref (sitter, old_refcount, old_refcount+1, __FUNCTION__);

  return sitter;
}

static void
close_socket_to_babysitter (DBusBabysitter *sitter)
{
  _dbus_verbose ("Closing babysitter\n");

  if (sitter->sitter_watch != NULL)
    {
      _dbus_assert (sitter->watches != NULL);
      _dbus_watch_list_remove_watch (sitter->watches,  sitter->sitter_watch);
      _dbus_watch_invalidate (sitter->sitter_watch);
      _dbus_watch_unref (sitter->sitter_watch);
      sitter->sitter_watch = NULL;
    }

  if (sitter->socket_to_babysitter.sock != INVALID_SOCKET)
    {
      _dbus_close_socket (sitter->socket_to_babysitter, NULL);
      sitter->socket_to_babysitter.sock = INVALID_SOCKET;
    }
}

/**
 * Decrement the reference count on the babysitter object.
 *
 * @param sitter the babysitter
 */
void
_dbus_babysitter_unref (DBusBabysitter *sitter)
{
  dbus_int32_t old_refcount;

  PING();
  _dbus_assert (sitter != NULL);

  old_refcount = _dbus_atomic_dec (&sitter->refcount);
  _dbus_assert (old_refcount > 0);
  _dbus_babysitter_trace_ref (sitter, old_refcount, old_refcount-1, __FUNCTION__);

  if (old_refcount == 1)
    {
      close_socket_to_babysitter (sitter);

      if (sitter->socket_to_main.sock != INVALID_SOCKET)
        {
          _dbus_close_socket (sitter->socket_to_main, NULL);
          sitter->socket_to_main.sock = INVALID_SOCKET;
        }

      if (sitter->child_handle != NULL)
        {
          CloseHandle (sitter->child_handle);
          sitter->child_handle = NULL;
        }

      if (sitter->sitter_watch)
        {
          _dbus_watch_invalidate (sitter->sitter_watch);
          _dbus_watch_unref (sitter->sitter_watch);
          sitter->sitter_watch = NULL;
        }

      if (sitter->watches)
        _dbus_watch_list_free (sitter->watches);

      if (sitter->thread_handle)
        {
          CloseHandle (sitter->thread_handle);
          sitter->thread_handle = NULL;
        }

      dbus_free (sitter->log_name);

      dbus_free (sitter);
    }
}

void
_dbus_babysitter_kill_child (DBusBabysitter *sitter)
{
  PING();
  if (sitter->child_handle == NULL)
    return; /* child is already dead, or we're so hosed we'll never recover */

  PING();
  TerminateProcess (sitter->child_handle, 12345);
}

/**
 * Checks whether the child has exited, without blocking.
 *
 * @param sitter the babysitter
 */
dbus_bool_t
_dbus_babysitter_get_child_exited (DBusBabysitter *sitter)
{
  PING();
  return (sitter->child_handle == NULL);
}

/**
 * Gets the exit status of the child. We do this so implementation specific
 * detail is not cluttering up dbus, for example the system launcher code.
 * This can only be called if the child has exited, i.e. call
 * _dbus_babysitter_get_child_exited(). It returns FALSE if the child
 * did not return a status code, e.g. because the child was signaled
 * or we failed to ever launch the child in the first place.
 *
 * @param sitter the babysitter
 * @param status the returned status code
 * @returns #FALSE on failure
 */
dbus_bool_t
_dbus_babysitter_get_child_exit_status (DBusBabysitter *sitter,
                                        int            *status)
{
  if (!_dbus_babysitter_get_child_exited (sitter))
    _dbus_assert_not_reached ("Child has not exited");

  if (!sitter->have_child_status ||
      sitter->child_status == STILL_ACTIVE)
    return FALSE;

  *status = sitter->child_status;
  return TRUE;
}

/**
 * Sets the #DBusError with an explanation of why the spawned
 * child process exited (on a signal, or whatever). If
 * the child process has not exited, does nothing (error
 * will remain unset).
 *
 * @param sitter the babysitter
 * @param error an error to fill in
 */
void
_dbus_babysitter_set_child_exit_error (DBusBabysitter *sitter,
                                       DBusError      *error)
{
  PING();
  if (!_dbus_babysitter_get_child_exited (sitter))
    return;

  PING();
  if (sitter->have_spawn_errno)
    {
      char *emsg = _dbus_win_error_string (sitter->spawn_errno);
      dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                      "Failed to execute program %s: %s",
                      sitter->log_name, emsg);
      _dbus_win_free_error_string (emsg);
    }
  else if (sitter->have_child_status)
    {
      PING();
      dbus_set_error (error, DBUS_ERROR_SPAWN_CHILD_EXITED,
                      "Process %s exited with status %d",
                      sitter->log_name, sitter->child_status);
    }
  else
    {
      PING();
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Process %s exited, status unknown",
                      sitter->log_name);
    }
  PING();
}

dbus_bool_t
_dbus_babysitter_set_watch_functions (DBusBabysitter            *sitter,
                                      DBusAddWatchFunction       add_function,
                                      DBusRemoveWatchFunction    remove_function,
                                      DBusWatchToggledFunction   toggled_function,
                                      void                      *data,
                                      DBusFreeFunction           free_data_function)
{
  PING();
  return _dbus_watch_list_set_functions (sitter->watches,
                                         add_function,
                                         remove_function,
                                         toggled_function,
                                         data,
                                         free_data_function);
}

static dbus_bool_t
handle_watch (DBusWatch       *watch,
              unsigned int     condition,
              void            *data)
{
  DBusBabysitter *sitter = data;

  /* On Unix dbus-spawn uses a babysitter *process*, thus it has to
   * actually send the exit statuses, error codes and whatnot through
   * sockets and/or pipes. On Win32, the babysitter is jus a thread,
   * so it can set the status fields directly in the babysitter struct
   * just fine. The socket pipe is used just so we can watch it with
   * select(), as soon as anything is written to it we know that the
   * babysitter thread has recorded the status in the babysitter
   * struct.
   */

  PING();
  close_socket_to_babysitter (sitter);
  PING();

  if (_dbus_babysitter_get_child_exited (sitter) &&
      sitter->finished_cb != NULL)
    {
      sitter->finished_cb (sitter, sitter->finished_data);
      sitter->finished_cb = NULL;
    }

  return TRUE;
}

/* protect_argv lifted from GLib, relicensed by author, Tor Lillqvist */
static int
protect_argv (char  * const *argv,
              char        ***new_argv)
{
  int i;
  int argc = 0;
  char **args = NULL;

  while (argv[argc])
    ++argc;
  args = dbus_malloc ((argc + 1) * sizeof (char *));
  if (args == NULL)
    return -1;

  for (i = 0; i < argc; i++)
    (args)[i] = NULL;

  /* Quote each argv element if necessary, so that it will get
   * reconstructed correctly in the C runtime startup code.  Note that
   * the unquoting algorithm in the C runtime is really weird, and
   * rather different than what Unix shells do. See stdargv.c in the C
   * runtime sources (in the Platform SDK, in src/crt).
   *
   * Note that an new_argv[0] constructed by this function should
   * *not* be passed as the filename argument to a spawn* or exec*
   * family function. That argument should be the real file name
   * without any quoting.
   */
  for (i = 0; i < argc; i++)
    {
      const char *p = argv[i];
      char *q;
      int len = 0;
      int need_dblquotes = FALSE;
      while (*p)
        {
          if (*p == ' ' || *p == '\t')
            need_dblquotes = TRUE;
          else if (*p == '"')
            len++;
          else if (*p == '\\')
            {
              const char *pp = p;
              while (*pp && *pp == '\\')
                pp++;
              if (*pp == '"')
                len++;
            }
          len++;
          p++;
        }

      q = args[i] = dbus_malloc (len + need_dblquotes*2 + 1);

      if (q == NULL)
        {
          dbus_free_string_array (args);
          return -1;
        }

      p = argv[i];

      if (need_dblquotes)
        *q++ = '"';

      while (*p)
        {
          if (*p == '"')
            *q++ = '\\';
          else if (*p == '\\')
            {
              const char *pp = p;
              while (*pp && *pp == '\\')
                pp++;
              if (*pp == '"')
                *q++ = '\\';
            }
          *q++ = *p;
          p++;
        }

      if (need_dblquotes)
        *q++ = '"';
      *q++ = '\0';
      /* printf ("argv[%d]:%s, need_dblquotes:%s len:%d => %s\n", i, argv[i], need_dblquotes?"TRUE":"FALSE", len, (args)[i]); */
    }
  args[argc] = NULL;
  *new_argv = args;

  return argc;
}

static dbus_bool_t
build_commandline (char **argv, DBusString *result)
{
  return _dbus_string_append_strings (result, argv, ' ');
}

static dbus_bool_t
build_env_block (char** envp, DBusString *result)
{
  if (!_dbus_string_append_strings (result, envp, '\0'))
    return FALSE;

   /* We need a double `\0` to terminate the environment block.
    * DBusString provides one `\0` after the length-counted data,
    * so add one more. */
   if (!_dbus_string_append_byte (result, '\0'))
     return FALSE;

  return TRUE;
}

/**
 * Creates a process with arguments and environment variables
 *
 * @param name name of the program
 * @param argv list of char* pointers for the arguments
 * @param envp list of char pointers for the environment
 * @param inherit_handles specifies whether handles should be inherited by the child process
 * @param error the error to set, if NULL no error will be set
 * @return #NULL if an error occurred, the reason is returned in \p error
 * @note The call to GetLastError() after this function may not return the expected value.
 */
HANDLE
_dbus_spawn_program (const char *name,
                     char      **argv,
                     char      **envp,
                     dbus_bool_t inherit_handles,
                     DBusError  *error)
{
  PROCESS_INFORMATION pi = { NULL, 0, 0, 0 };
  STARTUPINFOA si;
  DBusString arg_string = _DBUS_STRING_INIT_INVALID;
  DBusString env_block = _DBUS_STRING_INIT_INVALID;
  BOOL result = FALSE;
  char *env = NULL;

  if (!_dbus_string_init (&arg_string) || !_dbus_string_init (&env_block))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

#ifdef DBUS_WINCE
  if (argv && argv[0])
    {
      if (!build_commandline (argv + 1, &arg_string))
        _DBUS_SET_OOM (error);
        goto out;
    }
#else
  if (!build_commandline (argv, &arg_string))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }
#endif
  if (_dbus_string_get_length (&arg_string) == 0)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED, "No arguments given to start '%s'", name);
      goto out;
    }

  if (envp != NULL)
    {
      if (!build_env_block (envp, &env_block))
        {
          _DBUS_SET_OOM (error);
          goto out;
        }
      /* env_block consists of '0' terminated strings */
      env = _dbus_string_get_data (&env_block);
    }

  memset (&si, 0, sizeof (si));
  si.cb = sizeof (si);

#ifdef DBUS_ENABLE_VERBOSE_MODE
  {
    DBusString temp = _DBUS_STRING_INIT_INVALID;

    if (!_dbus_string_init (&temp))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

    if (!_dbus_string_append_strings (&temp, envp, ';'))
      {
        _dbus_string_free (&temp);
        _DBUS_SET_OOM (error);
        goto out;
      }

    _dbus_verbose ("spawning '%s'' with args: '%s' env: '%s'\n", name,
                   _dbus_string_get_const_data (&arg_string),
                   _dbus_string_get_const_data (&temp));
    _dbus_string_free (&temp);
  }
#endif

#ifdef DBUS_WINCE
  result = CreateProcessA (name, _dbus_string_get_const_data (&arg_string), NULL, NULL, FALSE, 0,
#else
  result = CreateProcessA (NULL,  /* no application name */
                           _dbus_string_get_data (&arg_string),
                           NULL, /* no process attributes */
                           NULL, /* no thread attributes */
                           inherit_handles, /* inherit handles */
                           0, /* flags */
#endif
                           env, NULL, &si, &pi);
  if (!result)
    {
      _dbus_win_set_error_from_last_error (error, "Unable to start '%s' with arguments '%s'",
                                           name, _dbus_string_get_const_data (&arg_string));
      goto out;
    }

out:
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, result);

  _dbus_string_free (&arg_string);
  _dbus_string_free (&env_block);

  if (result)
    {
      CloseHandle (pi.hThread);
      return pi.hProcess;
    }

  return NULL;
}


static DWORD __stdcall
babysitter (void *parameter)
{
  int ret = 0;
  DBusBabysitter *sitter = (DBusBabysitter *) parameter;

  PING();
  if (sitter->child_handle != NULL)
    {
      DWORD status;

      PING();
      // wait until process finished
      WaitForSingleObject (sitter->child_handle, INFINITE);

      PING();
      ret = GetExitCodeProcess (sitter->child_handle, &status);
      if (ret)
        {
          sitter->child_status = status;
          sitter->have_child_status = TRUE;
        }

      CloseHandle (sitter->child_handle);
      sitter->child_handle = NULL;
    }

  PING();
  send (sitter->socket_to_main.sock, " ", 1, 0);

  _dbus_babysitter_unref (sitter);

  return ret ? 0 : 1;
}

dbus_bool_t
_dbus_spawn_async_with_babysitter (DBusBabysitter           **sitter_p,
                                   const char                *log_name,
                                   char              * const *argv,
                                   char              * const *envp,
                                   DBusSpawnFlags             flags _DBUS_GNUC_UNUSED,
                                   DBusSpawnChildSetupFunc    child_setup _DBUS_GNUC_UNUSED,
                                   void                      *user_data _DBUS_GNUC_UNUSED,
                                   DBusError                 *error)
{
  DBusBabysitter *sitter;
  DWORD sitter_thread_id;
  HANDLE handle;
  int argc;
  char **my_argv = NULL;
  DBusError local_error = DBUS_ERROR_INIT;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  _dbus_assert (argv[0] != NULL);

  if (sitter_p != NULL)
    *sitter_p = NULL;

  PING();
  sitter = _dbus_babysitter_new ();
  if (sitter == NULL)
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  sitter->log_name = _dbus_strdup (log_name);
  if (sitter->log_name == NULL && log_name != NULL)
    {
      _DBUS_SET_OOM (error);
      goto out0;
    }

  if (sitter->log_name == NULL)
    sitter->log_name = _dbus_strdup (argv[0]);

  if (sitter->log_name == NULL)
    {
      _DBUS_SET_OOM (error);
      goto out0;
    }

  PING();
  if (!_dbus_socketpair (&sitter->socket_to_babysitter,
                         &sitter->socket_to_main,
                         FALSE, error))
    goto out0;

  sitter->sitter_watch = _dbus_watch_new (sitter->socket_to_babysitter,
                                          DBUS_WATCH_READABLE,
                                          TRUE, handle_watch, sitter, NULL);
  PING();
  if (sitter->sitter_watch == NULL)
    {
      _DBUS_SET_OOM (error);
      goto out0;
    }

  PING();
  if (!_dbus_watch_list_add_watch (sitter->watches,  sitter->sitter_watch))
    {
      /* we need to free it early so the destructor won't try to remove it
       * without it having been added, which DBusLoop doesn't allow */
      _dbus_watch_invalidate (sitter->sitter_watch);
      _dbus_watch_unref (sitter->sitter_watch);
      sitter->sitter_watch = NULL;

      _DBUS_SET_OOM (error);
      goto out0;
    }

  argc = protect_argv (argv, &my_argv);
  if (argc == -1)
    {
      _DBUS_SET_OOM (error);
      goto out0;
    }

  _dbus_verbose ("babysitter: spawn child '%s'\n", my_argv[0]);

  PING();
  handle = _dbus_spawn_program (sitter->log_name, my_argv, (char **) envp, FALSE, &local_error);

  if (my_argv != NULL)
    {
      dbus_free_string_array (my_argv);
    }

  PING();
  if (handle == NULL)
    {
      if (dbus_error_has_name (&local_error, DBUS_ERROR_NO_MEMORY))
        {
          sitter->child_handle = NULL;
          sitter->have_spawn_errno = TRUE;
          sitter->spawn_errno = ERROR_NOT_ENOUGH_MEMORY;
          dbus_move_error (&local_error, error);
        }
      else
        {
          sitter->child_handle = NULL;
          sitter->have_spawn_errno = TRUE;
          sitter->spawn_errno = GetLastError();
          dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                          "Failed to spawn child: %s", local_error.message);
        }
      dbus_error_free (&local_error);
      goto out0;
    }
  else
    dbus_error_free (&local_error);

  sitter->child_handle = handle;

  PING();
  sitter->thread_handle = (HANDLE) CreateThread (NULL, 0, babysitter,
                  _dbus_babysitter_ref (sitter), 0, &sitter_thread_id);

  if (sitter->thread_handle == NULL)
    {
      PING();
      dbus_set_error_const (error, DBUS_ERROR_SPAWN_FORK_FAILED,
                            "Failed to create new thread");
      goto out0;
    }

  PING();
  if (sitter_p != NULL)
    *sitter_p = sitter;
  else
    _dbus_babysitter_unref (sitter);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  PING();
  return TRUE;

out0:
  _dbus_babysitter_unref (sitter);

  return FALSE;
}

void
_dbus_babysitter_set_result_function  (DBusBabysitter             *sitter,
                                       DBusBabysitterFinishedFunc  finished,
                                       void                       *user_data)
{
  sitter->finished_cb = finished;
  sitter->finished_data = user_data;
}

#define LIVE_CHILDREN(sitter) ((sitter)->child_handle != NULL)

void
_dbus_babysitter_block_for_child_exit (DBusBabysitter *sitter)
{
  /* The thread terminates after the child does. We want to wait for the thread,
   * not just the child, to avoid data races and ensure that it has freed all
   * its memory. */
  WaitForSingleObject (sitter->thread_handle, INFINITE);
}
