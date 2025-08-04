use buspirate_hal::{open, Configuration};

fn main() {
    let mut bp = open("/dev/cu.usbmodem5buspirate3").unwrap();
    let response = bp.configure(Configuration::builder().led_resume(true).build());
    println!("{response:?}");
}
