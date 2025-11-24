/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-util-unix.c Would be in dbus-sysdeps-unix.c, but not used in libdbus
 *
 * Copyright (C) 2002, 2003, 2004, 2005  Red Hat, Inc.
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
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-unix.h"
#include "dbus-internals.h"
#include "dbus-list.h"
#include "dbus-pipe.h"
#include "dbus-protocol.h"
#include "dbus-string.h"
#define DBUS_USERDB_INCLUDES_PRIVATE 1
#include "dbus-userdb.h"
#include "dbus-test.h"

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sys/stat.h>
#ifdef HAVE_SYS_RESOURCE_H
#include <sys/resource.h>
#endif
#include <grp.h>
#include <sys/socket.h>
#include <dirent.h>
#include <sys/un.h>

#ifdef HAVE_SYS_PRCTL_H
#include <sys/prctl.h>
#endif

#ifdef HAVE_SYSTEMD
#include <systemd/sd-daemon.h>
#endif

#ifndef O_BINARY
#define O_BINARY 0
#endif

/**
 * @addtogroup DBusInternalsUtils
 * @{
 */


/**
 * Does the chdir, fork, setsid, etc. to become a daemon process.
 *
 * @param pidfile #NULL, or pidfile to create
 * @param print_pid_pipe pipe to print daemon's pid to, or -1 for none
 * @param error return location for errors
 * @param keep_umask #TRUE to keep the original umask
 * @returns #FALSE on failure
 */
dbus_bool_t
_dbus_become_daemon (const DBusString *pidfile,
                     DBusPipe         *print_pid_pipe,
                     DBusError        *error,
                     dbus_bool_t       keep_umask)
{
  const char *s;
  pid_t child_pid;
  DBusEnsureStandardFdsFlags flags;

  _dbus_verbose ("Becoming a daemon...\n");

  _dbus_verbose ("chdir to /\n");
  if (chdir ("/") < 0)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Could not chdir() to root directory");
      return FALSE;
    }

  _dbus_verbose ("forking...\n");

  /* Make sure our output buffers aren't redundantly printed by both the
   * parent and the child */
  fflush (stdout);
  fflush (stderr);

  switch ((child_pid = fork ()))
    {
    case -1:
      _dbus_verbose ("fork failed\n");
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to fork daemon: %s", _dbus_strerror (errno));
      return FALSE;
      break;

    case 0:
      _dbus_verbose ("in child, closing std file descriptors\n");

      flags = DBUS_FORCE_STDIN_NULL | DBUS_FORCE_STDOUT_NULL;
      s = _dbus_getenv ("DBUS_DEBUG_OUTPUT");

      if (s == NULL || *s == '\0')
        flags |= DBUS_FORCE_STDERR_NULL;
      else
        _dbus_verbose ("keeping stderr open due to DBUS_DEBUG_OUTPUT\n");

      if (!_dbus_ensure_standard_fds (flags, &s))
        {
          _dbus_warn ("%s: %s", s, _dbus_strerror (errno));
          _exit (1);
        }

      if (!keep_umask)
        {
          /* Get a predictable umask */
          _dbus_verbose ("setting umask\n");
          umask (022);
        }

      _dbus_verbose ("calling setsid()\n");
      if (setsid () == -1)
        _dbus_assert_not_reached ("setsid() failed");
      
      break;

    default:
      if (!_dbus_write_pid_to_file_and_pipe (pidfile, print_pid_pipe,
                                             child_pid, error))
        {
          _dbus_verbose ("pid file or pipe write failed: %s\n",
                         error->message);
          kill (child_pid, SIGTERM);
          return FALSE;
        }

      _dbus_verbose ("parent exiting\n");
      _exit (0);
      break;
    }
  
  return TRUE;
}


/**
 * Creates a file containing the process ID.
 *
 * @param filename the filename to write to
 * @param pid our process ID
 * @param error return location for errors
 * @returns #FALSE on failure
 */
static dbus_bool_t
_dbus_write_pid_file (const DBusString *filename,
                      unsigned long     pid,
		      DBusError        *error)
{
  const char *cfilename;
  int fd;
  FILE *f;

  cfilename = _dbus_string_get_const_data (filename);
  
  fd = open (cfilename, O_WRONLY|O_CREAT|O_EXCL|O_BINARY, 0644);
  
  if (fd < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to open \"%s\": %s", cfilename,
                      _dbus_strerror (errno));
      return FALSE;
    }

  if ((f = fdopen (fd, "w")) == NULL)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to fdopen fd %d: %s", fd, _dbus_strerror (errno));
      _dbus_close (fd, NULL);
      return FALSE;
    }
  
  if (fprintf (f, "%lu\n", pid) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to write to \"%s\": %s", cfilename,
                      _dbus_strerror (errno));
      
      fclose (f);
      return FALSE;
    }

  if (fclose (f) == EOF)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to close \"%s\": %s", cfilename,
                      _dbus_strerror (errno));
      return FALSE;
    }
  
  return TRUE;
}

/**
 * Writes the given pid_to_write to a pidfile (if non-NULL) and/or to a
 * pipe (if non-NULL). Does nothing if pidfile and print_pid_pipe are both
 * NULL.
 *
 * @param pidfile the file to write to or #NULL
 * @param print_pid_pipe the pipe to write to or #NULL
 * @param pid_to_write the pid to write out
 * @param error error on failure
 * @returns FALSE if error is set
 */
