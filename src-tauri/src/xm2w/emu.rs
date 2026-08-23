//! In-memory XM2w firmware emulator.
//!
//! Replicates the real device's config behavior (mapped via hardware sweeps):
//!   [0xA1][0x12]                     -> full 8192-byte config blob
//!   [0xA1][0x02]                     -> fw version response ("16.1 (id 0x1001)")
//!   [0xA1][0x13]                     -> factory reset
//!   [0xA1][0x14 0x0F 0x1C] + 32B     -> sensor/CPI block + mirrors
//!   [0xA1][0x15 0x0F 0x1C] + 28B     -> B region (10B) + C region (3B)
//!   [0xA1][0x16 0x0F 0x1C] part 1/2  -> cfg[0x47:0x63]/[0x63:0x7F]
//!                                       + mirror A (+0x404), B (0x814), C chunks
//!   any other part/command           -> B region (0x814..0x830) only
//!   [0xA0] write                     -> ignored (real device ignores it too)
//!
//! Also simulates physical button presses so the UI can be tested end-to-end
//! without a mouse: `press(slot)` interprets a slot's struct into an action.

use super::Transport;
use std::sync::Mutex;

pub const BLOB_SIZE: usize = 8192;
pub const FACTORY_CPI: [u16; 4] = [400, 800, 1600, 3200];
pub const FW_MAJOR: u8 = 0x10;
pub const FW_MINOR: u8 = 0x01;

/// Physical button names by config slot (mirrors the UI's BUTTON_NAMES).
pub const SLOT_NAMES: [&str; 8] = [
    "Left", "Right", "Middle", "Back", "Forward", "DPI", "Scroll Up", "Scroll Down",
];

pub fn factory_blob() -> [u8; BLOB_SIZE] {
    let mut b = [0u8; BLOB_SIZE];
    b[0] = 0xA1;
    b[1] = 0x01;
    b[0x11] = 0x80;
    b[0x13] = 0x03;
    b[0x14] = 0x01;
    b[0x15] = 0x02;
    b[0x16] = 0x11;
    b[0x20] = 0xFF;
    b[0x21] = 0x02; // divider 2 = 4000 Hz
    b[0x22] = 0x11; // flags: slamclick + jitter
    b[0x25] = 0x01; // LOD
    b[0x30] = 0x04; // cpi_levels
    for (i, cpi) in FACTORY_CPI.iter().enumerate() {
        let base = 51 + i * 5;
        b[base + 1] = (cpi & 0xFF) as u8;
        b[base + 2] = (cpi >> 8) as u8;
        b[base + 3] = (cpi & 0xFF) as u8;
        b[base + 4] = (cpi >> 8) as u8;
    }
    // 8 button structs at verified offsets, layout [kind][v0..v4][debounce]
    // 0x4E=Right 0x55=Middle 0x5C=Back 0x63=Forward 0x6A=DPI 0x71=Up 0x78=Down
    // Left at 0x7F (outside the writable delta; v0=0 = Left click)
    let buttons: [(usize, u8, [u8; 5]); 8] = [
        (0x7F, 0x00, [0x00, 0, 0, 0, 0]), // Left (locked)
        (0x4E, 0x00, [0x02, 0, 0, 0, 0]), // Right
        (0x55, 0x00, [0x04, 0, 0, 0, 0]), // Middle
        (0x5C, 0x00, [0x08, 0, 0, 0, 0]), // Back
        (0x63, 0x00, [0x10, 0, 0, 0, 0]), // Forward
        (0x6A, 0x09, [0xF1, 0, 0, 0, 0]), // DPI
        (0x71, 0x01, [0x01, 0, 0, 0, 0]), // Scroll up
        (0x78, 0x01, [0xFF, 0, 0, 0, 0]), // Scroll down
    ];
    for (off, kind, val) in buttons.iter() {
        b[*off] = *kind;
        b[*off + 1..*off + 6].copy_from_slice(val);
        if *off != 0x7F {
            b[*off + 6] = 8; // debounce (Left@0x7F is outside the writable delta)
        }
    }
    b
}

