// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Duration;

use dbus::arg::RefArg;
use dbus::{arg::cast, blocking::Connection, strings::Path, Message};

use crate::proxy::prompt::{Prompt, PromptCompleted};
use crate::{Error, SecretService};

const SYSTEM_WINDOW: &str = "";
const ONE_YEAR_SECONDS: u64 = 365 * 24 * 60 * 60;

impl SecretService {
    pub(crate) fn prompt_for_create(&self, path: &Path) -> Result<Path<'static>, Error> {
        self.execute_prompt(path, handle_prompt_for_create)
    }

    pub(crate) fn prompt_for_lock_unlock_delete(&self, path: &Path) -> Result<(), Error> {
        self.execute_prompt(path, handle_prompt_for_lock_unlock_delete)
    }

    fn execute_prompt<T: Send + 'static>(
        &self,
        path: &Path,
        handler: fn(PromptCompleted) -> Result<T, Error>,
    ) -> Result<T, Error> {
        // set up handler
        #[allow(clippy::type_complexity)]
        let (tx, rx): (Sender<Result<T, Error>>, Receiver<Result<T, Error>>) = channel();
        let internal_handler = move |signal: PromptCompleted, _: &Connection, _: &Message| {
            tx.send(handler(signal)).unwrap();
            false
        };
        // execute handler
        let timeout = self.timeout.unwrap_or(ONE_YEAR_SECONDS);
        if timeout == 0 {
            return Err(Error::Prompt);
        }
        let one_second = Duration::from_millis(1000);
        let proxy = super::new_proxy(&self.connection, path);
        let token = proxy.match_signal(internal_handler)?;
        proxy.prompt(SYSTEM_WINDOW)?;
        let mut result = Err(Error::Prompt);
        for _ in 0..timeout {
            match self.connection.process(one_second) {
                Ok(false) => continue,
                Ok(true) => match rx.try_recv() {
                    Ok(res) => {
                        result = res;
                        break;
                    }
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => break,
                },
                _ => break,
            }
        }
        proxy.match_stop(token, true)?;
        result
    }
}

fn handle_prompt_for_create(signal: PromptCompleted) -> Result<Path<'static>, Error> {
    if signal.dismissed {
        Err(Error::Prompt)
    } else if let Some(first) = signal.result.as_static_inner(0) {
        if let Some(path) = cast::<Path<'_>>(first) {
            Ok(path.clone().into_static())
        } else {
            println!("Cast to path failed: {first:?}");
            Err(Error::Parse)
        }
    } else {
        println!("Can't understand prompt result: {:?}", signal.result);
        Err(Error::Parse)
    }
}

fn handle_prompt_for_lock_unlock_delete(signal: PromptCompleted) -> Result<(), Error> {
    if signal.dismissed {
        Err(Error::Prompt)
    } else {
        Ok(())
    }
}
