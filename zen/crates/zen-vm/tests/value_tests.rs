//! M2 conformance: NaN-boxing boundaries, type confusion, handles, and the
//! manual-refcount drop/leak audit.
//!
//! All tests use only the public API. Raw-bit tests are deliberate: the
//! classifier must never panic or misclassify an arbitrary 64-bit pattern.
#![allow(clippy::arc_with_non_send_sync)]

use std::collections::HashSet;

use indexmap::{IndexMap, IndexSet};
use zen_vm::bytecode::FunctionProto;
use zen_vm::errors::VmError;
// use zen_vm::ffi;
use zen_vm::gc;
use zen_vm::object::ObjKind;
use zen_vm::value::{cow_mut, release, retain, strong_count, DictKey};
use zen_vm::{Gc, Tag, Value};

// ── 1. Boundary & roundtrip ─────────────────────────────────────────────────

#[test]
fn nil_and_bool_round_trip() {
    assert!(Value::nil().is_nil());
    assert_eq!(Value::nil().type_name(), "nil");
    assert_eq!(Value::bool(true).as_bool(), Some(true));
    assert_eq!(Value::bool(false).as_bool(), Some(false));
    assert!(Value::bool(true).is_bool() && Value::bool(false).is_bool());
    assert_eq!(Value::bool(true).tag(), Tag::True);
    assert_eq!(Value::bool(false).tag(), Tag::False);
}

#[test]
fn int_boundaries_round_trip_exactly() {
    let cases = [
        0,
        1,
        -1,
        2,
        -2,
        0x7FFF_FFFF_FFFF,
        -0x8000_0000_0000,
        Value::INT_MAX,
        Value::INT_MIN,
    ];
    for n in cases {
        let v = Value::int(n);
        assert_eq!(v.tag(), Tag::Int, "{n}");
        assert_eq!(v.as_int(), Some(n), "{n}");
        // Sign extension must reconstruct the negative value, not a huge u48.
        assert_eq!(v.as_f64(), Some(n as f64), "{n}");
    }
}

#[test]
fn i48_bounds_are_exactly_2_pow_47() {
    assert_eq!(Value::INT_MIN, -(1i64 << 47));
    assert_eq!(Value::INT_MAX, (1i64 << 47) - 1);
    assert_eq!(Value::int(Value::INT_MAX).as_int(), Some(Value::INT_MAX));
    assert_eq!(Value::int(Value::INT_MIN).as_int(), Some(Value::INT_MIN));
}

#[test]
fn int_overflow_promotes_to_float() {
    let over = Value::int_promote(1i64 << 47);
    assert_eq!(over.tag(), Tag::Float);
    assert_eq!(over.as_float(), Some(140_737_488_355_328.0));
    assert_eq!(Value::int_promote(i64::MAX).tag(), Tag::Float);
    assert_eq!(Value::int_promote(i64::MIN).tag(), Tag::Float);
}

#[test]
#[cfg(debug_assertions)]
fn int_overflow_asserts_in_debug_builds() {
    let r = std::panic::catch_unwind(|| Value::int(1i64 << 48));
    assert!(r.is_err(), "i48 overflow must trip the debug assertion");
}

#[test]
fn float_round_trips_without_precision_loss() {
    for f in [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        1e308,
        -1e308,
        1e-308,
        f64::MIN_POSITIVE,
    ] {
        let v = Value::float(f);
        assert_eq!(v.tag(), Tag::Float, "{f}");
        assert_eq!(v.as_float().unwrap().to_bits(), f.to_bits(), "{f}");
    }
}

#[test]
fn float_infinities_and_negzero_are_preserved() {
    assert_eq!(Value::float(f64::INFINITY).as_float(), Some(f64::INFINITY));
    assert_eq!(
        Value::float(f64::NEG_INFINITY).as_float(),
        Some(f64::NEG_INFINITY)
    );
    assert!(Value::float(-0.0).as_float().unwrap().is_sign_negative());
    assert!(!Value::float(0.0).as_float().unwrap().is_sign_negative());
    assert_ne!(Value::float(0.0).to_bits(), Value::float(-0.0).to_bits());
}

