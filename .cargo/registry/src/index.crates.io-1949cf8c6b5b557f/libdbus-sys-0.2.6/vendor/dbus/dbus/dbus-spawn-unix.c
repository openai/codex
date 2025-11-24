/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-spawn-unix.c — Wrapper around fork/exec
 *
 * Copyright (C) 2002, 2003, 2004  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
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

#if defined(DBUS_WIN) || !defined(DBUS_UNIX)
#error "This file only makes sense on Unix OSs"
#endif

#include "dbus-spawn.h"
#include "dbus-sysdeps-unix.h"
#include "dbus-internals.h"
#include "dbus-test.h"
#include "dbus-protocol.h"

#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <stdio.h>
#include <stdlib.h>
#ifdef HAVE_ERRNO_H
#include <errno.h>
#endif
#ifdef HAVE_SYSTEMD
#ifdef HAVE_SYSLOG_H
#include <syslog.h>
#endif
#include <systemd/sd-journal.h>
#endif

#if defined(__APPLE__)
# include <crt_externs.h>
# define environ (*_NSGetEnviron ())
#elif !HAVE_DECL_ENVIRON
extern char **environ;
#endif

/**
 * @addtogroup DBusInternalsUtils
 * @{
 */

/*
 * I'm pretty sure this whole spawn file could be made simpler,
 * if you thought about it a bit.
 */

/**
 * Enumeration for status of a read()
 */
typedef enum
{
  READ_STATUS_OK,    /**< Read succeeded */
  READ_STATUS_ERROR, /**< Some kind of error */
  READ_STATUS_EOF    /**< EOF returned */
} ReadStatus;

static ReadStatus
read_ints (int        fd,
	   int       *buf,
	   int        n_ints_in_buf,
	   int       *n_ints_read,
	   DBusError *error)
{
  size_t bytes = 0;
  ReadStatus retval;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  retval = READ_STATUS_OK;

  while (TRUE)
    {
      ssize_t chunk;
      size_t to_read;

      to_read = sizeof (int) * n_ints_in_buf - bytes;

      if (to_read == 0)
        break;

    again:

      chunk = read (fd,
                    ((char*)buf) + bytes,
                    to_read);

      if (chunk < 0 && errno == EINTR)
        goto again;

      if (chunk < 0)
        {
          dbus_set_error (error,
			  DBUS_ERROR_SPAWN_FAILED,
			  "Failed to read from child pipe (%s)",
			  _dbus_strerror (errno));

          retval = READ_STATUS_ERROR;
          break;
        }
      else if (chunk == 0)
        {
          retval = READ_STATUS_EOF;
          break; /* EOF */
        }
      else /* chunk > 0 */
	bytes += chunk;
    }

  *n_ints_read = (int)(bytes / sizeof(int));

  return retval;
}

static ReadStatus
read_pid (int        fd,
          pid_t     *buf,
          DBusError *error)
{
  size_t bytes = 0;
  ReadStatus retval;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  retval = READ_STATUS_OK;

  while (TRUE)
    {
      ssize_t chunk;
      size_t to_read;

      to_read = sizeof (pid_t) - bytes;

      if (to_read == 0)
        break;

    again:

      chunk = read (fd,
                    ((char*)buf) + bytes,
                    to_read);
      if (chunk < 0 && errno == EINTR)
        goto again;

      if (chunk < 0)
        {
          dbus_set_error (error,
			  DBUS_ERROR_SPAWN_FAILED,
			  "Failed to read from child pipe (%s)",
			  _dbus_strerror (errno));

          retval = READ_STATUS_ERROR;
          break;
        }
      else if (chunk == 0)
        {
          retval = READ_STATUS_EOF;
          break; /* EOF */
        }
      else /* chunk > 0 */
	bytes += chunk;
    }

  return retval;
}

/* The implementation uses an intermediate child between the main process
 * and the grandchild. The grandchild is our spawned process. The intermediate
 * child is a babysitter process; it keeps track of when the grandchild
 * exits/crashes, and reaps the grandchild.
 *
 * We automatically reap the babysitter process, killing it if necessary,
 * when the DBusBabysitter's refcount goes to zero.
 *
 * Processes:
 *
 * main process
 * | fork() A
 * \- babysitter
 *    | fork () B
 *    \- grandchild     --> exec -->    spawned process
 *
 * IPC:
 *                  child_err_report_pipe
 *          /-----------<---------<--------------\
 *          |                                    ^
 *          v                                    |
 * main process           babysitter          grandchild
 *          ^                 ^
 *          v                 v
 *          \-------<->-------/
 *            babysitter_pipe
 *
 * child_err_report_pipe is genuinely a pipe.
 * The READ_END (also called error_pipe_from_child) is used in the main
 * process. The WRITE_END (also called child_err_report_fd) is used in
 * the grandchild process.
 *
 * On failure, the grandchild process sends CHILD_EXEC_FAILED + errno.
 * On success, the pipe just closes (because it's close-on-exec) without
 * sending any bytes.
 *
 * babysitter_pipe is mis-named: it's really a bidirectional socketpair.
 * The [0] end (also called socket_to_babysitter) is used in the main
 * process, the [1] end (also called parent_pipe) is used in the babysitter.
 *
 * If the fork() labelled B in the diagram above fails, the babysitter sends
 * CHILD_FORK_FAILED + errno.
 * On success, the babysitter sends CHILD_PID + the grandchild's pid.
 * On SIGCHLD, the babysitter sends CHILD_EXITED + the exit status.
 * The main process doesn't explicitly send anything, but when it exits,
 * the babysitter gets POLLHUP or POLLERR.
 */

