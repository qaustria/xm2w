//! Embedded HTTP server: serves the UI + mouse API on 127.0.0.1 (random port).

use std::io::Read;
use std::path::Path;

use tiny_http::{Header, Response, Server, StatusCode};

use crate::xm2w::{Device, DeviceSettings, Transport};

const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui");

fn mime(path: &str) -> &'static str {
    if path.ends_with(".html") { "text/html; charset=utf-8" }
    else if path.ends_with(".css") { "text/css; charset=utf-8" }
    else if path.ends_with(".js") { "text/javascript; charset=utf-8" }
    else if path.ends_with(".png") { "image/png" }
    else if path.ends_with(".jpg") || path.ends_with(".jpeg") { "image/jpeg" }
    else if path.ends_with(".woff2") { "font/woff2" }
    else if path.ends_with(".svg") { "image/svg+xml" }
    else { "application/octet-stream" }
}

fn json_response(body: String, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(body.into_bytes())
        .with_status_code(StatusCode(code))
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
        .with_header(Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap())
}

fn with_device<T: serde::Serialize>(f: impl FnOnce(&mut Device<Box<dyn Transport>>) -> Result<T, String>) -> String {
    if crate::xm2w::emu::emu_enabled() {
        let t: Box<dyn Transport> = Box::new(crate::xm2w::emu::EmuRef);
        let mut dev = Device { t };
        return match f(&mut dev) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        };
    }
    match crate::xm2w::open_device() {
        Ok(mut dev) => match f(&mut dev) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        },
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

fn handle_api(method: &str, path: &str, body: &str) -> String {
    match (method, path) {
        ("GET", "/api/permissions") => crate::permissions::status().to_string(),
        ("POST", "/api/permissions/request") => crate::permissions::request_all().to_string(),
        ("POST", "/api/emu") => {
            let on = body.contains("true");
            crate::xm2w::emu::emu_set(on);
            format!("{{\"emu\":{on}}}")
        }
        ("POST", "/api/emu/press") => {
            #[derive(serde::Deserialize)]
            struct Press { slot: usize }
            match serde_json::from_str::<Press>(body) {
                Err(e) => format!("{{\"error\":\"bad json: {e}\"}}"),
                Ok(p) => {
                    let mut g = crate::xm2w::emu::emu_lock();
                    match g.as_mut() {
                        None => "{\"error\":\"emulator not enabled\"}".into(),
                        Some(e) => match e.press(p.slot) {
                            Ok(action) => {
                                let name = crate::xm2w::emu::SLOT_NAMES
                                    .get(p.slot)
                                    .copied()
                                    .unwrap_or("?");
                                serde_json::json!({ "slot": p.slot, "name": name, "action": action }).to_string()
                            }
                            Err(err) => format!("{{\"error\":\"{err}\"}}"),
                        },
                    }
                }
            }
        }
        ("GET", "/api/info") => with_device(|dev| {
            let fw = dev.get_fw_version()?;
            let cfg = dev.read_config()?;
            let emu = crate::xm2w::emu::emu_enabled();
            Ok(serde_json::json!({ "fw": fw, "emu": emu, "settings": cfg.to_settings() }))
        }),
        ("GET", "/api/raw") => with_device(|dev| {
            let cfg = dev.read_config()?;
            let hex: String = cfg.blob.iter().map(|b| format!("{b:02x}")).collect();
            Ok(serde_json::json!({ "hex": hex, "len": cfg.blob.len() }))
        }),
        ("POST", "/api/apply") => {
            let settings: DeviceSettings = match serde_json::from_str(body) {
                Ok(s) => s,
                Err(e) => return format!("{{\"error\":\"bad json: {e}\"}}"),
            };
            with_device(|dev| {
                let mut cfg = dev.read_config()?;
                cfg.apply_settings(&settings);
                dev.write_sensor(&cfg)?;
                dev.write_buttons(&cfg)?;
                let fresh = dev.read_config()?;
                Ok(serde_json::json!({ "settings": fresh.to_settings() }))
            })
        }
        ("POST", "/api/reset") => with_device(|dev| {
            dev.factory_reset()?;
            Ok(serde_json::json!({ "ok": true }))
        }),
        ("POST", "/api/log") => {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/xm2w_ui.log") {
                let _ = writeln!(f, "{body}");
            }
            "{\"ok\":true}".to_string()
        }
        _ => format!("{{\"error\":\"unknown endpoint {method} {path}\"}}"),
    }
}

/// Start the server on a random localhost port; returns the port.
pub fn start(port_hint: u16) -> Result<u16, String> {
    let server = Server::http(("127.0.0.1", port_hint)).map_err(|e| e.to_string())?;
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(port_hint);
    let _ = std::fs::write("/tmp/xm2w_port.txt", port.to_string());

    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let url = req.url().to_string();
            let method = req.method().to_string();
            let mut body = String::new();
            if method == "POST" {
                let _ = req.as_reader().take(1 << 20).read_to_string(&mut body);
            }
            let resp: Response<std::io::Cursor<Vec<u8>>> = if url.starts_with("/api/") {
                let j = handle_api(&method, &url, &body);
                json_response(j.clone(), if j.contains("\"error\"") { 500 } else { 200 })
            } else {
                let rel = if url == "/" { "index.html" } else { url.trim_start_matches('/') };
                let safe = rel.split('?').next().unwrap_or("index.html");
                let p = Path::new(UI_DIR).join(safe);
                match std::fs::read(&p) {
                    Ok(data) => Response::from_data(data)
                        .with_header(Header::from_bytes(&b"Content-Type"[..], mime(&p.to_string_lossy()).as_bytes()).unwrap())
                        .with_header(Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap()),
                    Err(_) => Response::from_data(b"not found".to_vec()).with_status_code(StatusCode(404)),
                }
            };
            let _ = req.respond(resp);
        }
    });
    Ok(port)
}
