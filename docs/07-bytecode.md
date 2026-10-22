# 07 — Register Bytecode and the Compiler

> **Status:** Draft for review. The rulings in §2.3, §3 (preamble), §5.2, §6.2, §9.1, §10.4 and
> §11.4 are the ones a reviewer should attack first; they are listed together in §14.
> **Milestone:** M4 (Register compiler + VM). The bytecode defined here is also the input to the
> M11 Cranelift translator — see [`10-jit.md`](10-jit.md) and [`11-jit-internals.md`](11-jit-internals.md).
> **Supersedes:** the abandoned stack-machine `src/bytecode.rs` in the Zen 2.2.0 tree.

### How to read the numbers in this document

| Marker | Meaning |
|---|---|
| **[M]** | **Measured**, by a command named in the same sentence. Reproducible today. |
| **[T]** | **Target.** A gate we commit to; checked by an M4 exit test. Not yet achieved. |
| **[E]** | **Estimate.** A design-time number reasoned from first principles or from a comparable system (LuaJIT, CPython 3.13, V8). Used to size a structure. If an **[E]** number is load-bearing for a decision, the decision is repeated in §14 as something to measure. |

**At the time of writing there are zero [M] numbers in this document**, because the compiler does
not exist yet. Every number below is **[T]** or **[E]**. This is stated up front so that no
reviewer mistakes a design target for a result.

---

## Contents

