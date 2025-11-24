//Copyright 2016 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! Contains helpers for:
//!   locking/unlocking
//!   exec_prompt
//!   formatting secrets

use crate::error::Error;
use crate::proxy::prompt::{Completed, PromptProxy, PromptProxyBlocking};
use crate::proxy::service::{ServiceProxy, ServiceProxyBlocking};
use crate::proxy::SecretStruct;
use crate::session::encrypt;
use crate::session::Session;
use crate::ss::SS_DBUS_NAME;

use rand::{rngs::OsRng, Rng};
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::{
    zvariant::{self, ObjectPath},
    CacheProperties,
};

// Helper enum for locking
pub(crate) enum LockAction {
    Lock,
    Unlock,
}

pub(crate) async fn lock_or_unlock(
    conn: zbus::Connection,
    service_proxy: &ServiceProxy<'_>,
    object_path: &ObjectPath<'_>,
    lock_action: LockAction,
) -> Result<(), Error> {
    let objects = vec![object_path];

    let lock_action_res = match lock_action {
        LockAction::Lock => service_proxy.lock(objects).await?,
        LockAction::Unlock => service_proxy.unlock(objects).await?,
    };

    if lock_action_res.object_paths.is_empty() {
        exec_prompt(conn, &lock_action_res.prompt).await?;
    }
    Ok(())
}

pub(crate) fn lock_or_unlock_blocking(
    conn: zbus::blocking::Connection,
    service_proxy: &ServiceProxyBlocking,
    object_path: &ObjectPath,
    lock_action: LockAction,
) -> Result<(), Error> {
    let objects = vec![object_path];

    let lock_action_res = match lock_action {
        LockAction::Lock => service_proxy.lock(objects)?,
        LockAction::Unlock => service_proxy.unlock(objects)?,
    };

    if lock_action_res.object_paths.is_empty() {
        exec_prompt_blocking(conn, &lock_action_res.prompt)?;
    }
    Ok(())
}

pub(crate) fn format_secret(
    session: &Session,
    secret: &[u8],
    content_type: &str,
) -> Result<SecretStruct, Error> {
    let content_type = content_type.to_owned();

    if let Some(session_key) = session.get_aes_key() {
        let mut rng = OsRng {};
        let mut aes_iv = [0; 16];
        rng.fill(&mut aes_iv);

        let encrypted_secret = encrypt(secret, session_key, &aes_iv);

        // Construct secret struct
        let parameters = aes_iv.to_vec();
        let value = encrypted_secret;

        Ok(SecretStruct {
            session: session.object_path.clone(),
            parameters,
            value,
            content_type,
        })
    } else {
        // just Plain for now
        let parameters = Vec::new();
        let value = secret.to_vec();

        Ok(SecretStruct {
            session: session.object_path.clone(),
            parameters,
            value,
            content_type,
        })
    }
}

// TODO: Users could pass their own window ID in.
const NO_WINDOW_ID: &str = "";

pub(crate) async fn exec_prompt(
    conn: zbus::Connection,
    prompt: &ObjectPath<'_>,
) -> Result<zvariant::OwnedValue, Error> {
    let prompt_proxy = PromptProxy::builder(&conn)
        .destination(SS_DBUS_NAME)?
        .path(prompt)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;

    let mut receive_completed_iter = prompt_proxy.receive_completed().await?;
    prompt_proxy.prompt(NO_WINDOW_ID).await?;

    handle_signal(receive_completed_iter.next().await.unwrap())
}

pub(crate) fn exec_prompt_blocking(
    conn: zbus::blocking::Connection,
    prompt: &ObjectPath,
) -> Result<zvariant::OwnedValue, Error> {
    let prompt_proxy = PromptProxyBlocking::builder(&conn)
        .destination(SS_DBUS_NAME)?
        .path(prompt)?
        .cache_properties(CacheProperties::No)
        .build()?;

    let mut receive_completed_iter = prompt_proxy.receive_completed()?;
    prompt_proxy.prompt(NO_WINDOW_ID)?;

    handle_signal(receive_completed_iter.next().unwrap())
}

fn handle_signal(signal: Completed) -> Result<zvariant::OwnedValue, Error> {
    let args = signal.args()?;
    if args.dismissed {
        Err(Error::Prompt)
    } else {
        zvariant::OwnedValue::try_from(args.result).map_err(From::from)
    }
}

pub(crate) fn handle_conn_error(e: zbus::Error) -> Error {
    match e {
        zbus::Error::InterfaceNotFound | zbus::Error::Address(_) => Error::Unavailable,
        zbus::Error::InputOutput(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Error::Unavailable
        }
        e => e.into(),
    }
}
