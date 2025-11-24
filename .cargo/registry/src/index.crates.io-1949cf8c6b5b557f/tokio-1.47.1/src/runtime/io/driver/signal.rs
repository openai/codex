use super::{Driver, Handle, TOKEN_SIGNAL};

use std::io;

impl Handle {
    pub(crate) fn register_signal_receiver(
        &self,
        receiver: &mut mio::net::UnixStream,
    ) -> io::Result<()> {
        self.registry
            .register(receiver, TOKEN_SIGNAL, mio::Interest::READABLE)?;
        Ok(())
    }
}

impl Driver {
    pub(crate) fn consume_signal_ready(&mut self) -> bool {
        let ret = self.signal_ready;
        self.signal_ready = false;
        ret
    }
}
