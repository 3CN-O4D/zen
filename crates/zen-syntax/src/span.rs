//! Spans, file identity, and the cold line index.
//!
//! Span arithmetic is the one place in `zen-syntax` where an overflow would be a
//! silently wrong answer rather than a panic, so this module additionally denies
//! `clippy::arithmetic_side_effects` (see `docs/05-lexer.md` §9.3). Every
//! operation here is therefore saturating or checked.

#![deny(clippy::arithmetic_side_effects)]

use std::fmt;

use serde::{Deserialize, Serialize};

/// Identity of a source file within a [`crate::source::SourceMap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FileId(pub usize);

impl FileId {
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file#{}", self.0)
    }
}

/// A half-open byte range `[lo, hi)` within a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file: FileId,
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    #[inline]
    pub const fn new(file: FileId, lo: u32, hi: u32) -> Self {
        Self { file, lo, hi }
    }

    /// A zero-length span, which renders as a single `^`.
    #[inline]
    pub const fn empty_at(file: FileId, off: u32) -> Self {
        Self {
            file,
            lo: off,
            hi: off,
        }
    }

    /// The smallest span covering both operands.
    ///
    /// Unlike the legacy `Span::merge`, this does not assert that the two spans
    /// share a file: spans from different files are not produced by the lexer,
    /// and a panic in a diagnostic path is strictly worse than a slightly odd
    /// merged span.
    pub fn merge(self, other: Span) -> Span {
        Span {
            file: self.file,
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    /// Length in bytes, saturating at zero for an inverted span.
    #[inline]
    pub const fn len(self) -> u32 {
        self.hi.saturating_sub(self.lo)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.hi <= self.lo
    }

    /// `self` moved `n` bytes forward. Offsets saturate rather than wrap, so a
    /// 4 GiB file degrades to a clamped span instead of a bogus one.
    pub fn advance(self, n: u32) -> Span {
        Span {
            file: self.file,
            lo: self.lo.saturating_add(n),
            hi: self.hi.saturating_add(n),
        }
    }

    /// True when `off` lies inside the half-open range.
    #[inline]
    pub const fn contains(self, off: u32) -> bool {
        off >= self.lo && off < self.hi
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}..{}", self.file.0, self.lo, self.hi)
    }
}

/// Anything that can report the source range it came from.
pub trait Spanned {
    fn span(&self) -> Span;
}

impl<T: Spanned> Spanned for &T {
    #[inline]
    fn span(&self) -> Span {
        (*self).span()
    }
}

impl<T: Spanned> Spanned for Box<T> {
    #[inline]
    fn span(&self) -> Span {
        (**self).span()
    }
}

/// Byte offset of every line's first byte; `starts[0] == 0` always.
///
/// Built in one pass in [`LineIndex::build`] (rule **M15**) and only ever read on
/// the diagnostic path, which is why the lexer itself never touches it.
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    starts: Vec<u32>,
    src: &'a str,
}

impl<'a> LineIndex<'a> {
    /// One pass over the source, recording the offset just after each `\n`.
    pub fn build(src: &'a str) -> Self {
        let mut starts = Vec::with_capacity(src.len().saturating_div(24).saturating_add(1));
        starts.push(0u32);
        for (off, byte) in src.as_bytes().iter().enumerate() {
            if *byte == b'\n' {
                // `off` is bounded by `src.len()`, so this cannot realistically
                // saturate; it is written to keep the module panic-free.
                starts.push(u32::try_from(off.saturating_add(1)).unwrap_or(u32::MAX));
            }
        }
        Self { starts, src }
    }

    /// Number of lines. A source with no `\n` has exactly one line.
    pub fn line_count(&self) -> u32 {
        u32::try_from(self.starts.len()).unwrap_or(u32::MAX)
    }

    /// 1-based line containing `off`.
    pub fn line_of(&self, off: u32) -> u32 {
        if self.starts.is_empty() {
            return 1;
        }
        // Greatest `starts[k] <= off`, as a 1-based line number.
        let idx = self.starts.partition_point(|&s| s <= off);
        let line = idx.saturating_sub(1).saturating_add(1);
        u32::try_from(line).unwrap_or(u32::MAX)
    }

