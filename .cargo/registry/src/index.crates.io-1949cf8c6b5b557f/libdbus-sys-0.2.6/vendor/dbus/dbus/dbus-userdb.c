/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-userdb.c User database abstraction
 *
 * Copyright (C) 2003, 2004  Red Hat, Inc.
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
#define DBUS_USERDB_INCLUDES_PRIVATE 1
#include "dbus-userdb.h"
#include "dbus-hash.h"
#include "dbus-test.h"
#include "dbus-internals.h"
#include "dbus-protocol.h"
#include "dbus-credentials.h"
#include <string.h>

/* It isn't obvious from its name, but this file is part of the Unix
 * system-dependent part of libdbus. Windows has a parallel
 * implementation of some of it in dbus-sysdeps-win.c. */
#if defined(DBUS_WIN) || !defined(DBUS_UNIX)
#error "This file only makes sense on Unix OSs"
#endif

/**
 * @addtogroup DBusInternalsUtils
 * @{
 */

static DBusUserInfo *
_dbus_user_info_ref (DBusUserInfo *info)
{
  _dbus_assert (info->refcount > 0);
  _dbus_assert (info->refcount < SIZE_MAX);
  info->refcount++;
  return info;
}

/**
 * Decrements the reference count. If it reaches 0,
 * frees the given #DBusUserInfo's members with _dbus_user_info_free()
 * and also calls dbus_free() on the block itself
 *
 * @param info the info
 */
void
_dbus_user_info_unref (DBusUserInfo *info)
{
  if (info == NULL) /* hash table will pass NULL */
    return;

  _dbus_assert (info->refcount > 0);
  _dbus_assert (info->refcount < SIZE_MAX);

  if (--info->refcount > 0)
    return;

  _dbus_user_info_free (info);
  dbus_free (info);
}

/**
 * Decrements the reference count. If it reaches 0,
 * frees the given #DBusGroupInfo's members with _dbus_group_info_free()
 * and also calls dbus_free() on the block itself
 *
 * @param info the info
 */
void
_dbus_group_info_unref (DBusGroupInfo *info)
{
  if (info == NULL) /* hash table will pass NULL */
    return;

  _dbus_assert (info->refcount > 0);
  _dbus_assert (info->refcount < SIZE_MAX);

  if (--info->refcount > 0)
    return;

  _dbus_group_info_free (info);
  dbus_free (info);
}

/**
 * Frees the members of info
 * (but not info itself)
 * @param info the user info struct
 */
void
_dbus_user_info_free (DBusUserInfo *info)
{
  dbus_free (info->group_ids);
  dbus_free (info->username);
  dbus_free (info->homedir);
}

/**
 * Frees the members of info (but not info itself).
 *
 * @param info the group info
 */
void
_dbus_group_info_free (DBusGroupInfo    *info)
{
  dbus_free (info->groupname);
}

/**
 * Checks if a given string is actually a number 
 * and converts it if it is 
 *
 * @param str the string to check
 * @param num the memory location of the unsigned long to fill in
 * @returns TRUE if str is a number and num is filled in 
 */
dbus_bool_t
_dbus_is_a_number (const DBusString *str,
                   unsigned long    *num)
{
  int end;

  if (_dbus_string_parse_uint (str, 0, num, &end) &&
      end == _dbus_string_get_length (str))
    return TRUE;
  else
    return FALSE;
}

/**
 * Looks up a uid or username in the user database.  Only one of name
 * or UID can be provided. There are wrapper functions for this that
 * are better to use, this one does no locking or anything on the
 * database and otherwise sort of sucks.
 *
 * @param db the database
 * @param uid the user ID or #DBUS_UID_UNSET
 * @param username username or #NULL 
 * @param error error to fill in
 * @returns the entry in the database (borrowed, do not free)
 */
