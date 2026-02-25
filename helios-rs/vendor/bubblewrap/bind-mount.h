/* bubblewrap
 * Copyright (C) 2016 Alexander Larsson
 * SPDX-License-Identifier: LGPL-2.0-or-later
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library. If not, see <http://www.gnu.org/licenses/>.
 *
 */

#pragma once

#include "utils.h"

typedef enum {
  BIND_READONLY = (1 << 0),
  BIND_DEVICES = (1 << 2),
  BIND_RECURSIVE = (1 << 3),
} bind_option_t;

typedef enum
{
  BIND_MOUNT_SUCCESS = 0,
  BIND_MOUNT_ERROR_MOUNT,
  BIND_MOUNT_ERROR_REALPATH_DEST,
  BIND_MOUNT_ERROR_REOPEN_DEST,
  BIND_MOUNT_ERROR_READLINK_DEST_PROC_FD,
  BIND_MOUNT_ERROR_FIND_DEST_MOUNT,
  BIND_MOUNT_ERROR_REMOUNT_DEST,
  BIND_MOUNT_ERROR_REMOUNT_SUBMOUNT,
} bind_mount_result;

bind_mount_result bind_mount (int           proc_fd,
                              const char   *src,
                              const char   *dest,
                              bind_option_t options,
                              char        **failing_path);

void die_with_bind_result (bind_mount_result res,
                           int               saved_errno,
                           const char       *failing_path,
                           const char       *format,
                           ...)
  __attribute__((__noreturn__))
  __attribute__((format (printf, 4, 5)));
