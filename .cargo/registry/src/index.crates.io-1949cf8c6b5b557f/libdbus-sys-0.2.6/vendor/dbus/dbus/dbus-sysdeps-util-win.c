/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-util.c Would be in dbus-sysdeps.c, but not used in libdbus
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

#define STRSAFE_NO_DEPRECATE

#include "dbus-sysdeps.h"
#include "dbus-internals.h"
#include "dbus-protocol.h"
#include "dbus-string.h"
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-win.h"
#include "dbus-sockets-win.h"
#include "dbus-memory.h"
#include "dbus-pipe.h"

#include <stdio.h>
#include <stdlib.h>
#if HAVE_ERRNO_H
#include <errno.h>
#endif
#include <winsock2.h>   // WSA error codes

#ifndef DBUS_WINCE
#include <io.h>
#include <lm.h>
#include <sys/stat.h>
#endif


/**
 * Does the chdir, fork, setsid, etc. to become a daemon process.
 *
 * @param pidfile #NULL, or pidfile to create
 * @param print_pid_pipe file descriptor to print daemon's pid to, or -1 for none
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
  dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                  "Cannot daemonize on Windows");
  return FALSE;
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
  HANDLE hnd;
  char pidstr[20];
  int total;
  int bytes_to_write;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  cfilename = _dbus_string_get_const_data (filename);

  hnd = CreateFileA (cfilename, GENERIC_WRITE,
                     FILE_SHARE_READ | FILE_SHARE_WRITE,
                     NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
                     INVALID_HANDLE_VALUE);
  if (hnd == INVALID_HANDLE_VALUE)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                      "Could not create PID file %s: %s",
                      cfilename, emsg);
      _dbus_win_free_error_string (emsg);
      return FALSE;
    }

  if (snprintf (pidstr, sizeof (pidstr), "%lu\n", pid) < 0)
    {
      dbus_set_error (error, _dbus_error_from_system_errno (),
                      "Failed to format PID for \"%s\": %s", cfilename,
                      _dbus_strerror_from_errno ());
      CloseHandle (hnd);
      return FALSE;
    }

  total = 0;
  bytes_to_write = strlen (pidstr);;

  while (total < bytes_to_write)
    {
      DWORD bytes_written;
      BOOL res;

      res = WriteFile (hnd, pidstr + total, bytes_to_write - total,
                       &bytes_written, NULL);

      if (res == 0 || bytes_written <= 0)
        {
          char *emsg = _dbus_win_error_string (GetLastError ());
          dbus_set_error (error, _dbus_win_error_from_last_error (),
                           "Could not write to %s: %s", cfilename, emsg);
          _dbus_win_free_error_string (emsg);
          CloseHandle (hnd);
          return FALSE;
        }

      total += bytes_written;
    }

  if (CloseHandle (hnd) == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not close file %s: %s",
                      cfilename, emsg);
      _dbus_win_free_error_string (emsg);

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

      _dbus_verbose ("writing our pid to pipe %d\n", print_pid_pipe->fd);

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
  return TRUE;
}

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
  return TRUE;
}

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

void
_dbus_rlimit_free (DBusRLimit *lim)
{
  /* _dbus_rlimit_save_fd_limit() cannot return non-NULL on Windows
   * so there cannot be anything to free */
  _dbus_assert (lim == NULL);
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
_dbus_stat(const DBusString *filename,
           DBusStat         *statbuf,
           DBusError        *error)
{
  const char *filename_c;
  WIN32_FILE_ATTRIBUTE_DATA wfad;
  char *lastdot;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  if (!GetFileAttributesExA (filename_c, GetFileExInfoStandard, &wfad))
    {
      _dbus_win_set_error_from_win_error (error, GetLastError ());
      return FALSE;
    }

  if (wfad.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
    statbuf->mode = _S_IFDIR;
  else
    statbuf->mode = _S_IFREG;

  statbuf->mode |= _S_IREAD;
  if (wfad.dwFileAttributes & FILE_ATTRIBUTE_READONLY)
    statbuf->mode |= _S_IWRITE;

  lastdot = strrchr (filename_c, '.');
  if (lastdot && stricmp (lastdot, ".exe") == 0)
    statbuf->mode |= _S_IEXEC;

  statbuf->mode |= (statbuf->mode & 0700) >> 3;
  statbuf->mode |= (statbuf->mode & 0700) >> 6;

  statbuf->nlink = 1;

#ifdef ENABLE_UID_TO_SID
  {
    PSID owner_sid, group_sid;
    PSECURITY_DESCRIPTOR sd;

    sd = NULL;
    rc = GetNamedSecurityInfo ((char *) filename_c, SE_FILE_OBJECT,
                               OWNER_SECURITY_INFORMATION |
                               GROUP_SECURITY_INFORMATION,
                               &owner_sid, &group_sid,
                               NULL, NULL,
                               &sd);
    if (rc != ERROR_SUCCESS)
      {
        _dbus_win_set_error_from_win_error (error, rc);
        if (sd != NULL)
          LocalFree (sd);
        return FALSE;
      }
    
    /* FIXME */
    statbuf->uid = _dbus_win_sid_to_uid_t (owner_sid);
    statbuf->gid = _dbus_win_sid_to_uid_t (group_sid);

    LocalFree (sd);
  }
#else
  statbuf->uid = DBUS_UID_UNSET;
  statbuf->gid = DBUS_GID_UNSET;
#endif

  statbuf->size = ((dbus_int64_t) wfad.nFileSizeHigh << 32) + wfad.nFileSizeLow;

  statbuf->atime =
    (((dbus_int64_t) wfad.ftLastAccessTime.dwHighDateTime << 32) +
     wfad.ftLastAccessTime.dwLowDateTime) / 10000000 - DBUS_INT64_CONSTANT (116444736000000000);

  statbuf->mtime =
    (((dbus_int64_t) wfad.ftLastWriteTime.dwHighDateTime << 32) +
     wfad.ftLastWriteTime.dwLowDateTime) / 10000000 - DBUS_INT64_CONSTANT (116444736000000000);

  statbuf->ctime =
    (((dbus_int64_t) wfad.ftCreationTime.dwHighDateTime << 32) +
     wfad.ftCreationTime.dwLowDateTime) / 10000000 - DBUS_INT64_CONSTANT (116444736000000000);

  return TRUE;
}

/**
 * Internals of directory iterator
 */
struct DBusDirIter
  {
    HANDLE handle;
    WIN32_FIND_DATAA fileinfo;  /* from FindFirst/FindNext */
    dbus_bool_t finished;       /* true if there are no more entries */
    int offset;
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
  DBusDirIter *iter;
  DBusString filespec;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init_from_string (&filespec, filename))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Could not allocate memory for directory filename copy");
      return NULL;
    }

  if (_dbus_string_ends_with_c_str (&filespec, "/") || _dbus_string_ends_with_c_str (&filespec, "\\") )
    {
      if (!_dbus_string_append (&filespec, "*"))
        {
          _dbus_string_free (&filespec);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                          "Could not append filename wildcard");
          return NULL;
        }
    }
  else if (!_dbus_string_ends_with_c_str (&filespec, "*"))
    {
      if (!_dbus_string_append (&filespec, "\\*"))
        {
          _dbus_string_free (&filespec);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                          "Could not append filename wildcard 2");
          return NULL;
        }
    }

  iter = dbus_new0 (DBusDirIter, 1);
  if (iter == NULL)
    {
      _dbus_string_free (&filespec);
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Could not allocate memory for directory iterator");
      return NULL;
    }

  iter->finished = FALSE;
  iter->offset = 0;
  iter->handle = FindFirstFileA (_dbus_string_get_const_data (&filespec), &(iter->fileinfo));
  if (iter->handle == INVALID_HANDLE_VALUE)
    {
      if (GetLastError () == ERROR_NO_MORE_FILES)
        iter->finished = TRUE;
      else
        {
          char *emsg = _dbus_win_error_string (GetLastError ());
          dbus_set_error (error, _dbus_win_error_from_last_error (),
                          "Failed to read directory \"%s\": %s",
                          _dbus_string_get_const_data (filename), emsg);
          _dbus_win_free_error_string (emsg);
          dbus_free (iter);
          _dbus_string_free (&filespec);
          return NULL;
        }
    }
  _dbus_string_free (&filespec);
  return iter;
}

