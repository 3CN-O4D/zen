use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "zen.json";

pub fn modules_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ZEN_MODULES") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from("zen_modules");
    if local.exists() {
        return local;
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".zen").join("modules");
    }
    local
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::blocking::get(url).map_err(|e| format!("http_get failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("http_get {} -> HTTP {}", url, status));
    }
    response.bytes().map(|b| b.to_vec()).map_err(|e| format!("http_get read failed: {e}"))
}

/// Resolve an install spec into a fetchable source + a human label.
/// Accepts: `owner/repo[@tag]`, `http(s)://...`, `file:///path`, or a plain path.
fn resolve_source(spec: &str) -> (String, String) {
    if spec.starts_with("file://") {
        (spec[7..].to_string(), spec.to_string())
    } else if spec.starts_with("http://") || spec.starts_with("https://") {
        (spec.to_string(), spec.to_string())
    } else if Path::new(spec).exists() {
        (spec.to_string(), spec.to_string())
    } else if spec.contains('/') {
        let (repo, tag) = match spec.split_once('@') {
            Some((r, t)) => (r.to_string(), t.to_string()),
            None => (spec.to_string(), "main".to_string()),
        };
        let url = format!("https://codeload.github.com/{repo}/tar.gz/refs/{tag}");
        (url, format!("{repo}@{tag}"))
    } else {
        (spec.to_string(), spec.to_string())
    }
}

fn fetch_archive(source: &str) -> Result<Vec<u8>, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        http_get(source)
    } else {
        fs::read(source).map_err(|e| format!("failed to read {source}: {e}"))
    }
}

/// Install a module. `force` reinstalls over an existing copy.
pub fn install(spec: &str, force: bool) -> Result<String, String> {
    let (source, label) = resolve_source(spec);
    eprintln!("Fetching {label} ...");
    let bytes = fetch_archive(&source)?;
    let sha = sha256_hex(&bytes);
    eprintln!("Downloaded {} bytes (sha256 {})", bytes.len(), &sha[..16]);

    let files = extract_tarball(&bytes)?;
    let manifest = files
        .iter()
        .find(|f| f.path.ends_with(MANIFEST))
        .ok_or_else(|| format!("no {MANIFEST} found in {label}"))?;
    let manifest_value: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&manifest.content))
        .map_err(|e| format!("invalid {MANIFEST}: {e}"))?;
    let name = manifest_value
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{MANIFEST} missing 'name'"))?
        .to_string();
    if name.trim().is_empty() || name.contains('/') || name.contains("..") {
        return Err(format!("invalid module name: {name}"));
    }

    let target = modules_dir().join(&name);
    if target.exists() {
        if !force {
            return Err(format!(
                "{name} is already installed (use --force to reinstall)"
            ));
        }
        fs::remove_dir_all(&target).map_err(|e| format!("failed to clear {}: {e}", target.display()))?;
    }
    fs::create_dir_all(&target).map_err(|e| format!("failed to create {}: {e}", target.display()))?;

    // Codeload tarballs have a single root dir (owner-repo-tag/); our packed
    // artifacts have files at the top level. Strip the root only if it is a
    // common prefix shared by every entry, and not a bare file.
    let first_parts: Vec<&str> = files[0].path.split('/').collect();
    let root = if first_parts.len() > 1 {
        let candidate = first_parts[0].to_string();
        if files.iter().all(|f| f.path.starts_with(&format!("{candidate}/"))) {
            candidate
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let mut manifest_written = false;
    for file in &files {
        let rel = file.path.strip_prefix(&root).unwrap_or(&file.path);
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }
        let out = target.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&out, &file.content)
            .map_err(|e| format!("failed to write {}: {e}", out.display()))?;
        if rel == MANIFEST {
            manifest_written = true;
        }
    }
    if !manifest_written {
        if let Some(m) = manifest_path(&files, &root) {
            let out = target.join(MANIFEST);
            fs::write(&out, &m.content).ok();
        }
    }
    // Record the source sha256 for later verification.
    let mut locked = serde_json::Map::new();
    locked.insert("name".into(), serde_json::json!(name));
    locked.insert("source".into(), serde_json::json!(label));
    locked.insert("sha256".into(), serde_json::json!(sha));
    fs::write(target.join(".zen-lock.json"), serde_json::Value::Object(locked).to_string())
        .map_err(|e| format!("failed to write lockfile: {e}"))?;

    eprintln!("Installed {name} -> {}", target.display());
    Ok(name)
}

