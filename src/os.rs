//! Zen `os` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_os_module(vm: &mut Vm) {
let os = Value::Dict(BTreeMap::from([
    ("env".into(), Value::NativeFunction("os_getenv".into())),
    ("getenv".into(), Value::NativeFunction("os_getenv".into())),
    ("setenv".into(), Value::NativeFunction("os_setenv".into())),
    ("unsetenv".into(), Value::NativeFunction("os_unsetenv".into())),
    ("exit".into(), Value::NativeFunction("exit".into())),
    ("platform".into(), Value::NativeFunction("os_platform".into())),
    ("hostname".into(), Value::NativeFunction("os_hostname".into())),
    ("pid".into(), Value::NativeFunction("os_pid".into())),
    ("cwd".into(), Value::NativeFunction("os_cwd".into())),
    ("chdir".into(), Value::NativeFunction("fs_cd".into())),
    ("name".into(), Value::String(std::env::consts::OS.into())),
    ("sep".into(), Value::String(std::path::MAIN_SEPARATOR.to_string())),
    ("linesep".into(), Value::String("\n".into())),
    ("cpu_count".into(), Value::NativeFunction("os_cpu_count".into())),
    ("system".into(), Value::NativeFunction("os_system".into())),
    ("arch".into(), Value::NativeFunction("os_arch".into())),
    ("execute".into(), Value::NativeFunction("os_execute".into())),
    ("run".into(), Value::NativeFunction("os_run".into())),
    ("popen".into(), Value::NativeFunction("os_popen".into())),
    ("args".into(), Value::NativeFunction("os_args".into())),
    ("pids".into(), Value::NativeFunction("os_pids".into())),
    ("kill".into(), Value::NativeFunction("os_kill".into())),
    ("home".into(), Value::NativeFunction("os_home".into())),
]));
vm.vars.insert("os".into(), os);
}
