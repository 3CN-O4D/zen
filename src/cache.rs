//! On-disk bytecode cache.
//!
//! Warm runs skip lex + parse + compile entirely: the compiled program
//! (`Vec<Arc<CompiledFunction>>`) is serialized next to the source under
//! `.zencache/<hash>.zc`, keyed by a hash of the source text, the Zen
//! version and the cache format version. Any mismatch, corruption, or
//! non-cacheable content (non-literal constants) silently falls back to
//! the normal parse/compile path.
//!
//! Set `ZEN_NO_CACHE=1` to disable reading and writing caches.

use crate::bytecode::{CompiledFunction, Instruction, Opcode};
use crate::runtime::Value;
use std::path::Path;
use std::sync::Arc;

const MAGIC: &[u8; 4] = b"ZENC";
const FORMAT: u32 = 1;
const MAX_DEPTH: usize = 64;

/// Stable opcode <-> u16 mapping. The enum's memory layout must never be
/// relied upon for persistence; these tables are explicit so reordering or
/// inserting variants cannot corrupt old caches (the Zen version in the key
/// invalidates stale files anyway).
fn op_to_id(op: Opcode) -> u16 {
    match op {
        Opcode::Pop => 0,
        Opcode::Dup => 1,
        Opcode::Const => 2,
        Opcode::True => 3,
        Opcode::False => 4,
        Opcode::Null => 5,
        Opcode::LoadLocal => 6,
        Opcode::StoreLocal => 7,
        Opcode::LoadGlobal => 8,
        Opcode::StoreGlobal => 9,
        Opcode::CheckLockedAssign => 10,
        Opcode::CheckLockedRedefine => 11,
        Opcode::LockGlobal => 12,
        Opcode::UnlockGlobal => 13,
        Opcode::Add => 14,
        Opcode::Sub => 15,
        Opcode::Mul => 16,
        Opcode::Div => 17,
        Opcode::Mod => 18,
        Opcode::Pow => 19,
        Opcode::Neg => 20,
        Opcode::Eq => 21,
        Opcode::Ne => 22,
        Opcode::Lt => 23,
        Opcode::Le => 24,
        Opcode::Gt => 25,
        Opcode::Ge => 26,
        Opcode::Not => 27,
        Opcode::AddGlobal => 28,
        Opcode::SubGlobal => 29,
        Opcode::MulGlobal => 30,
        Opcode::DivGlobal => 31,
        Opcode::ModGlobal => 32,
        Opcode::AddLocal => 33,
        Opcode::SubLocal => 34,
        Opcode::MulLocal => 35,
        Opcode::DivLocal => 36,
        Opcode::ModLocal => 37,
        Opcode::Jmp => 38,
        Opcode::JmpIfFalse => 39,
        Opcode::JmpIfTrue => 40,
        Opcode::JmpLtLocal => 41,
        Opcode::JmpLeLocal => 42,
        Opcode::JmpLtLocalConst => 43,
        Opcode::JmpLeLocalConst => 44,
        Opcode::JmpGtLocalConst => 45,
        Opcode::JmpGeLocalConst => 46,
        Opcode::AddLocalImm => 47,
        Opcode::SubLocalImm => 48,
        Opcode::JmpIfNotNull => 49,
        Opcode::Call => 50,
        Opcode::CallMethod => 51,
        Opcode::PushSlot => 52,
        Opcode::PopSlot => 53,
        Opcode::PushGlobal => 54,
        Opcode::PopGlobal => 55,
        Opcode::Return => 56,
        Opcode::Print => 57,
        Opcode::BuildList => 58,
        Opcode::BuildDict => 59,
        Opcode::Index => 60,
        Opcode::GetMember => 61,
        Opcode::Typeof => 62,
        Opcode::Len => 63,
        Opcode::DefineFunction => 64,
        Opcode::Closure => 65,
        Opcode::Import => 66,
        Opcode::ImportFrom => 67,
        Opcode::ImportStar => 68,
        Opcode::MakeRange => 69,
        Opcode::JmpLtGlobalConst => 70,
        Opcode::JmpLeGlobalConst => 71,
        Opcode::JmpGtGlobalConst => 72,
        Opcode::JmpGeGlobalConst => 73,
        Opcode::AddGlobalImm => 74,
        Opcode::SubGlobalImm => 75,
        Opcode::CallValue => 76,
    }
}

