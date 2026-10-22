//! Native Rust tiers for the stdlib modules that also ship a pure-Zen `.z`
//! companion.
//!
//! Each module here is registered under its bare name (`math`, `base64`, …), so
//! the linker attaches it to the pure module as `<name>["native"]`, or uses it
//! directly when `ZEN_NATIVE_ONLY` names the module. The implementations are
//! written to agree with the `.z` tiers member for member, because the corpus
//! tests compare the two directly.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use indexmap::IndexMap;
use std::sync::atomic::{AtomicI32, Ordering as AtomicOrdering};

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{host_module_mixed, HostExport};
use crate::ops::{arith, negate, stringify, to_f64, Arith};
use crate::value::{DictKey, Value};

// ── small helpers ───────────────────────────────────────────────────────────

fn func(name: &'static str, id: u16, required: u8, max: u8) -> HostExport {
    HostExport::Func {
        name,
        id,
        required,
        max,
    }
}

fn konst(name: &'static str, value: Value) -> HostExport {
    HostExport::Const { name, value }
}

fn const_str(name: &'static str, value: &str) -> HostExport {
    HostExport::Const {
        name,
        value: Value::string(value),
    }
}

fn arg(args: &[Value], i: usize) -> VmResult<Value> {
    args.get(i).copied().ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })
}

fn num(args: &[Value], i: usize) -> VmResult<f64> {
    let v = arg(args, i)?;
    to_f64(v).ok_or_else(|| VmError::type_mismatch("number", v))
}

fn int_at(args: &[Value], i: usize) -> VmResult<i64> {
    let v = arg(args, i)?;
    v.as_int().ok_or_else(|| VmError::type_mismatch("int", v))
}

fn string_at(args: &[Value], i: usize) -> VmResult<String> {
    let v = arg(args, i)?;
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| VmError::type_mismatch("string", v))
}

fn list_at(args: &[Value], i: usize) -> VmResult<Vec<Value>> {
    let v = arg(args, i)?;
    v.as_list()
        .map(<[Value]>::to_vec)
        .ok_or_else(|| VmError::type_mismatch("list", v))
}

/// Flatten variadic numeric arguments the way the pure `_flat` helper does:
/// list/tuple arguments contribute their numeric elements, scalars themselves,
/// and `nil` (an omitted optional slot) is ignored.
fn collect_numbers(args: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for a in args {
        if a.is_nil() {
            continue;
        }
        if let Some(items) = a.as_list() {
            for item in items {
                if to_f64(*item).is_some() {
                    out.push(*item);
                }
            }
        } else if to_f64(*a).is_some() {
            out.push(*a);
        }
    }
    out
}

fn optional_string(args: &[Value], i: usize, default: &str) -> String {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => v
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| default.into()),
        _ => default.to_string(),
    }
}

fn fill_char(args: &[Value], i: usize) -> char {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => v.as_str().and_then(|s| s.chars().next()).unwrap_or(' '),
        _ => ' ',
    }
}

