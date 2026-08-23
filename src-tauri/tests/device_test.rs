#[test]
fn open_and_read() {
    let mut dev = xm2w::xm2w::open_device().expect("open device");
    let fw = dev.get_fw_version().expect("fw");
    println!("fw: {fw}");
    let cfg = dev.read_config().expect("config");
    let s = cfg.to_settings();
    println!("polling: {}", s.polling_hz);
    println!("cpis: {:?}", s.cpis);
    println!("buttons: {}", s.buttons.len());
}