const DBusUserInfo *
_dbus_user_database_lookup (DBusUserDatabase *db,
                            dbus_uid_t        uid,
                            const DBusString *username,
                            DBusError        *error)
{
  DBusUserInfo *info;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  _dbus_assert (uid != DBUS_UID_UNSET || username != NULL);

  /* See if the username is really a number */
  if (uid == DBUS_UID_UNSET)
    {
      unsigned long n;

      if (_dbus_is_a_number (username, &n))
        uid = n;
    }

  if (uid != DBUS_UID_UNSET)
    info = _dbus_hash_table_lookup_uintptr (db->users, uid);
  else
    info = _dbus_hash_table_lookup_string (db->users_by_name, _dbus_string_get_const_data (username));

  if (info)
    {
      _dbus_verbose ("Using cache for UID "DBUS_UID_FORMAT" information\n",
                     info->uid);
      return info;
    }
  else
    {
      if (uid != DBUS_UID_UNSET)
	_dbus_verbose ("No cache for UID "DBUS_UID_FORMAT"\n",
		       uid);
      else
	_dbus_verbose ("No cache for user \"%s\"\n",
		       _dbus_string_get_const_data (username));
      
      info = dbus_new0 (DBusUserInfo, 1);
      if (info == NULL)
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          return NULL;
        }
      info->refcount = 1;

      if (uid != DBUS_UID_UNSET)
        {
          if (!_dbus_user_info_fill_uid (info, uid, error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_user_info_unref (info);
              return NULL;
            }
        }
      else
        {
          if (!_dbus_user_info_fill (info, username, error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_user_info_unref (info);
              return NULL;
            }
        }

      /* be sure we don't use these after here */
      uid = DBUS_UID_UNSET;
      username = NULL;

      /* insert into hash */
      if (_dbus_hash_table_insert_uintptr (db->users, info->uid, info))
        {
          _dbus_user_info_ref (info);
        }
      else
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          _dbus_user_info_unref (info);
          return NULL;
        }

      if (_dbus_hash_table_insert_string (db->users_by_name,
                                          info->username,
                                          info))
        {
          _dbus_user_info_ref (info);
        }
      else
        {
          _dbus_hash_table_remove_uintptr (db->users, info->uid);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          _dbus_user_info_unref (info);
          return NULL;
        }

      _dbus_user_info_unref (info);

      /* Return a borrowed pointer to the DBusUserInfo owned by the
       * hash tables */
      return info;
    }
}

/* Protected by _DBUS_LOCK_system_users */
static dbus_bool_t database_locked = FALSE;
static DBusUserDatabase *system_db = NULL;
static DBusString process_username;
static DBusString process_homedir;
      
static void
shutdown_system_db (void *data)
{
  if (system_db != NULL)
    _dbus_user_database_unref (system_db);
  system_db = NULL;
  _dbus_string_free (&process_username);
  _dbus_string_free (&process_homedir);
}

static dbus_bool_t
init_system_db (void)
{
  _dbus_assert (database_locked);
    
  if (system_db == NULL)
    {
      DBusError error = DBUS_ERROR_INIT;
      const DBusUserInfo *info;
      
      system_db = _dbus_user_database_new ();
      if (system_db == NULL)
        return FALSE;

      if (!_dbus_user_database_get_uid (system_db,
                                        _dbus_getuid (),
                                        &info,
                                        &error))
        {
          _dbus_user_database_unref (system_db);
          system_db = NULL;
          
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              return FALSE;
            }
          else
            {
              /* This really should not happen. */
              _dbus_warn ("Could not get password database information for UID of current process: %s",
                          error.message);
              dbus_error_free (&error);
              return FALSE;
            }
        }

      if (!_dbus_string_init (&process_username))
        {
          _dbus_user_database_unref (system_db);
          system_db = NULL;
          return FALSE;
        }

      if (!_dbus_string_init (&process_homedir))
        {
          _dbus_string_free (&process_username);
          _dbus_user_database_unref (system_db);
          system_db = NULL;
          return FALSE;
        }

      if (!_dbus_string_append (&process_username,
                                info->username) ||
          !_dbus_string_append (&process_homedir,
                                info->homedir) ||
          !_dbus_register_shutdown_func (shutdown_system_db, NULL))
        {
          _dbus_string_free (&process_username);
          _dbus_string_free (&process_homedir);
          _dbus_user_database_unref (system_db);
          system_db = NULL;
          return FALSE;
        }
    }

  return TRUE;
}

/**
 * Locks global system user database.
 */
