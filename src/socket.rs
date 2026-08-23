//! Socket module for Zen — networking operations.
//! 
//! Provides: open, open_udp, send, send_to, recv, recv_text, recv_from, recv_all,
//! listen, accept, close, set_timeout.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions; // need to check actual path

/// Initialize the socket module by creating the socket dict and registering natives.
pub fn init_socket_module(vm: &mut crate::runtime::Vm) {
    // Socket module dict — maps method names to native function references.
    let socket = Value::Dict(crate::runtime::BTreeMap::from([
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
    ]));
    vm.vars.insert("socket".into(), socket);
    
    // Note: NATIVES entries for socket functions are registered in runtime.rs
    // const NATIVES array. They are registered eagerly at startup.
}
