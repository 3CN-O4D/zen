
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct CdpBrowser {
    pub port: u16,
    pub child: Option<Child>,
}

pub struct CdpSession {
    sender: mpsc::Sender<String>,
    receiver: mpsc::Receiver<(u64, serde_json::Value)>,
    next_id: u64,
}

impl CdpBrowser {
    pub fn launch(chrome_path: Option<String>, port: u16, headless: bool, user_data_dir: String) -> Result<CdpBrowser, String> {
        let chrome = chrome_path.unwrap_or_else(|| {
            // Find chrome in common locations
            let candidates = [
                "/usr/bin/chromium",
                "/usr/bin/google-chrome",
                "/usr/bin/chromium-browser",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "C:/Program Files/Google/Chrome/Application/chrome.exe",
                "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
            ];
            candidates.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string()).unwrap_or_else(|| "chromium".into())
        });

        let mut args = vec![
            format!("--remote-debugging-port={port}"),
            format!("--user-data-dir={user_data_dir}"),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-background-networking".into(),
            "--disable-component-update".into(),
            "--disable-default-apps".into(),
            "--disable-sync".into(),
            "--disable-translate".into(),
            "--disable-extensions".into(),
            "--disable-gpu".into(),
            "--disable-software-rasterizer".into(),
            "--disable-dev-shm-usage".into(),
            "--disable-features=Translate".into(),
            "--metrics-recording-only".into(),
            "--mute-audio".into(),
            "--about:blank".into(),
        ];
        if headless {
            args.push("--headless=new".into());
        }

        let child = Command::new(&chrome)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to launch chrome ({chrome}): {e}"))?;

        // Wait for the debugging port to become available
        let mut attempts = 0;
        while attempts < 50 {
            if let Ok(response) = reqwest::blocking::get(format!("http://127.0.0.1:{port}/json/version")) {
                if response.status().is_success() {
                    return Ok(CdpBrowser { port, child: Some(child) });
                }
            }
            thread::sleep(Duration::from_millis(200));
            attempts += 1;
        }

        Err(format!("chrome did not open debugging port {port} in time"))
    }

    pub fn connect(&self) -> Result<CdpSession, String> {
        let targets = self.get_targets()?;
        let url = targets
            .iter()
            .find(|t| t["type"] == "page")
            .and_then(|t| t["webSocketDebuggerUrl"].as_str())
            .ok_or_else(|| "no page target found".to_string())?;
        CdpSession::connect(url.to_string())
    }

    pub fn get_version(&self) -> Result<serde_json::Value, String> {
        let response = reqwest::blocking::get(format!("http://127.0.0.1:{}/json/version", self.port))
            .map_err(|e| e.to_string())?;
        response.json().map_err(|e| e.to_string())
    }

    pub fn get_targets(&self) -> Result<Vec<serde_json::Value>, String> {
        let response = reqwest::blocking::get(format!("http://127.0.0.1:{}/json/list", self.port))
            .map_err(|e| e.to_string())?;
        response.json().map_err(|e| e.to_string())
    }

    pub fn create_target(&self, url: &str) -> Result<String, String> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .put(format!("http://127.0.0.1:{}/json/new?{}", self.port, url))
            .send()
            .map_err(|e| e.to_string())?;
        let json: serde_json::Value = response.json().map_err(|e| e.to_string())?;
        json["id"].as_str().map(|s| s.to_string()).ok_or_else(|| "no target id".into())
    }
}

impl CdpSession {
    pub fn connect(url: String) -> Result<CdpSession, String> {
        let (tx_sender, rx_receiver) = mpsc::channel::<(u64, serde_json::Value)>();
        let (ws_tx, ws_rx) = mpsc::channel::<String>();

        let thread_sender = tx_sender.clone();
        let ws_url = url.clone();

        thread::spawn(move || {
            let Ok(client) = websocket::ClientBuilder::new(&ws_url).unwrap().connect_insecure() else {
                return;
            };
            let (mut reader, mut writer) = client.split().unwrap();

            let reader_sender = thread_sender;
            let writer_rx = ws_rx;

            let read_thread = thread::spawn(move || {
                for message in reader.incoming_messages() {
                    if let Ok(websocket::OwnedMessage::Text(text)) = message {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(id) = json.get("id").and_then(|v| v.as_u64()) {
                                let _ = reader_sender.send((id, json));
                            }
                            // Events have no "id" — dispatch elsewhere
                        }
                    }
                }
            });

            while let Ok(text) = writer_rx.recv() {
                if writer.send_message(&websocket::OwnedMessage::Text(text)).is_err() {
                    break;
                }
            }

            read_thread.join().ok();
        });

        Ok(CdpSession {
            sender: ws_tx,
            receiver: rx_receiver,
            next_id: 1,
        })
    }

    pub fn command(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;

        let mut msg = serde_json::Map::new();
        msg.insert("id".into(), serde_json::json!(id));
        msg.insert("method".into(), serde_json::json!(method));
        if let Some(p) = params {
            msg.insert("params".into(), p);
        }

        self.sender
            .send(serde_json::Value::Object(msg).to_string())
            .map_err(|e| format!("failed to send command: {e}"))?;

        // Wait for response with matching id
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        while std::time::Instant::now() < deadline {
            let (resp_id, result) = match self
                .receiver
                .recv_timeout(Duration::from_millis(500))
            {
                Ok(m) => m,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("CDP connection closed".to_string()),
            };
            if resp_id == id {
                if let Some(error) = result.get("error") {
                    return Err(format!("CDP error: {error}"));
                }
                return Ok(result.get("result").cloned().unwrap_or(serde_json::Value::Null));
            }
            // Ignore responses for other ids (out of order)
        }
        Err("timed out waiting for CDP response".into())
    }

    pub fn evaluate(&mut self, expression: &str) -> Result<serde_json::Value, String> {
        let params = serde_json::json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        });
        let result = self.command("Runtime.evaluate", Some(params))?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("JS exception: {exception}"));
        }
        Ok(result.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }
}
