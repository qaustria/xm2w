mod web_server;
pub mod xm2w;

use tauri::{WebviewUrl, WebviewWindowBuilder};

pub fn run() {
    // --silent: run the server only, no window (headless CLI mode)
    // --emu: start in emulator mode (no real mouse needed)
    let silent = std::env::args().any(|a| a == "--silent");
    if std::env::args().any(|a| a == "--emu") {
        xm2w::emu::emu_set(true);
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .setup(move |app| {
            // start embedded web server on a random port
            let port = web_server::start(0).map_err(|e| e.to_string())?;
            if !silent {
                let url = format!("http://127.0.0.1:{port}");
                WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse().unwrap()))
                    .title("XM2w Control")
                    .inner_size(1000.0, 780.0)
                    .min_inner_size(860.0, 640.0)
                    .center()
                    .build()?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri app");
    app.run(|_handle, _event| {});
}
