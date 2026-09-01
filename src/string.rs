//! Zen `string` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_string_module(vm: &mut Vm) {
let string = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("upper".into(), Value::NativeFunction("str_upper".into())),
    ("lower".into(), Value::NativeFunction("str_lower".into())),
    ("title".into(), Value::NativeFunction("str_title".into())),
    ("capitalize".into(), Value::NativeFunction("str_capitalize".into())),
    ("swapcase".into(), Value::NativeFunction("str_swapcase".into())),
    ("strip".into(), Value::NativeFunction("str_strip".into())),
    ("lstrip".into(), Value::NativeFunction("str_lstrip".into())),
    ("rstrip".into(), Value::NativeFunction("str_rstrip".into())),
    ("split".into(), Value::NativeFunction("str_split".into())),
    ("splitlines".into(), Value::NativeFunction("str_splitlines".into())),
    ("join".into(), Value::NativeFunction("str_join".into())),
    ("replace".into(), Value::NativeFunction("str_replace".into())),
    ("count".into(), Value::NativeFunction("str_count".into())),
    ("find".into(), Value::NativeFunction("str_find".into())),
    ("rfind".into(), Value::NativeFunction("str_rfind".into())),
    ("startswith".into(), Value::NativeFunction("str_startswith".into())),
    ("endswith".into(), Value::NativeFunction("str_endswith".into())),
    ("contains".into(), Value::NativeFunction("str_contains".into())),
    ("ljust".into(), Value::NativeFunction("str_ljust".into())),
    ("rjust".into(), Value::NativeFunction("str_rjust".into())),
    ("center".into(), Value::NativeFunction("str_center".into())),
    ("zfill".into(), Value::NativeFunction("str_zfill".into())),
    ("repeat".into(), Value::NativeFunction("str_repeat".into())),
    ("isdigit".into(), Value::NativeFunction("str_isdigit".into())),
    ("isalpha".into(), Value::NativeFunction("str_isalpha".into())),
    ("isalnum".into(), Value::NativeFunction("str_isalnum".into())),
    ("isspace".into(), Value::NativeFunction("str_isspace".into())),
    ("islower".into(), Value::NativeFunction("str_islower".into())),
    ("isupper".into(), Value::NativeFunction("str_isupper".into())),
    ("digits".into(), Value::String("0123456789".into())),
    ("hexdigits".into(), Value::String("0123456789abcdefABCDEF".into())),
    ("octdigits".into(), Value::String("01234567".into())),
    ("ascii_letters".into(), Value::String("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".into())),
    ("ascii_lowercase".into(), Value::String("abcdefghijklmnopqrstuvwxyz".into())),
    ("ascii_uppercase".into(), Value::String("ABCDEFGHIJKLMNOPQRSTUVWXYZ".into())),
    ("punctuation".into(), Value::String("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".into())),
    ("whitespace".into(), Value::String(" \t\n\r\x0b\x0c".into())),
    ("printable".into(), Value::String("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c".into())),
])));
vm.vars.insert("string".into(), string);
}
