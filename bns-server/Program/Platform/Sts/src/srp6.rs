//! The 2020 client variant in Docs/Platform/Sts/07-client-login-wire.md.
//! Public parameters only. This implementation is not independently audited.
//! BigUint arithmetic is variable-time. Use only for the local test service.
use num_bigint::BigUint;
use num_traits::Zero;
use rand::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};

pub const SCHEME: &str = "sts-2020-sha256-word-srp6a";
/// RFC 5054 Appendix A, 1024-bit group, in standard (big-endian) hex.
pub const N_HEX: &str = concat!(
    "EEAF0AB9ADB38DD69C33F80AFA8FC5E86072618775FF3C0B9EA2314C9C256576",
    "D674DF7496EA81D3383B4813D692C6E0E0D5D8E250B98BE48E495C1D6089DAD15",
    "DC7D7B46154D6B6CE8EF4AD69B15D4982559B297BCF1885C529F566660E57EC68",
    "EDBC3C05726CC02FD4CBF4976EAA9AFD5138FE8376435B9FC61D2FC0EB06E3"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidNumber,
    InvalidVerifier,
    InvalidPublic,
    InvalidProof,
    InvalidSecret,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "STS SRP error: {self:?}")
    }
}
impl std::error::Error for Error {}

pub fn modulus() -> BigUint {
    BigUint::parse_bytes(N_HEX.as_bytes(), 16).expect("public RFC group")
}
/// Raw little-endian u32 words, with high zero words removed. Zero has no words.
pub fn number_bytes(n: &BigUint) -> Vec<u8> {
    n.to_u32_digits()
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
}
pub fn read_number(bytes: &[u8]) -> Result<BigUint, Error> {
    if !bytes.len().is_multiple_of(4) {
        return Err(Error::InvalidNumber);
    }
    Ok(BigUint::from_bytes_le(bytes))
}
fn hash(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}
/// Reverse words, not bytes within each word, before importing a digest.
fn hash_number(digest: [u8; 32]) -> BigUint {
    let bytes: Vec<u8> = digest
        .as_chunks::<4>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied()
        .collect();
    BigUint::from_bytes_le(&bytes)
}
fn generator() -> BigUint {
    BigUint::from(2u32)
}
fn multiplier(n: &BigUint) -> BigUint {
    hash_number(hash(&[&number_bytes(n), &2u32.to_le_bytes()]))
}
fn password_x(login: &str, password: &[u8], salt: &[u8]) -> BigUint {
    hash_number(hash(&[salt, &hash(&[login.as_bytes(), b":", password])]))
}
/// RFC 2945-style interleave, as StsMsgCli32.dll 0.122 `sub_10031040` does it.
/// The client skips low-end bytes while they are NOT zero (`0x10031061`:
/// `cmp byte [eax],0` / `jz` exits the loop), so the hash input starts at the
/// first zero byte. Checked against the DLL in emulation, 2026-11-08.
fn session_key(secret: &BigUint) -> [u8; 64] {
    let bytes = number_bytes(secret);
    let mut s = bytes.as_slice();
    while s.len() > 4 && s[0] != 0 {
        s = &s[1..];
    }
    if !s.len().is_multiple_of(2) {
        s = &s[1..];
    }
    let reverse: Vec<u8> = s.iter().rev().copied().collect();
    let even: Vec<u8> = reverse.iter().step_by(2).copied().collect();
    let odd: Vec<u8> = reverse.iter().skip(1).step_by(2).copied().collect();
    let (h0, h1) = (hash(&[&even]), hash(&[&odd]));
    let mut key = [0; 64];
    for i in 0..32 {
        key[2 * i] = h0[i];
        key[2 * i + 1] = h1[i];
    }
    key
}
fn client_proof(login: &str, salt: &[u8], a: &[u8], b: &[u8], key: &[u8; 64]) -> [u8; 32] {
    let hn = hash(&[&number_bytes(&modulus())]);
    let hg = hash(&[&2u32.to_le_bytes()]);
    let xor: Vec<u8> = hn.iter().zip(hg).map(|(a, b)| a ^ b).collect();
    hash(&[&xor, &hash(&[login.as_bytes()]), salt, a, b, key])
}
fn equal_proof(a: &[u8], b: &[u8; 32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

/// Only the salt and verifier leave enrollment. No plaintext is stored.
pub struct Enrollment {
    pub salt: Vec<u8>,
    pub verifier: Vec<u8>,
}
pub fn enroll<R: RngCore + CryptoRng>(login: &str, password: &[u8], rng: &mut R) -> Enrollment {
    let mut salt = [0; 32];
    loop {
        rng.fill_bytes(&mut salt);
        if salt != [0; 32] {
            break;
        }
    }
    let salt = number_bytes(&BigUint::from_bytes_le(&salt));
    let verifier = generator().modpow(&password_x(login, password, &salt), &modulus());
    Enrollment {
        salt,
        verifier: number_bytes(&verifier),
    }
}

/// Verifier for a given salt. For account tools: the server never gets the password.
pub fn verifier(login: &str, password: &[u8], salt: &[u8]) -> Vec<u8> {
    number_bytes(&generator().modpow(&password_x(login, password, salt), &modulus()))
}

/// (A, M1, expected M2, K) of [`client_model`].
pub type ClientModel = (Vec<u8>, [u8; 32], [u8; 32], [u8; 64]);

/// Client side of the exchange. A model for tests and loopback tools only.
/// Input: the challenge (salt, B) and 32 random bytes for a.
/// Output: (A, M1, expected M2, K).
pub fn client_model(
    login: &str,
    password: &[u8],
    salt: &[u8],
    server_public: &[u8],
    random: &[u8; 32],
) -> Result<ClientModel, Error> {
    let n = modulus();
    let b = read_number(server_public)?;
    let a = BigUint::from_bytes_le(random);
    let a_pub = number_bytes(&generator().modpow(&a, &n));
    let u = hash_number(hash(&[&a_pub, server_public]));
    let x = password_x(login, password, salt);
    let gx = generator().modpow(&x, &n);
    let base = (&b + &n * multiplier(&n) - (multiplier(&n) * gx) % &n) % &n;
    let key = session_key(&base.modpow(&(a + u * x), &n));
    let m1 = client_proof(login, salt, &a_pub, server_public, &key);
    let m2 = hash(&[&a_pub, &m1, &key]);
    Ok((a_pub, m1, m2, key))
}

/// Consumed by step 2. No Debug: contains ephemeral private data.
pub struct ServerExchange {
    login: String,
    salt: Vec<u8>,
    verifier: BigUint,
    private: BigUint,
    public: Vec<u8>,
}
pub struct Verified {
    pub proof: [u8; 32],
    pub key: [u8; 64],
}
impl ServerExchange {
    /// The caller supplies 32 random bytes for b. Do not reuse them.
    pub fn start(
        login: &str,
        salt: &[u8],
        verifier: &[u8],
        random: &[u8; 32],
    ) -> Result<Self, Error> {
        if salt.is_empty() || salt.len() > 1024 || verifier.is_empty() || verifier.len() > 128 {
            return Err(Error::InvalidVerifier);
        }
        let n = modulus();
        let v = read_number(verifier)?;
        let s = read_number(salt)?;
        if salt.is_empty()
            || salt.len() > 1024
            || s.is_zero()
            || number_bytes(&s) != salt
            || v.is_zero()
            || v >= n
            || number_bytes(&v) != verifier
        {
            return Err(Error::InvalidVerifier);
        }
        let private = BigUint::from_bytes_le(random);
        if private.is_zero() {
            return Err(Error::InvalidSecret);
        }
        let public = (multiplier(&n) * &v + generator().modpow(&private, &n)) % &n;
        if public.is_zero() {
            return Err(Error::InvalidPublic);
        }
        Ok(Self {
            login: login.into(),
            salt: salt.to_vec(),
            verifier: v,
            private,
            public: number_bytes(&public),
        })
    }
    pub fn public(&self) -> &[u8] {
        &self.public
    }
    pub fn verify(self, public: &[u8], proof: &[u8]) -> Result<Verified, Error> {
        if public.is_empty() || public.len() > 128 || proof.len() != 32 {
            return Err(Error::InvalidPublic);
        }
        let n = modulus();
        let a = read_number(public)?;
        if (&a % &n).is_zero() {
            return Err(Error::InvalidPublic);
        }
        let a_bytes = number_bytes(&a);
        let u = hash_number(hash(&[&a_bytes, &self.public]));
        if u.is_zero() {
            return Err(Error::InvalidPublic);
        }
        let secret = (a * self.verifier.modpow(&u, &n) % &n).modpow(&self.private, &n);
        let key = session_key(&secret);
        let m1 = client_proof(&self.login, &self.salt, &a_bytes, &self.public, &key);
        if !equal_proof(proof, &m1) {
            return Err(Error::InvalidProof);
        }
        Ok(Verified {
            proof: hash(&[&a_bytes, &m1, &key]),
            key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_is_word_based_not_digest_big_endian() {
        let digest = std::array::from_fn(|i| i as u8);
        let expected: Vec<_> = (0..8).rev().flat_map(|i| (i * 4)..(i * 4 + 4)).collect();
        assert_eq!(number_bytes(&hash_number(digest)), expected);
        assert_eq!(number_bytes(&generator()), [2, 0, 0, 0]);
        assert_eq!(modulus().bits(), 1024);
        assert_eq!(number_bytes(&modulus()).len(), 128);
        assert_eq!(
            number_bytes(&read_number(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap()),
            [1, 0, 0, 0]
        );
        assert!(read_number(&[1, 2, 3]).is_err());
    }
    #[test]
    fn interleave_skips_to_first_zero_byte_and_handles_odd_length() {
        let s = BigUint::from_bytes_le(&[5, 0, 7, 8, 9, 10, 11, 12]);
        // Skip one non-zero low byte, then drop one low byte to get an even length.
        let e = Sha256::digest([12, 10, 8]);
        let o = Sha256::digest([11, 9, 7]);
        let expected: Vec<_> = e.iter().zip(o.iter()).flat_map(|(e, o)| [*e, *o]).collect();
        assert_eq!(session_key(&s).as_slice(), expected);
    }
}