fn op_from_id(id: u16) -> Option<Opcode> {
    Some(match id {
        0 => Opcode::Pop,
        1 => Opcode::Dup,
        2 => Opcode::Const,
        3 => Opcode::True,
        4 => Opcode::False,
        5 => Opcode::Null,
        6 => Opcode::LoadLocal,
        7 => Opcode::StoreLocal,
        8 => Opcode::LoadGlobal,
        9 => Opcode::StoreGlobal,
        10 => Opcode::CheckLockedAssign,
        11 => Opcode::CheckLockedRedefine,
        12 => Opcode::LockGlobal,
        13 => Opcode::UnlockGlobal,
        14 => Opcode::Add,
        15 => Opcode::Sub,
        16 => Opcode::Mul,
        17 => Opcode::Div,
        18 => Opcode::Mod,
        19 => Opcode::Pow,
        20 => Opcode::Neg,
        21 => Opcode::Eq,
        22 => Opcode::Ne,
        23 => Opcode::Lt,
        24 => Opcode::Le,
        25 => Opcode::Gt,
        26 => Opcode::Ge,
        27 => Opcode::Not,
        28 => Opcode::AddGlobal,
        29 => Opcode::SubGlobal,
        30 => Opcode::MulGlobal,
        31 => Opcode::DivGlobal,
        32 => Opcode::ModGlobal,
        33 => Opcode::AddLocal,
        34 => Opcode::SubLocal,
        35 => Opcode::MulLocal,
        36 => Opcode::DivLocal,
        37 => Opcode::ModLocal,
        38 => Opcode::Jmp,
        39 => Opcode::JmpIfFalse,
        40 => Opcode::JmpIfTrue,
        41 => Opcode::JmpLtLocal,
        42 => Opcode::JmpLeLocal,
        43 => Opcode::JmpLtLocalConst,
        44 => Opcode::JmpLeLocalConst,
        45 => Opcode::JmpGtLocalConst,
        46 => Opcode::JmpGeLocalConst,
        47 => Opcode::AddLocalImm,
        48 => Opcode::SubLocalImm,
        49 => Opcode::JmpIfNotNull,
        50 => Opcode::Call,
        51 => Opcode::CallMethod,
        52 => Opcode::PushSlot,
        53 => Opcode::PopSlot,
        54 => Opcode::PushGlobal,
        55 => Opcode::PopGlobal,
        56 => Opcode::Return,
        57 => Opcode::Print,
        58 => Opcode::BuildList,
        59 => Opcode::BuildDict,
        60 => Opcode::Index,
        61 => Opcode::GetMember,
        62 => Opcode::Typeof,
        63 => Opcode::Len,
        64 => Opcode::DefineFunction,
        65 => Opcode::Closure,
        66 => Opcode::Import,
        67 => Opcode::ImportFrom,
        68 => Opcode::ImportStar,
        69 => Opcode::MakeRange,
        70 => Opcode::JmpLtGlobalConst,
        71 => Opcode::JmpLeGlobalConst,
        72 => Opcode::JmpGtGlobalConst,
        73 => Opcode::JmpGeGlobalConst,
        74 => Opcode::AddGlobalImm,
        75 => Opcode::SubGlobalImm,
        76 => Opcode::CallValue,
        _ => return None,
    })
}

fn disabled() -> bool {
    std::env::var("ZEN_NO_CACHE").map(|v| v == "1" || v == "true").unwrap_or(false)
}

/// Content hash of the source plus everything that could change its meaning.
fn source_hash(source: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    FORMAT.hash(&mut h);
    env!("CARGO_PKG_VERSION").hash(&mut h);
    source.hash(&mut h);
    h.finish()
}

fn cache_file(source_path: &str, source: &str) -> Option<std::path::PathBuf> {
    if disabled() {
        return None;
    }
    let dir = Path::new(source_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Some(dir.join(".zencache").join(format!("{:016x}.zc", source_hash(source))))
}

// ─── byte writer ─────────────────────────────────────────────────────────

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
    fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.buf.len() {
            return None;
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes(s.try_into().unwrap()))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
    }
    fn f64(&mut self) -> Option<f64> {
        self.u64().map(|b| f64::from_bits(b))
    }
    fn str(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }
    /// Reject trailing garbage so truncated/mismatched files never parse.
    fn exhausted(&self) -> bool {
        self.pos == self.buf.len()
    }
}

// ─── value codec (literal values only) ──────────────────────────────────

const TAG_NULL: u8 = 0;
const TAG_BOOL: u8 = 1;
const TAG_NUM: u8 = 2;
const TAG_STR: u8 = 3;
const TAG_LIST: u8 = 4;
const TAG_DICT: u8 = 5;

fn write_value(w: &mut Writer, v: &Value, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    match v {
        Value::Null => {
            w.u8(TAG_NULL);
            true
        }
        Value::Bool(b) => {
            w.u8(TAG_BOOL);
            w.u8(*b as u8);
            true
        }
        Value::Number(n) => {
            w.u8(TAG_NUM);
            w.f64(*n);
            true
        }
        Value::String(s) => {
            w.u8(TAG_STR);
            w.str(s);
            true
        }
        Value::List(items) => {
            if items.len() > u32::MAX as usize {
                return false;
            }
            w.u8(TAG_LIST);
            w.u32(items.len() as u32);
            for item in items.iter() {
                if !write_value(w, item, depth + 1) {
                    return false;
                }
            }
            true
        }
        Value::Dict(map) => {
            w.u8(TAG_DICT);
            w.u32(map.len() as u32);
            for (k, val) in map.iter() {
                w.str(k);
                if !write_value(w, val, depth + 1) {
                    return false;
                }
            }
            true
        }
        // Functions, instances, sockets etc. can never appear in a compiled
        // constant pool; refuse rather than mis-serialize.
        _ => false,
    }
}

