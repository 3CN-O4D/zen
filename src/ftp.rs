//! FTP module for Zen — Pure-Rust FTP client.
//! 
//! Provides: connect, login, pwd, list, nlist, cwd, retr, stor, dele, mkdir, rmdir, rename, quit.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the FTP module by creating the ftp dict and registering natives.
pub fn init_ftp_module(vm: &mut crate::runtime::Vm) {
    let ftp = Value::Dict(crate::runtime::BTreeMap::from([
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
    ]));
    vm.vars.insert("ftp".into(), ftp);
}