/// Interpret a slot's struct into a human-readable action (for simulation).
pub fn interpret_slot(b: &[u8]) -> String {
    let kind = b[0];
    let v = &b[1..6];
    match kind {
        0x00 => match v[0] {
            0x00 => "Left click".into(),
            0x02 => "Right click".into(),
            0x04 => "Middle click".into(),
            0x08 => "Back".into(),
            0x10 => "Forward".into(),
            other => format!("Mouse 0x{other:02x}"),
        },
        0x01 => {
            if v[0] == 1 {
                "Scroll up".into()
            } else {
                "Scroll down".into()
            }
        }
        0x02 => {
            let mods = v[0];
            let code = v[1];
            let mut s = String::new();
            if mods & 1 != 0 {
                s.push_str("Ctrl+");
            }
            if mods & 2 != 0 {
                s.push_str("Shift+");
            }
            if mods & 4 != 0 {
                s.push_str("Alt+");
            }
            if mods & 8 != 0 {
                s.push_str("Win+");
            }
            s.push_str(&key_name(code));
            s
        }
        0x09 => "DPI cycle".into(),
        0xFF => "Disabled".into(),
        other => format!("Kind 0x{other:02x}"),
    }
}

/// HID usage table (device stores usage-1).
fn key_name(dev_code: u8) -> String {
    let usage = (dev_code as u16 + 1) & 0xFF;
    let name = match usage {
        0x04..=0x1D => ((b'A' + (usage - 0x04) as u8) as char).to_string(),
        0x1E..=0x27 => ((b'1' + (usage - 0x1E) as u8) as char).to_string(),
        0x28 => "Enter".into(),
        0x29 => "Esc".into(),
        0x2A => "Backspace".into(),
        0x2B => "Tab".into(),
        0x2C => "Space".into(),
        0x3A..=0x45 => format!("F{}", usage - 0x39),
        _ => format!("0x{usage:02x}"),
    };
    name
}

pub struct EmuTransport {
    pub blob: [u8; BLOB_SIZE],
    pending: Vec<u8>,
}

impl EmuTransport {
    pub fn new() -> Self {
        EmuTransport { blob: factory_blob(), pending: Vec::new() }
    }

    pub fn factory_reset(&mut self) {
        self.blob = factory_blob();
        self.pending.clear();
    }

    /// Simulate pressing a physical button: returns what the configured bind does.
    pub fn press(&mut self, slot: usize) -> Result<String, String> {
        if slot >= 8 {
            return Err(format!("slot {slot} out of range"));
        }
        let off = crate::xm2w::BUTTON_OFFSETS[slot];
        Ok(interpret_slot(&self.blob[off..off + 7]))
    }

    fn write_sensor_block(&mut self, block: &[u8]) {
        self.blob[0x17..0x1C].copy_from_slice(&block[0..5]);
        self.blob[0x1E] = block[6];
        self.blob[0x1D] = block[7];
        self.blob[0x33..0x47].copy_from_slice(&block[8..28]);
        // block[28..32] is ignored by the real device
        // mirrors
        self.blob[0x41B..0x420].copy_from_slice(&block[0..5]);
        self.blob[0x437..0x44B].copy_from_slice(&block[8..28]);
        self.blob[0x814..0x830].copy_from_slice(&block[..28]);
        // C region: chunked 7-byte writes at 8-byte stride
        for k in 0..4 {
            let dst = 0x90C + k * 8;
            self.blob[dst..dst + 7].copy_from_slice(&block[k * 8..k * 8 + 7]);
        }
    }

    fn write_button_part(&mut self, part: u8, block: &[u8]) {
        let src = match part {
            1 => 0x47usize,
            2 => 0x63usize,
            _ => {
                self.blob[0x814..0x830].copy_from_slice(block);
                return;
            }
        };
        self.blob[src..src + 28].copy_from_slice(block);
        self.blob[src + 0x404..src + 0x404 + 28].copy_from_slice(block);
        self.blob[0x814..0x830].copy_from_slice(block);
        // C: 4 chunks of 7 bytes at 8-byte stride (block chunks at 7-byte stride)
        let c_base = if part == 1 { 0x92E } else { 0x94E };
        for k in 0..4 {
            let dst = c_base + k * 8;
            self.blob[dst..dst + 7].copy_from_slice(&block[k * 7..k * 7 + 7]);
        }
    }

    fn write_15(&mut self, block: &[u8]) {
        // [0x15 0x0F 0x1C]: B region 10 bytes + C region 3 bytes
        self.blob[0x814..0x81E].copy_from_slice(&block[..10]);
        self.blob[0x909..0x90C].copy_from_slice(&block[10..13]);
    }

    fn fw_response(&self) -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[0] = 0xA1;
        r[1] = 0x01;
        r[17] = FW_MAJOR;
        r[18] = FW_MINOR;
        r
    }
}

