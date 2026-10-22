//! `zen build`: bundle a program into a standalone executable.
//!
//! A built binary is a copy of the `zen` interpreter with a payload appended
//! after the normal executable image. The payload holds the entry source plus
//! the source of every `.z` module the program imports (natives are compiled
//! into the interpreter and need no payload). At startup [`embedded_program`]
//! checks the running executable for the footer and, if present, links and runs
//! the embedded sources instead of reading the command line.
//!
//! Appending data to an ELF file is safe — the loader ignores bytes past the
//! program headers — and it makes the output a single self-contained artifact
//! that runs with no source tree present.
//!
//! Format (all little-endian):
//!
//! ```text
//! payload := u32 entry_path_len, entry_path,
//!            u32 module_count,
//!            { u32 name_len, name, u64 src_len, src } *,
//!            u64 entry_src_len, entry_src
//! file    := <elf image> <payload> <u64 payload_len> <u64 MAGIC>
//! ```

use std::collections::HashMap;

use zen_cli::linker;

/// Footer magic: ASCII `ZENBUILD`.
const MAGIC: u64 = 0x5A45_4E42_5549_4C44;
/// Bytes of footer: a `u64` length followed by the `u64` magic.
const FOOTER: usize = 16;

/// The pieces recovered from a built binary.
pub struct EmbeddedProgram {
    pub entry_path: String,
    pub entry_source: String,
    pub modules: Vec<(String, String)>,
}

impl EmbeddedProgram {
    pub fn overlay(&self) -> HashMap<String, String> {
        self.modules.iter().cloned().collect()
    }
}

/// Build `source_path` into a standalone executable at `output`.
pub fn build(source_path: &str, output: &str) -> Result<(), String> {
    let linked = linker::link(source_path)?;
    let payload = encode_payload(&linked);

    let exe = std::env::current_exe().map_err(|e| format!("build: cannot find interpreter: {e}"))?;
    let mut image = std::fs::read(&exe).map_err(|e| format!("build: cannot read interpreter: {e}"))?;
    image.extend_from_slice(&payload);
    image.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    image.extend_from_slice(&MAGIC.to_le_bytes());

    std::fs::write(output, &image).map_err(|e| format!("build: cannot write {output}: {e}"))?;
    make_executable(output)?;
    println!(
        "built {output} ({} bytes; {} module(s) embedded)",
        image.len(),
        linked.sources.len().saturating_sub(1)
    );
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).map_err(|e| format!("build: {e}"))?;
    let mut perms = meta.permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms).map_err(|e| format!("build: {e}"))
}

#[cfg(not(unix))]
fn make_executable(_path: &str) -> Result<(), String> {
    Ok(())
}

fn encode_payload(linked: &linker::LinkedProgram) -> Vec<u8> {
    let mut buf = Vec::new();
    let (entry_path, entry_source) = linked
        .sources
        .get("")
        .cloned()
        .unwrap_or_else(|| ("<embedded>".to_string(), String::new()));
    put_str(&mut buf, &entry_path);

    let deps: Vec<(&String, &(String, String))> = linked
        .sources
        .iter()
        .filter(|(name, _)| !name.is_empty())
        .collect();
    buf.extend_from_slice(&(deps.len() as u32).to_le_bytes());
    for (name, (_path, src)) in deps {
        put_str(&mut buf, name);
        put_u64_str(&mut buf, src);
    }

    put_u64_str(&mut buf, &entry_source);
    buf
}

/// Recover the embedded program from the currently running executable, or
/// `None` when the binary is the plain interpreter.
pub fn embedded_program() -> Option<EmbeddedProgram> {
    let exe = std::env::current_exe().ok()?;
    let image = std::fs::read(&exe).ok()?;
    decode_image(&image)
}

/// Decode a payload from a full executable image (also used by tests).
pub fn decode_image(image: &[u8]) -> Option<EmbeddedProgram> {
    if image.len() < FOOTER {
        return None;
    }
    let tail = &image[image.len() - FOOTER..];
    let len = u64::from_le_bytes(tail[0..8].try_into().ok()?);
    let magic = u64::from_le_bytes(tail[8..16].try_into().ok()?);
    if magic != MAGIC {
        return None;
    }
    let len = usize::try_from(len).ok()?;
    let start = image.len().checked_sub(FOOTER)?.checked_sub(len)?;
    decode_payload(&image[start..start + len])
}

fn decode_payload(buf: &[u8]) -> Option<EmbeddedProgram> {
    let mut cur = Cursor { buf, pos: 0 };
    let entry_path = cur.get_str()?;
    let count = cur.get_u32()? as usize;
    let mut modules = Vec::with_capacity(count);
    for _ in 0..count {
        let name = cur.get_str()?;
        let src = cur.get_long_str()?;
        modules.push((name, src));
    }
    let entry_source = cur.get_long_str()?;
    Some(EmbeddedProgram {
        entry_path,
        entry_source,
        modules,
    })
}

/// Link and run an embedded program. Mirrors `zen run` for the payload case.
pub fn run_embedded(prog: &EmbeddedProgram) -> Result<(), String> {
    let overlay = prog.overlay();
    let linked = linker::link_program(prog.entry_path.clone(), prog.entry_source.clone(), None, &overlay)?;
    crate::cli::run_linked(linked)
}

// ── small binary codec ──────────────────────────────────────────────────

fn put_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

fn put_u64_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn get_u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn get_str(&mut self) -> Option<String> {
        let n = self.get_u32()? as usize;
        let bytes = self.take(n)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    fn get_long_str(&mut self) -> Option<String> {
        let n = u64::from_le_bytes(self.take(8)?.try_into().ok()?);
        let n = usize::try_from(n).ok()?;
        let bytes = self.take(n)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Verify the payload round-trips through encode/decode.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trips() {
        let mut buf = Vec::new();
        put_str(&mut buf, "/tmp/a.z");
        buf.extend_from_slice(&1u32.to_le_bytes());
        put_str(&mut buf, "std.math");
        put_u64_str(&mut buf, "x = 1\n");
        put_u64_str(&mut buf, "print(1)\n");

        let prog = decode_payload(&buf).unwrap();
        assert_eq!(prog.entry_path, "/tmp/a.z");
        assert_eq!(prog.entry_source, "print(1)\n");
        assert_eq!(prog.modules, vec![("std.math".to_string(), "x = 1\n".to_string())]);
    }

    #[test]
    fn decode_rejects_plain_image() {
        assert!(decode_image(b"not an elf with a payload").is_none());
    }
}