/**
 * Get next file in the directory. Will not return "." or ".."  on
 * UNIX. If an error occurs, the contents of "filename" are
 * undefined. The error is never set if the function succeeds.
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
  int saved_err = GetLastError();

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

again:
  SetLastError (0);

  if (!iter || iter->finished)
      return FALSE;

  if (iter->offset > 0)
    {
      if (FindNextFileA (iter->handle, &(iter->fileinfo)) == 0)
        {
          if (GetLastError() == ERROR_NO_MORE_FILES)
            {
              SetLastError(saved_err);
              iter->finished = 1;
            }
          else
            {
              char *emsg = _dbus_win_error_string (GetLastError ());
              dbus_set_error (error, _dbus_win_error_from_last_error (),
                             "Failed to get next in directory: %s", emsg);
              _dbus_win_free_error_string (emsg);
              return FALSE;
            }
        }
    }

  iter->offset++;

  if (iter->finished)
      return FALSE;

  if (iter->fileinfo.cFileName[0] == '.' &&
     (iter->fileinfo.cFileName[1] == '\0' ||
        (iter->fileinfo.cFileName[1] == '.' && iter->fileinfo.cFileName[2] == '\0')))
      goto again;

  _dbus_string_set_length (filename, 0);
  if (!_dbus_string_append (filename, iter->fileinfo.cFileName))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "No memory to read directory entry");
      return FALSE;
    }

  return TRUE;
}

/**
 * Closes a directory iteration.
 */
void
_dbus_directory_close (DBusDirIter *iter)
{
  if (!iter)
      return;
  FindClose(iter->handle);
  dbus_free (iter);
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
_dbus_string_get_dirname(const DBusString *filename,
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

  while (sep > 0 &&
         (_dbus_string_get_byte (filename, sep - 1) == '/' ||
          _dbus_string_get_byte (filename, sep - 1) == '\\'))
    --sep;

  _dbus_assert (sep >= 0);

  if (sep == 0 ||
      (sep == 2 &&
       _dbus_string_get_byte (filename, 1) == ':' &&
       isalpha (_dbus_string_get_byte (filename, 0))))
    return _dbus_string_copy_len (filename, 0, sep + 1,
                                  dirname, _dbus_string_get_length (dirname));

  {
    int sep1, sep2;
    _dbus_string_find_byte_backward (filename, sep, '/', &sep1);
    _dbus_string_find_byte_backward (filename, sep, '\\', &sep2);

    sep = MAX (sep1, sep2);
  }
  if (sep < 0)
    return _dbus_string_append (dirname, ".");

  while (sep > 0 &&
         (_dbus_string_get_byte (filename, sep - 1) == '/' ||
          _dbus_string_get_byte (filename, sep - 1) == '\\'))
    --sep;

  _dbus_assert (sep >= 0);

  if ((sep == 0 ||
       (sep == 2 &&
        _dbus_string_get_byte (filename, 1) == ':' &&
        isalpha (_dbus_string_get_byte (filename, 0))))
      &&
      (_dbus_string_get_byte (filename, sep) == '/' ||
       _dbus_string_get_byte (filename, sep) == '\\'))
    return _dbus_string_copy_len (filename, 0, sep + 1,
                                  dirname, _dbus_string_get_length (dirname));
  else
    return _dbus_string_copy_len (filename, 0, sep - 0,
                                  dirname, _dbus_string_get_length (dirname));
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
  return FALSE;
}

dbus_bool_t _dbus_windows_user_is_process_owner (const char *windows_sid)
{
  return TRUE;
}

/*=====================================================================
  unix emulation functions - should be removed sometime in the future
 =====================================================================*/

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
  dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                  "UNIX user IDs not supported on Windows\n");
  return FALSE;
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
  return FALSE;
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
  return FALSE;
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
  return FALSE;
}



