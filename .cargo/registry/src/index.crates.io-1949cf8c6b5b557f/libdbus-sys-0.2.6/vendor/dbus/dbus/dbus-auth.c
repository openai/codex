/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-auth.c Authentication
 *
 * Copyright (C) 2002, 2003, 2004 Red Hat Inc.
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
#include "dbus-auth.h"
#include "dbus-string.h"
#include "dbus-list.h"
#include "dbus-internals.h"
#include "dbus-keyring.h"
#include "dbus-sha.h"
#include "dbus-protocol.h"
#include "dbus-credentials.h"

/**
 * @defgroup DBusAuth Authentication
 * @ingroup  DBusInternals
 * @brief DBusAuth object
 *
 * DBusAuth manages the authentication negotiation when a connection
 * is first established, and also manages any encryption used over a
 * connection.
 *
 * @todo some SASL profiles require sending the empty string as a
 * challenge/response, but we don't currently allow that in our
 * protocol.
 *
 * @todo right now sometimes both ends will block waiting for input
 * from the other end, e.g. if there's an error during
 * DBUS_COOKIE_SHA1.
 *
 * @todo the cookie keyring needs to be cached globally not just
 * per-auth (which raises threadsafety issues too)
 * 
 * @todo grep FIXME in dbus-auth.c
 */

/**
 * @defgroup DBusAuthInternals Authentication implementation details
 * @ingroup  DBusInternals
 * @brief DBusAuth implementation details
 *
 * Private details of authentication code.
 *
 * @{
 */

/**
 * This function appends an initial client response to the given string
 */
typedef dbus_bool_t (* DBusInitialResponseFunction)  (DBusAuth         *auth,
                                                      DBusString       *response);

/**
 * This function processes a block of data received from the peer.
 * i.e. handles a DATA command.
 */
typedef dbus_bool_t (* DBusAuthDataFunction)     (DBusAuth         *auth,
                                                  const DBusString *data);

/**
 * This function encodes a block of data from the peer.
 */
typedef dbus_bool_t (* DBusAuthEncodeFunction)   (DBusAuth         *auth,
                                                  const DBusString *data,
                                                  DBusString       *encoded);

/**
 * This function decodes a block of data from the peer.
 */
typedef dbus_bool_t (* DBusAuthDecodeFunction)   (DBusAuth         *auth,
                                                  const DBusString *data,
                                                  DBusString       *decoded);

/**
 * This function is called when the mechanism is abandoned.
 */
typedef void        (* DBusAuthShutdownFunction) (DBusAuth       *auth);

/**
 * Virtual table representing a particular auth mechanism.
 */
typedef struct
{
  const char *mechanism; /**< Name of the mechanism */
  DBusAuthDataFunction server_data_func; /**< Function on server side for DATA */
  DBusAuthEncodeFunction server_encode_func; /**< Function on server side to encode */
  DBusAuthDecodeFunction server_decode_func; /**< Function on server side to decode */
  DBusAuthShutdownFunction server_shutdown_func; /**< Function on server side to shut down */
  DBusInitialResponseFunction client_initial_response_func; /**< Function on client side to handle initial response */
  DBusAuthDataFunction client_data_func; /**< Function on client side for DATA */
  DBusAuthEncodeFunction client_encode_func; /**< Function on client side for encode */
  DBusAuthDecodeFunction client_decode_func; /**< Function on client side for decode */
  DBusAuthShutdownFunction client_shutdown_func; /**< Function on client side for shutdown */
} DBusAuthMechanismHandler;

/**
 * Enumeration for the known authentication commands.
 */
typedef enum {
  DBUS_AUTH_COMMAND_AUTH,
  DBUS_AUTH_COMMAND_CANCEL,
  DBUS_AUTH_COMMAND_DATA,
  DBUS_AUTH_COMMAND_BEGIN,
  DBUS_AUTH_COMMAND_REJECTED,
  DBUS_AUTH_COMMAND_OK,
  DBUS_AUTH_COMMAND_ERROR,
  DBUS_AUTH_COMMAND_UNKNOWN,
  DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD,
  DBUS_AUTH_COMMAND_AGREE_UNIX_FD
} DBusAuthCommand;

/**
 * Auth state function, determines the reaction to incoming events for
 * a particular state. Returns whether we had enough memory to
 * complete the operation.
 */
typedef dbus_bool_t (* DBusAuthStateFunction) (DBusAuth         *auth,
                                               DBusAuthCommand   command,
                                               const DBusString *args);

/**
 * Information about a auth state.
 */
typedef struct
{
  const char *name;               /**< Name of the state */
  DBusAuthStateFunction handler;  /**< State function for this state */
} DBusAuthStateData;

/**
 * Internal members of DBusAuth.
 */
struct DBusAuth
{
  int refcount;           /**< reference count */
  const char *side;       /**< Client or server */

  DBusString incoming;    /**< Incoming data buffer */
  DBusString outgoing;    /**< Outgoing data buffer */
  
  const DBusAuthStateData *state;         /**< Current protocol state */

  const DBusAuthMechanismHandler *mech;   /**< Current auth mechanism */

  DBusString identity;                   /**< Current identity we're authorizing
                                          *   as.
                                          */
  
  DBusCredentials *credentials;          /**< Credentials read from socket
                                          */

  DBusCredentials *authorized_identity; /**< Credentials that are authorized */

  DBusCredentials *desired_identity;    /**< Identity client has requested */
  
  DBusString context;               /**< Cookie scope */
  DBusKeyring *keyring;             /**< Keyring for cookie mechanism. */
  int cookie_id;                    /**< ID of cookie to use */
  DBusString challenge;             /**< Challenge sent to client */

  char **allowed_mechs;             /**< Mechanisms we're allowed to use,
                                     * or #NULL if we can use any
                                     */
  
  unsigned int needed_memory : 1;   /**< We needed memory to continue since last
                                     * successful getting something done
                                     */
  unsigned int already_got_mechanisms : 1;       /**< Client already got mech list */
  unsigned int already_asked_for_initial_response : 1; /**< Already sent a blank challenge to get an initial response */
  unsigned int buffer_outstanding : 1; /**< Buffer is "checked out" for reading data into */

  unsigned int unix_fd_possible : 1;  /**< This side could do unix fd passing */
  unsigned int unix_fd_negotiated : 1; /**< Unix fd was successfully negotiated */
};

/**
 * "Subclass" of DBusAuth for client side
 */
typedef struct
{
  DBusAuth base;    /**< Parent class */

  DBusList *mechs_to_try; /**< Mechanisms we got from the server that we're going to try using */

  DBusString guid_from_server; /**< GUID received from server */
  
} DBusAuthClient;

/**
 * "Subclass" of DBusAuth for server side.
 */
typedef struct
{
  DBusAuth base;    /**< Parent class */

  int failures;     /**< Number of times client has been rejected */
  int max_failures; /**< Number of times we reject before disconnect */

  DBusString guid;  /**< Our globally unique ID in hex encoding */
  
} DBusAuthServer;

static void        goto_state                (DBusAuth                       *auth,
                                              const DBusAuthStateData        *new_state);
static dbus_bool_t send_auth                 (DBusAuth *auth,
                                              const DBusAuthMechanismHandler *mech);
static dbus_bool_t send_data                 (DBusAuth *auth,
                                              DBusString *data);
static dbus_bool_t send_rejected             (DBusAuth *auth);
static dbus_bool_t send_error                (DBusAuth *auth,
                                              const char *message);
static dbus_bool_t send_ok                   (DBusAuth *auth);
static dbus_bool_t send_begin                (DBusAuth *auth);
static dbus_bool_t send_cancel               (DBusAuth *auth);
static dbus_bool_t send_negotiate_unix_fd    (DBusAuth *auth);
static dbus_bool_t send_agree_unix_fd        (DBusAuth *auth);

/**
 * Client states
 */
 
static dbus_bool_t handle_server_state_waiting_for_auth  (DBusAuth         *auth,
                                                          DBusAuthCommand   command,
                                                          const DBusString *args);
static dbus_bool_t handle_server_state_waiting_for_data  (DBusAuth         *auth,
                                                          DBusAuthCommand   command,
                                                          const DBusString *args);
static dbus_bool_t handle_server_state_waiting_for_begin (DBusAuth         *auth,
                                                          DBusAuthCommand   command,
                                                          const DBusString *args);
  
static const DBusAuthStateData server_state_waiting_for_auth = {
  "WaitingForAuth", handle_server_state_waiting_for_auth
};
static const DBusAuthStateData server_state_waiting_for_data = {
  "WaitingForData", handle_server_state_waiting_for_data
};
static const DBusAuthStateData server_state_waiting_for_begin = {
  "WaitingForBegin", handle_server_state_waiting_for_begin
};
  
/**
 * Client states
 */
 
static dbus_bool_t handle_client_state_waiting_for_data   (DBusAuth         *auth,
                                                           DBusAuthCommand   command,
                                                           const DBusString *args);
static dbus_bool_t handle_client_state_waiting_for_ok     (DBusAuth         *auth,
                                                           DBusAuthCommand   command,
                                                           const DBusString *args);
static dbus_bool_t handle_client_state_waiting_for_reject (DBusAuth         *auth,
                                                           DBusAuthCommand   command,
                                                           const DBusString *args);
static dbus_bool_t handle_client_state_waiting_for_agree_unix_fd (DBusAuth         *auth,
                                                           DBusAuthCommand   command,
                                                           const DBusString *args);

static const DBusAuthStateData client_state_need_send_auth = {
  "NeedSendAuth", NULL
};
static const DBusAuthStateData client_state_waiting_for_data = {
  "WaitingForData", handle_client_state_waiting_for_data
};
/* The WaitingForOK state doesn't appear to be used.
 * See https://bugs.freedesktop.org/show_bug.cgi?id=97298 */
_DBUS_GNUC_UNUSED
static const DBusAuthStateData client_state_waiting_for_ok = {
  "WaitingForOK", handle_client_state_waiting_for_ok
};
static const DBusAuthStateData client_state_waiting_for_reject = {
  "WaitingForReject", handle_client_state_waiting_for_reject
};
static const DBusAuthStateData client_state_waiting_for_agree_unix_fd = {
  "WaitingForAgreeUnixFD", handle_client_state_waiting_for_agree_unix_fd
};

/**
 * Common terminal states.  Terminal states have handler == NULL.
 */