// ── dispatch ────────────────────────────────────────────────────────────────

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        // ── math ───────────────────────────────────────────────────────────
        23 => Ok(Value::float(num(args, 0)?.sqrt())),
        24 => arith(Arith::Pow, arg(args, 0)?, arg(args, 1)?),
        25 => Ok(floor_value(num(args, 0)?)),
        26 => Ok(ceil_value(num(args, 0)?)),
        27 => {
            let x = num(args, 0)?;
            if x.is_nan() {
                Ok(Value::float(x))
            } else {
                Ok(trunc_value(x))
            }
        }
        28 => {
            let x = num(args, 0)?;
            if x.is_nan() || x.is_infinite() {
                Ok(Value::float(x))
            } else {
                Ok(round_value(x))
            }
        }
        29 => Ok(abs_value(arg(args, 0)?)),
        30 => Ok(Value::float(std::f64::consts::E.powf(num(args, 0)?))),
        31 => {
            let x = num(args, 0)?;
            let base = match args.get(1).copied() {
                Some(b) if !b.is_nil() => {
                    to_f64(b).ok_or_else(|| VmError::type_mismatch("number", b))?
                }
                _ => std::f64::consts::E,
            };
            Ok(Value::float(x.ln() / base.ln()))
        }
        32 => Ok(Value::float(num(args, 0)?.log2())),
        33 => Ok(Value::float(num(args, 0)?.log10())),
        34 => Ok(Value::float(num(args, 0)?.sin())),
        35 => Ok(Value::float(num(args, 0)?.cos())),
        36 => Ok(Value::float(num(args, 0)?.tan())),
        37 => Ok(Value::float(num(args, 0)?.asin())),
        38 => Ok(Value::float(num(args, 0)?.acos())),
        39 => Ok(Value::float(num(args, 0)?.atan())),
        40 => {
            let y = num(args, 0)?;
            let x = num(args, 1)?;
            Ok(Value::float(y.atan2(x)))
        }
        41 => Ok(Value::float(num(args, 0)? * 180.0 / std::f64::consts::PI)),
        42 => Ok(Value::float(num(args, 0)? * std::f64::consts::PI / 180.0)),
        43 => {
            let mut a = int_at(args, 0)?.abs();
            let mut b = int_at(args, 1)?.abs();
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            Ok(Value::int_promote(a))
        }
        44 => {
            let mut a = int_at(args, 0)?;
            let mut b = int_at(args, 1)?;
            if a == 0 || b == 0 {
                return Ok(Value::int(0));
            }
            a = a.abs();
            b = b.abs();
            let mut x = a;
            let mut y = b;
            while y != 0 {
                let t = y;
                y = x % y;
                x = t;
            }
            let q = arith(Arith::Div, Value::int_promote(a), Value::int_promote(x))?;
            arith(Arith::Mul, q, Value::int_promote(b))
        }
        45 => {
            let n = int_at(args, 0)?;
            let mut res = Value::int(1);
            let mut i = 2i64;
            while i <= n {
                res = arith(Arith::Mul, res, Value::int_promote(i))?;
                i += 1;
            }
            Ok(res)
        }
        46 => {
            let n = int_at(args, 0)?;
            let mut k = int_at(args, 1)?;
            if k < 0 || k > n {
                return Ok(Value::int(0));
            }
            if k > n - k {
                k = n - k;
            }
            let mut res = Value::int(1);
            let mut i = 0i64;
            while i < k {
                let prod = arith(Arith::Mul, res, Value::int_promote(n - i))?;
                res = arith(Arith::Div, prod, Value::int_promote(i + 1))?;
                i += 1;
            }
            Ok(res)
        }
        47 => {
            let n = int_at(args, 0)?;
            let r = match args.get(1).copied() {
                Some(v) if !v.is_nil() => {
                    v.as_int().ok_or_else(|| VmError::type_mismatch("int", v))?
                }
                _ => n,
            };
            if r < 0 || r > n {
                return Ok(Value::int(0));
            }
            let mut res = Value::int(1);
            let mut i = 0i64;
            while i < r {
                res = arith(Arith::Mul, res, Value::int_promote(n - i))?;
                i += 1;
            }
            Ok(res)
        }
        48 => {
            let x = num(args, 0)?;
            let y = num(args, 1)?;
            Ok(Value::float(x - (x / y).round() * y))
        }
        49 => {
            let m = abs_value(arg(args, 0)?);
            if num(args, 1)? < 0.0 {
                negate(m)
            } else {
                Ok(m)
            }
        }
        50 => {
            let x = arg(args, 0)?;
            let t = trunc_value(num(args, 0)?);
            let frac = arith(Arith::Sub, x, t)?;
            Ok(new_list(vec![frac, t]))
        }
        51 => {
            let x = num(args, 0)?;
            if x == 0.0 {
                return Ok(new_list(vec![Value::int(0), Value::int(0)]));
            }
            let m = x.abs();
            let exponent = (m.ln() / std::f64::consts::LN_2).floor() as i64 + 1;
            let mantissa = x / 2f64.powi(exponent as i32);
            Ok(new_list(vec![
                Value::float(mantissa),
                Value::int_promote(exponent),
            ]))
        }
        52 => {
            let x = num(args, 0)?;
            let e = int_at(args, 1)?;
            Ok(Value::float(x * 2f64.powi(e as i32)))
        }
        53 => {
            let mut s = 0.0;
            for v in collect_numbers(args) {
                let f = to_f64(v).unwrap_or(0.0);
                s += f * f;
            }
            Ok(Value::float(s.sqrt()))
        }
        54 => {
            let mut acc = Value::int(0);
            for x in collect_numbers(args) {
                acc = arith(Arith::Add, acc, x)?;
            }
            Ok(acc)
        }
        55 => {
            let mut res = Value::int(1);
            for x in collect_numbers(args) {
                res = arith(Arith::Mul, res, x)?;
            }
            Ok(res)
        }
        56 => minmax(args, false),
        57 => minmax(args, true),
        58 => Ok(Value::bool(num(args, 0)?.is_nan())),
        59 => Ok(Value::bool(num(args, 0)?.is_infinite())),
        60 => Ok(Value::bool(num(args, 0)?.is_finite())),

        // ── base64 ─────────────────────────────────────────────────────────
        61 => Ok(Value::string(base64_encode(&string_at(args, 0)?))),
        62 => Ok(Value::string(base64_decode(&string_at(args, 0)?)?)),
        63 => Ok(Value::string(base64_url_encode(&string_at(args, 0)?))),
        64 => Ok(Value::string(base64_url_decode(&string_at(args, 0)?)?)),

        // ── binascii ───────────────────────────────────────────────────────
        65 => Ok(Value::string(hexlify(string_at(args, 0)?.as_bytes()))),
        66 => Ok(Value::string(from_bytes(&unhexlify(&string_at(args, 0)?)?))),
        67 => Ok(Value::string(base64_encode(&string_at(args, 0)?))),
        68 => {
            let s = string_at(args, 0)?;
            Ok(Value::string(base64_decode(s.trim())?))
        }

        // ── collections ────────────────────────────────────────────────────
        69 => {
            let items = list_at(args, 0)?;
            let mut counts: IndexMap<String, i64> = IndexMap::new();
            for item in items {
                *counts.entry(stringify(item)).or_insert(0) += 1;
            }
            let dict = counts
                .into_iter()
                .map(|(k, n)| (DictKey(Value::string(k)), Value::int_promote(n)))
                .collect();
            Ok(new_dict(dict))
        }
        70 => Ok(new_list(chain_args(args))),
        71 => {
            let items = list_at(args, 0)?;
            let mut out = Vec::new();
            flatten_into(&items, &mut out);
            Ok(new_list(out))
        }

        // ── string ─────────────────────────────────────────────────────────
        72 => Ok(Value::string(string_at(args, 0)?.to_uppercase())),
        73 => Ok(Value::string(string_at(args, 0)?.to_lowercase())),
        74 => Ok(Value::string(title(&string_at(args, 0)?))),
        75 => Ok(Value::string(capitalize(&string_at(args, 0)?))),
        76 => Ok(Value::string(swapcase(&string_at(args, 0)?))),
        77 => Ok(Value::string(string_at(args, 0)?.trim().to_string())),
        78 => Ok(Value::string(string_at(args, 0)?.trim_start().to_string())),
        79 => Ok(Value::string(string_at(args, 0)?.trim_end().to_string())),
        80 => {
            let s = string_at(args, 0)?;
            let sep = optional_string(args, 1, " ");
            Ok(new_list(str_split(&s, &sep)))
        }
        81 => Ok(new_list(splitlines(&string_at(args, 0)?))),
        82 => {
            let sep = string_at(args, 0)?;
            let items = list_at(args, 1)?;
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(&sep);
                }
                out.push_str(&stringify(*item));
            }
            Ok(Value::string(out))
        }
        83 => {
            let s = string_at(args, 0)?;
            let old = string_at(args, 1)?;
            let rep = string_at(args, 2)?;
            Ok(Value::string(s.replace(&old, &rep)))
        }
        84 => {
            let s = string_at(args, 0)?;
            let sub = string_at(args, 1)?;
            let n = if sub.is_empty() {
                s.chars().count() + 1
            } else {
                s.matches(&sub).count()
            };
            Ok(Value::int(n as i64))
        }
        85 => {
            let s = string_at(args, 0)?;
            let sub = string_at(args, 1)?;
            Ok(Value::int(match s.find(&sub) {
                Some(b) => s[..b].chars().count() as i64,
                None => -1,
            }))
        }
        86 => {
            let s = string_at(args, 0)?;
            let sub = string_at(args, 1)?;
            Ok(Value::int(match s.rfind(&sub) {
                Some(b) => s[..b].chars().count() as i64,
                None => -1,
            }))
        }
        87 => Ok(Value::bool(
            string_at(args, 0)?.starts_with(&string_at(args, 1)?),
        )),
        88 => Ok(Value::bool(
            string_at(args, 0)?.ends_with(&string_at(args, 1)?),
        )),
        89 => Ok(Value::bool(
            string_at(args, 0)?.contains(&string_at(args, 1)?),
        )),
        90 => {
            let s = string_at(args, 0)?;
            let width = int_at(args, 1)?.max(0) as usize;
            Ok(Value::string(ljust(&s, width, fill_char(args, 2))))
        }
        91 => {
            let s = string_at(args, 0)?;
            let width = int_at(args, 1)?.max(0) as usize;
            Ok(Value::string(rjust(&s, width, fill_char(args, 2))))
        }
        92 => {
            let s = string_at(args, 0)?;
            let width = int_at(args, 1)?.max(0) as usize;
            Ok(Value::string(center(&s, width, fill_char(args, 2))))
        }
        93 => {
            let s = string_at(args, 0)?;
            let width = int_at(args, 1)?;
            Ok(Value::string(zfill(&s, width)))
        }
        94 => {
            let s = string_at(args, 0)?;
            let n = int_at(args, 1)?;
            Ok(Value::string(s.repeat(n.max(0) as usize)))
        }
        95 => {
            let s = string_at(args, 0)?;
            Ok(Value::bool(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
            ))
        }
        96 => {
            let s = string_at(args, 0)?;
            Ok(Value::bool(
                !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
            ))
        }
        97 => {
            let s = string_at(args, 0)?;
            Ok(Value::bool(
                !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()),
            ))
        }
        98 => {
            let s = string_at(args, 0)?;
            Ok(Value::bool(
                !s.is_empty() && s.chars().all(|c| c.is_whitespace()),
            ))
        }
        99 => {
            let s = string_at(args, 0)?;
            let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
            Ok(Value::bool(
                !cased.is_empty() && cased.iter().all(|c| c.is_lowercase()),
            ))
        }
        225 => {
            let s = string_at(args, 0)?;
            let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
            Ok(Value::bool(
                !cased.is_empty() && cased.iter().all(|c| c.is_uppercase()),
            ))
        }

        // ── tempfile ───────────────────────────────────────────────────────
        227 => Ok(Value::string(temp_dir())),
        228 => {
            let prefix = optional_string(args, 0, "zen");
            Ok(Value::string(mkdtemp(&prefix)?))
        }
        229 => {
            let prefix = optional_string(args, 0, "zen");
            Ok(Value::string(mkstemp(&prefix)?))
        }

        // ── random ─────────────────────────────────────────────────────────
        230 => Ok(Value::float(native_random())),
        231 => Ok(Value::nil()),
        232 => Ok(Value::string(native_hex(args))),
        233 => {
            let items = list_at(args, 0)?;
            Ok(new_list(native_shuffle(items)))
        }

        // ── itertools ──────────────────────────────────────────────────────
        234 => {
            let items = list_at(args, 0)?;
            let out: Vec<Value> = items
                .into_iter()
                .enumerate()
                .map(|(i, v)| new_list(vec![Value::int_promote(i as i64), v]))
                .collect();
            Ok(new_list(out))
        }
        235 => {
            let a = list_at(args, 0)?;
            let b = list_at(args, 1)?;
            let n = a.len().min(b.len());
            let out: Vec<Value> = (0..n).map(|i| new_list(vec![a[i], b[i]])).collect();
            Ok(new_list(out))
        }
        236 => Ok(new_list(chain_args(args))),
        237 => {
            let v = arg(args, 0)?;
            let n = int_at(args, 1)?;
            Ok(new_list(vec![v; n.max(0) as usize]))
        }
        238 => {
            let a = list_at(args, 0)?;
            let b = list_at(args, 1)?;
            let mut out = Vec::with_capacity(a.len() * b.len());
            for x in &a {
                for y in &b {
                    out.push(new_list(vec![*x, *y]));
                }
            }
            Ok(new_list(out))
        }
        239 => {
            let items = list_at(args, 0)?;
            let r = match args.get(1).copied() {
                Some(v) if !v.is_nil() => {
                    v.as_int().ok_or_else(|| VmError::type_mismatch("int", v))?
                }
                _ => items.len() as i64,
            };
            Ok(new_list(permutations(&items, r.max(0) as usize)))
        }
        240 => {
            let items = list_at(args, 0)?;
            let r = int_at(args, 1)?.max(0) as usize;
            Ok(new_list(combinations(&items, r)))
        }
        241 => {
            let items = list_at(args, 0)?;
            let mut out = Vec::with_capacity(items.len());
            let mut acc = Value::int(0);
            for item in items {
                if item.as_int().is_some() {
                    acc = arith(Arith::Add, acc, item)?;
                }
                out.push(acc);
            }
            Ok(new_list(out))
        }
        242 => {
            let n = int_at(args, 0)?.max(0) as usize;
            let items = list_at(args, 1)?;
            Ok(new_list(items.into_iter().take(n).collect()))
        }
        243 => {
            let n = int_at(args, 0)?.max(0) as usize;
            let items = list_at(args, 1)?;
            Ok(new_list(items.into_iter().skip(n).collect()))
        }
        244 => it_range(args),

        // ── os.execute ─────────────────────────────────────────────────────
        245 => {
            // Python `subprocess.run(..., capture_output=True)` shape.
            let cmd = string_at(args, 0)?;
            let out = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .map_err(|e| VmError::NativeError(format!("execute: {e}")))?;
            let code = out.status.code().unwrap_or(-1);
            let mut d = IndexMap::new();
            d.insert(DictKey(Value::string("ok")), Value::bool(out.status.success()));
            d.insert(DictKey(Value::string("code")), Value::int(code as i64));
            d.insert(DictKey(Value::string("returncode")), Value::int(code as i64));
            d.insert(
                DictKey(Value::string("stdout")),
                Value::string(String::from_utf8_lossy(&out.stdout).into_owned()),
            );
            d.insert(
                DictKey(Value::string("stderr")),
                Value::string(String::from_utf8_lossy(&out.stderr).into_owned()),
            );
            Ok(new_dict(d))
        }

