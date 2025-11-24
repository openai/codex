// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use super::{
    padding::{self, RsaEncoding},
    KeyPairComponents, PublicExponent, PublicKey, PublicKeyComponents, N,
};

/// RSA PKCS#1 1.5 signatures.
use crate::{
    arithmetic::{
        bigint,
        montgomery::{R, RR, RRR},
        LimbSliceError,
    },
    bits::BitLength,
    cpu, digest,
    error::{self, KeyRejected},
    io::der,
    pkcs8, rand, signature,
};

/// An RSA key pair, used for signing.
pub struct KeyPair {
    p: PrivateCrtPrime<P>,
    q: PrivateCrtPrime<Q>,
    qInv: bigint::Elem<P, R>,
    public: PublicKey,
}

derive_debug_via_field!(KeyPair, stringify!(RsaKeyPair), public);

impl KeyPair {
    /// Parses an unencrypted PKCS#8-encoded RSA private key.
    ///
    /// This will generate a 2048-bit RSA private key of the correct form using
    /// OpenSSL's command line tool:
    ///
    /// ```sh
    ///    openssl genpkey -algorithm RSA \
    ///        -pkeyopt rsa_keygen_bits:2048 \
    ///        -pkeyopt rsa_keygen_pubexp:65537 | \
    ///      openssl pkcs8 -topk8 -nocrypt -outform der > rsa-2048-private-key.pk8
    /// ```
    ///
    /// This will generate a 3072-bit RSA private key of the correct form:
    ///
    /// ```sh
    ///    openssl genpkey -algorithm RSA \
    ///        -pkeyopt rsa_keygen_bits:3072 \
    ///        -pkeyopt rsa_keygen_pubexp:65537 | \
    ///      openssl pkcs8 -topk8 -nocrypt -outform der > rsa-3072-private-key.pk8
    /// ```
    ///
    /// Often, keys generated for use in OpenSSL-based software are stored in
    /// the Base64 “PEM” format without the PKCS#8 wrapper. Such keys can be
    /// converted to binary PKCS#8 form using the OpenSSL command line tool like
    /// this:
    ///
    /// ```sh
    /// openssl pkcs8 -topk8 -nocrypt -outform der \
    ///     -in rsa-2048-private-key.pem > rsa-2048-private-key.pk8
    /// ```
    ///
    /// Base64 (“PEM”) PKCS#8-encoded keys can be converted to the binary PKCS#8
    /// form like this:
    ///
    /// ```sh
    /// openssl pkcs8 -nocrypt -outform der \
    ///     -in rsa-2048-private-key.pem > rsa-2048-private-key.pk8
    /// ```
    ///
    /// See [`Self::from_components`] for more details on how the input is
    /// validated.
    ///
    /// See [RFC 5958] and [RFC 3447 Appendix A.1.2] for more details of the
    /// encoding of the key.
    ///
    /// [NIST SP-800-56B rev. 1]:
    ///     http://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-56Br1.pdf
    ///
    /// [RFC 3447 Appendix A.1.2]:
    ///     https://tools.ietf.org/html/rfc3447#appendix-A.1.2
    ///
    /// [RFC 5958]:
    ///     https://tools.ietf.org/html/rfc5958
    pub fn from_pkcs8(pkcs8: &[u8]) -> Result<Self, KeyRejected> {
        const RSA_ENCRYPTION: &[u8] = include_bytes!("../data/alg-rsa-encryption.der");
        let (der, _) = pkcs8::unwrap_key_(
            untrusted::Input::from(RSA_ENCRYPTION),
            pkcs8::Version::V1Only,
            untrusted::Input::from(pkcs8),
        )?;
        Self::from_der(der.as_slice_less_safe())
    }

    /// Parses an RSA private key that is not inside a PKCS#8 wrapper.
    ///
    /// The private key must be encoded as a binary DER-encoded ASN.1
    /// `RSAPrivateKey` as described in [RFC 3447 Appendix A.1.2]). In all other
    /// respects, this is just like `from_pkcs8()`. See the documentation for
    /// `from_pkcs8()` for more details.
    ///
    /// It is recommended to use `from_pkcs8()` (with a PKCS#8-encoded key)
    /// instead.
    ///
    /// See [`Self::from_components()`] for more details on how the input is
    /// validated.
    ///
    /// [RFC 3447 Appendix A.1.2]:
    ///     https://tools.ietf.org/html/rfc3447#appendix-A.1.2
    ///
    /// [NIST SP-800-56B rev. 1]:
    ///     http://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-56Br1.pdf
    pub fn from_der(input: &[u8]) -> Result<Self, KeyRejected> {
        untrusted::Input::from(input).read_all(KeyRejected::invalid_encoding(), |input| {
            der::nested(
                input,
                der::Tag::Sequence,
                KeyRejected::invalid_encoding(),
                Self::from_der_reader,
            )
        })
    }

