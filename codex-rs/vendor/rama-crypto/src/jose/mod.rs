//! # JOSE: JSON Object Signing and Encryption
//!
//! JOSE is an IETF standard for securely transferring data between parties using JSON.
//! It provides a general framework for signing and encrypting any kind of data, and it's
//! the foundation for technologies like JSON Web Tokens (JWTs).
//!
//! The JOSE framework is made up of several key components:
//!
//! * JWS (JSON Web Signature): This specification defines how to create a digital signature for
//!   any data. A JWS proves data integrity and authenticity. It consists of a Header, a
//!   Payload (the data), and a Signature, all encoded in Base64Url and joined by dots.
//!   See [`rfc7515`] for more details.
//!
//! * JWE (JSON Web Encryption): This defines a standard way to encrypt data. A JWE ensures
//!   the confidentiality of the information, making sure only authorized parties can read it.
//!   See [`rfc7516`] for more details.
//!
//! * JWK (JSON Web Key): This specifies a JSON format for representing cryptographic keys.
//!   This makes it simple to share the public keys required to verify signatures or encrypt data.
//!   See [`rfc7517`] for more details.
//!
//! * JWA (JSON Web Algorithm): This is essentially a list of the specific cryptographic
//!   algorithms that are used for signing and encryption within the JOSE framework. The alg
//!   parameter in the JOSE header identifies which algorithm was used.
//!   See [`rfc7518`] for more details.
//!
//! [`rfc7515`]: https://datatracker.ietf.org/doc/html/rfc7515
//! [`rfc7516`]: https://datatracker.ietf.org/doc/html/rfc7516
//! [`rfc7517`]: https://datatracker.ietf.org/doc/html/rfc7517
//! [`rfc7518`]: https://datatracker.ietf.org/doc/html/rfc7518

mod jwa;
pub use jwa::JWA;

mod jwk;
pub use jwk::{EcdsaKey, JWK, JWKEllipticCurves, JWKType, JWKUse};

mod jws;
pub use jws::{
    DecodedJWS, DecodedJWSFlattened, DecodedSignature, EMPTY_PAYLOAD, Empty, Headers, JWS,
    JWSBuilder, JWSCompact, JWSFlattened, NO_PAYLOAD, Signer, ToVerifySignature, Verifier,
};