1. [Purpose and scope](#1-purpose-and-scope)
2. [Instruction encoding](#2-instruction-encoding)
3. [The opcode table](#3-the-opcode-table)
4. [`Instruction` and disassembly](#4-instruction-and-disassembly)
5. [`FunctionProto`, constants, and the local/upvalue tables](#5-functionproto-constants-and-the-localupvalue-tables)
6. [The compiler](#6-the-compiler)
7. [Loop compilation and the iteration protocol](#7-loop-compilation-and-the-iteration-protocol)
8. [`try` / `catch` / `finally` lowering](#8-try--catch--finally-lowering)
9. [Closures and upvalues](#9-closures-and-upvalues)
10. [Serialisation: the `.zeno` container](#10-serialisation-the-zeno-container)
11. [Optimisation passes](#11-optimisation-passes)
12. [Test plan](#12-test-plan)
13. [Performance notes](#13-performance-notes)
14. [Rulings a reviewer should double-check](#14-rulings-a-reviewer-should-double-check)

---

## 1. Purpose and scope

M4 replaces the Zen 2.2.0 stack-based bytecode with a **register machine**. The compiler's job is
to take the AST produced by [`06-parser.md`](06-parser.md) and emit, for every function:

- a flat `Vec<Instruction>` of fixed-width 32-bit words,
- a per-function **constant pool**,
- a **local-slot table** mapping binding names to register indices,
- an **upvalue table** describing what a closure captures and how,
- a **line table** for diagnostics, the REPL, and the M3↔M4 differential harness,
- an **exception table** for `try`/`catch`/`finally`,
- a table of **inline-cache sites**, so the M5 patcher and the M11 translator can find them without
  a separate analysis pass.

**One bytecode, two consumers.** The M4 VM in [`08-vm.md`](08-vm.md) interprets these instructions
directly. The M11 Cranelift translator walks the *same* `FunctionProto` and emits machine code. The
JIT is not fed a re-lowered, de-optimised IR; it sees the identical opcode stream the interpreter
saw, so **any disagreement between the two is a bug in the translator, not an artefact of a
divergent lowering.** This is the single most important structural decision in the document and it
constrains everything in §3: every opcode must have an unambiguous meaning that a compiler can turn
into either a machine instruction or an interpreter arm.

### 1.1 What this replaces

The old module is [`src/bytecode.rs`](../../src/bytecode.rs) in the Zen 2.2.0 tree. It is a **stack
machine** with `u16` operands:

- `enum Opcode` at `src/bytecode.rs:8` — 78 variants, including 21 fused arithmetic-on-a-slot
  variants (`AddGlobal`, `AddLocal`, `AddLocalImm`, `AddGlobalImm`, …) at `src/bytecode.rs:55-101`.
- `struct Instruction { opcode, arg1: u16, arg2: u16, arg3: u16 }` at `src/bytecode.rs:161-166` —
  **10 bytes**, so the instruction stream is not naturally aligned and the fetch is two loads plus
  a shift.
- `struct CompiledFunction` at `src/bytecode.rs:176-189`, whose `params` field holds
  `Option<crate::runtime::Expr>` — **raw AST nodes** (`src/bytecode.rs:181`). That is why
  [`src/cache.rs:388-393`](../../src/cache.rs) refuses to cache *any* function that has a default
  parameter: the default is a tree, and the tree is not in the serialiser.
- `compile_function` returns `Err(String)` for anything it cannot handle, and the caller's
  documented response is to "fall back to tree-walk interpretation for the whole function"
  (`src/bytecode.rs:217-219`). **A whole-function silent fallback is unacceptable in the new
  system**: two engines means two answers. If M4 cannot compile a construct, that is a
  `CompileError`, not a fallback. Recorded in
  [`00-blueprint-review.md`](00-blueprint-review.md) §1 K4.

Two further concrete defects in the old lowering that the new one must not reproduce:

- **`for` only works on lists.** `StmtKind::For` lowers to an index loop built from `Len`, `Index`
  and `AddLocalImm` (`src/bytecode.rs:640-686`). Iterating a `dict`, a `string`, a `set` or a
  generator through that path is either wrong or rejected. §7 replaces it with a real iteration
  protocol.
- **The fused compare-jumps are a per-site hack.** `JmpLtLocalConst`, `JmpGeLocalConst`,
  `JmpLtGlobalConst` and friends (`src/bytecode.rs:75-101`, `src/bytecode.rs:85-91`) exist because
  the stack machine could not express a cheap loop. On a register machine `JMP_IF_LT` is a
  first-class opcode (§3, category F) and these variants disappear.

---

## 2. Instruction encoding

### 2.1 The ruling

```rust
/// One instruction. Exactly 4 bytes, no padding, naturally aligned.
#[repr(C)]
pub struct Instruction {
    pub op: Op,   // u8
    pub a:  u8,
    pub b:  u8,
    pub c:  u8,
}
```

```text
 Bit  31      24 23       16 15        8 7         0
     +----------+-----------+-----------+-----------+
     |    c     |     b     |     a     |    op     |
     +----------+-----------+-----------+-----------+
     byte 3      byte 2      byte 1      byte 0
```

The diagram is a **byte-offset view on a little-endian target**, and it is the one the Rust struct
layout produces: `#[repr(C)]` puts the first field at offset 0, so `op` is the low byte. This is
not incidental — the whole point of the encoding is that the opcode is at a *fixed, known* offset
so the dispatch loop can load the byte it needs without a shift, and the JIT can emit
`cmp byte [rip], 0x2A` rather than a masked 32-bit compare.

**The file format does not depend on the host's endianness.** `SECT_CODE` stores each instruction
as an explicit little-endian `u32` per `to_bits` (§4.3), so a `.zeno` written on a big-endian host
and loaded on a little-endian one produces the same instruction stream. The in-memory `u32` is then
byte-swapped on load if and only if the host is big-endian, which is one `if` at the top of the
loader and never on a hot path.

Justification of each field:

| Field | Width | Why this width |
|---|---|---|
| `op` | `u8` | 104 core opcodes + 6 gated superinstructions = 110 variants. A `u8` leaves 146 spare IDs, which is the headroom M9–M11 need without a format break. A `u16` op would make the instruction 8 bytes for no benefit. |
| `a` | `u8` | **Destination register** in nearly every opcode. `u8` is the field size in every register VM that is fast in practice (LuaJIT, Lua 5.4, V8's baseline). |
| `b` | `u8` | **First source** — register index *or* constant-pool index *or* name id *or* jump target, depending on `op`. The `u8` keeps the whole instruction in one 32-bit load, which is the entire point. |
| `c` | `u8` | **Second source** — same. Two sources is what lets `a = b + c` be one instruction. |

The fields are overloaded on purpose. A dedicated operand *descriptor* (à la WebAssembly) costs an
extra byte and a dispatch step per instruction to save nothing, because the interpreter already has
to know the operand shape to execute it — a descriptor only moves that knowledge from a `match` arm
into a second `match`.

### 2.2 Consequences, and what we do about them

| Consequence | Limit | Ruling |
|---|---|---|
| Register indices are `u8` | **256 registers per function** | A function has at most 250 usable registers. Beyond that the compiler emits `CompileError::TooManyRegisters`. See §2.3. |
| Constant indices are `u8` | **256 distinct constants per function** | Constants are *deduplicated* by value, and only immutable-shaped values may be constants (§5.2). Over the cap → `CompileError::TooManyConstants`. |
| Jump targets are `u8` | **±127 instructions** from any jump | Jump targets are **pc-relative**, and a target that does not fit uses the reserved wide form `JMP_FAR` (§2.4). |
| Instruction stream is fixed-width | — | A feature, not a cost. `pc += 1` is an add; disassembly is a `for` loop; the `.zeno` CRC covers a dense array; the JIT's fetch is one aligned load. |

### 2.3 Why spill slots instead of an extended encoding

The blueprint offers two escapes for a function that outgrows a field: **spill slots**, or a
**function-level "extended" encoding** in which some instructions are 8 or 12 bytes wide.

**Ruling: no extended encoding. Ever.** Reasons, in order of how much they matter:

1. **A variable-length instruction stream destroys the `pc` arithmetic everything rests on.** With
   a fixed width, the interpreter's `pc`, the exception table's `try_start`/`try_end`, the inline
   cache site's `pc`, the line table's `pc`, and the JIT's block map are *the same integer* and can
   be compared and stored interchangeably. With a variable width every one of them needs a
   different unit and a bias. That is a permanent tax paid by five subsystems to avoid a case a
   compile error handles correctly.
2. **The extended form is an implicit `a = b + c` in disguise.** A 12-byte `ADD_WIDE r0, r1000,
   r1001` is just an instruction with wider operand fields. Admitting it "only in rare functions"
   creates two encodings of every concept, and the JIT must implement both. The blueprint's own
   warning applies — this is how you get an opcode soup ([`00-blueprint-review.md`](00-blueprint-review.md) §A5).
3. **A spill slot is a real, understandable, diagnosable thing.** A function needing more than 250
   simultaneously live values is a function no human can read, and generated code that hits the
   limit should be split into functions.

**How the compiler keeps real code under 250.** Two mechanisms, both already required:

- **Locals are allocated densely from register 0** in declaration order, so a function with 40
  variables uses registers 0–39.
- **Temporaries come from a free-list stack and are popped as soon as their subexpression is
  consumed** (§6.2). The high-water mark of a function is
  `num_locals + max_expression_nesting_depth`, and nesting depth is small in real code.
  Expected p99 nesting depth on the Zen stdlib: **≤ 8 [E]**; `zen check --stats` reports the real
  figure and this number is on the §14 measurement list.

What the compiler does when the limit is still exceeded:

- **`num_locals > 250`** → `CompileError::TooManyRegisters { function, needed }`, reported with a
  source span like every other compile error. A **compile error, never a wrong answer and never a
  crash** — the whole stability contract is
  [`00-blueprint-review.md`](00-blueprint-review.md) §5.
- **Reserved opcode ID `0x08` (`LOAD_LOCAL_FRAME`)** exists to remove that error class later without
  a format break: it indexes a per-frame spill `Vec<Value>`, giving effectively unbounded locals to
  code that wants them. **Reserved now, not implemented in M4** (§3.14).

### 2.4 The two wide forms we *do* allow, and the exact rule

Two, and only two, operands may be wider than a byte, each as its own opcode, and only when the
narrow form provably does not fit:

| Concept | Narrow | Wide | Rule |
|---|---|---|---|
| Jump target | `JMP b`, `b` = signed 8-bit relative | `JMP_FAR`, `u16` relative payload | Emit the wide form when `\|target - (pc+1)\| > 127`. A function larger than ~250 instructions gets a handful of `JMP_FAR`s at its edges and `JMP` everywhere else. |
| Constant index | `LOAD_CONST a, b` (`u8`) | `LOAD_CONST_W a, b, c`, index = `(b as u16) << 8 \| c` | Only reachable above 255 distinct constants, which is a compile error in M4. Reserved ID `0x07`. |

Both are in the reserved-ID space (§3.14) and are **not** in the M4 opcode table, so M4's decoder is
a single `match` on `op` with no fallback. `zen check --disasm` prints them the moment they are
implemented, and the `.zeno` format version does not change because the IDs were never reused.

### 2.5 The calling convention (stated once, used everywhere)

A register machine needs a convention for "where are the arguments". Zen uses a **register window**
with **no operand stack**:

```text
CALL          a, b, c     // callee = R[a - 1 - c], args = R[a - c .. a), result -> R[a]
CALL_KW       a, b, c     // kwargs dict = R[a - 2 - c], callee = R[a - 1 - c],
                           //   args = R[a - c .. a), result -> R[a]
CALL_METHOD   a, b, c     // receiver = R[a - 1 - c], name id = b,
                           //   args = R[a - c .. a), result -> R[a]
TAIL_CALL     a, b, c     // identical window to CALL; replaces the frame
MAKE_LIST     a, b, c     // R[a] = [R[c], R[c+1], ..., R[c + b - 1]]
MAKE_DICT     a, b, c     // R[a] = {R[c]:R[c+1], R[c+2]:R[c+3], ...}  (b key/value pairs)
MAKE_TUPLE    a, b, c     // as MAKE_LIST, immutable
MAKE_SET      a, b, c     // as MAKE_LIST, deduplicated
MAKE_RANGE    a, b, c     // b = 1..3, args in R[c .. c + b - 1] = (start, end[, step])
MAKE_KWARGS   a, b, c     // name ids b..b+c-1 paired with R[c .. c + b - 1]
MAKE_CLOSURE  a, b, c     // proto = b, upvalue sources in R[a + 1 .. a + 1 + c]
ITER_INIT     a, b        // hidden 2-slot iterator state at R[a], R[a+1]; source R[b]
```

Two things follow, and both are why the design is fast:

- **There is no operand stack.** A stack VM pushes, pops and spills on roughly one instruction in
  three. Zen's equivalent traffic is a `Vec<u8>` push/pop in the *compiler*, which happens once.
- **Arguments are already in the right place.** A call is a bounds check, a `memcpy` of `c × 8`
  bytes into the callee's register window, and `base += window_size`. No marshalling loop, no
  shuffling.

The convention is **load-bearing and frozen.** The M11 translator generates the same window layout
in machine code, and the C-level ABI in [`08-vm.md`](08-vm.md) §9 depends on it. Changing it after
M4 means re-benchmarking everything.

---

## 3. The opcode table

**104 core opcodes** plus **6 superinstructions** that are compiled only under the `zen-superins`
feature (§11.4). That is **110 variants in `enum Opcode`** — up from the 78 the abandoned
`src/bytecode.rs:8` had, and well above the blueprint's "26 opcodes" claim. That claim is formally
rejected in [`00-blueprint-review.md`](00-blueprint-review.md) **§A5**; this table is what that
section promised.

### 3.0 Preamble: the two rulings that shape the whole table

**Ruling 1 — one opcode per *operator*, not per *(operator, type)*.** There is no `ADD_INT` /
`ADD_FLOAT` / `ADD_STR` triple. `ADD` is one opcode with a fast path in the interpreter (int+int,
float+float, str+str) and a slow path that raises `TypeError`.

- The blueprint's named failure mode is "an opcode soup with `ADD_LIST_INT` style specialisations"
  ([`00-blueprint-review.md`](00-blueprint-review.md) §A5). Specialising on type *inside the opcode
  set* makes every future type a new opcode.
- The type dispatch a JIT wants is a **guard**, not an opcode. `TYPE_GUARD r0, INT, L_deopt`
  followed by a single `ADD` is exactly what a compiler backend needs: the guard is separate, so a
  failed guard is *deoptimisation*, not a branch back into generic code.
- The interpreter's cost of unification is one tag test on an operand already sitting in a register
  — the same cache line as everything else. Predicted cost: **0 ns on the int fast path, one
  predictable branch [E]**.

**Ruling 2 — one deliberate exception, and it is principled.** `CONCAT` (`0x25`) is a separate
opcode from `ADD`, emitted **only when the compiler can prove from the AST that the left operand is
a `string` or `bytes` literal or a binding with a flow-insensitive `str` type assignment.** This is a
*statically decided language-type* question, not a runtime type guess. It is the same distinction V8
draws between `Add` and `StringAdd`, and it removes two tag tests from the string-building
benchmark's critical path. The test is that the decision is made at **compile time from the AST**;
if it needed a runtime check it would be a `TypeError` case, not an opcode.

**Ruling 3 — fused compare-branches are not superinstructions.** Category F (`JMP_IF_LT` and
friends) is included. The line drawn, and enforced in the peephole's feature gate:

> A fused compare-branch **removes an intermediate materialisation** and adds **O(1)** opcodes.
> A superinstruction fuses an operation with a **load or a store**, multiplying across the operand
> space.

`JMP_IF_LT` replaces `LT r3, r0, r1` + `JMP_IF_FALSE r3, t` with one instruction and there are
exactly four of them. `ADD_LOCAL_IMM` would require one per (op × local/field × immediate), which is
the 21-variant explosion the old module actually suffered (`src/bytecode.rs:55-101`).

---

### 3.1 Category A — Constants and immediates (7)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x00` | `NOP` | — | — | — | Does nothing. Emitted as alignment padding and as a jump target. Never removed by the peephole: removing it would renumber the code array and invalidate every `pc` in every table. |
| `0x01` | `LOAD_CONST` | dst reg | const idx | — | `R[a] = K[b]`. `b` indexes the per-function constant pool (§5.2). |
| `0x02` | `LOAD_NIL` | dst reg | — | — | `R[a] = nil`. Separate from `LOAD_CONST` because `nil` appears in roughly a third of all Zen functions and the pool round-trip is two dependent loads. |
| `0x03` | `LOAD_TRUE` | dst reg | — | — | `R[a] = true`. |
| `0x04` | `LOAD_FALSE` | dst reg | — | — | `R[a] = false`. |
| `0x05` | `LOAD_SMALL_INT` | dst reg | signed imm (i8) | — | `R[a] = i48(sext(b))`. The literals `0`, `1`, `-1` and small `n` are the most frequent constants in real code. Emitted only when the value fits `i8`; everything else goes in the pool. |
| `0x06` | `LOAD_DEFAULT` | dst reg | param index | — | `R[a] = P.defaults[b]`, the pre-evaluated default for parameter `b`. This is the opcode that lets default parameters be cached (§10.4). |

### 3.2 Category B — Moves and conversions (4)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x10` | `MOVE` | dst reg | src reg | — | `R[a] = R[b]`. **Consumes** `R[b]`: the compiler guarantees `b` is dead afterwards, which is what lets the allocator reuse it. |
| `0x11` | `DUP` | dst reg | src reg | — | `R[a] = R[b]`, **and `R[b]` stays live.** Emitted when a value is used twice and the source is a named local. `MOVE` and `DUP` are distinct because collapsing them forces the allocator to pin every twice-used local, and pinning is expensive (§9.2). |
| `0x12` | `CONV` | dst reg | src reg | target type id | `R[a] = coerce(R[b], TypeId(c))`. One opcode for `int()`, `float()`, `str()`, `bytes()`, `bool()` because they are one operation with a compile-time-known target type. `TypeId` is a compile-time constant enum, never a runtime value. |
| `0x13` | `TO_BOOL` | dst reg | src reg | — | `R[a] = truthy(R[b])`. Distinct from `CONV` because truthiness is a *different function* from `bool()`: `truthy(0)` is `false` in Zen (the old runtime does exactly this at `src/runtime.rs:311-321`) but `bool(0)` would be a `TypeError`. |

### 3.3 Category C — Arithmetic (14)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x18` | `ADD` | dst | lhs reg | rhs reg | `R[a] = R[b] + R[c]`. int+int → int (i48; **overflow promotes to float**, never wraps — ADR-0001), float+float → float, str+str → string, bytes+bytes → bytes, list+list → list, set+set → set. Anything else is `TypeError`. |
| `0x19` | `SUB` | dst | lhs | rhs | `R[a] = R[b] - R[c]`. int, float, `set - set` → set. No string or dict form. |
| `0x1A` | `MUL` | dst | lhs | rhs | `R[a] = R[b] * R[c]`. int×int, float, str×int (repeat), list×int (repeat), bytes×int. |
| `0x1B` | `DIV` | dst | lhs | rhs | `R[a] = R[b] / R[c]`. **int / int promotes to float** (`10 / 4 == 2.5`, per [`00-blueprint-review.md`](00-blueprint-review.md) §6). `R[c] == 0` raises `ZeroDivisionError` — a runtime error, never constant-folded, never a trap. |
| `0x1C` | `MOD` | dst | lhs | rhs | `R[a] = R[b] % R[c]`, Python semantics (result takes the sign of the divisor). `R[c] == 0` raises `ZeroDivisionError`. |
| `0x1D` | `POW` | dst | lhs | rhs | `R[a] = R[b] ** R[c]`. int, float. Raises `OverflowError` if the result is not representable as a finite f64. |
| `0x1E` | `NEG` | dst | src | — | `R[a] = -R[b]`. Unary only; the interpreter uses the same overflow check as `SUB 0, b`. |
| `0x1F` | `BIT_AND` | dst | lhs | rhs | `R[a] = R[b] & R[c]`. int only. |
| `0x20` | `BIT_OR` | dst | lhs | rhs | `R[a] = R[b] \| R[c]`. int only. |
| `0x21` | `BIT_XOR` | dst | lhs | rhs | `R[a] = R[b] ^ R[c]`. int only. |
| `0x22` | `BIT_NOT` | dst | src | — | `R[a] = ~R[b]`. int only. |
| `0x23` | `SHL` | dst | lhs | rhs | `R[a] = R[b] << R[c]`. A shift count ≥ 48 or negative raises `ValueError`; the count is masked to 47 **only** when it is a compile-time constant, matching the existing range-check behaviour. |
| `0x24` | `SHR` | dst | lhs | rhs | `R[a] = R[b] >> R[c]`, **arithmetic** (sign-extending) shift, as in Python. There is no unsigned variant: Zen has one integer type ([`00-blueprint-review.md`](00-blueprint-review.md) §3). |
| `0x25` | `CONCAT` | dst | lhs | rhs | `R[a] = R[b] ++ R[c]` for `string`, `bytes`, `list`. Emitted only under the Ruling 2 test of §3.0. |
| `0x26` | `BUILD_STRING` | dst | template idx | count | `R[a] = build_string(K[b], R[c .. c+count-1])`. Build a string from a template constant `K[b]` (format string with `{0}`, `{1}`, … placeholders) and `count` values from registers `R[c]` onward. The template is a constant string where each `{n}` is replaced by `R[c+n]`. Emitted for string interpolation `"hello ${a} world ${b}"`. |

### 3.4 Category D — Comparison (10)

All of these produce a `bool` in `R[a]`. `EQ`/`NE` never raise; the ordering comparisons raise
`TypeError` for an unordered pair.

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x28` | `EQ` | dst | lhs | rhs | `R[a] = R[b] == R[c]`. Total: never raises, for any pair of types. |
| `0x29` | `NE` | dst | lhs | rhs | `R[a] = R[b] != R[c]`. Total: never raises. |
| `0x2A` | `LT` | dst | lhs | rhs | `R[a] = R[b] < R[c]`. int/int, float/float, str/str (lexicographic by code point), bytes/bytes, list/list (elementwise). int/float compares after promotion. Otherwise `TypeError`. |
| `0x2B` | `LE` | dst | lhs | rhs | `R[a] = R[b] <= R[c]`. As `LT`. |
| `0x2C` | `GT` | dst | lhs | rhs | `R[a] = R[b] > R[c]`. As `LT`. |
| `0x2D` | `GE` | dst | lhs | rhs | `R[a] = R[b] >= R[c]`. As `LT`. |
| `0x2E` | `IS` | dst | lhs | rhs | `R[a] = R[b] is R[c]`. For heap values this is a pointer compare; for int/float/bool/nil it is a value compare. **The only comparison that is meaningful for closures and instances.** |
| `0x2F` | `IDENTICAL` | dst | lhs | rhs | `R[a] = deep_equal(R[b], R[c])`, recursive over containers. |
| `0x30` | `NOT` | dst | src | — | `R[a] = !truthy(R[b])`. |
| `0x31` | `IN` | dst | needle | haystack | `R[a] = R[b] in R[c]`. Works on string, bytes, list, tuple, set, dict (key test). It lives here because it is a *predicate*, not a production. |

### 3.5 Category E — Jumps (6)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x38` | `JMP` | — | rel target (i8) | — | Unconditional jump. The target is **relative to the next instruction**, not absolute, so a code array relocated by a `.zeno` load needs no fixups. |
| `0x39` | `JMP_BACK` | — | rel target (i8, negative) | — | Backward jump; identical semantics to `JMP`. It exists so the interpreter's loop-head arm and the branch predictor's loop detector can recognise a back edge **without inspecting the sign of the target**. Backward branches are the most predictable branch class on any modern predictor, and naming them stops that prediction being destroyed by falling into the generic `JMP` arm. |
| `0x3A` | `JMP_IF_FALSE` | tested reg | rel target | — | `if !truthy(R[a]) { pc += b }`. |
| `0x3B` | `JMP_IF_NIL` | tested reg | rel target | — | `if R[a] is nil { pc += b }`. The optional-chaining (`x?.y`) and null-coalescing (`x ?? y`) primitive. |
| `0x3C` | `JMP_IF_NOT_NIL` | tested reg | rel target | — | `if R[a] is not nil { pc += b }`. |
| `0x3D` | `JUMP_TABLE` | scratch reg | const idx | default target | `idx = R[a] as int`; if `0 <= idx < K[b].len` then `pc += K[b][idx]` else `pc += c`. The table is a constant tuple of relative offsets. Powers `match` and `switch` with dense integer cases. |

### 3.6 Category F — Fused compare-and-branch (4)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x40` | `JMP_IF_LT` | lhs reg | rel target | rhs reg | `if R[a] < R[c] { pc += b }`. One instruction instead of `LT` + `JMP_IF_FALSE`. |
| `0x41` | `JMP_IF_LE` | lhs reg | rel target | rhs reg | `if R[a] <= R[c] { pc += b }`. |
| `0x42` | `JMP_IF_GT` | lhs reg | rel target | rhs reg | `if R[a] > R[c] { pc += b }`. |
| `0x43` | `JMP_IF_GE` | lhs reg | rel target | rhs reg | `if R[a] >= R[c] { pc += b }`. |

`b` sits *between* the two operand registers deliberately: the hot operands `a` and `c` are usually
low-numbered, frequently-touched registers, while the cold jump target should not share their load.

These four cover every `while` and `if` condition, which is where essentially all branch traffic
is. Under Ruling 3 of §3.0 they are core, not superinstructions.

### 3.7 Category G — Calls and returns (11)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x48` | `CALL` | dst reg | proto idx | argc | `R[a] = call(proto[b], R[a-1-c .. a))`. The callee proto is a compile-time constant, so this is a direct call with **no method or symbol lookup at all**. This is the form for every `func` reference. |
| `0x49` | `CALL_KW` | dst reg | proto idx | argc | As `CALL`, plus a keyword dict at `R[a-2-c]` merged into the callee's parameters. |
| `0x4A` | `CALL_INDIRECT` | dst reg | — | argc | `R[a] = call_value(R[a-1-c], R[a-c .. a))`. The callee is a `Value` in the window: a `func`, a `lambda`, a native, or a class constructor. Raises `TypeError` if it is not callable. |
| `0x4B` | `CALL_METHOD` | dst reg | name id | argc | `R[a] = method_call(receiver = R[a-1-c], name = K_names[b], R[a-c .. a))`. This is `s.upper()`. Goes through the M5 inline cache on the name. |
| `0x4C` | `CALL_NATIVE` | dst reg | native idx | argc | `R[a] = natives[b].call(vm, &R[a-c .. a])`. The native index is compile-time and the window layout lets the ABI take a `&[Value]` with no copy (see [`08-vm.md`](08-vm.md) §8). |
| `0x4D` | `CALL_SUPER` | dst reg | name id | argc | `R[a] = super_method(class_of(self), K_names[b], args)`. `self` is register 0. For `__init__` chains. |
| `0x4E` | `TAIL_CALL` | dst reg | proto idx | argc | Identical window to `CALL`; the current frame is **replaced** rather than pushed, so `return f(x)` in tail position is O(1) stack depth. |
| `0x4F` | `TAIL_CALL_METHOD` | dst reg | name id | argc | Tail-call form of `CALL_METHOD`. |
| `0x50` | `RETURN` | value reg | — | — | Return `R[a]`. |
| `0x51` | `RETURN_UNIT` | — | — | — | Return `nil`. Separate from `RETURN r0, r0, nil` because implicit returns are the common case and a one-instruction return should be one instruction. |
| `0x52` | `MAKE_KWARGS` | dst reg | count | first reg | `R[a] = {name_ids[c..c+b-1] paired with R[c..c+b-1]}`. The `**kwargs` object at a call site. |

### 3.8 Category H — Containers and the iteration protocol (15)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x58` | `MAKE_LIST` | dst reg | count | first reg | `R[a] = [R[c .. c+b-1]]`. Preallocates capacity `b`, so no realloc occurs. |
| `0x59` | `MAKE_TUPLE` | dst reg | count | first reg | As `MAKE_LIST`, immutable and `Arc`-shared. |
| `0x5A` | `MAKE_SET` | dst reg | count | first reg | As `MAKE_LIST`, deduplicated on insert. |
| `0x5B` | `MAKE_DICT` | dst reg | pair count | first reg | `R[a] = {R[c]:R[c+1], R[c+2]:R[c+3], ...}`. **Also interns a shape** when every key is a string constant ([`08-vm.md`](08-vm.md) §7). |
| `0x5C` | `MAKE_RANGE` | dst reg | argc (1..3) | first reg | `R[a] = range(R[c], R[c+1][, R[c+2]])`. **Lazy**: a `range` is an iterator state object, never a materialised list. This is the single biggest fix over the old lowering, whose `MakeRange` "pops end, start … and pushes the **materialized** range list" (`src/bytecode.rs:145`). |
| `0x5D` | `LIST_APPEND` | list reg | value reg | — | Append to the list in `R[a]`. Distinct from `R[b].append(R[c])` because the compiler knows this list is a freshly built literal. |
| `0x5E` | `LIST_INSERT` | list reg | index reg | value reg | Insert at index. |
| `0x5F` | `DICT_SET` | dict reg | key reg | value reg | Insert or overwrite. Records the shape transition on the IC site. |
| `0x60` | `SET_ADD` | set reg | value reg | — | Insert into a set. |
| `0x61` | `LEN` | dst reg | src reg | — | `R[a] = len(R[b])`. string, bytes, list, tuple, set, dict. Folded when `R[b]` is a folded constant. |
| `0x62` | `SLICE` | dst reg | src reg | argc (0..3) | `R[a] = R[b][R[a-c .. a]]`. |
| `0x63` | `UNPACK` | dst reg | count | first reg | Destructuring: `R[a .. a+b-1] = R[c]`, iterated, with `UnpackError` on arity mismatch. |
| `0x64` | `ITER_INIT` | iter reg | seq reg | — | Initialise the **hidden 2-slot iterator state** at `R[a]`, `R[a+1]`. See §7.2. |
| `0x65` | `ITER_NEXT` | iter reg | dst reg | — | Advance; `R[b]` = next item, or `nil` with the iterator marked exhausted. |
| `0x66` | `ITER_DONE` | iter reg | dst reg | — | `R[b] = iterator_exhausted(R[a])`. See §7.2 for why this is not fused into `ITER_NEXT`. |

### 3.9 Category I — Field and index access (8)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x6C` | `GET_INDEX` | dst reg | container reg | key reg | `R[a] = R[b][R[c]]`. Integer index on list/tuple/string/bytes, any hashable key on dict/set, integer key into an instance's field vector. **The single most important opcode for the benchmark gate** and the primary inline-cache site. |
| `0x6D` | `SET_INDEX` | container reg | key reg | value reg | `R[a][R[b]] = R[c]`. Evaluation order is fixed: container, then key, then value (§6.5). |
| `0x6E` | `GET_FIELD` | dst reg | obj reg | name id | `R[a] = R[b].K_names[c]`. The dot form. Also an inline-cache site for shaped objects. |
| `0x6F` | `SET_FIELD` | obj reg | name id | value reg | `R[a].K_names[b] = R[c]`. |
| `0x70` | `GET_INDEX_DEFAULT` | dst reg | container reg | key reg | `if R[b] has R[c] then R[a] = R[b][c]`, else `R[a]` is **left unchanged**. The default is pre-loaded into the destination register by the compiler, which is why a 4-operand form is not needed: `d.get(k, 0)` is `LOAD_SMALL_INT %a, 0; GET_INDEX_DEFAULT %a, %d, %k`. This is why `dict.get` is two instructions and not a `try` block. |
| `0x71` | `DELETE_INDEX` | container reg | key reg | — | `del R[a][R[b]]`. Raises `KeyError` / `IndexError` if absent. |
| `0x72` | `HAS_FIELD` | dst reg | obj reg | name id | `R[a] = K_names[c] in R[b]`. |
| `0x73` | `TYPEOF` | dst reg | src reg | — | `R[a] = typeof_id(R[b])` as an interned string **id** (an integer in the register, not a `Value`; the string is resolved only for display). Note this returns the **honest** type per [ADR-0004](decisions/ADR-0004-honest-types.md) — a `handle` reports `"handle"`, never `"int"`. |

### 3.10 Category J — Closures and upvalues (5)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x78` | `MAKE_CLOSURE` | dst reg | proto idx | upvalue count | `R[a] = Closure { proto: b, upvalues: resolved from R[a+1 .. a+1+c] }` using `upvalue_descriptors` (§9). May **alias** `R[a]` with a source register: the VM reads all `c` sources into a 4-entry stack buffer *before* writing `R[a]`, so the aliasing is safe and costs no extra instruction. |
| `0x79` | `GET_UPVAL` | dst reg | upvalue idx | — | `R[a] = upvalue[b].load()`. Handles both open (points into the register file) and closed (points at a box) upvalues; see §9.3. |
| `0x7A` | `SET_UPVAL` | upvalue idx | value reg | — | `upvalue[a].store(R[b])`. |
| `0x7B` | `OPEN_UPVALUE` | local reg | upvalue slot | — | Convert a *local register* into an open upvalue so a later `MAKE_CLOSURE` can reference it in place. The slot is **pinned** in the allocator from here to the matching `CLOSE_UPVALUE` (§9.2). |
| `0x7C` | `CLOSE_UPVALUE` | — | first local reg | count | Box the values of `R[b .. b+c-1]` and repoint every open upvalue referring to them. Emitted at scope exit. |

### 3.11 Category S — Superinstructions (6, **feature-gated**)

Not built unless `zen-superins` is enabled, and the gate is **off by default**. See §11.4 for why,
and for the conflict with the M11 translator.

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x80` | `INC_DEC` | slot | delta (i8) | — | `R[a] += sext(b)`. |
| `0x81` | `COMPARE_IMM` | lhs reg | rel target | imm (i8) | `if R[a] < sext(c) { pc += b }`. |
| `0x82` | `LOAD_CONST_ADD` | dst reg | const idx | — | `R[a] = R[a] + K[b]`. The `total = total + 1` accumulator pattern. |
| `0x83` | `CONCAT3` | dst reg | first reg | count | `R[a] = R[b] ++ R[b+1] ++ … ++ R[b+c-1]`. Multi-line string building; folded further when the parts are all constants. |
| `0x84` | `JUMP_OVER` | — | rel target | — | Unconditional jump, semantically identical to `JMP`. Exists to pad short forward jumps so a long jump cannot straddle a 32-byte instruction-cache line. A pure alignment pass, not a semantic change. |
| `0x85` | `ROT` | base reg | r1 | r2 | Rotate three consecutive registers, for a postfix-expression stack discipline. |

### 3.12 Category K — Errors and exceptions (8)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0x90` | `TRY_PUSH` | — | handler pc (u16) | finally pc (u16) | Push a handler entry covering `[pc, try_end)`; `c = 0` means "no finally". `try_end` is patched by the compiler after the body is emitted. Happy path: one `Vec` push into a preallocated buffer. |
| `0x91` | `TRY_POP` | — | — | — | Pop the innermost handler. The matching happy-path instruction. |
| `0x92` | `THROW` | value reg | — | — | Raise `R[a]` as a Zen error. Never returns. |
| `0x93` | `RERAISE` | — | — | — | Re-raise the in-flight error after a `finally` block has run, propagating to the next outer handler. |
| `0x94` | `CATCH_MATCH` | dst reg (bool) | class name id | — | `R[a] = thrown_matches_class(K_names[b])`. The typed-`catch` predicate (`catch TypeError as e`), matching the parser's `CatchClause { kind, var, body }` shape at `src/runtime.rs:1430-1435`. |
| `0x95` | `ERROR_NEW` | dst reg | class name id | msg reg | `R[a] = new_error(K_names[b], R[c])`. |
| `0x96` | `ERROR_TYPE` | dst reg | error reg | — | `R[a] = class_name_of(R[b])`. |
| `0x97` | `ERROR_MSG` | dst reg | error reg | — | `R[a] = message_of(R[b])`. Implements the documented behaviour that a typed catch binding a dict error yields the `message` string. |

### 3.13 Category L — JIT and patch-point interface (8)

Emitted unconditionally in M4 even though nothing consumes them until M11. Reason: they are
**anchors**. A function compiled today must be byte-identical when the JIT lands, so a `.zeno` cache
written by M4 is still valid under M11. Emitting them from day one and deleting them later would
change every `pc` and invalidate every cached file.

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0xA0` | `TYPE_GUARD` | src reg | expected type id | deopt id | Interpreter: a no-op (the following arithmetic performs its own type dispatch anyway). JIT: if `typeof(R[a]) != TypeId(b)`, deoptimise to landing pad `c`. **This is the mechanism that makes the unified `ADD` of Ruling 1 correct under a JIT.** |
| `0xA1` | `DEOPT` | deopt id | — | — | Interpreter: unreachable, because `TYPE_GUARD` cannot fail in the interpreter. JIT: the landing-pad marker. Present so deopt targets live in the same address space as code. |
| `0xA2` | `SAFEPOINT` | — | reason name id | — | A point at which the VM may be paused by a debugger, a signal handler, or the M11 hotswap. Interpreter: checks a `bool`; a predictable branch on a cold cache line. JIT: a landing pad. |
| `0xA3` | `IC_GET_FIELD` | dst reg | IC site idx | name id | `R[a] = R[dst].K_names[c]`, cache handle at `P.ic_sites[b]`. Semantically identical to `GET_FIELD`; a separate opcode so the M5 patcher can find and rewrite it by ID without re-analysing the code array. |
| `0xA4` | `IC_SET_FIELD` | obj reg | IC site idx | value reg | As `SET_FIELD`, plus an IC site. |
| `0xA5` | `IC_CALL` | dst reg | IC site idx | argc | Call with an inline cache on callee identity. Semantically `CALL_INDIRECT`. |
| `0xA6` | `PATCH_POINT` | — | IC site idx | kind | A no-op in the interpreter. Marks a site the M5/M11 patcher may rewrite in place; `kind` ∈ {`field-get`, `field-set`, `call-target`}. |
| `0xA7` | `IC_BUMP` | — | IC site idx | — | Mark a site as having exceeded its monomorphic budget. Emitted by the patcher, not the compiler; present so the transition is visible in `--disasm`. |

**Where the compiler emits them:**

- `TYPE_GUARD` before every arithmetic/bitwise/comparison instruction whose operand types the
  compiler has inferred consistently on one path. The inference is deliberately weak (one
  type-assignment pass, no fixed-point iteration) because **a wrong guard can only be a
  deoptimisation, never a miscompile** — the guard compares the *runtime* type against the
  *inferred* one.
- `SAFEPOINT` at the entry of every function and at every loop back-edge.
- `IC_GET_FIELD` / `IC_SET_FIELD` in place of `GET_FIELD` / `SET_FIELD` whenever the field name is
  a string constant, which is every `d.a` in the source.
- `IC_CALL` in place of `CALL_INDIRECT` when the callee expression is a local or a field read.

### 3.14 Category M — Modules and globals (4)

| ID | Name | `a` | `b` | `c` | Description |
|---|---|---|---|---|---|
| `0xB0` | `IMPORT` | dst reg | module name id | flags | `R[a] = import(K_names[b])`. `c` bit 0 = bind all names into the current scope (`from m import *`). |
| `0xB1` | `IMPORT_FROM` | dst reg | module name id | symbol name id | `R[a] = import(K_names[b]).K_names[c]`. |
| `0xB2` | `DEFINE_GLOBAL` | value reg | name id | flags | `globals()[K_names[b]] = R[a]`. `c` bit 0 = `const` binding; a later assignment raises `NameError`. |
| `0xB3` | `BIND_NAME` | value reg | name id | flags | Bind in the *current* scope. `c` bits 0–1 select `let` / `var` / `const`; bit 2 = export. The only place any `c` is a bitfield rather than an index, and it is fixed at compile time. |

### 3.15 Reserved IDs

Allocated and **never used in M4**, so a future format version can add them without renumbering and
without invalidating cached bytecode.

| ID | Reserved for | Milestone |
|---|---|---|
| `0x07` | `LOAD_CONST_W` — 16-bit constant index (§2.4) | M9 |
| `0x08` | `LOAD_LOCAL_FRAME` — index a per-frame spill array, lifting the 250-register limit (§2.3) | M9 |
| `0x09` | `LOAD_UPVALUE_FRAME` — index a spill array for a deep capture chain | M9 |
| `0x0A` | `YIELD_FROM` — coroutine suspension point | M10, only if the channel model needs it; [ADR-0003](decisions/ADR-0003-concurrency-semantics.md) suggests it does not |
| `0x0B` | `SPAWN` — `spawn f(x)`, distinct from `CALL` so the M10 scheduler can intercept it | M10 |

### 3.16 Summary

| Category | Core count | Range |
|---|---|---|
| A — Constants and immediates | 7 | `0x00`–`0x06` |
| B — Moves and conversions | 4 | `0x10`–`0x13` |
| C — Arithmetic | 14 | `0x18`–`0x25` |
| D — Comparison | 10 | `0x28`–`0x31` |
| E — Jumps | 6 | `0x38`–`0x3D` |
| F — Fused compare-and-branch | 4 | `0x40`–`0x43` |
| G — Calls and returns | 11 | `0x48`–`0x52` |
| H — Containers and iteration | 15 | `0x58`–`0x66` |
| I — Field and index access | 8 | `0x6C`–`0x73` |
| J — Closures and upvalues | 5 | `0x78`–`0x7C` |
| K — Errors and exceptions | 8 | `0x90`–`0x97` |
| L — JIT interface | 8 | `0xA0`–`0xA7` |
| M — Modules and globals | 4 | `0xB0`–`0xB3` |
| **Core total** | **104** | |
| S — Superinstructions (gated, §11.4) | 6 | `0x80`–`0x85` |
| **Enum total** | **110** | |
| Reserved | 5 | `0x07`–`0x0B` |

The old module had **78** (`src/bytecode.rs:8`, counted from the explicit table in
[`src/cache.rs:25-106`](../../src/cache.rs)). The increase is not bloat: 27 of the 78 were
arithmetic-on-a-specific-slot variants that a register machine makes unnecessary, and the 32 new
ones are the iteration protocol, exception unwind, tail calls, shape/inline-cache sites, and the JIT
anchors — none of which the old module had at all.

---

## 4. `Instruction` and disassembly

```rust
//! zen-vm/src/instruction.rs
//!
//! The compiled-program representation. Consumed by the M4 VM (08-vm.md), by
//! the M11 Cranelift translator, and by `zen check --disasm`.

/// A single instruction. **Exactly 4 bytes, no padding.**
///
/// `#[repr(C)]` so the on-disk `.zeno` image and the in-memory array are
/// byte-identical (§10).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Instruction {
    /// Opcode. See §3.
    pub op: Op,
    /// First operand: destination register in most opcodes.
    pub a: u8,
    /// Second operand: source register, constant index, name id, or jump target.
    pub b: u8,
    /// Third operand: second source register, or a count.
    pub c: u8,
}

impl Instruction {
    /// The canonical constructor. All compiler code paths go through this, so
    /// the size assertion runs exactly once per emitted instruction.
    #[inline]
    pub fn new(op: Op, a: u8, b: u8, c: u8) -> Self {
        debug_assert_eq!(
            std::mem::size_of::<Instruction>(),
            4,
            "Instruction must stay 4 bytes: the dispatch loop, the .zeno CRC \
             and the JIT's fetch all depend on it"
        );
        Instruction { op, a, b, c }
    }

    /// Reinterpret a raw 4-byte word. Used by the `.zeno` loader and by the
    /// M11 hotswap path, which may read a memory-mapped code array.
    #[inline]
    pub fn from_bits(bits: u32) -> Self {
        Instruction {
            op: Op::from_u8((bits & 0xFF) as u8),
            a: ((bits >> 8) & 0xFF) as u8,
            b: ((bits >> 16) & 0xFF) as u8,
            c: ((bits >> 24) & 0xFF) as u8,
        }
    }

    /// The 32-bit form, for the serialiser.
    #[inline]
    pub fn to_bits(self) -> u32 {
        (self.op as u32) | ((self.a as u32) << 8)
            | ((self.b as u32) << 16) | ((self.c as u32) << 24)
    }
}

```
### 4.2 The validator

The function below is the single most safety-critical piece of code in this document, and it is
short enough to read in one sitting — which is the point. It is grouped by **which operand field
is a register**, not by opcode category, because that is the only grouping in which a missing case
is visible: a category grouping hides `SET_UPVAL` next to `GET_UPVAL` and lets the mirror-image
operand order go unnoticed.

Three consequences of that grouping are worth calling out, because each one was a bug in an earlier
draft of this document and each is exactly the kind of thing a category grouping hides:

- `SET_FIELD`'s `b` is a **name id**, not a register. Checking it against `frame_size` is a *false
  rejection* — a name id of 40 is legal in a 30-slot frame.
- `SET_UPVAL`'s `a` is an **upvalue slot**, not a register, and `b` is the value register. The
  mirror of `GET_UPVAL`. Checking `a` as a register would pass while writing outside the closure's
  upvalue array.
- `THROW`'s `a` **is** a register. Having it in the "no operands" group is harmless at run time
  (the value is read from a register the compiler picked) and wrong as documentation, and it would
  hide a real bug the moment `THROW` gained a second operand.

```rust
/// Compile-time validation of one emitted instruction.
///
/// This is what makes "the compiler emitted an out-of-range register index" a
/// `CompileError` rather than an out-of-bounds read in the VM. It is also
/// what makes the VM's `get_unchecked` sound: **the same validator runs
/// again at `.zeno` load time**, over the whole image, before a single
/// instruction executes ([`08-vm.md`](08-vm.md) §3.2, §4.2). That second
/// pass is not redundant — bytecode can also arrive from a file or a native
/// module, and it is the only thing standing between a corrupted image and a
/// wrong answer rather than an error.
pub fn validate(inst: &Instruction, proto: &FunctionProto) -> CompileResult<()> {
    use Op::*;

    let span = proto.current_span();

    match inst.op {
        // ── no register operands at all ───────────────────────────────────
        NOP | RETURN_UNIT | TRY_POP | RERAISE | SAFEPOINT | PATCH_POINT
        | JMP | JMP_BACK | JUMP_OVER | DEOPT => {}

        // ── exactly one register operand: `a` ─────────────────────────────
        // The other two fields are immediates: a constant index, a name id, a
        // type id, a delta, a bitfield, or a patch target. Grouping these by
        // *which field is a register* rather than by category is the whole
        // design of this function: it is the only way to be sure that no
        // opcode is validated against the wrong operand shape.
        LOAD_CONST | LOAD_NIL | LOAD_TRUE | LOAD_FALSE | LOAD_SMALL_INT
        | LOAD_DEFAULT | CATCH_MATCH | GET_UPVAL | OPEN_UPVALUE | COMPARE_IMM
        | INC_DEC | TYPE_GUARD | THROW | JUMP_TABLE | DEFINE_GLOBAL
        | BIND_NAME | IMPORT | IMPORT_FROM | LOAD_CONST_ADD => {
            check_reg(inst.a, proto, span)?
        }

        // `b` of these is a constant-pool index, and it is the one operand
        // that can point past the end of a buffer.
        LOAD_CONST | LOAD_SMALL_INT | LOAD_DEFAULT | JUMP_TABLE | LOAD_CONST_ADD => {
            if usize::from(inst.b) >= proto.constants.len() {
                return Err(CompileError::ConstantIndexOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    index: inst.b,
                    len: proto.constants.len(),
                });
            }
        }

        // `b` of TYPE_GUARD must be a real type id, or the M11 translator would
        // guard on garbage. See 08-vm.md §11.2, test 14. `c` of CONV is the
        // same kind of operand for the same reason.
        TYPE_GUARD => {
            if usize::from(inst.b) >= proto.type_count() {
                return Err(CompileError::TypeIdOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    type_id: inst.b,
                });
            }
        }
        CONV => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.b, proto, span)?;
            if usize::from(inst.c) >= proto.type_count() {
                return Err(CompileError::TypeIdOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    type_id: inst.c,
                });
            }
        }

        // SET_UPVAL is the mirror image of GET_UPVAL: `a` is the upvalue
        // *slot* and `b` is the value register. Validating `a` as a register
        // here would be a false rejection, and failing to validate it as a
        // slot would let the VM write outside the closure's upvalue array.
        SET_UPVAL => {
            if usize::from(inst.a) >= usize::from(proto.num_upvalues) {
                return Err(CompileError::UpvalueIndexOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    index: inst.a,
                    len: proto.num_upvalues,
                });
            }
            check_reg(inst.b, proto, span)?;
        }

        // ── two registers: `a` and `b` ─────────────────────────────────────
        MOVE | DUP | TO_BOOL | NOT | NEG | BIT_NOT | TYPEOF | LEN
        | ITER_INIT | ITER_NEXT | ITER_DONE | LIST_APPEND | SET_ADD
        | DELETE_INDEX | ERROR_TYPE | ERROR_MSG => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.b, proto, span)?;
        }

        // `a` and `b` are registers, but `c` is a **name id**. Validating `c`
        // as a register would be a false rejection: a name id of 40 is legal
        // in a 30-slot frame.
        GET_FIELD | HAS_FIELD | IC_GET_FIELD => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.b, proto, span)?;
            check_name(inst.c, proto, span)?;
        }
        // `b` of IC_GET_FIELD is an IC site index instead of a name id.
        IC_GET_FIELD => {
            if usize::from(inst.b) >= proto.ic_sites.len() {
                return Err(CompileError::IcSiteOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    index: inst.b,
                    len: proto.ic_sites.len(),
                });
            }
        }

        // `a` and `c` are registers; `b` is a name id (SET_FIELD,
        // ERROR_NEW) or an IC site (IC_SET_FIELD).
        SET_FIELD | IC_SET_FIELD | ERROR_NEW => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.c, proto, span)?;
        }
        SET_FIELD | ERROR_NEW => check_name(inst.b, proto, span)?,
        IC_SET_FIELD => {
            if usize::from(inst.b) >= proto.ic_sites.len() {
                return Err(CompileError::IcSiteOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                    index: inst.b,
                    len: proto.ic_sites.len(),
                });
            }
        }

        // ── three registers: `a`, `b`, `c` ─────────────────────────────────
        ADD | SUB | MUL | DIV | MOD | POW | BIT_AND | BIT_OR | BIT_XOR
        | SHL | SHR | CONCAT | EQ | NE | LT | LE | GT | GE | IS | IDENTICAL
        | IN | GET_INDEX | GET_INDEX_DEFAULT | SET_INDEX | LIST_INSERT
        | DICT_SET | ROT => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.b, proto, span)?;
            check_reg(inst.c, proto, span)?;
        }

        // SLICE is `R[a] = R[b][R[a-c] .. R[a]]`: three *values* but only two
        // register indices, because `c` is an argument count.
        SLICE => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.b, proto, span)?;
        }

        // ── source windows: `a` = dst, `b` = count, `c` = first source ─────
        MAKE_LIST | MAKE_TUPLE | MAKE_SET | MAKE_DICT | MAKE_RANGE
        | MAKE_KWARGS | UNPACK | CONCAT3 => {
            check_reg(inst.a, proto, span)?;
            // `c + width*b` is the last source register touched. MAKE_DICT
            // takes 2 registers per entry (key, value); everything else 1.
            let width = if inst.op == MAKE_DICT { 2 } else { 1 };
            let last = usize::from(inst.c) + width * usize::from(inst.b);
            if last > proto.frame_size() {
                return Err(CompileError::WindowOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                });
            }
        }

        // ── call window: `a` = dst, `b` = callee, `c` = argc ───────────────
        // The window the callee occupies starts at `a - 1 - argc`; a method's
        // receiver occupies `a - 1`. Both must be inside the frame, and this
        // is the check that keeps the VM's `get_unchecked` sound for *nested*
        // frames (08-vm.md §5.1, rule 4).
        CALL | CALL_KW | CALL_INDIRECT | CALL_METHOD | CALL_NATIVE | CALL_SUPER
        | TAIL_CALL | TAIL_CALL_METHOD | IC_CALL => {
            check_reg(inst.a, proto, span)?;
            let need = match inst.op {
                CALL_KW => usize::from(inst.a).checked_sub(2 + usize::from(inst.c)),
                _ => usize::from(inst.a).checked_sub(1 + usize::from(inst.c)),
            };
            if need.is_none_or(|n| n > proto.frame_size()) {
                return Err(CompileError::WindowUnderflow {
                    function: proto.qualified_name.clone(),
                    span,
                });
            }
        }

        // MAKE_CLOSURE needs the destination check *and* a check that the
        // upvalue source window `R[c .. c+b-1]` is inside the frame.
        MAKE_CLOSURE => {
            check_reg(inst.a, proto, span)?;
            let last = usize::from(inst.c) + usize::from(inst.b);
            if last > proto.frame_size() {
                return Err(CompileError::WindowOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                });
            }
        }

        // ── `a` is a register, `b` is a jump target ────────────────────────
        JMP_IF_FALSE | JMP_IF_NIL | JMP_IF_NOT_NIL => check_reg(inst.a, proto, span)?,
        // Fused compare-branches: `a` and `c` are the operands and `b` is
        // the target. The asymmetry with the arms above is what lets the
        // peephole collapse a separate `TYPE_GUARD` + compare pair
        // (07-bytecode.md §11.2).
        JMP_IF_LT | JMP_IF_LE | JMP_IF_GT | JMP_IF_GE => {
            check_reg(inst.a, proto, span)?;
            check_reg(inst.c, proto, span)?;
        }

        // ── no registers, but patch targets in `b`/`c` ─────────────────────
        // TRY_PUSH's handler and finally pcs are u16 immediates patched after
        // the body is emitted; they are checked by `validate_jump_targets`
        // once every pc is final. IC_BUMP's `b` is a site index that the
        // patcher, not the compiler, writes.
        TRY_PUSH | IC_BUMP => {}

        // `a` is unused; `b` is the first pinned local, `c` the count.
        CLOSE_UPVALUE => {
            let last = usize::from(inst.b) + usize::from(inst.c);
            if last > proto.frame_size() {
                return Err(CompileError::WindowOutOfRange {
                    function: proto.qualified_name.clone(),
                    span,
                });
            }
        }

        RETURN => check_reg(inst.a, proto, span)?,
    }
    Ok(())
}

