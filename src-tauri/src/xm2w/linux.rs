//! Linux transport: hidapi (hidraw). Requires udev rules for non-root access.

use super::Transport;
use std::os::raw::c_int;

pub struct LinuxTransport {
    dev: hidapi::HidDevice,
}

impl LinuxTransport {
    pub fn open(pid: u16) -> Result<Self, String> {
        let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
        let mut paths: Vec<(c_int, String)> = Vec::new();
        for info in api.device_list() {
            if info.vendor_id() == 0x3367 && info.product_id() == pid {
                paths.push((info.interface_number(), info.path().to_string_lossy().into_owned()));
            }
        }
        if paths.is_empty() {
            return Err("device not found - is the mouse plugged in?".into());
        }
        // prefer interface 1 (vendor config interface)
        paths.sort_by_key(|p| std::cmp::Reverse(p.0));
        let mut last_err = String::new();
        for (_, path) in &paths {
            match api.open_path(path) {
                Ok(dev) => return Ok(LinuxTransport { dev }),
                Err(e) => last_err = e.to_string(),
            }
        }
        Err(format!(
            "failed to open config interface: {last_err}\ninstall udev rules (60-endgamegear.rules)"
        ))
    }
}

impl Transport for LinuxTransport {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String> {
        let mut buf = Vec::with_capacity(payload.len() + 1);
        buf.push(report_id);
        buf.extend_from_slice(payload);
        self.dev
            .send_feature_report(&buf)
            .map_err(|e| format!("SetReport(0x{report_id:02x}) failed: {e}"))?;
        Ok(())
    }

    fn get_feature(&mut self, report_id: u8, size: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; size];
        buf[0] = report_id;
        let n = self
            .dev
            .get_feature_report(&mut buf)
            .map_err(|e| format!("GetReport(0x{report_id:02x}) failed: {e}"))?;
        buf.truncate(n);
        Ok(buf)
    }
}