static const DBusAuthStateData common_state_authenticated = {
  "Authenticated",  NULL
};

static const DBusAuthStateData common_state_need_disconnect = {
  "NeedDisconnect",  NULL
};

static const char auth_side_client[] = "client";
static const char auth_side_server[] = "server";
/**
 * @param auth the auth conversation
 * @returns #TRUE if the conversation is the server side
 */
#define DBUS_AUTH_IS_SERVER(auth) ((auth)->side == auth_side_server)
/**
 * @param auth the auth conversation
 * @returns #TRUE if the conversation is the client side
 */
#define DBUS_AUTH_IS_CLIENT(auth) ((auth)->side == auth_side_client)
/**
 * @param auth the auth conversation
 * @returns auth cast to DBusAuthClient
 */
#define DBUS_AUTH_CLIENT(auth)    ((DBusAuthClient*)(auth))
/**
 * @param auth the auth conversation
 * @returns auth cast to DBusAuthServer
 */
#define DBUS_AUTH_SERVER(auth)    ((DBusAuthServer*)(auth))

/**
 * The name of the auth ("client" or "server")
 * @param auth the auth conversation
 * @returns a string
 */
#define DBUS_AUTH_NAME(auth)      ((auth)->side)

static DBusAuth*
_dbus_auth_new (int size)
{
  DBusAuth *auth;
  
  auth = dbus_malloc0 (size);
  if (auth == NULL)
    return NULL;
  
  auth->refcount = 1;
  
  auth->keyring = NULL;
  auth->cookie_id = -1;
  
  /* note that we don't use the max string length feature,
   * because you can't use that feature if you're going to
   * try to recover from out-of-memory (it creates
   * what looks like unrecoverable inability to alloc
   * more space in the string). But we do handle
   * overlong buffers in _dbus_auth_do_work().
   */
  
  if (!_dbus_string_init (&auth->incoming))
    goto enomem_0;

  if (!_dbus_string_init (&auth->outgoing))
    goto enomem_1;
    
  if (!_dbus_string_init (&auth->identity))
    goto enomem_2;

  if (!_dbus_string_init (&auth->context))
    goto enomem_3;

  if (!_dbus_string_init (&auth->challenge))
    goto enomem_4;

  /* default context if none is specified */
  if (!_dbus_string_append (&auth->context, "org_freedesktop_general"))
    goto enomem_5;

  auth->credentials = _dbus_credentials_new ();
  if (auth->credentials == NULL)
    goto enomem_6;
  
  auth->authorized_identity = _dbus_credentials_new ();
  if (auth->authorized_identity == NULL)
    goto enomem_7;

  auth->desired_identity = _dbus_credentials_new ();
  if (auth->desired_identity == NULL)
    goto enomem_8;
  
  return auth;

#if 0
 enomem_9:
  _dbus_credentials_unref (auth->desired_identity);
#endif
 enomem_8:
  _dbus_credentials_unref (auth->authorized_identity);
 enomem_7:
  _dbus_credentials_unref (auth->credentials);
 enomem_6:
 /* last alloc was an append to context, which is freed already below */ ;
 enomem_5:
  _dbus_string_free (&auth->challenge);
 enomem_4:
  _dbus_string_free (&auth->context);
 enomem_3:
  _dbus_string_free (&auth->identity);
 enomem_2:
  _dbus_string_free (&auth->outgoing);
 enomem_1:
  _dbus_string_free (&auth->incoming);
 enomem_0:
  dbus_free (auth);
  return NULL;
}

static void
shutdown_mech (DBusAuth *auth)
{
  /* Cancel any auth */
  auth->already_asked_for_initial_response = FALSE;
  _dbus_string_set_length (&auth->identity, 0);

  _dbus_credentials_clear (auth->authorized_identity);
  _dbus_credentials_clear (auth->desired_identity);
  
  if (auth->mech != NULL)
    {
      _dbus_verbose ("%s: Shutting down mechanism %s\n",
                     DBUS_AUTH_NAME (auth), auth->mech->mechanism);
      
      if (DBUS_AUTH_IS_CLIENT (auth))
        (* auth->mech->client_shutdown_func) (auth);
      else
        (* auth->mech->server_shutdown_func) (auth);
      
      auth->mech = NULL;
    }
}

/*
 * DBUS_COOKIE_SHA1 mechanism
 */

/* Returns TRUE but with an empty string hash if the
 * cookie_id isn't known. As with all this code
 * TRUE just means we had enough memory.
 */
static dbus_bool_t
sha1_compute_hash (DBusAuth         *auth,
                   int               cookie_id,
                   const DBusString *server_challenge,
                   const DBusString *client_challenge,
                   DBusString       *hash)
{
  DBusString cookie;
  DBusString to_hash;
  dbus_bool_t retval;
  
  _dbus_assert (auth->keyring != NULL);

  retval = FALSE;
  
  if (!_dbus_string_init (&cookie))
    return FALSE;

  if (!_dbus_keyring_get_hex_key (auth->keyring, cookie_id,
                                  &cookie))
    goto out_0;

  if (_dbus_string_get_length (&cookie) == 0)
    {
      retval = TRUE;
      goto out_0;
    }

  if (!_dbus_string_init (&to_hash))
    goto out_0;
  
  if (!_dbus_string_copy (server_challenge, 0,
                          &to_hash, _dbus_string_get_length (&to_hash)))
    goto out_1;

  if (!_dbus_string_append (&to_hash, ":"))
    goto out_1;
  
  if (!_dbus_string_copy (client_challenge, 0,
                          &to_hash, _dbus_string_get_length (&to_hash)))
    goto out_1;

  if (!_dbus_string_append (&to_hash, ":"))
    goto out_1;

  if (!_dbus_string_copy (&cookie, 0,
                          &to_hash, _dbus_string_get_length (&to_hash)))
    goto out_1;

  if (!_dbus_sha_compute (&to_hash, hash))
    goto out_1;
  
  retval = TRUE;

 out_1:
  _dbus_string_zero (&to_hash);
  _dbus_string_free (&to_hash);
 out_0:
  _dbus_string_zero (&cookie);
  _dbus_string_free (&cookie);
  return retval;
}

/** http://www.ietf.org/rfc/rfc2831.txt suggests at least 64 bits of
 * entropy, we use 128. This is the number of bytes in the random
 * challenge.
 */
#define N_CHALLENGE_BYTES (128/8)

static dbus_bool_t
sha1_handle_first_client_response (DBusAuth         *auth,
                                   const DBusString *data)
{
  /* We haven't sent a challenge yet, we're expecting a desired
   * username from the client.
   */
  DBusString tmp = _DBUS_STRING_INIT_INVALID;
  DBusString tmp2 = _DBUS_STRING_INIT_INVALID;
  dbus_bool_t retval = FALSE;
  DBusError error = DBUS_ERROR_INIT;
  DBusCredentials *myself = NULL;

  _dbus_string_set_length (&auth->challenge, 0);
  
  if (_dbus_string_get_length (data) > 0)
    {
      if (_dbus_string_get_length (&auth->identity) > 0)
        {
          /* Tried to send two auth identities, wtf */
          _dbus_verbose ("%s: client tried to send auth identity, but we already have one\n",
                         DBUS_AUTH_NAME (auth));
          return send_rejected (auth);
        }
      else
        {
          /* this is our auth identity */
          if (!_dbus_string_copy (data, 0, &auth->identity, 0))
            return FALSE;
        }
    }
      
  if (!_dbus_credentials_add_from_user (auth->desired_identity, data,
                                        DBUS_CREDENTIALS_ADD_FLAGS_USER_DATABASE,
                                        &error))
    {
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_error_free (&error);
          return FALSE;
        }

      _dbus_verbose ("%s: Did not get a valid username from client: %s\n",
                     DBUS_AUTH_NAME (auth), error.message);
      dbus_error_free (&error);
      return send_rejected (auth);
    }
      
  if (!_dbus_string_init (&tmp))
    return FALSE;

  if (!_dbus_string_init (&tmp2))
    {
      _dbus_string_free (&tmp);
      return FALSE;
    }

  myself = _dbus_credentials_new_from_current_process ();

  if (myself == NULL)
    goto out;

  if (!_dbus_credentials_same_user (myself, auth->desired_identity))
    {
      /*
       * DBUS_COOKIE_SHA1 is not suitable for authenticating that the
       * client is anyone other than the user owning the process
       * containing the DBusServer: we probably aren't allowed to write
       * to other users' home directories. Even if we can (for example
       * uid 0 on traditional Unix or CAP_DAC_OVERRIDE on Linux), we
       * must not, because the other user controls their home directory,
       * and could carry out symlink attacks to make us read from or
       * write to unintended locations. It's difficult to avoid symlink
       * attacks in a portable way, so we just don't try. This isn't a
       * regression, because DBUS_COOKIE_SHA1 never worked for other
       * users anyway.
       */
      _dbus_verbose ("%s: client tried to authenticate as \"%s\", "
                     "but that doesn't match this process",
                     DBUS_AUTH_NAME (auth),
                     _dbus_string_get_const_data (data));
      retval = send_rejected (auth);
      goto out;
    }

  /* we cache the keyring for speed, so here we drop it if it's the
   * wrong one. FIXME caching the keyring here is useless since we use
   * a different DBusAuth for every connection.
   */
  if (auth->keyring &&
      !_dbus_keyring_is_for_credentials (auth->keyring,
                                         auth->desired_identity))
    {
      _dbus_keyring_unref (auth->keyring);
      auth->keyring = NULL;
    }
  
  if (auth->keyring == NULL)
    {
      auth->keyring = _dbus_keyring_new_for_credentials (auth->desired_identity,
                                                         &auth->context,
                                                         &error);

      if (auth->keyring == NULL)
        {
          if (dbus_error_has_name (&error,
                                   DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              goto out;
            }
          else
            {
              _DBUS_ASSERT_ERROR_IS_SET (&error);
              _dbus_verbose ("%s: Error loading keyring: %s\n",
                             DBUS_AUTH_NAME (auth), error.message);
              if (send_rejected (auth))
                retval = TRUE; /* retval is only about mem */
              dbus_error_free (&error);
              goto out;
            }
        }
      else
        {
          _dbus_assert (!dbus_error_is_set (&error));
        }
    }

  _dbus_assert (auth->keyring != NULL);

  auth->cookie_id = _dbus_keyring_get_best_key (auth->keyring, &error);
  if (auth->cookie_id < 0)
    {
      _DBUS_ASSERT_ERROR_IS_SET (&error);
      _dbus_verbose ("%s: Could not get a cookie ID to send to client: %s\n",
                     DBUS_AUTH_NAME (auth), error.message);
      if (send_rejected (auth))
        retval = TRUE;
      dbus_error_free (&error);
      goto out;
    }
  else
    {
      _dbus_assert (!dbus_error_is_set (&error));
    }

  if (!_dbus_string_copy (&auth->context, 0,
                          &tmp2, _dbus_string_get_length (&tmp2)))
    goto out;

  if (!_dbus_string_append (&tmp2, " "))
    goto out;

  if (!_dbus_string_append_int (&tmp2, auth->cookie_id))
    goto out;

  if (!_dbus_string_append (&tmp2, " "))
    goto out;  
  
  if (!_dbus_generate_random_bytes (&tmp, N_CHALLENGE_BYTES, &error))
    {
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_error_free (&error);
          goto out;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_SET (&error);
          _dbus_verbose ("%s: Error generating challenge: %s\n",
                         DBUS_AUTH_NAME (auth), error.message);
          if (send_rejected (auth))
            retval = TRUE; /* retval is only about mem */

          dbus_error_free (&error);
          goto out;
        }
    }

  _dbus_string_set_length (&auth->challenge, 0);
  if (!_dbus_string_hex_encode (&tmp, 0, &auth->challenge, 0))
    goto out;
  
  if (!_dbus_string_hex_encode (&tmp, 0, &tmp2,
                                _dbus_string_get_length (&tmp2)))
    goto out;

  if (!send_data (auth, &tmp2))
    goto out;
      
  goto_state (auth, &server_state_waiting_for_data);
  retval = TRUE;
  
 out:
  _dbus_string_zero (&tmp);
  _dbus_string_free (&tmp);
  _dbus_string_zero (&tmp2);
  _dbus_string_free (&tmp2);
  _dbus_clear_credentials (&myself);

  return retval;
}

