/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-keyring.c Store secret cookies in your homedir
 *
 * Copyright (C) 2003, 2004  Red Hat Inc.
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
#include "dbus-keyring.h"
#include "dbus-protocol.h"
#include <dbus/dbus-string.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-sysdeps.h>
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusKeyring keyring class
 * @ingroup  DBusInternals
 * @brief DBusKeyring data structure
 *
 * Types and functions related to DBusKeyring. DBusKeyring is intended
 * to manage cookies used to authenticate clients to servers.  This is
 * essentially the "verify that client can read the user's homedir"
 * authentication mechanism.  Both client and server must have access
 * to the homedir.
 *
 * The secret keys are not kept in locked memory, and are written to a
 * file in the user's homedir. However they are transient (only used
 * by a single server instance for a fixed period of time, then
 * discarded). Also, the keys are not sent over the wire.
 *
 * @todo there's a memory leak on some codepath in here, I saw it once
 * when running make check - probably some specific initial cookies
 * present in the cookie file, then depending on what we do with them.
 */

/**
 * @defgroup DBusKeyringInternals DBusKeyring implementation details
 * @ingroup  DBusInternals
 * @brief DBusKeyring implementation details
 *
 * The guts of DBusKeyring.
 *
 * @{
 */

/** The maximum age of a key before we create a new key to use in
 * challenges.  This isn't super-reliably enforced, since system
 * clocks can change or be wrong, but we make a best effort to only
 * use keys for a short time.
 */
#define NEW_KEY_TIMEOUT_SECONDS     (60*5)
/**
 * The time after which we drop a key from the secrets file.
 * The EXPIRE_KEYS_TIMEOUT_SECONDS - NEW_KEY_TIMEOUT_SECONDS is the minimum
 * time window a client has to complete authentication.
 */
#define EXPIRE_KEYS_TIMEOUT_SECONDS (NEW_KEY_TIMEOUT_SECONDS + (60*2))
/**
 * The maximum amount of time a key can be in the future.
 */
#define MAX_TIME_TRAVEL_SECONDS (60*5)

/**
 * Maximum number of keys in the keyring before
 * we just ignore the rest
 */
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#define MAX_KEYS_IN_FILE 10
#else
#define MAX_KEYS_IN_FILE 256
#endif

/**
 * A single key from the cookie file
 */
typedef struct
{
  dbus_int32_t id; /**< identifier used to refer to the key */

  long creation_time; /**< when the key was generated,
                       *   as unix timestamp. signed long
                       *   matches struct timeval.
                       */
  
  DBusString secret; /**< the actual key */

} DBusKey;

/**
 * @brief Internals of DBusKeyring.
 * 
 * DBusKeyring internals. DBusKeyring is an opaque object, it must be
 * used via accessor functions.
 */
struct DBusKeyring
{
  int refcount;             /**< Reference count */
  DBusString directory;     /**< Directory the below two items are inside */
  DBusString filename;      /**< Keyring filename */
  DBusString filename_lock; /**< Name of lockfile */
  DBusKey *keys; /**< Keys loaded from the file */
  int n_keys;    /**< Number of keys */
  DBusCredentials *credentials; /**< Credentials containing user the keyring is for */
};

static DBusKeyring*
_dbus_keyring_new (void)
{
  DBusKeyring *keyring;

  keyring = dbus_new0 (DBusKeyring, 1);
  if (keyring == NULL)
    goto out_0;
  
  if (!_dbus_string_init (&keyring->directory))
    goto out_1;

  if (!_dbus_string_init (&keyring->filename))
    goto out_2;

  if (!_dbus_string_init (&keyring->filename_lock))
    goto out_3;
  
  keyring->refcount = 1;
  keyring->keys = NULL;
  keyring->n_keys = 0;

  return keyring;

 out_3:
  _dbus_string_free (&keyring->filename);
 out_2:
  _dbus_string_free (&keyring->directory);
 out_1:
  dbus_free (keyring);
 out_0:
  return NULL;
}

static void
free_keys (DBusKey *keys,
           int      n_keys)
{
  int i;

  /* should be safe for args NULL, 0 */
  
  i = 0;
  while (i < n_keys)
    {
      _dbus_string_free (&keys[i].secret);
      ++i;
    }

  dbus_free (keys);
}

