//! Inline caches for field access.
//!
//! Each cache guards on the receiver's [`crate::shape::Shape`] id and, on a
//! hit, yields the property slot directly. A site that has only ever seen one
//! shape stays [`InlineCache::Monomorphic`]; a second shape promotes it to
//! [`InlineCache::Polymorphic`] and further misses to
//! [`InlineCache::Megamorphic`], at which point the VM falls back to the
//! ordinary dictionary lookup.

/// Maximum number of distinct shapes a site may see before it is considered
/// megamorphic. Kept small: a shortcut that is right 99% of the time is only
/// worth it if the fallback path stays cheap.
pub const POLY_LIMIT: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineCache {
    /// No shape observed yet (or the site is not shape-stable).
    Uninitialized,
    /// Exactly one shape, with the cached slot index.
    Monomorphic { shape: u32, offset: usize },
    /// A small set of shapes; the VM resolves through the dictionary.
    Polymorphic,
    /// Too many shapes; the VM always uses the dictionary.
    Megamorphic,
    /// A monomorphic *method* call site: the receiver's class and the resolved
    /// method, both held (rooted) in `Vm::method_roots` at these indices. The
    /// roots keep the class alive, so equal header pointers mean the same class.
    Method { class_root: u32, method_root: u32 },
}

impl InlineCache {
    /// The slot for `current_shape`, or `None` on a miss.
    #[inline]
    pub fn lookup(&self, current_shape: u32) -> Option<usize> {
        match self {
            InlineCache::Monomorphic { shape, offset } if *shape == current_shape => Some(*offset),
            _ => None,
        }
    }

    /// Fold a newly observed shape into this cache.
    pub fn observe(&mut self, shape: u32, offset: usize) {
        match self {
            InlineCache::Uninitialized => {
                *self = InlineCache::Monomorphic { shape, offset };
            }
            InlineCache::Monomorphic {
                shape: s,
                offset: o,
            } => {
                if *s == shape {
                    *o = offset;
                } else {
                    *self = InlineCache::Polymorphic;
                }
            }
            InlineCache::Polymorphic | InlineCache::Megamorphic | InlineCache::Method { .. } => {}
        }
    }
}
