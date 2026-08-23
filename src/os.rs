//! OS module for Zen — operating system interface.
//! 
//! Provides: env, getenv, setenv, unsetenv, exit, platform, hostname, pid, cwd, chdir, name, sep, linesep, cpu_count, system, arch, execute.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the os module by creating the os dict and registering natives.
pub fn init_os_module(vm: &mut crate::runtime::Vm) {
    let os = Value::Dict(crate::runtime::BTreeMap::from([
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
    ]));
    vm.vars.insert("os".into(), os);
}
