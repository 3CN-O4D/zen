//! Zen `fs` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_fs_module(vm: &mut Vm) {
let fs = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("list".into(), Value::NativeFunction("fs_list_dir".into())),
    ("read".into(), Value::NativeFunction("fs_read".into())),
    ("write".into(), Value::NativeFunction("fs_write".into())),
    ("append".into(), Value::NativeFunction("fs_append".into())),
    ("read_binary".into(), Value::NativeFunction("fs_read_binary".into())),
    ("readBinary".into(), Value::NativeFunction("fs_read_binary".into())),
    ("write_binary".into(), Value::NativeFunction("fs_write_binary".into())),
    ("writeBinary".into(), Value::NativeFunction("fs_write_binary".into())),
    ("exists".into(), Value::NativeFunction("fs_exists".into())),
    ("is_file".into(), Value::NativeFunction("fs_is_file".into())),
    ("isFile".into(), Value::NativeFunction("fs_is_file".into())),
    ("is_dir".into(), Value::NativeFunction("fs_is_dir".into())),
    ("isDir".into(), Value::NativeFunction("fs_is_dir".into())),
    ("size".into(), Value::NativeFunction("fs_size".into())),
    ("mtime".into(), Value::NativeFunction("fs_mtime".into())),
    ("mkdir".into(), Value::NativeFunction("fs_mkdir".into())),
    ("mkdirs".into(), Value::NativeFunction("fs_mkdir".into())),
    ("remove".into(), Value::NativeFunction("fs_remove".into())),
    ("rmdir".into(), Value::NativeFunction("fs_rmdir".into())),
    ("rmtree".into(), Value::NativeFunction("fs_rmtree".into())),
    ("copy".into(), Value::NativeFunction("fs_copy".into())),
    ("move".into(), Value::NativeFunction("fs_move".into())),
    ("rename".into(), Value::NativeFunction("fs_move".into())),
    ("glob".into(), Value::NativeFunction("fs_glob".into())),
    ("join".into(), Value::NativeFunction("fs_join".into())),
    ("basename".into(), Value::NativeFunction("fs_basename".into())),
    ("dirname".into(), Value::NativeFunction("fs_dirname".into())),
    ("cwd".into(), Value::NativeFunction("os_cwd".into())),
    ("cd".into(), Value::NativeFunction("fs_cd".into())),
])));
vm.vars.insert("fs".into(), fs);
}
