use buspirate_hal::{open, ConfigurationRequest};

fn main() {
    let mut bp = open("/dev/cu.usbmodem5buspirate3").unwrap();
    let response = bp.configure(ConfigurationRequest::builder().led_resume(true).build());
    println!("{response:?}");
}