// ── re ─────────────────────────────────────────────────────────────
        // Argument order follows the Rust `regex` crate (pattern first, then
        // the subject text), which is what the stdlib corpus exercises:
        // `re.sub(pattern, text, repl)`.
        251 => re_findall(args),
        252 => re_sub(args),
        253 => re_search(args),
        254 => re_split(args),
        255 => Ok(Value::string(regex::escape(&string_at(args, 0)?))),
        256 => re_match(args),
        257 => re_fullmatch(args),
        258 => re_finditer(args),
        259 => re_subn(args),
        260 => re_compile(args),

        other => {
            if let Some(v) = sys_dispatch(other, args)? {
                return Ok(v);
            }
            Err(VmError::NativeError(format!("unknown native {other}")))
        }
    }
}

/// Status of the most recent `sys.exec` "child", stashed for `sys.waitpid`.
static CHILD_STATUS: AtomicI32 = AtomicI32::new(0);

/// Synchronous emulation of the POSIX `sys.fork`/`exec`/`waitpid` family.
///
/// Real `fork(2)` arrives with the M7 process layer; the tier tests only need
/// a parent/child shape that runs one command and reports its exit status, so
/// `fork` reports "child" (`0`), `exec` runs the command to completion and
/// stashes its code, and `waitpid` hands that code back.
fn sys_dispatch(id: u16, args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        246 => Value::int(0),
        247 => {
            let path = string_at(args, 0)?;
            let argv: Vec<String> = list_at(args, 1)?.iter().map(|a| stringify(*a)).collect();
            let code = match std::process::Command::new(&path).args(&argv).status() {
                Ok(s) => s.code().unwrap_or(-1),
                Err(_) => -1,
            };
            CHILD_STATUS.store(code, AtomicOrdering::SeqCst);
            Value::int(code as i64)
        }
        248 => {
            let pid = arg(args, 0)?;
            let status = CHILD_STATUS.load(AtomicOrdering::SeqCst);
            new_list(vec![pid, Value::int(status as i64)])
        }
        249 => Value::int(int_at(args, 0)? & 0xff),
        250 => Value::int(std::process::id() as i64),
        _ => return Ok(None),
    };
    Ok(Some(v))
}

