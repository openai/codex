/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-userdb.h User database abstraction
 *
 * Copyright (C) 2003  Red Hat, Inc.
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

#ifndef DBUS_USERDB_H
#define DBUS_USERDB_H

#include <dbus/dbus-sysdeps-unix.h>

#ifdef DBUS_WIN
#error "Don't include this on Windows"
#endif

DBUS_BEGIN_DECLS

typedef struct DBusUserDatabase DBusUserDatabase;

#ifdef DBUS_USERDB_INCLUDES_PRIVATE
#include <dbus/dbus-hash.h>

/**
 * Internals of DBusUserDatabase
 */
struct DBusUserDatabase
{
  int refcount; /**< Reference count */

  DBusHashTable *users; /**< Users in the database by UID */
  DBusHashTable *groups; /**< Groups in the database by GID */
  DBusHashTable *users_by_name; /**< Users in the database by name */
  DBusHashTable *groups_by_name; /**< Groups in the database by name */

};


DBusUserDatabase* _dbus_user_database_new           (void);
DBusUserDatabase* _dbus_user_database_ref           (DBusUserDatabase     *db);
void              _dbus_user_database_flush         (DBusUserDatabase     *db);
void              _dbus_user_database_unref         (DBusUserDatabase     *db);
DBUS_PRIVATE_EXPORT
dbus_bool_t       _dbus_user_database_get_uid       (DBusUserDatabase     *db,
                                                     dbus_uid_t            uid,
                                                     const DBusUserInfo  **info,
                                                     DBusError            *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t       _dbus_user_database_get_username  (DBusUserDatabase     *db,
                                                     const DBusString     *username,
                                                     const DBusUserInfo  **info,
                                                     DBusError            *error);
DBUS_PRIVATE_EXPORT
const DBusUserInfo *_dbus_user_database_lookup  (DBusUserDatabase *db,
                                                 dbus_uid_t        uid,
                                                 const DBusString *username,
                                                 DBusError        *error);
DBUS_PRIVATE_EXPORT
const DBusGroupInfo* _dbus_user_database_lookup_group (DBusUserDatabase *db,
                                                       dbus_gid_t        gid,
                                                       const DBusString *groupname,
                                                       DBusError        *error);

void           _dbus_user_info_unref            (DBusUserInfo     *info);
DBUS_PRIVATE_EXPORT
void           _dbus_group_info_unref           (DBusGroupInfo    *info);
#endif /* DBUS_USERDB_INCLUDES_PRIVATE */

DBUS_PRIVATE_EXPORT
DBusUserDatabase* _dbus_user_database_get_system    (void);
DBUS_PRIVATE_EXPORT _DBUS_WARN_UNUSED_RESULT
dbus_bool_t       _dbus_user_database_lock_system   (void);
DBUS_PRIVATE_EXPORT
void              _dbus_user_database_unlock_system (void);
void              _dbus_user_database_flush_system  (void);

dbus_bool_t _dbus_get_user_id                   (const DBusString  *username,
                                                 dbus_uid_t        *uid);
dbus_bool_t _dbus_get_group_id                  (const DBusString  *group_name,
                                                 dbus_gid_t        *gid);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_get_user_id_and_primary_group (const DBusString  *username,
                                                 dbus_uid_t        *uid_p,
                                                 dbus_gid_t        *gid_p);
dbus_bool_t _dbus_groups_from_uid		(dbus_uid_t            uid,
                                                 dbus_gid_t          **group_ids,
                                                 int                  *n_group_ids);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_is_console_user               (dbus_uid_t         uid,
                                                 DBusError         *error);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_is_a_number                   (const DBusString *str, 
                                                 unsigned long    *num);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_username_from_current_process (const DBusString **username);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_homedir_from_current_process  (const DBusString **homedir);
dbus_bool_t _dbus_homedir_from_uid              (dbus_uid_t         uid,
                                                 DBusString        *homedir);

DBUS_END_DECLS

#endif /* DBUS_USERDB_H */
