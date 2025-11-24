/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dir-watch.h  Watch directories
 *
 * Copyright (C) 2005 Red Hat, Inc.
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

#include "bus.h"

#ifndef DIR_WATCH_H
#define DIR_WATCH_H

/**
 * Update the set of directories to monitor for changes.  The
 * operating-system-specific implementation of this function should
 * avoid creating a window where a directory in both the
 * old and new set isn't monitored.
 *
 * @param context The bus context
 * @param dirs List of strings which are directory paths
 */
void bus_set_watched_dirs (BusContext *context, DBusList **dirs);

#endif /* DIR_WATCH_H */