/** @} */ /* DBusString stuff */

/************************************************************************
 
 error handling
 
 ************************************************************************/





/* lan manager error codes */
const char*
_dbus_lm_strerror(int error_number)
{
#ifdef DBUS_WINCE
  // TODO
  return "unknown";
#else
  const char *msg;
  switch (error_number)
    {
    case NERR_NetNotStarted:
      return "The workstation driver is not installed.";
    case NERR_UnknownServer:
      return "The server could not be located.";
    case NERR_ShareMem:
      return "An internal error occurred. The network cannot access a shared memory segment.";
    case NERR_NoNetworkResource:
      return "A network resource shortage occurred.";
    case NERR_RemoteOnly:
      return "This operation is not supported on workstations.";
    case NERR_DevNotRedirected:
      return "The device is not connected.";
    case NERR_ServerNotStarted:
      return "The Server service is not started.";
    case NERR_ItemNotFound:
      return "The queue is empty.";
    case NERR_UnknownDevDir:
      return "The device or directory does not exist.";
    case NERR_RedirectedPath:
      return "The operation is invalid on a redirected resource.";
    case NERR_DuplicateShare:
      return "The name has already been shared.";
    case NERR_NoRoom:
      return "The server is currently out of the requested resource.";
    case NERR_TooManyItems:
      return "Requested addition of items exceeds the maximum allowed.";
    case NERR_InvalidMaxUsers:
      return "The Peer service supports only two simultaneous users.";
    case NERR_BufTooSmall:
      return "The API return buffer is too small.";
    case NERR_RemoteErr:
      return "A remote API error occurred.";
    case NERR_LanmanIniError:
      return "An error occurred when opening or reading the configuration file.";
    case NERR_NetworkError:
      return "A general network error occurred.";
    case NERR_WkstaInconsistentState:
      return "The Workstation service is in an inconsistent state. Restart the computer before restarting the Workstation service.";
    case NERR_WkstaNotStarted:
      return "The Workstation service has not been started.";
    case NERR_BrowserNotStarted:
      return "The requested information is not available.";
    case NERR_InternalError:
      return "An internal error occurred.";
    case NERR_BadTransactConfig:
      return "The server is not configured for transactions.";
    case NERR_InvalidAPI:
      return "The requested API is not supported on the remote server.";
    case NERR_BadEventName:
      return "The event name is invalid.";
    case NERR_DupNameReboot:
      return "The computer name already exists on the network. Change it and restart the computer.";
    case NERR_CfgCompNotFound:
      return "The specified component could not be found in the configuration information.";
    case NERR_CfgParamNotFound:
      return "The specified parameter could not be found in the configuration information.";
    case NERR_LineTooLong:
      return "A line in the configuration file is too long.";
    case NERR_QNotFound:
      return "The printer does not exist.";
    case NERR_JobNotFound:
      return "The print job does not exist.";
    case NERR_DestNotFound:
      return "The printer destination cannot be found.";
    case NERR_DestExists:
      return "The printer destination already exists.";
    case NERR_QExists:
      return "The printer queue already exists.";
    case NERR_QNoRoom:
      return "No more printers can be added.";
    case NERR_JobNoRoom:
      return "No more print jobs can be added.";
    case NERR_DestNoRoom:
      return "No more printer destinations can be added.";
    case NERR_DestIdle:
      return "This printer destination is idle and cannot accept control operations.";
    case NERR_DestInvalidOp:
      return "This printer destination request contains an invalid control function.";
    case NERR_ProcNoRespond:
      return "The print processor is not responding.";
    case NERR_SpoolerNotLoaded:
      return "The spooler is not running.";
    case NERR_DestInvalidState:
      return "This operation cannot be performed on the print destination in its current state.";
    case NERR_QInvalidState:
      return "This operation cannot be performed on the printer queue in its current state.";
    case NERR_JobInvalidState:
      return "This operation cannot be performed on the print job in its current state.";
    case NERR_SpoolNoMemory:
      return "A spooler memory allocation failure occurred.";
    case NERR_DriverNotFound:
      return "The device driver does not exist.";
    case NERR_DataTypeInvalid:
      return "The data type is not supported by the print processor.";
    case NERR_ProcNotFound:
      return "The print processor is not installed.";
    case NERR_ServiceTableLocked:
      return "The service database is locked.";
    case NERR_ServiceTableFull:
      return "The service table is full.";
    case NERR_ServiceInstalled:
      return "The requested service has already been started.";
    case NERR_ServiceEntryLocked:
      return "The service does not respond to control actions.";
    case NERR_ServiceNotInstalled:
      return "The service has not been started.";
    case NERR_BadServiceName:
      return "The service name is invalid.";
    case NERR_ServiceCtlTimeout:
      return "The service is not responding to the control function.";
    case NERR_ServiceCtlBusy:
      return "The service control is busy.";
    case NERR_BadServiceProgName:
      return "The configuration file contains an invalid service program name.";
    case NERR_ServiceNotCtrl:
      return "The service could not be controlled in its present state.";
    case NERR_ServiceKillProc:
      return "The service ended abnormally.";
    case NERR_ServiceCtlNotValid:
      return "The requested pause or stop is not valid for this service.";
    case NERR_NotInDispatchTbl:
      return "The service control dispatcher could not find the service name in the dispatch table.";
    case NERR_BadControlRecv:
      return "The service control dispatcher pipe read failed.";
    case NERR_ServiceNotStarting:
      return "A thread for the new service could not be created.";
    case NERR_AlreadyLoggedOn:
      return "This workstation is already logged on to the local-area network.";
    case NERR_NotLoggedOn:
      return "The workstation is not logged on to the local-area network.";
    case NERR_BadUsername:
      return "The user name or group name parameter is invalid.";
    case NERR_BadPassword:
      return "The password parameter is invalid.";
    case NERR_UnableToAddName_W:
      return "@W The logon processor did not add the message alias.";
    case NERR_UnableToAddName_F:
      return "The logon processor did not add the message alias.";
    case NERR_UnableToDelName_W:
      return "@W The logoff processor did not delete the message alias.";
    case NERR_UnableToDelName_F:
      return "The logoff processor did not delete the message alias.";
    case NERR_LogonsPaused:
      return "Network logons are paused.";
    case NERR_LogonServerConflict:
      return "A centralized logon-server conflict occurred.";
    case NERR_LogonNoUserPath:
      return "The server is configured without a valid user path.";
    case NERR_LogonScriptError:
      return "An error occurred while loading or running the logon script.";
    case NERR_StandaloneLogon:
      return "The logon server was not specified. Your computer will be logged on as STANDALONE.";
    case NERR_LogonServerNotFound:
      return "The logon server could not be found.";
    case NERR_LogonDomainExists:
      return "There is already a logon domain for this computer.";
    case NERR_NonValidatedLogon:
      return "The logon server could not validate the logon.";
    case NERR_ACFNotFound:
      return "The security database could not be found.";
    case NERR_GroupNotFound:
      return "The group name could not be found.";
    case NERR_UserNotFound:
      return "The user name could not be found.";
    case NERR_ResourceNotFound:
      return "The resource name could not be found.";
    case NERR_GroupExists:
      return "The group already exists.";
    case NERR_UserExists:
      return "The user account already exists.";
    case NERR_ResourceExists:
      return "The resource permission list already exists.";
    case NERR_NotPrimary:
      return "This operation is only allowed on the primary domain controller of the domain.";
    case NERR_ACFNotLoaded:
      return "The security database has not been started.";
    case NERR_ACFNoRoom:
      return "There are too many names in the user accounts database.";
    case NERR_ACFFileIOFail:
      return "A disk I/O failure occurred.";
    case NERR_ACFTooManyLists:
      return "The limit of 64 entries per resource was exceeded.";
    case NERR_UserLogon:
      return "Deleting a user with a session is not allowed.";
    case NERR_ACFNoParent:
      return "The parent directory could not be located.";
    case NERR_CanNotGrowSegment:
      return "Unable to add to the security database session cache segment.";
    case NERR_SpeGroupOp:
      return "This operation is not allowed on this special group.";
    case NERR_NotInCache:
      return "This user is not cached in user accounts database session cache.";
    case NERR_UserInGroup:
      return "The user already belongs to this group.";
    case NERR_UserNotInGroup:
      return "The user does not belong to this group.";
    case NERR_AccountUndefined:
      return "This user account is undefined.";
    case NERR_AccountExpired:
      return "This user account has expired.";
    case NERR_InvalidWorkstation:
      return "The user is not allowed to log on from this workstation.";
    case NERR_InvalidLogonHours:
      return "The user is not allowed to log on at this time.";
    case NERR_PasswordExpired:
      return "The password of this user has expired.";
    case NERR_PasswordCantChange:
      return "The password of this user cannot change.";
    case NERR_PasswordHistConflict:
      return "This password cannot be used now.";
    case NERR_PasswordTooShort:
      return "The password does not meet the password policy requirements. Check the minimum password length, password complexity and password history requirements.";
    case NERR_PasswordTooRecent:
      return "The password of this user is too recent to change.";
    case NERR_InvalidDatabase:
      return "The security database is corrupted.";
    case NERR_DatabaseUpToDate:
      return "No updates are necessary to this replicant network/local security database.";
    case NERR_SyncRequired:
      return "This replicant database is outdated; synchronization is required.";
    case NERR_UseNotFound:
      return "The network connection could not be found.";
    case NERR_BadAsgType:
      return "This asg_type is invalid.";
    case NERR_DeviceIsShared:
      return "This device is currently being shared.";
    case NERR_NoComputerName:
      return "The computer name could not be added as a message alias. The name may already exist on the network.";
    case NERR_MsgAlreadyStarted:
      return "The Messenger service is already started.";
    case NERR_MsgInitFailed:
      return "The Messenger service failed to start.";
    case NERR_NameNotFound:
      return "The message alias could not be found on the network.";
    case NERR_AlreadyForwarded:
      return "This message alias has already been forwarded.";
    case NERR_AddForwarded:
      return "This message alias has been added but is still forwarded.";
    case NERR_AlreadyExists:
      return "This message alias already exists locally.";
    case NERR_TooManyNames:
      return "The maximum number of added message aliases has been exceeded.";
    case NERR_DelComputerName:
      return "The computer name could not be deleted.";
    case NERR_LocalForward:
      return "Messages cannot be forwarded back to the same workstation.";
    case NERR_GrpMsgProcessor:
      return "An error occurred in the domain message processor.";
    case NERR_PausedRemote:
      return "The message was sent, but the recipient has paused the Messenger service.";
    case NERR_BadReceive:
      return "The message was sent but not received.";
    case NERR_NameInUse:
      return "The message alias is currently in use. Try again later.";
    case NERR_MsgNotStarted:
      return "The Messenger service has not been started.";
    case NERR_NotLocalName:
      return "The name is not on the local computer.";
    case NERR_NoForwardName:
      return "The forwarded message alias could not be found on the network.";
    case NERR_RemoteFull:
      return "The message alias table on the remote station is full.";
    case NERR_NameNotForwarded:
      return "Messages for this alias are not currently being forwarded.";
    case NERR_TruncatedBroadcast:
      return "The broadcast message was truncated.";
    case NERR_InvalidDevice:
      return "This is an invalid device name.";
    case NERR_WriteFault:
      return "A write fault occurred.";
    case NERR_DuplicateName:
      return "A duplicate message alias exists on the network.";
    case NERR_DeleteLater:
      return "@W This message alias will be deleted later.";
    case NERR_IncompleteDel:
      return "The message alias was not successfully deleted from all networks.";
    case NERR_MultipleNets:
      return "This operation is not supported on computers with multiple networks.";
    case NERR_NetNameNotFound:
      return "This shared resource does not exist.";
    case NERR_DeviceNotShared:
      return "This device is not shared.";
    case NERR_ClientNameNotFound:
      return "A session does not exist with that computer name.";
    case NERR_FileIdNotFound:
      return "There is not an open file with that identification number.";
    case NERR_ExecFailure:
      return "A failure occurred when executing a remote administration command.";
    case NERR_TmpFile:
      return "A failure occurred when opening a remote temporary file.";
    case NERR_TooMuchData:
      return "The data returned from a remote administration command has been truncated to 64K.";
    case NERR_DeviceShareConflict:
      return "This device cannot be shared as both a spooled and a non-spooled resource.";
    case NERR_BrowserTableIncomplete:
      return "The information in the list of servers may be incorrect.";
    case NERR_NotLocalDomain:
      return "The computer is not active in this domain.";
#ifdef NERR_IsDfsShare

    case NERR_IsDfsShare:
      return "The share must be removed from the Distributed File System before it can be deleted.";
#endif

    case NERR_DevInvalidOpCode:
      return "The operation is invalid for this device.";
    case NERR_DevNotFound:
      return "This device cannot be shared.";
    case NERR_DevNotOpen:
      return "This device was not open.";
    case NERR_BadQueueDevString:
      return "This device name list is invalid.";
    case NERR_BadQueuePriority:
      return "The queue priority is invalid.";
    case NERR_NoCommDevs:
      return "There are no shared communication devices.";
    case NERR_QueueNotFound:
      return "The queue you specified does not exist.";
    case NERR_BadDevString:
      return "This list of devices is invalid.";
    case NERR_BadDev:
      return "The requested device is invalid.";
    case NERR_InUseBySpooler:
      return "This device is already in use by the spooler.";
    case NERR_CommDevInUse:
      return "This device is already in use as a communication device.";
    case NERR_InvalidComputer:
      return "This computer name is invalid.";
    case NERR_MaxLenExceeded:
      return "The string and prefix specified are too long.";
    case NERR_BadComponent:
      return "This path component is invalid.";
    case NERR_CantType:
      return "Could not determine the type of input.";
    case NERR_TooManyEntries:
      return "The buffer for types is not big enough.";
    case NERR_ProfileFileTooBig:
      return "Profile files cannot exceed 64K.";
    case NERR_ProfileOffset:
      return "The start offset is out of range.";
    case NERR_ProfileCleanup:
      return "The system cannot delete current connections to network resources.";
    case NERR_ProfileUnknownCmd:
      return "The system was unable to parse the command line in this file.";
    case NERR_ProfileLoadErr:
      return "An error occurred while loading the profile file.";
    case NERR_ProfileSaveErr:
      return "@W Errors occurred while saving the profile file. The profile was partially saved.";
    case NERR_LogOverflow:
      return "Log file %1 is full.";
    case NERR_LogFileChanged:
      return "This log file has changed between reads.";
    case NERR_LogFileCorrupt:
      return "Log file %1 is corrupt.";
    case NERR_SourceIsDir:
      return "The source path cannot be a directory.";
    case NERR_BadSource:
      return "The source path is illegal.";
    case NERR_BadDest:
      return "The destination path is illegal.";
    case NERR_DifferentServers:
      return "The source and destination paths are on different servers.";
    case NERR_RunSrvPaused:
      return "The Run server you requested is paused.";
    case NERR_ErrCommRunSrv:
      return "An error occurred when communicating with a Run server.";
    case NERR_ErrorExecingGhost:
      return "An error occurred when starting a background process.";
    case NERR_ShareNotFound:
      return "The shared resource you are connected to could not be found.";
    case NERR_InvalidLana:
      return "The LAN adapter number is invalid.";
    case NERR_OpenFiles:
      return "There are open files on the connection.";
    case NERR_ActiveConns:
      return "Active connections still exist.";
    case NERR_BadPasswordCore:
      return "This share name or password is invalid.";
    case NERR_DevInUse:
      return "The device is being accessed by an active process.";
    case NERR_LocalDrive:
      return "The drive letter is in use locally.";
    case NERR_AlertExists:
      return "The specified client is already registered for the specified event.";
    case NERR_TooManyAlerts:
      return "The alert table is full.";
    case NERR_NoSuchAlert:
      return "An invalid or nonexistent alert name was raised.";
    case NERR_BadRecipient:
      return "The alert recipient is invalid.";
    case NERR_AcctLimitExceeded:
      return "A user's session with this server has been deleted.";
    case NERR_InvalidLogSeek:
      return "The log file does not contain the requested record number.";
    case NERR_BadUasConfig:
      return "The user accounts database is not configured correctly.";
    case NERR_InvalidUASOp:
      return "This operation is not permitted when the Netlogon service is running.";
    case NERR_LastAdmin:
      return "This operation is not allowed on the last administrative account.";
    case NERR_DCNotFound:
      return "Could not find domain controller for this domain.";
    case NERR_LogonTrackingError:
      return "Could not set logon information for this user.";
    case NERR_NetlogonNotStarted:
      return "The Netlogon service has not been started.";
    case NERR_CanNotGrowUASFile:
      return "Unable to add to the user accounts database.";
    case NERR_TimeDiffAtDC:
      return "This server's clock is not synchronized with the primary domain controller's clock.";
    case NERR_PasswordMismatch:
      return "A password mismatch has been detected.";
    case NERR_NoSuchServer:
      return "The server identification does not specify a valid server.";
    case NERR_NoSuchSession:
      return "The session identification does not specify a valid session.";
    case NERR_NoSuchConnection:
      return "The connection identification does not specify a valid connection.";
    case NERR_TooManyServers:
      return "There is no space for another entry in the table of available servers.";
    case NERR_TooManySessions:
      return "The server has reached the maximum number of sessions it supports.";
    case NERR_TooManyConnections:
      return "The server has reached the maximum number of connections it supports.";
    case NERR_TooManyFiles:
      return "The server cannot open more files because it has reached its maximum number.";
    case NERR_NoAlternateServers:
      return "There are no alternate servers registered on this server.";
    case NERR_TryDownLevel:
      return "Try down-level (remote admin protocol) version of API instead.";
    case NERR_UPSDriverNotStarted:
      return "The UPS driver could not be accessed by the UPS service.";
    case NERR_UPSInvalidConfig:
      return "The UPS service is not configured correctly.";
    case NERR_UPSInvalidCommPort:
      return "The UPS service could not access the specified Comm Port.";
    case NERR_UPSSignalAsserted:
      return "The UPS indicated a line fail or low battery situation. Service not started.";
    case NERR_UPSShutdownFailed:
      return "The UPS service failed to perform a system shut down.";
    case NERR_BadDosRetCode:
      return "The program below returned an MS-DOS error code:";
    case NERR_ProgNeedsExtraMem:
      return "The program below needs more memory:";
    case NERR_BadDosFunction:
      return "The program below called an unsupported MS-DOS function:";
    case NERR_RemoteBootFailed:
      return "The workstation failed to boot.";
    case NERR_BadFileCheckSum:
      return "The file below is corrupt.";
    case NERR_NoRplBootSystem:
      return "No loader is specified in the boot-block definition file.";
    case NERR_RplLoadrNetBiosErr:
      return "NetBIOS returned an error:      The NCB and SMB are dumped above.";
    case NERR_RplLoadrDiskErr:
      return "A disk I/O error occurred.";
    case NERR_ImageParamErr:
      return "Image parameter substitution failed.";
    case NERR_TooManyImageParams:
      return "Too many image parameters cross disk sector boundaries.";
    case NERR_NonDosFloppyUsed:
      return "The image was not generated from an MS-DOS diskette formatted with /S.";
    case NERR_RplBootRestart:
      return "Remote boot will be restarted later.";
    case NERR_RplSrvrCallFailed:
      return "The call to the Remoteboot server failed.";
    case NERR_CantConnectRplSrvr:
      return "Cannot connect to the Remoteboot server.";
    case NERR_CantOpenImageFile:
      return "Cannot open image file on the Remoteboot server.";
    case NERR_CallingRplSrvr:
      return "Connecting to the Remoteboot server...";
    case NERR_StartingRplBoot:
      return "Connecting to the Remoteboot server...";
    case NERR_RplBootServiceTerm:
      return "Remote boot service was stopped; check the error log for the cause of the problem.";
    case NERR_RplBootStartFailed:
      return "Remote boot startup failed; check the error log for the cause of the problem.";
    case NERR_RPL_CONNECTED:
      return "A second connection to a Remoteboot resource is not allowed.";
    case NERR_BrowserConfiguredToNotRun:
      return "The browser service was configured with MaintainServerList=No.";
    case NERR_RplNoAdaptersStarted:
      return "Service failed to start since none of the network adapters started with this service.";
    case NERR_RplBadRegistry:
      return "Service failed to start due to bad startup information in the registry.";
    case NERR_RplBadDatabase:
      return "Service failed to start because its database is absent or corrupt.";
    case NERR_RplRplfilesShare:
      return "Service failed to start because RPLFILES share is absent.";
    case NERR_RplNotRplServer:
      return "Service failed to start because RPLUSER group is absent.";
    case NERR_RplCannotEnum:
      return "Cannot enumerate service records.";
    case NERR_RplWkstaInfoCorrupted:
      return "Workstation record information has been corrupted.";
    case NERR_RplWkstaNotFound:
      return "Workstation record was not found.";
    case NERR_RplWkstaNameUnavailable:
      return "Workstation name is in use by some other workstation.";
    case NERR_RplProfileInfoCorrupted:
      return "Profile record information has been corrupted.";
    case NERR_RplProfileNotFound:
      return "Profile record was not found.";
    case NERR_RplProfileNameUnavailable:
      return "Profile name is in use by some other profile.";
    case NERR_RplProfileNotEmpty:
      return "There are workstations using this profile.";
    case NERR_RplConfigInfoCorrupted:
      return "Configuration record information has been corrupted.";
    case NERR_RplConfigNotFound:
      return "Configuration record was not found.";
    case NERR_RplAdapterInfoCorrupted:
      return "Adapter ID record information has been corrupted.";
    case NERR_RplInternal:
      return "An internal service error has occurred.";
    case NERR_RplVendorInfoCorrupted:
      return "Vendor ID record information has been corrupted.";
    case NERR_RplBootInfoCorrupted:
      return "Boot block record information has been corrupted.";
    case NERR_RplWkstaNeedsUserAcct:
      return "The user account for this workstation record is missing.";
    case NERR_RplNeedsRPLUSERAcct:
      return "The RPLUSER local group could not be found.";
    case NERR_RplBootNotFound:
      return "Boot block record was not found.";
    case NERR_RplIncompatibleProfile:
      return "Chosen profile is incompatible with this workstation.";
    case NERR_RplAdapterNameUnavailable:
      return "Chosen network adapter ID is in use by some other workstation.";
    case NERR_RplConfigNotEmpty:
      return "There are profiles using this configuration.";
    case NERR_RplBootInUse:
      return "There are workstations, profiles, or configurations using this boot block.";
    case NERR_RplBackupDatabase:
      return "Service failed to backup Remoteboot database.";
    case NERR_RplAdapterNotFound:
      return "Adapter record was not found.";
    case NERR_RplVendorNotFound:
      return "Vendor record was not found.";
    case NERR_RplVendorNameUnavailable:
      return "Vendor name is in use by some other vendor record.";
    case NERR_RplBootNameUnavailable:
      return "(boot name, vendor ID) is in use by some other boot block record.";
    case NERR_RplConfigNameUnavailable:
      return "Configuration name is in use by some other configuration.";
    case NERR_DfsInternalCorruption:
      return "The internal database maintained by the Dfs service is corrupt.";
    case NERR_DfsVolumeDataCorrupt:
      return "One of the records in the internal Dfs database is corrupt.";
    case NERR_DfsNoSuchVolume:
      return "There is no DFS name whose entry path matches the input Entry Path.";
    case NERR_DfsVolumeAlreadyExists:
      return "A root or link with the given name already exists.";
    case NERR_DfsAlreadyShared:
      return "The server share specified is already shared in the Dfs.";
    case NERR_DfsNoSuchShare:
      return "The indicated server share does not support the indicated DFS namespace.";
    case NERR_DfsNotALeafVolume:
      return "The operation is not valid on this portion of the namespace.";
    case NERR_DfsLeafVolume:
      return "The operation is not valid on this portion of the namespace.";
    case NERR_DfsVolumeHasMultipleServers:
      return "The operation is ambiguous because the link has multiple servers.";
    case NERR_DfsCantCreateJunctionPoint:
      return "Unable to create a link.";
    case NERR_DfsServerNotDfsAware:
      return "The server is not Dfs Aware.";
    case NERR_DfsBadRenamePath:
      return "The specified rename target path is invalid.";
    case NERR_DfsVolumeIsOffline:
      return "The specified DFS link is offline.";
    case NERR_DfsNoSuchServer:
      return "The specified server is not a server for this link.";
    case NERR_DfsCyclicalName:
      return "A cycle in the Dfs name was detected.";
    case NERR_DfsNotSupportedInServerDfs:
      return "The operation is not supported on a server-based Dfs.";
    case NERR_DfsDuplicateService:
      return "This link is already supported by the specified server-share.";
    case NERR_DfsCantRemoveLastServerShare:
      return "Can't remove the last server-share supporting this root or link.";
    case NERR_DfsVolumeIsInterDfs:
      return "The operation is not supported for an Inter-DFS link.";
    case NERR_DfsInconsistent:
      return "The internal state of the Dfs Service has become inconsistent.";
    case NERR_DfsServerUpgraded:
      return "The Dfs Service has been installed on the specified server.";
    case NERR_DfsDataIsIdentical:
      return "The Dfs data being reconciled is identical.";
    case NERR_DfsCantRemoveDfsRoot:
      return "The DFS root cannot be deleted. Uninstall DFS if required.";
    case NERR_DfsChildOrParentInDfs:
      return "A child or parent directory of the share is already in a Dfs.";
    case NERR_DfsInternalError:
      return "Dfs internal error.";
      /* the following are not defined in mingw */
#if 0

    case NERR_SetupAlreadyJoined:
      return "This machine is already joined to a domain.";
    case NERR_SetupNotJoined:
      return "This machine is not currently joined to a domain.";
    case NERR_SetupDomainController:
      return "This machine is a domain controller and cannot be unjoined from a domain.";
    case NERR_DefaultJoinRequired:
      return "The destination domain controller does not support creating machine accounts in OUs.";
    case NERR_InvalidWorkgroupName:
      return "The specified workgroup name is invalid.";
    case NERR_NameUsesIncompatibleCodePage:
      return "The specified computer name is incompatible with the default language used on the domain controller.";
    case NERR_ComputerAccountNotFound:
      return "The specified computer account could not be found.";
    case NERR_PersonalSku:
      return "This version of Windows cannot be joined to a domain.";
    case NERR_PasswordMustChange:
      return "The password must change at the next logon.";
    case NERR_AccountLockedOut:
      return "The account is locked out.";
    case NERR_PasswordTooLong:
      return "The password is too long.";
    case NERR_PasswordNotComplexEnough:
      return "The password does not meet the complexity policy.";
    case NERR_PasswordFilterError:
      return "The password does not meet the requirements of the password filter DLLs.";
#endif

      default:
        msg = strerror (error_number);

        if (msg == NULL)
          msg = "unknown";

        return msg;
    }
#endif //DBUS_WINCE
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
  dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                  "_dbus_command_for_pid() not implemented on Windows");
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
#ifndef DBUS_PREFIX
  /* leave path unchanged */
  return TRUE;