fn manifest_path<'a>(files: &'a [TarballFile], root: &str) -> Option<&'a TarballFile> {
    files.iter().find(|f| f.path == format!("{root}/{MANIFEST}"))
}

pub fn list() -> Result<(), String> {
    let dir = modules_dir();
    if !dir.exists() {
        eprintln!("No modules installed ({})", dir.display());
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    println!("Modules in {}:", dir.display());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = module_meta(&dir.join(&name));
        match meta {
            Some(m) => println!("  {name} v{} ({})", m.version, m.description),
            None => println!("  {name}"),
        }
    }
    Ok(())
}

/// pip-freeze style listing: `name==version`, with a source comment for reinstall.
pub fn freeze() -> Result<(), String> {
    let dir = modules_dir();
    if !dir.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let mdir = dir.join(&name);
        let version = module_meta(&mdir).map(|m| m.version).unwrap_or_else(|| "0.0.0".into());
        println!("{name}=={version}");
        if let Ok(text) = fs::read_to_string(mdir.join(".zen-lock.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(source) = v.get("source").and_then(|s| s.as_str()) {
                    println!("# {name} <- {source}");
                }
            }
        }
    }
    Ok(())
}

/// Install every package listed in a freeze file (pip -r style).
pub fn install_requirements(path: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    let mut sources: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("# ") {
            if let Some((name, src)) = rest.split_once(" <- ") {
                sources.insert(name.trim().to_string(), src.trim().to_string());
            }
        }
    }
    let mut installed: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, _version) = match line.split_once("==") {
            Some(parts) => parts,
            None => (line, ""),
        };
        let spec = if let Some(src) = sources.get(name) {
            src.clone()
        } else {
            line.to_string()
        };
        match install(&spec, true) {
            Ok(n) => installed.push(n),
            Err(e) => return Err(format!("failed to install {spec}: {e}")),
        }
    }
    if installed.is_empty() {
        eprintln!("Nothing to install from {path}");
    } else {
        eprintln!("Installed {} package(s) from {path}", installed.len());
    }
    Ok(())
}

pub fn remove(name: &str) -> Result<(), String> {
    if name.contains('/') || name.contains("..") {
        return Err(format!("invalid module name: {name}"));
    }
    let target = modules_dir().join(name);
    if !target.exists() {
        return Err(format!("module {name} not installed"));
    }
    fs::remove_dir_all(&target).map_err(|e| format!("failed to remove {}: {e}", target.display()))?;
    println!("Removed {name}");
    Ok(())
}

pub fn verify(name: &str) -> Result<(), String> {
    if name.contains('/') || name.contains("..") {
        return Err(format!("invalid module name: {name}"));
    }
    let dir = modules_dir().join(name);
    let lock_path = dir.join(".zen-lock.json");
    if !lock_path.exists() {
        return Err(format!("module {name} has no lockfile (installed without verification info)"));
    }
    let text = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let expected = v.get("sha256").and_then(|s| s.as_str()).ok_or("lockfile missing sha256")?;
    let source = v.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let (src, _label) = resolve_source(source);
    eprintln!("Verifying {name} from {source} ...");
    let bytes = fetch_archive(&src)?;
    let actual = sha256_hex(&bytes);
    if actual == expected {
        println!("{name}: OK (sha256 matches)");
        Ok(())
    } else {
        Err(format!(
            "{name}: checksum MISMATCH\n  expected {expected}\n  got      {actual}"
        ))
    }
}

pub fn info(name: &str) -> Result<(), String> {
    if name.contains('/') || name.contains("..") {
        return Err(format!("invalid module name: {name}"));
    }
    let dir = modules_dir().join(name);
    if !dir.exists() {
        return Err(format!("module {name} not installed"));
    }
    println!("Module: {name}");
    println!("Location: {}", dir.display());
    if let Some(m) = module_meta(&dir) {
        println!("Version: {}", m.version);
        println!("Description: {}", m.description);
    }
    let lock = dir.join(".zen-lock.json");
    if lock.exists() {
        if let Ok(text) = fs::read_to_string(&lock) {
            println!("Lockfile: {text}");
        }
    }
    Ok(())
}

struct ModuleMeta {
    version: String,
    description: String,
}

