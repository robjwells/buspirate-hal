fn main() -> anyhow::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    buspirate_hal::open(&path)?.selftest()?;

    Ok(())
}
