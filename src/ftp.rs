//! Zen `ftp` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_ftp_module(vm: &mut Vm) {
let ftp = Value::Dict(Arc::new(BTreeMap::from([
    ("connect".into(), Value::NativeFunction("ftp_connect".into())),
    ("login".into(), Value::NativeFunction("ftp_login".into())),
    ("pwd".into(), Value::NativeFunction("ftp_pwd".into())),
    ("list".into(), Value::NativeFunction("ftp_list".into())),
    ("nlist".into(), Value::NativeFunction("ftp_nlist".into())),
    ("cwd".into(), Value::NativeFunction("ftp_cwd".into())),
    ("retr".into(), Value::NativeFunction("ftp_retr".into())),
    ("stor".into(), Value::NativeFunction("ftp_stor".into())),
    ("dele".into(), Value::NativeFunction("ftp_dele".into())),
    ("mkdir".into(), Value::NativeFunction("ftp_mkdir".into())),
    ("rmdir".into(), Value::NativeFunction("ftp_rmdir".into())),
    ("rename".into(), Value::NativeFunction("ftp_rename".into())),
    ("quit".into(), Value::NativeFunction("ftp_quit".into())),
])));
vm.vars.insert("ftp".into(), ftp);
}
