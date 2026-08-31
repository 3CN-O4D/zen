use std::cell::RefCell;

use crate::cdp::{CdpBrowser, CdpSession};
use crate::runtime::Value;
use base64::Engine;
use std::sync::Arc;

thread_local! {
    static BROWSER: RefCell<Option<CdpBrowser>> = RefCell::new(None);
    static SESSION: RefCell<Option<CdpSession>> = RefCell::new(None);
}

pub fn browser_launch(args: &Vec<Value>) -> Result<Value, String> {
    let headless = match args.first() {
        Some(Value::Bool(b)) => *b,
        _ => true,
    };
    let port = match args.get(1) {
        Some(Value::Number(n)) => *n as u16,
        _ => 9222,
    };

    let user_data_dir = std::env::temp_dir()
        .join(format!("zen-cdp-{}", std::process::id()))
        .to_string_lossy()
        .into_owned();

    let browser = CdpBrowser::launch(None, port, headless, user_data_dir)?;
    let session = browser.connect()?;

    // Enable core domains
    let mut session = session;
    session.command("Page.enable", None)?;
    session.command("Runtime.enable", None)?;
    session.command("Network.enable", None)?;

    BROWSER.with(|b| *b.borrow_mut() = Some(browser));
    SESSION.with(|s| *s.borrow_mut() = Some(session));

    Ok(Value::Bool(true))
}

fn ensure_browser() -> Result<(), String> {
    let has_session = SESSION.with(|s| s.borrow().is_some());
    if has_session {
        return Ok(());
    }
    let browser = CdpBrowser::launch(None, 9222, true, std::env::temp_dir().join("zen-auto").to_string_lossy().into_owned())?;
    let mut session = browser.connect()?;
    session.command("Page.enable", None)?;
    session.command("Runtime.enable", None)?;
    session.command("Network.enable", None)?;
    BROWSER.with(|b| *b.borrow_mut() = Some(browser));
    SESSION.with(|s| *s.borrow_mut() = Some(session));
    Ok(())
}

pub fn browser_connect() -> Result<Value, String> {
    let port = 9222;
    let browser = CdpBrowser::launch(None, port, false, std::env::temp_dir().join("zen-cdp-headful").to_string_lossy().into_owned())?;
    let mut session = browser.connect()?;
    session.command("Page.enable", None)?;
    session.command("Runtime.enable", None)?;
    session.command("Network.enable", None)?;
    BROWSER.with(|b| *b.borrow_mut() = Some(browser));
    SESSION.with(|s| *s.borrow_mut() = Some(session));
    Ok(Value::Bool(true))
}

pub fn browser_navigate(args: &Vec<Value>) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("navigate expects a url string".into()),
    };
    ensure_browser()?;
    with_session(|s| {
        s.command("Page.navigate", Some(serde_json::json!({ "url": url })))?;
        // Wait for load event
        for _ in 0..100 {
            if let Ok(result) = s.command("Runtime.evaluate", Some(serde_json::json!({
                "expression": "document.readyState",
                "returnByValue": true
            }))) {
                if result.get("result").and_then(|r| r.get("value")).and_then(|v| v.as_str()) == Some("complete") {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(Value::Bool(true))
    })
}

pub fn browser_evaluate(args: &Vec<Value>) -> Result<Value, String> {
    let expression = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("evaluate expects a js expression string".into()),
    };
    ensure_browser()?;
    let wrapped = wrap_js_expression(expression);
    with_session(|s| {
        let result = s.evaluate(&wrapped)?;
        Ok(js_value_to_zen(result))
    })
}

/// Replicates the old Python browser.execute() wrapping so scripts like
/// `js("var x = ...; return x")` and `execute("window.scrollTo(0, 0)")` work.
fn wrap_js_expression(code: &str) -> String {
    let code = code.trim();
    let stmt_keywords = [
        "var ", "let ", "const ", "if ", "for ", "while ", "function ",
        "switch ", "try ", "throw ",
    ];
    if code.starts_with("return ") || code.starts_with("return\n") {
        // `return` is only valid inside a function — wrap in an IIFE
        format!("(function() {{ {code} }})()")
    } else if stmt_keywords.iter().any(|kw| code.starts_with(kw)) {
        format!("(function() {{ {code} }})()")
    } else {
        // Plain expression — evaluate directly
        code.to_string()
    }
}

pub fn browser_screenshot(args: &Vec<Value>) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("screenshot expects a path string".into()),
    };
    with_session(|s| {
        let result = s.command("Page.captureScreenshot", Some(serde_json::json!({ "format": "png" })))?;
        let data = result.get("data").and_then(|d| d.as_str()).ok_or_else(|| "no screenshot data".to_string())?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("failed to write screenshot: {e}"))?;
        Ok(Value::Bool(true))
    })
}

