/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-userdb-util.c Would be in dbus-userdb.c, but not used in libdbus
 *
 * Copyright (C) 2003, 2004, 2005  Red Hat, Inc.
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
#include <unistd.h>
#define DBUS_USERDB_INCLUDES_PRIVATE 1
#include "dbus-userdb.h"
#include "dbus-test.h"
#include "dbus-internals.h"
#include "dbus-protocol.h"
#include <dbus/dbus-test-tap.h>
#include <string.h>

/* It isn't obvious from its name, but this file is part of the Unix
 * system-dependent part of libdbus. */
#if defined(DBUS_WIN) || !defined(DBUS_UNIX)
#error "This file only makes sense on Unix OSs"
#endif

#ifdef HAVE_SYSTEMD
#include <systemd/sd-login.h>
#endif

/**
 * @addtogroup DBusInternalsUtils
 * @{
 */

static DBusGroupInfo *
_dbus_group_info_ref (DBusGroupInfo *info)
{
  _dbus_assert (info->refcount > 0);
  _dbus_assert (info->refcount < SIZE_MAX);
  info->refcount++;
  return info;
}

/**
 * Checks to see if the UID sent in is the console user
 *
 * @param uid UID of person to check 
 * @param error return location for errors
 * @returns #TRUE if the UID is the same as the console user and there are no errors
 */
dbus_bool_t
_dbus_is_console_user (dbus_uid_t uid,
		       DBusError *error)
{

  DBusUserDatabase *db;
  const DBusUserInfo *info;
  dbus_bool_t result = FALSE;

#ifdef HAVE_SYSTEMD
  /* check if we have logind */
  if (access ("/run/systemd/seats/", F_OK) >= 0)
    {
      int r;

      /* Check whether this user is logged in on at least one physical
         seat */
      r = sd_uid_get_seats (uid, 0, NULL);
      if (r < 0)
        {
          dbus_set_error (error, _dbus_error_from_errno (-r),
                          "Failed to determine seats of user \"" DBUS_UID_FORMAT "\": %s",
                          uid,
                          _dbus_strerror (-r));
          return FALSE;
        }

      return (r > 0);
    }
#endif

#ifdef HAVE_CONSOLE_OWNER_FILE

  DBusString f;
  DBusStat st;

  if (!_dbus_string_init (&f))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_append(&f, DBUS_CONSOLE_OWNER_FILE))
    {
      _dbus_string_free(&f);
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (_dbus_stat(&f, &st, NULL) && (st.uid == uid))
    {
      _dbus_string_free(&f);
      return TRUE;
    }

  _dbus_string_free(&f);

#endif /* HAVE_CONSOLE_OWNER_FILE */

  if (!_dbus_user_database_lock_system ())
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  db = _dbus_user_database_get_system ();
  if (db == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED, "Could not get system database.");
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  /* TPTD: this should be cache-safe, we've locked the DB and
    _dbus_user_at_console doesn't pass it on. */
  info = _dbus_user_database_lookup (db, uid, NULL, error);

  if (info == NULL)
    {
      _dbus_user_database_unlock_system ();
       return FALSE;
    }

  result = _dbus_user_at_console (info->username, error);

  _dbus_user_database_unlock_system ();

  return result;
}

/**
 * Gets user ID given username
 *
 * @param username the username
 * @param uid return location for UID
 * @returns #TRUE if username existed and we got the UID
 */
dbus_bool_t
_dbus_get_user_id (const DBusString  *username,
                   dbus_uid_t        *uid)
{
  return _dbus_get_user_id_and_primary_group (username, uid, NULL);
}

/**
 * Gets group ID given groupname
 *
 * @param groupname the groupname
 * @param gid return location for GID
 * @returns #TRUE if group name existed and we got the GID
 */
dbus_bool_t
_dbus_get_group_id (const DBusString  *groupname,
                    dbus_gid_t        *gid)
{
  DBusUserDatabase *db;
  const DBusGroupInfo *info;

  /* FIXME: this can't distinguish ENOMEM from other errors */
  if (!_dbus_user_database_lock_system ())
    return FALSE;

  db = _dbus_user_database_get_system ();
  if (db == NULL)
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  info = _dbus_user_database_lookup_group (db, DBUS_GID_UNSET, groupname,
                                           NULL);

  if (info == NULL)
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  *gid = info->gid;
  
  _dbus_user_database_unlock_system ();
  return TRUE;
}

/**
 * Gets user ID and primary group given username
 *
 * @param username the username
 * @param uid_p return location for UID
 * @param gid_p return location for GID
 * @returns #TRUE if username existed and we got the UID and GID
 */