/* Our locking scheme is highly unreliable.  However, there is
 * unfortunately no reliable locking scheme in user home directories;
 * between bugs in Linux NFS, people using Tru64 or other total crap
 * NFS, AFS, random-file-system-of-the-week, and so forth, fcntl() in
 * homedirs simply generates tons of bug reports. This has been
 * learned through hard experience with GConf, unfortunately.
 *
 * This bad hack might work better for the kind of lock we have here,
 * which we don't expect to hold for any length of time.  Crashing
 * while we hold it should be unlikely, and timing out such that we
 * delete a stale lock should also be unlikely except when the
 * filesystem is running really slowly.  Stuff might break in corner
 * cases but as long as it's not a security-level breakage it should
 * be OK.
 */

/** Maximum number of timeouts waiting for lock before we decide it's stale */
#define MAX_LOCK_TIMEOUTS 32
/** Length of each timeout while waiting for a lock */
#define LOCK_TIMEOUT_MILLISECONDS 250

static dbus_bool_t
_dbus_keyring_lock (DBusKeyring *keyring)
{
  int n_timeouts;
  
  n_timeouts = 0;
  while (n_timeouts < MAX_LOCK_TIMEOUTS)
    {
      DBusError error = DBUS_ERROR_INIT;

      if (_dbus_create_file_exclusively (&keyring->filename_lock,
                                         &error))
        break;

      _dbus_verbose ("Did not get lock file, sleeping %d milliseconds (%s)\n",
                     LOCK_TIMEOUT_MILLISECONDS, error.message);
      dbus_error_free (&error);

      _dbus_sleep_milliseconds (LOCK_TIMEOUT_MILLISECONDS);
      
      ++n_timeouts;
    }

  if (n_timeouts == MAX_LOCK_TIMEOUTS)
    {
      DBusError error = DBUS_ERROR_INIT;

      _dbus_verbose ("Lock file timed out %d times, assuming stale\n",
                     n_timeouts);

      if (!_dbus_delete_file (&keyring->filename_lock, &error))
        {
          _dbus_verbose ("Couldn't delete old lock file: %s\n",
                         error.message);
          dbus_error_free (&error);
          return FALSE;
        }

      if (!_dbus_create_file_exclusively (&keyring->filename_lock,
                                          &error))
        {
          _dbus_verbose ("Couldn't create lock file after deleting stale one: %s\n",
                         error.message);
          dbus_error_free (&error);
          return FALSE;
        }
    }
  
  return TRUE;
}

static void
_dbus_keyring_unlock (DBusKeyring *keyring)
{
  DBusError error = DBUS_ERROR_INIT;

  if (!_dbus_delete_file (&keyring->filename_lock, &error))
    {
      _dbus_warn ("Failed to delete lock file: %s",
                  error.message);
      dbus_error_free (&error);
    }
}

static DBusKey*
find_key_by_id (DBusKey *keys,
                int      n_keys,
                int      id)
{
  int i;

  i = 0;
  while (i < n_keys)
    {
      if (keys[i].id == id)
        return &keys[i];
      
      ++i;
    }

  return NULL;
}

static dbus_bool_t
add_new_key (DBusKey  **keys_p,
             int       *n_keys_p,
             DBusError *error)
{
  DBusKey *new;
  DBusString bytes;
  int id;
  long timestamp;
  const unsigned char *s;
  dbus_bool_t retval;
  DBusKey *keys;
  int n_keys;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  if (!_dbus_string_init (&bytes))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  keys = *keys_p;
  n_keys = *n_keys_p;
  retval = FALSE;
      
  /* Generate an integer ID and then the actual key. */
 retry:
      
  if (!_dbus_generate_random_bytes (&bytes, 4, error))
    goto out;

  s = (const unsigned char*) _dbus_string_get_const_data (&bytes);
      
  id = s[0] | (s[1] << 8) | (s[2] << 16) | ((s[3] & 0x7f) << 24);
  _dbus_assert (id >= 0);

  if (find_key_by_id (keys, n_keys, id) != NULL)
    {
      _dbus_string_set_length (&bytes, 0);
      _dbus_verbose ("Key ID %d already existed, trying another one\n",
                     id);
      goto retry;
    }

  _dbus_verbose ("Creating key with ID %d\n", id);
      
#define KEY_LENGTH_BYTES 24
  _dbus_string_set_length (&bytes, 0);
  if (!_dbus_generate_random_bytes (&bytes, KEY_LENGTH_BYTES, error))
    {
      goto out;
    }

  new = dbus_realloc (keys, sizeof (DBusKey) * (n_keys + 1));
  if (new == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto out;
    }

  keys = new;
  *keys_p = keys; /* otherwise *keys_p ends up invalid */
  n_keys += 1;

  if (!_dbus_string_init (&keys[n_keys-1].secret))
    {
      n_keys -= 1; /* we don't want to free the one we didn't init */
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto out;
    }

  _dbus_get_real_time (&timestamp, NULL);
      
  keys[n_keys-1].id = id;
  keys[n_keys-1].creation_time = timestamp;
  if (!_dbus_string_move (&bytes, 0,
                          &keys[n_keys-1].secret,
                          0))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&keys[n_keys-1].secret);
      n_keys -= 1;
      goto out;
    }
  
  retval = TRUE;
  
 out:
  *n_keys_p = n_keys;
  
  _dbus_string_free (&bytes);
  return retval;
}

