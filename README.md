# XM2w Control

A desktop app for the **Endgame Gear XM2w 4K** gaming mouse on macOS and Linux.

Reverse-engineered from the device firmware (FW 1.10) and the official
Endgame Gear configuration tool. Not affiliated with Endgame Gear.

## Features

- **DPI Levels** — four levels with independent X/Y splits (editable by
  double-clicking a level), polling rate (1000/2000/4000 Hz) and lift-off
  distance
- **Assignments** — rebind every button:
  - Left (fixed by hardware), Right, Middle
  - **Forward** and **Back** (side buttons — fully rebindable)
  - DPI cycle, Scroll Up/Down
  - Binds: mouse actions, keyboard keys & combos (Ctrl+W, Shift+F1, …), DPI
    cycle, scroll, disabled
- **Emulator mode** (`--emu`) — an in-memory fake mouse that replicates the
  firmware exactly, with press simulation (▶ button per row) so the whole app
  can be tested without the hardware
- Dark G-HUB-style UI, tooltips, auto product-view switching, EGG-brand
  accent

## Usage

```sh
# normal (opens a window)
cargo run --bin xm2w

# silent: server only, no window (keeps running in the background)
cargo run --bin xm2w -- --silent

# emulator mode (no mouse needed)
cargo run --bin xm2w -- --silent --emu
```

The app embeds a local HTTP server on `127.0.0.1` (random port, written to
`/tmp/xm2w_port.txt`) and the window loads the UI from it. There is no Tauri
IPC involved — the UI talks to the server over `fetch`.

## The protocol (reverse-engineered)

Feature reports on the vendor HID interface (usage page 0xFF01):

| Command | Purpose |
|---|---|
| `[A1][12]` | read config (1041-byte blob; 8192 with mirrors) |
| `[A1][02]` | firmware version |
| `[A1][13]` | factory reset |
| `[A1][14 0F 1C]` + 32B | sensor/CPI block |
| `[A1][16 0F 1C]` + 2×28B | button write (parts 1-2) |

Button struct (7 bytes): `[kind][v0][v1][v2][v3][v4][debounce]`

| Slot | Offset | Default |
|---|---|---|
| Right | 0x4E | Right click |
| Middle | 0x55 | Middle click |
| Back | 0x5C | Back |
| Forward | 0x63 | Forward |
| DPI | 0x6A | DPI cycle |
| Scroll Up / Down | 0x71 / 0x78 | scroll |
| Left | 0x7F | left click (outside the writable delta — not rebindable) |

Mouse codes: `0=Left 2=Right 4=Middle 8=Back 0x10=Forward`.
Keyboard codes are stored as HID usage − 1 (the device table is shifted).

## Development

```sh
cd src-tauri
cargo test --lib emu        # emulator + protocol tests
cargo build --bin xm2w
```

## License

MIT