/* Messages from children to parents */
enum
{
  CHILD_EXITED,            /* This message is followed by the exit status int */
  CHILD_FORK_FAILED,       /* Followed by errno */
  CHILD_EXEC_FAILED,       /* Followed by errno */
  CHILD_PID                /* Followed by pid_t */
};

/**
 * Babysitter implementation details
 */
struct DBusBabysitter
{
  int refcount; /**< Reference count */

  char *log_name; /**< the name under which to log messages about this
		    process being spawned */

  DBusSocket socket_to_babysitter; /**< Connection to the babysitter process */
  int error_pipe_from_child; /**< Connection to the process that does the exec() */

  pid_t sitter_pid;  /**< PID Of the babysitter */
  pid_t grandchild_pid; /**< PID of the grandchild */

  DBusWatchList *watches; /**< Watches */

  DBusWatch *error_watch; /**< Error pipe watch */
  DBusWatch *sitter_watch; /**< Sitter pipe watch */

  DBusBabysitterFinishedFunc finished_cb;
  void *finished_data;

  int errnum; /**< Error number */
  int status; /**< Exit status code */
  unsigned int have_child_status : 1; /**< True if child status has been reaped */
  unsigned int have_fork_errnum : 1; /**< True if we have an error code from fork() */
  unsigned int have_exec_errnum : 1; /**< True if we have an error code from exec() */
};

static DBusBabysitter*
_dbus_babysitter_new (void)
{
  DBusBabysitter *sitter;

  sitter = dbus_new0 (DBusBabysitter, 1);
  if (sitter == NULL)
    return NULL;

  sitter->refcount = 1;

  sitter->socket_to_babysitter.fd = -1;
  sitter->error_pipe_from_child = -1;

  sitter->sitter_pid = -1;
  sitter->grandchild_pid = -1;

  sitter->watches = _dbus_watch_list_new ();
  if (sitter->watches == NULL)
    goto failed;

  return sitter;

 failed:
  _dbus_babysitter_unref (sitter);
  return NULL;
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
  _dbus_assert (sitter != NULL);
  _dbus_assert (sitter->refcount > 0);

  sitter->refcount += 1;

  return sitter;
}

static void close_socket_to_babysitter  (DBusBabysitter *sitter);
static void close_error_pipe_from_child (DBusBabysitter *sitter);

/**
 * Decrement the reference count on the babysitter object.
 * When the reference count of the babysitter object reaches
 * zero, the babysitter is killed and the child that was being
 * babysat gets emancipated.
 *
 * @param sitter the babysitter
 */
void
_dbus_babysitter_unref (DBusBabysitter *sitter)
{
  _dbus_assert (sitter != NULL);
  _dbus_assert (sitter->refcount > 0);

  sitter->refcount -= 1;
  if (sitter->refcount == 0)
    {
      /* If we haven't forked other babysitters
       * since this babysitter and socket were
       * created then this close will cause the
       * babysitter to wake up from poll with
       * a hangup and then the babysitter will
       * quit itself.
       */
      close_socket_to_babysitter (sitter);

      close_error_pipe_from_child (sitter);

      if (sitter->sitter_pid > 0)
        {
          int status;
          int ret;

          /* It's possible the babysitter died on its own above
           * from the close, or was killed randomly
           * by some other process, so first try to reap it
           */
          ret = waitpid (sitter->sitter_pid, &status, WNOHANG);

          /* If we couldn't reap the child then kill it, and
           * try again
           */
          if (ret == 0)
            kill (sitter->sitter_pid, SIGKILL);

          if (ret == 0)
            {
              do
                {
                  ret = waitpid (sitter->sitter_pid, &status, 0);
                }
              while (_DBUS_UNLIKELY (ret < 0 && errno == EINTR));
            }

          if (ret < 0)
            {
              if (errno == ECHILD)
                _dbus_warn ("Babysitter process not available to be reaped; should not happen");
              else
                _dbus_warn ("Unexpected error %d in waitpid() for babysitter: %s",
                            errno, _dbus_strerror (errno));
            }
          else
            {
              _dbus_verbose ("Reaped %ld, waiting for babysitter %ld\n",
                             (long) ret, (long) sitter->sitter_pid);

              if (WIFEXITED (sitter->status))
                _dbus_verbose ("Babysitter exited with status %d\n",
                               WEXITSTATUS (sitter->status));
              else if (WIFSIGNALED (sitter->status))
                _dbus_verbose ("Babysitter received signal %d\n",
                               WTERMSIG (sitter->status));
              else
                _dbus_verbose ("Babysitter exited abnormally\n");
            }

          sitter->sitter_pid = -1;
        }

      if (sitter->watches)
        _dbus_watch_list_free (sitter->watches);

      dbus_free (sitter->log_name);

      dbus_free (sitter);
    }
}