static dbus_bool_t
sha1_handle_second_client_response (DBusAuth         *auth,
                                    const DBusString *data)
{
  /* We are expecting a response which is the hex-encoded client
   * challenge, space, then SHA-1 hash of the concatenation of our
   * challenge, ":", client challenge, ":", secret key, all
   * hex-encoded.
   */
  int i;
  DBusString client_challenge;
  DBusString client_hash;
  dbus_bool_t retval;
  DBusString correct_hash;
  
  retval = FALSE;
  
  if (!_dbus_string_find_blank (data, 0, &i))
    {
      _dbus_verbose ("%s: no space separator in client response\n",
                     DBUS_AUTH_NAME (auth));
      return send_rejected (auth);
    }
  
  if (!_dbus_string_init (&client_challenge))
    goto out_0;

  if (!_dbus_string_init (&client_hash))
    goto out_1;  

  if (!_dbus_string_copy_len (data, 0, i, &client_challenge,
                              0))
    goto out_2;

  _dbus_string_skip_blank (data, i, &i);
  
  if (!_dbus_string_copy_len (data, i,
                              _dbus_string_get_length (data) - i,
                              &client_hash,
                              0))
    goto out_2;

  if (_dbus_string_get_length (&client_challenge) == 0 ||
      _dbus_string_get_length (&client_hash) == 0)
    {
      _dbus_verbose ("%s: zero-length client challenge or hash\n",
                     DBUS_AUTH_NAME (auth));
      if (send_rejected (auth))
        retval = TRUE;
      goto out_2;
    }

  if (!_dbus_string_init (&correct_hash))
    goto out_2;

  if (!sha1_compute_hash (auth, auth->cookie_id,
                          &auth->challenge, 
                          &client_challenge,
                          &correct_hash))
    goto out_3;

  /* if cookie_id was invalid, then we get an empty hash */
  if (_dbus_string_get_length (&correct_hash) == 0)
    {
      if (send_rejected (auth))
        retval = TRUE;
      goto out_3;
    }
  
  if (!_dbus_string_equal (&client_hash, &correct_hash))
    {
      if (send_rejected (auth))
        retval = TRUE;
      goto out_3;
    }

  if (!_dbus_credentials_add_credentials (auth->authorized_identity,
                                          auth->desired_identity))
    goto out_3;

  /* Copy process ID from the socket credentials if it's there
   */
  if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                         DBUS_CREDENTIAL_UNIX_PROCESS_ID,
                                         auth->credentials))
    goto out_3;
  
  if (!send_ok (auth))
    goto out_3;

  _dbus_verbose ("%s: authenticated client using DBUS_COOKIE_SHA1\n",
                 DBUS_AUTH_NAME (auth));
  
  retval = TRUE;
  
 out_3:
  _dbus_string_zero (&correct_hash);
  _dbus_string_free (&correct_hash);
 out_2:
  _dbus_string_zero (&client_hash);
  _dbus_string_free (&client_hash);
 out_1:
  _dbus_string_free (&client_challenge);
 out_0:
  return retval;
}

static dbus_bool_t
handle_server_data_cookie_sha1_mech (DBusAuth         *auth,
                                     const DBusString *data)
{
  if (auth->cookie_id < 0)
    return sha1_handle_first_client_response (auth, data);
  else
    return sha1_handle_second_client_response (auth, data);
}

static void
handle_server_shutdown_cookie_sha1_mech (DBusAuth *auth)
{
  auth->cookie_id = -1;  
  _dbus_string_set_length (&auth->challenge, 0);
}

static dbus_bool_t
handle_client_initial_response_cookie_sha1_mech (DBusAuth   *auth,
                                                 DBusString *response)
{
  DBusString username;
  dbus_bool_t retval;

  retval = FALSE;

  if (!_dbus_string_init (&username))
    return FALSE;
  
  if (!_dbus_append_user_from_current_process (&username))
    goto out_0;

  if (!_dbus_string_hex_encode (&username, 0,
				response,
				_dbus_string_get_length (response)))
    goto out_0;

  retval = TRUE;
  
 out_0:
  _dbus_string_free (&username);
  
  return retval;
}

static dbus_bool_t
handle_client_data_cookie_sha1_mech (DBusAuth         *auth,
                                     const DBusString *data)
{
  /* The data we get from the server should be the cookie context
   * name, the cookie ID, and the server challenge, separated by
   * spaces. We send back our challenge string and the correct hash.
   */
  dbus_bool_t retval = FALSE;
  DBusString context;
  DBusString cookie_id_str;
  DBusString server_challenge;
  DBusString client_challenge;
  DBusString correct_hash;
  DBusString tmp;
  int i, j;
  long val;
  DBusError error = DBUS_ERROR_INIT;

  if (!_dbus_string_find_blank (data, 0, &i))
    {
      if (send_error (auth,
                      "Server did not send context/ID/challenge properly"))
        retval = TRUE;
      goto out_0;
    }

  if (!_dbus_string_init (&context))
    goto out_0;

  if (!_dbus_string_copy_len (data, 0, i,
                              &context, 0))
    goto out_1;
  
  _dbus_string_skip_blank (data, i, &i);
  if (!_dbus_string_find_blank (data, i, &j))
    {
      if (send_error (auth,
                      "Server did not send context/ID/challenge properly"))
        retval = TRUE;
      goto out_1;
    }

  if (!_dbus_string_init (&cookie_id_str))
    goto out_1;
  
  if (!_dbus_string_copy_len (data, i, j - i,
                              &cookie_id_str, 0))
    goto out_2;  

  if (!_dbus_string_init (&server_challenge))
    goto out_2;

  i = j;
  _dbus_string_skip_blank (data, i, &i);
  j = _dbus_string_get_length (data);

  if (!_dbus_string_copy_len (data, i, j - i,
                              &server_challenge, 0))
    goto out_3;

  if (!_dbus_keyring_validate_context (&context))
    {
      if (send_error (auth, "Server sent invalid cookie context"))
        retval = TRUE;
      goto out_3;
    }

  if (!_dbus_string_parse_int (&cookie_id_str, 0, &val, NULL))
    {
      if (send_error (auth, "Could not parse cookie ID as an integer"))
        retval = TRUE;
      goto out_3;
    }

  if (_dbus_string_get_length (&server_challenge) == 0)
    {
      if (send_error (auth, "Empty server challenge string"))
        retval = TRUE;
      goto out_3;
    }

  if (auth->keyring == NULL)
    {
      auth->keyring = _dbus_keyring_new_for_credentials (NULL,
                                                         &context,
                                                         &error);

      if (auth->keyring == NULL)
        {
          if (dbus_error_has_name (&error,
                                   DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              goto out_3;
            }
          else
            {
              _DBUS_ASSERT_ERROR_IS_SET (&error);

              _dbus_verbose ("%s: Error loading keyring: %s\n",
                             DBUS_AUTH_NAME (auth), error.message);
              
              if (send_error (auth, "Could not load cookie file"))
                retval = TRUE; /* retval is only about mem */
              
              dbus_error_free (&error);
              goto out_3;
            }
        }
      else
        {
          _dbus_assert (!dbus_error_is_set (&error));
        }
    }
  
  _dbus_assert (auth->keyring != NULL);
  
  if (!_dbus_string_init (&tmp))
    goto out_3;

  if (!_dbus_generate_random_bytes (&tmp, N_CHALLENGE_BYTES, &error))
    {
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_error_free (&error);
          goto out_4;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_SET (&error);

          _dbus_verbose ("%s: Failed to generate challenge: %s\n",
                         DBUS_AUTH_NAME (auth), error.message);

          if (send_error (auth, "Failed to generate challenge"))
            retval = TRUE; /* retval is only about mem */

          dbus_error_free (&error);
          goto out_4;
        }
    }

  if (!_dbus_string_init (&client_challenge))
    goto out_4;

  if (!_dbus_string_hex_encode (&tmp, 0, &client_challenge, 0))
    goto out_5;

  if (!_dbus_string_init (&correct_hash))
    goto out_5;
  
  if (!sha1_compute_hash (auth, val,
                          &server_challenge,
                          &client_challenge,
                          &correct_hash))
    goto out_6;

  if (_dbus_string_get_length (&correct_hash) == 0)
    {
      /* couldn't find the cookie ID or something */
      if (send_error (auth, "Don't have the requested cookie ID"))
        retval = TRUE;
      goto out_6;
    }
  
  _dbus_string_set_length (&tmp, 0);
  
  if (!_dbus_string_copy (&client_challenge, 0, &tmp,
                          _dbus_string_get_length (&tmp)))
    goto out_6;

  if (!_dbus_string_append (&tmp, " "))
    goto out_6;

  if (!_dbus_string_copy (&correct_hash, 0, &tmp,
                          _dbus_string_get_length (&tmp)))
    goto out_6;

  if (!send_data (auth, &tmp))
    goto out_6;

  retval = TRUE;

 out_6:
  _dbus_string_zero (&correct_hash);
  _dbus_string_free (&correct_hash);
 out_5:
  _dbus_string_free (&client_challenge);
 out_4:
  _dbus_string_zero (&tmp);
  _dbus_string_free (&tmp);
 out_3:
  _dbus_string_free (&server_challenge);
 out_2:
  _dbus_string_free (&cookie_id_str);
 out_1:
  _dbus_string_free (&context);
 out_0:
  return retval;
}