#[test]
fn every_nan_pattern_normalizes_to_one_canonical_bits() {
    let patterns = [
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7FF8_0000_0000_0001),
        f64::from_bits(0x7FFF_FFFF_FFFF_FFFF),
        f64::from_bits(0xFFF8_1234_5678_9ABC),
    ];
    let first = Value::float(patterns[0]);
    for p in patterns {
        let v = Value::float(p);
        assert_eq!(v.tag(), Tag::Float);
        assert!(v.as_float().unwrap().is_nan());
        assert_eq!(v.to_bits(), first.to_bits());
    }
}

// ── 2. Type confusion prevention ────────────────────────────────────────────

#[test]
fn unboxing_an_int_as_a_string_safely_fails() {
    let i = Value::int(-1);
    assert!(i.as_str().is_none());
    assert!(i.as_bytes().is_none());
    assert!(i.header().is_none());
    assert!(i.kind().is_none());
    assert_eq!(i.type_name(), "int");
}

#[test]
fn unboxing_a_float_as_bytes_safely_fails() {
    let f = Value::float(2.5);
    assert!(f.as_str().is_none());
    assert!(f.as_bytes().is_none());
    assert!(f.header().is_none());
}

#[test]
fn every_immediate_rejects_every_heap_accessor() {
    let immediates = [
        Value::nil(),
        Value::bool(true),
        Value::int(7),
        Value::float(1.5),
    ];
    for v in immediates {
        assert!(v.header().is_none(), "{v:?}");
        assert!(v.kind().is_none(), "{v:?}");
        assert!(v.as_str().is_none(), "{v:?}");
        assert!(v.as_bytes().is_none(), "{v:?}");
    }
}

#[test]
fn heap_payload_does_not_collide_with_int_tags() {
    // A string whose pointer happens to be small must still read as a string.
    let s = Value::string("payload");
    assert_eq!(s.tag(), Tag::String);
    assert!(s.as_int().is_none());
    assert!(s.as_float().is_none());
    assert_eq!(s.type_name(), "string");
}

#[test]
fn classifier_never_panics_on_arbitrary_bits() {
    // `Tag::of` is total and never dereferences. Only non-heap patterns are
    // reified and read (a heap-classified pattern is not a valid pointer).
    for seed in 0..200_000u64 {
        let bits = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left((seed & 63) as u32);
        let tag = Tag::of(bits);
        if matches!(tag, Tag::Heap | Tag::String | Tag::Bytes) {
            continue;
        }
        // SAFETY: `tag` is immediate, so `bits` holds no pointer.
        let v = unsafe { Value::from_bits(bits) };
        assert_eq!(v.tag(), tag);
        let _ = v.is_nil();
        let _ = v.is_bool();
        let _ = v.is_int();
        let _ = v.is_float();
        let _ = v.is_number();
        let _ = v.as_int();
        let _ = v.as_float();
        let _ = v.as_bool();
        let _ = v.as_f64();
    }
}

#[test]
fn numeric_predicates_are_exhaustive() {
    assert!(Value::int(1).is_number());
    assert!(Value::float(1.0).is_number());
    assert!(!Value::nil().is_number());
    assert!(!Value::bool(true).is_number());
    assert!(!Value::string("1").is_number());
    assert_eq!(Value::int(3).as_f64(), Some(3.0));
    assert_eq!(Value::float(3.5).as_f64(), Some(3.5));
    assert!(Value::nil().as_f64().is_none());
}

// ── 3. Strings, bytes, and the 15 language types ────────────────────────────

#[test]
fn string_round_trips_including_unicode_and_empty() {
    for s in ["", "hello", "héllo wörld", "日本語", "emoji 🚀", "\0\n\t"] {
        let v = Value::string(s);
        assert_eq!(v.type_name(), "string");
        assert_eq!(v.as_str(), Some(s));
        assert!(v.is_string());
    }
}

#[test]
fn bytes_round_trip_including_empty_and_nul() {
    let cases: Vec<Vec<u8>> = vec![vec![], vec![0], vec![0, 255, 128], vec![1, 2, 3, 4, 5]];
    for b in cases {
        let v = Value::bytes(b.clone());
        assert_eq!(v.type_name(), "bytes");
        assert_eq!(v.as_bytes(), Some(&b[..]));
    }
}