pub fn browser_get_html() -> Result<Value, String> {
    with_session(|s| {
        let result = s.evaluate("document.documentElement.outerHTML")?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_get_title() -> Result<Value, String> {
    with_session(|s| {
        let result = s.evaluate("document.title")?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_get_url() -> Result<Value, String> {
    with_session(|s| {
        let result = s.evaluate("location.href")?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_get_text(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("get_text expects a selector string".into()),
    };
    with_session(|s| {
        let expr = format!("(() => {{ const el = document.querySelector({sel:?}); return el ? el.innerText : null }})()", sel = selector);
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_click(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("click expects a selector string".into()),
    };
    with_session(|s| {
        let expr = format!("(() => {{ const el = document.querySelector({sel:?}); if (!el) return false; el.click(); return true }})()", sel = selector);
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_fill(args: &Vec<Value>) -> Result<Value, String> {
    let (selector, value) = match args.as_slice() {
        [Value::String(sel), Value::String(val)] => (sel, val),
        _ => return Err("fill expects (selector, value)".into()),
    };
    with_session(|s| {
        let expr = format!("(() => {{ const el = document.querySelector({sel:?}); if (!el) return false; el.value = {val:?}; el.dispatchEvent(new Event('input', {{bubbles: true}})); el.dispatchEvent(new Event('change', {{bubbles: true}})); return true }})()", sel = selector, val = value);
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_query(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("query expects a selector string".into()),
    };
    with_session(|s| {
        let expr = format!("(() => {{ const els = Array.from(document.querySelectorAll({sel:?})); return els.map(e => e.innerText) }})()", sel = selector);
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_wait_for(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("wait_for expects a selector string".into()),
    };
    let max_ms = match args.get(1) {
        Some(Value::Number(n)) => (*n as u64).min(60_000),
        _ => 20_000,
    };
    with_session(|s| {
        let polls = (max_ms / 100).max(1) as u32;
        for _ in 0..polls {
            let expr = format!("document.querySelector({sel:?}) !== null", sel = selector);
            if let Ok(result) = s.evaluate(&expr) {
                if result.get("value").and_then(|v| v.as_bool()) == Some(true) {
                    return Ok(Value::Bool(true));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(Value::Bool(false))
    })
}

pub fn browser_attr(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("attr expects a selector string as argument 1".into()),
    };
    let name = match args.get(1) {
        Some(Value::String(s)) => s,
        _ => return Err("attr expects an attribute name string as argument 2".into()),
    };
    with_session(|s| {
        let expr = format!(
            "document.querySelector({sel:?})?.getAttribute({name:?})",
            sel = selector,
            name = name
        );
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_page_text() -> Result<Value, String> {
    with_session(|s| {
        let result = s.evaluate("document.body?.innerText ?? \"\"")?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_wait_for_ms(args: &Vec<Value>) -> Result<Value, String> {
    let selector = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err("wait_for_ms expects a selector string as argument 1".into()),
    };
    let max_ms = match args.get(1) {
        Some(Value::Number(n)) => (*n as u64).min(60_000),
        _ => return Err("wait_for_ms expects a timeout in ms as argument 2".into()),
    };
    with_session(|s| {
        let expr = format!(
            "new Promise(resolve => {{ const t0 = Date.now(); const check = () => {{ if (document.querySelector({sel:?})) {{ resolve(true); }} else if (Date.now() - t0 > {ms}) {{ resolve(false); }} else {{ setTimeout(check, 100); }} }}; check(); }})",
            sel = selector,
            ms = max_ms
        );
        let result = s.evaluate(&expr)?;
        Ok(js_value_to_zen(result))
    })
}

pub fn browser_close() -> Result<Value, String> {
    SESSION.with(|s| *s.borrow_mut() = None);
    BROWSER.with(|b| {
        let mut borrow = b.borrow_mut();
        if let Some(browser) = borrow.as_mut() {
            if let Some(mut child) = browser.child.take() {
                let _ = child.kill();
            }
        }
        *borrow = None;
    });
    Ok(Value::Bool(true))
}

fn with_session<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut CdpSession) -> Result<R, String>,
{
    SESSION.with(|s| {
        let mut borrow = s.borrow_mut();
        match borrow.as_mut() {
            Some(session) => f(session),
            None => Err("no browser session. call browser_launch() first".into()),
        }
    })
}

// Convert a JS RemoteObject value to a Zen value
fn js_value_to_zen(result: serde_json::Value) -> Value {
    // The evaluate() helper returns the "result" field of RemoteObject
    // which has {type, value, description, objectId...}
    let value = result.get("value").cloned().unwrap_or(serde_json::Value::Null);
    match value {
        serde_json::Value::Null => {
            // Maybe the result is undefined - treat as null
            Value::Null
        }
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(items) => {
            Value::List(Arc::new(items.into_iter().map(js_json_to_zen).collect::<Vec<_>>()))
        }
        serde_json::Value::Object(map) => {
            let mut dict = ahash::AHashMap::new();
            for (k, v) in map {
                dict.insert(k, js_json_to_zen(v));
            }
            Value::Dict(Arc::new(dict))
        }
    }
}

fn js_json_to_zen(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(items) => Value::List(Arc::new(items.into_iter().map(js_json_to_zen).collect::<Vec<_>>())),
        serde_json::Value::Object(map) => {
            let mut dict = ahash::AHashMap::new();
            for (k, v) in map {
                dict.insert(k, js_json_to_zen(v));
            }
            Value::Dict(Arc::new(dict))
        }
    }
}