static void
handle_client_shutdown_cookie_sha1_mech (DBusAuth *auth)
{
  auth->cookie_id = -1;  
  _dbus_string_set_length (&auth->challenge, 0);
}

/*
 * EXTERNAL mechanism
 */

static dbus_bool_t
handle_server_data_external_mech (DBusAuth         *auth,
                                  const DBusString *data)
{
  if (_dbus_credentials_are_anonymous (auth->credentials))
    {
      _dbus_verbose ("%s: no credentials, mechanism EXTERNAL can't authenticate\n",
                     DBUS_AUTH_NAME (auth));
      return send_rejected (auth);
    }
  
  if (_dbus_string_get_length (data) > 0)
    {
      if (_dbus_string_get_length (&auth->identity) > 0)
        {
          /* Tried to send two auth identities, wtf */
          _dbus_verbose ("%s: client tried to send auth identity, but we already have one\n",
                         DBUS_AUTH_NAME (auth));
          return send_rejected (auth);
        }
      else
        {
          /* this is our auth identity */
          if (!_dbus_string_copy (data, 0, &auth->identity, 0))
            return FALSE;
        }
    }

  /* Poke client for an auth identity, if none given */
  if (_dbus_string_get_length (&auth->identity) == 0 &&
      !auth->already_asked_for_initial_response)
    {
      if (send_data (auth, NULL))
        {
          _dbus_verbose ("%s: sending empty challenge asking client for auth identity\n",
                         DBUS_AUTH_NAME (auth));
          auth->already_asked_for_initial_response = TRUE;
          goto_state (auth, &server_state_waiting_for_data);
          return TRUE;
        }
      else
        return FALSE;
    }

  _dbus_credentials_clear (auth->desired_identity);
  
  /* If auth->identity is still empty here, then client
   * responded with an empty string after we poked it for
   * an initial response. This means to try to auth the
   * identity provided in the credentials.
   */
  if (_dbus_string_get_length (&auth->identity) == 0)
    {
      if (!_dbus_credentials_add_credentials (auth->desired_identity,
                                              auth->credentials))
        {
          return FALSE; /* OOM */
        }
    }
  else
    {
      DBusError error = DBUS_ERROR_INIT;

      if (!_dbus_credentials_add_from_user (auth->desired_identity,
                                            &auth->identity,
                                            DBUS_CREDENTIALS_ADD_FLAGS_NONE,
                                            &error))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              return FALSE;
            }

          _dbus_verbose ("%s: could not get credentials from uid string: %s\n",
                         DBUS_AUTH_NAME (auth), error.message);
          dbus_error_free (&error);
          return send_rejected (auth);
        }
    }

  if (_dbus_credentials_are_anonymous (auth->desired_identity))
    {
      _dbus_verbose ("%s: desired user %s is no good\n",
                     DBUS_AUTH_NAME (auth),
                     _dbus_string_get_const_data (&auth->identity));
      return send_rejected (auth);
    }
  
  if (_dbus_credentials_are_superset (auth->credentials,
                                      auth->desired_identity))
    {
      /* client has authenticated */
      if (!_dbus_credentials_add_credentials (auth->authorized_identity,
                                              auth->desired_identity))
        return FALSE;

      /* also copy misc process info from the socket credentials
       */
      if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                             DBUS_CREDENTIAL_UNIX_PROCESS_ID,
                                             auth->credentials))
        return FALSE;

      if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                             DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID,
                                             auth->credentials))
        return FALSE;

      if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                             DBUS_CREDENTIAL_UNIX_GROUP_IDS,
                                             auth->credentials))
        return FALSE;

      if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                             DBUS_CREDENTIAL_LINUX_SECURITY_LABEL,
                                             auth->credentials))
        return FALSE;

      if (!send_ok (auth))
        return FALSE;

      _dbus_verbose ("%s: authenticated client based on socket credentials\n",
                     DBUS_AUTH_NAME (auth));

      return TRUE;
    }
  else
    {
      _dbus_verbose ("%s: desired identity not found in socket credentials\n",
                     DBUS_AUTH_NAME (auth));
      return send_rejected (auth);
    }
}

static void
handle_server_shutdown_external_mech (DBusAuth *auth)
{

}

static dbus_bool_t
handle_client_initial_response_external_mech (DBusAuth         *auth,
                                              DBusString       *response)
{
  /* We always append our UID as an initial response, so the server
   * doesn't have to send back an empty challenge to check whether we
   * want to specify an identity. i.e. this avoids a round trip that
   * the spec for the EXTERNAL mechanism otherwise requires.
   */
  DBusString plaintext;

  if (!_dbus_string_init (&plaintext))
    return FALSE;

  if (!_dbus_append_user_from_current_process (&plaintext))
    goto failed;

  if (!_dbus_string_hex_encode (&plaintext, 0,
				response,
				_dbus_string_get_length (response)))
    goto failed;

  _dbus_string_free (&plaintext);
  
  return TRUE;

 failed:
  _dbus_string_free (&plaintext);
  return FALSE;  
}

static dbus_bool_t
handle_client_data_external_mech (DBusAuth         *auth,
                                  const DBusString *data)
{
  
  return TRUE;
}

static void
handle_client_shutdown_external_mech (DBusAuth *auth)
{

}

/*
 * ANONYMOUS mechanism
 */

static dbus_bool_t
handle_server_data_anonymous_mech (DBusAuth         *auth,
                                   const DBusString *data)
{  
  if (_dbus_string_get_length (data) > 0)
    {
      /* Client is allowed to send "trace" data, the only defined
       * meaning is that if it contains '@' it is an email address,
       * and otherwise it is anything else, and it's supposed to be
       * UTF-8
       */
      if (!_dbus_string_validate_utf8 (data, 0, _dbus_string_get_length (data)))
        {
          _dbus_verbose ("%s: Received invalid UTF-8 trace data from ANONYMOUS client\n",
                         DBUS_AUTH_NAME (auth));
          return send_rejected (auth);
        }
      
      _dbus_verbose ("%s: ANONYMOUS client sent trace string: '%s'\n",
                     DBUS_AUTH_NAME (auth),
                     _dbus_string_get_const_data (data));
    }

  /* We want to be anonymous (clear in case some other protocol got midway through I guess) */
  _dbus_credentials_clear (auth->desired_identity);

  /* Copy process ID from the socket credentials
   */
  if (!_dbus_credentials_add_credential (auth->authorized_identity,
                                         DBUS_CREDENTIAL_UNIX_PROCESS_ID,
                                         auth->credentials))
    return FALSE;
  
  /* Anonymous is always allowed */
  if (!send_ok (auth))
    return FALSE;

  _dbus_verbose ("%s: authenticated client as anonymous\n",
                 DBUS_AUTH_NAME (auth));

  return TRUE;
}

static void
handle_server_shutdown_anonymous_mech (DBusAuth *auth)
{
  
}

static dbus_bool_t
handle_client_initial_response_anonymous_mech (DBusAuth         *auth,
                                               DBusString       *response)
{
  /* Our initial response is a "trace" string which must be valid UTF-8
   * and must be an email address if it contains '@'.
   * We just send the dbus implementation info, like a user-agent or
   * something, because... why not. There's nothing guaranteed here
   * though, we could change it later.
   */
  DBusString plaintext;

  if (!_dbus_string_init (&plaintext))
    return FALSE;

  if (!_dbus_string_append (&plaintext,
                            "libdbus " DBUS_VERSION_STRING))
    goto failed;

  if (!_dbus_string_hex_encode (&plaintext, 0,
				response,
				_dbus_string_get_length (response)))
    goto failed;

  _dbus_string_free (&plaintext);
  
  return TRUE;

 failed:
  _dbus_string_free (&plaintext);
  return FALSE;  
}

static dbus_bool_t
handle_client_data_anonymous_mech (DBusAuth         *auth,
                                  const DBusString *data)
{
  
  return TRUE;
}

static void
handle_client_shutdown_anonymous_mech (DBusAuth *auth)
{
  
}

/* Put mechanisms here in order of preference.
 * Right now we have:
 *
 * - EXTERNAL checks socket credentials (or in the future, other info from the OS)
 * - DBUS_COOKIE_SHA1 uses a cookie in the home directory, like xauth or ICE
 * - ANONYMOUS checks nothing but doesn't auth the person as a user
 *
 * We might ideally add a mechanism to chain to Cyrus SASL so we can
 * use its mechanisms as well.
 * 
 */
static const DBusAuthMechanismHandler
all_mechanisms[] = {
  { "EXTERNAL",
    handle_server_data_external_mech,
    NULL, NULL,
    handle_server_shutdown_external_mech,
    handle_client_initial_response_external_mech,
    handle_client_data_external_mech,
    NULL, NULL,
    handle_client_shutdown_external_mech },
  { "DBUS_COOKIE_SHA1",
    handle_server_data_cookie_sha1_mech,
    NULL, NULL,
    handle_server_shutdown_cookie_sha1_mech,
    handle_client_initial_response_cookie_sha1_mech,
    handle_client_data_cookie_sha1_mech,
    NULL, NULL,
    handle_client_shutdown_cookie_sha1_mech },
  { "ANONYMOUS",
    handle_server_data_anonymous_mech,
    NULL, NULL,
    handle_server_shutdown_anonymous_mech,
    handle_client_initial_response_anonymous_mech,
    handle_client_data_anonymous_mech,
    NULL, NULL,
    handle_client_shutdown_anonymous_mech },  
  { NULL, NULL }
};

