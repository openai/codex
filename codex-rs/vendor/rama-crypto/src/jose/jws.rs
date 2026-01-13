use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use rama_core::error::{BoxError, ErrorContext as _, OpaqueError};
use rama_utils::macros::generate_set_and_with;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
/// When used with serde this will serialize to an empty JSON object (`{}`)
///
/// Some remarks about this struct:
/// - We use Empty {} and not Empty, because they serialize differently `{}` vs explicit `null`
/// - We don't make this `non_exhaustive` since we will never add fields to it, and using `Empty {}`
///   makes a a bit more clear that we are making an empty struct (object)
/// - In most cases you don't want to use this struct directly but instead you want to use [`NO_PAYLOAD`] or [`EMPTY_PAYLOAD`]
pub struct Empty {}

/// Serializes to a JSON null value
pub const NO_PAYLOAD: Option<&'static Empty> = None;
/// Serializes to a JSON empty object `{}`
pub const EMPTY_PAYLOAD: Option<&'static Empty> = Some(&Empty {});

#[derive(Default, Debug, Clone, PartialEq, Eq)]
/// [`JWSBuilder`] should be used when manually creating a [`JWS`], [`JWSCompact`] or [`JWSFlattened`]
pub struct JWSBuilder {
    protected_headers: Headers,
    unprotected_headers: Headers,
    payload: String,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// [`Headers`] store protected or unprotected headers and already
/// serializes them to correct JSON values.
pub struct Headers(Option<Map<String, Value>>);

impl Headers {
    generate_set_and_with! {
        /// Set provided header in the header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use `.header_map()` or `.header_map_raw()`
        /// to get access to the underlying header map
        pub fn header(
            mut self,
            name: String,
            value: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            let headers = self.0.get_or_insert_default();
            let value = serde_json::to_value(value).context("convert to value")?;
            headers.insert(name, value);
            Ok(self)
        }
    }

    generate_set_and_with! {
        /// Set provided headers in the header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use `.header_map()` or `.header_map_raw()`
        /// to get access to the underlying header map
        pub fn headers(mut self, headers: impl Serialize) -> Result<Self, OpaqueError> {
            let headers =
                serde_json::to_value(headers).context("convert headers to serde json value")?;

            let mut headers = match headers {
                Value::Object(map) => map,
                _ => Err(OpaqueError::from_display(
                    "Can only set multiple headers if input is key value object",
                ))?,
            };

            match &mut self.0 {
                Some(existing_headers) => existing_headers.append(&mut headers),
                None => self.0 = Some(headers),
            };

            Ok(self)
        }
    }

    /// Encode headers to a base64 url safe representation
    fn as_encoded_string(&self) -> Result<String, OpaqueError> {
        let encoded = match &self.0 {
            Some(headers) => {
                let headers = serde_json::to_vec(headers).context("convert to bytes")?;
                BASE64_URL_SAFE_NO_PAD.encode(headers)
            }
            None => String::new(),
        };
        Ok(encoded)
    }

    fn is_none(&self) -> bool {
        self.0.is_none()
    }

    fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// Try decode headers to the provided `T`
    pub fn decode<'de, 'a: 'de, T>(&'a self) -> Result<T, OpaqueError>
    where
        T: Deserialize<'de>,
    {
        match &self.0 {
            Some(headers) => Ok(T::deserialize(headers).context("deserialize headers into T")?),
            None => Err(OpaqueError::from_display(
                "headers are None, deserialize not supported",
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// [`ChainedJWSBuilder`] will be used to create a [`JWS`] with multiple signatures
pub struct ChainedJWSBuilder {
    signatures: Vec<Signature>,
    payload: String,
    protected_headers: Headers,
    unprotected_headers: Headers,
}

impl JWSBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    generate_set_and_with! {
        /// Add the provided payload to this [`JWSBuilder`]
        pub fn payload(mut self, payload: impl AsRef<[u8]>) -> Self {
            let payload = BASE64_URL_SAFE_NO_PAD.encode(payload);
            self.payload = payload;
            self
        }
    }

    generate_set_and_with! {
        /// Set provided header in the protected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use [`Self::protected_headers_mut`] to get access
        /// to the underlying header store
        pub fn protected_header(
            mut self,
            name: String,
            value: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            self.protected_headers.try_set_header(name, value)?;
            Ok(self)
        }
    }

    generate_set_and_with! {
        /// Set provided headers in the protected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use[`Self::protected_headers_mut]` to get access
        /// to the underlying header store
        pub fn protected_headers(mut self, headers: impl Serialize) -> Result<Self, OpaqueError> {
            self.protected_headers.try_set_headers(headers)?;
            Ok(self)
        }
    }

    /// Get mutable reference to the underlying protected header store
    ///
    /// This can be used in cases where more granual control is needed
    pub fn protected_headers_mut(&mut self) -> &mut Headers {
        &mut self.protected_headers
    }

    generate_set_and_with! {
        /// Set provided header in the unprotected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use [`Self::unprotected_headers_mut`] to get access
        /// to the underlying header store
        pub fn unprotected_header(
            mut self,
            name: String,
            value: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            self.unprotected_headers.try_set_header(name, value)?;
            Ok(self)
        }
    }

    generate_set_and_with! {
        /// Set provided headers in the unprotected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is needed, use [`Self::unprotected_headers_mut`] to get access
        /// to the underlying header store
        pub fn unprotected_headers(mut self, headers: impl Serialize) -> Result<Self, OpaqueError> {
            self.unprotected_headers.try_set_headers(headers)?;
            Ok(self)
        }
    }

    /// Get mutable reference to the underlying unprotected header store
    ///
    /// This can be used in cases where more granual control is needed
    pub fn unprotected_headers_mut(&mut self) -> &mut Headers {
        &mut self.unprotected_headers
    }

    /// Generate compact serialization of this `JWS`
    ///
    /// This only available if there is no unprotected header set
    pub fn build_compact(mut self, signer: &impl Signer) -> Result<JWSCompact, OpaqueError> {
        if self.unprotected_headers.is_some() {
            return Err(OpaqueError::from_display(
                "Compact jws does not support unprotected headers",
            ));
        }

        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;
        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        Ok(JWSCompact(format!("{signing_input}.{signature}")))
    }

    /// Build a [`JWSFlattened`]
    pub fn build_flattened(mut self, signer: &impl Signer) -> Result<JWSFlattened, OpaqueError> {
        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;

        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        Ok(JWSFlattened {
            signature: Signature {
                protected,
                unprotected: self.unprotected_headers,
                signature,
            },

            payload: self.payload,
        })
    }

    /// Build a [`JWS`]
    pub fn build_jws(mut self, signer: &impl Signer) -> Result<JWS, OpaqueError> {
        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;

        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        let signature = Signature {
            protected,
            signature,
            unprotected: self.unprotected_headers,
        };

        Ok(JWS {
            signatures: vec![signature],
            payload: self.payload,
        })
    }

    /// Create a [`ChainedJWSBuilder`] with the same payload but that can add a new set of headers
    /// and which will be signed again. This is needed to create a [`JWS`] with multiple signatures.
    pub fn add_signature(mut self, signer: &impl Signer) -> Result<ChainedJWSBuilder, OpaqueError> {
        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;

        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        let signature = Signature {
            protected,
            signature,
            unprotected: self.unprotected_headers,
        };

        Ok(ChainedJWSBuilder {
            signatures: vec![signature],
            protected_headers: Default::default(),
            unprotected_headers: Default::default(),
            payload: self.payload,
        })
    }
}

impl ChainedJWSBuilder {
    generate_set_and_with! {
        /// Set provided header in the protected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is use `.protected_headers_mut()` to get access
        /// to the underlying header store
        pub fn protected_header(
            mut self,
            name: String,
            value: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            self.protected_headers.try_set_header(name, value)?;
            Ok(self)
        }
    }

    generate_set_and_with! {
        /// Set provided headers in the protected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is use `.protected_headers_mut()` to get access
        /// to the underlying header store
        pub fn protected_headers(mut self, headers: impl Serialize) -> Result<Self, OpaqueError> {
            self.protected_headers.try_set_headers(headers)?;
            Ok(self)
        }
    }

    /// Get mutable reference to the underlying protected header store
    ///
    /// This can be used in cases where more granual control is needed
    pub fn protected_headers_mut(&mut self) -> &mut Headers {
        &mut self.protected_headers
    }

    generate_set_and_with! {
        /// Set provided header in the unprotected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is use `.unprotected_headers_mut()` to get access
        /// to the underlying header store
        pub fn unprotected_header(
            mut self,
            name: String,
            value: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            self.unprotected_headers.try_set_header(name, value)?;
            Ok(self)
        }
    }

    generate_set_and_with! {
        /// Set provided headers in the unprotected header map
        ///
        /// Warning: this function will replace already existing headers
        /// If more control is use `.unprotected_headers_mut()` to get access
        /// to the underlying header store
        pub fn with_unprotected_headers(
            mut self,
            headers: impl Serialize,
        ) -> Result<Self, OpaqueError> {
            self.unprotected_headers.try_set_headers(headers)?;
            Ok(self)
        }
    }

    /// Get mutable reference to the underlying unprotected header store
    ///
    /// This can be used in cases where more granual control is needed
    pub fn unprotected_headers_mut(&mut self) -> &mut Headers {
        &mut self.unprotected_headers
    }

    /// Create a new [`ChainedJWSBuilder`] so we can add another signature
    pub fn add_signature(mut self, signer: &impl Signer) -> Result<Self, OpaqueError> {
        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;
        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        let signature = Signature {
            protected,
            signature,
            unprotected: self.unprotected_headers,
        };

        self.signatures.push(signature);

        Ok(Self {
            signatures: self.signatures,
            protected_headers: Default::default(),
            unprotected_headers: Default::default(),
            payload: self.payload,
        })
    }

    /// Build the final [`JWS] containing all provided signatures
    pub fn build(mut self, signer: &impl Signer) -> Result<JWS, OpaqueError> {
        signer
            .set_headers(&mut self.protected_headers, &mut self.unprotected_headers)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer set headers")?;
        let protected = self.protected_headers.as_encoded_string()?;
        let signing_input = format!("{}.{}", protected, self.payload);

        let signature = signer
            .sign(&signing_input)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer sign protected data")?;
        let signature = BASE64_URL_SAFE_NO_PAD.encode(signature.as_ref());

        let signature = Signature {
            protected,
            signature,
            unprotected: self.unprotected_headers,
        };

        self.signatures.push(signature);

        Ok(JWS {
            payload: self.payload,
            signatures: self.signatures,
        })
    }
}

/// [`Signer`] implements all methods which are needed to sign our JWS requests,
/// and add the needed info to our JOSE headers (JOSE headers = protected + unprotected headers)
pub trait Signer {
    type Signature: AsRef<[u8]>;
    type Error: Into<BoxError>;

    /// Set headers which are needed to verify the final `Signature`
    ///
    /// Example headers are: `alg`, `curve`
    fn set_headers(
        &self,
        protected_headers: &mut Headers,
        unprotected_headers: &mut Headers,
    ) -> Result<(), Self::Error>;

    /// Sign the str encoded payload
    fn sign(&self, data: &str) -> Result<Self::Signature, Self::Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// [`JWSCompact`] is a compact `JWS` representation as defined in [`rfc7515, section 7.1`]
///
/// [`rfc7515, section 7.1`]: https://datatracker.ietf.org/doc/html/rfc7515#section-7.1
pub struct JWSCompact(String);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
/// [`JWSFlattened`] is a `JWS` which is optimized for a single signature, as defined in [`rfc7515, section 7.2.2`]
///
/// It does this by setting protected, header and signature at the root,
/// vs setting it in the signatures array
///
/// [`rfc7515, section 7.2.2`]: https://datatracker.ietf.org/doc/html/rfc7515#section-7.2.2
pub struct JWSFlattened {
    #[serde(default = "Default::default")]
    payload: String,
    #[serde(flatten)]
    signature: Signature,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// [`JWS`] is the general serialization format as defined in [`rfc7515, section 7.2.1`]
///
/// [`rfc7515, section 7.2.1`]: https://datatracker.ietf.org/doc/html/rfc7515#section-7.2.1
pub struct JWS {
    payload: String,
    signatures: Vec<Signature>,
}

impl JWSCompact {
    /// Create a builder which can be used to create a [`JWSCompact`]
    #[must_use]
    pub fn builder() -> JWSBuilder {
        JWSBuilder::new()
    }
}

impl JWS {
    /// Create a builder which can be used to create a [`JWS`]
    #[must_use]
    pub fn builder() -> JWSBuilder {
        JWSBuilder::new()
    }

    /// Decode this [`JWS`] to a [`DecodedJWS`] by decoding all values and checking with [`Verifier`]
    /// if all signatures are correct
    pub fn decode<V: Verifier>(self, verifier: &V) -> Result<(DecodedJWS, V::Output), OpaqueError> {
        let mut signatures = Vec::with_capacity(self.signatures.len());

        for signature in self.signatures.into_iter() {
            let protected = BASE64_URL_SAFE_NO_PAD
                .decode(&signature.protected)
                .context("decode protected header")?;

            let protected = serde_json::from_slice::<Headers>(&protected)
                .context("deserialize protected headers")?;

            let decoded_signature = BASE64_URL_SAFE_NO_PAD
                .decode(signature.signature)
                .context("decode signature")?;

            let decoded_signature = DecodedSignature {
                protected,
                signature: decoded_signature,
                unprotected: signature.unprotected,
            };

            let to_verify = ToVerifySignature {
                decoded_signature,
                signed_data: format!("{}.{}", signature.protected, self.payload),
            };
            signatures.push(to_verify);
        }

        let payload = BASE64_URL_SAFE_NO_PAD
            .decode(&self.payload)
            .context("decode payload")?;

        let verifier_output = verifier
            .verify(&payload, &signatures)
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer verify signatures")?;

        let signatures = signatures
            .into_iter()
            .map(|sig| sig.decoded_signature)
            .collect();

        Ok((
            DecodedJWS {
                signatures,
                payload,
            },
            verifier_output,
        ))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct Signature {
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default = "Default::default")]
    protected: String,
    #[serde(skip_serializing_if = "Headers::is_none")]
    #[serde(rename = "headers", default = "Default::default")]
    unprotected: Headers,
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default = "Default::default")]
    signature: String,
}

impl JWSFlattened {
    /// Create a builder which can be used to create a [`JWSFlattened`]
    #[must_use]
    pub fn builder() -> JWSBuilder {
        JWSBuilder::new()
    }

    /// Create a [`JWSCompact`] from this [`JWSFlattened`]
    pub fn as_compact(&self) -> Result<String, OpaqueError> {
        if self.signature.unprotected.is_some() {
            return Err(OpaqueError::from_display(
                "JWSCompact does not support unprotected headers",
            ));
        };

        Ok(format!(
            "{}.{}.{}",
            self.signature.protected, self.payload, self.signature.signature
        ))
    }

    /// Decode this [`JWS`] to a [`DecodedJWS`] by decoding all values and checking with [`Verifier`]
    /// if the provided signature is correct
    pub fn decode<V: Verifier>(
        self,
        verifier: &V,
    ) -> Result<(DecodedJWSFlattened, V::Output), OpaqueError> {
        let protected = BASE64_URL_SAFE_NO_PAD
            .decode(&self.signature.protected)
            .context("decode protected header")?;

        let protected = serde_json::from_slice::<Headers>(&protected)
            .context("deserialize protected headers")?;

        let decoded_signature = BASE64_URL_SAFE_NO_PAD
            .decode(self.signature.signature)
            .context("decode signature")?;

        let decoded_signature = DecodedSignature {
            protected,
            signature: decoded_signature,
            unprotected: self.signature.unprotected,
        };

        let to_verify = ToVerifySignature {
            decoded_signature,
            signed_data: format!("{}.{}", self.signature.protected, self.payload),
        };

        let payload = BASE64_URL_SAFE_NO_PAD
            .decode(&self.payload)
            .context("decode payload")?;

        let verify_output = verifier
            .verify(&payload, std::slice::from_ref(&to_verify))
            .map_err(|err| OpaqueError::from_boxed(err.into()))
            .context("signer verify signature")?;

        let signature = to_verify.decoded_signature;

        Ok((DecodedJWSFlattened { signature, payload }, verify_output))
    }
}

#[derive(Debug)]
/// Decoded version of a [`JWSFlattened`]
///
/// Data here has already been verified, so everything
/// here is ready for usage
pub struct DecodedJWSFlattened {
    payload: Vec<u8>,
    signature: DecodedSignature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Decode version of a [`JWS`]
///
/// Data here has already been verified, so everything
/// here is ready for usage
pub struct DecodedJWS {
    payload: Vec<u8>,
    signatures: Vec<DecodedSignature>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Decode version of a [`Signature`]
///
/// Data here has already been verified, so everything
/// here is ready for usage
pub struct DecodedSignature {
    protected: Headers,
    unprotected: Headers,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A `Signature` which still needs to be checked
///
/// It included a String representation of the signed data
/// so this doesn't need to be re-encoded
pub struct ToVerifySignature {
    signed_data: String,
    decoded_signature: DecodedSignature,
}

impl ToVerifySignature {
    /// Encoded String representation of protected + payload before it was decoded
    /// again. This should be used instead of re-encoding everything for efficiency
    #[must_use]
    pub fn signed_data(&self) -> &str {
        &self.signed_data
    }

    /// Reference to the [`DecodedSignature`]
    #[must_use]
    pub fn decoded_signature(&self) -> &DecodedSignature {
        &self.decoded_signature
    }
}

impl DecodedSignature {
    /// Reference to the protected [`Headers`]
    #[must_use]
    pub fn protected_headers(&self) -> &Headers {
        &self.protected
    }

    /// Trying decoding the protected headers to the provided `T`
    pub fn decode_protected_headers<'de, 'a: 'de, T: Deserialize<'de>>(
        &'a self,
    ) -> Result<T, OpaqueError> {
        self.protected.decode()
    }

    /// Reference to the unprotected [`Headers`]
    #[must_use]
    pub fn unprotected_headers(&self) -> &Headers {
        &self.unprotected
    }

    /// Trying decoding the unprotected headers to the provided `T`
    pub fn decode_unprotected_headers<'de, 'a: 'de, T: Deserialize<'de>>(
        &'a self,
    ) -> Result<T, OpaqueError> {
        self.unprotected.decode()
    }

    /// Signature which was provided for the encoded signature in decoded format
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        self.signature.as_slice()
    }
}

impl DecodedJWS {
    /// Get refence to the [`DecodedSignature`]s
    #[must_use]
    pub fn signatures(&self) -> &[DecodedSignature] {
        self.signatures.as_slice()
    }

    /// Get refence to the payload
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl DecodedJWSFlattened {
    /// Reference to the protected [`Headers`]
    #[must_use]
    pub fn protected_headers(&self) -> &Headers {
        self.signature.protected_headers()
    }

    /// Trying decoding the protected headers to the provided `T`
    pub fn decode_protected_headers<'de, 'a: 'de, T: Deserialize<'de>>(
        &'a self,
    ) -> Result<T, OpaqueError> {
        self.signature.decode_protected_headers()
    }

    /// Reference to the unprotected [`Headers`]
    #[must_use]
    pub fn unprotected_headers(&self) -> &Headers {
        self.signature.unprotected_headers()
    }

    /// Trying decoding the unprotected headers to the provided `T`
    pub fn decode_unprotected_headers<'de, 'a: 'de, T: Deserialize<'de>>(
        &'a self,
    ) -> Result<T, OpaqueError> {
        self.signature.decode_unprotected_headers()
    }

    /// Signature which was provided for the encoded signature in decoded format
    #[must_use]
    pub fn signature(&self) -> &[u8] {
        self.signature.signature()
    }

    /// Get refence to the payload
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// [`Verifier`] will be called to confirm if the received data is valid
///
/// For some algorithms all signatures need to be valid, but there are also
/// cases when only one or some need to be valid.
///
/// Warning: in some cases order of signatures is not always pre-determined,
/// so in those cases make sure that [`Verifier`] can handle this.
pub trait Verifier {
    type Error: Into<BoxError>;
    type Output;

    /// Verify if data is valid
    fn verify(
        &self,
        payload: &[u8],
        signatures: &[ToVerifySignature],
    ) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use tokio_test::assert_err;

    use super::*;

    #[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct AcmeProtected<'a> {
        alg: Option<&'a str>,
        nonce: &'a str,
    }

    #[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Random<'a> {
        data: &'a str,
    }

    struct DummyKey;

    impl Signer for DummyKey {
        type Signature = Vec<u8>;
        type Error = OpaqueError;

        fn sign(&self, data: &str) -> Result<Self::Signature, OpaqueError> {
            let mut out = data.as_bytes().to_vec();
            out.push(33);
            Ok(out)
        }

        fn set_headers(
            &self,
            protected_headers: &mut Headers,
            _unprotected_headers: &mut Headers,
        ) -> Result<(), OpaqueError> {
            protected_headers.try_set_header("alg".to_owned(), "test_algo".to_owned())?;
            Ok(())
        }
    }

    impl Verifier for DummyKey {
        type Error = OpaqueError;
        type Output = ();

        fn verify(
            &self,
            _payload: &[u8],
            to_verify_sigs: &[ToVerifySignature],
        ) -> Result<(), OpaqueError> {
            let to_verify = &to_verify_sigs[0];
            let original = to_verify.signed_data.as_bytes();

            let signature = to_verify.decoded_signature.signature();

            if original.len() + 1 != signature.len() {
                Err(OpaqueError::from_display(
                    "signature should add single u8 to original slice",
                ))
            } else if original[..] != signature[..original.len()] {
                Err(OpaqueError::from_display("original data should be equal"))
            } else if signature[signature.len() - 1] != 33 {
                Err(OpaqueError::from_display(
                    "last element in signature should be 33",
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn can_serialize_and_deserialize() {
        let nonce = "random".to_owned();
        let protected = AcmeProtected {
            nonce: &nonce,
            alg: None,
        };

        let something = "something_random".to_owned();
        let header = Random { data: &something };

        let payload = "something".to_owned();

        let signer_and_verifier = DummyKey;

        let jws = JWSBuilder::new()
            .with_payload(payload.clone())
            .try_with_protected_headers(protected.clone())
            .unwrap()
            .try_with_unprotected_headers(header.clone())
            .unwrap()
            .build_flattened(&signer_and_verifier)
            .unwrap();

        let serialized = serde_json::to_string(&jws).unwrap();
        let jws_received = serde_json::from_str::<JWSFlattened>(&serialized).unwrap();

        // This will be set by our signer
        let mut expected_protected = protected.clone();
        expected_protected.alg = Some("test_algo");

        assert_eq!(jws.signature.protected, jws_received.signature.protected);
        assert_eq!(
            jws.signature.unprotected,
            jws_received.signature.unprotected
        );
        assert_eq!(jws.payload, jws_received.payload);

        let (decoded_jws, _) = jws_received.decode(&signer_and_verifier).unwrap();

        let received_payload = String::from_utf8(decoded_jws.payload().to_vec()).unwrap();
        let received_protected = decoded_jws
            .decode_protected_headers::<AcmeProtected>()
            .unwrap();
        let received_header = decoded_jws.decode_unprotected_headers::<Random>().unwrap();

        assert_eq!(payload, received_payload);
        assert_eq!(expected_protected, received_protected);
        assert_eq!(header, received_header);
    }

    #[test]
    fn empty_vs_none() {
        let signer = DummyKey;

        let protected = AcmeProtected {
            nonce: "somthing",
            alg: None,
        };

        let jws = JWSFlattened::builder()
            .try_with_protected_headers(protected.clone())
            .unwrap()
            .build_flattened(&signer)
            .unwrap();

        assert_eq!(jws.payload, "".to_owned());

        let jws = JWSFlattened::builder()
            .with_payload(serde_json::to_vec(&EMPTY_PAYLOAD).unwrap())
            .try_with_protected_headers(protected.clone())
            .unwrap()
            .build_flattened(&signer)
            .unwrap();

        assert_eq!(jws.payload, "e30".to_owned());
    }

    #[test]
    fn tampering_should_be_detected() {
        // This is a very basic signer without and real logic behind it,
        // this test is just here to make sure we detected changes
        let nonce = "random".to_owned();
        let protected = AcmeProtected {
            nonce: &nonce,
            alg: None,
        };

        let payload = "something".to_owned();

        let signer_and_verifier = DummyKey;

        let jws = JWSFlattened::builder()
            .with_payload(payload)
            .try_with_protected_headers(protected.clone())
            .unwrap()
            .build_flattened(&signer_and_verifier)
            .unwrap();

        let serialized = serde_json::to_string(&jws).unwrap();

        // Something should fail in this part
        let server = move |serialized: String| {
            let received =
                serde_json::from_str::<JWSFlattened>(&serialized).context("decode jws")?;
            let _decoded = received.decode(&signer_and_verifier)?;
            Ok::<_, OpaqueError>(())
        };

        for i in 0..serialized.len() - 1 {
            let mut serialized: String = serialized.clone();
            serialized.insert(i, 't');
            assert_err!(server(serialized), "failed at {i}");
        }
    }

    #[test]
    fn can_create_jws() {
        let nonce = "random".to_owned();

        let something = "something_random".to_owned();
        let header = Random { data: &something };

        let payload = "something".to_owned();

        let signer_and_verifier = DummyKey;

        let jws = JWSBuilder::new()
            .with_payload(payload.clone())
            .try_with_protected_header("nonce".to_owned(), &nonce)
            .unwrap()
            .try_with_unprotected_headers(header.clone())
            .unwrap()
            .build_jws(&signer_and_verifier)
            .unwrap();

        let serialized = serde_json::to_string(&jws).unwrap();
        let received = serde_json::from_str::<JWS>(&serialized).unwrap();
        let (decoded, _) = received.decode(&signer_and_verifier).unwrap();
        let decoded_payload = String::from_utf8(decoded.payload().to_vec()).unwrap();

        assert_eq!(payload, decoded_payload);
    }

    #[test]
    fn can_create_multi_signature_jws() {
        let nonce = "random".to_owned();
        let protected = AcmeProtected {
            nonce: &nonce,
            alg: None,
        };

        let something = "something_random".to_owned();
        let header = Random { data: &something };

        let payload = "something".to_owned();
        let signer_and_verifier = DummyKey;

        let builder = JWSBuilder::new()
            .with_payload(payload.clone())
            .try_with_protected_headers(protected.clone())
            .unwrap()
            .try_with_unprotected_headers(header.clone())
            .unwrap();

        struct SecondSigner;

        impl Signer for SecondSigner {
            type Signature = Vec<u8>;
            type Error = OpaqueError;

            fn sign(&self, data: &str) -> Result<Self::Signature, OpaqueError> {
                Ok(data.as_bytes().to_owned())
            }

            fn set_headers(
                &self,
                protected_headers: &mut Headers,
                _unprotected_headers: &mut Headers,
            ) -> Result<(), OpaqueError> {
                protected_headers.try_set_header("data".to_owned(), "very protected")?;
                Ok(())
            }
        }

        let second_signer = SecondSigner;

        let jws = builder
            .add_signature(&signer_and_verifier)
            .unwrap()
            .try_with_unprotected_header("second".to_owned(), "something second")
            .unwrap()
            .try_with_protected_header("app specific".to_owned(), "will not be used by verifier")
            .unwrap()
            .build(&second_signer)
            .unwrap();

        let serialized = serde_json::to_string(&jws).unwrap();

        #[derive(Debug, Deserialize)]
        struct SecondProtectedHeader {
            data: String,
        }

        struct MultiVerifier {
            dummy_verifier: DummyKey,
        }

        impl Verifier for MultiVerifier {
            type Error = OpaqueError;
            type Output = ();
            fn verify(
                &self,
                payload: &[u8],
                to_verify_sigs: &[ToVerifySignature],
            ) -> Result<(), OpaqueError> {
                self.dummy_verifier.verify(payload, to_verify_sigs)?;
                let second = &to_verify_sigs[1];
                let protected_header = second
                    .decoded_signature
                    .decode_protected_headers::<SecondProtectedHeader>()?;

                if protected_header.data.as_str() == "very protected" {
                    Ok(())
                } else {
                    Err(OpaqueError::from_display(
                        "received unexpected second protected header",
                    ))
                }
            }
        }

        let multi_verifier = MultiVerifier {
            dummy_verifier: signer_and_verifier,
        };

        let received = serde_json::from_str::<JWS>(&serialized).unwrap();
        let (decoded, _) = received.decode(&multi_verifier).unwrap();
        let decoded_payload = String::from_utf8(decoded.payload().to_vec()).unwrap();
        assert_eq!(payload, decoded_payload);
    }
}