static ReadStatus
read_data (DBusBabysitter *sitter,
           int             fd)
{
  int what;
  int got;
  DBusError error = DBUS_ERROR_INIT;
  ReadStatus r;

  r = read_ints (fd, &what, 1, &got, &error);

  switch (r)
    {
    case READ_STATUS_ERROR:
      _dbus_warn ("Failed to read data from fd %d: %s", fd, error.message);
      dbus_error_free (&error);
      return r;

    case READ_STATUS_EOF:
      return r;

    case READ_STATUS_OK:
      break;

    default:
      _dbus_assert_not_reached ("invalid ReadStatus");
      break;
    }

  if (got == 1)
    {
      switch (what)
        {
        case CHILD_EXITED:
        case CHILD_FORK_FAILED:
        case CHILD_EXEC_FAILED:
          {
            int arg;

            r = read_ints (fd, &arg, 1, &got, &error);

            switch (r)
              {
              case READ_STATUS_ERROR:
                _dbus_warn ("Failed to read arg from fd %d: %s", fd, error.message);
                dbus_error_free (&error);
                return r;
              case READ_STATUS_EOF:
                return r;
              case READ_STATUS_OK:
                break;
              default:
                _dbus_assert_not_reached ("invalid ReadStatus");
                break;
              }

            if (got == 1)
              {
                if (what == CHILD_EXITED)
                  {
                    /* Do not reset sitter->errnum to 0 here. We get here if
                     * the babysitter reports that the grandchild process has
                     * exited, and there are two ways that can happen:
                     *
                     * 1. grandchild successfully exec()s the desired process,
                     * but then the desired process exits or is terminated
                     * by a signal. The babysitter observes this and reports
                     * CHILD_EXITED.
                     *
                     * 2. grandchild fails to exec() the desired process,
                     * attempts to report the exec() failure (which
                     * we will receive as CHILD_EXEC_FAILED), and then
                     * exits itself (which will prompt the babysitter to
                     * send CHILD_EXITED). We want the CHILD_EXEC_FAILED
                     * to take precedence (and have its errno logged),
                     * which _dbus_babysitter_set_child_exit_error() does.
                     */
                    sitter->have_child_status = TRUE;
                    sitter->status = arg;
                    _dbus_verbose ("recorded child status exited = %d signaled = %d exitstatus = %d termsig = %d\n",
                                   WIFEXITED (sitter->status), WIFSIGNALED (sitter->status),
                                   WEXITSTATUS (sitter->status), WTERMSIG (sitter->status));
                  }
                else if (what == CHILD_FORK_FAILED)
                  {
                    sitter->have_fork_errnum = TRUE;
                    sitter->errnum = arg;
                    _dbus_verbose ("recorded fork errnum %d\n", sitter->errnum);
                  }
                else if (what == CHILD_EXEC_FAILED)
                  {
                    sitter->have_exec_errnum = TRUE;
                    sitter->errnum = arg;
                    _dbus_verbose ("recorded exec errnum %d\n", sitter->errnum);
                  }
              }
          }
          break;
        case CHILD_PID:
          {
            pid_t pid = -1;

            r = read_pid (fd, &pid, &error);

            switch (r)
              {
              case READ_STATUS_ERROR:
                _dbus_warn ("Failed to read PID from fd %d: %s", fd, error.message);
                dbus_error_free (&error);
                return r;
              case READ_STATUS_EOF:
                return r;
              case READ_STATUS_OK:
                break;
              default:
                _dbus_assert_not_reached ("invalid ReadStatus");
                break;
              }

            sitter->grandchild_pid = pid;

            _dbus_verbose ("recorded grandchild pid %d\n", sitter->grandchild_pid);
          }
          break;
        default:
          _dbus_warn ("Unknown message received from babysitter process");
          break;
        }
    }

  return r;
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

  if (sitter->socket_to_babysitter.fd >= 0)
    {
      _dbus_close_socket (sitter->socket_to_babysitter, NULL);
      sitter->socket_to_babysitter.fd = -1;
    }
}

static void
close_error_pipe_from_child (DBusBabysitter *sitter)
{
  _dbus_verbose ("Closing child error\n");

  if (sitter->error_watch != NULL)
    {
      _dbus_assert (sitter->watches != NULL);
      _dbus_watch_list_remove_watch (sitter->watches,  sitter->error_watch);
      _dbus_watch_invalidate (sitter->error_watch);
      _dbus_watch_unref (sitter->error_watch);
      sitter->error_watch = NULL;
    }

  if (sitter->error_pipe_from_child >= 0)
    {
      _dbus_close (sitter->error_pipe_from_child, NULL);
      sitter->error_pipe_from_child = -1;
    }
}