fn module_meta(dir: &Path) -> Option<ModuleMeta> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(ModuleMeta {
        version: v.get("version").and_then(|x| x.as_str()).unwrap_or("0.0.0").to_string(),
        description: v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Build a publishable tarball from a package directory. Writes `<name>-<version>.tar.gz`.
pub fn pack(dir: &str) -> Result<String, String> {
    let dir_path = Path::new(dir);
    let manifest_path = dir_path.join(MANIFEST);
    if !manifest_path.is_file() {
        return Err(format!("no {MANIFEST} in {dir}"));
    }
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("invalid {MANIFEST}: {e}"))?;
    let name = v.get("name").and_then(|x| x.as_str()).ok_or("manifest missing 'name'")?.to_string();
    let version = v.get("version").and_then(|x| x.as_str()).unwrap_or("0.0.0").to_string();
    if name.contains('/') || name.contains("..") {
        return Err(format!("invalid module name: {name}"));
    }

    let out_name = format!("{name}-{version}.tar.gz");
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(enc);
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(dir_path, dir_path, &mut files)?;
    for path in &files {
        let rel = path.strip_prefix(dir_path).map_err(|e| e.to_string())?;
        let content = fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, rel, &content[..])
            .map_err(|e| format!("tar append failed: {e}"))?;
    }
    builder.finish().map_err(|e| format!("tar finish failed: {e}"))?;
    let bytes = builder.into_inner().map_err(|e| e.to_string())?.finish().map_err(|e| e.to_string())?;
    fs::write(&out_name, &bytes).map_err(|e| format!("failed to write {out_name}: {e}"))?;
    let sha = sha256_hex(&bytes);
    println!("Packed {out_name} ({} bytes)", bytes.len());
    println!("sha256: {sha}");
    Ok(out_name)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("failed to read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            collect_files(root, &path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Publish a package: pack it, clone a git repo, commit and push the artifact.
pub fn publish(dir: &str, remote: &str) -> Result<(), String> {
    let artifact = pack(dir)?;
    let tmp = std::env::temp_dir().join(format!("zen-pub-{}", std::process::id()));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).ok();
    }
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", remote])
        .arg(&tmp)
        .status()
        .map_err(|e| format!("git clone failed (is git installed?): {e}"))?;
    if !status.success() {
        return Err("git clone failed".into());
    }
    fs::copy(&artifact, tmp.join(&artifact)).map_err(|e| format!("failed to copy artifact: {e}"))?;
    let status = std::process::Command::new("git")
        .current_dir(&tmp)
        .args(["add", &artifact])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("git add failed".into());
    }
    let status = std::process::Command::new("git")
        .current_dir(&tmp)
        .args(["commit", "-m", &format!("Publish {}", artifact.trim_end_matches(".tar.gz"))])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("git commit failed".into());
    }
    let status = std::process::Command::new("git")
        .current_dir(&tmp)
        .args(["push", "origin", "HEAD"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("git push failed (check remote auth)".into());
    }
    println!("Published {artifact} -> {remote}");
    Ok(())
}

struct TarballFile {
    path: String,
    content: Vec<u8>,
}

fn extract_tarball(gz: &[u8]) -> Result<Vec<TarballFile>, String> {
    let decoder = flate2::read::GzDecoder::new(gz);
    let mut archive = tar::Archive::new(decoder);
    let mut files = Vec::new();
    for entry in archive.entries().map_err(|e| format!("tar entries failed: {e}"))? {
        let mut entry = entry.map_err(|e| format!("tar entry failed: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("tar path failed: {e}"))?
            .to_string_lossy()
            .into_owned();
        if entry.header().entry_type().is_file() {
            let mut content = Vec::new();
            entry
                .read_to_end(&mut content)
                .map_err(|e| format!("tar read failed: {e}"))?;
            files.push(TarballFile { path, content });
        }
    }
    Ok(files)
}

/// Resolve a module file (`name.z`) or module dir against installed packages.
/// Returns an absolute path if found in the local/global modules dir.
pub fn resolve_module_file(name: &str) -> Option<String> {
    let dir = modules_dir();
    let candidates = [
        dir.join(name).join(format!("{name}.z")),
        dir.join(format!("{name}.z")),
        dir.join(name).join("main.z"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c.to_string_lossy().into_owned());
        }
    }
    None
}