#else
  DBusString runtime_prefix;
  int i;

  if (!_dbus_string_init (&runtime_prefix))
    return FALSE;

  if (!_dbus_get_install_root (&runtime_prefix))
    {
      _dbus_string_free (&runtime_prefix);
      return FALSE;
    }

  if (_dbus_string_get_length (&runtime_prefix) == 0)
    {
      /* cannot determine install root, leave path unchanged */
      _dbus_string_free (&runtime_prefix);
      return TRUE;
    }

  if (_dbus_string_starts_with_c_str (path, DBUS_PREFIX "/"))
    {
      /* Replace DBUS_PREFIX "/" with runtime_prefix.
       * Note unusual calling convention: source is first, then dest */
      if (!_dbus_string_replace_len (
            &runtime_prefix, 0, _dbus_string_get_length (&runtime_prefix),
            path, 0, strlen (DBUS_PREFIX) + 1))
        {
          _dbus_string_free (&runtime_prefix);
          return FALSE;
        }
    }

  /* Somehow, in some situations, backslashes get collapsed in the string.
   * Since windows C library accepts both forward and backslashes as
   * path separators, convert all backslashes to forward slashes.
   */

  for (i = 0; i < _dbus_string_get_length (path); i++)
    {
      if (_dbus_string_get_byte (path, i) == '\\')
        _dbus_string_set_byte (path, i, '/');
    }

  _dbus_string_free (&runtime_prefix);
  return TRUE;
