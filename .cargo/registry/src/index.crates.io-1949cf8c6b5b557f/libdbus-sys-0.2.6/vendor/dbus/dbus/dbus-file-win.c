/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-file-win.c windows related file implementation (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003, 2006  Red Hat, Inc.
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
#include "dbus-protocol.h"
#include "dbus-string.h"
#include "dbus-internals.h"
#include "dbus-sysdeps-win.h"
#include "dbus-pipe.h"

#include <windows.h>


/**
 * Thin wrapper around the read() system call that appends
 * the data it reads to the DBusString buffer. It appends
 * up to the given count.
 * 
 * @param hnd the HANDLE to read from
 * @param buffer the buffer to append data to
 * @param count the amount of data to read
 * @param error place to set an error
 * @returns the number of bytes read or -1
 */
static int
_dbus_file_read (HANDLE            hnd,
                 DBusString       *buffer,
                 int               count,
                 DBusError        *error)
{
  BOOL result;
  DWORD bytes_read;
  int start;
  char *data;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_assert (count >= 0);

  start = _dbus_string_get_length (buffer);

  if (!_dbus_string_lengthen (buffer, count))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return -1;
    }

  data = _dbus_string_get_data_len (buffer, start, count);

  result = ReadFile (hnd, data, count, &bytes_read, NULL);
  if (result == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                      "Failed to read from %p: %s", hnd, emsg);
      _dbus_win_free_error_string (emsg);
      return -1;
    }

  if (bytes_read)
    {
      /* put length back (doesn't actually realloc) */
      _dbus_string_set_length (buffer, start + bytes_read);

#if 0
      if (bytes_read > 0)
        _dbus_verbose_bytes_of_string (buffer, start, bytes_read);
#endif
    }

  return bytes_read;
}


/**
 * Appends the contents of the given file to the string,
 * returning error code. At the moment, won't open a file
 * more than a megabyte in size.
 *
 * @param str the string to append to
 * @param filename filename to load
 * @param error place to set an error
 * @returns #FALSE if error was set
 */
dbus_bool_t
_dbus_file_get_contents (DBusString       *str,
                         const DBusString *filename,
                         DBusError        *error)
{
  HANDLE hnd;
  DWORD fsize;
  DWORD fsize_hi;
  int orig_len;
  unsigned int total;
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  hnd = CreateFileA (filename_c, GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
  if (hnd == INVALID_HANDLE_VALUE)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Failed to open \"%s\": %s", filename_c, emsg);
      _dbus_win_free_error_string (emsg);
      return FALSE;
    }

  _dbus_verbose ("file %s hnd %p opened\n", filename_c, hnd);
  
  fsize = GetFileSize (hnd, &fsize_hi);
  if (fsize == 0xFFFFFFFF && GetLastError() != NO_ERROR)
    { 
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                      "Failed to get file size for \"%s\": %s",
                      filename_c, emsg);
      _dbus_win_free_error_string (emsg);

      _dbus_verbose ("GetFileSize() failed: %s", emsg);

      CloseHandle (hnd);

      return FALSE;
    }

  if (fsize_hi != 0 || fsize > _DBUS_ONE_MEGABYTE)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "File size %lu/%lu of \"%s\" is too large.",
                      (unsigned long) fsize_hi,
                      (unsigned long) fsize, filename_c);
      CloseHandle (hnd);
      return FALSE;
    }

  total = 0;
  orig_len = _dbus_string_get_length (str);
  if (fsize > 0)
    {
      int bytes_read;

      while (total < fsize)
        {
          bytes_read = _dbus_file_read (hnd, str, fsize - total, error);
          if (bytes_read <= 0)
            {
              if (bytes_read == 0)
                {
                  dbus_set_error (error, DBUS_ERROR_FAILED,
                                  "Premature EOF reading \"%s\"",
                                  filename_c);
                }
              else
                _DBUS_ASSERT_ERROR_IS_SET (error);

              CloseHandle (hnd);
              _dbus_string_set_length (str, orig_len);
              return FALSE;
            }
          else
            total += bytes_read;
        }

      CloseHandle (hnd);
      return TRUE;
    }
  else
    {
      CloseHandle (hnd);
      return TRUE;
    }
}


/**
 * Writes a string out to a file. If the file exists,
 * it will be atomically overwritten by the new data.
 *
 * @param str the string to write out
 * @param filename the file to save string to
 * @param world_readable if true, ensure file is world readable
 * @param error error to be filled in on failure
 * @returns #FALSE on failure
 */
