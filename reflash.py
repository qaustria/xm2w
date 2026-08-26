#!/usr/bin/env python3
"""XM2w 4K dongle reflash — full official protocol, runnable on macOS.

Protocol reverse-engineered from Endgame.Gear.XM2w.4k.Firmware.Updater.v1.10.exe
(https://github.com/EndgameGear/XM2w-4k/releases/tag/Firmware_1.10):

  app device (PID 0x1968):
    [0xA0][0x00 x1040]  -> enter bootloader (dongle re-enumerates as 0x1967)
  bootloader (PID 0x1967):
    [0xA0][0x02 ...]    echo
    [0xA0][0x03 ...]    start flash session (chunk count at payload[0x10])
    [0xA0][0x06 ...]    one 1024-byte chunk (index = chunk_no + 52, checksum
                        over the 1024 payload bytes, 16-bit sum)
    [0xA0][0x09 ...]    run app (back to PID 0x1968)
  afterwards:
    [0xA1][0x13]        factory reset
    [0xA0][0x11 cfg]    restore saved config (report 0xA0 opcode 0x11)

The firmware image (fw_134.bin / fw_135.bin, 209920 bytes) is embedded in the
official updater exe (.rdata, fully byte-identical) — no download needed.

Usage:  python3 reflash.py [fw_135.bin] [config_backup.bin]
"""
import ctypes
import sys
import time
import binascii
import subprocess

sys.path.insert(0, "/Users/qstr/Desktop/Xm2w-Mac")
from xm2w_transport import XM2wTransport

VID = 0x3367
PID_APP = 0x1968
PID_BLDR = 0x1967
CHUNK = 1024

FW_FILE = sys.argv[1] if len(sys.argv) > 1 else "/Users/qstr/Desktop/Xm2w-Mac/fw_135.bin"
CFG_BACKUP = sys.argv[2] if len(sys.argv) > 2 else "/Users/qstr/Desktop/Xm2w-Mac/config_good.bin"

libusb = None


def usb_pids():
    """Enumerate Endgame Gear PIDs currently on the USB bus (via libusb)."""
    global libusb
    if libusb is None:
        libusb = ctypes.CDLL("/usr/local/opt/libusb/lib/libusb-1.0.dylib")
        libusb.libusb_init.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
        libusb.libusb_get_device_list.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))]
        libusb.libusb_get_device_list.restype = ctypes.c_ssize_t
        libusb.libusb_get_device_descriptor.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        libusb.libusb_open.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
        libusb.libusb_reset_device.argtypes = [ctypes.c_void_p]

        class DD(ctypes.Structure):
            _fields_ = [("bLength", ctypes.c_uint8), ("bDescriptorType", ctypes.c_uint8),
                        ("bcdUSB", ctypes.c_uint16), ("bDeviceClass", ctypes.c_uint8),
                        ("bDeviceSubClass", ctypes.c_uint8), ("bDeviceProtocol", ctypes.c_uint8),
                        ("bMaxPacketSize0", ctypes.c_uint8), ("idVendor", ctypes.c_uint16),
                        ("idProduct", ctypes.c_uint16), ("bcdDevice", ctypes.c_uint16),
                        ("iManufacturer", ctypes.c_uint8), ("iProduct", ctypes.c_uint8),
                        ("iSerialNumber", ctypes.c_uint8), ("bNumConfigurations", ctypes.c_uint8)]
        libusb._DD = DD

    ctx = ctypes.c_void_p()
    libusb.libusb_init(ctypes.byref(ctx))
    devs = ctypes.POINTER(ctypes.c_void_p)()
    n = libusb.libusb_get_device_list(ctx, ctypes.byref(devs))
    pids = []
    for i in range(n):
        dd = libusb._DD()
        libusb.libusb_get_device_descriptor(devs[i], ctypes.byref(dd))
        if dd.idVendor == VID:
            pids.append(dd.idProduct)
    return pids


def wait_pid(pid, timeout_s=30):
    for i in range(timeout_s):
        if pid in usb_pids():
            return True
        time.sleep(1)
    return False


def build_chunk_report(data1024, index):
    buf = bytearray(1041)
    buf[0] = 0xA0
    buf[1] = 0x06
    buf[2] = index & 0xFF
    buf[3] = (index >> 8) & 0xFF
    buf[16:1040] = data1024
    chk = sum(data1024) & 0xFFFF
    buf[4] = chk & 0xFF
    buf[5] = (chk >> 8) & 0xFF
    return bytes(buf)