dbus_bool_t
_dbus_get_user_id_and_primary_group (const DBusString  *username,
                                     dbus_uid_t        *uid_p,
                                     dbus_gid_t        *gid_p)
{
  DBusUserDatabase *db;
  const DBusUserInfo *info;

  /* FIXME: this can't distinguish ENOMEM from other errors */
  if (!_dbus_user_database_lock_system ())
    return FALSE;

  db = _dbus_user_database_get_system ();
  if (db == NULL)
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  if (!_dbus_user_database_get_username (db, username,
                                         &info, NULL))
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  if (uid_p)
    *uid_p = info->uid;
  if (gid_p)
    *gid_p = info->primary_gid;
  
  _dbus_user_database_unlock_system ();
  return TRUE;
}

/**
 * Looks up a gid or group name in the user database.  Only one of
 * name or GID can be provided. There are wrapper functions for this
 * that are better to use, this one does no locking or anything on the
 * database and otherwise sort of sucks.
 *
 * @param db the database
 * @param gid the group ID or #DBUS_GID_UNSET
 * @param groupname group name or #NULL 
 * @param error error to fill in
 * @returns the entry in the database (borrowed, do not free)
 */
const DBusGroupInfo *
_dbus_user_database_lookup_group (DBusUserDatabase *db,
                                  dbus_gid_t        gid,
                                  const DBusString *groupname,
                                  DBusError        *error)
{
  DBusGroupInfo *info;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

   /* See if the group is really a number */
   if (gid == DBUS_UID_UNSET)
    {
      unsigned long n;

      if (_dbus_is_a_number (groupname, &n))
        gid = n;
    }

  if (gid != DBUS_GID_UNSET)
    info = _dbus_hash_table_lookup_uintptr (db->groups, gid);
  else
    info = _dbus_hash_table_lookup_string (db->groups_by_name,
                                           _dbus_string_get_const_data (groupname));
  if (info)
    {
      _dbus_verbose ("Using cache for GID "DBUS_GID_FORMAT" information\n",
                     info->gid);
      return info;
    }
  else
    {
      if (gid != DBUS_GID_UNSET)
	_dbus_verbose ("No cache for GID "DBUS_GID_FORMAT"\n",
		       gid);
      else
	_dbus_verbose ("No cache for groupname \"%s\"\n",
		       _dbus_string_get_const_data (groupname));
      
      info = dbus_new0 (DBusGroupInfo, 1);
      if (info == NULL)
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          return NULL;
        }
      info->refcount = 1;

      if (gid != DBUS_GID_UNSET)
        {
          if (!_dbus_group_info_fill_gid (info, gid, error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_group_info_unref (info);
              return NULL;
            }
        }
      else
        {
          if (!_dbus_group_info_fill (info, groupname, error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_group_info_unref (info);
              return NULL;
            }
        }

      /* don't use these past here */
      gid = DBUS_GID_UNSET;
      groupname = NULL;

      if (_dbus_hash_table_insert_uintptr (db->groups, info->gid, info))
        {
          _dbus_group_info_ref (info);
        }
      else
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          _dbus_group_info_unref (info);
          return NULL;
        }


      if (_dbus_hash_table_insert_string (db->groups_by_name,
                                          info->groupname,
                                          info))
        {
          _dbus_group_info_ref (info);
        }
      else
        {
          _dbus_hash_table_remove_uintptr (db->groups, info->gid);
          _dbus_group_info_unref (info);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          return NULL;
        }

      /* Release the original reference */
      _dbus_group_info_unref (info);

      /* Return a borrowed reference to the DBusGroupInfo owned by the
       * two hash tables */
      return info;
    }
}

/**
 * Gets all groups  corresponding to the given UID. Returns #FALSE
 * if no memory, or user isn't known, but always initializes
 * group_ids to a NULL array. 
 *
 * @param uid the UID
 * @param group_ids return location for array of group IDs
 * @param n_group_ids return location for length of returned array
 * @returns #TRUE if the UID existed and we got some credentials
 */
dbus_bool_t
_dbus_groups_from_uid (dbus_uid_t         uid,
                       dbus_gid_t       **group_ids,
                       int               *n_group_ids)
{
  DBusUserDatabase *db;
  const DBusUserInfo *info;
  *group_ids = NULL;
  *n_group_ids = 0;

  /* FIXME: this can't distinguish ENOMEM from other errors */
  if (!_dbus_user_database_lock_system ())
    return FALSE;

  db = _dbus_user_database_get_system ();
  if (db == NULL)
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  if (!_dbus_user_database_get_uid (db, uid,
                                    &info, NULL))
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  _dbus_assert (info->uid == uid);
  
  if (info->n_group_ids > 0)
    {
      *group_ids = dbus_new (dbus_gid_t, info->n_group_ids);
      if (*group_ids == NULL)
        {
	  _dbus_user_database_unlock_system ();
          return FALSE;
        }

      *n_group_ids = info->n_group_ids;

      memcpy (*group_ids, info->group_ids, info->n_group_ids * sizeof (dbus_gid_t));
    }

  _dbus_user_database_unlock_system ();
  return TRUE;
}
/** @} */