/**
 * Reloads the keyring file, optionally adds one new key to the file,
 * removes all expired keys from the file iff a key was added, then
 * resaves the file.  Stores the keys from the file in keyring->keys.
 * Note that the file is only resaved (written to) if a key is added,
 * this means that only servers ever write to the file and need to
 * lock it, which avoids a lot of lock contention at login time and
 * such.
 *
 * @param keyring the keyring
 * @param add_new #TRUE to add a new key to the file, expire keys, and resave
 * @param error return location for errors
 * @returns #FALSE on failure
 */
static dbus_bool_t
_dbus_keyring_reload (DBusKeyring *keyring,
                      dbus_bool_t  add_new,
                      DBusError   *error)
{
  DBusString contents;
  DBusString line;
  dbus_bool_t retval;
  dbus_bool_t have_lock;
  DBusKey *keys;
  int n_keys;
  int i;
  long now;
  DBusError tmp_error;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  if (!_dbus_check_dir_is_private_to_user (&keyring->directory, error))
    return FALSE;
    
  if (!_dbus_string_init (&contents))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  if (!_dbus_string_init (&line))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&contents);
      return FALSE;
    }
   
  keys = NULL;
  n_keys = 0;
  retval = FALSE;
  have_lock = FALSE;

  _dbus_get_real_time (&now, NULL);
  
  if (add_new)
    {
      if (!_dbus_keyring_lock (keyring))
        {
          dbus_set_error (error, DBUS_ERROR_FAILED,
                          "Could not lock keyring file to add to it");
          goto out;
        }

      have_lock = TRUE;
    }

  dbus_error_init (&tmp_error);
  if (!_dbus_file_get_contents (&contents, 
                                &keyring->filename,
                                &tmp_error))
    {
      _dbus_verbose ("Failed to load keyring file: %s\n",
                     tmp_error.message);
      /* continue with empty keyring file, so we recreate it */
      dbus_error_free (&tmp_error);
    }

  if (!_dbus_string_validate_ascii (&contents, 0,
                                    _dbus_string_get_length (&contents)))
    {
      _dbus_warn ("Secret keyring file contains non-ASCII! Ignoring existing contents");
      _dbus_string_set_length (&contents, 0);
    }

  /* FIXME this is badly inefficient for large keyring files
   * (not that large keyring files exist outside of test suites)
   */
  while (_dbus_string_pop_line (&contents, &line))
    {
      int next;
      long val;
      int id;
      long timestamp;
      int len;
      int end;
      DBusKey *new;

      /* Don't load more than the max. */
      if (n_keys >= (add_new ? MAX_KEYS_IN_FILE - 1 : MAX_KEYS_IN_FILE))
        break;
      
      next = 0;
      if (!_dbus_string_parse_int (&line, 0, &val, &next))
        {
          _dbus_verbose ("could not parse secret key ID at start of line\n");
          continue;
        }

      if (val > _DBUS_INT32_MAX || val < 0)
        {
          _dbus_verbose ("invalid secret key ID at start of line\n");
          continue;
        }
      
      id = val;

      _dbus_string_skip_blank (&line, next, &next);
      
      if (!_dbus_string_parse_int (&line, next, &timestamp, &next))
        {
          _dbus_verbose ("could not parse secret key timestamp\n");
          continue;
        }

      if (timestamp < 0 ||
          (now + MAX_TIME_TRAVEL_SECONDS) < timestamp ||
          (now - EXPIRE_KEYS_TIMEOUT_SECONDS) > timestamp)
        {
          _dbus_verbose ("dropping/ignoring %ld-seconds old key with timestamp %ld as current time is %ld\n",
                         now - timestamp, timestamp, now);
          continue;
        }
      
      _dbus_string_skip_blank (&line, next, &next);

      len = _dbus_string_get_length (&line);

      if ((len - next) == 0)
        {
          _dbus_verbose ("no secret key after ID and timestamp\n");
          continue;
        }
      
      /* We have all three parts */
      new = dbus_realloc (keys, sizeof (DBusKey) * (n_keys + 1));
      if (new == NULL)
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          goto out;
        }

      keys = new;
      n_keys += 1;

      if (!_dbus_string_init (&keys[n_keys-1].secret))
        {
          n_keys -= 1; /* we don't want to free the one we didn't init */
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          goto out;
        }
      
      keys[n_keys-1].id = id;
      keys[n_keys-1].creation_time = timestamp;
      if (!_dbus_string_hex_decode (&line, next, &end,
                                    &keys[n_keys-1].secret, 0))
	{
	  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
	  goto out;
	}

      if (_dbus_string_get_length (&line) != end)
	{
	  _dbus_verbose ("invalid hex encoding in keyring file\n");
	  _dbus_string_free (&keys[n_keys - 1].secret);
	  n_keys -= 1;
	  continue;
	}
    }

  _dbus_verbose ("Successfully loaded %d existing keys\n",
                 n_keys);

  if (add_new)
    {
      if (!add_new_key (&keys, &n_keys, error))
        {
          _dbus_verbose ("Failed to generate new key: %s\n",
                         error ? error->message : "(unknown)");
          goto out;
        }

      _dbus_string_set_length (&contents, 0);

      i = 0;
      while (i < n_keys)
        {
          if (!_dbus_string_append_int (&contents,
                                        keys[i].id))
            goto nomem;

          if (!_dbus_string_append_byte (&contents, ' '))
            goto nomem;

          if (!_dbus_string_append_int (&contents,
                                        keys[i].creation_time))
            goto nomem;

          if (!_dbus_string_append_byte (&contents, ' '))
            goto nomem;

          if (!_dbus_string_hex_encode (&keys[i].secret, 0,
                                        &contents,
                                        _dbus_string_get_length (&contents)))
            goto nomem;

          if (!_dbus_string_append_byte (&contents, '\n'))
            goto nomem;          
          
          ++i;
          continue;

        nomem:
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          goto out;
        }
      
      if (!_dbus_string_save_to_file (&contents, &keyring->filename,
                                      FALSE, error))
        goto out;
    }

  if (keyring->keys)
    free_keys (keyring->keys, keyring->n_keys);
  keyring->keys = keys;
  keyring->n_keys = n_keys;
  keys = NULL;
  n_keys = 0;
  
  retval = TRUE;  
  
 out:
  if (have_lock)
    _dbus_keyring_unlock (keyring);
  
  if (! ((retval == TRUE && (error == NULL || error->name == NULL)) ||
         (retval == FALSE && (error == NULL || error->name != NULL))))
    {
      if (error && error->name)
        _dbus_verbose ("error is %s: %s\n", error->name, error->message);
      _dbus_warn ("returning %d but error pointer %p name %s",
                  retval, error, error->name ? error->name : "(none)");
      _dbus_assert_not_reached ("didn't handle errors properly");
    }
  
  if (keys != NULL)
    {
      i = 0;
      while (i < n_keys)
        {
          _dbus_string_zero (&keys[i].secret);
          _dbus_string_free (&keys[i].secret);
          ++i;
        }

      dbus_free (keys);
    }
  
  _dbus_string_free (&contents);
  _dbus_string_free (&line);

  return retval;
}