    fn from_der_reader(input: &mut untrusted::Reader) -> Result<Self, KeyRejected> {
        let version = der::small_nonnegative_integer(input)
            .map_err(|error::Unspecified| KeyRejected::invalid_encoding())?;
        if version != 0 {
            return Err(KeyRejected::version_not_supported());
        }

        fn nonnegative_integer<'a>(
            input: &mut untrusted::Reader<'a>,
        ) -> Result<&'a [u8], KeyRejected> {
            der::nonnegative_integer(input)
                .map(|input| input.as_slice_less_safe())
                .map_err(|error::Unspecified| KeyRejected::invalid_encoding())
        }

        let n = nonnegative_integer(input)?;
        let e = nonnegative_integer(input)?;
        let d = nonnegative_integer(input)?;
        let p = nonnegative_integer(input)?;
        let q = nonnegative_integer(input)?;
        let dP = nonnegative_integer(input)?;
        let dQ = nonnegative_integer(input)?;
        let qInv = nonnegative_integer(input)?;

        let components = KeyPairComponents {
            public_key: PublicKeyComponents { n, e },
            d,
            p,
            q,
            dP,
            dQ,
            qInv,
        };

        Self::from_components(&components)
    }

    /// Constructs an RSA private key from its big-endian-encoded components.
    ///
    /// Only two-prime (not multi-prime) keys are supported. The public modulus
    /// (n) must be at least 2047 bits. The public modulus must be no larger
    /// than 4096 bits. It is recommended that the public modulus be exactly
    /// 2048 or 3072 bits. The public exponent must be at least 65537 and must
    /// be no more than 33 bits long.
    ///
    /// The private key is validated according to [NIST SP-800-56B rev. 1]
    /// section 6.4.1.4.3, crt_pkv (Intended Exponent-Creation Method Unknown),
    /// with the following exceptions:
    ///
    /// * Section 6.4.1.2.1, Step 1: Neither a target security level nor an
    ///   expected modulus length is provided as a parameter, so checks
    ///   regarding these expectations are not done.
    /// * Section 6.4.1.2.1, Step 3: Since neither the public key nor the
    ///   expected modulus length is provided as a parameter, the consistency
    ///   check between these values and the private key's value of n isn't
    ///   done.
    /// * Section 6.4.1.2.1, Step 5: No primality tests are done, both for
    ///   performance reasons and to avoid any side channels that such tests
    ///   would provide.
    /// * Section 6.4.1.2.1, Step 6, and 6.4.1.4.3, Step 7:
    ///   * *ring* has a slightly looser lower bound for the values of `p`
    ///     and `q` than what the NIST document specifies. This looser lower
    ///     bound matches what most other crypto libraries do. The check might
    ///     be tightened to meet NIST's requirements in the future. Similarly,
    ///     the check that `p` and `q` are not too close together is skipped
    ///     currently, but may be added in the future.
    ///   * The validity of the mathematical relationship of `dP`, `dQ`, `e`
    ///     and `n` is verified only during signing. Some size checks of `d`,
    ///     `dP` and `dQ` are performed at construction, but some NIST checks
    ///     are skipped because they would be expensive and/or they would leak
    ///     information through side channels. If a preemptive check of the
    ///     consistency of `dP`, `dQ`, `e` and `n` with each other is
    ///     necessary, that can be done by signing any message with the key
    ///     pair.
    ///
    ///   * `d` is not fully validated, neither at construction nor during
    ///     signing. This is OK as far as *ring*'s usage of the key is
    ///     concerned because *ring* never uses the value of `d` (*ring* always
    ///     uses `p`, `q`, `dP` and `dQ` via the Chinese Remainder Theorem,
    ///     instead). However, *ring*'s checks would not be sufficient for
    ///     validating a key pair for use by some other system; that other
    ///     system must check the value of `d` itself if `d` is to be used.
    pub fn from_components<Public, Private>(
        components: &KeyPairComponents<Public, Private>,
    ) -> Result<Self, KeyRejected>
    where
        Public: AsRef<[u8]>,
        Private: AsRef<[u8]>,
    {
        let components = KeyPairComponents {
            public_key: PublicKeyComponents {
                n: components.public_key.n.as_ref(),
                e: components.public_key.e.as_ref(),
            },
            d: components.d.as_ref(),
            p: components.p.as_ref(),
            q: components.q.as_ref(),
            dP: components.dP.as_ref(),
            dQ: components.dQ.as_ref(),
            qInv: components.qInv.as_ref(),
        };
        Self::from_components_(&components, cpu::features())
    }

    fn from_components_(
        &KeyPairComponents {
            public_key,
            d,
            p,
            q,
            dP,
            dQ,
            qInv,
        }: &KeyPairComponents<&[u8]>,
        cpu_features: cpu::Features,
    ) -> Result<Self, KeyRejected> {
        let d = untrusted::Input::from(d);
        let p = untrusted::Input::from(p);
        let q = untrusted::Input::from(q);
        let dP = untrusted::Input::from(dP);
        let dQ = untrusted::Input::from(dQ);
        let qInv = untrusted::Input::from(qInv);

        // XXX: Some steps are done out of order, but the NIST steps are worded
        // in such a way that it is clear that NIST intends for them to be done
        // in order. TODO: Does this matter at all?

        // 6.4.1.4.3/6.4.1.2.1 - Step 1.

        // Step 1.a is omitted, as explained above.

        // Step 1.b is omitted per above. Instead, we check that the public
        // modulus is 2048 to `PRIVATE_KEY_PUBLIC_MODULUS_MAX_BITS` bits.
        // XXX: The maximum limit of 4096 bits is primarily due to lack of
        // testing of larger key sizes; see, in particular,
        // https://www.mail-archive.com/openssl-dev@openssl.org/msg44586.html
        // and
        // https://www.mail-archive.com/openssl-dev@openssl.org/msg44759.html.
        // Also, this limit might help with memory management decisions later.

        // Step 1.c. We validate e >= 65537.
        let n = untrusted::Input::from(public_key.n);
        let e = untrusted::Input::from(public_key.e);
        let public_key = PublicKey::from_modulus_and_exponent(
            n,
            e,
            BitLength::from_bits(2048),
            super::PRIVATE_KEY_PUBLIC_MODULUS_MAX_BITS,
            PublicExponent::_65537,
            cpu_features,
        )?;

        let n_one = public_key.inner().n().oneRR();
        let n = &public_key.inner().n().value(cpu_features);

        // 6.4.1.4.3 says to skip 6.4.1.2.1 Step 2.

        // 6.4.1.4.3 Step 3.

        // Step 3.a is done below, out of order.
        // Step 3.b is unneeded since `n_bits` is derived here from `n`.

        // 6.4.1.4.3 says to skip 6.4.1.2.1 Step 4. (We don't need to recover
        // the prime factors since they are already given.)

        // 6.4.1.4.3 - Step 5.

        // Steps 5.a and 5.b are omitted, as explained above.

        let n_bits = public_key.inner().n().len_bits();

        let p = PrivatePrime::new(p, n_bits, cpu_features)?;
        let q = PrivatePrime::new(q, n_bits, cpu_features)?;

        // TODO: Step 5.i
        //
        // 3.b is unneeded since `n_bits` is derived here from `n`.

        // 6.4.1.4.3 - Step 3.a (out of order).
        //
        // Verify that p * q == n. We restrict ourselves to modular
        // multiplication. We rely on the fact that we've verified
        // 0 < q < p < n. We check that q and p are close to sqrt(n) and then
        // assume that these preconditions are enough to let us assume that
        // checking p * q == 0 (mod n) is equivalent to checking p * q == n.
        let q_mod_n = q
            .modulus
            .to_elem(n)
            .map_err(|error::Unspecified| KeyRejected::inconsistent_components())?;
        let p_mod_n = p
            .modulus
            .to_elem(n)
            .map_err(|error::Unspecified| KeyRejected::inconsistent_components())?;
        let p_mod_n = bigint::elem_mul(n_one, p_mod_n, n);
        let pq_mod_n = bigint::elem_mul(&q_mod_n, p_mod_n, n);
        if !pq_mod_n.is_zero() {
            return Err(KeyRejected::inconsistent_components());
        }

        // 6.4.1.4.3/6.4.1.2.1 - Step 6.

        // Step 6.a, partial.
        //
        // First, validate `2**half_n_bits < d`. Since 2**half_n_bits has a bit
        // length of half_n_bits + 1, this check gives us 2**half_n_bits <= d,
        // and knowing d is odd makes the inequality strict.
        let d = bigint::OwnedModulusValue::<D>::from_be_bytes(d)
            .map_err(|_| KeyRejected::invalid_component())?;
        if !(n_bits.half_rounded_up() < d.len_bits()) {
            return Err(KeyRejected::inconsistent_components());
        }
        // XXX: This check should be `d < LCM(p - 1, q - 1)`, but we don't have
        // a good way of calculating LCM, so it is omitted, as explained above.
        d.verify_less_than(n)
            .map_err(|error::Unspecified| KeyRejected::inconsistent_components())?;

        // Step 6.b is omitted as explained above.

        let pm = &p.modulus.modulus(cpu_features);

        // 6.4.1.4.3 - Step 7.

        // Step 7.c.
        let qInv = bigint::Elem::from_be_bytes_padded(qInv, pm)
            .map_err(|error::Unspecified| KeyRejected::invalid_component())?;

        // Steps 7.d and 7.e are omitted per the documentation above, and
        // because we don't (in the long term) have a good way to do modulo
        // with an even modulus.

        // Step 7.f.
        let qInv = bigint::elem_mul(p.oneRR.as_ref(), qInv, pm);
        let q_mod_p = bigint::elem_reduced(pm.alloc_zero(), &q_mod_n, pm, q.modulus.len_bits());
        let q_mod_p = bigint::elem_mul(p.oneRR.as_ref(), q_mod_p, pm);
        bigint::verify_inverses_consttime(&qInv, q_mod_p, pm)
            .map_err(|error::Unspecified| KeyRejected::inconsistent_components())?;

        // This should never fail since `n` and `e` were validated above.

        let p = PrivateCrtPrime::new(p, dP, cpu_features)?;
        let q = PrivateCrtPrime::new(q, dQ, cpu_features)?;

        Ok(Self {
            p,
            q,
            qInv,
            public: public_key,
        })
    }

    /// Returns a reference to the public key.
    pub fn public(&self) -> &PublicKey {
        &self.public
    }

    /// Returns the length in bytes of the key pair's public modulus.
    ///
    /// A signature has the same length as the public modulus.
    #[deprecated = "Use `public().modulus_len()`"]
    #[inline]
    pub fn public_modulus_len(&self) -> usize {
        self.public().modulus_len()
    }
}

