//! Zen `pathlib` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_pathlib_module(vm: &mut Vm) {
let pathlib = Value::Dict(BTreeMap::from([
    ("join".into(), Value::NativeFunction("pathlib_join".into())),
    ("name".into(), Value::NativeFunction("pathlib_name".into())),
    ("parent".into(), Value::NativeFunction("pathlib_parent".into())),
    ("stem".into(), Value::NativeFunction("pathlib_stem".into())),
    ("suffix".into(), Value::NativeFunction("pathlib_suffix".into())),
    ("suffixes".into(), Value::NativeFunction("pathlib_suffixes".into())),
    ("is_absolute".into(), Value::NativeFunction("pathlib_is_absolute".into())),
    ("resolve".into(), Value::NativeFunction("pathlib_resolve".into())),
    ("absolute".into(), Value::NativeFunction("pathlib_absolute".into())),
    ("exists".into(), Value::NativeFunction("pathlib_exists".into())),
    ("is_file".into(), Value::NativeFunction("pathlib_is_file".into())),
    ("is_dir".into(), Value::NativeFunction("pathlib_is_dir".into())),
    ("glob".into(), Value::NativeFunction("pathlib_glob".into())),
    ("touch".into(), Value::NativeFunction("pathlib_touch".into())),
    ("mkdir".into(), Value::NativeFunction("pathlib_mkdir".into())),
    ("rmdir".into(), Value::NativeFunction("pathlib_rmdir".into())),
    ("unlink".into(), Value::NativeFunction("pathlib_unlink".into())),
    ("rename".into(), Value::NativeFunction("pathlib_rename".into())),
    ("read_text".into(), Value::NativeFunction("pathlib_read_text".into())),
    ("write_text".into(), Value::NativeFunction("pathlib_write_text".into())),
]));
vm.vars.insert("pathlib".into(), pathlib);
}