static void
handle_babysitter_socket (DBusBabysitter *sitter,
                          int             revents)
{
  /* Even if we have POLLHUP, we want to keep reading
   * data until POLLIN goes away; so this function only
   * looks at HUP/ERR if no IN is set.
   */
  if (revents & _DBUS_POLLIN)
    {
      _dbus_verbose ("Reading data from babysitter\n");
      if (read_data (sitter, sitter->socket_to_babysitter.fd) != READ_STATUS_OK)
        close_socket_to_babysitter (sitter);
    }
  else if (revents & (_DBUS_POLLERR | _DBUS_POLLHUP))
    {
      close_socket_to_babysitter (sitter);
    }
}

static void
handle_error_pipe (DBusBabysitter *sitter,
                   int             revents)
{
  if (revents & _DBUS_POLLIN)
    {
      _dbus_verbose ("Reading data from child error\n");
      if (read_data (sitter, sitter->error_pipe_from_child) != READ_STATUS_OK)
        close_error_pipe_from_child (sitter);
    }
  else if (revents & (_DBUS_POLLERR | _DBUS_POLLHUP))
    {
      close_error_pipe_from_child (sitter);
    }
}

/* returns whether there were any poll events handled */
static dbus_bool_t
babysitter_iteration (DBusBabysitter *sitter,
                      dbus_bool_t     block)
{
  DBusPollFD fds[2];
  int i;
  dbus_bool_t descriptors_ready;

  descriptors_ready = FALSE;

  i = 0;

  if (sitter->error_pipe_from_child >= 0)
    {
      fds[i].fd = sitter->error_pipe_from_child;
      fds[i].events = _DBUS_POLLIN;
      fds[i].revents = 0;
      ++i;
    }

  if (sitter->socket_to_babysitter.fd >= 0)
    {
      fds[i].fd = sitter->socket_to_babysitter.fd;
      fds[i].events = _DBUS_POLLIN;
      fds[i].revents = 0;
      ++i;
    }

  if (i > 0)
    {
      int ret;

      do
        {
          ret = _dbus_poll (fds, i, 0);
        }
      while (ret < 0 && errno == EINTR);

      if (ret == 0 && block)
        {
          do
            {
              ret = _dbus_poll (fds, i, -1);
            }
          while (ret < 0 && errno == EINTR);
        }

      if (ret > 0)
        {
          descriptors_ready = TRUE;

          while (i > 0)
            {
              --i;
              if (fds[i].fd == sitter->error_pipe_from_child)
                handle_error_pipe (sitter, fds[i].revents);
              else if (fds[i].fd == sitter->socket_to_babysitter.fd)
                handle_babysitter_socket (sitter, fds[i].revents);
            }
        }
    }

  return descriptors_ready;
}

/**
 * Macro returns #TRUE if the babysitter still has live sockets open to the
 * babysitter child or the grandchild.
 */
#define LIVE_CHILDREN(sitter) ((sitter)->socket_to_babysitter.fd >= 0 || (sitter)->error_pipe_from_child >= 0)

/**
 * Blocks until the babysitter process gives us the PID of the spawned grandchild,
 * then kills the spawned grandchild.
 *
 * @param sitter the babysitter object
 */
void
_dbus_babysitter_kill_child (DBusBabysitter *sitter)
{
  /* be sure we have the PID of the child */
  while (LIVE_CHILDREN (sitter) &&
         sitter->grandchild_pid == -1)
    babysitter_iteration (sitter, TRUE);

  _dbus_verbose ("Got child PID %ld for killing\n",
                 (long) sitter->grandchild_pid);

  if (sitter->grandchild_pid == -1)
    return; /* child is already dead, or we're so hosed we'll never recover */

  kill (sitter->grandchild_pid, SIGKILL);
}

/**
 * Checks whether the child has exited, without blocking.
 *
 * @param sitter the babysitter
 */
dbus_bool_t
_dbus_babysitter_get_child_exited (DBusBabysitter *sitter)
{

  /* Be sure we're up-to-date */
  while (LIVE_CHILDREN (sitter) &&
         babysitter_iteration (sitter, FALSE))
    ;

  /* We will have exited the babysitter when the child has exited */
  return sitter->socket_to_babysitter.fd < 0;
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
      !(WIFEXITED (sitter->status)))
    return FALSE;

  *status = WEXITSTATUS (sitter->status);
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
  if (!_dbus_babysitter_get_child_exited (sitter))
    return;

  /* Note that if exec fails, we will also get a child status
   * from the babysitter saying the child exited,
   * so we need to give priority to the exec error
   */
  if (sitter->have_exec_errnum)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                      "Failed to execute program %s: %s",
                      sitter->log_name, _dbus_strerror (sitter->errnum));
    }
  else if (sitter->have_fork_errnum)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Failed to fork a new process %s: %s",
                      sitter->log_name, _dbus_strerror (sitter->errnum));
    }
  else if (sitter->have_child_status)
    {
      if (WIFEXITED (sitter->status))
        dbus_set_error (error, DBUS_ERROR_SPAWN_CHILD_EXITED,
                        "Process %s exited with status %d",
                        sitter->log_name, WEXITSTATUS (sitter->status));
      else if (WIFSIGNALED (sitter->status))
        dbus_set_error (error, DBUS_ERROR_SPAWN_CHILD_SIGNALED,
                        "Process %s received signal %d",
                        sitter->log_name, WTERMSIG (sitter->status));
      else
        dbus_set_error (error, DBUS_ERROR_FAILED,
                        "Process %s exited abnormally",
                        sitter->log_name);
    }
  else
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Process %s exited, reason unknown",
                      sitter->log_name);
    }
}