dbus_bool_t
_dbus_write_pid_to_file_and_pipe (const DBusString *pidfile,
                                  DBusPipe         *print_pid_pipe,
                                  dbus_pid_t        pid_to_write,
                                  DBusError        *error)
{
  if (pidfile)
    {
      _dbus_verbose ("writing pid file %s\n", _dbus_string_get_const_data (pidfile));
      if (!_dbus_write_pid_file (pidfile,
                                 pid_to_write,
                                 error))
        {
          _dbus_verbose ("pid file write failed\n");
          _DBUS_ASSERT_ERROR_IS_SET(error);
          return FALSE;
        }
    }
  else
    {
      _dbus_verbose ("No pid file requested\n");
    }

  if (print_pid_pipe != NULL && _dbus_pipe_is_valid (print_pid_pipe))
    {
      DBusString pid;
      int bytes;

      _dbus_verbose ("writing our pid to pipe %d\n",
                     print_pid_pipe->fd);
      
      if (!_dbus_string_init (&pid))
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }
	  
      if (!_dbus_string_append_int (&pid, pid_to_write) ||
          !_dbus_string_append (&pid, "\n"))
        {
          _dbus_string_free (&pid);
          _DBUS_SET_OOM (error);
          return FALSE;
        }
	  
      bytes = _dbus_string_get_length (&pid);
      if (_dbus_pipe_write (print_pid_pipe, &pid, 0, bytes, error) != bytes)
        {
          /* _dbus_pipe_write sets error only on failure, not short write */
          if (error != NULL && !dbus_error_is_set(error))
            {
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Printing message bus PID: did not write enough bytes\n");
            }
          _dbus_string_free (&pid);
          return FALSE;
        }
	  
      _dbus_string_free (&pid);
    }
  else
    {
      _dbus_verbose ("No pid pipe to write to\n");
    }

  return TRUE;
}

/**
 * Verify that after the fork we can successfully change to this user.
 *
 * @param user the username given in the daemon configuration
 * @returns #TRUE if username is valid
 */
dbus_bool_t
_dbus_verify_daemon_user (const char *user)
{
  DBusString u;

  _dbus_string_init_const (&u, user);

  return _dbus_get_user_id_and_primary_group (&u, NULL, NULL);
}


/* The HAVE_LIBAUDIT case lives in selinux.c */
#ifndef HAVE_LIBAUDIT
/**
 * Changes the user and group the bus is running as.
 *
 * @param user the user to become
 * @param error return location for errors
 * @returns #FALSE on failure
 */
dbus_bool_t
_dbus_change_to_daemon_user  (const char    *user,
                              DBusError     *error)
{
  dbus_uid_t uid;
  dbus_gid_t gid;
  DBusString u;

  _dbus_string_init_const (&u, user);

  if (!_dbus_get_user_id_and_primary_group (&u, &uid, &gid))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "User '%s' does not appear to exist?",
                      user);
      return FALSE;
    }

  /* setgroups() only works if we are a privileged process,
   * so we don't return error on failure; the only possible
   * failure is that we don't have perms to do it.
   *
   * not sure this is right, maybe if setuid()
   * is going to work then setgroups() should also work.
   */
  if (setgroups (0, NULL) < 0)
    _dbus_warn ("Failed to drop supplementary groups: %s",
                _dbus_strerror (errno));

  /* Set GID first, or the setuid may remove our permission
   * to change the GID
   */
  if (setgid (gid) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to set GID to %lu: %s", gid,
                      _dbus_strerror (errno));
      return FALSE;
    }

  if (setuid (uid) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to set UID to %lu: %s", uid,
                      _dbus_strerror (errno));
      return FALSE;
    }

  return TRUE;
}
#endif /* !HAVE_LIBAUDIT */

#ifdef HAVE_SETRLIMIT

/* We assume that if we have setrlimit, we also have getrlimit and
 * struct rlimit.
 */

struct DBusRLimit {
    struct rlimit lim;
};

DBusRLimit *
_dbus_rlimit_save_fd_limit (DBusError *error)
{
  DBusRLimit *self;

  self = dbus_new0 (DBusRLimit, 1);

  if (self == NULL)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  if (getrlimit (RLIMIT_NOFILE, &self->lim) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to get fd limit: %s", _dbus_strerror (errno));
      dbus_free (self);
      return NULL;
    }

  return self;
}

/* Enough fds that we shouldn't run out, even if several uids work
 * together to carry out a denial-of-service attack. This happens to be
 * the same number that systemd < 234 would normally use. */
#define ENOUGH_FDS 65536

