//! `http.server` — a static-file HTTP/1.0 server (Python `http.server` parity).
//!
//! `serve(root, port?, host?)` blocks forever, one thread per connection.
//! `main(argv)` is the `zen -m http.server` entry point and accepts
//! `[port] [--bind ADDR] [--directory DIR] [-d DIR]`. Serves `GET`/`HEAD` with
//! `index.html` for directories and an HTML index otherwise, guards against
//! `..` traversal, and guesses `Content-Type` from the extension.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::ops::{int_arg, string_arg};
use crate::value::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::thread;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("serve", 780, 1, 3),
        func("main", 781, 1, 1),
        func("serve_once", 782, 1, 3),
        func("guess_type", 783, 1, 1),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        780 => serve(parse_root(args)?, opt_port(args, 1)?, opt_host(args, 2)?),
        781 => main(args),
        782 => serve_once(args),
        783 => guess_type(args),
        _ => Err(VmError::NativeError(format!(
            "unknown http.server native id {id}"
        ))),
    }
}

fn parse_root(args: &[Value]) -> VmResult<PathBuf> {
    let dir = string_arg(args, 0)?;
    Ok(PathBuf::from(dir))
}

fn opt_port(args: &[Value], i: usize) -> VmResult<u16> {
    match args.get(i) {
        Some(v) if !v.is_nil() => Ok(int_arg(args, i)? as u16),
        _ => Ok(8000),
    }
}

fn opt_host(args: &[Value], i: usize) -> VmResult<String> {
    match args.get(i) {
        Some(v) if !v.is_nil() => Ok(scalar_string(*v)),
        _ => Ok("0.0.0.0".to_string()),
    }
}

fn scalar_string(v: Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else if let Some(i) = v.as_int() {
        i.to_string()
    } else {
        String::new()
    }
}

/// Parse `main(argv)` and dispatch to `serve`.
fn main(args: &[Value]) -> VmResult<Value> {
    let argv = args
        .first()
        .and_then(|v| v.as_list())
        .map(<[Value]>::to_vec)
        .unwrap_or_default();
    let mut port: u16 = 8000;
    let mut host = "0.0.0.0".to_string();
    let mut dir = PathBuf::from(".");
    let mut i = 0;
    while i < argv.len() {
        let a = scalar_string(argv[i]);
        match a.as_str() {
            "--bind" | "-b" => {
                i += 1;
                if let Some(v) = argv.get(i) {
                    host = scalar_string(*v);
                }
            }
            "--directory" | "-d" => {
                i += 1;
                if let Some(v) = argv.get(i) {
                    dir = PathBuf::from(scalar_string(*v));
                }
            }
            "-h" | "--help" => {
                eprintln!("usage: zen -m http.server [port] [--bind ADDR] [--directory DIR]");
                return Ok(Value::nil());
            }
            _ => {
                if let Ok(p) = a.parse::<u16>() {
                    port = p;
                }
            }
        }
        i += 1;
    }
    serve(dir, port, host)
}

/// Block forever serving `root` on `host:port`.
fn serve(root: PathBuf, port: u16, host: String) -> VmResult<Value> {
    let root = root
        .canonicalize()
        .map_err(|e| VmError::NativeError(format!("http.server: cannot open {}: {e}", root.display())))?;
    let listener = TcpListener::bind((host.as_str(), port)).map_err(|e| {
        VmError::NativeError(format!("http.server: cannot bind {host}:{port}: {e}"))
    })?;
    if let Ok(addr) = listener.local_addr() {
        eprintln!("Serving HTTP on {addr} ...");
    }
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let root = root.clone();
                thread::spawn(move || {
                    if let Err(e) = handle(&root, s) {
                        eprintln!("http.server: {e}");
                    }
                });
            }
            Err(e) => eprintln!("http.server: accept: {e}"),
        }
    }
    Ok(Value::nil())
}

/// Accept exactly one connection, serve it, and return the status code.
fn serve_once(args: &[Value]) -> VmResult<Value> {
    let root = parse_root(args)?;
    let root = root.canonicalize().map_err(|e| {
        VmError::NativeError(format!("http.server: cannot open {}: {e}", root.display()))
    })?;
    let port = opt_port(args, 1)?;
    let host = opt_host(args, 2)?;
    let listener = TcpListener::bind((host.as_str(), port))
        .map_err(|e| VmError::NativeError(format!("http.server: cannot bind {host}:{port}: {e}")))?;
    let (stream, _) = listener
        .accept()
        .map_err(|e| VmError::NativeError(format!("http.server: accept: {e}")))?;
    let status = handle(&root, stream)?;
    Ok(Value::int_promote(status as i64))
}