impl Transport for EmuTransport {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String> {
        match report_id {
            0xA1 => {
                // payload = the app's buffer minus the report-id byte:
                // cmd at payload[0..3], part at payload[5], block at payload[15..43]
                if payload.len() >= 3 && payload[0] == 0x16 && payload[1] == 0x0F && payload[2] == 0x1C {
                    let part = if payload.len() > 5 { payload[5] } else { 0 };
                    if payload.len() >= 15 + 28 {
                        self.write_button_part(part, &payload[15..43]);
                    }
                } else if payload.len() >= 3 && payload[0] == 0x14 && payload[1] == 0x0F && payload[2] == 0x1C {
                    if payload.len() >= 15 + 32 {
                        let mut block = [0u8; 32];
                        block.copy_from_slice(&payload[15..47]);
                        self.write_sensor_block(&block);
                    }
                } else if payload.len() >= 3 && payload[0] == 0x15 && payload[1] == 0x0F && payload[2] == 0x1C {
                    if payload.len() >= 15 + 28 {
                        self.write_15(&payload[15..43]);
                    }
                } else if payload.len() >= 1 && payload[0] == 0x13 {
                    self.factory_reset();
                } else if payload.len() >= 1 && payload[0] == 0x02 {
                    self.pending = self.fw_response();
                } else if payload.len() >= 1 && payload[0] == 0x12 {
                    self.pending = self.blob.to_vec();
                }
            }
            0xA0 => {
                // real device ignores full-config writes via 0xA0
            }
            _ => {}
        }
        Ok(())
    }

    fn get_feature(&mut self, _report_id: u8, size: usize) -> Result<Vec<u8>, String> {
        if !self.pending.is_empty() {
            let n = size.min(self.pending.len());
            return Ok(self.pending[..n].to_vec());
        }
        let n = size.min(BLOB_SIZE);
        Ok(self.blob[..n].to_vec())
    }
}

pub static EMU: Mutex<Option<EmuTransport>> = Mutex::new(None);

pub fn emu_enabled() -> bool {
    EMU.lock().unwrap().is_some()
}

pub fn emu_set(enabled: bool) {
    let mut g = EMU.lock().unwrap();
    if enabled {
        *g = Some(EmuTransport::new());
    } else {
        *g = None;
    }
}

pub fn emu_lock() -> std::sync::MutexGuard<'static, Option<EmuTransport>> {
    EMU.lock().unwrap()
}

/// Transport handle that delegates to the global emulator instance.
pub struct EmuRef;

