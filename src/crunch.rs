//! Crunch module for Zen — Rust-native password wordlist generator.
//! 
//! Provides: charset, generate, pattern.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the crunch module by creating the crunch dict and registering natives.
pub fn init_crunch_module(vm: &mut crate::runtime::Vm) {
    let crunch = Value::Dict(crate::runtime::BTreeMap::from([
        ("charset".into(), Value::NativeFunction("crunch_charset".into())),
        ("generate".into(), Value::NativeFunction("crunch_generate".into())),
        ("pattern".into(), Value::NativeFunction("crunch_pattern".into())),
    ]));
    vm.vars.insert("crunch".into(), crunch);
}