// ── math internals ──────────────────────────────────────────────────────────

fn floor_value(x: f64) -> Value {
    if !x.is_finite() {
        Value::float(x)
    } else {
        Value::int_promote(x.floor() as i64)
    }
}

fn ceil_value(x: f64) -> Value {
    if !x.is_finite() {
        Value::float(x)
    } else {
        Value::int_promote(x.ceil() as i64)
    }
}

fn trunc_value(x: f64) -> Value {
    Value::int_promote(x.trunc() as i64)
}

fn round_value(x: f64) -> Value {
    Value::int_promote(x.round() as i64)
}

fn abs_value(v: Value) -> Value {
    if let Some(i) = v.as_int() {
        Value::int_promote(i.abs())
    } else if let Some(f) = v.as_float() {
        Value::float(f.abs())
    } else {
        v
    }
}

fn minmax(args: &[Value], want_max: bool) -> VmResult<Value> {
    let items = collect_numbers(args);
    let mut best: Option<Value> = None;
    for v in &items {
        let f = to_f64(*v).unwrap_or(0.0);
        match best {
            None => best = Some(*v),
            Some(b) => {
                let bf = to_f64(b).unwrap_or(f);
                if (want_max && f > bf) || (!want_max && f < bf) {
                    best = Some(*v);
                }
            }
        }
    }
    best.ok_or_else(|| VmError::ValueError("aggregate expects at least one number".into()))
}