/**
 * Sets watch functions to notify us when the
 * babysitter object needs to read/write file descriptors.
 *
 * @param sitter the babysitter
 * @param add_function function to begin monitoring a new descriptor.
 * @param remove_function function to stop monitoring a descriptor.
 * @param toggled_function function to notify when the watch is enabled/disabled
 * @param data data to pass to add_function and remove_function.
 * @param free_data_function function to be called to free the data.
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
_dbus_babysitter_set_watch_functions (DBusBabysitter            *sitter,
                                      DBusAddWatchFunction       add_function,
                                      DBusRemoveWatchFunction    remove_function,
                                      DBusWatchToggledFunction   toggled_function,
                                      void                      *data,
                                      DBusFreeFunction           free_data_function)
{
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
  DBusBabysitter *sitter = _dbus_babysitter_ref (data);
  int revents;
  int fd;

  revents = 0;
  if (condition & DBUS_WATCH_READABLE)
    revents |= _DBUS_POLLIN;
  if (condition & DBUS_WATCH_ERROR)
    revents |= _DBUS_POLLERR;
  if (condition & DBUS_WATCH_HANGUP)
    revents |= _DBUS_POLLHUP;

  fd = dbus_watch_get_socket (watch);

  if (fd == sitter->error_pipe_from_child)
    handle_error_pipe (sitter, revents);
  else if (fd == sitter->socket_to_babysitter.fd)
    handle_babysitter_socket (sitter, revents);

  while (LIVE_CHILDREN (sitter) &&
         babysitter_iteration (sitter, FALSE))
    ;

  /* fd.o #32992: if the handle_* methods closed their sockets, they previously
   * didn't always remove the watches. Check that we don't regress. */
  _dbus_assert (sitter->socket_to_babysitter.fd != -1 || sitter->sitter_watch == NULL);
  _dbus_assert (sitter->error_pipe_from_child != -1 || sitter->error_watch == NULL);

  if (_dbus_babysitter_get_child_exited (sitter) &&
      sitter->finished_cb != NULL)
    {
      sitter->finished_cb (sitter, sitter->finished_data);
      sitter->finished_cb = NULL;
    }

  _dbus_babysitter_unref (sitter);
  return TRUE;
}

/** Helps remember which end of the pipe is which */
#define READ_END 0
/** Helps remember which end of the pipe is which */
#define WRITE_END 1


/* Avoids a danger in re-entrant situations (calling close()
 * on a file descriptor twice, and another module has
 * re-opened it since the first close).
 *
 * This previously claimed to be relevant for threaded situations, but by
 * trivial inspection, it is not thread-safe. It doesn't actually
 * matter, since this module is only used in the -util variant of the
 * library, which is only used in single-threaded situations.
 */
static int
close_and_invalidate (int *fd)
{
  int ret;

  if (*fd < 0)
    return -1;
  else
    {
      ret = _dbus_close (*fd, NULL);
      *fd = -1;
    }

  return ret;
}

