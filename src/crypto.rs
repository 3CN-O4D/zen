//! Zen `crypto` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_crypto_module(vm: &mut Vm) {
let crypto = Value::Dict(Arc::new(BTreeMap::from([
    ("sha256".into(), Value::NativeFunction("crypto_sha256".into())),
    ("sha1".into(), Value::NativeFunction("crypto_sha1".into())),
    ("md5".into(), Value::NativeFunction("crypto_md5".into())),
    ("sha512".into(), Value::NativeFunction("crypto_sha512".into())),
    ("sha224".into(), Value::NativeFunction("crypto_sha224".into())),
    ("sha384".into(), Value::NativeFunction("crypto_sha384".into())),
    ("sha3_256".into(), Value::NativeFunction("crypto_sha3_256".into())),
    ("sha3_512".into(), Value::NativeFunction("crypto_sha3_512".into())),
    ("blake2b".into(), Value::NativeFunction("crypto_blake2b".into())),
    ("blake2s".into(), Value::NativeFunction("crypto_blake2s".into())),
    ("hmac_sha256".into(), Value::NativeFunction("crypto_hmac_sha256".into())),
    ("hmac_sha1".into(), Value::NativeFunction("crypto_hmac_sha1".into())),
    ("hmac_md5".into(), Value::NativeFunction("crypto_hmac_md5".into())),
    ("random_bytes".into(), Value::NativeFunction("crypto_random_bytes".into())),
    ("random_hex".into(), Value::NativeFunction("crypto_random_hex".into())),
    ("pbkdf2".into(), Value::NativeFunction("crypto_pbkdf2".into())),
    ("aes_encrypt".into(), Value::NativeFunction("crypto_aes_encrypt".into())),
    ("aes_decrypt".into(), Value::NativeFunction("crypto_aes_decrypt".into())),
])));
vm.vars.insert("crypto".into(), crypto);
}