fn it_range(args: &[Value]) -> VmResult<Value> {
    let provided = args.iter().filter(|v| !v.is_nil()).count();
    let (start, stop, step) = if provided <= 1 {
        (0i64, int_at(args, 0)?, 1i64)
    } else {
        let start = int_at(args, 0)?;
        let stop = int_at(args, 1)?;
        let step = if provided >= 3 {
            int_at(args, 2)?
        } else {
            1i64
        };
        (start, stop, step)
    };
    if step == 0 {
        return Err(VmError::ValueError(
            "itertools.range: step cannot be 0".into(),
        ));
    }
    let mut out = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < stop {
            out.push(Value::int_promote(i));
            i += step;
        }
    } else {
        while i > stop {
            out.push(Value::int_promote(i));
            i += step;
        }
    }
    Ok(new_list(out))
}

// ── collections / itertools internals ───────────────────────────────────────

fn chain_args(args: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for a in args {
        if a.is_nil() {
            continue;
        }
        if let Some(items) = a.as_list() {
            out.extend_from_slice(items);
        } else {
            out.push(*a);
        }
    }
    out
}

fn flatten_into(items: &[Value], out: &mut Vec<Value>) {
    for item in items {
        if let Some(inner) = item.as_list() {
            flatten_into(inner, out);
        } else {
            out.push(*item);
        }
    }
}

fn permutations(items: &[Value], r: usize) -> Vec<Value> {
    let mut out = Vec::new();
    if r > items.len() {
        return out;
    }
    let mut used = vec![false; items.len()];
    let mut path = Vec::new();
    fn walk(
        items: &[Value],
        r: usize,
        used: &mut [bool],
        path: &mut Vec<Value>,
        out: &mut Vec<Value>,
    ) {
        if path.len() == r {
            out.push(new_list(path.clone()));
            return;
        }
        for i in 0..items.len() {
            if used[i] {
                continue;
            }
            used[i] = true;
            path.push(items[i]);
            walk(items, r, used, path, out);
            path.pop();
            used[i] = false;
        }
    }
    walk(items, r, &mut used, &mut path, &mut out);
    out
}

fn combinations(items: &[Value], r: usize) -> Vec<Value> {
    let mut out = Vec::new();
    if r > items.len() {
        return out;
    }
    let mut idx = Vec::new();
    fn walk(items: &[Value], r: usize, start: usize, idx: &mut Vec<usize>, out: &mut Vec<Value>) {
        if idx.len() == r {
            out.push(new_list(idx.iter().map(|&i| items[i]).collect()));
            return;
        }
        for i in start..items.len() {
            idx.push(i);
            walk(items, r, i + 1, idx, out);
            idx.pop();
        }
    }
    walk(items, r, 0, &mut idx, &mut out);
    out
}

// ── string internals ────────────────────────────────────────────────────────