static const DBusAuthMechanismHandler*
find_mech (const DBusString  *name,
           char             **allowed_mechs)
{
  int i;
  
  if (allowed_mechs != NULL &&
      !_dbus_string_array_contains ((const char**) allowed_mechs,
                                    _dbus_string_get_const_data (name)))
    return NULL;
  
  i = 0;
  while (all_mechanisms[i].mechanism != NULL)
    {      
      if (_dbus_string_equal_c_str (name,
                                    all_mechanisms[i].mechanism))

        return &all_mechanisms[i];
      
      ++i;
    }
  
  return NULL;
}

static dbus_bool_t
send_auth (DBusAuth *auth, const DBusAuthMechanismHandler *mech)
{
  DBusString auth_command;

  if (!_dbus_string_init (&auth_command))
    return FALSE;
      
  if (!_dbus_string_append (&auth_command,
                            "AUTH "))
    {
      _dbus_string_free (&auth_command);
      return FALSE;
    }  
  
  if (!_dbus_string_append (&auth_command,
                            mech->mechanism))
    {
      _dbus_string_free (&auth_command);
      return FALSE;
    }

  if (mech->client_initial_response_func != NULL)
    {
      if (!_dbus_string_append (&auth_command, " "))
        {
          _dbus_string_free (&auth_command);
          return FALSE;
        }
      
      if (!(* mech->client_initial_response_func) (auth, &auth_command))
        {
          _dbus_string_free (&auth_command);
          return FALSE;
        }
    }
  
  if (!_dbus_string_append (&auth_command,
                            "\r\n"))
    {
      _dbus_string_free (&auth_command);
      return FALSE;
    }

  if (!_dbus_string_copy (&auth_command, 0,
                          &auth->outgoing,
                          _dbus_string_get_length (&auth->outgoing)))
    {
      _dbus_string_free (&auth_command);
      return FALSE;
    }

  _dbus_string_free (&auth_command);
  shutdown_mech (auth);
  auth->mech = mech;      
  goto_state (auth, &client_state_waiting_for_data);

  return TRUE;
}

static dbus_bool_t
send_data (DBusAuth *auth, DBusString *data)
{
  int old_len;

  if (data == NULL || _dbus_string_get_length (data) == 0)
    return _dbus_string_append (&auth->outgoing, "DATA\r\n");
  else
    {
      old_len = _dbus_string_get_length (&auth->outgoing);
      if (!_dbus_string_append (&auth->outgoing, "DATA "))
        goto out;

      if (!_dbus_string_hex_encode (data, 0, &auth->outgoing,
                                    _dbus_string_get_length (&auth->outgoing)))
        goto out;

      if (!_dbus_string_append (&auth->outgoing, "\r\n"))
        goto out;

      return TRUE;

    out:
      _dbus_string_set_length (&auth->outgoing, old_len);

      return FALSE;
    }
}

static dbus_bool_t
send_rejected (DBusAuth *auth)
{
  DBusString command;
  DBusAuthServer *server_auth;
  int i;
  
  if (!_dbus_string_init (&command))
    return FALSE;
  
  if (!_dbus_string_append (&command,
                            "REJECTED"))
    goto nomem;

  for (i = 0; all_mechanisms[i].mechanism != NULL; i++)
    {
      /* skip mechanisms that aren't allowed */
      if (auth->allowed_mechs != NULL &&
          !_dbus_string_array_contains ((const char**)auth->allowed_mechs,
                                        all_mechanisms[i].mechanism))
        continue;

      if (!_dbus_string_append (&command,
                                " "))
        goto nomem;

      if (!_dbus_string_append (&command,
                                all_mechanisms[i].mechanism))
        goto nomem;
    }
  
  if (!_dbus_string_append (&command, "\r\n"))
    goto nomem;

  if (!_dbus_string_copy (&command, 0, &auth->outgoing,
                          _dbus_string_get_length (&auth->outgoing)))
    goto nomem;

  shutdown_mech (auth);
  
  _dbus_assert (DBUS_AUTH_IS_SERVER (auth));
  server_auth = DBUS_AUTH_SERVER (auth);
  server_auth->failures += 1;

  if (server_auth->failures >= server_auth->max_failures)
    goto_state (auth, &common_state_need_disconnect);
  else
    goto_state (auth, &server_state_waiting_for_auth);

  _dbus_string_free (&command);
  
  return TRUE;

 nomem:
  _dbus_string_free (&command);
  return FALSE;
}

static dbus_bool_t
send_error (DBusAuth *auth, const char *message)
{
  return _dbus_string_append_printf (&auth->outgoing,
                                     "ERROR \"%s\"\r\n", message);
}

static dbus_bool_t
send_ok (DBusAuth *auth)
{
  int orig_len;

  orig_len = _dbus_string_get_length (&auth->outgoing);
  
  if (_dbus_string_append (&auth->outgoing, "OK ") &&
      _dbus_string_copy (& DBUS_AUTH_SERVER (auth)->guid,
                         0,
                         &auth->outgoing,
                         _dbus_string_get_length (&auth->outgoing)) &&
      _dbus_string_append (&auth->outgoing, "\r\n"))
    {
      goto_state (auth, &server_state_waiting_for_begin);
      return TRUE;
    }
  else
    {
      _dbus_string_set_length (&auth->outgoing, orig_len);
      return FALSE;
    }
}

static dbus_bool_t
send_begin (DBusAuth         *auth)
{

  if (!_dbus_string_append (&auth->outgoing,
                            "BEGIN\r\n"))
    return FALSE;

  goto_state (auth, &common_state_authenticated);
  return TRUE;
}

static dbus_bool_t
process_ok(DBusAuth *auth,
          const DBusString *args_from_ok) {

  int end_of_hex;
  
  /* "args_from_ok" should be the GUID, whitespace already pulled off the front */
  _dbus_assert (_dbus_string_get_length (& DBUS_AUTH_CLIENT (auth)->guid_from_server) == 0);

  /* We decode the hex string to binary, using guid_from_server as scratch... */
  
  end_of_hex = 0;
  if (!_dbus_string_hex_decode (args_from_ok, 0, &end_of_hex,
                                & DBUS_AUTH_CLIENT (auth)->guid_from_server, 0))
    return FALSE;

  /* now clear out the scratch */
  _dbus_string_set_length (& DBUS_AUTH_CLIENT (auth)->guid_from_server, 0);
  
  if (end_of_hex != _dbus_string_get_length (args_from_ok) ||
      end_of_hex == 0)
    {
      _dbus_verbose ("Bad GUID from server, parsed %d bytes and had %d bytes from server\n",
                     end_of_hex, _dbus_string_get_length (args_from_ok));
      goto_state (auth, &common_state_need_disconnect);
      return TRUE;
    }

  if (!_dbus_string_copy (args_from_ok, 0, &DBUS_AUTH_CLIENT (auth)->guid_from_server, 0)) {
      _dbus_string_set_length (& DBUS_AUTH_CLIENT (auth)->guid_from_server, 0);
      return FALSE;
  }

  _dbus_verbose ("Got GUID '%s' from the server\n",
                 _dbus_string_get_const_data (& DBUS_AUTH_CLIENT (auth)->guid_from_server));

  if (auth->unix_fd_possible)
    {
      if (!send_negotiate_unix_fd (auth))
        {
          _dbus_string_set_length (& DBUS_AUTH_CLIENT (auth)->guid_from_server, 0);
          return FALSE;
        }

      return TRUE;
    }

  _dbus_verbose("Not negotiating unix fd passing, since not possible\n");

  if (!send_begin (auth))
    {
      _dbus_string_set_length (& DBUS_AUTH_CLIENT (auth)->guid_from_server, 0);
      return FALSE;
    }

  return TRUE;
}

static dbus_bool_t
send_cancel (DBusAuth *auth)
{
  if (_dbus_string_append (&auth->outgoing, "CANCEL\r\n"))
    {
      goto_state (auth, &client_state_waiting_for_reject);
      return TRUE;
    }
  else
    return FALSE;
}

static dbus_bool_t
process_data (DBusAuth             *auth,
              const DBusString     *args,
              DBusAuthDataFunction  data_func)
{
  int end;
  DBusString decoded;

  if (!_dbus_string_init (&decoded))
    return FALSE;

  if (!_dbus_string_hex_decode (args, 0, &end, &decoded, 0))
    {
      _dbus_string_free (&decoded);
      return FALSE;
    }

  if (_dbus_string_get_length (args) != end)
    {
      _dbus_string_free (&decoded);
      if (!send_error (auth, "Invalid hex encoding"))
        return FALSE;

      return TRUE;
    }

#ifdef DBUS_ENABLE_VERBOSE_MODE
  if (_dbus_string_validate_ascii (&decoded, 0,
                                   _dbus_string_get_length (&decoded)))
    _dbus_verbose ("%s: data: '%s'\n",
                   DBUS_AUTH_NAME (auth),
                   _dbus_string_get_const_data (&decoded));
#endif
      
  if (!(* data_func) (auth, &decoded))
    {
      _dbus_string_free (&decoded);
      return FALSE;
    }

  _dbus_string_free (&decoded);
  return TRUE;
}

static dbus_bool_t
send_negotiate_unix_fd (DBusAuth *auth)
{
  if (!_dbus_string_append (&auth->outgoing,
                            "NEGOTIATE_UNIX_FD\r\n"))
    return FALSE;

  goto_state (auth, &client_state_waiting_for_agree_unix_fd);
  return TRUE;
}

static dbus_bool_t
send_agree_unix_fd (DBusAuth *auth)
{
  _dbus_assert(auth->unix_fd_possible);

  auth->unix_fd_negotiated = TRUE;
  _dbus_verbose("Agreed to UNIX FD passing\n");

  if (!_dbus_string_append (&auth->outgoing,
                            "AGREE_UNIX_FD\r\n"))
    return FALSE;

  goto_state (auth, &server_state_waiting_for_begin);
  return TRUE;
}