dbus_bool_t
_dbus_rlimit_raise_fd_limit (DBusError *error)
{
  struct rlimit old, lim;

  if (getrlimit (RLIMIT_NOFILE, &lim) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to get fd limit: %s", _dbus_strerror (errno));
      return FALSE;
    }

  old = lim;

  if (getuid () == 0)
    {
      /* We are privileged, so raise the soft limit to at least
       * ENOUGH_FDS, and the hard limit to at least the desired soft
       * limit. This assumes we can exercise CAP_SYS_RESOURCE on Linux,
       * or other OSs' equivalents. */
      if (lim.rlim_cur != RLIM_INFINITY &&
          lim.rlim_cur < ENOUGH_FDS)
        lim.rlim_cur = ENOUGH_FDS;

      if (lim.rlim_max != RLIM_INFINITY &&
          lim.rlim_max < lim.rlim_cur)
        lim.rlim_max = lim.rlim_cur;
    }

  /* Raise the soft limit to match the hard limit, which we can do even
   * if we are unprivileged. In particular, systemd >= 240 will normally
   * set rlim_cur to 1024 and rlim_max to 512*1024, recent Debian
   * versions end up setting rlim_cur to 1024 and rlim_max to 1024*1024,
   * and older and non-systemd Linux systems would typically set rlim_cur
   * to 1024 and rlim_max to 4096. */
  if (lim.rlim_max == RLIM_INFINITY || lim.rlim_cur < lim.rlim_max)
    {
#if defined(__APPLE__) && defined(__MACH__)
      /* macOS 10.5 and above no longer allows RLIM_INFINITY for rlim_cur */
      lim.rlim_cur = MIN (OPEN_MAX, lim.rlim_max);
#else
      lim.rlim_cur = lim.rlim_max;
#endif
    }

  /* Early-return if there is nothing to do. */
  if (lim.rlim_max == old.rlim_max &&
      lim.rlim_cur == old.rlim_cur)
    return TRUE;

  if (setrlimit (RLIMIT_NOFILE, &lim) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to set fd limit to %lu: %s",
                      (unsigned long) lim.rlim_cur,
                      _dbus_strerror (errno));
      return FALSE;
    }

  return TRUE;
}

dbus_bool_t
_dbus_rlimit_restore_fd_limit (DBusRLimit *saved,
                               DBusError  *error)
{
  if (setrlimit (RLIMIT_NOFILE, &saved->lim) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to restore old fd limit: %s",
                      _dbus_strerror (errno));
      return FALSE;
    }

  return TRUE;
}

#else /* !HAVE_SETRLIMIT */

static void
fd_limit_not_supported (DBusError *error)
{
  dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                  "cannot change fd limit on this platform");
}

DBusRLimit *
_dbus_rlimit_save_fd_limit (DBusError *error)
{
  fd_limit_not_supported (error);
  return NULL;
}

dbus_bool_t
_dbus_rlimit_raise_fd_limit (DBusError *error)
{
  fd_limit_not_supported (error);
  return FALSE;
}

dbus_bool_t
_dbus_rlimit_restore_fd_limit (DBusRLimit *saved,
                               DBusError  *error)
{
  fd_limit_not_supported (error);
  return FALSE;
}

#endif

void
_dbus_rlimit_free (DBusRLimit *lim)
{
  dbus_free (lim);
}

/** Installs a UNIX signal handler
 *
 * @param sig the signal to handle
 * @param handler the handler
 */
void
_dbus_set_signal_handler (int               sig,
                          DBusSignalHandler handler)
{
  struct sigaction act;
  sigset_t empty_mask;
  
  sigemptyset (&empty_mask);
  act.sa_handler = handler;
  act.sa_mask    = empty_mask;
  act.sa_flags   = 0;
  sigaction (sig,  &act, NULL);
}

/** Checks if a file exists
*
* @param file full path to the file
* @returns #TRUE if file exists
*/
dbus_bool_t 
_dbus_file_exists (const char *file)
{
  return (access (file, F_OK) == 0);
}

/** Checks if user is at the console
*
* @param username user to check
* @param error return location for errors
* @returns #TRUE is the user is at the consolei and there are no errors
*/
dbus_bool_t 
_dbus_user_at_console (const char *username,
                       DBusError  *error)
{
#ifdef DBUS_CONSOLE_AUTH_DIR
  DBusString u, f;
  dbus_bool_t result;

  result = FALSE;
  if (!_dbus_string_init (&f))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_append (&f, DBUS_CONSOLE_AUTH_DIR))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  _dbus_string_init_const (&u, username);

  if (!_dbus_concat_dir_and_file (&f, &u))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  result = _dbus_file_exists (_dbus_string_get_const_data (&f));

 out:
  _dbus_string_free (&f);

  return result;
#else
  return FALSE;
#endif
}


/**
 * Checks whether the filename is an absolute path
 *
 * @param filename the filename
 * @returns #TRUE if an absolute path
 */
dbus_bool_t
_dbus_path_is_absolute (const DBusString *filename)
{
  if (_dbus_string_get_length (filename) > 0)
    return _dbus_string_get_byte (filename, 0) == '/';
  else
    return FALSE;
}

/**
 * stat() wrapper.
 *
 * @param filename the filename to stat
 * @param statbuf the stat info to fill in
 * @param error return location for error
 * @returns #FALSE if error was set
 */
dbus_bool_t
_dbus_stat (const DBusString *filename,
            DBusStat         *statbuf,
            DBusError        *error)
{
  const char *filename_c;
  struct stat sb;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  filename_c = _dbus_string_get_const_data (filename);

  if (stat (filename_c, &sb) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "%s", _dbus_strerror (errno));
      return FALSE;
    }

  statbuf->mode = sb.st_mode;
  statbuf->nlink = sb.st_nlink;
  statbuf->uid = sb.st_uid;
  statbuf->gid = sb.st_gid;
  statbuf->size = sb.st_size;
  statbuf->atime = sb.st_atime;
  statbuf->mtime = sb.st_mtime;
  statbuf->ctime = sb.st_ctime;

  return TRUE;
}


/**
 * Internals of directory iterator
 */
struct DBusDirIter
{
  DIR *d; /**< The DIR* from opendir() */
  
};

/**
 * Open a directory to iterate over.
 *
 * @param filename the directory name
 * @param error exception return object or #NULL
 * @returns new iterator, or #NULL on error
 */