dbus_bool_t
_dbus_user_database_lock_system (void)
{
  if (_DBUS_LOCK (system_users))
    {
      database_locked = TRUE;
      return TRUE;
    }
  else
    {
      return FALSE;
    }
}

/**
 * Unlocks global system user database.
 */
void
_dbus_user_database_unlock_system (void)
{
  database_locked = FALSE;
  _DBUS_UNLOCK (system_users);
}

/**
 * Gets the system global user database;
 * must be called with lock held (_dbus_user_database_lock_system()).
 *
 * @returns the database or #NULL if no memory
 */
DBusUserDatabase*
_dbus_user_database_get_system (void)
{
  _dbus_assert (database_locked);

  init_system_db ();
  
  return system_db;
}

/**
 * Flushes the system global user database;
 */
void
_dbus_user_database_flush_system (void)
{
  if (!_dbus_user_database_lock_system ())
    {
      /* nothing to flush */
      return;
    }

   if (system_db != NULL)
    _dbus_user_database_flush (system_db);

  _dbus_user_database_unlock_system ();
}

/**
 * Gets username of user owning current process.  The returned string
 * is valid until dbus_shutdown() is called.
 *
 * @param username place to store pointer to username
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_username_from_current_process (const DBusString **username)
{
  if (!_dbus_user_database_lock_system ())
    return FALSE;

  if (!init_system_db ())
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }
  *username = &process_username;
  _dbus_user_database_unlock_system ();  

  return TRUE;
}

/**
 * Gets homedir of user owning current process.  The returned string
 * is valid until dbus_shutdown() is called.
 *
 * @param homedir place to store pointer to homedir
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_homedir_from_current_process (const DBusString  **homedir)
{
  if (!_dbus_user_database_lock_system ())
    return FALSE;

  if (!init_system_db ())
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }
  *homedir = &process_homedir;
  _dbus_user_database_unlock_system ();

  return TRUE;
}

/**
 * Gets the home directory for the given user.
 *
 * @param uid the uid
 * @param homedir string to append home directory to
 * @returns #TRUE if user existed and we appended their homedir
 */
dbus_bool_t
_dbus_homedir_from_uid (dbus_uid_t         uid,
                        DBusString        *homedir)
{
  DBusUserDatabase *db;
  const DBusUserInfo *info;

  if (uid == _dbus_getuid () && uid == _dbus_geteuid ())
    {
      const char *from_environment;

      from_environment = _dbus_getenv ("HOME");

      if (from_environment != NULL)
        return _dbus_string_append (homedir, from_environment);
    }

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

  if (!_dbus_string_append (homedir, info->homedir))
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }
  
  _dbus_user_database_unlock_system ();
  return TRUE;
}

/**
 * Adds the credentials corresponding to the given username.
 *
 * Used among other purposes to parses a desired identity provided
 * from a client in the auth protocol. On UNIX this means parsing a
 * UID, on Windows probably parsing an SID string.
 * 
 * @todo this is broken because it treats OOM and parse error
 * the same way. Needs a #DBusError.
 * 
 * @param credentials credentials to fill in 
 * @param username the username
 * @returns #TRUE if the username existed and we got some credentials
 */
dbus_bool_t
_dbus_credentials_add_from_user (DBusCredentials         *credentials,
                                 const DBusString        *username,
                                 DBusCredentialsAddFlags  flags,
                                 DBusError               *error)
{
  DBusUserDatabase *db;
  const DBusUserInfo *info;
  unsigned long uid = DBUS_UID_UNSET;

  /* Fast-path for the common case: if the "username" is all-numeric,
   * then it's a Unix uid. This is true regardless of whether that uid
   * exists in NSS or /etc/passwd or equivalent. */
  if (_dbus_is_a_number (username, &uid))
    {
      _DBUS_STATIC_ASSERT (sizeof (uid) == sizeof (dbus_uid_t));

      if (_dbus_credentials_add_unix_uid (credentials, uid))
        {
          return TRUE;
        }
      else
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }
    }

  /* If we aren't allowed to look in NSS or /etc/passwd, fail now. */
  if (!(flags & DBUS_CREDENTIALS_ADD_FLAGS_USER_DATABASE))
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Expected a numeric Unix uid");
      return FALSE;
    }

  if (!_dbus_user_database_lock_system ())
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  db = _dbus_user_database_get_system ();
  if (db == NULL)
    {
      _dbus_user_database_unlock_system ();
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_user_database_get_username (db, username,
                                         &info, error))
    {
      _dbus_user_database_unlock_system ();
      return FALSE;
    }

  if (!_dbus_credentials_add_unix_uid(credentials, info->uid))
    {
      _dbus_user_database_unlock_system ();
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  _dbus_user_database_unlock_system ();
  return TRUE;
}