/** @} */ /* end of internals */

/**
 * @addtogroup DBusKeyring
 *
 * @{
 */

/**
 * Increments reference count of the keyring
 *
 * @param keyring the keyring
 * @returns the keyring
 */
DBusKeyring *
_dbus_keyring_ref (DBusKeyring *keyring)
{
  keyring->refcount += 1;

  return keyring;
}

/**
 * Decrements refcount and finalizes if it reaches
 * zero.
 *
 * @param keyring the keyring
 */
void
_dbus_keyring_unref (DBusKeyring *keyring)
{
  keyring->refcount -= 1;

  if (keyring->refcount == 0)
    {
      if (keyring->credentials)
        _dbus_credentials_unref (keyring->credentials);

      _dbus_string_free (&keyring->filename);
      _dbus_string_free (&keyring->filename_lock);
      _dbus_string_free (&keyring->directory);
      free_keys (keyring->keys, keyring->n_keys);
      dbus_free (keyring);      
    }
}

/**
 * Creates a new keyring that lives in the ~/.dbus-keyrings directory
 * of the user represented by @p credentials. If the @p credentials are
 * #NULL or empty, uses those of the current process.
 *
 * @param credentials a set of credentials representing a user or #NULL
 * @param context which keyring to get
 * @param error return location for errors
 * @returns the keyring or #NULL on error
 */