static dbus_bool_t
make_pipe (int         p[2],
           DBusError  *error)
{
  int retval;

#ifdef HAVE_PIPE2
  dbus_bool_t cloexec_done;

  retval = pipe2 (p, O_CLOEXEC);
  cloexec_done = retval >= 0;

  /* Check if kernel seems to be too old to know pipe2(). We assume
     that if pipe2 is available, O_CLOEXEC is too.  */
  if (retval < 0 && errno == ENOSYS)
#endif
    {
      retval = pipe(p);
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (retval < 0)
    {
      dbus_set_error (error,
		      DBUS_ERROR_SPAWN_FAILED,
		      "Failed to create pipe for communicating with child process (%s)",
		      _dbus_strerror (errno));
      return FALSE;
    }

#ifdef HAVE_PIPE2
  if (!cloexec_done)
#endif
    {
      _dbus_fd_set_close_on_exec (p[0]);
      _dbus_fd_set_close_on_exec (p[1]);
    }

  return TRUE;
}

static void
do_write (int fd, const void *buf, size_t count)
{
  size_t bytes_written;
  int ret;

  bytes_written = 0;

 again:

  ret = write (fd, ((const char*)buf) + bytes_written, count - bytes_written);

  if (ret < 0)
    {
      if (errno == EINTR)
        goto again;
      else
        {
          _dbus_warn ("Failed to write data to pipe!");
          exit (1); /* give up, we suck */
        }
    }
  else
    bytes_written += ret;

  if (bytes_written < count)
    goto again;
}

static void write_err_and_exit (int fd, int msg) _DBUS_GNUC_NORETURN;

static void
write_err_and_exit (int fd, int msg)
{
  int en = errno;

  do_write (fd, &msg, sizeof (msg));
  do_write (fd, &en, sizeof (en));

  exit (1);
}

static void
write_pid (int fd, pid_t pid)
{
  int msg = CHILD_PID;

  do_write (fd, &msg, sizeof (msg));
  do_write (fd, &pid, sizeof (pid));
}

static void write_status_and_exit (int fd, int status) _DBUS_GNUC_NORETURN;

static void
write_status_and_exit (int fd, int status)
{
  int msg = CHILD_EXITED;

  do_write (fd, &msg, sizeof (msg));
  do_write (fd, &status, sizeof (status));

  exit (0);
}

static void do_exec (int                       child_err_report_fd,
                     char             * const *argv,
                     char             * const *envp,
                     DBusSpawnChildSetupFunc   child_setup,
                     void                     *user_data) _DBUS_GNUC_NORETURN;

static void
do_exec (int                       child_err_report_fd,
	 char             * const *argv,
	 char             * const *envp,
	 DBusSpawnChildSetupFunc   child_setup,
	 void                     *user_data)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  int i, max_open;
#endif

  _dbus_verbose_reset ();
  _dbus_verbose ("Child process has PID " DBUS_PID_FORMAT "\n",
                 _dbus_getpid ());

  if (child_setup)
    (* child_setup) (user_data);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  max_open = sysconf (_SC_OPEN_MAX);

  for (i = 3; i < max_open; i++)
    {
      int retval;

      if (i == child_err_report_fd)
        continue;

      retval = fcntl (i, F_GETFD);

      if (retval != -1 && !(retval & FD_CLOEXEC))
        {
          char description[256] = { 0 };
          char proc_self_fd[256] = { 0 };
          size_t description_length = sizeof (description) - 1;

          snprintf (proc_self_fd, sizeof (proc_self_fd) - 1,
                    "/proc/self/fd/%d", i);
          proc_self_fd[sizeof (proc_self_fd) - 1] = '\0';

          if (readlink (proc_self_fd, description, description_length) <= 0)
            snprintf (description, sizeof (description) - 1, "(unknown)");

          description[sizeof (description) - 1] = '\0';
          _dbus_warn ("Fd %d \"%s\" did not have the close-on-exec flag set!",
                      i, description);
        }
    }
#endif

  if (envp == NULL)
    {
      _dbus_assert (environ != NULL);

      envp = environ;
    }

  execve (argv[0], argv, envp);

  /* Exec failed */
  write_err_and_exit (child_err_report_fd,
                      CHILD_EXEC_FAILED);
}

static void
check_babysit_events (pid_t grandchild_pid,
                      int   parent_pipe,
                      int   revents)
{
  pid_t ret;
  int status;

  do
    {
      ret = waitpid (grandchild_pid, &status, WNOHANG);
      /* The man page says EINTR can't happen with WNOHANG,
       * but there are reports of it (maybe only with valgrind?)
       */
    }
  while (ret < 0 && errno == EINTR);

  if (ret == 0)
    {
      _dbus_verbose ("no child exited\n");

      ; /* no child exited */
    }
  else if (ret < 0)
    {
      /* This isn't supposed to happen. */
      _dbus_warn ("unexpected waitpid() failure in check_babysit_events(): %s",
                  _dbus_strerror (errno));
      exit (1);
    }
  else if (ret == grandchild_pid)
    {
      /* Child exited */
      _dbus_verbose ("reaped child pid %ld\n", (long) ret);

      write_status_and_exit (parent_pipe, status);
    }
  else
    {
      _dbus_warn ("waitpid() reaped pid %d that we've never heard of",
                  (int) ret);
      exit (1);
    }

  if (revents & _DBUS_POLLIN)
    {
      _dbus_verbose ("babysitter got POLLIN from parent pipe\n");
    }

  if (revents & (_DBUS_POLLERR | _DBUS_POLLHUP))
    {
      /* Parent is gone, so we just exit */
      _dbus_verbose ("babysitter got POLLERR or POLLHUP from parent\n");
      exit (0);
    }
}

/* Only used in a single-threaded child process, does not need to be
 * thread-safe */
static int babysit_sigchld_pipe = -1;

static void
babysit_signal_handler (int signo)
{
  /* Signal handlers that might set errno must save and restore the errno
   * that the interrupted function might have been relying on. */
  int saved_errno = errno;
  char b = '\0';

 again:
  if (write (babysit_sigchld_pipe, &b, 1) <= 0)
    if (errno == EINTR)
      goto again;

  errno = saved_errno;
}

static void babysit (pid_t grandchild_pid,
                     int   parent_pipe) _DBUS_GNUC_NORETURN;