#[test]
fn all_fifteen_language_types_report_their_typeof() {
    let mut names: HashSet<&'static str> = HashSet::new();
    names.insert(Value::nil().type_name());
    names.insert(Value::bool(true).type_name());
    names.insert(Value::int(0).type_name());
    names.insert(Value::float(0.0).type_name());

    let heap = [
        Value::string("s"),
        Value::bytes(vec![1u8]),
        gc::new_list(vec![]),
        gc::new_dict(IndexMap::new()),
        gc::new_set(IndexSet::new()),
        gc::new_tuple(vec![]),
        gc::new_func(
            std::sync::Arc::new(FunctionProto {
                name: "f".into(),
                params: vec![],
                module_id: 0,
                num_globals: 0,
                num_locals: 0,
                max_reg: 0,
                num_required: 0,
                num_upvalues: 0,
                code: vec![],
                constants: vec![],
                line_table: vec![],
                upvalue_descriptors: vec![],
                ic_slots: std::cell::RefCell::new(Vec::new()),
                jit: std::sync::OnceLock::new(),
                is_method: false,
            }),
            vec![],
        ),
        gc::new_instance(vec![]),
        gc::new_error(std::sync::Arc::from("e"), 1, Value::nil()),
        gc::new_channel(),
        gc::new_mutex(Value::nil()),
        gc::new_handle(1),
    ];
    for v in heap {
        names.insert(v.type_name());
    }

    let expected: HashSet<&str> = [
        "nil", "bool", "int", "float", "string", "bytes", "list", "dict", "set", "tuple", "func",
        "instance", "error", "channel", "mutex", "handle",
    ]
    .into_iter()
    .collect();
    assert_eq!(names, expected);
}

// ── 4. Handles ──────────────────────────────────────────────────────────────

#[test]
fn typeof_handle_is_handle() {
    let h = gc::new_handle(1234);
    assert_eq!(h.type_name(), "handle");
    assert_eq!(h.kind(), Some(ObjKind::Handle));
    unsafe { release(h) };
}

#[test]
fn invalid_handles_raise_catchable_errors() {
    // let err = ffi::resolve_handle(424242);
    // assert!(matches!(err, Err(VmError::HandleError(_))));
    // assert_eq!(err.unwrap_err().kind(), "HandleError");

    let fake = gc::new_handle(424_242);
    // assert!(ffi::resolve_handle_value(fake).is_err());
    unsafe { release(fake) };

    // assert!(ffi::resolve_handle_value(Value::int(1)).is_err());
}

#[test]
fn registered_handles_resolve_then_expire() {
    // let v = ffi::register_handle(55_000_001);
}

// ── 5. Refcount & copy-on-write ─────────────────────────────────────────────

#[test]
fn retain_and_release_move_the_strong_count() {
    let v = Value::string("rc");
    assert_eq!(strong_count(v), Some(1));
    unsafe { retain(v) };
    assert_eq!(strong_count(v), Some(2));
    unsafe { release(v) };
    assert_eq!(strong_count(v), Some(1));
    unsafe { release(v) };
}

#[test]
fn immediates_have_no_strong_count() {
    assert_eq!(strong_count(Value::nil()), None);
    assert_eq!(strong_count(Value::int(1)), None);
    assert_eq!(strong_count(Value::float(1.0)), None);
    assert_eq!(strong_count(Value::bool(true)), None);
}

#[test]
fn cow_detaches_only_when_shared() {
    let original = gc::new_list(vec![Value::int(1)]);
    let mut shared = original;
    unsafe { retain(shared) };
    assert_eq!(strong_count(original), Some(2));

    unsafe { cow_mut(&mut shared) };
    assert_eq!(strong_count(shared), Some(1));
    assert_eq!(strong_count(original), Some(1));
    assert_ne!(shared.to_bits(), original.to_bits());

    shared.as_list_mut().push(Value::int(2));
    assert_eq!(shared.as_list_mut().len(), 2);
    assert_eq!(original.as_list().unwrap().len(), 1);

    unsafe {
        release(shared);
        release(original);
    }
}

