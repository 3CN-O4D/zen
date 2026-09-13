//! Zen `qr` module — pure-Rust QR code generation.
//!
//! ```python
//! import qr
//! print(qr.render("https://example.com", 2))
//! m = qr.matrix("hello")          # [[0,1,1,..], ...]
//! ```

use crate::runtime::{Value, Vm};
use qrcode::{Color, EcLevel, QrCode, Version};
use std::sync::Arc;

fn encode(data: &str, min_version: Option<u8>) -> Result<QrCode, String> {
    let ec = EcLevel::M;
    let code = match min_version {
        Some(v) => QrCode::with_version(data.as_bytes(), Version::Normal(v as i16), ec),
        None => QrCode::with_error_correction_level(data.as_bytes(), ec),
    }
    .map_err(|e| format!("qr: {e}"))?;
    Ok(code)
}

/// True when the module at (x, y) is dark.
fn is_dark(code: &QrCode, x: usize, y: usize) -> bool {
    let colors = code.to_colors();
    colors[y * code.width() + x] == Color::Dark
}

fn string_arg(args: &[Value], idx: usize, what: &str) -> Result<String, String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(format!("qr.{what}: expected a string argument")),
    }
}

fn int_arg(args: &[Value], idx: usize, default: i64, what: &str) -> Result<i64, String> {
    match args.get(idx) {
        Some(Value::Number(n)) => Ok(*n as i64),
        Some(_) => Err(format!("qr.{what}: expected a number argument")),
        None => Ok(default),
    }
}

/// Binary matrix (list[list[int]]) of the QR modules, most significant order.
fn matrix_value(code: &QrCode) -> Value {
    let side = code.width();
    let mut rows = Vec::with_capacity(side);
    for y in 0..side {
        let mut row = Vec::with_capacity(side);
        for x in 0..side {
            row.push(Value::Number(if is_dark(code, x, y) { 1.0 } else { 0.0 }));
        }
        rows.push(Value::List(Arc::new(row)));
    }
    Value::List(Arc::new(rows))
}

/// Render the QR as a UTF-8 half-block ASCII-art string suitable for
/// terminal scanning. `border` is the quiet-zone width in modules (default 2).
fn render_value(code: &QrCode, border: usize, bright: bool) -> String {
    let total = code.width() + 2 * border.max(1);
    // Row height is 2 modules per output line (upper/lower half block).
    let mut out = String::new();
    for y in (0..total).step_by(2) {
        for x in 0..total {
            let upper = module_at(code, x, y, border.max(1));
            let lower = module_at(code, x, y + 1, border.max(1));
            let ch = match (upper, lower) {
                (false, false) => if bright { "░" } else { " " },
                (true, false) => "▀",
                (false, true) => "▄",
                (true, true) => "█",
            };
            out.push_str(ch);
        }
        out.push('\n');
    }
    out
}

fn module_at(code: &QrCode, x: usize, y: usize, border: usize) -> bool {
    let side = code.width();
    if x < border || y < border || x >= side + border || y >= side + border {
        return false;
    }
    is_dark(code, x - border, y - border)
}

pub fn qr_matrix(args: &[Value]) -> Result<Value, String> {
    let data = string_arg(args, 0, "matrix")?;
    let min = int_arg(args, 1, 0, "matrix")?;
    let code = encode(&data, (min > 0).then_some(min.min(40) as u8))?;
    Ok(matrix_value(&code))
}

pub fn qr_render(args: &[Value]) -> Result<Value, String> {
    let data = string_arg(args, 0, "render")?;
    let border = int_arg(args, 1, 2, "render")?;
    let bright = matches!(args.get(2), Some(Value::Bool(true)));
    let code = encode(&data, None)?;
    Ok(Value::String(render_value(&code, border as usize, bright)))
}

pub fn qr_version(args: &[Value]) -> Result<Value, String> {
    let data = string_arg(args, 0, "version")?;
    let code = encode(&data, None)?;
    Ok(Value::Number(match code.version() {
        Version::Normal(v) => v as f64,
        Version::Micro(v) => v as f64,
    }))
}

pub fn qr_dimension(args: &[Value]) -> Result<Value, String> {
    let data = string_arg(args, 0, "dimension")?;
    let code = encode(&data, None)?;
    Ok(Value::Number(code.width() as f64))
}

pub fn init_qr_module(vm: &mut Vm) {
    let qr = Value::Dict(Arc::new(indexmap::IndexMap::from([
        ("matrix".into(), Value::NativeFunction("qr_matrix".into())),
        ("render".into(), Value::NativeFunction("qr_render".into())),
        ("version".into(), Value::NativeFunction("qr_version".into())),
        ("dimension".into(), Value::NativeFunction("qr_dimension".into())),
    ])));
    vm.vars.insert("qr".into(), qr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_is_square_and_has_finder_patterns() {
        let code = encode("zen", None).unwrap();
        let side = code.width();
        assert!(side >= 21 && side % 4 == 1);
        // Top-left finder: 7x7 dark ring with 3x3 dark center.
        for y in 0..7 {
            for x in 0..7 {
                let on_ring = x == 0 || x == 6 || y == 0 || y == 6;
                let on_center = (2..=4).contains(&x) && (2..=4).contains(&y);
                assert_eq!(is_dark(&code, x, y), on_ring || on_center);
            }
        }
    }

    #[test]
    fn render_has_quiet_zone_and_lines() {
        let code = encode("hello world", None).unwrap();
        let s = render_value(&code, 2, false);
        let first_line: String = s.lines().next().unwrap_or("").to_string();
        let expected_width = code.width() + 4;
        assert_eq!(first_line.len(), expected_width);
        assert_eq!(first_line, " ".repeat(expected_width), "quiet zone must be blank");
    }

    #[test]
    fn deterministic_across_calls() {
        let args: &[Value] = &[Value::String("abc".into())];
        assert_eq!(qr_matrix(args).unwrap(), qr_matrix(args).unwrap());
    }
}