static void
babysit (pid_t grandchild_pid,
         int   parent_pipe)
{
  int sigchld_pipe[2];

  /* We don't exec, so we keep parent state, such as the pid that
   * _dbus_verbose() uses. Reset the pid here.
   */
  _dbus_verbose_reset ();

  /* I thought SIGCHLD would just wake up the poll, but
   * that didn't seem to work, so added this pipe.
   * Probably the pipe is more likely to work on busted
   * operating systems anyhow.
   */
  if (pipe (sigchld_pipe) < 0)
    {
      _dbus_warn ("Not enough file descriptors to create pipe in babysitter process");
      exit (1);
    }

  babysit_sigchld_pipe = sigchld_pipe[WRITE_END];

  _dbus_set_signal_handler (SIGCHLD, babysit_signal_handler);

  write_pid (parent_pipe, grandchild_pid);

  check_babysit_events (grandchild_pid, parent_pipe, 0);

  while (TRUE)
    {
      DBusPollFD pfds[2];

      pfds[0].fd = parent_pipe;
      pfds[0].events = _DBUS_POLLIN;
      pfds[0].revents = 0;

      pfds[1].fd = sigchld_pipe[READ_END];
      pfds[1].events = _DBUS_POLLIN;
      pfds[1].revents = 0;

      if (_dbus_poll (pfds, _DBUS_N_ELEMENTS (pfds), -1) < 0 && errno != EINTR)
        {
          _dbus_warn ("_dbus_poll() error: %s", strerror (errno));
          exit (1);
        }

      if (pfds[0].revents != 0)
        {
          check_babysit_events (grandchild_pid, parent_pipe, pfds[0].revents);
        }
      else if (pfds[1].revents & _DBUS_POLLIN)
        {
          char b;
          if (read (sigchld_pipe[READ_END], &b, 1) == -1)
            {
              /* ignore */
            }
          /* do waitpid check */
          check_babysit_events (grandchild_pid, parent_pipe, 0);
        }
    }

  exit (1);
}

/**
 * Spawns a new process.
 *
 * On Unix platforms, the child_setup function is passed the given
 * user_data and is run in the child after fork() but before calling exec().
 * This can be used to change uid, resource limits and so on.
 * On Windows, this functionality does not fit the multi-processing model
 * (Windows does the equivalent of fork() and exec() in a single API call),
 * and the child_setup function and its user_data are ignored.
 *
 * Also creates a "babysitter" which tracks the status of the
 * child process, advising the parent if the child exits.
 * If the spawn fails, no babysitter is created.
 * If sitter_p is #NULL, no babysitter is kept.
 *
 * @param sitter_p return location for babysitter or #NULL
 * @param log_name the name under which to log messages about this process being spawned
 * @param argv the executable and arguments
 * @param env the environment, or #NULL to copy the parent's
 * @param child_setup function to call in child pre-exec()
 * @param user_data user data for setup function
 * @param error error object to be filled in if function fails
 * @returns #TRUE on success, #FALSE if error is filled in
 */
