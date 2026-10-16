# ADR-0005 — Module import paths and native/`.z` tier selection

**Status:** Accepted · **Date:** 2026-10-05

## Context

Two things were unresolved and interacted badly.

**Import spelling.** `crates/zen-cli/src/linker.rs` joins an import path verbatim into a filename
(`fs` → `fs.z`, `a.b` → `a/b.z`) and looks under the entry file's directory, then under `std/`. Every
source file in this repository — `std/*.z` and all 30-odd `tests/cases/*.z` — imports by **bare
name**: `import fs`, `import sys`, `import json`. Nothing imports `std.fs`.

The native registry nonetheless registered each host module under *two* keys, the bare name and
`std.<name>`, on the theory that both spellings should link. That theory is wrong twice over. It
invites a module to be reachable under a name no file agrees with, and it guarantees that whichever
spelling a user picks, the registry is doing something the linker never asked for.

**Tier selection.** 15 stdlib modules ship a pure-Zen `.z` tier, and each declares at its top level:

```
ZEN_PURE = true
```

with a header comment promising that "the `.z` tier wins over the Rust tier on import, the native
tier stays reachable as `<mod>["native"]` (`.native` is a lexer keyword, so bracket access is the
escape hatch), and `ZEN_NATIVE_ONLY` restores the native tier."

None of that was implemented. There were two places that consulted `native_modules()`, and they
disagreed: `link()` checked natives first, and `resolve_module()` *also* short-circuited to `None`
for any name the registry knew. The comment on that second check called the behaviour "handled
separately in link()" — but `link()` interpreted the `None` as "no source file, fall back to a
native", so a native silently shadowed the `.z` tier. `8` of the test cases depend on the promised
escape hatch and all of them failed with `KeyError: native`.

## Decision

**Bare names are the only canonical spelling.** The registry holds bare keys — `fs`, `os`, `time`,
`socket`, `ffi` — and nothing else. `std.fs` does not link. A dotted path names a module *package*,
not a stdlib namespace; `std` is a directory in the search path, not a prefix in the name.

**The linker owns tier selection, and the registry is inert data.** `native_modules()` is a plain
`HashMap<String, ModuleUnit>` with no opinion about precedence. `resolve_module()` no longer consults
it — it answers "is there a source file", full stop. Only `link()` chooses, in this order:

1. `ZEN_NATIVE_ONLY=<mod>` names this module → use the native tier if one exists.
2. Otherwise, a `.z` file exists → use it. If it declares `ZEN_PURE = true` **and** a native of the
   same name exists, attach the native as the module's `native` export member.
3. Otherwise, a native exists → use it.
4. Otherwise → `module '<name>' not found`.

So a `.z` tier wins over a same-named native, and `ZEN_PURE` is what makes the pair coherent rather
than accidental. Host modules with no source file (`fs`, `socket`) fall through to step 3 and are
imported by bare name.

**The attached native is registered under `<name>.native`,** not `<name>`. A module's export
dictionary is published *by* its `Import` of its own name, so reusing the bare key would make the
module resolve to the half-built dictionary it is currently constructing. The suffixed key also
means `import json.native` and `json["native"]` reach the same dictionary, which is what the stdlib
headers already describe.

**`ZEN_PURE` is read from the AST,** by matching a top-level `ZEN_PURE = true` assignment. Not from
the source text: every pure stdlib header names the symbol in its own comment, so a textual scan
would match all 15 modules and defeat the whole mechanism. Only a plain `=` counts —
`ZEN_PURE += true` is not opting in.

**`native_modules()` registers no `.native` key of its own.** The `<name>.native` key is the linker's
to mint; a hard-coded one would collide with it.

## Consequences

- `import fs` is the only spelling, and it means the same thing in `std/*.z`, in `tests/cases/*.z`,
  and in the registry. One name, one resolution path.
- `std/json.z` is reachable as `import json` and keeps its Rust tier at `json["native"]`. The pure
  tier is now actually the default, which is what the header always claimed.
- Module bodies compile to a bare register window with **no trailing `Return`** — the frame runs off
  the end. Code that appends to a module's instruction list must therefore insert before an explicit
  `Return` if one exists and append otherwise. Assuming a trailing `Return` silently splices into the
  middle of the body and clobbers a live temporary. (This was found the hard way: it corrupted
  unrelated module exports.)
- The `native` member costs one extra global slot and one register above `num_locals` per attached
  module, and one extra `ModuleUnit` clone per program that imports one.
- Tier selection is now decided in one function with four ordered cases, testable without a VM. It is
  also the reason a missing Rust tier is now a *visible* `KeyError: native` rather than a silent
  shadow: modules that lost their Rust implementation (see below) announce themselves.
- `ZEN_NATIVE_ONLY` is read from the environment per module name, comma-separated. It is a debugging
  and differential-testing lever, not a supported deployment switch.

### Known consequence: several modules have no Rust tier

The escape hatch now works, which exposes the fact that for `base64`, `binascii`, `collections`,
`itertools`, `json`, `math`, `random`, `string`, and `tempfile` the Rust half no longer exists — the
registry has only `fs`, `os`, `time`, `socket`, `ffi`, `json`. Those modules are pure-Zen only, so
`mod["native"]` correctly reports `KeyError: native`, and 8 test cases that assert the escape hatch
stay red until the implementations are rewritten. That work is deliberately not in this ADR's scope:
it is a body of code, not a naming decision.

## Alternatives rejected

- **Keep both spellings in the registry.** Rejected: it makes the registry a second, silent source
  of resolution truth, and no caller of `std.<name>` exists to justify it.
- **`std.` as a required prefix.** Rejected: would mean rewriting every import in `std/` and
  `tests/cases/`, and `std` is already a directory in the search path, so the prefix would be
  redundant with the filesystem rather than with it.
- **Natives win over `.z`, `ZEN_PURE` ignored.** Rejected: that was the bug. It made the pure tier
  dead code for every module that ships both, and made the `ZEN_PURE` declarations and their headers
  a lie.
- **Detect `ZEN_PURE` textually.** Rejected: matches the comment in all 15 headers.
- **Rename `.native` to a private key.** Rejected: making it importable is a feature — it lets a
  test compare tiers directly, which is what `21_pure_zen.z` is for.