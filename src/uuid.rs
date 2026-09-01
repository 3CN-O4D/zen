//! Zen `uuid` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_uuid_module(vm: &mut Vm) {
let uuid = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("uuid4".into(), Value::NativeFunction("uuid_uuid4".into())),
    ("uuid1".into(), Value::NativeFunction("uuid_uuid1".into())),
    ("uuid3".into(), Value::NativeFunction("uuid_uuid3".into())),
    ("uuid5".into(), Value::NativeFunction("uuid_uuid5".into())),
    ("v4".into(), Value::NativeFunction("uuid_uuid4".into())),
    ("v1".into(), Value::NativeFunction("uuid_uuid1".into())),
    ("v3".into(), Value::NativeFunction("uuid_uuid3".into())),
    ("v5".into(), Value::NativeFunction("uuid_uuid5".into())),
    ("NAMESPACE_DNS".into(), Value::String("dns".into())),
    ("NAMESPACE_URL".into(), Value::String("url".into())),
    ("NAMESPACE_OID".into(), Value::String("oid".into())),
    ("NAMESPACE_X500".into(), Value::String("x500".into())),
])));
vm.vars.insert("uuid".into(), uuid);
}
