# 04 — 16-Tag NaN-Boxed Value System

> **Status:** Canonical (corrected from blueprint)  
> **Milestone:** M2  
> **ADR:** [ADR-0001](decisions/ADR-0001-tag-budget.md)

---

## 1. Why NaN-Boxing?

A 64-bit float (IEEE-754) has 52 explicit mantissa bits + 1 implicit. A Quiet NaN sets:
- Sign bit: 1 (arbitrary)
- Exponent (11 bits): all 1s (`0x7FF`)
- Quiet bit (mantissa bit 51): 1

This leaves **51 payload bits** that are never produced by hardware FP operations. We use:
- **3 tag bits** at positions 48–50 (shift 48, mask `0x7`)
- **48-bit payload** at positions 0–47

```
 63    62 ......... 52 51 50 49 48 47 ......................... 0
+---------+---------------+---+---+---+----------------------------+
| S       | Exponent (11) | 1 | T | A |   Tag (3 bits)   |  Payload   |
|         |  = all 1s     | q | a | g |  (0–7)           |  (48 bits) |
+---------+---------------+---+---+---+----------------------------+
                                            QNAN base = 0x7FF8_0000_0000_0000
```

**Why not 4 bits at shift 44 (blueprint)?** 5-level paging on x86-64 enables 57-bit userspace addresses. The Linux kernel explicitly warns: *"some JIT compilers use higher bits in pointers to encode their information. It collides with valid pointers with 5-level paging and leads to crashes."* A 44-bit payload cannot represent all legal allocations. 48-bit payload + 3 tag bits is safe on all current and future address spaces.

---

## 2. Tag Map (8 Machine Tags → 16 Language Types)

| Tag | Machine Type | Language Types Discriminated By `ObjHeader.kind` |
|---|---|---|
| `0b000` | **Nil** | `nil` / `null` |
| `0b001` | **False** | `false` |
| `0b010` | **True** | `true` |
| `0b011` | **Int** | `int` (i48, sign-extended) |
| `0b100` | **Float** | `float` (raw f64 bit pattern) |
| `0b101` | **String** | `string` (pointer to `StringObj`) |
| `0b110` | **Bytes** | `bytes` (pointer to `BytesObj`) |
| `0b111` | **Heap** | `list`, `dict`, `set`, `tuple`, `func`, `instance`, `error`, `channel`, `mutex`, `handle` |

The 9 heap types share tag `0b111`. Their concrete type is read from `ObjHeader.kind` (a `u8` at offset 0 of every heap object). One extra load from a cache line we're already touching.

---

## 3. Constants

```rust
pub const QNAN: u64 = 0x7FF8_0000_0000_0000;
pub const TAG_SHIFT: u64 = 48;
pub const TAG_MASK: u64 = 0x7;
pub const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF; // 48 bits
```

---

## 4. Immediate Constructors

```rust
impl Value {
    #[inline] pub const fn nil() -> Self { Self(QNAN | (Tag::Nil as u64) << TAG_SHIFT) }
    #[inline] pub const fn bool(b: bool) -> Self {
        Self(QNAN | (if b { Tag::True } else { Tag::False } as u64) << TAG_SHIFT)
    }
    #[inline] pub fn int(n: i64) -> Self {
        debug_assert!(n >= -(1i64 << 47) && n < (1i64 << 47), "i48 overflow");
        Self(QNAN | (Tag::Int as u64) << TAG_SHIFT | ((n as u64) & PAYLOAD_MASK))
    }
    #[inline] pub fn float(f: f64) -> Self {
        let bits = f.to_bits();
        // Normalize any NaN to our canonical QNAN
        if (bits & 0x7FF0_0000_0000_0000) == 0x7FF0_0000_0000_0000 {
            Self(QNAN | (Tag::Float as u64) << TAG_SHIFT)
        } else {
            Self(bits)
        }
    }
    #[inline] pub fn heap(ptr: *const ObjHeader) -> Self {
        Self(QNAN | (Tag::Heap as u64) << TAG_SHIFT | (ptr as u64 & PAYLOAD_MASK))
    }
}
```

---

## 5. Accessors

```rust
impl Value {
    #[inline] pub fn raw(self) -> u64 { self.0 }
    #[inline] pub fn tag(self) -> Tag { Tag::from_value(self) }
    #[inline] pub fn is_nil(self) -> bool { self.0 == Self::nil().0 }
    #[inline] pub fn as_bool(self) -> Option<bool> {
        match self.tag() { Tag::True => Some(true), Tag::False => Some(false), _ => None }
    }
    #[inline] pub fn as_int(self) -> Option<i64> {
        if self.tag() == Tag::Int {
            let raw = (self.0 & PAYLOAD_MASK) as i64;
            Some((raw << 16) >> 16) // sign-extend from 48 to 64
        } else { None }
    }
    #[inline] pub fn as_float(self) -> Option<f64> {
        if self.tag() == Tag::Float { Some(f64::from_bits(self.0)) } else { None }
    }
    #[inline] pub fn as_ptr(self) -> *const ObjHeader { (self.0 & PAYLOAD_MASK) as *const ObjHeader }
    #[inline] pub fn as_mut_ptr(self) -> *mut ObjHeader { (self.0 & PAYLOAD_MASK) as *mut ObjHeader }
}
```

---

## 6. Integer Semantics