DBusDirIter*
_dbus_directory_open (const DBusString *filename,
                      DBusError        *error)
{
  DIR *d;
  DBusDirIter *iter;
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  filename_c = _dbus_string_get_const_data (filename);

  d = opendir (filename_c);
  if (d == NULL)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to read directory \"%s\": %s",
                      filename_c,
                      _dbus_strerror (errno));
      return NULL;
    }
  iter = dbus_new0 (DBusDirIter, 1);
  if (iter == NULL)
    {
      closedir (d);
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Could not allocate memory for directory iterator");
      return NULL;
    }

  iter->d = d;

  return iter;
}

/**
 * Get next file in the directory. Will not return "." or ".."  on
 * UNIX. If an error occurs, the contents of "filename" are
 * undefined. The error is never set if the function succeeds.
 *
 * This function is not re-entrant, and not necessarily thread-safe.
 * Only use it for test code or single-threaded utilities.
 *
 * @param iter the iterator
 * @param filename string to be set to the next file in the dir
 * @param error return location for error
 * @returns #TRUE if filename was filled in with a new filename
 */
dbus_bool_t
_dbus_directory_get_next_file (DBusDirIter      *iter,
                               DBusString       *filename,
                               DBusError        *error)
{
  struct dirent *ent;
  int err;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

 again:
  errno = 0;
  ent = readdir (iter->d);

  if (!ent)
    {
      err = errno;

      if (err != 0)
        dbus_set_error (error,
                        _dbus_error_from_errno (err),
                        "%s", _dbus_strerror (err));

      return FALSE;
    }
  else if (ent->d_name[0] == '.' &&
           (ent->d_name[1] == '\0' ||
            (ent->d_name[1] == '.' && ent->d_name[2] == '\0')))
    goto again;
  else
    {
      _dbus_string_set_length (filename, 0);
      if (!_dbus_string_append (filename, ent->d_name))
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                          "No memory to read directory entry");
          return FALSE;
        }
      else
        {
          return TRUE;
        }
    }
}

/**
 * Closes a directory iteration.
 */
void
_dbus_directory_close (DBusDirIter *iter)
{
  closedir (iter->d);
  dbus_free (iter);
}