def main():
    fw = open(FW_FILE, "rb").read()
    print(f"firmware: {FW_FILE} ({len(fw)} bytes = {(len(fw) + CHUNK - 1) // CHUNK} chunks)")
    print(f"current USB PIDs: {[hex(p) for p in usb_pids()]}")

    # 0) back up the current config from the app device
    backup = None
    try:
        with XM2wTransport(pid=PID_APP).open() as t:
            t.set_feature(0xA1, bytes([0x12]) + b"\x00" * 62)
            time.sleep(0.1)
            backup = bytes(t.get_feature(0xA1, 1041))
            open(CFG_BACKUP, "wb").write(backup)
            print(f"config backed up -> {CFG_BACKUP} ({len(backup)} bytes)")
    except Exception as e:
        print(f"config backup failed (continuing anyway): {e}")

    # 1) enter bootloader: [A0][3A] magic handshake, then [A0][0x00 x1040]
    try:
        with XM2wTransport(pid=PID_APP).open() as t:
            magic = bytes([0x3A, 0x00, 0x00, 0x00, 0x5A, 0xA5, 0x32]) + b"\x00" * 1033
            t.set_feature(0xA0, magic)
            time.sleep(0.15)
            r = t.get_feature(0xA0, 1041)
            print(f"A0/3A magic resp: {r[:16].hex()}")
            t.set_feature(0xA0, b"\x00" * 1040)
            print("enter-bootloader sent ([0xA0][0x00 x1040])")
    except Exception as e:
        print(f"enter-bootloader failed: {e}")
    time.sleep(2)
    if not wait_pid(PID_BLDR, 10):
        print("ERROR: bootloader (0x1967) did not appear")
        sys.exit(1)
    print("BOOTLOADER PRESENT (0x1967)")

    # 2) echo
    with XM2wTransport(pid=PID_BLDR).open() as t:
        t.set_feature(0xA0, bytes([0x02]) + b"\x00" * 1040)
        time.sleep(0.05)
        r = t.get_feature(0xA0, 1041)
        print(f"echo [A0][02]: {r[:8].hex()} (expect 50 01...)")
        if r[1] != 0x01:
            print("echo failed - aborting")
            sys.exit(1)

        # 3) start flash session
        n_chunks = (len(fw) + CHUNK - 1) // CHUNK
        payload = bytearray(1041)
        payload[0] = 0x03
        payload[0x10] = n_chunks & 0xFF
        t.set_feature(0xA0, bytes(payload[1:]))
        time.sleep(0.1)
        r = t.get_feature(0xA0, 1041)
        print(f"bldr start [A0][03]: {r[:8].hex()} (50 07 = session already open, continuing)")
        if r[1] not in (0x01, 0x07):
            print("bldr start failed - aborting")
            sys.exit(1)

        # 4) flash chunks
        for i in range(n_chunks):
            chunk = fw[i * CHUNK:(i + 1) * CHUNK]
            if len(chunk) < CHUNK:
                chunk = chunk + b"\xFF" * (CHUNK - len(chunk))
            report = build_chunk_report(chunk, i + 52)
            ok = False
            for attempt in range(5):
                t.set_feature(0xA0, report[1:])
                time.sleep(0.03)
                r = t.get_feature(0xA0, 1041)
                if r and r[1] == 0x01:
                    ok = True
                    break
                time.sleep(0.1)
            if not ok:
                print(f"chunk {i}/{n_chunks} FAILED - aborting (rerun to recover)")
                sys.exit(1)
            if i % 16 == 0 or i == n_chunks - 1:
                print(f"  chunk {i + 1}/{n_chunks} ok")
            time.sleep(0.01)

        # 5) run app
        time.sleep(0.2)
        t.set_feature(0xA0, bytes([0x09]) + b"\x00" * 1040)
        print("run-app sent, waiting for PID 0x1968 ...")
    time.sleep(1)
    if not wait_pid(PID_APP, 30):
        print("ERROR: device did not return as 0x1968")
        sys.exit(1)
    print("DEVICE IS BACK AS 0x1968 - FLASH COMPLETE")

    # 6) factory reset + restore config
    time.sleep(1)
    try:
        with XM2wTransport(pid=PID_APP).open() as t:
            t.set_feature(0xA1, bytes([0x13]) + b"\x00" * 62)
            time.sleep(0.3)
            r = t.get_feature(0xA1, 64)
            print(f"factory reset [A1][13]: {r[:8].hex()}")
            if backup:
                t.set_feature(0xA0, bytes([0x11]) + bytes(backup[2:]))
                time.sleep(0.15)
                t.set_feature(0xA1, bytes([0x12]) + b"\x00" * 62)
                time.sleep(0.1)
                cfg2 = t.get_feature(0xA1, 1041)
                diff = [i for i in range(len(backup)) if i < len(cfg2) and cfg2[i] != backup[i]]
                print(f"config restored: {len(diff)} differing bytes {diff[:10]}")
    except Exception as e:
        print(f"post-flash step failed: {e}")
    print("DONE - unplug/replug the dongle and test")


if __name__ == "__main__":
    main()