dbus_bool_t
_dbus_spawn_async_with_babysitter (DBusBabysitter          **sitter_p,
                                   const char               *log_name,
                                   char             * const *argv,
                                   char             * const *env,
                                   DBusSpawnFlags            flags,
                                   DBusSpawnChildSetupFunc   child_setup,
                                   void                     *user_data,
                                   DBusError                *error)
{
  DBusBabysitter *sitter;
  int child_err_report_pipe[2] = { -1, -1 };
  DBusSocket babysitter_pipe[2] = { DBUS_SOCKET_INIT, DBUS_SOCKET_INIT };
  pid_t pid;
  int fd_out = -1;
  int fd_err = -1;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  _dbus_assert (argv[0] != NULL);

  if (sitter_p != NULL)
    *sitter_p = NULL;

  sitter = NULL;

  sitter = _dbus_babysitter_new ();
  if (sitter == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  sitter->log_name = _dbus_strdup (log_name);
  if (sitter->log_name == NULL && log_name != NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  if (sitter->log_name == NULL)
    sitter->log_name = _dbus_strdup (argv[0]);

  if (sitter->log_name == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  if (!make_pipe (child_err_report_pipe, error))
    goto cleanup_and_fail;

  if (!_dbus_socketpair (&babysitter_pipe[0], &babysitter_pipe[1], TRUE, error))
    goto cleanup_and_fail;

  /* Setting up the babysitter is only useful in the parent,
   * but we don't want to run out of memory and fail
   * after we've already forked, since then we'd leak
   * child processes everywhere.
   */
  sitter->error_watch = _dbus_watch_new (child_err_report_pipe[READ_END],
                                         DBUS_WATCH_READABLE,
                                         TRUE, handle_watch, sitter, NULL);
  if (sitter->error_watch == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  if (!_dbus_watch_list_add_watch (sitter->watches,  sitter->error_watch))
    {
      /* we need to free it early so the destructor won't try to remove it
       * without it having been added, which DBusLoop doesn't allow */
      _dbus_watch_invalidate (sitter->error_watch);
      _dbus_watch_unref (sitter->error_watch);
      sitter->error_watch = NULL;

      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  sitter->sitter_watch = _dbus_watch_new (babysitter_pipe[0].fd,
                                          DBUS_WATCH_READABLE,
                                          TRUE, handle_watch, sitter, NULL);
  if (sitter->sitter_watch == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  if (!_dbus_watch_list_add_watch (sitter->watches,  sitter->sitter_watch))
    {
      /* we need to free it early so the destructor won't try to remove it
       * without it having been added, which DBusLoop doesn't allow */
      _dbus_watch_invalidate (sitter->sitter_watch);
      _dbus_watch_unref (sitter->sitter_watch);
      sitter->sitter_watch = NULL;

      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto cleanup_and_fail;
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (flags & DBUS_SPAWN_SILENCE_OUTPUT)
    {
      fd_out = open ("/dev/null", O_RDONLY);

      if (fd_out < 0)
        {
          dbus_set_error (error, _dbus_error_from_errno (errno),
                          "Failed to open /dev/null: %s",
                          _dbus_strerror (errno));
          goto cleanup_and_fail;
        }

      _dbus_fd_set_close_on_exec (fd_out);

      fd_err = _dbus_dup (fd_out, error);

      if (fd_err < 0)
        goto cleanup_and_fail;
    }
#ifdef HAVE_SYSTEMD
  else if (flags & DBUS_SPAWN_REDIRECT_OUTPUT)
    {
      /* This may fail, but it's not critical.
       * In particular, if we were compiled with journald support but are now
       * running on a non-systemd system, this is going to fail, so we
       * have to cope gracefully. */
      fd_out = sd_journal_stream_fd (sitter->log_name, LOG_INFO, FALSE);
      fd_err = sd_journal_stream_fd (sitter->log_name, LOG_WARNING, FALSE);
    }
#endif

  /* Make sure our output buffers aren't redundantly printed by both the
   * parent and the child */
  fflush (stdout);
  fflush (stderr);

  pid = fork ();

  if (pid < 0)
    {
      dbus_set_error (error,
		      DBUS_ERROR_SPAWN_FORK_FAILED,
		      "Failed to fork (%s)",
		      _dbus_strerror (errno));
      goto cleanup_and_fail;
    }
  else if (pid == 0)
    {
      /* Immediate child, this is the babysitter process. */
      int grandchild_pid;

      /* Be sure we crash if the parent exits
       * and we write to the err_report_pipe
       */
      signal (SIGPIPE, SIG_DFL);

      /* Close the parent's end of the pipes. */
      close_and_invalidate (&child_err_report_pipe[READ_END]);
      close_and_invalidate (&babysitter_pipe[0].fd);

      fflush (stdout);
      fflush (stderr);

      /* Create the child that will exec () */
      grandchild_pid = fork ();

      if (grandchild_pid < 0)
	{
	  write_err_and_exit (babysitter_pipe[1].fd,
			      CHILD_FORK_FAILED);
          _dbus_assert_not_reached ("Got to code after write_err_and_exit()");
	}
      else if (grandchild_pid == 0)
        {
          /* This might not succeed in a dbus-daemon that started as root
           * and dropped privileges, so don't log an error on failure.
           * (Also, we can't safely log errors here anyway, because logging
           * is not async-signal safe). */
          _dbus_reset_oom_score_adj (NULL);

          /* Go back to ignoring SIGPIPE, since it's evil
           */
          signal (SIGPIPE, SIG_IGN);

          close_and_invalidate (&babysitter_pipe[1].fd);

          /* Redirect stdout, stderr to systemd Journal or /dev/null
           * as requested, if possible */
	  if (fd_out >= 0)
            dup2 (fd_out, STDOUT_FILENO);
	  if (fd_err >= 0)
            dup2 (fd_err, STDERR_FILENO);
          close_and_invalidate (&fd_out);
          close_and_invalidate (&fd_err);

	  do_exec (child_err_report_pipe[WRITE_END],
		   argv,
		   env,
		   child_setup, user_data);
          _dbus_assert_not_reached ("Got to code after exec() - should have exited on error");
	}
      else
	{
          close_and_invalidate (&child_err_report_pipe[WRITE_END]);
          close_and_invalidate (&fd_out);
          close_and_invalidate (&fd_err);
          babysit (grandchild_pid, babysitter_pipe[1].fd);
          _dbus_assert_not_reached ("Got to code after babysit()");
	}
    }
  else
    {
      /* Close the uncared-about ends of the pipes */
      close_and_invalidate (&child_err_report_pipe[WRITE_END]);
      close_and_invalidate (&babysitter_pipe[1].fd);
      close_and_invalidate (&fd_out);
      close_and_invalidate (&fd_err);

      sitter->socket_to_babysitter = babysitter_pipe[0];
      babysitter_pipe[0].fd = -1;

      sitter->error_pipe_from_child = child_err_report_pipe[READ_END];
      child_err_report_pipe[READ_END] = -1;

      sitter->sitter_pid = pid;

      if (sitter_p != NULL)
        *sitter_p = sitter;
      else
        _dbus_babysitter_unref (sitter);

      _DBUS_ASSERT_ERROR_IS_CLEAR (error);

      return TRUE;
    }

 cleanup_and_fail:

  _DBUS_ASSERT_ERROR_IS_SET (error);

  close_and_invalidate (&child_err_report_pipe[READ_END]);
  close_and_invalidate (&child_err_report_pipe[WRITE_END]);
  close_and_invalidate (&babysitter_pipe[0].fd);
  close_and_invalidate (&babysitter_pipe[1].fd);
  close_and_invalidate (&fd_out);
  close_and_invalidate (&fd_err);

  if (sitter != NULL)
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

/** @} */

void
_dbus_babysitter_block_for_child_exit (DBusBabysitter *sitter)
{
  while (LIVE_CHILDREN (sitter))
    babysitter_iteration (sitter, TRUE);
}