DBusKeyring*
_dbus_keyring_new_for_credentials (DBusCredentials  *credentials,
                                   const DBusString *context,
                                   DBusError        *error)
{
  DBusString ringdir;
  DBusKeyring *keyring;
  dbus_bool_t error_set;
  DBusError tmp_error;
  DBusCredentials *our_credentials;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (_dbus_check_setuid ())
    {
      dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
                            "Unable to create DBus keyring when setuid");
      return NULL;
    }
  
  keyring = NULL;
  error_set = FALSE;
  our_credentials = NULL;
  
  if (!_dbus_string_init (&ringdir))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if (credentials != NULL)
    {
      our_credentials = _dbus_credentials_copy (credentials);
    }
  else
    {
      our_credentials = _dbus_credentials_new_from_current_process ();
    }
  
  if (our_credentials == NULL)
    goto failed;

  if (_dbus_credentials_are_anonymous (our_credentials))
    {
      if (!_dbus_credentials_add_from_current_process (our_credentials))
        goto failed;
    }
  
  if (!_dbus_append_keyring_directory_for_credentials (&ringdir,
                                                       our_credentials))
    goto failed;
  
  keyring = _dbus_keyring_new ();
  if (keyring == NULL)
    goto failed;

  _dbus_assert (keyring->credentials == NULL);
  keyring->credentials = our_credentials;
  our_credentials = NULL; /* so we don't unref it again later */
  
  /* should have been validated already, but paranoia check here */
  if (!_dbus_keyring_validate_context (context))
    {
      error_set = TRUE;
      dbus_set_error_const (error,
                            DBUS_ERROR_FAILED,
                            "Invalid context in keyring creation");
      goto failed;
    }

  /* Save keyring dir in the keyring object */
  if (!_dbus_string_copy (&ringdir, 0,
                          &keyring->directory, 0))
    goto failed;  

  /* Create keyring->filename based on keyring dir and context */
  if (!_dbus_string_copy (&keyring->directory, 0,
                          &keyring->filename, 0))
    goto failed;

  if (!_dbus_concat_dir_and_file (&keyring->filename,
                                  context))
    goto failed;

  /* Create lockfile name */
  if (!_dbus_string_copy (&keyring->filename, 0,
                          &keyring->filename_lock, 0))
    goto failed;

  if (!_dbus_string_append (&keyring->filename_lock, ".lock"))
    goto failed;

  /* Reload keyring */
  dbus_error_init (&tmp_error);
  if (!_dbus_keyring_reload (keyring, FALSE, &tmp_error))
    {
      _dbus_verbose ("didn't load an existing keyring: %s\n",
                     tmp_error.message);
      dbus_error_free (&tmp_error);
    }
  
  /* We don't fail fatally if we can't create the directory,
   * but the keyring will probably always be empty
   * unless someone else manages to create it
   */
  dbus_error_init (&tmp_error);
  if (!_dbus_ensure_directory (&keyring->directory,
                               &tmp_error))
    {
      _dbus_verbose ("Creating keyring directory: %s\n",
                     tmp_error.message);
      dbus_error_free (&tmp_error);
    }

  _dbus_string_free (&ringdir);
  
  return keyring;
  
 failed:
  if (!error_set)
    dbus_set_error_const (error,
                          DBUS_ERROR_NO_MEMORY,
                          NULL);
  if (our_credentials)
    _dbus_credentials_unref (our_credentials);
  if (keyring)
    _dbus_keyring_unref (keyring);
  _dbus_string_free (&ringdir);
  return NULL;

}