fn handle(root: &Path, stream: TcpStream) -> VmResult<u16> {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| VmError::NativeError(format!("http.server: clone: {e}")))?,
    );
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| VmError::NativeError(format!("http.server: read: {e}")))?;
    if request_line.is_empty() {
        return Ok(400);
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    // Drain request headers so the client does not block on an unread body.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let mut stream = stream;
    let status = respond(root, &method, &target, &mut stream)?;
    let _ = stream.flush();
    Ok(status)
}

fn percent_decode(path: &str) -> String {
    let b = path.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16);
            let lo = (b[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolve a URL path to a file under `root`, rejecting traversal.
fn safe_join(root: &Path, url_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(url_path.split(['?', '#']).next().unwrap_or("/"));
    let rel = decoded.trim_start_matches('/');
    let candidate = root.join(rel);
    let mut clean = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            Component::ParentDir => return None,
            Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    if clean.starts_with(root) {
        Some(clean)
    } else {
        None
    }
}

fn respond(
    root: &Path,
    method: &str,
    target: &str,
    stream: &mut TcpStream,
) -> VmResult<u16> {
    if method != "GET" && method != "HEAD" {
        return write_status(stream, 405, "Method Not Allowed", &[]);
    }
    let path = match safe_join(root, target) {
        Some(p) => p,
        None => return write_status(stream, 403, "Forbidden", &[]),
    };
    let is_head = method == "HEAD";
    if path.is_dir() {
        let index = path.join("index.html");
        if index.is_file() {
            return serve_file(stream, &index, is_head);
        }
        let body = directory_listing(root, &path);
        let headers = [("Content-Type".to_string(), "text/html; charset=utf-8".to_string())];
        return write_body(stream, 200, "OK", &headers, body.as_bytes(), is_head);
    }
    if path.is_file() {
        return serve_file(stream, &path, is_head);
    }
    write_status(stream, 404, "Not Found", &[])
}

fn serve_file(stream: &mut TcpStream, path: &Path, is_head: bool) -> VmResult<u16> {
    let data = std::fs::read(path)
        .map_err(|e| VmError::NativeError(format!("http.server: read {}: {e}", path.display())))?;
    let ctype = content_type(path);
    let headers = [("Content-Type".to_string(), ctype)];
    write_body(stream, 200, "OK", &headers, &data, is_head)
}

fn directory_listing(root: &Path, dir: &Path) -> String {
    let rel = dir.strip_prefix(root).unwrap_or(Path::new(""));
    let mut items: Vec<(String, bool)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            items.push((name, is_dir));
        }
    }
    items.sort();
    let title = format!("/{}", rel.display());
    let mut html = format!(
        "<!DOCTYPE HTML>\n<html><head><meta charset=\"utf-8\"><title>Directory listing for {}</title></head><body>\n<h1>Directory listing for {}</h1><hr><ul>\n",
        html_escape(&title),
        html_escape(&title)
    );
    if !rel.as_os_str().is_empty() {
        html.push_str("<li><a href=\"../\">../</a></li>\n");
    }
    for (name, is_dir) in items {
        let slash = if is_dir { "/" } else { "" };
        html.push_str(&format!(
            "<li><a href=\"{}{}\">{}{}</a></li>\n",
            url_encode(&name),
            slash,
            html_escape(&name),
            slash
        ));
    }
    html.push_str("</ul><hr></body></html>\n");
    html
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

fn write_status(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    extra: &[(String, String)],
) -> VmResult<u16> {
    write_body(stream, code, reason, extra, &[], false)?;
    Ok(code)
}

fn write_body(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    extra: &[(String, String)],
    body: &[u8],
    is_head: bool,
) -> VmResult<u16> {
    let mut head = format!(
        "HTTP/1.0 {code} {reason}\r\nServer: zen-http.server\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|e| VmError::NativeError(format!("http.server: write: {e}")))?;
    if !is_head {
        stream
            .write_all(body)
            .map_err(|e| VmError::NativeError(format!("http.server: write: {e}")))?;
    }
    Ok(code)
}

fn guess_type(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    Ok(Value::string(content_type(Path::new(&path))))
}

fn content_type(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "txt" | "md" | "log" => "text/plain",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/vnd.microsoft.icon",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "wav" => "audio/wav",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    };
    if mime.starts_with("text/") || mime == "application/json" || mime == "application/xml" {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}
