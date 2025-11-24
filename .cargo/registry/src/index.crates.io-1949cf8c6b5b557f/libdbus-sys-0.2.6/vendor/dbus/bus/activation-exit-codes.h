/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* activation-exit-codes.h  Return values for the launch helper which is set
 *                          in the helper and read in dbus-spawn.
 *
 * Copyright (C) 2007 Red Hat, Inc.
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

#ifndef BUS_ACTIVATION_EXIT_CODES_H
#define BUS_ACTIVATION_EXIT_CODES_H

/** Return codes from the launch helper - not public API. However,
 *  presumably if some third party did write their own launch helper,
 *  they would have to rely on these, or at least always return
 *  1 for GENERIC_FAILURE.
 */
#define BUS_SPAWN_EXIT_CODE_GENERIC_FAILURE      1
#define BUS_SPAWN_EXIT_CODE_NO_MEMORY            2
#define BUS_SPAWN_EXIT_CODE_CONFIG_INVALID       3
#define BUS_SPAWN_EXIT_CODE_SETUP_FAILED         4
#define BUS_SPAWN_EXIT_CODE_NAME_INVALID         5
#define BUS_SPAWN_EXIT_CODE_SERVICE_NOT_FOUND    6
#define BUS_SPAWN_EXIT_CODE_PERMISSIONS_INVALID  7
#define BUS_SPAWN_EXIT_CODE_FILE_INVALID         8
#define BUS_SPAWN_EXIT_CODE_EXEC_FAILED          9
#define BUS_SPAWN_EXIT_CODE_INVALID_ARGS         10
#define BUS_SPAWN_EXIT_CODE_CHILD_SIGNALED       11

#endif /* BUS_ACTIVATION_EXIT_CODES_H */