impl Transport for EmuRef {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String> {
        EMU.lock().unwrap().as_mut().unwrap().set_feature(report_id, payload)
    }
    fn get_feature(&mut self, report_id: u8, size: usize) -> Result<Vec<u8>, String> {
        EMU.lock().unwrap().as_mut().unwrap().get_feature(report_id, size)
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::xm2w::{Config, DeviceSettings, ButtonCfg};

    fn fresh() -> EmuTransport {
        EmuTransport::new()
    }

    fn settings(buttons: Vec<ButtonCfg>) -> DeviceSettings {
        DeviceSettings {
            polling_hz: 4000,
            lod: 1,
            slamclick: true,
            jitter: true,
            angle_snapping: false,
            ripple: false,
            motion_sync: false,
            cpi_levels: 4,
            cpis: vec![
                crate::xm2w::CpiLevel { split: false, x: 200, y: 200 },
                crate::xm2w::CpiLevel { split: false, x: 400, y: 400 },
                crate::xm2w::CpiLevel { split: false, x: 800, y: 800 },
                crate::xm2w::CpiLevel { split: false, x: 1600, y: 1600 },
            ],
            buttons,
        }
    }

    #[test]
    fn factory_state_is_sane() {
        let e = fresh();
        assert_eq!(e.blob[0x21], 0x02, "divider");
        assert_eq!(e.blob[0x22], 0x11, "flags");
        assert_eq!(e.blob[0x30], 0x04, "cpi levels");
        // verified layout: Right@0x4E, Middle@0x55, Back@0x5C, Forward@0x63
        assert_eq!(&e.blob[0x4E..0x55], &[0, 2, 0, 0, 0, 0, 8], "Right");
        assert_eq!(&e.blob[0x5C..0x63], &[0, 8, 0, 0, 0, 0, 8], "Back");
        assert_eq!(&e.blob[0x63..0x6A], &[0, 0x10, 0, 0, 0, 0, 8], "Forward");
        assert_eq!(&e.blob[0x7F..0x86], &[0, 0, 0, 0, 0, 0, 0], "Left");
    }

    #[test]
    fn fw_version_read() {
        let mut e = fresh();
        e.set_feature(0xA1, &[0x02]).unwrap();
        let r = e.get_feature(0xA1, 64).unwrap();
        assert_eq!(r[1], 1);
        assert_eq!(r[17], FW_MAJOR);
        assert_eq!(r[18], FW_MINOR);
    }

    #[test]
    fn factory_reset_restores() {
        let mut e = fresh();
        let mut arr0 = [0u8; 1041]; arr0.copy_from_slice(&e.blob[..1041]); let mut cfg = Config { blob: arr0 };
        cfg.apply_settings(&settings(vec![
            ButtonCfg { debounce: 8, kind: 0, value: [0, 0, 0, 0, 0] },   // Left (locked)
            ButtonCfg { debounce: 8, kind: 2, value: [1, 25, 0, 0, 0] },  // Right = Ctrl+W
            ButtonCfg { debounce: 8, kind: 0, value: [4, 0, 0, 0, 0] },   // Middle
            ButtonCfg { debounce: 8, kind: 2, value: [0, 36, 0, 0, 0] },  // Back = key 8
            ButtonCfg { debounce: 8, kind: 2, value: [0, 37, 0, 0, 0] },  // Forward = key 9
            ButtonCfg { debounce: 8, kind: 9, value: [0xF1, 0, 0, 0, 0] },// DPI
            ButtonCfg { debounce: 8, kind: 1, value: [1, 0, 0, 0, 0] },   // Up
            ButtonCfg { debounce: 8, kind: 1, value: [0xFF, 0, 0, 0, 0] },// Down
        ]));
        e.write_button_part(1, &cfg.blob[0x47..0x63]);
        e.write_button_part(2, &cfg.blob[0x63..0x7F]);
        assert_eq!(&e.blob[0x4E..0x55], &[2, 1, 25, 0, 0, 0, 8], "Right = Ctrl+W");
        assert_eq!(&e.blob[0x5C..0x63], &[2, 0, 36, 0, 0, 0, 8], "Back = key 8");
        assert_eq!(&e.blob[0x63..0x6A], &[2, 0, 37, 0, 0, 0, 8], "Forward = key 9");
        e.set_feature(0xA1, &[0x13]).unwrap();
        assert_eq!(&e.blob[0x4E..0x55], &[0, 2, 0, 0, 0, 0, 8], "reset -> Right");
    }

    #[test]
    fn part_writes_hit_all_mirrors() {
        let mut e = fresh();
        let block = [0x5Au8; 28];
        e.write_button_part(1, &block);
        assert_eq!(&e.blob[0x47..0x63], &block[..], "main part1");
        assert_eq!(&e.blob[0x44B..0x467], &block[..], "mirror A");
        assert_eq!(&e.blob[0x814..0x830], &block[..], "mirror B");
        assert_eq!(&e.blob[0x92E..0x935], &block[0..7], "C chunk 0");
        assert_eq!(&e.blob[0x936..0x93D], &block[7..14], "C chunk 1");
        assert_eq!(&e.blob[0x93E..0x945], &block[14..21], "C chunk 2");
        assert_eq!(&e.blob[0x946..0x94D], &block[21..28], "C chunk 3");
        // other parts only touch B
        e.write_button_part(9, &block);
        assert_eq!(&e.blob[0x47..0x63], &block[..], "main untouched by part 9");
        assert_eq!(&e.blob[0x814..0x830], &block[..], "B still block");
    }

    #[test]
    fn sensor_write_maps_regions() {
        let mut e = fresh();
        let mut block = [0u8; 32];
        block[..28].fill(0x3C);
        e.write_sensor_block(&block);
        assert!(e.blob[0x17..0x1C].iter().all(|&x| x == 0x3C));
        assert!(e.blob[0x33..0x47].iter().all(|&x| x == 0x3C));
        assert!(e.blob[0x814..0x830].iter().all(|&x| x == 0x3C));
    }

    #[test]
    fn press_interprets_binds() {
        let mut e = fresh();
        assert_eq!(e.press(0).unwrap(), "Left click");
        assert_eq!(e.press(1).unwrap(), "Right click");
        assert_eq!(e.press(3).unwrap(), "Back");
        assert_eq!(e.press(4).unwrap(), "Forward");
        assert_eq!(e.press(5).unwrap(), "DPI cycle");
        // bind Back (0x5C) = Ctrl+W and press it
        let off = 0x5C;
        e.blob[off..off + 7].copy_from_slice(&[2, 1, 25, 0, 0, 0, 8]);
        assert_eq!(e.press(3).unwrap(), "Ctrl+W");
        // key 9: usage 0x26 -> dev code 0x25 = 37
        e.blob[off..off + 7].copy_from_slice(&[2, 0, 37, 0, 0, 0, 8]);
        assert_eq!(e.press(3).unwrap(), "9");
        assert!(e.press(8).is_err());
    }

    #[test]
    fn back_button_via_part1() {
        // the Back button lives at 0x5C..0x62 and is written by part 1's
        // last 7 bytes (block[21..27]) - verified on real hardware
        let mut e = fresh();
        let mut block = [0x00u8; 28];
        block[21..28].copy_from_slice(&[2, 0, 36, 0, 0, 0, 8]); // key 8
        e.write_button_part(1, &block);
        assert_eq!(&e.blob[0x5C..0x63], &[2, 0, 36, 0, 0, 0, 8], "Back = key 8");
        assert_eq!(e.press(3).unwrap(), "8", "rear button now types 8");
        // write Forward = key 9 via part 2's first bytes
        let mut block2 = [0x00u8; 28];
        block2[0..7].copy_from_slice(&[2, 0, 37, 0, 0, 0, 8]);
        e.write_button_part(2, &block2);
        assert_eq!(e.press(4).unwrap(), "9", "front button types 9");
    }

    #[test]
    fn set_feature_payload_offsets() {
        // the app builds: [0xA1][cmd 3B][0][part][9 zeros][block 28B] and sends
        // buf[1..] as payload -> block lands at payload[15..43]
        let mut e = fresh();
        let mut payload = vec![0u8; 43];
        payload[0..3].copy_from_slice(&[0x16, 0x0F, 0x1C]);
        payload[5] = 2; // part
        payload[15..43].fill(0x6B);
        e.set_feature(0xA1, &payload).unwrap();
        assert_eq!(&e.blob[0x63..0x7F], &payload[15..43], "part 2 lands at 0x63");
        assert_eq!(&e.blob[0x814..0x830], &payload[15..43], "B mirror");
        // fw version
        let mut e2 = fresh();
        e2.set_feature(0xA1, &[0x02]).unwrap();
        let r = e2.get_feature(0xA1, 64).unwrap();
        assert_eq!(r[17], FW_MAJOR);
    }

    #[test]
    fn config_roundtrip_via_parts() {
        // simulate the app's apply path against the transport
        let mut e = fresh();
        let s = settings(vec![
            ButtonCfg { debounce: 8, kind: 0, value: [0, 0, 0, 0, 0] },   // Left (locked)
            ButtonCfg { debounce: 8, kind: 2, value: [1, 25, 0, 0, 0] },  // Right = Ctrl+W
            ButtonCfg { debounce: 8, kind: 0, value: [4, 0, 0, 0, 0] },   // Middle
            ButtonCfg { debounce: 8, kind: 2, value: [0, 36, 0, 0, 0] },  // Back = key 8
            ButtonCfg { debounce: 8, kind: 2, value: [0, 37, 0, 0, 0] },  // Forward = key 9
            ButtonCfg { debounce: 8, kind: 9, value: [0xF1, 0, 0, 0, 0] },// DPI
            ButtonCfg { debounce: 8, kind: 1, value: [1, 0, 0, 0, 0] },   // Up
            ButtonCfg { debounce: 8, kind: 1, value: [0xFF, 0, 0, 0, 0] },// Down
        ]);
        let mut arr0 = [0u8; 1041]; arr0.copy_from_slice(&e.blob[..1041]); let mut cfg = Config { blob: arr0 };
        cfg.apply_settings(&s);
        // sensor/CPI block exactly like the app's write_sensor
        let mut block = [0u8; 32];
        block[0..5].copy_from_slice(&cfg.blob[0x17..0x1C]);
        block[6] = cfg.blob[0x1E];
        block[7] = cfg.blob[0x1D];
        block[8..28].copy_from_slice(&cfg.blob[0x33..0x47]);
        e.write_sensor_block(&block);
        e.write_button_part(1, &cfg.blob[0x47..0x63]);
        e.write_button_part(2, &cfg.blob[0x63..0x7F]);
        // read back via the transport's view (as the app does)
        e.set_feature(0xA1, &[0x12]).unwrap();
        let back = e.get_feature(0xA1, 1041).unwrap();
        let mut arr = [0u8; 1041];
        arr.copy_from_slice(&back[..1041]);
        let cfg2 = Config { blob: arr };
        let s2 = cfg2.to_settings();
        assert_eq!(s2.buttons[1].kind, 2);
        assert_eq!(s2.buttons[1].value, [1, 25, 0, 0, 0]);
        assert_eq!(s2.buttons[3].value, [0, 36, 0, 0, 0]); // Back = key 8
        assert_eq!(s2.buttons[4].value, [0, 37, 0, 0, 0]); // Forward = key 9
        assert_eq!(s2.cpis[0].x, 200);
        assert_eq!(s2.polling_hz, 4000);
    }
}