impl signature::KeyPair for KeyPair {
    type PublicKey = PublicKey;

    fn public_key(&self) -> &Self::PublicKey {
        self.public()
    }
}

struct PrivatePrime<M> {
    modulus: bigint::OwnedModulus<M>,
    oneRR: bigint::One<M, RR>,
}

impl<M> PrivatePrime<M> {
    fn new(
        p: untrusted::Input,
        n_bits: BitLength,
        cpu_features: cpu::Features,
    ) -> Result<Self, KeyRejected> {
        let p = bigint::OwnedModulusValue::from_be_bytes(p)?;

        // 5.c / 5.g:
        //
        // TODO: First, stop if `p < (√2) * 2**((nBits/2) - 1)`.
        // TODO: First, stop if `q < (√2) * 2**((nBits/2) - 1)`.
        //
        // Second, stop if `p > 2**(nBits/2) - 1`.
        // Second, stop if `q > 2**(nBits/2) - 1`.
        if p.len_bits() != n_bits.half_rounded_up() {
            return Err(KeyRejected::inconsistent_components());
        }

        if p.len_bits().as_bits() % 512 != 0 {
            return Err(KeyRejected::private_modulus_len_not_multiple_of_512_bits());
        }

        // TODO: Step 5.d: Verify GCD(p - 1, e) == 1.
        // TODO: Step 5.h: Verify GCD(q - 1, e) == 1.

        // Steps 5.e and 5.f are omitted as explained above.
        let p = bigint::OwnedModulus::from(p);
        let pm = p.modulus(cpu_features);
        let oneRR = bigint::One::newRR(pm.alloc_zero(), &pm);

        Ok(Self { modulus: p, oneRR })
    }
}