static dbus_bool_t
fill_user_info_from_group (struct group  *g,
                           DBusGroupInfo *info,
                           DBusError     *error)
{
  _dbus_assert (g->gr_name != NULL);
  
  info->gid = g->gr_gid;
  info->groupname = _dbus_strdup (g->gr_name);

  /* info->members = dbus_strdupv (g->gr_mem) */
  
  if (info->groupname == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  return TRUE;
}

static dbus_bool_t
fill_group_info (DBusGroupInfo    *info,
                 dbus_gid_t        gid,
                 const DBusString *groupname,
                 DBusError        *error)
{
  const char *group_c_str;

  _dbus_assert (groupname != NULL || gid != DBUS_GID_UNSET);
  _dbus_assert (groupname == NULL || gid == DBUS_GID_UNSET);

  if (groupname)
    group_c_str = _dbus_string_get_const_data (groupname);
  else
    group_c_str = NULL;
  
  /* For now assuming that the getgrnam() and getgrgid() flavors
   * always correspond to the pwnam flavors, if not we have
   * to add more configure checks.
   */
  
#ifdef HAVE_GETPWNAM_R
  {
    struct group *g;
    int result;
    size_t buflen;
    char *buf;
    struct group g_str;
    dbus_bool_t b;

    /* retrieve maximum needed size for buf */
    buflen = sysconf (_SC_GETGR_R_SIZE_MAX);

    /* sysconf actually returns a long, but everything else expects size_t,
     * so just recast here.
     * https://bugs.freedesktop.org/show_bug.cgi?id=17061
     */
    if ((long) buflen <= 0)
      buflen = 1024;

    result = -1;
    while (1)
      {
        buf = dbus_malloc (buflen);
        if (buf == NULL)
          {
            dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
            return FALSE;
          }

        g = NULL;
        if (group_c_str)
          result = getgrnam_r (group_c_str, &g_str, buf, buflen,
                               &g);
        else
          result = getgrgid_r (gid, &g_str, buf, buflen,
                               &g);
        /* Try a bigger buffer if ERANGE was returned:
           https://bugs.freedesktop.org/show_bug.cgi?id=16727
        */
        if (result == ERANGE && buflen < 512 * 1024)
          {
            dbus_free (buf);
            buflen *= 2;
          }
        else
          {
            break;
          }
      }

    if (result == 0 && g == &g_str)
      {
        b = fill_user_info_from_group (g, info, error);
        dbus_free (buf);
        return b;
      }
    else
      {
        dbus_set_error (error, _dbus_error_from_errno (errno),
                        "Group %s unknown or failed to look it up\n",
                        group_c_str ? group_c_str : "???");
        dbus_free (buf);
        return FALSE;
      }
  }
#else /* ! HAVE_GETPWNAM_R */
  {
    /* I guess we're screwed on thread safety here */
    struct group *g;

#warning getpwnam_r() not available, please report this to the dbus maintainers with details of your OS

    g = getgrnam (group_c_str);

    if (g != NULL)
      {
        return fill_user_info_from_group (g, info, error);
      }
    else
      {
        dbus_set_error (error, _dbus_error_from_errno (errno),
                        "Group %s unknown or failed to look it up\n",
                        group_c_str ? group_c_str : "???");
        return FALSE;
      }
  }
#endif  /* ! HAVE_GETPWNAM_R */
}

/**
 * Initializes the given DBusGroupInfo struct
 * with information about the given group name.
 *
 * @param info the group info struct
 * @param groupname name of group
 * @param error the error return
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_group_info_fill (DBusGroupInfo    *info,
                       const DBusString *groupname,
                       DBusError        *error)
{
  return fill_group_info (info, DBUS_GID_UNSET,
                          groupname, error);

}

/**
 * Initializes the given DBusGroupInfo struct
 * with information about the given group ID.
 *
 * @param info the group info struct
 * @param gid group ID
 * @param error the error return
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_group_info_fill_gid (DBusGroupInfo *info,
                           dbus_gid_t     gid,
                           DBusError     *error)
{
  return fill_group_info (info, gid, NULL, error);
}

/**
 * Parse a UNIX user from the bus config file. On Windows, this should
 * simply always fail (just return #FALSE).
 *
 * @param username the username text
 * @param uid_p place to return the uid
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_parse_unix_user_from_config (const DBusString  *username,
                                   dbus_uid_t        *uid_p)
{
  return _dbus_get_user_id (username, uid_p);

}

/**
 * Parse a UNIX group from the bus config file. On Windows, this should
 * simply always fail (just return #FALSE).
 *
 * @param groupname the groupname text
 * @param gid_p place to return the gid
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_parse_unix_group_from_config (const DBusString  *groupname,
                                    dbus_gid_t        *gid_p)
{
  return _dbus_get_group_id (groupname, gid_p);
}

/**
 * Gets all groups corresponding to the given UNIX user ID. On UNIX,
 * just calls _dbus_groups_from_uid(). On Windows, should always
 * fail since we don't know any UNIX groups.
 *
 * @param uid the UID
 * @param group_ids return location for array of group IDs
 * @param n_group_ids return location for length of returned array
 * @returns #TRUE if the UID existed and we got some credentials
 */
dbus_bool_t
_dbus_unix_groups_from_uid (dbus_uid_t            uid,
                            dbus_gid_t          **group_ids,
                            int                  *n_group_ids)
{
  return _dbus_groups_from_uid (uid, group_ids, n_group_ids);
}

/**
 * Checks to see if the UNIX user ID is at the console.
 * Should always fail on Windows (set the error to
 * #DBUS_ERROR_NOT_SUPPORTED).
 *
 * @param uid UID of person to check 
 * @param error return location for errors
 * @returns #TRUE if the UID is the same as the console user and there are no errors
 */
dbus_bool_t
_dbus_unix_user_is_at_console (dbus_uid_t         uid,
                               DBusError         *error)
{
  return _dbus_is_console_user (uid, error);

}

/**
 * Checks to see if the UNIX user ID matches the UID of
 * the process. Should always return #FALSE on Windows.
 *
 * @param uid the UNIX user ID
 * @returns #TRUE if this uid owns the process.
 */
dbus_bool_t
_dbus_unix_user_is_process_owner (dbus_uid_t uid)
{
  return uid == _dbus_geteuid ();
}

/**
 * Checks to see if the Windows user SID matches the owner of
 * the process. Should always return #FALSE on UNIX.
 *
 * @param windows_sid the Windows user SID
 * @returns #TRUE if this user owns the process.
 */
dbus_bool_t
_dbus_windows_user_is_process_owner (const char *windows_sid)
{
  return FALSE;
}

/** @} */ /* End of DBusInternalsUtils functions */

/**
 * @addtogroup DBusString
 *
 * @{
 */
/**
 * Get the directory name from a complete filename
 * @param filename the filename
 * @param dirname string to append directory name to
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_string_get_dirname  (const DBusString *filename,
                           DBusString       *dirname)
{
  int sep;
  
  _dbus_assert (filename != dirname);
  _dbus_assert (filename != NULL);
  _dbus_assert (dirname != NULL);

  /* Ignore any separators on the end */
  sep = _dbus_string_get_length (filename);
  if (sep == 0)
    return _dbus_string_append (dirname, "."); /* empty string passed in */
    
  while (sep > 0 && _dbus_string_get_byte (filename, sep - 1) == '/')
    --sep;

  _dbus_assert (sep >= 0);
  
  if (sep == 0)
    return _dbus_string_append (dirname, "/");
  
  /* Now find the previous separator */
  _dbus_string_find_byte_backward (filename, sep, '/', &sep);
  if (sep < 0)
    return _dbus_string_append (dirname, ".");
  
  /* skip multiple separators */
  while (sep > 0 && _dbus_string_get_byte (filename, sep - 1) == '/')
    --sep;

  _dbus_assert (sep >= 0);
  
  if (sep == 0 &&
      _dbus_string_get_byte (filename, 0) == '/')
    return _dbus_string_append (dirname, "/");
  else
    return _dbus_string_copy_len (filename, 0, sep - 0,
                                  dirname, _dbus_string_get_length (dirname));
}
/** @} */ /* DBusString stuff */

static void
string_squash_nonprintable (DBusString *str)
{
  unsigned char *buf;
  int i, len; 
  
  buf = _dbus_string_get_udata (str);
  len = _dbus_string_get_length (str);

  /* /proc/$pid/cmdline is a sequence of \0-terminated words, but we
   * want a sequence of space-separated words, with no extra trailing
   * space:
   *     "/bin/sleep" "\0" "60" "\0"
   *  -> "/bin/sleep" "\0" "60"
   *  -> "/bin/sleep" " " "60"
   *
   * so chop off the trailing NUL before cleaning up unprintable
   * characters. */
  if (len > 0 && buf[len - 1] == '\0')
    {
      _dbus_string_shorten (str, 1);
      len--;
    }

  for (i = 0; i < len; i++)
    {
      unsigned char c = (unsigned char) buf[i];
      if (c == '\0')
        buf[i] = ' ';
      else if (c < 0x20 || c > 127)
        buf[i] = '?';
    }
}

