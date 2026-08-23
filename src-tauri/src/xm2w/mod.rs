//! XM2w protocol implementation (verified against FW 1.10 on real hardware
//! and the official Endgame Gear configuration tool binary).
//!
//! Commands (feature reports on the vendor HID interface, usage page 0xFF01):
//!   read config   [0xA1][0x12]  -> 1041-byte blob (8192 with padding/mirrors)
//!   fw version    [0xA1][0x02]  -> version at bytes 17/18
//!   factory reset [0xA1][0x13]
//!   sensor write  [0xA1][0x14 0x0F 0x1C] + 32B block
//!                  block[0:5]  -> cfg[0x17:0x1C]  block[6] -> cfg[0x1E]
//!                  block[7]    -> cfg[0x1D]       block[8:28] -> cfg[0x33:0x47] (CPI)
//!   buttons write [0xA1][0x16 0x0F 0x1C] + 2 x 28B parts -> cfg[0x47:0x7F]
//!
//! Button structs (7 bytes): [kind][v0][v1][v2][v3][v4][debounce]
//!   Right@0x4E  Middle@0x55  Back@0x5C  Forward@0x63  DPI@0x6A
//!   ScrollUp@0x71  ScrollDown@0x78  Left@0x7F (outside writable delta)
//! Mouse codes: 0=Left 2=Right 4=Middle 8=Back 0x10=Forward
//! Key codes are HID usage-1 (device table is shifted by one).
//!
//! Config blob offsets (decimal): divider@21, flags@22, LOD@25, angle@26,
//! ripple@27, motion@28, cpi_levels@30, cpis@51 (5B).

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod emu;

use serde::{Deserialize, Serialize};

pub const VID: u16 = 0x3367;
pub const PID_MOUSE: u16 = 0x1968;

pub const CONFIG_SIZE: usize = 1041;
pub const CPI_COUNT: usize = 4;
pub const BUTTON_COUNT: usize = 8;

// config offsets (decimal)
pub const OFF_DIVIDER: usize = 21;
pub const OFF_FLAGS: usize = 22;
pub const OFF_LOD: usize = 25;
pub const OFF_ANGLE: usize = 26;
pub const OFF_RIPPLE: usize = 27;
pub const OFF_MOTION: usize = 28;
pub const OFF_CPI_LEVELS: usize = 30;
pub const OFF_CPIS: usize = 51;

pub const FLAG_SLAMCLICK: u8 = 0x01;
pub const FLAG_JITTER: u8 = 0x10;

// button mapping types
pub const MAP_MOUSE: u8 = 0x00;
pub const MAP_SCROLL: u8 = 0x01;
pub const MAP_KEYBOARD: u8 = 0x02;
pub const MAP_CPI_LOOP: u8 = 0x09;
pub const MAP_DISABLE: u8 = 0xFF;

// verified codes: 0=Left, 2=Right, 4=Middle, 8=Back, 0x10=Forward
pub const MOUSE_LEFT: u8 = 0x00;
pub const MOUSE_RIGHT: u8 = 0x02;
pub const MOUSE_MIDDLE: u8 = 0x04;
pub const MOUSE_BACK: u8 = 0x08;
pub const MOUSE_FORWARD: u8 = 0x10;

pub const SCROLL_UP: u8 = 0x01;
pub const SCROLL_DOWN: u8 = 0xFF;