/**
 * Checks whether the context is a valid context.
 * Contexts that might cause confusion when used
 * in filenames are not allowed (contexts can't
 * start with a dot or contain dir separators).
 *
 * @todo this is the most inefficient implementation
 * imaginable.
 *
 * @param context the context
 * @returns #TRUE if valid
 */
dbus_bool_t
_dbus_keyring_validate_context (const DBusString *context)
{
  if (_dbus_string_get_length (context) == 0)
    {
      _dbus_verbose ("context is zero-length\n");
      return FALSE;
    }

  if (!_dbus_string_validate_ascii (context, 0,
                                    _dbus_string_get_length (context)))
    {
      _dbus_verbose ("context not valid ascii\n");
      return FALSE;
    }
  
  /* no directory separators */  
  if (_dbus_string_find (context, 0, "/", NULL))
    {
      _dbus_verbose ("context contains a slash\n");
      return FALSE;
    }

  if (_dbus_string_find (context, 0, "\\", NULL))
    {
      _dbus_verbose ("context contains a backslash\n");
      return FALSE;
    }

  /* prevent attempts to use dotfiles or ".." or ".lock"
   * all of which might allow some kind of attack
   */
  if (_dbus_string_find (context, 0, ".", NULL))
    {
      _dbus_verbose ("context contains a dot\n");
      return FALSE;
    }

  /* no spaces/tabs, those are used for separators in the protocol */
  if (_dbus_string_find_blank (context, 0, NULL))
    {
      _dbus_verbose ("context contains a blank\n");
      return FALSE;
    }

  if (_dbus_string_find (context, 0, "\n", NULL))
    {
      _dbus_verbose ("context contains a newline\n");
      return FALSE;
    }

  if (_dbus_string_find (context, 0, "\r", NULL))
    {
      _dbus_verbose ("context contains a carriage return\n");
      return FALSE;
    }
  
  return TRUE;
}

static DBusKey*
find_recent_key (DBusKeyring *keyring)
{
  int i;
  long tv_sec, tv_usec;

  _dbus_get_real_time (&tv_sec, &tv_usec);
  
  i = 0;
  while (i < keyring->n_keys)
    {
      DBusKey *key = &keyring->keys[i];

      _dbus_verbose ("Key %d is %ld seconds old\n",
                     i, tv_sec - key->creation_time);
      
      if ((tv_sec - NEW_KEY_TIMEOUT_SECONDS) < key->creation_time)
        return key;
      
      ++i;
    }

  return NULL;
}

/**
 * Gets a recent key to use for authentication.
 * If no recent key exists, creates one. Returns
 * the key ID. If a key can't be written to the keyring
 * file so no recent key can be created, returns -1.
 * All valid keys are > 0.
 *
 * @param keyring the keyring
 * @param error error on failure
 * @returns key ID to use for auth, or -1 on failure
 */
int
_dbus_keyring_get_best_key (DBusKeyring  *keyring,
                            DBusError    *error)
{
  DBusKey *key;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  key = find_recent_key (keyring);
  if (key)
    return key->id;

  /* All our keys are too old, or we've never loaded the
   * keyring. Create a new one.
   */
  if (!_dbus_keyring_reload (keyring, TRUE,
                             error))
    return -1;

  key = find_recent_key (keyring);
  if (key)
    return key->id;
  else
    {
      dbus_set_error_const (error,
                            DBUS_ERROR_FAILED,
                            "No recent-enough key found in keyring, and unable to create a new key");
      return -1;
    }
}

/**
 * Checks whether the keyring is for the same user as the given credentials.
 *
 * @param keyring the keyring
 * @param credentials the credentials to check
 *
 * @returns #TRUE if the keyring belongs to the given user
 */
dbus_bool_t
_dbus_keyring_is_for_credentials (DBusKeyring           *keyring,
                                  DBusCredentials       *credentials)
{
  return _dbus_credentials_same_user (keyring->credentials,
                                      credentials);
}

/**
 * Gets the hex-encoded secret key for the given ID.
 * Returns #FALSE if not enough memory. Returns #TRUE
 * but empty key on any other error such as unknown
 * key ID.
 *
 * @param keyring the keyring
 * @param key_id the key ID
 * @param hex_key string to append hex-encoded key to
 * @returns #TRUE if we had enough memory
 */