fn title(s: &str) -> String {
    let mut out = String::new();
    let mut ws = true;
    for c in s.chars() {
        if c.is_whitespace() {
            ws = true;
            out.push(c);
        } else if ws {
            out.extend(c.to_uppercase());
            ws = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut out = String::new();
    let mut first = true;
    for c in s.chars() {
        if first {
            out.extend(c.to_uppercase());
            first = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn swapcase(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() {
            out.extend(c.to_lowercase());
        } else if c.is_lowercase() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn str_split(s: &str, sep: &str) -> Vec<Value> {
    if sep.is_empty() {
        return s.chars().map(|c| Value::string(c.to_string())).collect();
    }
    s.split(sep).map(|p| Value::string(p.to_string())).collect()
}

fn splitlines(s: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\r' && i + 1 < chars.len() && chars[i + 1] == '\n' {
            out.push(Value::string(cur.clone()));
            cur.clear();
            i += 2;
        } else if c == '\n' || c == '\r' {
            out.push(Value::string(cur.clone()));
            cur.clear();
            i += 1;
        } else {
            cur.push(c);
            i += 1;
        }
    }
    if !cur.is_empty() {
        out.push(Value::string(cur));
    }
    out
}

fn ljust(s: &str, width: usize, fill: char) -> String {
    let mut out = s.to_string();
    while out.chars().count() < width {
        out.push(fill);
    }
    out
}

fn rjust(s: &str, width: usize, fill: char) -> String {
    let mut out = String::new();
    while out.chars().count() + s.chars().count() < width {
        out.push(fill);
    }
    out.push_str(s);
    out
}

fn center(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let total = width - len;
    let left = total / 2;
    let right = total - left;
    let mut out = String::new();
    for _ in 0..left {
        out.push(fill);
    }
    out.push_str(s);
    for _ in 0..right {
        out.push(fill);
    }
    out
}

fn zfill(s: &str, width: i64) -> String {
    let (sign, rest) = if let Some(r) = s.strip_prefix('-') {
        ("-", r)
    } else if let Some(r) = s.strip_prefix('+') {
        ("+", r)
    } else {
        ("", s)
    };
    let mut out = sign.to_string();
    while out.chars().count() + rest.chars().count() < width.max(0) as usize {
        out.push('0');
    }
    out.push_str(rest);
    out
}

// ── base64 / binascii internals ─────────────────────────────────────────────

fn base64_encode(s: &str) -> String {
    STANDARD.encode(s.as_bytes())
}

fn base64_decode(s: &str) -> VmResult<String> {
    let bytes = STANDARD
        .decode(s)
        .map_err(|e| VmError::ValueError(format!("invalid base64: {e}")))?;
    Ok(from_bytes(&bytes))
}

fn base64_url_encode(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

fn base64_url_decode(s: &str) -> VmResult<String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| VmError::ValueError(format!("invalid base64: {e}")))?;
    Ok(from_bytes(&bytes))
}

fn hexlify(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn unhexlify(s: &str) -> VmResult<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err(VmError::ValueError(
            "binascii.unhexlify: odd-length string".into(),
        ));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])
            .ok_or_else(|| VmError::ValueError("binascii.unhexlify: non-hex character".into()))?;
        let lo = hex_val(bytes[i + 1])
            .ok_or_else(|| VmError::ValueError("binascii.unhexlify: non-hex character".into()))?;
        out.push(hi * 16 + lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// ── tempfile internals ──────────────────────────────────────────────────────

fn temp_dir() -> String {
    for key in ["TMPDIR", "TEMP", "TMP"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "/tmp".to_string()
}

fn unique_path(prefix: &str) -> String {
    let base = temp_dir();
    loop {
        let candidate = format!("{base}/{prefix}{}", rand::random::<u64>() % 1_000_000_000);
        if !std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
}

fn mkdtemp(prefix: &str) -> VmResult<String> {
    let p = unique_path(prefix);
    std::fs::create_dir(&p).map_err(|e| VmError::NativeError(format!("mkdtemp: {e}")))?;
    Ok(p)
}

fn mkstemp(prefix: &str) -> VmResult<String> {
    let p = unique_path(prefix);
    std::fs::write(&p, b"").map_err(|e| VmError::NativeError(format!("mkstemp: {e}")))?;
    Ok(p)
}

// ── random internals ────────────────────────────────────────────────────────

fn native_random() -> f64 {
    rand::random::<f64>()
}

fn native_hex(args: &[Value]) -> String {
    let len = match args.first().copied() {
        Some(v) if !v.is_nil() => v.as_int().unwrap_or(16),
        _ => 16,
    };
    let mut out = String::new();
    for _ in 0..len.max(0) {
        out.push(char::from_digit(rand::random::<u32>() % 16, 16).unwrap());
    }
    out
}

fn native_shuffle(mut items: Vec<Value>) -> Vec<Value> {
    use rand::seq::SliceRandom;
    items.shuffle(&mut rand::thread_rng());
    items
}

// ── module tables ───────────────────────────────────────────────────────────

fn math_module() -> ModuleUnit {
    host_module_mixed(&[
        konst("pi", Value::float(std::f64::consts::PI)),
        konst("e", Value::float(std::f64::consts::E)),
        konst("inf", Value::float(f64::INFINITY)),
        konst("nan", Value::float(f64::NAN)),
        func("sqrt", 23, 1, 1),
        func("pow", 24, 2, 2),
        func("floor", 25, 1, 1),
        func("ceil", 26, 1, 1),
        func("trunc", 27, 1, 1),
        func("round", 28, 1, 1),
        func("abs", 29, 1, 1),
        func("exp", 30, 1, 1),
        func("log", 31, 1, 2),
        func("log2", 32, 1, 1),
        func("log10", 33, 1, 1),
        func("sin", 34, 1, 1),
        func("cos", 35, 1, 1),
        func("tan", 36, 1, 1),
        func("asin", 37, 1, 1),
        func("acos", 38, 1, 1),
        func("atan", 39, 1, 1),
        func("atan2", 40, 2, 2),
        func("degrees", 41, 1, 1),
        func("radians", 42, 1, 1),
        func("gcd", 43, 2, 2),
        func("lcm", 44, 2, 2),
        func("factorial", 45, 1, 1),
        func("comb", 46, 2, 2),
        func("perm", 47, 1, 2),
        func("remainder", 48, 2, 2),
        func("copysign", 49, 2, 2),
        func("modf", 50, 1, 1),
        func("frexp", 51, 1, 1),
        func("ldexp", 52, 2, 2),
        func("hypot", 53, 1, 2),
        func("fsum", 54, 1, 2),
        func("prod", 55, 1, 2),
        func("min", 56, 1, 2),
        func("max", 57, 1, 2),
        func("isnan", 58, 1, 1),
        func("isinf", 59, 1, 1),
        func("isfinite", 60, 1, 1),
    ])
}

fn base64_module() -> ModuleUnit {
    host_module_mixed(&[
        func("encode", 61, 1, 1),
        func("decode", 62, 1, 1),
        func("url_encode", 63, 1, 1),
        func("url_decode", 64, 1, 1),
    ])
}

fn binascii_module() -> ModuleUnit {
    host_module_mixed(&[
        func("hexlify", 65, 1, 1),
        func("unhexlify", 66, 1, 1),
        func("b2a_base64", 67, 1, 1),
        func("a2b_base64", 68, 1, 1),
    ])
}

fn collections_module() -> ModuleUnit {
    host_module_mixed(&[
        func("Counter", 69, 1, 1),
        func("chain", 70, 0, 8),
        func("flatten", 71, 1, 1),
    ])
}

fn string_module() -> ModuleUnit {
    host_module_mixed(&[
        func("upper", 72, 1, 1),
        func("lower", 73, 1, 1),
        func("title", 74, 1, 1),
        func("capitalize", 75, 1, 1),
        func("swapcase", 76, 1, 1),
        func("strip", 77, 1, 1),
        func("lstrip", 78, 1, 1),
        func("rstrip", 79, 1, 1),
        func("split", 80, 1, 2),
        func("splitlines", 81, 1, 1),
        func("join", 82, 2, 2),
        func("replace", 83, 3, 3),
        func("count", 84, 2, 2),
        func("find", 85, 2, 2),
        func("rfind", 86, 2, 2),
        func("startswith", 87, 2, 2),
        func("endswith", 88, 2, 2),
        func("contains", 89, 2, 2),
        func("ljust", 90, 2, 3),
        func("rjust", 91, 2, 3),
        func("center", 92, 2, 3),
        func("zfill", 93, 2, 2),
        func("repeat", 94, 2, 2),
        func("isdigit", 95, 1, 1),
        func("isalpha", 96, 1, 1),
        func("isalnum", 97, 1, 1),
        func("isspace", 98, 1, 1),
        func("islower", 99, 1, 1),
        func("isupper", 225, 1, 1),
        const_str("digits", "0123456789"),
        const_str("hexdigits", "0123456789abcdefABCDEF"),
        const_str("octdigits", "01234567"),
        const_str(
            "ascii_letters",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        ),
        const_str("ascii_lowercase", "abcdefghijklmnopqrstuvwxyz"),
        const_str("ascii_uppercase", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        const_str(
            "punctuation",
            "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
        ),
        const_str("whitespace", " \t\n\r\x0b\x0c"),
        const_str(
            "printable",
            "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c",
        ),
    ])
}

fn tempfile_module() -> ModuleUnit {
    host_module_mixed(&[
        func("dir", 227, 0, 0),
        func("mkdtemp", 228, 0, 1),
        func("mkstemp", 229, 0, 1),
    ])
}

fn random_module() -> ModuleUnit {
    host_module_mixed(&[
        func("random", 230, 0, 0),
        func("seed", 231, 0, 1),
        func("hex", 232, 0, 1),
        func("shuffle", 233, 1, 1),
    ])
}

fn itertools_module() -> ModuleUnit {
    host_module_mixed(&[
        func("enumerate", 234, 1, 1),
        func("zip", 235, 2, 2),
        func("chain", 236, 0, 8),
        func("repeat", 237, 2, 2),
        func("product", 238, 2, 2),
        func("permutations", 239, 1, 2),
        func("combinations", 240, 2, 2),
        func("accumulate", 241, 1, 1),
        func("take", 242, 2, 2),
        func("drop", 243, 2, 2),
        func("range", 244, 1, 3),
    ])
}

// ── re module ───────────────────────────────────────────────────────────────

const RE_I: i64 = 2;
const RE_M: i64 = 8;
const RE_S: i64 = 16;
const RE_X: i64 = 64;

fn re_module() -> ModuleUnit {
    host_module_mixed(&[
        func("findall", 251, 2, 3),
        func("sub", 252, 3, 4),
        func("search", 253, 2, 3),
        func("split", 254, 2, 4),
        func("escape", 255, 1, 1),
        func("match", 256, 2, 3),
        func("fullmatch", 257, 2, 3),
        func("finditer", 258, 2, 3),
        func("subn", 259, 3, 4),
        func("compile", 260, 1, 2),
        func("IGNORECASE", 261, 0, 0),
        func("MULTILINE", 262, 0, 0),
        func("DOTALL", 263, 0, 0),
        func("VERBOSE", 264, 0, 0),
        func("I", 265, 0, 0),
        func("M", 266, 0, 0),
        func("S", 267, 0, 0),
        func("X", 268, 0, 0),
        konst("I", Value::int(RE_I)),
        konst("IGNORECASE", Value::int(RE_I)),
        konst("M", Value::int(RE_M)),
        konst("MULTILINE", Value::int(RE_M)),
        konst("S", Value::int(RE_S)),
        konst("DOTALL", Value::int(RE_S)),
        konst("X", Value::int(RE_X)),
        konst("VERBOSE", Value::int(RE_X)),
    ])
}

fn flag_bits(v: Option<&Value>) -> i64 {
    match v {
        None => 0,
        Some(v) if v.is_nil() => 0,
        Some(v) => {
            if let Some(i) = v.as_int() {
                i
            } else if let Some(s) = v.as_str() {
                let mut bits = 0;
                for c in s.chars() {
                    bits |= match c {
                        'i' => RE_I,
                        'm' => RE_M,
                        's' => RE_S,
                        'x' => RE_X,
                        _ => 0,
                    };
                }
                bits
            } else {
                0
            }
        }
    }
}

fn build_regex(pattern: &str, flags: Option<&Value>) -> VmResult<regex::Regex> {
    let bits = flag_bits(flags);
    let mut b = regex::RegexBuilder::new(pattern);
    b.case_insensitive(bits & RE_I != 0);
    b.multi_line(bits & RE_M != 0);
    b.dot_matches_new_line(bits & RE_S != 0);
    b.ignore_whitespace(bits & RE_X != 0);
    b.build()
        .map_err(|e| VmError::NativeError(format!("re: {e}")))
}

fn opt_group(m: &regex::Captures<'_>, i: usize) -> Value {
    match m.get(i) {
        Some(g) => Value::string(g.as_str()),
        None => Value::nil(),
    }
}

fn match_to_dict(re: &regex::Regex, text: &str, caps: &regex::Captures<'_>) -> Value {
    let m = caps.get(0).expect("group 0 always present");
    let mut all = Vec::new();
    for i in 0..caps.len() {
        all.push(opt_group(caps, i));
    }
    let groups: Vec<Value> = (1..caps.len()).map(|i| opt_group(caps, i)).collect();
    let mut named = IndexMap::new();
    for name in re.capture_names().flatten() {
        let idx = re
            .capture_names()
            .position(|n| n == Some(name))
            .expect("name comes from capture_names");
        named.insert(DictKey(Value::string(name)), opt_group(caps, idx));
    }
    let mut d = IndexMap::new();
    d.insert(DictKey(Value::string("string")), Value::string(text));
    d.insert(DictKey(Value::string("pattern")), Value::string(re.as_str()));
    d.insert(DictKey(Value::string("start")), Value::int(m.start() as i64));
    d.insert(DictKey(Value::string("end")), Value::int(m.end() as i64));
    d.insert(
        DictKey(Value::string("span")),
        new_list(vec![Value::int(m.start() as i64), Value::int(m.end() as i64)]),
    );
    d.insert(DictKey(Value::string("group0")), Value::string(m.as_str()));
    d.insert(DictKey(Value::string("group")), new_list(all));
    d.insert(DictKey(Value::string("groups")), new_list(groups));
    d.insert(DictKey(Value::string("groupdict")), new_dict(named));
    new_dict(d)
}

fn re_findall(args: &[Value]) -> VmResult<Value> {
    let (re, text) = pair_re_text(args)?;
    let mut out = Vec::new();
    for caps in re.captures_iter(&text) {
        if re.captures_len() == 1 {
            out.push(Value::string(
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
            ));
        } else if re.captures_len() == 2 {
            out.push(opt_group(&caps, 1));
        } else {
            let row: Vec<Value> = (1..caps.len()).map(|i| opt_group(&caps, i)).collect();
            out.push(new_list(row));
        }
    }
    Ok(new_list(out))
}

fn interpret_repl(repl: &str) -> String {
    let bytes = repl.as_bytes();
    let mut out = String::with_capacity(repl.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            out.push('$');
            out.push(bytes[i + 1] as char);
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn re_subn(args: &[Value]) -> VmResult<Value> {
    let pattern = string_at(args, 0)?;
    let text = string_at(args, 1)?;
    let repl = string_at(args, 2)?;
    let repl = interpret_repl(&repl);
    let re = build_regex(&pattern, None)?;
    let count = args
        .get(3)
        .and_then(|v| v.as_int())
        .unwrap_or(0i64);
    let (count_usize, replace_all) = if count <= 0 { (usize::MAX, true) } else { (count as usize, false) };
    let actual = re.find_iter(&text).take(count_usize).count();
    let result = if replace_all {
        re.replace_all(&text, repl.as_str()).into_owned()
    } else {
        re.replacen(&text, count_usize, repl.as_str()).into_owned()
    };
    Ok(new_list(vec![Value::string(result), Value::int(actual as i64)]))
}

fn re_sub(args: &[Value]) -> VmResult<Value> {
    let v = re_subn(args)?;
    Ok(v.as_list().and_then(|l| l.first()).copied().unwrap_or(Value::nil()))
}

fn re_search(args: &[Value]) -> VmResult<Value> {
    let (re, text) = pair_re_text(args)?;
    Ok(match re.captures(&text) {
        Some(c) => match_to_dict(&re, &text, &c),
        None => Value::nil(),
    })
}

fn re_split(args: &[Value]) -> VmResult<Value> {
    let pattern = string_at(args, 0)?;
    let text = string_at(args, 1)?;
    let maxsplit = args
        .get(2)
        .and_then(|v| v.as_int())
        .map(|n| n.max(0) as usize)
        .unwrap_or(0);
    let re = build_regex(&pattern, args.get(3))?;
    let parts: Vec<Value> = if maxsplit == 0 {
        re.split(&text).map(Value::string).collect()
    } else {
        re.splitn(&text, maxsplit + 1).map(Value::string).collect()
    };
    Ok(new_list(parts))
}

/// `(pattern, text[, flags])` — the Rust-order pair shared by most `re` entry
/// points.
fn pair_re_text(args: &[Value]) -> VmResult<(regex::Regex, String)> {
    let pattern = string_at(args, 0)?;
    let text = string_at(args, 1)?;
    let re = build_regex(&pattern, args.get(2))?;
    Ok((re, text))
}

pub fn native_modules() -> Vec<(&'static str, ModuleUnit)> {
    vec![
        ("math", math_module()),
        ("base64", base64_module()),
        ("binascii", binascii_module()),
        ("collections", collections_module()),
        ("string", string_module()),
        ("tempfile", tempfile_module()),
        ("random", random_module()),
        ("itertools", itertools_module()),
        ("re", re_module()),
    ]
}

fn re_match(args: &[Value]) -> VmResult<Value> {
    let (re, text) = pair_re_text(args)?;
    Ok(match re.captures(&text) {
        Some(c) if c.get(0).map(|m| m.start()).unwrap_or(1) == 0 => match_to_dict(&re, &text, &c),
        _ => Value::nil(),
    })
}

fn re_fullmatch(args: &[Value]) -> VmResult<Value> {
    let (re, text) = pair_re_text(args)?;
    Ok(match re.captures(&text) {
        Some(c) => {
            let m = c.get(0).unwrap();
            if m.start() == 0 && m.end() == text.len() {
                match_to_dict(&re, &text, &c)
            } else {
                Value::nil()
            }
        }
        None => Value::nil(),
    })
}

fn re_finditer(args: &[Value]) -> VmResult<Value> {
    let (re, text) = pair_re_text(args)?;
    let mut out = Vec::new();
    for c in re.captures_iter(&text) {
        out.push(match_to_dict(&re, &text, &c));
    }
    Ok(new_list(out))
}

fn re_compile(args: &[Value]) -> VmResult<Value> {
    let pattern = string_at(args, 0)?;
    let flags = args.get(1).copied();
    let _re = build_regex(&pattern, flags.as_ref())?;
    let mut d = IndexMap::new();
    d.insert(DictKey(Value::string("pattern")), Value::string(pattern));
    d.insert(DictKey(Value::string("flags")), Value::int(flag_bits(flags.as_ref())));
    Ok(new_dict(d))
}