fn read_value(r: &mut Reader, depth: usize) -> Option<Value> {
    if depth > MAX_DEPTH {
        return None;
    }
    match r.u8()? {
        TAG_NULL => Some(Value::Null),
        TAG_BOOL => Some(Value::Bool(r.u8()? != 0)),
        TAG_NUM => Some(Value::Number(r.f64()?)),
        TAG_STR => Some(Value::String(r.str()?)),
        TAG_LIST => {
            let n = r.u32()? as usize;
            let mut items = Vec::with_capacity(n.min(1024));
            for _ in 0..n {
                items.push(read_value(r, depth + 1)?);
            }
            Some(Value::List(Arc::new(items)))
        }
        TAG_DICT => {
            let n = r.u32()? as usize;
            let mut map = indexmap::IndexMap::new();
            for _ in 0..n {
                let k = r.str()?;
                let v = read_value(r, depth + 1)?;
                map.insert(k, v);
            }
            Some(Value::Dict(Arc::new(map)))
        }
        _ => None,
    }
}

// ─── compiled function codec ─────────────────────────────────────────────

fn write_cf(w: &mut Writer, cf: &CompiledFunction) -> bool {
    w.str(&cf.name);
    w.u32(cf.params.len() as u32);
    for p in &cf.params {
        w.str(p);
    }
    w.u16(cf.param_count);
    w.u32(cf.captured_names.len() as u32);
    for c in &cf.captured_names {
        w.str(c);
    }
    w.u16(cf.local_count);
    w.u32(cf.instructions.len() as u32);
    for inst in &cf.instructions {
        w.u16(op_to_id(inst.opcode));
        w.u16(inst.arg1);
        w.u16(inst.arg2);
        w.u16(inst.arg3);
    }
    w.u32(cf.constants.len() as u32);
    for c in &cf.constants {
        if !write_value(w, c, 0) {
            return false;
        }
    }
    true
}

fn read_cf(r: &mut Reader) -> Option<CompiledFunction> {
    let name = r.str()?;
    let pn = r.u32()? as usize;
    let mut params = Vec::with_capacity(pn.min(4096));
    for _ in 0..pn {
        params.push(r.str()?);
    }
    let param_count = r.u16()?;
    let cn = r.u32()? as usize;
    let mut captured_names = Vec::with_capacity(cn.min(4096));
    for _ in 0..cn {
        captured_names.push(r.str()?);
    }
    let local_count = r.u16()?;
    let in_n = r.u32()? as usize;
    let mut instructions = Vec::with_capacity(in_n.min(1 << 20));
    for _ in 0..in_n {
        let op = op_from_id(r.u16()?)?;
        instructions.push(Instruction::new(op, r.u16()?, r.u16()?, r.u16()?));
    }
    let c_n = r.u32()? as usize;
    let mut constants = Vec::with_capacity(c_n.min(1 << 20));
    for _ in 0..c_n {
        constants.push(read_value(r, 0)?);
    }
    Some(CompiledFunction {
        name,
        params,
        param_count,
        captured_names,
        local_count,
        instructions,
        constants,
    })
}

// ─── public API ──────────────────────────────────────────────────────────

/// Try to load a cached compiled program for this source. Returns None on
/// any miss, mismatch, corruption, or when caching is disabled.
pub fn load(source_path: &str, source: &str) -> Option<Vec<Arc<CompiledFunction>>> {
    let file = cache_file(source_path, source)?;
    let bytes = std::fs::read(file).ok()?;
    let mut r = Reader { buf: &bytes, pos: 0 };
    if r.take(4)? != MAGIC {
        return None;
    }
    if r.u32()? != FORMAT {
        return None;
    }
    let n = r.u32()? as usize;
    if n > 1 << 16 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(Arc::new(read_cf(&mut r)?));
    }
    if !r.exhausted() {
        return None;
    }
    Some(out)
}

/// Serialize and atomically store the compiled program. Best-effort: any
/// failure (unwritable directory, non-literal constants, ...) is silent.
pub fn store(source_path: &str, source: &str, funcs: &[Arc<CompiledFunction>]) {
    if disabled() || funcs.is_empty() {
        return;
    }
    let Some(file) = cache_file(source_path, source) else {
        return;
    };
    let mut w = Writer::new();
    w.buf.extend_from_slice(MAGIC);
    w.u32(FORMAT);
    w.u32(funcs.len() as u32);
    for cf in funcs {
        if !write_cf(&mut w, cf) {
            return;
        }
    }
    let bytes = w.into_bytes();
    let dir = match file.parent() {
        Some(d) => d,
        None => return,
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!(
        ".tmp-{}-{:016x}",
        std::process::id(),
        source_hash(source)
    ));
    if std::fs::write(&tmp, &bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
}
