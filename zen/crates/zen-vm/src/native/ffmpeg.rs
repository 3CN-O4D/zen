//! Native `ffmpeg` module: thin, safe wrappers around the `ffmpeg`/`ffprobe`
//! executables. Each call shells out (`-v error`) and reports a `{ok, ...}`
//! dict rather than raising, so callers can branch on availability/failure.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::host_module_arity;
use crate::value::{DictKey, Value};

const ID_AVAILABLE: u16 = 350;
const ID_PROBE: u16 = 351;
const ID_THUMBNAIL: u16 = 352;
const ID_STICKER: u16 = 353;
const ID_IMAGE: u16 = 355;
const ID_DURATION_MS: u16 = 356;
const ID_MERGE: u16 = 357;

pub fn module() -> ModuleUnit {
    host_module_arity(&[
        ("available", ID_AVAILABLE, 0, 0),
        ("probe", ID_PROBE, 1, 1),
        ("thumbnail", ID_THUMBNAIL, 2, 3),
        ("sticker", ID_STICKER, 2, 3),
        ("image", ID_IMAGE, 2, 3),
        ("duration_ms", ID_DURATION_MS, 1, 1),
        ("merge", ID_MERGE, 2, 3),
    ])
}

fn string_at(args: &[Value], i: usize) -> VmResult<String> {
    let v = args.get(i).copied().ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| VmError::type_mismatch("string", v))
}

fn dict(entries: &[(&str, Value)]) -> Value {
    let mut d = indexmap::IndexMap::new();
    for (k, v) in entries {
        d.insert(DictKey(Value::string(*k)), *v);
    }
    crate::gc::new_dict(d)
}

fn tool_on_path(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run(program: &str, args: &[String]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn probe_info(path: &str) -> Option<(i64, i64, f64)> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let stream = json.get("streams")?.as_array()?.first()?;
    let width = stream.get("width").and_then(|v| v.as_i64())?;
    let height = stream.get("height").and_then(|v| v.as_i64())?;
    let duration = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    Some((width, height, duration))
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        ID_AVAILABLE => Value::bool(tool_on_path("ffmpeg") && tool_on_path("ffprobe")),
        ID_PROBE => {
            let path = string_at(args, 0)?;
            match probe_info(&path) {
                Some((w, h, dur)) => dict(&[
                    ("ok", Value::bool(true)),
                    ("width", Value::int(w)),
                    ("height", Value::int(h)),
                    ("duration_ms", Value::int((dur * 1000.0) as i64)),
                ]),
                None => dict(&[("ok", Value::bool(false))]),
            }
        }
        ID_DURATION_MS => {
            let path = string_at(args, 0)?;
            let ms = probe_info(&path).map(|(_, _, d)| (d * 1000.0) as i64).unwrap_or(0);
            Value::int(ms)
        }
        ID_THUMBNAIL => {
            let input = string_at(args, 0)?;
            let output = string_at(args, 1)?;
            let size = args
                .get(2)
                .and_then(|v| v.as_int())
                .unwrap_or(4);
            let filter = format!("scale={size}:-1");
            let ok = run(
                "ffmpeg",
                &[
                    "-y".into(),
                    "-v".into(),
                    "error".into(),
                    "-i".into(),
                    input,
                    "-vf".into(),
                    filter,
                    output,
                ],
            );
            dict(&[("ok", Value::bool(ok))])
        }
        ID_STICKER => {
            let input = string_at(args, 0)?;
            let output = string_at(args, 1)?;
            let ok = run(
                "ffmpeg",
                &[
                    "-y".into(),
                    "-v".into(),
                    "error".into(),
                    "-i".into(),
                    input,
                    "-c:v".into(),
                    "libwebp".into(),
                    output,
                ],
            );
            dict(&[("ok", Value::bool(ok))])
        }
        ID_IMAGE => {
            let input = string_at(args, 0)?;
            let output = string_at(args, 1)?;
            let ok = run(
                "ffmpeg",
                &[
                    "-y".into(),
                    "-v".into(),
                    "error".into(),
                    "-i".into(),
                    input,
                    output,
                ],
            );
            dict(&[("ok", Value::bool(ok))])
        }
        ID_MERGE => {
            let output = string_at(args, 0)?;
            let inputs = args.get(1).and_then(|v| v.as_list()).map(<[Value]>::to_vec);
            let mut listfile = String::new();
            let mut ok = false;
            if let Some(items) = inputs {
                if items.is_empty() {
                    ok = false;
                } else {
                    for item in items.iter() {
                        listfile.push_str("file '");
                        listfile.push_str(&crate::ops::stringify(*item));
                        listfile.push_str("'\n");
                    }
                    let tmp = std::env::temp_dir().join("zen_ffmpeg_merge.txt");
                    let wrote = std::fs::write(&tmp, &listfile).is_ok();
                    ok = wrote
                        && run(
                            "ffmpeg",
                            &[
                                "-y".into(),
                                "-v".into(),
                                "error".into(),
                                "-f".into(),
                                "concat".into(),
                                "-safe".into(),
                                "0".into(),
                                "-i".into(),
                                tmp.display().to_string(),
                                "-c".into(),
                                "copy".into(),
                                output,
                            ],
                        );
                }
            }
            dict(&[("ok", Value::bool(ok))])
        }
        _ => return Ok(None),
    };
    Ok(Some(v))
}