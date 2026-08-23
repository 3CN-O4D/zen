//! Zen `hashlib` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_hashlib_module(vm: &mut Vm) {
let hashlib = Value::Dict(Arc::new(BTreeMap::from([
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
    ("pbkdf2_hmac".into(), Value::NativeFunction("crypto_pbkdf2".into())),
    ("create".into(), Value::NativeFunction("hashlib_new".into())),
    ("algorithms_available".into(), Value::List(Arc::new(vec![
        Value::String("md5".into()),
        Value::String("sha1".into()),
        Value::String("sha224".into()),
        Value::String("sha256".into()),
        Value::String("sha384".into()),
        Value::String("sha512".into()),
        Value::String("sha3_256".into()),
        Value::String("sha3_512".into()),
        Value::String("blake2b".into()),
        Value::String("blake2s".into()),
    ]))),
])));
vm.vars.insert("hashlib".into(), hashlib);
}