static dbus_bool_t
handle_auth (DBusAuth *auth, const DBusString *args)
{
  if (_dbus_string_get_length (args) == 0)
    {
      /* No args to the auth, send mechanisms */
      if (!send_rejected (auth))
        return FALSE;

      return TRUE;
    }
  else
    {
      int i;
      DBusString mech;
      DBusString hex_response;
      
      _dbus_string_find_blank (args, 0, &i);

      if (!_dbus_string_init (&mech))
        return FALSE;

      if (!_dbus_string_init (&hex_response))
        {
          _dbus_string_free (&mech);
          return FALSE;
        }
      
      if (!_dbus_string_copy_len (args, 0, i, &mech, 0))
        goto failed;

      _dbus_string_skip_blank (args, i, &i);
      if (!_dbus_string_copy (args, i, &hex_response, 0))
        goto failed;
     
      auth->mech = find_mech (&mech, auth->allowed_mechs);
      if (auth->mech != NULL)
        {
          _dbus_verbose ("%s: Trying mechanism %s\n",
                         DBUS_AUTH_NAME (auth),
                         auth->mech->mechanism);
          
          if (!process_data (auth, &hex_response,
                             auth->mech->server_data_func))
            goto failed;
        }
      else
        {
          /* Unsupported mechanism */
          _dbus_verbose ("%s: Unsupported mechanism %s\n",
                         DBUS_AUTH_NAME (auth),
                         _dbus_string_get_const_data (&mech));
          
          if (!send_rejected (auth))
            goto failed;
        }

      _dbus_string_free (&mech);      
      _dbus_string_free (&hex_response);

      return TRUE;
      
    failed:
      auth->mech = NULL;
      _dbus_string_free (&mech);
      _dbus_string_free (&hex_response);
      return FALSE;
    }
}

static dbus_bool_t
handle_server_state_waiting_for_auth  (DBusAuth         *auth,
                                       DBusAuthCommand   command,
                                       const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_AUTH:
      return handle_auth (auth, args);

    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_DATA:
      return send_error (auth, "Not currently in an auth conversation");

    case DBUS_AUTH_COMMAND_BEGIN:
      goto_state (auth, &common_state_need_disconnect);
      return TRUE;

    case DBUS_AUTH_COMMAND_ERROR:
      return send_rejected (auth);

    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
      return send_error (auth, "Need to authenticate first");

    case DBUS_AUTH_COMMAND_REJECTED:
    case DBUS_AUTH_COMMAND_OK:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");
    }
}

static dbus_bool_t
handle_server_state_waiting_for_data  (DBusAuth         *auth,
                                       DBusAuthCommand   command,
                                       const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_AUTH:
      return send_error (auth, "Sent AUTH while another AUTH in progress");

    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_ERROR:
      return send_rejected (auth);

    case DBUS_AUTH_COMMAND_DATA:
      return process_data (auth, args, auth->mech->server_data_func);

    case DBUS_AUTH_COMMAND_BEGIN:
      goto_state (auth, &common_state_need_disconnect);
      return TRUE;

    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
      return send_error (auth, "Need to authenticate first");

    case DBUS_AUTH_COMMAND_REJECTED:
    case DBUS_AUTH_COMMAND_OK:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");
    }
}

static dbus_bool_t
handle_server_state_waiting_for_begin (DBusAuth         *auth,
                                       DBusAuthCommand   command,
                                       const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_AUTH:
      return send_error (auth, "Sent AUTH while expecting BEGIN");

    case DBUS_AUTH_COMMAND_DATA:
      return send_error (auth, "Sent DATA while expecting BEGIN");

    case DBUS_AUTH_COMMAND_BEGIN:
      goto_state (auth, &common_state_authenticated);
      return TRUE;

    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
      if (auth->unix_fd_possible)
        return send_agree_unix_fd(auth);
      else
        return send_error(auth, "Unix FD passing not supported, not authenticated or otherwise not possible");

    case DBUS_AUTH_COMMAND_REJECTED:
    case DBUS_AUTH_COMMAND_OK:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");

    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_ERROR:
      return send_rejected (auth);
    }
}

/* return FALSE if no memory, TRUE if all OK */
static dbus_bool_t
get_word (const DBusString *str,
          int              *start,
          DBusString       *word)
{
  int i;

  _dbus_string_skip_blank (str, *start, start);
  _dbus_string_find_blank (str, *start, &i);
  
  if (i > *start)
    {
      if (!_dbus_string_copy_len (str, *start, i - *start, word, 0))
        return FALSE;
      
      *start = i;
    }

  return TRUE;
}

static dbus_bool_t
record_mechanisms (DBusAuth         *auth,
                   const DBusString *args)
{
  int next;
  int len;

  if (auth->already_got_mechanisms)
    return TRUE;
  
  len = _dbus_string_get_length (args);
  
  next = 0;
  while (next < len)
    {
      DBusString m;
      const DBusAuthMechanismHandler *mech;
      
      if (!_dbus_string_init (&m))
        goto nomem;
      
      if (!get_word (args, &next, &m))
        {
          _dbus_string_free (&m);
          goto nomem;
        }

      mech = find_mech (&m, auth->allowed_mechs);

      if (mech != NULL)
        {
          /* FIXME right now we try mechanisms in the order
           * the server lists them; should we do them in
           * some more deterministic order?
           *
           * Probably in all_mechanisms order, our order of
           * preference. Of course when the server is us,
           * it lists things in that order anyhow.
           */

          if (mech != &all_mechanisms[0])
            {
              _dbus_verbose ("%s: Adding mechanism %s to list we will try\n",
                             DBUS_AUTH_NAME (auth), mech->mechanism);
          
              if (!_dbus_list_append (& DBUS_AUTH_CLIENT (auth)->mechs_to_try,
                                      (void*) mech))
                {
                  _dbus_string_free (&m);
                  goto nomem;
                }
            }
          else
            {
              _dbus_verbose ("%s: Already tried mechanism %s; not adding to list we will try\n",
                             DBUS_AUTH_NAME (auth), mech->mechanism);
            }
        }
      else
        {
          _dbus_verbose ("%s: Server offered mechanism \"%s\" that we don't know how to use\n",
                         DBUS_AUTH_NAME (auth),
                         _dbus_string_get_const_data (&m));
        }

      _dbus_string_free (&m);
    }
  
  auth->already_got_mechanisms = TRUE;
  
  return TRUE;

 nomem:
  _dbus_list_clear (& DBUS_AUTH_CLIENT (auth)->mechs_to_try);
  
  return FALSE;
}

static dbus_bool_t
process_rejected (DBusAuth *auth, const DBusString *args)
{
  const DBusAuthMechanismHandler *mech;
  DBusAuthClient *client;

  client = DBUS_AUTH_CLIENT (auth);

  if (!auth->already_got_mechanisms)
    {
      if (!record_mechanisms (auth, args))
        return FALSE;
    }
  
  if (DBUS_AUTH_CLIENT (auth)->mechs_to_try != NULL)
    {
      mech = client->mechs_to_try->data;

      if (!send_auth (auth, mech))
        return FALSE;

      _dbus_list_pop_first (&client->mechs_to_try);

      _dbus_verbose ("%s: Trying mechanism %s\n",
                     DBUS_AUTH_NAME (auth),
                     mech->mechanism);
    }
  else
    {
      /* Give up */
      _dbus_verbose ("%s: Disconnecting because we are out of mechanisms to try using\n",
                     DBUS_AUTH_NAME (auth));
      goto_state (auth, &common_state_need_disconnect);
    }
  
  return TRUE;
}


static dbus_bool_t
handle_client_state_waiting_for_data (DBusAuth         *auth,
                                      DBusAuthCommand   command,
                                      const DBusString *args)
{
  _dbus_assert (auth->mech != NULL);
 
  switch (command)
    {
    case DBUS_AUTH_COMMAND_DATA:
      return process_data (auth, args, auth->mech->client_data_func);

    case DBUS_AUTH_COMMAND_REJECTED:
      return process_rejected (auth, args);

    case DBUS_AUTH_COMMAND_OK:
      return process_ok(auth, args);

    case DBUS_AUTH_COMMAND_ERROR:
      return send_cancel (auth);

    case DBUS_AUTH_COMMAND_AUTH:
    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_BEGIN:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");
    }
}

static dbus_bool_t
handle_client_state_waiting_for_ok (DBusAuth         *auth,
                                    DBusAuthCommand   command,
                                    const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_REJECTED:
      return process_rejected (auth, args);

    case DBUS_AUTH_COMMAND_OK:
      return process_ok(auth, args);

    case DBUS_AUTH_COMMAND_DATA:
    case DBUS_AUTH_COMMAND_ERROR:
      return send_cancel (auth);

    case DBUS_AUTH_COMMAND_AUTH:
    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_BEGIN:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");
    }
}

static dbus_bool_t
handle_client_state_waiting_for_reject (DBusAuth         *auth,
                                        DBusAuthCommand   command,
                                        const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_REJECTED:
      return process_rejected (auth, args);
      
    case DBUS_AUTH_COMMAND_AUTH:
    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_DATA:
    case DBUS_AUTH_COMMAND_BEGIN:
    case DBUS_AUTH_COMMAND_OK:
    case DBUS_AUTH_COMMAND_ERROR:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
    default:
      goto_state (auth, &common_state_need_disconnect);
      return TRUE;
    }
}

static dbus_bool_t
handle_client_state_waiting_for_agree_unix_fd(DBusAuth         *auth,
                                              DBusAuthCommand   command,
                                              const DBusString *args)
{
  switch (command)
    {
    case DBUS_AUTH_COMMAND_AGREE_UNIX_FD:
      _dbus_assert(auth->unix_fd_possible);
      auth->unix_fd_negotiated = TRUE;
      _dbus_verbose("Successfully negotiated UNIX FD passing\n");
      return send_begin (auth);

    case DBUS_AUTH_COMMAND_ERROR:
      _dbus_assert(auth->unix_fd_possible);
      auth->unix_fd_negotiated = FALSE;
      _dbus_verbose("Failed to negotiate UNIX FD passing\n");
      return send_begin (auth);

    case DBUS_AUTH_COMMAND_OK:
    case DBUS_AUTH_COMMAND_DATA:
    case DBUS_AUTH_COMMAND_REJECTED:
    case DBUS_AUTH_COMMAND_AUTH:
    case DBUS_AUTH_COMMAND_CANCEL:
    case DBUS_AUTH_COMMAND_BEGIN:
    case DBUS_AUTH_COMMAND_UNKNOWN:
    case DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD:
    default:
      return send_error (auth, "Unknown command");
    }
}

/**
 * Mapping from command name to enum
 */
typedef struct {
  const char *name;        /**< Name of the command */
  DBusAuthCommand command; /**< Corresponding enum */
} DBusAuthCommandName;