// verified button struct offsets (7 bytes each, layout [kind][v0..v4][debounce]):
// 0x4E=Right 0x55=Middle 0x5C=Back 0x63=Forward 0x6A=DPI 0x71=Up 0x78=Down;
// Left lives at 0x7F (outside the writable delta -> not rebindable).
pub const BUTTON_OFFSETS: [usize; 8] = [0x7F, 0x4E, 0x55, 0x5C, 0x63, 0x6A, 0x71, 0x78];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpiLevel {
    pub x: u16,
    pub y: u16,
    pub split: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonCfg {
    pub debounce: u8,
    pub kind: u8,
    pub value: [u8; 5],
}

impl ButtonCfg {
    pub fn mouse(key: u8) -> Self {
        let mut value = [0u8; 5];
        value[0] = key;
        Self { debounce: 8, kind: MAP_MOUSE, value }
    }
    pub fn scroll(dir: u8) -> Self {
        let mut value = [0u8; 5];
        value[0] = dir;
        Self { debounce: 8, kind: MAP_SCROLL, value }
    }
    pub fn key(mods: u8, usage: u8) -> Self {
        let mut value = [0u8; 5];
        value[0] = mods;
        value[1] = usage;
        Self { debounce: 8, kind: MAP_KEYBOARD, value }
    }
    pub fn cpi_loop() -> Self {
        Self { debounce: 8, kind: MAP_CPI_LOOP, value: [0; 5] }
    }
    pub fn disabled() -> Self {
        Self { debounce: 8, kind: MAP_DISABLE, value: [0; 5] }
    }
    // device struct layout: [kind][v0][v1][v2][v3][v4][debounce]
    pub fn from_blob(b: &[u8; CONFIG_SIZE], off: usize) -> Self {
        let mut value = [0u8; 5];
        value.copy_from_slice(&b[off + 1..off + 6]);
        Self { debounce: b[off + 6], kind: b[off], value }
    }
    pub fn to_blob(&self, b: &mut [u8; CONFIG_SIZE], off: usize) {
        b[off] = self.kind;
        b[off + 1..off + 6].copy_from_slice(&self.value);
        b[off + 6] = self.debounce;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub polling_hz: u32,
    pub lod: u8,
    pub slamclick: bool,
    pub jitter: bool,
    pub angle_snapping: bool,
    pub ripple: bool,
    pub motion_sync: bool,
    pub cpi_levels: u8,
    pub cpis: Vec<CpiLevel>,
    pub buttons: Vec<ButtonCfg>,
}

pub struct Config {
    pub blob: [u8; CONFIG_SIZE],
}

impl Config {
    pub fn divider_to_hz(d: u8) -> u32 {
        match d {
            1 => 8000,
            2 => 4000,
            4 => 2000,
            _ => 1000,
        }
    }
    pub fn hz_to_divider(hz: u32) -> u8 {
        match hz {
            8000 => 1,
            4000 => 2,
            2000 => 4,
            _ => 8,
        }
    }
    pub fn to_settings(&self) -> DeviceSettings {
        let mut cpis = Vec::new();
        for i in 0..CPI_COUNT {
            let base = OFF_CPIS + i * 5;
            cpis.push(CpiLevel {
                split: self.blob[base] != 0,
                x: u16::from_le_bytes([self.blob[base + 1], self.blob[base + 2]]),
                y: u16::from_le_bytes([self.blob[base + 3], self.blob[base + 4]]),
            });
        }
        let mut buttons = Vec::new();
        for i in 0..BUTTON_COUNT {
            buttons.push(ButtonCfg::from_blob(&self.blob, BUTTON_OFFSETS[i]));
        }
        DeviceSettings {
            polling_hz: Self::divider_to_hz(self.blob[OFF_DIVIDER]),
            lod: self.blob[OFF_LOD],
            slamclick: self.blob[OFF_FLAGS] & FLAG_SLAMCLICK != 0,
            jitter: self.blob[OFF_FLAGS] & FLAG_JITTER != 0,
            angle_snapping: self.blob[OFF_ANGLE] != 0,
            ripple: self.blob[OFF_RIPPLE] != 0,
            motion_sync: self.blob[OFF_MOTION] != 0,
            cpi_levels: self.blob[OFF_CPI_LEVELS],
            cpis,
            buttons,
        }
    }
    pub fn apply_settings(&mut self, s: &DeviceSettings) {
        self.blob[OFF_DIVIDER] = Self::hz_to_divider(s.polling_hz);
        let mut flags = self.blob[OFF_FLAGS] & !(FLAG_SLAMCLICK | FLAG_JITTER);
        if s.slamclick { flags |= FLAG_SLAMCLICK; }
        if s.jitter { flags |= FLAG_JITTER; }
        self.blob[OFF_FLAGS] = flags;
        self.blob[OFF_LOD] = s.lod;
        self.blob[OFF_ANGLE] = s.angle_snapping as u8;
        self.blob[OFF_RIPPLE] = s.ripple as u8;
        self.blob[OFF_MOTION] = s.motion_sync as u8;
        self.blob[OFF_CPI_LEVELS] = s.cpi_levels;
        for (i, c) in s.cpis.iter().enumerate() {
            let base = OFF_CPIS + i * 5;
            self.blob[base] = c.split as u8;
            self.blob[base + 1..base + 3].copy_from_slice(&c.x.to_le_bytes());
            self.blob[base + 3..base + 5].copy_from_slice(&c.y.to_le_bytes());
        }
        for (i, b) in s.buttons.iter().enumerate() {
            b.to_blob(&mut self.blob, BUTTON_OFFSETS[i]);
        }
    }
}

/// Transport trait: feature-report exchange.
pub trait Transport {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String>;
    fn get_feature(&mut self, report_id: u8, size: usize) -> Result<Vec<u8>, String>;
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String> {
        (**self).set_feature(report_id, payload)
    }
    fn get_feature(&mut self, report_id: u8, size: usize) -> Result<Vec<u8>, String> {
        (**self).get_feature(report_id, size)
    }
}

pub struct Device<T: Transport> {
    pub t: T,
}

impl<T: Transport> Device<T> {
    pub fn read_config(&mut self) -> Result<Config, String> {
        self.t.set_feature(0xA1, &[0x12])?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        let blob = self.t.get_feature(0xA1, CONFIG_SIZE)?;
        if blob.len() < CONFIG_SIZE {
            return Err(format!("config read: got {} bytes", blob.len()));
        }
        let mut arr = [0u8; CONFIG_SIZE];
        arr.copy_from_slice(&blob[..CONFIG_SIZE]);
        Ok(Config { blob: arr })
    }

    pub fn get_fw_version(&mut self) -> Result<String, String> {
        self.t.set_feature(0xA1, &[0x02])?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        let r = self.t.get_feature(0xA1, 64)?;
        if r.len() < 19 || r[1] != 1 {
            return Err("fw version read failed".into());
        }
        Ok(format!("{}.{} (id 0x{:02x}{:02x})", r[17], r[18], r[17], r[18]))
    }

    /// [A1][14 0F 1C] + 32B sensor/CPI block
    ///
    /// Block layout (verified on hardware):
    ///   block[0..5]  -> cfg[0x17..0x1C]   block[6] -> cfg[0x1E]
    ///   block[7]     -> cfg[0x1D]         block[8..28] -> cfg[0x33..0x47] (CPI)
    /// (block[28..32] is ignored by the device).
    pub fn write_sensor(&mut self, cfg: &Config) -> Result<(), String> {
        let b = &cfg.blob;
        let mut block = [0u8; 32];
        block[0..5].copy_from_slice(&b[0x17..0x1C]);
        block[6] = b[0x1E];
        block[7] = b[0x1D];
        for i in 0..CPI_COUNT {
            let base = OFF_CPIS + i * 5;
            let dst = 8 + i * 5;
            block[dst] = b[base];
            block[dst + 1..dst + 5].copy_from_slice(&b[base + 1..base + 5]);
        }
        let mut buf = [0u8; 64];
        buf[0] = 0xA1;
        buf[1..4].copy_from_slice(&[0x14, 0x0F, 0x1C]);
        buf[16..48].copy_from_slice(&block);
        self.t.set_feature(0xA1, &buf[1..])?;
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.t.get_feature(0xA1, 64)?;
        Ok(())
    }

    /// [A1][16 0F 1C] + 2 x 28B button parts (cfg[0x47..0x7F]).
    /// Part 1 covers slots Right/Middle/Back + Forward's debounce area start;
    /// part 2 covers Forward/DPI/ScrollUp/ScrollDown. The Left button lives
    /// at 0x7F, outside the writable delta (it is not rebindable).
    pub fn write_buttons(&mut self, cfg: &Config) -> Result<(), String> {
        for part in 1..=2u8 {
            let mut buf = [0u8; 64];
            buf[0] = 0xA1;
            buf[1..4].copy_from_slice(&[0x16, 0x0F, 0x1C]);
            buf[6] = part;
            let src = 0x47 + (part as usize - 1) * 28;
            buf[16..44].copy_from_slice(&cfg.blob[src..src + 28]);
            self.t.set_feature(0xA1, &buf[1..])?;
            std::thread::sleep(std::time::Duration::from_millis(100));
            self.t.get_feature(0xA1, 64)?;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(())
    }

    pub fn factory_reset(&mut self) -> Result<(), String> {
        self.t.set_feature(0xA1, &[0x13])?;
        std::thread::sleep(std::time::Duration::from_millis(400));
        self.t.get_feature(0xA1, 64)?;
        Ok(())
    }
}

/// Open a device on the current platform.
pub fn open_device() -> Result<Device<Box<dyn Transport>>, String> {
    #[cfg(target_os = "macos")]
    {
        let t = macos::MacTransport::open(PID_MOUSE)?;
        Ok(Device { t: Box::new(t) })
    }
    #[cfg(target_os = "linux")]
    {
        let t = linux::LinuxTransport::open(PID_MOUSE)?;
        Ok(Device { t: Box::new(t) })
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("unsupported platform".into())
    }
}
