//! Native `qr` module: deterministic QR-code generation via the `qrcode`
//! crate. Exposes the module grid as `0`/`1` integers, its dimension and
//! version, and a terminal rendering using UTF-8 half-block glyphs.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::new_list;
use crate::native::host_module_arity;
use crate::value::Value;
use qrcode::QrCode;

const ID_MATRIX: u16 = 310;
const ID_DIMENSION: u16 = 311;
const ID_VERSION: u16 = 312;
const ID_RENDER: u16 = 313;

pub fn module() -> ModuleUnit {
    host_module_arity(&[
        ("matrix", ID_MATRIX, 1, 1),
        ("dimension", ID_DIMENSION, 1, 1),
        ("version", ID_VERSION, 1, 1),
        ("render", ID_RENDER, 1, 1),
    ])
}

fn arg_str(args: &[Value]) -> VmResult<String> {
    let v = args.first().copied().ok_or(VmError::ArgumentCount {
        expected: 1,
        got: 0,
    })?;
    Ok(v.as_str().map(str::to_string).unwrap_or_else(|| {
        crate::ops::stringify(v)
    }))
}

fn make(data: &str) -> VmResult<QrCode> {
    QrCode::new(data.as_bytes()).map_err(|e| VmError::NativeError(format!("qr: {e}")))
}

/// `(dark, width, row-major bools)` for the module grid.
fn grid(data: &str) -> VmResult<(Vec<bool>, usize)> {
    let code = make(data)?;
    let width = code.width();
    let colors = code.to_colors();
    let dark = colors
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    Ok((dark, width))
}

fn id_matrix(args: &[Value]) -> VmResult<Value> {
    let data = arg_str(args)?;
    let (dark, width) = grid(&data)?;
    let mut rows = Vec::with_capacity(width);
    for r in 0..width {
        let mut row = Vec::with_capacity(width);
        for c in 0..width {
            row.push(Value::int(i64::from(dark[r * width + c])));
        }
        rows.push(new_list(row));
    }
    Ok(new_list(rows))
}

fn id_render(args: &[Value]) -> VmResult<Value> {
    let data = arg_str(args)?;
    let (dark, width) = grid(&data)?;
    let quiet = 4usize;
    let total = width + quiet * 2;
    let at = |r: i64, c: i64| -> bool {
        if r < 0 || c < 0 || r >= width as i64 || c >= width as i64 {
            return false;
        }
        dark[r as usize * width + c as usize]
    };
    let mut out = String::new();
    let mut r = 0usize;
    while r < total {
        for c in 0..total {
            let gc = c as i64 - quiet as i64;
            let gr = r as i64 - quiet as i64;
            let upper = at(gr, gc);
            let lower = if r + 1 < total {
                at(gr + 1, gc)
            } else {
                false
            };
            out.push(match (upper, lower) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push('\n');
        r += 2;
    }
    Ok(Value::string(out))
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        ID_MATRIX => id_matrix(args)?,
        ID_DIMENSION => {
            let data = arg_str(args)?;
            let (_, width) = grid(&data)?;
            Value::int(width as i64)
        }
        ID_VERSION => {
            let data = arg_str(args)?;
            let (_, width) = grid(&data)?;
            Value::int(((width as i64 - 17) / 4).max(1))
        }
        ID_RENDER => id_render(args)?,
        _ => return Ok(None),
    };
    Ok(Some(v))
}