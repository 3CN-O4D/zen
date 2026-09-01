//! Zen `socket` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_socket_module(vm: &mut Vm) {
let socket = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("open".into(), Value::NativeFunction("socket_open".into())),
    ("open_udp".into(), Value::NativeFunction("socket_open_udp".into())),
    ("send".into(), Value::NativeFunction("socket_send".into())),
    ("send_to".into(), Value::NativeFunction("socket_send_to".into())),
    ("recv".into(), Value::NativeFunction("socket_recv".into())),
    ("recv_text".into(), Value::NativeFunction("socket_recv_text".into())),
    ("recv_from".into(), Value::NativeFunction("socket_recv_from".into())),
    ("recv_all".into(), Value::NativeFunction("socket_recv_all".into())),
    ("listen".into(), Value::NativeFunction("socket_listen".into())),
    ("accept".into(), Value::NativeFunction("socket_accept".into())),
    ("close".into(), Value::NativeFunction("socket_close".into())),
    ("set_timeout".into(), Value::NativeFunction("socket_set_timeout".into())),
    ("scan".into(), Value::NativeFunction("socket_scan".into())),
])));
vm.vars.insert("socket".into(), socket);
}
