//! Zen `math` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_math_module(vm: &mut Vm) {
let math = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("pi".into(), Value::Number(std::f64::consts::PI)),
    ("e".into(), Value::Number(std::f64::consts::E)),
    ("inf".into(), Value::Number(f64::INFINITY)),
    ("nan".into(), Value::Number(f64::NAN)),
    ("floor".into(), Value::NativeFunction("math_floor".into())),
    ("ceil".into(), Value::NativeFunction("math_ceil".into())),
    ("trunc".into(), Value::NativeFunction("math_trunc".into())),
    ("sqrt".into(), Value::NativeFunction("math_sqrt".into())),
    ("abs".into(), Value::NativeFunction("math_abs".into())),
    ("pow".into(), Value::NativeFunction("math_pow".into())),
    ("exp".into(), Value::NativeFunction("math_exp".into())),
    ("log".into(), Value::NativeFunction("math_log".into())),
    ("log2".into(), Value::NativeFunction("math_log2".into())),
    ("log10".into(), Value::NativeFunction("math_log10".into())),
    ("sin".into(), Value::NativeFunction("math_sin".into())),
    ("cos".into(), Value::NativeFunction("math_cos".into())),
    ("tan".into(), Value::NativeFunction("math_tan".into())),
    ("asin".into(), Value::NativeFunction("math_asin".into())),
    ("acos".into(), Value::NativeFunction("math_acos".into())),
    ("atan".into(), Value::NativeFunction("math_atan".into())),
    ("atan2".into(), Value::NativeFunction("math_atan2".into())),
    ("degrees".into(), Value::NativeFunction("math_degrees".into())),
    ("radians".into(), Value::NativeFunction("math_radians".into())),
    ("hypot".into(), Value::NativeFunction("math_hypot".into())),
    ("isnan".into(), Value::NativeFunction("math_isnan".into())),
    ("isfinite".into(), Value::NativeFunction("math_isfinite".into())),
    ("isinf".into(), Value::NativeFunction("math_isinf".into())),
    ("copysign".into(), Value::NativeFunction("math_copysign".into())),
    ("gcd".into(), Value::NativeFunction("math_gcd".into())),
    ("lcm".into(), Value::NativeFunction("math_lcm".into())),
    ("factorial".into(), Value::NativeFunction("math_factorial".into())),
    ("comb".into(), Value::NativeFunction("math_comb".into())),
    ("perm".into(), Value::NativeFunction("math_perm".into())),
    ("remainder".into(), Value::NativeFunction("math_remainder".into())),
    ("fsum".into(), Value::NativeFunction("math_fsum".into())),
    ("prod".into(), Value::NativeFunction("math_prod".into())),
    ("modf".into(), Value::NativeFunction("math_modf".into())),
    ("frexp".into(), Value::NativeFunction("math_frexp".into())),
    ("ldexp".into(), Value::NativeFunction("math_ldexp".into())),
    ("round".into(), Value::NativeFunction("math_round".into())),
    ("min".into(), Value::NativeFunction("math_min".into())),
    ("max".into(), Value::NativeFunction("math_max".into())),
])));
vm.vars.insert("math".into(), math);
}