struct PrivateCrtPrime<M> {
    modulus: bigint::OwnedModulus<M>,
    oneRRR: bigint::One<M, RRR>,
    exponent: bigint::PrivateExponent,
}

impl<M> PrivateCrtPrime<M> {
    /// Constructs a `PrivateCrtPrime` from the private prime `p` and `dP` where
    /// dP == d % (p - 1).
    fn new(
        p: PrivatePrime<M>,
        dP: untrusted::Input,
        cpu_features: cpu::Features,
    ) -> Result<Self, KeyRejected> {
        let m = &p.modulus.modulus(cpu_features);
        // [NIST SP-800-56B rev. 1] 6.4.1.4.3 - Steps 7.a & 7.b.
        let dP = bigint::PrivateExponent::from_be_bytes_padded(dP, m)
            .map_err(|error::Unspecified| KeyRejected::inconsistent_components())?;

        // XXX: Steps 7.d and 7.e are omitted. We don't check that
        // `dP == d % (p - 1)` because we don't (in the long term) have a good
        // way to do modulo with an even modulus. Instead we just check that
        // `1 <= dP < p - 1`. We'll check it, to some unknown extent, when we
        // do the private key operation, since we verify that the result of the
        // private key operation using the CRT parameters is consistent with `n`
        // and `e`. TODO: Either prove that what we do is sufficient, or make
        // it so.

        let oneRRR = bigint::One::newRRR(p.oneRR, m);

        Ok(Self {
            modulus: p.modulus,
            oneRRR,
            exponent: dP,
        })
    }
}