dbus_bool_t
_dbus_keyring_get_hex_key (DBusKeyring       *keyring,
                           int                key_id,
                           DBusString        *hex_key)
{
  DBusKey *key;

  key = find_key_by_id (keyring->keys,
                        keyring->n_keys,
                        key_id);
  if (key == NULL)
    return TRUE; /* had enough memory, so TRUE */

  return _dbus_string_hex_encode (&key->secret, 0,
                                  hex_key,
                                  _dbus_string_get_length (hex_key));
}

/** @} */ /* end of exposed API */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "dbus-test.h"
#include <stdio.h>

dbus_bool_t
_dbus_keyring_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  DBusString context;
  DBusKeyring *ring1;
  DBusKeyring *ring2;
  int id;
  DBusError error;
  int i;

  ring1 = NULL;
  ring2 = NULL;
  
  /* Context validation */
  
  _dbus_string_init_const (&context, "foo");
  _dbus_assert (_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "org_freedesktop_blah");
  _dbus_assert (_dbus_keyring_validate_context (&context));
  
  _dbus_string_init_const (&context, "");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, ".foo");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "bar.foo");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "bar/foo");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "bar\\foo");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "foo\xfa\xf0");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "foo\x80");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "foo\x7f");
  _dbus_assert (_dbus_keyring_validate_context (&context));
  _dbus_string_init_const (&context, "foo bar");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  
  if (!_dbus_string_init (&context))
    _dbus_test_fatal ("no memory");
  if (!_dbus_string_append_byte (&context, '\0'))
    _dbus_test_fatal ("no memory");
  _dbus_assert (!_dbus_keyring_validate_context (&context));
  _dbus_string_free (&context);

  /* Now verify that if we create a key in keyring 1,
   * it is properly loaded in keyring 2
   */

  _dbus_string_init_const (&context, "org_freedesktop_dbus_testsuite");
  dbus_error_init (&error);
  ring1 = _dbus_keyring_new_for_credentials (NULL, &context,
                                             &error);
  _dbus_assert (ring1 != NULL);
  _dbus_assert (error.name == NULL);

  id = _dbus_keyring_get_best_key (ring1, &error);
  if (id < 0)
    {
      fprintf (stderr, "Could not load keyring: %s\n", error.message);
      dbus_error_free (&error);
      goto failure;
    }

  ring2 = _dbus_keyring_new_for_credentials (NULL, &context, &error);
  _dbus_assert (ring2 != NULL);
  _dbus_assert (error.name == NULL);
  
  if (ring1->n_keys != ring2->n_keys)
    {
      fprintf (stderr, "Different number of keys in keyrings\n");
      goto failure;
    }

  /* We guarantee we load and save keeping keys in a fixed
   * order
   */
  i = 0;
  while (i < ring1->n_keys)
    {
      if (ring1->keys[i].id != ring2->keys[i].id)
        {
          fprintf (stderr, "Keyring 1 has first key ID %d and keyring 2 has %d\n",
                   ring1->keys[i].id, ring2->keys[i].id);
          goto failure;
        }      

      if (ring1->keys[i].creation_time != ring2->keys[i].creation_time)
        {
          fprintf (stderr, "Keyring 1 has first key time %ld and keyring 2 has %ld\n",
                   ring1->keys[i].creation_time, ring2->keys[i].creation_time);
          goto failure;
        }

      if (!_dbus_string_equal (&ring1->keys[i].secret,
                               &ring2->keys[i].secret))
        {
          fprintf (stderr, "Keyrings 1 and 2 have different secrets for same ID/timestamp\n");
          goto failure;
        }
      
      ++i;
    }

  _dbus_test_diag (" %d keys in test", ring1->n_keys);

  /* Test ref/unref */
  _dbus_keyring_ref (ring1);
  _dbus_keyring_ref (ring2);
  _dbus_keyring_unref (ring1);
  _dbus_keyring_unref (ring2);


  /* really unref */
  _dbus_keyring_unref (ring1);
  _dbus_keyring_unref (ring2);
  
  return TRUE;

 failure:
  if (ring1)
    _dbus_keyring_unref (ring1);
  if (ring2)
    _dbus_keyring_unref (ring2);

  return FALSE;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
     