- **Range:** i48 = `[-2^47, 2^47)` ≈ `±140 trillion`
- **Overflow:** **Promotes to float** (Lua 5.4 style), does NOT wrap.
  - `2^47 + 1` → `140737488355328.0` (float)
  - `INT_MIN / -1` = `2^47` → in range, no trap
- **Division:** `int / int` → float (so `10 / 4 == 2.5` per existing tests)
- **Floor division:** `//` operator for integer division (truncates toward negative infinity)

---

## 7. Float Semantics

- Raw f64 when exponent != all-1s
- All NaN bit patterns normalized to canonical QNAN (tag `Float`, payload 0)
- `-0.0` preserved (distinct bit pattern from `+0.0`)
- `Float(0.0).tag() == Tag::Float` (not `Nil`)

---

## 8. Heap Object Layout

Every heap object is a `#[repr(C)]` struct:

```rust
#[repr(C)]
pub struct ObjHeader {
    pub kind: ObjKind,  // u8 discriminant
    pub flags: u8,      // reserved
    pub rc: u32,        // refcount for ArcGc
}

// Kind discriminant (matches ObjKind enum)
#[repr(u8)]
pub enum ObjKind {
    List    = 0,
    Dict    = 1,
    Set     = 2,
    Tuple   = 3,
    Func    = 4,
    Instance= 5,
    Error   = 6,
    Channel = 7,
    Mutex   = 8,
    Handle  = 9,
    String  = 10,   // Tag::String has its own machine tag, but kept for uniformity
    Bytes   = 11,
}

// Payload structs (header + data)
#[repr(C)] pub struct StringObj  { pub header: ObjHeader, pub data: String }
#[repr(C)] pub struct BytesObj   { pub header: ObjHeader, pub data: Vec<u8> }
#[repr(C)] pub struct ListObj    { pub header: ObjHeader, pub data: Arc<Vec<Value>> }
#[repr(C)] pub struct DictObj    { pub header: ObjHeader, pub data: Arc<IndexMap<Value, Value>> }
#[repr(C)] pub struct SetObj     { pub header: ObjHeader, pub data: Arc<IndexSet<Value>> }
#[repr(C)] pub struct TupleObj   { pub header: ObjHeader, pub data: Arc<Vec<Value>> }
#[repr(C)] pub struct FuncObj    { pub header: ObjHeader, pub proto: Arc<FunctionProto>, pub upvals: Vec<Value> }
#[repr(C)] pub struct ErrorObj   { pub header: ObjHeader, pub msg: String, pub code: i32 }
#[repr(C)] pub struct ChannelObj { pub header: ObjHeader, pub sender: Sender<Value>, pub receiver: Receiver<Value> }
#[repr(C)] pub struct MutexObj   { pub header: ObjHeader, pub data: Mutex<Value> }
#[repr(C)] pub struct HandleObj  { pub header: ObjHeader, pub id: u32 }
```

**Refcount discipline:** `ArcGc::allocate_*` does `Box::new(payload) → Box::into_raw() → Value::heap(ptr)`. Drop is manual via `Arc::from_raw` when refcount hits 0 (handled by `ArcGc` internals).

---

## 9. Type Predicates (Language-Level)

```zen
typeof(42)           # "int"
typeof(3.14)         # "float"
typeof("hi")         # "string"
typeof(bytes([1]))   # "bytes"
typeof([1,2])        # "list"
typeof(#{a:1})       # "dict"
typeof(%{1,2})       # "set"
typeof(#(1,2))       # "tuple"
typeof(fn(){})       # "func"
typeof(nil)          # "nil"
typeof(true)         # "bool"
typeof(error("x"))   # "error"
typeof(channel())    # "channel"
typeof(mutex.new())  # "mutex"
typeof(tcp.connect())# "handle"  (NOT "int" — blueprint was wrong)
```

**`bool(v)` coercion:**
- `nil`, `false`, `0`, `0.0`, `""`, `bytes([])`, `[]`, `#{}`, `%{}`, `#()` → `false`
- Everything else → `true`

---

## 10. Safety Invariants (Enforced by `Value` module)

| Invariant | Check |
|---|---|
| Pointer fits in 48 bits | `debug_assert!` in `Value::heap()` |
| `Tag::Float` values never have QNAN bits | Normalized in constructor |
| `Tag::Int` payload sign-extends correctly | Unit test boundary values |
| `ObjHeader.kind` matches `ObjKind` enum | Exhaustive match in `Debug` impl |
| No `unsafe` outside `value.rs` + `gc.rs` | Clippy `forbid(unsafe_op_in_unsafe_fn)` + crate lint |

---

## 11. Differences from Blueprint

| Blueprint Claim | Corrected Spec |
|---|---|
| 4 tag bits @ shift 44 | **3 tag bits @ shift 48** (48-bit payload) |
| 16 machine tags | **8 machine tags** + header discriminant |
| `typeof(handle) == "int"` | **`typeof(handle) == "handle"`** (honest) |
| i44 integer | **i48 integer** (±140T) |
| Int overflow wraps | **Promotes to float** |
| `Float` tag not in table | **Tag 0b100 = Float** |
| `QNAN` diagram wrong | **Corrected** (see §1) |

---

## 12. Implementation Location

- `crates/zen-vm/src/value.rs` — `Value`, `Tag`, `ObjKind`, `ObjHeader`, constructors, accessors
- `crates/zen-vm/src/gc.rs` — Object structs (`StringObj`, `ListObj`, etc.), `ArcGc` implementation
- All `unsafe` confined to these two files.