static const DBusAuthCommandName auth_command_names[] = {
  { "AUTH",              DBUS_AUTH_COMMAND_AUTH },
  { "CANCEL",            DBUS_AUTH_COMMAND_CANCEL },
  { "DATA",              DBUS_AUTH_COMMAND_DATA },
  { "BEGIN",             DBUS_AUTH_COMMAND_BEGIN },
  { "REJECTED",          DBUS_AUTH_COMMAND_REJECTED },
  { "OK",                DBUS_AUTH_COMMAND_OK },
  { "ERROR",             DBUS_AUTH_COMMAND_ERROR },
  { "NEGOTIATE_UNIX_FD", DBUS_AUTH_COMMAND_NEGOTIATE_UNIX_FD },
  { "AGREE_UNIX_FD",     DBUS_AUTH_COMMAND_AGREE_UNIX_FD }
};

static DBusAuthCommand
lookup_command_from_name (DBusString *command)
{
  int i;

  for (i = 0; i < _DBUS_N_ELEMENTS (auth_command_names); i++)
    {
      if (_dbus_string_equal_c_str (command,
                                    auth_command_names[i].name))
        return auth_command_names[i].command;
    }

  return DBUS_AUTH_COMMAND_UNKNOWN;
}

static void
goto_state (DBusAuth *auth,
            const DBusAuthStateData *state)
{
  _dbus_verbose ("%s: going from state %s to state %s\n",
                 DBUS_AUTH_NAME (auth),
                 auth->state->name,
                 state->name);

  auth->state = state;
}

/* returns whether to call it again right away */
static dbus_bool_t
process_command (DBusAuth *auth)
{
  DBusAuthCommand command;
  DBusString line;
  DBusString args;
  int eol;
  int i, j;
  dbus_bool_t retval;

  /* _dbus_verbose ("%s:   trying process_command()\n"); */
  
  retval = FALSE;
  
  eol = 0;
  if (!_dbus_string_find (&auth->incoming, 0, "\r\n", &eol))
    return FALSE;
  
  if (!_dbus_string_init (&line))
    {
      auth->needed_memory = TRUE;
      return FALSE;
    }

  if (!_dbus_string_init (&args))
    {
      _dbus_string_free (&line);
      auth->needed_memory = TRUE;
      return FALSE;
    }
  
  if (!_dbus_string_copy_len (&auth->incoming, 0, eol, &line, 0))
    goto out;

  if (!_dbus_string_validate_ascii (&line, 0,
                                    _dbus_string_get_length (&line)))
    {
      _dbus_verbose ("%s: Command contained non-ASCII chars or embedded nul\n",
                     DBUS_AUTH_NAME (auth));
      if (!send_error (auth, "Command contained non-ASCII"))
        goto out;
      else
        goto next_command;
    }
  
  _dbus_verbose ("%s: got command \"%s\"\n",
                 DBUS_AUTH_NAME (auth),
                 _dbus_string_get_const_data (&line));
  
  _dbus_string_find_blank (&line, 0, &i);
  _dbus_string_skip_blank (&line, i, &j);

  if (j > i)
    _dbus_string_delete (&line, i, j - i);
  
  if (!_dbus_string_move (&line, i, &args, 0))
    goto out;

  /* FIXME 1.0 we should probably validate that only the allowed
   * chars are in the command name
   */
  
  command = lookup_command_from_name (&line);
  if (!(* auth->state->handler) (auth, command, &args))
    goto out;

 next_command:
  
  /* We've succeeded in processing the whole command so drop it out
   * of the incoming buffer and return TRUE to try another command.
   */

  _dbus_string_delete (&auth->incoming, 0, eol);
  
  /* kill the \r\n */
  _dbus_string_delete (&auth->incoming, 0, 2);

  retval = TRUE;
  
 out:
  _dbus_string_free (&args);
  _dbus_string_free (&line);

  if (!retval)
    auth->needed_memory = TRUE;
  else
    auth->needed_memory = FALSE;
  
  return retval;
}


/** @} */

/**
 * @addtogroup DBusAuth
 * @{
 */

/**
 * Creates a new auth conversation object for the server side.
 * See http://dbus.freedesktop.org/doc/dbus-specification.html#auth-protocol
 * for full details on what this object does.
 *
 * @returns the new object or #NULL if no memory
 */
DBusAuth*
_dbus_auth_server_new (const DBusString *guid)
{
  DBusAuth *auth;
  DBusAuthServer *server_auth;
  DBusString guid_copy;

  if (!_dbus_string_init (&guid_copy))
    return NULL;

  if (!_dbus_string_copy (guid, 0, &guid_copy, 0))
    {
      _dbus_string_free (&guid_copy);
      return NULL;
    }

  auth = _dbus_auth_new (sizeof (DBusAuthServer));
  if (auth == NULL)
    {
      _dbus_string_free (&guid_copy);
      return NULL;
    }
  
  auth->side = auth_side_server;
  auth->state = &server_state_waiting_for_auth;

  server_auth = DBUS_AUTH_SERVER (auth);

  server_auth->guid = guid_copy;
  
  /* perhaps this should be per-mechanism with a lower
   * max
   */
  server_auth->failures = 0;
  server_auth->max_failures = 6;
  
  return auth;
}

/**
 * Creates a new auth conversation object for the client side.
 * See http://dbus.freedesktop.org/doc/dbus-specification.html#auth-protocol
 * for full details on what this object does.
 *
 * @returns the new object or #NULL if no memory
 */
DBusAuth*
_dbus_auth_client_new (void)
{
  DBusAuth *auth;
  DBusString guid_str;

  if (!_dbus_string_init (&guid_str))
    return NULL;

  auth = _dbus_auth_new (sizeof (DBusAuthClient));
  if (auth == NULL)
    {
      _dbus_string_free (&guid_str);
      return NULL;
    }

  DBUS_AUTH_CLIENT (auth)->guid_from_server = guid_str;

  auth->side = auth_side_client;
  auth->state = &client_state_need_send_auth;

  /* Start the auth conversation by sending AUTH for our default
   * mechanism */
  if (!send_auth (auth, &all_mechanisms[0]))
    {
      _dbus_auth_unref (auth);
      return NULL;
    }
  
  return auth;
}

/**
 * Increments the refcount of an auth object.
 *
 * @param auth the auth conversation
 * @returns the auth conversation
 */
DBusAuth *
_dbus_auth_ref (DBusAuth *auth)
{
  _dbus_assert (auth != NULL);
  
  auth->refcount += 1;
  
  return auth;
}

/**
 * Decrements the refcount of an auth object.
 *
 * @param auth the auth conversation
 */
void
_dbus_auth_unref (DBusAuth *auth)
{
  _dbus_assert (auth != NULL);
  _dbus_assert (auth->refcount > 0);

  auth->refcount -= 1;
  if (auth->refcount == 0)
    {
      shutdown_mech (auth);

      if (DBUS_AUTH_IS_CLIENT (auth))
        {
          _dbus_string_free (& DBUS_AUTH_CLIENT (auth)->guid_from_server);
          _dbus_list_clear (& DBUS_AUTH_CLIENT (auth)->mechs_to_try);
        }
      else
        {
          _dbus_assert (DBUS_AUTH_IS_SERVER (auth));

          _dbus_string_free (& DBUS_AUTH_SERVER (auth)->guid);
        }

      if (auth->keyring)
        _dbus_keyring_unref (auth->keyring);

      _dbus_string_free (&auth->context);
      _dbus_string_free (&auth->challenge);
      _dbus_string_free (&auth->identity);
      _dbus_string_free (&auth->incoming);
      _dbus_string_free (&auth->outgoing);

      dbus_free_string_array (auth->allowed_mechs);

      _dbus_credentials_unref (auth->credentials);
      _dbus_credentials_unref (auth->authorized_identity);
      _dbus_credentials_unref (auth->desired_identity);
      
      dbus_free (auth);
    }
}

/**
 * Sets an array of authentication mechanism names
 * that we are willing to use.
 *
 * @param auth the auth conversation
 * @param mechanisms #NULL-terminated array of mechanism names
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_auth_set_mechanisms (DBusAuth    *auth,
                           const char **mechanisms)
{
  char **copy;

  if (mechanisms != NULL)
    {
      copy = _dbus_dup_string_array (mechanisms);
      if (copy == NULL)
        return FALSE;
    }
  else
    copy = NULL;
  
  dbus_free_string_array (auth->allowed_mechs);

  auth->allowed_mechs = copy;

  return TRUE;
}

/**
 * @param auth the auth conversation object
 * @returns #TRUE if we're in a final state
 */
#define DBUS_AUTH_IN_END_STATE(auth) ((auth)->state->handler == NULL)

/**
 * Analyzes buffered input and moves the auth conversation forward,
 * returning the new state of the auth conversation.
 *
 * @param auth the auth conversation
 * @returns the new state
 */
DBusAuthState
_dbus_auth_do_work (DBusAuth *auth)
{
  auth->needed_memory = FALSE;

  /* Max amount we'll buffer up before deciding someone's on crack */
#define MAX_BUFFER (16 * _DBUS_ONE_KILOBYTE)

  do
    {
      if (DBUS_AUTH_IN_END_STATE (auth))
        break;
      
      if (_dbus_string_get_length (&auth->incoming) > MAX_BUFFER ||
          _dbus_string_get_length (&auth->outgoing) > MAX_BUFFER)
        {
          goto_state (auth, &common_state_need_disconnect);
          _dbus_verbose ("%s: Disconnecting due to excessive data buffered in auth phase\n",
                         DBUS_AUTH_NAME (auth));
          break;
        }
    }
  while (process_command (auth));

  if (auth->needed_memory)
    return DBUS_AUTH_STATE_WAITING_FOR_MEMORY;
  else if (_dbus_string_get_length (&auth->outgoing) > 0)
    return DBUS_AUTH_STATE_HAVE_BYTES_TO_SEND;
  else if (auth->state == &common_state_need_disconnect)
    return DBUS_AUTH_STATE_NEED_DISCONNECT;
  else if (auth->state == &common_state_authenticated)
    return DBUS_AUTH_STATE_AUTHENTICATED;
  else return DBUS_AUTH_STATE_WAITING_FOR_INPUT;
}

/**
 * Gets bytes that need to be sent to the peer we're conversing with.
 * After writing some bytes, _dbus_auth_bytes_sent() must be called
 * to notify the auth object that they were written.
 *
 * @param auth the auth conversation
 * @param str return location for a ref to the buffer to send
 * @returns #FALSE if nothing to send
 */