/**
 * Creates a new user database object used to look up and
 * cache user information.
 * @returns new database, or #NULL on out of memory
 */
DBusUserDatabase*
_dbus_user_database_new (void)
{
  DBusUserDatabase *db;
  
  db = dbus_new0 (DBusUserDatabase, 1);
  if (db == NULL)
    return NULL;

  db->refcount = 1;

  db->users = _dbus_hash_table_new (DBUS_HASH_UINTPTR,
                                    NULL, (DBusFreeFunction) _dbus_user_info_unref);
  
  if (db->users == NULL)
    goto failed;

  db->groups = _dbus_hash_table_new (DBUS_HASH_UINTPTR,
                                     NULL, (DBusFreeFunction) _dbus_group_info_unref);
  
  if (db->groups == NULL)
    goto failed;

  db->users_by_name = _dbus_hash_table_new (DBUS_HASH_STRING,
                                            NULL, (DBusFreeFunction) _dbus_user_info_unref);
  if (db->users_by_name == NULL)
    goto failed;
  
  db->groups_by_name = _dbus_hash_table_new (DBUS_HASH_STRING,
                                             NULL, (DBusFreeFunction) _dbus_group_info_unref);
  if (db->groups_by_name == NULL)
    goto failed;
  
  return db;
  
 failed:
  _dbus_user_database_unref (db);
  return NULL;
}

/**
 * Flush all information out of the user database. 
 */
void
_dbus_user_database_flush (DBusUserDatabase *db) 
{
  _dbus_hash_table_remove_all(db->users_by_name);
  _dbus_hash_table_remove_all(db->groups_by_name);
  _dbus_hash_table_remove_all(db->users);
  _dbus_hash_table_remove_all(db->groups);
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Increments refcount of user database.
 * @param db the database
 * @returns the database
 */
DBusUserDatabase *
_dbus_user_database_ref (DBusUserDatabase  *db)
{
  _dbus_assert (db->refcount > 0);

  db->refcount += 1;

  return db;
}
#endif /* DBUS_ENABLE_EMBEDDED_TESTS */

/**
 * Decrements refcount of user database.
 * @param db the database
 */
void
_dbus_user_database_unref (DBusUserDatabase  *db)
{
  _dbus_assert (db->refcount > 0);

  db->refcount -= 1;
  if (db->refcount == 0)
    {
      if (db->users)
        _dbus_hash_table_unref (db->users);

      if (db->groups)
        _dbus_hash_table_unref (db->groups);

      if (db->users_by_name)
        _dbus_hash_table_unref (db->users_by_name);

      if (db->groups_by_name)
        _dbus_hash_table_unref (db->groups_by_name);
      
      dbus_free (db);
    }
}

/**
 * Gets the user information for the given UID,
 * returned user info should not be freed. 
 *
 * @param db user database
 * @param uid the user ID
 * @param info return location for const ref to user info
 * @param error error location
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_user_database_get_uid (DBusUserDatabase    *db,
                             dbus_uid_t           uid,
                             const DBusUserInfo **info,
                             DBusError           *error)
{
  *info = _dbus_user_database_lookup (db, uid, NULL, error);
  return *info != NULL;
}

/**
 * Gets the user information for the given username.
 *
 * @param db user database
 * @param username the user name
 * @param info return location for const ref to user info
 * @param error error location
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_user_database_get_username  (DBusUserDatabase     *db,
                                   const DBusString     *username,
                                   const DBusUserInfo  **info,
                                   DBusError            *error)
{
  *info = _dbus_user_database_lookup (db, DBUS_UID_UNSET, username, error);
  return *info != NULL;
}

/** @} */

/* Tests in dbus-userdb-util.c */