fn elem_exp_consttime<M>(
    c: &bigint::Elem<N>,
    p: &PrivateCrtPrime<M>,
    other_prime_len_bits: BitLength,
    cpu_features: cpu::Features,
) -> Result<bigint::Elem<M>, error::Unspecified> {
    let m = &p.modulus.modulus(cpu_features);
    bigint::elem_exp_consttime(
        m.alloc_zero(),
        c,
        &p.oneRRR,
        &p.exponent,
        m,
        other_prime_len_bits,
    )
    .map_err(error::erase::<LimbSliceError>)
}

// Type-level representations of the different moduli used in RSA signing, in
// addition to `super::N`. See `super::bigint`'s modulue-level documentation.

enum P {}

enum Q {}

enum D {}

impl KeyPair {
    /// Computes the signature of `msg` and writes it into `signature`.
    ///
    /// `msg` is digested using the digest algorithm from `padding_alg` and the
    /// digest is then padded using the padding algorithm from `padding_alg`.
    ///
    /// The signature it written into `signature`; `signature`'s length must be
    /// exactly the length returned by `self::public().modulus_len()` or else
    /// an error will be returned. On failure, `signature` may contain
    /// intermediate results, but won't contain anything that would endanger the
    /// private key.
    ///
    /// `rng` may be used to randomize the padding (e.g. for PSS).
    ///
    /// Many other crypto libraries have signing functions that takes a
    /// precomputed digest as input, instead of the message to digest. This
    /// function does *not* take a precomputed digest; instead, `sign`
    /// calculates the digest itself.
    pub fn sign(
        &self,
        padding_alg: &'static dyn RsaEncoding,
        rng: &dyn rand::SecureRandom,
        msg: &[u8],
        signature: &mut [u8],
    ) -> Result<(), error::Unspecified> {
        let cpu_features = cpu::features();

        if signature.len() != self.public().modulus_len() {
            return Err(error::Unspecified);
        }

        let m_hash = digest::digest(padding_alg.digest_alg(), msg);

        // Use the output buffer as the scratch space for the signature to
        // reduce the required stack space.
        padding::encode(
            padding_alg,
            m_hash,
            signature,
            self.public().inner().n().len_bits(),
            rng,
        )?;

        // RFC 8017 Section 5.1.2: RSADP, using the Chinese Remainder Theorem
        // with Garner's algorithm.

        // Steps 1 and 2.
        let m = self.private_exponentiate(signature, cpu_features)?;

        // Step 3.
        m.fill_be_bytes(signature);

        Ok(())
    }