    /// 1-based **character** column of `off`, so a caret under a 3-byte `█`
    /// lands correctly. Counts characters whose start byte is strictly before
    /// `off`; an offset *inside* a multi-byte character therefore points one
    /// past that character. Decoding backwards from the line start makes this
    /// diagnostic-path-only work (see `docs/05-lexer.md` §2.4).
    pub fn col_of(&self, off: u32) -> u32 {
        let line = self.line_of(off);
        let start = self.line_start(line);
        let text = self.line_text(line).unwrap_or("");
        let shifted = off.saturating_sub(start);
        let col = text
            .char_indices()
            .take_while(|(rel, _)| u32::try_from(*rel).unwrap_or(u32::MAX) < shifted)
            .count()
            .saturating_add(1);
        u32::try_from(col).unwrap_or(u32::MAX)
    }

    #[inline]
    pub fn line_col(&self, off: u32) -> (u32, u32) {
        (self.line_of(off), self.col_of(off))
    }

    /// Byte offset where the 1-based `line` starts.
    pub fn line_start(&self, line: u32) -> u32 {
        if line == 0 {
            return 0;
        }
        let idx = usize::try_from(line.saturating_sub(1)).unwrap_or(0);
        self.starts.get(idx).copied().unwrap_or(0)
    }

    /// Byte range of the 1-based `line`, excluding its line terminator.
    pub fn line_range(&self, line: u32) -> Option<(u32, u32)> {
        let start = self.line_start(line);
        if line == 0 || line > self.line_count() {
            return None;
        }
        let next = self.line_start(line.saturating_add(1));
        let text_end = if next > start {
            next.saturating_sub(1)
        } else {
            u32::try_from(self.src.len()).unwrap_or(start)
        };
        // Trim a CR so CRLF files do not render a trailing ^ on a blank cell.
        let hi = match self
            .src
            .as_bytes()
            .get(usize::try_from(text_end).unwrap_or(0).saturating_sub(1))
        {
            Some(b'\r') => text_end.saturating_sub(1),
            _ => text_end,
        };
        if hi < start {
            return None;
        }
        Some((start, hi))
    }

    /// The text of the 1-based `line`, without its terminator.
    pub fn line_text(&self, line: u32) -> Option<&'a str> {
        let (lo, hi) = self.line_range(line)?;
        let lo = usize::try_from(lo).ok()?;
        let hi = usize::try_from(hi).ok()?;
        self.src.get(lo..hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_is_one_based() {
        let idx = LineIndex::build("a\nbb\n\nccc");
        assert_eq!(idx.line_of(0), 1);
        assert_eq!(idx.line_of(2), 2);
        assert_eq!(idx.line_of(5), 3);
        assert_eq!(idx.line_of(6), 4);
        assert_eq!(idx.col_of(0), 1);
        assert_eq!(idx.col_of(3), 2);
    }

    #[test]
    fn col_counts_characters_not_bytes() {
        let idx = LineIndex::build("█▀x");
        assert_eq!(idx.col_of(0), 1);
        assert_eq!(idx.col_of(3), 2);
        assert_eq!(idx.col_of(4), 3);
    }

    #[test]
    fn empty_source_has_one_line() {
        let idx = LineIndex::build("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_of(0), 1);
        assert_eq!(idx.col_of(0), 1);
    }

    #[test]
    fn line_text_drops_the_terminator() {
        let idx = LineIndex::build("one\ntwo\r\n");
        assert_eq!(idx.line_text(1), Some("one"));
        assert_eq!(idx.line_text(2), Some("two"));
    }

    #[test]
    fn span_merge_and_len_saturate() {
        let a = Span::new(FileId(0), 5, 9);
        let b = Span::new(FileId(0), 1, 3);
        assert_eq!(a.merge(b), Span::new(FileId(0), 1, 9));
        assert_eq!(a.len(), 4);
        assert_eq!(Span::new(FileId(0), 9, 2).len(), 0);
        assert_eq!(
            a.advance(u32::MAX),
            Span::new(FileId(0), u32::MAX, u32::MAX)
        );
    }
}