fn check_reg(r: u8, proto: &FunctionProto, span: Span) -> CompileResult<()> {
    if usize::from(r) >= proto.frame_size() {
        return Err(CompileError::RegisterOutOfRange {
            function: proto.qualified_name.clone(),
            span,
            register: r,
            frame_size: proto.frame_size(),
        });
    }
    Ok(())
}

/// Name ids index the **module's** name interner, which every proto in the
/// module shares. Validating them here is what stops a `.zeno` file from
/// sending the VM to name 4 000 of a 12-name table.
fn check_name(id: u8, proto: &FunctionProto, span: Span) -> CompileResult<()> {
    if usize::from(id) >= proto.name_count() {
        return Err(CompileError::NameIndexOutOfRange {
            function: proto.qualified_name.clone(),
            span,
            index: id,
            len: proto.name_count(),
        });
    }
    Ok(())
}

```

### 4.1 The disassembler

`Display` is **hand-written**, not derived, so the output is stable, diffable and golden-testable. A
derived `Debug` would print `Op::Add` and change the moment the enum is renamed; `--disasm` output is
in the test corpus and in bug reports, so it is a public interface. `Debug` *is* derived, so `dbg!`
and `assert_eq!` work in tests, but derived formatting is never part of the `--disasm` contract.

```rust
use std::fmt;