    /// Returns base**d (mod n).
    ///
    /// This does not return or write any intermediate results into any buffers
    /// that are provided by the caller so that no intermediate state will be
    /// leaked that would endanger the private key.
    ///
    /// Panics if `in_out` is not `self.public().modulus_len()`.
    fn private_exponentiate(
        &self,
        base: &[u8],
        cpu_features: cpu::Features,
    ) -> Result<bigint::Elem<N>, error::Unspecified> {
        assert_eq!(base.len(), self.public().modulus_len());

        // RFC 8017 Section 5.1.2: RSADP, using the Chinese Remainder Theorem
        // with Garner's algorithm.

        let n = &self.public.inner().n().value(cpu_features);
        let n_one = self.public.inner().n().oneRR();

        // Step 1. The value zero is also rejected.
        let base = bigint::Elem::from_be_bytes_padded(untrusted::Input::from(base), n)?;

        // Step 2
        let c = base;

        // Step 2.b.i.
        let q_bits = self.q.modulus.len_bits();
        let m_1 = elem_exp_consttime(&c, &self.p, q_bits, cpu_features)?;
        let m_2 = elem_exp_consttime(&c, &self.q, self.p.modulus.len_bits(), cpu_features)?;

        // Step 2.b.ii isn't needed since there are only two primes.

        // Step 2.b.iii.
        let h = {
            let p = &self.p.modulus.modulus(cpu_features);
            let m_2 = bigint::elem_reduced_once(p.alloc_zero(), &m_2, p, q_bits);
            let m_1_minus_m_2 = bigint::elem_sub(m_1, &m_2, p);
            bigint::elem_mul(&self.qInv, m_1_minus_m_2, p)
        };

        // Step 2.b.iv. The reduction in the modular multiplication isn't
        // necessary because `h < p` and `p * q == n` implies `h * q < n`.
        // Modular arithmetic is used simply to avoid implementing
        // non-modular arithmetic.
        let p_bits = self.p.modulus.len_bits();
        let h = bigint::elem_widen(n.alloc_zero(), h, n, p_bits)?;
        let q_mod_n = self.q.modulus.to_elem(n)?;
        let q_mod_n = bigint::elem_mul(n_one, q_mod_n, n);
        let q_times_h = bigint::elem_mul(&q_mod_n, h, n);
        let m_2 = bigint::elem_widen(n.alloc_zero(), m_2, n, q_bits)?;
        let m = bigint::elem_add(m_2, q_times_h, n);

        // Step 2.b.v isn't needed since there are only two primes.

        // Verify the result to protect against fault attacks as described
        // in "On the Importance of Checking Cryptographic Protocols for
        // Faults" by Dan Boneh, Richard A. DeMillo, and Richard J. Lipton.
        // This check is cheap assuming `e` is small, which is ensured during
        // `KeyPair` construction. Note that this is the only validation of `e`
        // that is done other than basic checks on its size, oddness, and
        // minimum value, since the relationship of `e` to `d`, `p`, and `q` is
        // not verified during `KeyPair` construction.
        {
            let verify = n.alloc_zero();
            let verify = self
                .public
                .inner()
                .exponentiate_elem(verify, &m, cpu_features);
            bigint::elem_verify_equal_consttime(&verify, &c)?;
        }

        // Step 3 will be done by the caller.

        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil as test;
    use alloc::vec;

    #[test]
    fn test_rsakeypair_private_exponentiate() {
        let cpu = cpu::features();
        test::run(
            test_vector_file!("keypair_private_exponentiate_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let key = test_case.consume_bytes("Key");
                let key = KeyPair::from_pkcs8(&key).unwrap();
                let test_cases = &[
                    test_case.consume_bytes("p"),
                    test_case.consume_bytes("p_plus_1"),
                    test_case.consume_bytes("p_minus_1"),
                    test_case.consume_bytes("q"),
                    test_case.consume_bytes("q_plus_1"),
                    test_case.consume_bytes("q_minus_1"),
                ];
                for test_case in test_cases {
                    // THe call to `elem_verify_equal_consttime` will cause
                    // `private_exponentiate` to fail if the computation is
                    // incorrect.
                    let mut padded = vec![0; key.public.modulus_len()];
                    let zeroes = padded.len() - test_case.len();
                    padded[zeroes..].copy_from_slice(test_case);
                    let _: bigint::Elem<_> = key.private_exponentiate(&padded, cpu).unwrap();
                }
                Ok(())
            },
        );
    }
}
