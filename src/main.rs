mod args;
mod driver;
mod greet;
mod utils;

use crate::args::Args;
use crate::driver::{kill_pid, load_driver, unload_driver, Driver};
use crate::utils::check_pid;
use anyhow::anyhow;
use clap::Parser;
use std::ffi::CStr;
use std::fs;
use std::ops::Not;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::SyncSender;

/// Init whole program
fn init() -> anyhow::Result<PathBuf> {
    let driver = include_bytes!("../driver/superman.sys");

    greet::greeting();

    let mut path = dirs::cache_dir().unwrap_or("C:\\Windows".into());
    path.push("Temp");

    if path.exists().not() {
        fs::create_dir_all(&path)?;
    }

    path.push("superman");

    if path.exists().not() {
        fs::write(&path, driver)?;
    }

    Ok(path)
}

/// Init ctrl+C handler
fn init_ctrlc(sx: SyncSender<bool>) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        sx.send(true).unwrap();
    })?;

    Ok(())
}

fn main() {
    let args = Args::parse();
    let path = init().unwrap();
    let service_name = CStr::from_bytes_with_nul(b"superman\0").unwrap().to_owned();

    let driver = Driver::new(path, service_name);

    if let Err(e) = try_main(&args, &driver) {
        eprintln!("{}", e);
    }
    let _ = unload_driver(&driver);
}

fn try_main(args: &Args, driver: &Driver) -> anyhow::Result<()> {
    let (sx, rx) = mpsc::sync_channel(1);
    init_ctrlc(sx)?;

    if check_pid(args.pid).not() {
        return Err(anyhow!("[-]Process not exists!"));
    }

    load_driver(driver)?;

    kill_pid(args, driver, rx)?;
    Ok(())
}
