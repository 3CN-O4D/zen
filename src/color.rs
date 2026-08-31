//! Zen `color` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_color_module(vm: &mut Vm) {
let color = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("reset".into(), Value::String("\x1b[0m".into())),
    ("bold".into(), Value::NativeFunction("color_style_bold".into())),
    ("dim".into(), Value::NativeFunction("color_style_dim".into())),
    ("italic".into(), Value::NativeFunction("color_style_italic".into())),
    ("underline".into(), Value::NativeFunction("color_style_underline".into())),
    ("blink".into(), Value::NativeFunction("color_style_blink".into())),
    ("reverse".into(), Value::NativeFunction("color_style_reverse".into())),
    ("hidden".into(), Value::NativeFunction("color_style_hidden".into())),
    ("strike".into(), Value::NativeFunction("color_style_strike".into())),
    ("rgb".into(), Value::NativeFunction("color_rgb".into())),
    ("bg_rgb".into(), Value::NativeFunction("color_bg_rgb".into())),
    ("hex".into(), Value::NativeFunction("color_hex".into())),
    ("strip".into(), Value::NativeFunction("color_strip".into())),
    ("black".into(), Value::NativeFunction("color_fg_black".into())),
    ("red".into(), Value::NativeFunction("color_fg_red".into())),
    ("green".into(), Value::NativeFunction("color_fg_green".into())),
    ("yellow".into(), Value::NativeFunction("color_fg_yellow".into())),
    ("blue".into(), Value::NativeFunction("color_fg_blue".into())),
    ("magenta".into(), Value::NativeFunction("color_fg_magenta".into())),
    ("cyan".into(), Value::NativeFunction("color_fg_cyan".into())),
    ("white".into(), Value::NativeFunction("color_fg_white".into())),
    ("bg_black".into(), Value::NativeFunction("color_bg_black".into())),
    ("bg_red".into(), Value::NativeFunction("color_bg_red".into())),
    ("bg_green".into(), Value::NativeFunction("color_bg_green".into())),
    ("bg_yellow".into(), Value::NativeFunction("color_bg_yellow".into())),
    ("bg_blue".into(), Value::NativeFunction("color_bg_blue".into())),
    ("bg_magenta".into(), Value::NativeFunction("color_bg_magenta".into())),
    ("bg_cyan".into(), Value::NativeFunction("color_bg_cyan".into())),
    ("bg_white".into(), Value::NativeFunction("color_bg_white".into())),
    ("bright_black".into(), Value::NativeFunction("color_fg_bright_black".into())),
    ("bright_red".into(), Value::NativeFunction("color_fg_bright_red".into())),
    ("bright_green".into(), Value::NativeFunction("color_fg_bright_green".into())),
    ("bright_yellow".into(), Value::NativeFunction("color_fg_bright_yellow".into())),
    ("bright_blue".into(), Value::NativeFunction("color_fg_bright_blue".into())),
    ("bright_magenta".into(), Value::NativeFunction("color_fg_bright_magenta".into())),
    ("bright_cyan".into(), Value::NativeFunction("color_fg_bright_cyan".into())),
    ("bright_white".into(), Value::NativeFunction("color_fg_bright_white".into())),
])));
vm.vars.insert("color".into(), color);
}
