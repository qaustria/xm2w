//! XM2w web server: serves the UI and exposes the mouse over HTTP.
//! Run: cargo run --bin web   ->   open http://127.0.0.1:8723

use std::io::Read;
use std::path::Path;
use tiny_http::{Header, Response, Server, StatusCode};

use xm2w::xm2w::{Device, DeviceSettings, Transport, open_device};

const PORT: u16 = 8723;
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
    match open_device() {
        Ok(mut dev) => match f(&mut dev) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
            Err(e) => format!("{{\"error\":\"{e}\"}}"),
        },
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

fn handle_api(method: &str, path: &str, body: &str) -> String {
    match (method, path) {
        ("GET", "/api/info") => with_device(|dev| {
            let fw = dev.get_fw_version()?;
            let cfg = dev.read_config()?;
            Ok(serde_json::json!({ "fw": fw, "settings": cfg.to_settings() }))
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
        _ => format!("{{\"error\":\"unknown endpoint {method} {path}\"}}"),
    }
}

fn main() {
    let server = Server::http(format!("127.0.0.1:{PORT}")).expect("bind failed (port in use?)");
    println!("XM2w web server on http://127.0.0.1:{PORT}");

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
            // static file
            let rel = if url == "/" { "index.html" } else { url.trim_start_matches('/') };
            let safe = rel.split('?').next().unwrap_or("index.html");
            let p = Path::new(UI_DIR).join(safe);
            match std::fs::read(&p) {
                Ok(data) => Response::from_data(data)
                    .with_header(Header::from_bytes(&b"Content-Type"[..], mime(&p.to_string_lossy()).as_bytes()).unwrap())
                    .with_header(Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap()),
                Err(_) => Response::from_data(b"not found".to_vec())
                    .with_status_code(StatusCode(404)),
            }
        };
        let _ = req.respond(resp);
    }
}