/**
 * Get a printable string describing the command used to execute
 * the process with pid.  This string should only be used for
 * informative purposes such as logging; it may not be trusted.
 * 
 * The command is guaranteed to be printable ASCII and no longer
 * than max_len.
 * 
 * @param pid Process id
 * @param str Append command to this string
 * @param max_len Maximum length of returned command
 * @param error return location for errors
 * @returns #FALSE on error
 */
dbus_bool_t 
_dbus_command_for_pid (unsigned long  pid,
                       DBusString    *str,
                       int            max_len,
                       DBusError     *error)
{
  /* This is all Linux-specific for now */
  DBusString path;
  DBusString cmdline;
  int fd;
  
  if (!_dbus_string_init (&path)) 
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_string_init (&cmdline))
    {
      _DBUS_SET_OOM (error);
      _dbus_string_free (&path);
      return FALSE;
    }
  
  if (!_dbus_string_append_printf (&path, "/proc/%ld/cmdline", pid))
    goto oom;
  
  fd = open (_dbus_string_get_const_data (&path), O_RDONLY);
  if (fd < 0) 
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (errno),
                      "Failed to open \"%s\": %s",
                      _dbus_string_get_const_data (&path),
                      _dbus_strerror (errno));
      goto fail;
    }
  
  if (!_dbus_read (fd, &cmdline, max_len))
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (errno),
                      "Failed to read from \"%s\": %s",
                      _dbus_string_get_const_data (&path),
                      _dbus_strerror (errno));      
      _dbus_close (fd, NULL);
      goto fail;
    }
  
  if (!_dbus_close (fd, error))
    goto fail;
  
  string_squash_nonprintable (&cmdline);  

  if (!_dbus_string_copy (&cmdline, 0, str, _dbus_string_get_length (str)))
    goto oom;

  _dbus_string_free (&cmdline);  
  _dbus_string_free (&path);
  return TRUE;
oom:
  _DBUS_SET_OOM (error);
fail:
  _dbus_string_free (&cmdline);
  _dbus_string_free (&path);
  return FALSE;
}

/**
 * Replace the DBUS_PREFIX in the given path, in-place, by the
 * current D-Bus installation directory. On Unix this function
 * does nothing, successfully.
 *
 * @param path path to edit
 * @return #FALSE on OOM
 */
dbus_bool_t
_dbus_replace_install_prefix (DBusString *path)
{
  return TRUE;
}

static dbus_bool_t
ensure_owned_directory (const char *label,
                        const DBusString *string,
                        dbus_bool_t create,
                        DBusError *error)
{
  const char *dir = _dbus_string_get_const_data (string);
  struct stat buf;

  if (create && !_dbus_ensure_directory (string, error))
    return FALSE;

  /*
   * The stat()-based checks in this function are to protect against
   * mistakes, not malice. We are working in a directory that is meant
   * to be trusted; but if a user has used `su` or similar to escalate
   * their privileges without correctly clearing the environment, the
   * XDG_RUNTIME_DIR in the environment might still be the user's
   * and not root's. We don't want to write root-owned files into that
   * directory, so just warn and don't provide support for transient
   * services in that case.
   *
   * In particular, we use stat() and not lstat() so that if we later
   * decide to use a different directory name for transient services,
   * we can drop in a compatibility symlink without breaking older
   * libdbus.
   */

  if (stat (dir, &buf) != 0)
    {
      int saved_errno = errno;

      dbus_set_error (error, _dbus_error_from_errno (saved_errno),
                      "%s \"%s\" not available: %s", label, dir,
                      _dbus_strerror (saved_errno));
      return FALSE;
    }

  if (!S_ISDIR (buf.st_mode))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED, "%s \"%s\" is not a directory",
                      label, dir);
      return FALSE;
    }

  if (buf.st_uid != geteuid ())
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "%s \"%s\" is owned by uid %ld, not our uid %ld",
                      label, dir, (long) buf.st_uid, (long) geteuid ());
      return FALSE;
    }

  /* This is just because we have the stat() results already, so we might
   * as well check opportunistically. */
  if ((S_IWOTH | S_IWGRP) & buf.st_mode)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "%s \"%s\" can be written by others (mode 0%o)",
                      label, dir, buf.st_mode);
      return FALSE;
    }

  return TRUE;
}

#define DBUS_UNIX_STANDARD_SESSION_SERVICEDIR "/dbus-1/services"
#define DBUS_UNIX_STANDARD_SYSTEM_SERVICEDIR "/dbus-1/system-services"

/**
 * Returns the standard directories for a session bus to look for
 * transient service activation files.
 *
 * @param dirs the directory list we are returning
 * @returns #FALSE on error
 */
