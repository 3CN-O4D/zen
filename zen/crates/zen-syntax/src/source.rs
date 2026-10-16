//! Source files, file identity, and module-path derivation.
//!
//! See `docs/05-lexer.md` §2.4 and `docs/06-parser.md` §9.

use std::path::{Path, PathBuf};

use crate::span::FileId;

/// One file's identity, path, and text.
#[derive(Debug, Clone)]
pub struct LoadedFile {
    pub id: FileId,
    pub path: PathBuf,
    pub text: String,
}

impl LoadedFile {
    /// The dotted module path this file would be imported as, e.g.
    /// `std/http/client.z` becomes `std.http.client`.
    pub fn module_path(&self) -> String {
        module_path_of(&self.path)
    }
}

/// Owns every source the compiler will look at, and hands out [`FileId`]s.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<LoadedFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Register a file and return its id. Ids are dense and assigned in
    /// insertion order, starting at 0.
    pub fn add(&mut self, path: impl Into<PathBuf>, text: String) -> FileId {
        let id = FileId(self.files.len());
        self.files.push(LoadedFile {
            id,
            path: path.into(),
            text,
        });
        id
    }

    pub fn get(&self, id: FileId) -> Option<&LoadedFile> {
        self.files.get(id.0)
    }

    pub fn text(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0).map(|f| f.text.as_str())
    }

    pub fn path(&self, id: FileId) -> Option<&Path> {
        self.files.get(id.0).map(|f| f.path.as_path())
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LoadedFile> {
        self.files.iter()
    }
}

/// Dotted module path for a `.z` file: strip a leading `src/`, drop the
/// extension, and turn separators into dots. A file outside any module root
/// keeps its full stem, so `/tmp/x.z` becomes `x`.
pub fn module_path_of(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        let text = comp.as_os_str().to_string_lossy().into_owned();
        parts.push(text);
    }
    // Drop a leading `src` or `lib`, which are layout directories, not modules.
    if matches!(parts.first().map(String::as_str), Some("src") | Some("lib")) {
        parts.remove(0);
    }
    let last = parts.pop().unwrap_or_default();
    let stem = last.strip_suffix(".z").unwrap_or(&last).to_string();
    if stem.is_empty() {
        parts.join(".")
    } else {
        parts.push(stem);
        parts.join(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_dense_and_ordered() {
        let mut sm = SourceMap::new();
        let a = sm.add("a.z", "let a = 1".to_string());
        let b = sm.add("b.z", "let b = 2".to_string());
        assert_eq!((a.0, b.0), (0, 1));
        assert_eq!(sm.len(), 2);
        assert_eq!(sm.text(a), Some("let a = 1"));
        assert_eq!(sm.text(FileId(9)), None);
    }

    #[test]
    fn module_paths_drop_the_extension() {
        assert_eq!(module_path_of(Path::new("std/http.z")), "std.http");
        assert_eq!(module_path_of(Path::new("src/lib/core.z")), "lib.core");
        assert_eq!(module_path_of(Path::new("main.z")), "main");
        assert_eq!(module_path_of(Path::new("noext")), "noext");
    }
}
