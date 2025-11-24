use core::fmt::Debug;

#[cfg(all(doc, feature = "std"))]
use crate::KeyLogFile;

/// This trait represents the ability to do something useful
/// with key material, such as logging it to a file for debugging.
///
/// Naturally, secrets passed over the interface are *extremely*
/// sensitive and can break the security of past, present and
/// future sessions.
///
/// You'll likely want some interior mutability in your
/// implementation to make this useful.
///
/// See [`KeyLogFile`] that implements the standard
/// `SSLKEYLOGFILE` environment variable behaviour.
pub trait KeyLog: Debug + Send + Sync {
    /// Log the given `secret`.  `client_random` is provided for
    /// session identification.  `label` describes precisely what
    /// `secret` means:
    ///
    /// - `CLIENT_RANDOM`: `secret` is the master secret for a TLSv1.2 session.
    /// - `CLIENT_EARLY_TRAFFIC_SECRET`: `secret` encrypts early data
    ///   transmitted by a client
    /// - `SERVER_HANDSHAKE_TRAFFIC_SECRET`: `secret` encrypts
    ///   handshake messages from the server during a TLSv1.3 handshake.
    /// - `CLIENT_HANDSHAKE_TRAFFIC_SECRET`: `secret` encrypts
    ///   handshake messages from the client during a TLSv1.3 handshake.
    /// - `SERVER_TRAFFIC_SECRET_0`: `secret` encrypts post-handshake data
    ///   from the server in a TLSv1.3 session.
    /// - `CLIENT_TRAFFIC_SECRET_0`: `secret` encrypts post-handshake data
    ///   from the client in a TLSv1.3 session.
    /// - `EXPORTER_SECRET`: `secret` is the post-handshake exporter secret
    ///   in a TLSv1.3 session.
    ///
    /// These strings are selected to match the NSS key log format:
    /// <https://nss-crypto.org/reference/security/nss/legacy/key_log_format/index.html>
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]);

    /// Indicates whether the secret with label `label` will be logged.
    ///
    /// If `will_log` returns true then `log` will be called with the secret.
    /// Otherwise, `log` will not be called for the secret. This is a
    /// performance optimization.
    fn will_log(&self, _label: &str) -> bool {
        true
    }
}

/// KeyLog that does exactly nothing.
#[derive(Debug)]
pub struct NoKeyLog;

impl KeyLog for NoKeyLog {
    fn log(&self, _: &str, _: &[u8], _: &[u8]) {}
    #[inline]
    fn will_log(&self, _label: &str) -> bool {
        false
    }
}