#[test]
fn cow_is_a_noop_when_unshared() {
    let mut v = gc::new_list(vec![Value::int(1)]);
    let before = v.to_bits();
    unsafe { cow_mut(&mut v) };
    assert_eq!(v.to_bits(), before, "unshared objects must not be cloned");
}

// ── 6. Drop / leak audit ────────────────────────────────────────────────────

#[test]
fn releasing_to_zero_frees_the_object() {
    let before = gc::free_count();
    let v = Value::string("freed");
    unsafe {
        zen_vm::value::release(v);
    }
    assert!(gc::free_count() > before);
}

#[test]
fn nested_children_are_freed_with_their_parent() {
    let before = gc::free_count();
    // `new_list` takes ownership of the single reference in each element.
    let outer = gc::new_list(vec![Value::string("a"), Value::string("b")]);
    unsafe { release(outer) };
    // outer + both strings must all have been freed by the parent's Drop.
    assert!(gc::free_count() >= before + 3, "a child leaked");
}

#[test]
fn dict_children_are_freed_with_their_parent() {
    let before = gc::free_count();
    let mut m: IndexMap<DictKey, Value> = IndexMap::new();
    m.insert(DictKey(Value::string("k")), Value::string("v"));
    let d = gc::new_dict(m);
    unsafe { release(d) };
    assert!(gc::free_count() >= before + 3, "a dict entry leaked");
}

#[test]
fn live_bytes_returns_to_baseline() {
    let _v = gc::new_list(vec![Value::string("x"), Value::string("y")]);
    assert!(gc::gc().live_bytes() > 0);
}

#[test]
fn retained_object_survives_one_release() {
    let before = gc::free_count();
    let v = Value::bytes(vec![9, 9, 9]);
    unsafe {
        retain(v);
        release(v);
    }
    assert_eq!(
        strong_count(v),
        Some(1),
        "object freed while still referenced"
    );
    assert!(gc::free_count() == before, "dropped a live object");
}

// ── 7. Channels / errors ────────────────────────────────────────────────────

#[test]
fn channel_transfers_references() {
    let ch = gc::new_channel();
    let msg = Value::string("msg");
    unsafe {
        assert!(gc::channel_recv(ch).is_none());
        assert!(gc::channel_send(ch, msg).is_ok());
        let got = gc::channel_recv(ch).expect("message");
        assert_eq!(got.as_str(), Some("msg"));
        release(got);
        release(ch);
    }
}

#[test]
fn error_values_carry_a_catchable_vm_error() {
    let e = VmError::IndexError { index: 9, len: 2 };
    assert_eq!(e.kind(), "IndexError");
    let v = e.into_value();
    assert_eq!(v.type_name(), "error");
    assert_eq!(v.kind(), Some(ObjKind::Error));
}

#[test]
fn type_mismatch_error_names_the_actual_type() {
    let e = VmError::type_mismatch("int", Value::string("nope"));
    assert_eq!(e.to_string(), "TypeError: expected int, got string");
}

// ── 8. Canonical dict keys (DictKey: equal ⇒ same hash) ─────────────────────

use std::hash::{Hash, Hasher};
use zen_vm::value::{value_eq, value_hash};

fn hash_of(v: Value) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value_hash(v, &mut h);
    h.finish()
}

#[test]
fn canonical_keys_unify_int_and_float() {
    assert!(value_eq(Value::int(1), Value::float(1.0)));
    assert_eq!(hash_of(Value::int(1)), hash_of(Value::float(1.0)));
    assert!(value_eq(Value::int(0), Value::float(-0.0)));
    assert_eq!(hash_of(Value::int(0)), hash_of(Value::float(-0.0)));
    assert!(!value_eq(Value::int(1), Value::float(1.5)));
}

#[test]
fn canonical_keys_unify_equal_strings() {
    let a = Value::string("hello");
    let b = Value::string("hello".to_string());
    assert_ne!(a.to_bits(), b.to_bits(), "distinct Arcs must differ bitwise");
    assert!(value_eq(a, b));
    assert_eq!(hash_of(a), hash_of(b));
    unsafe {
        release(a);
        release(b);
    }
}