dbus_bool_t
_dbus_string_save_to_file (const DBusString *str,
                           const DBusString *filename,
                           dbus_bool_t       world_readable,
                           DBusError        *error)
{
  HANDLE hnd;
  int bytes_to_write;
  const char *filename_c;
  DBusString tmp_filename;
  const char *tmp_filename_c;
  int total;
  const char *str_c;
  dbus_bool_t need_unlink;
  dbus_bool_t retval;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  hnd = INVALID_HANDLE_VALUE;
  retval = FALSE;
  need_unlink = FALSE;

  if (!_dbus_string_init (&tmp_filename))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  if (!_dbus_string_copy (filename, 0, &tmp_filename, 0))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&tmp_filename);
      return FALSE;
    }

  if (!_dbus_string_append (&tmp_filename, "."))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&tmp_filename);
      return FALSE;
    }

#define N_TMP_FILENAME_RANDOM_BYTES 8
  if (!_dbus_generate_random_ascii (&tmp_filename, N_TMP_FILENAME_RANDOM_BYTES,
                                    error))
    {
      _dbus_string_free (&tmp_filename);
      return FALSE;
    }

  filename_c = _dbus_string_get_const_data (filename);
  tmp_filename_c = _dbus_string_get_const_data (&tmp_filename);

  /* TODO - support world-readable in an atomic fashion */
  hnd = CreateFileA (tmp_filename_c, GENERIC_WRITE,
                     FILE_SHARE_READ | FILE_SHARE_WRITE,
                     NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
                     INVALID_HANDLE_VALUE);
  if (hnd == INVALID_HANDLE_VALUE)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not create \"%s\": %s", filename_c, emsg);
      _dbus_win_free_error_string (emsg);
      goto out;
    }
  if (world_readable)
    {
      if (! _dbus_make_file_world_readable (&tmp_filename, error))
        goto out;
    }

  _dbus_verbose ("tmp file %s hnd %p opened\n", tmp_filename_c, hnd);

  need_unlink = TRUE;

  total = 0;
  bytes_to_write = _dbus_string_get_length (str);
  str_c = _dbus_string_get_const_data (str);

  while (total < bytes_to_write)
    {
      DWORD bytes_written;
      BOOL res;

      res = WriteFile (hnd, str_c + total, bytes_to_write - total,
                       &bytes_written, NULL);

      if (res == 0 || bytes_written <= 0)
        {
          char *emsg = _dbus_win_error_string (GetLastError ());
          dbus_set_error (error, _dbus_win_error_from_last_error (),
                           "Could not write to %s: %s", tmp_filename_c, emsg);
          _dbus_win_free_error_string (emsg);
          goto out;
        }

      total += bytes_written;
    }

  if (CloseHandle (hnd) == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not close file %s: %s", tmp_filename_c, emsg);
      _dbus_win_free_error_string (emsg);
      goto out;
    }

  hnd = INVALID_HANDLE_VALUE;

  /* Unlike rename(), MoveFileEx() can replace existing files */
  if (!MoveFileExA (tmp_filename_c, filename_c, MOVEFILE_REPLACE_EXISTING))
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not rename %s to %s: %s",
                       tmp_filename_c, filename_c, emsg);
      _dbus_win_free_error_string (emsg);

      goto out;
    }

  need_unlink = FALSE;

  retval = TRUE;

 out:
  /* close first, then unlink */

  if (hnd != INVALID_HANDLE_VALUE)
    CloseHandle (hnd);

  if (need_unlink && DeleteFileA (tmp_filename_c) == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      _dbus_verbose ("Failed to unlink temp file %s: %s", tmp_filename_c,
                     emsg);
      _dbus_win_free_error_string (emsg);
    }

  _dbus_string_free (&tmp_filename);

  if (!retval)
    _DBUS_ASSERT_ERROR_IS_SET (error);

  return retval;
}


/** Creates the given file, failing if the file already exists.
 *
 * @param filename the filename
 * @param error error location
 * @returns #TRUE if we created the file and it didn't exist
 */
dbus_bool_t
_dbus_create_file_exclusively (const DBusString *filename,
                               DBusError        *error)
{
  HANDLE hnd;
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  hnd = CreateFileA (filename_c, GENERIC_WRITE,
                     FILE_SHARE_READ | FILE_SHARE_WRITE,
                     NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
                     INVALID_HANDLE_VALUE);
  if (hnd == INVALID_HANDLE_VALUE)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not create file %s: %s",
                       filename_c, emsg);
      _dbus_win_free_error_string (emsg);
      return FALSE;
    }

  _dbus_verbose ("exclusive file %s hnd %p opened\n", filename_c, hnd);

  if (CloseHandle (hnd) == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                       "Could not close file %s: %s",
                       filename_c, emsg);
      _dbus_win_free_error_string (emsg);

      return FALSE;
    }

  return TRUE;
}