#endif
}

#define DBUS_STANDARD_SESSION_SERVICEDIR "/dbus-1/services"
#define DBUS_STANDARD_SYSTEM_SERVICEDIR "/dbus-1/system-services"

/**
 * Returns the standard directories for a session bus to look for
 * transient service activation files. On Windows, there are none.
 *
 * @param dirs the directory list we are returning
 * @returns #TRUE
 */
dbus_bool_t
_dbus_set_up_transient_session_servicedirs (DBusList  **dirs,
                                            DBusError  *error)
{
  /* Not an error, we just don't have transient session services on Windows */
  return TRUE;
}

/**
 * Returns the standard directories for a session bus to look for service
 * activation files
 *
 * On Windows this should be data directories:
 *
 * %CommonProgramFiles%/dbus
 *
 * and
 *
 * relocated DBUS_DATADIR
 *
 * @param dirs the directory list we are returning
 * @returns #FALSE on OOM
 */

dbus_bool_t
_dbus_get_standard_session_servicedirs (DBusList **dirs)
{
  const char *common_progs;
  DBusString servicedir_path;

  if (!_dbus_string_init (&servicedir_path))
    return FALSE;

#ifdef DBUS_WINCE
  {
    /* On Windows CE, we adjust datadir dynamically to installation location.  */
    const char *data_dir = _dbus_getenv ("DBUS_DATADIR");

    if (data_dir != NULL)
      {
        if (!_dbus_string_append (&servicedir_path, data_dir))
          goto oom;

        if (!_dbus_string_append (&servicedir_path, _DBUS_PATH_SEPARATOR))
          goto oom;
      }
  }
#else
  {
    DBusString p;

    if (!_dbus_string_init (&p))
      goto oom;

    /* DBUS_DATADIR is assumed to be absolute; the build systems should
     * ensure that. */
    if (!_dbus_string_append (&p, DBUS_DATADIR) ||
        !_dbus_replace_install_prefix (&p))
      {
        _dbus_string_free (&p);
        goto oom;
      }

    if (!_dbus_string_append (&servicedir_path,
          _dbus_string_get_const_data (&p)))
      {
        _dbus_string_free (&p);
        goto oom;
      }

    _dbus_string_free (&p);
  }

  if (!_dbus_string_append (&servicedir_path, _DBUS_PATH_SEPARATOR))
    goto oom;
#endif

  common_progs = _dbus_getenv ("CommonProgramFiles");

  if (common_progs != NULL)
    {
      if (!_dbus_string_append (&servicedir_path, common_progs))
        goto oom;

      if (!_dbus_string_append (&servicedir_path, _DBUS_PATH_SEPARATOR))
        goto oom;
    }

  if (!_dbus_split_paths_and_append (&servicedir_path,
                               DBUS_STANDARD_SESSION_SERVICEDIR,
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
  *dirs = NULL;
  return TRUE;
}

static dbus_bool_t
_dbus_get_config_file_name (DBusString *str,
                            const char *basename)
{
  DBusString tmp;

  if (!_dbus_string_append (str, DBUS_DATADIR) ||
      !_dbus_replace_install_prefix (str))
    return FALSE;

  _dbus_string_init_const (&tmp, "dbus-1");

  if (!_dbus_concat_dir_and_file (str, &tmp))
    return FALSE;

  _dbus_string_init_const (&tmp, basename);

  if (!_dbus_concat_dir_and_file (str, &tmp))
    return FALSE;

  return TRUE;
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

  return _dbus_get_config_file_name(str, "system.conf");
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

  return _dbus_get_config_file_name(str, "session.conf");
}

void
_dbus_daemon_report_ready (void)
{
}

void
_dbus_daemon_report_reloading (void)
{
}

void
_dbus_daemon_report_reloaded (void)
{
}

void
_dbus_daemon_report_stopping (void)
{
}
