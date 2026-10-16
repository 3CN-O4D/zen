//! Interning of identifier and string payloads.
//!
//! See `docs/05-lexer.md` §10.4 rule **M10`. An identifier, and a string with no
//! escapes, is interned *zero-copy* as a `&'a` slice into the source buffer. A
//! string whose escapes were cooked is a fresh allocation and is stored owned.
//!
//! The doc sketched `strs: Interner<'a>` returning `&'a [u8]`, but a cooked
//! payload does not exist in the source, so that signature could not be
//! sound. `Cow<'a, T>` is the honest form: borrowing when possible, owning when
//! not, and the accessor borrows from the interner rather than from `'a`.

use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;

use ahash::AHashMap;

/// A payload-interning table. `T` is `str` for identifiers and `[u8]` for
/// cooked string literals, which is why it is generic over an unsized type.
///
/// `Debug`, `Clone` and `PartialEq` are hand-rolled because `Cow<'a, T>`
/// bounds `T::Owned` (not `T`), and derive macros cannot express that.
pub struct Interner<'a, T: ?Sized + ToOwned> {
    /// Only borrowed (zero-copy) entries are keys, so the map never borrows
    /// from `items` and the struct stays self-referential-free.
    map: AHashMap<&'a T, u32>,
    items: Vec<Cow<'a, T>>,
}

impl<'a, T: ?Sized + ToOwned> Clone for Interner<'a, T>
where
    T::Owned: Clone,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            items: self.items.clone(),
        }
    }
}

impl<'a, T: ?Sized + ToOwned + fmt::Debug> fmt::Debug for Interner<'a, T>
where
    T::Owned: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner")
            .field("items", &self.items)
            .finish_non_exhaustive()
    }
}

impl<'a, T: ?Sized + ToOwned + PartialEq> PartialEq for Interner<'a, T>
where
    T::Owned: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<'a, T: ?Sized + ToOwned> Default for Interner<'a, T> {
    fn default() -> Self {
        Self {
            map: AHashMap::new(),
            items: Vec::new(),
        }
    }
}

impl<'a, T: ?Sized + Hash + Eq + ToOwned> Interner<'a, T> {
    pub fn new() -> Self {
        Self {
            map: AHashMap::new(),
            items: Vec::new(),
        }
    }

    /// Intern a payload that lives in the source buffer. The first occurrence
    /// wins, so equal payloads share one id and one AST node.
    pub fn intern(&mut self, value: &'a T) -> u32 {
        if let Some(&id) = self.map.get(value) {
            return id;
        }
        let id = self.next_id();
        self.items.push(Cow::Borrowed(value));
        self.map.insert(value, id);
        id
    }

    /// Store a payload that had to be built. It reuses an equal existing entry
    /// when there is one — so `"a"` still shares with `a` — but does not add a
    /// map key, because a key would have to borrow from the value being stored.
    pub fn intern_cow(&mut self, value: Cow<'a, T>) -> u32 {
        if let Some(&id) = self.map.get(value.as_ref()) {
            return id;
        }
        let id = self.next_id();
        self.items.push(value);
        id
    }

    fn next_id(&self) -> u32 {
        u32::try_from(self.items.len()).unwrap_or(u32::MAX)
    }

    /// The payload for `id`. A stale id yields `None` rather than panicking, so
    /// a corrupt token stream produces a parse error, not a crash.
    #[inline]
    pub fn get(&self, id: u32) -> Option<&T> {
        self.items
            .get(usize::try_from(id).unwrap_or(usize::MAX))
            .map(Cow::as_ref)
    }

    /// True when the entry at `id` is a slice of the source rather than a
    /// cooked allocation.
    #[inline]
    pub fn is_borrowed(&self, id: u32) -> bool {
        matches!(
            self.items.get(usize::try_from(id).unwrap_or(usize::MAX)),
            Some(Cow::Borrowed(_))
        )
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every interned payload, in id order. Used by `zentok` to dump tables.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(Cow::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_payloads_share_one_id() {
        let src = String::from("a bb a");
        let mut ix: Interner<str> = Interner::new();
        let a = src.as_str().get(0..1).unwrap_or("");
        let b = src.as_str().get(2..4).unwrap_or("");
        let a2 = src.as_str().get(5..6).unwrap_or("");
        let first = ix.intern(a);
        assert_eq!(ix.intern(a2), first, "equal strings must dedupe");
        let second = ix.intern(b);
        assert_ne!(first, second);
        assert_eq!(ix.len(), 2);
        assert_eq!(ix.get(first), Some(a));
        assert_eq!(ix.get(second), Some(b));
    }

    #[test]
    fn owned_payloads_reuse_a_borrowed_entry() {
        let src = String::from("abc");
        let mut ix: Interner<[u8]> = Interner::new();
        let borrowed = ix.intern(src.as_bytes());
        assert!(ix.is_borrowed(borrowed));
        // A cooked string that happens to equal a source slice reuses its id.
        let cooked = ix.intern_cow(Cow::Owned(b"abc".to_vec()));
        assert_eq!(cooked, borrowed);
        assert_eq!(ix.len(), 1, "the duplicate must not be stored twice");
    }

    #[test]
    fn owned_payloads_are_stored_when_new() {
        let mut ix: Interner<[u8]> = Interner::new();
        let id = ix.intern_cow(Cow::Owned(b"\n".to_vec()));
        assert!(!ix.is_borrowed(id));
        assert_eq!(ix.get(id), Some(&b"\n"[..]));
        assert_eq!(ix.len(), 1);
    }

    #[test]
    fn stale_id_is_none_not_a_panic() {
        let ix: Interner<str> = Interner::new();
        assert_eq!(ix.get(0), None);
        assert_eq!(ix.get(u32::MAX), None);
    }
}