dbus_bool_t
_dbus_set_up_transient_session_servicedirs (DBusList  **dirs,
                                            DBusError  *error)
{
  const char *xdg_runtime_dir;
  DBusString services;
  DBusString dbus1;
  DBusString xrd;
  dbus_bool_t ret = FALSE;
  char *data = NULL;

  if (!_dbus_string_init (&dbus1))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_init (&services))
    {
      _dbus_string_free (&dbus1);
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_init (&xrd))
    {
      _dbus_string_free (&dbus1);
      _dbus_string_free (&services);
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  xdg_runtime_dir = _dbus_getenv ("XDG_RUNTIME_DIR");

  /* Not an error, we just can't have transient session services */
  if (xdg_runtime_dir == NULL)
    {
      _dbus_verbose ("XDG_RUNTIME_DIR is unset: transient session services "
                     "not available here\n");
      ret = TRUE;
      goto out;
    }

  if (!_dbus_string_append (&xrd, xdg_runtime_dir) ||
      !_dbus_string_append_printf (&dbus1, "%s/dbus-1",
                                   xdg_runtime_dir) ||
      !_dbus_string_append_printf (&services, "%s/dbus-1/services",
                                   xdg_runtime_dir))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  if (!ensure_owned_directory ("XDG_RUNTIME_DIR", &xrd, FALSE, error) ||
      !ensure_owned_directory ("XDG_RUNTIME_DIR subdirectory", &dbus1, TRUE,
                               error) ||
      !ensure_owned_directory ("XDG_RUNTIME_DIR subdirectory", &services,
                               TRUE, error))
    goto out;

  if (!_dbus_string_steal_data (&services, &data) ||
      !_dbus_list_append (dirs, data))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  _dbus_verbose ("Transient service directory is %s\n", data);
  /* Ownership was transferred to @dirs */
  data = NULL;
  ret = TRUE;

out:
  _dbus_string_free (&dbus1);
  _dbus_string_free (&services);
  _dbus_string_free (&xrd);
  dbus_free (data);
  return ret;
}

/**
 * Returns the standard directories for a session bus to look for service
 * activation files
 *
 * On UNIX this should be the standard xdg freedesktop.org data directories:
 *
 * XDG_DATA_HOME=${XDG_DATA_HOME-$HOME/.local/share}
 * XDG_DATA_DIRS=${XDG_DATA_DIRS-/usr/local/share:/usr/share}
 *
 * and
 *
 * DBUS_DATADIR
 *
 * @param dirs the directory list we are returning
 * @returns #FALSE on OOM
 */

dbus_bool_t
_dbus_get_standard_session_servicedirs (DBusList **dirs)
{
  const char *xdg_data_home;
  const char *xdg_data_dirs;
  DBusString servicedir_path;

  if (!_dbus_string_init (&servicedir_path))
    return FALSE;

  xdg_data_home = _dbus_getenv ("XDG_DATA_HOME");
  xdg_data_dirs = _dbus_getenv ("XDG_DATA_DIRS");

  if (xdg_data_home != NULL)
    {
      if (!_dbus_string_append (&servicedir_path, xdg_data_home))
        goto oom;
    }
  else
    {
      const DBusString *homedir;
      DBusString local_share;

      if (!_dbus_homedir_from_current_process (&homedir))
        goto oom;

      if (!_dbus_string_append (&servicedir_path, _dbus_string_get_const_data (homedir)))
        goto oom;

      _dbus_string_init_const (&local_share, "/.local/share");
      if (!_dbus_concat_dir_and_file (&servicedir_path, &local_share))
        goto oom;
    }

  if (!_dbus_string_append (&servicedir_path, ":"))
    goto oom;

  if (xdg_data_dirs != NULL)
    {
      if (!_dbus_string_append (&servicedir_path, xdg_data_dirs))
        goto oom;

      if (!_dbus_string_append (&servicedir_path, ":"))
        goto oom;
    }
  else
    {
      if (!_dbus_string_append (&servicedir_path, "/usr/local/share:/usr/share:"))
        goto oom;
    }

  /*
   * add configured datadir to defaults
   * this may be the same as an xdg dir
   * however the config parser should take
   * care of duplicates
   */
  if (!_dbus_string_append (&servicedir_path, DBUS_DATADIR))
    goto oom;

  if (!_dbus_split_paths_and_append (&servicedir_path,
                                     DBUS_UNIX_STANDARD_SESSION_SERVICEDIR,
                                     dirs))
    goto oom;

  _dbus_string_free (&servicedir_path);
  return TRUE;

 oom:
  _dbus_string_free (&servicedir_path);
  return FALSE;
}


/**
 * Returns the standard directories for a system bus to look for service
 * activation files
 *
 * On UNIX this should be the standard xdg freedesktop.org data directories:
 *
 * XDG_DATA_DIRS=${XDG_DATA_DIRS-/usr/local/share:/usr/share}
 *
 * and
 *
 * DBUS_DATADIR
 *
 * On Windows there is no system bus and this function can return nothing.
 *
 * @param dirs the directory list we are returning
 * @returns #FALSE on OOM
 */

dbus_bool_t
_dbus_get_standard_system_servicedirs (DBusList **dirs)
{
  /*
   * DBUS_DATADIR may be the same as one of the standard directories. However,
   * the config parser should take care of the duplicates.
   *
   * Also, append /lib as counterpart of /usr/share on the root
   * directory (the root directory does not know /share), in order to
   * facilitate early boot system bus activation where /usr might not
   * be available.
   */
  static const char standard_search_path[] =
    "/usr/local/share:"
    "/usr/share:"
    DBUS_DATADIR ":"
    "/lib";
  DBusString servicedir_path;

  _dbus_string_init_const (&servicedir_path, standard_search_path);

  return _dbus_split_paths_and_append (&servicedir_path,
                                       DBUS_UNIX_STANDARD_SYSTEM_SERVICEDIR,
                                       dirs);
}