impl fmt::Display for Instruction {
    /// Renders one instruction in Zen's canonical assembly syntax.
    ///
    /// Operands naming registers render as `%r<N>`, constant-pool operands as
    /// `$K<N>`, interned names as `$N<id>`, and jump targets as a bare
    /// `-> 0x0031`. Jump targets are pc-relative, so a disassembler must add
    /// `pc + 1` when it knows the pc; `disassemble` rewrites them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Op::*;
        let r = |n: u8| write!(f, "%r{n}");
        match self.op {
            NOP            => write!(f, "nop"),
            LOAD_CONST     => write!(f, "load_const {}, $K{}", r(self.a), self.b),
            LOAD_NIL       => write!(f, "load_nil {}", r(self.a)),
            LOAD_TRUE      => write!(f, "load_true {}", r(self.a)),
            LOAD_FALSE     => write!(f, "load_false {}", r(self.a)),
            LOAD_SMALL_INT => write!(f, "load_int {}, {}", r(self.a), self.b as i8),
            LOAD_DEFAULT   => write!(f, "load_default {}, #{}", r(self.a), self.b),

            MOVE    => write!(f, "move {}, {}", r(self.a), r(self.b)),
            DUP     => write!(f, "dup {}, {}", r(self.a), r(self.b)),
            CONV    => write!(f, "conv {}, {}, {}", r(self.a), r(self.b), self.c),
            TO_BOOL => write!(f, "to_bool {}, {}", r(self.a), r(self.b)),

            ADD     => write!(f, "add {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            SUB     => write!(f, "sub {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            MUL     => write!(f, "mul {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            DIV     => write!(f, "div {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            MOD     => write!(f, "mod {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            POW     => write!(f, "pow {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            NEG     => write!(f, "neg {}, {}", r(self.a), r(self.b)),
            BIT_AND => write!(f, "bit_and {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            BIT_OR  => write!(f, "bit_or {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            BIT_XOR => write!(f, "bit_xor {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            BIT_NOT => write!(f, "bit_not {}, {}", r(self.a), r(self.b)),
            SHL     => write!(f, "shl {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            SHR     => write!(f, "shr {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            CONCAT  => write!(f, "concat {}, {}, {}", r(self.a), r(self.b), r(self.c)),

            EQ        => write!(f, "eq {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            NE        => write!(f, "ne {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            LT        => write!(f, "lt {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            LE        => write!(f, "le {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            GT        => write!(f, "gt {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            GE        => write!(f, "ge {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            IS        => write!(f, "is {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            IDENTICAL => write!(f, "identical {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            NOT       => write!(f, "not {}, {}", r(self.a), r(self.b)),
            IN        => write!(f, "in {}, {}, {}", r(self.a), r(self.b), r(self.c)),

            JMP            => write!(f, "jmp"),
            JMP_BACK       => write!(f, "jmp_back"),
            JMP_IF_FALSE   => write!(f, "jmp_if_false {}, +{}", r(self.a), self.b as i8),
            JMP_IF_NIL     => write!(f, "jmp_if_nil {}, +{}", r(self.a), self.b as i8),
            JMP_IF_NOT_NIL => write!(f, "jmp_if_not_nil {}, +{}", r(self.a), self.b as i8),
            JUMP_TABLE     => write!(f, "jump_table {}, $K{}, default +{}",
                                      r(self.a), self.b, self.c as i8),
            JMP_IF_LT      => write!(f, "jmp_if_lt {}, {} +{}", r(self.a), r(self.c), self.b as i8),
            JMP_IF_LE      => write!(f, "jmp_if_le {}, {} +{}", r(self.a), r(self.c), self.b as i8),
            JMP_IF_GT      => write!(f, "jmp_if_gt {}, {} +{}", r(self.a), r(self.c), self.b as i8),
            JMP_IF_GE      => write!(f, "jmp_if_ge {}, {} +{}", r(self.a), r(self.c), self.b as i8),

            CALL             => write!(f, "call {}, #{}, {}", r(self.a), self.b, self.c),
            CALL_KW          => write!(f, "call_kw {}, #{}, {}", r(self.a), self.b, self.c),
            CALL_INDIRECT    => write!(f, "call_indirect {}, {}", r(self.a), self.c),
            CALL_METHOD      => write!(f, "call_method {}, $N{}, {}", r(self.a), self.b, self.c),
            CALL_NATIVE      => write!(f, "call_native {}, #{}, {}", r(self.a), self.b, self.c),
            CALL_SUPER       => write!(f, "call_super {}, $N{}, {}", r(self.a), self.b, self.c),
            TAIL_CALL        => write!(f, "tail_call {}, #{}, {}", r(self.a), self.b, self.c),
            TAIL_CALL_METHOD => write!(f, "tail_call_method {}, $N{}, {}", r(self.a), self.b, self.c),
            RETURN           => write!(f, "return {}", r(self.a)),
            RETURN_UNIT      => write!(f, "return_unit"),
            MAKE_KWARGS      => write!(f, "make_kwargs {}, %r{}..{}", r(self.a), self.c,
                                        self.c as u16 + self.b as u16),

            MAKE_LIST   => write!(f, "make_list {}, %r{}..{}", r(self.a), self.c,
                                  self.c as u16 + self.b as u16),
            MAKE_TUPLE  => write!(f, "make_tuple {}, %r{}..{}", r(self.a), self.c,
                                  self.c as u16 + self.b as u16),
            MAKE_SET    => write!(f, "make_set {}, %r{}..{}", r(self.a), self.c,
                                  self.c as u16 + self.b as u16),
            MAKE_DICT   => write!(f, "make_dict {}, %r{}..{} ({} pairs)", r(self.a), self.c,
                                  self.c as u16 + 2 * self.b as u16, self.b),
            MAKE_RANGE  => write!(f, "make_range {}, %r{}..{}", r(self.a), self.c,
                                  self.c as u16 + self.b as u16),
            LIST_APPEND => write!(f, "list_append {}, {}", r(self.a), r(self.b)),
            LIST_INSERT => write!(f, "list_insert {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            DICT_SET    => write!(f, "dict_set {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            SET_ADD     => write!(f, "set_add {}, {}", r(self.a), r(self.b)),
            LEN         => write!(f, "len {}, {}", r(self.a), r(self.b)),
            SLICE       => write!(f, "slice {}, {}, {}", r(self.a), r(self.b), self.c),
            UNPACK      => write!(f, "unpack {}, %r{}..{}", r(self.a), self.c,
                                  self.c as u16 + self.b as u16),
            ITER_INIT   => write!(f, "iter_init {} [state=%r{}, cursor=%r{}] <- {}",
                                  r(self.a), self.a, self.a as u16 + 1, r(self.b)),
            ITER_NEXT   => write!(f, "iter_next {}, {}", r(self.a), r(self.b)),
            ITER_DONE   => write!(f, "iter_done {}, {}", r(self.a), r(self.b)),

            GET_INDEX         => write!(f, "get_index {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            SET_INDEX         => write!(f, "set_index {}, {}, {}", r(self.a), r(self.b), r(self.c)),
            GET_FIELD         => write!(f, "get_field {}, {}, $N{}", r(self.a), r(self.b), self.c),
            SET_FIELD         => write!(f, "set_field {}, $N{}, {}", r(self.a), self.b, r(self.c)),
            GET_INDEX_DEFAULT => write!(f, "get_index_default {}, {}, {}",
                                         r(self.a), r(self.b), r(self.c)),
            DELETE_INDEX      => write!(f, "delete_index {}, {}", r(self.a), r(self.b)),
            HAS_FIELD         => write!(f, "has_field {}, {}, $N{}", r(self.a), r(self.b), self.c),
            TYPEOF            => write!(f, "typeof {}, {}", r(self.a), r(self.b)),

            MAKE_CLOSURE => write!(f, "make_closure {}, #{}, {} upvalues from %r{}..{}",
                                   r(self.a), self.b, self.c, self.a as u16 + 1,
                                   self.a as u16 + 1 + self.c as u16),
            GET_UPVAL     => write!(f, "get_upval {}, #{}", r(self.a), self.b),
            SET_UPVAL     => write!(f, "set_upval #{}, {}", self.a, r(self.b)),
            OPEN_UPVALUE  => write!(f, "open_upvalue {}, #{}", r(self.a), self.b),
            CLOSE_UPVALUE => write!(f, "close_upvalue %r{}..{} ({} slots)", self.b,
                                   self.b as u16 + self.c as u16, self.c),

            INC_DEC        => write!(f, "{} %r{} by {}", if (self.b as i8) < 0 { "dec" } else { "inc" },
                                      r(self.a), self.b as i8),
            COMPARE_IMM    => write!(f, "cmp_imm %r{}, {} +{}", r(self.a), self.c as i8, self.b as i8),
            LOAD_CONST_ADD => write!(f, "load_const_add {}, $K{}", r(self.a), self.b),
            CONCAT3        => write!(f, "concat3 {}, %r{}..{}", r(self.a), self.b,
                                      self.b as u16 + self.c as u16),
            JUMP_OVER      => write!(f, "jmp_over"),
            ROT            => write!(f, "rot %r{}, %r{}, %r{}", r(self.a), r(self.b), r(self.c)),

            TRY_PUSH    => write!(f, "try_push handler={} finally={}", self.b, self.c),
            TRY_POP     => write!(f, "try_pop"),
            THROW       => write!(f, "throw {}", r(self.a)),
            RERAISE     => write!(f, "reraise"),
            CATCH_MATCH => write!(f, "catch_match {}, $N{}", r(self.a), self.b),
            ERROR_NEW   => write!(f, "error_new {}, $N{}, {}", r(self.a), self.b, r(self.c)),
            ERROR_TYPE  => write!(f, "error_type {}, {}", r(self.a), r(self.b)),
            ERROR_MSG   => write!(f, "error_msg {}, {}", r(self.a), r(self.b)),

            TYPE_GUARD   => write!(f, "type_guard {}, {}, [deopt #{}]", r(self.a), self.b, self.c),
            DEOPT        => write!(f, "deopt #{}", self.a),
            SAFEPOINT    => write!(f, "safepoint $N{}", self.b),
            IC_GET_FIELD => write!(f, "ic_get_field {}, #{}, $N{}", r(self.a), self.b, self.c),
            IC_SET_FIELD => write!(f, "ic_set_field {}, #{}, {}", r(self.a), self.b, r(self.c)),
            IC_CALL      => write!(f, "ic_call {}, #{}, {}", r(self.a), self.b, self.c),
            PATCH_POINT  => write!(f, "patch_point #{} kind={}", self.b, self.c),
            IC_BUMP      => write!(f, "ic_bump #{}", self.b),

            IMPORT        => write!(f, "import {}, $N{}{}", r(self.a), self.b,
                                    if self.c & 1 != 0 { " *" } else { "" }),
            IMPORT_FROM   => write!(f, "import_from {}, $N{}.{}", r(self.a), self.b, self.c),
            DEFINE_GLOBAL => write!(f, "define_global $N{} {}{}", self.b, r(self.a),
                                    if self.c & 1 != 0 { " (const)" } else { "" }),
            BIND_NAME     => write!(f, "bind_name $N{} {} {}{}", self.b, r(self.a),
                                    ["let", "var", "const"][(self.c & 3) as usize % 3],
                                    if self.c & 4 != 0 { " (exported)" } else { "" }),
        }
    }
}

/// Disassembles one function into the format consumed by `zen check --disasm`
/// and by the golden files in `tests/disasm/`.
pub fn disassemble(proto: &FunctionProto) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(proto.code.len() * 40);
    let params: Vec<&str> = proto.params.iter().map(|p| p.name.as_ref()).collect();
    let _ = writeln!(out, "func {}({}) {{", proto.qualified_name, params.join(", "));
    let _ = writeln!(out,
        "  ; frame={} locals={} upvalues={} code={} consts={} handlers={} ic_sites={}",
        proto.frame_size(), proto.num_locals, proto.num_upvalues, proto.code.len(),
        proto.constants.len(), proto.exception_table.len(), proto.ic_sites.len());
    for (i, slot) in proto.local_table.iter().enumerate() {
        let _ = writeln!(out, "  ; %r{i} = {} {}{}", slot.kind, slot.name,
                         if slot.pinned { "   ; PINNED: open upvalue" } else { "" });
    }
    for (k, v) in proto.constants.iter().enumerate() {
        let _ = writeln!(out, "  ; $K{k} = {}", v.repr_for_disasm());
    }
    for (pc, inst) in proto.code.iter().enumerate() {
        let text = disassemble_one(proto, pc as u32, inst);
        let _ = writeln!(out, "  {pc:04}  {text:<46} ; {}",
                         proto.line_table.line_for_pc(pc as u32));
    }
    for h in &proto.exception_table {
        let _ = writeln!(out,
            "  ; handler 0x{:04x}..0x{:04x} -> 0x{:04x}  finally 0x{:04x}  catch {:?}",
            h.try_start, h.try_end, h.handler_pc, h.finally_pc, h.catch_classes);
    }
    for s in &proto.ic_sites {
        let _ = writeln!(out, "  ; ic_site #{} {:?} name={} slot={} pc=0x{:04x} state={:?}",
                         s.index, s.kind, s.name, s.slot, s.pc, s.state);
    }
    for u in &proto.upvalue_descriptors {
        let _ = writeln!(out, "  ; upvalue #{} {:?} <- {}", u.index, u.kind, u.source);
    }
    out.push_str("}\n\n");
    out
}
```

`disassemble_one` is `Display`'s output with two substitutions: relative jump offsets are
rewritten to absolute `0x…` targets using `pc`, and `$N<id>` placeholders are replaced with the
interned string. Example output for `tests/disasm/upper.zen`, source `print("hello".upper().length())`:

```text
func main() {
  ; frame=5 locals=0 upvalues=0 code=8 consts=1 handlers=0 ic_sites=1
  ; $K0 = "hello"
  0000  load_const         %r0, $K0                            ; 1
  0001  ic_call            %r1, #0, 0                          ; 1
  0002  ic_call            %r2, #1, 0                          ; 1
  0003  len                %r3, %r2                            ; 1
  0004  call_native        %r4, #12, 1                         ; 1
  0005  return             %r4                                ; 1
  ; ic_site #0 CallTarget name=upper   slot=%r0 pc=0x0001 state=Uninitialized
  ; ic_site #1 CallTarget name=length  slot=%r1 pc=0x0002 state=Uninitialized
}
```

---

## 5. `FunctionProto`, constants, and the local/upvalue tables

```rust
//! zen-vm/src/proto.rs

use std::sync::Arc;

/// One compiled function. Immutable and shared: every closure over this
/// function holds an `Arc<FunctionProto>`, so a recursive or multiply-aliased
/// function allocates the proto exactly once.
pub struct FunctionProto {
    /// Short name, e.g. `inc`. Used in `zen check --disasm` headers.
    pub name: Arc<str>,
    /// Fully qualified name, e.g. `Counter.inc`. Used in tracebacks.
    pub qualified_name: Arc<str>,

    /// Parameter names and positions, in declaration order. Parameters occupy
    /// registers 0..params.len().
    pub params: Vec<ParamInfo>,

    /// Total register slots this function owns: params + named locals +
    /// pinned upvalue slots + the temporary free list's high-water mark.
    /// **Never more than 250** (§2.3).
    pub num_locals: u16,

    /// Number of upvalues this function captures from enclosing scopes.
    pub num_upvalues: u8,

    /// The instruction stream. 4 bytes per entry, dense.
    pub code: Vec<Instruction>,

    /// Per-function constant pool (§5.2).
    pub constants: ConstantPool,

    /// pc -> source line, for diagnostics, the REPL and the differential harness.
    pub line_table: LineTable,

    /// pc -> exact source span, for every instruction that can raise (§5.1.1).
    /// This is what the VM's error path reads, so an error underlines the
    /// expression that failed rather than the start of its statement. Read
    /// by binary search on the error path only; zero cost on the happy path.
    pub span_table: SpanTable,

    /// What each upvalue refers to (§9).
    pub upvalue_descriptors: Vec<UpvalueDesc>,

    /// Protected regions and their handlers (§8).
    pub exception_table: Vec<HandlerEntry>,

    /// Inline-cache sites, referenced by `b` of the `IC_*` opcodes (§3.13).
    pub ic_sites: Vec<IcSite>,

    /// Nested protos, as indices into the module's flat proto table.
    /// **Index 0 is always the function itself**, so a nested closure's parent
    /// is `parent - 1` in the flat table and needs no extra pointer.
    pub children: Vec<u32>,

    /// Pre-evaluated default parameter values, index-aligned with `params`.
    /// Populated by the defaults thunk at module load (§10.4).
    pub defaults: Vec<Value>,

    /// Local binding name -> register, for `zen check --disasm`, the REPL's `?`
    /// introspection, and the "did you mean" hint machinery.
    pub local_table: Vec<LocalSlot>,

    pub flags: ProtoFlags,
}

impl FunctionProto {
    /// Registers this frame occupies: locals plus the allocator's high-water
    /// mark, so the VM can bound-check the window with one compare.
    #[inline]
    pub fn frame_size(&self) -> usize {
        self.frame_size as usize
    }
    /// Kept private so it cannot drift from the `num_locals` the VM reserved.
    frame_size: u16,
}

#[derive(Clone, Copy)]
pub struct ParamInfo {
    pub name: Arc<str>,
    pub line: u32,
    /// Has a default (an entry in `defaults`).
    pub optional: bool,
    /// Declares `*args`; consumes remaining positionals into a tuple.
    pub rest: bool,
    /// Declares `**kwargs`.
    pub kwargs: bool,
}

bitflags::bitflags! {
    pub struct ProtoFlags: u32 {
        /// Has a `defaults` thunk that must run at module load.
        const HAS_DEFAULTS     = 1 << 0;
        /// Declares `*args`.
        const HAS_REST         = 1 << 1;
        /// Declares `**kwargs`.
        const HAS_KWARGS       = 1 << 2;
        /// Is a `native` shim, not compiled from Zen source.
        const IS_NATIVE        = 1 << 3;
        /// Compiled by the M4 compiler. Absent means the proto came from a
        /// `.zeno` image, which is how the differential harness labels the
        /// oracle.
        const FROM_M4          = 1 << 4;
    }
}
```

### 5.1 The line table

A sorted `Vec<LineEntry>`, **run-length collapsed**: one entry per *change* of line, not one per
instruction. A 1000-line function compiles to roughly 600 entries = 4.8 KB, and it is read with a
binary search on the error path only.

```rust
#[derive(Clone, Copy)]
pub struct LineEntry {
    /// First instruction of this source line. Strictly increasing.
    pub pc: u32,
    pub line: u32,
    /// 0-based byte column of the first token on the line.
    pub col: u16,
    /// End column, for the REPL's caret underline.
    pub end_col: u16,
}

pub struct LineTable {
    entries: Vec<LineEntry>,
}

impl LineTable {
    /// Source line for `pc`. Called on the cold path (every raised error), so a
    /// binary search is cheaper than the branchless linear scan it replaces.
    #[inline]
    pub fn line_for_pc(&self, pc: u32) -> u32 {
        match self.entries.binary_search_by_key(&pc, |e| e.pc) {
            Ok(i)  => self.entries[i].line,
            Err(0) => self.entries.first().map_or(0, |e| e.line),
            Err(i)  => self.entries[i - 1].line,
        }
    }

    /// `(line, col, end_col)` for `pc`, for the REPL caret and `zen check --trace`.
    #[inline]
    pub fn span_for_pc(&self, pc: u32) -> (u32, u16, u16) {
        let i = self.entries.partition_point(|e| e.pc <= pc).saturating_sub(1);
        self.entries.get(i).map_or((0, 0, 0), |e| (e.line, e.col, e.end_col))
    }
}
```

The line table is **not** a debug-only structure. It is in the `.zeno` file for any artifact built
with debug info (a header flag, §10.2); it is what makes the REPL and `zen check --trace` possible;
and it is what the M3↔M4 differential harness uses to prove that an error from the VM and an error
from the tree-walker came from the *same source line*, not merely the same message.

#### 5.1.1 The span table — exact source spans for errors

The line table answers "which line", which is not good enough. [`06-parser.md`](../docs/06-parser.md)
§3.3.3 rules that **the compiler records, for every instruction that can raise, the `Span` of the
AST node it came from, and the VM's error path pops the span from the frame** — and it justifies
this as "a strict improvement on a bytecode line number, which points at a generated artifact the
user has never seen". The legacy runtime cannot do this: it has `line`/`col` on `Stmt` only
(`src/runtime.rs:1394-1399`), so an error *inside* an expression reports the statement's start. That
is why its messages are so often unlocatable, and `TypeError: int has no member 'lenght'` pointing
at the start of a 3-line statement instead of at `lenght`.

A `Span` is two `u32`s — a byte offset into the source and a length — because the source text is
kept alive by the module handle. A span table is therefore much smaller than a line table: it is
`(pc, span_lo, span_hi)` triples, 12 bytes each, and it is **not** run-length collapsed to *line*
changes but to *span* changes.

```rust
/// One entry per change of source span, not per instruction. A `Span` is a
/// byte offset plus a length, which is why the VM needs no line/column
/// arithmetic to render a caret: the renderer has the source and the
/// (offset, len) pair.
#[derive(Clone, Copy)]
pub struct SpanEntry {
    /// First instruction covered by this span. Strictly increasing.
    pub pc: u32,
    /// Byte offset of the AST node in the module source.
    pub lo: u32,
    /// Byte length. `hi - lo` is never stored, so this is 12 bytes.
    pub len: u32,
}

pub struct SpanTable {
    entries: Vec<SpanEntry>,
}

impl SpanTable {
    /// The span for `pc`. Cold path only — called when an error is raised and
    /// when a frame is captured for a traceback.
    #[inline]
    pub fn span_for_pc(&self, pc: u32) -> Option<Span> {
        let i = self.entries.partition_point(|e| e.pc <= pc);
        self.entries.get(i.wrapping_sub(1))
            .map(|e| Span::new(e.lo, e.len))
    }
}
```

**Only instructions that can raise get an entry.** `MOVE`, `JMP`, `JMP_BACK`, `JUMP_TABLE` and
`NOP` cannot raise, so they are absent from the table and `span_for_pc` for their `pc` returns the
*enclosing* raising instruction's span. That is the correct answer — a traceback frame is attributed
to the expression that was being evaluated — and it roughly halves the table.

Three consequences, all of which are rulings:

1. **The table is bigger than the line table but still small.** For a 1000-line function, ~300
   distinct spans × 12 bytes = 3.6 KB, versus 4.8 KB for the line table. The two tables together are
   ~8 KB per 1000 lines, which is 8 bytes per line of source — irrelevant next to the code array
   (4 bytes per instruction × ~2 instructions per line = 8 bytes per line as well).
2. **The VM does not advance a span cursor per instruction.** Doing so would be a compare and a
   branch on the hottest path in the VM. Instead the error path binary-searches
   ([`08-vm.md`](08-vm.md) §10.4), so the cost is 8 compares on an error and **zero instructions
   on the happy path**. This is the whole design constraint that makes the table viable.
3. **`Span` crosses the crate boundary as two `u32`s, not as a `zen-syntax` type.** `zen-syntax`
   must not be a dependency of `zen-vm`'s error path, or `zenn check` links the whole VM
   ([`06-parser.md`](../docs/06-parser.md) §2.1, point 2). `Span` is a `#[repr(C)]` 8-byte struct
   defined in `zen-core`, and `zen-syntax::Span` is a `#[repr(transparent)]` wrapper over it.

### 5.2 The constant pool

```rust
pub struct ConstantPool {
    values: Vec<Value>,
    /// `ConstKey` (a `u64` hash of the value's bits, or `(tag, interned_id)`)
    /// -> pool index, so identical constants are stored once.
    index: ahash::AHashMap<ConstKey, u32>,
    /// Interned *string values* (dictionary keys, names, messages).
    strings: Arc<str_table::Interner>,
    /// Interned *identifiers* (field names, method names, module names).
    /// Kept separate from `strings` so `GET_FIELD`'s `c` field can be a name
    /// id and the VM never has to unbox a `Value` on a field access.
    names: Arc<str_table::Interner>,
}
```

**What may be a constant.** This is the correctness-critical decision of the document, and it is
where the old cache went wrong.

| Value | May be a constant? | Why |
|---|---|---|
| `nil`, `true`, `false` | **Yes** | Immutable. |
| `int`, `float` | **Yes** | Immutable. |
| `string` | **Yes** | Immutable; `Arc<str>`, so folding `"a" + "b"` costs one allocation at compile time and zero at runtime. |
| `bytes` | **Yes** | Immutable. |
| `tuple` of constants | **Yes** | Immutable by construction — see below. |
| `list` | **No — ever** | See the argument below. |
| `dict` | **No — ever** | Same. |
| `set` | **No — ever** | Same. |
| `func` / `lambda` | **No** | These are `MAKE_CLOSURE` instructions, not constants. A closure captures live state and cannot be shared between two executions of the same function. |
| native function | **No** | A native is a `CALL_NATIVE` operand (a compile-time index), never a pool entry. |
| `error`, `handle`, `instance`, `channel`, `mutex` | **No** | Not expressible as a literal, and not serialisable. |

**A tuple of constants is safe; a list of constants is not, and the difference is mutability, not
sharing.** `Arc<[Value]>` gives a tuple copy-on-write semantics identical to a list, so
`Arc::make_mut` would make a folded list *correct* under [ADR-0002](decisions/ADR-0002-memory-model.md).
We still refuse to fold it, for three reasons:

1. A folded list is allocated once at compile time and then **cloned on the first mutation** by
   `Arc::make_mut`, which is strictly worse than allocating a fresh list on each execution.
2. It would make constant-pool correctness depend on a memory-model detail that a future tracing
   GC (ADR-0002 Phase 2) would change. The constant pool should not be a place where GC soundness
   depends on which phase we are in.
3. `#{a:1}` needs a **shape**, and a folded dict would have to carry its shape in the `.zeno` file
   too — doubling the constant encoding to speed up a case that is already two instructions.

**Constant deduplication.** `add_constant` interns by value; a function mentioning the string
`"error"` nine times stores it once. M4 targets a mean pool size of **24 [T]** and a p99 of
**140 [E]** on the Zen stdlib; the 256-entry cap is a compile error above it.

**Name interning is a separate interner from value interning.** `GET_FIELD` and `CALL_METHOD` carry
a name *id*, not a pool index, so the VM compares a `u32` against a `u32` on the inline-cache hit
path. Funnelling names through the value pool would add a `Value` unbox and a pointer chase to
exactly the operation inline caching exists to make fast ([`08-vm.md`](08-vm.md) §6).

### 5.3 The local-slot table

```rust
#[derive(Clone, Copy)]
pub struct LocalSlot {
    pub name: Arc<str>,
    /// Register index. Dense from 0 in M4.
    pub reg: u8,
    /// Scope depth. Two bindings with the same name in different scopes get
    /// different registers; the shadowing relation is recorded here so the
    /// REPL and the error hints can explain it.
    pub scope: u16,
    pub kind: BindingKind,
    /// **True if this register is the target of an open upvalue** and therefore
    /// may not be reused by the temporary allocator for the rest of the
    /// function. See §9.2.
    pub pinned: bool,
    pub line: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BindingKind { Param, Let, Var, Const, LoopVar, CatchVar }
```

The table is **not** on the execution hot path. It is read by `zen check --disasm`, by the REPL's
`?` introspection, and by the compiler itself when deciding whether a name is captured (§9.1). At
~40 entries × 32 bytes it is under 1.3 KB per function.

---

## 6. The compiler

### 6.1 The struct

```rust
//! zen-vm/src/compile.rs

use crate::proto::{FunctionProto, UpvalueDesc, HandlerEntry, IcSite, LineTable};
use crate::syntax::ast::{Expr, Stmt, Span};

pub struct Compiler {
    // ── output ───────────────────────────────────────────────────────────
    code: Vec<Instruction>,
    constants: ConstantPool,
    line_table: LineTableBuilder,
    exception_table: Vec<HandlerEntry>,
    ic_sites: Vec<IcSite>,
    children: Vec<u32>,
    /// Instructions whose `b` field holds a not-yet-known relative target.
    fixups: Vec<Fixup>,

    // ── register allocation (§6.2) ───────────────────────────────────────
    /// Named bindings currently in scope: name -> (register, scope, pinned).
    scopes: Vec<ScopeFrame>,
    /// Free list of temporary registers. Popped on subexpression completion.
    free_regs: Vec<u8>,
    /// High-water mark, for `TooManyRegisters` and `zen check --stats`.
    high_water: u16,

    // ── control flow ─────────────────────────────────────────────────────
    /// One entry per enclosing loop or `try`. Mirrors the old `LoopCtx`
    /// (src/bytecode.rs:191-195) but nests try state alongside loop state.
    ctrl: Vec<CtrlFrame>,

    // ── module context (inherited by reference, never cloned) ────────────
    module: Rc<ModuleCtx>,
    /// Index of this function in the module's flat proto table.
    proto_index: u32,
    /// Parent function index, for upvalue resolution (§9.1).
    parent: Option<u32>,
    /// Monotonic counter for IC site ids and JIT deopt ids.
    next_site_id: u32,
}

impl Compiler {
    /// Compiles one function body.
    ///
    /// # Errors
    /// Returns `CompileError` for anything the M4 compiler cannot express.
    /// **There is no tree-walk fallback.** If this returns `Err`, the program
    /// does not run; it is a compile error with a source span, like any other.
    /// The old compiler's documented alternative — "fall back to tree-walk
    /// interpretation for the whole function" (src/bytecode.rs:217-219) — is
    /// what produced Zen 2.x's two-engine divergence, and is rejected in
    /// [`00-blueprint-review.md`](00-blueprint-review.md) §1 K4.
    pub fn compile_function(
        module: &ModuleCtx,
        parent: Option<u32>,
        name: &str,
        params: &[Param],
        body: &[Stmt],
    ) -> CompileResult<FunctionProto> {
        /* ... */
    }
}
```

### 6.2 Register allocation: a stack-discipline scan, not a graph-colouring allocator

**Ruling.** Two rules, no interference graph:

- **Locals are fixed.** Every named binding gets a register at its declaration point, numbered
  densely from 0, live until the end of its scope. M4 does not reuse a local's register for a later
  local *in the same scope*, even after the earlier one is provably dead.
- **Temporaries come from a free list.** Compiling a binary expression pops a temporary, compiles
  the left operand, compiles the right operand, emits the fused instruction, and pushes the
  temporaries back. Expression nesting depth **is** the temporary count.

**Why not graph colouring.** A proper allocator (Chaitin/Briggs, or LLVM's) builds interference
edges over the whole CFG and colours them, producing measurably better register use on straight-line
code with many assignments. We are not doing it in M4, for four reasons that are about *risk*:

1. **The win is small on Zen-shaped code.** Colouring's benefit scales with straight-line statement
   count, because a statement's destination register can be reused immediately. Zen's hot functions
   are loops with a handful of live accumulators. Expected gain on the benchmark suite:
   **≤ 4 % [E]**, most of it in `bench_string_build`.
2. **The correctness risk is not small.** A mis-coloured register is a wrong answer, and it is a
   wrong answer that surfaces on one input, in one benchmark, in release mode. Stack discipline is
   correct *by construction*: a register is either on `free_regs` or owned by exactly one live
   subexpression, and that ownership is visible in the recursion.
3. **It is a second parallel analysis with its own test surface.** Every line of it is untested code
   on the path that produces every value the VM computes.
4. **M9's GC wants a register-indexed root map.** Stack discipline gives a trivially-computable
   liveness set (the current free-list depth plus the pinned set); a coloured allocator needs the
   interference graph at every `pc`. Making the collector's job easy is worth more than 4 %.

**What we buy instead**, in ~20 lines: **dead-local reuse at scope exit.** When a scope ends, its
locals' registers return to the free list unless pinned (§9.2). A function with 200 sequential
`if`/`else` blocks that each declare one temporary uses ~5 registers instead of ~200. That is the
common shape of macro-expanded and generated code, and it is where a naive no-reuse rule would
actually bite.

**Escape hatch.** If the M4 profile shows register pressure, M6 adds `--opt-regalloc`: a linear-scan
allocator over live intervals computed by one backward pass. The `Compiler` struct is factored so
the allocator is one field.

### 6.3 `a = b + c` in one instruction

The core of a register compiler: **an expression compiles to a register index, not a stack effect.**

```rust
impl Compiler {
    /// Compiles `expr` and returns the register holding its value.
    ///
    /// This one function is what makes `a = b + c` a single instruction: the
    /// caller never pops or peeks, because there is nothing to pop.
    fn expr(&mut self, e: &Expr) -> CompileResult<u8> {
        match &e.kind {
            ExprKind::Int(i) if *i >= -128 && *i <= 127 => {
                let d = self.alloc_temp();
                self.emit(e.span, Op::LOAD_SMALL_INT, d, *i as u8, 0);
                Ok(d)
            }
            ExprKind::Int(i) => {
                let d = self.alloc_temp();
                let k = self.add_constant(Value::Int(*i))?;
                self.emit(e.span, Op::LOAD_CONST, d, k, 0);
                Ok(d)
            }
            ExprKind::List(items) => {
                // 1. One register for the result.
                let dst = self.alloc_temp();
                // 2. Compile each element into a consecutive window, LEFT TO RIGHT.
                let first = self.alloc_temps(items.len())?;
                for (i, item) in items.iter().enumerate() {
                    let reg = self.expr(item)?;
                    self.move_to(first + i as u8, reg);
                }
                // 3. One instruction, not n pushes and a BUILD_LIST.
                self.emit(e.span, Op::MAKE_LIST, dst, items.len() as u8, first);
                self.free_temps(first, items.len());
                Ok(dst)
            }
            ExprKind::BinOp(op, lhs, rhs) => {
                // Evaluation order is **left operand fully, then right operand.**
                // See §6.5.
                let a = self.expr(lhs)?;
                let b = self.expr(rhs)?;
                let dst = self.alloc_temp();
                self.emit_typed(e.span, op, dst, a, b);
                self.free_temp(a);
                self.free_temp(b);
                Ok(dst)
            }
            _ => self.expr_fallback(e),
        }
    }
}
```

Worked example — `let c = a + b * d`, with `a`, `b`, `d` in registers 0, 1, 2:

```text
0000  type_guard  %r0, 3, [deopt #0]    ; inferred int
0001  type_guard  %r2, 3, [deopt #1]    ; inferred int
0002  mul         %r3, %r1, %r2         ; r3 = b * d
0003  add         %r0, %r0, %r3         ; r0 = a + (b*d)
0004  move        %r4, %r0              ; bind c = r0
```

**Four instructions** (two of them free-standing guards the interpreter skips). The equivalent on
the old stack machine is `LOAD_LOCAL a; LOAD_LOCAL b; LOAD_LOCAL d; MUL; ADD; STORE_LOCAL c` —
**six instructions plus two pushes and two pops**, because `MUL` and `ADD` must pop their operands
(`src/bytecode.rs:32-38`).

The allocator's behaviour is visible: `%r3` is allocated for the `*` subexpression and freed
immediately, and `%r0` is reused as the destination because the statement's value *is* `a`'s old
register. The `MOVE` at `0004` is a single 8-byte copy that the peephole (§11.2) will usually
delete by coalescing it into the binding.

### 6.4 A method call: `s.upper()`

`ExprKind::Method(receiver, name, args)` lowers into the register window of §2.5. The order of
registers in the window *is* the order of evaluation, so the lowering is the ordering.

```text
; s is a local in %r0
0000  ic_call  %r1, #0, 0
;         receiver = R[%r1 - 1 - 0] = R[%r0]      ✔ evaluated first
;         argc     = 0
;         result   -> R[%r1]
;         IC site #0 caches the `upper` lookup on the receiver's shape
```

One instruction for a method call on a receiver already sitting in a register. Concretely, in
`print("hello".upper().length())`:

```text
0000  load_const  %r0, $K0            ; "hello"
0001  ic_call     %r1, #0, 0          ; "hello".upper()  -> %r1
0002  ic_call     %r2, #1, 0          ; %r1.length()      -> %r2
0003  len         %r3, %r2            ; redundant: peel it into the IC target
0004  call_native %r4, #12, 1         ; print(%r3)
0005  return      %r4
```

The `IC_CALL` at `0002` and the `LEN` at `0003` are the shape of every method call in the corpus:
the compiler peels the method's IC target off a builtin function object at compile time where the
method is a known builtin, which is what keeps method dispatch out of the inner loop of
`bench_string_build`. Where the receiver type is unknown, the peephole rewrites
`IC_CALL r2, s, 0; LEN r3, r2` into `LEN r3, r1` — a length query has no reason to go through a
method lookup at all.

### 6.5 Evaluation order, and why it is load-bearing

Zen's evaluation order is **left to right, all operands before the operation**, and the compiler
must preserve it exactly, because Zen expressions have side effects.

The test case from the spec is `a[f()] = g()`: the key is computed and evaluated *before* the
value. Lowering:

```text
; d is %r0
; expr f() -> %r2 ; expr g() -> %r3
0000  dup        %r1, %r0          ; container first
0001  call       %r2, #3, 0        ; f()   -- must run FIRST
0002  call       %r3, #7, 0        ; g()   -- must run SECOND
0003  set_index  %r1, %r2, %r3     ; d[%r2] = %r3
```

The compiler's `expr` function is written so the order is **structural**, not a convention that has
to be remembered: for `BinOp` it evaluates `lhs` completely before calling `self.expr(rhs)`, and for
`Method`/`Call` it evaluates the receiver before the arguments before the callee lookup. A
post-order traversal that reordered children would need a separate "evaluation order" pass; this
one does not.

Three rules, all enforced by tests:

| Construct | Order |
|---|---|
| `a op b op c` (left-assoc) | `a`, then `b`, then `c` |
| `f(a, b)` | the callee expression, then `a`, then `b` |
| `recv.m(a)` | the receiver, then `a`, then the method lookup |
| `a[i] = v` | the container, then the key, then the value |
| `a.b.c = v` | the receiver chain, then the value |
| `#{k1: e1, k2: e2}` | `e1` then `e2`; keys are compile-time constants |

The differential harness ([`17-testing-strategy.md`](17-testing-strategy.md)) is the enforcement
mechanism, not a comment.

---

## 7. Loop compilation and the iteration protocol

### 7.1 `for x in seq`

```rust
StmtKind::For(var, seq_expr, body) => {
    // 1. Evaluate the sequence ONCE, into a hidden 2-slot iterator.
    let seq_reg = self.expr(seq_expr)?;
    let iter    = self.alloc_temps(2)?;        // <-- the hidden 2 slots
    self.emit(span, Op::ITER_INIT, iter, seq_reg, 0);

    // 2. Loop head.
    let head = self.here();
    self.emit(span, Op::ITER_DONE, iter, self.scratch());
    let exit = self.emit_jump(span, Op::JMP_IF_FALSE, self.scratch(), 0);

    // 3. Bind the loop variable, then compile the body.
    self.bind_loop_var(var);
    self.emit(span, Op::ITER_NEXT, iter, var_reg);
    self.stmt_list(body);

    // 4. `continue` targets the loop head directly: there is no separate
    //    increment step to jump to. The old lowering had one
    //    (src/bytecode.rs:696-706); the iterator protocol does not.
    self.patch_loop_continues(head);
    self.emit_jump(span, Op::JMP_BACK, head);

    // 5. Close any upvalues opened inside the body, then exit.
    self.close_scope_upvalues();
    let end = self.here();
    self.patch(exit, end);
    self.free_temps(iter, 2);
}
```

`for x in [10, 20, 30]: print(x)` compiles to exactly seven instructions:

```text
0000  make_list  %r3, %r4..%r6      ; [10, 20, 30]  -- 3 elems in %r4..%r6
0001  iter_init  %r0 [state=%r0, cursor=%r1] <- %r3
0002  iter_done  %r0, %r2            ; r2 = exhausted?
0003  jmp_if_false %r2, +4           ; exit at 0008
0004  iter_next  %r0, %r3            ; r3 = x
0005  call_native %r4, #12, 1        ; print(%r3)
0006  jmp_back                      ; -> 0002
0007  return_unit
0008  ; end
```

**Compare with the old lowering** (`src/bytecode.rs:640-686`): it emitted a `StoreLocal` for the
list, a `Const 0` for the index, then per iteration `LoadLocal i; LoadLocal list; Len; Lt;
JmpIfFalse; LoadLocal list; LoadLocal i; Index; StoreGlobal name` — **10 instructions per iteration
against our 5**, and it worked only on lists. `for` over a `dict`, a `string`, a `set` or a
generator was wrong or rejected. The iteration protocol is not an optimisation here; it is a
correctness fix.

### 7.2 The hidden 2-slot iterator convention

```text
        R[a]  = iterator STATE       R[a+1] = CURSOR
        ───────────────────────     ───────────────────────
ITER_INIT  state = the sequence,    cursor = 0
           or an iterator object
ITER_DONE  dst = (state exhausted)
ITER_NEXT  dst = next item, and advances the cursor
```

Precisely:

- **`R[a]`, the state slot.** For a `list`, `tuple`, `string`, `bytes` or `dict`, this is the
  container `Value` itself — no allocation, no wrapper. For a `range`, a `set`, a generator, a
  channel receiver, or a native iterator object, it is an `IterState` heap object whose
  `ObjHeader.kind` is `ObjKind::Iter`. A `range` therefore costs one 24-byte allocation **at loop
  entry, not per element**, which is the fix for the old `MakeRange` that materialised the whole
  list (`src/bytecode.rs:145`).
- **`R[a+1]`, the cursor slot.** A small integer. For a sequence, it is the next index. For a
  generic iterator, `ITER_NEXT` calls the state's `advance` method and increments the counter for
  diagnostics. **The cursor reaching `len` (sequences) or `-1` (generic) is the exhaustion
  sentinel.** There is no third slot and no separate flag: a sentinel-valued cursor is what
  `ITER_DONE` tests, so the iterator costs exactly two registers whether it is a `list` or a
  generator.
- **The two slots are always consecutive and adjacent to the loop variable when possible**, so a
  hot loop's iterator and its element live in the same cache line.

**Why `ITER_DONE` and `ITER_NEXT` are separate instructions** rather than one fused
`ITER_NEXT_OR_JUMP`:

- Fusing forces the opcode to be either `ITER_NEXT a, dst, target` (3 sources, no way to express
  "no target") or two opcodes, which is the same problem again.
- More importantly, the `for` loop needs the done-check *before* the body and the advance *inside*
  the head; a `while`-style loop over the same iterator needs only the advance. Separating them
  means the iterator is a first-class value that a hand-written loop can drive manually — which is
  what `bench_iter_protocol` is built to measure, and what makes the M11 translator's loop
  translation straightforward.

**`while` loops** do not use the protocol unless the source is a generic `for`. A
`while i < n: i = i + 1` loop is compiled directly to `JMP_IF_GE` + body + `JMP_BACK`, which is
2 fewer instructions than the old lowering needed (it emitted `JmpGeLocalConst` plus a separate
`AddLocalImm` increment; the peephole fuses the latter only if `zen-superins` is on, §11.4).

**`break` / `continue` inside a `for`.** `continue` jumps to the loop head — there is no increment
block to skip, because the iterator owns the advance. `break` jumps past `close_scope_upvalues` to
the loop exit. Both are single `JMP`/`JMP_BACK` instructions with a fixup list, exactly as the old
compiler structured them (`src/bytecode.rs:687-708`), but the continue target is one instruction
earlier and the fixup is a relative `i8` rather than an absolute `u16`.

---

## 8. `try` / `catch` / `finally` lowering

Zen's `try` statement is `StmtKind::Try(body, catches, finally_body)` in the old AST
(`src/runtime.rs:1414`), with each catch a `CatchClause { kind: Option<String>, var: Option<String>,
body }` (`src/runtime.rs:1430-1435`). The M4 compiler consumes the same shape from
[`06-parser.md`](06-parser.md).

### 8.1 The lowering

```text
        TRY_PUSH  handler=0x…  finally=0x…     ; protect [pc, try_end)
        <try body>
   try_end:
        TRY_POP                              ; the happy path
        JMP     after
        handler=:
          <for each catch clause, in source order>
            CATCH_MATCH  %r_tmp, $N"TypeError"
            JMP_IF_FALSE  %r_tmp, next_clause
            <bind the catch variable>       ; ERROR_MSG if the clause has a var
            <catch body>
            JMP   after
          next_clause:
            <bare `catch as e` / `catch e` clause, if present — always last>
            <bind e to the thrown value>
            <catch body>
            JMP   after
        finally=:
          <finally body>
        after:
```

Three properties, all required by the language:

1. **Clauses are tried in source order**, and the *first* match wins. The parser puts a bare
   `catch` last by construction in every well-formed program; the compiler does not rely on that,
   it just emits them in order.
2. **`finally` always runs**, on the normal path and on the throw path. On the normal path control
   falls through the `TRY_POP` into the finally block. On the throw path the unwinder jumps to
   `finally_pc`, runs it, then hits `RERAISE` to continue the search for an outer handler.
3. **An unmatched error propagates.** If no clause matches, control falls out of the bottom of the
   handler into the `finally` block and then `RERAISE` re-raises to the next enclosing handler.

### 8.2 The exception table

```rust
/// One protected region. Entries are sorted by `try_start` within a proto so
/// the unwind search is a binary search, then a linear scan inward.
#[derive(Clone, Copy, Debug)]
pub struct HandlerEntry {
    /// First protected instruction (inclusive).
    pub try_start: u32,
    /// One past the last protected instruction (exclusive).
    pub try_end: u32,
    /// Where to resume when this handler catches.
    pub handler_pc: u16,
    /// Where the `finally` body starts, or 0 if there is none.
    pub finally_pc: u16,
    /// Interned class names this handler catches, in source order. Empty means
    /// "catch anything" (the bare `catch as e` form).
    pub catch_classes: &'static [u32],
    /// How many registers the handler must restore before jumping to
    /// `handler_pc`. Always the full `frame_size` in M4, because Zen has no
    /// destructuring unwind; kept as a field so M9's GC can reuse the entry.
    pub restore_regs: u16,
}

/// The per-frame handler cursor. `FunctionProto::exception_table` is immutable
/// and shared; the cursor is not.
#[derive(Clone, Copy)]
pub struct HandlerCursor {
    /// Index one past the innermost live entry in `proto.exception_table`.
    pub depth: u16,
    /// Number of open upvalues at the time the handler was pushed. The
    /// unwinder closes down to this count, which is why `CLOSE_UPVALUE` can be
    /// skipped on the throw path.
    pub upvalue_count: u8,
}
```

A `try` block's *runtime* cost on the happy path is `TRY_PUSH` (one `u32` write plus one `u8`
write into a `Vec` that was `reserve`d at frame entry) and `TRY_POP` (the mirror). Two
instructions, both of which fall through in the common case.

### 8.3 Why this is cheaper than Rust-style unwinding

Rust's `panic::catch_unwind` requires a landing pad for every frame, a personality routine, and DWARF
CFI so the unwinder can walk the stack. On the *happy* path that is genuinely free, but it costs:

- A `#[cold]` landing-pad block in **every** function, which is instruction-cache pressure across
  the whole program, not just on the error path.
- A `DWARF` unwind table in the binary, and a binary that cannot be stripped to a small size.
- A stack walk on the error path, which is proportional to call depth and cannot be made cheap.
- Every `unsafe` boundary has to be `unsafe_op_in_unsafe_fn`-clean, because a panic crossing it is
  a bug — a rule that M4 would have to honour for the JIT's trampolines too.

Zen's unwinder is a **linear scan of a `Vec<HandlerEntry>` the VM already keeps hot**, with no
per-frame cost, no CFI, and no personality routine. The three concrete reasons it is cheaper *here*:

1. **The happy path is not free in Rust either, and it is nearly free here.** Rust pays nothing at
   *runtime* but pays in code size and I-cache. `TRY_PUSH`/`TRY_POP` are two straight-line
   instructions in the *same cache line* as the try body.
2. **Zen calls do not grow the native stack.** Every Zen→Zen call is an iteration of one `loop {}`
   in [`08-vm.md`](08-vm.md) §3. The only native frames that exist are Zen→native calls, and the
   unwinder's frame walk is a walk over `vm.frames`, a `Vec<Frame>` that is already in L1. A Zen
   exception crossing a native frame is the one case that touches the real stack, and it is handled
   by requiring every `NativeFn` to be `extern "C"`-clean and returning `VmResult<Value>` — never
   by panicking.
3. **It makes stack overflow catchable.** The same linear walk finds the frame-depth limit and
   raises a `StackOverflow` `ZenError` that a `catch` clause can observe, which a guard-page
   segfault categorically cannot ([`00-blueprint-review.md`](00-blueprint-review.md) §5.3).

**The honest cost.** `TRY_PUSH`/`TRY_POP` is ~2 ns per try-block *entry*. A function with twenty
sequential `try` blocks pays 40 ns once. A `try` inside a hot loop pays 2 ns per iteration, and
that is a real regression against Rust. The mitigation is the peephole's handler-stack-depth
simplification (§11.2): two adjacent `TRY_PUSH`es where the outer region fully contains the inner
one and the outer has no `finally` collapse to one, which is the shape generated by
`with`-style helper functions.

---

## 9. Closures and upvalues

### 9.1 Ruling: capture by reference, through a heap cell

The old runtime's `Value` has a variant for exactly this, and the language's semantics depend on
it:

```rust
// src/runtime.rs:197-200
// Shared mutable cell used for closure captures. Cloning a Cell shares
// the same storage, so a captured variable read/written through different
// closures (or the defining frame) observes one value.
Cell(Arc<Mutex<Value>>),
```

`deref_cell` (`src/runtime.rs:237-251`) and `cell_set` (`src/runtime.rs:255-263`) implement
read-through and write-through. The corpus makes the requirement unambiguous — in
`tests/cases/11_closures_and_callables.z`:

- `make_counter()` returns a `lambda` over `var n`; the closure **outlives the frame** and keeps
  mutating `n` across calls, so the capture must be **by reference**, not by value.
- Two sibling closures made in the same scope (`inc` and `get`) **observe the same `n`**, so two
  captures of the same variable must resolve to **one** cell.
- `outer()` in that file calls a nested `func add(v)` that mutates `total`, then reads `total` in
  the **live parent frame** and sees the update, so the cell must be shared with the defining
  frame's register, not copied into it.

**Ruling: every captured variable is a single heap cell shared by the defining frame, every closure
that captures it, and every other closure that captures it transitively. Capture by value is never
emitted.**

Two changes from the old representation, both required:

- **No `Mutex`.** The cell is `Arc<Upvalue>` with an `UnsafeCell<Value>`. Zen is single-threaded per
  VM instance; sharing a value across threads is already governed by
  [ADR-0003](decisions/ADR-0003-concurrency-semantics.md), where a non-`shared` value touched from
  a second thread is a catchable error, and `shared x` wraps in `Arc<Mutex<T>>` at the **value**
  level, not the upvalue level. Keeping a lock in the upvalue would pay for a guarantee the
  language does not make.
- **At most one level of indirection.** The old `deref_cells` (`src/runtime.rs:204-235`) walks a
  *chain* of cells and carries a `panic!` for a cyclic chain at `src/runtime.rs:225` — an
  unconditional panic reachable from a script, which violates
  [`00-blueprint-review.md`](00-blueprint-review.md) §5.1. The M4 compiler **proves** single-level:
  a captured cell is captured *as a cell*, and is never re-boxed, so `GET_UPVAL` is one pointer load
  with no chain walk and no cycle possible. The test for this is a compiler unit test, not a
  runtime guard.

### 9.2 Open upvalues, and the pinning rule

Following Lua 5.4, an upvalue may be **open** — pointing directly at the register that holds the
variable — or **closed** — pointing at a heap box. The distinction is worth a real opcode pair
(`OPEN_UPVALUE`, `CLOSE_UPVALUE`) because it removes the allocation for the common non-escaping
closure.

```rust
/// An upvalue is a tagged pointer. Bit 0 is the tag.
///
///   0 -> CLOSED: a `*mut Value` pointing at the heap box's payload.
///   1 -> OPEN:   a `u32` register index into `Vm::registers`. The cell is
///                 still owned by the enclosing frame's `open_upvalues` list,
///                 so `CLOSE_UPVALUE` can find and box it.
///
/// A pointer is 8-byte aligned, so bit 0 is always free, and the tag test is
/// one `test`/`and` on a value already in a register.
#[repr(transparent)]
pub struct Upvalue {
    tagged: usize,
}

impl Upvalue {
    #[inline] pub fn is_open(&self) -> bool { self.tagged & 1 == 1 }
    #[inline] pub fn as_open_index(&self) -> u32 { (self.tagged >> 1) as u32 }
    #[inline] pub fn as_closed_ptr(&self) -> *mut Value { self.tagged as *mut Value }
}
```

**The cost of an open upvalue is a pinned register.** If `%r4` is captured by a closure and the
upvalue points at `R[base + 4]`, then every write to the variable must be a write to `R[base + 4]`
for the whole life of the frame. So the register allocator must never hand `%r4` to a temporary:

> **Pinning rule.** From the `OPEN_UPVALUE` that captures a local to the `CLOSE_UPVALUE` that ends
> its scope, that local's register is marked `pinned: true` in the `LocalSlot` table and is excluded
> from `free_regs`. The `zen check --disasm` header prints pinned slots, so the cost of a closure
> is visible.

This is a real, named trade: open upvalues save one 16-byte allocation per captured variable per
closure creation and cost one pinned register per captured variable. For `make_counter()` (one
captured variable) the trade is a clear win; for a function with 30 captured variables it is 30
pinned registers, which is a third of the frame. §14 lists "is the open-upvalue threshold right?"
as a question to resolve with the `bench_closure_alloc` numbers.

**Closure creation cost, concretely.** `make_counter()` at its `MAKE_CLOSURE`:
one `Arc<Upvalue>` allocation of 16 bytes (only if the cell is *closed*, i.e. the closure escapes
the frame), one `Arc<Closure>` allocation of 48 bytes (`proto: u32`, `upvalues: [Upvalue; N]`
inline, `Arity { min, max }`, `name: u32`, `kind: u8`), and two non-atomic refcount increments
because everything else is borrowed from the shared `FunctionProto`. **≈ 55 ns [E]** on a 2024
x86 core. That is the number `bench_closure_alloc` measures, and it is the one entry in
[`00-blueprint-review.md`](00-blueprint-review.md) §4's "known slower than CPython" table that M4
is expected to improve on — CPython allocates a `PyFunctionObject` **plus** a `PyCell` **plus** a
`PyTuple` for the upvalues.

### 9.3 The upvalue descriptor table

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpvalueSource {
    /// Take from the enclosing function's local register.
    ParentLocal { reg: u8 },
    /// Take from the enclosing function's upvalue, one level of transitive
    /// capture. `f` may name a variable `g` captured, which itself captured
    /// one from `h`.
    ParentUpvalue { index: u8 },
}

#[derive(Clone, Copy, Debug)]
pub struct UpvalueDesc {
    pub index: u8,
    /// Interned name, for `--disasm` and the REPL. **Not** on the hot path.
    pub name: u32,
    pub source: UpvalueSource,
    /// `true` if the variable is ever assigned after the closure is created.
    /// `false` means the compiler may capture **by value** — a copy taken at
    /// `MAKE_CLOSURE` time — because no code can observe the difference.
    /// This is the one place capture-by-value is legal, and it is what makes
    /// `for _, x in list: use(lambda: x)` not allocate a cell per iteration.
    pub never_mutated: bool,
}
```

Resolution happens **once, at compile time**, producing the flat `upvalue_descriptors` that
`MAKE_CLOSURE`'s VM arm walks:

```rust
fn resolve_upvalue(c: &Compiler, name: &str, from: u32) -> CompileResult<u8> {
    // 1. Already in this function's table?
    if let Some(i) = c.upvalue_index(from, name) {
        return Ok(i);
    }
    // 2. A local of the *parent*? Then this function captures it.
    if let Some(&reg) = c.parent_local_reg(from, name) {
        let idx = c.push_upvalue(from, UpvalueDesc {
            index: c.upvalues_len(from) as u8,
            name: c.intern(name),
            source: UpvalueSource::ParentLocal { reg },
            never_mutated: c.never_mutated(from, name),
        });
        return Ok(idx);
    }
    // 3. Recurse one level: the parent also captured it.
    let parent = c.parent_of(from).ok_or(CompileError::UnknownUpvalue { name })?;
    let via = resolve_upvalue(c, name, parent)?;
    let idx = c.push_upvalue(from, UpvalueDesc {
        index: c.upvalues_len(from) as u8,
        name: c.intern(name),
        source: UpvalueSource::ParentUpvalue { index: via },
        never_mutated: c.upvalue_never_mutated(parent, via),
    });
    Ok(idx)
}
```

`ParentUpvalue` chains are flattened at compile time, so a closure nested three functions deep
resolves to a flat two-entry chain, not a runtime walk — which is precisely the flattening the old
`deref_cells` had to do at *runtime*, on every read.

### 9.4 The three closure opcodes in practice

```zen
func make_counter() {
    var n = 0              # %r1
    return lambda() { n = n + 1; return n }
}
```

```text
func make_counter() {
  ; frame=3 locals=2 upvalues=0 code=9 consts=0 handlers=0 ic_sites=1
  ; %r0 = n  var   (PINNED: open upvalue)
  0000  load_small_int %r0, 0
  0001  open_upvalue  %r0, #0        ; n becomes an open upvalue of this frame
  0002  make_closure  %r1, #0, 1     ; proto #0, upvalue source = %r1 (i.e. %r0 + 1)
  0003  tail_call     %r1, #0, 0     ; `return lambda(...)` is a tail call
  ; upvalue #0 n <- ParentLocal{reg:0}
}

func <lambda> {
  ; frame=2 locals=1 upvalues=1 code=5 consts=1 handlers=0 ic_sites=0
  ; %r0 = __ret  scratch
  0000  get_upval     %r0, #0        ; n
  0001  load_small_int %r1, 1
  0002  add           %r0, %r0, %r1
  0003  set_upval     #0, %r0        ; n = n + 1
  0004  return        %r0
  ; upvalue #0 n <- ParentUpvalue{index:0}
}
```

`CLOSE_UPVALUE` does not appear in the disassembly because `make_counter` **tail-calls** — the
frame is replaced, so the compiler knows the whole frame is going away and the open upvalues are
closed by the tail-call sequence, not by a separate instruction. When the closure is stored and the
frame returns normally, the implicit `CLOSE_UPVALUE %r0, 1` is emitted just before `RETURN`.

---

## 10. Serialisation: the `.zeno` container

### 10.1 Header

```text
offset  size  field
------  ----  ---------------------------------------------------------
0x00      4   magic       = "ZEN\x03"
0x04      4   format_ver  = 1              (u32 LE)   -- §10.2
0x08      4   flags                          (u32 LE)   -- §10.2
0x0c      4   section_count                  (u32 LE)
0x10      4   payload_len                    (u32 LE)   -- bytes after this header
0x14    ...   sections[section_count]
...      4   crc32c(payload)
```

The CRC is **CRC-32C (Castagnoli)**, computed over everything from `0x10` to the end of the last
section, and stored little-endian. Not CRC-32 (IEEE): Castagnoli's polynomial has hardware
support on every x86 since SSE 4.2, on every ARM since Cortex-A72, and — more importantly — it is
what the SSD and network stacks that actually corrupt our files use, so a bit flip that a
conventional CRC would miss is more likely to be caught. It is a **payload** checksum, not a
security mechanism; the threat model is corruption, not a hostile actor.

The old container used `MAGIC = b"ZENC"` with `FORMAT: u32 = 2` (`src/cache.rs:17-18`) and **no
checksum at all** — corruption was detected only by `Reader::exhausted()` rejecting trailing bytes
(`src/cache.rs:287-290`), which catches truncation but not a flipped byte in the middle of the code
array. A flipped `op` byte is a silent wrong answer. The CRC is the fix, and it is a 4-byte cost on
a file that is already tens of kilobytes.

### 10.2 Flags and versions

| Bit | Name | Meaning |
|---|---|---|
| 0 | `HAS_DEBUG_INFO` | The `SECT_LINES` and `SECT_SPANS` sections are present. A release build omits both and `zen run` reports a source-less traceback. |
| 1 | `HAS_IC_SITES` | The `SECT_IC_SITES` section is present. Absent in images built before M5, in which case M5 seeds inline caches from scratch. |
| 2 | `IS_NATIVE_LIB` | The image is a `.zeno` **shared library** (a `native` module), not a program. |
| 3–31 | reserved | Must be 0. A loader that sees a set reserved bit rejects the file. |

`format_ver` is **1** and is bumped for any change to the section layout, the opcode set, or the
`Op` numeric mapping. The `Op` enum's *discriminant* is never written directly; the serialiser uses
an explicit `op_to_u16` table exactly as the old cache does (`src/cache.rs:25-106`), with the
comment that "the enum's memory layout must never be relied upon for persistence" — that reasoning
is correct and is kept. The difference is that our table is **generated by a build-time test**
that asserts it is a bijection onto the 110 core+gated IDs, so inserting a variant without adding a
row is a compile error in the test, not a silent cache corruption in the field.

### 10.3 Sections

Sections are `(u32 kind, u32 len, bytes[len])`, dense, in ascending `kind` order so a reader can
binary-search and a writer can stream.

| Kind | Name | Contents |
|---|---|---|
| `0x01` | `SECT_MODULE` | Source path, source hash, module name, top-level proto count. |
| `0x02` | `SECT_NAMES` | The module's **identifier** interner, deduplicated and delta-compressed. |
| `0x03` | `SECT_STRINGS` | The module's **string value** interner, deduplicated and delta-compressed. |
| `0x04` | `SECT_PROTOS` | Per-proto fixed header: name ids, `num_locals`, `num_upvalues`, `frame_size`, `flags`, and the offsets/lengths of that proto's sub-sections. |
| `0x05` | `SECT_CODE` | All protos' `Vec<Instruction>` concatenated. **Raw 4-byte words, no per-instruction re-encoding**, so loading is a `Vec<u8>` copy plus a `from_bits` per instruction on 64-bit targets that need alignment. |
| `0x06` | `SECT_CONSTS` | A tagged constant stream (§5.2's table is the whitelist; anything not in it is a load error, and the compiler already guarantees it cannot appear). |
| `0x07` | `SECT_LINES` | `LineEntry` records, per proto. Present only if `HAS_DEBUG_INFO`. |
| `0x08` | `SECT_EXC` | `HandlerEntry` records, per proto, plus the `catch_classes` name-id lists. |
| `0x09` | `SECT_UPVALS` | `UpvalueDesc` records, per proto. |
| `0x0A` | `SECT_IC_SITES` | `IcSite` records, per proto. Present only if `HAS_IC_SITES`. |
| `0x0B` | `SECT_NATIVE_SIGS` | For `IS_NATIVE_LIB`: name, arity, and the `NativeFn` table id, so a native module's signature is checkable without loading the `.so`. |
| `0x0C` | `SECT_TOC` | A table of contents so a reader can `mmap` and address sections without walking. Written last. |
| `0x0D` | `SECT_SPANS` | `SpanEntry` records, per proto — exact `(lo, len)` spans for every raising instruction (§5.1.1). Present only if `HAS_DEBUG_INFO`. |

**`.zeno` is a whole module, not a whole program.** The old cache wrote `Vec<Arc<CompiledFunction>>`
for the entire program in one file (`src/cache.rs:487-518`); Zen 3.0 caches **per module** so
editing one file in a 40-file project re-compiles one file. The per-module unit also matches the
`SECT_TOC` layout, so a large program is many small `mmap`-able files rather than one growing one.

### 10.4 The defaults rule — replacing `src/cache.rs:388-393`

The old cache refuses to store any function with a default parameter:

```rust
// src/cache.rs:388-393
// Default-parameter expressions are AST (not serializable); a cache entry
// would silently lose them, so never cache functions with defaults.
if cf.params.iter().any(|| _, d| d.is_some()) {
    return false;
}
```

That is a direct consequence of storing `Option<crate::runtime::Expr>` in `CompiledFunction`
(`src/bytecode.rs:181`). The consequence at the user level is that **every** Zen program using a
default parameter silently re-parses and re-compiles on every run — and `print`, `range`, `error`,
and most of the stdlib have defaults, so in practice the old cache is off for most real code.

**Ruling: defaults are pre-evaluated into a synthetic "defaults thunk" proto, and that thunk is
itself ordinary cached bytecode. There is no special case for the cache.**

```
func greet(name = "world", punct = "!")
```

lowers to three protos:

| Proto | Role | Cached? |
|---|---|---|
| `greet` | the body. Starts with `LOAD_DEFAULT %r0, #0; LOAD_DEFAULT %r1, #1`. | **Yes**, verbatim. |
| `greet$defaults` | a 0-arg, 0-local thunk whose `code` is `LOAD_CONST %r0, $K0; RETURN %r0; LOAD_CONST %r0, $K1; RETURN %r0` and whose `defaults` is `[value_of(K0), value_of(K1)]`. | **Yes**, verbatim. |
| `greet$defaults$capture` | emitted **only if** a default expression is not constant-foldable. Its code reads the module globals the default mentions. | **Yes**, verbatim. |

At **module load**, the VM runs the defaults thunk once per proto that has `HAS_DEFAULTS`, and
stores the resulting `Vec<Value>` in `FunctionProto::defaults`. That vector is *runtime state*, it
is never serialised, and it is recomputed on every process start — so `func now() -> f(x = time())`
is correct, and the cache still hits.

The consequence for the old rejection: `write_cf`'s `return false` disappears, and with it the whole
class of "your program was not cached and you did not know why" failures.

### 10.5 Non-serialisable values: prove it at compile time, not at cache-write time

The old `write_value` returns `false` for anything outside null/bool/number/string/list/dict
(`src/cache.rs:301-353`), with the comment "Functions, instances, sockets etc. can never appear in
a compiled constant pool; refuse rather than mis-serialize" (`src/cache.rs:350-352`). `store` then
silently returns without writing (`src/cache.rs:499-501`). The result is a correct-but-mysterious
performance cliff.

**Ruling: the compiler refuses to place a non-whitelisted value in a constant pool, as a
`CompileError::NonConstant` with a source span.** §5.2's table is the whitelist, and it is enforced
by a `debug_assert` in `add_constant` plus a release-mode check in the same place. By the time
`store` runs, serialisation cannot fail; the only remaining failure modes are I/O, and those are
reported (`zen build` warns when it cannot write a cache, instead of pretending).

### 10.6 The cache key

The old key is a `DefaultHasher` over `FORMAT`, `CARGO_PKG_VERSION`, and the source text
(`src/cache.rs:197-204`), stored as `<hash>.zc` under `.zencache/` next to the source
(`src/cache.rs:206-215`), written atomically via a `pid`-suffixed temp file plus `rename`
(`src/cache.rs:511-518`). Three changes:

1. **`DefaultHasher` is replaced by an explicit FNV-1a 64** for the file name.
   `DefaultHasher`'s algorithm is not a stable contract across Rust releases, so a toolchain upgrade
   could invalidate every cache — harmless, but it makes `zen build --incremental` mysteriously slow
   after a `rustup update`. FNV-1a is 20 lines, hand-written, and never changes.
2. **The key covers the format version, the engine version, and the source** — the same three
   inputs as the old one, in that order, with the source length mixed in before the bytes so that
   `("ab", "c")` and `("a", "bc")` cannot collide.
3. **The key covers the opcode table's own hash.** A build with `zen-superins` on and one with it
   off produce different bytecode from identical source, so `SUPERINS_ENABLED` is a fourth key
   input. The old cache did not have this problem because it had no feature-gated passes; the new
   one does, and the fix is cheaper than discovering it in a bug report.

```rust
fn cache_key(source: &str) -> u64 {
    // FNV-1a 64, offset basis 0xcbf29ce484222325, prime 0x100000001b3.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    mix(&ZENO_FORMAT_VER.to_le_bytes());       // 1
    mix(env!("CARGO_PKG_VERSION").as_bytes()); // "3.0.0"
    mix(&[cfg!(feature = "zen-superins") as u8]);
    mix(&(source.len() as u64).to_le_bytes());
    mix(source.as_bytes());
    h
}
```

**A corrupt or stale cache is a cache miss, never an error.** Every mismatch — wrong magic, wrong
format version, failed CRC, a section that runs past the end of the file, a reserved flag bit set,
a proto whose `num_locals > 250` — returns `None` from the loader and the caller falls back to
compiling from source. There is no path in which a bad cache produces a wrong answer; that property
is tested directly (§12, test 24).

---

## 11. Optimisation passes

Passes run in this order. Each has an independent gate, a `#[cfg(feature)]`, and a `--no-<pass>`
escape in `zen run`, so any one of them can be bisected out of a performance regression without a
rebuild.

| # | Pass | Feature | Milestone | Default | Expected gain **[E]** |
|---|---|---|---|---|---|
| P0 | Constant folding | always | M4 | on | 3–8 % on expression-heavy code; 0 on I/O-bound code |
| P1 | Dead code and unreachable-code elimination | always | M4 | on | 0–1 % |
| P2 | Jump threading and branch simplification | always | M4 | on | 1–3 % on branch-heavy code |
| P3 | Peephole: redundant loads, `MOVE` coalescing, `COPY` propagation | always | M4 | on | 2–5 % |
| P4 | Superinstruction selection | `zen-superins` | M5 | **off** | 3–8 % interpreter-only, **0 % under the JIT** |
| P5 | Type-specialisation-driven re-lowering | `zen-jit` | M11 | off | see §11.5 |

### 11.1 P0 — constant folding

Folds any expression whose operands are constants and whose operator is total. `1 + 2 * 3` becomes
one `LOAD_SMALL_INT %r0, 7`.

**The rules, and the two that are not obvious:**

- **Only immutable-shaped values are folded**: `nil`, `bool`, `int`, `float`, `string`, `bytes`, and
  tuples of those. **Lists, dicts and sets are never folded**, for the reasons in §5.2.
- **`DIV` and `MOD` by a literal zero are NOT folded and the division is NOT removed.** `1 / 0`
  must raise `ZeroDivisionError` at run time, because the alternative — the compiler proving the
  error and raising it at compile time — changes when the error occurs and whether a `catch` can
  observe it. Folding `1 / 0` to a `THROW` is tempting and wrong: the program might never execute
  that line, and a compile-time error on unreachable code is a regression.
- **Overflow is folded the same way the runtime computes it.** `MAX_I48 + 1` folds to a `float`,
  matching `ADD`'s int-overflow-promotes rule from [ADR-0001](decisions/ADR-0001-tag-budget.md). A
  folder that wrapped instead would produce a *different answer from the interpreter*, and the
  differential harness would catch it — which is the point of having one.
- **`EQ` between two constants of different types folds to a `bool`.** `1 == "1"` is `false` at
  compile time, not a `TypeError` at run time, because `EQ` is total (§3.4).

### 11.2 P1–P3 — the always-on peephole

These run as a single backward pass over `code`, followed by a `pc` renumbering, then a fixup
re-resolution. `NOP`s are inserted to keep the renumbering simple, and removed by a final
compaction that also rewrites the line table, exception table, IC-site table and every fixup
together — one function, one place to get wrong, one test.

| Rewrite | Pattern | Before | After |
|---|---|---|---|
| Constant fold | `LOAD_SMALL_INT r, 2; LOAD_SMALL_INT r, 3; ADD r, r, r` | 3 | `LOAD_SMALL_INT r, 5` (1) |
| Redundant load pair | `MOVE r1, r0; MOVE r1, r0` | 2 | `MOVE r1, r0` (1) |
| Redundant `MOVE` | `MOVE r1, r0; MOVE r0, r1` | 2 | `MOVE r1, r0` (1) — `r0`'s old value is dead |
| `MOVE` coalescing | `MOVE r2, r0` immediately before a binding that assigns `r2` | 2 | 0 — the binding writes `r0` directly |
| Jump threading | `JMP t` where `code[t]` is `JMP u` | 2 | `JMP u` (1) |
| Jump-to-jump collapse | `JMP_IF_FALSE r, t` where `code[t]` is `JMP u` | 2 | `JMP_IF_FALSE r, u` (1) |
| Inverted branch | `JMP_IF_FALSE r, t; JMP f` with `f == t + 1` | 2 | `JMP_IF_TRUE r, t` — **and `JMP_IF_TRUE` is not in the table**, so the compiler inverts the *test* instead, via `NOT`, and the peephole folds `NOT; JMP_IF_FALSE` back into `JMP_IF_TRUE`… **which does not exist.** See the ruling below. |
| Unreachable after return | any instruction after `RETURN`/`RETURN_UNIT`/`THROW` | *n* | `NOP` × *n*, then compacted away |
| Dead store | `LOAD_CONST r, K` where `r` is a temporary never read | 1 | 0 |
| `COPY` propagation | `MAKE_LIST r0, 1, r1` where `r1` is a temp holding a constant | 2 | `LOAD_CONST r0, K` (1) |
| Method peel | `IC_CALL r1, s, 0; LEN r2, r1` where `s` is the IC target | 2 | `LEN r2, s` (1) |
| Nested handler collapse | `TRY_PUSH a; …; TRY_PUSH b; …; TRY_POP; TRY_POP` with `b ⊂ a`, `a` has no `finally`, and no `RERAISE` between | 4 | `TRY_PUSH a; …; TRY_POP` (2) |
| `SAFEPOINT` dedup | `SAFEPOINT` immediately dominated by another with no `pc` between | 2 | 1 |
| Temp coalescing | `MOVE r1, r0` where `r0` is a temp, dead after, and `r1` is also a temp | 1 | 0 — the allocator's numbering is fixed up instead |

**The `JMP_IF_TRUE` hole, and the ruling.** The table has `JMP_IF_FALSE` but not `JMP_IF_TRUE`,
because the compiler always inverts at the source level: `if x: A else: B` becomes
`JMP_IF_FALSE x, Lelse; A; JMP Lend; Lelse: B`, and the test is written as `!x` when the source
says `x`. A peephole that wants to invert a branch must therefore insert a `NOT` and a `TO_BOOL`
(two instructions for a one-instruction saving) — a loss, not a win, and the peephole correctly
skips the rewrite. **The ruling stands: no `JMP_IF_TRUE` in M4.** The branch-predictor cost of the
inverted form is one extra `NOT` on the cold path of a branch, which is unmeasurable against the
saving of a whole opcode and a guaranteed-correct polarity rule. §14 flags this as the ruling most
likely to be reversed after M5 profiling.

**Bug classes the peephole must never have, and their tests:** rewriting a `pc` mentioned by the
exception table, the line table, an IC site, or a fixup (each has a dedicated test that shuffles
the code array and checks all four tables still resolve); and rewriting a `DUP` into a `MOVE`
(aliasing bug — `DUP`'s source must stay live, so `DUP`→`MOVE` is only legal after a liveness
check, and the peephole does not do it).

### 11.3 What the peephole does **not** do

- **No loop inversion, unrolling, or vectorisation.** Those are P5's job and they need type
  information the M4 compiler does not have.
- **No constant propagation into registers across a call.** After a `CALL`, every register in the
  callee's window is clobbered and every local above the callee's frame is still valid but unknown
  to a cheap analysis. Getting this wrong is a wrong answer, so the peephole is conservative.
- **No inlining.** Decided separately, and not in M4.

### 11.4 P4 — superinstructions, and the conflict with the JIT

This is the honest part of the document, and the part most likely to be argued about.

**The gate.** `zen-superins` is **off by default**, off in the JIT-enabled build, and
`zen bench` records both. The gate opens only if enabling it improves the `bench_core` aggregate by
**≥ 3 %** without regressing any single benchmark — the same "never slower individually" clause as
the overall gate ([`00-blueprint-review.md`](00-blueprint-review.md) §4).

**Why it conflicts with a JIT, stated concretely.** A superinstruction exists to reduce the number
of *interpreted dispatches*. `LOAD_CONST_ADD r3, $K5` replaces `LOAD_CONST r3, $K5; ADD r3, r3, r3`,
saving exactly one dispatch — a 32-bit load, an indirect branch, a `match` lookup. The M11
translator lowers `LOAD_CONST` to a `mov` and `ADD` to an `add`; lowering `LOAD_CONST_ADD` requires
the translator to recognise the fused form and split it back into two machine operations, or to
re-specialise it — and **re-splitting it is the same work the peephole just did, done again in a
worse place.** Worse, the fused form destroys the `TYPE_GUARD` insertion point: the guard belongs
*before* the `ADD`, and a fused opcode has no place to put it, so a JIT that must handle
`LOAD_CONST_ADD` has to re-derive the guard from the constant's type — correct, but it means the
fused opcode costs the JIT a type check it would otherwise have had for free.

**The concrete ruling:**

> Superinstructions are an **interpreter-only optimisation**. Under the `zen-jit` feature the
> peephole's P4 stage is a no-op, and the corresponding peephole tests are `#[cfg(not(feature =
> "zen-jit"))]`. The opcode IDs stay allocated either way, so a `.zeno` file written by a
> superinstruction build still loads in a non-superinstruction build — the IDs decode, the
> interpreter arms exist, and the semantics are identical. The reverse also holds.
>
> This is why the 6 superinstructions have their own ID range (`0x80`–`0x85`) rather than being
> interleaved with the core table: the range is a *feature boundary*, visible in the file format
> and in the disassembler's grouping.

**The measurement that settles it.** `bench_core` with and without `zen-superins`, run under
`perf stat` for branch misses. If superinstructions reduce branch misses by >3 % of total
instructions, they earn the gate; if the aggregate gain is under 3 %, the gate stays shut and the
6 opcodes remain permanently off. That number does not exist yet and is on the §14 list.

### 11.5 P5 — the JIT-only passes (M11, listed for completeness)

`TYPE_GUARD` insertion driven by Cranelift's own type analysis, loop-invariant hoisting of
`len(seq)` out of an index loop, and unboxing a `range` loop into a counted loop. None of these
run in M4, and none of them change the semantics — the differential harness is the check.

---

## 12. Test plan

Two kinds of test: **golden tests** compare the full `disassemble()` output against a file in
`tests/disasm/`, and **assertion tests** check a structural property. Both live in
`crates/zen-vm/tests/compile/`. Every one of them also runs through the M3↔M4 differential harness,
so a compiler bug shows up as a *wrong runtime answer*, not just a diff.

### 12.1 Constants, arithmetic, and folding

| # | Input | Expected | Test kind |
|---|---|---|---|
| 1 | `1 + 2` | `LOAD_SMALL_INT %r0, 1; LOAD_SMALL_INT %r1, 2; ADD %r0, %r0, %r1` — three instructions, **not folded** (the RHS is not a compile-time constant in a general expression; folding is per-statement) | golden |
| 2 | `let x = 1 + 2` | `LOAD_SMALL_INT %r0, 3` — **one** instruction, folded at the statement level | golden |
| 3 | `1 + 2 * 3` | `LOAD_SMALL_INT %r0, 7` — folded, correct precedence | golden |
| 4 | `x / 0` where `x` is a local | `LOAD_CONST %r1, 0; DIV %r0, %r0, %r1` — **not** folded, **not** removed, **not** turned into a `THROW` | golden |
| 5 | `"a" + "b"` | `LOAD_CONST %r0, $K0` where `$K0 = "ab"` — folded, `string` is whitelisted | golden |
| 6 | `[1,2] + [3]` | `MAKE_LIST %r0, 1, %r1; MAKE_LIST %r1, 1, %r2; LOAD_SMALL_INT %r2, 3; ADD %r0, %r0, %r1` — **not** folded; lists are never constants | golden |
| 7 | `MAX_I48 + 1` | `LOAD_CONST %r0, $K0` where `$K0` is a **float**, not an int — the folder uses the runtime's overflow rule | golden |
| 8 | `-5` | `LOAD_SMALL_INT %r0, -5` — folded into the immediate, no `NEG` | golden |
| 9 | `x ** 2` | `LOAD_SMALL_INT %r1, 2; POW %r0, %r0, %r1` — not folded, `x` unknown | golden |

### 12.2 Registers, variables, and evaluation order

| # | Input | Expected | Test kind |
|---|---|---|---|
| 10 | `a + b * d` (locals 0,1,2) | `TYPE_GUARD %r0; TYPE_GUARD %r2; MUL %r3, %r1, %r2; ADD %r0, %r0, %r3; MOVE %r4, %r0` — 4 real instructions (§6.3) | golden |
| 11 | `a[f()] = g()` | `DUP %r1, %r0; CALL %r2, #3, 0; CALL %r3, #7, 0; SET_INDEX %r1, %r2, %r3` — **`f()` strictly before `g()`** | golden + differential with side-effecting `f`/`g` |
| 12 | `f(g(), h())` | `CALL %r0, #0, 0` (g); `CALL %r1, #1, 0` (h); `CALL %r2, #2, 2` (f) — callee's args left to right, callee last | golden |
| 13 | `s.upper()` (`s` in `%r0`) | `IC_CALL %r1, #0, 0` — **one** instruction, receiver at `R[%r1-1-0]` | golden |
| 14 | `recv.m(a)` | `MOVE %r1, %r0; CALL %r2, #1, 0; IC_CALL %r3, #0, 1` — receiver, then arg, then the call | golden |
| 15 | 300 sequential `let`s in one function | **`CompileError::TooManyRegisters`**, with a span. Not a crash, not a wrong answer | assertion |
| 16 | a function with 300 distinct string constants | **`CompileError::TooManyConstants`**, with a span | assertion |
| 17 | compiling the same AST twice | byte-identical `Vec<u8>` after `to_bits` — determinism, which the cache key depends on | assertion |

### 12.3 Control flow

| # | Input | Expected | Test kind |
|---|---|---|---|
| 18 | `while i < n { i = i + 1 }` | `JMP_IF_GE %r0, +k` over `LOAD_SMALL_INT %r1, 1; ADD %r0, %r0, %r1; JMP_BACK` — 4 instructions, no `LEN`/`LT` pair | golden |
| 19 | `for x in [10,20,30] { print(x) }` | the 7-instruction sequence of §7.1 | golden |
| 20 | `for k in d { … }` over a dict | `ITER_INIT; ITER_DONE; JMP_IF_FALSE; ITER_NEXT; …` — iterates **keys**, and works, which the old index-loop lowering could not do (`src/bytecode.rs:640-686`) | golden + differential |
| 21 | `for i in 0 -> 1000 { … }` | `MAKE_RANGE` with a **lazy** range; `ITER_INIT` allocates one `IterState`, not 1000 elements. Assert the allocation count with a counting allocator | golden + assertion |
| 22 | `continue` inside a `for` | jumps to the loop **head** (`ITER_DONE`), not to a separate increment block | golden |
| 23 | `return f(x)` as the last statement of a function | `TAIL_CALL`, not `CALL; RETURN` | assertion |
| 24 | `match x { 1: a; 2: b; _:` default }` | `JUMP_TABLE` with a 3-entry constant tuple; a sparse `match` falls back to a compare chain | golden |
| 25 | `x?.y` where `x` is nil | `JMP_IF_NIL` to a `LOAD_NIL` block — does **not** raise | golden + differential |

### 12.4 Functions, closures, containers

| # | Input | Expected | Test kind |
|---|---|---|---|
| 26 | `func make_counter() { var n = 0; return lambda() { n = n + 1; return n } }` | the two-protos output of §9.4, including `OPEN_UPVALUE` and the `pinned` marker in the header comment | golden |
| 27 | `make_counter()` called twice | the two closures get **distinct** cells: `c1() == 1` after `c2() == 1` | differential (this is the `tests/cases/11_closures_and_callables.z` corpus) |
| 28 | two sibling closures over one variable | both descriptors point at the **same** `ParentLocal { reg }`; one `Arc<Upvalue>` | assertion on the descriptor table |
| 29 | a 3-deep nested lambda chain | `upvalue_descriptors` are **flattened** to `ParentUpvalue { index }` entries, max depth 1 per closure, not a runtime chain | assertion |
| 30 | `#{a:1, b:2}` | `MAKE_DICT %r0, 2, %r1` with the shape interned at runtime; **not** folded to a constant | golden |
| 31 | `d.a` where `d` is a dict local | `IC_GET_FIELD %r1, #0, $N"a"` — the IC opcode, not `GET_FIELD` | golden |
| 32 | `d[k]` with a non-constant `k` | `GET_INDEX %r1, %r0, %r2` — **no** IC site, because a dynamic key has no shape to cache on | golden |
| 33 | `f(a, b = 2, *rest, **kw)` | `MAKE_KWARGS` + `CALL_KW`; the callee proto has `HAS_REST | HAS_KWARGS` | golden |
| 34 | `class Counter { func inc(by) { self.n = self.n + by; return self.n } }` | `self` is register 0, `%r1` is `by`; `self.n` is `IC_GET_FIELD`/`IC_SET_FIELD` with a shape IC site | golden |

### 12.5 Errors, cache, and the format

| # | Input | Expected | Test kind |
|---|---|---|---|
| 35 | `try { f() } catch TypeError as e { … } finally { … }` | `TRY_PUSH handler finally; <body>; TRY_POP; JMP; CATCH_MATCH; JMP_IF_FALSE; …; RERAISE` — the layout of §8.1, with the handler, finally and `try_end` values all patched correctly | golden |
| 36 | nested `try` inside `try` | two `HandlerEntry`s with non-overlapping-then-nested ranges; `THROW` resolves to the **inner** one | golden + differential |
| 37 | an unmatched `catch` type | the unwinder falls through the handler to `finally`, then `RERAISE` to the outer handler | differential |
| 38 | `func f(x = 1) { }` | three protos per §10.4, with a `greet$defaults`-style thunk; `load_default %r0, #0` in the body | golden |
| 39 | `func f(x = time()) { }` | a `$defaults$capture` thunk reading a module global; the `.zeno` file caches, and the value is recomputed per process | golden + assertion that the file is written |
| 40 | a value that is not a constant reaches the pool | `CompileError::NonConstant` at **compile** time, not a silent `store()` failure | assertion |
| 41 | a `.zeno` file with one flipped byte in `SECT_CODE` | loader returns `None` (CRC mismatch) and the program is recompiled from source | assertion |
| 42 | a `.zeno` file truncated mid-section | loader returns `None`; **no panic, no partial load** | assertion |
| 43 | a `.zeno` file with `format_ver = 2` | loader returns `None` | assertion |
| 44 | a `.zeno` file with a reserved flag bit set | loader returns `None` | assertion |
| 45 | a `.zeno` file built with `zen-superins` on, loaded with it off | loads and runs correctly; the `0x80`–`0x85` IDs decode | assertion |
| 46 | compiling the whole `tests/cases/` corpus (32 files) | every file compiles, zero `CompileError`, zero tree-walk fallbacks. **This is the M4 exit test** | assertion |

### 12.6 The M4 exit criterion

M4 is done when:

1. All 46 tests above pass.
2. The full 32-file `tests/cases/` corpus compiles with **no** fallback, and the differential
   harness produces byte-identical stdout against the M3 tree-walker for all of them.
3. `zen check --disasm` output for every proto has a golden file.
4. Every `[E]` number in §6.2 and §6.3 that a design decision rests on has been measured and
   either confirmed or the decision amended.

---

## 13. Performance notes

### 13.1 Compile-time budget

| Quantity | Target | Rationale |
|---|---|---|
| **1000-line file, cold (no cache)** | **< 1 ms** **[T]** | Zen 2.x's lexer+parser+compiler is already fast enough that a warm start is dominated by process startup, not compilation ([`00-blueprint-review.md`](00-blueprint-review.md) §4, `bench_startup`). A 1 ms budget keeps a 40-file project under 40 ms of compile, which is under one frame. |
| Per source line | < 1 µs **[T]** | Follows from the above; 1000 lines in 1 ms is exactly 1 µs/line. |
| Per AST node | < 150 ns **[E]** | One `Vec` push, one match arm, one register-allocation operation. CPython's own compiler is in this range. |
| Peak memory for a 1000-line file | < 2 MB **[E]** | The AST dominates; the code array is ~4 bytes per node. |
| Warm start (cache hit) | < 50 µs **[T]** | One `mmap` + one CRC over the payload. The CRC over a 40 KB image is ~10 µs with hardware CRC32C. |

`zen check --stats` prints all of these, and `zen build --time` is the command the numbers come
from. **None of them are [M] yet.**

### 13.2 Why compilation is not on the hot path

This is worth stating explicitly because it is the whole reason M4 is safe to iterate on:

1. **A Zen program is compiled once and executed many times.** The benchmark suite runs each
   benchmark body 10⁶–10⁹ times; the 1000-line file containing it is compiled once. Compile time
   is amortised to below the measurement noise floor.
2. **The REPL and the test suite are the exceptions, and they are small.** A REPL line is < 1 KB and
   a test case is < 200 lines, so both are under 200 µs — below the ~5 ms a human notices, and the
   REPL never feels it. The REPL additionally keeps the `FunctionProto` in memory and recompiles
   only the changed function.
3. **`.zeno` caching removes compilation from repeat runs entirely.** With the defaults fix of
   §10.4, a cached start is `mmap` + CRC, not lex+parse+compile.
4. **The JIT is M11 and runs on a background thread**, on a design where K6 was already accepted
   ([`00-blueprint-review.md`](00-blueprint-review.md) §1). Compilation and JIT compilation are
   different activities with different amortisation, and neither is on the per-iteration path.

The one place compile time *could* matter is `spawn`-ing many short-lived workers, each of which
`import`s the same modules. That is handled by the import cache (one compiled proto per module per
process), not by the compiler.

### 13.3 Instruction-count reduction, honestly

The blueprint's "~40 % fewer instructions than a stack VM" claim is a stack-vs-register claim, not a
NaN-boxing claim ([`00-blueprint-review.md`](00-blueprint-review.md) §A5). Concretely, for
`acc = acc + i` inside a `while i < n` loop:

| | Instructions per iteration |
|---|---|
| Zen 2.2.0 bytecode (`src/bytecode.rs:610-639`, `640-686`) | 14 |
| M4 without peephole | 6 |
| M4 with P0–P3 peephole | 4 |

A **71 % reduction** on that loop, which is consistent with the blueprint's order of magnitude.
Note that M4 gets there *without* superinstructions, which is the point of §11.4: the reduction
comes from the calling convention and the register model, not from opcode fusion.

---

## 14. Rulings a reviewer should double-check

These are the decisions where a reasonable engineer could disagree, ordered by how much damage
getting them wrong would do.

| # | Ruling | Where | Why it might be wrong |
|---|---|---|---|
| 1 | **256 registers, `u8` index, no spill encoding, `TooManyRegisters` as a compile error** | §2.1, §2.3 | If expression nesting or named-binding counts in real code turn out to be much larger than the **≤ 8 [E]** p99 assumed, this becomes a compile error users hit. Mitigation (`LOAD_LOCAL_FRAME`, ID `0x08`) is reserved but unimplemented. **Measure p99 and max nesting depth on the stdlib before M4 exits.** |
| 2 | **Superinstructions exist, are gated off by default, and are permanently disabled under `zen-jit`** | §3.11, §11.4 | The argument that the JIT re-splits them is sound, but the measured interpreter gain is unknown. If it is > 8 % rather than 3–8 %, shipping two bytecode dialects is a real maintenance cost. **Measure `bench_core` both ways before M5.** |
| 3 | **The register-window calling convention, and no operand stack** | §2.5 | The single most load-bearing decision. It is good for Zen's call shapes (few arguments, receiver-first) and *bad* for a function with 40 arguments, where a stack VM's push order costs nothing and a window needs 40 consecutive registers. **Check the stdlib's argument-count distribution.** |
| 4 | **Unified `ADD` with a single `CONCAT` exception** | §3.0 | Ruling 1 is well supported by the JIT-guard argument. The `CONCAT` split is the weakest part: it is justified by string-building being a benchmark, and a reviewer may reasonably call that benchmark-driven design. It is cheap to reverse (delete `0x25`, map it to `ADD`) and that reversibility is the mitigation. |
| 5 | **Lists, dicts and sets are never constant-folded** | §5.2, §11.1 | Defensible, and the ADR-0002 CoW argument means folding them would be *correct*, not a bug. The objection is purely performance: a hot literal list re-allocated per execution. **Measure a list-literal-heavy benchmark with folding enabled as a diagnostic build.** |
| 6 | **Open upvalues pin registers** | §9.2 | The trade is one allocation saved per captured variable per closure creation, against one permanently-unavailable register per captured variable. It could go the other way for capture-heavy functions. **`bench_closure_alloc` decides it.** |
| 7 | **`num_locals > 250` is a compile error** | §2.3 | Code generators and macro expanders can produce wide single-scope blocks. If that turns out to matter, `LOAD_LOCAL_FRAME` is the fix and it is already reserved. |
| 8 | **256 constants per function** | §2.2, §2.4 | Same shape of risk as #1, and the same reserved fix (`LOAD_CONST_W`). |
| 9 | **`TYPE_GUARD` is emitted by M4 and interpreted as a no-op** | §3.13 | It costs code size and compile time today for a benefit that only lands in M11. The counter-argument is that a `.zeno` file must be byte-identical across the M4→M11 boundary, and that is a real constraint, not a preference. |
| 10 | **The `.zeno` key includes `SUPERINS_ENABLED`** | §10.6 | Correct but it means two builds of the same Zen version have mutually useless caches. The alternative — making superinstructions a load-time rewrite instead of a compile-time one — costs a pass on every cache hit. |
| 11 | **CRC-32C over the payload, no signature** | §10.1 | Adequate for corruption, which is the stated threat. If a `.zeno` file is ever distributed as an untrusted artefact, this is not a security measure and would need a signature. Noted in the manual. |
| 12 | **Fused compare-branches are core; no `JMP_IF_TRUE`** | §3.6, §11.2 | The two rulings pull in opposite directions and the net effect is an asymmetry (four fused comparisons, no fused `true` branch) that a reader will notice. Both are deliberate; both are on this list so the reviewer decides rather than inherits them. |

---

*Cross-references: the VM that executes this bytecode is [`08-vm.md`](08-vm.md). The value
representation is [ADR-0001](decisions/ADR-0001-tag-budget.md) and the allocation strategy is
[ADR-0002](decisions/ADR-0002-memory-model.md). The rationale for the opcode count, the build order
and the performance gate is [`00-blueprint-review.md`](00-blueprint-review.md) §A5, §A6 and §4.*