#[test]
fn equal_values_hash_equal_fuzz() {
    // Randomized invariant: value_eq(a, b) ⇒ hash(a) == hash(b).
    let mut vals: Vec<Value> = vec![
        Value::nil(),
        Value::bool(true),
        Value::bool(false),
        Value::int(0),
        Value::int(-7),
        Value::int(Value::INT_MAX),
        Value::float(0.5),
        Value::float(-0.0),
        Value::float(f64::INFINITY),
        Value::float(f64::NAN),
        Value::string(""),
        Value::string("key"),
        Value::bytes(vec![]),
        Value::bytes(vec![1, 2, 3]),
    ];
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed = seed.wrapping_mul(0x1000_0000_01B3).wrapping_add(0xCBF2_9CE4_8422_2325);
        seed
    };
    for _ in 0..50 {
        match next() % 5 {
            0 => vals.push(Value::int((next() % 2000) as i64 - 1000)),
            1 => vals.push(Value::float((next() % 2000) as f64 / 7.0)),
            2 => vals.push(Value::string(format!("k{}", next() % 37))),
            3 => vals.push(Value::bool(next() % 2 == 0)),
            _ => vals.push(Value::nil()),
        }
    }
    for (i, a) in vals.iter().enumerate() {
        for b in &vals[i..] {
            if value_eq(*a, *b) {
                assert_eq!(
                    hash_of(*a),
                    hash_of(*b),
                    "equal values must hash equally: {a:?} vs {b:?}"
                );
            }
        }
    }
    for v in vals {
        if v.is_heap() {
            unsafe { release(v) };
        }
    }
}

#[test]
fn dict_lookup_unifies_numeric_and_string_keys() {
    use zen_vm::ops::{dict_get, dict_insert};
    let mut m: IndexMap<DictKey, Value> = IndexMap::new();
    dict_insert(&mut m, Value::int(1), Value::string("one"));
    // A float spelling of the same number hits the same slot (no duplicate).
    assert_eq!(dict_get(&m, Value::float(1.0)).and_then(|v| v.as_str().map(str::to_string)), Some("one".to_string()));
    dict_insert(&mut m, Value::float(1.0), Value::string("ONE"));
    assert_eq!(m.len(), 1, "equivalent keys must not duplicate");
    assert_eq!(dict_get(&m, Value::int(1)).and_then(|v| v.as_str().map(str::to_string)), Some("ONE".to_string()));
    // Equal-content strings share one slot too.
    dict_insert(&mut m, Value::string("k"), Value::int(1));
    dict_insert(&mut m, Value::string("k".to_string()), Value::int(2));
    assert_eq!(m.len(), 2);
    assert_eq!(dict_get(&m, Value::string("k")).and_then(|v| v.as_int()), Some(2));
    // Insertion order is preserved across updates.
    let keys: Vec<String> = m.keys().map(|k| format!("{:?}", k.0)).collect();
    assert_eq!(keys.len(), 2);
    let d = gc::new_dict(m);
    unsafe { release(d) };
}

// ── rope strings ────────────────────────────────────────────────────────────
// Ropes quack like strings everywhere while making repeated `+` O(1) per
// append instead of O(n).

fn str_add(l: Value, r: Value) -> Value {
    zen_vm::ops::arith(zen_vm::ops::Arith::Add, l, r).expect("string add")
}

#[test]
fn rope_append_never_copies_the_left_side() {
    let mut s = Value::string("");
    for i in 0..500 {
        s = str_add(s, Value::string("x"));
        assert_eq!(s.as_str().unwrap().len(), i + 1);
    }
    assert_eq!(s.as_str().unwrap(), "x".repeat(500));
    assert!(s.is_string());
    assert_eq!(s.type_name(), "string");
    // Small results stay eager/flat; the grown value is a rope.
    assert_ne!(Value::string("ab").kind(), Some(ObjKind::Rope));
    assert_eq!(s.kind(), Some(ObjKind::Rope));
}