dbus_bool_t
_dbus_auth_get_bytes_to_send (DBusAuth          *auth,
                              const DBusString **str)
{
  _dbus_assert (auth != NULL);
  _dbus_assert (str != NULL);

  *str = NULL;
  
  if (_dbus_string_get_length (&auth->outgoing) == 0)
    return FALSE;

  *str = &auth->outgoing;

  return TRUE;
}

/**
 * Notifies the auth conversation object that
 * the given number of bytes of the outgoing buffer
 * have been written out.
 *
 * @param auth the auth conversation
 * @param bytes_sent number of bytes written out
 */
void
_dbus_auth_bytes_sent (DBusAuth *auth,
                       int       bytes_sent)
{
  _dbus_verbose ("%s: Sent %d bytes of: %s\n",
                 DBUS_AUTH_NAME (auth),
                 bytes_sent,
                 _dbus_string_get_const_data (&auth->outgoing));
  
  _dbus_string_delete (&auth->outgoing,
                       0, bytes_sent);
}

/**
 * Get a buffer to be used for reading bytes from the peer we're conversing
 * with. Bytes should be appended to this buffer.
 *
 * @param auth the auth conversation
 * @param buffer return location for buffer to append bytes to
 */
void
_dbus_auth_get_buffer (DBusAuth     *auth,
                       DBusString **buffer)
{
  _dbus_assert (auth != NULL);
  _dbus_assert (!auth->buffer_outstanding);
  
  *buffer = &auth->incoming;

  auth->buffer_outstanding = TRUE;
}

/**
 * Returns a buffer with new data read into it.
 *
 * @param auth the auth conversation
 * @param buffer the buffer being returned
 */
void
_dbus_auth_return_buffer (DBusAuth               *auth,
                          DBusString             *buffer)
{
  _dbus_assert (buffer == &auth->incoming);
  _dbus_assert (auth->buffer_outstanding);

  auth->buffer_outstanding = FALSE;
}

/**
 * Returns leftover bytes that were not used as part of the auth
 * conversation.  These bytes will be part of the message stream
 * instead. This function may not be called until authentication has
 * succeeded.
 *
 * @param auth the auth conversation
 * @param str return location for pointer to string of unused bytes
 */
void
_dbus_auth_get_unused_bytes (DBusAuth           *auth,
                             const DBusString **str)
{
  if (!DBUS_AUTH_IN_END_STATE (auth))
    return;

  *str = &auth->incoming;
}


/**
 * Gets rid of unused bytes returned by _dbus_auth_get_unused_bytes()
 * after we've gotten them and successfully moved them elsewhere.
 *
 * @param auth the auth conversation
 */
void
_dbus_auth_delete_unused_bytes (DBusAuth *auth)
{
  if (!DBUS_AUTH_IN_END_STATE (auth))
    return;

  _dbus_string_set_length (&auth->incoming, 0);
}

/**
 * Called post-authentication, indicates whether we need to encode
 * the message stream with _dbus_auth_encode_data() prior to
 * sending it to the peer.
 *
 * @param auth the auth conversation
 * @returns #TRUE if we need to encode the stream
 */
dbus_bool_t
_dbus_auth_needs_encoding (DBusAuth *auth)
{
  if (auth->state != &common_state_authenticated)
    return FALSE;
  
  if (auth->mech != NULL)
    {
      if (DBUS_AUTH_IS_CLIENT (auth))
        return auth->mech->client_encode_func != NULL;
      else
        return auth->mech->server_encode_func != NULL;
    }
  else
    return FALSE;
}

/**
 * Called post-authentication, encodes a block of bytes for sending to
 * the peer. If no encoding was negotiated, just copies the bytes
 * (you can avoid this by checking _dbus_auth_needs_encoding()).
 *
 * @param auth the auth conversation
 * @param plaintext the plain text data
 * @param encoded initialized string to where encoded data is appended
 * @returns #TRUE if we had enough memory and successfully encoded
 */
dbus_bool_t
_dbus_auth_encode_data (DBusAuth         *auth,
                        const DBusString *plaintext,
                        DBusString       *encoded)
{
  _dbus_assert (plaintext != encoded);
  
  if (auth->state != &common_state_authenticated)
    return FALSE;
  
  if (_dbus_auth_needs_encoding (auth))
    {
      if (DBUS_AUTH_IS_CLIENT (auth))
        return (* auth->mech->client_encode_func) (auth, plaintext, encoded);
      else
        return (* auth->mech->server_encode_func) (auth, plaintext, encoded);
    }
  else
    {
      return _dbus_string_copy (plaintext, 0, encoded,
                                _dbus_string_get_length (encoded));
    }
}

/**
 * Called post-authentication, indicates whether we need to decode
 * the message stream with _dbus_auth_decode_data() after
 * receiving it from the peer.
 *
 * @param auth the auth conversation
 * @returns #TRUE if we need to encode the stream
 */
dbus_bool_t
_dbus_auth_needs_decoding (DBusAuth *auth)
{
  if (auth->state != &common_state_authenticated)
    return FALSE;
    
  if (auth->mech != NULL)
    {
      if (DBUS_AUTH_IS_CLIENT (auth))
        return auth->mech->client_decode_func != NULL;
      else
        return auth->mech->server_decode_func != NULL;
    }
  else
    return FALSE;
}


/**
 * Called post-authentication, decodes a block of bytes received from
 * the peer. If no encoding was negotiated, just copies the bytes (you
 * can avoid this by checking _dbus_auth_needs_decoding()).
 *
 * @todo 1.0? We need to be able to distinguish "out of memory" error
 * from "the data is hosed" error.
 *
 * @param auth the auth conversation
 * @param encoded the encoded data
 * @param plaintext initialized string where decoded data is appended
 * @returns #TRUE if we had enough memory and successfully decoded
 */
dbus_bool_t
_dbus_auth_decode_data (DBusAuth         *auth,
                        const DBusString *encoded,
                        DBusString       *plaintext)
{
  _dbus_assert (plaintext != encoded);
  
  if (auth->state != &common_state_authenticated)
    return FALSE;
  
  if (_dbus_auth_needs_decoding (auth))
    {
      if (DBUS_AUTH_IS_CLIENT (auth))
        return (* auth->mech->client_decode_func) (auth, encoded, plaintext);
      else
        return (* auth->mech->server_decode_func) (auth, encoded, plaintext);
    }
  else
    {
      return _dbus_string_copy (encoded, 0, plaintext,
                                _dbus_string_get_length (plaintext));
    }
}

/**
 * Sets credentials received via reliable means from the operating
 * system.
 *
 * @param auth the auth conversation
 * @param credentials the credentials received
 * @returns #FALSE on OOM
 */
dbus_bool_t
_dbus_auth_set_credentials (DBusAuth               *auth,
                            DBusCredentials        *credentials)
{
  _dbus_credentials_clear (auth->credentials);
  return _dbus_credentials_add_credentials (auth->credentials,
                                            credentials);
}

/**
 * Gets the identity we authorized the client as.  Apps may have
 * different policies as to what identities they allow.
 *
 * Returned credentials are not a copy and should not be modified
 *
 * @param auth the auth conversation
 * @returns the credentials we've authorized BY REFERENCE do not modify
 */
DBusCredentials*
_dbus_auth_get_identity (DBusAuth               *auth)
{
  if (auth->state == &common_state_authenticated)
    {
      return auth->authorized_identity;
    }
  else
    {
      /* FIXME instead of this, keep an empty credential around that
       * doesn't require allocation or something
       */
      /* return empty credentials */
      _dbus_assert (_dbus_credentials_are_empty (auth->authorized_identity));
      return auth->authorized_identity;
    }
}

/**
 * Gets the GUID from the server if we've authenticated; gets
 * #NULL otherwise.
 * @param auth the auth object
 * @returns the GUID in ASCII hex format
 */
const char*
_dbus_auth_get_guid_from_server (DBusAuth *auth)
{
  _dbus_assert (DBUS_AUTH_IS_CLIENT (auth));
  
  if (auth->state == &common_state_authenticated)
    return _dbus_string_get_const_data (& DBUS_AUTH_CLIENT (auth)->guid_from_server);
  else
    return NULL;
}

/**
 * Sets the "authentication context" which scopes cookies
 * with the DBUS_COOKIE_SHA1 auth mechanism for example.
 *
 * @param auth the auth conversation
 * @param context the context
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_auth_set_context (DBusAuth               *auth,
                        const DBusString       *context)
{
  return _dbus_string_replace_len (context, 0, _dbus_string_get_length (context),
                                   &auth->context, 0, _dbus_string_get_length (context));
}

/**
 * Sets whether unix fd passing is potentially on the transport and
 * hence shall be negotiated.
 *
 * @param auth the auth conversation
 * @param b TRUE when unix fd passing shall be negotiated, otherwise FALSE
 */
void
_dbus_auth_set_unix_fd_possible(DBusAuth *auth, dbus_bool_t b)
{
  auth->unix_fd_possible = b;
}

/**
 * Queries whether unix fd passing was successfully negotiated.
 *
 * @param auth the auth conversion
 * @returns #TRUE when unix fd passing was negotiated.
 */
dbus_bool_t
_dbus_auth_get_unix_fd_negotiated(DBusAuth *auth)
{
  return auth->unix_fd_negotiated;
}

/**
 * Queries whether the given auth mechanism is supported.
 *
 * @param auth the auth mechanism to query for
 * @returns #TRUE when auth mechanism is supported
 */
dbus_bool_t
_dbus_auth_is_supported_mechanism (DBusString *name)
{
  _dbus_assert (name != NULL);

  return find_mech (name, NULL) != NULL;
}

/**
 * Return a human-readable string containing all supported auth mechanisms.
 *
 * @param string to hold the supported auth mechanisms
 * @returns #FALSE on oom
 */
dbus_bool_t
_dbus_auth_dump_supported_mechanisms (DBusString *buffer)
{
  unsigned int i;
  _dbus_assert (buffer != NULL);

  for (i = 0; all_mechanisms[i].mechanism != NULL; i++)
    {
      if (i > 0)
        {
          if (!_dbus_string_append (buffer, ", "))
            return FALSE;
        }
      if (!_dbus_string_append (buffer, all_mechanisms[i].mechanism))
        return FALSE;
    }
  return TRUE;
}

/** @} */

/* tests in dbus-auth-util.c */
