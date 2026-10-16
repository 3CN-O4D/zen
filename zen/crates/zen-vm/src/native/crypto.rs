//! Native `crypto` module: hashing, HMAC, PBKDF2 and simple encodings.
//!
//! These are pure functions (no VM state), so they are dispatched from
//! [`crate::ops::native_call`] alongside the other host functions. The
//! pure-Zen `hashlib` and `uuid` modules build their API on top of this.
//!
//! Every hash returns a lowercase hex string; `pbkdf2` also returns hex so the
//! caller can `unhex` it into a binary key.

use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};
use sha3::{Sha3_256, Sha3_512};

use crate::value::Value;
use crate::vm::VmError;

/// Read argument `i` as a string, erroring as a `TypeError` otherwise.
fn str_arg(args: &[Value], i: usize) -> Result<String, VmError> {
    args.get(i)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| VmError::TypeError(format!("missing string argument {i}")))
}

fn int_arg(args: &[Value], i: usize, default: i64) -> i64 {
    args.get(i).and_then(|v| v.as_int()).unwrap_or(default)
}

fn hex(bytes: &[u8]) -> Value {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    Value::string(s)
}

/// Digest `data` with the named algorithm. Shared by the per-algorithm
/// entry points and the generic `hash`.
fn digest(alg: &str, data: &[u8]) -> Result<Vec<u8>, VmError> {
    Ok(match alg {
        "md5" => md5::compute(data).0.to_vec(),
        "sha1" => sha1::Sha1::digest(data).to_vec(),
        "sha224" => Sha224::digest(data).to_vec(),
        "sha256" => Sha256::digest(data).to_vec(),
        "sha384" => Sha384::digest(data).to_vec(),
        "sha512" => Sha512::digest(data).to_vec(),
        "sha3_256" => Sha3_256::digest(data).to_vec(),
        "sha3_512" => Sha3_512::digest(data).to_vec(),
        "blake2b" => blake2::Blake2b512::digest(data).to_vec(),
        "blake2s" => blake2::Blake2s256::digest(data).to_vec(),
        "blake3" => blake3::hash(data).as_bytes().to_vec(),
        _ => {
            return Err(VmError::ValueError(format!(
                "unknown hash algorithm: {alg}"
            )))
        }
    })
}

/// `hash(alg, data) -> hex`.
pub fn crypto_hash(args: &[Value]) -> Result<Value, VmError> {
    let alg = str_arg(args, 0)?;
    let data = str_arg(args, 1)?;
    Ok(hex(&digest(&alg, data.as_bytes())?))
}

macro_rules! hash_fn {
    ($name:ident, $alg:literal) => {
        pub fn $name(args: &[Value]) -> Result<Value, VmError> {
            let data = str_arg(args, 0)?;
            Ok(hex(&digest($alg, data.as_bytes())?))
        }
    };
}

hash_fn!(crypto_md5, "md5");
hash_fn!(crypto_sha1, "sha1");
hash_fn!(crypto_sha224, "sha224");
hash_fn!(crypto_sha256, "sha256");
hash_fn!(crypto_sha384, "sha384");
hash_fn!(crypto_sha512, "sha512");
hash_fn!(crypto_sha3_256, "sha3_256");
hash_fn!(crypto_sha3_512, "sha3_512");
hash_fn!(crypto_blake2b, "blake2b");
hash_fn!(crypto_blake2s, "blake2s");

/// `hmac(alg, key, data) -> hex`.
pub fn crypto_hmac(args: &[Value]) -> Result<Value, VmError> {
    use hmac::Mac;
    let alg = str_arg(args, 0)?;
    let key = str_arg(args, 1)?;
    let data = str_arg(args, 2)?;
    let out = match alg.as_str() {
        "sha256" => {
            let mut m = hmac::Hmac::<Sha256>::new_from_slice(key.as_bytes())
                .map_err(|_| VmError::ValueError("invalid HMAC key".into()))?;
            m.update(data.as_bytes());
            m.finalize().into_bytes().to_vec()
        }
        "sha512" => {
            let mut m = hmac::Hmac::<Sha512>::new_from_slice(key.as_bytes())
                .map_err(|_| VmError::ValueError("invalid HMAC key".into()))?;
            m.update(data.as_bytes());
            m.finalize().into_bytes().to_vec()
        }
        _ => {
            return Err(VmError::ValueError(format!(
                "unknown HMAC algorithm: {alg}"
            )))
        }
    };
    Ok(hex(&out))
}

/// `pbkdf2(password, salt, iterations, dklen?) -> hex`.
pub fn crypto_pbkdf2(args: &[Value]) -> Result<Value, VmError> {
    use pbkdf2::pbkdf2_hmac;
    let password = str_arg(args, 0)?;
    let salt = str_arg(args, 1)?;
    let iters = int_arg(args, 2, 100_000).max(1) as u32;
    let dklen = int_arg(args, 3, 32).max(1) as usize;
    let mut out = vec![0u8; dklen];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt.as_bytes(), iters, &mut out);
    Ok(hex(&out))
}

/// `hex_encode(data) -> hex`.
pub fn crypto_hex_encode(args: &[Value]) -> Result<Value, VmError> {
    let data = str_arg(args, 0)?;
    Ok(hex(data.as_bytes()))
}

/// `hex_decode(hex) -> data`.
pub fn crypto_hex_decode(args: &[Value]) -> Result<Value, VmError> {
    let s = str_arg(args, 0)?;
    if s.len() % 2 != 0 {
        return Err(VmError::ValueError("hex string has odd length".into()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char)
            .to_digit(16)
            .ok_or_else(|| VmError::ValueError("invalid hex digit".into()))?;
        let lo = (b[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| VmError::ValueError("invalid hex digit".into()))?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Ok(Value::string(String::from_utf8_lossy(&out).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::string(v)
    }

    #[test]
    fn md5_and_sha_are_known_vectors() {
        let out = crypto_md5(&[s("abc")]).unwrap();
        assert_eq!(out.as_str().unwrap(), "900150983cd24fb0d6963f7d28e17f72");
        let out = crypto_sha256(&[s("abc")]).unwrap();
        assert_eq!(
            out.as_str().unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn pbkdf2_length_is_exact() {
        let out =
            crypto_pbkdf2(&[s("password"), s("salt"), Value::int(1000), Value::int(20)]).unwrap();
        assert_eq!(out.as_str().unwrap().len(), 40);
    }
}