/**
 * Get the absolute path of the system.conf file
 * (there is no system bus on Windows so this can just
 * return FALSE and print a warning or something)
 *
 * @param str the string to append to, which must be empty on entry
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_get_system_config_file (DBusString *str)
{
  _dbus_assert (_dbus_string_get_length (str) == 0);

  return _dbus_string_append (str, DBUS_SYSTEM_CONFIG_FILE);
}

/**
 * Get the absolute path of the session.conf file.
 *
 * @param str the string to append to, which must be empty on entry
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_get_session_config_file (DBusString *str)
{
  _dbus_assert (_dbus_string_get_length (str) == 0);

  return _dbus_string_append (str, DBUS_SESSION_CONFIG_FILE);
}

/**
 * Report to a service manager that the daemon calling this function is
 * ready for use. This is currently only implemented for systemd.
 */
void
_dbus_daemon_report_ready (void)
{
#ifdef HAVE_SYSTEMD
  sd_notify (0, "READY=1");
#endif
}

/**
 * Report to a service manager that the daemon calling this function is
 * reloading configuration. This is currently only implemented for systemd.
 */
void
_dbus_daemon_report_reloading (void)
{
#ifdef HAVE_SYSTEMD
  sd_notify (0, "RELOADING=1");
#endif
}

/**
 * Report to a service manager that the daemon calling this function is
 * reloading configuration. This is currently only implemented for systemd.
 */
void
_dbus_daemon_report_reloaded (void)
{
#ifdef HAVE_SYSTEMD
  /* For systemd, this is the same code */
  _dbus_daemon_report_ready ();
#endif
}

/**
 * Report to a service manager that the daemon calling this function is
 * shutting down. This is currently only implemented for systemd.
 */
void
_dbus_daemon_report_stopping (void)
{
#ifdef HAVE_SYSTEMD
  sd_notify (0, "STOPPING=1");
#endif
}

/**
 * If the current process has been protected from the Linux OOM killer
 * (the oom_score_adj process parameter is negative), reset it to the
 * default level of protection from the OOM killer (set oom_score_adj
 * to zero).
 *
 * This function does not use DBusError, to avoid calling malloc(), so
 * that it can be used in contexts where an async-signal-safe function
 * is required (for example after fork()). Instead, on failure it sets
 * errno and returns something like "Failed to open /dev/null" in
 * *error_str_p. Callers are expected to combine *error_str_p
 * with _dbus_strerror (errno) to get a full error report.
 */
dbus_bool_t
_dbus_reset_oom_score_adj (const char **error_str_p)
{
#ifdef __linux__
  int fd = -1;
  dbus_bool_t ret = FALSE;
  int saved_errno = 0;
  const char *error_str = NULL;

#ifdef O_CLOEXEC
  fd = open ("/proc/self/oom_score_adj", O_RDONLY | O_CLOEXEC);
#endif

  if (fd < 0)
    {
      fd = open ("/proc/self/oom_score_adj", O_RDONLY);
      if (fd >= 0)
        _dbus_fd_set_close_on_exec (fd);
    }

  if (fd >= 0)
    {
      ssize_t read_result = -1;
      /* It doesn't actually matter whether we read the whole file,
       * as long as we get the presence or absence of the minus sign */
      char first_char = '\0';

      read_result = read (fd, &first_char, 1);

      if (read_result < 0)
        {
          /* This probably can't actually happen in practice: if we can
           * open it, then we can hopefully read from it */
          ret = FALSE;
          error_str = "failed to read from /proc/self/oom_score_adj";
          saved_errno = errno;
          goto out;
        }

      /* If we are running with protection from the OOM killer
       * (typical for the system dbus-daemon under systemd), then
       * oom_score_adj will be negative. Drop that protection,
       * returning to oom_score_adj = 0.
       *
       * Conversely, if we are running with increased susceptibility
       * to the OOM killer (as user sessions typically do in
       * systemd >= 250), oom_score_adj will be strictly positive,
       * and we are not allowed to decrease it to 0 without privileges.
       *
       * If it's exactly 0 (typical for non-systemd systems, and
       * user processes on older systemd) then there's no need to
       * alter it.
       *
       * We shouldn't get an empty result, but if we do, assume it
       * means zero and don't try to change it. */
      if (read_result == 0 || first_char != '-')
        {
          /* Nothing needs to be done: the OOM score adjustment is
           * non-negative */
          ret = TRUE;
          goto out;
        }

      close (fd);
#ifdef O_CLOEXEC
      fd = open ("/proc/self/oom_score_adj", O_WRONLY | O_CLOEXEC);

      if (fd < 0)
#endif
        {
          fd = open ("/proc/self/oom_score_adj", O_WRONLY);
          if (fd >= 0)
            _dbus_fd_set_close_on_exec (fd);
        }

      if (fd < 0)
        {
          ret = FALSE;
          error_str = "open(/proc/self/oom_score_adj) for writing";
          saved_errno = errno;
          goto out;
        }

      if (pwrite (fd, "0", sizeof (char), 0) < 0)
        {
          ret = FALSE;
          error_str = "writing oom_score_adj error";
          saved_errno = errno;
          goto out;
        }

      /* Success */
      ret = TRUE;
    }
  else if (errno == ENOENT)
    {
      /* If /proc/self/oom_score_adj doesn't exist, assume the kernel
       * doesn't support this feature and ignore it. */
      ret = TRUE;
    }
  else
    {
      ret = FALSE;
      error_str = "open(/proc/self/oom_score_adj) for reading";
      saved_errno = errno;
      goto out;
    }

out:
  if (fd >= 0)
    _dbus_close (fd, NULL);

  if (error_str_p != NULL)
    *error_str_p = error_str;

  errno = saved_errno;
  return ret;
#else
  /* nothing to do on this platform */
  return TRUE;
#endif
}
