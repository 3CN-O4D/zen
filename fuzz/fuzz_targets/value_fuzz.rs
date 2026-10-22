//! M2 fuzz target: total classification, total constructors, and a bits
//! round-trip. Heap-classified patterns are never reified because fuzzer bits
//! are not a valid object pointer.
//!
//! Run with `cargo fuzz run value_fuzz -- -runs=1000000` (M2 gate).

#![no_main]

use libfuzzer_sys::fuzz_target;
use zen_vm::value::release;
use zen_vm::{Tag, Value};

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let bits = u64::from_le_bytes(data[..8].try_into().unwrap());

    // Classification is total over arbitrary bits and never dereferences.
    let tag = Tag::of(bits);
    let is_pointer = matches!(tag, Tag::Heap | Tag::String | Tag::Bytes);
    if !is_pointer {
        // SAFETY: `tag` is an immediate, so `bits` is a valid immediate value.
        let v = unsafe { Value::from_bits(bits) };
        assert_eq!(v.tag(), tag);
        let _ = v.as_int();
        let _ = v.as_float();
        let _ = v.as_bool();
        let _ = v.as_f64();
        let _ = format!("{v:?}");
        assert_eq!(v.to_bits(), bits);
    }

    // Constructors are total.
    let n = i64::from_le_bytes(data[..8].try_into().unwrap());
    let promoted = Value::int_promote(n);
    assert!(promoted.is_int() || promoted.is_float());

    let f = f64::from_bits(bits);
    let w = Value::float(f);
    assert_eq!(w.tag(), Tag::Float);
    assert_eq!(w.as_float().unwrap().is_nan(), f.is_nan());

    // Strings built from the trailing bytes round-trip.
    if let Ok(s) = std::str::from_utf8(&data[8..]) {
        let sv = Value::string(s);
        assert_eq!(sv.as_str(), Some(s));
        // SAFETY: `sv` is a live string value owned by this scope.
        unsafe { release(sv) };
    }
});