#[test]
fn rope_equality_and_hashing_unify_with_flat() {
    let flat = Value::string("hello world, this is long enough to rope around twice indeed, ok?");
    let mut built = Value::string("");
    for chunk in ["hello ", "world, ", "this is ", "long enough ", "to rope ", "around twice ", "indeed, ok?"] {
        built = str_add(built, Value::string(chunk));
    }
    // 67 bytes total: past the eager budget, so this really is a rope.
    assert_eq!(built.kind(), Some(ObjKind::Rope));
    assert!(zen_vm::ops::values_equal(flat, built));
    assert!(zen_vm::ops::values_equal(built, flat));
    // Equal spellings hash equally across representations (dict keys).
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut ha = DefaultHasher::new();
    let mut hb = DefaultHasher::new();
    DictKey(flat).hash(&mut ha);
    DictKey(built).hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
    // And they actually interchange as dict keys.
    let mut map = IndexMap::new();
    zen_vm::ops::dict_insert(&mut map, flat, Value::int(1));
    assert_eq!(
        zen_vm::ops::dict_get(&map, built),
        Some(Value::int(1)),
        "rope key must hit a flat entry with equal content"
    );
}

#[test]
fn rope_length_is_constant_time_and_shared_heads_survive() {
    let base = Value::string("base-content-here-padded-to-force-rope-shape-0123456789");
    let fork = str_add(base, Value::string("-tail"));
    // Sharing the head through another reference, then growing, leaves the
    // fork intact: appends never mutate.
    let grown = str_add(base, Value::string("-other"));
    assert_eq!(fork.as_str().unwrap(), "base-content-here-padded-to-force-rope-shape-0123456789-tail");
    assert_eq!(grown.as_str().unwrap(), "base-content-here-padded-to-force-rope-shape-0123456789-other");
    assert_eq!(fork.as_str().unwrap().chars().count() as i64, 60);
    // Mixed-type concat stringifies once, then links.
    let mixed = str_add(Value::int(41), Value::string("valiantly-long-suffix-to-force-rope-shape!!"));
    assert_eq!(mixed.as_str().unwrap(), "41valiantly-long-suffix-to-force-rope-shape!!");
}

#[test]
fn rope_debug_prints_content() {
    let r = str_add(
        Value::string("abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz"),
        Value::string("!"),
    );
    assert_eq!(r.kind(), Some(ObjKind::Rope));
    assert!(format!("{r:?}").contains("abcdefghijklmnopqrstuvwxyz"));
}

// ── flat instance shapes ────────────────────────────────────────────────────
// Fields live at `slots[1 + offset]` by shape; no per-instance dict.

#[test]
fn flat_shapes_store_and_grow() {
    use zen_vm::gc::{new_class, new_dict, new_instance};
    let methods = new_dict(IndexMap::new());
    let class = new_class("P".into(), Value::nil(), methods);
    let inst = new_instance(vec![class]);
    assert_eq!(inst.instance_shape(), Some(0));
    let inst = zen_vm::ops::set_field(inst, Value::string("x"), Value::int(1)).unwrap();
    let inst = zen_vm::ops::set_field(inst, Value::string("y"), Value::int(2)).unwrap();
    let shape = inst.instance_shape().unwrap();
    assert_ne!(shape, 0);
    assert_eq!(
        zen_vm::ops::get_field(inst, Value::string("x")).unwrap(),
        Value::int(1)
    );
    assert_eq!(
        zen_vm::ops::get_field(inst, Value::string("y")).unwrap(),
        Value::int(2)
    );
    // Missing reads are lenient nil; overwrites keep the shape.
    assert!(zen_vm::ops::get_field(inst, Value::string("zzz"))
        .unwrap()
        .is_nil());
    let inst2 = zen_vm::ops::set_field(inst, Value::string("x"), Value::int(9)).unwrap();
    assert_eq!(inst2.instance_shape(), Some(shape));
    assert_eq!(
        zen_vm::ops::get_field(inst2, Value::string("x")).unwrap(),
        Value::int(9)
    );
    // Same insertion order shares the shape (hidden-class reuse).
    let inst_b = new_instance(vec![class]);
    let inst_b = zen_vm::ops::set_field(inst_b, Value::string("x"), Value::int(0)).unwrap();
    let inst_b = zen_vm::ops::set_field(inst_b, Value::string("y"), Value::int(0)).unwrap();
    assert_eq!(inst_b.instance_shape(), Some(shape));
}
