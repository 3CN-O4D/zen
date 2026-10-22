//! Hidden classes ("shapes") for instance field access.
//!
//! A shape records the ordered set of data properties an instance has, mapping
//! each property name to the slot index its value occupies in the instance's
//! field slots (`slots[1 + offset]`, with `slots[0]` holding the class).
//! Shapes are immutable and process-wide: ids are never reused, so a stale
//! inline-cache guard is a miss rather than a wrong answer.
//!
//! The registry is global (not per-VM) so the shared `ops::get_field` /
//! `ops::set_field` implementations — used by the VM, the tree-walking
//! interpreter, and native bridges alike — can resolve shapes without a `Vm`
//! handle. Only slow paths touch it (fast paths run off baked offsets), so a
//! lock per lookup is noise-free in practice: shapes are looked up by id and
//! never mutated in place, and transitions are memoized.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// One hidden class. `properties` maps a data-property name to its slot.
#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub properties: HashMap<String, usize>,
}

impl Shape {
    /// The root shape: an instance with no data properties yet.
    pub fn root() -> Self {
        Shape {
            properties: HashMap::new(),
        }
    }

    /// The slot index of `name`, if this shape carries it.
    #[inline]
    pub fn offset_of(&self, name: &str) -> Option<usize> {
        self.properties.get(name).copied()
    }

    /// Derive the shape that adds `name` as the next slot. The caller is
    /// responsible for memoising the transition so identical shapes are
    /// shared.
    pub fn add(&self, name: &str) -> Shape {
        let mut next = self.clone();
        next.properties
            .insert(name.to_string(), self.properties.len());
        next
    }
}

/// Process-wide shape registry (see the module note).
struct Registry {
    shapes: Vec<Shape>,
    transitions: HashMap<u32, HashMap<String, u32>>,
}

static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();

fn registry() -> &'static RwLock<Registry> {
    REGISTRY.get_or_init(|| {
        RwLock::new(Registry {
            // Id 0 is the root shape, matching every fresh instance.
            shapes: vec![Shape::root()],
            transitions: HashMap::new(),
        })
    })
}

fn lock_read() -> std::sync::RwLockReadGuard<'static, Registry> {
    registry().read().unwrap_or_else(|e| e.into_inner())
}

fn lock_write() -> std::sync::RwLockWriteGuard<'static, Registry> {
    registry().write().unwrap_or_else(|e| e.into_inner())
}

/// Slot index of `name` under `shape`, or `None` if the shape lacks it.
#[inline]
pub fn shape_offset(shape: u32, name: &str) -> Option<usize> {
    lock_read()
        .shapes
        .get(shape as usize)
        .and_then(|s| s.offset_of(name))
}

/// Property count of `shape` (0 for unknown ids). Shapes never shrink, so the
/// count doubles as the next free slot index.
#[inline]
pub fn shape_len(shape: u32) -> usize {
    lock_read()
        .shapes
        .get(shape as usize)
        .map(|s| s.properties.len())
        .unwrap_or(0)
}

/// The shape that adds `name` to `shape`, creating (and memoising) it on
/// first use. Deterministic: the same `(shape, name)` always yields the same
/// id, so concurrent transitions agree without coordination.
pub fn shape_transition(shape: u32, name: &str) -> u32 {
    if let Some(next) = lock_read()
        .transitions
        .get(&shape)
        .and_then(|m| m.get(name))
    {
        return *next;
    }
    let mut reg = lock_write();
    if let Some(next) = reg.transitions.get(&shape).and_then(|m| m.get(name)) {
        return *next;
    }
    let base = reg.shapes.get(shape as usize).cloned().unwrap_or_default();
    let next_shape = base.add(name);
    let id = reg.shapes.len() as u32;
    reg.shapes.push(next_shape);
    reg.transitions
        .entry(shape)
        .or_default()
        .insert(name.to_string(), id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_has_no_properties() {
        let root = Shape::root();
        assert!(root.offset_of("x").is_none());
    }

    #[test]
    fn add_assigns_sequential_slots() {
        let root = Shape::root();
        let s1 = root.add("x");
        assert_eq!(s1.offset_of("x"), Some(0));
        assert_eq!(s1.offset_of("y"), None);
        let s2 = s1.add("y");
        assert_eq!(s2.offset_of("x"), Some(0));
        assert_eq!(s2.offset_of("y"), Some(1));
        // `add` never mutates its receiver.
        assert_eq!(s1.offset_of("y"), None);
    }
}
